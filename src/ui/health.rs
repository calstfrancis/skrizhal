use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use skrizhal_core::Finding;

use super::state::SharedState;

/// Groups findings by kind so a file with forty untagged entries doesn't
/// bury its one real typo.
fn group_title(finding: &Finding) -> &'static str {
    match finding {
        Finding::NearDuplicate { .. } => "Possible Duplicates",
        Finding::SuspiciousTag { .. } => "Possible Tag Typos",
        Finding::Untagged { .. } => "Untagged Entries",
        Finding::UnknownCategory { .. } => "Unrecognized Categories",
    }
}

/// Order groups by how likely each is to be a real problem rather than a
/// deliberate choice — untagged entries are often perfectly intentional,
/// a misspelled tag almost never is.
fn group_rank(finding: &Finding) -> u8 {
    match finding {
        Finding::SuspiciousTag { .. } => 0,
        Finding::NearDuplicate { .. } => 1,
        Finding::UnknownCategory { .. } => 2,
        Finding::Untagged { .. } => 3,
    }
}

pub fn show(
    parent: &adw::ApplicationWindow,
    state: &SharedState,
    select_entry: Rc<dyn Fn(String)>,
) {
    let window = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Database Health")
        .default_width(620)
        .default_height(680)
        .build();

    let header = adw::HeaderBar::new();
    let scrolled = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    body.set_margin_top(12);
    body.set_margin_bottom(12);
    body.set_margin_start(12);
    body.set_margin_end(12);
    scrolled.set_child(Some(&body));

    let mut findings = skrizhal_core::analyze_health(&state.borrow().entries);
    findings.sort_by_key(group_rank);

    if findings.is_empty() {
        let status = adw::StatusPage::builder()
            .icon_name("emblem-ok-symbolic")
            .title("Nothing to Flag")
            .description(
                "No duplicate-looking entries, no suspicious tags, and every \
                 entry is tagged and uses a known category.",
            )
            .build();
        body.append(&status);
    } else {
        let mut current_group: Option<&'static str> = None;
        let mut group_widget: Option<adw::PreferencesGroup> = None;
        for finding in &findings {
            let title = group_title(finding);
            if current_group != Some(title) {
                let group = adw::PreferencesGroup::builder().title(title).build();
                body.append(&group);
                group_widget = Some(group);
                current_group = Some(title);
            }
            let row = adw::ActionRow::builder()
                .title(finding.message())
                .subtitle_lines(0)
                .title_lines(0)
                .activatable(true)
                .build();
            // Every finding names at least one entry; jumping straight to the
            // first one is the only action that makes sense here.
            if let Some(key) = finding.keys().first().map(|k| k.to_string()) {
                let select_entry = select_entry.clone();
                let window = window.clone();
                row.connect_activated(move |_| {
                    select_entry(key.clone());
                    window.close();
                });
                row.add_suffix(
                    &gtk4::Image::builder()
                        .icon_name("go-next-symbolic")
                        .css_classes(["dim-label"])
                        .build(),
                );
            }
            if let Some(group) = &group_widget {
                group.add(&row);
            }
        }
    }

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scrolled));
    window.set_content(Some(&toolbar));
    window.present();
}
