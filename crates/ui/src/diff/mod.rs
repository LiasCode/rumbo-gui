//! The changes a patch makes, file by file: each file's header, with what
//! happened to it and how many lines it adds and takes out, folds it to
//! that header; under it, its hunks, each line with its numbers before and
//! after, on green where it was added and on red where it was taken out.
//! Only the rows in view are drawn, so a long patch scrolls freely.

mod patch;

use std::collections::HashSet;
use std::ops::Range;

use gpui::{
    AnyElement, App, Context, ElementId, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ListAlignment, ListOffset, ListState, ParentElement as _, Pixels, Refineable as _, Rems,
    RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, list,
    prelude::FluentBuilder as _, px,
};

use crate::child_id;
use crate::icon::{Icon, IconName};
use crate::root::h_flex;
use crate::scroll::Scrollbar;
use crate::size::{Sizable as _, Size, rems_from_px};
use crate::theme::ActiveTheme as _;
use patch::{Row, rows};

pub use patch::{DiffFile, DiffLine, FileStatus, Hunk, LineKind};

/// How far past the view rows are drawn, so scrolling finds them ready.
const OVERDRAW: Pixels = px(600.);

/// The size the text of lines is drawn at, and the height of each.
const CODE: Rems = rems_from_px(12.);
const CODE_LINE: Rems = rems_from_px(18.);

/// The files of a patch, which are folded, and where it is scrolled.
pub struct DiffState {
    files: Vec<DiffFile>,
    /// The paths of the files folded to their header.
    folded: HashSet<String>,
    rows: Vec<Row>,
    list: ListState,
}

impl DiffState {
    pub fn new(files: Vec<DiffFile>, _: &mut Context<Self>) -> Self {
        let folded = HashSet::new();
        let rows = rows(&files, &folded);
        let list = ListState::new(rows.len(), ListAlignment::Top, OVERDRAW);

        DiffState {
            files,
            folded,
            rows,
            list,
        }
    }

    pub fn files(&self) -> &[DiffFile] {
        &self.files
    }

    /// Other files, read again: the ones folded that are still there stay
    /// folded, and the diff stays scrolled where it was, as far as it goes.
    pub fn set_files(&mut self, files: Vec<DiffFile>, cx: &mut Context<Self>) {
        let top = self.list.logical_scroll_top();

        self.folded
            .retain(|path| files.iter().any(|file| file.path == *path));
        self.files = files;
        self.rows = rows(&self.files, &self.folded);
        self.list.reset(self.rows.len());

        if top.item_ix < self.rows.len() {
            self.list.scroll_to(top);
        } else if !self.rows.is_empty() {
            self.list.scroll_to(ListOffset {
                item_ix: self.rows.len() - 1,
                offset_in_item: Pixels::ZERO,
            });
        }

        cx.notify();
    }

    /// Folds the file at `index` to its header, or opens it.
    pub fn toggle(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(file) = self.files.get(index) else {
            return;
        };

        if !self.folded.remove(&file.path) {
            self.folded.insert(file.path.clone());
        }

        let before = self.rows_of(index);

        self.rows = rows(&self.files, &self.folded);

        let after = self.rows_of(index);

        // Its header stays; what is under it comes or goes.
        self.list
            .splice(before.start + 1..before.end, after.len() - 1);
        cx.notify();
    }

    pub fn is_folded(&self, index: usize) -> bool {
        self.files
            .get(index)
            .is_some_and(|file| self.folded.contains(&file.path))
    }

    /// The rows of the file at `index`, its header first.
    fn rows_of(&self, index: usize) -> Range<usize> {
        let start = self
            .rows
            .iter()
            .position(|row| *row == Row::File(index))
            .unwrap_or(self.rows.len());
        let end = self.rows[start..]
            .iter()
            .skip(1)
            .position(|row| matches!(row, Row::File(_)))
            .map_or(self.rows.len(), |after| start + 1 + after);

        start..end
    }
}

#[derive(IntoElement)]
pub struct Diff {
    id: ElementId,
    state: Entity<DiffState>,
    style: StyleRefinement,
}

impl Diff {
    pub fn new(id: impl Into<ElementId>, state: &Entity<DiffState>) -> Self {
        Diff {
            id: id.into(),
            state: state.clone(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Diff {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A line's number on one side, in its column.
fn number(number: Option<usize>, color: gpui::Hsla) -> impl IntoElement {
    div()
        .flex_none()
        .w(rems_from_px(44.))
        .pr_2()
        .text_right()
        .text_color(color)
        .when_some(number, |cell, number| cell.child(number.to_string()))
}

fn row(id: &ElementId, state: &Entity<DiffState>, index: usize, cx: &mut App) -> AnyElement {
    let diff = state.read(cx);

    let Some(&row) = diff.rows.get(index) else {
        return div().into_any_element();
    };

    let theme = cx.theme();
    let colors = theme.colors;
    let mono = theme.mono_family.clone();
    let ring = theme.focus_ring();

    let code = || {
        div()
            .font_family(mono.clone())
            .text_size(CODE)
            .line_height(CODE_LINE)
    };
    let note = |words: &'static str| {
        div()
            .px_3()
            .py_2()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child(words)
    };

    match row {
        Row::File(file_index) => {
            let file = &diff.files[file_index];
            let folded = diff.is_folded(file_index);
            let toggled = state.clone();

            let (letter, color, status) = match file.status {
                FileStatus::Added => ("A", colors.success, "Added"),
                FileStatus::Deleted => ("D", colors.danger, "Deleted"),
                FileStatus::Modified => ("M", colors.warning, "Changed"),
                FileStatus::Renamed => ("R", colors.link, "Renamed"),
            };

            let path = match &file.old_path {
                Some(old) => format!("{old} → {}", file.path),
                None => file.path.clone(),
            };

            h_flex()
                .id(child_id(id, format!("file-{file_index}")))
                .debug_selector(|| format!("diff-file-{file_index}"))
                .h(rems_from_px(32.))
                .px_3()
                .gap_2()
                .bg(colors.muted)
                .border_b_1()
                .border_color(colors.border)
                .when(index > 0, |header| header.border_t_1())
                .text_sm()
                .cursor_pointer()
                .tab_index(0)
                .focus_visible(move |style| style.shadow(ring))
                .on_click(move |_, _, cx| {
                    toggled.update(cx, |state, cx| state.toggle(file_index, cx));
                })
                .child(
                    Icon::new(match folded {
                        true => IconName::ChevronRight,
                        false => IconName::ChevronDown,
                    })
                    .with_size(Size::Small)
                    .color(colors.muted_foreground),
                )
                .child(
                    div()
                        .flex_none()
                        .w(rems_from_px(14.))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(color)
                        .child(letter),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .child(path),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(status),
                )
                .child(
                    code()
                        .flex_none()
                        .text_color(colors.success)
                        .child(format!("+{}", file.additions())),
                )
                .child(
                    code()
                        .flex_none()
                        .text_color(colors.danger)
                        .child(format!("−{}", file.deletions())),
                )
                .into_any_element()
        }

        Row::Hunk(file, hunk) => code()
            .px_3()
            .py_1()
            .text_color(colors.muted_foreground)
            .bg(colors.muted.opacity(0.5))
            .truncate()
            .child(diff.files[file].hunks[hunk].header.clone())
            .into_any_element(),

        Row::Line(file, hunk, line) => {
            let line = &diff.files[file].hunks[hunk].lines[line];

            let (sign, ground, mark) = match line.kind {
                LineKind::Added => ("+", Some(colors.success.opacity(0.12)), colors.success),
                LineKind::Removed => ("−", Some(colors.danger.opacity(0.12)), colors.danger),
                LineKind::Context => ("", None, colors.muted_foreground),
            };

            // GPUI gives a tab no width of its own.
            let text = line.text.replace('\t', "    ");

            code()
                .flex()
                .flex_row()
                .items_start()
                .when_some(ground, |line, ground| line.bg(ground))
                .child(number(line.old, colors.muted_foreground))
                .child(number(line.new, colors.muted_foreground))
                .child(
                    div()
                        .flex_none()
                        .w(rems_from_px(16.))
                        .text_color(mark)
                        .child(sign),
                )
                .child(div().flex_1().min_w_0().pr_3().child(text))
                .into_any_element()
        }

        Row::Binary(_) => note("A binary file, whose lines are not shown.").into_any_element(),

        Row::Empty(_) => note("No lines changed.").into_any_element(),
    }
}

impl RenderOnce for Diff {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let list_state = self.state.read(cx).list.clone();
        let state = self.state.clone();
        let id = self.id.clone();

        let rows = list(list_state.clone(), move |index, _, cx| {
            row(&id, &state, index, cx)
        })
        .size_full();

        let mut diff = div()
            .relative()
            .size_full()
            .child(rows)
            .child(Scrollbar::vertical(
                child_id(&self.id, "scrollbar"),
                list_state,
            ));

        diff.style().refine(&self.style);
        diff
    }
}

/// Folding a file, in a window GPUI keeps in the test's own process.
#[cfg(test)]
mod tests {
    use gpui::{
        AppContext as _, Context, Entity, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke,
        Modifiers, ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext,
        Window, div, point, px,
    };

    use super::{Diff, DiffFile, DiffState};

    const PATCH: &str = "\
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1 +1 @@
-one
+uno
diff --git a/b.txt b/b.txt
--- a/b.txt
+++ b/b.txt
@@ -1 +1 @@
-two
+dos
";

    /// The diff 400 wide and 300 tall at the window's corner: the first
    /// file's header takes its first 32.
    struct Changes {
        state: Entity<DiffState>,
    }

    impl Render for Changes {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(400.))
                .h(px(300.))
                .child(Diff::new("changes", &self.state))
        }
    }

    fn changes(cx: &mut TestAppContext) -> (Entity<DiffState>, &mut VisualTestContext) {
        cx.update(|cx| {
            crate::theme::init(cx);
            crate::root::bind_keys(cx);
        });

        let state = cx.new(|cx| DiffState::new(DiffFile::parse(PATCH), cx));
        let shown = state.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Changes { state: shown });

        cx.run_until_parked();
        (state, cx)
    }

    /// A key as the keyboard gives it: GPUI presses a control when the key
    /// comes up.
    fn key(cx: &mut VisualTestContext, key: &'static str) {
        let keystroke = Keystroke::parse(key).unwrap();

        cx.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke });
    }

    #[gpui::test]
    fn folds_a_file_by_its_header(cx: &mut TestAppContext) {
        let (state, cx) = changes(cx);
        let header = point(px(200.), px(16.));

        // A header, a hunk's, and two lines; then the other file.
        assert_eq!(state.read_with(cx, |state, _| state.list.item_count()), 8);

        cx.simulate_click(header, Modifiers::none());

        assert!(state.read_with(cx, |state, _| state.is_folded(0)));
        assert_eq!(state.read_with(cx, |state, _| state.list.item_count()), 5);

        // The other file's header is the second row now; the keyboard
        // reaches the first header, and Enter opens it again.
        key(cx, "tab");
        key(cx, "enter");

        assert!(state.read_with(cx, |state, _| !state.is_folded(0)));
        assert_eq!(state.read_with(cx, |state, _| state.list.item_count()), 8);
    }

    #[gpui::test]
    fn keeps_the_files_folded_when_read_again(cx: &mut TestAppContext) {
        let (state, cx) = changes(cx);

        cx.update(|_, cx| state.update(cx, |state, cx| state.toggle(1, cx)));
        cx.update(|_, cx| {
            state.update(cx, |state, cx| state.set_files(DiffFile::parse(PATCH), cx))
        });

        assert!(state.read_with(cx, |state, _| state.is_folded(1) && !state.is_folded(0)));
        assert_eq!(state.read_with(cx, |state, _| state.list.item_count()), 5);
    }
}
