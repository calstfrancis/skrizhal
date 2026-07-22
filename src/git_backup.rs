use std::path::{Path, PathBuf};
use std::process::Command;

/// One commit touching the data file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub date: String,
    pub subject: String,
}

/// Route git through the host when running inside a flatpak sandbox —
/// `org.gnome.Platform` doesn't bundle a git binary, so a bare "git" call
/// would fail there with a confusing "not found". Requires
/// `--talk-name=org.freedesktop.Flatpak` in the manifest's finish-args.
///
/// Lifted wholesale from Retseptura's `git_backup.py`, which solved exactly
/// this problem for exactly this reason — including the `-C` detail below.
fn git_command() -> Command {
    if Path::new("/.flatpak-info").exists() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg("git");
        cmd
    } else {
        Command::new("git")
    }
}

/// `-C <dir>` rather than `Command::current_dir` — under `flatpak-spawn
/// --host`, the working directory applies to the sandboxed client process,
/// not the host-side git it spawns, so it has to be an explicit git argument.
fn run(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = git_command();
    cmd.arg("-C").arg(dir);
    cmd.args(args);
    let output = cmd.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn parent_of(file: &Path) -> Result<PathBuf, String> {
    file.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "Data file has no parent directory.".to_string())
}

pub fn is_repo(file: &Path) -> bool {
    let Ok(dir) = parent_of(file) else {
        return false;
    };
    run(&dir, &["rev-parse", "--git-dir"]).is_ok()
}

pub fn init(file: &Path) -> Result<(), String> {
    let dir = parent_of(file)?;
    run(&dir, &["init"])?;
    Ok(())
}

/// The file's path relative to the repository root, which is what `git show
/// <hash>:<path>` needs — a path relative to the working directory won't
/// resolve unless the file happens to sit at the root.
fn repo_relative(file: &Path) -> Result<String, String> {
    let dir = parent_of(file)?;
    let prefix = run(&dir, &["rev-parse", "--show-prefix"])?.trim().to_string();
    let name = file
        .file_name()
        .ok_or_else(|| "Data file has no name.".to_string())?
        .to_string_lossy()
        .to_string();
    Ok(format!("{prefix}{name}"))
}

/// True if the data file has uncommitted changes.
pub fn has_uncommitted_changes(file: &Path) -> bool {
    let Ok(dir) = parent_of(file) else {
        return false;
    };
    let name = file.file_name().map(|n| n.to_string_lossy().to_string());
    let Some(name) = name else { return false };
    match run(&dir, &["status", "--porcelain", "--", &name]) {
        Ok(out) => !out.trim().is_empty(),
        Err(_) => false,
    }
}

/// Stages and commits just the data file. Returns `Ok(None)` when there was
/// nothing to commit, which is a normal outcome rather than an error — the
/// close-time snapshot hits it constantly.
pub fn commit(file: &Path, message: &str) -> Result<Option<String>, String> {
    let dir = parent_of(file)?;
    let name = file
        .file_name()
        .ok_or_else(|| "Data file has no name.".to_string())?
        .to_string_lossy()
        .to_string();
    if !has_uncommitted_changes(file) {
        return Ok(None);
    }
    run(&dir, &["add", "--", &name])?;
    run(&dir, &["commit", "-m", message, "--", &name])?;
    let hash = run(&dir, &["rev-parse", "HEAD"])?.trim().to_string();
    Ok(Some(hash))
}

pub fn history(file: &Path, limit: usize) -> Result<Vec<Commit>, String> {
    let dir = parent_of(file)?;
    let name = file
        .file_name()
        .ok_or_else(|| "Data file has no name.".to_string())?
        .to_string_lossy()
        .to_string();
    let limit_arg = format!("-{limit}");
    // Unit separator between fields: commit subjects routinely contain
    // spaces, dashes and colons, so anything printable is ambiguous.
    let out = run(
        &dir,
        &[
            "log",
            &limit_arg,
            "--date=short",
            "--format=%H\x1f%ad\x1f%s",
            "--",
            &name,
        ],
    )?;
    Ok(out
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\x1f');
            Some(Commit {
                hash: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
                subject: parts.next().unwrap_or_default().to_string(),
            })
        })
        .collect())
}

/// The data file's full contents as of `hash`.
pub fn file_at(file: &Path, hash: &str) -> Result<String, String> {
    let dir = parent_of(file)?;
    let rel = repo_relative(file)?;
    run(&dir, &["show", &format!("{hash}:{rel}")])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Configures identity locally so the test doesn't depend on (or touch)
    /// the developer's global git config.
    fn init_test_repo(dir: &Path) {
        run(dir, &["init"]).unwrap();
        run(dir, &["config", "user.email", "test@example.com"]).unwrap();
        run(dir, &["config", "user.name", "Test"]).unwrap();
    }

    #[test]
    fn commit_history_and_restore_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        init_test_repo(dir);
        let file = dir.join("cv-elements.yaml");

        std::fs::write(&file, "a:\n  category: Employment\n  title: First\n").unwrap();
        assert!(is_repo(&file));
        assert!(has_uncommitted_changes(&file));
        let first = commit(&file, "first version").unwrap();
        assert!(first.is_some());
        assert!(!has_uncommitted_changes(&file));

        // Nothing changed since — a second snapshot is a no-op, not an error.
        assert_eq!(commit(&file, "no change").unwrap(), None);

        std::fs::write(&file, "a:\n  category: Employment\n  title: Second\n").unwrap();
        commit(&file, "second version").unwrap();

        let log = history(&file, 10).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].subject, "second version");
        assert_eq!(log[1].subject, "first version");
        assert!(!log[0].date.is_empty());

        let old = file_at(&file, &log[1].hash).unwrap();
        assert!(old.contains("First"));
        assert!(!old.contains("Second"));
    }

    #[test]
    fn a_file_in_a_subdirectory_resolves_against_the_repo_root() {
        let tmp = tempfile::tempdir().unwrap();
        init_test_repo(tmp.path());
        let sub = tmp.path().join("data");
        std::fs::create_dir(&sub).unwrap();
        let file = sub.join("cv-elements.yaml");
        std::fs::write(&file, "a:\n  category: Award\n  title: Nested\n").unwrap();

        commit(&file, "nested file").unwrap();
        let log = history(&file, 10).unwrap();
        assert_eq!(log.len(), 1);
        // Would fail with a bare filename if `repo_relative` didn't prepend
        // the `--show-prefix` path.
        assert!(file_at(&file, &log[0].hash).unwrap().contains("Nested"));
    }

    #[test]
    fn a_directory_that_is_not_a_repo_is_reported_as_such() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("cv-elements.yaml");
        std::fs::write(&file, "a: {}\n").unwrap();
        assert!(!is_repo(&file));
        assert!(history(&file, 5).is_err());
    }

    #[test]
    fn init_creates_a_usable_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("cv-elements.yaml");
        std::fs::write(&file, "a: {}\n").unwrap();
        init(&file).unwrap();
        assert!(is_repo(&file));
    }
}
