//! The settings, shown in place of the work: the window's preferences,
//! which it keeps itself, and its keys; and rumbo's configuration, which is
//! asked of the host of the project shown. rumbo keeps it the same for every
//! project, so any host answers for all: what is set and what applies, with
//! `config`, and what each provider has, with `provider-show`.
//!
//! What the host says is shown as it says it, and every change goes through
//! the host's functions, with the form their descriptions make. rumbo checks
//! a change before it keeps it, and says why when it does not.

use std::collections::HashMap;
use std::path::PathBuf;

use gpui::{
    AnyElement, App, AppContext as _, Context, Div, ElementId, Entity, EventEmitter, FocusHandle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Subscription,
    WeakEntity, Window, div, prelude::FluentBuilder as _, px,
};
use serde_json::Value as Json;
use ui::{
    ActiveTheme as _, Button, Input, InputEvent, InputState, Settings, SettingsGroup, SettingsItem,
    SettingsPage, Sizable as _, Spinner, Switch, Toggle, ToggleGroup, h_flex, v_flex,
};

use super::SHORTCUTS;
use crate::preferences::{Preferences, Theme as Colors, Width};
use crate::protocol::{self, HostResult, Value};
use crate::workspace::Workspace;

pub(super) const KEY_CONTEXT: &str = "Settings";

pub(super) enum SettingsEvent {
    /// The person changed a preference of the window.
    Preferences(Preferences),
}

/// What the host answered for a part of the settings.
#[derive(Clone)]
enum Asked<T> {
    Asking,
    Here(T),
    Failed(String),
}

pub(super) struct SettingsView {
    /// The workspace whose host is asked; none with no project open.
    workspace: Option<WeakEntity<Workspace>>,
    preferences: Preferences,
    /// The command that opens a project in an editor, as it is typed.
    editor: Entity<InputState>,
    /// Where the preferences are kept, and why they could not be.
    path: Option<PathBuf>,
    problem: Option<String>,
    /// None for what the host does not offer.
    configuration: Option<Asked<String>>,
    providers: Option<Asked<Vec<Value>>>,
    /// What each provider shown says it offers.
    details: HashMap<String, Asked<String>>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl SettingsView {
    pub(super) fn new(
        workspace: Option<Entity<Workspace>>,
        preferences: Preferences,
        path: Option<PathBuf>,
        problem: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Found on the PATH: zed, cursor, code, …")
                .default_value(preferences.editor.clone())
        });

        // Kept when the person is done typing, not at every key.
        let editing = cx.subscribe(&editor, |view, editor, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::Submit) {
                let typed = editor.read(cx).value().trim().to_string();

                if typed != view.preferences.editor {
                    view.preferences.editor = typed;
                    cx.emit(SettingsEvent::Preferences(view.preferences.clone()));
                }
            }
        });

        let mut view = SettingsView {
            workspace: workspace.map(|workspace| workspace.downgrade()),
            preferences,
            editor,
            path,
            problem,
            configuration: None,
            providers: None,
            details: HashMap::new(),
            focus_handle: cx.focus_handle(),
            _subscriptions: vec![editing],
        };

        view.refresh(cx);
        view
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    /// Asks the host again what the settings show, after a change. What was
    /// shown stays until the new answer comes.
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        let configuration = self.ask(protocol::FUNCTION_CONFIG, None, cx, |view, answer| {
            view.configuration = Some(asked(answer.map(|result| result.text)));
        });
        let providers = self.ask(protocol::FUNCTION_PROVIDERS, None, cx, |view, answer| {
            view.providers = Some(asked(answer.map(|result| result.values)));
        });

        waiting(&mut self.configuration, configuration);
        waiting(&mut self.providers, providers);

        // What a provider has can change with what is set for it.
        let shown: Vec<String> = self.details.keys().cloned().collect();

        for provider in shown {
            self.ask_details(provider, cx);
        }
    }

    /// Asks the host's function `name`, and gives its answer to `take`.
    /// False when there is no host to ask, or it does not offer it.
    fn ask(
        &self,
        name: &str,
        params: Option<Json>,
        cx: &mut Context<Self>,
        take: impl FnOnce(&mut Self, Result<HostResult, String>) + 'static,
    ) -> bool {
        let Some(workspace) = self.workspace.as_ref().and_then(WeakEntity::upgrade) else {
            return false;
        };

        let view = cx.weak_entity();

        workspace.update(cx, |workspace, _| {
            workspace.ask_host(
                name,
                params,
                Box::new(move |answer, _, cx| {
                    let _ = view.update(cx, |view, cx| {
                        take(view, answer);
                        cx.notify();
                    });
                }),
            )
        })
    }

    /// Shows what a provider offers, starting it, or hides it.
    fn toggle_details(&mut self, provider: String, cx: &mut Context<Self>) {
        if self.details.remove(&provider).is_none() {
            self.ask_details(provider, cx);
        }

        cx.notify();
    }

    fn ask_details(&mut self, provider: String, cx: &mut Context<Self>) {
        let name = provider.clone();
        let params = serde_json::json!({ "provider": provider });

        let asking = self.ask(
            protocol::FUNCTION_PROVIDER_SHOW,
            Some(params),
            cx,
            move |view, answer| {
                // Hidden meanwhile, it stays hidden.
                if view.details.contains_key(&name) {
                    view.details
                        .insert(name, asked(answer.map(|result| result.text)));
                }
            },
        );

        if asking {
            self.details.entry(provider).or_insert(Asked::Asking);
        }
    }

    /// Whether the host of the project shown offers a function.
    fn offers(&self, name: &str, cx: &App) -> bool {
        self.workspace
            .as_ref()
            .and_then(WeakEntity::upgrade)
            .is_some_and(|workspace| workspace.read(cx).offers(name))
    }

    /// Changes a preference of the window, which the shell keeps.
    fn prefer(view: &WeakEntity<Self>, change: impl FnOnce(&mut Preferences), cx: &mut App) {
        let _ = view.update(cx, |view, cx| {
            change(&mut view.preferences);
            cx.emit(SettingsEvent::Preferences(view.preferences.clone()));
            cx.notify();
        });
    }

    fn appearance_page(&self, cx: &mut Context<Self>) -> SettingsPage {
        let view = cx.weak_entity();
        let theme = self.preferences.theme;
        let width = self.preferences.width;
        let kept = match &self.path {
            Some(path) => format!("The window keeps its preferences in {}.", path.display()),
            None => "Without $HOME, they are kept for this start only.".into(),
        };
        let problem = self.problem.clone();

        let theming = view.clone();
        let theme_item = SettingsItem::new(move |_, cx| {
            let view = theming.clone();
            let choices = ToggleGroup::new()
                .outline()
                .small()
                .single()
                .children(Colors::ALL.iter().map(|choice| {
                    Toggle::new(ElementId::Name(format!("theme-{}", choice.label()).into()))
                        .label(choice.label())
                        .checked(*choice == theme)
                }))
                .on_change(move |checks: &[bool], _, cx| {
                    if let Some(chosen) = chosen(&Colors::ALL, checks) {
                        Self::prefer(&view, |preferences| preferences.theme = chosen, cx)
                    }
                });

            h_flex()
                .gap_4()
                .justify_between()
                .child(described(
                    "Theme",
                    "System follows the desktop, and changes when it does.",
                    cx,
                ))
                .child(choices)
        })
        .keywords(["theme", "dark", "light", "appearance", "color"]);

        let widening = view.clone();
        let width_item = SettingsItem::new(move |_, cx| {
            let view = widening.clone();
            let choices = ToggleGroup::new()
                .outline()
                .small()
                .single()
                .children(Width::ALL.iter().map(|choice| {
                    Toggle::new(ElementId::Name(format!("width-{}", choice.label()).into()))
                        .label(choice.label())
                        .checked(*choice == width)
                }))
                .on_change(move |checks: &[bool], _, cx| {
                    if let Some(chosen) = chosen(&Width::ALL, checks) {
                        Self::prefer(&view, |preferences| preferences.width = chosen, cx)
                    }
                });

            h_flex()
                .gap_4()
                .justify_between()
                .child(described(
                    "Conversation width",
                    "How wide the conversation and the composer grow in a large window.",
                    cx,
                ))
                .child(choices)
        })
        .keywords(["width", "wide", "narrow", "chat", "conversation"]);

        let kept_item = SettingsItem::new(move |_, cx| {
            v_flex()
                .gap_1()
                .child(note(&kept, cx))
                .when_some(problem.clone(), |column, problem| {
                    column.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().colors.danger)
                            .child(problem),
                    )
                })
        });

        let quiet = self.preferences.quiet;
        let quieting = view.clone();
        let notify_item = SettingsItem::new(move |_, cx| {
            let view = quieting.clone();

            h_flex()
                .gap_4()
                .justify_between()
                .child(described(
                    "Say when a run ends elsewhere",
                    "A notification when a run ends in a session that is not shown. The sidebar marks the session either way.",
                    cx,
                ))
                .child(Switch::new("notify").checked(!quiet).on_change(
                    move |checked, _, cx| {
                        let quiet = !*checked;

                        Self::prefer(&view, |preferences| preferences.quiet = quiet, cx)
                    },
                ))
        })
        .keywords(["notification", "notify", "background", "quiet"]);

        let editor = self.editor.clone();
        let editor_item = SettingsItem::new(move |_, cx| {
            h_flex()
                .gap_4()
                .justify_between()
                .child(described(
                    "Editor",
                    "The command that opens a project's folder, such as zed or code --new-window. Empty uses the first editor found on the PATH.",
                    cx,
                ))
                .child(div().w(px(240.)).flex_shrink_0().child(Input::new(&editor).small()))
        })
        .keywords(["editor", "open", "zed", "code", "cursor"]);

        SettingsPage::new("Window")
            .group(
                SettingsGroup::new()
                    .title("Appearance")
                    .item(theme_item)
                    .item(width_item),
            )
            .group(
                SettingsGroup::new()
                    .title("Behavior")
                    .item(notify_item)
                    .item(editor_item),
            )
            .group(SettingsGroup::new().item(kept_item))
    }

    fn keys_page(&self) -> SettingsPage {
        let items = SHORTCUTS.iter().map(|(keys, does)| {
            SettingsItem::new(move |_, cx| {
                h_flex()
                    .gap_4()
                    .justify_between()
                    .child(div().text_sm().child(does.to_string()))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .font_family(cx.theme().mono_family.clone())
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .border_1()
                            .border_color(cx.theme().colors.border)
                            .child(keys.to_string()),
                    )
            })
            .keywords([keys.to_string(), does.to_string()])
        });

        SettingsPage::new("Keyboard").group(
            SettingsGroup::new()
                .title("Shortcuts")
                .description("Every key works wherever the keyboard is in the window, unless a field needs it.")
                .items(items),
        )
    }

    fn system_page(&self, cx: &mut Context<Self>) -> SettingsPage {
        let page = SettingsPage::new("System");

        if self.workspace.is_none() {
            return page.group(unreachable());
        }

        let shown = self.configuration.clone();
        let in_force = SettingsItem::new(move |_, cx| match &shown {
            Some(answer) => said(answer, cx),
            None => note("This rumbo does not show its configuration.", cx).into_any_element(),
        })
        .keywords(["settings", "configuration", "in force", "config"]);

        let page = page.group(
            SettingsGroup::new()
                .title("In Force")
                .description("What is set and what applies here, the session shown first, each value with where it comes from. What a program has by default is in its details, under Providers.")
                .item(in_force),
        );

        if !self.offers(protocol::FUNCTION_SYSTEM_SET, cx) {
            return page;
        }

        let workspace = self.workspace.clone();

        page.group(SettingsGroup::new().title("Settings").item(
            SettingsItem::new(move |_, cx| {
                h_flex()
                    .gap_4()
                    .justify_between()
                    .child(described(
                        "Change a system setting",
                        "Chosen from the list, or typed, as name=value; an empty value gives it back to its default. rumbo checks it before it keeps it.",
                        cx,
                    ))
                    .child(action(
                        "change-system",
                        "Change…",
                        &workspace,
                        protocol::FUNCTION_SYSTEM_SET,
                        ("change", String::new()),
                        true,
                    ))
            })
            .keywords(["default provider", "frontend", "max turns", "max tokens", "compact"]),
        ))
    }

    fn providers_page(&self, cx: &mut Context<Self>) -> SettingsPage {
        let page = SettingsPage::new("Providers");

        if self.workspace.is_none() {
            return page.group(unreachable());
        }

        let workspace = self.workspace.clone();
        let view = cx.weak_entity();

        let offered = [
            (protocol::FUNCTION_PROVIDER_SET, "Change…", true),
            (protocol::FUNCTION_PROVIDER_RM, "Remove…", false),
        ]
        .into_iter()
        .filter(|(name, _, _)| self.offers(name, cx))
        .collect::<Vec<_>>();
        let can_show = self.offers(protocol::FUNCTION_PROVIDER_SHOW, cx);

        let providers: Vec<SettingsItem> = match &self.providers {
            Some(Asked::Here(providers)) => providers
                .iter()
                .map(|provider| {
                    let name = provider.value.clone();
                    let command = provider.description.clone();
                    let details = self.details.get(&name).cloned();
                    let workspace = workspace.clone();
                    let offered = offered.clone();
                    let view = view.clone();

                    SettingsItem::new(move |_, cx| {
                        let toggling = view.clone();
                        let toggled = name.clone();

                        let details_button = Button::new(element_id("details", &name))
                            .ghost()
                            .small()
                            .label(if details.is_some() { "Hide" } else { "Details" })
                            .tooltip("Each of its settings, where its value comes from, and what it offers; it is started to say")
                            .on_click(move |_, _, cx| {
                                let _ = toggling.update(cx, |view, cx| {
                                    view.toggle_details(toggled.clone(), cx)
                                });
                            });

                        let actions = offered.iter().map(|(function, label, ask)| {
                            action(
                                element_id(function, &name),
                                label,
                                &workspace,
                                function,
                                ("provider", name.clone()),
                                *ask,
                            )
                        });

                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .gap_4()
                                    .justify_between()
                                    .child(described(&name, &command, cx))
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .flex_shrink_0()
                                            .when(can_show, |row| row.child(details_button))
                                            .children(actions),
                                    ),
                            )
                            .when_some(details.clone(), |column, details| {
                                column.child(said(&details, cx))
                            })
                    })
                    .keywords([provider.value.clone(), provider.description.clone()])
                })
                .collect(),

            Some(answer) => {
                let answer = unlisted(answer);

                vec![SettingsItem::new(move |_, cx| said(&answer, cx))]
            }

            None => vec![SettingsItem::new(|_, cx| {
                note("This rumbo does not list its providers.", cx)
            })],
        };

        let can_add = self.offers(protocol::FUNCTION_PROVIDER_ADD, cx);
        let adding = workspace.clone();

        let page = page.group(
            SettingsGroup::new()
                .title("Providers")
                .description("The programs that reach a model, those rumbo has built in and those added. Each session uses one; a new session starts with the default provider, a system setting, and the model set for it. Change sets its model, a field of its configuration, or one of its options, which it is started to check.")
                .items(providers),
        );

        if !can_add {
            return page;
        }

        page.group(SettingsGroup::new().item(SettingsItem::new(move |_, cx| {
            h_flex()
                .gap_4()
                .justify_between()
                .child(described(
                    "Add a provider",
                    "A name and the command that starts it, which is started to see that it works.",
                    cx,
                ))
                .child(action(
                    "add-provider",
                    "Add Provider…",
                    &adding,
                    protocol::FUNCTION_PROVIDER_ADD,
                    ("name", String::new()),
                    true,
                ))
        })))
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pages = [
            self.appearance_page(cx),
            self.system_page(cx),
            self.providers_page(cx),
            self.keys_page(),
        ];

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .child(Settings::new("settings").pages(pages))
    }
}

/// The choice a group of one checked gives, of `choices` in its order.
fn chosen<T: Copy>(choices: &[T], checks: &[bool]) -> Option<T> {
    checks
        .iter()
        .position(|checked| *checked)
        .and_then(|index| choices.get(index).copied())
}

/// An answer, once the host gave it.
fn asked<T>(answer: Result<T, String>) -> Asked<T> {
    match answer {
        Ok(value) => Asked::Here(value),
        Err(problem) => Asked::Failed(problem),
    }
}

/// A list that is not here yet, or could not be had, said as words are.
fn unlisted<T>(answer: &Asked<T>) -> Asked<String> {
    match answer {
        Asked::Failed(problem) => Asked::Failed(problem.clone()),
        _ => Asked::Asking,
    }
}

/// Records what is being asked: nothing when it could not be asked, and
/// what was shown before until the answer comes.
fn waiting<T>(shown: &mut Option<Asked<T>>, asking: bool) {
    if !asking {
        *shown = None;
    } else if shown.is_none() {
        *shown = Some(Asked::Asking);
    }
}

/// What the host said, as it said it, or that it is being asked.
fn said(answer: &Asked<String>, cx: &App) -> AnyElement {
    match answer {
        Asked::Asking => h_flex()
            .gap_2()
            .child(Spinner::new().xsmall())
            .child(note("Asking rumbo", cx))
            .into_any_element(),

        Asked::Here(text) => div()
            .p_3()
            .rounded_md()
            .bg(cx.theme().colors.muted)
            .font_family(cx.theme().mono_family.clone())
            .text_xs()
            .child(text.clone())
            .into_any_element(),

        Asked::Failed(problem) => div()
            .text_sm()
            .text_color(cx.theme().colors.danger)
            .child(problem.clone())
            .into_any_element(),
    }
}

/// A button that offers a function of the host with one parameter given:
/// with a form for the rest when `ask`, else with its confirmation.
fn action(
    id: impl Into<ElementId>,
    label: &str,
    workspace: &Option<WeakEntity<Workspace>>,
    function: &'static str,
    given: (&'static str, String),
    ask: bool,
) -> Button {
    let workspace = workspace.clone();

    Button::new(id)
        .outline()
        .small()
        .label(label.to_string())
        .on_click(move |_, window, cx| {
            let Some(workspace) = workspace.as_ref().and_then(WeakEntity::upgrade) else {
                return;
            };

            let given = [(given.0, given.1.clone())];

            workspace.update(cx, |workspace, cx| {
                workspace.offer(function, &given, ask, window, cx)
            });
        })
}

/// An id for a control of one row, such as a provider's.
fn element_id(control: &str, row: &str) -> ElementId {
    ElementId::Name(format!("{control}-{row}").into())
}

/// A title with what it is for below it.
fn described(title: &str, description: &str, cx: &App) -> impl IntoElement {
    v_flex()
        .min_w_0()
        .gap_0p5()
        .child(div().text_sm().child(title.to_string()))
        .when(!description.is_empty(), |column| {
            column.child(note(description, cx))
        })
}

fn note(text: &str, cx: &App) -> Div {
    div()
        .text_xs()
        .text_color(cx.theme().colors.muted_foreground)
        .child(text.to_string())
}

/// What a page of rumbo's says with no project open.
fn unreachable() -> SettingsGroup {
    SettingsGroup::new().item(SettingsItem::new(|_, cx| {
        note(
            "rumbo's settings are asked of the rumbo working in a project. Open a project to see and change them.",
            cx,
        )
    }))
}
