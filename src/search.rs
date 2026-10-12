//! Searching the diary. Type words to find entries containing all of them; put a phrase in
//! "quotes" to match it exactly, and a `-` in front of a word to leave out entries containing it.
//! Filters narrow the results by category, mood, time period, starred entries and attachments,
//! and results can be sorted by date or by how well they match. Private entries are never
//! searched by their text.

use super::*;
use egui::text::{LayoutJob, TextFormat};

/// How far back to look.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Period {
    Any,
    Week,
    Month,
    Year,
    Custom,
}

impl Period {
    pub const ALL: [(Period, &'static str); 5] =
        [(Period::Any, "Any time"), (Period::Week, "Last 7 days"), (Period::Month, "Last 30 days"), (Period::Year, "Last year"), (Period::Custom, "Between dates…")];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Sort {
    Newest,
    Oldest,
    BestMatch,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Filters {
    /// Empty means every category.
    pub category: String,
    /// 0 means any mood, otherwise 1 to 5.
    pub mood: u8,
    pub period: Period,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub starred: bool,
    pub with_files: bool,
    pub sort: Sort,
}

impl Filters {
    pub fn new(today: NaiveDate) -> Self {
        Self { category: String::new(), mood: 0, period: Period::Any, from: today - chrono::Duration::days(30), to: today, starred: false, with_files: false, sort: Sort::Newest }
    }

    /// Is anything other than the sort order narrowing the results?
    pub fn is_active(&self) -> bool {
        !self.category.is_empty() || self.mood != 0 || self.period != Period::Any || self.starred || self.with_files
    }

    fn date_range(&self, today: NaiveDate) -> Option<(NaiveDate, NaiveDate)> {
        let back = |days| Some((today - chrono::Duration::days(days), today));
        match self.period {
            Period::Any => None,
            Period::Week => back(7),
            Period::Month => back(30),
            Period::Year => back(365),
            Period::Custom => Some((self.from.min(self.to), self.from.max(self.to))),
        }
    }
}

// ---- the query ---------------------------------------------------------------------------------

/// Lower-case one character at a time so positions in the folded text line up with the original.
fn fold(s: &str) -> Vec<char> {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

#[derive(Default, Debug, PartialEq)]
pub(crate) struct Query {
    /// Everything that must appear (already folded).
    pub include: Vec<Vec<char>>,
    /// Anything that must not appear.
    pub exclude: Vec<Vec<char>>,
}

impl Query {
    pub fn parse(text: &str) -> Query {
        let mut q = Query::default();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }
            let exclude = chars[i] == '-' && chars.get(i + 1).is_some_and(|c| !c.is_whitespace());
            if exclude {
                i += 1;
            }
            let word: String = if chars[i] == '"' {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '"' {
                    i += 1;
                }
                let phrase: String = chars[start..i].iter().collect();
                i += 1; // the closing quote
                phrase.split_whitespace().collect::<Vec<_>>().join(" ")
            } else {
                let start = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                chars[start..i].iter().collect()
            };
            if word.is_empty() {
                continue;
            }
            let term = fold(&word);
            if exclude { q.exclude.push(term) } else { q.include.push(term) }
        }
        q
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty()
    }
}

/// Every position where `needle` starts in `hay`.
fn find_all(hay: &[char], needle: &[char]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return Vec::new();
    }
    (0..=hay.len() - needle.len()).filter(|&i| hay[i..i + needle.len()] == *needle).collect()
}

// ---- the index and the results -----------------------------------------------------------------

/// What can be searched in one entry, prepared once per change of the diary.
struct Indexed {
    /// The entry as plain text (empty for private entries) and the same lower-cased.
    plain: Vec<char>,
    folded: Vec<char>,
    /// Category, date and attachment names, lower-cased.
    meta: Vec<char>,
}

fn index_entry(e: &Entry) -> Indexed {
    let plain_text = if e.sensitive { String::new() } else { markdown::plain_text(&e.text) };
    let mut meta = format!("{} {}", e.category, e.date);
    if !e.sensitive {
        for a in &e.attachments {
            meta.push(' ');
            meta.push_str(&a.name);
        }
    }
    Indexed { plain: plain_text.chars().collect(), folded: fold(&plain_text), meta: fold(&meta) }
}

/// A piece of the snippet and whether it matched.
pub(crate) type Snippet = Vec<(String, bool)>;

pub(crate) struct Hit {
    /// Index into `Data::entries`.
    pub entry: usize,
    pub score: usize,
    pub snippet: Snippet,
}

const BEFORE: usize = 45;
const AFTER: usize = 100;

/// A short piece of the text around the first match, with every match marked.
fn snippet(item: &Indexed, terms: &[Vec<char>]) -> Snippet {
    let positions: Vec<(usize, usize)> = terms
        .iter()
        .flat_map(|t| find_all(&item.folded, t).into_iter().map(move |p| (p, p + t.len())))
        .collect();
    let first = positions.iter().map(|r| r.0).min().unwrap_or(0);
    let start = first.saturating_sub(BEFORE);
    let end = (first + AFTER).min(item.plain.len());
    // Mark the characters that are part of a match, then cut the window into runs.
    let mut marked = vec![false; item.plain.len()];
    for (a, b) in positions {
        for m in &mut marked[a..b.min(item.plain.len())] {
            *m = true;
        }
    }
    let mut out: Snippet = Vec::new();
    if start > 0 {
        out.push(("…".into(), false));
    }
    for i in start..end {
        match out.last_mut() {
            Some((s, m)) if *m == marked[i] => s.push(item.plain[i]),
            _ => out.push((item.plain[i].to_string(), marked[i])),
        }
    }
    if end < item.plain.len() {
        out.push(("…".into(), false));
    }
    out
}

#[derive(Clone, PartialEq)]
struct CacheKey {
    text: String,
    filters: Filters,
    rev: u64,
    today: NaiveDate,
}

pub(crate) struct SearchState {
    /// The filter row is showing (and results are listed even with nothing typed).
    pub open: bool,
    pub filters: Filters,
    index: Option<(u64, Vec<Indexed>)>,
    cache: Option<(CacheKey, Vec<Hit>)>,
}

impl SearchState {
    pub fn new() -> Self {
        Self { open: false, filters: Filters::new(Local::now().date_naive()), index: None, cache: None }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Should the results replace the day panel?
    pub fn showing(&self, text: &str) -> bool {
        !text.trim().is_empty() || self.open || self.filters.is_active()
    }
}

/// Run a search. Pure: the caller supplies the entries and today's date.
#[cfg(test)]
pub(crate) fn run(entries: &[Entry], text: &str, f: &Filters, today: NaiveDate) -> Vec<Hit> {
    let items: Vec<Indexed> = entries.iter().map(index_entry).collect();
    run_indexed(entries, &items, text, f, today)
}

fn run_indexed(entries: &[Entry], items: &[Indexed], text: &str, f: &Filters, today: NaiveDate) -> Vec<Hit> {
    let q = Query::parse(text);
    let range = f.date_range(today);
    let mut hits = Vec::new();
    for (idx, (e, item)) in entries.iter().zip(items).enumerate() {
        if (!f.category.is_empty() && e.category != f.category)
            || (f.mood != 0 && e.mood != Some(f.mood))
            || (f.starred && !e.pinned)
            || (f.with_files && (e.attachments.is_empty() || e.sensitive))
        {
            continue;
        }
        if let Some((from, to)) = range {
            match NaiveDate::parse_from_str(&e.date, "%Y-%m-%d") {
                Ok(d) if d >= from && d <= to => {}
                _ => continue,
            }
        }
        let in_text = |t: &Vec<char>| find_all(&item.folded, t).len();
        let in_meta = |t: &Vec<char>| find_all(&item.meta, t).len();
        if q.exclude.iter().any(|t| in_text(t) + in_meta(t) > 0) {
            continue;
        }
        let mut score = 0;
        let mut all = true;
        for t in &q.include {
            let (a, b) = (in_text(t), in_meta(t));
            if a + b == 0 {
                all = false;
                break;
            }
            score += a + 3 * b;
        }
        if !all {
            continue;
        }
        let snip = if e.sensitive {
            vec![("Private entry".to_string(), false)]
        } else if q.include.is_empty() || !q.include.iter().any(|t| in_text(t) > 0) {
            snippet(item, &[])
        } else {
            snippet(item, &q.include)
        };
        hits.push(Hit { entry: idx, score, snippet: snip });
    }
    match f.sort {
        Sort::Newest => hits.sort_by(|a, b| entries[b.entry].added_at.cmp(&entries[a.entry].added_at)),
        Sort::Oldest => hits.sort_by(|a, b| entries[a.entry].added_at.cmp(&entries[b.entry].added_at)),
        Sort::BestMatch => hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| entries[b.entry].added_at.cmp(&entries[a.entry].added_at))),
    }
    hits
}

// ---- the panel ---------------------------------------------------------------------------------

/// A date picker button that works on chrono dates (the picker itself uses `jiff`).
fn date_button(ui: &mut egui::Ui, date: &mut NaiveDate, id: &str) {
    let mut picked = jiff::civil::date(date.year() as i16, date.month() as i8, date.day() as i8);
    ui.add(egui_extras::DatePickerButton::new(&mut picked).id_salt(id).show_icon(false));
    if let Some(d) = NaiveDate::from_ymd_opt(i32::from(picked.year()), picked.month() as u32, picked.day() as u32) {
        *date = d;
    }
}

const PAGE: usize = 200;

fn job(snip: &Snippet, fg: Color32, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    for (s, hit) in snip {
        let mut fmt = TextFormat { font_id: FontId::proportional(size), color: fg, ..Default::default() };
        if *hit {
            fmt.background = Color32::from_rgb(253, 224, 71);
            fmt.color = Color32::from_rgb(30, 41, 59);
        }
        job.append(s, 0.0, fmt);
    }
    job
}

impl DiaryApp {
    /// Replaces the day panel while searching.
    pub(crate) fn search_results(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let today = Local::now().date_naive();
        // Rebuild what is needed only when the diary or the question changed.
        if self.search_state.index.as_ref().map(|i| i.0) != Some(self.rev) {
            self.search_state.index = Some((self.rev, self.data.entries.iter().map(index_entry).collect()));
            self.search_state.cache = None;
        }
        let key = CacheKey { text: self.search.clone(), filters: self.search_state.filters.clone(), rev: self.rev, today };
        if self.search_state.cache.as_ref().map(|c| &c.0) != Some(&key) {
            let items = &self.search_state.index.as_ref().unwrap().1;
            let hits = run_indexed(&self.data.entries, items, &self.search, &self.search_state.filters, today);
            self.search_state.cache = Some((key, hits));
        }

        let categories: Vec<String> = self.data.categories.iter().map(|c| c.name.clone()).collect();
        let mut clear = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Search").size(22.0).strong().color(p.title));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Clear").on_hover_text("Clear the search and the filters").clicked() {
                    clear = true;
                }
            });
        });
        let f = &mut self.search_state.filters;
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("search_cat")
                .selected_text(if f.category.is_empty() { "Any category".to_string() } else { f.category.clone() })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut f.category, String::new(), "Any category");
                    for c in &categories {
                        ui.selectable_value(&mut f.category, c.clone(), c);
                    }
                });
            egui::ComboBox::from_id_salt("search_mood")
                .selected_text(if f.mood == 0 { "Any mood" } else { MOOD_LABELS[usize::from(f.mood) - 1] })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut f.mood, 0, "Any mood");
                    for (i, label) in MOOD_LABELS.iter().enumerate() {
                        ui.selectable_value(&mut f.mood, i as u8 + 1, *label);
                    }
                });
            let period_label = Period::ALL.iter().find(|(p, _)| *p == f.period).map_or("", |(_, l)| *l);
            egui::ComboBox::from_id_salt("search_period").selected_text(period_label).show_ui(ui, |ui| {
                for (period, label) in Period::ALL {
                    ui.selectable_value(&mut f.period, period, label);
                }
            });
        });
        if f.period == Period::Custom {
            ui.horizontal(|ui| {
                ui.label(RichText::new("From").color(p.ink));
                date_button(ui, &mut f.from, "search_from");
                ui.label(RichText::new("to").color(p.ink));
                date_button(ui, &mut f.to, "search_to");
            });
        }
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut f.starred, "Starred only");
            ui.checkbox(&mut f.with_files, "Has attachments");
            egui::ComboBox::from_id_salt("search_sort")
                .selected_text(match f.sort {
                    Sort::Newest => "Newest first",
                    Sort::Oldest => "Oldest first",
                    Sort::BestMatch => "Best match",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut f.sort, Sort::Newest, "Newest first");
                    ui.selectable_value(&mut f.sort, Sort::Oldest, "Oldest first");
                    ui.selectable_value(&mut f.sort, Sort::BestMatch, "Best match");
                });
        });

        let hits = &self.search_state.cache.as_ref().unwrap().1;
        let found = match hits.len() {
            0 => "Nothing found.".to_string(),
            1 => "1 entry found.".to_string(),
            n => format!("{n} entries found."),
        };
        ui.label(RichText::new(format!("{found} Click one to open its day.")).color(p.muted));
        ui.label(RichText::new("Tip: \"quotes\" match an exact phrase; a - before a word leaves it out.").small().color(p.muted));
        ui.separator();

        let mut jump = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for hit in hits.iter().take(PAGE) {
                let e = &self.data.entries[hit.entry];
                let c = category_color(&self.data.categories, &e.category);
                let fg = text_on(c);
                let card = egui::Frame::new().fill(c).corner_radius(10).inner_margin(10).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&e.date).strong().color(fg));
                        ui.label(RichText::new(e.added_at.get(11..16).unwrap_or("")).small().color(fg));
                        if e.pinned {
                            ui.label(RichText::new("★").color(fg));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(&e.category).small().color(fg));
                            if let Some(m) = e.mood {
                                mood_badge(ui, m);
                            }
                        });
                    });
                    let mut j = job(&hit.snippet, fg, 15.0);
                    j.wrap.max_width = ui.available_width();
                    ui.label(j);
                });
                let resp = ui.interact(card.response.rect, ui.id().with(("hit", e.id)), Sense::click());
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    jump = NaiveDate::parse_from_str(&e.date, "%Y-%m-%d").ok();
                }
                ui.add_space(6.0);
            }
            if hits.len() > PAGE {
                ui.label(RichText::new(format!("Showing the first {PAGE}. Add a word or a filter to narrow it down.")).color(p.muted));
            }
        });

        if let Some(d) = jump {
            self.selected = d;
            self.month = d.with_day(1).unwrap();
            clear = true;
        }
        if clear {
            self.search.clear();
            self.search_state.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn entry(id: u64, date: &str, category: &str, text: &str) -> Entry {
        Entry { id, date: date.into(), added_at: format!("{date} 10:00:00"), category: category.into(), text: text.into(), ..Default::default() }
    }

    fn sample() -> Vec<Entry> {
        let mut v = vec![
            entry(1, "2026-10-01", "Personal", "Went for a **long** walk by the river with Sam."),
            entry(2, "2026-10-05", "Work", "Planning meeting. The river project is late."),
            entry(3, "2026-10-09", "Health", "Doctor said rest. River walk helped, river views are calming."),
            entry(4, "2026-09-20", "Personal", "Quiet day, read a book."),
            entry(5, "2026-10-10", "Personal", "Secret about the river"),
        ];
        v[0].mood = Some(5);
        v[2].mood = Some(3);
        v[2].pinned = true;
        v[4].sensitive = true;
        v[1].attachments.push(Attachment { id: "f1".into(), name: "Budget.xlsx".into(), size: 10 });
        v
    }

    fn ids(entries: &[Entry], hits: &[Hit]) -> Vec<u64> {
        hits.iter().map(|h| entries[h.entry].id).collect()
    }

    fn find(entries: &[Entry], q: &str, f: &Filters) -> Vec<u64> {
        ids(entries, &run(entries, q, f, d(2026, 10, 12)))
    }

    fn plain() -> Filters {
        Filters::new(d(2026, 10, 12))
    }

    #[test]
    fn queries_split_into_words_phrases_and_exclusions() {
        let q = Query::parse(r#"River  "long walk" -Work  - x -"late project""#);
        assert_eq!(q.include, vec![fold("river"), fold("long walk"), fold("-"), fold("x")]);
        assert_eq!(q.exclude, vec![fold("work"), fold("late project")]);
        assert!(Query::parse("   ").is_empty());
        // An unfinished quote takes the rest of the line.
        assert_eq!(Query::parse(r#""half open"#).include, vec![fold("half open")]);
        assert!(Query::parse(r#""""#).is_empty());
    }

    #[test]
    fn every_word_must_match_in_any_order_ignoring_case() {
        let e = sample();
        assert_eq!(find(&e, "river", &plain()), vec![3, 2, 1]);
        assert_eq!(find(&e, "WALK river", &plain()), vec![3, 1]);
        assert_eq!(find(&e, "sam walk", &plain()), vec![1]);
        assert_eq!(find(&e, "zebra", &plain()), Vec::<u64>::new());
        // Markdown marks aren't part of the text.
        assert_eq!(find(&e, "long walk", &plain()), vec![1]);
        assert_eq!(find(&e, "**long**", &plain()), Vec::<u64>::new());
    }

    #[test]
    fn phrases_are_exact_and_exclusions_remove_entries() {
        let e = sample();
        assert_eq!(find(&e, "\"river walk\"", &plain()), vec![3]);
        assert_eq!(find(&e, "\"walk river\"", &plain()), Vec::<u64>::new());
        assert_eq!(find(&e, "river -project", &plain()), vec![3, 1]);
        assert_eq!(find(&e, "-river", &plain()), vec![5, 4], "private entries have no searchable text");
    }

    #[test]
    fn private_entries_are_never_found_by_their_text() {
        let e = sample();
        assert!(!find(&e, "secret", &plain()).contains(&5));
        assert!(!find(&e, "river", &plain()).contains(&5));
        // They can still be found by category or date, and show no text.
        let hits = run(&e, "2026-10-10", &plain(), d(2026, 10, 12));
        assert_eq!(ids(&e, &hits), vec![5]);
        assert_eq!(hits[0].snippet, vec![("Private entry".to_string(), false)]);
        // A private entry's attachments aren't searched either.
        let mut e = sample();
        e[4].attachments.push(Attachment { id: "x".into(), name: "diary-secret.pdf".into(), size: 1 });
        assert!(find(&e, "diary-secret", &plain()).is_empty());
        let mut f = plain();
        f.with_files = true;
        assert_eq!(find(&e, "", &f), vec![2]);
    }

    #[test]
    fn category_date_and_attachment_names_are_searched() {
        let e = sample();
        assert_eq!(find(&e, "health", &plain()), vec![3]);
        assert_eq!(find(&e, "2026-09", &plain()), vec![4]);
        assert_eq!(find(&e, "budget xlsx", &plain()), vec![2]);
    }

    #[test]
    fn filters_narrow_the_results_with_or_without_words() {
        let e = sample();
        let mut f = plain();
        f.category = "Personal".into();
        assert_eq!(find(&e, "", &f), vec![5, 1, 4]);
        assert_eq!(find(&e, "river", &f), vec![1]);

        let mut f = plain();
        f.mood = 5;
        assert_eq!(find(&e, "", &f), vec![1]);
        f.mood = 3;
        assert_eq!(find(&e, "river", &f), vec![3]);

        let mut f = plain();
        f.starred = true;
        assert_eq!(find(&e, "", &f), vec![3]);

        let mut f = plain();
        f.with_files = true;
        assert_eq!(find(&e, "", &f), vec![2]);

        // Everything at once is an AND.
        let mut f = plain();
        f.category = "Personal".into();
        f.mood = 5;
        f.starred = true;
        assert!(find(&e, "", &f).is_empty());
        assert!(!plain().is_active());
        assert!(f.is_active());
    }

    #[test]
    fn periods_count_back_from_today_and_custom_ranges_include_both_ends() {
        let e = sample();
        let mut f = plain();
        f.period = Period::Week;
        assert_eq!(find(&e, "", &f), vec![5, 3, 2], "7 days back from 12 Oct is 5 Oct");
        f.period = Period::Month;
        assert_eq!(find(&e, "", &f), vec![5, 3, 2, 1, 4]);
        f.period = Period::Custom;
        f.from = d(2026, 10, 5);
        f.to = d(2026, 10, 1); // given the wrong way round
        assert_eq!(find(&e, "", &f), vec![2, 1]);
        f.from = d(2026, 9, 20);
        f.to = d(2026, 9, 20);
        assert_eq!(find(&e, "", &f), vec![4]);
    }

    #[test]
    fn results_can_be_sorted() {
        let e = sample();
        let mut f = plain();
        f.sort = Sort::Oldest;
        assert_eq!(find(&e, "river", &f), vec![1, 2, 3]);
        f.sort = Sort::BestMatch;
        // Entry 3 says "river" twice, the others once.
        assert_eq!(find(&e, "river", &f), vec![3, 2, 1]);
        f.sort = Sort::Newest;
        assert_eq!(find(&e, "river", &f), vec![3, 2, 1]);
    }

    #[test]
    fn snippets_show_the_match_with_context_and_mark_every_occurrence() {
        let e = sample();
        let hits = run(&e, "river", &plain(), d(2026, 10, 12));
        let top = hits.iter().find(|h| e[h.entry].id == 3).unwrap();
        let joined: String = top.snippet.iter().map(|(s, _)| s.as_str()).collect();
        assert!(joined.contains("River walk helped, river views"), "{joined}");
        let marked: Vec<&str> = top.snippet.iter().filter(|(_, m)| *m).map(|(s, _)| s.as_str()).collect();
        assert_eq!(marked, vec!["River", "river"], "the original case is kept");

        // A long entry is cut around the match with ellipses.
        let long = format!("{} needle {}", "start ".repeat(40), "end ".repeat(60));
        let entries = vec![entry(1, "2026-10-01", "Personal", &long)];
        let hit = &run(&entries, "needle", &plain(), d(2026, 10, 12))[0];
        assert_eq!(hit.snippet.first().unwrap().0.chars().next(), Some('…'));
        assert_eq!(hit.snippet.last().unwrap().0.chars().last(), Some('…'));
        assert!(hit.snippet.iter().map(|(s, _)| s.chars().count()).sum::<usize>() < 170);
        assert!(hit.snippet.iter().any(|(s, m)| *m && s == "needle"));

        // With no words, the start of the entry is shown, unmarked.
        let hit = &run(&e, "", &plain(), d(2026, 10, 12))[0];
        assert!(hit.snippet.iter().all(|(_, m)| !m));
    }

    #[test]
    fn accents_and_capitals_fold_without_shifting_positions() {
        let entries = vec![entry(1, "2026-10-01", "Personal", "Café ÉCLAIR with İstanbul friends")];
        let hits = run(&entries, "éclair friends", &plain(), d(2026, 10, 12));
        assert_eq!(hits.len(), 1);
        let marked: Vec<&str> = hits[0].snippet.iter().filter(|(_, m)| *m).map(|(s, _)| s.as_str()).collect();
        assert_eq!(marked, vec!["ÉCLAIR", "friends"]);
    }

    #[test]
    fn the_panel_shows_only_when_there_is_something_to_show() {
        let mut s = SearchState::new();
        assert!(!s.showing("") && !s.showing("   "));
        assert!(s.showing("river"));
        s.filters.starred = true;
        assert!(s.showing(""));
        s.reset();
        s.open = true;
        assert!(s.showing(""));
        s.reset();
        assert!(!s.showing(""));
    }
}

impl SearchState {
    /// How many results the last search found.
    #[cfg(test)]
    pub fn hit_count(&self) -> Option<usize> {
        self.cache.as_ref().map(|c| c.1.len())
    }
}
