mod config;
mod ui;

use gtk4::prelude::*;
use libadwaita as adw;

const APP_ID: &str = "io.github.calstfrancis.Skrizhal";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(ui::app_window::build);
    app.run()
}
