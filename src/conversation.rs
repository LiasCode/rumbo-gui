//! What the window shows of a session: the tasks the person gave, what the
//! model answered, the tools it called, and what the host and the provider
//! said. It is built from the protocol's events alone, so the view only
//! draws it.

use serde_json::Value as Json;

use crate::protocol::{self, Event};

/// Identifies an item for as long as the conversation lives, whatever its
/// position.
pub type ItemId = u64;

/// Where a diagnostic comes from, which says who can fix what it reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Host,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    pub name: String,
    /// The call by the argument that identifies it, such as the command.
    pub summary: String,
    /// The arguments as the model gave them, laid out for reading.
    pub args: String,
    /// What the model received, once the call finished.
    pub result: Option<String>,
    /// The call failed, or never finished because its run ended first.
    pub failed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// An objective the person gave, which starts a run.
    Task(String),
    /// What the person told a run while it worked, once the run had it.
    Steer(String),
    /// The model's visible answer, which grows while it streams.
    Reply(String),
    Tool(Tool),
    /// The names of the ambient context sections sent to the model.
    Context(Vec<String>),
    Log {
        source: Source,
        text: String,
    },
    /// A run that did not finish its task, and why.
    Stopped {
        cancelled: bool,
        reason: String,
    },
    /// What a command answered, or why it failed.
    Answer {
        source: Source,
        command: String,
        text: String,
        failed: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: ItemId,
    pub kind: Kind,
}

/// What an event did to the conversation, so a view can update only that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Nothing,
    /// A new item at the end.
    Appended,
    /// The item at this index changed.
    Updated(usize),
}

#[derive(Debug, Default)]
pub struct Conversation {
    items: Vec<Item>,
    next_id: ItemId,
}

impl Conversation {
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn push(&mut self, kind: Kind) -> Change {
        self.next_id += 1;
        self.items.push(Item {
            id: self.next_id,
            kind,
        });

        Change::Appended
    }

    /// Adds what one event of a run says. Events it does not show, such as
    /// a turn's usage, change nothing.
    pub fn apply(&mut self, event: &Event) -> Change {
        match event.kind.as_str() {
            protocol::EVENT_TEXT => self.reply(&event.text),

            protocol::EVENT_TOOL_STARTED => self.push(Kind::Tool(Tool {
                name: event.name.clone(),
                summary: summary(&event.name, event.args.as_ref()),
                args: layout(event.args.as_ref()),
                result: None,
                failed: false,
            })),

            protocol::EVENT_TOOL_FINISHED => self.finish(event),

            // A new objective starts a new run, so the one before is over.
            protocol::EVENT_OBJECTIVE => {
                self.settle();
                self.push(Kind::Task(event.text.clone()))
            }

            protocol::EVENT_STEER => self.push(Kind::Steer(event.text.clone())),

            protocol::EVENT_ENDED => self.end(&event.status, &event.text),

            protocol::EVENT_CONTEXT => self.push(Kind::Context(event.sections.clone())),

            protocol::EVENT_LOG => {
                let source = if event.source == protocol::SOURCE_PROVIDER {
                    Source::Provider
                } else {
                    Source::Host
                };

                self.push(Kind::Log {
                    source,
                    text: event.text.clone(),
                })
            }

            _ => Change::Nothing,
        }
    }

    /// Records how a run ended, when it did not finish its task.
    pub fn end(&mut self, status: &str, error: &str) -> Change {
        self.settle();

        match status {
            protocol::RUN_DONE => Change::Nothing,
            protocol::RUN_CANCELLED => self.push(Kind::Stopped {
                cancelled: true,
                reason: String::new(),
            }),
            _ => self.push(Kind::Stopped {
                cancelled: false,
                reason: error.to_string(),
            }),
        }
    }

    /// Marks the tool calls still open as failed: their run is over, so they
    /// never finish. Their rows keep their height, so nothing needs
    /// measuring again.
    pub fn settle(&mut self) {
        for item in &mut self.items {
            if let Kind::Tool(tool) = &mut item.kind
                && tool.is_open()
            {
                tool.failed = true;
            }
        }
    }

    /// Puts a session's history before what the conversation holds, and
    /// returns how many items that added. Every run a history tells is
    /// over: one in progress started after the history was told.
    pub fn earlier(&mut self, events: &[Event]) -> usize {
        let mut told = Conversation::default();

        for event in events {
            told.apply(event);
        }

        told.settle();

        let count = told.items.len();
        let mut items = Vec::with_capacity(count + self.items.len());

        for item in told.items {
            self.next_id += 1;
            items.push(Item {
                id: self.next_id,
                kind: item.kind,
            });
        }

        items.append(&mut self.items);
        self.items = items;

        count
    }

    /// Streams text into the reply at the end, or starts one.
    fn reply(&mut self, text: &str) -> Change {
        if let Some(Item {
            kind: Kind::Reply(reply),
            ..
        }) = self.items.last_mut()
        {
            reply.push_str(text);

            return Change::Updated(self.items.len() - 1);
        }

        self.push(Kind::Reply(text.to_string()))
    }

    /// Gives its result to the oldest unfinished call of the tool. The
    /// events name the tool but not the call, and calls finish in the
    /// order they started.
    fn finish(&mut self, event: &Event) -> Change {
        let found =
            self.items
                .iter_mut()
                .enumerate()
                .find_map(|(index, item)| match &mut item.kind {
                    Kind::Tool(tool) if tool.name == event.name && tool.is_open() => {
                        Some((index, tool))
                    }
                    _ => None,
                });

        let Some((index, tool)) = found else {
            return Change::Nothing;
        };

        tool.result = Some(event.result.clone());
        tool.failed = event.failed;

        Change::Updated(index)
    }
}

impl Tool {
    /// Whether the call may still finish.
    pub fn is_open(&self) -> bool {
        self.result.is_none() && !self.failed
    }
}

/// The tool and the argument that identifies the call, as rumbo's own front
/// ends show it.
fn summary(name: &str, args: Option<&Json>) -> String {
    let identifying = ["command", "path"]
        .iter()
        .find_map(|key| args?.get(key)?.as_str());

    match identifying {
        Some(value) => format!("{name} {}", first_line(value)),
        None => name.to_string(),
    }
}

/// The first line of text, shortened to fit one line of a view.
pub fn first_line(text: &str) -> String {
    let line = text.trim().lines().next().unwrap_or_default();

    if line.chars().count() > 160 {
        let cut: String = line.chars().take(160).collect();

        return cut + "...";
    }

    line.to_string()
}

/// The arguments for a person to read: each field by name, and text as it
/// is, so a command or a file's content keeps its lines.
fn layout(args: Option<&Json>) -> String {
    let fields = match args {
        None => return String::new(),
        Some(Json::String(text)) => return text.clone(),
        Some(Json::Object(fields)) => fields,
        Some(args) => return serde_json::to_string_pretty(args).unwrap_or_default(),
    };

    let lines: Vec<String> = fields
        .iter()
        .map(|(name, value)| match value {
            Json::String(text) if text.contains('\n') => format!("{name}:\n{text}"),
            Json::String(text) => format!("{name}: {text}"),
            value => format!("{name}: {value}"),
        })
        .collect();

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn event(value: Json) -> Event {
        serde_json::from_value(value).unwrap()
    }

    fn kinds(conversation: &Conversation) -> Vec<Kind> {
        conversation
            .items()
            .iter()
            .map(|item| item.kind.clone())
            .collect()
    }

    #[test]
    fn streams_text_into_one_reply_until_something_else_comes() {
        let mut conversation = Conversation::default();

        assert_eq!(
            conversation.push(Kind::Task("say hi".into())),
            Change::Appended
        );
        assert_eq!(
            conversation.apply(&event(json!({"type": "text", "text": "Hi"}))),
            Change::Appended
        );
        assert_eq!(
            conversation.apply(&event(json!({"type": "text", "text": " there"}))),
            Change::Updated(1)
        );
        assert_eq!(
            conversation.apply(&event(
                json!({"type": "context", "sections": ["environment"]})
            )),
            Change::Appended
        );
        assert_eq!(
            conversation.apply(&event(json!({"type": "text", "text": "Again"}))),
            Change::Appended
        );

        assert_eq!(
            kinds(&conversation),
            [
                Kind::Task("say hi".into()),
                Kind::Reply("Hi there".into()),
                Kind::Context(vec!["environment".into()]),
                Kind::Reply("Again".into()),
            ]
        );
    }

    #[test]
    fn gives_each_result_to_the_oldest_open_call() {
        let mut conversation = Conversation::default();

        conversation.apply(&event(
            json!({"type": "tool_started", "name": "bash", "args": {"command": "ls\nwc -l"}}),
        ));
        conversation.apply(&event(
            json!({"type": "tool_started", "name": "read", "args": {"path": "a.txt"}}),
        ));
        conversation.apply(&event(
            json!({"type": "tool_started", "name": "bash", "args": {"command": "false"}}),
        ));

        assert_eq!(
            conversation.apply(&event(
                json!({"type": "tool_finished", "name": "bash", "result": "a.txt"})
            )),
            Change::Updated(0)
        );
        assert_eq!(
            conversation.apply(&event(
                json!({"type": "tool_finished", "name": "bash", "result": "exit 1", "failed": true})
            )),
            Change::Updated(2)
        );
        assert_eq!(
            conversation.apply(&event(
                json!({"type": "tool_finished", "name": "write", "result": "?"})
            )),
            Change::Nothing
        );

        let Kind::Tool(first) = &conversation.items()[0].kind else {
            panic!("not a tool");
        };

        assert_eq!(first.summary, "bash ls");
        assert_eq!(first.result.as_deref(), Some("a.txt"));
        assert!(!first.failed);
        assert_eq!(first.args, "command:\nls\nwc -l");
        assert!(matches!(&conversation.items()[2].kind, Kind::Tool(tool) if tool.failed));
    }

    #[test]
    fn tells_logs_apart_by_source_and_skips_what_it_does_not_show() {
        let mut conversation = Conversation::default();

        conversation.apply(&event(
            json!({"type": "log", "source": "provider", "text": "rate limited"}),
        ));
        conversation.apply(&event(
            json!({"type": "log", "source": "host", "text": "retrying"}),
        ));

        assert_eq!(
            conversation.apply(&event(json!({"type": "turn", "usage": {"input": 1}}))),
            Change::Nothing
        );
        assert_eq!(
            conversation.apply(&event(json!({"type": "unknown"}))),
            Change::Nothing
        );

        assert_eq!(
            kinds(&conversation),
            [
                Kind::Log {
                    source: Source::Provider,
                    text: "rate limited".into()
                },
                Kind::Log {
                    source: Source::Host,
                    text: "retrying".into()
                },
            ]
        );
    }

    #[test]
    fn records_only_the_runs_that_did_not_finish() {
        let mut conversation = Conversation::default();

        assert_eq!(conversation.end("done", ""), Change::Nothing);

        conversation.apply(&event(json!({"type": "tool_started", "name": "bash"})));

        conversation.end("cancelled", "");
        conversation.end("failed", "the provider is gone");

        assert_eq!(
            kinds(&conversation)[1..],
            [
                Kind::Stopped {
                    cancelled: true,
                    reason: String::new()
                },
                Kind::Stopped {
                    cancelled: false,
                    reason: "the provider is gone".into()
                },
            ]
        );
        assert!(
            matches!(&conversation.items()[0].kind, Kind::Tool(tool) if tool.failed && tool.result.is_none())
        );
    }

    #[test]
    fn tells_a_history_before_what_happens_now() {
        let mut conversation = Conversation::default();

        conversation.push(Kind::Task("now".into()));

        let history = [
            event(json!({"type": "objective", "text": "before"})),
            event(json!({"type": "tool_started", "name": "bash"})),
            event(json!({"type": "steer", "text": "faster"})),
            event(json!({"type": "objective", "text": "again"})),
            event(json!({"type": "text", "text": "done"})),
            event(json!({"type": "ended", "status": "failed", "text": "no credit"})),
        ];

        assert_eq!(conversation.earlier(&history), 6);

        let kinds = kinds(&conversation);

        assert_eq!(kinds[0], Kind::Task("before".into()));
        assert!(matches!(&kinds[1], Kind::Tool(tool) if tool.failed && tool.result.is_none()));
        assert_eq!(kinds[2], Kind::Steer("faster".into()));
        assert_eq!(kinds[3], Kind::Task("again".into()));
        assert_eq!(
            kinds[5],
            Kind::Stopped {
                cancelled: false,
                reason: "no credit".into()
            }
        );
        assert_eq!(kinds[6], Kind::Task("now".into()));

        let mut ids: Vec<ItemId> = conversation.items().iter().map(|item| item.id).collect();
        ids.sort();
        ids.dedup();

        assert_eq!(ids.len(), 7, "every item keeps an id of its own");
    }

    #[test]
    fn a_closed_call_takes_no_result() {
        let mut conversation = Conversation::default();

        conversation.apply(&event(json!({"type": "tool_started", "name": "bash"})));
        conversation.end("cancelled", "");
        conversation.apply(&event(json!({"type": "tool_started", "name": "bash"})));

        assert_eq!(
            conversation.apply(&event(
                json!({"type": "tool_finished", "name": "bash", "result": "ok"})
            )),
            Change::Updated(2)
        );
    }

    #[test]
    fn lays_out_arguments_by_field() {
        assert_eq!(layout(None), "");
        assert_eq!(layout(Some(&json!("raw"))), "raw");
        assert_eq!(
            layout(Some(
                &json!({"content": "one\ntwo", "offset": 3, "path": "a.txt"})
            )),
            "content:\none\ntwo\noffset: 3\npath: a.txt"
        );
    }

    #[test]
    fn shortens_long_first_lines() {
        let long = "x".repeat(200);

        assert_eq!(first_line("  one\ntwo"), "one");
        assert_eq!(first_line(&long).chars().count(), 163);
    }
}
