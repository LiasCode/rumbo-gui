//! The header above a session: which project and session are shown, a
//! mark while a run works in it, the panels beside the conversation, and
//! the session's menu, with what the host offers to do with it.

use gpui::{
    App, ClipboardItem, Context, IntoElement, ParentElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};
use ui::{
    ActiveTheme as _, Button, DropdownMenu, IconName, MenuItem, Placement, Sizable as _, Spinner,
    h_flex, rems_from_px,
};

use super::{Panel, Workspace};
use crate::protocol;

impl Workspace {
    pub(super) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().colors.muted_foreground;
        let project = folder_name(&self.dir);
        let title = self
            .session_title()
            .unwrap_or_else(|| match self.current.is_empty() {
                true => "New session".into(),
                false => self.current.clone(),
            });
        let working = self
            .views
            .get(&self.current)
            .is_some_and(|view| view.run.is_some());

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
            .h(rems_from_px(44.))
            .flex_shrink_0()
            .px_4()
            .gap_2()
            .justify_between()
            .border_b_1()
            .border_color(cx.theme().colors.border)
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .text_sm()
                    .child(div().flex_shrink_0().text_color(muted).child(project))
                    .child(div().flex_shrink_0().text_color(muted).child("/"))
                    .child(div().min_w_0().truncate().child(title))
                    .when(working, |row| row.child(Spinner::new().xsmall())),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_1()
                    .child(panel(
                        "panel-changes",
                        IconName::GitCompare,
                        Panel::Changes,
                        "Changes in the project's folder (Ctrl-Shift-G)",
                    ))
                    .child(panel(
                        "panel-details",
                        IconName::PanelRight,
                        Panel::Details,
                        "The session's details (Ctrl-Alt-B)",
                    ))
                    .child(self.session_menu(cx)),
            )
    }

    /// What can be done with the session shown, as the host offers it. A new
    /// session has nothing to act on until its first run makes it.
    fn session_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = cx.weak_entity();
        let made = !self.current.is_empty() && self.ended.is_none();
        let session = self.current.clone();
        let offers = |name: &str| made && self.host_function(name).is_some();
        let (rename, branch, compact, delete) = (
            offers(protocol::FUNCTION_RENAME),
            offers(protocol::FUNCTION_BRANCH),
            offers(protocol::FUNCTION_COMPACT),
            offers(protocol::FUNCTION_DELETE),
        );
        let exportable = self
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

/// The last part of a folder's path, which is how a person tells projects
/// apart.
pub(super) fn folder_name(dir: &str) -> String {
    std::path::Path::new(dir)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.to_string())
}
