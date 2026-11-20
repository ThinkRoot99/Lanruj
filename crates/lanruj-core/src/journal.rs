// SPDX-License-Identifier: GPL-3.0-or-later
//! Jurnalul de pe disc: `~/.lanruj/2026/11/2026-11-20.md`.

use crate::day::{Day, Entry, Kind, TaskState};
use crate::lang::Lang;
use crate::future::{Future, split_day};
use crate::month::Month;
use crate::PageRef;
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
        for path in self.page_files() {
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

    /// Toate paginile cu intrări, ca referințe: zilele și listele lunilor.
    pub(crate) fn pages(&self) -> Vec<PageRef> {
        let mut out = Vec::new();
        for p in self.page_files() {
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else { continue };
            if name == "luna.md" {
                let month = p.parent().and_then(|d| d.file_name()).and_then(|n| n.to_str()).and_then(|n| n.parse::<u32>().ok());
                let year = p.parent().and_then(|d| d.parent()).and_then(|d| d.file_name()).and_then(|n| n.to_str()).and_then(|n| n.parse::<i32>().ok());
                if let (Some(year), Some(month)) = (year, month)
                    && (1..=12).contains(&month)
                {
                    out.push(PageRef::Month { year, month });
                }
            } else if let Some(stem) = name.strip_suffix(".md")
                && let Ok(d) = NaiveDate::parse_from_str(stem, "%Y-%m-%d")
            {
                out.push(PageRef::Day(d));
            } else if let Some(stem) = name.strip_suffix(".md")
                && let Some((y, w)) = stem.split_once("-S")
                && let (Ok(year), Ok(week)) = (y.parse::<i32>(), w.parse::<u32>())
                && NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Mon).is_some()
            {
                out.push(PageRef::Week { year, week });
            }
        }
        if let Ok(f) = self.load_future() {
            out.extend(f.months().into_iter().map(|(year, month)| PageRef::Future { year, month }));
        }
        out
    }

    fn page_files(&self) -> Vec<PathBuf> {
        let mut out = self.day_files();
        if self.future_path().is_file() {
            out.push(self.future_path());
        }
        let Ok(years) = fs::read_dir(&self.root) else { return out };
        for y in years.flatten() {
            let Ok(months) = fs::read_dir(y.path()) else { continue };
            for m in months.flatten() {
                let p = m.path().join("luna.md");
                if p.is_file() {
                    out.push(p);
                }
            }
        }
        out
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

    pub fn month_path(&self, year: i32, month: u32) -> PathBuf {
        self.month_dir(year, month).join("luna.md")
    }

    pub fn load_month(&self, year: i32, month: u32) -> io::Result<Month> {
        match fs::read_to_string(self.month_path(year, month)) {
            Ok(text) => Ok(Month::parse(year, month, &text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Month::new(year, month)),
            Err(e) => Err(e),
        }
    }

    pub fn save_month(&self, m: &Month, lang: Lang) -> io::Result<()> {
        let path = self.month_path(m.year, m.month);
        if m.is_empty() && !path.exists() {
            return Ok(());
        }
        fs::create_dir_all(path.parent().expect("are dosar"))?;
        let tmp = path.with_extension("md.tmp");
        fs::write(&tmp, m.to_markdown(lang))?;
        fs::rename(&tmp, &path)
    }

    pub fn update_month(&self, year: i32, month: u32, lang: Lang, f: impl FnOnce(&mut Month)) -> io::Result<Month> {
        let mut m = self.load_month(year, month)?;
        f(&mut m);
        self.save_month(&m, lang)?;
        Ok(m)
    }

    pub fn future_path(&self) -> PathBuf {
        self.root.join("viitor.md")
    }

    pub fn load_future(&self) -> io::Result<Future> {
        match fs::read_to_string(self.future_path()) {
            Ok(text) => Ok(Future::parse(&text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Future::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save_future(&self, f: &Future, lang: Lang) -> io::Result<()> {
        let path = self.future_path();
        if f.is_empty() && !path.exists() {
            return Ok(());
        }
        fs::create_dir_all(&self.root)?;
        let tmp = path.with_extension("md.tmp");
        fs::write(&tmp, f.to_markdown(lang))?;
        fs::rename(&tmp, &path)
    }

    /// Pe 1 ale lunii: intrările lunii din jurnalul viitor trec în pagina lunii.
    /// Sarcinile intră în lista de sarcini (și primesc `[>]` în jurnalul viitor);
    /// evenimentele și notițele cu zi intră în calendarul lunii, cele fără zi în listă.
    /// Întoarce câte intrări au trecut.
    pub fn transfer_future(&self, year: i32, month: u32, lang: Lang) -> io::Result<usize> {
        use crate::day::{Item, Kind, TaskState};
        let mut future = self.load_future()?;
        let mut items = future.items(year, month);
        if items.is_empty() {
            return Ok(0);
        }
        let mut m = self.load_month(year, month)?;
        let mut moved = 0;
        let mut i = 0;
        while i < items.len() {
            let Item::Entry(e) = &items[i] else {
                i += 1;
                continue;
            };
            if e.depth > 0 {
                i += 1;
                continue;
            }
            let e = e.clone();
            // Notițele de sub o intrare merg cu ea.
            let kids: Vec<Item> = items[i + 1..]
                .iter()
                .take_while(|it| matches!(it, Item::Entry(c) if c.depth > 0))
                .cloned()
                .collect();
            match e.kind {
                Kind::Task(TaskState::Open) => {
                    let id = e.id.clone().unwrap_or_else(|| crate::migrate::fresh_id(&[]));
                    let mut copy = e.clone();
                    copy.id = Some(id.clone());
                    m.tasks.push(Item::Entry(copy));
                    m.tasks.extend(kids.iter().cloned());
                    if let Item::Entry(src) = &mut items[i] {
                        src.kind = Kind::Task(TaskState::Migrated);
                        src.id = Some(id);
                    }
                    moved += 1;
                }
                Kind::Task(_) => {}
                Kind::Event | Kind::Note => {
                    let (day, text) = split_day(&e.text);
                    match day {
                        Some(d) => {
                            let mut texts: Vec<String> = m.day_texts(d).iter().map(|t| t.to_string()).collect();
                            let t = match e.time.as_deref() {
                                Some(h) => format!("{h} {text}"),
                                None => text.to_string(),
                            };
                            if !texts.contains(&t) {
                                texts.push(t);
                            }
                            m.set_day(d, &texts);
                        }
                        None => {
                            // O a doua trecere nu o mai adaugă o dată.
                            if !m.tasks.iter().any(|t| matches!(t, Item::Entry(x) if *x == e)) {
                                m.tasks.push(Item::Entry(e.clone()));
                                m.tasks.extend(kids.iter().cloned());
                            }
                        }
                    }
                    moved += 1;
                }
            }
            i += 1 + kids.len();
        }
        self.save_month(&m, lang)?;
        future.set_items(year, month, items, lang);
        self.save_future(&future, lang)?;
        Ok(moved)
    }

    /// `2026/saptamani/2026-S47.md`
    pub fn week_path(&self, year: i32, week: u32) -> PathBuf {
        self.root.join(format!("{year:04}")).join("saptamani").join(format!("{year:04}-S{week:02}.md"))
    }

    /// O pagină cu intrări, ca zi: pentru o lună, lista ei de sarcini.
    pub fn load_ref(&self, r: PageRef) -> io::Result<Day> {
        match r {
            PageRef::Day(d) => self.load(d),
            PageRef::Month { year, month } => {
                let m = self.load_month(year, month)?;
                let mut day = Day::new(NaiveDate::from_ymd_opt(year, month, 1).expect("lună validă"));
                day.items = m.tasks;
                Ok(day)
            }
            PageRef::Future { year, month } => {
                let f = self.load_future()?;
                let mut day = Day::new(NaiveDate::from_ymd_opt(year, month, 1).expect("lună validă"));
                day.items = f.items(year, month);
                Ok(day)
            }
            PageRef::Week { year, week } => {
                let monday = crate::week_monday(year, week);
                match fs::read_to_string(self.week_path(year, week)) {
                    Ok(text) => Ok(Day::parse(monday, &text)),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Day::new(monday)),
                    Err(e) => Err(e),
                }
            }
        }
    }

    pub fn update_ref(&self, r: PageRef, lang: Lang, f: impl FnOnce(&mut Day)) -> io::Result<Day> {
        match r {
            PageRef::Day(d) => self.update(d, lang, f),
            PageRef::Month { year, month } => {
                let mut day = self.load_ref(r)?;
                f(&mut day);
                let items = day.items.clone();
                self.update_month(year, month, lang, move |m| m.tasks = items)?;
                Ok(day)
            }
            PageRef::Future { year, month } => {
                let mut day = self.load_ref(r)?;
                f(&mut day);
                let mut fut = self.load_future()?;
                fut.set_items(year, month, day.items.clone(), lang);
                self.save_future(&fut, lang)?;
                Ok(day)
            }
            PageRef::Week { year, week } => {
                let mut day = self.load_ref(r)?;
                f(&mut day);
                if day.heading.is_none() {
                    let title = lang.week_title(week);
                    let range = lang.week_range(day.date);
                    day.heading = Some(format!("{title}, {range}"));
                }
                let path = self.week_path(year, week);
                if day.is_empty() && !path.exists() {
                    return Ok(day);
                }
                fs::create_dir_all(path.parent().expect("are dosar"))?;
                let tmp = path.with_extension("md.tmp");
                fs::write(&tmp, day.to_markdown(lang))?;
                fs::rename(&tmp, &path)?;
                Ok(day)
            }
        }
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
