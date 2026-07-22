use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use sourceview5::prelude::*;

use skrizhal_core::{slugify, unique_key, validate_entries, CvEntry, DateMode, Warning};

type ExtraRow = (gtk4::ListBoxRow, gtk4::Entry, gtk4::Entry);
/// One description bullet: its list row and the entry holding its text.
pub type DescriptionRow = (gtk4::ListBoxRow, gtk4::Entry);
/// Slot for the "a field changed" callback `app_window` installs to drive
/// autosave. Optional because the form is built before that wiring exists.
type DirtyHook = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

#[derive(Clone)]
pub struct DetailWidgets {
    pub raw_toggle: gtk4::ToggleButton,
    pub warnings_label: gtk4::Label,
    pub key_row: adw::EntryRow,
    pub category_row: adw::EntryRow,
    pub category_warning: gtk4::Image,
    pub title_row: adw::EntryRow,
    pub org_row: adw::EntryRow,
    pub org_warning: gtk4::Image,
    pub location_row: adw::EntryRow,
    pub location_warning: gtk4::Image,
    pub date_mode_dropdown: gtk4::DropDown,
    pub start_date_row: adw::EntryRow,
    pub date_warning: gtk4::Image,
    pub end_date_row: adw::EntryRow,
    pub tags_row: adw::EntryRow,
    pub tags_warning: gtk4::Image,
    pub tags_suggest_popover: gtk4::Popover,
    pub tags_suggest_box: gtk4::Box,
    pub description_list: gtk4::ListBox,
    pub description_rows: Rc<RefCell<Vec<DescriptionRow>>>,
    pub extra_list: gtk4::ListBox,
    pub extra_rows: Rc<RefCell<Vec<ExtraRow>>>,
    pub raw_view: sourceview5::View,
    pub save_button: gtk4::Button,
    pub outer_stack: gtk4::Stack,
    pub empty_add_button: gtk4::Button,
    /// True while a brand-new (never-saved) entry's Key should keep following
    /// Title/Organization edits. Cleared the moment the user edits Key
    /// directly, or once the entry has gone through a normal (re)load.
    pub key_autogen_active: Rc<Cell<bool>>,
    /// Guards `key_row`'s own `changed` handler against the programmatic
    /// `set_text` calls `regenerate_key_if_autogen` makes, so auto-updates
    /// don't look like a manual edit and turn themselves off.
    pub suppress_key_change: Rc<Cell<bool>>,
    /// Fired whenever any field changes, so `app_window` can schedule an
    /// autosave. Held in a cell because Additional Fields rows are created
    /// dynamically — long after `build()` — and each needs to hook into the
    /// same callback the fixed rows use.
    pub on_dirty: DirtyHook,
}

impl DetailWidgets {
    fn notify_dirty(on_dirty: &DirtyHook) {
        let cb = on_dirty.borrow().clone();
        if let Some(cb) = cb {
            cb();
        }
    }
}

fn entry_row(title: &str) -> adw::EntryRow {
    adw::EntryRow::builder().title(title).build()
}

/// A hidden-by-default warning suffix icon for an `AdwEntryRow`, shown with
/// a tooltip when `update_warnings` finds an issue for that specific field.
fn field_warning_icon() -> gtk4::Image {
    gtk4::Image::builder()
        .icon_name("dialog-warning-symbolic")
        .css_classes(["warning"])
        .valign(gtk4::Align::Center)
        .visible(false)
        .build()
}

/// Best-effort placeholder text for an `AdwEntryRow` — the widget has no
/// `placeholder-text` property of its own, but (like plain `GtkEntry`) it
/// delegates `GtkEditable` to an internal `GtkText`, which does.
fn set_placeholder(row: &adw::EntryRow, text: &str) {
    if let Some(text_widget) = row.delegate().and_then(|d| d.downcast::<gtk4::Text>().ok()) {
        text_widget.set_placeholder_text(Some(text));
    }
}

/// Selects the field's full contents on focus (e.g. via Tab), so typing
/// immediately replaces the value instead of requiring a manual select-all.
fn select_all_on_focus(row: &adw::EntryRow) {
    if let Some(text_widget) = row.delegate().and_then(|d| d.downcast::<gtk4::Text>().ok()) {
        let focus_controller = gtk4::EventControllerFocus::new();
        let text_widget = text_widget.clone();
        focus_controller.connect_enter(move |_| {
            text_widget.select_region(0, -1);
        });
        row.add_controller(focus_controller);
    }
}

/// GtkSourceView ships "Adwaita"/"Adwaita-dark" schemes specifically so
/// embedders can follow the system color scheme without hand-picking colors.
fn apply_source_style_scheme(buffer: &sourceview5::Buffer) {
    let scheme_id = if adw::StyleManager::default().is_dark() {
        "Adwaita-dark"
    } else {
        "Adwaita"
    };
    let manager = sourceview5::StyleSchemeManager::default();
    if let Some(scheme) = manager.scheme(scheme_id) {
        buffer.set_style_scheme(Some(&scheme));
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
    on_dirty: &DirtyHook,
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

    for entry in [&key_entry, &value_entry] {
        let on_dirty = on_dirty.clone();
        entry.connect_changed(move |_| DetailWidgets::notify_dirty(&on_dirty));
    }

    let rows_for_remove = rows.clone();
    let extra_list_for_remove = extra_list.clone();
    let on_dirty_for_remove = on_dirty.clone();
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
            DetailWidgets::notify_dirty(&on_dirty_for_remove);
        }
    });
}

/// Adds an empty Additional Fields row for each field the category registry
/// recommends but the entry doesn't have yet — so choosing "Education"
/// immediately offers a `degree` row instead of leaving the user to know
/// that's the expected field name.
///
/// The registry has driven validation warnings since it was written; this
/// just puts the same knowledge in front of the user *before* they get
/// warned about it. Rows stay empty until filled and are dropped on save.
pub fn suggest_category_fields(widgets: &DetailWidgets, category: &str) {
    let existing: Vec<String> = widgets
        .extra_rows
        .borrow()
        .iter()
        .map(|(_, k, _)| k.text().to_string().trim().to_string())
        .collect();
    for field in skrizhal_core::category_specific_fields(category) {
        if skrizhal_core::COMMON_FIELDS.contains(field) || existing.iter().any(|e| e == field) {
            continue;
        }
        add_extra_row(
            &widgets.extra_list,
            &widgets.extra_rows,
            &widgets.on_dirty,
            field,
            "",
        );
    }
}

/// Appends one bullet row. Bullets are the highest-churn, highest-value
/// content on a CV, so each gets its own row with reorder and remove
/// controls plus a live character count — a single wrapped text blob made
/// all three of those impossible.
pub fn add_description_row(
    list: &gtk4::ListBox,
    rows: &Rc<RefCell<Vec<DescriptionRow>>>,
    on_dirty: &DirtyHook,
    text: &str,
) -> gtk4::Entry {
    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    hbox.set_margin_top(4);
    hbox.set_margin_bottom(4);
    hbox.set_margin_start(6);
    hbox.set_margin_end(6);

    let entry = gtk4::Entry::builder()
        .placeholder_text("Accomplishment or responsibility")
        .text(text)
        .hexpand(true)
        .build();
    let count_label = gtk4::Label::builder()
        .css_classes(["dim-label", "caption", "numeric"])
        .width_chars(4)
        .xalign(1.0)
        .build();
    let up_btn = gtk4::Button::from_icon_name("go-up-symbolic");
    let down_btn = gtk4::Button::from_icon_name("go-down-symbolic");
    let remove_btn = gtk4::Button::from_icon_name("list-remove-symbolic");
    for (btn, tip) in [
        (&up_btn, "Move Up"),
        (&down_btn, "Move Down"),
        (&remove_btn, "Remove Bullet"),
    ] {
        btn.add_css_class("flat");
        btn.set_tooltip_text(Some(tip));
        btn.set_valign(gtk4::Align::Center);
    }

    hbox.append(&entry);
    hbox.append(&count_label);
    hbox.append(&up_btn);
    hbox.append(&down_btn);
    hbox.append(&remove_btn);
    list.append(&hbox);

    let row = list
        .last_child()
        .and_downcast::<gtk4::ListBoxRow>()
        .expect("ListBox wraps appended child in a ListBoxRow");
    rows.borrow_mut().push((row.clone(), entry.clone()));

    let update_count = {
        let count_label = count_label.clone();
        move |e: &gtk4::Entry| {
            let n = e.text().chars().count();
            count_label.set_label(&if n == 0 { String::new() } else { n.to_string() });
        }
    };
    update_count(&entry);
    {
        let on_dirty = on_dirty.clone();
        entry.connect_changed(move |e| {
            update_count(e);
            DetailWidgets::notify_dirty(&on_dirty);
        });
    }

    // Reordering swaps the text rather than the widgets: the rows are already
    // in the list in order, and moving GTK children around mid-signal is a
    // reliable way to invalidate the very iteration that triggered it.
    for (btn, delta) in [(&up_btn, -1i32), (&down_btn, 1i32)] {
        let rows = rows.clone();
        let entry = entry.clone();
        let on_dirty = on_dirty.clone();
        btn.connect_clicked(move |_| {
            let rows_ref = rows.borrow();
            let Some(index) = rows_ref.iter().position(|(_, e)| e == &entry) else {
                return;
            };
            let target = index as i32 + delta;
            if target < 0 || target as usize >= rows_ref.len() {
                return;
            }
            let other = rows_ref[target as usize].1.clone();
            let this_text = entry.text().to_string();
            let other_text = other.text().to_string();
            drop(rows_ref);
            entry.set_text(&other_text);
            other.set_text(&this_text);
            other.grab_focus();
            DetailWidgets::notify_dirty(&on_dirty);
        });
    }
    {
        let rows = rows.clone();
        let list = list.clone();
        let entry = entry.clone();
        let on_dirty = on_dirty.clone();
        remove_btn.connect_clicked(move |_| {
            let target = rows
                .borrow()
                .iter()
                .find(|(_, e)| e == &entry)
                .map(|(r, _)| r.clone());
            if let Some(row) = target {
                list.remove(&row);
                rows.borrow_mut().retain(|(r, _)| r != &row);
                DetailWidgets::notify_dirty(&on_dirty);
            }
        });
    }
    entry
}

pub fn clear_description_rows(widgets: &DetailWidgets) {
    while let Some(child) = widgets.description_list.first_child() {
        widgets.description_list.remove(&child);
    }
    widgets.description_rows.borrow_mut().clear();
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
    let empty_add_button = gtk4::Button::builder()
        .label("Add Entry")
        .halign(gtk4::Align::Center)
        .css_classes(["suggested-action", "pill"])
        .build();
    empty_placeholder.set_child(Some(&empty_add_button));
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
    let category_warning = field_warning_icon();
    category_row.add_suffix(&category_warning);

    let title_row = entry_row("Title");
    let org_row = entry_row("Organization");
    let org_warning = field_warning_icon();
    org_row.add_suffix(&org_warning);
    let location_row = entry_row("Location");
    let location_warning = field_warning_icon();
    location_row.add_suffix(&location_warning);
    let tags_row = entry_row("Tags (comma-separated)");
    let tags_suggest = gtk4::MenuButton::builder()
        .icon_name("pan-down-symbolic")
        .valign(gtk4::Align::Center)
        .css_classes(["flat"])
        .tooltip_text("Recently used tags")
        .build();
    let tags_suggest_popover = gtk4::Popover::new();
    let tags_suggest_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    tags_suggest_popover.set_child(Some(&tags_suggest_box));
    tags_suggest.set_popover(Some(&tags_suggest_popover));
    tags_row.add_suffix(&tags_suggest);
    let tags_warning = field_warning_icon();
    tags_row.add_suffix(&tags_warning);

    for row in [&key_row, &category_row, &title_row, &org_row, &location_row, &tags_row] {
        select_all_on_focus(row);
    }

    common_group.add(&key_row);
    common_group.add(&category_row);
    common_group.add(&title_row);
    common_group.add(&org_row);
    common_group.add(&location_row);

    // ── Date: mode dropdown + start/end rows, folded into the same group ──
    let date_mode_dropdown =
        gtk4::DropDown::from_strings(&["Single Date", "Date Range", "Ongoing"]);
    let date_mode_row = adw::ActionRow::builder().title("Date Type").build();
    date_mode_row.add_suffix(&date_mode_dropdown);
    let start_date_row = entry_row("Date");
    let date_warning = field_warning_icon();
    start_date_row.add_suffix(&date_warning);
    let end_date_row = entry_row("End Date");
    end_date_row.set_visible(false);
    select_all_on_focus(&start_date_row);
    select_all_on_focus(&end_date_row);
    common_group.add(&date_mode_row);
    common_group.add(&start_date_row);
    common_group.add(&end_date_row);

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
    form_box.append(&common_group);

    let desc_label_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let desc_label = gtk4::Label::builder()
        .label("Description")
        .xalign(0.0)
        .hexpand(true)
        .css_classes(["heading"])
        .build();
    let add_bullet_button = gtk4::Button::from_icon_name("list-add-symbolic");
    add_bullet_button.set_tooltip_text(Some("Add Bullet"));
    desc_label_row.append(&desc_label);
    desc_label_row.append(&add_bullet_button);
    form_box.append(&desc_label_row);
    let description_list = gtk4::ListBox::new();
    description_list.set_selection_mode(gtk4::SelectionMode::None);
    description_list.add_css_class("boxed-list");
    form_box.append(&description_list);
    let description_rows: Rc<RefCell<Vec<DescriptionRow>>> = Rc::new(RefCell::new(Vec::new()));

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

    // ── Raw YAML view, syntax-highlighted via GtkSourceView ────────────
    let raw_buffer = sourceview5::Buffer::new(None);
    if let Some(lang) = sourceview5::LanguageManager::default().language("yaml") {
        raw_buffer.set_language(Some(&lang));
    }
    apply_source_style_scheme(&raw_buffer);
    {
        let raw_buffer = raw_buffer.clone();
        adw::StyleManager::default().connect_dark_notify(move |_| {
            apply_source_style_scheme(&raw_buffer);
        });
    }
    let raw_view = sourceview5::View::builder()
        .buffer(&raw_buffer)
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::WordChar)
        .show_line_numbers(true)
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
    let on_dirty: DirtyHook = Rc::new(RefCell::new(None));
    {
        let extra_list_clone = extra_list.clone();
        let rows_clone = extra_rows.clone();
        let on_dirty = on_dirty.clone();
        add_field_button.connect_clicked(move |_| {
            add_extra_row(&extra_list_clone, &rows_clone, &on_dirty, "", "");
        });
    }

    // Every field that contributes to `read_form` reports edits, so autosave
    // sees them. The raw-YAML view is deliberately excluded — it only ever
    // commits via an explicit Save.
    for row in [
        &key_row,
        &category_row,
        &title_row,
        &org_row,
        &location_row,
        &start_date_row,
        &end_date_row,
        &tags_row,
    ] {
        let on_dirty = on_dirty.clone();
        row.connect_changed(move |_| DetailWidgets::notify_dirty(&on_dirty));
    }
    {
        let on_dirty = on_dirty.clone();
        date_mode_dropdown.connect_selected_notify(move |_| DetailWidgets::notify_dirty(&on_dirty));
    }
    {
        let description_list = description_list.clone();
        let description_rows = description_rows.clone();
        let on_dirty = on_dirty.clone();
        add_bullet_button.connect_clicked(move |_| {
            let entry = add_description_row(&description_list, &description_rows, &on_dirty, "");
            entry.grab_focus();
        });
    }

    DetailWidgets {
        raw_toggle,
        warnings_label,
        key_row,
        category_row,
        category_warning,
        title_row,
        org_row,
        org_warning,
        location_row,
        location_warning,
        date_mode_dropdown,
        start_date_row,
        date_warning,
        end_date_row,
        tags_row,
        tags_warning,
        tags_suggest_popover,
        tags_suggest_box,
        description_list,
        description_rows,
        extra_list,
        extra_rows,
        raw_view,
        save_button,
        outer_stack,
        empty_add_button,
        key_autogen_active: Rc::new(Cell::new(false)),
        suppress_key_change: Rc::new(Cell::new(false)),
        on_dirty,
    }
}

pub fn show_empty(widgets: &DetailWidgets) {
    widgets.outer_stack.set_visible_child_name("empty");
}

fn text_of(view: &impl IsA<gtk4::TextView>) -> String {
    let buf = view.as_ref().buffer();
    buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string()
}

fn set_text_of(view: &impl IsA<gtk4::TextView>, text: &str) {
    view.as_ref().buffer().set_text(text);
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
    clear_description_rows(widgets);
    for bullet in &entry.description {
        add_description_row(
            &widgets.description_list,
            &widgets.description_rows,
            &widgets.on_dirty,
            bullet,
        );
    }

    clear_extra_rows(widgets);
    for (k, v) in &entry.extra {
        let value_str = match v {
            serde_yaml_ng::Value::String(s) => s.clone(),
            other => serde_yaml_ng::to_string(other)
                .unwrap_or_default()
                .trim()
                .to_string(),
        };
        add_extra_row(
            &widgets.extra_list,
            &widgets.extra_rows,
            &widgets.on_dirty,
            k,
            &value_str,
        );
    }

    suggest_category_fields(widgets, &entry.category);

    let raw = skrizhal_core::to_yaml_string(std::slice::from_ref(entry), &Default::default(), &[])
        .unwrap_or_default();
    set_text_of(&widgets.raw_view, &raw);

    update_warnings(widgets, entry);
}

/// Clears a field's inline warning icon (used before re-applying below).
fn clear_field_warning(icon: &gtk4::Image) {
    icon.set_visible(false);
    icon.set_tooltip_text(None);
}

fn set_field_warning(icon: &gtk4::Image, message: &str) {
    icon.set_tooltip_text(Some(message));
    icon.set_visible(true);
}

pub fn update_warnings(widgets: &DetailWidgets, entry: &CvEntry) {
    for icon in [
        &widgets.category_warning,
        &widgets.org_warning,
        &widgets.location_warning,
        &widgets.date_warning,
        &widgets.tags_warning,
    ] {
        clear_field_warning(icon);
    }

    let warnings = validate_entries(std::slice::from_ref(entry));
    if warnings.is_empty() {
        widgets.warnings_label.set_label("");
        return;
    }
    let text: Vec<String> = warnings
        .iter()
        .map(|w| match w {
            Warning::UnknownCategory { category, .. } => {
                let msg = format!("Unrecognized category \"{category}\"");
                set_field_warning(&widgets.category_warning, &msg);
                msg
            }
            Warning::MissingRecommendedField { field, .. } => {
                let msg = format!("Missing recommended field \"{field}\"");
                match *field {
                    "organization" => set_field_warning(&widgets.org_warning, &msg),
                    "location" => set_field_warning(&widgets.location_warning, &msg),
                    "date" => set_field_warning(&widgets.date_warning, &msg),
                    "tags" => set_field_warning(&widgets.tags_warning, &msg),
                    _ => {}
                }
                msg
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
    let description: Vec<String> = widgets
        .description_rows
        .borrow()
        .iter()
        .map(|(_, e)| e.text().trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let mut extra = std::collections::BTreeMap::new();
    for (_, key_entry, value_entry) in widgets.extra_rows.borrow().iter() {
        let key = key_entry.text().to_string();
        let value = value_entry.text().to_string();
        // An empty value means the row is an unfilled prompt — either one the
        // user added and hasn't typed into, or one `suggest_category_fields`
        // conjured from the category registry. Writing those out would put an
        // empty `degree:` on every Education entry.
        if key.trim().is_empty() || value.trim().is_empty() {
            continue;
        }
        extra.insert(key.trim().to_string(), serde_yaml_ng::Value::String(value));
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
        // Not exposed in the form yet — preserved from the loaded entry by
        // the caller rather than being silently dropped on every save.
        order: None,
        tags,
        description,
        extra,
    }
}

/// Parses the raw-YAML pane's text, expecting exactly one entry.
pub fn read_raw(widgets: &DetailWidgets) -> Result<CvEntry, String> {
    let text = text_of(&widgets.raw_view);
    let outcome = skrizhal_core::parse_str(&text).map_err(|e| e.to_string())?;
    if let Some((key, err)) = outcome.failed.first() {
        return Err(format!("{key}: {err}"));
    }
    let mut entries = outcome.entries;
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
