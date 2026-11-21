//! Markdown as a model writes it, drawn: paragraphs and headings, lists
//! and tasks, code colored by its syntax, formulas in TeX, Mermaid
//! diagrams, images, quotes, rules, and tables. Each block's words can be
//! selected and copied on their own; a selection does not run from one
//! block into the next, nor through a paragraph that holds a formula,
//! whose words are laid out one by one around it.
//!
//! An image of the web shows only once the person asks for it, since an
//! image a model names could carry what it read to whoever serves it; a
//! file on this machine, or one in the text itself, shows at once.

mod blocks;
mod diagram;
mod highlight;
mod image;
mod math;
mod selectable;

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, ElementId, Entity, FontWeight, ImageAssetLoader, ImageCacheError,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Refineable as _, RenderImage,
    RenderOnce, Resource, Rgba, SMOOTH_SVG_SCALE_FACTOR, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, StyledText, Window, canvas, div, img,
    prelude::FluentBuilder as _, px,
};

use crate::button::Button;
use crate::child_id;
use crate::icon::{Icon, IconName};
use crate::list::{List, ListItem, ListMarker};
use crate::root::{h_flex, v_flex};
use crate::scroll::Scrollbar;
use crate::size::Sizable as _;
use crate::theme::{ActiveTheme as _, ThemeMode};
use blocks::{Align, Block, Inline, Marks};
use image::Source;
use selectable::SelectableText;

pub(crate) use selectable::bind_keys;

/// The size of a paragraph's words, and of a formula on its own line.
const TEXT: Pixels = px(14.);
const DISPLAY: Pixels = px(16.);

/// What is drawn once for a text and kept while it shows it, by what it is
/// drawn from: code's colors, and the pictures of formulas, diagrams, and
/// images the text holds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    Colors {
        lang: String,
        code: String,
        dark: bool,
    },
    Formula {
        tex: String,
        size: u32,
        color: u32,
        display: bool,
    },
    Diagram {
        source: String,
        dark: bool,
    },
    Data {
        url: String,
    },
}

/// An image ready to draw, and its size, with its baseline for a formula.
#[derive(Clone)]
struct Picture {
    image: Arc<RenderImage>,
    width: Pixels,
    height: Pixels,
    baseline: Option<Pixels>,
}

#[derive(Clone)]
enum Drawn {
    Colors(Option<Rc<highlight::Colored>>),
    /// A picture, or why it could not be drawn.
    Picture(Result<Picture, SharedString>),
}

/// The blocks of the last text drawn, read again only when it changes, as
/// it does while a reply streams in, and what was drawn of them.
struct Parsed {
    source: SharedString,
    blocks: Rc<Vec<Block>>,
    /// How wide the text was laid out last, for pictures to fit in.
    width: Option<Pixels>,
    /// The images of the web the person asked to see.
    shown: HashSet<String>,
    drawn: HashMap<Key, Drawn>,
    /// What this frame drew; what it did not is let go.
    used: HashSet<Key>,
}

#[derive(IntoElement)]
pub struct Markdown {
    id: ElementId,
    source: SharedString,
    base: Option<PathBuf>,
    style: StyleRefinement,
}

impl Markdown {
    pub fn new(id: impl Into<ElementId>, source: impl Into<SharedString>) -> Self {
        Markdown {
            id: id.into(),
            source: source.into(),
            base: None,
            style: StyleRefinement::default(),
        }
    }

    /// The folder an image's relative path starts from, such as the
    /// project's the text is about. Without it such an image is not found.
    pub fn base(mut self, base: impl Into<PathBuf>) -> Self {
        self.base = Some(base.into());
        self
    }
}

impl Styled for Markdown {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A color as 0xRRGGBB, as the drawing libraries take one.
fn rgb_of(color: gpui::Hsla) -> u32 {
    let color = Rgba::from(color);
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u32;

    channel(color.r) << 16 | channel(color.g) << 8 | channel(color.b)
}

/// A picture of `image` at its size, which GPUI keeps doubled for an SVG,
/// so that its edges are smooth.
fn sized(image: Arc<RenderImage>, svg: bool) -> Picture {
    let scale = if svg { SMOOTH_SVG_SCALE_FACTOR } else { 1. };
    let size = image.size(0);

    Picture {
        width: px(size.width.0 as f32 / scale),
        height: px(size.height.0 as f32 / scale),
        baseline: None,
        image,
    }
}

/// An SVG drawn as GPUI draws one.
fn rasterized(svg: &str, cx: &App) -> Result<Picture, SharedString> {
    cx.svg_renderer()
        .render_single_frame(svg.as_bytes(), 1.0)
        .map(|image| sized(image, true))
        .map_err(|error| SharedString::from(error.to_string()))
}

/// A picture drawn at its size, or narrower, keeping its shape, to fit in
/// `fit`.
fn picture(picture: &Picture, fit: Option<Pixels>) -> gpui::Img {
    let (width, height) = match fit {
        Some(fit) if picture.width > fit && fit > px(0.) => {
            (fit, picture.height * (fit / picture.width))
        }
        _ => (picture.width, picture.height),
    };

    img(picture.image.clone()).flex_none().w(width).h(height)
}

/// Why an image could not be read, as a person reads it.
fn reason(error: &ImageCacheError) -> SharedString {
    error.to_string().into()
}

/// Draws blocks, each with an id made of where it stands among them.
struct Drawer<'a> {
    id: &'a ElementId,
    parsed: Entity<Parsed>,
    base: Option<&'a PathBuf>,
    window: &'a mut Window,
    cx: &'a mut App,
}

impl Drawer<'_> {
    fn id(&self, path: &str) -> ElementId {
        child_id(self.id, SharedString::from(path.to_string()))
    }

    fn dark(&self) -> bool {
        self.cx.theme().mode == ThemeMode::Dark
    }

    /// How wide the text was laid out last.
    fn fit(&self) -> Option<Pixels> {
        self.parsed.read(self.cx).width
    }

    /// What was drawn from `key`, drawn now with `make` the first time.
    fn drawn(&mut self, key: Key, make: impl FnOnce(&mut App) -> Drawn) -> Drawn {
        let known = self.parsed.update(self.cx, |parsed, _| {
            parsed.used.insert(key.clone());
            parsed.drawn.get(&key).cloned()
        });

        if let Some(drawn) = known {
            return drawn;
        }

        let drawn = make(self.cx);

        self.parsed.update(self.cx, |parsed, _| {
            parsed.drawn.insert(key, drawn.clone());
        });

        drawn
    }

    /// A picture drawn from `key` with `make`, or why it could not be.
    fn pictured(
        &mut self,
        key: Key,
        make: impl FnOnce(&mut App) -> Result<Picture, SharedString>,
    ) -> Result<Picture, SharedString> {
        match self.drawn(key, |cx| Drawn::Picture(make(cx))) {
            Drawn::Picture(made) => made,
            Drawn::Colors(_) => Err("it was drawn as something else".into()),
        }
    }

    fn formula(&mut self, tex: &str, size: Pixels, display: bool) -> Result<Picture, SharedString> {
        let color = rgb_of(self.cx.theme().colors.foreground);
        let size = f32::from(size).round() as u32;
        let key = Key::Formula {
            tex: tex.to_string(),
            size,
            color,
            display,
        };
        let tex = tex.to_string();

        self.pictured(key, move |cx| {
            let formula = math::formula(&tex, size, color, display)?;
            let picture = rasterized(&formula.svg, cx)?;

            Ok(Picture {
                width: px(formula.width),
                height: px(formula.height),
                baseline: Some(px(formula.baseline)),
                ..picture
            })
        })
    }

    /// A block's words: as words that can be selected, or laid out one by
    /// one around the formulas among them.
    fn words(&mut self, path: &str, words: &Inline, size: Pixels) -> AnyElement {
        if words.has_math() {
            return self.flow(path, words, size);
        }

        let links = words
            .links
            .iter()
            .map(|(range, url)| (range.clone(), SharedString::from(url.clone())))
            .collect();

        SelectableText::new(self.id(path), words.text.clone())
            .marked(words.spans.clone(), links)
            .into_any_element()
    }

    /// Words and the formulas among them, wrapped as words wrap. Each
    /// formula is set off so that its baseline and the words' stand at the
    /// same height when both are centered in a line.
    fn flow(&mut self, path: &str, words: &Inline, size: Pixels) -> AnyElement {
        let colors = self.cx.theme().colors;
        let mono = self.cx.theme().mono_family.clone();

        // How far below the middle of a line of words their baseline is.
        let font = self.window.text_style().font();
        let font_id = self.window.text_system().resolve_font(&font);
        let ascent = self.window.text_system().ascent(font_id, size);
        let descent = self.window.text_system().descent(font_id, size).abs();
        let below_middle = (ascent - descent) / 2.;

        // Every run of the words, marked or not, in order.
        let mut runs: Vec<(Range<usize>, Marks)> = Vec::new();
        let mut at = 0;

        for (range, marks) in &words.spans {
            if at < range.start {
                runs.push((at..range.start, Marks::default()));
            }

            runs.push((range.clone(), *marks));
            at = range.end;
        }

        if at < words.text.len() {
            runs.push((at..words.text.len(), Marks::default()));
        }

        let mut items: Vec<AnyElement> = Vec::new();

        for (range, marks) in runs {
            let text = &words.text[range.clone()];

            if marks.math {
                let item = match self.formula(text, size, false) {
                    Ok(formula) => {
                        let baseline = formula.baseline.unwrap_or(formula.height);
                        let shift = formula.height - baseline * 2. + below_middle * 2.;

                        div()
                            .debug_selector(|| "formula".into())
                            .flex_none()
                            .when(shift > px(0.), |set| set.pt(shift))
                            .when(shift < px(0.), |set| set.pb(-shift))
                            .child(picture(&formula, None))
                    }

                    // What cannot be typeset shows as it was written.
                    Err(_) => div()
                        .flex_none()
                        .font_family(mono.clone())
                        .bg(colors.muted)
                        .child(format!("${text}$")),
                };

                items.push(item.into_any_element());
                continue;
            }

            let link = words
                .links
                .iter()
                .find(|(link, _)| link.start <= range.start && range.end <= link.end)
                .map(|(_, url)| url.clone());

            for (line_index, line) in text.split('\n').enumerate() {
                // A break starts a line of its own.
                if line_index > 0 {
                    items.push(div().w_full().into_any_element());
                }

                for word in line.split_inclusive(' ') {
                    let style = selectable::style(marks, false, &colors);
                    let mut shown =
                        StyledText::new(word.to_string()).with_highlights([(0..word.len(), style)]);

                    if marks.code {
                        shown = shown.with_font_family_overrides([(0..word.len(), mono.clone())]);
                    }

                    let item = div().flex_none().whitespace_nowrap().child(shown);
                    let item = match &link {
                        Some(url) => {
                            let url = url.clone();

                            item.id(self.id(&format!("{path}.word-{}", items.len())))
                                .cursor_pointer()
                                .on_click(move |_, _, cx| cx.open_url(&url))
                                .into_any_element()
                        }
                        None => item.into_any_element(),
                    };

                    items.push(item);
                }
            }
        }

        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .children(items)
            .into_any_element()
    }

    fn blocks(&mut self, path: &str, blocks: &[Block]) -> Vec<AnyElement> {
        blocks
            .iter()
            .enumerate()
            .map(|(index, block)| self.block(&format!("{path}.{index}"), block))
            .collect()
    }

    /// A box that scrolls sideways when what it holds is wider than it,
    /// under its scrollbar, as code and formulas do.
    fn scrolled(&mut self, path: &str, content: impl IntoElement) -> gpui::Div {
        let scroll = self
            .window
            .use_keyed_state(self.id(&format!("{path}.scroll")), self.cx, |_, _| {
                ScrollHandle::new()
            })
            .read(self.cx)
            .clone();

        div()
            .relative()
            .child(
                h_flex()
                    .id(self.id(&format!("{path}.scrolled")))
                    .overflow_x_scroll()
                    .track_scroll(&scroll)
                    .child(div().flex_none().child(content)),
            )
            .child(Scrollbar::horizontal(
                self.id(&format!("{path}.scrollbar")),
                scroll,
            ))
    }

    fn code(&mut self, path: &str, lang: Option<&str>, code: &str) -> AnyElement {
        let theme = self.cx.theme();
        let colors = theme.colors;
        let radius = theme.radius;
        let mono = theme.mono_family.clone();
        let dark = self.dark();

        let colored = lang.and_then(|lang| {
            let key = Key::Colors {
                lang: lang.to_string(),
                code: code.to_string(),
                dark,
            };
            let (lang, source) = (lang.to_string(), code.to_string());

            match self.drawn(key, move |_| {
                Drawn::Colors(highlight::highlight(&source, &lang, dark).map(Rc::new))
            }) {
                Drawn::Colors(colored) => colored,
                Drawn::Picture(_) => None,
            }
        });

        let spans = colored
            .map(|runs| {
                runs.iter()
                    .map(|(range, color)| {
                        let marks = Marks {
                            color: Some(*color),
                            ..Marks::default()
                        };

                        (range.clone(), marks)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let words = SelectableText::new(self.id(path), code.to_string()).marked(spans, Vec::new());

        self.scrolled(
            path,
            div()
                .p_3()
                .font_family(mono)
                .text_size(px(13.))
                .whitespace_nowrap()
                .child(words),
        )
        .rounded(radius)
        .bg(colors.muted)
        .into_any_element()
    }

    /// A word that something could not be drawn, and why.
    fn problem(&self, said: &str, why: &str) -> AnyElement {
        let colors = self.cx.theme().colors;

        h_flex()
            .gap_1p5()
            .text_xs()
            .text_color(colors.muted_foreground)
            .child(Icon::new(IconName::CircleAlert).xsmall())
            .child(format!("{said}: {why}"))
            .into_any_element()
    }

    fn diagram(&mut self, path: &str, source: &str) -> AnyElement {
        let dark = self.dark();
        let key = Key::Diagram {
            source: source.to_string(),
            dark,
        };
        let written = source.to_string();

        let drawn = self.pictured(key, move |cx| {
            let svg = diagram::mermaid(&written, dark)?;

            rasterized(&svg, cx)
        });

        match drawn {
            Ok(drawn) => h_flex()
                .debug_selector(|| "diagram".into())
                .w_full()
                .justify_center()
                .child(picture(&drawn, self.fit()))
                .into_any_element(),

            // While a diagram streams in, and when it is wrong, its source
            // shows.
            Err(problem) => v_flex()
                .gap_1()
                .child(self.code(path, None, source))
                .child(self.problem("The diagram could not be drawn", &problem))
                .into_any_element(),
        }
    }

    fn display_math(&mut self, path: &str, tex: &str) -> AnyElement {
        match self.formula(tex, DISPLAY, true) {
            Ok(formula) => {
                let wide = self.fit().is_some_and(|fit| formula.width > fit);
                let drawn = picture(&formula, None);

                match wide {
                    true => self.scrolled(path, drawn).into_any_element(),
                    false => h_flex()
                        .w_full()
                        .justify_center()
                        .py_1()
                        .child(drawn)
                        .into_any_element(),
                }
            }

            Err(problem) => v_flex()
                .gap_1()
                .child(self.code(path, None, tex))
                .child(self.problem("The formula could not be drawn", &problem))
                .into_any_element(),
        }
    }

    /// An image, once it is read, as wide as it is or as the text.
    fn image(&mut self, path: &str, url: &str, alt: &str) -> AnyElement {
        let svg = url.to_lowercase().ends_with(".svg");

        let read: Option<Result<Picture, SharedString>> =
            match image::source(url, self.base.map(PathBuf::as_path)) {
                None => Some(Err("it is no image this window can show".into())),

                Some(Source::Data { format, bytes }) => {
                    let key = Key::Data {
                        url: url.to_string(),
                    };

                    Some(self.pictured(key, move |cx| {
                        gpui::Image::from_bytes(format, bytes)
                            .to_image_data(cx.svg_renderer())
                            .map(|image| sized(image, format == gpui::ImageFormat::Svg))
                            .map_err(|error| SharedString::from(error.to_string()))
                    }))
                }

                Some(Source::File(file)) => self
                    .window
                    .use_asset::<ImageAssetLoader>(&Resource::Path(file.into()), self.cx)
                    .map(|read| read.map(|image| sized(image, svg)).map_err(|e| reason(&e))),

                Some(Source::Web(web)) => {
                    if !self.parsed.read(self.cx).shown.contains(&web) {
                        return self.ask_for(path, &web, alt);
                    }

                    self.window
                        .use_asset::<ImageAssetLoader>(&Resource::Uri(web.into()), self.cx)
                        .map(|read| read.map(|image| sized(image, svg)).map_err(|e| reason(&e)))
                }
            };

        let theme = self.cx.theme();
        let colors = theme.colors;
        let radius = theme.radius;

        match read {
            Some(Ok(read)) => v_flex()
                .gap_1()
                .child(picture(&read, self.fit()).rounded(radius))
                .when(!alt.is_empty(), |image| {
                    image.child(
                        div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(alt.to_string()),
                    )
                })
                .into_any_element(),

            None => h_flex()
                .gap_2()
                .px_3()
                .py_2()
                .rounded(radius)
                .bg(colors.muted)
                .text_xs()
                .text_color(colors.muted_foreground)
                .child(Icon::new(IconName::Image).small())
                .child(match alt.is_empty() {
                    true => "Reading the image…".to_string(),
                    false => format!("Reading {alt}…"),
                })
                .into_any_element(),

            Some(Err(problem)) => {
                let said = match alt.is_empty() {
                    true => "The image could not be shown".to_string(),
                    false => format!("{alt} could not be shown"),
                };

                self.problem(&said, &problem)
            }
        }
    }

    /// An image of the web not asked for yet: what it is, where it is, and
    /// a button that shows it.
    fn ask_for(&self, path: &str, url: &str, alt: &str) -> AnyElement {
        let theme = self.cx.theme();
        let colors = theme.colors;
        let parsed = self.parsed.clone();
        let asked = url.to_string();

        h_flex()
            .gap_3()
            .px_3()
            .py_2()
            .rounded(theme.radius)
            .border_1()
            .border_color(colors.border)
            .text_sm()
            .child(Icon::new(IconName::Image).color(colors.muted_foreground))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().child(match alt.is_empty() {
                        true => "An image".to_string(),
                        false => alt.to_string(),
                    }))
                    .child(
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(format!("From {}", image::host(url))),
                    ),
            )
            .child(
                div().debug_selector(|| "show-image".into()).child(
                    Button::new(self.id(&format!("{path}.show")))
                        .outline()
                        .small()
                        .label("Show")
                        .tooltip("Fetch it from the web, which tells its server it was read")
                        .on_click(move |_, _, cx| {
                            parsed.update(cx, |parsed, cx| {
                                parsed.shown.insert(asked.clone());
                                cx.notify();
                            })
                        }),
                ),
            )
            .into_any_element()
    }

    fn block(&mut self, path: &str, block: &Block) -> AnyElement {
        let theme = self.cx.theme();
        let colors = theme.colors;
        let radius = theme.radius;

        match block {
            Block::Paragraph(words) => self.words(path, words, TEXT),

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
                    .child(self.words(path, words, size))
                    .into_any_element()
            }

            Block::Code { lang, code } => match lang.as_deref() {
                Some("mermaid") => self.diagram(path, code),
                lang => self.code(path, lang, code),
            },

            Block::Math(tex) => self.display_math(path, tex),

            Block::Image { url, alt } => self.image(path, url, alt),

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
                    let mut line = h_flex()
                        .items_start()
                        .when(row > 0, |line| {
                            line.border_t_1().border_color(colors.border)
                        })
                        .when(row == 0, |line| {
                            line.bg(colors.muted).font_weight(FontWeight::SEMIBOLD)
                        });

                    for (column, words) in cells.iter().enumerate() {
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

                        let words = self.words(&format!("{path}.{row}.{column}"), words, TEXT);

                        line = line.child(cell.child(words));
                    }

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
            width: None,
            shown: HashSet::new(),
            drawn: HashMap::new(),
            used: HashSet::new(),
        });

        let blocks = parsed.update(cx, |parsed, _| {
            if parsed.source != source {
                parsed.blocks = Rc::new(blocks::parse(&source));
                parsed.source = source;
            }

            parsed.used.clear();
            parsed.blocks.clone()
        });

        let mut drawer = Drawer {
            id: &self.id,
            parsed: parsed.clone(),
            base: self.base.as_ref(),
            window,
            cx,
        };
        let children = drawer.blocks("block", &blocks);

        // What this frame did not draw is let go.
        parsed.update(cx, |parsed, _| {
            let used = std::mem::take(&mut parsed.used);

            parsed.drawn.retain(|key, _| used.contains(key));
        });

        // Its width, for the pictures in it to fit in on the next frame.
        let measured = parsed.clone();
        let measure = canvas(
            move |bounds, _, cx| {
                measured.update(cx, |parsed, cx| {
                    if parsed.width != Some(bounds.size.width) {
                        parsed.width = Some(bounds.size.width);
                        cx.notify();
                    }
                })
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();

        let mut markdown = v_flex()
            .relative()
            .w_full()
            .gap_3()
            .text_size(TEXT)
            .line_height(px(20.))
            .children(children)
            .child(measure);

        markdown.style().refine(&self.style);
        markdown
    }
}

/// A text drawn, in a window GPUI keeps in the test's own process.
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures::FutureExt as _;
    use futures::future::BoxFuture;
    use gpui::http_client::http::HeaderValue;
    use gpui::http_client::{self, AsyncBody, Request, Response, Url};
    use gpui::{
        Context, IntoElement, Modifiers, ParentElement as _, Render, Styled as _, TestAppContext,
        Window, div, px,
    };

    use super::Markdown;

    struct Text(&'static str);

    impl Render for Text {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().w(px(500.)).child(Markdown::new("text", self.0))
        }
    }

    /// What counts the images it is asked for, and has none.
    struct Counting(Arc<AtomicUsize>);

    impl http_client::HttpClient for Counting {
        fn user_agent(&self) -> Option<&HeaderValue> {
            None
        }

        fn proxy(&self) -> Option<&Url> {
            None
        }

        fn send(
            &self,
            _: Request<AsyncBody>,
        ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
            self.0.fetch_add(1, Ordering::SeqCst);

            async { Err(anyhow::anyhow!("no network in a test")) }.boxed()
        }
    }

    #[gpui::test]
    fn draws_formulas_and_diagrams(cx: &mut TestAppContext) {
        cx.update(crate::theme::init);

        let (_, cx) = cx.add_window_view(|_, _| {
            Text("A root, $\\sqrt{x}$, among words.\n\n```mermaid\nflowchart LR\n  A --> B\n```\n")
        });

        cx.run_until_parked();

        let formula = cx.debug_bounds("formula").expect("the formula is drawn");
        let diagram = cx.debug_bounds("diagram").expect("the diagram is drawn");

        assert!(formula.size.width > px(0.) && formula.size.height > px(0.));
        assert!(diagram.size.height > px(0.));
    }

    #[gpui::test]
    fn fetches_an_image_of_the_web_only_once_asked(cx: &mut TestAppContext) {
        let asked = Arc::new(AtomicUsize::new(0));
        let client = Counting(asked.clone());

        cx.update(|cx| {
            crate::theme::init(cx);
            cx.set_http_client(Arc::new(client));
        });

        let (_, cx) = cx.add_window_view(|_, _| Text("![A chart](https://example.com/chart.png)"));

        cx.run_until_parked();

        assert_eq!(asked.load(Ordering::SeqCst), 0);

        let show = cx.debug_bounds("show-image").expect("the image is offered");

        cx.simulate_click(show.center(), Modifiers::none());
        cx.run_until_parked();

        assert_eq!(asked.load(Ordering::SeqCst), 1);
    }
}
