// SPDX-License-Identifier: GPL-3.0-or-later
//! O pagină din caiet: titlul zilei, intrările, etichetele și nota zilei.

use crate::{App, tr};
use adw::prelude::*;
use gtk::glib;
use lanruj_core::{Day, Entry, Item, Kind, Priority, TaskState};
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

    pub fn fill(&self, app: &Rc<App>, day: &Day, label: &str) {
        let lang = app.lang;
        self.label.set_label(&label.to_uppercase());
        self.label.set_visible(!label.is_empty());
        self.title.set_label(&lang.day_title(day.date));

        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        let mut any = false;
        for (index, item) in day.items.iter().enumerate() {
            match item {
                Item::Entry(e) => {
                    any = true;
                    self.list.append(&entry_row(app, day, index, e));
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
        if !any {
            let l = gtk::Label::builder()
                .label(tr(lang, "Nicio intrare încă.", "Nothing written yet."))
                .xalign(0.0)
                .css_classes(["intrare", "gol"])
                .build();
            self.list.append(&l);
        }

        self.fill_footer(app, day);
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

fn entry_row(app: &Rc<App>, day: &Day, index: usize, e: &Entry) -> gtk::Box {
    let lang = app.lang;
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("intrare");
    row.set_margin_start(26 * e.depth as i32);
    match e.kind {
        Kind::Task(TaskState::Done) => row.add_css_class("facut"),
        Kind::Task(TaskState::Migrated) | Kind::Task(TaskState::Scheduled) => row.add_css_class("mutat"),
        Kind::Task(TaskState::Dropped) => row.add_css_class("renuntat"),
        _ => {}
    }

    let sign = sign_of(e.kind);
    if matches!(e.kind, Kind::Task(TaskState::Open) | Kind::Task(TaskState::Done)) {
        let b = gtk::Button::with_label(sign);
        b.add_css_class("semn");
        b.set_valign(gtk::Align::Start);
        b.set_tooltip_text(Some(if e.kind == Kind::Task(TaskState::Done) {
            tr(lang, "Marchează ca nefăcută", "Mark as not done")
        } else {
            tr(lang, "Marchează ca făcută", "Mark as done")
        }));
        let (weak, date) = (Rc::downgrade(app), day.date);
        b.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.toggle(date, index);
            }
        });
        row.append(&b);
    } else {
        let l = gtk::Label::builder().label(sign).css_classes(["semn"]).valign(gtk::Align::Start).build();
        row.append(&l);
    }

    let mut markup = String::new();
    if let Some(t) = &e.time {
        markup.push_str(&format!("<b>{}</b>  ", glib::markup_escape_text(t)));
    }
    let text = glib::markup_escape_text(&e.text);
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
    row.append(&text_box);
    row
}
