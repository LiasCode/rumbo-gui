//! Rows of cells under titled columns. A column that sorts does so when its
//! title is pressed, and any is made wider or narrower by dragging the edge
//! of its title. A row is chosen with the pointer, or with Up, Down, Page
//! Up, Page Down, Home, and End once the table has the keyboard, and opened
//! with Enter or a double press. Only the rows in view are drawn, so a
//! table holds as many as it is given; they scroll under the titles, and
//! across with them when the columns are wider than the table.

mod order;

use gpui::{
    App, Context, CursorStyle, Div, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Pixels,
    Refineable as _, Rems, RenderOnce, ScrollHandle, ScrollStrategy, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, UniformListScrollHandle, Window,
    actions, div, prelude::FluentBuilder as _, px, uniform_list,
};

pub use order::SortDirection;
use order::{Move, Sort};

use crate::drag::{self, Hold};
use crate::focus;
use crate::icon::{Icon, IconName};
use crate::root::h_flex;
use crate::scroll::Scrollbar;
use crate::size::{Sizable as _, rems_from_px};
use crate::theme::ActiveTheme as _;

actions!(
    table,
    [
        SelectPrevious,
        SelectNext,
        SelectPageUp,
        SelectPageDown,
        SelectFirst,
        SelectLast,
        Open
    ]
);

/// A table's key context.
const CONTEXT: &str = "Table";

/// How tall a row is, and the titles.
const ROW: Rems = rems_from_px(36.);

/// The room between a cell's edge and its text.
const PADDING: Rems = rems_from_px(8.);

/// How wide the strip the pointer takes the edge of a column in is.
const EDGE: Pixels = px(8.);

/// The name a column's edge goes by, for its line to show under the
/// pointer.
const GROUP: &str = "table-column-edge";

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);

    cx.bind_keys([
        KeyBinding::new("up", SelectPrevious, context),
        KeyBinding::new("down", SelectNext, context),
        KeyBinding::new("pageup", SelectPageUp, context),
        KeyBinding::new("pagedown", SelectPageDown, context),
        KeyBinding::new("home", SelectFirst, context),
        KeyBinding::new("end", SelectLast, context),
        KeyBinding::new("enter", Open, context),
    ]);
}

/// A column: its title, how wide it is, and how its cells read.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    key: SharedString,
    title: SharedString,
    width: Pixels,
    min_width: Pixels,
    end: bool,
    sortable: bool,
    grow: bool,
}

impl TableColumn {
    /// 160 wide, its cells against its start.
    pub fn new(key: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        TableColumn {
            key: key.into(),
            title: title.into(),
            width: px(160.),
            min_width: px(48.),
            end: false,
            sortable: false,
            grow: false,
        }
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    /// The narrowest its edge makes it; 48 unless said.
    pub fn min_width(mut self, width: Pixels) -> Self {
        self.min_width = width;
        self
    }

    /// Its cells against its end, as numbers are.
    pub fn end(mut self) -> Self {
        self.end = true;
        self
    }

    /// Its title sorts the rows by it.
    pub fn sortable(mut self) -> Self {
        self.sortable = true;
        self
    }

    /// It takes what the table has over its columns' widths.
    pub fn grow(mut self) -> Self {
        self.grow = true;
        self
    }
}

/// A row: what tells it from the others, and its cells, one for each
/// column in their order.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub key: SharedString,
    pub cells: Vec<SharedString>,
}

impl TableRow {
    pub fn new(
        key: impl Into<SharedString>,
        cells: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        TableRow {
            key: key.into(),
            cells: cells.into_iter().map(Into::into).collect(),
        }
    }
}

impl AsRef<[SharedString]> for TableRow {
    fn as_ref(&self) -> &[SharedString] {
        &self.cells
    }
}

/// What a person did with a table's rows, by the row's key.
#[derive(Debug, Clone, PartialEq)]
pub enum TableEvent {
    /// A row was chosen.
    Select(SharedString),
    /// A row was opened with Enter or a double press.
    Open(SharedString),
}

/// A table's columns and rows, and what is done with them: the sort, the
/// row chosen, the columns' widths, and where it is scrolled.
pub struct TableState {
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    sort: Option<Sort>,
    /// The places of the rows in the order shown.
    order: Vec<usize>,
    selected: Option<SharedString>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    across: ScrollHandle,
}

impl EventEmitter<TableEvent> for TableState {}

impl Focusable for TableState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TableState {
    pub fn new(columns: Vec<TableColumn>, rows: Vec<TableRow>, cx: &mut Context<Self>) -> Self {
        TableState {
            order: order::order(&rows, None),
            columns,
            rows,
            sort: None,
            selected: None,
            focus: cx.focus_handle().tab_stop(true),
            scroll: UniformListScrollHandle::new(),
            across: ScrollHandle::new(),
        }
    }

    /// Shows other rows, keeping the sort, and the row chosen while it is
    /// among them.
    pub fn set_rows(&mut self, rows: Vec<TableRow>, cx: &mut Context<Self>) {
        self.rows = rows;
        self.order = order::order(&self.rows, self.sort);

        if !self
            .rows
            .iter()
            .any(|row| Some(&row.key) == self.selected.as_ref())
        {
            self.selected = None;
        }

        cx.notify();
    }

    /// The key of the row chosen.
    pub fn selected(&self) -> Option<&SharedString> {
        self.selected.as_ref()
    }

    /// Chooses the row `key`, or none, and brings it into view, without
    /// telling it.
    pub fn select(&mut self, key: Option<SharedString>, cx: &mut Context<Self>) {
        self.selected = key;
        self.reveal();
        cx.notify();
    }

    /// The column the rows are sorted by, by its key, and which way.
    pub fn sort(&self) -> Option<(&SharedString, SortDirection)> {
        let sort = self.sort?;

        Some((&self.columns[sort.column].key, sort.direction))
    }

    /// Sorts the rows by the column `key`, or shows them as given.
    pub fn sort_by(&mut self, sort: Option<(&str, SortDirection)>, cx: &mut Context<Self>) {
        let sort = sort.and_then(|(key, direction)| {
            let column = self
                .columns
                .iter()
                .position(|column| column.key.as_ref() == key)?;

            Some(Sort { column, direction })
        });

        self.sorted(sort, cx);
    }

    fn sorted(&mut self, sort: Option<Sort>, cx: &mut Context<Self>) {
        self.sort = sort;
        self.order = order::order(&self.rows, sort);
        self.reveal();
        cx.notify();
    }

    fn press_title(&mut self, column: usize, cx: &mut Context<Self>) {
        self.sorted(order::pressed(self.sort, column), cx);
    }

    fn resize_column(&mut self, column: usize, width: Pixels, cx: &mut Context<Self>) {
        let column = &mut self.columns[column];
        let width = width.max(column.min_width);

        if column.width != width {
            column.width = width;
            cx.notify();
        }
    }

    /// Where the row chosen is shown.
    fn selected_place(&self) -> Option<usize> {
        let selected = self.selected.as_ref()?;

        self.order
            .iter()
            .position(|row| &self.rows[*row].key == selected)
    }

    fn reveal(&self) {
        if let Some(place) = self.selected_place() {
            self.scroll.scroll_to_item(place, ScrollStrategy::Nearest);
        }
    }

    /// Chooses the row shown at `place`, and tells it if it was not.
    fn choose(&mut self, place: usize, cx: &mut Context<Self>) {
        let Some(row) = self.order.get(place) else {
            return;
        };

        let key = self.rows[*row].key.clone();

        if self.selected.as_ref() != Some(&key) {
            self.selected = Some(key.clone());
            cx.emit(TableEvent::Select(key));
        }

        self.reveal();
        cx.notify();
    }

    fn step(&mut self, step: Move, window: &Window, cx: &mut Context<Self>) {
        let shown = self.scroll.0.borrow().base_handle.bounds().size.height;
        let page = (shown / ROW.to_pixels(window.rem_size())).floor() as usize;

        if let Some(place) = order::moved(self.selected_place(), self.order.len(), page, step) {
            self.choose(place, cx);
        }
    }

    fn open(&mut self, cx: &mut Context<Self>) {
        if let Some(key) = self.selected.clone() {
            cx.emit(TableEvent::Open(key));
        }
    }

    fn previous(&mut self, _: &SelectPrevious, window: &mut Window, cx: &mut Context<Self>) {
        self.step(Move::Previous, window, cx);
    }

    fn next(&mut self, _: &SelectNext, window: &mut Window, cx: &mut Context<Self>) {
        self.step(Move::Next, window, cx);
    }

    fn page_up(&mut self, _: &SelectPageUp, window: &mut Window, cx: &mut Context<Self>) {
        self.step(Move::PageUp, window, cx);
    }

    fn page_down(&mut self, _: &SelectPageDown, window: &mut Window, cx: &mut Context<Self>) {
        self.step(Move::PageDown, window, cx);
    }

    fn first(&mut self, _: &SelectFirst, window: &mut Window, cx: &mut Context<Self>) {
        self.step(Move::First, window, cx);
    }

    fn last(&mut self, _: &SelectLast, window: &mut Window, cx: &mut Context<Self>) {
        self.step(Move::Last, window, cx);
    }

    fn open_selected(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        // A title with the keyboard is pressed by Enter instead, which GPUI
        // does only after the actions let the key go on.
        if !self.focus.is_focused(window) {
            cx.propagate();
            return;
        }

        self.open(cx);
    }
}

#[derive(IntoElement)]
pub struct Table {
    state: Entity<TableState>,
    empty: SharedString,
    style: StyleRefinement,
}

impl Table {
    /// A table as tall as its parent, unless given a height.
    pub fn new(state: &Entity<TableState>) -> Self {
        Table {
            state: state.clone(),
            empty: "Nothing to show.".into(),
            style: StyleRefinement::default(),
        }
    }

    /// What it says while it has no rows.
    pub fn empty(mut self, text: impl Into<SharedString>) -> Self {
        self.empty = text.into();
        self
    }
}

impl Styled for Table {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A cell of `column`: as wide as it, or wider if it grows, and its text
/// on one line, cut short where it does not fit.
fn cell(column: &TableColumn) -> Div {
    div()
        .h_full()
        .flex()
        .items_center()
        .px(PADDING)
        .map(|cell| match column.grow {
            true => cell.flex_1().min_w(column.width),
            false => cell.flex_none().w(column.width),
        })
        .when(column.end, |cell| cell.justify_end())
}

impl RenderOnce for Table {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let entity = self.state.clone();
        let id = entity.entity_id().as_u64();
        let state = entity.read(cx);
        let focus = state.focus.clone();
        let scroll = state.scroll.clone();
        let across = state.across.clone();
        let count = state.order.len();
        let sort = state.sort;
        let columns = state.columns.clone();
        let total: Pixels = columns.iter().map(|column| column.width).sum();

        let theme = cx.theme();
        let colors = theme.colors;
        let radius = theme.radius;
        let ring = theme.focus_ring();
        let ringed = focus::ringed(&focus, window);

        // The edge after column `index`, which drags its width. It lies at
        // the start of what comes after the column, so that it is over both.
        let edge = |index: usize, window: &mut Window, cx: &mut App| {
            let column = &columns[index];
            let hold: Hold<(Pixels, Pixels)> = window.use_keyed_state(
                (ElementId::from(("table-edge", id)), column.key.clone()),
                cx,
                |_, _| None,
            );
            let held = hold.read(cx).is_some();
            let width = column.width;
            let resized = entity.clone();

            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(-EDGE / 2.)
                .w(EDGE)
                .flex()
                .justify_center()
                .group(GROUP)
                .child(div().w(px(1.)).h_full().bg(colors.ring).map(|line| {
                    match held {
                        true => line,
                        false => line
                            .opacity(0.)
                            .group_hover(GROUP, |style| style.opacity(1.)),
                    }
                }))
                .child(drag::area(
                    hold,
                    CursorStyle::ResizeColumn,
                    move |at, _, _, _| Some((at.x, width)),
                    move |(origin, start), at, _, _, cx| {
                        let width = *start + at.x - *origin;

                        resized.update(cx, |state, cx| state.resize_column(index, width, cx));
                    },
                ))
        };

        let mut titles = Vec::with_capacity(columns.len() + 1);

        for (index, column) in columns.iter().enumerate() {
            let sorted = sort
                .filter(|sort| sort.column == index)
                .map(|sort| sort.direction);
            let words = div()
                .debug_selector(|| format!("words-{index}"))
                .min_w_0()
                .truncate()
                .child(column.title.clone());

            let title = match column.sortable {
                false => words.into_any_element(),

                true => {
                    let (icon, color) = match sorted {
                        Some(SortDirection::Ascending) => (IconName::ArrowUp, colors.foreground),
                        Some(SortDirection::Descending) => (IconName::ArrowDown, colors.foreground),
                        None => (IconName::ChevronsUpDown, colors.muted_foreground),
                    };
                    let pressed = entity.clone();
                    let ring = ring.clone();
                    let icon = Icon::new(icon).small().color(color);

                    // Its words line up with the values under it: they start
                    // where the values start, or end where they end, with the
                    // icon on the inner side.
                    h_flex()
                        .id(("table-title", index))
                        .min_w_0()
                        .h(rems_from_px(28.))
                        .px(PADDING)
                        .map(|title| match column.end {
                            true => title.mr(-PADDING),
                            false => title.ml(-PADDING),
                        })
                        .gap_1()
                        .rounded(radius)
                        .cursor_pointer()
                        .hover(move |style| style.bg(colors.accent))
                        .tab_index(0)
                        .focus_visible(move |style| style.shadow(ring.clone()))
                        .on_click(move |_, _, cx| {
                            pressed.update(cx, |state, cx| state.press_title(index, cx))
                        })
                        .map(|title| match column.end {
                            true => title.child(icon).child(words),
                            false => title.child(words).child(icon),
                        })
                        .into_any_element()
                }
            };

            let mut title_cell = cell(column)
                .debug_selector(|| format!("title-{index}"))
                .relative()
                .child(title);

            if index > 0 {
                title_cell = title_cell.child(edge(index - 1, window, cx));
            }

            titles.push(title_cell);
        }

        // What is left of the titles' row, and the last column's edge. It
        // takes no room from a column that grows, which takes the same in
        // every row.
        let grows = columns.iter().any(|column| column.grow);

        if let Some(last) = columns.len().checked_sub(1) {
            titles.push(
                div()
                    .relative()
                    .h_full()
                    .map(|rest| match grows {
                        true => rest.flex_none(),
                        false => rest.flex_1(),
                    })
                    .child(edge(last, window, cx)),
            );
        }

        let header = h_flex()
            .flex_none()
            .h(ROW)
            .border_b_1()
            .border_color(colors.border)
            .text_color(colors.foreground)
            .font_weight(FontWeight::MEDIUM)
            .children(titles);

        let body = match count {
            0 => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(colors.muted_foreground)
                .child(self.empty)
                .into_any_element(),

            _ => {
                let listed = entity.clone();

                uniform_list(("table-rows", id), count, move |range, _, cx| {
                    let state = listed.read(cx);
                    let colors = cx.theme().colors;

                    range
                        .map(|place| {
                            let row = &state.rows[state.order[place]];
                            let chosen = state.selected.as_ref() == Some(&row.key);
                            let clicked = listed.clone();
                            let focus = state.focus.clone();

                            h_flex()
                                .id(("table-row", place))
                                .h(ROW)
                                .w_full()
                                .when(place + 1 < count, |row| {
                                    row.border_b_1().border_color(colors.border)
                                })
                                .map(|row| match chosen {
                                    true => row.bg(colors.muted),
                                    false => row.hover(|style| style.bg(colors.muted.opacity(0.5))),
                                })
                                .children(state.columns.iter().enumerate().map(
                                    |(index, column)| {
                                        let text =
                                            row.cells.get(index).cloned().unwrap_or_default();

                                        cell(column)
                                            .debug_selector(|| format!("cell-{place}-{index}"))
                                            .child(
                                                div()
                                                    .debug_selector(|| {
                                                        format!("value-{place}-{index}")
                                                    })
                                                    .min_w_0()
                                                    .truncate()
                                                    .child(text),
                                            )
                                    },
                                ))
                                .on_click(move |event, window, cx| {
                                    window.focus(&focus, cx);
                                    clicked.update(cx, |state, cx| {
                                        state.choose(place, cx);

                                        if event.click_count() >= 2 {
                                            state.open(cx);
                                        }
                                    });
                                })
                        })
                        .collect::<Vec<_>>()
                })
                .flex_1()
                .track_scroll(&scroll)
                .into_any_element()
            }
        };

        let mut table = div()
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .rounded(radius)
            .border_1()
            .border_color(colors.border)
            .bg(colors.background)
            .overflow_hidden()
            .text_sm()
            .key_context(CONTEXT)
            .track_focus(&focus)
            .on_action(window.listener_for(&entity, TableState::previous))
            .on_action(window.listener_for(&entity, TableState::next))
            .on_action(window.listener_for(&entity, TableState::page_up))
            .on_action(window.listener_for(&entity, TableState::page_down))
            .on_action(window.listener_for(&entity, TableState::first))
            .on_action(window.listener_for(&entity, TableState::last))
            .on_action(window.listener_for(&entity, TableState::open_selected))
            .when(ringed, |table| table.shadow(ring.clone()))
            .child(
                div()
                    .id(("table-across", id))
                    .size_full()
                    .overflow_x_scroll()
                    .restrict_scroll_to_axis()
                    .track_scroll(&across)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .h_full()
                            .min_w_full()
                            .w(total)
                            .child(header)
                            .child(body),
                    ),
            )
            // Beside the rows, under the titles.
            .child(
                div()
                    .absolute()
                    .top(ROW)
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .child(Scrollbar::vertical(("table-down", id), scroll)),
            )
            .child(Scrollbar::horizontal(("table-across-bar", id), across));

        table.style().refine(&self.style);
        table
    }
}

/// The pointer and the keys on a table, in a window GPUI keeps in the
/// test's own process.
#[cfg(test)]
mod pointer_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{
        AppContext as _, Context, Entity, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke,
        Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement as _, Render,
        Styled as _, TestAppContext, VisualTestContext, Window, div, point, px,
    };

    use super::{SortDirection, Table, TableColumn, TableEvent, TableRow, TableState};

    /// A table 500 wide and 301 tall at the window's corner, inside a
    /// border of 1: titles 36 tall, then rows of 36 from 37. Its name
    /// column is 200 wide, and its tokens column 100. Row `n` is named
    /// "Session n" and spent `(n * 37) % 100` tokens.
    struct Sessions {
        state: Entity<TableState>,
    }

    /// What a table told.
    type Told = Rc<RefCell<Vec<TableEvent>>>;

    impl Render for Sessions {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(Table::new(&self.state).w(px(500.)).h(px(301.)))
        }
    }

    /// The table's state, what it told, and its window.
    fn sessions(cx: &mut TestAppContext) -> (Entity<TableState>, Told, &mut VisualTestContext) {
        cx.update(crate::theme::init);
        cx.update(super::bind_keys);

        let told = Rc::new(RefCell::new(Vec::new()));
        let heard = told.clone();
        let (view, cx) = cx.add_window_view(move |_, cx| {
            let columns = vec![
                TableColumn::new("name", "Name").width(px(200.)).sortable(),
                TableColumn::new("tokens", "Tokens")
                    .width(px(100.))
                    .end()
                    .sortable(),
            ];
            let rows = (0..50)
                .map(|n| {
                    TableRow::new(
                        format!("s{n}"),
                        [format!("Session {n}"), format!("{}", (n * 37) % 100)],
                    )
                })
                .collect();
            let state = cx.new(|cx| TableState::new(columns, rows, cx));

            cx.subscribe(&state, move |_, _, event: &TableEvent, _| {
                heard.borrow_mut().push(event.clone())
            })
            .detach();

            Sessions { state }
        });

        let state = view.read_with(cx, |sessions, _| sessions.state.clone());

        (state, told, cx)
    }

    /// Enter pressed and let go, as GPUI presses a control with it only
    /// when it comes up.
    fn enter(cx: &mut VisualTestContext) {
        let keystroke = Keystroke::parse("enter").unwrap();

        cx.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke });
    }

    /// The middle of the row shown at `place`.
    fn row(place: f32) -> gpui::Point<gpui::Pixels> {
        point(px(60.), px(37. + 18. + 36. * place))
    }

    #[gpui::test]
    fn chooses_a_row_and_moves_and_opens_with_the_keys(cx: &mut TestAppContext) {
        let (state, told, cx) = sessions(cx);
        let none = Modifiers::none();

        cx.simulate_click(row(2.), none);
        cx.simulate_keystrokes("down down up");
        cx.simulate_keystrokes("end enter");

        assert_eq!(
            *told.borrow(),
            [
                TableEvent::Select("s2".into()),
                TableEvent::Select("s3".into()),
                TableEvent::Select("s4".into()),
                TableEvent::Select("s3".into()),
                TableEvent::Select("s49".into()),
                TableEvent::Open("s49".into()),
            ]
        );

        // The last row was brought into view.
        let offset = cx.read(|cx| state.read(cx).scroll.0.borrow().base_handle.offset());

        assert!(offset.y < px(0.));
    }

    #[gpui::test]
    fn opens_a_row_pressed_twice(cx: &mut TestAppContext) {
        let (_, told, cx) = sessions(cx);
        let none = Modifiers::none();

        cx.simulate_click(row(1.), none);
        cx.simulate_event(MouseDownEvent {
            position: row(1.),
            modifiers: none,
            button: MouseButton::Left,
            click_count: 2,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position: row(1.),
            modifiers: none,
            button: MouseButton::Left,
            click_count: 2,
        });

        assert_eq!(
            *told.borrow(),
            [
                TableEvent::Select("s1".into()),
                TableEvent::Open("s1".into()),
            ]
        );
    }

    #[gpui::test]
    fn sorts_by_a_title_pressed(cx: &mut TestAppContext) {
        let (state, told, cx) = sessions(cx);
        let none = Modifiers::none();

        // The tokens' title, against the end of its column.
        cx.simulate_click(point(px(290.), px(18.)), none);

        let sort = cx.read(|cx| {
            state
                .read(cx)
                .sort()
                .map(|(key, direction)| (key.clone(), direction))
        });

        assert_eq!(sort, Some(("tokens".into(), SortDirection::Ascending)));

        // The names' title, reached with Tab after the table, takes Enter.
        cx.update(|window, cx| {
            window.focus_next(cx);
            window.focus_next(cx);
        });
        enter(cx);

        let sort = cx.read(|cx| {
            state
                .read(cx)
                .sort()
                .map(|(key, direction)| (key.clone(), direction))
        });

        assert_eq!(sort, Some(("name".into(), SortDirection::Ascending)));
        assert!(told.borrow().is_empty());

        enter(cx);
        enter(cx);
        cx.simulate_click(point(px(290.), px(18.)), none);

        // Session 0 spent 0, and Session 27 spent 99.
        cx.simulate_click(row(0.), none);
        cx.simulate_keystrokes("end");

        assert_eq!(
            *told.borrow(),
            [
                TableEvent::Select("s0".into()),
                TableEvent::Select("s27".into()),
            ]
        );
    }

    /// A table 702 wide, so 700 inside its border, with a column that takes
    /// what the others leave and one of numbers against its end.
    struct Spread {
        state: Entity<TableState>,
    }

    impl Render for Spread {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(Table::new(&self.state).w(px(702.)).h(px(301.)))
        }
    }

    #[gpui::test]
    fn lines_up_each_title_with_its_cells(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);

        let (_, cx) = cx.add_window_view(|_, cx| {
            let columns = vec![
                TableColumn::new("name", "Name")
                    .width(px(200.))
                    .grow()
                    .sortable(),
                TableColumn::new("tokens", "Tokens")
                    .width(px(100.))
                    .end()
                    .sortable(),
                TableColumn::new("model", "Model").width(px(150.)),
            ];
            let rows = vec![TableRow::new("s0", ["Session 0", "1,234", "opus"])];

            Spread {
                state: cx.new(|cx| TableState::new(columns, rows, cx)),
            }
        });

        let bounds = |cx: &mut VisualTestContext, selector: &'static str| {
            cx.debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} was not drawn"))
        };

        // Each column is as wide under its title as in it, and the one that
        // grows takes all that is left: 700 less 250.
        for (title, cell) in [
            ("title-0", "cell-0-0"),
            ("title-1", "cell-0-1"),
            ("title-2", "cell-0-2"),
        ] {
            let title = bounds(cx, title);
            let cell = bounds(cx, cell);

            assert_eq!((title.left(), title.right()), (cell.left(), cell.right()));
        }

        assert_eq!(bounds(cx, "cell-0-0").size.width, px(450.));

        // A title's words start where its values start, or end where they
        // end in a column against its end.
        assert_eq!(bounds(cx, "words-0").left(), bounds(cx, "value-0-0").left());
        assert_eq!(
            bounds(cx, "words-1").right(),
            bounds(cx, "value-0-1").right()
        );
        assert_eq!(bounds(cx, "words-2").left(), bounds(cx, "value-0-2").left());
    }

    #[gpui::test]
    fn lines_up_the_titles_when_no_column_grows(cx: &mut TestAppContext) {
        let (_, _, cx) = sessions(cx);

        for (title, cell) in [("title-0", "cell-0-0"), ("title-1", "cell-0-1")] {
            let title = cx.debug_bounds(title).unwrap();
            let cell = cx.debug_bounds(cell).unwrap();

            assert_eq!((title.left(), title.right()), (cell.left(), cell.right()));
        }
    }

    #[gpui::test]
    fn drags_the_edge_of_a_column(cx: &mut TestAppContext) {
        let (state, _, cx) = sessions(cx);
        let none = Modifiers::none();
        let width = |cx: &mut VisualTestContext| cx.read(|cx| state.read(cx).columns[0].width);

        // The edge after the names, at 201.
        cx.simulate_mouse_move(point(px(202.), px(18.)), None, none);
        cx.simulate_mouse_down(point(px(202.), px(18.)), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(262.), px(18.)), MouseButton::Left, none);

        assert_eq!(width(cx), px(260.));

        // No narrower than its least.
        cx.simulate_mouse_move(point(px(0.), px(18.)), MouseButton::Left, none);

        assert_eq!(width(cx), px(48.));

        cx.simulate_mouse_up(point(px(0.), px(18.)), MouseButton::Left, none);

        // The drag sorted nothing.
        assert_eq!(cx.read(|cx| state.read(cx).sort), None);
    }
}
