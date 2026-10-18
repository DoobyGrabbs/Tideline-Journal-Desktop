//! The AI helpers: draft an entry from your notes, summarise a day, suggest a reflection question.
//!
//! They use Ollama, a free program that runs AI models on this PC, so nothing written in the
//! diary is sent over the internet. They are off until you set them up (Settings → AI helpers),
//! and the buttons only appear while Ollama is running with a model chosen. Private entries and
//! entries written by the AI itself are never sent.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::ollama::{self, Client, ModelInfo, Progress};

// ---------------------------------------------------------------------------------------------
// What is asked
// ---------------------------------------------------------------------------------------------

pub(crate) const DRAFT_SYSTEM: &str = "You help someone write their private diary. Turn their rough notes into a short diary entry \
written in the first person, in a natural, personal voice. Use ONLY the facts in the notes. Do not invent events, people, places, \
feelings or details that are not in the notes. Keep it about the same length as the notes, or a little longer. You may use simple \
Markdown. Reply with the diary entry text only, with no introduction or explanation.";

pub(crate) const SUMMARY_SYSTEM: &str = "You summarise one day from someone's diary entries. Write 3 to 5 sentences in the first person, \
in a calm, plain style, using ONLY what the entries say. Do not invent anything and do not give advice. Reply with the summary only.";

pub(crate) const REFLECTION_SYSTEM: &str = "You suggest one gentle, thoughtful reflection question for someone's diary, inspired by their \
recent entries. Ask a single open question. Do not give advice, do not diagnose, and do not repeat their words back at length. \
Reply with the question only.";

const TEST_SYSTEM: &str = "You are a friendly assistant.";
const TEST_USER: &str = "Reply with a short friendly greeting of at most ten words, to confirm you are working.";

/// Cut `text` to at most `max` characters.
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max { text.to_string() } else { text.chars().take(max).collect::<String>() + " …" }
}

/// Entries that may be sent to the AI: not private, not written by the AI, and not empty.
fn shareable(e: &Entry) -> bool {
    !e.sensitive && !e.generated && !e.text.trim().is_empty()
}

/// The wording given to the AI (plain feelings, independent of the labels shown in the interface).
const AI_MOOD_WORDS: [&str; 5] = ["Really sad", "Sad", "Okay", "Happy", "Really happy"];

fn mood_word(mood: Option<u8>) -> Option<&'static str> {
    mood.filter(|m| (1..=5).contains(m)).map(|m| AI_MOOD_WORDS[usize::from(m) - 1])
}

/// (system, user) for turning rough notes into an entry.
pub(crate) fn draft_request(notes: &str, date: NaiveDate, mood: Option<u8>, category: &str) -> (String, String) {
    let mut user = format!("Date: {}\n", date.format("%A %e %B %Y"));
    if let Some(m) = mood_word(mood) {
        user.push_str(&format!("How the day felt: {m}\n"));
    }
    user.push_str(&format!("Category: {category}\n\nMy notes:\n{}", clip(notes.trim(), 4000)));
    (DRAFT_SYSTEM.into(), user)
}

/// (system, user) for summarising a day, or `None` if there is nothing that may be sent.
pub(crate) fn summary_request(entries: &[&Entry], date: NaiveDate) -> Option<(String, String)> {
    let mut usable: Vec<&&Entry> = entries.iter().filter(|e| shareable(e)).collect();
    if usable.is_empty() {
        return None;
    }
    usable.sort_by(|a, b| a.added_at.cmp(&b.added_at));
    let mut user = format!("Date: {}\n\nEntries:\n", date.format("%A %e %B %Y"));
    for e in usable {
        let mood = mood_word(e.mood).map(|m| format!(", mood: {m}")).unwrap_or_default();
        user.push_str(&format!("[{}, {}{mood}]\n{}\n\n", e.added_at.get(11..16).unwrap_or(""), e.category, markdown::plain_text(&e.text)));
    }
    Some((SUMMARY_SYSTEM.into(), clip(&user, 6000)))
}

/// (system, user) for a reflection question based on the latest entries.
pub(crate) fn reflection_request(recent: &[&Entry], avoid: Option<&str>) -> (String, String) {
    let mut usable: Vec<&&Entry> = recent.iter().filter(|e| shareable(e)).collect();
    usable.sort_by(|a, b| b.added_at.cmp(&a.added_at));
    let mut user = String::from("Recent diary entries, newest first:\n\n");
    if usable.is_empty() {
        user.push_str("(There are no recent entries yet. Suggest a gentle question to start a diary with.)");
    }
    for e in usable.into_iter().take(6) {
        user.push_str(&format!("[{}] {}\n\n", e.date, clip(&markdown::plain_text(&e.text), 350)));
    }
    let mut user = clip(&user, 2200);
    if let Some(previous) = avoid {
        user.push_str(&format!("\n\nDo not repeat this earlier question: {previous}"));
    }
    (REFLECTION_SYSTEM.into(), user)
}

/// The question itself, without bullets, numbering or quotation marks.
pub(crate) fn clean_question(raw: &str) -> String {
    let line = raw.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let line = line.trim_start_matches(|c: char| c == '-' || c == '*' || c == '•' || c.is_ascii_digit() || c == '.' || c == ')' || c == ' ');
    line.trim_matches(|c: char| matches!(c, '"' | '“' | '”' | '\'' | '`' | ' ')).to_string()
}

fn model_matches(installed: &str, wanted: &str) -> bool {
    installed == wanted || installed == format!("{wanted}:latest") || wanted == format!("{installed}:latest")
}

// ---------------------------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Status {
    /// Not looked yet.
    Unknown,
    NotInstalled,
    /// Installed, but the server isn't running.
    Stopped(std::path::PathBuf),
    Running { version: String, models: Vec<ModelInfo> },
}

fn detect() -> Status {
    let client = Client::new();
    match client.version() {
        Ok(version) => Status::Running { version, models: client.models().unwrap_or_default() },
        Err(_) => match ollama::find_exe() {
            Some(path) => Status::Stopped(path),
            None => Status::NotInstalled,
        },
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Draft,
    Summary,
    Reflection,
    Test,
}

#[derive(PartialEq, Debug)]
pub(crate) enum TaskState {
    Running,
    Done,
    Failed(String),
}

enum TaskMsg {
    Token(String),
    Finished(Result<(), String>),
}

pub(crate) struct Task {
    pub kind: Kind,
    pub title: String,
    /// Exactly what was sent to Ollama.
    pub system: String,
    pub user: String,
    /// The day a summary is for.
    pub date: NaiveDate,
    pub text: String,
    pub state: TaskState,
    rx: Receiver<TaskMsg>,
    cancel: Arc<AtomicBool>,
}

enum PullMsg {
    Progress(Progress),
    Done(Result<(), String>),
}

struct Pull {
    model: String,
    rx: Receiver<PullMsg>,
    cancel: Arc<AtomicBool>,
    progress: Option<Progress>,
    error: Option<String>,
}

enum InstallMsg {
    Line(String),
    Done(Result<(), String>),
}

struct Install {
    rx: Receiver<InstallMsg>,
    log: Vec<String>,
    outcome: Option<Result<(), String>>,
}

pub(crate) struct AiState {
    pub setup_open: bool,
    pub status: Status,
    status_rx: Option<Receiver<Status>>,
    last_check: Option<Instant>,
    pub task: Option<Task>,
    pull: Option<Pull>,
    install: Option<Install>,
    msg: String,
    agree_install: bool,
    winget: Option<bool>,
    show_sent: bool,
}

impl Default for AiState {
    fn default() -> Self {
        Self {
            setup_open: false,
            status: Status::Unknown,
            status_rx: None,
            last_check: None,
            task: None,
            pull: None,
            install: None,
            msg: String::new(),
            agree_install: false,
            winget: None,
            show_sent: false,
        }
    }
}

impl AiState {
    /// Forget everything about the current session (the diary was locked).
    pub fn reset(&mut self) {
        if let Some(t) = &self.task {
            t.cancel.store(true, Ordering::Relaxed);
        }
        if let Some(p) = &self.pull {
            p.cancel.store(true, Ordering::Relaxed);
        }
        *self = AiState::default();
    }
}

fn open_in_browser(url: &str) {
    std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn().ok();
}

fn percent(p: &Progress) -> Option<f32> {
    (p.total > 0).then(|| (p.completed as f32 / p.total as f32).clamp(0.0, 1.0))
}

impl DiaryApp {
    /// Are the AI helpers on, with Ollama running and the chosen model installed?
    pub(crate) fn ai_ready(&self) -> bool {
        let s = &self.data.settings;
        s.ai_enabled
            && !s.ai_model.is_empty()
            && matches!(&self.ai.status, Status::Running { models, .. } if models.iter().any(|m| model_matches(&m.name, &s.ai_model)))
    }

    fn ai_check_now(&mut self) {
        if self.ai.status_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            tx.send(detect()).ok();
        });
        self.ai.status_rx = Some(rx);
        self.ai.last_check = Some(Instant::now());
    }

    /// Called every frame: collects results from the background threads and schedules checks.
    pub(crate) fn ai_poll(&mut self, ctx: &egui::Context) {
        if self.lock.is_some() {
            return;
        }
        let mut busy = false;

        if let Some(rx) = &self.ai.status_rx {
            if let Ok(status) = rx.try_recv() {
                self.ai.status = status;
                self.ai.status_rx = None;
            } else {
                busy = true;
            }
        }
        let wanted = self.data.settings.ai_enabled || self.ai.setup_open;
        let every = Duration::from_secs(if self.ai.setup_open { 3 } else { 30 });
        if wanted && self.ai.status_rx.is_none() && self.ai.last_check.is_none_or(|t| t.elapsed() >= every) {
            self.ai_check_now();
        }

        let mut recheck = false;
        if let Some(task) = self.ai.task.as_mut() {
            while let Ok(msg) = task.rx.try_recv() {
                match msg {
                    TaskMsg::Token(t) => task.text.push_str(&t),
                    TaskMsg::Finished(Ok(())) => task.state = TaskState::Done,
                    TaskMsg::Finished(Err(e)) => {
                        recheck |= e.contains("isn't running");
                        task.state = TaskState::Failed(e);
                    }
                }
            }
            busy |= task.state == TaskState::Running;
        }
        let mut pull_done = false;
        if let Some(pull) = self.ai.pull.as_mut() {
            while let Ok(msg) = pull.rx.try_recv() {
                match msg {
                    PullMsg::Progress(p) => pull.progress = Some(p),
                    PullMsg::Done(Ok(())) => {
                        self.ai.msg = format!("Downloaded {}.", pull.model);
                        if self.data.settings.ai_model.is_empty() {
                            self.data.settings.ai_model = pull.model.clone();
                        }
                        pull_done = true;
                    }
                    PullMsg::Done(Err(e)) => pull.error = Some(e),
                }
            }
            busy |= pull.error.is_none() && !pull_done;
        }
        if pull_done {
            self.ai.pull = None;
            recheck = true;
            self.persist();
        }
        if let Some(inst) = self.ai.install.as_mut() {
            while let Ok(msg) = inst.rx.try_recv() {
                match msg {
                    InstallMsg::Line(l) => {
                        inst.log.push(l);
                        if inst.log.len() > 40 {
                            inst.log.remove(0);
                        }
                    }
                    InstallMsg::Done(r) => {
                        recheck = true;
                        inst.outcome = Some(r);
                    }
                }
            }
            busy |= inst.outcome.is_none();
        }
        if recheck {
            self.ai.last_check = None;
            self.ai_check_now();
        }
        if busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if wanted {
            ctx.request_repaint_after(every);
        }
    }

    /// Ask the model something and open the result window.
    pub(crate) fn ai_start(&mut self, kind: Kind, title: &str, system: String, user: String, date: NaiveDate) {
        if let Some(old) = self.ai.task.take() {
            old.cancel.store(true, Ordering::Relaxed);
        }
        let model = self.data.settings.ai_model.clone();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (flag, s, u) = (cancel.clone(), system.clone(), user.clone());
        std::thread::spawn(move || {
            let result = Client::new()
                .chat(&model, &s, &u, &mut |piece| !flag.load(Ordering::Relaxed) && tx.send(TaskMsg::Token(piece.to_string())).is_ok())
                .map_err(|e| e.to_string());
            tx.send(TaskMsg::Finished(result)).ok();
        });
        self.ai.task = Some(Task { kind, title: title.to_string(), system, user, date, text: String::new(), state: TaskState::Running, rx, cancel });
        self.ai.show_sent = false;
    }

    /// Turn the notes in the entry being written into a draft.
    pub(crate) fn ai_draft(&mut self) {
        let Some(ed) = self.editor.as_ref() else { return };
        let notes: String = live::split(&ed.text)
            .iter()
            .filter_map(|s| if let live::Seg::Text(t) = s { Some(t.as_str()) } else { None })
            .collect::<Vec<_>>()
            .join("\n");
        let (system, user) = draft_request(&notes, ed.date, ed.mood, &ed.category);
        let date = ed.date;
        self.ai_start(Kind::Draft, "AI draft from your notes", system, user, date);
    }

    /// Ask for a reflection question inspired by recent entries.
    pub(crate) fn ai_reflection(&mut self, avoid: Option<String>) {
        let recent: Vec<&Entry> = self.data.entries.iter().collect();
        let (system, user) = reflection_request(&recent, avoid.as_deref());
        let date = self.editor.as_ref().map_or(self.selected, |e| e.date);
        self.ai_start(Kind::Reflection, "A question to think about", system, user, date);
    }

    /// Summarise one day from its entries (private ones are never sent).
    pub(crate) fn ai_summarise_day(&mut self, date: NaiveDate) {
        let day = key(date);
        let entries: Vec<&Entry> = self.data.entries.iter().filter(|e| e.date == day).collect();
        let Some((system, user)) = summary_request(&entries, date) else {
            self.status = "There is nothing on that day that can be summarised (private entries are never sent).".into();
            return;
        };
        self.ai_start(Kind::Summary, &format!("Summary of {}", date.format("%A %e %B")), system, user, date);
    }

    /// Does the day have anything the AI may summarise?
    pub(crate) fn day_has_summarisable(&self, date: NaiveDate) -> bool {
        let day = key(date);
        self.data.entries.iter().any(|e| e.date == day && shareable(e))
    }

    fn ai_insert_into_editor(&mut self, text: &str, replace: bool, as_heading: bool) {
        let Some(ed) = self.editor.as_mut() else { return };
        let text = if as_heading { format!("## {}\n\n", text.trim()) } else { text.trim().to_string() };
        ed.text = if replace || ed.text.trim().is_empty() { text } else { format!("{}\n\n{}", ed.text.trim_end(), text) };
        ed.ai = true;
        ed.want_focus = Some((usize::MAX, usize::MAX));
    }

    /// Save a day summary as its own entry, in an "AI" category and marked as AI-written.
    fn ai_save_summary(&mut self, date: NaiveDate, text: &str) {
        if !self.data.categories.iter().any(|c| c.name == "AI") {
            self.data.categories.push(Category { name: "AI".into(), color: [168, 85, 247], extra: Default::default() });
        }
        let id = self.data.entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        let time = if date == Local::now().date_naive() { Local::now().format("%H:%M:%S").to_string() } else { "23:59:00".into() };
        self.data.entries.push(Entry {
            id,
            date: key(date),
            added_at: format!("{} {time}", key(date)),
            category: "AI".into(),
            text: format!("**Summary of the day**, written by AI from my entries\n\n{}", text.trim()),
            ai: true,
            generated: true,
            ..Default::default()
        });
        self.selected = date;
        self.month = date.with_day(1).unwrap();
        self.persist();
    }

    /// Install Ollama with winget in the background.
    fn ai_start_install(&mut self) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let sender = tx.clone();
            let result = ollama::install_with_winget(&mut |line| {
                sender.send(InstallMsg::Line(line.to_string())).ok();
            });
            tx.send(InstallMsg::Done(result)).ok();
        });
        self.ai.install = Some(Install { rx, log: Vec::new(), outcome: None });
    }

    /// Download a model in the background, with progress.
    fn ai_start_pull(&mut self, model: String) {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let name = model.clone();
        std::thread::spawn(move || {
            let sender = tx.clone();
            let result = Client::new()
                .pull(&name, &mut |pr| !flag.load(Ordering::Relaxed) && sender.send(PullMsg::Progress(pr)).is_ok())
                .map_err(|e| e.to_string());
            tx.send(PullMsg::Done(result)).ok();
        });
        self.ai.pull = Some(Pull { model, rx, cancel, progress: None, error: None });
    }

    // -----------------------------------------------------------------------------------------
    // Windows
    // -----------------------------------------------------------------------------------------

    /// The result of an AI request, with what to do with it.
    pub(crate) fn ai_task_window(&mut self, ctx: &egui::Context) {
        let Some(task) = self.ai.task.as_mut() else { return };
        let p = palette(ctx);
        let dark = ctx.theme() == egui::Theme::Dark;
        let editor_open = self.editor.is_some();
        let kind = task.kind;
        let (mut close, mut cancel, mut retry) = (false, false, false);
        let (mut insert, mut replace, mut heading, mut save, mut copy, mut another, mut open_setup) = (false, false, false, false, false, false, false);
        let mut show_sent = self.ai.show_sent;

        egui::Window::new("AI result")
            .title_bar(false)
            .frame(popup_frame(&p, dark))
            .resizable(true)
            .default_size([620.0, 520.0])
            .min_size([380.0, 280.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                close |= popup_header(ui, &p, &task.title);
                egui::Frame::new().inner_margin(14).show(ui, |ui| {
                    match &task.state {
                        TaskState::Running => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(RichText::new("Thinking… the first answer can take a minute while the model loads.").color(p.muted));
                                if ui.button("Stop").clicked() {
                                    cancel = true;
                                }
                            });
                        }
                        TaskState::Failed(e) => {
                            ui.add(egui::Label::new(RichText::new(e).color(Color32::from_rgb(239, 68, 68))).wrap());
                            ui.horizontal(|ui| {
                                retry = ui.button("Try again").clicked();
                                open_setup = ui.button("Open AI helpers").clicked();
                            });
                        }
                        TaskState::Done => {
                            ui.label(RichText::new("Written by an AI model on this PC. Check it, change anything you like, then choose what to do with it.").small().color(p.muted));
                        }
                    }
                    ui.add_space(6.0);
                    let height = (ui.available_height() - 110.0).max(100.0);
                    egui::ScrollArea::vertical().id_salt("ai_text").max_height(height).auto_shrink([false, false]).show(ui, |ui| {
                        if task.state == TaskState::Done {
                            ui.add(egui::TextEdit::multiline(&mut task.text).text_color(p.ink).desired_width(f32::INFINITY).desired_rows(8));
                        } else {
                            ui.add(egui::Label::new(RichText::new(&task.text).color(p.ink)).wrap());
                        }
                    });
                    ui.add_space(8.0);
                    if task.state == TaskState::Done {
                        ui.horizontal_wrapped(|ui| match kind {
                            Kind::Draft => {
                                if ui.add_enabled(editor_open, egui::Button::new("Add below my notes")).clicked() {
                                    insert = true;
                                }
                                if ui.add_enabled(editor_open, egui::Button::new("Replace my notes")).clicked() {
                                    replace = true;
                                }
                                retry = ui.button("Write it again").clicked();
                            }
                            Kind::Reflection => {
                                if ui.add_enabled(editor_open, egui::Button::new("Use as a heading")).clicked() {
                                    heading = true;
                                }
                                another = ui.button("Another question").clicked();
                            }
                            Kind::Summary => {
                                save = ui.button("Save as an entry").clicked();
                                copy = ui.button("Copy").clicked();
                                retry = ui.button("Write it again").clicked();
                            }
                            Kind::Test => {}
                        });
                        if !editor_open && matches!(kind, Kind::Draft | Kind::Reflection) {
                            ui.label(RichText::new("Open an entry to use this.").small().color(p.muted));
                        }
                        ui.add_space(4.0);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                        ui.checkbox(&mut show_sent, "Show exactly what was sent to Ollama");
                    });
                    if show_sent {
                        ui.add_space(4.0);
                        egui::Frame::new().fill(p.head_bg).corner_radius(6).inner_margin(8).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new("Instructions").small().strong().color(p.head_text));
                            ui.add(egui::Label::new(RichText::new(&task.system).small().color(p.head_text)).wrap());
                            ui.add_space(4.0);
                            ui.label(RichText::new("Your text").small().strong().color(p.head_text));
                            ui.add(egui::Label::new(RichText::new(&task.user).small().color(p.head_text)).wrap());
                        });
                    }
                });
            });

        self.ai.show_sent = show_sent;
        let (text, date, system, user, title) = (task.text.clone(), task.date, task.system.clone(), task.user.clone(), task.title.clone());
        if cancel {
            task.cancel.store(true, Ordering::Relaxed);
            task.state = TaskState::Done;
        }
        if insert || replace {
            self.ai_insert_into_editor(&text, replace, false);
            self.ai.task = None;
        } else if heading {
            self.ai_insert_into_editor(&clean_question(&text), false, true);
            self.ai.task = None;
        } else if save {
            self.ai_save_summary(date, &text);
            self.ai.task = None;
        } else if another {
            self.ai_reflection(Some(clean_question(&text)));
        } else if retry {
            self.ai_start(kind, &title, system, user, date);
        }
        if copy {
            ctx.copy_text(text);
        }
        if open_setup {
            self.ai.setup_open = true;
        }
        if close {
            if let Some(t) = self.ai.task.take() {
                t.cancel.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Settings → AI helpers: install or start Ollama, choose a model, switch the helpers on.
    pub(crate) fn ai_setup_window(&mut self, ctx: &egui::Context) {
        if !self.ai.setup_open {
            return;
        }
        if self.ai.winget.is_none() {
            self.ai.winget = Some(ollama::winget_available());
        }
        let p = palette(ctx);
        let dark = ctx.theme() == egui::Theme::Dark;
        let status = self.ai.status.clone();
        let winget = self.ai.winget.unwrap_or(false);
        let (mut close, mut check, mut start_server, mut install, mut open_site, mut test) = (false, false, false, false, false, false);
        let mut pull_model: Option<String> = None;
        let mut cancel_pull = false;
        let mut choose: Option<String> = None;
        let mut enabled = self.data.settings.ai_enabled;
        let mut agree = self.ai.agree_install;
        let pull_info = self.ai.pull.as_ref().map(|pl| (pl.model.clone(), pl.progress.clone(), pl.error.clone()));
        let install_info = self.ai.install.as_ref().map(|i| (i.log.clone(), i.outcome.clone()));
        let current_model = self.data.settings.ai_model.clone();
        let ready = self.ai_ready();

        egui::Window::new("AI helpers")
            .title_bar(false)
            .frame(popup_frame(&p, dark))
            .resizable(true)
            .default_size([620.0, 640.0])
            .min_size([420.0, 360.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                close |= popup_header(ui, &p, "AI helpers");
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Frame::new().inner_margin(14).show(ui, |ui| {
                        ui.label(RichText::new("Three optional helpers").size(18.0).strong().color(p.title));
                        ui.label(RichText::new("Draft an entry from your rough notes · Summarise a day · Suggest a question to reflect on").color(p.ink));
                        ui.add_space(4.0);
                        ui.label(RichText::new("They use Ollama, a free program that runs AI models on this PC. Nothing you write is sent over the internet, and private entries are never sent at all. The helpers don't make up entries, and anything they write is marked as AI.").small().color(p.muted));
                        ui.add_space(10.0);
                        ui.separator();

                        // Step 1: Ollama itself.
                        ui.label(RichText::new("1. Ollama").size(17.0).strong().color(p.title));
                        match &status {
                            Status::Unknown => {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(RichText::new("Looking for Ollama…").color(p.muted));
                                });
                            }
                            Status::Running { version, models } => {
                                ui.label(RichText::new(format!("Ollama {version} is running. {} model{} installed.", models.len(), if models.len() == 1 { "" } else { "s" })).color(Color32::from_rgb(22, 163, 74)));
                            }
                            Status::Stopped(_) => {
                                ui.label(RichText::new("Ollama is installed but not running.").color(p.ink));
                                start_server = ui.button("Start Ollama").clicked();
                            }
                            Status::NotInstalled => {
                                ui.label(RichText::new("Ollama isn't installed.").color(p.ink));
                                ui.add_space(4.0);
                                if let Some((log, outcome)) = &install_info {
                                    match outcome {
                                        None => {
                                            ui.horizontal(|ui| {
                                                ui.spinner();
                                                ui.label(RichText::new("Installing Ollama… this is a large download and can take several minutes.").color(p.ink));
                                            });
                                        }
                                        Some(Err(e)) => {
                                            ui.add(egui::Label::new(RichText::new(e).color(Color32::from_rgb(239, 68, 68))).wrap());
                                        }
                                        Some(Ok(())) => {
                                            ui.label(RichText::new("Installed. Looking for it…").color(p.muted));
                                        }
                                    }
                                    for line in log.iter().rev().take(6).rev() {
                                        ui.label(RichText::new(line).small().monospace().color(p.muted));
                                    }
                                } else if winget {
                                    ui.checkbox(&mut agree, "Install Ollama for me. I understand this downloads a large file (hundreds of MB) from ollama.com using Windows' package manager (winget) and installs it for my user account.");
                                    install = ui.add_enabled(agree, egui::Button::new("Install Ollama")).clicked();
                                }
                                ui.add_space(4.0);
                                open_site = ui.button("Open the Ollama download page").clicked();
                                ui.label(RichText::new("After installing it yourself, come back here and press “Check again”.").small().color(p.muted));
                            }
                        }
                        check = ui.button("Check again").clicked();
                        ui.add_space(10.0);
                        ui.separator();

                        // Step 2: a model.
                        ui.label(RichText::new("2. A model").size(17.0).strong().color(p.title));
                        if let Status::Running { models, .. } = &status {
                            ui.label(RichText::new("The model writes the text. Bigger models write better but are slower and need more memory.").small().color(p.muted));
                            ui.add_space(4.0);
                            if !models.is_empty() {
                                ui.label(RichText::new("Installed (pick one to use):").color(p.ink));
                                for m in models {
                                    let on = model_matches(&m.name, &current_model);
                                    if ui.radio(on, format!("{}  ({})", m.name, human_size(m.size))).clicked() {
                                        choose = Some(m.name.clone());
                                    }
                                }
                                ui.add_space(6.0);
                            }
                            ui.label(RichText::new("Download one:").color(p.ink));
                            for (tag, what, size) in ollama::RECOMMENDED {
                                let have = models.iter().any(|m| model_matches(&m.name, tag));
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("{what}  ({size})")).color(p.ink));
                                    if have {
                                        ui.label(RichText::new("installed").small().color(p.muted));
                                    } else if ui.add_enabled(pull_info.is_none(), egui::Button::new("Download")).clicked() {
                                        pull_model = Some(tag.to_string());
                                    }
                                });
                            }
                            if let Some((model, progress, error)) = &pull_info {
                                ui.add_space(6.0);
                                if let Some(e) = error {
                                    ui.add(egui::Label::new(RichText::new(format!("Downloading {model} failed: {e}")).color(Color32::from_rgb(239, 68, 68))).wrap());
                                } else {
                                    let text = progress.as_ref().map_or("Starting…".to_string(), |pr| {
                                        match percent(pr) {
                                            Some(f) => format!("{} {:.0}% ({} of {})", pr.status, f * 100.0, human_size(pr.completed), human_size(pr.total)),
                                            None => pr.status.clone(),
                                        }
                                    });
                                    ui.label(RichText::new(format!("Downloading {model}")).color(p.ink));
                                    ui.add(egui::ProgressBar::new(progress.as_ref().and_then(percent).unwrap_or(0.0)).text(RichText::new(text).color(Color32::WHITE)).fill(accent_fill()));
                                    cancel_pull = ui.button("Cancel download").clicked();
                                }
                            }
                        } else {
                            ui.label(RichText::new("Available once Ollama is running.").color(p.muted));
                        }
                        ui.add_space(10.0);
                        ui.separator();

                        // Step 3: switch on.
                        ui.label(RichText::new("3. Turn the helpers on").size(17.0).strong().color(p.title));
                        let can_enable = matches!(status, Status::Running { .. }) && !current_model.is_empty();
                        ui.add_enabled_ui(can_enable, |ui| {
                            ui.checkbox(&mut enabled, "Use the AI helpers");
                        });
                        if !can_enable {
                            ui.label(RichText::new("Needs Ollama running and a model chosen.").small().color(p.muted));
                        } else if ready {
                            ui.label(RichText::new(format!("Ready. You'll see AI buttons in the editor (AI draft, AI question) and on a day (Summarise this day). Using {current_model}.")).small().color(p.muted));
                            test = ui.button("Test it").clicked();
                        }
                        if !self.ai.msg.is_empty() {
                            ui.add_space(6.0);
                            ui.label(RichText::new(&self.ai.msg).color(p.ink));
                        }
                    });
                });
            });

        self.ai.agree_install = agree;
        if check {
            self.ai.last_check = None;
            self.ai.msg.clear();
            self.ai_check_now();
        }
        if start_server {
            if let Status::Stopped(exe) = &status {
                self.ai.msg = match ollama::start_server(exe) {
                    Ok(()) => "Starting Ollama…".into(),
                    Err(e) => format!("Couldn't start Ollama: {e}"),
                };
                self.ai.last_check = None;
            }
        }
        if open_site {
            open_in_browser("https://ollama.com/download");
        }
if install {            self.ai_start_install();        }        if let Some(model) = pull_model {            self.ai_start_pull(model);        }
        if cancel_pull {
            if let Some(pl) = self.ai.pull.take() {
                pl.cancel.store(true, Ordering::Relaxed);
            }
        }
        if let Some(name) = choose {
            self.data.settings.ai_model = name;
            self.persist();
        }
        if enabled != self.data.settings.ai_enabled {
            self.data.settings.ai_enabled = enabled;
            self.persist();
        }
        if test {
            let today = Local::now().date_naive();
            self.ai_start(Kind::Test, "Test", TEST_SYSTEM.into(), TEST_USER.into(), today);
        }
        if close {
            self.ai.setup_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn entry(id: u64, date: &str, time: &str, text: &str) -> Entry {
        Entry { id, date: date.into(), added_at: format!("{date} {time}"), category: "Personal".into(), text: text.into(), ..Default::default() }
    }

    #[test]
    fn every_instruction_says_not_to_invent_things() {
        for (system, must) in [(DRAFT_SYSTEM, "ONLY the facts"), (SUMMARY_SYSTEM, "ONLY what the entries say")] {
            assert!(system.contains(must) && system.contains("nvent"), "{system}");
        }
        assert!(REFLECTION_SYSTEM.contains("single open question") && REFLECTION_SYSTEM.contains("not give advice"));
    }

    #[test]
    fn a_draft_request_carries_only_the_notes_and_context() {
        let (system, user) = draft_request("  walked dog\n- rained  ", d(2026, 10, 10), Some(4), "Personal");
        assert_eq!(system, DRAFT_SYSTEM);
        assert!(user.contains("Saturday 10 October 2026") && user.contains("How the day felt: Happy"));
        assert!(user.ends_with("My notes:\nwalked dog\n- rained"), "{user}");
        let (_, no_mood) = draft_request("x", d(2026, 10, 10), None, "Work");
        assert!(!no_mood.contains("How the day felt"));
        let (_, long) = draft_request(&"word ".repeat(2000), d(2026, 10, 10), None, "Work");
        assert!(long.chars().count() < 4300, "notes are cut to a sensible length");
    }

    #[test]
    fn private_and_ai_written_entries_are_never_sent() {
        let mut secret = entry(1, "2026-10-10", "09:00:00", "my secret thoughts");
        secret.sensitive = true;
        let mut ai = entry(2, "2026-10-10", "10:00:00", "an old AI summary");
        ai.generated = true;
        let public = entry(3, "2026-10-10", "11:00:00", "# Lunch\n\nHad **soup** with Sam.");
        let blank = entry(4, "2026-10-10", "12:00:00", "   ");

        assert!(summary_request(&[&secret, &ai, &blank], d(2026, 10, 10)).is_none(), "nothing shareable means no request");
        let (system, user) = summary_request(&[&secret, &ai, &public, &blank], d(2026, 10, 10)).unwrap();
        assert_eq!(system, SUMMARY_SYSTEM);
        assert!(user.contains("Had soup with Sam.") && user.contains("[11:00, Personal]"), "{user}");
        assert!(!user.contains("secret") && !user.contains("old AI summary"));

        let (_, reflect) = reflection_request(&[&secret, &ai, &public], None);
        assert!(reflect.contains("Lunch Had soup") && !reflect.contains("secret") && !reflect.contains("AI summary"), "{reflect}");
        let (_, none) = reflection_request(&[&secret], None);
        assert!(none.contains("no recent entries"));
        let (_, again) = reflection_request(&[&public], Some("What went well?"));
        assert!(again.ends_with("Do not repeat this earlier question: What went well?"));
    }

    #[test]
    fn entries_in_a_summary_are_in_time_order_with_moods() {
        let mut late = entry(1, "2026-10-10", "20:00:00", "evening");
        late.mood = Some(5);
        let early = entry(2, "2026-10-10", "08:00:00", "morning");
        let (_, user) = summary_request(&[&late, &early], d(2026, 10, 10)).unwrap();
        assert!(user.find("morning").unwrap() < user.find("evening").unwrap());
        assert!(user.contains("[20:00, Personal, mood: Really happy]"));
    }

    #[test]
    fn reflection_questions_are_cleaned_up() {
        assert_eq!(clean_question("  \"What made you smile today?\"  \n\nBecause..."), "What made you smile today?");
        assert_eq!(clean_question("1. What are you grateful for?"), "What are you grateful for?");
        assert_eq!(clean_question("- “How do you feel?”"), "How do you feel?");
        assert_eq!(clean_question(""), "");
    }

    #[test]
    fn model_names_match_with_or_without_latest() {
        assert!(model_matches("llama3.2:3b", "llama3.2:3b"));
        assert!(model_matches("mistral:latest", "mistral"));
        assert!(model_matches("mistral", "mistral:latest"));
        assert!(!model_matches("llama3.2:3b", "llama3.2:1b"));
    }

    // ---- end to end, against a fake Ollama on this PC ----

    use crate::ollama::tests::{Reply, serve};
    use std::sync::Mutex;

    /// The fake server is shared through a global, so these tests run one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    struct Fake {
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl Fake {
        fn start(base: &str) -> Fake {
            let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
            *ollama::TEST_BASE.lock().unwrap_or_else(|e| e.into_inner()) = Some(base.to_string());
            Fake { _guard: guard }
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            *ollama::TEST_BASE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
    }

    /// A fake Ollama with one model that answers every chat with `answer`, word by word.
    fn fake_ollama(answer: &'static str, seen: Arc<Mutex<Vec<String>>>) -> String {
        serve(move |r| match r.path.as_str() {
            "/api/version" => Reply::Json(200, r#"{"version":"0.5.1"}"#.into()),
            "/api/tags" => Reply::Json(200, r#"{"models":[{"name":"gemma2:2b","size":1629518495}]}"#.into()),
            "/api/chat" => {
                seen.lock().unwrap().push(r.body.clone());
                let mut lines: Vec<String> = answer
                    .split_inclusive(' ')
                    .map(|w| format!(r#"{{"message":{{"content":{}}},"done":false}}"#, serde_json::to_string(w).unwrap()))
                    .collect();
                lines.push(r#"{"message":{"content":""},"done":true}"#.into());
                Reply::Lines(lines)
            }
            "/api/pull" => Reply::Lines(vec![
                r#"{"status":"pulling manifest"}"#.into(),
                r#"{"status":"pulling abc","total":100,"completed":100}"#.into(),
                r#"{"status":"success"}"#.into(),
            ]),
            _ => Reply::Json(404, "{}".into()),
        })
    }

    fn new_app() -> DiaryApp {
        let mut app = DiaryApp::new();
        app.lock = None;
        app.data = fresh_data();
        app.selected = d(2026, 10, 10);
        app
    }

    fn pump(app: &mut DiaryApp, until: impl Fn(&DiaryApp) -> bool) {
        let ctx = egui::Context::default();
        for _ in 0..400 {
            app.ai_poll(&ctx);
            if until(app) {
                return;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        panic!("timed out waiting for the AI helpers");
    }

    fn switch_on(app: &mut DiaryApp) {
        app.data.settings.ai_enabled = true;
        app.data.settings.ai_model = "gemma2:2b".into();
        pump(app, |a| a.ai_ready());
    }

    #[test]
    fn the_helpers_are_off_until_ollama_runs_with_a_model_and_they_are_switched_on() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let _fake = Fake::start(&fake_ollama("hi", seen));
        let mut app = new_app();
        assert!(!app.ai_ready(), "off by default");
        // Ollama is running but the user hasn't switched the helpers on: still nothing happens.
        let ctx = egui::Context::default();
        app.ai_poll(&ctx);
        assert_eq!(app.ai.status, Status::Unknown);
        assert!(!app.ai_ready());

        app.data.settings.ai_enabled = true;
        pump(&mut app, |a| matches!(a.ai.status, Status::Running { .. }));
        assert!(!app.ai_ready(), "no model chosen yet");
        app.data.settings.ai_model = "not-installed:1b".into();
        assert!(!app.ai_ready(), "a model that isn't installed doesn't count");
        app.data.settings.ai_model = "gemma2:2b".into();
        assert!(app.ai_ready());
        app.data.settings.ai_enabled = false;
        assert!(!app.ai_ready(), "switching them off turns the buttons off");
    }

    #[test]
    fn a_draft_streams_in_and_goes_into_the_entry_marked_as_ai_assisted() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let _fake = Fake::start(&fake_ollama("I walked the dog in the rain.", seen.clone()));
        let mut app = new_app();
        switch_on(&mut app);

        app.new_entry(d(2026, 10, 10));
        app.editor.as_mut().unwrap().text = "walked dog\n- rain".into();
        app.editor.as_mut().unwrap().mood = Some(4);
        app.ai_draft();
        assert_eq!(app.ai.task.as_ref().unwrap().state, TaskState::Running);
        pump(&mut app, |a| a.ai.task.as_ref().is_some_and(|t| t.state == TaskState::Done));
        assert_eq!(app.ai.task.as_ref().unwrap().text, "I walked the dog in the rain.");

        // What was sent: the instructions and the notes, nothing else.
        let sent: serde_json::Value = serde_json::from_str(&seen.lock().unwrap()[0]).unwrap();
        assert_eq!(sent["model"], "gemma2:2b");
        assert_eq!(sent["messages"][0]["content"], DRAFT_SYSTEM);
        let user = sent["messages"][1]["content"].as_str().unwrap();
        assert!(user.contains("walked dog") && user.contains("Happy") && user.contains("Saturday 10 October 2026"));

        // Adding it below the notes keeps the notes and marks the entry as AI-assisted.
        let text = app.ai.task.as_ref().unwrap().text.clone();
        assert!(!app.editor.as_ref().unwrap().ai);
        app.ai_insert_into_editor(&text, false, false);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.text, "walked dog\n- rain\n\nI walked the dog in the rain.");
        assert!(ed.ai);
        // Replacing swaps the notes out.
        app.ai_insert_into_editor("A new version.", true, false);
        assert_eq!(app.editor.as_ref().unwrap().text, "A new version.");
    }

    #[test]
    fn a_day_summary_is_saved_as_an_ai_entry_that_does_not_count_as_your_writing() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let _fake = Fake::start(&fake_ollama("A quiet day with a long walk.", seen.clone()));
        let mut app = new_app();
        switch_on(&mut app);
        app.data.entries.push(entry(1, "2026-10-10", "09:00:00", "Went for a long walk by the river with Sam."));
        let mut secret = entry(2, "2026-10-10", "10:00:00", "Private worries about money");
        secret.sensitive = true;
        app.data.entries.push(secret);
        assert!(app.day_has_summarisable(d(2026, 10, 10)));
        assert!(!app.day_has_summarisable(d(2026, 10, 11)));

        app.ai_summarise_day(d(2026, 10, 10));
        pump(&mut app, |a| a.ai.task.as_ref().is_some_and(|t| t.state == TaskState::Done));
        let sent = seen.lock().unwrap()[0].clone();
        assert!(sent.contains("long walk by the river") && !sent.contains("worries about money"), "private text never leaves: {sent}");

        let words_before = stats::per_day(&app.data.entries)[&d(2026, 10, 10)].words;
        let text = app.ai.task.as_ref().unwrap().text.clone();
        app.ai_save_summary(d(2026, 10, 10), &text);
        let saved = app.data.entries.last().unwrap();
        assert!(saved.generated && saved.ai && saved.category == "AI");
        assert!(saved.text.contains("written by AI") && saved.text.contains("A quiet day with a long walk."));
        assert!(app.data.categories.iter().any(|c| c.name == "AI"), "an AI category is created");
        // It doesn't add to your words, entries or streaks.
        let day = &stats::per_day(&app.data.entries)[&d(2026, 10, 10)];
        assert_eq!((day.words, day.entries), (words_before, 2));
        // And it is never fed back into a later request.
        let all: Vec<&Entry> = app.data.entries.iter().collect();
        assert!(!summary_request(&all, d(2026, 10, 10)).unwrap().1.contains("quiet day"));
    }

    #[test]
    fn a_reflection_question_can_be_used_as_a_heading_or_asked_again() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let _fake = Fake::start(&fake_ollama("\"What made today feel slow?\"", seen.clone()));
        let mut app = new_app();
        switch_on(&mut app);
        app.data.entries.push(entry(1, "2026-10-09", "09:00:00", "A slow day at home."));
        app.new_entry(d(2026, 10, 10));
        app.ai_reflection(None);
        pump(&mut app, |a| a.ai.task.as_ref().is_some_and(|t| t.state == TaskState::Done));
        let q = clean_question(&app.ai.task.as_ref().unwrap().text);
        assert_eq!(q, "What made today feel slow?");
        app.ai_insert_into_editor(&q, false, true);
        assert_eq!(app.editor.as_ref().unwrap().text, "## What made today feel slow?\n\n");
        assert!(app.editor.as_ref().unwrap().ai);

        app.ai_reflection(Some(q));
        pump(&mut app, |a| a.ai.task.as_ref().is_some_and(|t| t.state == TaskState::Done));
        assert!(seen.lock().unwrap()[1].contains("Do not repeat this earlier question"));
    }

    #[test]
    fn if_ollama_stops_the_task_fails_clearly_and_the_status_is_rechecked() {
        let _fake = Fake::start("http://127.0.0.1:1"); // nothing listens here
        let mut app = new_app();
        app.data.settings.ai_enabled = true;
        app.data.settings.ai_model = "gemma2:2b".into();
        app.ai.status = Status::Running { version: "x".into(), models: vec![ModelInfo { name: "gemma2:2b".into(), size: 1 }] };
        assert!(app.ai_ready(), "it looked fine a moment ago");
        app.ai_start(Kind::Test, "Test", "s".into(), "u".into(), d(2026, 10, 10));
        pump(&mut app, |a| matches!(a.ai.task.as_ref().map(|t| &t.state), Some(TaskState::Failed(_))));
        let TaskState::Failed(e) = &app.ai.task.as_ref().unwrap().state else { unreachable!() };
        assert!(e.contains("isn't running"), "{e}");
        // The re-check notices Ollama is gone, so the AI buttons disappear again.
        pump(&mut app, |a| !a.ai_ready());
        assert!(matches!(app.ai.status, Status::NotInstalled | Status::Stopped(_)));
    }

    #[test]
    fn downloading_a_model_selects_it_and_updates_the_status() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let _fake = Fake::start(&fake_ollama("x", seen));
        let mut app = new_app();
        app.ai.setup_open = true;
        pump(&mut app, |a| matches!(a.ai.status, Status::Running { .. }));
        assert_eq!(app.data.settings.ai_model, "");
        app.ai_start_pull("gemma2:2b".into());
        assert!(app.ai.pull.is_some());
        pump(&mut app, |a| a.ai.pull.is_none());
        assert_eq!(app.data.settings.ai_model, "gemma2:2b", "the first model downloaded is chosen");
        assert!(app.ai.msg.contains("Downloaded gemma2:2b"));
    }

    #[test]
    fn the_setup_and_result_windows_render_in_every_state() {
        let _lock = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut app = new_app();
        app.ai.setup_open = true;
        let models = vec![ModelInfo { name: "gemma2:2b".into(), size: 1_629_518_495 }];
        let statuses = [
            Status::Unknown,
            Status::NotInstalled,
            Status::Stopped(std::path::PathBuf::from("C:/ollama.exe")),
            Status::Running { version: "0.5.1".into(), models: Vec::new() },
            Status::Running { version: "0.5.1".into(), models },
        ];
        for status in statuses {
            app.ai.status = status;
            app.ai.winget = Some(true);
            for _ in 0..2 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.ai_setup_window(ui.ctx()));
                out.textures_delta.clear();
            }
        }
        // With a download and an install under way.
        let (_tx, rx) = mpsc::channel();
        app.ai.pull = Some(Pull {
            model: "gemma2:2b".into(),
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Some(Progress { status: "pulling".into(), completed: 50, total: 100 }),
            error: None,
        });
        let (_tx2, rx2) = mpsc::channel();
        app.ai.install = Some(Install { rx: rx2, log: vec!["Downloading…".into()], outcome: None });
        app.ai.status = Status::NotInstalled;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.ai_setup_window(ui.ctx()));
        out.textures_delta.clear();

        // The result window while running, done, and failed, for each kind of request.
        for kind in [Kind::Draft, Kind::Summary, Kind::Reflection, Kind::Test] {
            for state in [TaskState::Running, TaskState::Done, TaskState::Failed("Ollama isn't running.".into())] {
                let (_t, rx) = mpsc::channel();
                app.ai.task = Some(Task {
                    kind,
                    title: "Test".into(),
                    system: "sys".into(),
                    user: "usr".into(),
                    date: d(2026, 10, 10),
                    text: "Some text".into(),
                    state,
                    rx,
                    cancel: Arc::new(AtomicBool::new(false)),
                });
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.ai_task_window(ui.ctx()));
                out.textures_delta.clear();
            }
        }
    }
}
