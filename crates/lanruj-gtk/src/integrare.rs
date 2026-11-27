// SPDX-License-Identifier: GPL-3.0-or-later
//! Lansatorul din meniu pentru AppImage. Un AppImage nu se instalează, deci la fiecare pornire Lanruj își pune
//! singur lansatorul și iconițele în dosarele utilizatorului (~/.local/share/applications și icons), pe care le
//! urmăresc toate mediile (GNOME, KDE, Cinnamon, Xfce, LXQt, COSMIC…). Dacă fișierul .AppImage a fost mutat,
//! lansatorul se actualizează; dacă nimic nu s-a schimbat, nu se scrie nimic.

use std::path::{Path, PathBuf};

const DESKTOP: &str = include_str!("../../../data/io.github.ThinkRoot99.Lanruj.desktop");
const ICONS: [(&str, &[u8]); 2] = [
    ("scalable/apps/io.github.ThinkRoot99.Lanruj.svg", include_bytes!("../../../data/icons/hicolor/scalable/apps/io.github.ThinkRoot99.Lanruj.svg")),
    (
        "symbolic/apps/io.github.ThinkRoot99.Lanruj-symbolic.svg",
        include_bytes!("../../../data/icons/hicolor/symbolic/apps/io.github.ThinkRoot99.Lanruj-symbolic.svg"),
    ),
];

/// Scrie `data` în `path` doar dacă diferă de ce e acolo.
fn write_if_changed(path: &Path, data: &[u8]) -> std::io::Result<bool> {
    if std::fs::read(path).is_ok_and(|old| old == data) {
        return Ok(false);
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(path, data)?;
    Ok(true)
}

/// Lansatorul pentru fișierul .AppImage dat: Exec și TryExec duc la el.
fn desktop_for(appimage: &Path) -> String {
    let exe = appimage.display().to_string().replace('\\', "\\\\").replace('"', "\\\"");
    DESKTOP
        .lines()
        .map(|l| if l.starts_with("Exec=") { format!("Exec=\"{exe}\"\nTryExec={exe}") } else { l.to_string() })
        .chain(["X-AppImage-Integrate=false".to_string()])
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// Pus la pornire: lucrează doar când Lanruj rulează din AppImage.
pub fn appimage() {
    let Some(appimage) = std::env::var_os("APPIMAGE").filter(|p| !p.is_empty()).map(PathBuf::from) else { return };
    // La probe (LANRUJ_DIR), lansatorul rămâne în dosarul de probă, nu în sistem.
    let data = match std::env::var_os("LANRUJ_DIR").filter(|d| !d.is_empty()) {
        Some(d) => PathBuf::from(d).join(".integrare"),
        None => gtk::glib::user_data_dir(),
    };
    let apps = data.join("applications");
    let mut changed = false;
    match write_if_changed(&apps.join(format!("{}.desktop", crate::APP_ID)), desktop_for(&appimage).as_bytes()) {
        Ok(c) => changed |= c,
        Err(e) => eprintln!("lanruj: nu am putut pune lansatorul în meniu: {e}"),
    }
    for (rel, bytes) in ICONS {
        match write_if_changed(&data.join("icons/hicolor").join(rel), bytes) {
            Ok(c) => changed |= c,
            Err(e) => eprintln!("lanruj: nu am putut pune iconița: {e}"),
        }
    }
    if changed {
        // Unele meniuri citesc și baza de date a lansatoarelor; dacă unealta lipsește, nu e nicio problemă.
        let _ = std::process::Command::new("update-desktop-database").arg(&apps).status();
        // Atingerea dosarului îi anunță pe cei care urmăresc tema de iconițe.
        let _ = std::fs::File::open(data.join("icons/hicolor")).and_then(|f| f.set_modified(std::time::SystemTime::now()));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn lansatorul_duce_la_appimage() {
        let d = super::desktop_for(std::path::Path::new("/home/ana/Aplicații/Lanruj-0.4.0-x86_64.AppImage"));
        assert!(d.contains("Exec=\"/home/ana/Aplicații/Lanruj-0.4.0-x86_64.AppImage\"\n"));
        assert!(d.contains("TryExec=/home/ana/Aplicații/Lanruj-0.4.0-x86_64.AppImage\n"));
        assert!(!d.contains("Exec=lanruj"));
        assert!(d.contains("Icon=io.github.ThinkRoot99.Lanruj\n"));
    }
}
