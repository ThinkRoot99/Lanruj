// SPDX-License-Identifier: GPL-3.0-or-later
//! Ecranul de migrare: pagina triată în stânga (o zi sau lista unei luni),
//! pagina următoare în dreapta, iar sub sarcina curentă patru alegeri:
//! Mâine / Luna următoare, Programează, Renunț, O las.

use crate::{App, View, tr};
use adw::prelude::*;
use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};
use gtk::{gdk, glib};
use lanruj_core::migrate::{candidates, month_of, next_page};
use lanruj_core::{Decision, Entry, Lang, PageRef};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

pub struct Migration {
    pub source: PageRef,
    pub target: PageRef,
    /// Pozițiile sarcinilor de triat, în ordinea din pagină.
    pub order: Vec<usize>,
    /// Sarcina curentă: un indice în `order`; `order.len()` = totul e triat.
    pub pos: usize,
    pub decided: HashMap<usize, Decision>,
    /// Semnele sarcinilor ajunse pe pagina din dreapta în această migrare.
    pub new_ids: Vec<String>,
    /// Ziua și ecranul de dinainte, ca să ne întoarcem la ele.
    pub prev_shown: NaiveDate,
    pub prev_view: View,
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
    /// Pagina următoare: mâine sau luna următoare.
    Tomorrow,
    Schedule(PageRef),
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
    state_path("migrare")
}

fn month_state_file() -> PathBuf {
    state_path("migrare-luna")
}

fn state_path(name: &str) -> PathBuf {
    // La probe (LANRUJ_DIR), starea stă lângă jurnalul de probă, nu în cea adevărată.
    if let Some(d) = std::env::var_os("LANRUJ_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join(format!(".stare-{name}"));
    }
    // XDG_STATE_HOME (în Flatpak, dosarul aplicației), altfel ~/.local/state.
    let base = std::env::var_os("XDG_STATE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| glib::home_dir().join(".local").join("state"));
    base.join("lanruj").join(name)
}

/// Ultima lună pentru care s-a încheiat migrarea de sfârșit de lună: (an, lună).
fn last_month_done() -> Option<(i32, u32)> {
    let s = std::fs::read_to_string(month_state_file()).ok()?;
    let (y, m) = s.trim().split_once('-')?;
    Some((y.parse().ok()?, m.parse().ok()?))
}

fn set_last_month_done(y: i32, m: u32) {
    let p = month_state_file();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if last_month_done().is_none_or(|old| (y, m) > old) {
        let _ = std::fs::write(p, format!("{y:04}-{m:02}\n"));
    }
}

/// Numele scurt al unei pagini: „3 decembrie” sau „lista lui decembrie”.
pub fn page_short(lang: Lang, r: PageRef) -> String {
    match r {
        PageRef::Day(d) => format!("{} {}", d.day(), lang.month(d.month())),
        PageRef::Month { month, .. } => match lang {
            Lang::Ro => format!("lista lunii {}", lang.month(month)),
            Lang::En => format!("{} list", lang.month(month)),
        },
        PageRef::Future { year, month } => match lang {
            Lang::Ro => format!("planuri, {}", lang.month_title(year, month).to_lowercase()),
            Lang::En => format!("future log, {}", lang.month_title(year, month)),
        },
        PageRef::Week { week, .. } => match lang {
            Lang::Ro => format!("planul săptămânii {week}"),
            Lang::En => format!("week {week} plan"),
        },
        PageRef::Topic { .. } => tr(lang, "o pagină din index", "an index page").to_string(),
    }
}

/// Ultima săptămână pentru care s-a încheiat migrarea: (an ISO, săptămână).
fn last_week_done() -> Option<(i32, u32)> {
    let s = std::fs::read_to_string(state_path("migrare-saptamana")).ok()?;
    let (y, w) = s.trim().split_once("-S")?;
    Some((y.parse().ok()?, w.parse().ok()?))
}

fn set_last_week_done(y: i32, w: u32) {
    let p = state_path("migrare-saptamana");
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if last_week_done().is_none_or(|old| (y, w) > old) {
        let _ = std::fs::write(p, format!("{y:04}-S{w:02}\n"));
    }
}

/// Ultima lună pentru care jurnalul viitor a coborât în pagina lunii.
fn last_future_done() -> Option<(i32, u32)> {
    let s = std::fs::read_to_string(state_path("viitor")).ok()?;
    let (y, m) = s.trim().split_once('-')?;
    Some((y.parse().ok()?, m.parse().ok()?))
}

fn set_last_future_done(y: i32, m: u32) {
    let p = state_path("viitor");
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, format!("{y:04}-{m:02}\n"));
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

/// Săptămâna dinaintea celei de azi.
fn next_page_back_week(today: NaiveDate) -> PageRef {
    lanruj_core::week_of(today - Duration::days(7))
}

fn plural_intrare(n: usize) -> String {
    match n {
        1 => "o intrare".into(),
        n if n % 100 >= 20 || n % 100 == 0 => format!("{n} de intrări"),
        n => format!("{n} intrări"),
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

    /// Pornește migrarea paginii `source` (o zi sau o lună). `auto` = pornită singură.
    pub fn start_migration(self: &Rc<Self>, source: PageRef, auto: bool) {
        if self.migrating() {
            return;
        }
        let day = match self.journal.load_ref(source) {
            Ok(d) => d,
            Err(e) => return self.error(&e),
        };
        // Planurile și paginile din index nu se migrează.
        if let PageRef::Future { .. } | PageRef::Topic { .. } = source {
            return;
        }
        let order = candidates(&day);
        if order.is_empty() {
            if !auto {
                self.toast(match source {
                    PageRef::Day(_) | PageRef::Topic { .. } => tr(self.lang, "Nicio sarcină neterminată în ziua aceasta.", "No unfinished tasks on this day."),
                    PageRef::Month { .. } | PageRef::Future { .. } => tr(self.lang, "Nicio sarcină neterminată în lista lunii.", "No unfinished tasks in the month's list."),
                    PageRef::Week { .. } => tr(self.lang, "Nicio sarcină neterminată în planul săptămânii.", "No unfinished tasks in the week's plan."),
                });
            }
            return;
        }
        *self.mig.borrow_mut() = Some(Migration {
            source,
            target: next_page(source),
            order,
            pos: 0,
            decided: HashMap::new(),
            new_ids: Vec::new(),
            prev_shown: self.shown.get(),
            prev_view: self.view.get(),
        });
        self.editing.set(None);
        self.cal_editing.set(None);
        gtk::prelude::GtkWindowExt::set_focus(&self.window, None::<&gtk::Widget>);
        self.refresh();
    }

    /// „Gata” (`done`) sau „Mai târziu”. Ce s-a ales rămâne ales.
    pub fn end_migration(self: &Rc<Self>, done: bool) {
        let Some(m) = self.mig.borrow_mut().take() else { return };
        if done {
            match m.source {
                PageRef::Day(d) => set_last_done(d),
                PageRef::Month { year, month } => set_last_month_done(year, month),
                PageRef::Week { year, week } => set_last_week_done(year, week),
                PageRef::Future { .. } | PageRef::Topic { .. } => {}
            }
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
        self.view.set(m.prev_view);
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

    /// Pornește singură: seara, la ora aleasă (sau la prima deschidere după ea), migrarea zilei;
    /// apoi, de la 1 ale lunii, migrarea lunii trecute.
    pub fn auto_migration(self: &Rc<Self>) {
        if self.migrating() {
            return;
        }
        let now = Local::now();
        let today = now.date_naive();
        self.transfer_future_if_due(today);
        let (auto, hour) = {
            let st = self.settings.borrow();
            (st.auto_migrate, st.evening_hour)
        };
        if !auto {
            return;
        }
        let pending = if now.hour() >= hour { today } else { today - Duration::days(1) };
        let day_done = last_done().is_some_and(|d| d >= pending);
        if !day_done && self.dismissed.get() != Some(PageRef::Day(pending)) {
            if candidates(&self.load(pending)).is_empty() {
                set_last_done(pending);
            } else {
                return self.start_migration(PageRef::Day(pending), true);
            }
        }
        // Luna trecută
        let first = today.with_day(1).expect("zi validă");
        let prev = first - Duration::days(1);
        let (y, m) = (prev.year(), prev.month());
        let r = PageRef::Month { year: y, month: m };
        if !(last_month_done().is_some_and(|d| d >= (y, m)) || self.dismissed.get() == Some(r)) {
            match self.journal.load_ref(r) {
                Ok(d) if !candidates(&d).is_empty() => return self.start_migration(r, true),
                _ => set_last_month_done(y, m),
            }
        }
        // Săptămâna trecută, de luni încolo
        let PageRef::Week { year: wy, week: ww } = next_page_back_week(today) else { return };
        let wr = PageRef::Week { year: wy, week: ww };
        if last_week_done().is_some_and(|d| d >= (wy, ww)) || self.dismissed.get() == Some(wr) {
            return;
        }
        match self.journal.load_ref(wr) {
            Ok(d) if !candidates(&d).is_empty() => self.start_migration(wr, true),
            _ => set_last_week_done(wy, ww),
        }
    }

    /// Pe 1 ale lunii (sau la prima deschidere după): jurnalul viitor coboară în luna aceasta,
    /// și în lunile sărite, dacă aplicația n-a fost deschisă de mult.
    fn transfer_future_if_due(self: &Rc<Self>, today: NaiveDate) {
        let cur = (today.year(), today.month());
        let mut next = match last_future_done() {
            Some(last) if last >= cur => return,
            Some(last) => lanruj_core::migrate::next_month(last.0, last.1),
            None => cur,
        };
        let mut moved = 0;
        loop {
            match self.journal.transfer_future(next.0, next.1, self.lang) {
                Ok(n) => moved += n,
                Err(e) => return self.error(&e),
            }
            set_last_future_done(next.0, next.1);
            if next >= cur {
                break;
            }
            next = lanruj_core::migrate::next_month(next.0, next.1);
        }
        if moved > 0 {
            let msg = match self.lang {
                Lang::Ro => format!("Din planuri au trecut în luna aceasta {}.", plural_intrare(moved)),
                Lang::En => format!("{moved} entries moved from the future log into this month."),
            };
            self.toast(&msg);
            self.refresh();
        }
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
        let title = match m.source {
            PageRef::Day(d) if d == Local::now().date_naive() => tr(lang, "Migrarea de seară", "Evening migration"),
            PageRef::Day(_) | PageRef::Topic { .. } => tr(lang, "Migrarea", "Migration"),
            PageRef::Month { .. } | PageRef::Future { .. } => tr(lang, "Migrarea lunii", "Monthly migration"),
            PageRef::Week { .. } => tr(lang, "Migrarea săptămânii", "Weekly migration"),
        };
        self.title.set_title(title);
        let src_name = match m.source {
            PageRef::Day(d) | PageRef::Topic { date: d, .. } => lang.day_title_short(d),
            PageRef::Month { year, month } | PageRef::Future { year, month } => lang.month_title(year, month),
            PageRef::Week { week, .. } => lang.week_title(week),
        };
        let n = m.order.len();
        let done = m.decided.len();
        let sub = if m.pos < n {
            match lang {
                Lang::Ro => format!("sarcina {} din {} · {}", m.pos + 1, n, src_name),
                Lang::En => format!("task {} of {} · {}", m.pos + 1, n, src_name),
            }
        } else {
            format!("{} · {}", tr(lang, "toate triate", "all sorted"), src_name)
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
                Decision::Moved(r) => match lang {
                    Lang::Ro => format!("Ai ales: mutată {} {}.", if matches!(r, PageRef::Day(_)) { "pe" } else { "în" }, page_short(lang, r)),
                    Lang::En => format!("You chose: moved to {}.", page_short(lang, r)),
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
        let (source, target) = self.mig.borrow().as_ref().map(|m| (m.source, m.target)).expect("în migrare");
        let (target_name, tomorrow_small) = match target {
            PageRef::Day(t) | PageRef::Topic { date: t, .. } => {
                // Dimineața (migrarea zilei de ieri), ziua din dreapta e azi.
                let mut name = self.relative(t);
                if (t - self.today.get()).num_days().abs() > 1 {
                    name = tr(lang, "Ziua următoare", "Next day").into();
                }
                let small = match lang {
                    Lang::Ro => format!("pe {} {}", t.day(), lang.month(t.month())),
                    Lang::En => format!("to {} {}", t.day(), lang.month(t.month())),
                };
                (name, small)
            }
            PageRef::Month { year, month } | PageRef::Future { year, month } => (
                tr(lang, "Luna următoare", "Next month").to_string(),
                match lang {
                    Lang::Ro => format!("în lista lunii {}", lang.month(month)),
                    Lang::En => format!("to the {} list", lang.month_title(year, month)),
                },
            ),
            PageRef::Week { year, week } => (
                tr(lang, "Săptămâna următoare", "Next week").to_string(),
                lang.week_range(lanruj_core::week_monday(year, week)),
            ),
        };

        // Prima zi pe care se poate programa și ziua propusă în calendar.
        let (min_day, suggested) = match source {
            PageRef::Day(d) | PageRef::Topic { date: d, .. } => (d + Duration::days(1), d + Duration::days(7)),
            PageRef::Month { year, month } | PageRef::Future { year, month } => {
                let first = NaiveDate::from_ymd_opt(year, month, 1).expect("lună validă");
                let next = match next_page(source) {
                    PageRef::Month { year, month } | PageRef::Future { year, month } => NaiveDate::from_ymd_opt(year, month, 1).expect("lună validă"),
                    PageRef::Week { year, week } => lanruj_core::week_monday(year, week),
                    PageRef::Day(d) | PageRef::Topic { date: d, .. } => d,
                };
                (first, next)
            }
            PageRef::Week { year, week } => {
                let monday = lanruj_core::week_monday(year, week);
                (monday, monday + Duration::days(7))
            }
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
            .child(&option(
                "<",
                tr(lang, "Programează", "Schedule"),
                match source {
                    PageRef::Day(_) | PageRef::Topic { .. } => tr(lang, "pe altă zi sau în lista lunii", "another day or a month's list"),
                    PageRef::Month { .. } | PageRef::Future { .. } => tr(lang, "pe o zi anume", "on a specific day"),
                    PageRef::Week { .. } => tr(lang, "pe o zi sau în lista lunii", "a day or a month's list"),
                },
                "P",
            ))
            .css_classes(["optiune", "opt-prog"])
            .build();
        let pop = gtk::Popover::new();
        let pbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
        // Din migrarea unei zile: și în lista lunii (aceasta sau următoarea), ca în caiet.
        if let PageRef::Day(_) | PageRef::Week { .. } = source {
            pbox.append(&gtk::Label::builder().label(tr(lang, "În lista unei luni", "To a month's list")).css_classes(["heading"]).xalign(0.0).build());
            let this_month = month_of(source);
            let next_month = next_page(this_month);
            for (r, label) in [
                (this_month, tr(lang, "Luna aceasta", "This month")),
                (next_month, tr(lang, "Luna viitoare", "Next month")),
            ] {
                let PageRef::Month { month, .. } = r else { continue };
                let b = gtk::Button::builder()
                    .label(format!("{label} ({})", lang.month(month)))
                    .css_classes(["flat", "prog-luna"])
                    .build();
                let (weak, pop_c) = (Rc::downgrade(self), pop.clone());
                b.connect_clicked(move |_| {
                    pop_c.popdown();
                    if let Some(s) = weak.upgrade() {
                        s.mig_decide(Choice::Schedule(r));
                    }
                });
                pbox.append(&b);
            }
            pbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        // Din migrarea unei zile: și în planul unei săptămâni.
        if let PageRef::Day(d) = source {
            pbox.append(&gtk::Label::builder().label(tr(lang, "În planul unei săptămâni", "To a week's plan")).css_classes(["heading"]).xalign(0.0).build());
            let this_week = lanruj_core::week_of(d);
            for (r, label) in [
                (this_week, tr(lang, "Săptămâna aceasta", "This week")),
                (next_page(this_week), tr(lang, "Săptămâna viitoare", "Next week")),
            ] {
                let b = gtk::Button::builder().label(label).css_classes(["flat", "prog-sapt"]).build();
                let (weak, pop_c) = (Rc::downgrade(self), pop.clone());
                b.connect_clicked(move |_| {
                    pop_c.popdown();
                    if let Some(s) = weak.upgrade() {
                        s.mig_decide(Choice::Schedule(r));
                    }
                });
                pbox.append(&b);
            }
            pbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        // În planuri: lunile de după luna viitoare (până la 6 luni de acum).
        {
            let PageRef::Month { year, month } = month_of(source) else { unreachable!() };
            let mut months = Vec::new();
            let (mut y, mut m) = lanruj_core::migrate::next_month(year, month);
            for _ in 0..5 {
                (y, m) = lanruj_core::migrate::next_month(y, m);
                months.push((y, m));
            }
            let names: Vec<String> = months.iter().map(|(y, m)| lang.month_title(*y, *m)).collect();
            let names_ref: Vec<&str> = names.iter().map(String::as_str).collect();
            pbox.append(&gtk::Label::builder().label(tr(lang, "În planuri", "To the future log")).css_classes(["heading"]).xalign(0.0).build());
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let dd = gtk::DropDown::from_strings(&names_ref);
            dd.set_hexpand(true);
            let put = gtk::Button::builder().label(tr(lang, "Pune", "Put")).css_classes(["prog-viitor"]).build();
            let (weak, pop_c, dd_c) = (Rc::downgrade(self), pop.clone(), dd.clone());
            put.connect_clicked(move |_| {
                let Some(&(year, month)) = months.get(dd_c.selected() as usize) else { return };
                pop_c.popdown();
                if let Some(s) = weak.upgrade() {
                    s.mig_decide(Choice::Schedule(PageRef::Future { year, month }));
                }
            });
            row.append(&dd);
            row.append(&put);
            pbox.append(&row);
            pbox.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        }
        pbox.append(&gtk::Label::builder().label(tr(lang, "Pe o zi anume", "On a specific day")).css_classes(["heading"]).xalign(0.0).build());
        let cal = gtk::Calendar::new();
        if let Ok(dt) = glib::DateTime::from_local(suggested.year(), suggested.month() as i32, suggested.day() as i32, 12, 0, 0.0) {
            cal.select_day(&dt);
        }
        pbox.append(&cal);
        let pick = gtk::Button::builder().label(tr(lang, "Programează pe ziua aleasă", "Schedule on the chosen day")).css_classes(["suggested-action"]).build();
        pbox.append(&pick);
        pop.set_child(Some(&pbox));
        b_sched.set_popover(Some(&pop));
        let (weak, pop_c, cal_c) = (Rc::downgrade(self), pop.clone(), cal.clone());
        pick.connect_clicked(move |_| {
            let dt = cal_c.date();
            let Some(d) = NaiveDate::from_ymd_opt(dt.year(), dt.month() as u32, dt.day_of_month() as u32) else { return };
            let Some(s) = weak.upgrade() else { return };
            if d < min_day {
                s.toast(match source {
                    PageRef::Day(_) | PageRef::Topic { .. } => tr(s.lang, "Alege o zi de după ziua sarcinii.", "Pick a day after the task's day."),
                    PageRef::Month { .. } | PageRef::Future { .. } => tr(s.lang, "Alege o zi din luna aceasta sau de după.", "Pick a day in this month or later."),
                    PageRef::Week { .. } => tr(s.lang, "Alege o zi din săptămâna aceasta sau de după.", "Pick a day in this week or later."),
                });
                return;
            }
            pop_c.popdown();
            let r = PageRef::Day(d);
            s.mig_decide(if r == target { Choice::Tomorrow } else { Choice::Schedule(r) });
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
