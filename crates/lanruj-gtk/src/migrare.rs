// SPDX-License-Identifier: GPL-3.0-or-later
//! Ecranul de migrare: ziua triată în stânga, ziua următoare în dreapta,
//! iar sub sarcina curentă patru alegeri: Mâine, Programează, Renunț, O las.

use crate::{App, tr};
use adw::prelude::*;
use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};
use gtk::{gdk, glib};
use lanruj_core::migrate::candidates;
use lanruj_core::{Decision, Entry, Lang};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

/// Ora de la care migrarea de seară pornește singură (se va alege din setări).
pub const EVENING_HOUR: u32 = 21;

pub struct Migration {
    pub source: NaiveDate,
    pub target: NaiveDate,
    /// Pozițiile sarcinilor de triat, în ordinea din pagină.
    pub order: Vec<usize>,
    /// Sarcina curentă: un indice în `order`; `order.len()` = totul e triat.
    pub pos: usize,
    pub decided: HashMap<usize, Decision>,
    /// Semnele sarcinilor ajunse pe pagina din dreapta în această migrare.
    pub new_ids: Vec<String>,
    /// Ziua arătată înainte, ca să ne întoarcem la ea.
    pub prev_shown: NaiveDate,
}

impl Migration {
    pub fn current(&self) -> Option<usize> {
        self.order.get(self.pos).copied()
    }

    /// Trece la următoarea sarcină netriată (după cea curentă, apoi de la început).
    fn advance(&mut self) {
        let n = self.order.len();
        for step in 1..=n {
            let p = (self.pos + step) % n;
            if !self.decided.contains_key(&self.order[p]) {
                self.pos = p;
                return;
            }
        }
        self.pos = n;
    }
}

#[derive(Clone, Copy)]
pub enum Choice {
    Tomorrow,
    Schedule(NaiveDate),
    Drop,
    Keep,
}

/// Ce vede pagina în timpul migrării.
pub struct MigView<'a> {
    pub current: Option<usize>,
    pub candidates: &'a [usize],
    pub decided: &'a HashMap<usize, Decision>,
    pub panel: Option<gtk::Widget>,
    pub new_ids: &'a [String],
    pub is_source: bool,
}

fn state_file() -> PathBuf {
    // La probe (LANRUJ_DIR), starea stă lângă jurnalul de probă, nu în cea adevărată.
    if let Some(d) = std::env::var_os("LANRUJ_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join(".stare-migrare");
    }
    // XDG_STATE_HOME (în Flatpak, dosarul aplicației), altfel ~/.local/state.
    let base = std::env::var_os("XDG_STATE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| glib::home_dir().join(".local").join("state"));
    base.join("lanruj").join("migrare")
}

/// Ultima zi pentru care s-a încheiat migrarea (pe acest calculator).
fn last_done() -> Option<NaiveDate> {
    let s = std::fs::read_to_string(state_file()).ok()?;
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

fn set_last_done(d: NaiveDate) {
    let p = state_file();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if last_done().is_none_or(|old| d > old) {
        let _ = std::fs::write(p, d.format("%Y-%m-%d\n").to_string());
    }
}

fn plural_sarcina(lang: Lang, n: usize) -> String {
    match lang {
        Lang::Ro => match n {
            1 => "o sarcină".into(),
            n if n % 100 >= 20 || n % 100 == 0 && n > 0 => format!("{n} de sarcini"),
            n => format!("{n} sarcini"),
        },
        Lang::En => if n == 1 { "1 task".into() } else { format!("{n} tasks") },
    }
}

impl App {
    pub fn migrating(&self) -> bool {
        self.mig.borrow().is_some()
    }

    /// Pornește migrarea zilei `source`. `auto` = pornită singură seara.
    pub fn start_migration(self: &Rc<Self>, source: NaiveDate, auto: bool) {
        if self.migrating() {
            return;
        }
        let day = self.load(source);
        let order = candidates(&day);
        if order.is_empty() {
            if !auto {
                self.toast(tr(self.lang, "Nicio sarcină neterminată în ziua aceasta.", "No unfinished tasks on this day."));
            }
            return;
        }
        *self.mig.borrow_mut() = Some(Migration {
            source,
            target: source + Duration::days(1),
            order,
            pos: 0,
            decided: HashMap::new(),
            new_ids: Vec::new(),
            prev_shown: self.shown.get(),
        });
        gtk::prelude::GtkWindowExt::set_focus(&self.window, None::<&gtk::Widget>);
        self.refresh();
    }

    /// „Gata” (`done`) sau „Mai târziu”. Ce s-a ales rămâne ales.
    pub fn end_migration(self: &Rc<Self>, done: bool) {
        let Some(m) = self.mig.borrow_mut().take() else { return };
        if done {
            set_last_done(m.source);
        } else {
            self.dismissed.set(Some(m.source));
        }
        if done && !m.decided.is_empty() {
            let msg = match self.lang {
                Lang::Ro => format!("Migrarea s-a încheiat: {} triate.", plural_sarcina(self.lang, m.decided.len())),
                Lang::En => format!("Migration finished: {} sorted.", plural_sarcina(self.lang, m.decided.len())),
            };
            self.toast(&msg);
        }
        self.shown.set(m.prev_shown);
        self.refresh();
        if let Some(q) = &self.right.quick {
            q.grab_focus();
        }
    }

    pub fn mig_select(self: &Rc<Self>, index: usize) {
        if let Some(m) = self.mig.borrow_mut().as_mut()
            && let Some(p) = m.order.iter().position(|&i| i == index)
        {
            m.pos = p;
        }
        self.refresh();
    }

    pub fn mig_move(self: &Rc<Self>, delta: i32) {
        if let Some(m) = self.mig.borrow_mut().as_mut() {
            let n = m.order.len() as i32;
            let cur = (m.pos as i32).min(n);
            m.pos = (cur + delta).clamp(0, n - 1) as usize;
        }
        self.refresh();
    }

    pub fn mig_decide(self: &Rc<Self>, choice: Choice) {
        let (source, target, index, previous) = {
            let m = self.mig.borrow();
            let Some(m) = m.as_ref() else { return };
            let Some(index) = m.current() else { return };
            (m.source, m.target, index, m.decided.get(&index).copied())
        };
        let lang = self.lang;
        // O alegere schimbată: întâi o anulăm pe cea veche.
        if previous.is_some()
            && let Err(e) = self.journal.reopen(source, index, lang)
        {
            self.error(&e);
            return;
        }
        let result = match choice {
            Choice::Tomorrow => self.journal.migrate(source, index, target, lang).map(|id| (Decision::Moved(target), Some(id))),
            Choice::Schedule(d) => self.journal.migrate(source, index, d, lang).map(|id| (Decision::Moved(d), Some(id))),
            Choice::Drop => self.journal.drop_task(source, index, lang).map(|_| (Decision::Dropped, None)),
            Choice::Keep => Ok((Decision::Kept, None)),
        };
        match result {
            Ok((decision, id)) => {
                if let Some(m) = self.mig.borrow_mut().as_mut() {
                    if let (Decision::Moved(d), Some(id)) = (decision, &id)
                        && d == target
                        && !m.new_ids.contains(id)
                    {
                        m.new_ids.push(id.clone());
                    }
                    m.decided.insert(index, decision);
                    m.advance();
                }
            }
            Err(e) => {
                // Dacă anularea a mers, dar noua alegere nu, sarcina rămâne neterminată.
                if let Some(m) = self.mig.borrow_mut().as_mut() {
                    m.decided.remove(&index);
                }
                self.error(&e);
            }
        }
        self.refresh();
    }

    /// Seara, la ora aleasă (sau la prima deschidere după ea), migrarea pornește singură.
    pub fn auto_migration(self: &Rc<Self>) {
        if self.migrating() {
            return;
        }
        let now = Local::now();
        let today = now.date_naive();
        let pending = if now.hour() >= EVENING_HOUR { today } else { today - Duration::days(1) };
        if last_done().is_some_and(|d| d >= pending) || self.dismissed.get() == Some(pending) {
            return;
        }
        let day = self.load(pending);
        if candidates(&day).is_empty() {
            set_last_done(pending);
            return;
        }
        self.start_migration(pending, true);
    }

    /// Antetul, titlul și progresul în timpul migrării.
    pub fn refresh_migration_header(&self) {
        let m = self.mig.borrow();
        let on = m.is_some();
        for w in &self.normal_header {
            w.set_visible(!on);
        }
        for w in &self.mig_header {
            w.set_visible(on);
        }
        let Some(m) = m.as_ref() else { return };
        let lang = self.lang;
        let evening = m.source == Local::now().date_naive();
        self.title.set_title(if evening { tr(lang, "Migrarea de seară", "Evening migration") } else { tr(lang, "Migrarea", "Migration") });
        let n = m.order.len();
        let done = m.decided.len();
        let sub = if m.pos < n {
            match lang {
                Lang::Ro => format!("sarcina {} din {} · {}", m.pos + 1, n, lang.day_title_short(m.source)),
                Lang::En => format!("task {} of {} · {}", m.pos + 1, n, lang.day_title_short(m.source)),
            }
        } else {
            format!("{} · {}", tr(lang, "toate triate", "all sorted"), lang.day_title_short(m.source))
        };
        self.title.set_subtitle(&sub);

        while let Some(c) = self.progress.first_child() {
            self.progress.remove(&c);
        }
        if n <= 12 {
            for (p, idx) in m.order.iter().enumerate() {
                let b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                b.add_css_class("progres-pas");
                if m.decided.contains_key(idx) {
                    b.add_css_class("gata");
                }
                if p == m.pos {
                    b.add_css_class("acum");
                }
                b.set_valign(gtk::Align::Center);
                self.progress.append(&b);
            }
        } else {
            self.progress.append(&gtk::Label::builder().label(format!("{done}/{n}")).css_classes(["dim-label"]).build());
        }
        if done == n {
            self.done_button.add_css_class("suggested-action");
        } else {
            self.done_button.remove_css_class("suggested-action");
        }
    }

    /// Panoul cu alegeri, pus sub sarcina curentă.
    pub fn build_panel(self: &Rc<Self>, entry: &Entry, previous: Option<Decision>) -> gtk::Widget {
        let lang = self.lang;
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 10);
        panel.add_css_class("alegere");
        panel.set_margin_start(26);
        panel.set_margin_top(6);
        panel.set_margin_bottom(10);

        let moved = entry.id.as_deref().map(|id| self.journal.migration_count(id)).unwrap_or(0);
        if moved >= 3 {
            let q = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            q.add_css_class("intrebare");
            let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
            icon.set_valign(gtk::Align::Start);
            q.append(&icon);
            let text = match lang {
                Lang::Ro => format!(
                    "<b>Mai merită?</b>\nA fost mutată de {moved} ori. Poate e mai bine să o programezi pe o zi anume sau să renunți la ea."
                ),
                Lang::En => format!("<b>Still worth it?</b>\nMoved {moved} times. Maybe schedule it for a specific day or let it go."),
            };
            q.append(&gtk::Label::builder().label(text).use_markup(true).wrap(true).xalign(0.0).hexpand(true).build());
            panel.append(&q);
        }
        if let Some(prev) = previous {
            let what = match prev {
                Decision::Moved(d) => match lang {
                    Lang::Ro => format!("Ai ales: mutată pe {} {}.", d.day(), lang.month(d.month())),
                    Lang::En => format!("You chose: moved to {} {}.", d.day(), lang.month(d.month())),
                },
                Decision::Dropped => tr(lang, "Ai ales: renunț.", "You chose: drop.").into(),
                Decision::Kept => tr(lang, "Ai ales: o las.", "You chose: keep.").into(),
            };
            let hint = tr(lang, " Alege din nou ca s-o schimbi.", " Choose again to change it.");
            panel.append(&gtk::Label::builder().label(format!("{what}{hint}")).wrap(true).xalign(0.0).css_classes(["ales"]).build());
        }

        // Două coloane când e loc, una singură pe pagină îngustă.
        let grid = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .min_children_per_line(1)
            .max_children_per_line(2)
            .column_spacing(8)
            .row_spacing(8)
            .build();
        let option = |sign: &str, title: &str, small: &str, key: &str| {
            let b = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            b.append(&gtk::Label::builder().label(sign).css_classes(["s"]).build());
            let t = gtk::Box::new(gtk::Orientation::Vertical, 0);
            t.set_hexpand(true);
            t.append(&gtk::Label::builder().label(title).xalign(0.0).build());
            t.append(&gtk::Label::builder().label(small).xalign(0.0).wrap(true).css_classes(["mic"]).build());
            b.append(&t);
            b.append(&gtk::Label::builder().label(key).css_classes(["tasta"]).valign(gtk::Align::Center).build());
            b
        };
        let target = self.mig.borrow().as_ref().map(|m| m.target).expect("în migrare");
        let source = target - Duration::days(1);
        // Dimineața (migrarea zilei de ieri), ziua din dreapta e azi.
        let mut target_name = self.relative(target);
        if (target - self.today.get()).num_days().abs() > 1 {
            target_name = tr(lang, "Ziua următoare", "Next day").into();
        }
        let tomorrow_small = match lang {
            Lang::Ro => format!("pe {} {}", target.day(), lang.month(target.month())),
            Lang::En => format!("to {} {}", target.day(), lang.month(target.month())),
        };

        let b_tomorrow = gtk::Button::builder()
            .child(&option(">", &target_name, &tomorrow_small, "M"))
            .css_classes(["optiune", "opt-maine"])
            .build();
        let weak = Rc::downgrade(self);
        b_tomorrow.connect_clicked(move |_| {
            if let Some(s) = weak.upgrade() {
                s.mig_decide(Choice::Tomorrow);
            }
        });

        let b_sched = gtk::MenuButton::builder()
            .child(&option("<", tr(lang, "Programează", "Schedule"), tr(lang, "pe altă zi", "on another day"), "P"))
            .css_classes(["optiune", "opt-prog"])
            .build();
        let pop = gtk::Popover::new();
        let pbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
        pbox.append(&gtk::Label::builder().label(tr(lang, "Alege ziua", "Pick the day")).css_classes(["heading"]).build());
        let cal = gtk::Calendar::new();
        let week = source + Duration::days(7);
        if let Ok(dt) = glib::DateTime::from_local(week.year(), week.month() as i32, week.day() as i32, 12, 0, 0.0) {
            cal.select_day(&dt);
        }
        pbox.append(&cal);
        let pick = gtk::Button::builder().label(tr(lang, "Programează", "Schedule")).css_classes(["suggested-action"]).build();
        pbox.append(&pick);
        pop.set_child(Some(&pbox));
        b_sched.set_popover(Some(&pop));
        let (weak, pop_c, cal_c) = (Rc::downgrade(self), pop.clone(), cal.clone());
        pick.connect_clicked(move |_| {
            let dt = cal_c.date();
            let Some(d) = NaiveDate::from_ymd_opt(dt.year(), dt.month() as u32, dt.day_of_month() as u32) else { return };
            let Some(s) = weak.upgrade() else { return };
            if d <= source {
                s.toast(tr(s.lang, "Alege o zi de după ziua sarcinii.", "Pick a day after the task's day."));
                return;
            }
            pop_c.popdown();
            s.mig_decide(if d == source + Duration::days(1) { Choice::Tomorrow } else { Choice::Schedule(d) });
        });
        *self.sched_button.borrow_mut() = Some(b_sched.clone());

        let b_drop = gtk::Button::builder()
            .child(&option("•", tr(lang, "Renunț", "Drop"), tr(lang, "rămâne tăiată", "stays crossed out"), "R"))
            .css_classes(["optiune", "opt-renunt"])
            .build();
        let weak = Rc::downgrade(self);
        b_drop.connect_clicked(move |_| {
            if let Some(s) = weak.upgrade() {
                s.mig_decide(Choice::Drop);
            }
        });
        let b_keep = gtk::Button::builder()
            .child(&option("•", tr(lang, "O las", "Keep"), tr(lang, "neterminată, fără mutare", "unfinished, not moved"), "L"))
            .css_classes(["optiune", "opt-las"])
            .build();
        let weak = Rc::downgrade(self);
        b_keep.connect_clicked(move |_| {
            if let Some(s) = weak.upgrade() {
                s.mig_decide(Choice::Keep);
            }
        });
        if moved >= 3 { b_sched.add_css_class("rec") } else { b_tomorrow.add_css_class("rec") }
        for b in [b_tomorrow.upcast_ref::<gtk::Widget>(), b_sched.upcast_ref(), b_drop.upcast_ref(), b_keep.upcast_ref()] {
            grid.append(b);
        }
        panel.append(&grid);

        let hint = match lang {
            Lang::Ro => format!("Enter = {} · Esc = mai târziu · ↑ ↓ alte sarcini", target_name.to_lowercase()),
            Lang::En => format!("Enter = {} · Esc = later · ↑ ↓ other tasks", target_name.to_lowercase()),
        };
        panel.append(&gtk::Label::builder().label(hint).xalign(0.0).wrap(true).css_classes(["mic"]).build());
        panel.upcast()
    }

    /// Panoul de la sfârșit, când toate sarcinile au fost triate.
    pub fn build_done_panel(self: &Rc<Self>) -> gtk::Widget {
        let lang = self.lang;
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        panel.add_css_class("alegere");
        panel.set_margin_top(10);
        panel.append(
            &gtk::Label::builder()
                .label(tr(lang, "Toate sarcinile au fost triate.", "All tasks are sorted."))
                .xalign(0.0)
                .css_classes(["heading"])
                .build(),
        );
        panel.append(
            &gtk::Label::builder()
                .label(tr(lang, "Apasă Gata, sau alege o sarcină ca să schimbi ce ai ales.", "Press Done, or pick a task to change your choice."))
                .xalign(0.0)
                .wrap(true)
                .css_classes(["mic"])
                .build(),
        );
        panel.upcast()
    }

    /// Tastele din timpul migrării. Când scrii în câmpul de jos, tastele sunt ale lui.
    pub fn migration_key(self: &Rc<Self>, key: gdk::Key, mods: gdk::ModifierType) -> glib::Propagation {
        if !self.migrating() || mods.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
            return glib::Propagation::Proceed;
        }
        if gtk::prelude::GtkWindowExt::focus(&self.window).is_some_and(|w| w.is::<gtk::Text>()) {
            if key == gdk::Key::Escape {
                gtk::prelude::GtkWindowExt::set_focus(&self.window, None::<&gtk::Widget>);
                return glib::Propagation::Stop;
            }
            return glib::Propagation::Proceed;
        }
        let all_done = self.mig.borrow().as_ref().is_some_and(|m| m.current().is_none());
        match key {
            gdk::Key::Return | gdk::Key::KP_Enter if all_done => self.end_migration(true),
            gdk::Key::m | gdk::Key::M | gdk::Key::Return | gdk::Key::KP_Enter => self.mig_decide(Choice::Tomorrow),
            gdk::Key::p | gdk::Key::P => {
                if let Some(b) = self.sched_button.borrow().as_ref() {
                    b.popup();
                }
            }
            gdk::Key::r | gdk::Key::R => self.mig_decide(Choice::Drop),
            gdk::Key::l | gdk::Key::L => self.mig_decide(Choice::Keep),
            gdk::Key::Escape => self.end_migration(false),
            gdk::Key::Up => self.mig_move(-1),
            gdk::Key::Down => self.mig_move(1),
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    }
}
