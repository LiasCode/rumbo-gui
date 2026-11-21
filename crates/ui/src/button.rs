//! A button: words, an icon, or both, in one of a few looks by how much it
//! matters. It takes the keyboard with Tab, and Enter or Space presses it.
//! Pressed, with the pointer or the keys, its face gives a little, and it
//! springs back once let go, a little past where it rests: the button's
//! box stays as it is, so nothing around it moves.

use std::rc::Rc;

use gpui::{
    AnimationExt as _, App, ClickEvent, ElementId, Entity, FocusHandle, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, Keystroke, MouseButton, ParentElement as _,
    Refineable as _, Rems, RenderOnce, SharedString, SpringAnimation, SpringConfig,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, transparent_black,
};

use crate::child_id;
use crate::icon::{Icon, IconName};
use crate::size::{Sizable, Size, rems_from_px};
use crate::theme::{ActiveTheme as _, ThemeColors};
use crate::tooltip::Tooltip;

pub(crate) type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// How the face moves in and out: quick, and loose enough to go a little
/// past where it rests before it settles.
const PRESS_SPRING: SpringConfig = SpringConfig::new(700., 16., 1.);

/// How far a press takes the face in on each side, as a share of the
/// button's height.
const PRESS_DEPTH: f32 = 0.08;

/// The group the face follows the pointer over the button by.
const GROUP: &str = "ui-button";

/// How much a button stands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// The one thing a view is for, such as sending.
    Primary,
    #[default]
    Secondary,
    /// One of several equal choices.
    Outline,
    /// An action that should not draw the eye, as in a toolbar.
    Ghost,
    /// What cannot be undone, such as deleting.
    Danger,
    /// Words that act, among other words.
    Link,
}

/// What a variant paints: its background at rest, under the pointer, and
/// pressed; its text; and its border.
struct Paint {
    background: Option<Hsla>,
    hover: Option<Hsla>,
    active: Option<Hsla>,
    text: Hsla,
    border: Option<Hsla>,
}

impl ButtonVariant {
    fn paint(self, colors: &ThemeColors, selected: bool) -> Paint {
        let paint = match self {
            ButtonVariant::Primary => Paint {
                background: Some(colors.primary),
                hover: Some(colors.primary_hover),
                active: Some(colors.primary_active),
                text: colors.primary_foreground,
                border: None,
            },
            ButtonVariant::Secondary => Paint {
                background: Some(colors.secondary),
                hover: Some(colors.secondary_hover),
                active: Some(colors.secondary_active),
                text: colors.secondary_foreground,
                border: None,
            },
            ButtonVariant::Outline => Paint {
                background: Some(colors.background),
                hover: Some(colors.accent),
                active: Some(colors.secondary_hover),
                text: colors.foreground,
                border: Some(colors.input),
            },
            ButtonVariant::Ghost => Paint {
                background: None,
                hover: Some(colors.accent),
                active: Some(colors.secondary_hover),
                text: colors.foreground,
                border: None,
            },
            ButtonVariant::Danger => Paint {
                background: Some(colors.danger),
                hover: Some(colors.danger_hover),
                active: Some(colors.danger_active),
                text: colors.danger_foreground,
                border: None,
            },
            ButtonVariant::Link => Paint {
                background: None,
                hover: None,
                active: None,
                text: colors.link,
                border: None,
            },
        };

        if !selected {
            return paint;
        }

        // A selected button looks pressed, and stays so under the pointer.
        Paint {
            background: paint.active.or(Some(colors.accent)),
            hover: paint.active.or(Some(colors.accent)),
            ..paint
        }
    }
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<IconName>,
    icon_end: bool,
    variant: ButtonVariant,
    size: Size,
    disabled: bool,
    selected: bool,
    tooltip: Option<SharedString>,
    shortcut: Option<SharedString>,
    on_click: Option<ClickHandler>,
    focus: Option<FocusHandle>,
    style: StyleRefinement,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Button {
            id: id.into(),
            label: None,
            icon: None,
            icon_end: false,
            variant: ButtonVariant::default(),
            size: Size::Medium,
            disabled: false,
            selected: false,
            tooltip: None,
            shortcut: None,
            on_click: None,
            focus: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// An icon before the label, or alone: a button with an icon and no
    /// label is square.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The icon after the label rather than before it, as a button that goes
    /// forward has it.
    pub fn icon_end(mut self) -> Self {
        self.icon_end = true;
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    pub fn outline(self) -> Self {
        self.variant(ButtonVariant::Outline)
    }

    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    pub fn danger(self) -> Self {
        self.variant(ButtonVariant::Danger)
    }

    pub fn link(self) -> Self {
        self.variant(ButtonVariant::Link)
    }

    /// A button that cannot be pressed now, and says so by fading.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// A button that stays pressed, such as the choice made in a group or
    /// the trigger of what is open.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// What it does, shown when the pointer rests on it.
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// The keys that do the same, said in its tooltip.
    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// The keyboard's handle it takes, for its view to give it the
    /// keyboard, as a screen does its main action.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }
}

impl Sizable for Button {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let pressed_state = window.use_keyed_state(child_id(&self.id, "pressed"), cx, |_, _| false);
        let pressed = !self.disabled && *pressed_state.read(cx);

        let theme = cx.theme();
        let paint = self.variant.paint(&theme.colors, self.selected);
        let square = self.label.is_none() && self.icon.is_some();
        let link = self.variant == ButtonVariant::Link;
        let ring = theme.focus_ring();
        let size = self.size;
        let height = size.height();

        // The face is what gives under a press, so it takes the corners and
        // the shadow it was given; the box keeps the rest.
        let mut style = self.style;
        let face_style = StyleRefinement {
            corner_radii: style.corner_radii.clone(),
            box_shadow: style.box_shadow.take(),
            ..StyleRefinement::default()
        };

        let background = match pressed {
            true => paint.active.or(paint.background),
            false => paint.background,
        };

        let mut face = div()
            .absolute()
            .rounded(theme.radius)
            .when_some(background, |face, background| face.bg(background))
            .when_some(paint.border, |face, border| {
                face.border_1().border_color(border)
            })
            .when(!self.disabled && !pressed, |face| {
                face.when_some(paint.hover, |face, hover| {
                    face.group_hover(GROUP, move |style| style.bg(hover))
                })
            });

        face.style().refine(&face_style);

        // A spring keeps its speed when its target changes, so a press let
        // go of before it is all the way in still springs back.
        let face = face.with_spring(
            "press",
            SpringAnimation::new(PRESS_SPRING).to(if pressed { 1. } else { 0. }),
            move |face, depth| {
                let inset = Rems(height.0 * PRESS_DEPTH * depth);

                face.top(inset).bottom(inset).left(inset).right(inset)
            },
        );

        let mut button = div()
            .id(self.id)
            .group(GROUP)
            .relative()
            .flex()
            .flex_none()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(match size {
                Size::XSmall => rems_from_px(4.),
                _ => rems_from_px(8.),
            })
            .h(height)
            .rounded(theme.radius)
            .text_size(size.text())
            .font_weight(FontWeight::MEDIUM)
            .text_color(paint.text)
            .whitespace_nowrap()
            .map(|button| match square {
                true => button.w(height),
                false => button.px(size.padding_x()),
            })
            // The face draws the border; the box keeps its room.
            .when(paint.border.is_some(), |button| {
                button.border_1().border_color(transparent_black())
            })
            .child(face)
            .when_some(self.icon.filter(|_| !self.icon_end), |button, icon| {
                button.child(Icon::new(icon).with_size(size).color(paint.text))
            })
            .when_some(self.label, |button, label| button.child(label))
            .when_some(self.icon.filter(|_| self.icon_end), |button, icon| {
                button.child(Icon::new(icon).with_size(size).color(paint.text))
            });

        if self.disabled {
            button = button.opacity(0.5).cursor_default();
        } else {
            let press = |down: bool| {
                let state = pressed_state.clone();

                move |cx: &mut App| set_pressed(&state, down, cx)
            };
            let (pointer_down, pointer_up, pointer_up_out) =
                (press(true), press(false), press(false));
            let (key_down, key_up) = (press(true), press(false));

            button = button
                .cursor_pointer()
                .map(|button| match self.focus {
                    Some(focus) => button.track_focus(&focus.tab_stop(true)),
                    None => button.tab_index(0),
                })
                .focus_visible(move |style| style.shadow(ring))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| pointer_down(cx))
                .on_mouse_up(MouseButton::Left, move |_, _, cx| pointer_up(cx))
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| pointer_up_out(cx))
                .on_key_down(move |event, _, cx| {
                    if presses(&event.keystroke) {
                        key_down(cx);
                    }
                })
                .on_key_up(move |event, _, cx| {
                    if presses(&event.keystroke) {
                        key_up(cx);
                    }
                })
                .when(link, |button| {
                    let hover = theme.colors.link_hover;

                    button.hover(move |style| style.underline().text_color(hover))
                })
                .when_some(self.on_click, |button, handler| {
                    button.on_click(move |event, window, cx| handler(event, window, cx))
                });
        }

        if let Some(text) = self.tooltip {
            let shortcut = self.shortcut;

            button = button.tooltip(move |_, cx| {
                let tooltip = Tooltip::new(text.clone());

                match &shortcut {
                    Some(keys) => tooltip.keys(keys.clone()).build(cx),
                    None => tooltip.build(cx),
                }
            });
        }

        button.style().refine(&style);
        button
    }
}

/// Whether a key presses a button, as GPUI takes it: Enter or Space alone.
fn presses(keystroke: &Keystroke) -> bool {
    (keystroke.key == "enter" || keystroke.key == "space") && !keystroke.modifiers.modified()
}

/// Says whether a button is held down, drawing it again when that changes.
fn set_pressed(state: &Entity<bool>, pressed: bool, cx: &mut App) {
    state.update(cx, |held, cx| {
        if *held != pressed {
            *held = pressed;
            cx.notify();
        }
    });
}

/// The pointer and the keys on a button, in a window GPUI keeps in the
/// test's own process.
#[cfg(test)]
mod pointer_tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        Context, FocusHandle, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
        MouseButton, MouseDownEvent, MouseUpEvent, ParentElement as _, Render, Styled as _,
        TestAppContext, VisualTestContext, Window, div, point, px,
    };

    use super::Button;

    /// A button at the window's corner, which counts its presses.
    struct Counter {
        presses: Rc<Cell<usize>>,
        focus: FocusHandle,
    }

    impl Render for Counter {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let presses = self.presses.clone();

            div().size_full().child(
                Button::new("send")
                    .label("Send")
                    .track_focus(&self.focus)
                    .on_click(move |_, _, _| presses.set(presses.get() + 1)),
            )
        }
    }

    /// Its presses, and its window with the button's keyboard handle.
    fn counter(cx: &mut TestAppContext) -> (Rc<Cell<usize>>, FocusHandle, &mut VisualTestContext) {
        cx.update(crate::theme::init);

        let presses = Rc::new(Cell::new(0));
        let counted = presses.clone();
        let (view, cx) = cx.add_window_view(move |_, cx| Counter {
            presses: counted,
            focus: cx.focus_handle(),
        });
        let focus = view.read_with(cx, |counter, _| counter.focus.clone());

        (presses, focus, cx)
    }

    /// A key pressed and let go, as GPUI presses a control with Enter or
    /// Space only when it comes up.
    fn key(cx: &mut VisualTestContext, key: &'static str) {
        let keystroke = Keystroke::parse(key).unwrap();

        cx.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke });
    }

    #[gpui::test]
    fn presses_once_with_the_pointer_enter_or_space(cx: &mut TestAppContext) {
        let (presses, focus, cx) = counter(cx);
        let on_it = point(px(20.), px(18.));

        cx.simulate_click(on_it, Modifiers::none());
        assert_eq!(presses.get(), 1);

        cx.update(|window, cx| focus.focus(window, cx));
        key(cx, "enter");
        key(cx, "space");
        assert_eq!(presses.get(), 3);
    }

    #[gpui::test]
    fn a_press_let_go_elsewhere_presses_nothing_and_lets_go(cx: &mut TestAppContext) {
        let (presses, _, cx) = counter(cx);
        let on_it = point(px(20.), px(18.));
        let none = Modifiers::none();

        cx.simulate_event(MouseDownEvent {
            position: on_it,
            modifiers: none,
            button: MouseButton::Left,
            click_count: 1,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position: point(px(300.), px(200.)),
            modifiers: none,
            button: MouseButton::Left,
            click_count: 1,
        });
        assert_eq!(presses.get(), 0);

        // It is not left held: the next press is one.
        cx.simulate_click(on_it, none);
        assert_eq!(presses.get(), 1);
    }
}
