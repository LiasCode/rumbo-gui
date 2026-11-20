use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement as _, Pixels, SharedString, Styled as _,
    Window, div, px,
};
use ui::{ActiveTheme as _, PanelSize, Resizable, ResizablePanel, v_flex};

use super::{Demos, note, section, told};
use crate::Overview;

pub struct Demo {
    sidebar: Pixels,
    details: Pixels,
    terminal: Pixels,
    told: Vec<SharedString>,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            sidebar: px(220.),
            details: px(260.),
            terminal: px(120.),
            told: Vec::new(),
        }
    }
}

/// What a panel shows: its name, and how it is sized.
fn content(name: &str, size: Option<Pixels>, cx: &App) -> impl IntoElement {
    let said = match size {
        Some(size) => format!("{:.0} px", f32::from(size)),
        None => "takes the rest".to_string(),
    };

    v_flex()
        .size_full()
        .p_4()
        .gap_1()
        .child(div().text_sm().child(name.to_string()))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().colors.muted_foreground)
                .child(said),
        )
}

/// A frame of `height` for a group to fill.
fn frame(height: f32, cx: &App) -> gpui::Div {
    let theme = cx.theme();

    div()
        .w_full()
        .h(px(height))
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.colors.border)
        .overflow_hidden()
}

/// Keeps the size a divider gave a panel of the page.
fn resized(this: &mut Overview, resized: &PanelSize, cx: &mut Context<Overview>) {
    let demo = &mut this.demos.resizable;
    let kept = match resized.key.as_ref() {
        "sidebar" => &mut demo.sidebar,
        "details" => &mut demo.details,
        "terminal" => &mut demo.terminal,
        _ => return,
    };

    *kept = resized.size;
    note(
        &mut demo.told,
        format!("{} is {:.0} px", resized.key, f32::from(resized.size)),
    );
    cx.notify();
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.resizable;
    let quiet = cx.theme().colors.muted.opacity(0.4);

    let beside = frame(280., cx).child(
        Resizable::horizontal("beside")
            .child(
                ResizablePanel::new("sidebar")
                    .size(demo.sidebar)
                    .min(px(160.))
                    .max(px(360.))
                    .child(div().size_full().bg(quiet).child(content(
                        "Sidebar",
                        Some(demo.sidebar),
                        cx,
                    ))),
            )
            .child(
                ResizablePanel::new("work")
                    .min(px(200.))
                    .child(content("Work", None, cx)),
            )
            .child(
                ResizablePanel::new("details")
                    .size(demo.details)
                    .min(px(200.))
                    .max(px(420.))
                    .child(content("Details", Some(demo.details), cx)),
            )
            .on_resize(cx.listener(|this, size: &PanelSize, _, cx| resized(this, size, cx))),
    );

    let above = frame(320., cx).child(
        Resizable::vertical("above")
            .child(
                ResizablePanel::new("conversation")
                    .min(px(80.))
                    .child(content("Conversation", None, cx)),
            )
            .child(
                ResizablePanel::new("terminal")
                    .size(demo.terminal)
                    .min(px(60.))
                    .max(px(240.))
                    .child(div().size_full().bg(quiet).child(content(
                        "Terminal",
                        Some(demo.terminal),
                        cx,
                    ))),
            )
            .on_resize(cx.listener(|this, size: &PanelSize, _, cx| resized(this, size, cx))),
    );

    let still = frame(120., cx).child(
        Resizable::horizontal("still")
            .child(ResizablePanel::new("left").size(px(240.)).child(content(
                "Left",
                Some(px(240.)),
                cx,
            )))
            .child(ResizablePanel::new("right").child(content("Right", None, cx))),
    );

    vec![
        section(
            "Side by side",
            "Drag a divider, or Tab to it and move it with the arrows; Home and End take it as far as it goes. A sized panel stops at its least and its most, and the one that takes the rest keeps its own least.",
            beside,
            cx,
        ),
        section("One above another", "", above, cx),
        section(
            "Without a handler",
            "A group no one keeps the sizes of draws its dividers, which do not move.",
            still,
            cx,
        ),
        section("What they told", "", told(&demo.told, cx), cx),
    ]
}
