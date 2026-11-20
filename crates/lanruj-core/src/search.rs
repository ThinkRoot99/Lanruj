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
    }
}

impl Journal {
    /// Intrările care conțin toate cuvintele căutate, grupate pe pagini, cele mai noi primele.
    /// Fără cuvinte, dar cu un filtru ales, întoarce toate intrările de acel fel.
    pub fn search(&self, query: &str, filter: Filter, limit: usize) -> Vec<(PageRef, Vec<(usize, Entry)>)> {
        let words: Vec<String> = fold(query).split_whitespace().map(str::to_string).collect();
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
                if words.iter().all(|w| text.contains(w.as_str())) {
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
                    if words.iter().all(|w| f.contains(w.as_str())) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::fs;

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
