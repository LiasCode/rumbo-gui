//! The sessions rumbo holds, asked of its command line. A host is one
//! session and knows no other, so the window learns the sessions there are
//! from `rumbo sessions -json`, starts a host for the one a person opens,
//! and deletes one with `rumbo sessions rm`. Nothing here reads rumbo's
//! files.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::environment::Environment;
use crate::protocol::Value;

/// A session as rumbo lists it for a program. Fields it does not know are
/// ignored, as rumbo may add some.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Listed {
    pub session: String,
    pub title: String,
    /// The session's root, which the window groups sessions by.
    pub dir: String,
    /// How its last run ended.
    pub state: String,
    /// Whether a rumbo process holds it now.
    pub in_use: bool,
    pub turns: u64,
}

impl Listed {
    /// The session as a row of a list: its id, its title, and how it
    /// stands, in words.
    pub fn value(&self) -> Value {
        let turns = match self.turns {
            1 => "1 turn".to_string(),
            turns => format!("{turns} turns"),
        };

        let standing = match self.in_use {
            true => format!("{}, {turns}, held by a rumbo", self.state),
            false => format!("{}, {turns}", self.state),
        };

        Value {
            value: self.session.clone(),
            label: self.title.clone(),
            description: standing,
        }
    }
}

/// The sessions of `dir`, or of every folder with `all`, most recently used
/// first. It runs rumbo and waits for it, so it belongs on a background
/// thread.
pub fn list(
    rumbo: &Path,
    dir: &Path,
    all: bool,
    environment: &Environment,
) -> Result<Vec<Listed>, String> {
    let mut flags = vec!["-json"];

    if all {
        flags.push("-all");
    }

    let printed = run(rumbo, dir, &flags, environment)?;

    read(&printed)
}

/// Deletes a session and its history. rumbo refuses one a process holds.
pub fn remove(
    rumbo: &Path,
    dir: &Path,
    session: &str,
    environment: &Environment,
) -> Result<(), String> {
    run(rumbo, dir, &["rm", session], environment).map(|_| ())
}

/// Runs `rumbo sessions` with `arguments` in `dir`, and returns what it
/// printed, or what it said went wrong.
fn run(
    rumbo: &Path,
    dir: &Path,
    arguments: &[&str],
    environment: &Environment,
) -> Result<String, String> {
    let output = Command::new(rumbo)
        .arg("sessions")
        .args(arguments)
        .current_dir(dir)
        .envs(
            environment
                .variables()
                .iter()
                .map(|(name, value)| (name, value)),
        )
        .output()
        .map_err(|error| format!("{} could not start: {error}", rumbo.display()))?;

    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(match said.is_empty() {
            true => format!("rumbo sessions stopped ({})", output.status),
            false => said,
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Reads what `rumbo sessions -json` printed.
fn read(printed: &str) -> Result<Vec<Listed>, String> {
    serde_json::from_str(printed)
        .map_err(|error| format!("rumbo did not list its sessions as expected: {error}"))
}

/// The folders that have sessions, most recently used first, each with its
/// sessions in the order rumbo listed them.
pub fn by_folder(listed: &[Listed]) -> Vec<(String, Vec<Listed>)> {
    let mut folders: Vec<(String, Vec<Listed>)> = Vec::new();

    for session in listed {
        match folders.iter_mut().find(|(dir, _)| *dir == session.dir) {
            Some((_, sessions)) => sessions.push(session.clone()),
            None => folders.push((session.dir.clone(), vec![session.clone()])),
        }
    }

    folders
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRINTED: &str = r#"[
        {"session": "b2", "title": "write the docs", "dir": "/home/me/site", "used": "2026-11-24T10:02:11Z", "state": "done", "in_use": true, "turns": 6, "later": 1},
        {"session": "a1", "title": "fix the test", "dir": "/home/me/project", "state": "failed", "in_use": false, "turns": 1},
        {"session": "c3", "title": "", "dir": "/home/me/site", "parent": "b2", "state": "new", "in_use": false, "turns": 0}
    ]"#;

    #[test]
    fn reads_what_rumbo_lists_and_ignores_what_it_does_not_know() {
        let listed = read(PRINTED).unwrap();

        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].dir, "/home/me/site");
        assert_eq!(
            listed[0].value(),
            Value {
                value: "b2".into(),
                label: "write the docs".into(),
                description: "done, 6 turns, held by a rumbo".into(),
            }
        );
        assert_eq!(listed[1].value().description, "failed, 1 turn");
        assert!(read("[]").unwrap().is_empty());
        assert!(read("no sessions yet").is_err());
    }

    #[test]
    fn groups_the_sessions_by_folder_keeping_their_order() {
        let folders = by_folder(&read(PRINTED).unwrap());
        let ids = |sessions: &[Listed]| -> Vec<String> {
            sessions.iter().map(|each| each.session.clone()).collect()
        };

        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].0, "/home/me/site");
        assert_eq!(ids(&folders[0].1), ["b2", "c3"]);
        assert_eq!(folders[1].0, "/home/me/project");
        assert_eq!(ids(&folders[1].1), ["a1"]);
    }

    #[test]
    fn says_what_went_wrong_when_rumbo_does_not_list() {
        // `sh sessions -json` fails the way rumbo would: a line on stderr.
        let error = list(
            Path::new("sh"),
            Path::new("."),
            false,
            &Environment::inherited(),
        )
        .unwrap_err();

        assert!(error.contains("sessions"), "{error}");
    }
}
