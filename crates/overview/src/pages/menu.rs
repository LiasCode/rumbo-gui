use gpui::{
    AnyElement, Context, InteractiveElement as _, ParentElement as _, SharedString, Styled as _,
    Window, div, px,
};
use ui::{
    ActiveTheme as _, Button, ContextMenu, DropdownMenu, IconName, MenuItem, Placement, v_flex,
};

use super::{Demos, note, row, section, told};
use crate::Overview;

#[derive(Default)]
pub struct Demo {
    wrap: bool,
    told: Vec<SharedString>,
}

/// An item that tells when it is done.
fn told_item(label: &'static str, cx: &mut Context<Overview>) -> MenuItem {
    let overview = cx.weak_entity();

    MenuItem::new(label).on_click(move |_, cx| {
        overview
            .update(cx, |this, cx| {
                note(&mut this.demos.menu.told, format!("Chose {label}"));
                cx.notify();
            })
            .ok();
    })
}

/// What a session's menu holds, as the window's has it.
fn session_items(cx: &mut Context<Overview>) -> Vec<MenuItem> {
    vec![
        MenuItem::label("done; 3 turns, 1,234 tokens"),
        MenuItem::separator(),
        told_item("Rename…", cx).icon(IconName::Pencil),
        told_item("Branch", cx).icon(IconName::GitBranch),
        told_item("Compact Now", cx)
            .icon(IconName::Shrink)
            .disabled(true),
        MenuItem::separator(),
        told_item("Export as Markdown…", cx)
            .icon(IconName::Download)
            .shortcut("Ctrl-E"),
        told_item("Copy Session ID", cx).icon(IconName::Copy),
        MenuItem::separator(),
        told_item("Delete…", cx).icon(IconName::Trash).danger(),
    ]
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let theme = cx.theme();
    let wrap = demos.menu.wrap;
    let overview = cx.weak_entity();

    let dropdowns = row()
        .child(
            DropdownMenu::new(
                "session-menu",
                Button::new("session-menu-button")
                    .ghost()
                    .icon(IconName::Ellipsis)
                    .tooltip("Session"),
            )
            .placement(Placement::BottomEnd)
            .items({
                let overview = overview.clone();

                move |_, cx| {
                    overview
                        .update(cx, |_, cx| session_items(cx))
                        .unwrap_or_default()
                }
            }),
        )
        .child(
            DropdownMenu::new(
                "view-menu",
                Button::new("view-menu-button").outline().label("View"),
            )
            .items(move |_, _| {
                let toggled = overview.clone();

                vec![
                    MenuItem::new("Wrap lines")
                        .checked(wrap)
                        .on_click(move |_, cx| {
                            toggled
                                .update(cx, |this, cx| {
                                    let demo = &mut this.demos.menu;

                                    demo.wrap = !demo.wrap;
                                    note(&mut demo.told, format!("Wrap lines: {}", demo.wrap));
                                    cx.notify();
                                })
                                .ok();
                        }),
                    MenuItem::new("Show tool calls").checked(true),
                ]
            }),
        );

    let area = ContextMenu::new(
        "row-menu",
        div()
            .id("row")
            .w(px(420.))
            .px_4()
            .py_6()
            .rounded(theme.radius)
            .border_1()
            .border_dashed()
            .border_color(theme.colors.border)
            .text_sm()
            .text_color(theme.colors.muted_foreground)
            .tab_index(0)
            .child("Press here with the other button, or Tab here and press Shift-F10."),
    )
    .items({
        let overview = cx.weak_entity();

        move |_, cx| {
            overview
                .update(cx, |_, cx| session_items(cx))
                .unwrap_or_default()
        }
    });

    vec![
        section(
            "Under a button",
            "Press it, or Tab to it and press Enter, which marks the first item. Up and Down go through the items past those disabled, Enter does the one marked, and Escape or a press outside closes it.",
            dropdowns,
            cx,
        ),
        section(
            "Where the pointer pressed",
            "A context menu, as a session's row in the sidebar has.",
            v_flex().child(area),
            cx,
        ),
        section("What they told", "", told(&demos.menu.told, cx), cx),
    ]
}
