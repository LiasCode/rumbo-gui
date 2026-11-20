use gpui::{AnyElement, Context, ParentElement as _, SharedString, Styled as _, Window};
use ui::{Button, Toast, Tone, v_flex};

use super::{Demos, note, row, section, told};
use crate::Overview;

#[derive(Default)]
pub struct Demo {
    told: Vec<SharedString>,
}

/// A button that pushes the toast `make` builds.
fn show(
    id: &'static str,
    label: &'static str,
    make: impl Fn(&mut Context<Overview>) -> Toast + 'static,
    cx: &mut Context<Overview>,
) -> Button {
    Button::new(id)
        .outline()
        .label(label)
        .on_click(cx.listener(move |this, _, _, cx| {
            let toast = make(cx);

            this.toasts.update(cx, |toasts, cx| {
                toasts.push(toast, cx);
            });
        }))
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let tones = row()
        .child(show(
            "news",
            "News",
            |_| Toast::new("The session was renamed").description("It is now “Port the composer”."),
            cx,
        ))
        .child(show(
            "success",
            "A success",
            |_| {
                Toast::new("A run finished in rumbo-gui")
                    .description("Port the composer to the new components")
                    .tone(Tone::Success)
            },
            cx,
        ))
        .child(show(
            "warning",
            "A warning",
            |_| {
                Toast::new("The provider has no models")
                    .description("Runs use its own until one is chosen.")
                    .warning()
            },
            cx,
        ))
        .child(show(
            "danger",
            "A problem",
            |_| {
                Toast::new("The folder could not be chosen")
                    .description("The desktop's folder dialog did not answer.")
                    .danger()
            },
            cx,
        ));

    let more = row()
        .child(show(
            "action",
            "With an action",
            |cx| {
                let overview = cx.weak_entity();

                Toast::new("A run finished in rumbo")
                    .description("Review the diff before the release")
                    .success()
                    .action("Open", move |_, _, cx| {
                        overview
                            .update(cx, |this, cx| {
                                note(&mut this.demos.toast.told, "Opened the session");
                                cx.notify();
                            })
                            .ok();
                    })
            },
            cx,
        ))
        .child(show(
            "persistent",
            "Until put away",
            |_| {
                Toast::new("rumbo serve stopped")
                    .description("The host of rumbo-gui ended; open the project again to go on.")
                    .danger()
                    .persistent()
            },
            cx,
        ))
        .child(
            Button::new("clear")
                .ghost()
                .label("Put all away")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toasts.update(cx, |toasts, cx| toasts.clear(cx));
                })),
        );

    vec![
        section(
            "Tones",
            "In the window's bottom right corner, over everything. Each goes after five seconds, and none goes while the pointer is on them.",
            tones,
            cx,
        ),
        section(
            "What can be done",
            "A button that does something about it, which puts it away; and one that stays until it is put away. Tab reaches their buttons after everything else.",
            v_flex().gap_3().child(more),
            cx,
        ),
        section("What they told", "", told(&demos.toast.told, cx), cx),
    ]
}
