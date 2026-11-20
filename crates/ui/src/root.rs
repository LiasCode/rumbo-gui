//! The base of a window drawn with these components: its colors and type,
//! and Tab and Shift-Tab moving the keyboard between its controls, which
//! GPUI leaves to the application.

use gpui::{App, Div, InteractiveElement as _, KeyBinding, Styled as _, actions, div};

use crate::theme::ActiveTheme as _;

actions!(ui, [FocusNext, FocusPrevious]);

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, None),
        KeyBinding::new("shift-tab", FocusPrevious, None),
    ]);
}

/// The element a window's view puts everything in. Its view must take the
/// keyboard when the window opens, so the keys reach it when no control
/// has it yet.
pub fn root(cx: &App) -> Div {
    let theme = cx.theme();

    div()
        .size_full()
        .bg(theme.colors.background)
        .text_color(theme.colors.foreground)
        .font_family(theme.font_family())
        .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
        .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
}

/// A row, its children centered across it.
pub fn h_flex() -> Div {
    div().flex().flex_row().items_center()
}

/// A column.
pub fn v_flex() -> Div {
    div().flex().flex_col()
}
