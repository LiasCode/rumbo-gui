//! The projects beside the work, each with its sessions: those open in the
//! window, whose host lists their sessions and marks the one shown and those
//! working, and the others rumbo holds, which open in a host of their own
//! when the person chooses one.

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::PopupMenuItem,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    spinner::Spinner,
};
use gpui_kit::{
    App, Context, IntoElement, ParentElement as _, Styled as _, prelude::FluentBuilder as _,
};

use super::{Opening, Project, Shell, State, folder_name};
use crate::protocol::{self, Value};

impl Shell {
    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let opens_projects = self.opens_projects();

        let open: Vec<SidebarMenuItem> = self
            .projects
            .iter()
            .map(|project| self.project_item(project, cx))
            .collect();

        let recent = self.recent_items(cx);

        let sidebar = Sidebar::new("projects")
            .header(
                SidebarHeader::new().child(
                    h_flex().w_full().child(
                        Button::new("open-project")
                            .ghost()
                            .small()
                            .w_full()
                            .icon(IconName::FolderOpen)
                            .label("Open Project…")
                            .disabled(!opens_projects)
                            .tooltip(if opens_projects {
                                "Work in another folder (Ctrl-O)"
                            } else {
                                "This window belongs to the rumbo that started it, which works in one project"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.choose_folder(window, cx)
                            })),
                    ),
                ),
            )
            .when(!open.is_empty(), |sidebar| {
                sidebar.child(SidebarGroup::new("Open").child(SidebarMenu::new().children(open)))
            });

        if recent.is_empty() {
            return sidebar;
        }

        sidebar.child(SidebarGroup::new("Recent").child(SidebarMenu::new().children(recent)))
    }

    /// The projects rumbo holds that are not open here, most recently used
    /// first, or why they could not be listed.
    fn recent_items(&self, cx: &mut Context<Self>) -> Vec<SidebarMenuItem> {
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
            .map(|listed| self.recent_item(listed, catalog.sessions(&listed.value), cx))
            .collect()
    }

    /// A project rumbo holds, with its sessions, which opens with the one
    /// chosen, or with its latest.
    fn recent_item(
        &self,
        listed: &Value,
        sessions: &[Value],
        cx: &Context<Self>,
    ) -> SidebarMenuItem {
        let root = listed.value.clone();
        let shell = cx.weak_entity();
        let label = if listed.label.is_empty() {
            root.clone()
        } else {
            listed.label.clone()
        };

        let rows: Vec<SidebarMenuItem> = sessions
            .iter()
            .map(|session| {
                let shell = shell.clone();
                let root = root.clone();
                let id = session.value.clone();
                let label = if session.label.is_empty() {
                    id.clone()
                } else {
                    session.label.clone()
                };

                SidebarMenuItem::new(label).on_click(move |_, window, cx| {
                    let opening = Opening::Session(id.clone());

                    let _ = shell.update(cx, |this, cx| {
                        this.open_listed(root.clone(), opening, window, cx)
                    });
                })
            })
            .collect();

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

        SidebarMenuItem::new(label)
            .icon(IconName::FolderClosed)
            .on_click(opened(Opening::Latest))
            .suffix(move |_, _| {
                let starting = starting.clone();

                Button::new(gpui_kit::ElementId::Name(format!("start-{root}").into()))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .tooltip("New session")
                    .on_click(move |event, window, cx| starting(event, window, cx))
            })
            .context_menu(move |menu, _, _| {
                let opening = menu_opening.clone();
                let starting = menu_starting.clone();

                menu.item(
                    PopupMenuItem::new("Open")
                        .on_click(move |event, window, cx| opening(event, window, cx)),
                )
                .item(
                    PopupMenuItem::new("New Session")
                        .on_click(move |event, window, cx| starting(event, window, cx)),
                )
            })
            .children(rows)
    }

    /// A project, with its sessions under it once its host lists them.
    fn project_item(&self, project: &Project, cx: &mut Context<Self>) -> SidebarMenuItem {
        let id = project.id;
        let shown = self.selected == Some(id) && self.settings.is_none();
        let shell = cx.weak_entity();

        let mut item = SidebarMenuItem::new(folder_name(&project.dir))
            .icon(IconName::FolderOpen)
            .default_open(true)
            .on_click(cx.listener(move |this, _, window, cx| this.select(id, window, cx)))
            .context_menu({
                let shell = shell.clone();

                move |menu, _, _| {
                    let starting = shell.clone();
                    let closing = shell.clone();

                    menu.item(
                        PopupMenuItem::new("New Session").on_click(move |_, window, cx| {
                            let _ =
                                starting.update(cx, |this, cx| this.new_session(id, window, cx));
                        }),
                    )
                    .item(
                        PopupMenuItem::new("Close Project").on_click(move |_, window, cx| {
                            let _ =
                                closing.update(cx, |this, cx| this.close_project(id, window, cx));
                        }),
                    )
                }
            });

        let workspace = match &project.state {
            State::Working { workspace, .. } => workspace.read(cx),

            State::Starting => {
                return item.suffix(|_, _| Spinner::new().xsmall());
            }

            State::Failed(_) => {
                return item.active(shown).suffix(|_, cx: &mut App| {
                    Icon::new(IconName::CircleAlert)
                        .xsmall()
                        .text_color(cx.theme().danger)
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
                    .icon(IconName::Plus)
                    .tooltip("New session")
                    .on_click(move |_, window, cx| {
                        let _ = shell.update(cx, |this, cx| this.new_session(id, window, cx));
                    })
            });
        }

        let current = workspace.current().to_string();
        let can_open = workspace.offers(protocol::FUNCTION_OPEN);
        let mut sessions = Vec::new();

        // A new session is shown before it has an id, and is listed once a
        // run makes it.
        if current.is_empty() && shown {
            sessions.push(SidebarMenuItem::new("New session").active(true));
        }

        for session in workspace.sessions() {
            let session_id = session.value.clone();
            let label = if session.label.is_empty() {
                session_id.clone()
            } else {
                session.label.clone()
            };

            let mut row = SidebarMenuItem::new(label).active(shown && session_id == current);

            if can_open {
                let opened = session_id.clone();

                row = row.on_click(cx.listener(move |this, _, window, cx| {
                    this.open_session(id, opened.clone(), window, cx)
                }));
            }

            // Only this window's runs work in this host.
            if workspace.is_working(&session_id) {
                row = row.suffix(|_, _| Spinner::new().xsmall());
            }

            sessions.push(row);
        }

        // The project's row stands for its session while it has none
        // listed yet.
        item.active(shown && sessions.is_empty()).children(sessions)
    }
}
