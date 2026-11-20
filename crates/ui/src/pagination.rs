//! Pages of something long, one shown at a time: the one before and the one
//! after, the first and the last, and those around the one shown, with a
//! gap for each run of pages left out, so the row keeps its length as the
//! page changes. Tab reaches each, and Enter or Space goes there.

use std::rc::Rc;

use gpui::{
    App, ElementId, IntoElement, ParentElement as _, Refineable as _, RenderOnce, StyleRefinement,
    Styled, Window, div, px,
};

use crate::button::Button;
use crate::icon::{Icon, IconName};
use crate::root::h_flex;
use crate::size::{Sizable, Size};
use crate::theme::ActiveTheme as _;
use crate::{ChangeHandler, child_id};

/// What a pagination shows between its Previous and Next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Page(usize),
    Gap,
}

/// What stands for `total` pages while `page` is shown, counted from 1:
/// the first and the last, `siblings` pages on each side of the one shown,
/// and a gap for each run left out. A gap would stand for one page only
/// where that page fits as well, so it shows instead, and the row is as
/// long whichever page is shown.
fn items(page: usize, total: usize, siblings: usize) -> Vec<Item> {
    if total <= siblings * 2 + 5 {
        return (1..=total).map(Item::Page).collect();
    }

    let page = page.clamp(1, total);
    let start = page
        .saturating_sub(siblings)
        .min(total - siblings * 2 - 2)
        .max(3);
    let end = (page + siblings).max(siblings * 2 + 3).min(total - 2);

    let mut items = vec![Item::Page(1)];

    items.push(match start > 3 {
        true => Item::Gap,
        false => Item::Page(2),
    });
    items.extend((start..=end).map(Item::Page));
    items.push(match end < total - 2 {
        true => Item::Gap,
        false => Item::Page(total - 1),
    });
    items.push(Item::Page(total));

    items
}

#[derive(IntoElement)]
pub struct Pagination {
    id: ElementId,
    page: usize,
    total: usize,
    siblings: usize,
    compact: bool,
    size: Size,
    on_change: Option<ChangeHandler<usize>>,
    style: StyleRefinement,
}

impl Pagination {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Pagination {
            id: id.into(),
            page: 1,
            total: 1,
            siblings: 1,
            compact: false,
            size: Size::Medium,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// The page shown, counted from 1.
    pub fn page(mut self, page: usize) -> Self {
        self.page = page;
        self
    }

    /// How many pages there are.
    pub fn total(mut self, total: usize) -> Self {
        self.total = total;
        self
    }

    /// How many pages show on each side of the one shown; 1 unless said.
    pub fn siblings(mut self, siblings: usize) -> Self {
        self.siblings = siblings;
        self
    }

    /// Previous and Next as arrows alone, without their words.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Called with the page to go to; the caller shows it and gives it back
    /// with `page`.
    pub fn on_change(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Sizable for Pagination {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Pagination {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Pagination {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().colors.muted_foreground;
        let size = self.size;
        let total = self.total;
        let page = self.page.clamp(1, total.max(1));
        let on_change = self.on_change;

        // A button that goes to `to`, unless there is nowhere to go.
        let go = |button: Button, to: Option<usize>| match (to, &on_change) {
            (Some(to), Some(on_change)) => {
                let on_change = on_change.clone();

                button.on_click(move |_, window, cx| on_change(&to, window, cx))
            }
            (Some(_), None) => button,
            (None, _) => button.disabled(true),
        };

        let previous = Button::new(child_id(&self.id, "previous"))
            .ghost()
            .with_size(size)
            .icon(IconName::ChevronLeft);
        let previous = match self.compact {
            true => previous.tooltip("Previous"),
            false => previous.label("Previous"),
        };
        let previous = go(previous, (page > 1).then(|| page - 1));

        let next = Button::new(child_id(&self.id, "next"))
            .ghost()
            .with_size(size)
            .icon(IconName::ChevronRight)
            .icon_end();
        let next = match self.compact {
            true => next.tooltip("Next"),
            false => next.label("Next"),
        };
        let next = go(next, (page < total).then(|| page + 1));

        let mut row = h_flex().gap_1().child(previous);

        for item in items(page, total, self.siblings) {
            row = row.child(match item {
                Item::Page(number) => {
                    let button = Button::new(child_id(&self.id, format!("page-{number}")))
                        .label(number.to_string())
                        .with_size(size)
                        .min_w(size.height())
                        .px(px(6.));

                    match number == page {
                        true => button.outline().into_any_element(),
                        false => go(button.ghost(), Some(number)).into_any_element(),
                    }
                }

                Item::Gap => div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(size.height())
                    .child(Icon::new(IconName::Ellipsis).with_size(size).color(muted))
                    .into_any_element(),
            });
        }

        let mut row = row.child(next);

        row.style().refine(&self.style);
        row
    }
}

#[cfg(test)]
mod tests {
    use super::Item::{Gap, Page};
    use super::items;

    #[test]
    fn shows_every_page_of_a_few() {
        assert_eq!(items(1, 1, 1), [Page(1)]);
        assert_eq!(
            items(4, 7, 1),
            [
                Page(1),
                Page(2),
                Page(3),
                Page(4),
                Page(5),
                Page(6),
                Page(7)
            ]
        );
        assert_eq!(items(1, 0, 1), []);
    }

    #[test]
    fn leaves_gaps_around_the_page_shown() {
        assert_eq!(
            items(10, 20, 1),
            [Page(1), Gap, Page(9), Page(10), Page(11), Gap, Page(20)]
        );
        assert_eq!(
            items(10, 20, 2),
            [
                Page(1),
                Gap,
                Page(8),
                Page(9),
                Page(10),
                Page(11),
                Page(12),
                Gap,
                Page(20)
            ]
        );
    }

    #[test]
    fn keeps_its_length_near_the_ends() {
        assert_eq!(
            items(1, 20, 1),
            [Page(1), Page(2), Page(3), Page(4), Page(5), Gap, Page(20)]
        );
        assert_eq!(
            items(4, 20, 1),
            [Page(1), Page(2), Page(3), Page(4), Page(5), Gap, Page(20)]
        );
        assert_eq!(
            items(5, 20, 1),
            [Page(1), Gap, Page(4), Page(5), Page(6), Gap, Page(20)]
        );
        assert_eq!(
            items(20, 20, 1),
            [
                Page(1),
                Gap,
                Page(16),
                Page(17),
                Page(18),
                Page(19),
                Page(20)
            ]
        );
    }

    #[test]
    fn shows_a_page_where_a_gap_would_stand_for_it_alone() {
        // Pages 3 to 5 around page 4 leave page 2 alone before them.
        assert_eq!(
            items(4, 9, 1),
            [Page(1), Page(2), Page(3), Page(4), Page(5), Gap, Page(9)]
        );

        // Pages 5 to 7 around page 6 leave page 8 alone after them.
        assert_eq!(
            items(6, 9, 1),
            [Page(1), Gap, Page(5), Page(6), Page(7), Page(8), Page(9)]
        );
    }

    #[test]
    fn takes_a_page_past_the_end_as_the_last() {
        assert_eq!(
            items(99, 20, 1),
            [
                Page(1),
                Gap,
                Page(16),
                Page(17),
                Page(18),
                Page(19),
                Page(20)
            ]
        );
    }
}
