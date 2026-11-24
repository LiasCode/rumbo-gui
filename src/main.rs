//! rumbo-gui is a graphical front end for rumbo. It starts `rumbo serve` in
//! a project, or is started by rumbo as a registered front end, and speaks
//! the Agent Frontend Protocol with it.

mod choices;
mod commands;
mod config;
mod connection;
mod conversation;
mod editor;
mod environment;
mod export;
mod file_tree;
mod git;
mod history;
mod host;
mod navigation;
mod order;
mod outline;
mod picture;
mod preferences;
mod protocol;
mod rumbos;
mod sessions;
mod shell;
mod usage;
mod workspace;

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use ui::TitleBar;

use crate::config::Parsed;
use crate::environment::Environment;
use crate::host::Descriptors;
use crate::shell::{Preferred, Shell, Start};

fn main() {
    let start = match config::parse(std::env::args().skip(1)) {
        Ok(Parsed::Open(config)) => Start::Desktop(config, Environment::new()),

        // Taken before the window opens anything that could get their
        // numbers.
        Ok(Parsed::Attached(rumbo)) => match Descriptors::take() {
            Ok(descriptors) => Start::Attached(descriptors, rumbo),

            Err(message) => {
                eprintln!("rumbo-gui: {message}");
                std::process::exit(2);
            }
        },

        Ok(Parsed::Help) => {
            print!("{}", config::USAGE);
            return;
        }

        Err(message) => {
            eprintln!("rumbo-gui: {message}\n\n{}", config::USAGE);
            std::process::exit(2);
        }
    };

    // After the descriptors are taken, so the file cannot get their numbers.
    let path = preferences::path(
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    );
    let (preferences, problem) = match &path {
        Some(path) => preferences::load(path),
        None => Default::default(),
    };
    let preferred = Preferred {
        preferences,
        path,
        problem,
    };

    // Images of the web in a reply are fetched with it, once the person
    // asks to see them.
    gpui_platform::application()
        .with_assets(ui::Assets)
        .with_http_client(Arc::new(ui::ImageClient::new(protocol::NAME)))
        .run(move |cx| {
            // Without the bundled typefaces the window draws in the
            // desktop's font, and works the same.
            if let Err(error) = ui::init(cx) {
                eprintln!("rumbo-gui: the bundled typefaces did not load: {error}");
            }

            shell::bind_keys(cx);
            workspace::bind_keys(cx);

            // The name the desktop shows the window's notifications under.
            cx.set_app_identity(protocol::NAME, "rumbo");

            // Closing the last window ends the app, and with it the host.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            // The toolbar is the title bar, as it draws itself.
            let options = WindowOptions {
                app_id: Some(protocol::NAME.into()),
                titlebar: Some(TitlebarOptions {
                    title: Some("rumbo".into()),
                    ..TitleBar::window_options().titlebar.unwrap_or_default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1120.), px(760.)),
                    cx,
                ))),
                window_min_size: Some(size(px(640.), px(420.))),
                ..TitleBar::window_options()
            };

            cx.open_window(options, |window, cx| {
                cx.new(|cx| Shell::new(start, preferred, window, cx))
            })
            .expect("the window did not open");

            cx.activate(true);
        });
}
