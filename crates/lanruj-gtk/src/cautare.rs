// SPDX-License-Identifier: GPL-3.0-or-later
//! Căutarea (Ctrl+F): un câmp, filtre și rezultatele grupate pe pagini.
//! Un clic pe un rezultat deschide pagina lui, cu intrarea aleasă; Esc închide căutarea.

use crate::{App, tr};
use adw::prelude::*;
use gtk::glib;
use chrono::{Datelike, NaiveDate};
use lanruj_core::search::{Query, fold};
use lanruj_core::{Entry, Filter, Kind, Lang, PageRef, Priority, TaskState};
use std::cell::RefCell;
use std::rc::Rc;

const LIMIT: usize = 300;

/// Ce bifează în filtru un buton.
type SetFilter = fn(&mut Filter);

/// Unde duce un rezultat.
#[derive(Clone, Debug)]
pub enum Goto {
    Entry(PageRef, usize),
    Page(PageRef),
    /// Ziua de jurnal și rândul.
    Diary(NaiveDate, usize),
    /// Al câtelea bilet.
    Note(usize),
    /// Monitorizarea: luna, fila, ziua.
    Tracking(i32, u32, &'static str, Option<u32>),
    Index,
}

/// Locurile în care se caută, pe lângă pagini.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Diary,
    Notes,
    Tracking,
}

pub struct Cautare {
    pub root: gtk::Box,
    pub entry: gtk::SearchEntry,
    filters: Vec<(gtk::ToggleButton, SetFilter)>,
    sources: Vec<(gtk::ToggleButton, Source)>,
    /// Cuvintele căutate acum (pentru deschiderea jurnalului la locul lor).
    pub words: RefCell<Vec<String>>,
    list: gtk::ListBox,
    status: adw::StatusPage,
    scroller: gtk::ScrolledWindow,
    count: gtk::Label,
    /// Ce deschide fiecare rând al listei (titlurile de pagină nu deschid nimic).
    targets: RefCell<Vec<Option<Goto>>>,
}

/// Textul cu cuvintele căutate îngroșate (fără să țină cont de diacritice).
fn highlight(text: &str, words: &[String]) -> String {
    let chars: Vec<char> = text.chars().collect();
    let folded: Vec<char> = chars.iter().map(|c| fold(&c.to_string()).chars().next().unwrap_or(*c)).collect();
    let mut mark = vec![false; chars.len()];
    for w in words {
        let w: Vec<char> = w.chars().collect();
        if w.is_empty() || w.len() > folded.len() {
            continue;
        }
        for i in 0..=folded.len() - w.len() {
            if folded[i..i + w.len()] == w[..] {
                mark[i..i + w.len()].iter_mut().for_each(|m| *m = true);
            }
        }
    }
    let mut out = String::new();
    let mut open = false;
    for (c, m) in chars.iter().zip(mark) {
        if m && !open {
            out.push_str("<b><u>");
            open = true;
        } else if !m && open {
            out.push_str("</u></b>");
            open = false;
        }
        out.push_str(&glib::markup_escape_text(&c.to_string()));
    }
    if open {
        out.push_str("</u></b>");
    }
    out
}

fn sign(k: Kind) -> &'static str {
    match k {
        Kind::Task(TaskState::Open) | Kind::Task(TaskState::Dropped) => "•",
        Kind::Task(TaskState::Done) => "×",
        Kind::Task(TaskState::Migrated) => ">",
        Kind::Task(TaskState::Scheduled) => "<",
        Kind::Event => "○",
        Kind::Note => "–",
    }
}

impl Cautare {
    pub fn new(lang: Lang) -> Cautare {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.add_css_class("cautare");
        let top = gtk::Box::new(gtk::Orientation::Vertical, 10);
        top.set_margin_top(18);
        top.set_margin_start(18);
        top.set_margin_end(18);
        let entry = gtk::SearchEntry::builder()
            .placeholder_text(tr(lang, "Caută peste tot: cuvinte, „o expresie”, -fără-cuvânt", "Search everywhere: words, \"a phrase\", -without-word"))
            .hexpand(true)
            .build();
        top.append(&entry);
        let fbox = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let mut filters: Vec<(gtk::ToggleButton, SetFilter)> = Vec::new();
        let defs: [(&str, &str, SetFilter); 5] = [
            (tr(lang, "• Sarcini", "• Tasks"), "", |f| f.tasks = true),
            (tr(lang, "○ Evenimente", "○ Events"), "", |f| f.events = true),
            (tr(lang, "– Notițe", "– Notes"), "", |f| f.notes = true),
            (tr(lang, "★ Importante", "★ Important"), "", |f| f.important = true),
            (tr(lang, "! Idei", "! Ideas"), "", |f| f.ideas = true),
        ];
        for (label, _, set) in defs {
            let b = gtk::ToggleButton::builder().label(label).css_classes(["filtru"]).build();
            fbox.append(&b);
            filters.push((b, set));
        }
        let mut sources: Vec<(gtk::ToggleButton, Source)> = Vec::new();
        let sbox = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for (label, src) in [
            (tr(lang, "Jurnal", "Journal"), Source::Diary),
            (tr(lang, "Bilete", "Notes"), Source::Notes),
            (tr(lang, "Monitorizare", "Tracking"), Source::Tracking),
        ] {
            let b = gtk::ToggleButton::builder().label(label).css_classes(["filtru"]).build();
            sbox.append(&b);
            sources.push((b, src));
        }
        let count = gtk::Label::builder().css_classes(["dim-label"]).hexpand(true).xalign(1.0).build();
        sbox.append(&count);
        // Pe fereastră îngustă, filtrele trec pe rândul următor.
        let wrap = adw::WrapBox::builder().child_spacing(6).line_spacing(6).build();
        wrap.append(&fbox);
        wrap.append(&sbox);
        top.append(&wrap);
        let clamp = adw::Clamp::builder().maximum_size(820).child(&top).build();
        root.append(&clamp);

        let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list", "rezultate"]).valign(gtk::Align::Start).build();
        let lclamp = adw::Clamp::builder().maximum_size(820).child(&list).build();
        lclamp.set_margin_start(18);
        lclamp.set_margin_end(18);
        lclamp.set_margin_bottom(18);
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&lclamp).build();
        let status = adw::StatusPage::builder()
            .icon_name("edit-find-symbolic")
            .title(tr(lang, "Caută în jurnal", "Search the journal"))
            .description(tr(
                lang,
                "Caută în pagini, în jurnal, în bilete, în Monitorizare și în Index. Literele mari și diacriticele nu contează. „Între ghilimele” caută exact expresia, iar -cuvânt scoate rezultatele care îl conțin. Enter deschide primul rezultat; ↓ trece în listă.",
                "Searches the pages, the journal, the notes, Tracking and the Index. Case and accents don't matter. \"In quotes\" finds the exact phrase, and -word leaves out results that contain it. Enter opens the first result; ↓ moves into the list.",
            ))
            .vexpand(true)
            .build();
        root.append(&status);
        root.append(&scroller);
        scroller.set_visible(false);

        Cautare { root, entry, filters, sources, words: RefCell::new(Vec::new()), list, status, scroller, count, targets: RefCell::new(Vec::new()) }
    }

    /// Leagă câmpul, filtrele și lista de aplicație.
    pub fn connect(self: &Rc<Self>, app: &Rc<App>) {
        let weak = Rc::downgrade(app);
        self.entry.connect_search_changed(move |_| {
            if let Some(a) = weak.upgrade() {
                a.cautare.run(&a);
            }
        });
        let weak = Rc::downgrade(app);
        self.entry.connect_stop_search(move |_| {
            if let Some(a) = weak.upgrade() {
                a.close_search();
            }
        });
        let buttons: Vec<gtk::ToggleButton> = self.filters.iter().map(|(b, _)| b.clone()).chain(self.sources.iter().map(|(b, _)| b.clone())).collect();
        for b in &buttons {
            let weak = Rc::downgrade(app);
            b.connect_toggled(move |_| {
                if let Some(a) = weak.upgrade() {
                    a.cautare.run(&a);
                }
            });
        }
        let weak = Rc::downgrade(app);
        self.list.connect_row_activated(move |_, row| {
            let Some(a) = weak.upgrade() else { return };
            let target = a.cautare.targets.borrow().get(row.index() as usize).cloned().flatten();
            if let Some(g) = target {
                glib::idle_add_local_once(move || a.open_goto(g));
            }
        });
        // Enter în câmp: primul rezultat. ↓: în listă.
        let weak = Rc::downgrade(app);
        self.entry.connect_activate(move |_| {
            let Some(a) = weak.upgrade() else { return };
            let first = a.cautare.targets.borrow().iter().flatten().next().cloned();
            if let Some(g) = first {
                glib::idle_add_local_once(move || a.open_goto(g));
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(app);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(a) = weak.upgrade() else { return glib::Propagation::Proceed };
            if key == gtk::gdk::Key::Down {
                let idx = a.cautare.targets.borrow().iter().position(|t| t.is_some());
                if let Some(row) = idx.and_then(|i| a.cautare.list.row_at_index(i as i32)) {
                    row.grab_focus();
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        self.entry.add_controller(keys);
    }

    #[cfg(debug_assertions)]
    pub fn first_target(&self) -> Option<(PageRef, usize)> {
        self.targets.borrow().iter().flatten().find_map(|g| match g {
            Goto::Entry(r, i) => Some((*r, *i)),
            _ => None,
        })
    }

    fn page_title(app: &App, r: PageRef) -> String {
        let l = app.lang;
        match r {
            PageRef::Day(d) => l.day_title(d),
            PageRef::Month { year, month } => format!("{} · {}", l.month_title(year, month), tr(l, "lista lunii", "monthly list")),
            PageRef::Week { year, week } => format!("{} · {}", l.week_title(week), l.week_range(lanruj_core::week_monday(year, week))),
            PageRef::Future { year, month } => format!("{} · {}", tr(l, "Planuri", "Future log"), l.month_title(year, month)),
            PageRef::Topic { .. } => {
                let title = app.journal.load_ref(r).ok().and_then(|d| d.heading).unwrap_or_default();
                format!("Index · {title}")
            }
        }
    }

    pub fn run(&self, app: &Rc<App>) {
        let query = self.entry.text().to_string();
        let mut filter = Filter::default();
        for (b, set) in &self.filters {
            if b.is_active() {
                set(&mut filter);
            }
        }
        let chosen: Vec<Source> = self.sources.iter().filter(|(b, _)| b.is_active()).map(|(_, s)| *s).collect();
        let kinds = filter != Filter::default();
        // Fără nimic ales: peste tot. Cu felul intrărilor ales: doar paginile (și locurile alese).
        let pages_on = chosen.is_empty() || kinds;
        let on = |s: Source| (chosen.is_empty() && !kinds) || chosen.contains(&s);
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        let q = Query::parse(&query);
        let words = q.must.clone();
        *self.words.borrow_mut() = words.clone();
        let mut targets: Vec<Option<Goto>> = Vec::new();
        if words.is_empty() && !kinds && chosen.is_empty() {
            self.status.set_visible(true);
            self.scroller.set_visible(false);
            self.status.set_title(tr(app.lang, "Caută în jurnal", "Search the journal"));
            self.count.set_label("");
            *self.targets.borrow_mut() = targets;
            return;
        }
        let l = app.lang;
        // Grupele: (ordinea, titlul, rândurile), cele mai noi primele.
        type Rows = Vec<(gtk::ListBoxRow, Goto)>;
        type Group = ((i32, u32, u32, u8), String, Rows);
        let mut groups: Vec<Group> = Vec::new();
        let far = (i32::MAX, 0, 0, 0);
        if on(Source::Notes) {
            let notes = app.journal.search_notes(&q);
            if !notes.is_empty() && (!words.is_empty() || chosen.contains(&Source::Notes)) {
                let rows = notes.into_iter().map(|(i, t)| (plain_row("🗒", &highlight(&t.replace('\n', " · "), &words)), Goto::Note(i))).collect();
                groups.push((far, tr(l, "Bilete", "Notes").to_string(), rows));
            }
        }
        if pages_on && !words.is_empty() && !kinds {
            let idx = app.journal.search_index(&q);
            if !idx.is_empty() {
                let rows = idx
                    .into_iter()
                    .map(|(title, start, page)| {
                        let g = match page {
                            Some((date, n)) => Goto::Page(PageRef::Topic { date, n }),
                            None => Goto::Index,
                        };
                        (plain_row("☰", &format!("{} · {}", highlight(&title, &words), glib::markup_escape_text(&l.day_title_short(start)))), g)
                    })
                    .collect();
                groups.push(((i32::MAX, 0, 0, 1), "Index".to_string(), rows));
            }
        }
        if pages_on {
            for (r, hits) in app.journal.search(&query, filter, LIMIT) {
                let key = match r {
                    PageRef::Day(d) | PageRef::Topic { date: d, .. } => (d.year(), d.month(), d.day(), 2),
                    PageRef::Week { year, week } => {
                        let d = lanruj_core::week_monday(year, week);
                        (d.year(), d.month(), d.day(), 3)
                    }
                    PageRef::Month { year, month } | PageRef::Future { year, month } => (year, month, 0, 4),
                };
                let rows = hits.into_iter().map(|(i, e)| (result_row(&e, &words), if i == usize::MAX { Goto::Page(r) } else { Goto::Entry(r, i) })).collect();
                groups.push((key, Self::page_title(app, r), rows));
            }
        }
        if on(Source::Diary) && (!words.is_empty() || chosen.contains(&Source::Diary)) {
            for (d, lines) in app.journal.search_diary(&q, LIMIT) {
                let rows = lines.into_iter().map(|(n, t)| (plain_row("¶", &highlight(&snippet(&t, &words), &words)), Goto::Diary(d, n))).collect();
                groups.push(((d.year(), d.month(), d.day(), 1), format!("{} · {}", tr(l, "Jurnal", "Journal"), l.day_title(d)), rows));
            }
        }
        if on(Source::Tracking) && !words.is_empty() {
            let mut by_month: Vec<((i32, u32), Rows)> = Vec::new();
            for h in app.journal.search_tracking(&q) {
                let day = h.day.map(|d| format!("{d} {} · ", l.month(h.month).to_lowercase())).unwrap_or_default();
                let tab = match h.tab {
                    "somn" => tr(l, "Somn", "Sleep"),
                    "serviciu" => tr(l, "Serviciu", "Work"),
                    _ => tr(l, "Activități", "Activities"),
                };
                let row = plain_row("▤", &format!("{}{} · {}", glib::markup_escape_text(&day), tab, highlight(&h.text, &words)));
                let g = Goto::Tracking(h.year, h.month, h.tab, h.day);
                match by_month.iter_mut().find(|(k, _)| *k == (h.year, h.month)) {
                    Some((_, v)) => v.push((row, g)),
                    None => by_month.push(((h.year, h.month), vec![(row, g)])),
                }
            }
            for ((y, m), rows) in by_month {
                groups.push(((y, m, 0, 5), format!("{} · {}", tr(l, "Monitorizare", "Tracking"), l.month_title(y, m)), rows));
            }
        }
        groups.sort_by_key(|g| std::cmp::Reverse(g.0));
        let total: usize = groups.iter().map(|g| g.2.len()).sum();
        self.count.set_label(&match l {
            Lang::Ro => match total {
                0 => String::new(),
                1 => "un rezultat".into(),
                n if n % 100 >= 20 || n % 100 == 0 => format!("{n} de rezultate"),
                n => format!("{n} rezultate"),
            },
            Lang::En => if total == 1 { "1 result".into() } else { format!("{total} results") },
        });
        if groups.is_empty() {
            self.status.set_visible(true);
            self.scroller.set_visible(false);
            self.status.set_title(tr(l, "Nimic găsit", "Nothing found"));
            *self.targets.borrow_mut() = targets;
            return;
        }
        self.status.set_visible(false);
        self.scroller.set_visible(true);
        for (_, title, rows) in groups {
            let head = gtk::ListBoxRow::builder().activatable(false).selectable(false).focusable(false).css_classes(["titlu-pagina-rezultat"]).build();
            head.set_child(Some(&gtk::Label::builder().label(title).xalign(0.0).css_classes(["heading", "rezultat-pagina"]).build()));
            self.list.append(&head);
            targets.push(None);
            for (row, g) in rows {
                self.list.append(&row);
                targets.push(Some(g));
            }
        }
        *self.targets.borrow_mut() = targets;
    }
}

/// Un rând lung, tăiat în jurul primului cuvânt găsit.
fn snippet(text: &str, words: &[String]) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 180 {
        return text.to_string();
    }
    let folded = fold(text);
    let pos = words.iter().filter_map(|w| folded.find(w.as_str())).min().map(|b| folded[..b].chars().count()).unwrap_or(0);
    let start = pos.saturating_sub(60);
    let end = (start + 180).min(chars.len());
    let mut s: String = chars[start..end].iter().collect();
    if start > 0 {
        s = format!("…{s}");
    }
    if end < chars.len() {
        s.push('…');
    }
    s
}

fn plain_row(sign: &str, markup: &str) -> gtk::ListBoxRow {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    b.set_margin_start(12);
    b.set_margin_end(12);
    b.set_margin_top(6);
    b.set_margin_bottom(6);
    b.append(&gtk::Label::builder().label(sign).css_classes(["semn"]).valign(gtk::Align::Start).build());
    b.append(&gtk::Label::builder().label(markup).use_markup(true).xalign(0.0).wrap(true).wrap_mode(gtk::pango::WrapMode::WordChar).hexpand(true).build());
    gtk::ListBoxRow::builder().child(&b).activatable(true).build()
}

impl App {
    /// Deschide un rezultat al căutării.
    pub fn open_goto(self: &Rc<Self>, g: Goto) {
        match g {
            Goto::Entry(r, i) => self.open_result(r, i),
            Goto::Page(r) => {
                self.close_search();
                self.go_page(r);
            }
            Goto::Diary(d, line) => {
                let words = self.cautare.words.borrow().clone();
                self.close_search();
                self.open_diary_at(d, line, &words);
            }
            Goto::Note(i) => {
                self.close_search();
                self.edit_note(Some(i));
            }
            Goto::Tracking(y, m, tab, day) => {
                self.close_search();
                self.open_link(&lanruj_core::diary::Target::Tracking { year: y, month: m, tab: Some(tab.to_string()) }.to_url());
                if let Some(d) = day.and_then(|d| NaiveDate::from_ymd_opt(y, m, d)) {
                    self.tracking_focus.set(Some(d));
                    self.refresh();
                }
            }
            Goto::Index => {
                self.close_search();
                self.go_index();
            }
        }
    }
}

fn result_row(e: &Entry, words: &[String]) -> gtk::ListBoxRow {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    b.set_margin_start(12);
    b.set_margin_end(12);
    b.set_margin_top(6);
    b.set_margin_bottom(6);
    b.append(&gtk::Label::builder().label(sign(e.kind)).css_classes(["semn"]).valign(gtk::Align::Start).build());
    let mut markup = String::new();
    match e.priority {
        Priority::Important => markup.push_str("★ "),
        Priority::Inspiration => markup.push_str("! "),
        Priority::None => {}
    }
    if let Some(t) = &e.time {
        markup.push_str(&format!("{}  ", highlight(t, words)));
    }
    markup.push_str(&highlight(&lanruj_core::colors::plain(&e.text), words));
    if e.kind == Kind::Task(TaskState::Dropped) {
        markup = format!("<s>{markup}</s>");
    }
    b.append(&gtk::Label::builder().label(markup).use_markup(true).xalign(0.0).wrap(true).hexpand(true).build());
    gtk::ListBoxRow::builder().child(&b).activatable(true).build()
}
