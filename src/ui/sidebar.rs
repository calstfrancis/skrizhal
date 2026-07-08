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
    pub tag_filter: gtk4::DropDown,
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

    let tag_filter = gtk4::DropDown::from_strings(&[ALL_TAGS]);
    tag_filter.set_hexpand(true);

    filter_row.append(&category_filter);
    filter_row.append(&tag_filter);
    root.append(&filter_row);

    let add_button = gtk4::Button::from_icon_name("list-add-symbolic");
    add_button.set_tooltip_text(Some("Add Entry"));

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
        tag_filter,
        list_box,
        add_button,
        window: window.clone(),
    }
}

/// Rebuilds the tag filter's options from the current entry set, keeping the
/// selection on "All Tags" if the previously selected tag no longer exists.
pub fn refresh_tag_filter_options(widgets: &SidebarWidgets, state: &SharedState) {
    let tags = skrizhal_core::all_tags_with_counts(&state.borrow().entries);
    let mut strings = vec![ALL_TAGS.to_string()];
    strings.extend(tags.into_iter().map(|(t, c)| format!("{t} ({c})")));
    let refs: Vec<&str> = strings.iter().map(|s| s.as_str()).collect();
    let model = gtk4::StringList::new(&refs);
    widgets.tag_filter.set_model(Some(&model));
    widgets.tag_filter.set_selected(0);
}

fn selected_dropdown_text(dd: &gtk4::DropDown) -> Option<String> {
    dd.selected_item()
        .and_downcast::<gtk4::StringObject>()
        .map(|s| s.string().to_string())
}

/// Extracts the bare tag name from a "tag (N)" dropdown entry.
fn strip_count_suffix(s: &str) -> String {
    match s.rfind(" (") {
        Some(idx) => s[..idx].to_string(),
        None => s.to_string(),
    }
}

pub fn current_filter_category(widgets: &SidebarWidgets) -> Option<String> {
    selected_dropdown_text(&widgets.category_filter).filter(|s| s != ALL_CATEGORIES)
}

pub fn current_filter_tag(widgets: &SidebarWidgets) -> Option<String> {
    selected_dropdown_text(&widgets.tag_filter)
        .filter(|s| s != ALL_TAGS)
        .map(|s| strip_count_suffix(&s))
}

/// Clears and repopulates the entry list from `state`, applying the current
/// search/category/tag filters. Rows carry their entry's key as the widget
/// name so selection handlers can look the entry back up.
pub fn refresh_list(widgets: &SidebarWidgets, state: &SharedState, on_change: &ChangeCallback) {
    while let Some(child) = widgets.list_box.first_child() {
        widgets.list_box.remove(&child);
    }

    let s = state.borrow();
    let opts = FilterOptions {
        category: s.filter_category.as_deref(),
        tag: s.filter_tag.as_deref(),
        query: if s.search.trim().is_empty() {
            None
        } else {
            Some(s.search.as_str())
        },
    };
    let mut matches = filter_entries(&s.entries, &opts);
    matches.sort_by_key(|e| e.title.to_lowercase());

    for entry in matches {
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
            .build();
        let popover = gtk4::Popover::new();
        let popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let duplicate_btn = gtk4::Button::builder()
            .label("Duplicate")
            .css_classes(["flat"])
            .build();
        let delete_btn = gtk4::Button::builder()
            .label("Delete")
            .css_classes(["flat", "destructive-action"])
            .build();
        popover_box.append(&duplicate_btn);
        popover_box.append(&delete_btn);
        popover.set_child(Some(&popover_box));
        menu_button.set_popover(Some(&popover));

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
            let popover = popover.clone();
            let on_change = on_change.clone();
            let window = widgets.window.clone();
            delete_btn.connect_clicked(move |_| {
                popover.popdown();
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

        row.add_suffix(&menu_button);
        widgets.list_box.append(&row);
    }
}
