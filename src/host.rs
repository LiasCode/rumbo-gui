//! Starting `rumbo serve` in a folder, which is how the desktop app reaches a
//! host: the protocol runs on the host's stdin and stdout.

use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use async_channel::Receiver;

use crate::connection::{Connection, Incoming};
use crate::protocol::InitializeParams;

/// How much of what the host writes to stderr is kept. The host writes there
/// only before a front end is attached, so this holds any error that made it
/// stop.
const KEPT_STDERR: usize = 16 * 1024;

/// A host this front end started, and is attached to.
pub struct Host {
    pub connection: Connection,
    pub initialize: InitializeParams,
    pub incoming: Receiver<Incoming>,
    process: Arc<Mutex<Child>>,
    stderr: Arc<Mutex<Vec<u8>>>,
}

impl Host {
    /// Starts `rumbo serve` with `flags` in `dir` and answers its
    /// initialize. It blocks until the host has spoken, so it belongs on a
    /// background thread.
    pub fn start(rumbo: &Path, dir: &Path, flags: &[String]) -> Result<Host, String> {
        let mut process = Command::new(rumbo)
            .arg("serve")
            .args(flags)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| match error.kind() {
                io::ErrorKind::NotFound => format!(
                    "{} was not found. Install rumbo on PATH, or start rumbo-gui with --rumbo and its path.",
                    rumbo.display()
                ),
                _ => format!("{} could not start: {error}", rumbo.display()),
            })?;

        let stdin = process.stdin.take().expect("stdin is piped");
        let stdout = process.stdout.take().expect("stdout is piped");
        let (stderr, reading_stderr) = keep_stderr(process.stderr.take().expect("stderr is piped"));
        let process = Arc::new(Mutex::new(process));

        match Connection::open(stdout, stdin) {
            Ok((connection, initialize, incoming)) => Ok(Host {
                connection,
                initialize,
                incoming,
                process,
                stderr,
            }),

            Err(error) => {
                let mut child = process
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());

                // A host that did speak but said something wrong is still
                // running.
                let _ = child.kill();

                let status = child
                    .wait()
                    .map(|status| status.to_string())
                    .unwrap_or_default();
                let _ = reading_stderr.join();
                let said = String::from_utf8_lossy(
                    &stderr
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()),
                )
                .trim()
                .to_string();

                if said.is_empty() {
                    Err(format!("rumbo serve stopped ({status}): {error}"))
                } else {
                    Err(said)
                }
            }
        }
    }

    /// What the host wrote to stderr, such as the reason it stopped.
    pub fn stderr(&self) -> String {
        let kept = self
            .stderr
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        String::from_utf8_lossy(&kept).trim().to_string()
    }

    /// Closes the connection, which ends the host, and waits for it in the
    /// background so it does not linger as a zombie while the window lives.
    pub fn stop(&self) {
        self.connection.close();

        let process = self.process.clone();

        thread::spawn(move || {
            let _ = process
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .wait();
        });
    }
}

/// Reads stderr to its end, keeping its last part. Reading it all keeps the
/// host from blocking on a full pipe.
fn keep_stderr(mut stderr: impl Read + Send + 'static) -> (Arc<Mutex<Vec<u8>>>, JoinHandle<()>) {
    let kept = Arc::new(Mutex::new(Vec::new()));
    let filling = kept.clone();

    let reading = thread::spawn(move || {
        let mut buffer = [0; 4096];

        while let Ok(read) = stderr.read(&mut buffer) {
            if read == 0 {
                break;
            }

            let mut kept = filling
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

            kept.extend_from_slice(&buffer[..read]);

            if kept.len() > KEPT_STDERR {
                let excess = kept.len() - KEPT_STDERR;
                kept.drain(..excess);
            }
        }
    });

    (kept, reading)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_how_to_fix_a_missing_rumbo() {
        let error = Host::start(Path::new("/no/such/rumbo"), Path::new("."), &[])
            .err()
            .expect("a missing rumbo started");

        assert!(error.contains("was not found"), "{error}");
        assert!(error.contains("--rumbo"), "{error}");
    }

    #[test]
    fn passes_on_what_a_host_said_before_it_stopped() {
        // `sh serve` fails the way rumbo does: a line on stderr, and exit.
        let error = Host::start(Path::new("sh"), Path::new("."), &[])
            .err()
            .expect("sh served");

        assert!(error.contains("serve"), "{error}");
    }
}
