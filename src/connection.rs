//! The front end's side of a connection to the rumbo host: JSON-RPC 2.0, one
//! message per line, in both directions.
//!
//! Everything the host sends arrives on one channel, in the order it was
//! sent: answers to requests, events, and the host's own questions. Keeping a
//! single order matters, since a run's last events come before the answer
//! that ends it.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use async_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use crate::protocol::{
    self, AskParams, CODE_METHOD_NOT_FOUND, Event, InitializeParams, InitializeResult, RpcError,
};

/// What the host sent.
#[derive(Debug)]
pub enum Incoming {
    /// The answer to the request with this id.
    Answer {
        id: i64,
        answer: Result<Json, RpcError>,
    },
    /// Something that happened in a run, or that a provider said.
    Event(Box<Event>),
    /// A question for the person, which `answer` or `refuse` must reply to.
    Ask { id: i64, params: AskParams },
    /// The host closed the connection. Nothing follows.
    Closed,
}

/// A connection the host has initialized. It can be shared between threads.
#[derive(Clone)]
pub struct Connection {
    shared: Arc<Shared>,
}

struct Shared {
    /// None once the front end closed the connection.
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    next_id: AtomicI64,
}

#[derive(Deserialize)]
struct Frame {
    id: Option<i64>,
    method: Option<String>,
    params: Option<Json>,
    result: Option<Json>,
    error: Option<RpcError>,
}

impl Connection {
    /// Answers the host's `initialize` and starts reading what follows.
    /// It returns what the host said it is attached to, and the channel
    /// everything else arrives on.
    pub fn open(
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
    ) -> io::Result<(Connection, InitializeParams, Receiver<Incoming>)> {
        let mut reader = BufReader::new(reader);

        let Some(frame) = read_frame(&mut reader)? else {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the host ended before it said anything",
            ));
        };

        let (Some(id), Some(protocol::METHOD_INITIALIZE)) = (frame.id, frame.method.as_deref())
        else {
            return Err(invalid("the host did not start with initialize"));
        };

        let params: InitializeParams =
            serde_json::from_value(frame.params.unwrap_or(Json::Null)).map_err(io::Error::from)?;

        if params.protocol != protocol::VERSION {
            return Err(invalid(&format!(
                "the host speaks protocol {}, and this front end speaks {}",
                params.protocol,
                protocol::VERSION
            )));
        }

        let connection = Connection {
            shared: Arc::new(Shared {
                writer: Mutex::new(Some(Box::new(writer))),
                next_id: AtomicI64::new(1),
            }),
        };

        connection.answer(
            id,
            InitializeResult {
                protocol: protocol::VERSION,
                name: protocol::NAME,
                config: json!({}),
            },
        )?;

        let (sender, receiver) = async_channel::unbounded();
        let reading = connection.clone();

        thread::Builder::new()
            .name("rumbo-host-reader".into())
            .spawn(move || reading.read(reader, sender))?;

        Ok((connection, params, receiver))
    }

    /// Sends a request and returns its id, which its answer will carry.
    pub fn request(&self, method: &str, params: impl Serialize) -> io::Result<i64> {
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);

        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;

        Ok(id)
    }

    /// Sends a notification, which has no answer.
    pub fn notify(&self, method: &str, params: impl Serialize) -> io::Result<()> {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    /// Answers a request of the host.
    pub fn answer(&self, id: i64, result: impl Serialize) -> io::Result<()> {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }

    /// Answers a request of the host with an error.
    pub fn refuse(&self, id: i64, code: i64, message: &str) -> io::Result<()> {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}))
    }

    /// Closes the connection, which ends the host: it cancels its runs,
    /// stops its providers, and exits.
    pub fn close(&self) {
        self.writer().take();
    }

    fn send(&self, frame: &Json) -> io::Result<()> {
        let mut line = serde_json::to_vec(frame)?;
        line.push(b'\n');

        let mut writer = self.writer();

        let Some(writer) = writer.as_mut() else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "the connection is closed",
            ));
        };

        writer.write_all(&line)?;
        writer.flush()
    }

    fn writer(&self) -> std::sync::MutexGuard<'_, Option<Box<dyn Write + Send>>> {
        // A poisoned lock means another write panicked midway; the stream
        // is still whole up to there.
        self.shared
            .writer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn read(&self, mut reader: impl BufRead, sender: Sender<Incoming>) {
        loop {
            let frame = match read_frame(&mut reader) {
                Ok(Some(frame)) => frame,
                Ok(None) | Err(_) => break,
            };

            // An error means nobody listens any more.
            if let Some(incoming) = self.sort(frame)
                && sender.send_blocking(incoming).is_err()
            {
                return;
            }
        }

        let _ = sender.send_blocking(Incoming::Closed);
    }

    /// Turns a frame into what the front end handles, answering at once a
    /// request this front end does not know.
    fn sort(&self, frame: Frame) -> Option<Incoming> {
        match (frame.id, frame.method) {
            (Some(id), None) => {
                let answer = match frame.error {
                    Some(error) => Err(error),
                    None => Ok(frame.result.unwrap_or(Json::Null)),
                };

                Some(Incoming::Answer { id, answer })
            }

            (None, Some(method)) if method == protocol::METHOD_EVENT => {
                let event = serde_json::from_value(frame.params.unwrap_or(Json::Null)).ok()?;

                Some(Incoming::Event(Box::new(event)))
            }

            (Some(id), Some(method)) if method == protocol::METHOD_ASK => {
                let params =
                    serde_json::from_value(frame.params.unwrap_or(Json::Null)).unwrap_or_default();

                Some(Incoming::Ask { id, params })
            }

            (Some(id), Some(method)) => {
                let _ = self.refuse(
                    id,
                    CODE_METHOD_NOT_FOUND,
                    &format!("{} does not know {method}", protocol::NAME),
                );

                None
            }

            // A notification nobody here knows, or a frame with neither an id
            // nor a method.
            _ => None,
        }
    }
}

/// Reads the next frame, skipping lines that are not JSON, as rumbo's own
/// client does. `None` is the end of the stream.
fn read_frame(reader: &mut impl BufRead) -> io::Result<Option<Frame>> {
    let mut line = Vec::new();

    loop {
        line.clear();

        if reader.read_until(b'\n', &mut line)? == 0 {
            return Ok(None);
        }

        if let Ok(frame) = serde_json::from_slice(&line) {
            return Ok(Some(frame));
        }
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};

    use serde_json::{Value as Json, json};

    use super::*;
    use crate::protocol::{RunParams, RunResult};

    /// A host on the other end of two pipes, driven by the test line by line.
    struct FakeHost {
        from_frontend: BufReader<io::PipeReader>,
        to_frontend: io::PipeWriter,
    }

    impl FakeHost {
        fn send(&mut self, frame: Json) {
            writeln!(self.to_frontend, "{frame}").unwrap();
        }

        fn receive(&mut self) -> Json {
            let mut line = String::new();

            self.from_frontend.read_line(&mut line).unwrap();

            serde_json::from_str(&line).unwrap()
        }
    }

    fn connect(
        initialize: Json,
    ) -> io::Result<(Connection, InitializeParams, Receiver<Incoming>, FakeHost)> {
        let (frontend_reads, host_writes) = io::pipe()?;
        let (host_reads, frontend_writes) = io::pipe()?;

        let mut host = FakeHost {
            from_frontend: BufReader::new(host_reads),
            to_frontend: host_writes,
        };

        host.send(initialize);

        let (connection, params, incoming) = Connection::open(frontend_reads, frontend_writes)?;

        Ok((connection, params, incoming, host))
    }

    /// Collects the events that arrive until the answer to request `id`.
    fn collect_until_answer(
        receiver: &Receiver<Incoming>,
        id: i64,
    ) -> (Vec<Event>, Result<Json, RpcError>) {
        let mut events = Vec::new();

        loop {
            match receiver.recv_blocking().expect("the connection ended") {
                Incoming::Answer {
                    id: answered,
                    answer,
                } if answered == id => {
                    return (events, answer);
                }
                Incoming::Event(event) => events.push(*event),
                Incoming::Closed => panic!("the connection closed before answer {id}"),
                _ => {}
            }
        }
    }

    fn initialize() -> Json {
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocol": 1, "dir": "/project", "session": "4cf0261c",
            "provider": "anthropic", "model": "claude-opus-5-5", "functions": []
        }})
    }

    #[test]
    fn answers_initialize_with_its_name_and_its_configuration() {
        let (_connection, params, _incoming, mut host) = connect(initialize()).unwrap();

        assert_eq!(params.dir, "/project");
        assert_eq!(params.session, "4cf0261c");
        assert_eq!(params.provider, "anthropic");
        assert_eq!(
            host.receive(),
            json!({"jsonrpc": "2.0", "id": 1, "result": {"protocol": 1, "name": "rumbo-gui", "config": {}}})
        );
    }

    #[test]
    fn refuses_another_protocol_version() {
        let mut frame = initialize();
        frame["params"]["protocol"] = json!(2);

        let error = connect(frame).err().expect("version 2 was accepted");

        assert!(error.to_string().contains("protocol 2"), "{error}");
    }

    #[test]
    fn delivers_a_runs_events_before_its_answer() {
        let (connection, _params, incoming, mut host) = connect(initialize()).unwrap();
        host.receive();

        let run = connection
            .request(
                protocol::METHOD_RUN,
                RunParams {
                    objective: "say hi".into(),
                    ..RunParams::default()
                },
            )
            .unwrap();

        let sent = host.receive();

        assert_eq!(sent["method"], "run");
        assert_eq!(sent["params"], json!({"objective": "say hi"}));
        assert_eq!(sent["id"], json!(run));

        host.send(json!({"jsonrpc": "2.0", "method": "event", "params": {"type": "text", "run": run, "text": "Hi"}}));
        host.send(json!("not a frame"));
        host.send(json!({"jsonrpc": "2.0", "method": "event", "params": {"type": "text", "run": run, "text": " there"}}));
        host.send(json!({"jsonrpc": "2.0", "id": run, "result": {"status": "done", "session": "4cf0261c"}}));

        let (events, answer) = collect_until_answer(&incoming, run);
        let result: RunResult = serde_json::from_value(answer.unwrap()).unwrap();
        let text: Vec<&str> = events.iter().map(|event| event.text.as_str()).collect();

        assert_eq!(text, ["Hi", " there"]);
        assert_eq!(result.status, "done");
    }

    #[test]
    fn passes_on_an_error_answer() {
        let (connection, _params, incoming, mut host) = connect(initialize()).unwrap();
        host.receive();

        let steer = connection
            .request(protocol::METHOD_STEER, json!({"run": 3, "text": "stop"}))
            .unwrap();
        host.receive();
        host.send(json!({"jsonrpc": "2.0", "id": steer, "error": {"code": -32602, "message": "no run 3 is in progress"}}));

        let (_events, answer) = collect_until_answer(&incoming, steer);

        assert_eq!(answer.unwrap_err().message, "no run 3 is in progress");
    }

    #[test]
    fn hands_over_questions_and_refuses_unknown_requests() {
        let (connection, _params, incoming, mut host) = connect(initialize()).unwrap();
        host.receive();

        host.send(json!({"jsonrpc": "2.0", "id": 8, "method": "paint", "params": {}}));

        assert_eq!(
            host.receive()["error"]["code"],
            json!(CODE_METHOD_NOT_FOUND)
        );

        host.send(json!({"jsonrpc": "2.0", "id": 9, "method": "ask", "params": {"invoke": 4, "text": "Code?", "secret": true}}));

        let Incoming::Ask { id, params } = incoming.recv_blocking().unwrap() else {
            panic!("the question did not arrive");
        };

        assert_eq!(
            (id, params.invoke, params.text.as_str(), params.secret),
            (9, 4, "Code?", true)
        );

        connection
            .refuse(id, protocol::CODE_INVALID_PARAMS, "no")
            .unwrap();

        assert_eq!(host.receive()["error"]["message"], "no");
    }

    #[test]
    fn closing_ends_the_stream_to_the_host() {
        let (connection, _params, _incoming, mut host) = connect(initialize()).unwrap();
        host.receive();

        connection.close();

        let mut line = String::new();

        assert_eq!(host.from_frontend.read_line(&mut line).unwrap(), 0);
        assert!(
            connection
                .notify(protocol::METHOD_CANCEL, json!({}))
                .is_err()
        );
    }

    #[test]
    fn says_when_the_host_closes() {
        let (_connection, _params, incoming, mut host) = connect(initialize()).unwrap();
        host.receive();

        drop(host);

        assert!(matches!(
            incoming.recv_blocking().unwrap(),
            Incoming::Closed
        ));
    }
}
