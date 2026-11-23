//! The rows of a session's conversation: what the person asked, with ways
//! to copy it, change it, or ask it again in a branch; what they told a
//! run, faded until the run reads it; what the model wrote; each tool call,
//! which opens to show what it sent and got back, and what it changed in a
//! file; and how runs ended.

use gpui::{
    AnyElement, App, ClipboardItem, Div, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity,
    div, prelude::FluentBuilder as _,
};
use ui::{
    ActiveTheme as _, Button, Icon, IconName, Markdown, Message, Sizable as _, Spinner, h_flex,
    v_flex,
};

use super::Workspace;
use crate::conversation::{Item, Kind, Line, RowGroup, Source, Tool};
use crate::picture::Picture;
use crate::protocol;

/// The most lines of a change shown under its call; the rest are counted.
const CHANGE_LINES: usize = 40;

/// The most lines of a result shown when a call is opened.
const RESULT_LINES: usize = 60;

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

    let Some(group) = view.groups.get(index) else {
        return div().into_any_element();
    };

    let Some((item, waiting)) = view.conversation.row(group.start) else {
        return div().into_any_element();
    };

    let row = div().id(("item", item.id)).w_full().min_w_0();
    let colors = cx.theme().colors;
    let muted = colors.muted_foreground;

    match &item.kind {
        Kind::Task(text, images) => {
            let number = view.conversation.task_number(item.id);
            let can_branch = this.host_function(protocol::FUNCTION_BRANCH).is_some()
                && this.ended.is_none()
                && !key.is_empty();

            row.child(said_by_person(text, images, None))
                .child(task_actions(
                    workspace.clone(),
                    item.id,
                    text,
                    images,
                    number.filter(|_| can_branch),
                ))
                .into_any_element()
        }

        // Until the run reads it, it shows faded, saying so.
        Kind::Steer(text, images) if waiting => row
            .opacity(0.6)
            .child(said_by_person(
                text,
                images,
                Some("Waiting for the run's next turn"),
            ))
            .into_any_element(),

        Kind::Steer(text, images) => row
            .child(said_by_person(text, images, Some("Told the run")))
            .into_any_element(),

        Kind::Reply(text) => {
            let group = SharedString::from(format!("reply-{}", item.id));

            row.group(group.clone())
                .child(
                    Markdown::new(("reply", item.id), SharedString::from(text.clone()))
                        .base(this.dir.clone()),
                )
                .child(
                    h_flex()
                        .pt_1()
                        .opacity(0.)
                        .group_hover(group, |style| style.opacity(1.))
                        .child(copy_button(("copy-reply", item.id), text.clone())),
                )
                .into_any_element()
        }

        Kind::Tool(_) => row
            .child(render_tools(workspace, key, view, group, cx))
            .into_any_element(),

        Kind::Context(sections) => row
            .child(note(
                IconName::BookOpen,
                format!("Context sent: {}", sections.join(", ")),
                muted,
            ))
            .into_any_element(),

        Kind::Log { source, text } => row
            .text_xs()
            .child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(muted)
                            .child(this.label(*source)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(match source {
                                Source::Provider => colors.warning,
                                Source::Host => muted,
                            })
                            .child(text.clone()),
                    ),
            )
            .into_any_element(),

        Kind::Stopped {
            cancelled: true, ..
        } => row
            .child(note(IconName::CircleStop, "Cancelled".into(), muted))
            .into_any_element(),

        Kind::Stopped { reason, .. } => row
            .child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.danger.opacity(0.4))
                    .bg(colors.danger.opacity(0.06))
                    .text_sm()
                    .child(Icon::new(IconName::CircleX).small().color(colors.danger))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(div().child("The run failed"))
                            .child(div().text_xs().text_color(muted).child(reason.clone())),
                    ),
            )
            .into_any_element(),

        Kind::Finished { seconds, tools } => row
            .child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .text_color(muted)
                    .child(div().h_px().flex_1().bg(colors.border))
                    .child(
                        Icon::new(IconName::CircleCheck)
                            .xsmall()
                            .color(colors.success),
                    )
                    .child(finished(*seconds, *tools))
                    .child(div().h_px().flex_1().bg(colors.border)),
            )
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
                            .bg(colors.muted)
                            .font_family(cx.theme().mono_family.clone())
                            .text_xs()
                            .when(*failed, |block| block.text_color(colors.danger))
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

/// How long a run that finished took, and the tools it called.
pub(super) fn finished(seconds: u64, tools: usize) -> String {
    let took = match seconds {
        0 => "in under a second".to_string(),
        seconds if seconds < 60 => format!("in {seconds}s"),
        seconds => format!("in {}m {}s", seconds / 60, seconds % 60),
    };

    match tools {
        0 => format!("Done {took}"),
        1 => format!("Done {took} · 1 tool call"),
        tools => format!("Done {took} · {tools} tool calls"),
    }
}

fn note(icon: IconName, text: String, color: Hsla) -> impl IntoElement {
    h_flex()
        .gap_2()
        .text_xs()
        .text_color(color)
        .child(Icon::new(icon).xsmall())
        .child(text)
}

fn copy_button(id: (&'static str, u64), text: String) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(IconName::Copy)
        .tooltip("Copy")
        .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
}

/// What can be done with something the person asked: copy it, and, in a
/// session the host can branch, change it or ask it again in a new branch
/// that starts before it, leaving this session as it is.
fn task_actions(
    workspace: WeakEntity<Workspace>,
    id: u64,
    text: &str,
    images: &[Picture],
    point: Option<usize>,
) -> impl IntoElement {
    let group = SharedString::from(format!("task-{id}"));
    let editing = workspace.clone();
    let edited = (text.to_string(), images.to_vec());
    let retried = edited.clone();

    h_flex()
        .id(("task-actions", id))
        .group(group.clone())
        .w_full()
        .pt_1()
        .justify_end()
        .gap_1()
        .opacity(0.)
        .hover(|style| style.opacity(1.))
        .group_hover(group, |style| style.opacity(1.))
        .child(copy_button(("copy-task", id), text.to_string()))
        .when_some(point, move |row, point| {
            row.child(
                Button::new(("edit-task", id))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Pencil)
                    .tooltip("Change it, in a branch that starts before it")
                    .on_click(move |_, window, cx| {
                        let (text, images) = edited.clone();

                        let _ = editing.update(cx, |this, cx| {
                            this.branch(Some(point), Some((text, images, false)), window, cx)
                        });
                    }),
            )
            .child(
                Button::new(("retry-task", id))
                    .ghost()
                    .xsmall()
                    .icon(IconName::RotateCcw)
                    .tooltip("Ask it again, in a branch that starts before it")
                    .on_click(move |_, window, cx| {
                        let (text, images) = retried.clone();

                        let _ = workspace.update(cx, |this, cx| {
                            this.branch(Some(point), Some((text, images, true)), window, cx)
                        });
                    }),
            )
        })
}

fn said_by_person(
    text: &str,
    images: &[Picture],
    caption: Option<&'static str>,
) -> impl IntoElement {
    let message = Message::new();

    match caption {
        Some(caption) => message.caption(caption),
        None => message,
    }
    .when(!text.is_empty(), |message| message.child(text.to_string()))
    .when(!images.is_empty(), |message| {
        message.child(
            h_flex()
                .flex_wrap()
                .gap_2()
                .when(!text.is_empty(), |row| row.pt_2())
                .children(images.iter().map(|image| image.shown(320., 240.))),
        )
    })
}

/// A compact heading for the calls the agent made without speaking in between.
fn render_tools(
    workspace: &WeakEntity<Workspace>,
    key: &str,
    view: &super::SessionView,
    group: &RowGroup,
    cx: &App,
) -> impl IntoElement {
    let items = &view.conversation.items()[group.start..group.start + group.len];
    let mut tools = items.iter().filter_map(|item| match &item.kind {
        Kind::Tool(tool) => Some((item, tool)),
        _ => None,
    });
    let mut names = tools.clone().map(|(_, tool)| tool.name.as_str());
    let first = names.next().unwrap_or_default();
    let activity = if names.all(|name| name == first) {
        match first {
            "read" => "Read files",
            "write" => "Write files",
            "edit" => "Edit files",
            "bash" => "Run commands",
            _ => "Tool activity",
        }
    } else {
        "Tool activity"
    };

    let count = if group.len == 1 {
        "1 call".to_string()
    } else {
        format!("{} calls", group.len)
    };
    let working = tools.clone().any(|(_, tool)| tool.is_open());
    let failed = tools.clone().filter(|(_, tool)| tool.failed).count();
    let colors = cx.theme().colors;
    let id = group.first;
    let open = view.open_groups.contains(&id);
    let target = workspace.clone();
    let session = key.to_string();
    let heading = Button::new(("tool-group", id))
        .ghost()
        .small()
        .icon(if open {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        })
        .label(format!("{activity} · {count}"))
        .on_click(move |_, _, cx| {
            let _ = target.update(cx, |this, cx| this.toggle(&session, id, true, cx));
        });

    v_flex()
        .gap_1()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(heading)
                .when(working, |row| row.child(Spinner::new().xsmall()))
                .when(failed > 0, |row| {
                    row.child(
                        div()
                            .text_xs()
                            .text_color(colors.danger)
                            .child(format!("{failed} failed")),
                    )
                }),
        )
        .map(|group| {
            if open {
                group.child(v_flex().pl_3().gap_1().children(tools.map(|(item, tool)| {
                    render_tool(
                        workspace.clone(),
                        key,
                        item,
                        tool,
                        view.expanded.contains(&item.id),
                        cx,
                    )
                })))
            } else {
                group.children(tools.next_back().map(|(_, tool)| {
                    tool_line(tool, cx)
                        .pl_5()
                        .text_color(colors.muted_foreground)
                }))
            }
        })
}

/// The icon a tool is shown with, by what it does.
fn tool_icon(name: &str) -> IconName {
    match name {
        "bash" => IconName::SquareTerminal,
        "read" => IconName::FileText,
        "write" => IconName::FilePlus,
        "edit" => IconName::FilePen,
        _ => IconName::Wrench,
    }
}

fn tool_line(tool: &Tool, cx: &App) -> Div {
    let colors = cx.theme().colors;
    let muted = colors.muted_foreground;

    let status: AnyElement = match (&tool.result, tool.failed) {
        (None, false) => Spinner::new().xsmall().into_any_element(),
        (None, true) => Icon::new(IconName::CircleX)
            .xsmall()
            .color(muted)
            .into_any_element(),
        (Some(_), false) => Icon::new(IconName::Check)
            .xsmall()
            .color(colors.success)
            .into_any_element(),
        (Some(_), true) => Icon::new(IconName::CircleX)
            .xsmall()
            .color(colors.danger)
            .into_any_element(),
    };

    h_flex()
        .w_full()
        .min_w_0()
        .gap_2()
        .px_2()
        .py_1()
        .child(Icon::new(tool_icon(&tool.name)).small().color(muted))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_sm()
                .font_family(cx.theme().mono_family.clone())
                .child(tool.summary.clone()),
        )
        .child(status)
}

/// A tool call as one line that opens to show what the model sent and what
/// it got back. What it changes in a file shows under it, open or not.
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
    let colors = cx.theme().colors;
    let muted = colors.muted_foreground;
    let hovered = colors.muted;

    let header = tool_line(tool, cx)
        .id(("tool", id))
        .rounded_md()
        .cursor_pointer()
        .hover(move |style| style.bg(hovered))
        .on_click(move |_, _, cx| {
            let _ = workspace.update(cx, |this, cx| this.toggle(&key, id, false, cx));
        })
        .child(
            Icon::new(if open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .xsmall()
            .color(muted),
        );

    let block = |label: &'static str, text: String, failed: bool| {
        v_flex()
            .gap_1()
            .child(div().text_xs().text_color(muted).child(label))
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(colors.muted)
                    .font_family(cx.theme().mono_family.clone())
                    .text_xs()
                    .when(failed, |block| block.text_color(colors.danger))
                    .child(text),
            )
    };

    let details = v_flex()
        .gap_2()
        .pl_8()
        .when(!tool.args.is_empty() && tool.change.is_empty(), |details| {
            details.child(block("Arguments", tool.args.clone(), false))
        })
        .when_some(tool.result.clone(), |details, result| {
            details.child(block(
                if tool.failed { "Failed with" } else { "Result" },
                shortened(&result),
                tool.failed,
            ))
        });

    v_flex()
        .gap_1()
        .child(header)
        .when(!tool.change.is_empty(), |column| {
            column.child(render_change(&tool.change, cx))
        })
        .when(open, |column| column.child(details))
}

/// What a call changes in a file, as lines taken out and put in.
fn render_change(change: &[Line], cx: &App) -> impl IntoElement {
    let colors = cx.theme().colors;
    let removed = colors.danger;
    let added = colors.success;
    let left = change.len().saturating_sub(CHANGE_LINES);

    v_flex()
        .ml_8()
        .rounded_md()
        .border_1()
        .border_color(colors.border)
        .overflow_hidden()
        .font_family(cx.theme().mono_family.clone())
        .text_xs()
        .children(change.iter().take(CHANGE_LINES).map(|line| {
            let (sign, text, color) = match line {
                Line::Removed(text) => ("-", text, removed),
                Line::Added(text) => ("+", text, added),
            };

            h_flex()
                .px_2()
                .bg(color.opacity(0.08))
                .child(div().w_4().flex_shrink_0().text_color(color).child(sign))
                .child(div().flex_1().min_w_0().truncate().child(text.clone()))
        }))
        .when(left > 0, |column| {
            column.child(
                div()
                    .px_2()
                    .py_1()
                    .text_color(colors.muted_foreground)
                    .child(format!("{left} more lines")),
            )
        })
}

/// The start of a long result, saying how much was left out.
fn shortened(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();

    if lines.len() <= RESULT_LINES {
        return text.to_string();
    }

    format!(
        "{}\n… {} more lines",
        lines[..RESULT_LINES].join("\n"),
        lines.len() - RESULT_LINES
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_how_long_a_run_took_in_words() {
        assert_eq!(finished(0, 0), "Done in under a second");
        assert_eq!(finished(42, 1), "Done in 42s · 1 tool call");
        assert_eq!(finished(125, 7), "Done in 2m 5s · 7 tool calls");
    }
}
