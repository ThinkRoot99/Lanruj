// SPDX-License-Identifier: GPL-3.0-or-later
//! Luna: `2026/11/luna.md`, ca pagina dublă din caiet.
//!
//! ```text
//! # Noiembrie 2026
//!
//! ## Calendar
//!
//! - 3 Concert la Ateneu
//! - 15 Revizia mașinii
//!
//! ## Sarcini
//!
//! - [ ] Plătește impozitul
//! - [ ] * Programare la dentist
//! ```
//!
//! În calendar se scriu doar zilele cu text; aplicația arată toate zilele lunii.
//! Rândurile și secțiunile necunoscute se păstrează neschimbate.

use crate::day::{Item, item_line, parse_item};
use crate::lang::Lang;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalLine {
    /// `- 15 Revizia mașinii`
    Day(u32, String),
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Month {
    pub year: i32,
    pub month: u32,
    pub heading: Option<String>,
    /// Rândurile dinaintea primei secțiuni.
    pub prelude: Vec<String>,
    pub calendar: Vec<CalLine>,
    pub tasks: Vec<Item>,
    /// Alte secțiuni (`## …`), cu tot cu titlul lor, păstrate la sfârșit.
    pub extra: Vec<String>,
}

#[derive(PartialEq)]
enum Section {
    Prelude,
    Calendar,
    Tasks,
    Extra,
}

fn is_calendar(h: &str) -> bool {
    h.eq_ignore_ascii_case("calendar")
}

fn is_tasks(h: &str) -> bool {
    h.eq_ignore_ascii_case("sarcini") || h.eq_ignore_ascii_case("tasks")
}

fn parse_cal(line: &str) -> CalLine {
    if let Some(rest) = line.strip_prefix("- ") {
        let end = rest.find(' ').unwrap_or(rest.len());
        if let Ok(n) = rest[..end].parse::<u32>()
            && (1..=31).contains(&n)
        {
            return CalLine::Day(n, rest[end..].trim().to_string());
        }
    }
    CalLine::Other(line.to_string())
}

fn trim_blank_end(v: &mut Vec<String>) {
    while v.last().is_some_and(|s| s.trim().is_empty()) {
        v.pop();
    }
}

impl Month {
    pub fn new(year: i32, month: u32) -> Month {
        Month { year, month, heading: None, prelude: Vec::new(), calendar: Vec::new(), tasks: Vec::new(), extra: Vec::new() }
    }

    pub fn parse(year: i32, month: u32, text: &str) -> Month {
        let mut m = Month::new(year, month);
        let mut sec = Section::Prelude;
        let mut lines = text.lines().peekable();
        while lines.peek().is_some_and(|l| l.trim().is_empty()) {
            lines.next();
        }
        if let Some(h) = lines.peek().and_then(|l| l.strip_prefix("# ")) {
            m.heading = Some(h.trim().to_string());
            lines.next();
        }
        for line in lines {
            let line = line.trim_end();
            if let Some(h) = line.strip_prefix("## ") {
                let h = h.trim();
                sec = if is_calendar(h) && m.calendar.is_empty() {
                    Section::Calendar
                } else if is_tasks(h) && m.tasks.is_empty() {
                    Section::Tasks
                } else {
                    m.extra.push(line.to_string());
                    Section::Extra
                };
                continue;
            }
            match sec {
                Section::Prelude => m.prelude.push(line.to_string()),
                Section::Calendar => {
                    if !(line.is_empty() && m.calendar.is_empty()) {
                        m.calendar.push(parse_cal(line));
                    }
                }
                Section::Tasks => {
                    if !(line.is_empty() && m.tasks.is_empty()) {
                        m.tasks.push(parse_item(line));
                    }
                }
                Section::Extra => m.extra.push(line.to_string()),
            }
        }
        while matches!(m.calendar.last(), Some(CalLine::Other(s)) if s.trim().is_empty()) {
            m.calendar.pop();
        }
        while matches!(m.tasks.last(), Some(Item::Other(s)) if s.trim().is_empty()) {
            m.tasks.pop();
        }
        while m.prelude.first().is_some_and(|s| s.trim().is_empty()) {
            m.prelude.remove(0);
        }
        trim_blank_end(&mut m.prelude);
        trim_blank_end(&mut m.extra);
        m
    }

    pub fn to_markdown(&self, lang: Lang) -> String {
        let mut out = format!(
            "# {}\n\n",
            self.heading.clone().unwrap_or_else(|| lang.month_title(self.year, self.month))
        );
        if !self.prelude.is_empty() {
            for l in &self.prelude {
                out.push_str(l);
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str("## Calendar\n\n");
        for c in &self.calendar {
            match c {
                CalLine::Day(n, t) if t.is_empty() => out.push_str(&format!("- {n}")),
                CalLine::Day(n, t) => out.push_str(&format!("- {n} {t}")),
                CalLine::Other(s) => out.push_str(s),
            }
            out.push('\n');
        }
        if !self.calendar.is_empty() {
            out.push('\n');
        }
        out.push_str(match lang {
            Lang::Ro => "## Sarcini\n\n",
            Lang::En => "## Tasks\n\n",
        });
        for it in &self.tasks {
            out.push_str(&item_line(it));
            out.push('\n');
        }
        if !self.extra.is_empty() {
            out.push('\n');
            for l in &self.extra {
                out.push_str(l);
                out.push('\n');
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.prelude.iter().all(|s| s.trim().is_empty())
            && self.calendar.iter().all(|c| matches!(c, CalLine::Other(s) if s.trim().is_empty()))
            && self.tasks.iter().all(|i| matches!(i, Item::Other(s) if s.trim().is_empty()))
            && self.extra.iter().all(|s| s.trim().is_empty())
    }

    /// Textele zilei `day` din calendar (pot fi mai multe rânduri).
    pub fn day_texts(&self, day: u32) -> Vec<&str> {
        self.calendar
            .iter()
            .filter_map(|c| match c {
                CalLine::Day(n, t) if *n == day => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Înlocuiește textele zilei `day` cu `texts` (fără niciunul = ziua se golește),
    /// păstrând calendarul în ordinea zilelor.
    pub fn set_day(&mut self, day: u32, texts: &[String]) {
        let first = self.calendar.iter().position(|c| matches!(c, CalLine::Day(n, _) if *n == day));
        self.calendar.retain(|c| !matches!(c, CalLine::Day(n, _) if *n == day));
        let new: Vec<CalLine> = texts.iter().filter(|t| !t.trim().is_empty()).map(|t| CalLine::Day(day, t.trim().to_string())).collect();
        let at = first.unwrap_or_else(|| {
            self.calendar
                .iter()
                .position(|c| matches!(c, CalLine::Day(n, _) if *n > day))
                .unwrap_or(self.calendar.len())
        });
        let at = at.min(self.calendar.len());
        self.calendar.splice(at..at, new);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUNA: &str = "# Noiembrie 2026

## Calendar

- 3 Concert la Ateneu
- 15 Revizia mașinii
- 15 Ziua lui Andrei

## Sarcini

- [ ] Plătește impozitul
- [ ] * Programare la dentist
  - sună dimineața
- [x] Schimbă anvelopele

## Cheltuieli

- chirie 1 500 de lei
";

    #[test]
    fn citita_si_rescrisa_identic() {
        let m = Month::parse(2026, 11, LUNA);
        assert_eq!(m.to_markdown(Lang::Ro), LUNA);
        assert_eq!(m.day_texts(15), vec!["Revizia mașinii", "Ziua lui Andrei"]);
        assert_eq!(m.tasks.len(), 4);
        assert_eq!(m.extra[0], "## Cheltuieli");
    }

    #[test]
    fn luna_noua_si_zile() {
        let mut m = Month::new(2026, 11);
        assert!(m.is_empty());
        m.set_day(20, &["Cină".into()]);
        m.set_day(3, &["Concert".into()]);
        m.set_day(10, &["Ședință".into(), "Dentist".into()]);
        assert_eq!(
            m.to_markdown(Lang::Ro),
            "# Noiembrie 2026\n\n## Calendar\n\n- 3 Concert\n- 10 Ședință\n- 10 Dentist\n- 20 Cină\n\n## Sarcini\n\n"
        );
        m.set_day(10, &[]);
        assert_eq!(m.day_texts(10), Vec::<&str>::new());
        m.set_day(3, &["Concert mutat".into()]);
        assert_eq!(m.day_texts(3), vec!["Concert mutat"]);
        assert!(!m.is_empty());
    }

    #[test]
    fn engleza() {
        let text = "# November 2026\n\n## Calendar\n\n- 1 Trip\n\n## Tasks\n\n- [ ] Pay rent\n";
        let m = Month::parse(2026, 11, text);
        assert_eq!(m.to_markdown(Lang::En), text);
    }
}
