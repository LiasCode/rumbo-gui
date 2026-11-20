//! Words that can be selected and copied: a press and a drag over them
//! choose a run, a double press a word, a triple press all of them, and
//! Ctrl-C copies what is chosen. A press elsewhere lets it go. A press on
//! a link that moves nothing follows it.

use std::ops::Range;

use gpui::Styled as _;
use gpui::{
    App, ClipboardItem, CursorStyle, DispatchPhase, ElementId, FocusHandle, FontStyle, FontWeight,
    HighlightStyle, HitboxBehavior, InteractiveElement as _, IntoElement, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, RenderOnce,
    SharedString, StrikethroughStyle, StyledText, TextLayout, UnderlineStyle, Window, canvas, div,
    px,
};

use super::blocks::{Marks, segments, word_at};
use crate::text::{Copy, SelectAll};
use crate::theme::ActiveTheme as _;

const CONTEXT: &str = "SelectableText";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-c", Copy, Some(CONTEXT)),
        KeyBinding::new("ctrl-a", SelectAll, Some(CONTEXT)),
    ]);
}

/// What is chosen of the words, from where the press went down to where
/// the pointer is, and whether the button is still down.
struct Selection {
    anchor: usize,
    head: usize,
    held: bool,
    /// The link the button went down on, followed if it comes up there
    /// having chosen nothing.
    pressed: Option<SharedString>,
    focus: FocusHandle,
}

impl Selection {
    fn range(&self) -> Option<Range<usize>> {
        let range = self.anchor.min(self.head)..self.anchor.max(self.head);

        (!range.is_empty()).then_some(range)
    }
}

#[derive(IntoElement)]
pub(crate) struct SelectableText {
    id: ElementId,
    text: SharedString,
    spans: Vec<(Range<usize>, Marks)>,
    links: Vec<(Range<usize>, SharedString)>,
}

impl SelectableText {
    pub(crate) fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        SelectableText {
            id: id.into(),
            text: text.into(),
            spans: Vec::new(),
            links: Vec::new(),
        }
    }

    /// The runs that are marked, in order and apart from each other, and
    /// the links some of them go to.
    pub(crate) fn marked(
        mut self,
        spans: Vec<(Range<usize>, Marks)>,
        links: Vec<(Range<usize>, SharedString)>,
    ) -> Self {
        self.spans = spans;
        self.links = links;
        self
    }
}

/// The boundary between characters nearest to `position`: before the
/// first line, the start; after the last, the end.
fn index_at(layout: &TextLayout, position: Point<Pixels>) -> usize {
    let bounds = layout.bounds();
    let line_height = layout.line_height();

    if position.y < bounds.top() {
        return 0;
    }

    let mut origin = bounds.origin;
    let mut start = 0;

    for line in layout.line_layouts() {
        let bottom = origin.y + line.size(line_height).height;

        if position.y < bottom {
            let (Ok(index) | Err(index)) =
                line.closest_index_for_position(position - origin, line_height);

            return start + index;
        }

        origin.y = bottom;
        start += line.len() + 1;
    }

    layout.len()
}

/// The link under `position`, if the pointer is on one of its characters.
fn link_at(
    links: &[(Range<usize>, SharedString)],
    layout: &TextLayout,
    position: Point<Pixels>,
) -> Option<SharedString> {
    if links.is_empty() {
        return None;
    }

    let index = layout.index_for_position(position).ok()?;

    links
        .iter()
        .find(|(range, _)| range.contains(&index))
        .map(|(_, url)| url.clone())
}

impl RenderOnce for SelectableText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| Selection {
            anchor: 0,
            head: 0,
            held: false,
            pressed: None,
            focus: cx.focus_handle().tab_stop(false),
        });

        let selection = state.read(cx);
        let chosen = selection.range();
        let focus = selection.focus.clone();
        let held = selection.held;

        let theme = cx.theme();
        let colors = theme.colors;
        let len = self.text.len();

        let highlights = segments(len, &self.spans, chosen.clone())
            .into_iter()
            .map(|(range, marks, chosen)| {
                let mut style = HighlightStyle::default();

                if marks.strong {
                    style.font_weight = Some(FontWeight::SEMIBOLD);
                }

                if marks.emphasis {
                    style.font_style = Some(FontStyle::Italic);
                }

                if marks.code {
                    style.background_color = Some(colors.muted);
                }

                if marks.delete {
                    style.strikethrough = Some(StrikethroughStyle {
                        thickness: px(1.),
                        color: None,
                    });
                }

                if marks.link {
                    style.color = Some(colors.link);
                    style.underline = Some(UnderlineStyle {
                        thickness: px(1.),
                        color: None,
                        wavy: false,
                    });
                }

                if chosen {
                    style.background_color = Some(colors.selection);
                }

                (range, style)
            })
            .collect::<Vec<_>>();

        let code = self
            .spans
            .iter()
            .filter(|(_, marks)| marks.code)
            .map(|(range, _)| (range.clone(), theme.mono_family.clone()))
            .collect::<Vec<_>>();

        let text = StyledText::new(self.text.clone())
            .with_highlights(highlights)
            .with_font_family_overrides(code);
        let layout = text.layout().clone();

        let copied = self.text.clone();
        let copy = {
            let state = state.clone();

            move |_: &Copy, _: &mut Window, cx: &mut App| match state.read(cx).range() {
                Some(range) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(copied[range].to_string()))
                }
                None => cx.propagate(),
            }
        };
        let select_all = {
            let state = state.clone();

            move |_: &SelectAll, _: &mut Window, cx: &mut App| {
                state.update(cx, |selection, cx| {
                    selection.anchor = 0;
                    selection.head = len;
                    cx.notify();
                });
            }
        };

        let links = self.links;
        let words = self.text;

        let listen = canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                let position = window.mouse_position();
                let over_link =
                    hitbox.is_hovered(window) && link_at(&links, &layout, position).is_some();

                let cursor = match over_link {
                    true => CursorStyle::PointingHand,
                    false => CursorStyle::IBeam,
                };

                window.set_cursor_style(cursor, &hitbox);

                if held {
                    window.set_window_cursor_style(CursorStyle::IBeam);
                }

                let on_text = hitbox.clone();
                let pressed = state.clone();
                let pressed_layout = layout.clone();
                let pressed_links = links.clone();

                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if event.button != MouseButton::Left {
                        return;
                    }

                    // A press anywhere else lets what is chosen go.
                    if !on_text.is_hovered(window) {
                        if phase == DispatchPhase::Capture {
                            pressed.update(cx, |selection, cx| {
                                if selection.range().is_some() {
                                    selection.head = selection.anchor;
                                    cx.notify();
                                }
                            });
                        }

                        return;
                    }

                    if phase != DispatchPhase::Bubble {
                        return;
                    }

                    let index = index_at(&pressed_layout, event.position);
                    let link = link_at(&pressed_links, &pressed_layout, event.position);

                    pressed.update(cx, |selection, cx| {
                        match event.click_count {
                            1 if event.modifiers.shift => selection.head = index,
                            1 => {
                                selection.anchor = index;
                                selection.head = index;
                            }
                            2 => {
                                let word = word_at(&words, index);

                                selection.anchor = word.start;
                                selection.head = word.end;
                            }
                            _ => {
                                selection.anchor = 0;
                                selection.head = words.len();
                            }
                        }

                        selection.held = event.click_count == 1;
                        selection.pressed = link.filter(|_| event.click_count == 1);
                        selection.focus.focus(window, cx);
                        cx.notify();
                    });

                    cx.stop_propagation();
                });

                let moved = state.clone();
                let moved_layout = layout.clone();
                let hovered = hitbox.clone();
                let hovered_links = links.clone();

                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }

                    if !moved.read(cx).held {
                        // The pointer went on or off a link: its cursor changes.
                        let on_link = hovered.is_hovered(window)
                            && link_at(&hovered_links, &moved_layout, event.position).is_some();

                        if on_link != over_link {
                            window.refresh();
                        }

                        return;
                    }

                    // The button came up where no window heard it.
                    if event.pressed_button != Some(MouseButton::Left) {
                        moved.update(cx, |selection, cx| {
                            selection.held = false;
                            selection.pressed = None;
                            cx.notify();
                        });

                        return;
                    }

                    let index = index_at(&moved_layout, event.position);

                    moved.update(cx, |selection, cx| {
                        if selection.head != index {
                            selection.head = index;
                            cx.notify();
                        }
                    });
                });

                let released = state.clone();
                let released_layout = layout.clone();
                let released_links = links.clone();

                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture
                        || event.button != MouseButton::Left
                        || !released.read(cx).held
                    {
                        return;
                    }

                    let link = link_at(&released_links, &released_layout, event.position);
                    let followed = released.update(cx, |selection, cx| {
                        selection.held = false;
                        cx.notify();

                        selection.pressed.take().filter(|pressed| {
                            selection.range().is_none() && link == Some(pressed.clone())
                        })
                    });

                    if let Some(url) = followed {
                        cx.open_url(&url);
                    }

                    window.refresh();
                });
            },
        )
        .absolute()
        .inset_0();

        div()
            .id(self.id)
            .key_context(CONTEXT)
            .track_focus(&focus)
            .relative()
            .on_action(copy)
            .on_action(select_all)
            .child(text)
            .child(listen)
    }
}

/// The pointer on words, in a window GPUI keeps in the test's own process,
/// whose text system gives each character 0.6 of the font's size: 9.6 at
/// the 16 the window draws in.
#[cfg(test)]
mod tests {
    use gpui::{
        Context, IntoElement, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent,
        ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext, Window, div,
        point, px,
    };

    use super::SelectableText;

    const WORDS: &str = "read the changes";

    struct Words;

    impl Render for Words {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().w(px(400.)).child(SelectableText::new("words", WORDS)))
                .child(div().h(px(100.)))
        }
    }

    fn words(cx: &mut TestAppContext) -> &mut VisualTestContext {
        cx.update(|cx| {
            crate::theme::init(cx);
            crate::text::bind_keys(cx);
            super::bind_keys(cx);
        });

        let (_, cx) = cx.add_window_view(|_, _| Words);

        cx.run_until_parked();
        cx
    }

    /// Where the boundary before character `index` is, on the first line.
    fn at(index: usize) -> gpui::Point<gpui::Pixels> {
        point(px(index as f32 * 9.6), px(8.))
    }

    fn copied(cx: &mut VisualTestContext) -> Option<String> {
        cx.simulate_keystrokes("ctrl-c");
        cx.read_from_clipboard().and_then(|item| item.text())
    }

    #[gpui::test]
    fn copies_the_words_dragged_over(cx: &mut TestAppContext) {
        let cx = words(cx);
        let none = Modifiers::none();

        cx.simulate_mouse_move(at(5), None, none);
        cx.simulate_mouse_down(at(5), MouseButton::Left, none);
        cx.simulate_mouse_move(at(16), MouseButton::Left, none);
        cx.simulate_mouse_up(at(16), MouseButton::Left, none);

        assert_eq!(copied(cx).as_deref(), Some("the changes"));

        // Dragged back past where it started, it chooses what is before.
        cx.simulate_mouse_down(at(8), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(-20.), px(8.)), MouseButton::Left, none);
        cx.simulate_mouse_up(point(px(-20.), px(8.)), MouseButton::Left, none);

        assert_eq!(copied(cx).as_deref(), Some("read the"));
    }

    fn press(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, click_count: usize) {
        let modifiers = Modifiers::none();
        let button = MouseButton::Left;

        cx.simulate_event(MouseDownEvent {
            position: at,
            modifiers,
            button,
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position: at,
            modifiers,
            button,
            click_count,
        });
    }

    #[gpui::test]
    fn chooses_a_word_or_everything_by_how_many_presses(cx: &mut TestAppContext) {
        let cx = words(cx);

        cx.simulate_mouse_move(at(10), None, Modifiers::none());
        press(cx, at(10), 1);
        press(cx, at(10), 2);

        assert_eq!(copied(cx).as_deref(), Some("changes"));

        press(cx, at(10), 3);

        assert_eq!(copied(cx).as_deref(), Some(WORDS));
    }

    #[gpui::test]
    fn lets_go_on_a_press_elsewhere(cx: &mut TestAppContext) {
        let cx = words(cx);
        let none = Modifiers::none();

        cx.simulate_mouse_move(at(0), None, none);
        cx.simulate_mouse_down(at(0), MouseButton::Left, none);
        cx.simulate_mouse_move(at(4), MouseButton::Left, none);
        cx.simulate_mouse_up(at(4), MouseButton::Left, none);

        assert_eq!(copied(cx).as_deref(), Some("read"));

        cx.write_to_clipboard(gpui::ClipboardItem::new_string("kept".into()));
        cx.simulate_click(point(px(10.), px(60.)), none);

        assert_eq!(copied(cx).as_deref(), Some("kept"));
    }
}
