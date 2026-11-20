// SPDX-License-Identifier: GPL-3.0-or-later
//! Lanruj: jurnal în stilul Bullet Journal, pe fișiere Markdown.

mod bara;
mod cautare;
mod migrare;
mod page;
mod setari;
mod tava;

use adw::prelude::*;
use chrono::{Datelike, Duration, Local, NaiveDate};
use gtk::{gdk, gio, glib};
#[cfg(debug_assertions)]
use gtk::glib::translate::IntoGlib;
use lanruj_core::{Day, Journal, Lang, PageRef, parse_quick};
use page::{Page, Side};
use migrare::{MigView, Migration};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub const APP_ID: &str = "io.github.ThinkRoot99.Lanruj";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Un text în limba aleasă.
pub fn tr(lang: Lang, ro: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::Ro => ro,
        Lang::En => en,
    }
}

pub struct App {
    pub journal: Journal,
    pub lang: Lang,
    /// Ziua din pagina din dreapta; în stânga e ziua dinainte.
    shown: Cell<NaiveDate>,
    pub today: Cell<NaiveDate>,
    window: adw::ApplicationWindow,
    left: Page,
    right: Page,
    title: adw::WindowTitle,
    calendar: gtk::Calendar,
    today_count: gtk::Label,
    toasts: adw::ToastOverlay,
    calendar_busy: Cell<bool>,
    cotor: gtk::Box,
    /// Fereastra îngustă: o singură pagină.
    narrow: Cell<bool>,
    mig: RefCell<Option<Migration>>,
    /// Migrarea amânată cu „Mai târziu” (până la următoarea pornire).
    dismissed: Cell<Option<PageRef>>,
    normal_header: Vec<gtk::Widget>,
    mig_header: Vec<gtk::Widget>,
    progress: gtk::Box,
    done_button: gtk::Button,
    sched_button: RefCell<Option<gtk::MenuButton>>,
    /// Intrarea care se editează acum: (pagina, poziția).
    pub editing: Cell<Option<(PageRef, usize)>>,
    /// Ziua din calendarul lunii care se editează acum.
    pub cal_editing: Cell<Option<u32>>,
    pub view: Cell<View>,
    caiet: gtk::Box,
    today_button: gtk::Button,
    migrate_button: gtk::Button,
    nav: gtk::ListBox,
    nav_rows: Vec<gtk::ListBoxRow>,
    pub settings: RefCell<setari::Settings>,
    /// Intrarea aleasă din tastatură (↑ ↓): (pagina, poziția).
    pub selected: Cell<Option<(PageRef, usize)>>,
    /// Iconița din systray, dacă e pornită.
    tray: RefCell<Option<ksni::blocking::Handle<tava::LanrujTray>>>,
    /// Bara laterală (lățimea și dacă e strânsă).
    bara: Rc<bara::Bara>,
    /// Căutarea și foaia care o arată în locul caietului.
    pub cautare: Rc<cautare::Cautare>,
    content_stack: gtk::Stack,
    searching: Cell<bool>,
    /// Ziua propusă în „Adaugă în index” (Ctrl+I) și dacă formularul trebuie deschis.
    pub index_add_date: Cell<Option<NaiveDate>>,
    index_add_open: Cell<bool>,
}

/// Ce ecran se vede.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Day,
    Month(i32, u32),
    /// Jurnalul viitor: luna din stânga (în dreapta, cea de după).
    Future(i32, u32),
    /// Pagina săptămânii (an și săptămână ISO).
    Week(i32, u32),
    /// Indexul.
    Index,
    /// Pagina unui rând din index (ziua și numărul din numele fișierului).
    Topic(NaiveDate, u8),
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

impl App {
    /// Cum se numește o zi față de azi: „Azi”, „Ieri”, „Mâine”, „Acum 5 zile”.
    pub fn relative(&self, d: NaiveDate) -> String {
        let n = (d - self.today.get()).num_days();
        let l = self.lang;
        match n {
            0 => tr(l, "Azi", "Today").into(),
            -1 => tr(l, "Ieri", "Yesterday").into(),
            1 => tr(l, "Mâine", "Tomorrow").into(),
            n if n < 0 => match l {
                Lang::Ro => format!("Acum {} {}", -n, if -n < 20 { "zile" } else { "de zile" }),
                Lang::En => format!("{} days ago", -n),
            },
            n => match l {
                Lang::Ro => format!("Peste {} {}", n, if n < 20 { "zile" } else { "de zile" }),
                Lang::En => format!("In {n} days"),
            },
        }
    }

    fn load(&self, d: NaiveDate) -> Day {
        match self.journal.load(d) {
            Ok(day) => day,
            Err(e) => {
                self.error(&e);
                Day::new(d)
            }
        }
    }

    fn error(&self, e: &std::io::Error) {
        let msg = format!("{} {e}", tr(self.lang, "Nu am putut citi sau scrie jurnalul:", "Could not read or write the journal:"));
        self.toast(&msg);
    }

    pub fn toast(&self, msg: &str) {
        self.toasts.add_toast(adw::Toast::new(msg));
    }

    /// Ce pagini se văd: pe fereastră îngustă una singură
    /// (în migrare, cea cu sarcinile de triat).
    fn layout(&self) {
        // Luna pe fereastră îngustă: calendarul și sarcinile una sub alta.
        let month_stacked = self.narrow.get() && matches!(self.view.get(), View::Month(..) | View::Future(..) | View::Week(..) | View::Index | View::Topic(..)) && !self.migrating();
        self.caiet.set_orientation(if month_stacked { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal });
        if month_stacked {
            self.left.root.set_visible(true);
            self.right.root.set_visible(true);
            self.cotor.set_visible(false);
            for p in [&self.left.root, &self.right.root] {
                p.add_css_class("singura");
            }
            return;
        }
        let (show_left, show_right) = match (self.narrow.get(), self.migrating()) {
            (false, _) => (true, true),
            (true, true) => (true, false),
            (true, false) => (false, true),
        };
        self.left.root.set_visible(show_left);
        self.right.root.set_visible(show_right);
        self.cotor.set_visible(show_left && show_right);
        let single = !(show_left && show_right);
        for p in [&self.left.root, &self.right.root] {
            if single { p.add_css_class("singura") } else { p.remove_css_class("singura") }
        }
    }

    #[cfg(debug_assertions)]
    fn cautare_first_target(&self) -> Option<(PageRef, usize)> {
        self.cautare.first_target()
    }

    fn refresh_index(self: &Rc<Self>) {
        let idx = self.journal.load_index().unwrap_or_else(|e| {
            self.error(&e);
            lanruj_core::Index::default()
        });
        let add = self.left.fill_index_manual(self, &idx);
        self.right.fill_index_auto(self, &self.journal.tags_index(), &self.journal.months_written());
        self.title.set_title("Index");
        let n = idx.items.iter().filter(|i| matches!(i, lanruj_core::IndexItem::Entry { .. })).count();
        self.title.set_subtitle(&match self.lang {
            Lang::Ro => match n {
                0 => "gol încă".to_string(),
                1 => "un rând".to_string(),
                n if n % 100 >= 20 => format!("{n} de rânduri"),
                n => format!("{n} rânduri"),
            },
            Lang::En => format!("{n} lines"),
        });
        if self.index_add_open.take() {
            glib::idle_add_local_once(move || add.popup());
        }
    }

    pub fn go_index(self: &Rc<Self>) {
        self.leave_search();
        if self.migrating() {
            self.end_migration(false);
        }
        self.editing.set(None);
        self.cal_editing.set(None);
        self.selected.set(None);
        self.view.set(View::Index);
        self.refresh();
    }

    pub fn index_add(self: &Rc<Self>, title: &str, start: NaiveDate, end: Option<NaiveDate>) {
        let res = self.journal.load_index().and_then(|mut idx| {
            idx.add(title, start, end, None);
            self.journal.save_index(&idx, self.lang)
        });
        match res {
            Ok(()) => self.toast(tr(self.lang, "Adăugat în index", "Added to the index")),
            Err(e) => self.error(&e),
        }
        self.index_add_date.set(None);
        self.refresh();
    }

    /// Schimbă titlul și zilele unui rând (și titlul paginii lui, dacă are una).
    pub fn index_edit(self: &Rc<Self>, i: usize, title: &str, start: NaiveDate, end: Option<NaiveDate>) {
        let res = self.journal.load_index().and_then(|mut idx| {
            if let Some(lanruj_core::IndexItem::Entry { title: t, start: s, end: e, page }) = idx.items.get_mut(i) {
                *t = title.trim().to_string();
                *s = start;
                *e = end.filter(|x| *x > start);
                if let Some((pd, n)) = page {
                    self.journal.retitle_topic(*pd, *n, title)?;
                }
            }
            self.journal.save_index(&idx, self.lang)
        });
        if let Err(e) = res {
            self.error(&e);
        }
        self.refresh();
    }

    /// Clic pe un rând din index: deschide pagina lui (o creează la primul clic).
    pub fn open_index_item(self: &Rc<Self>, i: usize) {
        let res = self.journal.load_index().and_then(|mut idx| {
            let Some(lanruj_core::IndexItem::Entry { title, start, page, .. }) = idx.items.get_mut(i) else {
                return Ok(None);
            };
            if page.is_none() {
                let n = self.journal.create_topic(title, *start)?;
                *page = Some((*start, n));
                let p = *page;
                self.journal.save_index(&idx, self.lang)?;
                return Ok(p);
            }
            Ok(*page)
        });
        match res {
            Ok(Some((d, n))) => {
                self.leave_search();
                self.editing.set(None);
                self.selected.set(None);
                self.view.set(View::Topic(d, n));
                self.refresh();
                if let Some(q) = &self.left.quick {
                    q.grab_focus();
                }
            }
            Ok(None) => {}
            Err(e) => self.error(&e),
        }
    }

    /// Pagina unui rând din index: în stânga, ce scrii despre el; în dreapta, zilele lui pe scurt.
    fn refresh_topic(self: &Rc<Self>, date: NaiveDate, n: u8) {
        let lang = self.lang;
        let idx = self.journal.load_index().unwrap_or_default();
        let item = idx.items.iter().find_map(|it| match it {
            lanruj_core::IndexItem::Entry { title, start, end, page: Some(p) } if *p == (date, n) => Some((title.clone(), *start, *end)),
            _ => None,
        });
        let r = PageRef::Topic { date, n };
        let page = self.journal.load_ref(r).unwrap_or_else(|_| Day::new(date));
        let (title, start, end) = item.unwrap_or_else(|| (page.heading.clone().unwrap_or_default(), date, None));
        let range = lanruj_core::index::range_text(lang, start, end);
        self.left.fill(self, &page, r, &format!("Index · {range}"), &title, None);
        let count = end.map(|e| (e - start).num_days() + 1).unwrap_or(1).clamp(1, 62);
        self.right.fill_days(self, start, count, tr(lang, "Zilele lui", "Its days"), &range);
        self.title.set_title(&title);
        self.title.set_subtitle(&range);
    }

    pub fn index_remove(self: &Rc<Self>, i: usize) {
        let res = self.journal.load_index().and_then(|mut idx| {
            if i < idx.items.len() {
                idx.items.remove(i);
            }
            self.journal.save_index(&idx, self.lang)
        });
        if let Err(e) = res {
            self.error(&e);
        }
        self.refresh();
    }

    /// Mersul pe altă pagină închide căutarea (fără să mute cursorul).
    fn leave_search(&self) {
        if self.searching.get() {
            self.searching.set(false);
            self.content_stack.set_visible_child_name("caiet");
        }
    }

    /// Ctrl+F: arată căutarea în locul caietului.
    pub fn open_search(self: &Rc<Self>) {
        if self.migrating() {
            return;
        }
        self.searching.set(true);
        self.content_stack.set_visible_child_name("cautare");
        self.refresh();
        self.cautare.entry.grab_focus();
        self.cautare.run(self);
    }

    pub fn close_search(self: &Rc<Self>) {
        if !self.searching.get() {
            return;
        }
        self.searching.set(false);
        self.content_stack.set_visible_child_name("caiet");
        self.refresh();
        if let Some(q) = &self.right.quick {
            q.grab_focus();
        }
    }

    /// Un rezultat al căutării: deschide pagina lui, cu intrarea aleasă.
    pub fn open_result(self: &Rc<Self>, r: PageRef, index: usize) {
        self.searching.set(false);
        self.content_stack.set_visible_child_name("caiet");
        match r {
            PageRef::Day(d) => self.go(d),
            PageRef::Month { year, month } => self.go_month(year, month),
            PageRef::Week { year, week } => self.go_week(year, week),
            PageRef::Future { year, month } => self.go_future(year, month),
            PageRef::Topic { date, n } => {
                self.editing.set(None);
                self.view.set(View::Topic(date, n));
            }
        }
        self.selected.set(Some((r, index)));
        self.refresh();
        gtk::prelude::GtkWindowExt::set_focus(&self.window, None::<&gtk::Widget>);
    }

    pub fn refresh(self: &Rc<Self>) {
        if self.searching.get() {
            for w in &self.normal_header {
                w.set_visible(false);
            }
            self.today_button.set_visible(false);
            self.migrate_button.set_visible(false);
            self.title.set_title(tr(self.lang, "Căutare", "Search"));
            self.title.set_subtitle(tr(self.lang, "Esc închide căutarea", "Esc closes the search"));
            return;
        }
        let month_view = matches!(self.view.get(), View::Month(..)) && !self.migrating();
        let future_view = matches!(self.view.get(), View::Future(..)) && !self.migrating();
        let week_view = matches!(self.view.get(), View::Week(..)) && !self.migrating();
        let topic_view = matches!(self.view.get(), View::Topic(..)) && !self.migrating();
        let index_view = matches!(self.view.get(), View::Index) && !self.migrating();
        if self.migrating() {
            self.refresh_migration();
        } else if let View::Month(y, m) = self.view.get() {
            self.refresh_month(y, m);
        } else if let View::Future(y, m) = self.view.get() {
            self.refresh_future(y, m);
        } else if let View::Week(y, w) = self.view.get() {
            self.refresh_week(y, w);
        } else if index_view {
            self.refresh_index();
        } else if let View::Topic(d, n) = self.view.get() {
            self.refresh_topic(d, n);
        } else {
            self.refresh_normal();
        }
        // Câmpurile de scriere: în viitor pe ambele pagini, în rest doar în dreapta.
        if let Some(q) = &self.left.quick {
            q.set_visible(future_view || week_view || topic_view);
        }
        if let Some(q) = &self.right.quick {
            q.set_visible(!week_view && !index_view && !topic_view);
        }
        let ph = if future_view {
            tr(self.lang, "Scrie aici; ziua la început: . 15 Revizia, o 24 Crăciun", "Write here; day first: . 15 Service, o 24 Christmas")
        } else {
            tr(self.lang, "Scrie aici: . sarcină, o eveniment, - notă, apoi Enter", "Write here: . task, o event, - note, then Enter")
        };
        for q in [&self.left.quick, &self.right.quick].into_iter().flatten() {
            q.set_placeholder_text(Some(ph));
        }
        self.refresh_migration_header();
        let on_today = !month_view && !future_view && !week_view && !index_view && !topic_view && self.shown.get() == self.today.get();
        self.today_button.set_visible(!self.migrating() && !on_today);
        self.migrate_button.set_visible(!self.migrating() && !future_view && !index_view && !topic_view);
        if index_view || topic_view {
            for w in &self.normal_header {
                w.set_visible(false);
            }
        }
        let row = if index_view || topic_view {
            5
        } else if month_view {
            2
        } else if week_view {
            1
        } else if future_view {
            3
        } else {
            0
        };
        if self.nav.selected_row().map(|r| r.index()) != Some(row as i32) {
            self.nav.select_row(Some(&self.nav_rows[row]));
        }
        self.layout();
        let t = self.today.get();
        self.today_count.set_label(&match self.load(t).open_tasks() {
            0 => String::new(),
            n => n.to_string(),
        });
        self.sync_calendar();
    }

    fn refresh_migration(self: &Rc<Self>) {
        let (source, target, current, order, decided, new_ids) = {
            let m = self.mig.borrow();
            let m = m.as_ref().expect("în migrare");
            (m.source, m.target, m.current(), m.order.clone(), m.decided.clone(), m.new_ids.clone())
        };
        let left = self.journal.load_ref(source).unwrap_or_else(|e| {
            self.error(&e);
            Day::new(self.today.get())
        });
        let right = self.journal.load_ref(target).unwrap_or_else(|e| {
            self.error(&e);
            Day::new(self.today.get())
        });
        let panel = match current {
            Some(i) => match left.items.get(i) {
                Some(lanruj_core::Item::Entry(e)) => Some(self.build_panel(e, decided.get(&i).copied())),
                _ => None,
            },
            None => Some(self.build_done_panel()),
        };
        let view = MigView { current, candidates: &order, decided: &decided, panel, new_ids: &new_ids, is_source: true };
        let (sl, st) = self.page_labels(source);
        self.left.fill(self, &left, source, &sl, &st, Some(&view));
        let view = MigView { current: None, candidates: &[], decided: &decided, panel: None, new_ids: &new_ids, is_source: false };
        let (tl, tt) = self.page_labels(target);
        self.right.fill(self, &right, target, &tl, &tt, Some(&view));
    }

    /// Eticheta și titlul unei pagini: („Azi”, „Vineri, 20 noiembrie 2026”) sau
    /// („Luna aceasta · sarcini”, „Noiembrie 2026”).
    fn page_labels(&self, r: PageRef) -> (String, String) {
        match r {
            PageRef::Future { year, month } => (self.future_label(year, month), self.lang.month_title(year, month)),
            PageRef::Week { year, week } => {
                let plan = tr(self.lang, "Planul săptămânii", "Week plan");
                let rel = self.relative_week(year, week);
                (if rel.is_empty() { plan.to_string() } else { format!("{rel} · {plan}") }, self.lang.week_title(week))
            }
            PageRef::Day(d) => (self.relative(d), self.lang.day_title(d)),
            PageRef::Topic { .. } => ("Index".into(), self.journal.load_ref(r).ok().and_then(|d| d.heading).unwrap_or_default()),
            PageRef::Month { year, month } => {
                let rel = self.relative_month(year, month);
                let tasks = tr(self.lang, "Sarcinile lunii", "Monthly tasks");
                (if rel.is_empty() { tasks.to_string() } else { format!("{rel} · {tasks}") }, self.lang.month_title(year, month))
            }
        }
    }

    fn refresh_normal(self: &Rc<Self>) {
        let shown = self.shown.get();
        let prev = shown - Duration::days(1);
        let right = self.load(shown);
        let left = self.load(prev);
        self.right.fill(self, &right, PageRef::Day(shown), &self.relative(shown), &self.lang.day_title(shown), None);
        self.left.fill(self, &left, PageRef::Day(prev), &self.relative(prev), &self.lang.day_title(prev), None);
        self.title.set_title(&self.lang.day_title_short(shown));
        let mut sub = self.relative(shown);
        if shown.year() != self.today.get().year() {
            sub = format!("{} · {}", shown.year(), sub);
        }
        self.title.set_subtitle(&sub);
    }

    /// Cum se numește o lună față de cea de acum.
    fn relative_month(&self, y: i32, m: u32) -> String {
        let t = self.today.get();
        let n = (y - t.year()) * 12 + m as i32 - t.month() as i32;
        let l = self.lang;
        match n {
            0 => tr(l, "Luna aceasta", "This month").into(),
            -1 => tr(l, "Luna trecută", "Last month").into(),
            1 => tr(l, "Luna viitoare", "Next month").into(),
            _ => String::new(),
        }
    }

    fn refresh_month(self: &Rc<Self>, y: i32, m: u32) {
        let month = match self.journal.load_month(y, m) {
            Ok(mo) => mo,
            Err(e) => {
                self.error(&e);
                lanruj_core::Month::new(y, m)
            }
        };
        let title = self.lang.month_title(y, m);
        self.left.fill_calendar(self, &month, &title);
        let mut tasks = Day::new(NaiveDate::from_ymd_opt(y, m, 1).expect("lună validă"));
        tasks.items = month.tasks.clone();
        let r = PageRef::Month { year: y, month: m };
        self.right.fill(self, &tasks, r, tr(self.lang, "Sarcinile lunii", "Monthly tasks"), &title, None);
        self.title.set_title(&title);
        self.title.set_subtitle(&self.relative_month(y, m));
    }

    /// „Săptămâna aceasta”, „Săptămâna trecută”, „Săptămâna viitoare”.
    fn relative_week(&self, y: i32, w: u32) -> String {
        let monday = lanruj_core::week_monday(y, w);
        let this = lanruj_core::week_monday(self.today.get().iso_week().year(), self.today.get().iso_week().week());
        match (monday - this).num_days() / 7 {
            0 => tr(self.lang, "Săptămâna aceasta", "This week").into(),
            -1 => tr(self.lang, "Săptămâna trecută", "Last week").into(),
            1 => tr(self.lang, "Săptămâna viitoare", "Next week").into(),
            _ => String::new(),
        }
    }

    fn refresh_week(self: &Rc<Self>, y: i32, w: u32) {
        let r = PageRef::Week { year: y, week: w };
        let plan = self.journal.load_ref(r).unwrap_or_else(|e| {
            self.error(&e);
            Day::new(lanruj_core::week_monday(y, w))
        });
        let (label, title) = self.page_labels(r);
        self.left.fill(self, &plan, r, &label, &title, None);
        let monday = lanruj_core::week_monday(y, w);
        self.right.fill_week_days(self, monday);
        self.title.set_title(&self.lang.week_title(w));
        self.title.set_subtitle(&self.lang.week_range(monday));
    }

    pub fn go_week(self: &Rc<Self>, y: i32, w: u32) {
        self.leave_search();
        if self.migrating() {
            self.end_migration(false);
        }
        self.editing.set(None);
        self.cal_editing.set(None);
        self.selected.set(None);
        self.view.set(View::Week(y, w));
        self.refresh();
    }

    fn shift_week(self: &Rc<Self>, delta: i32) {
        if let View::Week(y, w) = self.view.get() {
            let d = lanruj_core::week_monday(y, w) + Duration::days(7 * delta as i64);
            self.go_week(d.iso_week().year(), d.iso_week().week());
        }
    }

    /// „Luna viitoare”, „Peste 3 luni”.
    fn future_label(&self, y: i32, m: u32) -> String {
        let t = self.today.get();
        let n = (y - t.year()) * 12 + m as i32 - t.month() as i32;
        match (self.lang, n) {
            (_, 1) => tr(self.lang, "Luna viitoare", "Next month").into(),
            (Lang::Ro, n) if n >= 20 => format!("Peste {n} de luni"),
            (Lang::Ro, n) => format!("Peste {n} luni"),
            (Lang::En, n) => format!("In {n} months"),
        }
    }

    fn refresh_future(self: &Rc<Self>, y: i32, m: u32) {
        let (y2, m2) = lanruj_core::migrate::next_month(y, m);
        for (page, (yy, mm)) in [(&self.left, (y, m)), (&self.right, (y2, m2))] {
            let r = PageRef::Future { year: yy, month: mm };
            let day = self.journal.load_ref(r).unwrap_or_else(|e| {
                self.error(&e);
                Day::new(self.today.get())
            });
            page.fill(self, &day, r, &self.future_label(yy, mm), &self.lang.month_title(yy, mm), None);
        }
        self.title.set_title(tr(self.lang, "Planuri", "Future log"));
        self.title.set_subtitle(&format!("{} - {}", self.lang.month_title(y, m), self.lang.month_title(y2, m2)));
    }

    /// Prima lună din jurnalul viitor: luna viitoare.
    fn future_start(&self) -> (i32, u32) {
        let t = self.today.get();
        lanruj_core::migrate::next_month(t.year(), t.month())
    }

    pub fn go_future(self: &Rc<Self>, y: i32, m: u32) {
        self.leave_search();
        if self.migrating() {
            self.end_migration(false);
        }
        self.editing.set(None);
        self.cal_editing.set(None);
        // Nu mai devreme de luna viitoare.
        let start = self.future_start();
        let (y, m) = if (y, m) < start { start } else { (y, m) };
        self.selected.set(None);
        self.view.set(View::Future(y, m));
        self.refresh();
    }

    /// Lunile din jurnalul viitor, două câte două.
    fn shift_future(self: &Rc<Self>, delta: i32) {
        if let View::Future(y, m) = self.view.get() {
            let idx = y * 12 + m as i32 - 1 + 2 * delta;
            self.go_future(idx.div_euclid(12), (idx.rem_euclid(12) + 1) as u32);
        }
    }

    pub fn go_month(self: &Rc<Self>, y: i32, m: u32) {
        self.leave_search();
        if self.migrating() {
            self.end_migration(false);
        }
        self.editing.set(None);
        self.cal_editing.set(None);
        self.selected.set(None);
        self.view.set(View::Month(y, m));
        self.refresh();
    }

    /// Luna de dinainte (-1) sau de după (+1).
    fn shift_month(self: &Rc<Self>, delta: i32) {
        if let View::Month(y, m) = self.view.get() {
            let idx = y * 12 + m as i32 - 1 + delta;
            self.go_month(idx.div_euclid(12), (idx.rem_euclid(12) + 1) as u32);
        }
    }

    pub fn start_cal_edit(self: &Rc<Self>, day: u32) {
        self.cal_editing.set(Some(day));
        self.refresh();
    }

    /// Textul unei zile din calendarul lunii; mai multe rânduri se despart prin „;”.
    pub fn finish_cal_edit(self: &Rc<Self>, text: Option<String>) {
        let Some(day) = self.cal_editing.take() else { return };
        if let (Some(text), View::Month(y, m)) = (text, self.view.get()) {
            let texts: Vec<String> = text.split(';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
            if let Err(e) = self.journal.update_month(y, m, self.lang, |mo| mo.set_day(day, &texts)) {
                self.error(&e);
            }
        }
        self.refresh();
    }

    /// Calendarul mic: ziua arătată selectată, zilele scrise cu un punct.
    fn sync_calendar(&self) {
        self.calendar_busy.set(true);
        let d = match self.mig.borrow().as_ref().map(|m| m.source) {
            Some(PageRef::Day(d)) => d,
            _ => self.shown.get(),
        };
        if let Ok(dt) = glib::DateTime::from_local(d.year(), d.month() as i32, d.day() as i32, 12, 0, 0.0) {
            self.calendar.select_day(&dt);
        }
        self.mark_calendar();
        self.calendar_busy.set(false);
    }

    fn mark_calendar(&self) {
        self.calendar.clear_marks();
        let (y, m) = (self.calendar.year(), self.calendar.month() as u32 + 1);
        for day in self.journal.written_days(y, m) {
            self.calendar.mark_day(day);
        }
    }

    pub fn go(self: &Rc<Self>, d: NaiveDate) {
        self.leave_search();
        // Plecarea din migrare spre altă zi e ca „Mai târziu”.
        if self.migrating() {
            self.end_migration(false);
        }
        self.editing.set(None);
        self.selected.set(None);
        self.view.set(View::Day);
        self.shown.set(d);
        self.refresh();
    }

    pub fn change(self: &Rc<Self>, date: NaiveDate, f: impl FnOnce(&mut Day)) {
        if let Err(e) = self.journal.update(date, self.lang, f) {
            self.error(&e);
        }
        self.refresh();
    }

    pub fn toggle(self: &Rc<Self>, r: PageRef, index: usize) {
        use lanruj_core::{Kind, TaskState};
        let res = self.journal.update_ref(r, self.lang, |d| {
            if let Some(e) = d.entry_mut(index) {
                e.kind = match e.kind {
                    Kind::Task(TaskState::Open) => Kind::Task(TaskState::Done),
                    Kind::Task(TaskState::Done) => Kind::Task(TaskState::Open),
                    k => k,
                };
            }
        });
        if let Err(e) = res {
            self.error(&e);
        }
        self.refresh();
    }

    pub fn start_edit(self: &Rc<Self>, r: PageRef, index: usize) {
        self.editing.set(Some((r, index)));
        self.refresh();
    }

    /// Enter salvează (`Some`), Esc renunță (`None`).
    pub fn finish_edit(self: &Rc<Self>, text: Option<String>) {
        let Some((r, index)) = self.editing.take() else { return };
        if let Some(text) = text {
            let mut ok = true;
            if let Err(e) = self.journal.update_ref(r, self.lang, |d| ok = d.edit(index, &text)) {
                self.error(&e);
            } else if !ok {
                self.toast(tr(
                    self.lang,
                    "Intrarea nu poate rămâne goală. Ca s-o scoți, folosește Șterge.",
                    "An entry can't be empty. To remove it, use Delete.",
                ));
            }
        }
        self.refresh();
        if let Some(q) = &self.right.quick {
            q.grab_focus();
        }
    }

    /// Șterge intrarea (cu notițele ei); un buton „Anulează” o pune la loc.
    pub fn window_night(&self) -> bool {
        self.window.has_css_class("noapte")
    }

    /// Culoarea întregii intrări (din meniul „Culoare…”): `(marker?, culoare)` sau `None` = fără culoare.
    pub fn color_entry(self: &Rc<Self>, r: PageRef, index: usize, paint: Option<(bool, String)>) {
        use lanruj_core::colors::{Paint, paint as do_paint};
        let res = self.journal.update_ref(r, self.lang, |d| {
            if let Some(e) = d.entry_mut(index) {
                let p = paint.as_ref().map(|(m, c)| if *m { Paint::Marker(c) } else { Paint::Color(c) });
                e.text = do_paint(&e.text, 0, usize::MAX, p);
            }
        });
        if let Err(e) = res {
            self.error(&e);
        }
        self.refresh();
    }

    /// Mută intrarea pe altă pagină (din meniul „Mută…”).
    pub fn move_entry(self: &Rc<Self>, r: PageRef, index: usize, to: PageRef) {
        match self.journal.move_entry(r, index, to, self.lang) {
            Ok(()) => {
                let msg = match self.lang {
                    Lang::Ro => format!("Mutată: {}", migrare::page_short(self.lang, to)),
                    Lang::En => format!("Moved: {}", migrare::page_short(self.lang, to)),
                };
                self.toast(&msg);
            }
            Err(e) => self.error(&e),
        }
        self.refresh();
    }

    pub fn delete_entry(self: &Rc<Self>, r: PageRef, index: usize) {
        let day = match self.journal.load_ref(r) {
            Ok(d) => d,
            Err(e) => return self.error(&e),
        };
        if !matches!(day.items.get(index), Some(lanruj_core::Item::Entry(_))) {
            return;
        }
        let removed: Vec<lanruj_core::Item> = day.items[index..=index + day.children_count(index)].to_vec();
        if let Err(e) = self.journal.update_ref(r, self.lang, |d| {
            d.remove(index);
        }) {
            self.error(&e);
            return;
        }
        self.refresh();
        let toast = adw::Toast::builder()
            .title(tr(self.lang, "Intrare ștearsă", "Entry deleted"))
            .button_label(tr(self.lang, "Anulează", "Undo"))
            .build();
        let weak = Rc::downgrade(self);
        toast.connect_button_clicked(move |_| {
            let Some(s) = weak.upgrade() else { return };
            let items = removed.clone();
            if let Err(e) = s.journal.update_ref(r, s.lang, move |d| {
                let at = index.min(d.items.len());
                d.items.splice(at..at, items);
            }) {
                s.error(&e);
            }
            s.refresh();
        });
        self.toasts.add_toast(toast);
    }

    fn quick_add(self: &Rc<Self>, entry: &gtk::Entry, left: bool) {
        let Some(e) = parse_quick(&entry.text()) else { return };
        // În migrare, câmpul de jos scrie pe ziua din dreapta (mâine).
        let r = match (self.mig.borrow().as_ref().map(|m| m.target), self.view.get()) {
            (Some(target), _) => target,
            (None, View::Month(year, month)) => PageRef::Month { year, month },
            (None, View::Future(year, month)) => {
                let (year, month) = if left { (year, month) } else { lanruj_core::migrate::next_month(year, month) };
                PageRef::Future { year, month }
            }
            (None, View::Week(year, week)) => PageRef::Week { year, week },
            (None, View::Index) => return,
            (None, View::Topic(date, n)) => PageRef::Topic { date, n },
            (None, View::Day) => PageRef::Day(self.shown.get()),
        };
        match self.journal.update_ref(r, self.lang, |d| d.push(e)) {
            Ok(_) => entry.set_text(""),
            Err(err) => self.error(&err),
        }
        self.refresh();
    }
}

impl App {
    /// Pornește sau oprește iconița din systray. Dacă sistemul nu are systray,
    /// setarea rămâne oprită și apare un mesaj.
    fn set_tray(&self, on: bool) -> bool {
        if on {
            if self.tray.borrow().is_some() {
                return true;
            }
            match tava::start(self.lang) {
                Some(h) => {
                    *self.tray.borrow_mut() = Some(h);
                    true
                }
                None => {
                    self.toast(tr(
                        self.lang,
                        "Sistemul nu are systray. Pe GNOME e nevoie de extensia AppIndicator.",
                        "There is no system tray. On GNOME, the AppIndicator extension is needed.",
                    ));
                    false
                }
            }
        } else {
            if let Some(h) = self.tray.borrow_mut().take() {
                h.shutdown().wait();
            }
            true
        }
    }

    /// Fereastra cu scurtăturile de taste (Ctrl+?).
    /// Pagina pe care lucrează tastatura: cea cu câmpul de scriere
    /// (în dreapta; în Planuri și Pagina săptămânii, cea din stânga).
    fn key_page(&self) -> Option<(PageRef, &page::Page)> {
        if self.migrating() || self.searching.get() {
            return None;
        }
        Some(match self.view.get() {
            View::Day => (PageRef::Day(self.shown.get()), &self.right),
            View::Month(year, month) => (PageRef::Month { year, month }, &self.right),
            View::Week(year, week) => (PageRef::Week { year, week }, &self.left),
            View::Future(year, month) => (PageRef::Future { year, month }, &self.left),
            View::Index => return None,
            View::Topic(date, n) => (PageRef::Topic { date, n }, &self.left),
        })
    }

    /// Tastele pentru intrări: ↑ ↓ alege, Enter editează, Delete șterge,
    /// Ctrl+Enter bifează, Esc întoarce în câmpul de scriere.
    fn entry_key(self: &Rc<Self>, key: gdk::Key, mods: gdk::ModifierType) -> glib::Propagation {
        let Some((r, page)) = self.key_page() else { return glib::Propagation::Proceed };
        if self.editing.get().is_some() || self.cal_editing.get().is_some() {
            return glib::Propagation::Proceed;
        }
        let in_quick = page.quick.as_ref().is_some_and(|q| q.has_focus() || q.focus_child().is_some());
        let focus = gtk::prelude::GtkWindowExt::focus(&self.window);
        // În alte câmpuri de text (etichete, căutare) tastele sunt ale lor.
        if !in_quick && focus.as_ref().is_some_and(|w| w.is::<gtk::Text>()) {
            return glib::Propagation::Proceed;
        }
        let day = match self.journal.load_ref(r) {
            Ok(d) => d,
            Err(_) => return glib::Propagation::Proceed,
        };
        let entries: Vec<usize> = day.items.iter().enumerate().filter(|(_, it)| matches!(it, lanruj_core::Item::Entry(_))).map(|(i, _)| i).collect();
        let sel = self.selected.get().filter(|(sr, _)| *sr == r).map(|(_, i)| i);
        let pos = sel.and_then(|i| entries.iter().position(|&e| e == i));
        let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
        let focus_quick = |s: &Rc<Self>| {
            s.selected.set(None);
            s.refresh();
            if let Some(q) = &page.quick {
                q.grab_focus();
            }
        };
        match key {
            gdk::Key::Up if !ctrl => {
                let next = match pos {
                    Some(p) if p > 0 => Some(entries[p - 1]),
                    Some(p) => Some(entries[p]),
                    None => entries.last().copied(),
                };
                if let Some(i) = next {
                    self.selected.set(Some((r, i)));
                    gtk::prelude::GtkWindowExt::set_focus(&self.window, None::<&gtk::Widget>);
                    self.refresh();
                }
                glib::Propagation::Stop
            }
            gdk::Key::Down if !ctrl && pos.is_some() => {
                let p = pos.expect("verificat");
                if p + 1 < entries.len() {
                    self.selected.set(Some((r, entries[p + 1])));
                    self.refresh();
                } else {
                    focus_quick(self);
                }
                glib::Propagation::Stop
            }
            gdk::Key::Return | gdk::Key::KP_Enter if pos.is_some() && !in_quick => {
                let i = sel.expect("verificat");
                if ctrl {
                    self.toggle(r, i);
                } else {
                    self.start_edit(r, i);
                }
                glib::Propagation::Stop
            }
            gdk::Key::Delete | gdk::Key::KP_Delete if pos.is_some() && !in_quick => {
                let i = sel.expect("verificat");
                // După ștergere, selecția trece pe intrarea de dinainte (sau rămâne la loc).
                let after = match pos {
                    Some(p) if p > 0 => Some(entries[p - 1]),
                    // Prima intrare: cea de după îi ia locul.
                    Some(_) if entries.len() > 1 => Some(i),
                    _ => None,
                };
                self.selected.set(after.map(|a| (r, a)));
                self.delete_entry(r, i);
                glib::Propagation::Stop
            }
            gdk::Key::Escape if pos.is_some() => {
                focus_quick(self);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    }

    fn show_shortcuts(&self) {
        let l = self.lang;
        let dialog = adw::ShortcutsDialog::new();
        let sections: [(&str, Vec<(&str, &str)>); 4] = [
            (
                tr(l, "General", "General"),
                vec![
                    (tr(l, "Setări", "Preferences"), "<Control>comma"),
                    (tr(l, "Scurtăturile de taste", "Keyboard shortcuts"), "<Control>question"),
                    (tr(l, "Ieșire", "Quit"), "<Control>q"),
                    (tr(l, "Strânge sau întinde bara laterală", "Collapse or expand the sidebar"), "F9"),
                    (tr(l, "Caută în tot jurnalul", "Search the whole journal"), "<Control>f"),
                    (tr(l, "Adaugă ziua în index", "Add the day to the index"), "<Control>i"),
                    (tr(l, "Mărește sau micșorează fereastra", "Maximize or restore the window"), "F11"),
                    (tr(l, "Închide fereastra", "Close window"), "<Control>w"),
                ],
            ),
            (
                tr(l, "Navigare", "Navigation"),
                vec![
                    (tr(l, "Ziua, săptămâna sau luna de dinainte", "Previous day, week or month"), "<Alt>Left <Control>Page_Up"),
                    (tr(l, "Ziua, săptămâna sau luna de după", "Next day, week or month"), "<Alt>Right <Control>Page_Down"),
                    (tr(l, "Înapoi la ziua de azi", "Back to today"), "<Alt>Home"),
                ],
            ),
            (
                tr(l, "Scriere", "Writing"),
                vec![
                    (tr(l, "Adaugă ce ai scris în câmpul de jos", "Add what you typed in the field below"), "Return"),
                    (tr(l, "Alege intrarea de deasupra sau de dedesubt", "Select the entry above or below"), "Up Down"),
                    (tr(l, "Editează intrarea aleasă", "Edit the selected entry"), "Return"),
                    (tr(l, "Bifează sau debifează sarcina aleasă", "Check or uncheck the selected task"), "<Control>Return"),
                    (tr(l, "Șterge intrarea aleasă", "Delete the selected entry"), "Delete"),
                    (tr(l, "Înapoi în câmpul de scriere", "Back to the writing field"), "Escape"),
                    (tr(l, "Salvează intrarea editată", "Save the edited entry"), "Return"),
                    (tr(l, "Renunță la editare", "Cancel editing"), "Escape"),
                ],
            ),
            (
                tr(l, "Migrare", "Migration"),
                vec![
                    (tr(l, "Pornește migrarea", "Start the migration"), "<Control>m"),
                    (tr(l, "Mâine, luna sau săptămâna următoare", "Tomorrow, next month or week"), "m Return"),
                    (tr(l, "Programează", "Schedule"), "p"),
                    (tr(l, "Renunț", "Drop"), "r"),
                    (tr(l, "O las", "Keep"), "l"),
                    (tr(l, "Altă sarcină", "Another task"), "Up Down"),
                    (tr(l, "Mai târziu", "Later"), "Escape"),
                ],
            ),
        ];
        for (title, items) in sections {
            let sec = adw::ShortcutsSection::new(Some(title));
            for (what, accel) in items {
                sec.add(adw::ShortcutsItem::new(what, accel));
            }
            dialog.add(sec);
        }
        // Prefixele scrierii rapide nu sunt scurtături, dar e bine să fie la îndemână.
        let sec = adw::ShortcutsSection::new(Some(tr(l, "Semnele de la începutul rândului", "Signs at the start of a line")));
        for (what, accel) in [
            (tr(l, "Sarcină: . text", "Task: . text"), "period"),
            (tr(l, "Eveniment: o 10:00 text", "Event: o 10:00 text"), "o"),
            (tr(l, "Notă: - text", "Note: - text"), "minus"),
            (tr(l, "Important: . * text", "Important: . * text"), "asterisk"),
            (tr(l, "Idee: - ! text", "Idea: - ! text"), "exclam"),
        ] {
            sec.add(adw::ShortcutsItem::new(what, accel));
        }
        dialog.add(sec);
        dialog.present(Some(&self.window));
    }

    /// Aplică setările care se văd pe loc: hârtia și noaptea; apoi le salvează.
    fn apply_settings(&self) {
        let s = self.settings.borrow().clone();
        for c in self.window.css_classes() {
            if c.starts_with("hartie-") || c.starts_with("culoare-") {
                self.window.remove_css_class(&c);
            }
        }
        for c in s.css_classes() {
            self.window.add_css_class(&c);
        }
        adw::StyleManager::default().set_color_scheme(match s.scheme {
            setari::Scheme::System => adw::ColorScheme::Default,
            setari::Scheme::Light => adw::ColorScheme::ForceLight,
            setari::Scheme::Dark => adw::ColorScheme::ForceDark,
        });
        if let Err(e) = s.save() {
            self.error(&e);
        }
    }

    fn show_preferences(self: &Rc<Self>) {
        let lang = self.lang;
        let s = self.settings.borrow().clone();
        let dialog = adw::PreferencesDialog::new();
        dialog.set_title(tr(lang, "Setări", "Preferences"));
        let page = adw::PreferencesPage::new();

        // Un rând cu variante, legat de o setare.
        fn combo<T: Copy + PartialEq + 'static, const N: usize>(
            app: &Rc<App>,
            title: &str,
            table: &'static [(T, &'static str, &'static str, &'static str); N],
            current: T,
            set: fn(&mut setari::Settings, T),
            after: fn(&Rc<App>),
        ) -> adw::ComboRow {
            let names: Vec<&str> = table.iter().map(|(_, _, ro, en)| if app.lang == Lang::Ro { *ro } else { *en }).collect();
            let row = adw::ComboRow::builder().title(title).model(&gtk::StringList::new(&names)).build();
            row.set_selected(table.iter().position(|(v, ..)| *v == current).unwrap_or(0) as u32);
            let weak = Rc::downgrade(app);
            row.connect_selected_notify(move |r| {
                let Some(app) = weak.upgrade() else { return };
                if let Some((v, ..)) = table.get(r.selected() as usize) {
                    set(&mut app.settings.borrow_mut(), *v);
                    app.apply_settings();
                    after(&app);
                }
            });
            row
        }

        let paper = adw::PreferencesGroup::builder().title(tr(lang, "Hârtia", "Paper")).build();
        paper.add(&combo(self, tr(lang, "Model", "Pattern"), &setari::PAPERS, s.paper, |st, v| st.paper = v, |_| {}));
        paper.add(&combo(self, tr(lang, "Culoare", "Color"), &setari::COLORS, s.color, |st, v| st.color = v, |_| {}));
        paper.add(&combo(
            self,
            tr(lang, "Noaptea", "Dark mode"),
            &setari::SCHEMES,
            s.scheme,
            |st, v| st.scheme = v,
            |_| {},
        ));
        page.add(&paper);

        let mig = adw::PreferencesGroup::builder()
            .title(tr(lang, "Migrarea", "Migration"))
            .description(tr(
                lang,
                "Trierea sarcinilor neterminate: seara pentru zi, la început de lună și de săptămână pentru listele lor.",
                "Sorting unfinished tasks: in the evening for the day, at the start of the month and week for their lists.",
            ))
            .build();
        let auto = adw::SwitchRow::builder()
            .title(tr(lang, "Pornește singură", "Starts by itself"))
            .subtitle(tr(lang, "Altfel doar cu butonul Migrează (Ctrl+M)", "Otherwise only with the Migrate button (Ctrl+M)"))
            .active(s.auto_migrate)
            .build();
        let weak = Rc::downgrade(self);
        auto.connect_active_notify(move |r| {
            if let Some(app) = weak.upgrade() {
                app.settings.borrow_mut().auto_migrate = r.is_active();
                app.apply_settings();
            }
        });
        mig.add(&auto);
        let hour = adw::SpinRow::with_range(16.0, 23.0, 1.0);
        hour.set_title(tr(lang, "Ora migrării de seară", "Evening migration time"));
        hour.set_subtitle(tr(lang, "De la ora aceasta (de exemplu 21 = 21:00)", "From this hour (for example 21 = 9 pm)"));
        hour.set_value(s.evening_hour as f64);
        auto.bind_property("active", &hour, "sensitive").sync_create().build();
        let weak = Rc::downgrade(self);
        hour.connect_value_notify(move |r| {
            if let Some(app) = weak.upgrade() {
                app.settings.borrow_mut().evening_hour = r.value() as u32;
                app.apply_settings();
            }
        });
        mig.add(&hour);
        page.add(&mig);

        let win = adw::PreferencesGroup::builder().title(tr(lang, "Fereastra", "Window")).build();
        let tray = adw::SwitchRow::builder()
            .title(tr(lang, "La închidere rămâne în systray", "Keep in the system tray when closed"))
            .subtitle(tr(
                lang,
                "Iconița din bara de sus o redeschide; Ieșire din meniu o oprește. Pe Wayland, minimizarea nu poate trimite fereastra în systray.",
                "The tray icon reopens it; Quit in the menu stops it. On Wayland, minimizing can't send the window to the tray.",
            ))
            .active(s.tray)
            .build();
        let weak = Rc::downgrade(self);
        tray.connect_active_notify(move |r| {
            let Some(app) = weak.upgrade() else { return };
            let ok = app.set_tray(r.is_active());
            if r.is_active() && !ok {
                r.set_active(false);
                return;
            }
            app.settings.borrow_mut().tray = r.is_active();
            app.apply_settings();
        });
        win.add(&tray);
        page.add(&win);

        let lg = adw::PreferencesGroup::builder()
            .title(tr(lang, "Limba", "Language"))
            .description(tr(lang, "Aplicația se redeschide în limba aleasă.", "The app reopens in the chosen language."))
            .build();
        lg.add(&combo(self, tr(lang, "Limba aplicației", "App language"), &setari::LANGS, s.lang, |st, v| st.lang = v, |app| {
            // Fereastra se reface, cu noua limbă, după ce se închide dialogul.
            let weak = Rc::downgrade(app);
            glib::idle_add_local_once(move || {
                let Some(app) = weak.upgrade() else { return };
                let Some(gapp) = app.window.application() else { return };
                let Some(gapp) = gapp.downcast_ref::<adw::Application>().cloned() else { return };
                let _hold = gapp.hold();
                app.set_tray(false);
                let old = app.window.clone();
                drop(app);
                old.destroy();
                build_ui(&gapp);
            });
        }));
        page.add(&lg);

        dialog.add(&page);
        dialog.present(Some(&self.window));
    }
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        // Iconițele din dosarul proiectului, cât timp aplicația nu e instalată.
        let theme = gtk::IconTheme::for_display(&display);
        theme.add_search_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/icons"));
    }
    gtk::Window::set_default_icon_name(APP_ID);
}

/// Un rând din bara laterală: iconița, numele și (opțional) un număr.
/// Întoarce și ce dispare când bara e strânsă.
fn nav_row(icon: &str, text: &str, extra: Option<&gtk::Label>) -> (gtk::ListBoxRow, gtk::Box, Vec<gtk::Widget>) {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    b.append(&gtk::Image::from_icon_name(icon));
    let l = gtk::Label::builder().label(text).xalign(0.0).hexpand(true).ellipsize(gtk::pango::EllipsizeMode::End).build();
    b.append(&l);
    let mut hide: Vec<gtk::Widget> = vec![l.upcast()];
    if let Some(e) = extra {
        b.append(e);
        hide.push(e.clone().upcast());
    }
    let row = gtk::ListBoxRow::builder().child(&b).tooltip_text(text).build();
    (row, b, hide)
}

fn build_ui(app: &adw::Application) {
    // A doua pornire aduce în față fereastra deja deschisă.
    if let Some(w) = app.active_window() {
        w.present();
        return;
    }
    let settings = setari::Settings::load();
    let lang = settings.lang();
    let journal = Journal::new(Journal::default_root());

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Lanruj")
        .default_width(1240)
        .default_height(780)
        .width_request(360)
        .height_request(420)
        .build();
    window.add_css_class("lanruj");
    let style = adw::StyleManager::default();
    style.set_color_scheme(match settings.scheme {
        setari::Scheme::System => adw::ColorScheme::Default,
        setari::Scheme::Light => adw::ColorScheme::ForceLight,
        setari::Scheme::Dark => adw::ColorScheme::ForceDark,
    });
    for c in settings.css_classes() {
        window.add_css_class(&c);
    }
    let set_night = {
        let w = window.clone();
        move |s: &adw::StyleManager| {
            if s.is_dark() { w.add_css_class("noapte") } else { w.remove_css_class("noapte") }
        }
    };
    set_night(&style);
    style.connect_dark_notify(set_night);

    // Bara laterală
    let today_count = gtk::Label::builder().css_classes(["dim-label", "caption"]).build();
    let nav = gtk::ListBox::builder().css_classes(["navigation-sidebar"]).build();
    let soon = tr(lang, "Vine într-un pas următor", "Coming in a later step");
    let built = [
        nav_row("document-edit-symbolic", tr(lang, "Pagina zilei", "Day"), Some(&today_count)),
        nav_row("view-dual-symbolic", tr(lang, "Pagina săptămânii", "Week"), None),
        nav_row("view-grid-symbolic", tr(lang, "Pagina lunii", "Month"), None),
        nav_row("task-due-symbolic", tr(lang, "Planuri", "Future log"), None),
        nav_row(
            "view-sort-ascending-symbolic",
            tr(lang, "Monitorizare", "Tracking"),
            Some(&gtk::Label::builder().label("v2").css_classes(["dim-label", "caption"]).build()),
        ),
        nav_row("view-list-bullet-symbolic", "Index", None),
    ];
    let rows: Vec<gtk::ListBoxRow> = built.iter().map(|(r, _, _)| r.clone()).collect();
    let row_boxes: Vec<gtk::Box> = built.iter().map(|(_, b, _)| b.clone()).collect();
    let mut hide_when_compact: Vec<gtk::Widget> = built.iter().flat_map(|(_, _, h)| h.clone()).collect();
    for (i, r) in rows.iter().enumerate() {
        if !matches!(i, 0..=3 | 5) {
            r.set_sensitive(false);
            r.set_tooltip_text(Some(if i == 4 { tr(lang, "În versiunea a doua", "In version two") } else { soon }));
        }
        nav.append(r);
    }
    nav.select_row(Some(&rows[0]));

    let calendar = gtk::Calendar::builder().css_classes(["calendar-mic"]).build();
    let side_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    side_box.append(&nav);
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    side_box.append(&spacer);
    // Calendarul se poate îngusta odată cu bara (taie din margini, fără bară de derulare).
    let cal_box = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .propagate_natural_width(true)
        .child(&calendar)
        .build();
    side_box.append(&cal_box);

    // Meniul principal: toate acțiunile care au taste, pe grupuri (GNOME arată singur tasta).
    let menu = gio::Menu::new();
    let section = |items: &[(&str, &str)]| {
        let m = gio::Menu::new();
        for (label, action) in items {
            m.append(Some(label), Some(action));
        }
        m
    };
    menu.append_section(
        None,
        &section(&[
            (tr(lang, "Caută în jurnal", "Search the journal"), "win.search"),
            (tr(lang, "Azi", "Today"), "win.today"),
            (tr(lang, "Pagina de dinainte", "Previous page"), "win.prev"),
            (tr(lang, "Pagina de după", "Next page"), "win.next"),
            (tr(lang, "Migrează", "Migrate"), "win.migrate"),
            (tr(lang, "Adaugă ziua în index", "Add the day to the index"), "win.index-add"),
        ]),
    );
    menu.append_section(None, &section(&[(tr(lang, "Strânge sau întinde bara laterală", "Collapse or expand the sidebar"), "win.sidebar")]));
    menu.append_section(
        None,
        &section(&[
            (tr(lang, "Setări", "Preferences"), "win.preferences"),
            (tr(lang, "Scurtături de taste", "Keyboard Shortcuts"), "win.shortcuts"),
            (tr(lang, "Despre Lanruj", "About Lanruj"), "app.about"),
        ]),
    );
    menu.append_section(
        None,
        &section(&[
            (tr(lang, "Mărește sau micșorează fereastra", "Maximize or restore the window"), "win.maximize"),
            (tr(lang, "Închide fereastra", "Close Window"), "window.close"),
            (tr(lang, "Ieșire", "Quit"), "app.quit"),
        ]),
    );
    let side_header = adw::HeaderBar::builder().show_end_title_buttons(false).show_start_title_buttons(false).build();
    let side_title = gtk::Label::builder().label("Lanruj").css_classes(["heading"]).ellipsize(gtk::pango::EllipsizeMode::End).build();
    side_header.set_title_widget(Some(&side_title));
    let toggle = gtk::Button::builder()
        .icon_name("sidebar-show-symbolic")
        .action_name("win.sidebar")
        .tooltip_text(tr(lang, "Strânge sau întinde bara laterală (F9)", "Collapse or expand the sidebar (F9)"))
        .build();
    side_header.pack_start(&toggle);
    let search = gtk::Button::from_icon_name("edit-find-symbolic");
    search.set_action_name(Some("win.search"));
    search.set_tooltip_text(Some(tr(lang, "Caută în tot jurnalul (Ctrl+F)", "Search the whole journal (Ctrl+F)")));
    side_header.pack_end(&search);
    hide_when_compact.push(search.clone().upcast());
    hide_when_compact.push(cal_box.clone().upcast());
    let side_view = adw::ToolbarView::new();
    side_view.add_top_bar(&side_header);
    side_view.set_content(Some(&side_box));
    side_view.add_css_class("sidebar-pane");

    // Caietul
    let placeholder = tr(lang, "Scrie aici: . sarcină, o eveniment, - notă, apoi Enter", "Write here: . task, o event, - note, then Enter");
    let left = Page::new(Side::Left, true, placeholder);
    let right = Page::new(Side::Right, true, placeholder);
    let caiet = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    caiet.add_css_class("caiet");
    let cotor = gtk::Box::new(gtk::Orientation::Vertical, 0);
    cotor.add_css_class("cotor");
    caiet.append(&left.root);
    caiet.append(&cotor);
    caiet.append(&right.root);
    // Caietul și, în locul lui, căutarea.
    let content_stack = gtk::Stack::builder().transition_type(gtk::StackTransitionType::Crossfade).build();
    content_stack.add_named(&caiet, Some("caiet"));
    let cautare = Rc::new(cautare::Cautare::new(lang));
    content_stack.add_named(&cautare.root, Some("cautare"));
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&content_stack));

    let title = adw::WindowTitle::new("", "");
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&title));
    let prev = gtk::Button::from_icon_name("go-previous-symbolic");
    prev.set_action_name(Some("win.prev"));
    prev.set_tooltip_text(Some(tr(lang, "Ziua de dinainte (Alt+←)", "Previous day (Alt+←)")));
    header.pack_start(&prev);
    let today_button = gtk::Button::builder()
        .label(tr(lang, "Azi", "Today"))
        .action_name("win.today")
        .tooltip_text(tr(lang, "Înapoi la ziua de azi (Alt+Home)", "Back to today (Alt+Home)"))
        .visible(false)
        .build();
    header.pack_start(&today_button);
    // Meniul principal stă în antetul din dreapta, ca să rămână la îndemână și cu bara strânsă.
    let menu_button = gtk::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&menu).primary(true).build();
    // F10 (din GNOME) deschide meniul; tot F10 îl și închide, nu doar Esc.
    if let Some(pop) = menu_button.popover() {
        let keys = gtk::EventControllerKey::new();
        let p = pop.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::F10 {
                p.popdown();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        pop.add_controller(keys);
    }
    header.pack_end(&menu_button);
    let next = gtk::Button::from_icon_name("go-next-symbolic");
    next.set_action_name(Some("win.next"));
    next.set_tooltip_text(Some(tr(lang, "Ziua de după (Alt+→)", "Next day (Alt+→)")));
    header.pack_end(&next);
    let migrate = gtk::Button::builder()
        .child(&adw::ButtonContent::builder().icon_name("go-jump-symbolic").label(tr(lang, "Migrează", "Migrate")).build())
        .action_name("win.migrate")
        .tooltip_text(tr(lang, "Triază sarcinile neterminate ale zilei (Ctrl+M)", "Sort the day's unfinished tasks (Ctrl+M)"))
        .build();
    header.pack_end(&migrate);
    // Antetul din timpul migrării
    let later = gtk::Button::builder()
        .label(tr(lang, "Mai târziu", "Later"))
        .tooltip_text(tr(lang, "Oprește migrarea; ce ai ales rămâne (Esc)", "Stop the migration; your choices stay (Esc)"))
        .visible(false)
        .build();
    header.pack_start(&later);
    let done_button = gtk::Button::builder().label(tr(lang, "Gata", "Done")).visible(false).build();
    header.pack_end(&done_button);
    let progress = gtk::Box::builder().spacing(4).visible(false).valign(gtk::Align::Center).build();
    progress.add_css_class("progres");
    header.pack_end(&progress);
    let content_view = adw::ToolbarView::new();
    content_view.add_top_bar(&header);
    content_view.set_content(Some(&toasts));

    // Bara laterală (lățime fixă) și caietul.
    side_view.set_hexpand(false);
    let main_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    main_box.append(&side_view);
    main_box.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    content_view.set_hexpand(true);
    main_box.append(&content_view);
    window.set_content(Some(&main_box));
    let bara = Rc::new(bara::Bara::new(
        side_view.clone().upcast(),
        hide_when_compact,
        row_boxes,
        side_header.clone(),
        toggle.clone(),
        side_title.clone().upcast(),
        settings.sidebar_compact,
    ));
    bara.apply();

    // Pe ecran îngust: o singură pagină; și mai îngust: bara laterală ascunsă.
    let one_page = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 900sp").expect("condiție"));
    // Fereastra prea mică: bara rămâne doar cu iconițe.
    let narrow = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 700sp").expect("condiție"));
    for (apply, forced) in [(true, true), (false, false)] {
        let b = bara.clone();
        let f = move |_: &adw::Breakpoint| {
            b.forced.set(forced);
            b.apply();
        };
        if apply { narrow.connect_apply(f); } else { narrow.connect_unapply(f); }
    }
    window.add_breakpoint(narrow);

    let t = today();
    let state = Rc::new(App {
        journal,
        lang,
        shown: Cell::new(t),
        today: Cell::new(t),
        window: window.clone(),
        left,
        right,
        title,
        calendar: calendar.clone(),
        today_count,
        toasts,
        calendar_busy: Cell::new(false),
        cotor,
        narrow: Cell::new(false),
        mig: RefCell::new(None),
        dismissed: Cell::new(None),
        normal_header: vec![prev.upcast(), next.upcast()],
        mig_header: vec![later.clone().upcast(), progress.clone().upcast(), done_button.clone().upcast()],
        progress,
        done_button: done_button.clone(),
        sched_button: RefCell::new(None),
        editing: Cell::new(None),
        cal_editing: Cell::new(None),
        view: Cell::new(View::Day),
        caiet: caiet.clone(),
        today_button,
        migrate_button: migrate.clone(),
        nav: nav.clone(),
        nav_rows: rows.clone(),
        settings: RefCell::new(settings),
        tray: RefCell::new(None),
        bara: bara.clone(),
        cautare: cautare.clone(),
        content_stack,
        searching: Cell::new(false),
        index_add_date: Cell::new(None),
        index_add_open: Cell::new(false),
        selected: Cell::new(None),
    });
    if state.settings.borrow().tray {
        state.set_tray(true);
    }

    let a = gio::SimpleAction::new("preferences", None);
    let weak = Rc::downgrade(&state);
    a.connect_activate(move |_, _| {
        if let Some(s) = weak.upgrade() {
            s.show_preferences();
        }
    });
    window.add_action(&a);
    app.set_accels_for_action("win.preferences", &["<Control>comma"]);
    cautare.connect(&state);
    let a = gio::SimpleAction::new("search", None);
    let weak = Rc::downgrade(&state);
    a.connect_activate(move |_, _| {
        if let Some(s) = weak.upgrade() {
            if s.searching.get() {
                s.close_search();
            } else {
                s.open_search();
            }
        }
    });
    window.add_action(&a);
    app.set_accels_for_action("win.search", &["<Control>f"]);
    // F11: fereastra mărită pe tot ecranul (cu bara de sus a sistemului) și înapoi.
    let a = gio::SimpleAction::new("maximize", None);
    let w = window.clone();
    a.connect_activate(move |_, _| {
        if w.is_maximized() {
            w.unmaximize();
        } else {
            w.maximize();
        }
    });
    window.add_action(&a);
    app.set_accels_for_action("win.maximize", &["F11"]);
    // Ctrl+I: „Adaugă în index”, cu ziua arătată propusă.
    let a = gio::SimpleAction::new("index-add", None);
    let weak = Rc::downgrade(&state);
    a.connect_activate(move |_, _| {
        if let Some(s) = weak.upgrade() {
            s.index_add_date.set(Some(s.shown.get()));
            s.index_add_open.set(true);
            s.go_index();
        }
    });
    window.add_action(&a);
    app.set_accels_for_action("win.index-add", &["<Control>i"]);
    let a = gio::SimpleAction::new("sidebar", None);
    let b = bara.clone();
    a.connect_activate(move |_, _| b.flip());
    window.add_action(&a);
    app.set_accels_for_action("win.sidebar", &["F9"]);
    let a = gio::SimpleAction::new("shortcuts", None);
    let weak = Rc::downgrade(&state);
    a.connect_activate(move |_, _| {
        if let Some(s) = weak.upgrade() {
            s.show_shortcuts();
        }
    });
    window.add_action(&a);
    app.set_accels_for_action("win.shortcuts", &["<Control>question"]);
    if app.lookup_action("quit").is_none() {
        let q = gio::SimpleAction::new("quit", None);
        let ga = app.clone();
        q.connect_activate(move |_, _| ga.quit());
        app.add_action(&q);
        app.set_accels_for_action("app.quit", &["<Control>q"]);
        app.set_accels_for_action("window.close", &["<Control>w"]);
    }

    for (bp_apply, narrow) in [(true, true), (false, false)] {
        let weak = Rc::downgrade(&state);
        let f = move |_: &adw::Breakpoint| {
            if let Some(s) = weak.upgrade() {
                s.narrow.set(narrow);
                s.layout();
            }
        };
        if bp_apply { one_page.connect_apply(f); } else { one_page.connect_unapply(f); }
    }
    window.add_breakpoint(one_page);

    // Migrarea
    let weak = Rc::downgrade(&state);
    later.connect_clicked(move |_| {
        if let Some(s) = weak.upgrade() {
            s.end_migration(false);
        }
    });
    let weak = Rc::downgrade(&state);
    done_button.connect_clicked(move |_| {
        if let Some(s) = weak.upgrade() {
            s.end_migration(true);
        }
    });
    let a = gio::SimpleAction::new("migrate", None);
    let weak = Rc::downgrade(&state);
    a.connect_activate(move |_, _| {
        if let Some(s) = weak.upgrade() {
            let source = match s.view.get() {
                View::Month(year, month) => PageRef::Month { year, month },
                View::Week(year, week) => PageRef::Week { year, week },
                View::Day | View::Future(..) | View::Index | View::Topic(..) => PageRef::Day(s.shown.get()),
            };
            s.start_migration(source, false);
        }
    });
    window.add_action(&a);
    app.set_accels_for_action("win.migrate", &["<Control>m"]);
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(&state);
    keys.connect_key_pressed(move |_, key, _, mods| match weak.upgrade() {
        Some(s) => s.migration_key(key, mods),
        None => glib::Propagation::Proceed,
    });
    window.add_controller(keys);
    // Tastele pentru intrări, după ce câmpurile și-au luat tastele lor.
    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(&state);
    keys.connect_key_pressed(move |_, key, _, mods| match weak.upgrade() {
        Some(s) => s.entry_key(key, mods),
        None => glib::Propagation::Proceed,
    });
    window.add_controller(keys);

    // Scrierea rapidă
    for (q, is_left) in [(&state.left.quick, true), (&state.right.quick, false)] {
        let Some(q) = q else { continue };
        let weak = Rc::downgrade(&state);
        q.connect_activate(move |e| {
            if let Some(s) = weak.upgrade() {
                s.quick_add(e, is_left);
            }
        });
    }

    // Navigarea: bara laterală, calendarul, săgețile
    let weak = Rc::downgrade(&state);
    nav.connect_row_activated(move |_, row| {
        let Some(s) = weak.upgrade() else { return };
        match row.index() {
            0 => s.go(s.today.get()),
            2 => {
                let t = s.today.get();
                s.go_month(t.year(), t.month());
            }
            1 => {
                let w = s.today.get().iso_week();
                s.go_week(w.year(), w.week());
            }
            3 => {
                let (y, m) = s.future_start();
                s.go_future(y, m);
            }
            5 => s.go_index(),
            _ => {}
        }
    });
    let weak = Rc::downgrade(&state);
    calendar.connect_day_selected(move |c| {
        let Some(s) = weak.upgrade() else { return };
        if s.calendar_busy.get() {
            return;
        }
        let dt = c.date();
        if let Some(d) = NaiveDate::from_ymd_opt(dt.year(), dt.month() as u32, dt.day_of_month() as u32) {
            s.go(d);
        }
    });
    for prop in ["month", "year"] {
        let weak = Rc::downgrade(&state);
        calendar.connect_notify_local(Some(prop), move |_, _| {
            if let Some(s) = weak.upgrade() {
                s.mark_calendar();
            }
        });
    }

    let actions = [
        ("prev", -1i64, ["<Alt>Left", "<Control>Page_Up"]),
        ("next", 1, ["<Alt>Right", "<Control>Page_Down"]),
    ];
    for (name, delta, accels) in actions {
        let a = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(&state);
        a.connect_activate(move |_, _| {
            if let Some(s) = weak.upgrade() {
                if matches!(s.view.get(), View::Month(..)) {
                    s.shift_month(delta as i32);
                } else if matches!(s.view.get(), View::Future(..)) {
                    s.shift_future(delta as i32);
                } else if matches!(s.view.get(), View::Week(..)) {
                    s.shift_week(delta as i32);
                } else {
                    s.go(s.shown.get() + Duration::days(delta));
                }
            }
        });
        window.add_action(&a);
        app.set_accels_for_action(&format!("win.{name}"), &accels);
    }
    let a = gio::SimpleAction::new("today", None);
    let weak = Rc::downgrade(&state);
    a.connect_activate(move |_, _| {
        if let Some(s) = weak.upgrade() {
            s.go(s.today.get());
            if let Some(q) = &s.right.quick {
                q.grab_focus();
            }
        }
    });
    window.add_action(&a);
    app.set_accels_for_action("win.today", &["<Alt>Home"]);

    let about = gio::SimpleAction::new("about", None);
    let weak = Rc::downgrade(&state);
    about.connect_activate(move |_, _| {
        let Some(s) = weak.upgrade() else { return };
        let d = adw::AboutDialog::builder()
            .application_name("Lanruj")
            .application_icon(APP_ID)
            .version(VERSION)
            .developer_name("ThinkRoot")
            .license_type(gtk::License::Gpl30)
            .comments(tr(
                s.lang,
                "Un jurnal cu cerneală, zi de zi. Inspirat de metoda Bullet Journal.",
                "An ink journal, day by day. Inspired by the Bullet Journal method.",
            ))
            .build();
        d.present(Some(&s.window));
    });
    app.add_action(&about);

    // La miezul nopții, „azi” trece la ziua următoare.
    let weak = Rc::downgrade(&state);
    glib::timeout_add_seconds_local(30, move || {
        let Some(s) = weak.upgrade() else { return glib::ControlFlow::Break };
        let t = today();
        if t != s.today.get() {
            let was_today = s.shown.get() == s.today.get();
            s.today.set(t);
            if was_today {
                s.shown.set(t);
            }
            s.refresh();
        }
        s.auto_migration();
        glib::ControlFlow::Continue
    });

    // Fereastra ține starea în viață; toate celelalte legături sunt slabe.
    // Fără asta, starea dispărea la ieșirea din build_ui și niciun buton nu mai mergea.
    // Cu iconița din systray pornită, închiderea doar ascunde fereastra.
    let keep = state.clone();
    window.connect_close_request(move |w| {
        // Starea barei (strânsă sau nu) se ține minte.
        keep.settings.borrow_mut().sidebar_compact = keep.bara.user_compact.get();
        let _ = keep.settings.borrow().save();
        if keep.tray.borrow().as_ref().is_some_and(|h| !h.is_closed()) {
            w.set_visible(false);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });

    state.refresh();
    window.present();
    let weak = Rc::downgrade(&state);
    glib::idle_add_local_once(move || {
        if let Some(s) = weak.upgrade() {
            s.auto_migration();
        }
    });
    // Doar la dezvoltare: pornește direct în ecranul lunii, pentru capturi.
    #[cfg(debug_assertions)]
    match std::env::var("LANRUJ_START").as_deref() {
        Ok("luna") => {
            let t = state.today.get();
            state.go_month(t.year(), t.month());
        }
        Ok("test-taste") => {
            let s = state.clone();
            glib::idle_add_local_once(move || {
                let q = s.right.quick.clone().expect("câmp");
                for t in [". Unu", ". Doi", "- Trei"] {
                    q.set_text(t);
                    q.emit_activate();
                }
                q.grab_focus();
                let none = gdk::ModifierType::empty();
                s.entry_key(gdk::Key::Up, none);
                eprintln!("selftest-taste: ↑ din câmp → {:?}", s.selected.get().map(|(_, i)| i));
                s.entry_key(gdk::Key::Up, none);
                eprintln!("selftest-taste: ↑ → {:?}", s.selected.get().map(|(_, i)| i));
                s.entry_key(gdk::Key::Return, gdk::ModifierType::CONTROL_MASK);
                s.entry_key(gdk::Key::Up, none);
                s.entry_key(gdk::Key::Delete, none);
                eprintln!("selftest-taste: după Delete, ales {:?}", s.selected.get().map(|(_, i)| i));
                s.entry_key(gdk::Key::Down, none);
                s.entry_key(gdk::Key::Return, none);
                eprintln!("selftest-taste: Enter → editare {:?}", s.editing.get().map(|(_, i)| i));
                s.finish_edit(None);
                s.entry_key(gdk::Key::Down, none);
                s.entry_key(gdk::Key::Down, none);
                eprintln!(
                    "selftest-taste: ↓ de pe ultima → ales {:?}, focus în câmp={}",
                    s.selected.get().map(|(_, i)| i),
                    gtk::prelude::GtkWindowExt::focus(&s.window).is_some_and(|w| w.is_ancestor(&q))
                );
                s.window.close();
            });
        }
        Ok("test-bara") => {
            let s = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(800), move || {
                eprintln!("selftest-bara: la pornire lățimea={} strânsă={}", s.bara.side.width(), s.bara.compact());
                gtk::prelude::WidgetExt::activate_action(&s.window, "win.sidebar", None).ok();
                let s2 = s.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                    eprintln!("selftest-bara: după F9 lățimea={} strânsă={}", s2.bara.side.width(), s2.bara.compact());
                    let (_, nat, _, _) = s2.bara.side.measure(gtk::Orientation::Horizontal, -1);
                    eprintln!("selftest-bara: strânsă, lățimea dorită={nat}");
                    if std::env::var("LANRUJ_BARA_RAMANE").is_err() {
                        gtk::prelude::WidgetExt::activate_action(&s2.window, "win.sidebar", None).ok();
                        let s3 = s2.clone();
                        glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                            eprintln!("selftest-bara: din nou F9 lățimea={} strânsă={}", s3.bara.side.width(), s3.bara.compact());
                        });
                    }
                });
            });
        }
        Ok("test-cautare") => {
            let s = state.clone();
            glib::idle_add_local_once(move || {
                gtk::prelude::WidgetExt::activate_action(&s.window, "win.search", None).ok();
                s.cautare.entry.set_text("sedinta");
                s.cautare.run(&s);
                eprintln!("selftest-cautare: titlu={} vizibil={:?}", s.title.title(), s.content_stack.visible_child_name());
                if let Ok(p) = std::env::var("LANRUJ_PNG") {
                    let w = s.window.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || save_png(&w, &p));
                }
                if std::env::var("LANRUJ_CAUTARE_RAMANE").is_err() {
                    let s2 = s.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                        // primul rezultat din lista (rândul 1, după titlul paginii)
                        let t = s2.cautare_first_target();
                        eprintln!("selftest-cautare: primul rezultat {:?}", t.map(|(r, i)| (format!("{r:?}"), i)));
                        if let Some((r, i)) = t {
                            s2.open_result(r, i);
                        }
                        eprintln!("selftest-cautare: după clic titlu={} ales={:?}", s2.title.title(), s2.selected.get().map(|(_, i)| i));
                        s2.window.close();
                    });
                }
            });
        }
        Ok("test-meniu") => {
            let s = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                fn find(w: &gtk::Widget) -> Option<gtk::MenuButton> {
                    if let Some(m) = w.downcast_ref::<gtk::MenuButton>()
                        && m.is_primary()
                    {
                        return Some(m.clone());
                    }
                    let mut c = w.first_child();
                    while let Some(ch) = c {
                        if let Some(m) = find(&ch) {
                            return Some(m);
                        }
                        c = ch.next_sibling();
                    }
                    None
                }
                if let Some(m) = find(s.window.upcast_ref()) {
                    m.popup();
                    if std::env::var("LANRUJ_F10").is_ok() {
                        let m2 = m.clone();
                        glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                            let pop = m2.popover().expect("meniul");
                            eprintln!("selftest-meniu: deschis={}", pop.is_visible());
                            for c in pop.observe_controllers().into_iter().flatten() {
                                if let Ok(k) = c.downcast::<gtk::EventControllerKey>() {
                                    let _: bool = k.emit_by_name("key-pressed", &[&gdk::Key::F10.into_glib(), &0u32, &gdk::ModifierType::empty()]);
                                }
                            }
                            eprintln!("selftest-meniu: după F10 deschis={}", pop.is_visible());
                        });
                    }
                }
                if let Ok(p) = std::env::var("LANRUJ_PNG") {
                    let w = s.window.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
                        let path = p;
                        // Meniul e o fereastră separată (popover): capturăm popover-ul.
                        fn pop(w: &gtk::Widget) -> Option<gtk::Widget> {
                            if w.is::<gtk::PopoverMenu>() && w.is_visible() {
                                return Some(w.clone());
                            }
                            let mut c = w.first_child();
                            while let Some(ch) = c {
                                if let Some(p) = pop(&ch) {
                                    return Some(p);
                                }
                                c = ch.next_sibling();
                            }
                            None
                        }
                        match pop(w.upcast_ref()) {
                            Some(m) => save_png(&m, &path),
                            None => eprintln!("selftest-meniu: nu găsesc meniul deschis"),
                        }
                    });
                }
            });
        }
        Ok("test-f11") => {
            let w = state.window.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                eprintln!("selftest-f11: la început mărită={}", w.is_maximized());
                gtk::prelude::WidgetExt::activate_action(&w, "win.maximize", None).ok();
                let w2 = w.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                    eprintln!("selftest-f11: după F11 mărită={}", w2.is_maximized());
                    gtk::prelude::WidgetExt::activate_action(&w2, "win.maximize", None).ok();
                    let w3 = w2.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                        eprintln!("selftest-f11: din nou F11 mărită={}", w3.is_maximized());
                    });
                });
            });
        }
        Ok("captura") => {
            // Doar o captură (LANRUJ_PNG), apoi ieșire; LANRUJ_ZI=2026-11-24 deschide întâi ziua,
            // LANRUJ_SAPT=1 apoi săptămâna ei.
            if let Ok(z) = std::env::var("LANRUJ_ZI")
                && let Ok(d) = NaiveDate::parse_from_str(&z, "%Y-%m-%d")
            {
                state.go(d);
                if std::env::var("LANRUJ_SAPT").is_ok() {
                    state.go_week(d.iso_week().year(), d.iso_week().week());
                }
            }
            if let Ok(p) = std::env::var("LANRUJ_PNG") {
                let w = state.window.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
                    save_png(&w, &p);
                    w.close();
                });
            }
        }
        Ok("test-index") => {
            let s = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
                gtk::prelude::WidgetExt::activate_action(&s.window, "win.index-add", None).ok();
                let s2 = s.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                    fn find(w: &gtk::Widget) -> Option<gtk::Entry> {
                        if let Some(e) = w.downcast_ref::<gtk::Entry>()
                            && e.has_css_class("index-form-titlu")
                        {
                            return Some(e.clone());
                        }
                        let mut c = w.first_child();
                        while let Some(ch) = c {
                            if let Some(e) = find(&ch) {
                                return Some(e);
                            }
                            c = ch.next_sibling();
                        }
                        None
                    }
                    match find(s2.left.root.upcast_ref()) {
                        Some(e) => {
                            e.set_text("Prezentarea proiectului");
                            e.emit_activate();
                            let s4 = s2.clone();
                            glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                                // Clic pe primul rând: deschide pagina lui (creată acum); scrie ceva în ea.
                                s4.open_index_item(0);
                                let q = s4.left.quick.clone().expect("câmp");
                                q.set_text("- Cazare la Hotel Delta");
                                q.emit_activate();
                                eprintln!("selftest-index: pagina: {} / {}", s4.title.title(), s4.title.subtitle());
                                s4.index_edit(0, "Concediu la mare", NaiveDate::from_ymd_opt(2026, 11, 23).unwrap(), NaiveDate::from_ymd_opt(2026, 11, 30));
                                if let Ok(p) = std::env::var("LANRUJ_PNG3") {
                                    let w = s4.window.clone();
                                    glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || save_png(&w, &p));
                                }
                            });
                        }
                        None => eprintln!("selftest-index: nu găsesc formularul"),
                    }
                    let s3 = s2.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                        eprintln!("selftest-index: titlu={} / {}", s3.title.title(), s3.title.subtitle());
                        eprintln!("selftest-index: fișier:\n{}", std::fs::read_to_string(s3.journal.index_path()).unwrap_or_default());
                        let r = s3.journal.search("sanatate", lanruj_core::Filter::default(), 50);
                        eprintln!("selftest-index: căutare „sanatate”: {} pagini", r.len());
                        if let Ok(p) = std::env::var("LANRUJ_PNG") {
                            save_png(&s3.window, &p);
                        }
                    });
                });
            });
        }
        Ok("test-culori") => {
            let s = state.clone();
            glib::idle_add_local_once(move || {
                let q = s.right.quick.clone().expect("câmp");
                for t in [". Sună la service pentru mașină", "- Prima zăpadă", "o 18:00 Cină cu familia"] {
                    q.set_text(t);
                    q.emit_activate();
                }
                let date = s.shown.get();
                let r = PageRef::Day(date);
                let day = s.journal.load(date).expect("zi");
                let base = day.items.len() - 3;
                // 1. din câmpul de editare: „Sună” în roșu, „mașină” cu marker galben
                s.start_edit(r, base);
                let s2 = s.clone();
                glib::idle_add_local_once(move || {
                    fn find<T: IsA<gtk::Widget>>(w: &gtk::Widget, class: &str) -> Vec<T> {
                        let mut out = Vec::new();
                        if w.has_css_class(class) && let Ok(t) = w.clone().downcast::<T>() {
                            out.push(t);
                        }
                        let mut c = w.first_child();
                        while let Some(ch) = c {
                            out.extend(find::<T>(&ch, class));
                            c = ch.next_sibling();
                        }
                        out
                    }
                    let root = s2.right.root.clone().upcast::<gtk::Widget>();
                    let v: gtk::TextView = find::<gtk::TextView>(&root, "editare").pop().expect("câmpul de editare");
                    let buf = v.buffer();
                    // „Sună” (după „. ”) în roșu
                    buf.select_range(&buf.iter_at_offset(2), &buf.iter_at_offset(6));
                    find::<gtk::Button>(&root, "text-c01c28")[0].emit_clicked();
                    let t = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
                    let start = t.find("mașină").map(|b| t[..b].chars().count() as i32).expect("mașină");
                    buf.select_range(&buf.iter_at_offset(start), &buf.iter_at_offset(start + 6));
                    find::<gtk::Button>(&root, "mark-f9f06b")[0].emit_clicked();
                    eprintln!("selftest-culori: câmpul arată: {}", buf.text(&buf.start_iter(), &buf.end_iter(), false));
                    eprintln!("selftest-culori: se salvează: {}", page::serialize_colored(&buf));
                    s2.finish_edit(Some(page::serialize_colored(&buf)));
                    let s3 = s2.clone();
                    glib::idle_add_local_once(move || {
                        // 2. toată intrarea, din meniu: nota în verde, evenimentul cu marker albastru
                        s3.color_entry(r, base + 1, Some((false, "#26a269".into())));
                        s3.color_entry(r, base + 2, Some((true, "#99c1f1".into())));
                        let text = std::fs::read_to_string(s3.journal.day_path(date)).unwrap_or_default();
                        for l in text.lines().filter(|l| l.contains("<") || l.contains("==")) {
                            eprintln!("selftest-culori: fișier: {l}");
                        }
                        if let Ok(p) = std::env::var("LANRUJ_PNG") {
                            let w = s3.window.clone();
                            glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || save_png(&w, &p));
                        }
                        // Încă o captură, cu bara de culori deschisă pe prima intrare.
                        if let Ok(p2) = std::env::var("LANRUJ_PNG2") {
                            let s4 = s3.clone();
                            glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
                                s4.start_edit(r, base);
                                let w = s4.window.clone();
                                glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || save_png(&w, &p2));
                            });
                        }
                    });
                });
            });
        }
        Ok("taste") => {
            let w = state.window.clone();
            glib::idle_add_local_once(move || {
                gtk::prelude::WidgetExt::activate_action(&w, "win.shortcuts", None).ok();
            });
        }
        Ok("tava") => {
            // Fără systray (de exemplu într-o sesiune D-Bus separată): setarea trebuie să rămână oprită.
            let ok = state.set_tray(true);
            eprintln!("selftest-tava: pornită={ok}, iconiță={}", state.tray.borrow().is_some());
            state.window.close();
            let st = state.clone();
            glib::idle_add_local_once(move || {
                eprintln!("selftest-tava: după închidere, fereastra vizibilă={}", st.window.is_visible());
                st.set_tray(false);
                if let Some(a) = st.window.application() {
                    a.quit();
                }
            });
        }
        Ok("setari") => {
            let w = state.window.clone();
            glib::idle_add_local_once(move || {
                gtk::prelude::WidgetExt::activate_action(&w, "win.preferences", None).ok();
            });
        }
        Ok("test-setari") => {
            // Schimbă hârtia și limba din dialogul de setări, ca un clic adevărat.
            gtk::prelude::WidgetExt::activate_action(&state.window, "win.preferences", None).ok();
            let w = state.window.clone();
            glib::idle_add_local_once(move || {
                fn rows(w: &gtk::Widget, out: &mut Vec<adw::ComboRow>) {
                    if let Some(r) = w.downcast_ref::<adw::ComboRow>() {
                        out.push(r.clone());
                    }
                    let mut c = w.first_child();
                    while let Some(ch) = c {
                        rows(&ch, out);
                        c = ch.next_sibling();
                    }
                }
                let mut v = Vec::new();
                rows(w.upcast_ref(), &mut v);
                eprintln!("selftest-setari: {} rânduri cu variante", v.len());
                if v.len() >= 4 {
                    v[0].set_selected(2); // Dungi
                    eprintln!("selftest-setari: clase după „Dungi”: {:?}", w.css_classes().iter().filter(|c| c.starts_with("hartie")).collect::<Vec<_>>());
                    v[3].set_selected(2); // English
                    let app = w.application().expect("aplicația");
                    glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                        let nw = app.active_window().expect("fereastra nouă");
                        eprintln!("selftest-setari: după limbă, fereastra nouă: {} (aceeași={})", nw.title().unwrap_or_default(), nw == w.upcast::<gtk::Window>());
                        fn labels(w: &gtk::Widget, out: &mut Vec<String>) {
                            if let Some(l) = w.downcast_ref::<gtk::Label>() {
                                out.push(l.label().to_string());
                            }
                            let mut c = w.first_child();
                            while let Some(ch) = c {
                                labels(&ch, out);
                                c = ch.next_sibling();
                            }
                        }
                        let mut ls = Vec::new();
                        labels(nw.upcast_ref(), &mut ls);
                        eprintln!("selftest-setari: „Week” în bara laterală: {}", ls.iter().any(|l| l == "Week"));
                        nw.close();
                    });
                }
            });
        }
        Ok("saptamana") => {
            let w = state.today.get().iso_week();
            state.go_week(w.year(), w.week());
        }
        Ok("viitor") => {
            let (y, m) = state.future_start();
            state.go_future(y, m);
        }
        _ => {}
    }
    #[cfg(debug_assertions)]
    if std::env::var_os("LANRUJ_SELFTEST").is_some() {
        // După ce build_ui s-a terminat, ca la un clic adevărat.
        let weak = Rc::downgrade(&state);
        glib::idle_add_local_once(move || match weak.upgrade() {
            Some(s) => selftest(&s),
            None => {
                eprintln!("selftest: EȘUAT, starea aplicației s-a pierdut");
                std::process::exit(1);
            }
        });
    }
    if let Some(q) = &state.right.quick {
        q.grab_focus();
    }
}

/// Doar la dezvoltare: apasă singură câteva butoane, ca să verificăm legăturile
/// dintre fereastră și nucleu fără mouse (LANRUJ_SELFTEST=1, cu LANRUJ_DIR de probă).
#[cfg(debug_assertions)]
fn selftest(s: &Rc<App>) {
    fn find_buttons(w: &gtk::Widget, class: &str, out: &mut Vec<gtk::Button>) {
        if let Some(b) = w.downcast_ref::<gtk::Button>()
            && b.has_css_class(class) {
                out.push(b.clone());
            }
        let mut c = w.first_child();
        while let Some(ch) = c {
            find_buttons(&ch, class, out);
            c = ch.next_sibling();
        }
    }
    let click = |root: &gtk::Widget, class: &str, nth: usize| {
        let mut v = Vec::new();
        find_buttons(root, class, &mut v);
        match v.get(nth) {
            Some(b) => b.emit_clicked(),
            None => eprintln!("selftest: nu găsesc butonul .{class}"),
        }
    };
    let q = s.right.quick.clone().expect("câmpul rapid");
    for t in [". Becuri", ". * Raport", ". Lampă", "o 9:30 Dentist"] {
        q.set_text(t);
        q.emit_activate();
    }
    click(s.right.root.upcast_ref(), "punct", 3);
    click(s.right.root.upcast_ref(), "semn", 2); // Lampă: făcută
    click(s.right.root.upcast_ref(), "semn", 2); // și înapoi
    // Migrarea zilei arătate
    gtk::prelude::WidgetExt::activate_action(&s.window, "win.migrate", None).ok();
    let left = s.left.root.clone().upcast::<gtk::Widget>();
    click(&left, "opt-maine", 0); // Becuri -> mâine
    click(&left, "opt-renunt", 0); // Raport -> renunț
    click(&left, "prog-luna", 0); // Lampă -> în lista lunii (Programează)
    // Răzgândire: Becuri rămâne totuși azi (copia de mâine trebuie să dispară)
    s.mig_select(0);
    click(&left, "opt-las", 0);
    s.mig_select(0);
    click(&left, "opt-maine", 0);
    eprintln!("selftest: titlu în migrare={} / {}", s.title.title(), s.title.subtitle());
    s.done_button.emit_clicked();
    eprintln!("selftest: după migrare, în migrare={} titlu={}", s.migrating(), s.title.title());
    // Editarea: „o 9:30 Dentist” devine sarcina „. Dentist” (poziția 3 în zi).
    let date = s.shown.get();
    s.start_edit(PageRef::Day(date), 3);
    let s2 = s.clone();
    glib::idle_add_local_once(move || {
        fn find_entry(w: &gtk::Widget) -> Option<gtk::TextView> {
            if let Some(e) = w.downcast_ref::<gtk::TextView>()
                && e.has_css_class("editare")
            {
                return Some(e.clone());
            }
            let mut c = w.first_child();
            while let Some(ch) = c {
                if let Some(e) = find_entry(&ch) {
                    return Some(e);
                }
                c = ch.next_sibling();
            }
            None
        }
        match find_entry(s2.right.root.upcast_ref()) {
            Some(v) => {
                let buf = v.buffer();
                eprintln!("selftest: câmpul de editare are „{}”", buf.text(&buf.start_iter(), &buf.end_iter(), false));
                buf.set_text(". Dentist");
                s2.finish_edit(Some(page::serialize_colored(&buf)));
            }
            None => eprintln!("selftest: EȘUAT, nu apare câmpul de editare"),
        }
        let s3 = s2.clone();
        glib::idle_add_local_once(move || {
            // Ștergerea primei intrări (Becuri), apoi „Anulează” din notificare nu se poate apăsa de aici;
            // verificăm doar ștergerea.
            s3.delete_entry(PageRef::Day(date), 0);
            eprintln!("selftest: gata, editare={:?}", s3.editing.get());
            // Luna: sarcină nouă, bifată; un rând în calendar; butonul Azi
            s3.go_month(date.year(), date.month());
            let q = s3.right.quick.clone().expect("câmp");
            q.set_text(". Plătește impozitul");
            q.emit_activate();
            q.set_text(". * Revizia mașinii");
            q.emit_activate();
            s3.toggle(PageRef::Month { year: date.year(), month: date.month() }, 0);
            s3.start_cal_edit(15);
            s3.finish_cal_edit(Some("Concert; Ziua lui Andrei".into()));
            eprintln!(
                "selftest: luna titlu={} / {} · butonul Azi vizibil={} · Migrează vizibil={}",
                s3.title.title(),
                s3.title.subtitle(),
                s3.today_button.is_visible(),
                s3.migrate_button.is_visible()
            );
            // Migrarea lunii: sarcinile neterminate din noiembrie trec în decembrie.
            gtk::prelude::WidgetExt::activate_action(&s3.window, "win.migrate", None).ok();
            eprintln!("selftest: migrarea lunii: {} / {}", s3.title.title(), s3.title.subtitle());
            let left = s3.left.root.clone().upcast::<gtk::Widget>();
            click(&left, "opt-maine", 0);
            click(&left, "opt-maine", 0);
            s3.done_button.emit_clicked();
            eprintln!("selftest: după migrarea lunii, în migrare={} titlu={}", s3.migrating(), s3.title.title());
            gtk::prelude::WidgetExt::activate_action(&s3.window, "win.next", None).ok();
            eprintln!("selftest: după „înainte” titlu={}", s3.title.title());
            // Pagina săptămânii: o sarcină în plan, apoi migrarea săptămânii spre cea următoare.
            s3.nav.emit_by_name::<()>("row-activated", &[&s3.nav_rows[1]]);
            eprintln!("selftest: săptămâna {} / {}", s3.title.title(), s3.title.subtitle());
            let q = s3.left.quick.clone().expect("câmp");
            q.set_text(". Curățenie în debara");
            q.emit_activate();
            gtk::prelude::WidgetExt::activate_action(&s3.window, "win.migrate", None).ok();
            eprintln!("selftest: migrarea săptămânii: {} / {}", s3.title.title(), s3.title.subtitle());
            click(&s3.left.root.clone().upcast::<gtk::Widget>(), "opt-maine", 0);
            s3.done_button.emit_clicked();
            eprintln!("selftest: după migrarea săptămânii titlu={}", s3.title.title());
            // Jurnalul viitor: câte o intrare pe fiecare pagină, apoi următoarele două luni.
            s3.nav.emit_by_name::<()>("row-activated", &[&s3.nav_rows[3]]);
            eprintln!("selftest: viitor {} / {}", s3.title.title(), s3.title.subtitle());
            for (q, t) in [(&s3.left.quick, ". 15 Revizia mașinii"), (&s3.right.quick, "o 7 Ziua lui Ion")] {
                let q = q.clone().expect("câmp");
                q.set_text(t);
                q.emit_activate();
            }
            // „Mută…” din meniul primei intrări din stânga: în lista lunii aceasteia.
            click(&s3.left.root.clone().upcast::<gtk::Widget>(), "muta", 0);
            click(&s3.left.root.clone().upcast::<gtk::Widget>(), "muta-spre", 4); // Azi, Mâine, 2 săptămâni, apoi luna aceasta
            let s3 = s3.clone();
            glib::idle_add_local_once(move || {
                let nov = s3.journal.load_month(2026, 11).map(|m| m.tasks.len()).unwrap_or(0);
                let dec = s3.journal.load_ref(PageRef::Future { year: 2026, month: 12 }).map(|d| d.entries().filter(|e| e.is_open_task()).count()).unwrap_or(9);
                eprintln!("selftest: după „Mută”: sarcini în lista lui noiembrie={nov}, sarcini deschise în planuri/decembrie={dec}");
                gtk::prelude::WidgetExt::activate_action(&s3.window, "win.next", None).ok();
                eprintln!("selftest: viitor după „înainte” {}", s3.title.subtitle());
            gtk::prelude::WidgetExt::activate_action(&s3.window, "win.prev", None).ok();
            gtk::prelude::WidgetExt::activate_action(&s3.window, "win.prev", None).ok();
            eprintln!("selftest: viitor nu trece înapoi de luna viitoare: {}", s3.title.subtitle());
            gtk::prelude::WidgetExt::activate_action(&s3.window, "win.today", None).ok();
            eprintln!("selftest: după Azi titlu={} butonul Azi vizibil={}", s3.title.title(), s3.today_button.is_visible());
            s3.window.close();
            });
        });
    });
}

/// Doar la dezvoltare: salvează fereastra ca imagine PNG (pentru probe fără ecran).
#[cfg(debug_assertions)]
pub fn save_png(w: &impl IsA<gtk::Widget>, path: &str) {
    let w = w.as_ref();
    let paintable = gtk::WidgetPaintable::new(Some(w));
    let (width, height) = (w.width() as f64, w.height() as f64);
    let snap = gtk::Snapshot::new();
    paintable.snapshot(&snap, width, height);
    let Some(node) = snap.to_node() else { return eprintln!("save_png: nimic de desenat") };
    let Some(native) = w.native() else { return };
    let Some(renderer) = native.renderer() else { return };
    let tex = renderer.render_texture(&node, None);
    match tex.save_to_png(path) {
        Ok(()) => eprintln!("save_png: {path}"),
        Err(e) => eprintln!("save_png: {e}"),
    }
}

fn main() -> glib::ExitCode {
    // Cu un jurnal de probă (LANRUJ_DIR), aplicația rulează separat de cea deschisă,
    // ca probele să nu ajungă în fereastra jurnalului adevărat.
    let flags = if std::env::var_os("LANRUJ_DIR").is_some_and(|d| !d.is_empty()) {
        gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::empty()
    };
    let app = adw::Application::builder().application_id(APP_ID).flags(flags).build();
    app.connect_startup(|_| load_css());
    app.connect_activate(build_ui);
    app.run()
}
