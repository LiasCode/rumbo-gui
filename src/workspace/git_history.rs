//! The History tab of the project's Git panel: the commits of the branch
//! checked out, or of another branch chosen, local or remote. Choosing one
//! checks nothing out, and nothing asks a remote; the window only reads.

use std::path::PathBuf;

use gpui::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, ScrollHandle, StatefulInteractiveElement as _, Styled as _, Subscription,
    Window, div, prelude::FluentBuilder as _,
};
use ui::{
    ActiveTheme as _, Button, Scrollbar, Select, SelectEvent, SelectItem, SelectState,
    Sizable as _, Spinner, h_flex, v_flex,
};

use super::Workspace;
use super::panel::{Read, centered};
use crate::git::history::{self, HEAD, PAGE};

pub(super) struct GitHistory {
    /// The branch whose commits show; none chosen follows `HEAD`.
    branch: Entity<SelectState>,
    _branch_chosen: Subscription,
    read: Option<Read<history::History>>,
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
            read: None,
            reading: 0,
            limit: PAGE,
            scroll: ScrollHandle::new(),
        }
    }
}

impl Workspace {
    /// Reads the branches and the commits of the one chosen on a background
    /// thread. What was shown stays until the new reading replaces it.
    pub(super) fn read_git_history(&mut self, cx: &mut Context<Self>) {
        let dir = PathBuf::from(&self.dir);
        let history = &mut self.panels.history;
        let branch = history
            .branch
            .read(cx)
            .selected_value()
            .map_or_else(|| HEAD.to_string(), ToString::to_string);
        let limit = history.limit;

        history.reading += 1;

        let reading = history.reading;

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

    pub(super) fn render_git_history(&self, cx: &mut Context<Self>) -> AnyElement {
        let history = &self.panels.history;
        let colors = cx.theme().colors;
        let muted = colors.muted_foreground;

        let body = match &history.read {
            None | Some(Read::Reading) => centered().child(Spinner::new()).into_any_element(),

            Some(Read::Failed(problem)) => centered()
                .child(div().text_sm().text_color(muted).child(problem.clone()))
                .into_any_element(),

            Some(Read::Here(read)) if read.commits.is_empty() => centered()
                .child(div().text_sm().text_color(muted).child("No commits yet."))
                .into_any_element(),

            Some(Read::Here(read)) => {
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
                });

                let list = v_flex()
                    .id("git-commits")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&history.scroll)
                    .children(commits)
                    .when(read.more, |list| {
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
                    .child(Scrollbar::vertical(
                        "git-commits-scrollbar",
                        history.scroll.clone(),
                    ))
                    .into_any_element()
            }
        };

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .border_b_1()
                    .border_color(colors.border)
                    .child("Branch")
                    .child(Select::new(&history.branch).small().flex_1().min_w_0()),
            )
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}
