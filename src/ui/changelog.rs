use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

const CHANGELOG_MD: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/CHANGELOG.md"));

/// Applies inline `**bold**`/`` `code` `` spans, threading open/close state
/// across calls — Markdown allows an inline code span to wrap across a
/// soft-wrapped source line (it's really one reflowed paragraph), so state
/// can't just reset at each newline or an odd backtick count on one line
/// leaves an unclosed `<tt>` that breaks Pango parsing for the rest of the
/// document.
fn inline_markup(s: &str, bold: &mut bool, code: &mut bool) -> String {
    let escaped = glib::markup_escape_text(s).to_string();
    let mut result = String::new();
    let mut chars = escaped.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next();
            result.push_str(if *bold { "</b>" } else { "<b>" });
            *bold = !*bold;
        } else if c == '`' {
            result.push_str(if *code { "</tt>" } else { "<tt>" });
            *code = !*code;
        } else {
            result.push(c);
        }
    }
    result
}

/// Minimal Markdown-to-Pango conversion covering exactly what CHANGELOG.md
/// actually uses: `##`/`###` headings, `**bold**`, `` `code` ``, `- ` lists,
/// blank lines, and `---` separators.
fn markdown_to_pango(md: &str) -> String {
    let mut out = String::new();
    let mut bold = false;
    let mut code = false;
    for line in md.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            out.push('\n');
            continue;
        }
        if trimmed.trim() == "---" {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            out.push_str(&format!(
                "<span weight=\"bold\" size=\"large\">{}</span>\n",
                inline_markup(rest, &mut bold, &mut code)
            ));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            out.push_str(&format!("<b>{}</b>\n", inline_markup(rest, &mut bold, &mut code)));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            out.push_str(&format!("  •  {}\n", inline_markup(rest, &mut bold, &mut code)));
            continue;
        }
        out.push_str(&inline_markup(trimmed, &mut bold, &mut code));
        out.push('\n');
    }
    out
}

/// Shows CHANGELOG.md (embedded at compile time, so it's always available
/// regardless of how/where the app was installed) in a small window.
pub fn show(window: &adw::ApplicationWindow) {
    let dialog = adw::Window::builder()
        .transient_for(window)
        .modal(true)
        .title("Changelog")
        .default_width(560)
        .default_height(640)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&adw::HeaderBar::new());

    let scrolled = gtk4::ScrolledWindow::builder().vexpand(true).build();
    let label = gtk4::Label::builder()
        .use_markup(true)
        .wrap(true)
        .xalign(0.0)
        .valign(gtk4::Align::Start)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build();
    let markup = markdown_to_pango(CHANGELOG_MD);
    // Validate before handing it to the label — GtkLabel::set_markup fails
    // silently (a logged critical, empty label) on malformed markup, which
    // is a bad way to find out a future CHANGELOG edit broke the converter.
    if gtk4::pango::parse_markup(&markup, '\0').is_ok() {
        label.set_markup(&markup);
    } else {
        label.set_text(CHANGELOG_MD);
    }
    scrolled.set_child(Some(&label));

    toolbar_view.set_content(Some(&scrolled));
    dialog.set_content(Some(&toolbar_view));
    dialog.present();
}
