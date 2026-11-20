//! A message that stays in the page, beside what it is about: news, a
//! success, a warning, or a problem, with an icon and a color for each, a
//! title, words under it, and what can be done about it. One that can be
//! put away has a button that does.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontWeight, IntoElement, ParentElement,
    Refineable as _, RenderOnce, SharedString, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _,
};

use crate::button::{Button, ClickHandler};
use crate::child_id;
use crate::icon::{Icon, IconName};
use crate::root::{h_flex, v_flex};
use crate::size::{Sizable as _, Size};
use crate::theme::ActiveTheme as _;
use crate::tone::Tone;

#[derive(IntoElement)]
pub struct Alert {
    id: ElementId,
    tone: Tone,
    title: Option<SharedString>,
    description: Option<SharedString>,
    children: Vec<AnyElement>,
    on_close: Option<ClickHandler>,
    style: StyleRefinement,
}

impl Alert {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Alert {
            id: id.into(),
            tone: Tone::default(),
            title: None,
            description: None,
            children: Vec::new(),
            on_close: None,
            style: StyleRefinement::default(),
        }
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

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// What it says under the title.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A button that puts it away, which calls `handler`; the caller stops
    /// drawing it.
    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

/// What it holds under its words, such as the buttons that do something
/// about it.
impl ParentElement for Alert {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Alert {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Alert {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = theme.colors;
        let color = self.tone.color(&colors);

        // A tone other than news tints the alert with its color.
        let (background, border) = match self.tone {
            Tone::Info => (colors.background, colors.border),
            _ => (
                colors.background.blend(color.opacity(0.06)),
                colors.background.blend(color.opacity(0.35)),
            ),
        };

        // The icon's middle lines up with the first line's.
        let lead = (Size::Medium.line_height() - Size::Medium.icon()) / 2.;

        let words = v_flex()
            .flex_1()
            .min_w_0()
            .gap_1()
            .text_sm()
            .when_some(self.title, |words, title| {
                words.child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(color)
                        .child(title),
                )
            })
            .when_some(self.description, |words, description| {
                words.child(div().text_color(colors.muted_foreground).child(description))
            })
            .when(!self.children.is_empty(), |words| {
                words.child(h_flex().pt_2().gap_2().children(self.children))
            });

        let mut alert = h_flex()
            .items_start()
            .w_full()
            .gap_3()
            .px_4()
            .py_3()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(border)
            .bg(background)
            .child(Icon::new(self.tone.icon()).color(color).mt(lead))
            .child(words)
            .when_some(self.on_close, |alert, on_close| {
                alert.child(
                    Button::new(child_id(&self.id, "close"))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .tooltip("Dismiss")
                        .on_click(move |event, window, cx| on_close(event, window, cx)),
                )
            });

        alert.style().refine(&self.style);
        alert
    }
}
