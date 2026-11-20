//! The colors, corners, and type every component draws with, light or dark.
//! The palette is shadcn's neutral one, which is what the window looked like
//! with GPUI Kit, so moving a view to these components keeps its look.

use gpui::{
    App, BoxShadow, Global, Hsla, Pixels, SharedString, WindowAppearance, point, px, rgb, rgba,
};

use crate::fonts::Typeface;

/// Light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

impl ThemeMode {
    /// The mode that matches how the desktop shows a window.
    pub fn of_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeMode::Dark,
            WindowAppearance::Light | WindowAppearance::VibrantLight => ThemeMode::Light,
        }
    }
}

/// What each part of a component is painted with. A `_foreground` is the
/// text and icons drawn on the color of the same name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    pub background: Hsla,
    pub foreground: Hsla,
    /// Quiet surfaces, and text that matters less.
    pub muted: Hsla,
    pub muted_foreground: Hsla,
    pub border: Hsla,
    /// The border of a field.
    pub input: Hsla,
    /// What marks the control the keyboard is on.
    pub ring: Hsla,
    pub primary: Hsla,
    pub primary_hover: Hsla,
    pub primary_active: Hsla,
    pub primary_foreground: Hsla,
    pub secondary: Hsla,
    pub secondary_hover: Hsla,
    pub secondary_active: Hsla,
    pub secondary_foreground: Hsla,
    /// What a row or a quiet button turns to under the pointer.
    pub accent: Hsla,
    pub accent_foreground: Hsla,
    pub danger: Hsla,
    pub danger_hover: Hsla,
    pub danger_active: Hsla,
    pub danger_foreground: Hsla,
    /// Surfaces that float over the rest: popovers, lists, tooltips, windows.
    pub popover: Hsla,
    pub popover_foreground: Hsla,
    /// What covers the window behind a modal window.
    pub overlay: Hsla,
    /// Text selected in a field.
    pub selection: Hsla,
    pub caret: Hsla,
    /// The track of a switch that is off.
    pub switch: Hsla,
    /// A scrollbar's thumb at rest, under the pointer, and dragged, and
    /// its track while the pointer is on it.
    pub scrollbar_thumb: Hsla,
    pub scrollbar_thumb_hover: Hsla,
    pub scrollbar_thumb_active: Hsla,
    pub scrollbar_track: Hsla,
    pub link: Hsla,
    pub link_hover: Hsla,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub mode: ThemeMode,
    pub colors: ThemeColors,
    /// The corners of controls.
    pub radius: Pixels,
    /// The corners of surfaces that float.
    pub radius_lg: Pixels,
    pub typeface: Typeface,
}

impl Global for Theme {}

/// The theme in force, as `cx.theme()`.
pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        Theme::global(self)
    }
}

impl Theme {
    pub fn light() -> Self {
        Theme {
            mode: ThemeMode::Light,
            colors: ThemeColors {
                background: hex(0xffffff),
                foreground: hex(NEUTRAL_950),
                muted: hex(NEUTRAL_100),
                muted_foreground: hex(NEUTRAL_500),
                border: hex(NEUTRAL_200),
                input: hex(NEUTRAL_200),
                ring: hex(NEUTRAL_400),
                primary: hex(NEUTRAL_900),
                primary_hover: hex(NEUTRAL_800),
                primary_active: hex(NEUTRAL_950),
                primary_foreground: hex(NEUTRAL_50),
                secondary: hex(NEUTRAL_100),
                secondary_hover: hex(NEUTRAL_200),
                secondary_active: hex(NEUTRAL_300),
                secondary_foreground: hex(NEUTRAL_900),
                accent: hex(NEUTRAL_100),
                accent_foreground: hex(NEUTRAL_900),
                danger: hex(RED_500),
                danger_hover: hex(RED_600),
                danger_active: hex(RED_700),
                danger_foreground: hex(NEUTRAL_50),
                popover: hex(0xffffff),
                popover_foreground: hex(NEUTRAL_950),
                overlay: rgba(0x00000033).into(),
                selection: rgba(0x55a0fc66).into(),
                caret: hex(NEUTRAL_950),
                switch: hex(NEUTRAL_300),
                scrollbar_thumb: rgba(0xa3a3a399).into(),
                scrollbar_thumb_hover: hex(NEUTRAL_400),
                scrollbar_thumb_active: hex(NEUTRAL_500),
                scrollbar_track: rgba(0xf5f5f5cc).into(),
                link: hex(NEUTRAL_950),
                link_hover: hex(NEUTRAL_700),
            },
            radius: px(6.),
            radius_lg: px(8.),
            typeface: Typeface::default(),
        }
    }

    pub fn dark() -> Self {
        Theme {
            mode: ThemeMode::Dark,
            colors: ThemeColors {
                background: hex(NEUTRAL_950),
                foreground: hex(NEUTRAL_50),
                muted: hex(NEUTRAL_800),
                muted_foreground: hex(NEUTRAL_400),
                border: hex(NEUTRAL_800),
                input: hex(0x2f2f2f),
                ring: hex(NEUTRAL_500),
                primary: hex(NEUTRAL_50),
                primary_hover: hex(NEUTRAL_200),
                primary_active: hex(NEUTRAL_300),
                primary_foreground: hex(NEUTRAL_900),
                secondary: hex(NEUTRAL_800),
                secondary_hover: hex(0x2e2e2e),
                secondary_active: hex(0x363636),
                secondary_foreground: hex(NEUTRAL_50),
                accent: hex(NEUTRAL_800),
                accent_foreground: hex(NEUTRAL_50),
                danger: hex(RED_600),
                danger_hover: hex(RED_500),
                danger_active: hex(RED_700),
                danger_foreground: hex(NEUTRAL_50),
                popover: hex(NEUTRAL_900),
                popover_foreground: hex(NEUTRAL_50),
                overlay: rgba(0x00000066).into(),
                selection: rgba(0x1d4ed899).into(),
                caret: hex(NEUTRAL_50),
                switch: hex(NEUTRAL_700),
                scrollbar_thumb: rgba(0x525252b3).into(),
                scrollbar_thumb_hover: hex(NEUTRAL_600),
                scrollbar_thumb_active: hex(NEUTRAL_500),
                scrollbar_track: rgba(0x171717cc).into(),
                link: hex(NEUTRAL_50),
                link_hover: hex(NEUTRAL_300),
            },
            radius: px(6.),
            radius_lg: px(8.),
            typeface: Typeface::default(),
        }
    }

    pub fn of_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Theme::light(),
            ThemeMode::Dark => Theme::dark(),
        }
    }

    /// The theme in force.
    pub fn global(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }

    /// The family text is drawn in.
    pub fn font_family(&self) -> SharedString {
        self.typeface.family()
    }

    /// Puts a mode in force, keeping the typeface, and draws every window
    /// again with it.
    pub fn change(mode: ThemeMode, cx: &mut App) {
        let typeface = cx
            .try_global::<Theme>()
            .map(|theme| theme.typeface)
            .unwrap_or_default();

        cx.set_global(Theme {
            typeface,
            ..Theme::of_mode(mode)
        });
        cx.refresh_windows();
    }

    /// Puts a typeface in force, and draws every window again with it.
    pub fn change_typeface(typeface: Typeface, cx: &mut App) {
        cx.global_mut::<Theme>().typeface = typeface;
        cx.refresh_windows();
    }

    /// The mark of the control the keyboard is on: a soft ring around it,
    /// drawn as a shadow so it takes no room and moves nothing.
    pub fn focus_ring(&self) -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: self.colors.ring.opacity(0.5),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(3.),
            inset: false,
        }]
    }

    /// The shadow of a surface that floats over the rest.
    pub fn floating_shadow(&self) -> Vec<BoxShadow> {
        let strength = match self.mode {
            ThemeMode::Light => 0.12,
            ThemeMode::Dark => 0.5,
        };

        vec![
            BoxShadow {
                color: Hsla::black().opacity(strength),
                offset: point(px(0.), px(8.)),
                blur_radius: px(24.),
                spread_radius: px(-4.),
                inset: false,
            },
            BoxShadow {
                color: Hsla::black().opacity(strength / 2.),
                offset: point(px(0.), px(2.)),
                blur_radius: px(6.),
                spread_radius: px(-2.),
                inset: false,
            },
        ]
    }
}

/// Puts the light theme in force unless one already is.
pub(crate) fn init(cx: &mut App) {
    if !cx.has_global::<Theme>() {
        cx.set_global(Theme::light());
    }
}

fn hex(color: u32) -> Hsla {
    rgb(color).into()
}

// Tailwind's neutral and red scales, which shadcn's palette is made of.
const NEUTRAL_50: u32 = 0xfafafa;
const NEUTRAL_100: u32 = 0xf5f5f5;
const NEUTRAL_200: u32 = 0xe5e5e5;
const NEUTRAL_300: u32 = 0xd4d4d4;
const NEUTRAL_400: u32 = 0xa3a3a3;
const NEUTRAL_500: u32 = 0x737373;
const NEUTRAL_600: u32 = 0x525252;
const NEUTRAL_700: u32 = 0x404040;
const NEUTRAL_800: u32 = 0x262626;
const NEUTRAL_900: u32 = 0x171717;
const NEUTRAL_950: u32 = 0x0a0a0a;
const RED_500: u32 = 0xef4444;
const RED_600: u32 = 0xdc2626;
const RED_700: u32 = 0xb91c1c;
