// SPDX-License-Identifier: GPL-3.0-or-later
//! Jurnalul viitor: `viitor.md`, câte o secțiune pe lună.
//!
//! ```text
//! # Jurnalul viitor
//!
//! ## Decembrie 2026
//!
//! - [ ] 15 Revizia mașinii
//! - o 24 Crăciun la părinți
//! - [ ] Reînnoiește abonamentul
//!
//! ## Ianuarie 2027
//!
//! - o 7 Ziua lui Ion
//! ```
//!
//! Ziua, dacă se știe, stă la începutul textului. Pe 1 ale lunii, intrările
//! lunii trec în pagina ei (vezi `Journal::transfer_future`).

use crate::day::{Item, item_line, parse_item};
use crate::lang::Lang;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// Luna secțiunii, dacă titlul e o lună („Decembrie 2026” / „December 2026”).
    pub month: Option<(i32, u32)>,
    pub heading: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Future {
    pub heading: Option<String>,
    pub prelude: Vec<String>,
    pub sections: Vec<Section>,
}

const LUNI_RO: [&str; 12] = [
    "ianuarie", "februarie", "martie", "aprilie", "mai", "iunie", "iulie", "august", "septembrie",
    "octombrie", "noiembrie", "decembrie",
];
const LUNI_EN: [&str; 12] = [
    "january", "february", "march", "april", "may", "june", "july", "august", "september", "october",
    "november", "december",
];

/// „Decembrie 2026” sau „December 2026” → (2026, 12).
pub fn parse_month_heading(h: &str) -> Option<(i32, u32)> {
    let mut parts = h.split_whitespace();
    let name = parts.next()?.to_lowercase();
    let year: i32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let m = LUNI_RO.iter().position(|n| *n == name).or_else(|| LUNI_EN.iter().position(|n| *n == name))?;
    Some((year, m as u32 + 1))
}

/// „15 Revizia mașinii” → (Some(15), „Revizia mașinii”).
pub fn split_day(text: &str) -> (Option<u32>, &str) {
    let end = text.find(' ').unwrap_or(text.len());
    match text[..end].parse::<u32>() {
        Ok(n) if (1..=31).contains(&n) => (Some(n), text[end..].trim_start()),
        _ => (None, text),
    }
}

fn trim_blank(items: &mut Vec<Item>) {
    while matches!(items.last(), Some(Item::Other(s)) if s.trim().is_empty()) {
        items.pop();
    }
    while matches!(items.first(), Some(Item::Other(s)) if s.trim().is_empty()) {
        items.remove(0);
    }
}

impl Future {
    pub fn parse(text: &str) -> Future {
        let mut f = Future::default();
        let mut lines = text.lines().peekable();
        while lines.peek().is_some_and(|l| l.trim().is_empty()) {
            lines.next();
        }
        if let Some(h) = lines.peek().and_then(|l| l.strip_prefix("# ")) {
            f.heading = Some(h.trim().to_string());
            lines.next();
        }
        for line in lines {
            let line = line.trim_end();
            if let Some(h) = line.strip_prefix("## ") {
                f.sections.push(Section { month: parse_month_heading(h.trim()), heading: h.trim().to_string(), items: Vec::new() });
                continue;
            }
            match f.sections.last_mut() {
                Some(sec) if sec.month.is_some() => sec.items.push(parse_item(line)),
                Some(sec) => sec.items.push(Item::Other(line.to_string())),
                None => f.prelude.push(line.to_string()),
            }
        }
        for s in &mut f.sections {
            trim_blank(&mut s.items);
        }
        while f.prelude.last().is_some_and(|s| s.trim().is_empty()) {
            f.prelude.pop();
        }
        f
    }

    pub fn to_markdown(&self, lang: Lang) -> String {
        let title = self.heading.clone().unwrap_or_else(|| {
            match lang {
                Lang::Ro => "Planuri",
                Lang::En => "Future log",
            }
            .to_string()
        });
        let mut out = format!("# {title}\n\n");
        if !self.prelude.is_empty() {
            for l in &self.prelude {
                out.push_str(l);
                out.push('\n');
            }
            out.push('\n');
        }
        let n = self.sections.len();
        for (i, s) in self.sections.iter().enumerate() {
            out.push_str(&format!("## {}\n\n", s.heading));
            for it in &s.items {
                out.push_str(&item_line(it));
                out.push('\n');
            }
            if i + 1 < n && !s.items.is_empty() {
                out.push('\n');
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.prelude.iter().all(|s| s.trim().is_empty())
            && self.sections.iter().all(|s| s.items.iter().all(|i| matches!(i, Item::Other(t) if t.trim().is_empty())))
    }

    pub fn items(&self, year: i32, month: u32) -> Vec<Item> {
        self.sections.iter().find(|s| s.month == Some((year, month))).map(|s| s.items.clone()).unwrap_or_default()
    }

    /// Pune intrările lunii; creează secțiunea la locul ei, în ordinea lunilor,
    /// și o scoate dacă rămâne goală.
    pub fn set_items(&mut self, year: i32, month: u32, items: Vec<Item>, lang: Lang) {
        let empty = items.iter().all(|i| matches!(i, Item::Other(t) if t.trim().is_empty()));
        if let Some(pos) = self.sections.iter().position(|s| s.month == Some((year, month))) {
            if empty {
                self.sections.remove(pos);
            } else {
                self.sections[pos].items = items;
            }
            return;
        }
        if empty {
            return;
        }
        let at = self
            .sections
            .iter()
            .position(|s| s.month.is_some_and(|m| m > (year, month)))
            .unwrap_or_else(|| self.sections.iter().rposition(|s| s.month.is_some()).map(|p| p + 1).unwrap_or(0));
        self.sections.insert(at, Section { month: Some((year, month)), heading: lang.month_title(year, month), items });
    }

    /// Lunile care au o secțiune.
    pub fn months(&self) -> Vec<(i32, u32)> {
        self.sections.iter().filter_map(|s| s.month).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIITOR: &str = "# Jurnalul viitor

## Decembrie 2026

- [ ] 15 Revizia mașinii
- o 24 Crăciun la părinți
- [ ] Reînnoiește abonamentul

## Ianuarie 2027

- o 7 Ziua lui Ion

## Idei de cadouri

- o carte pentru tata
";

    #[test]
    fn citit_si_rescris_identic() {
        let f = Future::parse(VIITOR);
        assert_eq!(f.to_markdown(Lang::Ro), VIITOR);
        assert_eq!(f.months(), vec![(2026, 12), (2027, 1)]);
        assert_eq!(f.items(2026, 12).len(), 3);
        assert_eq!(f.sections[2].month, None);
    }

    #[test]
    fn luni_si_zile() {
        assert_eq!(parse_month_heading("Decembrie 2026"), Some((2026, 12)));
        assert_eq!(parse_month_heading("march 2027"), Some((2027, 3)));
        assert_eq!(parse_month_heading("Idei de cadouri"), None);
        assert_eq!(split_day("15 Revizia"), (Some(15), "Revizia"));
        assert_eq!(split_day("2026 nu e zi"), (None, "2026 nu e zi"));
        assert_eq!(split_day("Fără zi"), (None, "Fără zi"));
    }

    #[test]
    fn sectiune_noua_in_ordine_si_scoasa_cand_e_goala() {
        let mut f = Future::parse(VIITOR);
        f.set_items(2027, 3, vec![parse_item("- [ ] Vacanța")], Lang::Ro);
        f.set_items(2027, 2, vec![parse_item("- o 14 Cină")], Lang::Ro);
        assert_eq!(f.months(), vec![(2026, 12), (2027, 1), (2027, 2), (2027, 3)]);
        assert_eq!(f.sections[3].heading, "Martie 2027");
        f.set_items(2027, 1, vec![], Lang::Ro);
        assert_eq!(f.months(), vec![(2026, 12), (2027, 2), (2027, 3)]);
        let text = f.to_markdown(Lang::Ro);
        assert!(text.contains("## Februarie 2027\n\n- o 14 Cină\n\n## Martie 2027"));
        assert_eq!(Future::parse(&text), f);
    }
}
