//! Demo mode (`TIDELINE_DEMO=1`): starts unlocked with a few made-up entries so the interface can
//! be looked at without a real diary. Everything it saves goes to a throw-away folder: the
//! program points its data folder there before it reads or writes anything.

use super::*;

pub(crate) fn requested() -> bool {
    std::env::var_os("TIDELINE_DEMO").is_some()
}

/// Send all the program's files to a temporary folder (must run before anything touches them).
pub(crate) fn redirect_data_dir() {
    // A folder named in the environment wins (so a picture can show a tidy-looking path).
    if std::env::var_os("TIDELINE_DATA_DIR").is_some() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("tideline-demo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok();
    // Safety: called at the very start of `main`, before any other thread exists.
    unsafe { std::env::set_var("TIDELINE_DATA_DIR", &dir) };
}

pub(crate) fn data() -> Data {
    let mut data = fresh_data();
    let rows: [(&str, &str, &str, &str, Option<u8>, bool); 9] = [
        ("2026-10-02", "09:10:00", "Work", "# Planning the week\n\nSketched out the next three weeks on the whiteboard and cut the list in half. Much calmer.", Some(4), false),
        ("2026-10-03", "21:05:00", "Personal", "# Long walk by the water\n\nThe tide was out and the light was low. Stayed until the streetlamps came on.", Some(5), false),
        ("2026-10-06", "15:40:00", "Personal", "# Odd little afternoon\n\nNothing happened, and I didn't mind.", Some(3), false),
        ("2026-10-07", "18:25:00", "Health", "# Slept badly\n\nUp at three again. Trying no screens after nine.", Some(2), false),
        ("2026-10-07", "20:40:00", "Ideas", "# Half a thought\n\nA small book of tide tables with a line of writing for every day.", Some(4), false),
        (
            "2026-10-08",
            "16:14:00",
            "Health",
            "# Feeling a bit achy today\n\nAchy knees, achy mood. Tea, blanket, early night.",
            Some(3),
            false,
        ),
        (
            "2026-10-08",
            "16:37:00",
            "Pink Stuff",
            "# On the colour pink\n\nPink sits somewhere between the passion of red and the calm of white, and that in-between quality gives it a surprisingly wide emotional range. It can be soft and tender, loud and brave, or quietly nostalgic, like the inside of a shell or the sky just after the sun has gone.",
            None,
            true,
        ),
        ("2026-10-12", "13:59:00", "Personal", "# Wrote this at lunch\n\nThe house is quiet and the kettle is on.", Some(4), false),
        ("2026-09-12", "08:30:00", "Personal", "# A year ago\n\nStarted keeping this diary.", Some(4), false),
    ];
    for (i, (date, time, category, text, mood, pinned)) in rows.into_iter().enumerate() {
        data.entries.push(Entry {
            id: i as u64 + 1,
            date: date.into(),
            added_at: format!("{date} {time}"),
            category: category.into(),
            text: text.into(),
            mood,
            pinned,
            ..Default::default()
        });
    }
    add_picture(&mut data);
    for id in [1, 3, 8] {
        if let Some(e) = data.entries.iter_mut().find(|e| e.id == id) {
            e.pinned = true;
        }
    }
    data.settings.privacy_screen = false;
    data.settings.auto_lock_minutes = 0;
    data
}

/// `TIDELINE_DEMO=editor` opens the writing view with some text in it.


pub(crate) fn fill_editor(app: &mut DiaryApp) {
    app.new_entry(app.selected);
    if let Some(ed) = app.editor.as_mut() {
        ed.title = "A quiet tide".into();
        ed.text = "Walked along the **shore** at dusk. The tide had left a fine line of shells, and the light was low and *gold*.\n\n## What I noticed\n\n- gulls working the edge of the water\n- the smell of salt and wet stone\n- one small boat, going home\n\n> Every day leaves a mark.\n\nI want to remember how still it was.".into();
        ed.mood = Some(4);
        ed.pinned = true;
    }
}

/// A made-up picture (a soft gradient) stored the way real pictures are.
fn demo_picture() -> String {
    use base64::Engine;
    let img = image::RgbaImage::from_fn(480, 280, |x, y| {
        let t = x as f32 / 480.0;
        let u = y as f32 / 280.0;
        image::Rgba([(60.0 + 120.0 * t) as u8, (110.0 + 60.0 * u) as u8, (150.0 + 70.0 * (1.0 - t)) as u8, 255])
    });
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).ok();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub(crate) fn add_picture(data: &mut Data) {
    data.images.insert("demo1".into(), demo_picture());
    if let Some(e) = data.entries.iter_mut().find(|e| e.id == 7) {
        e.text.push_str("\n\n![Pink light over the water](img:demo1)\n\nThe photo is from the same evening.");
    }
}

/// Open the screen or dialog named by `TIDELINE_DEMO` (used to take the pictures for the user
/// guide). `1` is the plain calendar.
pub(crate) fn apply(app: &mut DiaryApp) {
    let mode = std::env::var("TIDELINE_DEMO").unwrap_or_default();
    app.data.trash.push(trash::Trashed {
        entry: Entry { id: 90, date: "2026-10-05".into(), added_at: "2026-10-05 19:10:00".into(), category: "Personal".into(), text: "# A page I deleted\n\nChanged my mind about this one.".into(), ..Default::default() },
        deleted_at: "2026-10-11 09:30:00".into(),
    });
    match mode.as_str() {
        "view-year" => app.view = CalView::Year,
        "view-week" => app.view = CalView::Week,
        "view-day" => app.view = CalView::Day,
        "photos" => app.view = CalView::Photos,
        "insights" | "insights-writing" | "insights-lookback" => {
            app.insights.open = true;
            app.insights.tab = match mode.as_str() {
                "insights-writing" => insights::Tab::Writing,
                "insights-lookback" => insights::Tab::Review,
                _ => insights::Tab::Mood,
            };
        }
        "pinned" => app.show_pinned = true,
        "pinned-expanded" => {
            app.show_pinned = true;
            app.expanded.insert(7);
        }
        "expanded" => {
            app.expanded.insert(7);
        }
        "settings" | "settings-writing" | "settings-data" | "settings-display" => {
            app.show_settings = true;
            let section = match mode.as_str() {
                "settings-writing" => 1,
                "settings-data" => 2,
                "settings-display" => 3,
                _ => 0,
            };
            app.settings_section = section;
            app.settings_jump = Some(section);
        }
        "editor" => fill_editor(app),
        "editor-empty" => app.new_entry(app.selected),
        "search" => {
            app.search = "tide".into();
            app.search_state.open = true;
        }
        "timeline" => app.search_state.open = true,
        "categories" => app.show_categories = true,
        "templates" => app.templates.open = true,
        "export" => app.show_export = true,
        "recovery" => app.show_recovery = true,
        "trash" => app.show_trash = true,
        "location" => app.show_location = true,
        "shortcuts" => app.show_shortcuts = true,
        "ai-setup" => app.ai.setup_open = true,
        "password" => app.show_password = true,
        "backups" => {
            // A few dated copies of the (demo) diary, as automatic backups would have left.
            if let Some(v) = app.vault.as_ref() {
                let file = data_path();
                if write_diary(&file, &app.data, v).is_ok() {
                    let folder = backups::folder_for(&file);
                    std::fs::create_dir_all(&folder).ok();
                    for stamp in ["2026-10-12-081500", "2026-10-11-094500", "2026-10-09-210000", "2026-10-08-073000"] {
                        std::fs::copy(&file, folder.join(format!("entries-{stamp}.json"))).ok();
                    }
                }
            }
            app.show_backups = true;
            app.backups_dirty = true;
        }
        _ => {}
    }
}
