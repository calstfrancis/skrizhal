mod config;
mod git_backup;
mod search_helper;
mod ui;

use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;

const APP_ID: &str = "io.github.calstfrancis.Skrizhal";

fn main() -> glib::ExitCode {
    // `--search-json` is a command-line helper, not the app: answer and leave before GTK starts.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--search-json") {
        search_helper::run(&args[1..]);
        return glib::ExitCode::SUCCESS;
    }

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    // `skrizhal:KEY` (Porog's search results) selects that entry. GIO writes the URI back as
    // `skrizhal:///KEY`, so the slashes are dropped.
    app.connect_open(|app, files, _| {
        let window = existing_window(app).unwrap_or_else(|| open_window(app));
        window.present();
        for key in files.iter().filter_map(|f| {
            let uri = f.uri();
            let key = uri.strip_prefix("skrizhal:")?.trim_start_matches('/');
            (!key.is_empty()).then(|| key.to_string())
        }) {
            let _ = gtk4::prelude::WidgetExt::activate_action(
                &window,
                "win.select-key",
                Some(&key.to_variant()),
            );
        }
    });

    app.connect_activate(|app| match existing_window(app) {
        Some(window) => window.present(),
        None => {
            open_window(app);
        }
    });
    // The Porog status line is written on a worker thread; let it finish.
    app.connect_shutdown(|_| fond_status::summary::flush());
    app.run()
}

fn existing_window(app: &adw::Application) -> Option<gtk4::Window> {
    app.windows().into_iter().next()
}

fn open_window(app: &adw::Application) -> gtk4::Window {
    ui::styles::load_global_css();
    ui::app_window::build(app).upcast()
}
