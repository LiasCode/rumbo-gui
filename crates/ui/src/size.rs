//! How big a control is. Every control of one size has the same height, so
//! a button and a field of that size line up in a row.

use gpui::{Pixels, px};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Size {
    XSmall,
    Small,
    #[default]
    Medium,
    Large,
}

impl Size {
    /// The height of a control.
    pub fn height(self) -> Pixels {
        match self {
            Size::XSmall => px(24.),
            Size::Small => px(32.),
            Size::Medium => px(36.),
            Size::Large => px(40.),
        }
    }

    /// The room between a control's edge and what it holds.
    pub fn padding_x(self) -> Pixels {
        match self {
            Size::XSmall => px(8.),
            Size::Small => px(12.),
            Size::Medium => px(16.),
            Size::Large => px(20.),
        }
    }

    /// The size of its text.
    pub fn text(self) -> Pixels {
        match self {
            Size::XSmall => px(12.),
            Size::Small | Size::Medium => px(14.),
            Size::Large => px(16.),
        }
    }

    /// The height of a line of its text.
    pub fn line_height(self) -> Pixels {
        match self {
            Size::XSmall => px(16.),
            Size::Small | Size::Medium => px(20.),
            Size::Large => px(24.),
        }
    }

    /// The size of an icon in it.
    pub fn icon(self) -> Pixels {
        match self {
            Size::XSmall => px(12.),
            Size::Small => px(14.),
            Size::Medium => px(16.),
            Size::Large => px(18.),
        }
    }
}

/// A component that comes in sizes.
pub trait Sizable: Sized {
    fn with_size(self, size: Size) -> Self;

    fn xsmall(self) -> Self {
        self.with_size(Size::XSmall)
    }

    fn small(self) -> Self {
        self.with_size(Size::Small)
    }

    fn large(self) -> Self {
        self.with_size(Size::Large)
    }
}
