//! A field of one line, drawing an `InputState`: with an icon or words
//! before or after the text, and a button that empties it.

use gpui::{
    AnyElement, App, Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Refineable as _, RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    div, prelude::FluentBuilder as _,
};

use crate::icon::{Icon, IconName};
use crate::size::{Sizable, Size};
use crate::text::{self, InputState, TextElement};
use crate::theme::ActiveTheme as _;

#[derive(IntoElement)]
pub struct Input {
    state: Entity<InputState>,
    size: Size,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
    cleanable: bool,
    bare: bool,
    style: StyleRefinement,
}

impl Input {
    pub fn new(state: &Entity<InputState>) -> Self {
        Input {
            state: state.clone(),
            size: Size::Medium,
            prefix: None,
            suffix: None,
            cleanable: false,
            bare: false,
            style: StyleRefinement::default(),
        }
    }

    /// What shows before the text, such as a search icon.
    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    /// What shows after the text, such as a unit.
    pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
        self.suffix = Some(suffix.into_any_element());
        self
    }

    /// A button that empties the field, while it holds something.
    pub fn cleanable(mut self, cleanable: bool) -> Self {
        self.cleanable = cleanable;
        self
    }

    /// No border or background, for a field set in a surface of its own.
    pub fn bare(mut self, bare: bool) -> Self {
        self.bare = bare;
        self
    }
}

impl Sizable for Input {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Input {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Input {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().colors.muted_foreground;
        let field = self.state.read(cx);
        let clearing = self.cleanable && !field.value().is_empty() && !field.is_disabled();
        let size = self.size;
        let state = self.state.clone();

        let padding = match size {
            Size::XSmall => gpui::px(6.),
            Size::Small => gpui::px(8.),
            Size::Medium | Size::Large => gpui::px(12.),
        };

        let mut input = text::frame(&self.state, self.bare, window, cx)
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .h(size.height())
            .px(padding)
            .text_size(size.text())
            .line_height(size.line_height())
            .when_some(self.prefix, |input, prefix| {
                input.child(div().flex_none().text_color(muted).child(prefix))
            })
            .child(TextElement::new(&self.state))
            .when(clearing, |input| {
                // Not a button: Tab goes past it, from the field to what
                // follows.
                input.child(
                    div()
                        .id(("clear", state.entity_id().as_u64()))
                        .flex_none()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_color(muted)
                        .hover(|style| style.opacity(0.7))
                        .child(Icon::new(IconName::Close).with_size(size))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(move |_, window, cx| {
                            state.update(cx, |state, cx| {
                                state.clear(window, cx);
                                state.focus(window, cx);
                            })
                        }),
                )
            })
            .when_some(self.suffix, |input, suffix| {
                input.child(div().flex_none().text_color(muted).child(suffix))
            });

        input.style().refine(&self.style);
        input
    }
}
