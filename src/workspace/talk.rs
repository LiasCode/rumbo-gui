//! A function of a provider or of an extension that talks with the person
//! while it works, as a sign-in does: what it says shows in the
//! conversation, what it asks opens a dialog, and Escape gives it up. A
//! tool the model called can ask too, as one that asks before it runs a
//! command: its question opens the same dialog, and is over with its run.
//! A secret answer is not shown while it is typed, and is kept nowhere.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Focusable as _, ParentElement as _, Styled as _, Window, div,
};
use serde_json::json;
use ui::{ActiveTheme as _, Input, InputState, Toast, rems_from_px, v_flex};

use super::Workspace;
use crate::conversation::Source;
use crate::protocol::{self, AskParams, CODE_INTERNAL, CODE_INVALID_PARAMS, CancelParams};

/// A function of the provider or of an extension in progress, called from
/// this window.
pub(super) struct Talking {
    /// The host it was asked of, and the id of its invoke request there.
    pub(super) host: u64,
    pub(super) invoke: i64,
    /// Whose it is: the provider's, or an extension's.
    pub(super) source: Source,
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

        let Some(held) = self.held(talking.host) else {
            return;
        };

        let said = match held.host.connection.notify(protocol::METHOD_CANCEL, cancel) {
            Err(error) => {
                Toast::new(format!("The cancel could not reach the host: {error}")).danger()
            }
            Ok(()) => Toast::new(format!("Giving up {}", talking.name)),
        };

        self.toast(said, cx);
    }

    /// Asks the person what a function asks, or a tool of a run of this
    /// window, and answers the host with what they typed, or with an error
    /// when they give up, which gives the function up too, and leaves the
    /// tool to decide what no answer means.
    pub(super) fn asked(
        &mut self,
        serial: u64,
        id: i64,
        params: AskParams,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(connection) = self.held(serial).map(|held| held.host.connection.clone()) else {
            return;
        };

        let function = self
            .talking
            .as_ref()
            .filter(|talking| talking.host == serial && talking.invoke == params.invoke)
            .map(|talking| talking.name.clone());

        // A tool's question names its run, which is one of this window's.
        let tool = params.invoke == 0 && self.runs.contains_key(&params.run);

        let name = match function {
            Some(name) => name,

            None if tool => {
                let session = self
                    .runs
                    .get(&params.run)
                    .filter(|session| **session != self.current || self.blank)
                    .map(|session| self.title_of(session));

                match session {
                    Some(title) => format!("{} asks, in “{title}”", params.name),
                    None => format!("{} asks", params.name),
                }
            }

            None => {
                let _ = connection.refuse(
                    id,
                    CODE_INVALID_PARAMS,
                    "this front end is not following that function or that run, so it cannot answer for it",
                );

                return;
            }
        };

        let secret = params.secret;
        let field = cx.new(|cx| InputState::new(window, cx).masked(secret));
        let workspace = cx.weak_entity();

        // The dialog answers once, whichever way it closes.
        let answered = Rc::new(Cell::new(false));

        if tool {
            self.tool_question = Some((params.run, answered.clone()));
        }

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

                // A run goes on without the answer; a function is given up.
                if !tool {
                    let _ = workspace.update(cx, |this, cx| this.give_up(cx));
                }

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
                        .on_cancel(
                            if tool { "Decline" } else { "Give Up" },
                            move |window, cx| decline(window, cx),
                        )
                },
                window,
                cx,
            )
        });
    }
}
