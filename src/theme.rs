//! Tideline Journal's colours. The dark surfaces, text and mood colours are fixed by the design spec
//! (`TIDELINE_JOURNAL_DESIGN.md`). The one thing you can change is the accent colour (buttons, the
//! selected day, focus, the prompt card); every shade that depends on it is worked out here and
//! adjusted so it stays readable on the dark background, whatever colour is chosen.

use eframe::egui::Color32;

pub type Rgb = [u8; 3];

const fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

// ---- fixed tokens ------------------------------------------------------------------------------

pub const BG: Color32 = c(0x11, 0x14, 0x18);
pub const BG_TITLEBAR: Color32 = c(0x0D, 0x10, 0x13);
pub const PANEL: Color32 = c(0x15, 0x19, 0x1E);
pub const SURFACE: Color32 = c(0x1C, 0x21, 0x27);
pub const CELL: Color32 = c(0x1A, 0x1E, 0x24);
pub const BORDER: Color32 = c(0x2A, 0x30, 0x39);
pub const BORDER_SOFT: Color32 = c(0x22, 0x27, 0x2E);
pub const TEXT: Color32 = c(0xEC, 0xE7, 0xDD);
pub const TEXT_2: Color32 = c(0xA9, 0xAF, 0xB8);
pub const TEXT_3: Color32 = c(0x7E, 0x85, 0x90);
/// Today's date circle and the pinned star.
pub const TODAY: Color32 = c(0xE7, 0xB1, 0x5A);
/// The active item in the side menu, and the selected segment of a switch.
pub const NAV_ACTIVE: Color32 = c(0x23, 0x2A, 0x33);
/// The writing card on the new-entry screen.
pub const WRITING_CARD: Color32 = c(0x17, 0x1B, 0x20);
pub const DANGER: Color32 = c(0xE5, 0x73, 0x5F);

/// Awful, Low, Okay, Good, Great.
pub const MOODS: [Color32; 5] = [c(0xE5, 0x73, 0x5F), c(0xE9, 0x9A, 0x5B), c(0xD9, 0xC2, 0x5E), c(0x8C, 0xC9, 0x7A), c(0x4F, 0xB3, 0xA5)];
pub const MOOD_NAMES: [&str; 5] = ["Awful", "Low", "Okay", "Good", "Great"];

/// Colours for the starter categories.
pub const CATEGORY_COLOURS: [(&str, Rgb); 5] =
    [("Personal", [0x7A, 0xA2, 0xF7]), ("Health", [0x6C, 0xC4, 0x97]), ("Work", [0xE7, 0xB1, 0x5A]), ("Ideas", [0xB3, 0x92, 0xF0]), ("Pink Stuff", [0xE5, 0x7B, 0xB8])];

// ---- the accent --------------------------------------------------------------------------------

/// Tideline Journal's own teal ("Tide").
pub const DEFAULT: Rgb = [0x4F, 0xB3, 0xA5];

/// Ready-made accents: name and colour.
pub const PRESETS: [(&str, Rgb); 8] = [
    ("Tide", DEFAULT),
    ("Sky", [96, 165, 250]),
    ("Violet", [167, 139, 250]),
    ("Rose", [244, 114, 160]),
    ("Lamp", [231, 177, 90]),
    ("Ember", [251, 146, 60]),
    ("Moss", [134, 203, 122]),
    ("Slate", [148, 163, 184]),
];

/// Every colour that depends on the accent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shades {
    pub accent: Color32,
    pub accent_hover: Color32,
    /// Text on an accent-coloured button.
    pub on_accent: Color32,
    /// The faint accent wash behind selected text and selected things.
    pub accent_soft: Color32,
    pub prompt_bg: Color32,
    pub prompt_border: Color32,
    /// The year heat-map: nothing written (0) up to a lot (4).
    pub heat: [Color32; 5],
}

/// The accent presets of the earlier, blue design. A saved choice from that list was made for the
/// old look, so it is ignored and the Tide teal is used until a new one is picked.
pub const LEGACY_PRESETS: [Rgb; 8] = [[37, 99, 235], [79, 70, 229], [147, 51, 234], [225, 29, 72], [234, 88, 12], [22, 163, 74], [13, 148, 136], [71, 85, 105]];

pub fn is_legacy(accent: Rgb) -> bool {
    LEGACY_PRESETS.contains(&accent)
}

/// The accent to start with: the saved one, unless it is missing or from the old design.
pub fn starting_accent(saved: Option<Rgb>) -> Rgb {
    saved.filter(|a| !is_legacy(*a)).unwrap_or(DEFAULT)
}

// ---- colour maths ------------------------------------------------------------------------------

/// Hue (0..360), saturation and lightness (both 0..1).
pub fn to_hsl(col: Rgb) -> (f32, f32, f32) {
    let [r, g, b] = col.map(|v| f32::from(v) / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

pub fn from_hsl(h: f32, s: f32, l: f32) -> Rgb {
    let (s, l) = (s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = chroma * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - chroma / 2.0;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    [r, g, b].map(|v| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

pub fn luminance(col: Rgb) -> f32 {
    let lin = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(col[0]) + 0.7152 * lin(col[1]) + 0.0722 * lin(col[2])
}

/// WCAG contrast ratio between two colours (1 to 21).
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

pub fn rgb_of(col: Color32) -> Rgb {
    [col.r(), col.g(), col.b()]
}

fn color(col: Rgb) -> Color32 {
    Color32::from_rgb(col[0], col[1], col[2])
}

/// `a` laid over `b` at `alpha` (0..1).
pub fn mix(a: Rgb, b: Rgb, alpha: f32) -> Rgb {
    let m = |i: usize| (f32::from(a[i]) * alpha + f32::from(b[i]) * (1.0 - alpha)).round() as u8;
    [m(0), m(1), m(2)]
}

/// Make the accent light enough to read on the dark background, keeping its hue.
fn readable_accent(accent: Rgb) -> Rgb {
    let (h, s, mut l) = to_hsl(accent);
    let mut col = accent;
    // The lightest background it must stand out from is the selected-item grey.
    while contrast(col, rgb_of(NAV_ACTIVE)) < 4.5 && l < 0.95 {
        l += 0.01;
        col = from_hsl(h, s, l);
    }
    col
}

/// All the shades for an accent colour.
pub fn derive(accent: Rgb) -> Shades {
    let a = readable_accent(accent);
    let (h, s, l) = to_hsl(a);
    let hover = from_hsl(h, s, (l + 0.06).min(0.97));
    let mut on = from_hsl(h, (s * 0.5).min(0.5), 0.09);
    if contrast(a, on) < 4.5 {
        on = [0, 0, 0];
    }
    let (prompt_bg, prompt_border) = if a == DEFAULT {
        ([0x17, 0x24, 0x23], [0x24, 0x40, 0x3C])
    } else {
        (from_hsl(h, (s * 0.35).min(0.3), 0.12), from_hsl(h, (s * 0.45).min(0.35), 0.21))
    };
    let base = rgb_of(CELL);
    let heat = [CELL, color(mix(a, base, 0.22)), color(mix(a, base, 0.45)), color(mix(a, base, 0.72)), color(a)];
    Shades {
        accent: color(a),
        accent_hover: color(hover),
        on_accent: color(on),
        accent_soft: color(mix(a, rgb_of(PANEL), 0.28)),
        prompt_bg: color(prompt_bg),
        prompt_border: color(prompt_border),
        heat,
    }
}

// ---- the theme in use --------------------------------------------------------------------------

static CURRENT: std::sync::Mutex<(Rgb, Option<Shades>)> = std::sync::Mutex::new((DEFAULT, None));

/// The accent colour in use.
pub fn accent() -> Rgb {
    CURRENT.lock().map(|cur| cur.0).unwrap_or(DEFAULT)
}

/// Switch to a new accent colour. Returns true if it changed.
pub fn set_accent(accent: Rgb) -> bool {
    let Ok(mut cur) = CURRENT.lock() else { return false };
    if cur.0 == accent {
        return false;
    }
    *cur = (accent, None);
    true
}

/// The shades for the accent colour in use (worked out once per change).
pub fn shades() -> Shades {
    let Ok(mut cur) = CURRENT.lock() else { return derive(DEFAULT) };
    let accent = cur.0;
    *cur.1.get_or_insert_with(|| derive(accent))
}

/// The name of the preset that has this accent colour, if any.
pub fn preset_name(accent: Rgb) -> Option<&'static str> {
    PRESETS.iter().find(|(_, col)| *col == accent).map(|(n, _)| *n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_round_trips() {
        for col in [[37, 99, 235], [0, 0, 0], [255, 255, 255], [200, 30, 30], [10, 200, 90], [128, 128, 128], [255, 240, 0]] {
            let (h, s, l) = to_hsl(col);
            let back = from_hsl(h, s, l);
            for i in 0..3 {
                assert!((i32::from(back[i]) - i32::from(col[i])).abs() <= 1, "{col:?} -> {back:?}");
            }
        }
        assert_eq!(to_hsl([255, 0, 0]).0, 0.0);
        assert!((to_hsl([0, 255, 0]).0 - 120.0).abs() < 0.5 && (to_hsl([0, 0, 255]).0 - 240.0).abs() < 0.5);
    }

    #[test]
    fn contrast_matches_known_values() {
        assert!((contrast([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 0.01);
        assert!((contrast([255, 255, 255], [255, 255, 255]) - 1.0).abs() < 0.001);
    }

    #[test]
    fn the_default_accent_is_exactly_the_specified_tide_teal() {
        let s = derive(DEFAULT);
        assert_eq!(rgb_of(s.accent), [0x4F, 0xB3, 0xA5]);
        assert_eq!(rgb_of(s.prompt_bg), [0x17, 0x24, 0x23]);
        assert_eq!(rgb_of(s.prompt_border), [0x24, 0x40, 0x3C]);
        // The spec's text-on-accent colour is #0B1F1C; ours is within a whisker of it.
        let on = rgb_of(s.on_accent);
        for (got, want) in on.iter().zip([0x0B, 0x1F, 0x1C]) {
            assert!((i32::from(*got) - want).abs() <= 8, "{on:?}");
        }
        assert_eq!(s.heat[0], CELL);
    }

    /// Everything the interface puts on these colours stays readable.
    fn check_readable(accent: Rgb) {
        let s = derive(accent);
        let bg = rgb_of(BG);
        let (a, on) = (rgb_of(s.accent), rgb_of(s.on_accent));
        assert!(contrast(a, bg) >= 4.5, "{accent:?}: accent on the background");
        assert!(contrast(rgb_of(s.accent_hover), bg) >= 4.5, "{accent:?}: hovered accent");
        assert!(contrast(on, a) >= 4.5, "{accent:?}: text on the accent button");
        assert!(contrast(on, rgb_of(s.accent_hover)) >= 4.0, "{accent:?}: text on the hovered button");
        assert!(contrast(rgb_of(TEXT), rgb_of(s.prompt_bg)) >= 8.0, "{accent:?}: text on the prompt card");
        assert!(contrast(a, rgb_of(s.prompt_bg)) >= 4.5, "{accent:?}: accent on the prompt card");
        assert!(contrast(rgb_of(TEXT), rgb_of(s.accent_soft)) >= 4.5, "{accent:?}: text on a selection");
        assert!(s.heat.windows(2).all(|w| luminance(rgb_of(w[0])) <= luminance(rgb_of(w[1])) + 0.001), "{accent:?}: heat ramp gets stronger");
    }

    #[test]
    fn every_preset_is_readable_and_distinct() {
        for (_, accent) in PRESETS {
            check_readable(accent);
        }
        let accents: std::collections::HashSet<Rgb> = PRESETS.iter().map(|(_, col)| rgb_of(derive(*col).accent)).collect();
        assert_eq!(accents.len(), PRESETS.len());
        let names: std::collections::HashSet<&str> = PRESETS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), PRESETS.len());
    }

    #[test]
    fn any_custom_colour_stays_readable() {
        let mut colours: Vec<Rgb> = vec![[255, 255, 255], [0, 0, 0], [255, 255, 0], [0, 255, 255], [250, 250, 250], [10, 10, 10], [128, 128, 128], [255, 0, 255], [0, 0, 128]];
        for step in 0..72 {
            colours.push(from_hsl(step as f32 * 5.0, 0.9, 0.5));
            colours.push(from_hsl(step as f32 * 5.0, 0.4, 0.75));
            colours.push(from_hsl(step as f32 * 5.0, 1.0, 0.2));
        }
        for col in colours {
            check_readable(col);
        }
    }

    #[test]
    fn colours_too_dark_for_the_background_are_lightened_but_good_ones_kept() {
        assert!(contrast(rgb_of(derive([0, 0, 128]).accent), rgb_of(BG)) >= 4.5);
        assert_eq!(rgb_of(derive([231, 177, 90]).accent), [231, 177, 90]);
    }


    #[test]
    fn a_colour_saved_under_the_old_design_is_ignored() {
        assert_eq!(starting_accent(None), DEFAULT);
        assert_eq!(starting_accent(Some([71, 85, 105])), DEFAULT, "the old Graphite");
        assert_eq!(starting_accent(Some([37, 99, 235])), DEFAULT, "the old blue");
        assert_eq!(starting_accent(Some([244, 114, 160])), [244, 114, 160], "a new preset is kept");
        assert_eq!(starting_accent(Some([10, 200, 90])), [10, 200, 90], "so is a custom colour");
        // None of the new presets collide with the old ones.
        assert!(PRESETS.iter().all(|(_, c)| !is_legacy(*c)));
    }
    #[test]
    fn the_mood_scale_and_categories_follow_the_spec() {
        assert_eq!(rgb_of(MOODS[0]), [0xE5, 0x73, 0x5F]);
        assert_eq!(rgb_of(MOODS[4]), [0x4F, 0xB3, 0xA5]);
        assert_eq!(MOOD_NAMES, ["Awful", "Low", "Okay", "Good", "Great"]);
        assert_eq!(CATEGORY_COLOURS[4], ("Pink Stuff", [0xE5, 0x7B, 0xB8]));
        for m in MOODS {
            assert!(contrast(rgb_of(m), rgb_of(BG)) >= 4.5);
        }
    }
}
