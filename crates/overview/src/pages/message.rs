use gpui::{AnyElement, Context, ParentElement as _, Styled as _, Window, div, px};
use ui::{ActiveTheme as _, Message, MessageSide, v_flex};

use super::section;
use crate::Overview;

pub fn render(_: &mut super::Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let muted = cx.theme().colors.muted_foreground;

    let talk = v_flex()
        .w(px(560.))
        .gap_4()
        .child(Message::new().caption("Task 1").child("List the files in the project's folder."))
        .child(
            div()
                .text_sm()
                .child("There are three: Cargo.toml, README.md, and src/main.rs."),
        )
        .child(
            Message::new()
                .caption("Told the run")
                .child("Leave out what Git ignores."),
        )
        .child(
            Message::new()
                .side(MessageSide::Start)
                .caption("The provider asked")
                .child("Which provider should be set up?"),
        )
        .child(Message::new().child(
            "A message long enough to wrap: it keeps to four fifths of the conversation's width, so what is said to the person stays apart from what they said.",
        ));

    vec![
        section(
            "A conversation",
            "The person's own messages stand at the end; what is said to them, at the start.",
            talk,
            cx,
        ),
        section(
            "",
            "",
            div()
                .text_xs()
                .text_color(muted)
                .child("A message is styled as its bubble."),
            cx,
        ),
    ]
}
