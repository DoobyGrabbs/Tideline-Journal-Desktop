//! Demo mode (`TIDELINE_DEMO=1`): starts unlocked with a few made-up entries so the interface can
//! be looked at without a real diary. Everything it saves goes to a throw-away folder: the
//! program points `%APPDATA%` there before it reads or writes anything.

use super::*;

pub(crate) fn requested() -> bool {
    std::env::var_os("TIDELINE_DEMO").is_some()
}

/// Send all the program's files to a temporary folder (must run before anything touches them).
pub(crate) fn redirect_app_data() {
    let dir = std::env::temp_dir().join(format!("tideline-demo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok();
    // Safety: called at the very start of `main`, before any other thread exists.
    unsafe { std::env::set_var("APPDATA", &dir) };
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
    data.settings.privacy_screen = false;
    data.settings.auto_lock_minutes = 0;
    data
}

/// `TIDELINE_DEMO=editor` opens the writing view with some text in it.
pub(crate) fn wants_editor() -> bool {
    std::env::var("TIDELINE_DEMO").is_ok_and(|v| v == "editor")
}

pub(crate) fn fill_editor(app: &mut DiaryApp) {
    app.new_entry(app.selected);
    if let Some(ed) = app.editor.as_mut() {
        ed.title = "A quiet tide".into();
        ed.text = "Walked along the **shore** at dusk. The tide had left a fine line of shells, and the light was low and *gold*.\n\n## What I noticed\n\n- gulls working the edge of the water\n- the smell of salt and wet stone\n- one small boat, going home\n\n> Every day leaves a mark.\n\nI want to remember how still it was.".into();
        ed.mood = Some(4);
        ed.pinned = true;
    }
}
