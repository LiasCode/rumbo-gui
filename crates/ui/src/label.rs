//! The name of a field or a setting, with words that matter less beside it,
//! such as "(optional)".

use gpui::{
    App, FontWeight, IntoElement, ParentElement as _, Refineable as _, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
};

use crate::root::h_flex;
use crate::size::{Sizable, Size};
use crate::theme::ActiveTheme as _;

#[derive(IntoElement)]
pub struct Label {
    text: SharedString,
    secondary: Option<SharedString>,
    muted: bool,
    disabled: bool,
    size: Size,
    style: StyleRefinement,
}

impl Label {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Label {
            text: text.into(),
            secondary: None,
            muted: false,
            disabled: false,
            size: Size::Medium,
            style: StyleRefinement::default(),
        }
    }

    /// Words after the label, in a quieter color.
    pub fn secondary(mut self, text: impl Into<SharedString>) -> Self {
        self.secondary = Some(text.into());
        self
    }

    /// Says it in a quieter color, as a description does.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Labels a control that cannot be used now.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Label {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Label {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Label {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors;
        let color = match self.muted {
            true => colors.muted_foreground,
            false => colors.foreground,
        };

        let mut label = h_flex()
            .gap_1()
            .text_size(self.size.text())
            .line_height(self.size.line_height())
            .text_color(color)
            .when(!self.muted, |label| label.font_weight(FontWeight::MEDIUM))
            .when(self.disabled, |label| label.opacity(0.5))
            .child(self.text)
            .when_some(self.secondary, |label, secondary| {
                label.child(
                    div()
                        .font_weight(FontWeight::NORMAL)
                        .text_color(colors.muted_foreground)
                        .child(secondary),
                )
            });

        label.style().refine(&self.style);
        label
    }
}
