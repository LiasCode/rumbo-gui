use gpui::{
    AnyElement, AppContext as _, Context, Entity, IntoElement as _, ParentElement as _,
    Styled as _, Window, div, px,
};
use ui::{ActiveTheme as _, Button, Message, MessageList, MessageListState, Sizable as _, v_flex};

use super::{Demos, row, section};
use crate::Overview;

/// What the rows of the example say, the person's and the replies in turn.
const SAID: [&str; 6] = [
    "List the files in the project's folder.",
    "There are three: Cargo.toml, README.md, and src/main.rs.",
    "What does src/main.rs do?",
    "It reads the arguments, starts rumbo with them, and opens the window, which closes rumbo when it closes. A long answer wraps, and its row is as tall as it needs.",
    "Run the tests.",
    "They pass.",
];

pub struct Demo {
    list: Entity<MessageListState>,
    rows: Vec<String>,
}

impl Demo {
    pub fn new(cx: &mut Context<Overview>) -> Self {
        let rows: Vec<String> = (0..40).map(|n| SAID[n % SAID.len()].to_string()).collect();

        Demo {
            list: cx.new(|cx| MessageListState::new(rows.len(), cx)),
            rows,
        }
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let overview = cx.weak_entity();

    let conversation = MessageList::new(
        "conversation",
        &demos.message_list.list,
        move |index, _, cx| {
            let Some(overview) = overview.upgrade() else {
                return div().into_any_element();
            };

            let said = overview
                .read(cx)
                .demos
                .message_list
                .rows
                .get(index)
                .cloned()
                .unwrap_or_default();
            let muted = cx.theme().colors.muted_foreground;

            let row = div().px_4().py_2();

            match index % 2 {
                0 => row.child(
                    Message::new()
                        .caption(format!("Task {}", index / 2 + 1))
                        .child(said),
                ),
                _ => row.child(
                    v_flex().gap_1().text_sm().child(said).child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("Row {}", index + 1)),
                    ),
                ),
            }
            .into_any_element()
        },
    );

    let add = Button::new("add-row")
        .outline()
        .small()
        .label("Add a Reply")
        .on_click(cx.listener(|this, _, _, cx| {
            let demo = &mut this.demos.message_list;

            demo.rows
                .push(format!("Reply {} came in.", demo.rows.len() + 1));
            demo.list.update(cx, |list, cx| list.append(1, cx));
        }));

    let grow = Button::new("grow-row")
        .outline()
        .small()
        .label("Grow the Last")
        .on_click(cx.listener(|this, _, _, cx| {
            let demo = &mut this.demos.message_list;
            let last = demo.rows.len() - 1;

            demo.rows[last]
                .push_str(" And it goes on, as a reply does while it streams in, word after word.");
            demo.list.update(cx, |list, cx| list.remeasure(last, cx));
        }));

    let earlier = Button::new("earlier-rows")
        .outline()
        .small()
        .label("Bring Back Earlier")
        .on_click(cx.listener(|this, _, _, cx| {
            let demo = &mut this.demos.message_list;
            let earlier: Vec<String> = (0..4).map(|n| format!("Earlier row {}.", n + 1)).collect();

            demo.rows.splice(0..0, earlier);
            demo.list.update(cx, |list, cx| list.prepend(4, cx));
        }));

    vec![section(
        "A conversation",
        "Only the rows in view are drawn. It keeps to its end while rows come in or grow, until it is scrolled up; then Jump to latest takes it back.",
        v_flex()
            .gap_3()
            .child(row().child(add).child(grow).child(earlier))
            .child(div().w(px(560.)).h(px(420.)).child(conversation)),
        cx,
    )]
}
