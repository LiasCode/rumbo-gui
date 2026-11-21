use gpui::{AnyElement, Context, ParentElement as _, Styled as _, Window, px};
use ui::{ActiveTheme as _, Meter, Tone, h_flex, v_flex};

use super::section;
use crate::Overview;

pub fn render(_: &mut super::Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let muted = cx.theme().colors.muted_foreground;

    let shares = v_flex().w(px(320.)).gap_4().children(
        [(0., "Empty"), (0.3, "30%"), (1., "Full")]
            .into_iter()
            .map(|(share, said)| {
                v_flex()
                    .gap_1()
                    .child(Meter::new(share))
                    .child(h_flex().text_xs().text_color(muted).child(said))
            }),
    );

    let tones = v_flex().w(px(320.)).gap_4().children(
        [
            (0.55, Tone::Success, "Success"),
            (0.8, Tone::Warning, "Warning, nearly full"),
            (0.95, Tone::Danger, "Danger, all but full"),
        ]
        .into_iter()
        .map(|(share, tone, said)| {
            v_flex()
                .gap_1()
                .child(Meter::new(share).tone(tone))
                .child(h_flex().text_xs().text_color(muted).child(said))
        }),
    );

    vec![
        section(
            "Filled",
            "As far as the share used, from empty to full, in the primary color.",
            shares,
            cx,
        ),
        section(
            "Tones",
            "In the color of a tone, as a bar nearly full says it is.",
            tones,
            cx,
        ),
    ]
}
