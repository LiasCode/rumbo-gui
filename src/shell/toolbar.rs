//! The toolbar along the top of the window, which is also its title bar:
//! what is shown, and the commands and the settings, which act on the
//! project shown.

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
};
use gpui_kit::{Context, IntoElement, ParentElement as _, Styled as _, div};

use super::{CloseSettings, OpenPalette, OpenSettings, Shell};

impl Shell {
    pub(super) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_shown = self.settings.is_some();
        let has_commands = self.selected_workspace().is_some();

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

        TitleBar::new().child(
            h_flex()
                .flex_1()
                .min_w_0()
                .pr_2()
                .gap_2()
                .justify_between()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child(self.title(cx)),
                )
                .child(h_flex().gap_1().child(commands).child(settings)),
        )
    }
}
