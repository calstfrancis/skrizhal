mod config;
mod git_backup;
mod ui;

use gtk4::prelude::*;
use libadwaita as adw;

const APP_ID: &str = "io.github.calstfrancis.Skrizhal";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        ui::styles::load_global_css();
        ui::app_window::build(app);
    });
    // The Porog status line is written on a worker thread; let it finish.
    app.connect_shutdown(|_| fond_status::summary::flush());
    app.run()
}
