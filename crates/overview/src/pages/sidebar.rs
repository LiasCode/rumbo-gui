use gpui::{
    AnyElement, AppContext as _, Context, Entity, ParentElement as _, SharedString, Styled as _,
    Window, div, px,
};
use ui::{
    ActiveTheme as _, Button, IconName, Input, InputState, MenuItem, Sidebar, SidebarGroup,
    SidebarItem, Sizable as _, Spinner, h_flex, v_flex,
};

use super::{Demos, note, section, told};
use crate::Overview;

pub struct Demo {
    /// The session shown, by its place.
    shown: (usize, usize),
    search: Entity<InputState>,
    told: Vec<SharedString>,
}

impl Demo {
    pub fn new(window: &mut Window, cx: &mut Context<Overview>) -> Self {
        Demo {
            shown: (0, 1),
            search: cx.new(|cx| InputState::new(window, cx).placeholder("Search sessions")),
            told: Vec::new(),
        }
    }
}

const PROJECTS: [(&str, [&str; 3]); 2] = [
    (
        "rumbo-gui",
        [
            "Port the composer to the new components",
            "Find why the sidebar forgets its width",
            "Review the diff before the release",
        ],
    ),
    (
        "rumbo",
        [
            "Fix the failing test in the host",
            "Explain how the protocol orders its messages",
            "Write the release notes",
        ],
    ),
];

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let colors = cx.theme().colors;
    let radius = cx.theme().radius;
    let shown = demos.sidebar.shown;
    let search = demos.sidebar.search.clone();

    let projects = PROJECTS
        .iter()
        .enumerate()
        .map(|(project, (name, sessions))| {
            let overview = cx.weak_entity();
            let menu_owner = cx.weak_entity();
            let starter = cx.weak_entity();
            let name = *name;

            SidebarItem::new(("project", project), name)
                .icon(IconName::FolderOpen)
                .default_open(true)
                .suffix(
                    Button::new(("new-session", project))
                        .ghost()
                        .xsmall()
                        .icon(IconName::SquarePen)
                        .tooltip("New session")
                        .on_click(move |_, _, cx| {
                            starter
                                .update(cx, |this, cx| {
                                    note(
                                        &mut this.demos.sidebar.told,
                                        format!("New session in {name}"),
                                    );
                                    cx.notify();
                                })
                                .ok();
                        }),
                )
                .context_menu(move |_, _| {
                    let closing = menu_owner.clone();

                    vec![
                        MenuItem::new("Open in Editor").icon(IconName::ExternalLink),
                        MenuItem::separator(),
                        MenuItem::new("Close Project")
                            .icon(IconName::Close)
                            .on_click(move |_, cx| {
                                closing
                                    .update(cx, |this, cx| {
                                        note(
                                            &mut this.demos.sidebar.told,
                                            format!("Closed {name}"),
                                        );
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    ]
                })
                .children(sessions.iter().enumerate().map(move |(session, title)| {
                    let opening = overview.clone();
                    let title = *title;
                    let item = SidebarItem::new(("session", project * 10 + session), title)
                        .active(shown == (project, session))
                        .on_click(move |_, cx| {
                            opening
                                .update(cx, |this, cx| {
                                    let demo = &mut this.demos.sidebar;

                                    demo.shown = (project, session);
                                    note(&mut demo.told, format!("Opened {title:?}"));
                                    cx.notify();
                                })
                                .ok();
                        });

                    match (project, session) {
                        (0, 0) => item.suffix(Spinner::new().xsmall()),
                        (1, 2) => item.suffix(div().size(px(7.)).rounded_full().bg(colors.primary)),
                        _ => item,
                    }
                }))
        });

    let recent = SidebarGroup::new("Recent").child(
        SidebarItem::new("notes", "notes")
            .icon(IconName::FolderClosed)
            .child(SidebarItem::new("notes-1", "Tidy the meeting notes")),
    );

    let sidebar = Sidebar::new("sidebar")
        .w(px(272.))
        .h(px(460.))
        .header(
            v_flex()
                .gap_2()
                .child(
                    Button::new("sidebar-new")
                        .outline()
                        .small()
                        .w_full()
                        .icon(IconName::SquarePen)
                        .label("New Session"),
                )
                .child(Input::new(&search).small()),
        )
        .footer(
            h_flex()
                .gap_1()
                .child(
                    Button::new("sidebar-open")
                        .ghost()
                        .small()
                        .flex_1()
                        .icon(IconName::FolderOpen)
                        .label("Open Project…"),
                )
                .child(
                    Button::new("sidebar-settings")
                        .ghost()
                        .small()
                        .icon(IconName::Settings)
                        .tooltip("Settings"),
                ),
        )
        .child(SidebarGroup::new("Projects").children(projects))
        .child(recent);

    vec![
        section(
            "Projects and their sessions",
            "Tab reaches each row; Enter or Space opens it, Right and Left open and close what it holds, and Shift-F10 or the other button opens its menu. Its groups scroll between its head and its foot.",
            div()
                .rounded(radius)
                .border_1()
                .border_color(colors.border)
                .overflow_hidden()
                .w(px(274.))
                .child(sidebar),
            cx,
        ),
        section("What they told", "", told(&demos.sidebar.told, cx), cx),
    ]
}
