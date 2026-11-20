use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, Focusable as _, ParentElement as _,
    SharedString, Styled as _, WeakEntity, Window, div, px,
};
use ui::{
    ActiveTheme as _, Button, IconName, Input, InputEvent, InputState, Label, Placement, Popover,
    PopoverControl, Sizable as _, h_flex, v_flex,
};

use super::{Demos, note, row, section, told};
use crate::Overview;

pub struct Demo {
    title: Entity<InputState>,
    /// What closes the popover the title is typed in, while it is open.
    renaming: Rc<RefCell<Option<PopoverControl>>>,
    told: Vec<SharedString>,
}

impl Demo {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        let title = cx.new(|cx| InputState::new(window, cx).default_value("Fix the failing test"));
        let renaming: Rc<RefCell<Option<PopoverControl>>> = Rc::default();
        let entered = renaming.clone();

        // Enter in the field does what Save does.
        cx.subscribe_in(
            &title,
            window,
            move |this, title, event: &InputEvent, window, cx| {
                if *event != InputEvent::Submit {
                    return;
                }

                renamed(this, title.read(cx).value().to_string());
                cx.notify();

                if let Some(control) = entered.borrow().clone() {
                    control.close(window, cx);
                }
            },
        )
        .detach();

        Demo {
            title,
            renaming,
            told: Vec::new(),
        }
    }
}

const PLACEMENTS: [(Placement, &str); 4] = [
    (Placement::BottomStart, "Below, from the start"),
    (Placement::BottomEnd, "Below, from the end"),
    (Placement::TopStart, "Above, from the start"),
    (Placement::TopEnd, "Above, from the end"),
];

/// Tells the title kept.
fn renamed(this: &mut Overview, typed: String) {
    note(
        &mut this.demos.popover.told,
        format!("renamed to {typed:?}"),
    );
}

/// Keeps the title typed, and closes the popover it was typed in.
fn rename(
    title: &Entity<InputState>,
    control: &PopoverControl,
    overview: &WeakEntity<Overview>,
    window: &mut Window,
    cx: &mut App,
) {
    let typed = title.read(cx).value().to_string();

    let _ = overview.update(cx, |this, cx| {
        renamed(this, typed);
        cx.notify();
    });

    control.close(window, cx);
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.popover;
    let muted = cx.theme().colors.muted_foreground;
    let overview = cx.weak_entity();
    let title = demo.title.clone();
    let renaming = demo.renaming.clone();

    let words = Popover::new(
        "words",
        Button::new("words-trigger").outline().label("What is a run?"),
    )
    .w(px(300.))
    .content(move |_, _, _| {
        v_flex()
            .gap_1()
            .text_sm()
            .child(div().child("A run"))
            .child(div().text_color(muted).child(
                "The work from an objective until the model stops calling tools, or the run fails or is cancelled.",
            ))
    });

    let form = Popover::new(
        "form",
        Button::new("form-trigger")
            .outline()
            .icon(IconName::Settings)
            .label("Rename…"),
    )
    .w(px(320.))
    .initial_focus(demo.title.focus_handle(cx))
    .content(move |control, _, _| {
        renaming.replace(Some(control.clone()));

        let saving = (title.clone(), overview.clone());

        v_flex()
            .gap_3()
            .child(Label::new("Title"))
            .child(Input::new(&title))
            .child(
                h_flex().justify_end().child(
                    Button::new("save")
                        .primary()
                        .small()
                        .label("Save")
                        .on_click(move |_, window, cx| {
                            rename(&saving.0, &control, &saving.1, window, cx)
                        }),
                ),
            )
    });

    let placements = row().children(PLACEMENTS.iter().enumerate().map(
        |(index, (placement, name))| {
            Popover::new(
                ("placed", index),
                Button::new(("placed-trigger", index)).ghost().label(*name),
            )
            .placement(*placement)
            .content(|_, _, _| div().text_sm().child("Here."))
        },
    ));

    vec![
        section(
            "Words",
            "A press on the trigger opens it, and closes it again; so do Escape and a press outside.",
            words,
            cx,
        ),
        section(
            "A form",
            "The field takes the keyboard when it opens. Save, or Enter, keeps the title and closes it; Escape gives the keyboard back to the trigger.",
            v_flex()
                .gap_3()
                .child(form)
                .child(div().w(px(320.)).child(told(&demo.told, cx))),
            cx,
        ),
        section(
            "Placement",
            "Below or above, from the trigger's start or its end. It moves to the other side when it does not fit.",
            placements,
            cx,
        ),
    ]
}
