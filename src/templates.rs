//! Writing templates and prompts: ready-made starting points for an entry, so there is never a
//! blank page. Everything here is built in or stored in the encrypted diary; nothing needs AI.

use super::*;

/// A template you wrote yourself (the built-in ones are in [`BUILTIN`]).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Template {
    pub name: String,
    pub body: String,
    /// Fields written by a newer version of the app: kept untouched so saving never loses them.
    #[serde(flatten, default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Name and Markdown body. `{date}`, `{weekday}` and `{time}` are filled in when a template is used.
pub(crate) const BUILTIN: [(&str, &str); 8] = [
    ("Gratitude", "## Three things I'm grateful for\n\n1. \n2. \n3. \n\n## Why they mattered today\n\n"),
    (
        "Daily review",
        "## Highlight of the day\n\n\n## Something that was hard\n\n\n## What I learned\n\n\n## Tomorrow I want to\n\n- \n",
    ),
    ("Evening wind-down", "## What happened today\n\n\n## What I'm letting go of\n\n\n## Looking forward to\n\n"),
    (
        "Weekly reflection",
        "## The week in a sentence\n\n\n## What went well\n\n- \n\n## What didn't\n\n- \n\n## One thing to change next week\n\n",
    ),
    ("Mood check-in", "## How am I feeling right now?\n\n\n## What's behind it?\n\n\n## What would help?\n\n"),
    ("Goals check-in", "## Where I am with my goals\n\n- [ ] \n- [ ] \n- [ ] \n\n## What I'll do next\n\n"),
    ("Dream log", "## What I remember\n\n\n## How it felt\n\n\n## What it might connect to\n\n"),
    ("Travel log", "## Where I am\n\n\n## What I saw and did\n\n\n## Highlights\n\n- \n\n## Things to remember\n\n"),
];

/// Short questions to get started with.
pub(crate) const PROMPTS: [&str; 40] = [
    "What made you smile today?",
    "What is taking up the most space in your mind right now?",
    "What are you looking forward to?",
    "Describe a small moment from today that you want to remember.",
    "What did you learn this week?",
    "Who are you grateful for, and why?",
    "What is something you're proud of lately?",
    "What would make tomorrow a good day?",
    "What have you been avoiding, and why?",
    "How did you take care of yourself today?",
    "What surprised you today?",
    "What would you tell yourself a year ago?",
    "What does a perfect ordinary day look like for you?",
    "What are you curious about at the moment?",
    "What is one thing you'd like to let go of?",
    "What was the hardest part of today, and how did you handle it?",
    "What kind thing has someone done for you recently?",
    "What do you want more of in your life? What do you want less of?",
    "Describe where you are right now, using all your senses.",
    "What song, smell or taste reminds you of a happy time?",
    "What are you most looking forward to this season?",
    "What did you do today that was just for you?",
    "What decision are you facing, and what does your gut say?",
    "What made today different from yesterday?",
    "What are three small wins from this week?",
    "Who would you like to get in touch with, and what would you say?",
    "What have you outgrown?",
    "What do you need to forgive yourself for?",
    "Write about someone who shaped who you are.",
    "What are you afraid of, and what is the worst that could realistically happen?",
    "What did you notice outside today?",
    "What are your priorities for the next month?",
    "What are you saying yes to, and what are you saying no to?",
    "What would you do with an extra free hour tomorrow?",
    "What does rest look like for you?",
    "What are you building towards?",
    "Describe your mood as weather.",
    "What is a memory you have from this time of year?",
    "What do you want to remember about this stage of your life?",
    "What would make your days feel more like your own?",
];

/// The prompt for a day. It stays the same all day; `offset` steps to the next ones.
pub(crate) fn prompt_for(date: NaiveDate, offset: usize) -> &'static str {
    PROMPTS[(date.num_days_from_ce().unsigned_abs() as usize + offset) % PROMPTS.len()]
}

/// Fill in `{date}`, `{weekday}` and `{time}`.
pub(crate) fn expand(body: &str, date: NaiveDate, hour: u32, minute: u32) -> String {
    body.replace("{date}", date.format("%e %B %Y").to_string().trim())
        .replace("{weekday}", &date.format("%A").to_string())
        .replace("{time}", &format!("{hour:02}:{minute:02}"))
}

/// A template being edited in the Templates window.
#[derive(Default)]
pub(crate) struct Draft {
    /// Which of the user's templates this replaces; `None` for a new one.
    pub index: Option<usize>,
    pub name: String,
    pub body: String,
}

#[derive(Default)]
pub(crate) struct TemplateState {
    pub open: bool,
    pub draft: Option<Draft>,
    pub msg: String,
}

impl DiaryApp {
    /// Names of every template: the built-in ones, then yours.
    pub(crate) fn template_names(&self) -> Vec<String> {
        BUILTIN.iter().map(|(n, _)| n.to_string()).chain(self.data.templates.iter().map(|t| t.name.clone())).collect()
    }

    pub(crate) fn template_body(&self, name: &str) -> Option<String> {
        self.data
            .templates
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.body.clone())
            .or_else(|| BUILTIN.iter().find(|(n, _)| *n == name).map(|(_, b)| b.to_string()))
    }

    /// Put a template into the entry being written: it becomes the text if the page is empty,
    /// otherwise it is added at the end.
    pub(crate) fn apply_template(&mut self, name: &str) {
        let Some(body) = self.template_body(name) else { return };
        let Some(ed) = self.editor.as_mut() else { return };
        let text = expand(&body, ed.date, ed.hour, ed.minute);
        ed.text = if ed.text.trim().is_empty() { text } else { format!("{}\n\n{}", ed.text.trim_end(), text) };
        ed.want_focus = Some((usize::MAX, usize::MAX));
    }

    pub(crate) fn templates_window(&mut self, ctx: &egui::Context) {
        if !self.templates.open {
            return;
        }
        let p = palette(ctx);
        let dark = ctx.theme() == egui::Theme::Dark;
        let mut close = false;
        let mut start: Option<Draft> = None;
        let mut delete = None;
        let mut save = false;
        let mut cancel = false;
        let mut default_pick: Option<String> = None;
        let current_default = self.data.settings.default_template.clone();
        let names: Vec<(String, bool)> = BUILTIN.iter().map(|(n, _)| (n.to_string(), true)).chain(self.data.templates.iter().map(|t| (t.name.clone(), false))).collect();

        egui::Window::new("Templates")
            .title_bar(false)
            .frame(popup_frame(&p, dark))
            .resizable(true)
            .default_size([560.0, 560.0])
            .min_size([400.0, 320.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                close |= popup_header(ui, &p, "Templates");
                egui::Frame::new().inner_margin(14).show(ui, |ui| {
                    ui.label(RichText::new("Templates fill a new entry with headings and questions to answer. Use {date}, {weekday} and {time} in your own to insert them automatically.").color(p.muted));
                    ui.add_space(6.0);

                    if let Some(draft) = self.templates.draft.as_mut() {
                        ui.label(RichText::new(if draft.index.is_some() { "Edit template" } else { "New template" }).size(18.0).strong().color(p.title));
                        ui.add_space(4.0);
                        ui.add(egui::TextEdit::singleline(&mut draft.name).hint_text("Name").text_color(p.ink).desired_width(f32::INFINITY));
                        ui.add_space(4.0);
                        ui.add(
                            egui::TextEdit::multiline(&mut draft.body)
                                .hint_text("The text to start an entry with (Markdown works: ## headings, - lists)")
                                .text_color(p.ink)
                                .desired_rows(10)
                                .desired_width(f32::INFINITY),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            save = ui.button("Save").clicked();
                            cancel = ui.button("Cancel").clicked();
                        });
                    } else {
                        ui.horizontal(|ui| {
                            if ui.button("New template").clicked() {
                                start = Some(Draft::default());
                            }
                            ui.label(RichText::new("Used for every new entry:").color(p.muted));
                            let shown = if current_default.is_empty() { "None".to_string() } else { current_default.clone() };
                            egui::ComboBox::from_id_salt("default_template").selected_text(shown).show_ui(ui, |ui| {
                                if ui.selectable_label(current_default.is_empty(), "None").clicked() {
                                    default_pick = Some(String::new());
                                }
                                for (name, _) in &names {
                                    if ui.selectable_label(&current_default == name, name).clicked() {
                                        default_pick = Some(name.clone());
                                    }
                                }
                            });
                        });
                        ui.add_space(8.0);
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            for (name, builtin) in &names {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(name).strong().color(p.ink));
                                    ui.label(RichText::new(if *builtin { "built in" } else { "yours" }).small().color(p.muted));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if *builtin {
                                            if ui.button("Make my own copy").clicked() {
                                                let body = BUILTIN.iter().find(|(n, _)| n == name).map(|(_, b)| b.to_string()).unwrap_or_default();
                                                start = Some(Draft { index: None, name: format!("{name} (mine)"), body });
                                            }
                                        } else if let Some(i) = self.data.templates.iter().position(|t| &t.name == name) {
                                            if ui.add(egui::Button::new(RichText::new("Delete").color(theme::BG)).fill(RED)).clicked() {
                                                delete = Some(i);
                                            }
                                            if ui.button("Edit").clicked() {
                                                let t = &self.data.templates[i];
                                                start = Some(Draft { index: Some(i), name: t.name.clone(), body: t.body.clone() });
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                            }
                        });
                    }
                    if !self.templates.msg.is_empty() {
                        ui.add(egui::Label::new(RichText::new(&self.templates.msg).color(Color32::from_rgb(239, 68, 68))).wrap());
                    }
                });
            });

        if let Some(d) = start {
            self.templates.draft = Some(d);
            self.templates.msg.clear();
        }
        if cancel {
            self.templates.draft = None;
            self.templates.msg.clear();
        }
        if save {
            self.save_template_draft();
        }
        if let Some(i) = delete {
            let removed = self.data.templates.remove(i);
            if self.data.settings.default_template == removed.name {
                self.data.settings.default_template.clear();
            }
            self.persist();
        }
        if let Some(name) = default_pick {
            self.data.settings.default_template = name;
            self.persist();
        }
        if close {
            self.templates.open = false;
            self.templates.draft = None;
            self.templates.msg.clear();
        }
    }

    fn save_template_draft(&mut self) {
        let Some(draft) = self.templates.draft.take() else { return };
        let name = draft.name.trim().to_string();
        let taken = BUILTIN.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name))
            || self.data.templates.iter().enumerate().any(|(i, t)| Some(i) != draft.index && t.name.eq_ignore_ascii_case(&name));
        if name.is_empty() {
            self.templates.msg = "Give the template a name.".into();
        } else if taken {
            self.templates.msg = "There is already a template with that name.".into();
        } else if draft.body.trim().is_empty() {
            self.templates.msg = "The template is empty.".into();
        } else {
            let renamed = draft.index.and_then(|i| self.data.templates.get(i)).map(|t| t.name.clone());
            let template = Template { name: name.clone(), body: draft.body.clone(), extra: Default::default() };
            match draft.index {
                Some(i) if i < self.data.templates.len() => self.data.templates[i] = template,
                _ => self.data.templates.push(template),
            }
            if renamed.as_deref() == Some(self.data.settings.default_template.as_str()) {
                self.data.settings.default_template = name;
            }
            self.templates.msg.clear();
            self.persist();
            return;
        }
        // Something is wrong with it: keep the draft so nothing typed is lost.
        self.templates.draft = Some(draft);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn placeholders_are_filled_in() {
        let out = expand("# {weekday}, {date} at {time}", d(2026, 10, 9), 7, 5);
        assert_eq!(out, "# Friday, 9 October 2026 at 07:05");
        assert_eq!(expand("no placeholders", d(2026, 1, 1), 0, 0), "no placeholders");
    }

    #[test]
    fn prompts_are_steady_within_a_day_and_step_on_request() {
        let day = d(2026, 10, 9);
        assert_eq!(prompt_for(day, 0), prompt_for(day, 0));
        assert_ne!(prompt_for(day, 0), prompt_for(day, 1));
        assert_ne!(prompt_for(day, 0), prompt_for(day + chrono::Days::new(1), 0));
        assert_eq!(prompt_for(day, 0), prompt_for(day, PROMPTS.len()), "it wraps around");
        // Every prompt is a real, distinct question.
        let unique: std::collections::HashSet<&str> = PROMPTS.iter().copied().collect();
        assert_eq!(unique.len(), PROMPTS.len());
        assert!(PROMPTS.iter().all(|p| !p.trim().is_empty()));
    }

    #[test]
    fn built_in_templates_are_distinct_and_usable() {
        let names: std::collections::HashSet<&str> = BUILTIN.iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), BUILTIN.len());
        for (name, body) in BUILTIN {
            assert!(body.contains("## "), "{name} has headings");
            assert!(!markdown::summary(body).is_empty(), "{name} renders");
        }
    }
}

#[cfg(test)]
impl DiaryApp {
    pub(crate) fn save_template_draft_for_test(&mut self) {
        self.save_template_draft();
    }
}
