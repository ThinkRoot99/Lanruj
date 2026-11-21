// SPDX-License-Identifier: GPL-3.0-or-later
//! Monitorizarea: câte un fișier pe lună, `monitorizare/2026-11.md`.
//!
//! ```text
//! # Noiembrie 2026
//!
//! ## Serviciu
//!
//! - 2 tură
//! - 3 ore suplimentare 08:00-12:00
//! - 15-17 concediu
//!
//! ## Somn
//!
//! - 1 noapte 7:30
//! - 1 zi 0:45
//!
//! ## Activități
//!
//! Listă: cafea, citit, făcut patul
//!
//! - 1 cafea, citit
//! - 2 făcut patul
//! ```
//!
//! Zilele la rând cu același fel de serviciu se scriu ca interval. Secțiunile
//! necunoscute rămân neatinse. Citește și engleza (Work, Sleep, Activities).

use crate::journal::Journal;
use crate::lang::Lang;
use crate::search::fold;
use chrono::{Datelike, NaiveDate};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;

/// Ce a fost într-o zi, la serviciu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    /// O tură de 24 de ore.
    Shift,
    /// Concediu de odihnă.
    Leave,
}

impl Shift {
    pub const HOURS: u32 = 24;

    fn word(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Shift::Shift, Lang::Ro) => "tură",
            (Shift::Leave, Lang::Ro) => "concediu",
            (Shift::Shift, Lang::En) => "shift",
            (Shift::Leave, Lang::En) => "leave",
        }
    }

    fn parse(w: &str) -> Option<Shift> {
        match fold(w.trim()).as_str() {
            // „tură extra” din fișierele vechi rămâne o tură.
            "tura" | "shift" | "tura extra" | "extra" | "extra shift" => Some(Shift::Shift),
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
    pub overtime: BTreeMap<u32, Vec<Span>>,
    pub sleep: BTreeMap<u32, Sleep>,
    /// Activitățile urmărite, în ordinea lor.
    pub activities: Vec<String>,
    /// Ce activități s-au făcut în fiecare zi.
    pub done: BTreeMap<u32, BTreeSet<String>>,
    /// Alte secțiuni, ca text, ca să nu se piardă la scriere.
    pub other: Vec<String>,
}

/// Totalurile serviciului.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub shifts: u32,
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
                    let f = fold(word);
                    if let Some(span) = f.strip_prefix("ore suplimentare ").or_else(|| f.strip_prefix("overtime ")).and_then(Span::parse) {
                        if let Some((a, _)) = days(d) {
                            t.overtime.entry(a).or_default().push(span);
                        }
                    } else if let (Some((a, b)), Some(k)) = (days(d), Shift::parse(word)) {
                        for n in a..=b {
                            t.work.insert(n, k);
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
                        t.activities = list.split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
                    } else if let Some(rest) = item {
                        let (d, names) = rest.split_once(' ').unwrap_or((rest, ""));
                        let Ok(d) = d.parse::<u32>() else { continue };
                        let set = t.done.entry(d).or_default();
                        for a in names.split(',').map(str::trim).filter(|a| !a.is_empty()) {
                            set.insert(a.to_string());
                            if !t.activities.iter().any(|x| x == a) {
                                t.activities.push(a.to_string());
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
            let mut i = 0;
            while i < ds.len() {
                let k = self.work[&ds[i]];
                let mut j = i;
                while j + 1 < ds.len() && ds[j + 1] == ds[j] + 1 && self.work[&ds[j + 1]] == k {
                    j += 1;
                }
                let d = if j > i { format!("{}-{}", ds[i], ds[j]) } else { ds[i].to_string() };
                lines.push((ds[i], format!("- {d} {}\n", k.word(lang))));
                i = j + 1;
            }
            for (d, spans) in &self.overtime {
                for s in spans {
                    lines.push((*d, format!("- {d} {} {}\n", if ro { "ore suplimentare" } else { "overtime" }, s.text())));
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
            out.push_str(&format!("{} {}\n", if ro { "Listă:" } else { "List:" }, self.activities.join(", ")));
            let rows: Vec<String> = self
                .done
                .iter()
                .filter(|(_, s)| !s.is_empty())
                .map(|(d, s)| {
                    // În ordinea listei.
                    let names: Vec<&str> = self.activities.iter().filter(|a| s.contains(*a)).map(String::as_str).collect();
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
                Shift::Shift => t.shifts += 1,
                Shift::Leave => t.leave += 1,
            }
        }
        t.overtime = self.overtime.values().flatten().map(Span::minutes).sum();
        t
    }

    /// Media somnului (noaptea, ziua), în minute, din zilele trecute.
    pub fn sleep_average(&self) -> (Option<u32>, Option<u32>) {
        let avg = |v: Vec<u32>| (!v.is_empty()).then(|| v.iter().sum::<u32>() / v.len() as u32);
        (avg(self.sleep.values().filter_map(|s| s.night).collect()), avg(self.sleep.values().filter_map(|s| s.day).collect()))
    }

    /// În câte zile s-a făcut fiecare activitate.
    pub fn activity_counts(&self) -> Vec<(String, u32)> {
        self.activities.iter().map(|a| (a.clone(), self.done.values().filter(|s| s.contains(a)).count() as u32)).collect()
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

    /// Schimbă ziua `date` la serviciu (cu `None` o face liberă).
    pub fn set_shift(&self, date: NaiveDate, kind: Option<Shift>, lang: Lang) -> io::Result<Tracking> {
        self.update_tracking(date.year(), date.month(), lang, |t| match kind {
            Some(k) => {
                t.work.insert(date.day(), k);
            }
            None => {
                t.work.remove(&date.day());
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUNA: &str = "# Noiembrie 2026\n\n## Serviciu\n\n- 2 tură\n- 3 ore suplimentare 08:00-12:00\n- 15-17 concediu\n- 18 tură\n- 18 ore suplimentare 22:00-02:00\n\n## Somn\n\n- 1 noapte 7:30\n- 1 zi 0:45\n- 2 noapte 6:30\n\n## Activități\n\nListă: cafea, citit, făcut patul\n\n- 1 cafea, citit\n- 2 cafea, făcut patul\n\n## Note\n\nceva\n";

    #[test]
    fn citire_si_scriere() {
        let t = Tracking::parse(LUNA);
        assert_eq!(t.totals(), Totals { shifts: 2, leave: 3, overtime: 8 * 60 });
        assert_eq!(t.totals().worked(), 56 * 60);
        assert_eq!(t.sleep[&1], Sleep { night: Some(450), day: Some(45) });
        assert_eq!(t.sleep_average(), (Some(420), Some(45)));
        assert_eq!(t.activity_counts(), vec![("cafea".into(), 2), ("citit".into(), 1), ("făcut patul".into(), 1)]);
        assert_eq!(t.to_markdown(2026, 11, Lang::Ro), LUNA, "se scrie înapoi la fel");
        // Fișierele vechi: „tură extra” devine tură.
        assert_eq!(Tracking::parse("## Serviciu\n\n- 12 tură extra\n").work[&12], Shift::Shift);
        // Engleza.
        let en = Tracking::parse("# November 2026\n\n## Work\n\n- 2 shift\n- 3 overtime 8-12\n\n## Sleep\n\n- 1 night 7.5\n\n## Activities\n\nList: coffee\n\n- 1 coffee\n");
        assert_eq!(en.totals(), Totals { shifts: 1, leave: 0, overtime: 240 });
        assert_eq!(en.sleep[&1].night, Some(450));
        assert_eq!(en.activity_counts(), vec![("coffee".into(), 1)]);
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
        j.update_tracking(2026, 11, Lang::Ro, |t| t.activities = vec!["cafea".into(), "citit".into()]).unwrap();
        assert_eq!(j.load_tracking(2026, 12).unwrap().activities, vec!["cafea", "citit"], "decembrie primește lista din noiembrie");
        assert_eq!(j.load_tracking(2027, 1).unwrap().activities, vec!["cafea", "citit"]);
        let d = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        j.set_shift(d, Some(Shift::Shift), Lang::Ro).unwrap();
        j.set_shift(d, None, Lang::Ro).unwrap();
        assert!(!j.tracking_path(2026, 10).exists(), "o lună goală nu lasă fișier");
        std::fs::remove_dir_all(root).unwrap();
    }
}
