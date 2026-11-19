//! The environment the hosts the window starts run in, and with them the
//! providers and the tools. Started from a terminal, the window has the
//! person's environment, and passes it on as it is. Started from the
//! desktop, as a launcher does, it has the session's, which lacks what the
//! person's shell adds, such as where rumbo and the providers' commands are
//! installed; so the window asks the person's login shell for its
//! environment, once, when the first host starts.

use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal as _, Read as _};
use std::os::unix::ffi::OsStrExt as _;
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock, mpsc};
use std::thread;
use std::time::Duration;

/// How long the person's shell has to say its environment, before the
/// window's own is used.
const SHELL_TIME: Duration = Duration::from_secs(5);

/// Marks where the environment begins and ends in what the shell writes,
/// which also holds whatever its startup files print.
const MARKER: &str = "__rumbo_gui_environment__";

/// What describes the shell that was asked, not the person's environment.
const SHELL_OWN: [&str; 4] = ["PWD", "OLDPWD", "SHLVL", "_"];

pub type Variables = Vec<(OsString, OsString)>;

/// What the hosts are started with, on top of the window's own environment.
#[derive(Clone)]
pub struct Environment {
    /// The shell to ask; none when the window's environment is the person's.
    shell: Option<OsString>,
    asked: Arc<OnceLock<Variables>>,
}

impl Environment {
    /// The environment of this start: the window's own when it was started
    /// from a terminal, else that of the person's shell.
    pub fn new() -> Self {
        let from_terminal = io::stdout().is_terminal() || io::stderr().is_terminal();

        let shell = std::env::var_os("SHELL").filter(|shell| !shell.is_empty() && !from_terminal);

        Environment {
            shell,
            asked: Arc::default(),
        }
    }

    /// The window's own environment, as it is.
    pub fn inherited() -> Self {
        Environment {
            shell: None,
            asked: Arc::default(),
        }
    }

    /// The variables a host is started with. The first call asks the
    /// person's shell and blocks until it answers or its time is up, so it
    /// belongs on a background thread; the calls meanwhile wait for it. A
    /// shell that does not answer leaves the window's own environment.
    pub fn variables(&self) -> &[(OsString, OsString)] {
        self.asked.get_or_init(|| {
            self.shell
                .as_deref()
                .and_then(|shell| ask(shell, SHELL_TIME))
                .unwrap_or_default()
        })
    }
}

/// Asks `shell` for its environment, as a login and interactive shell, so
/// that it reads the files where people set their PATH. None when it does
/// not start, or does not answer within `limit`.
fn ask(shell: &OsStr, limit: Duration) -> Option<Variables> {
    let mut child = Command::new(shell)
        .args(["-l", "-i", "-c"])
        .arg(format!("echo {MARKER}; env -0; echo {MARKER}"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let mut stdout = child.stdout.take()?;
    let (send, answer) = mpsc::channel();

    // Read on a thread of its own so that waiting has a limit: a shell can
    // hang, and a program it started can keep its stdout open after it
    // exits, so the answer ends at its second marker, not at the end.
    thread::spawn(move || {
        let mut said = Vec::new();
        let mut buffer = [0; 4096];

        while let Ok(read) = stdout.read(&mut buffer) {
            if read == 0 {
                break;
            }

            said.extend_from_slice(&buffer[..read]);

            if let Some(variables) = parse(&said) {
                let _ = send.send(variables);
                return;
            }
        }
    });

    let answered = answer.recv_timeout(limit).ok();

    // A shell that answered has exited or is exiting; one that did not is
    // given up on.
    let _ = child.kill();
    let _ = child.wait();

    answered
}

/// The variables between the two markers in what the shell said, but those
/// of the shell itself; none until both markers are there.
fn parse(said: &[u8]) -> Option<Variables> {
    let marker = format!("{MARKER}\n");
    let marker = marker.as_bytes();

    let start = find(said, marker)? + marker.len();
    let end = start + find(&said[start..], marker)?;

    let variables = said[start..end]
        .split(|byte| *byte == 0)
        .filter_map(|entry| {
            let equals = entry.iter().position(|byte| *byte == b'=')?;
            let name = OsStr::from_bytes(&entry[..equals]);

            if equals == 0 || SHELL_OWN.iter().any(|own| OsStr::new(own) == name) {
                return None;
            }

            Some((
                name.to_os_string(),
                OsStr::from_bytes(&entry[equals + 1..]).to_os_string(),
            ))
        })
        .collect();

    Some(variables)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(text: &str) -> Vec<u8> {
        text.replace("{M}", MARKER).into_bytes()
    }

    fn pair(name: &str, value: &str) -> (OsString, OsString) {
        (name.into(), value.into())
    }

    #[test]
    fn takes_the_variables_between_what_the_startup_files_print() {
        let variables = parse(&said(
            "welcome back\n{M}\nPATH=/a:/b\0KEY=x=y\0MULTI=one\ntwo\0{M}\nbye\n",
        ));

        assert_eq!(
            variables,
            Some(vec![
                pair("PATH", "/a:/b"),
                pair("KEY", "x=y"),
                pair("MULTI", "one\ntwo"),
            ])
        );
    }

    #[test]
    fn leaves_out_what_describes_the_shell_asked() {
        let variables = parse(&said(
            "{M}\nPWD=/x\0SHLVL=2\0_=/usr/bin/env\0HOME=/h\0{M}\n",
        ));

        assert_eq!(variables, Some(vec![pair("HOME", "/h")]));
    }

    #[test]
    fn waits_for_both_markers() {
        assert_eq!(parse(&said("noise\n")), None);
        assert_eq!(parse(&said("{M}\nPATH=/a\0")), None);
    }

    #[test]
    fn asks_a_shell_for_its_environment() {
        let variables = ask(OsStr::new("sh"), SHELL_TIME).expect("sh did not answer");

        assert!(variables.iter().any(|(name, _)| name == "PATH"));
        assert!(!variables.iter().any(|(name, _)| name == "SHLVL"));
    }

    #[test]
    fn a_shell_that_does_not_start_leaves_the_window_environment() {
        assert_eq!(ask(OsStr::new("/no/such/shell"), SHELL_TIME), None);

        let environment = Environment {
            shell: Some("/no/such/shell".into()),
            asked: Arc::default(),
        };

        assert!(environment.variables().is_empty());
    }
}
