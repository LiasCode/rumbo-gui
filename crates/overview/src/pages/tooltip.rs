use gpui::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};
use ui::{ActiveTheme as _, Button, IconName, Tooltip};

use super::{Demos, row, section};
use crate::Overview;

pub struct Demo {
    /// Tooltips drawn in the page, to look at without resting the pointer.
    plain: Entity<Tooltip>,
    with_keys: Entity<Tooltip>,
}

impl Demo {
    pub fn new(cx: &mut Context<Overview>) -> Self {
        Demo {
            plain: cx.new(|_| Tooltip::new("Copy the session's id")),
            with_keys: cx.new(|_| Tooltip::new("Commands").keys("Ctrl-K")),
        }
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.tooltip;
    let theme = cx.theme();

    let hovered = row()
        .child(
            Button::new("tip-button")
                .outline()
                .label("Export")
                .tooltip("Write the conversation as Markdown"),
        )
        .child(
            Button::new("tip-icon")
                .ghost()
                .icon(IconName::Search)
                .tooltip("Commands")
                .shortcut("Ctrl-K"),
        )
        .child(
            div()
                .id("tip-words")
                .text_sm()
                .underline()
                .text_color(theme.colors.muted_foreground)
                .child("context 42% of 200k")
                .tooltip(Tooltip::text("How full the model's context is")),
        );

    vec![
        section(
            "On the pointer",
            "Rest the pointer on each: after a moment it says what the control does.",
            hovered,
            cx,
        ),
        section(
            "How it looks",
            "Drawn here as it is drawn by the pointer, with and without keys.",
            row()
                .child(demo.plain.clone())
                .child(demo.with_keys.clone()),
            cx,
        ),
    ]
}
