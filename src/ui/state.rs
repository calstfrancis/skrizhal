use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use skrizhal_core::CvEntry;

pub struct AppState {
    pub entries: Vec<CvEntry>,
    pub data_path: PathBuf,
    pub selected_key: Option<String>,
    /// Set when `data_path` exists but its YAML couldn't be parsed at all
    /// (bad syntax, not a mapping) — saving is refused while this is true so
    /// a bad-but-recoverable file on disk is never silently clobbered with an
    /// empty entry list.
    pub load_blocked: bool,
    /// `(key, message)` for individual entries that failed to parse (e.g. a
    /// missing required field) even though the file as a whole is valid
    /// YAML. These entries aren't shown/editable, but their raw YAML is kept
    /// in `raw_failed` and written back unchanged on every save, so nothing
    /// on disk is lost just because Skrizhal doesn't understand it yet.
    pub parse_warnings: Vec<(String, String)>,
    raw_failed: BTreeMap<String, serde_yaml_ng::Value>,
    pub search: String,
    pub filter_category: Option<String>,
    pub filter_tag: Option<String>,
}

pub type SharedState = Rc<RefCell<AppState>>;

/// Called after any mutation (add/edit/duplicate/delete/tag-rename) with the
/// key that should end up selected (or `None`). Implemented once in
/// `app_window.rs`, where it persists to disk, refreshes the sidebar list
/// and filters, and loads the selection into the detail pane.
pub type ChangeCallback = Rc<dyn Fn(Option<String>)>;

fn empty_state(data_path: PathBuf) -> AppState {
    AppState {
        entries: Vec::new(),
        data_path,
        selected_key: None,
        load_blocked: false,
        parse_warnings: Vec::new(),
        raw_failed: BTreeMap::new(),
        search: String::new(),
        filter_category: None,
        filter_tag: None,
    }
}

/// Returns a human-readable summary of `parse_warnings`, if any, suitable
/// for a toast — e.g. `"2 entries couldn't be read: guelphEconomics, yorkMES"`.
pub fn parse_warnings_summary(warnings: &[(String, String)]) -> Option<String> {
    if warnings.is_empty() {
        return None;
    }
    let keys: Vec<&str> = warnings.iter().map(|(k, _)| k.as_str()).collect();
    Some(format!(
        "{} {} couldn't be read and {} left untouched on disk: {}",
        warnings.len(),
        if warnings.len() == 1 { "entry" } else { "entries" },
        if warnings.len() == 1 { "was" } else { "were" },
        keys.join(", "),
    ))
}

pub fn load_initial(data_path: PathBuf) -> (AppState, Option<String>) {
    if !data_path.exists() {
        return (empty_state(data_path), None);
    }
    match skrizhal_core::load_file(&data_path) {
        Ok(outcome) => {
            let warning = parse_warnings_summary(&outcome.failed);
            let mut s = empty_state(data_path);
            s.entries = outcome.entries;
            s.parse_warnings = outcome.failed;
            s.raw_failed = outcome.raw_failed;
            (s, warning)
        }
        Err(err) => (
            AppState {
                load_blocked: true,
                ..empty_state(data_path)
            },
            Some(format!("{err}")),
        ),
    }
}

/// Re-reads `data_path` from disk, clearing `load_blocked` on success.
/// Returns an error message on failure without touching `entries`.
pub fn reload(state: &SharedState) -> Result<(), String> {
    let path = state.borrow().data_path.clone();
    if !path.exists() {
        let mut s = state.borrow_mut();
        s.entries = Vec::new();
        s.load_blocked = false;
        s.parse_warnings = Vec::new();
        s.raw_failed = BTreeMap::new();
        return Ok(());
    }
    match skrizhal_core::load_file(&path) {
        Ok(outcome) => {
            let mut s = state.borrow_mut();
            s.entries = outcome.entries;
            s.load_blocked = false;
            s.parse_warnings = outcome.failed;
            s.raw_failed = outcome.raw_failed;
            Ok(())
        }
        Err(err) => Err(format!("{err}")),
    }
}

/// Starts a brand new, empty data file at `path` — used by New File. Doesn't
/// touch anything at `path` beyond an immediate `persist()` by the caller;
/// if a file already exists there, the next persist overwrites it.
pub fn start_new_file(state: &SharedState, path: PathBuf) {
    let mut s = state.borrow_mut();
    *s = empty_state(path);
}

/// Save As: keeps the current entries, points `data_path` at `path`. The
/// caller is responsible for persisting afterward.
pub fn set_data_path(state: &SharedState, path: PathBuf) {
    state.borrow_mut().data_path = path;
}

pub fn persist(state: &SharedState) -> Result<(), String> {
    let s = state.borrow();
    if s.load_blocked {
        return Err(
            "Not saving: the data file has a parse error. Fix it externally, then Reload."
                .to_string(),
        );
    }
    if let Some(dir) = s.data_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    skrizhal_core::save_file(&s.data_path, &s.entries, &s.raw_failed).map_err(|e| e.to_string())
}
