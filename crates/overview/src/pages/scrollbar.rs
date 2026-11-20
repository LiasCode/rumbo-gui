use gpui::{
    AnyElement, Axis, Context, InteractiveElement as _, ParentElement as _, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use ui::{ActiveTheme as _, Measure, ScrollSource, Scrollbar, h_flex, v_flex};

use super::{Demos, section};
use crate::Overview;

pub struct Demo {
    rows: ScrollHandle,
    chips: ScrollHandle,
    grid: ScrollHandle,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            rows: ScrollHandle::new(),
            chips: ScrollHandle::new(),
            grid: ScrollHandle::new(),
        }
    }
}

/// How far an area is scrolled along `axis`, as its scrollbar reads it.
fn position(handle: &ScrollHandle, axis: Axis, cx: &gpui::App) -> String {
    match handle.measure(axis, cx) {
        Some(Measure { scrolled, most, .. }) if most > px(0.) => {
            format!(
                "Scrolled {:.0} of {:.0} px",
                f32::from(scrolled),
                f32::from(most)
            )
        }
        _ => "Nothing to scroll yet".to_string(),
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.scrollbar;
    let theme = cx.theme();
    let colors = theme.colors;
    let frame = |area: gpui::Div| {
        area.relative()
            .rounded(theme.radius)
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
    };
    let muted = |text: String| {
        div()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(text)
    };

    let rows = frame(div().w(px(360.)).h(px(240.)))
        .child(
            v_flex()
                .id("scrollbar-rows")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&demo.rows)
                .children((1..=40).map(|row| {
                    div()
                        .px_3()
                        .py_1p5()
                        .text_sm()
                        .border_b_1()
                        .border_color(colors.border)
                        .child(format!("Session {row}"))
                })),
        )
        .child(Scrollbar::vertical("scrollbar-rows-bar", demo.rows.clone()));

    let chips = frame(div().w(px(480.)).h(px(64.)))
        .child(
            h_flex()
                .id("scrollbar-chips")
                .size_full()
                .px_3()
                .gap_2()
                .overflow_x_scroll()
                .track_scroll(&demo.chips)
                .children((1..=30).map(|chip| {
                    div()
                        .flex_none()
                        .px_3()
                        .py_1()
                        .rounded_full()
                        .bg(colors.muted)
                        .text_sm()
                        .child(format!("model-{chip}"))
                })),
        )
        .child(Scrollbar::horizontal(
            "scrollbar-chips-bar",
            demo.chips.clone(),
        ));

    let grid = frame(div().w(px(420.)).h(px(260.)))
        .child(
            div()
                .id("scrollbar-grid")
                .size_full()
                .overflow_scroll()
                .track_scroll(&demo.grid)
                .child(
                    v_flex()
                        .w(px(1200.))
                        .p_3()
                        .gap_2()
                        .children((1..=30).map(|line| {
                            div()
                                .text_sm()
                                .whitespace_nowrap()
                                .child(format!(
                                    "{line:>2}  A line wider than the area, so it scrolls across as well as down, as a diff or a log does."
                                ))
                        })),
                ),
        )
        .child(Scrollbar::vertical("scrollbar-grid-down", demo.grid.clone()))
        .child(Scrollbar::horizontal("scrollbar-grid-across", demo.grid.clone()));

    vec![
        section(
            "Down",
            "The thumb shows how much is in view and where. Drag it, or press on the track to bring it there and go on dragging; the wheel scrolls through it.",
            v_flex()
                .gap_2()
                .child(rows)
                .child(muted(position(&demo.rows, Axis::Vertical, cx))),
            cx,
        ),
        section(
            "Across",
            "",
            v_flex()
                .gap_2()
                .child(chips)
                .child(muted(position(&demo.chips, Axis::Horizontal, cx))),
            cx,
        ),
        section(
            "Both ways",
            "",
            v_flex().gap_2().child(grid).child(muted(format!(
                "{} down · {} across",
                position(&demo.grid, Axis::Vertical, cx),
                position(&demo.grid, Axis::Horizontal, cx)
            ))),
            cx,
        ),
        section(
            "In a field and a list",
            "A textarea past its rows and a select's long list have one too, on their pages.",
            div(),
            cx,
        ),
    ]
}
