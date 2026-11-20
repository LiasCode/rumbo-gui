//! A scrollbar along the edge of what scrolls: where the view is in it, and
//! how much of it shows. The thumb is dragged with the pointer; a press on
//! the track takes the thumb there and goes on dragging it; the wheel goes
//! through it to what scrolls. It shows only while something is out of
//! view, and widens under the pointer.
//!
//! It goes in an element that does not scroll, over the one that does: a
//! `relative` parent holding both, or a field's frame.

use std::rc::Rc;

use gpui::{
    Along as _, App, Axis, Bounds, CursorStyle, DispatchPhase, Element, ElementId, Entity,
    GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId, ListState,
    MouseMoveEvent, Pixels, Point, RenderOnce, ScrollHandle, Size, Style, UniformListScrollHandle,
    Window, fill, point, px, size,
};

use crate::child_id;
use crate::drag::{self, Hold};
use crate::text::InputState;
use crate::theme::ActiveTheme as _;

/// How wide the strip the pointer takes the scrollbar in is.
const STRIP: Pixels = px(12.);

/// How thick the thumb is at rest, and under the pointer or dragged.
const THIN: Pixels = px(6.);
const THICK: Pixels = px(8.);

/// How far the thumb stands from the edge.
const MARGIN: Pixels = px(2.);

/// The shortest a thumb gets, so a long list still has one to take.
const SHORTEST: Pixels = px(24.);

/// Where a scrolled view is along an axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measure {
    /// How far it is scrolled from the start.
    pub scrolled: Pixels,
    /// How far it can be scrolled.
    pub most: Pixels,
    /// How much of it shows.
    pub shown: Pixels,
}

/// What a scrollbar shows and moves.
pub trait ScrollSource: 'static {
    /// Where it is along `axis`; none until it is laid out.
    fn measure(&self, axis: Axis, cx: &App) -> Option<Measure>;

    /// Scrolls it to `distance` from its start along `axis`.
    fn scroll_to(&self, axis: Axis, distance: Pixels, window: &mut Window, cx: &mut App);
}

/// What a `div` with `track_scroll` scrolls. Its offset grows negative as
/// it scrolls.
impl ScrollSource for ScrollHandle {
    fn measure(&self, axis: Axis, _: &App) -> Option<Measure> {
        let shown = self.bounds().size.along(axis);

        (shown > Pixels::ZERO).then(|| Measure {
            // From zero, so that at the start it is 0 and not -0.
            scrolled: Pixels::ZERO - self.offset().along(axis),
            most: self.max_offset().along(axis),
            shown,
        })
    }

    fn scroll_to(&self, axis: Axis, distance: Pixels, window: &mut Window, _: &mut App) {
        self.set_offset(self.offset().apply_along(axis, |_| -distance));
        window.refresh();
    }
}

/// What a `uniform_list` with `track_scroll` scrolls, which is a
/// `ScrollHandle` it keeps.
impl ScrollSource for UniformListScrollHandle {
    fn measure(&self, axis: Axis, cx: &App) -> Option<Measure> {
        self.0.borrow().base_handle.measure(axis, cx)
    }

    fn scroll_to(&self, axis: Axis, distance: Pixels, window: &mut Window, cx: &mut App) {
        let handle = self.0.borrow().base_handle.clone();

        handle.scroll_to(axis, distance, window, cx);
    }
}

/// What a `list` of rows of their own heights scrolls. It reaches only as
/// far as it has measured, which grows as rows come into view.
impl ScrollSource for ListState {
    fn measure(&self, axis: Axis, _: &App) -> Option<Measure> {
        if axis != Axis::Vertical {
            return None;
        }

        let shown = self.viewport_bounds().size.height;

        (shown > Pixels::ZERO).then(|| Measure {
            scrolled: Pixels::ZERO - self.scroll_px_offset_for_scrollbar().y,
            most: self.max_offset_for_scrollbar().y,
            shown,
        })
    }

    fn scroll_to(&self, axis: Axis, distance: Pixels, window: &mut Window, _: &mut App) {
        if axis == Axis::Vertical {
            self.set_offset_from_scrollbar(point(Pixels::ZERO, -distance));
            window.refresh();
        }
    }
}

/// A field of several lines, which scrolls down.
impl ScrollSource for Entity<InputState> {
    fn measure(&self, axis: Axis, cx: &App) -> Option<Measure> {
        match axis {
            Axis::Vertical => self.read(cx).vertical_scroll(),
            Axis::Horizontal => None,
        }
    }

    fn scroll_to(&self, axis: Axis, distance: Pixels, _: &mut Window, cx: &mut App) {
        if axis == Axis::Vertical {
            self.update(cx, |state, cx| state.scroll_down_to(distance, cx));
        }
    }
}

/// The thumb on a track `length` long: where it starts, and how long it is.
/// None while nothing is out of view.
fn thumb(length: Pixels, measure: &Measure) -> Option<(Pixels, Pixels)> {
    if measure.most < px(1.) || measure.shown <= Pixels::ZERO || length <= Pixels::ZERO {
        return None;
    }

    let share = measure.shown / (measure.shown + measure.most);
    let long = (length * share).max(SHORTEST.min(length)).min(length);
    let travel = length - long;
    let start = travel * (measure.scrolled / measure.most).clamp(0., 1.);

    Some((start, long))
}

/// How far to scroll for a thumb `long` on a track `length` long to start
/// at `start`.
fn distance_at(start: Pixels, length: Pixels, long: Pixels, most: Pixels) -> Pixels {
    let travel = length - long;

    if travel <= Pixels::ZERO {
        return Pixels::ZERO;
    }

    most * (start / travel).clamp(0., 1.)
}

/// Whether the pointer is on a scrollbar, which it remembers between
/// frames.
#[derive(Default)]
struct Bar {
    hovered: bool,
}

#[derive(IntoElement)]
pub struct Scrollbar {
    id: ElementId,
    axis: Axis,
    source: Rc<dyn ScrollSource>,
}

impl Scrollbar {
    /// Along the right edge.
    pub fn vertical(id: impl Into<ElementId>, source: impl ScrollSource) -> Self {
        Scrollbar {
            id: id.into(),
            axis: Axis::Vertical,
            source: Rc::new(source),
        }
    }

    /// Along the bottom edge.
    pub fn horizontal(id: impl Into<ElementId>, source: impl ScrollSource) -> Self {
        Scrollbar {
            id: id.into(),
            axis: Axis::Horizontal,
            source: Rc::new(source),
        }
    }
}

impl RenderOnce for Scrollbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let bar = window.use_keyed_state(self.id.clone(), cx, |_, _| Bar::default());

        // Where the pointer took the thumb, from its start, while it drags it.
        let grab = window.use_keyed_state(child_id(&self.id, "grab"), cx, |_, _| None);

        ScrollbarElement {
            axis: self.axis,
            source: self.source,
            bar,
            grab,
        }
    }
}

struct ScrollbarElement {
    axis: Axis,
    source: Rc<dyn ScrollSource>,
    bar: Entity<Bar>,
    grab: Hold<Pixels>,
}

/// Where the parts of a scrollbar are this frame.
struct Placed {
    track: Bounds<Pixels>,
    hitbox: Hitbox,
    measure: Measure,
    thumb_start: Pixels,
    thumb_long: Pixels,
}

/// A point `main` along `axis` and `cross` across it.
fn point_along(axis: Axis, main: Pixels, cross: Pixels) -> Point<Pixels> {
    match axis {
        Axis::Vertical => point(cross, main),
        Axis::Horizontal => point(main, cross),
    }
}

/// A size `main` along `axis` and `cross` across it.
fn size_along(axis: Axis, main: Pixels, cross: Pixels) -> Size<Pixels> {
    match axis {
        Axis::Vertical => size(cross, main),
        Axis::Horizontal => size(main, cross),
    }
}

impl IntoElement for ScrollbarElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ScrollbarElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<Placed>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style {
            position: gpui::Position::Absolute,
            ..Style::default()
        };

        // A strip along the edge of its parent, over what scrolls in it.
        match self.axis {
            Axis::Vertical => {
                style.inset.top = px(0.).into();
                style.inset.bottom = px(0.).into();
                style.inset.right = px(0.).into();
                style.size.width = STRIP.into();
            }

            Axis::Horizontal => {
                style.inset.left = px(0.).into();
                style.inset.right = px(0.).into();
                style.inset.bottom = px(0.).into();
                style.size.height = STRIP.into();
            }
        }

        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let measure = self.source.measure(self.axis, cx)?;
        let (thumb_start, thumb_long) = thumb(bounds.size.along(self.axis), &measure)?;

        // The pointer goes no further than the bar, but the wheel goes on
        // to what scrolls.
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::BlockMouseExceptScroll);

        Some(Placed {
            track: bounds,
            hitbox,
            measure,
            thumb_start,
            thumb_long,
        })
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        placed: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(placed) = placed.take() else {
            return;
        };

        let axis = self.axis;
        let colors = cx.theme().colors;
        let dragging = self.grab.read(cx).is_some();
        let hovered = placed.hitbox.is_hovered(window);
        let active = dragging || hovered;

        let thickness = if active { THICK } else { THIN };
        let color = if dragging {
            colors.scrollbar_thumb_active
        } else if hovered {
            colors.scrollbar_thumb_hover
        } else {
            colors.scrollbar_thumb
        };

        let track_start = placed.track.origin.along(axis);
        let length = placed.track.size.along(axis);
        let across = placed.track.size.along(axis.invert());

        // Against the far edge across the track: the right, or the bottom.
        let thumb = Bounds::new(
            placed.track.origin
                + point_along(axis, placed.thumb_start, across - thickness - MARGIN),
            size_along(axis, placed.thumb_long, thickness),
        );

        if active {
            window.paint_quad(fill(placed.track, colors.scrollbar_track));
        }

        window.paint_quad(fill(thumb, color).corner_radii(thickness / 2.));
        window.set_cursor_style(CursorStyle::Arrow, &placed.hitbox);

        let measure = placed.measure;
        let (thumb_start, thumb_long) = (placed.thumb_start, placed.thumb_long);
        let pressed = self.source.clone();
        let dragged = self.source.clone();

        // A press on the thumb takes it where the pointer is; one on the
        // track brings the thumb's middle there, and takes it. Then the
        // pointer moves it wherever it goes.
        drag::listen(
            &placed.hitbox,
            &self.grab,
            move |position, window, cx| {
                let at = position.along(axis) - track_start;
                let on_thumb = at >= thumb_start && at <= thumb_start + thumb_long;
                let grab = match on_thumb {
                    true => at - thumb_start,
                    false => thumb_long / 2.,
                };

                if !on_thumb {
                    let distance = distance_at(at - grab, length, thumb_long, measure.most);

                    pressed.scroll_to(axis, distance, window, cx);
                }

                Some(grab)
            },
            move |grab, position, window, cx| {
                let start = position.along(axis) - track_start - *grab;
                let distance = distance_at(start, length, thumb_long, measure.most);

                dragged.scroll_to(axis, distance, window, cx);
            },
            window,
        );

        let state = self.bar.clone();
        let hitbox = placed.hitbox.clone();

        // It widens and darkens under the pointer.
        window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
            let hovered = hitbox.is_hovered(window);

            if phase == DispatchPhase::Bubble && hovered != state.read(cx).hovered {
                state.update(cx, |bar, _| bar.hovered = hovered);
                window.refresh();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(scrolled: f32, most: f32, shown: f32) -> Measure {
        Measure {
            scrolled: px(scrolled),
            most: px(most),
            shown: px(shown),
        }
    }

    #[test]
    fn shows_no_thumb_while_everything_is_in_view() {
        assert_eq!(thumb(px(300.), &measure(0., 0., 300.)), None);
        assert_eq!(thumb(px(300.), &measure(0., 0.4, 300.)), None);
        assert_eq!(thumb(px(0.), &measure(0., 100., 300.)), None);
    }

    #[test]
    fn sizes_the_thumb_by_how_much_shows() {
        // A quarter shows: the thumb is a quarter of the track.
        assert_eq!(
            thumb(px(400.), &measure(0., 300., 100.)),
            Some((px(0.), px(100.)))
        );

        // Scrolled to the end, it ends at the track's end.
        assert_eq!(
            thumb(px(400.), &measure(300., 300., 100.)),
            Some((px(300.), px(100.)))
        );

        // Halfway, it is halfway along what it can travel.
        assert_eq!(
            thumb(px(400.), &measure(150., 300., 100.)),
            Some((px(150.), px(100.)))
        );
    }

    #[test]
    fn keeps_a_thumb_long_enough_to_take() {
        let (_, long) = thumb(px(300.), &measure(0., 100_000., 100.)).unwrap();

        assert_eq!(long, SHORTEST);
        assert_eq!(
            thumb(px(10.), &measure(0., 1000., 10.)),
            Some((px(0.), px(10.)))
        );
    }

    #[test]
    fn scrolls_as_far_as_the_thumb_is_taken() {
        // A thumb of 100 on a track of 300 travels 200, over 400 to scroll.
        assert_eq!(distance_at(px(0.), px(300.), px(100.), px(400.)), px(0.));
        assert_eq!(
            distance_at(px(100.), px(300.), px(100.), px(400.)),
            px(200.)
        );
        assert_eq!(
            distance_at(px(500.), px(300.), px(100.), px(400.)),
            px(400.)
        );
        assert_eq!(distance_at(px(-20.), px(300.), px(100.), px(400.)), px(0.));
        assert_eq!(distance_at(px(5.), px(100.), px(100.), px(400.)), px(0.));
    }
}

/// The pointer on a scrollbar, in a window GPUI keeps in the test's own
/// process.
#[cfg(test)]
mod pointer_tests {
    use gpui::{
        Context, InteractiveElement as _, IntoElement, Modifiers, MouseButton, ParentElement as _,
        Render, ScrollHandle, StatefulInteractiveElement as _, Styled as _, TestAppContext,
        VisualTestContext, Window, div, point, px,
    };

    use super::Scrollbar;

    /// 400 high in an area 100 high and 200 wide, at the window's corner:
    /// its thumb is 25 long, travels 75, over 300 to scroll, along the
    /// strip from 188 to 200 across.
    struct Area {
        scroll: ScrollHandle,
    }

    impl Render for Area {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                div()
                    .relative()
                    .w(px(200.))
                    .h(px(100.))
                    .child(
                        div()
                            .id("area")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .child(div().h(px(400.))),
                    )
                    .child(Scrollbar::vertical("bar", self.scroll.clone())),
            )
        }
    }

    fn area(cx: &mut TestAppContext) -> (ScrollHandle, &mut VisualTestContext) {
        cx.update(crate::theme::init);

        let scroll = ScrollHandle::new();
        let shown = scroll.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Area { scroll: shown });

        (scroll, cx)
    }

    fn on_strip(y: f32) -> gpui::Point<gpui::Pixels> {
        point(px(196.), px(y))
    }

    #[gpui::test]
    fn drags_the_thumb_and_scrolls_as_far(cx: &mut TestAppContext) {
        let (scroll, cx) = area(cx);
        let none = Modifiers::none();

        cx.simulate_mouse_move(on_strip(10.), None, none);
        cx.simulate_mouse_down(on_strip(10.), MouseButton::Left, none);
        cx.simulate_mouse_move(on_strip(40.), MouseButton::Left, none);

        // 30 of the thumb's 75 is 120 of the 300.
        assert_eq!(scroll.offset().y, px(-120.));

        // Past the end of the track, and out of it, it stays at the end.
        cx.simulate_mouse_move(point(px(400.), px(500.)), MouseButton::Left, none);

        assert_eq!(scroll.offset().y, px(-300.));

        // Let go, the pointer moves nothing.
        cx.simulate_mouse_up(point(px(400.), px(500.)), MouseButton::Left, none);
        cx.simulate_mouse_move(on_strip(10.), None, none);

        assert_eq!(scroll.offset().y, px(-300.));
    }

    #[gpui::test]
    fn a_press_on_the_track_brings_the_thumb_there_and_takes_it(cx: &mut TestAppContext) {
        let (scroll, cx) = area(cx);
        let none = Modifiers::none();

        cx.simulate_mouse_move(on_strip(60.), None, none);
        cx.simulate_mouse_down(on_strip(60.), MouseButton::Left, none);

        // The thumb's middle at 60: it starts at 47.5, which is 190.
        assert_eq!(scroll.offset().y, px(-190.));

        cx.simulate_mouse_move(on_strip(35.), MouseButton::Left, none);

        assert_eq!(scroll.offset().y, px(-90.));

        cx.simulate_mouse_up(on_strip(35.), MouseButton::Left, none);
    }

    #[gpui::test]
    fn leaves_what_is_beside_it_to_the_pointer(cx: &mut TestAppContext) {
        let (scroll, cx) = area(cx);
        let none = Modifiers::none();

        // A press on the area, off the strip, takes no thumb.
        cx.simulate_mouse_move(point(px(100.), px(60.)), None, none);
        cx.simulate_mouse_down(point(px(100.), px(60.)), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(100.), px(90.)), MouseButton::Left, none);
        cx.simulate_mouse_up(point(px(100.), px(90.)), MouseButton::Left, none);

        assert_eq!(scroll.offset().y, px(0.));
    }
}
