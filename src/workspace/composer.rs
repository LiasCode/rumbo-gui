//! The composer: a card at the foot of the conversation where the person
//! writes a task, or, while a run works, something for it to read; with the
//! model the session uses, which opens the picker, and the buttons that
//! send and cancel.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{MoveDown, MoveUp, Textarea},
    v_flex,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    div, prelude::FluentBuilder as _, px,
};

use super::{CancelRun, Workspace, WorkspaceEvent};

impl Workspace {
    pub(super) fn render_composer(&self, cx: &mut Context<Self>) -> AnyElement {
        let column = v_flex()
            .w_full()
            .max_w(px(self.width))
            .mx_auto()
            .px_4()
            .pb_3()
            .pt_1()
            .gap_1();

        if self.ended.is_some() {
            let session = self.current.clone();

            return column
                .items_center()
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
        let muted = cx.theme().muted_foreground;

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
            .icon(Lucide::Square)
            .tooltip("Cancel the run (Escape)")
            .on_click(cx.listener(|this, _, window, cx| this.cancel(&CancelRun, window, cx)));

        let model = Button::new("model")
            .ghost()
            .xsmall()
            .icon(Lucide::Cpu)
            .label(if self.model.is_empty() {
                self.provider.clone()
            } else {
                format!("{} · {}", self.provider, self.model)
            })
            .tooltip("The model and the options of this session")
            .on_click(cx.listener(|this, _, _, cx| this.open_picker(cx)));

        let hint = if working {
            "Enter tells the run · Escape cancels it"
        } else {
            "Enter sends · Shift-Enter starts a line · Up brings back what was asked"
        };

        let card = v_flex()
            .w_full()
            .rounded_xl()
            .border_1()
            .border_color(cx.theme().input)
            .bg(cx.theme().background)
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
            .child(Textarea::new(&self.composer).appearance(false))
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(model)
                    .child(div().flex_1())
                    .when(working, |row| row.child(cancel))
                    .child(send),
            );

        column
            .child(card)
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_xs()
                    .text_color(muted)
                    .child(hint),
            )
            .into_any_element()
    }
}
