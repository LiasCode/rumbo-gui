//! The command palette: the window's own actions, the project's sessions to
//! show one, and every function the host and the provider describe, offered
//! by name without knowing any of them, the host's by what they act on,
//! with a form for their parameters and a confirmation for what cannot be
//! undone.

use std::rc::Rc;

use gpui::{
    App, Context, Focusable as _, ParentElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};
use serde_json::Value as Json;
use ui::{
    ActiveTheme as _, CommandGroup, CommandIndex, CommandItem, CommandPalette, Toast, rems_from_px,
    v_flex,
};

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
    /// Actions of the workspace.
    NewSession,
    Changes,
    Files,
    Details,
    Export,
    Editor,
    /// A session of the project, by its id and how it is shown.
    Session(String, String, String),
    /// Commands of the window around the workspace.
    OpenProject,
    OpenSettings,
    Rumbos,
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
        if self.ended.is_some() || self.dialogs.read(cx).is_open() {
            return;
        }

        let mut labels = vec!["Window".to_string()];
        let mut sections = vec![vec![
            Entry::NewSession,
            Entry::Picker,
            Entry::Changes,
            Entry::Files,
            Entry::Details,
            Entry::Export,
            Entry::Editor,
            Entry::OpenProject,
            Entry::OpenSettings,
            Entry::Rumbos,
            Entry::CloseProject,
        ]];

        let sessions: Vec<Entry> = self
            .listed
            .iter()
            .filter(|session| session.value != self.current)
            .map(|session| {
                let name = match session.label.is_empty() {
                    true => session.value.clone(),
                    false => session.label.clone(),
                };

                Entry::Session(session.value.clone(), name, session.description.clone())
            })
            .collect();

        if !sessions.is_empty() {
            labels.push("Sessions".to_string());
            sections.push(sessions);
        }

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

        let provider: Vec<Entry> = match &self.provider_functions() {
            Ok(functions) => commands::offered(functions)
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
            labels.push(format!("Provider {}", self.provider()));
            sections.push(provider);
        }

        // Two extensions can each have a function called the same, so each
        // has a section of its own.
        for extension in self.extensions() {
            let functions: Vec<Entry> = commands::offered(&extension.functions)
                .map(|function| {
                    Entry::Function(Command {
                        source: Source::Extension(extension.name.clone()),
                        function: function.clone(),
                    })
                })
                .collect();

            if !functions.is_empty() {
                labels.push(format!("Extension {}", extension.name));
                sections.push(functions);
            }
        }

        let groups = labels
            .iter()
            .zip(&sections)
            .map(|(label, section)| {
                CommandGroup::new(label.clone()).items(section.iter().map(item))
            })
            .collect();
        let sections: Sections = Rc::new(sections);
        let state = self.palette.clone();
        let workspace = cx.weak_entity();
        let dialogs = self.dialogs.clone();

        state.update(cx, |state, cx| state.set_groups(groups, window, cx));

        self.dialogs.update(cx, |opened, cx| {
            opened.open(
                move |dialog, _, cx| {
                    let sections = sections.clone();
                    let workspace = workspace.clone();
                    let dialogs = dialogs.clone();

                    dialog
                        .title("Commands")
                        .width(rems_from_px(480.))
                        .initial_focus(state.focus_handle(cx))
                        .child(
                            CommandPalette::new(&state).on_confirm(move |index, window, cx| {
                                dialogs.update(cx, |dialogs, cx| dialogs.close(window, cx));
                                chose_entry(&workspace, &sections, index, window, cx);
                            }),
                        )
                },
                window,
                cx,
            )
        });
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
                source: command.source.clone(),
                initial: initial.get(index).cloned().unwrap_or_default(),
                empty: parameter.optional.then(|| "Leave out".to_string()),
                heading: None,
            })
            .collect();

        let other_functions = self.functions_of(&command.source);
        let form = self.open_form(fields, other_functions, window, cx);
        let command = Rc::new(command);
        let workspace = cx.weak_entity();
        let dialogs = self.dialogs.clone();
        let toasts = self.toasts.clone();
        let focused = form.clone();

        self.dialogs.update(cx, |opened, cx| {
            opened.open(
                move |dialog, _, cx| {
                    let accepted = {
                        let form = form.clone();
                        let command = command.clone();
                        let workspace = workspace.clone();
                        let dialogs = dialogs.clone();
                        let toasts = toasts.clone();

                        move |window: &mut Window, cx: &mut App| {
                            let typed = form.read(cx).values(cx);

                            match commands::arguments(&command.function, &typed) {
                                Ok(params) => {
                                    let command = (*command).clone();

                                    // The form closes first, so a confirmation
                                    // can take its place.
                                    dialogs.update(cx, |dialogs, cx| dialogs.close(window, cx));

                                    let _ = workspace.update(cx, |this, cx| {
                                        this.confirm_and_call(command, params, feedback, window, cx)
                                    });

                                    false
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

                    let dialog = match form.read(cx).first_focus(cx) {
                        Some(first) => dialog.initial_focus(first),
                        None => dialog,
                    };

                    dialog
                        .title(command.function.name.clone())
                        .width(rems_from_px(440.))
                        .child(
                            v_flex()
                                .gap_3()
                                .when(!command.function.description.is_empty(), |column| {
                                    column.child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().colors.muted_foreground)
                                            .child(command.function.description.clone()),
                                    )
                                })
                                .child(form.clone()),
                        )
                        .on_ok("Run", accepted)
                },
                window,
                cx,
            )
        });

        // A field that waits for its list takes the keyboard once it is
        // there.
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

        self.dialogs.update(cx, |dialogs, cx| {
            dialogs.open(
                move |dialog, _, _| {
                    let command = command.clone();
                    let params = params.clone();
                    let workspace = workspace.clone();

                    dialog
                        .title(command.function.name.clone())
                        .child(command.function.confirm.clone())
                        .danger()
                        .on_ok(
                            commands::capitalized(&command.function.name),
                            move |_, cx| {
                                let _ = workspace.update(cx, |this, cx| {
                                    this.call(&command, params.clone(), feedback, cx)
                                });
                                true
                            },
                        )
                },
                window,
                cx,
            )
        });
    }
}

/// Goes on with the row of the palette chosen.
fn chose_entry(
    workspace: &gpui::WeakEntity<Workspace>,
    sections: &Sections,
    index: CommandIndex,
    window: &mut Window,
    cx: &mut App,
) {
    let chosen = sections
        .get(index.group)
        .and_then(|section| section.get(index.item))
        .cloned();

    let _ = workspace.update(cx, |this, cx| match chosen {
        Some(Entry::Function(command)) => this.chose(command, window, cx),
        Some(Entry::Picker) => this.open_picker(cx),
        Some(Entry::NewSession) => this.new_session(window, cx),
        Some(Entry::Changes) => this.toggle_panel(super::Panel::Changes, window, cx),
        Some(Entry::Files) => this.toggle_panel(super::Panel::Files, window, cx),
        Some(Entry::Details) => this.toggle_panel(super::Panel::Details, window, cx),
        Some(Entry::Export) => this.export(window, cx),
        Some(Entry::Editor) => cx.emit(WorkspaceEvent::OpenInEditor {
            path: this.dir.clone(),
        }),
        Some(Entry::Session(id, _, _)) => this.open_session(&id, window, cx),
        Some(Entry::OpenProject) => cx.emit(WorkspaceEvent::OpenProject),
        Some(Entry::OpenSettings) => cx.emit(WorkspaceEvent::OpenSettings),
        Some(Entry::Rumbos) => cx.emit(WorkspaceEvent::OpenRumbos),
        Some(Entry::CloseProject) => cx.emit(WorkspaceEvent::CloseProject),
        _ => {}
    });
}

/// A row with its name, which is what the search matches, as rumbo's
/// terminal UI completes, so the first match is the one typed; and what it
/// does below.
fn item(entry: &Entry) -> CommandItem {
    let (name, description): (String, String) = match entry {
        Entry::Function(command) => (command.name(), command.function.description.clone()),

        Entry::Picker => (
            "model".into(),
            "Choose the model and the options this session keeps".into(),
        ),

        Entry::NewSession => (
            "New Session".into(),
            "Start the next task in a new session (Ctrl-N)".into(),
        ),

        Entry::Changes => (
            "Changes".into(),
            "What changed in the project's folder, as Git sees it (Ctrl-Shift-G)".into(),
        ),

        Entry::Files => (
            "Files".into(),
            "Explore the project files (Ctrl-Alt-E)".into(),
        ),

        Entry::Details => (
            "Details".into(),
            "How the session stands, and its skills (Ctrl-Alt-B)".into(),
        ),

        Entry::Export => (
            "Export as Markdown…".into(),
            "Write this session's conversation to a file".into(),
        ),

        Entry::Editor => (
            "Open in Editor".into(),
            "Open the project's folder in your editor (Ctrl-Shift-E)".into(),
        ),

        Entry::Session(_, name, said) => (name.clone(), said.clone()),

        Entry::OpenProject => (
            "Open Project…".into(),
            "Work in another folder, beside the projects open here".into(),
        ),

        Entry::OpenSettings => (
            "Settings…".into(),
            "The theme, and rumbo's settings and providers".into(),
        ),

        Entry::Rumbos => (
            "Rumbos…".into(),
            "What the rumbo of each session open uses: processes, memory, and processor time"
                .into(),
        ),

        Entry::CloseProject => (
            "Close Project".into(),
            "End the rumbo of this project; its sessions stay".into(),
        ),

        Entry::Unavailable(problem) => {
            return CommandItem::new(format!("Not available: {problem}")).disabled(true);
        }
    };

    CommandItem::new(name).description(description)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Function;

    #[test]
    fn an_extension_command_can_be_found_by_its_extension_name() {
        let command = Command {
            source: Source::Extension("linear".into()),
            function: Function {
                name: "logout".into(),
                ..Function::default()
            },
        };

        assert_eq!(
            item(&Entry::Function(command.clone())).name.as_ref(),
            "linear-logout"
        );
        assert_eq!(
            commands::invoke(&command.source, &command.function.name, None).name,
            "logout"
        );
    }
}
