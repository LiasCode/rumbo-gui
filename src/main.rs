//! rumbo-gui is a graphical front end for rumbo. It starts `rumbo serve` in
//! a project and speaks the Agent Frontend Protocol with it.

mod config;
mod connection;
mod conversation;
mod history;
mod host;
mod launcher;
mod protocol;
mod usage;
mod workspace;

use gpui_kit::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

use crate::config::Parsed;
use crate::launcher::Launcher;

fn main() {
    let config = match config::parse(std::env::args().skip(1)) {
        Ok(Parsed::Open(config)) => config,

        Ok(Parsed::Help) => {
            print!("{}", config::USAGE);
            return;
        }

        Err(message) => {
            eprintln!("rumbo-gui: {message}\n\n{}", config::USAGE);
            std::process::exit(2);
        }
    };

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            workspace::bind_keys(cx);

            // Closing the last window ends the app, and with it the host.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let options = WindowOptions {
                app_id: Some(protocol::NAME.into()),
                titlebar: Some(TitlebarOptions {
                    title: Some("rumbo".into()),
                    ..TitlebarOptions::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(960.), px(720.)),
                    cx,
                ))),
                window_min_size: Some(size(px(480.), px(360.))),
                ..WindowOptions::default()
            };

            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| Launcher::new(config, window, cx))
            })
            .expect("the window did not open");

            cx.activate(true);
        });
}
