use gpui::{
    AnyElement, AppContext as _, Context, Entity, ParentElement as _, SharedString, Styled as _,
    Window, div, px,
};
use ui::{Label, Select, SelectEvent, SelectItem, SelectState, Sizable as _, v_flex};

use super::{Demos, note, row, section, told};
use crate::Overview;

pub struct Demo {
    pub provider: Entity<SelectState>,
    model: Entity<SelectState>,
    searchable: Entity<SelectState>,
    disabled: Entity<SelectState>,
    small: Entity<SelectState>,
    told: Vec<SharedString>,
}

pub fn providers() -> Vec<SelectItem> {
    ["anthropic", "openai", "chatgpt", "toy"]
        .into_iter()
        .map(SelectItem::new)
        .collect()
}

fn models() -> Vec<SelectItem> {
    [
        ("claude-opus-5-5", "Claude Opus 5.5", "The most careful"),
        ("claude-sonnet-5", "Claude Sonnet 5", "Fast and thorough"),
        ("claude-haiku-5", "Claude Haiku 5", "The quickest"),
    ]
    .into_iter()
    .map(|(value, label, description)| SelectItem::new(value).label(label).description(description))
    .collect()
}

/// Enough values to scroll, and to search.
fn many() -> Vec<SelectItem> {
    let families = [
        "opus", "sonnet", "haiku", "gpt", "o", "gemini", "llama", "mistral",
    ];

    families
        .iter()
        .flat_map(|family| {
            (1..=5).map(move |version| {
                SelectItem::new(format!("{family}-{version}"))
                    .description(format!("version {version} of {family}"))
            })
        })
        .collect()
}

/// A select whose choices the page tells, by `name`.
fn select(
    name: &'static str,
    build: impl FnOnce(&mut Window, &mut Context<SelectState>) -> SelectState,
    window: &mut Window,
    cx: &mut Context<Overview>,
) -> Entity<SelectState> {
    let state = cx.new(|cx| build(window, cx));

    cx.subscribe(&state, move |this, _, event: &SelectEvent, cx| {
        let SelectEvent::Change(value) = event;

        note(
            &mut this.demos.select.told,
            format!("{name}: chose {value}"),
        );
        cx.notify();
    })
    .detach();

    state
}

impl Demo {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        Demo {
            provider: select(
                "provider",
                |window, cx| SelectState::new(providers(), Some(0), window, cx),
                window,
                cx,
            ),
            model: select(
                "model",
                |window, cx| {
                    SelectState::new(models(), None, window, cx).placeholder("Choose a model")
                },
                window,
                cx,
            ),
            searchable: select(
                "searchable",
                |window, cx| {
                    SelectState::new(many(), Some(7), window, cx)
                        .searchable(true)
                        .placeholder("Choose a model")
                },
                window,
                cx,
            ),
            disabled: select(
                "disabled",
                |window, cx| SelectState::new(providers(), Some(1), window, cx).disabled(true),
                window,
                cx,
            ),
            small: select(
                "small",
                |window, cx| SelectState::new(providers(), Some(3), window, cx),
                window,
                cx,
            ),
            told: Vec::new(),
        }
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.select;
    let width = px(280.);

    vec![
        section(
            "A list",
            "Up, Down, Enter, or Space open it; Up and Down move in it, Enter chooses, Escape closes it.",
            v_flex()
                .w(width)
                .gap_2()
                .child(Label::new("Provider"))
                .child(Select::new(&demo.provider)),
            cx,
        ),
        section(
            "With what tells each apart",
            "A placeholder until something is chosen.",
            div().w(width).child(Select::new(&demo.model)),
            cx,
        ),
        section(
            "Searched",
            "Typing narrows it to what holds every word typed. It scrolls past ten rows.",
            div().w(width).child(Select::new(&demo.searchable)),
            cx,
        ),
        section(
            "Small, and disabled",
            "",
            row()
                .child(div().w(px(180.)).child(Select::new(&demo.small).small()))
                .child(div().w(px(180.)).child(Select::new(&demo.disabled))),
            cx,
        ),
        section("What they told", "", told(&demo.told, cx), cx),
    ]
}
