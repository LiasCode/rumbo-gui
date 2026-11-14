//! What the person chose for the runs started from this window: a model and
//! option values over what the session uses, as rumbo's terminal UI keeps
//! them. They go with every run, and are forgotten when the provider
//! changes, since they were the old provider's.

use serde_json::{Map, Value as Json};

use crate::commands;
use crate::protocol::{Parameter, RunParams, TYPE_BOOLEAN, Value};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Choices {
    /// Empty keeps the session's model.
    pub model: String,
    pub options: Map<String, Json>,
}

impl Choices {
    pub fn is_empty(&self) -> bool {
        self.model.is_empty() && self.options.is_empty()
    }

    /// Adds the choices to a run's parameters.
    pub fn apply(&self, params: &mut RunParams) {
        params.model = self.model.clone();

        if !self.options.is_empty() {
            params.options = Some(Json::Object(self.options.clone()));
        }
    }
}

/// What a choice can be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accepts {
    /// One of these values.
    Values(Vec<Value>),
    /// One of the values this function of the provider lists.
    Listed(String),
    /// Whatever is typed, as the option's type reads it.
    Typed,
}

/// What an option, or the model, accepts.
pub fn accepts(parameter: &Parameter) -> Accepts {
    if !parameter.values.is_empty() {
        return Accepts::Listed(parameter.values.clone());
    }

    let values: Vec<&str> = if !parameter.choices.is_empty() {
        parameter.choices.iter().map(String::as_str).collect()
    } else if parameter.kind == TYPE_BOOLEAN {
        vec!["true", "false"]
    } else {
        return Accepts::Typed;
    };

    Accepts::Values(
        values
            .into_iter()
            .map(|value| Value {
                value: value.to_string(),
                ..Value::default()
            })
            .collect(),
    )
}

/// The value an option has for the next run, in words: what the window
/// chose, else what the session stores, else the provider's default.
pub fn in_force(parameter: &Parameter, stored: Option<&Json>, chosen: &Choices) -> Option<String> {
    let stored = stored.and_then(|stored| stored.get(&parameter.name));

    chosen
        .options
        .get(&parameter.name)
        .or(stored)
        .or(parameter.default.as_ref())
        .map(words)
}

/// What a value chosen for an option reads as, in the picker.
pub fn chosen_text(parameter: &Parameter, chosen: &Choices) -> String {
    chosen
        .options
        .get(&parameter.name)
        .map(words)
        .unwrap_or_default()
}

/// The choices for what the picker holds: the model, and the text for each
/// option in order. Empty text keeps what the session uses.
pub fn choose(model: &str, options: &[Parameter], picked: &[String]) -> Result<Choices, String> {
    let mut chosen = Choices {
        model: model.trim().to_string(),
        options: Map::new(),
    };

    for (parameter, text) in options.iter().zip(picked) {
        let text = text.trim();

        if !text.is_empty() {
            chosen
                .options
                .insert(parameter.name.clone(), commands::encode(parameter, text)?);
        }
    }

    Ok(chosen)
}

fn words(value: &Json) -> String {
    match value {
        Json::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::protocol::TYPE_NUMBER;

    fn option(name: &str, kind: &str) -> Parameter {
        Parameter {
            name: name.into(),
            kind: kind.into(),
            ..Parameter::default()
        }
    }

    #[test]
    fn says_what_each_option_accepts() {
        let mut effort = option("effort", "string");
        effort.choices = vec!["low".into(), "high".into()];

        let mut model = option("", "string");
        model.values = "models".into();

        assert_eq!(accepts(&model), Accepts::Listed("models".into()));
        assert_eq!(accepts(&option("temperature", TYPE_NUMBER)), Accepts::Typed);
        assert!(
            matches!(accepts(&option("verbose", TYPE_BOOLEAN)), Accepts::Values(values) if values.len() == 2)
        );
        assert!(matches!(accepts(&effort), Accepts::Values(values) if values[1].value == "high"));
    }

    #[test]
    fn tells_the_value_in_force_by_who_set_it() {
        let mut effort = option("effort", "string");
        effort.default = Some(json!("medium"));

        let stored = json!({"effort": "high"});
        let mut chosen = Choices::default();

        assert_eq!(in_force(&effort, None, &chosen).as_deref(), Some("medium"));
        assert_eq!(
            in_force(&effort, Some(&stored), &chosen).as_deref(),
            Some("high")
        );

        chosen.options.insert("effort".into(), json!("low"));

        assert_eq!(
            in_force(&effort, Some(&stored), &chosen).as_deref(),
            Some("low")
        );
        assert_eq!(in_force(&option("depth", TYPE_NUMBER), None, &chosen), None);
    }

    #[test]
    fn chooses_from_what_was_picked() {
        let options = [
            option("effort", "string"),
            option("temperature", TYPE_NUMBER),
        ];
        let picked = ["high".to_string(), " ".to_string()];

        let chosen = choose(" m2 ", &options, &picked).unwrap();
        let mut params = RunParams::default();

        chosen.apply(&mut params);

        assert_eq!(params.model, "m2");
        assert_eq!(params.options, Some(json!({"effort": "high"})));
        assert_eq!(chosen_text(&options[0], &chosen), "high");
        assert!(choose("", &options, &["x".into(), "warm".into()]).is_err());
        assert!(
            choose("", &options, &[String::new(), String::new()])
                .unwrap()
                .is_empty()
        );
    }
}
