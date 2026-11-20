//! A long conversation: rows of their own heights, of which only those in
//! view are drawn, that keeps to its end while rows come in or grow, until
//! it is scrolled up; then a button takes it back. It scrolls under a
//! scrollbar, and fades at its foot.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, ElementId, Entity, FollowMode, IntoElement, ListAlignment, ListState,
    ParentElement as _, Pixels, RenderOnce, Styled as _, Window, div, linear_color_stop,
    linear_gradient, list, prelude::FluentBuilder as _, px,
};

use crate::button::Button;
use crate::child_id;
use crate::icon::IconName;
use crate::scroll::Scrollbar;
use crate::size::Sizable as _;
use crate::theme::ActiveTheme as _;

/// How far past the view rows are drawn, so scrolling finds them ready.
const OVERDRAW: Pixels = px(1000.);

/// How tall the fade at the foot is.
const FADE: Pixels = px(24.);

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

    /// The row at `index` changed, and is measured again.
    pub fn remeasure(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.list.item_count() {
            self.list.remeasure_items(index..index + 1);
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

        let row = self.row.clone();
        let rows = list(list_state.clone(), move |index, window, cx| {
            row(index, window, cx)
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
