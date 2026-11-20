// SPDX-License-Identifier: GPL-3.0-or-later
//! Pozele și fișierele dintr-o pagină. Stau lângă pagina lor, într-un dosar cu numele ei:
//!
//! - zi: `2026/11/2026-11-21/poza.jpg`, în pagină `![](2026-11-21/poza.jpg)`;
//! - pagina unui rând din index: `pagini/2026-11-23/…`;
//! - lista lunii: `2026/11/luna/…`; planul săptămânii: `2026/saptamani/2026-S47/…`;
//!   Planuri: `viitor/…`.
//!
//! În pagină sunt rânduri Markdown obișnuite: `![](cale)` pentru poze, `[nume](cale)` pentru fișiere.

use crate::PageRef;
use crate::day::Item;
use crate::journal::Journal;
use crate::lang::Lang;
use std::io;
use std::path::{Path, PathBuf};

/// Un rând de atașament din pagină.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attachment {
    Image { path: String },
    File { name: String, path: String },
}

pub const IMAGE_EXT: [&str; 7] = ["jpg", "jpeg", "png", "webp", "gif", "bmp", "heic"];

pub fn is_image(name: &str) -> bool {
    Path::new(name).extension().and_then(|e| e.to_str()).is_some_and(|e| IMAGE_EXT.contains(&e.to_lowercase().as_str()))
}

/// `![…](cale)` sau `[nume](cale)` (o cale locală, nu o adresă web), singur pe rând.
pub fn parse(line: &str) -> Option<Attachment> {
    let t = line.trim();
    let (image, rest) = match t.strip_prefix('!') {
        Some(r) => (true, r),
        None => (false, t),
    };
    let rest = rest.strip_prefix('[')?;
    let close = rest.find("](")?;
    let name = &rest[..close];
    let after = &rest[close + 2..];
    let path = after.strip_suffix(')')?;
    if path.is_empty() || path.contains("://") || path.starts_with("mailto:") || path.contains(char::is_whitespace) {
        return None;
    }
    Some(if image { Attachment::Image { path: path.to_string() } } else { Attachment::File { name: name.to_string(), path: path.to_string() } })
}

impl Journal {
    /// Dosarul în care stă fișierul paginii (căile din pagină sunt față de el).
    pub fn page_dir(&self, r: PageRef) -> PathBuf {
        match r {
            PageRef::Day(d) => self.day_path(d).parent().expect("are dosar").to_path_buf(),
            PageRef::Month { year, month } => self.month_dir(year, month),
            PageRef::Future { .. } => self.root().to_path_buf(),
            PageRef::Week { year, week } => self.week_path(year, week).parent().expect("are dosar").to_path_buf(),
            PageRef::Topic { date, n } => self.topic_path(date, n).parent().expect("are dosar").to_path_buf(),
        }
    }

    /// Numele dosarului de atașamente al paginii (față de `page_dir`).
    fn attach_folder(&self, r: PageRef) -> String {
        match r {
            PageRef::Day(d) => d.format("%Y-%m-%d").to_string(),
            PageRef::Month { .. } => "luna".into(),
            PageRef::Future { .. } => "viitor".into(),
            PageRef::Week { year, week } => format!("{year:04}-S{week:02}"),
            PageRef::Topic { date, n } => {
                let p = self.topic_path(date, n);
                p.file_stem().and_then(|s| s.to_str()).unwrap_or("pagina").to_string()
            }
        }
    }

    /// Unde se copiază un fișier nou cu numele `name`: (calea completă, calea din pagină).
    /// Dacă numele e deja luat, se adaugă „-2”, „-3”…
    pub fn attach_target(&self, r: PageRef, name: &str) -> io::Result<(PathBuf, String)> {
        let folder = self.attach_folder(r);
        let dir = self.page_dir(r).join(&folder);
        std::fs::create_dir_all(&dir)?;
        let clean: String = name.chars().map(|c| if c == '/' || c == '\\' || c.is_control() { '_' } else { c }).collect();
        let clean = clean.replace(' ', "-");
        let (stem, ext) = match clean.rsplit_once('.') {
            Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
            _ => (clean.clone(), String::new()),
        };
        let mut n = 1;
        loop {
            let file = if n == 1 { format!("{stem}{ext}") } else { format!("{stem}-{n}{ext}") };
            let abs = dir.join(&file);
            if !abs.exists() {
                return Ok((abs, format!("{folder}/{file}")));
            }
            n += 1;
        }
    }

    /// Adaugă la sfârșitul paginii rândul unui atașament deja copiat.
    pub fn add_attachment_line(&self, r: PageRef, rel: &str, name: &str, lang: Lang) -> io::Result<()> {
        let line = if is_image(name) { format!("![]({rel})") } else { format!("[{name}]({rel})") };
        self.update_ref(r, lang, |d| {
            while matches!(d.items.last(), Some(Item::Other(s)) if s.trim().is_empty()) {
                d.items.pop();
            }
            d.items.push(Item::Other(line));
        })
        .map(|_| ())
    }

    /// Calea completă a unui atașament din pagină.
    pub fn attachment_abs(&self, r: PageRef, rel: &str) -> PathBuf {
        self.page_dir(r).join(rel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::fs;

    #[test]
    fn randuri() {
        assert_eq!(parse("![](2026-11-21/poza.jpg)"), Some(Attachment::Image { path: "2026-11-21/poza.jpg".into() }));
        assert_eq!(
            parse("[bilet tren.pdf](2026-11-21/bilet-tren.pdf)"),
            Some(Attachment::File { name: "bilet tren.pdf".into(), path: "2026-11-21/bilet-tren.pdf".into() })
        );
        assert_eq!(parse("[site](https://x.ro)"), None);
        assert_eq!(parse("text simplu"), None);
        assert!(is_image("A.JPG") && !is_image("a.pdf"));
    }

    #[test]
    fn copiat_langa_zi() {
        let root = std::env::temp_dir().join(format!("lanruj-atas-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let j = Journal::new(&root);
        let d = NaiveDate::from_ymd_opt(2026, 11, 21).unwrap();
        let r = PageRef::Day(d);
        let (abs, rel) = j.attach_target(r, "poza mare.jpg").unwrap();
        assert_eq!(rel, "2026-11-21/poza-mare.jpg");
        fs::write(&abs, b"x").unwrap();
        let (_, rel2) = j.attach_target(r, "poza mare.jpg").unwrap();
        assert_eq!(rel2, "2026-11-21/poza-mare-2.jpg", "numele luat primește -2");
        j.add_attachment_line(r, &rel, "poza mare.jpg", Lang::Ro).unwrap();
        j.add_attachment_line(r, "2026-11-21/bilet.pdf", "bilet.pdf", Lang::Ro).unwrap();
        let text = fs::read_to_string(j.day_path(d)).unwrap();
        assert_eq!(text, "# Sâmbătă, 21 noiembrie 2026\n\n![](2026-11-21/poza-mare.jpg)\n[bilet.pdf](2026-11-21/bilet.pdf)\n");
        assert_eq!(j.attachment_abs(r, &rel), abs);
        let t = PageRef::Topic { date: d, n: 1 };
        assert_eq!(j.attach_target(t, "a.png").unwrap().1, "2026-11-21/a.png");
        assert!(j.page_dir(t).ends_with("pagini"));
        fs::remove_dir_all(root).unwrap();
    }
}
