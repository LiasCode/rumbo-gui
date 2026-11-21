//! A row of names of which one is shown, for the parts of one view: the
//! caller shows the part of the tab chosen under the row. Pressing a tab
//! chooses it. The row is one stop for Tab; Left and Right go to the tab
//! beside, coming round at the ends, and Home and End to the first and the
//! last, past those disabled, choosing the one gone to.

use std::rc::Rc;

use gpui::{
    App, ElementId, FontWeight, InteractiveElement as _, IntoElement, KeyBinding,
    ParentElement as _, Refineable as _, RenderOnce, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, actions, div, prelude::FluentBuilder as _, px,
};

use crate::focus;
use crate::icon::{Icon, IconName};
use crate::root::h_flex;
use crate::roving::{self, Step};
use crate::size::{Sizable, Size, rems_from_px};
use crate::theme::ActiveTheme as _;
use crate::{ChangeHandler, child_id};

actions!(tabs, [Previous, Next, First, Last]);

/// A row of tabs' key context.
const CONTEXT: &str = "Tabs";

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);

    cx.bind_keys([
        KeyBinding::new("left", Previous, context),
        KeyBinding::new("right", Next, context),
        KeyBinding::new("home", First, context),
        KeyBinding::new("end", Last, context),
    ]);
}

/// How a row of tabs looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabsVariant {
    /// The tabs on a quiet ground, the one chosen raised from it.
    #[default]
    Pill,
    /// The tabs over a line, the one chosen marked on it.
    Underline,
}

/// One tab of a row.
pub struct Tab {
    label: SharedString,
    icon: Option<IconName>,
    disabled: bool,
}

impl Tab {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Tab {
            label: label.into(),
            icon: None,
            disabled: false,
        }
    }

    /// An icon before its name.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    tabs: Vec<Tab>,
    selected: usize,
    variant: TabsVariant,
    size: Size,
    on_change: Option<ChangeHandler<usize>>,
    style: StyleRefinement,
}

impl Tabs {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Tabs {
            id: id.into(),
            tabs: Vec::new(),
            selected: 0,
            variant: TabsVariant::default(),
            size: Size::Medium,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn child(mut self, tab: Tab) -> Self {
        self.tabs.push(tab);
        self
    }

    pub fn children(mut self, tabs: impl IntoIterator<Item = Tab>) -> Self {
        self.tabs.extend(tabs);
        self
    }

    /// The tab chosen, by its place.
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }

    pub fn variant(mut self, variant: TabsVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn underline(self) -> Self {
        self.variant(TabsVariant::Underline)
    }

    /// Called with the place of the tab chosen; the caller shows its part
    /// and gives it back with `selected`.
    pub fn on_change(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Sizable for Tabs {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Tabs {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Tabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = focus::focus_of(&self.id, window, cx);
        let ringed = focus::ringed(&focus, window);
        let theme = cx.theme();
        let colors = theme.colors;
        let radius = theme.radius;
        let ring = theme.focus_ring();
        let size = self.size;
        let pill = self.variant == TabsVariant::Pill;
        let selected = self.selected;
        let enabled: Rc<[bool]> = self.tabs.iter().map(|tab| !tab.disabled).collect();

        let padding = match size {
            Size::XSmall => rems_from_px(6.),
            Size::Small => rems_from_px(8.),
            Size::Medium | Size::Large => rems_from_px(12.),
        };

        let tabs = self.tabs.into_iter().enumerate().map(|(index, tab)| {
            let chosen = index == selected;
            let ring = ring.clone();

            let mut item = h_flex()
                .id(child_id(&self.id, format!("tab-{index}")))
                .debug_selector(|| format!("tab-{index}"))
                .relative()
                .h_full()
                .px(padding)
                .gap_1p5()
                .justify_center()
                .whitespace_nowrap()
                .text_color(match chosen {
                    true => colors.foreground,
                    false => colors.muted_foreground,
                })
                .when_some(tab.icon, |item, icon| {
                    item.child(Icon::new(icon).with_size(size))
                })
                .child(tab.label)
                .rounded(radius)
                .when(ringed && chosen, |item| item.shadow(ring))
                .map(|item| match pill {
                    true => item.when(chosen, |item| item.bg(colors.background).shadow_sm()),
                    false => item.when(chosen, |item| {
                        item.child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .h(px(2.))
                                .bg(colors.primary),
                        )
                    }),
                });

            if tab.disabled {
                item = item.opacity(0.5);
            } else if !chosen {
                item = item
                    .cursor_pointer()
                    .hover(move |style| style.text_color(colors.foreground));

                if let Some(on_change) = &self.on_change {
                    let on_change = on_change.clone();
                    let focus = focus.clone();

                    item = item.on_click(move |_, window, cx| {
                        window.focus(&focus, cx);
                        on_change(&index, window, cx);
                    });
                }
            }

            item
        });

        let mut row = h_flex()
            .id(self.id.clone())
            .flex_none()
            .h(size.height())
            .text_size(size.text())
            .font_weight(FontWeight::MEDIUM)
            .map(|row| match pill {
                true => row
                    .p(rems_from_px(3.))
                    .rounded(theme.radius_lg)
                    .bg(colors.muted),

                // The line goes under the tabs, as a border would go over the
                // mark of the one chosen.
                false => row.relative().gap_1().child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(1.))
                        .bg(colors.border),
                ),
            })
            .children(tabs);

        if let Some(on_change) = self.on_change {
            let keys = |step: Step| {
                let on_change = on_change.clone();
                let enabled = enabled.clone();

                move |window: &mut Window, cx: &mut App| {
                    if let Some(to) = roving::moved(Some(selected), &enabled, step) {
                        on_change(&to, window, cx);
                    }
                }
            };

            let previous = keys(Step::Previous);
            let next = keys(Step::Next);
            let first = keys(Step::First);
            let last = keys(Step::Last);

            row = row
                .key_context(CONTEXT)
                .track_focus(&focus)
                .on_action(move |_: &Previous, window, cx| previous(window, cx))
                .on_action(move |_: &Next, window, cx| next(window, cx))
                .on_action(move |_: &First, window, cx| first(window, cx))
                .on_action(move |_: &Last, window, cx| last(window, cx));
        }

        row.style().refine(&self.style);
        row
    }
}

/// The pointer and the keys on a row of tabs, in a window GPUI keeps in the
/// test's own process.
#[cfg(test)]
mod pointer_tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        Context, IntoElement, Modifiers, ParentElement as _, Render, TestAppContext,
        VisualTestContext, Window, div,
    };

    use super::{Tab, Tabs};

    /// Three tabs at the window's corner, the middle one disabled, and
    /// the one chosen kept here.
    struct Panel {
        chosen: Rc<Cell<usize>>,
    }

    impl Render for Panel {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let chosen = self.chosen.clone();

            div().child(
                Tabs::new("tabs")
                    .child(Tab::new("Conversation"))
                    .child(Tab::new("Usage").disabled(true))
                    .child(Tab::new("Files"))
                    .selected(self.chosen.get())
                    .on_change(cx.listener(move |_, to: &usize, _, cx| {
                        chosen.set(*to);
                        cx.notify();
                    })),
            )
        }
    }

    fn panel(cx: &mut TestAppContext) -> (Rc<Cell<usize>>, &mut VisualTestContext) {
        cx.update(crate::theme::init);
        cx.update(super::bind_keys);

        let chosen = Rc::new(Cell::new(0));
        let kept = chosen.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Panel { chosen: kept });

        (chosen, cx)
    }

    #[gpui::test]
    fn the_keys_choose_the_tab_beside_past_one_disabled(cx: &mut TestAppContext) {
        let (chosen, cx) = panel(cx);

        cx.update(|window, cx| window.focus_next(cx));
        cx.simulate_keystrokes("right");

        assert_eq!(chosen.get(), 2);

        cx.simulate_keystrokes("right");

        assert_eq!(chosen.get(), 0);

        cx.simulate_keystrokes("end");

        assert_eq!(chosen.get(), 2);
    }

    #[gpui::test]
    fn a_press_chooses_a_tab_but_not_one_disabled(cx: &mut TestAppContext) {
        let (chosen, cx) = panel(cx);
        let none = Modifiers::none();
        let middle = |cx: &mut VisualTestContext, tab: &'static str| {
            cx.debug_bounds(tab)
                .unwrap_or_else(|| panic!("{tab} was not drawn"))
                .center()
        };

        let disabled = middle(cx, "tab-1");
        cx.simulate_click(disabled, none);

        assert_eq!(chosen.get(), 0);

        let files = middle(cx, "tab-2");
        cx.simulate_click(files, none);

        assert_eq!(chosen.get(), 2);

        // The press gave the row the keyboard.
        cx.simulate_keystrokes("left");

        assert_eq!(chosen.get(), 0);
    }
}
