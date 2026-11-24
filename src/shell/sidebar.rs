//! Every project beside the work, in one list, each with its sessions:
//! those open in the window, whose host lists their sessions and marks the
//! one shown and those working, and the others rumbo holds, which open in a
//! host of their own when the person chooses one of their sessions, or
//! opens one from its menu. The projects the person pinned come first, in
//! the order they were pinned, then the rest, most recently used first. A
//! project's sessions keep the order they were first listed in, the newest
//! first, so using one moves none.
//! Pressing a project only shows or hides its sessions. A search, at the
//! top, narrows the list to the sessions and projects whose names, or what
//! the host says of them, hold what was typed; under it, New Session starts
//! one in the person's home folder, and the folder button opens a project.

use gpui::{
    AnyElement, App, ClipboardItem, Context, ElementId, IntoElement, ParentElement as _,
    Styled as _, WeakEntity, Window, div,
};
use ui::{
    ActiveTheme as _, Button, Icon, IconName, Input, MenuItem, Sidebar, SidebarGroup, SidebarItem,
    Sizable as _, Spinner, h_flex, rems_from_px, v_flex,
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

/// An id for a project's row. While a search narrows the list, the row is
/// another one, open from the start so what was found shows, and the row
/// as it was comes back once the search is cleared.
fn project_row_id(key: &str, query: &str) -> ElementId {
    match query.is_empty() {
        true => row_id("project", key),
        false => row_id("project-found", key),
    }
}

/// Projects by their folder, the pinned first, in the order they were
/// pinned, then the others in the order they came.
fn pinned_first<T>(projects: Vec<(String, T)>, pinned: &[String]) -> Vec<(String, T)> {
    let (mut first, rest): (Vec<_>, Vec<_>) = projects
        .into_iter()
        .partition(|(root, _)| pinned.contains(root));

    first.sort_by_key(|(root, _)| pinned.iter().position(|kept| kept == root));
    first.extend(rest);
    first
}

/// A project's folder, which pins it, and whether it is pinned.
#[derive(Clone)]
struct Pin {
    root: String,
    pinned: bool,
}

impl Pin {
    /// The item of a project's menu that pins it to the top, or takes its
    /// pin away.
    fn menu_item(&self, shell: WeakEntity<Shell>) -> MenuItem {
        let (root, pinned) = (self.root.clone(), self.pinned);
        let (label, icon) = match pinned {
            true => ("Unpin", IconName::PinOff),
            false => ("Pin to Top", IconName::Pin),
        };

        MenuItem::new(label).icon(icon).on_click(move |_, cx| {
            let _ = shell.update(cx, |this, cx| this.set_pinned(&root, !pinned, cx));
        })
    }

    /// The mark of a pinned project, which takes its pin away when pressed.
    fn mark(&self, shell: WeakEntity<Shell>) -> Option<Button> {
        let root = self.root.clone();

        self.pinned.then(|| {
            Button::new(row_id("unpin", &root))
                .ghost()
                .xsmall()
                .icon(IconName::Pin)
                .tooltip("Pinned to the top; press to unpin")
                .on_click(move |_, _, cx| {
                    let _ = shell.update(cx, |this, cx| this.set_pinned(&root, false, cx));
                })
        })
    }
}

/// What stands at the end of a project's row: how its host stands, and its
/// pin; nothing when there is neither.
fn project_suffix(status: Option<AnyElement>, pin: Option<Button>) -> Option<AnyElement> {
    if status.is_none() && pin.is_none() {
        return None;
    }

    Some(
        h_flex()
            .gap_1()
            .children(status)
            .children(pin)
            .into_any_element(),
    )
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
        let projects = self.project_rows(&query, cx);

        let new_session = Button::new("sidebar-new-session")
            .outline()
            .small()
            .flex_1()
            .icon(IconName::SquarePen)
            .label("New Session")
            .disabled(!opens_projects)
            .tooltip(if opens_projects {
                "Start a new session in your home folder"
            } else {
                "This window belongs to the rumbo that started it, which works in one project"
            })
            .on_click(cx.listener(|this, _, window, cx| this.new_session_in_home(window, cx)));

        let open_project = Button::new("open-project")
            .outline()
            .small()
            .icon(IconName::FolderOpen)
            .disabled(!opens_projects)
            .tooltip(if opens_projects {
                "Open Project… (Ctrl-O)"
            } else {
                "This window belongs to the rumbo that started it, which works in one project"
            })
            .on_click(cx.listener(|this, _, window, cx| this.choose_folder(window, cx)));

        let header = v_flex()
            .w_full()
            .gap_2()
            .child(
                Input::new(&self.search)
                    .small()
                    .cleanable(true)
                    .prefix(Icon::new(IconName::Search).xsmall()),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(new_session)
                    .child(open_project),
            );

        // The divider beside it draws the line between it and the work.
        let mut sidebar = Sidebar::new("projects")
            .w_full()
            .border_r_0()
            .header(header);

        if !projects.is_empty() {
            sidebar = sidebar.child(SidebarGroup::new("Projects").children(projects));
        }

        sidebar
    }

    /// Every project, open in the window or held by rumbo: the pinned first,
    /// then those just opened, which rumbo lists once they have a session,
    /// then the rest as rumbo lists them, most recently used first. Last,
    /// why rumbo's could not be listed, when they could not.
    fn project_rows(&self, query: &str, cx: &Context<Self>) -> Vec<SidebarItem> {
        enum Row<'a> {
            Open(&'a Project),
            Held(&'a Value),
        }

        let catalog = self.catalog.as_ref().map(|catalog| catalog.read(cx));
        let opened: Vec<(&Project, Option<String>)> = self
            .projects
            .iter()
            .map(|project| (project, self.root_of(project, cx)))
            .collect();

        let mut rows: Vec<(String, Row)> = opened
            .iter()
            .rev()
            .filter(|(_, root)| root.is_none())
            .map(|(project, _)| (project.dir.display().to_string(), Row::Open(project)))
            .collect();

        let problem = catalog.and_then(|catalog| catalog.problem());

        for listed in catalog
            .map(|catalog| catalog.projects())
            .unwrap_or_default()
        {
            let open = opened
                .iter()
                .find(|(_, root)| root.as_deref() == Some(listed.value.as_str()));

            rows.push((
                listed.value.clone(),
                match open {
                    Some((project, _)) => Row::Open(project),
                    None => Row::Held(listed),
                },
            ));
        }

        let pinned = &self.preferred.preferences.pinned;
        let mut items: Vec<SidebarItem> = pinned_first(rows, pinned)
            .into_iter()
            .filter_map(|(root, row)| {
                let pin = Pin {
                    pinned: pinned.contains(&root),
                    root,
                };

                match row {
                    Row::Open(project) => self.project_item(project, query, &pin, cx),
                    Row::Held(listed) => {
                        let sessions = catalog
                            .map(|catalog| catalog.sessions(&listed.value))
                            .unwrap_or_default();

                        self.held_item(listed, sessions, query, &pin, cx)
                    }
                }
            })
            .collect();

        if let Some(problem) = problem {
            let first = problem.lines().next().unwrap_or_default();

            items.push(
                SidebarItem::new("projects-problem", format!("Not listed: {first}")).disabled(true),
            );
        }

        items
    }

    /// A project rumbo holds that is not open here, with its sessions, which
    /// opens with the one chosen, or with its latest. None when the search
    /// leaves out the project and every session of it.
    fn held_item(
        &self,
        listed: &Value,
        sessions: &[Value],
        query: &str,
        pin: &Pin,
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

        let menu_opening = opened(Opening::Latest);
        let menu_starting = opened(Opening::New);
        let said = listed.description.clone();
        let menu_pin = pin.clone();
        let menu_shell = shell.clone();

        // Pressed, it opens in its latest session only when the search
        // leaves none of its sessions to show or hide. A new session in it
        // starts from its menu.
        let mut item = SidebarItem::new(project_row_id(&root, query), label)
            .icon(IconName::FolderClosed)
            .default_open(!query.is_empty())
            .on_click(opened(Opening::Latest))
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
                items.push(menu_pin.menu_item(menu_shell.clone()));
                items
            })
            .children(rows);

        if let Some(suffix) = project_suffix(None, pin.mark(shell)) {
            item = item.suffix(suffix);
        }

        Some(item)
    }

    /// A project, with its sessions under it once its host lists them. None
    /// when the search leaves out the project and every session of it.
    fn project_item(
        &self,
        project: &Project,
        query: &str,
        pin: &Pin,
        cx: &Context<Self>,
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
        let menu_pin = pin.clone();
        let item = SidebarItem::new(project_row_id(&id.to_string(), query), name)
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
                        menu_pin.menu_item(shell.clone()),
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

        let status = match &project.state {
            State::Working { .. } => None,
            State::Starting { .. } => Some(Spinner::new().xsmall().into_any_element()),
            State::Failed(_) => Some(
                Icon::new(IconName::CircleAlert)
                    .xsmall()
                    .color(cx.theme().colors.danger)
                    .into_any_element(),
            ),
        };

        let item = match project_suffix(status, pin.mark(shell.clone())) {
            Some(suffix) => item.suffix(suffix),
            None => item,
        };

        let workspace = match &project.state {
            State::Working { workspace, .. } => workspace.read(cx),

            // The list stays as it was while the host starts.
            State::Starting { .. } => {
                let sessions = self.starting_rows(project, query, project_matches, shown, cx);

                if !project_matches && sessions.is_empty() {
                    return None;
                }

                return Some(item.active(shown && sessions.is_empty()).children(sessions));
            }

            State::Failed(_) => return project_matches.then(|| item.active(shown)),
        };

        let current = workspace.current().to_string();
        let blank = workspace.is_blank();
        // The workspace opens a session in a host of its own, and deletes
        // one with rumbo's command line; renaming is the host's to offer.
        let can_open = true;
        let can_rename = workspace.offers(protocol::FUNCTION_RENAME);
        let can_delete = true;
        let mark = cx.theme().colors.primary;
        let mut sessions = Vec::new();

        // A new session is shown before it has an id, and is listed once a
        // run makes it. While no session is shown, none is marked.
        if current.is_empty() && shown && !blank && query.is_empty() {
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
                row = row.suffix(div().size(rems_from_px(7.)).rounded_full().bg(mark));
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
        Some(
            item.active(shown && !blank && sessions.is_empty())
                .children(sessions),
        )
    }

    /// The sessions of a project whose host starts, as rumbo lists them,
    /// with the one the window asked for marked, and the rows the host's
    /// list will have, so nothing in the list moves once it has spoken.
    /// They open nothing until then.
    fn starting_rows(
        &self,
        project: &Project,
        query: &str,
        project_matches: bool,
        shown: bool,
        cx: &Context<Self>,
    ) -> Vec<SidebarItem> {
        let id = project.id;
        let listed = self.held_sessions(project, cx);
        let opened = match &project.opening {
            Some(Opening::Session(session)) => session.as_str(),
            _ => "",
        };
        let mut rows = Vec::new();

        if shown && query.is_empty() && matches!(project.opening, Some(Opening::New)) {
            rows.push(
                SidebarItem::new(("new-session-row", id), "New session")
                    .icon(IconName::SquarePen)
                    .active(true),
            );
        }

        rows.extend(
            listed
                .iter()
                .filter(|session| project_matches || matches(query, session))
                .map(|session| {
                    SidebarItem::new(
                        row_id(&format!("session-{id}"), &session.value),
                        session_name(session),
                    )
                    .active(shown && session.value == opened)
                }),
        );

        rows
    }

    /// The sessions rumbo lists for a project, which show until its host
    /// lists them.
    fn held_sessions<'a>(&'a self, project: &Project, cx: &'a App) -> &'a [Value] {
        let catalog = self.catalog.as_ref().map(|catalog| catalog.read(cx));

        match (self.root_of(project, cx), catalog) {
            (Some(root), Some(catalog)) => catalog.sessions(&root),
            _ => &[],
        }
    }

    /// The name of the session a project's host starts in, while it starts,
    /// when the window asked for one.
    pub(super) fn opening_name(&self, project: &Project, cx: &App) -> Option<String> {
        match project.opening.as_ref()? {
            Opening::Latest => None,
            Opening::New => Some("New session".into()),
            Opening::Session(session) => Some(
                self.held_sessions(project, cx)
                    .iter()
                    .find(|listed| listed.value == *session)
                    .map(session_name)
                    .unwrap_or_else(|| session.clone()),
            ),
        }
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
    fn puts_the_pinned_first_in_the_order_they_were_pinned() {
        let projects = vec![
            ("/work/alpha".to_string(), 1),
            ("/work/beta".to_string(), 2),
            ("/work/gamma".to_string(), 3),
            ("/work/delta".to_string(), 4),
        ];
        let pinned = ["/work/gamma".to_string(), "/work/alpha".to_string()];

        let ordered: Vec<i32> = pinned_first(projects, &pinned)
            .into_iter()
            .map(|(_, project)| project)
            .collect();

        assert_eq!(ordered, [3, 1, 2, 4]);
    }

    #[test]
    fn names_a_session_by_its_title_or_else_its_id() {
        assert_eq!(session_name(&listed("Fix the test", "")), "Fix the test");
        assert_eq!(session_name(&listed("", "")), "4cf0261c");
    }
}
