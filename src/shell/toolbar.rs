//! The toolbar along the top of the window, which is also its title bar:
//! going back and forward between what was shown, where the window is, what
//! can be done with the session shown, and the commands and the settings,
//! which act on the project shown.

use gpui::{
    AnyElement, Context, IntoElement, ParentElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px,
};
use ui::{
    ActiveTheme as _, Breadcrumb, BreadcrumbItem, Button, IconName, Sizable as _, TitleBar, h_flex,
    rems_from_px,
};

use super::{
    CloseSettings, GoBack, GoForward, OpenPalette, OpenSettings, Shell, State, ToggleSidebar,
    folder_name,
};

impl Shell {
    pub(super) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_shown = self.settings.is_some();
        let has_commands = self.selected_workspace().is_some();
        let can_go_back = self.places.can_go_back(|place| self.can_show(place, cx));
        let can_go_forward = self.places.can_go_forward(|place| self.can_show(place, cx));

        let back = Button::new("go-back")
            .ghost()
            .small()
            .icon(IconName::ChevronLeft)
            .disabled(!can_go_back)
            .tooltip("Back (Alt-Left)")
            .on_click(cx.listener(|this, _, window, cx| this.go_back(&GoBack, window, cx)));

        let forward = Button::new("go-forward")
            .ghost()
            .small()
            .icon(IconName::ChevronRight)
            .disabled(!can_go_forward)
            .tooltip("Forward (Alt-Right)")
            .on_click(cx.listener(|this, _, window, cx| this.go_forward(&GoForward, window, cx)));

        let commands = Button::new("commands")
            .ghost()
            .small()
            .icon(IconName::Search)
            .label("Commands")
            .disabled(!has_commands)
            .tooltip(if has_commands {
                "The commands of rumbo and the provider (Ctrl-P)"
            } else {
                "Open a project first: its rumbo says what can be done"
            })
            .on_click(
                cx.listener(|this, _, window, cx| this.open_palette(&OpenPalette, window, cx)),
            );

        let settings =
            if settings_shown {
                Button::new("done")
                    .primary()
                    .small()
                    .label("Done")
                    .tooltip("Back to the work (Escape)")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.close_settings(&CloseSettings, window, cx)
                    }))
            } else {
                Button::new("settings")
                    .ghost()
                    .small()
                    .icon(IconName::Settings)
                    .tooltip("Settings (Ctrl-,)")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings(&OpenSettings, window, cx)
                    }))
            };

        // What can be done with the session shown, which its workspace
        // draws.
        let session_actions = self
            .selected_workspace()
            .filter(|_| !settings_shown)
            .map(|workspace| workspace.update(cx, |workspace, cx| workspace.render_actions(cx)));

        TitleBar::new().child(
            h_flex()
                .flex_1()
                .min_w_0()
                .pr_2()
                .gap_2()
                .justify_between()
                .child(
                    h_flex()
                        .min_w_0()
                        .gap_1()
                        .when(!settings_shown, |row| {
                            row.child(
                                Button::new("toggle-sidebar")
                                    .ghost()
                                    .small()
                                    .icon(if self.preferred.preferences.sidebar_hidden {
                                        IconName::PanelLeftOpen
                                    } else {
                                        IconName::PanelLeftClose
                                    })
                                    .tooltip("Show or put away the sidebar (Ctrl-B)")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_sidebar(&ToggleSidebar, window, cx)
                                    })),
                            )
                        })
                        .child(back)
                        .child(forward)
                        .child(div().min_w_0().pl_1().child(self.render_place(cx))),
                )
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .gap_1()
                        .when_some(session_actions, |row, actions| {
                            row.child(actions).child(
                                div()
                                    .mx_1()
                                    .w(px(1.))
                                    .h(rems_from_px(16.))
                                    .bg(cx.theme().colors.border),
                            )
                        })
                        .child(commands)
                        .child(settings),
                ),
        )
    }

    /// Where the window is: the settings, or the project shown and its
    /// session.
    fn render_place(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.settings.is_some() {
            return Breadcrumb::new("place")
                .child(BreadcrumbItem::new("Settings"))
                .into_any_element();
        }

        let Some(project) = self.selected.and_then(|id| self.project(id)) else {
            return div().into_any_element();
        };

        let session = match &project.state {
            State::Working { workspace, .. } => Some(workspace.read(cx).shown_name()),
            _ => None,
        };

        Breadcrumb::new("place")
            .child(BreadcrumbItem::new(folder_name(&project.dir)))
            .children(session.map(BreadcrumbItem::new))
            .into_any_element()
    }
}
