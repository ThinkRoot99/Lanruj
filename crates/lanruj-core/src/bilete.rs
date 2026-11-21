// SPDX-License-Identifier: GPL-3.0-or-later
//! Biletele: idei notate în grabă, ca niște foi mici lipite pe jurnal. Stau separat de pagini,
//! în `bilete.md`, se văd pe fiecare pagină și dispar doar când sunt șterse. Locul fiecăruia
//! pe caiet stă la sfârșitul primului rând, ca un comentariu care nu se vede în Markdown.
//!
//! ```text
//! # Bilete
//!
//! - Un caiet pentru rețete <!-- 640,420 -->
//! - Cadou pentru mama:
//!   o carte despre grădini
//! ```

use crate::journal::Journal;
use std::io;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub text: String,
    /// Locul pe caiet (x, y), dacă a fost mutat.
    pub pos: Option<(i32, i32)>,
}

impl Note {
    pub fn new(text: &str) -> Note {
        Note { text: text.trim().to_string(), pos: None }
    }
}

/// „text <!-- 640,420 -->” → („text”, Some((640, 420))).
fn split_pos(line: &str) -> (&str, Option<(i32, i32)>) {
    if let Some(i) = line.rfind("<!--")
        && let Some(inner) = line[i + 4..].trim().strip_suffix("-->")
        && let Some((x, y)) = inner.trim().split_once(',')
        && let (Ok(x), Ok(y)) = (x.trim().parse(), y.trim().parse())
    {
        return (line[..i].trim_end(), Some((x, y)));
    }
    (line, None)
}

pub fn parse(text: &str) -> Vec<Note> {
    let mut out: Vec<Note> = Vec::new();
    for line in text.lines() {
        if let Some(t) = line.strip_prefix("- ") {
            let (t, pos) = split_pos(t);
            out.push(Note { text: t.to_string(), pos });
        } else if let (Some(t), Some(last)) = (line.strip_prefix("  "), out.last_mut()) {
            last.text.push('\n');
            last.text.push_str(t);
        }
    }
    out
}

pub fn to_markdown(notes: &[Note]) -> String {
    let mut s = String::from("# Bilete\n\n");
    for n in notes {
        let mut lines = n.text.trim().lines();
        if let Some(first) = lines.next() {
            match n.pos {
                Some((x, y)) => s.push_str(&format!("- {first} <!-- {x},{y} -->\n")),
                None => s.push_str(&format!("- {first}\n")),
            }
            for l in lines {
                s.push_str(&format!("  {l}\n"));
            }
        }
    }
    s
}

impl Journal {
    pub fn notes_path(&self) -> PathBuf {
        self.root().join("bilete.md")
    }

    pub fn load_notes(&self) -> io::Result<Vec<Note>> {
        match self.read_text(&self.notes_path()) {
            Ok(t) => Ok(parse(&t)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Schimbă biletele cu `f` și le salvează (fără bilete, fișierul dispare).
    pub fn update_notes(&self, f: impl FnOnce(&mut Vec<Note>)) -> io::Result<Vec<Note>> {
        let mut notes = self.load_notes()?;
        f(&mut notes);
        notes.retain(|n| !n.text.trim().is_empty());
        let path = self.notes_path();
        if notes.is_empty() {
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        } else {
            self.write_text(&path, &to_markdown(&notes))?;
        }
        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilete() {
        let text = "# Bilete\n\n- Un caiet pentru rețete <!-- 640,420 -->\n- Cadou pentru mama:\n  o carte despre grădini\n";
        let n = parse(text);
        assert_eq!(n[0], Note { text: "Un caiet pentru rețete".into(), pos: Some((640, 420)) });
        assert_eq!(n[1], Note { text: "Cadou pentru mama:\no carte despre grădini".into(), pos: None });
        assert_eq!(to_markdown(&n), text);
        let root = std::env::temp_dir().join(format!("lanruj-bilete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let j = Journal::new(&root);
        j.update_notes(|n| n.push(Note::new("prima"))).unwrap();
        j.update_notes(|n| n[0].pos = Some((10, 20))).unwrap();
        assert_eq!(j.load_notes().unwrap(), vec![Note { text: "prima".into(), pos: Some((10, 20)) }]);
        j.update_notes(|n| n.clear()).unwrap();
        assert!(!j.notes_path().exists(), "fără bilete, fără fișier");
        let _ = std::fs::remove_dir_all(root);
    }
}
