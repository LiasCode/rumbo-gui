//! Where something is, as the places that lead to it: each place before the
//! last can be pressed to go there, and the last is where it is. Tab
//! reaches each place that can be pressed, and Enter or Space presses it.
//! A name too long for the room is cut short.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    Refineable as _, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, Window, div, prelude::FluentBuilder as _,
};

use crate::button::ClickHandler;
use crate::child_id;
use crate::icon::{Icon, IconName};
use crate::root::h_flex;
use crate::size::Sizable as _;
use crate::theme::ActiveTheme as _;

/// One place of a breadcrumb.
pub struct BreadcrumbItem {
    label: SharedString,
    icon: Option<IconName>,
    on_click: Option<ClickHandler>,
}

impl BreadcrumbItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        BreadcrumbItem {
            label: label.into(),
            icon: None,
            on_click: None,
        }
    }

    /// An icon before its name.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// What going there does. The last place is where it is, and is not
    /// pressed.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

#[derive(IntoElement)]
pub struct Breadcrumb {
    id: ElementId,
    items: Vec<BreadcrumbItem>,
    style: StyleRefinement,
}

impl Breadcrumb {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Breadcrumb {
            id: id.into(),
            items: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    pub fn child(mut self, item: BreadcrumbItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn children(mut self, items: impl IntoIterator<Item = BreadcrumbItem>) -> Self {
        self.items.extend(items);
        self
    }
}

impl Styled for Breadcrumb {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Breadcrumb {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = theme.colors;
        let radius = theme.radius;
        let ring = theme.focus_ring();
        let last = self.items.len().saturating_sub(1);

        let mut trail = h_flex()
            .min_w_0()
            .gap_1p5()
            .text_sm()
            .text_color(colors.muted_foreground);

        for (index, item) in self.items.into_iter().enumerate() {
            if index > 0 {
                trail = trail.child(
                    Icon::new(IconName::ChevronRight)
                        .small()
                        .color(colors.muted_foreground),
                );
            }

            let place = h_flex()
                .id(child_id(&self.id, format!("place-{index}")))
                .min_w_0()
                .gap_1p5()
                .rounded(radius)
                .when_some(item.icon, |place, icon| {
                    place.child(Icon::new(icon).small())
                })
                .child(div().min_w_0().truncate().child(item.label));

            let place = match (index == last, item.on_click) {
                (true, _) => place.text_color(colors.foreground),

                (false, Some(on_click)) => {
                    let ring = ring.clone();

                    place
                        .cursor_pointer()
                        .hover(move |style| style.text_color(colors.foreground))
                        .tab_index(0)
                        .focus_visible(move |style| style.shadow(ring.clone()))
                        .on_click(move |event, window, cx| on_click(event, window, cx))
                }

                (false, None) => place,
            };

            trail = trail.child(place);
        }

        trail.style().refine(&self.style);
        trail
    }
}
