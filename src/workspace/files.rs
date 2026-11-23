//! The project file explorer. Only opened folders are read, off the UI
//! thread; a file pressed shows beside the tree, read off the UI thread
//! too, and its divider moves. Each file and folder is marked with what
//! Git says of it, in the project's repository and in any repository nested
//! in it whose folder is opened.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, AppContext as _, Context, Div, ElementId, Entity, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, ScrollHandle, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _,
};
use ui::{
    ActiveTheme as _, Button, CodeView, CodeViewState, Icon, IconName, PanelSize, Resizable,
    ResizablePanel, Scrollbar, Sizable as _, Spinner, ThemeColors, h_flex, rems_from_px, v_flex,
};

use super::panel::{Read, centered, message};
use super::{Workspace, WorkspaceEvent};
use crate::file_tree::{self, Entry, Kind};
use crate::git::{self, FileStatus};

/// The tree's width beside a file shown, in pixels at the interface's own
/// size, and the least and most it can be dragged to.
const TREE_WIDTH: f32 = 200.;
const TREE_MIN_WIDTH: f32 = 120.;
const TREE_MAX_WIDTH: f32 = 480.;

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
    /// The file shown beside the tree, if any.
    shown: Option<Shown>,
    /// The tree's width beside it, once dragged.
    tree_width: Option<f32>,
}

/// A file shown beside the tree, and its text once read.
struct Shown {
    path: PathBuf,
    text: Read<Entity<CodeViewState>>,
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
    /// reads again what is open, and the file shown, as a run may have
    /// changed them, keeping the folders open.
    pub(super) fn refresh_files(&mut self, cx: &mut Context<Self>) {
        if self.files.root != Path::new(&self.dir) || self.files.folders.is_empty() {
            self.reload_files(cx);
            return;
        }

        let loaded: Vec<PathBuf> = self.files.folders.keys().cloned().collect();

        for folder in loaded {
            self.read_folder(folder, cx);
        }

        self.read_shown_file(cx);
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

    /// Shows a file beside the tree.
    fn show_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.files.shown = Some(Shown {
            path,
            text: Read::Reading,
        });

        self.read_shown_file(cx);
        cx.notify();
    }

    fn close_file(&mut self, cx: &mut Context<Self>) {
        self.files.shown = None;
        cx.notify();
    }

    /// Reads the file shown, off the UI thread. What was shown of it stays,
    /// scrolled where it was, until it is read again.
    fn read_shown_file(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = &self.files.shown else {
            return;
        };

        let path = shown.path.clone();
        let generation = self.files.generation;

        cx.spawn(async move |this, cx| {
            let (path, read) = cx
                .background_spawn(async move {
                    let read = file_tree::text(&path);

                    (path, read)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if this.files.generation != generation {
                    return;
                }

                // Another file may be shown by now.
                let Some(shown) = this.files.shown.as_mut().filter(|shown| shown.path == path)
                else {
                    return;
                };

                // The language is named by the file's extension, or by its
                // name when it has none, as a Dockerfile's is.
                let lang = path
                    .extension()
                    .or_else(|| path.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();

                shown.text = match (read, &shown.text) {
                    (Ok(text), Read::Here(state)) => {
                        state.update(cx, |state, cx| state.set_text(&text, &lang, cx));

                        Read::Here(state.clone())
                    }

                    (Ok(text), _) => Read::Here(cx.new(|cx| {
                        let mut state = CodeViewState::new(cx);

                        state.set_text(&text, &lang, cx);
                        state
                    })),

                    (Err(problem), _) => Read::Failed(problem),
                };

                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn render_files(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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

        let tree = div()
            .relative()
            .size_full()
            .child(tree)
            .child(Scrollbar::vertical(
                "files-scrollbar",
                self.files.scroll.clone(),
            ))
            .into_any_element();

        let Some(shown) = &self.files.shown else {
            return tree;
        };

        let rem = window.rem_size();
        let width = self.files.tree_width.unwrap_or(TREE_WIDTH);

        // The file keeps no least width, so a narrow panel cuts off none of
        // the tree.
        Resizable::horizontal("files-split")
            .child(
                ResizablePanel::new("tree")
                    .size(rems_from_px(width).to_pixels(rem))
                    .min(rems_from_px(TREE_MIN_WIDTH).to_pixels(rem))
                    .max(rems_from_px(TREE_MAX_WIDTH).to_pixels(rem))
                    .child(tree),
            )
            .child(
                ResizablePanel::new("file")
                    .min(Pixels::ZERO)
                    .child(self.render_shown_file(shown, cx)),
            )
            .on_resize(cx.listener(|this, size: &PanelSize, window, cx| {
                if size.key != "tree" {
                    return;
                }

                // It is kept at the interface's own size, as it grows with it.
                let pixel = rems_from_px(1.).to_pixels(window.rem_size());

                this.files.tree_width =
                    Some((size.size / pixel).clamp(TREE_MIN_WIDTH, TREE_MAX_WIDTH));
                cx.notify();
            }))
            .into_any_element()
    }

    /// The file shown beside the tree: its path under the project, with
    /// ways to open it in the editor and to close it, over its text.
    fn render_shown_file(&self, shown: &Shown, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors;
        let muted = colors.muted_foreground;
        let name = shown
            .path
            .strip_prefix(&self.files.root)
            .unwrap_or(&shown.path)
            .display()
            .to_string();

        // The editor is given the path as text, which only a path in UTF-8
        // is.
        let editable = shown.path.to_str().map(str::to_string);

        let header = h_flex()
            .flex_shrink_0()
            .h(rems_from_px(36.))
            .pl_3()
            .pr_1()
            .gap_2()
            .text_sm()
            .border_b_1()
            .border_color(colors.border)
            .child(Icon::new(IconName::FileText).small().color(muted))
            .child(div().flex_1().min_w_0().truncate().child(name))
            .when_some(editable, |header, path| {
                header.child(
                    Button::new("shown-file-editor")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ExternalLink)
                        .tooltip("Open this file in the editor")
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(WorkspaceEvent::OpenInEditor { path: path.clone() })
                        })),
                )
            })
            .child(
                Button::new("shown-file-close")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .tooltip("Close this file")
                    .on_click(cx.listener(|this, _, _, cx| this.close_file(cx))),
            );

        let body = match &shown.text {
            Read::Reading => centered().child(Spinner::new()).into_any_element(),
            Read::Failed(problem) => message(problem.clone(), cx),
            Read::Here(state) if state.read(cx).is_empty() => message("This file is empty.", cx),
            Read::Here(state) => CodeView::new("shown-file", state).into_any_element(),
        };

        v_flex()
            .size_full()
            .child(header)
            .child(div().flex_1().min_h_0().child(body))
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
                    let file = entry.kind == Kind::File;
                    let shown = self
                        .files
                        .shown
                        .as_ref()
                        .is_some_and(|shown| shown.path == child);
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
                        .selected(shown)
                        .when_some(color, |row, color| row.text_color(color));
                    let row = if folder {
                        let folder = child.clone();

                        row.tooltip(format!("{name}{said}")).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.toggle_folder(folder.clone(), cx);
                            },
                        ))
                    } else if file {
                        let file = child.clone();

                        row.tooltip(format!("{name}{said}")).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.show_file(file.clone(), cx);
                            },
                        ))
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
