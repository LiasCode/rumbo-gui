//! What shows beside the conversation: the project's Git repository, with
//! the changes in its folder, which is how a person reviews what a run did
//! to the files, and the history of its branches; the project file
//! explorer; or the session's details. It is as wide as the person drags
//! it, the same for every project.

use std::path::PathBuf;

use gpui::{
    AnyElement, AppContext as _, Context, Div, Entity, IntoElement, ParentElement as _,
    ScrollHandle, Styled as _, Window, div, prelude::FluentBuilder as _,
};
use ui::{
    ActiveTheme as _, Button, Diff, DiffFile, DiffState, Icon, IconName, Sizable as _, Spinner,
    Tab, Tabs, h_flex, rems_from_px, v_flex,
};

use super::git_history::GitHistory;
use super::{Panel, Workspace, WorkspaceEvent};
use crate::git;
use crate::protocol;

/// The panel's width, in pixels at the interface's own size, when the
/// person has not chosen one, and the least and most it can be dragged to.
pub(super) const DEFAULT_WIDTH: f32 = 480.;
pub(super) const MIN_WIDTH: f32 = 320.;
pub(super) const MAX_WIDTH: f32 = 800.;

/// The panel's width, from the one the person chose, if any.
pub(super) fn width(chosen: Option<u32>) -> f32 {
    chosen
        .map(|width| (width as f32).clamp(MIN_WIDTH, MAX_WIDTH))
        .unwrap_or(DEFAULT_WIDTH)
}

/// What was read for a part of a panel.
#[derive(Clone)]
pub(super) enum Read<T> {
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
    /// The Git panel shows the history of its branches, not the changes.
    showing_history: bool,
    pub(super) history: GitHistory,
    /// Every changed file, in one list.
    diff: Entity<DiffState>,
    /// Tells a reading of the changes from one asked for after it.
    reading: u64,
    pub(super) status: Option<Read<String>>,
    pub(super) skills: Option<Read<String>>,
    /// Where the details are scrolled.
    pub(super) details_scroll: ScrollHandle,
    /// Whether all the host says of the session shows, below its details.
    pub(super) everything: bool,
}

impl Panels {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        Panels {
            changes: None,
            showing_history: false,
            history: GitHistory::new(window, cx),
            diff: cx.new(|cx| DiffState::new(Vec::<DiffFile>::new(), cx)),
            reading: 0,
            status: None,
            skills: None,
            details_scroll: ScrollHandle::new(),
            everything: false,
        }
    }
}

impl Workspace {
    /// Shows a panel beside the conversation, or puts it away when it is
    /// the one shown.
    pub fn toggle_panel(&mut self, panel: Panel, _: &mut Window, cx: &mut Context<Self>) {
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
            Some(Panel::Changes) if self.panels.showing_history => self.read_git_history(cx),
            Some(Panel::Changes) => self.read_changes(cx),
            Some(Panel::Files) => self.refresh_files(cx),
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

        let read = read.map(|changes| {
            let files = DiffFile::parse(&changes.patch);

            (changes, files)
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
            Panel::Changes if self.panels.showing_history => {
                ("Git", IconName::GitBranch, self.render_git_history(cx))
            }
            Panel::Changes => ("Git", IconName::GitBranch, self.render_changes(cx)),
            Panel::Files => ("Files", IconName::FolderOpen, self.render_files(cx)),
            Panel::Details => ("Session", IconName::Info, self.render_details(cx)),
        };

        let actions =
            h_flex()
                .gap_1()
                .child(
                    Button::new("panel-refresh")
                        .ghost()
                        .xsmall()
                        .icon(IconName::RefreshCw)
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
                        .icon(IconName::Close)
                        .tooltip("Close")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.toggle_panel(panel, window, cx)
                        })),
                );

        // Its width is the divider's beside the conversation, which draws
        // the line between them too.
        v_flex()
            .size_full()
            .bg(cx.theme().colors.background)
            .child(
                h_flex()
                    .h(rems_from_px(40.))
                    .flex_shrink_0()
                    .px_3()
                    .gap_2()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().colors.border)
                    .child(
                        h_flex()
                            .gap_2()
                            .text_sm()
                            .child(Icon::new(icon).small())
                            .child(title),
                    )
                    .when(panel == Panel::Changes, |header| {
                        header.child(self.render_git_tabs(cx))
                    })
                    .child(actions),
            )
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    /// The Git panel's tabs: the changes in the folder, or the history of
    /// its branches.
    fn render_git_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        Tabs::new("git-tabs")
            .child(Tab::new("Changes"))
            .child(Tab::new("History"))
            .selected(usize::from(self.panels.showing_history))
            .xsmall()
            .on_change(cx.listener(|this, tab, _, cx| {
                let showing_history = *tab == 1;

                if this.panels.showing_history != showing_history {
                    this.panels.showing_history = showing_history;
                    this.refresh_panel(cx);
                    cx.notify();
                }
            }))
    }

    fn render_changes(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors;
        let muted = colors.muted_foreground;

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
            .border_color(colors.border)
            .when(!changed.branch.is_empty(), |row| {
                row.child(
                    h_flex()
                        .gap_1()
                        .child(Icon::new(IconName::GitBranch).xsmall())
                        .child(changed.branch.clone()),
                )
            })
            .child(match changed.files {
                1 => "1 file".to_string(),
                files => format!("{files} files"),
            })
            .child(
                div()
                    .text_color(colors.success)
                    .child(format!("+{}", changed.additions)),
            )
            .child(
                div()
                    .text_color(colors.danger)
                    .child(format!("−{}", changed.deletions)),
            )
            .child(div().flex_1())
            .child(
                Button::new("changes-editor")
                    .ghost()
                    .xsmall()
                    .icon(IconName::ExternalLink)
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
                        .child(Icon::new(IconName::CircleCheck).color(muted))
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
                    .child(Diff::new("changes", &self.panels.diff)),
            )
            .when(changed.more_new > 0, |column| {
                column.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(muted)
                        .border_t_1()
                        .border_color(colors.border)
                        .child(format!(
                            "{} more new files are not shown.",
                            changed.more_new
                        )),
                )
            })
            .into_any_element()
    }
}

pub(super) fn centered() -> Div {
    v_flex()
        .size_full()
        .p_6()
        .gap_2()
        .items_center()
        .justify_center()
        .text_center()
}
