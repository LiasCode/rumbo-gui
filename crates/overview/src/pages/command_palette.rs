use gpui::{
    AnyElement, AppContext as _, Context, Entity, Focusable as _, ParentElement as _, SharedString,
    Styled as _, Window, div, px,
};
use ui::{
    ActiveTheme as _, Button, CommandGroup, CommandIndex, CommandItem, CommandPalette,
    CommandPaletteState, v_flex,
};

use super::{Demos, note, section, told};
use crate::Overview;

fn groups() -> Vec<CommandGroup> {
    vec![
        CommandGroup::new("Window")
            .item(
                CommandItem::new("New Session").description("Start the next task in a new session"),
            )
            .item(CommandItem::new("Changes").description("What changed in the project's folder"))
            .item(CommandItem::new("Settings…").description("The theme, and rumbo's settings")),
        CommandGroup::new("Sessions")
            .item(CommandItem::new("Fix the failing test").description("2 hours ago"))
            .item(CommandItem::new("Write the release notes").description("Yesterday")),
        CommandGroup::new("Provider toy")
            .item(CommandItem::new("ask").description("Asks a question, and says the answer"))
            .item(CommandItem::new("Not available: the provider did not start").disabled(true)),
    ]
}

pub struct Demo {
    inline: Entity<CommandPaletteState>,
    dialog: Entity<CommandPaletteState>,
    told: Vec<SharedString>,
}

impl Demo {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        let palette = |window: &mut Window, cx: &mut Context<Overview>| {
            cx.new(|cx| {
                let mut state = CommandPaletteState::new(window, cx)
                    .placeholder("Type a command or a session", cx);

                state.set_groups(groups(), window, cx);
                state
            })
        };

        Demo {
            inline: palette(window, cx),
            dialog: palette(window, cx),
            told: Vec::new(),
        }
    }
}

/// The name of the command at `index`.
fn named(index: CommandIndex) -> String {
    groups()[index.group].items[index.item].name.to_string()
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let demo = &demos.command_palette;
    let overview = cx.weak_entity();

    let inline = CommandPalette::new(&demo.inline).on_confirm(move |index, _, cx| {
        overview
            .update(cx, |this, cx| {
                note(
                    &mut this.demos.command_palette.told,
                    format!("Chose {}", named(index)),
                );
                cx.notify();
            })
            .ok();
    });

    let open = Button::new("open-palette")
        .outline()
        .label("Open the Palette")
        .shortcut("Ctrl-P")
        .on_click(cx.listener(|this, _, window, cx| {
            let state = this.demos.command_palette.dialog.clone();
            let overview = cx.weak_entity();
            let dialogs = this.dialogs.clone();

            state.update(cx, |state, cx| state.set_groups(groups(), window, cx));

            this.dialogs.update(cx, |opened, cx| {
                opened.open(
                    move |dialog, _, cx| {
                        let overview = overview.clone();
                        let dialogs = dialogs.clone();

                        dialog
                            .title("Commands")
                            .width(px(480.))
                            .initial_focus(state.focus_handle(cx))
                            .child(CommandPalette::new(&state).on_confirm(
                                move |index, window, cx| {
                                    dialogs.update(cx, |dialogs, cx| dialogs.close(window, cx));
                                    overview
                                        .update(cx, |this, cx| {
                                            note(
                                                &mut this.demos.command_palette.told,
                                                format!("Chose {} from the dialog", named(index)),
                                            );
                                            cx.notify();
                                        })
                                        .ok();
                                },
                            ))
                    },
                    window,
                    cx,
                )
            });
        }));

    let muted = cx.theme().colors.muted_foreground;

    vec![
        section(
            "In place",
            "Type to keep the commands whose name holds it; Up and Down pass over what is not available, and Enter or a press chooses.",
            v_flex()
                .w(px(480.))
                .gap_3()
                .child(inline.w_full())
                .child(told(&demo.told, cx)),
            cx,
        ),
        section(
            "In a dialog",
            "As the window opens it with Ctrl-P. Escape closes it, giving the keyboard back.",
            v_flex().items_start().gap_2().child(open).child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("It opens with nothing typed."),
            ),
            cx,
        ),
    ]
}
