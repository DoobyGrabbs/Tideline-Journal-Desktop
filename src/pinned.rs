//! The Pinned page: every starred entry as a card, with a category filter and a sort order.

use super::*;
use eframe::egui::text::{LayoutJob, TextFormat};

pub(crate) const SORTS: [&str; 3] = ["Newest entry first", "Oldest entry first", "Longest first"];

/// What the cards can be asked to do.
#[derive(Default)]
struct Actions {
    unpin: Option<u64>,
    open: Option<NaiveDate>,
    more: Option<u64>,
}

fn columns_for(width: f32) -> usize {
    if width >= 1450.0 {
        4
    } else if width >= 1050.0 {
        3
    } else if width >= 680.0 {
        2
    } else {
        1
    }
}

/// A filter chip: "All 4", "Personal 2". `col` adds a category dot.
fn filter_chip(ui: &mut egui::Ui, label: &str, count: usize, col: Option<Color32>, on: bool) -> egui::Response {
    let accent = theme::shades().accent;
    let font = FontId::new(14.0, fonts::ui_bold());
    let count_text = count.to_string();
    let label_w = ui.painter().layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE).size().x;
    let count_w = ui.painter().layout_no_wrap(count_text.clone(), FontId::proportional(13.0), Color32::WHITE).size().x;
    let lead = if col.is_some() { 20.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0 + lead + label_w + 10.0 + count_w, 38.0), Sense::click());
    let (fill, border) = if on {
        (accent.gamma_multiply(0.14), accent)
    } else if resp.hovered() {
        (theme::NAV_ACTIVE, theme::TEXT_3)
    } else {
        (theme::PANEL, theme::BORDER)
    };
    ui.painter().rect(rect, 19.0, fill, Stroke::new(1.0, border), egui::StrokeKind::Inside);
    let cy = rect.center().y;
    let mut x = rect.min.x + 16.0;
    if let Some(c) = col {
        ui.painter().circle_filled(egui::pos2(x + 4.0, cy), 4.0, c);
        x += 16.0;
    }
    ui.painter().text(egui::pos2(x, cy), Align2::LEFT_CENTER, label, font, if on { theme::TEXT } else { theme::TEXT_2 });
    ui.painter().text(egui::pos2(x + label_w + 10.0, cy), Align2::LEFT_CENTER, count_text, FontId::proportional(13.0), theme::TEXT_3);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A small tile with the month and the day number.
fn date_tile(ui: &mut egui::Ui, date: &str) {
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok();
    let (rect, _) = ui.allocate_exact_size(vec2(50.0, 56.0), Sense::hover());
    ui.painter().rect(rect, 10.0, theme::BG, Stroke::new(1.0, theme::BORDER_SOFT), egui::StrokeKind::Inside);
    if let Some(d) = parsed {
        let month: String = d.format("%b").to_string().to_uppercase().chars().flat_map(|c| [c, '\u{200A}']).collect();
        ui.painter().text(egui::pos2(rect.center().x, rect.min.y + 14.0), Align2::CENTER_CENTER, month, FontId::new(11.0, fonts::ui_bold()), theme::TEXT_3);
        ui.painter().text(egui::pos2(rect.center().x, rect.min.y + 36.0), Align2::CENTER_CENTER, d.day().to_string(), FontId::new(25.0, fonts::serif_medium()), theme::TEXT);
    }
}

impl DiaryApp {
    pub(crate) fn pinned_page(&mut self, ui: &mut egui::Ui) {
        let p = palette(ui.ctx());
        let mut actions = Actions::default();
        let all: Vec<&Entry> = self.data.entries.iter().filter(|e| e.pinned).collect();
        let total = all.len();

        // Categories that have pinned entries, in the order they are listed in Settings.
        let cats: Vec<(String, Color32, usize)> = self
            .data
            .categories
            .iter()
            .map(|c| (c.name.clone(), rgb(c.color), all.iter().filter(|e| e.category == c.name).count()))
            .filter(|(_, _, n)| *n > 0)
            .collect();
        if self.pinned_category.as_ref().is_some_and(|c| !cats.iter().any(|(n, _, _)| n == c)) {
            self.pinned_category = None;
        }
        let mut shown: Vec<Entry> = all.iter().copied().filter(|e| self.pinned_category.as_ref().is_none_or(|c| &e.category == c)).cloned().collect();
        match self.pinned_sort {
            1 => shown.sort_by(|a, b| a.added_at.cmp(&b.added_at)),
            2 => shown.sort_by_key(|e| std::cmp::Reverse(markdown::word_count(&e.text))),
            _ => shown.sort_by(|a, b| b.added_at.cmp(&a.added_at)),
        }

        let mut category = self.pinned_category.clone();
        let mut sort = self.pinned_sort;
        egui::ScrollArea::vertical().id_salt("pinned_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let width = (ui.available_width() - 12.0).max(300.0);
            ui.set_max_width(width);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Pinned").font(FontId::new(44.0, fonts::serif_display())).color(p.title));
                    let subtitle = match total {
                        0 => "Nothing starred yet.".to_string(),
                        1 => "1 entry you've starred to keep close.".to_string(),
                        n => format!("{n} entries you've starred to keep close."),
                    };
                    ui.label(RichText::new(subtitle).size(15.0).color(p.muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.add_space(2.0);
                    egui::ComboBox::from_id_salt("pinned_sort").selected_text(SORTS[sort]).width(170.0).show_ui(ui, |ui| {
                        for (i, name) in SORTS.iter().enumerate() {
                            ui.selectable_value(&mut sort, i, *name);
                        }
                    });
                    ui.label(RichText::new("Sort by").size(14.0).color(p.muted));
                });
            });
            ui.add_space(14.0);

            if total > 0 {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                    if filter_chip(ui, "All", total, None, category.is_none()).clicked() {
                        category = None;
                    }
                    for (name, col, n) in &cats {
                        if filter_chip(ui, name, *n, Some(*col), category.as_ref() == Some(name)).clicked() {
                            category = Some(name.clone());
                        }
                    }
                });
                ui.add_space(14.0);
            }

            let per_row = columns_for(width);
            let gap = 18.0;
            let card_w = (width - gap * (per_row as f32 - 1.0)) / per_row as f32;
            let mut index = 0;
            let count = shown.len() + 1; // the last slot is the hint card
            while index < count {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for _ in 0..per_row {
                        if index >= count {
                            break;
                        }
                        ui.vertical(|ui| {
                            if index < shown.len() {
                                self.pinned_card(ui, &p, &shown[index], card_w, &mut actions);
                            } else {
                                hint_card(ui, &p, card_w);
                            }
                        });
                        index += 1;
                    }
                });
                ui.add_space(gap - 8.0);
            }
            ui.add_space(30.0);
        });

        self.pinned_category = category;
        self.pinned_sort = sort;
        if let Some(id) = actions.more {
            if !self.expanded.remove(&id) {
                self.expanded.insert(id);
            }
        }
        if let Some(id) = actions.unpin {
            if let Some(e) = self.data.entries.iter_mut().find(|e| e.id == id) {
                e.pinned = false;
            }
            self.persist();
        }
        if let Some(d) = actions.open {
            self.selected = d;
            self.month = d.with_day(1).unwrap();
            self.day_scroll_pending = true;
            self.show_pinned = false;
        }
    }

    fn pinned_card(&mut self, ui: &mut egui::Ui, p: &Palette, e: &Entry, width: f32, actions: &mut Actions) {
        let c = category_color(&self.data.categories, &e.category);
        let hidden = e.sensitive && !self.revealed.contains(&e.id);
        egui::Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border_soft)).corner_radius(14).inner_margin(egui::Margin::same(22)).show(ui, |ui| {
            ui.set_width(width - 46.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.horizontal_top(|ui| {
                date_tile(ui, &e.date);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let when = NaiveDate::parse_from_str(&e.date, "%Y-%m-%d").map(|d| d.format("%A %-d %B").to_string()).unwrap_or_else(|_| e.date.clone());
                    ui.label(RichText::new(format!("{when} · {}", e.added_at.get(11..16).unwrap_or(""))).size(14.0).color(p.muted));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        kit::category_pill(ui, c, &e.category);
                        if let Some(m) = e.mood.filter(|_| !hidden) {
                            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                            kit::paint_face(ui.painter(), r.center(), 17.0, theme::MOODS[usize::from(m.clamp(1, 5)) - 1], m);
                            ui.label(RichText::new(theme::MOOD_NAMES[usize::from(m.clamp(1, 5)) - 1]).size(13.5).color(p.muted));
                        }
                    });
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    let (r, star) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                    kit::paint(ui.painter(), r.center(), 19.0, theme::TODAY, kit::Icon::Star, true);
                    if star.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Unpin").clicked() {
                        actions.unpin = Some(e.id);
                    }
                });
            });
            ui.add_space(4.0);
            if hidden {
                ui.label(RichText::new("Private entry").font(FontId::new(20.0, fonts::serif_italic())).color(p.faint));
            } else {
                let (title, body) = markdown::split_title(&e.text);
                if let Some(title) = title {
                    ui.label(RichText::new(title).font(FontId::new(23.0, fonts::serif_medium())).color(p.title));
                }
                let is_open = self.expanded.contains(&e.id);
                let has_picture = !markdown::image_ids(&body).is_empty();
                let preview = markdown::plain_text(&body);
                let longer = has_picture || preview.chars().count() > 190 || body.trim().contains('\n');
                if is_open {
                    let style = markdown::Style { text: p.ink, link: p.accent, size: 16.0, serif: true };
                    markdown::render(ui, &body, &style, &mut self.media, &self.data.images);
                } else if !preview.is_empty() {
                    let mut job = LayoutJob::default();
                    job.wrap.max_width = ui.available_width();
                    job.wrap.max_rows = 4;
                    job.wrap.overflow_character = Some('…');
                    job.append(&preview, 0.0, TextFormat { font_id: FontId::new(16.0, fonts::serif()), color: p.muted, line_height: Some(16.0 * 1.7), ..Default::default() });
                    ui.label(job);
                }
                if longer && kit::text_link(ui, if is_open { "Show less" } else if has_picture { "Show more, with picture" } else { "Show more" }, if is_open { kit::Icon::ChevronUp } else { kit::Icon::ChevronDown }).clicked() {
                    actions.more = Some(e.id);
                }
            }
            ui.add_space(4.0);
            let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0.0, p.border_soft);
            ui.horizontal(|ui| {
                if !hidden {
                    ui.label(RichText::new(format!("{} words", markdown::word_count(&e.text))).size(13.0).color(p.faint));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if kit::small_button_after(ui, kit::Look::Ghost, "Open day", kit::Icon::ChevronRight).clicked() {
                        actions.open = NaiveDate::parse_from_str(&e.date, "%Y-%m-%d").ok();
                    }
                });
            });
        });
    }
}

/// The dashed card at the end: how to pin things.
fn hint_card(ui: &mut egui::Ui, p: &Palette, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 230.0), Sense::hover());
    let outline = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
    ui.painter().extend(egui::Shape::dashed_line(&outline, Stroke::new(1.2, theme::BORDER), 6.0, 5.0));
    let inner = rect.shrink2(vec2(24.0, 0.0));
    let cy = rect.center().y;
    kit::paint(ui.painter(), egui::pos2(inner.min.x + 9.0, cy - 12.0), 17.0, theme::TODAY, kit::Icon::Star, false);
    ui.painter().text(egui::pos2(inner.min.x + 28.0, cy - 12.0), Align2::LEFT_CENTER, "Keep the ones that matter close.", FontId::new(18.0, fonts::serif_italic()), p.muted);
    let mut job = LayoutJob::default();
    job.wrap.max_width = inner.width();
    job.append("Click the star on any entry, or press Ctrl D while writing, to pin it here.", 0.0, TextFormat { font_id: FontId::proportional(13.5), color: p.faint, line_height: Some(22.0), ..Default::default() });
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(egui::pos2(inner.min.x, cy + 6.0), galley, p.faint);
}
