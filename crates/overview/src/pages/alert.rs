use gpui::{
    AnyElement, Context, IntoElement, ParentElement as _, SharedString, Styled as _, Window, px,
};
use ui::{Alert, Button, Sizable as _, v_flex};

use super::{Demos, note, section, told};
use crate::Overview;

pub struct Demo {
    /// Whether the alert that can be put away shows.
    shown: bool,
    told: Vec<SharedString>,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            shown: true,
            told: Vec::new(),
        }
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.alert;

    let tones = v_flex()
        .w(px(560.))
        .gap_3()
        .child(
            Alert::new("info")
                .title("The host speaks a newer protocol")
                .description(
                    "Everything the window knows works; what it does not know is left out.",
                ),
        )
        .child(
            Alert::new("success")
                .success()
                .title("The provider is ready")
                .description("toy answered and lists its models."),
        )
        .child(
            Alert::new("warning")
                .warning()
                .title("No model chosen")
                .description("Runs use the provider's own until one is chosen in the picker."),
        )
        .child(
            Alert::new("danger")
                .danger()
                .title("The run failed")
                .description("The provider stopped before it answered: connection refused."),
        );

    let acted = match demo.shown {
        true => Alert::new("acted")
            .danger()
            .title("The changes could not be read")
            .description("Git is not installed, or the folder is not in a repository.")
            .child(
                Button::new("retry")
                    .small()
                    .outline()
                    .label("Read them again")
                    .on_click(cx.listener(|this, _, _, cx| {
                        note(&mut this.demos.alert.told, "Read the changes again");
                        cx.notify();
                    })),
            )
            .on_close(cx.listener(|this, _, _, cx| {
                let demo = &mut this.demos.alert;

                demo.shown = false;
                note(&mut demo.told, "Put the alert away");
                cx.notify();
            }))
            .into_any_element(),

        false => Button::new("again")
            .outline()
            .label("Show it again")
            .on_click(cx.listener(|this, _, _, cx| {
                this.demos.alert.shown = true;
                cx.notify();
            }))
            .into_any_element(),
    };

    let short = v_flex()
        .w(px(560.))
        .gap_3()
        .child(
            Alert::new("title-only")
                .warning()
                .title("The session is being compacted."),
        )
        .child(
            Alert::new("words-only")
                .description("Sessions are kept by rumbo; the window keeps only its preferences."),
        );

    vec![
        section(
            "Tones",
            "News, a success, a warning, and a problem, each with its icon and color.",
            tones,
            cx,
        ),
        section(
            "What can be done",
            "Buttons under its words, and one that puts it away; Tab reaches them.",
            v_flex().w(px(560.)).child(acted),
            cx,
        ),
        section("A title or words alone", "", short, cx),
        section("What they told", "", told(&demo.told, cx), cx),
    ]
}
