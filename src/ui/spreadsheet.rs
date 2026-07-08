use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;

use skrizhal_core::CvEntry;

use super::state::{ChangeCallback, SharedState};

type ColumnSetter = Rc<dyn Fn(&mut CvEntry, &str)>;

/// One of the fillable (non-Key) columns. Key is deliberately excluded —
/// dragging the same key across several rows would just create duplicates,
/// so it gets a plain editable cell with no fill handle instead.
#[derive(Clone)]
struct ColumnSpec {
    label: &'static str,
    /// Minimum width in characters — chosen per column so typical content
    /// (a title, an organization name, a `YYYY-MM/YYYY-MM` range) fits
    /// without truncating; columns still grow further if the window is
    /// wider than the sum of these minimums.
    width_chars: i32,
    get: Rc<dyn Fn(&CvEntry) -> String>,
    set: ColumnSetter,
}

const KEY_WIDTH_CHARS: i32 = 20;

fn column_specs() -> Vec<ColumnSpec> {
    fn opt(v: &str) -> Option<String> {
        let v = v.trim();
        if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        }
    }
    vec![
        ColumnSpec {
            label: "Category",
            width_chars: 14,
            get: Rc::new(|e| e.category.clone()),
            set: Rc::new(|e, v| e.category = v.trim().to_string()),
        },
        ColumnSpec {
            label: "Title",
            width_chars: 24,
            get: Rc::new(|e| e.title.clone()),
            set: Rc::new(|e, v| e.title = v.trim().to_string()),
        },
        ColumnSpec {
            label: "Organization",
            width_chars: 24,
            get: Rc::new(|e| e.organization.clone().unwrap_or_default()),
            set: Rc::new(|e, v| e.organization = opt(v)),
        },
        ColumnSpec {
            label: "Location",
            width_chars: 16,
            get: Rc::new(|e| e.location.clone().unwrap_or_default()),
            set: Rc::new(|e, v| e.location = opt(v)),
        },
        ColumnSpec {
            label: "Date",
            width_chars: 16,
            get: Rc::new(|e| e.date.clone().unwrap_or_default()),
            set: Rc::new(|e, v| e.date = opt(v)),
        },
        ColumnSpec {
            label: "Tags",
            width_chars: 18,
            get: Rc::new(|e| e.tags.join(", ")),
            set: Rc::new(|e, v| {
                e.tags = v
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            }),
        },
    ]
}

struct RowWidgets {
    key: Rc<RefCell<String>>,
    key_entry: gtk4::Entry,
    cell_entries: Vec<gtk4::Entry>,
}

#[derive(Clone)]
pub struct SpreadsheetWidgets {
    /// A Box containing the "+ Add Row" toolbar above the scrollable grid —
    /// this is what the caller adds to its view stack.
    pub root: gtk4::Box,
    pub add_row_button: gtk4::Button,
    grid: gtk4::Grid,
    rows: Rc<RefCell<Vec<RowWidgets>>>,
    /// Exactly the widgets `refresh` has attached for data rows (never the
    /// permanent header row) — tracked explicitly so each refresh can remove
    /// precisely those, rather than guessing from grid traversal.
    data_widgets: Rc<RefCell<Vec<gtk4::Widget>>>,
}

const FILL_HANDLE_CSS: &str = "
.skrizhal-fill-handle {
    background-color: @accent_bg_color;
    min-width: 9px;
    min-height: 9px;
    border-radius: 2px;
}
.skrizhal-fill-preview {
    background-color: alpha(@accent_bg_color, 0.25);
}
.skrizhal-sheet-header {
    padding-bottom: 6px;
    border-bottom: 2px solid alpha(@window_fg_color, 0.15);
}
entry.skrizhal-sheet-row-even {
    background-color: alpha(@window_fg_color, 0.06);
}
";

pub fn build() -> SpreadsheetWidgets {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(FILL_HANDLE_CSS);
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let grid = gtk4::Grid::new();
    grid.set_hexpand(true);
    grid.set_row_spacing(4);
    grid.set_column_spacing(10);
    grid.set_margin_top(8);
    grid.set_margin_bottom(12);
    grid.set_margin_start(12);
    grid.set_margin_end(12);

    let key_header = gtk4::Label::builder()
        .label("Key")
        .css_classes(["heading", "skrizhal-sheet-header"])
        .xalign(0.0)
        .width_chars(KEY_WIDTH_CHARS)
        .build();
    grid.attach(&key_header, 0, 0, 1, 1);
    for (i, spec) in column_specs().iter().enumerate() {
        let header = gtk4::Label::builder()
            .label(spec.label)
            .css_classes(["heading", "skrizhal-sheet-header"])
            .xalign(0.0)
            .width_chars(spec.width_chars)
            .build();
        grid.attach(&header, (i + 1) as i32, 0, 1, 1);
    }

    let scrolled = gtk4::ScrolledWindow::builder().vexpand(true).hexpand(true).child(&grid).build();

    let add_row_button = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Add Row")
        .css_classes(["flat"])
        .halign(gtk4::Align::Start)
        .margin_start(8)
        .margin_top(8)
        .build();
    let toolbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    toolbar.append(&add_row_button);

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root.append(&toolbar);
    root.append(&scrolled);

    SpreadsheetWidgets {
        root,
        add_row_button,
        grid,
        rows: Rc::new(RefCell::new(Vec::new())),
        data_widgets: Rc::new(RefCell::new(Vec::new())),
    }
}

fn make_cell(initial: &str, width_chars: i32) -> gtk4::Entry {
    gtk4::Entry::builder()
        .text(initial)
        .hexpand(true)
        .width_chars(width_chars)
        .build()
}

/// A cell's column: `None` is the Key column, `Some(i)` is `column_specs()[i]`.
type CellCol = Option<usize>;

/// Wires focus-out (click away, Tab away — Tab's own commit-and-move is
/// handled separately below, but this is the fallback for e.g. clicking
/// straight into another row) to call `commit` with the entry's current text.
fn wire_commit(entry: &gtk4::Entry, commit: impl Fn(String) + 'static + Clone) {
    let focus = gtk4::EventControllerFocus::new();
    {
        let entry = entry.clone();
        focus.connect_leave(move |_| commit(entry.text().to_string()));
    }
    entry.add_controller(focus);
}

/// Finds a row's current grid index by key rather than trusting a captured
/// `row_idx` — editing the Key column can change sort order (rows are
/// sorted by key), so after a commit-triggered `refresh()` rebuilds the
/// grid, the row we were just in may no longer be at the same index.
fn row_index_for_key(rows: &Rc<RefCell<Vec<RowWidgets>>>, key: &str) -> Option<usize> {
    rows.borrow().iter().position(|r| *r.key.borrow() == key)
}

/// Focuses the given cell (if it still exists) and selects its text, so
/// typing immediately replaces the previous value — matches normal
/// spreadsheet cell-to-cell navigation.
fn focus_cell(widgets: &SpreadsheetWidgets, row_idx: usize, col: CellCol) {
    let rows = widgets.rows.borrow();
    let Some(row) = rows.get(row_idx) else { return };
    let entry = match col {
        None => &row.key_entry,
        Some(idx) => &row.cell_entries[idx],
    };
    entry.grab_focus();
    entry.select_region(0, -1);
}

/// Where Tab should go next: across the row, then wrapping to the Key cell
/// of the next row. Returns `None` at the very last cell of the last row —
/// nothing to move to.
fn next_tab_position(row: usize, col: CellCol, row_count: usize, col_count: usize) -> Option<(usize, CellCol)> {
    match col {
        None if col_count > 0 => Some((row, Some(0))),
        None => (row + 1 < row_count).then_some((row + 1, None)),
        Some(idx) if idx + 1 < col_count => Some((row, Some(idx + 1))),
        Some(_) => (row + 1 < row_count).then_some((row + 1, None)),
    }
}

/// Wires Enter and Tab for a spreadsheet cell:
/// - **Enter** commits the cell. If this row is the last one, that's read as
///   "I'm done with this entry, give me another" — a new blank row is
///   appended and focus moves to its Key cell. Otherwise focus just stays
///   put (the commit's `refresh()` already rebuilt this cell fresh).
/// - **Tab** commits the cell and moves focus to the next cell — across the
///   row, then wrapping to the next row's Key cell — instead of relying on
///   GTK's default focus-chain, which doesn't survive `refresh()` rebuilding
///   every cell as a new widget instance mid-navigation.
///
/// Both re-locate the row by key (`row_index_for_key`) rather than the
/// `row_idx`/`row_count` captured when this cell was built, since editing
/// the Key column can reorder rows (sorted by key) as part of the very
/// commit this triggers.
fn wire_enter_tab(
    entry: &gtk4::Entry,
    widgets: &SpreadsheetWidgets,
    key_rc: Rc<RefCell<String>>,
    col: CellCol,
    col_count: usize,
    commit: impl Fn(String) + 'static + Clone,
    add_row: Rc<dyn Fn()>,
) {
    let controller = gtk4::EventControllerKey::new();
    controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let entry_for_handler = entry.clone();
    let widgets = widgets.clone();
    controller.connect_key_pressed(move |_, keyval, _, _| {
        use gtk4::gdk::Key;
        let is_enter = keyval == Key::Return || keyval == Key::KP_Enter;
        let is_tab = keyval == Key::Tab || keyval == Key::ISO_Left_Tab;
        if !is_enter && !is_tab {
            return glib::Propagation::Proceed;
        }
        commit(entry_for_handler.text().to_string());
        let key = key_rc.borrow().clone();
        let row_count = widgets.rows.borrow().len();
        let Some(row_idx) = row_index_for_key(&widgets.rows, &key) else {
            return glib::Propagation::Stop;
        };
        if is_enter {
            if row_idx + 1 == row_count {
                add_row();
            } else {
                focus_cell(&widgets, row_idx, col);
            }
        } else if let Some((r, c)) = next_tab_position(row_idx, col, row_count, col_count) {
            focus_cell(&widgets, r, c);
        }
        glib::Propagation::Stop
    });
    entry.add_controller(controller);
}

/// Rebuilds every row from `state`, sorted by key. Any commit (direct edit,
/// fill-drag) re-persists via `on_change` and then calls this again, so the
/// grid always reflects what's actually on disk.
pub fn refresh(
    widgets: &SpreadsheetWidgets,
    state: &SharedState,
    on_change: &ChangeCallback,
    toast_overlay: &adw::ToastOverlay,
) {
    for w in widgets.data_widgets.borrow_mut().drain(..) {
        widgets.grid.remove(&w);
    }
    widgets.rows.borrow_mut().clear();

    let mut entries = state.borrow().entries.clone();
    entries.sort_by(|a, b| a.key.cmp(&b.key));
    let specs = column_specs();
    let row_count = entries.len();

    let recommit = {
        let widgets = widgets.clone();
        let state = state.clone();
        let on_change = on_change.clone();
        let toast_overlay = toast_overlay.clone();
        Rc::new(move || {
            on_change(None);
            refresh(&widgets, &state, &on_change, &toast_overlay);
        })
    };

    let add_row_trigger: Rc<dyn Fn()> = {
        let widgets = widgets.clone();
        let state = state.clone();
        let on_change = on_change.clone();
        let toast_overlay = toast_overlay.clone();
        Rc::new(move || add_row(&widgets, &state, &on_change, &toast_overlay))
    };

    for (row_idx, entry) in entries.iter().enumerate() {
        let grid_row = (row_idx + 1) as i32;
        let key_rc = Rc::new(RefCell::new(entry.key.clone()));

        let key_entry = make_cell(&entry.key, KEY_WIDTH_CHARS);
        if row_idx % 2 == 1 {
            key_entry.add_css_class("skrizhal-sheet-row-even");
        }
        widgets.grid.attach(&key_entry, 0, grid_row, 1, 1);
        widgets.data_widgets.borrow_mut().push(key_entry.clone().upcast());
        let commit_key = {
            let state = state.clone();
            let toast_overlay = toast_overlay.clone();
            let recommit = recommit.clone();
            let key_rc = key_rc.clone();
            let key_entry_clone = key_entry.clone();
            move |new_key: String| {
                let new_key = new_key.trim().to_string();
                let old_key = key_rc.borrow().clone();
                if new_key == old_key {
                    return;
                }
                if new_key.is_empty() {
                    key_entry_clone.set_text(&old_key);
                    toast_overlay.add_toast(adw::Toast::new("Key can't be empty."));
                    return;
                }
                let collides = state.borrow().entries.iter().any(|e| e.key == new_key);
                if collides {
                    key_entry_clone.set_text(&old_key);
                    toast_overlay.add_toast(adw::Toast::new(&format!(
                        "Key \"{new_key}\" is already used by another entry."
                    )));
                    return;
                }
                super::state::push_undo(&state);
                {
                    let mut s = state.borrow_mut();
                    if let Some(e) = s.entries.iter_mut().find(|e| e.key == old_key) {
                        e.key = new_key.clone();
                    }
                }
                *key_rc.borrow_mut() = new_key;
                recommit();
            }
        };
        wire_commit(&key_entry, commit_key.clone());
        wire_enter_tab(
            &key_entry,
            widgets,
            key_rc.clone(),
            None,
            specs.len(),
            commit_key,
            add_row_trigger.clone(),
        );

        let mut cell_entries = Vec::with_capacity(specs.len());
        for (col_idx, spec) in specs.iter().enumerate() {
            let value = (spec.get)(entry);
            let overlay = gtk4::Overlay::new();
            let cell_entry = make_cell(&value, spec.width_chars);
            // Leave room at the end so the fill handle sits clear of the
            // text instead of overlapping the last character or two.
            cell_entry.set_margin_end(12);
            if row_idx % 2 == 1 {
                cell_entry.add_css_class("skrizhal-sheet-row-even");
            }
            overlay.set_child(Some(&cell_entry));

            let handle = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
            handle.add_css_class("skrizhal-fill-handle");
            handle.set_halign(gtk4::Align::End);
            handle.set_valign(gtk4::Align::End);
            handle.set_margin_bottom(3);
            handle.set_cursor(gtk4::gdk::Cursor::from_name("crosshair", None).as_ref());
            overlay.add_overlay(&handle);

            let commit_cell = {
                let state = state.clone();
                let recommit = recommit.clone();
                let key_rc = key_rc.clone();
                let spec = spec.clone();
                move |value: String| {
                    let key = key_rc.borrow().clone();
                    // Skip the undo snapshot/refresh entirely if this cell's
                    // value didn't actually change — otherwise just Tabbing
                    // or pressing Enter through unedited cells (normal when
                    // navigating a row) would flood the undo stack with
                    // no-op entries.
                    let unchanged = state
                        .borrow()
                        .entries
                        .iter()
                        .find(|e| e.key == key)
                        .is_some_and(|e| (spec.get)(e) == value);
                    if unchanged {
                        return;
                    }
                    super::state::push_undo(&state);
                    {
                        let mut s = state.borrow_mut();
                        if let Some(e) = s.entries.iter_mut().find(|e| e.key == key) {
                            (spec.set)(e, &value);
                        }
                    }
                    recommit();
                }
            };
            wire_commit(&cell_entry, commit_cell.clone());
            wire_enter_tab(
                &cell_entry,
                widgets,
                key_rc.clone(),
                Some(col_idx),
                specs.len(),
                commit_cell,
                add_row_trigger.clone(),
            );

            attach_fill_handle(
                &handle,
                widgets.rows.clone(),
                row_idx,
                col_idx,
                row_count,
                state.clone(),
                recommit.clone(),
            );

            widgets.grid.attach(&overlay, (col_idx + 1) as i32, grid_row, 1, 1);
            widgets.data_widgets.borrow_mut().push(overlay.clone().upcast());
            cell_entries.push(cell_entry);
        }

        widgets.rows.borrow_mut().push(RowWidgets {
            key: key_rc,
            key_entry,
            cell_entries,
        });
    }
}

/// Appends a new blank entry (unique auto-generated key, otherwise default)
/// and focuses its Key cell — used by both the "+ Add Row" button and
/// pressing Enter in the last row.
pub fn add_row(
    widgets: &SpreadsheetWidgets,
    state: &SharedState,
    on_change: &ChangeCallback,
    toast_overlay: &adw::ToastOverlay,
) {
    super::state::push_undo(state);
    let new_key = {
        let mut s = state.borrow_mut();
        let key = skrizhal_core::unique_key("new-entry", &s.entries);
        s.entries.push(CvEntry {
            key: key.clone(),
            ..Default::default()
        });
        key
    };
    on_change(Some(new_key.clone()));
    refresh(widgets, state, on_change, toast_overlay);
    if let Some(row_idx) = row_index_for_key(&widgets.rows, &new_key) {
        focus_cell(widgets, row_idx, None);
    }
}

/// Attaches a spreadsheet-style fill-handle drag to `handle`: drag down (or
/// up) from a cell's corner and releasing copies that cell's current value
/// into every row the drag passed over, in that same column. Applied and
/// persisted once, on release — no live preview while dragging, to keep this
/// a reasonably-scoped first cut.
fn attach_fill_handle(
    handle: &gtk4::Box,
    rows: Rc<RefCell<Vec<RowWidgets>>>,
    row_index: usize,
    col_index: usize,
    row_count: usize,
    state: SharedState,
    on_commit: Rc<dyn Fn()>,
) {
    let drag = gtk4::GestureDrag::new();
    drag.connect_drag_end(move |_gesture, _offset_x, offset_y| {
        // Measure the actual cell's height, not the tiny fill-handle's own
        // (a few px) — using the handle's height here previously inflated
        // the row delta hugely, so almost any drag ended up spanning to the
        // very last row.
        let row_height = rows
            .borrow()
            .get(row_index)
            .map(|r| r.cell_entries[col_index].allocated_height() as f64)
            .filter(|h| *h > 0.0)
            .unwrap_or(36.0);
        let delta = (offset_y / row_height).round() as i32;
        let target = (row_index as i32 + delta).clamp(0, row_count as i32 - 1) as usize;
        let (lo, hi) = if target >= row_index {
            (row_index, target)
        } else {
            (target, row_index)
        };
        if lo == hi {
            return;
        }

        let specs = column_specs();
        let spec = &specs[col_index];
        let rows_ref = rows.borrow();
        let Some(source_row) = rows_ref.get(row_index) else { return };
        let value = source_row.cell_entries[col_index].text().to_string();

        super::state::push_undo(&state);
        {
            let mut s = state.borrow_mut();
            for i in lo..=hi {
                let Some(row) = rows_ref.get(i) else { continue };
                row.cell_entries[col_index].set_text(&value);
                let key = row.key.borrow().clone();
                if let Some(e) = s.entries.iter_mut().find(|e| e.key == key) {
                    (spec.set)(e, &value);
                }
            }
        }
        drop(rows_ref);
        on_commit();
    });
    handle.add_controller(drag);
}
