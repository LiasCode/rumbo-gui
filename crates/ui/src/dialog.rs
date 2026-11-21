//! A question put to the person over the work, which waits for an answer:
//! a modal window with a title, what it asks, and its buttons, the last of
//! which does what it asks. Enter does it from a field of one line or the
//! window; Escape, the button that gives up, or the close button give up.
//!
//! A window has one `Dialogs`, which its view keeps and draws with a
//! `DialogLayer` in its root; anything that has it opens a dialog, which
//! shows over any opened before it.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, IntoElement, ParentElement, Rems, RenderOnce,
    SharedString, Styled as _, Window, div,
};

use crate::button::Button;
use crate::floating_window::FloatingWindow;
use crate::root::h_flex;
use crate::size::rems_from_px;

/// What is called when a dialog's button is pressed: whether the dialog
/// closes.
type Answer = Rc<dyn Fn(&mut Window, &mut App) -> bool>;

/// What a dialog shows and does, built anew on each frame it shows.
pub struct Dialog {
    title: SharedString,
    width: Rems,
    children: Vec<AnyElement>,
    ok: Option<(SharedString, Answer)>,
    cancel: Option<(SharedString, Answer)>,
    danger: bool,
    footer: Option<AnyElement>,
    close_on_backdrop: bool,
    initial_focus: Option<FocusHandle>,
}

impl Dialog {
    fn new() -> Self {
        Dialog {
            title: SharedString::default(),
            width: rems_from_px(440.),
            children: Vec::new(),
            ok: None,
            cancel: None,
            danger: false,
            footer: None,
            close_on_backdrop: true,
            initial_focus: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn width(mut self, width: Rems) -> Self {
        self.width = width;
        self
    }

    /// The button that does what it asks, and Enter: whether the dialog
    /// closes once it has.
    pub fn on_ok(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.ok = Some((label.into(), Rc::new(handler)));
        self
    }

    /// The button that gives up, Escape, and the close button: whether the
    /// dialog closes. Without it they close it.
    pub fn on_cancel(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.cancel = Some((label.into(), Rc::new(handler)));
        self
    }

    /// What it does cannot be undone, as deleting: its button is red.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// Buttons of its own along its foot, in place of those it makes.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// Whether a press on the work behind it gives up, as it does unless
    /// it holds what would be lost.
    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    /// What takes the keyboard when it shows, such as its first field.
    pub fn initial_focus(mut self, focus: FocusHandle) -> Self {
        self.initial_focus = Some(focus);
        self
    }
}

impl ParentElement for Dialog {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

type Build = Rc<dyn Fn(Dialog, &mut Window, &mut App) -> Dialog>;

/// One dialog shown, and where the keyboard goes back to when it closes.
struct Opened {
    id: u64,
    build: Build,
    restore: Option<FocusHandle>,
}

/// The dialogs a window shows, the last over the others.
#[derive(Default)]
pub struct Dialogs {
    next: u64,
    shown: Vec<Opened>,
}

impl Dialogs {
    pub fn new() -> Self {
        Dialogs::default()
    }

    /// Shows the dialog `build` makes over the others, until it closes.
    pub fn open(
        &mut self,
        build: impl Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = self.next;

        self.next += 1;
        self.shown.push(Opened {
            id,
            build: Rc::new(build),
            restore: window.focused(cx),
        });
        cx.notify();
    }

    /// Closes the last dialog, and gives the keyboard back to where it was.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(opened) = self.shown.pop() {
            if let Some(restore) = opened.restore {
                restore.focus(window, cx);
            }

            cx.notify();
        }
    }

    pub fn is_open(&self) -> bool {
        !self.shown.is_empty()
    }
}

/// Where a window's dialogs show, over the work. It goes in the root of the
/// window's view.
#[derive(IntoElement)]
pub struct DialogLayer {
    dialogs: Entity<Dialogs>,
}

impl DialogLayer {
    pub fn new(dialogs: &Entity<Dialogs>) -> Self {
        DialogLayer {
            dialogs: dialogs.clone(),
        }
    }
}

/// Closes the dialog `id` of `dialogs`, if it is still the last one.
fn close(dialogs: &Entity<Dialogs>, id: u64, window: &mut Window, cx: &mut App) {
    dialogs.update(cx, |dialogs, cx| {
        if dialogs.shown.last().is_some_and(|opened| opened.id == id) {
            dialogs.close(window, cx);
        }
    });
}

/// What a button of the dialog `id` does: its answer, and the dialog closes
/// if the answer says so. Without an answer it closes.
fn answered(
    dialogs: &Entity<Dialogs>,
    id: u64,
    answer: Option<Answer>,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let dialogs = dialogs.clone();

    move |window, cx| {
        let closes = answer.as_ref().is_none_or(|answer| answer(window, cx));

        if closes {
            close(&dialogs, id, window, cx);
        }
    }
}

impl RenderOnce for DialogLayer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let shown: Vec<(u64, Build)> = self
            .dialogs
            .read(cx)
            .shown
            .iter()
            .map(|opened| (opened.id, opened.build.clone()))
            .collect();

        let mut layer = div();

        for (id, build) in shown {
            let dialog = build(Dialog::new(), window, cx);
            let ok = dialog.ok.as_ref().map(|(_, answer)| answer.clone());
            let cancel = dialog.cancel.as_ref().map(|(_, answer)| answer.clone());

            let footer = dialog.footer.or_else(|| {
                let (ok_label, ok_answer) = dialog.ok.clone()?;
                let cancel_label = dialog
                    .cancel
                    .as_ref()
                    .map(|(label, _)| label.clone())
                    .unwrap_or_else(|| "Cancel".into());
                let giving_up = answered(&self.dialogs, id, cancel.clone());
                let doing = answered(&self.dialogs, id, Some(ok_answer));

                let action = Button::new(("dialog-ok", id))
                    .label(ok_label)
                    .on_click(move |_, window, cx| doing(window, cx));
                let action = match dialog.danger {
                    true => action.danger(),
                    false => action.primary(),
                };

                Some(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new(("dialog-cancel", id))
                                .outline()
                                .label(cancel_label)
                                .on_click(move |_, window, cx| giving_up(window, cx)),
                        )
                        .child(action)
                        .into_any_element(),
                )
            });

            let mut floating = FloatingWindow::new(("dialog", id), dialog.title)
                .modal(true)
                .width(dialog.width)
                .close_on_backdrop(dialog.close_on_backdrop)
                .on_close(answered(&self.dialogs, id, cancel))
                .children(dialog.children);

            if let Some(footer) = footer {
                floating = floating.footer(footer);
            }

            if let Some(focus) = dialog.initial_focus {
                floating = floating.initial_focus(focus);
            }

            if ok.is_some() {
                floating = floating.on_enter(answered(&self.dialogs, id, ok));
            }

            layer = layer.child(floating);
        }

        layer
    }
}

/// A dialog's buttons and keys, in a window GPUI keeps in the test's own
/// process.
#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{
        AppContext as _, Context, Entity, Focusable as _, IntoElement, KeyDownEvent, KeyUpEvent,
        Keystroke, ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext,
        Window, div,
    };

    use super::{DialogLayer, Dialogs};
    use crate::input::Input;
    use crate::text::InputState;

    /// A view that keeps its dialogs, and what they told.
    struct Host {
        dialogs: Entity<Dialogs>,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(DialogLayer::new(&self.dialogs))
        }
    }

    type Told = Rc<RefCell<Vec<&'static str>>>;

    /// A dialog that asks for a name in a field, opened over a host.
    fn asked(
        cx: &mut TestAppContext,
    ) -> (
        Entity<Dialogs>,
        Entity<InputState>,
        Told,
        &mut VisualTestContext,
    ) {
        cx.update(crate::theme::init);
        cx.update(crate::text::bind_keys);
        cx.update(crate::overlay::bind_keys);

        let (host, cx) = cx.add_window_view(|_, cx| Host {
            dialogs: cx.new(|_| Dialogs::new()),
        });
        let dialogs = host.read_with(cx, |host, _| host.dialogs.clone());
        let field = cx.update(|window, cx| cx.new(|cx| InputState::new(window, cx)));
        let told: Told = Rc::new(RefCell::new(Vec::new()));

        let shown = field.clone();
        let (answering, declining) = (told.clone(), told.clone());

        cx.update(|window, cx| {
            dialogs.update(cx, |dialogs, cx| {
                dialogs.open(
                    move |dialog, _, cx| {
                        let (answering, declining) = (answering.clone(), declining.clone());

                        dialog
                            .title("Name")
                            .child(Input::new(&shown))
                            .initial_focus(shown.focus_handle(cx))
                            .on_ok("Answer", move |_, _| {
                                answering.borrow_mut().push("answered");
                                true
                            })
                            .on_cancel("Give Up", move |_, _| {
                                declining.borrow_mut().push("gave up");
                                true
                            })
                    },
                    window,
                    cx,
                )
            })
        });
        cx.run_until_parked();

        (dialogs, field, told, cx)
    }

    fn press(cx: &mut VisualTestContext, key: &'static str) {
        let keystroke = Keystroke::parse(key).unwrap();

        cx.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke });
    }

    #[gpui::test]
    fn enter_in_its_field_answers_and_closes_it(cx: &mut TestAppContext) {
        let (dialogs, _, told, cx) = asked(cx);

        cx.simulate_input("toy");
        press(cx, "enter");

        assert_eq!(*told.borrow(), ["answered"]);
        assert!(!dialogs.read_with(cx, |dialogs, _| dialogs.is_open()));
    }

    #[gpui::test]
    fn enter_on_a_button_presses_that_button(cx: &mut TestAppContext) {
        let (dialogs, _, told, cx) = asked(cx);

        // From the field, Tab reaches Give Up first.
        cx.update(|window, cx| window.focus_next(cx));
        press(cx, "enter");

        assert_eq!(*told.borrow(), ["gave up"]);
        assert!(!dialogs.read_with(cx, |dialogs, _| dialogs.is_open()));
    }

    #[gpui::test]
    fn escape_gives_up(cx: &mut TestAppContext) {
        let (dialogs, _, told, cx) = asked(cx);

        press(cx, "escape");

        assert_eq!(*told.borrow(), ["gave up"]);
        assert!(!dialogs.read_with(cx, |dialogs, _| dialogs.is_open()));
    }
}
