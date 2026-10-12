//! The Settings page: its own left navigation (Back to journal, then the sections) and one
//! scrolling column of cards. Each setting is a row with a title and a line of explanation on the
//! left and its control (a button, a drop-down or a switch) on the right. Changes are saved at once.

use super::*;
use crate::kit::{self, Icon, Look};

const SECTIONS: [(&str, Icon); 5] = [("Security", Icon::Shield), ("Writing", Icon::Pencil), ("Your data", Icon::Database), ("Display", Icon::Sliders), ("Help", Icon::Help)];

const AUTO_LOCK: [(&str, u32); 5] = [("Off", 0), ("After 1 minute", 1), ("After 5 minutes", 5), ("After 10 minutes", 10), ("After 30 minutes", 30)];
const WORD_GOALS: [(&str, u32); 6] = [("Off", 0), ("100 words", 100), ("250 words", 250), ("500 words", 500), ("750 words", 750), ("1,000 words", 1000)];
const TEXT_SIZES: [(&str, f32); 7] = [("80%", 0.8), ("90%", 0.9), ("100%", 1.0), ("110%", 1.1), ("125%", 1.25), ("140%", 1.4), ("160%", 1.6)];
const START_MODES: [(&str, location::StartMode); 3] = [("Normal window", location::StartMode::Normal), ("Maximised", location::StartMode::Maximized), ("Full screen", location::StartMode::Fullscreen)];

/// One row of a settings card: title and description on the left, `control` on the right.
fn row(ui: &mut egui::Ui, p: &Palette, title: &str, description: &str, last: bool, control: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 78.0), Sense::hover());
    let control_w = (width * 0.42).clamp(150.0, 330.0);
    let painter = ui.painter_at(rect);
    painter.text(egui::pos2(rect.min.x + 24.0, rect.min.y + 28.0), Align2::LEFT_CENTER, title, FontId::new(16.0, fonts::ui_bold()), p.ink);
    let font = FontId::proportional(13.5);
    let text_w = (width - control_w - 70.0).max(100.0);
    painter.text(egui::pos2(rect.min.x + 24.0, rect.min.y + 52.0), Align2::LEFT_CENTER, elide(&painter, description, &font, text_w), font, p.faint);
    if !last {
        painter.line_segment([egui::pos2(rect.min.x, rect.max.y - 0.5), egui::pos2(rect.max.x, rect.max.y - 0.5)], Stroke::new(1.0, p.border_soft));
    }
    let area = egui::Rect::from_min_max(egui::pos2(rect.max.x - 24.0 - control_w, rect.min.y), egui::pos2(rect.max.x - 24.0, rect.max.y));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::right_to_left(egui::Align::Center)));
    child.spacing_mut().item_spacing.x = 8.0;
    control(&mut child);
}

/// A card holding the rows of one section.
fn card(ui: &mut egui::Ui, p: &Palette, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border_soft)).corner_radius(14).show(ui, |ui| {
        ui.set_width(width - 2.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        add(ui);
    });
}

fn dropdown<T: PartialEq + Copy>(ui: &mut egui::Ui, id: &str, current: &mut T, options: &[(&str, T)]) -> bool {
    let label = options.iter().find(|(_, v)| v == current).map_or("", |(l, _)| *l);
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(label).width(170.0).show_ui(ui, |ui| {
        for (name, value) in options {
            changed |= ui.selectable_value(current, *value, *name).changed();
        }
    });
    changed
}

impl DiaryApp {
    /// The left column while Settings is open.
    pub(crate) fn settings_nav(&mut self, ui: &mut egui::Ui) {
        let p = palette(ui.ctx());
        ui.add_space(4.0);
        let (rect, back) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
        let fg = if back.hovered() { theme::TEXT } else { theme::TEXT_2 };
        kit::paint(ui.painter(), egui::pos2(rect.min.x + 14.0, rect.center().y), 16.0, fg, Icon::ChevronLeft, false);
        ui.painter().text(egui::pos2(rect.min.x + 34.0, rect.center().y), Align2::LEFT_CENTER, "Back to journal", FontId::new(15.0, fonts::ui_bold()), fg);
        if back.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            self.show_settings = false;
        }
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            kit::section_label(ui, "Settings");
        });
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        for (i, (name, icon)) in SECTIONS.iter().enumerate() {
            if kit::nav_item(ui, Some(*icon), None, name, None, self.settings_section == i).clicked() {
                self.settings_section = i;
                self.settings_jump = Some(i);
            }
        }
        let _ = p;
    }

    pub(crate) fn settings_page(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let p = palette(&ctx);
        let mut changed = false;
        let jump = self.settings_jump.take();
        let mut tops: Vec<f32> = Vec::new();
        let last_backup = backups::list(&backups::folder_for(&data_path())).into_iter().find(|b| b.kind == backups::Kind::Auto).map(|b| b.when);
        let has_recovery = self.vault.as_ref().is_some_and(|v| v.has_recovery());

        egui::ScrollArea::vertical().id_salt("settings_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let width = (ui.available_width() - 16.0).clamp(300.0, 880.0);
            ui.set_max_width(width);
            ui.label(RichText::new("Settings").font(FontId::new(44.0, fonts::serif_display())).color(p.title));
            ui.label(RichText::new("Changes save as you make them.").size(15.0).color(p.muted));
            ui.add_space(22.0);

            let section = |ui: &mut egui::Ui, index: usize, tops: &mut Vec<f32>| {
                let r = ui.horizontal(|ui| kit::section_label(ui, SECTIONS[index].0)).response.rect;
                tops.push(r.top());
                if jump == Some(index) {
                    ui.scroll_to_rect(r.expand2(vec2(0.0, 6.0)), Some(egui::Align::TOP));
                }
                ui.add_space(8.0);
            };

            // ---- Security ----
            section(ui, 0, &mut tops);
            card(ui, &p, width, |ui| {
                row(ui, &p, "Password", "Needed to open your journal.", false, |ui| {
                    if kit::button(ui, Look::Ghost, None, "Change password", None).clicked() {
                        self.show_password = true;
                    }
                });
                row(ui, &p, "Recovery key", "The only way back in if you forget your password. Keep a copy somewhere safe.", false, |ui| {
                    if kit::button(ui, Look::Ghost, None, if has_recovery { "Show key" } else { "Set up" }, None).clicked() {
                        self.show_recovery = true;
                    }
                });
                row(ui, &p, "Auto-lock", "Lock the journal after a period of no activity.", false, |ui| {
                    changed |= dropdown(ui, "set_autolock", &mut self.data.settings.auto_lock_minutes, &AUTO_LOCK);
                });
                row(ui, &p, "Hide when in the background", "Hide your entries while the window is not the one you are using.", true, |ui| {
                    changed |= kit::toggle(ui, &mut self.data.settings.privacy_screen).changed();
                });
            });

            // ---- Writing ----
            ui.add_space(26.0);
            section(ui, 1, &mut tops);
            card(ui, &p, width, |ui| {
                row(ui, &p, "Writing prompts", "Show a question to get you started on each new entry.", false, |ui| {
                    changed |= kit::toggle(ui, &mut self.data.settings.show_prompts).changed();
                });
                row(ui, &p, "Daily word goal", "Shows progress in the editor while you write.", false, |ui| {
                    changed |= dropdown(ui, "set_goal", &mut self.data.settings.daily_word_goal, &WORD_GOALS);
                });
                row(ui, &p, "Templates", "Reusable starting points, like a gratitude list or weekly review.", false, |ui| {
                    if kit::button(ui, Look::Ghost, None, "Manage", None).clicked() {
                        self.templates.open = true;
                    }
                });
                let ai_label = if self.data.settings.ai_enabled { "Manage" } else { "Set up" };
                row(ui, &p, "AI helpers", "Drafting, questions and day summaries, run on this PC with Ollama.", true, |ui| {
                    if kit::button(ui, Look::Ghost, None, ai_label, None).clicked() {
                        self.ai.setup_open = true;
                    }
                });
            });

            // ---- Your data ----
            ui.add_space(26.0);
            section(ui, 2, &mut tops);
            card(ui, &p, width, |ui| {
                let desc = match &last_backup {
                    Some(when) => format!("An encrypted copy every day. Last backup: {}.", when.get(..16).unwrap_or(when)),
                    None => "An encrypted copy every day. No backup yet.".to_string(),
                };
                let mut auto = self.data.settings.backup_keep > 0;
                row(ui, &p, "Automatic backups", &desc, false, |ui| {
                    if kit::toggle(ui, &mut auto).changed() {
                        self.data.settings.backup_keep = if auto { backups::DEFAULT_KEEP } else { 0 };
                        changed = true;
                    }
                });
                if self.data.settings.backup_keep > 0 {
                    let options: Vec<(String, u32)> = backups::KEEP_CHOICES.iter().filter(|n| **n > 0).map(|n| (format!("{n} copies"), *n)).collect();
                    let options: Vec<(&str, u32)> = options.iter().map(|(l, n)| (l.as_str(), *n)).collect();
                    row(ui, &p, "Copies to keep", "Older copies are removed automatically.", false, |ui| {
                        changed |= dropdown(ui, "set_keep", &mut self.data.settings.backup_keep, &options);
                    });
                }
                row(ui, &p, "Back up now", "Save an encrypted copy to a folder or drive you choose.", false, |ui| {
                    if kit::button(ui, Look::Ghost, None, "Back up…", None).clicked() {
                        self.backup();
                    }
                });
                row(ui, &p, "Backups and restore", "See every automatic backup, or put one back.", false, |ui| {
                    if kit::button(ui, Look::Ghost, None, "Open…", None).clicked() {
                        self.show_backups = true;
                        self.backups_dirty = true;
                    }
                });
                row(ui, &p, "Export to PDF", "A readable, printable copy. Not encrypted.", false, |ui| {
                    if kit::button(ui, Look::Ghost, None, "Export…", None).clicked() {
                        self.show_export = true;
                    }
                });
                row(ui, &p, "Data location", &format!("Where your journal is kept: {}", location::data_dir().display()), false, |ui| {
                    if kit::button(ui, Look::Ghost, None, "Change…", None).clicked() {
                        self.show_location = true;
                    }
                });
                let n = self.data.trash.len();
                row(ui, &p, "Recently deleted", &format!("{n} {} kept for {} days.", if n == 1 { "entry" } else { "entries" }, trash::TRASH_DAYS), true, |ui| {
                    if kit::button(ui, Look::Ghost, None, "Open…", None).clicked() {
                        self.show_trash = true;
                    }
                });
            });

            // ---- Display ----
            ui.add_space(26.0);
            section(ui, 3, &mut tops);
            card(ui, &p, width, |ui| {
                row(ui, &p, "Accent colour", "Buttons, the selected day and the focus ring. Shades are adjusted to stay readable.", false, |ui| {
                    let current = theme::accent();
                    let mut chosen = None;
                    // Right to left, so the custom picker comes first and the presets follow.
                    let mut custom = current;
                    if ui.color_edit_button_srgb(&mut custom).on_hover_text("Pick your own colour").changed() {
                        chosen = Some(custom);
                    }
                    for (name, colour) in theme::PRESETS.iter().rev() {
                        let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        ui.painter().circle_filled(r.center(), 9.0, theme::derive(*colour).accent);
                        if current == *colour {
                            ui.painter().circle_stroke(r.center(), 11.0, Stroke::new(2.0, theme::TEXT));
                        }
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(*name).clicked() {
                            chosen = Some(*colour);
                        }
                    }
                    if let Some(colour) = chosen {
                        self.set_colour_theme(&ctx, colour);
                    }
                });
                row(ui, &p, "Text size", "Makes everything bigger or smaller. Ctrl + and Ctrl - work too.", false, |ui| {
                    // Compare with a little slack (the saved value is a float).
                    let mut idx = TEXT_SIZES.iter().position(|(_, s)| (self.data.settings.ui_scale - s).abs() < 0.01).unwrap_or(2);
                    let before = idx;
                    egui::ComboBox::from_id_salt("set_text").selected_text(TEXT_SIZES[idx].0).width(170.0).show_ui(ui, |ui| {
                        for (i, (name, _)) in TEXT_SIZES.iter().enumerate() {
                            ui.selectable_value(&mut idx, i, *name);
                        }
                    });
                    if idx != before {
                        self.data.settings.ui_scale = TEXT_SIZES[idx].1;
                        changed = true;
                    }
                });
                row(ui, &p, "Window on startup", "How the window opens next time. Takes effect after a restart.", false, |ui| {
                    let mut mode = self.start_mode;
                    if dropdown(ui, "set_start", &mut mode, &START_MODES) {
                        self.start_mode = mode;
                        if let Err(e) = location::set_start_mode(mode) {
                            self.status = format!("Save failed: couldn't remember the window setting ({e})");
                        }
                    }
                });
                row(ui, &p, "Full screen", "Switch full screen on or off right now.", true, |ui| {
                    if kit::button(ui, Look::Ghost, None, "Toggle (F11)", None).clicked() {
                        toggle_fullscreen(&ctx);
                    }
                });
            });

            // ---- Help ----
            ui.add_space(26.0);
            section(ui, 4, &mut tops);
            card(ui, &p, width, |ui| {
                row(ui, &p, "Keyboard shortcuts", "Every shortcut in one list.", true, |ui| {
                    if kit::button(ui, Look::Ghost, None, "View", None).clicked() {
                        self.show_shortcuts = true;
                    }
                });
            });
            ui.add_space(40.0);

            // The section nearest the top of the view is the one lit up in the left column.
            let top = ui.clip_rect().top() + 80.0;
            let active = tops.iter().rposition(|t| *t <= top).unwrap_or(0);
            if jump.is_none() {
                self.settings_section = active;
            }
        });

        if changed {
            self.persist();
        }
    }
}
