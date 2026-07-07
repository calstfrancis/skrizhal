use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::config::Config;

const FIELDS: &[(&str, &str)] = &[
    (
        "Key",
        "A short, unique identifier for this entry, like a citation key. It \
         auto-fills from Organization + Title as you type — edit it directly \
         any time if you'd rather choose your own.",
    ),
    (
        "Category",
        "What kind of entry this is — Education, Employment, Award, \
         Publication, and so on. Start typing to see suggestions, or make up \
         your own category if nothing fits.",
    ),
    ("Title", "The position, degree, award, or publication name."),
    (
        "Organization",
        "The employer, institution, or body this entry is associated with.",
    ),
    ("Location", "City or region, if it matters for this entry."),
    (
        "Date",
        "When this happened. Choose Single Date, Date Range, or Ongoing (for \
         something still in progress) from the dropdown.",
    ),
    (
        "Tags",
        "Probably the most useful field here. Tag entries with things like \
         \"ministry\" or \"academic\", then filter the sidebar by tag to see \
         just the entries relevant to one CV — the same database can produce \
         a ministry-focused CV, an academic CV, or anything else, just by \
         filtering on different tags.",
    ),
    ("Description", "Bullet points describing this entry, one per line."),
    (
        "Additional Fields",
        "Anything specific to this category that doesn't have its own field \
         — degree name, DOI, award amount, and so on.",
    ),
    (
        "Raw YAML",
        "A toggle in the top corner of the entry pane — switches to editing \
         this entry's data directly as YAML text, for anything the form \
         doesn't cover.",
    ),
];

fn field_row(name: &str, description: &str) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    row.set_margin_top(10);
    row.set_margin_bottom(10);
    row.set_margin_start(12);
    row.set_margin_end(12);

    let name_label = gtk4::Label::builder()
        .label(name)
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let desc_label = gtk4::Label::builder()
        .label(description)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    row.append(&name_label);
    row.append(&desc_label);
    row
}

/// Shows the field guide. When `mark_seen` is true (first-run only), records
/// that it's been shown so it won't pop up unprompted again — it stays
/// reachable afterward via the header menu.
pub fn show(window: &adw::ApplicationWindow, mark_seen: bool) {
    let dialog = adw::Window::builder()
        .transient_for(window)
        .modal(true)
        .title("Field Guide")
        .default_width(480)
        .default_height(560)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&adw::HeaderBar::new());

    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let intro = gtk4::Label::builder()
        .label("What each field on a CV entry is for:")
        .xalign(0.0)
        .wrap(true)
        .margin_start(12)
        .margin_end(12)
        .margin_top(12)
        .css_classes(["dim-label"])
        .build();
    content_box.append(&intro);

    let scrolled = gtk4::ScrolledWindow::builder().vexpand(true).build();
    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::None);
    list_box.add_css_class("boxed-list");
    list_box.set_margin_top(12);
    list_box.set_margin_bottom(12);
    list_box.set_margin_start(12);
    list_box.set_margin_end(12);
    for (name, desc) in FIELDS {
        list_box.append(&field_row(name, desc));
    }
    scrolled.set_child(Some(&list_box));
    content_box.append(&scrolled);

    let got_it = gtk4::Button::builder()
        .label("Got It")
        .css_classes(["suggested-action"])
        .halign(gtk4::Align::End)
        .margin_top(6)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    content_box.append(&got_it);

    toolbar_view.set_content(Some(&content_box));
    dialog.set_content(Some(&toolbar_view));

    {
        let dialog = dialog.clone();
        got_it.connect_clicked(move |_| dialog.close());
    }

    if mark_seen {
        let mut cfg = Config::load();
        cfg.has_seen_field_guide = true;
        let _ = cfg.save();
    }

    dialog.present();
}
