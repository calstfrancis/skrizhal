use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use super::state::{ChangeCallback, SharedState};
use crate::config::Config;

pub fn show_manage_tags_dialog(
    window: &adw::ApplicationWindow,
    state: &SharedState,
    on_change: &ChangeCallback,
) {
    let dialog = adw::Window::builder()
        .transient_for(window)
        .modal(true)
        .title("Manage Tags")
        .default_width(400)
        .default_height(500)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&adw::HeaderBar::new());

    let scrolled = gtk4::ScrolledWindow::builder().vexpand(true).build();
    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    list_box.set_margin_top(12);
    list_box.set_margin_bottom(12);
    list_box.set_margin_start(12);
    list_box.set_margin_end(12);

    let tags = skrizhal::all_tags_with_counts(&state.borrow().entries);
    if tags.is_empty() {
        let placeholder = adw::ActionRow::builder()
            .title("No tags yet")
            .subtitle("Tags appear here once entries have them.")
            .build();
        list_box.append(&placeholder);
    }
    for (tag, count) in tags {
        let row = adw::EntryRow::builder()
            .title(format!("{tag} ({count}) — rename and press Enter"))
            .text(tag.clone())
            .build();
        let state = state.clone();
        let on_change = on_change.clone();
        let dialog_clone = dialog.clone();
        let old_tag = tag.clone();
        row.connect_apply(move |row| {
            let new_name = row.text().trim().to_string();
            if new_name.is_empty() || new_name == old_tag {
                return;
            }
            skrizhal::rename_tag(&mut state.borrow_mut().entries, &old_tag, &new_name);
            on_change(None);
            dialog_clone.close();
        });
        list_box.append(&row);
    }
    scrolled.set_child(Some(&list_box));
    toolbar_view.set_content(Some(&scrolled));
    dialog.set_content(Some(&toolbar_view));
    dialog.present();
}

pub fn choose_data_file(
    window: &adw::ApplicationWindow,
    state: &SharedState,
    on_change: &ChangeCallback,
) {
    let file_dialog = gtk4::FileDialog::builder()
        .title("Choose CV Elements File")
        .build();

    let filter = gtk4::FileFilter::new();
    filter.add_suffix("yaml");
    filter.add_suffix("yml");
    filter.set_name(Some("YAML files"));
    let filters = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    file_dialog.set_filters(Some(&filters));

    let state = state.clone();
    let on_change = on_change.clone();
    file_dialog.open(
        Some(window),
        None::<&gio::Cancellable>,
        move |result| {
            let Ok(file) = result else { return };
            let Some(path) = file.path() else { return };

            state.borrow_mut().data_path = path.clone();
            let _ = Config { data_path: path }.save();

            match super::state::reload(&state) {
                Ok(()) => on_change(None),
                Err(err) => {
                    state.borrow_mut().load_blocked = true;
                    eprintln!("skrizhal: failed to load chosen data file: {err}");
                    on_change(None);
                }
            }
        },
    );
}
