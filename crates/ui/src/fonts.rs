//! The typefaces the components draw with: two bundled, Montserrat and
//! Shantell Sans, under the SIL Open Font License in
//! `assets/fonts/*/OFL.txt`, and the desktop's own.
//!
//! The bundled ones are static files, one per weight, since GPUI on Linux
//! draws a variable font only at its default weight.

use std::borrow::Cow;

use gpui::{App, Result, SharedString};

/// What text is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Typeface {
    /// Geometric and plain, for work.
    #[default]
    Montserrat,
    /// Hand-drawn and friendly.
    ShantellSans,
    /// The desktop's interface font, as GPUI Kit draws.
    System,
}

impl Typeface {
    pub const ALL: [Typeface; 3] = [
        Typeface::Montserrat,
        Typeface::ShantellSans,
        Typeface::System,
    ];

    /// Its name for a person.
    pub fn name(self) -> &'static str {
        match self {
            Typeface::Montserrat => "Montserrat",
            Typeface::ShantellSans => "Shantell Sans",
            Typeface::System => "System",
        }
    }

    /// The family GPUI is asked for. Shantell Sans's static files name
    /// their family after the style they are of its axes.
    pub fn family(self) -> SharedString {
        match self {
            Typeface::Montserrat => "Montserrat".into(),
            Typeface::ShantellSans => "Shantell Sans Normal".into(),
            Typeface::System => ".SystemUIFont".into(),
        }
    }

    /// Whether GPUI has it: the desktop's always, a bundled one once `init`
    /// loaded it.
    pub fn is_loaded(self, cx: &App) -> bool {
        match self {
            Typeface::System => true,
            bundled => cx
                .text_system()
                .all_font_names()
                .iter()
                .any(|name| *name == bundled.family()),
        }
    }
}

/// Every weight the components use, of each bundled typeface: regular,
/// medium, semibold, and bold.
const FILES: [&[u8]; 8] = [
    include_bytes!("../assets/fonts/montserrat/Montserrat-Regular.ttf"),
    include_bytes!("../assets/fonts/montserrat/Montserrat-Medium.ttf"),
    include_bytes!("../assets/fonts/montserrat/Montserrat-SemiBold.ttf"),
    include_bytes!("../assets/fonts/montserrat/Montserrat-Bold.ttf"),
    include_bytes!("../assets/fonts/shantell-sans/Shantell_Sans-Normal-Regular.otf"),
    include_bytes!("../assets/fonts/shantell-sans/Shantell_Sans-Normal-Medium.otf"),
    include_bytes!("../assets/fonts/shantell-sans/Shantell_Sans-Normal-SemiBold.otf"),
    include_bytes!("../assets/fonts/shantell-sans/Shantell_Sans-Normal-Bold.otf"),
];

/// Gives GPUI the bundled typefaces.
pub(crate) fn load(cx: &App) -> Result<()> {
    let fonts = FILES.iter().map(|bytes| Cow::Borrowed(*bytes)).collect();

    cx.text_system().add_fonts(fonts)
}
