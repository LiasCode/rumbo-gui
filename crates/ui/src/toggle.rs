//! A button that stays pressed or not, for a mode that is on or off, such
//! as a panel shown; and a group of them, where several can be pressed at
//! once or at most one. Tab reaches each, and Enter or Space presses it.

use std::rc::Rc;

use gpui::{
    App, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Refineable as _, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, Window, div, prelude::FluentBuilder as _, px,
};

use crate::ChangeHandler;
use crate::icon::{Icon, IconName};
use crate::root::h_flex;
use crate::size::{Sizable, Size};
use crate::theme::ActiveTheme as _;
use crate::tooltip::Tooltip;

/// How a toggle looks while it is not pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToggleVariant {
    /// Nothing but what it holds, as in a toolbar.
    #[default]
    Ghost,
    /// Within a border; a group of them is joined into one.
    Outline,
}

/// Where a toggle stands in a group joined into one, which says which of
/// its corners are rounded and which edges it draws: each draws its right
/// edge, and the first its left one too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Join {
    First,
    Middle,
    Last,
}

#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<IconName>,
    checked: bool,
    variant: ToggleVariant,
    size: Size,
    disabled: bool,
    tooltip: Option<SharedString>,
    shortcut: Option<SharedString>,
    on_change: Option<ChangeHandler<bool>>,
    join: Option<Join>,
    style: StyleRefinement,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Toggle {
            id: id.into(),
            label: None,
            icon: None,
            checked: false,
            variant: ToggleVariant::default(),
            size: Size::Medium,
            disabled: false,
            tooltip: None,
            shortcut: None,
            on_change: None,
            join: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// An icon before the label, or alone: a toggle with an icon and no
    /// label is square.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Whether it is pressed.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn variant(mut self, variant: ToggleVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn outline(self) -> Self {
        self.variant(ToggleVariant::Outline)
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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

    /// Called with the value it is pressed to; the caller keeps it and
    /// gives it back with `checked`.
    pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Sizable for Toggle {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Toggle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Toggle {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = theme.colors;
        let size = self.size;
        let radius = theme.radius;
        let ring = theme.focus_ring();
        let square = self.label.is_none() && self.icon.is_some();
        let outline = self.variant == ToggleVariant::Outline;

        // Pressed, it looks as a selected button does, and stays so under
        // the pointer.
        let background = match (self.checked, outline) {
            (true, _) => Some(colors.secondary_hover),
            (false, true) => Some(colors.background),
            (false, false) => None,
        };
        let hover = match self.checked {
            true => colors.secondary_hover,
            false => colors.accent,
        };

        let padding = match size {
            Size::XSmall => px(4.),
            Size::Small => px(6.),
            Size::Medium => px(8.),
            Size::Large => px(10.),
        };

        let mut toggle = div()
            .id(self.id)
            .flex()
            .flex_none()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(match size {
                Size::XSmall => px(4.),
                _ => px(8.),
            })
            .h(size.height())
            .min_w(size.height())
            .when(!square, |toggle| toggle.px(padding))
            .text_size(size.text())
            .font_weight(FontWeight::MEDIUM)
            .text_color(colors.foreground)
            .whitespace_nowrap()
            .when_some(background, |toggle, background| toggle.bg(background))
            .map(|toggle| match self.join {
                None => toggle.rounded(radius),
                Some(Join::First) => toggle.rounded_l(radius),
                Some(Join::Middle) => toggle,
                Some(Join::Last) => toggle.rounded_r(radius),
            })
            .when(outline, |toggle| {
                let toggle = toggle.border_color(colors.input);

                match self.join {
                    None | Some(Join::First) => toggle.border_1(),
                    Some(Join::Middle | Join::Last) => {
                        toggle.border_t_1().border_r_1().border_b_1()
                    }
                }
            })
            .when_some(self.icon, |toggle, icon| {
                toggle.child(Icon::new(icon).with_size(size))
            })
            .when_some(self.label, |toggle, label| toggle.child(label));

        if self.disabled {
            toggle = toggle.opacity(0.5).cursor_default();
        } else {
            let checked = self.checked;

            toggle = toggle
                .cursor_pointer()
                .tab_index(0)
                .focus_visible(move |style| style.shadow(ring))
                .hover(move |style| style.bg(hover))
                .when_some(self.on_change, |toggle, on_change| {
                    toggle.on_click(move |_, window, cx| on_change(&!checked, window, cx))
                });
        }

        if let Some(text) = self.tooltip {
            let shortcut = self.shortcut;

            toggle = toggle.tooltip(move |_, cx| {
                let tooltip = Tooltip::new(text.clone());

                match &shortcut {
                    Some(keys) => tooltip.keys(keys.clone()).build(cx),
                    None => tooltip.build(cx),
                }
            });
        }

        toggle.style().refine(&self.style);
        toggle
    }
}

/// Toggles side by side, of one look and size. Each is pressed on its own,
/// or, in a group of one choice, pressing one lets go of the others, and
/// pressing the one pressed lets go of it too, so none can be.
#[derive(IntoElement)]
pub struct ToggleGroup {
    items: Vec<Toggle>,
    single: bool,
    variant: ToggleVariant,
    size: Size,
    disabled: bool,
    on_change: Option<ChangeHandler<[bool]>>,
    style: StyleRefinement,
}

impl ToggleGroup {
    pub fn new() -> Self {
        ToggleGroup {
            items: Vec::new(),
            single: false,
            variant: ToggleVariant::default(),
            size: Size::Medium,
            disabled: false,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// A toggle of the group, which takes the group's look, size, and
    /// handler in place of its own.
    pub fn child(mut self, toggle: Toggle) -> Self {
        self.items.push(toggle);
        self
    }

    pub fn children(mut self, toggles: impl IntoIterator<Item = Toggle>) -> Self {
        self.items.extend(toggles);
        self
    }

    /// At most one is pressed at a time.
    pub fn single(mut self) -> Self {
        self.single = true;
        self
    }

    pub fn variant(mut self, variant: ToggleVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Within one border, joined.
    pub fn outline(self) -> Self {
        self.variant(ToggleVariant::Outline)
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with whether each toggle is pressed after one was; the
    /// caller keeps it and gives it back with each toggle's `checked`.
    pub fn on_change(mut self, handler: impl Fn(&[bool], &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Default for ToggleGroup {
    fn default() -> Self {
        ToggleGroup::new()
    }
}

impl Sizable for ToggleGroup {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for ToggleGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ToggleGroup {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let checks: Rc<[bool]> = self.items.iter().map(|item| item.checked).collect();
        let joined = self.variant == ToggleVariant::Outline && self.items.len() > 1;
        let last = self.items.len().saturating_sub(1);

        let items = self.items.into_iter().enumerate().map(|(index, item)| {
            let mut item = Toggle {
                variant: self.variant,
                size: self.size,
                disabled: item.disabled || self.disabled,
                on_change: None,
                ..item
            };

            if joined {
                item.join = Some(match index {
                    0 => Join::First,
                    _ if index == last => Join::Last,
                    _ => Join::Middle,
                });
            }

            match &self.on_change {
                Some(on_change) => {
                    let on_change = on_change.clone();
                    let checks = checks.clone();
                    let single = self.single;

                    item.on_change(move |_, window, cx| {
                        on_change(&pressed(&checks, index, single), window, cx)
                    })
                }
                None => item,
            }
        });

        let mut group = h_flex()
            .flex_none()
            .when(!joined, |group| group.gap_1())
            .children(items);

        group.style().refine(&self.style);
        group
    }
}

/// What a group holds after its toggle `index` is pressed: that one
/// changes, and in a group of one choice the others are let go.
fn pressed(checks: &[bool], index: usize, single: bool) -> Vec<bool> {
    checks
        .iter()
        .enumerate()
        .map(|(at, checked)| match (at == index, single) {
            (true, _) => !checked,
            (false, true) => false,
            (false, false) => *checked,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::pressed;

    #[test]
    fn presses_each_on_its_own() {
        assert_eq!(
            pressed(&[false, true, false], 0, false),
            [true, true, false]
        );
        assert_eq!(
            pressed(&[false, true, false], 1, false),
            [false, false, false]
        );
    }

    #[test]
    fn keeps_at_most_one_of_one_choice() {
        assert_eq!(
            pressed(&[false, true, false], 2, true),
            [false, false, true]
        );

        // The one pressed lets go, and none is.
        assert_eq!(
            pressed(&[false, true, false], 1, true),
            [false, false, false]
        );

        // A group given more than one pressed is left with one.
        assert_eq!(pressed(&[true, true, false], 2, true), [false, false, true]);
    }
}
