//! What a markdown text says, as blocks of words with their marks, apart
//! from drawing it: paragraphs, headings, lists of their own blocks, code,
//! formulas, images, quotes, rules, and tables, as GitHub's flavor writes
//! them, with math between dollars.

use std::collections::HashMap;
use std::ops::Range;

use markdown::mdast::{AlignKind, Node};
use markdown::{Constructs, ParseOptions, to_mdast};

/// How a run of words is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Marks {
    pub(crate) strong: bool,
    pub(crate) emphasis: bool,
    pub(crate) code: bool,
    pub(crate) delete: bool,
    pub(crate) link: bool,
    /// A formula, whose words are its TeX.
    pub(crate) math: bool,
    /// The color of code's words, as 0xRRGGBB, where its syntax gives one.
    pub(crate) color: Option<u32>,
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

        // A run marked as the one before it goes on from it; a formula is
        // one of its own.
        match self.spans.last_mut() {
            Some((range, last)) if range.end == start && *last == marks && !marks.math => {
                range.end = end
            }
            _ => self.spans.push((start..end, marks)),
        }
    }

    /// Whether it holds a formula, which is drawn among its words.
    pub(crate) fn has_math(&self) -> bool {
        self.spans.iter().any(|(_, marks)| marks.math)
    }

    fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
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
    /// Code, with the language its fence names, if any.
    Code {
        lang: Option<String>,
        code: String,
    },
    /// A formula on a line of its own, in TeX.
    Math(String),
    /// An image, by where it is and the words that stand for it.
    Image {
        url: String,
        alt: String,
    },
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

/// What a text is read with: GitHub's flavor, and formulas between
/// dollars, one for one among words and two for one on its own.
fn options() -> ParseOptions {
    let gfm = ParseOptions::gfm();

    ParseOptions {
        constructs: Constructs {
            math_flow: true,
            math_text: true,
            ..gfm.constructs
        },
        ..gfm
    }
}

/// What a text needs to read its nodes: the text itself, for what a node
/// says only by where it stands, and the images defined by name.
struct Reader<'a> {
    source: &'a str,
    definitions: HashMap<String, String>,
}

/// The blocks `source` says. What it cannot read is said as it is written.
pub(crate) fn parse(source: &str) -> Vec<Block> {
    let root = match to_mdast(source, &options()) {
        Ok(root) => root,
        Err(_) => {
            return vec![Block::Paragraph(Inline {
                text: source.to_string(),
                ..Inline::default()
            })];
        }
    };

    let nodes = root.children().map(Vec::as_slice).unwrap_or_default();
    let mut definitions = HashMap::new();

    collect_definitions(nodes, &mut definitions);

    let reader = Reader {
        source,
        definitions,
    };

    reader.blocks(nodes)
}

fn collect_definitions(nodes: &[Node], definitions: &mut HashMap<String, String>) {
    for node in nodes {
        if let Node::Definition(definition) = node {
            definitions.insert(definition.identifier.to_lowercase(), definition.url.clone());
        }

        if let Some(children) = node.children() {
            collect_definitions(children, definitions);
        }
    }
}

impl Reader<'_> {
    fn blocks(&self, nodes: &[Node]) -> Vec<Block> {
        nodes.iter().flat_map(|node| self.block(node)).collect()
    }

    fn block(&self, node: &Node) -> Vec<Block> {
        let block = match node {
            Node::Paragraph(paragraph) => return self.paragraph(&paragraph.children),
            Node::Heading(heading) => Block::Heading(heading.depth, self.inline(&heading.children)),

            // A fence named math holds a formula, as GitHub reads it.
            Node::Code(code) if code.lang.as_deref() == Some("math") => {
                Block::Math(code.value.clone())
            }
            Node::Code(code) => Block::Code {
                lang: code.lang.clone().filter(|lang| !lang.is_empty()),
                code: code.value.clone(),
            },

            Node::Math(math) => Block::Math(math.value.clone()),
            Node::Blockquote(quote) => Block::Quote(self.blocks(&quote.children)),
            Node::ThematicBreak(_) => Block::Rule,
            Node::Definition(_) => return Vec::new(),

            Node::List(list) => Block::List {
                ordered: list.ordered,
                start: list.start.unwrap_or(1) as usize,
                items: list
                    .children
                    .iter()
                    .filter_map(|node| match node {
                        Node::ListItem(item) => Some(Item {
                            checked: item.checked,
                            blocks: self.blocks(&item.children),
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
                                    Node::TableCell(cell) => self.inline(&cell.children),
                                    other => self.inline(std::slice::from_ref(other)),
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
                let words = self.inline(std::slice::from_ref(other));

                if words.text.is_empty() {
                    return Vec::new();
                }

                Block::Paragraph(words)
            }
        };

        vec![block]
    }

    /// A paragraph's words, and each image among them as a block of its
    /// own where it stands, between the words before and after it.
    fn paragraph(&self, nodes: &[Node]) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut words = Inline::default();

        for node in nodes {
            let image = match node {
                Node::Image(image) => Some((image.url.clone(), image.alt.clone())),
                Node::ImageReference(reference) => self
                    .definitions
                    .get(&reference.identifier.to_lowercase())
                    .map(|url| (url.clone(), reference.alt.clone())),
                _ => None,
            };

            match image {
                Some((url, alt)) => {
                    let before = std::mem::take(&mut words);

                    if !before.is_blank() {
                        blocks.push(Block::Paragraph(before));
                    }

                    blocks.push(Block::Image { url, alt });
                }
                None => self.walk(node, Marks::default(), &mut words),
            }
        }

        if !words.is_blank() || blocks.is_empty() {
            blocks.push(Block::Paragraph(words));
        }

        blocks
    }

    fn inline(&self, nodes: &[Node]) -> Inline {
        let mut words = Inline::default();

        for node in nodes {
            self.walk(node, Marks::default(), &mut words);
        }

        words
    }

    fn walk(&self, node: &Node, marks: Marks, words: &mut Inline) {
        let within = |words: &mut Inline, children: &[Node], marks: Marks| {
            for child in children {
                self.walk(child, marks, words);
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
            Node::Break(_) => words.push("\n", marks),
            Node::Html(html) => words.push(&html.value, marks),
            Node::Image(image) => words.push(&image.alt, marks),
            Node::ImageReference(image) => words.push(&image.alt, marks),

            Node::InlineMath(math) => {
                let position = math.position.as_ref();
                let after = position.and_then(|position| self.source.get(position.end.offset..));
                let written = position.and_then(|position| {
                    self.source.get(position.start.offset..position.end.offset)
                });

                // Read as written, since one space either side is dropped
                // from what is between the dollars.
                let between =
                    written.map_or(math.value.as_str(), |written| written.trim_matches('$'));

                match (is_formula(between, after), written) {
                    // Dollars that are no formula, as prices are, stay as
                    // they are written.
                    (false, Some(written)) => words.push(written, marks),
                    _ => words.push(
                        &math.value,
                        Marks {
                            math: true,
                            ..marks
                        },
                    ),
                }
            }

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
}

/// Whether what stands between dollars among words is a formula, as
/// Pandoc tells: not when it starts or ends with a space, nor when a figure
/// follows the closing dollar, so that "$5 and $10" stays as written.
fn is_formula(tex: &str, after: Option<&str>) -> bool {
    let spaced = tex.starts_with(char::is_whitespace) || tex.ends_with(char::is_whitespace);
    let figure_after = after
        .and_then(|after| after.chars().next())
        .is_some_and(|next| next.is_ascii_digit());

    !tex.is_empty() && !spaced && !figure_after
}

/// The runs of `0..len` that are marked or selected, in order and apart
/// from each other, with their marks and whether they are selected, as a
/// line of text is drawn. It walks the runs once, since code can have many.
pub(crate) fn segments(
    len: usize,
    spans: &[(Range<usize>, Marks)],
    selected: Option<Range<usize>>,
) -> Vec<(Range<usize>, Marks, bool)> {
    let selected = selected
        .map(|range| range.start.min(len)..range.end.min(len))
        .filter(|range| range.start < range.end);
    let mut segments = Vec::new();

    // A run, cut where the selection starts and ends.
    let mut cut = |range: Range<usize>, marks: Marks| {
        let pieces = match &selected {
            Some(chosen) => [
                (range.start..range.end.min(chosen.start), false),
                (
                    range.start.max(chosen.start)..range.end.min(chosen.end),
                    true,
                ),
                (range.start.max(chosen.end)..range.end, false),
            ],
            None => [(range, false), (0..0, false), (0..0, false)],
        };

        for (piece, chosen) in pieces {
            if piece.start < piece.end && (marks.any() || chosen) {
                segments.push((piece, marks, chosen));
            }
        }
    };

    let mut at = 0;

    for (span, marks) in spans {
        let span = span.start.min(len).max(at)..span.end.min(len);

        if at < span.start {
            cut(at..span.start, Marks::default());
        }

        if span.start < span.end {
            cut(span.clone(), *marks);
            at = span.end;
        }
    }

    if at < len {
        cut(at..len, Marks::default());
    }

    segments
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
        assert_eq!(
            blocks[1],
            Block::Code {
                lang: Some("rust".to_string()),
                code: "fn main() {}".to_string(),
            }
        );
        assert!(matches!(&blocks[2], Block::Quote(inner) if inner.len() == 1));
        assert_eq!(blocks[3], Block::Rule);
    }

    #[test]
    fn reads_formulas_among_words_and_on_their_own() {
        let blocks = parse("The root is $\\sqrt{x}$, and\n\n$$\nE = mc^2\n$$\n\n```math\na^2\n```");
        let words = paragraph(&blocks, 0);
        let math = Marks {
            math: true,
            ..Marks::default()
        };

        assert_eq!(words.text, "The root is \\sqrt{x}, and");
        assert_eq!(words.spans, [(12..20, math)]);
        assert!(words.has_math());
        assert_eq!(blocks[1], Block::Math("E = mc^2".to_string()));
        assert_eq!(blocks[2], Block::Math("a^2".to_string()));
    }

    #[test]
    fn leaves_prices_as_they_are_written() {
        let blocks = parse("It costs $5 and $10, or $ 3 $; but $x$ is one.");
        let words = paragraph(&blocks, 0);

        assert_eq!(words.text, "It costs $5 and $10, or $ 3 $; but x is one.");
        assert_eq!(
            words.spans,
            [(
                35..36,
                Marks {
                    math: true,
                    ..Marks::default()
                }
            )]
        );
    }

    #[test]
    fn takes_images_out_of_their_paragraph() {
        let blocks = parse(
            "Before ![a chart](chart.png) after.\n\n![logo][brand]\n\n[brand]: https://rumbo.dev/logo.svg",
        );

        assert_eq!(paragraph(&blocks, 0).text, "Before ");
        assert_eq!(
            blocks[1],
            Block::Image {
                url: "chart.png".to_string(),
                alt: "a chart".to_string(),
            }
        );
        assert_eq!(paragraph(&blocks, 2).text, " after.");
        assert_eq!(
            blocks[3],
            Block::Image {
                url: "https://rumbo.dev/logo.svg".to_string(),
                alt: "logo".to_string(),
            }
        );
        assert_eq!(blocks.len(), 4);
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

        // Runs side by side, the selection over both and past them.
        let code = Marks {
            code: true,
            ..Marks::default()
        };

        assert_eq!(
            segments(8, &[(0..2, strong), (2..4, code)], Some(1..6)),
            [
                (0..1, strong, false),
                (1..2, strong, true),
                (2..4, code, true),
                (4..6, Marks::default(), true),
            ]
        );
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
