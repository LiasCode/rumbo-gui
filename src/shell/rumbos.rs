//! What the rumbos the window holds use, in a dialog: one for each session
//! open, by project, with its processes, their memory, and their processor
//! time, and what they use together. Each rumbo is asked when the dialog
//! opens and when the person asks again; `crate::rumbos` counts.

use gpui::{
    AnyElement, App, Context, Div, IntoElement, ParentElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};
use ui::{ActiveTheme as _, h_flex, rems_from_px, v_flex};

use super::{Shell, State, folder_name};
use crate::rumbos::{self, Rumbo, Standing, Used};

impl Shell {
    /// The rumbos of every project open, each project with its name.
    fn rumbos(&self, cx: &App) -> Vec<(String, Vec<Rumbo>)> {
        self.projects
            .iter()
            .filter_map(|project| match &project.state {
                State::Working { workspace, .. } => {
                    let mut held = workspace.read(cx).rumbos();

                    // A project put aside shows none of its sessions.
                    if self.selected != Some(project.id) {
                        for rumbo in &mut held {
                            if rumbo.standing == Standing::Shown {
                                rumbo.standing = Standing::Open;
                            }
                        }
                    }

                    Some((folder_name(&project.dir), held))
                }
                _ => None,
            })
            .collect()
    }

    /// Asks every rumbo of the window what its processes use.
    fn measure_rumbos(&self, cx: &mut App) {
        for project in &self.projects {
            if let State::Working { workspace, .. } = &project.state {
                workspace.update(cx, |workspace, _| workspace.measure());
            }
        }
    }

    pub(super) fn open_rumbos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.measure_rumbos(cx);

        let shell = cx.weak_entity();

        self.dialogs.update(cx, |dialogs, cx| {
            dialogs.open(
                move |dialog, _, cx| {
                    let projects = shell
                        .read_with(cx, |shell, cx| shell.rumbos(cx))
                        .unwrap_or_default();
                    let again = shell.clone();

                    dialog
                        .title("Rumbos")
                        .width(rems_from_px(640.))
                        .child(table(&projects, cx))
                        .on_cancel("Close", |_, _| true)
                        .on_ok("Measure Again", move |_, cx| {
                            let _ = again.update(cx, |shell, cx| shell.measure_rumbos(cx));
                            false
                        })
                },
                window,
                cx,
            )
        });
    }
}

/// The rumbos by project, one to a row, and what they use together.
fn table(projects: &[(String, Vec<Rumbo>)], cx: &App) -> AnyElement {
    let colors = cx.theme().colors;

    // Each process is counted once across the window, whichever rumbo
    // lists it.
    let listed: Vec<&[crate::protocol::ProcessUsage]> = projects
        .iter()
        .flat_map(|(_, held)| held)
        .map(|rumbo| match &rumbo.processes {
            Some(Ok(processes)) => processes.as_slice(),
            _ => &[],
        })
        .collect();

    let mut each = rumbos::used(&listed).into_iter();
    let mut total = Used::default();
    let mut count = 0;

    let mut rows = v_flex().gap_1().text_sm().child(
        row("Session", "", "Processes", "Memory", "CPU")
            .text_xs()
            .text_color(colors.muted_foreground),
    );

    for (project, held) in projects {
        rows = rows.child(
            div()
                .pt_2()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.muted_foreground)
                .child(project.clone()),
        );

        for rumbo in held {
            let used = each.next().unwrap_or_default();

            count += 1;
            total.add(used);

            rows = rows.child(match &rumbo.processes {
                Some(Ok(_)) => row(
                    &rumbo.title,
                    rumbo.standing.said(),
                    &used.processes.to_string(),
                    &rumbos::memory(used.memory),
                    &rumbos::processor_time(used.cpu),
                ),

                Some(Err(problem)) => row(&rumbo.title, rumbo.standing.said(), "", "", "").child(
                    div()
                        .text_xs()
                        .text_color(colors.danger)
                        .child(problem.clone()),
                ),

                None => row(&rumbo.title, rumbo.standing.said(), "…", "…", "…"),
            });
        }
    }

    let together = match count {
        1 => "1 rumbo".to_string(),
        _ => format!("{count} rumbos"),
    };

    rows.child(
        row(
            &together,
            "",
            &total.processes.to_string(),
            &rumbos::memory(total.memory),
            &rumbos::processor_time(total.cpu),
        )
        .mt_2()
        .pt_2()
        .border_t_1()
        .border_color(colors.border)
        .font_weight(gpui::FontWeight::MEDIUM),
    )
    .when(count == 0, |rows| rows.child("No rumbo is open."))
    .into_any_element()
}

/// A row of the table: what it is of, how it stands, and its figures, set
/// to the right under their headings.
fn row(name: &str, standing: &str, processes: &str, memory: &str, cpu: &str) -> Div {
    let figure = |text: &str, width: f32| {
        div()
            .w(rems_from_px(width))
            .flex_shrink_0()
            .text_right()
            .child(text.to_string())
    };

    h_flex()
        .gap_3()
        .child(div().flex_1().min_w_0().truncate().child(name.to_string()))
        .child(
            div()
                .w(rems_from_px(64.))
                .flex_shrink_0()
                .child(standing.to_string()),
        )
        .child(figure(processes, 72.))
        .child(figure(memory, 88.))
        .child(figure(cpu, 64.))
}
