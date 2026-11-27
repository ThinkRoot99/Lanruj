# Lanruj

A bullet journal app for Linux, built with GTK 4 and libadwaita. Lanruj helps you take notes and organise your time and routines with the bullet journal method, and keeps your journal as plain Markdown files on your computer.

The name is "jurnal" (Romanian for *journal*) written backwards.

## Features

- **Day, week and month pages** and a **future log**, with yesterday and today side by side.
- **Quick writing with signs**: `.` task, `o` event, `-` note, `*` important, `!` idea.
- **Migration**: every evening, unfinished tasks are reviewed one by one (move to today, schedule, drop or keep). A task postponed several times asks whether it is still worth it.
- **Journal**: a notebook page for every day, for writing at length about what you lived and what you think:
  - bold, italic, underline, strikethrough, text colours and markers;
  - fonts and text sizes for any piece of text, and a size of its own for the whole journal;
  - headings, subheadings, quotes, bulleted, numbered (2, 2.1, 2.1.1) and check lists;
  - the time for each entry of the day;
  - web links (typed addresses become links by themselves) and links to a day, a specific task, event or note, a page or Tracking, with a sign on the linked entry that leads back to the journal.
- **Notes**: ideas jotted down quickly, like small slips of paper stuck on the notebook, on every page, until you delete them.
- **Tracking**, month by month: your own activities (ticked, counted or timed), sleep with a chart, and work (duty, standby, leave, overtime), with notes.
- **Index** with a page for each topic, **tags**, and **search** across everything: pages, journal, notes, Tracking and the Index, with exact phrases and words to leave out.
- **Colours, links, pictures and files** inside your pages.
- **Export** to PDF (a page as it looks, several days, the journal or the month's Tracking on A4 sheets), **printing**, and the whole journal as a **zip** archive, with or without a password.
- **Lock** with a password or a 4-digit PIN, locking when the screen locks, and **optional encryption** of every file (age), with a recovery key.
- **Automatic backups**, a **keyboard-first** design (every action has a key, listed with Ctrl+?) with an optional **Vim key scheme**, full screen with only the pages, several paper styles, seven built-in fonts and a dark mode.
- A **first start** in three steps (language; paper, font and text size; optional encryption) and a guide (F1).
- Available in **English** and **Romanian**.

See [CHANGELOG.md](CHANGELOG.md) for everything that has been done so far.

## Your journal

The journal lives in `~/.lanruj`, as plain Markdown files:

- one file per day, for example `2026/11/2026-11-21.md`, plus files for the months, weeks, the future log (`viitor.md`) and the index (`index.md`, `pagini/`);
- the journal, one file per day: `jurnal/2026/2026-11-21.md`;
- Tracking, one file per month: `monitorizare/2026-11.md`;
- the notes: `bilete.md`.

Pictures sit next to the day they belong to. When encryption is off, you can read and edit the files with any text editor, and sync them between computers with Syncthing.

## Building from source

You need Rust (1.88 or newer), GTK 4.18 or newer and libadwaita 1.8 or newer, with their development files.

```
cargo build --release
./target/release/lanruj
```

To try the app without touching your real journal, point it to another folder:

```
LANRUJ_DIR=/path/to/a/test/folder cargo run
```

## Project layout

- `crates/lanruj-core`: the journal itself (Markdown format, migration, index, search, encryption), with tests.
- `crates/lanruj-gtk`: the GTK application.
- `data/icons`: the app icon.
- `data/fonts`: the bundled fonts and their licences.

## Fonts

Lanruj comes with seven fonts for the notebook pages, chosen in Preferences: Inter, Noto Sans, Caveat and Patrick Hand (handwriting), Lora (book), Courier Prime (typewriter) and Nunito (rounded). They are built into the app, so they work without being installed on the system. All of them are released under the SIL Open Font License 1.1; their licences are in `data/fonts`.

## Licence

Lanruj is free software, released under the GNU General Public License, version 3 or later (GPL-3.0-or-later). See [LICENSE](LICENSE).

The Bullet Journal method was created by Ryder Carroll. Lanruj is an independent app and is not affiliated with it or its creator.
