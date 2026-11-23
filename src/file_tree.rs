//! Read-only directory discovery for the project explorer, and the text of
//! a file it shows.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Folder,
    File,
    Other,
}

pub struct Entry {
    pub name: OsString,
    pub kind: Kind,
}

/// Whether a folder is a Git repository's own: it holds `.git`, a folder,
/// or a file for a worktree or a submodule.
pub fn holds_repository(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Lists one folder, without walking into children or following links.
pub fn children(path: &Path) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name();

        if name == ".git" {
            continue;
        }

        let kind = match entry.file_type() {
            Ok(kind) if kind.is_dir() => Kind::Folder,
            Ok(kind) if kind.is_file() => Kind::File,
            Ok(_) => Kind::Other,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };

        entries.push(Entry { name, kind });
    }

    entries.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

/// The largest file shown as text; a larger one is the editor's to show.
const MOST_SHOWN: u64 = 2 * 1024 * 1024;

/// A file's text, to show it, or why it is not shown: it is too large, or
/// it is not text, which Git takes a NUL in it to say.
pub fn text(path: &Path) -> Result<String, String> {
    let unreadable = |error: io::Error| format!("Could not read this file: {error}");
    let size = fs::metadata(path).map_err(unreadable)?.len();

    if size > MOST_SHOWN {
        return Err(format!(
            "This file is larger than {} MB, so it is not shown here. Open it in the editor to read it.",
            MOST_SHOWN / 1024 / 1024
        ));
    }

    let bytes = fs::read(path).map_err(unreadable)?;

    if bytes.contains(&0) {
        return Err(
            "This file is not text, so it is not shown here. Open it in the editor to see it."
                .into(),
        );
    }

    // Text in another encoding still shows, with what is not UTF-8 marked.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_immediate_children_with_folders_first() -> io::Result<()> {
        let root = std::env::temp_dir().join(format!("rumbo-files-{}", std::process::id()));
        fs::create_dir(&root)?;

        let checked = (|| {
            fs::create_dir(root.join("src"))?;
            fs::create_dir(root.join(".git"))?;
            fs::write(root.join("src/nested.rs"), "")?;
            fs::write(root.join("README"), "")?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(root.join("src"), root.join("linked"))?;

            let listed = children(&root)?;
            let mut expected = vec![("src", Kind::Folder), ("README", Kind::File)];
            #[cfg(unix)]
            expected.push(("linked", Kind::Other));

            let got: Vec<_> = listed
                .iter()
                .map(|entry| (entry.name.to_str().unwrap(), entry.kind))
                .collect();

            assert_eq!(got, expected);
            assert_eq!(children(&root.join("src"))?[0].name, "nested.rs");
            assert!(children(&root.join("missing")).is_err());
            assert!(holds_repository(&root));
            assert!(!holds_repository(&root.join("src")));
            Ok(())
        })();

        fs::remove_dir_all(&root)?;
        checked
    }

    #[test]
    fn reads_a_text_file_and_says_why_another_is_not_shown() -> io::Result<()> {
        let root = std::env::temp_dir().join(format!("rumbo-file-text-{}", std::process::id()));

        fs::create_dir(&root)?;

        let checked = (|| {
            fs::write(root.join("notes.txt"), "one\ntwo\n")?;
            fs::write(root.join("image.png"), [0x89, b'P', b'N', b'G', 0, 1])?;
            fs::write(root.join("big.log"), vec![b'x'; MOST_SHOWN as usize + 1])?;

            assert_eq!(text(&root.join("notes.txt")).as_deref(), Ok("one\ntwo\n"));

            let binary = text(&root.join("image.png")).unwrap_err();
            let big = text(&root.join("big.log")).unwrap_err();
            let missing = text(&root.join("missing.txt")).unwrap_err();

            assert!(binary.contains("not text"), "{binary}");
            assert!(big.contains("larger than 2 MB"), "{big}");
            assert!(missing.starts_with("Could not read this file"), "{missing}");
            Ok(())
        })();

        fs::remove_dir_all(&root)?;
        checked
    }
}
