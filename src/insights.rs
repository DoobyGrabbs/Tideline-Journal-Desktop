//! The Insights page: mood patterns, writing habits, and a look back at a week, month or year.
//! It fills the middle of the window (the side menu stays) as a header, a row of tabs and cards.

use super::*;
use crate::reports::{self, Avg, MoodReport, Period, Review, WritingReport};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Mood,
    Writing,
    Review,
}

struct Cache {
    key: (u64, usize, usize),
    mood: MoodReport,
    writing: WritingReport,
    review: Review,
}

pub(crate) struct InsightsState {
    pub open: bool,
    pub tab: Tab,
    /// Index into `RANGES` (for the Mood and Writing tabs).
    pub range: usize,
    /// Index into `Period::ALL` (for the Look back tab).
    pub period: usize,
    /// Show the mood numbers as a table instead of bars.
    pub table: bool,
    msg: String,
    cache: Option<Cache>,
}

impl Default for InsightsState {
    fn default() -> Self {
        Self { open: false, tab: Tab::Mood, range: 2, period: 2, table: false, msg: String::new(), cache: None }
    }
}

impl InsightsState {
    /// Forget the computed numbers (the diary was locked).
    pub fn clear(&mut self) {
        self.cache = None;
        self.open = false;
        self.msg.clear();
    }
}

const RANGES: [&str; 4] = ["30 days", "90 days", "This year", "All time"];
/// How the range reads in a sentence ("of 8 entries this year").
const RANGE_PHRASES: [&str; 4] = ["in the last 30 days", "in the last 90 days", "this year", "in total"];
/// How many mood ratings make the patterns worth reading.
const RATINGS_NEEDED: usize = 10;
/// Fewer ratings than this in a group is only an early signal.
const RELIABLE: usize = 3;

const DAYPART_LABELS: [&str; 4] = ["Morning · 05:00–12:00", "Afternoon · 12:00–17:00", "Evening · 17:00–22:00", "Night · after 22:00"];
const LENGTH_LABELS: [&str; 3] = ["Short · under 50 words", "Medium · 50–199 words", "Long · 200+ words"];
const NUMBER_WORDS: [&str; 11] = ["Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten"];

fn range_bounds(range: usize, today: NaiveDate, entries: &[Entry]) -> (NaiveDate, NaiveDate) {
    match range {
        0 => (today - chrono::Days::new(29), today),
        1 => (today - chrono::Days::new(89), today),
        2 => Period::ThisYear.bounds(today),
        _ => {
            let first = entries.iter().filter_map(|e| NaiveDate::parse_from_str(&e.date, "%Y-%m-%d").ok()).min().unwrap_or(today);
            let last = entries.iter().filter_map(|e| NaiveDate::parse_from_str(&e.date, "%Y-%m-%d").ok()).max().unwrap_or(today).max(today);
            (first, last)
        }
    }
}

// ---- building blocks ---------------------------------------------------------------------------

/// A card: surface colour, soft border, 14 px corners.
fn card(ui: &mut egui::Ui, p: &Palette, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border_soft)).corner_radius(14).inner_margin(egui::Margin::same(22)).show(ui, |ui| {
        ui.set_width(width - 46.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        add(ui);
    });
}

/// Lay cards out `per_row` to a row, each `width` wide. Each item draws one card.
fn grid(ui: &mut egui::Ui, per_row: usize, width: f32, gap: f32, items: Vec<Box<dyn FnOnce(&mut egui::Ui, f32) + '_>>) {
    let mut items = items.into_iter().peekable();
    while items.peek().is_some() {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for item in items.by_ref().take(per_row) {
                ui.vertical(|ui| item(ui, width));
            }
        });
        ui.add_space(gap - 8.0);
    }
}

fn spaced_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    let spaced: String = text.to_uppercase().chars().flat_map(|c| [c, '\u{200A}']).collect();
    ui.label(RichText::new(spaced).font(FontId::new(12.0, fonts::ui_bold())).color(color));
}

/// A figure card: a small label, a big serif number and a line of explanation.
fn stat_card(ui: &mut egui::Ui, p: &Palette, width: f32, label: &str, value: &str, note: &str) {
    card(ui, p, width, |ui| {
        spaced_label(ui, label, p.muted);
        ui.horizontal(|ui| {
            ui.label(RichText::new(value).font(FontId::new(44.0, fonts::serif_display())).color(p.title));
            ui.add_space(2.0);
            ui.label(RichText::new(note).size(14.5).color(p.muted));
        });
    });
}

fn card_title(ui: &mut egui::Ui, p: &Palette, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).font(FontId::new(24.0, fonts::serif_medium())).color(p.title));
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).size(13.5).color(p.faint));
    }
    ui.add_space(8.0);
}

/// Mood bars: a track with ticks at 1 to 5, a bar for the average, and the figures beside it.
fn mood_bars(ui: &mut egui::Ui, p: &Palette, inner: f32, rows: &[(String, Avg)]) {
    let (label_w, value_w, gap) = (inner * 0.36, inner * 0.36, 10.0);
    let track_w = (inner - label_w - value_w - gap * 2.0).max(60.0);
    let accent = p.accent;
    for (label, avg) in rows {
        let (row, _) = ui.allocate_exact_size(vec2(inner, 30.0), Sense::hover());
        let painter = ui.painter_at(row);
        let cy = row.center().y;
        let font = FontId::proportional(14.0);
        painter.text(egui::pos2(row.min.x, cy), Align2::LEFT_CENTER, elide(&painter, label, &font, label_w - 6.0), font.clone(), p.ink);
        let track = egui::Rect::from_min_size(egui::pos2(row.min.x + label_w + gap, cy - 7.0), vec2(track_w, 14.0));
        painter.rect_filled(track, 7.0, theme::BG);
        for i in 1..5 {
            let x = track.min.x + track.width() * i as f32 / 5.0;
            painter.line_segment([egui::pos2(x, track.min.y + 2.0), egui::pos2(x, track.max.y - 2.0)], Stroke::new(1.0, p.border_soft));
        }
        let value_at = egui::pos2(track.max.x + gap, cy);
        match avg.mean() {
            Some(m) => {
                let fill = egui::Rect::from_min_size(track.min, vec2(track.width() * (m / 5.0).clamp(0.0, 1.0), track.height()));
                let shade = if avg.n >= RELIABLE { accent } else { accent.gamma_multiply(0.45) };
                painter.rect_filled(fill, 7.0, shade);
                let name = theme::MOOD_NAMES[(m.round() as usize).clamp(1, 5) - 1];
                let first = painter.text(value_at, Align2::LEFT_CENTER, format!("{m:.1}"), FontId::new(14.0, fonts::ui_bold()), p.ink);
                painter.text(
                    egui::pos2(first.max.x + 5.0, cy),
                    Align2::LEFT_CENTER,
                    format!("{name} · {} {}", avg.n, if avg.n == 1 { "entry" } else { "entries" }),
                    FontId::proportional(13.0),
                    p.muted,
                );
            }
            None => {
                painter.text(value_at, Align2::LEFT_CENTER, "No ratings yet", FontId::proportional(13.0), p.faint);
            }
        }
    }
    // The scale under the track.
    let (axis, _) = ui.allocate_exact_size(vec2(inner, 18.0), Sense::hover());
    let track_min = axis.min.x + label_w + gap;
    for i in 1..=5 {
        ui.painter().text(egui::pos2(track_min + track_w * i as f32 / 5.0, axis.center().y), Align2::CENTER_CENTER, i.to_string(), FontId::proportional(11.5), p.faint);
    }
}

fn word_cloud(ui: &mut egui::Ui, p: &Palette, words: &[(String, usize)]) {
    let most = words.first().map_or(1, |w| w.1).max(1) as f32;
    let colors = [p.accent, p.title, p.muted, p.accent.gamma_multiply(0.7), p.ink];
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(14.0, 2.0);
        // Alphabetical, so the size (not the order) shows what is common.
        let mut sorted: Vec<&(String, usize)> = words.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        for (i, (w, n)) in sorted.iter().map(|x| (&x.0, x.1)).enumerate() {
            let size = 15.0 + 22.0 * (n as f32 / most).sqrt();
            ui.label(RichText::new(w).font(FontId::new(size, fonts::serif_medium())).color(colors[i % colors.len()])).on_hover_text(format!("{n} times"));
        }
    });
}

/// Columns for hours of the day (24) or days of the week (7).
fn column_chart(ui: &mut egui::Ui, p: &Palette, width: f32, values: &[usize], labels: &[String]) {
    let most = values.iter().copied().max().unwrap_or(0).max(1) as f32;
    let (rect, _) = ui.allocate_exact_size(vec2(width, 130.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let n = values.len() as f32;
    let slot = rect.width() / n;
    for (i, v) in values.iter().enumerate() {
        let h = (rect.height() - 40.0) * (*v as f32 / most);
        let x = rect.min.x + slot * i as f32;
        let bar = egui::Rect::from_min_max(egui::pos2(x + slot * 0.18, rect.max.y - 20.0 - h.max(3.0)), egui::pos2(x + slot * 0.82, rect.max.y - 20.0));
        painter.rect_filled(bar, 3.0, if *v > 0 { p.accent } else { theme::BG });
        if !labels[i].is_empty() {
            painter.text(egui::pos2(x + slot * 0.5, rect.max.y - 2.0), Align2::CENTER_BOTTOM, &labels[i], FontId::proportional(11.5), p.faint);
        }
        if *v > 0 && slot > 18.0 {
            painter.text(egui::pos2(x + slot * 0.5, bar.min.y - 2.0), Align2::CENTER_BOTTOM, v.to_string(), FontId::proportional(11.0), p.muted);
        }
    }
}

fn columns_for(width: f32) -> usize {
    if width >= 1000.0 {
        3
    } else if width >= 640.0 {
        2
    } else {
        1
    }
}

impl DiaryApp {
    pub(crate) fn insights_page(&mut self, ui: &mut egui::Ui) {
        self.ensure_stats();
        let ctx = ui.ctx().clone();
        let today = Local::now().date_naive();
        let goal = self.data.settings.daily_word_goal;

        // Work out the numbers again only when something they depend on has changed.
        let key = (self.rev, self.insights.range, self.insights.period);
        if self.insights.cache.as_ref().map(|c| c.key) != Some(key) {
            let (from, to) = range_bounds(self.insights.range, today, &self.data.entries);
            let per_day = &self.stats_cache.as_ref().unwrap().1;
            let entries = &self.data.entries;
            self.insights.cache = Some(Cache {
                key,
                mood: reports::mood_report(entries, from, to),
                writing: reports::writing_report(entries, from, to, 40),
                review: reports::review(entries, per_day, &self.data.images, Period::ALL[self.insights.period].0, today, goal),
            });
        }

        let p = palette(&ctx);
        let DiaryApp { insights, data, media, .. } = self;
        let cache = insights.cache.as_ref().unwrap();
        let mut jump = None;
        let mut export = false;
        let (mut tab, mut range, mut period, mut table) = (insights.tab, insights.range, insights.period, insights.table);
        let msg = insights.msg.clone();

        egui::ScrollArea::vertical().id_salt("insights_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let width = (ui.available_width() - 12.0).max(300.0);
            ui.set_max_width(width);

            // Title and the range switch.
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Insights").font(FontId::new(44.0, fonts::serif_display())).color(p.title));
                    ui.label(RichText::new("Patterns in how you feel and how you write.").size(15.0).color(p.muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    if tab == Tab::Review {
                        let names: Vec<&str> = Period::ALL.iter().map(|(_, n)| *n).collect();
                        if let Some(i) = kit::segmented(ui, &names, Some(period)) {
                            period = i;
                        }
                    } else if let Some(i) = kit::segmented(ui, &RANGES, Some(range)) {
                        range = i;
                    }
                });
            });
            ui.add_space(16.0);

            // Tabs with an underline on the active one.
            let tab_font = FontId::new(15.0, fonts::ui_bold());
            let start_y = ui.cursor().min.y;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (t, name) in [(Tab::Mood, "Mood patterns"), (Tab::Writing, "Writing habits"), (Tab::Review, "Look back")] {
                    let w = ui.painter().layout_no_wrap(name.to_string(), tab_font.clone(), Color32::WHITE).size().x + 36.0;
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 44.0), Sense::click());
                    let on = tab == t;
                    let col = if on { p.title } else if resp.hovered() { p.ink } else { p.muted };
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, name, tab_font.clone(), col);
                    if on {
                        ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(r.min.x + 10.0, r.max.y - 2.0), egui::pos2(r.max.x - 10.0, r.max.y)), 1.0, p.accent);
                    }
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        tab = t;
                    }
                }
            });
            let line = egui::Rect::from_min_size(egui::pos2(ui.min_rect().min.x, start_y + 43.0), vec2(width, 1.0));
            ui.painter().rect_filled(line, 0.0, p.border_soft);
            ui.add_space(18.0);

            let per_row = columns_for(width);
            let gap = 16.0;
            let col_w = (width - gap * (per_row as f32 - 1.0)) / per_row as f32;

            match tab {
                Tab::Mood => {
                    let m = &cache.mood;
                    let (mut sum, mut n) = (0.0f32, 0usize);
                    for a in &m.weekday {
                        if let Some(mean) = a.mean() {
                            sum += mean * a.n as f32;
                            n += a.n;
                        }
                    }
                    let overall = (n > 0).then(|| sum / n as f32);
                    let (avg_value, avg_note) = match overall {
                        Some(v) => (format!("{v:.1}"), format!("out of 5 · {}", theme::MOOD_NAMES[(v.round() as usize).clamp(1, 5) - 1])),
                        None => ("–".to_string(), "no ratings yet".to_string()),
                    };
                    let entries_total = cache.writing.entries;
                    let phrase = RANGE_PHRASES[range];
                    let rated = m.rated;
                    let notes = m.notes.clone();
                    let shades = theme::shades();

                    let intro: Vec<Box<dyn FnOnce(&mut egui::Ui, f32)>> = vec![
                        Box::new(|ui, w| stat_card(ui, &p, w, "Average mood", &avg_value, &avg_note)),
                        Box::new(|ui, w| stat_card(ui, &p, w, "Entries with a mood", &rated.to_string(), &format!("of {entries_total} {} {phrase}", if entries_total == 1 { "entry" } else { "entries" }))),
                        Box::new(|ui, w| {
                            egui::Frame::new().fill(shades.prompt_bg).stroke(Stroke::new(1.0, shades.prompt_border)).corner_radius(14).inner_margin(egui::Margin::same(22)).show(ui, |ui| {
                                ui.set_width(w - 46.0);
                                ui.spacing_mut().item_spacing.y = 8.0;
                                if rated < RATINGS_NEEDED {
                                    spaced_label(ui, &format!("Patterns unlock at {RATINGS_NEEDED} ratings"), shades.accent);
                                    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
                                    ui.painter().rect_filled(bar, 3.0, theme::BG);
                                    let mut filled = bar;
                                    filled.set_width(bar.width() * rated as f32 / RATINGS_NEEDED as f32);
                                    ui.painter().rect_filled(filled, 3.0, shades.accent);
                                    let more = RATINGS_NEEDED - rated;
                                    ui.label(
                                        RichText::new(format!("Pick a mood when you write. {} more and the charts below become reliable.", NUMBER_WORDS[more]))
                                            .size(14.0)
                                            .color(theme::TEXT),
                                    );
                                } else {
                                    spaced_label(ui, "What stands out", shades.accent);
                                    if notes.is_empty() {
                                        ui.label(RichText::new("No clear pattern yet. Keep writing.").size(14.0).color(theme::TEXT));
                                    }
                                    for note in &notes {
                                        ui.label(RichText::new(note).size(14.0).color(theme::TEXT));
                                    }
                                }
                            });
                        }),
                    ];
                    grid(ui, per_row.min(3), col_w, gap, intro);

                    let rows = |labels: &[&str], avgs: &[Avg]| -> Vec<(String, Avg)> { labels.iter().zip(avgs).map(|(n, a)| (n.to_string(), *a)).collect() };
                    let groups: Vec<(&str, Vec<(String, Avg)>)> = vec![
                        ("By day of the week", rows(&reports::WEEKDAYS, &m.weekday)),
                        ("By time of day", rows(&DAYPART_LABELS, &m.daypart)),
                        ("By length of entry", rows(&LENGTH_LABELS, &m.length)),
                        ("By category", m.categories.iter().map(|(c, mean, n)| (c.clone(), Avg::from_mean(*mean, *n))).collect()),
                    ];
                    ui.add_space(8.0);
                    if table {
                        card(ui, &p, width.min(760.0), |ui| {
                            card_title(ui, &p, "All the numbers", "Average mood, 1 (awful) to 5 (great)");
                            for (title, rows) in &groups {
                                spaced_label(ui, title, p.faint);
                                egui::Grid::new(("mood_table", *title)).num_columns(3).spacing([28.0, 6.0]).show(ui, |ui| {
                                    for (label, avg) in rows {
                                        ui.label(RichText::new(label).color(p.ink));
                                        ui.label(RichText::new(avg.mean().map_or("–".into(), |m| format!("{m:.1}"))).color(p.title));
                                        ui.label(RichText::new(format!("{} {}", avg.n, if avg.n == 1 { "entry" } else { "entries" })).color(p.muted));
                                        ui.end_row();
                                    }
                                });
                                ui.add_space(6.0);
                            }
                        });
                    } else {
                        let charts: Vec<Box<dyn FnOnce(&mut egui::Ui, f32)>> = groups
                            .iter()
                            .filter(|(title, rows)| *title != "By category" || !rows.is_empty())
                            .map(|(title, rows)| {
                                let (title, rows, p) = (*title, rows.clone(), &p);
                                Box::new(move |ui: &mut egui::Ui, w: f32| {
                                    card(ui, p, w, |ui| {
                                        card_title(ui, p, title, "Average mood, 1 (awful) to 5 (great)");
                                        mood_bars(ui, p, w - 46.0, &rows);
                                    })
                                }) as Box<dyn FnOnce(&mut egui::Ui, f32)>
                            })
                            .collect();
                        grid(ui, per_row, col_w, gap, charts);
                    }

                    // Legend and the table switch.
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        for (shade, text) in [(p.accent, "3 or more ratings"), (p.accent.gamma_multiply(0.45), "Fewer than 3 — early signal only")] {
                            let (r, _) = ui.allocate_exact_size(vec2(22.0, 10.0), Sense::hover());
                            ui.painter().rect_filled(r, 5.0, shade);
                            ui.label(RichText::new(text).size(13.0).color(p.muted));
                            ui.add_space(10.0);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let label = if table { "View as bars" } else { "View as table" };
                            if ui.add(egui::Label::new(RichText::new(label).size(14.0).color(p.accent).underline()).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                table = !table;
                            }
                        });
                    });
                    ui.add_space(6.0);
                    ui.label(RichText::new("These are patterns in your own ratings, not causes. Groups with fewer than three entries are left out of the written notes.").size(12.5).color(p.faint));
                }
                Tab::Writing => {
                    let w = &cache.writing;
                    let (entries, words, avg_words) = (w.entries.to_string(), w.words.to_string(), w.avg_words.to_string());
                    let phrase = RANGE_PHRASES[range];
                    let top = w.top_words.clone();
                    let intro: Vec<Box<dyn FnOnce(&mut egui::Ui, f32)>> = vec![
                        Box::new(|ui, cw| stat_card(ui, &p, cw, "Entries", &entries, phrase)),
                        Box::new(|ui, cw| stat_card(ui, &p, cw, "Words", &words, "written")),
                        Box::new(|ui, cw| stat_card(ui, &p, cw, "Words per entry", &avg_words, "on average")),
                    ];
                    grid(ui, per_row.min(3), col_w, gap, intro);
                    ui.add_space(8.0);

                    let hours: Vec<String> = (0..24).map(|h| if h % 3 == 0 { format!("{h}") } else { String::new() }).collect();
                    let days: Vec<String> = reports::WEEKDAYS.iter().map(|d| d.to_string()).collect();
                    let longest = w.longest.clone();
                    let lines: Vec<(String, String, usize, String)> = longest
                        .iter()
                        .map(|(id, date, words)| {
                            let mut label = data.entries.iter().find(|e| e.id == *id).map(entry_label).unwrap_or_default();
                            if label.chars().count() > 48 {
                                label = label.chars().take(48).collect::<String>() + "…";
                            }
                            (date.clone(), label, *words, id.to_string())
                        })
                        .collect();
                    let by_hour = w.by_hour;
                    let by_weekday = w.by_weekday;
                    let cards: Vec<Box<dyn FnOnce(&mut egui::Ui, f32)>> = vec![
                        Box::new(|ui, cw| {
                            card(ui, &p, cw, |ui| {
                                card_title(ui, &p, "Words you use most", "Common filler words and private entries are left out.");
                                if top.is_empty() {
                                    ui.label(RichText::new("Write a few more entries and your favourite words will appear here.").color(p.muted));
                                } else {
                                    word_cloud(ui, &p, &top);
                                }
                            })
                        }),
                        Box::new(|ui, cw| {
                            card(ui, &p, cw, |ui| {
                                card_title(ui, &p, "When you write", "Entries by hour of the day");
                                column_chart(ui, &p, cw - 46.0, &by_hour, &hours);
                                ui.add_space(10.0);
                                ui.label(RichText::new("Entries by day of the week").size(13.5).color(p.faint));
                                column_chart(ui, &p, cw - 46.0, &by_weekday, &days);
                            })
                        }),
                        Box::new(|ui, cw| {
                            card(ui, &p, cw, |ui| {
                                card_title(ui, &p, "Longest entries", "");
                                if lines.is_empty() {
                                    ui.label(RichText::new("Nothing here yet.").color(p.muted));
                                }
                                for (date, label, words, _) in &lines {
                                    ui.label(RichText::new(format!("{date}  ·  {words} words")).size(13.0).color(p.faint));
                                    if ui.add(egui::Label::new(RichText::new(label).size(15.0).color(p.ink)).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                        jump = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok();
                                    }
                                    ui.add_space(4.0);
                                }
                            })
                        }),
                    ];
                    grid(ui, per_row, col_w, gap, cards);
                }
                Tab::Review => {
                    let r = &cache.review;
                    let s = &r.summary;
                    ui.label(RichText::new(r.title).font(FontId::new(30.0, fonts::serif_medium())).color(p.title));
                    ui.label(RichText::new(format!("{} to {}", r.from.format("%-d %B %Y"), r.to.format("%-d %B %Y"))).size(14.5).color(p.muted));
                    ui.add_space(14.0);
                    let figures = [
                        ("Entries", s.entries.to_string(), ""),
                        ("Days written", s.days_written.to_string(), ""),
                        ("Words", s.words.to_string(), ""),
                        ("Longest streak", s.longest_streak.to_string(), "days"),
                        ("Average mood", s.mood_avg.map_or("–".into(), |m| format!("{m:.1}")), "out of 5"),
                    ];
                    let items: Vec<Box<dyn FnOnce(&mut egui::Ui, f32)>> = figures
                        .iter()
                        .map(|(label, value, note)| {
                            let (label, value, note, p) = (*label, value.clone(), *note, &p);
                            Box::new(move |ui: &mut egui::Ui, w: f32| stat_card(ui, p, w, label, &value, note)) as Box<dyn FnOnce(&mut egui::Ui, f32)>
                        })
                        .collect();
                    let figs_per_row = if width >= 1000.0 { 5 } else { per_row.max(2) };
                    let fig_w = (width - gap * (figs_per_row as f32 - 1.0)) / figs_per_row as f32;
                    grid(ui, figs_per_row, fig_w, gap, items);

                    ui.add_space(4.0);
                    if let (Some(m), Some(prev)) = (s.mood_avg, r.previous_mood) {
                        let diff = m - prev;
                        let text = if diff.abs() < 0.05 {
                            "Your mood was about the same as in the period before.".to_string()
                        } else {
                            format!("Your mood was {} by {:.1} compared with the period before.", if diff > 0.0 { "up" } else { "down" }, diff.abs())
                        };
                        ui.label(RichText::new(text).size(15.0).color(p.ink));
                    }
                    if let Some((d, m)) = r.best_day {
                        ui.label(RichText::new(format!("Happiest day: {} ({m:.1})", d.format("%A %-d %B"))).size(15.0).color(p.ink));
                    }
                    if let Some((d, m)) = r.hardest_day.filter(|h| Some(h.0) != r.best_day.map(|b| b.0)) {
                        ui.label(RichText::new(format!("Hardest day: {} ({m:.1})", d.format("%A %-d %B"))).size(15.0).color(p.ink));
                    }
                    if s.goal_days > 0 {
                        ui.label(RichText::new(format!("You reached your daily word goal on {} {}.", s.goal_days, if s.goal_days == 1 { "day" } else { "days" })).size(15.0).color(p.ink));
                    }
                    ui.add_space(14.0);

                    let cats = s.categories.clone();
                    let words = r.top_words.clone();
                    let mut sections: Vec<Box<dyn FnOnce(&mut egui::Ui, f32) + '_>> = Vec::new();
                    if !cats.is_empty() {
                        let categories = &data.categories;
                        sections.push(Box::new(|ui, cw| {
                            card(ui, &p, cw, |ui| {
                                card_title(ui, &p, "Where you wrote", "");
                                let most = cats.first().map_or(1, |c| c.1).max(1) as f32;
                                for (name, n) in &cats {
                                    let (row, _) = ui.allocate_exact_size(vec2(cw - 46.0, 28.0), Sense::hover());
                                    let painter = ui.painter_at(row);
                                    painter.text(egui::pos2(row.min.x, row.center().y), Align2::LEFT_CENTER, name, FontId::proportional(14.0), p.ink);
                                    let track = egui::Rect::from_min_max(egui::pos2(row.min.x + (cw - 46.0) * 0.34, row.center().y - 7.0), egui::pos2(row.max.x - 40.0, row.center().y + 7.0));
                                    painter.rect_filled(track, 7.0, theme::BG);
                                    painter.rect_filled(egui::Rect::from_min_size(track.min, vec2(track.width() * *n as f32 / most, track.height())), 7.0, category_color(categories, name));
                                    painter.text(egui::pos2(row.max.x, row.center().y), Align2::RIGHT_CENTER, n.to_string(), FontId::proportional(13.0), p.muted);
                                }
                            })
                        }));
                    }
                    if !words.is_empty() {
                        sections.push(Box::new(|ui, cw| {
                            card(ui, &p, cw, |ui| {
                                card_title(ui, &p, "Words that kept coming up", "");
                                word_cloud(ui, &p, &words);
                            })
                        }));
                    }
                    let section_cols = per_row.min(2).max(1);
                    let section_w = (width - gap * (section_cols as f32 - 1.0)) / section_cols as f32;
                    grid(ui, section_cols, section_w, gap, sections);

                    if !r.pictures.is_empty() {
                        card(ui, &p, width, |ui| {
                            card_title(ui, &p, "Pictures", "");
                            ui.horizontal_wrapped(|ui| {
                                for (id, caption) in &r.pictures {
                                    if let Some(tex) = media.thumbnail(&ctx, &data.images, id) {
                                        let natural = tex.size_vec2();
                                        let scale = 120.0 / natural.x.max(natural.y);
                                        ui.image((tex.id(), natural * scale)).on_hover_text(caption);
                                    }
                                }
                            });
                        });
                        ui.add_space(gap - 8.0);
                    }
                    if !r.highlights.is_empty() {
                        card(ui, &p, width, |ui| {
                            card_title(ui, &p, "Entries to remember", "");
                            for h in &r.highlights {
                                let c = category_color(&data.categories, &h.category);
                                let entry_card = egui::Frame::new().fill(theme::BG).stroke(Stroke::new(1.0, p.border_soft)).corner_radius(12).inner_margin(14).show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        kit::category_pill(ui, c, &h.category);
                                        ui.label(RichText::new(format!("{}   {}{}", h.date, h.time, if h.pinned { "   (pinned)" } else { "" })).size(13.0).color(p.faint));
                                    });
                                    ui.label(RichText::new(&h.snippet).font(FontId::new(16.0, fonts::serif())).color(p.muted));
                                });
                                if ui.interact(entry_card.response.rect, ui.id().with(("hl", h.entry_id)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                    jump = NaiveDate::parse_from_str(&h.date, "%Y-%m-%d").ok();
                                }
                            }
                        });
                        ui.add_space(gap - 8.0);
                    }
                    ui.add_space(6.0);
                    if kit::button(ui, kit::Look::Ghost, None, "Save this review as a PDF…", None).clicked() {
                        export = true;
                    }
                    if !msg.is_empty() {
                        ui.label(RichText::new(&msg).size(13.5).color(p.muted));
                    }
                }
            }
            ui.add_space(30.0);
        });

        insights.tab = tab;
        insights.range = range;
        insights.period = period;
        insights.table = table;
        if export {
            let review = &insights.cache.as_ref().unwrap().review;
            let name = format!("journal-{}-{}.pdf", review.title.to_lowercase().replace(' ', "-"), review.from.format("%Y-%m-%d"));
            if let Some(dest) = rfd::FileDialog::new().set_file_name(name).add_filter("PDF document", &["pdf"]).save_file() {
                insights.msg = match export::build_review_pdf(review, &data.categories, &data.images, true)
                    .and_then(|bytes| std::fs::write(&dest, bytes).map_err(|e| format!("Couldn't save the PDF: {e}")))
                {
                    Ok(()) => format!("Saved {}", dest.display()),
                    Err(e) => e,
                };
            }
        }
        if let Some(d) = jump {
            self.selected = d;
            self.month = d.with_day(1).unwrap();
            self.day_scroll_pending = true;
            self.insights.open = false;
        }
    }
}
