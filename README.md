# Tideline Journal

*Every day leaves a mark.*

A private diary / journal for Windows, written in Rust with [eframe/egui](https://github.com/emilk/egui).

Write down your thoughts each day with rich formatting and pictures, tag them with colour-coded categories, rate your mood, and see at a glance which days have entries. Everything is encrypted with a password.

## Features

**Calendar**
- **Day, Week, Month, Year and Photos views.** In Day, Week and Month each entry is a small chip (category dot, time, title); the Day view is an hour-by-hour timeline. A coloured dot on a day shows its average mood, today is in an amber circle and the selected day has an outline. Days from the neighbouring months are dimmed.
- **Photos view**: every picture in the diary as thumbnails, newest first and grouped by month. Click one to see it large (arrow keys move between pictures) or jump to its entry.
- **Year view**: a heat-map of the days you wrote (shade = words written), plus entries, days written, words, current and longest streak, average mood, entries per category and a chart of your mood through the year.
- **Drag and drop**: drag an entry's pill onto another day to move it (its time is kept). In the Day view, drag it to another hour to change its time. Press Esc to cancel a drag; double-click a pill to edit it.
- **Show more**: an entry card in the day panel shows a short preview; when there is more to see (a longer entry, several paragraphs or a picture) a **Show more** link opens the whole entry in the card, pictures included, and **Show less** folds it again.
- **On this day**: the day panel lists what you wrote on the same date in earlier years; click one to jump to it.
- **Surprise me**: the button (or `Ctrl+R`) jumps to a random entry from the past. Private entries are never picked.
- **Pinned entries**: click the star on an entry (or press `Ctrl+D` while writing) to pin it. Pinned entries are starred on the calendar. **Pinned** in the side menu opens a page of cards (date tile, category, mood, title, a short preview with **Show more** / **Show less** to open the whole entry, pictures included, word count and **Open day**), with category filter chips and a sort order (newest, oldest or longest first). Click a card's star to unpin it.
- **Insights** (side menu) is a full page with a range switch (30 days, 90 days, This year, All time) and three tabs. *Mood patterns*: average mood, how many entries have a mood, and bars of average mood by day of the week, time of day, entry length and category (bars are dimmer where fewer than three entries back them up; the first ten ratings unlock the written patterns; **View as table** shows the numbers). *Writing habits*: entries, words, words per entry, favourite words, when you write and your longest entries. *Look back*: a recap of a week, month or year with highlights and pictures, which can be saved as a PDF.
- **Search** across all entries from the box in the title bar (`Ctrl+F`). Every word you type must appear, in any order and any capitalisation; put a phrase in "quotes" to match it exactly, and a `-` before a word to leave out entries containing it. Matches are highlighted in a short excerpt. The **Filters** button narrows results by category, mood, time period (or between two dates), starred entries and entries with attachments, and results can be sorted newest first, oldest first or by best match. Filters work with or without words. Category, date and attachment names are searched too; private entries are found only by category or date, never by their text. Click a result to jump to its day.

**Writing**
- **The writing view** is a full page rather than a pop-up (**New entry**, or **Open** on an entry): details chips for the date, time, category, Pin and Private; the mood row; a prompt card; and a writing card with a toolbar, an optional title and the text. **Discard** asks you to click twice when you have written something. The page is a **WYSIWYG rich text editor** (Markdown underneath; a title is stored as a first-line `# Heading`): type in a single page where bold looks bold, headings are large and the Markdown marks disappear. The Markdown marks are never in the way: they stay hidden (the arrow keys, Backspace and Delete step over them as if they were not there) unless you tick "Show formatting marks". A toolbar offers bold, italic, strikethrough, three heading sizes, bulleted and numbered lists and quotes.
- **Pictures** appear in the page where you put them. Add them with the **Image** button or by dropping a file onto the editor, and remove them with the button under each one. Large images are shrunk (longest side 1600 px) and stored inside the encrypted diary.
- **Mood**: rate each entry with five outline faces, from Awful to Great, each in its own colour. Click one again to clear it. The scale is stored as 1 to 5.
- **Templates** (**Template** menu in the editor, or **Settings → Templates**): eight ready-made starting points (Gratitude, Daily review, Evening wind-down, Weekly reflection, Mood check-in, Goals check-in, Dream log, Travel log) plus your own. Use `{date}`, `{weekday}` and `{time}` in your own and they are filled in automatically. You can pick one to open with every new entry.
- **Writing prompts**: a card above the page offers a short question ("What made you smile today?") and stays while you write. **Use this prompt** puts it into the entry as a heading at the cursor, on a line of its own, ready for your answer; do it again for as many prompts as you like, anywhere in the entry. **Another** shows the next one, and the × turns them off (back on under **Settings**).
- **AI helpers** (optional, off until you set them up; see below): draft an entry from your rough notes, summarise a day, or suggest a question to reflect on.
- **Attachments**: attach any file (up to 25 MB each) with **Attach file…** or by dropping it on the editor. Files are stored inside the encrypted diary; use **Save as…** or **Open** on the entry's card to get them out. (**Open** writes a temporary, unencrypted copy that is deleted when the diary locks or the app next starts.)
- **Private entries**: tick **Private** and the entry stays hidden (its text, mood, pictures and search matches) until you click it open. It is hidden again when the diary locks or the window loses focus.
- **Word count and daily goal**: the editor shows a live word count. Pick a daily goal under **Settings → Daily word goal** and a progress bar tracks the day's total in the editor and the day panel.
- **Multiple entries per day**, each with a time and a category. You can change an entry's date and time when you edit it.
- **Categories** with their own colours. Add, rename, recolour and delete them from the Categories window. Renaming a category updates the entries that use it.

**Privacy and data**
- **Password protection.** The diary is encrypted with a password you choose, and the app asks for it every time it starts. **Lock** locks it immediately.
- **Recovery key** (**Settings → Recovery key**): a long code, shown once, that can open the diary if you forget your password. On the unlock screen choose **Forgot your password?**, enter the key and pick a new password. You can make a new key (the old one then stops working) or remove it. You are offered one when you first create a diary.
- **Recently deleted** (**Settings → Recently deleted**): deleting an entry moves it to a bin for 30 days, with an **Undo** straight away, **Restore**, **Delete for good** and **Empty the bin**. Pictures and attachments are kept while an entry is in the bin.
- **Auto-lock** after a period of inactivity (default 10 minutes; change or turn off under **Settings → Auto-lock**). An entry you're in the middle of writing survives locking.
- **Privacy screen**: the diary is hidden while its window is in the background or minimised (on by default; switch it off under **Settings**).
- **Text size**: **Settings → Text size**, or `Ctrl +` / `Ctrl -` (and `Ctrl 0` to reset), makes everything bigger or smaller; the choice is remembered.
- **Export to PDF** (**Settings → Export to PDF**, or `Ctrl+E`): a day, week, month, year or everything, with or without pictures. Private entries are left out unless you tick the box for them.
- **Backup**: **Settings → Back up encrypted copy** saves a copy of the encrypted file wherever you like.
- **Automatic backups** (**Settings → Automatic backups**): the first time the diary is opened each day, and only if it has changed, a dated copy of the encrypted file is saved in a `backups` folder next to it. You choose how many to keep (default 10, or off); older ones are removed. The window lists them, can make one now, open the folder, delete one, or **restore** one: the current file is first saved as a "before restore" safety copy (never removed automatically), the diary locks, and you unlock it with the password that copy had when it was made. Backups are encrypted exactly like the diary.
- **Data location**: **Settings → Data location** shows where the diary lives, moves it to another folder, or opens a diary (such as a backup) from another folder.

**Look and feel**
- **Settings** is a full page (the Settings item at the bottom of the side menu): its own left column (Back to journal, then Security, Writing, Your data, Display and Help, which scroll to the section) and cards of rows, each with a title, a line of explanation and its control on the right (button, drop-down or switch). Changes are saved as you make them. Pop-up dialogs still open for changing the password, the recovery key, templates, backups, export, data location, the bin and the shortcut list.
- **Tideline Journal design**: a dark interface (there is no light mode) following `TIDELINE_JOURNAL_DESIGN.md`: a title bar with the search box and window controls (drag it to move the window, double-click to maximise), a side menu (New entry, Today, Calendar, Timeline, Pinned, Photos, Insights, your categories with their counts, Lock journal and Settings), the calendar in the middle and the day panel on the right. Text is set in two typefaces bundled with the program (see `assets/fonts`, both under the SIL Open Font License): Hanken Grotesk for the interface and Newsreader for headings and for everything you write. **Timeline** lists every entry newest first. The Settings menu is the one at the bottom of the side menu.- Pop-up windows are draggable and resizable.
- **Window on startup** (**Settings → Display**): a normal window, maximised (fills the screen, keeps the taskbar) or full screen (covers the taskbar). `F11` switches full screen on and off any time, even on the unlock screen. The choice is kept in the small plain settings file `datanfig.json` (next to the data-location setting), because the window opens before the diary is unlocked.
- **Accent colour** (**Settings → Display**): the one colour you can change. Pick one of eight (Tide, Sky, Violet, Rose, Lamp, Ember, Moss, Slate) or any colour of your own; it drives the main buttons, the selected day, focus and the prompt card, and is automatically adjusted so it stays readable on the dark background. It applies instantly and is also used on the unlock screen (kept in the same plain `config.json`). A choice saved under the earlier blue design is ignored. PDF exports keep their blue headings.
- Keyboard shortcuts (see below, or **Settings → Keyboard shortcuts**).

## Using the app

| Action | How |
| --- | --- |
| Select a day | Click it on the calendar |
| New entry | Double-click a day, press `Ctrl+N`, or use **New entry** / **Add entry for this day** |
| Edit or delete an entry | Use the buttons on its card in the day panel, or double-click its pill |
| Format text | Select text and use the toolbar buttons (click again to remove the format), or type Markdown such as `**bold**` or `# Heading` |
| Add a picture | **Image…** in the editor, or drop an image file onto the editor |
| Switch view | **Day / Week / Month / Year / Photos** buttons above the calendar, or `Ctrl+1` … `Ctrl+5` |
| Look back | **Random** or `Ctrl+R` |
| Move around | **◀** / **▶** or the left/right arrow keys step a day, week, month or year; **Today** (`Ctrl+T`) jumps back |
| Move an entry | Drag its pill onto another day (or, in the Day view, another hour) |
| Search | `Ctrl+F`, then type (or use **Filters**); **Clear** goes back to the day view |
| Save an entry | **Save**, `Ctrl+S` or `Ctrl+Enter` |
| Lock | **Lock** or `Ctrl+L` |
| Full screen | `F11` (or **Settings → Full screen on / off**) |

Other editor shortcuts: `Ctrl+B` bold, `Ctrl+I` italic.

Deleting a category does not delete its entries; they are shown in grey.

## AI helpers (optional)

Three helpers use [Ollama](https://ollama.com), a free program that runs AI models **on your own PC**:

- **AI draft**: in the editor, jot rough notes, then **AI draft** turns them into a diary entry in your voice. You choose whether to add it below your notes or replace them.
- **Summarise this day**: on a day's panel, writes a short summary of that day's entries and can save it as an entry in an "AI" category.
- **AI question**: suggests a thoughtful question based on your recent entries; **Use as a heading** starts your entry with it.

They are off until you set them up under **Settings → AI helpers**, which walks through three steps: **1.** Ollama (the app can install it for you with Windows' own package manager, winget, after you tick a confirmation; or open the download page; or start it if it is installed but not running), **2.** a model (pick one that is installed, or download one such as Llama 3.2, Qwen 2.5, Gemma 2 or Phi-3 mini, with a progress bar), **3.** turn the helpers on. The AI buttons only appear while Ollama is running with a model chosen.

How your privacy is protected:

- The app only ever talks to Ollama on this PC (`127.0.0.1`); nothing is sent over the internet.
- **Private entries are never sent**, and neither are summaries the AI wrote earlier.
- The helpers are told to use only what you wrote and not to invent anything. Each result window has **Show exactly what was sent to Ollama**.
- Nothing is saved until you choose to: you can edit the AI's text first. Entries that include AI text are marked **AI-assisted**; day summaries are marked **AI summary**, and they don't count towards your word counts, streaks or statistics.
- The first answer can take a minute while the model loads into memory. Small models write modestly; bigger ones are better but slower and need more RAM.

## Running it

You need the [Rust toolchain](https://rustup.rs/). From the project folder:

```
cargo run
```

Use `cargo run --release` for an optimised build. The build embeds the app icon in the `.exe`, which needs the Windows SDK's resource compiler (installed with the Visual Studio C++ build tools). If it is missing, the build still succeeds with a warning and the file just keeps the default icon.

## Where your data is stored

Entries, categories, images and settings are saved, encrypted, to `entries.json` in a **`data` folder next to the program** (while developing, in the project folder), with the automatic backups in `data/backups`, so it is easy to find and copy. You can choose another folder from **Settings → Data location**; the choice is remembered in `data/config.json`. The `data` and `backups` folders are in `.gitignore`, so a diary is never committed. Set the environment variable `TIDELINE_DATA_DIR` to run the program against a different folder.

Earlier versions kept the diary in `%APPDATA%/VibeDiary` (or, before that, next to the source code). If one is found and there is none yet in the `data` folder, it is **copied** across the first time you run the app (the original is left alone, so you can delete it once you are happy).

## Encryption

- The diary is sealed with **XChaCha20-Poly1305** under a random 256-bit data key, which also detects a damaged or tampered file. That data key is stored only in "wrapped" (encrypted) form: once under a key stretched from your password with **Argon2id** (64 MiB, 3 passes), and, if you made one, once under a key stretched from your recovery key. Either opens the diary. Changing the password re-wraps the data key, so the recovery key keeps working.
- `entries.json` is a small JSON envelope holding the key-derivation settings, the wrapped keys, a nonce and the ciphertext. Nothing in it reveals your entries or pictures, and the recovery key itself is never stored. Diaries from before recovery keys are upgraded to this format automatically the first time you unlock them (after that, older versions of the app can't open the file).
- Every save uses a fresh nonce, is checked by decrypting it again, and replaces the old file atomically.
- **Without a recovery key there is no way back.** If you forget the password and have no recovery key, nobody can open the diary. That is what makes the encryption real. Keep a recovery key, or a backup whose password you remember.
- Passwords must be at least 8 characters.
- An unencrypted `entries.json` from an earlier version is offered for encryption the first time you run the app. It is replaced by the encrypted version and no plain-text backup is kept.
- Text and pictures held in memory while the app is unlocked are not protected. Use **Lock** if you step away.
- Because pictures are inside the file, a diary with many large pictures makes every save slower and the file bigger.
- **PDF exports are not encrypted.** They are a plain copy of what you chose to export.

Once decrypted, the data looks like this:

```json
{
  "categories": [
    { "name": "Personal", "color": [96, 165, 250] }
  ],
  "entries": [
    {
      "id": 1,
      "date": "2026-10-08",
      "added_at": "2026-10-08 16:11:27",
      "category": "Personal",
      "text": "# A good day\n\nSome **bold** thoughts.\n\n![photo](img:i1760000000000)",
      "mood": 4,
      "pinned": false,
      "sensitive": false,
      "attachments": [{ "id": "f1760000000001", "name": "ticket.pdf", "size": 48213 }]
    }
  ],
  "images": { "i1760000000000": "<base64 JPEG or PNG>" },
  "files": { "f1760000000001": "<base64 file>" },
  "settings": { "auto_lock_minutes": 10, "daily_word_goal": 250, "ui_scale": 1.0, "privacy_screen": true }
}
```

## PDF export notes

- Fonts come from Windows (Segoe UI, plus Consolas when an entry contains code) and are embedded in the PDF, so a PDF is a few megabytes even for a single day.
- Attachments are listed by name but not included in the PDF.
- Characters the fonts don't have, such as emoji, appear as empty boxes.
- Strikethrough text is shown in grey rather than struck through.
- Pictures are shrunk to 900 px and transparency is flattened onto white.

## Project layout

- `src/main.rs` contains the application: the UI, lock screen, calendar views, editor and file handling.
- `src/markdown.rs` parses and renders entries for display, counts words, and holds the toolbar's text-formatting logic.
- `src/live.rs` styles the text as you type (the WYSIWYG layout) and splits an entry into text and picture blocks.
- `src/stats.rs` works out totals, streaks, the heat-map levels, the mood series and "On this day".
- `src/reports.rs` works out mood patterns, writing statistics and week / month / year reviews; `src/insights.rs` is the Insights window that shows them.
- `src/gallery.rs` is the Photos view and picture viewer; `src/pinned.rs` is the Pinned list.
- `src/export.rs` builds the PDF.
- `src/location.rs` decides where the diary file lives and moves it.
- `src/templates.rs` holds the templates and writing prompts and their window. `src/ollama.rs` is the small client for a local Ollama server, and `src/ai.rs` the AI helpers (requests, background tasks, the setup window and result window).
- `src/vault.rs` contains the encryption: the password, the recovery key and the wrapped data key. `src/recovery.rs` is the Recovery key window; `src/trash.rs` is Recently deleted.
- `src/theme.rs` holds Tideline Journal's colours (fixed tokens plus the accent and everything derived from it, with contrast checks); `src/fonts.rs` installs the bundled typefaces; `src/kit.rs` draws the outline icons, mood faces and the custom buttons, chips, switch and date picker; `src/editor.rs` is the writing view; `src/search.rs` and `src/backups.rs` are search and automatic backups.
- `src/icon.rs` draws the app icon (the Tideline Journal mark); `build.rs` turns it into the `.exe` icon. `src/demo.rs` is a look-around mode: run with the environment variable `TIDELINE_DEMO=1` to start unlocked with made-up entries (nothing real is read or written).
- `Cargo.toml` lists the dependencies: `eframe`/`egui_extras` (UI), `chrono`/`jiff` (dates), `serde`/`serde_json`, `pulldown-cmark` (Markdown), `image` (pictures), `rfd` (file dialogs), `argon2`/`chacha20poly1305`/`base64`/`getrandom` (encryption), `genpdf`/`lopdf` (PDF), `ureq` (talking to Ollama on this PC), and `winresource` (build only).
