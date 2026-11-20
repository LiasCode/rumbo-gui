//! Toasts: short messages in a corner of the window, over everything, that
//! say what happened elsewhere or once it is done, with a button that does
//! something about it if there is one. Each goes after a while unless it
//! stays until put away, and none goes while the pointer is on them.
//!
//! A window has one `Toasts`, which its view keeps and draws with a
//! `Toaster` in its root; anything that has it pushes to it.

mod queue;

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, ClickEvent, Context, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, deferred, div, ease_out_quint,
    prelude::FluentBuilder as _, px,
};

use queue::Queue;
pub use queue::ToastId;

use crate::button::{Button, ClickHandler};
use crate::icon::{Icon, IconName};
use crate::root::{h_flex, v_flex};
use crate::size::{Sizable as _, Size};
use crate::theme::ActiveTheme as _;
use crate::tone::Tone;

/// How long a toast shows unless told.
const SHOWN: Duration = Duration::from_secs(5);

/// How long a toast takes to come in.
const ARRIVAL: Duration = Duration::from_millis(200);

const WIDTH: Pixels = px(360.);

/// How far the toasts stand from the window's corner.
const MARGIN: Pixels = px(16.);

/// Over every other surface that floats, a select's list included.
const LAYER: usize = 4;

/// One message, to push to a window's `Toasts`.
pub struct Toast {
    title: SharedString,
    description: Option<SharedString>,
    tone: Tone,
    duration: Option<Duration>,
    action: Option<(SharedString, ClickHandler)>,
}

impl Toast {
    /// News that goes after five seconds.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Toast {
            title: title.into(),
            description: None,
            tone: Tone::default(),
            duration: Some(SHOWN),
            action: None,
        }
    }

    /// What it says under the title.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn success(self) -> Self {
        self.tone(Tone::Success)
    }

    pub fn warning(self) -> Self {
        self.tone(Tone::Warning)
    }

    pub fn danger(self) -> Self {
        self.tone(Tone::Danger)
    }

    /// How long it shows.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// It stays until it is put away.
    pub fn persistent(mut self) -> Self {
        self.duration = None;
        self
    }

    /// A button that does something about it; pressing it puts the toast
    /// away.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }
}

/// The toasts a window shows.
pub struct Toasts {
    queue: Queue<Toast>,
}

impl Default for Toasts {
    fn default() -> Self {
        Toasts::new()
    }
}

impl Toasts {
    pub fn new() -> Self {
        Toasts {
            queue: Queue::new(),
        }
    }

    /// Shows `toast` under the others, and lets it go when its time is up.
    pub fn push(&mut self, toast: Toast, cx: &mut Context<Self>) -> ToastId {
        let duration = toast.duration;
        let id = self.queue.push(toast);

        if let Some(duration) = duration {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(duration).await;

                this.update(cx, |toasts, cx| {
                    if toasts.queue.expire(id) {
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }

        cx.notify();
        id
    }

    /// Takes the toast `id` away, if it is still shown.
    pub fn dismiss(&mut self, id: ToastId, cx: &mut Context<Self>) {
        if self.queue.dismiss(id) {
            cx.notify();
        }
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.queue.clear();
        cx.notify();
    }

    fn hold(&mut self, held: bool, cx: &mut Context<Self>) {
        if self.queue.hold(held) {
            cx.notify();
        }
    }
}

/// Where a window's toasts show: its bottom right corner, over everything.
/// It goes last in the root of the window's view.
#[derive(IntoElement)]
pub struct Toaster {
    toasts: Entity<Toasts>,
}

impl Toaster {
    pub fn new(toasts: &Entity<Toasts>) -> Self {
        Toaster {
            toasts: toasts.clone(),
        }
    }
}

impl RenderOnce for Toaster {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let entity = self.toasts;
        let shown = entity.read(cx).queue.shown();

        if shown.is_empty() {
            return div().into_any_element();
        }

        let theme = cx.theme();
        let colors = theme.colors;
        let radius = theme.radius_lg;
        let shadow = theme.floating_shadow();

        // The icon's middle lines up with the title's.
        let lead = (Size::Medium.line_height() - Size::Medium.icon()) / 2.;

        let items: Vec<_> = shown
            .iter()
            .map(|entry| {
                let id = entry.id;
                let toast = &entry.item;
                let color = toast.tone.color(&colors);
                let closed = entity.clone();

                let words = v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .text_sm()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(toast.title.clone()),
                    )
                    .when_some(toast.description.clone(), |words, description| {
                        words.child(div().text_color(colors.muted_foreground).child(description))
                    })
                    .when_some(toast.action.clone(), |words, (label, handler)| {
                        let acted = entity.clone();

                        words.child(
                            h_flex().pt_2().child(
                                Button::new(("toast-action", id.0))
                                    .outline()
                                    .small()
                                    .label(label)
                                    .on_click(move |event, window, cx| {
                                        handler(event, window, cx);
                                        acted.update(cx, |toasts, cx| toasts.dismiss(id, cx));
                                    }),
                            ),
                        )
                    });

                h_flex()
                    .items_start()
                    .w_full()
                    .gap_3()
                    .p_4()
                    .rounded(radius)
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.popover)
                    .text_color(colors.popover_foreground)
                    .shadow(shadow.clone())
                    .child(Icon::new(toast.tone.icon()).color(color).mt(lead))
                    .child(words)
                    .child(
                        Button::new(("toast-close", id.0))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip("Dismiss")
                            .on_click(move |_, _, cx| {
                                closed.update(cx, |toasts, cx| toasts.dismiss(id, cx))
                            }),
                    )
                    .with_animation(
                        ("toast-arrival", id.0),
                        Animation::new(ARRIVAL).with_easing(ease_out_quint()),
                        |toast, delta| toast.opacity(delta),
                    )
            })
            .collect();

        deferred(
            v_flex()
                .id("toaster")
                .absolute()
                .right(MARGIN)
                .bottom(MARGIN)
                .w(WIDTH)
                .gap_2()
                .on_hover(
                    window
                        .listener_for(&entity, |toasts, held: &bool, _, cx| toasts.hold(*held, cx)),
                )
                .children(items),
        )
        .with_priority(LAYER)
        .into_any_element()
    }
}

/// A toast's time, on GPUI's clock in the test's own process.
#[cfg(test)]
mod clock_tests {
    use std::time::Duration;

    use gpui::{AppContext as _, TestAppContext};

    use super::{Toast, Toasts};

    #[gpui::test]
    fn a_toast_goes_when_its_time_is_up_unless_it_stays(cx: &mut TestAppContext) {
        let toasts = cx.new(|_| Toasts::new());

        toasts.update(cx, |toasts, cx| {
            toasts.push(Toast::new("A run finished"), cx);
            toasts.push(Toast::new("Kept").persistent(), cx);
            toasts.push(Toast::new("Longer").duration(Duration::from_secs(10)), cx);
        });

        let count =
            |cx: &mut TestAppContext| toasts.read_with(cx, |toasts, _| toasts.queue.shown().len());

        cx.executor().advance_clock(Duration::from_secs(6));
        cx.run_until_parked();

        assert_eq!(count(cx), 2);

        cx.executor().advance_clock(Duration::from_secs(5));
        cx.run_until_parked();

        assert_eq!(count(cx), 1);
    }

    #[gpui::test]
    fn the_pointer_on_the_toasts_keeps_them(cx: &mut TestAppContext) {
        let toasts = cx.new(|_| Toasts::new());

        toasts.update(cx, |toasts, cx| {
            toasts.push(Toast::new("A run finished"), cx);
            toasts.hold(true, cx);
        });

        let count =
            |cx: &mut TestAppContext| toasts.read_with(cx, |toasts, _| toasts.queue.shown().len());

        cx.executor().advance_clock(Duration::from_secs(6));
        cx.run_until_parked();

        assert_eq!(count(cx), 1);

        toasts.update(cx, |toasts, cx| toasts.hold(false, cx));

        assert_eq!(count(cx), 0);
    }
}
