//! The components of rumbo's window, built on GPUI alone. Each module is
//! one component; `overview` shows them all.
//!
//! An application calls `init` once, gives GPUI `Assets`, and puts its
//! views in `root`.

mod alert;
mod assets;
mod breadcrumb;
mod button;
mod check_row;
mod checkbox;
mod drag;
mod floating_window;
mod focus;
mod fonts;
mod icon;
mod input;
mod label;
mod list;
mod overlay;
mod pagination;
mod popover;
mod radio;
mod resizable;
mod root;
mod roving;
mod scroll;
mod select;
mod size;
mod skeleton;
mod slider;
mod spinner;
mod switch;
mod table;
mod tabs;
mod text;
mod textarea;
mod theme;
mod toast;
mod toggle;
mod tone;
mod tooltip;

use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, ElementId, SharedString, Window};

pub use alert::Alert;
pub use assets::Assets;
pub use breadcrumb::{Breadcrumb, BreadcrumbItem};
pub use button::{Button, ButtonVariant};
pub use checkbox::Checkbox;
pub use floating_window::FloatingWindow;
pub use fonts::Typeface;
pub use icon::{Icon, IconName};
pub use input::Input;
pub use label::Label;
pub use list::{List, ListItem, ListMarker};
pub use overlay::{Dismiss, surface};
pub use pagination::Pagination;
pub use popover::{Placement, Popover, PopoverControl, PopoverTrigger};
pub use radio::{Radio, RadioGroup};
pub use resizable::{PanelSize, Resizable, ResizablePanel};
pub use root::{FocusNext, FocusPrevious, h_flex, root, v_flex};
pub use scroll::{Measure, ScrollSource, Scrollbar};
pub use select::{Select, SelectEvent, SelectItem, SelectState};
pub use size::{Sizable, Size};
pub use skeleton::Skeleton;
pub use slider::Slider;
pub use spinner::Spinner;
pub use switch::Switch;
pub use table::{SortDirection, Table, TableColumn, TableEvent, TableRow, TableState};
pub use tabs::{Tab, Tabs, TabsVariant};
pub use text::{InputEvent, InputState};
pub use textarea::Textarea;
pub use theme::{ActiveTheme, Theme, ThemeColors, ThemeMode};
pub use toast::{Toast, ToastId, Toaster, Toasts};
pub use toggle::{Toggle, ToggleGroup, ToggleVariant};
pub use tone::Tone;
pub use tooltip::Tooltip;

/// The actions of a field, which a view can bind other keys to, or catch
/// before the field does, as a composer catches Up to bring back what was
/// asked.
pub mod input_actions {
    pub use crate::text::{
        Backspace, Copy, Cut, Delete, DeleteWordLeft, DeleteWordRight, Enter, MoveDown, MoveEnd,
        MoveLeft, MoveLineEnd, MoveLineStart, MoveRight, MoveStart, MoveUp, MoveWordLeft,
        MoveWordRight, NewLine, Paste, Redo, SelectAll, SelectDown, SelectEnd, SelectLeft,
        SelectLineEnd, SelectLineStart, SelectRight, SelectStart, SelectUp, SelectWordLeft,
        SelectWordRight, Submit, Undo,
    };
}

/// Gives GPUI the bundled typefaces, puts the light theme in force unless
/// one is, and binds the components' keys. It fails when the platform does
/// not take the typefaces; the components then draw in the desktop's font,
/// and the rest is done.
pub fn init(cx: &mut App) -> gpui::Result<()> {
    theme::init(cx);
    root::bind_keys(cx);
    text::bind_keys(cx);
    overlay::bind_keys(cx);
    radio::bind_keys(cx);
    resizable::bind_keys(cx);
    select::bind_keys(cx);
    slider::bind_keys(cx);
    table::bind_keys(cx);
    tabs::bind_keys(cx);

    fonts::load(cx)
}

/// What is called with the value a control changes to. The caller keeps the
/// value and gives it back to the control on the next frame.
pub(crate) type ChangeHandler<T> = Rc<dyn Fn(&T, &mut Window, &mut App)>;

/// An id for a part of the element `id`, unique where `id` is.
pub(crate) fn child_id(id: &ElementId, part: impl Into<SharedString>) -> ElementId {
    ElementId::NamedChild(Arc::new(id.clone()), part.into())
}
