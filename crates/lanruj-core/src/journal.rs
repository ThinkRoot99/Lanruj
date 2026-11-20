// SPDX-License-Identifier: GPL-3.0-or-later
//! Jurnalul de pe disc: `~/.lanruj/2026/11/2026-11-20.md`.

use crate::day::{Day, Entry, Kind, TaskState};
use crate::lang::Lang;
use chrono::{Datelike, NaiveDate};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct Journal {
    root: PathBuf,
}

impl Journal {
    pub fn new(root: impl Into<PathBuf>) -> Journal {
        Journal { root: root.into() }
    }

    /// Dosarul fix `~/.lanruj`. `LANRUJ_DIR` îl schimbă doar pentru teste.
    pub fn default_root() -> PathBuf {
        if let Some(d) = std::env::var_os("LANRUJ_DIR").filter(|d| !d.is_empty()) {
            return PathBuf::from(d);
        }
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        home.join(".lanruj")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn month_dir(&self, year: i32, month: u32) -> PathBuf {
        self.root.join(format!("{year:04}")).join(format!("{month:02}"))
    }

    pub fn day_path(&self, date: NaiveDate) -> PathBuf {
        self.month_dir(date.year(), date.month()).join(format!("{}.md", date.format("%Y-%m-%d")))
    }

    /// Ziua de pe disc; o zi fără fișier e o zi goală.
    pub fn load(&self, date: NaiveDate) -> io::Result<Day> {
        match fs::read_to_string(self.day_path(date)) {
            Ok(text) => Ok(Day::parse(date, &text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Day::new(date)),
            Err(e) => Err(e),
        }
    }

    /// Scrie ziua dintr-o dată (fișier temporar, apoi redenumit), ca o
    /// întrerupere de curent să nu lase un fișier pe jumătate.
    /// O zi goală fără fișier nu creează unul.
    pub fn save(&self, day: &Day, lang: Lang) -> io::Result<()> {
        let path = self.day_path(day.date);
        if day.is_empty() && !path.exists() {
            return Ok(());
        }
        fs::create_dir_all(path.parent().expect("are dosar"))?;
        let tmp = path.with_extension("md.tmp");
        fs::write(&tmp, day.to_markdown(lang))?;
        fs::rename(&tmp, &path)
    }

    /// Zilele lunii care au ceva scris, pentru calendarul mic.
    pub fn written_days(&self, year: i32, month: u32) -> Vec<u32> {
        let mut days = Vec::new();
        let Ok(rd) = fs::read_dir(self.month_dir(year, month)) else { return days };
        for f in rd.flatten() {
            let name = f.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(stem) = name.strip_suffix(".md") else { continue };
            let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") else { continue };
            if date.year() == year && date.month() == month
                && let Ok(day) = self.load(date)
                    && !day.is_empty() {
                        days.push(date.day());
                    }
        }
        days.sort_unstable();
        days
    }

    /// De câte ori a fost mutată sarcina cu semnul `id` (zilele în care apare cu `[>]` sau `[<]`).
    pub fn migration_count(&self, id: &str) -> usize {
        let needle = format!(" ^{id}");
        let mut n = 0;
        for path in self.day_files() {
            let Ok(text) = fs::read_to_string(&path) else { continue };
            for line in text.lines() {
                let t = line.trim_start();
                if (t.starts_with("- [>] ") || t.starts_with("- [<] ")) && t.ends_with(&needle) {
                    n += 1;
                }
            }
        }
        n
    }

    pub(crate) fn day_files(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let Ok(years) = fs::read_dir(&self.root) else { return out };
        for y in years.flatten() {
            if !y.file_name().to_str().is_some_and(|n| n.len() == 4 && n.chars().all(|c| c.is_ascii_digit())) {
                continue;
            }
            let Ok(months) = fs::read_dir(y.path()) else { continue };
            for m in months.flatten() {
                let Ok(files) = fs::read_dir(m.path()) else { continue };
                for f in files.flatten() {
                    let p = f.path();
                    if p.extension().is_some_and(|e| e == "md") && p.file_name().is_some_and(|n| n != "luna.md") {
                        out.push(p);
                    }
                }
            }
        }
        out
    }

    /// Adaugă o intrare la sfârșitul zilei și o salvează.
    pub fn add(&self, date: NaiveDate, entry: Entry, lang: Lang) -> io::Result<Day> {
        let mut day = self.load(date)?;
        day.push(entry);
        self.save(&day, lang)?;
        Ok(day)
    }

    /// Citește ziua, o schimbă și o salvează.
    pub fn update(&self, date: NaiveDate, lang: Lang, f: impl FnOnce(&mut Day)) -> io::Result<Day> {
        let mut day = self.load(date)?;
        f(&mut day);
        self.save(&day, lang)?;
        Ok(day)
    }

    /// Bifează sau debifează sarcina de pe poziția `index`.
    pub fn toggle_done(&self, date: NaiveDate, index: usize, lang: Lang) -> io::Result<Day> {
        let mut day = self.load(date)?;
        if let Some(e) = day.entry_mut(index) {
            e.kind = match e.kind {
                Kind::Task(TaskState::Open) => Kind::Task(TaskState::Done),
                Kind::Task(TaskState::Done) => Kind::Task(TaskState::Open),
                k => k,
            };
        }
        self.save(&day, lang)?;
        Ok(day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day::parse_quick;

    fn temp_root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("lanruj-test-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn adauga_citeste_bifeaza() {
        let root = temp_root("adauga");
        let j = Journal::new(&root);
        let d = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        assert!(j.load(d).unwrap().is_empty());
        j.save(&Day::new(d), Lang::Ro).unwrap();
        assert!(!j.day_path(d).exists(), "o zi goală nu creează fișier");

        j.add(d, parse_quick(". Cumpără becuri").unwrap(), Lang::Ro).unwrap();
        j.add(d, parse_quick("o 18:30 Cină").unwrap(), Lang::Ro).unwrap();
        let text = fs::read_to_string(root.join("2026/11/2026-11-20.md")).unwrap();
        assert_eq!(text, "# Vineri, 20 noiembrie 2026\n\n- [ ] Cumpără becuri\n- o 18:30 Cină\n");

        let day = j.toggle_done(d, 0, Lang::Ro).unwrap();
        assert_eq!(day.entries().next().unwrap().kind, Kind::Task(TaskState::Done));
        assert_eq!(j.written_days(2026, 11), vec![20]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn numara_mutarile() {
        let root = temp_root("mutari");
        fs::create_dir_all(root.join("2026/11")).unwrap();
        for (zi, linie) in [(17, "- [>] Service ^ab12"), (18, "- [>] Service ^ab12"), (19, "- [>] Service ^ab12"), (20, "- [ ] Service ^ab12")] {
            fs::write(root.join(format!("2026/11/2026-11-{zi}.md")), format!("# x\n\n{linie}\n")).unwrap();
        }
        let j = Journal::new(&root);
        assert_eq!(j.migration_count("ab12"), 3);
        assert_eq!(j.migration_count("zz99"), 0);
        fs::remove_dir_all(root).unwrap();
    }
}
