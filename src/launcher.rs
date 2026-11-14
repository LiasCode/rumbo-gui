//! The window before and around a host: choosing the folder, starting
//! `rumbo serve` in it, and saying why it did not start.

use std::path::{Path, PathBuf};

use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    spinner::Spinner,
    v_flex,
};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement as _,
    PathPromptOptions, Render, Styled as _, Subscription, Window, div,
};

use crate::config::{self, Config};
use crate::host::Host;
use crate::workspace::{Workspace, WorkspaceEvent};

enum Screen {
    /// No folder was named yet. The error is why the last choice failed.
    Choose(Option<String>),
    Starting(PathBuf),
    Failed {
        dir: PathBuf,
        message: String,
    },
    Working {
        workspace: Entity<Workspace>,
        /// Keeps the window listening to what the workspace asks.
        _subscription: Subscription,
    },
}

pub struct Launcher {
    config: Config,
    screen: Screen,
    /// The main action of the screen shown, which has the keyboard.
    action_focus: FocusHandle,
}

impl Launcher {
    pub fn new(config: Config, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dir = config.dir.clone();

        let mut launcher = Launcher {
            config,
            screen: Screen::Choose(None),
            action_focus: cx.focus_handle(),
        };

        match dir {
            Some(dir) => {
                let flags = launcher.config.serve_flags.clone();

                launcher.start(dir, flags, window, cx);
            }

            None => launcher.action_focus.focus(window, cx),
        }

        launcher
    }

    /// Starts a host with `flags` in `dir` in the background, and attaches
    /// the window to it once it has spoken.
    fn start(
        &mut self,
        dir: PathBuf,
        flags: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.screen = Screen::Starting(dir.clone());
        window.set_window_title(&title(&dir));

        let rumbo = self.config.rumbo.clone();

        cx.spawn_in(window, async move |this, cx| {
            let starting = dir.clone();
            let started = cx
                .background_spawn(async move { Host::start(&rumbo, &starting, &flags) })
                .await;

            let _ = this.update_in(cx, |this, window, cx| {
                this.screen = match started {
                    Ok(host) => this.attach(host, dir, window, cx),

                    Err(message) => {
                        this.action_focus.focus(window, cx);

                        Screen::Failed { dir, message }
                    }
                };

                cx.notify();
            });
        })
        .detach();

        cx.notify();
    }

    fn attach(
        &mut self,
        host: Host,
        dir: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Screen {
        let workspace = cx.new(|cx| Workspace::new(host, window, cx));

        let subscription = cx.subscribe_in(
            &workspace,
            window,
            move |this, _, event: &WorkspaceEvent, window, cx| match event {
                WorkspaceEvent::StartAgain { session } => {
                    let flags = config::continuing(&this.config.serve_flags, session);

                    this.start(dir.clone(), flags, window, cx);
                }
            },
        );

        Screen::Working {
            workspace,
            _subscription: subscription,
        }
    }

    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open".into()),
        });

        cx.spawn_in(window, async move |this, cx| {
            let chosen = chosen.await;

            let _ = this.update_in(cx, |this, window, cx| match chosen {
                Ok(Ok(Some(paths))) if !paths.is_empty() => {
                    let flags = this.config.serve_flags.clone();

                    this.start(paths[0].clone(), flags, window, cx);
                }

                Ok(Err(error)) => {
                    this.screen =
                        Screen::Choose(Some(format!("The folder could not be chosen: {error}")));
                    this.action_focus.focus(window, cx);
                    cx.notify();
                }

                // The person closed the dialog.
                _ => {}
            });
        })
        .detach();
    }

    fn render_choose(&self, error: Option<&str>, cx: &mut Context<Self>) -> AnyElement {
        centered()
            .child(div().text_xl().child("rumbo"))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Open the folder of a project to work in it."),
            )
            .child(
                Button::new("open-folder")
                    .track_focus(&self.action_focus)
                    .primary()
                    .icon(IconName::FolderOpen)
                    .label("Open Folder…")
                    .on_click(cx.listener(|this, _, window, cx| this.choose(window, cx))),
            )
            .children(error.map(|error| {
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.to_string())
            }))
            .into_any_element()
    }

    fn render_failed(&self, dir: &Path, message: &str, cx: &mut Context<Self>) -> AnyElement {
        let again = dir.to_path_buf();

        centered()
            .child(
                div()
                    .text_lg()
                    .child(format!("rumbo could not start in {}", dir.display())),
            )
            .child(
                div()
                    .max_w_full()
                    .p_3()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_sm()
                    .child(message.to_string()),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("try-again")
                            .track_focus(&self.action_focus)
                            .primary()
                            .label("Try Again")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let flags = this.config.serve_flags.clone();

                                this.start(again.clone(), flags, window, cx)
                            })),
                    )
                    .child(
                        Button::new("open-another")
                            .outline()
                            .label("Open Another Folder…")
                            .on_click(cx.listener(|this, _, window, cx| this.choose(window, cx))),
                    ),
            )
            .into_any_element()
    }
}

impl Render for Launcher {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let screen = match &self.screen {
            Screen::Working { workspace, .. } => workspace.clone().into_any_element(),

            Screen::Choose(error) => self.render_choose(error.as_deref(), cx),

            Screen::Starting(dir) => centered()
                .child(Spinner::new().large())
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("Starting rumbo in {}", dir.display())),
                )
                .into_any_element(),

            Screen::Failed { dir, message } => {
                let (dir, message) = (dir.clone(), message.clone());

                self.render_failed(&dir, &message, cx)
            }
        };

        div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(screen)
    }
}

fn centered() -> gpui_kit::Div {
    v_flex()
        .size_full()
        .p_8()
        .gap_3()
        .items_center()
        .justify_center()
}

/// The window's title: the folder's name, which is how a person tells
/// windows apart.
fn title(dir: &Path) -> String {
    let name = dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.display().to_string());

    format!("{name} — rumbo")
}
