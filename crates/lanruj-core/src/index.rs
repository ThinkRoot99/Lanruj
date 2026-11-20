// SPDX-License-Identifier: GPL-3.0-or-later
//! Indexul: partea scrisă de mână (`index.md`) și partea calculată din jurnal
//! (etichetele cu zilele lor, lunile cu zilele scrise).
//!
//! ```text
//! # Index
//!
//! - [Concediu la mare](2026/07/2026-07-12.md): 12-19 iulie 2026
//! - [Ziua lui Andrei](2026/11/2026-11-30.md): 30 noiembrie 2026
//! ```
//!
//! Fiecare rând e un link Markdown obișnuit spre prima zi, deci merge și în alte editoare.

use crate::journal::Journal;
use crate::lang::Lang;
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;
use std::fs;
use std::io;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexItem {
    Entry { title: String, start: NaiveDate, range: String },
    /// Un rând pe care nu-l înțelegem; se păstrează.
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Index {
    pub heading: Option<String>,
    pub items: Vec<IndexItem>,
}

fn parse_line(line: &str) -> Option<IndexItem> {
    let rest = line.strip_prefix("- [")?;
    let close = rest.find("](")?;
    let title = &rest[..close];
    let after = &rest[close + 2..];
    let end = after.find(')')?;
    let path = &after[..end];
    let stem = path.rsplit('/').next()?.strip_suffix(".md")?;
    let start = NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()?;
    let range = after[end + 1..].trim_start_matches(':').trim().to_string();
    Some(IndexItem::Entry { title: title.to_string(), start, range })
}

/// „12-19 iulie 2026”, „30 noiembrie - 2 decembrie 2026”, „30 noiembrie 2026”.
pub fn range_text(lang: Lang, start: NaiveDate, end: Option<NaiveDate>) -> String {
    let one = |d: NaiveDate| format!("{} {} {}", d.day(), lang.month(d.month()), d.year());
    match end.filter(|e| *e > start) {
        None => one(start),
        Some(e) if e.year() != start.year() => format!("{} - {}", one(start), one(e)),
        Some(e) if e.month() != start.month() => {
            format!("{} {} - {} {} {}", start.day(), lang.month(start.month()), e.day(), lang.month(e.month()), e.year())
        }
        Some(e) => format!("{}-{} {} {}", start.day(), e.day(), lang.month(e.month()), e.year()),
    }
}

impl Index {
    pub fn parse(text: &str) -> Index {
        let mut idx = Index::default();
        let mut lines = text.lines().peekable();
        while lines.peek().is_some_and(|l| l.trim().is_empty()) {
            lines.next();
        }
        if let Some(h) = lines.peek().and_then(|l| l.strip_prefix("# ")) {
            idx.heading = Some(h.trim().to_string());
            lines.next();
            if lines.peek().is_some_and(|l| l.trim().is_empty()) {
                lines.next();
            }
        }
        for line in lines {
            let line = line.trim_end();
            idx.items.push(parse_line(line).unwrap_or_else(|| IndexItem::Other(line.to_string())));
        }
        while matches!(idx.items.last(), Some(IndexItem::Other(s)) if s.trim().is_empty()) {
            idx.items.pop();
        }
        idx
    }

    pub fn to_markdown(&self) -> String {
        let mut out = format!("# {}\n\n", self.heading.as_deref().unwrap_or("Index"));
        for it in &self.items {
            match it {
                IndexItem::Entry { title, start, range } => {
                    let path = format!("{:04}/{:02}/{}.md", start.year(), start.month(), start.format("%Y-%m-%d"));
                    if range.is_empty() {
                        out.push_str(&format!("- [{title}]({path})\n"));
                    } else {
                        out.push_str(&format!("- [{title}]({path}): {range}\n"));
                    }
                }
                IndexItem::Other(s) => {
                    out.push_str(s);
                    out.push('\n');
                }
            }
        }
        out
    }

    /// Adaugă un rând și ține indexul în ordinea zilelor (rândurile scrise de mână rămân la locul lor).
    pub fn add(&mut self, title: &str, start: NaiveDate, end: Option<NaiveDate>, lang: Lang) {
        let item = IndexItem::Entry { title: title.trim().to_string(), start, range: range_text(lang, start, end) };
        let at = self
            .items
            .iter()
            .position(|it| matches!(it, IndexItem::Entry { start: s, .. } if *s > start))
            .unwrap_or(self.items.len());
        self.items.insert(at, item);
    }
}

impl Journal {
    pub fn index_path(&self) -> std::path::PathBuf {
        self.root().join("index.md")
    }

    pub fn load_index(&self) -> io::Result<Index> {
        match fs::read_to_string(self.index_path()) {
            Ok(t) => Ok(Index::parse(&t)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Index::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save_index(&self, idx: &Index) -> io::Result<()> {
        fs::create_dir_all(self.root())?;
        let path = self.index_path();
        let tmp = path.with_extension("md.tmp");
        fs::write(&tmp, idx.to_markdown())?;
        fs::rename(&tmp, &path)
    }

    /// Etichetele zilelor, fiecare cu zilele ei (cele mai noi primele).
    pub fn tags_index(&self) -> BTreeMap<String, Vec<NaiveDate>> {
        let mut out: BTreeMap<String, Vec<NaiveDate>> = BTreeMap::new();
        for r in self.pages() {
            let crate::PageRef::Day(d) = r else { continue };
            let Ok(day) = self.load(d) else { continue };
            for t in day.tags {
                out.entry(t).or_default().push(d);
            }
        }
        for days in out.values_mut() {
            days.sort_unstable_by(|a, b| b.cmp(a));
        }
        out
    }

    /// Lunile care au zile scrise, cu câte zile (cele mai noi primele).
    pub fn months_written(&self) -> Vec<((i32, u32), usize)> {
        let mut out: BTreeMap<(i32, u32), usize> = BTreeMap::new();
        for r in self.pages() {
            let crate::PageRef::Day(d) = r else { continue };
            if self.load(d).is_ok_and(|day| !day.is_empty()) {
                *out.entry((d.year(), d.month())).or_default() += 1;
            }
        }
        out.into_iter().rev().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, z: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, z).unwrap()
    }

    #[test]
    fn citit_scris_adaugat() {
        let text = "# Index\n\n- [Concediu la mare](2026/07/2026-07-12.md): 12-19 iulie 2026\nO notă scrisă de mână\n";
        let mut idx = Index::parse(text);
        assert_eq!(idx.to_markdown(), text);
        idx.add("Ziua lui Andrei", d(2026, 11, 30), None, Lang::Ro);
        idx.add("Revelion", d(2026, 12, 31), Some(d(2027, 1, 1)), Lang::Ro);
        idx.add("Paște", d(2026, 4, 12), Some(d(2026, 4, 13)), Lang::Ro);
        let out = idx.to_markdown();
        assert!(out.starts_with("# Index\n\n- [Paște](2026/04/2026-04-12.md): 12-13 aprilie 2026\n- [Concediu la mare]"));
        assert!(out.contains("- [Revelion](2026/12/2026-12-31.md): 31 decembrie 2026 - 1 ianuarie 2027\n"));
        assert!(out.contains("O notă scrisă de mână"));
        assert_eq!(range_text(Lang::Ro, d(2026, 11, 30), Some(d(2026, 12, 2))), "30 noiembrie - 2 decembrie 2026");
    }

    #[test]
    fn etichete_si_luni() {
        let root = std::env::temp_dir().join(format!("lanruj-index-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("2026/10")).unwrap();
        fs::create_dir_all(root.join("2026/11")).unwrap();
        fs::write(root.join("2026/10/2026-10-05.md"), "# x\n\n- Ceva\n\nEtichete: carte\n").unwrap();
        fs::write(root.join("2026/11/2026-11-02.md"), "# x\n\n- Altceva\n\nEtichete: carte, serviciu\n").unwrap();
        fs::write(root.join("2026/11/2026-11-03.md"), "# x\n\n- Încă\n").unwrap();
        let j = Journal::new(&root);
        let t = j.tags_index();
        assert_eq!(t["carte"], vec![d(2026, 11, 2), d(2026, 10, 5)]);
        assert_eq!(t["serviciu"], vec![d(2026, 11, 2)]);
        assert_eq!(j.months_written(), vec![((2026, 11), 2), ((2026, 10), 1)]);
        fs::remove_dir_all(root).unwrap();
    }
}
