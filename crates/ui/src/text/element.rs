//! The text of a field as an element: laid out in one line, or wrapped in
//! several, scrolled to keep the cursor in view, and painted with the
//! selection and the cursor. What it painted is kept on the state, which
//! finds there the offset under the pointer and the bounds an input method
//! asks for.

use std::ops::Range;

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity,
    Focusable as _, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, PaintQuad,
    Pixels, Point, SharedString, Style, TextAlign, TextRun, UnderlineStyle, Window, WrappedLine,
    fill, point, px, relative, size,
};

use super::state::InputState;
use crate::theme::ActiveTheme as _;

/// How wide the cursor is.
const CURSOR_WIDTH: Pixels = px(1.5);

/// What is drawn past the end of a line whose line break is selected.
const LINE_BREAK_WIDTH: Pixels = px(4.);

pub(crate) struct TextElement {
    state: Entity<InputState>,
}

impl TextElement {
    pub(crate) fn new(state: &Entity<InputState>) -> Self {
        TextElement {
            state: state.clone(),
        }
    }
}

/// One logical line of what was laid out, wrapped into rows.
pub(crate) struct Line {
    /// Its offset in what is shown.
    start: usize,
    /// How far below the first line it starts.
    top: Pixels,
    layout: WrappedLine,
}

impl Line {
    fn rows(&self) -> usize {
        self.layout.wrap_boundaries().len() + 1
    }

    fn end(&self) -> usize {
        self.start + self.layout.len()
    }
}

/// What a field painted last.
pub(crate) struct Painted {
    bounds: Bounds<Pixels>,
    /// Where the first line starts in the window, scrolled.
    origin: Point<Pixels>,
    line_height: Pixels,
    lines: Vec<Line>,
}

impl Painted {
    pub(crate) fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    pub(crate) fn line_height(&self) -> Pixels {
        self.line_height
    }

    /// The height of all the text, scrolled or not.
    pub(crate) fn height(&self) -> Pixels {
        let rows: usize = self.lines.iter().map(Line::rows).sum();

        self.line_height * rows.max(1) as f32
    }

    /// The line an offset of what is shown is on.
    fn line_of(&self, offset: usize) -> Option<&Line> {
        self.lines.iter().rev().find(|line| line.start <= offset)
    }

    /// Where an offset of what is shown is, from the start of the text.
    pub(crate) fn position_of(&self, offset: usize) -> Point<Pixels> {
        let Some(line) = self.line_of(offset) else {
            return Point::default();
        };

        let index = offset.min(line.end()) - line.start;
        let at = line
            .layout
            .position_for_index(index, self.line_height)
            .unwrap_or_default();

        point(at.x, line.top + at.y)
    }

    /// The offset of what is shown at `position` in the window.
    pub(crate) fn offset_at(&self, position: Point<Pixels>) -> usize {
        self.offset_at_local(position - self.origin)
    }

    /// The offset of what is shown at `position` from the start of the
    /// text. A position above or below the text is on its first or last
    /// row.
    pub(crate) fn offset_at_local(&self, position: Point<Pixels>) -> usize {
        let Some(line) = self
            .lines
            .iter()
            .rev()
            .find(|line| line.top <= position.y)
            .or(self.lines.first())
        else {
            return 0;
        };

        let bottom = self.line_height * line.rows() as f32 - px(1.);
        let y = (position.y - line.top).clamp(Pixels::ZERO, bottom);
        let x = position.x.max(Pixels::ZERO);

        let index = match line
            .layout
            .closest_index_for_position(point(x, y), self.line_height)
        {
            Ok(index) | Err(index) => index,
        };

        line.start + index
    }

    /// The bounds in the window of a range of what is shown, from the top
    /// of its start to the bottom of its end.
    pub(crate) fn bounds_of(&self, range: Range<usize>) -> Bounds<Pixels> {
        let start = self.origin + self.position_of(range.start);
        let end = self.origin + self.position_of(range.end) + point(Pixels::ZERO, self.line_height);

        Bounds::from_corners(start, point(end.x.max(start.x), end.y))
    }

    /// The rectangles that cover a range of what is shown, row by row, in
    /// the window.
    fn cover(&self, range: Range<usize>, width: Pixels) -> Vec<Bounds<Pixels>> {
        let mut covered = Vec::new();

        for line in &self.lines {
            let breaks = range.end > line.end() && range.start <= line.end();
            let start = range.start.max(line.start);
            let end = range.end.min(line.end());

            if start > end || (start == end && !breaks) {
                continue;
            }

            let from = self.position_of(start);
            let to = self.position_of(end);
            let extra = if breaks {
                LINE_BREAK_WIDTH
            } else {
                Pixels::ZERO
            };
            let mut row = |left: Pixels, right: Pixels, top: Pixels| {
                covered.push(Bounds::from_corners(
                    self.origin + point(left, top),
                    self.origin + point(right, top + self.line_height),
                ));
            };

            if from.y == to.y {
                row(from.x, to.x + extra, from.y);
                continue;
            }

            row(from.x, width, from.y);

            let mut top = from.y + self.line_height;

            while top < to.y {
                row(Pixels::ZERO, width, top);
                top += self.line_height;
            }

            row(Pixels::ZERO, to.x + extra, to.y);
        }

        covered
    }
}

/// What the text shows: what it holds, or its placeholder in a quieter
/// color while it holds nothing.
fn shown_text(state: &InputState, color: Hsla, muted: Hsla) -> (SharedString, Hsla) {
    match state.value().is_empty() {
        true => (state.placeholder_text().clone(), muted),
        false => (state.shown(), color),
    }
}

/// How many rows `text` takes at `width`.
fn rows_of(
    text: SharedString,
    run: TextRun,
    font_size: Pixels,
    width: Option<Pixels>,
    window: &mut Window,
) -> usize {
    let runs = [TextRun {
        len: text.len(),
        ..run
    }];

    window
        .text_system()
        .shape_text(text, font_size, &runs, width, None)
        .map(|lines| {
            lines
                .iter()
                .map(|line| line.wrap_boundaries().len() + 1)
                .sum()
        })
        .unwrap_or(1)
}

pub(crate) struct Prepainted {
    painted: Painted,
    selection: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepainted;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let state = self.state.read(cx);
        let line_height = window.line_height();

        let mut style = Style::default();

        style.size.width = relative(1.).into();
        style.min_size.width = px(0.).into();
        style.flex_grow = 1.;
        style.flex_shrink = 1.;

        let Some((min, max)) = state.rows() else {
            style.size.height = line_height.into();

            return (window.request_layout(style, [], cx), ());
        };

        // Several lines grow with the text, so their height is measured at
        // the width the field gets.
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let colors = cx.theme().colors;
        let (text, color) = shown_text(state, text_style.color, colors.muted_foreground);
        let run = TextRun {
            len: 0,
            font: text_style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let layout = window.request_measured_layout(style, move |known, available, window, _| {
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(width) => Some(width),
                _ => None,
            });
            let rows = rows_of(text.clone(), run.clone(), font_size, width, window);

            size(
                width.unwrap_or_default(),
                line_height * rows.clamp(min, max) as f32,
            )
        });

        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let colors = cx.theme().colors;
        let state = self.state.read(cx);
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let multi_line = state.is_multi_line();
        let empty = state.value().is_empty();

        let (text, color) = shown_text(state, text_style.color, colors.muted_foreground);
        let run = TextRun {
            len: text.len(),
            font: text_style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        // What an input method is composing is underlined.
        let runs = match state.buffer().marked().filter(|_| !empty) {
            Some(marked) => {
                let marked = state.shown_offset(marked.start)..state.shown_offset(marked.end);

                [
                    TextRun {
                        len: marked.start,
                        ..run.clone()
                    },
                    TextRun {
                        len: marked.len(),
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(color),
                            wavy: false,
                        }),
                        ..run.clone()
                    },
                    TextRun {
                        len: text.len() - marked.end,
                        ..run
                    },
                ]
                .into_iter()
                .filter(|run| run.len > 0)
                .collect()
            }

            None => vec![run],
        };

        let wrap = multi_line.then_some(bounds.size.width);
        let shaped = window
            .text_system()
            .shape_text(text.clone(), font_size, &runs, wrap, None)
            .unwrap_or_default();

        let mut lines = Vec::with_capacity(shaped.len());
        let mut start = 0;
        let mut top = Pixels::ZERO;

        for layout in shaped {
            let line = Line { start, top, layout };

            start = line.end() + 1;
            top += line_height * line.rows() as f32;
            lines.push(line);
        }

        let mut painted = Painted {
            bounds,
            origin: bounds.origin,
            line_height,
            lines,
        };

        let selection = state.buffer().selection();
        let selection = match empty {
            true => 0..0,
            false => state.shown_offset(selection.start)..state.shown_offset(selection.end),
        };
        let cursor = match empty {
            true => 0,
            false => state.shown_offset(state.cursor()),
        };
        let at = painted.position_of(cursor);

        // Scrolls to the cursor when it moved, and never past the text.
        let mut scroll = state.scroll;

        if multi_line {
            if state.reveal {
                if at.y < scroll.y {
                    scroll.y = at.y;
                } else if at.y + line_height > scroll.y + bounds.size.height {
                    scroll.y = at.y + line_height - bounds.size.height;
                }
            }

            let most = (painted.height() - bounds.size.height).max(Pixels::ZERO);

            scroll.x = Pixels::ZERO;
            scroll.y = scroll.y.clamp(Pixels::ZERO, most);
        } else {
            let room = bounds.size.width - CURSOR_WIDTH;

            if state.reveal {
                if at.x < scroll.x {
                    scroll.x = at.x;
                } else if at.x > scroll.x + room {
                    scroll.x = at.x - room;
                }
            }

            let width = painted
                .lines
                .first()
                .map_or(Pixels::ZERO, |line| line.layout.width());
            let most = (width - room).max(Pixels::ZERO);

            scroll.x = scroll.x.clamp(Pixels::ZERO, most);
            scroll.y = Pixels::ZERO;
        }

        painted.origin = bounds.origin - scroll;

        let selection = match selection.is_empty() {
            true => Vec::new(),
            false => painted
                .cover(selection.clone(), bounds.size.width)
                .into_iter()
                .map(|covered| fill(covered, colors.selection))
                .collect(),
        };

        let shows_cursor = state.is_focused(window) && state.cursor_shown && selection.is_empty();
        let cursor = shows_cursor.then(|| {
            let inset = (line_height - font_size * 1.2).max(Pixels::ZERO) / 2.;
            let at = painted.origin + at + point(Pixels::ZERO, inset);

            fill(
                Bounds::new(at, size(CURSOR_WIDTH, line_height - inset * 2.)),
                colors.caret,
            )
        });

        self.state.update(cx, |state, _| {
            state.scroll = scroll;
            state.reveal = false;
        });

        Prepainted {
            painted,
            selection,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepainted: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.state.focus_handle(cx);

        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );

        let painted = std::mem::replace(
            &mut prepainted.painted,
            Painted {
                bounds,
                origin: bounds.origin,
                line_height: Pixels::ZERO,
                lines: Vec::new(),
            },
        );

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for quad in prepainted.selection.drain(..) {
                window.paint_quad(quad);
            }

            for line in &painted.lines {
                let origin = painted.origin + point(Pixels::ZERO, line.top);

                let _ = line.layout.paint(
                    origin,
                    painted.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }

            if let Some(cursor) = prepainted.cursor.take() {
                window.paint_quad(cursor);
            }
        });

        self.state
            .update(cx, |state, _| state.painted = Some(painted));
    }
}
