//! A field's text as an entity: what it holds and what is selected, its
//! keys, the pointer, the platform's input methods, and the blinking
//! cursor. One state serves a field of one line and one of several, which
//! differ in what Enter and the arrows do. `Input` and `Textarea` draw it.

use std::ops::Range;
use std::time::Duration;

use gpui::{
    App, Bounds, ClipboardItem, Context, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    KeyBinding, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollEvent,
    SharedString, Subscription, Task, UTF16Selection, Window, actions, point,
};

use super::edit::{self, Buffer, EditKind, Motion};
use super::element::Painted;
use crate::scroll::Measure;

actions!(
    input,
    [
        MoveLeft,
        MoveRight,
        MoveWordLeft,
        MoveWordRight,
        MoveLineStart,
        MoveLineEnd,
        MoveStart,
        MoveEnd,
        MoveUp,
        MoveDown,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        SelectLineStart,
        SelectLineEnd,
        SelectStart,
        SelectEnd,
        SelectUp,
        SelectDown,
        SelectAll,
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteWordRight,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        Enter,
        NewLine,
        Submit,
    ]
);

pub(crate) const KEY_CONTEXT: &str = "Input";

/// How long the cursor shows, and hides, while it blinks.
const BLINK: Duration = Duration::from_millis(500);

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(KEY_CONTEXT);

    cx.bind_keys([
        KeyBinding::new("left", MoveLeft, context),
        KeyBinding::new("right", MoveRight, context),
        KeyBinding::new("ctrl-left", MoveWordLeft, context),
        KeyBinding::new("ctrl-right", MoveWordRight, context),
        KeyBinding::new("home", MoveLineStart, context),
        KeyBinding::new("end", MoveLineEnd, context),
        KeyBinding::new("ctrl-home", MoveStart, context),
        KeyBinding::new("ctrl-end", MoveEnd, context),
        KeyBinding::new("up", MoveUp, context),
        KeyBinding::new("down", MoveDown, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, context),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, context),
        KeyBinding::new("shift-home", SelectLineStart, context),
        KeyBinding::new("shift-end", SelectLineEnd, context),
        KeyBinding::new("ctrl-shift-home", SelectStart, context),
        KeyBinding::new("ctrl-shift-end", SelectEnd, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new("ctrl-a", SelectAll, context),
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("shift-backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, context),
        KeyBinding::new("ctrl-delete", DeleteWordRight, context),
        KeyBinding::new("ctrl-c", Copy, context),
        KeyBinding::new("ctrl-insert", Copy, context),
        KeyBinding::new("ctrl-x", Cut, context),
        KeyBinding::new("shift-delete", Cut, context),
        KeyBinding::new("ctrl-v", Paste, context),
        KeyBinding::new("shift-insert", Paste, context),
        KeyBinding::new("ctrl-z", Undo, context),
        KeyBinding::new("ctrl-shift-z", Redo, context),
        KeyBinding::new("ctrl-y", Redo, context),
        KeyBinding::new("enter", Enter, context),
        KeyBinding::new("shift-enter", NewLine, context),
        KeyBinding::new("ctrl-enter", Submit, context),
    ]);
}

/// What a field tells those who subscribe to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// The person changed the text.
    Change,
    /// The person sent what the field holds: Enter in a field of one line,
    /// or of several that submits on Enter, and Ctrl-Enter in any.
    Submit,
    Focus,
    Blur,
}

pub struct InputState {
    focus_handle: FocusHandle,
    buffer: Buffer,
    placeholder: SharedString,
    /// The fewest and the most rows a field of several lines shows, growing
    /// between them; none for a field of one line.
    rows: Option<(usize, usize)>,
    submit_on_enter: bool,
    masked: bool,
    disabled: bool,
    /// How far the text is scrolled in the field: across for one line, down
    /// for several.
    pub(super) scroll: Point<Pixels>,
    /// The cursor moved, so the next frame scrolls to show it.
    pub(super) reveal: bool,
    /// What was painted last, for the pointer and the input methods.
    pub(super) painted: Option<Painted>,
    /// A press in the field selects as the pointer moves, until released.
    selecting: bool,
    /// Where a run of Up and Down keeps the cursor across lines.
    goal_x: Option<Pixels>,
    pub(super) cursor_shown: bool,
    /// Tells a blink from the one that took its place.
    blinks: usize,
    _blinking: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<InputEvent> for InputState {}

impl Focusable for InputState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl InputState {
    /// A field of one line.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle().tab_stop(true);

        let subscriptions = vec![
            cx.on_focus(&focus_handle, window, |this, _, cx| {
                this.blink(cx);
                cx.emit(InputEvent::Focus);
                cx.notify();
            }),
            cx.on_blur(&focus_handle, window, |this, _, cx| {
                this.stop_blinking();
                this.selecting = false;
                cx.emit(InputEvent::Blur);
                cx.notify();
            }),
        ];

        InputState {
            focus_handle,
            buffer: Buffer::default(),
            placeholder: SharedString::default(),
            rows: None,
            submit_on_enter: false,
            masked: false,
            disabled: false,
            scroll: Point::default(),
            reveal: true,
            painted: None,
            selecting: false,
            goal_x: None,
            cursor_shown: false,
            blinks: 0,
            _blinking: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    /// Makes it a field of several lines, which grows from `min` rows to
    /// `max` as it fills, and scrolls past that.
    pub fn multi_line(mut self, min: usize, max: usize) -> Self {
        let min = min.max(1);

        self.rows = Some((min, max.max(min)));
        self
    }

    /// In a field of several lines, Enter sends it and Shift-Enter starts a
    /// line, as a chat's composer does.
    pub fn submit_on_enter(mut self, submit: bool) -> Self {
        self.submit_on_enter = submit;
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        let value = self.fit(&value.into());

        self.buffer.set_text(&value);
        self
    }

    /// Shows a dot for each character, for a secret, and copies none of it.
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn value(&self) -> &str {
        self.buffer.text()
    }

    pub fn placeholder_text(&self) -> &SharedString {
        &self.placeholder
    }

    /// Where the cursor is, as a byte offset of the text.
    pub fn cursor(&self) -> usize {
        self.buffer.cursor()
    }

    pub fn is_multi_line(&self) -> bool {
        self.rows.is_some()
    }

    pub(super) fn rows(&self) -> Option<(usize, usize)> {
        self.rows
    }

    pub fn is_masked(&self) -> bool {
        self.masked
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle.focus(window, cx);
    }

    /// Replaces the text, as a program does: nothing before can be undone,
    /// and no `Change` is told, since the person changed nothing.
    pub fn set_value(
        &mut self,
        value: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = self.fit(&value.into());

        self.buffer.set_text(&value);
        self.scroll = Point::default();
        self.reveal = true;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    pub fn set_masked(&mut self, masked: bool, cx: &mut Context<Self>) {
        self.masked = masked;
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    /// Puts the cursor at a byte offset of the text.
    pub fn set_cursor(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.buffer.move_to(offset);
        self.moved(window, cx);
    }

    pub fn select_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.buffer.select_all();
        self.moved(window, cx);
    }

    /// Empties the field as the person does, which can be undone and tells
    /// `Change`.
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.text().is_empty() {
            return;
        }

        self.buffer.select_all();
        self.buffer.replace(None, "", EditKind::Other);
        self.changed(window, cx);
    }

    pub(super) fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// How far a field of several lines is scrolled down, how far it can
    /// be, and how much of it shows; none for one line, or until painted.
    pub(crate) fn vertical_scroll(&self) -> Option<Measure> {
        let painted = self.painted.as_ref().filter(|_| self.rows.is_some())?;
        let shown = painted.bounds().size.height;

        Some(Measure {
            scrolled: self.scroll.y,
            most: (painted.height() - shown).max(Pixels::ZERO),
            shown,
        })
    }

    /// Scrolls a field of several lines down to `distance`, as its
    /// scrollbar does; the cursor may go out of view.
    pub(crate) fn scroll_down_to(&mut self, distance: Pixels, cx: &mut Context<Self>) {
        self.scroll.y = distance.max(Pixels::ZERO);
        cx.notify();
    }

    /// What the field shows of its text, and where an offset of it falls
    /// in that: dots for a secret.
    pub(super) fn shown(&self) -> SharedString {
        match self.masked {
            true => edit::mask(self.buffer.text()).into(),
            false => SharedString::from(self.buffer.text().to_string()),
        }
    }

    pub(super) fn shown_offset(&self, offset: usize) -> usize {
        match self.masked {
            true => edit::masked_offset(self.buffer.text(), offset),
            false => offset,
        }
    }

    fn offset_of_shown(&self, shown: usize) -> usize {
        match self.masked {
            true => edit::unmasked_offset(self.buffer.text(), shown),
            false => shown,
        }
    }

    /// A field of one line holds no line breaks.
    fn fit(&self, text: &str) -> String {
        match self.rows {
            Some(_) => text.replace("\r\n", "\n"),
            None => text.replace("\r\n", " ").replace(['\n', '\r'], " "),
        }
    }

    /// After the text changed.
    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.reveal = true;
        self.restart_blink(window, cx);
        window.notify_text_input_changed(&self.focus_handle);
        cx.emit(InputEvent::Change);
        cx.notify();
    }

    /// After the cursor or the selection moved.
    fn moved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reveal = true;
        self.restart_blink(window, cx);
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn restart_blink(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.focus_handle.is_focused(window) {
            self.blink(cx);
        }
    }

    /// Shows the cursor, and blinks it from now on.
    fn blink(&mut self, cx: &mut Context<Self>) {
        self.cursor_shown = true;
        self.blinks += 1;

        let blink = self.blinks;

        self._blinking = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(BLINK).await;

                let going = this.update(cx, |this, cx| {
                    if this.blinks != blink {
                        return false;
                    }

                    this.cursor_shown = !this.cursor_shown;
                    cx.notify();
                    true
                });

                if !matches!(going, Ok(true)) {
                    break;
                }
            }
        });
    }

    fn stop_blinking(&mut self) {
        self.blinks += 1;
        self.cursor_shown = false;
        self._blinking = Task::ready(());
    }

    fn motion(
        &mut self,
        motion: Motion,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Words would tell what a secret holds.
        let motion = match (self.masked, motion) {
            (true, Motion::WordLeft) => Motion::LineStart,
            (true, Motion::WordRight) => Motion::LineEnd,
            _ => motion,
        };

        self.buffer.move_by(motion, select);
        self.goal_x = None;
        self.moved(window, cx);
    }

    /// Moves the cursor a row up (-1) or down (1), as laid out. A field of
    /// one line lets the key go on to what holds it.
    fn vertical(&mut self, step: f32, select: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(painted) = self.painted.as_ref().filter(|_| self.rows.is_some()) else {
            cx.propagate();
            return;
        };

        let at = painted.position_of(self.shown_offset(self.buffer.cursor()));
        let x = self.goal_x.unwrap_or(at.x);
        let y = at.y + painted.line_height() * (step + 0.5);

        let target = if y < Pixels::ZERO {
            0
        } else if y >= painted.height() {
            self.buffer.text().len()
        } else {
            self.offset_of_shown(painted.offset_at_local(point(x, y)))
        };

        match select {
            true => self.buffer.select_to(target),
            false => self.buffer.move_to(target),
        }

        self.moved(window, cx);
        self.goal_x = Some(x);
    }

    fn insert(&mut self, text: &str, kind: EditKind, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.fit(text);

        self.buffer.replace(None, &text, kind);
        self.changed(window, cx);
    }

    fn move_left(&mut self, _: &MoveLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::Left, false, window, cx);
    }

    fn move_right(&mut self, _: &MoveRight, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::Right, false, window, cx);
    }

    fn move_word_left(&mut self, _: &MoveWordLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::WordLeft, false, window, cx);
    }

    fn move_word_right(&mut self, _: &MoveWordRight, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::WordRight, false, window, cx);
    }

    fn move_line_start(&mut self, _: &MoveLineStart, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::LineStart, false, window, cx);
    }

    fn move_line_end(&mut self, _: &MoveLineEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::LineEnd, false, window, cx);
    }

    fn move_start(&mut self, _: &MoveStart, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::Start, false, window, cx);
    }

    fn move_end(&mut self, _: &MoveEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::End, false, window, cx);
    }

    fn move_up(&mut self, _: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1., false, window, cx);
    }

    fn move_down(&mut self, _: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1., false, window, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::Left, true, window, cx);
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::Right, true, window, cx);
    }

    fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.motion(Motion::WordLeft, true, window, cx);
    }

    fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.motion(Motion::WordRight, true, window, cx);
    }

    fn select_line_start(
        &mut self,
        _: &SelectLineStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.motion(Motion::LineStart, true, window, cx);
    }

    fn select_line_end(&mut self, _: &SelectLineEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::LineEnd, true, window, cx);
    }

    fn select_start(&mut self, _: &SelectStart, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::Start, true, window, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.motion(Motion::End, true, window, cx);
    }

    fn select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1., true, window, cx);
    }

    fn select_down(&mut self, _: &SelectDown, window: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1., true, window, cx);
    }

    fn on_select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.select_all(window, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.delete_backward(false) {
            self.changed(window, cx);
        }
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.delete_forward(false) {
            self.changed(window, cx);
        }
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.buffer.delete_backward(!self.masked) {
            self.changed(window, cx);
        }
    }

    fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.buffer.delete_forward(!self.masked) {
            self.changed(window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.masked || self.buffer.selection().is_empty() {
            return;
        }

        cx.write_to_clipboard(ClipboardItem::new_string(
            self.buffer.selected_text().to_string(),
        ));
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.masked || self.buffer.selection().is_empty() {
            return;
        }

        cx.write_to_clipboard(ClipboardItem::new_string(
            self.buffer.selected_text().to_string(),
        ));
        self.buffer.replace(None, "", EditKind::Other);
        self.changed(window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };

        self.insert(&text, EditKind::Other, window, cx);
    }

    fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.undo() {
            self.changed(window, cx);
        }
    }

    fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.redo() {
            self.changed(window, cx);
        }
    }

    /// A field of several lines takes Enter as a new line. Otherwise Enter
    /// sends it, and a field of one line lets the key go on too, so that
    /// what holds it, such as a dialog or a list, does what Enter does
    /// there.
    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        match self.rows.is_some() && !self.submit_on_enter {
            true => self.insert("\n", EditKind::Other, window, cx),
            false => {
                cx.emit(InputEvent::Submit);

                if self.rows.is_none() {
                    cx.propagate();
                }
            }
        }
    }

    fn new_line(&mut self, _: &NewLine, window: &mut Window, cx: &mut Context<Self>) {
        match self.rows.is_some() {
            true => self.insert("\n", EditKind::Other, window, cx),
            false => cx.propagate(),
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Submit);
    }

    /// The offset of the text under the pointer at `position`, in the
    /// window.
    fn offset_at(&self, position: Point<Pixels>) -> usize {
        match &self.painted {
            Some(painted) => self.offset_of_shown(painted.offset_at(position)),
            None => self.buffer.text().len(),
        }
    }

    pub(crate) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }

        self.focus_handle.focus(window, cx);

        let offset = self.offset_at(event.position);

        match event.click_count {
            1 if event.modifiers.shift => self.buffer.select_to(offset),
            1 => self.buffer.move_to(offset),
            2 if !self.masked => self.buffer.select_word_at(offset),
            2 => self.buffer.select_all(),
            _ => match self.rows {
                Some(_) => self.buffer.select_line_at(offset),
                None => self.buffer.select_all(),
            },
        }

        self.selecting = event.click_count == 1;
        self.goal_x = None;
        self.moved(window, cx);
    }

    pub(crate) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.selecting {
            return;
        }

        let offset = self.offset_at(event.position);

        self.buffer.select_to(offset);
        self.moved(window, cx);
    }

    pub(crate) fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    pub(crate) fn on_scroll(
        &mut self,
        event: &ScrollEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(painted) = &self.painted else {
            return;
        };

        let delta = event.delta.pixel_delta(painted.line_height());
        let most = (painted.height() - painted.bounds().size.height).max(Pixels::ZERO);
        let y = (self.scroll.y - delta.y).clamp(Pixels::ZERO, most);

        if y != self.scroll.y {
            self.scroll.y = y;
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// Binds the field's keys on the element that tracks its focus.
    pub(super) fn actions<E: gpui::InteractiveElement>(
        element: E,
        state: &gpui::Entity<InputState>,
        window: &Window,
    ) -> E {
        element
            .on_action(window.listener_for(state, Self::move_left))
            .on_action(window.listener_for(state, Self::move_right))
            .on_action(window.listener_for(state, Self::move_word_left))
            .on_action(window.listener_for(state, Self::move_word_right))
            .on_action(window.listener_for(state, Self::move_line_start))
            .on_action(window.listener_for(state, Self::move_line_end))
            .on_action(window.listener_for(state, Self::move_start))
            .on_action(window.listener_for(state, Self::move_end))
            .on_action(window.listener_for(state, Self::move_up))
            .on_action(window.listener_for(state, Self::move_down))
            .on_action(window.listener_for(state, Self::select_left))
            .on_action(window.listener_for(state, Self::select_right))
            .on_action(window.listener_for(state, Self::select_word_left))
            .on_action(window.listener_for(state, Self::select_word_right))
            .on_action(window.listener_for(state, Self::select_line_start))
            .on_action(window.listener_for(state, Self::select_line_end))
            .on_action(window.listener_for(state, Self::select_start))
            .on_action(window.listener_for(state, Self::select_end))
            .on_action(window.listener_for(state, Self::select_up))
            .on_action(window.listener_for(state, Self::select_down))
            .on_action(window.listener_for(state, Self::on_select_all))
            .on_action(window.listener_for(state, Self::backspace))
            .on_action(window.listener_for(state, Self::delete))
            .on_action(window.listener_for(state, Self::delete_word_left))
            .on_action(window.listener_for(state, Self::delete_word_right))
            .on_action(window.listener_for(state, Self::copy))
            .on_action(window.listener_for(state, Self::cut))
            .on_action(window.listener_for(state, Self::paste))
            .on_action(window.listener_for(state, Self::undo))
            .on_action(window.listener_for(state, Self::redo))
            .on_action(window.listener_for(state, Self::enter))
            .on_action(window.listener_for(state, Self::new_line))
            .on_action(window.listener_for(state, Self::submit))
    }
}

/// The byte offset of a UTF-16 offset in `text`.
fn byte_of_utf16(text: &str, offset: usize) -> usize {
    let mut utf16 = 0;

    for (index, c) in text.char_indices() {
        if utf16 >= offset {
            return index;
        }

        utf16 += c.len_utf16();
    }

    text.len()
}

/// The platform asks and tells in UTF-16 offsets of the text itself; what
/// is painted is in bytes of what is shown.
impl EntityInputHandler for InputState {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        // A secret is not handed to input methods.
        if self.masked {
            return None;
        }

        let range = self.buffer.range_from_utf16(&range);

        adjusted.replace(self.buffer.range_to_utf16(&range));
        Some(self.buffer.text()[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.range_to_utf16(&self.buffer.selection()),
            reversed: self.buffer.reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked()
            .map(|range| self.buffer.range_to_utf16(&range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.unmark();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }

        let range = range.map(|range| self.buffer.range_from_utf16(&range));
        let text = self.fit(text);

        self.buffer.replace(range, &text, EditKind::Typing);
        self.changed(window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }

        let range = range.map(|range| self.buffer.range_from_utf16(&range));
        let text = self.fit(text);
        let selected = selected.map(|selected| {
            byte_of_utf16(&text, selected.start)..byte_of_utf16(&text, selected.end)
        });

        self.buffer.replace_and_mark(range, &text, selected);
        self.changed(window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let painted = self.painted.as_ref()?;
        let range = self.buffer.range_from_utf16(&range);

        Some(painted.bounds_of(self.shown_offset(range.start)..self.shown_offset(range.end)))
    }

    fn selection_bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.bounds_for_range(range, bounds, window, cx)
            .into_iter()
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let painted = self.painted.as_ref()?;
        let offset = self.offset_of_shown(painted.offset_at(point));

        Some(self.buffer.offset_to_utf16(offset))
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.buffer.range_from_utf16(&range);

        self.buffer.select_range(range, false);
        self.moved(window, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.buffer.len_utf16())
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        !self.disabled
    }
}
