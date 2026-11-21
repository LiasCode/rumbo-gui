//! How much of a whole is used, as a bar filled that far: the context a
//! model holds, a disk, a quota. It fills in the primary color, or in a
//! tone that says how things stand, as a bar nearly full does.

use gpui::{
    App, IntoElement, ParentElement as _, Refineable as _, RenderOnce, StyleRefinement, Styled,
    Window, div, relative,
};

use crate::size::rems_from_px;
use crate::theme::ActiveTheme as _;
use crate::tone::Tone;

#[derive(IntoElement)]
pub struct Meter {
    /// The share used, from 0 to 1.
    value: f32,
    tone: Option<Tone>,
    style: StyleRefinement,
}

impl Meter {
    /// A meter filled to `value`, a share from 0 to 1; what is outside
    /// those is taken as the nearest of them.
    pub fn new(value: f32) -> Self {
        Meter {
            value: if value.is_nan() {
                0.
            } else {
                value.clamp(0., 1.)
            },
            tone: None,
            style: StyleRefinement::default(),
        }
    }

    /// Fills it in the color of a tone instead of the primary one.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }
}

impl Styled for Meter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Meter {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors;
        let fill = match self.tone {
            None | Some(Tone::Info) => colors.primary,
            Some(tone) => tone.color(&colors),
        };

        let mut track = div()
            .flex_none()
            .w_full()
            .h(rems_from_px(6.))
            .rounded_full()
            .overflow_hidden()
            .bg(colors.muted)
            .child(
                div()
                    .h_full()
                    .w(relative(self.value))
                    .rounded_full()
                    .bg(fill),
            );

        track.style().refine(&self.style);
        track
    }
}
