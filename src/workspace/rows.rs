//! The rows of a session's conversation.

use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _,
    bubble::{Bubble, BubbleVariant},
    button::{Button, ButtonVariants as _},
    h_flex,
    message::{Message, MessageAlignment, MessageContent, MessageHeader},
    spinner::Spinner,
    text::TextView,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _, WeakEntity, div, prelude::FluentBuilder as _,
};

use super::Workspace;
use crate::conversation::{Item, Kind, Source, Tool};

/// One row of a session's conversation. It reads the workspace when the
/// list asks for the row, so the conversation is not copied on every frame.
pub(super) fn render_row(
    workspace: &WeakEntity<Workspace>,
    key: &str,
    index: usize,
    cx: &mut App,
) -> AnyElement {
    let Some(entity) = workspace.upgrade() else {
        return div().into_any_element();
    };

    let this = entity.read(cx);

    let Some(view) = this.views.get(key) else {
        return div().into_any_element();
    };

    let Some(item) = view.conversation.items().get(index) else {
        return div().into_any_element();
    };

    let row = div().id(("item", item.id)).w_full().min_w_0();
    let muted = cx.theme().muted_foreground;

    match &item.kind {
        Kind::Task(text) => row.child(said_by_person(text, None)).into_any_element(),

        Kind::Steer(text) => row
            .child(said_by_person(text, Some("Told the run")))
            .into_any_element(),

        Kind::Reply(text) => row
            .child(TextView::markdown(
                ("reply", item.id),
                SharedString::from(text.clone()),
            ))
            .into_any_element(),

        Kind::Tool(tool) => {
            let open = view.expanded.contains(&item.id);

            row.child(render_tool(workspace.clone(), key, item, tool, open, cx))
                .into_any_element()
        }

        Kind::Context(sections) => row
            .text_xs()
            .text_color(muted)
            .child(format!("Context sent: {}", sections.join(", ")))
            .into_any_element(),

        Kind::Log { source, text } => row
            .text_xs()
            .child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .child(div().text_color(muted).child(this.label(*source)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(cx.theme().warning)
                            .child(text.clone()),
                    ),
            )
            .into_any_element(),

        Kind::Stopped {
            cancelled: true, ..
        } => row
            .text_sm()
            .text_color(muted)
            .child("Cancelled")
            .into_any_element(),

        Kind::Stopped { reason, .. } => row
            .text_sm()
            .text_color(cx.theme().danger)
            .child(format!("Failed: {reason}"))
            .into_any_element(),

        Kind::Answer {
            source,
            command,
            text,
            failed,
        } => row
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("{} {command}", this.label(*source))),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(cx.theme().muted)
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_xs()
                            .when(*failed, |block| block.text_color(cx.theme().danger))
                            .child(text.clone()),
                    ),
            )
            .into_any_element(),
    }
}

impl Workspace {
    /// Where a line comes from, as rumbo's front ends mark it.
    fn label(&self, source: Source) -> String {
        match source {
            Source::Host => "[host]".into(),
            Source::Provider => format!("[provider {}]", self.provider),
        }
    }
}

fn said_by_person(text: &str, caption: Option<&'static str>) -> impl IntoElement {
    let mut message = Message::new().alignment(MessageAlignment::End);

    if let Some(caption) = caption {
        message = message.header(MessageHeader::new().child(caption));
    }

    message.content(
        MessageContent::new().bubble(
            Bubble::new()
                .with_variant(BubbleVariant::Muted)
                .child(text.to_string()),
        ),
    )
}

/// A tool call as one line that opens to show what the model sent and what
/// it got back.
fn render_tool(
    workspace: WeakEntity<Workspace>,
    key: &str,
    item: &Item,
    tool: &Tool,
    open: bool,
    cx: &App,
) -> impl IntoElement {
    let id = item.id;
    let key = key.to_string();

    let status: AnyElement = match (&tool.result, tool.failed) {
        (None, false) => Spinner::new().xsmall().into_any_element(),
        (None, true) => Icon::new(IconName::CircleX)
            .xsmall()
            .text_color(cx.theme().muted_foreground)
            .into_any_element(),
        (Some(_), false) => Icon::new(IconName::Check)
            .xsmall()
            .text_color(cx.theme().success)
            .into_any_element(),
        (Some(_), true) => Icon::new(IconName::CircleX)
            .xsmall()
            .text_color(cx.theme().danger)
            .into_any_element(),
    };

    let header = h_flex()
        .gap_1()
        .child(
            Button::new(("tool", id))
                .ghost()
                .xsmall()
                .icon(if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .label(tool.summary.clone())
                .on_click(move |_, _, cx| {
                    let _ = workspace.update(cx, |this, cx| this.toggle(&key, id, cx));
                }),
        )
        .child(status);

    let block = |label: &'static str, text: String| {
        v_flex()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            )
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_xs()
                    .child(text),
            )
    };

    let details = v_flex()
        .gap_2()
        .pl_6()
        .when(!tool.args.is_empty(), |details| {
            details.child(block("Arguments", tool.args.clone()))
        })
        .when_some(tool.result.clone(), |details, result| {
            details.child(block(
                if tool.failed { "Failed with" } else { "Result" },
                result,
            ))
        });

    v_flex()
        .gap_2()
        .child(header)
        .when(open, |column| column.child(details))
}
