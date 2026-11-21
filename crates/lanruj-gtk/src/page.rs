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
    /// Temele din index care privesc ziua (concediul în care e, rândurile cu data ei).
    banner: gtk::Box,
    /// Pagina arătată acum (pentru pozele trase în ea); `None` pe paginile făcute automat.
    pub current: std::cell::Cell<Option<PageRef>>,
    pub list: gtk::Box,
    pub scroll: gtk::ScrolledWindow,
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
        let banner = gtk::Box::new(gtk::Orientation::Vertical, 4);
        banner.set_margin_start(30);
        banner.set_margin_end(30);
        banner.set_visible(false);
        root.append(&banner);

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
            let e = gtk::Entry::builder()
                .placeholder_text(placeholder)
                .css_classes(["rapid"])
                .secondary_icon_name("mail-attachment-symbolic")
                .secondary_icon_tooltip_text("Poze și fișiere (le poți și trage în pagină)")
                .build();
            e.set_margin_start(20);
            e.set_margin_end(20);
            e.set_margin_bottom(18);
            root.append(&e);
            e
        });

        Page { root, label, title, banner, list, scroll, footer, quick, current: std::cell::Cell::new(None) }
    }

    fn clear(&self) {
        self.current.set(None);
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        while let Some(c) = self.banner.first_child() {
            self.banner.remove(&c);
        }
        self.banner.set_visible(false);
    }

    pub fn fill(&self, app: &Rc<App>, day: &Day, r: PageRef, label: &str, title: &str, mig: Option<&MigView>) {
        let lang = app.lang;
        self.label.set_label(&label.to_uppercase());
        self.label.set_visible(!label.is_empty());
        self.title.set_label(title);

        self.clear();
        self.current.set(Some(r));
        // Pe o zi: temele din index (concediul în care e și rândurile cu data ei).
        if let (PageRef::Day(d), None) = (r, mig) {
            for t in app.journal.topics_on(d) {
                self.banner.append(&topic_box(app, &t, true));
                self.banner.set_visible(true);
            }
            // Și ce a fost la serviciu (din Monitorizare).
            let work = app.journal.work_on(d);
            if !work.is_empty() {
                self.banner.append(&Page::work_box(app, d, &work, true));
                self.banner.set_visible(true);
            }
        }
        let mut any = false;
        // Pozele paginii, pentru fereastra de mărire (← → trec de la una la alta).
        let images = Rc::new(crate::poze::page_images(app, r, &day.items));
        let mut skip_until = 0;
        for (index, item) in day.items.iter().enumerate() {
            if index < skip_until {
                continue;
            }
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
                Item::Other(s) if lanruj_core::attach::parse(s).is_some() => {
                    use lanruj_core::attach::{Attachment, depth_of, parse};
                    any = true;
                    let a = parse(s).expect("verificat");
                    let depth = depth_of(s);
                    let w: gtk::Widget = match &a {
                        Attachment::Image { .. } => {
                            // Pozele una după alta, la același nivel, se adună într-o galerie.
                            let mut run = vec![(index, a.clone())];
                            for (j, it) in day.items.iter().enumerate().skip(index + 1) {
                                match it {
                                    Item::Other(l) if depth_of(l) == depth && matches!(parse(l), Some(Attachment::Image { .. })) => {
                                        run.push((j, parse(l).expect("verificat")));
                                    }
                                    _ => break,
                                }
                            }
                            if run.len() >= 2 {
                                skip_until = run.last().map(|(j, _)| j + 1).unwrap_or(index + 1);
                                crate::poze::gallery(app, r, &run, images.clone())
                            } else {
                                crate::poze::single(app, r, index, &a, images.clone())
                            }
                        }
                        Attachment::File { .. } => crate::poze::file_row(app, r, index, &a),
                    };
                    w.set_margin_start(26 * depth as i32 + if depth > 0 { 28 } else { 0 });
                    self.list.append(&w);
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
        self.fill_days(app, monday, 7, tr(lang, "Zilele săptămânii", "The week's days"), &lang.week_range(monday));
    }

    /// Zilele dintre `start` și `start + count`, pe scurt, citite din paginile zilelor.
    pub fn fill_days(&self, app: &Rc<App>, start: NaiveDate, count: i64, label: &str, title: &str) {
        let lang = app.lang;
        self.label.set_label(&label.to_uppercase());
        self.label.set_visible(true);
        self.title.set_label(title);
        self.footer.set_visible(false);
        self.clear();
        let today = app.today.get();
        for i in 0..count {
            let d = start + chrono::Duration::days(i);
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
            for t in app.journal.topics_on(d) {
                let b = topic_box(app, &t, false);
                b.set_margin_start(10);
                self.list.append(&b);
                any = true;
            }
            let work = app.journal.work_on(d);
            if !work.is_empty() {
                let b = Page::work_box(app, d, &work, false);
                b.set_margin_start(10);
                self.list.append(&b);
                any = true;
            }
            if !any {
                let l = gtk::Label::builder().label("-").xalign(0.0).css_classes(["gol"]).build();
                l.set_margin_start(10);
                self.list.append(&l);
            }
        }
    }

    /// Doar la dezvoltare: înălțimea fiecărui rând din pagină.
    #[cfg(debug_assertions)]
    pub fn debug_heights(&self) {
        let mut c = self.list.first_child();
        while let Some(ch) = c {
            let (_, nat, _, _) = ch.measure(gtk::Orientation::Vertical, ch.width());
            eprintln!("rând: {} lățime={} înălțime={} (dorită {nat})", ch.type_().name(), ch.width(), ch.height());
            let mut g = ch.first_child();
            while let Some(x) = g {
                eprintln!("    {} {}x{} vizibil={}", x.type_().name(), x.width(), x.height(), x.is_visible());
                g = x.next_sibling();
            }
            c = ch.next_sibling();
        }
    }

    pub fn header(&self, label: &str, title: &str) {
        self.label.set_label(&label.to_uppercase());
        self.label.set_visible(!label.is_empty());
        self.title.set_label(title);
        self.footer.set_visible(false);
        self.clear();
    }

    /// Indexul scris de mână: rândurile din `index.md`, cu „+ Adaugă”.
    /// Clic = deschide ziua; clic dreapta = scoate din index.
    pub fn fill_index_manual(&self, app: &Rc<App>, idx: &lanruj_core::Index) -> gtk::MenuButton {
        let lang = app.lang;
        self.header(tr(lang, "Indexul tău", "Your index"), "Index");
        let add = gtk::MenuButton::builder()
            .child(&gtk::Label::new(Some(tr(lang, "+ Adaugă în index", "+ Add to index"))))
            .css_classes(["adauga-index"])
            .halign(gtk::Align::Start)
            .build();
        let pop = gtk::Popover::new();
        pop.set_child(Some(&index_form_box(app, None, &pop)));
        add.set_popover(Some(&pop));
        self.list.append(&add);
        let mut any = false;
        for (i, it) in idx.items.iter().enumerate() {
            match it {
                lanruj_core::IndexItem::Entry { title, start, end, .. } => {
                    any = true;
                    let range = lanruj_core::index::range_text(lang, *start, *end);
                    let row = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    row.add_css_class("index-rand");
                    row.append(&gtk::Label::builder().label(title.as_str()).xalign(0.0).wrap(true).css_classes(["index-titlu"]).build());
                    row.append(&gtk::Label::builder().label(range.as_str()).xalign(0.0).css_classes(["index-zile"]).build());
                    row.set_cursor_from_name(Some("pointer"));
                    row.set_tooltip_text(Some(tr(lang, "Clic: deschide pagina lui; clic dreapta: editează sau scoate", "Click: open its page; right-click: edit or remove")));
                    let click = gtk::GestureClick::builder().button(1).build();
                    let weak = Rc::downgrade(app);
                    click.connect_released(move |_, _, _, _| {
                        if let Some(app) = weak.upgrade() {
                            glib::idle_add_local_once(move || app.open_index_item(i));
                        }
                    });
                    row.add_controller(click);
                    let pop = gtk::Popover::new();
                    let stack = gtk::Stack::builder().vhomogeneous(false).hhomogeneous(false).build();
                    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    let ed = gtk::Button::builder().label(tr(lang, "Editează", "Edit")).css_classes(["flat", "index-editeaza"]).build();
                    let rm = gtk::Button::builder().label(tr(lang, "Scoate din index", "Remove from index")).css_classes(["flat", "error"]).build();
                    for b in [&ed, &rm] {
                        b.child().and_then(|c| c.downcast::<gtk::Label>().ok()).inspect(|l| l.set_xalign(0.0));
                        menu.append(b);
                    }
                    stack.add_named(&menu, Some("meniu"));
                    let form = index_form_box(app, Some((i, title.clone(), *start, *end)), &pop);
                    stack.add_named(&form, Some("editare"));
                    let st = stack.clone();
                    ed.connect_clicked(move |_| st.set_visible_child_name("editare"));
                    let st = stack.clone();
                    pop.connect_closed(move |_| st.set_visible_child_name("meniu"));
                    pop.set_child(Some(&stack));
                    pop.set_parent(&row);
                    let p = pop.clone();
                    row.connect_destroy(move |_| p.unparent());
                    let (weak, p) = (Rc::downgrade(app), pop.clone());
                    rm.connect_clicked(move |_| {
                        p.popdown();
                        if let Some(app) = weak.upgrade() {
                            glib::idle_add_local_once(move || app.index_remove(i));
                        }
                    });
                    let right = gtk::GestureClick::builder().button(3).build();
                    let p = pop.clone();
                    right.connect_pressed(move |g, _, _, _| {
                        p.popup();
                        g.set_state(gtk::EventSequenceState::Claimed);
                    });
                    row.add_controller(right);
                    self.list.append(&row);
                }
                lanruj_core::IndexItem::Other(t) if !t.trim().is_empty() => {
                    self.list.append(&gtk::Label::builder().label(t.as_str()).xalign(0.0).wrap(true).css_classes(["intrare", "altceva"]).build());
                }
                lanruj_core::IndexItem::Other(_) => {}
            }
        }
        if !any {
            self.list.append(
                &gtk::Label::builder()
                    .label(tr(
                        lang,
                        "Aici pui zilele de care vrei să-ți amintești: un concediu, o aniversare, un proiect. Din orice zi: Ctrl+I.",
                        "Put here the days you want to remember: a holiday, an anniversary, a project. From any day: Ctrl+I.",
                    ))
                    .xalign(0.0)
                    .wrap(true)
                    .css_classes(["gol"])
                    .margin_top(10)
                    .build(),
            );
        }
        add
    }

    /// Partea automată a indexului: etichetele cu zilele lor și lunile cu zilele scrise.
    pub fn fill_index_auto(
        &self,
        app: &Rc<App>,
        tags: &std::collections::BTreeMap<String, Vec<NaiveDate>>,
        months: &[((i32, u32), usize)],
    ) {
        let lang = app.lang;
        self.header(tr(lang, "Făcut singur, din jurnal", "Made automatically"), tr(lang, "Etichete și luni", "Tags and months"));
        self.list.append(&gtk::Label::builder().label(tr(lang, "Luni", "Months")).xalign(0.0).css_classes(["index-sectiune"]).build());
        for ((y, m), n) in months {
            let (y, m) = (*y, *m);
            let count = match lang {
                lanruj_core::Lang::Ro => match n {
                    1 => "o zi scrisă".to_string(),
                    n if n % 100 >= 20 => format!("{n} de zile scrise"),
                    n => format!("{n} zile scrise"),
                },
                lanruj_core::Lang::En => if *n == 1 { "1 day written".into() } else { format!("{n} days written") },
            };
            let b = gtk::Button::builder().label(format!("{} · {count}", lang.month_title(y, m))).css_classes(["flat", "index-luna"]).halign(gtk::Align::Start).build();
            let weak = Rc::downgrade(app);
            b.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    glib::idle_add_local_once(move || app.go_month(y, m));
                }
            });
            self.list.append(&b);
        }
        self.list.append(&gtk::Label::builder().label(tr(lang, "Etichete", "Tags")).xalign(0.0).css_classes(["index-sectiune"]).margin_top(14).build());
        if tags.is_empty() {
            self.list.append(
                &gtk::Label::builder()
                    .label(tr(lang, "Etichetele puse sub zile apar aici, fiecare cu zilele ei.", "Tags put under days appear here, each with its days."))
                    .xalign(0.0)
                    .wrap(true)
                    .css_classes(["gol"])
                    .build(),
            );
        }
        // Ordinea alfabetică românească (fără diacritice): „sănătate” stă lângă „sport”.
        let mut sorted: Vec<(&String, &Vec<NaiveDate>)> = tags.iter().collect();
        sorted.sort_by_key(|(t, _)| lanruj_core::search::fold(t));
        for (tag, days) in sorted {
            let flow = gtk::FlowBox::builder().selection_mode(gtk::SelectionMode::None).max_children_per_line(4).column_spacing(4).row_spacing(4).build();
            for d in days {
                let d = *d;
                let b = gtk::Button::builder()
                    .label(format!("{} {} {}", d.day(), lang.month(d.month()), d.year()))
                    .css_classes(["flat", "index-zi"])
                    .build();
                let weak = Rc::downgrade(app);
                b.connect_clicked(move |_| {
                    if let Some(app) = weak.upgrade() {
                        glib::idle_add_local_once(move || app.go(d));
                    }
                });
                flow.append(&b);
            }
            let exp = gtk::Expander::builder().label(format!("{tag}  ({})", days.len())).child(&flow).css_classes(["index-eticheta"]).build();
            self.list.append(&exp);
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
                let mut shown: Vec<String> = texts.iter().map(|t| lanruj_core::colors::plain(t)).collect();
                for t in app.journal.topics_on(d) {
                    if t.day_of.is_some_and(|(k, _)| k == 1) || (t.day_of.is_some() && d.day() == 1) {
                        shown.push(format!("▸ {}", t.title));
                    }
                }
                shown.extend(crate::monitorizare::work_short(lang, &app.journal.work_on(d)));
                let text = gtk::Label::builder()
                    .label(shown.join(" · "))
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

/// Deschide o legătură în programul implicit (browserul, programul de e-mail) și o notează
/// în jurnalul de legături (`~/.local/state/lanruj/legaturi.log`), ca să se vadă dacă a mers.
pub fn open_uri(widget: &impl IsA<gtk::Widget>, uri: &str) {
    fn log(line: &str) {
        let dir = std::env::var_os("XDG_STATE_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| glib::home_dir().join(".local").join("state"))
            .join("lanruj");
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("legaturi.log")) {
            use std::io::Write;
            let _ = writeln!(f, "{} {line}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        }
        eprintln!("legătură: {line}");
    }
    log(&format!("deschid {uri}"));
    // La probe (LANRUJ_NU_DESCHIDE), doar se notează; nu se pornește niciun program.
    if std::env::var_os("LANRUJ_NU_DESCHIDE").is_some() {
        return;
    }
    // În afara Flatpak-ului: direct cu programul implicit (fără portal; prin portal, prima
    // cerere se pierdea la utilizator și legătura se deschidea abia la al doilea clic).
    if std::env::var_os("FLATPAK_ID").is_none() {
        match gtk::gio::AppInfo::launch_default_for_uri(uri, gtk::gio::AppLaunchContext::NONE) {
            Ok(()) => return log(&format!("deschisă direct {uri}")),
            Err(e) => log(&format!("eroare directă {uri}: {e}; încerc prin portal")),
        }
    }
    let window = widget.as_ref().root().and_then(|r| r.downcast::<gtk::Window>().ok());
    let u = uri.to_string();
    gtk::UriLauncher::new(uri).launch(window.as_ref(), gtk::gio::Cancellable::NONE, move |res| match res {
        Ok(()) => log(&format!("deschisă {u}")),
        Err(e) => {
            log(&format!("eroare {u}: {e}; încerc altfel"));
            if let Err(e2) = gtk::gio::AppInfo::launch_default_for_uri(&u, gtk::gio::AppLaunchContext::NONE) {
                log(&format!("eroare și a doua oară {u}: {e2}"));
            }
        }
    });
}

/// Legăturile dintr-o etichetă se deschid cu `open_uri` (nu doar cu felul implicit al GTK).
pub fn hook_links(label: &gtk::Label) {
    label.connect_activate_link(|l, uri| {
        open_uri(l, uri);
        glib::Propagation::Stop
    });
}

/// Textul, cu adresele simple (`https://…`) făcute legături (markup Pango).
fn autolink(text: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let push_word = |w: &str, out: &mut String| {
        // Punctuația de la sfârșit nu face parte din adresă: „vezi https://x.ro.”
        let trimmed = w.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '”']);
        if lanruj_core::colors::is_url(trimmed) {
            let e = glib::markup_escape_text(trimmed);
            out.push_str(&format!("<a href=\"{e}\">{e}</a>"));
            out.push_str(&glib::markup_escape_text(&w[trimmed.len()..]));
        } else if lanruj_core::colors::is_email(trimmed) {
            let e = glib::markup_escape_text(trimmed);
            out.push_str(&format!("<a href=\"mailto:{e}\">{e}</a>"));
            out.push_str(&glib::markup_escape_text(&w[trimmed.len()..]));
        } else {
            out.push_str(&glib::markup_escape_text(w));
        }
    };
    for c in text.chars() {
        if c.is_whitespace() {
            push_word(&word, &mut out);
            word.clear();
            out.push(c);
        } else {
            word.push(c);
        }
    }
    push_word(&word, &mut out);
    out
}

/// Textul intrării cu culorile lui, ca markup Pango. Noaptea, textul colorat se
/// luminează și markerul devine pe jumătate transparent, ca cerneala să se vadă.
pub fn rich_markup(text: &str, night: bool) -> String {
    use lanruj_core::colors::{night_color, runs};
    let mut out = String::new();
    for r in runs(text) {
        // Legăturile: `[text](adresă)` și adresele simple scrise în text.
        let esc = match &r.link {
            Some(url) => format!("<a href=\"{}\">{}</a>", glib::markup_escape_text(url), glib::markup_escape_text(&r.text)),
            None => autolink(&r.text),
        };
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

/// O temă din index pe o zi: banda „Concediu · ziua 1 din 8” și rândurile din pagina ei
/// care încep cu data zilei. Clic = deschide pagina temei. `big` = în pagina zilei (mai mare).
fn topic_box(app: &Rc<App>, t: &lanruj_core::index::TopicOnDay, big: bool) -> gtk::Box {
    let lang = app.lang;
    let b = gtk::Box::new(gtk::Orientation::Vertical, 2);
    b.add_css_class("tema-zi");
    if big {
        b.add_css_class("mare");
    }
    let head = match t.day_of {
        Some((k, n)) if n > 1 => match lang {
            lanruj_core::Lang::Ro => format!("▸ {} · ziua {k} din {n}", t.title),
            lanruj_core::Lang::En => format!("▸ {} · day {k} of {n}", t.title),
        },
        _ => format!("▸ {}", t.title),
    };
    b.append(&gtk::Label::builder().label(head).xalign(0.0).wrap(true).css_classes(["tema-titlu"]).build());
    for (_, e) in &t.entries {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_start(14);
        if e.kind == Kind::Task(TaskState::Done) {
            row.add_css_class("facut");
        }
        row.append(&gtk::Label::builder().label(sign_of(e.kind)).css_classes(["semn"]).valign(gtk::Align::Start).build());
        let l = gtk::Label::builder()
            .label(rich_markup(&e.text, app.window_night()))
            .use_markup(true)
            .xalign(0.0)
            .wrap(true)
            .hexpand(true)
            .css_classes(["text"])
            .build();
        hook_links(&l);
        row.append(&l);
        b.append(&row);
    }
    b.set_tooltip_text(Some(tr(lang, "Clic: deschide pagina din index", "Click: open the index page")));
    b.set_cursor_from_name(Some("pointer"));
    let click = gtk::GestureClick::builder().button(1).build();
    let (weak, i) = (Rc::downgrade(app), t.index);
    click.connect_released(move |_, _, _, _| {
        if let Some(app) = weak.upgrade() {
            glib::idle_add_local_once(move || app.open_index_item(i));
        }
    });
    b.add_controller(click);
    b
}

/// Rândul în editare: un câmp cu intrarea scrisă ca jos („. text”, „o 10:00 text”, „- text”),
/// cu culorile arătate direct (fără etichetele din fișier). Enter salvează, Esc renunță.
fn edit_row(app: &Rc<App>, e: &Entry) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
    row.add_css_class("rand-editare");
    row.set_margin_start(26 * e.depth as i32);
    let view = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .accepts_tab(false)
        .hexpand(true)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(8)
        .right_margin(8)
        .css_classes(["editare"])
        .build();
    view.set_size_request(-1, 34);
    // Chenarul stă pe o cutie din jur; câmpul are doar marginile lui interioare.
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.add_css_class("chenar-editare");
    frame.append(&view);
    let night = app.window_night();
    let quick = e.to_quick();
    // Semnul de la început („. * ”, „o 10:00 ”) rămâne fără culoare; textul își păstrează culorile.
    let prefix = &quick[..quick.len() - e.text.len()];
    load_colored(&view.buffer(), prefix, &e.text, night);
    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(app);
    let v = view.clone();
    keys.connect_key_pressed(move |_, key, _, mods| {
        let Some(app) = weak.upgrade() else { return glib::Propagation::Proceed };
        match key {
            gtk::gdk::Key::Tab | gtk::gdk::Key::ISO_Left_Tab => {
                // Tab în editare: salvează și pune intrarea sub cea de deasupra; Shift+Tab o scoate.
                let right = key == gtk::gdk::Key::Tab && !mods.contains(gtk::gdk::ModifierType::SHIFT_MASK);
                let text = serialize_colored(&v.buffer());
                if let Some((r, i)) = app.editing.get() {
                    glib::idle_add_local_once(move || {
                        app.finish_edit(Some(text));
                        app.indent_entry(r, i, right);
                        app.start_edit(r, i);
                    });
                }
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Escape => {
                glib::idle_add_local_once(move || app.finish_edit(None));
                glib::Propagation::Stop
            }
            gtk::gdk::Key::k | gtk::gdk::Key::K if mods.contains(gtk::gdk::ModifierType::CONTROL_MASK) => {
                // Ctrl+K: fereastra mică pentru legătură, de la butonul ei din bara de culori.
                let mut w = v.parent().and_then(|p| p.parent());
                while let Some(x) = w.clone() {
                    if x.has_css_class("rand-editare") {
                        break;
                    }
                    w = x.parent();
                }
                if let Some(row) = w {
                    fn find(w: &gtk::Widget) -> Option<gtk::MenuButton> {
                        if let Some(m) = w.downcast_ref::<gtk::MenuButton>()
                            && m.has_css_class("buton-legatura")
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
                    if let Some(m) = find(&row) {
                        m.popup();
                    }
                }
                glib::Propagation::Stop
            }
            gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter if !mods.contains(gtk::gdk::ModifierType::SHIFT_MASK) => {
                let text = serialize_colored(&v.buffer());
                glib::idle_add_local_once(move || app.finish_edit(Some(text)));
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    view.add_controller(keys);
    let v = view.clone();
    glib::idle_add_local_once(move || {
        v.grab_focus();
        let buf = v.buffer();
        buf.place_cursor(&buf.end_iter());
    });
    row.append(&frame);
    row.append(&color_bar(app.lang, ColorTarget::Field(view.clone(), night)));
    row.append(
        &gtk::Label::builder()
            .label(tr(app.lang, "Selectează o bucată de text, apoi o culoare sau o legătură (Ctrl+K); fără selecție, culoarea merge pe tot textul. Enter salvează, Esc renunță.", "Select some text, then a color or a link (Ctrl+K); with no selection, the color goes on all the text. Enter saves, Esc cancels."))
            .css_classes(["mic", "dim-label"])
            .wrap(true)
            .xalign(0.0)
            .build(),
    );
    row
}

/// Eticheta de culoare din câmpul de editare. Numele ține culoarea din fișier
/// („f#c01c28” = text, „b#8ff0a4” = marker); afișarea folosește varianta de noapte când e cazul.
/// Eticheta unei legături în câmpul de editare: „l” + adresa; albastră și subliniată.
pub(crate) fn link_tag(buf: &gtk::TextBuffer, url: &str, night: bool) -> gtk::TextTag {
    let name = format!("l{url}");
    if let Some(t) = buf.tag_table().lookup(&name) {
        return t;
    }
    let tag = gtk::TextTag::new(Some(&name));
    tag.set_underline(gtk::pango::Underline::Single);
    tag.set_foreground(Some(if night { "#99c1f1" } else { "#1c71d8" }));
    buf.tag_table().add(&tag);
    tag
}

pub(crate) fn color_tag(buf: &gtk::TextBuffer, marker: bool, hex: &str, night: bool) -> gtk::TextTag {
    let name = format!("{}{}", if marker { 'b' } else { 'f' }, hex.to_lowercase());
    if let Some(t) = buf.tag_table().lookup(&name) {
        return t;
    }
    let tag = gtk::TextTag::new(Some(&name));
    if marker {
        if let Ok(mut c) = gtk::gdk::RGBA::parse(hex) {
            if night {
                c.set_alpha(0.4);
            }
            tag.set_background_rgba(Some(&c));
        }
        if !night {
            tag.set_foreground(Some("#1c3d6b"));
        }
    } else {
        let shown = if night { lanruj_core::colors::night_color(hex) } else { hex };
        tag.set_foreground(Some(shown));
    }
    buf.tag_table().add(&tag);
    // Culoarea textului are întâietate față de cerneala pusă pe marker:
    // markerele stau cele mai de jos, culorile cele mai de sus.
    if marker {
        tag.set_priority(0);
    } else {
        tag.set_priority(buf.tag_table().size() - 1);
    }
    tag
}

/// Pune în câmp semnul de început și textul, cu culorile lui.
fn load_colored(buf: &gtk::TextBuffer, prefix: &str, text: &str, night: bool) {
    buf.set_text(prefix);
    for r in lanruj_core::colors::runs(text) {
        let start = buf.end_iter().offset();
        let mut end = buf.end_iter();
        buf.insert(&mut end, &r.text);
        let (a, b) = (buf.iter_at_offset(start), buf.end_iter());
        if let Some(c) = &r.color {
            buf.apply_tag(&color_tag(buf, false, c, night), &a, &b);
        }
        if let Some(m) = &r.marker {
            buf.apply_tag(&color_tag(buf, true, m, night), &a, &b);
        }
        if let Some(l) = &r.link {
            buf.apply_tag(&link_tag(buf, l, night), &a, &b);
        }
    }
}

/// Textul din câmp, înapoi în Markdown cu etichetele de culoare.
pub fn serialize_colored(buf: &gtk::TextBuffer) -> String {
    use lanruj_core::colors::{Run, to_markdown};
    let mut runs: Vec<Run> = Vec::new();
    let mut it = buf.start_iter();
    while !it.is_end() {
        let c = it.char();
        let (mut color, mut marker, mut link) = (None, None, None);
        for t in it.tags() {
            let name = t.name().map(|n| n.to_string()).unwrap_or_default();
            if let Some(h) = name.strip_prefix('f') {
                color = Some(h.to_string());
            } else if let Some(h) = name.strip_prefix('b') {
                marker = Some(h.to_string());
            } else if let Some(u) = name.strip_prefix('l') {
                link = Some(u.to_string());
            }
        }
        match runs.last_mut() {
            Some(last) if last.color == color && last.marker == marker && last.link == link => last.text.push(c),
            _ => runs.push(Run { text: c.to_string(), color, marker, link }),
        }
        it.forward_char();
    }
    // Un rând nou scris din greșeală (Shift+Enter) devine spațiu: intrarea rămâne pe un rând.
    to_markdown(&runs).replace('\n', " ")
}

/// Pune culoarea pe bucata selectată sau, fără selecție, pe tot textul (fără semnul de la început).
fn paint_view(view: &gtk::TextView, paint: Option<lanruj_core::colors::Paint>, night: bool) {
    use lanruj_core::colors::Paint;
    let buf = view.buffer();
    let (a, b) = match buf.selection_bounds() {
        Some((a, b)) => (a, b),
        // În jurnal, fără selecție: cuvântul de la cursor (nu tot textul).
        None if view.has_css_class("jurnal-text") => {
            let at = buf.iter_at_mark(&buf.get_insert());
            let (mut a, mut b) = (at, at);
            if !a.starts_word() {
                a.backward_word_start();
            }
            if !b.ends_word() {
                b.forward_word_end();
            }
            if !at.inside_word() && !at.ends_word() {
                view.grab_focus();
                return;
            }
            (a, b)
        }
        None => {
            let full = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
            let start = match lanruj_core::parse_quick(&full) {
                Some(p) if full.trim_end().ends_with(&p.text) => full.trim_end().chars().count() - p.text.chars().count(),
                _ => 0,
            };
            (buf.iter_at_offset(start as i32), buf.end_iter())
        }
    };
    // Scoatem culorile de același fel (sau toate, pentru „fără culoare”), apoi o punem pe cea nouă.
    let names: Vec<gtk::TextTag> = {
        let mut v = Vec::new();
        buf.tag_table().foreach(|t| v.push(t.clone()));
        v
    };
    for t in &names {
        let name = t.name().map(|n| n.to_string()).unwrap_or_default();
        let remove = match paint {
            None => true,
            Some(Paint::Color(_)) => name.starts_with('f'),
            Some(Paint::Marker(_)) => name.starts_with('b'),
        };
        if remove {
            buf.remove_tag(t, &a, &b);
        }
    }
    match paint {
        Some(Paint::Color(c)) => buf.apply_tag(&color_tag(&buf, false, c, night), &a, &b),
        Some(Paint::Marker(m)) => buf.apply_tag(&color_tag(&buf, true, m, night), &a, &b),
        None => {}
    }
    view.grab_focus();
}

/// Formularul unui rând din index: titlul, ziua și, opțional, ziua de sfârșit.
/// Cu `edit` = Some((poziția, titlul, început, sfârșit)), schimbă rândul acela; altfel adaugă unul nou.
fn index_form_box(app: &Rc<App>, edit: Option<(usize, String, NaiveDate, Option<NaiveDate>)>, pop: &gtk::Popover) -> gtk::Box {
    let lang = app.lang;
    let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
    b.set_margin_top(6);
    b.set_margin_bottom(6);
    b.set_margin_start(6);
    b.set_margin_end(6);
    let title = gtk::Entry::builder().placeholder_text(tr(lang, "De exemplu: Concediu la mare", "For example: Holiday at the sea")).width_chars(28).build();
    title.add_css_class("index-form-titlu");
    b.append(&title);
    b.append(&gtk::Label::builder().label(tr(lang, "Ziua", "Day")).xalign(0.0).css_classes(["heading"]).build());
    let start = gtk::Calendar::new();
    b.append(&start);
    let more = gtk::CheckButton::with_label(tr(lang, "Mai multe zile, până la:", "Several days, until:"));
    b.append(&more);
    let end = gtk::Calendar::builder().visible(false).build();
    more.bind_property("active", &end, "visible").sync_create().build();
    b.append(&end);
    let ok = gtk::Button::builder()
        .label(if edit.is_some() { tr(lang, "Salvează", "Save") } else { tr(lang, "Adaugă", "Add") })
        .css_classes(["suggested-action"])
        .build();
    b.append(&ok);
    let set = |c: &gtk::Calendar, d: NaiveDate| {
        if let Ok(dt) = glib::DateTime::from_local(d.year(), d.month() as i32, d.day() as i32, 12, 0, 0.0) {
            c.select_day(&dt);
        }
    };
    match &edit {
        Some((_, t, s, e)) => {
            title.set_text(t);
            set(&start, *s);
            set(&end, e.unwrap_or(*s));
            more.set_active(e.is_some());
        }
        None => {
            // Ziua propusă: cea arătată (sau cea cerută cu Ctrl+I).
            let d = app.index_add_date.get().unwrap_or(app.today.get());
            set(&start, d);
            set(&end, d);
        }
    }
    let to_date = |c: &gtk::Calendar| {
        let dt = c.date();
        NaiveDate::from_ymd_opt(dt.year(), dt.month() as u32, dt.day_of_month() as u32)
    };
    let pos = edit.as_ref().map(|(i, ..)| *i);
    let (weak, p) = (Rc::downgrade(app), pop.clone());
    let (t, sc, ec, m) = (title.clone(), start.clone(), end.clone(), more.clone());
    let submit = move || {
        let text = t.text().to_string();
        let Some(app) = weak.upgrade() else { return };
        if text.trim().is_empty() {
            app.toast(tr(app.lang, "Scrie un titlu pentru rândul din index.", "Type a title for the index line."));
            return;
        }
        let Some(s) = to_date(&sc) else { return };
        let e = if m.is_active() { to_date(&ec) } else { None };
        p.popdown();
        glib::idle_add_local_once(move || match pos {
            Some(i) => app.index_edit(i, &text, s, e),
            None => app.index_add(&text, s, e),
        });
    };
    let sub = submit.clone();
    ok.connect_clicked(move |_| sub());
    title.connect_activate(move |_| submit());
    let t = title.clone();
    pop.connect_show(move |_| {
        t.grab_focus();
    });
    b
}

/// Unde pune bara de culori culoarea: în câmpul de editare (pe bucata selectată
/// sau pe tot textul) ori direct pe o intrare (din meniul de la clic dreapta).
#[derive(Clone)]
pub enum ColorTarget {
    /// Câmpul de editare și dacă e noapte (pentru culorile arătate).
    Field(gtk::TextView, bool),
    Entry(std::rc::Weak<App>, PageRef, usize, gtk::Popover),
}

/// Butonul de legătură (și Ctrl+K în câmp): pune adresa pe textul selectat; fără selecție,
/// scrie adresa la cursor, ca legătură. „Scoate legătura” o scoate de pe selecție.
fn link_button(lang: lanruj_core::Lang, view: &gtk::TextView, night: bool) -> gtk::MenuButton {
    let mb = gtk::MenuButton::builder()
        .icon_name("insert-link-symbolic")
        .focus_on_click(false)
        .css_classes(["flat", "buton-legatura"])
        .tooltip_text(tr(lang, "Legătură (Ctrl+K): selectează textul, apoi lipește adresa", "Link (Ctrl+K): select the text, then paste the address"))
        .build();
    let pop = gtk::Popover::new();
    let b = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let url = gtk::Entry::builder().placeholder_text("https://…").width_chars(34).css_classes(["adresa-legatura"]).build();
    b.append(&url);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let put = gtk::Button::builder().label(tr(lang, "Pune legătura", "Add link")).css_classes(["suggested-action"]).build();
    let rm = gtk::Button::builder().label(tr(lang, "Scoate legătura", "Remove link")).build();
    row.append(&put);
    row.append(&rm);
    b.append(&row);
    pop.set_child(Some(&b));
    mb.set_popover(Some(&pop));
    // Selecția din câmp se ține minte când se deschide fereastra mică (focusul pleacă în ea).
    let sel: std::rc::Rc<std::cell::Cell<Option<(i32, i32)>>> = Default::default();
    let (v, sc, u) = (view.clone(), sel.clone(), url.clone());
    pop.connect_show(move |_| {
        let buf = v.buffer();
        sc.set(buf.selection_bounds().map(|(a, b)| (a.offset(), b.offset())));
        u.set_text("");
        u.grab_focus();
    });
    let apply = {
        let (v, sc, p) = (view.clone(), sel.clone(), pop.clone());
        move |address: Option<String>| {
            let buf = v.buffer();
            // Tag-urile de legătură existente pe selecție se scot întâi.
            let remove_links = |a: &gtk::TextIter, b: &gtk::TextIter| {
                let mut tags = Vec::new();
                buf.tag_table().foreach(|t| {
                    if t.name().is_some_and(|n| n.starts_with('l')) {
                        tags.push(t.clone());
                    }
                });
                for t in &tags {
                    buf.remove_tag(t, a, b);
                }
            };
            match (sc.get(), address) {
                (Some((a, b)), Some(url)) => {
                    let (ia, ib) = (buf.iter_at_offset(a), buf.iter_at_offset(b));
                    remove_links(&ia, &ib);
                    buf.apply_tag(&link_tag(&buf, &url, night), &ia, &ib);
                }
                (None, Some(url)) => {
                    // Fără selecție: adresa se scrie la cursor, ca legătură.
                    let mut at = buf.iter_at_mark(&buf.get_insert());
                    let start = at.offset();
                    buf.insert(&mut at, &url);
                    let (ia, ib) = (buf.iter_at_offset(start), buf.iter_at_offset(start + url.chars().count() as i32));
                    buf.apply_tag(&link_tag(&buf, &url, night), &ia, &ib);
                }
                (Some((a, b)), None) => remove_links(&buf.iter_at_offset(a), &buf.iter_at_offset(b)),
                (None, None) => {}
            }
            p.popdown();
            v.grab_focus();
        }
    };
    let (ap, u) = (apply.clone(), url.clone());
    put.connect_clicked(move |_| {
        let t = u.text().trim().to_string();
        if !t.is_empty() {
            ap(Some(normalize_url(&t)));
        }
    });
    let (ap, u) = (apply.clone(), url.clone());
    url.connect_activate(move |_| {
        let t = u.text().trim().to_string();
        if !t.is_empty() {
            ap(Some(normalize_url(&t)));
        }
    });
    rm.connect_clicked(move |_| apply(None));
    mb
}

/// „exemplu.ro” → „https://exemplu.ro”; o adresă de e-mail → „mailto:…”.
fn normalize_url(t: &str) -> String {
    if lanruj_core::colors::is_url(t) {
        t.to_string()
    } else if t.contains('@') && !t.contains('/') {
        format!("mailto:{t}")
    } else {
        format!("https://{t}")
    }
}

/// Bara de culori: textul colorat (A colorat), markerul (pătrățele) și „fără culoare”.
pub fn color_bar(lang: lanruj_core::Lang, target: ColorTarget) -> gtk::Box {
    use lanruj_core::colors::{MARKER_COLORS, Paint, TEXT_COLORS};
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    bar.add_css_class("bara-culori");
    let apply = move |target: &ColorTarget, paint: Option<Paint>| match target {
        ColorTarget::Field(v, night) => paint_view(v, paint, *night),
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
    if let ColorTarget::Field(v, night) = &target {
        bar.append(&link_button(lang, v, *night));
    }
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
fn add_entry_menu(app: &Rc<App>, row: &gtk::Box, date: PageRef, index: usize, movable: bool, urls: Vec<String>) {
    let lang = app.lang;
    let pop = gtk::Popover::builder().has_arrow(true).build();
    let stack = gtk::Stack::builder().vhomogeneous(false).hhomogeneous(false).transition_type(gtk::StackTransitionType::SlideLeftRight).build();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let edit = gtk::Button::builder().label(tr(lang, "Editează", "Edit")).css_classes(["flat"]).build();
    let mv = gtk::Button::builder().label(tr(lang, "Mută…", "Move…")).css_classes(["flat", "muta"]).visible(movable).build();
    let col = gtk::Button::builder().label(tr(lang, "Culoare…", "Color…")).css_classes(["flat", "culoare"]).build();
    let ind = gtk::Button::builder().label(tr(lang, "Sub intrarea de deasupra (Tab)", "Under the entry above (Tab)")).css_classes(["flat"]).build();
    let outd = gtk::Button::builder().label(tr(lang, "Scoate de sub ea (Shift+Tab)", "Out from under it (Shift+Tab)")).css_classes(["flat"]).build();
    for (b, right) in [(&ind, true), (&outd, false)] {
        let (weak, p) = (Rc::downgrade(app), pop.clone());
        b.connect_clicked(move |_| {
            p.popdown();
            if let Some(app) = weak.upgrade() {
                glib::idle_add_local_once(move || app.indent_entry(date, index, right));
            }
        });
    }
    let pics = gtk::Button::builder().label(tr(lang, "Adaugă poze sau fișiere…", "Add pictures or files…")).css_classes(["flat", "adauga-poze"]).build();
    let (weak, p) = (Rc::downgrade(app), pop.clone());
    pics.connect_clicked(move |_| {
        p.popdown();
        if let Some(app) = weak.upgrade() {
            glib::idle_add_local_once(move || app.pick_files_under(date, index));
        }
    });
    let del = gtk::Button::builder().label(tr(lang, "Șterge", "Delete")).css_classes(["flat", "error"]).build();
    let about = gtk::Button::builder().label(tr(lang, "Scrie în jurnal despre asta", "Write about it in the journal")).css_classes(["flat", "scrie-jurnal"]).build();
    let (weak, p) = (Rc::downgrade(app), pop.clone());
    about.connect_clicked(move |_| {
        p.popdown();
        if let Some(app) = weak.upgrade() {
            glib::idle_add_local_once(move || app.write_about_entry(date, index));
        }
    });
    // Legăturile din intrare: câte un rând „Deschide …”, cale sigură pe lângă clicul pe legătură.
    for u in &urls {
        let shown = u.strip_prefix("mailto:").map(|m| format!("{} {m}", tr(lang, "Scrie la", "Write to"))).unwrap_or_else(|| {
            let short: String = u.trim_start_matches("https://").trim_start_matches("http://").chars().take(40).collect();
            format!("{} {short}", tr(lang, "Deschide", "Open"))
        });
        let b = gtk::Button::builder().label(shown).css_classes(["flat", "deschide-legatura"]).build();
        b.child().and_then(|c| c.downcast::<gtk::Label>().ok()).inspect(|l| l.set_xalign(0.0));
        let (p, u2) = (pop.clone(), u.clone());
        b.connect_clicked(move |btn| {
            p.popdown();
            open_uri(btn, &u2);
        });
        menu.append(&b);
    }
    if !urls.is_empty() {
        menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    }
    for b in [&edit, &mv, &col, &about, &pics, &ind, &outd, &del] {
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

    // Poze trase peste intrare: se leagă de ea.
    let drop = gtk::DropTarget::new(gtk::gdk::FileList::static_type(), gtk::gdk::DragAction::COPY);
    let weak = Rc::downgrade(app);
    drop.connect_drop(move |_, value, _, _| {
        let (Some(app), Ok(list)) = (weak.upgrade(), value.get::<gtk::gdk::FileList>()) else { return false };
        let files: Vec<std::path::PathBuf> = list.files().iter().filter_map(|f| f.path()).collect();
        glib::idle_add_local_once(move || app.attach_files_under(date, files, Some(index)));
        true
    });
    row.add_controller(drop);
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
    if mig.is_none() && !app.exporting.get() && app.editing.get() == Some((r, index)) {
        return edit_row(app, e);
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("intrare");
    if mig.is_none() && !app.exporting.get() && app.selected.get() == Some((r, index)) {
        row.add_css_class("selectat");
    }
    if mig.is_none() {
        let movable = e.is_open_task() || matches!(e.kind, Kind::Event | Kind::Note);
        add_entry_menu(app, &row, r, index, movable && e.depth == 0, lanruj_core::colors::urls(&e.text));
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
    hook_links(&label);
    text_box.append(&label);

    // ↻N: de câte ori a fost mutată; de la 3, „mai merită?”.
    if e.is_open_task()
        && let Some(id) = &e.id {
            let n = app.journal.migration_count(id);
            if n > 0 {
                let m = gtk::Label::builder().label(format!("↻{n}")).css_classes(["migrari"]).valign(gtk::Align::Start).build();
                m.set_margin_top(3);
                if n >= app.settings.borrow().worth as usize {
                    m.add_css_class("mult");
                    m.set_label(&format!("↻{n} {}", tr(lang, "mai merită?", "still worth it?")));
                }
                m.set_tooltip_text(Some(&format!("{} {n}", tr(lang, "Mutată de", "Moved"))));
                text_box.append(&m);
            }
        }
    // Pomenit în jurnal: un semn mic care duce la ziua de jurnal care vorbește despre rând.
    if mig.is_none()
        && !app.exporting.get()
        && let Some(id) = &e.id
    {
        let m = app.entry_mentions(r, id);
        if let Some((d, _)) = m.first() {
            let d = *d;
            let b = gtk::Button::builder().icon_name("accessories-text-editor-symbolic").css_classes(["flat", "pomenit"]).valign(gtk::Align::Start).build();
            let days: Vec<String> = m.iter().map(|(d, t)| format!("{} · „{}”", lang.day_title_short(*d), t.chars().take(40).collect::<String>())).collect();
            b.set_tooltip_text(Some(&format!("{}\n{}", tr(lang, "Pomenit în jurnal:", "Mentioned in the journal:"), days.join("\n"))));
            let weak = Rc::downgrade(app);
            b.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.go_diary(d);
                }
            });
            text_box.append(&b);
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
