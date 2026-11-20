//! The bar along the foot of a view, in small, quiet words: how things
//! stand at its start, and what is counted at its end.

use gpui::{
    AnyElement, App, IntoElement, ParentElement as _, Refineable as _, RenderOnce, StyleRefinement,
    Styled, Window, px,
};

use crate::root::h_flex;
use crate::theme::ActiveTheme as _;

#[derive(IntoElement, Default)]
pub struct StatusBar {
    start: Vec<AnyElement>,
    end: Vec<AnyElement>,
    style: StyleRefinement,
}

impl StatusBar {
    pub fn new() -> Self {
        StatusBar::default()
    }

    /// What goes at its start, after what is there.
    pub fn start(mut self, child: impl IntoElement) -> Self {
        self.start.push(child.into_any_element());
        self
    }

    /// What goes at its end, after what is there.
    pub fn end(mut self, child: impl IntoElement) -> Self {
        self.end.push(child.into_any_element());
        self
    }
}

impl Styled for StatusBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors;
        let part = || h_flex().min_w_0().overflow_hidden().gap_2();

        let mut bar = h_flex()
            .flex_none()
            .w_full()
            .h(px(28.))
            .px_2()
            .gap_2()
            .justify_between()
            .border_t_1()
            .border_color(colors.border)
            .bg(colors.background)
            .text_xs()
            .text_color(colors.muted_foreground)
            .whitespace_nowrap()
            .child(part().children(self.start))
            .child(part().flex_none().children(self.end));

        bar.style().refine(&self.style);
        bar
    }
}
