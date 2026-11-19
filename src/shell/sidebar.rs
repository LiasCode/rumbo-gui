//! The projects beside the work, each with its sessions: those open in the
//! window, whose host lists their sessions and marks the one shown and those
//! working, and the others rumbo holds, which open in a host of their own
//! when the person chooses one. A search narrows both to the sessions and
//! projects whose names, or what the host says of them, hold what was typed.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    menu::PopupMenuItem,
    resizable::{HandleEdge, resize_handle},
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    spinner::Spinner,
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Axis, ClipboardItem, Context, DragMoveEvent, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Window, div, prelude::FluentBuilder as _,
    px,
};

use super::{Opening, Project, Shell, State, folder_name};
use crate::protocol::{self, Value};

/// The sidebar's width when the person has not chosen one, and the least
/// and most it can be dragged to.
pub(super) const DEFAULT_WIDTH: f32 = 272.;
const MIN_WIDTH: f32 = 200.;
const MAX_WIDTH: f32 = 520.;

/// What is dragged when the sidebar's edge is.
#[derive(Clone)]
pub(super) struct ResizeSidebar;

impl Render for ResizeSidebar {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

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
fn described(
    menu: gpui_kit::component::menu::PopupMenu,
    said: &str,
) -> gpui_kit::component::menu::PopupMenu {
    if said.is_empty() {
        return menu;
    }

    menu.item(PopupMenuItem::label(said.to_string()))
        .item(PopupMenuItem::separator())
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

        let open: Vec<SidebarMenuItem> = self
            .projects
            .iter()
            .filter_map(|project| self.project_item(project, &query, cx))
            .collect();

        let recent = self.recent_items(&query, cx);
        let width = self.sidebar_width();

        let new_session = Button::new("sidebar-new-session")
            .outline()
            .small()
            .w_full()
            .icon(Lucide::SquarePen)
            .label("New Session")
            .disabled(self.selected_workspace().is_none())
            .tooltip("Start the next task in a new session (Ctrl-N)")
            .on_click(cx.listener(|this, _, window, cx| {
                this.new_session_shown(&super::NewSession, window, cx)
            }));

        let header = SidebarHeader::new().child(
            v_flex().w_full().gap_2().child(new_session).child(
                Input::new(&self.search)
                    .small()
                    .cleanable(true)
                    .prefix(Icon::new(Lucide::Search).xsmall()),
            ),
        );

        let footer = SidebarFooter::new().child(
            h_flex()
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
                        .on_click(
                            cx.listener(|this, _, window, cx| this.choose_folder(window, cx)),
                        ),
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
                ),
        );

        let mut sidebar = Sidebar::new("projects")
            .w(px(width))
            .header(header)
            .footer(footer);

        if !open.is_empty() {
            sidebar = sidebar
                .child(SidebarGroup::new("Projects").child(SidebarMenu::new().children(open)));
        }

        if !recent.is_empty() {
            sidebar = sidebar
                .child(SidebarGroup::new("Recent").child(SidebarMenu::new().children(recent)));
        }

        div()
            .relative()
            .h_full()
            .flex_shrink_0()
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<ResizeSidebar>, _, cx| {
                    let width = (event.event.position.x - event.bounds.left()).as_f32();

                    this.preferred.preferences.sidebar_width =
                        Some(width.clamp(MIN_WIDTH, MAX_WIDTH).round() as u32);
                    this.resized = true;
                    cx.notify();
                }),
            )
            .child(sidebar)
            .child(
                resize_handle("sidebar-edge", Axis::Horizontal)
                    .inside(HandleEdge::Trailing)
                    .on_drag(ResizeSidebar, |drag, _, _, cx| cx.new(|_| drag.clone())),
            )
    }

    /// The projects rumbo holds that are not open here, most recently used
    /// first, or why they could not be listed.
    fn recent_items(&self, query: &str, cx: &mut Context<Self>) -> Vec<SidebarMenuItem> {
        let Some(catalog) = &self.catalog else {
            return Vec::new();
        };

        let catalog = catalog.read(cx);

        if let Some(problem) = catalog.problem() {
            let first = problem.lines().next().unwrap_or_default();

            return vec![SidebarMenuItem::new(format!("Not listed: {first}")).disable(true)];
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
    ) -> Option<SidebarMenuItem> {
        let root = listed.value.clone();
        let shell = cx.weak_entity();
        let label = if listed.label.is_empty() {
            root.clone()
        } else {
            listed.label.clone()
        };

        let project_matches = matches(query, listed);

        let rows: Vec<SidebarMenuItem> = sessions
            .iter()
            .filter(|session| project_matches || matches(query, session))
            .map(|session| {
                let shell = shell.clone();
                let root = root.clone();
                let id = session.value.clone();
                let copied = id.clone();
                let said = session.description.clone();

                SidebarMenuItem::new(session_name(session))
                    .on_click(move |_, window, cx| {
                        let opening = Opening::Session(id.clone());

                        let _ = shell.update(cx, |this, cx| {
                            this.open_listed(root.clone(), opening, window, cx)
                        });
                    })
                    .context_menu(move |menu, _, _| {
                        let copied = copied.clone();

                        described(menu, &said).item(
                            PopupMenuItem::new("Copy Session ID")
                                .icon(Lucide::Copy)
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
                                }),
                        )
                    })
            })
            .collect();

        if !project_matches && rows.is_empty() {
            return None;
        }

        let opened = |opening: Opening| {
            let shell = shell.clone();
            let root = root.clone();

            move |_: &gpui_kit::ClickEvent, window: &mut gpui_kit::Window, cx: &mut App| {
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

        let item = SidebarMenuItem::new(label)
            .icon(IconName::FolderClosed)
            .default_open(!query.is_empty())
            .on_click(opened(Opening::Latest))
            .suffix(move |_, _| {
                let starting = starting.clone();

                Button::new(gpui_kit::ElementId::Name(format!("start-{root}").into()))
                    .ghost()
                    .xsmall()
                    .icon(Lucide::SquarePen)
                    .tooltip("New session")
                    .on_click(move |event, window, cx| starting(event, window, cx))
            })
            .context_menu(move |menu, _, _| {
                let opening = menu_opening.clone();
                let starting = menu_starting.clone();

                described(menu, &said)
                    .item(
                        PopupMenuItem::new("Open")
                            .icon(IconName::FolderOpen)
                            .on_click(move |event, window, cx| opening(event, window, cx)),
                    )
                    .item(
                        PopupMenuItem::new("New Session")
                            .icon(Lucide::SquarePen)
                            .on_click(move |event, window, cx| starting(event, window, cx)),
                    )
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
    ) -> Option<SidebarMenuItem> {
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

        let mut item = SidebarMenuItem::new(name)
            .icon(IconName::FolderOpen)
            .default_open(true)
            .on_click(cx.listener(move |this, _, window, cx| this.select(id, window, cx)))
            .context_menu({
                let shell = shell.clone();

                move |menu, _, _| {
                    let starting = shell.clone();
                    let editing = shell.clone();
                    let closing = shell.clone();

                    menu.item(
                        PopupMenuItem::new("New Session")
                            .icon(Lucide::SquarePen)
                            .on_click(move |_, window, cx| {
                                let _ = starting
                                    .update(cx, |this, cx| this.new_session(id, window, cx));
                            }),
                    )
                    .item(
                        PopupMenuItem::new("Open in Editor")
                            .icon(Lucide::ExternalLink)
                            .on_click(move |_, window, cx| {
                                let _ = editing
                                    .update(cx, |this, cx| this.open_in_editor(id, window, cx));
                            }),
                    )
                    .item(PopupMenuItem::separator())
                    .item(
                        PopupMenuItem::new("Close Project")
                            .icon(IconName::Close)
                            .on_click(move |_, window, cx| {
                                let _ = closing
                                    .update(cx, |this, cx| this.close_project(id, window, cx));
                            }),
                    )
                }
            });

        let workspace = match &project.state {
            State::Working { workspace, .. } => workspace.read(cx),

            State::Starting => {
                return project_matches.then(|| item.suffix(|_, _| Spinner::new().xsmall()));
            }

            State::Failed(_) => {
                return project_matches.then(|| {
                    item.active(shown).suffix(|_, cx: &mut App| {
                        Icon::new(IconName::CircleAlert)
                            .xsmall()
                            .text_color(cx.theme().danger)
                    })
                });
            }
        };

        if workspace.offers(protocol::FUNCTION_NEW) && !workspace.has_ended() {
            let shell = shell.clone();

            item = item.suffix(move |_, _| {
                let shell = shell.clone();

                Button::new(("new-session", id))
                    .ghost()
                    .xsmall()
                    .icon(Lucide::SquarePen)
                    .tooltip("New session")
                    .on_click(move |_, window, cx| {
                        let _ = shell.update(cx, |this, cx| this.new_session(id, window, cx));
                    })
            });
        }

        let current = workspace.current().to_string();
        let can_open = workspace.offers(protocol::FUNCTION_OPEN);
        let can_rename = workspace.offers(protocol::FUNCTION_RENAME);
        let can_delete = workspace.offers(protocol::FUNCTION_DELETE);
        let mut sessions = Vec::new();

        // A new session is shown before it has an id, and is listed once a
        // run makes it.
        if current.is_empty() && shown && query.is_empty() {
            sessions.push(
                SidebarMenuItem::new("New session")
                    .icon(Lucide::SquarePen)
                    .active(true),
            );
        }

        for session in workspace
            .sessions()
            .iter()
            .filter(|session| project_matches || matches(query, session))
        {
            let session_id = session.value.clone();
            let mut row =
                SidebarMenuItem::new(session_name(session)).active(shown && session_id == current);

            if can_open {
                let opened = session_id.clone();

                row = row.on_click(cx.listener(move |this, _, window, cx| {
                    this.open_session(id, opened.clone(), window, cx)
                }));
            }

            // Only this window's runs work in this host. A session whose run
            // ended while another was shown is marked until it is opened.
            if workspace.is_working(&session_id) {
                row = row.suffix(|_, _| Spinner::new().xsmall());
            } else if workspace.has_unseen(&session_id) {
                row = row.suffix(|_, cx: &mut App| {
                    div().size(px(7.)).rounded_full().bg(cx.theme().primary)
                });
            }

            let said = session.description.clone();
            let shell = shell.clone();

            row = row.context_menu(move |menu, _, _| {
                let opening = shell.clone();
                let renaming = shell.clone();
                let deleting = shell.clone();
                let target = session_id.clone();
                let renamed = session_id.clone();
                let deleted = session_id.clone();
                let copied = session_id.clone();

                described(menu, &said)
                    .when(can_open, |menu| {
                        menu.item(
                            PopupMenuItem::new("Open")
                                .icon(Lucide::MessageSquare)
                                .on_click(move |_, window, cx| {
                                    let _ = opening.update(cx, |this, cx| {
                                        this.open_session(id, target.clone(), window, cx)
                                    });
                                }),
                        )
                    })
                    .when(can_rename && can_open, |menu| {
                        menu.item(PopupMenuItem::new("Rename…").icon(Lucide::Pencil).on_click(
                            move |_, window, cx| {
                                let _ = renaming.update(cx, |this, cx| {
                                    this.rename_session(id, renamed.clone(), window, cx)
                                });
                            },
                        ))
                    })
                    .item(
                        PopupMenuItem::new("Copy Session ID")
                            .icon(Lucide::Copy)
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
                            }),
                    )
                    .when(can_delete, |menu| {
                        menu.item(PopupMenuItem::separator()).item(
                            PopupMenuItem::new("Delete…").icon(Lucide::Trash).on_click(
                                move |_, window, cx| {
                                    let _ = deleting.update(cx, |this, cx| {
                                        this.delete_session(id, deleted.clone(), window, cx)
                                    });
                                },
                            ),
                        )
                    })
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
