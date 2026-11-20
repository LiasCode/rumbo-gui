//! What a checkbox and a switch share: the control, its label and
//! description beside it, the whole row pressed to change it, and the
//! keyboard's ring drawn on the control rather than on the row.

use gpui::{
    AnyElement, App, Div, ElementId, FocusHandle, InteractiveElement as _, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _,
};

use crate::ChangeHandler;
use crate::label::Label;
use crate::root::{h_flex, v_flex};
use crate::size::{Sizable as _, Size};
use crate::theme::ActiveTheme as _;

/// What a control says beside it.
#[derive(Default)]
pub(crate) struct Words {
    pub(crate) label: Option<SharedString>,
    pub(crate) description: Option<SharedString>,
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
    on_change: Option<ChangeHandler<bool>>,
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
