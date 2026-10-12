//! Where the diary file lives.
//!
//! By default it is `%APPDATA%\VibeDiary\entries.json`. The folder can be changed from the
//! settings; the choice is remembered in `%APPDATA%\VibeDiary\config.json`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "entries.json";
const APP_FOLDER: &str = "VibeDiary";

/// How the window opens.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum StartMode {
    /// A normal window.
    #[default]
    Normal,
    /// Fills the screen but keeps the title bar and taskbar.
    Maximized,
    /// Covers the whole screen, taskbar included (F11 switches it off).
    Fullscreen,
}

/// The small plain-text settings file. It holds only things the app needs before the diary is
/// unlocked, so nothing private belongs in it.
#[derive(Serialize, Deserialize, Default, Clone)]
struct Config {
    #[serde(default)]
    data_dir: Option<PathBuf>,
    #[serde(default)]
    start: StartMode,
}

/// Where per-user app files go (`%APPDATA%` on Windows).
fn app_data_root() -> PathBuf {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Tests must never touch the real diary, so while they run everything lives in a throw-away folder.
fn test_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vibe-diary-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn default_dir() -> PathBuf {
    if cfg!(test) {
        return test_dir();
    }
    app_data_root().join(APP_FOLDER)
}

fn config_path() -> PathBuf {
    default_dir().join("config.json")
}

/// The folder named in the config file, or `default` if there is none.
fn resolve(config: &Path, default: &Path) -> PathBuf {
    std::fs::read_to_string(config)
        .ok()
        .and_then(|raw| serde_json::from_str::<Config>(&raw).ok())
        .and_then(|c| c.data_dir)
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| default.to_path_buf())
}

fn load_config(config: &Path) -> Config {
    std::fs::read_to_string(config).ok().and_then(|raw| serde_json::from_str::<Config>(&raw).ok()).unwrap_or_default()
}

fn save_config(config: &Path, settings: &Config) -> Result<(), String> {
    if let Some(parent) = config.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = config.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, config).map_err(|e| e.to_string())
}

/// Remember the data folder (keeping the other settings in the file).
fn write_config(config: &Path, dir: &Path) -> Result<(), String> {
    let mut settings = load_config(config);
    settings.data_dir = Some(dir.to_path_buf());
    save_config(config, &settings)
}

/// How the window should open (normal if nothing has been chosen).
pub fn start_mode() -> StartMode {
    load_config(&config_path()).start
}

/// Remember how the window should open next time (keeping the other settings in the file).
pub fn set_start_mode(mode: StartMode) -> Result<(), String> {
    let mut settings = load_config(&config_path());
    settings.start = mode;
    save_config(&config_path(), &settings)
}

/// Development builds used to keep the diary next to the source code. If there is such a file
/// and nothing at `target` yet, copy it over so the diary carries on. The original is left alone.
fn migrate_legacy(target: &Path, legacy: &Path) -> bool {
    let dest = target.join(FILE_NAME);
    if dest.exists() || !legacy.is_file() {
        return false;
    }
    std::fs::create_dir_all(target).is_ok() && std::fs::copy(legacy, &dest).is_ok()
}

/// Copy the diary file into `to_dir`, check the copy, then remove the original.
fn move_diary(from_file: &Path, to_dir: &Path) -> Result<(), String> {
    let dest = to_dir.join(FILE_NAME);
    if dest == from_file {
        return Ok(());
    }
    if dest.exists() {
        return Err("There is already a diary file in that folder.".into());
    }
    std::fs::create_dir_all(to_dir).map_err(|e| format!("Couldn't create the folder: {e}"))?;
    if from_file.exists() {
        std::fs::copy(from_file, &dest).map_err(|e| format!("Couldn't copy the diary: {e}"))?;
        let same = std::fs::read(from_file).ok() == std::fs::read(&dest).ok();
        if !same {
            std::fs::remove_file(&dest).ok();
            return Err("The copy didn't match the original, so nothing was moved.".into());
        }
    }
    Ok(())
}

static CURRENT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The folder holding the diary file.
pub fn data_dir() -> PathBuf {
    if cfg!(test) {
        return test_dir();
    }
    let mut current = CURRENT.lock().unwrap();
    current
        .get_or_insert_with(|| {
            let dir = resolve(&config_path(), &default_dir());
            let legacy = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE_NAME);
            migrate_legacy(&dir, &legacy);
            dir
        })
        .clone()
}

pub fn data_path() -> PathBuf {
    data_dir().join(FILE_NAME)
}

/// Move the diary file to `dir` and remember it as the new location.
pub fn move_to(dir: PathBuf) -> Result<(), String> {
    let old_file = data_path();
    move_diary(&old_file, &dir)?;
    write_config(&config_path(), &dir)?;
    if old_file != dir.join(FILE_NAME) {
        std::fs::remove_file(&old_file).ok();
    }
    *CURRENT.lock().unwrap() = Some(dir);
    Ok(())
}

/// Start using a diary that already exists in `dir` (nothing is copied or deleted).
pub fn use_existing(dir: PathBuf) -> Result<(), String> {
    if !dir.join(FILE_NAME).is_file() {
        return Err(format!("There is no {FILE_NAME} in that folder."));
    }
    write_config(&config_path(), &dir)?;
    *CURRENT.lock().unwrap() = Some(dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("diary-loc-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn config_overrides_the_default_folder() {
        let dir = temp("config");
        let config = dir.join("sub").join("config.json");
        let default = dir.join("default");
        assert_eq!(resolve(&config, &default), default);
        write_config(&config, &dir.join("chosen")).unwrap();
        assert_eq!(resolve(&config, &default), dir.join("chosen"));
        std::fs::write(&config, "not json").unwrap();
        assert_eq!(resolve(&config, &default), default);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_diary_is_copied_once() {
        let dir = temp("legacy");
        let legacy = dir.join("old.json");
        std::fs::write(&legacy, "secret").unwrap();
        let target = dir.join("new");
        assert!(migrate_legacy(&target, &legacy));
        assert_eq!(std::fs::read_to_string(target.join(FILE_NAME)).unwrap(), "secret");
        assert!(legacy.exists(), "the original is kept");
        std::fs::write(target.join(FILE_NAME), "changed").unwrap();
        assert!(!migrate_legacy(&target, &legacy), "never overwrites");
        assert_eq!(std::fs::read_to_string(target.join(FILE_NAME)).unwrap(), "changed");
        assert!(!migrate_legacy(&dir.join("other"), &dir.join("missing.json")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn moving_copies_and_refuses_to_overwrite() {
        let dir = temp("move");
        let from = dir.join("a").join(FILE_NAME);
        std::fs::create_dir_all(from.parent().unwrap()).unwrap();
        std::fs::write(&from, "data").unwrap();

        let to = dir.join("b");
        move_diary(&from, &to).unwrap();
        assert_eq!(std::fs::read_to_string(to.join(FILE_NAME)).unwrap(), "data");
        // A second move into the same folder is refused rather than overwriting.
        assert!(move_diary(&from, &to).is_err());
        // Moving to where it already is does nothing.
        assert!(move_diary(&from, from.parent().unwrap()).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod isolation {
    use super::*;

    /// Tests run inside the app's own code, so they must never be pointed at the real diary.
    #[test]
    fn tests_never_use_the_real_diary_folder() {
        assert!(data_dir().starts_with(std::env::temp_dir()), "{}", data_dir().display());
        assert!(config_path().starts_with(std::env::temp_dir()));
        let real = app_data_root().join(APP_FOLDER);
        assert!(!data_dir().starts_with(&real) && !config_path().starts_with(&real));
    }
}

#[cfg(test)]
mod start_mode_tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("diary-start-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_start_mode_and_the_data_folder_are_kept_together() {
        let dir = temp("both");
        let config = dir.join("config.json");
        // Nothing saved yet: a normal window.
        assert_eq!(load_config(&config).start, StartMode::Normal);

        let mut settings = load_config(&config);
        settings.start = StartMode::Fullscreen;
        save_config(&config, &settings).unwrap();
        // Choosing a data folder afterwards doesn't forget the start mode...
        write_config(&config, &dir.join("diary")).unwrap();
        let back = load_config(&config);
        assert_eq!((back.start, back.data_dir), (StartMode::Fullscreen, Some(dir.join("diary"))));
        // ...and neither does changing the mode forget the folder.
        let mut again = load_config(&config);
        again.start = StartMode::Maximized;
        save_config(&config, &again).unwrap();
        assert_eq!(resolve(&config, &dir.join("default")), dir.join("diary"));
        assert_eq!(load_config(&config).start, StartMode::Maximized);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn old_and_damaged_settings_files_mean_a_normal_window() {
        let dir = temp("old");
        let config = dir.join("config.json");
        // A file from before the start mode existed.
        std::fs::write(&config, r#"{"data_dir": "C:/Diaries"}"#).unwrap();
        let old = load_config(&config);
        assert_eq!((old.start, old.data_dir), (StartMode::Normal, Some(PathBuf::from("C:/Diaries"))));
        // The mode is written as plain words, and an unknown one falls back safely.
        assert!(serde_json::to_string(&StartMode::Fullscreen).unwrap().contains("fullscreen"));
        std::fs::write(&config, r#"{"start": "sideways"}"#).unwrap();
        assert_eq!(load_config(&config).start, StartMode::Normal);
        std::fs::write(&config, "not json").unwrap();
        assert_eq!(load_config(&config).start, StartMode::Normal);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_real_setting_helpers_use_the_throw_away_folder_in_tests() {
        set_start_mode(StartMode::Maximized).unwrap();
        assert_eq!(start_mode(), StartMode::Maximized);
        assert!(config_path().starts_with(std::env::temp_dir()));
        set_start_mode(StartMode::Normal).unwrap();
        assert_eq!(start_mode(), StartMode::Normal);
    }
}
