use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use skrizhal_core::CvEntry;

use super::changelog;
use super::detail;
use super::dialogs;
use super::field_guide;
use super::sidebar;
use super::spreadsheet;
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
    // Set by the Add-entry handler right before it triggers a refresh, so the
    // row-selected handler that follows knows to enable key auto-generation
    // for that one selection instead of treating it like any other load.
    let next_selection_is_new: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
    paned.set_start_child(Some(&sidebar_widgets.root));
    paned.set_end_child(Some(&detail_widgets.outer_stack));
    paned.set_resize_start_child(false);
    paned.set_shrink_start_child(false);
    paned.set_position(320);

    let spreadsheet_widgets = spreadsheet::build();
    // Nested inside toast_overlay (not the other way around) so toasts from
    // either view — including spreadsheet cell/key validation errors — stay
    // visible no matter which one is active.
    let main_view_stack = gtk4::Stack::new();
    main_view_stack.add_named(&paned, Some("browse"));
    main_view_stack.add_named(&spreadsheet_widgets.root, Some("spreadsheet"));
    main_view_stack.set_visible_child_name("browse");
    toast_overlay.set_child(Some(&main_view_stack));

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
    let field_guide_btn = gtk4::Button::builder()
        .label("Field Guide…")
        .css_classes(["flat"])
        .build();
    menu_box.append(&manage_tags_btn);
    menu_box.append(&choose_file_btn);
    menu_box.append(&reload_btn);
    menu_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    menu_box.append(&field_guide_btn);
    menu_popover.set_child(Some(&menu_box));
    menu_button.set_popover(Some(&menu_popover));
    header.pack_end(&menu_button);

    // ── Status bar ───────────────────────────────────────────────────
    let status_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    status_bar.set_margin_start(10);
    status_bar.set_margin_end(10);
    status_bar.set_margin_top(4);
    status_bar.set_margin_bottom(4);

    let spreadsheet_toggle = gtk4::ToggleButton::builder()
        .label("Spreadsheet")
        .css_classes(["flat"])
        .build();
    status_bar.append(&spreadsheet_toggle);

    let status_spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    status_spacer.set_hexpand(true);
    status_bar.append(&status_spacer);

    let version_button = gtk4::Button::builder()
        .label(format!("v{}", env!("CARGO_PKG_VERSION")))
        .css_classes(["flat", "caption"])
        .tooltip_text("View changelog")
        .build();
    status_bar.append(&version_button);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&toast_overlay));
    toolbar_view.add_bottom_bar(&status_bar);
    window.set_content(Some(&toolbar_view));

    {
        let window = window.clone();
        version_button.connect_clicked(move |_| changelog::show(&window));
    }

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
        let next_selection_is_new = next_selection_is_new.clone();
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
                // No matching row (e.g. the entry got filtered out of view) —
                // no Some(row) selection will follow to consume this, so
                // clear it here rather than leaving it to leak onto whatever
                // row the user selects next.
                next_selection_is_new.set(false);
                state.borrow_mut().selected_key = None;
                detail::show_empty(&detail_widgets);
            }
        })
    };
    *on_change_cell.borrow_mut() = Some(on_change.clone());

    // ── Spreadsheet toggle: swaps the main view; refreshes on the way in
    // so it reflects whatever changed while it was hidden. ──
    {
        let state = state.clone();
        let on_change = on_change.clone();
        let toast_overlay = toast_overlay.clone();
        let main_view_stack = main_view_stack.clone();
        let spreadsheet_widgets = spreadsheet_widgets.clone();
        spreadsheet_toggle.connect_toggled(move |btn| {
            if btn.is_active() {
                spreadsheet::refresh(&spreadsheet_widgets, &state, &on_change, &toast_overlay);
                main_view_stack.set_visible_child_name("spreadsheet");
            } else {
                main_view_stack.set_visible_child_name("browse");
            }
        });
    }

    // ── Filter/search changes: refresh the view without touching disk ──
    let refresh_view = {
        let state = state.clone();
        let sidebar_widgets = sidebar_widgets.clone();
        let on_change_cell = on_change_cell.clone();
        move || {
            {
                let mut s = state.borrow_mut();
                s.search = sidebar_widgets.search_entry.text().to_string();
                s.filter_category = sidebar::current_filter_category(&sidebar_widgets);
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
            .category_filter
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
        let next_selection_is_new = next_selection_is_new.clone();
        sidebar_widgets.list_box.connect_row_selected(move |_, row| {
            match row {
                Some(row) => {
                    // Only a real (Some) selection consumes the flag — the
                    // transient `row-selected(None)` that fires while
                    // refresh_list clears the old rows must not eat it
                    // before the actual new-row selection arrives.
                    let is_new = next_selection_is_new.replace(false);
                    let key = row.widget_name().to_string();
                    let entry = state
                        .borrow()
                        .entries
                        .iter()
                        .find(|e| e.key == key)
                        .cloned();
                    state.borrow_mut().selected_key = Some(key);
                    if let Some(entry) = entry {
                        detail::load_entry(&detail_widgets, &entry, is_new);
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

    // ── Key auto-generation: Title/Organization edits regenerate Key while
    // it's still following (see DetailWidgets::key_autogen_active); any
    // direct edit to Key itself turns auto-follow off. Both paths also
    // refresh the live duplicate/empty error state. ──
    {
        let state = state.clone();
        let detail_widgets = detail_widgets.clone();
        detail_widgets.title_row.clone().connect_changed(move |_| {
            let s = state.borrow();
            let original = s.selected_key.clone();
            detail::regenerate_key_if_autogen(&detail_widgets, &s.entries, original.as_deref());
        });
    }
    {
        let state = state.clone();
        let detail_widgets = detail_widgets.clone();
        detail_widgets.org_row.clone().connect_changed(move |_| {
            let s = state.borrow();
            let original = s.selected_key.clone();
            detail::regenerate_key_if_autogen(&detail_widgets, &s.entries, original.as_deref());
        });
    }
    {
        let state = state.clone();
        let detail_widgets = detail_widgets.clone();
        detail_widgets.key_row.clone().connect_changed(move |_| {
            detail::mark_key_dirty_if_user_edit(&detail_widgets);
            let s = state.borrow();
            let original = s.selected_key.clone();
            detail::update_key_error_state(&detail_widgets, &s.entries, original.as_deref());
        });
    }

    // ── Add entry: seed category/tag from the active filters so the new blank
    // entry is actually visible in the current view, and clear any search
    // text that would otherwise hide it. Key starts blank — the very next
    // selection (this one) is flagged so it enables key auto-generation. ──
    {
        let state = state.clone();
        let sidebar_widgets = sidebar_widgets.clone();
        let on_change_cell = on_change_cell.clone();
        let next_selection_is_new = next_selection_is_new.clone();
        sidebar_widgets.add_button.clone().connect_clicked(move |_| {
            let seeded_category =
                sidebar::current_filter_category(&sidebar_widgets).unwrap_or_default();
            let seeded_tag = sidebar::current_filter_tag(&sidebar_widgets);
            let new_key = {
                let mut s = state.borrow_mut();
                let key = skrizhal_core::unique_key("new-entry", &s.entries);
                s.entries.push(CvEntry {
                    key: key.clone(),
                    category: seeded_category,
                    tags: seeded_tag.into_iter().collect(),
                    ..Default::default()
                });
                key
            };
            sidebar_widgets.search_entry.set_text("");
            state.borrow_mut().search = String::new();
            next_selection_is_new.set(true);
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
        let window = window.clone();
        let popover = menu_popover.clone();
        field_guide_btn.connect_clicked(move |_| {
            popover.popdown();
            field_guide::show(&window, false);
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

    if !config.has_seen_field_guide {
        field_guide::show(&window, true);
    }
}
