//! The functions of the host and of the provider as commands: what a person
//! types for their parameters, the values a parameter offers, and their
//! answers in words. rumbo's terminal UI reads and shows them the same way.

use serde_json::{Map, Value as Json};

use crate::conversation::Source;
use crate::protocol::{
    self, Function, HostResult, InvokeParams, Parameter, TYPE_BOOLEAN, TYPE_NUMBER, Value,
    ValuesResult,
};

/// A function someone offers, and whose it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub source: Source,
    pub function: Function,
}

impl Command {
    /// The name shown in the palette, qualified for extension commands as
    /// in the terminal UI. The function's own name still goes to the host.
    pub fn name(&self) -> String {
        match &self.source {
            Source::Extension(name) => format!("{name}-{}", self.function.name),
            _ => self.function.name.clone(),
        }
    }
}

/// Where the values a parameter takes come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lister {
    /// One of these values.
    Fixed(Vec<Value>),
    /// One of the values this function lists. It belongs to whoever
    /// described the parameter: the host for its own functions, the
    /// provider for its options and functions.
    Function(String),
    /// Whatever is typed, as the parameter's type reads it.
    Typed,
}

/// What a parameter, an option, or the model accepts.
pub fn lister(parameter: &Parameter) -> Lister {
    if !parameter.values.is_empty() {
        return Lister::Function(parameter.values.clone());
    }

    let values: Vec<&str> = if !parameter.choices.is_empty() {
        parameter.choices.iter().map(String::as_str).collect()
    } else if parameter.kind == TYPE_BOOLEAN {
        vec!["true", "false"]
    } else {
        return Lister::Typed;
    };

    Lister::Fixed(
        values
            .into_iter()
            .map(|value| Value {
                value: value.to_string(),
                ..Value::default()
            })
            .collect(),
    )
}

/// What a function that lists values is given: what was typed for the
/// parameters before the one it lists for, those it takes, so the models
/// listed after the `provider` of `use` are that provider's. None when it
/// takes none of them, or is not described.
pub fn passed(lister: Option<&Function>, given: &[(&str, &str)]) -> Option<Json> {
    let mut params = Map::new();

    for parameter in &lister?.params {
        let typed = given
            .iter()
            .find(|(name, _)| *name == parameter.name)
            .map(|(_, text)| text.trim())
            .unwrap_or_default();

        if !typed.is_empty() {
            params.insert(parameter.name.clone(), Json::String(typed.to_string()));
        }
    }

    (!params.is_empty()).then_some(Json::Object(params))
}

/// The call of a function of `source`, by name.
pub fn invoke(source: &Source, name: &str, params: Option<Json>) -> InvokeParams {
    let (target, extension) = match source {
        Source::Host => (protocol::TARGET_HOST, String::new()),
        Source::Provider => ("", String::new()),
        Source::Extension(name) => (protocol::TARGET_EXTENSION, name.clone()),
    };

    InvokeParams {
        name: name.to_string(),
        params,
        target,
        extension,
    }
}

/// The values a function listed, and what it said with them: a function of
/// the host answers as all of them do, and a provider's or an extension's
/// with its values.
pub fn listed(source: &Source, answer: Json) -> (Vec<Value>, String) {
    match source {
        Source::Host => {
            let result: HostResult = serde_json::from_value(answer).unwrap_or_default();

            (result.values, result.text)
        }

        Source::Provider | Source::Extension(_) => {
            let result: ValuesResult = serde_json::from_value(answer).unwrap_or_default();

            (result.values, String::new())
        }
    }
}

/// The functions offered as commands: all but those there to list the
/// values a parameter takes, which its form lists in their place.
pub fn offered(functions: &[Function]) -> impl Iterator<Item = &Function> {
    functions.iter().filter(|function| !function.lists)
}

/// The functions of the host offered as commands, by what they act on, in
/// the order the host lists them, each group named for a person.
pub fn by_group(functions: &[Function]) -> Vec<(String, Vec<Function>)> {
    let mut groups: Vec<(String, Vec<Function>)> = Vec::new();

    for function in offered(functions) {
        let title = group_title(&function.group);

        match groups.iter_mut().find(|(known, _)| *known == title) {
            Some((_, listed)) => listed.push(function.clone()),
            None => groups.push((title, vec![function.clone()])),
        }
    }

    groups
}

/// A group's name for a person. One this window does not know is shown as
/// the host names it, and a function without one is rumbo's.
fn group_title(group: &str) -> String {
    match group {
        protocol::GROUP_SYSTEM => "System".into(),
        protocol::GROUP_SESSION => "Session".into(),
        protocol::GROUP_PROVIDER => "Providers".into(),
        protocol::GROUP_EXTENSION => "Extensions".into(),
        "" => "rumbo".into(),
        other => capitalized(other),
    }
}

/// A name with its first letter in capitals, as a label or a button says it.
pub fn capitalized(name: &str) -> String {
    let mut letters = name.chars();

    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

/// The parameters of a call, from what was typed for each, in the order the
/// function lists them. A parameter left empty is left out when it is
/// optional. None means the function takes none.
pub fn arguments(function: &Function, typed: &[String]) -> Result<Option<Json>, String> {
    if function.params.is_empty() {
        return Ok(None);
    }

    let mut params = Map::new();

    for (index, parameter) in function.params.iter().enumerate() {
        let text = typed.get(index).map(|text| text.trim()).unwrap_or_default();

        if text.is_empty() {
            if parameter.optional {
                continue;
            }

            return Err(format!("{} is needed", parameter.name));
        }

        params.insert(parameter.name.clone(), encode(parameter, text)?);
    }

    Ok(Some(Json::Object(params)))
}

/// A typed value as the parameter's type asks for it.
pub fn encode(parameter: &Parameter, text: &str) -> Result<Json, String> {
    match parameter.kind.as_str() {
        TYPE_BOOLEAN => text
            .parse::<bool>()
            .map(Json::Bool)
            .map_err(|_| format!("{} is true or false", parameter.name)),

        TYPE_NUMBER => text
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Json::Number)
            .ok_or_else(|| format!("{} is a number", parameter.name)),

        _ if !parameter.choices.is_empty()
            && !parameter.choices.iter().any(|choice| choice == text) =>
        {
            Err(format!(
                "{} is one of: {}",
                parameter.name,
                parameter.choices.join(", ")
            ))
        }

        _ => Ok(Json::String(text.to_string())),
    }
}

/// The answer of a function of the host, in words.
pub fn host_answer(result: &HostResult) -> String {
    format!("{}\n{}", result.text, list(&result.values))
        .trim()
        .to_string()
}

/// The answer of a function of the provider, in words: the values it lists,
/// or its JSON as it came.
pub fn provider_answer(answer: &Json) -> String {
    let listed: Option<Vec<Value>> = answer
        .get("values")
        .and_then(|values| serde_json::from_value(values.clone()).ok());

    match listed {
        Some(values) if !values.is_empty() => list(&values),
        _ => serde_json::to_string_pretty(answer).unwrap_or_default(),
    }
}

/// One line per value: the value, then what tells it apart.
fn list(values: &[Value]) -> String {
    let lines: Vec<String> = values
        .iter()
        .map(|value| {
            [value.value.as_str(), &value.label, &value.description]
                .iter()
                .filter(|part| !part.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join("  ")
        })
        .collect();

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parameter(name: &str, kind: &str, optional: bool) -> Parameter {
        Parameter {
            name: name.into(),
            kind: kind.into(),
            optional,
            ..Parameter::default()
        }
    }

    fn typed(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|text| text.to_string()).collect()
    }

    #[test]
    fn encodes_each_parameter_by_its_type() {
        let function = Function {
            name: "setting".into(),
            params: vec![
                parameter("key", "string", false),
                parameter("depth", TYPE_NUMBER, false),
                parameter("on", TYPE_BOOLEAN, false),
                parameter("scope", "", true),
            ],
            ..Function::default()
        };

        assert_eq!(
            arguments(&function, &typed(&["model", "2", "true", " "])),
            Ok(Some(json!({"key": "model", "depth": 2.0, "on": true})))
        );
        assert_eq!(
            arguments(&function, &typed(&["model", "two", "true", ""])),
            Err("depth is a number".into())
        );
        assert_eq!(
            arguments(&function, &typed(&["", "2", "true"])),
            Err("key is needed".into())
        );
    }

    #[test]
    fn keeps_a_choice_to_what_it_accepts() {
        let mut effort = parameter("effort", "string", false);
        effort.choices = vec!["low".into(), "high".into()];

        let function = Function {
            name: "set".into(),
            params: vec![effort],
            ..Function::default()
        };

        assert_eq!(
            arguments(&function, &typed(&["high"])),
            Ok(Some(json!({"effort": "high"})))
        );
        assert_eq!(
            arguments(&function, &typed(&["max"])),
            Err("effort is one of: low, high".into())
        );
        assert_eq!(arguments(&Function::default(), &[]), Ok(None));
    }

    #[test]
    fn says_answers_in_words() {
        let listed = HostResult {
            text: String::new(),
            values: vec![Value {
                value: "4cf0261c".into(),
                label: "fix the test".into(),
                description: "last used today".into(),
            }],
            ..HostResult::default()
        };

        assert_eq!(
            host_answer(&listed),
            "4cf0261c  fix the test  last used today"
        );
        assert_eq!(
            provider_answer(&json!({"values": [{"value": "m1", "label": "Model one"}]})),
            "m1  Model one"
        );
        assert_eq!(
            provider_answer(&json!({"ok": true})),
            "{\n  \"ok\": true\n}"
        );
    }

    #[test]
    fn says_where_the_values_of_a_parameter_come_from() {
        let mut effort = parameter("effort", "string", false);
        effort.choices = vec!["low".into(), "high".into()];

        let mut session = parameter("session", "string", false);
        session.values = "sessions".into();

        assert_eq!(lister(&session), Lister::Function("sessions".into()));
        assert_eq!(
            lister(&parameter("depth", TYPE_NUMBER, false)),
            Lister::Typed
        );
        assert!(
            matches!(lister(&parameter("on", TYPE_BOOLEAN, false)), Lister::Fixed(values) if values.len() == 2)
        );
        assert!(matches!(lister(&effort), Lister::Fixed(values) if values[1].value == "high"));
    }

    #[test]
    fn gives_a_lister_what_was_typed_before_for_what_it_takes() {
        let models = Function {
            name: "provider-models".into(),
            params: vec![parameter("provider", "string", true)],
            ..Function::default()
        };

        assert_eq!(
            passed(Some(&models), &[("provider", " openai "), ("other", "x")]),
            Some(json!({"provider": "openai"}))
        );
        assert_eq!(passed(Some(&models), &[("provider", "")]), None);
        assert_eq!(passed(Some(&models), &[]), None);
        assert_eq!(passed(None, &[("provider", "openai")]), None);
    }

    #[test]
    fn reads_what_a_lister_answered_by_whose_it_is() {
        let host = json!({"text": "the models toy offers:", "values": [{"value": "big", "label": "Big"}], "state": {}});
        let provider = json!({"values": [{"value": "small"}]});

        let (values, text) = listed(&Source::Host, host);

        assert_eq!(values[0].label, "Big");
        assert_eq!(text, "the models toy offers:");

        let (values, text) = listed(&Source::Provider, provider);

        assert_eq!(values[0].value, "small");
        assert!(text.is_empty());
        assert!(listed(&Source::Provider, json!("odd")).0.is_empty());
    }

    #[test]
    fn groups_the_functions_of_the_host_by_what_they_act_on() {
        let function = |name: &str, group: &str| Function {
            name: name.into(),
            group: group.into(),
            ..Function::default()
        };

        let listing = Function {
            lists: true,
            ..function("system-values", "system")
        };

        let groups = by_group(&[
            function("config", "system"),
            function("status", "session"),
            listing,
            function("quit", "system"),
            function("providers", "provider"),
            function("plugins", "plugin"),
            function("older", ""),
        ]);

        let titles: Vec<&str> = groups.iter().map(|(title, _)| title.as_str()).collect();
        let system: Vec<&str> = groups[0]
            .1
            .iter()
            .map(|function| function.name.as_str())
            .collect();

        assert_eq!(
            titles,
            ["System", "Session", "Providers", "Plugin", "rumbo"]
        );
        // One there to list a parameter's values is no command.
        assert_eq!(system, ["config", "quit"]);
    }

    #[test]
    fn capitalizes_a_name() {
        assert_eq!(capitalized("delete"), "Delete");
        assert_eq!(capitalized(""), "");
    }
}
