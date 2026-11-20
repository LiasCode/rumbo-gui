use gpui::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, ParentElement as _, SharedString,
    Styled as _, Window, div, px,
};
use ui::{
    ActiveTheme as _, Input, InputState, Settings, SettingsGroup, SettingsItem, SettingsPage,
    Sizable as _, Switch, Toggle, ToggleGroup, h_flex, v_flex,
};

use super::{Demos, note, section, told};
use crate::Overview;

const THEMES: [&str; 3] = ["Light", "Dark", "System"];

pub struct Demo {
    theme: usize,
    notify: bool,
    editor: Entity<InputState>,
    told: Vec<SharedString>,
}

impl Demo {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        Demo {
            theme: 2,
            notify: true,
            editor: cx.new(|cx| InputState::new(window, cx).placeholder("zed, code, …")),
            told: Vec::new(),
        }
    }
}

/// A setting's title with what it is for below it, and its control.
fn setting(title: &str, about: &str, control: impl IntoElement, cx: &gpui::App) -> AnyElement {
    h_flex()
        .gap_4()
        .justify_between()
        .child(
            v_flex()
                .min_w_0()
                .gap_0p5()
                .child(div().text_sm().child(title.to_string()))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().colors.muted_foreground)
                        .child(about.to_string()),
                ),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.settings;
    let overview = cx.weak_entity();
    let theme = demo.theme;
    let notify = demo.notify;
    let editor = demo.editor.clone();

    let theming = overview.clone();
    let theme_item = SettingsItem::new(move |_, cx| {
        let theming = theming.clone();

        setting(
            "Theme",
            "System follows the desktop.",
            ToggleGroup::new()
                .outline()
                .small()
                .single()
                .children(THEMES.iter().enumerate().map(|(index, name)| {
                    Toggle::new(("theme", index))
                        .label(*name)
                        .checked(index == theme)
                }))
                .on_change(move |checks: &[bool], _, cx| {
                    let Some(chosen) = checks.iter().position(|checked| *checked) else {
                        return;
                    };

                    theming
                        .update(cx, |this, cx| {
                            this.demos.settings.theme = chosen;
                            note(
                                &mut this.demos.settings.told,
                                format!("Theme {}", THEMES[chosen]),
                            );
                            cx.notify();
                        })
                        .ok();
                }),
            cx,
        )
    })
    .keywords(["theme", "dark", "light"]);

    let notifying = overview.clone();
    let notify_item = SettingsItem::new(move |_, cx| {
        let notifying = notifying.clone();

        setting(
            "Say when a run ends elsewhere",
            "A notification when a run ends in a session that is not shown.",
            Switch::new("notify")
                .checked(notify)
                .on_change(move |on: &bool, _, cx| {
                    let on = *on;

                    notifying
                        .update(cx, |this, cx| {
                            this.demos.settings.notify = on;
                            note(&mut this.demos.settings.told, format!("Notify {on}"));
                            cx.notify();
                        })
                        .ok();
                }),
            cx,
        )
    })
    .keywords(["notification", "notify", "quiet"]);

    let editor_item = SettingsItem::new(move |_, cx| {
        setting(
            "Editor",
            "The command that opens a project's folder.",
            div().w(px(220.)).child(Input::new(&editor).small()),
            cx,
        )
    })
    .keywords(["editor", "zed", "code"]);

    let keys = [
        ("Ctrl-P", "Open the commands"),
        ("Ctrl-N", "Start a new session"),
        ("Ctrl-B", "Show or hide the sidebar"),
        ("Escape", "Cancel the run"),
    ]
    .map(|(keys, does)| {
        SettingsItem::new(move |_, cx| {
            setting(
                does,
                "",
                div()
                    .text_xs()
                    .font_family(cx.theme().mono_family.clone())
                    .child(keys),
                cx,
            )
        })
        .keywords([keys, does])
    });

    let providers = ["toy", "claude", "openai"].map(|name| {
        SettingsItem::new(move |_, cx| setting(name, "A program that reaches a model.", div(), cx))
            .keywords([name, "provider"])
    });

    let settings = Settings::new("settings")
        .page(
            SettingsPage::new("Window")
                .group(SettingsGroup::new().title("Appearance").item(theme_item))
                .group(
                    SettingsGroup::new()
                        .title("Behavior")
                        .description("What the window does by itself.")
                        .item(notify_item)
                        .item(editor_item),
                ),
        )
        .page(
            SettingsPage::new("Providers")
                .group(SettingsGroup::new().title("Providers").items(providers)),
        )
        .page(
            SettingsPage::new("Keyboard")
                .group(SettingsGroup::new().title("Shortcuts").items(keys)),
        );

    vec![
        section(
            "Pages of settings",
            "Type in the search to keep the settings whose words hold it; it shows the first page that has any, until another is chosen.",
            div().w(px(900.)).h(px(520.)).child(settings),
            cx,
        ),
        section("", "", told(&demo.told, cx), cx),
    ]
}
