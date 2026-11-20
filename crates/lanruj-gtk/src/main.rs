// SPDX-License-Identifier: GPL-3.0-or-later
//! Lanruj: jurnal în stilul Bullet Journal, pe fișiere Markdown.

mod migrare;
mod page;

use adw::prelude::*;
use chrono::{Datelike, Duration, Local, NaiveDate};
use gtk::{gdk, gio, glib};
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
    dismissed: Cell<Option<NaiveDate>>,
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
}

/// Ce ecran se vede.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Day,
    Month(i32, u32),
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
        let month_stacked = self.narrow.get() && matches!(self.view.get(), View::Month(..)) && !self.migrating();
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

    pub fn refresh(self: &Rc<Self>) {
        let month_view = matches!(self.view.get(), View::Month(..)) && !self.migrating();
        if self.migrating() {
            self.refresh_migration();
        } else if let View::Month(y, m) = self.view.get() {
            self.refresh_month(y, m);
        } else {
            self.refresh_normal();
        }
        self.refresh_migration_header();
        let on_today = !month_view && self.shown.get() == self.today.get();
        self.today_button.set_visible(!self.migrating() && !on_today);
        self.migrate_button.set_visible(!self.migrating() && !month_view);
        let row = if month_view { 2 } else { 0 };
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
        let left = self.load(source);
        let right = self.load(target);
        let panel = match current {
            Some(i) => match left.items.get(i) {
                Some(lanruj_core::Item::Entry(e)) => Some(self.build_panel(e, decided.get(&i).copied())),
                _ => None,
            },
            None => Some(self.build_done_panel()),
        };
        let view = MigView { current, candidates: &order, decided: &decided, panel, new_ids: &new_ids, is_source: true };
        self.left.fill(self, &left, PageRef::Day(source), &self.relative(source), &self.lang.day_title(source), Some(&view));
        let view = MigView { current: None, candidates: &[], decided: &decided, panel: None, new_ids: &new_ids, is_source: false };
        self.right.fill(self, &right, PageRef::Day(target), &self.relative(target), &self.lang.day_title(target), Some(&view));
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

    pub fn go_month(self: &Rc<Self>, y: i32, m: u32) {
        if self.migrating() {
            self.end_migration(false);
        }
        self.editing.set(None);
        self.cal_editing.set(None);
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
        let d = self.mig.borrow().as_ref().map(|m| m.source).unwrap_or(self.shown.get());
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
        // Plecarea din migrare spre altă zi e ca „Mai târziu”.
        if self.migrating() {
            self.end_migration(false);
        }
        self.editing.set(None);
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

    fn quick_add(self: &Rc<Self>, entry: &gtk::Entry) {
        let Some(e) = parse_quick(&entry.text()) else { return };
        // În migrare, câmpul de jos scrie pe ziua din dreapta (mâine).
        let r = match (self.mig.borrow().as_ref().map(|m| m.target), self.view.get()) {
            (Some(target), _) => PageRef::Day(target),
            (None, View::Month(year, month)) => PageRef::Month { year, month },
            (None, View::Day) => PageRef::Day(self.shown.get()),
        };
        match self.journal.update_ref(r, self.lang, |d| d.push(e)) {
            Ok(_) => entry.set_text(""),
            Err(err) => self.error(&err),
        }
        self.refresh();
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

fn nav_row(icon: &str, text: &str, extra: Option<&gtk::Label>) -> gtk::ListBoxRow {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    b.append(&gtk::Image::from_icon_name(icon));
    let l = gtk::Label::builder().label(text).xalign(0.0).hexpand(true).build();
    b.append(&l);
    if let Some(e) = extra {
        b.append(e);
    }
    gtk::ListBoxRow::builder().child(&b).build()
}

fn build_ui(app: &adw::Application) {
    let lang = Lang::from_env();
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
    let rows = [
        nav_row("document-edit-symbolic", tr(lang, "Azi", "Today"), Some(&today_count)),
        nav_row("view-dual-symbolic", tr(lang, "Săptămâna", "Week"), None),
        nav_row("view-grid-symbolic", tr(lang, "Luna", "Month"), None),
        nav_row("task-due-symbolic", tr(lang, "Viitor", "Future"), None),
        nav_row(
            "view-sort-ascending-symbolic",
            tr(lang, "Monitorizare", "Tracking"),
            Some(&gtk::Label::builder().label("v2").css_classes(["dim-label", "caption"]).build()),
        ),
        nav_row("view-list-bullet-symbolic", "Index", None),
    ];
    for (i, r) in rows.iter().enumerate() {
        if i != 0 && i != 2 {
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
    side_box.append(&calendar);

    let menu = gio::Menu::new();
    menu.append(Some(tr(lang, "Despre Lanruj", "About Lanruj")), Some("app.about"));
    let side_header = adw::HeaderBar::new();
    let search = gtk::Button::from_icon_name("edit-find-symbolic");
    search.set_sensitive(false);
    search.set_tooltip_text(Some(tr(lang, "Căutarea vine într-un pas următor", "Search comes in a later step")));
    side_header.pack_start(&search);
    side_header.pack_end(&gtk::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&menu).build());
    let side_view = adw::ToolbarView::new();
    side_view.add_top_bar(&side_header);
    side_view.set_content(Some(&side_box));
    let sidebar = adw::NavigationPage::new(&side_view, "Lanruj");

    // Caietul
    let placeholder = tr(lang, "Scrie aici: . sarcină, o eveniment, - notă, apoi Enter", "Write here: . task, o event, - note, then Enter");
    let left = Page::new(Side::Left, false, placeholder);
    let right = Page::new(Side::Right, true, placeholder);
    let caiet = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    caiet.add_css_class("caiet");
    let cotor = gtk::Box::new(gtk::Orientation::Vertical, 0);
    cotor.add_css_class("cotor");
    caiet.append(&left.root);
    caiet.append(&cotor);
    caiet.append(&right.root);
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&caiet));

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
    let content = adw::NavigationPage::new(&content_view, "Lanruj");

    let split = adw::NavigationSplitView::builder()
        .sidebar(&sidebar)
        .content(&content)
        .min_sidebar_width(230.0)
        .max_sidebar_width(270.0)
        .build();
    window.set_content(Some(&split));

    // Pe ecran îngust: o singură pagină; și mai îngust: bara laterală ascunsă.
    let one_page = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 900sp").expect("condiție"));
    let narrow = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 600sp").expect("condiție"));
    narrow.add_setter(&split, "collapsed", Some(&true.to_value()));
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
        nav_rows: rows.to_vec(),
    });

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
            s.start_migration(s.shown.get(), false);
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

    // Scrierea rapidă
    if let Some(q) = &state.right.quick {
        let weak = Rc::downgrade(&state);
        q.connect_activate(move |e| {
            if let Some(s) = weak.upgrade() {
                s.quick_add(e);
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
    let keep = state.clone();
    window.connect_close_request(move |_| {
        let _ = &keep;
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
    if std::env::var("LANRUJ_START").as_deref() == Ok("luna") {
        let t = state.today.get();
        state.go_month(t.year(), t.month());
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
    click(&left, "opt-las", 0); // Lampă -> o las
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
        fn find_entry(w: &gtk::Widget) -> Option<gtk::Entry> {
            if let Some(e) = w.downcast_ref::<gtk::Entry>()
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
            Some(e) => {
                eprintln!("selftest: câmpul de editare are „{}”", e.text());
                e.set_text(". Dentist");
                e.emit_activate();
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
            gtk::prelude::WidgetExt::activate_action(&s3.window, "win.next", None).ok();
            eprintln!("selftest: după „înainte” titlu={}", s3.title.title());
            gtk::prelude::WidgetExt::activate_action(&s3.window, "win.today", None).ok();
            eprintln!("selftest: după Azi titlu={} butonul Azi vizibil={}", s3.title.title(), s3.today_button.is_visible());
            s3.window.close();
        });
    });
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| load_css());
    app.connect_activate(build_ui);
    app.run()
}
