//! A file's text, read only: each line numbered, its words colored by
//! their syntax when the language is known, and a line too long for the
//! width wrapped under itself. Only the rows in view are drawn, so a long
//! file scrolls freely. The colors are worked out off the UI thread, and
//! again when the theme turns light or dark; until they come, the text
//! shows in the theme's own color.

use std::ops::Range;

use gpui::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, HighlightStyle, IntoElement,
    ListAlignment, ListOffset, ListState, ParentElement as _, Pixels, Refineable as _, Rems,
    RenderOnce, StyleRefinement, Styled, StyledText, Task, Window, div, list, px, rgb,
};

use crate::child_id;
use crate::markdown::highlight;
use crate::root::h_flex;
use crate::scroll::Scrollbar;
use crate::size::rems_from_px;
use crate::theme::{ActiveTheme as _, ThemeMode};

/// How far past the view rows are drawn, so scrolling finds them ready.
const OVERDRAW: Pixels = px(600.);

/// The size the text is drawn at, and the height of each line, as a diff
/// draws its lines.
const CODE: Rems = rems_from_px(12.);
const CODE_LINE: Rems = rems_from_px(18.);

/// Runs of a line and their colors, as 0xRRGGBB, from the line's start.
type LineColors = Vec<(Range<usize>, u32)>;

/// The colors of every line, and whether they were worked out for a dark
/// theme.
struct Colors {
    dark: bool,
    lines: Vec<LineColors>,
}

pub struct CodeViewState {
    /// The text, its tabs given the width GPUI does not give them.
    text: String,
    /// Where each line is in the text, without its line break.
    lines: Vec<Range<usize>>,
    /// The language, by name or by file extension.
    lang: String,
    /// None for a language it does not know, or until they are worked out.
    colors: Option<Colors>,
    /// The theme the colors are being worked out for, or were.
    coloring_for: Option<bool>,
    _coloring: Option<Task<()>>,
    list: ListState,
}

impl CodeViewState {
    pub fn new(_: &mut Context<Self>) -> Self {
        CodeViewState {
            text: String::new(),
            lines: Vec::new(),
            lang: String::new(),
            colors: None,
            coloring_for: None,
            _coloring: None,
            list: ListState::new(0, ListAlignment::Top, OVERDRAW),
        }
    }

    /// Shows `text`, colored as the language `lang` names it, by name or by
    /// file extension, such as `rs` or `Dockerfile`. Text shown before
    /// stays scrolled where it was, as far as it goes, as when a file is
    /// read again.
    pub fn set_text(&mut self, text: &str, lang: &str, cx: &mut Context<Self>) {
        let top = self.list.logical_scroll_top();

        self.text = text.replace('\t', "    ");
        self.lines = lines(&self.text);
        self.lang = lang.to_string();
        self.colors = None;
        self.coloring_for = None;
        self.list.reset(self.lines.len());

        if top.item_ix < self.lines.len() {
            self.list.scroll_to(top);
        } else if !self.lines.is_empty() {
            self.list.scroll_to(ListOffset {
                item_ix: self.lines.len() - 1,
                offset_in_item: Pixels::ZERO,
            });
        }

        let dark = cx.theme().mode == ThemeMode::Dark;

        self.color(dark, cx);
        cx.notify();
    }

    /// Whether it has no lines, as an empty file has none.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Works out the colors for a light or a dark theme off the UI thread,
    /// unless they are being worked out for it already.
    fn color(&mut self, dark: bool, cx: &mut Context<Self>) {
        if self.coloring_for == Some(dark) {
            return;
        }

        let text = self.text.clone();
        let lang = self.lang.clone();
        let lines = self.lines.clone();

        self.coloring_for = Some(dark);
        self._coloring = Some(cx.spawn(async move |this, cx| {
            let colored = cx
                .background_spawn(async move {
                    highlight::highlight(&text, &lang, dark).map(|runs| by_line(&runs, &lines))
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                // Another text may have come meanwhile, and colors of its own.
                if this.coloring_for == Some(dark) {
                    this.colors = colored.map(|lines| Colors { dark, lines });
                    cx.notify();
                }
            });
        }));
    }
}

/// Where each line of `text` is, without its line break: a `\n`, or a
/// `\r\n`. A break at the end starts no line, so a text without one has
/// none.
fn lines(text: &str) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;

    for (at, _) in text.match_indices('\n') {
        let end = match text[start..at].ends_with('\r') {
            true => at - 1,
            false => at,
        };

        lines.push(start..end);
        start = at + 1;
    }

    if start < text.len() {
        lines.push(start..text.len());
    }

    lines
}

/// The runs of the whole text, each cut at the lines it crosses and
/// counted from its line's start.
fn by_line(runs: &[(Range<usize>, u32)], lines: &[Range<usize>]) -> Vec<LineColors> {
    let mut colored = Vec::with_capacity(lines.len());
    let mut first = 0;

    for line in lines {
        // Runs are in order, so those before this line are done with.
        while runs
            .get(first)
            .is_some_and(|(run, _)| run.end <= line.start)
        {
            first += 1;
        }

        let mut parts = Vec::new();

        for (run, color) in &runs[first..] {
            if run.start >= line.end {
                break;
            }

            let start = run.start.max(line.start) - line.start;
            let end = run.end.min(line.end) - line.start;

            if start < end {
                parts.push((start..end, *color));
            }
        }

        colored.push(parts);
    }

    colored
}

#[derive(IntoElement)]
pub struct CodeView {
    id: ElementId,
    state: Entity<CodeViewState>,
    style: StyleRefinement,
}

impl CodeView {
    pub fn new(id: impl Into<ElementId>, state: &Entity<CodeViewState>) -> Self {
        CodeView {
            id: id.into(),
            state: state.clone(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for CodeView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// One line: its number, and its text in its colors.
fn row(state: &Entity<CodeViewState>, index: usize, cx: &mut App) -> AnyElement {
    let view = state.read(cx);

    let Some(line) = view.lines.get(index) else {
        return div().into_any_element();
    };

    let theme = cx.theme();
    let colors = theme.colors;
    let dark = theme.mode == ThemeMode::Dark;

    // The numbers' column is as wide as the last one.
    let digits = view.lines.len().to_string().len() as f32;
    let text = view.text[line.clone()].to_string();

    let highlights: Vec<(Range<usize>, HighlightStyle)> = view
        .colors
        .as_ref()
        .filter(|colors| colors.dark == dark)
        .and_then(|colors| colors.lines.get(index))
        .map(|runs| {
            runs.iter()
                .map(|(range, color)| {
                    let style = HighlightStyle {
                        color: Some(rgb(*color).into()),
                        ..HighlightStyle::default()
                    };

                    (range.clone(), style)
                })
                .collect()
        })
        .unwrap_or_default();

    // As wide as the list, so the text wraps at its edge rather than run
    // past it.
    h_flex()
        .w_full()
        .items_start()
        .min_h(CODE_LINE)
        .font_family(theme.mono_family.clone())
        .text_size(CODE)
        .line_height(CODE_LINE)
        .child(
            div()
                .flex_none()
                .w(rems_from_px(16. + 8. * digits))
                .pr_3()
                .text_right()
                .text_color(colors.muted_foreground)
                .child((index + 1).to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pr_3()
                .text_color(colors.foreground)
                .child(StyledText::new(text).with_highlights(highlights)),
        )
        .into_any_element()
}

impl RenderOnce for CodeView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // The theme may have turned light or dark since the colors were
        // worked out.
        let dark = cx.theme().mode == ThemeMode::Dark;

        self.state.update(cx, |state, cx| {
            if !state.lines.is_empty() {
                state.color(dark, cx);
            }
        });

        let list_state = self.state.read(cx).list.clone();
        let state = self.state.clone();

        let rows = list(list_state.clone(), move |index, _, cx| {
            row(&state, index, cx)
        })
        .size_full();

        let mut view = div()
            .relative()
            .size_full()
            .child(rows)
            .child(Scrollbar::vertical(
                child_id(&self.id, "scrollbar"),
                list_state,
            ));

        view.style().refine(&self.style);
        view
    }
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, TestAppContext};

    use super::{CodeViewState, by_line, lines};

    #[test]
    fn finds_the_lines_without_their_breaks() {
        let text = "one\r\ntwo\n\nlast";

        let found: Vec<&str> = lines(text).into_iter().map(|line| &text[line]).collect();

        assert_eq!(found, ["one", "two", "", "last"]);
        assert_eq!(lines("one\n").len(), 1);
        assert!(lines("").is_empty());
        assert_eq!(lines("\n").len(), 1);
    }

    #[test]
    fn cuts_the_colors_at_each_line() {
        let text = "let a;\n/* one\ntwo */\n";
        let found = lines(text);
        let comment = text.find("/*").unwrap();
        let runs = [(0..3, 1), (3..comment, 2), (comment..text.len(), 3)];

        assert_eq!(
            by_line(&runs, &found),
            [vec![(0..3, 1), (3..6, 2)], vec![(0..6, 3)], vec![(0..6, 3)],]
        );
    }

    #[gpui::test]
    fn colors_a_file_off_the_ui_thread(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);

        let state = cx.new(|cx| {
            let mut state = CodeViewState::new(cx);

            state.set_text("fn main() {\n\tlet answer = 42;\n}\n", "rs", cx);
            state
        });

        cx.run_until_parked();

        state.read_with(cx, |state, _| {
            assert_eq!(state.lines.len(), 3);
            assert_eq!(&state.text[state.lines[1].clone()], "    let answer = 42;");

            let colors = state.colors.as_ref().expect("Rust is colored");

            assert!(!colors.dark);
            assert_eq!(colors.lines.len(), 3);
            assert!(colors.lines[1].len() > 1, "{:?}", colors.lines[1]);
        });

        // A language it does not know shows in the theme's own color.
        state.update(cx, |state, cx| {
            state.set_text("plain\n", "no-such-language", cx)
        });
        cx.run_until_parked();

        state.read_with(cx, |state, _| {
            assert!(state.colors.is_none());
            assert!(!state.is_empty());
        });
    }
}
