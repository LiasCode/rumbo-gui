//! Read-only directory discovery for the project explorer.

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
}
