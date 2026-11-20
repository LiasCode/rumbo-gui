use gpui::{
    AnyElement, Context, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div,
    px,
};
use ui::{ActiveTheme as _, Label, Slider, h_flex, v_flex};

use super::{Demos, note, section, told};
use crate::Overview;

pub struct Demo {
    width: f32,
    temperature: f32,
    share: f32,
    told: Vec<SharedString>,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            width: 800.,
            temperature: 0.7,
            share: 37.5,
            told: Vec::new(),
        }
    }
}

/// A slider under its name, with the value it holds beside the name.
fn labeled(name: &str, value: String, slider: Slider, cx: &Context<Overview>) -> impl IntoElement {
    v_flex()
        .w(px(320.))
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .child(Label::new(name.to_string()))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().colors.muted_foreground)
                        .child(value),
                ),
        )
        .child(slider)
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.slider;

    let width = labeled(
        "Conversation width",
        format!("{:.0} px", demo.width),
        Slider::new("width")
            .range(560., 1280.)
            .step(40.)
            .value(demo.width)
            .on_change(cx.listener(|this, width: &f32, _, cx| {
                let demo = &mut this.demos.slider;

                demo.width = *width;
                note(&mut demo.told, format!("Conversation width: {width:.0}"));
                cx.notify();
            })),
        cx,
    );

    let temperature = labeled(
        "Temperature",
        format!("{:.1}", demo.temperature),
        Slider::new("temperature")
            .range(0., 1.)
            .step(0.1)
            .value(demo.temperature)
            .on_change(cx.listener(|this, temperature: &f32, _, cx| {
                let demo = &mut this.demos.slider;

                demo.temperature = *temperature;
                note(&mut demo.told, format!("Temperature: {temperature:.1}"));
                cx.notify();
            })),
        cx,
    );

    let share = labeled(
        "Share of the context",
        format!("{:.1} %", demo.share),
        Slider::new("share")
            .step(0.)
            .value(demo.share)
            .on_change(cx.listener(|this, share: &f32, _, cx| {
                this.demos.slider.share = *share;
                cx.notify();
            })),
        cx,
    );

    let disabled = labeled(
        "Not now",
        "60".to_string(),
        Slider::new("disabled").value(60.).disabled(true),
        cx,
    );

    vec![
        section(
            "In steps",
            "Drag the thumb, or press the track to bring it there and go on dragging. Tab reaches it; the arrows move it a step, Page Up and Page Down ten, and Home and End to its ends.",
            v_flex().gap_8().child(width).child(temperature),
            cx,
        ),
        section(
            "Any value",
            "Without steps it takes any value between its ends, and a key moves it a hundredth of the way.",
            share,
            cx,
        ),
        section("Disabled", "", disabled, cx),
        section("What they told", "", told(&demo.told, cx), cx),
    ]
}
