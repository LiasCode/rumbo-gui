//! The sessions of the project, beside the conversation: the one shown, the
//! ones working, and a way to open any of them or start a new one.

use gpui_kit::component::{
    IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    spinner::Spinner,
};
use gpui_kit::{Context, IntoElement, ParentElement as _, Styled as _, Window, div};
use serde_json::json;

use super::{OpenPalette, Workspace};

impl Workspace {
    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut items = Vec::new();

        // A new session is shown before it has an id, and is listed once a
        // run makes it.
        if self.current.is_empty() {
            items.push(SidebarMenuItem::new("New session").active(true));
        }

        for session in &self.listed {
            let id = session.value.clone();
            // Only this window's runs work in this host.
            let working = self.runs.values().any(|key| *key == id);

            let label = if session.label.is_empty() {
                id.clone()
            } else {
                session.label.clone()
            };

            let mut item = SidebarMenuItem::new(label)
                .active(id == self.current)
                .on_click(
                    cx.listener(move |this, _, window, cx| this.open_session(&id, window, cx)),
                );

            if working {
                item = item.suffix(|_, _| Spinner::new().xsmall());
            }

            items.push(item);
        }

        Sidebar::new("sessions")
            .header(
                SidebarHeader::new().child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .child(div().text_sm().child("Sessions"))
                        .child(
                            Button::new("new-session")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Plus)
                                .tooltip("New session")
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.new_session(window, cx)),
                                ),
                        ),
                ),
            )
            .child(SidebarGroup::new("Recent").child(SidebarMenu::new().children(items)))
            .footer(
                SidebarFooter::new().child(
                    Button::new("commands")
                        .ghost()
                        .small()
                        .icon(IconName::Menu)
                        .label("Commands")
                        .tooltip("Ctrl-P")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_palette(&OpenPalette, window, cx)
                        })),
                ),
            )
    }

    /// Makes another session the current one. A run working in the one left
    /// behind goes on in the background.
    fn open_session(&mut self, id: &str, _: &mut Window, cx: &mut Context<Self>) {
        if id == self.current || self.ended.is_some() {
            return;
        }

        self.call_host("open", Some(json!({ "session": id })), cx);
    }

    /// Makes the next task start a new session.
    fn new_session(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.current.is_empty() || self.ended.is_some() {
            return;
        }

        self.call_host("new", None, cx);
    }
}
