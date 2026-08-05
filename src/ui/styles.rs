//! Skrizhal's static, app-wide CSS, loaded once at
//! `STYLE_PROVIDER_PRIORITY_APPLICATION`. Follows zerkalo/src/ui/styles.rs.
//!
//! Skrizhal had no stylesheet at all before adopting the suite layer — every
//! surface was whatever Adwaita's default happened to be, which is why the
//! sidebar and the detail pane read as one undifferentiated plane. The classes
//! that fix that come from fond.css; this file only holds what is Skrizhal's
//! own.

/// The suite's shared interface layer, vendored from fond-style. Loaded before
/// GLOBAL_CSS so app rules can still override it. Do not edit the copy in
/// `style/` — change it in fond-style and run its `sync.sh`, or the next sync
/// silently reverts you.
const FOND_CSS: &str = include_str!("../../style/fond.css");

/// Skrizhal's own rules. Deliberately short: the suite layer already settles
/// surfaces, rows, cards and chrome, so anything added here should be
/// something only a CV element database needs.
const GLOBAL_CSS: &str = "\
    /* Paned separators get a hover affordance so it reads as draggable — \
       the same idiom as Zerkalo's, which is where the value came from. */ \
    .paned > separator { \
        min-width: 5px; \
        min-height: 5px; \
        transition: background-color 150ms ease; \
    } \
    .paned > separator:hover { \
        background-color: alpha(@accent_color, 0.45); \
    } \
    /* The sidebar's section label, matching fond.css's section header without \
       needing the dot-and-count structure a full .fond-section carries. */ \
    .sidebar-header { \
        font-size: 0.78em; \
        font-weight: 700; \
        letter-spacing: 0.09em; \
        text-transform: uppercase; \
        opacity: 0.55; \
        padding: 2px 4px; \
    }";

/// Loads all static, app-wide CSS once.
pub fn load_global_css() {
    let css = gtk4::CssProvider::new();
    css.load_from_data(&format!("{FOND_CSS}\n{GLOBAL_CSS}"));
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
