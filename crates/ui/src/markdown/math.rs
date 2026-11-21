//! Formulas written in TeX, typeset by latex-rust in STIX Two Math and
//! drawn as SVG, with where their baseline stands so they line up with the
//! words around them.

use std::sync::LazyLock;

use latex_rust::{Color, Dim, MathFont, MathStyle, SvgOptions};

/// Loaded once: the font is in latex-rust itself.
static FONT: LazyLock<Result<MathFont, String>> =
    LazyLock::new(|| MathFont::stix_two_math().map_err(|error| error.to_string()));

/// A formula drawn, and its size and baseline in pixels.
#[derive(Debug, Clone)]
pub(crate) struct Formula {
    pub(crate) svg: String,
    pub(crate) width: f32,
    pub(crate) height: f32,
    /// How far down from its top its baseline is.
    pub(crate) baseline: f32,
}

/// Points, as latex-rust measures, in the pixels an SVG is drawn in.
fn pixels(points: &Dim) -> f32 {
    let (numerator, denominator) = points.as_ratio().unwrap_or((0, 1));

    (numerator as f64 / denominator as f64 * 4. / 3.) as f32
}

/// `tex` typeset with an em of `size` pixels in `color`, 0xRRGGBB, as a
/// formula on its own line when `display`, or among words. Why it could
/// not be, when it could not.
pub(crate) fn formula(tex: &str, size: u32, color: u32, display: bool) -> Result<Formula, String> {
    let font = FONT.as_ref().map_err(Clone::clone)?;
    let style = match display {
        true => MathStyle::Display,
        false => MathStyle::Text,
    };

    // An SVG's points are three quarters of its pixels.
    let em = Dim::ratio(i64::from(size) * 3, 4);
    let ast = latex_rust::parse(tex.trim()).map_err(|error| error.to_string())?;
    let tree = latex_rust::layout_with_em_size_pt(&ast, font, style, &em)
        .map_err(|error| error.to_string())?;
    let options = SvgOptions {
        font_size_pt: em,
        color: Color::rgb((color >> 16) as u8, (color >> 8) as u8, color as u8),
        display,
    };
    let image = latex_rust::render_svg_with_metrics(&tree, font, &options)
        .map_err(|error| error.to_string())?;

    Ok(Formula {
        width: pixels(&image.width_pt),
        height: pixels(&image.height_pt),
        baseline: pixels(&image.baseline_pt),
        svg: image.svg,
    })
}

#[cfg(test)]
mod tests {
    use super::formula;

    #[test]
    fn typesets_a_formula_with_its_baseline_inside_it() {
        let drawn = formula(r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}", 14, 0x171717, true).unwrap();

        assert!(drawn.svg.contains("<svg"));
        assert!(drawn.svg.contains("#171717"));
        assert!(drawn.width > drawn.height && drawn.height > 14.);
        assert!(0. < drawn.baseline && drawn.baseline < drawn.height);
    }

    #[test]
    fn grows_with_the_size_asked() {
        let small = formula("x^2", 14, 0, false).unwrap();
        let large = formula("x^2", 28, 0, false).unwrap();

        assert!((large.width / small.width - 2.).abs() < 0.05);
    }

    #[test]
    fn says_why_a_formula_cannot_be_typeset() {
        assert!(formula(r"\frac{1}{", 14, 0, false).is_err());
    }
}
