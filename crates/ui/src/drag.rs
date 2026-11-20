//! A press that follows the pointer: once the left button goes down on an
//! area, every move of the pointer is told, wherever it goes, until the
//! button comes up. A scrollbar's thumb, a slider's, the divider between
//! panels, and the edge of a table's column are dragged so.

use gpui::{
    App, Bounds, CursorStyle, DispatchPhase, Entity, Hitbox, HitboxBehavior, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Styled as _, Window,
    canvas,
};

/// What a press took, kept until the button comes up; none while nothing
/// is held.
pub(crate) type Hold<T> = Entity<Option<T>>;

/// Hears a drag on `hitbox`, for an element to call while it paints.
/// `press` is told where the left button went down on it, and what it gives
/// back is held, if anything; `drag` is told each move while it is held,
/// wherever the pointer is; and the button coming up lets it go. Nothing
/// under the pointer hears a press it takes, nor the moves while it holds.
pub(crate) fn listen<T: Clone + 'static>(
    hitbox: &Hitbox,
    hold: &Hold<T>,
    press: impl Fn(Point<Pixels>, &mut Window, &mut App) -> Option<T> + 'static,
    drag: impl Fn(&T, Point<Pixels>, &mut Window, &mut App) + 'static,
    window: &mut Window,
) {
    let hitbox = hitbox.clone();
    let held = hold.clone();

    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble
            || event.button != MouseButton::Left
            || !hitbox.is_hovered(window)
        {
            return;
        }

        let Some(taken) = press(event.position, window, cx) else {
            return;
        };

        held.update(cx, |held, _| *held = Some(taken));
        cx.stop_propagation();
        window.refresh();
    });

    let held = hold.clone();

    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture {
            return;
        }

        let Some(taken) = held.read(cx).clone() else {
            return;
        };

        // The button came up where no window heard it.
        if event.pressed_button != Some(MouseButton::Left) {
            held.update(cx, |held, _| *held = None);
            window.refresh();
            return;
        }

        drag(&taken, event.position, window, cx);
        cx.stop_propagation();
    });

    let held = hold.clone();

    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Capture
            && event.button == MouseButton::Left
            && held.read(cx).is_some()
        {
            held.update(cx, |held, _| *held = None);
            window.refresh();
        }
    });
}

/// An area over the whole of its parent, which is `relative`, that takes a
/// drag as `listen` does. The pointer shows `cursor` over it, and anywhere
/// while it holds. Its callbacks are told the area's bounds as well.
pub(crate) fn area<T: Clone + 'static>(
    hold: Hold<T>,
    cursor: CursorStyle,
    press: impl Fn(Point<Pixels>, Bounds<Pixels>, &mut Window, &mut App) -> Option<T> + 'static,
    drag: impl Fn(&T, Point<Pixels>, Bounds<Pixels>, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |bounds, hitbox, window, cx| {
            window.set_cursor_style(cursor, &hitbox);

            if hold.read(cx).is_some() {
                window.set_window_cursor_style(cursor);
            }

            listen(
                &hitbox,
                &hold,
                move |at, window, cx| press(at, bounds, window, cx),
                move |taken, at, window, cx| drag(taken, at, bounds, window, cx),
                window,
            );
        },
    )
    .absolute()
    .inset_0()
}
