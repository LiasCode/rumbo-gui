use gpui::{AnyElement, Context, ParentElement as _, SharedString, Styled as _, Window, div};
use ui::{ActiveTheme as _, Pagination, Sizable as _, v_flex};

use super::{Demos, note, section, told};
use crate::Overview;

pub struct Demo {
    /// The page each example shows.
    pages: [usize; 3],
    told: Vec<SharedString>,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            pages: [1, 12, 2],
            told: Vec::new(),
        }
    }
}

/// How many pages each example has.
const TOTALS: [usize; 3] = [20, 50, 5];

/// A pagination whose page the page keeps, numbered among them.
fn pagination(index: usize, demos: &Demos, cx: &mut Context<Overview>) -> Pagination {
    Pagination::new(("pagination", index))
        .total(TOTALS[index])
        .page(demos.pagination.pages[index])
        .on_change(cx.listener(move |this, page: &usize, _, cx| {
            let demo = &mut this.demos.pagination;

            demo.pages[index] = *page;
            note(&mut demo.told, format!("Page {page} of {}", TOTALS[index]));
            cx.notify();
        }))
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let muted = cx.theme().colors.muted_foreground;
    let shown = |index: usize, demos: &Demos| {
        div().text_sm().text_color(muted).child(format!(
            "Page {} of {}",
            demos.pagination.pages[index], TOTALS[index]
        ))
    };

    vec![
        section(
            "Many pages",
            "The first and the last, those around the page shown, and a gap for each run left out, so the row keeps its length. Tab reaches each, and Enter or Space goes there.",
            v_flex()
                .gap_3()
                .child(pagination(0, demos, cx))
                .child(shown(0, demos)),
            cx,
        ),
        section(
            "Compact and small",
            "Previous and Next as arrows alone, which say what they do when the pointer rests on them.",
            v_flex()
                .gap_3()
                .child(pagination(1, demos, cx).compact().small())
                .child(shown(1, demos)),
            cx,
        ),
        section(
            "A few pages",
            "Every page shows.",
            pagination(2, demos, cx),
            cx,
        ),
        section("What they told", "", told(&demos.pagination.told, cx), cx),
    ]
}
