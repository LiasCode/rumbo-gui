//! Every component of `ui` on a page of its own: the ways it comes, its
//! states, and what it tells, so they are looked at and changed in one
//! place before the window uses them. The theme and the typeface are
//! changed under the navigation, and every page takes them at once.
//!
//! ```sh
//! cargo run -p overview [PAGE] [--dark] [--font TYPEFACE]
//! ```
//!
//! PAGE is a page's name as the navigation shows it, and TYPEFACE a
//! typeface's, in lower case and with dashes, such as `floating-window` or
//! `shantell-sans`.

mod pages;

use std::sync::Arc;

use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Point, Render, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Window, WindowBounds, WindowOptions, div, px,
    size,
};
use ui::{
    ActiveTheme as _, Button, DialogLayer, Dialogs, Label, Scrollbar, Select, SelectEvent,
    SelectItem, SelectState, Sizable as _, StatusBar, Switch, Theme, ThemeMode, TitleBar, Toaster,
    Toasts, Typeface, WindowFrame, h_flex, v_flex,
};

use pages::{Demos, Page};

struct Overview {
    page: Page,
    /// Where the page shown is scrolled, and the list of pages.
    scroll: ScrollHandle,
    pages: ScrollHandle,
    demos: Demos,
    /// The window's toasts and dialogs, which any page can show.
    toasts: Entity<Toasts>,
    dialogs: Entity<Dialogs>,
    /// The typeface in force, chosen under the navigation.
    typeface: Entity<SelectState>,
    focus_handle: FocusHandle,
}

/// A typeface's name on the command line.
fn slug(typeface: Typeface) -> String {
    typeface.name().to_lowercase().replace(' ', "-")
}

impl Overview {
    fn new(page: Page, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // The keys reach the view before any control has them.
        focus_handle.focus(window, cx);

        let items = Typeface::ALL
            .iter()
            .map(|typeface| {
                let item = SelectItem::new(typeface.name());

                match typeface.is_loaded(cx) {
                    true => item,
                    false => item.description("did not load; the desktop's font shows instead"),
                }
            })
            .collect();
        let in_force = Typeface::ALL
            .iter()
            .position(|typeface| *typeface == cx.theme().typeface);

        let typeface = cx.new(|cx| SelectState::new(items, in_force, window, cx));

        cx.subscribe(&typeface, |_, _, event: &SelectEvent, cx| {
            let SelectEvent::Change(name) = event;

            if let Some(chosen) = Typeface::ALL
                .into_iter()
                .find(|typeface| typeface.name() == name.as_ref())
            {
                Theme::change_typeface(chosen, cx);
            }
        })
        .detach();

        Overview {
            page,
            scroll: ScrollHandle::new(),
            pages: ScrollHandle::new(),
            demos: Demos::new(window, cx),
            toasts: cx.new(|_| Toasts::new()),
            dialogs: cx.new(|_| Dialogs::new()),
            typeface,
            focus_handle,
        }
    }

    /// Puts a typeface in force, as the navigation's select does.
    pub(crate) fn use_typeface(&mut self, typeface: Typeface, cx: &mut Context<Self>) {
        Theme::change_typeface(typeface, cx);
        self.typeface.update(cx, |select, cx| {
            select.set_selected_value(typeface.name(), cx)
        });
    }

    fn render_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let dark = theme.mode == ThemeMode::Dark;

        let pages = Page::ALL.iter().map(|page| {
            let page = *page;

            Button::new(page.slug())
                .ghost()
                .small()
                .label(page.title())
                .selected(page == self.page)
                .w_full()
                .justify_start()
                .on_click(cx.listener(move |this, _, _, cx| {
                    // Another page starts at its top.
                    this.page = page;
                    this.scroll.set_offset(Point::default());
                    cx.notify();
                }))
        });

        v_flex()
            .flex_none()
            .w(px(220.))
            .h_full()
            .border_r_1()
            .border_color(theme.colors.border)
            .bg(theme.colors.muted.opacity(0.4))
            .child(
                div()
                    .pt_3()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .id("pages")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.pages)
                            .px_2()
                            .pb_2()
                            .gap_0p5()
                            .children(pages),
                    )
                    .child(Scrollbar::vertical("pages-scrollbar", self.pages.clone())),
            )
            .child(
                v_flex()
                    .p_4()
                    .gap_4()
                    .border_t_1()
                    .border_color(theme.colors.border)
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(Label::new("Typeface").small())
                            .child(Select::new(&self.typeface).small()),
                    )
                    .child(
                        Switch::new("dark")
                            .small()
                            .label("Dark")
                            .checked(dark)
                            .on_change(|dark, _, cx| {
                                let mode = match dark {
                                    true => ThemeMode::Dark,
                                    false => ThemeMode::Light,
                                };

                                Theme::change(mode, cx);
                            }),
                    ),
            )
    }
}

impl Render for Overview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sections = self.demos.render(self.page, window, cx);
        let page_content = self.render_page(sections, cx);

        let theme = cx.theme();
        let mode = match theme.mode {
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        };
        let status = StatusBar::new()
            .start(format!("{} · {}", self.page.title(), self.page.slug()))
            .end(format!("{} components", Page::ALL.len()))
            .end(format!("{mode} · {}", theme.typeface.name()));

        WindowFrame::new().child(
            ui::root(cx)
                .track_focus(&self.focus_handle)
                .flex()
                .flex_col()
                .child(
                    TitleBar::new().child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("rumbo · components"),
                    ),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .items_start()
                        .child(self.render_navigation(cx))
                        .child(
                            div()
                                .relative()
                                .flex_1()
                                .h_full()
                                .min_w_0()
                                .child(page_content)
                                .child(Scrollbar::vertical("page-scrollbar", self.scroll.clone())),
                        ),
                )
                .child(status)
                .child(DialogLayer::new(&self.dialogs))
                .child(Toaster::new(&self.toasts)),
        )
    }
}

impl Overview {
    /// The page shown, which scrolls under its scrollbar.
    fn render_page(&self, sections: Vec<AnyElement>, cx: &App) -> AnyElement {
        let muted = cx.theme().colors.muted_foreground;
        let page = self.page;

        v_flex()
            .id("page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .px_10()
            .pt_8()
            .pb_16()
            .gap_8()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(page.title()),
                    )
                    .child(div().text_sm().text_color(muted).child(page.about())),
            )
            .children(sections)
            .into_any_element()
    }
}

/// Stops with what was wrong on the command line, and what it takes.
fn refuse(problem: String) -> ! {
    let pages: Vec<&str> = Page::ALL.iter().map(|page| page.slug()).collect();
    let typefaces: Vec<String> = Typeface::ALL.into_iter().map(slug).collect();

    eprintln!(
        "overview: {problem}\nusage: overview [PAGE] [--dark] [--font TYPEFACE]\n  PAGE: {}\n  TYPEFACE: {}",
        pages.join(", "),
        typefaces.join(", ")
    );
    std::process::exit(2);
}

fn main() {
    let mut page = Page::Typography;
    let mut mode = ThemeMode::Light;
    let mut typeface = None;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dark" => mode = ThemeMode::Dark,

            "--font" => {
                let Some(name) = args.next() else {
                    refuse("--font needs a typeface".into());
                };

                match Typeface::ALL.into_iter().find(|known| slug(*known) == name) {
                    Some(named) => typeface = Some(named),
                    None => refuse(format!("there is no typeface {name}")),
                }
            }

            name => match Page::from_slug(name) {
                Some(named) => page = named,
                None => refuse(format!("there is no page {name}")),
            },
        }
    }

    gpui_platform::application()
        .with_assets(ui::Assets)
        .with_http_client(Arc::new(ui::ImageClient::new("rumbo-ui-overview")))
        .run(move |cx: &mut App| {
            if let Err(error) = ui::init(cx) {
                eprintln!("overview: the bundled typefaces did not load: {error}");
            }

            Theme::change(mode, cx);

            if let Some(typeface) = typeface {
                Theme::change_typeface(typeface, cx);
            }

            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            // The title bar is the overview's own, as the window's is.
            let options = WindowOptions {
                app_id: Some("rumbo-ui-overview".into()),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1180.), px(820.)),
                    cx,
                ))),
                ..TitleBar::window_options()
            };

            cx.open_window(options, |window, cx| {
                cx.new(|cx| Overview::new(page, window, cx))
            })
            .expect("the window did not open");

            cx.activate(true);
        });
}
