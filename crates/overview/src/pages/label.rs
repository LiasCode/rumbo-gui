use gpui::{AnyElement, Context, ParentElement as _, Styled as _, Window, px};
use ui::{Input, Label, Sizable as _, v_flex};

use super::{Demos, row, section};
use crate::Overview;

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    vec![
        section("Label", "", Label::new("Email address"), cx),
        section(
            "With words that matter less",
            "Said after it, in a quieter color.",
            Label::new("Project").secondary("(optional)"),
            cx,
        ),
        section(
            "Muted",
            "For a description, or a state that matters less than the label beside it.",
            Label::new("Saved 2 minutes ago").muted(true),
            cx,
        ),
        section(
            "Disabled",
            "The label of a control that cannot be used now.",
            Label::new("Model").disabled(true),
            cx,
        ),
        section(
            "Sizes",
            "",
            row()
                .items_baseline()
                .child(Label::new("Extra small").xsmall())
                .child(Label::new("Small").small())
                .child(Label::new("Medium"))
                .child(Label::new("Large").large()),
            cx,
        ),
        section(
            "Over a field",
            "",
            v_flex()
                .w(px(320.))
                .gap_2()
                .child(Label::new("Name").secondary("(optional)"))
                .child(Input::new(&demos.input.name)),
            cx,
        ),
    ]
}
