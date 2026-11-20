//! A ring that turns while something is being done whose end is not known,
//! in the color of the text around it unless given one.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, Hsla, IntoElement, RenderOnce, SharedString, Styled as _,
    Transformation, Window, percentage, svg,
};

use crate::icon::IconName;
use crate::size::{Sizable, Size};

/// How long a turn takes.
const TURN: Duration = Duration::from_millis(800);

#[derive(IntoElement)]
pub struct Spinner {
    size: Size,
    color: Option<Hsla>,
}

impl Spinner {
    pub fn new() -> Self {
        Spinner {
            size: Size::Medium,
            color: None,
        }
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Spinner::new()
    }
}

impl Sizable for Spinner {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let color = self.color.unwrap_or_else(|| window.text_style().color);

        svg()
            .path(SharedString::new_static(IconName::LoaderCircle.path()))
            .flex_none()
            .size(self.size.icon())
            .text_color(color)
            .with_animation("spinner", Animation::new(TURN).repeat(), |ring, delta| {
                ring.with_transformation(Transformation::rotate(percentage(delta)))
            })
    }
}
