use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use skrizhal::CvEntry;

use super::detail;
use super::dialogs;
use super::sidebar;
use super::state::{self, ChangeCallback, SharedState};
use crate::config::Config;

/// Selects the row whose widget name matches `key`, firing `row-selected` so
/// the detail pane follows. Returns whether a matching row was found.
fn select_row_by_key(list_box: &gtk4::ListBox, key: Option<&str>) -> bool {
    let Some(key) = key else {
        list_box.unselect_all();
        return false;
    };
    let mut child = list_box.first_child();
    while let Some(widget) = child {
        if widget.widget_name() == key {
            if let Some(row) = widget.downcast_ref::<gtk4::ListBoxRow>() {
                list_box.select_row(Some(row));
                return true;
            }
        }
        child = widget.next_sibling();
    }
    list_box.unselect_all();
    false
}

pub fn build(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Skrizhal")
        .default_width(1000)
        .default_height(650)
        .build();

    let config = Config::load();
    let (initial_state, load_error) = state::load_initial(config.data_path.clone());
    let state: SharedState = Rc::new(RefCell::new(initial_state));

    let sidebar_widgets = sidebar::build(&window);
    let detail_widgets = detail::build();
    let toast_overlay = adw::ToastOverlay::new();

    let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
    paned.set_start_child(Some(&sidebar_widgets.root));
    paned.set_end_child(Some(&detail_widgets.outer_stack));
    paned.set_resize_start_child(false);
    paned.set_shrink_start_child(false);
    paned.set_position(320);
    toast_overlay.set_child(Some(&paned));

    // ── Header bar ───────────────────────────────────────────────────
    let header = adw::HeaderBar::new();

    let sidebar_toggle = gtk4::ToggleButton::builder()
        .icon_name("sidebar-show-symbolic")
        .active(true)
        .build();
    {
        let sidebar_root = sidebar_widgets.root.clone();
        sidebar_toggle.connect_toggled(move |btn| sidebar_root.set_visible(btn.is_active()));
    }
    header.pack_start(&sidebar_toggle);
    header.pack_start(&sidebar_widgets.add_button);

    let menu_button = gtk4::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .build();
    let menu_popover = gtk4::Popover::new();
    let menu_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let manage_tags_btn = gtk4::Button::builder()
        .label("Manage Tags…")
        .css_classes(["flat"])
        .build();
    let choose_file_btn = gtk4::Button::builder()
        .label("Choose Data File…")
        .css_classes(["flat"])
        .build();
    let reload_btn = gtk4::Button::builder()
        .label("Reload from Disk")
        .css_classes(["flat"])
        .build();
    menu_box.append(&manage_tags_btn);
    menu_box.append(&choose_file_btn);
    menu_box.append(&reload_btn);
    menu_popover.set_child(Some(&menu_box));
    menu_button.set_popover(Some(&menu_popover));
    header.pack_end(&menu_button);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&toast_overlay));
    window.set_content(Some(&toolbar_view));

    // ── on_change: the only path that mutates + persists + refreshes ───
    // Built via a shared cell so refresh_list can hand a clone of it to
    // each row's Duplicate/Delete buttons — see state::ChangeCallback.
    let on_change_cell: Rc<RefCell<Option<ChangeCallback>>> = Rc::new(RefCell::new(None));
    let on_change: ChangeCallback = {
        let state = state.clone();
        let sidebar_widgets = sidebar_widgets.clone();
        let detail_widgets = detail_widgets.clone();
        let toast_overlay = toast_overlay.clone();
        let on_change_cell = on_change_cell.clone();
        Rc::new(move |select_key: Option<String>| {
            if let Err(err) = state::persist(&state) {
                toast_overlay.add_toast(adw::Toast::new(&err));
            }
            sidebar::refresh_tag_filter_options(&sidebar_widgets, &state);
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            sidebar::refresh_list(&sidebar_widgets, &state, &cb);
            if !select_row_by_key(&sidebar_widgets.list_box, select_key.as_deref()) {
                state.borrow_mut().selected_key = None;
                detail::show_empty(&detail_widgets);
            }
        })
    };
    *on_change_cell.borrow_mut() = Some(on_change.clone());

    // ── Filter/search changes: refresh the view without touching disk ──
    let refresh_view = {
        let state = state.clone();
        let sidebar_widgets = sidebar_widgets.clone();
        let on_change_cell = on_change_cell.clone();
        move || {
            {
                let mut s = state.borrow_mut();
                s.search = sidebar_widgets.search_entry.text().to_string();
                s.filter_type = sidebar::current_filter_type(&sidebar_widgets);
                s.filter_tag = sidebar::current_filter_tag(&sidebar_widgets);
            }
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            sidebar::refresh_list(&sidebar_widgets, &state, &cb);
            let selected = state.borrow().selected_key.clone();
            select_row_by_key(&sidebar_widgets.list_box, selected.as_deref());
        }
    };
    {
        let refresh_view = refresh_view.clone();
        sidebar_widgets
            .search_entry
            .connect_search_changed(move |_| refresh_view());
    }
    {
        let refresh_view = refresh_view.clone();
        sidebar_widgets
            .type_filter
            .connect_selected_notify(move |_| refresh_view());
    }
    {
        let refresh_view = refresh_view.clone();
        sidebar_widgets
            .tag_filter
            .connect_selected_notify(move |_| refresh_view());
    }

    // ── Row selection drives the detail pane (single source of truth) ──
    {
        let state = state.clone();
        let detail_widgets = detail_widgets.clone();
        sidebar_widgets.list_box.connect_row_selected(move |_, row| {
            match row {
                Some(row) => {
                    let key = row.widget_name().to_string();
                    let entry = state
                        .borrow()
                        .entries
                        .iter()
                        .find(|e| e.key == key)
                        .cloned();
                    state.borrow_mut().selected_key = Some(key);
                    if let Some(entry) = entry {
                        detail::load_entry(&detail_widgets, &entry);
                    } else {
                        detail::show_empty(&detail_widgets);
                    }
                }
                None => {
                    state.borrow_mut().selected_key = None;
                    detail::show_empty(&detail_widgets);
                }
            }
        });
    }

    // ── Add entry: seed type/tag from the active filters so the new blank
    // entry is actually visible in the current view, and clear any search
    // text that would otherwise hide it. ──
    {
        let state = state.clone();
        let sidebar_widgets = sidebar_widgets.clone();
        let on_change_cell = on_change_cell.clone();
        sidebar_widgets.add_button.clone().connect_clicked(move |_| {
            let seeded_type = sidebar::current_filter_type(&sidebar_widgets).unwrap_or_default();
            let seeded_tag = sidebar::current_filter_tag(&sidebar_widgets);
            let new_key = {
                let mut s = state.borrow_mut();
                let key = skrizhal::unique_key("new-entry", &s.entries);
                s.entries.push(CvEntry {
                    key: key.clone(),
                    entry_type: seeded_type,
                    tags: seeded_tag.into_iter().collect(),
                    ..Default::default()
                });
                key
            };
            sidebar_widgets.search_entry.set_text("");
            state.borrow_mut().search = String::new();
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            cb(Some(new_key));
        });
    }

    // ── Save button ──────────────────────────────────────────────────
    {
        let state = state.clone();
        let detail_widgets = detail_widgets.clone();
        let toast_overlay = toast_overlay.clone();
        let on_change_cell = on_change_cell.clone();
        detail_widgets.save_button.clone().connect_clicked(move |_| {
            let result = if detail::is_raw_active(&detail_widgets) {
                detail::read_raw(&detail_widgets)
            } else {
                Ok(detail::read_form(&detail_widgets))
            };
            let mut new_entry = match result {
                Ok(e) => e,
                Err(err) => {
                    toast_overlay.add_toast(adw::Toast::new(&err));
                    return;
                }
            };
            new_entry.key = new_entry.key.trim().to_string();
            if new_entry.key.is_empty() {
                toast_overlay.add_toast(adw::Toast::new("Key can't be empty."));
                return;
            }

            let original_key = state.borrow().selected_key.clone();
            let collides = state.borrow().entries.iter().any(|e| {
                e.key == new_entry.key && Some(&e.key) != original_key.as_ref()
            });
            if collides {
                toast_overlay.add_toast(adw::Toast::new(&format!(
                    "Key \"{}\" is already used by another entry.",
                    new_entry.key
                )));
                return;
            }

            {
                let mut s = state.borrow_mut();
                let existing_idx = original_key
                    .as_ref()
                    .and_then(|k| s.entries.iter().position(|e| &e.key == k));
                match existing_idx {
                    Some(idx) => s.entries[idx] = new_entry.clone(),
                    None => s.entries.push(new_entry.clone()),
                }
            }
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            cb(Some(new_entry.key));
        });
    }

    // ── Header menu actions ──────────────────────────────────────────
    {
        let window = window.clone();
        let state = state.clone();
        let on_change_cell = on_change_cell.clone();
        let popover = menu_popover.clone();
        manage_tags_btn.connect_clicked(move |_| {
            popover.popdown();
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            dialogs::show_manage_tags_dialog(&window, &state, &cb);
        });
    }
    {
        let window = window.clone();
        let state = state.clone();
        let on_change_cell = on_change_cell.clone();
        let popover = menu_popover.clone();
        choose_file_btn.connect_clicked(move |_| {
            popover.popdown();
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            dialogs::choose_data_file(&window, &state, &cb);
        });
    }
    {
        let state = state.clone();
        let toast_overlay = toast_overlay.clone();
        let on_change_cell = on_change_cell.clone();
        let popover = menu_popover.clone();
        reload_btn.connect_clicked(move |_| {
            popover.popdown();
            match state::reload(&state) {
                Ok(()) => {
                    let cb = on_change_cell
                        .borrow()
                        .clone()
                        .expect("on_change_cell set before first use");
                    cb(None);
                    toast_overlay.add_toast(adw::Toast::new("Reloaded from disk."));
                }
                Err(err) => {
                    toast_overlay.add_toast(adw::Toast::new(&format!("Reload failed: {err}")));
                }
            }
        });
    }

    // ── Initial paint ────────────────────────────────────────────────
    if let Some(err) = load_error {
        toast_overlay.add_toast(adw::Toast::new(&format!(
            "Couldn't parse the data file, starting read-only: {err}"
        )));
    }
    sidebar::refresh_tag_filter_options(&sidebar_widgets, &state);
    sidebar::refresh_list(&sidebar_widgets, &state, &on_change);
    detail::show_empty(&detail_widgets);

    window.present();
}
