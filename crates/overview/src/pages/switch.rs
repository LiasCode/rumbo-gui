use gpui::{AnyElement, Context, ParentElement as _, SharedString, Styled as _, Window};
use ui::{Sizable as _, Switch, v_flex};

use super::{Demos, note, row, section, told};
use crate::Overview;

pub struct Demo {
    on: [bool; 5],
    told: Vec<SharedString>,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            on: [false, true, true, false, true],
            told: Vec::new(),
        }
    }
}

/// A switch whose value the page keeps, numbered among them.
fn switch(index: usize, demos: &Demos, cx: &mut Context<Overview>) -> Switch {
    Switch::new(("switch", index))
        .checked(demos.switch.on[index])
        .on_change(cx.listener(move |this, on: &bool, _, cx| {
            let demo = &mut this.demos.switch;

            demo.on[index] = *on;
            note(&mut demo.told, format!("Switch {} is {on}", index + 1));
            cx.notify();
        }))
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    vec![
        section(
            "Alone",
            "Off and on. Its knob slides when it changes; Tab reaches it and Space changes it.",
            row()
                .gap_6()
                .child(switch(0, demos, cx))
                .child(switch(1, demos, cx)),
            cx,
        ),
        section(
            "With a label and a description",
            "Pressing the words changes it too.",
            v_flex()
                .gap_4()
                .child(switch(2, demos, cx).label("Say when a run ends elsewhere"))
                .child(
                    switch(3, demos, cx).label("Quiet").description(
                        "No notification when a run ends in a session that is not shown.",
                    ),
                )
                .child(switch(4, demos, cx).small().label("Small")),
            cx,
        ),
        section(
            "Disabled",
            "",
            row()
                .gap_6()
                .child(Switch::new("disabled-off").label("Off").disabled(true))
                .child(
                    Switch::new("disabled-on")
                        .label("On")
                        .checked(true)
                        .disabled(true),
                ),
            cx,
        ),
        section("What they told", "", told(&demos.switch.told, cx), cx),
    ]
}
