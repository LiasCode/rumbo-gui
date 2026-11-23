//! The History tab of the project's Git panel: the commits of the branch
//! checked out, or of another branch chosen, local or remote, as a list;
//! or those of every branch as a graph, as `git log --oneline --decorate
//! --all --graph` draws it. Choosing a branch checks nothing out, and
//! nothing asks a remote; the window only reads.

use std::ops::Range;
use std::path::PathBuf;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, HighlightStyle, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, StyledText, Subscription, Window, div, hsla,
    prelude::FluentBuilder as _, rgb,
};
use ui::{
    ActiveTheme as _, Button, Scrollbar, Select, SelectEvent, SelectItem, SelectState,
    Sizable as _, Spinner, Switch, ThemeMode, h_flex, rems_from_px, v_flex,
};

use super::Workspace;
use super::panel::{Read, centered, message};
use crate::git::graph::{self, Color, Label};
use crate::git::history::{self, HEAD, PAGE};

/// The colors Git gives a commit's hash and what points at it, as a
/// terminal's: the hash and the parentheses yellow, and, bright, `HEAD`
/// cyan, a branch green, a remote's red, a tag yellow, and any other
/// reference, such as the stash, magenta.
const HASH_COLOR: Color = Color::Indexed(3);
const HEAD_COLOR: Color = Color::Indexed(14);
const BRANCH_COLOR: Color = Color::Indexed(10);
const REMOTE_COLOR: Color = Color::Indexed(9);
const TAG_COLOR: Color = Color::Indexed(11);
const OTHER_COLOR: Color = Color::Indexed(13);

pub(super) struct GitHistory {
    /// The branch whose commits the list shows; none chosen follows `HEAD`.
    branch: Entity<SelectState>,
    _branch_chosen: Subscription,
    /// The commits of every branch show as a graph, not the list.
    showing_graph: bool,
    read: Option<Read<history::History>>,
    graph: Option<Read<graph::Graph>>,
    /// Tells a reading from one asked for after it.
    reading: u64,
    /// How many commits are read.
    limit: usize,
    scroll: ScrollHandle,
}

impl GitHistory {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        // The branches are known once the history is first read.
        let branch = cx.new(|cx| {
            SelectState::new(Vec::new(), None, window, cx)
                .placeholder("Current branch")
                .searchable(true)
        });

        let branch_chosen = cx.subscribe(&branch, |this, _, _: &SelectEvent, cx| {
            let history = &mut this.panels.history;

            history.limit = PAGE;
            history.scroll.set_offset(Default::default());
            history.read = Some(Read::Reading);

            this.read_git_history(cx);
        });

        GitHistory {
            branch,
            _branch_chosen: branch_chosen,
            showing_graph: false,
            read: None,
            graph: None,
            reading: 0,
            limit: PAGE,
            scroll: ScrollHandle::new(),
        }
    }
}

impl Workspace {
    /// Reads, on a background thread, the branches and the commits of the
    /// one chosen, or the graph of every branch. What was shown stays until
    /// the new reading replaces it.
    pub(super) fn read_git_history(&mut self, cx: &mut Context<Self>) {
        let dir = PathBuf::from(&self.dir);
        let history = &mut self.panels.history;
        let limit = history.limit;

        history.reading += 1;

        let reading = history.reading;

        if history.showing_graph {
            if history.graph.is_none() {
                history.graph = Some(Read::Reading);
            }

            cx.spawn(async move |this, cx| {
                let read = cx
                    .background_spawn(async move { graph::read(&dir, limit) })
                    .await;

                let _ = this.update(cx, |this, cx| this.git_graph_read(reading, read, cx));
            })
            .detach();
        } else {
            let branch = history
                .branch
                .read(cx)
                .selected_value()
                .map_or_else(|| HEAD.to_string(), ToString::to_string);

            if history.read.is_none() {
                history.read = Some(Read::Reading);
            }

            cx.spawn(async move |this, cx| {
                let read = cx
                    .background_spawn(async move { history::read(&dir, &branch, limit) })
                    .await;

                let _ = this.update(cx, |this, cx| this.git_history_read(reading, read, cx));
            })
            .detach();
        }

        cx.notify();
    }

    fn git_history_read(
        &mut self,
        reading: u64,
        read: Result<history::History, String>,
        cx: &mut Context<Self>,
    ) {
        let history = &mut self.panels.history;

        if reading != history.reading {
            return;
        }

        history.read = Some(match read {
            Ok(read) => {
                let head = match &read.current {
                    Some(name) => format!("Current branch: {name}"),
                    None => "Detached HEAD".into(),
                };

                let mut items = vec![SelectItem::new(HEAD).label(head)];

                items.extend(read.branches.iter().map(|branch| {
                    SelectItem::new(branch.reference.clone())
                        .label(branch.name.clone())
                        .description(match branch.remote {
                            true => "Remote",
                            false => "Local",
                        })
                }));

                history.branch.update(cx, |branch, cx| {
                    branch.set_items(items, cx);
                    branch.set_selected_value(&read.selected, cx);
                });

                Read::Here(read)
            }

            Err(problem) => Read::Failed(problem),
        });

        cx.notify();
    }

    fn git_graph_read(
        &mut self,
        reading: u64,
        read: Result<graph::Graph, String>,
        cx: &mut Context<Self>,
    ) {
        let history = &mut self.panels.history;

        if reading != history.reading {
            return;
        }

        history.graph = Some(match read {
            Ok(graph) => Read::Here(graph),
            Err(problem) => Read::Failed(problem),
        });

        cx.notify();
    }

    /// Shows the commits of every branch as a graph, or those of the branch
    /// chosen as a list, from the newest.
    fn show_git_graph(&mut self, shown: bool, cx: &mut Context<Self>) {
        let history = &mut self.panels.history;

        history.showing_graph = shown;
        history.limit = PAGE;
        history.scroll.set_offset(Default::default());

        self.read_git_history(cx);
    }

    pub(super) fn render_git_history(&self, cx: &mut Context<Self>) -> AnyElement {
        let history = &self.panels.history;
        let colors = cx.theme().colors;
        let muted = colors.muted_foreground;

        // The graph shows every branch, so no branch is chosen for it.
        let shown = match history.showing_graph {
            true => div()
                .flex_1()
                .text_color(muted)
                .child("All branches")
                .into_any_element(),
            false => h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .child("Branch")
                .child(Select::new(&history.branch).small().flex_1().min_w_0())
                .into_any_element(),
        };

        let switch = Switch::new("git-graph")
            .small()
            .label("Graph")
            .checked(history.showing_graph)
            .on_change(cx.listener(|this, shown: &bool, _, cx| this.show_git_graph(*shown, cx)));

        let body = match history.showing_graph {
            true => self.render_git_graph(cx),
            false => self.render_git_commits(cx),
        };

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .flex_shrink_0()
                    .h(rems_from_px(48.))
                    .gap_3()
                    .px_3()
                    .text_sm()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(shown)
                    .child(switch),
            )
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    /// The commits of the branch chosen, one under another.
    fn render_git_commits(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors;
        let muted = colors.muted_foreground;

        let read = match &self.panels.history.read {
            Some(Read::Here(read)) => read,
            other => return unread(other, cx),
        };

        if read.commits.is_empty() {
            return no_commits(cx);
        }

        let commits = read.commits.iter().map(|commit| {
            v_flex()
                .flex_shrink_0()
                .gap_1()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(colors.border)
                .child(div().text_sm().truncate().child(commit.subject.clone()))
                .child(
                    h_flex()
                        .gap_2()
                        .text_xs()
                        .text_color(muted)
                        .child(commit.hash.clone())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(commit.author.clone()),
                        )
                        .child(commit.date.clone()),
                )
                .into_any_element()
        });

        self.render_commit_scroll(commits.collect(), read.more, cx)
    }

    /// The commits of every branch as Git draws them, a line of text each,
    /// so the graph's lines meet from one to the next.
    fn render_git_graph(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let mode = theme.mode;
        let mono = theme.mono_family.clone();
        let foreground = theme.colors.foreground;

        let read = match &self.panels.history.graph {
            Some(Read::Here(read)) => read,
            other => return unread(other, cx),
        };

        if read.lines.is_empty() {
            return no_commits(cx);
        }

        let lines = read.lines.iter().map(|line| {
            let (text, highlights) = graph_line(line, mode);

            // As tall as a terminal's line, so the graph's lines nearly meet.
            div()
                .flex_shrink_0()
                .px_3()
                .h(rems_from_px(16.))
                .line_height(rems_from_px(16.))
                .font_family(mono.clone())
                .text_xs()
                .text_color(foreground)
                .truncate()
                .child(StyledText::new(text).with_highlights(highlights))
                .into_any_element()
        });

        self.render_commit_scroll(lines.collect(), read.more, cx)
    }

    /// Rows of commits that scroll, with a button for older ones when there
    /// are more.
    fn render_commit_scroll(
        &self,
        rows: Vec<AnyElement>,
        more: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let scroll = &self.panels.history.scroll;

        let list = v_flex()
            .id("git-commits")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(scroll)
            .children(rows)
            .when(more, |list| {
                list.child(
                    h_flex().flex_shrink_0().justify_center().p_2().child(
                        Button::new("more-commits")
                            .ghost()
                            .small()
                            .label("Load More Commits")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.panels.history.limit += PAGE;
                                this.read_git_history(cx);
                            })),
                    ),
                )
            });

        div()
            .relative()
            .size_full()
            .child(list)
            .child(Scrollbar::vertical("git-commits-scrollbar", scroll.clone()))
            .into_any_element()
    }
}

/// What shows while the commits are read, or why they could not be.
fn unread<T>(read: &Option<Read<T>>, cx: &App) -> AnyElement {
    match read {
        Some(Read::Failed(problem)) => message(problem.clone(), cx),
        _ => centered().child(Spinner::new()).into_any_element(),
    }
}

fn no_commits(cx: &App) -> AnyElement {
    message("No commits yet.", cx)
}

/// A line of the graph as Git writes it, with the colors it gives each
/// part: the graph, then the commit's hash, what points at it in
/// parentheses, and its subject.
fn graph_line(
    line: &graph::Line,
    mode: ThemeMode,
) -> (String, Vec<(Range<usize>, HighlightStyle)>) {
    let mut text = String::new();
    let mut highlights = Vec::new();

    let mut write = |part: &str, color: Option<Color>| {
        let start = text.len();

        text.push_str(part);

        if let Some(color) = color {
            highlights.push((
                start..text.len(),
                HighlightStyle {
                    color: Some(terminal(color, mode)),
                    ..HighlightStyle::default()
                },
            ));
        }
    };

    for stroke in &line.graph {
        write(&stroke.text, stroke.color);
    }

    if let Some(commit) = &line.commit {
        write(&commit.hash, Some(HASH_COLOR));

        if !commit.labels.is_empty() {
            write(" (", Some(HASH_COLOR));

            for (index, label) in commit.labels.iter().enumerate() {
                if index > 0 {
                    write(", ", Some(HASH_COLOR));
                }

                match label {
                    Label::Head(branch) => {
                        write("HEAD", Some(HEAD_COLOR));

                        if let Some(branch) = branch {
                            write(" -> ", Some(HASH_COLOR));
                            write(branch, Some(BRANCH_COLOR));
                        }
                    }
                    Label::Branch(name) => write(name, Some(BRANCH_COLOR)),
                    Label::Remote(name) => write(name, Some(REMOTE_COLOR)),
                    Label::Tag(name) => write(&format!("tag: {name}"), Some(TAG_COLOR)),
                    Label::Other(name) => write(name, Some(OTHER_COLOR)),
                }
            }

            write(")", Some(HASH_COLOR));
        }

        write(" ", None);
        write(&commit.subject, None);
    }

    (text, highlights)
}

/// A terminal's color, as it reads on the theme's background: the usual
/// six and their bright ones darker on a light background and lighter on a
/// dark one, black a gray that reads on either, and white the shade of the
/// theme's text.
fn terminal(color: Color, mode: ThemeMode) -> Hsla {
    let index = match color {
        Color::Rgb(red, green, blue) => {
            return rgb(u32::from_be_bytes([0, red, green, blue])).into();
        }
        Color::Indexed(index) => index,
    };

    let lightness = |bright: bool| match (mode, bright) {
        (ThemeMode::Light, false) => 0.38,
        (ThemeMode::Light, true) => 0.46,
        (ThemeMode::Dark, false) => 0.62,
        (ThemeMode::Dark, true) => 0.72,
    };

    match index {
        0..=15 => {
            let bright = index >= 8;

            // Red, green, yellow, blue, magenta, and cyan, by their hue.
            let hue = match index % 8 {
                1 => 0.,
                2 => 130.,
                3 => 42.,
                4 => 215.,
                5 => 300.,
                6 => 185.,
                0 => return hsla(0., 0., 0.5, 1.),
                _ => {
                    return match mode {
                        ThemeMode::Light => hsla(0., 0., 0.25, 1.),
                        ThemeMode::Dark => hsla(0., 0., 0.85, 1.),
                    };
                }
            };

            hsla(hue / 360., 0.7, lightness(bright), 1.)
        }

        // A cube of six levels of red, green, and blue.
        16..=231 => {
            let level = |step: u8| match step {
                0 => 0,
                step => 55 + step * 40,
            };
            let cube = index - 16;

            rgb(u32::from_be_bytes([
                0,
                level(cube / 36),
                level(cube / 6 % 6),
                level(cube % 6),
            ]))
            .into()
        }

        // Twenty-four grays, from nearly black to nearly white.
        _ => {
            let gray = 8 + (index - 232) * 10;

            rgb(u32::from_be_bytes([0, gray, gray, gray])).into()
        }
    }
}
