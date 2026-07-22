use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use super::state::{self, ChangeCallback, SharedState};
use crate::git_backup;

/// Message for an automatic snapshot. Dated rather than sequential so the
/// history reads as "what the file said on this day", which is the actual
/// question being asked of it ("what did my CV say when I applied there?").
pub fn snapshot_message() -> String {
    format!(
        "Skrizhal snapshot {}",
        chrono_like_timestamp()
    )
}

/// A `YYYY-MM-DD HH:MM` stamp without pulling in a date library for one
/// string — `SystemTime` plus civil-date arithmetic is enough here.
fn chrono_like_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let time_of_day = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        time_of_day / 3600,
        (time_of_day % 3600) / 60
    )
}

/// Howard Hinnant's days-from-civil inverse — the standard branch-free way
/// to turn a Unix day count into a calendar date.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn show(
    parent: &adw::ApplicationWindow,
    state: &SharedState,
    on_change: &ChangeCallback,
) {
    let path = state.borrow().data_path.clone();

    let window = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("File History")
        .default_width(620)
        .default_height(640)
        .build();

    let toasts = adw::ToastOverlay::new();
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

    if !git_backup::is_repo(&path) {
        let status = adw::StatusPage::builder()
            .icon_name("document-open-recent-symbolic")
            .title("Not Version Controlled")
            .description(
                "This file isn't in a git repository, so there's no history to show. \
                 Initializing one lets Skrizhal snapshot the file as you work.",
            )
            .build();
        let init_btn = gtk4::Button::builder()
            .label("Initialize Repository")
            .halign(gtk4::Align::Center)
            .css_classes(["suggested-action", "pill"])
            .build();
        {
            let path = path.clone();
            let toasts = toasts.clone();
            let window = window.clone();
            init_btn.connect_clicked(move |_| match git_backup::init(&path) {
                Ok(()) => {
                    let _ = git_backup::commit(&path, "Initial Skrizhal snapshot");
                    window.close();
                }
                Err(err) => toasts.add_toast(adw::Toast::new(&format!("git init failed: {err}"))),
            });
        }
        status.set_child(Some(&init_btn));
        body.append(&status);
    } else {
        let snapshot_group = adw::PreferencesGroup::builder().build();
        let snapshot_row = adw::ActionRow::builder()
            .title("Snapshot Now")
            .subtitle(if git_backup::has_uncommitted_changes(&path) {
                "This file has changes that aren't committed yet"
            } else {
                "Everything is already committed"
            })
            .build();
        let snapshot_btn = gtk4::Button::builder()
            .label("Commit")
            .valign(gtk4::Align::Center)
            .sensitive(git_backup::has_uncommitted_changes(&path))
            .build();
        snapshot_row.add_suffix(&snapshot_btn);
        snapshot_group.add(&snapshot_row);
        body.append(&snapshot_group);
        {
            let path = path.clone();
            let toasts = toasts.clone();
            let window = window.clone();
            snapshot_btn.connect_clicked(move |_| {
                match git_backup::commit(&path, &snapshot_message()) {
                    Ok(Some(_)) => window.close(),
                    Ok(None) => toasts.add_toast(adw::Toast::new("Nothing to commit.")),
                    Err(err) => {
                        toasts.add_toast(adw::Toast::new(&format!("Commit failed: {err}")))
                    }
                }
            });
        }

        match git_backup::history(&path, 50) {
            Ok(commits) if commits.is_empty() => {
                body.append(
                    &adw::StatusPage::builder()
                        .icon_name("document-open-recent-symbolic")
                        .title("No Snapshots Yet")
                        .description("This file hasn't been committed to the repository yet.")
                        .build(),
                );
            }
            Ok(commits) => {
                let group = adw::PreferencesGroup::builder().title("Snapshots").build();
                for commit in commits {
                    let row = adw::ActionRow::builder()
                        .title(&commit.subject)
                        .subtitle(format!("{} · {}", commit.date, &commit.hash[..7.min(commit.hash.len())]))
                        .build();
                    let restore = gtk4::Button::builder()
                        .label("Restore")
                        .valign(gtk4::Align::Center)
                        .build();
                    row.add_suffix(&restore);
                    group.add(&row);

                    let path = path.clone();
                    let state = state.clone();
                    let on_change = on_change.clone();
                    let toasts = toasts.clone();
                    let window = window.clone();
                    let parent = parent.clone();
                    let hash = commit.hash.clone();
                    let subject = commit.subject.clone();
                    restore.connect_clicked(move |_| {
                        let dialog = adw::MessageDialog::new(
                            Some(&parent),
                            Some("Restore this snapshot?"),
                            Some(&format!(
                                "\"{subject}\" will replace the current contents of the file. \
                                 The current version is committed first, so this is reversible."
                            )),
                        );
                        dialog.add_response("cancel", "Cancel");
                        dialog.add_response("restore", "Restore");
                        dialog
                            .set_response_appearance("restore", adw::ResponseAppearance::Destructive);
                        dialog.set_default_response(Some("cancel"));
                        dialog.set_close_response("cancel");

                        let path = path.clone();
                        let state = state.clone();
                        let on_change = on_change.clone();
                        let toasts = toasts.clone();
                        let window = window.clone();
                        let hash = hash.clone();
                        dialog.connect_response(None, move |dialog, response| {
                            if response == "restore" {
                                // Commit what's there now before overwriting it —
                                // restoring must never be the step that loses work.
                                let _ = git_backup::commit(
                                    &path,
                                    &format!("Before restoring {}", &hash[..7.min(hash.len())]),
                                );
                                match git_backup::file_at(&path, &hash) {
                                    Ok(content) => match std::fs::write(&path, content) {
                                        Ok(()) => {
                                            let _ = state::reload(&state);
                                            on_change(None);
                                            window.close();
                                        }
                                        Err(err) => toasts.add_toast(adw::Toast::new(&format!(
                                            "Couldn't write the file: {err}"
                                        ))),
                                    },
                                    Err(err) => toasts.add_toast(adw::Toast::new(&format!(
                                        "Couldn't read that snapshot: {err}"
                                    ))),
                                }
                            }
                            dialog.close();
                        });
                        dialog.present();
                    });
                }
                body.append(&group);
            }
            Err(err) => {
                body.append(
                    &adw::StatusPage::builder()
                        .icon_name("dialog-warning-symbolic")
                        .title("Couldn't Read History")
                        .description(&err)
                        .build(),
                );
            }
        }
    }

    toasts.set_child(Some(&scrolled));
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&toasts));
    window.set_content(Some(&toolbar));
    window.present();
}

/// Commits the data file if it's in a repo and has changes — called on
/// window close so a session's work lands in history without the user
/// having to remember to snapshot it.
pub fn snapshot_on_close(state: &SharedState) {
    let path = state.borrow().data_path.clone();
    if git_backup::is_repo(&path) {
        let _ = git_backup::commit(&path, &snapshot_message());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        // A leap day, the case naive month arithmetic gets wrong.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }

    #[test]
    fn snapshot_message_has_the_expected_shape() {
        let msg = snapshot_message();
        assert!(msg.starts_with("Skrizhal snapshot "));
        // "Skrizhal snapshot YYYY-MM-DD HH:MM"
        assert_eq!(msg.len(), "Skrizhal snapshot ".len() + 16);
    }
}
