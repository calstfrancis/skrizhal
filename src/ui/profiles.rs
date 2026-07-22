use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use skrizhal_core::{resolve_section, Profile, ProfileSection};

use super::state::{self, SharedState};

type SharedProfiles = Rc<RefCell<Vec<Profile>>>;
/// Self-reference slot: section buttons need to trigger the rebuild that
/// creates them, so it can only be filled in after it exists.
type RebuildCell = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

fn split_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn list_row(title: &str, subtitle: &str, value: &[String]) -> adw::EntryRow {
    let row = adw::EntryRow::builder().title(title).build();
    row.set_text(&value.join(", "));
    if let Some(text) = row.delegate().and_then(|d| d.downcast::<gtk4::Text>().ok()) {
        text.set_placeholder_text(Some(subtitle));
    }
    row
}

/// Profiles are pure configuration — they never appear in the sidebar — so
/// they persist straight to disk instead of going through `on_change`, which
/// would rebuild the entry list and reload the detail form for no reason
/// (and could stomp on a half-typed entry behind the dialog).
fn commit(state: &SharedState, profiles: &SharedProfiles, toasts: &adw::ToastOverlay) {
    state.borrow_mut().profiles = profiles.borrow().clone();
    if let Err(err) = state::persist(state) {
        toasts.add_toast(adw::Toast::new(&err));
    }
}

/// Recomputes an expander's title and match count from the section's current
/// rules. Shared by the initial build and the live-edit path so the two can't
/// drift apart.
fn refresh_section_header(
    expander: &adw::ExpanderRow,
    state: &SharedState,
    section: &ProfileSection,
    index: usize,
) {
    let heading = if section.heading.trim().is_empty() {
        format!("Section {}", index + 1)
    } else {
        section.heading.clone()
    };
    let matched = resolve_section(&state.borrow().entries, section).len();
    expander.set_title(&heading);
    expander.set_subtitle(&format!(
        "{matched} {}",
        if matched == 1 { "entry" } else { "entries" }
    ));
}

pub fn show(parent: &adw::ApplicationWindow, state: &SharedState) {
    let profiles: SharedProfiles = Rc::new(RefCell::new(state.borrow().profiles.clone()));
    let current: Rc<Cell<usize>> = Rc::new(Cell::new(0));

    let window = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("CV Profiles")
        .default_width(620)
        .default_height(720)
        .build();

    let toasts = adw::ToastOverlay::new();
    let header = adw::HeaderBar::new();
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let selector_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    selector_row.set_margin_top(12);
    selector_row.set_margin_bottom(6);
    selector_row.set_margin_start(12);
    selector_row.set_margin_end(12);
    let profile_dropdown = gtk4::DropDown::from_strings(&[]);
    profile_dropdown.set_hexpand(true);
    let new_btn = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("New Profile")
        .css_classes(["suggested-action"])
        .build();
    let duplicate_btn = gtk4::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text("Duplicate Profile")
        .build();
    let delete_btn = gtk4::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Delete Profile")
        .css_classes(["destructive-action"])
        .build();
    selector_row.append(&profile_dropdown);
    selector_row.append(&new_btn);
    selector_row.append(&duplicate_btn);
    selector_row.append(&delete_btn);
    content.append(&selector_row);

    let scrolled = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    body.set_margin_top(6);
    body.set_margin_bottom(12);
    body.set_margin_start(12);
    body.set_margin_end(12);
    scrolled.set_child(Some(&body));
    content.append(&scrolled);

    toasts.set_child(Some(&content));
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&toasts));
    window.set_content(Some(&toolbar));

    // Rebuilt wholesale whenever the structure changes (profile switched,
    // section added/removed/reordered). Text edits inside a section commit
    // without a rebuild, so typing is never interrupted.
    let rebuild: RebuildCell = Rc::new(RefCell::new(None));

    let rebuild_body: Rc<dyn Fn()> = Rc::new({
        let body = body.clone();
        let profiles = profiles.clone();
        let current = current.clone();
        let state = state.clone();
        let toasts = toasts.clone();
        let rebuild = rebuild.clone();
        let profile_dropdown = profile_dropdown.clone();
        move || {
            while let Some(child) = body.first_child() {
                body.remove(&child);
            }

            let names: Vec<String> = profiles
                .borrow()
                .iter()
                .map(|p| p.display_name().to_string())
                .collect();
            let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            profile_dropdown.set_model(Some(&gtk4::StringList::new(&name_refs)));

            if profiles.borrow().is_empty() {
                let status = adw::StatusPage::builder()
                    .icon_name("view-list-symbolic")
                    .title("No Profiles Yet")
                    .description(
                        "A profile is a named, ordered set of CV sections — \
                         which entries appear, under which heading, in what order.",
                    )
                    .build();
                body.append(&status);
                return;
            }

            let index = current.get().min(profiles.borrow().len() - 1);
            current.set(index);
            profile_dropdown.set_selected(index as u32);

            let details = adw::PreferencesGroup::builder().title("Profile").build();
            let name_row = adw::EntryRow::builder().title("Name (used in Typst)").build();
            name_row.set_text(&profiles.borrow()[index].name);
            let label_row = adw::EntryRow::builder().title("Label").build();
            label_row.set_text(&profiles.borrow()[index].label);
            details.add(&name_row);
            details.add(&label_row);
            body.append(&details);

            {
                let profiles = profiles.clone();
                let state = state.clone();
                let toasts = toasts.clone();
                name_row.connect_changed(move |row| {
                    profiles.borrow_mut()[index].name = row.text().trim().to_string();
                    commit(&state, &profiles, &toasts);
                });
            }
            {
                let profiles = profiles.clone();
                let state = state.clone();
                let toasts = toasts.clone();
                label_row.connect_changed(move |row| {
                    profiles.borrow_mut()[index].label = row.text().to_string();
                    commit(&state, &profiles, &toasts);
                });
            }

            let sections_group = adw::PreferencesGroup::builder().title("Sections").build();
            let section_count = profiles.borrow()[index].sections.len();
            for si in 0..section_count {
                let section = profiles.borrow()[index].sections[si].clone();
                // Resolving live against the real entries turns the rules from
                // something you reason about into something you can see.
                let expander = adw::ExpanderRow::new();
                refresh_section_header(&expander, &state, &section, si);

                let heading_row = adw::EntryRow::builder().title("Heading").build();
                heading_row.set_text(&section.heading);
                let categories_row =
                    list_row("Categories", "any category", &section.categories);
                let tags_row = list_row("Tags", "any tag", &section.tags);
                let include_row =
                    list_row("Always include (keys)", "entry-key, …", &section.include);
                let exclude_row =
                    list_row("Always exclude (keys)", "entry-key, …", &section.exclude);
                for row in [
                    &heading_row,
                    &categories_row,
                    &tags_row,
                    &include_row,
                    &exclude_row,
                ] {
                    expander.add_row(row);
                }

                let controls = adw::ActionRow::new();
                let up_btn = gtk4::Button::builder()
                    .icon_name("go-up-symbolic")
                    .tooltip_text("Move Up")
                    .valign(gtk4::Align::Center)
                    .css_classes(["flat"])
                    .sensitive(si > 0)
                    .build();
                let down_btn = gtk4::Button::builder()
                    .icon_name("go-down-symbolic")
                    .tooltip_text("Move Down")
                    .valign(gtk4::Align::Center)
                    .css_classes(["flat"])
                    .sensitive(si + 1 < section_count)
                    .build();
                let remove_btn = gtk4::Button::builder()
                    .icon_name("user-trash-symbolic")
                    .tooltip_text("Remove Section")
                    .valign(gtk4::Align::Center)
                    .css_classes(["flat", "destructive-action"])
                    .build();
                controls.add_suffix(&up_btn);
                controls.add_suffix(&down_btn);
                controls.add_suffix(&remove_btn);
                expander.add_row(&controls);
                sections_group.add(&expander);

                // Text edits commit in place — no rebuild, so the field keeps
                // focus and the caret stays put while typing.
                macro_rules! bind_text {
                    ($row:expr, $apply:expr) => {{
                        let profiles = profiles.clone();
                        let state = state.clone();
                        let toasts = toasts.clone();
                        let expander = expander.clone();
                        $row.connect_changed(move |row| {
                            let section = {
                                let mut p = profiles.borrow_mut();
                                let section = &mut p[index].sections[si];
                                let apply: &dyn Fn(&mut ProfileSection, String) = &$apply;
                                apply(section, row.text().to_string());
                                section.clone()
                            };
                            commit(&state, &profiles, &toasts);
                            // Refresh the header in place. A full rebuild would
                            // be simpler but would steal focus mid-keystroke,
                            // and a match count you can't watch react to the
                            // rule you're typing is worth very little.
                            refresh_section_header(&expander, &state, &section, si);
                        });
                    }};
                }
                bind_text!(heading_row, |s: &mut ProfileSection, v: String| s.heading =
                    v.trim().to_string());
                bind_text!(categories_row, |s: &mut ProfileSection, v: String| {
                    s.categories = split_list(&v)
                });
                bind_text!(tags_row, |s: &mut ProfileSection, v: String| s.tags =
                    split_list(&v));
                bind_text!(include_row, |s: &mut ProfileSection, v: String| s.include =
                    split_list(&v));
                bind_text!(exclude_row, |s: &mut ProfileSection, v: String| s.exclude =
                    split_list(&v));

                for (btn, delta) in [(&up_btn, -1i64), (&down_btn, 1i64)] {
                    let profiles = profiles.clone();
                    let state = state.clone();
                    let toasts = toasts.clone();
                    let rebuild = rebuild.clone();
                    btn.connect_clicked(move |_| {
                        let target = (si as i64 + delta) as usize;
                        profiles.borrow_mut()[index].sections.swap(si, target);
                        commit(&state, &profiles, &toasts);
                        if let Some(rb) = rebuild.borrow().clone() {
                            rb();
                        }
                    });
                }
                {
                    let profiles = profiles.clone();
                    let state = state.clone();
                    let toasts = toasts.clone();
                    let rebuild = rebuild.clone();
                    remove_btn.connect_clicked(move |_| {
                        profiles.borrow_mut()[index].sections.remove(si);
                        commit(&state, &profiles, &toasts);
                        if let Some(rb) = rebuild.borrow().clone() {
                            rb();
                        }
                    });
                }
            }
            body.append(&sections_group);

            let add_section = gtk4::Button::builder()
                .label("Add Section")
                .halign(gtk4::Align::Center)
                .css_classes(["pill"])
                .build();
            {
                let profiles = profiles.clone();
                let state = state.clone();
                let toasts = toasts.clone();
                let rebuild = rebuild.clone();
                add_section.connect_clicked(move |_| {
                    profiles.borrow_mut()[index]
                        .sections
                        .push(ProfileSection::default());
                    commit(&state, &profiles, &toasts);
                    if let Some(rb) = rebuild.borrow().clone() {
                        rb();
                    }
                });
            }
            body.append(&add_section);
        }
    });
    *rebuild.borrow_mut() = Some(rebuild_body.clone());

    {
        let profiles = profiles.clone();
        let current = current.clone();
        let state = state.clone();
        let toasts = toasts.clone();
        let rebuild_body = rebuild_body.clone();
        new_btn.connect_clicked(move |_| {
            let name = unique_profile_name("new-profile", &profiles.borrow());
            profiles.borrow_mut().push(Profile {
                name,
                label: String::new(),
                sections: vec![ProfileSection::default()],
            });
            current.set(profiles.borrow().len() - 1);
            commit(&state, &profiles, &toasts);
            rebuild_body();
        });
    }
    {
        let profiles = profiles.clone();
        let current = current.clone();
        let state = state.clone();
        let toasts = toasts.clone();
        let rebuild_body = rebuild_body.clone();
        duplicate_btn.connect_clicked(move |_| {
            let copy = {
                let p = profiles.borrow();
                let Some(source) = p.get(current.get()) else {
                    return;
                };
                let mut copy = source.clone();
                copy.name = unique_profile_name(&format!("{}-copy", source.name), &p);
                copy
            };
            profiles.borrow_mut().push(copy);
            current.set(profiles.borrow().len() - 1);
            commit(&state, &profiles, &toasts);
            rebuild_body();
        });
    }
    {
        let profiles = profiles.clone();
        let current = current.clone();
        let state = state.clone();
        let toasts = toasts.clone();
        let rebuild_body = rebuild_body.clone();
        let window = window.clone();
        delete_btn.connect_clicked(move |_| {
            let index = current.get();
            let name = match profiles.borrow().get(index) {
                Some(p) => p.display_name().to_string(),
                None => return,
            };
            let dialog = adw::MessageDialog::new(
                Some(&window),
                Some(&format!("Delete profile \"{name}\"?")),
                Some("The entries it references are not affected."),
            );
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("delete", "Delete");
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            let profiles = profiles.clone();
            let current = current.clone();
            let state = state.clone();
            let toasts = toasts.clone();
            let rebuild_body = rebuild_body.clone();
            dialog.connect_response(None, move |dialog, response| {
                if response == "delete" {
                    profiles.borrow_mut().remove(index);
                    current.set(index.saturating_sub(1));
                    commit(&state, &profiles, &toasts);
                    rebuild_body();
                }
                dialog.close();
            });
            dialog.present();
        });
    }
    {
        let current = current.clone();
        let rebuild_body = rebuild_body.clone();
        let profiles = profiles.clone();
        profile_dropdown.connect_selected_notify(move |dd| {
            let selected = dd.selected() as usize;
            // Guard against the notify that `set_model` itself emits during a
            // rebuild, which would otherwise recurse.
            if selected < profiles.borrow().len() && selected != current.get() {
                current.set(selected);
                rebuild_body();
            }
        });
    }

    rebuild_body();
    window.present();
}

fn unique_profile_name(base: &str, existing: &[Profile]) -> String {
    if !existing.iter().any(|p| p.name == base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !existing.iter().any(|p| p.name == candidate) {
            return candidate;
        }
        n += 1;
    }
}
