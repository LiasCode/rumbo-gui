use gpui::{
    AnyElement, AppContext as _, Context, Entity, ParentElement as _, Styled as _, Window, div, px,
};
use ui::{CodeView, CodeViewState};

use super::{Demos, section};
use crate::Overview;

const RUST: &str = r#"//! Starts the window, or says why it cannot.

use std::process::ExitCode;

/// The longest a host may take to start before the window says it starts.
const QUIET_START: u64 = 400;

fn main() -> ExitCode {
	let config = match config::parse(std::env::args().skip(1)) {
		Ok(config) => config,
		Err(problem) => return say(problem),
	};

	/* A line long enough that a narrow view wraps it under itself, rather than cut it off or scroll it sideways. */
	run(config).unwrap_or_else(say)
}

fn say(problem: String) -> ExitCode {
	eprintln!("rumbo-gui: {problem}");

	ExitCode::FAILURE
}
"#;

const NOTES: &str = "What the window keeps, and where:\n\n- its preferences, in its own file;\n- nothing else, since rumbo keeps the rest.\n";

pub struct Demo {
    rust: Entity<CodeViewState>,
    notes: Entity<CodeViewState>,
}

impl Demo {
    pub fn new(cx: &mut Context<Overview>) -> Self {
        let shown = |text: &'static str, lang: &'static str| {
            move |cx: &mut Context<CodeViewState>| {
                let mut state = CodeViewState::new(cx);

                state.set_text(text, lang, cx);
                state
            }
        };

        Demo {
            rust: cx.new(shown(RUST, "rs")),
            notes: cx.new(shown(NOTES, "no-such-language")),
        }
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.code_view;

    vec![
        section(
            "A file of a language it knows",
            "Rust, its tabs four spaces wide, and a line too long for the width wrapped under its number. The colors follow the theme.",
            div()
                .w(px(560.))
                .h(px(360.))
                .child(CodeView::new("rust", &demo.rust)),
            cx,
        ),
        section(
            "A file of a language it does not know",
            "Its text in the theme's own color.",
            div()
                .w(px(560.))
                .h(px(120.))
                .child(CodeView::new("notes", &demo.notes)),
            cx,
        ),
    ]
}
