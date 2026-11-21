//! The column beside the work: a head, groups of rows under their names,
//! and a foot. A row has its icon and name, is shown or not, does what it
//! is pressed for, or holds rows of its own that open and close under it
//! when it is pressed, has something at its end, and a menu of its own.
//! Tab reaches each row; Enter or Space presses it, Right opens what it
//! holds and Left closes it, and Shift-F10 opens its menu. The groups
//! scroll between the head and the foot.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement, KeyBinding,
    MouseButton, ParentElement as _, Refineable as _, RenderOnce, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, actions, div,
    prelude::FluentBuilder as _,
};

use crate::child_id;
use crate::icon::{Icon, IconName};
use crate::menu::{ContextMenu, MenuItem};
use crate::root::{h_flex, v_flex};
use crate::scroll::Scrollbar;
use crate::size::{Sizable as _, rems_from_px};
use crate::theme::ActiveTheme as _;

actions!(sidebar, [Open, Close]);

/// A row's key context.
const CONTEXT: &str = "SidebarItem";

pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(CONTEXT);

    cx.bind_keys([
        KeyBinding::new("right", Open, context),
        KeyBinding::new("left", Close, context),
    ]);
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type Items = Rc<dyn Fn(&mut Window, &mut App) -> Vec<MenuItem>>;

/// One row of a sidebar, known by its id.
pub struct SidebarItem {
    id: ElementId,
    label: SharedString,
    icon: Option<IconName>,
    active: bool,
    disabled: bool,
    on_click: Option<Handler>,
    suffix: Option<AnyElement>,
    menu: Option<Items>,
    children: Vec<SidebarItem>,
    default_open: bool,
}

impl SidebarItem {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        SidebarItem {
            id: id.into(),
            label: label.into(),
            icon: None,
            active: false,
            disabled: false,
            on_click: None,
            suffix: None,
            menu: None,
            children: Vec::new(),
            default_open: false,
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Whether it is what is shown.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// What pressing it does while it holds no rows; holding some, it
    /// opens and closes them instead.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// What stands at its end, such as a mark or a button of its own,
    /// whose presses are not the row's.
    pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
        self.suffix = Some(suffix.into_any_element());
        self
    }

    /// Its menu, built while it is open.
    pub fn context_menu(
        mut self,
        items: impl Fn(&mut Window, &mut App) -> Vec<MenuItem> + 'static,
    ) -> Self {
        self.menu = Some(Rc::new(items));
        self
    }

    /// A row it holds, under it while it is open.
    pub fn child(mut self, item: SidebarItem) -> Self {
        self.children.push(item);
        self
    }

    pub fn children(mut self, items: impl IntoIterator<Item = SidebarItem>) -> Self {
        self.children.extend(items);
        self
    }

    /// Whether what it holds shows until it is closed.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    fn render(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let theme = cx.theme();
        let colors = theme.colors;
        let radius = theme.radius;
        let ring = theme.focus_ring();
        let holds = !self.children.is_empty();
        let default_open = self.default_open;

        let open = window.use_keyed_state(child_id(&self.id, "open"), cx, |_, _| default_open);
        let is_open = holds && *open.read(cx);
        let hoverable = !self.active && !self.disabled;

        let caret = holds.then(|| {
            let toggled = open.clone();

            div()
                .id(child_id(&self.id, "caret"))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(rems_from_px(20.))
                .rounded(radius)
                .text_color(colors.muted_foreground)
                .hover(move |style| style.bg(colors.sidebar_accent))
                // Opening is not the row's press.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    toggled.update(cx, |open, cx| {
                        *open = !*open;
                        cx.notify();
                    });
                })
                .child(
                    Icon::new(match is_open {
                        true => IconName::ChevronDown,
                        false => IconName::ChevronRight,
                    })
                    .small(),
                )
        });

        let suffix = self.suffix.map(|suffix| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(suffix)
        });

        let opening = open.clone();
        let closing = open.clone();
        let pressed = open.clone();
        let named = self.label.clone();

        let mut row = h_flex()
            .id(self.id.clone())
            .debug_selector(move || format!("sidebar-{named}"))
            .h(rems_from_px(28.))
            .w_full()
            .px_2()
            .gap_2()
            .rounded(radius)
            .text_sm()
            .text_color(colors.foreground)
            .when_some(self.icon, |row, icon| {
                row.child(Icon::new(icon).small().color(colors.muted_foreground))
            })
            .child(div().flex_1().min_w_0().truncate().child(self.label))
            .children(suffix)
            .children(caret)
            .when(self.active, |row| {
                row.bg(colors.sidebar_accent)
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(hoverable, |row| {
                row.hover(move |style| style.bg(colors.sidebar_accent.opacity(0.8)))
            })
            .key_context(CONTEXT)
            .on_action(move |_: &Open, _, cx| {
                opening.update(cx, |open, cx| {
                    *open = true;
                    cx.notify();
                })
            })
            .on_action(move |_: &Close, _, cx| {
                closing.update(cx, |open, cx| {
                    *open = false;
                    cx.notify();
                })
            });

        row = match self.disabled {
            true => row.text_color(colors.muted_foreground),
            false => {
                let ring = ring.clone();

                let row = row
                    .cursor_pointer()
                    .tab_index(0)
                    .focus_visible(move |style| style.shadow(ring.clone()));

                match holds {
                    true => row.on_click(move |_, _, cx| {
                        pressed.update(cx, |open, cx| {
                            *open = !*open;
                            cx.notify();
                        })
                    }),
                    false => row.when_some(self.on_click, |row, on_click| {
                        row.on_click(move |_, window, cx| on_click(window, cx))
                    }),
                }
            }
        };

        let row = match self.menu {
            Some(items) => ContextMenu::new(child_id(&self.id, "menu"), row)
                .items(move |window, cx| items(window, cx))
                .into_any_element(),
            None => row.into_any_element(),
        };

        let children: Vec<AnyElement> = match is_open {
            true => self
                .children
                .into_iter()
                .map(|item| item.render(window, cx))
                .collect(),
            false => Vec::new(),
        };

        v_flex()
            .w_full()
            .child(row)
            .when(!children.is_empty(), |item| {
                item.child(
                    v_flex()
                        .gap_0p5()
                        .ml(rems_from_px(14.))
                        .pl(rems_from_px(10.))
                        .py_0p5()
                        .border_l_1()
                        .border_color(colors.border)
                        .children(children),
                )
            })
            .into_any_element()
    }
}

/// Rows under a name.
pub struct SidebarGroup {
    label: SharedString,
    items: Vec<SidebarItem>,
}

impl SidebarGroup {
    pub fn new(label: impl Into<SharedString>) -> Self {
        SidebarGroup {
            label: label.into(),
            items: Vec::new(),
        }
    }

    pub fn child(mut self, item: SidebarItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn children(mut self, items: impl IntoIterator<Item = SidebarItem>) -> Self {
        self.items.extend(items);
        self
    }
}

#[derive(IntoElement)]
pub struct Sidebar {
    id: ElementId,
    header: Option<AnyElement>,
    footer: Option<AnyElement>,
    groups: Vec<SidebarGroup>,
    style: StyleRefinement,
}

impl Sidebar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Sidebar {
            id: id.into(),
            header: None,
            footer: None,
            groups: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// What stands over the groups, such as a search.
    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    /// What stands under them, such as settings.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    pub fn child(mut self, group: SidebarGroup) -> Self {
        self.groups.push(group);
        self
    }

    pub fn children(mut self, groups: impl IntoIterator<Item = SidebarGroup>) -> Self {
        self.groups.extend(groups);
        self
    }
}

impl Styled for Sidebar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors;
        let scroll = window
            .use_keyed_state(child_id(&self.id, "scroll"), cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();

        let mut groups = Vec::with_capacity(self.groups.len());

        for group in self.groups {
            let items: Vec<AnyElement> = group
                .items
                .into_iter()
                .map(|item| item.render(window, cx))
                .collect();

            groups.push(
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .h(rems_from_px(28.))
                            .px_2()
                            .flex()
                            .items_center()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors.muted_foreground)
                            .child(group.label),
                    )
                    .children(items),
            );
        }

        let mut sidebar = v_flex()
            .id(self.id.clone())
            .flex_none()
            .h_full()
            .w(rems_from_px(256.))
            .bg(colors.sidebar)
            .border_r_1()
            .border_color(colors.border)
            .when_some(self.header, |sidebar, header| {
                sidebar.child(div().flex_none().p_2().child(header))
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .id(child_id(&self.id, "groups"))
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&scroll)
                            .px_2()
                            .pb_2()
                            .gap_4()
                            .children(groups),
                    )
                    .child(Scrollbar::vertical(child_id(&self.id, "scrollbar"), scroll)),
            )
            .when_some(self.footer, |sidebar, footer| {
                sidebar.child(
                    div()
                        .flex_none()
                        .p_2()
                        .border_t_1()
                        .border_color(colors.border)
                        .child(footer),
                )
            });

        sidebar.style().refine(&self.style);
        sidebar
    }
}

/// The pointer and the keys on a sidebar, in a window GPUI keeps in the
/// test's own process.
#[cfg(test)]
mod pointer_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{
        Context, IntoElement, Modifiers, MouseButton, ParentElement as _, Render, Styled as _,
        TestAppContext, VisualTestContext, Window, div,
    };

    use super::{Sidebar, SidebarGroup, SidebarItem};
    use crate::button::Button;
    use crate::menu::MenuItem;

    type Told = Rc<RefCell<Vec<&'static str>>>;

    /// A project with two sessions, open at first, a button at its end,
    /// and a menu.
    struct Projects {
        told: Told,
    }

    impl Render for Projects {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let told = self.told.clone();
            let tell = move |what: &'static str| {
                let told = told.clone();

                move |_: &mut Window, _: &mut gpui::App| told.borrow_mut().push(what)
            };
            let started = tell("new session");

            div().size_full().child(
                Sidebar::new("sidebar").h(gpui::px(400.)).child(
                    SidebarGroup::new("Projects").child(
                        SidebarItem::new("rumbo", "rumbo")
                            .default_open(true)
                            .on_click(tell("project"))
                            .suffix(
                                Button::new("start")
                                    .label("New")
                                    .on_click(move |_, window, cx| started(window, cx)),
                            )
                            .context_menu({
                                let closed = tell("closed");

                                move |_, _| {
                                    let closed = closed.clone();

                                    vec![
                                        MenuItem::new("Close Project")
                                            .on_click(move |window, cx| closed(window, cx)),
                                    ]
                                }
                            })
                            .child(SidebarItem::new("first", "First").on_click(tell("first")))
                            .child(SidebarItem::new("second", "Second")),
                    ),
                ),
            )
        }
    }

    fn projects(cx: &mut TestAppContext) -> (Told, &mut VisualTestContext) {
        cx.update(crate::theme::init);
        cx.update(super::bind_keys);
        cx.update(crate::menu::bind_keys);

        let told: Told = Rc::new(RefCell::new(Vec::new()));
        let kept = told.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Projects { told: kept });

        (told, cx)
    }

    fn middle(
        cx: &mut VisualTestContext,
        selector: &'static str,
    ) -> Option<gpui::Point<gpui::Pixels>> {
        cx.debug_bounds(selector).map(|bounds| bounds.center())
    }

    #[gpui::test]
    fn a_row_does_what_it_is_pressed_for_and_its_button_its_own(cx: &mut TestAppContext) {
        let (told, cx) = projects(cx);
        let none = Modifiers::none();

        let first = middle(cx, "sidebar-First").unwrap();
        cx.simulate_click(first, none);

        let project = cx.debug_bounds("sidebar-rumbo").unwrap();

        // The button at the row's end: on its right, before the caret.
        let button = gpui::point(project.right() - gpui::px(48.), project.center().y);
        cx.simulate_click(button, none);

        assert_eq!(*told.borrow(), ["first", "new session"]);
    }

    #[gpui::test]
    fn a_row_that_holds_rows_only_opens_and_closes_them(cx: &mut TestAppContext) {
        let (told, cx) = projects(cx);
        let none = Modifiers::none();
        let project = middle(cx, "sidebar-rumbo").unwrap();

        cx.simulate_click(project, none);

        assert!(middle(cx, "sidebar-First").is_none());

        cx.simulate_click(project, none);

        assert!(middle(cx, "sidebar-First").is_some());

        // Nothing else was done for it.
        assert!(told.borrow().is_empty());
    }

    #[gpui::test]
    fn the_keys_close_and_open_what_a_row_holds(cx: &mut TestAppContext) {
        let (_, cx) = projects(cx);

        // The project's row, then its button.
        cx.update(|window, cx| window.focus_next(cx));
        cx.simulate_keystrokes("left");

        assert!(middle(cx, "sidebar-First").is_none());

        cx.simulate_keystrokes("right");

        assert!(middle(cx, "sidebar-First").is_some());
    }

    #[gpui::test]
    fn a_row_has_its_menu(cx: &mut TestAppContext) {
        let (told, cx) = projects(cx);
        let none = Modifiers::none();
        let project = middle(cx, "sidebar-rumbo").unwrap();

        cx.simulate_mouse_down(project, MouseButton::Right, none);
        cx.simulate_mouse_up(project, MouseButton::Right, none);

        let close = middle(cx, "menu-item-0").unwrap();
        cx.simulate_click(close, none);

        assert_eq!(*told.borrow(), ["closed"]);
    }
}
