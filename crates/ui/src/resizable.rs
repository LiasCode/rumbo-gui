//! Panels side by side, or one above another, with a divider between each
//! two. A panel is given a size, or takes what the sized ones leave; a
//! divider is dragged to change the size of the panel beside it on the side
//! away from the first that takes what is left, which gives or takes the
//! difference. The keyboard reaches a divider with Tab and moves it with
//! the arrows, and Home and End take it as far as it goes.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    Along as _, AnyElement, App, Axis, CursorStyle, ElementId, InteractiveElement as _,
    IntoElement, KeyBinding, ParentElement, Pixels, Refineable as _, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, actions, canvas, div, prelude::FluentBuilder as _, px,
};

use crate::drag::{self, Hold};
use crate::focus;
use crate::theme::ActiveTheme as _;
use crate::{ChangeHandler, child_id};

actions!(resizable, [Back, Forward, ToStart, ToEnd]);

/// A divider's key context.
const CONTEXT: &str = "Divider";

/// How thick a divider's line is, and the strip across it the pointer
/// takes it in.
const LINE: Pixels = px(1.);
const GRIP: Pixels = px(8.);

/// How far an arrow moves a divider.
const STEP: Pixels = px(16.);

/// How far Home and End move a divider: further than any panel goes.
const FAR: Pixels = px(100_000.);

/// The name a divider's strip goes by, for its mark to show under the
/// pointer.
const GROUP: &str = "resizable-divider";

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);

    cx.bind_keys([
        KeyBinding::new("left", Back, context),
        KeyBinding::new("up", Back, context),
        KeyBinding::new("right", Forward, context),
        KeyBinding::new("down", Forward, context),
        KeyBinding::new("home", ToStart, context),
        KeyBinding::new("end", ToEnd, context),
    ]);
}

/// The size a divider gave a panel, known by its key.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelSize {
    pub key: SharedString,
    pub size: Pixels,
}

/// The room a panel takes: its size, or none for one that takes what is
/// left, and how small and how large a divider makes it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Room {
    size: Option<Pixels>,
    min: Pixels,
    max: Option<Pixels>,
}

impl Room {
    /// Its size within its least and its most.
    fn clamped(self) -> Pixels {
        let size = self.size.unwrap_or(self.min).max(self.min);

        match self.max {
            Some(max) => size.min(max.max(self.min)),
            None => size,
        }
    }
}

/// Which panels take what the sized ones leave: those with no size, or the
/// last one when every panel has a size.
fn flexible(rooms: &[Room]) -> Vec<bool> {
    let any = rooms.iter().any(|room| room.size.is_none());

    rooms
        .iter()
        .enumerate()
        .map(|(index, room)| match any {
            true => room.size.is_none(),
            false => index + 1 == rooms.len(),
        })
        .collect()
}

/// The panel the divider after panel `divider` resizes: the one before it
/// while the first panel that takes what is left is after it, and the one
/// after it otherwise.
fn resized(rooms: &[Room], divider: usize) -> usize {
    let first = flexible(rooms)
        .iter()
        .position(|flexible| *flexible)
        .unwrap_or(0);

    match divider < first {
        true => divider,
        false => divider + 1,
    }
}

/// What a divider holds while it is moved: where the pointer took it, the
/// size of the panel it resizes then, how small and large that panel can
/// get, and whether it grows as the divider moves toward the end.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Grip {
    origin: Pixels,
    start: Pixels,
    least: Pixels,
    most: Pixels,
    forward: bool,
}

impl Grip {
    /// The panel's size for the divider `moved` from where it was taken.
    fn size_at(&self, moved: Pixels) -> Pixels {
        let size = match self.forward {
            true => self.start + moved,
            false => self.start - moved,
        };

        size.max(self.least).min(self.most)
    }
}

/// The grip of the divider after panel `divider`, in a group `length`
/// long, taken at `origin`. A panel grows only into what the panels that
/// take what is left have over their least.
fn grip(rooms: &[Room], divider: usize, length: Pixels, origin: Pixels) -> Grip {
    let flexible = flexible(rooms);
    let panel = resized(rooms, divider);
    let room = rooms[panel];

    let lines = LINE * rooms.len().saturating_sub(1) as f32;
    let sized: Pixels = rooms
        .iter()
        .zip(&flexible)
        .filter(|(_, flexible)| !**flexible)
        .map(|(room, _)| room.clamped())
        .sum();
    let free = (length - lines - sized).max(Pixels::ZERO);
    let shares = flexible.iter().filter(|flexible| **flexible).count().max(1);

    let start = match flexible[panel] {
        true => free / shares as f32,
        false => room.clamped(),
    };
    let least_of_others: Pixels = rooms
        .iter()
        .zip(&flexible)
        .enumerate()
        .filter(|(index, (_, flexible))| **flexible && *index != panel)
        .map(|(_, (room, _))| room.min)
        .sum();
    let left_to_others = match flexible[panel] {
        true => free - start,
        false => free,
    };
    let spare = (left_to_others - least_of_others).max(Pixels::ZERO);

    let most = match room.max {
        Some(max) => (start + spare).min(max),
        None => start + spare,
    };

    Grip {
        origin,
        start,
        least: room.min,
        most: most.max(room.min),
        forward: panel == divider,
    }
}

/// One panel of a group, known by its key.
pub struct ResizablePanel {
    key: SharedString,
    room: Room,
    children: Vec<AnyElement>,
}

impl ResizablePanel {
    /// A panel that takes what the sized ones leave, until it is given a
    /// size.
    pub fn new(key: impl Into<SharedString>) -> Self {
        ResizablePanel {
            key: key.into(),
            room: Room {
                size: None,
                min: px(48.),
                max: None,
            },
            children: Vec::new(),
        }
    }

    /// Its size along the group, which a divider changes.
    pub fn size(mut self, size: Pixels) -> Self {
        self.room.size = Some(size);
        self
    }

    /// The least a divider makes it; 48 unless said.
    pub fn min(mut self, min: Pixels) -> Self {
        self.room.min = min;
        self
    }

    /// The most a divider makes it.
    pub fn max(mut self, max: Pixels) -> Self {
        self.room.max = Some(max);
        self
    }
}

impl ParentElement for ResizablePanel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

#[derive(IntoElement)]
pub struct Resizable {
    id: ElementId,
    axis: Axis,
    panels: Vec<ResizablePanel>,
    on_resize: Option<ChangeHandler<PanelSize>>,
    style: StyleRefinement,
}

impl Resizable {
    /// Panels side by side, which fill their parent.
    pub fn horizontal(id: impl Into<ElementId>) -> Self {
        Resizable::new(id.into(), Axis::Horizontal)
    }

    /// Panels one above another, which fill their parent.
    pub fn vertical(id: impl Into<ElementId>) -> Self {
        Resizable::new(id.into(), Axis::Vertical)
    }

    fn new(id: ElementId, axis: Axis) -> Self {
        Resizable {
            id,
            axis,
            panels: Vec::new(),
            on_resize: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn child(mut self, panel: ResizablePanel) -> Self {
        self.panels.push(panel);
        self
    }

    pub fn children(mut self, panels: impl IntoIterator<Item = ResizablePanel>) -> Self {
        self.panels.extend(panels);
        self
    }

    /// Called with the size a divider gave a panel; the caller keeps it
    /// and gives it back with the panel's `size`. Without it the dividers
    /// do not move.
    pub fn on_resize(
        mut self,
        handler: impl Fn(&PanelSize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize = Some(Rc::new(handler));
        self
    }
}

impl Styled for Resizable {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// What every divider of a group reads when it moves.
#[derive(Clone)]
struct Shared {
    axis: Axis,
    rooms: Rc<[Room]>,
    keys: Rc<[SharedString]>,
    /// How long the group was when it was last laid out.
    length: Rc<Cell<Pixels>>,
    on_resize: Option<ChangeHandler<PanelSize>>,
}

impl Shared {
    /// Gives the panel the divider after panel `divider` resizes the size
    /// `grip` takes it to when moved `moved`, if that changes it.
    fn resize(
        &self,
        divider: usize,
        grip: &Grip,
        moved: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(on_resize) = &self.on_resize else {
            return;
        };

        let panel = resized(&self.rooms, divider);
        let size = grip.size_at(moved);

        if self.rooms[panel].size != Some(size) {
            let key = self.keys[panel].clone();

            on_resize(&PanelSize { key, size }, window, cx);
        }
    }

    /// Moves the divider after panel `divider` by `moved`, as a key does.
    fn nudge(&self, divider: usize, moved: Pixels, window: &mut Window, cx: &mut App) {
        let grip = grip(&self.rooms, divider, self.length.get(), Pixels::ZERO);

        self.resize(divider, &grip, moved, window, cx);
    }
}

impl RenderOnce for Resizable {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let axis = self.axis;
        let colors = cx.theme().colors;
        let ring = cx.theme().focus_ring();
        let flexible = flexible(
            &self
                .panels
                .iter()
                .map(|panel| panel.room)
                .collect::<Vec<_>>(),
        );

        let length = window
            .use_keyed_state(child_id(&self.id, "length"), cx, |_, _| {
                Rc::new(Cell::new(Pixels::ZERO))
            })
            .read(cx)
            .clone();
        let measured = length.clone();

        let shared = Shared {
            axis,
            rooms: self.panels.iter().map(|panel| panel.room).collect(),
            keys: self.panels.iter().map(|panel| panel.key.clone()).collect(),
            length,
            on_resize: self.on_resize,
        };

        let mut group = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .size_full()
            .overflow_hidden()
            .map(|group| match axis {
                Axis::Horizontal => group.flex_row(),
                Axis::Vertical => group.flex_col(),
            })
            .child(
                canvas(
                    move |bounds, _, _| measured.set(bounds.size.along(axis)),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );

        for (index, panel) in self.panels.into_iter().enumerate() {
            let room = panel.room;
            let content = div().size_full().overflow_hidden().children(panel.children);

            let mut outer = div()
                .relative()
                .map(|outer| match (axis, flexible[index]) {
                    (Axis::Horizontal, true) => outer.flex_1().min_w(room.min).h_full(),
                    (Axis::Horizontal, false) => outer.flex_none().w(room.clamped()).h_full(),
                    (Axis::Vertical, true) => outer.flex_1().min_h(room.min).w_full(),
                    (Axis::Vertical, false) => outer.flex_none().h(room.clamped()).w_full(),
                })
                .child(content);

            // The divider before this panel: its line, which the keyboard
            // takes, and the strip the pointer takes it in, which is this
            // panel's last child so that it lies over both panels.
            if index > 0 {
                let divider = index - 1;
                let name = &shared.keys[divider];
                let focus =
                    focus::focus_of(&child_id(&self.id, format!("line-{name}")), window, cx);
                let ringed = focus::ringed(&focus, window);
                let hold: Hold<Grip> = window.use_keyed_state(
                    child_id(&self.id, format!("grip-{name}")),
                    cx,
                    |_, _| None,
                );
                let held = hold.read(cx).is_some();
                let movable = shared.on_resize.is_some();

                let mut line = div()
                    .id(child_id(&self.id, format!("divider-{name}")))
                    .flex_none()
                    .bg(colors.border)
                    .map(|line| match axis {
                        Axis::Horizontal => line.w(LINE).h_full(),
                        Axis::Vertical => line.h(LINE).w_full(),
                    });

                if movable {
                    let nudge = |moved: Pixels| {
                        let shared = shared.clone();

                        move |window: &mut Window, cx: &mut App| {
                            shared.nudge(divider, moved, window, cx)
                        }
                    };
                    let back = nudge(-STEP);
                    let forward = nudge(STEP);
                    let to_start = nudge(-FAR);
                    let to_end = nudge(FAR);

                    line = line
                        .key_context(CONTEXT)
                        .track_focus(&focus)
                        .on_action(move |_: &Back, window, cx| back(window, cx))
                        .on_action(move |_: &Forward, window, cx| forward(window, cx))
                        .on_action(move |_: &ToStart, window, cx| to_start(window, cx))
                        .on_action(move |_: &ToEnd, window, cx| to_end(window, cx));
                }

                // A mark on the line, which shows under the pointer and
                // while the divider is held or has the keyboard.
                let mark = div()
                    .flex_none()
                    .rounded_full()
                    .bg(colors.muted_foreground)
                    .map(|mark| match axis {
                        Axis::Horizontal => mark.w(px(3.)).h(px(32.)),
                        Axis::Vertical => mark.h(px(3.)).w(px(32.)),
                    })
                    .map(|mark| match held || ringed {
                        true => mark.opacity(0.9),
                        false => mark.opacity(0.).when(movable, |mark| {
                            mark.group_hover(GROUP, |style| style.opacity(0.5))
                        }),
                    })
                    .when(ringed, |mark| mark.shadow(ring.clone()));

                let offset = -(GRIP + LINE) / 2.;
                let mut strip = div()
                    .absolute()
                    .flex()
                    .items_center()
                    .justify_center()
                    .group(GROUP)
                    .map(|strip| match axis {
                        Axis::Horizontal => strip.top_0().bottom_0().left(offset).w(GRIP),
                        Axis::Vertical => strip.left_0().right_0().top(offset).h(GRIP),
                    })
                    .child(mark);

                if movable {
                    let pressed = shared.clone();
                    let dragged = shared.clone();
                    let cursor = match axis {
                        Axis::Horizontal => CursorStyle::ResizeColumn,
                        Axis::Vertical => CursorStyle::ResizeRow,
                    };

                    strip = strip.child(drag::area(
                        hold,
                        cursor,
                        move |at, _, _, _| {
                            Some(grip(
                                &pressed.rooms,
                                divider,
                                pressed.length.get(),
                                at.along(pressed.axis),
                            ))
                        },
                        move |grip, at, _, window, cx| {
                            let moved = at.along(dragged.axis) - grip.origin;

                            dragged.resize(divider, grip, moved, window, cx);
                        },
                    ));
                }

                group = group.child(line);
                outer = outer.child(strip);
            }

            group = group.child(outer);
        }

        group.style().refine(&self.style);
        group
    }
}

#[cfg(test)]
mod tests {
    use gpui::px;

    use super::{Room, flexible, grip, resized};

    fn sized(size: f32, min: f32, max: Option<f32>) -> Room {
        Room {
            size: Some(px(size)),
            min: px(min),
            max: max.map(px),
        }
    }

    fn rest(min: f32) -> Room {
        Room {
            size: None,
            min: px(min),
            max: None,
        }
    }

    #[test]
    fn leaves_what_is_left_to_the_panels_with_no_size() {
        assert_eq!(
            flexible(&[sized(200., 0., None), rest(0.), sized(100., 0., None)]),
            [false, true, false]
        );

        // With a size for each, the last takes what is left.
        assert_eq!(
            flexible(&[sized(200., 0., None), sized(100., 0., None)]),
            [false, true]
        );
    }

    #[test]
    fn resizes_the_panel_away_from_what_takes_what_is_left() {
        let side = sized(200., 0., None);

        // A sidebar, the work, and a panel at the other side.
        assert_eq!(resized(&[side, rest(0.), side], 0), 0);
        assert_eq!(resized(&[side, rest(0.), side], 1), 2);

        // Two sized before it, or two after.
        assert_eq!(resized(&[side, side, rest(0.)], 1), 1);
        assert_eq!(resized(&[rest(0.), side, side], 0), 1);
        assert_eq!(resized(&[rest(0.), side, side], 1), 2);

        // Between two that take what is left, the one after it.
        assert_eq!(resized(&[rest(0.), rest(0.)], 0), 1);
    }

    #[test]
    fn grows_a_panel_into_what_the_others_spare() {
        // 801 long: a 200 panel, a line, and 600 left over a least of 200.
        let rooms = [sized(200., 100., Some(500.)), rest(200.)];
        let taken = grip(&rooms, 0, px(801.), px(200.));

        assert_eq!(taken.start, px(200.));
        assert_eq!(taken.size_at(px(50.)), px(250.));
        assert_eq!(taken.size_at(px(1000.)), px(500.));
        assert_eq!(taken.size_at(px(-1000.)), px(100.));

        // Its most holds before what is spare.
        let rooms = [sized(200., 100., Some(300.)), rest(200.)];

        assert_eq!(
            grip(&rooms, 0, px(801.), px(0.)).size_at(px(500.)),
            px(300.)
        );

        // With nothing spare it does not grow, but shrinks.
        let taken = grip(&rooms, 0, px(401.), px(0.));

        assert_eq!(taken.size_at(px(50.)), px(200.));
        assert_eq!(taken.size_at(px(-50.)), px(150.));
    }

    #[test]
    fn shrinks_a_panel_after_its_divider_as_the_divider_moves_on() {
        let rooms = [rest(100.), sized(200., 50., None)];
        let grip = grip(&rooms, 0, px(601.), px(0.));

        assert_eq!(grip.size_at(px(30.)), px(170.));
        assert_eq!(grip.size_at(px(1000.)), px(50.));
        assert_eq!(grip.size_at(px(-1000.)), px(500.));
    }

    #[test]
    fn sizes_a_panel_that_took_what_was_left() {
        // Two halves of 200 over a least of 100 each.
        let rooms = [rest(100.), rest(100.)];
        let grip = grip(&rooms, 0, px(401.), px(0.));

        assert_eq!(grip.start, px(200.));
        assert_eq!(grip.size_at(px(50.)), px(150.));
        assert_eq!(grip.size_at(px(-500.)), px(300.));
    }
}

/// The pointer and the keys on a divider, in a window GPUI keeps in the
/// test's own process.
#[cfg(test)]
mod pointer_tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        Context, IntoElement, Modifiers, MouseButton, ParentElement as _, Pixels, Render,
        Styled as _, TestAppContext, VisualTestContext, Window, div, point, px,
    };

    use super::{PanelSize, Resizable, ResizablePanel};

    /// A group 601 wide at the window's corner: a sidebar 200 wide, kept
    /// here, between 100 and 400, then a line at 200, and the rest.
    struct Shell {
        sidebar: Rc<Cell<Pixels>>,
    }

    impl Render for Shell {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let sidebar = self.sidebar.clone();

            div().size_full().child(
                div().w(px(601.)).h(px(300.)).child(
                    Resizable::horizontal("shell")
                        .child(
                            ResizablePanel::new("sidebar")
                                .size(self.sidebar.get())
                                .min(px(100.))
                                .max(px(400.)),
                        )
                        .child(ResizablePanel::new("work").min(px(150.)))
                        .on_resize(cx.listener(move |_, resized: &PanelSize, _, cx| {
                            assert_eq!(resized.key.as_ref(), "sidebar");
                            sidebar.set(resized.size);
                            cx.notify();
                        })),
                ),
            )
        }
    }

    fn shell(cx: &mut TestAppContext) -> (Rc<Cell<Pixels>>, &mut VisualTestContext) {
        cx.update(crate::theme::init);
        cx.update(super::bind_keys);

        let sidebar = Rc::new(Cell::new(px(200.)));
        let kept = sidebar.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Shell { sidebar: kept });

        (sidebar, cx)
    }

    #[gpui::test]
    fn drags_the_divider_as_far_as_the_panels_let_it(cx: &mut TestAppContext) {
        let (sidebar, cx) = shell(cx);
        let none = Modifiers::none();

        // Taken beside its line, on the side of the panel after it.
        cx.simulate_mouse_move(point(px(203.), px(100.)), None, none);
        cx.simulate_mouse_down(point(px(203.), px(100.)), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(263.), px(100.)), MouseButton::Left, none);

        assert_eq!(sidebar.get(), px(260.));

        // The work keeps its 150, so the sidebar stops at 450 less that,
        // which is under its own most of 400.
        cx.simulate_mouse_move(point(px(590.), px(100.)), MouseButton::Left, none);

        assert_eq!(sidebar.get(), px(400.));

        cx.simulate_mouse_move(point(px(10.), px(100.)), MouseButton::Left, none);

        assert_eq!(sidebar.get(), px(100.));

        cx.simulate_mouse_up(point(px(10.), px(100.)), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(300.), px(100.)), None, none);

        assert_eq!(sidebar.get(), px(100.));
    }

    #[gpui::test]
    fn the_keyboard_moves_the_divider(cx: &mut TestAppContext) {
        let (sidebar, cx) = shell(cx);

        // The divider is the only control, so Tab takes it.
        cx.update(|window, cx| window.focus_next(cx));
        cx.simulate_keystrokes("right right");

        assert_eq!(sidebar.get(), px(232.));

        cx.simulate_keystrokes("home");

        assert_eq!(sidebar.get(), px(100.));

        cx.simulate_keystrokes("end");

        assert_eq!(sidebar.get(), px(400.));
    }
}
