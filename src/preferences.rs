//! What the person prefers of this window, kept from one start to the next
//! in a file of its own, since rumbo keeps everything else: the theme, how
//! the window is laid out and how big it draws, and the editor projects
//! open in.
//!
//! The file is `rumbo-gui/preferences.json` under `$XDG_CONFIG_HOME`, or
//! under `~/.config` without it. It is read once at startup, and written
//! when a preference changes.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The colors of the window. Light is what the window had before it could
/// choose, so it stays the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Light or dark, as the desktop is, and changing when it does.
    System,
    #[default]
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }
}

/// How wide the conversation grows on a large window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Width {
    Narrow,
    #[default]
    Comfortable,
    Wide,
}

impl Width {
    pub const ALL: [Width; 3] = [Width::Narrow, Width::Comfortable, Width::Wide];

    pub fn label(self) -> &'static str {
        match self {
            Width::Narrow => "Narrow",
            Width::Comfortable => "Comfortable",
            Width::Wide => "Wide",
        }
    }

    /// The widest the conversation and the composer grow, in pixels.
    pub fn pixels(self) -> f32 {
        match self {
            Width::Narrow => 640.,
            Width::Comfortable => 800.,
            Width::Wide => 1080.,
        }
    }
}

/// The sizes the interface is drawn at, in percent of its own, from the
/// smallest. Ctrl-+ and Ctrl-- step through them.
pub const ZOOMS: [u32; 9] = [75, 80, 90, 100, 110, 125, 150, 175, 200];

/// The interface at its own size.
pub const OWN_ZOOM: u32 = 100;

/// The next size up from `zoom` percent, or the largest.
pub fn zoom_in(zoom: u32) -> u32 {
    ZOOMS
        .into_iter()
        .find(|step| *step > zoom)
        .unwrap_or(ZOOMS[ZOOMS.len() - 1])
}

/// The next size down from `zoom` percent, or the smallest.
pub fn zoom_out(zoom: u32) -> u32 {
    ZOOMS
        .into_iter()
        .rev()
        .find(|step| *step < zoom)
        .unwrap_or(ZOOMS[0])
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub theme: Theme,
    /// The sidebar is put away, for more room for the work.
    pub sidebar_hidden: bool,
    /// The sidebar's width in pixels at the interface's own size; none keeps
    /// the window's own.
    pub sidebar_width: Option<u32>,
    /// How wide the conversation grows.
    pub width: Width,
    /// The command a project's folder is opened with in an editor, such as
    /// `zed` or `code`; empty finds one of those known on the PATH.
    pub editor: String,
    /// Keeps quiet when a run ends in a session that is not shown, which the
    /// window otherwise says in a notification.
    pub quiet: bool,
    /// How big the interface is drawn, in percent of its own size; none
    /// draws it at its own.
    pub zoom: Option<u32>,
}

impl Preferences {
    /// How big the interface is drawn, in percent, among the sizes it is
    /// drawn at.
    pub fn zoom(&self) -> u32 {
        self.zoom
            .unwrap_or(OWN_ZOOM)
            .clamp(ZOOMS[0], ZOOMS[ZOOMS.len() - 1])
    }

    /// How many times its own size the interface is drawn.
    pub fn scale(&self) -> f32 {
        self.zoom() as f32 / OWN_ZOOM as f32
    }

    /// Draws the interface at `zoom` percent; at its own size, nothing is
    /// kept.
    pub fn set_zoom(&mut self, zoom: u32) {
        self.zoom = (zoom != OWN_ZOOM).then_some(zoom);
    }
}

/// Where the preferences are kept, given `$XDG_CONFIG_HOME` and `$HOME`.
/// None when neither is set, and they are then kept nowhere.
pub fn path(config_home: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let base = config_home
        .filter(|dir| dir.is_absolute())
        .or_else(|| home.map(|home| home.join(".config")))?;

    Some(base.join("rumbo-gui").join("preferences.json"))
}

/// Reads the preferences. A missing file holds the defaults; one that cannot
/// be read gives the defaults too, with what is wrong with it, so the window
/// can say so instead of failing to open.
pub fn load(path: &Path) -> (Preferences, Option<String>) {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Preferences::default(), None);
        }
        Err(error) => {
            return (
                Preferences::default(),
                Some(format!("{} could not be read: {error}", path.display())),
            );
        }
    };

    match serde_json::from_str(&text) {
        Ok(preferences) => (preferences, None),
        Err(error) => (
            Preferences::default(),
            Some(format!("{} is not usable: {error}", path.display())),
        ),
    }
}

/// Writes the preferences, making the folder they go in.
pub fn save(preferences: &Preferences, path: &Path) -> Result<(), String> {
    let written = path
        .parent()
        .map(fs::create_dir_all)
        .transpose()
        .and_then(|_| {
            let text = serde_json::to_string_pretty(preferences).map_err(std::io::Error::other)?;

            fs::write(path, text + "\n")
        });

    written.map_err(|error| format!("{} could not be written: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of its own for one test, removed when it ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!(
                "rumbo-gui-preferences-{name}-{}",
                std::process::id()
            ));

            let _ = fs::remove_dir_all(&dir);

            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn keeps_them_in_the_configuration_folder() {
        assert_eq!(
            path(Some("/xdg".into()), Some("/home/me".into())),
            Some(PathBuf::from("/xdg/rumbo-gui/preferences.json"))
        );
        assert_eq!(
            path(Some("relative".into()), Some("/home/me".into())),
            Some(PathBuf::from("/home/me/.config/rumbo-gui/preferences.json"))
        );
        assert_eq!(path(None, None), None);
    }

    #[test]
    fn starts_light_without_a_file_and_keeps_what_is_saved() {
        let scratch = Scratch::new("saved");
        let file = scratch.0.join("rumbo-gui").join("preferences.json");

        assert_eq!(load(&file), (Preferences::default(), None));
        assert_eq!(Preferences::default().theme, Theme::Light);

        let dark = Preferences {
            theme: Theme::Dark,
            sidebar_width: Some(300),
            editor: "zed".into(),
            ..Preferences::default()
        };

        save(&dark, &file).unwrap();

        assert_eq!(load(&file), (dark, None));
        assert!(
            fs::read_to_string(&file)
                .unwrap()
                .contains(r#""theme": "dark""#)
        );
    }

    #[test]
    fn steps_the_zoom_through_its_sizes() {
        let mut preferences = Preferences::default();

        assert_eq!(preferences.zoom(), OWN_ZOOM);
        assert_eq!(zoom_in(100), 110);
        assert_eq!(zoom_out(100), 90);

        // At either end it stays.
        assert_eq!(zoom_in(200), 200);
        assert_eq!(zoom_out(75), 75);

        // A size between two steps, as written by hand, goes to the next.
        assert_eq!(zoom_in(115), 125);
        assert_eq!(zoom_out(115), 110);

        // One beyond the sizes is drawn at the nearest.
        preferences.zoom = Some(500);
        assert_eq!(preferences.zoom(), 200);

        preferences.set_zoom(150);
        assert_eq!(preferences.zoom, Some(150));
        assert_eq!(preferences.scale(), 1.5);

        preferences.set_zoom(OWN_ZOOM);
        assert_eq!(preferences.zoom, None);
    }

    #[test]
    fn says_what_is_wrong_with_a_file_it_cannot_use() {
        let scratch = Scratch::new("wrong");
        let file = scratch.0.join("preferences.json");

        fs::create_dir_all(&scratch.0).unwrap();
        fs::write(&file, r#"{"theme": "blue"}"#).unwrap();

        let (preferences, problem) = load(&file);

        assert_eq!(preferences, Preferences::default());
        assert!(problem.unwrap().contains("not usable"));

        fs::write(&file, r#"{"later": 1}"#).unwrap();

        assert_eq!(load(&file), (Preferences::default(), None));
    }
}
