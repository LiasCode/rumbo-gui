//! The messages of the Agent Frontend Protocol, version 2, as rumbo specifies
//! them in `docs/frontend-protocol.md` and its Go package `protocol`. In it
//! a host is one session, so nothing here names a session to a host.
//!
//! Only what this front end sends or reads is here. Fields it does not know
//! are ignored when reading, as the protocol asks, so a newer host still
//! works.

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// The protocol version this front end speaks.
pub const VERSION: i64 = 2;

/// The name this front end gives the host.
pub const NAME: &str = "rumbo-gui";

pub const METHOD_INITIALIZE: &str = "initialize";
pub const METHOD_RUN: &str = "run";
pub const METHOD_STEER: &str = "steer";
pub const METHOD_CANCEL: &str = "cancel";
pub const METHOD_EVENT: &str = "event";
pub const METHOD_ASK: &str = "ask";
pub const METHOD_DESCRIBE: &str = "describe";
pub const METHOD_INVOKE: &str = "invoke";
pub const METHOD_HISTORY: &str = "history";
pub const METHOD_RECALL: &str = "recall";
pub const METHOD_PROCESSES: &str = "processes";

/// The `target` of an invoke that calls a function of the host, and of one
/// that calls a function of an extension, which `extension` then names.
/// Without a target the function is the provider's.
pub const TARGET_HOST: &str = "host";
pub const TARGET_EXTENSION: &str = "extension";

/// What a function of the host acts on. A front end lists them apart, and
/// takes a group it does not know for one more.
pub const GROUP_SYSTEM: &str = "system";
pub const GROUP_SESSION: &str = "session";
pub const GROUP_PROVIDER: &str = "provider";
/// A function of this group may change what the extensions offer, so
/// `describe` is asked again after one.
pub const GROUP_EXTENSION: &str = "extension";

/// The functions of the host the window offers in a place of its own: the
/// sidebar and the session's menu rename a session, and the menu and the
/// person's messages branch and compact it; the details panel shows its
/// status and skills;
/// the picker changes the model and the options a session keeps; and the
/// settings show everything that is set, with `config`, and change the
/// system settings and the providers. Each is offered only when
/// `initialize` lists it, and called with the parameters it describes there.
pub const FUNCTION_RENAME: &str = "rename";
pub const FUNCTION_BRANCH: &str = "branch";
pub const FUNCTION_COMPACT: &str = "compact";
pub const FUNCTION_STATUS: &str = "status";
pub const FUNCTION_SKILLS: &str = "skills";
pub const FUNCTION_USE: &str = "use";
pub const FUNCTION_SESSION_SET: &str = "session-set";
pub const FUNCTION_CONFIG: &str = "config";
pub const FUNCTION_SYSTEM_SET: &str = "system-set";
pub const FUNCTION_PROVIDERS: &str = "providers";
pub const FUNCTION_PROVIDER_SHOW: &str = "provider-show";
pub const FUNCTION_PROVIDER_SET: &str = "provider-set";
pub const FUNCTION_PROVIDER_ADD: &str = "provider-add";
pub const FUNCTION_PROVIDER_RM: &str = "provider-rm";

pub const EVENT_CONTEXT: &str = "context";
pub const EVENT_TEXT: &str = "text";
pub const EVENT_TOOL_STARTED: &str = "tool_started";
pub const EVENT_TOOL_FINISHED: &str = "tool_finished";
pub const EVENT_TOOL_PROGRESS: &str = "tool_progress";
pub const EVENT_TURN: &str = "turn";
pub const EVENT_LOG: &str = "log";
pub const EVENT_OBJECTIVE: &str = "objective";
pub const EVENT_STEER: &str = "steer";
pub const EVENT_ENDED: &str = "ended";

pub const RUN_DONE: &str = "done";
pub const RUN_CANCELLED: &str = "cancelled";
pub const RUN_FAILED: &str = "failed";

/// The `source` of a log event that is the provider's own output, and of
/// one that is an extension's, whose text starts with the extension's name.
/// Any other is the host's.
pub const SOURCE_PROVIDER: &str = "provider";
pub const SOURCE_EXTENSION: &str = "extension";

/// JSON-RPC error codes this front end answers with.
pub const CODE_METHOD_NOT_FOUND: i64 = -32601;
pub const CODE_INVALID_PARAMS: i64 = -32602;
pub const CODE_INTERNAL: i64 = -32603;

/// Types of the options and parameters a description lists.
pub const TYPE_NUMBER: &str = "number";
pub const TYPE_BOOLEAN: &str = "boolean";

/// What the host says it is attached to when it speaks first.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InitializeParams {
    pub protocol: i64,
    pub dir: String,
    pub session: String,
    /// What the session is called, once it has a title.
    pub title: String,
    pub provider: String,
    pub model: String,
    pub objective: String,
    pub resume: bool,
    /// What the user chose for this front end in rumbo's configuration.
    pub config: Option<Json>,
    /// What the host itself can be asked, whatever the provider.
    pub functions: Vec<Function>,
}

#[derive(Debug, Serialize)]
pub struct InitializeResult {
    pub protocol: i64,
    pub name: &'static str,
    /// The start configuration in force, with a default for every field.
    /// This front end has no fields of its own yet.
    pub config: Json,
}

/// Starts a run on `objective`, and the images given with it, or continues
/// the session's last one. The run uses the model and the options its
/// session keeps.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RunParams {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub objective: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    #[serde(skip_serializing_if = "is_false")]
    pub resume: bool,
}

/// An image the person gave, as the protocol carries one: its media type,
/// such as `image/png`, and the encoded file in base64.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Image {
    pub media_type: String,
    pub data: String,
}

/// How a run ended.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RunResult {
    pub status: String,
    pub error: String,
    pub session: String,
}

/// What the person tells a run while it works, and the images given with it.
#[derive(Debug, Clone, Serialize)]
pub struct SteerParams {
    pub run: i64,
    pub text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
}

/// Names the run to cancel by the id of its run request.
#[derive(Debug, Clone, Serialize)]
pub struct CancelParams {
    #[serde(skip_serializing_if = "is_zero")]
    pub run: i64,
    /// A function in progress to give up, by the id of its invoke request.
    #[serde(skip_serializing_if = "is_zero")]
    pub invoke: i64,
}

/// A question the host asks the person: for a function of a provider or
/// of an extension, named by its invoke request, or for a tool the model
/// called, which names its run, the run's session, and the tool instead.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AskParams {
    /// The id of the invoke request of the function that asks.
    pub invoke: i64,
    /// The id of the run request of the run whose tool asks, its session,
    /// and the tool.
    pub run: i64,
    pub session: String,
    pub name: String,
    pub text: String,
    /// The answer is not shown while it is typed, nor kept.
    pub secret: bool,
}

/// Tokens of one model response.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
}

/// Something that happened in a run. `kind` selects the other fields, as
/// the protocol's table of event types says.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Event {
    #[serde(rename = "type")]
    pub kind: String,
    pub run: i64,
    pub session: String,
    pub invoke: i64,
    pub text: String,
    pub sections: Vec<String>,
    pub name: String,
    pub args: Option<Json>,
    pub result: String,
    pub failed: bool,
    pub source: String,
    pub context: i64,
    pub window: i64,
    pub usage: Option<Usage>,
    pub status: String,
    /// The images given with an objective or a steer, in a history.
    pub images: Vec<Image>,
}

/// A function of the host or of the provider, which a front end offers
/// without knowing it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub params: Vec<Parameter>,
    /// Set when the function cannot be undone: what to ask the person
    /// before calling it.
    pub confirm: String,
    /// What a function of the host acts on; empty for a provider's.
    pub group: String,
    /// Set when the function is there to list the values a parameter of
    /// another function takes: it is asked for them, and not offered as a
    /// command.
    pub lists: bool,
}

/// One value a function takes: a small subset of JSON Schema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub description: String,
    #[serde(rename = "enum")]
    pub choices: Vec<String>,
    /// The function that lists the accepted values: the host's for a
    /// parameter of the host, else the provider's.
    pub values: String,
    /// The value in force when none is chosen, for display.
    pub default: Option<Json>,
    pub optional: bool,
}

/// Calls a function of the provider, of the host with `target`, or of the
/// extension `extension` names.
#[derive(Debug, Clone, Serialize)]
pub struct InvokeParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Json>,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub target: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub extension: String,
}

/// The answer of a function of the host.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HostResult {
    pub text: String,
    pub values: Vec<Value>,
    pub state: HostState,
}

/// What the front end is attached to after a function of the host.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct HostState {
    pub dir: String,
    pub session: String,
    /// What the session is called, which a function may have changed.
    pub title: String,
    pub provider: String,
    pub model: String,
}

/// One of the values a function lists.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Value {
    pub value: String,
    pub label: String,
    pub description: String,
}

/// The answer of a function that lists the values an option accepts.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ValuesResult {
    pub values: Vec<Value>,
}

/// What the provider of the current session offers.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DescribeResult {
    pub provider: String,
    /// The model and option values a run uses unless it says otherwise.
    pub model: String,
    pub options: Option<Json>,
    pub offers: Description,
    /// The extensions that offer functions, by name; none when they did
    /// not all start, which a log event says.
    pub extensions: Vec<ExtensionOffer>,
}

/// What an extension offers the person: its functions, described as a
/// provider's are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ExtensionOffer {
    pub name: String,
    pub functions: Vec<Function>,
}

/// What a provider says can be chosen and asked of it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Description {
    /// How a model is chosen; its name is ignored.
    pub model: Option<Parameter>,
    pub options: Vec<Parameter>,
    pub functions: Vec<Function>,
}

/// A session told in events, oldest first.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HistoryResult {
    pub session: String,
    pub events: Vec<Event>,
}

/// What the person asked in the project's sessions, most recent first and
/// each once.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RecallResult {
    pub asked: Vec<String>,
}

/// One process of a host and what it uses of the machine: `role` says what
/// it is, and `name` which provider or extension; `parent` is the process
/// that started it; `memory` is what it holds now, in bytes, and `cpu` the
/// processor time it has used, in seconds.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ProcessUsage {
    pub pid: i64,
    pub parent: i64,
    pub role: String,
    pub name: String,
    pub command: String,
    pub memory: i64,
    pub cpu: f64,
}

/// The processes of a host, itself first and each of the others after the
/// one that started it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProcessesResult {
    pub processes: Vec<ProcessUsage>,
}

/// A JSON-RPC error object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RpcError {}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_events_with_fields_it_does_not_know() {
        let line = r#"{"type":"turn","run":7,"session":"4cf0261c","usage":{"input":10,"output":5,"cache_read":90},"context":105,"window":200000,"later":true}"#;
        let event: Event = serde_json::from_str(line).unwrap();

        assert_eq!(event.kind, EVENT_TURN);
        assert_eq!(event.run, 7);
        assert_eq!(event.session, "4cf0261c");
        assert_eq!(event.context, 105);
        assert_eq!(event.window, 200_000);
        assert_eq!(
            event.usage,
            Some(Usage {
                input: 10,
                output: 5,
                cache_read: 90,
                cache_write: 0
            })
        );
    }

    #[test]
    fn reads_functions_as_the_host_describes_them() {
        let line = r#"{"protocol":2,"dir":"/p","provider":"anthropic","model":"m","functions":[
            {"name":"delete","description":"Delete a session","params":[{"name":"session","type":"string","values":"sessions"}],"confirm":"Delete it?","group":"session"},
            {"name":"status","params":[{"name":"session","optional":true}]},
            {"name":"session-values","group":"session","lists":true}]}"#;
        let params: InitializeParams = serde_json::from_str(line).unwrap();

        assert_eq!(params.functions.len(), 3);
        assert!(!params.functions[0].lists);
        assert!(params.functions[2].lists);
        assert_eq!(params.functions[0].confirm, "Delete it?");
        assert_eq!(params.functions[0].group, GROUP_SESSION);
        assert_eq!(params.functions[0].params[0].kind, "string");
        assert_eq!(params.functions[0].params[0].values, "sessions");
        assert!(params.functions[1].params[0].optional);
        assert!(params.functions[1].group.is_empty());
    }

    #[test]
    fn calls_the_host_only_when_it_says_so() {
        let provider = InvokeParams {
            name: "models".into(),
            params: None,
            target: "",
            extension: String::new(),
        };
        let extension = InvokeParams {
            name: "forget".into(),
            params: None,
            target: TARGET_EXTENSION,
            extension: "notes".into(),
        };
        let host = InvokeParams {
            name: "open".into(),
            params: Some(serde_json::json!({"session": "x"})),
            target: TARGET_HOST,
            extension: String::new(),
        };

        assert_eq!(
            serde_json::to_string(&extension).unwrap(),
            r#"{"name":"forget","target":"extension","extension":"notes"}"#
        );
        assert_eq!(
            serde_json::to_string(&provider).unwrap(),
            r#"{"name":"models"}"#
        );
        assert_eq!(
            serde_json::to_string(&host).unwrap(),
            r#"{"name":"open","params":{"session":"x"},"target":"host"}"#
        );
    }

    #[test]
    fn leaves_out_what_a_run_does_not_say() {
        let objective = RunParams {
            objective: "add a flag".into(),
            ..RunParams::default()
        };
        let resume = RunParams {
            resume: true,
            ..RunParams::default()
        };
        let cancel = CancelParams { run: 0, invoke: 4 };

        assert_eq!(serde_json::to_string(&cancel).unwrap(), r#"{"invoke":4}"#);

        assert_eq!(
            serde_json::to_string(&objective).unwrap(),
            r#"{"objective":"add a flag"}"#
        );
        assert_eq!(
            serde_json::to_string(&resume).unwrap(),
            r#"{"resume":true}"#
        );
    }
}
