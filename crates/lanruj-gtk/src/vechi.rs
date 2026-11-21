// SPDX-License-Identifier: GPL-3.0-or-later
//! Aplicația s-a numit întâi „Inkday”. La prima pornire ca Lanruj, datele vechi
//! se mută la locurile noi, o singură dată (nimic nu se suprascrie).

use std::path::{Path, PathBuf};

/// Mută `from` în `to` doar dacă `from` există și `to` nu.
fn move_once(from: &Path, to: &Path) {
    if from.exists() && !to.exists() {
        if let Some(d) = to.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        match std::fs::rename(from, to) {
            Ok(()) => eprintln!("lanruj: mutat {} -> {}", from.display(), to.display()),
            Err(e) => eprintln!("lanruj: nu am putut muta {}: {e}", from.display()),
        }
    }
}

/// Fișierele cu prefixul vechi dintr-un dosar primesc prefixul nou.
fn rename_prefix(dir: &Path, old: &str, new: &str) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(rest) = name.strip_prefix(old) {
            move_once(&e.path(), &dir.join(format!("{new}{rest}")));
        }
    }
}

pub fn migrate() {
    // Jurnalele de probă (LANRUJ_DIR) nu au nimic vechi de mutat.
    if std::env::var_os("LANRUJ_DIR").is_some_and(|d| !d.is_empty()) {
        return;
    }
    let home = gtk::glib::home_dir();
    let journal = home.join(".lanruj");
    move_once(&home.join(".inkday"), &journal);
    rename_prefix(&journal, "inkday-cheie-", "lanruj-cheie-");

    let config = gtk::glib::user_config_dir();
    move_once(&config.join("inkday"), &config.join("lanruj"));
    // Setările: grupul din fișier se numea [inkday].
    let ini = config.join("lanruj").join("setari.ini");
    if let Ok(text) = std::fs::read_to_string(&ini)
        && text.contains("[inkday]")
    {
        let _ = std::fs::write(&ini, text.replace("[inkday]", "[lanruj]"));
    }
    let state = std::env::var_os("XDG_STATE_HOME").filter(|d| !d.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".local").join("state"));
    move_once(&state.join("inkday"), &state.join("lanruj"));

    // Copiile de siguranță din dosarul implicit.
    if let Some(docs) = gtk::glib::user_special_dir(gtk::glib::UserDirectory::Documents) {
        let new: PathBuf = docs.join("Lanruj - copii");
        move_once(&docs.join("Inkday - copii"), &new);
        rename_prefix(&new, "inkday-copie-", "lanruj-copie-");
    }

    // Pornirea automată: fișierul vechi se înlocuiește cu cel nou.
    let auto = config.join("autostart").join("io.github.ThinkRoot99.Inkday.desktop");
    if auto.is_file() {
        let _ = std::fs::remove_file(&auto);
        let _ = crate::incuiere::set_autostart(true);
    }
}
