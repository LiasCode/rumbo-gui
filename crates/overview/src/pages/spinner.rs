use gpui::{AnyElement, Context, ParentElement as _, Styled as _, Window, div};
use ui::{ActiveTheme as _, Sizable as _, Size, Spinner, h_flex, v_flex};

use super::{row, section};
use crate::Overview;

const SIZES: [(Size, &str); 4] = [
    (Size::XSmall, "Extra small"),
    (Size::Small, "Small"),
    (Size::Medium, "Medium"),
    (Size::Large, "Large"),
];

pub fn render(_: &mut super::Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let colors = cx.theme().colors;

    let sizes = row().gap_8().children(SIZES.iter().map(|(size, name)| {
        v_flex()
            .items_center()
            .gap_2()
            .child(Spinner::new().with_size(*size))
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(*name),
            )
    }));

    let beside = v_flex()
        .gap_3()
        .child(
            h_flex()
                .gap_2()
                .text_sm()
                .text_color(colors.muted_foreground)
                .child(Spinner::new().small())
                .child("Reading the changes in the project's folder…"),
        )
        .child(
            h_flex()
                .gap_2()
                .text_sm()
                .child(Spinner::new().xsmall())
                .child("Port the composer to the new components"),
        )
        .child(
            h_flex()
                .gap_2()
                .text_sm()
                .child(Spinner::new().small().color(colors.success))
                .child("In a color of its own"),
        );

    vec![
        section("Sizes", "As large as an icon of the same size.", sizes, cx),
        section(
            "Beside words",
            "It takes the color of the text around it, as a session that is working shows it.",
            beside,
            cx,
        ),
    ]
}
