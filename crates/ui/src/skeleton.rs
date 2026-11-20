//! A shape that breathes where something will show once it is read, as
//! long as a line of text unless given a size.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, IntoElement, Refineable as _, RenderOnce, StyleRefinement,
    Styled, Window, div, pulsating_between,
};

use crate::theme::ActiveTheme as _;

/// How long a breath takes.
const BREATH: Duration = Duration::from_secs(2);

#[derive(IntoElement, Default)]
pub struct Skeleton {
    style: StyleRefinement,
}

impl Skeleton {
    pub fn new() -> Self {
        Skeleton::default()
    }
}

impl Styled for Skeleton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let mut shape = div()
            .flex_none()
            .w_full()
            .h_4()
            .rounded(theme.radius)
            .bg(theme.colors.muted);

        shape.style().refine(&self.style);

        shape.with_animation(
            "skeleton",
            Animation::new(BREATH)
                .repeat()
                .with_easing(pulsating_between(0.5, 1.)),
            |shape, opacity| shape.opacity(opacity),
        )
    }
}
