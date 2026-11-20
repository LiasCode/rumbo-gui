//! What a checkbox and a switch share: the control, its label and
//! description beside it, the whole row pressed to change it, and the
//! keyboard's ring drawn on the control rather than on the row.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, ElementId, FocusHandle, InteractiveElement as _, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};

use crate::label::Label;
use crate::root::{h_flex, v_flex};
use crate::size::{Sizable as _, Size};
use crate::theme::ActiveTheme as _;

/// What is called with the value a control changes to.
pub(crate) type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// What a control says beside it.
#[derive(Default)]
pub(crate) struct Words {
    pub(crate) label: Option<SharedString>,
    pub(crate) description: Option<SharedString>,
}

/// The keyboard's handle of the control `id`, kept from one frame to the
/// next so the row can tell when it has the keyboard.
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

/// The row: the control and its words, which a press anywhere on changes to
/// `!checked`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn row(
    id: ElementId,
    focus: &FocusHandle,
    control: AnyElement,
    words: Words,
    checked: bool,
    disabled: bool,
    size: Size,
    on_change: Option<ToggleHandler>,
    cx: &App,
) -> Stateful<Div> {
    let muted = cx.theme().colors.muted_foreground;
    let has_words = words.label.is_some() || words.description.is_some();

    h_flex()
        .id(id)
        .items_start()
        .gap_2()
        .child(control)
        .when(has_words, |row| {
            row.child(
                v_flex()
                    .gap_0p5()
                    .when_some(words.label, |column, label| {
                        column.child(Label::new(label).with_size(size).disabled(disabled))
                    })
                    .when_some(words.description, |column, description| {
                        column.child(
                            div()
                                .text_size(size.text())
                                .text_color(muted)
                                .when(disabled, |text| text.opacity(0.5))
                                .child(description),
                        )
                    }),
            )
        })
        .map(|row| match disabled {
            true => row.cursor_default(),
            false => {
                row.cursor_pointer()
                    .track_focus(focus)
                    .when_some(on_change, |row, on_change| {
                        row.on_click(move |_, window, cx| on_change(&!checked, window, cx))
                    })
            }
        })
}
