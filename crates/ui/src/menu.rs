//! A menu of what can be done, which opens under the button that has it or
//! where the pointer pressed with its other button: each item with its
//! icon, name, and keys, some set apart by a line, and words at its head
//! that say how something stands. Up and Down go through the items past
//! those disabled, coming round at the ends, Enter does the one marked,
//! and Escape or a press outside closes it. Shift-F10 opens a context menu
//! from the keyboard.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, ElementId, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, KeyBinding, MouseButton, ParentElement as _, Pixels,
    Point, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, actions,
    anchored, deferred, div, point, prelude::FluentBuilder as _, px,
};

use crate::child_id;
use crate::icon::{Icon, IconName};
use crate::overlay::{self, Dismiss};
use crate::popover::{Placement, PopoverTrigger};
use crate::root::{h_flex, v_flex};
use crate::roving::{self, Step};
use crate::size::Sizable as _;
use crate::theme::ActiveTheme as _;

actions!(
    menu,
    [Previous, Next, First, Last, Confirm, OpenContextMenu]
);

/// A menu's key context, and that of what has a context menu.
const CONTEXT: &str = "Menu";
const CONTEXT_MENU_CONTEXT: &str = "ContextMenu";

/// How tall an item is.
const ITEM: Pixels = px(32.);

pub(crate) fn bind_keys(cx: &mut App) {
    let menu = Some(CONTEXT);

    cx.bind_keys([
        KeyBinding::new("up", Previous, menu),
        KeyBinding::new("down", Next, menu),
        KeyBinding::new("home", First, menu),
        KeyBinding::new("end", Last, menu),
        KeyBinding::new("enter", Confirm, menu),
        KeyBinding::new("space", Confirm, menu),
        KeyBinding::new("shift-f10", OpenContextMenu, Some(CONTEXT_MENU_CONTEXT)),
        KeyBinding::new("menu", OpenContextMenu, Some(CONTEXT_MENU_CONTEXT)),
    ]);
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One line of a menu.
pub enum MenuItem {
    /// Something to do.
    Action {
        label: SharedString,
        icon: Option<IconName>,
        shortcut: Option<SharedString>,
        disabled: bool,
        danger: bool,
        checked: bool,
        on_click: Option<Handler>,
    },
    /// A line between items that go apart.
    Separator,
    /// Words that say how something stands, which do nothing.
    Label(SharedString),
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        MenuItem::Action {
            label: label.into(),
            icon: None,
            shortcut: None,
            disabled: false,
            danger: false,
            checked: false,
            on_click: None,
        }
    }

    pub fn separator() -> Self {
        MenuItem::Separator
    }

    pub fn label(text: impl Into<SharedString>) -> Self {
        MenuItem::Label(text.into())
    }

    pub fn icon(mut self, name: IconName) -> Self {
        if let MenuItem::Action { icon, .. } = &mut self {
            *icon = Some(name);
        }
        self
    }

    /// The keys that do the same, said at its end.
    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        if let MenuItem::Action { shortcut, .. } = &mut self {
            *shortcut = Some(keys.into());
        }
        self
    }

    pub fn disabled(mut self, is: bool) -> Self {
        if let MenuItem::Action { disabled, .. } = &mut self {
            *disabled = is;
        }
        self
    }

    /// What cannot be undone, such as deleting, in the danger's color.
    pub fn danger(mut self) -> Self {
        if let MenuItem::Action { danger, .. } = &mut self {
            *danger = true;
        }
        self
    }

    /// A tick before it, for a choice that is on.
    pub fn checked(mut self, is: bool) -> Self {
        if let MenuItem::Action { checked, .. } = &mut self {
            *checked = is;
        }
        self
    }

    /// What it does. The menu closes before it is called.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        if let MenuItem::Action { on_click, .. } = &mut self {
            *on_click = Some(Rc::new(handler));
        }
        self
    }

    /// Whether it can be marked and done.
    fn is_open(&self) -> bool {
        matches!(
            self,
            MenuItem::Action {
                disabled: false,
                ..
            }
        )
    }
}

type Items = Rc<dyn Fn(&mut Window, &mut App) -> Vec<MenuItem>>;

/// Where an open menu stands.
#[derive(Clone, Copy)]
enum Place {
    /// Beside its button, as placed.
    Trigger(Placement),
    /// Where the pointer pressed, in the window.
    At(Point<Pixels>),
}

/// What a menu remembers between frames.
struct MenuState {
    open: Option<Place>,
    /// The item the keys or the pointer are on.
    marked: Option<usize>,
    /// Where what opens it was last painted.
    trigger: Bounds<Pixels>,
    focus: FocusHandle,
    /// What had the keyboard before it opened.
    restore: Option<FocusHandle>,
}

impl MenuState {
    fn new(cx: &mut Context<Self>) -> Self {
        MenuState {
            open: None,
            marked: None,
            trigger: Bounds::default(),
            focus: cx.focus_handle(),
            restore: None,
        }
    }
}

/// Opens it at `place`, marking `marked`, and gives it the keyboard.
fn open(
    state: &Entity<MenuState>,
    place: Place,
    marked: Option<usize>,
    window: &mut Window,
    cx: &mut App,
) {
    let restore = window.focused(cx);
    let focus = state.update(cx, |state, cx| {
        state.open = Some(place);
        state.marked = marked;
        state.restore = restore;
        cx.notify();
        state.focus.clone()
    });

    focus.focus(window, cx);
}

/// Closes it, and gives the keyboard back to where it was.
fn close(state: &Entity<MenuState>, window: &mut Window, cx: &mut App) {
    let restore = state.update(cx, |state, cx| {
        state.open = None;
        state.marked = None;
        cx.notify();
        state.restore.take()
    });

    if let Some(restore) = restore {
        restore.focus(window, cx);
    }
}

/// The first item that can be done, which the keyboard marks when it opens
/// a menu.
fn first_open(items: &[MenuItem]) -> Option<usize> {
    let open: Vec<bool> = items.iter().map(MenuItem::is_open).collect();

    roving::moved(None, &open, Step::First)
}

/// The open menu: its items on a surface, where it was opened.
fn surface(
    id: &ElementId,
    state: &Entity<MenuState>,
    items: Vec<MenuItem>,
    cx: &mut App,
) -> AnyElement {
    let (marked, focus) = {
        let state = state.read(cx);

        (state.marked, state.focus.clone())
    };

    let theme = cx.theme();
    let colors = theme.colors;
    let radius = theme.radius;
    let open: Rc<[bool]> = items.iter().map(MenuItem::is_open).collect();
    let handlers: Rc<[Option<Handler>]> = items
        .iter()
        .map(|item| match item {
            MenuItem::Action { on_click, .. } => on_click.clone(),
            _ => None,
        })
        .collect();

    // Items line up their names whether or not they have an icon.
    let gutter = items.iter().any(|item| {
        matches!(
            item,
            MenuItem::Action { icon: Some(_), .. } | MenuItem::Action { checked: true, .. }
        )
    });

    // Does the item at `index`: the menu closes, then it acts.
    let choose = {
        let state = state.clone();
        let handlers = handlers.clone();

        Rc::new(move |index: usize, window: &mut Window, cx: &mut App| {
            close(&state, window, cx);

            if let Some(Some(handler)) = handlers.get(index) {
                handler(window, cx);
            }
        })
    };

    let lines = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| match item {
            MenuItem::Separator => div()
                .my_1()
                .mx(px(-4.))
                .h(px(1.))
                .bg(colors.border)
                .into_any_element(),

            MenuItem::Label(text) => div()
                .px_2()
                .py_1p5()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child(text)
                .into_any_element(),

            MenuItem::Action {
                label,
                icon,
                shortcut,
                disabled,
                danger,
                checked,
                ..
            } => {
                let color = match danger {
                    true => colors.danger,
                    false => colors.popover_foreground,
                };
                let mark = match (checked, icon) {
                    (true, _) => Some(IconName::Check),
                    (false, icon) => icon,
                };
                let hovering = state.clone();
                let pressed = choose.clone();

                h_flex()
                    .id(child_id(id, format!("item-{index}")))
                    .debug_selector(|| format!("menu-item-{index}"))
                    .h(ITEM)
                    .px_2()
                    .gap_2()
                    .rounded(radius)
                    .text_sm()
                    .text_color(color)
                    .when(marked == Some(index), |line| line.bg(colors.accent))
                    .when(gutter, |line| {
                        line.child(
                            div()
                                .flex()
                                .flex_none()
                                .size(px(16.))
                                .items_center()
                                .justify_center()
                                .when_some(mark, |slot, mark| {
                                    slot.child(Icon::new(mark).small().color(match danger {
                                        true => colors.danger,
                                        false => colors.muted_foreground,
                                    }))
                                }),
                        )
                    })
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .when_some(shortcut, |line, keys| {
                        line.child(
                            div()
                                .flex_none()
                                .pl_4()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(keys),
                        )
                    })
                    .map(|line| match disabled {
                        true => line.opacity(0.5),
                        false => line
                            .cursor_pointer()
                            .on_mouse_move(move |_, _, cx| {
                                hovering.update(cx, |state, cx| {
                                    if state.marked != Some(index) {
                                        state.marked = Some(index);
                                        cx.notify();
                                    }
                                })
                            })
                            .on_click(move |_, window, cx| pressed(index, window, cx)),
                    })
                    .into_any_element()
            }
        });

    let keys = |step: Step| {
        let state = state.clone();
        let open = open.clone();

        move |_: &mut Window, cx: &mut App| {
            state.update(cx, |state, cx| {
                if let Some(to) = roving::moved(state.marked, &open, step) {
                    state.marked = Some(to);
                    cx.notify();
                }
            })
        }
    };

    let previous = keys(Step::Previous);
    let next = keys(Step::Next);
    let first = keys(Step::First);
    let last = keys(Step::Last);
    let confirming = (state.clone(), choose.clone());
    let dismissing = state.clone();
    let outside = state.clone();
    let trigger = state.read(cx).trigger;
    let place = state.read(cx).open;

    let list = v_flex()
        .id(child_id(id, "menu"))
        .min_w(px(180.))
        .max_w(px(320.))
        .p_1()
        .key_context(CONTEXT)
        .track_focus(&focus)
        .on_action(move |_: &Previous, window, cx| previous(window, cx))
        .on_action(move |_: &Next, window, cx| next(window, cx))
        .on_action(move |_: &First, window, cx| first(window, cx))
        .on_action(move |_: &Last, window, cx| last(window, cx))
        .on_action(move |_: &Confirm, window, cx| {
            if let Some(index) = confirming.0.read(cx).marked {
                (confirming.1)(index, window, cx);
            }
        })
        .children(lines);

    let layer = overlay::surface(cx)
        .key_context(overlay::POPOVER_CONTEXT)
        .occlude()
        .on_action(move |_: &Dismiss, window, cx| close(&dismissing, window, cx))
        // A press on what opened it closes it as its own press.
        .on_mouse_down_out(move |event, window, cx| {
            let on_trigger =
                matches!(place, Some(Place::Trigger(_))) && trigger.contains(&event.position);

            if !on_trigger {
                close(&outside, window, cx);
            }
        })
        .child(list);

    let placed = match place {
        Some(Place::At(at)) => anchored().position(at),
        _ => {
            let placement = match place {
                Some(Place::Trigger(placement)) => placement,
                _ => Placement::default(),
            };
            let (anchor, corner, offset) = placement.corners();

            anchored()
                .attach(trigger, corner)
                .anchor(anchor)
                .offset(offset)
        }
    };

    deferred(placed.snap_to_window_with_margin(px(8.)).child(layer))
        // Over a floating window, which a menu can open from.
        .with_priority(2)
        .into_any_element()
}

/// A button that opens a menu under it.
#[derive(IntoElement)]
pub struct DropdownMenu<T: PopoverTrigger> {
    id: ElementId,
    trigger: T,
    placement: Placement,
    items: Option<Items>,
}

impl<T: PopoverTrigger> DropdownMenu<T> {
    pub fn new(id: impl Into<ElementId>, trigger: T) -> Self {
        DropdownMenu {
            id: id.into(),
            trigger,
            placement: Placement::default(),
            items: None,
        }
    }

    /// Where it opens beside its button.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Its items, built while it is open.
    pub fn items(
        mut self,
        items: impl Fn(&mut Window, &mut App) -> Vec<MenuItem> + 'static,
    ) -> Self {
        self.items = Some(Rc::new(items));
        self
    }
}

impl<T: PopoverTrigger> RenderOnce for DropdownMenu<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| MenuState::new(cx));
        let is_open = state.read(cx).open.is_some();
        let items = self.items;

        let pressing = state.clone();
        let building = items.clone();
        let placement = self.placement;
        let trigger = self
            .trigger
            .open(is_open)
            .on_press(move |event: &ClickEvent, window, cx| {
                if pressing.read(cx).open.is_some() {
                    close(&pressing, window, cx);
                    return;
                }

                // Opened from the keyboard, it marks its first item.
                let marked = match (event.is_keyboard(), &building) {
                    (true, Some(items)) => first_open(&items(window, cx)),
                    _ => None,
                };

                open(&pressing, Place::Trigger(placement), marked, window, cx);
            });

        let tracking = state.clone();
        let wrapper = div()
            .child(trigger)
            .on_children_prepainted(move |bounds, _, cx| {
                if let Some(bounds) = bounds.first() {
                    tracking.update(cx, |state, _| state.trigger = *bounds);
                }
            });

        let Some(items) = items.filter(|_| is_open) else {
            return wrapper;
        };

        let built = items(window, cx);

        wrapper.child(surface(&self.id, &state, built, cx))
    }
}

/// What opens a menu where the pointer presses its other button on it, or
/// under it with Shift-F10 while it has the keyboard.
#[derive(IntoElement)]
pub struct ContextMenu {
    id: ElementId,
    child: AnyElement,
    items: Option<Items>,
}

impl ContextMenu {
    pub fn new(id: impl Into<ElementId>, child: impl IntoElement) -> Self {
        ContextMenu {
            id: id.into(),
            child: child.into_any_element(),
            items: None,
        }
    }

    /// Its items, built while it is open.
    pub fn items(
        mut self,
        items: impl Fn(&mut Window, &mut App) -> Vec<MenuItem> + 'static,
    ) -> Self {
        self.items = Some(Rc::new(items));
        self
    }
}

impl RenderOnce for ContextMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| MenuState::new(cx));
        let is_open = state.read(cx).open.is_some();
        let items = self.items;

        let pressing = state.clone();
        let keyed = state.clone();
        let building = items.clone();
        let tracking = state.clone();

        let wrapper = div()
            .on_children_prepainted(move |bounds, _, cx| {
                if let Some(bounds) = bounds.first() {
                    tracking.update(cx, |state, _| state.trigger = *bounds);
                }
            })
            .id(child_id(&self.id, "area"))
            .key_context(CONTEXT_MENU_CONTEXT)
            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                cx.stop_propagation();
                open(&pressing, Place::At(event.position), None, window, cx);
            })
            .on_action(move |_: &OpenContextMenu, window, cx| {
                let bounds = keyed.read(cx).trigger;
                let marked = building
                    .as_ref()
                    .and_then(|items| first_open(&items(window, cx)));
                let at = point(bounds.left(), bounds.bottom());

                open(&keyed, Place::At(at), marked, window, cx);
            })
            .child(self.child);

        match items.filter(|_| is_open) {
            Some(items) => {
                let built = items(window, cx);

                wrapper.child(surface(&self.id, &state, built, cx))
            }
            None => wrapper,
        }
    }
}

/// The pointer and the keys on menus, in a window GPUI keeps in the test's
/// own process.
#[cfg(test)]
mod pointer_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{
        Context, InteractiveElement as _, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke,
        Modifiers, MouseButton, ParentElement as _, Render, Styled as _, TestAppContext,
        VisualTestContext, Window, div, point, px,
    };

    use super::{ContextMenu, DropdownMenu, MenuItem};
    use crate::button::Button;

    type Told = Rc<RefCell<Vec<&'static str>>>;

    /// Rename, a disabled Branch, a line, and Delete.
    fn items(told: &Told) -> Vec<MenuItem> {
        let (renamed, deleted) = (told.clone(), told.clone());

        vec![
            MenuItem::new("Rename").on_click(move |_, _| renamed.borrow_mut().push("rename")),
            MenuItem::new("Branch").disabled(true),
            MenuItem::separator(),
            MenuItem::new("Delete").on_click(move |_, _| deleted.borrow_mut().push("delete")),
        ]
    }

    /// A button with a menu at the window's corner, and an area with a
    /// context menu under it.
    struct Session {
        told: Told,
    }

    impl Render for Session {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let (dropdown, context) = (self.told.clone(), self.told.clone());

            div()
                .size_full()
                .child(
                    DropdownMenu::new("session", Button::new("session-button").label("Session"))
                        .items(move |_, _| items(&dropdown)),
                )
                .child(
                    ContextMenu::new(
                        "row",
                        div()
                            .id("row")
                            .debug_selector(|| "row".to_string())
                            .w(px(300.))
                            .h(px(40.))
                            .mt(px(200.))
                            .tab_index(0),
                    )
                    .items(move |_, _| items(&context)),
                )
        }
    }

    fn session(cx: &mut TestAppContext) -> (Told, &mut VisualTestContext) {
        cx.update(crate::theme::init);
        cx.update(super::bind_keys);
        cx.update(crate::overlay::bind_keys);

        let told: Told = Rc::new(RefCell::new(Vec::new()));
        let kept = told.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Session { told: kept });

        (told, cx)
    }

    fn press(cx: &mut VisualTestContext, key: &'static str) {
        let keystroke = Keystroke::parse(key).unwrap();

        cx.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke });
    }

    fn item(cx: &mut VisualTestContext, index: usize) -> Option<gpui::Bounds<gpui::Pixels>> {
        let selector: &'static str = match index {
            0 => "menu-item-0",
            1 => "menu-item-1",
            _ => "menu-item-3",
        };

        cx.debug_bounds(selector)
    }

    #[gpui::test]
    fn the_keys_go_past_what_is_disabled_and_do_the_item(cx: &mut TestAppContext) {
        let (told, cx) = session(cx);

        cx.simulate_click(point(px(20.), px(16.)), Modifiers::none());

        assert!(item(cx, 0).is_some());

        // Down marks Rename, then passes Branch and the line to Delete.
        cx.simulate_keystrokes("down down");
        press(cx, "enter");

        assert_eq!(*told.borrow(), ["delete"]);
        assert!(item(cx, 0).is_none());
    }

    #[gpui::test]
    fn opened_from_the_keyboard_it_marks_its_first_item(cx: &mut TestAppContext) {
        let (told, cx) = session(cx);

        cx.update(|window, cx| window.focus_next(cx));
        press(cx, "enter");
        press(cx, "enter");

        assert_eq!(*told.borrow(), ["rename"]);
    }

    #[gpui::test]
    fn escape_and_a_press_outside_close_it(cx: &mut TestAppContext) {
        let (told, cx) = session(cx);

        cx.simulate_click(point(px(20.), px(16.)), Modifiers::none());
        cx.simulate_keystrokes("escape");

        assert!(item(cx, 0).is_none());

        cx.simulate_click(point(px(20.), px(16.)), Modifiers::none());
        cx.simulate_click(point(px(500.), px(500.)), Modifiers::none());

        assert!(item(cx, 0).is_none());

        // A disabled item does nothing pressed.
        cx.simulate_click(point(px(20.), px(16.)), Modifiers::none());

        let branch = item(cx, 1).unwrap().center();
        cx.simulate_click(branch, Modifiers::none());

        assert!(told.borrow().is_empty());
    }

    #[gpui::test]
    fn the_other_button_opens_a_context_menu_where_it_pressed(cx: &mut TestAppContext) {
        let (told, cx) = session(cx);
        let at = cx.debug_bounds("row").unwrap().center();

        cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());

        let rename = item(cx, 0).unwrap();

        assert!(rename.top() >= at.y && rename.left() >= at.x - px(8.));

        cx.simulate_click(rename.center(), Modifiers::none());

        assert_eq!(*told.borrow(), ["rename"]);
    }

    #[gpui::test]
    fn shift_f10_opens_the_context_menu_of_what_has_the_keyboard(cx: &mut TestAppContext) {
        let (told, cx) = session(cx);

        // The button, then the row.
        cx.update(|window, cx| {
            window.focus_next(cx);
            window.focus_next(cx);
        });
        cx.simulate_keystrokes("shift-f10 down");
        press(cx, "enter");

        assert_eq!(*told.borrow(), ["delete"]);
    }
}
