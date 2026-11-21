// SPDX-License-Identifier: GPL-3.0-or-later
//! Indexul: partea scrisă de mână (`index.md`) și partea calculată din jurnal
//! (etichetele cu zilele lor, lunile cu zilele scrise).
//!
//! ```text
//! # Index
//!
//! - [Concediu la mare](pagini/2026-07-12.md): [12-19 iulie 2026](2026/07/2026-07-12.md)
//! - [Ziua lui Andrei](2026/11/2026-11-30.md): 30 noiembrie 2026
//! ```
//!
//! Titlul duce la pagina rândului (`pagini/…`, ca o colecție din caiet), zilele la prima zi.
//! Un rând fără pagină (al doilea) o primește la primul clic. Sunt linkuri Markdown obișnuite,
//! deci merg și în alte editoare.

use crate::journal::Journal;
use crate::lang::Lang;
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;
#[cfg(test)]
use std::fs;
use std::io;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexItem {
    /// `page`: pagina rândului, dacă are una: ziua și numărul din numele fișierului
    /// (`pagini/2026-07-12.md` = (12 iulie, 1), `…-2.md` = (12 iulie, 2)).
    Entry { title: String, start: NaiveDate, end: Option<NaiveDate>, page: Option<(NaiveDate, u8)> },
    /// Un rând pe care nu-l înțelegem; se păstrează.
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Index {
    pub heading: Option<String>,
    pub items: Vec<IndexItem>,
}

/// `[text](cale)` de la începutul lui `s` → (text, cale, restul).
fn link(s: &str) -> Option<(&str, &str, &str)> {
    let rest = s.strip_prefix('[')?;
    let close = rest.find("](")?;
    let after = &rest[close + 2..];
    let end = after.find(')')?;
    Some((&rest[..close], &after[..end], &after[end + 1..]))
}

/// `pagini/2026-07-12.md` → (2026-07-12, 1); `pagini/2026-07-12-2.md` → (2026-07-12, 2).
fn page_of(path: &str) -> Option<(NaiveDate, u8)> {
    let stem = path.strip_prefix("pagini/")?.strip_suffix(".md")?;
    if let Ok(d) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
        return Some((d, 1));
    }
    let (date, n) = stem.rsplit_once('-')?;
    Some((NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?, n.parse().ok()?))
}

/// Numele unui fișier din `pagini/` → (ziua, numărul paginii).
pub fn topic_of_file(name: &str) -> Option<(NaiveDate, u8)> {
    page_of(&format!("pagini/{name}"))
}

fn day_of(path: &str) -> Option<NaiveDate> {
    let stem = path.rsplit('/').next()?.strip_suffix(".md")?;
    NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()
}

fn parse_line(line: &str) -> Option<IndexItem> {
    let rest = line.strip_prefix("- ")?;
    let (title, path, after) = link(rest)?;
    let after = after.trim_start_matches(':').trim();
    if let Some((date, n)) = page_of(path) {
        // Cu pagină: zilele sunt al doilea link (sau text simplu).
        let (range, start) = match link(after) {
            Some((r, p, _)) => (r, day_of(p).unwrap_or(date)),
            None => (after, date),
        };
        let end = parse_range(range).and_then(|(_, e)| e);
        return Some(IndexItem::Entry { title: title.to_string(), start, end, page: Some((date, n)) });
    }
    let start = day_of(path)?;
    let end = parse_range(after).and_then(|(_, e)| e);
    Some(IndexItem::Entry { title: title.to_string(), start, end, page: None })
}

fn month_number(name: &str) -> Option<u32> {
    let lang_months = |l: Lang| (1..=12).find(|m| l.month(*m).eq_ignore_ascii_case(name));
    lang_months(Lang::Ro).or_else(|| lang_months(Lang::En))
}

/// Citește zilele scrise de `range_text` (în română sau engleză): (început, sfârșit).
pub fn parse_range(text: &str) -> Option<(NaiveDate, Option<NaiveDate>)> {
    let date = |d: &str, m: &str, y: &str| NaiveDate::from_ymd_opt(y.parse().ok()?, month_number(m)?, d.parse().ok()?);
    let w: Vec<&str> = text.split_whitespace().collect();
    match w.as_slice() {
        [d, m, y] => {
            if let Some((a, b)) = d.split_once('-') {
                Some((date(a, m, y)?, Some(date(b, m, y)?)))
            } else {
                Some((date(d, m, y)?, None))
            }
        }
        [d1, m1, "-", d2, m2, y] => Some((date(d1, m1, y)?, Some(date(d2, m2, y)?))),
        [d1, m1, y1, "-", d2, m2, y2] => Some((date(d1, m1, y1)?, Some(date(d2, m2, y2)?))),
        _ => None,
    }
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

    pub fn to_markdown(&self, lang: Lang) -> String {
        let mut out = format!("# {}\n\n", self.heading.as_deref().unwrap_or("Index"));
        for it in &self.items {
            match it {
                IndexItem::Entry { title, start, end, page } => {
                    let day = format!("{:04}/{:02}/{}.md", start.year(), start.month(), start.format("%Y-%m-%d"));
                    let range = range_text(lang, *start, *end);
                    match page {
                        Some((pd, n)) => out.push_str(&format!("- [{title}]({}): [{range}]({day})\n", topic_rel_path(*pd, *n))),
                        None => out.push_str(&format!("- [{title}]({day}): {range}\n")),
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
    pub fn add(&mut self, title: &str, start: NaiveDate, end: Option<NaiveDate>, page: Option<(NaiveDate, u8)>) {
        let item = IndexItem::Entry { title: title.trim().to_string(), start, end: end.filter(|e| *e > start), page };
        let at = self
            .items
            .iter()
            .position(|it| matches!(it, IndexItem::Entry { start: s, .. } if *s > start))
            .unwrap_or(self.items.len());
        self.items.insert(at, item);
    }
}

/// `pagini/2026-07-12.md` sau `pagini/2026-07-12-2.md`.
pub fn topic_rel_path(date: NaiveDate, n: u8) -> String {
    if n <= 1 { format!("pagini/{}.md", date.format("%Y-%m-%d")) } else { format!("pagini/{}-{n}.md", date.format("%Y-%m-%d")) }
}

/// Ziua de la începutul unui rând din pagina unei teme: „26.11 …”, „26.11.2026 …” sau „26 …”
/// (doar ziua, în intervalul temei). Întoarce ziua și restul textului.
pub fn entry_date(text: &str, start: NaiveDate, end: Option<NaiveDate>) -> Option<(NaiveDate, &str)> {
    let cut = text.find(' ').unwrap_or(text.len());
    let (head, rest) = (&text[..cut], text[cut..].trim_start());
    let parts: Vec<&str> = head.trim_end_matches('.').split('.').collect();
    let num = |s: &str| s.parse::<u32>().ok();
    let end = end.unwrap_or(start);
    let d = match parts.as_slice() {
        [d, m, y] => NaiveDate::from_ymd_opt(y.parse().ok()?, num(m)?, num(d)?)?,
        [d, m] => {
            let (d, m) = (num(d)?, num(m)?);
            // Anul: al începutului; dacă luna a trecut deja, anul următor (concediu peste Revelion).
            let y = if m < start.month() && end.year() > start.year() { end.year() } else { start.year() };
            NaiveDate::from_ymd_opt(y, m, d)?
        }
        [d] => {
            let d = num(d)?;
            // Doar ziua: în luna începutului sau, dacă iese înainte de început, în luna sfârșitului.
            match NaiveDate::from_ymd_opt(start.year(), start.month(), d) {
                Some(x) if x >= start => x,
                _ => NaiveDate::from_ymd_opt(end.year(), end.month(), d).filter(|x| *x >= start && *x <= end)?,
            }
        }
        _ => return None,
    };
    Some((d, rest))
}

/// O temă din index care are ceva de spus despre o zi: e în intervalul ei sau are rânduri cu ziua aceea.
#[derive(Clone, Debug)]
pub struct TopicOnDay {
    /// Poziția rândului în index.
    pub index: usize,
    pub title: String,
    pub start: NaiveDate,
    pub end: Option<NaiveDate>,
    /// A câta zi din interval (1 = prima) și câte sunt, dacă ziua e în interval.
    pub day_of: Option<(i64, i64)>,
    /// Rândurile din pagina temei care încep cu ziua aceasta: (poziția în pagină, intrarea fără dată).
    pub entries: Vec<(usize, crate::day::Entry)>,
}

impl Journal {
    /// Temele din index care privesc ziua `d` (pentru pagina zilei, a săptămânii și a lunii).
    pub fn topics_on(&self, d: NaiveDate) -> Vec<TopicOnDay> {
        let Ok(idx) = self.load_index() else { return Vec::new() };
        let mut out = Vec::new();
        for (i, it) in idx.items.iter().enumerate() {
            let IndexItem::Entry { title, start, end, page } = it else { continue };
            let last = end.unwrap_or(*start);
            let day_of = (d >= *start && d <= last).then(|| ((d - *start).num_days() + 1, (last - *start).num_days() + 1));
            let mut entries = Vec::new();
            if let Some((pd, n)) = page
                && let Ok(p) = self.load_ref(crate::PageRef::Topic { date: *pd, n: *n })
            {
                for (pos, item) in p.items.iter().enumerate() {
                    let crate::day::Item::Entry(e) = item else { continue };
                    if let Some((ed, rest)) = entry_date(&e.text, *start, *end)
                        && ed == d
                    {
                        let mut e2 = e.clone();
                        e2.text = rest.to_string();
                        entries.push((pos, e2));
                    }
                }
            }
            if day_of.is_some() || !entries.is_empty() {
                out.push(TopicOnDay { index: i, title: title.clone(), start: *start, end: *end, day_of, entries });
            }
        }
        out
    }

    pub fn topic_path(&self, date: NaiveDate, n: u8) -> std::path::PathBuf {
        self.root().join(topic_rel_path(date, n))
    }

    /// Creează pagina unui rând din index (cu titlul ca antet); întoarce numărul ei.
    pub fn create_topic(&self, title: &str, date: NaiveDate) -> io::Result<u8> {
        let n = (1..=99u8).find(|n| !self.topic_path(date, *n).exists()).unwrap_or(99);
        let path = self.topic_path(date, n);
        self.write_text(&path, &format!("# {}\n", title.trim()))?;
        Ok(n)
    }

    /// Schimbă titlul din antetul paginii unui rând.
    pub fn retitle_topic(&self, date: NaiveDate, n: u8, title: &str) -> io::Result<()> {
        let path = self.topic_path(date, n);
        let text = self.read_text(&path).unwrap_or_default();
        let mut day = crate::day::Day::parse(date, &text);
        day.heading = Some(title.trim().to_string());
        self.write_text(&path, &day.to_markdown(Lang::Ro))
    }

    pub fn index_path(&self) -> std::path::PathBuf {
        self.root().join("index.md")
    }

    pub fn load_index(&self) -> io::Result<Index> {
        match self.read_text(&self.index_path()) {
            Ok(t) => Ok(Index::parse(&t)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Index::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save_index(&self, idx: &Index, lang: Lang) -> io::Result<()> {
        self.write_text(&self.index_path(), &idx.to_markdown(lang))
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
        let text = "# Index\n\n- [Concediu la mare](2026/07/2026-07-12.md): 12-19 iulie 2026\n- [Munte](pagini/2026-08-02.md): [2-9 august 2026](2026/08/2026-08-02.md)\nO notă scrisă de mână\n";
        let mut idx = Index::parse(text);
        assert_eq!(idx.to_markdown(Lang::Ro), text);
        assert_eq!(idx.items[0], IndexItem::Entry { title: "Concediu la mare".into(), start: d(2026, 7, 12), end: Some(d(2026, 7, 19)), page: None });
        assert_eq!(idx.items[1], IndexItem::Entry { title: "Munte".into(), start: d(2026, 8, 2), end: Some(d(2026, 8, 9)), page: Some((d(2026, 8, 2), 1)) });
        idx.add("Ziua lui Andrei", d(2026, 11, 30), None, None);
        idx.add("Revelion", d(2026, 12, 31), Some(d(2027, 1, 1)), Some((d(2026, 12, 31), 2)));
        idx.add("Paște", d(2026, 4, 12), Some(d(2026, 4, 13)), None);
        let out = idx.to_markdown(Lang::Ro);
        assert!(out.starts_with("# Index\n\n- [Paște](2026/04/2026-04-12.md): 12-13 aprilie 2026\n- [Concediu la mare]"));
        assert!(out.contains("- [Revelion](pagini/2026-12-31-2.md): [31 decembrie 2026 - 1 ianuarie 2027](2026/12/2026-12-31.md)\n"));
        assert_eq!(Index::parse(&out), idx, "rescris și citit la fel");
        assert_eq!(parse_range("30 noiembrie - 2 decembrie 2026"), Some((d(2026, 11, 30), Some(d(2026, 12, 2)))));
        assert_eq!(parse_range("12 July 2026"), Some((d(2026, 7, 12), None)));
        assert!(out.contains("O notă scrisă de mână"));
        assert_eq!(range_text(Lang::Ro, d(2026, 11, 30), Some(d(2026, 12, 2))), "30 noiembrie - 2 decembrie 2026");
    }

    #[test]
    fn pagina_unui_rand() {
        let root = std::env::temp_dir().join(format!("lanruj-tema-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let j = Journal::new(&root);
        let n1 = j.create_topic("Concediu", d(2026, 11, 23)).unwrap();
        let n2 = j.create_topic("Altceva", d(2026, 11, 23)).unwrap();
        assert_eq!((n1, n2), (1, 2));
        let r = crate::PageRef::Topic { date: d(2026, 11, 23), n: 1 };
        j.update_ref(r, Lang::Ro, |p| p.push(crate::day::parse_quick("- Cazare la Hotel Delta").unwrap())).unwrap();
        assert_eq!(fs::read_to_string(root.join("pagini/2026-11-23.md")).unwrap(), "# Concediu\n\n- Cazare la Hotel Delta\n");
        j.retitle_topic(d(2026, 11, 23), 1, "Concediu la mare").unwrap();
        assert!(fs::read_to_string(root.join("pagini/2026-11-23.md")).unwrap().starts_with("# Concediu la mare\n"));
        assert!(j.pages().contains(&crate::PageRef::Topic { date: d(2026, 11, 23), n: 2 }));
        let found = j.search("hotel", crate::search::Filter::default(), 10);
        assert_eq!(found[0].0, r);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn temele_unei_zile() {
        let s = d(2026, 11, 23);
        let e = Some(d(2026, 11, 30));
        assert_eq!(entry_date("26.11 Vizitare Delfinariu", s, e), Some((d(2026, 11, 26), "Vizitare Delfinariu")));
        assert_eq!(entry_date("26.11.2026 Delfinariu", s, e), Some((d(2026, 11, 26), "Delfinariu")));
        assert_eq!(entry_date("24 Plecare", s, e), Some((d(2026, 11, 24), "Plecare")));
        assert_eq!(entry_date("Oprire la Bușteni", s, e), None);
        assert_eq!(entry_date("2.01 Întoarcere", d(2026, 12, 28), Some(d(2027, 1, 3))), Some((d(2027, 1, 2), "Întoarcere")));

        let root = std::env::temp_dir().join(format!("lanruj-teme-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let j = Journal::new(&root);
        let n = j.create_topic("Concediu", s).unwrap();
        let r = crate::PageRef::Topic { date: s, n };
        for t in ["- Oprire peste noapte la Bușteni", "- [ ] 26.11 Vizitare Delfinariu", "- [ ] 23.11 Plecare spre Bușteni"] {
            j.update_ref(r, Lang::Ro, |p| p.push(crate::day::parse_quick(t).unwrap())).unwrap();
        }
        let mut idx = Index::default();
        idx.add("Concediu", s, e, Some((s, n)));
        j.save_index(&idx, Lang::Ro).unwrap();
        let t23 = j.topics_on(s);
        assert_eq!(t23.len(), 1);
        assert_eq!(t23[0].day_of, Some((1, 8)));
        assert_eq!(t23[0].entries.len(), 1);
        assert_eq!(t23[0].entries[0].1.text, "Plecare spre Bușteni");
        assert_eq!(j.topics_on(d(2026, 11, 26))[0].entries[0].1.text, "Vizitare Delfinariu");
        assert!(j.topics_on(d(2026, 12, 1)).is_empty());
        fs::remove_dir_all(root).unwrap();
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
