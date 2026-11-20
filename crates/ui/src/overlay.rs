//! What the surfaces that float over the rest share: how they look, Escape
//! putting them away, and Enter doing what a dialog asks.

use gpui::{App, Div, KeyBinding, Styled as _, actions, div};

use crate::theme::ActiveTheme as _;

actions!(overlay, [Dismiss, Confirm]);

/// The key contexts of the surfaces Escape puts away.
pub(crate) const POPOVER_CONTEXT: &str = "Popover";
pub(crate) const SELECT_CONTEXT: &str = "Select";
pub(crate) const WINDOW_CONTEXT: &str = "FloatingWindow";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some(POPOVER_CONTEXT)),
        KeyBinding::new("escape", Dismiss, Some(SELECT_CONTEXT)),
        KeyBinding::new("escape", Dismiss, Some(WINDOW_CONTEXT)),
        KeyBinding::new("enter", Confirm, Some(WINDOW_CONTEXT)),
    ]);
}

/// A surface that floats over the rest: a popover, a list, a window.
pub fn surface(cx: &App) -> Div {
    let theme = cx.theme();

    div()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.colors.border)
        .bg(theme.colors.popover)
        .text_color(theme.colors.popover_foreground)
        .shadow(theme.floating_shadow())
}
