//! The projects beside the work, each with its sessions: those open in the
//! window, whose host lists their sessions and marks the one shown and those
//! working, and the others rumbo holds, which open in a host of their own
//! when the person chooses one of their sessions, or opens one from its
//! menu. Pressing a project only shows or hides its sessions. A search
//! narrows both to the sessions and projects whose names, or what the host
//! says of them, hold what was typed.

use gpui::{
    App, ClipboardItem, Context, ElementId, IntoElement, ParentElement as _, Styled as _, Window,
    div, px,
};
use ui::{
    ActiveTheme as _, Button, Icon, IconName, Input, MenuItem, Sidebar, SidebarGroup, SidebarItem,
    Sizable as _, Spinner, h_flex, v_flex,
};

use super::{Opening, Project, Shell, State, folder_name};
use crate::protocol::{self, Value};

/// The sidebar's width when the person has not chosen one, and the least
/// and most it can be dragged to.
pub(super) const DEFAULT_WIDTH: f32 = 272.;
pub(super) const MIN_WIDTH: f32 = 200.;
pub(super) const MAX_WIDTH: f32 = 520.;

/// Whether a session or a project matches what was typed in the search:
/// its name, its id, or what the host says of it holds every word, in any
/// case.
pub(super) fn matches(query: &str, listed: &Value) -> bool {
    let haystack =
        format!("{} {} {}", listed.label, listed.value, listed.description).to_lowercase();

    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

/// The name a session is shown by: its title, or its id until it has one.
pub(super) fn session_name(session: &Value) -> String {
    if session.label.is_empty() {
        session.value.clone()
    } else {
        session.label.clone()
    }
}

/// What the host says of a session or a project, first in its menu, where
/// a person looks for how it stands.
fn described(said: &str) -> Vec<MenuItem> {
    if said.is_empty() {
        return Vec::new();
    }

    vec![MenuItem::label(said.to_string()), MenuItem::separator()]
}

/// An id for a row of the sidebar, unique among them.
fn row_id(kind: &str, key: &str) -> ElementId {
    ElementId::Name(format!("{kind}-{key}").into())
}

impl Shell {
    pub(super) fn sidebar_width(&self) -> f32 {
        self.preferred
            .preferences
            .sidebar_width
            .map(|width| (width as f32).clamp(MIN_WIDTH, MAX_WIDTH))
            .unwrap_or(DEFAULT_WIDTH)
    }

    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let opens_projects = self.opens_projects();
        let query = self.search.read(cx).value().to_string();

        let open: Vec<SidebarItem> = self
            .projects
            .iter()
            .filter_map(|project| self.project_item(project, &query, cx))
            .collect();

        let recent = self.recent_items(&query, cx);

        let new_session = Button::new("sidebar-new-session")
            .outline()
            .small()
            .w_full()
            .icon(IconName::SquarePen)
            .label("New Session")
            .disabled(self.selected_workspace().is_none())
            .tooltip("Start the next task in a new session (Ctrl-N)")
            .on_click(cx.listener(|this, _, window, cx| {
                this.new_session_shown(&super::NewSession, window, cx)
            }));

        let header = v_flex().w_full().gap_2().child(new_session).child(
            Input::new(&self.search)
                .small()
                .cleanable(true)
                .prefix(Icon::new(IconName::Search).xsmall()),
        );

        let footer = h_flex()
            .w_full()
            .gap_1()
            .child(
                Button::new("open-project")
                    .ghost()
                    .small()
                    .flex_1()
                    .icon(IconName::FolderOpen)
                    .label("Open Project…")
                    .disabled(!opens_projects)
                    .tooltip(if opens_projects {
                        "Work in another folder (Ctrl-O)"
                    } else {
                        "This window belongs to the rumbo that started it, which works in one project"
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.choose_folder(window, cx))),
            )
            .child(
                Button::new("sidebar-settings")
                    .ghost()
                    .small()
                    .icon(IconName::Settings)
                    .tooltip("Settings (Ctrl-,)")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings(&super::OpenSettings, window, cx)
                    })),
            );

        // The divider beside it draws the line between it and the work.
        let mut sidebar = Sidebar::new("projects")
            .w_full()
            .border_r_0()
            .header(header)
            .footer(footer);

        if !open.is_empty() {
            sidebar = sidebar.child(SidebarGroup::new("Projects").children(open));
        }

        if !recent.is_empty() {
            sidebar = sidebar.child(SidebarGroup::new("Recent").children(recent));
        }

        sidebar
    }

    /// The projects rumbo holds that are not open here, most recently used
    /// first, or why they could not be listed.
    fn recent_items(&self, query: &str, cx: &mut Context<Self>) -> Vec<SidebarItem> {
        let Some(catalog) = &self.catalog else {
            return Vec::new();
        };

        let catalog = catalog.read(cx);

        if let Some(problem) = catalog.problem() {
            let first = problem.lines().next().unwrap_or_default();

            return vec![
                SidebarItem::new("recent-problem", format!("Not listed: {first}")).disabled(true),
            ];
        }

        let open: Vec<String> = self
            .projects
            .iter()
            .filter_map(|project| self.root_of(project, cx))
            .collect();

        catalog
            .projects()
            .iter()
            .filter(|listed| !open.contains(&listed.value))
            .filter_map(|listed| {
                self.recent_item(listed, catalog.sessions(&listed.value), query, cx)
            })
            .collect()
    }

    /// A project rumbo holds, with its sessions, which opens with the one
    /// chosen, or with its latest. None when the search leaves out the
    /// project and every session of it.
    fn recent_item(
        &self,
        listed: &Value,
        sessions: &[Value],
        query: &str,
        cx: &Context<Self>,
    ) -> Option<SidebarItem> {
        let root = listed.value.clone();
        let shell = cx.weak_entity();
        let label = if listed.label.is_empty() {
            root.clone()
        } else {
            listed.label.clone()
        };

        let project_matches = matches(query, listed);

        let rows: Vec<SidebarItem> = sessions
            .iter()
            .filter(|session| project_matches || matches(query, session))
            .map(|session| {
                let shell = shell.clone();
                let root = root.clone();
                let id = session.value.clone();
                let copied = id.clone();
                let said = session.description.clone();

                SidebarItem::new(row_id("recent-session", &id), session_name(session))
                    .on_click(move |window, cx| {
                        let opening = Opening::Session(id.clone());

                        let _ = shell.update(cx, |this, cx| {
                            this.open_listed(root.clone(), opening, window, cx)
                        });
                    })
                    .context_menu(move |_, _| {
                        let copied = copied.clone();
                        let mut items = described(&said);

                        items.push(
                            MenuItem::new("Copy Session ID")
                                .icon(IconName::Copy)
                                .on_click(move |_, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
                                }),
                        );
                        items
                    })
            })
            .collect();

        if !project_matches && rows.is_empty() {
            return None;
        }

        let opened = |opening: Opening| {
            let shell = shell.clone();
            let root = root.clone();

            move |window: &mut Window, cx: &mut App| {
                let opening = opening.clone();

                let _ = shell.update(cx, |this, cx| {
                    this.open_listed(root.clone(), opening, window, cx)
                });
            }
        };

        let starting = opened(Opening::New);
        let menu_opening = opened(Opening::Latest);
        let menu_starting = opened(Opening::New);
        let said = listed.description.clone();

        // Pressed, it opens in its latest session only when the search
        // leaves none of its sessions to show or hide.
        let item = SidebarItem::new(row_id("recent", &root), label)
            .icon(IconName::FolderClosed)
            .default_open(!query.is_empty())
            .on_click(opened(Opening::Latest))
            .suffix(
                Button::new(row_id("start", &root))
                    .ghost()
                    .xsmall()
                    .icon(IconName::SquarePen)
                    .tooltip("New session")
                    .on_click(move |_, window, cx| starting(window, cx)),
            )
            .context_menu(move |_, _| {
                let opening = menu_opening.clone();
                let starting = menu_starting.clone();
                let mut items = described(&said);

                items.push(
                    MenuItem::new("Open")
                        .icon(IconName::FolderOpen)
                        .on_click(move |window, cx| opening(window, cx)),
                );
                items.push(
                    MenuItem::new("New Session")
                        .icon(IconName::SquarePen)
                        .on_click(move |window, cx| starting(window, cx)),
                );
                items
            })
            .children(rows);

        Some(item)
    }

    /// A project, with its sessions under it once its host lists them. None
    /// when the search leaves out the project and every session of it.
    fn project_item(
        &self,
        project: &Project,
        query: &str,
        cx: &mut Context<Self>,
    ) -> Option<SidebarItem> {
        let id = project.id;
        let shown = self.selected == Some(id) && self.settings.is_none();
        let shell = cx.weak_entity();
        let name = folder_name(&project.dir);
        let project_matches = matches(
            query,
            &Value {
                value: project.dir.display().to_string(),
                label: name.clone(),
                description: String::new(),
            },
        );

        // Pressed, it shows the project only while it lists no session, as
        // while its host starts; otherwise it shows or hides them.
        let selecting = shell.clone();
        let mut item = SidebarItem::new(("project", id), name)
            .icon(IconName::FolderOpen)
            .default_open(true)
            .on_click(move |window, cx| {
                let _ = selecting.update(cx, |this, cx| this.select(id, window, cx));
            })
            .context_menu({
                let shell = shell.clone();

                move |_, _| {
                    let starting = shell.clone();
                    let editing = shell.clone();
                    let closing = shell.clone();

                    vec![
                        MenuItem::new("New Session")
                            .icon(IconName::SquarePen)
                            .on_click(move |window, cx| {
                                let _ = starting
                                    .update(cx, |this, cx| this.new_session(id, window, cx));
                            }),
                        MenuItem::new("Open in Editor")
                            .icon(IconName::ExternalLink)
                            .on_click(move |_, cx| {
                                let _ = editing.update(cx, |this, cx| this.open_in_editor(id, cx));
                            }),
                        MenuItem::separator(),
                        MenuItem::new("Close Project")
                            .icon(IconName::Close)
                            .on_click(move |window, cx| {
                                let _ = closing
                                    .update(cx, |this, cx| this.close_project(id, window, cx));
                            }),
                    ]
                }
            });

        let workspace = match &project.state {
            State::Working { workspace, .. } => workspace.read(cx),

            State::Starting => {
                return project_matches.then(|| item.suffix(Spinner::new().xsmall()));
            }

            State::Failed(_) => {
                let danger = cx.theme().colors.danger;

                return project_matches.then(|| {
                    item.active(shown)
                        .suffix(Icon::new(IconName::CircleAlert).xsmall().color(danger))
                });
            }
        };

        if workspace.offers(protocol::FUNCTION_NEW) && !workspace.has_ended() {
            let shell = shell.clone();

            item = item.suffix(
                Button::new(("new-session", id))
                    .ghost()
                    .xsmall()
                    .icon(IconName::SquarePen)
                    .tooltip("New session")
                    .on_click(move |_, window, cx| {
                        let _ = shell.update(cx, |this, cx| this.new_session(id, window, cx));
                    }),
            );
        }

        let current = workspace.current().to_string();
        let can_open = workspace.offers(protocol::FUNCTION_OPEN);
        let can_rename = workspace.offers(protocol::FUNCTION_RENAME);
        let can_delete = workspace.offers(protocol::FUNCTION_DELETE);
        let mark = cx.theme().colors.primary;
        let mut sessions = Vec::new();

        // A new session is shown before it has an id, and is listed once a
        // run makes it.
        if current.is_empty() && shown && query.is_empty() {
            sessions.push(
                SidebarItem::new(("new-session-row", id), "New session")
                    .icon(IconName::SquarePen)
                    .active(true),
            );
        }

        for session in workspace
            .sessions()
            .iter()
            .filter(|session| project_matches || matches(query, session))
        {
            let session_id = session.value.clone();
            let mut row = SidebarItem::new(
                row_id(&format!("session-{id}"), &session_id),
                session_name(session),
            )
            .active(shown && session_id == current);

            if can_open {
                let opened = session_id.clone();
                let shell = shell.clone();

                row = row.on_click(move |window, cx| {
                    let _ = shell.update(cx, |this, cx| {
                        this.open_session(id, opened.clone(), window, cx)
                    });
                });
            }

            // Only this window's runs work in this host. A session whose run
            // ended while another was shown is marked until it is opened.
            if workspace.is_working(&session_id) {
                row = row.suffix(Spinner::new().xsmall());
            } else if workspace.has_unseen(&session_id) {
                row = row.suffix(div().size(px(7.)).rounded_full().bg(mark));
            }

            let said = session.description.clone();
            let shell = shell.clone();

            row = row.context_menu(move |_, _| {
                let opening = shell.clone();
                let renaming = shell.clone();
                let deleting = shell.clone();
                let target = session_id.clone();
                let renamed = session_id.clone();
                let deleted = session_id.clone();
                let copied = session_id.clone();

                let mut items = described(&said);

                if can_open {
                    items.push(
                        MenuItem::new("Open")
                            .icon(IconName::MessageSquare)
                            .on_click(move |window, cx| {
                                let _ = opening.update(cx, |this, cx| {
                                    this.open_session(id, target.clone(), window, cx)
                                });
                            }),
                    );
                }

                if can_rename && can_open {
                    items.push(MenuItem::new("Rename…").icon(IconName::Pencil).on_click(
                        move |window, cx| {
                            let _ = renaming.update(cx, |this, cx| {
                                this.rename_session(id, renamed.clone(), window, cx)
                            });
                        },
                    ));
                }

                items.push(
                    MenuItem::new("Copy Session ID")
                        .icon(IconName::Copy)
                        .on_click(move |_, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
                        }),
                );

                if can_delete {
                    items.push(MenuItem::separator());
                    items.push(MenuItem::new("Delete…").icon(IconName::Trash).on_click(
                        move |window, cx| {
                            let _ = deleting.update(cx, |this, cx| {
                                this.delete_session(id, deleted.clone(), window, cx)
                            });
                        },
                    ));
                }

                items
            });

            sessions.push(row);
        }

        if !project_matches && sessions.is_empty() {
            return None;
        }

        // The project's row stands for its session while it has none
        // listed yet.
        Some(item.active(shown && sessions.is_empty()).children(sessions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(label: &str, description: &str) -> Value {
        Value {
            value: "4cf0261c".into(),
            label: label.into(),
            description: description.into(),
        }
    }

    #[test]
    fn matches_every_word_typed_in_the_name_or_what_the_host_says() {
        let session = listed("Add a --json flag", "done; 3 turns");

        assert!(matches("", &session));
        assert!(matches("JSON", &session));
        assert!(matches("json done", &session));
        assert!(matches("4cf0", &session));
        assert!(!matches("json failed", &session));
    }

    #[test]
    fn names_a_session_by_its_title_or_else_its_id() {
        assert_eq!(session_name(&listed("Fix the test", "")), "Fix the test");
        assert_eq!(session_name(&listed("", "")), "4cf0261c");
    }
}
