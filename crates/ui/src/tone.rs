//! What a message tells, which an alert and a toast say with an icon and a
//! color: news, a success, a warning, or a problem.

use gpui::Hsla;

use crate::icon::IconName;
use crate::theme::ThemeColors;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Info,
    Success,
    Warning,
    Danger,
}

impl Tone {
    pub(crate) fn icon(self) -> IconName {
        match self {
            Tone::Info => IconName::Info,
            Tone::Success => IconName::CircleCheck,
            Tone::Warning => IconName::TriangleAlert,
            Tone::Danger => IconName::CircleAlert,
        }
    }

    /// The color of its icon and title: news is in the text's own.
    pub(crate) fn color(self, colors: &ThemeColors) -> Hsla {
        match self {
            Tone::Info => colors.foreground,
            Tone::Success => colors.success,
            Tone::Warning => colors.warning,
            Tone::Danger => colors.danger,
        }
    }
}
