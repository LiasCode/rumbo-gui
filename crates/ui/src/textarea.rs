//! A field of several lines, drawing an `InputState` made with
//! `multi_line`: it grows with its text between the rows it was given, and
//! scrolls past them.

use gpui::{
    App, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Refineable as _,
    RenderOnce, StyleRefinement, Styled, Window,
};

use crate::size::{Sizable, Size};
use crate::text::{self, InputState, TextElement};

#[derive(IntoElement)]
pub struct Textarea {
    state: Entity<InputState>,
    size: Size,
    bare: bool,
    style: StyleRefinement,
}

impl Textarea {
    pub fn new(state: &Entity<InputState>) -> Self {
        Textarea {
            state: state.clone(),
            size: Size::Medium,
            bare: false,
            style: StyleRefinement::default(),
        }
    }

    /// No border or background, for a field set in a surface of its own,
    /// as a composer is.
    pub fn bare(mut self, bare: bool) -> Self {
        self.bare = bare;
        self
    }
}

impl Sizable for Textarea {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Textarea {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Textarea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let size = self.size;

        let mut textarea = text::frame(&self.state, self.bare, window, cx)
            .flex()
            .flex_col()
            .w_full()
            .px(match size {
                Size::XSmall | Size::Small => gpui::px(8.),
                Size::Medium | Size::Large => gpui::px(12.),
            })
            .py(match size {
                Size::XSmall | Size::Small => gpui::px(4.),
                Size::Medium | Size::Large => gpui::px(8.),
            })
            .text_size(size.text())
            .line_height(size.line_height())
            .on_scroll(window.listener_for(&self.state, InputState::on_scroll))
            .child(TextElement::new(&self.state));

        textarea.style().refine(&self.style);
        textarea
    }
}
