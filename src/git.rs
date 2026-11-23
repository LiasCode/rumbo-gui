//! What changed in a project's folder, as Git sees it: the patch of every
//! tracked file against the last commit, and of every new file that Git
//! does not ignore; and what Git says of each file, for the files panel.
//! rumbo keeps the conversation, not the files, so this is how a person
//! sees what a run did to them. The window only reads.

pub mod graph;
pub mod history;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
    repository(dir, "changes")?;

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

/// Fails, saying why plainly, when `dir` is not in a Git repository, so
/// what Git would show of it cannot be.
fn repository(dir: &Path, shown: &str) -> Result<(), String> {
    git(dir, &["rev-parse", "--is-inside-work-tree"])
        .map(|_| ())
        .map_err(|problem| match problem.contains("not a git repository") {
            true => {
                format!("This folder is not in a Git repository, so its {shown} cannot be shown.")
            }
            false => problem,
        })
}

/// What Git says of a file, from the least to the most a person should
/// notice; a folder shows the most of what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileStatus {
    Untracked,
    Modified,
    Added,
    Renamed,
    Deleted,
    Conflicted,
}

impl FileStatus {
    /// The letter beside the file's name.
    pub fn letter(self) -> &'static str {
        match self {
            FileStatus::Untracked => "U",
            FileStatus::Modified => "M",
            FileStatus::Added => "A",
            FileStatus::Renamed => "R",
            FileStatus::Deleted => "D",
            FileStatus::Conflicted => "!",
        }
    }

    pub fn words(self) -> &'static str {
        match self {
            FileStatus::Untracked => "Untracked",
            FileStatus::Modified => "Modified",
            FileStatus::Added => "Added",
            FileStatus::Renamed => "Renamed",
            FileStatus::Deleted => "Deleted",
            FileStatus::Conflicted => "Conflicted",
        }
    }
}

/// What Git says of the files under the folder it was asked in, by their
/// paths under that folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Statuses {
    files: HashMap<PathBuf, FileStatus>,
    /// Untracked folders, which Git names instead of each file in them.
    untracked: Vec<PathBuf>,
    /// The most of what each folder holds, the folder asked in included.
    folders: HashMap<PathBuf, FileStatus>,
}

impl Statuses {
    pub fn file(&self, path: &Path) -> Option<FileStatus> {
        self.files.get(path).copied().or_else(|| {
            self.untracked
                .iter()
                .any(|folder| path.starts_with(folder))
                .then_some(FileStatus::Untracked)
        })
    }

    pub fn folder(&self, path: &Path) -> Option<FileStatus> {
        self.folders.get(path).copied().or_else(|| self.file(path))
    }

    fn mark(&mut self, dir: &Path, path: PathBuf, status: FileStatus) {
        for folder in path
            .ancestors()
            .skip(1)
            .take_while(|folder| folder.starts_with(dir))
        {
            let most = self.folders.entry(folder.to_path_buf()).or_insert(status);

            *most = (*most).max(status);
        }

        self.files.insert(path, status);
    }
}

/// What `git status --porcelain -z` says, run in `dir`, whose path under the
/// repository's root is `prefix`, as Git's `--show-prefix` gives it.
fn parse(dir: &Path, prefix: &str, porcelain: &str) -> Statuses {
    let mut statuses = Statuses::default();
    let mut fields = porcelain.split('\0').filter(|field| !field.is_empty());

    while let Some(field) = fields.next() {
        let (Some(code), Some(path)) = (field.get(..2), field.get(3..)) else {
            continue;
        };

        // A rename or a copy names where it came from next, which is no
        // file of the folder now.
        if code.contains(['R', 'C']) {
            fields.next();
        }

        let Some(path) = path.strip_prefix(prefix) else {
            continue;
        };

        let status = match code {
            "??" => FileStatus::Untracked,
            "DD" | "AU" | "UD" | "UA" | "DU" | "AA" | "UU" => FileStatus::Conflicted,
            _ if code.contains('D') => FileStatus::Deleted,
            _ if code.starts_with('R') => FileStatus::Renamed,
            _ if code.starts_with(['A', 'C']) => FileStatus::Added,
            _ => FileStatus::Modified,
        };

        let absolute = dir.join(path.trim_end_matches('/'));

        if status == FileStatus::Untracked && path.ends_with('/') {
            statuses.mark(dir, absolute.clone(), status);
            statuses.untracked.push(absolute);
        } else {
            statuses.mark(dir, absolute, status);
        }
    }

    statuses
}

/// What Git says of the files under `dir`. It fails, saying why, when the
/// folder is not in a Git repository or Git cannot be run.
pub fn statuses(dir: &Path) -> Result<Statuses, String> {
    let prefix = git(dir, &["rev-parse", "--show-prefix"])?;
    let porcelain = git(
        dir,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=normal",
            "--",
            ".",
        ],
    )?;

    Ok(parse(dir, prefix.trim_end_matches('\n'), &porcelain))
}

/// What Git says of a path, in the deepest of the repositories asked about
/// that holds it, so a repository nested in another speaks for its files.
pub fn status_at(
    statuses: &HashMap<PathBuf, Statuses>,
    path: &Path,
    folder: bool,
) -> Option<FileStatus> {
    let (_, statuses) = statuses
        .iter()
        .filter(|(dir, _)| path.starts_with(dir))
        .max_by_key(|(dir, _)| dir.components().count())?;

    match folder {
        true => statuses.folder(path),
        false => statuses.file(path),
    }
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
    fn reads_branch_history_without_switching_branches() {
        let scratch = Scratch::new("branch-history");

        scratch.git(&["init", "-q", "-b", "main"]);
        scratch.git(&["commit", "--allow-empty", "-qm", "First commit"]);
        scratch.git(&["branch", "topic"]);
        scratch.git(&["commit", "--allow-empty", "-qm", "Main commit"]);
        scratch.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
        scratch.git(&[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ]);

        let current = history::read(&scratch.0, history::HEAD, 1).unwrap();

        assert_eq!(current.current.as_deref(), Some("main"));
        assert_eq!(current.commits.len(), 1);
        assert_eq!(current.commits[0].subject, "Main commit");
        assert_eq!(current.commits[0].author, "t");
        assert!(current.more);
        assert_eq!(
            current
                .branches
                .iter()
                .map(|branch| (branch.name.as_str(), branch.remote))
                .collect::<Vec<_>>(),
            [("main", false), ("topic", false), ("origin/main", true)]
        );

        let topic = history::read(&scratch.0, "refs/heads/topic", 1).unwrap();

        assert_eq!(topic.selected, "refs/heads/topic");
        assert_eq!(topic.current.as_deref(), Some("main"));
        assert_eq!(topic.commits[0].subject, "First commit");
        assert!(!topic.more);

        let missing = history::read(&scratch.0, "refs/heads/gone", 2).unwrap();

        assert_eq!(missing.selected, history::HEAD);
        assert_eq!(missing.commits.len(), 2);
        assert!(!missing.more);

        scratch.git(&["checkout", "--detach", "-q", "topic"]);

        let detached = history::read(&scratch.0, history::HEAD, 1).unwrap();

        assert_eq!(detached.current, None);
        assert_eq!(detached.commits[0].subject, "First commit");
    }

    #[test]
    fn tells_a_folder_outside_git_from_a_repository_without_commits() {
        let scratch = Scratch::new("empty-history");

        let problem = history::read(&scratch.0, history::HEAD, 1).unwrap_err();

        assert!(problem.contains("not in a Git repository"), "{problem}");

        scratch.git(&["init", "-q", "-b", "main"]);

        let empty = history::read(&scratch.0, history::HEAD, 1).unwrap();

        assert_eq!(empty.current.as_deref(), Some("main"));
        assert!(empty.commits.is_empty());
        assert!(!empty.more);
    }

    #[test]
    fn draws_every_branch_as_git_log_graph_does() {
        let scratch = Scratch::new("graph");

        scratch.git(&["init", "-q", "-b", "main"]);
        scratch.git(&["commit", "--allow-empty", "-qm", "First commit"]);
        scratch.git(&["checkout", "-q", "-b", "topic"]);
        scratch.git(&["commit", "--allow-empty", "-qm", "Topic commit"]);
        scratch.git(&["checkout", "-q", "main"]);
        scratch.git(&["commit", "--allow-empty", "-qm", "Main commit"]);
        scratch.git(&["merge", "-q", "--no-ff", "-m", "Merge topic", "topic"]);
        scratch.git(&["tag", "v1"]);
        scratch.git(&["update-ref", "refs/remotes/origin/main", "HEAD~1"]);

        let all = graph::read(&scratch.0, 10).unwrap();
        let commits: Vec<&graph::Commit> = all
            .lines
            .iter()
            .filter_map(|line| line.commit.as_ref())
            .collect();

        assert!(!all.more);
        assert_eq!(
            commits
                .iter()
                .map(|commit| commit.subject.as_str())
                .collect::<Vec<_>>(),
            ["Merge topic", "Topic commit", "Main commit", "First commit"]
        );
        assert_eq!(
            commits[0].labels,
            [
                graph::Label::Head(Some("main".into())),
                graph::Label::Tag("v1".into()),
            ]
        );
        assert_eq!(commits[1].labels, [graph::Label::Branch("topic".into())]);
        assert_eq!(
            commits[2].labels,
            [graph::Label::Remote("origin/main".into())]
        );

        // The merge's two lines are joined by lines with no commit, which
        // Git colors as it does in a terminal.
        assert!(all.lines.len() > commits.len());
        assert!(
            all.lines
                .iter()
                .flat_map(|line| &line.graph)
                .any(|stroke| stroke.color.is_some())
        );

        let first = graph::read(&scratch.0, 1).unwrap();

        assert!(first.more);
        assert_eq!(
            first
                .lines
                .iter()
                .filter(|line| line.commit.is_some())
                .count(),
            1
        );
    }

    #[test]
    fn draws_no_graph_without_commits_or_outside_git() {
        let scratch = Scratch::new("empty-graph");

        let problem = graph::read(&scratch.0, 1).unwrap_err();

        assert!(problem.contains("not in a Git repository"), "{problem}");

        scratch.git(&["init", "-q", "-b", "main"]);

        let empty = graph::read(&scratch.0, 1).unwrap();

        assert!(empty.lines.is_empty());
        assert!(!empty.more);
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

    #[test]
    fn reads_what_git_says_of_each_file_under_the_folder_asked() {
        let dir = Path::new("/work/app");
        let porcelain = concat!(
            " M app/src/main.rs\0",
            "A  app/src/new.rs\0",
            "R  app/docs/after.md\0app/docs/before.md\0",
            " D app/old.txt\0",
            "UU app/clash.rs\0",
            "?? app/notes/\0",
            "?? app/scratch.txt\0",
            " M elsewhere.rs\0",
        );

        let statuses = parse(dir, "app/", porcelain);
        let file = |path: &str| statuses.file(&dir.join(path));
        let folder = |path: &str| statuses.folder(&dir.join(path));

        assert_eq!(file("src/main.rs"), Some(FileStatus::Modified));
        assert_eq!(file("src/new.rs"), Some(FileStatus::Added));
        assert_eq!(file("docs/after.md"), Some(FileStatus::Renamed));
        assert_eq!(file("docs/before.md"), None);
        assert_eq!(file("old.txt"), Some(FileStatus::Deleted));
        assert_eq!(file("clash.rs"), Some(FileStatus::Conflicted));
        assert_eq!(file("scratch.txt"), Some(FileStatus::Untracked));
        assert_eq!(file("README.md"), None);

        // A file in an untracked folder is untracked too.
        assert_eq!(file("notes/deep/idea.md"), Some(FileStatus::Untracked));
        assert_eq!(folder("notes"), Some(FileStatus::Untracked));

        // A folder shows the most of what it holds; the one asked, all.
        assert_eq!(folder("src"), Some(FileStatus::Added));
        assert_eq!(folder("docs"), Some(FileStatus::Renamed));
        assert_eq!(statuses.folder(dir), Some(FileStatus::Conflicted));
        assert_eq!(folder("clean"), None);

        // What is outside the folder asked is left out.
        assert_eq!(statuses.file(Path::new("/work/elsewhere.rs")), None);
    }

    #[test]
    fn lets_a_repository_nested_in_another_speak_for_its_files() {
        let outer = Path::new("/work/app");
        let inner = Path::new("/work/app/vendor/lib");
        let statuses = HashMap::from([
            (
                outer.to_path_buf(),
                parse(outer, "", "?? vendor/\0 M main.rs\0"),
            ),
            (inner.to_path_buf(), parse(inner, "", " M lib.rs\0")),
        ]);

        let at = |path: &str, folder| status_at(&statuses, Path::new(path), folder);

        assert_eq!(at("/work/app/main.rs", false), Some(FileStatus::Modified));
        assert_eq!(
            at("/work/app/vendor/lib/lib.rs", false),
            Some(FileStatus::Modified)
        );
        assert_eq!(at("/work/app/vendor/lib/clean.rs", false), None);
        assert_eq!(at("/work/app/vendor/lib", true), Some(FileStatus::Modified));
        assert_eq!(at("/work/app/vendor", true), Some(FileStatus::Untracked));
        assert_eq!(at("/elsewhere/file.rs", false), None);
    }

    #[test]
    fn asks_git_for_the_statuses_under_a_folder_of_a_repository() {
        let scratch = Scratch::new("statuses");

        scratch.git(&["init", "-q", "-b", "main"]);
        fs::create_dir_all(scratch.0.join("app/src")).unwrap();
        fs::write(scratch.0.join("app/src/kept.rs"), "one\n").unwrap();
        fs::write(scratch.0.join("app/gone.txt"), "bye\n").unwrap();
        fs::write(scratch.0.join("top.txt"), "top\n").unwrap();
        scratch.git(&["add", "."]);
        scratch.git(&["commit", "-q", "-m", "first"]);

        fs::write(scratch.0.join("app/src/kept.rs"), "two\n").unwrap();
        fs::remove_file(scratch.0.join("app/gone.txt")).unwrap();
        fs::create_dir_all(scratch.0.join("app/new")).unwrap();
        fs::write(scratch.0.join("app/new/file.rs"), "new\n").unwrap();
        fs::write(scratch.0.join("top.txt"), "changed\n").unwrap();

        let app = scratch.0.join("app");
        let statuses = statuses(&app).unwrap();

        assert_eq!(
            statuses.file(&app.join("src/kept.rs")),
            Some(FileStatus::Modified)
        );
        assert_eq!(
            statuses.file(&app.join("gone.txt")),
            Some(FileStatus::Deleted)
        );
        assert_eq!(
            statuses.file(&app.join("new/file.rs")),
            Some(FileStatus::Untracked)
        );
        assert_eq!(statuses.folder(&app), Some(FileStatus::Deleted));
        assert_eq!(statuses.file(&scratch.0.join("top.txt")), None);

        let plain = Scratch::new("statuses-plain");

        assert!(super::statuses(&plain.0).is_err());
    }
}
