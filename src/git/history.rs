//! The commits of the branch checked out, or of another branch, local or
//! remote, read without checking it out or asking a remote.

use std::path::Path;

use super::{git, repository, run};

/// What the history follows when no branch is chosen: the commit checked
/// out.
pub const HEAD: &str = "HEAD";

/// How many commits are read at a time.
pub const PAGE: usize = 100;

/// A branch whose commits can be shown.
#[derive(Debug)]
pub struct Branch {
    /// Its full name, such as `refs/heads/main`, which tells a local branch
    /// from a remote one of the same name.
    pub reference: String,
    /// Its name as Git shows it, such as `main` or `origin/main`.
    pub name: String,
    pub remote: bool,
}

#[derive(Debug)]
pub struct Commit {
    /// The abbreviated hash.
    pub hash: String,
    /// The day it was written, as `YYYY-MM-DD`.
    pub date: String,
    pub author: String,
    /// The first line of its message.
    pub subject: String,
}

#[derive(Debug)]
pub struct History {
    /// The branch checked out, or none on a detached head.
    pub current: Option<String>,
    pub branches: Vec<Branch>,
    /// What the commits are of: `HEAD`, or the reference of a branch.
    pub selected: String,
    /// The newest first.
    pub commits: Vec<Commit>,
    /// More commits come after the last one read.
    pub more: bool,
}

/// Reads the branches of the repository `dir` is in, and the newest `limit`
/// commits of `selection`: `HEAD`, or the reference of one of them. A
/// branch gone since it was chosen gives the commits of `HEAD` instead.
pub fn read(dir: &Path, selection: &str, limit: usize) -> Result<History, String> {
    repository(dir, "history")?;

    let refs = git(
        dir,
        &[
            "for-each-ref",
            "--sort=refname",
            "--format=%(refname)%00%(objectname)%00%(symref)",
            "refs/heads/",
            "refs/remotes/",
        ],
    )?;

    // Fails on a detached head, which has no branch to name.
    let current = run(dir, &["symbolic-ref", "--short", "-q", HEAD], &[0, 1])?;

    let mut branches = Vec::new();
    let mut tip = None;

    for line in refs.lines() {
        let mut fields = line.split('\0');

        // A symbolic reference, such as `origin/HEAD`, names another
        // branch, which is listed already.
        let (Some(reference), Some(hash), Some("")) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };

        let (name, remote) = match reference.strip_prefix("refs/heads/") {
            Some(name) => (name, false),
            None => match reference.strip_prefix("refs/remotes/") {
                Some(name) => (name, true),
                None => continue,
            },
        };

        if selection == reference {
            tip = Some(hash);
        }

        branches.push(Branch {
            reference: reference.into(),
            name: name.into(),
            remote,
        });
    }

    // Git is given the commit the listing found, never what was asked for,
    // which cannot then be taken for an option. A repository without a
    // commit has no HEAD, and so no history yet.
    let (selected, hash) = match tip {
        Some(hash) => (selection.to_string(), hash.to_string()),
        None => (
            HEAD.to_string(),
            run(dir, &["rev-parse", "--verify", "--quiet", HEAD], &[0, 1])?,
        ),
    };
    let hash = hash.trim();

    let mut commits = Vec::new();

    if !hash.is_empty() {
        // One more than asked for tells whether there are more.
        let log = git(
            dir,
            &[
                "log",
                "--no-color",
                "--no-show-signature",
                "--no-notes",
                "--no-decorate",
                "-z",
                "--format=%h%x00%as%x00%an%x00%s",
                &format!("--max-count={}", limit + 1),
                hash,
                "--",
            ],
        )?;
        let mut fields = log.split_terminator('\0');

        while let (Some(hash), Some(date), Some(author), Some(subject)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        {
            commits.push(Commit {
                hash: hash.into(),
                date: date.into(),
                author: author.into(),
                subject: subject.into(),
            });
        }
    }

    let more = commits.len() > limit;

    commits.truncate(limit);

    let current = current.trim();

    Ok(History {
        current: (!current.is_empty()).then(|| current.into()),
        branches,
        selected,
        commits,
        more,
    })
}
