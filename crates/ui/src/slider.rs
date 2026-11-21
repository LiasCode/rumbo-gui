//! A value between two ends, in steps, chosen by dragging a thumb along a
//! track or pressing the track where it should be. The keyboard moves it a
//! step with the arrows, ten with Page Up and Page Down, and to its ends
//! with Home and End.

use std::rc::Rc;

use gpui::{
    App, CursorStyle, ElementId, InteractiveElement as _, IntoElement, KeyBinding,
    ParentElement as _, Pixels, Refineable as _, Rems, RenderOnce, StyleRefinement, Styled, Window,
    actions, div, prelude::FluentBuilder as _, relative,
};

use crate::drag;
use crate::focus;
use crate::size::rems_from_px;
use crate::theme::ActiveTheme as _;
use crate::{ChangeHandler, child_id};

actions!(
    slider,
    [
        Decrease,
        Increase,
        DecreaseMore,
        IncreaseMore,
        ToStart,
        ToEnd
    ]
);

/// A slider's key context.
const CONTEXT: &str = "Slider";

/// How wide the thumb is, and how thick the track.
const THUMB: Rems = rems_from_px(16.);
const TRACK: Rems = rems_from_px(6.);

/// How many steps Page Up and Page Down move it.
const PAGE: f32 = 10.;

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);

    cx.bind_keys([
        KeyBinding::new("left", Decrease, context),
        KeyBinding::new("down", Decrease, context),
        KeyBinding::new("right", Increase, context),
        KeyBinding::new("up", Increase, context),
        KeyBinding::new("pagedown", DecreaseMore, context),
        KeyBinding::new("pageup", IncreaseMore, context),
        KeyBinding::new("home", ToStart, context),
        KeyBinding::new("end", ToEnd, context),
    ]);
}

/// The values a slider takes: from `min` to `max`, on steps counted from
/// `min`, so an end that is not on a step is not taken: the last step
/// before it is. A step that is not above zero takes any value between
/// them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Scale {
    min: f32,
    max: f32,
    step: f32,
}

impl Scale {
    /// `value` on its nearest step, within the ends.
    fn snap(self, value: f32) -> f32 {
        let max = self.max.max(self.min);
        let value = value.clamp(self.min, max);

        if !(self.step > 0. && self.step.is_finite()) {
            return value;
        }

        // A little over, so a span that is a whole number of steps does not
        // come out one short of it.
        let last = ((max - self.min) / self.step + 1e-4).floor();
        let steps = ((value - self.min) / self.step).round().min(last);

        self.min + steps * self.step
    }

    /// How far along `value` is: 0 at `min` and 1 at `max`.
    fn fraction(self, value: f32) -> f32 {
        let span = self.max - self.min;

        match span > 0. {
            true => ((value - self.min) / span).clamp(0., 1.),
            false => 0.,
        }
    }

    /// The value `fraction` of the way along, on its step.
    fn at(self, fraction: f32) -> f32 {
        self.snap(self.min + (self.max - self.min) * fraction.clamp(0., 1.))
    }

    /// `value` moved `steps` steps, on its step and within the ends. A
    /// scale without steps moves a hundredth of the way for each.
    fn moved(self, value: f32, steps: f32) -> f32 {
        let step = match self.step > 0. && self.step.is_finite() {
            true => self.step,
            false => (self.max - self.min) / 100.,
        };

        self.snap(self.snap(value) + step * steps)
    }
}

/// How far along a track the pointer at `at` puts the thumb, for a track
/// `length` long that starts at 0: the thumb's middle goes no closer to an
/// end than half the `thumb`.
fn fraction_at(at: Pixels, length: Pixels, thumb: Pixels) -> f32 {
    let travel = length - thumb;

    match travel > Pixels::ZERO {
        true => ((at - thumb / 2.) / travel).clamp(0., 1.),
        false => 0.,
    }
}

#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    scale: Scale,
    value: f32,
    disabled: bool,
    on_change: Option<ChangeHandler<f32>>,
    style: StyleRefinement,
}

impl Slider {
    /// From 0 to 100, a whole number at a time.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Slider {
            id: id.into(),
            scale: Scale {
                min: 0.,
                max: 100.,
                step: 1.,
            },
            value: 0.,
            disabled: false,
            on_change: None,
            style: StyleRefinement::default(),
        }
    }

    /// Its ends.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.scale.min = min;
        self.scale.max = max;
        self
    }

    /// How far apart the values it takes are, from `min`; zero for any.
    pub fn step(mut self, step: f32) -> Self {
        self.scale.step = step;
        self
    }

    pub fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the value it is moved to, on its step; the caller keeps
    /// it and gives it back with `value`.
    pub fn on_change(mut self, handler: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for Slider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = focus::focus_of(&self.id, window, cx);
        let ringed = focus::ringed(&focus, window);
        let hold = window.use_keyed_state(child_id(&self.id, "hold"), cx, |_, _| None::<()>);
        let held = hold.read(cx).is_some();

        let theme = cx.theme();
        let colors = theme.colors;
        let ring = theme.focus_ring();
        let scale = self.scale;
        let value = scale.snap(self.value);
        let fraction = scale.fraction(value);
        let side = THUMB.to_pixels(window.rem_size());

        // The track, and what is filled of it up to the thumb's middle.
        let inset = (side - TRACK.to_pixels(window.rem_size())) / 2.;
        let track = div()
            .absolute()
            .left_0()
            .right_0()
            .top(inset)
            .h(TRACK)
            .rounded_full()
            .bg(colors.secondary);

        let fill = div()
            .absolute()
            .top(inset)
            .h(TRACK)
            .left(-side / 2.)
            .right(relative(1. - fraction))
            .rounded_l_full()
            .bg(colors.primary);

        let thumb = div()
            .absolute()
            .top_0()
            .left(relative(fraction))
            .ml(-side / 2.)
            .size(side)
            .rounded_full()
            .border_1()
            .border_color(colors.primary)
            .bg(colors.background)
            .shadow_sm()
            .when(!self.disabled, |thumb| {
                let ring = ring.clone();

                thumb.hover(move |style| style.shadow(ring.clone()))
            })
            .when(held || ringed, |thumb| thumb.shadow(ring));

        // The thumb's middle moves between half a thumb from each end.
        let travel = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(side / 2.)
            .right(side / 2.)
            .child(fill)
            .child(thumb);

        let mut slider = div()
            .id(self.id)
            .relative()
            .flex_none()
            .w_full()
            .h(side)
            .child(track)
            .child(travel);

        match (self.disabled, self.on_change) {
            (true, _) | (_, None) => {
                slider = slider.when(self.disabled, |slider| slider.opacity(0.5));
            }

            (false, Some(on_change)) => {
                let change = move |to: f32, window: &mut Window, cx: &mut App| {
                    if to != value {
                        on_change(&to, window, cx);
                    }
                };
                let change = Rc::new(change);
                let keys = |steps: f32| {
                    let change = change.clone();

                    move |window: &mut Window, cx: &mut App| {
                        change(scale.moved(value, steps), window, cx)
                    }
                };

                let decrease = keys(-1.);
                let increase = keys(1.);
                let decrease_more = keys(-PAGE);
                let increase_more = keys(PAGE);
                let to_start = change.clone();
                let to_end = change.clone();
                let pressed = change.clone();
                let dragged = change.clone();
                let taken = focus.clone();

                slider = slider
                    .key_context(CONTEXT)
                    .track_focus(&focus)
                    .on_action(move |_: &Decrease, window, cx| decrease(window, cx))
                    .on_action(move |_: &Increase, window, cx| increase(window, cx))
                    .on_action(move |_: &DecreaseMore, window, cx| decrease_more(window, cx))
                    .on_action(move |_: &IncreaseMore, window, cx| increase_more(window, cx))
                    .on_action(move |_: &ToStart, window, cx| to_start(scale.at(0.), window, cx))
                    .on_action(move |_: &ToEnd, window, cx| to_end(scale.at(1.), window, cx))
                    .child(drag::area(
                        hold,
                        CursorStyle::PointingHand,
                        move |at, bounds, window, cx| {
                            let along = fraction_at(at.x - bounds.left(), bounds.size.width, side);

                            window.focus(&taken, cx);
                            pressed(scale.at(along), window, cx);
                            Some(())
                        },
                        move |_, at, bounds, window, cx| {
                            let along = fraction_at(at.x - bounds.left(), bounds.size.width, side);

                            dragged(scale.at(along), window, cx);
                        },
                    ));
            }
        }

        slider.style().refine(&self.style);
        slider
    }
}

#[cfg(test)]
mod tests {
    use gpui::px;

    use super::{Scale, fraction_at};

    fn scale(min: f32, max: f32, step: f32) -> Scale {
        Scale { min, max, step }
    }

    #[test]
    fn puts_a_value_on_its_step_within_the_ends() {
        let percent = scale(0., 100., 5.);

        assert_eq!(percent.snap(42.), 40.);
        assert_eq!(percent.snap(43.), 45.);
        assert_eq!(percent.snap(-10.), 0.);
        assert_eq!(percent.snap(180.), 100.);

        // Steps are counted from the start, not from zero.
        assert_eq!(scale(1., 10., 2.).snap(4.2), 5.);

        // An end off the steps is not taken, but the last step before it.
        assert_eq!(scale(0., 10., 3.).snap(10.), 9.);
        assert_eq!(scale(0., 10., 3.).snap(25.), 9.);
        assert_eq!(scale(0., 1., 0.1).at(1.), 1.);

        // Without steps, any value between the ends.
        assert_eq!(scale(0., 1., 0.).snap(0.123), 0.123);
    }

    #[test]
    fn says_how_far_along_a_value_is() {
        assert_eq!(scale(0., 200., 1.).fraction(50.), 0.25);
        assert_eq!(scale(-1., 1., 0.).fraction(0.), 0.5);
        assert_eq!(scale(0., 10., 1.).fraction(20.), 1.);

        // Ends that do not open hold every value at the start.
        assert_eq!(scale(5., 5., 1.).fraction(5.), 0.);
        assert_eq!(scale(5., 1., 1.).at(0.5), 5.);
    }

    #[test]
    fn takes_the_value_of_a_place_on_its_step() {
        let percent = scale(0., 100., 10.);

        assert_eq!(percent.at(0.), 0.);
        assert_eq!(percent.at(0.44), 40.);
        assert_eq!(percent.at(0.46), 50.);
        assert_eq!(percent.at(1.5), 100.);
    }

    #[test]
    fn moves_by_steps_and_stops_at_the_ends() {
        let percent = scale(0., 100., 5.);

        assert_eq!(percent.moved(40., 1.), 45.);
        assert_eq!(percent.moved(40., -10.), 0.);
        assert_eq!(percent.moved(98., 1.), 100.);

        // A value off its step moves from the step it is nearest.
        assert_eq!(percent.moved(41., 1.), 45.);

        // Without steps, a hundredth of the way.
        assert_eq!(scale(0., 200., 0.).moved(100., 1.), 102.);
    }

    #[test]
    fn keeps_the_thumb_inside_the_track() {
        let length = px(216.);
        let thumb = px(16.);

        assert_eq!(fraction_at(thumb / 2., length, thumb), 0.);
        assert_eq!(fraction_at(px(0.), length, thumb), 0.);
        assert_eq!(fraction_at(px(108.), length, thumb), 0.5);
        assert_eq!(fraction_at(length - thumb / 2., length, thumb), 1.);
        assert_eq!(fraction_at(px(500.), length, thumb), 1.);
        assert_eq!(fraction_at(px(5.), thumb, thumb), 0.);
    }
}

/// The pointer on a slider, in a window GPUI keeps in the test's own
/// process.
#[cfg(test)]
mod pointer_tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        Context, IntoElement, Modifiers, MouseButton, ParentElement as _, Render, Styled as _,
        TestAppContext, VisualTestContext, Window, div, point, px,
    };

    use super::Slider;

    /// A slider 216 wide at the window's corner, from 0 to 100 by 10: its
    /// thumb's middle travels 200, from 8 to 208, and its value is kept here.
    struct Form {
        value: Rc<Cell<f32>>,
    }

    impl Render for Form {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let value = self.value.clone();

            div().size_full().child(
                Slider::new("slider")
                    .step(10.)
                    .w(px(216.))
                    .value(self.value.get())
                    .on_change(cx.listener(move |_, to: &f32, _, cx| {
                        value.set(*to);
                        cx.notify();
                    })),
            )
        }
    }

    fn form(cx: &mut TestAppContext) -> (Rc<Cell<f32>>, &mut VisualTestContext) {
        cx.update(crate::theme::init);
        cx.update(super::bind_keys);

        let value = Rc::new(Cell::new(0.));
        let kept = value.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Form { value: kept });

        (value, cx)
    }

    #[gpui::test]
    fn a_press_puts_the_thumb_there_and_a_drag_takes_it_along(cx: &mut TestAppContext) {
        let (value, cx) = form(cx);
        let none = Modifiers::none();

        cx.simulate_mouse_move(point(px(108.), px(8.)), None, none);
        cx.simulate_mouse_down(point(px(108.), px(8.)), MouseButton::Left, none);

        assert_eq!(value.get(), 50.);

        // Off the slider and past its end, it stays at the end.
        cx.simulate_mouse_move(point(px(400.), px(300.)), MouseButton::Left, none);

        assert_eq!(value.get(), 100.);

        cx.simulate_mouse_move(point(px(66.), px(300.)), MouseButton::Left, none);

        assert_eq!(value.get(), 30.);

        // Let go, the pointer moves nothing.
        cx.simulate_mouse_up(point(px(66.), px(300.)), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(200.), px(8.)), None, none);

        assert_eq!(value.get(), 30.);
    }

    #[gpui::test]
    fn the_keys_move_it_once_pressed(cx: &mut TestAppContext) {
        let (value, cx) = form(cx);
        let none = Modifiers::none();

        cx.simulate_mouse_move(point(px(108.), px(8.)), None, none);
        cx.simulate_mouse_down(point(px(108.), px(8.)), MouseButton::Left, none);
        cx.simulate_mouse_up(point(px(108.), px(8.)), MouseButton::Left, none);

        cx.simulate_keystrokes("right right");
        assert_eq!(value.get(), 70.);

        cx.simulate_keystrokes("pagedown");
        assert_eq!(value.get(), 0.);

        cx.simulate_keystrokes("end left");
        assert_eq!(value.get(), 90.);
    }
}
