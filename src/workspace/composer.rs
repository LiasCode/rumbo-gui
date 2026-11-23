//! The composer: a card at the foot of the conversation where the person
//! writes a task, or, while a run works, something for it to read, with the
//! images pasted into it; with the model the session uses, which opens the
//! picker, how full the model's context is and the tokens spent, and the
//! buttons that send and cancel. Over it, Working turns while a run works
//! in the session shown. Once the host is gone, it says why, and starts it
//! again.

use gpui::{
    AnyElement, ClipboardEntry, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
};
use ui::input_actions::{MoveDown, MoveUp, Paste};
use ui::{
    ActiveTheme as _, Button, IconName, Sizable as _, Spinner, Textarea, Toast, Tooltip, h_flex,
    rems_from_px, v_flex,
};

use super::{CancelRun, Workspace, WorkspaceEvent};
use crate::picture::{self, Picture};

impl Workspace {
    pub(super) fn render_composer(&self, cx: &mut Context<Self>) -> AnyElement {
        let column = v_flex()
            .w_full()
            .max_w(rems_from_px(self.width))
            .mx_auto()
            .px_4()
            .pb_3()
            .pt_1()
            .gap_1();

        let muted = cx.theme().colors.muted_foreground;

        if let Some(ended) = &self.ended {
            let session = self.current.clone();

            return column
                .items_center()
                .gap_2()
                .child(
                    div()
                        .max_w_full()
                        .text_sm()
                        .text_center()
                        .text_color(muted)
                        .child(ended.clone()),
                )
                .child(
                    Button::new("start-again")
                        .track_focus(&self.start_again_focus)
                        .primary()
                        .icon(IconName::RotateCw)
                        .label("Start Again")
                        .tooltip("Start rumbo again in this session")
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(WorkspaceEvent::StartAgain {
                                session: session.clone(),
                            })
                        })),
                )
                .into_any_element();
        }

        let working = self
            .views
            .get(&self.current)
            .is_some_and(|view| view.run.is_some());

        let send = Button::new("send")
            .primary()
            .small()
            .rounded_full()
            .icon(IconName::ArrowUp)
            .tooltip(if working {
                "Tell the run (Enter)"
            } else {
                "Send the task (Enter)"
            })
            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)));

        let cancel = Button::new("cancel")
            .outline()
            .small()
            .rounded_full()
            .icon(IconName::Square)
            .tooltip("Cancel the run (Escape)")
            .on_click(cx.listener(|this, _, window, cx| this.cancel(&CancelRun, window, cx)));

        // It stays pressed while the provider is asked what can be chosen.
        let model = Button::new("model")
            .ghost()
            .xsmall()
            .icon(IconName::Cpu)
            .label(self.model_name())
            .selected(self.picking)
            .tooltip("The model and the options of this session")
            .on_click(cx.listener(|this, _, _, cx| this.open_picker(cx)));

        // Rounded to read at a glance; its tooltip has every token.
        let usage = self.views.get(&self.current).and_then(|view| {
            let shown: Vec<String> = [view.spent.context(), view.spent.tokens()]
                .into_iter()
                .flatten()
                .collect();

            Some((shown.join(" · "), view.spent.exactly()?))
        });

        let card = v_flex()
            .w_full()
            .rounded_xl()
            .border_1()
            .border_color(cx.theme().colors.input)
            .bg(cx.theme().colors.background)
            .shadow_sm()
            .px_2()
            .pt_1()
            .pb_1()
            .gap_1()
            .capture_action(cx.listener(|this, _: &MoveUp, window, cx| {
                if this.recall(-1, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &MoveDown, window, cx| {
                if this.recall(1, window, cx) {
                    cx.stop_propagation();
                }
            }))
            // An image in the clipboard is pasted here; text is left for the
            // field to paste.
            .capture_action(cx.listener(|this, _: &Paste, _, cx| {
                if this.paste_images(cx) {
                    cx.stop_propagation();
                }
            }))
            .when(!self.pasted.is_empty(), |card| {
                card.child(self.render_pasted(cx))
            })
            .child(Textarea::new(&self.composer).bare(true))
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(model)
                    .when_some(usage, |row, (shown, exactly)| {
                        row.child(
                            div()
                                .id("usage")
                                .min_w_0()
                                .px_1()
                                .truncate()
                                .text_xs()
                                .text_color(muted)
                                .child(shown)
                                .tooltip(move |_, cx| Tooltip::new(exactly.clone()).build(cx)),
                        )
                    })
                    .child(div().flex_1())
                    .when(working, |row| row.child(cancel))
                    .child(send),
            );

        // While a run works in the session shown, it says so over the card.
        let status = working.then(|| {
            h_flex()
                .px_2()
                .gap_1p5()
                .items_center()
                .text_xs()
                .text_color(muted)
                .child(Spinner::new().xsmall())
                .child("Working")
        });

        column.children(status).child(card).into_any_element()
    }

    /// The images pasted into the composer, each with a button that takes it
    /// away.
    fn render_pasted(&self, cx: &mut Context<Self>) -> AnyElement {
        let border = cx.theme().colors.border;

        h_flex()
            .flex_wrap()
            .gap_2()
            .pt_1()
            .children(self.pasted.iter().enumerate().map(|(index, image)| {
                h_flex()
                    .items_start()
                    .gap_0p5()
                    .child(image.shown(128., 64.).border_1().border_color(border))
                    .child(
                        Button::new(("unpaste", index))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip("Take the image away")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if index < this.pasted.len() {
                                    this.pasted.remove(index);
                                }

                                // Back to the composer, so Enter sends
                                // instead of pressing the next button.
                                this.focus(window, cx);
                                cx.notify();
                            })),
                    )
            }))
            .into_any_element()
    }

    /// Takes the images in the clipboard into the composer, and says whether
    /// it held any. A model takes PNG, JPEG, GIF, and WebP; another image is
    /// refused, with a word why.
    fn paste_images(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(item) = cx.read_from_clipboard() else {
            return false;
        };

        let images: Vec<_> = item
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                ClipboardEntry::Image(image) => Some(image.clone()),
                _ => None,
            })
            .collect();

        if images.is_empty() {
            return false;
        }

        let (taken, refused): (Vec<_>, Vec<_>) = images
            .into_iter()
            .partition(|image| picture::media_type(image).is_some());

        if taken.is_empty() && !refused.is_empty() {
            let said = "The clipboard's image is not a PNG, JPEG, GIF, or WebP image";

            self.toast(Toast::new(said).danger(), cx);
        }

        self.pasted.extend(taken.into_iter().map(Picture::new));
        cx.notify();

        true
    }
}
