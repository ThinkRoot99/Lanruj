// SPDX-License-Identifier: GPL-3.0-or-later
//! Monitorizarea: câte un fișier pe lună, `monitorizare/2026-11.md`.
//!
//! ```text
//! # Noiembrie 2026
//!
//! ## Serviciu
//!
//! - 2 serviciu: predare la 8
//! - 3 ore suplimentare 08:00-12:00: inventar
//! - 5 rezervă
//! - 15-17 concediu
//!
//! ## Somn
//!
//! - 1 noapte 7:30
//! - 1 zi 0:45
//!
//! ## Activități
//!
//! Listă: cafea, citit, flotări (număr), apă (pahare)
//!
//! - 1 cafea, citit, flotări 30, apă 6
//! - 2 apă 4
//! ```
//!
//! Zilele la rând cu același fel de serviciu se scriu ca interval. Secțiunile
//! necunoscute rămân neatinse. Citește și engleza (Work, Sleep, Activities).

use crate::journal::Journal;
use crate::lang::Lang;
use crate::search::fold;
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

/// Ce a fost într-o zi, la serviciu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    /// O zi de serviciu, de 24 de ore.
    Service,
    /// O zi de rezervă.
    Reserve,
    /// Concediu de odihnă.
    Leave,
}

impl Shift {
    pub const HOURS: u32 = 24;

    fn word(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Shift::Service, Lang::Ro) => "serviciu",
            (Shift::Reserve, Lang::Ro) => "rezervă",
            (Shift::Leave, Lang::Ro) => "concediu",
            (Shift::Service, Lang::En) => "duty",
            (Shift::Reserve, Lang::En) => "standby",
            (Shift::Leave, Lang::En) => "leave",
        }
    }

    fn parse(w: &str) -> Option<Shift> {
        match fold(w.trim()).as_str() {
            // „tură” și „tură extra” din fișierele vechi sunt zile de serviciu.
            "serviciu" | "duty" | "tura" | "shift" | "tura extra" | "extra" | "extra shift" => Some(Shift::Service),
            "rezerva" | "standby" | "reserve" => Some(Shift::Reserve),
            "concediu" | "co" | "leave" => Some(Shift::Leave),
            _ => None,
        }
    }
}

/// Un interval de ore suplimentare, în minute de la miezul nopții (sfârșitul poate trece de miezul nopții).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn minutes(&self) -> u32 {
        if self.end > self.start { self.end - self.start } else { self.end + 24 * 60 - self.start }
    }

    pub fn text(&self) -> String {
        format!("{}-{}", clock(self.start), clock(self.end))
    }

    pub fn parse(s: &str) -> Option<Span> {
        let (a, b) = s.trim().split_once('-')?;
        Some(Span { start: parse_clock(a)?, end: parse_clock(b)? })
    }
}

/// „08:00”.
pub fn clock(m: u32) -> String {
    format!("{:02}:{:02}", m / 60 % 24, m % 60)
}

/// „8”, „8:30”, „08.30”, „8h30” → minute de la miezul nopții.
pub fn parse_clock(s: &str) -> Option<u32> {
    let s = s.trim().replace(['.', 'h', 'H'], ":");
    let (h, m) = s.split_once(':').unwrap_or((&s, "0"));
    let (h, m): (u32, u32) = (h.trim().parse().ok()?, if m.trim().is_empty() { 0 } else { m.trim().parse().ok()? });
    (h <= 24 && m < 60).then_some(h * 60 + m)
}

/// O durată: „7:30”, „7h30”, „7,5”, „7.5”, „7” (ore), „45m” (minute) → minute.
pub fn parse_duration(s: &str) -> Option<u32> {
    let s = s.trim().to_lowercase().replace(' ', "");
    if s.is_empty() {
        return None;
    }
    if let Some(m) = s.strip_suffix("min").or_else(|| s.strip_suffix('m')) {
        return m.parse::<u32>().ok().filter(|m| *m <= 24 * 60);
    }
    let s = s.trim_end_matches(['h', 'o']);
    if let Some((h, m)) = s.split_once(':').or_else(|| s.split_once('h')) {
        let (h, m): (u32, u32) = (h.parse().ok()?, if m.is_empty() { 0 } else { m.parse().ok()? });
        return (h <= 24 && m < 60).then_some(h * 60 + m);
    }
    let f: f64 = s.replace(',', ".").parse().ok()?;
    (0.0..=24.0).contains(&f).then(|| (f * 60.0).round() as u32)
}

/// „7:30”.
pub fn duration(m: u32) -> String {
    format!("{}:{:02}", m / 60, m % 60)
}

/// Ore suplimentare: intervalul și o notă (poate fi goală).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overtime {
    pub span: Span,
    pub note: String,
}

/// O activitate urmărită: cu bifă sau cu număr (și unitate, de exemplu „pahare”).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    pub name: String,
    pub counted: bool,
    pub unit: Option<String>,
}

impl Activity {
    pub fn check(name: &str) -> Activity {
        Activity { name: name.to_string(), counted: false, unit: None }
    }

    /// „cafea”, „flotări (număr)”, „apă (pahare)”.
    fn parse(s: &str) -> Option<Activity> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if let Some((name, rest)) = s.split_once('(') {
            let unit = rest.trim_end_matches(')').trim();
            let plain = matches!(fold(unit).as_str(), "numar" | "number" | "");
            return Some(Activity { name: name.trim().to_string(), counted: true, unit: (!plain).then(|| unit.to_string()) });
        }
        Some(Activity::check(s))
    }

    fn text(&self, lang: Lang) -> String {
        match (&self.unit, self.counted) {
            (Some(u), _) => format!("{} ({u})", self.name),
            (None, true) => format!("{} ({})", self.name, if lang == Lang::Ro { "număr" } else { "number" }),
            (None, false) => self.name.clone(),
        }
    }
}

/// Ce s-a întâmplat la serviciu într-o zi, pentru celelalte pagini (zi, săptămână, lună).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkOnDay {
    Day(Shift, String),
    Overtime(Span, String),
}

/// Somnul unei zile, în minute.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sleep {
    pub night: Option<u32>,
    pub day: Option<u32>,
}

/// Luna de monitorizare.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracking {
    pub work: BTreeMap<u32, Shift>,
    /// Notele zilelor de serviciu, rezervă sau concediu.
    pub work_notes: BTreeMap<u32, String>,
    pub overtime: BTreeMap<u32, Vec<Overtime>>,
    pub sleep: BTreeMap<u32, Sleep>,
    /// Activitățile urmărite, în ordinea lor.
    pub activities: Vec<Activity>,
    /// Ce activități s-au făcut în fiecare zi și de câte ori (o bifă = 1).
    pub done: BTreeMap<u32, BTreeMap<String, u32>>,
    /// Alte secțiuni, ca text, ca să nu se piardă la scriere.
    pub other: Vec<String>,
}

/// Totalurile serviciului.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub shifts: u32,
    pub reserve: u32,
    pub leave: u32,
    /// Minute suplimentare.
    pub overtime: u32,
}

impl Totals {
    /// Minutele lucrate: turele și orele suplimentare.
    pub fn worked(&self) -> u32 {
        self.shifts * Shift::HOURS * 60 + self.overtime
    }
}

#[derive(PartialEq)]
enum Sec {
    None,
    Work,
    Sleep,
    Activities,
    Other,
}

fn section(h: &str) -> Sec {
    match fold(h.trim()).as_str() {
        "serviciu" | "work" => Sec::Work,
        "somn" | "sleep" => Sec::Sleep,
        "activitati" | "activities" => Sec::Activities,
        _ => Sec::Other,
    }
}

/// „15-17” → (15, 17); „3” → (3, 3).
fn days(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once('-').unwrap_or((s, s));
    let (a, b): (u32, u32) = (a.parse().ok()?, b.parse().ok()?);
    let (a, b) = (a.min(b), a.max(b).min(31));
    (a >= 1).then_some((a, b))
}

impl Tracking {
    pub fn parse(text: &str) -> Tracking {
        let mut t = Tracking::default();
        let mut sec = Sec::None;
        for line in text.lines() {
            if line.starts_with("# ") {
                continue;
            }
            if let Some(h) = line.strip_prefix("## ") {
                sec = section(h);
                if sec == Sec::Other {
                    t.other.push(line.to_string());
                }
                continue;
            }
            let item = line.trim().strip_prefix("- ");
            match sec {
                Sec::Work => {
                    let Some(rest) = item else { continue };
                    let (d, word) = rest.split_once(' ').unwrap_or((rest, ""));
                    // Nota stă după „: ” (orele „08:00” nu au spațiu după două puncte).
                    let (word, note) = word.split_once(": ").map(|(w, n)| (w, n.trim().to_string())).unwrap_or((word, String::new()));
                    let f = fold(word);
                    if let Some(span) = f.strip_prefix("ore suplimentare ").or_else(|| f.strip_prefix("overtime ")).and_then(Span::parse) {
                        if let Some((a, _)) = days(d) {
                            t.overtime.entry(a).or_default().push(Overtime { span, note });
                        }
                    } else if let (Some((a, b)), Some(k)) = (days(d), Shift::parse(word)) {
                        for n in a..=b {
                            t.work.insert(n, k);
                            if !note.is_empty() {
                                t.work_notes.insert(n, note.clone());
                            }
                        }
                    }
                }
                Sec::Sleep => {
                    let Some(rest) = item else { continue };
                    let mut w = rest.split_whitespace();
                    let (Some(d), Some(kind), Some(v)) = (w.next().and_then(|d| d.parse::<u32>().ok()), w.next(), w.next()) else { continue };
                    let Some(min) = parse_duration(v) else { continue };
                    let e = t.sleep.entry(d).or_default();
                    match fold(kind).as_str() {
                        "noapte" | "night" => e.night = Some(min),
                        "zi" | "day" => e.day = Some(min),
                        _ => {}
                    }
                }
                Sec::Activities => {
                    if let Some(list) = line.strip_prefix("Listă:").or_else(|| line.strip_prefix("Lista:")).or_else(|| line.strip_prefix("List:")) {
                        t.activities = list.split(',').filter_map(Activity::parse).collect();
                    } else if let Some(rest) = item {
                        let (d, names) = rest.split_once(' ').unwrap_or((rest, ""));
                        let Ok(d) = d.parse::<u32>() else { continue };
                        let set = t.done.entry(d).or_default();
                        for a in names.split(',').map(str::trim).filter(|a| !a.is_empty()) {
                            // „flotări 30” = 30; „cafea” = 1.
                            let (name, n) = match a.rsplit_once(' ') {
                                Some((name, n)) if n.parse::<u32>().is_ok() => (name.trim(), n.parse::<u32>().unwrap_or(1)),
                                _ => (a, 1),
                            };
                            set.insert(name.to_string(), n);
                            if !t.activities.iter().any(|x| x.name == name) {
                                t.activities.push(Activity::check(name));
                            }
                        }
                    }
                }
                Sec::Other => t.other.push(line.to_string()),
                Sec::None => {}
            }
        }
        while t.other.last().is_some_and(|l| l.trim().is_empty()) {
            t.other.pop();
        }
        t
    }

    pub fn to_markdown(&self, year: i32, month: u32, lang: Lang) -> String {
        let ro = lang == Lang::Ro;
        let mut out = format!("# {}\n", lang.month_title(year, month));
        // Serviciul: zilele (cu intervale) și orele suplimentare, în ordinea zilelor.
        if !self.work.is_empty() || !self.overtime.is_empty() {
            out.push_str(if ro { "\n## Serviciu\n\n" } else { "\n## Work\n\n" });
            let mut lines: Vec<(u32, String)> = Vec::new();
            let ds: Vec<u32> = self.work.keys().copied().collect();
            let note = |d: &u32| self.work_notes.get(d).map(String::as_str).unwrap_or("");
            let mut i = 0;
            while i < ds.len() {
                let k = self.work[&ds[i]];
                let mut j = i;
                // Doar zilele fără notă se strâng într-un interval.
                while j + 1 < ds.len() && ds[j + 1] == ds[j] + 1 && self.work[&ds[j + 1]] == k && note(&ds[j]).is_empty() && note(&ds[j + 1]).is_empty() {
                    j += 1;
                }
                let d = if j > i { format!("{}-{}", ds[i], ds[j]) } else { ds[i].to_string() };
                let n = note(&ds[i]);
                let n = if n.is_empty() { String::new() } else { format!(": {n}") };
                lines.push((ds[i], format!("- {d} {}{n}\n", k.word(lang))));
                i = j + 1;
            }
            for (d, list) in &self.overtime {
                for o in list {
                    let n = if o.note.is_empty() { String::new() } else { format!(": {}", o.note) };
                    lines.push((*d, format!("- {d} {} {}{n}\n", if ro { "ore suplimentare" } else { "overtime" }, o.span.text())));
                }
            }
            lines.sort_by_key(|(d, _)| *d);
            for (_, l) in lines {
                out.push_str(&l);
            }
        }
        if self.sleep.values().any(|s| s.night.is_some() || s.day.is_some()) {
            out.push_str(if ro { "\n## Somn\n\n" } else { "\n## Sleep\n\n" });
            for (d, s) in &self.sleep {
                if let Some(n) = s.night {
                    out.push_str(&format!("- {d} {} {}\n", if ro { "noapte" } else { "night" }, duration(n)));
                }
                if let Some(z) = s.day {
                    out.push_str(&format!("- {d} {} {}\n", if ro { "zi" } else { "day" }, duration(z)));
                }
            }
        }
        if !self.activities.is_empty() {
            out.push_str(if ro { "\n## Activități\n\n" } else { "\n## Activities\n\n" });
            let list: Vec<String> = self.activities.iter().map(|a| a.text(lang)).collect();
            out.push_str(&format!("{} {}\n", if ro { "Listă:" } else { "List:" }, list.join(", ")));
            let rows: Vec<String> = self
                .done
                .iter()
                .filter(|(_, s)| s.values().any(|n| *n > 0))
                .map(|(d, s)| {
                    // În ordinea listei; cele cu număr îl au după nume.
                    let names: Vec<String> = self
                        .activities
                        .iter()
                        .filter_map(|a| s.get(&a.name).filter(|n| **n > 0).map(|n| if a.counted { format!("{} {n}", a.name) } else { a.name.clone() }))
                        .collect();
                    format!("- {d} {}\n", names.join(", "))
                })
                .collect();
            if !rows.is_empty() {
                out.push('\n');
                out.push_str(&rows.concat());
            }
        }
        if !self.other.is_empty() {
            out.push('\n');
            out.push_str(&self.other.join("\n"));
            out.push('\n');
        }
        out
    }

    pub fn totals(&self) -> Totals {
        let mut t = Totals::default();
        for k in self.work.values() {
            match k {
                Shift::Service => t.shifts += 1,
                Shift::Reserve => t.reserve += 1,
                Shift::Leave => t.leave += 1,
            }
        }
        t.overtime = self.overtime.values().flatten().map(|o| o.span.minutes()).sum();
        t
    }

    /// Media somnului (noaptea, ziua), în minute, din zilele trecute.
    pub fn sleep_average(&self) -> (Option<u32>, Option<u32>) {
        let avg = |v: Vec<u32>| (!v.is_empty()).then(|| v.iter().sum::<u32>() / v.len() as u32);
        (avg(self.sleep.values().filter_map(|s| s.night).collect()), avg(self.sleep.values().filter_map(|s| s.day).collect()))
    }

    /// Pentru fiecare activitate: în câte zile s-a făcut și cât în total (la cele cu număr).
    pub fn activity_counts(&self) -> Vec<(Activity, u32, u32)> {
        self.activities
            .iter()
            .map(|a| {
                let vals: Vec<u32> = self.done.values().filter_map(|s| s.get(&a.name).copied()).filter(|n| *n > 0).collect();
                (a.clone(), vals.len() as u32, vals.iter().sum())
            })
            .collect()
    }

    /// Ce s-a întâmplat la serviciu în ziua `day`: ziua (cu nota) și orele suplimentare.
    pub fn work_on(&self, day: u32) -> Vec<WorkOnDay> {
        let mut out = Vec::new();
        if let Some(k) = self.work.get(&day) {
            out.push(WorkOnDay::Day(*k, self.work_notes.get(&day).cloned().unwrap_or_default()));
        }
        for o in self.overtime.get(&day).into_iter().flatten() {
            out.push(WorkOnDay::Overtime(o.span, o.note.clone()));
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.work.is_empty() && self.overtime.is_empty() && self.sleep.is_empty() && self.activities.is_empty() && self.other.is_empty()
    }
}

impl Journal {
    pub fn tracking_path(&self, year: i32, month: u32) -> PathBuf {
        self.root().join("monitorizare").join(format!("{year:04}-{month:02}.md"))
    }

    /// Luna; dacă nu are încă listă de activități, o ia de la cea mai apropiată lună dinainte.
    pub fn load_tracking(&self, year: i32, month: u32) -> io::Result<Tracking> {
        let mut t = self.read_tracking(year, month)?;
        if t.activities.is_empty() {
            let mut idx = year * 12 + month as i32 - 1;
            for _ in 0..24 {
                idx -= 1;
                let prev = self.read_tracking(idx.div_euclid(12), (idx.rem_euclid(12) + 1) as u32)?;
                if !prev.activities.is_empty() {
                    t.activities = prev.activities;
                    break;
                }
            }
        }
        Ok(t)
    }

    fn read_tracking(&self, year: i32, month: u32) -> io::Result<Tracking> {
        match self.read_text(&self.tracking_path(year, month)) {
            Ok(t) => Ok(Tracking::parse(&t)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Tracking::default()),
            Err(e) => Err(e),
        }
    }

    /// Schimbă luna lui `date` cu `f` și o salvează (o lună goală nu lasă fișier).
    pub fn update_tracking(&self, year: i32, month: u32, lang: Lang, f: impl FnOnce(&mut Tracking)) -> io::Result<Tracking> {
        let mut t = self.load_tracking(year, month)?;
        f(&mut t);
        let path = self.tracking_path(year, month);
        if t.is_empty() {
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        } else {
            self.write_text(&path, &t.to_markdown(year, month, lang))?;
        }
        Ok(t)
    }

    /// Schimbă ziua `date` la serviciu (cu `None` o face liberă, fără notă).
    pub fn set_shift(&self, date: NaiveDate, kind: Option<Shift>, lang: Lang) -> io::Result<Tracking> {
        self.update_tracking(date.year(), date.month(), lang, |t| match kind {
            Some(k) => {
                t.work.insert(date.day(), k);
            }
            None => {
                t.work.remove(&date.day());
                t.work_notes.remove(&date.day());
            }
        })
    }

    /// Ce s-a întâmplat la serviciu în ziua `date` (pentru pagina zilei, a săptămânii și a lunii).
    pub fn work_on(&self, date: NaiveDate) -> Vec<WorkOnDay> {
        self.read_tracking(date.year(), date.month()).map(|t| t.work_on(date.day())).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUNA: &str = "# Noiembrie 2026\n\n## Serviciu\n\n- 2 serviciu: predare la 8\n- 3 ore suplimentare 08:00-12:00: inventar\n- 5 rezervă\n- 15-17 concediu\n- 18 serviciu\n- 18 ore suplimentare 22:00-02:00\n\n## Somn\n\n- 1 noapte 7:30\n- 1 zi 0:45\n- 2 noapte 6:30\n\n## Activități\n\nListă: cafea, citit, flotări (număr), apă (pahare)\n\n- 1 cafea, citit, flotări 30, apă 6\n- 2 cafea, apă 4\n\n## Note\n\nceva\n";

    #[test]
    fn citire_si_scriere() {
        let t = Tracking::parse(LUNA);
        assert_eq!(t.totals(), Totals { shifts: 2, reserve: 1, leave: 3, overtime: 8 * 60 });
        assert_eq!(t.totals().worked(), 56 * 60);
        assert_eq!(t.work_notes[&2], "predare la 8");
        assert_eq!(t.overtime[&3][0].note, "inventar");
        assert_eq!(t.sleep[&1], Sleep { night: Some(450), day: Some(45) });
        assert_eq!(t.sleep_average(), (Some(420), Some(45)));
        let c = t.activity_counts();
        assert_eq!((c[0].1, c[2].1, c[2].2, c[3].2), (2, 1, 30, 10), "cafea 2 zile; flotări o zi, 30; apă 10 pahare");
        assert_eq!(c[3].0.unit.as_deref(), Some("pahare"));
        assert_eq!(t.to_markdown(2026, 11, Lang::Ro), LUNA, "se scrie înapoi la fel");
        assert_eq!(t.work_on(3), vec![WorkOnDay::Overtime(Span { start: 480, end: 720 }, "inventar".into())]);
        assert_eq!(t.work_on(2), vec![WorkOnDay::Day(Shift::Service, "predare la 8".into())]);
        // Fișierele vechi: „tură” și „tură extra” sunt zile de serviciu.
        let old = Tracking::parse("## Serviciu\n\n- 11 tură\n- 12 tură extra\n");
        assert_eq!((old.work[&11], old.work[&12]), (Shift::Service, Shift::Service));
        // Engleza.
        let en = Tracking::parse("# November 2026\n\n## Work\n\n- 2 duty\n- 3 overtime 8-12\n- 4 standby\n\n## Sleep\n\n- 1 night 7.5\n\n## Activities\n\nList: coffee, push-ups (number)\n\n- 1 coffee, push-ups 20\n");
        assert_eq!(en.totals(), Totals { shifts: 1, reserve: 1, leave: 0, overtime: 240 });
        assert_eq!(en.sleep[&1].night, Some(450));
        assert_eq!(en.activity_counts()[1].2, 20);
    }

    #[test]
    fn durate() {
        for (s, m) in [("7:30", 450), ("7h30", 450), ("7,5", 450), ("7.5", 450), ("7", 420), ("45m", 45), ("0:45", 45), ("8h", 480)] {
            assert_eq!(parse_duration(s), Some(m), "{s}");
        }
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(Span::parse("08:00-12:00").unwrap().minutes(), 240);
        assert_eq!(Span::parse("22-2").unwrap().minutes(), 240);
    }

    #[test]
    fn lista_trece_in_luna_urmatoare_si_fisier_sters() {
        let root = std::env::temp_dir().join(format!("lanruj-monitor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let j = Journal::new(&root);
        j.update_tracking(2026, 11, Lang::Ro, |t| t.activities = vec![Activity::check("cafea"), Activity { name: "apă".into(), counted: true, unit: Some("pahare".into()) }]).unwrap();
        let dec = j.load_tracking(2026, 12).unwrap().activities;
        assert_eq!(dec.len(), 2, "decembrie primește lista din noiembrie");
        assert_eq!(dec[1].unit.as_deref(), Some("pahare"));
        let d = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        j.set_shift(d, Some(Shift::Service), Lang::Ro).unwrap();
        assert_eq!(j.work_on(d), vec![WorkOnDay::Day(Shift::Service, String::new())]);
        j.set_shift(d, None, Lang::Ro).unwrap();
        assert!(!j.tracking_path(2026, 10).exists(), "o lună goală nu lasă fișier");
        std::fs::remove_dir_all(root).unwrap();
    }
}
