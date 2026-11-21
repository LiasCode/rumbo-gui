//! The session's details beside the conversation: what the window knows of
//! the session shown, from the host's answers and the runs' events, drawn
//! to be read at a glance, with the skills the model is told of; and, when
//! asked for, all the host says of the session. What the host says in words
//! is laid out by `outline`, and its words kept as they are.

use gpui::{
    AnyElement, App, ClipboardItem, Context, Div, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _,
};
use ui::{
    ActiveTheme as _, Button, IconName, Meter, Scrollbar, Sizable as _, Spinner, Tone, h_flex,
    rems_from_px, v_flex,
};

use super::panel::Read;
use super::{Workspace, WorkspaceEvent};
use crate::commands::capitalized;
use crate::outline::{self, Block};
use crate::usage::{grouped, short};

/// How full a context is before its meter says it nearly is, and before it
/// says it all but is.
const NEARLY_FULL: f32 = 0.75;
const ALL_BUT_FULL: f32 = 0.9;

impl Workspace {
    pub(super) fn render_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let details = v_flex()
            .id("details")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.panels.details_scroll)
            .p_4()
            .gap_6()
            .child(self.render_session(cx))
            .child(self.render_usage(cx))
            .child(section(
                "Skills",
                said(&self.panels.skills, "skills", cx),
                cx,
            ))
            .child(self.render_everything(cx));

        div()
            .relative()
            .size_full()
            .child(details)
            .child(Scrollbar::vertical(
                "details-scrollbar",
                self.panels.details_scroll.clone(),
            ))
            .into_any_element()
    }

    /// Which session it is and how the host says it stands, the folder it
    /// works in, and the model it uses.
    fn render_session(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().colors.muted_foreground;
        let listed = self
            .listed
            .iter()
            .find(|listed| !self.current.is_empty() && listed.value == self.current);

        let session = match self.current.is_empty() {
            true => div()
                .text_sm()
                .text_color(muted)
                .child("New; its first task starts it.")
                .into_any_element(),

            false => {
                let copied = self.current.clone();

                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                div()
                                    .px_1p5()
                                    .rounded(cx.theme().radius)
                                    .bg(cx.theme().colors.muted)
                                    .font_family(cx.theme().mono_family.clone())
                                    .text_xs()
                                    .child(self.current.clone()),
                            )
                            .child(
                                Button::new("details-copy-id")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Copy)
                                    .tooltip("Copy the session's id")
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            copied.clone(),
                                        ))
                                    }),
                            ),
                    )
                    .when_some(
                        listed.filter(|listed| !listed.description.is_empty()),
                        |column, listed| {
                            column.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(capitalized(&listed.description)),
                            )
                        },
                    )
                    .into_any_element()
            }
        };

        let folder = h_flex()
            .items_start()
            .gap_2()
            .justify_between()
            .child(div().min_w_0().text_sm().child(self.dir.clone()))
            .child(
                Button::new("details-editor")
                    .ghost()
                    .xsmall()
                    .icon(IconName::ExternalLink)
                    .label("Open in Editor")
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.emit(WorkspaceEvent::OpenInEditor {
                            path: this.dir.clone(),
                        })
                    })),
            );

        let model = h_flex()
            .items_start()
            .gap_2()
            .justify_between()
            .child(div().min_w_0().text_sm().child(self.model_name()))
            .child(
                Button::new("details-model")
                    .ghost()
                    .xsmall()
                    .label("Change…")
                    .selected(self.picking)
                    .tooltip("The model and the options of this session")
                    .on_click(cx.listener(|this, _, _, cx| this.open_picker(cx))),
            );

        // The panel's title says it is the session's.
        v_flex()
            .gap_6()
            .child(session)
            .child(section("Folder", folder.into_any_element(), cx))
            .child(section("Model", model.into_any_element(), cx))
    }

    /// How full the model's context is, and the tokens the session spent,
    /// as its runs' events counted them.
    fn render_usage(&self, cx: &mut Context<Self>) -> Div {
        let colors = cx.theme().colors;
        let muted = colors.muted_foreground;
        let spent = self.views.get(&self.current).map(|view| &view.spent);
        let context = spent.and_then(|spent| spent.context_of_window());
        let totals = spent.and_then(|spent| spent.totals());

        if context.is_none() && totals.is_none() {
            return v_flex().child(section(
                "Usage",
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Nothing has been asked in this session yet.")
                    .into_any_element(),
                cx,
            ));
        }

        let context = context.map(|(used, window)| {
            let share = used as f32 / window as f32;
            let tone = match share {
                share if share >= ALL_BUT_FULL => Tone::Danger,
                share if share >= NEARLY_FULL => Tone::Warning,
                _ => Tone::Info,
            };

            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .justify_between()
                        .items_end()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(format!("{}%", used * 100 / window)),
                        )
                        .child(div().text_xs().text_color(muted).child(format!(
                            "{} of {} tokens",
                            grouped(used),
                            grouped(window)
                        ))),
                )
                .child(Meter::new(share).tone(tone))
        });

        let tokens = totals.map(|totals| {
            let cached = spent.and_then(|spent| spent.cached_share());

            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .child(tile("In", totals.input, cx))
                        .child(tile("Cached", totals.cached, cx))
                        .child(tile("Out", totals.output, cx)),
                )
                .when_some(cached, |column, share| {
                    column.child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("{share}% of the input was read from the cache")),
                    )
                })
        });

        v_flex()
            .gap_6()
            .when_some(context, |column, context| {
                column.child(section("Context", context.into_any_element(), cx))
            })
            .when_some(tokens, |column, tokens| {
                column.child(section("Tokens", tokens.into_any_element(), cx))
            })
    }

    /// All the host says of the session, which repeats much of what is above
    /// it, so it shows only when asked for.
    fn render_everything(&self, cx: &mut Context<Self>) -> Div {
        let shown = self.panels.everything;

        v_flex()
            .gap_3()
            .child(
                Button::new("details-everything")
                    .ghost()
                    .small()
                    .justify_start()
                    .icon(match shown {
                        true => IconName::ChevronDown,
                        false => IconName::ChevronRight,
                    })
                    .label("Everything rumbo reports")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.panels.everything = !this.panels.everything;
                        cx.notify();
                    })),
            )
            .when(shown, |column| {
                column.child(said(&self.panels.status, "status", cx))
            })
    }
}

/// A part of the details: its name, and what it holds under it.
fn section(title: &'static str, body: AnyElement, cx: &App) -> Div {
    v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(cx.theme().colors.muted_foreground)
                .child(title),
        )
        .child(body)
}

/// A count of tokens in a tile: at a glance, and to the last one.
fn tile(name: &'static str, count: i64, cx: &App) -> Div {
    let colors = cx.theme().colors;

    v_flex()
        .flex_1()
        .min_w_0()
        .gap_0p5()
        .px_3()
        .py_2()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(colors.border)
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(short(count)),
        )
        .child(
            div()
                .truncate()
                .text_xs()
                .text_color(colors.muted_foreground)
                .child(format!("{name} · {}", grouped(count))),
        )
}

/// What the host said, laid out by its shape, or that it is being asked, or
/// why it could not say.
fn said(read: &Option<Read<String>>, function: &str, cx: &App) -> AnyElement {
    let colors = cx.theme().colors;

    match read {
        None => div()
            .text_sm()
            .text_color(colors.muted_foreground)
            .child(format!("This host does not offer {function}."))
            .into_any_element(),

        Some(Read::Reading) => Spinner::new().small().into_any_element(),

        Some(Read::Failed(problem)) => div()
            .text_sm()
            .text_color(colors.danger)
            .child(problem.clone())
            .into_any_element(),

        Some(Read::Here(text)) => laid_out(&outline::read(text), cx),
    }
}

/// The parts of what the host said: headings over what they head; what it
/// names in a card of rows, each name beside what it is; what it sets in
/// under a line in a card of its own; and the rest as words.
fn laid_out(blocks: &[Block], cx: &App) -> AnyElement {
    let colors = cx.theme().colors;
    let muted = colors.muted_foreground;
    let mut column = v_flex().gap_3();
    let mut rows: Vec<AnyElement> = Vec::new();
    let mut items: Vec<AnyElement> = Vec::new();

    // Rows and items next to each other go in one card.
    let close = |column: Div, rows: &mut Vec<AnyElement>| match rows.is_empty() {
        true => column,
        false => column.child(card(std::mem::take(rows), cx)),
    };

    for block in blocks {
        if !matches!(block, Block::Field { .. }) {
            column = close(column, &mut rows);
        }

        if !matches!(block, Block::Item { .. }) {
            column = close(column, &mut items);
        }

        match block {
            Block::Heading(heading) => {
                column = column.child(
                    div()
                        .pt_1()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(capitalized(heading)),
                );
            }

            Block::Field { label, value } => rows.push(
                h_flex()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .w(rems_from_px(132.))
                            .flex_none()
                            .text_xs()
                            .text_color(muted)
                            .child(capitalized(label)),
                    )
                    .child(words(value, colors.foreground, cx).flex_1())
                    .into_any_element(),
            ),

            Block::Item {
                title,
                text,
                details,
            } => items.push(
                v_flex()
                    .gap_0p5()
                    .child(
                        words(title, colors.foreground, cx).font_weight(gpui::FontWeight::MEDIUM),
                    )
                    .when(!text.is_empty(), |item| item.child(words(text, muted, cx)))
                    .children(details.iter().map(|detail| words(detail, muted, cx)))
                    .into_any_element(),
            ),

            Block::Paragraph(paragraph) => {
                column = column.child(words(&capitalized(paragraph), muted, cx));
            }
        }
    }

    column = close(column, &mut rows);
    column = close(column, &mut items);
    column.into_any_element()
}

/// Rows one under another in a card, a line between each.
fn card(rows: Vec<AnyElement>, cx: &App) -> Div {
    let colors = cx.theme().colors;

    v_flex()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(colors.border)
        .children(rows.into_iter().enumerate().map(|(index, row)| {
            div()
                .px_3()
                .py_2()
                .when(index > 0, |row| {
                    row.border_t_1().border_color(colors.border)
                })
                .child(row)
        }))
}

/// Words of the host in a small size, which wrap; a path or another word
/// that holds no space, such as JSON, in the typeface of code.
fn words(text: &str, color: Hsla, cx: &App) -> Div {
    let code = !text.contains(' ') && (text.contains('/') || text.starts_with(['{', '[']));

    div()
        .min_w_0()
        .text_xs()
        .text_color(color)
        .when(code, |words| {
            words.font_family(cx.theme().mono_family.clone())
        })
        .child(SharedString::from(text.to_string()))
}
