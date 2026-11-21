// SPDX-License-Identifier: GPL-3.0-or-later
//! Copiile de siguranță: tot jurnalul ca arhivă zip, într-un dosar ales,
//! o dată pe zi sau pe săptămână; se păstrează doar ultimele câteva.

use crate::setari::Backup;
use chrono::NaiveDate;
use std::path::{Path, PathBuf};

const PREFIX: &str = "lanruj-copie-";

/// Ziua unei copii, din numele ei („lanruj-copie-2026-11-21.zip”).
fn date_of(name: &str) -> Option<NaiveDate> {
    let d = name.strip_prefix(PREFIX)?.strip_suffix(".zip")?;
    NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
}

/// Copiile din dosar, de la cea mai veche la cea mai nouă.
pub fn list(dir: &Path) -> Vec<(NaiveDate, PathBuf)> {
    let mut out: Vec<(NaiveDate, PathBuf)> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().filter_map(|e| Some((date_of(&e.file_name().to_string_lossy())?, e.path()))).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// E vremea unei copii noi?
pub fn due(dir: &Path, freq: Backup, today: NaiveDate) -> bool {
    let last = list(dir).last().map(|(d, _)| *d);
    match (freq, last) {
        (Backup::Off, _) => false,
        (_, None) => true,
        (Backup::Daily, Some(d)) => d < today,
        (Backup::Weekly, Some(d)) => (today - d).num_days() >= 7,
    }
}

/// Face copia zilei (o înlocuiește pe a zilei, dacă există) și le șterge pe cele vechi peste `keep`.
pub fn make(root: &Path, dir: &Path, today: NaiveDate, keep: u32) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{PREFIX}{}.zip", today.format("%Y-%m-%d")));
    let tmp = dir.join(format!(".{PREFIX}{}.zip.tmp", today.format("%Y-%m-%d")));
    if let Err(e) = crate::export::write_zip(root, &tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, &path)?;
    let all = list(dir);
    let extra = all.len().saturating_sub(keep.max(1) as usize);
    for (_, old) in all.into_iter().take(extra) {
        std::fs::remove_file(old)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn copii_si_pastrare() {
        let base = std::env::temp_dir().join(format!("lanruj-copii-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (root, dir) = (base.join("jurnal"), base.join("copii"));
        std::fs::create_dir_all(root.join("2026/11")).unwrap();
        std::fs::write(root.join("2026/11/2026-11-21.md"), "# zi\n").unwrap();
        assert!(due(&dir, Backup::Weekly, d("2026-11-21")), "fără nicio copie");
        assert!(!due(&dir, Backup::Off, d("2026-11-21")));
        for day in ["2026-11-01", "2026-11-08", "2026-11-15", "2026-11-21"] {
            make(&root, &dir, d(day), 3).unwrap();
        }
        let names: Vec<NaiveDate> = list(&dir).into_iter().map(|(x, _)| x).collect();
        assert_eq!(names, vec![d("2026-11-08"), d("2026-11-15"), d("2026-11-21")], "rămân ultimele 3");
        assert!(!due(&dir, Backup::Daily, d("2026-11-21")));
        assert!(due(&dir, Backup::Daily, d("2026-11-22")));
        assert!(!due(&dir, Backup::Weekly, d("2026-11-27")));
        assert!(due(&dir, Backup::Weekly, d("2026-11-28")));
        std::fs::remove_dir_all(base).unwrap();
    }
}
