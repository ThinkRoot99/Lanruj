// SPDX-License-Identifier: GPL-3.0-or-later
//! Iconița din systray (StatusNotifierItem): clic = deschide Lanruj,
//! meniul are „Deschide Lanruj” și „Ieșire”. Rulează pe firul ei; acțiunile
//! ajung în firul ferestrei prin `glib::idle_add_once`.

use crate::APP_ID;
use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib;
use lanruj_core::Lang;

pub struct LanrujTray {
    pub lang: Lang,
    /// Iconița ca imagine, albă, la câteva mărimi. Bara de sus o arată fără să caute
    /// după nume (cât timp aplicația nu e instalată, numele n-ar fi găsit).
    pub pixmaps: Vec<ksni::Icon>,
}

/// Iconița simbolică, desenată albă pe transparent, în ARGB32 (ordinea rețelei).
fn white_pixmaps() -> Vec<ksni::Icon> {
    // Iconița e în program, nu citită de pe disc: în Flatpak și AppImage dosarul proiectului nu există.
    const SVG: &[u8] = include_bytes!("../../../data/icons/hicolor/symbolic/apps/io.github.ThinkRoot99.Lanruj-symbolic.svg");
    let mut out = Vec::new();
    for size in [16, 22, 24, 32, 48] {
        let stream = gtk::gio::MemoryInputStream::from_bytes(&gtk::glib::Bytes::from_static(SVG));
        let Ok(pb) = gtk::gdk_pixbuf::Pixbuf::from_stream_at_scale(&stream, size, size, true, gtk::gio::Cancellable::NONE) else { continue };
        let (w, h) = (pb.width(), pb.height());
        let (stride, n) = (pb.rowstride() as usize, pb.n_channels() as usize);
        let bytes = pb.read_pixel_bytes();
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            for x in 0..w as usize {
                let i = y * stride + x * n;
                let a = if n == 4 { bytes[i + 3] } else { 255 };
                data.extend_from_slice(&[a, 255, 255, 255]);
            }
        }
        out.push(ksni::Icon { width: w, height: h, data });
    }
    out
}

fn open_window() {
    glib::idle_add_once(|| {
        if let Some(app) = gio::Application::default() {
            app.activate();
        }
    });
}

fn quit() {
    glib::idle_add_once(|| {
        if let Some(app) = gio::Application::default() {
            app.quit();
        }
    });
}

impl ksni::Tray for LanrujTray {
    fn id(&self) -> String {
        APP_ID.into()
    }

    fn title(&self) -> String {
        "Lanruj".into()
    }

    fn icon_name(&self) -> String {
        // Cu imaginea gata făcută, numele rămâne gol, ca bara să folosească imaginea.
        if self.pixmaps.is_empty() { format!("{APP_ID}-symbolic") } else { String::new() }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.pixmaps.clone()
    }

    fn icon_theme_path(&self) -> String {
        // Cât timp aplicația nu e instalată, iconițele stau în dosarul proiectului.
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/icons");
        if std::path::Path::new(dir).is_dir() { dir.into() } else { String::new() }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        open_window();
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        let ro = self.lang == Lang::Ro;
        vec![
            StandardItem {
                label: if ro { "Deschide Lanruj" } else { "Open Lanruj" }.into(),
                activate: Box::new(|_| open_window()),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: if ro { "Ieșire" } else { "Quit" }.into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| quit()),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Pornește iconița; `None` dacă sistemul nu are systray (de exemplu GNOME fără extensia AppIndicator).
pub fn start(lang: Lang) -> Option<ksni::blocking::Handle<LanrujTray>> {
    use ksni::blocking::TrayMethods;
    // În Flatpak, iconița nu-și poate lua un nume pe D-Bus; se anunță doar cu numele unic al conexiunii.
    LanrujTray { lang, pixmaps: white_pixmaps() }.disable_dbus_name(crate::in_flatpak()).spawn().ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn iconita_alba_la_toate_marimile() {
        let icons = super::white_pixmaps();
        assert_eq!(icons.iter().map(|i| i.width).collect::<Vec<_>>(), vec![16, 22, 24, 32, 48]);
        for i in &icons {
            assert_eq!(i.data.len(), (i.width * i.height * 4) as usize);
            let opaque = i.data.chunks(4).filter(|p| p[0] > 128).count();
            assert!(opaque > (i.width * i.height / 8) as usize, "iconița de {} px e aproape goală", i.width);
        }
    }
}
