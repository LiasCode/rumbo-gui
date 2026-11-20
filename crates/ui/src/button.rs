//! A button: words, an icon, or both, in one of a few looks by how much it
//! matters. It takes the keyboard with Tab, and Enter or Space presses it.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Refineable as _, RenderOnce, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
};

use crate::icon::{Icon, IconName};
use crate::size::{Sizable, Size};
use crate::theme::{ActiveTheme as _, ThemeColors};
use crate::tooltip::Tooltip;

pub(crate) type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// How much a button stands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// The one thing a view is for, such as sending.
    Primary,
    #[default]
    Secondary,
    /// One of several equal choices.
    Outline,
    /// An action that should not draw the eye, as in a toolbar.
    Ghost,
    /// What cannot be undone, such as deleting.
    Danger,
    /// Words that act, among other words.
    Link,
}

/// What a variant paints: its background at rest, under the pointer, and
/// pressed; its text; and its border.
struct Paint {
    background: Option<Hsla>,
    hover: Option<Hsla>,
    active: Option<Hsla>,
    text: Hsla,
    border: Option<Hsla>,
}

impl ButtonVariant {
    fn paint(self, colors: &ThemeColors, selected: bool) -> Paint {
        let paint = match self {
            ButtonVariant::Primary => Paint {
                background: Some(colors.primary),
                hover: Some(colors.primary_hover),
                active: Some(colors.primary_active),
                text: colors.primary_foreground,
                border: None,
            },
            ButtonVariant::Secondary => Paint {
                background: Some(colors.secondary),
                hover: Some(colors.secondary_hover),
                active: Some(colors.secondary_active),
                text: colors.secondary_foreground,
                border: None,
            },
            ButtonVariant::Outline => Paint {
                background: Some(colors.background),
                hover: Some(colors.accent),
                active: Some(colors.secondary_hover),
                text: colors.foreground,
                border: Some(colors.input),
            },
            ButtonVariant::Ghost => Paint {
                background: None,
                hover: Some(colors.accent),
                active: Some(colors.secondary_hover),
                text: colors.foreground,
                border: None,
            },
            ButtonVariant::Danger => Paint {
                background: Some(colors.danger),
                hover: Some(colors.danger_hover),
                active: Some(colors.danger_active),
                text: colors.danger_foreground,
                border: None,
            },
            ButtonVariant::Link => Paint {
                background: None,
                hover: None,
                active: None,
                text: colors.link,
                border: None,
            },
        };

        if !selected {
            return paint;
        }

        // A selected button looks pressed, and stays so under the pointer.
        Paint {
            background: paint.active.or(Some(colors.accent)),
            hover: paint.active.or(Some(colors.accent)),
            ..paint
        }
    }
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<IconName>,
    icon_end: bool,
    variant: ButtonVariant,
    size: Size,
    disabled: bool,
    selected: bool,
    tooltip: Option<SharedString>,
    shortcut: Option<SharedString>,
    on_click: Option<ClickHandler>,
    style: StyleRefinement,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Button {
            id: id.into(),
            label: None,
            icon: None,
            icon_end: false,
            variant: ButtonVariant::default(),
            size: Size::Medium,
            disabled: false,
            selected: false,
            tooltip: None,
            shortcut: None,
            on_click: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// An icon before the label, or alone: a button with an icon and no
    /// label is square.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The icon after the label rather than before it, as a button that goes
    /// forward has it.
    pub fn icon_end(mut self) -> Self {
        self.icon_end = true;
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    pub fn outline(self) -> Self {
        self.variant(ButtonVariant::Outline)
    }

    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    pub fn danger(self) -> Self {
        self.variant(ButtonVariant::Danger)
    }

    pub fn link(self) -> Self {
        self.variant(ButtonVariant::Link)
    }

    /// A button that cannot be pressed now, and says so by fading.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// A button that stays pressed, such as the choice made in a group or
    /// the trigger of what is open.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// What it does, shown when the pointer rests on it.
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// The keys that do the same, said in its tooltip.
    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Sizable for Button {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let paint = self.variant.paint(&theme.colors, self.selected);
        let square = self.label.is_none() && self.icon.is_some();
        let link = self.variant == ButtonVariant::Link;
        let ring = theme.focus_ring();
        let size = self.size;

        let mut button = div()
            .id(self.id)
            .flex()
            .flex_none()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(match size {
                Size::XSmall => gpui::px(4.),
                _ => gpui::px(8.),
            })
            .h(size.height())
            .rounded(theme.radius)
            .text_size(size.text())
            .font_weight(FontWeight::MEDIUM)
            .text_color(paint.text)
            .whitespace_nowrap()
            .map(|button| match square {
                true => button.w(size.height()),
                false => button.px(size.padding_x()),
            })
            .when_some(paint.background, |button, background| button.bg(background))
            .when_some(paint.border, |button, border| {
                button.border_1().border_color(border)
            })
            .when_some(self.icon.filter(|_| !self.icon_end), |button, icon| {
                button.child(Icon::new(icon).with_size(size).color(paint.text))
            })
            .when_some(self.label, |button, label| button.child(label))
            .when_some(self.icon.filter(|_| self.icon_end), |button, icon| {
                button.child(Icon::new(icon).with_size(size).color(paint.text))
            });

        if self.disabled {
            button = button.opacity(0.5).cursor_default();
        } else {
            button = button
                .cursor_pointer()
                .tab_index(0)
                .focus_visible(move |style| style.shadow(ring))
                .when_some(paint.hover, |button, hover| {
                    button.hover(move |style| style.bg(hover))
                })
                .when_some(paint.active, |button, active| {
                    button.active(move |style| style.bg(active))
                })
                .when(link, |button| {
                    let hover = theme.colors.link_hover;

                    button.hover(move |style| style.underline().text_color(hover))
                })
                .when_some(self.on_click, |button, handler| {
                    button.on_click(move |event, window, cx| handler(event, window, cx))
                });
        }

        if let Some(text) = self.tooltip {
            let shortcut = self.shortcut;

            button = button.tooltip(move |_, cx| {
                let tooltip = Tooltip::new(text.clone());

                match &shortcut {
                    Some(keys) => tooltip.keys(keys.clone()).build(cx),
                    None => tooltip.build(cx),
                }
            });
        }

        button.style().refine(&self.style);
        button
    }
}
