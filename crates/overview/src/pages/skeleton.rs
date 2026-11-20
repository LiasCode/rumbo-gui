use gpui::{AnyElement, Context, ParentElement as _, Styled as _, Window, px};
use ui::{ActiveTheme as _, Skeleton, h_flex, v_flex};

use super::section;
use crate::Overview;

pub fn render(_: &mut super::Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let theme = cx.theme();

    let sessions = v_flex().w(px(280.)).gap_4().children((0..4).map(|index| {
        h_flex()
            .gap_3()
            .child(Skeleton::new().size(px(32.)).rounded_full())
            .child(
                v_flex()
                    .flex_1()
                    .gap_2()
                    .child(Skeleton::new().w(px(160. - index as f32 * 16.)))
                    .child(Skeleton::new().w(px(96.)).h_3()),
            )
    }));

    let paragraph = v_flex()
        .w(px(480.))
        .gap_2()
        .child(Skeleton::new())
        .child(Skeleton::new())
        .child(Skeleton::new().w(px(300.)));

    let card = v_flex()
        .w(px(320.))
        .gap_3()
        .p_4()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.colors.border)
        .child(Skeleton::new().h(px(120.)).rounded(theme.radius_lg))
        .child(Skeleton::new().w(px(200.)))
        .child(Skeleton::new().w(px(140.)).h_3());

    vec![
        section(
            "A list",
            "Where the sessions will show while they are listed.",
            sessions,
            cx,
        ),
        section(
            "Lines",
            "As long as a line of text unless given a size.",
            paragraph,
            cx,
        ),
        section(
            "A card",
            "Any shape: it is styled as any element is.",
            card,
            cx,
        ),
    ]
}
