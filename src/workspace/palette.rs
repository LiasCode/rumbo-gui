//! The command palette: every function the host and the provider describe,
//! offered by name without knowing any of them, with a form for their
//! parameters and a confirmation for what cannot be undone.

use std::rc::Rc;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, WindowExt as _,
    button::{Button, ButtonVariant, ButtonVariants as _},
    command::{Command as Palette, CommandGroup, CommandItem, CommandState},
    dialog::{DialogAction, DialogClose, DialogFooter},
    input::{Input, InputState},
    v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, ParentElement as _, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde_json::Value as Json;

use super::{OpenPalette, Workspace};
use crate::commands::{self, Command};
use crate::conversation::Source;

/// The palette's commands by the section and row the palette gives them.
/// A row with nothing is a line that explains, not a command.
type Sections = Rc<Vec<Vec<Option<Command>>>>;

impl Workspace {
    pub(super) fn open_palette(
        &mut self,
        _: &OpenPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ended.is_some() || window.has_active_dialog(cx) {
            return;
        }

        let host: Vec<Option<Command>> = self
            .host_functions
            .iter()
            .map(|function| {
                Some(Command {
                    source: Source::Host,
                    function: function.clone(),
                })
            })
            .collect();

        let provider: Vec<Option<Command>> = match &self.provider_functions {
            Ok(functions) => functions
                .iter()
                .map(|function| {
                    Some(Command {
                        source: Source::Provider,
                        function: function.clone(),
                    })
                })
                .collect(),

            Err(_) => vec![None],
        };

        let mut labels = Vec::new();
        let mut sections = Vec::new();

        if !host.is_empty() {
            labels.push("rumbo".to_string());
            sections.push(host);
        }

        if !provider.is_empty() {
            labels.push(format!("Provider {}", self.provider));
            sections.push(provider);
        }

        let sections: Sections = Rc::new(sections);
        let labels = Rc::new(labels);
        let unavailable = self.provider_functions.clone().err().unwrap_or_default();
        let state = self.palette.clone();
        let workspace = cx.weak_entity();

        state.update(cx, |state, cx| state.set_query("", window, cx));

        window.open_dialog(cx, move |dialog, _, _| {
            let sections = sections.clone();
            let labels = labels.clone();
            let unavailable = unavailable.clone();
            let state = state.clone();
            let workspace = workspace.clone();

            dialog
                .close_button(false)
                .p_0()
                .content(move |content, _, cx| {
                    content.child(palette(
                        &state,
                        &sections,
                        &labels,
                        &unavailable,
                        workspace.clone(),
                        cx,
                    ))
                })
        });

        // After the dialog opens, which takes the focus and remembers where
        // it was, to give it back on closing.
        self.palette.update(cx, |state, cx| state.focus(window, cx));
    }

    /// Goes on with the command the person chose: asks for its parameters,
    /// or its confirmation, or calls it.
    fn chose(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        if !command.function.params.is_empty() {
            self.ask_parameters(command, window, cx);
        } else {
            self.confirm_and_call(command, None, window, cx);
        }
    }

    /// Asks for the parameters of a command in a form, one field each.
    fn ask_parameters(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        let fields: Vec<Entity<InputState>> = command
            .function
            .params
            .iter()
            .map(|parameter| {
                let hint = match (
                    parameter.description.is_empty(),
                    parameter.choices.is_empty(),
                ) {
                    (false, _) => parameter.description.clone(),
                    (true, false) => parameter.choices.join(", "),
                    (true, true) => String::new(),
                };

                cx.new(|cx| InputState::new(window, cx).placeholder(hint))
            })
            .collect();

        let fields = Rc::new(fields);
        let command = Rc::new(command);
        let workspace = cx.weak_entity();

        let first = fields.first().cloned();

        window.open_dialog(cx, move |dialog, _, cx| {
            let form = command.function.params.iter().zip(fields.iter()).fold(
                v_flex().gap_3(),
                |form, (parameter, field)| {
                    let label = if parameter.optional {
                        format!("{} (optional)", parameter.name)
                    } else {
                        parameter.name.clone()
                    };

                    form.child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().child(label))
                            .child(Input::new(field)),
                    )
                },
            );

            let accepted = {
                let fields = fields.clone();
                let command = command.clone();
                let workspace = workspace.clone();

                move |_: &_, window: &mut Window, cx: &mut gpui_kit::App| {
                    let typed: Vec<String> = fields
                        .iter()
                        .map(|field| field.read(cx).value().to_string())
                        .collect();

                    match commands::arguments(&command.function, &typed) {
                        Ok(params) => {
                            let command = (*command).clone();

                            // The form closes first, so a confirmation can
                            // take its place.
                            window.close_dialog(cx);

                            let _ = workspace.update(cx, |this, cx| {
                                this.confirm_and_call(command, params, window, cx)
                            });

                            false
                        }

                        Err(problem) => {
                            window.push_notification(problem, cx);
                            false
                        }
                    }
                }
            };

            dialog
                .title(command.function.name.clone())
                .w(px(440.))
                .child(
                    v_flex()
                        .gap_3()
                        .when(!command.function.description.is_empty(), |column| {
                            column.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(command.function.description.clone()),
                            )
                        })
                        .child(form),
                )
                .on_ok(accepted)
                .footer(
                    DialogFooter::new()
                        .gap_2()
                        .child(
                            DialogClose::new()
                                .child(Button::new("cancel").outline().label("Cancel")),
                        )
                        .child(
                            DialogAction::new().child(Button::new("run").primary().label("Run")),
                        ),
                )
        });

        // After the dialog opens, which takes the focus and remembers where
        // it was, to give it back on closing.
        if let Some(first) = first {
            first.update(cx, |state, cx| state.focus(window, cx));
        }
    }

    /// Asks the person to confirm a command that cannot be undone, with the
    /// words its description gives, and calls it once they agree.
    fn confirm_and_call(
        &mut self,
        command: Command,
        params: Option<Json>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if command.function.confirm.is_empty() {
            self.call(&command, params, false, cx);
            return;
        }

        let workspace = cx.weak_entity();
        let command = Rc::new(command);

        window.open_alert_dialog(cx, move |alert, _, _| {
            let command = command.clone();
            let params = params.clone();
            let workspace = workspace.clone();

            alert
                .title(command.function.name.clone())
                .description(command.function.confirm.clone())
                .confirm()
                .ok_text(capitalized(&command.function.name))
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    let _ = workspace.update(cx, |this, cx| {
                        this.call(&command, params.clone(), false, cx)
                    });
                    true
                })
        });
    }
}

/// The palette, rebuilt each time the dialog renders from what was
/// described when it opened.
fn palette(
    state: &Entity<CommandState>,
    sections: &Sections,
    labels: &[String],
    unavailable: &str,
    workspace: WeakEntity<Workspace>,
    cx: &gpui_kit::App,
) -> Palette {
    let muted = cx.theme().muted_foreground;
    let mut palette = Palette::new(state)
        .bordered(false)
        .placeholder("Type a command")
        .w(px(480.));

    for (label, section) in labels.iter().zip(sections.iter()) {
        let items = section.iter().map(|command| match command {
            Some(command) => {
                let name = command.function.name.clone();
                let description = command.function.description.clone();
                let shown = (name.clone(), description.clone());

                CommandItem::new()
                    // Matched by name alone, as rumbo's terminal UI
                    // completes, so the first match is the one typed.
                    .label(name)
                    .child(move |_, _| {
                        v_flex()
                            .py_1()
                            .child(shown.0.clone())
                            .child(div().text_xs().text_color(muted).child(shown.1.clone()))
                    })
            }

            None => CommandItem::new()
                .label(format!("Not available: {unavailable}"))
                .disabled(true),
        });

        palette = palette.group(CommandGroup::new().label(label.clone()).items(items));
    }

    let sections = sections.clone();

    palette.on_confirm(move |index, window, cx| {
        let chosen = sections
            .get(index.section)
            .and_then(|section| section.get(index.row))
            .cloned()
            .flatten();

        window.close_dialog(cx);

        if let Some(command) = chosen {
            let _ = workspace.update(cx, |this, cx| this.chose(command, window, cx));
        }
    })
}

/// The name of a command as the word on the button that runs it.
fn capitalized(name: &str) -> String {
    let mut letters = name.chars();

    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::capitalized;

    #[test]
    fn names_the_button_after_the_command() {
        assert_eq!(capitalized("delete"), "Delete");
        assert_eq!(capitalized(""), "");
    }
}
