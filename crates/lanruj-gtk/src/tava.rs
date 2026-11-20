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
        format!("{APP_ID}-symbolic")
    }

    fn icon_theme_path(&self) -> String {
        // Cât timp aplicația nu e instalată, iconițele stau în dosarul proiectului.
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/icons").into()
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
    LanrujTray { lang }.spawn().ok()
}
