use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, Focusable as _, IntoElement as _,
    ParentElement as _, SharedString, Styled as _, Window, div, px,
};
use ui::{
    ActiveTheme as _, Button, Checkbox, FloatingWindow, Input, InputState, Label, Select,
    SelectState, rems_from_px, v_flex,
};

use super::{Demos, note, row, section, told};
use crate::Overview;

pub struct Demo {
    beside: bool,
    modal: bool,
    name: Entity<InputState>,
    provider: Entity<SelectState>,
    keep: bool,
    told: Vec<SharedString>,
}

impl Demo {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        Demo {
            beside: false,
            modal: false,
            name: cx.new(|cx| InputState::new(window, cx).placeholder("my-provider")),
            provider: cx
                .new(|cx| SelectState::new(super::select::providers(), Some(0), window, cx)),
            keep: true,
            told: Vec::new(),
        }
    }
}

/// What a window's `on_close` calls: `close` stops showing it.
fn closer(
    cx: &Context<Overview>,
    close: fn(&mut Demo),
) -> impl Fn(&mut Window, &mut App) + 'static {
    let overview = cx.weak_entity();

    move |_, cx| {
        let _ = overview.update(cx, |this, cx| {
            close(&mut this.demos.floating_window);
            cx.notify();
        });
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.floating_window;
    let muted = cx.theme().colors.muted_foreground;

    let opening = row()
        .child(
            Button::new("open-beside")
                .outline()
                .label("Open a Window")
                .selected(demo.beside)
                .on_click(cx.listener(|this, _, _, cx| {
                    let demo = &mut this.demos.floating_window;

                    demo.beside = !demo.beside;
                    cx.notify();
                })),
        )
        .child(
            Button::new("open-modal")
                .primary()
                .label("Open a Modal Window")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.demos.floating_window.modal = true;
                    cx.notify();
                })),
        );

    let mut sections = vec![section(
        "Beside the work, and modal",
        "Drag either by its title. Escape or its button closes it, and gives the keyboard back. The modal one dims the work, keeps the pointer from it, and closes with a press on what it dims.",
        v_flex()
            .gap_3()
            .child(opening)
            .child(div().w(px(420.)).child(told(&demo.told, cx))),
        cx,
    )];

    if demo.beside {
        sections.push(
            FloatingWindow::new("beside", "Session details")
                .width(rems_from_px(360.))
                .on_close(closer(cx, |demo| demo.beside = false))
                .child(div().text_color(muted).child(
                    "The work behind stays usable: this window does not dim it, and the pointer reaches it.",
                ))
                .child(Label::new("Folder"))
                .child(div().child("~/my-space/dev/rumbo-agent/rumbo-gui"))
                .into_any_element(),
        );
    }

    if demo.modal {
        let closing = closer(cx, |demo| demo.modal = false);

        let footer = row()
            .child(
                Button::new("modal-cancel")
                    .outline()
                    .label("Cancel")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.demos.floating_window.modal = false;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("modal-add")
                    .primary()
                    .label("Add Provider")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let demo = &mut this.demos.floating_window;
                        let name = demo.name.read(cx).value().to_string();
                        let like = demo
                            .provider
                            .read(cx)
                            .selected_value()
                            .cloned()
                            .unwrap_or_default();

                        note(
                            &mut demo.told,
                            format!("added {name:?} like {like}, kept: {}", demo.keep),
                        );
                        demo.modal = false;
                        cx.notify();
                    })),
            );

        let keep = demo.keep;

        sections.push(
            FloatingWindow::new("modal", "Add a provider")
                .modal(true)
                .initial_focus(demo.name.focus_handle(cx))
                .on_close(closing)
                .footer(footer)
                .child(
                    div()
                        .text_color(muted)
                        .child("A name and how it starts. rumbo starts it to see that it works."),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new("Name"))
                        .child(Input::new(&demo.name)),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new("Starts like"))
                        .child(Select::new(&demo.provider)),
                )
                .child(
                    Checkbox::new("modal-keep")
                        .label("Make it the default provider")
                        .checked(keep)
                        .on_change(cx.listener(|this, checked: &bool, _, cx| {
                            this.demos.floating_window.keep = *checked;
                            cx.notify();
                        })),
                )
                .into_any_element(),
        );
    }

    sections
}
