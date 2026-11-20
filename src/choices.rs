//! The model and the options a session uses, and the changes the picker asks
//! the host for. The session keeps them, as rumbo's terminal UI leaves them:
//! a run gets the session's model, and the option values the provider's
//! configuration gives, with those chosen in the session over them. `use`
//! changes the provider and the model, and `session-set` one option at a
//! time, as `name=value`.

use serde_json::{Map, Value as Json};

use crate::commands;
use crate::protocol::Parameter;

/// What a row of the picker changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A parameter of `use`: the provider or the model.
    Use,
    /// An option of the provider the session uses now.
    Option,
}

/// One row of the picker: what it opened with, and what it holds now.
#[derive(Debug, Clone, Copy)]
pub struct Row<'a> {
    pub role: Role,
    pub parameter: &'a Parameter,
    pub before: &'a str,
    pub after: &'a str,
}

impl Row<'_> {
    fn changed(&self) -> bool {
        self.before.trim() != self.after.trim()
    }
}

/// A change to ask of the host, in the order it is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Change the provider and the model, with these parameters of `use`.
    Use(Json),
    /// Change one option, as `name=value`. An empty value gives back what
    /// the session chose, so the provider's own applies.
    Option(String),
}

/// The value an option has in the session, in words: what the host says is
/// in force there, else the provider's default.
pub fn in_force(parameter: &Parameter, options: Option<&Json>) -> String {
    options
        .and_then(|options| options.get(&parameter.name))
        .or(parameter.default.as_ref())
        .map(words)
        .unwrap_or_default()
}

/// The changes the rows ask for. A new provider takes the options it offers
/// instead of the old one's, so the options are left as they are then.
pub fn changes(rows: &[Row]) -> Result<Vec<Change>, String> {
    let mut changes = Vec::new();
    let using: Vec<&Row> = rows.iter().filter(|row| row.role == Role::Use).collect();

    if using.iter().any(|row| row.changed()) {
        let mut params = Map::new();

        for row in &using {
            let text = row.after.trim();

            if text.is_empty() {
                if row.parameter.optional {
                    continue;
                }

                return Err(format!(
                    "{} is needed",
                    commands::capitalized(&row.parameter.name)
                ));
            }

            params.insert(
                row.parameter.name.clone(),
                commands::encode(row.parameter, text)?,
            );
        }

        changes.push(Change::Use(Json::Object(params)));
    }

    let new_provider = using
        .iter()
        .any(|row| row.parameter.name == "provider" && row.changed());

    if new_provider {
        return Ok(changes);
    }

    for row in rows
        .iter()
        .filter(|row| row.role == Role::Option && row.changed())
    {
        let text = row.after.trim();

        // What the provider accepts is the provider's to say, but a value of
        // the wrong type is caught here.
        if !text.is_empty() {
            commands::encode(row.parameter, text)?;
        }

        changes.push(Change::Option(format!("{}={text}", row.parameter.name)));
    }

    Ok(changes)
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

    fn parameter(name: &str, kind: &str) -> Parameter {
        Parameter {
            name: name.into(),
            kind: kind.into(),
            ..Parameter::default()
        }
    }

    fn row<'a>(role: Role, parameter: &'a Parameter, before: &'a str, after: &'a str) -> Row<'a> {
        Row {
            role,
            parameter,
            before,
            after,
        }
    }

    #[test]
    fn tells_the_value_in_force_and_else_the_default() {
        let mut effort = parameter("effort", "string");
        effort.default = Some(json!("medium"));

        let stored = json!({"effort": "high", "depth": 2});

        assert_eq!(in_force(&effort, None), "medium");
        assert_eq!(in_force(&effort, Some(&stored)), "high");
        assert_eq!(
            in_force(&parameter("depth", TYPE_NUMBER), Some(&stored)),
            "2"
        );
        assert_eq!(in_force(&parameter("web", "boolean"), None), "");
    }

    #[test]
    fn asks_for_nothing_when_nothing_changed() {
        let provider = parameter("provider", "string");
        let effort = parameter("effort", "string");

        let rows = [
            row(Role::Use, &provider, "toy", " toy "),
            row(Role::Option, &effort, "high", "high"),
        ];

        assert_eq!(changes(&rows), Ok(vec![]));
    }

    #[test]
    fn changes_the_model_and_the_options_of_the_same_provider() {
        let provider = parameter("provider", "string");
        let model = parameter("model", "string");
        let effort = parameter("effort", "string");
        let depth = parameter("depth", TYPE_NUMBER);

        let rows = [
            row(Role::Use, &provider, "toy", "toy"),
            row(Role::Use, &model, "small", "big"),
            row(Role::Option, &effort, "high", ""),
            row(Role::Option, &depth, "", "3"),
        ];

        assert_eq!(
            changes(&rows),
            Ok(vec![
                Change::Use(json!({"provider": "toy", "model": "big"})),
                Change::Option("effort=".into()),
                Change::Option("depth=3".into()),
            ])
        );
    }

    #[test]
    fn leaves_the_options_to_a_new_provider() {
        let provider = parameter("provider", "string");
        let model = parameter("model", "string");
        let effort = parameter("effort", "string");

        let rows = [
            row(Role::Use, &provider, "toy", "openai"),
            row(Role::Use, &model, "small", "gpt"),
            row(Role::Option, &effort, "high", "low"),
        ];

        assert_eq!(
            changes(&rows),
            Ok(vec![Change::Use(
                json!({"provider": "openai", "model": "gpt"})
            )])
        );
    }

    #[test]
    fn refuses_what_cannot_be_sent() {
        let provider = parameter("provider", "string");
        let model = parameter("model", "string");
        let depth = parameter("depth", TYPE_NUMBER);

        let missing = [
            row(Role::Use, &provider, "toy", "openai"),
            row(Role::Use, &model, "small", ""),
        ];
        let mistyped = [row(Role::Option, &depth, "", "deep")];

        assert_eq!(changes(&missing), Err("Model is needed".into()));
        assert_eq!(changes(&mistyped), Err("depth is a number".into()));
    }
}
