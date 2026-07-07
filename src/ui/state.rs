use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use skrizhal_core::CvEntry;

pub struct AppState {
    pub entries: Vec<CvEntry>,
    pub data_path: PathBuf,
    pub selected_key: Option<String>,
    /// Set when `data_path` exists but failed to parse on load — saving is
    /// refused while this is true so a bad-but-recoverable file on disk is
    /// never silently clobbered with an empty entry list.
    pub load_blocked: bool,
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

pub fn load_initial(data_path: PathBuf) -> (AppState, Option<String>) {
    if !data_path.exists() {
        return (
            AppState {
                entries: Vec::new(),
                data_path,
                selected_key: None,
                load_blocked: false,
                search: String::new(),
                filter_category: None,
                filter_tag: None,
            },
            None,
        );
    }
    match skrizhal_core::load_file(&data_path) {
        Ok(entries) => (
            AppState {
                entries,
                data_path,
                selected_key: None,
                load_blocked: false,
                search: String::new(),
                filter_category: None,
                filter_tag: None,
            },
            None,
        ),
        Err(err) => (
            AppState {
                entries: Vec::new(),
                data_path,
                selected_key: None,
                load_blocked: true,
                search: String::new(),
                filter_category: None,
                filter_tag: None,
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
        return Ok(());
    }
    match skrizhal_core::load_file(&path) {
        Ok(entries) => {
            let mut s = state.borrow_mut();
            s.entries = entries;
            s.load_blocked = false;
            Ok(())
        }
        Err(err) => Err(format!("{err}")),
    }
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
    skrizhal_core::save_file(&s.data_path, &s.entries).map_err(|e| e.to_string())
}
