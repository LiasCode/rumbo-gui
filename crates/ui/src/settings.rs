//! Settings in pages: a column with a search and a button for each page,
//! and the page shown beside it, its settings in groups. Typing in the
//! search keeps the settings whose words hold what is typed, and shows the
//! first page that has any, until another is chosen.

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Refineable as _, RenderOnce, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::button::Button;
use crate::child_id;
use crate::icon::{Icon, IconName};
use crate::input::Input;
use crate::root::{h_flex, v_flex};
use crate::scroll::Scrollbar;
use crate::size::Sizable as _;
use crate::text::{InputEvent, InputState};
use crate::theme::ActiveTheme as _;

type Render = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// One setting, drawn by what it is given, found by its words.
pub struct SettingsItem {
    render: Render,
    keywords: Vec<SharedString>,
}

impl SettingsItem {
    pub fn new<E: IntoElement>(render: impl Fn(&mut Window, &mut App) -> E + 'static) -> Self {
        SettingsItem {
            render: Rc::new(move |window, cx| render(window, cx).into_any_element()),
            keywords: Vec::new(),
        }
    }

    /// The words a search finds it by.
    pub fn keywords<S: Into<SharedString>>(mut self, words: impl IntoIterator<Item = S>) -> Self {
        self.keywords.extend(words.into_iter().map(Into::into));
        self
    }
}

#[derive(Default)]
pub struct SettingsGroup {
    title: Option<SharedString>,
    description: Option<SharedString>,
    items: Vec<SettingsItem>,
}

impl SettingsGroup {
    pub fn new() -> Self {
        SettingsGroup::default()
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn item(mut self, item: SettingsItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = SettingsItem>) -> Self {
        self.items.extend(items);
        self
    }
}

pub struct SettingsPage {
    title: SharedString,
    groups: Vec<SettingsGroup>,
}

impl SettingsPage {
    pub fn new(title: impl Into<SharedString>) -> Self {
        SettingsPage {
            title: title.into(),
            groups: Vec::new(),
        }
    }

    pub fn group(mut self, group: SettingsGroup) -> Self {
        self.groups.push(group);
        self
    }
}

/// Whether `query` finds a setting: in its words, its group's title, or
/// its page's, whatever their case. Nothing typed finds everything.
fn finds<'a>(query: &str, words: impl IntoIterator<Item = &'a str>) -> bool {
    let query = query.trim().to_lowercase();

    query.is_empty()
        || words
            .into_iter()
            .any(|word| word.to_lowercase().contains(&query))
}

/// The page shown: the one chosen, unless a search is followed or finds
/// nothing on it; then the first that has something found.
fn shown_page(chosen: usize, following: bool, found: &[bool]) -> Option<usize> {
    match found.get(chosen) {
        Some(true) if !following => Some(chosen),
        _ => found.iter().position(|found| *found),
    }
}

/// What the search holds, and which page was chosen.
struct Search {
    search: gpui::Entity<InputState>,
    chosen: usize,
    /// Whether the page shown follows what is typed, until one is chosen.
    following: bool,
    scroll: ScrollHandle,
    _typed: Subscription,
}

#[derive(IntoElement)]
pub struct Settings {
    id: ElementId,
    pages: Vec<SettingsPage>,
    style: StyleRefinement,
}

impl Settings {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Settings {
            id: id.into(),
            pages: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    pub fn page(mut self, page: SettingsPage) -> Self {
        self.pages.push(page);
        self
    }

    pub fn pages(mut self, pages: impl IntoIterator<Item = SettingsPage>) -> Self {
        self.pages.extend(pages);
        self
    }
}

impl Styled for Settings {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Settings {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(child_id(&self.id, "search"), cx, |window, cx| {
            let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings"));
            let typed = cx.subscribe(&search, |state: &mut Search, search, event, cx| {
                if *event == InputEvent::Change {
                    state.following = !search.read(cx).value().trim().is_empty();
                    cx.notify();
                }
            });

            Search {
                search,
                chosen: 0,
                following: false,
                scroll: ScrollHandle::new(),
                _typed: typed,
            }
        });

        let (search, chosen, following, scroll) = {
            let state = state.read(cx);

            (
                state.search.clone(),
                state.chosen,
                state.following,
                state.scroll.clone(),
            )
        };
        let query = search.read(cx).value().to_string();

        // Which settings of each page the search finds, by group.
        let found: Vec<Vec<Vec<bool>>> = self
            .pages
            .iter()
            .map(|page| {
                page.groups
                    .iter()
                    .map(|group| {
                        group
                            .items
                            .iter()
                            .map(|item| {
                                let words = item.keywords.iter().map(|word| word.as_ref());
                                let titles = [Some(page.title.as_ref()), group.title.as_deref()];

                                finds(&query, words.chain(titles.into_iter().flatten()))
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();
        let has_found: Vec<bool> = found
            .iter()
            .map(|groups| groups.iter().flatten().any(|found| *found))
            .collect();
        let shown = shown_page(chosen, following, &has_found);

        // What the search shows stays chosen once it is cleared.
        if let Some(shown) = shown.filter(|shown| *shown != chosen) {
            state.update(cx, |state, _| state.chosen = shown);
        }

        let theme = cx.theme();
        let colors = theme.colors;
        let radius_lg = theme.radius_lg;

        let buttons = self
            .pages
            .iter()
            .enumerate()
            .filter(|(index, _)| has_found[*index])
            .map(|(index, page)| {
                let choosing = state.clone();

                Button::new(child_id(&self.id, format!("page-{index}")))
                    .ghost()
                    .small()
                    .label(page.title.clone())
                    .selected(shown == Some(index))
                    .w_full()
                    .justify_start()
                    .on_click(move |_, _, cx| {
                        choosing.update(cx, |state, cx| {
                            state.chosen = index;
                            state.following = false;
                            state.scroll.set_offset(gpui::Point::default());
                            cx.notify();
                        })
                    })
            });

        let pages = v_flex()
            .flex_none()
            .w(px(220.))
            .h_full()
            .p_3()
            .gap_1()
            .border_r_1()
            .border_color(colors.border)
            .child(
                div().pb_2().child(
                    Input::new(&search)
                        .small()
                        .cleanable(true)
                        .prefix(Icon::new(IconName::Search).small()),
                ),
            )
            .children(buttons);

        let body: AnyElement = match shown {
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(colors.muted_foreground)
                .child("No settings match")
                .into_any_element(),

            Some(shown) => {
                let page = self.pages.into_iter().nth(shown).expect("a page shown");
                let found = &found[shown];

                let groups = page
                    .groups
                    .into_iter()
                    .zip(found)
                    .filter(|(_, found)| found.iter().any(|found| *found))
                    .map(|(group, found)| {
                        let items = group
                            .items
                            .into_iter()
                            .zip(found)
                            .filter(|(_, found)| **found)
                            .enumerate()
                            .map(|(index, (item, _))| {
                                div()
                                    .px_4()
                                    .py_3()
                                    .when(index > 0, |item| {
                                        item.border_t_1().border_color(colors.border)
                                    })
                                    .child((item.render)(window, cx))
                            });

                        v_flex()
                            .gap_2()
                            .when_some(group.title, |column, title| {
                                column.child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(title),
                                )
                            })
                            .when_some(group.description, |column, description| {
                                column.child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.muted_foreground)
                                        .child(description),
                                )
                            })
                            .child(
                                v_flex()
                                    .rounded(radius_lg)
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.background)
                                    .children(items),
                            )
                    });

                div()
                    .relative()
                    .size_full()
                    .child(
                        div()
                            .id(child_id(&self.id, "page"))
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&scroll)
                            .child(
                                v_flex()
                                    .w_full()
                                    .max_w(px(760.))
                                    .mx_auto()
                                    .p_6()
                                    .gap_6()
                                    .child(
                                        div()
                                            .text_xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(page.title),
                                    )
                                    .children(groups),
                            ),
                    )
                    .child(Scrollbar::vertical(
                        child_id(&self.id, "scrollbar"),
                        scroll.clone(),
                    ))
                    .into_any_element()
            }
        };

        let mut settings = h_flex()
            .size_full()
            .items_start()
            .child(pages)
            .child(div().flex_1().min_w_0().h_full().child(body));

        settings.style().refine(&self.style);
        settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_setting_by_any_of_its_words() {
        assert!(finds("", ["theme"]));
        assert!(finds("DARK", ["theme", "dark"]));
        assert!(finds(" wid ", ["Conversation width"]));
        assert!(!finds("zed", ["theme", "dark"]));
        assert!(!finds("zed", []));
    }

    #[test]
    fn shows_the_chosen_page_or_the_first_found() {
        // Nothing typed finds every page: the chosen one shows.
        assert_eq!(shown_page(2, false, &[true, true, true]), Some(2));

        // Typing shows the first page found, until one is chosen.
        assert_eq!(shown_page(2, true, &[false, true, true]), Some(1));
        assert_eq!(shown_page(2, false, &[false, true, true]), Some(2));

        // A chosen page with nothing found gives way to the first found.
        assert_eq!(shown_page(0, false, &[false, false, true]), Some(2));
        assert_eq!(shown_page(0, true, &[false, false, false]), None);
    }
}
