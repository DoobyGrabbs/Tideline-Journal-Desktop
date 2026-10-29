//! Tideline Journal's building blocks: thin outline icons drawn with the painter (no emoji, no icon
//! font) and the few custom widgets the design uses: buttons, side-menu rows, a segmented switch
//! and tinted category labels.

use crate::theme;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Response, Sense, Shape, Stroke, StrokeKind, Ui, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Plus,
    Sun,
    Calendar,
    List,
    Star,
    Image,
    Chart,
    Lock,
    Sliders,
    Search,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    Shield,
    Pencil,
    Database,
    Help,
    Minimize,
    Maximize,
    Restore,
    Close,
    Sparkle,
    Shuffle,
    Dots,
    Back,
    Clock,
    Paperclip,
    Bullets,
    Numbers,
    Quote,
}

fn sample_arc(c: Pos2, r: f32, from_deg: f32, to_deg: f32) -> Vec<Pos2> {
    (0..=16)
        .map(|i| {
            let a = (from_deg + (to_deg - from_deg) * i as f32 / 16.0).to_radians();
            Pos2::new(c.x + r * a.cos(), c.y - r * a.sin())
        })
        .collect()
}

/// Draw an icon centred on `center`, `size` points across, in a ~1.8 stroke on a 24-unit grid.
pub fn paint(painter: &egui::Painter, center: Pos2, size: f32, color: Color32, icon: Icon, filled: bool) {
    let k = size / 24.0;
    let o = center - vec2(12.0, 12.0) * k;
    let p = |x: f32, y: f32| o + vec2(x, y) * k;
    let stroke = Stroke::new((1.8 * k).max(1.2), color);
    let line = |pts: &[(f32, f32)]| {
        painter.add(Shape::line(pts.iter().map(|&(x, y)| p(x, y)).collect(), stroke));
    };
    let ring = |x: f32, y: f32, r: f32| {
        painter.circle_stroke(p(x, y), r * k, stroke);
    };
    let dot = |x: f32, y: f32, r: f32| {
        painter.circle_filled(p(x, y), r * k, color);
    };
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        painter.rect_stroke(egui::Rect::from_min_max(p(x0, y0), p(x1, y1)), r * k, stroke, StrokeKind::Middle);
    };
    match icon {
        Icon::Plus => {
            line(&[(12.0, 5.0), (12.0, 19.0)]);
            line(&[(5.0, 12.0), (19.0, 12.0)]);
        }
        Icon::Sun => {
            ring(12.0, 12.0, 4.2);
            for i in 0..8 {
                let a = (i as f32 * 45.0).to_radians();
                line(&[(12.0 + 7.4 * a.cos(), 12.0 + 7.4 * a.sin()), (12.0 + 9.6 * a.cos(), 12.0 + 9.6 * a.sin())]);
            }
        }
        Icon::Calendar => {
            rect(4.0, 5.5, 20.0, 20.0, 2.5);
            line(&[(4.0, 10.5), (20.0, 10.5)]);
            line(&[(8.5, 3.0), (8.5, 7.5)]);
            line(&[(15.5, 3.0), (15.5, 7.5)]);
        }
        Icon::List => {
            for y in [7.0, 12.0, 17.0] {
                line(&[(9.0, y), (20.0, y)]);
                dot(4.8, y, 1.2);
            }
        }
        Icon::Star => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let r = if i % 2 == 0 { 9.0 } else { 4.1 };
                    let a = (i as f32 * 36.0 - 90.0).to_radians();
                    p(12.0 + r * a.cos(), 12.6 + r * a.sin())
                })
                .collect();
            if filled {
                painter.add(Shape::convex_polygon(pts, color, Stroke::new(1.0 * k, color)));
            } else {
                painter.add(Shape::closed_line(pts, stroke));
            }
        }
        Icon::Image => {
            rect(3.5, 4.5, 20.5, 19.5, 2.5);
            ring(8.5, 10.0, 1.7);
            line(&[(3.5, 17.5), (9.0, 12.5), (13.0, 16.0), (16.0, 13.5), (20.5, 17.0)]);
        }
        Icon::Chart => {
            line(&[(4.0, 4.0), (4.0, 20.0), (20.0, 20.0)]);
            line(&[(9.0, 16.0), (9.0, 11.0)]);
            line(&[(13.5, 16.0), (13.5, 7.5)]);
            line(&[(18.0, 16.0), (18.0, 13.0)]);
        }
        Icon::Lock => {
            rect(5.0, 11.0, 19.0, 20.5, 2.5);
            let mut arc = vec![(8.0, 11.0), (8.0, 8.0)];
            arc.extend(sample_arc(Pos2::new(12.0, 8.0), 4.0, 180.0, 0.0).into_iter().map(|q| (q.x, q.y)));
            arc.push((16.0, 11.0));
            line(&arc);
        }
        Icon::Sliders => {
            for (y, x) in [(7.0, 15.0), (12.0, 8.5), (17.0, 16.0)] {
                line(&[(4.0, y), (x - 2.6, y)]);
                line(&[(x + 2.6, y), (20.0, y)]);
                ring(x, y, 2.4);
            }
        }
        Icon::Search => {
            ring(11.0, 11.0, 6.2);
            line(&[(15.6, 15.6), (20.0, 20.0)]);
        }
        Icon::ChevronLeft => line(&[(14.5, 6.0), (8.5, 12.0), (14.5, 18.0)]),
        Icon::ChevronRight => line(&[(9.5, 6.0), (15.5, 12.0), (9.5, 18.0)]),
        Icon::ChevronDown => line(&[(6.0, 9.5), (12.0, 15.5), (18.0, 9.5)]),
        Icon::ChevronUp => line(&[(6.0, 14.5), (12.0, 8.5), (18.0, 14.5)]),
        Icon::Shield => {
            line(&[(12.0, 3.0), (19.5, 6.0), (19.5, 12.0), (12.0, 21.0), (4.5, 12.0), (4.5, 6.0), (12.0, 3.0)]);
        }
        Icon::Pencil => {
            line(&[(4.0, 20.0), (5.0, 15.0), (16.0, 4.0), (20.0, 8.0), (9.0, 19.0), (4.0, 20.0)]);
            line(&[(14.0, 6.0), (18.0, 10.0)]);
        }
        Icon::Database => {
            line(&sample_arc(Pos2::new(12.0, 6.0), 8.0, 0.0, 360.0).iter().map(|q| (q.x, 6.0 + (q.y - 6.0) * 0.4)).collect::<Vec<_>>());
            line(&[(4.0, 6.0), (4.0, 18.0)]);
            line(&[(20.0, 6.0), (20.0, 18.0)]);
            line(&sample_arc(Pos2::new(12.0, 18.0), 8.0, 180.0, 360.0).iter().map(|q| (q.x, 18.0 + (q.y - 18.0) * 0.4)).collect::<Vec<_>>());
            line(&sample_arc(Pos2::new(12.0, 12.0), 8.0, 180.0, 360.0).iter().map(|q| (q.x, 12.0 + (q.y - 12.0) * 0.4)).collect::<Vec<_>>());
        }
        Icon::Help => {
            ring(12.0, 12.0, 9.0);
            painter.text(p(12.0, 12.4), Align2::CENTER_CENTER, "?", FontId::new(13.0 * k.max(0.7), crate::fonts::ui_bold()), color);
        }
        Icon::Minimize => line(&[(6.0, 12.0), (18.0, 12.0)]),
        Icon::Maximize => rect(6.0, 6.0, 18.0, 18.0, 1.5),
        Icon::Restore => {
            rect(5.5, 9.0, 15.0, 18.5, 1.5);
            line(&[(9.0, 9.0), (9.0, 5.5), (18.5, 5.5), (18.5, 15.0), (15.0, 15.0)]);
        }
        Icon::Close => {
            line(&[(6.5, 6.5), (17.5, 17.5)]);
            line(&[(17.5, 6.5), (6.5, 17.5)]);
        }
        Icon::Sparkle => {
            let pts: Vec<Pos2> = [(12.0, 3.0), (13.9, 10.1), (21.0, 12.0), (13.9, 13.9), (12.0, 21.0), (10.1, 13.9), (3.0, 12.0), (10.1, 10.1)].iter().map(|&(x, y)| p(x, y)).collect();
            painter.add(Shape::closed_line(pts, stroke));
        }
        Icon::Shuffle => {
            line(&[(3.5, 7.0), (8.0, 7.0), (15.0, 17.0), (20.0, 17.0)]);
            line(&[(3.5, 17.0), (8.0, 17.0), (10.2, 13.9)]);
            line(&[(13.0, 10.2), (15.0, 7.0), (20.0, 7.0)]);
            line(&[(17.0, 4.0), (20.0, 7.0), (17.0, 10.0)]);
            line(&[(17.0, 14.0), (20.0, 17.0), (17.0, 20.0)]);
        }
        Icon::Dots => {
            for x in [6.0, 12.0, 18.0] {
                dot(x, 12.0, 1.5);
            }
        }
        Icon::Back => {
            line(&[(19.0, 12.0), (5.0, 12.0)]);
            line(&[(11.0, 6.0), (5.0, 12.0), (11.0, 18.0)]);
        }
        Icon::Clock => {
            ring(12.0, 12.0, 8.5);
            line(&[(12.0, 7.0), (12.0, 12.0), (15.5, 14.0)]);
        }
        Icon::Paperclip => {
            // Three arcs joined by straight runs (the usual paperclip outline).
            let arc = |cx: f32, cy: f32, r: f32, from: f32, to: f32| -> Vec<(f32, f32)> {
                (0..=14)
                    .map(|i| {
                        let a = (from + (to - from) * i as f32 / 14.0).to_radians();
                        (cx + r * a.cos(), cy + r * a.sin())
                    })
                    .collect()
            };
            let mut pts = vec![(21.4, 11.1), (12.25, 20.24)];
            pts.extend(arc(8.0, 16.0, 6.0, 45.0, 225.0));
            pts.push((12.95, 2.56));
            pts.extend(arc(15.775, 5.38, 4.0, 225.0, 405.0));
            pts.push((9.4, 17.4));
            pts.extend(arc(7.985, 15.985, 2.0, 45.0, 225.0));
            pts.push((15.06, 6.09));
            line(&pts);
        }
        Icon::Bullets => {
            for y in [7.0, 12.0, 17.0] {
                line(&[(10.0, y), (20.0, y)]);
                dot(5.2, y, 1.3);
            }
        }
        Icon::Numbers => {
            for (i, y) in [7.0, 12.0, 17.0].into_iter().enumerate() {
                line(&[(10.0, y), (20.0, y)]);
                painter.text(p(5.0, y), Align2::CENTER_CENTER, (i + 1).to_string(), FontId::proportional(8.0 * k.max(0.7)), color);
            }
        }
        Icon::Quote => {
            painter.text(p(12.0, 14.0), Align2::CENTER_CENTER, "\u{201D}", FontId::new(30.0 * k, crate::fonts::serif_medium()), color);
        }
    }
}

// ---- widgets -----------------------------------------------------------------------------------

const BUTTON_H: f32 = 36.0;

fn text_width(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.painter().layout_no_wrap(text.to_string(), font, Color32::WHITE).size().x
}

fn ui_font(size: f32) -> FontId {
    FontId::new(size, crate::fonts::ui_bold())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Filled with the accent colour.
    Primary,
    /// A dark button with a border.
    Ghost,
    /// A border and text in the accent colour.
    AccentOutline,
}

/// A button with an optional icon. `width` of `None` fits the content.
pub fn button(ui: &mut Ui, look: Look, icon: Option<Icon>, label: &str, width: Option<f32>) -> Response {
    button_h(ui, look, icon, label, width, BUTTON_H)
}

/// A smaller button for card footers.
pub fn small_button(ui: &mut Ui, look: Look, icon: Option<Icon>, label: &str) -> Response {
    button_h(ui, look, icon, label, None, 30.0)
}

/// The general form: `height` is the button's height in points.
pub fn button_h(ui: &mut Ui, look: Look, icon: Option<Icon>, label: &str, width: Option<f32>, height: f32) -> Response {
    button_impl(ui, look, icon, false, label, width, height)
}

/// A small button with the icon after the label (for "Open day ›").
pub fn small_button_after(ui: &mut Ui, look: Look, label: &str, icon: Icon) -> Response {
    button_impl(ui, look, Some(icon), true, label, None, 30.0)
}

fn button_impl(ui: &mut Ui, look: Look, icon: Option<Icon>, icon_after: bool, label: &str, width: Option<f32>, height: f32) -> Response {
    let shades = theme::shades();
    let font = ui_font(if height < 32.0 { 13.0 } else { 14.0 });
    let icon_w = if icon.is_some() { 18.0 + if label.is_empty() { 0.0 } else { 8.0 } } else { 0.0 };
    let text_w = if label.is_empty() { 0.0 } else { text_width(ui, label, font.clone()) };
    let natural = icon_w + text_w + if label.is_empty() { 20.0 } else { 32.0 };
    let w = width.unwrap_or(natural).max(if label.is_empty() { height } else { 0.0 });
    let (rect, resp) = ui.allocate_exact_size(vec2(w, height), Sense::click());
    let (fill, stroke, fg) = match look {
        Look::Primary => (if resp.hovered() { shades.accent_hover } else { shades.accent }, Stroke::NONE, shades.on_accent),
        Look::Ghost => (if resp.hovered() { theme::NAV_ACTIVE } else { theme::PANEL }, Stroke::new(1.0, if resp.hovered() { theme::TEXT_3 } else { theme::BORDER }), theme::TEXT),
        Look::AccentOutline => (if resp.hovered() { shades.accent.gamma_multiply(0.12) } else { Color32::TRANSPARENT }, Stroke::new(1.0, shades.accent), shades.accent),
    };
    let painter = ui.painter();
    painter.rect(rect, 8.0, fill, stroke, StrokeKind::Inside);
    let content_w = icon_w + text_w;
    let mut x = rect.center().x - content_w / 2.0;
    if let Some(icon) = icon.filter(|_| !icon_after) {
        paint(painter, Pos2::new(x + 9.0, rect.center().y), if height < 32.0 { 16.0 } else { 18.0 }, fg, icon, false);
        x += icon_w;
    }
    if !label.is_empty() {
        painter.text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, label, font, fg);
        if let Some(icon) = icon.filter(|_| icon_after) {
            paint(painter, Pos2::new(x + text_w + 15.0, rect.center().y), 15.0, fg, icon, false);
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A small square button holding only an icon.
pub fn icon_button(ui: &mut Ui, icon: Icon, tip: &str) -> Response {
    button(ui, Look::Ghost, Some(icon), "", Some(BUTTON_H)).on_hover_text(tip)
}

/// A row of the side menu. `trailing` is a count shown at the right.
pub fn nav_item(ui: &mut Ui, icon: Option<Icon>, dot: Option<Color32>, label: &str, trailing: Option<String>, active: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::click());
    if active || resp.hovered() {
        ui.painter().rect_filled(rect, 8.0, if active { theme::NAV_ACTIVE } else { theme::NAV_ACTIVE.gamma_multiply(0.55) });
    }
    let fg = if active { theme::TEXT } else { theme::TEXT_2 };
    let cy = rect.center().y;
    if let Some(icon) = icon {
        paint(ui.painter(), Pos2::new(rect.min.x + 26.0, cy), 18.0, fg, icon, false);
    }
    if let Some(col) = dot {
        ui.painter().circle_filled(Pos2::new(rect.min.x + 26.0, cy), 4.5, col);
    }
    ui.painter().text(Pos2::new(rect.min.x + 52.0, cy), Align2::LEFT_CENTER, label, FontId::new(15.0, crate::fonts::ui_bold()), fg);
    if let Some(t) = trailing {
        ui.painter().text(Pos2::new(rect.max.x - 14.0, cy), Align2::RIGHT_CENTER, t, FontId::proportional(13.0), theme::TEXT_3);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A segmented switch. Returns the index clicked, if any.
pub fn segmented(ui: &mut Ui, labels: &[&str], current: Option<usize>) -> Option<usize> {
    let font = ui_font(14.0);
    let widths: Vec<f32> = labels.iter().map(|l| text_width(ui, l, font.clone()) + 32.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 8.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, 44.0), Sense::hover());
    ui.painter().rect(rect, 10.0, theme::PANEL, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    let mut x = rect.min.x + 4.0;
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let r = egui::Rect::from_min_size(Pos2::new(x, rect.min.y + 4.0), vec2(widths[i], 36.0));
        let resp = ui.interact(r, ui.id().with(("seg", i, *label)), Sense::click());
        let on = current == Some(i);
        if on {
            ui.painter().rect_filled(r, 7.0, theme::NAV_ACTIVE);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 7.0, theme::NAV_ACTIVE.gamma_multiply(0.5));
        }
        ui.painter().text(r.center(), Align2::CENTER_CENTER, label, font.clone(), if on { theme::TEXT } else { theme::TEXT_2 });
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            clicked = Some(i);
        }
        x += widths[i];
    }
    clicked
}

/// The colour at about 12% behind text in the full colour: a tinted label.
pub fn tint(col: Color32) -> Color32 {
    let bg = theme::rgb_of(theme::PANEL);
    let m = theme::mix(theme::rgb_of(col), bg, 0.14);
    Color32::from_rgb(m[0], m[1], m[2])
}

/// A pill-shaped category label: dot + name, tinted. Returns its response.
pub fn category_pill(ui: &mut Ui, col: Color32, name: &str) -> Response {
    let font = FontId::new(13.0, crate::fonts::ui_bold());
    let w = text_width(ui, name, font.clone()) + 34.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
    ui.painter().rect_filled(rect, 13.0, tint(col));
    ui.painter().circle_filled(Pos2::new(rect.min.x + 13.0, rect.center().y), 3.5, col);
    ui.painter().text(Pos2::new(rect.min.x + 24.0, rect.center().y), Align2::LEFT_CENTER, name, font, col);
    resp
}

/// A small uppercase section label (text_3, letter-spaced).
pub fn section_label(ui: &mut Ui, text: &str) {
    let spaced: String = text.to_uppercase().chars().flat_map(|c| [c, '\u{200A}']).collect();
    ui.label(egui::RichText::new(spaced).font(FontId::new(11.5, crate::fonts::ui_bold())).color(theme::TEXT_3));
}

/// The Tideline Journal mark: a ring, a wave across its lower half and a small moon above it.
pub fn paint_logo(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    let k = size / 24.0;
    let o = center - vec2(12.0, 12.0) * k;
    let p = |x: f32, y: f32| o + vec2(x, y) * k;
    let stroke = Stroke::new((1.7 * k).max(1.2), color);
    painter.circle_stroke(p(12.0, 12.0), 9.0 * k, stroke);
    let mut wave = Vec::new();
    // Three humps: y = 14.5 + a gentle sine from x = 5.5 to 18.5.
    for i in 0..=40 {
        let x = 5.5 + 13.0 * i as f32 / 40.0;
        let y = 14.5 + 1.4 * ((x - 5.5) / 13.0 * std::f32::consts::TAU * 1.5).sin();
        wave.push(p(x, y));
    }
    painter.add(Shape::line(wave, stroke));
    painter.circle_filled(p(12.0, 8.2), 1.6 * k, color);
}

// ---- mood faces, chips and the date picker -------------------------------------------------------

/// An outline face for mood 1 (Awful) to 5 (Great), in a ~1.8 stroke on a 24-unit grid.
pub fn paint_face(painter: &egui::Painter, center: Pos2, size: f32, color: Color32, mood: u8) {
    let k = size / 24.0;
    let o = center - vec2(12.0, 12.0) * k;
    let p = |x: f32, y: f32| o + vec2(x, y) * k;
    let stroke = Stroke::new((1.7 * k).max(1.2), color);
    painter.circle_stroke(p(12.0, 12.0), 9.0 * k, stroke);
    painter.circle_filled(p(9.0, 10.0), 1.15 * k, color);
    painter.circle_filled(p(15.0, 10.0), 1.15 * k, color);
    let line = |pts: Vec<Pos2>| {
        painter.add(Shape::line(pts.into_iter().map(|q| p(q.x, q.y)).collect(), stroke));
    };
    match mood.clamp(1, 5) {
        1 => line(sample_arc(Pos2::new(12.0, 20.0), 5.0, 40.0, 140.0)),
        2 => line(sample_arc(Pos2::new(12.0, 22.0), 7.0, 60.0, 120.0)),
        3 => line(vec![Pos2::new(9.0, 15.5), Pos2::new(15.0, 15.5)]),
        4 => line(sample_arc(Pos2::new(12.0, 10.0), 6.0, 235.0, 305.0)),
        _ => {
            let mut pts = sample_arc(Pos2::new(12.0, 13.0), 4.8, 190.0, 350.0);
            pts.push(pts[0]);
            painter.add(Shape::line(pts.into_iter().map(|q| p(q.x, q.y)).collect(), stroke));
        }
    }
}

/// A rounded pill with an optional icon or colour dot, a label and an optional dropdown arrow.
/// `tint` colours the pill (a category). `on` marks a switched-on toggle.
pub fn chip(ui: &mut Ui, icon: Option<Icon>, dot: Option<Color32>, label: &str, tint_col: Option<Color32>, on: bool, chevron: bool) -> Response {
    let font = FontId::new(14.0, crate::fonts::ui_bold());
    let accent = theme::shades().accent;
    let lead = if icon.is_some() || dot.is_some() { 24.0 } else { 0.0 };
    let w = 28.0 + lead + text_width(ui, label, font.clone()) + if chevron { 22.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
    let (fill, border, fg) = match tint_col {
        Some(col) => (tint(col), col.gamma_multiply(0.45), col),
        None if on => (theme::shades().accent.gamma_multiply(0.14), accent, accent),
        None => (if resp.hovered() { theme::NAV_ACTIVE } else { theme::PANEL }, if resp.hovered() { theme::TEXT_3 } else { theme::BORDER }, theme::TEXT),
    };
    ui.painter().rect(rect, 20.0, fill, Stroke::new(1.0, border), StrokeKind::Inside);
    let cy = rect.center().y;
    let mut x = rect.min.x + 16.0;
    if let Some(icon) = icon {
        paint(ui.painter(), Pos2::new(x + 8.0, cy), 18.0, if tint_col.is_some() { fg } else { theme::TEXT_2 }, icon, on);
        x += 26.0;
    } else if let Some(col) = dot {
        ui.painter().circle_filled(Pos2::new(x + 5.0, cy), 4.5, col);
        x += 18.0;
    }
    ui.painter().text(Pos2::new(x, cy), Align2::LEFT_CENTER, label, font, fg);
    if chevron {
        paint(ui.painter(), Pos2::new(rect.max.x - 18.0, cy), 14.0, fg, Icon::ChevronDown, false);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A mood pill: face icon and word. Selected is tinted in the mood's colour.
pub fn mood_pill(ui: &mut Ui, mood: u8, selected: bool) -> Response {
    let col = theme::MOODS[usize::from(mood.clamp(1, 5)) - 1];
    let label = theme::MOOD_NAMES[usize::from(mood.clamp(1, 5)) - 1];
    let font = FontId::new(14.0, crate::fonts::ui_bold());
    let w = 22.0 + 24.0 + text_width(ui, label, font.clone()) + 10.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
    let (fill, border) = if selected {
        (tint(col), col)
    } else if resp.hovered() {
        (theme::NAV_ACTIVE, theme::TEXT_3)
    } else {
        (theme::PANEL, theme::BORDER)
    };
    ui.painter().rect(rect, 20.0, fill, Stroke::new(1.0, border), StrokeKind::Inside);
    paint_face(ui.painter(), Pos2::new(rect.min.x + 26.0, rect.center().y), 20.0, col, mood);
    ui.painter().text(Pos2::new(rect.min.x + 44.0, rect.center().y), Align2::LEFT_CENTER, label, font, if selected { theme::TEXT } else { theme::TEXT_2 });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A month calendar to pick a day. Returns true when a day was chosen.
pub fn date_picker(ui: &mut Ui, date: &mut chrono::NaiveDate) -> bool {
    use chrono::{Datelike, Days, NaiveDate};
    let id = ui.id().with("date_picker_month");
    let first_of = |d: NaiveDate| d.with_day(1).unwrap_or(d);
    let mut shown: NaiveDate = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| first_of(*date));
    let mut chosen = false;
    ui.set_width(264.0);
    ui.horizontal(|ui| {
        if icon_button(ui, Icon::ChevronLeft, "Previous month").clicked() {
            shown = first_of(shown - Days::new(1));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, Icon::ChevronRight, "Next month").clicked() {
                shown = first_of(shown + Days::new(32));
            }
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new(shown.format("%B %Y").to_string()).font(FontId::new(19.0, crate::fonts::serif_medium())).color(theme::TEXT));
            });
        });
    });
    ui.add_space(4.0);
    let today = chrono::Local::now().date_naive();
    let start = {
        let lead = u64::from(shown.weekday().num_days_from_monday());
        shown - Days::new(lead)
    };
    let cell = vec2(36.0, 32.0);
    let (grid, _) = ui.allocate_exact_size(vec2(cell.x * 7.0, cell.y * 7.0), Sense::hover());
    for (i, name) in ["M", "T", "W", "T", "F", "S", "S"].iter().enumerate() {
        ui.painter().text(grid.min + vec2(cell.x * (i as f32 + 0.5), cell.y * 0.5), Align2::CENTER_CENTER, *name, FontId::proportional(12.0), theme::TEXT_3);
    }
    for n in 0..42u64 {
        let day = start + Days::new(n);
        let (col, row) = ((n % 7) as f32, (n / 7) as f32 + 1.0);
        let r = egui::Rect::from_min_size(grid.min + vec2(cell.x * col, cell.y * row), cell).shrink(1.0);
        let resp = ui.interact(r, ui.id().with(("pick", day)), Sense::click());
        let selected = day == *date;
        if selected {
            ui.painter().rect_filled(r, 8.0, theme::shades().accent);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 8.0, theme::NAV_ACTIVE);
        }
        let color = if selected {
            theme::shades().on_accent
        } else if day == today {
            theme::TODAY
        } else if day.month() == shown.month() {
            theme::TEXT
        } else {
            theme::TEXT_3.gamma_multiply(0.7)
        };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, day.day().to_string(), FontId::new(13.5, crate::fonts::ui_bold()), color);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            *date = day;
            chosen = true;
        }
    }
    ui.data_mut(|d| d.insert_temp(id, shown));
    chosen
}

/// What a toolbar button shows.
#[derive(Clone, Copy)]
pub enum Tool {
    Bold,
    Italic,
    Heading(u8),
    /// Normal: take the formatting off ("Tx").
    Normal,
    Icon(Icon),
}

/// A formatting-toolbar button: 36 x 34, no border until hovered.
pub fn tool_button(ui: &mut Ui, tool: Tool, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 34.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 8.0, theme::NAV_ACTIVE);
    }
    let fg = if resp.hovered() { theme::TEXT } else { theme::TEXT_2 };
    let c = rect.center();
    let painter = ui.painter();
    match tool {
        Tool::Bold => {
            painter.text(c, Align2::CENTER_CENTER, "B", FontId::new(16.0, crate::fonts::ui_bold()), fg);
        }
        Tool::Italic => {
            painter.text(c, Align2::CENTER_CENTER, "I", FontId::new(18.0, crate::fonts::serif_italic()), fg);
        }
        Tool::Heading(_) => {
            painter.text(c, Align2::CENTER_CENTER, "H", FontId::new(16.0, crate::fonts::ui_bold()), fg);
        }
        Tool::Normal => {
            painter.text(c + vec2(-1.0, 0.0), Align2::CENTER_CENTER, "T", FontId::new(16.0, crate::fonts::ui_bold()), fg);
            painter.text(c + vec2(7.0, 4.0), Align2::CENTER_CENTER, "x", FontId::new(11.0, crate::fonts::ui_bold()), fg);
        }
        Tool::Icon(icon) => paint(painter, c, 19.0, fg, icon, false),
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(tip)
}

/// A thin vertical divider between toolbar groups.
pub fn tool_divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 22.0), Sense::hover());
    ui.painter().line_segment([rect.center_top(), rect.center_bottom()], Stroke::new(1.0, theme::BORDER));
}

/// A checkbox that is visible on the dark background: a rounded square, filled with the accent
/// colour when ticked.
pub fn checkbox(ui: &mut Ui, checked: &mut bool, label: &str) -> Response {
    let font = FontId::proportional(13.5);
    let w = 26.0 + text_width(ui, label, font.clone());
    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    if resp.clicked() {
        *checked = !*checked;
        resp.mark_changed();
    }
    let boxr = egui::Rect::from_min_size(Pos2::new(rect.min.x, rect.center().y - 8.0), vec2(16.0, 16.0));
    let accent = theme::shades().accent;
    if *checked {
        ui.painter().rect_filled(boxr, 4.0, accent);
        let stroke = Stroke::new(1.8, theme::shades().on_accent);
        ui.painter().add(Shape::line(vec![boxr.min + vec2(3.5, 8.2), boxr.min + vec2(6.7, 11.2), boxr.min + vec2(12.5, 4.8)], stroke));
    } else {
        ui.painter().rect(boxr, 4.0, Color32::TRANSPARENT, Stroke::new(1.3, if resp.hovered() { theme::TEXT_2 } else { theme::TEXT_3 }), StrokeKind::Inside);
    }
    ui.painter().text(Pos2::new(rect.min.x + 25.0, rect.center().y), Align2::LEFT_CENTER, label, font, theme::TEXT_2);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A quiet text link in the accent colour with a small chevron after it (for "Show more").
pub fn text_link(ui: &mut Ui, label: &str, icon: Icon) -> Response {
    let font = FontId::new(13.5, crate::fonts::ui_bold());
    let w = text_width(ui, label, font.clone()) + 24.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    let col = if resp.hovered() { theme::shades().accent_hover } else { theme::shades().accent };
    ui.painter().text(Pos2::new(rect.min.x, rect.center().y), Align2::LEFT_CENTER, label, font.clone(), col);
    let tw = text_width(ui, label, font);
    paint(ui.painter(), Pos2::new(rect.min.x + tw + 12.0, rect.center().y), 14.0, col, icon, false);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// An on/off switch (46 x 26). The knob is dark on the accent colour when on.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(48.0, 28.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let accent = theme::shades().accent;
    let track = if *on { accent } else { theme::BORDER };
    ui.painter().rect_filled(rect, 14.0, if resp.hovered() { track.gamma_multiply(1.1) } else { track });
    let x = if *on { rect.max.x - 14.0 } else { rect.min.x + 14.0 };
    ui.painter().circle_filled(Pos2::new(x, rect.center().y), 10.0, if *on { theme::BG } else { theme::TEXT_2 });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A horizontal slider: a visible track filled with the accent colour up to a round knob.
pub fn slider(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, width: f32) -> Response {
    let (lo, hi) = (*range.start(), *range.end());
    let (rect, mut resp) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click_and_drag());
    let pad = 10.0;
    if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        let t = ((pos.x - rect.min.x - pad) / (rect.width() - pad * 2.0)).clamp(0.0, 1.0);
        let new = lo + (hi - lo) * t;
        if (new - *value).abs() > f32::EPSILON {
            *value = new;
            resp.mark_changed();
        }
    }
    let t = ((*value - lo) / (hi - lo)).clamp(0.0, 1.0);
    let accent = theme::shades().accent;
    let track = egui::Rect::from_center_size(rect.center(), vec2(rect.width() - pad * 2.0, 6.0));
    ui.painter().rect_filled(track, 3.0, theme::BORDER);
    let x = track.min.x + track.width() * t;
    ui.painter().rect_filled(egui::Rect::from_min_max(track.min, Pos2::new(x, track.max.y)), 3.0, accent);
    let hot = resp.hovered() || resp.dragged();
    ui.painter().circle_filled(Pos2::new(x, rect.center().y), if hot { 9.0 } else { 8.0 }, theme::TEXT);
    ui.painter().circle_stroke(Pos2::new(x, rect.center().y), if hot { 9.0 } else { 8.0 }, Stroke::new(2.0, accent));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
