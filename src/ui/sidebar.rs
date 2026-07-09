use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use skrizhal_core::{filter_entries, FilterOptions};

use super::state::{ChangeCallback, SharedState};

const ALL_CATEGORIES: &str = "All Categories";
const ALL_TAGS: &str = "All Tags";

#[derive(Clone)]
pub struct SidebarWidgets {
    pub root: gtk4::Box,
    pub search_entry: gtk4::SearchEntry,
    pub category_filter: gtk4::DropDown,
    pub tag_filter_button: gtk4::MenuButton,
    tag_filter_popover_box: gtk4::Box,
    selected_tags: Rc<RefCell<BTreeSet<String>>>,
    pub list_box: gtk4::ListBox,
    pub add_button: gtk4::Button,
    pub window: adw::ApplicationWindow,
}

pub fn build(window: &adw::ApplicationWindow) -> SidebarWidgets {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    root.set_margin_top(6);
    root.set_margin_bottom(6);
    root.set_margin_start(6);
    root.set_margin_end(6);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Search title, organization, description…")
        .build();
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
    list_box.add_css_class("boxed-list");
    scrolled.set_child(Some(&list_box));
    root.append(&scrolled);

    SidebarWidgets {
        root,
        search_entry,
        category_filter,
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

pub fn current_filter_tags(widgets: &SidebarWidgets) -> Vec<String> {
    widgets.selected_tags.borrow().iter().cloned().collect()
}

/// Clears and repopulates the entry list from `state`, applying the current
/// search/category/tag filters. Rows carry their entry's key as the widget
/// name so selection handlers can look the entry back up.
pub fn refresh_list(widgets: &SidebarWidgets, state: &SharedState, on_change: &ChangeCallback) {
    while let Some(child) = widgets.list_box.first_child() {
        widgets.list_box.remove(&child);
    }

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
    matches.sort_by_key(|e| e.title.to_lowercase());

    for entry in &matches {
        let subtitle_parts: Vec<&str> = [entry.organization.as_deref(), entry.date.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        let row = adw::ActionRow::builder()
            .title(if entry.title.is_empty() {
                entry.key.clone()
            } else {
                entry.title.clone()
            })
            .subtitle(subtitle_parts.join(" · "))
            .activatable(true)
            .build();
        row.set_widget_name(&entry.key);

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

        {
            let state = state.clone();
            let key = entry.key.clone();
            let popover = popover.clone();
            let on_change = on_change.clone();
            duplicate_btn.connect_clicked(move |_| {
                popover.popdown();
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
            });
        }
        {
            let state = state.clone();
            let key = entry.key.clone();
            let title = entry.title.clone();
            let on_change = on_change.clone();
            let window = widgets.window.clone();
            delete_btn.connect_clicked(move |_| {
                let label = if title.is_empty() { key.clone() } else { title.clone() };
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
            });
        }

        row.add_suffix(&delete_btn);
        row.add_suffix(&menu_button);
        widgets.list_box.append(&row);
    }
}
