// SPDX-License-Identifier: GPL-3.0-or-later
//! O pagină din caiet: titlul zilei, intrările, etichetele și nota zilei.

use crate::migrare::MigView;
use crate::{App, tr};
use adw::prelude::*;
use gtk::glib;
use lanruj_core::{Day, Decision, Entry, Item, Kind, Priority, TaskState};
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

    pub fn fill(&self, app: &Rc<App>, day: &Day, label: &str, mig: Option<&MigView>) {
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
                    let row = entry_row(app, day, index, e, mig);
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

/// Rândul în editare: un câmp cu intrarea scrisă ca jos („. text”, „o 10:00 text”, „- text”).
fn edit_row(app: &Rc<App>, e: &Entry) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
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
    row
}

/// Meniul de la clic dreapta: Editează, Șterge. Dublu clic = editează.
fn add_entry_menu(app: &Rc<App>, row: &gtk::Box, date: chrono::NaiveDate, index: usize) {
    let lang = app.lang;
    let pop = gtk::Popover::builder().has_arrow(true).build();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let edit = gtk::Button::builder().label(tr(lang, "Editează", "Edit")).css_classes(["flat"]).build();
    let del = gtk::Button::builder().label(tr(lang, "Șterge", "Delete")).css_classes(["flat", "error"]).build();
    for b in [&edit, &del] {
        b.child().and_then(|c| c.downcast::<gtk::Label>().ok()).inspect(|l| l.set_xalign(0.0));
        menu.append(b);
    }
    pop.set_child(Some(&menu));
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

fn entry_row(app: &Rc<App>, day: &Day, index: usize, e: &Entry, mig: Option<&MigView>) -> gtk::Box {
    let lang = app.lang;
    if mig.is_none() && app.editing.get() == Some((day.date, index)) {
        return edit_row(app, e);
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("intrare");
    if mig.is_none() {
        add_entry_menu(app, &row, day.date, index);
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
        && let Some(Decision::Moved(d)) = m.decided.get(&index)
        && *d != day.date + chrono::Duration::days(1)
    {
        let l = gtk::Label::builder()
            .label(format!("→ {} {}", chrono::Datelike::day(d), lang.month(chrono::Datelike::month(d))))
            .css_classes(["migrari"])
            .valign(gtk::Align::Start)
            .build();
        l.set_margin_top(3);
        text_box.append(&l);
    }
    row.append(&text_box);
    row
}
