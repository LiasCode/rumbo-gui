//! A choice of one option among a few, all in sight, each with its label
//! and words about it beside a round mark. Pressing an option or its words
//! chooses it. The group is one stop for Tab, on the option chosen; the
//! arrows choose the option beside, coming round at the ends, past those
//! disabled, and Space chooses the first while none is.

use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _,
    Refineable as _, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, Window, actions, div, prelude::FluentBuilder as _,
};

use crate::check_row::{self, Words};
use crate::focus;
use crate::root::h_flex;
use crate::roving::{self, Step};
use crate::size::{Sizable, Size, rems_from_px};
use crate::theme::ActiveTheme as _;
use crate::{ChangeHandler, child_id};

actions!(radio, [Previous, Next, Choose]);

/// A group of radios' key context.
const CONTEXT: &str = "RadioGroup";

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);

    cx.bind_keys([
        KeyBinding::new("up", Previous, context),
        KeyBinding::new("left", Previous, context),
        KeyBinding::new("down", Next, context),
        KeyBinding::new("right", Next, context),
        KeyBinding::new("space", Choose, context),
    ]);
}

/// One option of a group.
pub struct Radio {
    words: Words,
    disabled: bool,
}

impl Radio {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Radio {
            words: Words {
                label: Some(label.into()),
                description: None,
            },
            disabled: false,
        }
    }

    /// What the option means, under its label.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.words.description = Some(description.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(IntoElement)]
pub struct RadioGroup {
    id: ElementId,
    radios: Vec<Radio>,
    selected: Option<usize>,
    horizontal: bool,
    disabled: bool,
    size: Size,
    on_change: Option<ChangeHandler<usize>>,
    style: StyleRefinement,
}

impl RadioGroup {
    pub fn new(id: impl Into<ElementId>) -> Self {
        RadioGroup {
            id: id.into(),
            radios: Vec::new(),
            selected: None,
            horizontal: false,
            disabled: false,
            size: Size::Medium,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn child(mut self, radio: Radio) -> Self {
        self.radios.push(radio);
        self
    }

    pub fn children(mut self, radios: impl IntoIterator<Item = Radio>) -> Self {
        self.radios.extend(radios);
        self
    }

    /// The option chosen, by its place, or none yet.
    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// The options side by side, rather than one under another.
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the place of the option chosen; the caller keeps it and
    /// gives it back with `selected`.
    pub fn on_change(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Sizable for RadioGroup {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for RadioGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = focus::focus_of(&self.id, window, cx);
        let ringed = focus::ringed(&focus, window);
        let theme = cx.theme();
        let colors = theme.colors;
        let ring = theme.focus_ring();
        let size = self.size;
        let selected = self.selected;

        let enabled: Rc<[bool]> = self
            .radios
            .iter()
            .map(|radio| !radio.disabled && !self.disabled)
            .collect();

        // The option the keyboard is on: the one chosen, or the first that
        // can be.
        let current = selected.or_else(|| roving::moved(None, &enabled, Step::First));

        let (side, dot) = match size {
            Size::XSmall | Size::Small => (rems_from_px(14.), rems_from_px(6.)),
            Size::Medium | Size::Large => (rems_from_px(16.), rems_from_px(8.)),
        };

        // Its middle lines up with the label's first line.
        let lead = (size.line_height() - side) / 2.;

        let mut radios = Vec::with_capacity(self.radios.len());

        for (index, radio) in self.radios.into_iter().enumerate() {
            let chosen = selected == Some(index);
            let open = enabled[index];

            let mark = div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .mt(lead)
                .size(side)
                .rounded_full()
                .border_1()
                .bg(colors.background)
                .map(|mark| match chosen {
                    true => mark
                        .border_color(colors.primary)
                        .child(div().size(dot).rounded_full().bg(colors.primary)),
                    false => mark.border_color(colors.input),
                })
                .when(!open, |mark| mark.opacity(0.5))
                .when(ringed && current == Some(index), |mark| {
                    mark.shadow(ring.clone())
                });

            let mut option = h_flex()
                .id(child_id(&self.id, format!("radio-{index}")))
                .debug_selector(|| format!("radio-{index}"))
                .items_start()
                .gap_2()
                .child(mark)
                .children(check_row::words(radio.words, !open, size, cx));

            if open {
                option = option.cursor_pointer();

                if let (Some(on_change), false) = (&self.on_change, chosen) {
                    let on_change = on_change.clone();
                    let focus = focus.clone();

                    option = option.on_click(move |_, window, cx| {
                        window.focus(&focus, cx);
                        on_change(&index, window, cx);
                    });
                }
            }

            radios.push(option);
        }

        let mut group = div()
            .id(self.id.clone())
            .flex()
            .map(|group| match self.horizontal {
                true => group.flex_row().flex_wrap().gap_x_6().gap_y_3(),
                false => group.flex_col().gap_3(),
            })
            .children(radios);

        let any_open = enabled.iter().any(|open| *open);

        if let (Some(on_change), true) = (self.on_change, any_open) {
            let keys = |step: Step| {
                let on_change = on_change.clone();
                let enabled = enabled.clone();

                move |window: &mut Window, cx: &mut App| {
                    if let Some(to) = roving::moved(selected, &enabled, step) {
                        on_change(&to, window, cx);
                    }
                }
            };

            let previous = keys(Step::Previous);
            let next = keys(Step::Next);
            let choose = keys(Step::First);

            group = group
                .key_context(CONTEXT)
                .track_focus(&focus)
                .on_action(move |_: &Previous, window, cx| previous(window, cx))
                .on_action(move |_: &Next, window, cx| next(window, cx))
                .on_action(move |_: &Choose, window, cx| {
                    if selected.is_none() {
                        choose(window, cx);
                    }
                });
        }

        group.style().refine(&self.style);
        group
    }
}

/// The pointer and the keys on a group of radios, in a window GPUI keeps in
/// the test's own process.
#[cfg(test)]
mod pointer_tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        Context, IntoElement, Modifiers, ParentElement as _, Render, TestAppContext,
        VisualTestContext, Window, div,
    };

    use super::{Radio, RadioGroup};

    /// Three providers, the second disabled, none chosen at first, and the
    /// one chosen kept here.
    struct Form {
        chosen: Rc<Cell<Option<usize>>>,
    }

    impl Render for Form {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let chosen = self.chosen.clone();

            div().child(
                RadioGroup::new("providers")
                    .child(Radio::new("anthropic"))
                    .child(Radio::new("openai").disabled(true))
                    .child(Radio::new("toy").description("Needs no model"))
                    .selected(self.chosen.get())
                    .on_change(cx.listener(move |_, to: &usize, _, cx| {
                        chosen.set(Some(*to));
                        cx.notify();
                    })),
            )
        }
    }

    fn form(cx: &mut TestAppContext) -> (Rc<Cell<Option<usize>>>, &mut VisualTestContext) {
        cx.update(crate::theme::init);
        cx.update(super::bind_keys);

        let chosen = Rc::new(Cell::new(None));
        let kept = chosen.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Form { chosen: kept });

        (chosen, cx)
    }

    #[gpui::test]
    fn space_chooses_the_first_and_the_arrows_pass_one_disabled(cx: &mut TestAppContext) {
        let (chosen, cx) = form(cx);

        cx.update(|window, cx| window.focus_next(cx));
        cx.simulate_keystrokes("space");

        assert_eq!(chosen.get(), Some(0));

        cx.simulate_keystrokes("down");

        assert_eq!(chosen.get(), Some(2));

        // Coming round at the ends.
        cx.simulate_keystrokes("right");

        assert_eq!(chosen.get(), Some(0));

        cx.simulate_keystrokes("up");

        assert_eq!(chosen.get(), Some(2));
    }

    #[gpui::test]
    fn a_press_on_an_option_chooses_it(cx: &mut TestAppContext) {
        let (chosen, cx) = form(cx);
        let none = Modifiers::none();
        let middle = |cx: &mut VisualTestContext, radio: &'static str| {
            cx.debug_bounds(radio)
                .unwrap_or_else(|| panic!("{radio} was not drawn"))
                .center()
        };

        let disabled = middle(cx, "radio-1");
        cx.simulate_click(disabled, none);

        assert_eq!(chosen.get(), None);

        let toy = middle(cx, "radio-2");
        cx.simulate_click(toy, none);

        assert_eq!(chosen.get(), Some(2));

        cx.simulate_keystrokes("space");

        // Space chooses only while none is chosen.
        assert_eq!(chosen.get(), Some(2));
    }
}
