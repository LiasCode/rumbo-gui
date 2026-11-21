//! A box that is ticked or not, for a choice that is on or off and takes
//! effect when a form is sent. Its label and description press it too.

use gpui::{
    App, ElementId, IntoElement, ParentElement as _, Refineable as _, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
};

use crate::ChangeHandler;
use crate::check_row::{self, Words};
use crate::focus;
use crate::icon::{Icon, IconName};
use crate::size::{Sizable, Size, rems_from_px};
use crate::theme::ActiveTheme as _;

#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    words: Words,
    disabled: bool,
    size: Size,
    on_change: Option<ChangeHandler<bool>>,
    style: StyleRefinement,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Checkbox {
            id: id.into(),
            checked: false,
            words: Words::default(),
            disabled: false,
            size: Size::Medium,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.words.label = Some(label.into());
        self
    }

    /// What the choice means, under the label.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.words.description = Some(description.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the value the box is pressed to; the caller keeps it and
    /// gives it back with `checked`.
    pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::rc::Rc::new(handler));
        self
    }
}

impl Sizable for Checkbox {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Checkbox {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = focus::focus_of(&self.id, window, cx);
        let ringed = focus::ringed(&focus, window);
        let theme = cx.theme();
        let colors = theme.colors;

        let side = match self.size {
            Size::XSmall | Size::Small => rems_from_px(14.),
            Size::Medium | Size::Large => rems_from_px(16.),
        };

        // Its top lines up with the label's first line.
        let lead = (self.size.line_height() - side) / 2.;

        let control = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .mt(lead)
            .size(side)
            .rounded(rems_from_px(4.))
            .border_1()
            .map(|control| match self.checked {
                true => control
                    .bg(colors.primary)
                    .border_color(colors.primary)
                    .child(
                        Icon::new(IconName::Check)
                            .color(colors.primary_foreground)
                            .size(side - rems_from_px(4.)),
                    ),
                false => control.bg(colors.background).border_color(colors.input),
            })
            .when(self.disabled, |control| control.opacity(0.5))
            .when(ringed, |control| control.shadow(theme.focus_ring()));

        let mut row = check_row::row(
            self.id,
            &focus,
            control.into_any_element(),
            self.words,
            self.checked,
            self.disabled,
            self.size,
            self.on_change,
            cx,
        );

        row.style().refine(&self.style);
        row
    }
}
