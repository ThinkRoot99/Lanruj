# Lanruj

A bullet journal app for Linux, built with GTK 4 and libadwaita. Lanruj helps you take notes and organise your time and routines with the bullet journal method, and keeps your journal as plain Markdown files on your computer.

The name is "jurnal" (Romanian for *journal*) written backwards.

## Features

- **Day, week and month pages** and a **future log**, with yesterday and today side by side.
- **Quick writing with signs**: `.` task, `o` event, `-` note, `*` important, `!` idea.
- **Migration**: every evening, unfinished tasks are reviewed one by one (move to today, schedule, drop or keep). A task postponed several times asks whether it is still worth it.
- **Index** with a page for each topic, **tags**, and **search** across the whole journal.
- **Colours, links, pictures and files** inside your pages.
- **Export** to PDF (a page as it looks, or several days), **printing**, and the whole journal as a **zip** archive, with or without a password.
- **Lock** with a password or a 4-digit PIN, locking when the screen locks, and **optional encryption** of every file (age), with a recovery key.
- **Automatic backups**, a **keyboard-first** design with an optional **Vim key scheme**, several paper styles and a dark mode.
- Available in **English** and **Romanian**.

See [CHANGELOG.md](CHANGELOG.md) for everything that has been done so far.

## Your journal

The journal lives in `~/.lanruj`, as one Markdown file per day (for example `2026/11/2026-11-21.md`), plus files for the months, weeks, future log and index. Pictures sit next to the day they belong to. When encryption is off, you can read and edit the files with any text editor, and sync them between computers with Syncthing.

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
