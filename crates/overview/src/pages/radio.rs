use gpui::{AnyElement, Context, ParentElement as _, SharedString, Styled as _, Window};
use ui::{Label, Radio, RadioGroup, Sizable as _, v_flex};

use super::{Demos, note, section, told};
use crate::Overview;

pub struct Demo {
    /// The option each example has chosen.
    chosen: [Option<usize>; 3],
    told: Vec<SharedString>,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            chosen: [Some(0), None, Some(1)],
            told: Vec::new(),
        }
    }
}

const PROVIDERS: [(&str, &str); 3] = [
    ("anthropic", "Claude's models, with an API key."),
    ("openai", "GPT's models, with an API key."),
    ("toy", "Needs no model; it answers with what it is asked."),
];

const THEMES: [&str; 3] = ["Light", "Dark", "As the desktop"];

const SIGNED: [&str; 3] = ["anthropic", "openai", "chatgpt"];

/// A group whose choice the page keeps, numbered among them, telling it by
/// the names of its options.
fn group(
    index: usize,
    names: &'static [&'static str],
    demos: &Demos,
    cx: &mut Context<Overview>,
) -> RadioGroup {
    RadioGroup::new(("radios", index))
        .selected(demos.radio.chosen[index])
        .on_change(cx.listener(move |this, to: &usize, _, cx| {
            let demo = &mut this.demos.radio;

            demo.chosen[index] = Some(*to);
            note(&mut demo.told, format!("Chose {}", names[*to]));
            cx.notify();
        }))
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    const NAMES: [&str; 3] = [PROVIDERS[0].0, PROVIDERS[1].0, PROVIDERS[2].0];

    let providers = group(0, &NAMES, demos, cx).children(
        PROVIDERS
            .iter()
            .map(|(name, about)| Radio::new(*name).description(*about)),
    );

    let themes = v_flex().gap_2().child(Label::new("Theme")).child(
        group(1, &THEMES, demos, cx)
            .horizontal()
            .small()
            .children(THEMES.iter().map(|name| Radio::new(*name))),
    );

    let disabled = v_flex()
        .gap_6()
        .child(
            group(2, &SIGNED, demos, cx)
                .child(Radio::new(SIGNED[0]))
                .child(Radio::new(SIGNED[1]))
                .child(
                    Radio::new(SIGNED[2])
                        .description("Sign in from the settings first.")
                        .disabled(true),
                ),
        )
        .child(
            RadioGroup::new("all-disabled")
                .disabled(true)
                .selected(Some(0))
                .child(Radio::new("Ask before each tool call"))
                .child(Radio::new("Run tools without asking")),
        );

    vec![
        section(
            "One of a few",
            "Pressing an option or its words chooses it. The group is one stop for Tab, on the option chosen; the arrows choose the one beside, and Space the first while none is.",
            providers,
            cx,
        ),
        section("Side by side, none chosen yet", "", themes, cx),
        section(
            "Disabled",
            "An option that cannot be chosen now, which the arrows pass, and a whole group.",
            disabled,
            cx,
        ),
        section("What they told", "", told(&demos.radio.told, cx), cx),
    ]
}
