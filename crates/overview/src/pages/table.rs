use gpui::{AnyElement, AppContext as _, Context, Entity, SharedString, Styled as _, Window, px};
use ui::{Table, TableColumn, TableEvent, TableRow, TableState};

use super::{Demos, note, section, told};
use crate::Overview;

pub struct Demo {
    sessions: Entity<TableState>,
    wide: Entity<TableState>,
    empty: Entity<TableState>,
    told: Vec<SharedString>,
}

const TASKS: [&str; 8] = [
    "Fix the failing test in the host",
    "Add a slider to the settings",
    "Explain how the protocol orders its messages",
    "Port the composer to the new components",
    "Review the diff before the release",
    "Write the release notes",
    "Measure how long the build takes",
    "Find why the sidebar forgets its width",
];

const PROJECTS: [&str; 5] = ["rumbo", "rumbo-gui", "site", "notes", "infra"];

const MODELS: [&str; 4] = ["claude-sonnet-5", "claude-opus-5-5", "gpt-6", "local-qwen"];

/// A number with commas between its thousands.
fn thousands(number: u64) -> String {
    let digits = number.to_string();
    let mut grouped = String::new();

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }

        grouped.push(digit);
    }

    grouped
}

/// The sessions the page lists, made up but the same on every start.
fn sessions(count: u64) -> Vec<TableRow> {
    (0..count)
        .map(|n| {
            let tokens = (n * 7919 + 1234) % 180_000;
            let cost = tokens as f64 * 0.000_003;
            let day = 20 - (n / 24) % 20;
            let hour = 23 - n % 24;

            TableRow::new(
                format!("session-{n}"),
                [
                    TASKS[(n % 8) as usize].to_string(),
                    PROJECTS[(n % 5) as usize].to_string(),
                    MODELS[(n % 4) as usize].to_string(),
                    thousands(tokens),
                    format!("{cost:.2}"),
                    format!("2026-11-{day:02} {hour:02}:{:02}", (n * 13) % 60),
                ],
            )
        })
        .collect()
}

fn columns() -> Vec<TableColumn> {
    vec![
        TableColumn::new("task", "Task")
            .width(px(260.))
            .min_width(px(160.))
            .grow()
            .sortable(),
        TableColumn::new("project", "Project")
            .width(px(120.))
            .sortable(),
        TableColumn::new("model", "Model")
            .width(px(150.))
            .sortable(),
        TableColumn::new("tokens", "Tokens")
            .width(px(100.))
            .end()
            .sortable(),
        TableColumn::new("cost", "Cost (USD)")
            .width(px(130.))
            .end()
            .sortable(),
        TableColumn::new("updated", "Updated")
            .width(px(150.))
            .sortable(),
    ]
}

/// A table whose events the page tells, by `name`.
fn table(
    name: &'static str,
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    cx: &mut Context<Overview>,
) -> Entity<TableState> {
    let state = cx.new(|cx| TableState::new(columns, rows, cx));

    cx.subscribe(&state, move |this, _, event: &TableEvent, cx| {
        let line = match event {
            TableEvent::Select(key) => format!("{name}: chose {key}"),
            TableEvent::Open(key) => format!("{name}: opened {key}"),
        };

        note(&mut this.demos.table.told, line);
        cx.notify();
    })
    .detach();

    state
}

impl Demo {
    pub fn new(cx: &mut Context<Overview>) -> Self {
        let wide = (1..=8)
            .map(|n| {
                TableColumn::new(format!("c{n}"), format!("Column {n}"))
                    .width(px(140.))
                    .sortable()
            })
            .collect();
        let cells = (1..=30)
            .map(|row| {
                TableRow::new(
                    format!("row-{row}"),
                    (1..=8).map(|column| format!("Row {row}, column {column}")),
                )
            })
            .collect();

        Demo {
            sessions: table("Sessions", columns(), sessions(500), cx),
            wide: table("Wide", wide, cells, cx),
            empty: table("Empty", columns(), Vec::new(), cx),
            told: Vec::new(),
        }
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.table;

    vec![
        section(
            "Many rows",
            "500 sessions, of which only those in view are drawn. Press a title to sort by it, ascending, descending, then as given; drag the edge of a title to widen its column. Tab gives the table the keyboard: Up, Down, Page Up, Page Down, Home, and End choose a row, and Enter or a double press opens it.",
            Table::new(&demo.sessions).h(px(360.)),
            cx,
        ),
        section(
            "Wider than it is",
            "Columns wider than the table scroll across, with their titles.",
            Table::new(&demo.wide).w(px(560.)).h(px(260.)),
            cx,
        ),
        section(
            "Empty",
            "",
            Table::new(&demo.empty)
                .empty("No sessions yet. Start one from the sidebar.")
                .h(px(160.)),
            cx,
        ),
        section("What they told", "", told(&demo.told, cx), cx),
    ]
}
