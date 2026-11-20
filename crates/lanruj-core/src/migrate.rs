// SPDX-License-Identifier: GPL-3.0-or-later
//! Migrarea: trierea sarcinilor neterminate ale unei zile.
//!
//! - mutată pe mâine: `[>]` în ziua ei, sarcină nouă în ziua următoare;
//! - programată pe altă zi: `[<]` în ziua ei, sarcină nouă în ziua aleasă;
//! - renunțată: `[-]`;
//! - lăsată: rămâne `[ ]`.
//!
//! Sarcina mutată și copia ei primesc același semn `^k3f9`, ca să se poată
//! număra mutările (↻N) și ca o alegere să poată fi anulată.

use crate::day::{Day, Entry, Item, Kind, TaskState};
use crate::journal::Journal;
use crate::lang::Lang;
use chrono::NaiveDate;
use std::hash::{BuildHasher, Hasher};
use std::io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Mutată sau programată pe ziua aceasta.
    Moved(NaiveDate),
    Dropped,
    Kept,
}

/// Sarcinile neterminate de pe primul nivel, în ordinea din pagină.
pub fn candidates(day: &Day) -> Vec<usize> {
    day.items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| match it {
            Item::Entry(e) if e.depth == 0 && e.is_open_task() => Some(i),
            _ => None,
        })
        .collect()
}

/// Intrările de sub o intrare (notițele ei), până la următoarea de același nivel.
fn children(day: &Day, index: usize) -> Vec<Entry> {
    let Some(Item::Entry(parent)) = day.items.get(index) else { return Vec::new() };
    let mut out = Vec::new();
    for it in &day.items[index + 1..] {
        match it {
            Item::Entry(e) if e.depth > parent.depth => {
                let mut c = e.clone();
                c.depth -= parent.depth;
                out.push(c);
            }
            _ => break,
        }
    }
    out
}

fn new_id(taken: &[&Day]) -> String {
    const ABC: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let state = std::collections::hash_map::RandomState::new();
    for attempt in 0u64.. {
        let mut h = state.build_hasher();
        h.write_u64(attempt);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        let mut n = h.finish();
        let mut id = String::with_capacity(4);
        for _ in 0..4 {
            id.push(ABC[(n % 36) as usize] as char);
            n /= 36;
        }
        // Să nu înceapă cu o cifră, ca să nu semene cu o dată sau o oră.
        if id.as_bytes()[0].is_ascii_digit() {
            continue;
        }
        if !taken.iter().any(|d| d.entries().any(|e| e.id.as_deref() == Some(id.as_str()))) {
            return id;
        }
    }
    unreachable!()
}

fn not_a_task() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "intrarea nu e o sarcină neterminată")
}

impl Journal {
    /// Mută sarcina de pe poziția `index` din ziua `from` în ziua `to`
    /// (`[>]` dacă `to` e ziua următoare, `[<]` altfel). Întoarce semnul ei.
    pub fn migrate(&self, from: NaiveDate, index: usize, to: NaiveDate, lang: Lang) -> io::Result<String> {
        if to <= from {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "ziua aleasă trebuie să fie după ziua sarcinii"));
        }
        let mut source = self.load(from)?;
        let mut target = self.load(to)?;
        let entry = match source.items.get(index) {
            Some(Item::Entry(e)) if e.is_open_task() => e.clone(),
            _ => return Err(not_a_task()),
        };
        let id = entry.id.clone().unwrap_or_else(|| new_id(&[&source, &target]));

        let mut copy = entry.clone();
        copy.depth = 0;
        copy.id = Some(id.clone());
        let kids = children(&source, index);
        target.push(copy);
        for k in kids {
            target.push(k);
        }

        let e = source.entry_mut(index).expect("verificat mai sus");
        e.id = Some(id.clone());
        let next_day = from.succ_opt() == Some(to);
        e.kind = Kind::Task(if next_day { TaskState::Migrated } else { TaskState::Scheduled });

        // Întâi ziua nouă: dacă se întrerupe ceva, sarcina apare de două ori, nu deloc.
        self.save(&target, lang)?;
        self.save(&source, lang)?;
        Ok(id)
    }

    /// Renunță la sarcina de pe poziția `index` (o taie).
    pub fn drop_task(&self, from: NaiveDate, index: usize, lang: Lang) -> io::Result<()> {
        let mut day = self.load(from)?;
        match day.entry_mut(index) {
            Some(e) if e.is_open_task() => e.kind = Kind::Task(TaskState::Dropped),
            _ => return Err(not_a_task()),
        }
        self.save(&day, lang)
    }

    /// Anulează alegerea făcută pentru sarcina de pe poziția `index`:
    /// o face din nou neterminată și scoate copia din ziua în care fusese mutată.
    /// Nu anulează dacă acea copie a fost între timp bifată sau schimbată.
    pub fn reopen(&self, from: NaiveDate, index: usize, lang: Lang) -> io::Result<()> {
        let mut source = self.load(from)?;
        let entry = match source.items.get(index) {
            Some(Item::Entry(e)) => e.clone(),
            _ => return Err(not_a_task()),
        };
        match entry.kind {
            Kind::Task(TaskState::Dropped) => {}
            Kind::Task(TaskState::Migrated) | Kind::Task(TaskState::Scheduled) => {
                if let Some(id) = &entry.id {
                    self.remove_copy(from, id, lang)?;
                }
            }
            Kind::Task(TaskState::Open) => return Ok(()),
            _ => return Err(not_a_task()),
        }
        source.entry_mut(index).expect("verificat").kind = Kind::Task(TaskState::Open);
        self.save(&source, lang)
    }

    /// Scoate, din prima zi de după `from`, sarcina neterminată cu semnul `id` și notițele ei.
    fn remove_copy(&self, from: NaiveDate, id: &str, lang: Lang) -> io::Result<()> {
        let mut dates: Vec<NaiveDate> = self
            .day_files()
            .iter()
            .filter_map(|p| p.file_stem()?.to_str().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()))
            .filter(|d| *d > from)
            .collect();
        dates.sort_unstable();
        for d in dates {
            let mut day = self.load(d)?;
            let found = day.items.iter().position(|it| matches!(it, Item::Entry(e) if e.id.as_deref() == Some(id)));
            let Some(pos) = found else { continue };
            let Some(Item::Entry(e)) = day.items.get(pos) else { continue };
            if !e.is_open_task() {
                return Err(io::Error::other("sarcina a fost deja bifată sau mutată mai departe; alegerea nu se mai poate anula"));
            }
            let n = 1 + children(&day, pos).len();
            day.items.drain(pos..pos + n);
            return self.save(&day, lang);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("lanruj-mig-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }

    const ZI: &str = "# Vineri, 20 noiembrie 2026

- o 18:30 Cină
- [ ] Becuri
  - LED, lumină caldă
- [ ] * Raport
- [x] Plimbare
- [ ] Lampă
";

    fn setup(name: &str) -> (Journal, PathBuf) {
        let r = root(name);
        fs::create_dir_all(r.join("2026/11")).unwrap();
        fs::write(r.join("2026/11/2026-11-20.md"), ZI).unwrap();
        (Journal::new(&r), r)
    }

    #[test]
    fn candidatii() {
        let day = Day::parse(d(20), ZI);
        assert_eq!(candidates(&day), vec![1, 3, 5]);
    }

    #[test]
    fn muta_pe_maine_cu_notitele() {
        let (j, r) = setup("maine");
        let id = j.migrate(d(20), 1, d(21), Lang::Ro).unwrap();
        let src = fs::read_to_string(r.join("2026/11/2026-11-20.md")).unwrap();
        assert!(src.contains(&format!("- [>] Becuri ^{id}\n  - LED, lumină caldă\n")));
        let dst = fs::read_to_string(r.join("2026/11/2026-11-21.md")).unwrap();
        assert_eq!(dst, format!("# Sâmbătă, 21 noiembrie 2026\n\n- [ ] Becuri ^{id}\n  - LED, lumină caldă\n"));
        assert_eq!(j.migration_count(&id), 1);

        // A doua zi, aceeași sarcină mutată din nou: același semn, ↻2.
        let day21 = j.load(d(21)).unwrap();
        assert_eq!(candidates(&day21), vec![0]);
        let id2 = j.migrate(d(21), 0, d(22), Lang::Ro).unwrap();
        assert_eq!(id, id2);
        assert_eq!(j.migration_count(&id), 2);
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn programeaza_renunta_anuleaza() {
        let (j, r) = setup("prog");
        let id = j.migrate(d(20), 3, d(27), Lang::Ro).unwrap();
        let day = j.load(d(20)).unwrap();
        assert_eq!(day.entries().nth(3).unwrap().kind, Kind::Task(TaskState::Scheduled));
        assert!(fs::read_to_string(r.join("2026/11/2026-11-27.md")).unwrap().contains(&format!("- [ ] * Raport ^{id}")));

        j.drop_task(d(20), 5, Lang::Ro).unwrap();
        assert_eq!(j.load(d(20)).unwrap().entries().nth(5).unwrap().kind, Kind::Task(TaskState::Dropped));

        // Anularea: sarcina redevine neterminată, copia dispare.
        j.reopen(d(20), 3, Lang::Ro).unwrap();
        j.reopen(d(20), 5, Lang::Ro).unwrap();
        let after = fs::read_to_string(r.join("2026/11/2026-11-20.md")).unwrap();
        assert!(after.contains(&format!("- [ ] * Raport ^{id}\n")));
        assert!(after.contains("- [ ] Lampă\n"));
        assert!(j.load(d(27)).unwrap().is_empty());
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn nu_anuleaza_ce_s_a_bifat_intre_timp() {
        let (j, r) = setup("bifat");
        j.migrate(d(20), 1, d(21), Lang::Ro).unwrap();
        j.toggle_done(d(21), 0, Lang::Ro).unwrap();
        assert!(j.reopen(d(20), 1, Lang::Ro).is_err());
        assert_eq!(j.load(d(20)).unwrap().entries().nth(1).unwrap().kind, Kind::Task(TaskState::Migrated));
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn refuza_ce_nu_e_sarcina_sau_zi_gresita() {
        let (j, r) = setup("refuz");
        assert!(j.migrate(d(20), 0, d(21), Lang::Ro).is_err());
        assert!(j.migrate(d(20), 1, d(20), Lang::Ro).is_err());
        assert!(j.drop_task(d(20), 4, Lang::Ro).is_err());
        fs::remove_dir_all(r).unwrap();
    }
}
