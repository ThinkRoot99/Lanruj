// SPDX-License-Identifier: GPL-3.0-or-later
//! Migrarea: trierea sarcinilor neterminate ale unei zile.
//!
//! - mutată pe pagina următoare (ziua de mâine, luna următoare): `[>]`;
//! - programată în altă parte (o zi anume, lista unei luni): `[<]`;
//! - renunțată: `[-]`;
//! - lăsată: rămâne `[ ]`.
//!
//! Sarcina mutată și copia ei primesc același semn `^k3f9`, ca să se poată
//! număra mutările (↻N) și ca o alegere să poată fi anulată.

use crate::PageRef;
use crate::day::{Day, Item, Kind, TaskState};
use crate::journal::Journal;
use crate::lang::Lang;
use chrono::Datelike;
use std::hash::{BuildHasher, Hasher};
use std::io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Mutată sau programată pe pagina aceasta.
    Moved(PageRef),
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
/// Ce stă sub o intrare: notițele ei și pozele/fișierele legate de ea (cu nivelul refăcut față de ea).
fn children(day: &Day, index: usize) -> Vec<Item> {
    use crate::attach::{depth_of, parse};
    let Some(Item::Entry(parent)) = day.items.get(index) else { return Vec::new() };
    let mut out = Vec::new();
    for it in &day.items[index + 1..] {
        match it {
            Item::Entry(e) if e.depth > parent.depth => {
                let mut c = e.clone();
                c.depth -= parent.depth;
                out.push(Item::Entry(c));
            }
            Item::Other(l) if depth_of(l) > parent.depth && parse(l).is_some() => {
                let a = parse(l).expect("verificat");
                out.push(Item::Other(a.to_line(depth_of(l) - parent.depth)));
            }
            _ => break,
        }
    }
    out
}

/// Calea relativă `rel` (față de dosarul `from`) scrisă față de dosarul `to`.
pub fn rebase(rel: &str, from: &std::path::Path, to: &std::path::Path) -> String {
    use std::path::Component;
    let abs = from.join(rel);
    let a: Vec<Component> = abs.components().collect();
    let b: Vec<Component> = to.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = vec!["..".to_string(); b.len() - common];
    parts.extend(a[common..].iter().map(|c| c.as_os_str().to_string_lossy().to_string()));
    parts.join("/")
}

/// Pozele și fișierele mutate pe altă pagină: căile lor se refac față de dosarul ei.
fn rebase_items(items: Vec<Item>, from: &std::path::Path, to: &std::path::Path) -> Vec<Item> {
    use crate::attach::{Attachment, depth_of, parse};
    if from == to {
        return items;
    }
    items
        .into_iter()
        .map(|it| match &it {
            Item::Other(l) => match parse(l) {
                Some(Attachment::Image { name, width, path }) => {
                    Item::Other(Attachment::Image { name, width, path: rebase(&path, from, to) }.to_line(depth_of(l)))
                }
                Some(Attachment::File { name, path }) => Item::Other(Attachment::File { name, path: rebase(&path, from, to) }.to_line(depth_of(l))),
                None => it,
            },
            Item::Entry(_) => it,
        })
        .collect()
}

/// Un semn nou, pentru alte module ale nucleului.
pub(crate) fn fresh_id(taken: &[&Day]) -> String {
    new_id(taken)
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

/// Pagina care urmează în caiet: ziua de mâine sau luna următoare.
pub fn next_page(r: PageRef) -> PageRef {
    match r {
        PageRef::Day(d) => PageRef::Day(d.succ_opt().expect("zi validă")),
        PageRef::Month { year, month } => {
            let (year, month) = next_month(year, month);
            PageRef::Month { year, month }
        }
        PageRef::Future { year, month } => {
            let (year, month) = next_month(year, month);
            PageRef::Future { year, month }
        }
        PageRef::Week { year, week } => crate::week_of(crate::week_monday(year, week) + chrono::Duration::days(7)),
        // Paginile din index nu au „pagina următoare”.
        PageRef::Topic { .. } => r,
    }
}

pub fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 { (year + 1, 1) } else { (year, month + 1) }
}

/// Luna în care se află o pagină.
pub fn month_of(r: PageRef) -> PageRef {
    match r {
        PageRef::Day(d) => PageRef::Month { year: d.year(), month: d.month() },
        PageRef::Future { year, month } => PageRef::Month { year, month },
        PageRef::Week { year, week } => {
            let d = crate::week_monday(year, week);
            PageRef::Month { year: d.year(), month: d.month() }
        }
        PageRef::Topic { date, .. } => PageRef::Month { year: date.year(), month: date.month() },
        m => m,
    }
}

fn not_a_task() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "intrarea nu e o sarcină neterminată")
}

impl Journal {
    /// Mută sarcina de pe poziția `index` din pagina `from` în pagina `to`
    /// (`[>]` dacă `to` e pagina următoare, `[<]` altfel). Întoarce semnul ei.
    pub fn migrate(&self, from: PageRef, index: usize, to: PageRef, lang: Lang) -> io::Result<String> {
        let wrong = match (from, to) {
            (PageRef::Day(a), PageRef::Day(b)) => b <= a,
            _ => from == to,
        };
        if wrong {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "ziua aleasă trebuie să fie după ziua sarcinii"));
        }
        let source = self.load_ref(from)?;
        let target = self.load_ref(to)?;
        let entry = match source.items.get(index) {
            Some(Item::Entry(e)) if e.is_open_task() => e.clone(),
            _ => return Err(not_a_task()),
        };
        let id = entry.id.clone().unwrap_or_else(|| new_id(&[&source, &target]));

        let mut copy = entry.clone();
        copy.depth = 0;
        copy.id = Some(id.clone());
        let kids = rebase_items(children(&source, index), &self.page_dir(from), &self.page_dir(to));
        // Întâi pagina nouă: dacă se întrerupe ceva, sarcina apare de două ori, nu deloc.
        self.update_ref(to, lang, move |t| {
            t.push(copy);
            for k in kids {
                t.push_item(k);
            }
        })?;
        let state = if next_page(from) == to { TaskState::Migrated } else { TaskState::Scheduled };
        let id2 = id.clone();
        self.update_ref(from, lang, move |s| {
            if let Some(e) = s.entry_mut(index) {
                e.id = Some(id2);
                e.kind = Kind::Task(state);
            }
        })?;
        Ok(id)
    }

    /// Mută o intrare oarecare (sarcină, eveniment, notă) pe altă pagină, cu notițele ei.
    /// O sarcină neterminată lasă urma în pagina ei (`[<]`, sau `[>]` pe pagina următoare);
    /// evenimentele și notițele pleacă de tot. Ziua de la începutul textului, din Planuri,
    /// se folosește: pe o zi se scoate din text, iar în lună evenimentul intră în calendar.
    pub fn move_entry(&self, from: PageRef, index: usize, to: PageRef, lang: Lang) -> io::Result<()> {
        use crate::future::split_day;
        if from == to {
            return Ok(());
        }
        let source = self.load_ref(from)?;
        let entry = match source.items.get(index) {
            Some(Item::Entry(e)) => e.clone(),
            _ => return Err(not_a_task()),
        };
        if entry.is_open_task() {
            // Sarcinile: ca la migrare, cu semn și urmă.
            if let (PageRef::Future { .. }, PageRef::Day(_)) = (from, to) {
                let (_, rest) = split_day(&entry.text);
                let rest = rest.to_string();
                let id = self.migrate(from, index, to, lang)?;
                return self
                    .update_ref(to, lang, move |d| {
                        if let Some(Item::Entry(e)) = d.items.iter_mut().rev().find(|it| matches!(it, Item::Entry(e) if e.id.as_deref() == Some(id.as_str()))) {
                            e.text = rest;
                        }
                    })
                    .map(|_| ());
            }
            return self.migrate(from, index, to, lang).map(|_| ());
        }
        if !matches!(entry.kind, Kind::Event | Kind::Note) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "o sarcină încheiată nu se mută"));
        }
        let kids = rebase_items(children(&source, index), &self.page_dir(from), &self.page_dir(to));
        let (fday, rest) = match from {
            PageRef::Future { .. } => split_day(&entry.text),
            _ => (None, entry.text.as_str()),
        };
        // Un eveniment dintr-o zi a lunii merge în calendarul lunii, pe ziua lui.
        let cal_day = match (from, to) {
            (PageRef::Future { .. }, PageRef::Month { .. }) => fday,
            (PageRef::Day(d), PageRef::Month { year, month }) if entry.kind == Kind::Event && d.year() == year && d.month() == month => Some(d.day()),
            _ => None,
        };
        match (to, cal_day) {
            (PageRef::Month { year, month }, Some(day)) => {
                let text = match entry.time.as_deref() {
                    Some(h) => format!("{h} {rest}"),
                    None => rest.to_string(),
                };
                self.update_month(year, month, lang, |m| {
                    let mut texts: Vec<String> = m.day_texts(day).iter().map(|t| t.to_string()).collect();
                    if !texts.contains(&text) {
                        texts.push(text);
                    }
                    m.set_day(day, &texts);
                })?;
            }
            _ => {
                let mut copy = entry.clone();
                copy.depth = 0;
                if matches!(to, PageRef::Day(_)) {
                    copy.text = rest.to_string();
                }
                self.update_ref(to, lang, move |d| {
                    d.push(copy);
                    for k in kids {
                        d.push_item(k);
                    }
                })?;
            }
        }
        self.update_ref(from, lang, |d| {
            d.remove(index);
        })
        .map(|_| ())
    }

    /// Renunță la sarcina de pe poziția `index` (o taie).
    pub fn drop_task(&self, from: PageRef, index: usize, lang: Lang) -> io::Result<()> {
        let mut ok = false;
        self.update_ref(from, lang, |d| {
            if let Some(e) = d.entry_mut(index)
                && e.is_open_task()
            {
                e.kind = Kind::Task(TaskState::Dropped);
                ok = true;
            }
        })?;
        if ok { Ok(()) } else { Err(not_a_task()) }
    }

    /// Anulează alegerea făcută pentru sarcina de pe poziția `index`:
    /// o face din nou neterminată și scoate copia din pagina în care fusese mutată.
    /// Nu anulează dacă acea copie a fost între timp bifată sau schimbată.
    pub fn reopen(&self, from: PageRef, index: usize, lang: Lang) -> io::Result<()> {
        let source = self.load_ref(from)?;
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
        self.update_ref(from, lang, |d| {
            if let Some(e) = d.entry_mut(index) {
                e.kind = Kind::Task(TaskState::Open);
            }
        })?;
        Ok(())
    }

    /// Scoate copia sarcinii cu semnul `id` (și notițele ei) din pagina în care a ajuns:
    /// singura pagină, alta decât `from`, unde sarcina apare încă neterminată.
    fn remove_copy(&self, from: PageRef, id: &str, lang: Lang) -> io::Result<()> {
        let mut found_done = false;
        for r in self.pages() {
            if r == from {
                continue;
            }
            let day = self.load_ref(r)?;
            let Some(pos) = day.items.iter().position(|it| matches!(it, Item::Entry(e) if e.id.as_deref() == Some(id) && e.is_open_task())) else {
                // O copie bifată sau mutată mai departe, după `from`, oprește anularea.
                if day.entries().any(|e| e.id.as_deref() == Some(id) && matches!(e.kind, Kind::Task(TaskState::Done) | Kind::Task(TaskState::Migrated) | Kind::Task(TaskState::Scheduled)))
                    && !self.was_before(from, r)
                {
                    found_done = true;
                }
                continue;
            };
            let n = 1 + children(&day, pos).len();
            return self.update_ref(r, lang, move |d| {
                d.items.drain(pos..pos + n);
            }).map(|_| ());
        }
        if found_done {
            return Err(io::Error::other("sarcina a fost deja bifată sau mutată mai departe; alegerea nu se mai poate anula"));
        }
        Ok(())
    }

    /// Adevărat dacă pagina `r` vine înaintea lui `from` în caiet
    /// (acolo sunt mutările mai vechi ale aceleiași sarcini, nu copia ei).
    fn was_before(&self, from: PageRef, r: PageRef) -> bool {
        let key = |p: PageRef| match p {
            PageRef::Day(d) => (d.year(), d.month(), d.day()),
            // Lista lunii vine înaintea zilelor ei.
            PageRef::Month { year, month } | PageRef::Future { year, month } => (year, month, 0),
            PageRef::Week { year, week } => {
                let d = crate::week_monday(year, week);
                (d.year(), d.month(), d.day())
            }
            PageRef::Topic { date, .. } => (date.year(), date.month(), date.day()),
        };
        key(r) < key(from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
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

    fn z(day: u32) -> PageRef {
        PageRef::Day(d(day))
    }

    const NOV: PageRef = PageRef::Month { year: 2026, month: 11 };
    const DEC: PageRef = PageRef::Month { year: 2026, month: 12 };

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
        let id = j.migrate(z(20), 1, z(21), Lang::Ro).unwrap();
        let src = fs::read_to_string(r.join("2026/11/2026-11-20.md")).unwrap();
        assert!(src.contains(&format!("- [>] Becuri ^{id}\n  - LED, lumină caldă\n")));
        let dst = fs::read_to_string(r.join("2026/11/2026-11-21.md")).unwrap();
        assert_eq!(dst, format!("# Sâmbătă, 21 noiembrie 2026\n\n- [ ] Becuri ^{id}\n  - LED, lumină caldă\n"));
        assert_eq!(j.migration_count(&id), 1);

        // A doua zi, aceeași sarcină mutată din nou: același semn, ↻2.
        let day21 = j.load(d(21)).unwrap();
        assert_eq!(candidates(&day21), vec![0]);
        let id2 = j.migrate(z(21), 0, z(22), Lang::Ro).unwrap();
        assert_eq!(id, id2);
        assert_eq!(j.migration_count(&id), 2);
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn programeaza_renunta_anuleaza() {
        let (j, r) = setup("prog");
        let id = j.migrate(z(20), 3, z(27), Lang::Ro).unwrap();
        let day = j.load(d(20)).unwrap();
        assert_eq!(day.entries().nth(3).unwrap().kind, Kind::Task(TaskState::Scheduled));
        assert!(fs::read_to_string(r.join("2026/11/2026-11-27.md")).unwrap().contains(&format!("- [ ] * Raport ^{id}")));

        j.drop_task(z(20), 5, Lang::Ro).unwrap();
        assert_eq!(j.load(d(20)).unwrap().entries().nth(5).unwrap().kind, Kind::Task(TaskState::Dropped));

        // Anularea: sarcina redevine neterminată, copia dispare.
        j.reopen(z(20), 3, Lang::Ro).unwrap();
        j.reopen(z(20), 5, Lang::Ro).unwrap();
        let after = fs::read_to_string(r.join("2026/11/2026-11-20.md")).unwrap();
        assert!(after.contains(&format!("- [ ] * Raport ^{id}\n")));
        assert!(after.contains("- [ ] Lampă\n"));
        assert!(j.load(d(27)).unwrap().is_empty());
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn nu_anuleaza_ce_s_a_bifat_intre_timp() {
        let (j, r) = setup("bifat");
        j.migrate(z(20), 1, z(21), Lang::Ro).unwrap();
        j.toggle_done(d(21), 0, Lang::Ro).unwrap();
        assert!(j.reopen(z(20), 1, Lang::Ro).is_err());
        assert_eq!(j.load(d(20)).unwrap().entries().nth(1).unwrap().kind, Kind::Task(TaskState::Migrated));
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn din_zi_in_lista_lunii_si_de_la_luna_la_luna() {
        let (j, r) = setup("luna");
        // Din zi în lista lunii: [<]
        let id = j.migrate(z(20), 5, NOV, Lang::Ro).unwrap();
        assert!(fs::read_to_string(r.join("2026/11/2026-11-20.md")).unwrap().contains(&format!("- [<] Lampă ^{id}")));
        assert!(fs::read_to_string(r.join("2026/11/luna.md")).unwrap().contains(&format!("- [ ] Lampă ^{id}")));
        // Sfârșitul lunii: din noiembrie în decembrie, [>]
        let nov = j.load_ref(NOV).unwrap();
        assert_eq!(candidates(&nov), vec![0]);
        j.migrate(NOV, 0, DEC, Lang::Ro).unwrap();
        assert!(fs::read_to_string(r.join("2026/11/luna.md")).unwrap().contains(&format!("- [>] Lampă ^{id}")));
        assert!(fs::read_to_string(r.join("2026/12/luna.md")).unwrap().contains(&format!("- [ ] Lampă ^{id}")));
        assert_eq!(j.migration_count(&id), 2);
        // Răzgândire la lună: copia din decembrie dispare, iar cea din noiembrie redevine neterminată.
        j.reopen(NOV, 0, Lang::Ro).unwrap();
        assert!(j.load_ref(DEC).unwrap().entries().next().is_none());
        assert!(j.load_ref(NOV).unwrap().entries().next().unwrap().is_open_task());
        // Din lună pe o zi anume: [<]
        j.migrate(NOV, 0, PageRef::Day(NaiveDate::from_ymd_opt(2026, 12, 3).unwrap()), Lang::Ro).unwrap();
        assert!(fs::read_to_string(r.join("2026/12/2026-12-03.md")).unwrap().contains("- [ ] Lampă"));
        assert_eq!(next_page(DEC), PageRef::Month { year: 2027, month: 1 });
        assert_eq!(month_of(z(20)), NOV);
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn viitorul_coboara_in_luna() {
        let (j, r) = setup("viitor");
        fs::write(
            r.join("viitor.md"),
            "# Jurnalul viitor\n\n## Decembrie 2026\n\n- [ ] 15 Revizia mașinii\n  - la service-ul din Turda\n- o 24 Crăciun la părinți\n- o 18:00 31 Revelion\n- Idee: cadouri\n- [x] Deja făcută\n\n## Ianuarie 2027\n\n- o 7 Ziua lui Ion\n",
        )
        .unwrap();
        let n = j.transfer_future(2026, 12, Lang::Ro).unwrap();
        assert_eq!(n, 4);
        let luna = fs::read_to_string(r.join("2026/12/luna.md")).unwrap();
        assert!(luna.contains("- 24 Crăciun la părinți\n- 31 18:00 Revelion\n"), "{luna}");
        assert!(luna.contains("- [ ] 15 Revizia mașinii ^"));
        assert!(luna.contains("  - la service-ul din Turda\n- Idee: cadouri\n"), "{luna}");
        let viitor = fs::read_to_string(r.join("viitor.md")).unwrap();
        assert!(viitor.contains("- [>] 15 Revizia mașinii ^"), "{viitor}");
        assert!(viitor.contains("## Ianuarie 2027"));
        // A doua oară nu se dublează sarcinile (sunt deja [>]); evenimentele nu se repetă în calendar.
        j.transfer_future(2026, 12, Lang::Ro).unwrap();
        let luna2 = fs::read_to_string(r.join("2026/12/luna.md")).unwrap();
        assert_eq!(luna2.matches("Revizia").count(), 1);
        assert_eq!(luna2.matches("Crăciun").count(), 1);
        assert_eq!(luna2.matches("Idee: cadouri").count(), 1);
        // Pagina viitorului, ca pagină cu intrări.
        let ian = PageRef::Future { year: 2027, month: 1 };
        j.update_ref(ian, Lang::Ro, |d| d.push(crate::day::parse_quick(". 3 Plătește taxele").unwrap())).unwrap();
        assert_eq!(j.load_ref(ian).unwrap().entries().count(), 2);
        assert_eq!(next_page(ian), PageRef::Future { year: 2027, month: 2 });
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn muta_din_planuri_in_luna_si_pe_o_zi() {
        let (j, r) = setup("muta");
        fs::write(
            r.join("viitor.md"),
            "# Planuri\n\n## Decembrie 2026\n\n- [ ] 15 Revizia mașinii\n- o 24 Crăciun la părinți\n  - cu tort\n- Idee: cadouri\n",
        )
        .unwrap();
        let dec = PageRef::Future { year: 2026, month: 12 };
        // Evenimentul cu zi: în calendarul lunii noiembrie (mutat mai devreme), pe 24.
        j.move_entry(dec, 1, NOV, Lang::Ro).unwrap();
        assert_eq!(j.load_month(2026, 11).unwrap().day_texts(24), vec!["Crăciun la părinți"]);
        assert!(!fs::read_to_string(r.join("viitor.md")).unwrap().contains("Crăciun"));
        // Sarcina cu zi: pe o zi anume, fără numărul zilei; în Planuri rămâne cu <.
        j.move_entry(dec, 0, z(22), Lang::Ro).unwrap();
        let zi = fs::read_to_string(r.join("2026/11/2026-11-22.md")).unwrap();
        assert!(zi.contains("- [ ] Revizia mașinii ^"), "{zi}");
        assert!(fs::read_to_string(r.join("viitor.md")).unwrap().contains("- [<] 15 Revizia mașinii ^"));
        // Nota fără zi: în lista lunii.
        let idx = j.load_ref(dec).unwrap().items.iter().position(|it| matches!(it, Item::Entry(e) if e.text.starts_with("Idee"))).unwrap();
        j.move_entry(dec, idx, NOV, Lang::Ro).unwrap();
        assert!(fs::read_to_string(r.join("2026/11/luna.md")).unwrap().contains("- Idee: cadouri"));
        // O notă dintr-o zi, mutată pe mâine: pleacă de tot, cu notițele ei.
        j.move_entry(z(20), 0, z(21), Lang::Ro).unwrap();
        assert!(!fs::read_to_string(r.join("2026/11/2026-11-20.md")).unwrap().contains("Cină"));
        assert!(fs::read_to_string(r.join("2026/11/2026-11-21.md")).unwrap().contains("- o 18:30 Cină"));
        // O sarcină făcută nu se mută.
        assert!(j.move_entry(z(20), 3, z(21), Lang::Ro).is_err());
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn planul_saptamanii() {
        let (j, r) = setup("sapt");
        let s47 = crate::week_of(d(20));
        assert_eq!(s47, PageRef::Week { year: 2026, week: 47 });
        j.update_ref(s47, Lang::Ro, |w| w.push(crate::day::parse_quick(". Curățenie în debara").unwrap())).unwrap();
        let text = fs::read_to_string(r.join("2026/saptamani/2026-S47.md")).unwrap();
        assert_eq!(text, "# Săptămâna 47, 16-22 noiembrie 2026\n\n- [ ] Curățenie în debara\n");
        // Din zi în planul săptămânii, apoi din plan în săptămâna următoare.
        j.migrate(z(20), 5, s47, Lang::Ro).unwrap();
        let s48 = next_page(s47);
        assert_eq!(s48, PageRef::Week { year: 2026, week: 48 });
        let id = j.migrate(s47, 0, s48, Lang::Ro).unwrap();
        assert!(fs::read_to_string(r.join("2026/saptamani/2026-S47.md")).unwrap().contains(&format!("- [>] Curățenie în debara ^{id}")));
        assert!(fs::read_to_string(r.join("2026/saptamani/2026-S48.md")).unwrap().starts_with("# Săptămâna 48, 23-29 noiembrie 2026\n"));
        assert_eq!(j.migration_count(&id), 1);
        // Răzgândirea caută copia și în săptămâni.
        j.reopen(s47, 0, Lang::Ro).unwrap();
        assert!(j.load_ref(s48).unwrap().entries().next().is_none());
        assert_eq!(month_of(s47), NOV);
        assert_eq!(next_page(PageRef::Week { year: 2026, week: 53 }), PageRef::Week { year: 2027, week: 1 });
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn pozele_merg_cu_intrarea() {
        use std::path::Path;
        assert_eq!(rebase("2026-11-21/x.jpg", Path::new("/j/2026/11"), Path::new("/j/pagini")), "../2026/11/2026-11-21/x.jpg");
        assert_eq!(rebase("2026-11-21/x.jpg", Path::new("/j/2026/11"), Path::new("/j/2026/12")), "../11/2026-11-21/x.jpg");
        let (j, r) = setup("poze");
        // O poză sub „Becuri” (poziția 1), apoi Becuri mutată pe 1 decembrie: poza merge cu ea.
        j.update_ref(z(20), Lang::Ro, |d| d.items.insert(3, crate::day::parse_item("  ![Bec|160](2026-11-20/bec.jpg)"))).unwrap();
        assert_eq!(j.load(d(20)).unwrap().children_count(1), 2);
        let dec1 = PageRef::Day(NaiveDate::from_ymd_opt(2026, 12, 1).unwrap());
        j.migrate(z(20), 1, dec1, Lang::Ro).unwrap();
        let t = fs::read_to_string(r.join("2026/12/2026-12-01.md")).unwrap();
        assert!(t.contains("  - LED, lumină caldă\n  ![Bec|160](../11/2026-11-20/bec.jpg)\n"), "{t}");
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn refuza_ce_nu_e_sarcina_sau_zi_gresita() {
        let (j, r) = setup("refuz");
        assert!(j.migrate(z(20), 0, z(21), Lang::Ro).is_err());
        assert!(j.migrate(z(20), 1, z(20), Lang::Ro).is_err());
        assert!(j.drop_task(z(20), 4, Lang::Ro).is_err());
        fs::remove_dir_all(r).unwrap();
    }
}
