//! Colour themes. One accent colour drives every tinted part of the interface (the header,
//! buttons, highlights, the heat-map, dark mode), so a theme is just a single colour. All the
//! shades are worked out from it here, and a colour that would make white button text hard to
//! read is automatically darkened, so any choice stays usable.

use eframe::egui::Color32;

pub type Rgb = [u8; 3];

/// The original blue.
pub const DEFAULT: Rgb = [37, 99, 235];

/// Ready-made themes: name and accent colour.
pub const PRESETS: [(&str, Rgb); 8] = [
    ("Ocean", DEFAULT),
    ("Indigo", [79, 70, 229]),
    ("Plum", [147, 51, 234]),
    ("Rose", [225, 29, 72]),
    ("Ember", [234, 88, 12]),
    ("Forest", [22, 163, 74]),
    ("Teal", [13, 148, 136]),
    ("Graphite", [71, 85, 105]),
];

/// Every colour that depends on the theme, for both light and dark mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shades {
    /// Buttons (white text on top) and, in light mode, the header.
    pub button: Color32,
    pub button_hover: Color32,
    /// The "selected" look of a view button.
    pub selected_button: Color32,
    pub light_page: Color32,
    pub light_tint: Color32,
    pub light_tint_text: Color32,
    pub light_weekend: Color32,
    pub light_selection: Color32,
    pub dark_page: Color32,
    pub dark_panel: Color32,
    pub dark_header: Color32,
    pub dark_accent: Color32,
    pub dark_tint_text: Color32,
    pub dark_title: Color32,
    pub dark_weekend: Color32,
    pub dark_selection: Color32,
    /// Year heat-map: nothing written (0) up to a lot (4).
    pub heat_light: [Color32; 5],
    pub heat_dark: [Color32; 5],
}

// ---- colour maths ------------------------------------------------------------------------------

/// Hue (0..360), saturation and lightness (both 0..1).
pub fn to_hsl(c: Rgb) -> (f32, f32, f32) {
    let [r, g, b] = c.map(|v| f32::from(v) / 255.0);
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
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r, g, b].map(|v| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

fn luminance(c: Rgb) -> f32 {
    let lin = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG contrast ratio between two colours (1 to 21).
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn color(c: Rgb) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Keep the colour's hue and saturation but make it dark enough for white text (and not so dark
/// that it vanishes against dark mode).
fn button_colour(accent: Rgb) -> Rgb {
    let (h, s, l) = to_hsl(accent);
    let mut l = l.clamp(0.24, 0.9);
    let mut c = from_hsl(h, s, l);
    while contrast(c, [255, 255, 255]) < 4.5 && l > 0.1 {
        l -= 0.01;
        c = from_hsl(h, s, l);
    }
    c
}

/// Darken a colour (keeping its hue) until it reads on every one of the given backgrounds.
fn readable_on(h: f32, s: f32, mut l: f32, backgrounds: &[Rgb], min: f32) -> Rgb {
    let mut c = from_hsl(h, s, l);
    while backgrounds.iter().any(|bg| contrast(c, *bg) < min) && l > 0.02 {
        l -= 0.01;
        c = from_hsl(h, s, l);
    }
    c
}

/// All the shades for an accent colour.
pub fn derive(accent: Rgb) -> Shades {
    let (h, s, _) = to_hsl(accent);
    let button = button_colour(accent);
    let (_, bs, bl) = to_hsl(button);
    let tint = |s_scale: f32, l: f32| from_hsl(h, (s * s_scale).min(1.0), l);

    let button_hover = from_hsl(h, bs, (bl - 0.06).max(0.1));
    let dark_tint_text = tint(1.0, 0.87);
    let mut dark_l = 0.33;
    while contrast(dark_tint_text, tint(0.77, dark_l)) < 4.6 && dark_l > 0.05 {
        dark_l -= 0.01;
    }
    let dark_header = tint(0.77, dark_l);
    let dark_panel = tint(0.40, 0.175);
    let light_page = tint(1.2, 0.969);
    let light_tint = tint(1.15, 0.927);
    let light_tint_text = readable_on(h, (s * 0.85).min(1.0), 0.41, &[light_page, light_tint], 4.6);
    let mut heat_light = [tint(0.20, 0.92), tint(1.15, 0.87), tint(1.15, 0.78), tint(1.0, 0.60), tint(1.0, 0.48)];
    let mut heat_dark = [dark_panel, dark_header, tint(1.0, 0.54), tint(1.0, 0.68), tint(1.0, 0.87)];
    // Each step must be clearly darker (light mode) / lighter (dark mode) than the one before.
    for i in 1..5 {
        let (mut l, step) = (to_hsl(heat_light[i]).2, 0.01);
        while luminance(heat_light[i]) > luminance(heat_light[i - 1]) * 0.93 && l > 0.02 {
            l -= step;
            heat_light[i] = from_hsl(h, (s * 1.1).min(1.0), l);
        }
        let mut l = to_hsl(heat_dark[i]).2;
        while luminance(heat_dark[i]) < luminance(heat_dark[i - 1]) * 1.07 + 0.003 && l < 0.98 {
            l += step;
            heat_dark[i] = from_hsl(h, (s * 0.9).min(1.0), l);
        }
    }

    Shades {
        button: color(button),
        button_hover: color(button_hover),
        selected_button: color(tint(0.70, 0.21)),
        light_page: color(light_page),
        light_tint: color(light_tint),
        light_tint_text: color(light_tint_text),
        light_weekend: color(tint(1.2, 0.94)),
        light_selection: color(tint(1.15, 0.78)),
        dark_page: color(tint(0.57, 0.112)),
        dark_panel: color(dark_panel),
        dark_header: color(dark_header),
        dark_accent: color(tint(1.1, 0.68)),
        dark_tint_text: color(dark_tint_text),
        dark_title: color(tint(1.0, 0.78)),
        dark_weekend: color(tint(0.57, 0.165)),
        dark_selection: color(tint(0.85, 0.40)),
        heat_light: heat_light.map(color),
        heat_dark: heat_dark.map(color),
    }
}


// ---- the theme in use --------------------------------------------------------------------------

static CURRENT: std::sync::Mutex<(Rgb, Option<Shades>)> = std::sync::Mutex::new((DEFAULT, None));

/// The accent colour in use.
pub fn accent() -> Rgb {
    CURRENT.lock().map(|c| c.0).unwrap_or(DEFAULT)
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
    PRESETS.iter().find(|(_, c)| *c == accent).map(|(n, _)| *n)
}
#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(c: Color32) -> Rgb {
        [c.r(), c.g(), c.b()]
    }

    #[test]
    fn hsl_round_trips() {
        for c in [[37, 99, 235], [0, 0, 0], [255, 255, 255], [200, 30, 30], [10, 200, 90], [128, 128, 128], [255, 240, 0]] {
            let (h, s, l) = to_hsl(c);
            let back = from_hsl(h, s, l);
            for i in 0..3 {
                assert!((i32::from(back[i]) - i32::from(c[i])).abs() <= 1, "{c:?} -> {back:?}");
            }
        }
        assert_eq!(to_hsl([255, 0, 0]).0, 0.0);
        assert!((to_hsl([0, 255, 0]).0 - 120.0).abs() < 0.5 && (to_hsl([0, 0, 255]).0 - 240.0).abs() < 0.5);
    }

    #[test]
    fn contrast_matches_known_values() {
        assert!((contrast([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 0.01);
        assert!((contrast([255, 255, 255], [255, 255, 255]) - 1.0).abs() < 0.001);
        assert!(contrast([37, 99, 235], [255, 255, 255]) > 5.0, "the original blue reads well with white text");
    }

    #[test]
    fn the_default_theme_matches_the_original_blue_look() {
        let s = derive(DEFAULT);
        assert_eq!(rgb(s.button), DEFAULT, "the original blue is kept exactly");
        // The shades that used to be typed in by hand are reproduced closely.
        let near = |got: Color32, want: Rgb| {
            let g = rgb(got);
            (0..3).all(|i| (i32::from(g[i]) - i32::from(want[i])).abs() <= 20)
        };
        assert!(near(s.light_page, [240, 247, 255]), "{:?}", s.light_page);
        assert!(near(s.light_tint, [219, 234, 254]), "{:?}", s.light_tint);
        assert!(near(s.light_tint_text, [30, 64, 175]), "{:?}", s.light_tint_text);
        assert!(near(s.dark_page, [15, 23, 42]), "{:?}", s.dark_page);
        assert!(near(s.dark_panel, [30, 41, 59]), "{:?}", s.dark_panel);
        assert!(near(s.dark_header, [30, 58, 138]), "{:?}", s.dark_header);
        assert!(near(s.dark_accent, [96, 165, 250]), "{:?}", s.dark_accent);
        assert!(near(s.button_hover, [29, 78, 216]), "{:?}", s.button_hover);
    }

    /// The text colours the interface puts on the derived backgrounds.
    fn check_readable(accent: Rgb) {
        let s = derive(accent);
        let ink_light = [30, 41, 59];
        let ink_dark = [241, 245, 249];
        let white = [255, 255, 255];
        assert!(contrast(white, rgb(s.button)) >= 4.5, "{accent:?}: white on the button");
        assert!(contrast(white, rgb(s.button_hover)) >= 4.5, "{accent:?}: white on the hovered button");
        assert!(contrast(rgb(s.light_tint_text), rgb(s.light_tint)) >= 4.5, "{accent:?}: tinted text, light");
        assert!(contrast(rgb(s.light_tint_text), rgb(s.light_page)) >= 4.5, "{accent:?}: titles on the page, light");
        assert!(contrast(ink_light, rgb(s.light_page)) >= 7.0, "{accent:?}: body text, light");
        assert!(contrast(rgb(s.dark_tint_text), rgb(s.dark_header)) >= 4.5, "{accent:?}: tinted text, dark");
        assert!(contrast(rgb(s.dark_title), rgb(s.dark_page)) >= 4.5, "{accent:?}: titles, dark");
        assert!(contrast(rgb(s.dark_accent), rgb(s.dark_page)) >= 3.0, "{accent:?}: highlight outline, dark");
        assert!(contrast(ink_dark, rgb(s.dark_page)) >= 7.0, "{accent:?}: body text, dark");
        assert!(contrast(white, rgb(s.selected_button)) >= 4.5, "{accent:?}: white on a selected view button");
        // Heat-map shades get stronger.
        let lum = |c: Color32| luminance(rgb(c));
        assert!(s.heat_light.windows(2).all(|w| lum(w[0]) > lum(w[1])), "{accent:?}: light heat-map ramp");
        assert!(s.heat_dark.windows(2).all(|w| lum(w[0]) < lum(w[1])), "{accent:?}: dark heat-map ramp");
    }

    #[test]
    fn every_preset_is_readable_and_distinct() {
        for (name, accent) in PRESETS {
            check_readable(accent);
            let _ = name;
        }
        let buttons: std::collections::HashSet<Rgb> = PRESETS.iter().map(|(_, c)| rgb(derive(*c).button)).collect();
        assert_eq!(buttons.len(), PRESETS.len(), "the presets look different from each other");
        let names: std::collections::HashSet<&str> = PRESETS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), PRESETS.len());
    }

    #[test]
    fn any_custom_colour_stays_readable() {
        // Awkward choices: white, black, yellow, pure cyan, near-greys, and a sweep of hues.
        let mut colours: Vec<Rgb> = vec![[255, 255, 255], [0, 0, 0], [255, 255, 0], [0, 255, 255], [250, 250, 250], [10, 10, 10], [128, 128, 128], [255, 0, 255]];
        for step in 0..72 {
            colours.push(from_hsl(step as f32 * 5.0, 0.9, 0.5));
            colours.push(from_hsl(step as f32 * 5.0, 0.4, 0.75));
            colours.push(from_hsl(step as f32 * 5.0, 1.0, 0.25));
        }
        for c in colours {
            check_readable(c);
        }
    }

    #[test]
    fn light_colours_are_darkened_for_white_text_but_dark_ones_are_kept() {
        let yellow = derive([255, 235, 59]);
        assert!(luminance(rgb(yellow.button)) < 0.35, "{:?}", yellow.button);
        assert_eq!(rgb(derive([20, 60, 160]).button), [20, 60, 160]);
        // Far too dark to see against dark mode: lifted a little.
        assert!(luminance(rgb(derive([0, 0, 0]).button)) > 0.02);
    }
}
