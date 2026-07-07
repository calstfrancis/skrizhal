use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use skrizhal::{validate_entries, CvEntry, Warning};

type ExtraRow = (gtk4::ListBoxRow, gtk4::Entry, gtk4::Entry);

#[derive(Clone)]
pub struct DetailWidgets {
    pub raw_toggle: gtk4::ToggleButton,
    pub warnings_label: gtk4::Label,
    pub key_row: adw::EntryRow,
    pub type_row: adw::EntryRow,
    pub title_row: adw::EntryRow,
    pub org_row: adw::EntryRow,
    pub location_row: adw::EntryRow,
    pub date_row: adw::EntryRow,
    pub tags_row: adw::EntryRow,
    pub description_view: gtk4::TextView,
    pub extra_list: gtk4::ListBox,
    pub extra_rows: Rc<RefCell<Vec<ExtraRow>>>,
    pub raw_view: gtk4::TextView,
    pub save_button: gtk4::Button,
    pub outer_stack: gtk4::Stack,
}

fn entry_row(title: &str) -> adw::EntryRow {
    adw::EntryRow::builder().title(title).build()
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
    let type_row = entry_row("Type");

    let type_suggest = gtk4::MenuButton::builder()
        .icon_name("pan-down-symbolic")
        .valign(gtk4::Align::Center)
        .css_classes(["flat"])
        .build();
    let type_popover = gtk4::Popover::new();
    let type_popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    for spec in skrizhal::TYPE_REGISTRY {
        let btn = gtk4::Button::builder()
            .label(spec.id)
            .css_classes(["flat"])
            .build();
        let type_row_clone = type_row.clone();
        let popover_clone = type_popover.clone();
        btn.connect_clicked(move |_| {
            type_row_clone.set_text(spec.id);
            popover_clone.popdown();
        });
        type_popover_box.append(&btn);
    }
    type_popover.set_child(Some(&type_popover_box));
    type_suggest.set_popover(Some(&type_popover));
    type_row.add_suffix(&type_suggest);

    let title_row = entry_row("Title");
    let org_row = entry_row("Organization");
    let location_row = entry_row("Location");
    let date_row = entry_row("Date");
    date_row.set_show_apply_button(false);
    let tags_row = entry_row("Tags (comma-separated)");

    common_group.add(&key_row);
    common_group.add(&type_row);
    common_group.add(&title_row);
    common_group.add(&org_row);
    common_group.add(&location_row);
    common_group.add(&date_row);
    common_group.add(&tags_row);
    form_box.append(&common_group);

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
        type_row,
        title_row,
        org_row,
        location_row,
        date_row,
        tags_row,
        description_view,
        extra_list,
        extra_rows,
        raw_view,
        save_button,
        outer_stack,
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

pub fn load_entry(widgets: &DetailWidgets, entry: &CvEntry) {
    widgets.outer_stack.set_visible_child_name("entry");
    widgets.key_row.set_text(&entry.key);
    widgets.type_row.set_text(&entry.entry_type);
    widgets.title_row.set_text(&entry.title);
    widgets.org_row.set_text(entry.organization.as_deref().unwrap_or(""));
    widgets.location_row.set_text(entry.location.as_deref().unwrap_or(""));
    widgets.date_row.set_text(entry.date.as_deref().unwrap_or(""));
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

    let raw = skrizhal::to_yaml_string(std::slice::from_ref(entry)).unwrap_or_default();
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
            Warning::UnknownType { entry_type, .. } => {
                format!("Unrecognized type \"{entry_type}\"")
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

    CvEntry {
        key: widgets.key_row.text().trim().to_string(),
        entry_type: widgets.type_row.text().trim().to_string(),
        title: widgets.title_row.text().trim().to_string(),
        organization: opt(widgets.org_row.text().to_string()),
        location: opt(widgets.location_row.text().to_string()),
        date: opt(widgets.date_row.text().to_string()),
        tags,
        description,
        extra,
    }
}

/// Parses the raw-YAML pane's text, expecting exactly one entry.
pub fn read_raw(widgets: &DetailWidgets) -> Result<CvEntry, String> {
    let text = text_of(&widgets.raw_view);
    let mut entries = skrizhal::parse_str(&text).map_err(|e| e.to_string())?;
    match entries.len() {
        1 => Ok(entries.remove(0)),
        0 => Err("No entry found — expected one top-level key.".to_string()),
        n => Err(format!("Expected exactly one entry, found {n}.")),
    }
}

pub fn is_raw_active(widgets: &DetailWidgets) -> bool {
    widgets.raw_toggle.is_active()
}
