//! A switch, for a setting that is on or off and takes effect at once. Its
//! label and description press it too, and its knob slides when it changes.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, ElementId, IntoElement, ParentElement as _, Refineable as _,
    RenderOnce, SharedString, StyleRefinement, Styled, Window, div, ease_out_quint,
    prelude::FluentBuilder as _, px,
};

use crate::check_row::{self, Words};
use crate::focus;
use crate::size::{Sizable, Size};
use crate::theme::{ActiveTheme as _, ThemeMode};
use crate::{ChangeHandler, child_id};

/// How long the knob takes to slide.
const SLIDE: Duration = Duration::from_millis(160);

#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    words: Words,
    disabled: bool,
    size: Size,
    on_change: Option<ChangeHandler<bool>>,
    style: StyleRefinement,
}

/// What a switch remembers between frames: the value it showed last, and
/// how many times it changed, which names the slide of each change.
struct Motion {
    shown: bool,
    changes: u64,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Switch {
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

    /// What the setting does, under the label.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.words.description = Some(description.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the value the switch is pressed to; the caller keeps it
    /// and gives it back with `checked`.
    pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::rc::Rc::new(handler));
        self
    }
}

impl Sizable for Switch {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Switch {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = focus::focus_of(&self.id, window, cx);
        let ringed = focus::ringed(&focus, window);

        // A switch shown for the first time does not slide; one that changed
        // since the frame before does, once.
        let motion = window.use_keyed_state(child_id(&self.id, "motion"), cx, |_, _| Motion {
            shown: self.checked,
            changes: 0,
        });
        let changes = motion.update(cx, |motion, _| {
            if motion.shown != self.checked {
                motion.shown = self.checked;
                motion.changes += 1;
            }

            motion.changes
        });

        let theme = cx.theme();
        let colors = theme.colors;

        let (width, height, knob) = match self.size {
            Size::XSmall | Size::Small => (px(28.), px(16.), px(12.)),
            Size::Medium | Size::Large => (px(32.), px(18.), px(14.)),
        };
        let inset = (height - knob) / 2.;
        let travel = width - knob - inset * 2.;

        let track = match self.checked {
            true => colors.primary,
            false => colors.switch,
        };
        let knob_color = match (self.checked, theme.mode) {
            (true, _) => colors.primary_foreground,
            (false, ThemeMode::Light) => colors.background,
            (false, ThemeMode::Dark) => colors.foreground,
        };

        // Where the knob rests for a value, from the track's left edge.
        let checked = self.checked;
        let rest = move |delta: f32| {
            let progress = match checked {
                true => delta,
                false => 1. - delta,
            };

            inset + travel * progress
        };

        let knob = div()
            .absolute()
            .top(inset)
            .size(knob)
            .rounded_full()
            .bg(knob_color)
            .shadow_sm();

        let knob = match changes {
            0 => knob.left(rest(1.)).into_any_element(),
            _ => knob
                .with_animation(
                    child_id(&self.id, format!("slide-{changes}")),
                    Animation::new(SLIDE).with_easing(ease_out_quint()),
                    move |knob, delta| knob.left(rest(delta)),
                )
                .into_any_element(),
        };

        // Its middle lines up with the label's first line.
        let lead = ((self.size.line_height() - height) / 2.).max(px(0.));

        let control = div()
            .relative()
            .flex_none()
            .mt(lead)
            .w(width)
            .h(height)
            .rounded_full()
            .bg(track)
            .child(knob)
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
