//! What changed in a project's folder, as Git sees it: the patch of every
//! tracked file against the last commit, and of every new file that Git
//! does not ignore. rumbo keeps the conversation, not the files, so this is
//! how a person sees what a run did to them. The window only reads.

use std::path::Path;
use std::process::Command;

/// The most new files given a patch; the rest are only counted, since a
/// run can make many, such as a build's output.
const MOST_NEW_FILES: usize = 50;

/// What the folder holds now against its last commit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    /// The branch checked out, or empty when none is, as on a detached head.
    pub branch: String,
    /// The patch, in Git's format, of every file that changed.
    pub patch: String,
    /// New files left out of the patch.
    pub more_new: usize,
}

/// Reads what changed in `dir`. It fails, saying why, when the folder is
/// not in a Git repository or Git cannot be run.
pub fn changes(dir: &Path) -> Result<Changes, String> {
    git(dir, &["rev-parse", "--is-inside-work-tree"]).map_err(|problem| {
        match problem.contains("not a git repository") {
            true => {
                "This folder is not in a Git repository, so its changes cannot be shown.".into()
            }
            false => problem,
        }
    })?;

    // Fails on a detached head, which has no branch to name.
    let branch = git(dir, &["symbolic-ref", "--short", "-q", "HEAD"])
        .map(|name| name.trim().to_string())
        .unwrap_or_default();

    // A repository without a commit has no HEAD to compare with: every
    // file in it is new.
    let has_commit = git(dir, &["rev-parse", "--verify", "--quiet", "HEAD"]).is_ok();

    let mut patch = if has_commit {
        git(
            dir,
            &["diff", "HEAD", "--no-color", "--no-ext-diff", "--relative"],
        )?
    } else {
        String::new()
    };

    let untracked = git(dir, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    let new: Vec<&str> = untracked
        .split('\0')
        .filter(|path| !path.is_empty())
        .collect();

    for path in new.iter().take(MOST_NEW_FILES) {
        // Exits with 1 when the files differ, which they always do here.
        let added = run(
            dir,
            &["diff", "--no-color", "--no-index", "--", "/dev/null", path],
            &[0, 1],
        )?;

        patch.push_str(&added);
    }

    Ok(Changes {
        branch,
        patch,
        more_new: new.len().saturating_sub(MOST_NEW_FILES),
    })
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    run(dir, args, &[0])
}

/// Runs Git in `dir`, taking any of `ok` as its exit status to succeed.
fn run(dir: &Path, args: &[&str], ok: &[i32]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .map_err(|error| format!("Git could not be run: {error}"))?;

    if !output.status.code().is_some_and(|code| ok.contains(&code)) {
        let said = String::from_utf8_lossy(&output.stderr).trim().to_string();

        return Err(if said.is_empty() {
            format!("git {} failed", args.join(" "))
        } else {
            said
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir =
                std::env::temp_dir().join(format!("rumbo-gui-git-{name}-{}", std::process::id()));

            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();

            Scratch(dir)
        }

        fn git(&self, args: &[&str]) {
            let status = Command::new("git")
                .arg("-C")
                .arg(&self.0)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .output()
                .unwrap()
                .status;

            assert!(status.success(), "git {args:?}");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn shows_the_changed_and_the_new_files_against_the_last_commit() {
        let scratch = Scratch::new("changes");

        scratch.git(&["init", "-q", "-b", "main"]);
        fs::write(scratch.0.join("kept.txt"), "one\n").unwrap();
        fs::write(scratch.0.join(".gitignore"), "ignored.txt\n").unwrap();
        scratch.git(&["add", "."]);
        scratch.git(&["commit", "-q", "-m", "first"]);

        assert_eq!(
            changes(&scratch.0).unwrap(),
            Changes {
                branch: "main".into(),
                ..Changes::default()
            }
        );

        fs::write(scratch.0.join("kept.txt"), "one\ntwo\n").unwrap();
        fs::write(scratch.0.join("new.txt"), "fresh\n").unwrap();
        fs::write(scratch.0.join("ignored.txt"), "noise\n").unwrap();

        let changed = changes(&scratch.0).unwrap();

        assert!(changed.patch.contains("+two"), "{}", changed.patch);
        assert!(changed.patch.contains("new.txt"), "{}", changed.patch);
        assert!(changed.patch.contains("+fresh"), "{}", changed.patch);
        assert!(!changed.patch.contains("ignored"), "{}", changed.patch);
    }

    #[test]
    fn says_when_the_folder_is_not_a_repository() {
        let scratch = Scratch::new("plain");

        let problem = changes(&scratch.0).unwrap_err();

        assert!(problem.contains("not in a Git repository"), "{problem}");
    }

    #[test]
    fn takes_every_file_of_a_repository_without_a_commit_as_new() {
        let scratch = Scratch::new("unborn");

        scratch.git(&["init", "-q", "-b", "main"]);
        fs::write(scratch.0.join("first.txt"), "hello\n").unwrap();

        let changed = changes(&scratch.0).unwrap();

        assert!(changed.patch.contains("+hello"), "{}", changed.patch);
    }
}
