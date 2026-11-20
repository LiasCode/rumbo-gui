//! The bar along the top of a window that draws its own: what the view puts
//! in it, the window's buttons at its end where the desktop leaves them to
//! the window, and room that moves the window when dragged and makes it
//! fill the screen when pressed twice. Its window opens with
//! `TitleBar::window_options()`, and draws in a `WindowFrame`.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, Decorations, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement, Pixels, Refineable as _, RenderOnce, StatefulInteractiveElement as _,
    StyleRefinement, Styled, TitlebarOptions, Window, WindowControlArea, WindowOptions, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::icon::{Icon, IconName};
use crate::root::h_flex;
use crate::size::Sizable as _;
use crate::theme::ActiveTheme as _;

/// How tall the bar is.
pub const TITLE_BAR_HEIGHT: Pixels = px(34.);

/// Room at its start: on macOS, for the window's own buttons there.
#[cfg(target_os = "macos")]
const LEADING: Pixels = px(80.);
#[cfg(not(target_os = "macos"))]
const LEADING: Pixels = px(12.);

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement, Default)]
pub struct TitleBar {
    children: Vec<AnyElement>,
    on_close_window: Option<CloseHandler>,
    style: StyleRefinement,
}

impl TitleBar {
    pub fn new() -> Self {
        TitleBar::default()
    }

    /// What a window with this bar opens with: its own title bar, which
    /// moves it, over what the desktop would draw.
    pub fn window_options() -> WindowOptions {
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: None,
                appears_transparent: true,
                traffic_light_position: Some(point(px(9.), px(9.))),
            }),
            app_owns_titlebar_drag: true,
            ..WindowOptions::default()
        }
    }

    /// What the close button does where the window draws it; without it, it
    /// closes the window.
    pub fn on_close_window(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close_window = Some(Rc::new(handler));
        self
    }
}

impl ParentElement for TitleBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for TitleBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// One of the window's buttons.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// The window's buttons, where the desktop leaves them to the window: on
/// Linux, when the window draws its own frame. Close is always offered;
/// the others when the desktop can do them.
fn controls(on_close: Option<CloseHandler>, window: &Window, cx: &App) -> Option<AnyElement> {
    if !cfg!(target_os = "linux") {
        return None;
    }

    if !matches!(window.window_decorations(), Decorations::Client { .. }) {
        return None;
    }

    let colors = cx.theme().colors;
    let can = window.window_controls();
    let mut shown = Vec::new();

    if can.minimize {
        shown.push(Control::Minimize);
    }

    if can.maximize {
        shown.push(match window.is_maximized() {
            true => Control::Restore,
            false => Control::Maximize,
        });
    }

    shown.push(Control::Close);

    let buttons = shown.into_iter().map(|control| {
        let (id, icon) = match control {
            Control::Minimize => ("window-minimize", IconName::Minus),
            Control::Maximize => ("window-maximize", IconName::Square),
            Control::Restore => ("window-restore", IconName::Copy),
            Control::Close => ("window-close", IconName::Close),
        };
        let (hover, hover_text) = match control {
            Control::Close => (colors.danger, colors.danger_foreground),
            _ => (colors.secondary_hover, colors.foreground),
        };
        let on_close = on_close.clone();

        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(TITLE_BAR_HEIGHT)
            .h_full()
            .text_color(colors.foreground)
            .hover(move |style| style.bg(hover).text_color(hover_text))
            // The press is the button's, and moves no window.
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();

                match control {
                    Control::Minimize => window.minimize_window(),
                    Control::Maximize | Control::Restore => window.zoom_window(),
                    Control::Close => match &on_close {
                        Some(on_close) => on_close(window, cx),
                        None => window.remove_window(),
                    },
                }
            })
            .child(Icon::new(icon).small())
    });

    Some(
        h_flex()
            .flex_none()
            .h_full()
            .children(buttons)
            .into_any_element(),
    )
}

/// Whether a press on the bar has yet to move the window: the window moves
/// once the pointer moves while it is down.
#[derive(Default)]
struct Moving {
    pressed: bool,
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors;
        let client = matches!(window.window_decorations(), Decorations::Client { .. });
        let moving = window.use_keyed_state("title-bar-moving", cx, |_, _| Moving::default());

        let pressed = moving.clone();
        let released = moving.clone();
        let left = moving.clone();
        let moved = moving.clone();

        let mut bar = h_flex()
            .id("title-bar")
            .flex_none()
            .w_full()
            .h(TITLE_BAR_HEIGHT)
            .pl(LEADING)
            .justify_between()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.background)
            // Pressed twice, it fills the screen with the window, or gives
            // it back its size.
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    match cfg!(target_os = "macos") {
                        true => window.titlebar_double_click(),
                        false => window.zoom_window(),
                    }
                }
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                set(&pressed, true, cx);
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                set(&released, false, cx);
            })
            .on_mouse_down_out(move |_, _, cx| {
                set(&left, false, cx);
            })
            .on_mouse_move(move |_, window, cx| {
                if moved.read(cx).pressed {
                    set(&moved, false, cx);
                    window.start_window_move();
                }
            })
            .child(
                h_flex()
                    .id("title-bar-room")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag)
                    // The desktop's menu of the window, where it has one.
                    .when(cfg!(target_os = "linux") && client, |room| {
                        room.on_mouse_down(MouseButton::Right, |event, window, _| {
                            window.show_window_menu(event.position)
                        })
                    })
                    .children(self.children),
            )
            .children(controls(self.on_close_window, window, cx));

        bar.style().refine(&self.style);
        bar
    }
}

fn set(moving: &gpui::Entity<Moving>, pressed: bool, cx: &mut App) {
    moving.update(cx, |moving: &mut Moving, _: &mut Context<Moving>| {
        moving.pressed = pressed
    });
}
