//! A provider's function that talks with the person while it works, as a
//! sign-in does: what it says shows in the conversation, what it asks opens
//! a dialog, and Escape gives it up. A secret answer is not shown while it
//! is typed, and is kept nowhere.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Focusable as _, ParentElement as _, Styled as _, Window, div,
};
use serde_json::json;
use ui::{ActiveTheme as _, Input, InputState, rems_from_px, v_flex};

use super::Workspace;
use crate::protocol::{self, AskParams, CODE_INTERNAL, CODE_INVALID_PARAMS, CancelParams};

/// A function of the provider in progress, called from this window.
pub(super) struct Talking {
    /// The id of its invoke request.
    pub(super) invoke: i64,
    pub(super) name: String,
}

impl Workspace {
    /// Gives up the function in progress, which then answers with an error.
    pub(super) fn give_up(&mut self, cx: &mut Context<Self>) {
        let Some(talking) = &self.talking else {
            return;
        };

        let cancel = CancelParams {
            run: 0,
            invoke: talking.invoke,
        };

        if let Err(error) = self.host.connection.notify(protocol::METHOD_CANCEL, cancel) {
            self.notice = Some(format!("The cancel could not reach the host: {error}"));
        } else {
            self.notice = Some(format!("Giving up {}", talking.name));
        }

        cx.notify();
    }

    /// Asks the person what a function of the provider asks, and answers
    /// the host with what they typed, or with an error when they give up.
    pub(super) fn asked(
        &mut self,
        id: i64,
        params: AskParams,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(talking) = self
            .talking
            .as_ref()
            .filter(|talking| talking.invoke == params.invoke)
        else {
            let _ = self.host.connection.refuse(
                id,
                CODE_INVALID_PARAMS,
                "this front end is not following that function, so it cannot answer for it",
            );

            return;
        };

        let name = talking.name.clone();
        let secret = params.secret;
        let field = cx.new(|cx| InputState::new(window, cx).masked(secret));
        let connection = self.host.connection.clone();
        let workspace = cx.weak_entity();

        // The dialog answers once, whichever way it closes.
        let answered = Rc::new(Cell::new(false));

        let answer = {
            let field = field.clone();
            let connection = connection.clone();
            let answered = answered.clone();

            move |window: &mut Window, cx: &mut App| {
                if answered.replace(true) {
                    return true;
                }

                let text = field.read(cx).value().to_string();

                // Nothing of a secret stays in the field once it is sent.
                field.update(cx, |state, cx| state.set_value("", window, cx));

                let _ = connection.answer(id, json!({ "text": text }));

                true
            }
        };

        let decline = {
            let answered = answered.clone();

            move |_: &mut Window, cx: &mut App| {
                if answered.replace(true) {
                    return true;
                }

                let _ = connection.refuse(id, CODE_INTERNAL, "the person did not answer");
                let _ = workspace.update(cx, |this, cx| this.give_up(cx));

                true
            }
        };

        let question = params.text;
        let shown = field.clone();

        self.dialogs.update(cx, |dialogs, cx| {
            dialogs.open(
                move |dialog, _, cx| {
                    let hint = if secret {
                        "What is typed here is not shown, and not kept."
                    } else {
                        ""
                    };
                    let answer = answer.clone();
                    let decline = decline.clone();

                    dialog
                        .title(name.clone())
                        .width(rems_from_px(440.))
                        .close_on_backdrop(false)
                        .initial_focus(shown.focus_handle(cx))
                        .child(
                            v_flex()
                                .gap_2()
                                .child(div().text_sm().child(question.clone()))
                                .child(Input::new(&shown))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().colors.muted_foreground)
                                        .child(hint),
                                ),
                        )
                        .on_ok("Answer", move |window, cx| answer(window, cx))
                        .on_cancel("Give Up", move |window, cx| decline(window, cx))
                },
                window,
                cx,
            )
        });
    }
}
