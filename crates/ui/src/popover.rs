//! A surface that opens from a trigger, beside it, and holds anything: a
//! form, a list, words. A press on the trigger opens and closes it; Escape
//! and a press outside close it, and give the keyboard back to where it
//! was.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, ElementId, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Refineable as _, RenderOnce,
    StyleRefinement, Styled, WeakEntity, Window, anchored, deferred, div, point, px,
};

use crate::button::Button;
use crate::overlay::{self, Dismiss};

/// How far a popover stands from its trigger.
const GAP: Pixels = px(4.);

/// What opens a popover: it is pressed, and shows when what it opens is
/// open.
pub trait PopoverTrigger: IntoElement + 'static {
    fn on_press(self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self;

    fn open(self, open: bool) -> Self;
}

impl PopoverTrigger for Button {
    fn on_press(self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click(handler)
    }

    fn open(self, open: bool) -> Self {
        self.selected(open)
    }
}

/// Where a popover opens: below or above its trigger, lined up with the
/// trigger's start or end. It moves to the other side when it does not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    #[default]
    BottomStart,
    BottomEnd,
    TopStart,
    TopEnd,
}

impl Placement {
    /// The popover's corner, the trigger's corner it sits on, and how far
    /// from it.
    pub(crate) fn corners(self) -> (gpui::Anchor, gpui::Anchor, gpui::Point<Pixels>) {
        use gpui::Anchor::*;

        match self {
            Placement::BottomStart => (TopLeft, BottomLeft, point(px(0.), GAP)),
            Placement::BottomEnd => (TopRight, BottomRight, point(px(0.), GAP)),
            Placement::TopStart => (BottomLeft, TopLeft, point(px(0.), -GAP)),
            Placement::TopEnd => (BottomRight, TopRight, point(px(0.), -GAP)),
        }
    }
}

/// Closes the popover it was given to, as a button in it does once its
/// work is done.
#[derive(Clone)]
pub struct PopoverControl {
    state: WeakEntity<PopoverState>,
}

impl PopoverControl {
    pub fn close(&self, window: &mut Window, cx: &mut App) {
        if let Some(state) = self.state.upgrade() {
            close(&state, window, cx);
        }
    }
}

type Content = Rc<dyn Fn(PopoverControl, &mut Window, &mut App) -> AnyElement>;

/// What a popover remembers between frames.
pub(crate) struct PopoverState {
    open: bool,
    /// Where its trigger was last painted.
    trigger: Bounds<Pixels>,
    focus: FocusHandle,
    /// What had the keyboard before it opened.
    restore: Option<FocusHandle>,
}

impl PopoverState {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        PopoverState {
            open: false,
            trigger: Bounds::default(),
            focus: cx.focus_handle(),
            restore: None,
        }
    }
}

/// Opens it, giving the keyboard to `initial`, or to the popover.
fn open(
    state: &Entity<PopoverState>,
    initial: Option<FocusHandle>,
    window: &mut Window,
    cx: &mut App,
) {
    let restore = window.focused(cx);
    let focus = state.update(cx, |state, cx| {
        state.open = true;
        state.restore = restore;
        cx.notify();
        state.focus.clone()
    });

    initial.unwrap_or(focus).focus(window, cx);
}

fn close(state: &Entity<PopoverState>, window: &mut Window, cx: &mut App) {
    let restore = state.update(cx, |state, cx| {
        state.open = false;
        cx.notify();
        state.restore.take()
    });

    if let Some(restore) = restore {
        restore.focus(window, cx);
    }
}

#[derive(IntoElement)]
pub struct Popover<T: PopoverTrigger> {
    id: ElementId,
    trigger: T,
    content: Option<Content>,
    placement: Placement,
    initial_focus: Option<FocusHandle>,
    style: StyleRefinement,
}

impl<T: PopoverTrigger> Popover<T> {
    pub fn new(id: impl Into<ElementId>, trigger: T) -> Self {
        Popover {
            id: id.into(),
            trigger,
            content: None,
            placement: Placement::default(),
            initial_focus: None,
            style: StyleRefinement::default(),
        }
    }

    /// What it holds, built while it is open.
    pub fn content<E: IntoElement>(
        mut self,
        content: impl Fn(PopoverControl, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.content = Some(Rc::new(move |control, window, cx| {
            content(control, window, cx).into_any_element()
        }));
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// What takes the keyboard when it opens, such as its first field;
    /// without it, the popover does.
    pub fn initial_focus(mut self, focus: FocusHandle) -> Self {
        self.initial_focus = Some(focus);
        self
    }
}

/// Its style is its surface's, such as its width or padding.
impl<T: PopoverTrigger> Styled for Popover<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: PopoverTrigger> RenderOnce for Popover<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| PopoverState::new(cx));
        let (is_open, trigger_bounds, focus) = {
            let state = state.read(cx);

            (state.open, state.trigger, state.focus.clone())
        };

        let pressing = state.clone();
        let initial = self.initial_focus.clone();
        let trigger = self.trigger.open(is_open).on_press(move |_, window, cx| {
            match pressing.read(cx).open {
                true => close(&pressing, window, cx),
                false => open(&pressing, initial.clone(), window, cx),
            }
        });

        let tracking = state.clone();
        let wrapper = div()
            .child(trigger)
            .on_children_prepainted(move |bounds, _, cx| {
                if let Some(bounds) = bounds.first() {
                    tracking.update(cx, |state, _| state.trigger = *bounds);
                }
            });

        let Some(content) = self.content.filter(|_| is_open) else {
            return wrapper;
        };

        let control = PopoverControl {
            state: state.downgrade(),
        };
        let (anchor, corner, offset) = self.placement.corners();
        let dismissing = state.clone();
        let outside = state.clone();

        let mut surface = overlay::surface(cx)
            .p_4()
            .key_context(overlay::POPOVER_CONTEXT)
            .track_focus(&focus)
            .occlude()
            .on_action(move |_: &Dismiss, window, cx| close(&dismissing, window, cx))
            // A press on the trigger closes it as the trigger's press.
            .on_mouse_down_out(move |event, window, cx| {
                if !trigger_bounds.contains(&event.position) {
                    close(&outside, window, cx);
                }
            })
            .child(content(control, window, cx));

        surface.style().refine(&self.style);

        wrapper.child(
            deferred(
                anchored()
                    .attach(trigger_bounds, corner)
                    .anchor(anchor)
                    .offset(offset)
                    .snap_to_window_with_margin(px(8.))
                    .child(surface),
            )
            // Over a floating window, which a popover can open from.
            .with_priority(2),
        )
    }
}
