// SPDX-License-Identifier: GPL-3.0-or-later
//! Lanruj: jurnal în stilul Bullet Journal, pe fișiere Markdown.

mod page;

use adw::prelude::*;
use chrono::{Datelike, Duration, Local, NaiveDate};
use gtk::{gdk, gio, glib};
use lanruj_core::{Day, Journal, Lang, parse_quick};
use page::{Page, Side};
use std::cell::Cell;
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
    today: Cell<NaiveDate>,
    window: adw::ApplicationWindow,
    left: Page,
    right: Page,
    title: adw::WindowTitle,
    calendar: gtk::Calendar,
    today_count: gtk::Label,
    toasts: adw::ToastOverlay,
    calendar_busy: Cell<bool>,
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

impl App {
    /// Cum se numește o zi față de azi: „Azi”, „Ieri”, „Mâine”, „Acum 5 zile”.
    fn relative(&self, d: NaiveDate) -> String {
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
        self.toasts.add_toast(adw::Toast::new(&msg));
    }

    pub fn refresh(self: &Rc<Self>) {
        let shown = self.shown.get();
        let prev = shown - Duration::days(1);
        let right = self.load(shown);
        let left = self.load(prev);
        self.right.fill(self, &right, &self.relative(shown));
        self.left.fill(self, &left, &self.relative(prev));
        self.title.set_title(&self.lang.day_title_short(shown));
        let mut sub = self.relative(shown);
        if shown.year() != self.today.get().year() {
            sub = format!("{} · {}", shown.year(), sub);
        }
        self.title.set_subtitle(&sub);

        let t = self.today.get();
        let open = if shown == t { right.open_tasks() } else { self.load(t).open_tasks() };
        self.today_count.set_label(&if open > 0 { open.to_string() } else { String::new() });
        self.sync_calendar();
    }

    /// Calendarul mic: ziua arătată selectată, zilele scrise cu un punct.
    fn sync_calendar(&self) {
        self.calendar_busy.set(true);
        let d = self.shown.get();
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
        self.shown.set(d);
        self.refresh();
    }

    pub fn change(self: &Rc<Self>, date: NaiveDate, f: impl FnOnce(&mut Day)) {
        if let Err(e) = self.journal.update(date, self.lang, f) {
            self.error(&e);
        }
        self.refresh();
    }

    pub fn toggle(self: &Rc<Self>, date: NaiveDate, index: usize) {
        if let Err(e) = self.journal.toggle_done(date, index, self.lang) {
            self.error(&e);
        }
        self.refresh();
    }

    fn quick_add(self: &Rc<Self>, entry: &gtk::Entry) {
        let Some(e) = parse_quick(&entry.text()) else { return };
        match self.journal.add(self.shown.get(), e, self.lang) {
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
        if i > 0 {
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
    let next = gtk::Button::from_icon_name("go-next-symbolic");
    next.set_action_name(Some("win.next"));
    next.set_tooltip_text(Some(tr(lang, "Ziua de după (Alt+→)", "Next day (Alt+→)")));
    header.pack_end(&next);
    let migrate = gtk::Button::builder()
        .child(&adw::ButtonContent::builder().icon_name("go-jump-symbolic").label(tr(lang, "Migrează", "Migrate")).build())
        .sensitive(false)
        .tooltip_text(soon)
        .build();
    header.pack_end(&migrate);
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
    one_page.add_setter(&left.root, "visible", Some(&false.to_value()));
    one_page.add_setter(&cotor, "visible", Some(&false.to_value()));
    let right_root = right.root.clone();
    one_page.connect_apply(move |_| right_root.add_css_class("singura"));
    let right_root = right.root.clone();
    one_page.connect_unapply(move |_| right_root.remove_css_class("singura"));
    window.add_breakpoint(one_page);
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
    });

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
        if let Some(s) = weak.upgrade()
            && row.index() == 0 {
                s.go(s.today.get());
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
                s.go(s.shown.get() + Duration::days(delta));
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
        glib::ControlFlow::Continue
    });

    state.refresh();
    window.present();
    #[cfg(debug_assertions)]
    if std::env::var_os("LANRUJ_SELFTEST").is_some() {
        selftest(&state);
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
    let q = s.right.quick.clone().expect("câmpul rapid");
    q.set_text(". * Caută o lampă de birou");
    q.emit_activate();
    q.set_text("o 9:30 Dentist");
    q.emit_activate();
    let mut signs = Vec::new();
    find_buttons(s.right.root.upcast_ref(), "semn", &mut signs);
    if let Some(b) = signs.first() {
        b.emit_clicked();
    }
    let mut dots = Vec::new();
    find_buttons(s.right.root.upcast_ref(), "punct", &mut dots);
    if let Some(b) = dots.get(1) {
        b.emit_clicked();
    }
    s.go(s.shown.get() - Duration::days(1));
    eprintln!("selftest: titlu={} câmp=„{}”", s.title.title(), q.text());
    s.window.close();
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| load_css());
    app.connect_activate(build_ui);
    app.run()
}
