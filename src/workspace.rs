//! The window attached to a host: the conversation of the current session,
//! the composer, and the status bar.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    bubble::{Bubble, BubbleVariant},
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputEvent, MoveDown, MoveUp, Position, Textarea, TextareaState},
    message::{Message, MessageAlignment, MessageContent, MessageHeader},
    message_scroller::{MessageScroller, MessageScrollerState},
    spinner::Spinner,
    status_bar::StatusBar,
    text::TextView,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, SharedString,
    StyleRefinement, Styled as _, Subscription, Task, WeakEntity, Window, actions, div,
    prelude::FluentBuilder as _,
};

use crate::connection::Incoming;
use crate::conversation::{Change, Conversation, Item, ItemId, Kind, Source, Tool};
use crate::history::History;
use crate::host::Host;
use crate::protocol::{
    self, CODE_INVALID_PARAMS, CancelParams, Event, RpcError, RunParams, RunResult, SteerParams,
};
use crate::usage::Spent;

actions!(workspace, [CancelRun]);

const KEY_CONTEXT: &str = "Workspace";

/// What the composer says Enter will do, with and without a run.
const TASK_HINT: &str = "Give rumbo a task";
const STEER_HINT: &str = "Tell the run something; it reads it before its next request";

/// The keys of the workspace. Escape reaches it after the composer, which
/// lets it through when it has nothing of its own to dismiss.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CancelRun, Some(KEY_CONTEXT))]);
}

/// What the workspace asks of the window around it.
pub enum WorkspaceEvent {
    /// Start a host again, in this session, after the last one ended.
    StartAgain { session: String },
}

/// A request whose answer the workspace waits for.
enum Pending {
    Run,
    /// What the person told the run, shown once the run has it.
    Steer(String),
}

pub struct Workspace {
    host: Host,
    /// What the window is attached to, as the host said last.
    dir: String,
    session: String,
    provider: String,
    model: String,
    conversation: Conversation,
    scroller: Entity<MessageScrollerState>,
    composer: Entity<TextareaState>,
    history: History,
    spent: Spent,
    /// The run in progress, by the id of its run request.
    run: Option<i64>,
    pending: HashMap<i64, Pending>,
    /// The tool calls opened to show their arguments and result.
    expanded: HashSet<ItemId>,
    /// A passing word for the person, shown in the status bar.
    notice: Option<String>,
    /// Why the host is gone, once it is.
    ended: Option<String>,
    focus_handle: FocusHandle,
    /// Where focus goes once the host ended, so Enter starts it again.
    start_again_focus: FocusHandle,
    _reading: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(host: Host, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 8)
                .submit_on_enter(true)
                .placeholder(TASK_HINT)
        });

        let scroller = cx.new(|cx| MessageScrollerState::new(0, cx));

        let subscriptions = vec![
            cx.observe(&scroller, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &composer,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { shift: false, .. } = event {
                        this.submit(window, cx);
                    }
                },
            ),
        ];

        let incoming = host.incoming.clone();

        let reading = cx.spawn_in(window, async move |this, cx| {
            while let Ok(message) = incoming.recv().await {
                let closed = matches!(message, Incoming::Closed);
                let kept = this.update_in(cx, |this, window, cx| this.receive(message, window, cx));

                if kept.is_err() || closed {
                    break;
                }
            }
        });

        let initialize = host.initialize.clone();

        let mut workspace = Workspace {
            host,
            dir: initialize.dir,
            session: initialize.session,
            provider: initialize.provider,
            model: initialize.model,
            conversation: Conversation::default(),
            scroller,
            composer,
            history: History::default(),
            spent: Spent::default(),
            run: None,
            pending: HashMap::new(),
            expanded: HashSet::new(),
            notice: None,
            ended: None,
            focus_handle: cx.focus_handle(),
            start_again_focus: cx.focus_handle(),
            _reading: reading,
            _subscriptions: subscriptions,
        };

        // What the command line already asked for runs without asking again.
        if initialize.resume {
            workspace.start(
                RunParams {
                    resume: true,
                    ..RunParams::default()
                },
                "Continue the unfinished run".into(),
                window,
                cx,
            );
        } else if !initialize.objective.is_empty() {
            let objective = initialize.objective;

            workspace.start(
                RunParams {
                    objective: objective.clone(),
                    ..RunParams::default()
                },
                objective,
                window,
                cx,
            );
        }

        workspace
            .composer
            .update(cx, |state, cx| state.focus(window, cx));

        workspace
    }

    /// Sends what the composer holds: a task, or, during a run, something
    /// for the run to read before its next request.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.composer.read(cx).value().trim().to_string();

        if text.is_empty() || self.ended.is_some() {
            return;
        }

        self.history.remember(&text);
        self.composer
            .update(cx, |state, cx| state.set_value("", window, cx));

        match self.run {
            Some(run) => self.steer(run, text, cx),
            None => self.start(
                RunParams {
                    objective: text.clone(),
                    ..RunParams::default()
                },
                text,
                window,
                cx,
            ),
        }
    }

    fn start(
        &mut self,
        params: RunParams,
        label: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.host.connection.request(protocol::METHOD_RUN, &params) {
            Ok(id) => {
                self.set_run(Some(id), window, cx);
                self.pending.insert(id, Pending::Run);
                self.notice = None;

                let change = self.conversation.push(Kind::Task(label));
                self.show(change, cx);
            }

            Err(error) => {
                self.notice = Some(format!("The task could not reach the host: {error}"));
            }
        }

        cx.notify();
    }

    fn steer(&mut self, run: i64, text: String, cx: &mut Context<Self>) {
        match self.host.connection.request(
            protocol::METHOD_STEER,
            SteerParams {
                run,
                text: text.clone(),
            },
        ) {
            Ok(id) => {
                self.pending.insert(id, Pending::Steer(text));
            }

            Err(error) => {
                self.notice = Some(format!("This could not reach the run: {error}"));
            }
        }

        cx.notify();
    }

    fn cancel(&mut self, _: &CancelRun, _: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.run else {
            return;
        };

        if let Err(error) = self
            .host
            .connection
            .notify(protocol::METHOD_CANCEL, CancelParams { run })
        {
            self.notice = Some(format!("The cancel could not reach the host: {error}"));
        } else {
            self.notice = Some("Cancelling".into());
        }

        cx.notify();
    }

    /// Records whether a run is in progress, and says in the composer what
    /// Enter does now.
    fn set_run(&mut self, run: Option<i64>, window: &mut Window, cx: &mut Context<Self>) {
        self.run = run;

        let hint = if run.is_some() { STEER_HINT } else { TASK_HINT };

        self.composer
            .update(cx, |state, cx| state.set_placeholder(hint, window, cx));
    }

    /// Puts text in the composer with the cursor at its end, where typing
    /// goes on.
    fn fill(&self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        // Columns past the end of a line are clamped to it.
        let end = Position::new(text.matches('\n').count() as u32, u32::MAX);

        self.composer.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.set_cursor_position(end, window, cx);
        });
    }

    /// Brings back an earlier entry when the cursor is on the composer's
    /// first line, or a later one on its last line. Otherwise the arrow
    /// moves the cursor, as usual.
    fn recall(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let state = self.composer.read(cx);
        let text = state.value().to_string();
        let (before, after) = text.split_at(state.cursor().min(text.len()));

        let at_edge = match step {
            -1 => !before.contains('\n'),
            _ => !after.contains('\n'),
        };

        if !at_edge {
            return false;
        }

        let Some(recalled) = self.history.recall(step, &text) else {
            return false;
        };

        self.fill(recalled, window, cx);

        true
    }

    fn receive(&mut self, message: Incoming, window: &mut Window, cx: &mut Context<Self>) {
        match message {
            Incoming::Answer { id, answer } => self.answered(id, answer, window, cx),

            Incoming::Event(event) => self.event(*event, cx),

            // This window calls no provider function yet, so a question
            // can be for none it follows.
            Incoming::Ask { id, .. } => {
                let _ = self.host.connection.refuse(
                    id,
                    CODE_INVALID_PARAMS,
                    "this front end is not following that function, so it cannot answer for it",
                );
            }

            Incoming::Closed => self.closed(window, cx),
        }

        cx.notify();
    }

    fn answered(
        &mut self,
        id: i64,
        answer: Result<serde_json::Value, RpcError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.pending.remove(&id) {
            Some(Pending::Run) => {
                if self.run == Some(id) {
                    self.set_run(None, window, cx);
                    self.notice = None;
                }

                let change = match answer {
                    Ok(result) => {
                        let result: RunResult = serde_json::from_value(result).unwrap_or_default();

                        if !result.session.is_empty() {
                            self.session = result.session;
                        }

                        self.conversation.end(&result.status, &result.error)
                    }

                    Err(error) => self.conversation.push(Kind::Stopped {
                        cancelled: false,
                        reason: error.message,
                    }),
                };

                self.show(change, cx);
            }

            Some(Pending::Steer(text)) => match answer {
                Ok(_) => {
                    let change = self.conversation.push(Kind::Steer(text));
                    self.show(change, cx);
                }

                // The run never reads it, so it goes back to be sent again,
                // unless something new is being typed.
                Err(error) if self.composer.read(cx).value().is_empty() => {
                    self.fill(text, window, cx);
                    self.notice = Some(format!("{}; Enter sends it as a new task", error.message));
                }

                Err(error) => {
                    self.notice = Some(format!("{}: {text}", error.message));
                }
            },

            None => {}
        }
    }

    fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        // A run in a new session names it in its first event.
        if event.run != 0 && Some(event.run) == self.run && !event.session.is_empty() {
            self.session = event.session.clone();
        }

        if event.kind == protocol::EVENT_TURN {
            self.spent.turn(&event);
            return;
        }

        let change = self.conversation.apply(&event);
        self.show(change, cx);
    }

    fn closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let said = self.host.stderr();

        self.ended = Some(if said.is_empty() {
            "The host ended.".into()
        } else {
            format!("The host ended: {said}")
        });

        self.notice = None;
        self.pending.clear();
        self.start_again_focus.focus(window, cx);

        if self.run.is_some() {
            self.set_run(None, window, cx);

            let change = self
                .conversation
                .end(protocol::RUN_FAILED, "The host ended before the run did.");

            self.show(change, cx);
        }
    }

    fn toggle(&mut self, id: ItemId, cx: &mut Context<Self>) {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }

        if let Some(index) = self
            .conversation
            .items()
            .iter()
            .position(|item| item.id == id)
        {
            self.show(Change::Updated(index), cx);
        }

        cx.notify();
    }

    /// Tells the scroller what changed, so it measures only that row.
    fn show(&mut self, change: Change, cx: &mut Context<Self>) {
        match change {
            Change::Nothing => {}

            Change::Appended => self.scroller.update(cx, |state, cx| {
                let _ = state.append(1, cx);
            }),

            Change::Updated(index) => self.scroller.update(cx, |state, cx| {
                let _ = state.remeasure_items(index..index + 1, cx);
            }),
        }
    }

    fn steers_waiting(&self) -> usize {
        self.pending
            .values()
            .filter(|pending| matches!(pending, Pending::Steer(_)))
            .count()
    }

    fn render_conversation(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.conversation.items().is_empty() {
            return v_flex()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .gap_1()
                .child(div().text_lg().child("What should rumbo do?"))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("It works in {}", self.dir)),
                )
                .into_any_element();
        }

        let workspace = cx.weak_entity();

        div()
            .flex_1()
            .min_h_0()
            .child(
                MessageScroller::new(
                    "conversation",
                    self.scroller.clone(),
                    move |index, _, cx| render_row(&workspace, index, cx),
                )
                .with_list_style(StyleRefinement::default().px_4().py_3())
                .with_row_style(StyleRefinement::default().pb_3())
                .with_bottom_fade(cx.theme().background),
            )
            .into_any_element()
    }

    fn render_composer(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.ended.is_some() {
            let session = self.session.clone();

            return h_flex()
                .justify_center()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(
                    Button::new("start-again")
                        .track_focus(&self.start_again_focus)
                        .primary()
                        .icon(IconName::RotateCw)
                        .label("Start Again")
                        .tooltip("Start rumbo again in this session")
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(WorkspaceEvent::StartAgain {
                                session: session.clone(),
                            })
                        })),
                )
                .into_any_element();
        }

        let working = self.run.is_some();

        let send = Button::new("send")
            .primary()
            .icon(IconName::ArrowUp)
            .tooltip(if working {
                "Tell the run (Enter)"
            } else {
                "Send the task (Enter)"
            })
            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)));

        let cancel = Button::new("cancel")
            .outline()
            .icon(IconName::Square)
            .tooltip("Cancel the run (Escape)")
            .on_click(cx.listener(|this, _, window, cx| this.cancel(&CancelRun, window, cx)));

        h_flex()
            .items_end()
            .gap_2()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .capture_action(cx.listener(|this, _: &MoveUp, window, cx| {
                if this.recall(-1, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &MoveDown, window, cx| {
                if this.recall(1, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Textarea::new(&self.composer)),
            )
            .when(working, |row| row.child(cancel))
            .child(send)
            .into_any_element()
    }

    fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let waiting = self.steers_waiting();

        let state = match (&self.ended, &self.notice, self.run) {
            (Some(ended), _, _) => ended.clone(),
            (None, Some(notice), _) => notice.clone(),
            (None, None, Some(_)) if waiting == 1 => {
                "Working; 1 message waits for the next turn".into()
            }
            (None, None, Some(_)) if waiting > 1 => {
                format!("Working; {waiting} messages wait for the next turn")
            }
            (None, None, Some(_)) => "Working".into(),
            (None, None, None) => "Ready".into(),
        };

        let mut bar = StatusBar::new()
            .left(
                h_flex()
                    .gap_1()
                    .when(self.run.is_some(), |row| row.child(Spinner::new().xsmall()))
                    .child(state),
            )
            .child(format!("{} · {}", self.provider, self.model));

        if !self.session.is_empty() {
            bar = bar.right(format!("session {}", self.session));
        }

        if let Some(context) = self.spent.context() {
            bar = bar.right(context);
        }

        let mut tokens = self.spent.tokens();

        if let Some(share) = self.spent.cached_share() {
            tokens.push_str(&format!(" ({share}% from the cache)"));
        }

        bar.right(tokens)
            .border_t_1()
            .border_color(cx.theme().border)
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::cancel))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_conversation(cx))
            .child(self.render_composer(cx))
            .child(self.render_status(cx))
    }
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

impl Drop for Workspace {
    fn drop(&mut self) {
        self.host.stop();
    }
}

/// One row of the conversation. It reads the workspace when the list asks
/// for the row, so the conversation is not copied on every frame.
fn render_row(workspace: &WeakEntity<Workspace>, index: usize, cx: &mut App) -> AnyElement {
    let Some(entity) = workspace.upgrade() else {
        return div().into_any_element();
    };

    let this = entity.read(cx);

    let Some(item) = this.conversation.items().get(index) else {
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
            let open = this.expanded.contains(&item.id);

            row.child(render_tool(workspace.clone(), item, tool, open, cx))
                .into_any_element()
        }

        Kind::Context(sections) => row
            .text_xs()
            .text_color(muted)
            .child(format!("Context sent: {}", sections.join(", ")))
            .into_any_element(),

        Kind::Log { source, text } => {
            let from = match source {
                Source::Host => "host".to_string(),
                Source::Provider => format!("provider {}", this.provider),
            };

            row.text_xs()
                .child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(div().text_color(muted).child(format!("[{from}]")))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_color(cx.theme().warning)
                                .child(text.clone()),
                        ),
                )
                .into_any_element()
        }

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
    item: &Item,
    tool: &Tool,
    open: bool,
    cx: &App,
) -> impl IntoElement {
    let id = item.id;

    let status: AnyElement = match (&tool.result, tool.failed) {
        (None, false) => Spinner::new().xsmall().into_any_element(),
        (None, true) => gpui_kit::component::Icon::new(IconName::CircleX)
            .xsmall()
            .text_color(cx.theme().muted_foreground)
            .into_any_element(),
        (Some(_), false) => gpui_kit::component::Icon::new(IconName::Check)
            .xsmall()
            .text_color(cx.theme().success)
            .into_any_element(),
        (Some(_), true) => gpui_kit::component::Icon::new(IconName::CircleX)
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
                    let _ = workspace.update(cx, |this, cx| this.toggle(id, cx));
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
