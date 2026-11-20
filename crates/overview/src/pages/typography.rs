use gpui::{
    AnyElement, Context, FontWeight, IntoElement as _, ParentElement as _, Styled as _, Window,
    div, px,
};
use ui::{ActiveTheme as _, Button, Sizable as _, Typeface, h_flex, v_flex};

use super::{Demos, row, section};
use crate::Overview;

/// The sizes the components' text comes in, by the name GPUI gives them.
const SIZES: [(&str, f32); 7] = [
    ("3xl", 30.),
    ("2xl", 24.),
    ("xl", 20.),
    ("lg", 18.),
    ("base", 16.),
    ("sm", 14.),
    ("xs", 12.),
];

const WEIGHTS: [(&str, FontWeight); 4] = [
    ("Regular", FontWeight::NORMAL),
    ("Medium", FontWeight::MEDIUM),
    ("Semibold", FontWeight::SEMIBOLD),
    ("Bold", FontWeight::BOLD),
];

const CHARACTERS: &str = "ABCDEFGHIJKLMNÑOPQRSTUVWXYZ abcdefghijklmnñopqrstuvwxyz\n0123456789 áéíóú ü ¿qué? ¡sí! (…) [—] {·} @ # & % → «»";

const PARAGRAPH: &str = "A run is the work from an objective until the model stops calling tools, or the run fails or is cancelled. A session holds one or more runs, and keeps its provider, its model, and the options chosen in it.";

pub fn render(_: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    Typeface::ALL
        .iter()
        .enumerate()
        .map(|(index, typeface)| specimen(index, *typeface, cx))
        .collect()
}

/// A typeface at every size and weight the components use, drawn in it
/// whichever is in force.
fn specimen(index: usize, typeface: Typeface, cx: &mut Context<Overview>) -> AnyElement {
    let theme = cx.theme();
    let muted = theme.colors.muted_foreground;
    let in_force = theme.typeface == typeface;

    let about = match (typeface.is_loaded(cx), typeface) {
        (false, _) => "It did not load: what shows here is the desktop's font.",
        (true, Typeface::Montserrat) => "Geometric and plain, for work. Bundled.",
        (true, Typeface::ShantellSans) => "Hand-drawn and friendly. Bundled.",
        (true, Typeface::System) => "The desktop's interface font.",
    };

    let using = Button::new(("use-typeface", index))
        .small()
        .selected(in_force)
        .on_click(cx.listener(move |this, _, _, cx| this.use_typeface(typeface, cx)));
    let using = match in_force {
        true => using.primary().label("In Force"),
        false => using.outline().label("Use It"),
    };

    let sizes = v_flex().gap_2().children(SIZES.iter().map(|(name, size)| {
        h_flex()
            .items_baseline()
            .gap_4()
            .child(
                div()
                    .w_10()
                    .flex_none()
                    .text_xs()
                    .text_color(muted)
                    .child(*name),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(*size))
                    .child("Give rumbo a task, and follow its work"),
            )
    }));

    let weights = row().gap_6().children(WEIGHTS.iter().map(|(name, weight)| {
        div()
            .text_lg()
            .font_weight(*weight)
            .child(format!("{name} Aa Gg 0123"))
    }));

    let example = v_flex()
        .gap_6()
        .font_family(typeface.family())
        .child(h_flex().justify_end().child(using))
        .child(sizes)
        .child(weights)
        .child(div().text_base().child(CHARACTERS))
        .child(div().max_w(px(560.)).text_sm().child(PARAGRAPH));

    section(typeface.name(), about, example.into_any_element(), cx)
}
