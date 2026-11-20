//! The keyboard's handle of a control that is drawn anew each frame, and
//! when to ring it.

use gpui::{App, ElementId, FocusHandle, Window};

/// The keyboard's handle of the control `id`, kept from one frame to the
/// next so the control can tell when it has the keyboard.
pub(crate) fn focus_of(id: &ElementId, window: &mut Window, cx: &mut App) -> FocusHandle {
    window
        .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle().tab_stop(true))
        .read(cx)
        .clone()
}

/// Whether to ring the control: it has the keyboard, and the keyboard
/// brought it there.
pub(crate) fn ringed(focus: &FocusHandle, window: &Window) -> bool {
    focus.is_focused(window) && window.last_input_was_keyboard()
}
