//! The model and the options for the runs started from this window, chosen
//! among what the provider describes. What it lists in a function is asked
//! for first, so the picker opens with every list whole.

use std::rc::Rc;

use gpui_kit::component::{
    ActiveTheme as _, IndexPath, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex,
    input::{Input, InputState},
    select::{SearchableVec, Select, SelectItem, SelectState},
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, SharedString,
    Styled as _, Window, div, prelude::FluentBuilder as _, px,
};
use serde_json::Value as Json;

use super::{Pending, Workspace};
use crate::choices::{self, Accepts, Choices};
use crate::protocol::{self, DescribeResult, InvokeParams, Parameter, Value};

/// What the picker is being made of, while the provider's lists arrive.
pub(super) struct Picking {
    described: DescribeResult,
    /// The model first, then each option the provider describes.
    rows: Vec<Row>,
    /// How many lists are still to arrive.
    waiting: usize,
}

struct Row {
    parameter: Parameter,
    accepts: Accepts,
    /// What a function listed, or why it could not.
    listed: Option<Result<Vec<Value>, String>>,
    in_force: Option<String>,
    chosen: String,
}

/// How a row is picked.
enum Control {
    List(Entity<SelectState<SearchableVec<Choice>>>),
    Text(Entity<InputState>),
}

impl Control {
    fn text(&self, cx: &App) -> String {
        match self {
            Control::List(state) => state
                .read(cx)
                .selected_value()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            Control::Text(state) => state.read(cx).value().to_string(),
        }
    }
}

/// One value in a list, with what tells it apart. An empty value keeps what
/// the session uses.
#[derive(Clone)]
struct Choice {
    value: SharedString,
    title: SharedString,
    detail: SharedString,
}

impl SelectItem for Choice {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &SharedString {
        &self.value
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

        self.title.to_lowercase().contains(&query) || self.detail.to_lowercase().contains(&query)
    }
}

impl Workspace {
    /// Asks the provider what can be chosen, to open the picker with it.
    pub(super) fn open_picker(&mut self, cx: &mut Context<Self>) {
        if self.ended.is_some() || self.picking.is_some() {
            return;
        }

        self.notice = Some("Asking the provider what can be chosen".into());
        self.request(protocol::METHOD_DESCRIBE, Json::Null, Pending::Picker);
        cx.notify();
    }

    /// Takes what the provider described, and asks for the lists it names.
    pub(super) fn described_for_picker(
        &mut self,
        described: Result<DescribeResult, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let described = match described {
            Ok(described) => described,
            Err(problem) => {
                self.notice = Some(format!(
                    "The provider could not say what it offers: {problem}"
                ));
                return;
            }
        };

        let mut model = described.offers.model.clone().unwrap_or_default();
        model.name = "model".into();

        let mut rows = vec![Row {
            accepts: choices::accepts(&model),
            parameter: model,
            listed: None,
            in_force: Some(described.model.clone()).filter(|model| !model.is_empty()),
            chosen: self.choices.model.clone(),
        }];

        for option in &described.offers.options {
            rows.push(Row {
                accepts: choices::accepts(option),
                listed: None,
                in_force: choices::in_force(
                    option,
                    described.options.as_ref(),
                    &Choices::default(),
                ),
                chosen: choices::chosen_text(option, &self.choices),
                parameter: option.clone(),
            });
        }

        let mut waiting = 0;

        for (index, row) in rows.iter().enumerate() {
            if let Accepts::Listed(function) = &row.accepts {
                let asked = self.request(
                    protocol::METHOD_INVOKE,
                    InvokeParams {
                        name: function.clone(),
                        params: None,
                        target: "",
                    },
                    Pending::Values(index),
                );

                waiting += usize::from(asked.is_some());
            }
        }

        self.picking = Some(Picking {
            described,
            rows,
            waiting,
        });

        if waiting == 0 {
            self.show_picker(window, cx);
        }
    }

    /// Takes a list the provider gave for a row, and opens the picker once
    /// every list is in.
    pub(super) fn listed_for_picker(
        &mut self,
        index: usize,
        listed: Result<Vec<Value>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(picking) = self.picking.as_mut() else {
            return;
        };

        if let Some(row) = picking.rows.get_mut(index) {
            row.listed = Some(listed);
        }

        picking.waiting = picking.waiting.saturating_sub(1);

        if picking.waiting == 0 {
            self.show_picker(window, cx);
        }
    }

    fn show_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picking) = self.picking.take() else {
            return;
        };

        self.notice = None;

        let controls: Vec<Control> = picking
            .rows
            .iter()
            .map(|row| control(row, window, cx))
            .collect();

        let rows = Rc::new(picking.rows);
        let controls = Rc::new(controls);
        let provider = picking.described.provider.clone();
        let workspace = cx.weak_entity();

        window.open_dialog(cx, move |dialog, _, cx| {
            let form = rows.iter().zip(controls.iter()).fold(v_flex().gap_3(), |form, (row, control)| {
                form.child(render_row(row, control, cx))
            });

            let apply = {
                let rows = rows.clone();
                let controls = controls.clone();
                let workspace = workspace.clone();

                move |_: &_, window: &mut Window, cx: &mut App| {
                    let picked: Vec<String> = controls.iter().map(|control| control.text(cx)).collect();
                    let options: Vec<Parameter> = rows[1..].iter().map(|row| row.parameter.clone()).collect();

                    match choices::choose(&picked[0], &options, &picked[1..]) {
                        Ok(chosen) => {
                            let _ = workspace.update(cx, |this, cx| {
                                this.choices = chosen;
                                cx.notify();
                            });

                            true
                        }

                        Err(problem) => {
                            window.push_notification(problem, cx);
                            false
                        }
                    }
                }
            };

            let reset = {
                let workspace = workspace.clone();

                move |_: &_, window: &mut Window, cx: &mut App| {
                    let _ = workspace.update(cx, |this, cx| {
                        this.choices = Choices::default();
                        cx.notify();
                    });

                    window.close_dialog(cx);
                }
            };

            dialog
                .title("Model and options")
                .w(px(480.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "For the tasks started from this window, with provider {provider}. What is left as the session has it keeps its value."
                                )),
                        )
                        .child(form),
                )
                .on_ok(apply)
                .footer(
                    DialogFooter::new()
                        .gap_2()
                        .child(Button::new("reset").ghost().label("Use the Session's").on_click(reset))
                        .child(DialogClose::new().child(Button::new("cancel").outline().label("Cancel")))
                        .child(DialogAction::new().child(Button::new("apply").primary().label("Apply"))),
                )
        });
    }
}

/// The control for a row: a list when the values are known, else a field.
fn control(row: &Row, window: &mut Window, cx: &mut Context<Workspace>) -> Control {
    let values = match (&row.accepts, &row.listed) {
        (Accepts::Values(values), _) => Some(values.clone()),
        (Accepts::Listed(_), Some(Ok(values))) => Some(values.clone()),
        _ => None,
    };

    let Some(values) = values else {
        let in_force = row.in_force.clone().unwrap_or_default();
        let chosen = row.chosen.clone();

        return Control::Text(cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(in_force)
                .default_value(chosen)
        }));
    };

    let session = match &row.in_force {
        Some(value) => format!("As the session has it: {value}"),
        None => "As the session has it".to_string(),
    };

    let mut items = vec![Choice {
        value: SharedString::default(),
        title: session.into(),
        detail: SharedString::default(),
    }];

    items.extend(values.iter().map(|value| {
        Choice {
            value: value.value.clone().into(),
            title: value.value.clone().into(),
            detail: [value.label.as_str(), value.description.as_str()]
                .iter()
                .filter(|part| !part.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" · ")
                .into(),
        }
    }));

    let selected = items
        .iter()
        .position(|item| item.value.as_ref() == row.chosen)
        .unwrap_or(0);
    let searchable = items.len() > 8;

    Control::List(cx.new(|cx| {
        SelectState::new(
            SearchableVec::new(items),
            Some(IndexPath::new(selected)),
            window,
            cx,
        )
        .searchable(searchable)
    }))
}

fn render_row(row: &Row, control: &Control, cx: &App) -> impl IntoElement {
    let field = match control {
        Control::List(state) => Select::new(state).into_any_element(),
        Control::Text(state) => Input::new(state).into_any_element(),
    };

    let unlisted = match &row.listed {
        Some(Err(problem)) => Some(format!("The provider could not list them: {problem}")),
        _ => None,
    };

    v_flex()
        .gap_1()
        .child(div().text_sm().child(row.parameter.name.clone()))
        .when(!row.parameter.description.is_empty(), |column| {
            column.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(row.parameter.description.clone()),
            )
        })
        .child(field)
        .when_some(unlisted, |column, problem| {
            column.child(div().text_xs().text_color(cx.theme().danger).child(problem))
        })
}
