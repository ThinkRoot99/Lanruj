# Changelog

All notable changes to Lanruj are listed here, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Lanruj has not had a release yet; everything below belongs to the first version.

## [Unreleased]

### 2026-11-21 (afternoon)

#### Added
- **Journal toolbar in groups**: text style, colors, lines, lists, then time and links; each group wraps as a whole on narrow windows. Clearing the formatting keeps the links. The keyboard shortcuts window describes every key exactly, with its own sections for pages and the journal. Printing and PDF say plainly that the journal can't be exported yet.
- **Journal writing**: typed or pasted web addresses, `www.` addresses and e-mails become links by themselves; the link button (Ctrl+K) asks for the text and the address, or changes and removes a link; lists change level with Tab and Shift+Tab, Enter on an empty line and Backspace after the sign go up one level, and numbered lists follow the levels (2, 2.1, 2.1.1); keyboard shortcuts for everything (headings, quotes, lists, colors, markers, clearing).
- **Search everywhere**: the journal, the notes, Tracking notes and Index titles, besides the pages; exact phrases in quotes and `-word` to leave words out; Enter opens the first result, ↓ moves into the list, and journal results open at the line found.
- **Journal**: a page for writing at length, as in a notebook, one file per day (`jurnal/YYYY/YYYY-MM-DD.md`, plain Markdown, encrypted when the journal is), saved automatically. Bold, italic, underline, strikethrough, text colors and markers, headings, quotes, bulleted, numbered and check lists, the time for each entry of the day, web links.
- **Journal links**: any selected text can link to a day, a specific task, event or note, the week plan, the month page, the future log, the Index or Tracking (Ctrl+L, opened with Ctrl+click); linked entries show a sign that leads back to the journal, Tracking lists the journal days that mention it, and every entry's menu has "Write about it in the journal".
- **Notes** (light bulb in the header, Ctrl+N): ideas jotted down quickly, shown like small yellow slips of paper on the notebook on every page, kept apart from the pages until you delete them. Notes can be dragged anywhere, edited, turned into an idea in today's page, or deleted with Undo; text being written is never lost when the window is closed. Saved in `bilete.md`, encrypted when the journal is.
- **Full screen** (button in the header, Shift+F11): only the notebook pages, without the sidebar, header and window borders; Esc leaves it.
- **Writing font** in Preferences: the system font or one of seven fonts that come with the app (Inter, Noto Sans, Caveat, Patrick Hand, Lora, Courier Prime, Nunito), applied to the notebook pages only. Handwriting fonts are shown slightly larger so they stay easy to read.
- **First start**: a short welcome in three steps (language; paper, font and text size; optional encryption with a recovery key), after which today's page opens with a small example of the signs.
- **Tracking**, with three tabs, month by month:
  - **Activities**: your own list, ticked (coffee, making the bed), counted (push-ups; water in glasses) or timed (plank), day by day, with how many days you did each one and, for counted ones, the total and daily average. The list carries over to the following months.
  - **Sleep**: how long you slept at night and during the day, with an automatic chart of the month and the averages.
  - **Work**: duty (24 hours), standby, leave and overtime (from-to, for example 08:00-12:00), each with an optional note, the month's totals and a list of all the month's notes. Duty, standby, leave and overtime also appear on the day page, the week's days and the month calendar.
  Each month is saved as `monitorizare/YYYY-MM.md`, encrypted when the journal is.
- **Tracking on paper**: Ctrl+P (or "Page as PDF") in Tracking prints the month on two A4 sheets: activities and sleep, with the sleep chart; work, with the calendar, totals and the month's notes. Days still to come stay empty, to be filled in by hand.
- A guide chapter about tracking.

#### Changed
- Preferences are arranged in four tabs: Appearance, Journal, Security and General.
- The two pages of the notebook always have the same width, however much text they hold.

### 2026-11-21

#### Added
- **Website** (not published yet): an English home page and a Romanian page (`lang/ro.html`), with styles, scripts and images in `assets/`. The chosen language is remembered in the visitor's browser. It includes a features section, frequently asked questions and a download section, plus small touches such as a hand-drawn ink underline, a margin note, an animated migration example and sections that fade in as you scroll.
- Screenshots for the website, taken from an English sample journal (`site/tools/jurnal_en.py`).
- **Lock**: Lanruj can ask for a password or a 4-digit PIN when it opens, and can lock itself when the screen locks. An encrypted journal always locks with the screen.
- **Keyring**: the password of an encrypted journal can be kept in the GNOME keyring, so the journal opens as soon as the keyring is unlocked.
- **Start with the computer** option.
- **Password-protected zip export** (AES-256). An encrypted journal always exports a password-protected archive.
- **Encryption** (optional): every page and picture is encrypted separately with age, and file names stay the same. The journal key is locked with your password and with a recovery key, which is shown only once and confirmed by typing its last four characters. The password can be changed, and encryption can be turned off again. A backup is made before the files are converted.
- **"Lanruj is locked" window** at startup, with "I forgot the password", which opens the journal with the recovery key and sets a new password.
- **Vim key scheme** (optional): `j` `k` `gg` `G` `h` `l` `t` `x` `i` `o` `dd` `>>` `<<` `/`, with Esc to leave the writing field.
- **New preferences**: text size, start page, single-page view, the number of moves before "still worth it?" appears, and automatic backups (daily or weekly, in a folder of your choice, keeping the last copies).
- **Export and print**: the current page as a PDF, exactly as it looks and in full even when it scrolls; several days as one PDF; printing; the whole journal as a zip archive.
- Guide chapters for export, encryption and lock.
- **Index**: a manual index where each line has its own page, plus an automatic list of months and tags. Index topics also appear on their days, weeks and months.
- **Links**: web addresses, `[text](address)` and e-mail addresses become links that open with one click.
- **Pictures and files**: attached to a page or under an entry, with names, three sizes, a gallery and a built-in viewer with zoom.
- **Notes under an entry** (Tab and Shift+Tab) and **Go to date** (Ctrl+G).
- **Guide** (F1): every instruction, in chapters, with search.

#### Changed
- **The app is now called Lanruj** ("jurnal" written backwards), because the previous name was already used by other apps. The app ID is now `io.github.ThinkRoot99.Lanruj`. On first start, existing data (journal, settings, state, backups, keys and autostart entry) is moved to the new locations once, without overwriting anything.

#### Fixed
- No empty gap under small and medium pictures in a maximised window.
- Zoom in the picture viewer works again.
- Links open directly with the default program outside Flatpak.

### 2026-11-20

#### Added
- **App icon**: a pen nib writing on a dotted page, with small and symbolic versions.
- **Core and the Day page**: quick writing with signs (`.` task, `o` event, `-` note, `*` important, `!` idea), with yesterday and today side by side. Everything is saved as Markdown files.
- **Migration**: unfinished tasks are sorted one by one (today, schedule, drop, keep), at the end of the day, the week and the month.
- Editing and deleting entries, with undo.
- **Month page** with a calendar and monthly tasks, and a Today button.
- **Future log**, with entries that move into their month on the first day of that month.
- **"Move…"** for any entry, and new names in the sidebar.
- **Week page**, with the week plan next to the days of the week.
- **Preferences**: paper pattern and colour, dark mode, migration and language (English and Romanian).
- **System tray** (optional) when the window is closed, and a keyboard shortcuts window.
- **Keyboard selection** of entries: arrows choose, Enter edits, Delete deletes, Ctrl+Enter checks.
- **Collapsible sidebar** (F9), with a fixed width.
- **Search** across the whole journal (Ctrl+F), with filters and accent-insensitive matching.
- **Main menu** with every action that has a key; F10 opens and closes it, and F11 maximises the window.
- **Colours**: coloured text and highlighter, shown directly in the editing field.

#### Fixed
- Buttons and Enter stopped working right after start-up, because the window state was released too early.
- The tray icon showed three dots instead of the Lanruj icon.
