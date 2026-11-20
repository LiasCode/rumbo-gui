//! The pages of the overview, one per component, simplest first, and what
//! their examples hold between frames.

mod button;
mod checkbox;
mod floating_window;
mod input;
mod label;
mod popover;
mod select;
mod switch;
mod textarea;
mod tooltip;
mod typography;

use gpui::{
    AnyElement, App, Context, Div, FontWeight, IntoElement, ParentElement as _, SharedString,
    Styled as _, Window, div, prelude::FluentBuilder as _,
};
use ui::{ActiveTheme as _, h_flex, v_flex};

use crate::Overview;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Typography,
    Label,
    Button,
    Checkbox,
    Switch,
    Tooltip,
    Input,
    Textarea,
    Select,
    Popover,
    FloatingWindow,
}

impl Page {
    pub const ALL: [Page; 11] = [
        Page::Typography,
        Page::Label,
        Page::Button,
        Page::Checkbox,
        Page::Switch,
        Page::Tooltip,
        Page::Input,
        Page::Textarea,
        Page::Select,
        Page::Popover,
        Page::FloatingWindow,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Typography => "Typography",
            Page::Label => "Label",
            Page::Button => "Button",
            Page::Checkbox => "Checkbox",
            Page::Switch => "Switch",
            Page::Tooltip => "Tooltip",
            Page::Input => "Input",
            Page::Textarea => "Textarea",
            Page::Select => "Select",
            Page::Popover => "Popover",
            Page::FloatingWindow => "Floating window",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Page::Typography => "typography",
            Page::Label => "label",
            Page::Button => "button",
            Page::Checkbox => "checkbox",
            Page::Switch => "switch",
            Page::Tooltip => "tooltip",
            Page::Input => "input",
            Page::Textarea => "textarea",
            Page::Select => "select",
            Page::Popover => "popover",
            Page::FloatingWindow => "floating-window",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|page| page.slug() == slug)
    }

    pub fn about(self) -> &'static str {
        match self {
            Page::Typography => {
                "The typefaces the components draw with: two bundled, and the desktop's own. The one in force is chosen here or under the navigation, and every page takes it at once."
            }
            Page::Label => {
                "The name of a field or a setting, with words that matter less beside it."
            }
            Page::Button => {
                "Words, an icon, or both, in a look by how much the action matters. Tab reaches it, and Enter or Space presses it."
            }
            Page::Checkbox => "A choice that is on or off and takes effect when a form is sent.",
            Page::Switch => "A setting that is on or off and takes effect at once.",
            Page::Tooltip => "A word about a control, shown when the pointer rests on it.",
            Page::Input => {
                "A field of one line: typed, selected, copied, undone, and composed with an input method."
            }
            Page::Textarea => {
                "A field of several lines that grows with its text and scrolls past its rows."
            }
            Page::Select => {
                "A choice of one value from a list that opens under its trigger, and can be searched."
            }
            Page::Popover => {
                "A surface that opens beside its trigger and holds anything. Escape or a press outside closes it."
            }
            Page::FloatingWindow => {
                "A window inside the window, dragged by its title. A modal one dims the work behind it."
            }
        }
    }
}

/// What the examples hold between frames: each page's, in its module.
pub struct Demos {
    button: button::Demo,
    checkbox: checkbox::Demo,
    switch: switch::Demo,
    tooltip: tooltip::Demo,
    input: input::Demo,
    textarea: textarea::Demo,
    select: select::Demo,
    popover: popover::Demo,
    floating_window: floating_window::Demo,
}

impl Demos {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        Demos {
            button: button::Demo::default(),
            checkbox: checkbox::Demo::default(),
            switch: switch::Demo::default(),
            tooltip: tooltip::Demo::new(cx),
            input: input::Demo::new(window, cx),
            textarea: textarea::Demo::new(window, cx),
            select: select::Demo::new(window, cx),
            popover: popover::Demo::new(window, cx),
            floating_window: floating_window::Demo::new(window, cx),
        }
    }

    /// The sections of a page.
    pub fn render(
        &mut self,
        page: Page,
        window: &mut Window,
        cx: &mut Context<Overview>,
    ) -> Vec<AnyElement> {
        match page {
            Page::Typography => typography::render(self, window, cx),
            Page::Label => label::render(self, window, cx),
            Page::Button => button::render(self, window, cx),
            Page::Checkbox => checkbox::render(self, window, cx),
            Page::Switch => switch::render(self, window, cx),
            Page::Tooltip => tooltip::render(self, window, cx),
            Page::Input => input::render(self, window, cx),
            Page::Textarea => textarea::render(self, window, cx),
            Page::Select => select::render(self, window, cx),
            Page::Popover => popover::render(self, window, cx),
            Page::FloatingWindow => floating_window::render(self, window, cx),
        }
    }
}

/// One example of a page: what it shows, and the example in a frame.
fn section(title: &str, about: &str, example: impl IntoElement, cx: &App) -> AnyElement {
    let theme = cx.theme();

    v_flex()
        .gap_3()
        .child(
            v_flex()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_string()),
                )
                .when(!about.is_empty(), |heading| {
                    heading.child(
                        div()
                            .text_sm()
                            .text_color(theme.colors.muted_foreground)
                            .child(about.to_string()),
                    )
                }),
        )
        .child(
            div()
                .p_6()
                .rounded(theme.radius_lg)
                .border_1()
                .border_color(theme.colors.border)
                .child(example),
        )
        .into_any_element()
}

/// Examples side by side, wrapping when they do not fit.
fn row() -> Div {
    h_flex().flex_wrap().gap_3()
}

/// What the examples of a page told, newest first, or that they told
/// nothing yet.
fn told(lines: &[SharedString], cx: &App) -> AnyElement {
    let theme = cx.theme();

    v_flex()
        .gap_1()
        .min_h_16()
        .p_3()
        .rounded(theme.radius)
        .bg(theme.colors.muted)
        .text_xs()
        .font_family("monospace")
        .text_color(theme.colors.muted_foreground)
        .map(|column| match lines.is_empty() {
            true => column.child("Nothing yet."),
            false => column.children(lines.iter().rev().take(6).cloned()),
        })
        .into_any_element()
}

/// Keeps what an example told, for `told` to show.
fn note(lines: &mut Vec<SharedString>, line: impl Into<SharedString>) {
    lines.push(line.into());

    if lines.len() > 20 {
        lines.remove(0);
    }
}
