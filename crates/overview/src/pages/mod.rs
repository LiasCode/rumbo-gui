//! The pages of the overview, one per component, simplest first, and what
//! their examples hold between frames.

mod alert;
mod breadcrumb;
mod button;
mod checkbox;
mod command_palette;
mod dialog;
mod diff;
mod floating_window;
mod input;
mod label;
mod list;
mod markdown;
mod menu;
mod message;
mod message_list;
mod meter;
mod pagination;
mod popover;
mod radio;
mod resizable;
mod scrollbar;
mod select;
mod settings;
mod sidebar;
mod skeleton;
mod slider;
mod spinner;
mod switch;
mod table;
mod tabs;
mod textarea;
mod toast;
mod toggle;
mod tooltip;
mod typography;
mod window;

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
    List,
    Button,
    Toggle,
    Checkbox,
    Radio,
    Switch,
    Slider,
    Spinner,
    Skeleton,
    Meter,
    Tooltip,
    Scrollbar,
    Alert,
    Breadcrumb,
    Tabs,
    Pagination,
    Input,
    Textarea,
    Select,
    Popover,
    Menu,
    Dialog,
    CommandPalette,
    Toast,
    Message,
    Markdown,
    MessageList,
    Sidebar,
    Settings,
    Resizable,
    Table,
    Diff,
    FloatingWindow,
    Window,
}

impl Page {
    pub const ALL: [Page; 36] = [
        Page::Typography,
        Page::Label,
        Page::List,
        Page::Button,
        Page::Toggle,
        Page::Checkbox,
        Page::Radio,
        Page::Switch,
        Page::Slider,
        Page::Spinner,
        Page::Skeleton,
        Page::Meter,
        Page::Tooltip,
        Page::Scrollbar,
        Page::Alert,
        Page::Breadcrumb,
        Page::Tabs,
        Page::Pagination,
        Page::Input,
        Page::Textarea,
        Page::Select,
        Page::Popover,
        Page::Menu,
        Page::Dialog,
        Page::CommandPalette,
        Page::Toast,
        Page::Message,
        Page::Markdown,
        Page::MessageList,
        Page::Sidebar,
        Page::Settings,
        Page::Resizable,
        Page::Table,
        Page::Diff,
        Page::FloatingWindow,
        Page::Window,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Typography => "Typography",
            Page::Label => "Label",
            Page::List => "List",
            Page::Button => "Button",
            Page::Toggle => "Toggle",
            Page::Checkbox => "Checkbox",
            Page::Radio => "Radio",
            Page::Switch => "Switch",
            Page::Slider => "Slider",
            Page::Spinner => "Spinner",
            Page::Skeleton => "Skeleton",
            Page::Meter => "Meter",
            Page::Tooltip => "Tooltip",
            Page::Scrollbar => "Scrollbar",
            Page::Alert => "Alert",
            Page::Breadcrumb => "Breadcrumb",
            Page::Tabs => "Tabs",
            Page::Pagination => "Pagination",
            Page::Input => "Input",
            Page::Textarea => "Textarea",
            Page::Select => "Select",
            Page::Popover => "Popover",
            Page::Menu => "Menu",
            Page::Dialog => "Dialog",
            Page::CommandPalette => "Command palette",
            Page::Toast => "Toast",
            Page::Message => "Message",
            Page::Markdown => "Markdown",
            Page::MessageList => "Message list",
            Page::Sidebar => "Sidebar",
            Page::Settings => "Settings",
            Page::Resizable => "Resizable",
            Page::Table => "Table",
            Page::Diff => "Diff",
            Page::FloatingWindow => "Floating window",
            Page::Window => "Window",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Page::Typography => "typography",
            Page::Label => "label",
            Page::List => "list",
            Page::Button => "button",
            Page::Toggle => "toggle",
            Page::Checkbox => "checkbox",
            Page::Radio => "radio",
            Page::Switch => "switch",
            Page::Slider => "slider",
            Page::Spinner => "spinner",
            Page::Skeleton => "skeleton",
            Page::Meter => "meter",
            Page::Tooltip => "tooltip",
            Page::Scrollbar => "scrollbar",
            Page::Alert => "alert",
            Page::Breadcrumb => "breadcrumb",
            Page::Tabs => "tabs",
            Page::Pagination => "pagination",
            Page::Input => "input",
            Page::Textarea => "textarea",
            Page::Select => "select",
            Page::Popover => "popover",
            Page::Menu => "menu",
            Page::Dialog => "dialog",
            Page::CommandPalette => "command-palette",
            Page::Toast => "toast",
            Page::Message => "message",
            Page::Markdown => "markdown",
            Page::MessageList => "message-list",
            Page::Sidebar => "sidebar",
            Page::Settings => "settings",
            Page::Resizable => "resizable",
            Page::Table => "table",
            Page::Diff => "diff",
            Page::FloatingWindow => "floating-window",
            Page::Window => "window",
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
            Page::Toggle => {
                "A button that stays pressed or not, for a mode that is on or off, such as a panel shown; alone or in a group."
            }
            Page::List => {
                "Items one under another after a mark: a number, a letter, a numeral, a bullet, a dash, or a box."
            }
            Page::Checkbox => "A choice that is on or off and takes effect when a form is sent.",
            Page::Radio => "A choice of one option among a few, all in sight.",
            Page::Switch => "A setting that is on or off and takes effect at once.",
            Page::Slider => "A value between two ends, in steps, chosen along a track.",
            Page::Spinner => "A ring that turns while something is done whose end is not known.",
            Page::Skeleton => "A shape that breathes where something will show once it is read.",
            Page::Meter => "How much of a whole is used, as a bar filled that far.",
            Page::Tooltip => "A word about a control, shown when the pointer rests on it.",
            Page::Scrollbar => {
                "Along the edge of what scrolls: where the view is in it, and how much of it shows. It shows only while something is out of view."
            }
            Page::Alert => {
                "A message that stays in the page beside what it is about: news, a success, a warning, or a problem."
            }
            Page::Breadcrumb => {
                "Where something is, as the places that lead to it, each but the last a way back."
            }
            Page::Tabs => "A row of names of which one is shown, for the parts of one view.",
            Page::Pagination => "Pages of something long, one shown at a time.",
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
            Page::Menu => {
                "What can be done, under a button or where the pointer pressed with its other button."
            }
            Page::Dialog => "A question over the work, which waits for an answer.",
            Page::CommandPalette => {
                "Commands in groups, found by typing part of their name, and chosen with the keyboard."
            }
            Page::Markdown => {
                "What a model writes, drawn: code colored by its syntax, formulas, diagrams, and images, and each block's words can be selected and copied."
            }
            Page::MessageList => {
                "A long conversation, of which only the rows in view are drawn, that keeps to its end."
            }
            Page::Settings => "Settings in pages, with a search that finds them by their words.",
            Page::Diff => {
                "The changes a patch makes, file by file, each line on green where added and red where taken out."
            }
            Page::Message => "A message in a conversation, on a quiet ground, with words over it.",
            Page::Sidebar => {
                "The column beside the work: a head, groups of rows that hold rows, and a foot."
            }
            Page::Toast => {
                "A short message in a corner of the window that says what happened, and goes after a while."
            }
            Page::Resizable => {
                "Panels side by side or one above another, with dividers dragged to share the room between them."
            }
            Page::Table => {
                "Rows of cells under titled columns, which sort and widen; a row is chosen and opened."
            }
            Page::FloatingWindow => {
                "A window inside the window, dragged by its title. A modal one dims the work behind it."
            }
            Page::Window => {
                "What a window that draws its own frame has: its title bar, its frame, and a status bar."
            }
        }
    }
}

/// What the examples hold between frames: each page's, in its module.
pub struct Demos {
    button: button::Demo,
    alert: alert::Demo,
    menu: menu::Demo,
    dialog: dialog::Demo,
    sidebar: sidebar::Demo,
    radio: radio::Demo,
    breadcrumb: breadcrumb::Demo,
    pagination: pagination::Demo,
    tabs: tabs::Demo,
    toast: toast::Demo,
    toggle: toggle::Demo,
    checkbox: checkbox::Demo,
    switch: switch::Demo,
    slider: slider::Demo,
    tooltip: tooltip::Demo,
    scrollbar: scrollbar::Demo,
    input: input::Demo,
    textarea: textarea::Demo,
    select: select::Demo,
    popover: popover::Demo,
    resizable: resizable::Demo,
    table: table::Demo,
    floating_window: floating_window::Demo,
    command_palette: command_palette::Demo,
    message_list: message_list::Demo,
    settings: settings::Demo,
    diff: diff::Demo,
}

impl Demos {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        Demos {
            button: button::Demo::default(),
            alert: alert::Demo::default(),
            menu: menu::Demo::default(),
            dialog: dialog::Demo::new(window, cx),
            sidebar: sidebar::Demo::new(window, cx),
            radio: radio::Demo::default(),
            breadcrumb: breadcrumb::Demo::default(),
            pagination: pagination::Demo::default(),
            tabs: tabs::Demo::default(),
            toast: toast::Demo::default(),
            toggle: toggle::Demo::default(),
            checkbox: checkbox::Demo::default(),
            switch: switch::Demo::default(),
            slider: slider::Demo::default(),
            tooltip: tooltip::Demo::new(cx),
            scrollbar: scrollbar::Demo::default(),
            input: input::Demo::new(window, cx),
            textarea: textarea::Demo::new(window, cx),
            select: select::Demo::new(window, cx),
            popover: popover::Demo::new(window, cx),
            resizable: resizable::Demo::default(),
            table: table::Demo::new(cx),
            floating_window: floating_window::Demo::new(window, cx),
            command_palette: command_palette::Demo::new(window, cx),
            message_list: message_list::Demo::new(cx),
            settings: settings::Demo::new(window, cx),
            diff: diff::Demo::new(cx),
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
            Page::List => list::render(self, window, cx),
            Page::Button => button::render(self, window, cx),
            Page::Toggle => toggle::render(self, window, cx),
            Page::Checkbox => checkbox::render(self, window, cx),
            Page::Radio => radio::render(self, window, cx),
            Page::Switch => switch::render(self, window, cx),
            Page::Slider => slider::render(self, window, cx),
            Page::Spinner => spinner::render(self, window, cx),
            Page::Skeleton => skeleton::render(self, window, cx),
            Page::Meter => meter::render(self, window, cx),
            Page::Tooltip => tooltip::render(self, window, cx),
            Page::Scrollbar => scrollbar::render(self, window, cx),
            Page::Alert => alert::render(self, window, cx),
            Page::Breadcrumb => breadcrumb::render(self, window, cx),
            Page::Tabs => tabs::render(self, window, cx),
            Page::Pagination => pagination::render(self, window, cx),
            Page::Input => input::render(self, window, cx),
            Page::Textarea => textarea::render(self, window, cx),
            Page::Select => select::render(self, window, cx),
            Page::Popover => popover::render(self, window, cx),
            Page::Menu => menu::render(self, window, cx),
            Page::Dialog => dialog::render(self, window, cx),
            Page::CommandPalette => command_palette::render(self, window, cx),
            Page::Markdown => markdown::render(self, window, cx),
            Page::MessageList => message_list::render(self, window, cx),
            Page::Settings => settings::render(self, window, cx),
            Page::Diff => diff::render(self, window, cx),
            Page::Toast => toast::render(self, window, cx),
            Page::Message => message::render(self, window, cx),
            Page::Sidebar => sidebar::render(self, window, cx),
            Page::Resizable => resizable::render(self, window, cx),
            Page::Table => table::render(self, window, cx),
            Page::FloatingWindow => floating_window::render(self, window, cx),
            Page::Window => window::render(self, window, cx),
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
        .font_family(theme.mono_family.clone())
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
