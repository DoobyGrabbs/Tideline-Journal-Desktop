//! Keeping this diary in step with your other devices through the sync server (see the core's
//! `docs/SYNC_DESIGN.md`).
//!
//! Everything is encrypted here before it leaves the PC; the server only ever holds ciphertext. The work
//! happens in three places:
//!
//! * **On every save** ([`DiaryApp::sync_stamp`]) the changes you made are stamped with this device's clock, so
//!   later they can be merged field by field with changes made elsewhere.
//! * **On a background thread** ([`job`]) a copy of the diary is synced with the server. The diary you are
//!   working in is never held up, and a dropped connection just leaves the job unfinished.
//! * **When the job finishes** ([`DiaryApp::sync_finish`]) its result is merged into the live diary, which
//!   also keeps anything you changed while it ran. Text edited on two devices is kept as a separate entry.

use super::*;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use tideline_core::hlc::Clock;
use tideline_core::sync::{self as core_sync, Poll, Report, Request, Response, Sync, SyncState, ValueReplica};
use tideline_core::{device, merge};

/// How long after the last change the diary is synced, and how often it is synced regardless (so changes from other devices turn up quickly).
const AFTER_EDIT: Duration = Duration::from_secs(5);
const EVERY: Duration = Duration::from_secs(30);
/// After a failure (offline, the server is off) wait this long before trying again.
const RETRY: Duration = Duration::from_secs(60);

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Status {
    /// Sync is not set up on this PC.
    Off,
    /// Set up; `Some(time)` is when it last finished.
    Idle(Option<String>),
    Running,
    Failed(String),
}

/// Everything the app keeps about syncing while the diary is unlocked.
pub(crate) struct Runtime {
    pub status: Status,
    /// What this PC knows about the server: its address, the access token and how far it has read.
    pub state: SyncState,
    clock: Option<Clock>,
    /// The diary's records as of the last stamping, to tell what has changed since.
    baseline: serde_json::Value,
    /// Where the diary file is, which names the files kept beside it.
    path: std::path::PathBuf,
    running: Option<Receiver<Outcome>>,
    /// The diary changed since a sync started.
    dirty: bool,
    last_change: Option<Instant>,
    last_started: Option<Instant>,
    retry_at: Option<Instant>,
    /// Text typed in Settings.
    pub server_text: String,
    pub notice: String,
    pub show_code: bool,
    pub show_qr: bool,
    /// The Rename diary and Sync code windows are open; "Turn off" is waiting for a second click.
    pub rename_window: bool,
    pub code_window: bool,
    pub confirm_off: bool,
    /// "Change code" was clicked once and is waiting for a second click to confirm.
    pub confirm_change: bool,
    /// The code typed in "Use a new code".
    pub new_code_text: String,
    /// The name typed in Settings (for a new setup, or to rename).
    pub name_text: String,
    /// What the state was before "Use a new code", to go back to if the new one is refused.
    previous: Option<SyncState>,
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime {
            status: Status::Off,
            state: SyncState::default(),
            clock: None,
            baseline: serde_json::Value::Null,
            path: std::path::PathBuf::new(),
            running: None,
            dirty: false,
            last_change: None,
            last_started: None,
            retry_at: None,
            server_text: String::new(),
            notice: String::new(),
            show_code: false,
            show_qr: false,
            rename_window: false,
            code_window: false,
            confirm_off: false,
            confirm_change: false,
            new_code_text: String::new(),
            name_text: String::new(),
            previous: None,
        }
    }
}

impl Runtime {
    pub(crate) fn configured(&self) -> bool {
        !self.state.server.is_empty() && !self.state.token.is_empty()
    }
}

/// What a background sync hands back.
pub(crate) struct Outcome {
    result: Result<Report, String>,
    state: SyncState,
    data: serde_json::Value,
}

/// A plain address typed by a person, tidied: `192.168.1.20:8787` becomes `http://192.168.1.20:8787`.
pub(crate) fn clean_server(text: &str) -> String {
    let t = text.trim().trim_end_matches('/');
    if t.is_empty() {
        String::new()
    } else if t.starts_with("http://") || t.starts_with("https://") {
        t.to_string()
    } else {
        format!("http://{t}")
    }
}

/// Carry one request to the server.
pub(crate) fn http(base: &str, req: &Request) -> Result<Response, String> {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    let agent = AGENT.get_or_init(|| ureq::Agent::config_builder().http_status_as_error(false).timeout_connect(Some(Duration::from_secs(5))).timeout_global(Some(Duration::from_secs(300))).build().into());
    let mut builder = ureq::http::Request::builder().method(req.method).uri(format!("{base}{}", req.path));
    for (k, v) in &req.headers {
        builder = builder.header(k, v);
    }
    let request = builder.body(req.body.clone()).map_err(|_| "That server address doesn't look right.".to_string())?;
    let unreachable = || format!("Couldn't reach the sync server at {base}. Is it running, and is this PC on the same network?");
    let mut response = agent.run(request).map_err(|e| match e {
        ureq::Error::Io(_) | ureq::Error::ConnectionFailed | ureq::Error::HostNotFound | ureq::Error::Timeout(_) => unreachable(),
        ureq::Error::BadUri(_) => "That server address doesn't look right.".to_string(),
        other => format!("Couldn't talk to the sync server: {other}"),
    })?;
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_vec().map_err(|_| unreachable())?;
    Ok(Response { status, body })
}

/// Run one sync to the end, over HTTP. The state comes back whether or not it finished.
fn run_machine(base: &str, state: SyncState, replica: &mut ValueReplica) -> (SyncState, Result<Report, String>) {
    let mut machine = Sync::new(state);
    let mut answer: Option<Response> = None;
    loop {
        match machine.poll(replica, answer.take()) {
            Err(e) => return (machine.into_state(), Err(e)),
            Ok(Poll::Done(report)) => return (machine.into_state(), Ok(report)),
            Ok(Poll::Request(req)) => match http(base, &req) {
                Ok(reply) => answer = Some(reply),
                Err(e) => return (machine.into_state(), Err(e)),
            },
        }
    }
}

/// The work of a background sync. With `register`, the diary is first put on the server under the token.
pub(crate) fn job(state: SyncState, mut replica: ValueReplica, register: bool) -> Outcome {
    let base = state.server.clone();
    if register {
        let registered = http(&base, &core_sync::register_request_named(&replica, &state.token, &state.name)).and_then(|r| core_sync::registered(&r));
        if let Err(e) = registered {
            return Outcome { result: Err(e), state, data: replica.data };
        }
    }
    let (state, result) = run_machine(&base, state, &mut replica);
    Outcome { result, state, data: replica.data }
}

fn names(device: u32) -> String {
    format!("device {device:08x}")
}

impl DiaryApp {
    /// The records of the diary as plain JSON, without the pictures: what stamping looks at.
    fn record_view(&self) -> serde_json::Value {
        fn to<T: Serialize>(t: &T) -> serde_json::Value {
            serde_json::to_value(t).unwrap_or(serde_json::Value::Null)
        }
        let mut view = serde_json::json!({
            "schema": self.data.extra.get("schema").cloned().unwrap_or(serde_json::Value::Null),
            "diary_id": self.data.extra.get("diary_id").cloned().unwrap_or(serde_json::Value::Null),
            "categories": to(&self.data.categories),
            "entries": to(&self.data.entries),
            "templates": to(&self.data.templates),
            "trash": to(&self.data.trash),
            "settings": to(&self.data.settings),
        });
        view["tombstones"] = self.data.extra.get("tombstones").cloned().unwrap_or_else(|| serde_json::json!([]));
        view
    }

    fn diary_id(&self) -> String {
        self.data.extra.get("diary_id").and_then(|v| v.as_str()).unwrap_or("").to_string()
    }

    /// The diary has just been unlocked (or made): set up this device's clock and load what it knows about
    /// the server. `path` is where the diary file is.
    pub(crate) fn sync_unlocked(&mut self, path: &std::path::Path) {
        let Some(vault) = self.vault.as_ref() else { return };
        let id = self.diary_id();
        let dev = device::load(path, &id, vault);
        let state = core_sync::load_state(path, &id, vault);
        let server_text = state.server.clone();
        self.sync = Runtime { clock: Some(Clock::new(dev.id, dev.last)), baseline: merge::snapshot(&self.record_view()), path: path.to_path_buf(), state, server_text, ..Runtime::default() };
        self.sync.status = if self.sync.configured() { Status::Idle(None) } else { Status::Off };
    }

    /// The diary is locked: forget the clock and anything still running (its result is thrown away).
    pub(crate) fn sync_locked(&mut self) {
        self.sync = Runtime::default();
    }

    /// Stamp what has changed since the last time with this device's clock. Called on every save.
    pub(crate) fn sync_stamp(&mut self) {
        let Some(mut clock) = self.sync.clock.take() else { return };
        let mut view = self.record_view();
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        if merge::stamp_local_changes(&mut view, &self.sync.baseline, &mut clock, now) {
            self.write_back(&view);
            self.sync.dirty = true;
            self.sync.last_change = Some(Instant::now());
        }
        self.sync.baseline = merge::snapshot(&view);
        if let Some(vault) = self.vault.as_ref() {
            device::save(&self.sync.path, &self.diary_id(), vault, &device::Device { id: clock.device(), last: clock.last() }).ok();
        }
        self.sync.clock = Some(clock);
    }

    /// Put stamped records back into the diary's own types.
    fn write_back(&mut self, view: &serde_json::Value) {
        fn take<T: serde::de::DeserializeOwned>(view: &serde_json::Value, key: &str, into: &mut T) {
            if let Ok(v) = serde_json::from_value::<T>(view[key].clone()) {
                *into = v;
            }
        }
        take(view, "categories", &mut self.data.categories);
        take(view, "entries", &mut self.data.entries);
        take(view, "templates", &mut self.data.templates);
        take(view, "trash", &mut self.data.trash);
        take(view, "settings", &mut self.data.settings);
        if view["tombstones"].as_array().is_some_and(|a| !a.is_empty()) {
            self.data.extra.insert("tombstones".into(), view["tombstones"].clone());
        }
    }

    /// Start a sync on a background thread, if one isn't running. With `register` the diary is first put on
    /// the server (the very first time).
    pub(crate) fn sync_start(&mut self, ctx: &egui::Context, register: bool) {
        // Only for a diary that was properly unlocked (it has a clock and a place to keep its state).
        if self.sync.running.is_some() || !self.sync.configured() || self.lock.is_some() || self.sync.clock.is_none() || self.diary_id().is_empty() {
            return;
        }
        let Some(vault) = self.vault.clone() else { return };
        self.sync_stamp();
        let replica = ValueReplica { data: serde_json::to_value(&self.data).unwrap_or_default(), vault };
        let state = self.sync.state.clone();
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let outcome = job(state, replica, register);
            let _ = tx.send(outcome);
            ctx.request_repaint();
        });
        self.sync.running = Some(rx);
        self.sync.status = Status::Running;
        self.sync.dirty = false;
        self.sync.last_started = Some(Instant::now());
        self.sync.retry_at = None;
    }

    /// Called every frame while unlocked: collect a finished sync, and start one when it's time.
    pub(crate) fn sync_tick(&mut self, ctx: &egui::Context) {
        if self.lock.is_some() || !self.sync.configured() {
            return;
        }
        if let Some(rx) = &self.sync.running {
            match rx.try_recv() {
                Ok(outcome) => {
                    self.sync.running = None;
                    self.sync_finish(outcome);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(300));
                    return;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.sync.running = None;
                    self.sync.status = Status::Failed("The sync stopped unexpectedly.".into());
                    self.sync.retry_at = Some(Instant::now() + RETRY);
                }
            }
        }
        let now = Instant::now();
        let due = if let Some(retry) = self.sync.retry_at {
            retry
        } else if self.sync.dirty {
            self.sync.last_change.unwrap_or(now) + AFTER_EDIT
        } else {
            self.sync.last_started.map_or(now, |t| t + EVERY)
        };
        if now >= due {
            self.sync_start(ctx, false);
        } else {
            ctx.request_repaint_after(due - now);
        }
    }

    /// A background sync is done: keep what it learned about the server, and merge what it brought into the
    /// diary you are working in.
    pub(crate) fn sync_finish(&mut self, outcome: Outcome) {
        let Outcome { result, state, data } = outcome;
        let id = self.diary_id();
        // A code typed in by the person that the server refused (wrong, or for another diary): go back to the old one.
        let (state, result) = match (self.sync.previous.take(), &result) {
            (Some(before), Err(e)) if e.contains("did not accept") || e.contains("different diary") => (before, result),
            (_, _) => (state, result),
        };
        if let Some(vault) = self.vault.as_ref() {
            core_sync::save_state(&self.sync.path, &id, vault, &state).ok();
        }
        self.sync.state = state;
        let mut copies = 0;
        let mut merge_error = None;
        if let Some(mut clock) = self.sync.clock.take() {
            let local = serde_json::to_value(&self.data).unwrap_or_default();
            let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
            match merge::merge_resolving(&local, &data, &mut clock, now, &names) {
                Ok((merged, _conflicts, made)) => match serde_json::from_value::<Data>(merged) {
                    Ok(new) => {
                        copies = made;
                        let changed = serde_json::to_value(&new).ok() != Some(local);
                        self.data = new;
                        if changed {
                            self.rev += 1;
                            self.stats_cache = None;
                            if let Some(vault) = self.vault.as_ref() {
                                if let Err(e) = save(&self.data, vault) {
                                    merge_error = Some(format!("couldn't save the synced diary ({e})"));
                                }
                            }
                        }
                        self.sync.baseline = merge::snapshot(&self.record_view());
                    }
                    Err(e) => merge_error = Some(format!("couldn't read the synced diary ({e})")),
                },
                Err(e) => merge_error = Some(e),
            }
            if let Some(vault) = self.vault.as_ref() {
                device::save(&self.sync.path, &id, vault, &device::Device { id: clock.device(), last: clock.last() }).ok();
            }
            self.sync.clock = Some(clock);
        }
        let now = Instant::now();
        match (result, merge_error) {
            (_, Some(e)) | (Err(e), None) => {
                self.sync.status = Status::Failed(e);
                self.sync.retry_at = Some(now + RETRY);
            }
            (Ok(report), None) => {
                self.sync.status = Status::Idle(Some(Local::now().format("%H:%M").to_string()));
                self.sync.notice = if report.code_changed {
                    self.sync.show_code = true;
                    self.sync.code_window = true;
                    "The sync code was changed and the old one no longer works. Enter the new code on your other devices: copy it from Sync code here, and enter it under Sync code on the other device.".to_string()
                } else if copies > 0 {
                    format!("{copies} {} edited on two devices at once: the other version was kept as a separate entry, with “conflicting version” in its title.", if copies == 1 { "entry was" } else { "entries were" })
                } else if report.pulled > 0 || report.pushed > 0 {
                    format!("Synced: {} received, {} sent.", report.pulled, report.pushed)
                } else {
                    "Everything is up to date.".to_string()
                };
                // Copies made just now still need sending, and so does anything changed while it ran.
                if copies > 0 || self.sync.dirty {
                    self.sync.retry_at = Some(now + Duration::from_secs(2));
                }
            }
        }
    }

    /// Set up syncing with the server at the address typed in Settings: make a secret token, put the diary on
    /// the server under it, and sync.
    pub(crate) fn sync_set_up(&mut self, ctx: &egui::Context) {
        let server = clean_server(&self.sync.server_text);
        if server.is_empty() {
            self.sync.notice = "Type the address of your sync server first.".into();
            return;
        }
        let typed = core_sync::clean_name(&self.sync.name_text);
        let name = if typed.is_empty() { "My diary".to_string() } else { typed };
        self.sync.state = SyncState { server, token: core_sync::new_token(), name, ..SyncState::default() };
        self.sync.notice.clear();
        self.sync_start(ctx, true);
    }

    /// Give the diary a new name on the server (the next sync tells it; until then the old name stays on the server).
    pub(crate) fn sync_rename(&mut self, ctx: &egui::Context) {
        let name = core_sync::clean_name(&self.sync.name_text);
        if name.is_empty() {
            self.sync.notice = "Type a name for the diary first.".into();
            return;
        }
        self.sync.state.begin_rename(&name);
        self.sync.name_text.clear();
        let id = self.diary_id();
        if let Some(vault) = self.vault.as_ref() {
            core_sync::save_state(&self.sync.path, &id, vault, &self.sync.state).ok();
        }
        self.sync.notice.clear();
        self.sync.retry_at = None;
        self.sync_start(ctx, false);
    }

    /// Choose a new sync code. The next sync swaps it on the server; until the server confirms, the old code stays
    /// in use, so nothing can go wrong if the connection drops.
    pub(crate) fn sync_change_code(&mut self, ctx: &egui::Context) {
        self.sync.confirm_change = false;
        self.sync.state.begin_code_change();
        let id = self.diary_id();
        if let Some(vault) = self.vault.as_ref() {
            core_sync::save_state(&self.sync.path, &id, vault, &self.sync.state).ok();
        }
        self.sync.notice.clear();
        self.sync.retry_at = None;
        self.sync_start(ctx, false);
    }

    /// Use a sync code that was made on another device.
    pub(crate) fn sync_use_code(&mut self, ctx: &egui::Context) {
        let code = self.sync.new_code_text.trim().to_string();
        if code.is_empty() {
            self.sync.notice = "Type or paste the new sync code first.".into();
            return;
        }
        self.sync.previous = Some(self.sync.state.clone());
        self.sync.state.token = code;
        self.sync.state.next_token.clear();
        self.sync.new_code_text.clear();
        let id = self.diary_id();
        if let Some(vault) = self.vault.as_ref() {
            core_sync::save_state(&self.sync.path, &id, vault, &self.sync.state).ok();
        }
        self.sync.notice.clear();
        self.sync.retry_at = None;
        self.sync_start(ctx, false);
    }

    /// Stop syncing on this PC. The diary and the server's copy are left as they are.
    pub(crate) fn sync_turn_off(&mut self) {
        let id = self.diary_id();
        self.sync.running = None;
        self.sync.state = SyncState::default();
        if let Some(vault) = self.vault.as_ref() {
            core_sync::save_state(&self.sync.path, &id, vault, &self.sync.state).ok();
        }
        self.sync.status = Status::Off;
        self.sync.notice = "Sync is off on this PC.".into();
        self.sync.show_code = false;
        self.sync.show_qr = false;
        self.sync.rename_window = false;
        self.sync.code_window = false;
        self.sync.confirm_off = false;
    }

    /// A short line for the status area, empty when there's nothing worth saying.
    pub(crate) fn sync_status_text(&self) -> String {
        match &self.sync.status {
            Status::Off => String::new(),
            Status::Running => "Syncing…".into(),
            Status::Idle(Some(t)) => format!("Synced {t}"),
            Status::Idle(None) => "Sync is on".into(),
            Status::Failed(_) => "Sync paused: will try again".into(),
        }
    }

    /// What the person needs to type on another device: the server address and the secret code.
    pub(crate) fn sync_pairing_text(&self) -> String {
        let diary = if self.sync.state.name.is_empty() { String::new() } else { format!("Diary: {}\n", self.sync.state.name) };
        format!("{diary}Server: {}\nSync code: {}", self.sync.state.server, self.sync.state.token)
    }

    /// The two small windows opened from Settings → Sync: Rename diary and Sync code.
    pub(crate) fn sync_windows(&mut self, ctx: &egui::Context) {
        if !self.sync.configured() {
            self.sync.rename_window = false;
            self.sync.code_window = false;
            return;
        }
        let p = palette(ctx);
        let dark = ctx.theme() == egui::Theme::Dark;
        let running = self.sync.status == Status::Running;

        if self.sync.rename_window {
            let (mut close, mut apply) = (false, false);
            egui::Window::new("Rename diary")
                .title_bar(false)
                .frame(popup_frame(&p, dark))
                .resizable(false)
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.content_rect().center())
                .show(ctx, |ui| {
                    if popup_header(ui, &p, "Rename diary") {
                        close = true;
                    }
                    egui::Frame::new().inner_margin(14).show(ui, |ui| {
                        ui.set_width(380.0);
                        ui.add(egui::TextEdit::singleline(&mut self.sync.name_text).desired_width(f32::INFINITY).hint_text("Diary name"));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            apply = ui.add_enabled(!running && !self.sync.name_text.trim().is_empty(), egui::Button::new("Rename")).clicked();
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    });
                });
            if apply {
                self.sync_rename(ctx);
                close = true;
            }
            if close {
                self.sync.rename_window = false;
            }
        }

        if self.sync.code_window {
            let (mut close, mut change, mut use_code) = (false, false, false);
            let code = self.sync.state.token.clone();
            egui::Window::new("Sync code")
                .title_bar(false)
                .frame(popup_frame(&p, dark))
                .resizable(false)
                .pivot(Align2::CENTER_CENTER)
                .default_pos(ctx.content_rect().center())
                .show(ctx, |ui| {
                    if popup_header(ui, &p, "Sync code") {
                        close = true;
                    }
                    egui::Frame::new().inner_margin(14).show(ui, |ui| {
                        ui.set_width(480.0);
                        ui.add(egui::Label::new(RichText::new("A secret that lets a device use your journal on the server. Anyone with it can add to your synced data but cannot read it, so keep it private.").color(p.muted)).wrap());
                        ui.add_space(8.0);
                        if self.sync.show_code {
                            ui.add(egui::Label::new(RichText::new(&code).monospace().size(13.0).color(p.ink)).selectable(true));
                        }
                        if ui.button(if self.sync.show_code { "Hide the code" } else { "Show the code" }).clicked() {
                            self.sync.show_code = !self.sync.show_code;
                        }
                        ui.add_space(14.0);
                        ui.label(RichText::new("Make a new code").size(16.0).strong().color(p.title));
                        ui.add(egui::Label::new(RichText::new("Stops the old code working at once, on every device. You then enter the new code on your other devices.").color(p.muted)).wrap());
                        ui.add_space(6.0);
                        let label = if self.sync.confirm_change { "Click again: the old code stops working" } else { "Make a new code…" };
                        if ui.add_enabled(!running, egui::Button::new(label)).clicked() {
                            if self.sync.confirm_change {
                                change = true;
                            } else {
                                self.sync.confirm_change = true;
                            }
                        }
                        ui.add_space(14.0);
                        ui.label(RichText::new("Changed on another device?").size(16.0).strong().color(p.title));
                        ui.add(egui::Label::new(RichText::new("Enter the new code here.").color(p.muted)).wrap());
                        ui.add_space(6.0);
                        ui.add(egui::TextEdit::singleline(&mut self.sync.new_code_text).desired_width(f32::INFINITY).hint_text("new sync code").password(true));
                        ui.add_space(6.0);
                        use_code = ui.add_enabled(!running && !self.sync.new_code_text.trim().is_empty(), egui::Button::new("Use this code")).clicked();
                    });
                });
            if change {
                self.sync_change_code(ctx);
            }
            if use_code {
                self.sync_use_code(ctx);
                close = true;
            }
            if close {
                self.sync.code_window = false;
                self.sync.confirm_change = false;
                self.sync.show_code = false;
                self.sync.new_code_text.clear();
            }
        }
    }
    /// The Sync card in Settings. Rows show one line each, so anything longer goes in wrapped text beneath.
    pub(crate) fn sync_card(&mut self, ui: &mut egui::Ui, p: &Palette, width: f32) {
        use crate::kit::{self, Look};
        let ctx = ui.ctx().clone();
        let configured = self.sync.configured();
        let running = self.sync.status == Status::Running;
        let paragraph = |ui: &mut egui::Ui, text: &str, colour: Color32| {
            ui.add_space(8.0);
            ui.label(RichText::new(text).size(14.0).color(colour));
        };
        if !configured {
            ui.label(RichText::new("Keep this journal in step with your phone through a sync server you run yourself. Everything is encrypted on this PC before it is sent, so the server only ever holds scrambled data.").size(14.0).color(p.muted));
            ui.add_space(10.0);
        }
        crate::settings::card(ui, p, width, |ui| {
            if !configured {
                crate::settings::row(ui, p, "Diary name", "So you can tell what you are connected to, for example Dave's Diary.", false, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.sync.name_text).desired_width(250.0).hint_text("My diary"));
                });
                crate::settings::row(ui, p, "Server address", "For example 192.168.1.20:8787", true, |ui| {
                    if kit::button(ui, Look::Primary, None, if running { "Setting up…" } else { "Set up sync" }, None).clicked() && !running {
                        self.sync_set_up(&ctx);
                    }
                    ui.add(egui::TextEdit::singleline(&mut self.sync.server_text).desired_width(210.0).hint_text("192.168.1.20:8787"));
                });
            } else {
                // How it is going: the diary's name, the state of syncing, and the one button most people need.
                let title = if let Some(pending) = &self.sync.state.pending_name { format!("{pending} (updating…)") } else if self.sync.state.name.is_empty() { "Your synced diary".to_string() } else { self.sync.state.name.clone() };
                let description = match &self.sync.status {
                    Status::Running => "Syncing now…".to_string(),
                    Status::Idle(Some(t)) => format!("Synced at {t}."),
                    Status::Idle(None) => "Not synced yet.".to_string(),
                    Status::Failed(_) => "Paused. It will try again soon.".to_string(),
                    Status::Off => String::new(),
                };
                crate::settings::row(ui, p, &title, &description, false, |ui| {
                    if kit::button(ui, Look::Primary, None, if running { "Syncing…" } else { "Sync now" }, None).clicked() && !running {
                        self.sync.retry_at = None;
                        self.sync_start(&ctx, false);
                    }
                });
                // Adding another device: the QR code for the phone to scan, or the details to type.
                crate::settings::row(ui, p, "Add another device", "Scan the QR code on your phone, or copy the details to type there.", false, |ui| {
                    let label = if self.sync.show_qr { "Hide QR code" } else { "Show QR code" };
                    if kit::button(ui, Look::Ghost, None, label, None).clicked() {
                        self.sync.show_qr = !self.sync.show_qr;
                    }
                    if kit::button(ui, Look::Ghost, None, "Copy details", None).clicked() {
                        ctx.copy_text(self.sync_pairing_text());
                        self.sync.notice = "Copied. On the other device, enter the server address and sync code, then your journal password.".into();
                    }
                });
                if self.sync.show_qr {
                    ui.add_space(6.0);
                    let text = tideline_core::pairing::uri(&self.sync.state.server, &self.sync.state.token, &self.sync.state.name);
                    ui.vertical_centered(|ui| paint_qr(ui, &text, 220.0));
                    ui.add_space(4.0);
                    ui.vertical_centered(|ui| ui.label(RichText::new("It contains the sync code, so show it only when you need it. It never contains your password.").size(13.0).color(p.faint)));
                    ui.add_space(12.0);
                }
                // The things you rarely need.
                let name_now = if self.sync.state.name.is_empty() { "No name yet.".to_string() } else { self.sync.state.name.clone() };
                crate::settings::row(ui, p, "Rename diary", &name_now, false, |ui| {
                    if kit::button(ui, Look::Ghost, None, "Rename…", None).clicked() {
                        self.sync.rename_window = true;
                        self.sync.name_text = self.sync.state.name.clone();
                    }
                });
                crate::settings::row(ui, p, "Sync code", "Make a new one, or use one made on another device.", false, |ui| {
                    if kit::button(ui, Look::Ghost, None, "Manage…", None).clicked() {
                        self.sync.code_window = true;
                    }
                });
                crate::settings::row(ui, p, "Server", &self.sync.state.server.clone(), false, |_| {});
                crate::settings::row(ui, p, "Turn off sync", "Stops syncing on this PC. Nothing is deleted.", true, |ui| {
                    if self.sync.confirm_off {
                        if kit::button(ui, Look::Ghost, None, "Cancel", None).clicked() {
                            self.sync.confirm_off = false;
                        }
                        if kit::button(ui, Look::Primary, None, "Yes, turn off", None).clicked() {
                            self.sync_turn_off();
                        }
                    } else if kit::button(ui, Look::Ghost, None, "Turn off…", None).clicked() {
                        self.sync.confirm_off = true;
                    }
                });
            }
        });
        if configured {
            if let Status::Failed(e) = &self.sync.status {
                paragraph(ui, &format!("{e} Your journal is unaffected, and syncing will try again in a minute."), RED);
            }
            if !self.sync.notice.is_empty() && !matches!(self.sync.status, Status::Failed(_)) {
                paragraph(ui, &self.sync.notice.clone(), p.muted);
            }
            paragraph(ui, "It syncs by itself a few seconds after you save, and about every 30 seconds. Anyone with the sync code can add to your synced data but cannot read it, so keep it private.", p.faint);
        } else if !self.sync.notice.is_empty() {
            paragraph(ui, &self.sync.notice.clone(), p.muted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tideline_core::sync as cs;

    const PASSWORD: &str = "correct horse";

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("diary-sync-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A desktop app with a made-up diary, unlocked, with sync not yet set up.
    fn desktop(name: &str) -> (DiaryApp, std::path::PathBuf) {
        let path = temp(name).join("entries.json");
        let mut app = DiaryApp::new();
        app.lock = None;
        let v = vault::Vault::create(PASSWORD).unwrap();
        app.data = fresh_data();
        assign_ids(&mut app.data, &v);
        app.vault = Some(v);
        app.sync_unlocked(&path);
        for (id, text) in [(1, "# Morning\n\ncoffee on the balcony"), (2, "# Evening\n\na long walk")] {
            app.data.entries.push(Entry { id, date: "2026-10-09".into(), added_at: format!("2026-10-09 0{id}:00:00"), category: "Personal".into(), text: text.into(), ..Default::default() });
        }
        app.persist();
        (app, path)
    }

    /// Wait for the background sync to finish, as the frame loop would.
    fn settle(app: &mut DiaryApp) {
        let ctx = egui::Context::default();
        for _ in 0..1500 {
            app.sync_tick(&ctx);
            if app.sync.running.is_none() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the sync did not finish");
    }

    fn sync_now(app: &mut DiaryApp, register: bool) {
        let ctx = egui::Context::default();
        app.sync.retry_at = None;
        app.sync_start(&ctx, register);
        settle(app);
        assert!(matches!(app.sync.status, Status::Idle(_)), "the sync failed: {:?}", app.sync.status);
    }

    fn phone_sync(phone: &mut tideline_core::Session, state: &mut SyncState) -> Report {
        let mut machine = Sync::new(std::mem::take(state));
        let mut answer = None;
        let report = loop {
            match machine.poll(phone, answer.take()).expect("the phone sync works") {
                Poll::Done(r) => break r,
                Poll::Request(req) => answer = Some(http(&state_server(&machine), &req).expect("the server answers")),
            }
        };
        *state = machine.into_state();
        report
    }

    fn state_server(machine: &Sync) -> String {
        machine.state().server.clone()
    }

    fn start_server(name: &str) -> tideline_server::Running {
        let store = tideline_server::store::Store::open(&temp(name).join("server.db")).unwrap();
        tideline_server::spawn(store, "127.0.0.1:0").unwrap()
    }

    fn phone_entries(phone: &tideline_core::Session) -> Vec<serde_json::Value> {
        phone.export_data()["entries"].as_array().cloned().unwrap_or_default()
    }

    fn phone_id(phone: &tideline_core::Session, part: &str) -> u64 {
        phone_entries(phone).iter().find(|e| e["text"].as_str().unwrap().contains(part)).unwrap()["id"].as_u64().unwrap()
    }

    #[test]
    fn the_desktop_and_a_phone_stay_in_step_through_the_server() {
        let server = start_server("e2e-server");
        let (mut app, _) = desktop("e2e-desktop");
        app.sync.state = SyncState { server: server.url(), token: cs::new_token(), ..Default::default() };
        sync_now(&mut app, true);
        assert!(app.sync.notice.contains("sent"), "{}", app.sync.notice);

        // The phone signs in with the keyring and the password, and receives both entries.
        let ring = app.vault.as_ref().unwrap().keyring();
        let mut phone = cs::join(&temp("e2e-phone").join("entries.json"), &ring, PASSWORD).unwrap();
        let mut phone_state = SyncState { server: server.url(), token: app.sync.state.token.clone(), ..Default::default() };
        let report = phone_sync(&mut phone, &mut phone_state);
        assert!(report.pulled >= 2, "{report:?}");
        assert_eq!(phone_entries(&phone).len(), 2);

        // The phone edits one entry and pins the other; the desktop receives both changes.
        let (coffee, walk) = (phone_id(&phone, "coffee"), phone_id(&phone, "walk"));
        phone.save_entry(Some(coffee), "Morning", "tea on the balcony", None).unwrap();
        phone.pin_entry(walk, true).unwrap();
        phone_sync(&mut phone, &mut phone_state);
        sync_now(&mut app, false);
        let tea = app.data.entries.iter().find(|e| e.text.contains("tea on the balcony")).expect("the phone edit arrived");
        assert!(!tea.text.contains("coffee"));
        assert!(app.data.entries.iter().find(|e| e.text.contains("long walk")).unwrap().pinned, "the phone pin arrived");

        // The desktop deletes an entry and writes a new one; the phone receives them.
        app.data.entries.retain(|e| !e.text.contains("long walk"));
        app.data.entries.push(Entry { id: 99, date: "2026-10-10".into(), added_at: "2026-10-10 08:00:00".into(), category: "Personal".into(), text: "written on the PC".into(), ..Default::default() });
        app.persist();
        sync_now(&mut app, false);
        phone_sync(&mut phone, &mut phone_state);
        let texts: Vec<String> = phone_entries(&phone).iter().map(|e| e["text"].as_str().unwrap().to_string()).collect();
        assert!(texts.iter().any(|t| t == "written on the PC"), "{texts:?}");
        assert!(texts.iter().all(|t| !t.contains("long walk")), "the PC deletion arrived: {texts:?}");

        // Nothing is left to do.
        sync_now(&mut app, false);
        let quiet = phone_sync(&mut phone, &mut phone_state);
        let quiet = if quiet.pushed > 0 || quiet.pulled > 0 { phone_sync(&mut phone, &mut phone_state) } else { quiet };
        assert_eq!((quiet.pulled, quiet.pushed), (0, 0), "{quiet:?}");
        server.stop();
    }

    #[test]
    fn text_edited_on_the_pc_and_the_phone_at_once_is_kept_as_two_entries() {
        let server = start_server("conflict-server");
        let (mut app, _) = desktop("conflict-desktop");
        app.sync.state = SyncState { server: server.url(), token: cs::new_token(), ..Default::default() };
        sync_now(&mut app, true);
        let ring = app.vault.as_ref().unwrap().keyring();
        let mut phone = cs::join(&temp("conflict-phone").join("entries.json"), &ring, PASSWORD).unwrap();
        let mut phone_state = SyncState { server: server.url(), token: app.sync.state.token.clone(), ..Default::default() };
        phone_sync(&mut phone, &mut phone_state);

        // Both rewrite the same entry while apart.
        app.data.entries.iter_mut().find(|e| e.text.contains("coffee")).unwrap().text = "# Morning\n\nplan written on the PC".into();
        app.persist();
        let id = phone_id(&phone, "coffee");
        phone.save_entry(Some(id), "Morning", "plan written on the phone", None).unwrap();

        phone_sync(&mut phone, &mut phone_state);
        sync_now(&mut app, false);
        sync_now(&mut app, false);
        phone_sync(&mut phone, &mut phone_state);
        phone_sync(&mut phone, &mut phone_state);
        sync_now(&mut app, false);

        let mine: Vec<&str> = app.data.entries.iter().map(|e| e.text.as_str()).filter(|t| t.contains("plan written")).collect();
        assert_eq!(mine.len(), 2, "both versions are kept: {mine:?}");
        assert_eq!(mine.iter().filter(|t| t.contains("conflicting version")).count(), 1);
        let theirs = phone_entries(&phone);
        let phone_plans = theirs.iter().filter(|e| e["text"].as_str().unwrap().contains("plan written")).count();
        assert_eq!(phone_plans, 2, "the phone has the same two");
        assert_eq!(app.data.entries.len(), theirs.len(), "the same number of entries on both");
        server.stop();
    }

    #[test]
    fn saving_stamps_changes_and_deletes_leave_tombstones() {
        let (mut app, _) = desktop("stamps");
        let clock_of = |app: &DiaryApp, id: u64, field: &str| -> Option<String> { app.data.entries.iter().find(|e| e.id == id)?.extra.get("clock")?.get(field)?.as_str().map(str::to_string) };
        let created = clock_of(&app, 1, "text").expect("a new entry has its fields stamped");
        app.data.entries[0].pinned = true;
        app.persist();
        let pinned = clock_of(&app, 1, "pinned").unwrap();
        assert!(pinned > created, "the later change has the later clock");
        assert_eq!(clock_of(&app, 1, "text").unwrap(), created, "untouched fields keep their clock");

        // Move to the bin: a tombstone. Empty the bin: a permanent one.
        let entry = app.data.entries.remove(1);
        let uid = entry.extra.get("uid").unwrap().as_str().unwrap().to_string();
        app.data.trash.push(trash::Trashed { entry, deleted_at: "2026-10-09 12:00:00".into(), extra: Default::default() });
        app.persist();
        assert!(app.data.trash[0].entry.extra.get("deleted").is_some(), "deleting leaves a tombstone");
        app.data.trash.clear();
        app.persist();
        let stubs = app.data.extra.get("tombstones").and_then(|t| t.as_array()).expect("erasing leaves a tombstone");
        assert!(stubs.iter().any(|s| s["uid"] == uid.as_str() && s["purged"] == true));
    }

    #[test]
    fn a_server_that_is_off_is_reported_and_retried_without_harm() {
        let (mut app, _) = desktop("offline");
        app.sync.state = SyncState { server: "http://127.0.0.1:9".into(), token: cs::new_token(), ..Default::default() };
        let ctx = egui::Context::default();
        app.sync_start(&ctx, true);
        settle(&mut app);
        match &app.sync.status {
            Status::Failed(e) => assert!(e.contains("Couldn't reach the sync server"), "{e}"),
            other => panic!("{other:?}"),
        }
        assert!(app.sync.retry_at.is_some(), "it will try again");
        assert_eq!(app.data.entries.len(), 2, "the diary is untouched");
        assert!(app.sync_status_text().contains("paused"));
    }


    /// `TIDELINE_TEST_FILE=<path> TIDELINE_TEST_PW=<password> cargo test --release real_diary -- --ignored`:
    /// syncs a copy of a real diary through the real server to a second device. The file itself is only read.
    #[test]
    #[ignore]
    fn real_diary_syncs_to_a_second_device() {
        let (Ok(src), Ok(pw)) = (std::env::var("TIDELINE_TEST_FILE"), std::env::var("TIDELINE_TEST_PW")) else { return };
        let env = vault::Envelope::parse(&std::fs::read_to_string(src).unwrap()).unwrap();
        let (v, plain) = vault::Vault::unlock(&env, &pw).unwrap();
        let server = start_server("real-server");
        let path = temp("real-desktop").join("entries.json");
        let mut app = DiaryApp::new();
        app.lock = None;
        app.data = serde_json::from_slice::<Data>(&plain).unwrap();
        assign_ids(&mut app.data, &v);
        app.vault = Some(v);
        app.sync_unlocked(&path);
        let (entries, images, files) = (app.data.entries.len(), app.data.images.len(), app.data.files.len());
        app.sync.state = SyncState { server: server.url(), token: cs::new_token(), ..Default::default() };
        sync_now(&mut app, true);

        let ring = app.vault.as_ref().unwrap().keyring();
        let mut phone = cs::join(&temp("real-phone").join("entries.json"), &ring, &pw).unwrap();
        let mut state = SyncState { server: server.url(), token: app.sync.state.token.clone(), ..Default::default() };
        let report = phone_sync(&mut phone, &mut state);
        let theirs = phone.export_data();
        println!("{entries} entries, {images} pictures, {files} files; phone received {report:?}");
        assert_eq!(theirs["entries"].as_array().unwrap().len(), entries);
        assert_eq!(theirs["images"].as_object().map_or(0, |m| m.len()), images);
        assert_eq!(theirs["files"].as_object().map_or(0, |m| m.len()), files);
        for e in &app.data.entries {
            let uid = e.extra.get("uid").unwrap().as_str().unwrap();
            let there = theirs["entries"].as_array().unwrap().iter().find(|x| x["uid"] == uid).expect("every entry arrived");
            assert_eq!(there["text"], e.text.as_str());
            assert_eq!(there["date"], e.date.as_str());
        }
        // And the phone's view is quiet and stable.
        let again = phone_sync(&mut phone, &mut state);
        assert_eq!((again.pulled, again.pushed), (0, 0), "{again:?}");
        sync_now(&mut app, false);
        println!("OK");
        server.stop();
    }

    #[test]
    fn changing_the_sync_code_on_the_pc_and_entering_it_on_the_phone() {
        let server = start_server("code-server");
        let (mut app, _) = desktop("code-desktop");
        let old = cs::new_token();
        app.sync.state = SyncState { server: server.url(), token: old.clone(), ..Default::default() };
        sync_now(&mut app, true);
        let ring = app.vault.as_ref().unwrap().keyring();
        let mut phone = cs::join(&temp("code-phone").join("entries.json"), &ring, PASSWORD).unwrap();
        let mut phone_state = SyncState { server: server.url(), token: old.clone(), ..Default::default() };
        phone_sync(&mut phone, &mut phone_state);
        assert_eq!(phone_entries(&phone).len(), 2);

        // The PC changes the code: it needs a confirming click first, and then the old code stops working.
        app.sync.confirm_change = true;
        let ctx = egui::Context::default();
        app.sync_change_code(&ctx);
        settle(&mut app);
        assert!(matches!(app.sync.status, Status::Idle(_)), "{:?}", app.sync.status);
        let new = app.sync.state.token.clone();
        assert_ne!(new, old);
        assert!(!app.sync.confirm_change && !app.sync.state.changing_code());
        assert!(app.sync.notice.contains("sync code was changed"), "{}", app.sync.notice);
        assert!(app.sync.show_code, "the new code is shown so it can be copied");

        // The phone, still on the old code, is refused with a message that says what to do.
        let mut machine = Sync::new(std::mem::take(&mut phone_state));
        let refused = loop {
            match machine.poll(&mut phone, None) {
                Err(e) => break e,
                Ok(Poll::Done(_)) => panic!("the old code must not work"),
                Ok(Poll::Request(req)) => {
                    // Feed the reply back in, as the real client would.
                    let reply = http(&machine.state().server.clone(), &req).unwrap();
                    match machine.poll(&mut phone, Some(reply)) {
                        Err(e) => break e,
                        Ok(_) => continue,
                    }
                }
            }
        };
        assert!(refused.contains("did not accept"), "{refused}");
        // With the new code it carries on, and the diaries agree.
        phone_state = SyncState { server: server.url(), token: new.clone(), ..Default::default() };
        phone_sync(&mut phone, &mut phone_state);
        let id = phone_id(&phone, "coffee");
        phone.save_entry(Some(id), "Morning", "after the change", None).unwrap();
        phone_sync(&mut phone, &mut phone_state);
        sync_now(&mut app, false);
        assert!(app.data.entries.iter().any(|e| e.text.contains("after the change")));
        server.stop();
    }

    #[test]
    fn a_new_code_the_server_refuses_is_put_back() {
        let server = start_server("badcode-server");
        let (mut app, _) = desktop("badcode-desktop");
        let old = cs::new_token();
        app.sync.state = SyncState { server: server.url(), token: old.clone(), ..Default::default() };
        sync_now(&mut app, true);

        // Someone types a code the server has never heard of.
        app.sync.new_code_text = "x".repeat(43);
        let ctx = egui::Context::default();
        app.sync_use_code(&ctx);
        settle(&mut app);
        match &app.sync.status {
            Status::Failed(e) => assert!(e.contains("did not accept"), "{e}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(app.sync.state.token, old, "the old, working code is back");
        sync_now(&mut app, false);
        // An empty box does nothing.
        app.sync.new_code_text = "  ".into();
        app.sync_use_code(&ctx);
        assert!(app.sync.notice.contains("Type or paste"));
        assert_eq!(app.sync.state.token, old);
        server.stop();
    }

    #[test]
    fn the_diary_is_named_when_set_up_and_can_be_renamed() {
        let server = start_server("named-server");
        let (mut app, _) = desktop("named-desktop");
        // Set up from the Settings screen: server address and a name typed in.
        app.sync.server_text = server.url();
        app.sync.name_text = "  Dave's   Diary ".into();
        let ctx = egui::Context::default();
        app.sync_set_up(&ctx);
        settle(&mut app);
        assert!(matches!(app.sync.status, Status::Idle(_)), "{:?}", app.sync.status);
        assert_eq!(app.sync.state.name, "Dave's Diary");
        assert!(app.sync_pairing_text().starts_with("Diary: Dave's Diary\nServer: "), "{}", app.sync_pairing_text());

        // A phone joining sees the name straight away, and again after its first sync.
        let token = app.sync.state.token.clone();
        let keyring = http(&server.url(), &core_sync::keyring_request(&token)).unwrap();
        assert_eq!(core_sync::name_from_response(&keyring), "Dave's Diary");
        let ring = app.vault.as_ref().unwrap().keyring();
        let mut phone = cs::join(&temp("named-phone").join("entries.json"), &ring, PASSWORD).unwrap();
        let mut phone_state = SyncState { server: server.url(), token, ..Default::default() };
        phone_sync(&mut phone, &mut phone_state);
        assert_eq!(phone_state.name, "Dave's Diary");

        // Renamed on the PC: shown as updating, then confirmed, and the phone hears of it.
        app.sync.name_text = "Dave's Work Diary".into();
        app.sync_rename(&ctx);
        settle(&mut app);
        assert_eq!((app.sync.state.name.as_str(), app.sync.state.pending_name.clone()), ("Dave's Work Diary", None));
        phone_sync(&mut phone, &mut phone_state);
        assert_eq!(phone_state.name, "Dave's Work Diary");
        // An empty box does nothing; a blank name at set-up falls back to a default.
        app.sync.name_text = "  ".into();
        app.sync_rename(&ctx);
        assert!(app.sync.notice.contains("Type a name"));
        server.stop();
    }

    #[test]
    fn server_addresses_are_tidied() {
        assert_eq!(clean_server("192.168.1.20:8787"), "http://192.168.1.20:8787");
        assert_eq!(clean_server("  http://host:1/ "), "http://host:1");
        assert_eq!(clean_server("https://sync.example.com"), "https://sync.example.com");
        assert_eq!(clean_server("   "), "");
    }
}

/// Draws `text` as a QR code `size` points wide: dark squares on white, with the blank border scanners need.
fn paint_qr(ui: &mut egui::Ui, text: &str, size: f32) {
    let Ok(code) = qrcode::QrCode::new(text.as_bytes()) else {
        ui.label("The details are too long to show as a QR code.");
        return;
    };
    let modules = code.width();
    let border = 4usize;
    let cell = size / (modules + 2 * border) as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, Color32::WHITE);
    for (i, colour) in code.to_colors().into_iter().enumerate() {
        if colour == qrcode::Color::Dark {
            let (x, y) = ((i % modules) as f32, (i / modules) as f32);
            let top_left = rect.min + egui::vec2((x + border as f32) * cell, (y + border as f32) * cell);
            painter.rect_filled(egui::Rect::from_min_size(top_left, egui::vec2(cell + 0.5, cell + 0.5)), 0.0, Color32::BLACK);
        }
    }
}
