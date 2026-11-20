//! A palette of commands: a search over groups of them, each with its name
//! and what it does. Typing keeps those whose name holds what is typed, in
//! their order, and marks the first; Up and Down move the mark past those
//! that cannot be chosen, and Enter or a press chooses. It goes in a
//! dialog, which closes it with Escape.

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Pixels, Refineable as _,
    RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, Subscription, Window, actions, div, prelude::FluentBuilder as _, px,
};

use crate::icon::{Icon, IconName};
use crate::input::Input;
use crate::root::v_flex;
use crate::scroll::Scrollbar;
use crate::size::Sizable as _;
use crate::text::{InputEvent, InputState};
use crate::theme::ActiveTheme as _;

actions!(command_palette, [MarkPrevious, MarkNext]);

const CONTEXT: &str = "CommandPalette";

/// The most the list shows before it scrolls.
const LIST_HEIGHT: Pixels = px(360.);

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);

    cx.bind_keys([
        KeyBinding::new("up", MarkPrevious, context),
        KeyBinding::new("down", MarkNext, context),
    ]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItem {
    /// What the search matches.
    pub name: SharedString,
    pub description: SharedString,
    pub disabled: bool,
}

impl CommandItem {
    pub fn new(name: impl Into<SharedString>) -> Self {
        CommandItem {
            name: name.into(),
            description: SharedString::default(),
            disabled: false,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = description.into();
        self
    }

    /// Shown, and never chosen.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandGroup {
    pub label: SharedString,
    pub items: Vec<CommandItem>,
}

impl CommandGroup {
    pub fn new(label: impl Into<SharedString>) -> Self {
        CommandGroup {
            label: label.into(),
            items: Vec::new(),
        }
    }

    pub fn item(mut self, item: CommandItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = CommandItem>) -> Self {
        self.items.extend(items);
        self
    }
}

/// Where a command stands: its group, and its place in the group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandIndex {
    pub group: usize,
    pub item: usize,
}

/// The commands whose name holds `query`, whatever its case, in order.
fn matching(groups: &[CommandGroup], query: &str) -> Vec<CommandIndex> {
    let query = query.trim().to_lowercase();

    groups
        .iter()
        .enumerate()
        .flat_map(|(group, commands)| {
            commands
                .items
                .iter()
                .enumerate()
                .map(move |(item, command)| (CommandIndex { group, item }, command))
        })
        .filter(|(_, command)| command.name.to_lowercase().contains(&query))
        .map(|(index, _)| index)
        .collect()
}

/// The next of `shown` from `marked` that can be chosen, `step` at a time,
/// round from the end to the start; from none, the first or the last.
fn moved(
    groups: &[CommandGroup],
    shown: &[CommandIndex],
    marked: Option<usize>,
    step: isize,
) -> Option<usize> {
    let count = shown.len() as isize;
    let enabled = |row: isize| {
        let index = shown[row as usize];

        !groups[index.group].items[index.item].disabled
    };

    let start = match marked {
        Some(row) => row as isize,
        None if step > 0 => -1,
        None => count,
    };

    (1..=count)
        .map(|taken| (start + step * taken).rem_euclid(count))
        .find(|row| enabled(*row))
        .map(|row| row as usize)
}

pub struct CommandPaletteState {
    search: Entity<InputState>,
    groups: Vec<CommandGroup>,
    shown: Vec<CommandIndex>,
    /// The row of `shown` Enter chooses.
    marked: Option<usize>,
    scroll: ScrollHandle,
    _search: Subscription,
}

impl CommandPaletteState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx));
        let typed = cx.subscribe(&search, |state, _, event: &InputEvent, cx| {
            if *event == InputEvent::Change {
                state.filter(cx);
            }
        });

        CommandPaletteState {
            search,
            groups: Vec::new(),
            shown: Vec::new(),
            marked: None,
            scroll: ScrollHandle::new(),
            _search: typed,
        }
    }

    /// What the search says while nothing is typed.
    pub fn placeholder(self, placeholder: impl Into<SharedString>, cx: &mut App) -> Self {
        let placeholder = placeholder.into();

        self.search
            .update(cx, |search, cx| search.set_placeholder(placeholder, cx));
        self
    }

    /// The commands offered, with nothing typed and the first marked.
    pub fn set_groups(
        &mut self,
        groups: Vec<CommandGroup>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.groups = groups;
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.filter(cx);
    }

    pub fn query(&self, cx: &App) -> String {
        self.search.read(cx).value().to_string()
    }

    /// Gives the search the keyboard.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        let search = self.search.read(cx).focus_handle(cx);

        search.focus(window, cx);
    }

    /// The command Enter chooses.
    pub fn marked(&self) -> Option<CommandIndex> {
        self.marked.and_then(|row| self.shown.get(row).copied())
    }

    fn filter(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);

        self.shown = matching(&self.groups, &query);
        self.marked = moved(&self.groups, &self.shown, None, 1);
        self.scroll.scroll_to_item(0);
        cx.notify();
    }

    fn step(&mut self, step: isize, cx: &mut Context<Self>) {
        self.marked = moved(&self.groups, &self.shown, self.marked, step);

        if let Some(row) = self.marked {
            self.scroll.scroll_to_item(self.child_of(row));
        }

        cx.notify();
    }

    /// Where the row `row` stands among the list's children, which hold a
    /// label before each group's rows.
    fn child_of(&self, row: usize) -> usize {
        let groups_before = self.shown[..=row]
            .iter()
            .enumerate()
            .filter(|(at, index)| *at == 0 || self.shown[at - 1].group != index.group)
            .count();

        row + groups_before
    }
}

/// Its keyboard is its search's.
impl Focusable for CommandPaletteState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.read(cx).focus_handle(cx)
    }
}

type ConfirmHandler = Rc<dyn Fn(CommandIndex, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct CommandPalette {
    state: Entity<CommandPaletteState>,
    on_confirm: Option<ConfirmHandler>,
    style: StyleRefinement,
}

impl CommandPalette {
    pub fn new(state: &Entity<CommandPaletteState>) -> Self {
        CommandPalette {
            state: state.clone(),
            on_confirm: None,
            style: StyleRefinement::default(),
        }
    }

    /// What is called with the command chosen.
    pub fn on_confirm(
        mut self,
        handler: impl Fn(CommandIndex, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_confirm = Some(Rc::new(handler));
        self
    }
}

impl Styled for CommandPalette {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for CommandPalette {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = theme.colors;
        let radius = theme.radius;
        let state = self.state.read(cx);
        let marked = state.marked;
        let on_confirm = self.on_confirm;

        let mut rows: Vec<AnyElement> = Vec::new();

        for (row, index) in state.shown.iter().enumerate() {
            let group = &state.groups[index.group];
            let command = &group.items[index.item];

            if row == 0 || state.shown[row - 1].group != index.group {
                rows.push(
                    div()
                        .px_2()
                        .pt_2()
                        .pb_1()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(group.label.clone())
                        .into_any_element(),
                );
            }

            let lit = marked == Some(row);
            let index = *index;
            let hovered = self.state.clone();
            let chosen = on_confirm.clone();

            rows.push(
                v_flex()
                    .id(("command", row))
                    .debug_selector(|| format!("command-{}", command.name))
                    .w_full()
                    .px_2()
                    .py_1p5()
                    .rounded(radius)
                    .text_sm()
                    .when(lit, |row| {
                        row.bg(colors.accent).text_color(colors.accent_foreground)
                    })
                    .map(|item| match command.disabled {
                        true => item.opacity(0.5),
                        false => item
                            .cursor_pointer()
                            .on_mouse_move(move |_, _, cx| {
                                hovered.update(cx, |state, cx| {
                                    if state.marked != Some(row) {
                                        state.marked = Some(row);
                                        cx.notify();
                                    }
                                })
                            })
                            .when_some(chosen, |item, chosen| {
                                item.on_click(move |_, window, cx| chosen(index, window, cx))
                            }),
                    })
                    .child(div().truncate().child(command.name.clone()))
                    .when(!command.description.is_empty(), |item| {
                        item.child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(command.description.clone()),
                        )
                    })
                    .into_any_element(),
            );
        }

        let nothing = state.shown.is_empty();
        let list = div()
            .id("command-list")
            .max_h(LIST_HEIGHT)
            .overflow_y_scroll()
            .track_scroll(&state.scroll)
            .p_1()
            .children(rows)
            .when(nothing, |list| {
                list.child(
                    div()
                        .px_2()
                        .py_6()
                        .text_center()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child("No commands match"),
                )
            });

        let scroll = state.scroll.clone();
        let search = state.search.clone();
        let stepping_up = self.state.clone();
        let stepping_down = self.state.clone();
        let entered = self.state.clone();

        let mut palette = v_flex()
            .key_context(CONTEXT)
            .w_full()
            .on_action(move |_: &MarkPrevious, _, cx| {
                stepping_up.update(cx, |state, cx| state.step(-1, cx))
            })
            .on_action(move |_: &MarkNext, _, cx| {
                stepping_down.update(cx, |state, cx| state.step(1, cx))
            })
            .on_action(move |_: &crate::text::Enter, window, cx| {
                let marked = entered.read(cx).marked();

                if let (Some(index), Some(chosen)) = (marked, on_confirm.as_ref()) {
                    chosen(index, window, cx);
                }
            })
            .child(
                div().pb_2().border_b_1().border_color(colors.border).child(
                    Input::new(&search)
                        .bare(true)
                        .prefix(Icon::new(IconName::Search).small()),
                ),
            )
            .child(
                div()
                    .relative()
                    .child(list)
                    .child(Scrollbar::vertical("command-scrollbar", scroll)),
            );

        palette.style().refine(&self.style);
        palette
    }
}

/// Commands of the window and of a host, one of which cannot be chosen.
#[cfg(test)]
fn tests_groups() -> Vec<CommandGroup> {
    vec![
        CommandGroup::new("Window")
            .item(CommandItem::new("New Session"))
            .item(CommandItem::new("Changes"))
            .item(CommandItem::new("Not available").disabled(true)),
        CommandGroup::new("Host")
            .item(CommandItem::new("rename"))
            .item(CommandItem::new("compact")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups() -> Vec<CommandGroup> {
        tests_groups()
    }

    fn at(group: usize, item: usize) -> CommandIndex {
        CommandIndex { group, item }
    }

    #[test]
    fn keeps_the_commands_whose_name_holds_what_is_typed() {
        let groups = groups();

        assert_eq!(matching(&groups, "").len(), 5);
        assert_eq!(matching(&groups, "SESS"), [at(0, 0)]);
        assert_eq!(matching(&groups, "c"), [at(0, 1), at(1, 1)]);
        assert_eq!(matching(&groups, "zzz"), []);
    }

    #[test]
    fn moves_past_what_cannot_be_chosen_and_round() {
        let groups = groups();
        let shown = matching(&groups, "");

        assert_eq!(moved(&groups, &shown, None, 1), Some(0));
        assert_eq!(moved(&groups, &shown, None, -1), Some(4));

        // From Changes down, past what is not available, to rename.
        assert_eq!(moved(&groups, &shown, Some(1), 1), Some(3));
        assert_eq!(moved(&groups, &shown, Some(3), -1), Some(1));

        // Past the end, back to the start.
        assert_eq!(moved(&groups, &shown, Some(4), 1), Some(0));

        // Nothing to mark when nothing can be chosen.
        let shown = matching(&groups, "available");

        assert_eq!(moved(&groups, &shown, None, 1), None);
        assert_eq!(moved(&groups, &[], None, 1), None);
    }
}

/// The keys of a palette, in a window GPUI keeps in the test's own process.
#[cfg(test)]
mod key_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{
        AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
        TestAppContext, VisualTestContext, Window, div, px,
    };

    use super::{CommandIndex, CommandPalette, CommandPaletteState, tests_groups};

    struct Palette {
        state: Entity<CommandPaletteState>,
        chosen: Rc<RefCell<Vec<CommandIndex>>>,
    }

    impl Render for Palette {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let chosen = self.chosen.clone();

            div().w(px(400.)).child(
                CommandPalette::new(&self.state)
                    .on_confirm(move |index, _, _| chosen.borrow_mut().push(index)),
            )
        }
    }

    fn palette(
        cx: &mut TestAppContext,
    ) -> (Rc<RefCell<Vec<CommandIndex>>>, &mut VisualTestContext) {
        cx.update(|cx| {
            crate::theme::init(cx);
            crate::text::bind_keys(cx);
            super::bind_keys(cx);
        });

        let chosen = Rc::new(RefCell::new(Vec::new()));
        let told = chosen.clone();
        let (view, cx) = cx.add_window_view(move |window, cx| {
            let state = cx.new(|cx| {
                let mut state = CommandPaletteState::new(window, cx);

                state.set_groups(tests_groups(), window, cx);
                state
            });

            Palette {
                state,
                chosen: told,
            }
        });

        cx.update(|window, cx| {
            let state = view.read(cx).state.clone();
            let search = gpui::Focusable::focus_handle(state.read(cx), cx);

            search.focus(window, cx);
        });
        cx.run_until_parked();

        (chosen, cx)
    }

    fn at(group: usize, item: usize) -> CommandIndex {
        CommandIndex { group, item }
    }

    #[gpui::test]
    fn chooses_the_first_match_or_the_one_marked(cx: &mut TestAppContext) {
        let (chosen, cx) = palette(cx);

        cx.simulate_input("c");
        cx.simulate_keystrokes("enter");

        assert_eq!(*chosen.borrow(), [at(0, 1)]);

        // Down to the next match, and round past the end.
        cx.simulate_keystrokes("down enter down enter");

        assert_eq!(*chosen.borrow(), [at(0, 1), at(1, 1), at(0, 1)]);
    }

    #[gpui::test]
    fn passes_over_what_cannot_be_chosen(cx: &mut TestAppContext) {
        let (chosen, cx) = palette(cx);

        cx.simulate_keystrokes("down down enter");

        assert_eq!(*chosen.borrow(), [at(1, 0)]);

        // Only what cannot be chosen matches: Enter chooses nothing.
        cx.simulate_input("available");
        cx.simulate_keystrokes("enter");

        assert_eq!(chosen.borrow().len(), 1);
    }
}
