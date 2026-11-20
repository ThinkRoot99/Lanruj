// SPDX-License-Identifier: GPL-3.0-or-later
//! Culorile din text, păstrate ca Markdown obișnuit:
//!
//! - marker galben: `==text==` (ca în Obsidian și Typora);
//! - marker de altă culoare: `<mark style="background:#8ff0a4">text</mark>`;
//! - text colorat: `<span style="color:#c01c28">text</span>`.
//!
//! Aici textul se împarte în bucăți cu culoarea lor, se scoate culoarea
//! (pentru căutare) și se pune culoare pe o bucată de text.

/// O bucată de text cu culoarea ei (culori în forma `#rrggbb`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub color: Option<String>,
    pub marker: Option<String>,
}

/// Culoarea markerului `==…==`.
pub const YELLOW_MARKER: &str = "#f9f06b";

/// Culorile textului: (nume român, nume englez, ziua, noaptea).
pub const TEXT_COLORS: [(&str, &str, &str, &str); 6] = [
    ("Roșu", "Red", "#c01c28", "#ff7b63"),
    ("Portocaliu", "Orange", "#c64600", "#ffa348"),
    ("Verde", "Green", "#26a269", "#8ff0a4"),
    ("Albastru", "Blue", "#1c71d8", "#99c1f1"),
    ("Violet", "Purple", "#813d9c", "#dc8add"),
    ("Maro", "Brown", "#865e3c", "#cdab8f"),
];

/// Culorile markerului: (nume român, nume englez, culoarea).
pub const MARKER_COLORS: [(&str, &str, &str); 5] = [
    ("Galben", "Yellow", YELLOW_MARKER),
    ("Verde", "Green", "#8ff0a4"),
    ("Albastru", "Blue", "#99c1f1"),
    ("Roz", "Pink", "#f6a6c1"),
    ("Portocaliu", "Orange", "#ffbe6f"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint<'a> {
    Color(&'a str),
    Marker(&'a str),
}

fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// `<span style="color:#c01c28">` → („span”, „color”, „#c01c28”, lungimea etichetei).
fn open_tag(s: &str) -> Option<(&'static str, String, usize)> {
    for (tag, prop) in [("span", "color"), ("mark", "background")] {
        let start = format!("<{tag} style=\"{prop}:");
        if let Some(rest) = s.strip_prefix(&start) {
            let end = rest.find("\">")?;
            let value = rest[..end].trim().trim_end_matches(';').trim();
            if is_hex(value) {
                return Some((tag, value.to_lowercase(), start.len() + end + 2));
            }
        }
    }
    None
}

/// Împarte textul în bucăți cu culoare. Etichetele necunoscute rămân text.
pub fn runs(text: &str) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    let mut color: Vec<String> = Vec::new();
    let mut marker: Vec<String> = Vec::new();
    let mut eq_marker = false;
    let mut buf = String::new();
    let flush = |buf: &mut String, out: &mut Vec<Run>, color: &[String], marker: &[String], eq: bool| {
        if buf.is_empty() {
            return;
        }
        let m = if eq { Some(YELLOW_MARKER.to_string()) } else { marker.last().cloned() };
        let c = color.last().cloned();
        match out.last_mut() {
            Some(last) if last.color == c && last.marker == m => last.text.push_str(buf),
            _ => out.push(Run { text: buf.clone(), color: c, marker: m }),
        }
        buf.clear();
    };
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if let Some(r) = rest.strip_prefix("==") {
            // `==` deschide un marker doar dacă se și închide mai încolo.
            if eq_marker || r.contains("==") {
                flush(&mut buf, &mut out, &color, &marker, eq_marker);
                eq_marker = !eq_marker;
                i += 2;
                continue;
            }
        }
        if let Some((tag, value, len)) = open_tag(rest) {
            let close = format!("</{tag}>");
            if rest[len..].contains(&close) {
                flush(&mut buf, &mut out, &color, &marker, eq_marker);
                if tag == "span" { color.push(value) } else { marker.push(value) }
                i += len;
                continue;
            }
        }
        if rest.starts_with("</span>") && !color.is_empty() {
            flush(&mut buf, &mut out, &color, &marker, eq_marker);
            color.pop();
            i += "</span>".len();
            continue;
        }
        if rest.starts_with("</mark>") && !marker.is_empty() {
            flush(&mut buf, &mut out, &color, &marker, eq_marker);
            marker.pop();
            i += "</mark>".len();
            continue;
        }
        let c = rest.chars().next().expect("nu e gol");
        buf.push(c);
        i += c.len_utf8();
    }
    flush(&mut buf, &mut out, &color, &marker, eq_marker);
    out
}

/// Textul fără culori (pentru căutare și pentru locurile unde culoarea nu se vede).
pub fn plain(text: &str) -> String {
    runs(text).into_iter().map(|r| r.text).collect()
}

/// Scrie bucățile înapoi în Markdown.
pub fn to_markdown(runs: &[Run]) -> String {
    let mut out = String::new();
    for r in runs {
        let mut s = r.text.clone();
        if let Some(c) = &r.color {
            s = format!("<span style=\"color:{c}\">{s}</span>");
        }
        match r.marker.as_deref() {
            Some(m) if m == YELLOW_MARKER => s = format!("=={s}=="),
            Some(m) => s = format!("<mark style=\"background:{m}\">{s}</mark>"),
            None => {}
        }
        out.push_str(&s);
    }
    out
}

/// Pune culoarea (sau, cu `None`, scoate culorile) pe caracterele `start..end` ale textului fără culori.
pub fn paint(text: &str, start: usize, end: usize, paint: Option<Paint>) -> String {
    let rs = runs(text);
    // Câte un rând de însușiri pe fiecare caracter, apoi le adunăm la loc.
    let mut chars: Vec<(char, Option<String>, Option<String>)> = Vec::new();
    for r in &rs {
        for c in r.text.chars() {
            chars.push((c, r.color.clone(), r.marker.clone()));
        }
    }
    let end = end.min(chars.len());
    for ch in chars.iter_mut().take(end).skip(start) {
        match paint {
            Some(Paint::Color(c)) => ch.1 = Some(c.to_string()),
            Some(Paint::Marker(m)) => ch.2 = Some(m.to_string()),
            None => {
                ch.1 = None;
                ch.2 = None;
            }
        }
    }
    let mut out: Vec<Run> = Vec::new();
    for (c, color, marker) in chars {
        match out.last_mut() {
            Some(last) if last.color == color && last.marker == marker => last.text.push(c),
            _ => out.push(Run { text: c.to_string(), color, marker }),
        }
    }
    to_markdown(&out)
}

/// Varianta de noapte a unei culori de text (din paletă); altfel aceeași culoare.
pub fn night_color(c: &str) -> &str {
    TEXT_COLORS.iter().find(|(_, _, day, _)| day.eq_ignore_ascii_case(c)).map(|(_, _, _, n)| *n).unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucati_si_text_simplu() {
        let t = "Plătește ==impozitul== până <span style=\"color:#c01c28\">vineri</span> și <mark style=\"background:#8ff0a4\">gata</mark>";
        let r = runs(t);
        assert_eq!(r.len(), 6);
        assert_eq!(r[1], Run { text: "impozitul".into(), color: None, marker: Some(YELLOW_MARKER.into()) });
        assert_eq!(r[3].color.as_deref(), Some("#c01c28"));
        assert_eq!(r[5].marker.as_deref(), Some("#8ff0a4"));
        assert_eq!(plain(t), "Plătește impozitul până vineri și gata");
        assert_eq!(to_markdown(&r), t, "citit și rescris identic");
    }

    #[test]
    fn fara_culori_ramane_la_fel() {
        for t in ["Text simplu", "a == b", "1 < 2 și <b>nu e culoare</b>", "<span style=\"color:red\">nume</span>"] {
            assert_eq!(plain(t), t);
            assert_eq!(to_markdown(&runs(t)), t);
        }
    }

    #[test]
    fn coloreaza_si_sterge() {
        let t = "Sună la service";
        let t2 = paint(t, 0, 4, Some(Paint::Color("#c01c28")));
        assert_eq!(t2, "<span style=\"color:#c01c28\">Sună</span> la service");
        let t3 = paint(&t2, 8, 15, Some(Paint::Marker(YELLOW_MARKER)));
        assert_eq!(t3, "<span style=\"color:#c01c28\">Sună</span> la ==service==");
        let t4 = paint(&t3, 0, 15, Some(Paint::Marker("#99c1f1")));
        assert_eq!(plain(&t4), t);
        assert_eq!(runs(&t4).iter().filter(|r| r.marker.as_deref() == Some("#99c1f1")).count(), 2);
        assert_eq!(paint(&t4, 0, 100, None), t, "fără culoare pe tot");
        assert_eq!(night_color("#C01C28"), "#ff7b63");
    }
}
