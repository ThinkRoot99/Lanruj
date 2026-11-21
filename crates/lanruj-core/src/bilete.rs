// SPDX-License-Identifier: GPL-3.0-or-later
//! Biletele: idei notate în grabă, ca niște foi mici lipite pe jurnal. Stau separat de pagini,
//! în `bilete.md`, se văd pe fiecare pagină și dispar doar când sunt șterse.
//!
//! ```text
//! # Bilete
//!
//! - Un caiet pentru rețete
//! - Cadou pentru mama:
//!   o carte despre grădini
//! ```

use crate::journal::Journal;
use std::io;
use std::path::PathBuf;

pub fn parse(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(t) = line.strip_prefix("- ") {
            out.push(t.to_string());
        } else if let (Some(t), Some(last)) = (line.strip_prefix("  "), out.last_mut()) {
            last.push('\n');
            last.push_str(t);
        }
    }
    out
}

pub fn to_markdown(notes: &[String]) -> String {
    let mut s = String::from("# Bilete\n\n");
    for n in notes {
        let mut lines = n.trim().lines();
        if let Some(first) = lines.next() {
            s.push_str(&format!("- {first}\n"));
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

    pub fn load_notes(&self) -> io::Result<Vec<String>> {
        match self.read_text(&self.notes_path()) {
            Ok(t) => Ok(parse(&t)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Schimbă biletele cu `f` și le salvează (fără bilete, fișierul dispare).
    pub fn update_notes(&self, f: impl FnOnce(&mut Vec<String>)) -> io::Result<Vec<String>> {
        let mut notes = self.load_notes()?;
        f(&mut notes);
        notes.retain(|n| !n.trim().is_empty());
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
        let text = "# Bilete\n\n- Un caiet pentru rețete\n- Cadou pentru mama:\n  o carte despre grădini\n";
        let n = parse(text);
        assert_eq!(n, vec!["Un caiet pentru rețete".to_string(), "Cadou pentru mama:\no carte despre grădini".to_string()]);
        assert_eq!(to_markdown(&n), text);
        let root = std::env::temp_dir().join(format!("lanruj-bilete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let j = Journal::new(&root);
        j.update_notes(|n| n.push("prima".into())).unwrap();
        assert_eq!(j.load_notes().unwrap(), vec!["prima"]);
        j.update_notes(|n| n.clear()).unwrap();
        assert!(!j.notes_path().exists(), "fără bilete, fără fișier");
        let _ = std::fs::remove_dir_all(root);
    }
}
