//! A message in a conversation: what was said, on a quiet ground, with a
//! few words over it that say how it was said. The person's own messages
//! stand at the end, and what is said to them at the start.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, Refineable as _, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, relative,
};

use crate::root::v_flex;
use crate::theme::ActiveTheme as _;

/// Which side a message stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageSide {
    /// What the person said, at the end.
    #[default]
    End,
    /// What is said to them, at the start.
    Start,
}

#[derive(IntoElement, Default)]
pub struct Message {
    side: MessageSide,
    caption: Option<SharedString>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl Message {
    pub fn new() -> Self {
        Message::default()
    }

    pub fn side(mut self, side: MessageSide) -> Self {
        self.side = side;
        self
    }

    /// The words over it, such as how it was said.
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }
}

impl ParentElement for Message {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Its style is its bubble's.
impl Styled for Message {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Message {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = theme.colors;
        let end = self.side == MessageSide::End;

        let mut bubble = div()
            .max_w(relative(0.8))
            .px_3()
            .py_2()
            .rounded(theme.radius_lg)
            .bg(colors.muted)
            .text_sm()
            .text_color(colors.foreground)
            .children(self.children);

        bubble.style().refine(&self.style);

        v_flex()
            .w_full()
            .gap_1()
            .map(|message| match end {
                true => message.items_end(),
                false => message.items_start(),
            })
            .when_some(self.caption, |message, caption| {
                message.child(
                    div()
                        .px_1()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(caption),
                )
            })
            .child(bubble)
    }
}
