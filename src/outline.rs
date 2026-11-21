//! What a function of the host says in words, laid out by its shape rather
//! than its meaning: rumbo writes a line as a heading when it ends in a
//! colon, as `label: value` when it names something, and sets in under a
//! line what belongs to it. The words are kept as they are; only where each
//! goes is read, so a host that words things differently still shows, as
//! lines of words.

/// The longest a heading or a label is: a longer line is a sentence.
const SHORT: usize = 48;

/// A part of what was said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// What the lines after it are, as "left out".
    Heading(String),
    /// A line that names something and says what it is.
    Field { label: String, value: String },
    /// A line set in under no other, with the lines set in under it: what
    /// it names, what it says of it, and its details.
    Item {
        title: String,
        text: String,
        details: Vec<String>,
    },
    /// Words that are none of those.
    Paragraph(String),
}

/// Reads what was said into its parts.
pub fn read(said: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut item_depth = None;

    for line in said.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            item_depth = None;
            continue;
        }

        let depth = line.len() - line.trim_start().len();

        // What is set in deeper than an item belongs to it.
        if let (Some(at), Some(Block::Item { details, .. })) = (item_depth, blocks.last_mut())
            && depth > at
        {
            details.push(trimmed.to_string());
            continue;
        }

        item_depth = None;

        if depth > 0 {
            let (title, text) = match labeled(trimmed) {
                Some((label, value)) => (label, value),
                None => (trimmed.to_string(), String::new()),
            };

            blocks.push(Block::Item {
                title,
                text,
                details: Vec::new(),
            });
            item_depth = Some(depth);
            continue;
        }

        let block = match (trimmed.strip_suffix(':'), labeled(trimmed)) {
            (Some(heading), _) if heading.len() <= SHORT => Block::Heading(heading.to_string()),
            (_, Some((label, value))) => Block::Field { label, value },
            _ => Block::Paragraph(trimmed.to_string()),
        };

        blocks.push(block);
    }

    blocks
}

/// A line's label and value, when it names something: a short label, which
/// is no sentence, before a colon and a space, and something after them.
fn labeled(line: &str) -> Option<(String, String)> {
    let (label, value) = line.split_once(": ")?;
    let value = value.trim();

    let short = label.len() <= SHORT && !label.contains(". ");

    (short && !label.is_empty() && !value.is_empty())
        .then(|| (label.trim().to_string(), value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(label: &str, value: &str) -> Block {
        Block::Field {
            label: label.into(),
            value: value.into(),
        }
    }

    fn item(title: &str, text: &str, details: &[&str]) -> Block {
        Block::Item {
            title: title.into(),
            text: text.into(),
            details: details.iter().map(|detail| detail.to_string()).collect(),
        }
    }

    #[test]
    fn reads_what_names_something_as_fields() {
        let said = "provider: toy\n\
                    options chosen in the session: none\n\
                    context: 59665 tokens of 1000000, 5%; summarized at 800000\n\
                    the last run did not finish, and can be resumed";

        assert_eq!(
            read(said),
            [
                field("provider", "toy"),
                field("options chosen in the session", "none"),
                field(
                    "context",
                    "59665 tokens of 1000000, 5%; summarized at 800000"
                ),
                Block::Paragraph("the last run did not finish, and can be resumed".into()),
            ]
        );
    }

    #[test]
    fn reads_what_is_set_in_as_items_with_their_details() {
        let said = "the model is told of these skills:\n  \
                    review: Read a change as a reviewer would\n    \
                    /home/me/.config/rumbo/skills/review/SKILL.md\n\
                    left out:\n  \
                    /home/me/project/.agents/skills/broken\n    \
                    its SKILL.md has no name";

        assert_eq!(
            read(said),
            [
                Block::Heading("the model is told of these skills".into()),
                item(
                    "review",
                    "Read a change as a reviewer would",
                    &["/home/me/.config/rumbo/skills/review/SKILL.md"]
                ),
                Block::Heading("left out".into()),
                item(
                    "/home/me/project/.agents/skills/broken",
                    "",
                    &["its SKILL.md has no name"]
                ),
            ]
        );
    }

    #[test]
    fn a_long_line_ending_in_a_colon_is_a_sentence() {
        let said = "no skills here. A skill is a directory with a SKILL.md, in one of:\n  \
                    /home/me/.config/rumbo/skills\n  \
                    /home/me/project/.agents/skills";

        assert_eq!(
            read(said),
            [
                Block::Paragraph(
                    "no skills here. A skill is a directory with a SKILL.md, in one of:".into()
                ),
                item("/home/me/.config/rumbo/skills", "", &[]),
                item("/home/me/project/.agents/skills", "", &[]),
            ]
        );
    }

    #[test]
    fn a_sentence_with_a_colon_is_no_field() {
        assert_eq!(
            read("The host ended. It said this: nothing"),
            [Block::Paragraph(
                "The host ended. It said this: nothing".into()
            )]
        );
        assert_eq!(read(""), []);
    }
}
