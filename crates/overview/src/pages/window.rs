use gpui::{AnyElement, Context, ParentElement as _, Styled as _, Window, div, px};
use ui::{ActiveTheme as _, Sizable as _, Spinner, StatusBar, h_flex, v_flex};

use super::section;
use crate::Overview;

pub fn render(_: &mut super::Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let theme = cx.theme();

    let bar = div()
        .w(px(640.))
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.colors.border)
        .overflow_hidden()
        .child(div().h(px(80.)))
        .child(
            StatusBar::new()
                .start(
                    h_flex()
                        .gap_1()
                        .child(Spinner::new().xsmall())
                        .child("Working: Port the composer"),
                )
                .end("1 working in other sessions")
                .end("41% of the context")
                .end("12,345 tokens"),
        );

    vec![
        section(
            "Title bar and frame",
            "The bar along the top of this window is a TitleBar: dragging it moves the window, pressing it twice fills the screen with it, and its buttons are drawn where the desktop leaves them to the window. The WindowFrame around everything draws the border and the shadow of a floating window and resizes it from its edges; a tiled window has none of them.",
            v_flex().child(
                div()
                    .text_sm()
                    .text_color(theme.colors.muted_foreground)
                    .child("Both are this window's own."),
            ),
            cx,
        ),
        section(
            "Status bar",
            "How things stand at its start, and what is counted at its end. This window has one at its foot.",
            bar,
            cx,
        ),
    ]
}
