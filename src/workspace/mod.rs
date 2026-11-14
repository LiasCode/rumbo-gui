//! The window attached to a host: the sessions of the project, the
//! conversation of the one shown, the composer, and the status bar.
//!
//! The window shows the host's current session. Runs started in other
//! sessions go on in the background, and their events go to the session
//! they work in, so each session's conversation is whole when it is shown
//! again.

mod palette;
mod picker;
mod rows;
mod sidebar;
mod talk;

use std::collections::{HashMap, HashSet};

use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    command::CommandState,
    h_flex,
    input::{InputEvent, MoveDown, MoveUp, Position, Textarea, TextareaState},
    message_scroller::{MessageScroller, MessageScrollerState},
    spinner::Spinner,
    status_bar::StatusBar,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, StyleRefinement,
    Styled as _, Subscription, Task, Window, actions, div, prelude::FluentBuilder as _,
};
use serde_json::Value as Json;

use crate::choices::Choices;
use crate::commands::{self, Command};
use crate::connection::Incoming;
use crate::conversation::{Change, Conversation, ItemId, Kind, Source};
use crate::history::History;
use crate::host::Host;
use crate::protocol::{
    self, CancelParams, DescribeResult, Event, Function, HistoryParams, HistoryResult, HostResult,
    HostState, InvokeParams, RpcError, RunParams, RunResult, SteerParams, Value, ValuesResult,
};
use crate::usage::Spent;

actions!(workspace, [CancelRun, OpenPalette]);

const KEY_CONTEXT: &str = "Workspace";

/// What the composer says Enter will do, with and without a run.
const TASK_HINT: &str = "Give rumbo a task";
const STEER_HINT: &str = "Tell the run something; it reads it before its next request";

/// The keys of the workspace. Escape reaches it after the composer, which
/// lets it through when it has nothing of its own to dismiss.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", CancelRun, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-k", OpenPalette, Some(KEY_CONTEXT)),
    ]);
}

/// What the workspace asks of the window around it.
pub enum WorkspaceEvent {
    /// Start a host again, in this session, after the last one ended.
    StartAgain { session: String },
}

/// A request whose answer the workspace waits for.
enum Pending {
    Run,
    /// What the person told a run, shown once the run has it.
    Steer {
        run: i64,
        text: String,
    },
    /// The history of a session, for its view.
    History(String),
    /// The project's sessions, for the sidebar.
    Sessions,
    /// What the provider offers, for the palette.
    Describe,
    /// What the provider offers, for the picker of the model and options.
    Picker,
    /// A list the provider gives for a row of the picker.
    Values(usize),
    /// A function called for the person. A quiet one, such as opening a
    /// session from the sidebar, says nothing in the conversation unless
    /// it fails.
    Call {
        source: Source,
        name: String,
        quiet: bool,
    },
}

/// What the window keeps of one session.
struct SessionView {
    conversation: Conversation,
    scroller: Entity<MessageScrollerState>,
    spent: Spent,
    /// The run of this window working in the session, by the id of its run
    /// request.
    run: Option<i64>,
    /// The tool calls opened to show their arguments and result.
    expanded: HashSet<ItemId>,
    _scrolling: Subscription,
}

impl SessionView {
    fn new(cx: &mut Context<Workspace>) -> Self {
        let scroller = cx.new(|cx| MessageScrollerState::new(0, cx));
        let scrolling = cx.observe(&scroller, |_, _, cx| cx.notify());

        SessionView {
            conversation: Conversation::default(),
            scroller,
            spent: Spent::default(),
            run: None,
            expanded: HashSet::new(),
            _scrolling: scrolling,
        }
    }
}

pub struct Workspace {
    host: Host,
    /// What the window is attached to, as the host said last.
    dir: String,
    provider: String,
    model: String,
    /// The session shown, which is the host's current one. It is empty for
    /// a new session until its first run makes it.
    current: String,
    views: HashMap<String, SessionView>,
    /// The session each run of this window works in, by the id of its run
    /// request.
    runs: HashMap<i64, String>,
    pending: HashMap<i64, Pending>,
    /// The project's sessions, most recently used first.
    listed: Vec<Value>,
    /// What the host can be asked, whatever the provider.
    host_functions: Vec<Function>,
    /// What the provider can be asked, or why that is not known.
    provider_functions: Result<Vec<Function>, String>,
    composer: Entity<TextareaState>,
    palette: Entity<CommandState>,
    /// The model and options chosen for the runs started from this window.
    choices: Choices,
    /// The picker being made, while the provider's lists arrive.
    picking: Option<picker::Picking>,
    /// The provider's function in progress, called from this window.
    talking: Option<talk::Talking>,
    history: History,
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

        let palette = cx.new(|cx| CommandState::new(window, cx));

        let subscriptions = vec![cx.subscribe_in(
            &composer,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { shift: false, .. } = event {
                    this.submit(window, cx);
                }
            },
        )];

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
            provider: initialize.provider,
            model: initialize.model,
            current: String::new(),
            views: HashMap::new(),
            runs: HashMap::new(),
            pending: HashMap::new(),
            listed: Vec::new(),
            host_functions: initialize.functions,
            provider_functions: Ok(Vec::new()),
            composer,
            palette,
            choices: Choices::default(),
            picking: None,
            talking: None,
            history: History::default(),
            notice: None,
            ended: None,
            focus_handle: cx.focus_handle(),
            start_again_focus: cx.focus_handle(),
            _reading: reading,
            _subscriptions: subscriptions,
        };

        // The history is asked for first: it is answered before the run
        // that follows starts, so nothing of that run is in it.
        workspace.show_session(initialize.session, window, cx);
        workspace.refresh_sessions();
        workspace.describe();

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

    /// The view of a session, made when it is first needed.
    fn view(&mut self, key: &str, cx: &mut Context<Self>) -> &mut SessionView {
        if !self.views.contains_key(key) {
            let view = SessionView::new(cx);
            self.views.insert(key.to_string(), view);
        }

        self.views.get_mut(key).expect("the view was just made")
    }

    /// Shows a session, asking for its history the first time. A new
    /// session starts empty, unless a run is already making it.
    fn show_session(&mut self, session: String, window: &mut Window, cx: &mut Context<Self>) {
        let known = self.views.contains_key(&session);
        let busy = self
            .views
            .get(&session)
            .is_some_and(|view| view.run.is_some());

        if session.is_empty() && known && !busy {
            self.views.remove(&session);
        }

        if !session.is_empty() && !known {
            self.request(
                protocol::METHOD_HISTORY,
                HistoryParams {
                    session: session.clone(),
                },
                Pending::History(session.clone()),
            );
        }

        self.view(&session, cx);
        self.current = session;
        self.set_hint(window, cx);
        cx.notify();
    }

    /// Asks for the project's sessions, which the sidebar lists.
    fn refresh_sessions(&mut self) {
        self.request(
            protocol::METHOD_INVOKE,
            InvokeParams {
                name: "sessions".into(),
                params: None,
                target: protocol::TARGET_HOST,
            },
            Pending::Sessions,
        );
    }

    /// Asks what the provider offers, which the palette lists.
    fn describe(&mut self) {
        self.request(protocol::METHOD_DESCRIBE, Json::Null, Pending::Describe);
    }

    /// Sends a request whose answer the workspace waits for. A request that
    /// cannot be sent means the host is gone, which the reader says.
    fn request(
        &mut self,
        method: &str,
        params: impl serde::Serialize,
        pending: Pending,
    ) -> Option<i64> {
        match self.host.connection.request(method, params) {
            Ok(id) => {
                self.pending.insert(id, pending);
                Some(id)
            }

            Err(error) => {
                self.notice = Some(format!("The host could not be reached: {error}"));
                None
            }
        }
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

        let current = self.current.clone();

        match self.view(&current, cx).run {
            Some(run) => {
                let pending = Pending::Steer {
                    run,
                    text: text.clone(),
                };

                self.request(protocol::METHOD_STEER, SteerParams { run, text }, pending);
            }

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

        cx.notify();
    }

    /// Starts a run in the current session.
    fn start(
        &mut self,
        mut params: RunParams,
        label: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.choices.apply(&mut params);

        let Some(id) = self.request(protocol::METHOD_RUN, &params, Pending::Run) else {
            return;
        };

        let current = self.current.clone();
        let view = self.view(&current, cx);

        view.run = Some(id);

        let change = view.conversation.push(Kind::Task(label));

        self.runs.insert(id, current.clone());
        self.notice = None;
        self.show(&current, change, cx);
        self.set_hint(window, cx);
        cx.notify();
    }

    /// Gives up the provider's function in progress, else cancels the run
    /// of the session shown, as rumbo's terminal UI does with Ctrl-C.
    fn cancel(&mut self, _: &CancelRun, _: &mut Window, cx: &mut Context<Self>) {
        if self.talking.is_some() {
            self.give_up(cx);
            return;
        }

        let Some(run) = self.views.get(&self.current).and_then(|view| view.run) else {
            return;
        };

        if let Err(error) = self
            .host
            .connection
            .notify(protocol::METHOD_CANCEL, CancelParams { run, invoke: 0 })
        {
            self.notice = Some(format!("The cancel could not reach the host: {error}"));
        } else {
            self.notice = Some("Cancelling".into());
        }

        cx.notify();
    }

    /// Calls a function of the host or of the provider for the person.
    fn call(
        &mut self,
        command: &Command,
        params: Option<Json>,
        quiet: bool,
        cx: &mut Context<Self>,
    ) {
        let target = match command.source {
            Source::Host => protocol::TARGET_HOST,
            Source::Provider => "",
        };

        let pending = Pending::Call {
            source: command.source,
            name: command.function.name.clone(),
            quiet,
        };

        let called = self.request(
            protocol::METHOD_INVOKE,
            InvokeParams {
                name: command.function.name.clone(),
                params,
                target,
            },
            pending,
        );

        // A function of the provider may talk with the person while it
        // works; it is followed until it answers.
        if let (Some(invoke), Source::Provider) = (called, command.source) {
            self.talking = Some(talk::Talking {
                invoke,
                name: command.function.name.clone(),
            });
        }

        cx.notify();
    }

    /// Calls a function of the host by name, as the sidebar does, without
    /// an answer in the conversation.
    fn call_host(&mut self, name: &str, params: Option<Json>, cx: &mut Context<Self>) {
        let command = Command {
            source: Source::Host,
            function: Function {
                name: name.into(),
                ..Function::default()
            },
        };

        self.call(&command, params, true, cx);
    }

    /// Records whether the shown session has a run in progress in the
    /// composer, which says what Enter does now.
    fn set_hint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let working = self
            .views
            .get(&self.current)
            .is_some_and(|view| view.run.is_some());
        let hint = if working { STEER_HINT } else { TASK_HINT };

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

            Incoming::Event(event) => self.event(*event, window, cx),

            Incoming::Ask { id, params } => self.asked(id, params, window, cx),

            Incoming::Closed => self.closed(window, cx),
        }

        cx.notify();
    }

    fn answered(
        &mut self,
        id: i64,
        answer: Result<Json, RpcError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.pending.remove(&id) else {
            return;
        };

        match pending {
            Pending::Run => self.run_ended(id, answer, window, cx),

            Pending::Steer { run, text } => self.steered(run, text, answer, window, cx),

            Pending::History(session) => {
                let told: HistoryResult = match answer {
                    Ok(result) => serde_json::from_value(result).unwrap_or_default(),
                    Err(error) => {
                        self.notice =
                            Some(format!("The session's history could not be read: {error}"));
                        return;
                    }
                };

                self.told(&session, &told.events, cx);
            }

            Pending::Sessions => {
                if let Ok(result) = answer {
                    let listed: HostResult = serde_json::from_value(result).unwrap_or_default();
                    self.listed = listed.values;
                }
            }

            Pending::Describe => {
                self.provider_functions = match answer {
                    Ok(result) => {
                        let described: DescribeResult =
                            serde_json::from_value(result).unwrap_or_default();
                        Ok(described.offers.functions)
                    }

                    Err(error) => Err(error.message),
                };
            }

            Pending::Picker => {
                let described = answer
                    .map(|result| serde_json::from_value(result).unwrap_or_default())
                    .map_err(|error| error.message);

                self.described_for_picker(described, window, cx);
            }

            Pending::Values(row) => {
                let listed = answer
                    .map(|result| {
                        let listed: ValuesResult =
                            serde_json::from_value(result).unwrap_or_default();
                        listed.values
                    })
                    .map_err(|error| error.message);

                self.listed_for_picker(row, listed, window, cx);
            }

            Pending::Call {
                source,
                name,
                quiet,
            } => self.call_answered(source, name, quiet, answer, window, cx),
        }
    }

    fn run_ended(
        &mut self,
        id: i64,
        answer: Result<Json, RpcError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self.runs.remove(&id) else {
            return;
        };

        let (status, error, session) = match answer {
            Ok(result) => {
                let result: RunResult = serde_json::from_value(result).unwrap_or_default();
                (result.status, result.error, result.session)
            }

            // A run the host refused never started.
            Err(error) => (
                protocol::RUN_FAILED.to_string(),
                error.message,
                String::new(),
            ),
        };

        // A run that ended before saying anything names its session here.
        let key = if key.is_empty() && !session.is_empty() {
            self.name_session(&session);
            session
        } else {
            key
        };

        let view = self.view(&key, cx);

        view.run = None;

        let change = view.conversation.end(&status, &error);

        self.show(&key, change, cx);

        if key == self.current {
            self.notice = None;
            self.set_hint(window, cx);
        }

        self.refresh_sessions();
    }

    fn steered(
        &mut self,
        run: i64,
        text: String,
        answer: Result<Json, RpcError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match answer {
            Ok(_) => {
                let key = self
                    .runs
                    .get(&run)
                    .cloned()
                    .unwrap_or_else(|| self.current.clone());
                let change = self.view(&key, cx).conversation.push(Kind::Steer(text));

                self.show(&key, change, cx);
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
        }
    }

    /// Puts what a session held before the window opened it in its view.
    fn told(&mut self, session: &str, events: &[Event], cx: &mut Context<Self>) {
        let view = self.view(session, cx);
        let before = view.conversation.items().len();
        let count = view.conversation.earlier(events);

        for event in events
            .iter()
            .filter(|event| event.kind == protocol::EVENT_TURN)
        {
            view.spent.turn(event);
        }

        let total = before + count;

        view.scroller.update(cx, |state, cx| {
            // An empty view starts at its end, where the latest work is.
            if before == 0 {
                state.reset(total, cx);
            } else {
                let _ = state.prepend(count, cx);
            }
        });
    }

    fn call_answered(
        &mut self,
        source: Source,
        name: String,
        quiet: bool,
        answer: Result<Json, RpcError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if source == Source::Provider
            && self
                .talking
                .as_ref()
                .is_some_and(|talking| talking.name == name)
        {
            self.talking = None;
            self.notice = None;
        }

        let (text, failed) = match (answer, source) {
            (Err(error), _) => (error.message, true),

            (Ok(result), Source::Host) => {
                let result: HostResult = serde_json::from_value(result).unwrap_or_default();

                self.attached(result.state.clone(), window, cx);

                (commands::host_answer(&result), false)
            }

            (Ok(result), Source::Provider) => (commands::provider_answer(&result), false),
        };

        if source == Source::Host {
            self.refresh_sessions();
        }

        if quiet && failed {
            self.notice = Some(text);
            return;
        }

        if quiet || text.is_empty() {
            return;
        }

        let current = self.current.clone();
        let change = self.view(&current, cx).conversation.push(Kind::Answer {
            source,
            command: name,
            text,
            failed,
        });

        self.show(&current, change, cx);
    }

    /// Takes in what a function of the host left the window attached to.
    fn attached(&mut self, state: HostState, window: &mut Window, cx: &mut Context<Self>) {
        if !state.dir.is_empty() {
            self.dir = state.dir;
        }

        // What was chosen and described was for the provider in use.
        if state.provider != self.provider {
            self.provider_functions = Ok(Vec::new());
            self.choices = Choices::default();
            self.describe();
        }

        self.provider = state.provider;
        self.model = state.model;

        if state.session != self.current {
            self.show_session(state.session, window, cx);
        }
    }

    fn event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        // What a provider's function says while it works.
        if event.invoke != 0 {
            let current = self.current.clone();
            let change = self.view(&current, cx).conversation.push(Kind::Log {
                source: Source::Provider,
                text: event.text,
            });

            self.show(&current, change, cx);
            return;
        }

        let Some(key) = self.session_of(&event, window, cx) else {
            return;
        };

        let view = self.view(&key, cx);

        if event.kind == protocol::EVENT_TURN {
            view.spent.turn(&event);
            return;
        }

        let change = view.conversation.apply(&event);

        self.show(&key, change, cx);
    }

    /// The session whose view an event belongs to: the session of the run
    /// it names, the session it names, or the one shown for what names
    /// neither, such as a provider's diagnostics. None for a session the
    /// window has not opened: its history will tell it.
    fn session_of(
        &mut self,
        event: &Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if let Some(key) = self.runs.get(&event.run).cloned() {
            // A run in a new session names it in its first event.
            if key.is_empty() && !event.session.is_empty() {
                let shown = self.current.is_empty();

                self.name_session(&event.session);

                if shown {
                    self.set_hint(window, cx);
                }

                return Some(event.session.clone());
            }

            return Some(key);
        }

        if event.session.is_empty() {
            return Some(self.current.clone());
        }

        self.views
            .contains_key(&event.session)
            .then(|| event.session.clone())
    }

    /// Gives the new session its id, once a run has made it.
    fn name_session(&mut self, session: &str) {
        if let Some(view) = self.views.remove("") {
            self.views.insert(session.to_string(), view);
        }

        for key in self.runs.values_mut().filter(|key| key.is_empty()) {
            *key = session.to_string();
        }

        if self.current.is_empty() {
            self.current = session.to_string();
        }

        self.refresh_sessions();
    }

    fn closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The rumbo that started this window is gone, and cannot be started
        // again from here.
        if self.host.is_attached() {
            cx.quit();
            return;
        }

        let said = self.host.stderr();

        self.ended = Some(if said.is_empty() {
            "The host ended.".into()
        } else {
            format!("The host ended: {said}")
        });

        self.notice = None;
        self.pending.clear();
        self.start_again_focus.focus(window, cx);

        let runs: Vec<String> = self.runs.drain().map(|(_, key)| key).collect();

        for key in runs {
            let view = self.view(&key, cx);

            view.run = None;

            let change = view
                .conversation
                .end(protocol::RUN_FAILED, "The host ended before the run did.");

            self.show(&key, change, cx);
        }

        self.set_hint(window, cx);
    }

    fn toggle(&mut self, key: &str, id: ItemId, cx: &mut Context<Self>) {
        let view = self.view(key, cx);

        if !view.expanded.remove(&id) {
            view.expanded.insert(id);
        }

        if let Some(index) = view
            .conversation
            .items()
            .iter()
            .position(|item| item.id == id)
        {
            self.show(key, Change::Updated(index), cx);
        }

        cx.notify();
    }

    /// Tells a session's scroller what changed, so it measures only that row.
    fn show(&mut self, key: &str, change: Change, cx: &mut Context<Self>) {
        let Some(view) = self.views.get(key) else {
            return;
        };

        match change {
            Change::Nothing => {}

            Change::Appended => view.scroller.update(cx, |state, cx| {
                let _ = state.append(1, cx);
            }),

            Change::Updated(index) => view.scroller.update(cx, |state, cx| {
                let _ = state.remeasure_items(index..index + 1, cx);
            }),
        }
    }

    fn steers_waiting(&self) -> usize {
        self.pending
            .values()
            .filter(|pending| matches!(pending, Pending::Steer { run, .. } if self.runs.get(run) == Some(&self.current)))
            .count()
    }

    /// How many runs of this window work in sessions other than the one
    /// shown.
    fn working_elsewhere(&self) -> usize {
        self.runs
            .values()
            .filter(|key| **key != self.current)
            .count()
    }

    fn render_conversation(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(view) = self.views.get(&self.current) else {
            return div().flex_1().into_any_element();
        };

        if view.conversation.items().is_empty() {
            return v_flex()
                .flex_1()
                .min_h_0()
                .px_8()
                .items_center()
                .justify_center()
                .text_center()
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
        let key = self.current.clone();

        div()
            .flex_1()
            .min_h_0()
            .child(
                MessageScroller::new(
                    // One scroller per session, so each keeps its place.
                    ElementId::Name(format!("conversation-{}", self.current).into()),
                    view.scroller.clone(),
                    move |index, _, cx| rows::render_row(&workspace, &key, index, cx),
                )
                .with_list_style(StyleRefinement::default().px_4().py_3())
                .with_row_style(StyleRefinement::default().pb_3())
                .with_bottom_fade(cx.theme().background),
            )
            .into_any_element()
    }

    fn render_composer(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.ended.is_some() {
            let session = self.current.clone();

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

        let working = self
            .views
            .get(&self.current)
            .is_some_and(|view| view.run.is_some());

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
        let view = self.views.get(&self.current);
        let model = if self.choices.model.is_empty() {
            &self.model
        } else {
            &self.choices.model
        };
        let working = view.is_some_and(|view| view.run.is_some());
        let waiting = self.steers_waiting();
        let elsewhere = self.working_elsewhere();

        let state = match (&self.ended, &self.notice, &self.talking) {
            (Some(ended), _, _) => ended.clone(),
            (None, Some(notice), _) => notice.clone(),
            (None, None, Some(talking)) => {
                format!("Waiting for {}; Escape gives it up", talking.name)
            }
            _ if working && waiting == 1 => "Working; 1 message waits for the next turn".into(),
            _ if working && waiting > 1 => {
                format!("Working; {waiting} messages wait for the next turn")
            }
            _ if working => "Working".into(),
            _ => "Ready".into(),
        };

        let mut bar = StatusBar::new()
            .left(
                h_flex()
                    .gap_1()
                    .when(working, |row| row.child(Spinner::new().xsmall()))
                    .child(state),
            )
            .child(
                Button::new("model")
                    .ghost()
                    .xsmall()
                    .label(if self.choices.is_empty() {
                        format!("{} · {}", self.provider, model)
                    } else {
                        format!("{} · {} (chosen here)", self.provider, model)
                    })
                    .tooltip("Model and options for the tasks started here")
                    .on_click(cx.listener(|this, _, _, cx| this.open_picker(cx))),
            );

        if elsewhere > 0 {
            bar = bar.right(format!("{elsewhere} in the background"));
        }

        if let Some(view) = view {
            if let Some(context) = view.spent.context() {
                bar = bar.right(context);
            }

            bar = bar.right(view.spent.tokens());
        }

        bar.border_t_1().border_color(cx.theme().border)
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::open_palette))
            .size_full()
            .items_stretch()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(self.render_conversation(cx))
                    .child(self.render_composer(cx))
                    .child(self.render_status(cx)),
            )
    }
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

impl Drop for Workspace {
    fn drop(&mut self) {
        self.host.stop();
    }
}
