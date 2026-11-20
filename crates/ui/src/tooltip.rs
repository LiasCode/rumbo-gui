//! A word about a control, shown when the pointer rests on it: what it does,
//! and the keys that do the same. GPUI shows it after a moment and places
//! it by the pointer; this is what it shows.

use gpui::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, prelude::FluentBuilder as _,
};

use crate::root::h_flex;
use crate::theme::ActiveTheme as _;

pub struct Tooltip {
    text: SharedString,
    keys: Option<SharedString>,
}

impl Tooltip {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Tooltip {
            text: text.into(),
            keys: None,
        }
    }

    /// The keys that do the same, such as `Ctrl-K`.
    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.keys = Some(keys.into());
        self
    }

    /// The view GPUI shows, as an element's `tooltip` asks for it.
    pub fn build(self, cx: &mut App) -> AnyView {
        cx.new(|_| self).into()
    }

    /// What an element's `tooltip` takes, for a tooltip of words alone.
    pub fn text(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView {
        let text = text.into();

        move |_, cx| Tooltip::new(text.clone()).build(cx)
    }
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // GPUI puts it at the pointer; the margin keeps the pointer off it.
        div().child(
            h_flex()
                .m_3()
                .max_w_80()
                .gap_3()
                .px_2()
                .py_0p5()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.colors.border)
                .bg(theme.colors.popover)
                .text_color(theme.colors.popover_foreground)
                .font_family(theme.font_family())
                .text_sm()
                .shadow_md()
                .child(self.text.clone())
                .when_some(self.keys.clone(), |row, keys| {
                    row.child(
                        div()
                            .flex_none()
                            .px_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(theme.colors.border)
                            .text_xs()
                            .text_color(theme.colors.muted_foreground)
                            .child(keys),
                    )
                }),
        )
    }
}
