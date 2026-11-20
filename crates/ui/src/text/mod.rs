//! Editable text: the buffer every edit goes through, the state of a field,
//! and the element that lays it out and paints it.

pub(crate) mod edit;
mod element;
mod frame;
mod state;

pub(crate) use element::TextElement;
pub(crate) use frame::frame;
pub(crate) use state::bind_keys;
pub use state::{
    Backspace, Copy, Cut, Delete, DeleteWordLeft, DeleteWordRight, Enter, InputEvent, InputState,
    MoveDown, MoveEnd, MoveLeft, MoveLineEnd, MoveLineStart, MoveRight, MoveStart, MoveUp,
    MoveWordLeft, MoveWordRight, NewLine, Paste, Redo, SelectAll, SelectDown, SelectEnd,
    SelectLeft, SelectLineEnd, SelectLineStart, SelectRight, SelectStart, SelectUp, SelectWordLeft,
    SelectWordRight, Submit, Undo,
};
