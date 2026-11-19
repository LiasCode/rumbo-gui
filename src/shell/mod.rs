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
    input::{InputEvent, InputState},
    notification::Notification,
    spinner::Spinner,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyBinding, MouseButton, ParentElement as _, PathPromptOptions, Render,
    Styled as _, Subscription, Window, actions, div, prelude::FluentBuilder as _,
};

use crate::config::{self, Config};
use crate::editor;
use crate::environment::Environment;
use crate::host::{Descriptors, Host};
use crate::preferences::{self, Preferences, Theme as Colors};
use crate::workspace::{Workspace, WorkspaceEvent};

use catalog::Catalog;
use settings::{SettingsEvent, SettingsView};

actions!(
    shell,
    [
        OpenPalette,
        OpenSettings,
        OpenProject,
        CloseSettings,
        NewSession,
        ToggleSidebar,
        OpenInEditor,
        FocusSearch,
        Jump1,
        Jump2,
        Jump3,
        Jump4,
        Jump5,
        Jump6,
        Jump7,
        Jump8,
        Jump9
    ]
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
        KeyBinding::new("ctrl-n", NewSession, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-b", ToggleSidebar, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-shift-e", OpenInEditor, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-f", FocusSearch, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-1", Jump1, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-2", Jump2, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-3", Jump3, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-4", Jump4, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-5", Jump5, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-6", Jump6, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-7", Jump7, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-8", Jump8, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-9", Jump9, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", CloseSettings, Some(settings::KEY_CONTEXT)),
    ]);
}

/// The window's keys, for the person to read: the keys, and what they do.
/// They are bound in `bind_keys` here and in the workspace's.
pub const SHORTCUTS: [(&str, &str); 15] = [
    ("Enter", "Send the task, or tell the run in progress"),
    ("Shift-Enter", "Start a new line"),
    ("Up / Down", "Bring back what was asked in the project"),
    ("Escape", "Cancel the run, or give up a provider's function"),
    ("Ctrl-K, Ctrl-P", "Commands, sessions, and projects"),
    ("Ctrl-N", "New session"),
    (
        "Ctrl-1 … Ctrl-9",
        "Show the project's sessions, most recent first",
    ),
    ("Ctrl-B", "Show or put away the sidebar"),
    ("Ctrl-Shift-G", "Changes in the project's folder"),
    ("Ctrl-Alt-B", "The session's details"),
    ("Ctrl-Shift-E", "Open the project in the editor"),
    ("Ctrl-O", "Open a project"),
    ("Ctrl-F", "Search the sessions"),
    ("Ctrl-,", "Settings"),
    ("Escape, in the settings", "Back to the work"),
];

/// How the window reaches its first host.
pub enum Start {
    /// It starts `rumbo serve` itself, as the command line asks, in this
    /// environment.
    Desktop(Config, Environment),
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
    /// What the hosts the window starts run in; the window's own when rumbo
    /// started it, as it then starts none.
    environment: Environment,
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
    /// What the sidebar is narrowed to.
    search: Entity<InputState>,
    /// The sidebar was dragged to another width, kept once the drag ends.
    resized: bool,
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

        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search sessions"));
        let searching = cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });

        let mut shell = Shell {
            config: None,
            environment: Environment::inherited(),
            projects: Vec::new(),
            next_id: 1,
            selected: None,
            settings: None,
            catalog: None,
            preferred,
            focus_handle: cx.focus_handle(),
            action_focus: cx.focus_handle(),
            search,
            resized: false,
            _subscriptions: vec![appearance, activation, searching],
        };

        match start {
            Start::Attached(descriptors) => shell.connect(descriptors, window, cx),

            Start::Desktop(config, environment) => {
                let named = config.dir.clone();
                let flags = config.serve_flags.clone();
                let rumbo = config.rumbo.clone();
                let listing = environment.clone();

                let catalog = cx.new(|cx| {
                    Catalog::start(rumbo, listing, catalog::home(), cx);
                    Catalog::new()
                });

                shell
                    ._subscriptions
                    .push(cx.observe(&catalog, |_, _, cx| cx.notify()));
                shell.catalog = Some(catalog);
                shell.config = Some(config);
                shell.environment = environment;

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
        let environment = self.environment.clone();

        if let Some(project) = self.project_mut(id) {
            project.state = State::Starting;
        }

        cx.spawn_in(window, async move |this, cx| {
            let started = cx
                .background_spawn(async move { Host::start(&rumbo, &dir, &flags, &environment) })
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
        let width = self.preferred.preferences.width.pixels();
        let workspace = cx.new(|cx| {
            let mut workspace = Workspace::new(host, window, cx);

            workspace.set_width(width, cx);
            workspace
        });

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

            WorkspaceEvent::OpenInEditor { path } => open_with_editor(
                &self.preferred.preferences.editor,
                &self.environment,
                Path::new(path),
                window,
                cx,
            ),

            WorkspaceEvent::EndedElsewhere {
                session,
                title,
                status,
            } => {
                let _ = session;

                self.ended_elsewhere(id, title, status, window, cx)
            }
        }
    }

    /// Says that a run ended in a session that is not shown, unless the
    /// person asked for quiet. The sidebar marks the session until it is
    /// shown.
    fn ended_elsewhere(
        &mut self,
        id: ProjectId,
        title: &str,
        status: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preferred.preferences.quiet {
            return;
        }

        let project = self
            .project(id)
            .map(|project| folder_name(&project.dir))
            .unwrap_or_default();
        let said = match status {
            crate::protocol::RUN_DONE => "finished",
            crate::protocol::RUN_CANCELLED => "was cancelled",
            _ => "failed",
        };
        let note = Notification::new()
            .title(format!("A run {said} in {project}"))
            .message(title.to_string());

        let note = match status {
            crate::protocol::RUN_DONE => {
                note.with_type(gpui_kit::component::notification::NotificationType::Success)
            }
            crate::protocol::RUN_CANCELLED => note,
            _ => note.with_type(gpui_kit::component::notification::NotificationType::Error),
        };

        window.push_notification(note, cx);
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

    /// Makes the next task of the project shown start a new session.
    fn new_session_shown(&mut self, _: &NewSession, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.selected {
            self.new_session(id, window, cx);
        }
    }

    /// Asks for a new title for a session of a project, which shows it.
    fn rename_session(
        &mut self,
        id: ProjectId,
        session: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select(id, window, cx);

        if let Some(workspace) = self.workspace_of(id) {
            workspace.update(cx, |workspace, cx| workspace.rename(&session, window, cx));
        }
    }

    /// Deletes a session of a project, once the person confirms it.
    fn delete_session(
        &mut self,
        id: ProjectId,
        session: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(workspace) = self.workspace_of(id) {
            workspace.update(cx, |workspace, cx| workspace.delete(&session, window, cx));
        }
    }

    /// Opens a project's folder in the editor the person chose, or in one
    /// found on the PATH the hosts get.
    fn open_in_editor(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.project(id) else {
            return;
        };

        let dir = match self.workspace_of(id) {
            Some(workspace) => PathBuf::from(workspace.read(cx).dir()),
            None => project.dir.clone(),
        };

        open_with_editor(
            &self.preferred.preferences.editor,
            &self.environment,
            &dir,
            window,
            cx,
        );
    }

    /// Puts the sidebar away, or brings it back.
    fn toggle_sidebar(&mut self, _: &ToggleSidebar, window: &mut Window, cx: &mut Context<Self>) {
        self.preferred.preferences.sidebar_hidden = !self.preferred.preferences.sidebar_hidden;
        self.save_preferences(window, cx);
    }

    /// Keeps the preferences for the next start, saying when they could not
    /// be kept.
    fn save_preferences(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.resized = false;

        if let Some(path) = &self.preferred.path {
            self.preferred.problem = preferences::save(&self.preferred.preferences, path).err();
        }

        if let Some(problem) = &self.preferred.problem {
            window.push_notification(Notification::error(problem.clone()), cx);
        }

        cx.notify();
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
        let preferences = self.preferred.preferences.clone();
        let path = self.preferred.path.clone();
        let problem = self.preferred.problem.clone();

        let settings =
            cx.new(|cx| SettingsView::new(workspace, preferences, path, problem, window, cx));
        let subscription = cx.subscribe_in(
            &settings,
            window,
            |this, _, event: &SettingsEvent, window, cx| match event {
                SettingsEvent::Preferences(preferences) => {
                    this.set_preferences(preferences.clone(), window, cx)
                }
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

    /// Takes in what the person changed in the settings, puts it in force,
    /// and keeps it for the next start.
    fn set_preferences(
        &mut self,
        preferences: Preferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if preferences.theme != self.preferred.preferences.theme {
            apply_theme(preferences.theme, window, cx);
        }

        let width = preferences.width.pixels();

        for project in &self.projects {
            if let State::Working { workspace, .. } = &project.state {
                workspace.update(cx, |workspace, cx| workspace.set_width(width, cx));
            }
        }

        self.preferred.preferences = preferences;
        self.save_preferences(window, cx);
    }

    /// Opens the project shown in the editor.
    fn open_shown_in_editor(
        &mut self,
        _: &OpenInEditor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.selected {
            self.open_in_editor(id, window, cx);
        }
    }

    /// Gives the keyboard to the sidebar's search, showing the sidebar.
    fn focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        if self.preferred.preferences.sidebar_hidden {
            self.preferred.preferences.sidebar_hidden = false;
            self.save_preferences(window, cx);
        }

        self.close_settings(&CloseSettings, window, cx);
        self.search
            .update(cx, |search, cx| search.focus(window, cx));
    }

    /// Shows the project's session at `position`, counted from 1, in the
    /// order the host lists them: most recently used first.
    fn jump(&mut self, position: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected else {
            return;
        };

        let Some(session) = self
            .workspace_of(id)
            .and_then(|workspace| workspace.read(cx).sessions().get(position - 1).cloned())
        else {
            return;
        };

        self.open_session(id, session.value, window, cx);
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
        let sidebar_shown = self.settings.is_none() && !self.preferred.preferences.sidebar_hidden;

        v_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_palette))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::open_project_action))
            .on_action(cx.listener(Self::close_settings))
            .on_action(cx.listener(Self::new_session_shown))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::open_shown_in_editor))
            .on_action(cx.listener(Self::focus_search))
            .on_action(cx.listener(|this, _: &Jump1, window, cx| this.jump(1, window, cx)))
            .on_action(cx.listener(|this, _: &Jump2, window, cx| this.jump(2, window, cx)))
            .on_action(cx.listener(|this, _: &Jump3, window, cx| this.jump(3, window, cx)))
            .on_action(cx.listener(|this, _: &Jump4, window, cx| this.jump(4, window, cx)))
            .on_action(cx.listener(|this, _: &Jump5, window, cx| this.jump(5, window, cx)))
            .on_action(cx.listener(|this, _: &Jump6, window, cx| this.jump(6, window, cx)))
            .on_action(cx.listener(|this, _: &Jump7, window, cx| this.jump(7, window, cx)))
            .on_action(cx.listener(|this, _: &Jump8, window, cx| this.jump(8, window, cx)))
            .on_action(cx.listener(|this, _: &Jump9, window, cx| this.jump(9, window, cx)))
            // A drag of the sidebar's edge ends wherever the pointer is.
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if this.resized {
                        this.save_preferences(window, cx);
                    }
                }),
            )
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_toolbar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .when(sidebar_shown, |body| body.child(self.render_sidebar(cx)))
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

/// Opens a folder or a file in the editor chosen, or in one found on the
/// PATH of `environment`, saying why when there is none or it fails.
pub fn open_with_editor(
    chosen: &str,
    environment: &Environment,
    target: &Path,
    window: &mut Window,
    cx: &mut App,
) {
    let path = environment
        .variables()
        .iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.clone())
        .or_else(|| std::env::var_os("PATH"));

    let Some((program, args)) = editor::command(chosen, path.as_deref(), target) else {
        window.push_notification(
            Notification::error(
                "No editor was found. Choose the command that opens one in Settings, General.",
            ),
            cx,
        );
        return;
    };

    let started = std::process::Command::new(&program)
        .args(&args)
        .envs(
            environment
                .variables()
                .iter()
                .map(|(name, value)| (name, value)),
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    match started {
        // Waited for apart, so the editor leaves nothing behind once it
        // exits.
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }

        Err(error) => {
            window.push_notification(
                Notification::error(format!("{program} could not be started: {error}")),
                cx,
            );
        }
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
