// SPDX-License-Identifier: GPL-3.0-or-later
//! Monitorizarea, pe luni, cu trei file:
//! - Activități: lista ta (cafea, citit…) bifată pe zile, și de câte ori ai făcut fiecare;
//! - Somn: câte ore ai dormit noaptea și ziua, cu graficul lunii;
//! - Serviciu: ture, concediu și ore suplimentare, cu totalurile lunii.

use crate::page::Page;
use crate::{App, View, tr};
use adw::prelude::*;
use chrono::{Datelike, Duration, NaiveDate};
use gtk::glib;
use lanruj_core::Lang;
use lanruj_core::monitor::{Activity, Overtime, Shift, Span, Totals, Tracking, WorkOnDay, duration, parse_duration, parse_seconds, seconds_text};
use std::rc::Rc;

/// Fila aleasă.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Activities,
    Sleep,
    Work,
}

fn days_in(y: i32, m: u32) -> Vec<NaiveDate> {
    let mut d = NaiveDate::from_ymd_opt(y, m, 1).expect("lună validă");
    let mut out = Vec::new();
    while d.month() == m {
        out.push(d);
        d += Duration::days(1);
    }
    out
}

fn short_day(lang: Lang, d: NaiveDate) -> String {
    let n = match lang {
        Lang::Ro => ["Lu", "Ma", "Mi", "Jo", "Vi", "Sâ", "Du"],
        Lang::En => ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    }[d.weekday().num_days_from_monday() as usize];
    format!("{n} {}", d.day())
}

/// „7 h 30 min”, „45 min”, „8 h”.
fn hours_text(m: u32) -> String {
    match (m / 60, m % 60) {
        (0, mm) => format!("{mm} min"),
        (h, 0) => format!("{h} h"),
        (h, mm) => format!("{h} h {mm} min"),
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
        let tab = self.tracking_tab.get();
        self.left.fill_tracking(self, y, m, &t, &title, tab);
        let lang = self.lang;
        let (name, sub) = match tab {
            Tab::Work => {
                self.right.fill_work_totals(self, &t);
                (tr(lang, "Serviciu", "Work"), work_summary(lang, t.totals()))
            }
            Tab::Sleep => {
                self.right.fill_sleep_chart(self, y, m, &t);
                let (n, _) = t.sleep_average();
                (tr(lang, "Somn", "Sleep"), match n {
                    Some(n) => format!("{} {}", tr(lang, "noaptea, în medie", "on average at night"), hours_text(n)),
                    None => tr(lang, "nimic trecut încă", "nothing entered yet").to_string(),
                })
            }
            Tab::Activities => {
                self.right.fill_activity_counts(self, y, m, &t);
                (tr(lang, "Activități", "Activities"), match t.activities.len() {
                    0 => tr(lang, "adaugă prima activitate", "add your first activity").to_string(),
                    n => match lang {
                        Lang::Ro if n == 1 => "o activitate".to_string(),
                        Lang::Ro if n >= 20 => format!("{n} de activități"),
                        Lang::Ro => format!("{n} activități"),
                        Lang::En if n == 1 => "1 activity".to_string(),
                        Lang::En => format!("{n} activities"),
                    },
                })
            }
        };
        self.title.set_title(&format!("{name} · {title}"));
        self.title.set_subtitle(&sub);
        if let Some(b) = self.tracking_mentions_box(y, m) {
            b.set_margin_top(14);
            self.right.list.append(&b);
        }
    }

    /// Schimbă luna și reface ecranul; `focus` = ziua de ales după (pentru tastatură).
    fn change_tracking(self: &Rc<Self>, y: i32, m: u32, focus: Option<NaiveDate>, f: impl FnOnce(&mut Tracking)) {
        match self.journal.update_tracking(y, m, self.lang, f) {
            Ok(_) => {
                self.tracking_focus.set(focus);
                self.refresh();
            }
            Err(e) => self.error(&e),
        }
    }
}

/// „8 ture · 4 h suplimentare · 3 zile de concediu”.
fn work_summary(lang: Lang, t: Totals) -> String {
    let ot = hours_text(t.overtime);
    match lang {
        Lang::Ro => {
            let ture = match t.shifts {
                1 => "un serviciu".to_string(),
                n if n % 100 >= 20 => format!("{n} de servicii"),
                n => format!("{n} servicii"),
            };
            let rez = match t.reserve {
                1 => "o rezervă".to_string(),
                n if n % 100 >= 20 => format!("{n} de rezerve"),
                n => format!("{n} rezerve"),
            };
            let co = match t.leave {
                1 => "o zi de concediu".to_string(),
                n if n % 100 >= 20 => format!("{n} de zile de concediu"),
                n => format!("{n} zile de concediu"),
            };
            format!("{ture} · {rez} · {ot} suplimentare · {co}")
        }
        Lang::En => {
            let s = |n: u32, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
            format!("{} · {} · {ot} overtime · {}", s(t.shifts, "duty", "duties"), s(t.reserve, "standby", "standbys"), s(t.leave, "day of leave", "days of leave"))
        }
    }
}

impl Page {
    /// Stânga: filele și, după fila aleasă, activitățile, somnul sau luna de serviciu.
    pub fn fill_tracking(&self, app: &Rc<App>, y: i32, m: u32, t: &Tracking, title: &str, tab: Tab) {
        let lang = app.lang;
        self.header(tr(lang, "Monitorizare", "Tracking"), title);
        if let Some(q) = &self.quick {
            q.set_visible(false);
        }
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        tabs.add_css_class("file-monitorizare");
        for (which, name) in [
            (Tab::Activities, tr(lang, "Activități", "Activities")),
            (Tab::Sleep, tr(lang, "Somn", "Sleep")),
            (Tab::Work, tr(lang, "Serviciu", "Work")),
        ] {
            let b = gtk::Button::builder().label(name).css_classes(["fila"]).build();
            if which == tab {
                b.add_css_class("aleasa");
            }
            let weak = Rc::downgrade(app);
            b.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.tracking_tab.set(which);
                    app.tracking_focus.set(None);
                    app.refresh();
                }
            });
            tabs.append(&b);
        }
        self.list.append(&tabs);
        match tab {
            Tab::Activities => self.activities_grid(app, y, m, t),
            Tab::Sleep => self.sleep_rows(app, y, m, t),
            Tab::Work => self.work_calendar(app, y, m, t),
        }
    }

    // ---- Activități ----

    fn activities_grid(&self, app: &Rc<App>, y: i32, m: u32, t: &Tracking) {
        let lang = app.lang;
        // Adaugă o activitate: numele, felul (bifă sau număr) și, la număr, unitatea (opțional).
        let form = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        form.add_css_class("activitate-noua");
        let add = gtk::Entry::builder()
            .placeholder_text(tr(lang, "Activitate nouă (cafea, flotări, apă…)", "New activity (coffee, push-ups, water…)"))
            .hexpand(true)
            .build();
        let kinds = gtk::StringList::new(&[tr(lang, "cu bifă", "tick"), tr(lang, "cu număr", "count"), tr(lang, "cu timp", "time")]);
        let kind = gtk::DropDown::builder().model(&kinds).build();
        let unit = gtk::Entry::builder().placeholder_text(tr(lang, "unitate: pahare", "unit: glasses")).width_chars(12).visible(false).build();
        let u2 = unit.clone();
        kind.connect_selected_notify(move |k| u2.set_visible(k.selected() == 1));
        form.append(&add);
        form.append(&kind);
        form.append(&unit);
        let submit = {
            let (weak, add, kind, unit) = (Rc::downgrade(app), add.clone(), kind.clone(), unit.clone());
            move || {
                let name = add.text().trim().replace([',', '(', ')'], " ").trim().to_string();
                let Some(app) = weak.upgrade() else { return };
                if name.is_empty() {
                    return;
                }
                let (counted, timed) = (kind.selected() >= 1, kind.selected() == 2);
                let u = unit.text().trim().replace([',', '(', ')'], " ").trim().to_string();
                let a = Activity { name: name.clone(), counted, unit: (counted && !timed && !u.is_empty()).then_some(u), timed };
                app.change_tracking(y, m, None, move |t| {
                    if !t.activities.iter().any(|x| x.name.to_lowercase() == name.to_lowercase()) {
                        t.activities.push(a);
                    }
                });
            }
        };
        let s1 = submit.clone();
        add.connect_activate(move |_| s1());
        unit.connect_activate(move |_| submit());
        self.list.append(&form);
        if t.activities.is_empty() {
            self.list.append(
                &gtk::Label::builder()
                    .label(tr(
                        lang,
                        "Scrie mai sus ce vrei să urmărești în fiecare zi. „Cu bifă” e pentru lucruri făcute sau nu (cafea, făcut patul); „cu număr” e pentru cantități (flotări, apă în pahare); „cu timp” e pentru durate (plank: 1:30).",
                        "Type above what you want to follow every day. \"Tick\" is for things done or not (coffee, making the bed); \"count\" is for amounts (push-ups, water in glasses); \"time\" is for durations (plank: 1:30).",
                    ))
                    .wrap(true)
                    .xalign(0.0)
                    .css_classes(["indiciu"])
                    .build(),
            );
            return;
        }
        let grid = gtk::Grid::builder().row_spacing(2).column_spacing(6).css_classes(["grila-activitati"]).build();
        // Capul de tabel: numele activităților (și unitatea); clic dreapta: scoate.
        for (c, a) in t.activities.iter().enumerate() {
            let head = match (&a.unit, a.timed) {
                (_, true) => format!("{}\n({})", a.name, tr(lang, "min:s", "min:s")),
                (Some(u), _) => format!("{}\n({u})", a.name),
                (None, _) => a.name.clone(),
            };
            let l = gtk::Label::builder()
                .label(head)
                .justify(gtk::Justification::Center)
                .css_classes(["activitate-nume"])
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .max_width_chars(7)
                .valign(gtk::Align::End)
                .build();
            l.set_tooltip_text(Some(&format!("{}\n{}", a.name, tr(lang, "Clic dreapta: scoate din listă", "Right-click: remove from the list"))));
            let menu = gtk::GestureClick::builder().button(3).build();
            let (weak, name) = (Rc::downgrade(app), a.name.clone());
            menu.connect_pressed(move |_, _, _, _| {
                if let Some(app) = weak.upgrade() {
                    let n = name.clone();
                    app.change_tracking(y, m, None, move |t| {
                        t.activities.retain(|x| x.name != n);
                        for s in t.done.values_mut() {
                            s.remove(&n);
                        }
                    });
                }
            });
            l.add_controller(menu);
            grid.attach(&l, c as i32 + 1, 0, 1, 1);
        }
        let today = app.today.get();
        for (r, d) in days_in(y, m).into_iter().enumerate() {
            let row = r as i32 + 1;
            let l = gtk::Label::builder().label(short_day(lang, d)).xalign(0.0).css_classes(["zi-activitate"]).build();
            if d.weekday().num_days_from_monday() >= 5 {
                l.add_css_class("weekend");
            }
            if d == today {
                l.add_css_class("azi");
            }
            grid.attach(&l, 0, row, 1, 1);
            let done = t.done.get(&d.day());
            for (c, a) in t.activities.iter().enumerate() {
                let n = done.and_then(|s| s.get(&a.name)).copied().unwrap_or(0);
                let (name, day) = (a.name.clone(), d.day());
                // Salvează valoarea zilei fără să refacă ecranul; se refac doar totalurile din dreapta.
                let store = {
                    let weak = Rc::downgrade(app);
                    move |v: u32| {
                        let Some(app) = weak.upgrade() else { return };
                        let n = name.clone();
                        let res = app.journal.update_tracking(y, m, app.lang, move |t| {
                            let s = t.done.entry(day).or_default();
                            if v > 0 { s.insert(n, v); } else { s.remove(&n); }
                            if s.is_empty() {
                                t.done.remove(&day);
                            }
                        });
                        match res {
                            Ok(t) => app.right.fill_activity_counts(&app, y, m, &t),
                            Err(e) => app.error(&e),
                        }
                    }
                };
                let w: gtk::Widget = if a.counted {
                    let timed = a.timed;
                    let e = gtk::Entry::builder().width_chars(if timed { 4 } else { 3 }).max_width_chars(5).xalign(0.5).css_classes(["numar-activitate"]).build();
                    if n > 0 {
                        e.set_text(&a.value_text(n));
                    }
                    if !timed {
                        e.set_input_purpose(gtk::InputPurpose::Digits);
                    }
                    e.set_sensitive(d <= today);
                    let last = Rc::new(std::cell::Cell::new(n));
                    let save = {
                        let last = last.clone();
                        move |e: &gtk::Entry| {
                            let text = e.text().trim().to_string();
                            let v = if text.is_empty() { Some(0) } else if timed { parse_seconds(&text) } else { text.parse::<u32>().ok() };
                            let Some(v) = v else {
                                e.add_css_class("error");
                                return;
                            };
                            e.remove_css_class("error");
                            if v != last.get() {
                                last.set(v);
                                store(v);
                            }
                        }
                    };
                    let s1 = save.clone();
                    e.connect_activate(move |e| s1(e));
                    let fo = gtk::EventControllerFocus::new();
                    let ew = e.clone();
                    fo.connect_leave(move |_| save(&ew));
                    e.add_controller(fo);
                    e.upcast()
                } else {
                    let cb = gtk::CheckButton::builder().active(n > 0).halign(gtk::Align::Center).build();
                    cb.set_sensitive(d <= today);
                    cb.connect_toggled(move |b| store(u32::from(b.is_active())));
                    cb.upcast()
                };
                grid.attach(&w, c as i32 + 1, row, 1, 1);
            }
        }
        self.list.append(&grid);
    }

    /// Dreapta, la Activități: în câte zile ai făcut fiecare și, la cele cu număr, cât în total.
    pub fn fill_activity_counts(&self, app: &Rc<App>, y: i32, m: u32, t: &Tracking) {
        let lang = app.lang;
        self.header(tr(lang, "Luna aceasta", "This month"), tr(lang, "De câte ori", "How often"));
        if let Some(q) = &self.quick {
            q.set_visible(false);
        }
        let today = app.today.get();
        // Zilele trecute ale lunii (toată luna, dacă e o lună trecută).
        let passed = days_in(y, m).into_iter().filter(|d| *d <= today).count() as u32;
        for (a, days, sum) in t.activity_counts() {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
            row.add_css_class("total-rand");
            let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            top.append(&gtk::Label::builder().label(&a.name).xalign(0.0).hexpand(true).build());
            let of = match lang {
                Lang::Ro => format!("{days} din {passed} zile"),
                Lang::En => format!("{days} of {passed} days"),
            };
            if a.timed {
                let avg = sum.checked_div(days).unwrap_or(0);
                let total = match lang {
                    Lang::Ro => format!("{} în total · {} pe zi · {of}", seconds_text(sum), seconds_text(avg)),
                    Lang::En => format!("{} in total · {} a day · {of}", seconds_text(sum), seconds_text(avg)),
                };
                top.append(&gtk::Label::builder().label(total).css_classes(["total-ore"]).build());
            } else if a.counted {
                let unit = a.unit.clone().unwrap_or_default();
                let avg = if days > 0 { sum as f64 / days as f64 } else { 0.0 };
                let avg = format!("{avg:.1}");
                let avg = if lang == Lang::Ro { avg.replace('.', ",") } else { avg };
                let total = match lang {
                    Lang::Ro => format!("{sum} {unit} · {avg} pe zi · {of}"),
                    Lang::En => format!("{sum} {unit} · {avg} a day · {of}"),
                };
                top.append(&gtk::Label::builder().label(total.replace("  ", " ")).css_classes(["total-ore"]).build());
            } else {
                top.append(&gtk::Label::builder().label(of).css_classes(["total-ore"]).build());
            }
            row.append(&top);
            let bar = gtk::LevelBar::builder().min_value(0.0).max_value(passed.max(1) as f64).value(days as f64).css_classes(["bara-activitate"]).build();
            row.append(&bar);
            self.list.append(&row);
        }
    }

    /// O casetă pentru pagina zilei sau a săptămânii: ce a fost la serviciu (cu notele).
    /// Clic: deschide Monitorizare → Serviciu în luna zilei.
    pub fn work_box(app: &Rc<App>, d: NaiveDate, items: &[WorkOnDay], big: bool) -> gtk::Box {
        let lang = app.lang;
        let b = gtk::Box::new(gtk::Orientation::Vertical, 2);
        b.add_css_class("tema-zi");
        b.add_css_class("serviciu-zi");
        if big {
            b.add_css_class("mare");
        }
        for it in items {
            let (head, note) = match it {
                WorkOnDay::Day(Shift::Service, n) => (format!("▸ {} · 24 h", tr(lang, "Serviciu", "Duty")), n),
                WorkOnDay::Day(Shift::Reserve, n) => (format!("▸ {}", tr(lang, "Rezervă", "Standby")), n),
                WorkOnDay::Day(Shift::Leave, n) => (format!("▸ {}", tr(lang, "Concediu", "Leave")), n),
                WorkOnDay::Overtime(span, n) => (format!("▸ {} {}", tr(lang, "Ore suplimentare", "Overtime"), span.text()), n),
            };
            b.append(&gtk::Label::builder().label(head).xalign(0.0).wrap(true).css_classes(["tema-titlu"]).build());
            if !note.is_empty() {
                let l = gtk::Label::builder().label(note.as_str()).xalign(0.0).wrap(true).css_classes(["text"]).build();
                l.set_margin_start(14);
                b.append(&l);
            }
        }
        b.set_tooltip_text(Some(tr(lang, "Clic: deschide Monitorizare → Serviciu", "Click: open Tracking → Work")));
        b.set_cursor_from_name(Some("pointer"));
        let click = gtk::GestureClick::builder().button(1).build();
        let weak = Rc::downgrade(app);
        click.connect_released(move |_, _, _, _| {
            if let Some(app) = weak.upgrade() {
                glib::idle_add_local_once(move || {
                    app.tracking_tab.set(Tab::Work);
                    app.tracking_focus.set(Some(d));
                    app.go_tracking(d.year(), d.month());
                });
            }
        });
        b.add_controller(click);
        b
    }

    // ---- Somn ----

    fn sleep_rows(&self, app: &Rc<App>, y: i32, m: u32, t: &Tracking) {
        let lang = app.lang;
        self.list.append(
            &gtk::Label::builder()
                .label(tr(lang, "Câte ore ai dormit: scrie 7:30, 7,5 sau 45m, apoi Enter.", "How long you slept: type 7:30, 7.5 or 45m, then Enter."))
                .wrap(true)
                .xalign(0.0)
                .css_classes(["indiciu"])
                .build(),
        );
        let grid = gtk::Grid::builder().row_spacing(3).column_spacing(10).css_classes(["grila-somn"]).build();
        grid.attach(&gtk::Label::builder().label(tr(lang, "Noaptea", "Night")).css_classes(["activitate-nume"]).build(), 1, 0, 1, 1);
        grid.attach(&gtk::Label::builder().label(tr(lang, "Ziua", "Day")).css_classes(["activitate-nume"]).build(), 2, 0, 1, 1);
        let today = app.today.get();
        let focus = app.tracking_focus.get();
        for (r, d) in days_in(y, m).into_iter().enumerate() {
            let row = r as i32 + 1;
            let l = gtk::Label::builder().label(short_day(lang, d)).xalign(0.0).css_classes(["zi-activitate"]).build();
            if d.weekday().num_days_from_monday() >= 5 {
                l.add_css_class("weekend");
            }
            if d == today {
                l.add_css_class("azi");
            }
            grid.attach(&l, 0, row, 1, 1);
            let s = t.sleep.get(&d.day()).copied().unwrap_or_default();
            for (col, night) in [(1, true), (2, false)] {
                let e = gtk::Entry::builder().width_chars(6).max_width_chars(6).css_classes(["ore-somn"]).build();
                if let Some(v) = if night { s.night } else { s.day } {
                    e.set_text(&duration(v));
                }
                e.set_sensitive(d <= today);
                let save = {
                    let weak = Rc::downgrade(app);
                    let day = d.day();
                    move |e: &gtk::Entry, next: bool| {
                        let Some(app) = weak.upgrade() else { return };
                        let text = e.text().to_string();
                        let v = parse_duration(&text);
                        if !text.trim().is_empty() && v.is_none() {
                            e.add_css_class("error");
                            return;
                        }
                        e.remove_css_class("error");
                        let old = app.journal.load_tracking(y, m).ok().and_then(|t| t.sleep.get(&day).copied()).unwrap_or_default();
                        if (if night { old.night } else { old.day }) == v && !next {
                            return;
                        }
                        let focus = next.then(|| NaiveDate::from_ymd_opt(y, m, day + 1)).flatten();
                        app.change_tracking(y, m, focus, move |t| {
                            let s = t.sleep.entry(day).or_default();
                            if night { s.night = v } else { s.day = v }
                            if s.night.is_none() && s.day.is_none() {
                                t.sleep.remove(&day);
                            }
                        });
                    }
                };
                let s1 = save.clone();
                e.connect_activate(move |e| s1(e, true));
                let focus_out = gtk::EventControllerFocus::new();
                let ew = e.clone();
                focus_out.connect_leave(move |_| {
                    let ew = ew.clone();
                    let s2 = save.clone();
                    // După ce se termină schimbarea de focus, ca reîmprospătarea să nu o încurce.
                    glib::idle_add_local_once(move || s2(&ew, false));
                });
                e.add_controller(focus_out);
                if night && focus == Some(d) {
                    let ef = e.clone();
                    glib::idle_add_local_once(move || {
                        ef.grab_focus();
                    });
                }
                grid.attach(&e, col, row, 1, 1);
            }
        }
        self.list.append(&grid);
    }

    /// Dreapta, la Somn: graficul lunii (noaptea și ziua, una peste alta) și mediile.
    pub fn fill_sleep_chart(&self, app: &Rc<App>, y: i32, m: u32, t: &Tracking) {
        let lang = app.lang;
        self.header(tr(lang, "Graficul lunii", "The month in a chart"), tr(lang, "Somn", "Sleep"));
        if let Some(q) = &self.quick {
            q.set_visible(false);
        }
        let n_days = days_in(y, m).len() as u32;
        let data: Vec<(u32, u32)> = (1..=n_days).map(|d| t.sleep.get(&d).map(|s| (s.night.unwrap_or(0), s.day.unwrap_or(0))).unwrap_or((0, 0))).collect();
        let area = gtk::DrawingArea::builder().content_height(240).hexpand(true).css_classes(["grafic-somn"]).build();
        let dark = app.window.has_css_class("noapte");
        area.set_draw_func(move |_, cr, w, h| draw_sleep(cr, w as f64, h as f64, &data, dark));
        self.list.append(&area);

        let leg = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        leg.add_css_class("legenda");
        for (class, text) in [("noaptea", tr(lang, "Noaptea", "Night")), ("ziua", tr(lang, "Ziua", "Day"))] {
            let b = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            b.append(&gtk::Box::builder().css_classes(["patrat", class]).valign(gtk::Align::Center).build());
            b.append(&gtk::Label::new(Some(text)));
            leg.append(&b);
        }
        self.list.append(&leg);

        let (night, day) = t.sleep_average();
        let nights = t.sleep.values().filter(|s| s.night.is_some()).count();
        let rows = [
            (tr(lang, "Noaptea, în medie", "Average at night"), night.map(hours_text).unwrap_or_else(|| "-".into())),
            (tr(lang, "Ziua, în medie", "Average during the day"), day.map(hours_text).unwrap_or_else(|| "-".into())),
            (tr(lang, "Nopți trecute", "Nights entered"), nights.to_string()),
        ];
        for (name, value) in rows {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("total-rand");
            row.append(&gtk::Label::builder().label(name).xalign(0.0).hexpand(true).build());
            row.append(&gtk::Label::builder().label(value).css_classes(["total-nr", "mic"]).build());
            self.list.append(&row);
        }
    }

    // ---- Serviciu ----

    fn work_calendar(&self, app: &Rc<App>, y: i32, m: u32, t: &Tracking) {
        let lang = app.lang;
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
        for d in days_in(y, m) {
            let i = offset + d.day() as i32 - 1;
            let ot: Vec<Overtime> = t.overtime.get(&d.day()).cloned().unwrap_or_default();
            let note = t.work_notes.get(&d.day()).cloned().unwrap_or_default();
            let cell = day_button(app, d, t.work.get(&d.day()).copied(), &note, &ot, d == today);
            if focus == Some(d) {
                let c = cell.clone();
                glib::idle_add_local_once(move || {
                    c.grab_focus();
                });
            }
            grid.attach(&cell, i % 7, 1 + i / 7, 1, 1);
        }
        self.list.append(&grid);
        self.list.append(
            &gtk::Label::builder()
                .label(tr(
                    lang,
                    "Clic pe o zi: Serviciu, Rezervă, Concediu sau Liber, o notă și orele suplimentare. Cu tastatura, pe ziua aleasă: S, R, C, O, Delete.",
                    "Click a day: Duty, Standby, Leave or Free, a note and overtime. With the keyboard, on the chosen day: D, S, L, O, Delete.",
                ))
                .wrap(true)
                .xalign(0.0)
                .css_classes(["indiciu"])
                .build(),
        );
    }

    /// Dreapta, la Serviciu: totalurile lunii și legenda.
    pub fn fill_work_totals(&self, app: &Rc<App>, t: &Tracking) {
        let lang = app.lang;
        self.header(tr(lang, "Totalurile lunii", "Month totals"), tr(lang, "Serviciu", "Work"));
        if let Some(q) = &self.quick {
            q.set_visible(false);
        }
        let tot = t.totals();
        let rows = [
            (tr(lang, "Servicii", "Duties"), tot.shifts.to_string(), format!("{} h", tot.shifts * Shift::HOURS)),
            (tr(lang, "Rezerve", "Standbys"), tot.reserve.to_string(), String::new()),
            (tr(lang, "Ore suplimentare", "Overtime"), hours_text(tot.overtime), String::new()),
            (tr(lang, "Zile de concediu", "Days of leave"), tot.leave.to_string(), String::new()),
            (tr(lang, "Total ore lucrate", "Total hours worked"), hours_text(tot.worked()), String::new()),
        ];
        for (name, value, extra) in rows {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("total-rand");
            row.append(&gtk::Label::builder().label(name).xalign(0.0).hexpand(true).build());
            if !extra.is_empty() {
                row.append(&gtk::Label::builder().label(extra).css_classes(["total-ore"]).build());
            }
            row.append(&gtk::Label::builder().label(value).css_classes(["total-nr"]).build());
            self.list.append(&row);
        }
        // Legenda, câte două pe rând, ca să nu lățească pagina.
        let leg = gtk::FlowBox::builder().selection_mode(gtk::SelectionMode::None).max_children_per_line(2).min_children_per_line(1).column_spacing(14).row_spacing(4).css_classes(["legenda"]).build();
        for (class, text) in [
            ("tura", tr(lang, "Serv. = serviciu, 24 h", "Duty, 24 h")),
            ("rezerva", tr(lang, "Rez. = rezervă", "Standby")),
            ("concediu", tr(lang, "CO = concediu", "Leave")),
            ("extra", tr(lang, "Ore suplimentare", "Overtime")),
        ] {
            let b = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            b.append(&gtk::Box::builder().css_classes(["patrat", class]).valign(gtk::Align::Center).build());
            b.append(&gtk::Label::new(Some(text)));
            leg.append(&b);
        }
        self.list.append(&leg);
        // Notele lunii, pe zile, ca să se citească ușor.
        let notes = t.work_notes_list();
        if !notes.is_empty() {
            self.list.append(&gtk::Label::builder().label(tr(lang, "Notele lunii", "This month's notes")).xalign(0.0).css_classes(["index-sectiune", "note-luna-titlu"]).build());
            for (d, it) in notes {
                let (what, note) = work_label(lang, &it);
                let row = gtk::Box::new(gtk::Orientation::Vertical, 1);
                row.add_css_class("nota-luna");
                row.append(&gtk::Label::builder().label(format!("{d} · {what}")).xalign(0.0).css_classes(["tema-titlu"]).build());
                row.append(&gtk::Label::builder().label(note).xalign(0.0).wrap(true).css_classes(["text"]).build());
                self.list.append(&row);
            }
        }
    }
}

/// („Serviciu · 24 h”, nota) pentru o intrare de serviciu.
fn work_label(lang: Lang, it: &WorkOnDay) -> (String, String) {
    match it {
        WorkOnDay::Day(Shift::Service, n) => (format!("{} · 24 h", tr(lang, "Serviciu", "Duty")), n.clone()),
        WorkOnDay::Day(Shift::Reserve, n) => (tr(lang, "Rezervă", "Standby").to_string(), n.clone()),
        WorkOnDay::Day(Shift::Leave, n) => (tr(lang, "Concediu", "Leave").to_string(), n.clone()),
        WorkOnDay::Overtime(span, n) => (format!("{} {}", tr(lang, "Ore suplimentare", "Overtime"), span.text()), n.clone()),
    }
}

/// Graficul somnului: o bară pe zi, noaptea jos și ziua deasupra; linii la 4, 8 și 12 ore.
fn draw_sleep(cr: &gtk::cairo::Context, w: f64, h: f64, data: &[(u32, u32)], dark: bool) {
    let (ink, sun, grid, text) = if dark {
        ((0.62, 0.74, 0.90), (0.91, 0.70, 0.29), (1.0, 1.0, 1.0, 0.12), (1.0, 1.0, 1.0, 0.55))
    } else {
        ((0.12, 0.23, 0.37), (0.79, 0.54, 0.07), (0.12, 0.23, 0.37, 0.15), (0.12, 0.23, 0.37, 0.6))
    };
    let (left, bottom, top) = (28.0, 18.0, 6.0);
    let max = 12.0 * 60.0;
    let plot_h = h - bottom - top;
    let y_of = |min: f64| top + plot_h * (1.0 - (min / max).min(1.0));
    cr.select_font_face("sans-serif", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Normal);
    cr.set_font_size(10.0);
    for hrs in [4.0, 8.0, 12.0] {
        let y = y_of(hrs * 60.0);
        cr.set_source_rgba(grid.0, grid.1, grid.2, grid.3);
        cr.set_line_width(1.0);
        cr.move_to(left, y.round() + 0.5);
        cr.line_to(w, y.round() + 0.5);
        let _ = cr.stroke();
        cr.set_source_rgba(text.0, text.1, text.2, text.3);
        cr.move_to(2.0, y + 3.5);
        let _ = cr.show_text(&format!("{hrs:.0}h"));
    }
    let n = data.len().max(1) as f64;
    let slot = (w - left - 12.0) / n;
    let bar = (slot * 0.7).max(2.0);
    for (i, (night, day)) in data.iter().enumerate() {
        let x = left + slot * i as f64 + (slot - bar) / 2.0;
        let yn = y_of(*night as f64);
        cr.set_source_rgb(ink.0, ink.1, ink.2);
        cr.rectangle(x, yn, bar, y_of(0.0) - yn);
        let _ = cr.fill();
        if *day > 0 {
            let yd = y_of((*night + *day) as f64);
            cr.set_source_rgb(sun.0, sun.1, sun.2);
            cr.rectangle(x, yd, bar, yn - yd);
            let _ = cr.fill();
        }
        // Numărul zilei, din cinci în cinci.
        let d = i + 1;
        if d == 1 || d % 5 == 0 {
            cr.set_source_rgba(text.0, text.1, text.2, text.3);
            cr.move_to(x, h - 4.0);
            let _ = cr.show_text(&d.to_string());
        }
    }
}

/// O zi din calendarul de serviciu: numărul, ce a fost și orele suplimentare.
fn day_button(app: &Rc<App>, d: NaiveDate, kind: Option<Shift>, note: &str, ot: &[Overtime], today: bool) -> gtk::Button {
    let lang = app.lang;
    let (y, m, day) = (d.year(), d.month(), d.day());
    let b = gtk::Button::builder().css_classes(["zi-serviciu"]).build();
    if d.weekday().num_days_from_monday() >= 5 {
        b.add_css_class("weekend");
    }
    if today {
        b.add_css_class("azi");
    }
    let inside = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let has_note = !note.is_empty() || ot.iter().any(|o| !o.note.is_empty());
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    top.append(&gtk::Label::builder().label(day.to_string()).xalign(0.0).hexpand(true).css_classes(["nr"]).build());
    if has_note {
        top.append(&gtk::Label::builder().label("✎").css_classes(["are-nota"]).build());
    }
    inside.append(&top);
    let ot_min: u32 = ot.iter().map(|o| o.span.minutes()).sum();
    let tag = gtk::Label::builder().css_classes(["eticheta-zi"]).build();
    match kind {
        Some(Shift::Service) => {
            tag.set_label(tr(lang, "Serv.", "Duty"));
            b.add_css_class("tura");
        }
        Some(Shift::Reserve) => {
            tag.set_label(tr(lang, "Rez.", "Stby"));
            b.add_css_class("rezerva");
        }
        Some(Shift::Leave) => {
            tag.set_label(tr(lang, "CO", "Leave"));
            b.add_css_class("concediu");
        }
        None => {
            tag.set_label(" ");
            tag.add_css_class("goala");
        }
    }
    inside.append(&tag);
    // Orele suplimentare, dedesubt, ca să nu lățească ziua.
    let supl = gtk::Label::builder().css_classes(["supl"]).build();
    if ot_min > 0 {
        supl.set_label(&if ot_min.is_multiple_of(60) { format!("+{} h", ot_min / 60) } else { format!("+{}", duration(ot_min)) });
    } else {
        supl.set_label(" ");
    }
    inside.append(&supl);
    b.set_child(Some(&inside));
    let mut tip = lang.day_title(d);
    if !note.is_empty() {
        tip.push_str(&format!("\n{note}"));
    }
    for o in ot {
        tip.push_str(&format!("\n{} {}", tr(lang, "Ore suplimentare:", "Overtime:"), o.span.text()));
        if !o.note.is_empty() {
            tip.push_str(&format!(" ({})", o.note));
        }
    }
    b.set_tooltip_text(Some(&tip));

    // Meniul zilei: Tură, Concediu, Liber și orele suplimentare (de la, până la).
    let pop = gtk::Popover::new();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let set = {
        let weak = Rc::downgrade(app);
        move |k: Option<Shift>| {
            if let Some(app) = weak.upgrade() {
                app.change_tracking(y, m, Some(d), move |t| match k {
                    Some(k) => {
                        t.work.insert(day, k);
                    }
                    None => {
                        t.work.remove(&day);
                    }
                });
            }
        }
    };
    for (k, ro, en, key) in [
        (Some(Shift::Service), "Serviciu", "Duty", if lang == Lang::En { "D" } else { "S" }),
        (Some(Shift::Reserve), "Rezervă", "Standby", if lang == Lang::En { "S" } else { "R" }),
        (Some(Shift::Leave), "Concediu", "Leave", if lang == Lang::En { "L" } else { "C" }),
        (None, "Liber", "Free", "Del"),
    ] {
        let item = gtk::Button::builder().css_classes(["flat"]).build();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        row.append(&gtk::Label::builder().label(tr(lang, ro, en)).xalign(0.0).hexpand(true).build());
        row.append(&gtk::Label::builder().label(key).css_classes(["dim-label", "monospace"]).build());
        item.set_child(Some(&row));
        let (p, set) = (pop.clone(), set.clone());
        item.connect_clicked(move |_| {
            p.popdown();
            set(k);
        });
        menu.append(&item);
    }
    // Nota zilei (la serviciu, rezervă sau concediu).
    menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    menu.append(&gtk::Label::builder().label(tr(lang, "Notă", "Note")).xalign(0.0).css_classes(["heading", "titlu-meniu"]).build());
    let note_entry = gtk::Entry::builder()
        .text(note)
        .placeholder_text(if kind.is_some() { tr(lang, "o notă pentru ziua aceasta, apoi Enter", "a note for this day, then Enter") } else { tr(lang, "alege întâi Serviciu, Rezervă sau Concediu", "choose Duty, Standby or Leave first") })
        .sensitive(kind.is_some())
        .build();
    let (weak, p) = (Rc::downgrade(app), pop.clone());
    note_entry.connect_activate(move |e| {
        let text = e.text().trim().to_string();
        p.popdown();
        if let Some(app) = weak.upgrade() {
            app.change_tracking(y, m, Some(d), move |t| {
                if text.is_empty() { t.work_notes.remove(&day); } else { t.work_notes.insert(day, text); }
            });
        }
    });
    menu.append(&note_entry);
    menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    menu.append(&gtk::Label::builder().label(tr(lang, "Ore suplimentare", "Overtime")).xalign(0.0).css_classes(["heading", "titlu-meniu"]).build());
    for (i, o) in ot.iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let text = if o.note.is_empty() { o.span.text() } else { format!("{} · {}", o.span.text(), o.note) };
        row.append(&gtk::Label::builder().label(text).xalign(0.0).hexpand(true).wrap(true).max_width_chars(28).build());
        let del = gtk::Button::builder().icon_name("edit-delete-symbolic").css_classes(["flat", "circular"]).tooltip_text(tr(lang, "Șterge", "Delete")).build();
        let (weak, p) = (Rc::downgrade(app), pop.clone());
        del.connect_clicked(move |_| {
            p.popdown();
            if let Some(app) = weak.upgrade() {
                app.change_tracking(y, m, Some(d), move |t| {
                    if let Some(v) = t.overtime.get_mut(&day) {
                        if i < v.len() {
                            v.remove(i);
                        }
                        if v.is_empty() {
                            t.overtime.remove(&day);
                        }
                    }
                });
            }
        });
        row.append(&del);
        menu.append(&row);
    }
    let form = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let from = gtk::Entry::builder().placeholder_text("08:00").width_chars(5).max_width_chars(5).build();
    let to = gtk::Entry::builder().placeholder_text("12:00").width_chars(5).max_width_chars(5).build();
    form.append(&from);
    form.append(&gtk::Label::new(Some("-")));
    form.append(&to);
    let add = gtk::Button::builder().label(tr(lang, "Adaugă", "Add")).css_classes(["suggested-action"]).build();
    form.append(&add);
    menu.append(&form);
    let ot_note = gtk::Entry::builder().placeholder_text(tr(lang, "notă (opțional)", "note (optional)")).build();
    menu.append(&ot_note);
    let submit = {
        let (weak, p, from, to, on) = (Rc::downgrade(app), pop.clone(), from.clone(), to.clone(), ot_note.clone());
        move || {
            let note = on.text().trim().to_string();
            let Some(span) = Span::parse(&format!("{}-{}", from.text(), to.text())) else {
                from.add_css_class("error");
                to.add_css_class("error");
                return;
            };
            p.popdown();
            if let Some(app) = weak.upgrade() {
                app.change_tracking(y, m, Some(d), move |t| t.overtime.entry(day).or_default().push(Overtime { span, note }));
            }
        }
    };
    let s1 = submit.clone();
    add.connect_clicked(move |_| s1());
    let s2 = submit.clone();
    to.connect_activate(move |_| s2());
    let s3 = submit.clone();
    ot_note.connect_activate(move |_| s3());
    let t2 = to.clone();
    from.connect_activate(move |_| {
        t2.grab_focus();
    });
    pop.set_child(Some(&menu));
    pop.set_parent(&b);
    let p = pop.clone();
    b.connect_clicked(move |_| p.popup());
    let p = pop.clone();
    b.connect_destroy(move |_| p.unparent());

    // Tastele, pe ziua aleasă: T, C (L în engleză), O, Delete.
    let keys = gtk::EventControllerKey::new();
    let (p, f) = (pop.clone(), from.clone());
    keys.connect_key_pressed(move |_, key, _, mods| {
        if mods.intersects(gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::ALT_MASK) {
            return glib::Propagation::Proceed;
        }
        let en = lang == Lang::En;
        match key.to_unicode().map(|c| c.to_ascii_lowercase()) {
            Some('s') if !en => set(Some(Shift::Service)),
            Some('d') if en => set(Some(Shift::Service)),
            Some('r') if !en => set(Some(Shift::Reserve)),
            Some('s') if en => set(Some(Shift::Reserve)),
            Some('c') | Some('l') => set(Some(Shift::Leave)),
            Some('o') => {
                p.popup();
                f.grab_focus();
            }
            _ if matches!(key, gtk::gdk::Key::Delete | gtk::gdk::Key::BackSpace) => set(None),
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    });
    b.add_controller(keys);
    b
}

/// Pentru calendarul lunii: „▸ Serviciu”, „▸ Rezervă”, „▸ CO”, „▸ +4 h”.
pub fn work_short(lang: Lang, items: &[WorkOnDay]) -> Vec<String> {
    items
        .iter()
        .map(|it| {
            let (head, note) = match it {
                WorkOnDay::Day(Shift::Service, n) => (format!("▸ {}", tr(lang, "Serviciu", "Duty")), n),
                WorkOnDay::Day(Shift::Reserve, n) => (format!("▸ {}", tr(lang, "Rezervă", "Standby")), n),
                WorkOnDay::Day(Shift::Leave, n) => (format!("▸ {}", tr(lang, "Concediu", "Leave")), n),
                WorkOnDay::Overtime(span, n) => (format!("▸ +{} ({})", hours_text(span.minutes()), span.text()), n),
            };
            // Nota, direct în calendar, ca să se citească fără alt clic.
            if note.is_empty() { head } else { format!("{head}: {note}") }
        })
        .collect()
}
