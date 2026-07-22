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

    let tags = skrizhal_core::all_tags_with_counts(&state.borrow().entries);
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
            super::state::push_undo(&state);
            skrizhal_core::rename_tag(&mut state.borrow_mut().entries, &old_tag, &new_name);
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

fn yaml_filters() -> gio::ListStore {
    let filter = gtk4::FileFilter::new();
    filter.add_suffix("yaml");
    filter.add_suffix("yml");
    filter.set_name(Some("YAML files"));
    let filters = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    filters
}

fn remember_data_path(path: &std::path::Path) {
    let mut cfg = Config::load();
    cfg.data_path = path.to_path_buf();
    let _ = cfg.save();
}

/// Open: switch to a different, already-existing data file.
pub fn choose_data_file(
    window: &adw::ApplicationWindow,
    state: &SharedState,
    on_change: &ChangeCallback,
) {
    let file_dialog = gtk4::FileDialog::builder()
        .title("Open CV Elements File")
        .filters(&yaml_filters())
        .build();

    let state = state.clone();
    let on_change = on_change.clone();
    file_dialog.open(Some(window), None::<&gio::Cancellable>, move |result| {
        let Ok(file) = result else { return };
        let Some(path) = file.path() else { return };

        super::state::set_data_path(&state, path.clone());
        remember_data_path(&path);

        match super::state::reload(&state) {
            Ok(()) => on_change(None),
            Err(err) => {
                state.borrow_mut().load_blocked = true;
                eprintln!("skrizhal: failed to load chosen data file: {err}");
                on_change(None);
            }
        }
    });
}

/// New File: start a fresh, empty CV element database at a chosen path.
/// Doesn't touch whatever was open before — that file is left exactly as it
/// was on disk, just no longer the active one.
pub fn new_file(window: &adw::ApplicationWindow, state: &SharedState, on_change: &ChangeCallback) {
    let file_dialog = gtk4::FileDialog::builder()
        .title("New CV Elements File")
        .initial_name("cv-elements.yaml")
        .filters(&yaml_filters())
        .build();

    let state = state.clone();
    let on_change = on_change.clone();
    file_dialog.save(Some(window), None::<&gio::Cancellable>, move |result| {
        let Ok(file) = result else { return };
        let Some(path) = file.path() else { return };

        super::state::start_new_file(&state, path.clone());
        remember_data_path(&path);
        if let Err(err) = super::state::persist(&state) {
            eprintln!("skrizhal: failed to create new file: {err}");
        }
        on_change(None);
    });
}

/// Save As: keep the current entries, write them to a new path, and switch
/// to it (further changes save to the new location, not the old one).
pub fn save_as(window: &adw::ApplicationWindow, state: &SharedState, on_change: &ChangeCallback) {
    let file_dialog = gtk4::FileDialog::builder()
        .title("Save CV Elements File As")
        .initial_name("cv-elements.yaml")
        .filters(&yaml_filters())
        .build();

    let state = state.clone();
    let on_change = on_change.clone();
    file_dialog.save(Some(window), None::<&gio::Cancellable>, move |result| {
        let Ok(file) = result else { return };
        let Some(path) = file.path() else { return };

        super::state::set_data_path(&state, path.clone());
        remember_data_path(&path);
        if let Err(err) = super::state::persist(&state) {
            eprintln!("skrizhal: failed to save as new file: {err}");
        }
        on_change(None);
    });
}

/// Preferences: currently just the data file location, shown as a path with
/// a "Change…" button that opens the same picker as Open/New — pick an
/// existing file to switch to it, or type a new name to start fresh there.
pub fn show_preferences(window: &adw::ApplicationWindow, state: &SharedState, on_change: &ChangeCallback) {
    let dialog = adw::PreferencesWindow::builder()
        .transient_for(window)
        .modal(true)
        .search_enabled(false)
        .build();

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder()
        .title("Data File")
        .description("Where Skrizhal reads and saves your CV elements. Zerkalo's CV mode looks for this same file.")
        .build();

    let path_row = adw::ActionRow::builder()
        .title("Location")
        .subtitle(state.borrow().data_path.display().to_string())
        .build();
    let change_button = gtk4::Button::builder()
        .label("Change…")
        .valign(gtk4::Align::Center)
        .css_classes(["flat"])
        .build();
    path_row.add_suffix(&change_button);
    group.add(&path_row);
    page.add(&group);
    dialog.add(&page);

    {
        let window = window.clone();
        let state = state.clone();
        let on_change = on_change.clone();
        let path_row = path_row.clone();
        change_button.connect_clicked(move |_| {
            let file_dialog = gtk4::FileDialog::builder()
                .title("Choose Data File Location")
                .initial_name("cv-elements.yaml")
                .filters(&yaml_filters())
                .build();
            let state = state.clone();
            let on_change = on_change.clone();
            let path_row = path_row.clone();
            file_dialog.save(Some(&window), None::<&gio::Cancellable>, move |result| {
                let Ok(file) = result else { return };
                let Some(path) = file.path() else { return };

                remember_data_path(&path);
                if path.exists() {
                    super::state::set_data_path(&state, path.clone());
                    if let Err(err) = super::state::reload(&state) {
                        state.borrow_mut().load_blocked = true;
                        eprintln!("skrizhal: failed to load chosen data file: {err}");
                    }
                } else {
                    super::state::start_new_file(&state, path.clone());
                    if let Err(err) = super::state::persist(&state) {
                        eprintln!("skrizhal: failed to create new file: {err}");
                    }
                }
                path_row.set_subtitle(&path.display().to_string());
                on_change(None);
            });
        });
    }

    dialog.present();
}

fn bibtex_filters() -> gio::ListStore {
    let filter = gtk4::FileFilter::new();
    filter.add_suffix("bib");
    filter.add_suffix("bibtex");
    filter.set_name(Some("BibTeX files"));
    let filters = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    filters
}

/// Import publications from a `.bib` file. Always additive and always
/// confirmed first: nothing existing is modified or replaced, imported keys
/// are made unique against what's already there, and the user sees exactly
/// what will be added before anything is written.
pub fn import_bibtex(
    window: &adw::ApplicationWindow,
    state: &SharedState,
    on_change: &ChangeCallback,
) {
    let file_dialog = gtk4::FileDialog::builder()
        .title("Import from BibTeX")
        .filters(&bibtex_filters())
        .build();

    let state = state.clone();
    let on_change = on_change.clone();
    let window = window.clone();
    file_dialog.open(Some(&window.clone()), None::<&gio::Cancellable>, move |result| {
        let Ok(file) = result else { return };
        let Some(path) = file.path() else { return };

        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(err) => {
                let dialog = adw::MessageDialog::new(
                    Some(&window),
                    Some("Couldn't read that file"),
                    Some(&err.to_string()),
                );
                dialog.add_response("ok", "OK");
                dialog.present();
                return;
            }
        };

        let imported = skrizhal_core::parse_bibtex(&text, &state.borrow().entries);
        if imported.is_empty() {
            let dialog = adw::MessageDialog::new(
                Some(&window),
                Some("Nothing to import"),
                Some("No BibTeX entries were found in that file."),
            );
            dialog.add_response("ok", "OK");
            dialog.present();
            return;
        }

        let preview: Vec<String> = imported
            .iter()
            .take(8)
            .map(|e| format!("• {} ({})", e.title, e.category))
            .collect();
        let more = imported.len().saturating_sub(preview.len());
        let mut body = preview.join("\n");
        if more > 0 {
            body.push_str(&format!("\n…and {more} more"));
        }

        let dialog = adw::MessageDialog::new(
            Some(&window),
            Some(&format!(
                "Import {} {}?",
                imported.len(),
                if imported.len() == 1 { "entry" } else { "entries" }
            )),
            Some(&body),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("import", "Import");
        dialog.set_response_appearance("import", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("import"));
        dialog.set_close_response("cancel");

        let state = state.clone();
        let on_change = on_change.clone();
        dialog.connect_response(None, move |dialog, response| {
            if response == "import" {
                super::state::push_undo(&state);
                let first = imported.first().map(|e| e.key.clone());
                state.borrow_mut().entries.extend(imported.iter().cloned());
                on_change(first);
            }
            dialog.close();
        });
        dialog.present();
    });
}
