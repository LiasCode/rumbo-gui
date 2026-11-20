use gpui::{AnyElement, Context, ParentElement as _, SharedString, Styled as _, Window, div, px};
use ui::{ActiveTheme as _, Breadcrumb, BreadcrumbItem, IconName};

use super::{Demos, note, section, told};
use crate::Overview;

#[derive(Default)]
pub struct Demo {
    told: Vec<SharedString>,
}

/// A place that tells when it is gone to.
fn place(name: &'static str, cx: &mut Context<Overview>) -> BreadcrumbItem {
    BreadcrumbItem::new(name).on_click(cx.listener(move |this, _, _, cx| {
        note(&mut this.demos.breadcrumb.told, format!("Went to {name}"));
        cx.notify();
    }))
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let theme = cx.theme();
    let frame = div()
        .w(px(320.))
        .p_3()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.colors.border);

    let session = Breadcrumb::new("session")
        .child(place("rumbo-gui", cx))
        .child(place("Sessions", cx))
        .child(BreadcrumbItem::new(
            "Port the composer to the new components",
        ));

    let settings = Breadcrumb::new("settings")
        .child(place("Settings", cx).icon(IconName::Settings))
        .child(place("Providers", cx))
        .child(BreadcrumbItem::new("toy"));

    let long = frame.child(
        Breadcrumb::new("long")
            .child(place("a-project-with-a-long-name", cx))
            .child(BreadcrumbItem::new(
                "Find why the sidebar forgets its width after a restart",
            )),
    );

    vec![
        section(
            "A session",
            "Each place before the last goes there; the last is where it is. Tab reaches each place that goes somewhere, and Enter or Space goes.",
            session,
            cx,
        ),
        section("With an icon", "", settings, cx),
        section(
            "Cut short",
            "A name too long for the room is cut short.",
            long,
            cx,
        ),
        section("What they told", "", told(&demos.breadcrumb.told, cx), cx),
    ]
}
