//! How big a control is. Every control of one size has the same height, so
//! a button and a field of that size line up in a row.
//!
//! Lengths are in rems, so that the whole interface grows and shrinks with
//! the rem size a window sets, as a page does when a browser zooms. They
//! are written as the pixels they are at the default size, sixteen to a
//! rem.

use gpui::{Pixels, Rems, Window, px};

/// The pixels of a rem at the default size.
const REM: f32 = 16.;

/// A length of `px` pixels at the default size, which grows and shrinks
/// with the window's rem size.
pub const fn rems_from_px(px: f32) -> Rems {
    Rems(px / REM)
}

/// Draws everything in `window` at `factor` times its default size, as a
/// browser zooms a page, from its next frame on.
pub fn set_zoom(window: &mut Window, factor: f32) {
    window.set_rem_size(px(REM * factor));
    window.refresh();
}

/// What `px` pixels at the default size are at the window's size now, for
/// what is measured in pixels, as a pointer is.
pub(crate) fn scaled(px: f32, window: &Window) -> Pixels {
    rems_from_px(px).to_pixels(window.rem_size())
}

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
    pub fn height(self) -> Rems {
        match self {
            Size::XSmall => rems_from_px(24.),
            Size::Small => rems_from_px(32.),
            Size::Medium => rems_from_px(36.),
            Size::Large => rems_from_px(40.),
        }
    }

    /// The room between a control's edge and what it holds.
    pub fn padding_x(self) -> Rems {
        match self {
            Size::XSmall => rems_from_px(8.),
            Size::Small => rems_from_px(12.),
            Size::Medium => rems_from_px(16.),
            Size::Large => rems_from_px(20.),
        }
    }

    /// The size of its text.
    pub fn text(self) -> Rems {
        match self {
            Size::XSmall => rems_from_px(12.),
            Size::Small | Size::Medium => rems_from_px(14.),
            Size::Large => rems_from_px(16.),
        }
    }

    /// The height of a line of its text.
    pub fn line_height(self) -> Rems {
        match self {
            Size::XSmall => rems_from_px(16.),
            Size::Small | Size::Medium => rems_from_px(20.),
            Size::Large => rems_from_px(24.),
        }
    }

    /// The size of an icon in it.
    pub fn icon(self) -> Rems {
        match self {
            Size::XSmall => rems_from_px(12.),
            Size::Small => rems_from_px(14.),
            Size::Medium => rems_from_px(16.),
            Size::Large => rems_from_px(18.),
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
