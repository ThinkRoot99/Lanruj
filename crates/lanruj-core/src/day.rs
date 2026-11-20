// SPDX-License-Identifier: GPL-3.0-or-later
//! O zi din jurnal: un fișier Markdown obișnuit.
//!
//! ```text
//! # Vineri, 20 noiembrie 2026
//!
//! - o 18:30 Cină cu familia
//! - [ ] Cumpără becuri pentru hol ^k3f9
//! - [ ] * Trimite raportul lunar
//! - [x] Plimbare în parc
//!   - frig, dar senin
//! - ! Idee pentru blog
//!
//! Etichete: carte, sănătate
//! Ziua: 4/5
//! ```
//!
//! Rândurile pe care nu le recunoaștem (paragrafe, poze, tabele) se păstrează
//! exact cum sunt, ca nimic scris de mână să nu se piardă.

use crate::lang::Lang;
use chrono::NaiveDate;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    /// `[ ]` •
    Open,
    /// `[x]` ×
    Done,
    /// `[>]` > mutată în altă zi
    Migrated,
    /// `[<]` < programată în viitor
    Scheduled,
    /// `[-]` renunțată (tăiată)
    Dropped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Task(TaskState),
    /// `o` ○
    Event,
    /// fără semn: –
    Note,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Priority {
    #[default]
    None,
    /// `*` important
    Important,
    /// `!` inspirație
    Inspiration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub kind: Kind,
    pub priority: Priority,
    /// Ora, pentru evenimente: „10:00”.
    pub time: Option<String>,
    pub text: String,
    /// Semnul `^k3f9` al sarcinilor migrate, ca să se numere mutările.
    pub id: Option<String>,
    /// 0 = intrare, 1 = sub o intrare, și așa mai departe.
    pub depth: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Entry(Entry),
    /// Un rând pe care nu-l interpretăm; se scrie înapoi neschimbat.
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Day {
    pub date: NaiveDate,
    /// Titlul din fișier, dacă există (fără „# ”).
    pub heading: Option<String>,
    pub items: Vec<Item>,
    pub tags: Vec<String>,
    /// Nota zilei, de la 1 la 5.
    pub rating: Option<u8>,
}

impl Entry {
    pub fn new(kind: Kind, text: impl Into<String>) -> Entry {
        Entry { kind, priority: Priority::None, time: None, text: text.into(), id: None, depth: 0 }
    }

    /// Intrarea scrisă ca în câmpul rapid, pentru editare: „. * text”, „o 10:00 text”, „- text”.
    pub fn to_quick(&self) -> String {
        let mut s = String::from(match self.kind {
            Kind::Task(_) => ". ",
            Kind::Event => "o ",
            Kind::Note => "- ",
        });
        match self.priority {
            Priority::Important => s.push_str("* "),
            Priority::Inspiration => s.push_str("! "),
            Priority::None => {}
        }
        if let Some(t) = &self.time {
            s.push_str(t);
            s.push(' ');
        }
        s.push_str(&self.text);
        s
    }

    pub fn is_open_task(&self) -> bool {
        self.kind == Kind::Task(TaskState::Open)
    }

    /// Rândul Markdown, fără indentare.
    pub fn to_line(&self) -> String {
        let mut s = String::from("- ");
        match self.kind {
            Kind::Task(st) => s.push_str(match st {
                TaskState::Open => "[ ] ",
                TaskState::Done => "[x] ",
                TaskState::Migrated => "[>] ",
                TaskState::Scheduled => "[<] ",
                TaskState::Dropped => "[-] ",
            }),
            Kind::Event => s.push_str("o "),
            Kind::Note => {}
        }
        match self.priority {
            Priority::Important => s.push_str("* "),
            Priority::Inspiration => s.push_str("! "),
            Priority::None => {}
        }
        if let Some(t) = &self.time {
            s.push_str(t);
            s.push(' ');
        }
        s.push_str(&self.text);
        if let Some(id) = &self.id {
            s.push_str(" ^");
            s.push_str(id);
        }
        s
    }

    /// Citește un rând `- …` (fără indentare). `None` dacă nu e o intrare.
    pub fn parse_line(line: &str) -> Option<Entry> {
        let rest = line.strip_prefix("- ").or_else(|| (line == "-").then_some(""))?;
        let (kind, mut rest) = parse_marker(rest);
        let mut priority = Priority::None;
        let mut time = None;
        // Prioritatea și ora pot veni în orice ordine: „o * 10:00 …” sau „o 10:00 * …”.
        for _ in 0..2 {
            if priority == Priority::None {
                if let Some(r) = rest.strip_prefix("* ") {
                    priority = Priority::Important;
                    rest = r;
                    continue;
                }
                if let Some(r) = rest.strip_prefix("! ") {
                    priority = Priority::Inspiration;
                    rest = r;
                    continue;
                }
            }
            if time.is_none() && kind == Kind::Event
                && let Some((t, r)) = split_time(rest) {
                    time = Some(t.to_string());
                    rest = r;
                    continue;
                }
            break;
        }
        let (text, id) = split_id(rest);
        Some(Entry { kind, priority, time, text: text.to_string(), id: id.map(str::to_string), depth: 0 })
    }
}

fn parse_marker(s: &str) -> (Kind, &str) {
    let task = |st| Kind::Task(st);
    for (m, k) in [
        ("[ ] ", task(TaskState::Open)),
        ("[x] ", task(TaskState::Done)),
        ("[X] ", task(TaskState::Done)),
        ("[>] ", task(TaskState::Migrated)),
        ("[<] ", task(TaskState::Scheduled)),
        ("[-] ", task(TaskState::Dropped)),
    ] {
        if let Some(r) = s.strip_prefix(m) {
            return (k, r);
        }
    }
    if let Some(r) = s.strip_prefix("o ") {
        return (Kind::Event, r);
    }
    (Kind::Note, s)
}

/// „10:00 rest” → („10:00”, „rest”)
fn split_time(s: &str) -> Option<(&str, &str)> {
    let end = s.find(' ').unwrap_or(s.len());
    let t = &s[..end];
    let (h, m) = t.split_once(':')?;
    let ok = (1..=2).contains(&h.len())
        && m.len() == 2
        && h.chars().all(|c| c.is_ascii_digit())
        && m.chars().all(|c| c.is_ascii_digit())
        && h.parse::<u32>().ok()? < 24
        && m.parse::<u32>().ok()? < 60;
    if !ok {
        return None;
    }
    Some((t, s[end..].strip_prefix(' ').unwrap_or("")))
}

/// „text ^k3f9” → („text”, Some(„k3f9”))
fn split_id(s: &str) -> (&str, Option<&str>) {
    if let Some(pos) = s.rfind(" ^") {
        let id = &s[pos + 2..];
        if (4..=8).contains(&id.len()) && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
            return (&s[..pos], Some(id));
        }
    }
    (s, None)
}

fn indent_of(line: &str) -> (usize, &str) {
    let mut spaces = 0;
    let mut bytes = 0;
    for c in line.chars() {
        match c {
            ' ' => spaces += 1,
            '\t' => spaces += 2,
            _ => break,
        }
        bytes += 1;
    }
    (spaces / 2, &line[bytes..])
}

/// Un rând din fișier: o intrare (cu indentarea ei) sau altceva, păstrat neschimbat.
pub fn parse_item(line: &str) -> Item {
    let line = line.trim_end();
    let (depth, rest) = indent_of(line);
    match Entry::parse_line(rest) {
        Some(mut e) => {
            e.depth = depth;
            Item::Entry(e)
        }
        None => Item::Other(line.to_string()),
    }
}

/// Rândul din fișier pentru o intrare (cu indentare) sau pentru un rând păstrat.
pub fn item_line(item: &Item) -> String {
    match item {
        Item::Entry(e) => format!("{}{}", "  ".repeat(e.depth), e.to_line()),
        Item::Other(s) => s.clone(),
    }
}

fn parse_tags(s: &str) -> Vec<String> {
    s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

fn parse_rating(s: &str) -> Option<u8> {
    let (n, total) = s.trim().split_once('/')?;
    if total.trim() != "5" {
        return None;
    }
    n.trim().parse::<u8>().ok().filter(|n| (1..=5).contains(n))
}

impl Day {
    pub fn new(date: NaiveDate) -> Day {
        Day { date, heading: None, items: Vec::new(), tags: Vec::new(), rating: None }
    }

    pub fn parse(date: NaiveDate, text: &str) -> Day {
        let mut day = Day::new(date);
        let mut lines = text.lines().peekable();
        // Rândurile goale de la început și titlul.
        while lines.peek().is_some_and(|l| l.trim().is_empty()) {
            lines.next();
        }
        if let Some(h) = lines.peek().and_then(|l| l.strip_prefix("# ")) {
            day.heading = Some(h.trim().to_string());
            lines.next();
            if lines.peek().is_some_and(|l| l.trim().is_empty()) {
                lines.next();
            }
        }
        for line in lines {
            let line = line.trim_end();
            if let Some(v) = line.strip_prefix("Etichete:").or_else(|| line.strip_prefix("Tags:")) {
                day.tags = parse_tags(v);
                continue;
            }
            if let Some(v) = line.strip_prefix("Ziua:").or_else(|| line.strip_prefix("Day:"))
                && let Some(r) = parse_rating(v) {
                    day.rating = Some(r);
                    continue;
                }
            day.items.push(parse_item(line));
        }
        day.trim_trailing_blank();
        day
    }

    fn trim_trailing_blank(&mut self) {
        while matches!(self.items.last(), Some(Item::Other(s)) if s.trim().is_empty()) {
            self.items.pop();
        }
    }

    pub fn to_markdown(&self, lang: Lang) -> String {
        let mut out = String::new();
        out.push_str("# ");
        match &self.heading {
            Some(h) => out.push_str(h),
            None => out.push_str(&lang.day_title(self.date)),
        }
        out.push_str("\n\n");
        let mut body_end = self.items.len();
        while body_end > 0 && matches!(&self.items[body_end - 1], Item::Other(s) if s.trim().is_empty()) {
            body_end -= 1;
        }
        for item in &self.items[..body_end] {
            out.push_str(&item_line(item));
            out.push('\n');
        }
        if !self.tags.is_empty() || self.rating.is_some() {
            if body_end > 0 {
                out.push('\n');
            }
            if !self.tags.is_empty() {
                out.push_str(&format!("{}: {}\n", lang.tags_label(), self.tags.join(", ")));
            }
            if let Some(r) = self.rating {
                out.push_str(&format!("{}: {}/5\n", lang.rating_label(), r));
            }
        }
        out
    }

    /// Adevărat dacă ziua nu are nimic scris (nu merită un fișier).
    pub fn is_empty(&self) -> bool {
        self.items.iter().all(|i| matches!(i, Item::Other(s) if s.trim().is_empty()))
            && self.tags.is_empty()
            && self.rating.is_none()
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.items.iter().filter_map(|i| match i {
            Item::Entry(e) => Some(e),
            Item::Other(_) => None,
        })
    }

    pub fn entry_mut(&mut self, index: usize) -> Option<&mut Entry> {
        match self.items.get_mut(index) {
            Some(Item::Entry(e)) => Some(e),
            _ => None,
        }
    }

    pub fn open_tasks(&self) -> usize {
        self.entries().filter(|e| e.depth == 0 && e.is_open_task()).count()
    }

    /// Câte intrări de sub intrarea `index` (notițele ei) îi aparțin.
    pub fn children_count(&self, index: usize) -> usize {
        let Some(Item::Entry(parent)) = self.items.get(index) else { return 0 };
        self.items[index + 1..]
            .iter()
            .take_while(|it| matches!(it, Item::Entry(e) if e.depth > parent.depth))
            .count()
    }

    /// Șterge intrarea `index` împreună cu notițele de sub ea.
    pub fn remove(&mut self, index: usize) -> bool {
        if !matches!(self.items.get(index), Some(Item::Entry(_))) {
            return false;
        }
        let n = 1 + self.children_count(index);
        self.items.drain(index..index + n);
        true
    }

    /// Înlocuiește intrarea `index` cu ce s-a scris în câmpul de editare
    /// (aceeași scriere ca jos: `.`, `o`, `-`). Păstrează nivelul și semnul `^id`;
    /// o sarcină rămâne în starea ei (făcută rămâne făcută).
    pub fn edit(&mut self, index: usize, input: &str) -> bool {
        let Some(new) = parse_quick(input) else { return false };
        let Some(Item::Entry(old)) = self.items.get_mut(index) else { return false };
        let kind = match (old.kind, new.kind) {
            (Kind::Task(st), Kind::Task(_)) => Kind::Task(st),
            (_, k) => k,
        };
        *old = Entry { kind, depth: old.depth, id: old.id.clone(), ..new };
        true
    }

    pub fn push(&mut self, entry: Entry) {
        self.trim_trailing_blank();
        self.items.push(Item::Entry(entry));
    }
}

/// Scrierea rapidă: `. ` sarcină, `o ` eveniment, `- ` notă.
/// Fără semn, rândul devine notă. `*` și `!` pot urma semnul.
pub fn parse_quick(input: &str) -> Option<Entry> {
    let s = input.trim();
    // Un semn singur („.”, „o”, „-”, „•”) nu e încă o intrare.
    if s.is_empty() || matches!(s, "." | "o" | "-" | "•" | "*" | "!") {
        return None;
    }
    let line = if let Some(r) = s.strip_prefix(". ").or_else(|| s.strip_prefix('•').map(str::trim_start)) {
        format!("- [ ] {r}")
    } else if let Some(r) = s.strip_prefix("o ") {
        format!("- o {r}")
    } else if let Some(r) = s.strip_prefix("- ") {
        format!("- {r}")
    } else {
        format!("- {s}")
    };
    let mut e = Entry::parse_line(&line)?;
    e.text = e.text.trim().to_string();
    (!e.text.is_empty()).then_some(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
    }

    const EXEMPLU: &str = "# Vineri, 20 noiembrie 2026

- o 18:30 Cină cu familia
- [ ] Cumpără becuri pentru hol ^k3f9
- [ ] * Trimite raportul lunar
- [x] Plimbare în parc, 30 de minute
  - frig, dar senin
- Am terminat cartea începută luna trecută
- ! Un articol despre Syncthing
- [>] Sună la service ^ab12
- [<] Programare la dentist
- [-] Abonament la sală

Un paragraf scris de mână, care trebuie păstrat.
![](2026-11-20/balea.jpg)

Etichete: carte, sănătate
Ziua: 4/5
";

    #[test]
    fn citit_si_rescris_identic() {
        let day = Day::parse(d(), EXEMPLU);
        assert_eq!(day.to_markdown(Lang::Ro), EXEMPLU);
    }

    #[test]
    fn semnele() {
        let day = Day::parse(d(), EXEMPLU);
        let e: Vec<_> = day.entries().collect();
        assert_eq!(e[0].kind, Kind::Event);
        assert_eq!(e[0].time.as_deref(), Some("18:30"));
        assert_eq!(e[0].text, "Cină cu familia");
        assert_eq!(e[1].id.as_deref(), Some("k3f9"));
        assert_eq!(e[1].text, "Cumpără becuri pentru hol");
        assert_eq!(e[2].priority, Priority::Important);
        assert_eq!(e[3].kind, Kind::Task(TaskState::Done));
        assert_eq!(e[4].depth, 1);
        assert_eq!(e[4].kind, Kind::Note);
        assert_eq!(e[6].priority, Priority::Inspiration);
        assert_eq!(e[7].kind, Kind::Task(TaskState::Migrated));
        assert_eq!(e[8].kind, Kind::Task(TaskState::Scheduled));
        assert_eq!(e[9].kind, Kind::Task(TaskState::Dropped));
        assert_eq!(day.tags, vec!["carte", "sănătate"]);
        assert_eq!(day.rating, Some(4));
        assert_eq!(day.open_tasks(), 2);
    }

    #[test]
    fn engleza() {
        let text = "# Friday, 20 November 2026\n\n- [ ] Buy bulbs\n\nTags: home\nDay: 3/5\n";
        let day = Day::parse(d(), text);
        assert_eq!(day.tags, vec!["home"]);
        assert_eq!(day.rating, Some(3));
        assert_eq!(day.to_markdown(Lang::En), text);
    }

    #[test]
    fn titlu_generat() {
        let mut day = Day::new(d());
        day.push(Entry::new(Kind::Task(TaskState::Open), "Ceva"));
        assert_eq!(day.to_markdown(Lang::Ro), "# Vineri, 20 noiembrie 2026\n\n- [ ] Ceva\n");
    }

    #[test]
    fn ora_doar_la_evenimente() {
        let e = Entry::parse_line("- 10:00 nu e eveniment").unwrap();
        assert_eq!(e.kind, Kind::Note);
        assert_eq!(e.time, None);
        assert_eq!(e.text, "10:00 nu e eveniment");
        let e = Entry::parse_line("- o * 25:00 ora greșită").unwrap();
        assert_eq!(e.time, None);
        assert_eq!(e.priority, Priority::Important);
    }

    #[test]
    fn scriere_rapida() {
        let e = parse_quick(". * Trimite raportul").unwrap();
        assert_eq!(e.kind, Kind::Task(TaskState::Open));
        assert_eq!(e.priority, Priority::Important);
        assert_eq!(e.text, "Trimite raportul");
        let e = parse_quick("o 9:30 Dentist").unwrap();
        assert_eq!(e.kind, Kind::Event);
        assert_eq!(e.time.as_deref(), Some("9:30"));
        let e = parse_quick("- ! Idee").unwrap();
        assert_eq!(e.kind, Kind::Note);
        assert_eq!(e.priority, Priority::Inspiration);
        let e = parse_quick("Doar text").unwrap();
        assert_eq!(e.kind, Kind::Note);
        assert!(parse_quick("   ").is_none());
        assert!(parse_quick(". ").is_none());
    }

    #[test]
    fn editeaza_si_sterge() {
        let mut day = Day::parse(d(), EXEMPLU);
        let e: Vec<_> = day.entries().cloned().collect();
        assert_eq!(e[0].to_quick(), "o 18:30 Cină cu familia");
        assert_eq!(e[2].to_quick(), ". * Trimite raportul lunar");
        assert_eq!(e[6].to_quick(), "- ! Un articol despre Syncthing");

        // Sarcina făcută rămâne făcută; semnul ^id rămâne.
        assert!(day.edit(1, ". Cumpără două becuri"));
        assert_eq!(day.items[1], Item::Entry(Entry { text: "Cumpără două becuri".into(), id: Some("k3f9".into()), ..Entry::new(Kind::Task(TaskState::Open), "") }));
        assert!(day.edit(3, ". * Plimbare lungă"));
        let Item::Entry(p) = &day.items[3] else { panic!() };
        assert_eq!(p.kind, Kind::Task(TaskState::Done));
        assert_eq!(p.priority, Priority::Important);
        // O notă devenită eveniment.
        assert!(day.edit(6, "o 20:00 Film"));
        let Item::Entry(f) = &day.items[6] else { panic!() };
        assert_eq!((f.kind, f.time.as_deref(), f.text.as_str()), (Kind::Event, Some("20:00"), "Film"));
        assert!(!day.edit(6, "   "), "un rând gol nu șterge intrarea");

        // Ștergerea ia și notița de dedesubt.
        let before = day.items.len();
        assert_eq!(day.children_count(3), 1);
        assert!(day.remove(3));
        assert_eq!(day.items.len(), before - 2);
        assert!(!day.remove(100));
    }

    #[test]
    fn zi_goala() {
        assert!(Day::parse(d(), "# Titlu\n\n\n").is_empty());
        assert!(!Day::parse(d(), EXEMPLU).is_empty());
    }
}
