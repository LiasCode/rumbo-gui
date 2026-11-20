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
    BookOpen,
    Check,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronsUpDown,
    CircleAlert,
    CircleCheck,
    CircleStop,
    CircleX,
    Clock,
    Close,
    Copy,
    Cpu,
    Download,
    Ellipsis,
    ExternalLink,
    FilePen,
    FilePlus,
    FileText,
    FolderClosed,
    FolderOpen,
    GitBranch,
    GitCompare,
    Info,
    LoaderCircle,
    MessageSquare,
    Minus,
    PanelLeft,
    PanelLeftClose,
    PanelLeftOpen,
    PanelRight,
    Pencil,
    Plus,
    RefreshCw,
    RotateCcw,
    RotateCw,
    Search,
    Settings,
    Shrink,
    Sparkles,
    Square,
    SquarePen,
    SquareTerminal,
    Trash,
    TriangleAlert,
    Wrench,
}

impl IconName {
    pub const ALL: [IconName; 50] = [
        IconName::ArrowDown,
        IconName::ArrowUp,
        IconName::BookOpen,
        IconName::Check,
        IconName::ChevronDown,
        IconName::ChevronLeft,
        IconName::ChevronRight,
        IconName::ChevronUp,
        IconName::ChevronsUpDown,
        IconName::CircleAlert,
        IconName::CircleCheck,
        IconName::CircleStop,
        IconName::CircleX,
        IconName::Clock,
        IconName::Close,
        IconName::Copy,
        IconName::Cpu,
        IconName::Download,
        IconName::Ellipsis,
        IconName::ExternalLink,
        IconName::FilePen,
        IconName::FilePlus,
        IconName::FileText,
        IconName::FolderClosed,
        IconName::FolderOpen,
        IconName::GitBranch,
        IconName::GitCompare,
        IconName::Info,
        IconName::LoaderCircle,
        IconName::MessageSquare,
        IconName::Minus,
        IconName::PanelLeft,
        IconName::PanelLeftClose,
        IconName::PanelLeftOpen,
        IconName::PanelRight,
        IconName::Pencil,
        IconName::Plus,
        IconName::RefreshCw,
        IconName::RotateCcw,
        IconName::RotateCw,
        IconName::Search,
        IconName::Settings,
        IconName::Shrink,
        IconName::Sparkles,
        IconName::Square,
        IconName::SquarePen,
        IconName::SquareTerminal,
        IconName::Trash,
        IconName::TriangleAlert,
        IconName::Wrench,
    ];

    /// Where `Assets` keeps it.
    pub fn path(self) -> &'static str {
        match self {
            IconName::ArrowDown => "icons/arrow-down.svg",
            IconName::ArrowUp => "icons/arrow-up.svg",
            IconName::BookOpen => "icons/book-open.svg",
            IconName::Check => "icons/check.svg",
            IconName::ChevronDown => "icons/chevron-down.svg",
            IconName::ChevronLeft => "icons/chevron-left.svg",
            IconName::ChevronRight => "icons/chevron-right.svg",
            IconName::ChevronUp => "icons/chevron-up.svg",
            IconName::ChevronsUpDown => "icons/chevrons-up-down.svg",
            IconName::CircleAlert => "icons/circle-alert.svg",
            IconName::CircleCheck => "icons/circle-check.svg",
            IconName::CircleStop => "icons/circle-stop.svg",
            IconName::CircleX => "icons/circle-x.svg",
            IconName::Clock => "icons/clock.svg",
            IconName::Close => "icons/x.svg",
            IconName::Copy => "icons/copy.svg",
            IconName::Cpu => "icons/cpu.svg",
            IconName::Download => "icons/download.svg",
            IconName::Ellipsis => "icons/ellipsis.svg",
            IconName::ExternalLink => "icons/external-link.svg",
            IconName::FilePen => "icons/file-pen.svg",
            IconName::FilePlus => "icons/file-plus.svg",
            IconName::FileText => "icons/file-text.svg",
            IconName::FolderClosed => "icons/folder-closed.svg",
            IconName::FolderOpen => "icons/folder-open.svg",
            IconName::GitBranch => "icons/git-branch.svg",
            IconName::GitCompare => "icons/git-compare.svg",
            IconName::Info => "icons/info.svg",
            IconName::LoaderCircle => "icons/loader-circle.svg",
            IconName::MessageSquare => "icons/message-square.svg",
            IconName::Minus => "icons/minus.svg",
            IconName::PanelLeft => "icons/panel-left.svg",
            IconName::PanelLeftClose => "icons/panel-left-close.svg",
            IconName::PanelLeftOpen => "icons/panel-left-open.svg",
            IconName::PanelRight => "icons/panel-right.svg",
            IconName::Pencil => "icons/pencil.svg",
            IconName::Plus => "icons/plus.svg",
            IconName::RefreshCw => "icons/refresh-cw.svg",
            IconName::RotateCcw => "icons/rotate-ccw.svg",
            IconName::RotateCw => "icons/rotate-cw.svg",
            IconName::Search => "icons/search.svg",
            IconName::Settings => "icons/settings.svg",
            IconName::Shrink => "icons/shrink.svg",
            IconName::Sparkles => "icons/sparkles.svg",
            IconName::Square => "icons/square.svg",
            IconName::SquarePen => "icons/square-pen.svg",
            IconName::SquareTerminal => "icons/square-terminal.svg",
            IconName::Trash => "icons/trash.svg",
            IconName::TriangleAlert => "icons/triangle-alert.svg",
            IconName::Wrench => "icons/wrench.svg",
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
