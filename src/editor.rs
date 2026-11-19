//! The editor a project's folder, or a file of it, opens in: the command
//! the person chose, or else the first of the editors this knows that is on
//! the PATH the hosts get.

use std::ffi::OsStr;
use std::path::Path;

/// Editors a person likely has, in the order they are looked for. Each
/// opens the folder or the file it is given.
const KNOWN: [&str; 7] = ["zed", "zeditor", "cursor", "code", "codium", "subl", "kate"];

/// The program and the arguments that open `target`: the command chosen,
/// split at its spaces, with the target last; or a known editor found on
/// `path`, a PATH variable. None when there is neither.
pub fn command(chosen: &str, path: Option<&OsStr>, target: &Path) -> Option<(String, Vec<String>)> {
    let mut words: Vec<String> = chosen.split_whitespace().map(str::to_string).collect();

    if words.is_empty() {
        words.push(find(path?)?);
    }

    let program = words.remove(0);

    words.push(target.display().to_string());

    Some((program, words))
}

/// The first known editor on `path`.
fn find(path: &OsStr) -> Option<String> {
    KNOWN
        .iter()
        .find(|name| std::env::split_paths(path).any(|dir| dir.join(name).is_file()))
        .map(|name| name.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn opens_with_the_command_chosen_and_its_arguments() {
        assert_eq!(
            command("code --new-window", None, Path::new("/work/app")),
            Some((
                "code".into(),
                vec!["--new-window".into(), "/work/app".into()]
            ))
        );
    }

    #[test]
    fn finds_a_known_editor_on_the_path_when_none_is_chosen() {
        let dir = std::env::temp_dir().join(format!("rumbo-gui-editor-{}", std::process::id()));

        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("code"), "").unwrap();

        let path = std::env::join_paths([Path::new("/no/such/dir"), &dir]).unwrap();

        assert_eq!(
            command(" ", Some(&path), Path::new("/work/app")),
            Some(("code".into(), vec!["/work/app".into()]))
        );
        assert_eq!(command("", None, Path::new("/work/app")), None);

        fs::remove_dir_all(&dir).unwrap();
    }
}
