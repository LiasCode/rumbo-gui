//! What rumbo holds in every folder: the projects that have sessions, and
//! the sessions of each. A host is one session and lists no other, so the
//! window asks rumbo's command line, `rumbo sessions -json -all`, on a
//! background thread, and reads none of rumbo's files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gpui::{AppContext as _, Context, Task};

use crate::environment::Environment;
use crate::protocol::Value;
use crate::sessions;

pub(super) struct Catalog {
    /// The rumbo that lists, and what it runs in.
    rumbo: PathBuf,
    environment: Environment,
    /// The projects that have sessions, most recently used first, by root.
    projects: Vec<Value>,
    /// The sessions of each project, by root, in the order they were first
    /// listed, the newest first.
    sessions: HashMap<String, Vec<Value>>,
    /// Why there is nothing to list: rumbo did not start, or did not list.
    problem: Option<String>,
    _listing: Option<Task<()>>,
}

impl Catalog {
    pub(super) fn new(rumbo: PathBuf, environment: Environment, cx: &mut Context<Self>) -> Self {
        let mut catalog = Catalog {
            rumbo,
            environment,
            projects: Vec::new(),
            sessions: HashMap::new(),
            problem: None,
            _listing: None,
        };

        catalog.refresh(cx);
        catalog
    }

    /// Asks again what rumbo holds, as it can change from anywhere: another
    /// front end may have made a session. One asked while another is on its
    /// way takes its place.
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        let rumbo = self.rumbo.clone();
        let environment = self.environment.clone();

        self._listing = Some(cx.spawn(async move |this, cx| {
            let listed = cx
                .background_spawn(
                    async move { sessions::list(&rumbo, &home(), true, &environment) },
                )
                .await;

            let _ = this.update(cx, |this, cx| {
                this.listed(listed);
                cx.notify();
            });
        }));
    }

    fn listed(&mut self, listed: Result<Vec<sessions::Listed>, String>) {
        let listed = match listed {
            Ok(listed) => listed,

            Err(problem) => {
                self.problem = Some(problem);
                return;
            }
        };

        let folders = sessions::by_folder(&listed);

        self.problem = None;
        self.projects = folders
            .iter()
            .map(|(root, sessions)| Value {
                value: root.clone(),
                label: folder_name(root),
                description: match sessions.len() {
                    1 => "1 session".to_string(),
                    count => format!("{count} sessions"),
                },
            })
            .collect();

        let mut kept = HashMap::new();

        for (root, sessions) in folders {
            let now = sessions.iter().map(sessions::Listed::value).collect();

            kept.insert(root.clone(), crate::order::kept(self.sessions(&root), now));
        }

        self.sessions = kept;
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
}

/// What a project is called: its folder's name, or its path when it has
/// none, as the root of the file system.
fn folder_name(root: &str) -> String {
    Path::new(root)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| root.to_string())
}

/// The person's home, where a session asked for with no project open
/// starts, and where rumbo is run to list: the root of the file system
/// without one.
pub(super) fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_dir())
        .unwrap_or_else(|| Path::new("/").to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(session: &str, dir: &str) -> sessions::Listed {
        sessions::Listed {
            session: session.into(),
            dir: dir.into(),
            ..sessions::Listed::default()
        }
    }

    fn catalog() -> Catalog {
        Catalog {
            rumbo: PathBuf::from("rumbo"),
            environment: Environment::inherited(),
            projects: Vec::new(),
            sessions: HashMap::new(),
            problem: None,
            _listing: None,
        }
    }

    fn ids(values: &[Value]) -> Vec<&str> {
        values.iter().map(|value| value.value.as_str()).collect()
    }

    #[test]
    fn lists_the_projects_and_keeps_the_order_of_their_sessions() {
        let mut catalog = catalog();

        catalog.listed(Ok(vec![
            listed("b", "/home/me/site"),
            listed("a", "/home/me/site"),
        ]));

        assert_eq!(ids(catalog.projects()), ["/home/me/site"]);
        assert_eq!(catalog.projects()[0].label, "site");
        assert_eq!(ids(catalog.sessions("/home/me/site")), ["b", "a"]);

        // A run in the older one lists it first; it stays where it was, and
        // a new one comes first.
        catalog.listed(Ok(vec![
            listed("a", "/home/me/site"),
            listed("c", "/home/me/site"),
            listed("b", "/home/me/site"),
            listed("z", "/"),
        ]));

        assert_eq!(ids(catalog.sessions("/home/me/site")), ["c", "b", "a"]);
        assert_eq!(catalog.projects()[1].label, "/");
    }

    #[test]
    fn says_why_nothing_is_listed_and_keeps_what_was() {
        let mut catalog = catalog();

        catalog.listed(Ok(vec![listed("a", "/p")]));
        catalog.listed(Err("rumbo was not found".into()));

        assert_eq!(catalog.problem(), Some("rumbo was not found"));
        assert_eq!(ids(catalog.projects()), ["/p"]);

        catalog.listed(Ok(Vec::new()));

        assert_eq!(catalog.problem(), None);
        assert!(catalog.projects().is_empty());
    }
}
