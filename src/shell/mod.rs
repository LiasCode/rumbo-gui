//! The window: a toolbar, the projects open in it with their sessions, and
//! the work of the session shown, or the settings. Each project has a host
//! of its own, started when the project is opened, whose runs go on while
//! another project is shown. Closing the window ends every host.

mod catalog;
mod settings;
mod sidebar;
mod toolbar;

use std::path::{Path, PathBuf};

use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _, Theme, ThemeMode, WindowExt as _,
    button::{Button, ButtonVariant, ButtonVariants as _},
    h_flex,
    notification::Notification,
    spinner::Spinner,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyBinding, ParentElement as _, PathPromptOptions, Render, Styled as _,
    Subscription, Window, actions, div, prelude::FluentBuilder as _,
};

use crate::config::{self, Config};
use crate::host::{Descriptors, Host};
use crate::preferences::{self, Preferences, Theme as Colors};
use crate::workspace::{Workspace, WorkspaceEvent};

use catalog::Catalog;
use settings::{SettingsEvent, SettingsView};

actions!(
    shell,
    [OpenPalette, OpenSettings, OpenProject, CloseSettings]
);

const KEY_CONTEXT: &str = "Shell";

/// How many of the projects rumbo holds the window with none open offers.
const RECENT_SHOWN: usize = 8;

/// The keys of the window, which work wherever the keyboard is in it.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-p", OpenPalette, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-k", OpenPalette, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-,", OpenSettings, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-o", OpenProject, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", CloseSettings, Some(settings::KEY_CONTEXT)),
    ]);
}

/// How the window reaches its first host.
pub enum Start {
    /// It starts `rumbo serve` itself, as the command line asks.
    Desktop(Config),
    /// rumbo started it, and gives it the protocol.
    Attached(Descriptors),
}

/// What the person prefers, as read at startup.
pub struct Preferred {
    pub preferences: Preferences,
    /// Where they are kept; none when there is nowhere to keep them.
    pub path: Option<PathBuf>,
    /// Why the file could not be used.
    pub problem: Option<String>,
}

type ProjectId = u64;

/// How a project the window does not have open is opened.
#[derive(Clone)]
enum Opening {
    /// In the session used last.
    Latest,
    Session(String),
    /// With a new session for the next task.
    New,
}

struct Project {
    id: ProjectId,
    /// Empty until the rumbo that started the window says it.
    dir: PathBuf,
    /// What its host is started with again.
    flags: Vec<String>,
    state: State,
}

enum State {
    Starting,
    Failed(String),
    Working {
        workspace: Entity<Workspace>,
        _subscriptions: Vec<Subscription>,
    },
}

pub struct Shell {
    /// What the command line asked for; none when rumbo started the window,
    /// which then works only in the project rumbo gave it.
    config: Option<Config>,
    projects: Vec<Project>,
    next_id: ProjectId,
    selected: Option<ProjectId>,
    /// The settings, while they are shown instead of the work.
    settings: Option<(Entity<SettingsView>, Subscription)>,
    /// Every project rumbo holds, when the window opens projects of its own.
    catalog: Option<Entity<Catalog>>,
    preferred: Preferred,
    focus_handle: FocusHandle,
    /// The main action of a screen without a workspace, which has the
    /// keyboard.
    action_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub fn new(
        start: Start,
        preferred: Preferred,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        apply_theme(preferred.preferences.theme, window, cx);

        // A theme that follows the desktop changes when the desktop does.
        let appearance = cx.observe_window_appearance(window, |this, window, cx| {
            if this.preferred.preferences.theme == Colors::System {
                apply_theme(Colors::System, window, cx);
            }
        });

        // What rumbo holds elsewhere can change while the window is away.
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.refresh_catalog(cx);
            }
        });

        let mut shell = Shell {
            config: None,
            projects: Vec::new(),
            next_id: 1,
            selected: None,
            settings: None,
            catalog: None,
            preferred,
            focus_handle: cx.focus_handle(),
            action_focus: cx.focus_handle(),
            _subscriptions: vec![appearance, activation],
        };

        match start {
            Start::Attached(descriptors) => shell.connect(descriptors, window, cx),

            Start::Desktop(config) => {
                let named = config.dir.clone();
                let flags = config.serve_flags.clone();
                let rumbo = config.rumbo.clone();

                let catalog = cx.new(|cx| {
                    Catalog::start(rumbo, catalog::home(), cx);
                    Catalog::new()
                });

                shell
                    ._subscriptions
                    .push(cx.observe(&catalog, |_, _, cx| cx.notify()));
                shell.catalog = Some(catalog);
                shell.config = Some(config);

                match named {
                    Some(dir) => shell.open_project(dir, flags, window, cx),
                    None => shell.action_focus.focus(window, cx),
                }
            }
        }

        shell
    }

    fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    fn project_mut(&mut self, id: ProjectId) -> Option<&mut Project> {
        self.projects.iter_mut().find(|project| project.id == id)
    }

    fn workspace_of(&self, id: ProjectId) -> Option<Entity<Workspace>> {
        match &self.project(id)?.state {
            State::Working { workspace, .. } => Some(workspace.clone()),
            _ => None,
        }
    }

    /// The workspace of the project shown, once its host is working.
    fn selected_workspace(&self) -> Option<Entity<Workspace>> {
        self.workspace_of(self.selected?)
    }

    /// Whether the window can open projects of its own: rumbo, when it
    /// started the window, gave it one.
    fn opens_projects(&self) -> bool {
        self.config.is_some()
    }

    fn refresh_catalog(&self, cx: &mut Context<Self>) {
        if let Some(catalog) = &self.catalog {
            catalog.update(cx, |catalog, _| catalog.refresh());
        }
    }

    /// The root rumbo lists an open project by: its folder, when that is
    /// one, or the root its sessions are listed under.
    fn root_of(&self, project: &Project, cx: &App) -> Option<String> {
        let catalog = self.catalog.as_ref()?.read(cx);
        let dir = project.dir.to_string_lossy();

        if let Some(listed) = catalog.projects().iter().find(|listed| listed.value == dir) {
            return Some(listed.value.clone());
        }

        let State::Working { workspace, .. } = &project.state else {
            return None;
        };

        workspace
            .read(cx)
            .sessions()
            .iter()
            .find_map(|session| catalog.root_of(&session.value))
            .map(str::to_string)
    }

    /// Opens a project rumbo lists, or shows it when it is open already.
    fn open_listed(
        &mut self,
        root: String,
        opening: Opening,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let open = self
            .projects
            .iter()
            .find(|project| self.root_of(project, cx).as_deref() == Some(root.as_str()))
            .map(|project| project.id);

        if let Some(id) = open {
            match opening {
                Opening::Latest => self.select(id, window, cx),
                Opening::Session(session) => self.open_session(id, session, window, cx),
                Opening::New => self.new_session(id, window, cx),
            }

            return;
        }

        let base = self
            .config
            .as_ref()
            .map(|config| config.serve_flags.clone())
            .unwrap_or_default();

        let flags = match opening {
            Opening::Latest => config::continuing_latest(&base),
            Opening::Session(session) => config::continuing(&base, &session),
            Opening::New => config::continuing(&base, ""),
        };

        self.open_project(PathBuf::from(root), flags, window, cx);
    }

    /// Shows a project, or opens it and starts its host when it is not open.
    fn open_project(
        &mut self,
        dir: PathBuf,
        flags: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(open) = self.projects.iter().find(|project| project.dir == dir) {
            let id = open.id;

            self.select(id, window, cx);
            return;
        }

        let id = self.next_id;

        self.next_id += 1;
        self.projects.push(Project {
            id,
            dir,
            flags,
            state: State::Starting,
        });

        self.selected = Some(id);
        self.start(id, window, cx);
    }

    /// Starts the host of a project in the background, and attaches its
    /// workspace once it has spoken.
    fn start(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(config), Some(project)) = (&self.config, self.project(id)) else {
            return;
        };

        let rumbo = config.rumbo.clone();
        let dir = project.dir.clone();
        let flags = project.flags.clone();

        if let Some(project) = self.project_mut(id) {
            project.state = State::Starting;
        }

        cx.spawn_in(window, async move |this, cx| {
            let started = cx
                .background_spawn(async move { Host::start(&rumbo, &dir, &flags) })
                .await;

            let _ = this.update_in(cx, |this, window, cx| {
                match started {
                    Ok(host) => this.attach(id, host, window, cx),
                    Err(message) => this.failed(id, message, window, cx),
                }

                cx.notify();
            });
        })
        .detach();

        self.set_title(window, cx);
        cx.notify();
    }

    /// Attaches to the rumbo that started the window, in the background.
    fn connect(&mut self, descriptors: Descriptors, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_id;

        self.next_id += 1;
        self.projects.push(Project {
            id,
            dir: PathBuf::new(),
            flags: Vec::new(),
            state: State::Starting,
        });
        self.selected = Some(id);

        cx.spawn_in(window, async move |this, cx| {
            let attached = cx
                .background_spawn(async move { Host::attach(descriptors) })
                .await;

            let _ = this.update_in(cx, |this, window, cx| {
                match attached {
                    Ok(host) => {
                        if let Some(project) = this.project_mut(id) {
                            project.dir = PathBuf::from(&host.initialize.dir);
                        }

                        this.attach(id, host, window, cx);
                    }

                    Err(message) => this.failed(id, message, window, cx),
                }

                cx.notify();
            });
        })
        .detach();
    }

    fn attach(&mut self, id: ProjectId, host: Host, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = cx.new(|cx| Workspace::new(host, window, cx));

        let subscriptions = vec![
            cx.subscribe_in(
                &workspace,
                window,
                move |this, _, event: &WorkspaceEvent, window, cx| {
                    this.workspace_said(id, event, window, cx)
                },
            ),
            // The sidebar and the title show what the workspace holds.
            cx.observe_in(&workspace, window, |this, _, window, cx| {
                this.set_title(window, cx);
                cx.notify();
            }),
        ];

        if let Some(project) = self.project_mut(id) {
            project.state = State::Working {
                workspace: workspace.clone(),
                _subscriptions: subscriptions,
            };
        }

        if self.selected == Some(id) && self.settings.is_none() {
            workspace.update(cx, |workspace, cx| workspace.focus(window, cx));
        }

        self.set_title(window, cx);
    }

    fn failed(&mut self, id: ProjectId, message: String, window: &mut Window, cx: &mut App) {
        if let Some(project) = self.project_mut(id) {
            project.state = State::Failed(message);
        }

        if self.selected == Some(id) {
            self.action_focus.focus(window, cx);
        }
    }

    fn workspace_said(
        &mut self,
        id: ProjectId,
        event: &WorkspaceEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            WorkspaceEvent::StartAgain { session } => {
                if let Some(project) = self.project_mut(id) {
                    project.flags = config::continuing(&project.flags, session);
                }

                self.start(id, window, cx);
            }

            WorkspaceEvent::OpenProject => self.choose_folder(window, cx),

            WorkspaceEvent::OpenSettings => self.open_settings(&OpenSettings, window, cx),

            WorkspaceEvent::CloseProject => self.close_project(id, window, cx),

            WorkspaceEvent::Answered => {
                if let Some((settings, _)) = &self.settings {
                    settings.update(cx, |settings, cx| settings.refresh(cx));
                }
            }
        }
    }

    /// Shows a project: the session its host has current, or why it has
    /// none.
    fn select(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some(id);
        self.close_settings(&CloseSettings, window, cx);

        match self.workspace_of(id) {
            Some(workspace) => workspace.update(cx, |workspace, cx| workspace.focus(window, cx)),
            None => self.action_focus.focus(window, cx),
        }

        self.set_title(window, cx);
        cx.notify();
    }

    /// Shows a session of a project, opening it in the project's host.
    fn open_session(
        &mut self,
        id: ProjectId,
        session: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select(id, window, cx);

        if let Some(workspace) = self.workspace_of(id) {
            workspace.update(cx, |workspace, cx| workspace.open_session(&session, cx));
        }
    }

    /// Makes the next task of a project start a new session.
    fn new_session(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        self.select(id, window, cx);

        if let Some(workspace) = self.workspace_of(id) {
            workspace.update(cx, |workspace, cx| workspace.new_session(cx));
        }
    }

    /// Closes a project, which ends its host. Runs working in it are
    /// cancelled, so that is asked first.
    fn close_project(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        let working = self
            .workspace_of(id)
            .map(|workspace| workspace.read(cx).working())
            .unwrap_or_default();

        if working == 0 {
            self.remove_project(id, window, cx);
            return;
        }

        let name = self
            .project(id)
            .map(|project| folder_name(&project.dir))
            .unwrap_or_default();
        let shell = cx.weak_entity();

        window.open_alert_dialog(cx, move |alert, _, _| {
            let shell = shell.clone();

            alert
                .title(format!("Close {name}?"))
                .description(match working {
                    1 => "A run works in it. Closing the project cancels it.".to_string(),
                    _ => format!("{working} runs work in it. Closing the project cancels them."),
                })
                .confirm()
                .ok_text("Close Project")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    let _ = shell.update(cx, |this, cx| this.remove_project(id, window, cx));
                    true
                })
        });
    }

    fn remove_project(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        // Started by rumbo, the window has no other project to go on in.
        if !self.opens_projects() {
            cx.quit();
            return;
        }

        // Dropping its workspace closes the connection, which ends the host.
        // Its sessions are rumbo's, listed with the others once it is closed.
        self.projects.retain(|project| project.id != id);
        self.refresh_catalog(cx);

        if self.selected == Some(id) {
            match self.projects.first().map(|project| project.id) {
                Some(next) => self.select(next, window, cx),

                None => {
                    self.selected = None;
                    self.action_focus.focus(window, cx);
                }
            }
        }

        self.set_title(window, cx);
        cx.notify();
    }

    /// Asks for a folder, and opens it as a project.
    fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.opens_projects() {
            return;
        }

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
                    // What the command line asked for once is not asked
                    // again in another project; how the person works is.
                    let flags = this
                        .config
                        .as_ref()
                        .map(|config| config::continuing(&config.serve_flags, ""))
                        .unwrap_or_default();

                    this.open_project(paths[0].clone(), flags, window, cx);
                }

                Ok(Err(error)) => {
                    window.push_notification(
                        Notification::error(format!("The folder could not be chosen: {error}")),
                        cx,
                    );
                }

                // The person closed the dialog.
                _ => {}
            });
        })
        .detach();
    }

    fn open_project_action(
        &mut self,
        _: &OpenProject,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.choose_folder(window, cx);
    }

    /// Opens the commands of the project shown.
    fn open_palette(&mut self, _: &OpenPalette, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.selected_workspace() else {
            return;
        };

        self.close_settings(&CloseSettings, window, cx);
        workspace.update(cx, |workspace, cx| workspace.open_palette(window, cx));
    }

    /// Shows the settings in place of the work.
    fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.is_some() {
            return;
        }

        let workspace = self.selected_workspace();
        let theme = self.preferred.preferences.theme;
        let path = self.preferred.path.clone();
        let problem = self.preferred.problem.clone();

        let settings = cx.new(|cx| SettingsView::new(workspace, theme, path, problem, cx));
        let subscription = cx.subscribe_in(
            &settings,
            window,
            |this, _, event: &SettingsEvent, window, cx| match event {
                SettingsEvent::Theme(theme) => this.set_theme(*theme, window, cx),
            },
        );

        settings.update(cx, |settings, cx| settings.focus(window, cx));
        self.settings = Some((settings, subscription));
        self.set_title(window, cx);
        cx.notify();
    }

    /// Goes back from the settings to the work.
    fn close_settings(&mut self, _: &CloseSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.take().is_none() {
            return;
        }

        match self.selected_workspace() {
            Some(workspace) => workspace.update(cx, |workspace, cx| workspace.focus(window, cx)),
            None => self.action_focus.focus(window, cx),
        }

        self.set_title(window, cx);
        cx.notify();
    }

    /// Changes the theme, and keeps it for the next start.
    fn set_theme(&mut self, theme: Colors, window: &mut Window, cx: &mut Context<Self>) {
        self.preferred.preferences.theme = theme;
        apply_theme(theme, window, cx);

        if let Some(path) = &self.preferred.path {
            self.preferred.problem = preferences::save(&self.preferred.preferences, path).err();
        }

        if let Some(problem) = &self.preferred.problem {
            window.push_notification(Notification::error(problem.clone()), cx);
        }

        cx.notify();
    }

    /// The window's title, which is how a person tells windows apart: the
    /// session and the project shown.
    fn title(&self, cx: &App) -> String {
        if self.settings.is_some() {
            return "Settings — rumbo".into();
        }

        let Some(project) = self.selected.and_then(|id| self.project(id)) else {
            return "rumbo".into();
        };

        let session = self
            .workspace_of(project.id)
            .and_then(|workspace| workspace.read(cx).session_title());
        let name = folder_name(&project.dir);

        match session {
            Some(session) => format!("{session} — {name}"),
            None if project.dir.as_os_str().is_empty() => "rumbo".into(),
            None => format!("{name} — rumbo"),
        }
    }

    fn set_title(&self, window: &mut Window, cx: &App) {
        window.set_window_title(&self.title(cx));
    }

    fn render_main(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some((settings, _)) = &self.settings {
            return settings.clone().into_any_element();
        }

        let Some(project) = self.selected.and_then(|id| self.project(id)) else {
            return self.render_empty(cx);
        };

        match &project.state {
            State::Working { workspace, .. } => workspace.clone().into_any_element(),

            State::Starting if project.dir.as_os_str().is_empty() => centered()
                .child(Spinner::new().large())
                .child(muted("Waiting for rumbo", cx))
                .into_any_element(),

            State::Starting => centered()
                .child(Spinner::new().large())
                .child(muted(
                    &format!("Starting rumbo in {}", project.dir.display()),
                    cx,
                ))
                .into_any_element(),

            State::Failed(message) => self.render_failed(project, message, cx),
        }
    }

    /// The window with no project open yet: a way to open one, and the
    /// projects rumbo holds, most recently used first, which open in the
    /// session used last.
    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        let listed: Vec<gpui_kit::component::button::Button> = self
            .catalog
            .as_ref()
            .map(|catalog| catalog.read(cx).projects().to_vec())
            .unwrap_or_default()
            .into_iter()
            .take(RECENT_SHOWN)
            .map(|project| {
                let root = project.value.clone();
                let label = if project.label.is_empty() {
                    project.value.clone()
                } else {
                    project.label.clone()
                };

                Button::new(gpui_kit::ElementId::Name(format!("recent-{root}").into()))
                    .ghost()
                    .w_full()
                    .icon(IconName::FolderClosed)
                    .label(format!("{label} — {}", project.description))
                    .tooltip(root.clone())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_listed(root.clone(), Opening::Latest, window, cx)
                    }))
            })
            .collect();

        centered()
            .child(div().text_xl().child("rumbo"))
            .child(muted("Open the folder of a project to work in it.", cx))
            .child(
                Button::new("open-folder")
                    .track_focus(&self.action_focus)
                    .primary()
                    .icon(IconName::FolderOpen)
                    .label("Open Project…")
                    .on_click(cx.listener(|this, _, window, cx| this.choose_folder(window, cx))),
            )
            .when(!listed.is_empty(), |screen| {
                screen.child(
                    v_flex()
                        .w(gpui_kit::px(440.))
                        .pt_4()
                        .gap_1()
                        .child(muted("Recent", cx))
                        .children(listed),
                )
            })
            .into_any_element()
    }

    fn render_failed(
        &self,
        project: &Project,
        message: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = project.id;

        // Started by rumbo, there is no host of this window's to try again.
        let heading = if self.opens_projects() {
            format!("rumbo could not start in {}", project.dir.display())
        } else {
            "rumbo-gui could not attach to rumbo".to_string()
        };

        centered()
            .child(div().text_lg().child(heading))
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
            .when(self.opens_projects(), |screen| {
                screen.child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("try-again")
                                .track_focus(&self.action_focus)
                                .primary()
                                .label("Try Again")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.start(id, window, cx)
                                })),
                        )
                        .child(
                            Button::new("close-project")
                                .outline()
                                .label("Close Project")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.close_project(id, window, cx)
                                })),
                        ),
                )
            })
            .into_any_element()
    }
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_shown = self.settings.is_some();

        v_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_palette))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::open_project_action))
            .on_action(cx.listener(Self::close_settings))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_toolbar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .when(!settings_shown, |body| body.child(self.render_sidebar(cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(self.render_main(cx)),
                    ),
            )
    }
}

/// Puts a theme in force. One that follows the desktop asks the window,
/// which knows its appearance better than the app on Linux.
fn apply_theme(theme: Colors, window: &mut Window, cx: &mut App) {
    match theme {
        Colors::System => Theme::sync_system_appearance(Some(window), cx),
        Colors::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        Colors::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
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

fn muted(text: &str, cx: &App) -> gpui_kit::Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

/// A folder's name, which is how a person tells projects apart; rumbo's,
/// until the rumbo that started the window says which folder it works in.
fn folder_name(dir: &Path) -> String {
    if dir.as_os_str().is_empty() {
        return "rumbo".into();
    }

    dir.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.display().to_string())
}
