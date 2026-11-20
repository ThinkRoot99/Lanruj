// SPDX-License-Identifier: GPL-3.0-or-later
//! Căutarea (Ctrl+F): un câmp, filtre și rezultatele grupate pe pagini.
//! Un clic pe un rezultat deschide pagina lui, cu intrarea aleasă; Esc închide căutarea.

use crate::{App, tr};
use adw::prelude::*;
use gtk::glib;
use lanruj_core::search::fold;
use lanruj_core::{Entry, Filter, Kind, Lang, PageRef, Priority, TaskState};
use std::cell::RefCell;
use std::rc::Rc;

const LIMIT: usize = 300;

pub struct Cautare {
    pub root: gtk::Box,
    pub entry: gtk::SearchEntry,
    filters: Vec<(gtk::ToggleButton, fn(&mut Filter))>,
    list: gtk::ListBox,
    status: adw::StatusPage,
    scroller: gtk::ScrolledWindow,
    count: gtk::Label,
    /// Ce deschide fiecare rând al listei (titlurile de pagină nu deschid nimic).
    targets: RefCell<Vec<Option<(PageRef, usize)>>>,
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
            .placeholder_text(tr(lang, "Caută în tot jurnalul", "Search the whole journal"))
            .hexpand(true)
            .build();
        top.append(&entry);
        let fbox = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let mut filters: Vec<(gtk::ToggleButton, fn(&mut Filter))> = Vec::new();
        let defs: [(&str, &str, fn(&mut Filter)); 5] = [
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
        let count = gtk::Label::builder().css_classes(["dim-label"]).hexpand(true).xalign(1.0).build();
        fbox.append(&count);
        top.append(&fbox);
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
                "Scrie unul sau mai multe cuvinte, sau alege un filtru. Literele mari și diacriticele nu contează.",
                "Type one or more words, or pick a filter. Case and accents don't matter.",
            ))
            .vexpand(true)
            .build();
        root.append(&status);
        root.append(&scroller);
        scroller.set_visible(false);

        Cautare { root, entry, filters, list, status, scroller, count, targets: RefCell::new(Vec::new()) }
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
        for (b, _) in &self.filters {
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
            let target = a.cautare.targets.borrow().get(row.index() as usize).copied().flatten();
            if let Some((r, i)) = target {
                glib::idle_add_local_once(move || a.open_result(r, i));
            }
        });
    }

    #[cfg(debug_assertions)]
    pub fn first_target(&self) -> Option<(PageRef, usize)> {
        self.targets.borrow().iter().flatten().next().copied()
    }

    fn page_title(app: &App, r: PageRef) -> String {
        let l = app.lang;
        match r {
            PageRef::Day(d) => l.day_title(d),
            PageRef::Month { year, month } => format!("{} · {}", l.month_title(year, month), tr(l, "lista lunii", "monthly list")),
            PageRef::Week { year, week } => format!("{} · {}", l.week_title(week), l.week_range(lanruj_core::week_monday(year, week))),
            PageRef::Future { year, month } => format!("{} · {}", tr(l, "Planuri", "Future log"), l.month_title(year, month)),
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
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        let mut targets = Vec::new();
        let words: Vec<String> = fold(&query).split_whitespace().map(str::to_string).collect();
        if words.is_empty() && filter == Filter::default() {
            self.status.set_visible(true);
            self.scroller.set_visible(false);
            self.status.set_title(tr(app.lang, "Caută în jurnal", "Search the journal"));
            self.count.set_label("");
            *self.targets.borrow_mut() = targets;
            return;
        }
        let results = app.journal.search(&query, filter, LIMIT);
        let total: usize = results.iter().map(|(_, h)| h.len()).sum();
        self.count.set_label(&match app.lang {
            Lang::Ro => match total {
                0 => String::new(),
                1 => "un rezultat".into(),
                n if n % 100 >= 20 || n % 100 == 0 => format!("{n} de rezultate"),
                n => format!("{n} rezultate"),
            },
            Lang::En => if total == 1 { "1 result".into() } else { format!("{total} results") },
        });
        if results.is_empty() {
            self.status.set_visible(true);
            self.scroller.set_visible(false);
            self.status.set_title(tr(app.lang, "Nimic găsit", "Nothing found"));
            *self.targets.borrow_mut() = targets;
            return;
        }
        self.status.set_visible(false);
        self.scroller.set_visible(true);
        for (r, hits) in results {
            let head = gtk::ListBoxRow::builder().activatable(false).selectable(false).css_classes(["titlu-pagina-rezultat"]).build();
            head.set_child(Some(&gtk::Label::builder().label(Self::page_title(app, r)).xalign(0.0).css_classes(["heading", "rezultat-pagina"]).build()));
            self.list.append(&head);
            targets.push(None);
            for (i, e) in hits {
                self.list.append(&result_row(&e, &words));
                targets.push(Some((r, i)));
            }
        }
        *self.targets.borrow_mut() = targets;
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
    markup.push_str(&highlight(&e.text, words));
    if e.kind == Kind::Task(TaskState::Dropped) {
        markup = format!("<s>{markup}</s>");
    }
    b.append(&gtk::Label::builder().label(markup).use_markup(true).xalign(0.0).wrap(true).hexpand(true).build());
    gtk::ListBoxRow::builder().child(&b).activatable(true).build()
}
