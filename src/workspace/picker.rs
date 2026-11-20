//! The model and the options of the session shown, which the session keeps.
//! The provider and the model are the parameters of the host's `use`, with
//! the values it lists for them; the options are those the provider
//! describes, each changed with the host's `session-set`, which checks it
//! against what the provider describes. A run in progress goes on with what
//! it started with.

use std::rc::Rc;

use gpui::{
    App, Context, Entity, ParentElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde_json::{Value as Json, json};
use ui::{ActiveTheme as _, Toast, v_flex};

use super::form::{Field, Form};
use super::{Feedback, Pending, Workspace};
use crate::choices::{self, Change, Role};
use crate::conversation::Source;
use crate::protocol::{self, DescribeResult, Function, Parameter};

/// What the picker opened with: the parameters of `use` and the options,
/// in the order of the form's fields.
struct Opened {
    rows: Vec<(Role, Parameter)>,
    initial: Vec<String>,
    /// The provider in use when it opened, and where its row is.
    provider: String,
    provider_row: Option<usize>,
}

impl Workspace {
    /// Asks the provider what can be chosen, to open the picker with it.
    pub(super) fn open_picker(&mut self, cx: &mut Context<Self>) {
        if self.ended.is_some() || self.picking {
            return;
        }

        self.picking = true;
        self.notice = Some("Asking the provider what can be chosen".into());
        self.request(protocol::METHOD_DESCRIBE, Json::Null, Pending::Picker);
        cx.notify();
    }

    /// Opens the picker with what the provider described. A provider that
    /// could not say still leaves the provider and the model to change.
    pub(super) fn described_for_picker(
        &mut self,
        described: Result<DescribeResult, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.picking = false;
        self.notice = None;

        let using = self.host_function(protocol::FUNCTION_USE).cloned();
        let options = self.host_function(protocol::FUNCTION_SESSION_SET).is_some();

        let (provider, model) = match &described {
            Ok(described) => (described.provider.clone(), described.model.clone()),
            Err(_) => (self.provider.clone(), self.model.clone()),
        };

        let mut fields = Vec::new();
        let mut rows = Vec::new();

        for parameter in using.iter().flat_map(|function| &function.params) {
            let initial = match parameter.name.as_str() {
                "provider" => provider.clone(),
                "model" => model.clone(),
                _ => String::new(),
            };

            rows.push((Role::Use, parameter.clone()));
            fields.push(Field {
                parameter: parameter.clone(),
                source: Source::Host,
                initial,
                empty: parameter.optional.then(|| "Leave out".to_string()),
                heading: None,
            });
        }

        let (offered, provider_functions, problem): (Vec<Parameter>, Vec<Function>, _) =
            match described {
                Ok(described) if options => {
                    for (index, option) in described.offers.options.iter().enumerate() {
                        rows.push((Role::Option, option.clone()));
                        fields.push(Field {
                            parameter: option.clone(),
                            source: Source::Provider,
                            initial: choices::in_force(option, described.options.as_ref()),
                            empty: Some("As the provider has it".into()),
                            heading: (index == 0).then(|| format!("Options of {provider}")),
                        });
                    }

                    (described.offers.options, described.offers.functions, None)
                }

                Ok(described) => (Vec::new(), described.offers.functions, None),

                Err(problem) => (
                    Vec::new(),
                    Vec::new(),
                    Some(format!(
                        "{provider} could not say what it offers: {problem}"
                    )),
                ),
            };

        if fields.is_empty() {
            self.notice = Some(problem.unwrap_or_else(|| {
                "Neither the host nor the provider offer anything to choose".into()
            }));
            return;
        }

        let opened = Rc::new(Opened {
            initial: fields.iter().map(|field| field.initial.clone()).collect(),
            provider_row: rows
                .iter()
                .position(|(role, parameter)| *role == Role::Use && parameter.name == "provider"),
            provider,
            rows,
        });

        let form = self.open_form(fields, provider_functions, window, cx);
        let without_options = offered.is_empty() && problem.is_none();

        self.show_picker(form, opened, problem, without_options, window, cx);
    }

    fn show_picker(
        &mut self,
        form: Entity<Form>,
        opened: Rc<Opened>,
        problem: Option<String>,
        without_options: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workspace = cx.weak_entity();
        let toasts = self.toasts.clone();
        let focused = form.clone();

        self.dialogs.update(cx, |dialogs, cx| {
            dialogs.open(
                move |dialog, _, cx| {
                    let values = form.read(cx).values(cx);

                    // The options shown are the provider's in use; another
                    // provider has others.
                    let other_provider = opened
                        .provider_row
                        .and_then(|row| values.get(row))
                        .filter(|chosen| chosen.trim() != opened.provider)
                        .map(|chosen| {
                            format!(
                                "The options of {} can be chosen once the session uses it.",
                                chosen.trim()
                            )
                        });

                    let apply = {
                        let form = form.clone();
                        let opened = opened.clone();
                        let workspace = workspace.clone();
                        let toasts = toasts.clone();

                        move |_: &mut Window, cx: &mut App| {
                            let after = form.read(cx).values(cx);
                            let rows: Vec<choices::Row> = opened
                                .rows
                                .iter()
                                .zip(&opened.initial)
                                .zip(&after)
                                .map(|(((role, parameter), before), after)| choices::Row {
                                    role: *role,
                                    parameter,
                                    before,
                                    after,
                                })
                                .collect();

                            match choices::changes(&rows) {
                                Ok(changes) => {
                                    let _ = workspace
                                        .update(cx, |this, cx| this.change_session(changes, cx));
                                    true
                                }

                                Err(problem) => {
                                    toasts.update(cx, |toasts, cx| {
                                        toasts.push(Toast::new(problem).danger(), cx);
                                    });
                                    false
                                }
                            }
                        }
                    };

                    let colors = cx.theme().colors;

                    let first = form.read(cx).first_focus(cx);

                    let dialog = match first {
                        Some(first) => dialog.initial_focus(first),
                        None => dialog,
                    };

                    dialog
                        .title("Model and Options")
                        .width(px(480.))
                        .child(
                            v_flex()
                                .gap_3()
                                .child(div().text_sm().text_color(colors.muted_foreground).child(
                                    "The session keeps what is chosen here, for the tasks that start after. A task in progress goes on with what it started with.",
                                ))
                                .child(form.clone())
                                .when(without_options, |column| {
                                    column.child(
                                        div()
                                            .text_xs()
                                            .text_color(colors.muted_foreground)
                                            .child(format!(
                                                "{} describes no options.",
                                                opened.provider
                                            )),
                                    )
                                })
                                .when_some(problem.clone(), |column, problem| {
                                    column.child(
                                        div().text_xs().text_color(colors.danger).child(problem),
                                    )
                                })
                                .when_some(other_provider, |column, note| {
                                    column.child(
                                        div().text_xs().text_color(colors.warning).child(note),
                                    )
                                }),
                        )
                        .on_ok("Apply", apply)
                },
                window,
                cx,
            )
        });

        // After the dialog opens, which remembers where the keyboard was,
        // to give it back on closing.
        focused.update(cx, |form, cx| form.focus(window, cx));
    }

    /// Asks the host for the changes the picker holds, in order. `use`
    /// goes first and is answered before the next is read; the options
    /// that follow may be answered together, and rumbo lays each over what
    /// the session has then, so none is lost.
    fn change_session(&mut self, changes: Vec<Change>, cx: &mut Context<Self>) {
        for change in changes {
            let (name, params) = match change {
                Change::Use(params) => (protocol::FUNCTION_USE, params),
                Change::Option(change) => {
                    (protocol::FUNCTION_SESSION_SET, json!({ "change": change }))
                }
            };

            self.call_host(name, Some(params), Feedback::Notice, cx);
        }
    }
}
