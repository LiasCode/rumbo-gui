//! What rumbo holds in every project: the projects that have sessions, and
//! the sessions of each. A host works in one project, but lists any
//! project's sessions, so the window starts one only to ask it; it works in
//! nothing and runs nothing. A host without `projects` lists nothing here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gpui_kit::{AppContext as _, Context, Task};
use serde_json::{Value as Json, json};

use crate::commands;
use crate::connection::Incoming;
use crate::conversation::Source;
use crate::host::Host;
use crate::protocol::{self, CODE_INVALID_PARAMS, InvokeParams, Value};

/// What a request of the catalog asked.
enum Asked {
    Projects,
    /// The sessions of the project with this root.
    Sessions(String),
}

pub(super) struct Catalog {
    host: Option<Host>,
    pending: HashMap<i64, Asked>,
    /// The projects that have sessions, most recently used first, by root.
    projects: Vec<Value>,
    sessions: HashMap<String, Vec<Value>>,
    /// Why there is nothing to list: rumbo did not start, or stopped.
    problem: Option<String>,
    _reading: Option<Task<()>>,
}

impl Catalog {
    pub(super) fn new() -> Self {
        Catalog {
            host: None,
            pending: HashMap::new(),
            projects: Vec::new(),
            sessions: HashMap::new(),
            problem: None,
            _reading: None,
        }
    }

    /// Starts the host that lists, in the background. It works in `dir`,
    /// where it makes nothing until a run, and runs none.
    pub(super) fn start(rumbo: PathBuf, dir: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let started = cx
                .background_spawn(async move { Host::start(&rumbo, &dir, &[]) })
                .await;

            let _ = this.update(cx, |this, cx| match started {
                Ok(host) => this.attach(host, cx),

                Err(problem) => {
                    this.problem = Some(problem);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn attach(&mut self, host: Host, cx: &mut Context<Self>) {
        let offered = [protocol::FUNCTION_PROJECTS, protocol::FUNCTION_SESSIONS]
            .iter()
            .all(|name| {
                host.initialize
                    .functions
                    .iter()
                    .any(|function| function.name == *name)
            });

        // An older rumbo cannot list other projects; its host would only
        // wait.
        if !offered {
            host.stop();
            return;
        }

        let incoming = host.incoming.clone();

        self._reading = Some(cx.spawn(async move |this, cx| {
            while let Ok(message) = incoming.recv().await {
                let closed = matches!(message, Incoming::Closed);
                let kept = this.update(cx, |this, cx| this.receive(message, cx));

                if kept.is_err() || closed {
                    break;
                }
            }
        }));

        self.host = Some(host);
        self.refresh();
    }

    /// Asks again what rumbo holds, as it can change from anywhere: another
    /// front end may have made a session.
    pub(super) fn refresh(&mut self) {
        self.ask(protocol::FUNCTION_PROJECTS, None, Asked::Projects);
    }

    pub(super) fn projects(&self) -> &[Value] {
        &self.projects
    }

    pub(super) fn sessions(&self, root: &str) -> &[Value] {
        self.sessions
            .get(root)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(super) fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    /// The root of the project a session belongs to, as listed.
    pub(super) fn root_of(&self, session: &str) -> Option<&str> {
        self.sessions
            .iter()
            .find(|(_, sessions)| sessions.iter().any(|listed| listed.value == session))
            .map(|(root, _)| root.as_str())
    }

    fn ask(&mut self, name: &str, params: Option<Json>, asked: Asked) {
        let Some(host) = &self.host else {
            return;
        };

        let sent = host.connection.request(
            protocol::METHOD_INVOKE,
            InvokeParams {
                name: name.into(),
                params,
                target: protocol::TARGET_HOST,
            },
        );

        if let Ok(id) = sent {
            self.pending.insert(id, asked);
        }
    }

    fn receive(&mut self, message: Incoming, cx: &mut Context<Self>) {
        match message {
            Incoming::Answer { id, answer } => {
                let Some(asked) = self.pending.remove(&id) else {
                    return;
                };

                let Ok(answer) = answer else {
                    return;
                };

                let (values, _) = commands::listed(Source::Host, answer);

                match asked {
                    Asked::Projects => {
                        for project in &values {
                            let params = json!({ "project": project.value });

                            self.ask(
                                protocol::FUNCTION_SESSIONS,
                                Some(params),
                                Asked::Sessions(project.value.clone()),
                            );
                        }

                        self.projects = values;
                    }

                    Asked::Sessions(root) => {
                        self.sessions.insert(root, values);
                    }
                }
            }

            // It runs nothing, and follows no function.
            Incoming::Event(_) => return,

            Incoming::Ask { id, .. } => {
                if let Some(host) = &self.host {
                    let _ = host.connection.refuse(
                        id,
                        CODE_INVALID_PARAMS,
                        "this front end is not following that function, so it cannot answer for it",
                    );
                }

                return;
            }

            Incoming::Closed => {
                let said = self
                    .host
                    .take()
                    .map(|host| host.stderr())
                    .unwrap_or_default();

                self.problem = Some(if said.is_empty() {
                    "rumbo stopped listing the projects.".into()
                } else {
                    format!("rumbo stopped listing the projects: {said}")
                });
            }
        }

        cx.notify();
    }
}

impl Drop for Catalog {
    fn drop(&mut self) {
        if let Some(host) = &self.host {
            host.stop();
        }
    }
}

/// Where the host that lists works: the person's home, or the root of the
/// file system without one. It makes nothing there until a run.
pub(super) fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_dir())
        .unwrap_or_else(|| Path::new("/").to_path_buf())
}
