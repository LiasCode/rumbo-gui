use gpui::{AnyElement, Context, ParentElement as _, Styled as _, Window, div, px};
use ui::{ActiveTheme as _, IconName, Sizable as _, Tab, Tabs, v_flex};

use super::{Demos, section};
use crate::Overview;

/// The tab each example has chosen.
#[derive(Default)]
pub struct Demo {
    chosen: [usize; 3],
}

const PARTS: [(&str, &str); 3] = [
    (
        "Conversation",
        "What was asked, what the model wrote, and each tool call.",
    ),
    ("Changes", "What the runs changed in the project's folder."),
    ("Details", "The session's provider, model, and usage."),
];

/// A row of tabs whose choice the page keeps, numbered among them.
fn tabs(index: usize, demos: &Demos, cx: &mut Context<Overview>) -> Tabs {
    Tabs::new(("tabs", index))
        .selected(demos.tabs.chosen[index])
        .on_change(cx.listener(move |this, to: &usize, _, cx| {
            this.demos.tabs.chosen[index] = *to;
            cx.notify();
        }))
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let colors = cx.theme().colors;
    let radius = cx.theme().radius;
    let part = |chosen: usize| {
        div()
            .w(px(480.))
            .p_4()
            .rounded(radius)
            .border_1()
            .border_color(colors.border)
            .text_sm()
            .text_color(colors.muted_foreground)
            .child(PARTS.get(chosen).map(|part| part.1).unwrap_or_default())
    };

    let pill = v_flex()
        .gap_3()
        .child(tabs(0, demos, cx).children(PARTS.iter().map(|part| Tab::new(part.0))))
        .child(part(demos.tabs.chosen[0]));

    let underline = v_flex()
        .gap_3()
        .child(
            tabs(1, demos, cx)
                .underline()
                .child(Tab::new("Conversation").icon(IconName::Info))
                .child(Tab::new("Changes").icon(IconName::GitCompare))
                .child(Tab::new("Details").icon(IconName::PanelRight))
                .child(Tab::new("Usage").disabled(true)),
        )
        .child(part(demos.tabs.chosen[1]));

    let small = tabs(2, demos, cx)
        .small()
        .child(Tab::new("Day"))
        .child(Tab::new("Week"))
        .child(Tab::new("Month"));

    vec![
        section(
            "On a ground",
            "Pressing a tab chooses it. The row is one stop for Tab: Left and Right go to the tab beside, coming round at the ends, and Home and End to the first and the last.",
            pill,
            cx,
        ),
        section(
            "Over a line",
            "With icons, and a tab disabled that the keys pass.",
            underline,
            cx,
        ),
        section("Small", "", small, cx),
    ]
}
