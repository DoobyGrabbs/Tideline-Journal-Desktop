# Tideline Journal — UI redesign spec

Rename the app from "Diary" to **Tideline Journal** (tagline: "Every day leaves a mark.").
Use this file with the three mockup images (calendar, new entry, brand sheet). Where the images and this file disagree, this file wins.

## Colour tokens

| Token | Hex | Use |
|---|---|---|
| bg | #111418 | Main background |
| bg_titlebar | #0D1013 | Title bar |
| panel | #15191E | Side menu, right day panel, action bars |
| surface | #1C2127 | Cards |
| cell | #1A1E24 | Calendar day cells |
| border | #2A3039 | Card and button borders |
| border_soft | #22272E | Panel dividers |
| text | #ECE7DD | Main text (warm off-white) |
| text_2 | #A9AFB8 | Secondary text |
| text_3 | #7E8590 | Labels, weekday headers, counts |
| accent | #4FB3A5 | Primary buttons, selected day outline, focus |
| on_accent | #0B1F1C | Text on accent buttons |
| today | #E7B15A | Today's date circle, pinned star |

**Categories.** Show each category as a small dot plus a tinted label: the colour at about 12% opacity behind text in the full colour. Don't fill whole cards with it.
Personal #7AA2F7 · Health #6CC497 · Work #E7B15A · Ideas #B392F0 · Pink Stuff #E57BB8

**Mood scale (1–5).** Awful #E5735F · Low #E99A5B · Okay #D9C25E · Good #8CC97A · Great #4FB3A5

## Typography

- **Newsreader** (serif) for entry titles, entry body text, the month heading and the day heading. Body text 15–18 px, line-height 1.6–1.7.
- **Hanken Grotesk** (sans) for all UI: buttons, menus, labels and numbers. 13–15 px, weights 500 and 600.
- Both are free on Google Fonts. Bundle the .ttf files with the app; don't load them at runtime.
- Small section labels are 11–12 px, uppercase, letter-spacing about 1.2 px, colour text_3.

## Shape and spacing

- Radius: 8 px buttons, 10 px calendar cells, 14 px cards, fully rounded (pill) for chips and labels.
- Spacing: 4/8 px grid. Gaps of 8 px between calendar cells, 16 px between panel sections, 24–28 px padding in the main area.
- Make click targets at least 36 px tall where possible.
- Icons are thin outline icons (stroke about 1.8 px). No emoji.

## Main window layout (calendar)

1. **Title bar** (44 px): logo and the wordmark "Tideline *Journal*" ("Journal" in italic, colour text_2) on the left, a search box in the centre ("Search your journal", Ctrl+F hint), window controls on the right.
2. **Left side menu** (about 220 px, colour panel):
   - Accent "New entry" button.
   - Navigation: Today, Calendar, Timeline, Pinned, Photos, Insights. The active item gets a #232A33 background.
   - "CATEGORIES" list: colour dot, name, and entry count on the right.
   - Lock journal and Settings at the bottom.
3. **Centre**:
   - Header row: ◀ ▶ buttons, a large serif "October 2026" with the year in text_3, then Today, "Surprise me" (was Random) and a Year/Month/Week/Day segmented control.
   - The Photos button moves to the side menu.
4. **Month grid**: 7 columns starting Monday.
   - Each cell has the date number at top left and a mood dot (10 px) at top right.
   - Entry chips sit inside the cell: category dot, time, title, all one line, cut off with "…" if too long.
   - Today's date number sits in an amber (#E7B15A) circle.
   - The selected day gets a 2 px accent outline.
   - Days outside the month have no fill and dim numbers.
5. **Right day panel** (about 320 px, colour panel):
   - Weekday label, a large serif date, and a summary line ("2 entries · 141 words · Mood: Okay").
   - Two buttons: "Write about this day" (accent outline) and "Summarise" (AI).
   - Entry cards: category label, time, pin star, serif title, body preview (5 lines max), then a footer with the word count, Open and ⋯.
   - Empty day: a dashed box reading "A blank page."

## New entry screen

Make this a full writing view, not a pop-up.

1. **Top bar**:
   - "← Calendar / New entry" on the left.
   - On the right: a "Draft saved" status, Discard, and an accent "Save entry" button.
2. **Centred column**, max width about 780 px:
   - **Detail chips:** date, time, and category (tinted, with a dropdown arrow). Pin and Private toggles sit on the right.
   - **Mood row:** "How are you feeling?" followed by 5 pill buttons, each with a face icon and a word label. The selected pill is tinted in its mood colour.
   - **Prompt card:** dark teal (#172423, border #24403C) with a "TODAY'S PROMPT" label. The prompt is in italic serif. Buttons: "Use this prompt" and "Another", plus × to hide.
   - **Writing card** (colour #171B20):
     - Icon toolbar in groups: B I S | H1 H2 H3 | lists, quote, code | image, attach, Template, with "Ask AI" on the right.
     - Below the toolbar: an optional serif title (32 px), then the serif body (18 px).
     - Footer: word count and read time, "Show formatting marks" checkbox, and a "Ctrl S to save" hint.

## Logo

A circle outline, a wave line across its lower half, and a small filled dot (moon) above the wave. All in accent #4FB3A5 on #15302D. As SVG paths (24×24 viewBox):

- circle: `M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0`
- wave: `M5.5 14.5c1.6-1.4 3.1-1.4 4.6 0s3.1 1.4 4.6 0 3.1-1.4 3.8 0`
- moon: filled circle r=1.6 at (12, 8.2)

At 16 px, show only the wave, in dark on an accent tile.

## Insights screen

Make Insights a full screen with the side menu still visible (Insights highlighted), not a pop-up.

1. **Header:**
   - Serif "Insights" (34 px), with the subtitle "Patterns in how you feel and how you write."
   - On the right, a date-range control with four buttons in one bar (30 days / 90 days / This year / All time) instead of radio buttons. The selected one gets a #2B323C background.
2. **Tabs:** Mood patterns / Writing habits / Look back. These are text tabs on a bottom border line; the active tab gets a 2 px accent underline and bold text.
3. **Summary tiles:** three cards in a row, wrapping on narrow windows.
   - **Average mood:** a large serif number with "out of 5 · Okay".
   - **Entries with a mood:** the count, with "of N entries this year".
   - **Progress card:** uses the prompt-card teal. "Patterns unlock at 10 ratings", a progress bar (3 of 10), and one line of encouragement. It replaces the plain-text "rate a few more entries" warning. Hide it once 10 or more entries are rated.
4. **Chart cards** (colour #1A1E24, radius 14): one card each for day of week, time of day and length of entry, two columns when wide.
   - Serif card title, then the subtitle "Average mood, 1 (awful) to 5 (great)".
   - **Each row is a grid:** label (132 px) | bar track | value (120 px). Label the time and length buckets in full, e.g. "Afternoon · 12:00–17:00", "Short · under 50 words".
   - **Bars:** the track is #21262D, 12 px high and fully rounded. Every bar is a single accent colour (#4FB3A5). Don't colour bars by mood value, because red/amber/green fails for colour-blind users and the number already says it. Bar width = mean / 5.
   - **Low data:** if a row has fewer than 3 ratings, draw its bar at 40% opacity as an "early signal".
   - **Value text:** "2.7 Okay · 3 entries", in the main text colour. Rows with no data show "No ratings yet" in #6B727C and no bar.
   - **Gridlines and axis:** faint vertical gridlines (#2C333C) at 1, 2, 3, 4, and a small 1–5 axis under each card.
   - **Hover:** a row gets a #22282F background and a tooltip, e.g. "Thu: average 3.0 (Okay) from 2 entries".
5. **Footer:** a legend explaining solid vs faded bars, plus a "View as table" link that shows the same numbers as a plain table.

## Pinned screen

Make Pinned a full screen with the side menu still visible, not a pop-up.

**Side menu bug:** only the current screen should be highlighted. In the current build, Timeline and Pinned are both highlighted at once.

1. **Header:**
   - Serif "Pinned" (34 px), with the subtitle "N entries you've starred to keep close."
   - On the right, a "Sort by" dropdown: Newest entry first / Oldest entry first / Recently pinned.
2. **Category filter row:**
   - Pill buttons (34 px tall): "All" first, then one per category that has pinned entries. Each shows the colour dot, the name and a count.
   - The selected pill gets an accent border and a #2B323C background.
   - Hide categories with no pinned entries.
3. **Card grid:** cards about 340 px wide, as many columns as fit (radius 14, colour surface). Each card has:
   - **Date block** (52×56, #232A33, radius 10) on the left: "OCT" in small caps over a serif day number.
   - **Beside it:** "Monday 12 October · 13:59" in text_2. Below that, the tinted category label and the mood face icon with its word (e.g. "Low").
   - **A filled amber star** at top right. Clicking it unpins the entry, with an undo message for 5 seconds.
   - **Serif title** (20 px). Entries without a title use the first line of the text, cut at about 60 characters.
   - **Body preview:** serif 15 px, 4 lines max.
   - **Footer:** a top border line, the word count, and an "Open day ›" button that jumps to the calendar on that day.
4. **Dates:** never show ISO dates like 2026-10-12 to the user. Use "Monday 12 October" for this year and add the year for older entries.
5. **How-to card:** always the last tile in the grid, with a dashed border. It reads "Keep the ones that matter close." plus how to pin: click the star, or press Ctrl+D while writing.
6. **Empty state:** if nothing is pinned, show only the how-to card, centred, with the subtitle "Nothing pinned yet."

## Search panel (also seen in the current build)

1. **Dates and stars:** results use the same friendly date format and a filled star for pinned entries.
2. **Filter controls:**
   - Put the filter dropdowns (category, mood) on one row that wraps.
   - Replace the "Starred only" and "Has attachment" radio buttons with toggle pills, since they can each be on or off.

## Settings screen

**Replace the gear pop-up menu with a full Settings screen.** The pop-up is too small, needs scrolling, and cuts off long labels. Clicking Settings in the side menu (or pressing Ctrl+,) opens this screen.

1. **Left column** (same width and colour as the side menu, which it replaces while Settings is open):
   - "‹ Back to journal" at the top.
   - A small "SETTINGS" label.
   - Section links with icons: Security, Writing, Your data. Clicking one scrolls to that section and highlights it.
   - Add new sections here later (e.g. Appearance) rather than making the page longer.
2. **Main area:** content max width about 820 px, so lines stay readable on wide screens.
   - Serif "Settings" (34 px), with the subtitle "Changes save as you make them." There's no Save button; every change applies straight away.
3. **Sections:**
   - Each section has a small uppercase heading (12 px, text_3).
   - Below it is one card (surface colour, radius 14) holding its rows, with 1 px dividers between rows.
4. **Each row:** a name (15 px, weight 500) and a one-line explanation (13 px, text_3) on the left, and its control on the right. Rows are at least 64 px tall. The control depends on the setting:
   - **On/off settings:** a switch (44×26, accent when on, #3A414B when off). Use it instead of a ✓ menu item.
   - **Choose-one settings:** a dropdown.
   - **Actions:** an outline button (34 px high).
5. **Contents:**
   - **Security:**
     - Password: [Change password]
     - Recovery key: [Show key], with a note to keep a copy safe
     - Auto-lock: dropdown, 1 min / 5 min / 15 min / 1 hour / Never
     - Hide when in the background: switch
   - **Writing:**
     - Writing prompts: switch
     - Daily word goal: dropdown, Off / 100 / 250 / 500 / 750 words
     - Templates: [Manage]
     - AI helpers: [Set up]
   - **Your data:**
     - Automatic backups: switch, with the last backup time in its explanation line
     - Back up now: [Back up…]
     - Export to PDF: [Export…], with the warning "Not encrypted"
6. **Narrow windows:** the left column stacks above the content, and row controls drop below their text instead of squashing it.
