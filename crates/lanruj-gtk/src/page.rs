// SPDX-License-Identifier: GPL-3.0-or-later
//! O pagină din caiet: titlul zilei, intrările, etichetele și nota zilei.

use crate::migrare::MigView;
use crate::{App, tr};
use adw::prelude::*;
use gtk::glib;
use chrono::{Datelike, NaiveDate};
use lanruj_core::{Day, Decision, Entry, Item, Kind, Month, PageRef, Priority, TaskState};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

pub struct Page {
    pub root: gtk::Box,
    label: gtk::Label,
    title: gtk::Label,
    list: gtk::Box,
    footer: gtk::Box,
    pub quick: Option<gtk::Entry>,
}

impl Page {
    pub fn new(side: Side, with_quick: bool, placeholder: &str) -> Page {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("pagina");
        root.add_css_class(if side == Side::Left { "stanga" } else { "dreapta" });
        root.set_hexpand(true);
        root.set_width_request(300);

        let head = gtk::Box::new(gtk::Orientation::Vertical, 2);
        head.set_margin_top(22);
        head.set_margin_start(30);
        head.set_margin_end(30);
        head.set_margin_bottom(6);
        let label = gtk::Label::builder().xalign(0.0).css_classes(["eticheta-pagina"]).build();
        let title = gtk::Label::builder().xalign(0.0).wrap(true).css_classes(["titlu-pagina"]).build();
        head.append(&label);
        head.append(&title);
        root.append(&head);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.set_margin_start(30);
        list.set_margin_end(30);
        list.set_margin_top(8);
        list.set_margin_bottom(8);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&list)
            .build();
        root.append(&scroll);

        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.set_margin_start(30);
        footer.set_margin_end(30);
        footer.set_margin_top(10);
        footer.set_margin_bottom(14);
        root.append(&footer);

        let quick = with_quick.then(|| {
            let e = gtk::Entry::builder().placeholder_text(placeholder).css_classes(["rapid"]).build();
            e.set_margin_start(20);
            e.set_margin_end(20);
            e.set_margin_bottom(18);
            root.append(&e);
            e
        });

        Page { root, label, title, list, footer, quick }
    }

    fn clear(&self) {
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
    }

    pub fn fill(&self, app: &Rc<App>, day: &Day, r: PageRef, label: &str, title: &str, mig: Option<&MigView>) {
        let lang = app.lang;
        self.label.set_label(&label.to_uppercase());
        self.label.set_visible(!label.is_empty());
        self.title.set_label(title);

        self.clear();
        let mut any = false;
        for (index, item) in day.items.iter().enumerate() {
            match item {
                Item::Entry(e) => {
                    any = true;
                    let row = entry_row(app, r, index, e, mig);
                    self.list.append(&row);
                    if let Some(m) = mig
                        && m.is_source
                        && m.current == Some(index)
                        && let Some(p) = &m.panel
                    {
                        self.list.append(p);
                    }
                }
                Item::Other(s) if !s.trim().is_empty() => {
                    any = true;
                    let l = gtk::Label::builder()
                        .label(s.as_str())
                        .xalign(0.0)
                        .wrap(true)
                        .wrap_mode(gtk::pango::WrapMode::WordChar)
                        .css_classes(["intrare", "altceva"])
                        .build();
                    self.list.append(&l);
                }
                Item::Other(_) => {
                    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    spacer.set_height_request(11);
                    self.list.append(&spacer);
                }
            }
        }
        // Totul triat: panoul de sfârșit stă sub listă.
        if let Some(m) = mig
            && m.is_source
            && m.current.is_none()
            && let Some(p) = &m.panel
        {
            self.list.append(p);
        }
        if !any {
            let l = gtk::Label::builder()
                .label(tr(lang, "Nicio intrare încă.", "Nothing written yet."))
                .xalign(0.0)
                .css_classes(["intrare", "gol"])
                .build();
            self.list.append(&l);
        }

        // Etichetele și nota sunt doar ale zilelor.
        self.footer.set_visible(matches!(r, PageRef::Day(_)));
        if matches!(r, PageRef::Day(_)) {
            self.fill_footer(app, day);
        }
    }

    /// Cele șapte zile ale săptămânii, pe scurt, citite din paginile zilelor.
    /// Clic pe o zi = o deschide.
    pub fn fill_week_days(&self, app: &Rc<App>, monday: NaiveDate) {
        let lang = app.lang;
        self.label.set_label(&tr(lang, "Zilele săptămânii", "The week's days").to_uppercase());
        self.label.set_visible(true);
        self.title.set_label(&lang.week_range(monday));
        self.footer.set_visible(false);
        self.clear();
        let today = app.today.get();
        for i in 0..7 {
            let d = monday + chrono::Duration::days(i);
            let day = app.journal.load(d).unwrap_or_else(|_| Day::new(d));
            let head = gtk::Button::builder()
                .label(format!("{} {}", lang.weekday(d.weekday()), d.day()))
                .css_classes(["zi-sapt-titlu"])
                .halign(gtk::Align::Start)
                .build();
            if d == today {
                head.add_css_class("azi");
            }
            head.set_tooltip_text(Some(&lang.day_title(d)));
            let weak = Rc::downgrade(app);
            head.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    glib::idle_add_local_once(move || app.go(d));
                }
            });
            if i > 0 {
                head.set_margin_top(8);
            }
            self.list.append(&head);
            let mut any = false;
            for e in day.entries() {
                any = true;
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                row.add_css_class("zi-pe-scurt");
                row.set_margin_start(10 + 18 * e.depth as i32);
                match e.kind {
                    Kind::Task(TaskState::Done) => row.add_css_class("facut"),
                    Kind::Task(TaskState::Migrated) | Kind::Task(TaskState::Scheduled) => row.add_css_class("mutat"),
                    Kind::Task(TaskState::Dropped) => row.add_css_class("renuntat"),
                    _ => {}
                }
                row.append(&gtk::Label::builder().label(sign_of(e.kind)).css_classes(["semn"]).valign(gtk::Align::Start).build());
                let mut text = String::new();
                if let Some(t) = &e.time {
                    text.push_str(&format!("<b>{}</b>  ", glib::markup_escape_text(t)));
                }
                match e.priority {
                    Priority::Important => text.push_str("★ "),
                    Priority::Inspiration => text.push_str("! "),
                    Priority::None => {}
                }
                let esc = rich_markup(&e.text, app.window_night());
                if e.kind == Kind::Task(TaskState::Dropped) {
                    text.push_str(&format!("<s>{esc}</s>"));
                } else {
                    text.push_str(&esc);
                }
                row.append(
                    &gtk::Label::builder()
                        .label(text)
                        .use_markup(true)
                        .xalign(0.0)
                        .wrap(true)
                        .wrap_mode(gtk::pango::WrapMode::WordChar)
                        .hexpand(true)
                        .css_classes(["text"])
                        .build(),
                );
                self.list.append(&row);
            }
            if !any {
                let l = gtk::Label::builder().label("-").xalign(0.0).css_classes(["gol"]).build();
                l.set_margin_start(10);
                self.list.append(&l);
            }
        }
    }

    /// Calendarul lunii: toate zilele, fiecare cu rândul ei scurt.
    /// Clic pe număr = deschide ziua; dublu clic pe rând = scrie la ziua aceea.
    pub fn fill_calendar(&self, app: &Rc<App>, month: &Month, title: &str) {
        let lang = app.lang;
        self.label.set_label(&tr(lang, "Calendar", "Calendar").to_uppercase());
        self.label.set_visible(true);
        self.title.set_label(title);
        self.footer.set_visible(false);
        if let Some(q) = &self.quick {
            q.set_visible(false);
        }
        self.clear();
        let (y, m) = (month.year, month.month);
        let written = app.journal.written_days(y, m);
        let today = app.today.get();
        let mut d = NaiveDate::from_ymd_opt(y, m, 1).expect("lună validă");
        while d.month() == m {
            let n = d.day();
            let texts = month.day_texts(n);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("zi-luna");
            if d.weekday().num_days_from_monday() >= 5 {
                row.add_css_class("weekend");
            }
            if d == today {
                row.add_css_class("azi");
            }
            let num = gtk::Button::builder().label(n.to_string()).css_classes(["zi-nr"]).valign(gtk::Align::Start).build();
            if written.contains(&n) {
                num.add_css_class("scrisa");
            }
            num.set_tooltip_text(Some(&lang.day_title(d)));
            let weak = Rc::downgrade(app);
            num.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    glib::idle_add_local_once(move || app.go(d));
                }
            });
            row.append(&num);
            let wd: String = lang.weekday(d.weekday()).chars().take(2).collect();
            row.append(&gtk::Label::builder().label(wd).css_classes(["zi-sapt"]).valign(gtk::Align::Start).build());
            if app.cal_editing.get() == Some(n) {
                let entry = gtk::Entry::builder()
                    .text(texts.join("; "))
                    .hexpand(true)
                    .placeholder_text(tr(lang, "Ce e în ziua asta (mai multe, despărțite prin ;)", "What's on this day (several, separated by ;)"))
                    .css_classes(["editare"])
                    .build();
                let weak = Rc::downgrade(app);
                entry.connect_activate(move |en| {
                    let t = en.text().to_string();
                    if let Some(app) = weak.upgrade() {
                        glib::idle_add_local_once(move || app.finish_cal_edit(Some(t)));
                    }
                });
                let keys = gtk::EventControllerKey::new();
                let weak = Rc::downgrade(app);
                keys.connect_key_pressed(move |_, key, _, _| {
                    if key == gtk::gdk::Key::Escape {
                        if let Some(app) = weak.upgrade() {
                            glib::idle_add_local_once(move || app.finish_cal_edit(None));
                        }
                        return glib::Propagation::Stop;
                    }
                    glib::Propagation::Proceed
                });
                entry.add_controller(keys);
                let en = entry.clone();
                glib::idle_add_local_once(move || {
                    en.grab_focus();
                    en.set_position(-1);
                });
                row.append(&entry);
            } else {
                let text = gtk::Label::builder()
                    .label(texts.join(" · "))
                    .xalign(0.0)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .hexpand(true)
                    .css_classes(["zi-text"])
                    .build();
                row.append(&text);
                let double = gtk::GestureClick::builder().button(1).build();
                let weak = Rc::downgrade(app);
                double.connect_pressed(move |_, k, _, _| {
                    if k == 2
                        && let Some(app) = weak.upgrade()
                    {
                        glib::idle_add_local_once(move || app.start_cal_edit(n));
                    }
                });
                row.add_controller(double);
                let right = gtk::GestureClick::builder().button(3).build();
                let weak = Rc::downgrade(app);
                right.connect_pressed(move |_, _, _, _| {
                    if let Some(app) = weak.upgrade() {
                        glib::idle_add_local_once(move || app.start_cal_edit(n));
                    }
                });
                row.add_controller(right);
            }
            self.list.append(&row);
            d = d.succ_opt().expect("zi validă");
        }
        let hint = gtk::Label::builder()
            .label(tr(lang, "Dublu clic pe o zi ca să scrii ce e atunci; clic pe număr ca s-o deschizi.", "Double-click a day to write what's on; click its number to open it."))
            .xalign(0.0)
            .wrap(true)
            .css_classes(["gol"])
            .build();
        hint.set_margin_top(10);
        self.list.append(&hint);
    }

    fn fill_footer(&self, app: &Rc<App>, day: &Day) {
        let lang = app.lang;
        while let Some(c) = self.footer.first_child() {
            self.footer.remove(&c);
        }
        let date = day.date;

        let tags = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .column_spacing(6)
            .row_spacing(6)
            .max_children_per_line(20)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();
        for tag in &day.tags {
            // Cu o etichetă proprie ca fiu, butonul nu mai arată săgeata.
            let b = gtk::MenuButton::builder().child(&gtk::Label::new(Some(tag))).css_classes(["et"]).build();
            let pop = gtk::Popover::new();
            let remove = gtk::Button::with_label(tr(lang, "Scoate eticheta", "Remove tag"));
            remove.add_css_class("flat");
            let (weak, tag_s, pop_c) = (Rc::downgrade(app), tag.clone(), pop.clone());
            remove.connect_clicked(move |_| {
                pop_c.popdown();
                if let Some(app) = weak.upgrade() {
                    let t = tag_s.clone();
                    app.change(date, move |d| d.tags.retain(|x| *x != t));
                }
            });
            pop.set_child(Some(&remove));
            b.set_popover(Some(&pop));
            tags.append(&b);
        }
        // „+ etichetă”: un câmp mic; mai multe etichete se despart prin virgulă.
        let add = gtk::MenuButton::builder()
            .child(&gtk::Label::new(Some(tr(lang, "+ etichetă", "+ tag"))))
            .css_classes(["adauga"])
            .build();
        let pop = gtk::Popover::new();
        let entry = gtk::Entry::builder()
            .placeholder_text(tr(lang, "carte, film", "book, film"))
            .width_chars(18)
            .build();
        pop.set_child(Some(&entry));
        let (weak, pop_c) = (Rc::downgrade(app), pop.clone());
        entry.connect_activate(move |e| {
            let text = e.text().to_string();
            pop_c.popdown();
            let Some(app) = weak.upgrade() else { return };
            let new: Vec<String> = text.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
            if new.is_empty() {
                return;
            }
            app.change(date, move |d| {
                for t in new {
                    if !d.tags.contains(&t) {
                        d.tags.push(t);
                    }
                }
            });
        });
        add.set_popover(Some(&pop));
        tags.append(&add);
        self.footer.append(&tags);

        let nota = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        nota.set_valign(gtk::Align::Center);
        let l = gtk::Label::builder().label(lang.rating_label()).css_classes(["nota-label"]).build();
        l.set_margin_end(4);
        nota.append(&l);
        for i in 1..=5u8 {
            let b = gtk::Button::new();
            b.add_css_class("punct");
            b.set_valign(gtk::Align::Center);
            if day.rating.is_some_and(|r| i <= r) {
                b.add_css_class("plin");
            }
            b.set_tooltip_text(Some(&format!("{i}/5")));
            let weak = Rc::downgrade(app);
            b.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.change(date, move |d| d.rating = if d.rating == Some(i) { None } else { Some(i) });
                }
            });
            nota.append(&b);
        }
        self.footer.append(&nota);
    }
}

/// Textul intrării cu culorile lui, ca markup Pango. Noaptea, textul colorat se
/// luminează și markerul devine pe jumătate transparent, ca cerneala să se vadă.
pub fn rich_markup(text: &str, night: bool) -> String {
    use lanruj_core::colors::{night_color, runs};
    let mut out = String::new();
    for r in runs(text) {
        let esc = glib::markup_escape_text(&r.text);
        let mut attrs = String::new();
        if let Some(c) = &r.color {
            let c = if night { night_color(c) } else { c.as_str() };
            attrs.push_str(&format!(" foreground=\"{c}\""));
        }
        if let Some(m) = &r.marker {
            attrs.push_str(&format!(" background=\"{m}\""));
            if night {
                attrs.push_str(" bgalpha=\"40%\"");
            } else if r.color.is_none() {
                // Pe marker, cerneala închisă (și în paginile cu altă culoare de cerneală).
                attrs.push_str(" foreground=\"#1c3d6b\"");
            }
        }
        if attrs.is_empty() {
            out.push_str(&esc);
        } else {
            out.push_str(&format!("<span{attrs}>{esc}</span>"));
        }
    }
    out
}

fn sign_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Task(TaskState::Open) | Kind::Task(TaskState::Dropped) => "•",
        Kind::Task(TaskState::Done) => "×",
        Kind::Task(TaskState::Migrated) => ">",
        Kind::Task(TaskState::Scheduled) => "<",
        Kind::Event => "○",
        Kind::Note => "–",
    }
}

/// Rândul în editare: un câmp cu intrarea scrisă ca jos („. text”, „o 10:00 text”, „- text”).
fn edit_row(app: &Rc<App>, e: &Entry) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
    row.set_margin_start(26 * e.depth as i32);
    let entry = gtk::Entry::builder().text(e.to_quick()).hexpand(true).css_classes(["editare"]).build();
    let weak = Rc::downgrade(app);
    entry.connect_activate(move |en| {
        let text = en.text().to_string();
        if let Some(app) = weak.upgrade() {
            glib::idle_add_local_once(move || app.finish_edit(Some(text)));
        }
    });
    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(app);
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(app) = weak.upgrade() {
                glib::idle_add_local_once(move || app.finish_edit(None));
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    entry.add_controller(keys);
    let en = entry.clone();
    glib::idle_add_local_once(move || {
        en.grab_focus();
        en.set_position(-1);
    });
    row.append(&entry);
    row.append(&color_bar(app.lang, ColorTarget::Field(entry.clone())));
    row.append(
        &gtk::Label::builder()
            .label(tr(app.lang, "Selectează o bucată de text, apoi o culoare; fără selecție, se colorează tot textul.", "Select some text, then a color; with no selection, all the text is colored."))
            .css_classes(["mic", "dim-label"])
            .wrap(true)
            .xalign(0.0)
            .build(),
    );
    row
}

/// Unde pune bara de culori culoarea: în câmpul de editare (pe bucata selectată
/// sau pe tot textul) ori direct pe o intrare (din meniul de la clic dreapta).
#[derive(Clone)]
pub enum ColorTarget {
    Field(gtk::Entry),
    Entry(std::rc::Weak<App>, PageRef, usize, gtk::Popover),
}

/// Pune culoarea în câmpul de editare: pe bucata selectată sau, fără selecție, pe tot textul
/// (fără semnul de la început, „. ”, „o 10:00 ”…).
fn paint_field(entry: &gtk::Entry, paint: Option<lanruj_core::colors::Paint>) {
    use lanruj_core::colors::{paint as do_paint, plain};
    let full = entry.text().to_string();
    let Some(parsed) = lanruj_core::parse_quick(&full) else { return };
    let trimmed = full.trim_end();
    let body_start = if trimmed.ends_with(&parsed.text) { trimmed.len() - parsed.text.len() } else { 0 };
    let (prefix, body) = full.split_at(body_start);
    let prefix_chars = prefix.chars().count() as i32;
    let body_chars: Vec<char> = body.chars().collect();
    let to_plain = |raw: i32| -> usize {
        let raw = (raw - prefix_chars).clamp(0, body_chars.len() as i32) as usize;
        plain(&body_chars[..raw].iter().collect::<String>()).chars().count()
    };
    let (start, end) = match entry.selection_bounds() {
        Some((a, b)) if a != b => (to_plain(a.min(b)), to_plain(a.max(b))),
        _ => (0, usize::MAX),
    };
    let new_body = do_paint(body, start, end, paint);
    entry.set_text(&format!("{prefix}{new_body}"));
    entry.grab_focus();
    entry.set_position(-1);
}

/// Bara de culori: textul colorat (A colorat), markerul (pătrățele) și „fără culoare”.
pub fn color_bar(lang: lanruj_core::Lang, target: ColorTarget) -> gtk::Box {
    use lanruj_core::colors::{MARKER_COLORS, Paint, TEXT_COLORS};
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    bar.add_css_class("bara-culori");
    let apply = move |target: &ColorTarget, paint: Option<Paint>| match target {
        ColorTarget::Field(e) => paint_field(e, paint),
        ColorTarget::Entry(weak, r, i, pop) => {
            pop.popdown();
            if let Some(app) = weak.upgrade() {
                let (r, i) = (*r, *i);
                let owned = paint.map(|p| match p {
                    Paint::Color(c) => (false, c.to_string()),
                    Paint::Marker(m) => (true, m.to_string()),
                });
                glib::idle_add_local_once(move || app.color_entry(r, i, owned));
            }
        }
    };
    for (ro, en, day, _) in TEXT_COLORS {
        // Butoanele nu iau focusul, ca selecția din câmpul de editare să rămână.
        let b = gtk::Button::builder().label("A").focus_on_click(false).css_classes(["culoare-text", &format!("text-{}", &day[1..])]).build();
        b.set_tooltip_text(Some(&format!("Text {}", if lang == lanruj_core::Lang::Ro { ro.to_lowercase() } else { en.to_lowercase() })));
        let t = target.clone();
        b.connect_clicked(move |_| apply(&t, Some(Paint::Color(day))));
        bar.append(&b);
    }
    bar.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    for (ro, en, color) in MARKER_COLORS {
        let b = gtk::Button::builder().focus_on_click(false).css_classes(["culoare-marker", &format!("mark-{}", &color[1..])]).build();
        b.set_tooltip_text(Some(&format!("Marker {}", if lang == lanruj_core::Lang::Ro { ro.to_lowercase() } else { en.to_lowercase() })));
        let t = target.clone();
        b.connect_clicked(move |_| apply(&t, Some(Paint::Marker(color))));
        bar.append(&b);
    }
    bar.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    let none = gtk::Button::builder().icon_name("edit-clear-symbolic").focus_on_click(false).css_classes(["flat", "fara-culoare"]).build();
    none.set_tooltip_text(Some(tr(lang, "Fără culoare", "No color")));
    let t = target.clone();
    none.connect_clicked(move |_| apply(&t, None));
    bar.append(&none);
    for b in [bar.clone().upcast::<gtk::Widget>()] {
        let mut c = b.first_child();
        while let Some(ch) = c {
            ch.set_valign(gtk::Align::Center);
            c = ch.next_sibling();
        }
    }
    bar
}

/// Unde se poate muta o intrare: azi, mâine, lista lunii aceasteia și a celei viitoare.
/// (O zi anume și o lună din Planuri se aleg separat, în același meniu.)
fn move_targets(app: &App, from: PageRef) -> Vec<(PageRef, String)> {
    let lang = app.lang;
    let t = app.today.get();
    let this_week = lanruj_core::week_of(t);
    let next_week = lanruj_core::migrate::next_page(this_week);
    let this_month = PageRef::Month { year: t.year(), month: t.month() };
    let next_month = lanruj_core::migrate::next_page(this_month);
    let name = |r: PageRef| match r {
        PageRef::Month { month, .. } => lang.month(month).to_string(),
        _ => String::new(),
    };
    [
        (PageRef::Day(t), tr(lang, "Azi", "Today").to_string()),
        (PageRef::Day(t.succ_opt().expect("zi")), tr(lang, "Mâine", "Tomorrow").to_string()),
        (this_week, tr(lang, "Planul săptămânii aceasteia", "This week's plan").to_string()),
        (next_week, tr(lang, "Planul săptămânii viitoare", "Next week's plan").to_string()),
        (this_month, format!("{} ({})", tr(lang, "Lista lunii aceasteia", "This month's list"), name(this_month))),
        (next_month, format!("{} ({})", tr(lang, "Lista lunii viitoare", "Next month's list"), name(next_month))),
    ]
    .into_iter()
    .filter(|(r, _)| *r != from)
    .collect()
}

/// A doua pagină a meniului, „Mută…”: se construiește abia la primul clic,
/// ca rândurile să nu poarte fiecare un calendar și o listă de luni.
fn build_move_page(app: &Rc<App>, pop: &gtk::Popover, stack: &gtk::Stack, date: PageRef, index: usize) {
    let lang = app.lang;
    let mbox = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let back = gtk::Button::builder().icon_name("go-previous-symbolic").css_classes(["flat"]).halign(gtk::Align::Start).build();
    let st = stack.clone();
    back.connect_clicked(move |_| st.set_visible_child_name("meniu"));
    mbox.append(&back);
    for (to, label) in move_targets(app, date) {
        let b = gtk::Button::builder().label(label.as_str()).css_classes(["flat", "muta-spre"]).build();
        b.child().and_then(|c| c.downcast::<gtk::Label>().ok()).inspect(|l| l.set_xalign(0.0));
        let (weak, p) = (Rc::downgrade(app), pop.clone());
        b.connect_clicked(move |_| {
            p.popdown();
            if let Some(app) = weak.upgrade() {
                glib::idle_add_local_once(move || app.move_entry(date, index, to));
            }
        });
        mbox.append(&b);
    }
    // O lună din Planuri (de după luna viitoare)
    let t = app.today.get();
    let mut months = Vec::new();
    let (mut y, mut m) = lanruj_core::migrate::next_month(t.year(), t.month());
    for _ in 0..5 {
        (y, m) = lanruj_core::migrate::next_month(y, m);
        months.push((y, m));
    }
    let names: Vec<String> = months.iter().map(|(y, m)| lang.month_title(*y, *m)).collect();
    let names_ref: Vec<&str> = names.iter().map(String::as_str).collect();
    mbox.append(&gtk::Label::builder().label(tr(lang, "În planuri", "To the future log")).css_classes(["heading"]).xalign(0.0).margin_top(6).build());
    let prow = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let dd = gtk::DropDown::from_strings(&names_ref);
    dd.set_hexpand(true);
    let put = gtk::Button::builder().label(tr(lang, "Mută", "Move")).build();
    let (weak, p, dd_c) = (Rc::downgrade(app), pop.clone(), dd.clone());
    put.connect_clicked(move |_| {
        let Some(&(year, month)) = months.get(dd_c.selected() as usize) else { return };
        p.popdown();
        if let Some(app) = weak.upgrade() {
            glib::idle_add_local_once(move || app.move_entry(date, index, PageRef::Future { year, month }));
        }
    });
    prow.append(&dd);
    prow.append(&put);
    mbox.append(&prow);
    // O zi anume
    mbox.append(&gtk::Label::builder().label(tr(lang, "Pe o zi anume", "On a specific day")).css_classes(["heading"]).xalign(0.0).margin_top(6).build());
    let cal = gtk::Calendar::new();
    mbox.append(&cal);
    let pick = gtk::Button::builder().label(tr(lang, "Mută pe ziua aleasă", "Move to the chosen day")).css_classes(["suggested-action"]).build();
    let (weak, p, cal_c) = (Rc::downgrade(app), pop.clone(), cal.clone());
    pick.connect_clicked(move |_| {
        let dt = cal_c.date();
        let Some(d) = NaiveDate::from_ymd_opt(dt.year(), dt.month() as u32, dt.day_of_month() as u32) else { return };
        p.popdown();
        if let Some(app) = weak.upgrade() {
            glib::idle_add_local_once(move || app.move_entry(date, index, PageRef::Day(d)));
        }
    });
    mbox.append(&pick);
    stack.add_named(&mbox, Some("muta"));
}

/// Meniul de la clic dreapta: Editează, Mută…, Șterge. Dublu clic = editează.
fn add_entry_menu(app: &Rc<App>, row: &gtk::Box, date: PageRef, index: usize, movable: bool) {
    let lang = app.lang;
    let pop = gtk::Popover::builder().has_arrow(true).build();
    let stack = gtk::Stack::builder().vhomogeneous(false).hhomogeneous(false).transition_type(gtk::StackTransitionType::SlideLeftRight).build();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let edit = gtk::Button::builder().label(tr(lang, "Editează", "Edit")).css_classes(["flat"]).build();
    let mv = gtk::Button::builder().label(tr(lang, "Mută…", "Move…")).css_classes(["flat", "muta"]).visible(movable).build();
    let col = gtk::Button::builder().label(tr(lang, "Culoare…", "Color…")).css_classes(["flat", "culoare"]).build();
    let del = gtk::Button::builder().label(tr(lang, "Șterge", "Delete")).css_classes(["flat", "error"]).build();
    for b in [&edit, &mv, &col, &del] {
        b.child().and_then(|c| c.downcast::<gtk::Label>().ok()).inspect(|l| l.set_xalign(0.0));
        menu.append(b);
    }
    stack.add_named(&menu, Some("meniu"));


    let (st, weak, p) = (stack.clone(), Rc::downgrade(app), pop.clone());
    mv.connect_clicked(move |_| {
        if st.child_by_name("muta").is_none()
            && let Some(app) = weak.upgrade()
        {
            build_move_page(&app, &p, &st, date, index);
        }
        st.set_visible_child_name("muta");
    });
    let (st, weak, p) = (stack.clone(), Rc::downgrade(app), pop.clone());
    col.connect_clicked(move |_| {
        if st.child_by_name("culoare").is_none()
            && let Some(app) = weak.upgrade()
        {
            let page = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let back = gtk::Button::builder().icon_name("go-previous-symbolic").css_classes(["flat"]).halign(gtk::Align::Start).build();
            let st2 = st.clone();
            back.connect_clicked(move |_| st2.set_visible_child_name("meniu"));
            page.append(&back);
            page.append(&gtk::Label::builder().label(tr(app.lang, "Culoarea întregii intrări", "Color of the whole entry")).css_classes(["heading"]).xalign(0.0).build());
            page.append(&color_bar(app.lang, ColorTarget::Entry(Rc::downgrade(&app), date, index, p.clone())));
            st.add_named(&page, Some("culoare"));
        }
        st.set_visible_child_name("culoare");
    });
    let st = stack.clone();
    pop.connect_closed(move |_| st.set_visible_child_name("meniu"));
    pop.set_child(Some(&stack));
    pop.set_parent(row);
    let p = pop.clone();
    row.connect_destroy(move |_| p.unparent());

    let (weak, p) = (Rc::downgrade(app), pop.clone());
    edit.connect_clicked(move |_| {
        p.popdown();
        if let Some(app) = weak.upgrade() {
            glib::idle_add_local_once(move || app.start_edit(date, index));
        }
    });
    let (weak, p) = (Rc::downgrade(app), pop.clone());
    del.connect_clicked(move |_| {
        p.popdown();
        if let Some(app) = weak.upgrade() {
            glib::idle_add_local_once(move || app.delete_entry(date, index));
        }
    });

    let right = gtk::GestureClick::builder().button(3).build();
    let p = pop.clone();
    right.connect_pressed(move |g, _, x, y| {
        p.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        p.popup();
        g.set_state(gtk::EventSequenceState::Claimed);
    });
    row.add_controller(right);
    let long = gtk::GestureLongPress::new();
    let p = pop.clone();
    long.connect_pressed(move |_, x, y| {
        p.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        p.popup();
    });
    row.add_controller(long);
    let double = gtk::GestureClick::builder().button(1).build();
    let weak = Rc::downgrade(app);
    double.connect_pressed(move |_, n, _, _| {
        if n == 2
            && let Some(app) = weak.upgrade()
        {
            glib::idle_add_local_once(move || app.start_edit(date, index));
        }
    });
    row.add_controller(double);
}

fn entry_row(app: &Rc<App>, r: PageRef, index: usize, e: &Entry, mig: Option<&MigView>) -> gtk::Box {
    let lang = app.lang;
    if mig.is_none() && app.editing.get() == Some((r, index)) {
        return edit_row(app, e);
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("intrare");
    if mig.is_none() && app.selected.get() == Some((r, index)) {
        row.add_css_class("selectat");
    }
    if mig.is_none() {
        let movable = e.is_open_task() || matches!(e.kind, Kind::Event | Kind::Note);
        add_entry_menu(app, &row, r, index, movable && e.depth == 0);
    }
    // În migrare: sarcina curentă încadrată, cele triate și restul stinse;
    // un clic pe o sarcină de triat o face curentă.
    if let Some(m) = mig.filter(|m| m.is_source) {
        if m.current == Some(index) {
            row.add_css_class("acum");
        } else if !m.candidates.contains(&index) || m.decided.contains_key(&index) {
            row.add_css_class("terminat");
        }
        if m.candidates.contains(&index) {
            let click = gtk::GestureClick::new();
            let weak = Rc::downgrade(app);
            click.connect_released(move |_, _, _, _| {
                if let Some(app) = weak.upgrade() {
                    app.mig_select(index);
                }
            });
            row.add_controller(click);
            row.set_cursor_from_name(Some("pointer"));
        }
    }
    row.set_margin_start(26 * e.depth as i32);
    match e.kind {
        Kind::Task(TaskState::Done) => row.add_css_class("facut"),
        Kind::Task(TaskState::Migrated) | Kind::Task(TaskState::Scheduled) => row.add_css_class("mutat"),
        Kind::Task(TaskState::Dropped) => row.add_css_class("renuntat"),
        _ => {}
    }

    let sign = sign_of(e.kind);
    if mig.is_none() && matches!(e.kind, Kind::Task(TaskState::Open) | Kind::Task(TaskState::Done)) {
        let b = gtk::Button::with_label(sign);
        b.add_css_class("semn");
        b.set_valign(gtk::Align::Start);
        b.set_tooltip_text(Some(if e.kind == Kind::Task(TaskState::Done) {
            tr(lang, "Marchează ca nefăcută", "Mark as not done")
        } else {
            tr(lang, "Marchează ca făcută", "Mark as done")
        }));
        let weak = Rc::downgrade(app);
        b.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.toggle(r, index);
            }
        });
        row.append(&b);
    } else {
        let l = gtk::Label::builder().label(sign).css_classes(["semn"]).valign(gtk::Align::Start).build();
        row.append(&l);
    }

    let mut markup = String::new();
    // În jurnalul viitor, ziua de la începutul textului e îngroșată: „15 Revizia”.
    let (fday, ftext) = match r {
        PageRef::Future { .. } => lanruj_core::future::split_day(&e.text),
        _ => (None, e.text.as_str()),
    };
    if let Some(d) = fday {
        markup.push_str(&format!("<b>{d}</b>  "));
    }
    if let Some(t) = &e.time {
        markup.push_str(&format!("<b>{}</b>  ", glib::markup_escape_text(t)));
    }
    let text = rich_markup(ftext, app.window_night());
    if e.kind == Kind::Task(TaskState::Dropped) {
        markup.push_str(&format!("<s>{text}</s>"));
    } else {
        markup.push_str(&text);
    }

    // Steluța și becul au culoarea lor, deci stau într-o etichetă separată.
    let text_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    text_box.set_hexpand(true);
    if e.priority != Priority::None {
        let p = gtk::Label::builder()
            .label(if e.priority == Priority::Important { "★" } else { "!" })
            .css_classes([if e.priority == Priority::Important { "stea" } else { "bec" }])
            .valign(gtk::Align::Start)
            .build();
        text_box.append(&p);
    }
    let label = gtk::Label::builder()
        .use_markup(true)
        .label(markup.as_str())
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .css_classes(["text"])
        .build();
    text_box.append(&label);

    // ↻N: de câte ori a fost mutată; de la 3, „mai merită?”.
    if e.is_open_task()
        && let Some(id) = &e.id {
            let n = app.journal.migration_count(id);
            if n > 0 {
                let m = gtk::Label::builder().label(format!("↻{n}")).css_classes(["migrari"]).valign(gtk::Align::Start).build();
                m.set_margin_top(3);
                if n >= 3 {
                    m.add_css_class("mult");
                    m.set_label(&format!("↻{n} {}", tr(lang, "mai merită?", "still worth it?")));
                }
                m.set_tooltip_text(Some(&format!("{} {n}", tr(lang, "Mutată de", "Moved"))));
                text_box.append(&m);
            }
        }
    // Pe pagina din dreapta: ce a ajuns acum, din migrare.
    if let Some(m) = mig
        && !m.is_source
        && e.id.as_ref().is_some_and(|id| m.new_ids.contains(id))
    {
        let n = gtk::Label::builder().label(tr(lang, "nou", "new")).css_classes(["nou"]).valign(gtk::Align::Start).build();
        n.set_margin_top(3);
        text_box.append(&n);
    }
    // Pe pagina din stânga: unde a plecat sarcina triată.
    if let Some(m) = mig
        && m.is_source
        && let Some(Decision::Moved(to)) = m.decided.get(&index)
        && *to != lanruj_core::migrate::next_page(r)
    {
        let l = gtk::Label::builder()
            .label(format!("→ {}", crate::migrare::page_short(lang, *to)))
            .css_classes(["migrari"])
            .valign(gtk::Align::Start)
            .build();
        l.set_margin_top(3);
        text_box.append(&l);
    }
    row.append(&text_box);
    row
}
