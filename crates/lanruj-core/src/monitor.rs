// SPDX-License-Identifier: GPL-3.0-or-later
//! Monitorizarea: câte un fișier pe lună, `monitorizare/2026-11.md`.
//!
//! ```text
//! # Noiembrie 2026
//!
//! ## Serviciu
//!
//! - 2 tură
//! - 12 tură extra
//! - 15-17 concediu
//! ```
//!
//! Zilele la rând cu același fel se scriu ca interval. Secțiunile pe care nu le
//! cunoaștem (Somn, Activități, mai târziu) rămân neatinse.

use crate::journal::Journal;
use crate::lang::Lang;
use chrono::NaiveDate;
use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

/// Ce a fost într-o zi, la serviciu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    /// O tură obișnuită, de 24 de ore.
    Shift,
    /// O tură în plus.
    Extra,
    /// Concediu de odihnă.
    Leave,
}

impl Shift {
    pub const HOURS: u32 = 24;

    fn word(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Shift::Shift, Lang::Ro) => "tură",
            (Shift::Extra, Lang::Ro) => "tură extra",
            (Shift::Leave, Lang::Ro) => "concediu",
            (Shift::Shift, Lang::En) => "shift",
            (Shift::Extra, Lang::En) => "extra shift",
            (Shift::Leave, Lang::En) => "leave",
        }
    }

    fn parse(w: &str) -> Option<Shift> {
        match crate::search::fold(w.trim()).as_str() {
            "tura" | "shift" => Some(Shift::Shift),
            "tura extra" | "extra" | "extra shift" => Some(Shift::Extra),
            "concediu" | "co" | "leave" => Some(Shift::Leave),
            _ => None,
        }
    }
}

/// Luna de monitorizare: zilele de serviciu și restul fișierului, păstrat așa cum e.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracking {
    pub work: BTreeMap<u32, Shift>,
    /// Alte secțiuni („## Somn” etc.), ca text, ca să nu se piardă la scriere.
    pub other: Vec<String>,
}

/// Totalurile lunii.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub shifts: u32,
    pub extra: u32,
    pub leave: u32,
}

impl Totals {
    pub fn hours(&self) -> u32 {
        (self.shifts + self.extra) * Shift::HOURS
    }
}

fn is_work_heading(h: &str) -> bool {
    matches!(crate::search::fold(h.trim()).as_str(), "serviciu" | "work")
}

impl Tracking {
    pub fn parse(text: &str) -> Tracking {
        let mut t = Tracking::default();
        let mut in_work = false;
        let mut other = Vec::new();
        for line in text.lines() {
            if line.starts_with("# ") {
                continue;
            }
            if let Some(h) = line.strip_prefix("## ") {
                in_work = is_work_heading(h);
                if !in_work {
                    other.push(line.to_string());
                }
                continue;
            }
            if in_work {
                let Some(rest) = line.trim().strip_prefix("- ") else { continue };
                let (days, word) = rest.split_once(' ').unwrap_or((rest, ""));
                let Some(kind) = Shift::parse(word) else { continue };
                let (a, b) = match days.split_once('-') {
                    Some((a, b)) => (a.parse::<u32>().ok(), b.parse::<u32>().ok()),
                    None => (days.parse::<u32>().ok(), days.parse::<u32>().ok()),
                };
                if let (Some(a), Some(b)) = (a, b) {
                    for d in a.min(b)..=a.max(b).min(31) {
                        if d >= 1 {
                            t.work.insert(d, kind);
                        }
                    }
                }
            } else {
                other.push(line.to_string());
            }
        }
        // Fără rândurile goale de la început și de la sfârșit.
        while other.first().is_some_and(|l| l.trim().is_empty()) {
            other.remove(0);
        }
        while other.last().is_some_and(|l| l.trim().is_empty()) {
            other.pop();
        }
        t.other = other;
        t
    }

    pub fn to_markdown(&self, year: i32, month: u32, lang: Lang) -> String {
        let mut out = format!("# {}\n\n## {}\n\n", lang.month_title(year, month), match lang {
            Lang::Ro => "Serviciu",
            Lang::En => "Work",
        });
        let days: Vec<u32> = self.work.keys().copied().collect();
        let mut i = 0;
        while i < days.len() {
            let kind = self.work[&days[i]];
            let mut j = i;
            while j + 1 < days.len() && days[j + 1] == days[j] + 1 && self.work[&days[j + 1]] == kind {
                j += 1;
            }
            if j > i {
                out.push_str(&format!("- {}-{} {}\n", days[i], days[j], kind.word(lang)));
            } else {
                out.push_str(&format!("- {} {}\n", days[i], kind.word(lang)));
            }
            i = j + 1;
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
                Shift::Extra => t.extra += 1,
                Shift::Leave => t.leave += 1,
            }
        }
        t
    }

    pub fn is_empty(&self) -> bool {
        self.work.is_empty() && self.other.is_empty()
    }
}

impl Journal {
    pub fn tracking_path(&self, year: i32, month: u32) -> PathBuf {
        self.root().join("monitorizare").join(format!("{year:04}-{month:02}.md"))
    }

    pub fn load_tracking(&self, year: i32, month: u32) -> io::Result<Tracking> {
        match self.read_text(&self.tracking_path(year, month)) {
            Ok(t) => Ok(Tracking::parse(&t)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Tracking::default()),
            Err(e) => Err(e),
        }
    }

    /// Schimbă ziua `day` (cu `None` o face liberă) și salvează luna.
    pub fn set_shift(&self, date: NaiveDate, kind: Option<Shift>, lang: Lang) -> io::Result<Tracking> {
        use chrono::Datelike;
        let (y, m) = (date.year(), date.month());
        let mut t = self.load_tracking(y, m)?;
        match kind {
            Some(k) => {
                t.work.insert(date.day(), k);
            }
            None => {
                t.work.remove(&date.day());
            }
        }
        let path = self.tracking_path(y, m);
        if t.is_empty() {
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        } else {
            self.write_text(&path, &t.to_markdown(y, m, lang))?;
        }
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUNA: &str = "# Noiembrie 2026\n\n## Serviciu\n\n- 2 tură\n- 12 tură extra\n- 15-17 concediu\n- 18 tură\n\n## Somn\n\n- 1 23:30-07:00\n";

    #[test]
    fn citire_si_scriere() {
        let t = Tracking::parse(LUNA);
        assert_eq!(t.work.len(), 6);
        assert_eq!(t.work[&16], Shift::Leave);
        assert_eq!(t.totals(), Totals { shifts: 2, extra: 1, leave: 3 });
        assert_eq!(t.totals().hours(), 72);
        assert_eq!(t.to_markdown(2026, 11, Lang::Ro), LUNA, "se scrie înapoi la fel, cu Somnul păstrat");
        // În engleză și fără diacritice se citește la fel.
        let en = Tracking::parse("# November 2026\n\n## Work\n\n- 2 shift\n- 3-4 leave\n- 9 extra shift\n- 10 tura\n");
        assert_eq!(en.totals(), Totals { shifts: 2, extra: 1, leave: 2 });
    }

    #[test]
    fn zile_schimbate_si_fisier_sters() {
        let root = std::env::temp_dir().join(format!("lanruj-monitor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let j = Journal::new(&root);
        let d = |n| NaiveDate::from_ymd_opt(2026, 11, n).unwrap();
        j.set_shift(d(2), Some(Shift::Shift), Lang::Ro).unwrap();
        j.set_shift(d(15), Some(Shift::Leave), Lang::Ro).unwrap();
        j.set_shift(d(16), Some(Shift::Leave), Lang::Ro).unwrap();
        let text = std::fs::read_to_string(j.tracking_path(2026, 11)).unwrap();
        assert!(text.contains("- 2 tură\n- 15-16 concediu\n"), "{text}");
        for n in [2, 15, 16] {
            j.set_shift(d(n), None, Lang::Ro).unwrap();
        }
        assert!(!j.tracking_path(2026, 11).exists(), "o lună goală nu lasă fișier");
        std::fs::remove_dir_all(root).unwrap();
    }
}
