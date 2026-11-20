//! The typefaces the components draw with: two bundled, Montserrat and
//! Shantell Sans, under the SIL Open Font License in
//! `assets/fonts/*/OFL.txt`, and the desktop's own.
//!
//! The bundled ones are static files, one per weight, since GPUI on Linux
//! draws a variable font only at its default weight.

use std::borrow::Cow;
use std::sync::OnceLock;

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

/// The families code is drawn in, the platform's own first.
const MONO: &[&str] = if cfg!(target_os = "macos") {
    &["Menlo", "Monaco", "Courier New"]
} else if cfg!(target_os = "windows") {
    &["Consolas", "Cascadia Mono", "Courier New"]
} else {
    &[
        "DejaVu Sans Mono",
        "Noto Sans Mono",
        "Liberation Mono",
        "Ubuntu Mono",
    ]
};

/// What GPUI always has, the desktop's interface font.
pub(crate) const FALLBACK: &str = ".SystemUIFont";

/// The first of `candidates` the machine has, or the fallback: GPUI cannot
/// lay out a line in a family it does not have.
fn first_installed(candidates: &[&str], installed: &[String]) -> SharedString {
    candidates
        .iter()
        .find(|candidate| installed.iter().any(|name| name == *candidate))
        .copied()
        .unwrap_or(FALLBACK)
        .into()
}

/// The family code is drawn in on this machine, found once, since listing
/// its fonts takes a while and they do not change while it runs.
pub(crate) fn mono(cx: &App) -> SharedString {
    static FOUND: OnceLock<SharedString> = OnceLock::new();

    FOUND
        .get_or_init(|| first_installed(MONO, &cx.text_system().all_font_names()))
        .clone()
}

/// Gives GPUI the bundled typefaces.
pub(crate) fn load(cx: &App) -> Result<()> {
    let fonts = FILES.iter().map(|bytes| Cow::Borrowed(*bytes)).collect();

    cx.text_system().add_fonts(fonts)
}

#[cfg(test)]
mod tests {
    use super::{FALLBACK, first_installed};

    #[test]
    fn draws_code_in_the_first_family_the_machine_has() {
        let installed = ["Noto Sans Mono".to_string(), "Liberation Mono".to_string()];

        assert_eq!(
            first_installed(
                &["DejaVu Sans Mono", "Noto Sans Mono", "Liberation Mono"],
                &installed
            ),
            "Noto Sans Mono"
        );
        assert_eq!(first_installed(&["Menlo"], &installed), FALLBACK);
    }
}
