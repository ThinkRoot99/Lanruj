// SPDX-License-Identifier: GPL-3.0-or-later
//! Jurnalul propriu-zis: textul liber al fiecărei zile, scris pe larg, ca într-un caiet.
//! Fiecare zi are fișierul ei, `jurnal/AAAA/AAAA-LL-ZZ.md`, în Markdown obișnuit:
//!
//! ```text
//! # Jurnal · 21 noiembrie 2026
//!
//! ### 08:30
//! Am dormit **bine**, iar dimineața a fost *liniștită*.
//! Am trecut pe la [piața cu mama](lanruj:intrare/zi/2026-11-21/k3f9).
//!
//! ## Gânduri
//! > Un citat care mi-a rămas în minte.
//! - o listă
//! - [ ] ceva de verificat
//! ```
//!
//! Scrisul: `**îngroșat**`, `*cursiv*`, `~~tăiat~~`, `<u>subliniat</u>`, culorile și
//! markerul ca în pagini (`<span style="color:…">`, `==…==`, `<mark style="background:…">`),
//! legături `[text](adresă)`. Rândurile: `### 08:30` (ora unei scrieri), `## Titlu`, `> citat`,
//! liste `- `, `1. `, `- [ ] ` / `- [x] `.
//!
//! Fontul unei bucăți: `<span style="font-family:Caveat">…</span>`; mărimea: `<span style="font-size:125%">…</span>`.
//!
//! Legăturile spre restul jurnalului încep cu `lanruj:` (vezi [`Target`]).

use crate::journal::Journal;
use crate::{Lang, PageRef};
use chrono::{Datelike, NaiveDate};
use std::io;
use std::path::PathBuf;

/// Cum arată o bucată de text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub underline: bool,
    pub color: Option<String>,
    pub marker: Option<String>,
    pub link: Option<String>,
    /// Fontul bucății (familia), dacă e altul decât cel din setări.
    pub font: Option<String>,
    /// Mărimea bucății, în procente din mărimea jurnalului (125 = mai mare), dacă nu e cea obișnuită.
    pub size: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// Felul unui rând.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineKind {
    #[default]
    Text,
    /// Ora unei scrieri: `### 08:30`.
    Time,
    /// Un titlu: `## …`.
    Title,
    /// Un subtitlu: `### …` (un `### ` urmat doar de o oră, `### 08:30`, e ora unei scrieri).
    Subtitle,
    /// Un citat: `> …`.
    Quote,
}

/// Un rând al jurnalului. Listele își păstrează semnul în text, așa cum se vede pe ecran:
/// `• `, `☐ `, `☑ `, `1. ` (cu spațiile din față pentru liste în listă).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub kind: LineKind,
    pub spans: Vec<Span>,
}

impl Line {
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}

fn push(out: &mut Vec<Span>, c: &str, style: &Style) {
    match out.last_mut() {
        Some(l) if l.style == *style => l.text.push_str(c),
        _ => out.push(Span { text: c.to_string(), style: style.clone() }),
    }
}

/// O adresă pe care o acceptăm într-o legătură: web, e-mail, fișier sau `lanruj:`.
pub fn is_link(s: &str) -> bool {
    crate::colors::is_url(s) || (s.starts_with("lanruj:") && !s.contains(char::is_whitespace) && s.len() > 7)
}

/// Bucățile de text cu stilul lor, dintr-un rând de Markdown.
pub fn parse_inline(text: &str) -> Vec<Span> {
    let mut out = Vec::new();
    parse_into(text, Style::default(), &mut out);
    out
}

/// `<span style="font-family:Patrick Hand">` → („Patrick Hand”, lungimea etichetei).
fn font_tag(s: &str) -> Option<(String, usize)> {
    let rest = s.strip_prefix("<span style=\"font-family:")?;
    let end = rest.find("\">")?;
    let name = rest[..end].trim().trim_end_matches(';').trim().trim_matches(['\'', '"']).trim();
    let ok = !name.is_empty() && name.len() <= 60 && name.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-');
    ok.then(|| (name.to_string(), "<span style=\"font-family:".len() + end + 2))
}

/// `<span style="font-size:125%">` → (125, lungimea etichetei).
fn size_tag(s: &str) -> Option<(u32, usize)> {
    let rest = s.strip_prefix("<span style=\"font-size:")?;
    let end = rest.find("\">")?;
    let v: u32 = rest[..end].trim().trim_end_matches(';').trim().strip_suffix('%')?.trim().parse().ok()?;
    (40..=400).contains(&v).then_some((v, "<span style=\"font-size:".len() + end + 2))
}

/// O etichetă `<span>` deschisă: culoare, font sau mărime (`</span>` o închide pe ultima).
enum Open {
    Color(String),
    Font(String),
    Size(u32),
}

fn parse_into(text: &str, base: Style, out: &mut Vec<Span>) {
    let mut st = base.clone();
    let mut spans: Vec<Open> = Vec::new();
    let mut marks: Vec<String> = Vec::new();
    let mut eq = false;
    let mut i = 0;
    let set_paint = |st: &mut Style, spans: &[Open], marks: &[String], eq: bool| {
        st.color = spans.iter().rev().find_map(|o| if let Open::Color(c) = o { Some(c.clone()) } else { None }).or_else(|| base.color.clone());
        st.font = spans.iter().rev().find_map(|o| if let Open::Font(f) = o { Some(f.clone()) } else { None }).or_else(|| base.font.clone());
        st.size = spans.iter().rev().find_map(|o| if let Open::Size(z) = o { Some(*z) } else { None }).or(base.size);
        st.marker = marks.last().cloned().or_else(|| eq.then(|| crate::colors::YELLOW_MARKER.to_string())).or_else(|| base.marker.clone());
    };
    while i < text.len() {
        let rest = &text[i..];
        let c = rest.chars().next().expect("caracter");
        if c == '\\'
            && let Some(n) = rest[1..].chars().next()
            && n.is_ascii_punctuation()
        {
            push(out, &n.to_string(), &st);
            i += 1 + n.len_utf8();
            continue;
        }
        // Un semn deschide doar dacă are pereche mai încolo (un „a == b” scris de mână rămâne text).
        let opens = |on: bool, tok: &str| on || rest[tok.len()..].contains(tok);
        if rest.starts_with("**") && opens(st.bold != base.bold, "**") {
            st.bold = !st.bold;
            i += 2;
        } else if rest.starts_with("~~") && opens(st.strike != base.strike, "~~") {
            st.strike = !st.strike;
            i += 2;
        } else if rest.starts_with("==") && opens(eq, "==") {
            eq = !eq;
            set_paint(&mut st, &spans, &marks, eq);
            i += 2;
        } else if c == '*' && opens(st.italic != base.italic, "*") {
            st.italic = !st.italic;
            i += 1;
        } else if rest.starts_with("<u>") {
            st.underline = true;
            i += 3;
        } else if rest.starts_with("</u>") {
            st.underline = base.underline;
            i += 4;
        } else if rest.starts_with("</span>") {
            spans.pop();
            set_paint(&mut st, &spans, &marks, eq);
            i += 7;
        } else if rest.starts_with("</mark>") {
            marks.pop();
            set_paint(&mut st, &spans, &marks, eq);
            i += 7;
        } else if let Some((z, len)) = size_tag(rest) {
            spans.push(Open::Size(z));
            set_paint(&mut st, &spans, &marks, eq);
            i += len;
        } else if let Some((name, len)) = font_tag(rest) {
            spans.push(Open::Font(name));
            set_paint(&mut st, &spans, &marks, eq);
            i += len;
        } else if let Some((tag, value, len)) = crate::colors::open_tag(rest) {
            if tag == "span" { spans.push(Open::Color(value)) } else { marks.push(value) }
            set_paint(&mut st, &spans, &marks, eq);
            i += len;
        } else if let Some((content, url, len)) = md_link(rest) {
            let mut inner = st.clone();
            inner.link = Some(url.to_string());
            parse_into(content, inner, out);
            i += len;
        } else {
            push(out, &c.to_string(), &st);
            i += c.len_utf8();
        }
    }
}

/// `[conținut](adresă)` la începutul lui `s`; `\]` în conținut nu închide.
fn md_link(s: &str) -> Option<(&str, &str, usize)> {
    let rest = s.strip_prefix('[')?;
    let b = rest.as_bytes();
    let mut k = 0;
    let close = loop {
        if k + 1 >= b.len() {
            return None;
        }
        if b[k] == b'\\' {
            k += 2;
            continue;
        }
        if b[k] == b'[' {
            return None;
        }
        if b[k] == b']' {
            if b[k + 1] == b'(' {
                break k;
            }
            return None;
        }
        k += 1;
    };
    let after = &rest[close + 2..];
    let end = after.find(')')?;
    let url = &after[..end];
    is_link(url).then_some((&rest[..close], url, 1 + close + 2 + end + 1))
}

fn escape(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    for (k, &c) in chars.iter().enumerate() {
        let next = chars.get(k + 1).copied();
        let needs = match c {
            '\\' | '*' | '[' | ']' | '<' => true,
            '=' | '~' => next.is_none_or(|n| n == c) || k == 0,
            _ => false,
        };
        if needs {
            s.push('\\');
        }
        s.push(c);
    }
    s
}

/// Comutatoarele dintre două stiluri (`**`, `*`, `~~`), cele lungi întâi.
fn toggles(from: &Style, to: &Style) -> String {
    let mut s = String::new();
    if from.bold != to.bold {
        s.push_str("**");
    }
    if from.italic != to.italic {
        s.push('*');
    }
    if from.strike != to.strike {
        s.push_str("~~");
    }
    s
}

fn plain(st: &Style) -> Style {
    Style { bold: st.bold, italic: st.italic, strike: st.strike, ..Style::default() }
}

/// Bucățile, înapoi în Markdown.
pub fn to_inline(spans: &[Span]) -> String {
    let mut out = String::new();
    let mut cur = Style::default();
    let mut k = 0;
    while k < spans.len() {
        let link = spans[k].style.link.clone();
        let mut j = k;
        while j < spans.len() && spans[j].style.link == link {
            j += 1;
        }
        if let Some(url) = &link {
            // Legătura începe și se termină cu toate comutatoarele închise.
            out.push_str(&toggles(&cur, &Style::default()));
            cur = Style::default();
            out.push('[');
            out.push_str(&write_run(&spans[k..j], &mut cur));
            out.push_str(&toggles(&cur, &Style::default()));
            cur = Style::default();
            out.push_str("](");
            out.push_str(url);
            out.push(')');
        } else {
            out.push_str(&write_run(&spans[k..j], &mut cur));
        }
        k = j;
    }
    out.push_str(&toggles(&cur, &Style::default()));
    out
}

/// Bucățile vecine cu același font și aceeași mărime stau sub o singură pereche de etichete.
fn write_run(spans: &[Span], cur: &mut Style) -> String {
    let mut out = String::new();
    let mut k = 0;
    while k < spans.len() {
        let key = (spans[k].style.font.clone(), spans[k].style.size);
        let mut j = k;
        while j < spans.len() && (spans[j].style.font.clone(), spans[j].style.size) == key {
            j += 1;
        }
        if key == (None, None) {
            out.push_str(&write_plain(&spans[k..j], cur));
        } else {
            out.push_str(&toggles(cur, &Style::default()));
            *cur = Style::default();
            let mut open = String::new();
            let mut close = String::new();
            if let Some(f) = &key.0 {
                open.push_str(&format!("<span style=\"font-family:{f}\">"));
                close.push_str("</span>");
            }
            if let Some(z) = key.1 {
                open.push_str(&format!("<span style=\"font-size:{z}%\">"));
                close.push_str("</span>");
            }
            out.push_str(&open);
            out.push_str(&write_plain(&spans[k..j], cur));
            out.push_str(&toggles(cur, &Style::default()));
            *cur = Style::default();
            out.push_str(&close);
        }
        k = j;
    }
    out
}

fn write_plain(spans: &[Span], cur: &mut Style) -> String {
    let mut out = String::new();
    for sp in spans {
        if sp.text.is_empty() {
            continue;
        }
        let want = plain(&sp.style);
        out.push_str(&toggles(cur, &want));
        *cur = want;
        let mut t = escape(&sp.text);
        if sp.style.underline {
            t = format!("<u>{t}</u>");
        }
        match sp.style.marker.as_deref() {
            Some(m) if m == crate::colors::YELLOW_MARKER => t = format!("=={t}=="),
            Some(m) => t = format!("<mark style=\"background:{m}\">{t}</mark>"),
            None => {}
        }
        if let Some(c) = &sp.style.color {
            t = format!("<span style=\"color:{c}\">{t}</span>");
        }

        out.push_str(&t);
    }
    out
}

/// Semnul de listă de la începutul rândului, din Markdown în forma de pe ecran.
fn list_in(line: &str) -> Option<String> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = &line[indent..];
    let pad = " ".repeat(indent);
    for (md, shown) in [("- [ ] ", "☐ "), ("- [x] ", "☑ "), ("- [X] ", "☑ "), ("- ", "• "), ("* ", "• ")] {
        if let Some(r) = rest.strip_prefix(md) {
            return Some(format!("{pad}{shown}{r}"));
        }
    }
    None
}

/// Semnul de listă de pe ecran, înapoi în Markdown (doar la începutul rândului).
fn list_out(md: &str) -> String {
    let indent = md.len() - md.trim_start_matches(' ').len();
    let rest = &md[indent..];
    let pad = &md[..indent];
    for (shown, out) in [("☐ ", "- [ ] "), ("☑ ", "- [x] "), ("• ", "- ")] {
        if let Some(r) = rest.strip_prefix(shown) {
            return format!("{pad}{out}{r}");
        }
    }
    // Un rând obișnuit care ar arăta ca un titlu, citat sau listă se scapă.
    if rest.starts_with('#') || rest.starts_with('>') || rest.starts_with("- ") || rest.starts_with("* ") {
        return format!("{pad}\\{rest}");
    }
    md.to_string()
}

/// „08:30”, „8:05”: o oră.
fn is_time(s: &str) -> bool {
    let Some((h, m)) = s.split_once(':') else { return false };
    (1..=2).contains(&h.len())
        && m.len() == 2
        && h.chars().chain(m.chars()).all(|c| c.is_ascii_digit())
        && h.parse::<u32>().is_ok_and(|h| h < 24)
        && m.parse::<u32>().is_ok_and(|m| m < 60)
}

/// Textul unei zile (fără titlul fișierului), rând cu rând.
pub fn parse_doc(text: &str) -> Vec<Line> {
    let mut lines: Vec<Line> = text
        .lines()
        .map(|l| {
            let l = l.trim_end_matches('\r');
            if let Some(r) = l.strip_prefix("### ") {
                let spans = parse_inline(r);
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                let kind = if is_time(text.trim()) { LineKind::Time } else { LineKind::Subtitle };
                Line { kind, spans }
            } else if let Some(r) = l.strip_prefix("## ") {
                Line { kind: LineKind::Title, spans: parse_inline(r) }
            } else if let Some(r) = l.strip_prefix("> ").or_else(|| (l == ">").then_some("")) {
                Line { kind: LineKind::Quote, spans: parse_inline(r) }
            } else if let Some(shown) = list_in(l) {
                Line { kind: LineKind::Text, spans: parse_inline(&shown) }
            } else {
                Line { kind: LineKind::Text, spans: parse_inline(l) }
            }
        })
        .collect();
    while lines.last().is_some_and(|l| l.kind == LineKind::Text && l.text().trim().is_empty()) {
        lines.pop();
    }
    lines
}

pub fn to_doc(lines: &[Line]) -> String {
    let mut out: Vec<String> = lines
        .iter()
        .map(|l| {
            let md = to_inline(&l.spans);
            match l.kind {
                LineKind::Time => format!("### {md}"),
                LineKind::Title => format!("## {md}"),
                LineKind::Subtitle => format!("### {md}"),
                LineKind::Quote => format!("> {md}").trim_end().to_string(),
                LineKind::Text => list_out(&md),
            }
        })
        .collect();
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Textul simplu (fără semne), pentru căutare și numărarea cuvintelor.
pub fn plain_text(text: &str) -> String {
    parse_doc(text).iter().map(|l| l.text()).collect::<Vec<_>>().join("\n")
}

/// Unde duce o legătură `lanruj:`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// O pagină: ziua, săptămâna, lista lunii, o lună din Planuri, pagina unui rând din index.
    Page(PageRef),
    /// Un rând anume dintr-o pagină, după semnul lui (`^k3f9`).
    Entry(PageRef, String),
    /// Jurnalul unei zile.
    Diary(NaiveDate),
    /// Monitorizarea unei luni, cu fila („activitati”, „somn”, „serviciu”).
    Tracking { year: i32, month: u32, tab: Option<String> },
    Index,
}

fn ref_path(r: PageRef) -> String {
    match r {
        PageRef::Day(d) => format!("zi/{}", d.format("%Y-%m-%d")),
        PageRef::Month { year, month } => format!("luna/{year:04}-{month:02}"),
        PageRef::Future { year, month } => format!("planuri/{year:04}-{month:02}"),
        PageRef::Week { year, week } => format!("saptamana/{year:04}-{week:02}"),
        PageRef::Topic { date, n } => format!("pagina/{}-{n}", date.format("%Y-%m-%d")),
    }
}

fn ym(s: &str) -> Option<(i32, u32)> {
    let (y, m) = s.split_once('-')?;
    Some((y.parse().ok()?, m.parse().ok()?))
}

fn parse_ref<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Option<PageRef> {
    let kind = parts.next()?;
    let v = parts.next()?;
    Some(match kind {
        "zi" => PageRef::Day(NaiveDate::parse_from_str(v, "%Y-%m-%d").ok()?),
        "luna" => {
            let (year, month) = ym(v)?;
            PageRef::Month { year, month }
        }
        "planuri" => {
            let (year, month) = ym(v)?;
            PageRef::Future { year, month }
        }
        "saptamana" => {
            let (year, week) = ym(v)?;
            PageRef::Week { year, week }
        }
        "pagina" => {
            let (d, n) = v.rsplit_once('-')?;
            PageRef::Topic { date: NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()?, n: n.parse().ok()? }
        }
        _ => return None,
    })
}

impl Target {
    pub fn to_url(&self) -> String {
        match self {
            Target::Page(r) => format!("lanruj:{}", ref_path(*r)),
            Target::Entry(r, id) => format!("lanruj:intrare/{}/{id}", ref_path(*r)),
            Target::Diary(d) => format!("lanruj:jurnal/{}", d.format("%Y-%m-%d")),
            Target::Tracking { year, month, tab } => match tab {
                Some(t) => format!("lanruj:monitorizare/{year:04}-{month:02}/{t}"),
                None => format!("lanruj:monitorizare/{year:04}-{month:02}"),
            },
            Target::Index => "lanruj:index".into(),
        }
    }

    pub fn parse(url: &str) -> Option<Target> {
        let rest = url.strip_prefix("lanruj:")?;
        let mut parts = rest.split('/');
        match parts.next()? {
            "intrare" => {
                let r = parse_ref(&mut parts)?;
                Some(Target::Entry(r, parts.next()?.to_string()))
            }
            "jurnal" => Some(Target::Diary(NaiveDate::parse_from_str(parts.next()?, "%Y-%m-%d").ok()?)),
            "monitorizare" => {
                let (year, month) = ym(parts.next()?)?;
                Some(Target::Tracking { year, month, tab: parts.next().map(str::to_string) })
            }
            "index" => Some(Target::Index),
            _ => {
                let mut all = rest.split('/');
                Some(Target::Page(parse_ref(&mut all)?))
            }
        }
    }
}

/// O legătură din jurnal: ziua jurnalului, textul legat și unde duce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiaryLink {
    pub diary: NaiveDate,
    pub text: String,
    pub target: Target,
}

impl Journal {
    pub fn diary_path(&self, date: NaiveDate) -> PathBuf {
        self.root().join("jurnal").join(format!("{:04}", date.year())).join(format!("{}.md", date.format("%Y-%m-%d")))
    }

    /// Textul zilei, fără titlul fișierului.
    pub fn load_diary(&self, date: NaiveDate) -> io::Result<String> {
        let text = match self.read_text(&self.diary_path(date)) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(String::new()),
            Err(e) => return Err(e),
        };
        let mut lines = text.lines().peekable();
        if lines.peek().is_some_and(|l| l.starts_with("# ")) {
            lines.next();
            if lines.peek().is_some_and(|l| l.trim().is_empty()) {
                lines.next();
            }
        }
        Ok(lines.collect::<Vec<_>>().join("\n"))
    }

    /// Salvează textul zilei; un text gol șterge fișierul.
    pub fn save_diary(&self, date: NaiveDate, body: &str, lang: Lang) -> io::Result<()> {
        let path = self.diary_path(date);
        if body.trim().is_empty() {
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
            return Ok(());
        }
        let head = match lang {
            Lang::Ro => "Jurnal",
            Lang::En => "Journal",
        };
        let title = format!("# {head} · {} {} {}", date.day(), lang.month(date.month()).to_lowercase(), date.year());
        self.write_text(&path, &format!("{title}\n\n{}\n", body.trim_end()))
    }

    /// Zilele lunii care au ceva scris în jurnal.
    pub fn diary_days(&self, year: i32, month: u32) -> Vec<u32> {
        let dir = self.root().join("jurnal").join(format!("{year:04}"));
        let prefix = format!("{year:04}-{month:02}-");
        let mut out: Vec<u32> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.strip_prefix(&prefix)?.strip_suffix(".md")?.parse().ok()
            })
            .collect();
        out.sort();
        out
    }

    /// Toate zilele cu jurnal, cele mai noi primele.
    pub fn diary_dates(&self) -> Vec<NaiveDate> {
        let mut out = Vec::new();
        for y in std::fs::read_dir(self.root().join("jurnal")).into_iter().flatten().flatten() {
            for f in std::fs::read_dir(y.path()).into_iter().flatten().flatten() {
                let n = f.file_name().to_string_lossy().to_string();
                if let Some(d) = n.strip_suffix(".md").and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()) {
                    out.push(d);
                }
            }
        }
        out.sort_by(|a, b| b.cmp(a));
        out
    }

    /// Toate legăturile `lanruj:` din jurnal (pentru „pomenit în jurnal”).
    pub fn diary_links(&self) -> Vec<DiaryLink> {
        let mut out = Vec::new();
        for d in self.diary_dates() {
            let Ok(text) = self.load_diary(d) else { continue };
            for line in parse_doc(&text) {
                let mut k = 0;
                while k < line.spans.len() {
                    let link = line.spans[k].style.link.clone();
                    let mut j = k;
                    let mut t = String::new();
                    while j < line.spans.len() && line.spans[j].style.link == link {
                        t.push_str(&line.spans[j].text);
                        j += 1;
                    }
                    if let Some(target) = link.as_deref().and_then(Target::parse) {
                        out.push(DiaryLink { diary: d, text: t, target });
                    }
                    k = j;
                }
            }
        }
        out
    }

    /// Semnul rândului `index` din pagina `r`; dacă nu are, primește unul acum.
    pub fn entry_anchor(&self, r: PageRef, index: usize, lang: Lang) -> io::Result<Option<String>> {
        let day = self.load_ref(r)?;
        let Some(e) = day.entries().nth(index) else { return Ok(None) };
        if let Some(id) = &e.id {
            return Ok(Some(id.clone()));
        }
        let id = crate::migrate::fresh_id(&[&day]);
        let id2 = id.clone();
        self.update_ref(r, lang, move |d| {
            if let Some(e) = d.entry_mut(index) {
                e.id = Some(id2);
            }
        })?;
        Ok(Some(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(md: &str) -> String {
        to_doc(&parse_doc(md))
    }

    #[test]
    fn scrisul_se_pastreaza() {
        for md in [
            "Am dormit **bine**, iar dimineața a fost *liniștită*.",
            "~~tăiat~~ și <u>subliniat</u> și ***amândouă***",
            "Plătește ==impozitul== până <span style=\"color:#c01c28\">vineri</span>",
            "<mark style=\"background:#8ff0a4\">verde</mark> apoi [piața **cu** mama](lanruj:intrare/zi/2026-11-21/k3f9).",
            "### 08:30\nText\n\n## Titlu\n### Un subtitlu\n> citat\n- unu\n  - doi\n- [ ] de făcut\n- [x] gata\n1. primul",
            "5 \\* 3 = 15, a \\== b, \\[nu e legătură\\]",
            "**bold *bold italic* bold**",
            "Un cuvânt <span style=\"font-size:150%\">mare</span> și <span style=\"font-family:Lora\"><span style=\"font-size:85%\">mic cu Lora</span></span>",
            "<span style=\"font-family:Caveat\">Primul rând, de mână</span>\n## <span style=\"font-family:Patrick Hand\">Un **titlu** <span style=\"color:#c01c28\">roșu</span></span>",
        ] {
            assert_eq!(round(md), md, "{md}");
        }
    }

    #[test]
    fn bucati() {
        let s = parse_inline("a **b *c*** [d](https://x.ro)");
        assert_eq!(s[0].text, "a ");
        assert!(s[1].style.bold && !s[1].style.italic);
        assert!(s[2].style.bold && s[2].style.italic && s[2].text == "c");
        assert_eq!(s.last().unwrap().style.link.as_deref(), Some("https://x.ro"));
        // Un text care arată ca semnele se scapă la scriere.
        let t = vec![Span { text: "a*b ==c== [x](y) # nu".into(), style: Style::default() }];
        assert_eq!(parse_inline(&to_inline(&t)), t);
        let lines = vec![Line { kind: LineKind::Text, spans: vec![Span { text: "# nu e titlu".into(), style: Style::default() }] }];
        assert_eq!(parse_doc(&to_doc(&lines)), lines);
    }

    #[test]
    fn tinte() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 21).unwrap();
        for t in [
            Target::Page(PageRef::Day(d)),
            Target::Page(PageRef::Week { year: 2026, week: 47 }),
            Target::Page(PageRef::Topic { date: d, n: 2 }),
            Target::Entry(PageRef::Month { year: 2026, month: 11 }, "k3f9".into()),
            Target::Diary(d),
            Target::Tracking { year: 2026, month: 11, tab: Some("somn".into()) },
            Target::Tracking { year: 2026, month: 11, tab: None },
            Target::Index,
        ] {
            assert_eq!(Target::parse(&t.to_url()), Some(t.clone()));
        }
    }

    #[test]
    fn fisierul() {
        let root = std::env::temp_dir().join(format!("lanruj-jurnal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let j = Journal::new(&root);
        let d = NaiveDate::from_ymd_opt(2026, 11, 21).unwrap();
        j.save_diary(d, "### 08:30\nO zi [bună](lanruj:intrare/zi/2026-11-21/k3f9).", Lang::Ro).unwrap();
        assert!(std::fs::read_to_string(j.diary_path(d)).unwrap().starts_with("# Jurnal · 21 noiembrie 2026\n\n### 08:30"));
        assert_eq!(j.load_diary(d).unwrap(), "### 08:30\nO zi [bună](lanruj:intrare/zi/2026-11-21/k3f9).");
        assert_eq!(j.diary_days(2026, 11), vec![21]);
        let l = j.diary_links();
        assert_eq!(l[0].text, "bună");
        assert_eq!(l[0].target, Target::Entry(PageRef::Day(d), "k3f9".into()));
        j.add(d, crate::parse_quick(". sună la service").unwrap(), Lang::Ro).unwrap();
        let id = j.entry_anchor(PageRef::Day(d), 0, Lang::Ro).unwrap().unwrap();
        assert_eq!(j.entry_anchor(PageRef::Day(d), 0, Lang::Ro).unwrap().unwrap(), id, "semnul rămâne");
        j.save_diary(d, "  ", Lang::Ro).unwrap();
        assert!(!j.diary_path(d).exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
