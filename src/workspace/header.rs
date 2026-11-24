//! What the window's title bar shows of the session shown: its name, the
//! panels beside the conversation, and the session's menu, with what the
//! host offers to do with it.

use gpui::{
    AnyElement, App, ClipboardItem, Context, IntoElement, ParentElement as _, Styled as _, Window,
};
use ui::{Button, DropdownMenu, IconName, MenuItem, Placement, Sizable as _, h_flex};

use super::{Panel, Workspace};
use crate::protocol;

impl Workspace {
    /// The name of the session shown: its title, or its id until it has
    /// one, or New session until its first run makes it. None while no
    /// session is shown.
    pub fn shown_name(&self) -> Option<String> {
        if self.blank {
            return None;
        }

        Some(
            self.session_title()
                .unwrap_or_else(|| match self.current.is_empty() {
                    true => "New session".into(),
                    false => self.current.clone(),
                }),
        )
    }

    /// The buttons of the panels beside the conversation, and the session's
    /// menu, for the window's title bar.
    pub fn render_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let panel = |id: &'static str, icon: IconName, which: Panel, tooltip: &'static str| {
            Button::new(id)
                .ghost()
                .small()
                .icon(icon)
                .selected(self.panel == Some(which))
                .tooltip(tooltip)
                .on_click(
                    cx.listener(move |this, _, window, cx| this.toggle_panel(which, window, cx)),
                )
        };

        h_flex()
            .flex_shrink_0()
            .gap_1()
            .child(panel(
                "panel-changes",
                IconName::GitBranch,
                Panel::Changes,
                "The project's Git changes and history (Ctrl-Shift-G)",
            ))
            .child(panel(
                "panel-files",
                IconName::FolderOpen,
                Panel::Files,
                "Explore project files (Ctrl-Alt-E)",
            ))
            .child(panel(
                "panel-details",
                IconName::Info,
                Panel::Details,
                "The session's details (Ctrl-Alt-B)",
            ))
            .child(self.session_menu(cx))
            .into_any_element()
    }

    /// What can be done with the session shown, as the host offers it. A new
    /// session has nothing to act on until its first run makes it, and
    /// neither has the window while it shows none.
    fn session_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = cx.weak_entity();
        let made = !self.current.is_empty() && self.ended.is_none();
        let session = self.current.clone();
        let offers = |name: &str| made && self.host_function(name).is_some();
        let (rename, branch, compact, delete) = (
            offers(protocol::FUNCTION_RENAME),
            offers(protocol::FUNCTION_BRANCH),
            offers(protocol::FUNCTION_COMPACT),
            // The window deletes a session itself, with rumbo's command
            // line, since no host deletes its own.
            made,
        );
        let exportable = !self.blank
            && self
                .views
                .get(&self.current)
                .is_some_and(|view| !view.conversation.items().is_empty());

        let button = Button::new("session-menu")
            .ghost()
            .small()
            .icon(IconName::Ellipsis)
            .tooltip("Session")
            .disabled(self.ended.is_some());

        DropdownMenu::new("session-menu-items", button)
            .placement(Placement::BottomEnd)
            .items(move |_, _| {
                let act = |action: fn(&mut Workspace, &mut Window, &mut Context<Workspace>)| {
                    let workspace = workspace.clone();

                    move |window: &mut Window, cx: &mut App| {
                        let _ = workspace.update(cx, |this, cx| action(this, window, cx));
                    }
                };
                let copied = session.clone();

                vec![
                    MenuItem::new("Rename…")
                        .icon(IconName::Pencil)
                        .disabled(!rename)
                        .on_click(act(|this, window, cx| {
                            let current = this.current.clone();
                            this.rename(&current, window, cx)
                        })),
                    MenuItem::new("Branch")
                        .icon(IconName::GitBranch)
                        .disabled(!branch)
                        .on_click(act(|this, window, cx| this.branch(None, None, window, cx))),
                    MenuItem::new("Compact Now")
                        .icon(IconName::Shrink)
                        .disabled(!compact)
                        .on_click(act(|this, window, cx| this.compact(window, cx))),
                    MenuItem::separator(),
                    MenuItem::new("Export as Markdown…")
                        .icon(IconName::Download)
                        .disabled(!exportable)
                        .on_click(act(|this, window, cx| this.export(window, cx))),
                    MenuItem::new("Copy Session ID")
                        .icon(IconName::Copy)
                        .disabled(copied.is_empty())
                        .on_click(move |_, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
                        }),
                    MenuItem::separator(),
                    MenuItem::new("Delete…")
                        .icon(IconName::Trash)
                        .disabled(!delete)
                        .on_click(act(|this, window, cx| {
                            let current = this.current.clone();
                            this.delete(&current, window, cx)
                        })),
                ]
            })
    }
}
