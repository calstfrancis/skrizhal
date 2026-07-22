use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use skrizhal_core::{CvEntry, Profile, SortMode};

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
    pub filter_tags: Vec<String>,
    pub sort_mode: SortMode,
    /// CV profiles from the file's reserved `_profiles` key. Saved back
    /// alongside the entries on every persist.
    pub profiles: Vec<Profile>,
    /// Modification time of `data_path` as of the last read or write Skrizhal
    /// itself performed. The file monitor compares against it to tell an
    /// external edit apart from the echo of our own save.
    pub last_known_mtime: Option<SystemTime>,
    undo_stack: Vec<Vec<CvEntry>>,
    redo_stack: Vec<Vec<CvEntry>>,
}

/// Cap on undo history — snapshots are whole-entries clones, so this bounds
/// memory use rather than keeping every edit ever made in a session.
const UNDO_LIMIT: usize = 50;

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
        filter_tags: Vec::new(),
        sort_mode: SortMode::default(),
        profiles: Vec::new(),
        last_known_mtime: None,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    }
}

/// Snapshots the current entries onto the undo stack and clears redo —
/// call this immediately before mutating `entries` at any call site (add,
/// edit, duplicate, delete, tag rename). Every mutation site is responsible for calling this itself;
/// there's no single choke point to hook automatically since callers mutate
/// `state.borrow_mut().entries` directly rather than going through a shared
/// mutation function.
pub fn push_undo(state: &SharedState) {
    let mut s = state.borrow_mut();
    let snapshot = s.entries.clone();
    s.undo_stack.push(snapshot);
    if s.undo_stack.len() > UNDO_LIMIT {
        s.undo_stack.remove(0);
    }
    s.redo_stack.clear();
}

pub fn can_undo(state: &SharedState) -> bool {
    !state.borrow().undo_stack.is_empty()
}

pub fn can_redo(state: &SharedState) -> bool {
    !state.borrow().redo_stack.is_empty()
}

/// Restores the previous entries snapshot, if any. Returns whether it did
/// anything — the caller still needs to persist and refresh the UI. Doesn't
/// touch `selected_key`; a selection pointing at an entry that no longer
/// exists post-undo is handled the same way as any other stale selection
/// (the sidebar refresh clears it when it can't find a matching row).
pub fn undo(state: &SharedState) -> bool {
    let mut s = state.borrow_mut();
    let Some(prev) = s.undo_stack.pop() else {
        return false;
    };
    let current = s.entries.clone();
    s.redo_stack.push(current);
    s.entries = prev;
    true
}

pub fn redo(state: &SharedState) -> bool {
    let mut s = state.borrow_mut();
    let Some(next) = s.redo_stack.pop() else {
        return false;
    };
    let current = s.entries.clone();
    s.undo_stack.push(current);
    s.entries = next;
    true
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

pub fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

/// True if `data_path` has been modified by something other than Skrizhal
/// since our last read or write.
pub fn changed_externally(state: &SharedState) -> bool {
    let s = state.borrow();
    let on_disk = file_mtime(&s.data_path);
    on_disk.is_some() && on_disk != s.last_known_mtime
}

pub fn load_initial(data_path: PathBuf) -> (AppState, Option<String>) {
    if !data_path.exists() {
        return (empty_state(data_path), None);
    }
    match skrizhal_core::load_file(&data_path) {
        Ok(outcome) => {
            let warning = parse_warnings_summary(&outcome.failed);
            let mut s = empty_state(data_path);
            s.last_known_mtime = file_mtime(&s.data_path);
            s.entries = outcome.entries;
            s.profiles = outcome.profiles;
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
        s.profiles = Vec::new();
        s.last_known_mtime = None;
        s.undo_stack.clear();
        s.redo_stack.clear();
        return Ok(());
    }
    match skrizhal_core::load_file(&path) {
        Ok(outcome) => {
            let mut s = state.borrow_mut();
            s.entries = outcome.entries;
            s.profiles = outcome.profiles;
            s.load_blocked = false;
            s.parse_warnings = outcome.failed;
            s.raw_failed = outcome.raw_failed;
            s.last_known_mtime = file_mtime(&path);
            s.undo_stack.clear();
            s.redo_stack.clear();
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
    // Sort order is a view preference, not file content — a new file
    // shouldn't silently snap the list back to the default ordering.
    let sort_mode = s.sort_mode;
    *s = empty_state(path);
    s.sort_mode = sort_mode;
}

/// Save As: keeps the current entries, points `data_path` at `path`. The
/// caller is responsible for persisting afterward.
pub fn set_data_path(state: &SharedState, path: PathBuf) {
    state.borrow_mut().data_path = path;
}

pub fn persist(state: &SharedState) -> Result<(), String> {
    let mut s = state.borrow_mut();
    if s.load_blocked {
        return Err(
            "Not saving: the data file has a parse error. Fix it externally, then Reload."
                .to_string(),
        );
    }
    if let Some(dir) = s.data_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    skrizhal_core::save_file(&s.data_path, &s.entries, &s.raw_failed, &s.profiles)
        .map_err(|e| e.to_string())?;
    // Recorded after the write so the monitor recognizes this change as ours.
    s.last_known_mtime = file_mtime(&s.data_path);
    Ok(())
}
