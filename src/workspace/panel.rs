//! What shows beside the conversation: the changes in the project's folder,
//! as Git sees them, which is how a person reviews what a run did to the
//! files; or the session's details, as the host tells them, with its
//! skills and the folder it works in.

use std::path::PathBuf;

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    diff::{Diff, DiffFile, DiffState},
    h_flex,
    scroll::ScrollableElement as _,
    spinner::Spinner,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::{Panel, Workspace, WorkspaceEvent};
use crate::git;
use crate::protocol;

/// How wide the panel is.
const WIDTH: f32 = 480.;

/// What was read for a part of a panel.
#[derive(Clone)]
enum Read<T> {
    Reading,
    Here(T),
    Failed(String),
}

/// The changes, once read.
#[derive(Clone)]
struct Changed {
    branch: String,
    files: usize,
    additions: usize,
    deletions: usize,
    more_new: usize,
}

pub(super) struct Panels {
    changes: Option<Read<Changed>>,
    /// Every changed file, in one list.
    diff: Entity<DiffState>,
    /// Tells a reading of the changes from one asked for after it.
    reading: u64,
    status: Option<Read<String>>,
    skills: Option<Read<String>>,
}

impl Panels {
    pub(super) fn new(cx: &mut Context<Workspace>) -> Self {
        Panels {
            changes: None,
            diff: cx.new(|cx| DiffState::new(Vec::<DiffFile>::new(), cx)),
            reading: 0,
            status: None,
            skills: None,
        }
    }
}

impl Workspace {
    /// Shows a panel beside the conversation, or puts it away when it is
    /// the one shown.
    pub(super) fn toggle_panel(&mut self, panel: Panel, _: &mut Window, cx: &mut Context<Self>) {
        self.panel = match self.panel == Some(panel) {
            true => None,
            false => Some(panel),
        };

        self.refresh_panel(cx);
        cx.notify();
    }

    /// Reads again what the panel shown shows, since a run or another
    /// session may have changed it.
    pub(super) fn refresh_panel(&mut self, cx: &mut Context<Self>) {
        match self.panel {
            Some(Panel::Changes) => self.read_changes(cx),
            Some(Panel::Details) => self.read_details(cx),
            None => {}
        }
    }

    fn read_changes(&mut self, cx: &mut Context<Self>) {
        let dir = PathBuf::from(&self.dir);

        self.panels.reading += 1;

        let reading = self.panels.reading;

        // What was shown stays until the new reading replaces it.
        if self.panels.changes.is_none() {
            self.panels.changes = Some(Read::Reading);
        }

        cx.spawn(async move |this, cx| {
            let read = cx.background_spawn(async move { git::changes(&dir) }).await;

            let _ = this.update(cx, |this, cx| this.changes_read(reading, read, cx));
        })
        .detach();
    }

    fn changes_read(
        &mut self,
        reading: u64,
        read: Result<git::Changes, String>,
        cx: &mut Context<Self>,
    ) {
        if reading != self.panels.reading {
            return;
        }

        let read = read.and_then(|changes| {
            let files = DiffFile::parse(&changes.patch)
                .map_err(|error| format!("The changes could not be read: {error}"))?;

            Ok((changes, files))
        });

        self.panels.changes = Some(match read {
            Ok((changes, files)) => {
                let changed = Changed {
                    branch: changes.branch,
                    files: files.len(),
                    additions: files.iter().map(DiffFile::additions).sum(),
                    deletions: files.iter().map(DiffFile::deletions).sum(),
                    more_new: changes.more_new,
                };

                self.panels
                    .diff
                    .update(cx, |state, cx| state.set_files(files, cx));

                Read::Here(changed)
            }

            Err(problem) => Read::Failed(problem),
        });

        cx.notify();
    }

    /// Asks the host how the session stands and which skills it has.
    fn read_details(&mut self, cx: &mut Context<Self>) {
        for name in [protocol::FUNCTION_STATUS, protocol::FUNCTION_SKILLS] {
            let offered = self.host_function(name).is_some() && self.ended.is_none();
            let slot = match name == protocol::FUNCTION_STATUS {
                true => &mut self.panels.status,
                false => &mut self.panels.skills,
            };

            if !offered {
                *slot = None;
                continue;
            }

            if slot.is_none() {
                *slot = Some(Read::Reading);
            }

            let status = name == protocol::FUNCTION_STATUS;

            self.then_host(
                name,
                None,
                Box::new(move |this, answer, _, cx| {
                    let text = crate::commands::host_answer(&answer);
                    let slot = match status {
                        true => &mut this.panels.status,
                        false => &mut this.panels.skills,
                    };

                    *slot = Some(Read::Here(text));
                    cx.notify();
                }),
                cx,
            );
        }
    }

    pub(super) fn render_panel(&self, panel: Panel, cx: &mut Context<Self>) -> AnyElement {
        let (title, icon, body) = match panel {
            Panel::Changes => ("Changes", Lucide::GitCompare, self.render_changes(cx)),
            Panel::Details => ("Session", Lucide::Info, self.render_details(cx)),
        };

        let actions =
            h_flex()
                .gap_1()
                .child(
                    Button::new("panel-refresh")
                        .ghost()
                        .xsmall()
                        .icon(Lucide::RefreshCw)
                        .tooltip("Read it again")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_panel(cx);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("panel-close")
                        .ghost()
                        .xsmall()
                        .icon(Lucide::X)
                        .tooltip("Close")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.toggle_panel(panel, window, cx)
                        })),
                );

        v_flex()
            .w(px(WIDTH))
            .flex_shrink_0()
            .h_full()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .h(px(40.))
                    .flex_shrink_0()
                    .px_3()
                    .gap_2()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .gap_2()
                            .text_sm()
                            .child(Icon::new(icon).small())
                            .child(title),
                    )
                    .child(actions),
            )
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    fn render_changes(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;

        let changed = match &self.panels.changes {
            None | Some(Read::Reading) => {
                return centered().child(Spinner::new()).into_any_element();
            }

            Some(Read::Failed(problem)) => {
                return centered()
                    .child(div().text_sm().text_color(muted).child(problem.clone()))
                    .into_any_element();
            }

            Some(Read::Here(changed)) => changed.clone(),
        };

        let summary = h_flex()
            .flex_shrink_0()
            .px_3()
            .py_2()
            .gap_3()
            .text_xs()
            .text_color(muted)
            .border_b_1()
            .border_color(cx.theme().border)
            .when(!changed.branch.is_empty(), |row| {
                row.child(
                    h_flex()
                        .gap_1()
                        .child(Icon::new(Lucide::GitBranch).xsmall())
                        .child(changed.branch.clone()),
                )
            })
            .child(match changed.files {
                1 => "1 file".to_string(),
                files => format!("{files} files"),
            })
            .child(
                div()
                    .text_color(cx.theme().success)
                    .child(format!("+{}", changed.additions)),
            )
            .child(
                div()
                    .text_color(cx.theme().danger)
                    .child(format!("−{}", changed.deletions)),
            )
            .child(div().flex_1())
            .child(
                Button::new("changes-editor")
                    .ghost()
                    .xsmall()
                    .icon(Lucide::ExternalLink)
                    .label("Open in Editor")
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.emit(WorkspaceEvent::OpenInEditor {
                            path: this.dir.clone(),
                        })
                    })),
            );

        if changed.files == 0 && changed.more_new == 0 {
            return v_flex()
                .size_full()
                .child(summary)
                .child(
                    centered()
                        .child(Icon::new(Lucide::CircleCheck).text_color(muted))
                        .child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child("Nothing changed since the last commit."),
                        ),
                )
                .into_any_element();
        }

        v_flex()
            .size_full()
            .child(summary)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(Diff::new(&self.panels.diff).size_full()),
            )
            .when(changed.more_new > 0, |column| {
                column.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(muted)
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(format!(
                            "{} more new files are not shown.",
                            changed.more_new
                        )),
                )
            })
            .into_any_element()
    }

    fn render_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;

        let section = |title: &'static str, read: &Option<Read<String>>, cx: &App| {
            let body: AnyElement = match read {
                None => div()
                    .text_sm()
                    .text_color(muted)
                    .child("This host does not say.")
                    .into_any_element(),

                Some(Read::Reading) => Spinner::new().small().into_any_element(),

                Some(Read::Failed(problem)) | Some(Read::Here(problem)) => div()
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_xs()
                    .whitespace_normal()
                    .child(problem.clone())
                    .into_any_element(),
            };

            v_flex()
                .gap_2()
                .child(div().text_xs().text_color(muted).child(title))
                .child(body)
        };

        v_flex()
            .id("details")
            .size_full()
            .overflow_y_scrollbar()
            .p_3()
            .gap_4()
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_xs().text_color(muted).child("Folder"))
                    .child(div().text_sm().child(self.dir.clone()))
                    .child(
                        Button::new("details-editor")
                            .outline()
                            .small()
                            .icon(Lucide::ExternalLink)
                            .label("Open in Editor")
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.emit(WorkspaceEvent::OpenInEditor {
                                    path: this.dir.clone(),
                                })
                            })),
                    ),
            )
            .child(section("Status", &self.panels.status, cx))
            .child(section("Skills", &self.panels.skills, cx))
            .into_any_element()
    }
}

fn centered() -> gpui_kit::Div {
    v_flex()
        .size_full()
        .p_6()
        .gap_2()
        .items_center()
        .justify_center()
        .text_center()
}
