//! Markdown as a model writes it, drawn: paragraphs and headings, lists
//! and tasks, code, quotes, rules, and tables. Each block's words can be
//! selected and copied on their own; a selection does not run from one
//! block into the next.

mod blocks;
mod selectable;

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Refineable as _, RenderOnce, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::child_id;
use crate::list::{List, ListItem, ListMarker};
use crate::root::{h_flex, v_flex};
use crate::scroll::Scrollbar;
use crate::theme::ActiveTheme as _;
use blocks::{Align, Block, Inline};
use selectable::SelectableText;

pub(crate) use selectable::bind_keys;

/// The blocks of the last text drawn, read again only when it changes, as
/// it does while a reply streams in.
struct Parsed {
    source: SharedString,
    blocks: Rc<Vec<Block>>,
}

#[derive(IntoElement)]
pub struct Markdown {
    id: ElementId,
    source: SharedString,
    style: StyleRefinement,
}

impl Markdown {
    pub fn new(id: impl Into<ElementId>, source: impl Into<SharedString>) -> Self {
        Markdown {
            id: id.into(),
            source: source.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Markdown {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Draws blocks, each with an id made of where it stands among them.
struct Drawer<'a> {
    id: &'a ElementId,
    window: &'a mut Window,
    cx: &'a mut App,
}

impl Drawer<'_> {
    fn id(&self, path: &str) -> ElementId {
        child_id(self.id, SharedString::from(path.to_string()))
    }

    fn words(&self, path: &str, words: &Inline) -> SelectableText {
        let links = words
            .links
            .iter()
            .map(|(range, url)| (range.clone(), SharedString::from(url.clone())))
            .collect();

        SelectableText::new(self.id(path), words.text.clone()).marked(words.spans.clone(), links)
    }

    fn blocks(&mut self, path: &str, blocks: &[Block]) -> Vec<AnyElement> {
        blocks
            .iter()
            .enumerate()
            .map(|(index, block)| self.block(&format!("{path}.{index}"), block))
            .collect()
    }

    fn block(&mut self, path: &str, block: &Block) -> AnyElement {
        let theme = self.cx.theme();
        let colors = theme.colors;
        let radius = theme.radius;
        let mono = theme.mono_family.clone();

        match block {
            Block::Paragraph(words) => self.words(path, words).into_any_element(),

            Block::Heading(depth, words) => {
                let (size, line) = match depth {
                    1 => (px(20.), px(28.)),
                    2 => (px(18.), px(26.)),
                    3 => (px(16.), px(24.)),
                    _ => (px(14.), px(20.)),
                };

                div()
                    .text_size(size)
                    .line_height(line)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.words(path, words))
                    .into_any_element()
            }

            Block::Code(code) => {
                let scroll = self
                    .window
                    .use_keyed_state(self.id(&format!("{path}.scroll")), self.cx, |_, _| {
                        ScrollHandle::new()
                    })
                    .read(self.cx)
                    .clone();

                div()
                    .relative()
                    .rounded(radius)
                    .bg(colors.muted)
                    .child(
                        h_flex()
                            .id(self.id(&format!("{path}.code")))
                            .overflow_x_scroll()
                            .track_scroll(&scroll)
                            .p_3()
                            .font_family(mono)
                            .text_size(px(13.))
                            .whitespace_nowrap()
                            .child(
                                div()
                                    .flex_none()
                                    .child(SelectableText::new(self.id(path), code.clone())),
                            ),
                    )
                    .child(Scrollbar::horizontal(
                        self.id(&format!("{path}.scrollbar")),
                        scroll,
                    ))
                    .into_any_element()
            }

            Block::Quote(inner) => v_flex()
                .gap_2()
                .pl_3()
                .border_l_2()
                .border_color(colors.border)
                .text_color(colors.muted_foreground)
                .children(self.blocks(path, inner))
                .into_any_element(),

            Block::List {
                ordered,
                start,
                items,
            } => self.list(path, *ordered, *start, items).into_any_element(),

            Block::Rule => div().h_px().my_1().bg(colors.border).into_any_element(),

            Block::Table { align, rows } => {
                let mut table = v_flex()
                    .w_full()
                    .rounded(radius)
                    .border_1()
                    .border_color(colors.border)
                    .overflow_hidden();

                for (row, cells) in rows.iter().enumerate() {
                    let line = h_flex()
                        .items_start()
                        .when(row > 0, |line| {
                            line.border_t_1().border_color(colors.border)
                        })
                        .when(row == 0, |line| {
                            line.bg(colors.muted).font_weight(FontWeight::SEMIBOLD)
                        })
                        .children(cells.iter().enumerate().map(|(column, words)| {
                            let cell = div()
                                .flex_1()
                                .min_w_0()
                                .px_2()
                                .py_1()
                                .when(column > 0, |cell| {
                                    cell.border_l_1().border_color(colors.border)
                                });

                            let cell = match align.get(column) {
                                Some(Align::Center) => cell.text_center(),
                                Some(Align::End) => cell.text_right(),
                                _ => cell,
                            };

                            cell.child(self.words(&format!("{path}.{row}.{column}"), words))
                        }));

                    table = table.child(line);
                }

                table.into_any_element()
            }
        }
    }

    fn list(&mut self, path: &str, ordered: bool, start: usize, items: &[blocks::Item]) -> List {
        let tasks = items.iter().any(|item| item.checked.is_some());

        let marker = match (tasks, ordered) {
            (true, _) => ListMarker::Task,
            (false, true) => ListMarker::Decimal,
            (false, false) => ListMarker::Bullet,
        };

        let items = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                ListItem::empty()
                    .done(item.checked == Some(true))
                    .children(self.blocks(&format!("{path}.{index}"), &item.blocks))
            })
            .collect::<Vec<_>>();

        List::new(marker).start(start).children(items)
    }
}

impl RenderOnce for Markdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let source = self.source;
        let parsed = window.use_keyed_state(child_id(&self.id, "parsed"), cx, |_, _| Parsed {
            source: source.clone(),
            blocks: Rc::new(blocks::parse(&source)),
        });

        let blocks = parsed.update(cx, |parsed, _| {
            if parsed.source != source {
                parsed.blocks = Rc::new(blocks::parse(&source));
                parsed.source = source;
            }

            parsed.blocks.clone()
        });

        let mut drawer = Drawer {
            id: &self.id,
            window,
            cx,
        };
        let children = drawer.blocks("block", &blocks);

        let mut markdown = v_flex()
            .w_full()
            .gap_3()
            .text_size(px(14.))
            .line_height(px(20.))
            .children(children);

        markdown.style().refine(&self.style);
        markdown
    }
}
