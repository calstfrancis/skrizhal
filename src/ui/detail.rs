use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use skrizhal_core::{slugify, unique_key, validate_entries, CvEntry, DateMode, Warning};

type ExtraRow = (gtk4::ListBoxRow, gtk4::Entry, gtk4::Entry);

#[derive(Clone)]
pub struct DetailWidgets {
    pub raw_toggle: gtk4::ToggleButton,
    pub warnings_label: gtk4::Label,
    pub key_row: adw::EntryRow,
    pub category_row: adw::EntryRow,
    pub title_row: adw::EntryRow,
    pub org_row: adw::EntryRow,
    pub location_row: adw::EntryRow,
    pub date_mode_dropdown: gtk4::DropDown,
    pub start_date_row: adw::EntryRow,
    pub end_date_row: adw::EntryRow,
    pub tags_row: adw::EntryRow,
    pub description_view: gtk4::TextView,
    pub extra_list: gtk4::ListBox,
    pub extra_rows: Rc<RefCell<Vec<ExtraRow>>>,
    pub raw_view: gtk4::TextView,
    pub save_button: gtk4::Button,
    pub outer_stack: gtk4::Stack,
    /// True while a brand-new (never-saved) entry's Key should keep following
    /// Title/Organization edits. Cleared the moment the user edits Key
    /// directly, or once the entry has gone through a normal (re)load.
    pub key_autogen_active: Rc<Cell<bool>>,
    /// Guards `key_row`'s own `changed` handler against the programmatic
    /// `set_text` calls `regenerate_key_if_autogen` makes, so auto-updates
    /// don't look like a manual edit and turn themselves off.
    pub suppress_key_change: Rc<Cell<bool>>,
}

fn entry_row(title: &str) -> adw::EntryRow {
    adw::EntryRow::builder().title(title).build()
}

/// Best-effort placeholder text for an `AdwEntryRow` — the widget has no
/// `placeholder-text` property of its own, but (like plain `GtkEntry`) it
/// delegates `GtkEditable` to an internal `GtkText`, which does.
fn set_placeholder(row: &adw::EntryRow, text: &str) {
    if let Some(text_widget) = row.delegate().and_then(|d| d.downcast::<gtk4::Text>().ok()) {
        text_widget.set_placeholder_text(Some(text));
    }
}

fn date_mode_index(mode: DateMode) -> u32 {
    match mode {
        DateMode::Single => 0,
        DateMode::Range => 1,
        DateMode::Ongoing => 2,
    }
}

fn date_mode_from_index(idx: u32) -> DateMode {
    match idx {
        1 => DateMode::Range,
        2 => DateMode::Ongoing,
        _ => DateMode::Single,
    }
}

fn add_extra_row(
    extra_list: &gtk4::ListBox,
    rows: &Rc<RefCell<Vec<ExtraRow>>>,
    key: &str,
    value: &str,
) {
    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    hbox.set_margin_top(4);
    hbox.set_margin_bottom(4);
    hbox.set_margin_start(6);
    hbox.set_margin_end(6);
    let key_entry = gtk4::Entry::builder()
        .placeholder_text("field")
        .text(key)
        .hexpand(false)
        .width_chars(14)
        .build();
    let value_entry = gtk4::Entry::builder()
        .placeholder_text("value")
        .text(value)
        .hexpand(true)
        .build();
    let remove_btn = gtk4::Button::from_icon_name("list-remove-symbolic");
    remove_btn.add_css_class("flat");
    hbox.append(&key_entry);
    hbox.append(&value_entry);
    hbox.append(&remove_btn);

    extra_list.append(&hbox);
    let row = extra_list
        .last_child()
        .and_downcast::<gtk4::ListBoxRow>()
        .expect("ListBox wraps appended child in a ListBoxRow");
    rows.borrow_mut()
        .push((row.clone(), key_entry.clone(), value_entry.clone()));

    let rows_for_remove = rows.clone();
    let extra_list_for_remove = extra_list.clone();
    remove_btn.connect_clicked(move |_| {
        let row_to_remove = {
            let rows = rows_for_remove.borrow();
            rows.iter()
                .find(|(r, k, _)| k == &key_entry && r.parent().is_some())
                .map(|(r, _, _)| r.clone())
        };
        if let Some(row) = row_to_remove {
            extra_list_for_remove.remove(&row);
            rows_for_remove.borrow_mut().retain(|(r, _, _)| r != &row);
        }
    });
}

pub fn clear_extra_rows(widgets: &DetailWidgets) {
    while let Some(child) = widgets.extra_list.first_child() {
        widgets.extra_list.remove(&child);
    }
    widgets.extra_rows.borrow_mut().clear();
}

pub fn build() -> DetailWidgets {
    let outer_stack = gtk4::Stack::new();

    let empty_placeholder = adw::StatusPage::builder()
        .icon_name("document-edit-symbolic")
        .title("No Entry Selected")
        .description("Select an entry from the list, or add a new one.")
        .build();
    outer_stack.add_named(&empty_placeholder, Some("empty"));

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let top_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let warnings_label = gtk4::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .hexpand(true)
        .build();
    let raw_toggle = gtk4::ToggleButton::builder().label("Raw YAML").build();
    top_bar.append(&warnings_label);
    top_bar.append(&raw_toggle);
    root.append(&top_bar);

    let stack = gtk4::Stack::new();

    // ── Structured form ──────────────────────────────────────────────
    let form_scroll = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();
    let form_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);

    let common_group = adw::PreferencesGroup::builder().title("Entry").build();
    let key_row = entry_row("Key");
    let category_row = entry_row("Category");
    set_placeholder(&category_row, "Education, Employment, Awards, etc...");

    let category_suggest = gtk4::MenuButton::builder()
        .icon_name("pan-down-symbolic")
        .valign(gtk4::Align::Center)
        .css_classes(["flat"])
        .build();
    let category_popover = gtk4::Popover::new();
    let category_popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    for spec in skrizhal_core::CATEGORY_REGISTRY {
        let btn = gtk4::Button::builder()
            .label(spec.name)
            .css_classes(["flat"])
            .build();
        let category_row_clone = category_row.clone();
        let popover_clone = category_popover.clone();
        btn.connect_clicked(move |_| {
            category_row_clone.set_text(spec.name);
            popover_clone.popdown();
        });
        category_popover_box.append(&btn);
    }
    category_popover.set_child(Some(&category_popover_box));
    category_suggest.set_popover(Some(&category_popover));
    category_row.add_suffix(&category_suggest);

    let title_row = entry_row("Title");
    let org_row = entry_row("Organization");
    let location_row = entry_row("Location");
    let tags_row = entry_row("Tags (comma-separated)");

    common_group.add(&key_row);
    common_group.add(&category_row);
    common_group.add(&title_row);
    common_group.add(&org_row);
    common_group.add(&location_row);
    form_box.append(&common_group);

    // ── Date: mode dropdown + start/end rows ──────────────────────────
    let date_group = adw::PreferencesGroup::builder().title("Date").build();
    let date_mode_dropdown =
        gtk4::DropDown::from_strings(&["Single Date", "Date Range", "Ongoing"]);
    let date_mode_row = adw::ActionRow::builder().title("Date Type").build();
    date_mode_row.add_suffix(&date_mode_dropdown);
    let start_date_row = entry_row("Date");
    let end_date_row = entry_row("End Date");
    end_date_row.set_visible(false);
    date_group.add(&date_mode_row);
    date_group.add(&start_date_row);
    date_group.add(&end_date_row);
    form_box.append(&date_group);

    {
        let start_date_row = start_date_row.clone();
        let end_date_row = end_date_row.clone();
        date_mode_dropdown.connect_selected_notify(move |dd| {
            let mode = date_mode_from_index(dd.selected());
            end_date_row.set_visible(mode == DateMode::Range);
            start_date_row.set_title(if mode == DateMode::Single {
                "Date"
            } else {
                "Start Date"
            });
        });
    }

    common_group.add(&tags_row);

    let desc_label = gtk4::Label::builder()
        .label("Description (one bullet per line)")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    form_box.append(&desc_label);
    let description_view = gtk4::TextView::builder()
        .wrap_mode(gtk4::WrapMode::WordChar)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(6)
        .right_margin(6)
        .build();
    let desc_frame = gtk4::Frame::builder().child(&description_view).build();
    form_box.append(&desc_frame);

    let extra_label_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let extra_label = gtk4::Label::builder()
        .label("Additional Fields")
        .xalign(0.0)
        .hexpand(true)
        .css_classes(["heading"])
        .build();
    let add_field_button = gtk4::Button::from_icon_name("list-add-symbolic");
    add_field_button.set_tooltip_text(Some("Add Field"));
    extra_label_row.append(&extra_label);
    extra_label_row.append(&add_field_button);
    form_box.append(&extra_label_row);

    let extra_list = gtk4::ListBox::new();
    extra_list.set_selection_mode(gtk4::SelectionMode::None);
    extra_list.add_css_class("boxed-list");
    form_box.append(&extra_list);

    form_scroll.set_child(Some(&form_box));
    stack.add_named(&form_scroll, Some("form"));

    // ── Raw YAML view ────────────────────────────────────────────────
    let raw_view = gtk4::TextView::builder()
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::WordChar)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(6)
        .right_margin(6)
        .build();
    let raw_scroll = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .child(&raw_view)
        .build();
    stack.add_named(&raw_scroll, Some("raw"));

    stack.set_visible_child_name("form");
    root.append(&stack);

    let save_button = gtk4::Button::builder()
        .label("Save")
        .css_classes(["suggested-action"])
        .halign(gtk4::Align::End)
        .build();
    root.append(&save_button);

    outer_stack.add_named(&root, Some("entry"));
    outer_stack.set_visible_child_name("empty");

    let raw_toggle_clone = raw_toggle.clone();
    let stack_clone = stack.clone();
    raw_toggle_clone.connect_toggled(move |btn| {
        stack_clone.set_visible_child_name(if btn.is_active() { "raw" } else { "form" });
    });

    let extra_rows: Rc<RefCell<Vec<ExtraRow>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let extra_list_clone = extra_list.clone();
        let rows_clone = extra_rows.clone();
        add_field_button.connect_clicked(move |_| {
            add_extra_row(&extra_list_clone, &rows_clone, "", "");
        });
    }

    DetailWidgets {
        raw_toggle,
        warnings_label,
        key_row,
        category_row,
        title_row,
        org_row,
        location_row,
        date_mode_dropdown,
        start_date_row,
        end_date_row,
        tags_row,
        description_view,
        extra_list,
        extra_rows,
        raw_view,
        save_button,
        outer_stack,
        key_autogen_active: Rc::new(Cell::new(false)),
        suppress_key_change: Rc::new(Cell::new(false)),
    }
}

pub fn show_empty(widgets: &DetailWidgets) {
    widgets.outer_stack.set_visible_child_name("empty");
}

fn text_of(view: &gtk4::TextView) -> String {
    let buf = view.buffer();
    buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string()
}

fn set_text_of(view: &gtk4::TextView, text: &str) {
    view.buffer().set_text(text);
}

/// Loads `entry` into the form. `is_new` marks a just-created, never-saved
/// entry — only then does the Key field start auto-following Title/Organization
/// edits (see `regenerate_key_if_autogen`); loading any existing entry always
/// leaves its key alone.
pub fn load_entry(widgets: &DetailWidgets, entry: &CvEntry, is_new: bool) {
    widgets.outer_stack.set_visible_child_name("entry");
    widgets.suppress_key_change.set(true);
    widgets.key_row.set_text(&entry.key);
    widgets.suppress_key_change.set(false);
    widgets.key_row.remove_css_class("error");
    widgets.key_autogen_active.set(is_new);

    widgets.category_row.set_text(&entry.category);
    widgets.title_row.set_text(&entry.title);
    widgets.org_row.set_text(entry.organization.as_deref().unwrap_or(""));
    widgets.location_row.set_text(entry.location.as_deref().unwrap_or(""));

    let (mode, start, end) = skrizhal_core::split_date_string(entry.date.as_deref().unwrap_or(""));
    widgets.date_mode_dropdown.set_selected(date_mode_index(mode));
    widgets.start_date_row.set_title(if mode == DateMode::Single { "Date" } else { "Start Date" });
    widgets.start_date_row.set_text(&start);
    widgets.end_date_row.set_text(&end);
    widgets.end_date_row.set_visible(mode == DateMode::Range);

    widgets.tags_row.set_text(&entry.tags.join(", "));
    set_text_of(&widgets.description_view, &entry.description.join("\n"));

    clear_extra_rows(widgets);
    for (k, v) in &entry.extra {
        let value_str = match v {
            serde_yaml_ng::Value::String(s) => s.clone(),
            other => serde_yaml_ng::to_string(other)
                .unwrap_or_default()
                .trim()
                .to_string(),
        };
        add_extra_row(&widgets.extra_list, &widgets.extra_rows, k, &value_str);
    }

    let raw = skrizhal_core::to_yaml_string(std::slice::from_ref(entry)).unwrap_or_default();
    set_text_of(&widgets.raw_view, &raw);

    update_warnings(widgets, entry);
}

pub fn update_warnings(widgets: &DetailWidgets, entry: &CvEntry) {
    let warnings = validate_entries(std::slice::from_ref(entry));
    if warnings.is_empty() {
        widgets.warnings_label.set_label("");
        return;
    }
    let text: Vec<String> = warnings
        .iter()
        .map(|w| match w {
            Warning::UnknownCategory { category, .. } => {
                format!("Unrecognized category \"{category}\"")
            }
            Warning::MissingRecommendedField { field, .. } => {
                format!("Missing recommended field \"{field}\"")
            }
            Warning::DuplicateKey { key } => format!("Duplicate key \"{key}\""),
        })
        .collect();
    widgets.warnings_label.set_label(&text.join("  ·  "));
}

/// Reads the structured form into a `CvEntry`. Never fails — blank fields
/// just become empty/`None`.
pub fn read_form(widgets: &DetailWidgets) -> CvEntry {
    let tags: Vec<String> = widgets
        .tags_row
        .text()
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    let description: Vec<String> = text_of(&widgets.description_view)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let mut extra = std::collections::BTreeMap::new();
    for (_, key_entry, value_entry) in widgets.extra_rows.borrow().iter() {
        let key = key_entry.text().to_string();
        if key.trim().is_empty() {
            continue;
        }
        extra.insert(
            key.trim().to_string(),
            serde_yaml_ng::Value::String(value_entry.text().to_string()),
        );
    }

    let opt = |s: String| if s.trim().is_empty() { None } else { Some(s) };

    let mode = date_mode_from_index(widgets.date_mode_dropdown.selected());
    let date = opt(skrizhal_core::join_date_string(
        mode,
        &widgets.start_date_row.text(),
        &widgets.end_date_row.text(),
    ));

    CvEntry {
        key: widgets.key_row.text().trim().to_string(),
        category: widgets.category_row.text().trim().to_string(),
        title: widgets.title_row.text().trim().to_string(),
        organization: opt(widgets.org_row.text().to_string()),
        location: opt(widgets.location_row.text().to_string()),
        date,
        tags,
        description,
        extra,
    }
}

/// Parses the raw-YAML pane's text, expecting exactly one entry.
pub fn read_raw(widgets: &DetailWidgets) -> Result<CvEntry, String> {
    let text = text_of(&widgets.raw_view);
    let mut entries = skrizhal_core::parse_str(&text).map_err(|e| e.to_string())?;
    match entries.len() {
        1 => Ok(entries.remove(0)),
        0 => Err("No entry found — expected one top-level key.".to_string()),
        n => Err(format!("Expected exactly one entry, found {n}.")),
    }
}

pub fn is_raw_active(widgets: &DetailWidgets) -> bool {
    widgets.raw_toggle.is_active()
}

/// If Key is still in auto-follow mode, regenerates it from Organization +
/// Title (`slugify("{org} {title}")`), keeping it unique against every other
/// entry (excluding whichever entry is currently loaded, so re-deriving the
/// same key as it already has doesn't look like a collision).
pub fn regenerate_key_if_autogen(
    widgets: &DetailWidgets,
    existing: &[CvEntry],
    original_key: Option<&str>,
) {
    if !widgets.key_autogen_active.get() {
        return;
    }
    let org = widgets.org_row.text();
    let title = widgets.title_row.text();
    let base = if org.trim().is_empty() {
        slugify(&title)
    } else {
        slugify(&format!("{org} {title}"))
    };
    let base = if base.is_empty() { "entry".to_string() } else { base };
    let others: Vec<CvEntry> = existing
        .iter()
        .filter(|e| Some(e.key.as_str()) != original_key)
        .cloned()
        .collect();
    let candidate = unique_key(&base, &others);

    widgets.suppress_key_change.set(true);
    widgets.key_row.set_text(&candidate);
    widgets.suppress_key_change.set(false);
    update_key_error_state(widgets, existing, original_key);
}

/// Called on every `key_row` text change. A real (non-programmatic) edit
/// means the user has taken manual control — auto-generation stops.
pub fn mark_key_dirty_if_user_edit(widgets: &DetailWidgets) {
    if widgets.suppress_key_change.get() {
        return;
    }
    widgets.key_autogen_active.set(false);
}

pub fn key_is_valid(widgets: &DetailWidgets, existing: &[CvEntry], original_key: Option<&str>) -> bool {
    let key = widgets.key_row.text();
    let key = key.trim();
    if key.is_empty() {
        return false;
    }
    !existing
        .iter()
        .any(|e| e.key == key && Some(e.key.as_str()) != original_key)
}

/// Live visual feedback for an empty/duplicate key — doesn't block typing,
/// but makes it obvious before Save (which hard-rejects it) ever runs.
pub fn update_key_error_state(widgets: &DetailWidgets, existing: &[CvEntry], original_key: Option<&str>) {
    if key_is_valid(widgets, existing, original_key) {
        widgets.key_row.remove_css_class("error");
    } else {
        widgets.key_row.add_css_class("error");
    }
}
