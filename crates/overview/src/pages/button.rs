use gpui::{AnyElement, Context, ParentElement as _, SharedString, Styled as _, Window, px};
use ui::{Button, ButtonVariant, IconName, Sizable as _, Size, h_flex};

use super::{Demos, row, section, told};
use crate::Overview;

#[derive(Default)]
pub struct Demo {
    presses: usize,
    /// Which of a group of choices is chosen.
    chosen: usize,
    told: Vec<SharedString>,
}

const VARIANTS: [(ButtonVariant, &str); 6] = [
    (ButtonVariant::Primary, "Primary"),
    (ButtonVariant::Secondary, "Secondary"),
    (ButtonVariant::Outline, "Outline"),
    (ButtonVariant::Ghost, "Ghost"),
    (ButtonVariant::Danger, "Danger"),
    (ButtonVariant::Link, "Link"),
];

const SIZES: [(Size, &str); 4] = [
    (Size::XSmall, "Extra small"),
    (Size::Small, "Small"),
    (Size::Medium, "Medium"),
    (Size::Large, "Large"),
];

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.button;

    let variants = row().children(VARIANTS.iter().map(|(variant, name)| {
        Button::new(("variant", *variant as usize))
            .label(*name)
            .variant(*variant)
    }));

    let disabled = row().children(VARIANTS.iter().map(|(variant, name)| {
        Button::new(("disabled", *variant as usize))
            .label(*name)
            .variant(*variant)
            .disabled(true)
    }));

    let sizes = row()
        .children(SIZES.iter().map(|(size, name)| {
            Button::new(("size", *size as usize))
                .label(*name)
                .outline()
                .with_size(*size)
        }))
        .children(SIZES.iter().map(|(size, _)| {
            Button::new(("size-icon", *size as usize))
                .icon(IconName::Settings)
                .outline()
                .with_size(*size)
                .tooltip("Settings")
        }));

    let icons = row()
        .child(
            Button::new("new")
                .primary()
                .icon(IconName::Plus)
                .label("New Session"),
        )
        .child(
            Button::new("copy")
                .outline()
                .icon(IconName::Copy)
                .label("Copy"),
        )
        .child(
            Button::new("delete")
                .danger()
                .icon(IconName::Trash)
                .label("Delete"),
        )
        .child(
            Button::new("icon-settings")
                .ghost()
                .icon(IconName::Settings)
                .tooltip("Settings")
                .shortcut("Ctrl-,"),
        )
        .child(
            Button::new("icon-copy")
                .ghost()
                .icon(IconName::Copy)
                .tooltip("Copy"),
        )
        .child(
            Button::new("icon-send")
                .primary()
                .icon(IconName::ArrowUp)
                .rounded_full()
                .tooltip("Send the task")
                .shortcut("Enter"),
        );

    let choices = ["Light", "Dark", "System"];
    let group = h_flex()
        .gap_1()
        .children(choices.iter().enumerate().map(|(index, name)| {
            Button::new(("choice", index))
                .outline()
                .small()
                .label(*name)
                .selected(demo.chosen == index)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.demos.button.chosen = index;
                    super::note(
                        &mut this.demos.button.told,
                        format!("Chose {}", choices[index]),
                    );
                    cx.notify();
                }))
        }));

    let presses = demo.presses;
    let pressed = row()
        .child(
            Button::new("press")
                .primary()
                .label(match presses {
                    0 => "Press me".to_string(),
                    1 => "Pressed once".to_string(),
                    presses => format!("Pressed {presses} times"),
                })
                .on_click(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                    let demo = &mut this.demos.button;
                    let by = match event {
                        gpui::ClickEvent::Keyboard(_) => "the keyboard",
                        _ => "the pointer",
                    };

                    demo.presses += 1;
                    super::note(&mut demo.told, format!("Pressed with {by}"));
                    cx.notify();
                })),
        )
        .child(
            Button::new("reset")
                .ghost()
                .label("Reset")
                .disabled(presses == 0)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.demos.button.presses = 0;
                    cx.notify();
                })),
        );

    vec![
        section(
            "Variants",
            "Primary for the one thing a view is for; Outline among equal choices; Ghost where it should not draw the eye; Danger for what cannot be undone.",
            variants,
            cx,
        ),
        section(
            "Sizes",
            "Every control of a size has the same height, so they line up in a row. A button with only an icon is square.",
            sizes,
            cx,
        ),
        section(
            "With icons",
            "An icon before the label, or alone with a tooltip that says what it does, and the keys that do the same.",
            icons,
            cx,
        ),
        section(
            "Selected",
            "A button that stays pressed: the choice made in a group, or the trigger of what is open.",
            group,
            cx,
        ),
        section(
            "Disabled",
            "It fades, takes no press, and Tab goes past it.",
            disabled,
            cx,
        ),
        section(
            "Pressed",
            "Tab to it and press Enter or Space, or press it with the pointer.",
            gpui::div()
                .flex()
                .flex_col()
                .gap_3()
                .child(pressed)
                .child(told(&demo.told, cx)),
            cx,
        ),
        section(
            "Full width",
            "",
            gpui::div()
                .w(px(320.))
                .child(Button::new("wide").primary().label("Continue").w_full()),
            cx,
        ),
        section(
            "Long words",
            "Words wider than the button wrap, and it grows taller to hold them; truncated, they stay on one line, cut short.",
            gpui::div()
                .w(px(320.))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    Button::new("long-wraps")
                        .outline()
                        .small()
                        .w_full()
                        .label(LONG),
                )
                .child(
                    Button::new("long-starts")
                        .outline()
                        .small()
                        .w_full()
                        .justify_start()
                        .icon(IconName::Clock)
                        .label(LONG),
                )
                .child(
                    Button::new("long-truncated")
                        .outline()
                        .small()
                        .w_full()
                        .justify_start()
                        .truncate()
                        .icon(IconName::Clock)
                        .label(LONG),
                ),
            cx,
        ),
    ]
}

/// Words wider than a button of the page.
const LONG: &str =
    "Move every configuration value behind the config module, so that startup checks it";
