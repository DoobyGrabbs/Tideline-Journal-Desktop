//! Automatic backups: dated copies of the (already encrypted) diary file, kept in a `backups`
//! folder next to it. A copy is made the first time the diary is unlocked each day, only when
//! the diary has changed, and the oldest are removed beyond the number you choose to keep.
//! Restoring puts an old copy back in place (after saving the current file as a safety copy) and
//! locks the diary, so you unlock it with the password that copy was made with.

use super::*;
use std::path::{Path, PathBuf};

const FOLDER: &str = "backups";
const STAMP: &str = "%Y-%m-%d-%H%M%S";
/// How many automatic backups are kept unless you choose otherwise.
pub(crate) const DEFAULT_KEEP: u32 = 10;
pub(crate) const KEEP_CHOICES: [u32; 6] = [0, 5, 10, 20, 30, 60];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// Made by the app (or "Back up now").
    Auto,
    /// The diary as it was just before a restore replaced it.
    BeforeRestore,
}

#[derive(Clone, Debug)]
pub(crate) struct Backup {
    pub path: PathBuf,
    /// YYYY-MM-DD HH:MM:SS
    pub when: String,
    pub size: u64,
    pub kind: Kind,
}

/// Where the backups of this diary file live.
pub(crate) fn folder_for(file: &Path) -> PathBuf {
    file.parent().unwrap_or_else(|| Path::new(".")).join(FOLDER)
}

fn file_name(stamp: &str, kind: Kind) -> String {
    match kind {
        Kind::Auto => format!("entries-{stamp}.json"),
        Kind::BeforeRestore => format!("entries-{stamp}-before-restore.json"),
    }
}

/// Read `entries-2026-10-12-140300[.json|-before-restore.json]`.
fn parse_name(name: &str) -> Option<(String, Kind)> {
    let rest = name.strip_prefix("entries-")?.strip_suffix(".json")?;
    let (stamp, kind) = match rest.strip_suffix("-before-restore") {
        Some(s) => (s, Kind::BeforeRestore),
        None => (rest, Kind::Auto),
    };
    let dt = chrono::NaiveDateTime::parse_from_str(stamp, STAMP).ok()?;
    Some((dt.format("%Y-%m-%d %H:%M:%S").to_string(), kind))
}

/// All backups in the folder, newest first.
pub(crate) fn list(folder: &Path) -> Vec<Backup> {
    let Ok(read) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut out: Vec<Backup> = read
        .flatten()
        .filter_map(|f| {
            let (when, kind) = parse_name(f.file_name().to_str()?)?;
            let size = f.metadata().ok()?.len();
            Some(Backup { path: f.path(), when, size, kind })
        })
        .collect();
    out.sort_by(|a, b| b.when.cmp(&a.when));
    out
}

fn is_encrypted_diary(path: &Path) -> bool {
    std::fs::read_to_string(path).ok().and_then(|raw| vault::Envelope::parse(&raw)).is_some()
}

fn copy_atomic(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = to.with_extension("json.tmp");
    std::fs::copy(from, &tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, to).map_err(|e| e.to_string())
}

/// Remove the oldest automatic backups beyond `keep`. Safety copies from restores are never
/// removed automatically.
fn prune(folder: &Path, keep: u32) {
    for old in list(folder).into_iter().filter(|b| b.kind == Kind::Auto).skip(keep as usize) {
        std::fs::remove_file(old.path).ok();
    }
}

/// Copy the diary file into the backups folder. `stamp` is the time as `%Y-%m-%d-%H%M%S`.
/// Returns `None` when nothing was needed: backups are off, there is no encrypted diary yet, the
/// newest backup is already identical, or (with `once_per_day`) there is one from today.
pub(crate) fn snapshot(file: &Path, stamp: &str, keep: u32, once_per_day: bool) -> Result<Option<PathBuf>, String> {
    if keep == 0 || !is_encrypted_diary(file) {
        return Ok(None);
    }
    let folder = folder_for(file);
    let existing: Vec<Backup> = list(&folder).into_iter().filter(|b| b.kind == Kind::Auto).collect();
    if once_per_day && existing.first().is_some_and(|b| b.when.get(..10) == Some(stamp_day(stamp).as_str())) {
        return Ok(None);
    }
    let now = std::fs::read(file).map_err(|e| e.to_string())?;
    if let Some(newest) = existing.first() {
        if std::fs::read(&newest.path).ok().as_deref() == Some(&now[..]) {
            return Ok(None);
        }
    }
    let dest = folder.join(file_name(stamp, Kind::Auto));
    copy_atomic(file, &dest)?;
    prune(&folder, keep);
    Ok(Some(dest))
}

/// `2026-10-12-140300` -> `2026-10-12`
fn stamp_day(stamp: &str) -> String {
    stamp.get(..10).unwrap_or("").to_string()
}

/// Put `backup` in place of the diary file. The current file is kept first as a safety copy.
pub(crate) fn restore(file: &Path, backup: &Path, stamp: &str) -> Result<(), String> {
    if !is_encrypted_diary(backup) {
        return Err("that backup could not be read, so the diary was left as it is".into());
    }
    let folder = folder_for(file);
    if file.is_file() {
        copy_atomic(file, &folder.join(file_name(stamp, Kind::BeforeRestore)))?;
    }
    copy_atomic(backup, file)
}

/// Delete one backup (only files this module made).
pub(crate) fn delete(file: &Path, backup: &Path) -> Result<(), String> {
    let name = backup.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if backup.parent() != Some(folder_for(file).as_path()) || parse_name(name).is_none() {
        return Err("that is not one of the diary's backups".into());
    }
    std::fs::remove_file(backup).map_err(|e| e.to_string())
}

pub(crate) fn describe_size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else {
        format!("{} KB", (bytes / 1000).max(1))
    }
}

pub(crate) fn now_stamp() -> String {
    Local::now().format(STAMP).to_string()
}

impl DiaryApp {
    /// The first time the diary is open each day (and only if it changed), make a backup copy.
    pub(crate) fn auto_backup(&mut self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        if self.backup_day == today {
            return;
        }
        self.backup_day = today;
        match snapshot(&data_path(), &now_stamp(), self.data.settings.backup_keep, true) {
            Ok(Some(_)) => self.backups_dirty = true,
            Ok(None) => {}
            Err(e) => self.status = format!("Save failed: the automatic backup could not be made ({e})"),
        }
    }

    /// Put a backup in place of the diary file, then lock so it is opened with its own password.
    pub(crate) fn restore_backup(&mut self, file: &Path, backup: &Path) {
        match restore(file, backup, &now_stamp()) {
            Ok(()) => {
                self.backup_msg.clear();
                self.backup_restore_arm = None;
                self.show_backups = false;
                self.backups_dirty = true;
                self.lock_now();
                if let Some(lock) = self.lock.as_mut() {
                    lock.error = "Backup restored. Unlock with the password that copy was made with.".into();
                }
            }
            Err(e) => self.backup_msg = format!("Save failed: {e}"),
        }
    }

    pub(crate) fn backups_window(&mut self, ctx: &egui::Context) {
        if !self.show_backups {
            return;
        }
        let file = data_path();
        let folder = folder_for(&file);
        if self.backups_dirty {
            self.backup_items = list(&folder);
            self.backups_dirty = false;
        }
        let p = palette(ctx);
        let dark = ctx.theme() == egui::Theme::Dark;
        let mut close = false;
        let mut keep_changed = false;
        let mut make_now = false;
        let mut restore_it = None;
        let mut delete_it = None;
        let mut open_folder = false;

        egui::Window::new("Backups")
            .title_bar(false)
            .frame(popup_frame(&p, dark))
            .resizable(true)
            .default_size([560.0, 520.0])
            .min_size([380.0, 280.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                close |= popup_header(ui, &p, "Backups");
                egui::Frame::new().inner_margin(14).show(ui, |ui| {
                    ui.label(
                        RichText::new("Each day the diary is opened, a dated copy of the encrypted file is saved in the backups folder next to it. Copies are encrypted exactly like the diary, so they are safe to keep and to copy elsewhere.")
                            .color(p.muted),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Automatic backups to keep:").color(p.ink));
                        egui::ComboBox::from_id_salt("backup_keep")
                            .selected_text(if self.data.settings.backup_keep == 0 { "Off".to_string() } else { self.data.settings.backup_keep.to_string() })
                            .show_ui(ui, |ui| {
                                for n in KEEP_CHOICES {
                                    let label = if n == 0 { "Off".to_string() } else { n.to_string() };
                                    keep_changed |= ui.selectable_value(&mut self.data.settings.backup_keep, n, label).changed();
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Back up now").clicked() {
                            make_now = true;
                        }
                        if ui.button("Open backups folder").clicked() {
                            open_folder = true;
                        }
                    });
                    if !self.backup_msg.is_empty() {
                        ui.label(RichText::new(&self.backup_msg).color(p.title));
                    }
                    ui.add_space(6.0);
                    ui.separator();
                    if self.backup_items.is_empty() {
                        ui.label(RichText::new("No backups yet.").color(p.ink));
                    }
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        for b in &self.backup_items {
                            egui::Frame::new().fill(p.cell).stroke(Stroke::new(1.0, p.cell_border)).corner_radius(10).inner_margin(10).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(b.when.get(..16).unwrap_or(&b.when)).strong().color(p.ink));
                                    ui.label(RichText::new(describe_size(b.size)).small().color(p.muted));
                                    if b.kind == Kind::BeforeRestore {
                                        ui.label(RichText::new("kept before a restore").small().color(p.muted));
                                    }
                                });
                                ui.horizontal(|ui| {
                                    let armed = self.backup_restore_arm.as_ref() == Some(&b.path);
                                    let label = if armed { "Click again to restore this copy" } else { "Restore…" };
                                    if ui.button(label).clicked() {
                                        if armed {
                                            restore_it = Some(b.path.clone());
                                        } else {
                                            self.backup_restore_arm = Some(b.path.clone());
                                        }
                                    }
                                    if ui.add(egui::Button::new(RichText::new("Delete").color(theme::BG)).fill(RED)).clicked() {
                                        delete_it = Some(b.path.clone());
                                    }
                                });
                                if self.backup_restore_arm.as_ref() == Some(&b.path) {
                                    ui.label(
                                        RichText::new("The diary will lock and this copy will replace the current one (which is saved as a safety copy first). Unlock it with the password this copy had when it was made.")
                                            .small()
                                            .color(p.muted),
                                    );
                                }
                            });
                            ui.add_space(6.0);
                        }
                    });
                });
            });

        if keep_changed {
            self.persist();
            if self.data.settings.backup_keep > 0 {
                prune(&folder, self.data.settings.backup_keep);
            }
            self.backups_dirty = true;
        }
        if make_now {
            // A manual backup always makes a new copy unless nothing has changed.
            self.backup_msg = match snapshot(&file, &now_stamp(), self.data.settings.backup_keep.max(1), false) {
                Ok(Some(_)) => "Backup made.".into(),
                Ok(None) => "Nothing has changed since the last backup.".into(),
                Err(e) => format!("Save failed: the backup could not be made ({e})"),
            };
            self.backups_dirty = true;
        }
        if open_folder {
            std::fs::create_dir_all(&folder).ok();
            if std::process::Command::new("explorer").arg(&folder).spawn().is_err() {
                self.backup_msg = format!("The backups are in {}", folder.display());
            }
        }
        if let Some(path) = delete_it {
            self.backup_msg = match delete(&file, &path) {
                Ok(()) => "Backup deleted.".into(),
                Err(e) => format!("Save failed: {e}"),
            };
            self.backup_restore_arm = None;
            self.backups_dirty = true;
        }
        if let Some(path) = restore_it {
            self.restore_backup(&file, &path);
        }
        if close {
            self.show_backups = false;
            self.backup_restore_arm = None;
            self.backup_msg.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("diary-backups-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A real encrypted diary file with the given contents.
    fn diary_file(path: &Path, password: &str, text: &str) {
        let v = vault::Vault::create(password).unwrap();
        let mut data = fresh_data();
        data.entries.push(Entry { id: 1, date: "2026-10-12".into(), added_at: "2026-10-12 10:00:00".into(), category: "Personal".into(), text: text.into(), ..Default::default() });
        save_to(path, &data, &v);
    }

    fn save_to(path: &Path, data: &Data, v: &vault::Vault) {
        std::fs::write(path, v.seal(&serde_json::to_vec(data).unwrap()).unwrap()).unwrap();
    }

    fn names(folder: &Path) -> Vec<String> {
        list(folder).into_iter().map(|b| b.path.file_name().unwrap().to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn file_names_round_trip() {
        assert_eq!(file_name("2026-10-12-140300", Kind::Auto), "entries-2026-10-12-140300.json");
        assert_eq!(parse_name("entries-2026-10-12-140300.json"), Some(("2026-10-12 14:03:00".into(), Kind::Auto)));
        assert_eq!(parse_name("entries-2026-10-12-140300-before-restore.json"), Some(("2026-10-12 14:03:00".into(), Kind::BeforeRestore)));
        for bad in ["entries.json", "entries-x.json", "notes-2026-10-12-140300.json", "entries-2026-10-12-140300.txt", "entries-2026-13-12-140300.json"] {
            assert_eq!(parse_name(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_backup_is_an_exact_copy_and_never_made_from_nothing() {
        let dir = temp("copy");
        let file = dir.join("entries.json");
        // No diary yet, a plain-text file and an empty one are all left alone.
        assert_eq!(snapshot(&file, "2026-10-12-100000", 10, false).unwrap(), None);
        std::fs::write(&file, "{\"not\": \"encrypted\"}").unwrap();
        assert_eq!(snapshot(&file, "2026-10-12-100000", 10, false).unwrap(), None);
        assert!(!folder_for(&file).exists(), "no folder is made for nothing");

        diary_file(&file, "pw", "hello");
        let made = snapshot(&file, "2026-10-12-100000", 10, false).unwrap().unwrap();
        assert_eq!(std::fs::read(&made).unwrap(), std::fs::read(&file).unwrap());
        assert!(is_encrypted_diary(&made));
        assert!(!std::fs::read_to_string(&made).unwrap().contains("hello"), "the backup holds no plain text");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nothing_is_copied_when_nothing_changed_or_backups_are_off() {
        let dir = temp("same");
        let file = dir.join("entries.json");
        diary_file(&file, "pw", "one");
        assert!(snapshot(&file, "2026-10-12-100000", 10, false).unwrap().is_some());
        assert_eq!(snapshot(&file, "2026-10-12-110000", 10, false).unwrap(), None, "identical to the newest backup");
        assert_eq!(names(&folder_for(&file)).len(), 1);

        diary_file(&file, "pw", "two");
        assert_eq!(snapshot(&file, "2026-10-12-120000", 0, false).unwrap(), None, "turned off");
        assert!(snapshot(&file, "2026-10-12-120000", 10, false).unwrap().is_some());
        assert_eq!(names(&folder_for(&file)).len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn once_a_day_means_one_per_calendar_day() {
        let dir = temp("daily");
        let file = dir.join("entries.json");
        diary_file(&file, "pw", "a");
        assert!(snapshot(&file, "2026-10-12-080000", 10, true).unwrap().is_some());
        diary_file(&file, "pw", "b");
        assert_eq!(snapshot(&file, "2026-10-12-200000", 10, true).unwrap(), None, "already one today");
        assert!(snapshot(&file, "2026-10-13-080000", 10, true).unwrap().is_some(), "a new day");
        // "Back up now" ignores the once-a-day rule.
        diary_file(&file, "pw", "c");
        assert!(snapshot(&file, "2026-10-13-090000", 10, false).unwrap().is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_the_newest_few_are_kept() {
        let dir = temp("prune");
        let file = dir.join("entries.json");
        for day in 1..=6 {
            diary_file(&file, "pw", &format!("day {day}"));
            snapshot(&file, &format!("2026-10-{day:02}-080000"), 3, true).unwrap();
        }
        assert_eq!(names(&folder_for(&file)), ["entries-2026-10-06-080000.json", "entries-2026-10-05-080000.json", "entries-2026-10-04-080000.json"]);

        // Safety copies from restores are not counted or pruned.
        std::fs::write(folder_for(&file).join("entries-2026-09-01-080000-before-restore.json"), "x").unwrap();
        diary_file(&file, "pw", "day 7");
        snapshot(&file, "2026-10-07-080000", 3, true).unwrap();
        let n = names(&folder_for(&file));
        assert_eq!(n.len(), 4);
        assert!(n.contains(&"entries-2026-09-01-080000-before-restore.json".to_string()));
        // Other files in the folder are never touched.
        std::fs::write(folder_for(&file).join("readme.txt"), "mine").unwrap();
        prune(&folder_for(&file), 1);
        assert!(folder_for(&file).join("readme.txt").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn restoring_brings_back_the_old_diary_and_keeps_the_current_one() {
        let dir = temp("restore");
        let file = dir.join("entries.json");
        diary_file(&file, "old password", "the old days");
        let backup = snapshot(&file, "2026-10-01-080000", 10, false).unwrap().unwrap();
        let old_bytes = std::fs::read(&file).unwrap();

        diary_file(&file, "new password", "the new days");
        let new_bytes = std::fs::read(&file).unwrap();
        restore(&file, &backup, "2026-10-12-090000").unwrap();

        assert_eq!(std::fs::read(&file).unwrap(), old_bytes);
        let safety = folder_for(&file).join("entries-2026-10-12-090000-before-restore.json");
        assert_eq!(std::fs::read(&safety).unwrap(), new_bytes, "the diary being replaced is kept");
        // The restored file opens with the old password, the safety copy with the new one.
        let env = vault::Envelope::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert!(vault::Vault::unlock(&env, "old password").is_ok() && vault::Vault::unlock(&env, "new password").is_err());
        let env = vault::Envelope::parse(&std::fs::read_to_string(&safety).unwrap()).unwrap();
        assert!(vault::Vault::unlock(&env, "new password").is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_damaged_backup_is_refused_and_nothing_changes() {
        let dir = temp("damaged");
        let file = dir.join("entries.json");
        diary_file(&file, "pw", "precious");
        let before = std::fs::read(&file).unwrap();
        let folder = folder_for(&file);
        std::fs::create_dir_all(&folder).unwrap();
        let bad = folder.join("entries-2026-10-01-080000.json");
        std::fs::write(&bad, "garbage").unwrap();
        assert!(restore(&file, &bad, "2026-10-12-090000").is_err());
        assert_eq!(std::fs::read(&file).unwrap(), before);
        assert_eq!(names(&folder), ["entries-2026-10-01-080000.json"], "no safety copy for a refused restore");
        assert!(restore(&file, &folder.join("missing.json"), "2026-10-12-090000").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn deleting_only_removes_the_diarys_own_backups() {
        let dir = temp("delete");
        let file = dir.join("entries.json");
        diary_file(&file, "pw", "x");
        let backup = snapshot(&file, "2026-10-01-080000", 10, false).unwrap().unwrap();
        // The diary itself, a stranger's file and a path outside the folder are all refused.
        assert!(delete(&file, &file).is_err());
        let other = folder_for(&file).join("photos.json");
        std::fs::write(&other, "x").unwrap();
        assert!(delete(&file, &other).is_err() && other.exists());
        let outside = dir.join("entries-2026-10-02-080000.json");
        std::fs::write(&outside, "x").unwrap();
        assert!(delete(&file, &outside).is_err() && outside.exists());
        delete(&file, &backup).unwrap();
        assert!(!backup.exists() && file.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(describe_size(10), "1 KB");
        assert_eq!(describe_size(48_000), "48 KB");
        assert_eq!(describe_size(2_500_000), "2.5 MB");
    }
}
