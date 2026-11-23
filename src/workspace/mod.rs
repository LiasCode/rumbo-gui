//! One project of the window, attached to a host of its own: its sessions,
//! the conversation of the one shown, and the composer. The window around
//! it lists the sessions and opens them through it.
//!
//! The workspace shows the host's current session, or none once the one
//! shown is deleted, until the person chooses one or starts one. Runs
//! started in other sessions go on in the background, and their events go
//! to the session they work in, so each session's conversation is whole
//! when it is shown again.

mod composer;
mod details;
mod files;
mod form;
mod git_history;
mod header;
mod marks;
mod palette;
mod panel;
mod picker;
mod rows;
mod talk;

use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, EventEmitter, FocusHandle,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Pixels, Render,
    Styled as _, Subscription, Task, Window, actions, div, prelude::FluentBuilder as _,
};
use serde_json::{Value as Json, json};
use ui::{
    ActiveTheme as _, Button, CommandPaletteState, Dialogs, Icon, IconName, InputEvent, InputState,
    MessageList, MessageListState, PanelSize, Resizable, ResizablePanel, Sizable as _, Toast,
    Toasts, h_flex, rems_from_px, v_flex,
};

use crate::commands::{self, Command};
use crate::connection::Incoming;
use crate::conversation::{Change, Conversation, ItemId, Kind, MessageMark, RowGroup, Source};
use crate::history::History;
use crate::host::Host;
use crate::picture::{self, Picture};
use crate::protocol::{
    self, CancelParams, DescribeResult, Event, Function, HistoryParams, HistoryResult, HostResult,
    HostState, InvokeParams, RecallResult, RpcError, RunParams, RunResult, SteerParams, Value,
};
use crate::usage::Spent;

use form::{Changed, Field, Form};

actions!(workspace, [CancelRun, ToggleChanges, ToggleDetails]);

const KEY_CONTEXT: &str = "Workspace";

/// What the composer says Enter will do, with and without a run.
const TASK_HINT: &str = "Give rumbo a task";
const STEER_HINT: &str = "Tell the run something; it reads it before its next request";

/// The keys of the workspace. Escape reaches it after the composer, which
/// lets it through when it has nothing of its own to dismiss.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", CancelRun, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-shift-g", ToggleChanges, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-alt-b", ToggleDetails, Some(KEY_CONTEXT)),
    ]);
}

/// What shows beside the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// The project's Git repository: what changed in its folder, and the
    /// history of its branches.
    Changes,
    /// The project folder and its files.
    Files,
    /// How the session stands, as the host says, and the skills it has.
    Details,
}

/// What the workspace asks of the window around it.
pub enum WorkspaceEvent {
    /// Start a host again, in this session, after the last one ended.
    StartAgain {
        session: String,
    },
    /// The person chose a command of the window in the palette.
    OpenProject,
    OpenSettings,
    CloseProject,
    /// A function of the host answered, so what the settings show may have
    /// changed.
    Answered,
    /// Open a folder or a file in the person's editor.
    OpenInEditor {
        path: String,
    },
    /// A run of this window ended: in the session shown when `shown`, by
    /// the title a person knows the session by, with how it ended and the
    /// error it ended with.
    RunEnded {
        session: String,
        title: String,
        status: String,
        error: String,
        shown: bool,
    },
    /// The session shown is now another: a place the person went to when
    /// `went`, or else the place shown under its own name, as when going
    /// back or forward arrives, or a new session's first run names it.
    Shown {
        session: String,
        went: bool,
    },
    /// The person dragged the panel beside the conversation to this width,
    /// in pixels at the interface's own size.
    PanelResized {
        width: u32,
    },
}

/// What is done with the answer of a function of the host asked for by the
/// window, such as what the settings show.
pub type Reply = Box<dyn FnOnce(Result<HostResult, String>, &mut Window, &mut App)>;

/// What the workspace does once a function of the host it asked for itself
/// has answered, and the window took in where it is attached now.
type Then = Box<dyn FnOnce(&mut Workspace, HostResult, &mut Window, &mut Context<Workspace>)>;

/// A request whose answer the workspace waits for.
enum Pending {
    Run,
    /// What the person told a run, which waits in the session's
    /// conversation as this item until the run has it.
    Steer {
        session: String,
        item: ItemId,
    },
    /// The history of a session, for its view.
    History(String),
    /// What the person asked in the project's sessions, for the composer.
    Recall,
    /// The project's sessions, for the sidebar.
    Sessions,
    /// What the provider offers, for the palette.
    Describe,
    /// What the provider offers, for the picker of the model and options.
    Picker,
    /// A list a field of the open form asked for.
    Listed {
        form: u64,
        field: usize,
        generation: u64,
        source: Source,
    },
    /// A function called for the person.
    Call {
        source: Source,
        name: String,
        feedback: Feedback,
    },
    /// A function of the host the window asked, whose answer it reads.
    Reply(Reply),
    /// A function of the host the workspace asked for itself, to go on
    /// once it has answered.
    Then(Then),
    /// A session shown again, as going back or forward does.
    Revisit,
}

/// Where the answer of a function called for the person shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feedback {
    /// In the conversation, as a command from the palette answers.
    Conversation,
    /// In a notification, as the settings' and the picker's changes say
    /// they were made, which the conversation does not record.
    Toast,
    /// Nowhere unless it fails, as opening a session from the sidebar,
    /// which shows the session instead.
    Quiet,
}

/// The form in a dialog, whose lists arrive as answers.
struct OpenForm {
    /// Tells this form's answers from those of one that came before.
    serial: u64,
    form: Entity<Form>,
    _changes: Subscription,
}

/// What the window keeps of one session.
struct SessionView {
    conversation: Conversation,
    groups: Vec<RowGroup>,
    marks: Vec<MessageMark>,
    open_groups: HashSet<ItemId>,
    scroller: Entity<MessageListState>,
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
        let scroller = cx.new(|cx| MessageListState::new(0, cx));
        let scrolling = cx.observe(&scroller, |_, _, cx| cx.notify());

        SessionView {
            conversation: Conversation::default(),
            groups: Vec::new(),
            marks: Vec::new(),
            open_groups: HashSet::new(),
            scroller,
            spent: Spent::default(),
            run: None,
            expanded: HashSet::new(),
            _scrolling: scrolling,
        }
    }

    fn visible_index(&self, item_index: usize) -> Option<usize> {
        self.groups
            .partition_point(|group| group.start <= item_index)
            .checked_sub(1)
    }

    /// Updates only the changed range of the virtual list, keeping unchanged
    /// rows and their scroll position even if their item indices shifted.
    fn sync_groups(&mut self, cx: &mut Context<Workspace>) {
        let next = self.conversation.groups();
        let prefix = self
            .groups
            .iter()
            .zip(&next)
            .take_while(|(old, new)| old.same_items(new))
            .count();
        let suffix = self.groups[prefix..]
            .iter()
            .rev()
            .zip(next[prefix..].iter().rev())
            .take_while(|(old, new)| old.same_items(new))
            .count();
        let old_end = self.groups.len() - suffix;
        let new_end = next.len() - suffix;

        self.marks = self.conversation.message_marks(&next);
        self.groups = next;

        if prefix != old_end || prefix != new_end {
            self.scroller.update(cx, |state, cx| {
                state.replace(prefix..old_end, new_end - prefix, cx)
            });
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
    /// Whether no session is shown, as once the one shown is deleted, until
    /// the person chooses one or starts one. The host's current session is
    /// a new one meanwhile.
    blank: bool,
    views: HashMap<String, SessionView>,
    /// The session each run of this window works in, by the id of its run
    /// request.
    runs: HashMap<i64, String>,
    /// When each run of this window started.
    started: HashMap<i64, std::time::Instant>,
    pending: HashMap<i64, Pending>,
    /// The project's sessions, in the order they were first listed, the
    /// newest first.
    listed: Vec<Value>,
    /// What the host can be asked, whatever the provider.
    host_functions: Vec<Function>,
    /// What the provider can be asked, or why that is not known.
    provider_functions: Result<Vec<Function>, String>,
    composer: Entity<InputState>,
    /// The images pasted into the composer, which go with what is sent
    /// next.
    pasted: Vec<Picture>,
    palette: Entity<CommandPaletteState>,
    /// The picker waits for what the provider offers.
    picking: bool,
    /// The form of the dialog shown, and how many were opened.
    form: Option<OpenForm>,
    forms: u64,
    /// The provider's function in progress, called from this window.
    talking: Option<talk::Talking>,
    history: History,
    /// Why the host is gone, once it is.
    ended: Option<String>,
    /// Sessions whose run ended while another was shown, until they are.
    unseen: HashSet<String>,
    /// Whether the session shown is changing because going back or forward
    /// asked for it, which is no new place.
    revisiting: bool,
    /// What shows beside the conversation, if anything.
    panel: Option<Panel>,
    panels: panel::Panels,
    files: files::Files,
    /// The widest the conversation and the composer grow, in pixels.
    width: f32,
    /// How wide the panel beside the conversation is, in pixels at the
    /// interface's own size.
    panel_width: f32,
    focus_handle: FocusHandle,
    /// Where focus goes once the host ended, so Enter starts it again.
    start_again_focus: FocusHandle,
    /// Where focus goes while no session is shown, so Enter starts one.
    blank_focus: FocusHandle,
    /// The window's dialogs and notifications, which the workspace shows
    /// its own in.
    dialogs: Entity<Dialogs>,
    toasts: Entity<Toasts>,
    _reading: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(
        host: Host,
        dialogs: Entity<Dialogs>,
        toasts: Entity<Toasts>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let composer = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(1, 8)
                .submit_on_enter(true)
                .placeholder(TASK_HINT)
        });

        let palette = cx.new(|cx| {
            CommandPaletteState::new(window, cx).placeholder("Type a command or a session", cx)
        });

        let subscriptions = vec![cx.subscribe_in(
            &composer,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if *event == InputEvent::Submit {
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
            blank: false,
            views: HashMap::new(),
            runs: HashMap::new(),
            started: HashMap::new(),
            pending: HashMap::new(),
            listed: Vec::new(),
            host_functions: initialize.functions,
            provider_functions: Ok(Vec::new()),
            composer,
            pasted: Vec::new(),
            palette,
            picking: false,
            form: None,
            forms: 0,
            talking: None,
            history: History::default(),
            ended: None,
            unseen: HashSet::new(),
            revisiting: false,
            panel: None,
            panels: panel::Panels::new(window, cx),
            files: files::Files::default(),
            width: crate::preferences::Width::default().pixels(),
            panel_width: panel::DEFAULT_WIDTH,
            focus_handle: cx.focus_handle(),
            start_again_focus: cx.focus_handle(),
            blank_focus: cx.focus_handle(),
            dialogs,
            toasts,
            _reading: reading,
            _subscriptions: subscriptions,
        };

        // The history is asked for first: it is answered before the run
        // that follows starts, so nothing of that run is in it.
        workspace.show_session(initialize.session, window, cx);
        workspace.refresh_sessions();
        workspace.describe();

        // A host without it answers that it does not know it, and the
        // composer brings back only what is entered here.
        workspace.request(protocol::METHOD_RECALL, json!({}), Pending::Recall);

        // What the command line already asked for runs without asking again.
        if initialize.resume {
            workspace.resume(window, cx);
        } else if !initialize.objective.is_empty() {
            let objective = initialize.objective;

            workspace.start(
                RunParams {
                    objective: objective.clone(),
                    ..RunParams::default()
                },
                objective,
                Vec::new(),
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
    fn show_session(&mut self, session: String, _: &mut Window, cx: &mut Context<Self>) {
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
        self.unseen.remove(&session);
        self.current = session.clone();
        self.blank = false;
        self.set_hint(cx);
        self.refresh_panel(cx);
        cx.emit(WorkspaceEvent::Shown {
            session,
            went: !self.revisiting,
        });
        cx.notify();
    }

    /// Shows no session, as once the one shown is deleted, until the person
    /// chooses one or starts one. It is no place to go back to: the place
    /// shown stays the session deleted, which going back and forward pass
    /// over.
    fn show_nothing(&mut self, cx: &mut Context<Self>) {
        self.current = String::new();
        self.blank = true;
        self.set_hint(cx);
        self.refresh_panel(cx);
        cx.notify();
    }

    /// Asks for the project's sessions, which the sidebar lists.
    fn refresh_sessions(&mut self) {
        if self.host_function(protocol::FUNCTION_SESSIONS).is_none() {
            return;
        }

        self.request(
            protocol::METHOD_INVOKE,
            InvokeParams {
                name: protocol::FUNCTION_SESSIONS.into(),
                params: None,
                target: protocol::TARGET_HOST,
            },
            Pending::Sessions,
        );
    }

    /// A function the host described in `initialize`, by name.
    fn host_function(&self, name: &str) -> Option<&Function> {
        self.host_functions
            .iter()
            .find(|function| function.name == name)
    }

    /// The project's sessions as the host lists them, in the order they were
    /// first listed, the newest first.
    pub fn sessions(&self) -> &[Value] {
        &self.listed
    }

    /// Lists the project's sessions in the order the window listed them
    /// before it opened the project, until the host lists them, so opening
    /// it moves none.
    pub fn keep_order(&mut self, listed: Vec<Value>) {
        if self.listed.is_empty() {
            self.listed = listed;
        }
    }

    /// The session shown, empty for a new one until its first run makes it.
    pub fn current(&self) -> &str {
        &self.current
    }

    /// Whether no session is shown, since the one shown was deleted.
    pub fn is_blank(&self) -> bool {
        self.blank
    }

    /// Whether a run of this window works in the session.
    pub fn is_working(&self, session: &str) -> bool {
        self.runs.values().any(|key| key == session)
    }

    /// How many runs of this window work in the project.
    pub fn working(&self) -> usize {
        self.runs.len()
    }

    /// Whether the host is gone; the window can start it again.
    pub fn has_ended(&self) -> bool {
        self.ended.is_some()
    }

    /// Whether a run ended in the session while another was shown, and the
    /// session has not been shown since.
    pub fn has_unseen(&self, session: &str) -> bool {
        self.unseen.contains(session)
    }

    /// The folder the host works in, as it said last.
    pub fn dir(&self) -> &str {
        &self.dir
    }

    /// How wide the conversation and the composer grow.
    pub fn set_width(&mut self, width: f32, cx: &mut Context<Self>) {
        self.width = width;
        cx.notify();
    }

    /// How wide the panel beside the conversation is: the width the person
    /// chose, in pixels at the interface's own size, or its own.
    pub fn set_panel_width(&mut self, chosen: Option<u32>, cx: &mut Context<Self>) {
        self.panel_width = panel::width(chosen);
        cx.notify();
    }

    /// Takes the width a drag or the keys gave the panel, and tells the
    /// window, which keeps it for every project.
    fn resize_panel(&mut self, size: &PanelSize, window: &mut Window, cx: &mut Context<Self>) {
        if size.key != "panel" {
            return;
        }

        // It is kept at the interface's own size, as it grows with it.
        let pixel = rems_from_px(1.).to_pixels(window.rem_size());
        let width = panel::width(Some((size.size / pixel).round() as u32));

        self.panel_width = width;
        cx.emit(WorkspaceEvent::PanelResized {
            width: width as u32,
        });
        cx.notify();
    }

    /// Whether the host offers a function of its own, by name.
    pub fn offers(&self, name: &str) -> bool {
        self.host_function(name).is_some()
    }

    /// The name a person knows a session by: the title the host gives it,
    /// else the first line of its first task, as a new one has until the
    /// host lists it, else its id.
    fn title_of(&self, session: &str) -> String {
        let titled = self
            .listed
            .iter()
            .find(|listed| listed.value == session)
            .map(|listed| listed.label.clone())
            .filter(|label| !label.is_empty());

        let asked = || {
            self.views.get(session).and_then(|view| {
                view.conversation
                    .items()
                    .iter()
                    .find_map(|item| match &item.kind {
                        Kind::Task(text, _) => Some(crate::conversation::first_line(text)),
                        _ => None,
                    })
                    .filter(|line| !line.is_empty())
            })
        };

        titled.or_else(asked).unwrap_or_else(|| session.to_string())
    }

    /// The title the host gives the session shown, once it has one.
    pub fn session_title(&self) -> Option<String> {
        self.listed
            .iter()
            .find(|session| session.value == self.current && !self.current.is_empty())
            .map(|session| session.label.clone())
            .filter(|label| !label.is_empty())
    }

    /// Gives the keyboard to the composer, or, while no session is shown, to
    /// starting one.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        if self.blank && self.ended.is_none() {
            self.blank_focus.focus(window, cx);
            return;
        }

        self.composer
            .update(cx, |state, cx| state.focus(window, cx));
    }

    /// Makes another session the current one. A run working in the one left
    /// behind goes on in the background.
    pub fn open_session(&mut self, id: &str, cx: &mut Context<Self>) {
        if id == self.current || self.ended.is_some() {
            return;
        }

        self.call_host(
            protocol::FUNCTION_OPEN,
            Some(json!({ "session": id })),
            Feedback::Quiet,
            cx,
        );
    }

    /// Makes the next task start a new session. While no session is shown,
    /// the host's current one is new already, and is shown.
    pub fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.blank && self.ended.is_none() {
            self.show_session(String::new(), window, cx);
            self.focus(window, cx);
            return;
        }

        if self.current.is_empty() || self.ended.is_some() {
            return;
        }

        self.call_host(protocol::FUNCTION_NEW, None, Feedback::Quiet, cx);
    }

    /// Shows a session again, empty for a new one, as going back or forward
    /// does: what it shows is told as the place arrived at, not as a new
    /// one. A session that cannot be shown leaves the one shown as it is,
    /// and is told as the place arrived at instead.
    pub fn revisit(&mut self, session: &str, window: &mut Window, cx: &mut Context<Self>) {
        if session == self.current {
            // While no session is shown, the host's current one is new
            // already.
            if self.blank {
                self.revisiting = true;
                self.show_session(String::new(), window, cx);
                self.revisiting = false;
            }

            return;
        }

        let (name, params) = match session.is_empty() {
            true => (protocol::FUNCTION_NEW, None),
            false => (protocol::FUNCTION_OPEN, Some(json!({ "session": session }))),
        };

        let asked = self.ended.is_none()
            && self.host_function(name).is_some()
            && self
                .request(
                    protocol::METHOD_INVOKE,
                    InvokeParams {
                        name: name.into(),
                        params,
                        target: protocol::TARGET_HOST,
                    },
                    Pending::Revisit,
                )
                .is_some();

        if !asked {
            cx.emit(WorkspaceEvent::Shown {
                session: self.current.clone(),
                went: false,
            });
        }
    }

    /// Asks a function of the host for the workspace itself, and goes on
    /// once it has answered, attached to what it left current. A failure
    /// shows in a notification.
    fn then_host(
        &mut self,
        name: &str,
        params: Option<Json>,
        then: Then,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.ended.is_some() || self.host_function(name).is_none() {
            self.toast(
                Toast::new(format!("This host does not offer {name}")).danger(),
                cx,
            );
            return false;
        }

        self.request(
            protocol::METHOD_INVOKE,
            InvokeParams {
                name: name.into(),
                params,
                target: protocol::TARGET_HOST,
            },
            Pending::Then(then),
        )
        .is_some()
    }

    /// Runs `action` in a session: at once in the one shown, or once the
    /// host has made it current, since the host's functions act on the
    /// current session.
    fn in_session(
        &mut self,
        session: &str,
        action: impl FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if session == self.current {
            action(self, window, cx);
            return;
        }

        self.then_host(
            protocol::FUNCTION_OPEN,
            Some(json!({ "session": session })),
            Box::new(move |this, _, window, cx| action(this, window, cx)),
            cx,
        );
    }

    /// Asks for a new title for a session, which is shown for it.
    pub fn rename(&mut self, session: &str, window: &mut Window, cx: &mut Context<Self>) {
        let title = self
            .listed
            .iter()
            .find(|listed| listed.value == session)
            .map(|listed| listed.label.clone())
            .unwrap_or_default();

        self.in_session(
            session,
            move |this, window, cx| {
                this.offer(
                    protocol::FUNCTION_RENAME,
                    &[("title", title)],
                    true,
                    window,
                    cx,
                )
            },
            window,
            cx,
        );
    }

    /// Deletes a session for good, once the person confirms it as the host
    /// asks.
    pub fn delete(&mut self, session: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.offer(
            protocol::FUNCTION_DELETE,
            &[("session", session.to_string())],
            false,
            window,
            cx,
        );
    }

    /// Writes the conversation of the session shown as Markdown, in a file
    /// the person chooses.
    pub fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.views.get(&self.current) else {
            return;
        };

        let title = self
            .session_title()
            .unwrap_or_else(|| "rumbo session".to_string());
        let text = crate::export::markdown(&title, view.conversation.items());
        let chosen = cx.prompt_for_new_path(
            std::path::Path::new(&self.dir),
            Some(&crate::export::file_name(&title)),
        );

        let toasts = self.toasts.clone();

        cx.spawn_in(window, async move |_, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };

            let written = std::fs::write(&path, text);

            let toast = match written {
                Ok(()) => Toast::new(format!("The session was written to {}", path.display())),
                Err(error) => {
                    Toast::new(format!("{} could not be written: {error}", path.display())).danger()
                }
            };

            toasts.update(cx, |toasts, cx| {
                toasts.push(toast, cx);
            });
        })
        .detach();
    }

    /// Replaces the conversation of the session shown with a summary now.
    pub fn compact(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.offer(protocol::FUNCTION_COMPACT, &[], false, window, cx);
    }

    /// Goes on in a branch of the session shown: a copy of it as it is, or
    /// as it was before the person's message numbered `point`. With `then`,
    /// what that message said is written again in the branch, to change
    /// and send, or, with `send`, sent as it is.
    pub fn branch(
        &mut self,
        point: Option<usize>,
        again: Option<(String, Vec<Picture>, bool)>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let params = point.map(|point| json!({ "point": point.to_string() }));

        self.then_host(
            protocol::FUNCTION_BRANCH,
            params,
            Box::new(move |this, answer, window, cx| {
                this.toast(
                    Toast::new(commands::capitalized(&commands::host_answer(&answer))),
                    cx,
                );

                match again {
                    Some((text, images, true)) => this.start(
                        RunParams {
                            objective: text.clone(),
                            images: images.iter().map(picture::to_protocol).collect(),
                            ..RunParams::default()
                        },
                        text,
                        images,
                        window,
                        cx,
                    ),

                    Some((text, images, false)) => {
                        this.fill(text, window, cx);
                        this.pasted = images;
                        this.focus(window, cx);
                    }

                    None => {}
                }
            }),
            cx,
        );
    }

    /// Asks a function of the host for the window, which reads its answer,
    /// as the settings do. False when the host does not offer it, or is
    /// gone.
    pub fn ask_host(&mut self, name: &str, params: Option<Json>, reply: Reply) -> bool {
        if self.ended.is_some() || self.host_function(name).is_none() {
            return false;
        }

        self.request(
            protocol::METHOD_INVOKE,
            InvokeParams {
                name: name.into(),
                params,
                target: protocol::TARGET_HOST,
            },
            Pending::Reply(reply),
        )
        .is_some()
    }

    /// Offers a function of the host with some of its parameters given, as
    /// the settings do for a provider: a form for the rest when `ask` or when
    /// one is missing, else its confirmation, or the call. Its answer shows
    /// in a notification.
    pub fn offer(
        &mut self,
        name: &str,
        given: &[(&str, String)],
        ask: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(function) = self.host_function(name).cloned() else {
            return;
        };

        if self.ended.is_some() {
            return;
        }

        let typed: Vec<String> = function
            .params
            .iter()
            .map(|parameter| {
                given
                    .iter()
                    .find(|(name, _)| *name == parameter.name)
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default()
            })
            .collect();

        let command = Command {
            source: Source::Host,
            function,
        };

        match commands::arguments(&command.function, &typed) {
            Ok(params) if !ask => {
                self.confirm_and_call(command, params, Feedback::Toast, window, cx);
            }

            _ => self.ask_parameters(command, typed, Feedback::Toast, window, cx),
        }
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

            Err(_) => None,
        }
    }

    /// Sends what the composer holds: a task, or, during a run, something
    /// for the run to read before its next request.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.composer.read(cx).value().trim().to_string();

        if (text.is_empty() && self.pasted.is_empty()) || self.ended.is_some() {
            return;
        }

        if !text.is_empty() {
            self.history.remember(&text);
        }

        self.composer
            .update(cx, |state, cx| state.set_value("", window, cx));

        let images = std::mem::take(&mut self.pasted);
        let sent = images.iter().map(picture::to_protocol).collect();
        let current = self.current.clone();

        match self.view(&current, cx).run {
            Some(run) => {
                // It shows at once, waiting, until the run has it.
                let (item, change) = self
                    .view(&current, cx)
                    .conversation
                    .wait(text.clone(), images);

                self.show(&current, change, cx);

                let pending = Pending::Steer {
                    session: current.clone(),
                    item,
                };
                let params = SteerParams {
                    run,
                    text,
                    images: sent,
                };

                if self
                    .request(protocol::METHOD_STEER, params, pending)
                    .is_none()
                {
                    self.unheard(&current, item, "The host is gone", window, cx);
                }

                self.set_hint(cx);
            }

            None => self.start(
                RunParams {
                    objective: text.clone(),
                    images: sent,
                    ..RunParams::default()
                },
                text,
                images,
                window,
                cx,
            ),
        }

        cx.notify();
    }

    /// Continues the unfinished run of the session shown. It is not a
    /// message of the person's, so it shows as a note.
    fn resume(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let params = RunParams {
            resume: true,
            ..RunParams::default()
        };

        self.start_as(
            params,
            Kind::Log {
                source: Source::Host,
                text: "Continuing the unfinished run".into(),
            },
            window,
            cx,
        );
    }

    /// Starts a run in the current session, with the model and the options
    /// the session keeps.
    fn start(
        &mut self,
        params: RunParams,
        label: String,
        images: Vec<Picture>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_as(params, Kind::Task(label, images), window, cx);
    }

    /// Starts a run, showing `shown` for what started it.
    fn start_as(&mut self, params: RunParams, shown: Kind, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.request(protocol::METHOD_RUN, &params, Pending::Run) else {
            return;
        };

        let current = self.current.clone();
        let view = self.view(&current, cx);

        view.run = Some(id);

        let change = view.conversation.push(shown);

        self.runs.insert(id, current.clone());
        self.started.insert(id, std::time::Instant::now());
        self.show(&current, change, cx);
        self.set_hint(cx);
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
            let said = format!("The cancel could not reach the host: {error}");

            self.toast(Toast::new(said).danger(), cx);
        } else {
            self.toast(Toast::new("Cancelling the run"), cx);
        }
    }

    /// Calls a function of the host or of the provider for the person.
    fn call(
        &mut self,
        command: &Command,
        params: Option<Json>,
        feedback: Feedback,
        cx: &mut Context<Self>,
    ) {
        let target = match command.source {
            Source::Host => protocol::TARGET_HOST,
            Source::Provider => "",
        };

        let pending = Pending::Call {
            source: command.source,
            name: command.function.name.clone(),
            feedback,
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
            self.set_hint(cx);
        }

        cx.notify();
    }

    /// Calls a function of the host by name, as the sidebar and the picker
    /// do. One the host did not describe is not called, and says so.
    fn call_host(
        &mut self,
        name: &str,
        params: Option<Json>,
        feedback: Feedback,
        cx: &mut Context<Self>,
    ) {
        let Some(function) = self.host_function(name).cloned() else {
            self.toast(
                Toast::new(format!("This host does not offer {name}")).danger(),
                cx,
            );
            return;
        };

        let command = Command {
            source: Source::Host,
            function,
        };

        self.call(&command, params, feedback, cx);
    }

    /// Opens a form whose lists the workspace asks for, in place of any
    /// form before it.
    fn open_form(
        &mut self,
        fields: Vec<Field>,
        provider_functions: Vec<Function>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Form> {
        let host_functions = self.host_functions.clone();
        let form = cx.new(|cx| Form::new(fields, host_functions, provider_functions, window, cx));

        // The dialog around the form says things that depend on its values,
        // and is drawn again only with the whole window.
        let changes = cx.subscribe_in(&form, window, |this, _, _: &Changed, window, cx| {
            window.refresh();
            this.list_for_form(cx);
        });

        self.forms += 1;
        self.form = Some(OpenForm {
            serial: self.forms,
            form: form.clone(),
            _changes: changes,
        });

        self.list_for_form(cx);

        form
    }

    /// Asks for the lists the open form needs now.
    fn list_for_form(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &self.form else {
            return;
        };

        let (serial, form) = (open.serial, open.form.clone());
        let wanted = form.update(cx, |form, cx| form.wanted(cx));

        for wanted in wanted {
            let target = match wanted.source {
                Source::Host => protocol::TARGET_HOST,
                Source::Provider => "",
            };

            self.request(
                protocol::METHOD_INVOKE,
                InvokeParams {
                    name: wanted.function,
                    params: wanted.params,
                    target,
                },
                Pending::Listed {
                    form: serial,
                    field: wanted.field,
                    generation: wanted.generation,
                    source: wanted.source,
                },
            );
        }
    }

    /// Says in the composer how the session shown stands, and so what Enter
    /// does now: a provider's function waits, a run works, with what was
    /// told it still waiting for its next turn, or nothing does.
    fn set_hint(&mut self, cx: &mut Context<Self>) {
        let working = self
            .views
            .get(&self.current)
            .is_some_and(|view| view.run.is_some());

        let hint = match (&self.talking, working, self.steers_waiting()) {
            (Some(talking), _, _) => format!("Waiting for {}; Escape gives it up", talking.name),
            (None, true, 0) => STEER_HINT.into(),
            (None, true, 1) => "1 message waits for the run's next turn; tell it more".into(),
            (None, true, waiting) => {
                format!("{waiting} messages wait for the run's next turn; tell it more")
            }
            (None, false, _) => TASK_HINT.into(),
        };

        self.composer
            .update(cx, |state, cx| state.set_placeholder(hint, cx));
    }

    /// Puts text in the composer with the cursor at its end, where typing
    /// goes on.
    fn fill(&self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let end = text.len();

        self.composer.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.set_cursor(end, window, cx);
        });
    }

    /// Shows a notification in the window.
    fn toast(&self, toast: Toast, cx: &mut App) {
        self.toasts.update(cx, |toasts, cx| {
            toasts.push(toast, cx);
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

            Pending::Steer { session, item } => self.steered(session, item, answer, window, cx),

            Pending::History(session) => {
                let told: HistoryResult = match answer {
                    Ok(result) => serde_json::from_value(result).unwrap_or_default(),
                    Err(error) => {
                        let said = format!("The session's history could not be read: {error}");

                        self.toast(Toast::new(said).danger(), cx);
                        return;
                    }
                };

                self.told(&session, &told.events, cx);
            }

            // Without it, the composer brings back what is entered here.
            Pending::Recall => {
                if let Ok(result) = answer {
                    let recalled: RecallResult = serde_json::from_value(result).unwrap_or_default();

                    self.history.recalled(&recalled.asked);
                }
            }

            Pending::Sessions => {
                if let Ok(result) = answer {
                    let listed: HostResult = serde_json::from_value(result).unwrap_or_default();
                    self.listed = crate::order::kept(&self.listed, listed.values);
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

            Pending::Listed {
                form,
                field,
                generation,
                source,
            } => {
                let Some(open) = self.form.as_ref().filter(|open| open.serial == form) else {
                    return;
                };

                let listed = answer
                    .map(|result| commands::listed(source, result))
                    .map_err(|error| error.message);

                open.form.update(cx, |form, cx| {
                    form.listed(field, generation, listed, window, cx)
                });
            }

            Pending::Call {
                source,
                name,
                feedback,
            } => self.call_answered(source, name, feedback, answer, window, cx),

            Pending::Reply(reply) => {
                let answer = answer
                    .map(|result| serde_json::from_value(result).unwrap_or_default())
                    .map_err(|error| error.message);

                reply(answer, window, cx);
            }

            Pending::Then(then) => match answer {
                Ok(result) => {
                    let result: HostResult = serde_json::from_value(result).unwrap_or_default();

                    self.attached(result.state.clone(), window, cx);
                    self.refresh_sessions();
                    then(self, result, window, cx);
                }

                Err(error) => self.toast(Toast::new(error.message).danger(), cx),
            },

            Pending::Revisit => {
                match answer {
                    Ok(result) => {
                        let result: HostResult = serde_json::from_value(result).unwrap_or_default();

                        self.revisiting = true;
                        self.attached(result.state, window, cx);
                        self.revisiting = false;
                        self.refresh_sessions();
                    }

                    Err(error) => self.toast(Toast::new(error.message).danger(), cx),
                }

                // Whatever the host answered, the place arrived at is the
                // session shown now.
                cx.emit(WorkspaceEvent::Shown {
                    session: self.current.clone(),
                    went: false,
                });
            }
        }
    }

    fn run_ended(
        &mut self,
        id: i64,
        answer: Result<Json, RpcError>,
        _: &mut Window,
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
            self.name_session(&session, cx);
            session
        } else {
            key
        };

        let seconds = self
            .started
            .remove(&id)
            .map(|started| started.elapsed().as_secs());
        let view = self.view(&key, cx);

        view.run = None;

        let change = view.conversation.end_after(&status, &error, seconds);

        self.show(&key, change, cx);

        let shown = key == self.current;

        // A session not shown is marked until it is.
        if shown {
            self.set_hint(cx);
        } else {
            self.unseen.insert(key.clone());
        }

        cx.emit(WorkspaceEvent::RunEnded {
            session: key.clone(),
            title: self.title_of(&key),
            status,
            error,
            shown,
        });

        self.refresh_sessions();
        self.refresh_panel(cx);
    }

    fn steered(
        &mut self,
        session: String,
        item: ItemId,
        answer: Result<Json, RpcError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // One message fewer waits for the run.
        self.set_hint(cx);

        match answer {
            Ok(_) => {
                let Some(view) = self.views.get_mut(&session) else {
                    return;
                };

                let change = view.conversation.heard(item);

                self.show(&session, change, cx);
            }

            Err(error) => self.unheard(&session, item, &error.message, window, cx),
        }
    }

    /// Takes away what the person told a run that will never read it, and
    /// gives it back to be sent again, unless something new is being typed.
    fn unheard(
        &mut self,
        session: &str,
        item: ItemId,
        why: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((text, images, change)) = self
            .views
            .get_mut(session)
            .and_then(|view| view.conversation.withdraw(item))
        else {
            return;
        };

        self.show(session, change, cx);

        if self.composer.read(cx).value().is_empty() && self.pasted.is_empty() {
            self.fill(text, window, cx);
            self.pasted = images;

            let said = format!("{why}; Enter sends it as a new task");

            self.toast(Toast::new(said).danger(), cx);
        } else {
            self.toast(Toast::new(format!("{why}: {text}")).danger(), cx);
        }
    }

    /// Puts what a session held before the window opened it in its view.
    fn told(&mut self, session: &str, events: &[Event], cx: &mut Context<Self>) {
        let view = self.view(session, cx);
        view.conversation.earlier(events);

        for event in events
            .iter()
            .filter(|event| event.kind == protocol::EVENT_TURN)
        {
            view.spent.turn(event);
        }

        view.sync_groups(cx);
    }

    fn call_answered(
        &mut self,
        source: Source,
        name: String,
        feedback: Feedback,
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
            self.set_hint(cx);
        }

        let (text, failed) = match (answer, source) {
            (Err(error), _) => (error.message, true),

            (Ok(result), Source::Host) => {
                let result: HostResult = serde_json::from_value(result).unwrap_or_default();
                let session = &result.state.session;

                // Deleting the session shown leaves none shown, rather than
                // the new one the host makes current; asking for a new one
                // shows it.
                if name == protocol::FUNCTION_DELETE
                    && session.is_empty()
                    && !self.current.is_empty()
                {
                    self.show_nothing(cx);
                } else if name == protocol::FUNCTION_NEW && self.blank {
                    self.show_session(String::new(), window, cx);
                }

                self.attached(result.state.clone(), window, cx);

                (commands::host_answer(&result), false)
            }

            (Ok(result), Source::Provider) => (commands::provider_answer(&result), false),
        };

        if source == Source::Host {
            self.refresh_sessions();
            cx.emit(WorkspaceEvent::Answered);
        }

        // With no session shown, there is no conversation to answer in.
        let feedback = match feedback {
            Feedback::Conversation if self.blank => Feedback::Toast,
            feedback => feedback,
        };

        match feedback {
            Feedback::Toast | Feedback::Quiet if failed => {
                self.toast(Toast::new(text).danger(), cx);
                return;
            }

            // The first line says what was done; the rest is what it
            // applies to now.
            Feedback::Toast => {
                if let Some(line) = text.lines().next().filter(|line| !line.is_empty()) {
                    self.toast(Toast::new(commands::capitalized(line)), cx);
                }

                return;
            }

            Feedback::Quiet => return,

            Feedback::Conversation if text.is_empty() => return,

            Feedback::Conversation => {}
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

        // What was described was for the provider in use.
        if state.provider != self.provider {
            self.provider_functions = Ok(Vec::new());
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if let Some(key) = self.runs.get(&event.run).cloned() {
            // A run in a new session names it in its first event.
            if key.is_empty() && !event.session.is_empty() {
                let shown = self.current.is_empty();

                self.name_session(&event.session, cx);

                if shown {
                    self.set_hint(cx);
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
    fn name_session(&mut self, session: &str, cx: &mut Context<Self>) {
        if let Some(view) = self.views.remove("") {
            self.views.insert(session.to_string(), view);
        }

        for key in self.runs.values_mut().filter(|key| key.is_empty()) {
            *key = session.to_string();
        }

        for pending in self.pending.values_mut() {
            if let Pending::Steer { session: key, .. } = pending
                && key.is_empty()
            {
                *key = session.to_string();
            }
        }

        // While no session is shown, the new one the host has current is
        // another.
        if self.current.is_empty() && !self.blank {
            self.current = session.to_string();
            cx.emit(WorkspaceEvent::Shown {
                session: session.to_string(),
                went: false,
            });
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

        // What still waited for a run never reaches it.
        let waiting: Vec<(String, ItemId)> = self
            .pending
            .drain()
            .filter_map(|(_, pending)| match pending {
                Pending::Steer { session, item } => Some((session, item)),
                _ => None,
            })
            .collect();

        self.start_again_focus.focus(window, cx);

        let runs: Vec<String> = self.runs.drain().map(|(_, key)| key).collect();

        self.started.clear();

        for key in runs {
            let view = self.view(&key, cx);

            view.run = None;

            let change = view
                .conversation
                .end(protocol::RUN_FAILED, "The host ended before the run did.");

            self.show(&key, change, cx);
        }

        for (session, item) in waiting {
            let Some((text, _, change)) = self
                .views
                .get_mut(&session)
                .and_then(|view| view.conversation.withdraw(item))
            else {
                continue;
            };

            self.show(&session, change, cx);
            self.toast(
                Toast::new(format!("The host ended before the run read: {text}")).danger(),
                cx,
            );
        }

        self.set_hint(cx);
    }

    fn toggle(&mut self, key: &str, id: ItemId, group: bool, cx: &mut Context<Self>) {
        let view = self.view(key, cx);

        let expanded = if group {
            &mut view.open_groups
        } else {
            &mut view.expanded
        };

        if !expanded.remove(&id) {
            expanded.insert(id);
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
        let Some(view) = self.views.get_mut(key) else {
            return;
        };

        match change {
            Change::Nothing => {}

            Change::Updated(index) => {
                if let Some(row) = view.visible_index(index) {
                    view.scroller
                        .update(cx, |state, cx| state.remeasure(row, cx));
                }
            }

            Change::Inserted(_) | Change::Removed(_) | Change::Moved { .. } => view.sync_groups(cx),
        }
    }

    fn steers_waiting(&self) -> usize {
        self.pending
            .values()
            .filter(|pending| matches!(pending, Pending::Steer { session, .. } if *session == self.current))
            .count()
    }

    /// Whether the history of a session was asked for and has not come.
    fn reading_history(&self, session: &str) -> bool {
        self.pending
            .values()
            .any(|pending| matches!(pending, Pending::History(asked) if asked == session))
    }

    fn render_conversation(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(view) = self.views.get(&self.current) else {
            return div().flex_1().into_any_element();
        };

        if view.conversation.items().is_empty() {
            // A session whose history is on its way is no new one: it shows
            // nothing until its history comes.
            if self.reading_history(&self.current) {
                return div().flex_1().into_any_element();
            }

            return self.render_start(cx);
        }

        let workspace = cx.weak_entity();
        let key = self.current.clone();
        let width = self.width;

        let rail = (!view.marks.is_empty())
            .then(|| marks::rail(&self.current, &view.marks, &view.scroller, window, cx));

        h_flex()
            .flex_1()
            .min_h_0()
            .child(div().flex_1().min_w_0().h_full().child(MessageList::new(
                // One list per session, so each keeps its place.
                ElementId::Name(format!("conversation-{}", self.current).into()),
                &view.scroller,
                move |index, _, cx| {
                    div()
                        .w_full()
                        .max_w(rems_from_px(width))
                        .mx_auto()
                        .px_4()
                        .when(index == 0, |row| row.pt_4())
                        .pb_4()
                        .child(rows::render_row(&workspace, &key, index, cx))
                        .into_any_element()
                },
            )))
            .children(rail)
            .into_any_element()
    }

    /// A session with nothing in it yet: what it works in, and what was
    /// asked lately in the project, to ask again or start from.
    fn render_start(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().colors.muted_foreground;
        let latest = self.history.latest(3);

        let suggestions = latest.into_iter().enumerate().map(|(index, text)| {
            let shown = crate::conversation::first_line(&text);

            Button::new(("suggestion", index))
                .outline()
                .small()
                .w_full()
                .justify_start()
                .truncate()
                .icon(IconName::Clock)
                .label(shown)
                .tooltip("Write it in the composer again")
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.fill(text.clone(), window, cx);
                    this.focus(window, cx);
                }))
        });

        v_flex()
            .flex_1()
            .min_h_0()
            .px_8()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                Icon::new(IconName::Sparkles)
                    .large()
                    .color(cx.theme().colors.primary),
            )
            .child(div().text_2xl().child("What should rumbo do?"))
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(format!("It works in {}", self.dir)),
            )
            .child(
                v_flex()
                    .w_full()
                    .max_w(rems_from_px(self.width.min(520.)))
                    .pt_4()
                    .gap_2()
                    .when(self.history.latest(1).len() == 1, |column| {
                        column.child(div().text_xs().text_color(muted).child("Asked lately"))
                    })
                    .children(suggestions),
            )
            .into_any_element()
    }

    /// No session shown, once the one shown was deleted: what to do next,
    /// with a way to start a new session, which has the keyboard.
    fn render_blank(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().colors.muted_foreground;
        let said = match self.listed.is_empty() {
            true => "The project has no sessions left. Start a new one to work in it.",
            false => "Choose a session, or start a new one.",
        };

        v_flex()
            .flex_1()
            .min_h_0()
            .px_8()
            .items_center()
            .justify_center()
            .gap_2()
            .child(Icon::new(IconName::MessageSquare).large().color(muted))
            .child(div().text_2xl().child("No session selected"))
            .child(div().text_sm().text_color(muted).child(said))
            .child(
                div().pt_2().child(
                    Button::new("blank-new-session")
                        .track_focus(&self.blank_focus)
                        .primary()
                        .icon(IconName::SquarePen)
                        .label("New Session")
                        .tooltip("Start a new session in this project (Ctrl-N)")
                        .on_click(cx.listener(|this, _, window, cx| this.new_session(window, cx))),
                ),
            )
            .into_any_element()
    }

    /// The provider and the model of the session shown, as the host said.
    /// A session can start before its model is chosen, which the picker
    /// does, and a task is refused until then.
    fn model_name(&self) -> String {
        if self.model.is_empty() {
            format!("{} · no model yet", self.provider)
        } else {
            format!("{} · {}", self.provider, self.model)
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let work = v_flex().size_full().min_w_0();

        // With no session shown there is nothing to write in, until the host
        // ends and the composer says so.
        let work = match self.blank && self.ended.is_none() {
            true => work.child(self.render_blank(cx)),
            false => work
                .child(self.render_conversation(window, cx))
                .child(self.render_composer(cx)),
        };

        // The conversation keeps no least width, as before the panel could
        // be dragged, so a narrow window cuts off no more of the panel than
        // it did; the panel's most keeps the conversation in view on a
        // screen of usual width.
        let mut row = Resizable::horizontal("workspace-body")
            .child(ResizablePanel::new("work").min(Pixels::ZERO).child(work));

        if let Some(panel) = self.panel {
            let rem = window.rem_size();

            row = row.child(
                ResizablePanel::new("panel")
                    .size(rems_from_px(self.panel_width).to_pixels(rem))
                    .min(rems_from_px(panel::MIN_WIDTH).to_pixels(rem))
                    .max(rems_from_px(panel::MAX_WIDTH).to_pixels(rem))
                    .child(self.render_panel(panel, window, cx)),
            );
        }

        let row =
            row.on_resize(cx.listener(|this, size: &PanelSize, window, cx| {
                this.resize_panel(size, window, cx)
            }));

        v_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(|this, _: &ToggleChanges, window, cx| {
                this.toggle_panel(Panel::Changes, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleDetails, window, cx| {
                this.toggle_panel(Panel::Details, window, cx)
            }))
            .size_full()
            .min_w_0()
            .bg(cx.theme().colors.background)
            .text_color(cx.theme().colors.foreground)
            .child(div().flex_1().min_h_0().child(row))
    }
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

impl Drop for Workspace {
    fn drop(&mut self) {
        self.host.stop();
    }
}
