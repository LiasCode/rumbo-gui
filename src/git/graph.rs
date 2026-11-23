//! The commits of every branch as `git log --oneline --decorate --all
//! --graph` draws them: the lines of the graph in the colors Git gives
//! them, and each commit with what points at it. The window only reads.

use std::path::Path;

use super::{git, repository};

/// Separates the graph from the commit on a line Git writes, and the
/// commit's fields. No hash or reference name holds it; a subject that
/// does keeps it as part of itself.
const SEPARATOR: char = '\x1f';

/// A color Git gives a line of the graph, as a terminal names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// One of a terminal's 256: the usual eight, their bright ones, then
    /// a cube of colors and a ramp of grays.
    Indexed(u8),
    Rgb(u8, u8, u8),
}

/// A part of a line of the graph in one color, or in the text's own when
/// Git gives it none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stroke {
    pub text: String,
    pub color: Option<Color>,
}

/// What points at a commit, as `--decorate` says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Label {
    /// The commit checked out, and the branch checked out, if any.
    Head(Option<String>),
    Branch(String),
    /// A remote's branch, such as `origin/main`.
    Remote(String),
    Tag(String),
    /// Any other reference, such as `refs/stash`, by its full name.
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// The abbreviated hash.
    pub hash: String,
    pub labels: Vec<Label>,
    /// The first line of its message.
    pub subject: String,
}

/// One line Git draws: the lines of the graph, and the commit on it, if
/// any; a line without one only joins the commits above and below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub graph: Vec<Stroke>,
    pub commit: Option<Commit>,
}

#[derive(Debug)]
pub struct Graph {
    pub lines: Vec<Line>,
    /// More commits come after the last one read.
    pub more: bool,
}

/// Reads the newest `limit` commits of every branch of the repository
/// `dir` is in, as a graph.
pub fn read(dir: &Path, limit: usize) -> Result<Graph, String> {
    repository(dir, "history")?;

    // One more than asked for tells whether there are more. The full names
    // of what points at a commit tell a branch from a remote's or a tag.
    let log = git(
        dir,
        &[
            "log",
            "--graph",
            "--all",
            "--decorate=full",
            "--color=always",
            "--no-show-signature",
            "--no-notes",
            "--format=%x1f%h%x1f%D%x1f%s",
            &format!("--max-count={}", limit + 1),
            "--",
        ],
    )?;

    Ok(parse(&log, limit))
}

/// The lines of what Git wrote, up to the commit after the `limit`th.
fn parse(log: &str, limit: usize) -> Graph {
    let mut lines = Vec::new();
    let mut commits = 0;

    for text in log.lines() {
        let (graph, commit) = match text.split_once(SEPARATOR) {
            Some((graph, fields)) => (graph, Some(commit(fields))),
            None => (text, None),
        };

        if commit.is_some() {
            commits += 1;

            if commits > limit {
                return Graph { lines, more: true };
            }
        }

        lines.push(Line {
            graph: strokes(graph),
            commit,
        });
    }

    Graph { lines, more: false }
}

/// A commit from its fields: its hash, what points at it, and its subject.
fn commit(fields: &str) -> Commit {
    let mut fields = fields.splitn(3, SEPARATOR);
    let hash = fields.next().unwrap_or_default();
    let decorations = fields.next().unwrap_or_default();
    let subject = fields.next().unwrap_or_default();

    Commit {
        hash: hash.into(),
        labels: decorations
            .split(", ")
            .filter(|decoration| !decoration.is_empty())
            .map(label)
            .collect(),
        subject: subject.into(),
    }
}

/// What a decoration names, from the full name `--decorate=full` gives.
fn label(decoration: &str) -> Label {
    let branch = |name: &str| name.strip_prefix("refs/heads/").unwrap_or(name).to_string();

    if decoration == "HEAD" {
        return Label::Head(None);
    }

    if let Some(name) = decoration.strip_prefix("HEAD -> ") {
        return Label::Head(Some(branch(name)));
    }

    if let Some(name) = decoration.strip_prefix("tag: ") {
        return Label::Tag(name.strip_prefix("refs/tags/").unwrap_or(name).into());
    }

    if let Some(name) = decoration.strip_prefix("refs/heads/") {
        return Label::Branch(name.into());
    }

    if let Some(name) = decoration.strip_prefix("refs/remotes/") {
        return Label::Remote(name.into());
    }

    Label::Other(decoration.into())
}

/// The text of a line of the graph, in the colors of the escape sequences
/// Git puts in it.
fn strokes(text: &str) -> Vec<Stroke> {
    let mut strokes: Vec<Stroke> = Vec::new();
    let mut pen = Pen::default();
    let mut rest = text;

    loop {
        let (plain, sequence) = match rest.find("\x1b[") {
            Some(at) => (&rest[..at], Some(&rest[at + 2..])),
            None => (rest, None),
        };

        let color = pen.color();

        match strokes.last_mut() {
            Some(last) if last.color == color => last.text.push_str(plain),
            _ if !plain.is_empty() => strokes.push(Stroke {
                text: plain.into(),
                color,
            }),
            _ => {}
        }

        let Some(sequence) = sequence else {
            break;
        };

        // A sequence is numbers separated by semicolons, then the letter
        // that says what it does; only `m` sets the colors.
        let end = sequence
            .find(|character: char| !character.is_ascii_digit() && character != ';')
            .unwrap_or(sequence.len());

        if sequence[end..].starts_with('m') {
            pen.apply(&sequence[..end]);
        }

        rest = sequence.get(end + 1..).unwrap_or_default();
    }

    strokes
}

/// The color a terminal would draw with, as the sequences so far set it.
#[derive(Default)]
struct Pen {
    color: Option<Color>,
    bold: bool,
}

impl Pen {
    fn apply(&mut self, parameters: &str) {
        // An empty parameter is a zero, which sets everything back.
        let mut codes = parameters
            .split(';')
            .map(|code| code.parse::<u32>().unwrap_or(0));

        while let Some(code) = codes.next() {
            match code {
                0 => *self = Pen::default(),
                1 => self.bold = true,
                22 => self.bold = false,
                30..=37 => self.color = Some(Color::Indexed((code - 30) as u8)),
                39 => self.color = None,
                90..=97 => self.color = Some(Color::Indexed((code - 90 + 8) as u8)),
                38 => {
                    let mut next = || codes.next().and_then(|code| u8::try_from(code).ok());

                    match next() {
                        Some(5) => self.color = next().map(Color::Indexed),
                        Some(2) => {
                            if let (Some(red), Some(green), Some(blue)) = (next(), next(), next()) {
                                self.color = Some(Color::Rgb(red, green, blue));
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    /// Bold draws one of the usual eight as its bright one, as most
    /// terminals do.
    fn color(&self) -> Option<Color> {
        match self.color {
            Some(Color::Indexed(index)) if self.bold && index < 8 => {
                Some(Color::Indexed(index + 8))
            }
            color => color,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(text: &str, color: Option<Color>) -> Stroke {
        Stroke {
            text: text.into(),
            color,
        }
    }

    #[test]
    fn colors_the_graph_as_its_escape_sequences_say() {
        assert_eq!(
            strokes("\x1b[31m|\x1b[m\x1b[32m\\\x1b[m  "),
            [
                stroke("|", Some(Color::Indexed(1))),
                stroke("\\", Some(Color::Indexed(2))),
                stroke("  ", None),
            ]
        );
        assert_eq!(
            strokes("* \x1b[1;31m|\x1b[0m \x1b[38;5;208m/\x1b[38;2;1;2;3m|\x1b[K"),
            [
                stroke("* ", None),
                stroke("|", Some(Color::Indexed(9))),
                stroke(" ", None),
                stroke("/", Some(Color::Indexed(208))),
                stroke("|", Some(Color::Rgb(1, 2, 3))),
            ]
        );
        assert_eq!(strokes("| * "), [stroke("| * ", None)]);
    }

    #[test]
    fn tells_what_points_at_a_commit() {
        assert_eq!(
            commit(
                "abc1234\x1fHEAD -> refs/heads/main, tag: refs/tags/v1, refs/remotes/origin/main, refs/stash\x1fFix it"
            ),
            Commit {
                hash: "abc1234".into(),
                labels: vec![
                    Label::Head(Some("main".into())),
                    Label::Tag("v1".into()),
                    Label::Remote("origin/main".into()),
                    Label::Other("refs/stash".into()),
                ],
                subject: "Fix it".into(),
            }
        );
        assert_eq!(
            commit("abc1234\x1fHEAD, refs/heads/topic\x1fA \x1f in it").labels,
            [Label::Head(None), Label::Branch("topic".into())]
        );
        assert_eq!(
            commit("abc1234\x1f\x1fA \x1f in it").subject,
            "A \x1f in it"
        );
    }

    #[test]
    fn keeps_the_lines_up_to_the_commit_after_the_last_one_asked_for() {
        let log = "*   \x1fa\x1f\x1fMerge\n|\\  \n| * \x1fb\x1f\x1fTopic\n* | \x1fc\x1f\x1fMain\n|/  \n* \x1fd\x1f\x1fFirst\n";

        let all = parse(log, 4);

        assert_eq!(all.lines.len(), 6);
        assert!(!all.more);
        assert_eq!(all.lines[1].commit, None);
        assert_eq!(all.lines[1].graph, [stroke("|\\  ", None)]);

        let some = parse(log, 3);

        assert_eq!(some.lines.len(), 5);
        assert!(some.more);
        assert_eq!(
            some.lines.last().map(|line| &line.graph),
            Some(&vec![stroke("|/  ", None)])
        );
    }
}
