use gpui::{AnyElement, Context, ParentElement as _, SharedString, Styled as _, Window};
use ui::{IconName, Sizable as _, Size, Toggle, ToggleGroup, v_flex};

use super::{Demos, note, row, section, told};
use crate::Overview;

pub struct Demo {
    sidebar: bool,
    wrap: bool,
    changes: bool,
    sizes: [bool; 4],
    /// Which panel shows beside the work, as the session's header has it.
    panels: [bool; 2],
    /// Which rows of a conversation show.
    shown: [bool; 3],
    told: Vec<SharedString>,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            sidebar: true,
            wrap: false,
            changes: false,
            sizes: [false, true, false, true],
            panels: [false, true],
            shown: [true, true, false],
            told: Vec::new(),
        }
    }
}

const SIZES: [Size; 4] = [Size::XSmall, Size::Small, Size::Medium, Size::Large];

const PANELS: [(&str, IconName, &str); 2] = [
    (
        "Changes",
        IconName::GitCompare,
        "Changes in the project's folder",
    ),
    ("Details", IconName::PanelRight, "The session's details"),
];

const ROWS: [&str; 3] = ["Tasks", "Answers", "Tool calls"];

/// What a group holds pressed, by the names of its toggles.
fn pressed(names: impl IntoIterator<Item = &'static str>, checks: &[bool]) -> String {
    let named: Vec<&str> = names
        .into_iter()
        .zip(checks)
        .filter(|(_, checked)| **checked)
        .map(|(name, _)| name)
        .collect();

    match named.is_empty() {
        true => "none".to_string(),
        false => named.join(", "),
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.toggle;

    let alone = row()
        .gap_4()
        .child(
            Toggle::new("sidebar")
                .icon(IconName::PanelLeft)
                .checked(demo.sidebar)
                .tooltip("Show or put away the sidebar")
                .shortcut("Ctrl-B")
                .on_change(cx.listener(|this, on: &bool, _, cx| {
                    let demo = &mut this.demos.toggle;

                    demo.sidebar = *on;
                    note(&mut demo.told, format!("Sidebar shown: {on}"));
                    cx.notify();
                })),
        )
        .child(
            Toggle::new("wrap")
                .label("Wrap lines")
                .checked(demo.wrap)
                .on_change(cx.listener(|this, on: &bool, _, cx| {
                    let demo = &mut this.demos.toggle;

                    demo.wrap = *on;
                    note(&mut demo.told, format!("Wrap lines: {on}"));
                    cx.notify();
                })),
        )
        .child(
            Toggle::new("changes")
                .outline()
                .icon(IconName::GitCompare)
                .label("Changes")
                .checked(demo.changes)
                .on_change(cx.listener(|this, on: &bool, _, cx| {
                    let demo = &mut this.demos.toggle;

                    demo.changes = *on;
                    note(&mut demo.told, format!("Changes shown: {on}"));
                    cx.notify();
                })),
        );

    let sizes = row()
        .gap_4()
        .children(SIZES.iter().enumerate().map(|(index, size)| {
            Toggle::new(("size", index))
                .icon(IconName::PanelLeft)
                .outline()
                .with_size(*size)
                .checked(demo.sizes[index])
                .on_change(cx.listener(move |this, on: &bool, _, cx| {
                    this.demos.toggle.sizes[index] = *on;
                    cx.notify();
                }))
        }));

    let panels = ToggleGroup::new()
        .single()
        .children(
            PANELS
                .iter()
                .enumerate()
                .map(|(index, (_, icon, tooltip))| {
                    Toggle::new(("panel", index))
                        .icon(*icon)
                        .tooltip(*tooltip)
                        .checked(demo.panels[index])
                }),
        )
        .on_change(cx.listener(|this, checks: &[bool], _, cx| {
            let demo = &mut this.demos.toggle;

            demo.panels.copy_from_slice(checks);
            note(
                &mut demo.told,
                format!(
                    "Panel shown: {}",
                    pressed(PANELS.iter().map(|panel| panel.0), checks)
                ),
            );
            cx.notify();
        }));

    let shown = ToggleGroup::new()
        .outline()
        .children(ROWS.iter().enumerate().map(|(index, name)| {
            Toggle::new(("rows", index))
                .label(*name)
                .checked(demo.shown[index])
        }))
        .on_change(cx.listener(|this, checks: &[bool], _, cx| {
            let demo = &mut this.demos.toggle;

            demo.shown.copy_from_slice(checks);
            note(
                &mut demo.told,
                format!("Rows shown: {}", pressed(ROWS, checks)),
            );
            cx.notify();
        }));

    let small = ToggleGroup::new()
        .outline()
        .small()
        .single()
        .children(PANELS.iter().enumerate().map(|(index, (name, icon, _))| {
            Toggle::new(("small-panel", index))
                .icon(*icon)
                .label(*name)
                .checked(demo.panels[index])
        }))
        .on_change(cx.listener(|this, checks: &[bool], _, cx| {
            this.demos.toggle.panels.copy_from_slice(checks);
            cx.notify();
        }));

    let disabled =
        row()
            .gap_4()
            .child(
                Toggle::new("disabled-off")
                    .icon(IconName::PanelLeft)
                    .disabled(true),
            )
            .child(
                Toggle::new("disabled-on")
                    .label("Wrap lines")
                    .checked(true)
                    .disabled(true),
            )
            .child(ToggleGroup::new().outline().disabled(true).children(
                ROWS.iter().enumerate().map(|(index, name)| {
                    Toggle::new(("disabled-rows", index))
                        .label(*name)
                        .checked(index == 0)
                }),
            ));

    vec![
        section(
            "Alone",
            "Pressed, it stays so until pressed again. Tab reaches it, and Enter or Space presses it; its tooltip says what it does.",
            alone,
            cx,
        ),
        section("Sizes", "As tall as a button of the same size.", sizes, cx),
        section(
            "One choice",
            "At most one is pressed: pressing one lets go of the other, and pressing it again lets go of it, as the panels beside a session do.",
            v_flex().gap_4().child(panels).child(small),
            cx,
        ),
        section(
            "Several",
            "Each is pressed on its own. An outline group is joined into one.",
            shown,
            cx,
        ),
        section("Disabled", "", disabled, cx),
        section("What they told", "", told(&demo.told, cx), cx),
    ]
}
