//! Where the diary file lives.
//!
//! By default the diary lives in a `data` folder next to the program (`data/entries.json`, with
//! its backups in `data/backups`), where it is easy to find. While developing, "next to the
//! program" means the project folder. The folder can be changed from the settings; the choice is
//! remembered in `data/config.json`. Diaries kept by earlier versions in `%APPDATA%/VibeDiary`
//! are copied across the first time (the originals are left alone).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "entries.json";
/// The folder earlier versions used inside `%APPDATA%`.
const LEGACY_APPDATA_FOLDER: &str = "VibeDiary";
/// The default diary folder, next to the program.
const DATA_FOLDER: &str = "data";

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
    /// The chosen colour theme's accent colour (None = the original blue).
    #[serde(default)]
    accent: Option<[u8; 3]>,
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
    let dir = std::env::temp_dir().join(format!("tideline-journal-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// The folder the program lives in. A program run from the build folder (`target\...`) counts as
/// living in the project folder, so development and the installed copy both get a `data` folder
/// they can be seen to own.
fn app_home() -> PathBuf {
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    match exe_dir {
        Some(dir) if dir.ancestors().any(|a| a.file_name().is_some_and(|n| n == "target")) => PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        Some(dir) => dir,
        None => PathBuf::from("."),
    }
}

fn default_dir() -> PathBuf {
    if cfg!(test) {
        return test_dir();
    }
    // `TIDELINE_DATA_DIR` points the program somewhere else entirely (demo mode uses it).
    if let Some(dir) = std::env::var_os("TIDELINE_DATA_DIR") {
        return PathBuf::from(dir);
    }
    app_home().join(DATA_FOLDER)
}

/// Where earlier versions kept the diary and the settings file.
fn legacy_appdata_dir() -> PathBuf {
    app_data_root().join(LEGACY_APPDATA_FOLDER)
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

/// The saved accent colour for the colour theme, if one was chosen.
pub fn accent() -> Option<[u8; 3]> {
    load_config(&config_path()).accent
}

/// Remember the accent colour (None goes back to the original blue), keeping the other settings.
pub fn set_accent(accent: Option<[u8; 3]>) -> Result<(), String> {
    let mut settings = load_config(&config_path());
    settings.accent = accent;
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
            let custom = std::env::var_os("TIDELINE_DATA_DIR").is_some();
            if !custom {
                // Settings made in the old place come along (once).
                let old_config = legacy_appdata_dir().join("config.json");
                if !config_path().exists() && old_config.is_file() {
                    std::fs::create_dir_all(default_dir()).ok();
                    std::fs::copy(&old_config, config_path()).ok();
                }
            }
            let dir = resolve(&config_path(), &default_dir());
            if !custom {
                // Diaries kept by earlier versions are copied across (the originals stay).
                migrate_legacy(&dir, &legacy_appdata_dir().join(FILE_NAME));
                migrate_legacy(&dir, &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FILE_NAME));
            }
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
        for real in [legacy_appdata_dir(), app_home().join(DATA_FOLDER)] {
            assert!(!data_dir().starts_with(&real) && !config_path().starts_with(&real), "{}", real.display());
        }
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
        let _turn = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_start_mode(StartMode::Maximized).unwrap();
        assert_eq!(start_mode(), StartMode::Maximized);
        assert!(config_path().starts_with(std::env::temp_dir()));
        set_start_mode(StartMode::Normal).unwrap();
        assert_eq!(start_mode(), StartMode::Normal);
    }
}

#[cfg(test)]
mod accent_tests {
    use super::*;

    #[test]
    fn the_accent_is_kept_with_the_other_settings_and_old_files_have_none() {
        let dir = std::env::temp_dir().join(format!("diary-accent-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.json");
        std::fs::write(&config, r#"{"data_dir": "C:/Diaries", "start": "maximized"}"#).unwrap();
        assert_eq!(load_config(&config).accent, None);

        let mut settings = load_config(&config);
        settings.accent = Some([225, 29, 72]);
        save_config(&config, &settings).unwrap();
        write_config(&config, &dir.join("moved")).unwrap();
        let back = load_config(&config);
        assert_eq!((back.accent, back.start), (Some([225, 29, 72]), StartMode::Maximized));
        assert_eq!(back.data_dir, Some(dir.join("moved")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_real_accent_helpers_use_the_throw_away_folder_in_tests() {
        let _turn = CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_accent(Some([13, 148, 136])).unwrap();
        assert_eq!(accent(), Some([13, 148, 136]));
        set_accent(None).unwrap();
        assert_eq!(accent(), None);
    }
}

/// Tests that use the real settings helpers share one throw-away settings file, so they take turns.
#[cfg(test)]
pub(crate) static CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod default_folder_tests {
    use super::*;

    #[test]
    fn a_program_run_from_the_build_folder_lives_in_the_project_folder() {
        // Tests run from `target/.../deps`, so "next to the program" is the project folder.
        assert_eq!(app_home(), PathBuf::from(env!("CARGO_MANIFEST_DIR")));
        assert_eq!(app_home().join(DATA_FOLDER).file_name().unwrap(), "data");
    }

    #[test]
    fn an_old_diary_is_copied_into_the_data_folder_but_never_over_one_that_is_there() {
        let dir = std::env::temp_dir().join(format!("diary-default-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let (old, new) = (dir.join("old"), dir.join("data"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join(FILE_NAME), "old diary").unwrap();
        assert!(migrate_legacy(&new, &old.join(FILE_NAME)));
        assert_eq!(std::fs::read_to_string(new.join(FILE_NAME)).unwrap(), "old diary");
        assert!(old.join(FILE_NAME).exists(), "the original stays");
        // A diary already in the data folder is never replaced.
        std::fs::write(new.join(FILE_NAME), "current diary").unwrap();
        assert!(!migrate_legacy(&new, &old.join(FILE_NAME)));
        assert_eq!(std::fs::read_to_string(new.join(FILE_NAME)).unwrap(), "current diary");
        std::fs::remove_dir_all(&dir).ok();
    }
}
