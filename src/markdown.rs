//! Rich text for diary entries: entries are stored as Markdown, rendered with egui, and edited
//! through a toolbar that inserts Markdown syntax.
//!
//! Images are stored inside the (encrypted) diary data and referenced as `![name](img:ID)`.

use std::collections::{BTreeMap, HashMap};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use eframe::egui::{self, Color32, FontFamily, Label, TextureHandle, TextureOptions};
use eframe::egui::{FontId, Stroke, text::{LayoutJob, TextFormat}};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

pub const IMAGE_SCHEME: &str = "img:";

// ---------------------------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub(crate) struct Fmt {
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) strike: bool,
    pub(crate) code: bool,
    pub(crate) link: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Inline {
    Text(String, Fmt),
    Break,
    Image { src: String, alt: String },
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Block {
    Para(Vec<Inline>),
    Heading(u8, Vec<Inline>),
    Quote(Vec<Block>),
    List { start: Option<u64>, items: Vec<Vec<Block>> },
    Code(String),
    Rule,
}

struct Cursor<'a> {
    events: Vec<Event<'a>>,
    i: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<&Event<'a>> {
        self.events.get(self.i)
    }

    fn next(&mut self) -> Option<Event<'a>> {
        let e = self.events.get(self.i).cloned();
        if e.is_some() {
            self.i += 1;
        }
        e
    }

    /// Does the next event end or start a block, i.e. stop an inline run?
    fn at_block_boundary(&self) -> bool {
        match self.peek() {
            None | Some(Event::End(_)) | Some(Event::Rule) => true,
            Some(Event::Start(t)) => !matches!(
                t,
                Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. }
            ),
            _ => false,
        }
    }

    fn inlines(&mut self, fmt: Fmt) -> Vec<Inline> {
        let mut out = Vec::new();
        while !self.at_block_boundary() {
            match self.next() {
                Some(Event::Text(t) | Event::Html(t) | Event::InlineHtml(t)) => out.push(Inline::Text(t.to_string(), fmt)),
                Some(Event::Code(t)) => out.push(Inline::Text(t.to_string(), Fmt { code: true, ..fmt })),
                Some(Event::SoftBreak | Event::HardBreak) => out.push(Inline::Break),
                Some(Event::Start(Tag::Strong)) => self.nested(&mut out, Fmt { bold: true, ..fmt }),
                Some(Event::Start(Tag::Emphasis)) => self.nested(&mut out, Fmt { italic: true, ..fmt }),
                Some(Event::Start(Tag::Strikethrough)) => self.nested(&mut out, Fmt { strike: true, ..fmt }),
                Some(Event::Start(Tag::Link { .. })) => self.nested(&mut out, Fmt { link: true, ..fmt }),
                Some(Event::Start(Tag::Image { dest_url, .. })) => {
                    let mut alt = String::new();
                    while let Some(e) = self.next() {
                        match e {
                            Event::Text(t) | Event::Code(t) => alt.push_str(&t),
                            Event::End(TagEnd::Image) => break,
                            _ => {}
                        }
                    }
                    out.push(Inline::Image { src: dest_url.to_string(), alt });
                }
                _ => {}
            }
        }
        out
    }

    fn nested(&mut self, out: &mut Vec<Inline>, fmt: Fmt) {
        let inner = self.inlines(fmt);
        out.extend(inner);
        // Consume the matching End tag.
        if matches!(self.peek(), Some(Event::End(_))) {
            self.i += 1;
        }
    }

    fn eat_end(&mut self) {
        if matches!(self.peek(), Some(Event::End(_))) {
            self.i += 1;
        }
    }

    /// Parse blocks until the end of the enclosing container (or the end of input).
    fn blocks(&mut self) -> Vec<Block> {
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None => break,
                Some(Event::End(_)) => {
                    self.i += 1;
                    break;
                }
                Some(Event::Rule) => {
                    self.i += 1;
                    out.push(Block::Rule);
                }
                Some(Event::Start(tag)) => match tag.clone() {
                    Tag::Paragraph => {
                        self.i += 1;
                        let inl = self.inlines(Fmt::default());
                        self.eat_end();
                        out.push(Block::Para(inl));
                    }
                    Tag::Heading { level, .. } => {
                        self.i += 1;
                        let inl = self.inlines(Fmt::default());
                        self.eat_end();
                        out.push(Block::Heading(level as u8, inl));
                    }
                    Tag::BlockQuote(_) => {
                        self.i += 1;
                        out.push(Block::Quote(self.blocks()));
                    }
                    Tag::List(start) => {
                        self.i += 1;
                        let mut items = Vec::new();
                        loop {
                            match self.peek() {
                                Some(Event::Start(Tag::Item)) => {
                                    self.i += 1;
                                    items.push(self.blocks());
                                }
                                Some(Event::End(_)) => {
                                    self.i += 1;
                                    break;
                                }
                                None => break,
                                _ => self.i += 1,
                            }
                        }
                        out.push(Block::List { start, items });
                    }
                    Tag::CodeBlock(_) => {
                        self.i += 1;
                        let mut code = String::new();
                        while let Some(e) = self.next() {
                            match e {
                                Event::Text(t) => code.push_str(&t),
                                Event::End(_) => break,
                                _ => {}
                            }
                        }
                        out.push(Block::Code(code.trim_end_matches('\n').to_string()));
                    }
                    Tag::HtmlBlock => self.i += 1,
                    // Inline content directly inside a tight list item.
                    Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. } => {
                        let inl = self.inlines(Fmt::default());
                        out.push(Block::Para(inl));
                    }
                    // Anything unsupported: step over the start tag, keep its text.
                    _ => self.i += 1,
                },
                Some(_) => {
                    let before = self.i;
                    let inl = self.inlines(Fmt::default());
                    if !inl.is_empty() {
                        out.push(Block::Para(inl));
                    }
                    if self.i == before {
                        self.i += 1;
                    }
                }
            }
        }
        out
    }
}

pub(crate) fn parse(md: &str) -> Vec<Block> {
    let events: Vec<Event> = Parser::new_ext(md, Options::ENABLE_STRIKETHROUGH).collect();
    Cursor { events, i: 0 }.blocks()
}

/// Plain-text one-liner for the calendar: the first paragraph, heading or list item.
pub fn summary(md: &str) -> String {
    fn flatten(inl: &[Inline]) -> String {
        let mut s = String::new();
        for i in inl {
            match i {
                Inline::Text(t, _) => s.push_str(t),
                Inline::Break => s.push(' '),
                Inline::Image { alt, .. } => s.push_str(if alt.is_empty() { "[image]" } else { alt }),
            }
        }
        s
    }
    fn first(blocks: &[Block]) -> Option<String> {
        for b in blocks {
            let s = match b {
                Block::Para(i) | Block::Heading(_, i) => Some(flatten(i)),
                Block::Quote(b) => first(b),
                Block::List { items, .. } => items.iter().find_map(|i| first(i)),
                Block::Code(c) => Some(c.lines().next().unwrap_or("").to_string()),
                Block::Rule => None,
            };
            if let Some(s) = s.filter(|s| !s.trim().is_empty()) {
                return Some(s.trim().to_string());
            }
        }
        None
    }
    first(&parse(md)).unwrap_or_default()
}

/// Ids of the images an entry refers to.
pub fn image_ids(md: &str) -> Vec<String> {
    Parser::new(md)
        .filter_map(|e| match e {
            Event::Start(Tag::Image { dest_url, .. }) => dest_url.strip_prefix(IMAGE_SCHEME).map(str::to_string),
            _ => None,
        })
        .collect()
}

/// The pictures in an entry, as (id, caption) pairs in order.
pub fn images(md: &str) -> Vec<(String, String)> {
    let (mut found, mut current): (Vec<(String, String)>, Option<(String, String)>) = (Vec::new(), None);
    for event in Parser::new(md) {
        match event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                current = dest_url.strip_prefix(IMAGE_SCHEME).map(|id| (id.to_string(), String::new()));
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, alt)) = current.as_mut() {
                    alt.push_str(&t);
                }
            }
            Event::End(TagEnd::Image) => found.extend(current.take()),
            _ => {}
        }
    }
    found
}

// ---------------------------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------------------------

pub struct Style {
    pub text: Color32,
    pub link: Color32,
    pub size: f32,
    /// Set the text in the serif face (Newsreader) with a roomier line height.
    pub serif: bool,
}

/// Decoded images turned into textures, created on demand.
#[derive(Default)]
pub struct Media {
    textures: HashMap<String, Option<TextureHandle>>,
    thumbs: HashMap<String, Option<TextureHandle>>,
}

impl Media {
    pub fn texture(&mut self, ctx: &egui::Context, images: &BTreeMap<String, String>, id: &str) -> Option<TextureHandle> {
        self.textures
            .entry(id.to_string())
            .or_insert_with(|| {
                let bytes = B64.decode(images.get(id)?).ok()?;
                let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                Some(ctx.load_texture(format!("diary-{id}"), color, TextureOptions::LINEAR))
            })
            .clone()
    }

    /// A small (up to 256 px) version of a picture, for the gallery.
    pub fn thumbnail(&mut self, ctx: &egui::Context, images: &BTreeMap<String, String>, id: &str) -> Option<TextureHandle> {
        self.thumbs
            .entry(id.to_string())
            .or_insert_with(|| {
                let bytes = B64.decode(images.get(id)?).ok()?;
                let img = image::load_from_memory(&bytes).ok()?.thumbnail(256, 256).to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                Some(ctx.load_texture(format!("diary-thumb-{id}"), color, TextureOptions::LINEAR))
            })
            .clone()
    }
}

pub fn bold_family() -> FontFamily {
    crate::fonts::ui_bold()
}

fn job_for(inlines: &[Inline], style: &Style, size: f32, force_bold: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    for inl in inlines {
        let (text, fmt) = match inl {
            Inline::Text(t, f) => (t.as_str(), *f),
            Inline::Break => ("\n", Fmt::default()),
            Inline::Image { .. } => continue,
        };
        let family = if fmt.code {
            FontFamily::Monospace
        } else if style.serif {
            if fmt.bold || force_bold {
                crate::fonts::serif_bold()
            } else if fmt.italic {
                crate::fonts::serif_italic()
            } else {
                crate::fonts::serif()
            }
        } else if fmt.bold || force_bold {
            bold_family()
        } else {
            FontFamily::Proportional
        };
        let real_italic = style.serif && fmt.italic && !fmt.code && !(fmt.bold || force_bold);
        let color = if fmt.link { style.link } else { style.text };
        let fmt_out = TextFormat {
            font_id: FontId::new(if fmt.code { size * 0.92 } else { size }, family),
            color,
            italics: fmt.italic && !real_italic,
            line_height: style.serif.then(|| size * 1.55),
            strikethrough: if fmt.strike { Stroke::new(1.0, color) } else { Stroke::NONE },
            underline: if fmt.link { Stroke::new(1.0, color) } else { Stroke::NONE },
            background: if fmt.code { style.text.gamma_multiply(0.13) } else { Color32::TRANSPARENT },
            ..Default::default()
        };
        job.append(text, 0.0, fmt_out);
    }
    job
}

fn render_inlines(
    ui: &mut egui::Ui,
    inlines: &[Inline],
    style: &Style,
    size: f32,
    bold: bool,
    media: &mut Media,
    images: &BTreeMap<String, String>,
) {
    // Text runs are laid out together; images interrupt them.
    let mut run: Vec<Inline> = Vec::new();
    let flush = |ui: &mut egui::Ui, run: &mut Vec<Inline>| {
        if !run.is_empty() {
            ui.add(Label::new(job_for(run, style, size, bold)));
            run.clear();
        }
    };
    for inl in inlines {
        match inl {
            Inline::Image { src, alt } => {
                flush(ui, &mut run);
                show_image(ui, media, images, src, alt, style.text);
            }
            other => run.push(other.clone()),
        }
    }
    flush(ui, &mut run);
}

fn render_blocks(
    ui: &mut egui::Ui,
    blocks: &[Block],
    style: &Style,
    media: &mut Media,
    images: &BTreeMap<String, String>,
    depth: usize,
) {
    for (n, block) in blocks.iter().enumerate() {
        if n > 0 {
            ui.add_space(5.0);
        }
        match block {
            Block::Para(inl) => render_inlines(ui, inl, style, style.size, false, media, images),
            Block::Heading(level, inl) => {
                let k = match level {
                    1 => 1.7,
                    2 => 1.45,
                    3 => 1.25,
                    4 => 1.12,
                    _ => 1.0,
                };
                render_inlines(ui, inl, style, style.size * k, true, media, images);
            }
            Block::Quote(inner) => {
                ui.indent(("quote", depth, n), |ui| render_blocks(ui, inner, style, media, images, depth + 1));
            }
            Block::List { start, items } => {
                for (i, item) in items.iter().enumerate() {
                    ui.horizontal_top(|ui| {
                        let marker = match start {
                            Some(s) => format!("{}.", s + i as u64),
                            None => "\u{2022}".to_string(),
                        };
                        ui.add(Label::new(egui::RichText::new(marker).size(style.size).color(style.text)));
                        ui.vertical(|ui| render_blocks(ui, item, style, media, images, depth + 1));
                    });
                }
            }
            Block::Code(code) => {
                egui::Frame::new().fill(style.text.gamma_multiply(0.1)).corner_radius(6).inner_margin(8).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(Label::new(egui::RichText::new(code).monospace().size(style.size * 0.92).color(style.text)));
                });
            }
            Block::Rule => {
                ui.separator();
            }
        }
    }
}

/// Draw `md` into `ui`.
pub fn render(ui: &mut egui::Ui, md: &str, style: &Style, media: &mut Media, images: &BTreeMap<String, String>) {
    // Treat every line break as a line break (plain-text entries keep their layout).
    let md = md.replace("\r\n", "\n").replace('\n', "  \n");
    let blocks = parse(&md);
    ui.vertical(|ui| render_blocks(ui, &blocks, style, media, images, 0));
}

// ---------------------------------------------------------------------------------------------
// Editing helpers (toolbar actions)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Format {
    Bold,
    Italic,
    Heading(u8),
    Bullet,
    Numbered,
    /// Normal: take the formatting off the selected lines (or the line the cursor is on).
    Clear,
}

/// Apply `format` to the selection (char indices, start <= end). Returns the new selection.
pub fn apply(text: &mut String, sel: (usize, usize), format: Format) -> (usize, usize) {
    let mut chars: Vec<char> = text.chars().collect();
    let (a, b) = (sel.0.min(chars.len()), sel.1.min(chars.len()));
    let (a, b) = (a.min(b), a.max(b));

    let result = match format {
        Format::Bold => wrap(&mut chars, a, b, "**"),
        Format::Italic => wrap(&mut chars, a, b, "*"),
        Format::Heading(n) => prefix_lines(&mut chars, a, b, |_| format!("{} ", "#".repeat(n as usize))),
        Format::Bullet => prefix_lines(&mut chars, a, b, |_| "- ".to_string()),
        Format::Numbered => prefix_lines(&mut chars, a, b, |i| format!("{}. ", i + 1)),
        Format::Clear => clear_formatting(&mut chars, a, b),
    };
    *text = chars.into_iter().collect();
    result
}

fn wrap(chars: &mut Vec<char>, a: usize, b: usize, marker: &str) -> (usize, usize) {
    let m: Vec<char> = marker.chars().collect();
    let n = m.len();
    // Asterisks next to the selection: one pair is italic, two are bold, three are both. So italic
    // is on when the pair count is odd, and bold when there are at least two on each side.
    let stars_before = chars[..a].iter().rev().take_while(|c| **c == '*').count();
    let stars_after = chars[b..].iter().take_while(|c| **c == '*').count();
    let around = stars_before.min(stars_after);
    let wrapped = if m[0] == '*' {
        if n == 1 { around % 2 == 1 } else { around >= 2 }
    } else {
        a >= n && chars[a - n..a] == m[..] && chars.len() >= b + n && chars[b..b + n] == m[..]
    };
    if wrapped {
        // Already wrapped: remove the markers.
        chars.drain(b..b + n);
        chars.drain(a - n..a);
        (a - n, b - n)
    } else {
        for (k, c) in m.iter().enumerate() {
            chars.insert(b + k, *c);
        }
        for (k, c) in m.iter().enumerate() {
            chars.insert(a + k, *c);
        }
        (a + n, b + n)
    }
}

/// Add (or, if every line already has it, remove) a line prefix on each selected line.
fn prefix_lines(chars: &mut Vec<char>, a: usize, b: usize, prefix: impl Fn(usize) -> String) -> (usize, usize) {
    let start = chars[..a].iter().rposition(|c| *c == '\n').map_or(0, |p| p + 1);
    let end = chars[b..].iter().position(|c| *c == '\n').map_or(chars.len(), |p| b + p);
    let block: String = chars[start..end].iter().collect();
    let lines: Vec<&str> = block.split('\n').collect();

    let strip = strip_line_prefix;

    let all_have = lines.iter().enumerate().all(|(i, l)| {
        let want = prefix(i);
        l.starts_with(&want)
    });
    let new_lines: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if all_have {
                strip(l).1
            } else {
                format!("{}{}", prefix(i), strip(l).1)
            }
        })
        .collect();
    let joined = new_lines.join("\n");
    let new_len = joined.chars().count();
    chars.splice(start..end, joined.chars());
    (start, start + new_len)
}

/// The word the cursor is in or next to, as a range of characters (letters, digits, apostrophes and underscores).
pub fn word_at(chars: &[char], pos: usize) -> Option<(usize, usize)> {
    let is_word = |c: char| c.is_alphanumeric() || c == '\'' || c == '_';
    let pos = pos.min(chars.len());
    let mut start = pos;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = pos;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }
    (end > start).then_some((start, end))
}

/// Take the line prefix (heading marks, a bullet, a number, a quote mark) off a line: how many characters it was,
/// and the rest. A `#tag` is not a heading: the `#` must be followed by a space.
fn strip_line_prefix(line: &str) -> (usize, String) {
    let t = line.trim_start_matches('#');
    let hashes = line.len() - t.len();
    if hashes > 0 && hashes <= 6 && t.starts_with(' ') {
        return (hashes + 1, t[1..].to_string());
    }
    for p in ["- ", "* ", "> "] {
        if let Some(rest) = line.strip_prefix(p) {
            return (p.len(), rest.to_string());
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && line[digits..].starts_with(". ") {
        return (digits + 2, line[digits + 2..].to_string());
    }
    (0, line.to_string())
}

/// Take emphasis marks out of a line: `***bold italic***`, `**bold**`, `~~struck~~`, `*italic*`, `_italic_` and
/// `` `code` ``, leaving the words. Marks that aren't a matching pair, links, pictures and `#tags` are left alone.
fn strip_inline_marks(line: &str) -> String {
    let mut c: Vec<char> = line.chars().collect();
    if line.contains("](img:") {
        return line.to_string();
    }
    let starts = |c: &[char], i: usize, pat: &str| pat.chars().enumerate().all(|(k, p)| c.get(i + k) == Some(&p));
    // Paired marks of more than one character, then code, then single stars and underscores.
    for pat in ["***", "**", "~~"] {
        let n = pat.chars().count();
        let mut i = 0;
        while i + n <= c.len() {
            if starts(&c, i, pat) && c.get(i + n).is_some_and(|x| !x.is_whitespace() && *x != pat.chars().next().unwrap()) {
                let close = (i + n + 1..c.len()).find(|&j| starts(&c, j, pat) && !c[j - 1].is_whitespace() && c.get(j + n) != pat.chars().next().as_ref());
                if let Some(j) = close {
                    c.drain(j..j + n);
                    c.drain(i..i + n);
                    continue;
                }
            }
            i += 1;
        }
    }
    let mut i = 0;
    while i < c.len() {
        if c[i] == '`' {
            if let Some(j) = (i + 1..c.len()).find(|&j| c[j] == '`') {
                c.remove(j);
                c.remove(i);
                continue;
            }
        }
        i += 1;
    }
    let mut i = 0;
    while i < c.len() {
        let before_ok = i == 0 || !(c[i - 1].is_alphanumeric() || c[i - 1] == c[i]);
        if (c[i] == '*' || c[i] == '_') && before_ok && c.get(i + 1).is_some_and(|x| !x.is_whitespace() && *x != c[i]) {
            let m = c[i];
            let close = (i + 2..c.len()).find(|&j| c[j] == m && !c[j - 1].is_whitespace() && (m == '*' || c.get(j + 1).is_none_or(|x| !x.is_alphanumeric())));
            if let Some(j) = close {
                c.remove(j);
                c.remove(i);
                continue;
            }
        }
        i += 1;
    }
    // What is left: runs of two or more asterisks or tildes are empty or stray marks (`****`), not text.
    let mut cleaned: Vec<char> = Vec::with_capacity(c.len());
    let mut i = 0;
    while i < c.len() {
        let run = c[i..].iter().take_while(|x| **x == c[i]).count();
        if (c[i] == '*' || c[i] == '~') && run >= 2 {
            i += run;
        } else {
            cleaned.push(c[i]);
            i += 1;
        }
    }
    cleaned.into_iter().collect()
}

/// Normal: the selected lines (or the line the cursor is on) go back to plain text.
fn clear_formatting(chars: &mut Vec<char>, a: usize, b: usize) -> (usize, usize) {
    let start = chars[..a].iter().rposition(|c| *c == '\n').map_or(0, |p| p + 1);
    let end = chars[b..].iter().position(|c| *c == '\n').map_or(chars.len(), |p| b + p);
    let block: String = chars[start..end].iter().collect();
    let cleaned: Vec<String> = block.split('\n').map(|l| strip_inline_marks(&strip_line_prefix(l).1)).collect();
    let joined = cleaned.join("\n");
    let len = joined.chars().count();
    chars.splice(start..end, joined.chars());
    (start, start + len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(md: &str) -> Vec<Block> {
        parse(md)
    }

    #[test]
    fn parses_inline_formats() {
        let b = t("a **b** *c* ~~d~~ `e`");
        let Block::Para(inl) = &b[0] else { panic!("{b:?}") };
        let has = |pred: &dyn Fn(&Fmt) -> bool| inl.iter().any(|i| matches!(i, Inline::Text(_, f) if pred(f)));
        assert!(has(&|f| f.bold));
        assert!(has(&|f| f.italic));
        assert!(has(&|f| f.strike));
        assert!(has(&|f| f.code));
    }

    #[test]
    fn parses_blocks() {
        let b = t("# Title\n\n- one\n- two\n\n> quote\n\n1. a\n2. b\n\n```\ncode\n```\n\n---");
        assert!(matches!(b[0], Block::Heading(1, _)));
        assert!(matches!(&b[1], Block::List { start: None, items } if items.len() == 2));
        assert!(matches!(b[2], Block::Quote(_)));
        assert!(matches!(&b[3], Block::List { start: Some(1), items } if items.len() == 2));
        assert!(matches!(&b[4], Block::Code(c) if c == "code"));
        assert!(matches!(b[5], Block::Rule));
    }

    #[test]
    fn parses_images_and_ids() {
        let md = "hello\n\n![cat](img:i7)\n\n![x](https://example.com/a.png)";
        assert_eq!(image_ids(md), vec!["i7".to_string()]);
        assert!(t(md).iter().any(|b| matches!(b, Block::Para(i) if i.iter().any(|x| matches!(x, Inline::Image { src, .. } if src == "img:i7")))));
    }

    fn applied(text: &str, sel: (usize, usize), f: Format) -> String {
        let mut s = text.to_string();
        apply(&mut s, sel, f);
        s
    }

    #[test]
    fn normal_takes_headings_lists_quotes_and_emphasis_off_the_selected_lines() {
        let all = |t: &str| (0, t.chars().count());
        let t = "## **Hello** and *there* ~~gone~~ `code` #tag";
        assert_eq!(applied(t, all(t), Format::Clear), "Hello and there gone code #tag");
        let list = "- one\n- **two**\n1. three\n> four";
        assert_eq!(applied(list, all(list), Format::Clear), "one\ntwo\nthree\nfour");
        assert_eq!(applied("***both*** and _under_ and snake_case_name", (0, 3), Format::Clear), "both and under and snake_case_name");
        // A picture line, a link and unmatched marks are left alone.
        assert_eq!(applied("![a *b*](img:i1)", (0, 2), Format::Clear), "![a *b*](img:i1)");
        assert_eq!(applied("see [the **site**](http://x) 2 * 3 * 4", (0, 2), Format::Clear), "see [the site](http://x) 2 * 3 * 4");
    }

    #[test]
    fn normal_acts_on_the_lines_touched_by_the_selection_or_the_cursor() {
        let text = "**first**\n**second**\n**third**";
        // The cursor (no selection) in the second line changes only that line.
        assert_eq!(applied(text, (12, 12), Format::Clear), "**first**\nsecond\n**third**");
        // A selection from the first line into the second changes both.
        assert_eq!(applied(text, (2, 14), Format::Clear), "first\nsecond\n**third**");
        let mut s = text.to_string();
        let sel = apply(&mut s, (12, 12), Format::Clear);
        assert_eq!(&s[sel.0..sel.1], "second", "the cleaned line is left selected");
    }

    #[test]
    fn normal_also_removes_empty_and_stray_marks() {
        assert_eq!(applied("adadd****", (9, 9), Format::Clear), "adadd");
        assert_eq!(applied("a ~~ b ** c", (0, 0), Format::Clear), "a  b  c");
        assert_eq!(applied("2 * 3 * 4", (0, 0), Format::Clear), "2 * 3 * 4", "a single star can be arithmetic");
    }

    #[test]
    fn the_word_at_the_cursor_is_found_for_bold_and_italic() {
        let c: Vec<char> = "one two, don't stop".chars().collect();
        assert_eq!(word_at(&c, 5), Some((4, 7)));
        assert_eq!(word_at(&c, 4), Some((4, 7)), "at the start of a word");
        assert_eq!(word_at(&c, 7), Some((4, 7)), "just after a word");
        assert_eq!(word_at(&c, 12), Some((9, 14)), "apostrophes stay in the word");
        assert_eq!(word_at(&c, 8), None, "between a comma and a space there is no word");
        assert_eq!(word_at(&[], 0), None);
    }

    #[test]
    fn one_heading_level_toggles_and_replaces_other_levels() {
        assert_eq!(applied("A title", (0, 0), Format::Heading(2)), "## A title");
        assert_eq!(applied("## A title", (0, 0), Format::Heading(2)), "A title");
        assert_eq!(applied("# Old big heading", (0, 0), Format::Heading(2)), "## Old big heading");
        assert_eq!(applied("### Old small heading", (0, 0), Format::Heading(2)), "## Old small heading");
        // A tag is not a heading.
        assert_eq!(applied("#work today", (0, 0), Format::Heading(2)), "## #work today");
    }

    #[test]
    fn summaries() {
        assert_eq!(summary("# Big day\n\nmore"), "Big day");
        assert_eq!(summary("**bold** start"), "bold start");
        assert_eq!(summary("![pic](img:i1)"), "pic");
        assert_eq!(summary("- first\n- second"), "first");
        assert_eq!(summary(""), "");
    }

    #[test]
    fn wraps_and_unwraps() {
        let mut s = "hello world".to_string();
        let sel = apply(&mut s, (6, 11), Format::Bold);
        assert_eq!(s, "hello **world**");
        assert_eq!(sel, (8, 13));
        let sel = apply(&mut s, sel, Format::Bold);
        assert_eq!(s, "hello world");
        assert_eq!(sel, (6, 11));

        let mut e = "ab".to_string();
        assert_eq!(apply(&mut e, (1, 1), Format::Italic), (2, 2));
        assert_eq!(e, "a**b");
    }

    #[test]
    fn prefixes_lines() {
        let mut s = "one\ntwo\nthree".to_string();
        apply(&mut s, (0, 7), Format::Bullet);
        assert_eq!(s, "- one\n- two\nthree");
        apply(&mut s, (0, 11), Format::Bullet);
        assert_eq!(s, "one\ntwo\nthree");

        let mut s = "a\nb".to_string();
        apply(&mut s, (0, 3), Format::Numbered);
        assert_eq!(s, "1. a\n2. b");

        let mut s = "## old".to_string();
        apply(&mut s, (0, 0), Format::Heading(1));
        assert_eq!(s, "# old");
        apply(&mut s, (0, 0), Format::Heading(1));
        assert_eq!(s, "old");

        // Multi-byte characters don't break char/byte handling.
        let mut s = "héllo wörld".to_string();
        apply(&mut s, (6, 11), Format::Italic);
        assert_eq!(s, "héllo *wörld*");
    }
}

/// Show the picture `src` (`img:ID`), scaled to fit; a note is shown if it can't be found.
pub fn show_image(
    ui: &mut egui::Ui,
    media: &mut Media,
    images: &BTreeMap<String, String>,
    src: &str,
    alt: &str,
    text: Color32,
) {
    let tex = src.strip_prefix(IMAGE_SCHEME).and_then(|id| media.texture(ui.ctx(), images, id));
    match tex {
        Some(tex) => {
            let natural = tex.size_vec2();
            let scale = (ui.available_width() / natural.x).min(520.0 / natural.y).min(1.0);
            ui.add_space(2.0);
            ui.image((tex.id(), natural * scale));
            ui.add_space(2.0);
        }
        None => {
            ui.add(Label::new(egui::RichText::new(format!("[missing image: {alt}]")).color(text)));
        }
    }
}

/// Number of words in an entry, ignoring Markdown marks and picture names.
pub fn word_count(md: &str) -> usize {
    let mut words = 0;
    let mut in_image = false;
    for event in Parser::new(md) {
        match event {
            Event::Start(Tag::Image { .. }) => in_image = true,
            Event::End(TagEnd::Image) => in_image = false,
            Event::Text(t) | Event::Code(t) if !in_image => words += t.split_whitespace().count(),
            _ => {}
        }
    }
    words
}

#[cfg(test)]
mod word_tests {
    use super::word_count;

    #[test]
    fn counts_words_not_marks_or_picture_names() {
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("one two  three"), 3);
        assert_eq!(word_count("# Title\n\n**bold** words and `code`\n\n- a\n- b"), 7);
        assert_eq!(word_count("![a long picture name](img:i1)\n\nafter"), 1);
        assert_eq!(word_count("line one\nline two"), 4);
    }
}

/// An entry as plain text: no marks, no picture names, one space between blocks.
pub fn plain_text(md: &str) -> String {
    let mut out = String::new();
    let mut in_image = false;
    for event in Parser::new(md) {
        match event {
            Event::Start(Tag::Image { .. }) => in_image = true,
            Event::End(TagEnd::Image) => in_image = false,
            Event::Text(t) | Event::Code(t) if !in_image => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            Event::End(_) => out.push(' '),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Split an entry into its title (a first-line `# Heading`) and the rest of the text.
pub fn split_title(md: &str) -> (Option<String>, String) {
    if let Some(rest) = md.strip_prefix("# ") {
        let (first, tail) = rest.split_once('\n').unwrap_or((rest, ""));
        let title = first.trim();
        if !title.is_empty() {
            return (Some(title.to_string()), tail.trim_start_matches('\n').to_string());
        }
    }
    (None, md.to_string())
}

/// The reverse: put a title back in front of the body as a first-line heading.
pub fn join_title(title: &str, body: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        body.to_string()
    } else if body.is_empty() {
        format!("# {title}")
    } else {
        format!("# {title}\n\n{body}")
    }
}

#[cfg(test)]
mod title_tests {
    use super::*;


    #[test]
    fn bold_toggles_off_again_however_the_visible_text_was_selected() {
        // Select "hello" and make it bold.
        let mut t = "hello world".to_string();
        let sel = apply(&mut t, (0, 5), Format::Bold);
        assert_eq!((t.as_str(), sel), ("**hello** world", (2, 7)));
        let hidden = [(0, 2), (7, 9)];
        // The visible word can be selected with its edges on the hidden marks, outside them,
        // or in the middle of them: every way of doing it takes the bold off again.
        for edges in [(2, 7), (0, 9), (1, 8), (0, 7), (2, 9), (9, 0)] {
            let mut text = t.clone();
            let tight = tighten_selection(edges, &hidden);
            assert_eq!(tight, (2, 7), "edges {edges:?}");
            let sel = apply(&mut text, tight, Format::Bold);
            assert_eq!((text.as_str(), sel), ("hello world", (0, 5)), "edges {edges:?}");
        }
        // A selection that is not at the marks is left as it is, and so is a plain cursor.
        assert_eq!(tighten_selection((10, 14), &hidden), (10, 14));
        assert_eq!(tighten_selection((3, 3), &hidden), (3, 3));
        // Selecting only part of the bold word wraps just that part (nesting is not attempted).
        assert_eq!(tighten_selection((3, 5), &hidden), (3, 5));
    }

    #[test]
    fn italic_on_bold_text_adds_italic_instead_of_stripping_the_bold() {
        let mut t = "**word**".to_string();
        let sel = apply(&mut t, (2, 6), Format::Italic);
        assert_eq!(t, "***word***");
        // And italic comes off again, leaving the bold.
        apply(&mut t, sel, Format::Italic);
        assert_eq!(t, "**word**");
        // Plain italic still toggles.
        let mut t = "a *b* c".to_string();
        apply(&mut t, (3, 4), Format::Italic);
        assert_eq!(t, "a b c");
        // Bold comes off bold-italic, leaving the italic.
        let mut t = "***word***".to_string();
        apply(&mut t, (3, 7), Format::Bold);
        assert_eq!(t, "*word*");
    }
    #[test]
    fn a_first_line_heading_is_the_title() {
        assert_eq!(split_title("# Long walk\n\nThe tide was out."), (Some("Long walk".into()), "The tide was out.".into()));
        assert_eq!(split_title("# Only a title"), (Some("Only a title".into()), String::new()));
        // Not a title: no heading, a deeper heading, an empty heading, or a heading further down.
        for text in ["Just text", "## Second level\n\nbody", "# \nbody", "Intro\n\n# Later heading"] {
            assert_eq!(split_title(text), (None, text.to_string()), "{text:?}");
        }
    }

    #[test]
    fn titles_join_back_on() {
        assert_eq!(join_title("Walk", "Body"), "# Walk\n\nBody");
        assert_eq!(join_title("Walk", ""), "# Walk");
        assert_eq!(join_title("  ", "Body"), "Body");
        assert_eq!(join_title("", ""), "");
        for text in ["# A\n\nB", "plain", "# T"] {
            let (t, b) = split_title(text);
            assert_eq!(join_title(&t.unwrap_or_default(), &b), text);
        }
    }
}

/// Pull the edges of a selection in past any hidden formatting marks, so that selecting the
/// visible text of `**bold**` means the text between the marks however the edges landed (on the
/// marks, or just outside them). `hidden` is the list of mark ranges (see `live::hidden_marks`).
pub fn tighten_selection(sel: (usize, usize), hidden: &[(usize, usize)]) -> (usize, usize) {
    let (mut a, mut b) = (sel.0.min(sel.1), sel.0.max(sel.1));
    if a == b {
        return (a, b);
    }
    if let Some(run) = hidden.iter().find(|h| h.0 <= a && a < h.1) {
        a = run.1;
    }
    if let Some(run) = hidden.iter().find(|h| h.0 < b && b <= h.1) {
        b = run.0;
    }
    if a > b { (sel.0.min(sel.1), sel.0.max(sel.1)) } else { (a, b) }
}
