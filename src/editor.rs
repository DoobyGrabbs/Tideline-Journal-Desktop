//! The writing view: a full-window page for a new or existing entry (not a pop-up). A top bar
//! with Discard and Save, a centred column with the entry's details, mood, an optional writing
//! prompt, and a card holding the formatting toolbar, an optional title and the text.

use super::*;
use crate::kit::{self, Icon, Look, Tool};

/// Reading speed used for "min read".
const WORDS_PER_MINUTE: usize = 200;

impl EditState {
    /// The title and text as one Markdown document (the title is a first-line `# Heading`).
    pub(crate) fn full_text(&self) -> String {
        markdown::join_title(&self.title, &self.text)
    }

    pub(crate) fn has_content(&self) -> bool {
        !self.title.trim().is_empty() || !self.text.trim().is_empty()
    }

    /// Has anything been written since the entry was opened?
    pub(crate) fn changed(&self) -> bool {
        self.full_text() != self.baseline
    }

    /// The Discard / Back button was pressed. True means leave now; otherwise the button is
    /// armed and the next press discards.
    pub(crate) fn press_discard(&mut self) -> bool {
        if !self.changed() || self.discard_arm {
            true
        } else {
            self.discard_arm = true;
            false
        }
    }
}

pub(crate) fn read_minutes(words: usize) -> usize {
    words.div_ceil(WORDS_PER_MINUTE)
}

impl DiaryApp {
    pub(crate) fn entry_view(&mut self, ui: &mut egui::Ui) {
        if self.editor.is_none() {
            return;
        }
        let ctx = ui.ctx().clone();
        let p = palette(&ctx);
        let shades = theme::shades();
        let template_names = self.template_names();
        let ai_ready = self.ai_ready();
        let (mut ask_draft, mut ask_question, mut open_ai_setup) = (false, false, false);
        let (mut chosen_template, mut open_templates, mut persist_settings) = (None::<String>, false, false);
        let DiaryApp { editor, data, media, show_marks, editor_msg, save_requested, .. } = self;
        let ed = editor.as_mut().unwrap();
        let mut save_it = false;
        let mut cancel = false;
        let mut discard_clicked = false;
        if std::mem::take(save_requested) && ed.has_content() {
            save_it = true;
        }

        // Keyboard shortcuts for the common formats.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D)) {
            ed.pinned = !ed.pinned;
        }
        let shortcuts = [(egui::Key::B, markdown::Format::Bold), (egui::Key::I, markdown::Format::Italic)];
        for (key, format) in shortcuts {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, key)) {
                apply_format(&ctx, ed, format);
            }
        }
        // Images and files dropped onto the window.
        for file in ctx.input(|i| i.raw.dropped_files.clone()) {
            let path = file.path();
            if is_image_path(path) {
                match import_image(data, path) {
                    Ok(md) => insert_image(&ctx, ed, &md),
                    Err(e) => *editor_msg = e,
                }
            } else {
                match import_attachment(data, path) {
                    Ok(a) => {
                        if is_risky_file(&a.name) {
                            *editor_msg = format!("“{}” {RISKY_HINT_SHORT}", a.name);
                        }
                        ed.attachments.push(a);
                    }
                    Err(e) => *editor_msg = e,
                }
            }
        }

        // ---- top bar ------------------------------------------------------------------------
        egui::Panel::top("entry_bar")
            .exact_size(64.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(p.panel_bg).stroke(Stroke::new(1.0, p.border_soft)).inner_margin(egui::Margin::symmetric(28, 0)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let (r, back) = ui.allocate_exact_size(vec2(112.0, 36.0), Sense::click());
                    let fg = if back.hovered() { theme::TEXT } else { theme::TEXT_2 };
                    kit::paint(ui.painter(), egui::pos2(r.min.x + 12.0, r.center().y), 18.0, fg, Icon::Back, false);
                    ui.painter().text(egui::pos2(r.min.x + 32.0, r.center().y), Align2::LEFT_CENTER, "Calendar", FontId::new(15.0, fonts::ui_bold()), fg);
                    if back.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Back to the calendar").clicked() {
                        discard_clicked = true;
                    }
                    ui.label(RichText::new("/").size(16.0).color(p.faint));
                    ui.label(RichText::new(if ed.id.is_some() { "Edit entry" } else { "New entry" }).size(15.0).color(p.muted));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        let can_save = ed.has_content();
                        let saved = ui.add_enabled_ui(can_save, |ui| kit::button(ui, Look::Primary, None, "Save entry", None)).inner;
                        if saved.on_hover_text("Save (Ctrl+S)").clicked() {
                            save_it = true;
                        }
                        let label = if ed.discard_arm { "Click again to discard" } else { "Discard" };
                        if kit::button(ui, Look::Ghost, None, label, None).clicked() {
                            discard_clicked = true;
                        }
                        if ed.changed() {
                            ui.label(RichText::new("Unsaved draft").size(13.5).color(p.faint));
                            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                            ui.painter().circle_filled(dot.center(), 3.5, theme::TODAY);
                        }
                    });
                });
            });

        // ---- the page ------------------------------------------------------------------------
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.page_bg).inner_margin(egui::Margin::symmetric(24, 0))).show(ui, |ui| {
            let page_h = ui.available_height();
            egui::ScrollArea::vertical().id_salt("entry_page").auto_shrink([false, false]).show(ui, |ui| {
                let col_w = (ui.available_width() - 24.0).min(780.0);
                let side = ((ui.available_width() - col_w) / 2.0).max(0.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(side);
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        ui.spacing_mut().item_spacing.y = 16.0;
                        ui.add_space(26.0);

                        // Date, time, category, and the Pin / Private switches.
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let date_chip = kit::chip(ui, Some(Icon::Calendar), None, &ed.date.format("%A %-d %B %Y").to_string(), None, false, false);
                            egui::Popup::menu(&date_chip).show(|ui| {
                                if kit::date_picker(ui, &mut ed.date) {
                                    ui.close();
                                }
                            });
                            let time_chip = kit::chip(ui, Some(Icon::Clock), None, &format!("{:02}:{:02}", ed.hour, ed.minute), None, false, false);
                            egui::Popup::menu(&time_chip).show(|ui| {
                                ui.set_min_width(190.0);
                                ui.horizontal(|ui| {
                                    let two = |n: f64, _: std::ops::RangeInclusive<usize>| format!("{:02}", n as u32);
                                    ui.add(egui::DragValue::new(&mut ed.hour).range(0..=23).custom_formatter(two));
                                    ui.label(":");
                                    ui.add(egui::DragValue::new(&mut ed.minute).range(0..=59).custom_formatter(two));
                                    if kit::small_button(ui, Look::Ghost, None, "Now").clicked() {
                                        let now = Local::now();
                                        ed.hour = now.hour();
                                        ed.minute = now.minute();
                                    }
                                });
                            });
                            let cat_col = category_color(&data.categories, &ed.category);
                            let cat_chip = kit::chip(ui, None, Some(cat_col), &ed.category, Some(cat_col), false, true);
                            egui::Popup::menu(&cat_chip).show(|ui| {
                                ui.set_min_width(200.0);
                                for c in &data.categories {
                                    if kit::nav_item(ui, None, Some(rgb(c.color)), &c.name, None, c.name == ed.category).clicked() {
                                        ed.category = c.name.clone();
                                        ui.close();
                                    }
                                }
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 10.0;
                                if kit::chip(ui, Some(Icon::Lock), None, "Private", None, ed.sensitive, false).on_hover_text("Hide this entry until you click it").clicked() {
                                    ed.sensitive = !ed.sensitive;
                                }
                                if kit::chip(ui, Some(Icon::Star), None, "Pin", None, ed.pinned, false).on_hover_text("List this entry under Pinned and star it on the calendar").clicked() {
                                    ed.pinned = !ed.pinned;
                                }
                            });
                        });

                        // How are you feeling?
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                            ui.label(RichText::new("How are you feeling?").size(15.0).color(p.muted));
                            for m in 1..=5u8 {
                                let on = ed.mood == Some(m);
                                if kit::mood_pill(ui, m, on).on_hover_text("Click again to clear").clicked() {
                                    ed.mood = if on { None } else { Some(m) };
                                }
                            }
                        });

                        // A gentle nudge for a blank page; it disappears as soon as you start writing.
                        if data.settings.show_prompts {
                            let prompt = templates::prompt_for(ed.date, ed.prompt_offset);
                            let card = egui::Frame::new()
                                .fill(shades.prompt_bg)
                                .stroke(Stroke::new(1.0, shades.prompt_border))
                                .corner_radius(14)
                                .inner_margin(egui::Margin::same(20))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.spacing_mut().item_spacing.y = 10.0;
                                    ui.horizontal(|ui| {
                                        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                                        kit::paint(ui.painter(), r.center(), 17.0, shades.accent, Icon::Sparkle, false);
                                        let spaced: String = "TODAY'S PROMPT".chars().flat_map(|c| [c, '\u{200A}']).collect();
                                        ui.label(RichText::new(spaced).font(FontId::new(12.0, fonts::ui_bold())).color(shades.accent));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let (r, hide) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                                            kit::paint(ui.painter(), r.center(), 16.0, if hide.hovered() { theme::TEXT } else { theme::TEXT_3 }, Icon::Close, false);
                                            if hide.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Hide prompts (they can be turned back on under Settings)").clicked() {
                                                data.settings.show_prompts = false;
                                                persist_settings = true;
                                            }
                                        });
                                    });
                                    ui.label(RichText::new(prompt).font(FontId::new(21.0, fonts::serif_italic())).color(theme::TEXT));
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 10.0;
                                        if kit::small_button(ui, Look::Primary, None, "Use this prompt").clicked() {
                                            DiaryApp::insert_prompt_at_cursor(&ctx, ed, prompt);
                                        }
                                        if kit::small_button(ui, Look::Ghost, None, "Another").clicked() {
                                            ed.prompt_offset += 1;
                                        }
                                    });
                                });
                            let _ = card;
                        }

                        // The writing card: toolbar, title, text, footer.
                        egui::Frame::new().fill(theme::WRITING_CARD).stroke(Stroke::new(1.0, p.border_soft)).corner_radius(14).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);

                            // Toolbar.
                            egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 9)).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    use markdown::Format::*;
                                    let groups: [&[(Tool, &str, markdown::Format)]; 3] = [
                                        &[(Tool::Bold, "Bold (Ctrl+B)", Bold), (Tool::Italic, "Italic (Ctrl+I)", Italic), (Tool::Strike, "Strikethrough", Strike)],
                                        &[(Tool::Heading(1), "Large heading", Heading(1)), (Tool::Heading(2), "Medium heading", Heading(2)), (Tool::Heading(3), "Small heading", Heading(3))],
                                        &[
                                            (Tool::Icon(Icon::Bullets), "Bulleted list", Bullet),
                                            (Tool::Icon(Icon::Numbers), "Numbered list", Numbered),
                                            (Tool::Icon(Icon::Quote), "Quote", Quote),
                                        ],
                                    ];
                                    for (n, group) in groups.iter().enumerate() {
                                        if n > 0 {
                                            kit::tool_divider(ui);
                                        }
                                        for (tool, tip, format) in group.iter() {
                                            if kit::tool_button(ui, *tool, tip).clicked() {
                                                apply_format(&ctx, ed, *format);
                                            }
                                        }
                                    }
                                    kit::tool_divider(ui);
                                    if kit::tool_button(ui, Tool::Icon(Icon::Image), "Add a picture (or drop one onto this window)").clicked() {
                                        let picked = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "webp"]).pick_file();
                                        if let Some(path) = picked {
                                            match import_image(data, &path) {
                                                Ok(md) => {
                                                    insert_image(&ctx, ed, &md);
                                                    editor_msg.clear();
                                                }
                                                Err(e) => *editor_msg = e,
                                            }
                                        }
                                    }
                                    if kit::tool_button(ui, Tool::Icon(Icon::Paperclip), "Attach any file (up to 25 MB), or drop one onto this window").clicked() {
                                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                                            match import_attachment(data, &path) {
                                                Ok(a) => {
                                                    *editor_msg = if is_risky_file(&a.name) { format!("“{}” {RISKY_HINT_SHORT}", a.name) } else { String::new() };
                                                    ed.attachments.push(a);
                                                }
                                                Err(e) => *editor_msg = e,
                                            }
                                        }
                                    }
                                    let template = kit::small_button(ui, Look::Ghost, None, "Template");
                                    egui::Popup::menu(&template).show(|ui| {
                                        ui.set_min_width(200.0);
                                        for name in &template_names {
                                            if ui.button(name).clicked() {
                                                chosen_template = Some(name.clone());
                                                ui.close();
                                            }
                                        }
                                        ui.separator();
                                        if ui.button("Manage templates…").clicked() {
                                            open_templates = true;
                                            ui.close();
                                        }
                                    });
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        let ask = kit::small_button(ui, Look::AccentOutline, Some(Icon::Sparkle), "Ask AI");
                                        egui::Popup::menu(&ask).show(|ui| {
                                            ui.set_min_width(260.0);
                                            if ai_ready {
                                                let can_draft = !ed.text.trim().is_empty();
                                                if ui
                                                    .add_enabled(can_draft, egui::Button::new("Turn my notes into an entry"))
                                                    .on_hover_text("It runs on this PC, and anything it writes is marked as AI-assisted.")
                                                    .clicked()
                                                {
                                                    ask_draft = true;
                                                    ui.close();
                                                }
                                                if ui.button("Suggest a question to reflect on").on_hover_text("Based on your recent entries").clicked() {
                                                    ask_question = true;
                                                    ui.close();
                                                }
                                            } else {
                                                ui.label(RichText::new("The AI helpers run on this PC and are switched off.").size(13.0).color(theme::TEXT_3));
                                                if ui.button("Set up AI helpers…").clicked() {
                                                    open_ai_setup = true;
                                                    ui.close();
                                                }
                                            }
                                        });
                                    });
                                });
                            });
                            let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                            ui.painter().rect_filled(line, 0.0, p.border_soft);

                            // Title and text.
                            egui::Frame::new().inner_margin(egui::Margin { left: 30, right: 30, top: 24, bottom: 8 }).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.spacing_mut().item_spacing.y = 6.0;
                                let title_font = FontId::new(32.0, fonts::serif_medium());
                                let title = ui.add(
                                    egui::TextEdit::singleline(&mut ed.title)
                                        .font(title_font.clone())
                                        .hint_text(RichText::new("Give today a title (optional)").font(title_font).color(theme::TEXT_3))
                                        .text_color(theme::TEXT)
                                        .frame(egui::Frame::NONE)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.add_space(8.0);
                                if title.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    ed.want_focus = Some((0, usize::MAX));
                                }

                                let body_font = FontId::new(18.0, fonts::serif());
                                let st = live::LiveStyle { ink: theme::TEXT, muted: theme::TEXT_3, accent: shades.accent, size: 18.0, serif: true, line_height: 1.65 };
                                let marks = *show_marks;
                                let rows = (((page_h - 520.0).clamp(200.0, 900.0)) / (18.0 * 1.65)) as usize;
                                let mut segs = live::split(&ed.text);
                                let count = segs.len();
                                let is_text: Vec<bool> = segs.iter().map(|s| matches!(s, live::Seg::Text(_))).collect();
                                let mut changed = false;
                                let mut remove = None;
                                for i in 0..count {
                                    match &mut segs[i] {
                                        live::Seg::Text(s) => {
                                            let id = seg_id(i);
                                            let want = match ed.want_focus {
                                                Some((w, pos)) if w == i || (w == usize::MAX && i == count - 1) => Some(pos),
                                                _ => None,
                                            };
                                            if let Some(pos) = want {
                                                focus_at(&ctx, id, s, pos);
                                                ed.want_focus = None;
                                            }
                                            // Formatting marks stay hidden unless the option is on; the cursor steps over them.
                                            let mut keys_edited = false;
                                            if !marks && ctx.memory(|m| m.has_focus(id)) {
                                                let hidden = live::layout_with_hidden(s.as_str(), None, false, &st, 1000.0).1;
                                                keys_edited = step_over_hidden_marks(&ctx, id, s, &hidden);
                                            }
                                            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                                                let job = live::layout(text.as_str(), None, marks, &st, wrap);
                                                ui.fonts_mut(|f| f.layout_job(job))
                                            };
                                            let last = i == count - 1;
                                            let resp = ui.add(
                                                egui::TextEdit::multiline(s)
                                                    .id(id)
                                                    .font(body_font.clone())
                                                    .frame(egui::Frame::NONE)
                                                    .hint_text(RichText::new(if count == 1 { "What's on your mind?" } else { "" }).font(body_font.clone()).color(theme::TEXT_3))
                                                    .desired_width(f32::INFINITY)
                                                    .desired_rows(if last { rows.max(8) } else { 1 })
                                                    .layouter(&mut layouter),
                                            );
                                            changed |= resp.changed() || keys_edited;
                                            if !marks && resp.has_focus() && ctx.input(|i| !i.events.is_empty()) {
                                                let hidden = live::layout_with_hidden(s.as_str(), None, false, &st, 1000.0).1;
                                                snap_out_of_marks(&ctx, id, s.as_str(), &hidden);
                                            }
                                            if resp.has_focus() {
                                                ed.focus = i;
                                                // Let the arrow keys travel past pictures into the next text block.
                                                let (down, up) = ctx.input(|inp| (inp.key_pressed(egui::Key::ArrowDown), inp.key_pressed(egui::Key::ArrowUp)));
                                                if down || up {
                                                    let cursor = egui::TextEdit::load_state(&ctx, id).and_then(|state| state.cursor.char_range()).map_or(0, |r| usize::from(r.primary.index));
                                                    let first_line = s.chars().take_while(|c| *c != '\n').count();
                                                    if down && cursor >= s.chars().count() {
                                                        if let Some(j) = (i + 1..count).find(|&j| is_text[j]) {
                                                            ed.want_focus = Some((j, 0));
                                                        }
                                                    }
                                                    if up && cursor <= first_line {
                                                        if let Some(j) = (0..i).rev().find(|&j| is_text[j]) {
                                                            ed.want_focus = Some((j, usize::MAX));
                                                            ctx.request_repaint();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        live::Seg::Image { id, alt, .. } => {
                                            ui.add_space(4.0);
                                            let src = format!("{}{id}", markdown::IMAGE_SCHEME);
                                            markdown::show_image(ui, media, &data.images, &src, alt, p.ink);
                                            if kit::small_button(ui, Look::Ghost, Some(Icon::Close), "Remove picture").clicked() {
                                                remove = Some(i);
                                            }
                                            ui.add_space(4.0);
                                        }
                                    }
                                }
                                if let Some(i) = remove {
                                    // Cursor goes to where the two text blocks around the picture meet.
                                    let join_at = match &segs[i - 1] {
                                        live::Seg::Text(t) => t.chars().count(),
                                        _ => 0,
                                    };
                                    segs.remove(i);
                                    ed.want_focus = Some((i - 1, join_at));
                                    ctx.request_repaint();
                                    changed = true;
                                }
                                if changed {
                                    ed.text = live::join(&segs);
                                }
                            });

                            // Footer: words and reading time, formatting marks, save hint.
                            let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                            ui.painter().rect_filled(line, 0.0, p.border_soft);
                            egui::Frame::new().inner_margin(egui::Margin::symmetric(22, 12)).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    let words = markdown::word_count(&ed.full_text());
                                    ui.label(RichText::new(format!("{words} {} · {} min read", if words == 1 { "word" } else { "words" }, read_minutes(words))).size(13.0).color(p.faint));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new("to save").size(13.0).color(p.faint));
                                        let (r, _) = ui.allocate_exact_size(vec2(56.0, 24.0), Sense::hover());
                                        ui.painter().rect(r, 6.0, Color32::TRANSPARENT, Stroke::new(1.0, p.cell_border), egui::StrokeKind::Inside);
                                        ui.painter().text(r.center(), Align2::CENTER_CENTER, "Ctrl S", FontId::proportional(12.0), p.muted);
                                        ui.add_space(14.0);
                                        kit::checkbox(ui, show_marks, "Show formatting marks");
                                    });
                                });
                            });
                        });

                        // Attachments, errors and the day's word goal.
                        if !editor_msg.is_empty() {
                            ui.label(RichText::new(editor_msg.as_str()).color(RED));
                        }
                        if !ed.attachments.is_empty() {
                            let mut remove = None;
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                                ui.label(RichText::new("Attachments").size(13.0).color(p.faint));
                                for (i, a) in ed.attachments.iter().enumerate() {
                                    egui::Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border_soft)).corner_radius(10).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("{} ({})", a.name, human_size(a.size))).size(13.5).color(p.muted));
                                            if kit::small_button(ui, Look::Ghost, None, "Remove").clicked() {
                                                remove = Some(i);
                                            }
                                        });
                                    });
                                }
                            });
                            if let Some(i) = remove {
                                ed.attachments.remove(i);
                            }
                        }
                        let goal = data.settings.daily_word_goal as usize;
                        if goal > 0 {
                            // Progress for the whole day: this draft plus the day's other entries.
                            let draft = markdown::word_count(&ed.full_text());
                            let day = key(ed.date);
                            let others: usize = data.entries.iter().filter(|e| e.date == day && Some(e.id) != ed.id && !e.generated).map(|e| markdown::word_count(&e.text)).sum();
                            let total = others + draft;
                            let done = total >= goal;
                            ui.label(RichText::new(format!("Today so far: {total} / {goal} words{}", if done { "  ✓" } else { "" })).size(13.5).color(if done { shades.accent } else { p.muted }));
                            let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
                            ui.painter().rect_filled(bar, 3.0, p.cell_border);
                            let mut filled = bar;
                            filled.set_width(bar.width() * (total as f32 / goal as f32).min(1.0));
                            ui.painter().rect_filled(filled, 3.0, shades.accent);
                        }
                        ui.add_space(40.0);
                    });
                });
            });
        });

        // Back / Discard: leave at once if nothing was written, otherwise ask once.
        if discard_clicked {
            cancel = ed.press_discard();
        } else if ed.discard_arm && ed.changed() && ctx.input(|i| i.pointer.any_click()) {
            // Clicking anywhere else disarms it again.
            ed.discard_arm = false;
        }

        if ask_draft {
            self.ai_draft();
        }
        if ask_question {
            self.ai_reflection(None);
        }
        if open_ai_setup {
            self.ai.setup_open = true;
        }
        if let Some(name) = chosen_template {
            self.apply_template(&name);
        }
        if open_templates {
            self.templates.open = true;
        }
        if persist_settings {
            self.persist();
        }
        if save_it {
            self.save_editor();
        } else if cancel {
            self.editor = None;
            self.editor_msg.clear();
        }
    }

    /// Store the entry being written (a new one, or the changes to an existing one).
    pub(crate) fn save_editor(&mut self) {
        let Some(ed) = self.editor.take() else { return };
        let text = ed.full_text().trim().to_string();
        let added_at = format!("{} {:02}:{:02}:{:02}", key(ed.date), ed.hour, ed.minute, ed.second);
        match ed.id {
            Some(id) => {
                if let Some(e) = self.data.entries.iter_mut().find(|e| e.id == id) {
                    e.text = text;
                    e.category = ed.category;
                    e.date = key(ed.date);
                    e.added_at = added_at;
                    e.mood = ed.mood;
                    e.pinned = ed.pinned;
                    e.sensitive = ed.sensitive;
                    e.attachments = ed.attachments;
                    e.ai = ed.ai;
                }
            }
            None => {
                let id = self.data.entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
                self.data.entries.push(Entry {
                    id,
                    date: key(ed.date),
                    added_at,
                    category: ed.category,
                    text,
                    mood: ed.mood,
                    pinned: ed.pinned,
                    sensitive: ed.sensitive,
                    attachments: ed.attachments,
                    ai: ed.ai,
                    ..Default::default()
                });
            }
        }
        self.selected = ed.date;
        self.month = ed.date.with_day(1).unwrap();
        self.editor_msg.clear();
        self.persist();
    }
}

// ---- hidden formatting marks -----------------------------------------------------------------
//
// With "Show formatting marks" off, the Markdown marks (`**`, `#`, ...) are laid out invisibly.
// These helpers make the cursor treat each hidden run of marks as if it were not there: arrow
// keys step across it, Backspace and Delete remove the visible character beside it, and a click
// never leaves the cursor inside a mark.

fn char_to_byte(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(i, _)| i)
}

fn remove_chars(text: &mut String, from: usize, to: usize) {
    let (a, b) = (char_to_byte(text, from), char_to_byte(text, to));
    text.replace_range(a..b, "");
}

fn collapsed_cursor(ctx: &egui::Context, id: egui::Id) -> Option<usize> {
    let r = egui::TextEdit::load_state(ctx, id)?.cursor.char_range()?;
    (r.primary.index == r.secondary.index).then(|| usize::from(r.primary.index))
}

/// Handle Left, Right, Backspace and Delete next to a hidden run of marks. Returns true if the
/// text was changed.
pub(crate) fn step_over_hidden_marks(ctx: &egui::Context, id: egui::Id, text: &mut String, hidden: &[(usize, usize)]) -> bool {
    use egui::Key;
    let Some(p) = collapsed_cursor(ctx, id) else { return false };
    let before = hidden.iter().find(|h| h.1 == p).copied();
    let after = hidden.iter().find(|h| h.0 == p).copied();
    let pressed = |key: Key| ctx.input(|i| i.key_pressed(key) && i.modifiers.is_none());
    let consume = |key: Key| ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
    let len = text.chars().count();
    match (before, after) {
        (Some(run), _) if pressed(Key::ArrowLeft) => {
            consume(Key::ArrowLeft);
            focus_at(ctx, id, text, run.0);
        }
        (_, Some(run)) if pressed(Key::ArrowRight) => {
            consume(Key::ArrowRight);
            focus_at(ctx, id, text, run.1);
        }
        (Some(run), _) if pressed(Key::Backspace) => {
            consume(Key::Backspace);
            if text.chars().nth(run.0) == Some('#') {
                // Backspace at the start of a heading turns it back into a plain paragraph.
                remove_chars(text, run.0, run.1);
                focus_at(ctx, id, text, run.0);
                return true;
            }
            if run.0 > 0 {
                remove_chars(text, run.0 - 1, run.0);
                focus_at(ctx, id, text, run.0 - 1);
                return true;
            }
        }
        (_, Some(run)) if pressed(Key::Delete) => {
            consume(Key::Delete);
            if run.1 < len {
                remove_chars(text, run.1, run.1 + 1);
                focus_at(ctx, id, text, run.0);
                return true;
            }
        }
        _ => {}
    }
    false
}

/// If the cursor ended up inside a hidden run (after a click or an up/down move), move it to
/// the end of the run.
pub(crate) fn snap_out_of_marks(ctx: &egui::Context, id: egui::Id, text: &str, hidden: &[(usize, usize)]) {
    let Some(p) = collapsed_cursor(ctx, id) else { return };
    if let Some(run) = hidden.iter().find(|h| h.0 < p && p < h.1) {
        focus_at(ctx, id, text, run.1);
    }
}

impl DiaryApp {
    /// Put a writing prompt into the entry as a heading at the cursor, on a line of its own, and
    /// leave the cursor on the empty line below it ready to write.
    pub(crate) fn insert_prompt_at_cursor(ctx: &egui::Context, ed: &mut EditState, prompt: &str) {
        let mut segs = live::split(&ed.text);
        let mut idx = ed.focus.min(segs.len() - 1);
        if !matches!(segs[idx], live::Seg::Text(_)) {
            idx = segs.iter().rposition(|s| matches!(s, live::Seg::Text(_))).unwrap_or(0);
        }
        let live::Seg::Text(text) = &mut segs[idx] else { return };
        let chars: Vec<char> = text.chars().collect();
        let at = collapsed_cursor(ctx, seg_id(idx)).unwrap_or(chars.len()).min(chars.len());
        let before = &chars[..at];
        let lead = if before.is_empty() || before.ends_with(&['\n', '\n']) {
            ""
        } else if before.ends_with(&['\n']) {
            "\n"
        } else {
            "\n\n"
        };
        let inserted = format!("{lead}## {prompt}\n\n");
        let new_pos = at + inserted.chars().count();
        let byte = char_to_byte(text, at);
        text.insert_str(byte, &inserted);
        ed.text = live::join(&segs);
        ed.want_focus = Some((idx, new_pos));
        ed.prompt_offset += 1;
    }
}
