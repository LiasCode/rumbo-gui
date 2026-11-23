//! The project file explorer. Only opened folders are read, off the UI
//! thread; opening a file hands its path to the configured editor. Each
//! file and folder is marked with what Git says of it, in the project's
//! repository and in any repository nested in it whose folder is opened.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, AppContext as _, Context, Div, ElementId, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, ScrollHandle, StatefulInteractiveElement as _, Styled as _,
    div, prelude::FluentBuilder as _,
};
use ui::{
    ActiveTheme as _, Button, IconName, Scrollbar, Sizable as _, Spinner, ThemeColors, h_flex,
    rems_from_px, v_flex,
};

use super::panel::Read;
use super::{Workspace, WorkspaceEvent};
use crate::file_tree::{self, Entry, Kind};
use crate::git::{self, FileStatus};

#[derive(Default)]
pub(super) struct Files {
    root: PathBuf,
    generation: u64,
    folders: HashMap<PathBuf, Read<Vec<Entry>>>,
    expanded: HashSet<PathBuf>,
    /// What Git says, by the folder it was asked in: the project's, and
    /// each repository nested in it that was opened.
    statuses: HashMap<PathBuf, git::Statuses>,
    scroll: ScrollHandle,
}

/// The color a file or a folder is named in, by what Git says of it.
fn status_color(status: FileStatus, colors: &ThemeColors) -> Hsla {
    match status {
        FileStatus::Untracked | FileStatus::Added => colors.success,
        FileStatus::Modified | FileStatus::Renamed => colors.warning,
        FileStatus::Deleted | FileStatus::Conflicted => colors.danger,
    }
}

impl Workspace {
    /// Reads the files of a project shown for the first time; otherwise
    /// reads again what is open, as a run may have changed it, keeping the
    /// folders open.
    pub(super) fn refresh_files(&mut self, cx: &mut Context<Self>) {
        if self.files.root != Path::new(&self.dir) || self.files.folders.is_empty() {
            self.reload_files(cx);
            return;
        }

        let loaded: Vec<PathBuf> = self.files.folders.keys().cloned().collect();

        for folder in loaded {
            self.read_folder(folder, cx);
        }
    }

    fn reload_files(&mut self, cx: &mut Context<Self>) {
        self.files = Files {
            root: PathBuf::from(&self.dir),
            generation: self.files.generation + 1,
            ..Files::default()
        };

        if !self.files.root.as_os_str().is_empty() {
            self.read_folder(self.files.root.clone(), cx);
        }

        cx.notify();
    }

    /// Reads a folder, showing what was read of it before until it is read
    /// again; and what Git says when it is the project's or holds a
    /// repository.
    fn read_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let generation = self.files.generation;
        let root = path == self.files.root;

        self.files
            .folders
            .entry(path.clone())
            .or_insert(Read::Reading);

        cx.spawn(async move |this, cx| {
            let (path, result, repository) = cx
                .background_spawn(async move {
                    let result = file_tree::children(&path);
                    let repository = root || file_tree::holds_repository(&path);

                    (path, result, repository)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if this.files.generation != generation {
                    return;
                }

                let folder = match result {
                    Ok(entries) => Read::Here(entries),
                    Err(error) => Read::Failed(format!("Could not read this folder: {error}")),
                };

                if repository {
                    this.read_statuses(path.clone(), cx);
                }

                this.files.folders.insert(path, folder);
                cx.notify();
            });
        })
        .detach();
    }

    /// Asks Git what it says of the files under a folder. A folder in no
    /// repository has nothing to mark.
    fn read_statuses(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let generation = self.files.generation;

        cx.spawn(async move |this, cx| {
            let (path, read) = cx
                .background_spawn(async move {
                    let read = git::statuses(&path);

                    (path, read)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if this.files.generation != generation {
                    return;
                }

                match read {
                    Ok(statuses) => this.files.statuses.insert(path, statuses),
                    Err(_) => this.files.statuses.remove(&path),
                };

                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !self.files.expanded.remove(&path) {
            self.files.expanded.insert(path.clone());

            if !matches!(
                self.files.folders.get(&path),
                Some(Read::Here(_) | Read::Reading)
            ) {
                self.read_folder(path, cx);
            }
        }

        cx.notify();
    }

    pub(super) fn render_files(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().colors.muted_foreground;
        let root = &self.files.root;
        let tree = v_flex()
            .id("project-files")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.files.scroll)
            .p_2()
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .text_color(muted)
                    .truncate()
                    .child(self.dir.clone()),
            )
            .child(self.render_folder(root, 0, cx));

        div()
            .relative()
            .size_full()
            .child(tree)
            .child(Scrollbar::vertical(
                "files-scrollbar",
                self.files.scroll.clone(),
            ))
            .into_any_element()
    }

    fn render_folder(&self, path: &Path, depth: usize, cx: &mut Context<Self>) -> Div {
        let message = |text: String| {
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(cx.theme().colors.muted_foreground)
                .child(text)
        };
        let mut column = v_flex();

        match self.files.folders.get(path) {
            None => column.child(message("No project folder is open.".into())),
            Some(Read::Reading) => column.child(div().p_2().child(Spinner::new().xsmall())),
            Some(Read::Failed(problem)) => column.child(message(problem.clone())),
            Some(Read::Here(entries)) if entries.is_empty() => {
                column.child(message("This folder is empty.".into()))
            }
            Some(Read::Here(entries)) => {
                let colors = cx.theme().colors;

                for (index, entry) in entries.iter().enumerate() {
                    let child = path.join(&entry.name);
                    let expanded = self.files.expanded.contains(&child);
                    let name = entry.name.to_string_lossy().into_owned();
                    let folder = entry.kind == Kind::Folder;
                    let openable = entry.kind == Kind::File && child.to_str().is_some();
                    let status = git::status_at(&self.files.statuses, &child, folder);
                    let color = status.map(|status| status_color(status, &colors));
                    let said = status
                        .map(|status| format!(" · {}", status.words()))
                        .unwrap_or_default();
                    let icon = match (entry.kind, expanded) {
                        (Kind::Folder, true) => IconName::FolderOpen,
                        (Kind::Folder, false) => IconName::FolderClosed,
                        _ => IconName::FileText,
                    };
                    let id = ElementId::Name(format!("{}:{index}", path.display()).into());
                    let row = Button::new(id)
                        .ghost()
                        .small()
                        .flex_1()
                        .min_w_0()
                        .justify_start()
                        .pl(rems_from_px(8. + 14. * depth.min(16) as f32))
                        .truncate()
                        .icon(icon)
                        .label(name.clone())
                        .when_some(color, |row, color| row.text_color(color));
                    let row = if folder {
                        let folder = child.clone();

                        row.tooltip(format!("{name}{said}")).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.toggle_folder(folder.clone(), cx);
                            },
                        ))
                    } else if openable {
                        let path = child.to_string_lossy().into_owned();

                        row.tooltip(format!("Open {name} in Editor{said}"))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(WorkspaceEvent::OpenInEditor { path: path.clone() });
                            }))
                    } else {
                        row.disabled(true)
                    };

                    // A letter says what Git says of a file; a dot, that a
                    // folder holds what it says of some.
                    let mark = h_flex()
                        .flex_none()
                        .w(rems_from_px(20.))
                        .justify_center()
                        .when_some(status.zip(color), |mark, (status, color)| match folder {
                            true => {
                                mark.child(div().size(rems_from_px(6.)).rounded_full().bg(color))
                            }
                            false => mark
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(color)
                                .child(status.letter()),
                        });

                    column = column
                        .child(h_flex().w_full().items_center().child(row).child(mark))
                        .when(expanded, |column| {
                            column.child(self.render_folder(&child, depth + 1, cx))
                        });
                }

                column
            }
        }
    }
}
