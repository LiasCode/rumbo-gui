//! The frame a field's text sits in, shared by `Input` and `Textarea`: it
//! takes the keyboard and the pointer for the field, and draws its border,
//! which rings while the field has the keyboard.

use gpui::{
    App, CursorStyle, Div, Entity, Focusable as _, InteractiveElement as _, MouseButton,
    Styled as _, Window, div, prelude::FluentBuilder as _,
};

use super::state::{InputState, KEY_CONTEXT};
use crate::theme::ActiveTheme as _;

/// The frame of the field `state`, with its border and background unless
/// `bare`, for a field set in a surface of its own, as a composer is.
pub(crate) fn frame(state: &Entity<InputState>, bare: bool, window: &Window, cx: &App) -> Div {
    let theme = cx.theme();
    let field = state.read(cx);
    let focused = field.is_focused(window);
    let disabled = field.is_disabled();
    let focus = state.focus_handle(cx);
    let ring = theme.focus_ring();

    let frame = div().rounded(theme.radius).when(!bare, |frame| {
        frame
            .border_1()
            .border_color(theme.colors.input)
            .bg(theme.colors.background)
            .when(focused, |frame| {
                frame.border_color(theme.colors.ring).shadow(ring)
            })
    });

    if disabled {
        return frame.opacity(0.5).cursor_default();
    }

    let frame = frame
        .key_context(KEY_CONTEXT)
        .track_focus(&focus)
        .cursor(CursorStyle::IBeam)
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(state, InputState::on_mouse_down),
        )
        .on_mouse_move(window.listener_for(state, InputState::on_mouse_move))
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(state, InputState::on_mouse_up),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            window.listener_for(state, InputState::on_mouse_up),
        );

    InputState::actions(frame, state, window)
}
