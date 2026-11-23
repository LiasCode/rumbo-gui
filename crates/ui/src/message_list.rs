//! A long conversation: rows of their own heights, of which only those in
//! view are drawn, that keeps to its end while rows come in or grow, until
//! it is scrolled up; then a button takes it back. It scrolls under a
//! scrollbar, and fades at its foot.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, ElementId, Entity, FollowMode, IntoElement, ListAlignment, ListState,
    ParentElement as _, Pixels, Rems, RenderOnce, Styled as _, Window, div, linear_color_stop,
    linear_gradient, list, prelude::FluentBuilder as _, px,
};

use crate::button::Button;
use crate::child_id;
use crate::icon::IconName;
use crate::scroll::Scrollbar;
use crate::size::{Sizable as _, rems_from_px};
use crate::theme::ActiveTheme as _;

/// How far past the view rows are drawn, so scrolling finds them ready.
const OVERDRAW: Pixels = px(1000.);

/// How tall the fade at the foot is.
const FADE: Rems = rems_from_px(24.);

/// The rows of a conversation, and where it is scrolled. Each view of a
/// conversation keeps one, so each keeps its place.
pub struct MessageListState {
    list: ListState,
}

impl MessageListState {
    /// `count` rows, shown from their end.
    pub fn new(count: usize, _: &mut Context<Self>) -> Self {
        let list = ListState::new(count, ListAlignment::Top, OVERDRAW);

        list.set_follow_mode(FollowMode::Tail);
        list.scroll_to_end();

        MessageListState { list }
    }

    pub fn count(&self) -> usize {
        self.list.item_count()
    }

    /// Other rows, `count` of them, shown from their end.
    pub fn reset(&mut self, count: usize, cx: &mut Context<Self>) {
        self.list.reset(count);
        self.list.set_follow_mode(FollowMode::Tail);
        self.list.scroll_to_end();
        cx.notify();
    }

    /// `count` rows after the others.
    pub fn append(&mut self, count: usize, cx: &mut Context<Self>) {
        let end = self.list.item_count();

        self.list.splice(end..end, count);
        cx.notify();
    }

    /// `count` rows before the others, which stay where they are.
    pub fn prepend(&mut self, count: usize, cx: &mut Context<Self>) {
        self.list.splice(0..0, count);
        cx.notify();
    }

    /// `count` rows at `index`, before those that were there.
    pub fn insert(&mut self, index: usize, count: usize, cx: &mut Context<Self>) {
        let index = index.min(self.list.item_count());

        self.list.splice(index..index, count);
        cx.notify();
    }

    /// The row at `index` is gone; those after it close up.
    pub fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.list.item_count() {
            self.list.splice(index..index + 1, 0);
            cx.notify();
        }
    }

    /// Replaces a range of rows without moving the scroll position of rows outside it.
    pub fn replace(&mut self, rows: std::ops::Range<usize>, count: usize, cx: &mut Context<Self>) {
        self.list.splice(rows, count);
        cx.notify();
    }

    /// The row at `index` changed, and is measured again.
    pub fn remeasure(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.list.item_count() {
            self.list.remeasure_items(index..index + 1);
            cx.notify();
        }
    }

    /// Jumps to the start of a row without following new output.
    pub fn jump_to(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.list.item_count() {
            self.list.scroll_to(gpui::ListOffset {
                item_ix: index,
                offset_in_item: px(0.),
            });
            cx.notify();
        }
    }

    /// Back to the end, keeping to it again.
    pub fn scroll_to_end(&mut self, cx: &mut Context<Self>) {
        self.list.set_follow_mode(FollowMode::Tail);
        self.list.scroll_to_end();
        cx.notify();
    }

    /// Whether it was scrolled up from its end.
    pub fn is_scrolled_up(&self) -> bool {
        !self.list.is_following_tail() && self.list.is_scrolled_to_end() == Some(false)
    }
}

type Row = Rc<dyn Fn(usize, &mut Window, &mut App) -> AnyElement>;

#[derive(IntoElement)]
pub struct MessageList {
    id: ElementId,
    state: Entity<MessageListState>,
    row: Row,
}

impl MessageList {
    /// Draws the row at each index with `row`, which reads it from where
    /// the conversation is kept.
    pub fn new(
        id: impl Into<ElementId>,
        state: &Entity<MessageListState>,
        row: impl Fn(usize, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        MessageList {
            id: id.into(),
            state: state.clone(),
            row: Rc::new(row),
        }
    }
}

impl RenderOnce for MessageList {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors;
        let (list_state, scrolled_up) = {
            let state = self.state.read(cx);

            (state.list.clone(), state.is_scrolled_up())
        };

        // Each row is laid out on its own, where auto margins have nothing to
        // center in; as wide as the list, a row can keep to its middle.
        let row = self.row.clone();
        let rows = list(list_state.clone(), move |index, window, cx| {
            div()
                .w_full()
                .child(row(index, window, cx))
                .into_any_element()
        })
        .size_full();

        let fade = div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(FADE)
            .bg(linear_gradient(
                180.,
                linear_color_stop(colors.background.opacity(0.), 0.),
                linear_color_stop(colors.background, 1.),
            ));

        let back = self.state.clone();

        div()
            .relative()
            .size_full()
            .child(rows)
            .child(fade)
            .child(Scrollbar::vertical(
                child_id(&self.id, "scrollbar"),
                list_state,
            ))
            .when(scrolled_up, |area| {
                area.child(
                    div()
                        .absolute()
                        .bottom_4()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            Button::new(child_id(&self.id, "latest"))
                                .outline()
                                .small()
                                .icon(IconName::ArrowDown)
                                .label("Jump to latest")
                                .shadow_sm()
                                .on_click(move |_, _, cx| {
                                    back.update(cx, |state, cx| state.scroll_to_end(cx))
                                }),
                        ),
                )
            })
    }
}

/// Where a conversation's rows stand, in a window GPUI keeps in the test's
/// own process.
#[cfg(test)]
mod tests {
    use gpui::{
        AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
        Render, Styled as _, TestAppContext, Window, div, px,
    };

    use super::{MessageList, MessageListState};

    /// A conversation 400 wide whose rows keep to the middle, at most 200
    /// wide.
    struct Conversation {
        state: Entity<MessageListState>,
    }

    impl Render for Conversation {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().w(px(400.)).h(px(300.)).child(MessageList::new(
                "conversation",
                &self.state,
                |_, _, _| {
                    div()
                        .debug_selector(|| "row".into())
                        .max_w(px(200.))
                        .mx_auto()
                        .h(px(40.))
                        .into_any_element()
                },
            ))
        }
    }

    #[gpui::test]
    fn lets_a_row_keep_to_the_middle(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);

        let state = cx.new(|cx| MessageListState::new(1, cx));
        let shown = state.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Conversation { state: shown });

        cx.run_until_parked();

        let row = cx.debug_bounds("row").expect("the row is drawn");

        assert_eq!(row.origin.x, px(100.));
        assert_eq!(row.size.width, px(200.));
    }

    #[gpui::test]
    fn jumps_to_an_older_row_and_stops_following_until_asked(cx: &mut TestAppContext) {
        let state = cx.new(|cx| MessageListState::new(100, cx));

        state.update(cx, |state, cx| {
            state.jump_to(12, cx);
            assert_eq!(state.list.logical_scroll_top().item_ix, 12);
            assert!(!state.list.is_following_tail());

            state.append(1, cx);
            assert_eq!(state.list.logical_scroll_top().item_ix, 12);

            state.scroll_to_end(cx);
            assert!(state.list.is_following_tail());
        });
    }

    #[gpui::test]
    fn takes_rows_in_and_out_anywhere(cx: &mut TestAppContext) {
        let state = cx.new(|cx| MessageListState::new(2, cx));

        state.update(cx, |state, cx| {
            state.insert(1, 1, cx);
            assert_eq!(state.count(), 3);

            state.remove(0, cx);
            assert_eq!(state.count(), 2);

            // Past the end, nothing is taken out, and rows go in at the end.
            state.remove(5, cx);
            assert_eq!(state.count(), 2);

            state.insert(9, 2, cx);
            assert_eq!(state.count(), 4);
        });
    }
}
