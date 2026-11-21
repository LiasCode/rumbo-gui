//! Diagrams written in Mermaid, as GitHub draws a fence named mermaid:
//! laid out and drawn as SVG by merman, the Rust Mermaid that Zed uses, in
//! Mermaid's neutral theme or its dark one.

use std::sync::LazyLock;

use merman::{
    MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer,
    SourcePresentationConfig, SvgRequest,
};

/// Made once for each theme, and kept: a renderer can be used again.
static LIGHT: LazyLock<Renderer> = LazyLock::new(|| renderer("neutral"));
static DARK: LazyLock<Renderer> = LazyLock::new(|| renderer("dark"));

fn renderer(theme: &str) -> Renderer {
    let config = SourcePresentationConfig::from_value(serde_json::json!({ "theme": theme }));

    Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(config))
}

/// The SVG `source` draws, without what a browser alone can draw, so GPUI
/// draws it whole; or why it could not be drawn.
pub(crate) fn mermaid(source: &str, dark: bool) -> Result<String, String> {
    let renderer = match dark {
        true => &*DARK,
        false => &*LIGHT,
    };
    let request = RenderRequest::document(source, OperationControl::new(), SvgRequest::default());

    match renderer.render(request) {
        Ok(RenderOutput::Document(Some(document))) => Ok(document.svg().to_string()),
        Ok(_) => Err("It holds no diagram.".to_string()),
        Err(error) => Err(said(&error.to_string())),
    }
}

/// What merman says went wrong, without how it says it.
fn said(error: &str) -> String {
    error
        .split_once("message: \"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map_or(error, |(message, _)| message)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::mermaid;

    #[test]
    fn draws_a_flowchart_with_its_words() {
        let svg = mermaid("flowchart TD\n    A[Task sent] --> B{Needs a tool?}", false).unwrap();

        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("Task sent") && svg.contains("Needs a tool?"));

        // A browser's HTML in it would not be drawn.
        assert!(!svg.contains("foreignObject"));
    }

    #[test]
    fn says_why_a_diagram_cannot_be_drawn() {
        let problem = mermaid("flowchart TD\n A --> ", true).unwrap_err();

        assert!(problem.contains("unexpected end of input"), "{problem}");
        assert!(mermaid("just words", false).is_err());
    }
}
