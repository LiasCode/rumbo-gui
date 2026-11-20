//! A window inside the window: a surface with a title, which floats over the
//! work, is dragged by its title, and closes with its button or Escape. A
//! modal one dims what is behind it and keeps the pointer from it, as a
//! dialog does; one that is not leaves the work usable beside it.
//!
//! It shows while its view renders it, and its view stops rendering it in
//! `on_close`.

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, ElementId, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement, Pixels, Point,
    Refineable as _, Render, RenderOnce, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, anchored, deferred, div, point, prelude::FluentBuilder as _,
    px,
};

use crate::button::Button;
use crate::child_id;
use crate::icon::IconName;
use crate::overlay::{self, Dismiss};
use crate::root::{h_flex, v_flex};
use crate::size::Sizable as _;
use crate::theme::ActiveTheme as _;

/// How much of a window's top stays in the window, so its title can be
/// taken again.
const TITLE_HEIGHT: Pixels = px(40.);

/// What is dragged when a window's title is.
struct Dragged {
    window: ElementId,
}

/// What shows under the pointer while a window is dragged: nothing, since
/// the window itself moves.
struct Unseen;

impl Render for Unseen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// What a window remembers while it shows.
struct Placed {
    /// Where its top left corner is in the window; none until it first
    /// shows, centered.
    origin: Option<Point<Pixels>>,
    /// Where the pointer took its title, from its corner.
    grab: Point<Pixels>,
    focus: FocusHandle,
    /// What had the keyboard before it showed.
    restore: Option<FocusHandle>,
}

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct FloatingWindow {
    id: ElementId,
    title: SharedString,
    width: Pixels,
    modal: bool,
    close_on_backdrop: bool,
    children: Vec<AnyElement>,
    footer: Option<AnyElement>,
    on_close: Option<CloseHandler>,
    initial_focus: Option<FocusHandle>,
    style: StyleRefinement,
}

impl FloatingWindow {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        FloatingWindow {
            id: id.into(),
            title: title.into(),
            width: px(440.),
            modal: false,
            close_on_backdrop: true,
            children: Vec::new(),
            footer: None,
            on_close: None,
            initial_focus: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    /// Dims the work behind it and keeps the pointer from it.
    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    /// Whether a press on what a modal window dims closes it, as it does
    /// unless it holds work that would be lost.
    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    /// The buttons along its foot, such as Cancel and Save.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// What takes the keyboard when it shows, such as its first field;
    /// without it, the window does.
    pub fn initial_focus(mut self, focus: FocusHandle) -> Self {
        self.initial_focus = Some(focus);
        self
    }

    /// Called when the person closes it: its view stops rendering it here.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

impl ParentElement for FloatingWindow {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Its style is its body's, such as its padding or gap.
impl Styled for FloatingWindow {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Gives the keyboard back to where it was, and lets the view close it.
fn close(
    placed: &gpui::Entity<Placed>,
    on_close: &Option<CloseHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(restore) = placed.update(cx, |placed, _| placed.restore.take()) {
        restore.focus(window, cx);
    }

    if let Some(on_close) = on_close {
        on_close(window, cx);
    }
}

impl RenderOnce for FloatingWindow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let viewport = window.viewport_size();
        let width = self.width.min(viewport.width - px(16.));

        let initial = self.initial_focus.clone();
        let placed = window.use_keyed_state(child_id(&self.id, "placed"), cx, |window, cx| {
            let focus = cx.focus_handle();
            let restore = window.focused(cx);
            let taking = initial.unwrap_or_else(|| focus.clone());

            // It takes the keyboard once it is there to hold it.
            window.defer(cx, move |window, cx| taking.focus(window, cx));

            Placed {
                origin: None,
                grab: Point::default(),
                focus,
                restore,
            }
        });

        let origin = placed
            .read(cx)
            .origin
            .unwrap_or_else(|| point((viewport.width - width) / 2., viewport.height * 0.15));
        let focus = placed.read(cx).focus.clone();

        placed.update(cx, |placed, _| placed.origin = Some(origin));

        let theme = cx.theme();
        let on_close = self.on_close.clone();
        let dragged = self.id.clone();

        let closing = (placed.clone(), on_close.clone());
        let close_button = Button::new(child_id(&self.id, "close"))
            .ghost()
            .xsmall()
            .icon(IconName::Close)
            .tooltip("Close (Escape)")
            .on_click(move |_, window, cx| close(&closing.0, &closing.1, window, cx));

        let grabbing = placed.clone();
        let header = h_flex()
            .id(child_id(&self.id, "title"))
            .flex_none()
            .justify_between()
            .gap_2()
            .pl_4()
            .pr_2()
            .pt_2()
            .pb_1()
            .cursor_grab()
            .on_drag(
                Dragged {
                    window: self.id.clone(),
                },
                move |_, grab, _, cx| {
                    grabbing.update(cx, |placed, _| placed.grab = grab);
                    cx.new(|_| Unseen)
                },
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.title),
            )
            .child(close_button);

        let mut body = v_flex().gap_3().px_4().pt_2().pb_4().text_sm();

        body.style().refine(&self.style);

        let moving = placed.clone();
        let dismissing = (placed.clone(), on_close.clone());

        let panel = overlay::surface(cx)
            .id(child_id(&self.id, "panel"))
            .w(width)
            .max_h(viewport.height - px(16.))
            .flex()
            .flex_col()
            .key_context(overlay::WINDOW_CONTEXT)
            .track_focus(&focus)
            .occlude()
            .on_action(move |_: &Dismiss, window, cx| {
                close(&dismissing.0, &dismissing.1, window, cx)
            })
            .on_drag_move(move |event: &gpui::DragMoveEvent<Dragged>, window, cx| {
                if event.drag(cx).window != dragged {
                    return;
                }

                let viewport = window.viewport_size();
                let grab = moving.read(cx).grab;
                let wanted = event.event.position - grab;
                let kept = point(
                    wanted.x.clamp(px(0.), (viewport.width - width).max(px(0.))),
                    wanted
                        .y
                        .clamp(px(0.), (viewport.height - TITLE_HEIGHT).max(px(0.))),
                );

                moving.update(cx, |placed, cx| {
                    placed.origin = Some(kept);
                    cx.notify();
                });
            })
            .child(header)
            .child(body.children(self.children))
            .when_some(self.footer, |panel, footer| {
                panel.child(
                    h_flex()
                        .flex_none()
                        .justify_end()
                        .gap_2()
                        .px_4()
                        .pb_4()
                        .child(footer),
                )
            });

        // It stays whole in the window, however far it is dragged.
        let placed_panel = anchored().position(origin).snap_to_window().child(panel);

        if !self.modal {
            return deferred(placed_panel).with_priority(1).into_any_element();
        }

        let backdrop_closes = self.close_on_backdrop;
        let backdrop = (placed.clone(), on_close);

        let layer = div()
            .w(viewport.width)
            .h(viewport.height)
            .bg(theme.colors.overlay)
            .occlude()
            .when(backdrop_closes, |layer| {
                layer.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    close(&backdrop.0, &backdrop.1, window, cx)
                })
            })
            .child(placed_panel);

        deferred(anchored().position(point(px(0.), px(0.))).child(layer))
            .with_priority(1)
            .into_any_element()
    }
}
