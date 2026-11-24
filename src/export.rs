//! A session's conversation as Markdown, for a person to keep or share, as
//! rumbo's terminal UI exports it: what was asked, what the model wrote,
//! each tool call with what it returned, and how each run that did not
//! finish ended. It is written from what the window shows.

use crate::conversation::{Item, Kind, Source, first_line};
use crate::picture::{self, Picture};

/// How much of a tool's result is kept; the rest is said to be left out.
const RESULT_LINES: usize = 20;

/// The conversation of a session titled `title` as a Markdown document.
pub fn markdown(title: &str, items: &[Item]) -> String {
    let mut out = format!("# {title}\n");

    for item in items {
        out.push('\n');

        match &item.kind {
            Kind::Task(text, images) => {
                out.push_str("## Task\n\n");
                out.push_str(&quoted(text));
                out.push_str(&given(images));
            }

            Kind::Steer(text, images) => {
                out.push_str("**Told the run:**\n\n");
                out.push_str(&quoted(text));
                out.push_str(&given(images));
            }

            Kind::Reply(text) => {
                out.push_str(text.trim_end());
                out.push('\n');
            }

            Kind::Tool(tool) => {
                let state = match (&tool.result, tool.failed) {
                    (_, true) => " (failed)",
                    (None, false) => " (not finished)",
                    _ => "",
                };

                out.push_str(&format!("**{}**{state}\n\n", first_line(&tool.summary)));

                if !tool.args.is_empty() && tool.args.contains('\n') {
                    out.push_str(&fenced(&tool.args));
                }

                if let Some(result) = &tool.result {
                    out.push_str(&fenced(&shortened(result)));
                }
            }

            Kind::Context(sections) => {
                out.push_str(&format!("_Context sent: {}_\n", sections.join(", ")));
            }

            Kind::Log { source, text } => {
                let from = match source {
                    Source::Host => "host",
                    Source::Provider => "provider",
                    Source::Extension(_) => "extension",
                };

                out.push_str(&format!("_[{from}] {}_\n", first_line(text)));
            }

            Kind::Stopped {
                cancelled: true, ..
            } => out.push_str("_Cancelled._\n"),

            Kind::Stopped { reason, .. } => {
                out.push_str(&format!("_Failed: {}_\n", first_line(reason)));
            }

            Kind::Finished { .. } => {}

            Kind::Answer { command, text, .. } => {
                out.push_str(&format!("**{command}**\n\n"));
                out.push_str(&fenced(text));
            }
        }
    }

    out
}

/// A file name for a session's export, from its title.
pub fn file_name(title: &str) -> String {
    let slug: String = title
        .chars()
        .map(|c| match c.is_alphanumeric() {
            true => c.to_ascii_lowercase(),
            false => '-',
        })
        .collect();

    let slug: Vec<&str> = slug.split('-').filter(|part| !part.is_empty()).collect();
    let slug = slug.join("-");
    let slug: String = slug.chars().take(60).collect();

    match slug.is_empty() {
        true => "rumbo-session.md".into(),
        false => format!("rumbo-{slug}.md"),
    }
}

fn quoted(text: &str) -> String {
    text.trim_end()
        .lines()
        .map(|line| match line.is_empty() {
            true => ">\n".to_string(),
            false => format!("> {line}\n"),
        })
        .collect()
}

/// Text in a fence longer than any run of backticks in it.
fn fenced(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);

    format!("{fence}\n{}\n{fence}\n", text.trim_end())
}

fn shortened(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();

    if lines.len() <= RESULT_LINES {
        return text.to_string();
    }

    format!(
        "{}\n… {} more lines",
        lines[..RESULT_LINES].join("\n"),
        lines.len() - RESULT_LINES
    )
}

/// The images a person gave, which a document names, not shows.
fn given(images: &[Picture]) -> String {
    if images.is_empty() {
        return String::new();
    }

    let names: Vec<String> = images
        .iter()
        .enumerate()
        .map(|(index, image)| {
            let kind = picture::media_type(&image.image).unwrap_or("image");

            format!(
                "[image {}: {}]",
                index + 1,
                kind.trim_start_matches("image/")
            )
        })
        .collect();

    format!("\n*{}*\n", names.join("  "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::Tool;

    fn item(kind: Kind) -> Item {
        Item { id: 1, kind }
    }

    #[test]
    fn writes_the_tasks_answers_and_tool_calls_in_order() {
        let items = [
            item(Kind::Task("Add a flag\nto export".into(), vec![])),
            item(Kind::Tool(Tool {
                name: "bash".into(),
                summary: "bash ls".into(),
                args: "command: ls".into(),
                change: Vec::new(),
                progress: String::new(),
                result: Some("a.txt\nb.txt".into()),
                failed: false,
            })),
            item(Kind::Reply("Done, see `a.txt`.".into())),
            item(Kind::Stopped {
                cancelled: true,
                reason: String::new(),
            }),
        ];

        assert_eq!(
            markdown("Flags", &items),
            "# Flags\n\n## Task\n\n> Add a flag\n> to export\n\n**bash ls**\n\n```\na.txt\nb.txt\n```\n\nDone, see `a.txt`.\n\n_Cancelled._\n"
        );
    }

    #[test]
    fn names_the_images_a_person_gave() {
        let png = Picture::new(gpui::Image::from_bytes(
            gpui::ImageFormat::Png,
            b"png".to_vec(),
        ));
        let items = [item(Kind::Task("What is this?".into(), vec![png]))];

        assert_eq!(
            markdown("Pictures", &items),
            "# Pictures\n\n## Task\n\n> What is this?\n\n*[image 1: png]*\n"
        );
    }

    #[test]
    fn fences_text_that_holds_backticks_with_more_of_them() {
        assert_eq!(
            fenced("```rust\nfn x() {}\n```"),
            "````\n```rust\nfn x() {}\n```\n````\n"
        );
    }

    #[test]
    fn keeps_the_start_of_a_long_result() {
        let long: Vec<String> = (1..=25).map(|n| n.to_string()).collect();
        let kept = shortened(&long.join("\n"));

        assert!(kept.starts_with("1\n2\n"));
        assert!(kept.ends_with("… 5 more lines"));
    }

    #[test]
    fn names_the_file_after_the_title() {
        assert_eq!(file_name("Add a --json flag!"), "rumbo-add-a-json-flag.md");
        assert_eq!(file_name("¿?"), "rumbo-session.md");
    }
}
