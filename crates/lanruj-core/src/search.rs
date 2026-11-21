// SPDX-License-Identifier: GPL-3.0-or-later
//! Căutarea în tot jurnalul: zile, listele lunilor, planurile săptămânilor, Planuri.
//! Nu ține cont de literele mari și de diacritice („sedinta” găsește „Ședință”).

use crate::PageRef;
use crate::day::{Entry, Item, Kind, Priority};
use crate::journal::Journal;
use chrono::Datelike;

/// Ce fel de intrări se caută. Niciun filtru ales = toate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub tasks: bool,
    pub events: bool,
    pub notes: bool,
    pub important: bool,
    pub ideas: bool,
}

impl Filter {
    fn accepts(&self, e: &Entry) -> bool {
        let kinds = self.tasks || self.events || self.notes;
        let kind_ok = !kinds
            || (self.tasks && matches!(e.kind, Kind::Task(_)))
            || (self.events && e.kind == Kind::Event)
            || (self.notes && e.kind == Kind::Note);
        let marks = self.important || self.ideas;
        let mark_ok = !marks
            || (self.important && e.priority == Priority::Important)
            || (self.ideas && e.priority == Priority::Inspiration);
        kind_ok && mark_ok
    }
}

/// Literele mici, fără diacritice: „Ședință” → „sedinta”.
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'ă' | 'â' | 'á' | 'à' | 'ä' => 'a',
            'î' | 'í' | 'ï' => 'i',
            'ș' | 'ş' => 's',
            'ț' | 'ţ' => 't',
            'é' | 'è' | 'ë' => 'e',
            'ó' | 'ö' | 'ő' => 'o',
            'ú' | 'ü' | 'ű' => 'u',
            c => c,
        })
        .collect()
}

/// Ordinea în caiet, pentru sortare: (an, lună, zi); listele lunilor și Planurile au ziua 0.
fn key(r: PageRef) -> (i32, u32, u32) {
    match r {
        PageRef::Day(d) => (d.year(), d.month(), d.day()),
        PageRef::Month { year, month } | PageRef::Future { year, month } => (year, month, 0),
        PageRef::Week { year, week } => {
            let d = crate::week_monday(year, week);
            (d.year(), d.month(), d.day())
        }
        PageRef::Topic { date, .. } => (date.year(), date.month(), date.day()),
    }
}

impl Journal {
    /// Intrările care conțin toate cuvintele căutate, grupate pe pagini, cele mai noi primele.
    /// Fără cuvinte, dar cu un filtru ales, întoarce toate intrările de acel fel.
    pub fn search(&self, query: &str, filter: Filter, limit: usize) -> Vec<(PageRef, Vec<(usize, Entry)>)> {
        let q = Query::parse(query);
        let words = q.must.clone();
        if words.is_empty() && filter == Filter::default() {
            return Vec::new();
        }
        let mut pages = self.pages();
        pages.sort_by_key(|r| std::cmp::Reverse(key(*r)));
        pages.dedup();
        let mut out = Vec::new();
        let mut count = 0;
        for r in pages {
            let Ok(day) = self.load_ref(r) else { continue };
            let mut hits = Vec::new();
            for (i, it) in day.items.iter().enumerate() {
                let Item::Entry(e) = it else { continue };
                if !filter.accepts(e) {
                    continue;
                }
                let text = fold(&format!("{} {}", e.time.as_deref().unwrap_or(""), crate::colors::plain(&e.text)));
                if q.matches(&text) {
                    hits.push((i, e.clone()));
                    count += 1;
                }
            }
            // Etichetele zilei și rândurile din calendarul lunii se caută și ele (ca notițe).
            let as_note = filter == Filter::default() || (filter.notes && !filter.important && !filter.ideas);
            if as_note && !words.is_empty() {
                let mut extra = Vec::new();
                match r {
                    PageRef::Day(_) if !day.tags.is_empty() => extra.push(format!("Etichete: {}", day.tags.join(", "))),
                    PageRef::Month { year, month } => {
                        if let Ok(m) = self.load_month(year, month) {
                            for c in &m.calendar {
                                if let crate::month::CalLine::Day(n, t) = c {
                                    extra.push(format!("{n} {}", crate::colors::plain(t)));
                                }
                            }
                        }
                    }
                    _ => {}
                }
                for t in extra {
                    let f = fold(&t);
                    if q.matches(&f) {
                        hits.push((usize::MAX, Entry::new(Kind::Note, t)));
                        count += 1;
                    }
                }
            }
            if !hits.is_empty() {
                out.push((r, hits));
            }
            if count >= limit {
                break;
            }
        }
        out
    }
}

/// O căutare: cuvinte (toate trebuie să apară), „expresii între ghilimele” (exact așa)
/// și -cuvinte care nu trebuie să apară. Fără litere mari și diacritice.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub must: Vec<String>,
    pub not: Vec<String>,
}

impl Query {
    pub fn parse(q: &str) -> Query {
        let f = fold(q);
        let mut out = Query::default();
        let mut cur = String::new();
        let mut quoted = false;
        let flush = |cur: &mut String, out: &mut Query, quoted: bool| {
            let t = std::mem::take(cur);
            let t = t.trim();
            if t.is_empty() {
                return;
            }
            match t.strip_prefix('-') {
                Some(n) if !quoted && !n.is_empty() => out.not.push(n.to_string()),
                _ => out.must.push(t.to_string()),
            }
        };
        for c in f.chars() {
            match c {
                '"' | '„' | '“' | '”' | '«' | '»' => {
                    flush(&mut cur, &mut out, quoted);
                    quoted = !quoted;
                }
                c if c.is_whitespace() && !quoted => flush(&mut cur, &mut out, quoted),
                c => cur.push(c),
            }
        }
        flush(&mut cur, &mut out, quoted);
        out
    }

    pub fn is_empty(&self) -> bool {
        self.must.is_empty()
    }

    /// `text` trebuie să fie deja „împăturit” (`fold`).
    pub fn matches(&self, text: &str) -> bool {
        self.must.iter().all(|w| text.contains(w.as_str())) && !self.not.iter().any(|w| text.contains(w.as_str()))
    }

    /// Măcar un cuvânt căutat în rând (pentru rândurile arătate dintr-o zi găsită).
    pub fn touches(&self, text: &str) -> bool {
        self.must.iter().any(|w| text.contains(w.as_str()))
    }
}

/// Un rând din Index găsit la căutare: titlul, prima zi și pagina lui, dacă are.
pub type IndexHit = (String, chrono::NaiveDate, Option<(chrono::NaiveDate, u8)>);

/// Un loc din Monitorizare găsit la căutare.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackingHit {
    pub year: i32,
    pub month: u32,
    pub day: Option<u32>,
    /// „activitati”, „somn” sau „serviciu”.
    pub tab: &'static str,
    pub text: String,
}

impl Journal {
    /// Zilele de jurnal în care apar toate cuvintele, cu rândurile care le conțin
    /// (cel mult 4 pe zi; numărul rândului, ca să se deschidă acolo). Fără cuvinte: toate zilele,
    /// cu primul rând scris.
    pub fn search_diary(&self, q: &Query, limit: usize) -> Vec<(chrono::NaiveDate, Vec<(usize, String)>)> {
        let mut out = Vec::new();
        for d in self.diary_dates() {
            let Ok(text) = self.load_diary(d) else { continue };
            let lines: Vec<String> = crate::diary::parse_doc(&text).iter().map(|l| l.text()).collect();
            let all = fold(&lines.join("\n"));
            if !q.is_empty() && !q.matches(&all) {
                continue;
            }
            let shown: Vec<(usize, String)> = if q.is_empty() {
                lines.iter().enumerate().filter(|(_, l)| !l.trim().is_empty()).take(1).map(|(i, l)| (i, l.clone())).collect()
            } else {
                let hits: Vec<(usize, String)> = lines.iter().enumerate().filter(|(_, l)| q.touches(&fold(l))).take(4).map(|(i, l)| (i, l.clone())).collect();
                if hits.is_empty() {
                    // Cuvintele sunt pe rânduri diferite (o expresie peste rânduri nu apare aici).
                    lines.iter().enumerate().filter(|(_, l)| !l.trim().is_empty()).take(1).map(|(i, l)| (i, l.clone())).collect()
                } else {
                    hits
                }
            };
            out.push((d, shown));
            if out.len() >= limit {
                break;
            }
        }
        out
    }

    /// Biletele în care apar toate cuvintele (fără cuvinte: toate).
    pub fn search_notes(&self, q: &Query) -> Vec<(usize, String)> {
        self.load_notes()
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .filter(|(_, n)| q.is_empty() || q.matches(&fold(&n.text)))
            .map(|(i, n)| (i, n.text))
            .collect()
    }

    /// Lunile cu Monitorizare, cele mai noi primele.
    pub fn tracking_months(&self) -> Vec<(i32, u32)> {
        let mut out: Vec<(i32, u32)> = std::fs::read_dir(self.root().join("monitorizare"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                let (y, m) = n.strip_suffix(".md")?.split_once('-')?;
                Some((y.parse().ok()?, m.parse().ok()?))
            })
            .collect();
        out.sort_by(|a, b| b.cmp(a));
        out
    }

    /// Notele de la serviciu, rezervă, concediu și orele suplimentare și numele activităților.
    pub fn search_tracking(&self, q: &Query) -> Vec<TrackingHit> {
        let mut out = Vec::new();
        if q.is_empty() {
            return out;
        }
        for (year, month) in self.tracking_months() {
            let Ok(t) = self.load_tracking(year, month) else { continue };
            for (day, note) in &t.work_notes {
                let kind = match t.work.get(day) {
                    Some(crate::monitor::Shift::Service) => "serviciu service",
                    Some(crate::monitor::Shift::Reserve) => "rezerva reserve",
                    Some(crate::monitor::Shift::Leave) => "concediu leave",
                    None => "",
                };
                let text = format!("{kind} {note}");
                if q.matches(&fold(&text)) {
                    out.push(TrackingHit { year, month, day: Some(*day), tab: "serviciu", text: note.clone() });
                }
            }
            for (day, list) in &t.overtime {
                for o in list {
                    if q.matches(&fold(&o.note)) {
                        out.push(TrackingHit { year, month, day: Some(*day), tab: "serviciu", text: o.note.clone() });
                    }
                }
            }
            for a in &t.activities {
                if q.matches(&fold(&a.name)) && !out.iter().any(|h: &TrackingHit| h.tab == "activitati" && h.text == a.name) {
                    out.push(TrackingHit { year, month, day: None, tab: "activitati", text: a.name.clone() });
                }
            }
        }
        out
    }

    /// Rândurile din Index (titlul și zilele) în care apar toate cuvintele.
    pub fn search_index(&self, q: &Query) -> Vec<IndexHit> {
        if q.is_empty() {
            return Vec::new();
        }
        let Ok(idx) = self.load_index() else { return Vec::new() };
        idx.items
            .into_iter()
            .filter_map(|it| match it {
                crate::index::IndexItem::Entry { title, start, page, .. } if q.matches(&fold(&title)) => Some((title, start, page)),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::fs;

    #[test]
    fn cautarea_cu_expresii_si_excluderi() {
        let q = Query::parse("piață „cu mama” -tata");
        assert_eq!(q.must, vec!["piata", "cu mama"]);
        assert_eq!(q.not, vec!["tata"]);
        assert!(q.matches(&fold("Am fost la Piață cu mama")));
        assert!(!q.matches(&fold("Am fost la piață cu tata și cu mama")));
        assert!(!q.matches(&fold("Piață, mama")));
        let root = std::env::temp_dir().join(format!("lanruj-cauta-j-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let j = Journal::new(&root);
        let d = NaiveDate::from_ymd_opt(2026, 11, 21).unwrap();
        j.save_diary(d, "### 08:00\nO dimineață **liniștită**.\nApoi am fost la piață.", crate::Lang::Ro).unwrap();
        let r = j.search_diary(&Query::parse("liniștită piata"), 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![1, 2]);
        j.update_notes(|n| n.push(crate::bilete::Note::new("Cadou pentru mama"))).unwrap();
        assert_eq!(j.search_notes(&Query::parse("mama")).len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn diacritice_si_litere_mari() {
        assert_eq!(fold("Ședință ÎNTÂRZIATĂ Țară"), "sedinta intarziata tara");
    }

    #[test]
    fn cauta_peste_tot() {
        let root = std::env::temp_dir().join(format!("lanruj-cauta-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("2026/11")).unwrap();
        fs::write(root.join("2026/11/2026-11-18.md"), "# x\n\n- o 10:00 Ședință la serviciu\n- [x] * Trimite raportul\n").unwrap();
        fs::write(root.join("2026/11/2026-11-20.md"), "# x\n\n- [ ] Sună la service\n- ! Idee pentru ședința de luni\n").unwrap();
        fs::write(root.join("2026/11/luna.md"), "# Noiembrie 2026\n\n## Calendar\n\n## Sarcini\n\n- [ ] Revizia la service\n").unwrap();
        let j = Journal::new(&root);
        let d = |z| PageRef::Day(NaiveDate::from_ymd_opt(2026, 11, z).unwrap());

        let r = j.search("sedinta", Filter::default(), 100);
        assert_eq!(r.iter().map(|(p, h)| (*p, h.len())).collect::<Vec<_>>(), vec![(d(20), 1), (d(18), 1)]);

        let r = j.search("SERVICE", Filter::default(), 100);
        assert_eq!(r.len(), 2, "ziua și lista lunii");
        assert!(r.iter().any(|(p, _)| *p == PageRef::Month { year: 2026, month: 11 }));

        let r = j.search("", Filter { important: true, ..Default::default() }, 100);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1[0].1.text, "Trimite raportul");

        let r = j.search("sedinta", Filter { events: true, ..Default::default() }, 100);
        assert_eq!(r.len(), 1);
        fs::write(root.join("2026/11/2026-11-19.md"), "# x\n\n- Ceva\n\nEtichete: carte, sănătate\n").unwrap();
        fs::write(root.join("2026/11/luna.md"), "# Noiembrie 2026\n\n## Calendar\n\n- 25 Oftalmolog\n\n## Sarcini\n\n- [ ] Revizia la service\n").unwrap();
        let r = j.search("sanatate", Filter::default(), 100);
        assert_eq!(r.len(), 1, "eticheta");
        assert_eq!(r[0].1[0].1.text, "Etichete: carte, sănătate");
        let r = j.search("oftalmolog", Filter::default(), 100);
        assert_eq!(r[0].0, PageRef::Month { year: 2026, month: 11 });
        assert_eq!(r[0].1[0].1.text, "25 Oftalmolog");
        let r = j.search("10:00", Filter::default(), 100);
        assert_eq!(r.len(), 1, "și ora se caută");
        assert!(j.search("", Filter::default(), 100).is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
