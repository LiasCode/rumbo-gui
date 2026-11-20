use gpui::{
    AnyElement, AppContext as _, Context, Entity, ParentElement as _, SharedString, Styled as _,
    Window, div, px,
};
use ui::{Icon, IconName, Input, InputEvent, InputState, Label, Sizable as _, v_flex};

use super::{Demos, note, row, section, told};
use crate::Overview;

pub struct Demo {
    pub name: Entity<InputState>,
    search: Entity<InputState>,
    secret: Entity<InputState>,
    limit: Entity<InputState>,
    disabled: Entity<InputState>,
    sizes: [Entity<InputState>; 3],
    told: Vec<SharedString>,
}

/// A field whose events the page tells, by `name`.
fn field(
    name: &'static str,
    build: impl FnOnce(InputState) -> InputState,
    window: &mut Window,
    cx: &mut Context<Overview>,
) -> Entity<InputState> {
    let state = cx.new(|cx| build(InputState::new(window, cx)));

    cx.subscribe(&state, move |this, state, event: &InputEvent, cx| {
        let line = match event {
            InputEvent::Change => format!("{name}: changed to {:?}", state.read(cx).value()),
            InputEvent::Submit => format!("{name}: sent {:?}", state.read(cx).value()),
            InputEvent::Focus => format!("{name}: has the keyboard"),
            InputEvent::Blur => format!("{name}: let the keyboard go"),
        };

        // A secret is not told.
        let line = match state.read(cx).is_masked() {
            true => format!("{name}: {event:?}"),
            false => line,
        };

        note(&mut this.demos.input.told, line);
        cx.notify();
    })
    .detach();

    state
}

impl Demo {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        Demo {
            name: field(
                "name",
                |field| field.placeholder("Ada Lovelace"),
                window,
                cx,
            ),
            search: field(
                "search",
                |field| field.placeholder("Search sessions"),
                window,
                cx,
            ),
            secret: field(
                "secret",
                |field| field.placeholder("The key the provider gave").masked(true),
                window,
                cx,
            ),
            limit: field("limit", |field| field.default_value("90"), window, cx),
            disabled: field(
                "disabled",
                |field| {
                    field
                        .default_value("anthropic/claude-opus-5-5")
                        .disabled(true)
                },
                window,
                cx,
            ),
            sizes: [
                field(
                    "xsmall",
                    |field| field.placeholder("Extra small"),
                    window,
                    cx,
                ),
                field("small", |field| field.placeholder("Small"), window, cx),
                field("large", |field| field.placeholder("Large"), window, cx),
            ],
            told: Vec::new(),
        }
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.input;
    let field_width = px(320.);

    vec![
        section(
            "A field",
            "Arrows, Home and End move; Ctrl with them moves by words; Shift selects; Ctrl-A, Ctrl-C, Ctrl-X, Ctrl-V, Ctrl-Z, and Ctrl-Shift-Z do what they do everywhere. A double press selects a word, a triple press all.",
            v_flex()
                .w(field_width)
                .gap_2()
                .child(Label::new("Name"))
                .child(Input::new(&demo.name)),
            cx,
        ),
        section(
            "With an icon, and a button that empties it",
            "",
            div().w(field_width).child(
                Input::new(&demo.search)
                    .prefix(Icon::new(IconName::Search))
                    .cleanable(true),
            ),
            cx,
        ),
        section(
            "Secret",
            "A dot for each character. Nothing of it is copied, cut, or handed to an input method, and moving by words moves to its ends.",
            div().w(field_width).child(Input::new(&demo.secret)),
            cx,
        ),
        section(
            "With words after it",
            "",
            div()
                .w(px(200.))
                .child(Input::new(&demo.limit).suffix("turns")),
            cx,
        ),
        section(
            "Disabled",
            "It fades, takes no keys, and Tab goes past it.",
            div().w(field_width).child(Input::new(&demo.disabled)),
            cx,
        ),
        section(
            "Sizes",
            "The height of the button of the same size.",
            row()
                .items_center()
                .child(div().w(px(180.)).child(Input::new(&demo.sizes[0]).xsmall()))
                .child(div().w(px(180.)).child(Input::new(&demo.sizes[1]).small()))
                .child(div().w(px(180.)).child(Input::new(&demo.sizes[2]).large())),
            cx,
        ),
        section(
            "What they told",
            "Each change, Enter, and the keyboard coming and going.",
            told(&demo.told, cx),
            cx,
        ),
    ]
}
