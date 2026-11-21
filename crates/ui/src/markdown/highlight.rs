//! Code's words colored by their syntax: Sublime Text's grammars, as
//! syntect reads them, with the extra ones bat ships through two-face, and
//! One Half's light and dark themes, whose grounds are near the components'.

use std::ops::Range;
use std::sync::LazyLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use two_face::theme::EmbeddedThemeName;

/// Read once: they take a while to load and never change.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);
static LIGHT: LazyLock<Theme> = LazyLock::new(|| {
    two_face::theme::extra()
        .get(EmbeddedThemeName::OneHalfLight)
        .clone()
});
static DARK: LazyLock<Theme> = LazyLock::new(|| {
    two_face::theme::extra()
        .get(EmbeddedThemeName::OneHalfDark)
        .clone()
});

/// Runs of code and their colors, as 0xRRGGBB.
pub(crate) type Colored = Vec<(Range<usize>, u32)>;

/// The runs of `code` its syntax colors, in order and apart from each
/// other, read as the language `lang` names it, by name or by file
/// extension. None for a language it does not know.
pub(crate) fn highlight(code: &str, lang: &str, dark: bool) -> Option<Colored> {
    let syntax = SYNTAXES
        .find_syntax_by_token(lang)
        .or_else(|| SYNTAXES.find_syntax_by_name(lang))?;
    let theme = match dark {
        true => &*DARK,
        false => &*LIGHT,
    };
    let mut lines = HighlightLines::new(syntax, theme);
    let mut runs: Colored = Vec::new();
    let mut at = 0;

    for line in LinesWithEndings::from(code) {
        for (style, words) in lines.highlight_line(line, &SYNTAXES).ok()? {
            let color = style.foreground;
            let color = u32::from(color.r) << 16 | u32::from(color.g) << 8 | u32::from(color.b);
            let range = at..at + words.len();

            at = range.end;

            // Words of the color before them go on from them.
            match runs.last_mut() {
                Some((last, last_color)) if *last_color == color && last.end == range.start => {
                    last.end = range.end
                }
                _ => runs.push((range, color)),
            }
        }
    }

    Some(runs)
}

#[cfg(test)]
mod tests {
    use super::highlight;

    #[test]
    fn colors_code_by_its_language() {
        let code = "fn main() {\n    let answer = 42;\n}\n";
        let runs = highlight(code, "rust", false).expect("Rust is known");

        // Every byte is in one run, in order.
        assert_eq!(runs.first().map(|(range, _)| range.start), Some(0));
        assert_eq!(runs.last().map(|(range, _)| range.end), Some(code.len()));
        assert!(runs.windows(2).all(|pair| pair[0].0.end == pair[1].0.start));

        // The keyword and the number are not the color of the name.
        let color_at = |index: usize| {
            runs.iter()
                .find(|(range, _)| range.contains(&index))
                .map(|(_, color)| *color)
        };

        assert_ne!(color_at(0), color_at(3));
        assert_ne!(
            color_at(code.find("42").unwrap()),
            color_at(code.find("answer").unwrap())
        );
    }

    #[test]
    fn knows_languages_by_name_and_extension() {
        for lang in [
            "ts",
            "tsx",
            "toml",
            "py",
            "sh",
            "json",
            "yaml",
            "Dockerfile",
            "Rust",
        ] {
            assert!(highlight("x", lang, true).is_some(), "{lang}");
        }

        assert_eq!(highlight("x", "no-such-language", false), None);
    }
}
