// SPDX-License-Identifier: GPL-3.0-or-later
//! Monitorizarea: luna de serviciu ca un calendar (stânga) și totalurile ei (dreapta).
//! Somnul și Activitățile vin într-un pas următor.

use crate::page::Page;
use crate::{App, View, tr};
use adw::prelude::*;
use chrono::{Datelike, Duration, NaiveDate};
use lanruj_core::Lang;
use lanruj_core::monitor::{Shift, Totals, Tracking};
use std::rc::Rc;

const KINDS: [(Option<Shift>, &str, &str, &str); 4] = [
    (Some(Shift::Shift), "Tură", "Shift", "T"),
    (Some(Shift::Extra), "Tură extra", "Extra shift", "E"),
    (Some(Shift::Leave), "Concediu", "Leave", "C"),
    (None, "Liber", "Free", "Del"),
];

fn badge(kind: Shift, lang: Lang) -> (&'static str, &'static str) {
    match kind {
        Shift::Shift => (tr(lang, "Tură", "Shift"), "tura"),
        Shift::Extra => (tr(lang, "Extra", "Extra"), "extra"),
        Shift::Leave => (tr(lang, "CO", "Leave"), "concediu"),
    }
}

impl App {
    pub fn go_tracking(self: &Rc<Self>, y: i32, m: u32) {
        self.leave_search();
        if self.migrating() {
            self.end_migration(false);
        }
        self.editing.set(None);
        self.selected.set(None);
        self.view.set(View::Tracking(y, m));
        self.refresh();
    }

    pub fn shift_tracking(self: &Rc<Self>, delta: i32) {
        if let View::Tracking(y, m) = self.view.get() {
            let idx = y * 12 + m as i32 - 1 + delta;
            self.go_tracking(idx.div_euclid(12), (idx.rem_euclid(12) + 1) as u32);
        }
    }

    pub fn refresh_tracking(self: &Rc<Self>, y: i32, m: u32) {
        let t = self.journal.load_tracking(y, m).unwrap_or_else(|e| {
            self.error(&e);
            Tracking::default()
        });
        let title = self.lang.month_title(y, m);
        self.left.fill_tracking(self, y, m, &t, &title);
        self.right.fill_tracking_totals(self, &t);
        self.title.set_title(&format!("{} · {}", tr(self.lang, "Serviciu", "Work"), title));
        self.title.set_subtitle(&summary(self.lang, t.totals()));
    }

    /// Schimbă o zi și reface ecranul, păstrând ziua aleasă (pentru tastatură).
    fn set_day(self: &Rc<Self>, d: NaiveDate, kind: Option<Shift>) {
        match self.journal.set_shift(d, kind, self.lang) {
            Ok(_) => {
                self.tracking_focus.set(Some(d));
                self.refresh();
            }
            Err(e) => self.error(&e),
        }
    }
}

/// „8 ture · 2 extra · 3 zile de concediu”.
fn summary(lang: Lang, t: Totals) -> String {
    match lang {
        Lang::Ro => {
            let ture = match t.shifts {
                1 => "o tură".to_string(),
                n if n % 100 >= 20 => format!("{n} de ture"),
                n => format!("{n} ture"),
            };
            let co = match t.leave {
                1 => "o zi de concediu".to_string(),
                n if n % 100 >= 20 => format!("{n} de zile de concediu"),
                n => format!("{n} zile de concediu"),
            };
            format!("{ture} · {} extra · {co}", t.extra)
        }
        Lang::En => {
            let s = |n: u32, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
            format!("{} · {} · {}", s(t.shifts, "shift", "shifts"), s(t.extra, "extra", "extra"), s(t.leave, "day of leave", "days of leave"))
        }
    }
}

impl Page {
    /// Stânga: filele (Serviciu acum; Somn și Activități mai târziu) și luna ca un calendar.
    pub fn fill_tracking(&self, app: &Rc<App>, y: i32, m: u32, t: &Tracking, title: &str) {
        let lang = app.lang;
        self.header(tr(lang, "Monitorizare", "Tracking"), title);
        if let Some(q) = &self.quick {
            q.set_visible(false);
        }
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        tabs.add_css_class("file-monitorizare");
        for (name, on) in [(tr(lang, "Serviciu", "Work"), true), (tr(lang, "Somn", "Sleep"), false), (tr(lang, "Activități", "Activities"), false)] {
            let l = gtk::Label::new(Some(name));
            l.add_css_class("fila");
            if on {
                l.add_css_class("aleasa");
            } else {
                l.add_css_class("curand");
                l.set_tooltip_text(Some(tr(lang, "Vine într-un pas următor", "Coming in a later step")));
            }
            tabs.append(&l);
        }
        self.list.append(&tabs);

        let grid = gtk::Grid::builder().column_homogeneous(true).row_spacing(6).column_spacing(6).css_classes(["luna-serviciu"]).build();
        let names: [&str; 7] = match lang {
            Lang::Ro => ["Lu", "Ma", "Mi", "Jo", "Vi", "Sâ", "Du"],
            Lang::En => ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
        };
        for (i, n) in names.iter().enumerate() {
            grid.attach(&gtk::Label::builder().label(*n).css_classes(["zi-sapt"]).build(), i as i32, 0, 1, 1);
        }
        let first = NaiveDate::from_ymd_opt(y, m, 1).expect("lună validă");
        let offset = first.weekday().num_days_from_monday() as i32;
        let today = app.today.get();
        let focus = app.tracking_focus.get();
        let mut d = first;
        let mut focus_button = None;
        while d.month() == m {
            let i = offset + d.day() as i32 - 1;
            let cell = day_button(app, d, t.work.get(&d.day()).copied(), d == today);
            if focus == Some(d) || (focus.is_none() && d == today) {
                focus_button = Some(cell.clone());
            }
            grid.attach(&cell, i % 7, 1 + i / 7, 1, 1);
            d += Duration::days(1);
        }
        self.list.append(&grid);
        self.list.append(
            &gtk::Label::builder()
                .label(tr(
                    lang,
                    "Clic pe o zi: Tură, Tură extra, Concediu sau Liber. Cu tastatura, pe ziua aleasă: T, E, C, Delete.",
                    "Click a day: Shift, Extra shift, Leave or Free. With the keyboard, on the chosen day: T, E, L, Delete.",
                ))
                .wrap(true)
                .xalign(0.0)
                .css_classes(["indiciu"])
                .build(),
        );
        if let Some(b) = focus_button.filter(|_| focus.is_some()) {
            gtk::glib::idle_add_local_once(move || {
                b.grab_focus();
            });
        }
    }

    /// Dreapta: totalurile lunii și legenda.
    pub fn fill_tracking_totals(&self, app: &Rc<App>, t: &Tracking) {
        let lang = app.lang;
        self.header(tr(lang, "Totalurile lunii", "Month totals"), tr(lang, "Serviciu", "Work"));
        if let Some(q) = &self.quick {
            q.set_visible(false);
        }
        let tot = t.totals();
        let h = |n: u32| format!("{} h", n * Shift::HOURS);
        let rows = [
            (tr(lang, "Ture", "Shifts"), tot.shifts.to_string(), h(tot.shifts)),
            (tr(lang, "Ture extra", "Extra shifts"), tot.extra.to_string(), h(tot.extra)),
            (tr(lang, "Zile de concediu", "Days of leave"), tot.leave.to_string(), String::new()),
            (tr(lang, "Total ore lucrate", "Total hours worked"), format!("{} h", tot.hours()), String::new()),
        ];
        for (name, value, extra) in rows {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("total-rand");
            row.append(&gtk::Label::builder().label(name).xalign(0.0).hexpand(true).build());
            if !extra.is_empty() {
                row.append(&gtk::Label::builder().label(extra).css_classes(["total-ore"]).valign(gtk::Align::Baseline).build());
            }
            row.append(&gtk::Label::builder().label(value).css_classes(["total-nr"]).build());
            self.list.append(&row);
        }
        let leg = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        leg.add_css_class("legenda");
        for (kind, text) in [
            (Shift::Shift, tr(lang, "Tură, 24 h", "Shift, 24 h")),
            (Shift::Extra, tr(lang, "Tură extra", "Extra shift")),
            (Shift::Leave, tr(lang, "CO = concediu", "Leave")),
        ] {
            let b = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let dot = gtk::Box::builder().css_classes(["patrat", badge(kind, lang).1]).valign(gtk::Align::Center).build();
            b.append(&dot);
            b.append(&gtk::Label::new(Some(text)));
            leg.append(&b);
        }
        self.list.append(&leg);
    }
}

/// O zi din calendarul de serviciu: numărul și, dedesubt, ce a fost.
fn day_button(app: &Rc<App>, d: NaiveDate, kind: Option<Shift>, today: bool) -> gtk::Button {
    let lang = app.lang;
    let b = gtk::Button::builder().css_classes(["zi-serviciu"]).build();
    if d.weekday().num_days_from_monday() >= 5 {
        b.add_css_class("weekend");
    }
    if today {
        b.add_css_class("azi");
    }
    let inside = gtk::Box::new(gtk::Orientation::Vertical, 2);
    inside.append(&gtk::Label::builder().label(d.day().to_string()).xalign(0.0).css_classes(["nr"]).build());
    let tag = gtk::Label::builder().css_classes(["eticheta-zi"]).build();
    if let Some(k) = kind {
        let (text, class) = badge(k, lang);
        tag.set_label(text);
        b.add_css_class(class);
    } else {
        tag.set_label(" ");
        tag.add_css_class("goala");
    }
    inside.append(&tag);
    b.set_child(Some(&inside));
    b.set_tooltip_text(Some(&lang.day_title(d)));

    // Clic: meniul cu cele patru variante.
    let pop = gtk::Popover::new();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    for (k, ro, en, key) in KINDS {
        let item = gtk::Button::builder().css_classes(["flat"]).build();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        row.append(&gtk::Label::builder().label(tr(lang, ro, en)).xalign(0.0).hexpand(true).build());
        let key = if lang == Lang::En && key == "C" { "L" } else { key };
        row.append(&gtk::Label::builder().label(key).css_classes(["dim-label", "monospace"]).build());
        item.set_child(Some(&row));
        let (weak, p) = (Rc::downgrade(app), pop.clone());
        item.connect_clicked(move |_| {
            p.popdown();
            if let Some(app) = weak.upgrade() {
                app.set_day(d, k);
            }
        });
        menu.append(&item);
    }
    pop.set_child(Some(&menu));
    pop.set_parent(&b);
    let p = pop.clone();
    b.connect_clicked(move |_| p.popup());
    b.connect_destroy(move |_| pop.unparent());

    // Tastele, pe ziua aleasă: T, E, C (L în engleză), Delete.
    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(app);
    keys.connect_key_pressed(move |_, key, _, mods| {
        if mods.intersects(gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::ALT_MASK) {
            return gtk::glib::Propagation::Proceed;
        }
        let Some(app) = weak.upgrade() else { return gtk::glib::Propagation::Proceed };
        let kind = match key.to_unicode().map(|c| c.to_ascii_lowercase()) {
            Some('t') => Some(Some(Shift::Shift)),
            Some('e') => Some(Some(Shift::Extra)),
            Some('c') | Some('l') => Some(Some(Shift::Leave)),
            _ if matches!(key, gtk::gdk::Key::Delete | gtk::gdk::Key::BackSpace) => Some(None),
            _ => None,
        };
        match kind {
            Some(k) => {
                app.set_day(d, k);
                gtk::glib::Propagation::Stop
            }
            None => gtk::glib::Propagation::Proceed,
        }
    });
    b.add_controller(keys);
    b
}
