//! An icon, drawn in the color of the text around it unless given one.

use gpui::{
    App, Hsla, IntoElement, Refineable as _, RenderOnce, SharedString, StyleRefinement, Styled,
    Window, svg,
};

use crate::size::{Sizable, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    ArrowDown,
    ArrowUp,
    Check,
    ChevronDown,
    ChevronUp,
    ChevronsUpDown,
    Close,
    Copy,
    GitCompare,
    Info,
    Minus,
    PanelLeft,
    PanelRight,
    Plus,
    Search,
    Settings,
    Trash,
}

impl IconName {
    pub const ALL: [IconName; 17] = [
        IconName::ArrowDown,
        IconName::ArrowUp,
        IconName::Check,
        IconName::ChevronDown,
        IconName::ChevronUp,
        IconName::ChevronsUpDown,
        IconName::Close,
        IconName::Copy,
        IconName::GitCompare,
        IconName::Info,
        IconName::Minus,
        IconName::PanelLeft,
        IconName::PanelRight,
        IconName::Plus,
        IconName::Search,
        IconName::Settings,
        IconName::Trash,
    ];

    /// Where `Assets` keeps it.
    pub fn path(self) -> &'static str {
        match self {
            IconName::ArrowDown => "icons/arrow-down.svg",
            IconName::ArrowUp => "icons/arrow-up.svg",
            IconName::Check => "icons/check.svg",
            IconName::ChevronDown => "icons/chevron-down.svg",
            IconName::ChevronUp => "icons/chevron-up.svg",
            IconName::ChevronsUpDown => "icons/chevrons-up-down.svg",
            IconName::Close => "icons/x.svg",
            IconName::Copy => "icons/copy.svg",
            IconName::GitCompare => "icons/git-compare.svg",
            IconName::Info => "icons/info.svg",
            IconName::Minus => "icons/minus.svg",
            IconName::PanelLeft => "icons/panel-left.svg",
            IconName::PanelRight => "icons/panel-right.svg",
            IconName::Plus => "icons/plus.svg",
            IconName::Search => "icons/search.svg",
            IconName::Settings => "icons/settings.svg",
            IconName::Trash => "icons/trash.svg",
        }
    }
}

#[derive(IntoElement)]
pub struct Icon {
    name: IconName,
    size: Size,
    color: Option<Hsla>,
    style: StyleRefinement,
}

impl Icon {
    pub fn new(name: IconName) -> Self {
        Icon {
            name,
            size: Size::Medium,
            color: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl Sizable for Icon {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Icon {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Icon {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        // An svg takes its color from its own style, not from its parent.
        let color = self.color.unwrap_or_else(|| window.text_style().color);
        let mut icon = svg()
            .path(SharedString::new_static(self.name.path()))
            .flex_none()
            .size(self.size.icon())
            .text_color(color);

        icon.style().refine(&self.style);
        icon
    }
}

impl From<IconName> for Icon {
    fn from(name: IconName) -> Self {
        Icon::new(name)
    }
}
