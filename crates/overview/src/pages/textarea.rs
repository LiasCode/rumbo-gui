use gpui::{
    AnyElement, AppContext as _, Context, Entity, ParentElement as _, SharedString, Styled as _,
    Window, div, px,
};
use ui::{
    ActiveTheme as _, Button, IconName, InputEvent, InputState, Label, Sizable as _, Textarea,
    h_flex, v_flex,
};

use super::{Demos, note, section, told};
use crate::Overview;

pub struct Demo {
    notes: Entity<InputState>,
    fixed: Entity<InputState>,
    composer: Entity<InputState>,
    disabled: Entity<InputState>,
    sent: Vec<SharedString>,
}

const LONG: &str = "rumbo is a general agent harness: it is not tied to programming, it does not judge the model's work, and it sends no context beyond the environment, the user's instructions, and what the user's skills are for.\n\nA run is the work from an objective until the model stops calling tools, or the run fails or is cancelled. A session holds one or more runs.\n\nThis field shows four rows, and scrolls past them: with the wheel, or by moving the cursor.";

impl Demo {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        let composer = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(1, 8)
                .submit_on_enter(true)
                .placeholder("Give rumbo a task")
        });

        cx.subscribe_in(
            &composer,
            window,
            |this, composer, event: &InputEvent, window, cx| {
                if *event != InputEvent::Submit {
                    return;
                }

                send(this, composer, window, cx);
            },
        )
        .detach();

        Demo {
            notes: cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(2, 6)
                    .placeholder("Grows from two rows to six as it fills")
            }),
            fixed: cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(4, 4)
                    .default_value(LONG)
            }),
            composer,
            disabled: cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(2, 2)
                    .default_value("A run is in progress.")
                    .disabled(true)
            }),
            sent: Vec::new(),
        }
    }
}

/// Takes what the composer holds as sent, and empties it.
fn send(
    this: &mut Overview,
    composer: &Entity<InputState>,
    window: &mut Window,
    cx: &mut Context<Overview>,
) {
    let text = composer.read(cx).value().trim().to_string();

    if text.is_empty() {
        return;
    }

    note(&mut this.demos.textarea.sent, format!("sent {text:?}"));
    composer.update(cx, |composer, cx| composer.set_value("", window, cx));
    cx.notify();
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.textarea;
    let theme = cx.theme();
    let width = px(480.);
    let composer = demo.composer.clone();

    let card = v_flex()
        .w(width)
        .gap_1()
        .px_2()
        .pt_1()
        .pb_2()
        .rounded_xl()
        .border_1()
        .border_color(theme.colors.input)
        .bg(theme.colors.background)
        .shadow_sm()
        .child(Textarea::new(&demo.composer).bare(true))
        .child(
            h_flex().justify_end().child(
                Button::new("send")
                    .primary()
                    .small()
                    .rounded_full()
                    .icon(IconName::ArrowUp)
                    .tooltip("Send the task")
                    .shortcut("Enter")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        send(this, &composer, window, cx);
                    })),
            ),
        );

    vec![
        section(
            "Growing",
            "Enter starts a line. Up and Down move between rows, as they are wrapped.",
            v_flex()
                .w(width)
                .gap_2()
                .child(Label::new("Notes"))
                .child(Textarea::new(&demo.notes)),
            cx,
        ),
        section(
            "Of a fixed height",
            "Four rows, scrolled.",
            div().w(width).child(Textarea::new(&demo.fixed)),
            cx,
        ),
        section(
            "A composer",
            "Bare, in a card of its own. Enter sends it, Shift-Enter starts a line; it grows to eight rows.",
            v_flex()
                .gap_3()
                .child(card)
                .child(div().w(width).child(told(&demo.sent, cx))),
            cx,
        ),
        section(
            "Disabled",
            "",
            div().w(width).child(Textarea::new(&demo.disabled).small()),
            cx,
        ),
    ]
}
