//! The window: a toolbar, the projects open in it with their sessions, and
//! the work of the session shown, or the settings. Each project has a host
//! of its own, started when the project is opened, whose runs go on while
//! another project is shown. Closing the window ends every host.

mod catalog;
mod settings;
mod sidebar;
mod toolbar;

use std::path::{Path, PathBuf};

use std::time::Duration;

use gpui::{
    AnyElement, App, AppContext as _, Context, Div, ElementId, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, KeyBinding, MouseButton, NavigationDirection,
    ParentElement as _, PathPromptOptions, Render, Styled as _, Subscription, SystemNotification,
    Task, Window, actions, div, prelude::FluentBuilder as _, px,
};
use ui::{
    ActiveTheme as _, Button, DialogLayer, Dialogs, IconName, InputEvent, InputState, PanelSize,
    Resizable, ResizablePanel, Sizable as _, Spinner, Theme, ThemeMode, Toast, Toaster, Toasts,
    WindowFrame, h_flex, rems_from_px, v_flex,
};

use crate::config::{self, Config};
use crate::editor;
use crate::environment::Environment;
use crate::host::{Descriptors, Host};
use crate::navigation::Navigation;
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
        GoBack,
        GoForward,
        ZoomIn,
        ZoomOut,
        ZoomReset,
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

/// How long the sidebar's width waits after it last changed to be kept,
/// so a drag writes the preferences once.
const KEEP_WIDTH_AFTER: Duration = Duration::from_millis(400);

/// The least the work beside the sidebar is given.
const MAIN_MIN_WIDTH: f32 = 320.;

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
        KeyBinding::new("alt-left", GoBack, Some(KEY_CONTEXT)),
        KeyBinding::new("alt-right", GoForward, Some(KEY_CONTEXT)),
        // Plus is a key of its own on some keyboards and Shift-= on others,
        // and both it and minus are on the keypad too.
        KeyBinding::new("ctrl-+", ZoomIn, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-=", ZoomIn, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-add", ZoomIn, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl--", ZoomOut, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-subtract", ZoomOut, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-0", ZoomReset, Some(KEY_CONTEXT)),
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
pub const SHORTCUTS: [(&str, &str); 18] = [
    ("Enter", "Send the task, or tell the run in progress"),
    ("Shift-Enter", "Start a new line"),
    ("Up / Down", "Bring back what was asked in the project"),
    ("Escape", "Cancel the run, or give up a provider's function"),
    ("Ctrl-K, Ctrl-P", "Commands, sessions, and projects"),
    (
        "Ctrl-N",
        "New session, in your home folder when no project is open",
    ),
    (
        "Ctrl-1 … Ctrl-9",
        "Show the project's sessions, most recent first",
    ),
    (
        "Alt-Left / Alt-Right",
        "Go back or forward between what was shown",
    ),
    ("Ctrl-B", "Show or put away the sidebar"),
    ("Ctrl-Shift-G", "Changes in the project's folder"),
    ("Ctrl-Alt-B", "The session's details"),
    ("Ctrl-Shift-E", "Open the project in the editor"),
    ("Ctrl-O", "Open a project"),
    ("Ctrl-F", "Search the sessions"),
    ("Ctrl-+ / Ctrl--", "Make everything bigger or smaller"),
    ("Ctrl-0", "Everything at its own size"),
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

/// A place the window shows, to go back and forward between.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Place {
    /// A session of a project open in the window: empty for a new session,
    /// and while the project's host starts.
    Session {
        project: ProjectId,
        session: String,
    },
    Settings,
}

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
    /// The window's dialogs and notifications, which its workspaces show
    /// theirs in too.
    dialogs: Entity<Dialogs>,
    toasts: Entity<Toasts>,
    /// Keeps the sidebar's width once it stops changing.
    keeping_width: Option<Task<()>>,
    /// The places shown, to go back and forward between.
    places: Navigation<Place>,
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
        ui::set_zoom(window, preferred.preferences.scale());

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

        // When what had the keyboard is gone, as a session deleted from the
        // sidebar, the work takes it, so the window's keys still work.
        let focus_lost = cx.on_focus_lost(window, |this, window, cx| this.focus_work(window, cx));

        // Pressing a desktop notification of a run shows its session, and
        // asks the desktop to bring the window to the front, which it may
        // not do without a press in the window.
        let shell = cx.weak_entity();
        let handle = window.window_handle();

        cx.on_system_notification_response(move |response, cx| {
            let Some((id, session)) = tagged(&response.tag) else {
                return;
            };

            let _ = handle.update(cx, |_, window, cx| {
                window.activate_window();

                let _ = shell.update(cx, |this, cx| {
                    if this.project(id).is_some() {
                        this.open_session(id, session, window, cx);
                    }
                });
            });
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
            dialogs: cx.new(|_| Dialogs::new()),
            toasts: cx.new(|_| Toasts::new()),
            keeping_width: None,
            places: Navigation::default(),
            _subscriptions: vec![appearance, activation, searching, focus_lost],
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
        self.record(cx);
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
        self.record(cx);

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
        let dialogs = self.dialogs.clone();
        let toasts = self.toasts.clone();
        let workspace = cx.new(|cx| {
            let mut workspace = Workspace::new(host, dialogs, toasts, window, cx);

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

        // The place of a project whose host was starting is the session the
        // host shows now.
        let starting = Place::Session {
            project: id,
            session: String::new(),
        };

        if self.places.current() == Some(&starting) {
            self.places.replace(Place::Session {
                project: id,
                session: workspace.read(cx).current().to_string(),
            });
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
                &self.toasts,
                cx,
            ),

            WorkspaceEvent::RunEnded {
                session,
                title,
                status,
                error,
                shown,
            } => {
                let ended = Ended {
                    session,
                    title,
                    status,
                    error,
                };

                self.run_ended(id, &ended, *shown, window, cx)
            }

            WorkspaceEvent::Shown { session, went } => self.shown(id, session, *went, cx),
        }
    }

    /// Takes in the session a project shows now. Only the project shown,
    /// with no settings over it, shows a place.
    fn shown(&mut self, id: ProjectId, session: &str, went: bool, cx: &mut Context<Self>) {
        if self.selected != Some(id) || self.settings.is_some() {
            return;
        }

        let place = Place::Session {
            project: id,
            session: session.to_string(),
        };

        if went {
            self.places.visit(place);
        } else if matches!(self.places.current(), Some(Place::Session { project, .. }) if *project == id)
        {
            self.places.replace(place);
        }

        cx.notify();
    }

    /// Says that a run ended where the person may not see it: in a
    /// notification of the window when its session is not shown, unless
    /// they asked for quiet, and the sidebar marks the session until it is;
    /// and in one of the desktop when the window is in the background, if
    /// they asked for that.
    fn run_ended(
        &mut self,
        id: ProjectId,
        ended: &Ended<'_>,
        shown: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let project = self
            .project(id)
            .map(|project| folder_name(&project.dir))
            .unwrap_or_default();

        if !shown && !self.preferred.preferences.quiet {
            let said = match ended.status {
                crate::protocol::RUN_DONE => "finished",
                crate::protocol::RUN_CANCELLED => "was cancelled",
                _ => "failed",
            };
            let note = Toast::new(format!("A run {said} in {project}"))
                .description(ended.title.to_string());

            let note = match ended.status {
                crate::protocol::RUN_DONE => note.success(),
                crate::protocol::RUN_CANCELLED => note,
                _ => note.danger(),
            };

            self.toasts.update(cx, |toasts, cx| {
                toasts.push(note, cx);
            });
        }

        if self.preferred.preferences.desktop_notifications && !window.is_window_active() {
            cx.show_system_notification(desktop_note(id, &project, ended));
        }
    }

    /// Shows a project: the session its host has current, or why it has
    /// none, as a place the person went to.
    fn select(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        self.show_project(id, window, cx);
        self.record(cx);
    }

    /// Shows a project without recording a place: going on to one of its
    /// sessions records it then, and going back or forward records none.
    fn show_project(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some(id);
        self.leave_settings(window, cx);

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
        self.show_project(id, window, cx);

        // The workspace says when it shows another session, which records
        // the place.
        let Some(workspace) = self.workspace_of(id) else {
            self.record(cx);
            return;
        };

        let shown = workspace.read(cx);
        let follows = shown.current() != session
            && !shown.has_ended()
            && shown.offers(crate::protocol::FUNCTION_OPEN);

        match follows {
            true => workspace.update(cx, |workspace, cx| workspace.open_session(&session, cx)),
            false => self.record(cx),
        }
    }

    /// Makes the next task of a project start a new session.
    fn new_session(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        self.show_project(id, window, cx);

        let Some(workspace) = self.workspace_of(id) else {
            self.record(cx);
            return;
        };

        let shown = workspace.read(cx);
        let follows = !shown.current().is_empty()
            && !shown.has_ended()
            && shown.offers(crate::protocol::FUNCTION_NEW);

        match follows {
            true => workspace.update(cx, |workspace, cx| workspace.new_session(cx)),
            false => self.record(cx),
        }
    }

    /// Makes the next task of the project shown start a new session. With
    /// no project shown, the person's home folder is opened as one, in a
    /// new session.
    fn new_session_shown(&mut self, _: &NewSession, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.selected {
            self.new_session(id, window, cx);
            return;
        }

        if !self.opens_projects() {
            return;
        }

        let home = catalog::home().to_string_lossy().to_string();

        self.leave_settings(window, cx);
        self.open_listed(home, Opening::New, window, cx);
    }

    /// Asks for a new title for a session of a project, which shows it.
    fn rename_session(
        &mut self,
        id: ProjectId,
        session: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_project(id, window, cx);

        // Renaming another session shows it first, which records the place.
        if let Some(workspace) = self.workspace_of(id) {
            if workspace.read(cx).current() == session {
                self.record(cx);
            }

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
    fn open_in_editor(&mut self, id: ProjectId, cx: &mut Context<Self>) {
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
            &self.toasts,
            cx,
        );
    }

    /// Puts the sidebar away, or brings it back.
    fn toggle_sidebar(&mut self, _: &ToggleSidebar, window: &mut Window, cx: &mut Context<Self>) {
        let hidden = !self.preferred.preferences.sidebar_hidden;

        self.preferred.preferences.sidebar_hidden = hidden;

        // What had the keyboard in the sidebar goes with it: the work takes
        // it.
        if hidden && self.settings.is_none() {
            self.focus_work(window, cx);
        }

        self.save_preferences(cx);
    }

    fn zoom_in(&mut self, _: &ZoomIn, window: &mut Window, cx: &mut Context<Self>) {
        let zoom = preferences::zoom_in(self.preferred.preferences.zoom());

        self.set_zoom(zoom, window, cx);
    }

    fn zoom_out(&mut self, _: &ZoomOut, window: &mut Window, cx: &mut Context<Self>) {
        let zoom = preferences::zoom_out(self.preferred.preferences.zoom());

        self.set_zoom(zoom, window, cx);
    }

    fn zoom_reset(&mut self, _: &ZoomReset, window: &mut Window, cx: &mut Context<Self>) {
        self.set_zoom(preferences::OWN_ZOOM, window, cx);
    }

    /// Draws everything at `zoom` percent of its own size, and keeps it for
    /// the next start.
    fn set_zoom(&mut self, zoom: u32, window: &mut Window, cx: &mut Context<Self>) {
        if zoom == self.preferred.preferences.zoom() {
            return;
        }

        self.preferred.preferences.set_zoom(zoom);
        ui::set_zoom(window, self.preferred.preferences.scale());

        // The settings keep what they show, and would give the old size back
        // with the next preference changed there.
        if let Some((settings, _)) = &self.settings {
            let preferences = self.preferred.preferences.clone();

            settings.update(cx, |view, cx| view.set_preferences(preferences, cx));
        }

        self.save_preferences(cx);
    }

    /// Keeps the preferences for the next start, saying when they could not
    /// be kept.
    fn save_preferences(&mut self, cx: &mut Context<Self>) {
        self.keeping_width = None;

        if let Some(path) = &self.preferred.path {
            self.preferred.problem = preferences::save(&self.preferred.preferences, path).err();
        }

        if let Some(problem) = self.preferred.problem.clone() {
            self.toasts.update(cx, |toasts, cx| {
                toasts.push(Toast::new(problem).danger(), cx);
            });
        }

        cx.notify();
    }

    /// Takes the width a drag or the keys gave the sidebar, and keeps it
    /// once it stops changing.
    fn resize_sidebar(&mut self, size: &PanelSize, cx: &mut Context<Self>) {
        if size.key != "sidebar" {
            return;
        }

        // It is kept at the interface's own size, as it grows with it.
        let width = (f32::from(size.size) / self.preferred.preferences.scale())
            .clamp(sidebar::MIN_WIDTH, sidebar::MAX_WIDTH);

        self.preferred.preferences.sidebar_width = Some(width.round() as u32);
        self.keeping_width = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(KEEP_WIDTH_AFTER).await;

            let _ = this.update(cx, |this, cx| this.save_preferences(cx));
        }));
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
        let said = match working {
            1 => "A run works in it. Closing the project cancels it.".to_string(),
            _ => format!("{working} runs work in it. Closing the project cancels them."),
        };

        self.dialogs.update(cx, |dialogs, cx| {
            dialogs.open(
                move |dialog, _, _| {
                    let shell = shell.clone();

                    dialog
                        .title(format!("Close {name}?"))
                        .child(said.clone())
                        .danger()
                        .on_ok("Close Project", move |window, cx| {
                            let _ =
                                shell.update(cx, |this, cx| this.remove_project(id, window, cx));
                            true
                        })
                },
                window,
                cx,
            )
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
                    // Nothing is left to go back or forward to.
                    self.selected = None;
                    self.places = Navigation::default();
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
                    let said = format!("The folder could not be chosen: {error}");

                    this.toasts.update(cx, |toasts, cx| {
                        toasts.push(Toast::new(said).danger(), cx);
                    });
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

    /// Shows the settings in place of the work, as a place the person went
    /// to.
    fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        self.show_settings(window, cx);
        self.record(cx);
    }

    /// Shows the settings without recording a place.
    fn show_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    /// Goes back from the settings to the work, as a place the person went
    /// to.
    fn close_settings(&mut self, _: &CloseSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.is_none() {
            return;
        }

        self.leave_settings(window, cx);
        self.record(cx);
    }

    /// Puts the settings away without recording a place.
    fn leave_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.take().is_none() {
            return;
        }

        self.focus_work(window, cx);
        self.set_title(window, cx);
        cx.notify();
    }

    /// The place shown, once there is one.
    fn shown_place(&self, cx: &App) -> Option<Place> {
        if self.settings.is_some() {
            return Some(Place::Settings);
        }

        let project = self.selected?;
        let session = self
            .workspace_of(project)
            .map(|workspace| workspace.read(cx).current().to_string())
            .unwrap_or_default();

        Some(Place::Session { project, session })
    }

    /// Records the place shown as one the person went to.
    fn record(&mut self, cx: &App) {
        if let Some(place) = self.shown_place(cx) {
            self.places.visit(place);
        }
    }

    /// Whether a place can still be shown: its project is open, and its
    /// session is listed, or is new, or the host lists none.
    fn can_show(&self, place: &Place, cx: &App) -> bool {
        let Place::Session { project, session } = place else {
            return true;
        };

        if self.project(*project).is_none() {
            return false;
        }

        let Some(workspace) = self.workspace_of(*project) else {
            return true;
        };

        let workspace = workspace.read(cx);
        let listed = workspace.sessions();

        session.is_empty()
            || session == workspace.current()
            || listed.is_empty()
            || listed.iter().any(|listed| listed.value == *session)
    }

    fn go_back(&mut self, _: &GoBack, window: &mut Window, cx: &mut Context<Self>) {
        let mut places = std::mem::take(&mut self.places);
        let place = places.back(|place| self.can_show(place, cx));

        self.places = places;

        if let Some(place) = place {
            self.arrive(place, window, cx);
        }
    }

    fn go_forward(&mut self, _: &GoForward, window: &mut Window, cx: &mut Context<Self>) {
        let mut places = std::mem::take(&mut self.places);
        let place = places.forward(|place| self.can_show(place, cx));

        self.places = places;

        if let Some(place) = place {
            self.arrive(place, window, cx);
        }
    }

    /// Shows a place gone back or forward to, which records none.
    fn arrive(&mut self, place: Place, window: &mut Window, cx: &mut Context<Self>) {
        match place {
            Place::Settings => self.show_settings(window, cx),

            Place::Session { project, session } => {
                self.show_project(project, window, cx);

                if let Some(workspace) = self.workspace_of(project) {
                    workspace.update(cx, |workspace, cx| workspace.revisit(&session, cx));
                }
            }
        }

        cx.notify();
    }

    /// Gives the keyboard to what is shown: the settings, the composer of
    /// the project shown, or the main action of a screen without one.
    fn focus_work(&self, window: &mut Window, cx: &mut App) {
        if let Some((settings, _)) = &self.settings {
            settings.update(cx, |settings, cx| settings.focus(window, cx));
            return;
        }

        match self.selected_workspace() {
            Some(workspace) => workspace.update(cx, |workspace, cx| workspace.focus(window, cx)),
            None => self.action_focus.focus(window, cx),
        }
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

        if preferences.zoom() != self.preferred.preferences.zoom() {
            ui::set_zoom(window, preferences.scale());
        }

        let width = preferences.width.pixels();

        for project in &self.projects {
            if let State::Working { workspace, .. } = &project.state {
                workspace.update(cx, |workspace, cx| workspace.set_width(width, cx));
            }
        }

        self.preferred.preferences = preferences;
        self.save_preferences(cx);
    }

    /// Opens the project shown in the editor.
    fn open_shown_in_editor(&mut self, _: &OpenInEditor, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.selected {
            self.open_in_editor(id, cx);
        }
    }

    /// Gives the keyboard to the sidebar's search, showing the sidebar.
    fn focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        if self.preferred.preferences.sidebar_hidden {
            self.preferred.preferences.sidebar_hidden = false;
            self.save_preferences(cx);
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
    /// session used last. The sidebar lists those when it is shown.
    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        let sidebar_shown = !self.preferred.preferences.sidebar_hidden;
        let listed: Vec<Button> = self
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

                Button::new(ElementId::Name(format!("recent-{root}").into()))
                    .ghost()
                    .w_full()
                    .justify_start()
                    .truncate()
                    .icon(IconName::FolderClosed)
                    .label(format!("{label} — {}", project.description))
                    .tooltip(root.clone())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_listed(root.clone(), Opening::Latest, window, cx)
                    }))
            })
            .collect();

        let said = match sidebar_shown && !listed.is_empty() {
            true => "Open the folder of a project to work in it, or a recent one in the sidebar.",
            false => "Open the folder of a project to work in it.",
        };

        centered()
            .child(div().text_xl().child("rumbo"))
            .child(muted(said, cx))
            .child(
                Button::new("open-folder")
                    .track_focus(&self.action_focus)
                    .primary()
                    .icon(IconName::FolderOpen)
                    .label("Open Project…")
                    .on_click(cx.listener(|this, _, window, cx| this.choose_folder(window, cx))),
            )
            .when(!sidebar_shown && !listed.is_empty(), |screen| {
                screen.child(
                    v_flex()
                        .w(rems_from_px(440.))
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
                    .bg(cx.theme().colors.muted)
                    .font_family(cx.theme().mono_family.clone())
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

        let mut body = Resizable::horizontal("body");
        let scale = self.preferred.preferences.scale();

        if sidebar_shown {
            body = body.child(
                ResizablePanel::new("sidebar")
                    .size(px(self.sidebar_width() * scale))
                    .min(px(sidebar::MIN_WIDTH * scale))
                    .max(px(sidebar::MAX_WIDTH * scale))
                    .child(self.render_sidebar(cx)),
            );
        }

        let body = body
            .child(
                ResizablePanel::new("main")
                    .min(px(MAIN_MIN_WIDTH * scale))
                    .child(self.render_main(cx)),
            )
            .on_resize(cx.listener(|this, size: &PanelSize, _, cx| this.resize_sidebar(size, cx)));

        WindowFrame::new().child(
            ui::root(cx)
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
                .on_action(cx.listener(Self::go_back))
                .on_action(cx.listener(Self::go_forward))
                // The mouse's back and forward buttons, as in a browser.
                .on_mouse_down(
                    MouseButton::Navigate(NavigationDirection::Back),
                    cx.listener(|this, _, window, cx| this.go_back(&GoBack, window, cx)),
                )
                .on_mouse_down(
                    MouseButton::Navigate(NavigationDirection::Forward),
                    cx.listener(|this, _, window, cx| this.go_forward(&GoForward, window, cx)),
                )
                .on_action(cx.listener(Self::zoom_in))
                .on_action(cx.listener(Self::zoom_out))
                .on_action(cx.listener(Self::zoom_reset))
                .on_action(cx.listener(|this, _: &Jump1, window, cx| this.jump(1, window, cx)))
                .on_action(cx.listener(|this, _: &Jump2, window, cx| this.jump(2, window, cx)))
                .on_action(cx.listener(|this, _: &Jump3, window, cx| this.jump(3, window, cx)))
                .on_action(cx.listener(|this, _: &Jump4, window, cx| this.jump(4, window, cx)))
                .on_action(cx.listener(|this, _: &Jump5, window, cx| this.jump(5, window, cx)))
                .on_action(cx.listener(|this, _: &Jump6, window, cx| this.jump(6, window, cx)))
                .on_action(cx.listener(|this, _: &Jump7, window, cx| this.jump(7, window, cx)))
                .on_action(cx.listener(|this, _: &Jump8, window, cx| this.jump(8, window, cx)))
                .on_action(cx.listener(|this, _: &Jump9, window, cx| this.jump(9, window, cx)))
                .flex()
                .flex_col()
                .child(self.render_toolbar(cx))
                .child(div().flex_1().min_h_0().w_full().child(body))
                .child(DialogLayer::new(&self.dialogs))
                .child(Toaster::new(&self.toasts)),
        )
    }
}

/// A run that ended, as its workspace told it.
struct Ended<'a> {
    session: &'a str,
    title: &'a str,
    status: &'a str,
    error: &'a str,
}

/// What the desktop is told of a run that ended: the session's title, and
/// how the run ended in which project, with the first line of its error.
/// Its tag names the project and the session, which pressing it shows.
fn desktop_note(id: ProjectId, project: &str, ended: &Ended<'_>) -> SystemNotification {
    let error = crate::conversation::first_line(ended.error);
    let body = match ended.status {
        crate::protocol::RUN_DONE => format!("Finished in {project}"),
        crate::protocol::RUN_CANCELLED => format!("Cancelled in {project}"),
        _ if error.is_empty() => format!("Failed in {project}"),
        _ => format!("Failed in {project}: {error}"),
    };

    SystemNotification {
        tag: note_tag(id, ended.session).into(),
        title: ended.title.to_string().into(),
        body: body.into(),
        actions: Vec::new(),
    }
}

/// The tag of a desktop notification of a run in a session of a project.
fn note_tag(id: ProjectId, session: &str) -> String {
    format!("run:{id}:{session}")
}

/// The project and the session a desktop notification's tag names.
fn tagged(tag: &str) -> Option<(ProjectId, String)> {
    let (id, session) = tag.strip_prefix("run:")?.split_once(':')?;

    Some((id.parse().ok()?, session.to_string()))
}

/// Puts a theme in force. One that follows the desktop asks the window,
/// which knows its appearance better than the app on Linux.
fn apply_theme(theme: Colors, window: &mut Window, cx: &mut App) {
    let mode = match theme {
        Colors::System => ThemeMode::of_appearance(window.appearance()),
        Colors::Light => ThemeMode::Light,
        Colors::Dark => ThemeMode::Dark,
    };

    Theme::change(mode, cx);
}

/// Opens a folder or a file in the editor chosen, or in one found on the
/// PATH of `environment`, saying why when there is none or it fails.
pub fn open_with_editor(
    chosen: &str,
    environment: &Environment,
    target: &Path,
    toasts: &Entity<Toasts>,
    cx: &mut App,
) {
    let say = |problem: String, cx: &mut App| {
        toasts.update(cx, |toasts, cx| {
            toasts.push(Toast::new(problem).danger(), cx);
        });
    };

    let path = environment
        .variables()
        .iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.clone())
        .or_else(|| std::env::var_os("PATH"));

    let Some((program, args)) = editor::command(chosen, path.as_deref(), target) else {
        say(
            "No editor was found. Choose the command that opens one in Settings, General.".into(),
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

        Err(error) => say(format!("{program} could not be started: {error}"), cx),
    }
}

fn centered() -> Div {
    v_flex()
        .size_full()
        .p_8()
        .gap_3()
        .items_center()
        .justify_center()
}

fn muted(text: &str, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().colors.muted_foreground)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ended<'a>(status: &'a str, error: &'a str) -> Ended<'a> {
        Ended {
            session: "4cf0261c",
            title: "Fix the failing test",
            status,
            error,
        }
    }

    #[test]
    fn tells_the_desktop_how_a_run_ended_and_where() {
        let done = desktop_note(3, "alpha", &ended(crate::protocol::RUN_DONE, ""));

        assert_eq!(done.title, "Fix the failing test");
        assert_eq!(done.body, "Finished in alpha");
        assert_eq!(done.tag, "run:3:4cf0261c");

        let cancelled = desktop_note(3, "alpha", &ended(crate::protocol::RUN_CANCELLED, ""));

        assert_eq!(cancelled.body, "Cancelled in alpha");

        let failed = desktop_note(
            3,
            "alpha",
            &ended(crate::protocol::RUN_FAILED, "429 rate limited\nretry later"),
        );

        assert_eq!(failed.body, "Failed in alpha: 429 rate limited");

        let failed = desktop_note(3, "alpha", &ended(crate::protocol::RUN_FAILED, ""));

        assert_eq!(failed.body, "Failed in alpha");
    }

    #[test]
    fn a_notification_names_the_session_it_shows() {
        assert_eq!(
            tagged(&note_tag(3, "4cf0261c")),
            Some((3, "4cf0261c".into()))
        );
        assert_eq!(tagged("run:x:4cf0261c"), None);
        assert_eq!(tagged("other"), None);
    }
}
