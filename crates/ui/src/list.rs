//! Items one under another, each after its mark: a number, a letter, or a
//! roman numeral, counted from where the list starts; a bullet or a dash;
//! or a box, ticked for an item done. The numbers line up at their end, in
//! a column as wide as the widest. An item can hold a list of its own, set
//! in under its words, whose bullets change with how deep it is.

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement, Pixels, Refineable as _,
    RenderOnce, SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
    px,
};

use crate::icon::{Icon, IconName};
use crate::root::{h_flex, v_flex};
use crate::size::{Sizable, Size};
use crate::theme::ActiveTheme as _;

/// What marks each item of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListMarker {
    /// 1. 2. 3.
    Decimal,
    /// a. b. c., then aa. ab. past z.
    LowerAlpha,
    /// A. B. C.
    UpperAlpha,
    /// i. ii. iii.
    LowerRoman,
    /// I. II. III.
    UpperRoman,
    /// A dot, and deeper a ring, then a square.
    #[default]
    Bullet,
    Dash,
    /// A box, ticked for an item done.
    Task,
    /// No mark, the items one under another.
    None,
}

/// The letters of `n`, counted from 1 as a sheet's columns are: a to z,
/// then aa, ab. None for 0, which has none.
fn letters(n: usize) -> Option<String> {
    if n == 0 {
        return None;
    }

    let mut n = n;
    let mut letters = Vec::new();

    while n > 0 {
        n -= 1;
        letters.push(b'a' + (n % 26) as u8);
        n /= 26;
    }

    letters.reverse();
    String::from_utf8(letters).ok()
}

/// `n` in roman numerals, from 1 to 3999, which they reach.
fn roman(n: usize) -> Option<String> {
    const NUMERALS: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    if !(1..=3999).contains(&n) {
        return None;
    }

    let mut n = n;
    let mut numeral = String::new();

    for (value, letters) in NUMERALS {
        while n >= value {
            numeral.push_str(letters);
            n -= value;
        }
    }

    Some(numeral)
}

/// The mark of the item counted `n` by a marker that counts. A number with
/// no letters or numeral, such as 0, is written in figures.
fn counted(marker: ListMarker, n: usize) -> Option<String> {
    let mark = match marker {
        ListMarker::Decimal => None,
        ListMarker::LowerAlpha => letters(n),
        ListMarker::UpperAlpha => letters(n).map(|mark| mark.to_uppercase()),
        ListMarker::LowerRoman => roman(n).map(|mark| mark.to_lowercase()),
        ListMarker::UpperRoman => roman(n),
        ListMarker::Bullet | ListMarker::Dash | ListMarker::Task | ListMarker::None => {
            return None;
        }
    };

    Some(format!("{}.", mark.unwrap_or_else(|| n.to_string())))
}

/// The bullets, from the outer list in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bullet {
    Dot,
    Ring,
    Square,
}

/// The bullet of a list `depth` lists in, from 0, round again past the
/// third.
fn bullet(depth: usize) -> Bullet {
    match depth % 3 {
        0 => Bullet::Dot,
        1 => Bullet::Ring,
        _ => Bullet::Square,
    }
}

/// One item of a list: its words, anything under them, and a list of its
/// own.
pub struct ListItem {
    text: Option<SharedString>,
    done: bool,
    children: Vec<AnyElement>,
    nested: Option<List>,
}

impl ListItem {
    pub fn new(text: impl Into<SharedString>) -> Self {
        ListItem {
            text: Some(text.into()),
            done: false,
            children: Vec::new(),
            nested: None,
        }
    }

    /// An item with no words, only what it is given.
    pub fn empty() -> Self {
        ListItem {
            text: None,
            done: false,
            children: Vec::new(),
            nested: None,
        }
    }

    /// In a list of tasks, whether it is done: its box is ticked and its
    /// words struck out.
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    /// A list of its own, set in under its words.
    pub fn nested(mut self, list: List) -> Self {
        self.nested = Some(list);
        self
    }
}

impl ParentElement for ListItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

#[derive(IntoElement)]
pub struct List {
    marker: ListMarker,
    start: usize,
    items: Vec<ListItem>,
    /// How many lists it is in.
    depth: usize,
    size: Size,
    style: StyleRefinement,
}

impl List {
    pub fn new(marker: ListMarker) -> Self {
        List {
            marker,
            start: 1,
            items: Vec::new(),
            depth: 0,
            size: Size::Medium,
            style: StyleRefinement::default(),
        }
    }

    /// Numbered 1., 2., 3.
    pub fn ordered() -> Self {
        List::new(ListMarker::Decimal)
    }

    /// With a bullet before each item.
    pub fn bullet() -> Self {
        List::new(ListMarker::Bullet)
    }

    /// With a box before each item, ticked for one done.
    pub fn tasks() -> Self {
        List::new(ListMarker::Task)
    }

    /// The count of its first item, for a list that counts; 1 unless said.
    pub fn start(mut self, start: usize) -> Self {
        self.start = start;
        self
    }

    pub fn child(mut self, item: ListItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn children(mut self, items: impl IntoIterator<Item = ListItem>) -> Self {
        self.items.extend(items);
        self
    }
}

impl Sizable for List {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for List {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for List {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors;
        let size = self.size;
        let line = size.line_height();
        let marker = self.marker;

        let marks: Vec<Option<String>> = (0..self.items.len())
            .map(|index| counted(marker, self.start + index))
            .collect();

        // The column of marks: as wide as the widest number, as a bullet,
        // or as a box.
        let column = match marker {
            ListMarker::None => None,
            ListMarker::Bullet | ListMarker::Dash => Some(px(12.)),
            ListMarker::Task => Some(px(16.)),
            _ => {
                let style = window.text_style();
                let widest = marks
                    .iter()
                    .flatten()
                    .map(|mark| {
                        let run = style.to_run(mark.len());

                        window
                            .text_system()
                            .shape_line(mark.clone().into(), size.text(), &[run], None)
                            .width
                    })
                    .fold(Pixels::ZERO, Pixels::max);

                Some(widest.ceil())
            }
        };

        let gap = match size {
            Size::XSmall | Size::Small => px(4.),
            Size::Medium | Size::Large => px(6.),
        };

        let depth = self.depth;
        let rows = self.items.into_iter().zip(marks).enumerate();
        let rows = rows.map(|(index, (item, words))| {
            let done = marker == ListMarker::Task && item.done;

            let mark = column.map(|width| {
                let cell = div()
                    .debug_selector(|| format!("mark-{depth}-{index}"))
                    .flex()
                    .flex_none()
                    .items_center()
                    .w(width)
                    .h(line)
                    .text_color(colors.muted_foreground);

                match marker {
                    ListMarker::Bullet => cell.justify_center().child(match bullet(depth) {
                        Bullet::Dot => div().size(px(6.)).rounded_full().bg(colors.foreground),
                        Bullet::Ring => div()
                            .size(px(6.))
                            .rounded_full()
                            .border_1()
                            .border_color(colors.foreground),
                        Bullet::Square => div().size(px(5.)).bg(colors.foreground),
                    }),

                    ListMarker::Dash => cell
                        .justify_center()
                        .child(div().w(px(8.)).h(px(1.5)).bg(colors.muted_foreground)),

                    ListMarker::Task => cell.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(14.))
                            .rounded(px(4.))
                            .border_1()
                            .map(|tick| match done {
                                true => tick.bg(colors.primary).border_color(colors.primary).child(
                                    Icon::new(IconName::Check)
                                        .color(colors.primary_foreground)
                                        .size(px(10.)),
                                ),
                                false => tick.border_color(colors.input),
                            }),
                    ),

                    _ => cell.justify_end().children(words),
                }
            });

            let content = v_flex()
                .debug_selector(|| format!("words-{depth}-{index}"))
                .flex_1()
                .min_w_0()
                .gap(gap)
                .when_some(item.text, |content, text| {
                    content.child(
                        div()
                            .when(done, |words| {
                                words.text_color(colors.muted_foreground).line_through()
                            })
                            .child(text),
                    )
                })
                .children(item.children)
                .when_some(item.nested, |content, list| {
                    content.child(List {
                        depth: depth + 1,
                        size,
                        ..list
                    })
                });

            h_flex().items_start().gap_2().children(mark).child(content)
        });

        let mut list = v_flex()
            .gap(gap)
            .text_size(size.text())
            .line_height(line)
            .children(rows);

        list.style().refine(&self.style);
        list
    }
}

#[cfg(test)]
mod tests {
    use super::{Bullet, ListMarker, bullet, counted, letters, roman};

    #[test]
    fn counts_in_letters_as_a_sheet_counts_its_columns() {
        assert_eq!(letters(1).as_deref(), Some("a"));
        assert_eq!(letters(26).as_deref(), Some("z"));
        assert_eq!(letters(27).as_deref(), Some("aa"));
        assert_eq!(letters(52).as_deref(), Some("az"));
        assert_eq!(letters(53).as_deref(), Some("ba"));
        assert_eq!(letters(703).as_deref(), Some("aaa"));
        assert_eq!(letters(0), None);
    }

    #[test]
    fn counts_in_roman_numerals_as_far_as_they_go() {
        assert_eq!(roman(4).as_deref(), Some("IV"));
        assert_eq!(roman(9).as_deref(), Some("IX"));
        assert_eq!(roman(14).as_deref(), Some("XIV"));
        assert_eq!(roman(1994).as_deref(), Some("MCMXCIV"));
        assert_eq!(roman(3999).as_deref(), Some("MMMCMXCIX"));
        assert_eq!(roman(0), None);
        assert_eq!(roman(4000), None);
    }

    #[test]
    fn marks_each_item_by_its_count() {
        assert_eq!(counted(ListMarker::Decimal, 10).as_deref(), Some("10."));
        assert_eq!(counted(ListMarker::LowerAlpha, 3).as_deref(), Some("c."));
        assert_eq!(counted(ListMarker::UpperAlpha, 28).as_deref(), Some("AB."));
        assert_eq!(counted(ListMarker::LowerRoman, 4).as_deref(), Some("iv."));
        assert_eq!(counted(ListMarker::UpperRoman, 12).as_deref(), Some("XII."));

        // What has no letters or numeral is written in figures.
        assert_eq!(counted(ListMarker::LowerAlpha, 0).as_deref(), Some("0."));
        assert_eq!(
            counted(ListMarker::UpperRoman, 4000).as_deref(),
            Some("4000.")
        );

        // A list that does not count has no words for its marks.
        assert_eq!(counted(ListMarker::Bullet, 1), None);
        assert_eq!(counted(ListMarker::Task, 1), None);
    }

    #[test]
    fn changes_its_bullet_deeper_in() {
        assert_eq!(bullet(0), Bullet::Dot);
        assert_eq!(bullet(1), Bullet::Ring);
        assert_eq!(bullet(2), Bullet::Square);
        assert_eq!(bullet(3), Bullet::Dot);
    }
}

/// Where a list puts its marks and words, in a window GPUI keeps in the
/// test's own process, whose type gives every letter the same width.
#[cfg(test)]
mod layout_tests {
    use gpui::{Context, IntoElement, ParentElement as _, Render, TestAppContext, Window, div};

    use super::{List, ListItem};

    /// Ten numbered items, the second with a list of its own.
    struct Steps;

    impl Render for Steps {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().child(List::ordered().children((1..=10).map(|n| {
                let item = ListItem::new(format!("Step {n}"));

                match n {
                    2 => item.nested(List::bullet().child(ListItem::new("A detail"))),
                    _ => item,
                }
            })))
        }
    }

    #[gpui::test]
    fn lines_up_the_numbers_as_wide_as_the_widest(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);

        let (_, cx) = cx.add_window_view(|_, _| Steps);
        let bounds = |cx: &mut gpui::VisualTestContext, selector: &'static str| {
            cx.debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} was not drawn"))
        };

        let first = bounds(cx, "mark-0-0");
        let tenth = bounds(cx, "mark-0-9");

        // "1." takes the room "10." does, so the words start in one place.
        assert_eq!(first.size.width, tenth.size.width);
        assert!(first.size.width > gpui::px(0.));
        assert_eq!(
            bounds(cx, "words-0-0").left(),
            bounds(cx, "words-0-9").left()
        );

        // A list of an item's own starts where the item's words do.
        assert_eq!(
            bounds(cx, "mark-1-0").left(),
            bounds(cx, "words-0-1").left()
        );
    }
}
