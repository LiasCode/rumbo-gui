//! A form for the parameters of a function, or for what the picker changes,
//! built from their descriptions alone. A parameter whose values a function
//! lists offers them in a list, asked for with what was given for the
//! parameters before it, and asked again when that changes: the models
//! listed after a provider are that provider's. Such a list need not hold
//! every value the parameter takes, as a provider can take a model it does
//! not list, or a setting a value of the person's own; so the value can be
//! typed instead, starting from the one chosen.
//!
//! The form says which lists it needs; the workspace asks for them, since
//! everything the host answers arrives there, and hands the answers back.

use gpui_kit::component::{
    ActiveTheme as _, IndexPath, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState},
    spinner::Spinner,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, Hsla, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _,
};
use serde_json::Value as Json;

use crate::commands::{self, Lister};
use crate::conversation::Source;
use crate::protocol::{Function, Parameter, Value};

/// What a field of the form is for.
#[derive(Debug, Clone)]
pub(super) struct Field {
    pub(super) parameter: Parameter,
    /// Whose functions list its values: the host's for a parameter of the
    /// host, the provider's for one of its options or functions.
    pub(super) source: Source,
    /// What the field holds when the form opens.
    pub(super) initial: String,
    /// What the empty choice of a list says, when it has one.
    pub(super) empty: Option<String>,
    /// A heading the field starts a part of the form with.
    pub(super) heading: Option<String>,
}

/// A list the form needs, which the workspace asks for.
pub(super) struct Wanted {
    pub(super) field: usize,
    pub(super) generation: u64,
    pub(super) function: String,
    pub(super) source: Source,
    pub(super) params: Option<Json>,
}

/// Something in the form changed, so a list may be needed again.
pub(super) struct Changed;

pub(super) struct Form {
    rows: Vec<Row>,
    host_functions: Vec<Function>,
    provider_functions: Vec<Function>,
    /// The first field takes the keyboard once its list arrives.
    focus_when_listed: bool,
}

struct Row {
    field: Field,
    lister: Lister,
    control: Control,
    /// What the list was last asked with, and the number of that request,
    /// so an answer to an earlier one is left aside.
    asked: Option<Option<Json>>,
    generation: u64,
    /// What came with the list, or why there is none.
    note: Option<Note>,
    /// The person chose to type the value rather than choose it, so the
    /// list is not asked for again.
    typed: bool,
    _watching: Option<Subscription>,
}

enum Note {
    Said(String),
    Problem(String),
}

enum Control {
    /// A list is on its way; the value is what the field held before.
    Listing(String),
    List(Entity<SelectState<SearchableVec<Choice>>>),
    Text(Entity<InputState>),
}

/// One value in a list, with what tells it apart. An empty value is the
/// list's empty choice.
#[derive(Clone)]
struct Choice {
    value: SharedString,
    title: SharedString,
    detail: SharedString,
    /// The color of the detail, which the field shows without a theme.
    muted: Hsla,
}

impl SelectItem for Choice {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &SharedString {
        &self.value
    }

    /// The field shows what tells the value apart too, since values can
    /// share a label, as sessions share a title.
    fn display_title(&self) -> Option<AnyElement> {
        if self.detail.is_empty() {
            return None;
        }

        Some(
            h_flex()
                .min_w_0()
                .gap_2()
                .child(self.title.clone())
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(self.muted)
                        .child(self.detail.clone()),
                )
                .into_any_element(),
        )
    }

    /// One line: the list measures its rows as one.
    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex().gap_2().child(self.title.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.detail.clone()),
        )
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();

        [&self.value, &self.title, &self.detail]
            .iter()
            .any(|text| text.to_lowercase().contains(&query))
    }
}

impl Choice {
    /// A value as a person reads it: by its label when it has one, with the
    /// value and what describes it beside.
    fn of(value: &Value, muted: Hsla) -> Choice {
        let (title, shown) = if value.label.is_empty() {
            (value.value.as_str(), "")
        } else {
            (value.label.as_str(), value.value.as_str())
        };

        let detail = [shown, value.description.as_str()]
            .iter()
            .filter(|part| !part.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" · ");

        Choice {
            value: value.value.clone().into(),
            title: title.to_string().into(),
            detail: detail.into(),
            muted,
        }
    }
}

impl EventEmitter<Changed> for Form {}

impl Form {
    pub(super) fn new(
        fields: Vec<Field>,
        host_functions: Vec<Function>,
        provider_functions: Vec<Function>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut form = Form {
            rows: Vec::new(),
            host_functions,
            provider_functions,
            focus_when_listed: false,
        };

        for field in fields {
            let lister = commands::lister(&field.parameter);
            let initial = field.initial.clone();

            let mut row = Row {
                field,
                lister,
                control: Control::Listing(initial.clone()),
                asked: None,
                generation: 0,
                note: None,
                typed: false,
                _watching: None,
            };

            match row.lister.clone() {
                Lister::Fixed(values) => row.show_list(&values, &initial, window, cx),
                Lister::Typed => row.show_text(&initial, window, cx),
                Lister::Function(_) => {}
            }

            form.rows.push(row);
        }

        form
    }

    /// What each field holds, in order.
    pub(super) fn values(&self, cx: &App) -> Vec<String> {
        self.rows.iter().map(|row| row.value(cx)).collect()
    }

    /// The lists to ask for now: those never asked for, and those whose
    /// function would be given something else than it was.
    pub(super) fn wanted(&mut self, cx: &App) -> Vec<Wanted> {
        let values = self.values(cx);
        let mut wanted = Vec::new();

        for index in 0..self.rows.len() {
            let Lister::Function(function) = self.rows[index].lister.clone() else {
                continue;
            };

            if self.rows[index].typed {
                continue;
            }

            let source = self.rows[index].field.source;
            let functions = match source {
                Source::Host => &self.host_functions,
                Source::Provider => &self.provider_functions,
            };

            let given: Vec<(&str, &str)> = self.rows[..index]
                .iter()
                .zip(&values)
                .map(|(row, value)| (row.field.parameter.name.as_str(), value.as_str()))
                .collect();

            let lists = functions.iter().find(|known| known.name == function);
            let params = commands::passed(lists, &given);
            let row = &mut self.rows[index];

            if row.asked.as_ref() == Some(&params) {
                continue;
            }

            row.generation += 1;
            row.asked = Some(params.clone());
            row.control = Control::Listing(values[index].clone());
            row._watching = None;

            wanted.push(Wanted {
                field: index,
                generation: row.generation,
                function,
                source,
                params,
            });
        }

        wanted
    }

    /// Takes a list that was asked for: the values and what came with them,
    /// or why there are none. An answer to an earlier request than the one
    /// the field waits for is left aside.
    pub(super) fn listed(
        &mut self,
        field: usize,
        generation: u64,
        listed: Result<(Vec<Value>, String), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(row) = self.rows.get_mut(field) else {
            return;
        };

        if row.generation != generation || !matches!(row.control, Control::Listing(_)) {
            return;
        }

        let value = row.value(cx);

        match listed {
            Ok((values, said)) if !values.is_empty() => {
                row.note = (!said.is_empty()).then_some(Note::Said(said));
                row.show_list(&values, &value, window, cx);
            }

            // With nothing to choose from, the value is typed, and what the
            // function said tells why.
            Ok((_, said)) => {
                row.note = (!said.is_empty()).then_some(Note::Said(said));
                row.show_text(&value, window, cx);
            }

            Err(problem) => {
                row.note = Some(Note::Problem(format!("Not listed: {problem}")));
                row.show_text(&value, window, cx);
            }
        }

        if field == 0 && self.focus_when_listed {
            self.focus(window, cx);
        }

        cx.notify();
    }

    /// Turns a field whose values a function lists into one where the value
    /// is typed, starting from the one chosen, and gives it the keyboard.
    fn type_instead(&mut self, field: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.rows.get_mut(field) else {
            return;
        };

        let value = row.value(cx);

        row.typed = true;
        row.show_text(&value, window, cx);

        if let Control::Text(state) = &row.control {
            state.update(cx, |state, cx| state.focus(window, cx));
        }

        cx.notify();
    }

    /// Gives the keyboard to the first field, or to its list once it is
    /// there.
    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut App) {
        self.focus_when_listed = false;

        match self.rows.first().map(|row| &row.control) {
            Some(Control::List(state)) => state.update(cx, |state, cx| state.focus(window, cx)),
            Some(Control::Text(state)) => state.update(cx, |state, cx| state.focus(window, cx)),
            Some(Control::Listing(_)) => self.focus_when_listed = true,
            None => {}
        }
    }
}

impl Row {
    fn show_list(
        &mut self,
        values: &[Value],
        selected: &str,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) {
        let muted = cx.theme().muted_foreground;
        let mut choices = Vec::new();

        if let Some(empty) = &self.field.empty {
            choices.push(Choice {
                value: SharedString::default(),
                title: empty.clone().into(),
                detail: SharedString::default(),
                muted,
            });
        }

        choices.extend(values.iter().map(|value| Choice::of(value, muted)));

        // A value the list does not have is kept, rather than lost.
        let selected = selected.trim();

        if !selected.is_empty() && !choices.iter().any(|choice| choice.value == selected) {
            choices.push(Choice {
                value: selected.to_string().into(),
                title: selected.to_string().into(),
                detail: "not listed".into(),
                muted,
            });
        }

        let index = choices
            .iter()
            .position(|choice| choice.value == selected)
            .map(IndexPath::new);
        let searchable = choices.len() > 8;

        let state = cx.new(|cx| {
            SelectState::new(SearchableVec::new(choices), index, window, cx).searchable(searchable)
        });

        self._watching = Some(cx.subscribe_in(
            &state,
            window,
            |_, _, _: &SelectEvent<SearchableVec<Choice>>, _, cx| cx.emit(Changed),
        ));
        self.control = Control::List(state);
    }

    fn show_text(&mut self, value: &str, window: &mut Window, cx: &mut Context<Form>) {
        let hint = match (
            self.field.parameter.description.is_empty(),
            &self.field.parameter.default,
        ) {
            (false, _) => self.field.parameter.description.clone(),
            (true, Some(default)) => format!("By default {default}"),
            (true, None) => String::new(),
        };

        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(hint)
                .default_value(value.to_string())
        });

        // A value typed can change what a later list holds, which is asked
        // for once the field is left rather than on every key.
        self._watching =
            Some(
                cx.subscribe_in(&state, window, |_, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Blur) {
                        cx.emit(Changed);
                    }
                }),
            );
        self.control = Control::Text(state);
    }

    fn value(&self, cx: &App) -> String {
        match &self.control {
            Control::Listing(value) => value.clone(),
            Control::List(state) => state
                .read(cx)
                .selected_value()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            Control::Text(state) => state.read(cx).value().to_string(),
        }
    }
}

impl Render for Form {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let danger = cx.theme().danger;

        let rows = self.rows.iter().enumerate();

        rows.fold(v_flex().gap_3(), |form, (index, row)| {
            let parameter = &row.field.parameter;
            let label = if parameter.optional {
                format!("{} (optional)", commands::capitalized(&parameter.name))
            } else {
                commands::capitalized(&parameter.name)
            };

            let control = match &row.control {
                Control::Listing(_) => h_flex()
                    .gap_2()
                    .py_1()
                    .child(Spinner::new().xsmall())
                    .child(div().text_sm().text_color(muted).child("Listing…"))
                    .into_any_element(),
                Control::List(state) => Select::new(state).into_any_element(),
                Control::Text(state) => Input::new(state).into_any_element(),
            };

            let note = row.note.as_ref().map(|note| match note {
                Note::Said(text) => div().text_xs().text_color(muted).child(text.clone()),
                Note::Problem(text) => div().text_xs().text_color(danger).child(text.clone()),
            });

            // A list a function made may not hold the value wanted.
            let typing = (matches!(row.lister, Lister::Function(_))
                && matches!(row.control, Control::List(_)))
            .then(|| {
                Button::new(("type", index))
                    .link()
                    .xsmall()
                    .label("Type a value")
                    .tooltip("Type the value, starting from the one chosen, when the list does not have it")
                    .on_click(cx.listener(move |form, _, window, cx| {
                        form.type_instead(index, window, cx)
                    }))
            });

            form.when_some(row.field.heading.clone(), |form, heading| {
                form.child(div().pt_2().text_sm().text_color(muted).child(heading))
            })
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_sm().child(label))
                    .when(
                        !parameter.description.is_empty()
                            && !matches!(row.control, Control::Text(_)),
                        |column| {
                            column.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(parameter.description.clone()),
                            )
                        },
                    )
                    .child(control)
                    .children(note)
                    // At its own width, under the field's start.
                    .children(typing.map(|button| h_flex().child(button))),
            )
        })
    }
}
