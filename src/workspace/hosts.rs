//! The hosts of a project. A rumbo host is one session, so the workspace
//! holds one for each session of the project it has open: the one the
//! project was opened with, and those it starts as the person shows other
//! sessions, or new ones. It shows one at a time, follows the runs of all
//! of them, and closes those left with nothing to do, since each costs its
//! own processes.

use std::path::PathBuf;
use std::time::Instant;

use gpui::{AppContext as _, Context, ParentElement as _, Task, Window};
use serde_json::Value as Json;
use ui::Toast;

use super::{Pending, Workspace, WorkspaceEvent};
use crate::config;
use crate::connection::Incoming;
use crate::conversation::ItemId;
use crate::environment::Environment;
use crate::host::Host;
use crate::protocol::{self, Function};
use crate::sessions;

/// How many hosts are kept with nothing to do: neither shown nor working.
/// Past that, the one shown longest ago is closed; its session is as it
/// was, and showing it again starts a host for it.
const KEPT_IDLE: usize = 2;

/// How the workspace starts the hosts of the project's other sessions, and
/// asks rumbo for what no host says: the rumbo executable, and what it
/// runs in.
#[derive(Clone)]
pub struct Launcher {
    pub rumbo: PathBuf,
    pub environment: Environment,
}

/// A session of the project, attached to a host of its own.
pub(super) struct Attached {
    /// Tells this host from the others for as long as the window lives,
    /// while its session goes from none to the id its first run gives it.
    pub(super) serial: u64,
    /// The host's session, empty for a new one until its first run makes it.
    pub(super) session: String,
    /// What the host calls the session, empty until it has a title.
    pub(super) title: String,
    pub(super) host: Host,
    /// What the session uses, as the host said last.
    pub(super) provider: String,
    pub(super) model: String,
    /// What the provider can be asked, or why that is not known.
    pub(super) provider_functions: Result<Vec<Function>, String>,
    /// When the session last stopped being shown.
    left: Instant,
    _reading: Task<()>,
}

/// What the workspace does once a session it was asked to show is shown.
pub(super) type Arrived = Box<dyn FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>)>;

impl Workspace {
    /// Takes a host as one of the project's, and reads what it sends.
    pub(super) fn hold(&mut self, host: Host, window: &mut Window, cx: &mut Context<Self>) -> u64 {
        self.serials += 1;

        let serial = self.serials;
        let incoming = host.incoming.clone();

        let reading = cx.spawn_in(window, async move |this, cx| {
            while let Ok(message) = incoming.recv().await {
                let closed = matches!(message, Incoming::Closed);
                let kept = this.update_in(cx, |this, window, cx| {
                    this.receive(serial, message, window, cx)
                });

                if kept.is_err() || closed {
                    break;
                }
            }
        });

        let initialize = host.initialize.clone();

        self.hosts.push(Attached {
            serial,
            session: initialize.session,
            title: initialize.title,
            host,
            provider: initialize.provider,
            model: initialize.model,
            provider_functions: Ok(Vec::new()),
            left: Instant::now(),
            _reading: reading,
        });

        // What the provider offers, which the palette lists.
        self.request_of(
            serial,
            protocol::METHOD_DESCRIBE,
            Json::Null,
            Pending::Describe,
        );

        serial
    }

    /// The host of the session shown, when it has one.
    pub(super) fn here(&self) -> Option<&Attached> {
        self.hosts
            .iter()
            .find(|held| held.session == self.current && !self.blank)
    }

    pub(super) fn held(&self, serial: u64) -> Option<&Attached> {
        self.hosts.iter().find(|held| held.serial == serial)
    }

    pub(super) fn held_mut(&mut self, serial: u64) -> Option<&mut Attached> {
        self.hosts.iter_mut().find(|held| held.serial == serial)
    }

    /// The host that holds a session: for a new one, the one nothing was
    /// asked in yet.
    fn holding(&self, session: &str) -> Option<&Attached> {
        self.hosts
            .iter()
            .find(|held| held.session == session && (!session.is_empty() || !self.is_working("")))
    }

    /// The provider and the model of the session shown, as its host said.
    pub(super) fn provider(&self) -> &str {
        self.here()
            .map(|held| held.provider.as_str())
            .unwrap_or_default()
    }

    pub(super) fn model(&self) -> &str {
        self.here()
            .map(|held| held.model.as_str())
            .unwrap_or_default()
    }

    /// What the provider of the session shown can be asked, or why that is
    /// not known.
    pub(super) fn provider_functions(&self) -> Result<Vec<Function>, String> {
        self.here()
            .map(|held| held.provider_functions.clone())
            .unwrap_or_else(|| Ok(Vec::new()))
    }

    /// Sends a request to one of the hosts, whose answer the workspace
    /// waits for. A request that cannot be sent means the host is gone,
    /// which its reader says.
    pub(super) fn request_of(
        &mut self,
        serial: u64,
        method: &str,
        params: impl serde::Serialize,
        pending: Pending,
    ) -> Option<i64> {
        let sent = self.held(serial)?.host.connection.request(method, params);

        match sent {
            Ok(id) => {
                self.pending.insert(id, (serial, pending));
                Some(id)
            }

            Err(_) => None,
        }
    }

    /// Shows a session of the project, or a new one for none, and then
    /// goes on with `then`: at once when a host of the window holds it, and
    /// once one has started for it otherwise. `went` says the person went
    /// there, which is a place to go back to; going back or forward is not.
    pub(super) fn reach(
        &mut self,
        session: &str,
        went: bool,
        then: Option<Arrived>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.holding(session).is_some() {
            self.arrive(session, went, then, window, cx);
            return;
        }

        // The first run of a new session names it at once; until then
        // there is no telling it from another new one.
        let unnamed = session.is_empty() && self.is_working("");

        if self.opening.is_some() || unnamed {
            self.toast(Toast::new("Another session is still opening"), cx);
            self.stay(went, cx);
            return;
        }

        self.opening = Some(session.to_string());

        let launcher = self.launcher.clone();
        let dir = PathBuf::from(&self.dir);
        let flags = config::continuing(session);
        let session = session.to_string();

        cx.spawn_in(window, async move |this, cx| {
            let started = cx
                .background_spawn(async move {
                    Host::start(&launcher.rumbo, &dir, &flags, &launcher.environment)
                })
                .await;

            let _ = this.update_in(cx, |this, window, cx| {
                this.opening = None;

                match started {
                    Ok(host) => {
                        // What the window showed of the session before
                        // this host is read again from it.
                        this.views.remove(&session);
                        this.hold(host, window, cx);
                        this.arrive(&session, went, then, window, cx);
                    }

                    Err(problem) => {
                        let said = format!("The session could not be opened: {problem}");

                        this.toast(Toast::new(said).danger(), cx);
                        this.stay(went, cx);
                    }
                }

                cx.notify();
            });
        })
        .detach();

        cx.notify();
    }

    /// Shows a session a host of the window holds, and goes on.
    fn arrive(
        &mut self,
        session: &str,
        went: bool,
        then: Option<Arrived>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.revisiting = !went;
        self.show_session(session.to_string(), window, cx);
        self.revisiting = false;

        if let Some(then) = then {
            then(self, window, cx);
        }

        self.tidy();
    }

    /// Says that the session shown is still the place, when going back or
    /// forward could not show another.
    fn stay(&mut self, went: bool, cx: &mut Context<Self>) {
        if !went {
            cx.emit(WorkspaceEvent::Shown {
                session: self.current.clone(),
                went: false,
            });
        }
    }

    /// Notes when the session shown stops being so, which is what says
    /// which host was left longest ago.
    pub(super) fn leave(&mut self) {
        let (current, blank) = (self.current.clone(), self.blank);

        if let Some(held) = self
            .hosts
            .iter_mut()
            .find(|held| held.session == current && !blank)
        {
            held.left = Instant::now();
        }
    }

    /// Whether a host has nothing to do and can be closed: the window
    /// started it, its session is not shown, nothing runs in it, no run
    /// ended there that the person has not seen, and no function of its
    /// provider is in progress.
    fn idle(&self, held: &Attached) -> bool {
        let shown = held.session == self.current && !self.blank;
        let talking = self
            .talking
            .as_ref()
            .is_some_and(|talking| talking.host == held.serial);

        !held.host.is_attached()
            && !shown
            && !self.is_working(&held.session)
            && !self.unseen.contains(&held.session)
            && !talking
    }

    /// Closes the hosts with nothing to do past the ones shown last, so the
    /// sessions a person left do not go on costing their processes.
    pub(super) fn tidy(&mut self) {
        let mut idle: Vec<(Instant, u64)> = self
            .hosts
            .iter()
            .filter(|held| self.idle(held))
            .map(|held| (held.left, held.serial))
            .collect();

        if idle.len() <= KEPT_IDLE {
            return;
        }

        idle.sort();

        let closing = idle.len() - KEPT_IDLE;

        for (_, serial) in idle.into_iter().take(closing) {
            self.close(serial);
        }
    }

    /// Lets go of a host and ends it. What the window showed of its session
    /// goes with it, so the session is read again when it is shown again,
    /// as it may have gone on elsewhere.
    fn close(&mut self, serial: u64) {
        let Some(index) = self.hosts.iter().position(|held| held.serial == serial) else {
            return;
        };

        let held = self.hosts.remove(index);

        held.host.stop();
        self.pending.retain(|_, (owner, _)| *owner != serial);
        self.views.remove(&held.session);
        self.unseen.remove(&held.session);
    }

    /// Ends every host of the project, as closing it does.
    pub(super) fn close_all(&mut self) {
        for held in self.hosts.drain(..) {
            held.host.stop();
        }
    }

    /// Asks rumbo for the project's sessions, which the sidebar lists, in
    /// the background.
    pub(super) fn refresh_sessions(&mut self, cx: &mut Context<Self>) {
        let launcher = self.launcher.clone();
        let dir = PathBuf::from(&self.dir);

        self._listing = Some(cx.spawn(async move |this, cx| {
            let listed = cx
                .background_spawn(async move {
                    sessions::list(&launcher.rumbo, &dir, false, &launcher.environment)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if let Ok(listed) = listed {
                    let now = listed.iter().map(sessions::Listed::value).collect();

                    this.listed = crate::order::kept(&this.listed, now);
                    cx.notify();
                }
            });
        }));
    }

    /// Deletes a session for good, once the person agrees. Its host is
    /// closed first, since rumbo keeps a session a process holds; the one
    /// of the rumbo that started the window cannot be, and says so.
    pub fn delete(&mut self, session: &str, window: &mut Window, cx: &mut Context<Self>) {
        if session.is_empty() {
            return;
        }

        let attached = self
            .hosts
            .iter()
            .any(|held| held.session == session && held.host.is_attached());

        if attached {
            let said = "This is the session rumbo started the window in, which holds it. Delete it once the window is closed, with rumbo sessions rm.";

            self.toast(Toast::new(said).danger(), cx);
            return;
        }

        if self.is_working(session) {
            self.toast(
                Toast::new("A run works in that session. Cancel it before deleting the session.")
                    .danger(),
                cx,
            );
            return;
        }

        let workspace = cx.weak_entity();
        let title = self.title_of(session);
        let session = session.to_string();

        self.dialogs.update(cx, |dialogs, cx| {
            dialogs.open(
                move |dialog, _, _| {
                    let workspace = workspace.clone();
                    let session = session.clone();

                    dialog
                        .title("Delete session")
                        .child(format!(
                            "This deletes “{title}” and its history, for good. The files it changed stay as they are."
                        ))
                        .danger()
                        .on_ok("Delete", move |_, cx| {
                            let _ = workspace.update(cx, |this, cx| this.remove(&session, cx));
                            true
                        })
                },
                window,
                cx,
            )
        });
    }

    /// Deletes a session the person agreed to delete, with rumbo's command
    /// line, in the background.
    fn remove(&mut self, session: &str, cx: &mut Context<Self>) {
        if let Some(serial) = self
            .hosts
            .iter()
            .find(|held| held.session == session)
            .map(|held| held.serial)
        {
            self.close(serial);
        }

        // Deleting the session shown leaves none shown.
        if session == self.current && !self.blank {
            self.show_nothing(cx);
        }

        let launcher = self.launcher.clone();
        let dir = PathBuf::from(&self.dir);
        let deleted = session.to_string();

        cx.spawn(async move |this, cx| {
            let removed = cx
                .background_spawn(async move {
                    sessions::remove(&launcher.rumbo, &dir, &deleted, &launcher.environment)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                match removed {
                    Ok(()) => this.toast(Toast::new("The session was deleted"), cx),

                    Err(problem) => {
                        let said = format!("The session could not be deleted: {problem}");

                        this.toast(Toast::new(said).danger(), cx)
                    }
                }

                this.refresh_sessions(cx);
                cx.emit(WorkspaceEvent::Answered);
            });
        })
        .detach();

        cx.notify();
    }

    /// Takes in that a host ended. The window ends with the rumbo that
    /// started it. Another that ended by itself leaves its session as it
    /// was: its runs are told as failed, and when it is the one shown the
    /// workspace says so, and offers to start it again.
    pub(super) fn closed(&mut self, serial: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.hosts.iter().position(|held| held.serial == serial) else {
            return;
        };

        if self.hosts[index].host.is_attached() {
            cx.quit();
            return;
        }

        let held = self.hosts.remove(index);
        let said = held.host.stderr();
        let shown = held.session == self.current && !self.blank;

        if shown {
            self.ended = Some(if said.is_empty() {
                "The host ended.".into()
            } else {
                format!("The host ended: {said}")
            });

            self.start_again_focus.focus(window, cx);
        }

        if self
            .talking
            .as_ref()
            .is_some_and(|talking| talking.host == serial)
        {
            self.talking = None;
        }

        // What still waited for a run never reaches it.
        let asked: Vec<i64> = self
            .pending
            .iter()
            .filter(|(_, (owner, _))| *owner == serial)
            .map(|(id, _)| *id)
            .collect();

        let mut waiting: Vec<(String, ItemId)> = Vec::new();
        let mut runs: Vec<String> = Vec::new();

        for id in asked {
            match self.pending.remove(&id) {
                Some((_, Pending::Steer { session, item })) => waiting.push((session, item)),

                Some((_, Pending::Run)) => {
                    self.started.remove(&id);

                    if let Some(key) = self.runs.remove(&id) {
                        runs.push(key);
                    }
                }

                _ => {}
            }
        }

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

    /// Starts a host again for the session shown, after its host ended.
    pub(super) fn start_again(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.current.clone();

        self.reach(&session, false, None, window, cx);
    }
}
