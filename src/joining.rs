//! Joining a diary that is already synced from another device (the lock screen's "Join a synced diary").
//!
//! A new device needs three things: where the server is, the sync code, and the diary's password. The server hands
//! over the diary's keyring (the keys, wrapped by the password, so it learns nothing), the password opens it, and an
//! empty diary is made on this PC with the same keys. The first sync then pulls every entry and picture.

use super::*;
use std::path::{Path, PathBuf};
use tideline_core::sync::{self as core_sync, SyncState};

/// Make this PC's copy of a synced diary at `path` (which must not exist yet) and remember the server, so the first
/// sync can fetch everything. Returns the open diary: its key and its (still empty) contents.
pub(crate) fn join_diary(path: &Path, server: &str, code: &str, password: &str) -> Result<(vault::Vault, Data), String> {
    let server = core_sync::clean_server(server);
    let token = code.trim().to_string();
    if server.is_empty() || token.is_empty() {
        return Err("Enter the server address and the sync code.".into());
    }
    if password.is_empty() {
        return Err("Enter the diary's password.".into());
    }
    if path.exists() {
        return Err("There is already a diary here. Nothing was changed.".into());
    }
    let reply = crate::syncing::http(&server, &core_sync::keyring_request(&token))?;
    let keyring = core_sync::keyring_from_response(&reply)?;
    let name = core_sync::name_from_response(&reply);
    let v = vault::Vault::from_keyring(&keyring, password)?;
    let mut data = fresh_data();
    assign_ids(&mut data, &v);
    write_diary(path, &data, &v)?;
    let id = data.extra.get("diary_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    core_sync::save_state(path, &id, &v, &SyncState { server, token, name, ..SyncState::default() })?;
    Ok((v, data))
}

/// Move the diary file into its own folder under `set-aside` (never deleted), where "Use a diary from another folder…"
/// can open it again. Returns the folder.
pub(crate) fn set_aside(file: &Path, data_dir: &Path, stamp: &str) -> Result<PathBuf, String> {
    let folder = data_dir.join("set-aside").join(stamp);
    std::fs::create_dir_all(&folder).map_err(|e| format!("couldn't make {}: {e}", folder.display()))?;
    let dest = folder.join(location::FILE_NAME);
    if std::fs::rename(file, &dest).is_err() {
        std::fs::copy(file, &dest).map_err(|e| format!("couldn't put the diary aside: {e}"))?;
        std::fs::remove_file(file).map_err(|e| format!("couldn't put the diary aside: {e}"))?;
    }
    Ok(folder)
}

impl DiaryApp {
    /// Put this PC's diary aside and go to the Join screen, from Settings → Sync. Nothing is deleted.
    pub(crate) fn join_from_settings(&mut self) {
        let stamp = Local::now().format("%Y-%m-%d-%H%M%S").to_string();
        let file = data_path();
        if file.exists() {
            if let Err(e) = set_aside(&file, &location::data_dir(), &stamp) {
                self.sync.notice = e;
                self.sync.confirm_join = false;
                return;
            }
        }
        self.editor = None;
        self.show_settings = false;
        self.lock_now();
        self.lock = Some(Lock::new(LockMode::Join));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_a_diary_aside_keeps_it_in_a_folder_the_app_can_open_again() {
        let dir = std::env::temp_dir().join(format!("diary-aside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("entries.json");
        std::fs::write(&file, "sealed diary").unwrap();
        let folder = set_aside(&file, &dir, "2026-10-23-130000").unwrap();
        assert!(!file.exists(), "the diary is no longer in the way");
        assert_eq!(std::fs::read_to_string(folder.join("entries.json")).unwrap(), "sealed diary");
        assert_eq!(folder, dir.join("set-aside").join("2026-10-23-130000"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
