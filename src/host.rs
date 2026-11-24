//! Reaching a host: starting `rumbo serve` in a folder, as the desktop app
//! does, with the protocol on the host's stdin and stdout; or attaching to
//! the rumbo that started this front end, with the protocol on file
//! descriptors 3 and 4.

use std::fs::{self, File};
use std::io::{self, Read};
use std::os::fd::FromRawFd;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use async_channel::Receiver;

use crate::connection::{Connection, Incoming};
use crate::environment::Environment;
use crate::protocol::InitializeParams;

/// How much of what the host writes to stderr is kept. The host writes there
/// only before a front end is attached, so this holds any error that made it
/// stop.
const KEPT_STDERR: usize = 16 * 1024;

/// Where rumbo puts the protocol for a front end it starts.
const READ_FD: i32 = 3;
const WRITE_FD: i32 = 4;

/// The protocol rumbo gives a front end it starts, on two pipes.
pub struct Descriptors {
    reader: File,
    writer: File,
}

impl Descriptors {
    /// Takes file descriptors 3 and 4, which must be the pipes rumbo made.
    /// It runs first thing: any file the process opens before could get
    /// those numbers, and be taken for the protocol.
    pub fn take() -> Result<Descriptors, String> {
        let pipes = [READ_FD, WRITE_FD].iter().all(|fd| {
            fs::metadata(format!("/dev/fd/{fd}")).is_ok_and(|found| found.file_type().is_fifo())
        });

        if !pipes {
            return Err("--attached is for rumbo to start this front end, with the protocol on file descriptors 3 and 4. Register it with `rumbo frontend add gui rumbo-gui --attached --rumbo '{self}'`, and run `rumbo -f gui` in a project.".into());
        }

        // SAFETY: both descriptors are open pipes, as checked above, which
        // rumbo gave this process for the protocol alone, and nothing has
        // taken them yet.
        let (reader, writer) = unsafe { (File::from_raw_fd(READ_FD), File::from_raw_fd(WRITE_FD)) };

        Ok(Descriptors { reader, writer })
    }
}

/// A host this front end is attached to.
pub struct Host {
    pub connection: Connection,
    pub initialize: InitializeParams,
    pub incoming: Receiver<Incoming>,
    /// The `rumbo serve` this front end started; none when rumbo started
    /// this front end.
    process: Option<Arc<Mutex<Child>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
}

impl Host {
    /// Starts `rumbo serve` with `flags` in `dir`, in `environment`, and
    /// answers its initialize. It blocks until the host has spoken, so it
    /// belongs on a background thread.
    pub fn start(
        rumbo: &Path,
        dir: &Path,
        flags: &[String],
        environment: &Environment,
    ) -> Result<Host, String> {
        // The PATH set here is also where `rumbo` is looked for.
        let mut process = Command::new(rumbo)
            .arg("serve")
            .args(flags)
            .current_dir(dir)
            .envs(
                environment
                    .variables()
                    .iter()
                    .map(|(name, value)| (name, value)),
            )
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
                process: Some(process),
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

    /// Attaches to the rumbo that started this front end, as a registered
    /// one. It blocks until the host has spoken.
    pub fn attach(descriptors: Descriptors) -> Result<Host, String> {
        let (connection, initialize, incoming) =
            Connection::open(descriptors.reader, descriptors.writer)
                .map_err(|error| format!("rumbo did not start the conversation: {error}"))?;

        Ok(Host {
            connection,
            initialize,
            incoming,
            process: None,
            stderr: Arc::default(),
        })
    }

    /// Whether rumbo started this front end, rather than the other way
    /// round. Such a host cannot be started again from here.
    pub fn is_attached(&self) -> bool {
        self.process.is_none()
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

        let Some(process) = self.process.clone() else {
            return;
        };

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
        let error = Host::start(
            Path::new("/no/such/rumbo"),
            Path::new("."),
            &[],
            &Environment::inherited(),
        )
        .err()
        .expect("a missing rumbo started");

        assert!(error.contains("was not found"), "{error}");
        assert!(error.contains("--rumbo"), "{error}");
    }

    #[test]
    fn passes_on_what_a_host_said_before_it_stopped() {
        // `sh serve` fails the way rumbo does: a line on stderr, and exit.
        let error = Host::start(
            Path::new("sh"),
            Path::new("."),
            &[],
            &Environment::inherited(),
        )
        .err()
        .expect("sh served");

        assert!(error.contains("serve"), "{error}");
    }
}
