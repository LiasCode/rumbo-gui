//! The command palette: every function the host and the provider describe,
//! offered by name without knowing any of them, the host's by what they act
//! on, with a form for their parameters and a confirmation for what cannot
//! be undone.

use std::rc::Rc;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, WindowExt as _,
    button::{Button, ButtonVariant, ButtonVariants as _},
    command::{Command as Palette, CommandGroup, CommandItem, CommandState},
    dialog::{DialogAction, DialogClose, DialogFooter},
    v_flex,
};
use gpui_kit::{
    Context, Entity, ParentElement as _, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde_json::Value as Json;

use super::form::Field;
use super::{Feedback, Workspace, WorkspaceEvent};
use crate::commands::{self, Command};
use crate::conversation::Source;

/// One row of the palette.
#[derive(Clone)]
enum Entry {
    /// A function of the host or of the provider.
    Function(Command),
    /// The picker of the model and options, which belongs to this window.
    Picker,
    /// Commands of the window around the workspace.
    OpenProject,
    OpenSettings,
    CloseProject,
    /// Why the provider's functions are not known.
    Unavailable(String),
}

/// The palette's rows by the section and row the palette gives them.
type Sections = Rc<Vec<Vec<Entry>>>;

impl Workspace {
    /// Opens the commands: the window's own, the host's by what they act on,
    /// and the provider's.
    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ended.is_some() || window.has_active_dialog(cx) {
            return;
        }

        let mut labels = vec!["Window".to_string()];
        let mut sections = vec![vec![
            Entry::Picker,
            Entry::OpenProject,
            Entry::OpenSettings,
            Entry::CloseProject,
        ]];

        for (title, functions) in commands::by_group(&self.host_functions) {
            labels.push(title);
            sections.push(
                functions
                    .into_iter()
                    .map(|function| {
                        Entry::Function(Command {
                            source: Source::Host,
                            function,
                        })
                    })
                    .collect(),
            );
        }

        let provider: Vec<Entry> = match &self.provider_functions {
            Ok(functions) => functions
                .iter()
                .map(|function| {
                    Entry::Function(Command {
                        source: Source::Provider,
                        function: function.clone(),
                    })
                })
                .collect(),

            Err(problem) => vec![Entry::Unavailable(problem.clone())],
        };

        if !provider.is_empty() {
            labels.push(format!("Provider {}", self.provider));
            sections.push(provider);
        }

        let sections: Sections = Rc::new(sections);
        let labels = Rc::new(labels);
        let state = self.palette.clone();
        let workspace = cx.weak_entity();

        state.update(cx, |state, cx| state.set_query("", window, cx));

        window.open_dialog(cx, move |dialog, _, _| {
            let sections = sections.clone();
            let labels = labels.clone();
            let state = state.clone();
            let workspace = workspace.clone();

            dialog
                .close_button(false)
                .p_0()
                .content(move |content, _, cx| {
                    content.child(palette(&state, &sections, &labels, workspace.clone(), cx))
                })
        });

        // After the dialog opens, which takes the focus and remembers where
        // it was, to give it back on closing.
        self.palette.update(cx, |state, cx| state.focus(window, cx));
    }

    /// Goes on with the command the person chose: asks for its parameters,
    /// or its confirmation, or calls it.
    fn chose(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        let feedback = Feedback::Conversation;

        if !command.function.params.is_empty() {
            self.ask_parameters(command, Vec::new(), feedback, window, cx);
        } else {
            self.confirm_and_call(command, None, feedback, window, cx);
        }
    }

    /// Asks for the parameters of a command in a form, one field each, with
    /// the values a function lists for those that name one. `initial` is
    /// what the fields start with, in order.
    pub(super) fn ask_parameters(
        &mut self,
        command: Command,
        initial: Vec<String>,
        feedback: Feedback,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let fields = command
            .function
            .params
            .iter()
            .enumerate()
            .map(|(index, parameter)| Field {
                parameter: parameter.clone(),
                source: command.source,
                initial: initial.get(index).cloned().unwrap_or_default(),
                empty: parameter.optional.then(|| "Leave out".to_string()),
                heading: None,
            })
            .collect();

        let provider_functions = self.provider_functions.clone().unwrap_or_default();
        let form = self.open_form(fields, provider_functions, window, cx);
        let command = Rc::new(command);
        let workspace = cx.weak_entity();
        let focused = form.clone();

        window.open_dialog(cx, move |dialog, _, cx| {
            let accepted = {
                let form = form.clone();
                let command = command.clone();
                let workspace = workspace.clone();

                move |_: &_, window: &mut Window, cx: &mut gpui_kit::App| {
                    let typed = form.read(cx).values(cx);

                    match commands::arguments(&command.function, &typed) {
                        Ok(params) => {
                            let command = (*command).clone();

                            // The form closes first, so a confirmation can
                            // take its place.
                            window.close_dialog(cx);

                            let _ = workspace.update(cx, |this, cx| {
                                this.confirm_and_call(command, params, feedback, window, cx)
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
                        .child(form.clone()),
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
        focused.update(cx, |form, cx| form.focus(window, cx));
    }

    /// Asks the person to confirm a command that cannot be undone, with the
    /// words its description gives, and calls it once they agree.
    pub(super) fn confirm_and_call(
        &mut self,
        command: Command,
        params: Option<Json>,
        feedback: Feedback,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if command.function.confirm.is_empty() {
            self.call(&command, params, feedback, cx);
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
                .ok_text(commands::capitalized(&command.function.name))
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    let _ = workspace.update(cx, |this, cx| {
                        this.call(&command, params.clone(), feedback, cx)
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
    workspace: WeakEntity<Workspace>,
    cx: &gpui_kit::App,
) -> Palette {
    let muted = cx.theme().muted_foreground;
    let mut palette = Palette::new(state)
        .bordered(false)
        .placeholder("Type a command")
        .w(px(480.));

    for (label, section) in labels.iter().zip(sections.iter()) {
        let items = section.iter().map(|entry| match entry {
            Entry::Function(command) => item(
                command.function.name.clone(),
                command.function.description.clone(),
                muted,
            ),

            Entry::Picker => item(
                "model".into(),
                "Choose the model and the options this session keeps".into(),
                muted,
            ),

            Entry::OpenProject => item(
                "Open Project…".into(),
                "Work in another folder, beside the projects open here".into(),
                muted,
            ),

            Entry::OpenSettings => item(
                "Settings…".into(),
                "The theme, and rumbo's settings and providers".into(),
                muted,
            ),

            Entry::CloseProject => item(
                "Close Project".into(),
                "End the rumbo of this project; its sessions stay".into(),
                muted,
            ),

            Entry::Unavailable(problem) => CommandItem::new()
                .label(format!("Not available: {problem}"))
                .disabled(true),
        });

        palette = palette.group(CommandGroup::new().label(label.clone()).items(items));
    }

    let sections = sections.clone();

    palette.on_confirm(move |index, window, cx| {
        let chosen = sections
            .get(index.section)
            .and_then(|section| section.get(index.row))
            .cloned();

        window.close_dialog(cx);

        let _ = workspace.update(cx, |this, cx| match chosen {
            Some(Entry::Function(command)) => this.chose(command, window, cx),
            Some(Entry::Picker) => this.open_picker(cx),
            Some(Entry::OpenProject) => cx.emit(WorkspaceEvent::OpenProject),
            Some(Entry::OpenSettings) => cx.emit(WorkspaceEvent::OpenSettings),
            Some(Entry::CloseProject) => cx.emit(WorkspaceEvent::CloseProject),
            _ => {}
        });
    })
}

/// A row with its name, which is what the search matches, as rumbo's
/// terminal UI completes, so the first match is the one typed; and what it
/// does below.
fn item(name: String, description: String, muted: gpui_kit::Hsla) -> CommandItem {
    let shown = name.clone();

    CommandItem::new().label(name).child(move |_, _| {
        v_flex()
            .py_1()
            .child(shown.clone())
            .child(div().text_xs().text_color(muted).child(description.clone()))
    })
}
