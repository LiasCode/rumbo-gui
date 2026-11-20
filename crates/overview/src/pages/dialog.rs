use gpui::{
    AnyElement, AppContext as _, Context, Entity, Focusable as _, ParentElement as _, SharedString,
    Styled as _, Window, div,
};
use ui::{ActiveTheme as _, Button, Input, InputState, v_flex};

use super::{Demos, note, row, section, told};
use crate::Overview;

pub struct Demo {
    /// The field the dialog that asks has.
    answer: Entity<InputState>,
    told: Vec<SharedString>,
}

impl Demo {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        Demo {
            answer: cx.new(|cx| InputState::new(window, cx).placeholder("toy")),
            told: Vec::new(),
        }
    }
}

/// Keeps what a dialog told.
fn tell(overview: &gpui::WeakEntity<Overview>, line: String, cx: &mut gpui::App) {
    overview
        .update(cx, |this, cx| {
            note(&mut this.demos.dialog.told, line);
            cx.notify();
        })
        .ok();
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let ask = Button::new("ask")
        .outline()
        .label("Ask a question")
        .on_click(cx.listener(|this, _, window, cx| {
            let field = this.demos.dialog.answer.clone();
            let overview = cx.weak_entity();

            this.dialogs.update(cx, |dialogs, cx| {
                dialogs.open(
                    move |dialog, _, cx| {
                        let (answering, declining) = (overview.clone(), overview.clone());
                        let answered = field.clone();

                        dialog
                            .title("Which provider?")
                            .child(
                                div()
                                    .text_sm()
                                    .child("The function asks which provider to set up."),
                            )
                            .child(Input::new(&field))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().colors.muted_foreground)
                                    .child("Enter answers, and Escape gives up."),
                            )
                            .initial_focus(field.focus_handle(cx))
                            .on_ok("Answer", move |window, cx| {
                                let text = answered.read(cx).value().to_string();

                                answered.update(cx, |field, cx| field.set_value("", window, cx));
                                tell(&answering, format!("Answered {text:?}"), cx);
                                true
                            })
                            .on_cancel("Give Up", move |_, cx| {
                                tell(&declining, "Gave up".to_string(), cx);
                                true
                            })
                    },
                    window,
                    cx,
                )
            });
        }));

    let confirm = Button::new("confirm")
        .outline()
        .label("Delete a session")
        .on_click(cx.listener(|this, _, window, cx| {
            let overview = cx.weak_entity();

            this.dialogs.update(cx, |dialogs, cx| {
                dialogs.open(
                    move |dialog, _, cx| {
                        let deleting = overview.clone();

                        dialog
                            .title("Delete this session?")
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().colors.muted_foreground)
                                    .child("“Port the composer” and its history go for good."),
                            )
                            .danger()
                            .on_ok("Delete", move |_, cx| {
                                tell(&deleting, "Deleted the session".to_string(), cx);
                                true
                            })
                    },
                    window,
                    cx,
                )
            });
        }));

    let nested = Button::new("nested")
        .outline()
        .label("One over another")
        .on_click(cx.listener(|this, _, window, cx| {
            let dialogs = this.dialogs.clone();

            this.dialogs.update(cx, |opened, cx| {
                opened.open(
                    move |dialog, _, _| {
                        let dialogs = dialogs.clone();

                        dialog
                            .title("First")
                            .child(
                                Button::new("open-second")
                                    .outline()
                                    .label("Open another")
                                    .on_click(move |_, window, cx| {
                                        dialogs.update(cx, |dialogs, cx| {
                                            dialogs.open(
                                                |dialog, _, _| {
                                                    dialog
                                                        .title("Second")
                                                        .child("It closes before the one under it.")
                                                },
                                                window,
                                                cx,
                                            )
                                        })
                                    }),
                            )
                            .on_ok("Done", |_, _| true)
                    },
                    window,
                    cx,
                )
            });
        }));

    vec![
        section(
            "A question",
            "A modal window over the work, which waits for an answer. Enter in its field answers; on a button it presses that button. Escape, the button that gives up, and the close button give up.",
            row().child(ask).child(confirm).child(nested),
            cx,
        ),
        section(
            "What they told",
            "",
            v_flex().child(told(&demos.dialog.told, cx)),
            cx,
        ),
    ]
}
