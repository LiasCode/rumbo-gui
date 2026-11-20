//! What a markdown text says, as blocks of words with their marks, apart
//! from drawing it: paragraphs, headings, lists of their own blocks, code,
//! quotes, rules, and tables, as GitHub's flavor writes them.

use std::ops::Range;

use markdown::mdast::{AlignKind, Node};
use markdown::{ParseOptions, to_mdast};

/// How a run of words is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Marks {
    pub(crate) strong: bool,
    pub(crate) emphasis: bool,
    pub(crate) code: bool,
    pub(crate) delete: bool,
    pub(crate) link: bool,
}

impl Marks {
    pub(crate) fn any(self) -> bool {
        self != Marks::default()
    }
}

/// Words of one block, with the runs of them that are marked and the
/// links some of them go to.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Inline {
    pub(crate) text: String,
    /// Marked runs, in order, apart from each other.
    pub(crate) spans: Vec<(Range<usize>, Marks)>,
    pub(crate) links: Vec<(Range<usize>, String)>,
}

impl Inline {
    /// Adds `words` marked `marks` after what it holds.
    fn push(&mut self, words: &str, marks: Marks) {
        let start = self.text.len();

        self.text.push_str(words);

        if !marks.any() || words.is_empty() {
            return;
        }

        let end = self.text.len();

        // A run marked as the one before it goes on from it.
        match self.spans.last_mut() {
            Some((range, last)) if range.end == start && *last == marks => range.end = end,
            _ => self.spans.push((start..end, marks)),
        }
    }
}

/// Where a column's cells stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Align {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Item {
    /// For a list of tasks, whether it is done.
    pub(crate) checked: Option<bool>,
    pub(crate) blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Block {
    Paragraph(Inline),
    Heading(u8, Inline),
    Code(String),
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: usize,
        items: Vec<Item>,
    },
    Rule,
    /// The titles first, then the rows.
    Table {
        align: Vec<Align>,
        rows: Vec<Vec<Inline>>,
    },
}

/// The blocks `source` says. What it cannot read is said as it is written.
pub(crate) fn parse(source: &str) -> Vec<Block> {
    match to_mdast(source, &ParseOptions::gfm()) {
        Ok(root) => blocks(root.children().map(Vec::as_slice).unwrap_or_default()),
        Err(_) => vec![Block::Paragraph(Inline {
            text: source.to_string(),
            ..Inline::default()
        })],
    }
}

fn blocks(nodes: &[Node]) -> Vec<Block> {
    nodes.iter().filter_map(block).collect()
}

fn block(node: &Node) -> Option<Block> {
    let block = match node {
        Node::Paragraph(paragraph) => Block::Paragraph(inline(&paragraph.children)),
        Node::Heading(heading) => Block::Heading(heading.depth, inline(&heading.children)),
        Node::Code(code) => Block::Code(code.value.clone()),
        Node::Math(math) => Block::Code(math.value.clone()),
        Node::Blockquote(quote) => Block::Quote(blocks(&quote.children)),
        Node::ThematicBreak(_) => Block::Rule,

        Node::List(list) => Block::List {
            ordered: list.ordered,
            start: list.start.unwrap_or(1) as usize,
            items: list
                .children
                .iter()
                .filter_map(|node| match node {
                    Node::ListItem(item) => Some(Item {
                        checked: item.checked,
                        blocks: blocks(&item.children),
                    }),
                    _ => None,
                })
                .collect(),
        },

        Node::Table(table) => Block::Table {
            align: table
                .align
                .iter()
                .map(|align| match align {
                    AlignKind::Center => Align::Center,
                    AlignKind::Right => Align::End,
                    AlignKind::Left | AlignKind::None => Align::Start,
                })
                .collect(),
            rows: table
                .children
                .iter()
                .filter_map(|row| match row {
                    Node::TableRow(row) => Some(
                        row.children
                            .iter()
                            .map(|cell| match cell {
                                Node::TableCell(cell) => inline(&cell.children),
                                other => inline(std::slice::from_ref(other)),
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .collect(),
        },

        Node::Html(html) => Block::Paragraph(Inline {
            text: html.value.clone(),
            ..Inline::default()
        }),

        // Anything else that holds words, as a paragraph of them.
        other => {
            let words = inline(std::slice::from_ref(other));

            if words.text.is_empty() {
                return None;
            }

            Block::Paragraph(words)
        }
    };

    Some(block)
}

fn inline(nodes: &[Node]) -> Inline {
    let mut words = Inline::default();

    for node in nodes {
        walk(node, Marks::default(), &mut words);
    }

    words
}

fn walk(node: &Node, marks: Marks, words: &mut Inline) {
    let within = |words: &mut Inline, children: &[Node], marks: Marks| {
        for child in children {
            walk(child, marks, words);
        }
    };

    match node {
        Node::Text(text) => words.push(&text.value, marks),
        Node::Strong(strong) => within(
            words,
            &strong.children,
            Marks {
                strong: true,
                ..marks
            },
        ),
        Node::Emphasis(emphasis) => within(
            words,
            &emphasis.children,
            Marks {
                emphasis: true,
                ..marks
            },
        ),
        Node::Delete(delete) => within(
            words,
            &delete.children,
            Marks {
                delete: true,
                ..marks
            },
        ),
        Node::InlineCode(code) => words.push(
            &code.value,
            Marks {
                code: true,
                ..marks
            },
        ),
        Node::InlineMath(math) => words.push(
            &math.value,
            Marks {
                code: true,
                ..marks
            },
        ),
        Node::Break(_) => words.push("\n", marks),
        Node::Html(html) => words.push(&html.value, marks),
        Node::Image(image) => words.push(&image.alt, marks),

        Node::Link(link) => {
            let start = words.text.len();

            within(
                words,
                &link.children,
                Marks {
                    link: true,
                    ..marks
                },
            );
            words
                .links
                .push((start..words.text.len(), link.url.clone()));
        }

        other => {
            if let Some(children) = other.children() {
                within(words, children, marks);
            }
        }
    }
}

/// The runs of `0..len` that are marked or selected, in order and apart
/// from each other, with their marks and whether they are selected, as a
/// line of text is drawn.
pub(crate) fn segments(
    len: usize,
    spans: &[(Range<usize>, Marks)],
    selected: Option<Range<usize>>,
) -> Vec<(Range<usize>, Marks, bool)> {
    let selected = selected.filter(|range| range.start < range.end);
    let mut cuts = vec![0, len];

    for (range, _) in spans {
        cuts.push(range.start.min(len));
        cuts.push(range.end.min(len));
    }

    if let Some(range) = &selected {
        cuts.push(range.start.min(len));
        cuts.push(range.end.min(len));
    }

    cuts.sort_unstable();
    cuts.dedup();

    cuts.windows(2)
        .filter_map(|cut| {
            let range = cut[0]..cut[1];
            let marks = spans
                .iter()
                .find(|(span, _)| span.start <= range.start && range.end <= span.end)
                .map(|(_, marks)| *marks)
                .unwrap_or_default();
            let chosen = selected.as_ref().is_some_and(|selection| {
                selection.start <= range.start && range.end <= selection.end
            });

            (marks.any() || chosen).then_some((range, marks, chosen))
        })
        .collect()
}

/// The word `index` stands in, or what is not a word there.
pub(crate) fn word_at(text: &str, index: usize) -> Range<usize> {
    let index = index.min(text.len());
    let wordy = |c: char| c.is_alphanumeric() || c == '_';
    let after = text[index..].chars().next();
    let before = text[..index].chars().next_back();

    let kind = match (before, after) {
        (_, Some(c)) if wordy(c) => true,
        (Some(c), _) if wordy(c) => true,
        _ => false,
    };

    let start = text[..index]
        .char_indices()
        .rev()
        .take_while(|(_, c)| wordy(*c) == kind && *c != '\n')
        .last()
        .map(|(at, _)| at)
        .unwrap_or(index);
    let end = text[index..]
        .char_indices()
        .take_while(|(_, c)| wordy(*c) == kind && *c != '\n')
        .last()
        .map(|(at, c)| index + at + c.len_utf8())
        .unwrap_or(index);

    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(blocks: &[Block], index: usize) -> &Inline {
        match &blocks[index] {
            Block::Paragraph(words) => words,
            other => panic!("not a paragraph: {other:?}"),
        }
    }

    #[test]
    fn marks_the_words_of_a_paragraph() {
        let blocks =
            parse("Run **the checks** and read `cargo test`, then [the docs](https://rumbo.dev).");
        let words = paragraph(&blocks, 0);

        assert_eq!(
            words.text,
            "Run the checks and read cargo test, then the docs."
        );
        assert_eq!(
            words.spans,
            [
                (
                    4..14,
                    Marks {
                        strong: true,
                        ..Marks::default()
                    }
                ),
                (
                    24..34,
                    Marks {
                        code: true,
                        ..Marks::default()
                    }
                ),
                (
                    41..49,
                    Marks {
                        link: true,
                        ..Marks::default()
                    }
                ),
            ]
        );
        assert_eq!(words.links, [(41..49, "https://rumbo.dev".to_string())]);
    }

    #[test]
    fn keeps_marks_that_hold_each_other() {
        let blocks = parse("*all **of** it*");
        let words = paragraph(&blocks, 0);
        let emphasis = Marks {
            emphasis: true,
            ..Marks::default()
        };

        assert_eq!(words.text, "all of it");
        assert_eq!(
            words.spans,
            [
                (0..4, emphasis),
                (
                    4..6,
                    Marks {
                        strong: true,
                        ..emphasis
                    }
                ),
                (6..9, emphasis),
            ]
        );
    }

    #[test]
    fn reads_headings_code_quotes_and_rules() {
        let blocks = parse("# Plan\n\n```rust\nfn main() {}\n```\n\n> said\n\n---");

        assert!(matches!(&blocks[0], Block::Heading(1, words) if words.text == "Plan"));
        assert_eq!(blocks[1], Block::Code("fn main() {}".to_string()));
        assert!(matches!(&blocks[2], Block::Quote(inner) if inner.len() == 1));
        assert_eq!(blocks[3], Block::Rule);
    }

    #[test]
    fn reads_lists_within_lists_and_tasks() {
        let blocks = parse("3. one\n4. two\n   - deeper\n\n- [x] done\n- [ ] not yet");

        let Block::List {
            ordered,
            start,
            items,
        } = &blocks[0]
        else {
            panic!("not a list: {:?}", blocks[0]);
        };

        assert!(*ordered);
        assert_eq!(*start, 3);
        assert_eq!(items.len(), 2);
        assert!(matches!(
            &items[1].blocks[1],
            Block::List { ordered: false, .. }
        ));

        let Block::List { items, .. } = &blocks[1] else {
            panic!("not a list: {:?}", blocks[1]);
        };

        assert_eq!(items[0].checked, Some(true));
        assert_eq!(items[1].checked, Some(false));
    }

    #[test]
    fn reads_a_table_with_its_alignment() {
        let blocks = parse("| file | lines |\n|:-----|------:|\n| a.rs | 12 |");

        let Block::Table { align, rows } = &blocks[0] else {
            panic!("not a table: {:?}", blocks[0]);
        };

        assert_eq!(align, &[Align::Start, Align::End]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][1].text, "12");
    }

    #[test]
    fn cuts_marked_runs_where_the_selection_starts_and_ends() {
        let strong = Marks {
            strong: true,
            ..Marks::default()
        };

        assert_eq!(
            segments(10, &[(2..6, strong)], Some(4..8)),
            [
                (2..4, strong, false),
                (4..6, strong, true),
                (6..8, Marks::default(), true)
            ]
        );
        assert_eq!(segments(10, &[], None), []);

        // A selection of nothing marks nothing.
        assert_eq!(segments(10, &[], Some(3..3)), []);
    }

    #[test]
    fn finds_the_word_under_the_pointer() {
        let text = "fn read_changes(dir: &Path)";

        assert_eq!(&text[word_at(text, 5)], "read_changes");
        assert_eq!(&text[word_at(text, 3)], "read_changes");
        assert_eq!(&text[word_at(text, 0)], "fn");
        assert_eq!(&text[word_at(text, 16)], "dir");

        // Just past a word, it is that word; between words, what is there.
        assert_eq!(&text[word_at(text, 15)], "read_changes");
        assert_eq!(&text[word_at(text, 20)], ": &");
        assert_eq!(&text[word_at(text, text.len())], ")");
    }
}
