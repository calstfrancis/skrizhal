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

/// One logical Markdown block. A bullet or paragraph can span several raw
/// source lines (CHANGELOG.md hard-wraps prose at ~90 columns for
/// readability in a text editor) — those continuation lines get folded into
/// one block and joined with spaces, so the block renders as a single
/// paragraph that reflows naturally to whatever width the dialog actually
/// is, rather than keeping the source file's arbitrary line breaks.
enum Block {
    H1(String),
    H2(String),
    H3(String),
    Bullet(String),
    Para(String),
}

/// Splits `md` into blocks: `#`/`##`/`###` headings, blank lines, and `---`
/// each end whatever block came before; a `- ` line starts a new bullet;
/// any other non-blank line continues the previous bullet/paragraph (or
/// starts a fresh paragraph if the previous block was a heading, or there
/// wasn't one).
fn parse_blocks(md: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "---" {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            blocks.push(Block::H1(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            blocks.push(Block::H2(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("### ") {
            blocks.push(Block::H3(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("- ") {
            blocks.push(Block::Bullet(rest.to_string()));
        } else {
            match blocks.last_mut() {
                Some(Block::Bullet(s)) | Some(Block::Para(s)) => {
                    s.push(' ');
                    s.push_str(trimmed);
                }
                _ => blocks.push(Block::Para(trimmed.to_string())),
            }
        }
    }
    blocks
}

/// Minimal Markdown-to-Pango conversion covering exactly what CHANGELOG.md
/// actually uses: `#`/`##`/`###` headings, `**bold**`, `` `code` ``, `- `
/// lists, blank lines, and `---` separators.
fn markdown_to_pango(md: &str) -> String {
    let mut out = String::new();
    let mut bold = false;
    let mut code = false;
    for block in parse_blocks(md) {
        match block {
            Block::H1(s) => out.push_str(&format!(
                "\n<span weight=\"bold\" size=\"x-large\">{}</span>\n\n",
                inline_markup(&s, &mut bold, &mut code)
            )),
            Block::H2(s) => out.push_str(&format!(
                "\n<span weight=\"bold\" size=\"large\">{}</span>\n",
                inline_markup(&s, &mut bold, &mut code)
            )),
            Block::H3(s) => out.push_str(&format!(
                "\n<b><u>{}</u></b>\n",
                inline_markup(&s, &mut bold, &mut code)
            )),
            Block::Bullet(s) => out.push_str(&format!(
                "  •  {}\n",
                inline_markup(&s, &mut bold, &mut code)
            )),
            Block::Para(s) => {
                out.push_str(&inline_markup(&s, &mut bold, &mut code));
                out.push('\n');
            }
        }
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
        .margin_bottom(24)
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
    // Clamp keeps prose at a comfortable reading width if the window is
    // ever resized wider than that — matches the house convention (see
    // root CLAUDE.md's UI design standard) rather than letting text
    // stretch edge-to-edge.
    let clamp = adw::Clamp::builder().maximum_size(640).child(&label).build();
    scrolled.set_child(Some(&clamp));

    toolbar_view.set_content(Some(&scrolled));
    dialog.set_content(Some(&toolbar_view));
    dialog.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_changelog_produces_valid_pango_markup() {
        let markup = markdown_to_pango(CHANGELOG_MD);
        assert!(
            gtk4::pango::parse_markup(&markup, '\0').is_ok(),
            "markdown_to_pango(CHANGELOG.md) produced invalid Pango markup:\n{markup}"
        );
    }

    #[test]
    fn h1_is_styled_not_left_as_literal_hash() {
        let markup = markdown_to_pango("# Changelog\n\nSome intro text.\n");
        assert!(!markup.contains("# Changelog"));
        assert!(markup.contains("size=\"x-large\""));
        assert!(markup.contains("Changelog"));
    }

    #[test]
    fn wrapped_bullet_lines_join_into_one_paragraph() {
        let md = "- **New File**, **Open** in the header menu — the data file\n  location was previously only changeable via \"Choose Data File…\".\n";
        let markup = markdown_to_pango(md);
        // Should be a single bullet line, not broken mid-sentence — the
        // continuation text must not start a line of its own.
        assert_eq!(markup.matches('\u{2022}').count(), 1);
        assert!(markup.contains("location was previously"));
        assert!(!markup.contains("menu\n  location"));
    }

    #[test]
    fn section_heading_gets_underline_for_distinction_from_inline_bold() {
        let markup = markdown_to_pango("### Added\n- **Something** happened.\n");
        assert!(markup.contains("<b><u>Added</u></b>"));
    }

    #[test]
    fn separate_bullets_stay_separate() {
        let md = "- First item\n- Second item\n";
        let markup = markdown_to_pango(md);
        assert_eq!(markup.matches('\u{2022}').count(), 2);
    }
}
