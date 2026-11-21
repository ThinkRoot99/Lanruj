// SPDX-License-Identifier: GPL-3.0-or-later
//! Lanruj: jurnal în stilul Bullet Journal, pe fișiere Markdown.

mod bara;
mod cautare;
mod cheie;
mod incuiere;
mod vechi;
mod copii;
mod export;
mod ghid;
mod migrare;
mod page;
mod poze;
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
    pub shown: Cell<NaiveDate>,
    pub today: Cell<NaiveDate>,
    window: adw::ApplicationWindow,
    pub left: Page,
    pub right: Page,
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
    /// În timpul exportului: fără selecție, câmpuri de scris și butoane de adăugat.
    pub exporting: Cell<bool>,
    /// Schema Vim: prima tastă dintr-o pereche (dd, gg, >>, <<).
    vim_pending: Cell<Option<char>>,
    /// Ziua din calendarul lunii care se editează acum.
    pub cal_editing: Cell<Option<u32>>,
    pub view: Cell<View>,
    pub caiet: gtk::Box,
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
        let narrow = self.narrow.get() || self.settings.borrow().one_page;
        let month_stacked = narrow && matches!(self.view.get(), View::Month(..) | View::Future(..) | View::Week(..) | View::Index | View::Topic(..)) && !self.migrating();
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
        let (show_left, show_right) = match (narrow, self.migrating()) {
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

    /// Alege poze și fișiere (agrafa din câmpul de jos) și le pune în pagina câmpului.
    pub fn pick_files(self: &Rc<Self>, left: bool) {
        let Some(r) = self.quick_ref(left) else { return };
        let dialog = gtk::FileDialog::builder().title(tr(self.lang, "Poze și fișiere", "Pictures and files")).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.open_multiple(Some(&self.window), gtk::gio::Cancellable::NONE, move |res| {
            let Some(s) = weak.upgrade() else { return };
            let Ok(list) = res else { return };
            let files: Vec<std::path::PathBuf> = (0..list.n_items())
                .filter_map(|i| list.item(i).and_downcast::<gtk::gio::File>())
                .filter_map(|f| f.path())
                .collect();
            s.attach_files(r, files);
        });
    }

    /// Alege poze și fișiere pentru o intrare anume (se pun sub ea).
    pub fn pick_files_under(self: &Rc<Self>, r: PageRef, index: usize) {
        let dialog = gtk::FileDialog::builder().title(tr(self.lang, "Poze și fișiere", "Pictures and files")).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.open_multiple(Some(&self.window), gtk::gio::Cancellable::NONE, move |res| {
            let Some(s) = weak.upgrade() else { return };
            let Ok(list) = res else { return };
            let files: Vec<std::path::PathBuf> = (0..list.n_items())
                .filter_map(|i| list.item(i).and_downcast::<gtk::gio::File>())
                .filter_map(|f| f.path())
                .collect();
            s.attach_files_under(r, files, Some(index));
        });
    }

    pub fn window_widget(&self) -> gtk::Widget {
        self.window.clone().upcast()
    }

    pub fn set_attachment(self: &Rc<Self>, r: PageRef, index: usize, a: lanruj_core::attach::Attachment) {
        if let Err(e) = self.journal.set_attachment(r, index, a, self.lang) {
            self.error(&e);
        }
        self.refresh();
    }

    pub fn move_attachment(self: &Rc<Self>, r: PageRef, index: usize, under: Option<usize>) {
        if let Err(e) = self.journal.move_attachment(r, index, under, self.lang) {
            self.error(&e);
        }
        self.refresh();
    }

    /// Copiază fișierele lângă pagină (pozele mari micșorate) și le adaugă la sfârșitul ei.
    pub fn attach_files(self: &Rc<Self>, r: PageRef, files: Vec<std::path::PathBuf>) {
        self.attach_files_under(r, files, None);
    }

    /// Ca `attach_files`, dar sub intrarea `under`, dacă e dată.
    pub fn attach_files_under(self: &Rc<Self>, r: PageRef, files: Vec<std::path::PathBuf>, under: Option<usize>) {
        use lanruj_core::attach::is_image;
        let mut added = 0;
        for src in files {
            if !src.is_file() {
                continue;
            }
            let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("fisier").to_string();
            let res = self.journal.attach_target(r, &name).and_then(|(abs, rel)| {
                // Pozele mari se micșorează; totul se scrie prin jurnal (criptat, dacă e cazul).
                let data = match is_image(&name).then(|| resized_image(&src, &abs)).flatten() {
                    Some(d) => d,
                    None => std::fs::read(&src)?,
                };
                self.journal.write_bytes(&abs, &data)?;
                self.journal.add_attachment_line(r, &rel, &name, self.lang, under)
            });
            match res {
                Ok(()) => added += 1,
                Err(e) => self.error(&e),
            }
        }
        if added > 0 {
            let msg = match self.lang {
                Lang::Ro if added == 1 => "Am adăugat un fișier.".to_string(),
                Lang::Ro => format!("Am adăugat {added} fișiere."),
                Lang::En => format!("Added {added} file(s)."),
            };
            self.toast(&msg);
        }
        self.refresh();
    }

    /// Scoate din pagină rândul unei poze sau al unui fișier (fișierul rămâne pe disc).
    pub fn remove_line(self: &Rc<Self>, r: PageRef, index: usize) {
        let res = self.journal.update_ref(r, self.lang, |d| {
            if index < d.items.len() && matches!(d.items[index], lanruj_core::Item::Other(_)) {
                d.items.remove(index);
            }
        });
        if let Err(e) = res {
            self.error(&e);
        }
        self.refresh();
    }

    /// Tab / Shift+Tab: intrarea trece sub cea de deasupra (notiță a ei) sau iese înapoi.
    pub fn indent_entry(self: &Rc<Self>, r: PageRef, index: usize, right: bool) {
        let mut ok = false;
        if let Err(e) = self.journal.update_ref(r, self.lang, |d| ok = d.indent(index, right)) {
            self.error(&e);
        }
        if !ok && right {
            self.toast(tr(self.lang, "Intrarea trece sub cea de deasupra ei; prima intrare n-are sub ce sta.", "The entry goes under the one above it; the first entry has nothing to go under."));
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

    /// Pagina pe care scrie câmpul de jos (cel din stânga sau cel din dreapta).
    fn quick_ref(&self, left: bool) -> Option<PageRef> {
        // În migrare, câmpul de jos scrie pe ziua din dreapta (mâine).
        Some(match (self.mig.borrow().as_ref().map(|m| m.target), self.view.get()) {
            (Some(target), _) => target,
            (None, View::Month(year, month)) => PageRef::Month { year, month },
            (None, View::Future(year, month)) => {
                let (year, month) = if left { (year, month) } else { lanruj_core::migrate::next_month(year, month) };
                PageRef::Future { year, month }
            }
            (None, View::Week(year, week)) => PageRef::Week { year, week },
            (None, View::Index) => return None,
            (None, View::Topic(date, n)) => PageRef::Topic { date, n },
            (None, View::Day) => PageRef::Day(self.shown.get()),
        })
    }

    fn quick_add(self: &Rc<Self>, entry: &gtk::Entry, left: bool) {
        let Some(e) = parse_quick(&entry.text()) else { return };
        let Some(r) = self.quick_ref(left) else { return };
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
        if self.settings.borrow().vim
            && !ctrl
            && !mods.contains(gdk::ModifierType::ALT_MASK)
            && let Some(p) = self.vim_key(key, r, in_quick, &entries, sel, pos)
        {
            return p;
        }
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
            gdk::Key::Tab | gdk::Key::ISO_Left_Tab if pos.is_some() && !in_quick => {
                let right = key == gdk::Key::Tab && !mods.contains(gdk::ModifierType::SHIFT_MASK);
                self.indent_entry(r, sel.expect("verificat"), right);
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

    /// Schema Vim, în afara câmpurilor de scris („modul normal”).
    /// `None` = tasta nu e a ei și merge mai departe la tastele obișnuite.
    fn vim_key(self: &Rc<Self>, key: gdk::Key, r: PageRef, in_quick: bool, entries: &[usize], sel: Option<usize>, pos: Option<usize>) -> Option<glib::Propagation> {
        let stop = Some(glib::Propagation::Stop);
        let select = |i: Option<usize>| {
            if let Some(i) = i {
                self.selected.set(Some((r, i)));
                gtk::prelude::GtkWindowExt::set_focus(&self.window, None::<&gtk::Widget>);
                self.refresh();
            }
        };
        // Esc în câmpul de scris: înapoi în modul normal, pe ultima intrare.
        if in_quick {
            if key == gdk::Key::Escape {
                self.vim_pending.set(None);
                gtk::prelude::GtkWindowExt::set_focus(&self.window, None::<&gtk::Widget>);
                select(entries.last().copied());
                return stop;
            }
            return None;
        }
        let c = key.to_unicode()?;
        let prev = self.vim_pending.take();
        let action = |name: &str| {
            gtk::prelude::WidgetExt::activate_action(&self.window, name, None).ok();
        };
        match (prev, c) {
            (_, 'j') => select(match pos {
                Some(p) => entries.get(p + 1).or(entries.get(p)).copied(),
                None => entries.first().copied(),
            }),
            (_, 'k') => select(match pos {
                Some(p) => entries.get(p.saturating_sub(1)).copied(),
                None => entries.last().copied(),
            }),
            (Some('g'), 'g') => select(entries.first().copied()),
            (_, 'G') => select(entries.last().copied()),
            (_, 'h') => action("win.prev"),
            (_, 'l') => action("win.next"),
            (_, 't') => self.go(self.today.get()),
            (_, '/') => action("win.search"),
            (_, 'x') if sel.is_some() => self.toggle(r, sel.expect("verificat")),
            (_, 'i' | 'a' | 'c') if sel.is_some() => self.start_edit(r, sel.expect("verificat")),
            (_, 'i' | 'a' | 'o') => {
                self.selected.set(None);
                self.refresh();
                if let Some((_, page)) = self.key_page()
                    && let Some(q) = &page.quick
                {
                    q.grab_focus();
                }
            }
            (Some('d'), 'd') if sel.is_some() => {
                return Some(self.entry_key(gdk::Key::Delete, gdk::ModifierType::empty()));
            }
            (Some('>'), '>') if sel.is_some() => self.indent_entry(r, sel.expect("verificat"), true),
            (Some('<'), '<') if sel.is_some() => self.indent_entry(r, sel.expect("verificat"), false),
            (_, 'g' | 'd' | '>' | '<') => self.vim_pending.set(Some(c)),
            _ => return None,
        }
        stop
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
                    (tr(l, "Ghidul", "Guide"), "F1"),
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
                    (tr(l, "Mergi la o dată", "Go to a date"), "<Control>g"),
                    (tr(l, "Tipărește pagina", "Print the page"), "<Control>p"),
                    (tr(l, "Pagina ca PDF", "Page as PDF"), "<Control><Shift>e"),
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
                    (tr(l, "Pune intrarea aleasă sub cea de deasupra / scoate-o", "Indent / outdent the selected entry"), "Tab <Shift>Tab"),
                    (tr(l, "Înapoi în câmpul de scriere", "Back to the writing field"), "Escape"),
                    (tr(l, "Legătură pe textul selectat (la editare)", "Link on the selected text (while editing)"), "<Control>k"),
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
        // Schema Vim, când e pornită în setări.
        let title = if self.settings.borrow().vim { tr(l, "Vim (pornit)", "Vim (on)") } else { tr(l, "Vim (se pornește din Setări)", "Vim (turn it on in Preferences)") };
        let sec = adw::ShortcutsSection::new(Some(title));
        for (what, accel) in [
            (tr(l, "Intrarea de dedesubt / de deasupra", "Entry below / above"), "j k"),
            (tr(l, "Prima / ultima intrare", "First / last entry"), "g&g <Shift>g"),
            (tr(l, "Pagina de dinainte / de după", "Previous / next page"), "h l"),
            (tr(l, "Azi", "Today"), "t"),
            (tr(l, "Bifează sau debifează", "Check or uncheck"), "x"),
            (tr(l, "Editează intrarea aleasă", "Edit the selected entry"), "i"),
            (tr(l, "Scrie o intrare nouă", "Write a new entry"), "o"),
            (tr(l, "Șterge intrarea aleasă", "Delete the selected entry"), "d&d"),
            (tr(l, "Sub cea de deasupra / scoate-o", "Indent / outdent"), "greater&greater less&less"),
            (tr(l, "Caută", "Search"), "slash"),
            (tr(l, "Ieși din câmpul de scris", "Leave the writing field"), "Escape"),
        ] {
            sec.add(adw::ShortcutsItem::new(what, accel));
        }
        dialog.add(sec);
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
        set_text_size(s.text);
        self.layout();
        adw::StyleManager::default().set_color_scheme(match s.scheme {
            setari::Scheme::System => adw::ColorScheme::Default,
            setari::Scheme::Light => adw::ColorScheme::ForceLight,
            setari::Scheme::Dark => adw::ColorScheme::ForceDark,
        });
        if let Err(e) = s.save() {
            self.error(&e);
        }
    }

    /// Copia de siguranță: la pornire doar dacă e vremea ei; `now` = cerută din setări.
    /// Arhiva se face pe alt fir, ca fereastra să nu aștepte.
    pub fn backup(self: &Rc<Self>, now: bool) {
        let s = self.settings.borrow().clone();
        let (dir, today) = (s.backup_folder(), self.today.get());
        if !now && !copii::due(&dir, s.backup, today) {
            return;
        }
        let root = self.journal.root().to_path_buf();
        let keep = s.backup_keep;
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let res = gio::spawn_blocking(move || copii::make(&root, &dir, today, keep)).await;
            let Some(app) = weak.upgrade() else { return };
            match res {
                Ok(Ok(p)) if now => {
                    let msg = match app.lang {
                        Lang::Ro => format!("Copia e gata: {}", p.display()),
                        Lang::En => format!("Backup done: {}", p.display()),
                    };
                    app.toast(&msg);
                }
                Ok(Ok(_)) => {}
                Ok(Err(e)) => app.error(&e),
                Err(_) => app.error(&std::io::Error::other("copia de siguranță s-a oprit")),
            }
        });
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

        // Pagina și textul.
        let pages = adw::PreferencesGroup::builder().title(tr(lang, "Caietul", "Notebook")).build();
        pages.add(&combo(self, tr(lang, "Mărimea textului", "Text size"), &setari::TEXT_SIZES, s.text, |st, v| st.text = v, |_| {}));
        pages.add(&combo(self, tr(lang, "La pornire arată", "Show at startup"), &setari::STARTS, s.start, |st, v| st.start = v, |_| {}));
        let one = adw::SwitchRow::builder()
            .title(tr(lang, "O singură pagină", "A single page"))
            .subtitle(tr(lang, "Doar ziua arătată, fără ziua dinainte alături; luna și săptămâna una sub alta", "Only the shown day, without the day before; month and week stacked"))
            .active(s.one_page)
            .build();
        let weak = Rc::downgrade(self);
        one.connect_active_notify(move |r| {
            if let Some(app) = weak.upgrade() {
                app.settings.borrow_mut().one_page = r.is_active();
                app.apply_settings();
                app.refresh();
            }
        });
        pages.add(&one);
        let vim = adw::SwitchRow::builder()
            .title(tr(lang, "Taste ca în Vim", "Vim keys"))
            .subtitle(tr(
                lang,
                "În afara câmpului de scris: j k alege, h l pagina, x bifează, i editează, o scrie, dd șterge, >> << mută, / caută; Esc iese din câmp",
                "Outside the writing field: j k select, h l page, x check, i edit, o write, dd delete, >> << indent, / search; Esc leaves the field",
            ))
            .active(s.vim)
            .build();
        let weak = Rc::downgrade(self);
        vim.connect_active_notify(move |r| {
            if let Some(app) = weak.upgrade() {
                app.settings.borrow_mut().vim = r.is_active();
                app.apply_settings();
            }
        });
        pages.add(&vim);
        page.add(&pages);

        let worth = adw::SpinRow::with_range(2.0, 10.0, 1.0);
        worth.set_title(tr(lang, "„Mai merită?” după", "\"Still worth it?\" after"));
        worth.set_subtitle(tr(lang, "Câte mutări ale aceleiași sarcini", "How many moves of the same task"));
        worth.set_value(s.worth as f64);
        let weak = Rc::downgrade(self);
        worth.connect_value_notify(move |r| {
            if let Some(app) = weak.upgrade() {
                app.settings.borrow_mut().worth = r.value() as u32;
                app.apply_settings();
                app.refresh();
            }
        });
        mig.add(&worth);

        // Copiile de siguranță.
        let bk = adw::PreferencesGroup::builder()
            .title(tr(lang, "Copii de siguranță", "Backups"))
            .description(tr(
                lang,
                "Tot jurnalul ca arhivă zip, făcută la pornire când e vremea ei. Pune dosarul pe alt disc sau într-unul sincronizat.",
                "The whole journal as a zip archive, made at startup when it's due. Put the folder on another disk or a synced one.",
            ))
            .build();
        let how = combo(self, tr(lang, "Cât de des", "How often"), &setari::BACKUPS, s.backup, |st, v| st.backup = v, |_| {});
        bk.add(&how);
        let folder = adw::ActionRow::builder()
            .title(tr(lang, "Dosarul", "Folder"))
            .subtitle(s.backup_folder().display().to_string())
            .subtitle_selectable(true)
            .build();
        let pick = gtk::Button::builder().label(tr(lang, "Alege…", "Choose…")).valign(gtk::Align::Center).build();
        folder.add_suffix(&pick);
        let (weak, row, dl) = (Rc::downgrade(self), folder.clone(), dialog.clone());
        pick.connect_clicked(move |_| {
            let Some(app) = weak.upgrade() else { return };
            let fd = gtk::FileDialog::builder().title(tr(app.lang, "Dosarul copiilor", "Backup folder")).modal(true).build();
            let (weak, row) = (Rc::downgrade(&app), row.clone());
            let parent = dl.root().and_downcast::<gtk::Window>();
            fd.select_folder(parent.as_ref(), gio::Cancellable::NONE, move |res| {
                let (Some(app), Ok(f)) = (weak.upgrade(), res) else { return };
                if let Some(p) = f.path() {
                    row.set_subtitle(&p.display().to_string());
                    app.settings.borrow_mut().backup_dir = Some(p);
                    app.apply_settings();
                }
            });
        });
        bk.add(&folder);
        let keep = adw::SpinRow::with_range(1.0, 100.0, 1.0);
        keep.set_title(tr(lang, "Câte copii se păstrează", "How many backups to keep"));
        keep.set_subtitle(tr(lang, "Cele mai vechi se șterg", "The oldest are deleted"));
        keep.set_value(s.backup_keep as f64);
        let weak = Rc::downgrade(self);
        keep.connect_value_notify(move |r| {
            if let Some(app) = weak.upgrade() {
                app.settings.borrow_mut().backup_keep = r.value() as u32;
                app.apply_settings();
            }
        });
        bk.add(&keep);
        let last = copii::list(&s.backup_folder()).last().map(|(d, _)| self.lang.day_title(*d));
        let now = adw::ActionRow::builder()
            .title(tr(lang, "Ultima copie", "Last backup"))
            .subtitle(last.unwrap_or_else(|| tr(lang, "niciuna încă", "none yet").to_string()))
            .build();
        let make = gtk::Button::builder().label(tr(lang, "Fă o copie acum", "Back up now")).valign(gtk::Align::Center).build();
        now.add_suffix(&make);
        let (weak, row) = (Rc::downgrade(self), now.clone());
        make.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.backup(true);
                row.set_subtitle(&app.lang.day_title(app.today.get()));
            }
        });
        bk.add(&now);
        page.add(&bk);

        // Încuierea.
        let enc = self.journal.key().is_some();
        let lk = adw::PreferencesGroup::builder()
            .title(tr(lang, "Încuierea", "Lock"))
            .description(if enc {
                tr(lang, "Jurnalul e criptat: la pornire se cere parola lui, iar la blocarea ecranului Lanruj se încuie mereu.", "The journal is encrypted: its password is asked at startup, and Lanruj always locks with the screen.")
            } else {
                tr(lang, "O parolă sau un PIN la deschidere. Fișierele rămân necriptate (pentru asta, vezi Criptarea).", "A password or PIN when opening. The files stay unencrypted (for that, see Encryption).")
            })
            .build();
        let names: Vec<&str> = setari::LOCKS.iter().map(|(_, _, ro, en)| if lang == Lang::Ro { *ro } else { *en }).collect();
        let kind = adw::ComboRow::builder()
            .title(if enc { tr(lang, "După blocarea ecranului cere", "After the screen locks, ask for") } else { tr(lang, "La deschidere cere", "When opening, ask for") })
            .subtitle(if enc { tr(lang, "„Nimic” = parola jurnalului", "\"Nothing\" = the journal password") } else { "" })
            .model(&gtk::StringList::new(&names))
            .build();
        kind.set_selected(setari::LOCKS.iter().position(|(v, ..)| *v == s.lock).unwrap_or(0) as u32);
        let screen = adw::SwitchRow::builder()
            .title(tr(lang, "Se încuie la blocarea ecranului", "Lock when the screen locks"))
            .active(enc || s.lock_on_screen)
            .sensitive(!enc && s.lock != setari::Lock::None)
            .build();
        let weak = Rc::downgrade(self);
        let (scr, busy) = (screen.clone(), Rc::new(Cell::new(false)));
        kind.connect_selected_notify(move |row| {
            let Some(app) = weak.upgrade() else { return };
            if busy.get() {
                return;
            }
            let Some((v, ..)) = setari::LOCKS.get(row.selected() as usize).copied() else { return };
            let before = app.settings.borrow().lock;
            if v == before {
                return;
            }
            let enc = app.journal.key().is_some();
            if v == setari::Lock::None {
                if let Err(e) = incuiere::set_lock(None) {
                    app.error(&e);
                }
                app.settings.borrow_mut().lock = v;
                app.apply_settings();
                scr.set_sensitive(false);
                return;
            }
            let (weak, row, scr, busy) = (Rc::downgrade(&app), row.clone(), scr.clone(), busy.clone());
            incuiere::new_lock_dialog(&app, v, move |ok| {
                let Some(app) = weak.upgrade() else { return };
                if ok {
                    app.settings.borrow_mut().lock = v;
                    app.apply_settings();
                    scr.set_sensitive(!enc);
                } else {
                    busy.set(true);
                    row.set_selected(setari::LOCKS.iter().position(|(x, ..)| *x == before).unwrap_or(0) as u32);
                    busy.set(false);
                }
            });
        });
        lk.add(&kind);
        let weak = Rc::downgrade(self);
        screen.connect_active_notify(move |r| {
            if let Some(app) = weak.upgrade()
                && app.journal.key().is_none()
            {
                app.settings.borrow_mut().lock_on_screen = r.is_active();
                app.apply_settings();
            }
        });
        lk.add(&screen);
        if enc {
            let remember = adw::SwitchRow::builder()
                .title(tr(lang, "Ține minte parola pe acest calculator", "Remember the password on this computer"))
                .subtitle(tr(
                    lang,
                    "În portofelul de chei: se deschide odată cu sesiunea (parola de la pornirea calculatorului), iar Lanruj nu o mai cere la pornire.",
                    "In the keyring: it opens with your session (the login password), and Lanruj no longer asks at startup.",
                ))
                .active(s.remember)
                .build();
            let (weak, busy) = (Rc::downgrade(self), Rc::new(Cell::new(false)));
            remember.connect_active_notify(move |r| {
                let Some(app) = weak.upgrade() else { return };
                if busy.get() {
                    return;
                }
                let root = app.journal.root().to_path_buf();
                if !r.is_active() {
                    app.settings.borrow_mut().remember = false;
                    app.apply_settings();
                    glib::spawn_future_local(incuiere::keyring_clear(root));
                    return;
                }
                // Pornit: e nevoie de parola jurnalului, ca să fie pusă în portofel.
                let (r2, busy2) = (r.clone(), busy.clone());
                app.ask_password_then(
                    tr(app.lang, "Ține minte parola", "Remember the password"),
                    tr(app.lang, "Scrie parola jurnalului; ea se pune în portofelul de chei.", "Type the journal password; it goes into the keyring."),
                    tr(app.lang, "Ține minte", "Remember"),
                    move |app, secret| {
                        app.settings.borrow_mut().remember = true;
                        app.apply_settings();
                        app.remember_password(secret);
                    },
                    move || {
                        busy2.set(true);
                        r2.set_active(false);
                        busy2.set(false);
                    },
                );
            });
            lk.add(&remember);
        }
        let auto = adw::SwitchRow::builder()
            .title(tr(lang, "Pornește odată cu calculatorul", "Start with the computer"))
            .active(incuiere::autostart())
            .build();
        let weak = Rc::downgrade(self);
        auto.connect_active_notify(move |r| {
            if let (Some(app), Err(e)) = (weak.upgrade(), incuiere::set_autostart(r.is_active())) {
                app.error(&e);
            }
        });
        lk.add(&auto);
        page.add(&lk);

        // Criptarea.
        let cr = adw::PreferencesGroup::builder()
            .title(tr(lang, "Criptarea", "Encryption"))
            .description(tr(
                lang,
                "Fișierele jurnalului criptate cu o parolă; la pornire Lanruj cere parola. Copiile de siguranță ale unui jurnal criptat sunt și ele criptate.",
                "The journal files encrypted with a password; Lanruj asks for it at startup. Backups of an encrypted journal are encrypted too.",
            ))
            .build();
        let on = lanruj_core::crypt::enabled(self.journal.root());
        let state_row = adw::ActionRow::builder()
            .title(if on { tr(lang, "Jurnalul e criptat", "The journal is encrypted") } else { tr(lang, "Jurnalul nu e criptat", "The journal is not encrypted") })
            .subtitle(if on {
                tr(lang, "Fișierele se pot citi doar cu Lanruj și cu parola", "The files can be read only with Lanruj and the password")
            } else {
                tr(lang, "Fișierele se pot citi și cu alte programe", "The files can be read with other programs too")
            })
            .build();
        let close_prefs = {
            let d = dialog.clone();
            move || {
                d.close();
            }
        };
        if on {
            let change = gtk::Button::builder().label(tr(lang, "Schimbă parola…", "Change password…")).valign(gtk::Align::Center).build();
            let off = gtk::Button::builder().label(tr(lang, "Oprește…", "Turn off…")).valign(gtk::Align::Center).css_classes(["destructive-action"]).build();
            state_row.add_suffix(&change);
            state_row.add_suffix(&off);
            let weak = Rc::downgrade(self);
            change.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.change_password_dialog();
                }
            });
            let weak = Rc::downgrade(self);
            let cp = close_prefs.clone();
            off.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    cp();
                    app.decrypt_dialog(|| {});
                }
            });
        } else {
            let start = gtk::Button::builder().label(tr(lang, "Criptează…", "Encrypt…")).valign(gtk::Align::Center).build();
            state_row.add_suffix(&start);
            let weak = Rc::downgrade(self);
            let cp = close_prefs.clone();
            start.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    cp();
                    app.encrypt_dialog(|| {});
                }
            });
        }
        cr.add(&state_row);
        page.add(&cr);

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
                let key = app.journal.key();
                drop(app);
                old.destroy();
                build_main(&gapp, key);
            });
        }));
        page.add(&lg);

        dialog.add(&page);
        dialog.present(Some(&self.window));
    }
}

/// O poză JPEG sau PNG cu latura mare peste 2000 de pixeli, micșorată și întoarsă după EXIF.
/// `None` = nu e nevoie sau nu s-a putut (se copiază așa cum e).
fn resized_image(src: &std::path::Path, dst: &std::path::Path) -> Option<Vec<u8>> {
    const MAX: i32 = 2000;
    let ext = dst.extension()?.to_str()?.to_lowercase();
    let format = match ext.as_str() {
        "jpg" | "jpeg" => "jpeg",
        "png" => "png",
        _ => return None,
    };
    let pb = gtk::gdk_pixbuf::Pixbuf::from_file(src).ok()?;
    let pb = pb.apply_embedded_orientation().unwrap_or(pb);
    let (w, h) = (pb.width(), pb.height());
    if w.max(h) <= MAX {
        return None;
    }
    let scale = MAX as f64 / w.max(h) as f64;
    let small = pb.scale_simple((w as f64 * scale) as i32, (h as f64 * scale) as i32, gtk::gdk_pixbuf::InterpType::Bilinear)?;
    let opts: &[(&str, &str)] = if format == "jpeg" { &[("quality", "90")] } else { &[] };
    small.save_to_bufferv(format, opts).ok()
}

thread_local! {
    /// Mărimile textului, mărite sau micșorate după setare (peste style.css).
    static TEXT_CSS: gtk::CssProvider = {
        let p = gtk::CssProvider::new();
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&display, &p, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
        }
        p
    };
}

/// Regulile din style.css cu mărimea textului înmulțită cu `pct`/100,
/// plus textul obișnuit al ferestrei.
fn text_css(pct: u32) -> String {
    if pct == 100 {
        return String::new();
    }
    let f = pct as f64 / 100.0;
    let mut css = include_str!("style.css").to_string();
    while let Some(a) = css.find("/*") {
        let Some(b) = css[a..].find("*/") else { break };
        css.replace_range(a..a + b + 2, "");
    }
    let mut out = format!("window {{ font-size: {pct}%; }}\n");
    for block in css.split('}') {
        let Some((sel, body)) = block.split_once('{') else { continue };
        for decl in body.split(';') {
            if let Some((k, v)) = decl.split_once(':')
                && k.trim() == "font-size"
                && let Some(px) = v.trim().strip_suffix("px").and_then(|n| n.trim().parse::<f64>().ok())
            {
                out.push_str(&format!("{} {{ font-size: {:.1}px; }}\n", sel.trim(), px * f));
            }
        }
    }
    out
}

pub fn set_text_size(pct: u32) {
    TEXT_CSS.with(|p| p.load_from_string(&text_css(pct)));
}

/// Pentru probe: după o clipă (LANRUJ_JOS=1: derulat până jos), captura ferestrei în LANRUJ_PNG, apoi ieșire.
#[cfg(debug_assertions)]
fn capture_and_quit(w: &adw::ApplicationWindow) {
    let Ok(p) = std::env::var("LANRUJ_PNG") else { return };
    let w = w.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(1200), move || {
        if std::env::var_os("LANRUJ_JOS").is_some() {
            fn scrolls(w: &gtk::Widget, out: &mut Vec<gtk::ScrolledWindow>) {
                if let Some(s) = w.downcast_ref::<gtk::ScrolledWindow>() {
                    out.push(s.clone());
                }
                let mut c = w.first_child();
                while let Some(ch) = c {
                    scrolls(&ch, out);
                    c = ch.next_sibling();
                }
            }
            let mut all = Vec::new();
            scrolls(w.upcast_ref(), &mut all);
            for s in all {
                let a = s.vadjustment();
                a.set_value(a.upper());
            }
        }
        glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
            save_png(&w, &p);
            if let Some(app) = w.application() {
                app.quit();
            }
        });
    });
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
    // A doua pornire aduce în față fereastra deja deschisă (sau pe cea de încuiere).
    if incuiere::present_lock() {
        return;
    }
    if let Some(w) = app.active_window() {
        w.present();
        return;
    }
    // Doar la probe: o încuietoare pusă înainte de pornire (LANRUJ_PUNE_INCUIETOARE=pin:1234).
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("LANRUJ_PUNE_INCUIETOARE")
        && let Some((kind, secret)) = v.split_once(':')
    {
        let _ = incuiere::set_lock(Some(secret));
        let mut st = setari::Settings::load();
        st.lock = if kind == "pin" { setari::Lock::Pin } else { setari::Lock::Password };
        st.lock_on_screen = true;
        let _ = st.save();
    }
    let root = Journal::default_root();
    let settings = setari::Settings::load();
    let lang = settings.lang();
    // Jurnal criptat: fereastra cu parola apare imediat; dacă parola e ținută minte,
    // portofelul de chei o dă (după ce se deblochează) și fereastra se închide singură.
    if lanruj_core::crypt::enabled(&root) {
        let opened = Rc::new(Cell::new(false));
        let o = opened.clone();
        let win = cheie::unlock_window(app, root.clone(), lang, move |app, key| {
            if !o.replace(true) {
                build_main(app, Some(key));
            }
        });
        if settings.remember {
            let app = app.clone();
            glib::spawn_future_local(async move {
                let Some(p) = incuiere::keyring_get(root.clone()).await else { return };
                let r = root.clone();
                let Some(k) = gio::spawn_blocking(move || lanruj_core::crypt::unlock(&r, &p).ok()).await.ok().flatten() else { return };
                if !opened.replace(true) {
                    build_main(&app, Some(std::sync::Arc::new(k)));
                    win.destroy();
                }
            });
        }
        return;
    }
    // Jurnal necriptat cu încuietoare: întâi parola sau PIN-ul.
    if settings.lock != setari::Lock::None {
        incuiere::lock_window(app, lang, incuiere::Check::Lock(settings.lock), |app| build_main(app, None));
        return;
    }
    build_main(app, None);
}

/// Fereastra jurnalului, cu cheia (dacă jurnalul e criptat și a fost deschis).
fn build_main(app: &adw::Application, key: Option<std::sync::Arc<lanruj_core::crypt::Key>>) {
    let settings = setari::Settings::load();
    let lang = settings.lang();
    let journal = Journal::new(Journal::default_root());
    journal.set_key(key);

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
    set_text_size(settings.text);
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
            (tr(lang, "Mergi la data…", "Go to date…"), "win.goto"),
            (tr(lang, "Pagina de dinainte", "Previous page"), "win.prev"),
            (tr(lang, "Pagina de după", "Next page"), "win.next"),
            (tr(lang, "Migrează", "Migrate"), "win.migrate"),
            (tr(lang, "Adaugă ziua în index", "Add the day to the index"), "win.index-add"),
        ]),
    );
    menu.append_section(None, &section(&[(tr(lang, "Strânge sau întinde bara laterală", "Collapse or expand the sidebar"), "win.sidebar")]));
    // Exportul, într-un submeniu.
    let exp = section(&[
        (tr(lang, "Pagina ca PDF…", "Page as PDF…"), "win.export-pdf"),
        (tr(lang, "Mai multe zile ca PDF…", "Several days as PDF…"), "win.export-pdf-days"),
        (tr(lang, "Tipărește pagina…", "Print the page…"), "win.print"),
        (tr(lang, "Tipărește mai multe zile…", "Print several days…"), "win.print-days"),
        (tr(lang, "Tot jurnalul ca arhivă zip…", "The whole journal as zip…"), "win.export-zip"),
    ]);
    menu.append_submenu(Some(tr(lang, "Exportă și tipărește", "Export and print")), &exp);
    menu.append_section(
        None,
        &section(&[
            (tr(lang, "Setări", "Preferences"), "win.preferences"),
            (tr(lang, "Ghid", "Guide"), "win.guide"),
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
        exporting: Cell::new(false),
        vim_pending: Cell::new(None),
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
    // Exportul și tipărirea.
    for (name, f) in [
        ("export-pdf", App::export_pdf_current as fn(&Rc<App>)),
        ("export-pdf-days", App::export_pdf_days),
        ("print", App::print_current),
        ("print-days", App::print_days),
        ("export-zip", App::export_zip),
    ] {
        let a = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(&state);
        a.connect_activate(move |_, _| {
            if let Some(s) = weak.upgrade() {
                f(&s);
            }
        });
        window.add_action(&a);
    }
    app.set_accels_for_action("win.print", &["<Control>p"]);
    app.set_accels_for_action("win.export-pdf", &["<Control><Shift>e"]);
    // Ctrl+G: Mergi la data… (un calendar; clic pe o zi o deschide).
    let a = gio::SimpleAction::new("goto", None);
    let weak = Rc::downgrade(&state);
    a.connect_activate(move |_, _| {
        let Some(s) = weak.upgrade() else { return };
        let lang = s.lang;
        let dialog = adw::Dialog::builder().title(tr(lang, "Mergi la data…", "Go to date…")).content_width(340).build();
        let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
        b.set_margin_top(6);
        b.set_margin_bottom(12);
        b.set_margin_start(12);
        b.set_margin_end(12);
        let cal = gtk::Calendar::new();
        let d = s.shown.get();
        if let Ok(dt) = glib::DateTime::from_local(d.year(), d.month() as i32, d.day() as i32, 12, 0, 0.0) {
            cal.select_day(&dt);
        }
        b.append(&cal);
        // Butonul deschide ziua aleasă (schimbarea lunii din săgeți nu deschide nimic).
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_halign(gtk::Align::End);
        let today_b = gtk::Button::with_label(tr(lang, "Azi", "Today"));
        let open_b = gtk::Button::builder().label(tr(lang, "Deschide ziua", "Open the day")).css_classes(["suggested-action"]).build();
        row.append(&today_b);
        row.append(&open_b);
        b.append(&row);
        let view = adw::ToolbarView::new();
        view.add_top_bar(&adw::HeaderBar::new());
        view.set_content(Some(&b));
        dialog.set_child(Some(&view));
        let (weak2, dl, c2) = (Rc::downgrade(&s), dialog.clone(), cal.clone());
        open_b.connect_clicked(move |_| {
            let dt = c2.date();
            if let (Some(app), Some(d)) = (weak2.upgrade(), NaiveDate::from_ymd_opt(dt.year(), dt.month() as u32, dt.day_of_month() as u32)) {
                dl.close();
                app.go(d);
            }
        });
        let (weak2, dl) = (Rc::downgrade(&s), dialog.clone());
        today_b.connect_clicked(move |_| {
            if let Some(app) = weak2.upgrade() {
                dl.close();
                app.go(app.today.get());
            }
        });
        dialog.set_default_widget(Some(&open_b));
        dialog.present(Some(&s.window));
    });
    window.add_action(&a);
    app.set_accels_for_action("win.goto", &["<Control>g"]);
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
    let a = gio::SimpleAction::new("guide", None);
    let weak = Rc::downgrade(&state);
    a.connect_activate(move |_, _| {
        if let Some(s) = weak.upgrade() {
            ghid::show(&s);
        }
    });
    window.add_action(&a);
    app.set_accels_for_action("win.guide", &["F1"]);
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
        // Agrafa: poze și fișiere.
        q.set_icon_tooltip_text(gtk::EntryIconPosition::Secondary, Some(tr(lang, "Poze și fișiere (le poți și trage în pagină)", "Pictures and files (you can also drag them onto the page)")));
        let weak = Rc::downgrade(&state);
        q.connect_icon_release(move |_, pos| {
            if pos == gtk::EntryIconPosition::Secondary
                && let Some(s) = weak.upgrade()
            {
                s.pick_files(is_left);
            }
        });
    }
    // Poze și fișiere trase în pagină.
    for page in [&state.left, &state.right] {
        let drop = gtk::DropTarget::new(gtk::gdk::FileList::static_type(), gtk::gdk::DragAction::COPY);
        let weak = Rc::downgrade(&state);
        let is_left = std::ptr::eq(page, &state.left);
        drop.connect_drop(move |_, value, _, _| {
            let Some(s) = weak.upgrade() else { return false };
            let Ok(list) = value.get::<gtk::gdk::FileList>() else { return false };
            let page = if is_left { &s.left } else { &s.right };
            let Some(r) = page.current.get() else {
                s.toast(tr(s.lang, "Pozele se pun pe o zi sau pe o pagină cu intrări.", "Pictures go on a day or a page with entries."));
                return false;
            };
            let files: Vec<std::path::PathBuf> = list.files().iter().filter_map(|f| f.path()).collect();
            glib::idle_add_local_once(move || s.attach_files(r, files));
            true
        });
        page.root.add_controller(drop);
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

    // Pagina de pornire aleasă în setări (migrarea de dimineață, dacă vine, are întâietate).
    if std::env::var_os("LANRUJ_START").is_none() {
        let t = state.today.get();
        match state.settings.borrow().start {
            setari::Start::Day => {}
            setari::Start::Week => state.view.set(View::Week(t.iso_week().year(), t.iso_week().week())),
            setari::Start::Month => state.view.set(View::Month(t.year(), t.month())),
            setari::Start::Plans => {
                let (y, m) = state.future_start();
                state.view.set(View::Future(y, m));
            }
            setari::Start::Index => state.view.set(View::Index),
        }
    }
    state.refresh();
    window.present();
    state.watch_screen_lock();
    // Cu tastele Vim, fereastra pornește în modul normal (nu în câmpul de scris).
    if state.settings.borrow().vim {
        gtk::prelude::GtkWindowExt::set_focus(&window, None::<&gtk::Widget>);
    }
    let weak = Rc::downgrade(&state);
    glib::idle_add_local_once(move || {
        if let Some(s) = weak.upgrade() {
            s.auto_migration();
            s.backup(false);
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
            if std::env::var_os("LANRUJ_MARIT").is_some() {
                state.window.maximize();
            }
            // LANRUJ_VEDERE: alt ecran înainte de captură (pentru site).
            let t = state.today.get();
            match std::env::var("LANRUJ_VEDERE").as_deref() {
                Ok("luna") => state.go_month(t.year(), t.month()),
                Ok("saptamana") => state.go_week(t.iso_week().year(), t.iso_week().week()),
                Ok("planuri") => {
                    let (y, m) = state.future_start();
                    state.go_future(y, m);
                }
                Ok("index") => state.go_index(),
                Ok("ghid") => {
                    gtk::prelude::WidgetExt::activate_action(&state.window, "win.guide", None).ok();
                }
                Ok(v) if v.starts_with("cauta:") => {
                    state.open_search();
                    state.cautare.entry.set_text(&v["cauta:".len()..]);
                }
                _ => {}
            }
            if let Ok(p) = std::env::var("LANRUJ_PNG") {
                let w = state.window.clone();
                let st = state.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
                    if std::env::var_os("LANRUJ_MASOARA").is_some() {
                        st.right.debug_heights();
                    }
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
        Ok("test-legaturi") => {
            let s = state.clone();
            glib::idle_add_local_once(move || {
                let q = s.right.quick.clone().expect("câmp");
                for t in ["- Harta: https://www.openstreetmap.org/#map=12/44.1/28.6", ". Rezervă la Delfinariu"] {
                    q.set_text(t);
                    q.emit_activate();
                }
                let date = s.shown.get();
                let r = PageRef::Day(date);
                let base = s.journal.load(date).expect("zi").items.len() - 1;
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
                    let v: gtk::TextView = find::<gtk::TextView>(&root, "editare").pop().expect("câmpul");
                    let buf = v.buffer();
                    let t = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
                    let a = t.find("Delfinariu").map(|b| t[..b].chars().count() as i32).expect("cuvântul");
                    buf.select_range(&buf.iter_at_offset(a), &buf.iter_at_offset(a + 10));
                    let mb: gtk::MenuButton = find::<gtk::MenuButton>(&root, "buton-legatura").pop().expect("butonul de legătură");
                    mb.popup();
                    let pop = mb.popover().expect("fereastra mică");
                    let e: gtk::Entry = find::<gtk::Entry>(pop.upcast_ref(), "adresa-legatura").pop().expect("adresa");
                    e.set_text("delfinariu-constanta.ro");
                    e.emit_activate();
                    eprintln!("selftest-legaturi: se salvează: {}", page::serialize_colored(&buf));
                    s2.finish_edit(Some(page::serialize_colored(&buf)));
                    let text = std::fs::read_to_string(s2.journal.day_path(date)).unwrap_or_default();
                    for l in text.lines().filter(|l| l.contains("http")) {
                        eprintln!("selftest-legaturi: fișier: {l}");
                    }
                    if let Ok(p) = std::env::var("LANRUJ_PNG") {
                        let w = s2.window.clone();
                        glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || save_png(&w, &p));
                    }
                });
            });
        }
        Ok("test-deschide") => {
            // Legăturile din ziua arătată: activate-link pe etichete și „Deschide…” din meniu.
            let s = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
                fn all<T: IsA<gtk::Widget>>(w: &gtk::Widget, out: &mut Vec<T>) {
                    if let Ok(t) = w.clone().downcast::<T>() {
                        out.push(t);
                    }
                    let mut c = w.first_child();
                    while let Some(ch) = c {
                        all::<T>(&ch, out);
                        c = ch.next_sibling();
                    }
                }
                let mut labels: Vec<gtk::Label> = Vec::new();
                all(s.right.root.upcast_ref(), &mut labels);
                for l in labels.iter().filter(|l| l.label().contains("<a href")) {
                    eprintln!("selftest-deschide: etichetă: {}", l.label());
                    let _: bool = l.emit_by_name("activate-link", &[&"https://thinkroot.xyz/"]);
                }
                let mut buttons: Vec<gtk::Button> = Vec::new();
                all(s.right.root.upcast_ref(), &mut buttons);
                for b in buttons.iter().filter(|b| b.has_css_class("deschide-legatura")) {
                    eprintln!("selftest-deschide: meniu: {}", b.label().unwrap_or_default());
                    b.emit_clicked();
                }
                if let Ok(p) = std::env::var("LANRUJ_PNG") {
                    save_png(&s.window, &p);
                }
            });
        }
        Ok("test-export") => {
            // Exportul fără ferestre de salvare: pagina, o săptămână de zile ca PDF, arhiva.
            let s = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(800), move || {
                let out = std::path::PathBuf::from(std::env::var("LANRUJ_IESIRE").unwrap_or_else(|_| "/tmp".into()));
                for (name, r) in [
                    ("pagina.pdf", s.test_export_current(&out.join("pagina.pdf"))),
                    ("zile.pdf", s.test_export_days(&out.join("zile.pdf"))),
                ] {
                    eprintln!("selftest-export: {name}: {r:?}");
                }
                eprintln!("selftest-export: arhiva: {:?}", export::write_zip(s.journal.root(), &out.join("jurnal.zip")));
                if let Some(app) = s.window.application() {
                    app.quit();
                }
            });
        }
        Ok("test-vim") => {
            // Tastele Vim pe ziua arătată: j j x (bifează a doua), j >> (sub cea de deasupra), dd (șterge), apoi fișierul.
            let s = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
                s.settings.borrow_mut().vim = true;
                gtk::prelude::GtkWindowExt::set_focus(&s.window, None::<&gtk::Widget>);
                let path = s.journal.day_path(s.shown.get());
                eprintln!("selftest-vim: înainte:\n{}", std::fs::read_to_string(&path).unwrap_or_default());
                let none = gdk::ModifierType::empty();
                for k in ["j", "j", "x", "j", "greater", "greater", "j", "d", "d"] {
                    let key = gdk::Key::from_name(k).expect("tastă");
                    let r = s.entry_key(key, none);
                    eprintln!("selftest-vim: {k} -> {r:?}, aleasă {:?}", s.selected.get().map(|(_, i)| i));
                }
                eprintln!("selftest-vim: după:\n{}", std::fs::read_to_string(&path).unwrap_or_default());
                if let Some(app) = s.window.application() {
                    app.quit();
                }
            });
        }
        Ok("test-decripteaza") => {
            // Oprește criptarea: parola de acum, apoi butonul.
            fn all<T: IsA<gtk::Widget>>(w: &gtk::Widget, out: &mut Vec<T>) {
                if let Some(x) = w.downcast_ref::<T>() {
                    out.push(x.clone());
                }
                let mut c = w.first_child();
                while let Some(ch) = c {
                    all(&ch, out);
                    c = ch.next_sibling();
                }
            }
            let s = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
                s.decrypt_dialog(|| eprintln!("selftest-decripteaza: gata"));
                let s2 = s.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                    let mut ps: Vec<gtk::PasswordEntry> = Vec::new();
                    all(s2.window.upcast_ref(), &mut ps);
                    for p in &ps {
                        p.set_text(&std::env::var("LANRUJ_PAROLA").unwrap_or_default());
                    }
                    let mut bs: Vec<gtk::Button> = Vec::new();
                    all(s2.window.upcast_ref(), &mut bs);
                    if let Some(b) = bs.iter().find(|b| b.label().as_deref() == Some("Oprește criptarea")) {
                        b.emit_clicked();
                    }
                    let s3 = s2.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(7000), move || {
                        let files = s3.journal.all_files();
                        let enc = files.iter().filter(|p| std::fs::read(p).map(|b| lanruj_core::crypt::is_encrypted(&b)).unwrap_or(false)).count();
                        eprintln!("selftest-decripteaza: {enc} din {} fișiere încă criptate; cheie pornită: {}", files.len(), lanruj_core::crypt::enabled(s3.journal.root()));
                        if let Some(app) = s3.window.application() {
                            app.quit();
                        }
                    });
                });
            });
        }
        Ok("test-incuie") => {
            // Ca la blocarea ecranului: Lanruj se încuie, apoi LANRUJ_SECRET îl deschide.
            let s = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
                s.lock_now();
                eprintln!("selftest-incuie: încuiat, fereastra jurnalului vizibilă: {}", s.window.is_visible());
                let s2 = s.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(5000), move || {
                    eprintln!("selftest-incuie: după secret, fereastra jurnalului vizibilă: {}", s2.window.is_visible());
                    if let Some(app) = s2.window.application() {
                        app.quit();
                    }
                });
            });
        }
        Ok("test-portofel") => {
            // Pune parola LANRUJ_PAROLA în portofel (ca „Ține minte”), apoi ieșire.
            let s = state.clone();
            s.settings.borrow_mut().remember = true;
            let _ = s.settings.borrow().save();
            let root = s.journal.root().to_path_buf();
            let p = std::env::var("LANRUJ_PAROLA").unwrap_or_default();
            glib::spawn_future_local(async move {
                eprintln!("selftest-portofel: pus: {:?}", incuiere::keyring_set(root.clone(), p).await);
                eprintln!("selftest-portofel: citit înapoi: {}", incuiere::keyring_get(root).await.is_some());
                if let Some(app) = s.window.application() {
                    app.quit();
                }
            });
        }
        Ok("test-cripteaza") => {
            // Pornește criptarea ca un om: parola de două ori, Continuă, ultimele 4 caractere, Criptează.
            fn all<T: IsA<gtk::Widget>>(w: &gtk::Widget, out: &mut Vec<T>) {
                if let Some(x) = w.downcast_ref::<T>() {
                    out.push(x.clone());
                }
                let mut c = w.first_child();
                while let Some(ch) = c {
                    all(&ch, out);
                    c = ch.next_sibling();
                }
            }
            let s = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
                s.encrypt_dialog(|| eprintln!("selftest-cripteaza: gata"));
                let s2 = s.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
                    let mut ps: Vec<gtk::PasswordEntry> = Vec::new();
                    all(s2.window.upcast_ref(), &mut ps);
                    for p in &ps {
                        p.set_text("parola-de-proba");
                    }
                    s2.window.queue_draw();
                    let w = s2.window.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                        if let Ok(p) = std::env::var("LANRUJ_PNG") {
                            save_png(&w, &p);
                        }
                    });
                    let mut bs: Vec<gtk::Button> = Vec::new();
                    all(s2.window.upcast_ref(), &mut bs);
                    let w = s2.window.clone();
                    let bs2 = bs.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
                        if let Some(b) = bs2.iter().find(|b| b.label().as_deref() == Some("Continuă")) {
                            b.emit_clicked();
                        }
                        glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                            if let Ok(p) = std::env::var("LANRUJ_PNG2") {
                                save_png(&w, &p);
                            }
                        });
                    });
                    let s3 = s2.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(1400), move || {
                        let mut ls: Vec<gtk::Label> = Vec::new();
                        all(s3.window.upcast_ref(), &mut ls);
                        let code = ls.iter().find(|l| l.has_css_class("monospace")).map(|l| l.label().to_string()).unwrap_or_default();
                        eprintln!("selftest-cripteaza: cheia de recuperare: {code}");
                        let _ = std::fs::write(s3.journal.root().join(".cheie-de-proba"), &code);
                        let mut es: Vec<gtk::Entry> = Vec::new();
                        all(s3.window.upcast_ref(), &mut es);
                        if let Some(e) = es.iter().find(|e| e.max_length() == 4) {
                            e.set_text(&code[code.len() - 4..]);
                        }
                        let mut bs: Vec<gtk::Button> = Vec::new();
                        all(s3.window.upcast_ref(), &mut bs);
                        if let Some(b) = bs.iter().find(|b| b.label().as_deref() == Some("Criptează jurnalul")) {
                            eprintln!("selftest-cripteaza: butonul e activ: {}", b.is_sensitive());
                            b.emit_clicked();
                        }
                        let w = s3.window.clone();
                        glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                            if let Ok(p) = std::env::var("LANRUJ_PNG3") {
                                save_png(&w, &p);
                            }
                        });
                        let s4 = s3.clone();
                        glib::timeout_add_local_once(std::time::Duration::from_millis(6000), move || {
                            let files = s4.journal.all_files();
                            let enc = files.iter().filter(|p| std::fs::read(p).map(|b| lanruj_core::crypt::is_encrypted(&b)).unwrap_or(false)).count();
                            eprintln!("selftest-cripteaza: {enc} din {} fișiere criptate; cheie pornită: {}", files.len(), lanruj_core::crypt::enabled(s4.journal.root()));
                            if let Some(app) = s4.window.application() {
                                app.quit();
                            }
                        });
                    });
                });
            });
        }
        Ok("test-poze") => {
            let s = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                let dir = std::env::var("LANRUJ_FISIERE").unwrap_or_default();
                let files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
                let mut files = files;
                files.sort();
                let r = PageRef::Day(s.shown.get());
                // O intrare cu o poză sub ea, mare și cu nume; restul pozelor în galerie, la sfârșit.
                let q = s.right.quick.clone().expect("câmp");
                q.set_text("o 11:00 Delfinariu, spectacolul cu delfini");
                q.emit_activate();
                let (pics, others): (Vec<_>, Vec<_>) = files.into_iter().partition(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("delfin")));
                s.attach_files_under(r, pics, Some(0));
                s.attach_files(r, others);
                s.set_attachment(r, 1, lanruj_core::attach::Attachment::Image { name: "Delfinariul din Constanța".into(), width: Some(380), path: format!("{}/delfinariu.jpg", s.shown.get().format("%Y-%m-%d")) });
                eprintln!("selftest-poze: fișierul zilei:\n{}", std::fs::read_to_string(s.journal.day_path(s.shown.get())).unwrap_or_default());
                if let Ok(p) = std::env::var("LANRUJ_PNG2") {
                    let s2 = s.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(1200), move || {
                        let imgs: Vec<poze::Shown> = poze::page_images(&s2, r, &s2.journal.load_ref(r).unwrap().items).into_iter().map(|(_, x)| x).collect();
                        poze::viewer(&s2, imgs, 1);
                        let w = s2.window.clone();
                        glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
                            fn find(w: &gtk::Widget, icon: &str, out: &mut Vec<gtk::Button>) {
                                if let Some(b) = w.downcast_ref::<gtk::Button>()
                                    && b.icon_name().as_deref() == Some(icon)
                                {
                                    out.push(b.clone());
                                }
                                let mut c = w.first_child();
                                while let Some(ch) = c {
                                    find(&ch, icon, out);
                                    c = ch.next_sibling();
                                }
                            }
                            fn subtitle(w: &gtk::Widget) -> Option<String> {
                                if let Some(t) = w.downcast_ref::<adw::WindowTitle>()
                                    && t.subtitle().contains(" / ")
                                {
                                    return Some(t.subtitle().to_string());
                                }
                                let mut c = w.first_child();
                                while let Some(ch) = c {
                                    if let Some(x) = subtitle(&ch) {
                                        return Some(x);
                                    }
                                    c = ch.next_sibling();
                                }
                                None
                            }
                            let mut zin = Vec::new();
                            find(w.upcast_ref(), "zoom-in-symbolic", &mut zin);
                            eprintln!("selftest-vizor: la început {:?}", subtitle(w.upcast_ref()));
                            if let Some(b) = zin.first() {
                                b.emit_clicked();
                                b.emit_clicked();
                            }
                            eprintln!("selftest-vizor: după ++ {:?}", subtitle(w.upcast_ref()));
                            let mut zout = Vec::new();
                            find(w.upcast_ref(), "zoom-out-symbolic", &mut zout);
                            if let Some(b) = zout.first() {
                                b.emit_clicked();
                            }
                            eprintln!("selftest-vizor: după - {:?}", subtitle(w.upcast_ref()));
                            let w2 = w.clone();
                            glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || save_png(&w2, &p));
                        });
                    });
                }
                if let Ok(p) = std::env::var("LANRUJ_PNG") {
                    let w = s.window.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || save_png(&w, &p));
                }
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
        Ok("ghid") => {
            let w = state.window.clone();
            glib::idle_add_local_once(move || {
                gtk::prelude::WidgetExt::activate_action(&w, "win.guide", None).ok();
                if let Ok(p) = std::env::var("LANRUJ_PNG") {
                    let w2 = w.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || save_png(&w2, &p));
                }
            });
        }
        Ok("taste") => {
            let w = state.window.clone();
            glib::idle_add_local_once(move || {
                gtk::prelude::WidgetExt::activate_action(&w, "win.shortcuts", None).ok();
                capture_and_quit(&w);
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
                capture_and_quit(&w);
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
    vechi::migrate();
    let app = adw::Application::builder().application_id(APP_ID).flags(flags).build();
    app.connect_startup(|_| load_css());
    app.connect_activate(build_ui);
    app.run()
}
