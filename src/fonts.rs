//! Tideline Journal's two typefaces, bundled in the program (nothing is loaded at run time):
//! Hanken Grotesk for the interface and Newsreader (a serif) for what you write and for headings.
//! Both are variable fonts, so each weight is the same file with a different weight setting.

use eframe::egui::{self, FontData, FontFamily, FontTweak};
use std::sync::Arc;

const HANKEN: &[u8] = include_bytes!("../assets/fonts/HankenGrotesk.ttf");
const NEWSREADER: &[u8] = include_bytes!("../assets/fonts/Newsreader.ttf");
const NEWSREADER_ITALIC: &[u8] = include_bytes!("../assets/fonts/Newsreader-Italic.ttf");

/// UI text in medium weight is the default; this is the semibold used for emphasis.
pub fn ui_bold() -> FontFamily {
    FontFamily::Name("ui-bold".into())
}

/// Serif for what you write.
pub fn serif() -> FontFamily {
    FontFamily::Name("serif".into())
}

pub fn serif_italic() -> FontFamily {
    FontFamily::Name("serif-italic".into())
}

/// Serif, a little heavier (bold in writing, and titles).
pub fn serif_medium() -> FontFamily {
    FontFamily::Name("serif-medium".into())
}

/// Serif cut for large sizes: the month and day headings.
pub fn serif_display() -> FontFamily {
    FontFamily::Name("serif-display".into())
}

fn variable(bytes: &'static [u8], axes: &[(&[u8; 4], f32)]) -> Arc<FontData> {
    let coords = egui::epaint::text::VariationCoords::new(axes.iter().map(|(tag, v)| (*tag, *v)));
    Arc::new(FontData::from_static(bytes).tweak(FontTweak { coords, ..Default::default() }))
}

/// Install the fonts. Every family falls back to egui's built-in fonts for symbols and
/// characters the typefaces don't have.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let fallback = fonts.families[&FontFamily::Proportional].clone();

    let faces: [(&str, Arc<FontData>); 7] = [
        ("news-bold", variable(NEWSREADER, &[(b"wght", 650.0), (b"opsz", 16.0)])),
        ("hanken-500", variable(HANKEN, &[(b"wght", 500.0)])),
        ("hanken-600", variable(HANKEN, &[(b"wght", 640.0)])),
        ("news-400", variable(NEWSREADER, &[(b"wght", 400.0), (b"opsz", 16.0)])),
        ("news-500", variable(NEWSREADER, &[(b"wght", 520.0), (b"opsz", 20.0)])),
        ("news-display", variable(NEWSREADER, &[(b"wght", 400.0), (b"opsz", 56.0)])),
        ("news-italic", variable(NEWSREADER_ITALIC, &[(b"wght", 400.0), (b"opsz", 16.0)])),
    ];
    for (name, data) in faces {
        fonts.font_data.insert(name.into(), data);
    }

    let with_fallback = |first: &str| {
        let mut list = vec![first.to_string()];
        list.extend(fallback.clone());
        list
    };
    fonts.families.insert(FontFamily::Proportional, with_fallback("hanken-500"));
    fonts.families.insert(ui_bold(), with_fallback("hanken-600"));
    fonts.families.insert(serif(), with_fallback("news-400"));
    fonts.families.insert(serif_medium(), with_fallback("news-500"));
    fonts.families.insert(serif_display(), with_fallback("news-display"));
    fonts.families.insert(serif_italic(), with_fallback("news-italic"));
    fonts.families.insert(serif_bold(), with_fallback("news-bold"));
    ctx.set_fonts(fonts);
}

/// Serif, bold (for bold text and headings inside an entry).
pub fn serif_bold() -> FontFamily {
    FontFamily::Name("serif-bold".into())
}
