//! A choice of one value from a list, which opens under its trigger. A list
//! can be searched as it is typed in. The keyboard opens it with Up, Down,
//! Enter, or Space, moves through it with Up and Down, chooses with Enter,
//! and closes it with Escape.

use gpui::{
    App, AppContext as _, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable as _,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Pixels, Refineable as _,
    RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, Subscription, Window, actions, anchored, deferred, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::icon::{Icon, IconName};
use crate::input::Input;
use crate::overlay::{self, Dismiss};
use crate::root::{h_flex, v_flex};
use crate::size::{Sizable, Size};
use crate::text::{InputEvent, InputState};
use crate::theme::ActiveTheme as _;

actions!(select, [SelectPrevious, SelectNext, Confirm]);

/// The trigger's key context.
const TRIGGER_CONTEXT: &str = "Select";

/// The most a list shows before it scrolls.
const LIST_HEIGHT: Pixels = px(300.);

pub(crate) fn bind_keys(cx: &mut App) {
    let list = Some(overlay::SELECT_CONTEXT);
    let trigger = Some(TRIGGER_CONTEXT);

    cx.bind_keys([
        KeyBinding::new("up", SelectPrevious, trigger),
        KeyBinding::new("down", SelectNext, trigger),
        KeyBinding::new("up", SelectPrevious, list),
        KeyBinding::new("down", SelectNext, list),
        KeyBinding::new("enter", Confirm, list),
    ]);
}

/// One value of a list, with what a person reads for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectItem {
    pub value: SharedString,
    pub label: SharedString,
    /// What tells it apart, said beside it.
    pub description: SharedString,
}

impl SelectItem {
    pub fn new(value: impl Into<SharedString>) -> Self {
        let value = value.into();

        SelectItem {
            label: value.clone(),
            value,
            description: SharedString::default(),
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = description.into();
        self
    }

    /// Whether it holds every word of `query`, in any case.
    fn matches(&self, query: &str) -> bool {
        let haystack = format!("{} {} {}", self.label, self.value, self.description).to_lowercase();

        query
            .split_whitespace()
            .all(|word| haystack.contains(&word.to_lowercase()))
    }
}

/// What a select tells those who subscribe to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectEvent {
    /// The person chose this value.
    Change(SharedString),
}

pub struct SelectState {
    items: Vec<SelectItem>,
    selected: Option<usize>,
    placeholder: SharedString,
    searchable: bool,
    disabled: bool,
    search: Entity<InputState>,
    /// The items the search leaves, by their index.
    matches: Vec<usize>,
    open: bool,
    /// Which of `matches` the keyboard and the pointer are on.
    highlighted: usize,
    trigger_focus: FocusHandle,
    list_focus: FocusHandle,
    /// Where the trigger was last painted.
    trigger_bounds: Bounds<Pixels>,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SelectEvent> for SelectState {}

impl SelectState {
    pub fn new(
        items: Vec<SelectItem>,
        selected: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let subscriptions = vec![cx.subscribe_in(
            &search,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => this.searched(cx),
                InputEvent::Submit => this.confirm(&Confirm, window, cx),
                _ => {}
            },
        )];

        let matches = (0..items.len()).collect();

        SelectState {
            selected: selected.filter(|index| *index < items.len()),
            items,
            placeholder: "Choose…".into(),
            searchable: false,
            disabled: false,
            search,
            matches,
            open: false,
            highlighted: 0,
            trigger_focus: cx.focus_handle().tab_stop(true),
            list_focus: cx.focus_handle(),
            trigger_bounds: Bounds::default(),
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    /// A field over the list narrows it to what holds what is typed.
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }

    /// What the trigger says while nothing is chosen.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn items(&self) -> &[SelectItem] {
        &self.items
    }

    pub fn selected_item(&self) -> Option<&SelectItem> {
        self.items.get(self.selected?)
    }

    pub fn selected_value(&self) -> Option<&SharedString> {
        self.selected_item().map(|item| &item.value)
    }

    /// Replaces the list, keeping what was chosen when it is still there.
    pub fn set_items(&mut self, items: Vec<SelectItem>, cx: &mut Context<Self>) {
        let chosen = self.selected_value().cloned();

        self.selected =
            chosen.and_then(|chosen| items.iter().position(|item| item.value == chosen));
        self.items = items;
        self.searched(cx);
    }

    /// Chooses the item with this value, or nothing when none has it.
    pub fn set_selected_value(&mut self, value: &str, cx: &mut Context<Self>) {
        self.selected = self.items.iter().position(|item| item.value == value);
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    /// Gives the keyboard to its trigger.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.trigger_focus.focus(window, cx);
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    fn searched(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value().to_string();

        self.matches = (0..self.items.len())
            .filter(|index| self.items[*index].matches(&query))
            .collect();
        self.highlighted = 0;
        cx.notify();
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.open {
            return;
        }

        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.matches = (0..self.items.len()).collect();
        self.highlighted = self
            .selected
            .and_then(|selected| self.matches.iter().position(|index| *index == selected))
            .unwrap_or(0);
        self.open = true;

        // GPUI scrolls to an item against where the list was laid out the
        // frame before, and it was not laid out before this one: the chosen
        // item is brought into view on the next.
        cx.on_next_frame(window, |this, _, cx| {
            this.scroll.scroll_to_item(this.highlighted);
            cx.notify();
        });

        match self.searchable {
            true => {
                let search = self.search.focus_handle(cx);

                search.focus(window, cx);
            }
            false => self.list_focus.focus(window, cx),
        }

        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }

        self.open = false;
        self.trigger_focus.focus(window, cx);
        cx.notify();
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.open {
            true => self.close(window, cx),
            false => self.open(window, cx),
        }
    }

    fn choose(&mut self, highlighted: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.matches.get(highlighted).copied() else {
            return;
        };

        self.selected = Some(index);
        cx.emit(SelectEvent::Change(self.items[index].value.clone()));
        self.close(window, cx);
    }

    fn step(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            self.open(window, cx);
            return;
        }

        let count = self.matches.len();

        if count == 0 {
            return;
        }

        self.highlighted = (self.highlighted as isize + step).rem_euclid(count as isize) as usize;
        self.scroll.scroll_to_item(self.highlighted);
        cx.notify();
    }

    fn previous(&mut self, _: &SelectPrevious, window: &mut Window, cx: &mut Context<Self>) {
        self.step(-1, window, cx);
    }

    fn next(&mut self, _: &SelectNext, window: &mut Window, cx: &mut Context<Self>) {
        self.step(1, window, cx);
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        self.choose(self.highlighted, window, cx);
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        self.close(window, cx);
    }
}

#[derive(IntoElement)]
pub struct Select {
    state: Entity<SelectState>,
    size: Size,
    style: StyleRefinement,
}

impl Select {
    pub fn new(state: &Entity<SelectState>) -> Self {
        Select {
            state: state.clone(),
            size: Size::Medium,
            style: StyleRefinement::default(),
        }
    }
}

impl Sizable for Select {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

/// Its style is its trigger's, such as its width.
impl Styled for Select {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Select {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = theme.colors;
        let state = self.state.read(cx);
        let size = self.size;
        let id = self.state.entity_id().as_u64();
        let ring = theme.focus_ring();

        let (label, chosen) = match state.selected_item() {
            Some(item) => (item.label.clone(), true),
            None => (state.placeholder.clone(), false),
        };

        let padding = match size {
            Size::XSmall => px(6.),
            Size::Small => px(8.),
            Size::Medium | Size::Large => px(12.),
        };

        let mut trigger = h_flex()
            .id(("select", id))
            .w_full()
            .h(size.height())
            .px(padding)
            .gap_2()
            .justify_between()
            .rounded(theme.radius)
            .border_1()
            .border_color(colors.input)
            .bg(colors.background)
            .text_size(size.text())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(!chosen, |label| label.text_color(colors.muted_foreground))
                    .child(label),
            )
            .child(
                Icon::new(IconName::ChevronDown)
                    .with_size(size)
                    .color(colors.muted_foreground),
            );

        if state.disabled {
            trigger = trigger.opacity(0.5).cursor_default();
        } else {
            trigger = trigger
                .cursor_pointer()
                .key_context(TRIGGER_CONTEXT)
                .track_focus(&state.trigger_focus)
                .focus_visible(move |style| style.shadow(ring))
                .when(state.open, |trigger| trigger.border_color(colors.ring))
                .on_action(window.listener_for(&self.state, SelectState::previous))
                .on_action(window.listener_for(&self.state, SelectState::next))
                .on_click(
                    window
                        .listener_for(&self.state, |state, _, window, cx| state.toggle(window, cx)),
                );
        }

        trigger.style().refine(&self.style);

        let tracking = self.state.clone();
        let select = div()
            .child(trigger)
            .on_children_prepainted(move |bounds, _, cx| {
                if let Some(bounds) = bounds.first() {
                    tracking.update(cx, |state, _| state.trigger_bounds = *bounds);
                }
            });

        if !state.open {
            return select;
        }

        let trigger_bounds = state.trigger_bounds;
        let highlighted = state.highlighted;
        let selected = state.selected;
        let muted = colors.muted_foreground;

        let rows = state.matches.iter().enumerate().map(|(row, index)| {
            let item = &state.items[*index];
            let lit = row == highlighted;

            h_flex()
                .id(("select-item", row))
                .w_full()
                .px_2()
                .py_1p5()
                .gap_2()
                .rounded_sm()
                .cursor_pointer()
                .text_sm()
                .when(lit, |row| {
                    row.bg(colors.accent).text_color(colors.accent_foreground)
                })
                .on_mouse_move(window.listener_for(&self.state, move |state, _, _, cx| {
                    if state.highlighted != row {
                        state.highlighted = row;
                        cx.notify();
                    }
                }))
                .on_click(
                    window.listener_for(&self.state, move |state, _, window, cx| {
                        state.choose(row, window, cx)
                    }),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().child(item.label.clone()))
                        .when(!item.description.is_empty(), |column| {
                            column.child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(item.description.clone()),
                            )
                        }),
                )
                .when(selected == Some(*index), |row| {
                    row.child(Icon::new(IconName::Check).small())
                })
        });

        let list = div()
            .id(("select-list", id))
            .max_h(LIST_HEIGHT)
            .overflow_y_scroll()
            .track_scroll(&state.scroll)
            .p_1()
            .children(rows)
            .when(state.matches.is_empty(), |list| {
                list.child(
                    div()
                        .px_2()
                        .py_1p5()
                        .text_sm()
                        .text_color(muted)
                        .child("Nothing matches"),
                )
            });

        let closing = self.state.clone();
        let surface = overlay::surface(cx)
            .min_w(trigger_bounds.size.width)
            .max_w(trigger_bounds.size.width.max(px(360.)))
            .key_context(overlay::SELECT_CONTEXT)
            .when(!state.searchable, |surface| {
                surface.track_focus(&state.list_focus)
            })
            .occlude()
            .on_action(window.listener_for(&self.state, SelectState::previous))
            .on_action(window.listener_for(&self.state, SelectState::next))
            .on_action(window.listener_for(&self.state, SelectState::confirm))
            .on_action(window.listener_for(&self.state, SelectState::dismiss))
            // A press on the trigger closes it as the trigger's press.
            .on_mouse_down_out(move |event, window, cx| {
                if !trigger_bounds.contains(&event.position) {
                    closing.update(cx, |state, cx| state.close(window, cx));
                }
            })
            .when(state.searchable, |surface| {
                surface.child(
                    div().p_1().border_b_1().border_color(colors.border).child(
                        Input::new(&state.search)
                            .small()
                            .bare(true)
                            .prefix(Icon::new(IconName::Search).small()),
                    ),
                )
            })
            .child(list);

        select.child(
            deferred(
                anchored()
                    .attach(trigger_bounds, gpui::Anchor::BottomLeft)
                    .anchor(gpui::Anchor::TopLeft)
                    .offset(point(px(0.), px(4.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(surface),
            )
            // Over a floating window or a popover, which a select can be in.
            .with_priority(3),
        )
    }
}
