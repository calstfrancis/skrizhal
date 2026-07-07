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
    get: Rc<dyn Fn(&CvEntry) -> String>,
    set: ColumnSetter,
}

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
            get: Rc::new(|e| e.category.clone()),
            set: Rc::new(|e, v| e.category = v.trim().to_string()),
        },
        ColumnSpec {
            label: "Title",
            get: Rc::new(|e| e.title.clone()),
            set: Rc::new(|e, v| e.title = v.trim().to_string()),
        },
        ColumnSpec {
            label: "Organization",
            get: Rc::new(|e| e.organization.clone().unwrap_or_default()),
            set: Rc::new(|e, v| e.organization = opt(v)),
        },
        ColumnSpec {
            label: "Location",
            get: Rc::new(|e| e.location.clone().unwrap_or_default()),
            set: Rc::new(|e, v| e.location = opt(v)),
        },
        ColumnSpec {
            label: "Date",
            get: Rc::new(|e| e.date.clone().unwrap_or_default()),
            set: Rc::new(|e, v| e.date = opt(v)),
        },
        ColumnSpec {
            label: "Tags",
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
    cell_entries: Vec<gtk4::Entry>,
}

#[derive(Clone)]
pub struct SpreadsheetWidgets {
    pub root: gtk4::ScrolledWindow,
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
    grid.set_row_spacing(2);
    grid.set_column_spacing(6);
    grid.set_margin_top(8);
    grid.set_margin_bottom(8);
    grid.set_margin_start(8);
    grid.set_margin_end(8);

    let key_header = gtk4::Label::builder().label("Key").css_classes(["heading"]).xalign(0.0).build();
    grid.attach(&key_header, 0, 0, 1, 1);
    for (i, spec) in column_specs().iter().enumerate() {
        let header = gtk4::Label::builder()
            .label(spec.label)
            .css_classes(["heading"])
            .xalign(0.0)
            .build();
        grid.attach(&header, (i + 1) as i32, 0, 1, 1);
    }

    let root = gtk4::ScrolledWindow::builder().vexpand(true).hexpand(true).child(&grid).build();

    SpreadsheetWidgets {
        root,
        grid,
        rows: Rc::new(RefCell::new(Vec::new())),
        data_widgets: Rc::new(RefCell::new(Vec::new())),
    }
}

fn make_cell(initial: &str) -> gtk4::Entry {
    gtk4::Entry::builder().text(initial).hexpand(true).build()
}

/// Wires Enter and focus-out to call `commit` with the entry's current text.
fn wire_commit(entry: &gtk4::Entry, commit: impl Fn(String) + 'static + Clone) {
    {
        let commit = commit.clone();
        entry.connect_activate(move |e| commit(e.text().to_string()));
    }
    let focus = gtk4::EventControllerFocus::new();
    {
        let entry = entry.clone();
        focus.connect_leave(move |_| commit(entry.text().to_string()));
    }
    entry.add_controller(focus);
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

    for (row_idx, entry) in entries.iter().enumerate() {
        let grid_row = (row_idx + 1) as i32;
        let key_rc = Rc::new(RefCell::new(entry.key.clone()));

        let key_entry = make_cell(&entry.key);
        widgets.grid.attach(&key_entry, 0, grid_row, 1, 1);
        widgets.data_widgets.borrow_mut().push(key_entry.clone().upcast());
        {
            let state = state.clone();
            let toast_overlay = toast_overlay.clone();
            let recommit = recommit.clone();
            let key_rc = key_rc.clone();
            let key_entry_clone = key_entry.clone();
            wire_commit(&key_entry, move |new_key| {
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
                {
                    let mut s = state.borrow_mut();
                    if let Some(e) = s.entries.iter_mut().find(|e| e.key == old_key) {
                        e.key = new_key.clone();
                    }
                }
                *key_rc.borrow_mut() = new_key;
                recommit();
            });
        }

        let mut cell_entries = Vec::with_capacity(specs.len());
        for (col_idx, spec) in specs.iter().enumerate() {
            let value = (spec.get)(entry);
            let overlay = gtk4::Overlay::new();
            let cell_entry = make_cell(&value);
            overlay.set_child(Some(&cell_entry));

            let handle = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
            handle.add_css_class("skrizhal-fill-handle");
            handle.set_halign(gtk4::Align::End);
            handle.set_valign(gtk4::Align::End);
            handle.set_cursor(gtk4::gdk::Cursor::from_name("crosshair", None).as_ref());
            overlay.add_overlay(&handle);

            {
                let state = state.clone();
                let recommit = recommit.clone();
                let key_rc = key_rc.clone();
                let spec = spec.clone();
                wire_commit(&cell_entry, move |value| {
                    {
                        let mut s = state.borrow_mut();
                        let key = key_rc.borrow().clone();
                        if let Some(e) = s.entries.iter_mut().find(|e| e.key == key) {
                            (spec.set)(e, &value);
                        }
                    }
                    recommit();
                });
            }

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
            cell_entries,
        });
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
