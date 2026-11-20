//! A patch in Git's format, read into the files it changes, and the rows
//! they are drawn as, apart from drawing them.

use std::collections::HashSet;

/// What happened to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
}

/// What a line of a hunk is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// As it was, shown around the changes.
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    /// Its number before the change, if it was there.
    pub old: Option<usize>,
    /// Its number after the change, if it is there.
    pub new: Option<usize>,
    pub text: String,
}

/// A run of changed lines, with some around them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// Its first line, such as `@@ -3,7 +3,8 @@ fn main()`.
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFile {
    /// Where it is now, or where it was when it was deleted.
    pub path: String,
    /// Where it was before it was renamed.
    pub old_path: Option<String>,
    pub status: FileStatus,
    /// Whether Git only said it differs, as it says of a binary file.
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

impl DiffFile {
    /// The files `patch` changes. What it cannot read, it leaves out.
    pub fn parse(patch: &str) -> Vec<DiffFile> {
        let mut files = Vec::new();
        let mut lines = patch.lines().peekable();

        while let Some(line) = lines.next() {
            let Some(names) = line.strip_prefix("diff --git ") else {
                continue;
            };

            let (old, new) = header_paths(names);
            let mut file = DiffFile {
                path: new.clone().unwrap_or_default(),
                old_path: None,
                status: FileStatus::Modified,
                binary: false,
                hunks: Vec::new(),
            };
            let mut old_path = old;
            let mut new_path = new;

            // What Git says of the file before its hunks.
            while let Some(line) =
                lines.next_if(|line| !line.starts_with("@@ ") && !line.starts_with("diff --git "))
            {
                if line.starts_with("new file mode") {
                    file.status = FileStatus::Added;
                } else if line.starts_with("deleted file mode") {
                    file.status = FileStatus::Deleted;
                } else if let Some(from) = line.strip_prefix("rename from ") {
                    file.status = FileStatus::Renamed;
                    old_path = Some(unquote(from));
                } else if let Some(to) = line.strip_prefix("rename to ") {
                    file.status = FileStatus::Renamed;
                    new_path = Some(unquote(to));
                } else if let Some(from) = line.strip_prefix("--- ") {
                    if let Some(path) = side_path(from, "a/") {
                        old_path = Some(path);
                    }
                } else if let Some(to) = line.strip_prefix("+++ ") {
                    match side_path(to, "b/") {
                        Some(path) => new_path = Some(path),
                        None => file.status = FileStatus::Deleted,
                    }
                } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
                    file.binary = true;
                }
            }

            while let Some(header) = lines.next_if(|line| line.starts_with("@@ ")) {
                let [(mut old, mut old_left), (mut new, mut new_left)] = hunk_ranges(header);
                let mut hunk = Hunk {
                    header: header.to_string(),
                    lines: Vec::new(),
                };

                // As many lines as its header counts on each side.
                loop {
                    // "\ No newline at end of file", after the line it is of.
                    if lines.next_if(|line| line.starts_with('\\')).is_some() {
                        continue;
                    }

                    if old_left == 0 && new_left == 0 {
                        break;
                    }

                    // No line of a hunk starts as a header does.
                    let Some(line) = lines.next_if(|line| {
                        !line.starts_with("diff --git ") && !line.starts_with("@@ ")
                    }) else {
                        break;
                    };

                    let (kind, text) = match line.split_at_checked(1) {
                        Some(("+", text)) => (LineKind::Added, text),
                        Some(("-", text)) => (LineKind::Removed, text),
                        Some((_, text)) => (LineKind::Context, text),
                        // A line as it was, with nothing on it.
                        None => (LineKind::Context, ""),
                    };

                    let (old_number, new_number) = match kind {
                        LineKind::Context => (Some(old), Some(new)),
                        LineKind::Added => (None, Some(new)),
                        LineKind::Removed => (Some(old), None),
                    };

                    if old_number.is_some() {
                        old += 1;
                        old_left = old_left.saturating_sub(1);
                    }

                    if new_number.is_some() {
                        new += 1;
                        new_left = new_left.saturating_sub(1);
                    }

                    hunk.lines.push(DiffLine {
                        kind,
                        old: old_number,
                        new: new_number,
                        text: text.to_string(),
                    });
                }

                file.hunks.push(hunk);
            }

            file.path = match file.status {
                FileStatus::Deleted => old_path.clone().or(new_path).unwrap_or_default(),
                _ => new_path.or(old_path.clone()).unwrap_or_default(),
            };

            if file.status == FileStatus::Renamed {
                file.old_path = old_path;
            }

            files.push(file);
        }

        files
    }

    pub fn additions(&self) -> usize {
        self.count(LineKind::Added)
    }

    pub fn deletions(&self) -> usize {
        self.count(LineKind::Removed)
    }

    fn count(&self, kind: LineKind) -> usize {
        self.hunks
            .iter()
            .flat_map(|hunk| &hunk.lines)
            .filter(|line| line.kind == kind)
            .count()
    }
}

/// The paths of `a/old b/new` on a file's first line. They are read again
/// from the lines after it, which say them plainly; this is for a file
/// that has none, as a file whose mode alone changed.
fn header_paths(names: &str) -> (Option<String>, Option<String>) {
    if names.starts_with('"') {
        let mut parts = quoted_parts(names).into_iter();

        return (
            parts
                .next()
                .and_then(|old| old.strip_prefix("a/").map(str::to_string)),
            parts
                .next()
                .and_then(|new| new.strip_prefix("b/").map(str::to_string)),
        );
    }

    // Both sides are the same path when it was not renamed.
    let half = names.len() / 2;

    match (names.get(..half), names.get(half + 1..)) {
        (Some(old), Some(new)) if old.strip_prefix("a/") == new.strip_prefix("b/") => {
            let path = new.strip_prefix("b/").map(str::to_string);

            (path.clone(), path)
        }
        _ => (None, None),
    }
}

/// The path on a `---` or `+++` line, without the side's prefix; none for
/// `/dev/null`, the side of a file that is not there.
fn side_path(said: &str, prefix: &str) -> Option<String> {
    // Git may add a tab after a path with spaces.
    let said = said.trim_end_matches('\t');

    if said == "/dev/null" {
        return None;
    }

    let path = unquote(said);

    Some(
        path.strip_prefix(prefix)
            .map(str::to_string)
            .unwrap_or(path),
    )
}

/// Where a hunk starts and how many lines it has, before and after:
/// `@@ -3,7 +3,8 @@` starts at 3 with 7 lines, and at 3 with 8. A side
/// that does not say how many has one.
fn hunk_ranges(header: &str) -> [(usize, usize); 2] {
    let mut ranges = header.split_whitespace().skip(1).take(2).map(|range| {
        let mut numbers = range
            .get(1..)
            .unwrap_or_default()
            .split(',')
            .map(|number| number.parse().ok());

        let start = numbers.next().flatten().unwrap_or(1);
        let count = numbers.next().flatten().unwrap_or(1);

        (start, count)
    });

    [
        ranges.next().unwrap_or((1, 0)),
        ranges.next().unwrap_or((1, 0)),
    ]
}

/// A path as Git writes it: in quotes, with escapes, when it holds what
/// is not plain.
fn unquote(said: &str) -> String {
    match said.starts_with('"') {
        true => quoted_parts(said).into_iter().next().unwrap_or_default(),
        false => said.to_string(),
    }
}

/// The quoted strings in `said`, one after another, read as C reads them.
fn quoted_parts(said: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut bytes = said.bytes().peekable();

    while let Some(byte) = bytes.next() {
        if byte != b'"' {
            continue;
        }

        let mut part = Vec::new();

        while let Some(byte) = bytes.next() {
            match byte {
                b'"' => break,
                b'\\' => match bytes.next() {
                    Some(b'n') => part.push(b'\n'),
                    Some(b't') => part.push(b'\t'),
                    Some(digit @ b'0'..=b'7') => {
                        let mut value = (digit - b'0') as u32;

                        for _ in 0..2 {
                            if let Some(next) = bytes.next_if(|next| (b'0'..=b'7').contains(next)) {
                                value = value * 8 + (next - b'0') as u32;
                            }
                        }

                        part.push(value as u8);
                    }
                    Some(other) => part.push(other),
                    None => {}
                },
                other => part.push(other),
            }
        }

        parts.push(String::from_utf8_lossy(&part).into_owned());
    }

    parts
}

/// One row of a diff as drawn: a file's header, then, unless it is
/// folded, its hunks' headers and lines, or a word that it is binary or
/// changed in nothing shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Row {
    File(usize),
    Hunk(usize, usize),
    Line(usize, usize, usize),
    Binary(usize),
    Empty(usize),
}

/// The rows of `files`, the folded ones by their header alone.
pub(crate) fn rows(files: &[DiffFile], folded: &HashSet<String>) -> Vec<Row> {
    let mut rows = Vec::new();

    for (index, file) in files.iter().enumerate() {
        rows.push(Row::File(index));

        if folded.contains(&file.path) {
            continue;
        }

        if file.binary {
            rows.push(Row::Binary(index));
            continue;
        }

        if file.hunks.is_empty() {
            rows.push(Row::Empty(index));
            continue;
        }

        for (number, hunk) in file.hunks.iter().enumerate() {
            rows.push(Row::Hunk(index, number));
            rows.extend((0..hunk.lines.len()).map(|line| Row::Line(index, number, line)));
        }
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "\
diff --git a/src/main.rs b/src/main.rs
index 3b18e51..a0b1c2d 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@ fn main() {
 fn main() {
-    println!(\"hello\");
+    println!(\"hello, world\");
+    run();
 }
 
@@ -10,2 +11,2 @@ fn run() {
-old
+new
diff --git a/notes.txt b/notes.txt
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/notes.txt
@@ -0,0 +1 @@
+fresh
\\ No newline at end of file
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
index e69de29..0000000
--- a/gone.txt
+++ /dev/null
@@ -1 +0,0 @@
-gone
diff --git a/old name.txt b/new name.txt
similarity index 100%
rename from old name.txt
rename to new name.txt
diff --git a/logo.png b/logo.png
index 1111111..2222222 100644
Binary files a/logo.png and b/logo.png differ
";

    #[test]
    fn reads_each_file_and_what_happened_to_it() {
        let files = DiffFile::parse(PATCH);
        let read: Vec<_> = files
            .iter()
            .map(|file| (file.path.as_str(), file.status, file.binary))
            .collect();

        assert_eq!(
            read,
            [
                ("src/main.rs", FileStatus::Modified, false),
                ("notes.txt", FileStatus::Added, false),
                ("gone.txt", FileStatus::Deleted, false),
                ("new name.txt", FileStatus::Renamed, false),
                ("logo.png", FileStatus::Modified, true),
            ]
        );
        assert_eq!(files[3].old_path.as_deref(), Some("old name.txt"));
        assert_eq!(files[3].hunks, []);
    }

    #[test]
    fn numbers_the_lines_on_each_side() {
        let files = DiffFile::parse(PATCH);
        let main = &files[0];

        assert_eq!(main.hunks.len(), 2);
        assert_eq!(main.hunks[0].header, "@@ -1,4 +1,5 @@ fn main() {");

        let numbers: Vec<_> = main.hunks[0]
            .lines
            .iter()
            .map(|line| (line.kind, line.old, line.new))
            .collect();

        assert_eq!(
            numbers,
            [
                (LineKind::Context, Some(1), Some(1)),
                (LineKind::Removed, Some(2), None),
                (LineKind::Added, None, Some(2)),
                (LineKind::Added, None, Some(3)),
                (LineKind::Context, Some(3), Some(4)),
                (LineKind::Context, Some(4), Some(5)),
            ]
        );
        assert_eq!(main.hunks[1].lines[0].old, Some(10));
        assert_eq!(main.hunks[1].lines[1].new, Some(11));
        assert_eq!(
            main.hunks[0].lines[2].text,
            "    println!(\"hello, world\");"
        );

        // A line with nothing on it is a line as it was.
        assert_eq!(main.hunks[0].lines[5].text, "");
    }

    #[test]
    fn counts_what_was_added_and_taken_out() {
        let files = DiffFile::parse(PATCH);

        assert_eq!((files[0].additions(), files[0].deletions()), (3, 2));
        assert_eq!((files[1].additions(), files[1].deletions()), (1, 0));
        assert_eq!((files[2].additions(), files[2].deletions()), (0, 1));

        // "No newline at end of file" is no line.
        assert_eq!(files[1].hunks[0].lines.len(), 1);
    }

    #[test]
    fn ends_a_hunk_at_the_next_header_whatever_it_counted() {
        let patch = "\
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1,5 +1,5 @@
-one
+uno
@@ -9,1 +9,1 @@
-nine
+nueve
";
        let hunks = &DiffFile::parse(patch)[0].hunks;

        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].lines.len(), 2);
        assert_eq!(hunks[1].lines[0].old, Some(9));
    }

    #[test]
    fn reads_quoted_paths() {
        let patch = "\
diff --git \"a/caf\\303\\251 menu.txt\" \"b/caf\\303\\251 menu.txt\"
new file mode 100644
--- /dev/null
+++ \"b/caf\\303\\251 menu.txt\"
@@ -0,0 +1 @@
+soup
";

        assert_eq!(DiffFile::parse(patch)[0].path, "café menu.txt");
    }

    #[test]
    fn reads_nothing_from_what_is_no_patch() {
        assert_eq!(DiffFile::parse(""), []);
        assert_eq!(DiffFile::parse("fatal: not a git repository"), []);
    }

    #[test]
    fn folds_a_file_to_its_header() {
        let files = DiffFile::parse(PATCH);
        let folded = HashSet::from(["src/main.rs".to_string()]);
        let rows = rows(&files, &folded);

        assert_eq!(
            rows,
            [
                Row::File(0),
                Row::File(1),
                Row::Hunk(1, 0),
                Row::Line(1, 0, 0),
                Row::File(2),
                Row::Hunk(2, 0),
                Row::Line(2, 0, 0),
                Row::File(3),
                Row::Empty(3),
                Row::File(4),
                Row::Binary(4),
            ]
        );
    }
}
