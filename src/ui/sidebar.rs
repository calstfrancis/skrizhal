use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use skrizhal_core::{filter_entries, sort_entries, FilterOptions, SortMode};

use super::state::{ChangeCallback, SharedState};

const ALL_CATEGORIES: &str = "All Categories";
const ALL_TAGS: &str = "All Tags";

#[derive(Clone)]
pub struct SidebarWidgets {
    pub root: gtk4::Box,
    pub search_entry: gtk4::SearchEntry,
    pub category_filter: gtk4::DropDown,
    pub sort_dropdown: gtk4::DropDown,
    pub tag_filter_button: gtk4::MenuButton,
    tag_filter_popover_box: gtk4::Box,
    selected_tags: Rc<RefCell<BTreeSet<String>>>,
    pub list_box: gtk4::ListBox,
    pub add_button: gtk4::Button,
    pub window: adw::ApplicationWindow,
}

pub fn build(window: &adw::ApplicationWindow) -> SidebarWidgets {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    root.add_css_class("fond-sidebar");
    root.set_margin_top(6);
    root.set_margin_bottom(6);
    root.set_margin_start(6);
    root.set_margin_end(6);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Search title, organization, description…")
        .build();
    search_entry.add_css_class("fond-search");
    root.append(&search_entry);

    let filter_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let mut category_strings = vec![ALL_CATEGORIES.to_string()];
    category_strings.extend(skrizhal_core::CATEGORY_REGISTRY.iter().map(|c| c.name.to_string()));
    let category_refs: Vec<&str> = category_strings.iter().map(|s| s.as_str()).collect();
    let category_filter = gtk4::DropDown::from_strings(&category_refs);
    category_filter.set_hexpand(true);

    let tag_filter_button = gtk4::MenuButton::builder()
        .label(ALL_TAGS)
        .hexpand(true)
        .build();
    let tag_filter_popover = gtk4::Popover::new();
    let tag_filter_popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let tag_filter_scroll = gtk4::ScrolledWindow::builder()
        .max_content_height(280)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .child(&tag_filter_popover_box)
        .build();
    tag_filter_popover.set_child(Some(&tag_filter_scroll));
    tag_filter_button.set_popover(Some(&tag_filter_popover));

    filter_row.append(&category_filter);
    filter_row.append(&tag_filter_button);
    root.append(&filter_row);

    let sort_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let sort_label = gtk4::Label::builder()
        .label("Sort")
        .css_classes(["dim-label", "caption"])
        .build();
    let sort_labels: Vec<&str> = SortMode::ALL.iter().map(|m| m.label()).collect();
    let sort_dropdown = gtk4::DropDown::from_strings(&sort_labels);
    sort_dropdown.set_hexpand(true);
    sort_row.append(&sort_label);
    sort_row.append(&sort_dropdown);
    root.append(&sort_row);

    let add_button = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Add Entry")
        .css_classes(["suggested-action"])
        .build();

    let scrolled = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();
    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::Single);
    list_box.add_css_class("fond-list");
    scrolled.set_child(Some(&list_box));
    root.append(&scrolled);

    SidebarWidgets {
        root,
        search_entry,
        category_filter,
        sort_dropdown,
        tag_filter_button,
        tag_filter_popover_box,
        selected_tags: Rc::new(RefCell::new(BTreeSet::new())),
        list_box,
        add_button,
        window: window.clone(),
    }
}

fn update_tag_filter_button_label(widgets: &SidebarWidgets) {
    let selected = widgets.selected_tags.borrow();
    let label = match selected.len() {
        0 => ALL_TAGS.to_string(),
        1 => selected.iter().next().cloned().unwrap_or_default(),
        n => format!("{n} Tags"),
    };
    widgets.tag_filter_button.set_label(&label);
}

/// Rebuilds the tag filter popover's checkbox list from the current entry
/// set, dropping any previously selected tags that no longer exist. `on_toggle`
/// fires whenever a checkbox is flipped, so the caller can re-run filtering.
pub fn refresh_tag_filter_options(
    widgets: &SidebarWidgets,
    state: &SharedState,
    on_toggle: Rc<dyn Fn()>,
) {
    let tags = skrizhal_core::all_tags_with_counts(&state.borrow().entries);
    let tag_names: BTreeSet<String> = tags.iter().map(|(t, _)| t.clone()).collect();
    widgets.selected_tags.borrow_mut().retain(|t| tag_names.contains(t));

    while let Some(child) = widgets.tag_filter_popover_box.first_child() {
        widgets.tag_filter_popover_box.remove(&child);
    }
    for (tag, count) in tags {
        let check = gtk4::CheckButton::builder()
            .label(format!("{tag} ({count})"))
            .active(widgets.selected_tags.borrow().contains(&tag))
            .build();
        {
            let selected_tags = widgets.selected_tags.clone();
            let widgets = widgets.clone();
            let tag = tag.clone();
            let on_toggle = on_toggle.clone();
            check.connect_toggled(move |btn| {
                if btn.is_active() {
                    selected_tags.borrow_mut().insert(tag.clone());
                } else {
                    selected_tags.borrow_mut().remove(&tag);
                }
                update_tag_filter_button_label(&widgets);
                on_toggle();
            });
        }
        widgets.tag_filter_popover_box.append(&check);
    }
    update_tag_filter_button_label(widgets);
}

fn selected_dropdown_text(dd: &gtk4::DropDown) -> Option<String> {
    dd.selected_item()
        .and_downcast::<gtk4::StringObject>()
        .map(|s| s.string().to_string())
}

pub fn current_filter_category(widgets: &SidebarWidgets) -> Option<String> {
    selected_dropdown_text(&widgets.category_filter).filter(|s| s != ALL_CATEGORIES)
}

/// Clears every selected tag and resets the filter button's label — used
/// when jumping to a specific entry that the active filters might hide.
pub fn clear_tag_filter_selection(widgets: &SidebarWidgets) {
    widgets.selected_tags.borrow_mut().clear();
    let mut child = widgets.tag_filter_popover_box.first_child();
    while let Some(widget) = child {
        if let Some(check) = widget.downcast_ref::<gtk4::CheckButton>() {
            check.set_active(false);
        }
        child = widget.next_sibling();
    }
    update_tag_filter_button_label(widgets);
}

pub fn current_filter_tags(widgets: &SidebarWidgets) -> Vec<String> {
    widgets.selected_tags.borrow().iter().cloned().collect()
}

fn row_title(entry: &skrizhal_core::CvEntry) -> String {
    if entry.title.is_empty() {
        entry.key.clone()
    } else {
        entry.title.clone()
    }
}

fn row_tooltip(entry: &skrizhal_core::CvEntry) -> String {
    match entry.organization.as_deref() {
        Some(org) if !org.is_empty() => format!("{}\n{org}", row_title(entry)),
        _ => row_title(entry),
    }
}

fn row_meta(entry: &skrizhal_core::CvEntry) -> String {
    entry.date.clone().unwrap_or_default()
}

/// Updates one row's text (and key, if it changed) without rebuilding the
/// list — the autosave path, which must not disturb the row the user is
/// currently typing into.
///
/// Deliberately does *not* re-filter or re-sort: an edit that pushes the
/// entry out of the active filter, or past its neighbours in the sort order,
/// leaves the row where it is until the next full refresh. Having a row
/// vanish or jump mid-keystroke is worse than a momentarily stale position.
pub fn update_row_in_place(
    widgets: &SidebarWidgets,
    old_key: &str,
    entry: &skrizhal_core::CvEntry,
) {
    let mut child = widgets.list_box.first_child();
    while let Some(widget) = child {
        if widget.widget_name() == old_key {
            // The row's three labels are found by widget name rather than by
            // position, so reordering the row box can't silently retarget
            // this to the wrong label.
            if let Some(row) = widget.downcast_ref::<gtk4::ListBoxRow>() {
                if let Some(row_box) = row.child() {
                    let mut label = row_box.first_child();
                    while let Some(w) = label {
                        if let Some(l) = w.downcast_ref::<gtk4::Label>() {
                            match l.widget_name().as_str() {
                                "row-title" => l.set_text(&row_title(entry)),
                                "row-meta" => l.set_text(&row_meta(entry)),
                                _ => {}
                            }
                        }
                        label = w.next_sibling();
                    }
                }
                row.set_tooltip_text(Some(&row_tooltip(entry)));
                row.set_widget_name(&entry.key);
            }
            return;
        }
        child = widget.next_sibling();
    }
}

/// Clears and repopulates the entry list from `state`, applying the current
/// search/category/tag filters. Rows carry their entry's key as the widget
/// name so selection handlers can look the entry back up.
pub fn refresh_list(widgets: &SidebarWidgets, state: &SharedState, on_change: &ChangeCallback) {
    while let Some(child) = widgets.list_box.first_child() {
        widgets.list_box.remove(&child);
    }

    let sort_mode = state.borrow().sort_mode;
    let mut matches: Vec<skrizhal_core::CvEntry> = {
        let s = state.borrow();
        let opts = FilterOptions {
            category: s.filter_category.as_deref(),
            tags: s.filter_tags.iter().map(|t| t.as_str()).collect(),
            query: if s.search.trim().is_empty() {
                None
            } else {
                Some(s.search.as_str())
            },
        };
        filter_entries(&s.entries, &opts).into_iter().cloned().collect()
    };
    sort_entries(&mut matches, sort_mode);

    for entry in &matches {
        // One line per entry: the title, the organization as dim reference
        // text, and the date at the right edge. An AdwActionRow's title +
        // subtitle is two lines, which doubles the height of the list and
        // buries the titles — the thing the list is actually scanned for.
        let row = gtk4::ListBoxRow::new();
        row.set_activatable(true);
        row.set_widget_name(&entry.key);
        row.add_css_class("fond-card");
        row.add_css_class("fond-row");

        let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        row_box.set_margin_start(10);
        row_box.set_margin_end(10);

        let title_label = gtk4::Label::new(Some(&row_title(entry)));
        title_label.set_widget_name("row-title");
        title_label.add_css_class("fond-row-title");
        title_label.set_halign(gtk4::Align::Start);
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        // The title takes the slack. Without this the two labels beside it
        // claim their natural width first and the title — the thing the list
        // is scanned for — ellipsizes down to a few characters.
        title_label.set_hexpand(true);
        row_box.append(&title_label);

        // The organization is the row's tooltip rather than a third column.
        // Zerkalo can afford dim reference text beside the title because its
        // library window is wide; this sidebar is ~300px, and fitting title +
        // organization + date on one line left the title — the thing actually
        // being scanned — ellipsized to about six characters.
        row.set_tooltip_text(Some(&row_tooltip(entry)));

        let meta_label = gtk4::Label::new(Some(&row_meta(entry)));
        meta_label.set_widget_name("row-meta");
        meta_label.add_css_class("fond-row-meta");
        row_box.append(&meta_label);

        let menu_button = gtk4::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .tooltip_text("More")
            .build();
        let popover = gtk4::Popover::new();
        let popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let duplicate_btn = gtk4::Button::builder()
            .label("Duplicate")
            .css_classes(["flat"])
            .build();
        popover_box.append(&duplicate_btn);
        popover.set_child(Some(&popover_box));
        menu_button.set_popover(Some(&popover));

        // Delete sits directly on the row (not behind the kebab menu) since
        // it's the common destructive action users reach for most.
        let delete_btn = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .valign(gtk4::Align::Center)
            .css_classes(["flat", "destructive-action"])
            .tooltip_text("Delete")
            .build();

        // Key is read off the row at click time rather than captured: autosave
        // can rename an entry (auto-generated keys follow Title/Organization)
        // and rewrite the row's widget name in place, which would leave a
        // captured key pointing at an entry that no longer exists.
        {
            let state = state.clone();
            let popover = popover.clone();
            let on_change = on_change.clone();
            // Weak: the row owns this button, which owns this closure — a
            // strong `row` here would be a reference cycle that outlives
            // every list rebuild.
            duplicate_btn.connect_clicked(glib::clone!(@weak row => move |_| {
                popover.popdown();
                let key = row.widget_name().to_string();
                super::state::push_undo(&state);
                let new_key = {
                    let mut s = state.borrow_mut();
                    let Some(idx) = s.entries.iter().position(|e| e.key == key) else {
                        return;
                    };
                    let new_key = skrizhal_core::unique_key(&format!("{key}-copy"), &s.entries);
                    let dup = s.entries[idx].duplicate_with_key(new_key.clone());
                    s.entries.push(dup);
                    new_key
                };
                on_change(Some(new_key));
            }));
        }
        {
            let state = state.clone();
            let on_change = on_change.clone();
            let window = widgets.window.clone();
            delete_btn.connect_clicked(glib::clone!(@weak row => move |_| {
                let key = row.widget_name().to_string();
                // Looked up rather than read off the widget, for the same
                // reason the key is: autosave can rename an entry underneath
                // this row between build and click.
                let label = {
                    let s = state.borrow();
                    s.entries
                        .iter()
                        .find(|e| e.key == key)
                        .map(row_title)
                        .unwrap_or_else(|| key.clone())
                };
                let dialog = adw::MessageDialog::new(
                    Some(&window),
                    Some(&format!("Delete \"{label}\"?")),
                    Some("You can undo this with Ctrl+Z."),
                );
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("delete", "Delete");
                dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");
                let state = state.clone();
                let key = key.clone();
                let on_change = on_change.clone();
                dialog.connect_response(None, move |dialog, response| {
                    if response == "delete" {
                        super::state::push_undo(&state);
                        {
                            let mut s = state.borrow_mut();
                            s.entries.retain(|e| e.key != key);
                            if s.selected_key.as_deref() == Some(key.as_str()) {
                                s.selected_key = None;
                            }
                        }
                        on_change(None);
                    }
                    dialog.close();
                });
                dialog.present();
            }));
        }

        // Destructive and secondary actions appear on hover rather than
        // sitting permanently in every row.
        delete_btn.add_css_class("fond-onhover");
        menu_button.add_css_class("fond-onhover");
        row_box.append(&delete_btn);
        row_box.append(&menu_button);
        row.set_child(Some(&row_box));
        widgets.list_box.append(&row);
    }

    // The group reads as one object: only its ends are rounded.
    if let Some(first) = widgets.list_box.first_child() {
        first.add_css_class("fond-card-first");
    }
    if let Some(last) = widgets.list_box.last_child() {
        last.add_css_class("fond-card-last");
    }
}
