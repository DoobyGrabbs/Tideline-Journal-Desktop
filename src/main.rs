use std::collections::BTreeMap;

use chrono::{Datelike, Local, NaiveDate, Timelike};
use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, Stroke, vec2};
use serde::{Deserialize, Serialize};

use location::data_path;

mod icon;
mod joining;
mod kit;
mod ai;
mod export;
mod demo;
mod editor;
mod fonts;
mod gallery;
mod live;
mod insights;
mod backups;
mod location;
mod markdown;
mod ollama;
mod pinned;
mod privacy;
mod recovery;
mod reports;
mod search;
mod settings;
mod stats;
mod syncing;
mod templates;
mod theme;
mod trash;
// The vault (password-based encryption of the diary file) lives in the shared core.
use tideline_core::vault;

/// The accent colour for fills (white-ish text goes on top of it, see `theme::Shades::on_accent`).
fn accent_fill() -> Color32 {
    theme::shades().accent
}

const RED: Color32 = theme::DANGER;
const GREY: [u8; 3] = [156, 163, 175];
/// The PDF user guide, built from `docs/user-guide` and offered under Settings → Help.
const USER_GUIDE: &[u8] = include_bytes!("../assets/user-guide.pdf");

/// The colours the windows draw with. Tideline Journal is a dark design, so there is one set.
struct Palette {
    page_bg: Color32,
    panel_bg: Color32,
    accent: Color32,
    cell: Color32,
    surface: Color32,
    cell_border: Color32,
    border_soft: Color32,
    today: Color32,
    hover: Color32,
    head_bg: Color32,
    head_text: Color32,
    day_num: Color32,
    /// Secondary text.
    muted: Color32,
    /// Labels and counts.
    faint: Color32,
    title: Color32,
    ink: Color32,
}

fn palette(_ctx: &egui::Context) -> Palette {
    let t = theme::shades();
    Palette {
        page_bg: theme::BG,
        panel_bg: theme::PANEL,
        accent: t.accent,
        cell: theme::CELL,
        surface: theme::SURFACE,
        cell_border: theme::BORDER,
        border_soft: theme::BORDER_SOFT,
        today: theme::TODAY,
        hover: theme::SURFACE,
        head_bg: theme::SURFACE,
        head_text: theme::TEXT_2,
        day_num: theme::TEXT,
        muted: theme::TEXT_2,
        faint: theme::TEXT_3,
        title: theme::TEXT,
        ink: theme::TEXT,
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Category {
    name: String,
    color: [u8; 3],
    /// Fields written by a newer version of the app: kept untouched so saving never loses them.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct Entry {
    id: u64,
    /// The diary day the entry belongs to, YYYY-MM-DD.
    date: String,
    /// When the entry was first written, YYYY-MM-DD HH:MM:SS (local time).
    added_at: String,
    category: String,
    text: String,
    /// How the day felt, 1 (really sad) to 5 (really happy).
    #[serde(default)]
    mood: Option<u8>,
    /// Pinned entries are listed under "Pinned" and starred on the calendar.
    #[serde(default)]
    pinned: bool,
    /// Private entries stay hidden until you click to reveal them.
    #[serde(default)]
    sensitive: bool,
    /// Files attached to the entry (the data is in `Data::files`).
    #[serde(default)]
    attachments: Vec<Attachment>,
    /// Some of the text was drafted with the AI helpers (and then edited by you).
    #[serde(default)]
    ai: bool,
    /// Written entirely by the AI (a day summary): left out of word counts and statistics.
    #[serde(default)]
    generated: bool,
    /// Fields written by a newer version of the app: kept untouched so saving never loses them.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Data {
    categories: Vec<Category>,
    entries: Vec<Entry>,
    /// Images used by entries: id -> base64 of the (re-encoded) image file.
    #[serde(default)]
    images: BTreeMap<String, String>,
    /// Attached files: id -> base64 of the file.
    #[serde(default)]
    files: BTreeMap<String, String>,
    /// Deleted entries, kept for 30 days.
    #[serde(default)]
    trash: Vec<trash::Trashed>,
    /// Templates you wrote (the built-in ones are in the app).
    #[serde(default)]
    templates: Vec<templates::Template>,
    #[serde(default)]
    settings: Settings,
    /// Fields written by a newer version of the app: kept untouched so saving never loses them.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Settings {
    /// Lock after this many idle minutes; 0 turns auto-lock off.
    auto_lock_minutes: u32,
    /// Daily writing goal in words; 0 means no goal.
    daily_word_goal: u32,
    /// Interface zoom, 1.0 = normal.
    ui_scale: f32,
    /// Hide the diary while its window is not the active one.
    privacy_screen: bool,
    /// Name of the template used for every new entry; empty for none.
    default_template: String,
    /// Show a writing prompt above an empty entry.
    show_prompts: bool,
    /// The AI helpers are switched on (they also need Ollama and a model).
    ai_enabled: bool,
    /// The Ollama model the AI helpers use.
    ai_model: String,
    /// How many automatic backups to keep; 0 turns them off.
    backup_keep: u32,
    /// Settings written by a newer version of the app (the phone's, for instance): kept untouched.
    #[serde(flatten, skip_serializing_if = "serde_json::Map::is_empty")]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { auto_lock_minutes: 10, daily_word_goal: 0, ui_scale: 1.0, privacy_screen: true, default_template: String::new(), show_prompts: true, ai_enabled: false, ai_model: String::new(), backup_keep: backups::DEFAULT_KEEP, extra: Default::default() }
    }
}

fn default_categories() -> Vec<Category> {
    theme::CATEGORY_COLOURS.into_iter().map(|(n, c)| Category { name: n.into(), color: c, extra: Default::default() }).collect()
}


/// Record ids and the data-model version (see `tideline_core::schema`). The desktop keeps `uid`,
/// `schema` and `diary_id` in each type's `extra` map, so the fields travel with the data untouched.
fn uid_missing(extra: &serde_json::Map<String, serde_json::Value>) -> bool {
    !extra.get("uid").and_then(|u| u.as_str()).is_some_and(tideline_core::ids::is_uid)
}

fn set_uid(extra: &mut serde_json::Map<String, serde_json::Value>, uid: String) {
    extra.insert("uid".into(), uid.into());
}

/// Is this diary still in the older format (no `schema`, or below the current one)?
fn needs_upgrade(data: &Data) -> bool {
    data.extra.get("schema").and_then(|s| s.as_u64()).unwrap_or(2) < tideline_core::schema::SCHEMA
}

/// Refuse a diary saved by a newer program rather than risk damaging it.
fn check_schema(data: &Data) -> Result<(), String> {
    match data.extra.get("schema").and_then(|s| s.as_u64()).unwrap_or(2) {
        s if s > tideline_core::schema::SCHEMA => Err("This diary was saved by a newer version of Tideline Journal. Update the app to open it.".into()),
        _ => Ok(()),
    }
}

/// Give every entry, category and template a `uid`: one worked out from its old identity when the
/// diary is first upgraded (so every copy of the diary agrees), a random one for anything new.
/// Returns true if anything changed. The result matches `tideline_core::schema::upgrade`.
fn assign_ids(data: &mut Data, vault: &vault::Vault) -> bool {
    use tideline_core::{ids, schema};
    let upgrading = needs_upgrade(data);
    let diary = schema::diary_id(vault);
    let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let mut changed = upgrading;
    let mut give = |extra: &mut serde_json::Map<String, serde_json::Value>, kind: &str, old: String, entry_id: Option<u64>| {
        if uid_missing(extra) {
            set_uid(extra, if upgrading { ids::legacy_uid(&diary, kind, &old) } else { ids::new_uid(now) });
            changed = true;
        }
        if let (true, Some(id)) = (upgrading, entry_id) {
            extra.entry("legacy_id").or_insert(id.into());
        }
    };
    for c in &mut data.categories {
        give(&mut c.extra, schema::KIND_CATEGORY, c.name.clone(), None);
    }
    for e in &mut data.entries {
        give(&mut e.extra, schema::KIND_ENTRY, e.id.to_string(), Some(e.id));
    }
    for t in &mut data.trash {
        give(&mut t.entry.extra, schema::KIND_ENTRY, t.entry.id.to_string(), Some(t.entry.id));
    }
    for t in &mut data.templates {
        give(&mut t.extra, schema::KIND_TEMPLATE, t.name.clone(), None);
    }
    if upgrading {
        data.extra.insert("diary_id".into(), diary.into());
        data.extra.insert("schema".into(), schema::SCHEMA.into());
    }
    changed
}

/// Copy the diary file as it is on disk to `before-upgrade-<time>.json` beside it.
fn back_up_before_upgrade(path: &std::path::Path) -> Result<(), String> {
    let folder = path.parent().unwrap_or(std::path::Path::new("."));
    let name = format!("before-upgrade-{}.json", Local::now().format("%Y%m%d-%H%M%S"));
    std::fs::copy(path, folder.join(name)).map(|_| ()).map_err(|e| format!("Couldn't back up the diary before upgrading it: {e}"))
}

fn fresh_data() -> Data {
    Data { categories: default_categories(), entries: Vec::new(), images: BTreeMap::new(), files: BTreeMap::new(), trash: Vec::new(), templates: Vec::new(), settings: Settings::default(), extra: Default::default() }
}

/// What is currently on disk.
enum Disk {
    Missing,
    Encrypted(vault::Envelope),
    /// Unencrypted data written by an earlier version of the app.
    Plain(Data),
    Unreadable(String),
}

fn inspect() -> Disk {
    let raw = match std::fs::read_to_string(data_path()) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Disk::Missing,
        Err(e) => return Disk::Unreadable(e.to_string()),
    };
    if raw.trim().is_empty() {
        return Disk::Missing;
    }
    if let Some(env) = vault::Envelope::parse(&raw) {
        return Disk::Encrypted(env);
    }
    if let Ok(data) = serde_json::from_str::<Data>(&raw) {
        return Disk::Plain(data);
    }
    // Oldest format: one text per date, e.g. {"2026-10-08": "text"}.
    if let Ok(old) = serde_json::from_str::<BTreeMap<String, String>>(&raw) {
        let mut data = fresh_data();
        let category = data.categories[0].name.clone();
        data.entries = old
            .into_iter()
            .enumerate()
            .map(|(i, (date, text))| Entry {
                id: i as u64 + 1,
                added_at: format!("{date} 00:00:00"),
                date,
                category: category.clone(),
                text,
                mood: None,
                ..Default::default()
            })
            .collect();
        return Disk::Plain(data);
    }
    Disk::Unreadable("it is not a diary file this app recognises".into())
}

/// Encrypt and write the diary. The result is decrypted again and checked before the old
/// file is replaced, and the replacement is atomic, so a failure can't destroy your data.
fn save(data: &Data, vault: &vault::Vault) -> Result<(), String> {
    write_diary(&data_path(), data, vault)
}

/// The same, to a given file. The folder is created if it isn't there yet (the first save of a
/// new diary, or of a portable copy that has just been unzipped).
fn write_diary(path: &std::path::Path, data: &Data, vault: &vault::Vault) -> Result<(), String> {
    let json = serde_json::to_vec(data).map_err(|e| e.to_string())?;
    let sealed = vault.seal(&json)?;
    let env = vault::Envelope::parse(&sealed).ok_or("could not re-read the encrypted data")?;
    if vault.open(&env)? != json {
        return Err("encryption check failed".into());
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("couldn't create the folder {}: {e}", folder.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, sealed).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn key(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}


struct EditState {
    /// `Some` when editing an existing entry.
    id: Option<u64>,
    date: NaiveDate,
    hour: u32,
    minute: u32,
    /// Seconds are kept from the original timestamp so entries stay in order.
    second: u32,
    category: String,
    /// The title (a first-line heading in the saved text) and the rest of the entry.
    title: String,
    text: String,
    /// The entry as it was when opened, to tell whether anything changed.
    baseline: String,
    /// "Discard" was clicked once; the next click really discards.
    discard_arm: bool,
    mood: Option<u8>,
    pinned: bool,
    sensitive: bool,
    attachments: Vec<Attachment>,
    ai: bool,
    /// Index of the text block that last had the cursor (target of toolbar actions).
    focus: usize,
    /// Steps through the writing prompts ("Another").
    prompt_offset: usize,
    /// Move the cursor here next frame: (text block index, char position); `usize::MAX` = last / end.
    want_focus: Option<(usize, usize)>,
}

#[derive(Clone)]
enum LockMode {
    /// No diary file yet: choose a password.
    Create,
    /// Encrypted diary: enter the password.
    Unlock(vault::Envelope),
    /// Forgot the password: unlock with the recovery key and choose a new password.
    Recover(vault::Envelope),
    /// Unencrypted diary from an earlier version: choose a password to encrypt it.
    Encrypt(Data),
    /// Join a diary that is already synced from another device: server, sync code and the diary's password.
    Join,
    /// The file exists but can't be used; never overwrite it.
    Broken(String),
}

struct Lock {
    mode: LockMode,
    password: String,
    confirm: String,
    error: String,
    focus: bool,
    recovery_key: String,
    /// For joining a synced diary: where the server is and the sync code.
    server: String,
    code: String,
}

impl Lock {
    fn new(mode: LockMode) -> Self {
        Self { mode, password: String::new(), confirm: String::new(), error: String::new(), focus: true, recovery_key: String::new(), server: String::new(), code: String::new() }
    }
}

const MIN_PASSWORD: usize = 8;

fn check_new_password(password: &str, confirm: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD {
        Err(format!("Use at least {MIN_PASSWORD} characters."))
    } else if password != confirm {
        Err("The two passwords don't match.".into())
    } else {
        Ok(())
    }
}

struct DiaryApp {
    /// Syncing with the other devices (see `syncing.rs`).
    sync: syncing::Runtime,
    /// Empty placeholder while the diary is locked.
    data: Data,
    /// Present while unlocked; used to encrypt on every save.
    vault: Option<vault::Vault>,
    /// `Some` while the lock screen is showing.
    lock: Option<Lock>,
    selected: NaiveDate,
    /// First day of the month being displayed.
    month: NaiveDate,
    editor: Option<EditState>,
    show_categories: bool,
    new_cat_name: String,
    new_cat_color: [u8; 3],
    /// Category row being renamed and its in-progress text.
    rename_buf: Option<(usize, String)>,
    show_password: bool,
    show_location: bool,
    show_shortcuts: bool,
    show_privacy: bool,
    show_export: bool,
    export_scope: ExportScope,
    export_images: bool,
    export_private: bool,
    export_msg: String,
    export_path: Option<std::path::PathBuf>,
    /// Ctrl+S was pressed while writing an entry.
    save_requested: bool,
    location_msg: String,
    pw_new: String,
    pw_confirm: String,
    pw_error: String,
    status: String,
    media: markdown::Media,
    show_marks: bool,
    /// How the window opens (kept in the small settings file, not the encrypted diary).
    start_mode: location::StartMode,
    /// The start mode has been applied (it is done once, on the first frame).
    start_applied: bool,
    ai: ai::AiState,
    templates: templates::TemplateState,
    show_recovery: bool,
    /// The recovery key just made; shown once, then forgotten.
    recovery_shown: Option<String>,
    recovery_confirmed: bool,
    recovery_msg: String,
    /// 1 = "make a new key" clicked once, 2 = "remove" clicked once (needs a second click).
    recovery_arm: u8,
    recovery_hint_dismissed: bool,
    show_trash: bool,
    show_backups: bool,
    backup_items: Vec<backups::Backup>,
    backups_dirty: bool,
    backup_restore_arm: Option<std::path::PathBuf>,
    backup_msg: String,
    /// The day the automatic backup was last checked (YYYY-MM-DD).
    backup_day: String,
    trash_arm: bool,
    /// The entry most recently moved to the bin, for the Undo button.
    last_trashed: Option<u64>,
    insights: insights::InsightsState,
    photos_cache: Option<(u64, Vec<gallery::Photo>)>,
    thumb_size: f32,
    lightbox: Option<usize>,
    /// Private entries the user has clicked open (forgotten on lock).
    revealed: std::collections::HashSet<u64>,
    show_pinned: bool,
    /// The Settings page is open; the section lit up in its left column; a section to scroll to.
    show_settings: bool,
    settings_section: usize,
    settings_jump: Option<usize>,
    /// Entry cards in the day panel that are showing the whole entry.
    expanded: std::collections::HashSet<u64>,
    /// The Pinned page: the category filter (None = all) and the sort order (index into `pinned::SORTS`).
    pinned_category: Option<String>,
    pinned_sort: usize,
    view: CalView,
    /// Bumped whenever the diary changes; invalidates `stats_cache`.
    rev: u64,
    stats_cache: Option<(u64, BTreeMap<NaiveDate, stats::DayStat>)>,
    /// Entry being dragged on the calendar.
    drag: Option<u64>,
    /// Scroll the day view to the working hours on its next frame.
    day_scroll_pending: bool,
    search: String,
    search_state: search::SearchState,
    last_activity: std::time::Instant,
    /// Images used by an unsaved draft, kept while the diary is locked.
    draft_images: BTreeMap<String, String>,
    /// Message shown in the entry editor (e.g. an image that failed to load).
    editor_msg: String,
}

impl DiaryApp {
    fn new() -> Self {
        let mut app = Self::new_inner();
        if demo::requested() {
            // Look-around mode: unlocked, made-up entries, nothing real is read or written.
            app.lock = None;
            app.vault = vault::Vault::create("demo").ok();
            app.data = demo::data();
            app.recovery_hint_dismissed = true;
            app.selected = NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();
            app.month = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
            demo::apply(&mut app);
        }
        app
    }

    fn new_inner() -> Self {
clear_open_files(); // leftovers from opening attachments last time
        let today = Local::now().date_naive();
        let mode = current_lock_mode();
        Self {
            sync: syncing::Runtime::default(),
            data: fresh_data(),
            vault: None,
            lock: Some(Lock::new(mode)),
            selected: today,
            month: today.with_day(1).unwrap(),
            editor: None,
            show_categories: false,
            new_cat_name: String::new(),
            new_cat_color: [59, 130, 246],
            rename_buf: None,
            show_password: false,
            show_location: false,
            show_shortcuts: false,
            show_privacy: false,
            show_export: false,
            export_scope: ExportScope::Month,
            export_images: true,
            export_private: false,
            export_msg: String::new(),
            export_path: None,
            save_requested: false,
            location_msg: String::new(),
            pw_new: String::new(),
            pw_confirm: String::new(),
            pw_error: String::new(),
            status: String::new(),
            media: markdown::Media::default(),
            show_marks: false,
            start_mode: location::start_mode(),
            start_applied: false,
            ai: ai::AiState::default(),
            templates: templates::TemplateState::default(),
            show_recovery: false,
            recovery_shown: None,
            recovery_confirmed: false,
            recovery_msg: String::new(),
            recovery_arm: 0,
            recovery_hint_dismissed: false,
            show_trash: false,
            show_backups: false,
            backup_items: Vec::new(),
            backups_dirty: true,
            backup_restore_arm: None,
            backup_msg: String::new(),
            backup_day: String::new(),
            trash_arm: false,
            last_trashed: None,
            insights: insights::InsightsState::default(),
            photos_cache: None,
            thumb_size: 150.0,
            lightbox: None,
            revealed: std::collections::HashSet::new(),
            show_pinned: false,
            show_settings: false,
            settings_section: 0,
            settings_jump: None,
            expanded: std::collections::HashSet::new(),
            pinned_category: None,
            pinned_sort: 0,
            view: CalView::Month,
            rev: 0,
            stats_cache: None,
            drag: None,
            day_scroll_pending: true,
            search: String::new(),
            search_state: search::SearchState::new(),
            last_activity: std::time::Instant::now(),
            draft_images: BTreeMap::new(),
            editor_msg: String::new(),
        }
    }

    fn persist(&mut self) {
        self.rev += 1;
        self.purge_trash();
        self.drop_unused_images();
        if let Some(v) = &self.vault {
            assign_ids(&mut self.data, v);
        }
        self.sync_stamp();
        self.status = match &self.vault {
            Some(v) => match save(&self.data, v) {
                Ok(()) => "Saved".into(),
                Err(e) => format!("Save failed: {e}"),
            },
            None => "Save failed: the diary is locked".into(),
        };
    }

    /// Remove stored images that no entry (or open draft) refers to any more.
    fn drop_unused_images(&mut self) {
        let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
        for e in self.data.entries.iter().chain(self.data.trash.iter().map(|t| &t.entry)) {
            used.extend(markdown::image_ids(&e.text));
        }
        if let Some(ed) = &self.editor {
            used.extend(markdown::image_ids(&ed.text));
        }
        self.data.images.retain(|id, _| used.contains(id));
        // Attached files nobody refers to any more (including by an open draft) are dropped too.
        let mut files: std::collections::HashSet<String> = self.data.entries.iter().chain(self.data.trash.iter().map(|t| &t.entry)).flat_map(|e| e.attachments.iter().map(|a| a.id.clone())).collect();
        if let Some(ed) = &self.editor {
            files.extend(ed.attachments.iter().map(|a| a.id.clone()));
        }
        self.data.files.retain(|id, _| files.contains(id));
    }

    /// Try to unlock / create / encrypt / recover using what was typed on the lock screen.
    fn submit_lock(&mut self) {
        let Some(lock) = self.lock.as_mut() else { return };
        let password = lock.password.clone();
        let new_diary = matches!(lock.mode, LockMode::Create | LockMode::Encrypt(_));
        let result: Result<(vault::Vault, Data), String> = match lock.mode.clone() {
            LockMode::Unlock(env) => vault::Vault::unlock(&env, &password).and_then(|(v, plain)| {
                let mut data = serde_json::from_slice::<Data>(&plain).map_err(|e| e.to_string())?;
                check_schema(&data)?;
                if needs_upgrade(&data) {
                    // The older file is kept beside the diary before the upgraded one replaces it.
                    back_up_before_upgrade(&data_path())?;
                    assign_ids(&mut data, &v);
                    save(&data, &v)?;
                } else if env.is_legacy() {
                    // Write the diary in the newer format (the one that supports a recovery key) now.
                    save(&data, &v).ok();
                }
                Ok((v, data))
            }),
            LockMode::Recover(env) => check_new_password(&password, &lock.confirm).and_then(|()| {
                let (mut v, plain) = vault::Vault::unlock_with_recovery(&env, &lock.recovery_key)?;
                let mut data = serde_json::from_slice::<Data>(&plain).map_err(|e| e.to_string())?;
                check_schema(&data)?;
                if needs_upgrade(&data) {
                    back_up_before_upgrade(&data_path())?;
                }
                assign_ids(&mut data, &v);
                v.set_password(&password)?;
                save(&data, &v)?;
                Ok((v, data))
            }),
            LockMode::Join => joining::join_diary(&data_path(), &lock.server, &lock.code, &password),
            LockMode::Create => create_vault(&password, &lock.confirm, fresh_data()),
            LockMode::Encrypt(data) => create_vault(&password, &lock.confirm, data),
            LockMode::Broken(_) => return,
        };
        match result {
            Ok((v, data)) => {
                self.vault = Some(v);
                self.data = data;
                self.rev += 1;
                self.sync_unlocked(&data_path());
                self.data.images.extend(std::mem::take(&mut self.draft_images));
                self.last_activity = std::time::Instant::now();
                self.lock = None;
                self.status.clear();
                // A brand-new diary is the moment to offer a recovery key.
                self.show_recovery = new_diary;
                self.purge_trash();
            }
            Err(e) => {
                lock.error = e;
                lock.password.clear();
                lock.confirm.clear();
                lock.focus = true;
            }
        }
    }

    /// Forget the key and the decrypted data and go back to the lock screen.
    fn lock_now(&mut self) {
        self.sync_locked();
        let mode = current_lock_mode();
        // An open draft survives locking (it only exists in memory), along with its images.
        self.draft_images = match &self.editor {
            Some(ed) => markdown::image_ids(&ed.text)
                .into_iter()
                .filter_map(|id| self.data.images.get(&id).map(|b| (id, b.clone())))
                .collect(),
            None => BTreeMap::new(),
        };
        self.data = fresh_data();
        self.vault = None;
        self.media = markdown::Media::default();
        self.revealed.clear();
        clear_open_files();
        self.stats_cache = None;
        self.ai.reset();
        self.photos_cache = None;
        self.lightbox = None;
        self.insights.clear();
        self.search.clear();
        self.search_state.reset();
        self.show_categories = false;
        self.show_password = false;
        self.show_location = false;
        self.show_export = false;
        self.show_pinned = false;
        self.expanded.clear();
        self.show_settings = false;
        self.show_recovery = false;
        self.recovery_shown = None;
        self.show_trash = false;
        self.show_backups = false;
        self.backup_restore_arm = None;
        self.last_trashed = None;
        self.show_shortcuts = false;
        self.show_privacy = false;
        self.lock = Some(Lock::new(mode));
    }

    fn lock_screen(&mut self, ui: &mut egui::Ui) {
        let p = palette(ui.ctx());
        let dark = ui.ctx().theme() == egui::Theme::Dark;
        let Some(lock) = self.lock.as_mut() else { return };
        let (title, blurb, button) = match &lock.mode {
            LockMode::Create => (
                "Welcome to Tideline Journal",
                "Choose a password to protect your diary. You can also make a recovery key in case you forget it.".to_string(),
                "Create password",
            ),
            LockMode::Encrypt(_) => (
                "Protect your diary",
                "Your diary isn't encrypted yet. Choose a password to encrypt it. You can also make a recovery key in case you forget it."
                    .to_string(),
                "Encrypt diary",
            ),
            LockMode::Unlock(_) => ("Tideline Journal is locked", "Enter your password to open it.".to_string(), "Unlock"),
            LockMode::Recover(env) if env.has_recovery() => (
                "Recover your diary",
                "Enter the recovery key you saved, then choose a new password.".to_string(),
                "Unlock and set the new password",
            ),
            LockMode::Recover(_) => (
                "No recovery key",
                "This diary has no recovery key, so a forgotten password can't be recovered by anyone. That is what keeps it private.\n\nIf you have a copy of the diary whose password you do remember, you can open that instead."
                    .to_string(),
                "",
            ),
            LockMode::Join => (
                "Join a synced diary",
                "Use a diary that is already synced from another device. Enter your sync server and its sync code (on the other device, choose Copy details), then the diary's password.".to_string(),
                "Join",
            ),
            LockMode::Broken(why) => (
                "Can't open the diary file",
                format!("{}\n\nThe file was left untouched: {why}.\nFix or move it, then restart the app.", data_path().display()),
                "",
            ),
        };
        let recovering = matches!(&lock.mode, LockMode::Recover(env) if env.has_recovery());
        let needs_confirm = recovering || matches!(lock.mode, LockMode::Create | LockMode::Encrypt(_));
        let joining = matches!(lock.mode, LockMode::Join);
        let can_join = matches!(lock.mode, LockMode::Create);
        let no_fields = matches!(lock.mode, LockMode::Broken(_)) || matches!(&lock.mode, LockMode::Recover(env) if !env.has_recovery());
        let in_recover = matches!(lock.mode, LockMode::Recover(_));
        let can_switch = matches!(lock.mode, LockMode::Unlock(_) | LockMode::Recover(_));
        let (mut submit, mut forgot, mut back, mut other) = (false, false, false, false);
        let (mut go_join, mut leave_join) = (false, false);

        ui.vertical_centered(|ui| {
            ui.add_space(((ui.available_height() - 520.0) / 2.0).max(16.0));
            egui::Frame::new()
                .fill(p.panel_bg)
                .stroke(Stroke::new(1.0, p.cell_border))
                .shadow(popup_shadow(dark))
                .corner_radius(14)
                .inner_margin(34)
                .show(ui, |ui| {
                    ui.set_width(400.0);
                    let (logo, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::hover());
                    let tile = egui::Rect::from_center_size(logo.center(), vec2(60.0, 60.0));
                    ui.painter().rect_filled(tile, 16.0, Color32::from_rgb(0x15, 0x30, 0x2D));
                    kit::paint_logo(ui.painter(), tile.center(), 44.0, p.accent);
                    ui.add_space(10.0);
                    ui.label(RichText::new(title).font(FontId::new(32.0, fonts::serif_medium())).color(p.title));
                    ui.label(RichText::new("Every day leaves a mark.").font(FontId::new(16.0, fonts::serif_italic())).color(p.faint));
                    ui.add_space(4.0);
                    ui.add_space(6.0);
                    ui.label(RichText::new(blurb).size(14.0).color(p.muted));
                    ui.add_space(14.0);

                    if !no_fields {
                        if joining {
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut lock.server)
                                    .hint_text("Server address, for example 192.168.1.20:8787")
                                    .font(FontId::proportional(16.0))
                                    .margin(egui::Margin::symmetric(12, 8))
                                    .text_color(p.ink)
                                    .desired_width(f32::INFINITY),
                            );
                            if lock.focus {
                                r.request_focus();
                                lock.focus = false;
                            }
                            ui.add_space(8.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut lock.code)
                                    .hint_text("Sync code")
                                    .password(true)
                                    .font(FontId::proportional(16.0))
                                    .margin(egui::Margin::symmetric(12, 8))
                                    .text_color(p.ink)
                                    .desired_width(f32::INFINITY),
                            );
                            ui.add_space(8.0);
                        }
                        if recovering {
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut lock.recovery_key)
                                    .hint_text("Recovery key (XXXX-XXXX-XXXX-…)")
                                    .font(FontId::monospace(16.0))
                                    .margin(egui::Margin::symmetric(12, 8))
                                    .text_color(p.ink)
                                    .desired_width(f32::INFINITY),
                            );
                            if lock.focus {
                                r.request_focus();
                                lock.focus = false;
                            }
                            ui.add_space(8.0);
                        }
                        let hint = if recovering { "New password" } else if joining { "The diary's password" } else { "Password" };
                        let r = ui.add(password_field(&mut lock.password, hint, &p));
                        if lock.focus && !recovering && !joining {
                            r.request_focus();
                            lock.focus = false;
                        }
                        submit |= pressed_enter(&r, ui) && !needs_confirm;
                        if needs_confirm {
                            ui.add_space(8.0);
                            let r2 = ui.add(password_field(&mut lock.confirm, if recovering { "Confirm new password" } else { "Confirm password" }, &p));
                            submit |= pressed_enter(&r2, ui);
                        }
                        ui.add_space(14.0);
                        let width = ui.available_width();
                        if kit::button_h(ui, kit::Look::Primary, None, button, Some(width), 44.0).clicked() {
                            submit = true;
                        }
                    }
                    if !lock.error.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new(&lock.error).color(RED));
                    }
                    if can_join || joining {
                        ui.add_space(12.0);
                        if joining {
                            leave_join = ui.link("Back").clicked();
                        } else {
                            go_join = ui.link("Join a synced diary…").clicked();
                        }
                    }
                    if can_switch {
                        ui.add_space(12.0);
                        ui.horizontal_wrapped(|ui| {
                            if in_recover {
                                back = ui.link("Back to the password screen").clicked();
                            } else {
                                forgot = ui.link("Forgot your password?").clicked();
                            }
                            other = ui.link("Open a different diary…").clicked();
                        });
                    }
                });
        });

        if go_join {
            self.lock = Some(Lock::new(LockMode::Join));
        } else if leave_join {
            self.lock = Some(Lock::new(current_lock_mode()));
        } else if forgot {
            if let LockMode::Unlock(env) = lock.mode.clone() {
                self.lock = Some(Lock::new(LockMode::Recover(env)));
            }
        } else if back {
            if let LockMode::Recover(env) = lock.mode.clone() {
                self.lock = Some(Lock::new(LockMode::Unlock(env)));
            }
        } else if other {
            if let Some(dir) = rfd::FileDialog::new().set_directory(location::data_dir()).pick_folder() {
                match location::use_existing(dir) {
                    Ok(()) => self.lock = Some(Lock::new(current_lock_mode())),
                    Err(e) => lock.error = e,
                }
            }
        }
        if submit {
            self.submit_lock();
        }
    }

    fn password_window(&mut self, ctx: &egui::Context) {
        if !self.show_password {
            return;
        }
        let p = palette(ctx);
        let dark = ctx.theme() == egui::Theme::Dark;
        let mut close = false;
        let mut apply = false;

        egui::Window::new("Change password")
            .title_bar(false)
            .frame(popup_frame(&p, dark))
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                close |= popup_header(ui, &p, "Change password");
                egui::Frame::new().inner_margin(14).show(ui, |ui| {
                    ui.set_width(340.0);
                    ui.add(password_field(&mut self.pw_new, "New password", &p));
                    ui.add_space(8.0);
                    let r = ui.add(password_field(&mut self.pw_confirm, "Confirm new password", &p));
                    apply |= pressed_enter(&r, ui);
                    ui.add_space(10.0);
                    if ui.button("Change password").clicked() {
                        apply = true;
                    }
                    if !self.pw_error.is_empty() {
                        ui.add_space(6.0);
                        ui.label(RichText::new(&self.pw_error).color(Color32::from_rgb(239, 68, 68)));
                    }
                });
            });

        if apply {
            match create_vault(&self.pw_new, &self.pw_confirm, self.data.clone()) {
                Ok((v, _)) => {
                    self.vault = Some(v);
                    self.status = "Password changed".into();
                    close = true;
                }
                Err(e) => self.pw_error = e,
            }
        }
        if close {
            self.show_password = false;
            self.pw_new.clear();
            self.pw_confirm.clear();
            self.pw_error.clear();
        }
    }


    fn location_window(&mut self, ctx: &egui::Context) {
        if !self.show_location {
            return;
        }
        let p = palette(ctx);
        let dark = ctx.theme() == egui::Theme::Dark;
        let mut close = false;
        let (mut reveal, mut move_it, mut use_other) = (false, false, false);

        egui::Window::new("Data location")
            .title_bar(false)
            .frame(popup_frame(&p, dark))
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                close |= popup_header(ui, &p, "Data location");
                egui::Frame::new().inner_margin(14).show(ui, |ui| {
                    ui.set_width(480.0);
                    ui.label("Your diary (always encrypted) is stored in this folder:");
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(RichText::new(location::data_dir().display().to_string()).monospace().color(p.ink)).wrap());
                    ui.add_space(6.0);
                    reveal = ui.button("Show in Explorer").clicked();
                    ui.separator();
                    ui.label(RichText::new("Move it somewhere else").strong().color(p.title));
                    ui.label(RichText::new("Copies the diary to a folder you choose and uses it from there.").color(p.muted));
                    move_it = ui.button("Move diary to another folder…").clicked();
                    ui.add_space(8.0);
                    ui.label(RichText::new("Open a different diary").strong().color(p.title));
                    ui.label(RichText::new("Use a diary that already exists in another folder, such as a backup. You will be asked for its password.").color(p.muted));
                    use_other = ui.button("Use a diary from another folder…").clicked();
                    if !self.location_msg.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new(&self.location_msg).color(p.ink));
                    }
                });
            });

        if reveal {
            std::process::Command::new("explorer").arg(location::data_dir()).spawn().ok();
        }
        if move_it {
            if let Some(dir) = rfd::FileDialog::new().set_directory(location::data_dir()).pick_folder() {
                self.location_msg = match location::move_to(dir) {
                    Ok(()) => format!("Moved. The diary is now in {}", location::data_dir().display()),
                    Err(e) => e,
                };
            }
        }
        if use_other {
            if let Some(dir) = rfd::FileDialog::new().set_directory(location::data_dir()).pick_folder() {
                match location::use_existing(dir) {
                    Ok(()) => {
                        // Back to the lock screen, now pointing at the other diary.
                        self.editor = None;
                        self.location_msg.clear();
                        self.show_location = false;
                        self.lock_now();
                    }
                    Err(e) => self.location_msg = e,
                }
            }
        }
        if close {
            self.show_location = false;
            self.location_msg.clear();
        }
    }

    fn day_entries(&self, d: NaiveDate) -> Vec<&Entry> {
        let k = key(d);
        let mut v: Vec<&Entry> = self.data.entries.iter().filter(|e| e.date == k).collect();
        v.sort_by(|a, b| a.added_at.cmp(&b.added_at));
        v
    }

    fn shift_month(&mut self, delta: i32) {
        let total = self.month.year() * 12 + self.month.month0() as i32 + delta;
        self.month = NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap();
    }

    fn new_entry(&mut self, date: NaiveDate) {
        self.new_entry_at(date, None);
    }

    /// Start a new entry; `hour` picks the time (on the hour), otherwise it is now.
    fn new_entry_at(&mut self, date: NaiveDate, hour: Option<u32>) {
        let category = self.data.categories.first().map(|c| c.name.clone()).unwrap_or_default();
        let now = Local::now();
        let (hour, minute, second) = match hour {
            Some(h) => (h, 0, 0),
            None => (now.hour(), now.minute(), now.second()),
        };
        self.editor = Some(EditState {
            id: None,
            date,
            hour,
            minute,
            second,
            category,
            title: String::new(),
            text: String::new(),
            baseline: String::new(),
            discard_arm: false,
            mood: None,
            pinned: false,
            sensitive: false,
            attachments: Vec::new(),
            ai: false,
            focus: 0,
            prompt_offset: 0,
            want_focus: Some((usize::MAX, usize::MAX)),
        });
        self.editor_msg.clear();
        // A template chosen for every new entry (Settings → Templates) is filled in right away.
        let default = self.data.settings.default_template.clone();
        if !default.is_empty() {
            self.apply_template(&default);
        }
        if let Some(ed) = self.editor.as_mut() {
            ed.baseline = ed.full_text();
        }
    }

    /// Step the calendar one month / week / day / year.
    fn step_calendar(&mut self, dir: i64) {
        match self.view {
            CalView::Month => self.shift_month(dir as i32),
            CalView::Week => self.shift_days(7 * dir),
            CalView::Day => self.shift_days(dir),
            CalView::Year => self.shift_year(dir as i32),
            CalView::Photos => {}
        }
    }

    /// Make the interface bigger (`+1`) or smaller (`-1`) by one step and remember it.
    fn step_zoom(&mut self, dir: i32) {
        const STEPS: [f32; 7] = [0.8, 0.9, 1.0, 1.1, 1.25, 1.4, 1.6];
        let now = self.data.settings.ui_scale;
        let nearest = STEPS.iter().enumerate().min_by(|a, b| (a.1 - now).abs().total_cmp(&(b.1 - now).abs())).map_or(2, |(i, _)| i);
        let next = (nearest as i32 + dir).clamp(0, STEPS.len() as i32 - 1) as usize;
        if (STEPS[next] - now).abs() > 0.001 {
            self.data.settings.ui_scale = STEPS[next];
            self.persist();
        }
    }

    /// Jump to a random entry from the past, as a way of looking back. Private entries are skipped.
    fn random_entry(&mut self) {
        let today = key(Local::now().date_naive());
        let older: Vec<&Entry> = self.data.entries.iter().filter(|e| !e.sensitive && e.date < today).collect();
        let pool: Vec<&Entry> = if older.is_empty() { self.data.entries.iter().filter(|e| !e.sensitive).collect() } else { older };
        if pool.is_empty() {
            self.status = "There are no entries to look back on yet.".into();
            return;
        }
        let pick = pool[getrandom::u64().unwrap_or(0) as usize % pool.len()];
        if let Ok(d) = NaiveDate::parse_from_str(&pick.date, "%Y-%m-%d") {
            self.selected = d;
            self.month = d.with_day(1).unwrap();
            self.day_scroll_pending = true;
            self.search.clear();
            self.search_state.reset();
            self.status = format!("A look back: {}", d.format("%A %e %B %Y"));
        }
    }

    /// Switch the colour theme now and remember it for next time (the lock screen uses it too).
    fn set_colour_theme(&mut self, ctx: &egui::Context, accent: [u8; 3]) {
        if !theme::set_accent(accent) {
            return;
        }
        apply_theme(ctx);
        let saved = if accent == theme::DEFAULT { None } else { Some(accent) };
        if let Err(e) = location::set_accent(saved) {
            self.status = format!("Save failed: couldn't remember the colour theme ({e})");
        }
    }

    /// Open maximised or full screen, as chosen in Settings. This runs once, on the first frame,
    /// because asking for it when the window is created gets overridden by the initial size.
    fn apply_start_mode(&mut self, ctx: &egui::Context) {
        if std::mem::replace(&mut self.start_applied, true) {
            return;
        }
        match self.start_mode {
            location::StartMode::Normal => {}
            location::StartMode::Maximized => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true)),
            location::StartMode::Fullscreen => ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true)),
        }
    }

    fn go_today(&mut self) {
        let t = Local::now().date_naive();
        self.selected = t;
        self.month = t.with_day(1).unwrap();
        self.day_scroll_pending = true;
        self.search.clear();
        self.search_state.reset();
    }

    /// Keyboard shortcuts (listed in the "Keyboard shortcuts" window).
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        // F11 switches full screen on and off, even on the lock screen.
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F11)) {
            toggle_fullscreen(ctx);
        }
        if self.lock.is_some() {
            return;
        }
        let cmd = |key: Key| ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, key));
        // Text size (zoom).
        if cmd(Key::Equals) || cmd(Key::Plus) {
            self.step_zoom(1);
        }
        if cmd(Key::Minus) {
            self.step_zoom(-1);
        }
        if cmd(Key::Num0) {
            self.data.settings.ui_scale = 1.0;
            self.persist();
        }
        let editing = self.editor.is_some();

        if cmd(Key::L) {
            self.lock_now();
            return;
        }
        if editing {
            // Ctrl+S saves the entry being written.
            if cmd(Key::S) || cmd(Key::Enter) {
                self.save_requested = true;
            }
            return;
        }
        if cmd(Key::N) {
            self.new_entry(self.selected);
        }
        if cmd(Key::F) {
            ctx.memory_mut(|m| m.request_focus(search_id()));
        }
        if cmd(Key::E) {
            self.show_export = true;
        }
        if cmd(Key::R) {
            self.random_entry();
        }
        if cmd(Key::T) {
            self.go_today();
        }
        for (key, view) in [(Key::Num1, CalView::Day), (Key::Num2, CalView::Week), (Key::Num3, CalView::Month), (Key::Num4, CalView::Year), (Key::Num5, CalView::Photos)] {
            if cmd(key) {
                self.view = view;
                self.day_scroll_pending = true;
            }
        }
        // Arrow keys move through time, unless a text box wants them.
        if !ctx.egui_wants_keyboard_input() && self.lightbox.is_none() && self.view != CalView::Photos {
            if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
                self.step_calendar(-1);
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
                self.step_calendar(1);
            }
        }
    }

    /// First day, last day, a description and a file-name part for an export scope.
    fn export_range(&self, scope: ExportScope) -> (NaiveDate, NaiveDate, String, String) {
        let d = self.selected;
        match scope {
            ExportScope::Day => (d, d, d.format("%A %e %B %Y").to_string(), key(d)),
            ExportScope::Week => {
                let monday = week_start(d);
                (monday, monday + chrono::Days::new(6), week_title(d), format!("week-{}", key(monday)))
            }
            ExportScope::Month => {
                let first = d.with_day(1).unwrap();
                let last = self.shift_target(first).pred_opt().unwrap();
                (first, last, d.format("%B %Y").to_string(), d.format("%Y-%m").to_string())
            }
            ExportScope::Year => {
                let (first, last) = (NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap(), NaiveDate::from_ymd_opt(d.year(), 12, 31).unwrap());
                (first, last, d.year().to_string(), d.year().to_string())
            }
            ExportScope::All => (NaiveDate::from_ymd_opt(1, 1, 1).unwrap(), NaiveDate::from_ymd_opt(9999, 12, 31).unwrap(), "Everything".into(), "all".into()),
        }
    }

    fn entries_in(&self, from: NaiveDate, to: NaiveDate) -> Vec<&Entry> {
        let (from, to) = (key(from), key(to));
        self.data.entries.iter().filter(|e| e.date >= from && e.date <= to && (self.export_private || !e.sensitive)).collect()
    }

    fn export_window(&mut self, ctx: &egui::Context) {
        if !self.show_export {
            return;
        }
        let p = palette(ctx);
        let dark = ctx.theme() == egui::Theme::Dark;
        let mut close = false;
        let mut create = false;
        let mut open_file = false;
        let scopes = [
            (ExportScope::Day, "This day"),
            (ExportScope::Week, "This week"),
            (ExportScope::Month, "This month"),
            (ExportScope::Year, "This year"),
            (ExportScope::All, "Everything"),
        ];
        let choices: Vec<(ExportScope, String)> = scopes
            .iter()
            .map(|(scope, name)| {
                let (from, to, what, _) = self.export_range(*scope);
                let n = self.entries_in(from, to).len();
                let what = if *scope == ExportScope::All { String::new() } else { format!(" – {what}") };
                (*scope, format!("{name}{what}  ({n} {})", if n == 1 { "entry" } else { "entries" }))
            })
            .collect();

        egui::Window::new("Export to PDF")
            .title_bar(false)
            .frame(popup_frame(&p, dark))
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                close |= popup_header(ui, &p, "Export to PDF");
                egui::Frame::new().inner_margin(14).show(ui, |ui| {
                    ui.set_width(460.0);
                    ui.label(RichText::new("What to include").strong().color(p.title));
                    ui.add_space(4.0);
                    for (scope, label) in &choices {
                        ui.radio_value(&mut self.export_scope, *scope, label);
                    }
                    ui.add_space(8.0);
                    ui.checkbox(&mut self.export_images, "Include pictures");
                    ui.checkbox(&mut self.export_private, "Include private entries (their text will be readable in the PDF)");
                    ui.add_space(6.0);
                    ui.label(RichText::new("The PDF is a plain copy of your entries. It is not encrypted or password protected, so keep it somewhere safe.").small().color(p.muted));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        create = ui.button("Create PDF…").clicked();
                        if self.export_path.is_some() {
                            open_file = ui.button("Open last PDF").clicked();
                        }
                    });
                    if !self.export_msg.is_empty() {
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(RichText::new(&self.export_msg).color(p.ink)).wrap());
                    }
                });
            });

        if create {
            let (from, to, what, slug) = self.export_range(self.export_scope);
            let entries = self.entries_in(from, to);
            let name = format!("diary-{slug}.pdf");
            let dest = rfd::FileDialog::new().set_file_name(name).add_filter("PDF document", &["pdf"]).save_file();
            if let Some(dest) = dest {
                let report = export::Report {
                    title: "Tideline Journal".into(),
                    subtitle: format!(
                        "{what} · {} {} · created {}",
                        entries.len(),
                        if entries.len() == 1 { "entry" } else { "entries" },
                        Local::now().format("%e %B %Y")
                    ),
                    entries,
                    include_images: self.export_images,
                };
                self.export_msg = match export::build_pdf(&report, &self.data.categories, &self.data.images)
                    .and_then(|bytes| std::fs::write(&dest, bytes).map_err(|e| format!("Couldn't save the PDF: {e}")))
                {
                    Ok(()) => {
                        self.export_path = Some(dest.clone());
                        format!("Saved {}", dest.display())
                    }
                    Err(e) => e,
                };
            }
        }
        if open_file {
            if let Some(path) = &self.export_path {
                std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn().ok();
            }
        }
        if close {
            self.show_export = false;
        }
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.show_shortcuts {
            return;
        }
        let p = palette(ctx);
        let dark = ctx.theme() == egui::Theme::Dark;
        let mut close = false;
        egui::Window::new("Keyboard shortcuts")
            .title_bar(false)
            .frame(popup_frame(&p, dark))
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                close |= popup_header(ui, &p, "Keyboard shortcuts");
                egui::Frame::new().inner_margin(14).show(ui, |ui| {
                    let rows: [(&str, &str); 18] = [
                        ("Ctrl+N", "New entry on the selected day"),
                        ("Ctrl+F", "Search entries"),
                        ("Ctrl+T", "Jump to today"),
                        ("Ctrl+L", "Lock the diary"),
                        ("Ctrl+E", "Export entries to PDF"),
                        ("Ctrl+R", "Jump to a random entry"),
                        ("F11", "Full screen on / off"),
                        ("Ctrl + / Ctrl -", "Bigger / smaller text (Ctrl+0 resets)"),
                        ("← / →", "Previous / next month, week, day or year"),
                        ("Ctrl+1 … 5", "Day, Week, Month, Year, Photos view"),
                        ("Double-click", "A day: new entry. An entry: edit it"),
                        ("Drag", "Move an entry to another day or hour"),
                        ("Esc", "Cancel a drag"),
                        ("Ctrl+S", "Save the entry (while writing)"),
                        ("Ctrl+D", "Pin or unpin the entry (while writing)"),
                        ("Ctrl+Enter", "Save the entry (while writing)"),
                        ("Ctrl+B / Ctrl+I", "Bold / italic (while writing)"),
                        ("Click again", "A mood button or toolbar format removes it"),
                    ];
                    egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 170.0).max(200.0)).show(ui, |ui| {
                    egui::Grid::new("shortcut_grid").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
                        for (keys, what) in rows {
                            ui.label(RichText::new(keys).strong().monospace().color(p.title));
                            ui.label(RichText::new(what).color(p.ink));
                            ui.end_row();
                        }
                    });
                    });
                });
            });
        if close {
            self.show_shortcuts = false;
        }
    }

    /// The window's title bar: logo, search box and window controls. It is also how the window
    /// is dragged (double-click to maximise).
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let rect = ui.max_rect();
        let bar = ui.interact(rect, ui.id().with("title_drag"), Sense::click_and_drag());
        if bar.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if bar.double_clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
        let painter = ui.painter().clone();
        let cy = rect.center().y;

        // Logo tile and name.
        let tile = egui::Rect::from_center_size(egui::pos2(rect.min.x + 30.0, cy), vec2(28.0, 28.0));
        painter.rect_filled(tile, 8.0, Color32::from_rgb(0x15, 0x30, 0x2D));
        kit::paint_logo(&painter, tile.center(), 22.0, theme::shades().accent);
        let name_font = FontId::new(23.0, fonts::serif_medium());
        let tideline = painter.layout_no_wrap("Tideline".into(), name_font.clone(), theme::TEXT);
        let name_pos = egui::pos2(rect.min.x + 52.0, cy - 1.0);
        painter.text(name_pos, Align2::LEFT_CENTER, "Tideline", name_font, theme::TEXT);
        painter.text(name_pos + vec2(tideline.size().x + 6.0, 0.0), Align2::LEFT_CENTER, "Journal", FontId::new(23.0, fonts::serif_italic()), theme::TEXT_2);

        // Window controls on the right.
        let mut x = rect.max.x;
        for (icon, tip, action) in [
            (kit::Icon::Close, "Close", 0),
            (if maximized { kit::Icon::Restore } else { kit::Icon::Maximize }, if maximized { "Restore" } else { "Maximise" }, 1),
            (kit::Icon::Minimize, "Minimise", 2),
        ] {
            let r = egui::Rect::from_min_max(egui::pos2(x - 46.0, rect.min.y), egui::pos2(x, rect.max.y));
            x -= 46.0;
            let resp = ui.interact(r, ui.id().with(("win_ctl", action)), Sense::click()).on_hover_text(tip);
            if resp.hovered() {
                painter.rect_filled(r, 0.0, if action == 0 { RED } else { theme::NAV_ACTIVE });
            }
            let col = if resp.hovered() && action == 0 { theme::BG } else { theme::TEXT_2 };
            kit::paint(&painter, r.center(), 16.0, col, icon, false);
            if resp.clicked() {
                ctx.send_viewport_cmd(match action {
                    0 => egui::ViewportCommand::Close,
                    1 => egui::ViewportCommand::Maximized(!maximized),
                    _ => egui::ViewportCommand::Minimized(true),
                });
            }
        }

        // Search box in the middle (only while the diary is open).
        if self.lock.is_none() {
            let width = (rect.width() - 460.0).clamp(240.0, 550.0);
            let box_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, cy), vec2(width, 34.0));
            painter.rect(box_rect, 10.0, theme::PANEL, Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);
            kit::paint(&painter, egui::pos2(box_rect.min.x + 20.0, cy), 18.0, theme::TEXT_3, kit::Icon::Search, false);
            let focused = ctx.memory(|m| m.has_focus(search_id()));
            let filters_on = self.search_state.open || self.search_state.filters.is_active();
            let show_filters = focused || !self.search.is_empty() || filters_on;
            let right_pad = if show_filters { 92.0 } else { 70.0 };
            let edit_rect = egui::Rect::from_min_max(egui::pos2(box_rect.min.x + 38.0, box_rect.min.y + 2.0), egui::pos2(box_rect.max.x - right_pad, box_rect.max.y - 2.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
                ui.add_sized(
                    edit_rect.size(),
                    egui::TextEdit::singleline(&mut self.search)
                        .id(search_id())
                        .frame(egui::Frame::NONE)
                        .hint_text(RichText::new("Search your journal").color(theme::TEXT_3))
                        .text_color(theme::TEXT)
                        .vertical_align(egui::Align::Center),
                );
            });
            if show_filters {
                let fr = egui::Rect::from_min_max(egui::pos2(box_rect.max.x - 84.0, box_rect.min.y + 4.0), egui::pos2(box_rect.max.x - 6.0, box_rect.max.y - 4.0));
                let resp = ui.interact(fr, ui.id().with("filters_toggle"), Sense::click()).on_hover_text("Search by category, mood, dates and more");
                painter.rect(fr, 7.0, if filters_on { theme::NAV_ACTIVE } else { Color32::TRANSPARENT }, Stroke::new(1.0, if filters_on { theme::shades().accent } else { theme::BORDER }), egui::StrokeKind::Inside);
                painter.text(fr.center(), Align2::CENTER_CENTER, "Filters", FontId::new(13.0, fonts::ui_bold()), if filters_on { theme::TEXT } else { theme::TEXT_2 });
                if resp.clicked() {
                    self.search_state.open = !self.search_state.open;
                }
            } else {
                let hint = egui::Rect::from_min_max(egui::pos2(box_rect.max.x - 66.0, box_rect.min.y + 7.0), egui::pos2(box_rect.max.x - 10.0, box_rect.max.y - 7.0));
                painter.rect(hint, 5.0, Color32::TRANSPARENT, Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);
                painter.text(hint.center(), Align2::CENTER_CENTER, "Ctrl F", FontId::proportional(11.5), theme::TEXT_3);
            }
        }
    }

    /// The left menu: new entry, places to go, categories, lock and settings.
    fn side_menu(&mut self, ui: &mut egui::Ui) {
        let p = palette(ui.ctx());
        ui.add_space(4.0);
        if kit::button(ui, kit::Look::Primary, Some(kit::Icon::Plus), "New entry", Some(ui.available_width())).on_hover_text("New entry (Ctrl+N)").clicked() {
            self.new_entry(self.selected);
        }
        ui.add_space(14.0);
        ui.spacing_mut().item_spacing.y = 6.0;

        let calendar_on = self.view != CalView::Photos && !self.search_state.showing(&self.search) && !self.insights.open && !self.show_pinned;
        if kit::nav_item(ui, Some(kit::Icon::Sun), None, "Today", None, false).on_hover_text("Jump to today (Ctrl+T)").clicked() {
            self.go_today();
            self.insights.open = false;
            self.show_pinned = false;
        }
        if kit::nav_item(ui, Some(kit::Icon::Calendar), None, "Calendar", None, calendar_on).clicked() {
            self.insights.open = false;
            self.show_pinned = false;
            if self.view == CalView::Photos {
                self.view = CalView::Month;
            }
            self.search.clear();
            self.search_state.reset();
        }
        let timeline_on = self.search_state.open && self.search.is_empty() && !self.search_state.filters.is_active();
        if kit::nav_item(ui, Some(kit::Icon::List), None, "Timeline", None, timeline_on).on_hover_text("Every entry, newest first").clicked() {
            self.search.clear();
            self.search_state.reset();
            self.search_state.open = true;
            self.insights.open = false;
            self.show_pinned = false;
        }
        let pinned = self.data.entries.iter().filter(|e| e.pinned).count();
        if kit::nav_item(ui, Some(kit::Icon::Star), None, "Pinned", (pinned > 0).then(|| pinned.to_string()), self.show_pinned).clicked() {
            self.show_pinned = !self.show_pinned;
            self.insights.open = false;
        }
        if kit::nav_item(ui, Some(kit::Icon::Image), None, "Photos", None, self.view == CalView::Photos && !self.insights.open && !self.show_pinned).clicked() {
            self.view = CalView::Photos;
            self.insights.open = false;
            self.show_pinned = false;
            self.search.clear();
            self.search_state.reset();
        }
        if kit::nav_item(ui, Some(kit::Icon::Chart), None, "Insights", None, self.insights.open).on_hover_text("Mood patterns, writing statistics and reviews").clicked() {
            self.insights.open = !self.insights.open;
            self.show_pinned = false;
        }

        // The category list needs room; on a short window it is left out.
        if ui.available_height() > 330.0 {
            ui.add_space(22.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                kit::section_label(ui, "Categories");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    if ui
                        .add(egui::Label::new(RichText::new("Edit").size(12.5).color(p.faint)).sense(Sense::click()))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Rename categories and change their colours")
                        .clicked()
                    {
                        self.show_categories = true;
                    }
                });
            });
            ui.add_space(2.0);
            let counts: Vec<(String, Color32, usize)> = self
                .data
                .categories
                .iter()
                .map(|c| (c.name.clone(), rgb(c.color), self.data.entries.iter().filter(|e| e.category == c.name).count()))
                .collect();
            egui::ScrollArea::vertical().id_salt("side_categories").max_height((ui.available_height() - 150.0).max(40.0)).show(ui, |ui| {
                for (name, col, n) in counts {
                    let on = self.search_state.filters.category == name && self.search_state.showing(&self.search);
                    if kit::nav_item(ui, None, Some(col), &name, Some(n.to_string()), on).on_hover_text("Show this category's entries").clicked() {
                        self.search.clear();
                        self.search_state.reset();
                        self.search_state.filters.category = name;
                        self.insights.open = false;
                        self.show_pinned = false;
                    }
                }
            });
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            if kit::nav_item(ui, Some(kit::Icon::Sliders), None, "Settings", None, false).on_hover_text("Settings").clicked() {
                self.show_settings = true;
                self.settings_section = 0;
                self.settings_jump = Some(0);
                self.insights.open = false;
                self.show_pinned = false;
            }
            if kit::nav_item(ui, Some(kit::Icon::Lock), None, "Lock journal", None, false).on_hover_text("Lock the diary (Ctrl+L)").clicked() {
                self.lock_now();
            }
            ui.add_space(4.0);
            let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0.0, theme::BORDER_SOFT);
        });

    }
    /// Copy the (already encrypted) diary file somewhere chosen by the user.
    fn backup(&mut self) {
        let name = format!("diary-backup-{}.json", Local::now().format("%Y-%m-%d"));
        let dest = rfd::FileDialog::new().set_file_name(name).add_filter("Encrypted diary", &["json"]).save_file();
        if let Some(dest) = dest {
            self.status = match std::fs::copy(data_path(), &dest) {
                Ok(_) => format!("Backup saved to {}", dest.display()),
                Err(e) => format!("Save failed: backup could not be written ({e})"),
            };
        }
    }

    /// Above the calendar: arrows, the month, Today / Surprise me and the Year / Month / Week / Day switch.
    fn calendar_toolbar(&mut self, ui: &mut egui::Ui) {
        let timed = self.view != CalView::Photos;
        let title = self.calendar_title();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if timed {
                if kit::icon_button(ui, kit::Icon::ChevronLeft, "Previous").clicked() {
                    self.step_calendar(-1);
                }
                if kit::icon_button(ui, kit::Icon::ChevronRight, "Next").clicked() {
                    self.step_calendar(1);
                }
                ui.add_space(10.0);
            }
            let font = FontId::new(if matches!(self.view, CalView::Month | CalView::Year | CalView::Photos) { 40.0 } else { 30.0 }, fonts::serif_display());
            match (self.view, title.split_once(' ')) {
                // "October 2026": the year is dimmer.
                (CalView::Month, Some((month, year))) => {
                    ui.label(RichText::new(month).font(font.clone()).color(theme::TEXT));
                    ui.label(RichText::new(year).font(font).color(theme::TEXT_3));
                }
                _ => {
                    ui.label(RichText::new(title).font(font).color(theme::TEXT));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if kit::button(ui, kit::Look::Ghost, Some(kit::Icon::Shuffle), "Surprise me", None).on_hover_text("Jump to a random entry from the past (Ctrl+R)").clicked() {
                    self.random_entry();
                }
                if kit::button(ui, kit::Look::Ghost, None, "Today", None).on_hover_text("Jump to today (Ctrl+T)").clicked() {
                    self.go_today();
                }
            });
        });
        if timed {
            ui.add_space(10.0);
            let views = [CalView::Year, CalView::Month, CalView::Week, CalView::Day];
            let current = views.iter().position(|v| *v == self.view);
            if let Some(i) = kit::segmented(ui, &["Year", "Month", "Week", "Day"], current) {
                self.view = views[i];
                self.day_scroll_pending = true;
            }
        }
    }

    fn calendar_title(&self) -> String {
        match self.view {
            CalView::Month => self.month.format("%B %Y").to_string(),
            CalView::Week => week_title(self.selected),
            CalView::Day => self.selected.format("%A %e %B %Y").to_string(),
            CalView::Year => self.selected.year().to_string(),
            CalView::Photos => "Photos".to_string(),
        }
    }

    fn shift_days(&mut self, delta: i64) {
        let moved = if delta >= 0 {
            self.selected.checked_add_days(chrono::Days::new(delta as u64))
        } else {
            self.selected.checked_sub_days(chrono::Days::new(delta.unsigned_abs()))
        };
        if let Some(d) = moved {
            self.selected = d;
            self.month = d.with_day(1).unwrap();
            self.day_scroll_pending = true;
        }
    }

    fn calendar(&mut self, ui: &mut egui::Ui) {
        self.calendar_toolbar(ui);
        ui.add_space(6.0);

        let mut ev = CalEvents::default();
        match self.view {
            CalView::Month => {
                let first = self.month;
                let days = (self.shift_target(first) - first).num_days() as u64;
                let last = first + chrono::Days::new(days - 1);
                let start = week_start(first);
                let end = week_start(last) + chrono::Days::new(6);
                let days: Vec<NaiveDate> = (0..=(end - start).num_days() as u64).map(|n| start + chrono::Days::new(n)).collect();
                self.grid_view(ui, &mut ev, &days, Some(first), 24.0);
            }
            CalView::Week => {
                let monday = week_start(self.selected);
                let days: Vec<NaiveDate> = (0..7).map(|n| monday + chrono::Days::new(n)).collect();
                self.grid_view(ui, &mut ev, &days, None, 28.0);
            }
            CalView::Day => self.day_timeline(ui, &mut ev),
            CalView::Year => self.year_view(ui, &mut ev),
            CalView::Photos => self.gallery_view(ui),
        }

        // Escape cancels a drag in progress.
        if self.drag.is_some() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.drag = None;
        }
        if let Some(d) = ev.select {
            self.selected = d;
            self.month = d.with_day(1).unwrap();
        }
        if let Some((d, hour)) = ev.new_on {
            self.selected = d;
            self.new_entry_at(d, hour);
        }
        if let Some(id) = ev.edit {
            self.start_edit(id);
        }
        if let Some((id, date, hour)) = ev.drop {
            self.move_entry(id, date, hour);
        }
    }

    /// Month and week views: a grid of day cells, seven to a row. `month` (the first of the month)
    /// dims the days around it that belong to the neighbouring months.
    fn grid_view(&mut self, ui: &mut egui::Ui, ev: &mut CalEvents, days: &[NaiveDate], month: Option<NaiveDate>, pill_h: f32) {
        let rows = days.len().div_ceil(7);
        let gap = 8.0;
        let head_h = 34.0;
        let avail = ui.available_size();
        let cell_w = (avail.x - gap * 6.0) / 7.0;
        let fitted = (avail.y - head_h - gap * (rows as f32 - 1.0)) / rows as f32;
        let cell_h = if rows == 1 { fitted.max(240.0) } else { fitted.clamp(96.0, 190.0) };
        let (grid, _) = ui.allocate_exact_size(vec2(avail.x, head_h + cell_h * rows as f32 + gap * (rows as f32 - 1.0)), Sense::hover());
        let painter = ui.painter_at(grid);
        let p = palette(ui.ctx());
        let today = Local::now().date_naive();
        let chip_size = 12.0;

        for (i, name) in ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"].iter().enumerate() {
            let x = grid.min.x + (cell_w + gap) * i as f32 + 12.0;
            let spaced: String = name.chars().flat_map(|c| [c, '\u{200A}']).collect();
            painter.text(egui::pos2(x, grid.min.y + head_h / 2.0), Align2::LEFT_CENTER, spaced, FontId::new(12.0, fonts::ui_bold()), p.faint);
        }

        self.ensure_stats();
        let by_day: Vec<Vec<Entry>> = days.iter().map(|d| self.day_entries(*d).into_iter().cloned().collect()).collect();
        let cats = &self.data.categories;
        let drag = &mut self.drag;
        let selected = self.selected;
        let day_stats = &self.stats_cache.as_ref().unwrap().1;
        let mut cells: Vec<(egui::Rect, NaiveDate)> = Vec::new();

        for (n, day) in days.iter().copied().enumerate() {
            let (col, row) = (n % 7, n / 7);
            let rect = egui::Rect::from_min_size(
                grid.min + vec2((cell_w + gap) * col as f32, head_h + (cell_h + gap) * row as f32),
                vec2(cell_w, cell_h),
            );
            cells.push((rect, day));
            let in_month = month.is_none_or(|m| day.month() == m.month() && day.year() == m.year());

            let resp = ui.interact(rect, ui.id().with(("cell", day)), Sense::click());
            if in_month {
                let bg = if resp.hovered() && drag.is_none() { p.hover } else { p.cell };
                painter.rect(rect, 10.0, bg, Stroke::new(1.0, p.border_soft), egui::StrokeKind::Inside);
            }
            if day == selected {
                painter.rect_stroke(rect, 10.0, Stroke::new(2.0, p.accent), egui::StrokeKind::Inside);
            }
            // The date: today sits in an amber circle.
            let num_at = egui::pos2(rect.min.x + 26.0, rect.min.y + 25.0);
            if day == today {
                painter.circle_filled(num_at, 14.0, p.today);
            }
            let num_color = if day == today { theme::BG } else if in_month { p.day_num } else { theme::TEXT_3.gamma_multiply(0.7) };
            painter.text(num_at, Align2::CENTER_CENTER, day.day().to_string(), FontId::new(15.0, fonts::ui_bold()), num_color);
            if in_month {
                if let Some(m) = day_stats.get(&day).and_then(|s| s.mood()) {
                    painter.circle_filled(egui::pos2(rect.max.x - 20.0, rect.min.y + 25.0), 5.0, mood_color(m));
                }
            }

            if in_month {
                let entries = &by_day[n];
                let top = rect.min.y + 48.0;
                let step = pill_h + 4.0;
                let fit = (((rect.max.y - top - 6.0) / step).floor() as usize).max(1);
                let shown = if entries.len() > fit { fit - 1 } else { entries.len() };
                for (i, e) in entries.iter().take(shown).enumerate() {
                    let pr = egui::Rect::from_min_size(egui::pos2(rect.min.x + 10.0, top + i as f32 * step), vec2(rect.width() - 20.0, pill_h));
                    pill(ui, &painter, pr, e, cats, chip_size, drag, ev);
                }
                if entries.len() > shown {
                    painter.text(
                        egui::pos2(rect.min.x + 14.0, top + shown as f32 * step + pill_h / 2.0),
                        Align2::LEFT_CENTER,
                        format!("+{} more", entries.len() - shown),
                        FontId::proportional(chip_size),
                        p.faint,
                    );
                }
            }

            if resp.double_clicked() {
                ev.new_on = Some((day, None));
            } else if resp.clicked() {
                ev.select = Some(day);
            }
        }

        // Dropping an entry on a day moves it there; the day under the pointer is outlined.
        let pointer = ui.ctx().pointer_latest_pos();
        if let Some((id, pos)) = ev.drop_at.take() {
            if let Some((_, d)) = cells.iter().find(|(r, _)| r.contains(pos)) {
                ev.drop = Some((id, *d, None));
            }
        }
        if drag.is_some() {
            if let Some((r, _)) = pointer.and_then(|pos| cells.iter().find(|(r, _)| r.contains(pos))) {
                painter.rect_stroke(*r, 10.0, Stroke::new(2.5, p.accent), egui::StrokeKind::Inside);
            }
        }
    }

    /// Day view: the selected day hour by hour. Entries sit in the hour they were made.
    fn day_timeline(&mut self, ui: &mut egui::Ui, ev: &mut CalEvents) {
        let p = palette(ui.ctx());
        let day = self.selected;
        let now = Local::now();
        let entries: Vec<Entry> = self.day_entries(day).into_iter().cloned().collect();
        let first_hour = entries.iter().filter_map(|e| entry_time(&e.added_at).map(|t| t.0)).min().unwrap_or(8).min(8);
        let cats = &self.data.categories;
        let drag = &mut self.drag;
        let scroll_pending = &mut self.day_scroll_pending;
        let mut rows: Vec<(egui::Rect, u32)> = Vec::new();

        egui::ScrollArea::vertical().id_salt("day_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let width = ui.available_width();
            let gutter = 64.0;
            for hour in 0..24u32 {
                let in_hour: Vec<&Entry> = entries.iter().filter(|e| entry_time(&e.added_at).map(|t| t.0) == Some(hour)).collect();
                let row_h = (in_hour.len() as f32 * 32.0 + 12.0).max(56.0);
                let (rect, _) = ui.allocate_exact_size(vec2(width, row_h), Sense::hover());
                if *scroll_pending && hour == first_hour {
                    ui.scroll_to_rect(rect, Some(egui::Align::TOP));
                    *scroll_pending = false;
                }
                let painter = ui.painter_at(rect);
                let slot = egui::Rect::from_min_max(rect.min + vec2(gutter, 0.0), rect.max);
                rows.push((slot, hour));
                let resp = ui.interact(slot, ui.id().with(("hour", hour)), Sense::click());
                let bg = if day == now.date_naive() && hour == now.hour() {
                    p.today
                } else if resp.hovered() && drag.is_none() {
                    p.hover
                } else if !(7..22).contains(&hour) {
                    p.cell
                } else {
                    p.cell
                };
                painter.rect_filled(slot.shrink(1.0), 4.0, bg);
                painter.rect_stroke(slot.shrink(1.0), 4.0, Stroke::new(1.0, p.cell_border), egui::StrokeKind::Inside);
                painter.text(
                    egui::pos2(rect.min.x + gutter - 10.0, rect.min.y + 8.0),
                    Align2::RIGHT_TOP,
                    format!("{hour:02}:00"),
                    FontId::proportional(15.0),
                    p.muted,
                );
                for (i, e) in in_hour.iter().enumerate() {
                    let pr = egui::Rect::from_min_size(slot.min + vec2(8.0, 8.0 + i as f32 * 32.0), vec2(slot.width() - 16.0, 28.0));
                    pill(ui, &painter, pr, e, cats, 16.0, drag, ev);
                }
                if resp.double_clicked() {
                    ev.new_on = Some((day, Some(hour)));
                }
            }
        });

        // Dropping an entry on an hour changes its time (keeping the minutes).
        let pointer = ui.ctx().pointer_latest_pos();
        if let Some((id, pos)) = ev.drop_at.take() {
            if let Some((_, h)) = rows.iter().find(|(r, _)| r.contains(pos)) {
                ev.drop = Some((id, day, Some(*h)));
            }
        }
        if drag.is_some() {
            if let Some((r, _)) = pointer.and_then(|pos| rows.iter().find(|(r, _)| r.contains(pos))) {
                ui.painter().rect_stroke(*r, 4.0, Stroke::new(3.0, p.accent), egui::StrokeKind::Inside);
            }
        }
    }

    fn shift_year(&mut self, delta: i32) {
        let year = self.selected.year() + delta;
        // 29 February maps to 28 February in a non-leap year.
        let moved = NaiveDate::from_ymd_opt(year, self.selected.month(), self.selected.day())
            .or_else(|| NaiveDate::from_ymd_opt(year, self.selected.month(), 28));
        if let Some(d) = moved {
            self.selected = d;
            self.month = d.with_day(1).unwrap();
        }
    }

    /// Recompute the per-day totals if the diary has changed since they were last worked out.
    fn ensure_stats(&mut self) {
        if self.stats_cache.as_ref().map(|c| c.0) != Some(self.rev) {
            self.stats_cache = Some((self.rev, stats::per_day(&self.data.entries)));
        }
    }

    /// Year view: a GitHub-style heat-map of the days written, plus totals, categories and mood.
    fn year_view(&mut self, ui: &mut egui::Ui, ev: &mut CalEvents) {
        self.ensure_stats();
        let p = palette(ui.ctx());
        let dark = ui.ctx().theme() == egui::Theme::Dark;
        let year = self.selected.year();
        let today = Local::now().date_naive();
        let goal = self.data.settings.daily_word_goal;
        let (jan1, dec31) = (NaiveDate::from_ymd_opt(year, 1, 1).unwrap(), NaiveDate::from_ymd_opt(year, 12, 31).unwrap());
        let days = &self.stats_cache.as_ref().unwrap().1;
        let summary = stats::summarise(&self.data.entries, days, jan1, dec31, today, goal);
        let series = stats::mood_series(days, jan1, dec31);
        let cats = &self.data.categories;
        let selected = self.selected;
        let mut open_day = None;

        let content_w = ui.available_width() - 16.0;
        egui::ScrollArea::vertical().id_salt("year_scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(content_w);
            // ---- heat-map: one column per week, one row per weekday
            let (label_w, gap, top_h) = (38.0, 3.0, 22.0);
            let start = week_start(jan1);
            let weeks = ((week_start(dec31) - start).num_days() / 7 + 1) as f32;
            let cell = ((ui.available_width() - label_w - gap * weeks) / weeks).clamp(8.0, 24.0);
            let (rect, _) = ui.allocate_exact_size(vec2(label_w + weeks * (cell + gap), top_h + 7.0 * (cell + gap)), Sense::hover());
            let painter = ui.painter_at(rect);
            let cell_pos = |day: NaiveDate| {
                let col = ((week_start(day) - start).num_days() / 7) as f32;
                let row = day.weekday().num_days_from_monday() as f32;
                rect.min + vec2(label_w + col * (cell + gap), top_h + row * (cell + gap))
            };
            for m in 1..=12 {
                let first = NaiveDate::from_ymd_opt(year, m, 1).unwrap();
                painter.text(
                    egui::pos2(cell_pos(first).x, rect.min.y),
                    Align2::LEFT_TOP,
                    first.format("%b").to_string(),
                    FontId::proportional(14.0),
                    p.muted,
                );
            }
            for (row, name) in [(0.0, "Mon"), (2.0, "Wed"), (4.0, "Fri")] {
                painter.text(
                    egui::pos2(rect.min.x, rect.min.y + top_h + row * (cell + gap)),
                    Align2::LEFT_TOP,
                    name,
                    FontId::proportional(12.0),
                    p.muted,
                );
            }
            for n in 0..=(dec31 - jan1).num_days() {
                let day = jan1 + chrono::Days::new(n as u64);
                let r = egui::Rect::from_min_size(cell_pos(day), vec2(cell, cell));
                let stat = days.get(&day);
                painter.rect_filled(r, 3.0, heat_color(stats::heat_level(stat), dark));
                if day == today {
                    painter.rect_stroke(r, 3.0, Stroke::new(2.0, p.accent), egui::StrokeKind::Outside);
                }
                if day == selected {
                    painter.rect_stroke(r, 3.0, Stroke::new(2.0, p.day_num), egui::StrokeKind::Outside);
                }
                let resp = ui.interact(r, ui.id().with(("heat", day)), Sense::click());
                let resp = resp.on_hover_ui(|ui| {
                    let (n, w) = stat.map_or((0, 0), |s| (s.entries, s.words));
                    ui.label(RichText::new(day.format("%A %e %B %Y").to_string()).strong());
                    ui.label(format!("{n} {} · {w} words", if n == 1 { "entry" } else { "entries" }));
                });
                if resp.double_clicked() {
                    open_day = Some(day);
                } else if resp.clicked() {
                    ev.select = Some(day);
                }
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("Less").small().color(p.muted));
                for level in 0..=4u8 {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    ui.painter().rect_filled(r, 3.0, heat_color(level, dark));
                }
                ui.label(RichText::new("More  ·  shade shows words written that day. Click a day to select it, double-click to open it.").small().color(p.muted));
            });

            // ---- totals
            ui.add_space(14.0);
            ui.label(RichText::new(format!("{year} in numbers")).font(FontId::new(26.0, fonts::serif_medium())).color(p.title));
            ui.add_space(6.0);
            let mut tiles: Vec<(String, String)> = vec![
                ("entries".into(), summary.entries.to_string()),
                ("days written".into(), summary.days_written.to_string()),
                ("words".into(), summary.words.to_string()),
                ("day streak now".into(), summary.current_streak.to_string()),
                ("longest streak".into(), summary.longest_streak.to_string()),
                ("average mood".into(), summary.mood_avg.map_or("–".to_string(), |m| format!("{m:.1}"))),
            ];
            if goal > 0 {
                tiles.push((format!("days with {goal}+ words"), summary.goal_days.to_string()));
            }
            // Evenly sized tiles, as many to a row as fit.
            let gap = 10.0;
            let per_row = (((ui.available_width() + gap) / (170.0 + gap)).floor() as usize).clamp(1, tiles.len());
            let tile_w = (ui.available_width() - gap * (per_row as f32 - 1.0)) / per_row as f32;
            for row in tiles.chunks(per_row) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (label, value) in row {
                        egui::Frame::new().fill(p.cell).stroke(Stroke::new(1.0, p.border_soft)).corner_radius(12).inner_margin(14).show(ui, |ui| {
                            ui.set_width(tile_w - 28.0);
                            ui.label(RichText::new(value).font(FontId::new(30.0, fonts::serif_display())).color(p.title));
                            ui.label(RichText::new(label).size(13.5).color(p.muted));
                        });
                    }
                });
            }

            // ---- categories
            ui.add_space(14.0);
            ui.label(RichText::new("Entries by category").font(FontId::new(26.0, fonts::serif_medium())).color(p.title));
            ui.add_space(6.0);
            if summary.categories.is_empty() {
                ui.label(RichText::new("Nothing written in this year yet.").color(p.muted));
            }
            let most = summary.categories.first().map_or(1, |c| c.1).max(1) as f32;
            for (name, count) in &summary.categories {
                ui.horizontal(|ui| {
                    ui.add_sized([120.0, 22.0], egui::Label::new(RichText::new(name).color(p.ink)).truncate());
                    let max_w = (ui.available_width() - 50.0).max(40.0);
                    let (r, _) = ui.allocate_exact_size(vec2(max_w * *count as f32 / most, 20.0), Sense::hover());
                    ui.painter().rect_filled(r, 5.0, category_color(cats, name));
                    ui.label(RichText::new(count.to_string()).color(p.ink));
                });
            }

            // ---- mood
            ui.add_space(14.0);
            ui.label(RichText::new("Mood through the year").font(FontId::new(26.0, fonts::serif_medium())).color(p.title));
            ui.add_space(6.0);
            if series.is_empty() {
                ui.label(RichText::new("Rate your entries with the Mood buttons in the editor and they will be charted here.").color(p.muted));
            } else {
                mood_chart(ui, &p, &series, year);
                ui.label(RichText::new("Dots are the day's average mood; the line is a 7-day average.").small().color(p.muted));
            }
            ui.add_space(12.0);
        });

        if let Some(day) = open_day {
            ev.select = Some(day);
            self.view = CalView::Day;
            self.day_scroll_pending = true;
        }
    }

    /// Move an entry to another day (and, in the day view, another hour).
    fn move_entry(&mut self, id: u64, date: NaiveDate, hour: Option<u32>) {
        let Some(e) = self.data.entries.iter_mut().find(|e| e.id == id) else { return };
        let new_stamp = moved_timestamp(&e.added_at, date, hour);
        if e.date == key(date) && e.added_at == new_stamp {
            return;
        }
        e.date = key(date);
        e.added_at = new_stamp;
        self.selected = date;
        self.month = date.with_day(1).unwrap();
        self.persist();
    }

    /// Open the editor for an existing entry.
    fn start_edit(&mut self, id: u64) {
        let Some(e) = self.data.entries.iter().find(|e| e.id == id) else { return };
        let date = NaiveDate::parse_from_str(&e.date, "%Y-%m-%d").unwrap_or(self.selected);
        let (hour, minute, second) = entry_time(&e.added_at).unwrap_or((0, 0, 0));
        self.editor = Some(EditState {
            id: Some(id),
            date,
            hour,
            minute,
            second,
            category: e.category.clone(),
            title: markdown::split_title(&e.text).0.unwrap_or_default(),
            text: markdown::split_title(&e.text).1,
            baseline: String::new(),
            discard_arm: false,
            mood: e.mood,
            pinned: e.pinned,
            sensitive: e.sensitive,
            attachments: e.attachments.clone(),
            ai: e.ai,
            focus: 0,
            prompt_offset: 0,
            want_focus: Some((usize::MAX, usize::MAX)),
        });
        self.editor_msg.clear();
        if let Some(ed) = self.editor.as_mut() {
            ed.baseline = ed.full_text();
        }
    }

    fn shift_target(&self, first: NaiveDate) -> NaiveDate {
        let total = first.year() * 12 + first.month0() as i32 + 1;
        NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap()
    }

    fn day_panel(&mut self, ui: &mut egui::Ui) {
        let p = palette(ui.ctx());
        if self.search_state.showing(&self.search) {
            self.search_results(ui, &p);
            return;
        }
        let entries: Vec<Entry> = self.day_entries(self.selected).into_iter().cloned().collect();
        let memories: Vec<Entry> = stats::on_this_day(&self.data.entries, self.selected).into_iter().cloned().collect();
        let words: usize = entries.iter().filter(|e| !e.generated).map(|e| markdown::word_count(&e.text)).sum();
        let goal = self.data.settings.daily_word_goal as usize;

        // The day, in words.
        kit::section_label(ui, &self.selected.format("%A").to_string());
        ui.add_space(2.0);
        ui.label(RichText::new(self.selected.format("%-d %B %Y").to_string()).font(FontId::new(34.0, fonts::serif_display())).color(p.title));
        let moods: Vec<f32> = entries.iter().filter(|e| !e.sensitive).filter_map(|e| e.mood).map(f32::from).collect();
        let mut summary = format!("{} {} · {} words", entries.len(), if entries.len() == 1 { "entry" } else { "entries" }, words);
        if !moods.is_empty() {
            let avg = (moods.iter().sum::<f32>() / moods.len() as f32).round() as usize;
            summary.push_str(&format!(" · Mood: {}", theme::MOOD_NAMES[avg.clamp(1, 5) - 1]));
        }
        ui.label(RichText::new(summary).size(14.0).color(p.muted));
        ui.add_space(14.0);

        if kit::button(ui, kit::Look::AccentOutline, Some(kit::Icon::Plus), "Write about this day", Some(ui.available_width())).on_hover_text("New entry (Ctrl+N)").clicked() {
            self.new_entry(self.selected);
        }
        let mut ask_summary = false;
        if self.ai_ready() && self.day_has_summarisable(self.selected) {
            ui.add_space(8.0);
            if kit::button(ui, kit::Look::Ghost, Some(kit::Icon::Sparkle), "Summarise", None)
                .on_hover_text("An AI model on this PC writes a short summary of the day's entries (private entries are never used)")
                .clicked()
            {
                ask_summary = true;
            }
        }
        if goal > 0 {
            ui.add_space(12.0);
            let done = words >= goal;
            ui.label(RichText::new(format!("{words} / {goal} words{}", if done { "  ✓" } else { "" })).size(13.0).color(if done { p.accent } else { p.muted }));
            let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
            ui.painter().rect_filled(bar, 3.0, p.cell_border);
            let mut filled = bar;
            filled.set_width(bar.width() * (words as f32 / goal as f32).min(1.0));
            ui.painter().rect_filled(filled, 3.0, p.accent);
        }
        if self.status.starts_with("Save failed") {
            ui.add_space(6.0);
            ui.label(RichText::new(&self.status).color(RED));
        } else if !self.status.is_empty() && self.status != "Saved" {
            ui.add_space(6.0);
            ui.label(RichText::new(&self.status).small().color(p.faint));
        }
        let sync_line = self.sync_status_text();
        if !sync_line.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(sync_line).small().color(p.faint));
        }
        // Undo for the last delete.
        let mut undo = None;
        if let Some(id) = self.last_trashed.filter(|id| self.data.trash.iter().any(|t| t.entry.id == *id)) {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Entry moved to Recently deleted.").small().color(p.muted));
                if ui.small_button("Undo").clicked() {
                    undo = Some(id);
                }
            });
        }
        self.recovery_hint(ui, &p);
        ui.add_space(14.0);

        let mut edit = None;
        let mut delete = None;
        let mut jump = None;
        let mut toggle_pin = None;
        let mut toggle_more: Option<u64> = None;
        let mut reveal = None;
        let mut hide = None;
        let mut save_attachment: Option<Attachment> = None;
        let mut open_attachment: Option<Attachment> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 12.0;
            if entries.is_empty() {
                // A blank page: a dashed outline with a quiet note.
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 130.0), Sense::hover());
                let outline = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                ui.painter().extend(egui::Shape::dashed_line(&outline, Stroke::new(1.2, theme::BORDER), 6.0, 5.0));
                ui.painter().text(r.center(), Align2::CENTER_CENTER, "A blank page.", FontId::new(19.0, fonts::serif_italic()), p.faint);
            }
            for e in &entries {
                let c = category_color(&self.data.categories, &e.category);
                let hidden = e.sensitive && !self.revealed.contains(&e.id);
                egui::Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border_soft)).corner_radius(14).inner_margin(egui::Margin::same(18)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.horizontal(|ui| {
                        kit::category_pill(ui, c, &e.category);
                        ui.label(RichText::new(e.added_at.get(11..16).unwrap_or("")).size(13.0).color(p.faint));
                        if e.sensitive {
                            ui.label(RichText::new("Private").size(12.0).italics().color(p.faint));
                        }
                        if e.generated {
                            ui.label(RichText::new("AI summary").size(12.0).italics().color(p.faint));
                        } else if e.ai {
                            ui.label(RichText::new("AI-assisted").size(12.0).italics().color(p.faint));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Star: pin or unpin this entry.
                            let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                            kit::paint(ui.painter(), r.center(), 18.0, if e.pinned { theme::TODAY } else { p.faint }, kit::Icon::Star, e.pinned);
                            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(if e.pinned { "Unpin" } else { "Pin this entry" }).clicked() {
                                toggle_pin = Some(e.id);
                            }
                        });
                    });
                    if hidden {
                        // Stand-in lines where the text would be, and a click to reveal.
                        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 62.0), Sense::click());
                        for (i, width) in [0.95, 0.8, 0.55].iter().enumerate() {
                            let bar = egui::Rect::from_min_size(r.min + vec2(0.0, 4.0 + i as f32 * 18.0), vec2(r.width() * width, 10.0));
                            ui.painter().rect_filled(bar, 5.0, p.cell_border.gamma_multiply(0.6));
                        }
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, "Private entry - click to show", FontId::proportional(14.0), p.ink);
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            reveal = Some(e.id);
                        }
                    } else {
                        let (title, body) = markdown::split_title(&e.text);
                        if let Some(title) = title {
                            ui.label(RichText::new(title).font(FontId::new(22.0, fonts::serif_medium())).color(p.title));
                        }
                        let is_open = self.expanded.contains(&e.id);
                        let has_picture = !markdown::image_ids(&body).is_empty();
                        let preview = markdown::plain_text(&body);
                        let longer = has_picture || preview.chars().count() > 150 || body.trim().contains('\n');
                        if is_open {
                            // The whole entry, pictures and all.
                            let style = markdown::Style { text: p.ink, link: p.accent, size: 16.0, serif: true };
                            markdown::render(ui, &body, &style, &mut self.media, &self.data.images);
                        } else if !preview.is_empty() {
                            let mut job = egui::text::LayoutJob::default();
                            job.wrap.max_width = ui.available_width();
                            job.wrap.max_rows = 5;
                            job.wrap.overflow_character = Some('…');
                            job.append(
                                &preview,
                                0.0,
                                egui::text::TextFormat { font_id: FontId::new(16.0, fonts::serif()), color: p.muted, line_height: Some(16.0 * 1.6), ..Default::default() },
                            );
                            ui.label(job);
                        }
                        if longer && kit::text_link(ui, if is_open { "Show less" } else if has_picture { "Show more, with picture" } else { "Show more" }, if is_open { kit::Icon::ChevronUp } else { kit::Icon::ChevronDown }).clicked() {
                            toggle_more = Some(e.id);
                        }
                        for a in &e.attachments {
                            ui.horizontal(|ui| {
                                kit::paint(ui.painter(), ui.cursor().min + vec2(8.0, 10.0), 15.0, p.faint, kit::Icon::Paperclip, false);
                                ui.add_space(20.0);
                                ui.label(RichText::new(format!("{} ({})", a.name, human_size(a.size))).size(13.0).color(p.muted));
                                if ui.small_button("Save as…").clicked() {
                                    save_attachment = Some(a.clone());
                                }
                                if is_risky_file(&a.name) {
                                    // Programs and scripts can be saved but are never launched from the diary.
                                    ui.label(RichText::new("can't be opened here").size(12.0).color(p.faint)).on_hover_text(RISKY_HINT);
                                } else if ui.small_button("Open").clicked() {
                                    open_attachment = Some(a.clone());
                                }
                            });
                        }
                    }
                    // Footer: word count, Open and the more-menu.
                    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                    ui.painter().rect_filled(line, 0.0, p.border_soft);
                    ui.horizontal(|ui| {
                        if !hidden {
                            ui.label(RichText::new(format!("{} words", markdown::word_count(&e.text))).size(13.0).color(p.faint));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let more = kit::small_button(ui, kit::Look::Ghost, Some(kit::Icon::Dots), "").on_hover_text("More");
                            egui::Popup::menu(&more).show(|ui| {
                                ui.set_min_width(150.0);
                                if e.sensitive && !hidden && ui.button("Hide again").clicked() {
                                    hide = Some(e.id);
                                    ui.close();
                                }
                                if ui.add(egui::Button::new(RichText::new("Delete").color(RED))).clicked() {
                                    delete = Some(e.id);
                                    ui.close();
                                }
                            });
                            if !hidden && kit::small_button(ui, kit::Look::Ghost, None, "Open").clicked() {
                                edit = Some(e.id);
                            }
                        });
                    });
                });
            }

            // What was written on this date in earlier years.
            if !memories.is_empty() {
                ui.add_space(10.0);
                kit::section_label(ui, "On this day");
                for m in memories.iter().take(12) {
                    let c = category_color(&self.data.categories, &m.category);
                    let card = egui::Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border_soft)).corner_radius(12).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.painter().circle_filled(ui.cursor().min + vec2(5.0, 11.0), 3.5, c);
                            ui.add_space(14.0);
                            ui.label(RichText::new(m.date.get(0..4).unwrap_or("")).font(FontId::new(18.0, fonts::serif_medium())).color(p.title));
                            ui.label(RichText::new(m.added_at.get(11..16).unwrap_or("")).size(13.0).color(p.faint));
                        });
                        let mut s = entry_label(m);
                        if s.chars().count() > 100 {
                            s = s.chars().take(100).collect::<String>() + "…";
                        }
                        ui.label(RichText::new(s).size(14.0).color(p.muted));
                    });
                    let resp = ui.interact(card.response.rect, ui.id().with(("memory", m.id)), Sense::click());
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        jump = NaiveDate::parse_from_str(&m.date, "%Y-%m-%d").ok();
                    }
                }
            }
        });

        if let Some(id) = edit {
            self.start_edit(id);
        }
        if let Some(id) = delete {
            self.trash_entry(id);
        }
        if let Some(d) = jump {
            self.selected = d;
            self.month = d.with_day(1).unwrap();
            self.day_scroll_pending = true;
        }
        if ask_summary {
            self.ai_summarise_day(self.selected);
        }
        if let Some(id) = undo {
            self.restore_trashed(id);
        }
        if let Some(id) = toggle_more {
            if !self.expanded.remove(&id) {
                self.expanded.insert(id);
            }
        }
        if let Some(id) = toggle_pin {
            if let Some(e) = self.data.entries.iter_mut().find(|e| e.id == id) {
                e.pinned = !e.pinned;
            }
            self.persist();
        }
        if let Some(id) = reveal {
            self.revealed.insert(id);
        }
        if let Some(id) = hide {
            self.revealed.remove(&id);
        }
        if let Some(a) = save_attachment {
            self.save_attachment_as(&a);
        }
        if let Some(a) = open_attachment {
            self.open_attachment(&a);
        }
    }

    /// The bytes of an attached file, if it is in the diary.
    fn attachment_bytes(&self, a: &Attachment) -> Option<Vec<u8>> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.decode(self.data.files.get(&a.id)?).ok()
    }

    fn save_attachment_as(&mut self, a: &Attachment) {
        let Some(dest) = rfd::FileDialog::new().set_file_name(a.name.clone()).save_file() else { return };
        self.status = match self.attachment_bytes(a).map(|b| std::fs::write(&dest, b)) {
            Some(Ok(())) => format!("Saved {}", dest.display()),
            Some(Err(e)) => format!("Save failed: couldn't write the file ({e})"),
            None => "Save failed: that attachment is missing from the diary".into(),
        };
    }

    /// Write an attached file to a temporary folder and open it with the program Windows picks.
    /// The temporary copy is not encrypted; the folder is emptied when the diary locks or starts.
    fn open_attachment(&mut self, a: &Attachment) {
        // Opening hands the file to Windows, which would run a program or script. Never do that.
        if is_risky_file(&a.name) {
            self.status = format!("“{}” {RISKY_HINT_SHORT}", a.name);
            return;
        }
        let Some(bytes) = self.attachment_bytes(a) else {
            self.status = "Save failed: that attachment is missing from the diary".into();
            return;
        };
        let dir = open_files_dir();
        let name: String = a.name.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c }).collect();
        let path = dir.join(format!("{}-{name}", a.id));
        match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, bytes)) {
            Ok(()) => {
                std::process::Command::new("cmd").args(["/C", "start", ""]).arg(&path).spawn().ok();
            }
            Err(e) => self.status = format!("Save failed: couldn't open the file ({e})"),
        }
    }

    /// Write the user guide to a temporary folder and open it in the PDF viewer Windows picks.
    fn open_user_guide(&mut self) {
        let dir = open_files_dir();
        let path = dir.join("Tideline Journal user guide.pdf");
        match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, USER_GUIDE)) {
            Ok(()) => {
                std::process::Command::new("cmd").args(["/C", "start", ""]).arg(&path).spawn().ok();
            }
            Err(e) => self.status = format!("Save failed: couldn't open the user guide ({e})"),
        }
    }

    /// Let the user keep a copy of the user guide wherever they like.
    fn save_user_guide(&mut self) {
        let Some(dest) = rfd::FileDialog::new().set_file_name("Tideline Journal user guide.pdf").add_filter("PDF document", &["pdf"]).save_file() else {
            return;
        };
        self.status = match std::fs::write(&dest, USER_GUIDE) {
            Ok(()) => "Saved the user guide".into(),
            Err(e) => format!("Save failed: couldn't write the user guide ({e})"),
        };
    }

    fn categories_window(&mut self, ctx: &egui::Context) {
        let p = palette(ctx);
        if !self.show_categories {
            return;
        }
        let mut open = true;
        let dark = ctx.theme() == egui::Theme::Dark;
        let mut changed = false;
        let mut remove = None;
        let mut renames: Vec<(String, String)> = Vec::new();

        egui::Window::new("Categories")
            .title_bar(false)
            .frame(popup_frame(&p, dark))
            .resizable(true)
            .default_size([520.0, 420.0])
            .min_size([360.0, 260.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                if popup_header(ui, &p, "Categories") {
                    open = false;
                }
                egui::Frame::new().inner_margin(14).show(ui, |ui| {
                let can_delete = self.data.categories.len() > 1;
                let names: Vec<String> = self.data.categories.iter().map(|c| c.name.clone()).collect();
                let big = FontId::proportional(18.0);
                let row_h = 36.0;
                let field_margin = egui::Margin::symmetric(10, 0);
                for (i, c) in self.data.categories.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().interact_size = vec2(48.0, row_h);
                        changed |= ui.color_edit_button_srgb(&mut c.color).changed();

                        // Delete sits at the right edge; the name field takes the space in between.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let del = egui::Button::new(RichText::new("Delete").color(theme::BG)).fill(RED);
                            if ui.add_enabled(can_delete, del).clicked() {
                                remove = Some(i);
                            }

                            // Edit a buffered copy and only apply the rename when editing finishes.
                            let mut text = match &self.rename_buf {
                                Some((j, b)) if *j == i => b.clone(),
                                _ => c.name.clone(),
                            };
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut text)
                                    .font(big.clone())
                                    .margin(field_margin)
                                    .min_size(vec2(0.0, row_h))
                                    .vertical_align(egui::Align::Center)
                                    .text_color(p.ink)
                                    .desired_width(ui.available_width()),
                            );
                            if resp.changed() {
                                self.rename_buf = Some((i, text));
                            } else if resp.lost_focus() {
                                if let Some((j, new)) = self.rename_buf.take() {
                                    let new = new.trim().to_string();
                                    let clash = names.iter().enumerate().any(|(k, n)| k != j && n.eq_ignore_ascii_case(&new));
                                    if j == i && !new.is_empty() && !clash && new != c.name {
                                        renames.push((std::mem::replace(&mut c.name, new.clone()), new));
                                    }
                                }
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
                ui.weak("Click a name to rename it; press Enter to apply.");
                ui.separator();
                ui.label("Add a category");
                ui.horizontal(|ui| {
                    ui.spacing_mut().interact_size = vec2(48.0, row_h);
                    ui.color_edit_button_srgb(&mut self.new_cat_color);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let name = self.new_cat_name.trim().to_string();
                        let dup = self.data.categories.iter().any(|c| c.name.eq_ignore_ascii_case(&name));
                        if ui.add_enabled(!name.is_empty() && !dup, egui::Button::new("Add")).clicked() {
                            self.data.categories.push(Category { name, color: self.new_cat_color, extra: Default::default() });
                            self.new_cat_name.clear();
                            changed = true;
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_cat_name)
                                .font(big.clone())
                                .margin(field_margin)
                                .min_size(vec2(0.0, row_h))
                                .vertical_align(egui::Align::Center)
                                .hint_text("Name")
                                .text_color(p.ink)
                                .desired_width(ui.available_width()),
                        );
                    });
                });
                ui.weak("Entries in a deleted category show in grey.");
                });
            });

        for (old, new) in renames {
            for e in self.data.entries.iter_mut().filter(|e| e.category == old) {
                e.category = new.clone();
            }
            if let Some(ed) = self.editor.as_mut().filter(|ed| ed.category == old) {
                ed.category = new;
            }
            changed = true;
        }
        if let Some(i) = remove {
            self.data.categories.remove(i);
            changed = true;
        }
        if changed {
            self.persist();
        }
        if !open {
            self.show_categories = false;
        }
    }
}

fn make_visuals() -> egui::Visuals {
    let t = theme::shades();
    let mut v = egui::Visuals::dark();
    v.panel_fill = theme::BG;
    v.window_fill = theme::PANEL;
    v.window_corner_radius = 14.into();
    v.window_stroke = Stroke::new(1.0, theme::BORDER);
    v.window_shadow = popup_shadow(true);
    v.extreme_bg_color = theme::BG;
    v.faint_bg_color = theme::SURFACE;
    v.hyperlink_color = t.accent;
    v.widgets.noninteractive.bg_fill = theme::PANEL;
    v.widgets.noninteractive.weak_bg_fill = theme::PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme::BORDER_SOFT);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme::TEXT);
    v.widgets.noninteractive.corner_radius = 8.into();
    // Buttons are dark with a border; the accent is kept for the main action on a screen.
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = 8.into();
    }
    v.widgets.inactive.weak_bg_fill = theme::PANEL;
    v.widgets.inactive.bg_fill = theme::PANEL;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, theme::BORDER);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, theme::TEXT);
    v.widgets.hovered.weak_bg_fill = theme::NAV_ACTIVE;
    v.widgets.hovered.bg_fill = theme::NAV_ACTIVE;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, theme::TEXT_3);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, theme::TEXT);
    v.widgets.active.weak_bg_fill = theme::NAV_ACTIVE;
    v.widgets.active.bg_fill = theme::NAV_ACTIVE;
    v.widgets.active.bg_stroke = Stroke::new(1.0, t.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0, theme::TEXT);
    v.widgets.open.weak_bg_fill = theme::NAV_ACTIVE;
    v.widgets.open.bg_fill = theme::NAV_ACTIVE;
    v.widgets.open.bg_stroke = Stroke::new(1.0, theme::BORDER);
    v.selection.bg_fill = t.accent_soft;
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v
}

/// The fonts (Hanken Grotesk and Newsreader, bundled).
fn setup_fonts(ctx: &egui::Context) {
    fonts::install(ctx);
}

/// Use the current accent colour everywhere (call after `theme::set_accent`).
fn apply_theme(ctx: &egui::Context) {
    ctx.set_visuals_of(egui::Theme::Light, make_visuals());
    ctx.set_visuals_of(egui::Theme::Dark, make_visuals());
}

fn setup_style(ctx: &egui::Context) {
    setup_fonts(ctx);
    // Tideline Journal is dark-only.
    ctx.options_mut(|o| {
        o.theme_preference = egui::ThemePreference::Dark;
        o.zoom_with_keyboard = false; // the app has its own, saved, text size
    });
    apply_theme(ctx);
    ctx.global_style_mut(|s| {
        use egui::{FontFamily, TextStyle};
        s.spacing.button_padding = vec2(14.0, 8.0);
        s.spacing.interact_size.y = 34.0;
        s.spacing.item_spacing = vec2(8.0, 8.0);
        let sizes = [
            (TextStyle::Small, FontId::new(12.5, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.5, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(22.0, fonts::serif_medium())),
            (TextStyle::Monospace, FontId::new(14.0, FontFamily::Monospace)),
        ];
        for (style, font) in sizes {
            s.text_styles.insert(style, font);
        }
    });
}

impl eframe::App for DiaryApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_start_mode(&ctx);
        let p = palette(&ctx);
        if self.lock.is_none() {
            self.auto_backup();
        }
        if self.lock.is_none() {
            self.sync_tick(&ctx);
        }
        // Auto-lock after a period without any input.
        if self.lock.is_none() {
            if ctx.input(|i| !i.events.is_empty()) {
                self.last_activity = std::time::Instant::now();
            }
            let minutes = self.data.settings.auto_lock_minutes;
            if minutes > 0 {
                let limit = std::time::Duration::from_secs(u64::from(minutes) * 60);
                let idle = self.last_activity.elapsed();
                if idle >= limit {
                    self.lock_now();
                } else {
                    ctx.request_repaint_after(limit - idle + std::time::Duration::from_millis(250));
                }
            }
        }
        // Interface zoom (the saved text-size setting).
        if self.lock.is_none() {
            let scale = self.data.settings.ui_scale.clamp(0.7, 2.0);
            if (ctx.zoom_factor() - scale).abs() > 0.001 {
                ctx.set_zoom_factor(scale);
            }
        }
        // Privacy screen: hide everything while the window is not the active one.
        if self.lock.is_none() && self.data.settings.privacy_screen {
            let (focused, minimized) = ctx.input(|i| (i.viewport().focused.unwrap_or(true), i.viewport().minimized.unwrap_or(false)));
            if !focused || minimized {
                self.revealed.clear();
                egui::CentralPanel::default().frame(egui::Frame::new().fill(p.page_bg)).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space((ui.available_height() / 2.0 - 50.0).max(20.0));
                        ui.label(RichText::new("Tideline Journal").font(FontId::new(36.0, fonts::serif_display())).color(p.title));
                        ui.label(RichText::new("Hidden while this window is in the background. Click here to show it again.").color(p.muted));
                    });
                });
                return;
            }
        }
        self.handle_shortcuts(&ctx);
        self.ai_poll(&ctx);
        // The title bar is on every screen, the lock screen included.
        egui::Panel::top("titlebar")
            .exact_size(44.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(theme::BG_TITLEBAR))
            .show(ui, |ui| self.title_bar(ui));
        if self.lock.is_some() {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(p.page_bg))
                .show(ui, |ui| self.lock_screen(ui));
            return;
        }
        if self.show_settings && self.search_state.showing(&self.search) {
            self.show_settings = false;
        }
        if self.editor.is_some() {
            // Writing takes the whole window.
            self.entry_view(ui);
        } else if self.show_settings {
            egui::Panel::left("side")
                .exact_size(236.0)
                .frame(egui::Frame::new().fill(p.panel_bg).stroke(Stroke::new(1.0, p.border_soft)).inner_margin(egui::Margin::symmetric(16, 16)))
                .show(ui, |ui| self.settings_nav(ui));
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(p.page_bg).inner_margin(egui::Margin::symmetric(36, 26)))
                .show(ui, |ui| self.settings_page(ui));
        } else {
            egui::Panel::left("side")
                .exact_size(236.0)
                .frame(egui::Frame::new().fill(p.panel_bg).stroke(Stroke::new(1.0, p.border_soft)).inner_margin(egui::Margin::symmetric(16, 16)))
                .show(ui, |ui| self.side_menu(ui));
            if self.insights.open && !self.search_state.showing(&self.search) {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(p.page_bg).inner_margin(egui::Margin::symmetric(32, 26)))
                    .show(ui, |ui| self.insights_page(ui));
            } else if self.show_pinned && !self.search_state.showing(&self.search) {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(p.page_bg).inner_margin(egui::Margin::symmetric(32, 26)))
                    .show(ui, |ui| self.pinned_page(ui));
            } else {
                self.insights.open = false;
                self.show_pinned = false;
                egui::Panel::right("day")
                    .default_size(340.0)
                    .size_range(300.0..=460.0)
                    .frame(egui::Frame::new().fill(p.panel_bg).stroke(Stroke::new(1.0, p.border_soft)).inner_margin(egui::Margin::symmetric(24, 22)))
                    .show(ui, |ui| self.day_panel(ui));
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(p.page_bg).inner_margin(egui::Margin::symmetric(28, 24)))
                    .show(ui, |ui| self.calendar(ui));
            }
        }

        self.categories_window(&ctx);
        self.password_window(&ctx);
        self.location_window(&ctx);
        self.shortcuts_window(&ctx);
        self.privacy_window(&ctx);
        self.ai_setup_window(&ctx);
        self.ai_task_window(&ctx);
        self.templates_window(&ctx);
        self.recovery_window(&ctx);
        self.sync_windows(&ctx);
        self.trash_window(&ctx);
        self.backups_window(&ctx);
        self.lightbox_window(&ctx);

        self.export_window(&ctx);
    }
}

fn main() -> eframe::Result {
    if demo::requested() {
        demo::redirect_data_dir();
    }
    let icon = egui::IconData { rgba: icon::rgba(256), width: 256, height: 256 };
    // The window always opens at its normal size; "maximised" and "full screen" are applied on the
    // first frame (see `apply_start_mode`). Asking for them here is overridden by the initial size.
    let viewport = egui::ViewportBuilder::default()
        .with_icon(std::sync::Arc::new(icon))
        .with_title("Tideline Journal")
        .with_decorations(false)
        .with_inner_size([1280.0, 760.0])
        .with_min_inner_size([1200.0, 680.0]);
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "Tideline Journal",
        options,
        Box::new(|cc| {
            theme::set_accent(theme::starting_accent(location::accent()));
            setup_style(&cc.egui_ctx);
            Ok(Box::new(DiaryApp::new()))
        }),
    )
}

fn popup_shadow(dark: bool) -> egui::Shadow {
    egui::Shadow { offset: [0, 10], blur: 32, spread: 4, color: Color32::from_black_alpha(if dark { 220 } else { 150 }) }
}

/// Window frame for popups: same panel colour as the main window, square corners, dark shadow.
fn popup_frame(p: &Palette, dark: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(p.panel_bg)
        .stroke(Stroke::new(1.0, p.cell_border))
        .corner_radius(14)
        .shadow(popup_shadow(dark))
}

/// Title row of a pop-up window, with a hairline under it. Returns true when the close button is clicked.
fn popup_header(ui: &mut egui::Ui, p: &Palette, title: &str) -> bool {
    let mut close = false;
    egui::Frame::new().inner_margin(egui::Margin { left: 22, right: 14, top: 14, bottom: 12 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).font(FontId::new(25.0, fonts::serif_medium())).color(p.title));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 8.0, theme::NAV_ACTIVE);
                }
                kit::paint(ui.painter(), rect.center(), 18.0, if resp.hovered() { theme::TEXT } else { theme::TEXT_2 }, kit::Icon::Close, false);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Close").clicked() {
                    close = true;
                }
            });
        });
    });
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0.0, p.border_soft);
    close
}

/// Derive a key for a new password, write the encrypted file, and return the unlocked vault.
fn create_vault(password: &str, confirm: &str, data: Data) -> Result<(vault::Vault, Data), String> {
    check_new_password(password, confirm)?;
    let v = vault::Vault::create(password)?;
    let mut data = data;
    assign_ids(&mut data, &v);
    save(&data, &v)?;
    Ok((v, data))
}

fn password_field<'a>(text: &'a mut String, hint: &str, p: &Palette) -> egui::TextEdit<'a> {
    egui::TextEdit::singleline(text)
        .password(true)
        .hint_text(hint)
        .font(FontId::proportional(18.0))
        .margin(egui::Margin::symmetric(12, 8))
        .text_color(p.ink)
        .desired_width(f32::INFINITY)
}

fn pressed_enter(r: &egui::Response, ui: &egui::Ui) -> bool {
    r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

fn category_color(cats: &[Category], name: &str) -> Color32 {
    rgb(cats.iter().find(|c| c.name == name).map_or(GREY, |c| c.color))
}

/// Apply a toolbar format to the current selection of the entry text box.
fn format_selection(ctx: &egui::Context, id: egui::Id, text: &mut String, format: markdown::Format) {
    use egui::text::{CCursor, CCursorRange};
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    let end = text.chars().count();
    let (a, b) = state
        .cursor
        .char_range()
        .map_or((end, end), |r| (usize::from(r.primary.index), usize::from(r.secondary.index)));
    let mut sel = (a.min(b), a.max(b));
    if matches!(format, markdown::Format::Bold | markdown::Format::Italic | markdown::Format::Strike) {
        sel = markdown::tighten_selection(sel, &live::hidden_marks(text));
    }
    let (s, e) = markdown::apply(text, sel, format);
    state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(s), CCursor::new(e))));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

const MAX_IMAGE_SIDE: u32 = 1600;

/// Read an image file, shrink it if it is large, store it in the diary data and return the
/// Markdown that shows it.
fn import_image(data: &mut Data, path: &std::path::Path) -> Result<String, String> {
    use base64::Engine;
    let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    let mut img = image::load_from_memory(&bytes).map_err(|e| format!("Couldn't open that image: {e}"))?;
    if img.width() > MAX_IMAGE_SIDE || img.height() > MAX_IMAGE_SIDE {
        img = img.resize(MAX_IMAGE_SIDE, MAX_IMAGE_SIDE, image::imageops::FilterType::Lanczos3);
    }
    // Keep transparency as PNG; photos and everything else as a compact JPEG.
    let transparent = img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p[3] < 255);
    let mut out = Vec::new();
    if transparent {
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    } else {
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85);
        img.to_rgb8().write_with_encoder(enc).map_err(|e| e.to_string())?;
    }

    // A random suffix, so two devices adding a picture in the same moment never choose the same name.
    let mut salt = [0u8; 4];
    getrandom::fill(&mut salt).map_err(|e| e.to_string())?;
    let mut id = format!("i{}{:08x}", chrono::Utc::now().timestamp_millis(), u32::from_be_bytes(salt));
    while data.images.contains_key(&id) {
        id.push('x');
    }
    data.images.insert(id.clone(), base64::engine::general_purpose::STANDARD.encode(out));
    let name = path.file_stem().map(|s| s.to_string_lossy().replace(['[', ']', '(', ')'], "")).unwrap_or_default();
    Ok(format!("![{name}]({}{id})", markdown::IMAGE_SCHEME))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png(path: &std::path::Path, alpha: u8) {
        let img = image::RgbaImage::from_pixel(8, 6, image::Rgba([200, 30, 30, alpha]));
        img.save(path).unwrap();
    }

    /// A diary written by a newer app (extra fields at every level) must come back out unchanged.
    #[test]
    fn unknown_fields_survive_a_save() {
        let original = serde_json::json!({
            "schema": 3, "diary_id": "abc",
            "categories": [{"name": "Work", "color": [1, 2, 3], "uid": "c1"}],
            "entries": [{
                "id": 1, "date": "2026-10-01", "added_at": "2026-10-01 08:00:00", "category": "Work",
                "text": "hi", "uid": "e1", "clock": {"text": "0001-0000-aa"},
                "attachments": [{"id": "f1", "name": "a.txt", "size": 1, "uid": "a1"}]
            }],
            "trash": [{"entry": {"id": 2, "date": "2026-10-02", "added_at": "2026-10-02 08:00:00",
                "category": "Work", "text": "x", "uid": "e2"}, "deleted_at": "2026-10-03 09:00:00", "tomb": true}],
            "templates": [{"name": "T", "body": "b", "uid": "t1"}],
            "settings": {"auto_lock_minutes": 5, "phone_only_setting": true},
            "sync": {"cursor": 7}
        });
        let data: Data = serde_json::from_value(original.clone()).unwrap();
        let again = serde_json::to_value(&data).unwrap();
        for key in ["schema", "diary_id", "sync"] {
            assert_eq!(again[key], original[key], "{key}");
        }
        assert_eq!(again["categories"][0]["uid"], "c1");
        assert_eq!(again["entries"][0]["uid"], "e1");
        assert_eq!(again["entries"][0]["clock"], original["entries"][0]["clock"]);
        assert_eq!(again["entries"][0]["attachments"][0]["uid"], "a1");
        assert_eq!(again["trash"][0]["tomb"], true);
        assert_eq!(again["trash"][0]["entry"]["uid"], "e2");
        assert_eq!(again["templates"][0]["uid"], "t1");
        assert_eq!(again["settings"]["phone_only_setting"], true);
        assert_eq!(again["settings"]["auto_lock_minutes"], 5);
    }

    /// The desktop and the shared core must give an old diary exactly the same ids and fields, or two
    /// devices holding copies would disagree about which entry is which.
    #[test]
    fn upgrading_an_old_diary_matches_the_shared_core() {
        let old = serde_json::json!({
            "categories": [{"name": "Work", "color": [1, 2, 3]}],
            "entries": [{"id": 4, "date": "2026-10-01", "added_at": "2026-10-01 08:00:00", "category": "Work", "text": "a"},
                        {"id": 7, "date": "2026-10-02", "added_at": "2026-10-02 08:00:00", "category": "Work", "text": "b", "mystery": 1}],
            "trash": [{"entry": {"id": 9, "date": "2026-10-03", "added_at": "2026-10-03 08:00:00", "category": "Work", "text": "c"}, "deleted_at": "2026-10-04 09:00:00"}],
            "templates": [{"name": "Mine", "body": "x"}],
            "settings": {"daily_word_goal": 10}
        });
        let v = vault::Vault::create("correct horse").unwrap();
        let mut data: Data = serde_json::from_value(old.clone()).unwrap();
        assert!(needs_upgrade(&data));
        assert!(assign_ids(&mut data, &v));
        assert!(!needs_upgrade(&data));
        let mut core = old;
        assert!(tideline_core::schema::upgrade(&mut core, &tideline_core::schema::diary_id(&v)));
        // The desktop also writes its default fields, so compare what upgrading adds.
        let mine = serde_json::to_value(&data).unwrap();
        for key in ["schema", "diary_id"] {
            assert_eq!(mine[key], core[key], "{key}");
        }
        let pairs = |d: &serde_json::Value| -> Vec<(serde_json::Value, serde_json::Value)> {
            let mut all: Vec<&serde_json::Value> = d["categories"].as_array().unwrap().iter().chain(d["entries"].as_array().unwrap()).chain(d["templates"].as_array().unwrap()).collect();
            all.push(&d["trash"][0]["entry"]);
            all.iter().map(|r| (r["uid"].clone(), r["legacy_id"].clone())).collect()
        };
        assert_eq!(pairs(&mine), pairs(&core));
        assert_eq!(mine["entries"][1]["mystery"], 1);
        // A second pass changes nothing, and a new entry gets a fresh uid.
        assert!(!assign_ids(&mut data, &v));
        data.entries.push(Entry { id: 10, ..Default::default() });
        assert!(assign_ids(&mut data, &v));
        let mine = serde_json::to_value(&data).unwrap();
        assert!(tideline_core::ids::is_uid(mine["entries"][2]["uid"].as_str().unwrap()));
        assert_eq!(mine["entries"][0]["uid"], core["entries"][0]["uid"]);
    }

    /// `TIDELINE_TEST_FILE=<path> TIDELINE_TEST_PW=<password> cargo test --release real_diary -- --ignored`:
    /// reads a real diary (never writes it) and checks the desktop upgrade against the shared core.
    #[test]
    #[ignore]
    fn real_diary_upgrades_like_the_core() {
        let (Ok(src), Ok(pw)) = (std::env::var("TIDELINE_TEST_FILE"), std::env::var("TIDELINE_TEST_PW")) else { return };
        let env = vault::Envelope::parse(&std::fs::read_to_string(src).unwrap()).unwrap();
        let (v, plain) = vault::Vault::unlock(&env, &pw).unwrap();
        let original: serde_json::Value = serde_json::from_slice(&plain).unwrap();
        let mut data: Data = serde_json::from_slice(&plain).unwrap();
        // An already-upgraded diary (the usual case now) has nothing left to do.
        let was_old = needs_upgrade(&data);
        assert_eq!(assign_ids(&mut data, &v), was_old);
        let mine = serde_json::to_value(&data).unwrap();
        let mut core = original.clone();
        tideline_core::schema::upgrade(&mut core, &tideline_core::schema::diary_id(&v));
        assert_eq!(mine["diary_id"], core["diary_id"]);
        for key in ["entries", "categories"] {
            let (a, b) = (mine[key].as_array().unwrap(), core[key].as_array().unwrap());
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(b) {
                assert_eq!(x["uid"], y["uid"], "{key}");
            }
        }
        // Nothing the person wrote is lost or changed by a round trip through the desktop's types.
        for (x, o) in mine["entries"].as_array().unwrap().iter().zip(original["entries"].as_array().unwrap()) {
            for (k, val) in o.as_object().unwrap() {
                assert_eq!(&x[k], val, "field {k}");
            }
        }
        assert_eq!(mine["images"], original["images"]);
        println!("OK: {} entries", data.entries.len());
    }

    #[test]
    fn a_diary_from_a_newer_program_is_refused() {
        let newer: Data = serde_json::from_value(serde_json::json!({"schema": 99, "categories": [], "entries": []})).unwrap();
        assert!(check_schema(&newer).is_err());
        let current: Data = serde_json::from_value(serde_json::json!({"schema": 3, "categories": [], "entries": []})).unwrap();
        assert!(check_schema(&current).is_ok());
    }

    #[test]
    fn imports_images_and_drops_unused_ones() {
        let dir = std::env::temp_dir().join(format!("diary-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (opaque, clear) = (dir.join("my (photo).png"), dir.join("clear.png"));
        tiny_png(&opaque, 255);
        tiny_png(&clear, 100);

        let mut data = fresh_data();
        let md1 = import_image(&mut data, &opaque).unwrap();
        let md2 = import_image(&mut data, &clear).unwrap();
        assert!(md1.starts_with("![my photo](img:i"), "{md1}");
        assert_eq!(data.images.len(), 2);
        assert_eq!(markdown::image_ids(&md1).len(), 1);
        assert!(import_image(&mut data, &dir.join("missing.png")).is_err());
        assert!(md2.contains("img:"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn renders_rich_text_with_an_image_without_panicking() {
        let dir = std::env::temp_dir().join(format!("diary-render-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        tiny_png(&dir.join("a.png"), 255);
        let mut data = fresh_data();
        let img = import_image(&mut data, &dir.join("a.png")).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        let md = format!(
            "# Title\n\nSome **bold**, *italic*, ~~gone~~ and `code`.\n\n- one\n- two\n\n1. a\n2. b\n\n> quoted\n\n{img}\n\n```\nlet x = 1;\n```\n\n---\n"
        );
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut media = markdown::Media::default();
        // The second frame is the first one that uses the fonts set up by `setup_style`.
        for _ in 0..3 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let style = markdown::Style { text: Color32::BLACK, link: Color32::BLUE, size: 16.0, serif: false };
                markdown::render(ui, &md, &style, &mut media, &data.images);
            });
            out.textures_delta.clear();
        }
    }

    fn editor_state(text: &str) -> EditState {
        EditState {
            id: None,
            date: NaiveDate::from_ymd_opt(2026, 10, 9).unwrap(),
            hour: 9,
            minute: 30,
            second: 0,
            category: "Personal".into(),
            title: String::new(),
            text: text.into(),
            baseline: String::new(),
            discard_arm: false,
            mood: None,
            pinned: false,
            sensitive: false,
            attachments: Vec::new(),
            ai: false,
            focus: 0,
            prompt_offset: 0,
            want_focus: None,
        }
    }

    fn set_cursor(ctx: &egui::Context, id: egui::Id, a: usize, b: usize) {
        use egui::text::{CCursor, CCursorRange};
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
        state.store(ctx, id);
    }

    #[test]
    fn toolbar_formats_the_selection_of_the_focused_block() {
        let ctx = egui::Context::default();
        let mut ed = editor_state("hello world");
        set_cursor(&ctx, seg_id(0), 6, 11);
        apply_format(&ctx, &mut ed, markdown::Format::Bold);
        assert_eq!(ed.text, "hello **world**");
        // The selection now covers the word, so applying it again removes the format.
        apply_format(&ctx, &mut ed, markdown::Format::Bold);
        assert_eq!(ed.text, "hello world");
    }

    #[test]
    fn pictures_are_inserted_at_the_cursor_between_text_blocks() {
        let ctx = egui::Context::default();
        let mut ed = editor_state("hello world");
        set_cursor(&ctx, seg_id(0), 5, 5);
        insert_image(&ctx, &mut ed, "![p](img:i9)");
        assert_eq!(ed.text, "hello\n![p](img:i9)\n world");
        assert_eq!(ed.want_focus, Some((2, 0)));
        let segs = live::split(&ed.text);
        assert_eq!(segs.len(), 3);
        assert!(matches!(&segs[1], live::Seg::Image { id, .. } if id == "i9"));
    }

    #[test]
    fn live_layout_works_inside_a_text_edit() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let st = live::LiveStyle { ink: Color32::BLACK, muted: Color32::GRAY, accent: Color32::BLUE, size: 16.0, serif: false, line_height: 1.0 };
        let mut text = "# Title **b** `c`\n- item *i*\n\nplain".to_string();
        for _ in 0..3 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let mut layouter = |ui: &egui::Ui, t: &dyn egui::TextBuffer, wrap: f32| {
                    ui.fonts_mut(|f| f.layout_job(live::layout(t.as_str(), None, false, &st, wrap)))
                };
                let shown = egui::TextEdit::multiline(&mut text).layouter(&mut layouter).show(ui);
                // The bullet is drawn as a bullet, one character for one character.
                assert_eq!(shown.galley.text(), "# Title **b** `c`\n\u{2022} item *i*\n\nplain");
                assert_eq!(shown.galley.text().chars().count(), text.chars().count());
            });
            out.textures_delta.clear();
        }
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn week_helpers() {
        // 9 October 2026 is a Friday.
        assert_eq!(week_start(d(2026, 10, 9)), d(2026, 10, 5));
        assert_eq!(week_start(d(2026, 10, 5)), d(2026, 10, 5));
        assert_eq!(week_start(d(2026, 10, 11)), d(2026, 10, 5));
        assert_eq!(week_title(d(2026, 10, 9)), "5 – 11 October 2026");
        assert_eq!(week_title(d(2026, 9, 30)), "28 Sep – 4 Oct 2026");
        assert_eq!(week_title(d(2026, 12, 31)), "28 Dec 2026 – 3 Jan 2027");
    }

    #[test]
    fn moving_an_entry_keeps_its_time_unless_an_hour_is_given() {
        assert_eq!(entry_time("2026-10-08 16:11:27"), Some((16, 11, 27)));
        assert_eq!(entry_time("garbage"), None);
        assert_eq!(moved_timestamp("2026-10-08 16:11:27", d(2026, 10, 12), None), "2026-10-12 16:11:27");
        assert_eq!(moved_timestamp("2026-10-08 16:11:27", d(2026, 10, 8), Some(9)), "2026-10-08 09:11:27");
        assert_eq!(moved_timestamp("bad", d(2026, 1, 2), None), "2026-01-02 00:00:00");
    }

    fn app_with_entries() -> DiaryApp {
        let mut app = DiaryApp::new();
        app.lock = None;
        app.data = fresh_data();
        app.selected = d(2026, 10, 9);
        app.month = d(2026, 10, 1);
        for (id, date, time, text) in [
            (1, "2026-10-09", "09:15:00", "# Morning **run**"),
            (2, "2026-10-09", "09:40:30", "Coffee with Sam"),
            (3, "2026-10-09", "21:05:00", "Evening notes\n\nmore"),
            (4, "2026-10-12", "12:00:00", "Next week"),
        ] {
            app.data.entries.push(Entry {
                id,
                date: date.into(),
                added_at: format!("{date} {time}"),
                category: "Work".into(),
                text: text.into(),
                mood: None,
                ..Default::default()
            });
        }
        app.data.entries[0].mood = Some(4);
        app.data.entries[2].mood = Some(2);
        app.data.settings.daily_word_goal = 5;
        app
    }

    #[test]
    fn entries_move_between_days_and_hours() {
        let mut app = app_with_entries();
        app.move_entry(1, d(2026, 10, 14), None);
        let e = app.data.entries.iter().find(|e| e.id == 1).unwrap();
        assert_eq!((e.date.as_str(), e.added_at.as_str()), ("2026-10-14", "2026-10-14 09:15:00"));
        assert_eq!(app.selected, d(2026, 10, 14));

        app.move_entry(2, d(2026, 10, 9), Some(17));
        let e = app.data.entries.iter().find(|e| e.id == 2).unwrap();
        assert_eq!((e.date.as_str(), e.added_at.as_str()), ("2026-10-09", "2026-10-09 17:40:30"));
        assert!(app.day_entries(d(2026, 10, 9)).iter().any(|e| e.id == 2));
        assert!(app.day_entries(d(2026, 10, 14)).iter().any(|e| e.id == 1));
    }


    #[test]
    fn dragging_a_pill_to_another_day_moves_the_entry() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut app = app_with_entries();
        app.view = CalView::Month;
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(1200.0, 800.0));
        let (mut time, root) = (0.0f64, std::cell::Cell::new(None));
        let mut frame = |app: &mut DiaryApp, events: Vec<egui::Event>| {
            time += 0.1;
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(time), events, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                root.set(Some(ui.id()));
                app.calendar(ui);
            });
            out.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);

        let root = root.get().unwrap();
        let center = |ctx: &egui::Context, id: egui::Id| ctx.read_response(id).expect("widget was laid out").rect.center();
        let from = center(&ctx, root.with(("pill", 1u64)));
        let to = center(&ctx, root.with(("cell", d(2026, 10, 14))));
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };

        frame(&mut app, vec![egui::Event::PointerMoved(from)]);
        frame(&mut app, vec![button(from, true)]);
        frame(&mut app, vec![egui::Event::PointerMoved(from + vec2(30.0, 30.0))]);
        assert_eq!(app.drag, Some(1), "the drag should have started");
        frame(&mut app, vec![egui::Event::PointerMoved(to)]);
        frame(&mut app, vec![button(to, false)]);
        frame(&mut app, vec![]);

        assert_eq!(app.drag, None);
        let e = app.data.entries.iter().find(|e| e.id == 1).unwrap();
        assert_eq!(e.date, "2026-10-14");
        assert_eq!(e.added_at, "2026-10-14 09:15:00");
    }

    fn press(app: &mut DiaryApp, key: egui::Key, modifiers: egui::Modifiers) {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }],
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| app.handle_shortcuts(ui.ctx()));
        out.textures_delta.clear();
    }

    #[test]
    fn keyboard_shortcuts() {
        let ctrl = egui::Modifiers::COMMAND;
        let mut app = app_with_entries();
        app.view = CalView::Month;

        press(&mut app, egui::Key::Num2, ctrl);
        assert_eq!(app.view, CalView::Week);
        press(&mut app, egui::Key::Num4, ctrl);
        assert_eq!(app.view, CalView::Year);
        press(&mut app, egui::Key::Num1, ctrl);
        assert_eq!(app.view, CalView::Day);

        let before = app.selected;
        press(&mut app, egui::Key::ArrowRight, egui::Modifiers::NONE);
        assert_eq!(app.selected, before + chrono::Days::new(1));
        press(&mut app, egui::Key::ArrowLeft, egui::Modifiers::NONE);
        assert_eq!(app.selected, before);

        press(&mut app, egui::Key::T, ctrl);
        assert_eq!(app.selected, Local::now().date_naive());

        assert!(app.editor.is_none());
        press(&mut app, egui::Key::N, ctrl);
        assert!(app.editor.is_some(), "Ctrl+N starts a new entry");
        // While writing, Ctrl+S asks for a save and Ctrl+N does not start another entry.
        app.editor.as_mut().unwrap().text = "hello".into();
        press(&mut app, egui::Key::S, ctrl);
        assert!(app.save_requested);

        press(&mut app, egui::Key::L, ctrl);
        assert!(app.lock.is_some(), "Ctrl+L locks");
    }

    #[test]
    fn export_ranges_and_counts() {
        let app = app_with_entries(); // selected 2026-10-09 (a Friday); entries on the 9th (x3) and 12th
        let (from, to, _, slug) = app.export_range(ExportScope::Month);
        assert_eq!((from, to, slug.as_str()), (d(2026, 10, 1), d(2026, 10, 31), "2026-10"));
        assert_eq!(app.entries_in(from, to).len(), 4);
        let (from, to, _, slug) = app.export_range(ExportScope::Week);
        assert_eq!((from, to, slug.as_str()), (d(2026, 10, 5), d(2026, 10, 11), "week-2026-10-05"));
        assert_eq!(app.entries_in(from, to).len(), 3);
        let (from, to, ..) = app.export_range(ExportScope::Day);
        assert_eq!(app.entries_in(from, to).len(), 3);
        let (from, to, ..) = app.export_range(ExportScope::Year);
        assert_eq!((from, to), (d(2026, 1, 1), d(2026, 12, 31)));
        let (from, to, ..) = app.export_range(ExportScope::All);
        assert_eq!(app.entries_in(from, to).len(), 4);
        // And the real PDF builder accepts what the window would give it.
        let entries = app.entries_in(d(2026, 10, 1), d(2026, 10, 31));
        let report = export::Report { title: "T".into(), subtitle: "S".into(), entries, include_images: true };
        assert!(export::build_pdf(&report, &app.data.categories, &app.data.images).unwrap().starts_with(b"%PDF"));
    }

    #[test]
    fn diaries_from_earlier_versions_still_load() {
        // No pinned / private / attachments / files / zoom / privacy settings in this file.
        let old = r#"{
            "categories": [{"name": "Personal", "color": [96, 165, 250]}],
            "entries": [{"id": 1, "date": "2026-10-08", "added_at": "2026-10-08 16:11:27", "category": "Personal", "text": "hi"}],
            "settings": {"auto_lock_minutes": 5, "daily_word_goal": 100}
        }"#;
        let data: Data = serde_json::from_str(old).unwrap();
        let e = &data.entries[0];
        assert!(!e.pinned && !e.sensitive && e.attachments.is_empty() && e.mood.is_none());
        assert!(data.files.is_empty() && data.images.is_empty());
        assert_eq!((data.settings.auto_lock_minutes, data.settings.daily_word_goal), (5, 100));
        assert_eq!(data.settings.ui_scale, 1.0);
        assert!(data.settings.privacy_screen, "the privacy screen is on unless turned off");
    }

    #[test]
    fn private_entries_are_masked_in_lists() {
        let mut e = Entry { text: "# Secret plans".into(), ..Default::default() };
        assert_eq!(entry_label(&e), "Secret plans");
        e.sensitive = true;
        assert_eq!(entry_label(&e), "Private entry");
        assert_eq!(human_size(10), "10 B");
        assert_eq!(human_size(2048), "2 KB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0 MB");
    }

    #[test]
    fn attachments_are_stored_limited_and_cleaned_up() {
        let dir = std::env::temp_dir().join(format!("diary-att-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("notes.txt");
        std::fs::write(&file, "hello attachment").unwrap();

        let mut app = app_with_entries();
        let att = import_attachment(&mut app.data, &file).unwrap();
        assert_eq!((att.name.as_str(), att.size), ("notes.txt", 16));
        assert_eq!(app.attachment_bytes(&att).unwrap(), b"hello attachment");
        assert!(import_attachment(&mut app.data, &dir.join("missing.bin")).is_err());

        // An attachment on no entry is dropped when the diary is next saved; one in use is kept.
        let id = att.id.clone();
        app.persist();
        assert!(!app.data.files.contains_key(&id), "unused files are dropped");
        let att = import_attachment(&mut app.data, &file).unwrap();
        app.data.entries[0].attachments.push(att.clone());
        app.persist();
        assert!(app.data.files.contains_key(&att.id), "files in use are kept");
        assert!(is_image_path(std::path::Path::new("a/b/Photo.JPG")) && !is_image_path(std::path::Path::new("notes.txt")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn programs_and_scripts_are_never_opened_from_the_diary() {
        for risky in ["setup.exe", "RUN.BAT", "notes.PS1", "x.vbs", "app.msi", "link.lnk", "page.hta", "tool.jar", "a.b.c.cmd", "invoice.pdf.exe", "sneaky.exe.", "sneaky.exe ", "sneaky.exe. .", "driver.sys", "macro.js", "shortcut.url"] {
            assert!(is_risky_file(risky), "{risky} should be treated as risky");
        }
        for fine in ["notes.txt", "photo.JPG", "report.pdf", "archive.zip", "sheet.xlsx", "README", "exe", "my.exe.txt", ".bashrc", "song.mp3", "scriptures.docx", "bat"] {
            assert!(!is_risky_file(fine), "{fine} should be fine");
        }

        // Even if the Open button were reached some other way, nothing is written or launched.
        let mut app = app_with_entries();
        let file = std::env::temp_dir().join(format!("diary-risky-{}.exe", std::process::id()));
        std::fs::write(&file, "not really a program").unwrap();
        let att = import_attachment(&mut app.data, &file).unwrap();
        assert!(app.attachment_bytes(&att).is_some(), "it is still stored and can be saved as a copy");
        let _ = std::fs::remove_dir_all(open_files_dir());
        app.open_attachment(&att);
        assert!(app.status.contains("never opened"), "{}", app.status);
        assert!(!open_files_dir().exists(), "no temporary copy is made");
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn text_size_steps_and_is_remembered() {
        let mut app = app_with_entries();
        assert_eq!(app.data.settings.ui_scale, 1.0);
        app.step_zoom(1);
        assert_eq!(app.data.settings.ui_scale, 1.1);
        app.step_zoom(1);
        app.step_zoom(1);
        assert_eq!(app.data.settings.ui_scale, 1.4);
        for _ in 0..10 {
            app.step_zoom(1);
        }
        assert_eq!(app.data.settings.ui_scale, 1.6, "stops at the largest size");
        for _ in 0..10 {
            app.step_zoom(-1);
        }
        assert_eq!(app.data.settings.ui_scale, 0.8, "and at the smallest");
    }

    #[test]
    fn random_entry_skips_private_ones() {
        let mut app = app_with_entries();
        for e in &mut app.data.entries {
            e.sensitive = true;
        }
        app.data.entries[3].sensitive = false; // the entry on 2026-10-12
        let today = Local::now().date_naive();
        for _ in 0..20 {
            app.selected = today;
            app.random_entry();
            assert_eq!(app.selected, d(2026, 10, 12), "only the one public entry can be picked");
        }
        app.data.entries[3].sensitive = true;
        app.random_entry();
        assert!(app.status.contains("no entries"), "{}", app.status);
    }

    #[test]
    fn photos_are_listed_newest_first_and_private_ones_flagged() {
        assert_eq!(
            markdown::images("a ![one](img:i1) b\n\n![two words](img:i2) ![web](http://x/y.png)"),
            vec![("i1".to_string(), "one".to_string()), ("i2".to_string(), "two words".to_string())]
        );
        let images = BTreeMap::from([("i1".to_string(), "x".to_string()), ("i2".to_string(), "x".to_string())]);
        let entries = vec![
            Entry { id: 1, date: "2026-10-01".into(), added_at: "2026-10-01 09:00:00".into(), text: "![old](img:i1)".into(), ..Default::default() },
            Entry { id: 2, date: "2026-10-05".into(), added_at: "2026-10-05 09:00:00".into(), text: "![new](img:i2) ![gone](img:i9)".into(), sensitive: true, ..Default::default() },
        ];
        let photos = gallery::collect_photos(&entries, &images);
        assert_eq!(photos.iter().map(|p| (p.id.as_str(), p.private)).collect::<Vec<_>>(), vec![("i2", true), ("i1", false)]);
        assert_eq!(photos[0].caption, "new");
    }

    #[test]
    fn cards_gallery_and_viewer_render_with_every_kind_of_entry() {
        let dir = std::env::temp_dir().join(format!("diary-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        tiny_png(&dir.join("a.png"), 255);
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut app = app_with_entries();
        let picture = import_image(&mut app.data, &dir.join("a.png")).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        app.data.entries[0].text = format!("Hello {picture}");
        app.data.entries[0].pinned = true;
        app.data.entries[1].sensitive = true;
        app.data.entries[2].attachments.push(Attachment { id: "f1".into(), name: "a.pdf".into(), size: 1500, extra: Default::default() });
        app.rev += 1;

        for view in [CalView::Month, CalView::Day, CalView::Photos] {
            app.view = view;
            for _ in 0..3 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    app.calendar(ui);
                    app.day_panel(ui);
                });
                out.textures_delta.clear();
            }
        }
        // Private entries open when clicked; the viewer and the pinned list render too.
        app.revealed.insert(app.data.entries[1].id);
        app.lightbox = Some(0);
        app.show_pinned = true;
        for _ in 0..3 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                app.day_panel(ui);
                app.lightbox_window(ui.ctx());
                app.pinned_page(ui);
            });
            out.textures_delta.clear();
        }
        assert_eq!(app.lightbox, Some(0), "the viewer stays open");
    }

    #[test]
    fn insights_page_renders_every_tab_range_and_period() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut app = app_with_entries();
        for (i, e) in app.data.entries.iter_mut().enumerate() {
            e.mood = Some(1 + (i as u8 % 5));
        }
        app.insights.open = true;
        for tab in [insights::Tab::Mood, insights::Tab::Writing, insights::Tab::Review] {
            for range in 0..4 {
                for period in 0..6 {
                    app.insights.tab = tab;
                    app.insights.range = range;
                    app.insights.period = period;
                    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                        egui::CentralPanel::default().show(ui, |ui| app.insights_page(ui));
                    });
                    out.textures_delta.clear();
                }
            }
        }
        assert!(app.insights.open);
        app.insights.clear();
        assert!(!app.insights.open);
    }

    #[test]
    fn a_review_can_be_saved_as_a_pdf() {
        let dir = std::env::temp_dir().join(format!("diary-review-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        tiny_png(&dir.join("a.png"), 255);
        let mut app = app_with_entries();
        let picture = import_image(&mut app.data, &dir.join("a.png")).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        app.data.entries[0].text = format!("# A good morning\n\nWent for a **long** run by the river. {picture}");
        app.data.entries[0].mood = Some(5);
        app.data.entries[1].mood = Some(2);
        app.data.entries[0].pinned = true;
        let per_day = stats::per_day(&app.data.entries);
        let review = reports::review(&app.data.entries, &per_day, &app.data.images, reports::Period::ThisMonth, d(2026, 10, 9), 5);
        assert_eq!(review.summary.entries, 4);
        assert_eq!(review.pictures.len(), 1);
        for include_images in [true, false] {
            let pdf = export::build_review_pdf(&review, &app.data.categories, &app.data.images, include_images).unwrap();
            assert!(pdf.starts_with(b"%PDF") && pdf.len() > 2000);
            if let (Some(dir), true) = (std::env::var_os("DIARY_PDF_OUT"), include_images) {
                std::fs::write(std::path::Path::new(&dir).join("review.pdf"), &pdf).unwrap();
            }
        }
    }

    #[test]
    fn exports_leave_out_private_entries_unless_asked() {
        let mut app = app_with_entries();
        app.data.entries[1].sensitive = true;
        let (from, to, ..) = app.export_range(ExportScope::Year);
        assert_eq!(app.entries_in(from, to).len(), 3);
        app.export_private = true;
        assert_eq!(app.entries_in(from, to).len(), 4);
        app.data.entries[0].attachments.push(Attachment { id: "f1".into(), name: "report.pdf".into(), size: 2048, extra: Default::default() });
        let report = export::Report { title: "T".into(), subtitle: "S".into(), entries: app.entries_in(from, to), include_images: false };
        assert!(export::build_pdf(&report, &app.data.categories, &app.data.images).unwrap().starts_with(b"%PDF"));
    }

    #[test]
    fn deleted_entries_wait_in_the_bin_and_can_be_restored() {
        let mut app = app_with_entries();
        let id = app.data.entries[0].id;
        app.trash_entry(id);
        assert!(app.data.entries.iter().all(|e| e.id != id), "gone from the diary");
        assert_eq!(app.data.trash.len(), 1);
        assert_eq!(app.last_trashed, Some(id));
        assert!(app.day_entries(d(2026, 10, 9)).iter().all(|e| e.id != id), "and from the calendar");

        app.restore_trashed(id);
        assert!(app.data.trash.is_empty());
        assert!(app.data.entries.iter().any(|e| e.id == id));
        assert_eq!(app.selected, d(2026, 10, 9), "the day it belongs to is selected");

        // If its id was reused in the meantime, it comes back with a fresh one.
        app.trash_entry(id);
        app.data.entries.push(Entry { id, date: "2026-10-10".into(), text: "new one".into(), ..Default::default() });
        app.restore_trashed(id);
        let ids: std::collections::HashSet<u64> = app.data.entries.iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), app.data.entries.len(), "ids stay unique");
        assert_eq!(app.data.entries.len(), 5);
    }

    #[test]
    fn the_bin_empties_itself_after_thirty_days_and_keeps_pictures_meanwhile() {
        let mut app = app_with_entries();
        app.data.images.insert("i1".into(), "x".into());
        app.data.files.insert("f1".into(), "x".into());
        app.data.entries[1].text = "![p](img:i1)".into();
        app.data.entries[1].attachments.push(Attachment { id: "f1".into(), name: "a.txt".into(), size: 1, extra: Default::default() });
        let id = app.data.entries[1].id;
        app.trash_entry(id);
        // Its picture and attachment are not thrown away while it can still be restored.
        assert!(app.data.images.contains_key("i1") && app.data.files.contains_key("f1"));

        let today = Local::now().date_naive();
        assert_eq!(app.data.trash[0].days_left(today), 30);
        app.data.trash[0].deleted_at = format!("{} 10:00:00", key(today - chrono::Days::new(29)));
        app.purge_trash();
        assert_eq!(app.data.trash.len(), 1, "29 days old is still kept");
        assert_eq!(app.data.trash[0].days_left(today), 1);
        app.data.trash[0].deleted_at = format!("{} 10:00:00", key(today - chrono::Days::new(30)));
        app.purge_trash();
        assert!(app.data.trash.is_empty(), "30 days old is removed");
        app.persist();
        assert!(!app.data.images.contains_key("i1") && !app.data.files.contains_key("f1"), "and then its files go too");
    }

    #[test]
    fn diaries_without_a_bin_load_and_the_bin_round_trips() {
        let data: Data = serde_json::from_str(r#"{"categories": [], "entries": []}"#).unwrap();
        assert!(data.trash.is_empty());
        let mut app = app_with_entries();
        app.trash_entry(app.data.entries[0].id);
        let json = serde_json::to_string(&app.data).unwrap();
        let back: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(back.trash.len(), 1);
        assert_eq!(back.trash[0].entry.text, "# Morning **run**");
    }

    #[test]
    fn lock_screens_recovery_window_and_bin_render() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut with_key = vault::Vault::create("a password").unwrap();
        with_key.set_recovery().unwrap();
        let env_with = vault::Envelope::parse(&with_key.seal(b"{}").unwrap()).unwrap();
        let env_without = vault::Envelope::parse(&vault::Vault::create("a password").unwrap().seal(b"{}").unwrap()).unwrap();

        let mut app = app_with_entries();
        for mode in [
            LockMode::Create,
            LockMode::Join,
            LockMode::Unlock(env_with.clone()),
            LockMode::Recover(env_with),
            LockMode::Recover(env_without),
            LockMode::Broken("test".into()),
        ] {
            app.lock = Some(Lock::new(mode));
            for _ in 0..2 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.lock_screen(ui));
                out.textures_delta.clear();
            }
        }

        // The recovery window in each of its three states, and the bin with something in it.
        app.lock = None;
        app.vault = Some(vault::Vault::create("pw").unwrap());
        app.show_recovery = true;
        app.data.entries.push(Entry { id: 50, date: "2026-10-01".into(), text: "x".into(), ..Default::default() });
        app.trash_entry(50);
        app.show_trash = true;
        let hint_shown_before = app.recovery_hint_dismissed;
        for state in 0..3 {
            match state {
                1 => app.recovery_shown = Some("ABCD-EFGH-JKMN-PQRS-TVWX-YZ01-2345-6789".into()),
                2 => {
                    app.recovery_shown = None;
                    app.vault.as_mut().unwrap().set_recovery().unwrap();
                }
                _ => {}
            }
            for _ in 0..2 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    app.recovery_window(ui.ctx());
                    app.trash_window(ui.ctx());
                    app.day_panel(ui);
                });
                out.textures_delta.clear();
            }
        }
        assert!(app.show_recovery && !hint_shown_before);
    }

    #[test]
    fn templates_fill_the_page_and_a_default_applies_to_new_entries() {
        let mut app = app_with_entries();
        app.new_entry(d(2026, 10, 9));
        assert_eq!(app.editor.as_ref().unwrap().text, "", "no default template: a blank page");

        app.apply_template("Gratitude");
        let text = app.editor.as_ref().unwrap().text.clone();
        assert!(text.starts_with("## Three things I'm grateful for"), "{text}");
        // Using another one on a page that already has text adds it below.
        app.apply_template("Dream log");
        let text = app.editor.as_ref().unwrap().text.clone();
        assert!(text.contains("Three things") && text.contains("## What I remember"));
        app.apply_template("No such template");
        assert_eq!(app.editor.as_ref().unwrap().text, text, "unknown names do nothing");

        // Your own template, with placeholders, used for every new entry.
        app.data.templates.push(templates::Template { name: "Mine".into(), body: "# {weekday} {time}\n\n".into(), extra: Default::default() });
        app.data.settings.default_template = "Mine".into();
        app.new_entry_at(d(2026, 10, 9), Some(7)); // 7:00 on a Friday
        assert_eq!(app.editor.as_ref().unwrap().text, "# Friday 07:00\n\n");
        assert!(app.template_names().iter().any(|n| n == "Mine") && app.template_names().iter().any(|n| n == "Daily review"));
    }

    #[test]
    fn saving_templates_checks_names_and_keeps_the_draft_when_something_is_wrong() {
        let mut app = app_with_entries();
        let draft = |name: &str, body: &str| templates::Draft { index: None, name: name.into(), body: body.into() };

        for (name, body, expect) in [("", "x", "name"), ("gratitude", "x", "already"), ("New one", "  ", "empty")] {
            app.templates.draft = Some(draft(name, body));
            app.save_template_draft_for_test();
            assert!(app.templates.msg.contains(expect), "{name:?}: {}", app.templates.msg);
            assert!(app.templates.draft.is_some(), "the draft is kept so nothing typed is lost");
        }
        app.templates.draft = Some(draft("Morning", "## Plan\n"));
        app.save_template_draft_for_test();
        assert!(app.templates.draft.is_none() && app.templates.msg.is_empty());
        assert_eq!(app.data.templates.len(), 1);

        // Renaming keeps it as the default; editing in place does not duplicate it.
        app.data.settings.default_template = "Morning".into();
        app.templates.draft = Some(templates::Draft { index: Some(0), name: "Morning pages".into(), body: "## Plan\n".into() });
        app.save_template_draft_for_test();
        assert_eq!((app.data.templates.len(), app.data.settings.default_template.as_str()), (1, "Morning pages"));
    }

    #[test]
    fn the_templates_window_and_prompt_bar_render() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut app = app_with_entries();
        app.data.templates.push(templates::Template { name: "Mine".into(), body: "x".into(), extra: Default::default() });
        app.templates.open = true;
        app.new_entry(d(2026, 10, 9));
        for draft in [None, Some(templates::Draft::default())] {
            app.templates.draft = draft;
            for _ in 0..2 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    app.templates_window(ui.ctx());
                    app.entry_view(ui);
                });
                out.textures_delta.clear();
            }
        }
    }

    #[test]
    fn f11_switches_full_screen_on_and_off_even_on_the_lock_screen() {
        let ctx = egui::Context::default();
        let f11 = |ctx: &egui::Context, app: &mut DiaryApp, fullscreen: bool| {
            let input = egui::RawInput {
                viewports: std::iter::once((
                    egui::ViewportId::ROOT,
                    egui::ViewportInfo { fullscreen: Some(fullscreen), ..Default::default() },
                ))
                .collect(),
                events: vec![egui::Event::Key { key: egui::Key::F11, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }],
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| app.handle_shortcuts(ui.ctx()));
            out.textures_delta.clear();
            out.viewport_output.values().flat_map(|v| v.commands.iter()).filter_map(|c| match c {
                egui::ViewportCommand::Fullscreen(on) => Some(*on),
                _ => None,
            }).collect::<Vec<bool>>()
        };
        let mut app = app_with_entries();
        assert_eq!(f11(&ctx, &mut app, false), vec![true], "from a window to full screen");
        assert_eq!(f11(&ctx, &mut app, true), vec![false], "and back");
        app.lock = Some(Lock::new(LockMode::Create));
        assert_eq!(f11(&ctx, &mut app, false), vec![true], "it works on the lock screen too");
    }

    #[test]
    fn the_search_panel_filters_caches_and_clears() {
        let mut app = app_with_entries();
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let frame = |app: &mut DiaryApp| {
            for _ in 0..2 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.day_panel(ui));
                out.textures_delta.clear();
            }
        };
        assert!(!app.search_state.showing(&app.search), "the day panel shows normally");

        app.search = "coffee".into();
        frame(&mut app);
        assert_eq!(app.search_state.hit_count(), Some(1));
        // Editing an entry changes the diary, so the results are recalculated.
        app.data.entries[0].text = "Coffee again".into();
        app.rev += 1;
        app.search = "coffee".into();
        frame(&mut app);
        assert_eq!(app.search_state.hit_count(), Some(2));

        // Filters alone (no words) list entries too.
        app.search.clear();
        app.search_state.filters.mood = 2;
        frame(&mut app);
        assert_eq!(app.search_state.hit_count(), Some(1));
        app.search_state.filters.period = search::Period::Custom;
        frame(&mut app);

        // Locking forgets the question.
        app.lock_now();
        assert!(app.search.is_empty() && !app.search_state.filters.is_active() && !app.search_state.showing(""));
    }

    #[test]
    fn restoring_a_backup_locks_the_diary_and_the_backups_window_draws() {
        let mut app = app_with_entries();
        let ctx = egui::Context::default();
        setup_style(&ctx);
        app.show_backups = true;
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.backups_window(ui.ctx()));
            out.textures_delta.clear();
        }

        let dir = std::env::temp_dir().join(format!("diary-restore-ui-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("entries.json");
        let write = |pw: &str| {
            let v = vault::Vault::create(pw).unwrap();
            std::fs::write(&file, v.seal(&serde_json::to_vec(&fresh_data()).unwrap()).unwrap()).unwrap();
        };
        write("old");
        let backup = backups::snapshot(&file, "2026-10-01-080000", 10, false).unwrap().unwrap();
        write("new");

        app.vault = Some(vault::Vault::create("new").unwrap());
        app.restore_backup(&file, &backup);
        assert!(app.lock.is_some() && app.vault.is_none() && app.data.entries.is_empty(), "locked, nothing left in memory");
        assert!(!app.show_backups);
        assert!(app.lock.as_ref().unwrap().error.contains("Backup restored"));
        let env = vault::Envelope::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert!(vault::Vault::unlock(&env, "old").is_ok());

        // A bad backup leaves everything alone.
        let mut app = app_with_entries();
        app.restore_backup(&file, &dir.join("missing.json"));
        assert!(app.lock.is_none() && !app.data.entries.is_empty());
        assert!(app.backup_msg.contains("failed"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_writing_view_saves_the_title_with_the_text_and_splits_it_again_on_edit() {
        let mut app = app_with_entries();
        let ctx = egui::Context::default();
        setup_style(&ctx);
        app.new_entry(d(2026, 10, 9));
        {
            let ed = app.editor.as_mut().unwrap();
            ed.title = "A title".into();
            ed.text = "Some body".into();
            assert!(ed.has_content() && ed.changed());
        }
        for _ in 0..3 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.entry_view(ui));
            out.textures_delta.clear();
        }
        app.save_editor();
        assert!(app.editor.is_none());
        let saved = app.data.entries.last().unwrap().clone();
        assert_eq!(saved.text, "# A title\n\nSome body");

        // Editing it again shows the title and the text separately, unchanged until you type.
        app.start_edit(saved.id);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!((ed.title.as_str(), ed.text.as_str()), ("A title", "Some body"));
        assert!(!ed.changed());
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.entry_view(ui));
            out.textures_delta.clear();
        }
        // An entry without a title keeps all its text as the body.
        app.editor = None;
        app.start_edit(2);
        assert_eq!(app.editor.as_ref().unwrap().title, "");
        assert_eq!(app.editor.as_ref().unwrap().text, "Coffee with Sam");
    }

    #[test]
    fn discard_leaves_at_once_when_nothing_changed_and_asks_once_otherwise() {
        let mut app = app_with_entries();
        app.new_entry(d(2026, 10, 9));
        assert!(app.editor.as_mut().unwrap().press_discard(), "an empty page goes straight away");

        app.new_entry(d(2026, 10, 9));
        let ed = app.editor.as_mut().unwrap();
        ed.text = "half a thought".into();
        assert!(!ed.press_discard(), "the first press only arms it");
        assert!(ed.discard_arm);
        assert!(ed.press_discard(), "the second press discards");

        // Changing an existing entry counts as a change; opening it does not.
        app.start_edit(1);
        let ed = app.editor.as_mut().unwrap();
        assert!(!ed.changed());
        ed.title = "New heading".into();
        assert!(ed.changed());
    }

    #[test]
    fn the_new_chrome_renders_and_a_blank_day_says_so() {
        let mut app = app_with_entries();
        let ctx = egui::Context::default();
        setup_style(&ctx);
        app.data.entries.push(Entry { id: 9, date: "2026-10-09".into(), added_at: "2026-10-09 22:00:00".into(), category: "Work".into(), text: "# Secret\n\nhidden".into(), sensitive: true, ..Default::default() });
        for day in [d(2026, 10, 9), d(2026, 10, 20)] {
            app.selected = day;
            for view in [CalView::Month, CalView::Week, CalView::Day, CalView::Year, CalView::Photos] {
                app.view = view;
                for _ in 0..2 {
                    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                        egui::Panel::top("t").show(ui, |ui| app.title_bar(ui));
                        egui::Panel::left("l").show(ui, |ui| app.side_menu(ui));
                        egui::Panel::right("r").show(ui, |ui| app.day_panel(ui));
                        egui::CentralPanel::default().show(ui, |ui| app.calendar(ui));
                    });
                    out.textures_delta.clear();
                }
            }
        }
        // The same with a search showing instead of the day.
        app.search = "coffee".into();
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::Panel::top("t").show(ui, |ui| app.title_bar(ui));
                egui::Panel::right("r").show(ui, |ui| app.day_panel(ui));
            });
            out.textures_delta.clear();
        }
    }


    #[test]
    fn the_pinned_page_filters_sorts_unpins_and_opens_days() {
        let mut app = app_with_entries();
        let ctx = egui::Context::default();
        setup_style(&ctx);
        app.data.entries[0].pinned = true; // Work, 2026-10-09
        app.data.entries[3].pinned = true; // Work, 2026-10-12
        app.data.entries[1].pinned = true;
        app.data.entries[1].category = "Health".into();
        app.data.entries.push(Entry { id: 20, date: "2026-10-10".into(), added_at: "2026-10-10 08:00:00".into(), category: "Ideas".into(), text: "Secret".into(), sensitive: true, pinned: true, ..Default::default() });
        app.show_pinned = true;
        let show = |app: &mut DiaryApp| {
            for _ in 0..2 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    egui::CentralPanel::default().show(ui, |ui| app.pinned_page(ui));
                });
                out.textures_delta.clear();
            }
        };
        for sort in 0..pinned::SORTS.len() {
            app.pinned_sort = sort;
            show(&mut app);
        }
        app.pinned_category = Some("Work".into());
        show(&mut app);
        // A filter for a category that no longer has pins goes back to All.
        app.pinned_category = Some("Personal".into());
        show(&mut app);
        assert_eq!(app.pinned_category, None);
        // Cards can open to show everything, pictures included, and fold again.
        app.data.images.insert("pic".into(), {
            use base64::Engine;
            let img = image::RgbaImage::from_pixel(6, 6, image::Rgba([10, 120, 200, 255]));
            let mut bytes = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
            base64::engine::general_purpose::STANDARD.encode(bytes)
        });
        app.data.entries[0].text = "# Pinned with a picture

Some words.

![blue](img:pic)

More words.".into();
        app.pinned_category = None;
        for open in [true, false] {
            if open {
                app.expanded.insert(app.data.entries[0].id);
            } else {
                app.expanded.clear();
            }
            show(&mut app);
        }
        // Private pinned entries render without their text (covered by `hidden`) and stay pinned.
        assert!(app.data.entries.iter().any(|e| e.id == 20 && e.pinned));
    }

    #[test]
    fn ctrl_d_pins_the_entry_being_written() {
        let mut app = app_with_entries();
        let ctx = egui::Context::default();
        setup_style(&ctx);
        app.new_entry(d(2026, 10, 9));
        assert!(!app.editor.as_ref().unwrap().pinned);
        for _ in 0..2 {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::Key { key: egui::Key::D, physical_key: Some(egui::Key::D), pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
            let mut out = ctx.run_ui(input, |ui| app.entry_view(ui));
            out.textures_delta.clear();
            break;
        }
        assert!(app.editor.as_ref().unwrap().pinned, "Ctrl+D pins");
    }

    #[test]
    fn day_cards_can_show_the_whole_entry_including_pictures() {
        let mut app = app_with_entries();
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let png = {
            use base64::Engine;
            let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 50, 120, 255]));
            let mut bytes = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
            base64::engine::general_purpose::STANDARD.encode(bytes)
        };
        app.data.images.insert("pic1".into(), png);
        app.data.entries[1].text = "# With a picture\n\nLine one.\n\n![A pink square](img:pic1)\n\n- a list\n- of things".into();
        let id = app.data.entries[1].id;
        for open in [false, true, false] {
            if open {
                app.expanded.insert(id);
            } else {
                app.expanded.remove(&id);
            }
            for _ in 0..2 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.day_panel(ui));
                out.textures_delta.clear();
            }
        }
        app.expanded.insert(id);
        app.lock_now();
        assert!(app.expanded.is_empty(), "locking forgets which cards were open");
    }

    #[test]
    fn the_settings_page_renders_every_section_and_closes() {
        let mut app = app_with_entries();
        let ctx = egui::Context::default();
        setup_style(&ctx);
        app.show_settings = true;
        for jump in [None, Some(0), Some(2), Some(4)] {
            app.settings_jump = jump;
            for _ in 0..2 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    egui::Panel::left("l").show(ui, |ui| app.settings_nav(ui));
                    egui::CentralPanel::default().show(ui, |ui| app.settings_page(ui));
                });
                out.textures_delta.clear();
            }
        }
        // With backups turned on, the extra "Copies to keep" row shows too.
        app.data.settings.backup_keep = 10;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| app.settings_page(ui));
        });
        out.textures_delta.clear();
        app.lock_now();
        assert!(!app.show_settings, "locking closes Settings");
    }

    /// Run one frame in which `key` is pressed, calling `f` inside it.
    fn press_key_in(ctx: &egui::Context, key: egui::Key, f: &mut dyn FnMut(&egui::Context)) {
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        let mut out = ctx.run_ui(input, |ui| f(ui.ctx()));
        out.textures_delta.clear();
    }

    fn cursor_of(ctx: &egui::Context, id: egui::Id) -> usize {
        usize::from(egui::TextEdit::load_state(ctx, id).unwrap().cursor.char_range().unwrap().primary.index)
    }

    #[test]
    fn the_cursor_steps_over_hidden_marks_and_backspace_works_on_what_you_can_see() {
        use egui::Key;
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let id = seg_id(0);
        let style = live::LiveStyle { ink: Color32::WHITE, muted: Color32::GRAY, accent: Color32::BLUE, size: 16.0, serif: false, line_height: 1.0 };
        let hidden_of = |t: &str| live::layout_with_hidden(t, None, false, &style, 400.0).1;
        let step = |text: &mut String, key: Key, at: usize, ctx: &egui::Context| {
            focus_at(ctx, id, text, at);
            let hidden = hidden_of(text);
            let mut changed = false;
            press_key_in(ctx, key, &mut |c| changed = editor::step_over_hidden_marks(c, id, text, &hidden));
            changed
        };

        // "a **b** c": hidden runs at 2..4 and 5..7.
        let mut t = "a **b** c".to_string();
        assert!(!step(&mut t, Key::ArrowLeft, 4, &ctx));
        assert_eq!(cursor_of(&ctx, id), 2, "Left from after the opening marks goes to before them");
        step(&mut t, Key::ArrowRight, 2, &ctx);
        assert_eq!(cursor_of(&ctx, id), 4, "Right crosses the opening marks");
        step(&mut t, Key::ArrowRight, 5, &ctx);
        assert_eq!(cursor_of(&ctx, id), 7, "and the closing ones, so you can type after the bold text");
        // Backspace at the start of the bold text deletes the visible space before it.
        assert!(step(&mut t, Key::Backspace, 4, &ctx));
        assert_eq!(t, "a**b** c");
        assert_eq!(cursor_of(&ctx, id), 1);
        // Delete at the end of the bold text deletes the space after the closing marks.
        let mut t = "a **b** c".to_string();
        assert!(step(&mut t, Key::Delete, 5, &ctx));
        assert_eq!(t, "a **b**c");
        // Away from any marks the keys are left alone.
        let mut t = "a **b** c".to_string();
        assert!(!step(&mut t, Key::Backspace, 1, &ctx));
        assert_eq!(t, "a **b** c");

        // Backspace at the start of a heading turns it back into a paragraph.
        let mut t = "## Title".to_string();
        assert!(step(&mut t, Key::Backspace, 3, &ctx));
        assert_eq!(t, "Title");
        assert_eq!(cursor_of(&ctx, id), 0);

        // A click that lands inside a run of marks is moved to the end of it.
        let t = "a **b** c".to_string();
        focus_at(&ctx, id, &t, 3);
        editor::snap_out_of_marks(&ctx, id, &t, &hidden_of(&t));
        assert_eq!(cursor_of(&ctx, id), 4);
        // With multi-byte text the positions still line up.
        let mut t = "é **ü** ö".to_string();
        step(&mut t, Key::ArrowRight, 2, &ctx);
        assert_eq!(cursor_of(&ctx, id), 4);
    }

    #[test]
    fn prompts_go_in_at_the_cursor_one_after_another() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let id = seg_id(0);
        // Empty page.
        let mut ed = editor_state("");
        DiaryApp::insert_prompt_at_cursor(&ctx, &mut ed, "What went well?");
        assert_eq!(ed.text, "## What went well?\n\n");
        assert_eq!(ed.want_focus, Some((0, 20)));
        assert_eq!(ed.prompt_offset, 1);
        // Below some writing, with the cursor at the end: a blank line first.
        let mut ed = editor_state("Morning notes.");
        focus_at(&ctx, id, &ed.text, 14);
        DiaryApp::insert_prompt_at_cursor(&ctx, &mut ed, "Q1");
        assert_eq!(ed.text, "Morning notes.\n\n## Q1\n\n");
        // In the middle of a line the text after the cursor moves below the prompt.
        let mut ed = editor_state("one two");
        focus_at(&ctx, id, &ed.text, 4);
        DiaryApp::insert_prompt_at_cursor(&ctx, &mut ed, "Q2");
        assert_eq!(ed.text, "one \n\n## Q2\n\ntwo");
        assert_eq!(ed.want_focus, Some((0, 13)));
        // A second prompt after the first one, with writing between.
        let mut ed = editor_state("## Q1\n\nAnswer one.\n");
        focus_at(&ctx, id, &ed.text, 19);
        DiaryApp::insert_prompt_at_cursor(&ctx, &mut ed, "Q2");
        assert_eq!(ed.text, "## Q1\n\nAnswer one.\n\n## Q2\n\n");
        assert_eq!(ed.prompt_offset, 1);
        // Pictures are left alone: it goes in the text block that has the cursor.
        let mut ed = editor_state("before\n![pic](img:abc)\nafter");
        ed.focus = 2;
        DiaryApp::insert_prompt_at_cursor(&ctx, &mut ed, "Q3");
        assert!(ed.text.contains("![pic](img:abc)") && ed.text.contains("## Q3"));
        assert!(ed.text.starts_with("before\n![pic](img:abc)\nafter"), "{:?}", ed.text);
    }

    #[test]
    fn the_bold_button_toggles_bold_on_and_off_in_the_editor() {
        use egui::text::{CCursor, CCursorRange};
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let id = seg_id(0);
        let select = |a: usize, b: usize| {
            let mut state = egui::TextEdit::load_state(&ctx, id).unwrap_or_default();
            state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
            state.store(&ctx, id);
        };
        // The steps from the bug report: type, select it, Bold, select it again, Bold again.
        for second in [(2, 7), (0, 9), (1, 8), (9, 0)] {
            let mut ed = editor_state("hello world");
            select(0, 5);
            apply_format(&ctx, &mut ed, markdown::Format::Bold);
            assert_eq!(ed.text, "**hello** world");
            select(second.0, second.1);
            apply_format(&ctx, &mut ed, markdown::Format::Bold);
            assert_eq!(ed.text, "hello world", "second selection {second:?}");
        }
        // Bold, then italic on the same word, then each off again.
        let mut ed = editor_state("hello world");
        select(0, 5);
        apply_format(&ctx, &mut ed, markdown::Format::Bold);
        select(0, 9);
        apply_format(&ctx, &mut ed, markdown::Format::Italic);
        assert_eq!(ed.text, "***hello*** world");
        select(0, 11);
        apply_format(&ctx, &mut ed, markdown::Format::Italic);
        assert_eq!(ed.text, "**hello** world");
    }

    #[test]
    fn opened_attachments_are_kept_beside_the_diary_not_on_the_host_pc() {
        let dir = open_files_dir();
        assert!(dir.starts_with(location::data_dir()), "{}", dir.display());
        assert_eq!(dir.file_name().unwrap(), "opened-attachments-temporary");
        assert!(!dir.starts_with(std::env::temp_dir().join("TidelineJournal-open")));
    }

    #[test]
    fn the_user_guide_is_a_real_pdf() {
        assert!(USER_GUIDE.starts_with(b"%PDF-"), "the embedded guide is a PDF");
        assert!(USER_GUIDE.len() > 100_000, "the guide has its pictures");
        let doc = lopdf::Document::load_mem(USER_GUIDE).expect("the guide parses");
        assert!(doc.get_pages().len() > 10, "the guide has many pages");
    }

    #[test]
    fn the_first_save_creates_the_data_folder() {
        let base = std::env::temp_dir().join(format!("diary-first-save-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        // Neither the folder nor its parent exist yet: a newly unzipped portable copy.
        let file = base.join("Tideline Journal").join("data").join("entries.json");
        let v = vault::Vault::create("pw").unwrap();
        write_diary(&file, &fresh_data(), &v).unwrap();
        let env = vault::Envelope::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert!(vault::Vault::unlock(&env, "pw").is_ok());
        assert!(!file.with_extension("json.tmp").exists(), "no temporary file is left behind");
        // Saving again over it is fine too.
        write_diary(&file, &fresh_data(), &v).unwrap();
        std::fs::remove_dir_all(&base).ok();
    }
    #[test]
    fn choosing_a_colour_theme_recolours_the_app_and_is_remembered() {
        let _turn = location::CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut app = app_with_entries();
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let before = palette(&ctx).accent;
        let btn_before = accent_fill();

        let rose = theme::PRESETS[3].1;
        app.set_colour_theme(&ctx, rose);
        assert_ne!(palette(&ctx).accent, before, "the accent changes colour");
        assert_ne!(accent_fill(), btn_before, "buttons change colour");
        assert_eq!(ctx.global_style().visuals.selection.stroke.color, accent_fill(), "selection and focus colours follow too");
        assert_eq!(location::accent(), Some(rose), "remembered for the lock screen next time");
        assert_eq!(heat_color(4, true), accent_fill(), "the heat-map uses the accent");
        assert_eq!(heat_color(0, true), theme::CELL);

        // Choosing the same one again changes nothing; going back to the original clears the saved colour.
        app.set_colour_theme(&ctx, rose);
        assert!(app.status.is_empty() || !app.status.contains("failed"));
        app.set_colour_theme(&ctx, theme::DEFAULT);
        assert_eq!(location::accent(), None);
        assert_eq!(palette(&ctx).accent, before);
        assert_eq!(accent_fill(), btn_before);
    }

    #[test]
    fn the_start_mode_is_applied_once_on_the_first_frame() {
        let commands = |app: &mut DiaryApp| -> Vec<egui::ViewportCommand> {
            let ctx = egui::Context::default();
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.apply_start_mode(ui.ctx()));
            out.textures_delta.clear();
            out.viewport_output
                .values()
                .flat_map(|v| v.commands.clone())
                .filter(|c| matches!(c, egui::ViewportCommand::Maximized(_) | egui::ViewportCommand::Fullscreen(_)))
                .collect()
        };
        for (mode, expect) in [
            (location::StartMode::Normal, None),
            (location::StartMode::Maximized, Some(egui::ViewportCommand::Maximized(true))),
            (location::StartMode::Fullscreen, Some(egui::ViewportCommand::Fullscreen(true))),
        ] {
            let mut app = app_with_entries();
            app.start_mode = mode;
            app.start_applied = false;
            assert_eq!(commands(&mut app), expect.into_iter().collect::<Vec<_>>(), "{mode:?}: first frame");
            assert!(commands(&mut app).is_empty(), "{mode:?}: later frames do nothing, so F11 and un-maximising still work");
        }
    }
    #[test]
    fn all_calendar_views_render() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut app = app_with_entries();
        for view in [CalView::Month, CalView::Week, CalView::Day, CalView::Year, CalView::Month] {
            app.view = view;
            for _ in 0..3 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.calendar(ui));
                out.textures_delta.clear();
            }
        }
        // Navigation follows the view.
        app.view = CalView::Week;
        app.shift_days(7);
        assert_eq!(app.selected, d(2026, 10, 16));
        app.view = CalView::Day;
        app.shift_days(-1);
        assert_eq!(app.selected, d(2026, 10, 15));
        assert_eq!(app.calendar_title(), "Thursday 15 October 2026");
    }
}

fn seg_id(i: usize) -> egui::Id {
    egui::Id::new(("entry_seg", i))
}

/// The text block toolbar actions apply to: the one that last had the cursor.
fn focused_text_seg(segs: &[live::Seg], focus: usize) -> usize {
    if matches!(segs.get(focus), Some(live::Seg::Text(_))) {
        focus
    } else {
        segs.iter().rposition(|s| matches!(s, live::Seg::Text(_))).unwrap_or(0)
    }
}

fn apply_format(ctx: &egui::Context, ed: &mut EditState, format: markdown::Format) {
    let mut segs = live::split(&ed.text);
    let idx = focused_text_seg(&segs, ed.focus);
    if let live::Seg::Text(s) = &mut segs[idx] {
        format_selection(ctx, seg_id(idx), s, format);
    }
    ed.text = live::join(&segs);
}

/// Put the cursor at char `pos` (clamped) in a text block and give it keyboard focus.
fn focus_at(ctx: &egui::Context, id: egui::Id, text: &str, pos: usize) {
    use egui::text::{CCursor, CCursorRange};
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(pos.min(text.chars().count())))));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// Insert a picture (`![alt](img:ID)`) at the cursor, splitting the text block around it.
fn insert_image(ctx: &egui::Context, ed: &mut EditState, md: &str) {
    let Some(picture) = live::split(md).into_iter().find(|s| matches!(s, live::Seg::Image { .. })) else {
        return;
    };
    let mut segs = live::split(&ed.text);
    let idx = focused_text_seg(&segs, ed.focus);
    let chars: Vec<char> = match &segs[idx] {
        live::Seg::Text(s) => s.chars().collect(),
        _ => Vec::new(),
    };
    let at = egui::TextEdit::load_state(ctx, seg_id(idx))
        .and_then(|state| state.cursor.char_range())
        .map_or(chars.len(), |r| usize::from(r.primary.index).max(usize::from(r.secondary.index)))
        .min(chars.len());
    let before: String = chars[..at].iter().collect();
    let after: String = chars[at..].iter().collect();
    segs.splice(idx..=idx, [live::Seg::Text(before), picture, live::Seg::Text(after)]);
    ed.text = live::join(&segs);
    ed.want_focus = Some((idx + 2, 0));
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CalView {
    Month,
    Week,
    Year,
    Photos,
    Day,
}

/// What the user did on the calendar this frame.
#[derive(Default)]
struct CalEvents {
    select: Option<NaiveDate>,
    /// Double-click on empty space: new entry on this day (and hour, in the day view).
    new_on: Option<(NaiveDate, Option<u32>)>,
    edit: Option<u64>,
    /// An entry was released here; the view works out which day/hour that is.
    drop_at: Option<(u64, egui::Pos2)>,
    /// Move this entry to this day (and hour).
    drop: Option<(u64, NaiveDate, Option<u32>)>,
}

fn week_start(d: NaiveDate) -> NaiveDate {
    d - chrono::Days::new(u64::from(d.weekday().num_days_from_monday()))
}

fn week_title(d: NaiveDate) -> String {
    let (mon, sun) = (week_start(d), week_start(d) + chrono::Days::new(6));
    if mon.month() == sun.month() && mon.year() == sun.year() {
        format!("{} – {} {}", mon.day(), sun.day(), sun.format("%B %Y"))
    } else if mon.year() == sun.year() {
        format!("{} {} – {} {}", mon.day(), mon.format("%b"), sun.day(), sun.format("%b %Y"))
    } else {
        format!("{} {} – {} {}", mon.day(), mon.format("%b %Y"), sun.day(), sun.format("%b %Y"))
    }
}

/// (hour, minute, second) from an `added_at` timestamp.
fn entry_time(added_at: &str) -> Option<(u32, u32, u32)> {
    let mut parts = added_at.get(11..)?.split(':').map(|p| p.parse::<u32>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

/// The timestamp for an entry moved to `date` (and `hour`, keeping its minutes and seconds).
fn moved_timestamp(added_at: &str, date: NaiveDate, hour: Option<u32>) -> String {
    let (h, m, s) = entry_time(added_at).unwrap_or((0, 0, 0));
    format!("{} {:02}:{:02}:{:02}", key(date), hour.unwrap_or(h).min(23), m, s)
}

/// Cut `text` with an ellipsis so it fits in `max_w`.
fn elide(painter: &egui::Painter, text: &str, font: &FontId, max_w: f32) -> String {
    let width = |s: &str| painter.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x;
    if width(text) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let s: String = chars[..mid].iter().collect::<String>() + "…";
        if width(&s) <= max_w { lo = mid } else { hi = mid - 1 }
    }
    chars[..lo].iter().collect::<String>().trim_end().to_string() + "…"
}

/// One entry on the calendar: a small tinted chip (category dot, time, title) that can be clicked,
/// double-clicked (edit) or dragged to another day.
#[allow(clippy::too_many_arguments)]
fn pill(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    e: &Entry,
    cats: &[Category],
    size: f32,
    drag: &mut Option<u64>,
    ev: &mut CalEvents,
) {
    let c = category_color(cats, &e.category);
    let resp = ui.interact(rect, ui.id().with(("pill", e.id)), Sense::click_and_drag());
    let being_dragged = *drag == Some(e.id);
    let label = format!("{} {}", e.added_at.get(11..16).unwrap_or(""), entry_label(e));
    let font = FontId::proportional(size);
    let base = theme::rgb_of(theme::CELL);
    let tinted = |a: f32| {
        let m = theme::mix(theme::rgb_of(c), base, a);
        Color32::from_rgb(m[0], m[1], m[2])
    };

    let hovered = resp.hovered() && !being_dragged;
    painter.rect_filled(rect, 7.0, if being_dragged { tinted(0.05) } else { tinted(if hovered { 0.22 } else { 0.13 }) });
    if hovered {
        painter.rect_stroke(rect, 7.0, Stroke::new(1.0, c.gamma_multiply(0.7)), egui::StrokeKind::Inside);
    }
    painter.circle_filled(egui::pos2(rect.min.x + 11.0, rect.center().y), 3.5, if being_dragged { c.gamma_multiply(0.4) } else { c });
    let right_pad = if e.pinned { 24.0 } else { 8.0 };
    let max_w = rect.width() - 20.0 - right_pad;
    let text = elide(painter, &label, &font, max_w);
    painter.text(egui::pos2(rect.min.x + 20.0, rect.center().y), Align2::LEFT_CENTER, text, font.clone(), if being_dragged { theme::TEXT_3 } else { theme::TEXT_2 });
    if resp.hovered() || being_dragged {
        ui.ctx().set_cursor_icon(if being_dragged { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
    }

    if e.pinned {
        kit::paint(painter, egui::pos2(rect.max.x - 13.0, rect.center().y), 12.0, theme::TODAY, kit::Icon::Star, true);
    }
    if resp.drag_started() {
        *drag = Some(e.id);
    }
    if resp.dragged() && *drag == Some(e.id) {
        // A copy of the chip follows the pointer.
        if let Some(pos) = ui.ctx().pointer_latest_pos() {
            let gp = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("drag_ghost")));
            let r = egui::Rect::from_min_size(pos + vec2(12.0, 6.0), vec2(rect.width().min(300.0), rect.height()));
            gp.rect_filled(r.translate(vec2(2.0, 3.0)), 7.0, Color32::from_black_alpha(110));
            gp.rect(r, 7.0, theme::SURFACE, Stroke::new(1.0, c), egui::StrokeKind::Inside);
            gp.circle_filled(egui::pos2(r.min.x + 11.0, r.center().y), 3.5, c);
            let text = elide(&gp, &label, &font, r.width() - 30.0);
            gp.text(egui::pos2(r.min.x + 22.0, r.center().y), Align2::LEFT_CENTER, text, font.clone(), theme::TEXT);
        }
    }
    if resp.drag_stopped() && drag.take() == Some(e.id) {
        if let Some(pos) = ui.ctx().pointer_latest_pos() {
            ev.drop_at = Some((e.id, pos));
        }
    }
    if resp.double_clicked() {
        ev.edit = Some(e.id);
    } else if resp.clicked() {
        ev.select = NaiveDate::parse_from_str(&e.date, "%Y-%m-%d").ok();
    }
}

/// What the lock screen should ask for, judging by the diary file on disk.
fn current_lock_mode() -> LockMode {
    match inspect() {
        Disk::Missing => LockMode::Create,
        Disk::Encrypted(env) => LockMode::Unlock(env),
        Disk::Plain(data) => LockMode::Encrypt(data),
        Disk::Unreadable(why) => LockMode::Broken(why),
    }
}

const MOOD_LABELS: [&str; 5] = theme::MOOD_NAMES;

/// Red (1) through amber to green (5); fractional values blend.
fn mood_color(value: f32) -> Color32 {
    let v = (value.clamp(1.0, 5.0) - 1.0).min(3.999);
    let (i, t) = (v.floor() as usize, v.fract());
    let (a, b) = (theme::rgb_of(theme::MOODS[i]), theme::rgb_of(theme::MOODS[i + 1]));
    let m = theme::mix(b, a, t);
    Color32::from_rgb(m[0], m[1], m[2])
}

/// A small coloured circle holding the mood number.
fn mood_badge(ui: &mut egui::Ui, mood: u8) {
    let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
    kit::paint_face(ui.painter(), rect.center(), 20.0, theme::MOODS[usize::from(mood.clamp(1, 5)) - 1], mood);
    resp.on_hover_text(format!("Mood: {}", theme::MOOD_NAMES[usize::from(mood.clamp(1, 5)) - 1]));
}

/// Heat-map shade for a level from 0 (nothing written) to 4 (a lot).
fn heat_color(level: u8, _dark: bool) -> Color32 {
    theme::shades().heat[usize::from(level.min(4))]
}

/// Daily mood dots with a 7-day average line, over one year.
fn mood_chart(ui: &mut egui::Ui, p: &Palette, series: &[(NaiveDate, f32, f32)], year: i32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 190.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let plot = egui::Rect::from_min_max(rect.min + vec2(34.0, 8.0), rect.max - vec2(8.0, 24.0));
    painter.rect_filled(plot, 4.0, p.cell);
    painter.rect_stroke(plot, 4.0, Stroke::new(1.0, p.cell_border), egui::StrokeKind::Inside);

    let y_of = |v: f32| plot.max.y - (v - 1.0) / 4.0 * plot.height();
    for v in 1..=5 {
        let y = y_of(v as f32);
        painter.line_segment([egui::pos2(plot.min.x, y), egui::pos2(plot.max.x, y)], Stroke::new(1.0, p.cell_border.gamma_multiply(0.5)));
        painter.text(egui::pos2(plot.min.x - 6.0, y), Align2::RIGHT_CENTER, v.to_string(), FontId::proportional(12.0), p.muted);
    }
    let days_in_year = NaiveDate::from_ymd_opt(year, 12, 31).unwrap().ordinal() as f32;
    let x_of = |d: NaiveDate| plot.min.x + (d.ordinal0() as f32 + 0.5) / days_in_year * plot.width();
    for m in 1..=12 {
        let first = NaiveDate::from_ymd_opt(year, m, 1).unwrap();
        painter.text(egui::pos2(x_of(first), plot.max.y + 4.0), Align2::LEFT_TOP, first.format("%b").to_string(), FontId::proportional(12.0), p.muted);
    }
    for (d, m, _) in series {
        painter.circle_filled(egui::pos2(x_of(*d), y_of(*m)), 3.5, mood_color(*m));
    }
    if series.len() >= 2 {
        let points: Vec<egui::Pos2> = series.iter().map(|(d, _, avg)| egui::pos2(x_of(*d), y_of(*avg))).collect();
        painter.add(egui::Shape::line(points, Stroke::new(2.5, p.accent)));
    }
}

fn search_id() -> egui::Id {
    egui::Id::new("search_box")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExportScope {
    Day,
    Week,
    Month,
    Year,
    All,
}

/// A file attached to an entry.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Attachment {
    id: String,
    name: String,
    /// Size in bytes.
    size: u64,
    /// Fields written by a newer version of the app: kept untouched so saving never loses them.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    extra: serde_json::Map<String, serde_json::Value>,
}

const MAX_ATTACHMENT: u64 = 25 * 1024 * 1024;

fn human_size(bytes: u64) -> String {
    match bytes {
        0..=1023 => format!("{bytes} B"),
        1024..=1_048_575 => format!("{:.0} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

fn is_image_path(path: &std::path::Path) -> bool {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    ["png", "jpg", "jpeg", "gif", "bmp", "webp"].contains(&ext.as_str())
}

/// Read a file into the diary's attachment store. The file is kept inside the encrypted diary.
fn import_attachment(data: &mut Data, path: &std::path::Path) -> Result<Attachment, String> {
    use base64::Engine;
    let size = std::fs::metadata(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?.len();
    if size > MAX_ATTACHMENT {
        return Err(format!("That file is {}; attachments can be up to {}.", human_size(size), human_size(MAX_ATTACHMENT)));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    let mut id = format!("f{}", chrono::Utc::now().timestamp_millis());
    while data.files.contains_key(&id) {
        id.push('x');
    }
    data.files.insert(id.clone(), base64::engine::general_purpose::STANDARD.encode(&bytes));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
    Ok(Attachment { id, name, size, extra: Default::default() })
}

/// File types that run code or change the system when Windows opens them. They can be kept in the
/// diary and saved as a copy, but are never opened from here, so a diary someone else made (or a
/// careless click) can't start a program.
const RISKY_EXTENSIONS: [&str; 41] = [
    "exe", "com", "scr", "pif", "bat", "cmd", "ps1", "ps1xml", "psm1", "psd1", "vbs", "vbe", "vb", "js", "jse", "wsf", "wsh", "wsc", "sct", "msi", "msp", "mst", "msc", "lnk", "hta", "jar", "reg", "cpl", "dll",
    "ocx", "sys", "appx", "msix", "appxbundle", "msixbundle", "application", "gadget", "inf", "scf", "url", "chm",
];

const RISKY_HINT: &str = "This is a program or script type, so it can be saved as a copy but is never opened from the diary.";
const RISKY_HINT_SHORT: &str = "is a program or script type, so it can only be saved as a copy, never opened from the diary.";

/// Is this a file that would run code if Windows opened it? Case doesn't matter, and neither do
/// trailing dots or spaces, which Windows ignores (`setup.exe.` is `setup.exe`).
fn is_risky_file(name: &str) -> bool {
    let name = name.trim_end_matches(['.', ' ']);
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    RISKY_EXTENSIONS.contains(&ext.as_str())
}

/// Where attachments are written when they are opened (in the data folder, so a portable copy on a
/// USB drive leaves nothing on the host PC).
fn open_files_dir() -> std::path::PathBuf {
    // Beside the diary, so nothing is left behind on the PC the program is run on. The long
    // name makes sure only this folder is ever cleared.
    location::data_dir().join("opened-attachments-temporary")
}

/// Delete the temporary copies made by "Open".
fn clear_open_files() {
    std::fs::remove_dir_all(open_files_dir()).ok();
}

/// What to show of an entry in lists and on the calendar: its first line, or a placeholder for
/// private entries.
fn entry_label(e: &Entry) -> String {
    if e.sensitive { "Private entry".into() } else { markdown::summary(&e.text) }
}

/// Switch the whole-screen mode on or off.
fn toggle_fullscreen(ctx: &egui::Context) {
    let on = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!on));
}
