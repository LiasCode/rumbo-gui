use gpui::{AnyElement, Context, ParentElement as _, SharedString, Styled as _, Window};
use ui::{Checkbox, Sizable as _, v_flex};

use super::{Demos, note, row, section, told};
use crate::Overview;

pub struct Demo {
    checked: [bool; 5],
    told: Vec<SharedString>,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            checked: [false, true, false, true, false],
            told: Vec::new(),
        }
    }
}

/// A checkbox whose value the page keeps, numbered among them.
fn checkbox(index: usize, demos: &Demos, cx: &mut Context<Overview>) -> Checkbox {
    Checkbox::new(("checkbox", index))
        .checked(demos.checkbox.checked[index])
        .on_change(cx.listener(move |this, checked: &bool, _, cx| {
            let demo = &mut this.demos.checkbox;

            demo.checked[index] = *checked;
            note(
                &mut demo.told,
                format!("Checkbox {} is {checked}", index + 1),
            );
            cx.notify();
        }))
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    vec![
        section(
            "Alone",
            "Unchecked and checked. Tab reaches it and Space changes it; the ring shows only when the keyboard brought it there.",
            row()
                .gap_6()
                .child(checkbox(0, demos, cx))
                .child(checkbox(1, demos, cx)),
            cx,
        ),
        section(
            "With a label and a description",
            "Pressing the words changes it too.",
            v_flex()
                .gap_4()
                .child(checkbox(2, demos, cx).label("Keep the session's history"))
                .child(
                    checkbox(3, demos, cx)
                        .label("Compact when the context is full")
                        .description(
                            "Replaces the conversation with a summary, so the run goes on.",
                        ),
                )
                .child(checkbox(4, demos, cx).small().label("Small")),
            cx,
        ),
        section(
            "Disabled",
            "",
            row()
                .gap_6()
                .child(Checkbox::new("disabled-off").label("Off").disabled(true))
                .child(
                    Checkbox::new("disabled-on")
                        .label("On")
                        .checked(true)
                        .disabled(true),
                ),
            cx,
        ),
        section("What they told", "", told(&demos.checkbox.told, cx), cx),
    ]
}
