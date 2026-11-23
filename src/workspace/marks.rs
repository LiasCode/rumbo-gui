//! Message shortcuts drawn as a compact rail. The dashes move, but their
//! buttons do not, so the pointer can move along the rail without chasing it.

use gpui::{
    AnimationExt as _, App, ElementId, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, SpringAnimation, SpringConfig, StatefulInteractiveElement as _,
    Styled as _, Window, div,
};
use ui::{ActiveTheme as _, Button, MessageListState, Sizable as _, rems_from_px, v_flex};

use crate::conversation::MessageMark;

const SPRING: SpringConfig = SpringConfig::new(560., 40., 1.);

/// The dash under the pointer extends most, with a falloff to either side.
fn extension(index: usize, hovered: Option<usize>) -> f32 {
    match hovered.map(|hovered| index.abs_diff(hovered)) {
        Some(0) => 1.,
        Some(1) => 0.65,
        Some(2) => 0.28,
        _ => 0.,
    }
}

pub(super) fn rail(
    session: &str,
    marks: &[MessageMark],
    scroller: &Entity<MessageListState>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id = ElementId::Name(format!("message-history-{session}").into());
    let hover = window.use_keyed_state((id.clone(), "hover"), cx, |_, _| None::<usize>);
    let hovered = *hover.read(cx);
    let colors = cx.theme().colors;
    let width = rems_from_px(34.);
    let height = rems_from_px(9.);

    v_flex().flex_none().h_full().justify_center().pr_1().child(
        v_flex()
            .id(id)
            .max_h_full()
            .overflow_y_scroll()
            .children(marks.iter().enumerate().map(|(index, mark)| {
                let hover = hover.clone();
                let scroller = scroller.clone();
                let row = mark.row;
                let target = extension(index, hovered);
                let dash = div()
                    .absolute()
                    .right(rems_from_px(2.))
                    .top(rems_from_px(3.5))
                    .h(rems_from_px(2.))
                    .rounded_full()
                    .with_spring(
                        "extension",
                        SpringAnimation::new(SPRING).to(target),
                        move |dash, value| {
                            let value = value.clamp(0., 1.);

                            dash.w(rems_from_px(8. + 22. * value)).bg(if target == 1. {
                                colors.foreground
                            } else {
                                colors.muted_foreground
                            })
                        },
                    );

                div()
                    .id(("mark", mark.id))
                    .relative()
                    .flex_none()
                    .w(width)
                    .h(height)
                    .on_hover(move |inside, _, cx| {
                        hover.update(cx, |hovered, cx| {
                            let next = if *inside {
                                Some(index)
                            } else if *hovered == Some(index) {
                                None
                            } else {
                                *hovered
                            };

                            if *hovered != next {
                                *hovered = next;
                                cx.notify();
                            }
                        });
                    })
                    .child(
                        Button::new("jump")
                            .link()
                            .xsmall()
                            .w_full()
                            .h_full()
                            .min_h_0()
                            .p_0()
                            .tooltip(format!("Message {}: {}", index + 1, mark.preview))
                            .on_click(move |_, _, cx| {
                                scroller.update(cx, |state, cx| state.jump_to(row, cx));
                            }),
                    )
                    .child(dash)
            })),
    )
}

#[cfg(test)]
mod tests {
    use super::extension;

    #[test]
    fn extends_the_hovered_dash_and_falls_off_on_both_sides() {
        let wave = (0..7)
            .map(|index| extension(index, Some(3)))
            .collect::<Vec<_>>();

        assert_eq!(wave, [0., 0.28, 0.65, 1., 0.65, 0.28, 0.]);
        assert_eq!(extension(3, None), 0.);
    }
}
