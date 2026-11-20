//! The frame of a window that draws its own, where the desktop leaves it to
//! the window, as on Linux: a border, a shadow while the window floats, and
//! edges and corners that resize it when dragged. A window the desktop
//! frames has none of it, and a tiled one none on the sides it is tiled
//! on. It holds everything the window's view draws.

use gpui::{
    AnyElement, App, BoxShadow, Decorations, DispatchPhase, Edges, HitboxBehavior, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels,
    Point, RenderOnce, ResizeEdge, Size, Styled as _, Tiling, Window, canvas, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::theme::ActiveTheme as _;

/// How far the shadow reaches around a floating window, which the window
/// keeps clear for it.
#[cfg(target_os = "linux")]
const SHADOW: Pixels = px(20.);
#[cfg(not(target_os = "linux"))]
const SHADOW: Pixels = px(0.);

const BORDER: Pixels = px(1.);

/// How far inside its border an edge can still be taken.
const GRIP: Pixels = px(4.);

/// The room kept around the frame on each side, which a tiled side has
/// none of.
fn insets(tiling: &Tiling) -> Edges<Pixels> {
    let side = |tiled: bool| if tiled { Pixels::ZERO } else { SHADOW };

    Edges {
        top: side(tiling.top),
        right: side(tiling.right),
        bottom: side(tiling.bottom),
        left: side(tiling.left),
    }
}

/// The edge or corner a press at `at` resizes a window `size` large from:
/// in the room around its frame, or `grip` inside it, on a side that is
/// not tiled.
fn edge(
    at: Point<Pixels>,
    size: Size<Pixels>,
    insets: Edges<Pixels>,
    grip: Pixels,
    tiling: &Tiling,
) -> Option<ResizeEdge> {
    let top = !tiling.top && at.y < insets.top + grip;
    let bottom = !tiling.bottom && at.y > size.height - insets.bottom - grip;
    let left = !tiling.left && at.x < insets.left + grip;
    let right = !tiling.right && at.x > size.width - insets.right - grip;

    match (top, bottom, left, right) {
        (true, _, true, _) => Some(ResizeEdge::TopLeft),
        (true, _, _, true) => Some(ResizeEdge::TopRight),
        (_, true, true, _) => Some(ResizeEdge::BottomLeft),
        (_, true, _, true) => Some(ResizeEdge::BottomRight),
        (true, _, _, _) => Some(ResizeEdge::Top),
        (_, true, _, _) => Some(ResizeEdge::Bottom),
        (_, _, true, _) => Some(ResizeEdge::Left),
        (_, _, _, true) => Some(ResizeEdge::Right),
        _ => None,
    }
}

fn cursor(edge: ResizeEdge) -> gpui::CursorStyle {
    use gpui::CursorStyle::*;

    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => ResizeUpRightDownLeft,
    }
}

#[derive(IntoElement, Default)]
pub struct WindowFrame {
    children: Vec<AnyElement>,
}

impl WindowFrame {
    pub fn new() -> Self {
        WindowFrame::default()
    }
}

impl ParentElement for WindowFrame {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// The shadow of a floating window, fainter while another has the
/// keyboard. It reaches no further than the room kept for it.
fn shadow(active: bool) -> Vec<BoxShadow> {
    let strength = if active { 0.18 } else { 0.13 };
    let color = Hsla {
        h: 0.,
        s: 0.,
        l: 0.,
        a: strength,
    };

    vec![
        BoxShadow {
            color,
            offset: point(px(0.), px(2.)),
            blur_radius: px(10.),
            spread_radius: px(-1.),
            inset: false,
        },
        BoxShadow {
            color,
            offset: point(px(0.), px(1.)),
            blur_radius: px(3.),
            spread_radius: px(0.),
            inset: false,
        },
    ]
}

impl RenderOnce for WindowFrame {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Decorations::Client { tiling } = window.window_decorations() else {
            return div().size_full().children(self.children).into_any_element();
        };

        // The room stays kept while the window tiles, or its first resize
        // after it would count the shadow twice.
        window.set_client_inset(SHADOW);

        let border = cx.theme().colors.border;
        let room = insets(&tiling);
        let resizable = window.is_resizable();

        let frame = div()
            .size_full()
            .overflow_hidden()
            .border_color(border)
            .when(!tiling.top, |frame| frame.border_t(BORDER))
            .when(!tiling.bottom, |frame| frame.border_b(BORDER))
            .when(!tiling.left, |frame| frame.border_l(BORDER))
            .when(!tiling.right, |frame| frame.border_r(BORDER))
            .when(!tiling.is_tiled(), |frame| {
                frame.shadow(shadow(window.is_window_active()))
            })
            .children(self.children);

        // Over everything, so its edges are taken before what lies under
        // them, and only there.
        let edges = canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                if !resizable {
                    return;
                }

                let size = window.window_bounds().get_bounds().size;

                if let Some(edge) = edge(window.mouse_position(), size, room, GRIP, &tiling) {
                    window.set_cursor_style(cursor(edge), &hitbox);
                }

                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                        return;
                    }

                    let size = window.window_bounds().get_bounds().size;

                    if let Some(edge) = edge(event.position, size, room, GRIP, &tiling) {
                        window.start_window_resize(edge);
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .inset_0();

        div()
            .relative()
            .size_full()
            .when(!tiling.top, |kept| kept.pt(room.top))
            .when(!tiling.bottom, |kept| kept.pb(room.bottom))
            .when(!tiling.left, |kept| kept.pl(room.left))
            .when(!tiling.right, |kept| kept.pr(room.right))
            // The pointer moving near an edge shows its cursor.
            .on_mouse_move(|_, window, _| window.refresh())
            .child(frame)
            .child(edges)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Edges, ResizeEdge, Tiling, point, px, size};

    use super::edge;

    fn floating() -> Tiling {
        Tiling::default()
    }

    #[test]
    fn takes_an_edge_in_the_room_around_the_frame_or_just_inside_it() {
        let window = size(px(800.), px(600.));
        let room = Edges::all(px(20.));
        let at = |x: f32, y: f32| edge(point(px(x), px(y)), window, room, px(4.), &floating());

        assert_eq!(at(400., 10.), Some(ResizeEdge::Top));
        assert_eq!(at(400., 23.), Some(ResizeEdge::Top));
        assert_eq!(at(400., 25.), None);
        assert_eq!(at(790., 300.), Some(ResizeEdge::Right));
        assert_eq!(at(400., 590.), Some(ResizeEdge::Bottom));
        assert_eq!(at(5., 300.), Some(ResizeEdge::Left));
        assert_eq!(at(400., 300.), None);
    }

    #[test]
    fn takes_a_corner_where_two_edges_meet() {
        let window = size(px(800.), px(600.));
        let room = Edges::all(px(20.));
        let at = |x: f32, y: f32| edge(point(px(x), px(y)), window, room, px(4.), &floating());

        assert_eq!(at(10., 10.), Some(ResizeEdge::TopLeft));
        assert_eq!(at(790., 10.), Some(ResizeEdge::TopRight));
        assert_eq!(at(10., 590.), Some(ResizeEdge::BottomLeft));
        assert_eq!(at(790., 590.), Some(ResizeEdge::BottomRight));
    }

    #[test]
    fn takes_no_edge_on_a_side_that_is_tiled() {
        let window = size(px(800.), px(600.));
        let tiled = Tiling {
            top: true,
            left: true,
            right: false,
            bottom: false,
        };
        let room = Edges {
            top: px(0.),
            left: px(0.),
            right: px(20.),
            bottom: px(20.),
        };
        let at = |x: f32, y: f32| edge(point(px(x), px(y)), window, room, px(4.), &tiled);

        assert_eq!(at(400., 1.), None);
        assert_eq!(at(1., 300.), None);
        assert_eq!(at(1., 590.), Some(ResizeEdge::Bottom));
        assert_eq!(at(790., 1.), Some(ResizeEdge::Right));
    }
}
