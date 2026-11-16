//! What the person prefers of this window, kept from one start to the next
//! in a file of its own, since rumbo keeps everything else: the theme.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub theme: Theme,
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

        let dark = Preferences { theme: Theme::Dark };

        save(&dark, &file).unwrap();

        assert_eq!(load(&file), (dark, None));
        assert!(
            fs::read_to_string(&file)
                .unwrap()
                .contains(r#""theme": "dark""#)
        );
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
