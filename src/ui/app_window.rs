use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use skrizhal_core::{CvEntry, SortMode};

use super::changelog;
use super::detail;
use super::dialogs;
use super::field_guide;
use super::health;
use super::history;
use super::profiles;
use super::sidebar;
use super::state::{self, ChangeCallback, SharedState};
use crate::config::Config;

type RefreshViewCell = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// How long typing pauses before an edit is committed. Long enough not to
/// write on every keystroke, short enough that clicking away to another app
/// (rather than to another row, which flushes immediately) can't lose work.
const AUTOSAVE_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(600);

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
        .default_height(760)
        .build();

    let config = Config::load();
    let (initial_state, load_error) = state::load_initial(config.data_path.clone());
    let state: SharedState = Rc::new(RefCell::new(initial_state));
    state.borrow_mut().sort_mode = config.sort_mode;

    let sidebar_widgets = sidebar::build(&window);
    // Set before the change handler is connected (further down, next to the
    // other filter widgets) so restoring the saved mode isn't itself treated
    // as a user change worth persisting.
    sidebar_widgets.sort_dropdown.set_selected(config.sort_mode.index());
    let detail_widgets = detail::build();
    let toast_overlay = adw::ToastOverlay::new();

    // Recently-used tags quick-pick: rebuilt fresh each time the popover
    // opens, mirroring the Category suggestion popover but sourced from
    // actual tag usage across the file rather than a fixed list.
    {
        let state = state.clone();
        let tags_row = detail_widgets.tags_row.clone();
        let tags_suggest_box = detail_widgets.tags_suggest_box.clone();
        let popover = detail_widgets.tags_suggest_popover.clone();
        detail_widgets.tags_suggest_popover.connect_show(move |_| {
            while let Some(child) = tags_suggest_box.first_child() {
                tags_suggest_box.remove(&child);
            }
            let mut tags = skrizhal_core::all_tags_with_counts(&state.borrow().entries);
            tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            for (tag, count) in tags.into_iter().take(12) {
                let btn = gtk4::Button::builder()
                    .label(format!("{tag} ({count})"))
                    .css_classes(["flat"])
                    .build();
                let tags_row = tags_row.clone();
                let popover = popover.clone();
                btn.connect_clicked(move |_| {
                    let current = tags_row.text().to_string();
                    let mut parts: Vec<String> = current
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if !parts.iter().any(|p| p == &tag) {
                        parts.push(tag.clone());
                    }
                    tags_row.set_text(&parts.join(", "));
                    popover.popdown();
                });
                tags_suggest_box.append(&btn);
            }
            if tags_suggest_box.first_child().is_none() {
                let empty = gtk4::Label::builder()
                    .label("No tags used yet")
                    .css_classes(["dim-label"])
                    .margin_top(6)
                    .margin_bottom(6)
                    .margin_start(12)
                    .margin_end(12)
                    .build();
                tags_suggest_box.append(&empty);
            }
        });
    }
    // Set by the Add-entry handler right before it triggers a refresh, so the
    // row-selected handler that follows knows to enable key auto-generation
    // for that one selection instead of treating it like any other load.
    let next_selection_is_new: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    // Autosave bookkeeping. `suppress_autosave` is held while the app itself
    // is writing into the form (loading an entry, rebuilding the list) —
    // without it, a programmatic `set_text` looks exactly like the user
    // typing, and e.g. deleting an entry would immediately re-commit the
    // stale form contents and resurrect it.
    let suppress_autosave: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    // Bumped to invalidate an in-flight debounce — a pending commit that
    // fires after an undo would otherwise write the old form back over it.
    let autosave_generation: Rc<Cell<u64>> = Rc::new(Cell::new(0));
    // Which entry the current undo snapshot covers, so a run of keystrokes
    // costs one undo step rather than one per debounce tick.
    let undo_pushed_for: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    // Threaded through a cell for the same reason `refresh_view_cell` is: the
    // file monitor is built after `on_change`, which needs to call it.
    let rearm_monitor_cell: RefreshViewCell = Rc::new(RefCell::new(None));

    // Focus Mode: the sidebar collapses via a Revealer (animated slide) rather
    // than an instant set_visible, so hiding it for distraction-free editing
    // reads as a deliberate transition instead of a jump-cut.
    let sidebar_revealer = gtk4::Revealer::builder()
        .transition_type(gtk4::RevealerTransitionType::SlideLeft)
        .reveal_child(true)
        .child(&sidebar_widgets.root)
        .build();

    let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
    paned.set_start_child(Some(&sidebar_revealer));
    paned.set_end_child(Some(&detail_widgets.outer_stack));
    paned.set_resize_start_child(false);
    paned.set_shrink_start_child(false);
    paned.set_position(config.pane_position);

    // Remembers the split width to restore when Focus Mode is turned back
    // off — the revealer alone doesn't reclaim the paned's reserved width,
    // so the toggle handler below also drives `paned`'s position directly.
    let last_pane_position: Rc<Cell<i32>> = Rc::new(Cell::new(config.pane_position));
    // Set around Focus-Mode-driven position changes so they don't get
    // mistaken for a manual drag and persisted over the real preference.
    let suppress_position_persist: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    // Persist the sidebar/detail split, debounced 400ms after the last drag.
    // The realize-then-idle ready flag keeps the initial layout pass (which
    // also fires position-notify) from immediately overwriting the saved value.
    {
        let ready: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let ready_for_realize = ready.clone();
        paned.connect_realize(move |_| {
            let ready_for_idle = ready_for_realize.clone();
            glib::idle_add_local_once(move || ready_for_idle.set(true));
        });
        let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let suppress_position_persist = suppress_position_persist.clone();
        paned.connect_position_notify(move |p| {
            if !ready.get() || suppress_position_persist.get() {
                return;
            }
            let pos = p.position();
            let pending_for_cb = pending.clone();
            let mut slot = pending.borrow_mut();
            if let Some(id) = slot.take() {
                id.remove();
            }
            *slot = Some(glib::timeout_add_local_once(
                std::time::Duration::from_millis(400),
                move || {
                    *pending_for_cb.borrow_mut() = None;
                    let mut c = Config::load();
                    c.pane_position = pos;
                    let _ = c.save();
                },
            ));
        });
    }

    toast_overlay.set_child(Some(&paned));

    // ── Header bar ───────────────────────────────────────────────────
    let header = adw::HeaderBar::new();

    let sidebar_toggle = gtk4::ToggleButton::builder()
        .icon_name("sidebar-show-symbolic")
        .active(true)
        .build();
    {
        let sidebar_revealer = sidebar_revealer.clone();
        let paned = paned.clone();
        let last_pane_position = last_pane_position.clone();
        let suppress_position_persist = suppress_position_persist.clone();
        sidebar_toggle.connect_toggled(move |btn| {
            suppress_position_persist.set(true);
            if btn.is_active() {
                paned.set_position(last_pane_position.get());
                sidebar_revealer.set_reveal_child(true);
                // Re-enforce the normal drag-shrink floor now that the
                // sidebar's real (non-zero-min) content is back.
                paned.set_shrink_start_child(false);
            } else {
                last_pane_position.set(paned.position());
                sidebar_revealer.set_reveal_child(false);
                // shrink_start_child(false) clamps position to the sidebar
                // content's minimum width — lift that so position(0) can
                // actually reclaim the space instead of leaving a gap.
                paned.set_shrink_start_child(true);
                paned.set_position(0);
            }
            suppress_position_persist.set(false);
        });
    }
    header.pack_start(&sidebar_toggle);
    header.pack_start(&sidebar_widgets.add_button);

    let undo_button = gtk4::Button::builder()
        .icon_name("edit-undo-symbolic")
        .tooltip_text("Undo (Ctrl+Z)")
        .sensitive(false)
        .build();
    let redo_button = gtk4::Button::builder()
        .icon_name("edit-redo-symbolic")
        .tooltip_text("Redo (Ctrl+Shift+Z)")
        .sensitive(false)
        .build();
    header.pack_start(&undo_button);
    header.pack_start(&redo_button);

    let menu_button = gtk4::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .build();
    let menu_popover = gtk4::Popover::new();
    let menu_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let new_file_btn = make_menu_item("New File…", Some("Ctrl+N"));
    let choose_file_btn = make_menu_item("Open…", Some("Ctrl+O"));
    let save_as_btn = make_menu_item("Save As…", Some("Ctrl+Shift+S"));
    let reload_btn = make_menu_item("Reload from Disk", None);
    let import_btn = make_menu_item("Import from BibTeX…", None);
    let manage_tags_btn = make_menu_item("Manage Tags…", None);
    let profiles_btn = make_menu_item("CV Profiles…", None);
    let health_btn = make_menu_item("Database Health…", None);
    let history_btn = make_menu_item("File History…", None);
    let preferences_btn = make_menu_item("Preferences…", None);
    let field_guide_btn = make_menu_item("Field Guide…", None);
    menu_box.append(&new_file_btn);
    menu_box.append(&choose_file_btn);
    menu_box.append(&save_as_btn);
    menu_box.append(&reload_btn);
    menu_box.append(&import_btn);
    menu_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    menu_box.append(&manage_tags_btn);
    menu_box.append(&profiles_btn);
    menu_box.append(&health_btn);
    menu_box.append(&history_btn);
    menu_box.append(&preferences_btn);
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

    let status_spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    status_spacer.set_hexpand(true);
    status_bar.append(&status_spacer);

    let version_button = gtk4::Button::builder()
        .label(format!("v{}", env!("CARGO_PKG_VERSION")))
        .css_classes(["flat", "caption"])
        .tooltip_text("View changelog")
        .build();
    status_bar.append(&version_button);

    // Banner, not a toast: an external change is an actionable one-off tied to
    // file state, and it has to stay on screen until it's dealt with.
    let external_change_banner = adw::Banner::builder()
        .title("This file was changed by another program.")
        .button_label("Reload")
        .revealed(false)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.add_top_bar(&external_change_banner);
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
    // refresh_view (defined below, once search/category/tag widgets are all
    // wired up) needs to be reachable from on_change's body, which is built
    // first — so it's threaded through this cell rather than captured directly.
    let refresh_view_cell: RefreshViewCell = Rc::new(RefCell::new(None));
    let on_change: ChangeCallback = {
        let state = state.clone();
        let sidebar_widgets = sidebar_widgets.clone();
        let detail_widgets = detail_widgets.clone();
        let toast_overlay = toast_overlay.clone();
        let on_change_cell = on_change_cell.clone();
        let next_selection_is_new = next_selection_is_new.clone();
        let undo_button = undo_button.clone();
        let redo_button = redo_button.clone();
        let refresh_view_cell = refresh_view_cell.clone();
        let suppress_autosave = suppress_autosave.clone();
        let autosave_generation = autosave_generation.clone();
        let external_change_banner = external_change_banner.clone();
        let rearm_monitor_cell = rearm_monitor_cell.clone();
        Rc::new(move |select_key: Option<String>| {
            let was_suppressed = suppress_autosave.replace(true);
            autosave_generation.set(autosave_generation.get() + 1);
            match state::persist(&state) {
                // Our write is now the newest thing on disk, so any pending
                // "changed elsewhere" warning has been answered — by us.
                Ok(()) => external_change_banner.set_revealed(false),
                Err(err) => toast_overlay.add_toast(adw::Toast::new(&err)),
            }
            // Open/New File/Save As all change `data_path` and then land here,
            // so re-arming centrally covers every one of them.
            if let Some(rearm) = rearm_monitor_cell.borrow().clone() {
                rearm();
            }
            undo_button.set_sensitive(state::can_undo(&state));
            redo_button.set_sensitive(state::can_redo(&state));
            let refresh_view_cell = refresh_view_cell.clone();
            let on_tag_toggle: Rc<dyn Fn()> = Rc::new(move || {
                if let Some(rv) = refresh_view_cell.borrow().clone() {
                    rv();
                }
            });
            sidebar::refresh_tag_filter_options(&sidebar_widgets, &state, on_tag_toggle);
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
            suppress_autosave.set(was_suppressed);
        })
    };
    *on_change_cell.borrow_mut() = Some(on_change.clone());

    // ── Autosave ─────────────────────────────────────────────────────
    // Every other mutation in this window (delete, duplicate, tag rename)
    // persisted immediately while detail-pane edits needed an explicit Save,
    // and navigating to another row silently discarded them. Field edits now
    // commit on a debounce and on selection change, with undo as the net.
    //
    // `explicit` distinguishes the Save button from a debounce tick: an
    // explicit save reports failures as toasts and runs a full refresh (so
    // filters and sort re-apply), while an autosave stays silent and patches
    // the affected row in place, because rebuilding the list under a typing
    // user would reload the form and eat the keystroke mid-word.
    let commit_edit: Rc<dyn Fn(bool)> = Rc::new({
        let state = state.clone();
        let detail_widgets = detail_widgets.clone();
        let sidebar_widgets = sidebar_widgets.clone();
        let toast_overlay = toast_overlay.clone();
        let on_change_cell = on_change_cell.clone();
        let suppress_autosave = suppress_autosave.clone();
        let undo_pushed_for = undo_pushed_for.clone();
        let undo_button = undo_button.clone();
        let redo_button = redo_button.clone();
        let external_change_banner = external_change_banner.clone();
        move |explicit: bool| {
            if suppress_autosave.get() {
                return;
            }
            let Some(original_key) = state.borrow().selected_key.clone() else {
                return;
            };
            // Raw YAML commits only on an explicit Save — a debounce would be
            // parsing half-typed YAML and failing on nearly every keystroke.
            let raw = detail::is_raw_active(&detail_widgets);
            if raw && !explicit {
                return;
            }
            let result = if raw {
                detail::read_raw(&detail_widgets)
            } else {
                Ok(detail::read_form(&detail_widgets))
            };
            let mut new_entry = match result {
                Ok(e) => e,
                Err(err) => {
                    if explicit {
                        toast_overlay.add_toast(adw::Toast::new(&err));
                    }
                    return;
                }
            };
            new_entry.key = new_entry.key.trim().to_string();
            if new_entry.key.is_empty() {
                if explicit {
                    toast_overlay.add_toast(adw::Toast::new("Key can't be empty."));
                }
                return;
            }
            let collides = state
                .borrow()
                .entries
                .iter()
                .any(|e| e.key == new_entry.key && e.key != original_key);
            if collides {
                if explicit {
                    toast_overlay.add_toast(adw::Toast::new(&format!(
                        "Key \"{}\" is already used by another entry.",
                        new_entry.key
                    )));
                }
                return;
            }
            let existing_idx = state
                .borrow()
                .entries
                .iter()
                .position(|e| e.key == original_key);
            let Some(idx) = existing_idx else {
                // Selection points at an entry that's gone (deleted, or undone
                // out of existence). Committing here would resurrect it.
                return;
            };
            // `order` has no form field, so `read_form` always reports None.
            // Carry the stored value across, or every save would quietly strip
            // an entry's manual position. (Raw YAML edits set it explicitly.)
            if !raw {
                new_entry.order = state.borrow().entries[idx].order;
            }
            if state.borrow().entries[idx] == new_entry {
                return;
            }
            // One undo snapshot per entry per editing session — otherwise a
            // sentence of typing would flush the whole 50-deep stack.
            let already_snapshotted =
                undo_pushed_for.borrow().as_deref() == Some(original_key.as_str());
            if !already_snapshotted {
                state::push_undo(&state);
            }
            {
                let mut s = state.borrow_mut();
                s.entries[idx] = new_entry.clone();
                s.selected_key = Some(new_entry.key.clone());
            }
            // Follow the rename, so continuing to type doesn't start a second
            // undo snapshot every time an auto-generated key shifts.
            *undo_pushed_for.borrow_mut() = Some(new_entry.key.clone());

            if explicit {
                let cb = on_change_cell
                    .borrow()
                    .clone()
                    .expect("on_change_cell set before first use");
                cb(Some(new_entry.key));
            } else {
                match state::persist(&state) {
                    Ok(()) => external_change_banner.set_revealed(false),
                    Err(err) => toast_overlay.add_toast(adw::Toast::new(&err)),
                }
                sidebar::update_row_in_place(&sidebar_widgets, &original_key, &new_entry);
                detail::update_warnings(&detail_widgets, &new_entry);
                undo_button.set_sensitive(state::can_undo(&state));
                redo_button.set_sensitive(state::can_redo(&state));
            }
        }
    });

    // Commits any pending edit right now, cancelling the in-flight debounce.
    let flush_autosave: Rc<dyn Fn()> = Rc::new({
        let commit_edit = commit_edit.clone();
        let autosave_generation = autosave_generation.clone();
        move || {
            autosave_generation.set(autosave_generation.get() + 1);
            commit_edit(false);
        }
    });

    {
        let commit_edit = commit_edit.clone();
        let autosave_generation = autosave_generation.clone();
        let suppress_autosave = suppress_autosave.clone();
        let schedule: Rc<dyn Fn()> = Rc::new(move || {
            if suppress_autosave.get() {
                return;
            }
            let generation = autosave_generation.get() + 1;
            autosave_generation.set(generation);
            let commit_edit = commit_edit.clone();
            let autosave_generation = autosave_generation.clone();
            glib::timeout_add_local_once(AUTOSAVE_DEBOUNCE, move || {
                if autosave_generation.get() == generation {
                    commit_edit(false);
                }
            });
        });
        *detail_widgets.on_dirty.borrow_mut() = Some(schedule);
    }

    // ── External-change detection ────────────────────────────────────
    // Zerkalo re-reads the CV data on every compile, but nothing told
    // Skrizhal when the file moved underneath it — so a save here could
    // silently clobber an edit made anywhere else.
    let file_monitor: Rc<RefCell<Option<(std::path::PathBuf, gtk4::gio::FileMonitor)>>> =
        Rc::new(RefCell::new(None));
    let rearm_monitor: Rc<dyn Fn()> = Rc::new({
        let state = state.clone();
        let file_monitor = file_monitor.clone();
        let banner = external_change_banner.clone();
        move || {
            let path = state.borrow().data_path.clone();
            if file_monitor.borrow().as_ref().map(|(p, _)| p) == Some(&path) {
                return;
            }
            let file = gtk4::gio::File::for_path(&path);
            let monitor = match file.monitor_file(
                gtk4::gio::FileMonitorFlags::WATCH_MOVES,
                gtk4::gio::Cancellable::NONE,
            ) {
                Ok(m) => m,
                // Not fatal: without a monitor the app simply behaves as it
                // did before, with Reload From Disk as the manual fallback.
                Err(_) => {
                    *file_monitor.borrow_mut() = None;
                    return;
                }
            };
            {
                let state = state.clone();
                let banner = banner.clone();
                monitor.connect_changed(move |_, _, _, _| {
                    if state::changed_externally(&state) {
                        banner.set_revealed(true);
                    }
                });
            }
            *file_monitor.borrow_mut() = Some((path, monitor));
        }
    });
    rearm_monitor();
    *rearm_monitor_cell.borrow_mut() = Some(rearm_monitor.clone());

    {
        let state = state.clone();
        let toast_overlay = toast_overlay.clone();
        let on_change_cell = on_change_cell.clone();
        let banner = external_change_banner.clone();
        external_change_banner.connect_button_clicked(move |_| {
            banner.set_revealed(false);
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

    // A half-typed entry shouldn't need the window to stay open to survive.
    {
        let flush_autosave = flush_autosave.clone();
        let state_for_close = state.clone();
        window.connect_close_request(move |_| {
            flush_autosave();
            // Order matters: the flush must land on disk before the snapshot,
            // or the session's last edit misses the commit.
            history::snapshot_on_close(&state_for_close);
            glib::Propagation::Proceed
        });
    }

    // ── Undo/redo ────────────────────────────────────────────────────
    {
        let state = state.clone();
        let on_change = on_change.clone();
        undo_button.connect_clicked(move |_| {
            if state::undo(&state) {
                on_change(None);
            }
        });
    }
    {
        let state = state.clone();
        let on_change = on_change.clone();
        redo_button.connect_clicked(move |_| {
            if state::redo(&state) {
                on_change(None);
            }
        });
    }
    {
        let state = state.clone();
        let on_change = on_change.clone();
        let new_file_btn = new_file_btn.clone();
        let choose_file_btn = choose_file_btn.clone();
        let save_as_btn = save_as_btn.clone();
        let search_entry = sidebar_widgets.search_entry.clone();
        let save_button = detail_widgets.save_button.clone();
        let key_controller = gtk4::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        key_controller.connect_key_pressed(move |_, keyval, _, modifiers| {
            let ctrl = modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
            let shift = modifiers.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
            if ctrl && !shift && keyval == gtk4::gdk::Key::z {
                if state::undo(&state) {
                    on_change(None);
                }
                return glib::Propagation::Stop;
            }
            if ctrl && shift && keyval == gtk4::gdk::Key::Z {
                if state::redo(&state) {
                    on_change(None);
                }
                return glib::Propagation::Stop;
            }
            if ctrl && !shift && keyval == gtk4::gdk::Key::n {
                new_file_btn.emit_clicked();
                return glib::Propagation::Stop;
            }
            if ctrl && !shift && keyval == gtk4::gdk::Key::o {
                choose_file_btn.emit_clicked();
                return glib::Propagation::Stop;
            }
            if ctrl && shift && keyval == gtk4::gdk::Key::S {
                save_as_btn.emit_clicked();
                return glib::Propagation::Stop;
            }
            if ctrl && !shift && keyval == gtk4::gdk::Key::s {
                save_button.emit_clicked();
                return glib::Propagation::Stop;
            }
            if ctrl && !shift && keyval == gtk4::gdk::Key::f {
                search_entry.grab_focus();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(key_controller);
    }

    // ── Filter/search changes: refresh the view without touching disk ──
    let refresh_view: Rc<dyn Fn()> = Rc::new({
        let state = state.clone();
        let sidebar_widgets = sidebar_widgets.clone();
        let on_change_cell = on_change_cell.clone();
        let suppress_autosave = suppress_autosave.clone();
        move || {
            let was_suppressed = suppress_autosave.replace(true);
            {
                let mut s = state.borrow_mut();
                s.search = sidebar_widgets.search_entry.text().to_string();
                s.filter_category = sidebar::current_filter_category(&sidebar_widgets);
                s.filter_tags = sidebar::current_filter_tags(&sidebar_widgets);
                s.sort_mode = SortMode::from_index(sidebar_widgets.sort_dropdown.selected());
            }
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            sidebar::refresh_list(&sidebar_widgets, &state, &cb);
            let selected = state.borrow().selected_key.clone();
            select_row_by_key(&sidebar_widgets.list_box, selected.as_deref());
            suppress_autosave.set(was_suppressed);
        }
    });
    *refresh_view_cell.borrow_mut() = Some(refresh_view.clone());
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
        sidebar_widgets.sort_dropdown.connect_selected_notify(move |dd| {
            refresh_view();
            let mut c = Config::load();
            c.sort_mode = SortMode::from_index(dd.selected());
            let _ = c.save();
        });
    }

    // ── Row selection drives the detail pane (single source of truth) ──
    {
        let state = state.clone();
        let detail_widgets = detail_widgets.clone();
        let next_selection_is_new = next_selection_is_new.clone();
        let flush_autosave = flush_autosave.clone();
        let suppress_autosave = suppress_autosave.clone();
        let undo_pushed_for = undo_pushed_for.clone();
        sidebar_widgets.list_box.connect_row_selected(move |_, row| {
            // Commit whatever the outgoing entry has in the form before the
            // incoming one overwrites it. A no-op during list rebuilds, which
            // hold the suppress guard.
            flush_autosave();
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
                    // A fresh entry starts a fresh undo snapshot.
                    *undo_pushed_for.borrow_mut() = None;
                    // Populating the form is a programmatic write, not a user
                    // edit — it must not schedule an autosave of its own.
                    let was_suppressed = suppress_autosave.replace(true);
                    if let Some(entry) = entry {
                        detail::load_entry(&detail_widgets, &entry, is_new);
                    } else {
                        detail::show_empty(&detail_widgets);
                    }
                    suppress_autosave.set(was_suppressed);
                }
                None => {
                    state.borrow_mut().selected_key = None;
                    *undo_pushed_for.borrow_mut() = None;
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
    // Changing the category offers that category's recommended fields right
    // away, rather than waiting for a validation warning to mention them.
    {
        let detail_widgets = detail_widgets.clone();
        let suppress_autosave = suppress_autosave.clone();
        detail_widgets
            .category_row
            .clone()
            .connect_changed(move |row| {
                if suppress_autosave.get() {
                    return;
                }
                detail::suggest_category_fields(&detail_widgets, &row.text());
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
            state::push_undo(&state);
            let seeded_category =
                sidebar::current_filter_category(&sidebar_widgets).unwrap_or_default();
            let seeded_tags = sidebar::current_filter_tags(&sidebar_widgets);
            let new_key = {
                let mut s = state.borrow_mut();
                let key = skrizhal_core::unique_key("new-entry", &s.entries);
                s.entries.push(CvEntry {
                    key: key.clone(),
                    category: seeded_category,
                    tags: seeded_tags.into_iter().collect(),
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

    {
        let add_button = sidebar_widgets.add_button.clone();
        detail_widgets
            .empty_add_button
            .connect_clicked(move |_| add_button.emit_clicked());
    }

    // ── Save button ──────────────────────────────────────────────────
    // Autosave covers the form; Save stays as the explicit "commit now"
    // affordance and is the only way the raw-YAML pane ever commits.
    {
        let commit_edit = commit_edit.clone();
        detail_widgets
            .save_button
            .clone()
            .connect_clicked(move |_| commit_edit(true));
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
        import_btn.connect_clicked(move |_| {
            popover.popdown();
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            dialogs::import_bibtex(&window, &state, &cb);
        });
    }
    {
        let window = window.clone();
        let state = state.clone();
        let popover = menu_popover.clone();
        profiles_btn.connect_clicked(move |_| {
            popover.popdown();
            profiles::show(&window, &state);
        });
    }
    {
        let window = window.clone();
        let state = state.clone();
        let on_change_cell = on_change_cell.clone();
        let popover = menu_popover.clone();
        history_btn.connect_clicked(move |_| {
            popover.popdown();
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            history::show(&window, &state, &cb);
        });
    }
    {
        let window = window.clone();
        let state = state.clone();
        let popover = menu_popover.clone();
        let sidebar_widgets = sidebar_widgets.clone();
        let refresh_view = refresh_view.clone();
        health_btn.connect_clicked(move |_| {
            popover.popdown();
            // Jumping to a flagged entry has to clear the filters first, or
            // the row it wants to select may not be in the list at all.
            let select_entry: Rc<dyn Fn(String)> = Rc::new({
                let sidebar_widgets = sidebar_widgets.clone();
                let refresh_view = refresh_view.clone();
                move |key: String| {
                    sidebar_widgets.search_entry.set_text("");
                    sidebar_widgets.category_filter.set_selected(0);
                    sidebar::clear_tag_filter_selection(&sidebar_widgets);
                    refresh_view();
                    select_row_by_key(&sidebar_widgets.list_box, Some(&key));
                }
            });
            health::show(&window, &state, select_entry);
        });
    }
    {
        let window = window.clone();
        let state = state.clone();
        let on_change_cell = on_change_cell.clone();
        let popover = menu_popover.clone();
        new_file_btn.connect_clicked(move |_| {
            popover.popdown();
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            dialogs::new_file(&window, &state, &cb);
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
        let state = state.clone();
        let on_change_cell = on_change_cell.clone();
        let popover = menu_popover.clone();
        save_as_btn.connect_clicked(move |_| {
            popover.popdown();
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            dialogs::save_as(&window, &state, &cb);
        });
    }
    {
        let window = window.clone();
        let state = state.clone();
        let on_change_cell = on_change_cell.clone();
        let popover = menu_popover.clone();
        preferences_btn.connect_clicked(move |_| {
            popover.popdown();
            let cb = on_change_cell
                .borrow()
                .clone()
                .expect("on_change_cell set before first use");
            dialogs::show_preferences(&window, &state, &cb);
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
                    match state::parse_warnings_summary(&state.borrow().parse_warnings) {
                        Some(summary) => {
                            let toast = adw::Toast::new(&summary);
                            toast.set_timeout(0);
                            toast_overlay.add_toast(toast);
                        }
                        None => toast_overlay.add_toast(adw::Toast::new("Reloaded from disk.")),
                    }
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
    } else if let Some(summary) = state::parse_warnings_summary(&state.borrow().parse_warnings) {
        let toast = adw::Toast::new(&summary);
        toast.set_timeout(0);
        toast_overlay.add_toast(toast);
    }
    sidebar::refresh_tag_filter_options(&sidebar_widgets, &state, refresh_view.clone());
    sidebar::refresh_list(&sidebar_widgets, &state, &on_change);
    detail::show_empty(&detail_widgets);

    window.present();

    if !config.has_seen_field_guide {
        field_guide::show(&window, true);
    }
}

fn make_menu_item(label: &str, shortcut: Option<&str>) -> gtk4::Button {
    let btn = gtk4::Button::new();
    btn.add_css_class("flat");

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    row.set_margin_start(4);
    row.set_margin_end(6);

    let name_lbl = gtk4::Label::new(Some(label));
    name_lbl.set_halign(gtk4::Align::Start);
    name_lbl.set_hexpand(true);
    row.append(&name_lbl);

    if let Some(sc) = shortcut {
        let sc_lbl = gtk4::Label::new(Some(sc));
        sc_lbl.set_halign(gtk4::Align::End);
        sc_lbl.add_css_class("dim-label");
        sc_lbl.add_css_class("caption");
        sc_lbl.set_margin_start(16);
        row.append(&sc_lbl);
    }

    btn.set_child(Some(&row));
    btn
}
