//! The functions of the host and of the provider as commands: what a person
//! types for their parameters, and their answers in words. rumbo's terminal
//! UI reads and shows them the same way.

use serde_json::{Map, Value as Json};

use crate::conversation::Source;
use crate::protocol::{Function, HostResult, Parameter, TYPE_BOOLEAN, TYPE_NUMBER, Value};

/// A function someone offers, and whose it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub source: Source,
    pub function: Function,
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
}
