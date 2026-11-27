// SPDX-License-Identifier: GPL-3.0-or-later
//! Biletele pe ecran: foi mici, galbene, lipite pe caiet. Fiecare se trage cu mouse-ul oriunde
//! (locul se ține minte); clic = editează; clic dreapta = Editează, Pune în ziua de azi, Șterge.
//! × îl șterge, cu Anulează. Nu intră în textul paginilor.

use crate::{App, tr};
use adw::prelude::*;
use gtk::glib;
use lanruj_core::bilete::Note;
use std::cell::Cell;
use std::rc::Rc;

/// Mărimea aproximativă a unui bilet, pentru locurile implicite (dreapta jos, unul sub altul).
const W: i32 = 210;
const STEP: i32 = 86;

impl App {
    /// Reface biletele de pe caiet, din `bilete.md`.
    pub fn refresh_notes(self: &Rc<Self>) {
        for w in self.notes_widgets.borrow_mut().drain(..) {
            self.notes_over.remove_overlay(&w);
        }
        let notes = self.journal.load_notes().unwrap_or_default();
        let (aw, ah) = (self.notes_over.width(), self.notes_over.height());
        if (aw < 50 || ah < 50) && !notes.is_empty() {
            // Încă fără mărime: se reface după ce fereastra apare.
            let weak = Rc::downgrade(self);
            glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
                if let Some(app) = weak.upgrade() {
                    app.refresh_notes();
                }
            });
            return;
        }
        let without_pos = notes.iter().filter(|n| n.pos.is_none()).count() as i32;
        let mut k = 0;
        for (i, n) in notes.iter().enumerate() {
            let (x, y) = n.pos.unwrap_or_else(|| {
                let p = (aw - W - 40, (ah - 120 - (without_pos - k) * STEP).max(10));
                k += 1;
                p
            });
            let w = self.note_widget(i, n, (x.clamp(0, (aw - 60).max(0)), y.clamp(0, (ah - 40).max(0))));
            self.notes_over.add_overlay(&w);
            self.notes_widgets.borrow_mut().push(w.upcast());
        }
    }

    fn note_widget(self: &Rc<Self>, i: usize, n: &Note, (x, y): (i32, i32)) -> gtk::Box {
        let lang = self.lang;
        let note = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        note.add_css_class("bilet");
        note.add_css_class(["inclinat-a", "inclinat-b", "inclinat-c"][i % 3]);
        note.set_halign(gtk::Align::Start);
        note.set_valign(gtk::Align::Start);
        note.set_margin_start(x);
        note.set_margin_top(y);
        note.set_cursor_from_name(Some("grab"));
        note.set_tooltip_text(Some(tr(lang, "Trage ca să-l muți; clic ca să-l editezi; clic dreapta pentru mai multe", "Drag to move; click to edit; right-click for more")));
        let text = gtk::Label::builder()
            .label(&n.text)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .xalign(0.0)
            .max_width_chars(20)
            .hexpand(true)
            .css_classes(["text-bilet"])
            .build();
        note.append(&text);
        let del = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .css_classes(["flat", "circular", "sterge-bilet"])
            .valign(gtk::Align::Start)
            .tooltip_text(tr(lang, "Șterge biletul", "Delete the note"))
            .build();
        let weak = Rc::downgrade(self);
        del.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.delete_note(i);
            }
        });
        note.append(&del);

        // Tragerea: biletul urmează mouse-ul; la sfârșit locul se salvează.
        // Fără mișcare, e un clic: se deschide pentru editare.
        let drag = gtk::GestureDrag::new();
        // Unde a fost apucat biletul (în coordonatele lui), ca să rămână sub cursor.
        let start = Rc::new(Cell::new((0.0f64, 0.0f64)));
        let moved = Rc::new(Cell::new(false));
        let begun = Rc::new(Cell::new(false));
        {
            let (n2, start, moved, begun) = (note.clone(), start.clone(), moved.clone(), begun.clone());
            drag.connect_drag_begin(move |g, px, py| {
                // Apăsarea pe × nu e tragere și nici clic de editare.
                let on_button = n2.pick(px, py, gtk::PickFlags::DEFAULT).is_some_and(|w| w.is::<gtk::Button>() || w.ancestor(gtk::Button::static_type()).is_some());
                if on_button {
                    g.set_state(gtk::EventSequenceState::Denied);
                    begun.set(false);
                    return;
                }
                begun.set(true);
                start.set((px, py));
                moved.set(false);
                n2.add_css_class("trage");
            });
        }
        {
            let (n2, start, moved, over) = (note.clone(), start.clone(), moved.clone(), self.notes_over.clone());
            drag.connect_drag_update(move |g, dx, dy| {
                if !moved.get() && dx.abs() + dy.abs() <= 4.0 {
                    return;
                }
                moved.set(true);
                // Cursorul, socotit față de caiet (care stă pe loc), nu față de biletul care se mișcă:
                // altfel cele două se „ceartă” și biletul sare între două locuri.
                let seq = g.current_sequence();
                let Some((cx, cy)) = g.point(seq.as_ref()) else { return };
                let Some(p) = n2.compute_point(&over, &gtk::graphene::Point::new(cx as f32, cy as f32)) else { return };
                let (gx, gy) = start.get();
                let nx = (p.x() as f64 - gx).round() as i32;
                let ny = (p.y() as f64 - gy).round() as i32;
                let nx = nx.clamp(0, (over.width() - 60).max(0));
                let ny = ny.clamp(0, (over.height() - 40).max(0));
                if nx != n2.margin_start() {
                    n2.set_margin_start(nx);
                }
                if ny != n2.margin_top() {
                    n2.set_margin_top(ny);
                }
            });
        }
        {
            let (n2, moved, weak, begun) = (note.clone(), moved.clone(), Rc::downgrade(self), begun.clone());
            drag.connect_drag_end(move |_, _, _| {
                n2.remove_css_class("trage");
                if !begun.replace(false) {
                    return;
                }
                let Some(app) = weak.upgrade() else { return };
                if moved.get() {
                    let pos = (n2.margin_start(), n2.margin_top());
                    if let Err(e) = app.journal.update_notes(|n| {
                        if let Some(x) = n.get_mut(i) {
                            x.pos = Some(pos);
                        }
                    }) {
                        app.error(&e);
                    }
                } else {
                    let a = app.clone();
                    glib::idle_add_local_once(move || a.edit_note(Some(i)));
                }
            });
        }
        note.add_controller(drag);

        // Clic dreapta: meniul biletului.
        let pop = gtk::Popover::new();
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let item = |label: &str| gtk::Button::builder().label(label).css_classes(["flat"]).build();
        let edit = item(tr(lang, "Editează", "Edit"));
        let to_day = item(tr(lang, "Pune în ziua de azi", "Put into today's page"));
        let remove = item(tr(lang, "Șterge", "Delete"));
        for b in [&edit, &to_day, &remove] {
            b.child().and_then(|c| c.downcast::<gtk::Label>().ok()).inspect(|l| l.set_xalign(0.0));
            menu.append(b);
        }
        pop.set_child(Some(&menu));
        pop.set_parent(&note);
        let p2 = pop.clone();
        note.connect_destroy(move |_| p2.unparent());
        let (weak, p) = (Rc::downgrade(self), pop.clone());
        edit.connect_clicked(move |_| {
            p.popdown();
            if let Some(app) = weak.upgrade() {
                app.edit_note(Some(i));
            }
        });
        let (weak, p, t) = (Rc::downgrade(self), pop.clone(), n.text.clone());
        to_day.connect_clicked(move |_| {
            p.popdown();
            let Some(app) = weak.upgrade() else { return };
            // Biletul devine o idee (!) în ziua de azi și dispare de pe caiet.
            let first = t.lines().collect::<Vec<_>>().join(" ");
            let Some(entry) = lanruj_core::day::parse_quick(&format!("- ! {first}")) else { return };
            let res = app.journal.add(app.today.get(), entry, app.lang).and_then(|_| {
                app.journal.update_notes(|n| {
                    if i < n.len() {
                        n.remove(i);
                    }
                })
            });
            match res {
                Ok(_) => {
                    app.refresh();
                    app.refresh_notes();
                    app.toast(tr(app.lang, "Biletul e acum o idee în ziua de azi.", "The note is now an idea in today's page."));
                }
                Err(e) => app.error(&e),
            }
        });
        let (weak, p) = (Rc::downgrade(self), pop.clone());
        remove.connect_clicked(move |_| {
            p.popdown();
            if let Some(app) = weak.upgrade() {
                app.delete_note(i);
            }
        });
        let right = gtk::GestureClick::builder().button(3).build();
        let p = pop.clone();
        right.connect_pressed(move |_, _, _, _| p.popup());
        note.add_controller(right);
        note
    }

    /// Șterge biletul `i`, cu Anulează în notificarea de jos.
    fn delete_note(self: &Rc<Self>, i: usize) {
        let mut removed = None;
        if let Err(e) = self.journal.update_notes(|n| {
            if i < n.len() {
                removed = Some(n.remove(i));
            }
        }) {
            self.error(&e);
            return;
        }
        self.refresh_notes();
        let Some(note) = removed else { return };
        let t = adw::Toast::builder().title(tr(self.lang, "Biletul a fost șters", "The note was deleted")).button_label(tr(self.lang, "Anulează", "Undo")).build();
        let weak = Rc::downgrade(self);
        t.connect_button_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                let n2 = note.clone();
                match app.journal.update_notes(move |n| n.insert(i.min(n.len()), n2)) {
                    Ok(_) => app.refresh_notes(),
                    Err(e) => app.error(&e),
                }
            }
        });
        self.toasts.add_toast(t);
    }

    /// Becul din antet (Ctrl+N): bilet nou. Cu `Some(i)`: editează biletul `i`.
    /// Ce e scris nu se pierde: închiderea ferestrei (× sau Esc) îl păstrează; doar „Renunță” îl aruncă.
    pub fn quick_idea(self: &Rc<Self>) {
        self.edit_note(None);
    }

    pub(crate) fn edit_note(self: &Rc<Self>, which: Option<usize>) {
        let lang = self.lang;
        let old = which.and_then(|i| self.journal.load_notes().ok().and_then(|n| n.get(i).cloned()));
        let d = adw::Dialog::builder().title(if which.is_some() { tr(lang, "Biletul", "The note") } else { tr(lang, "Bilet nou", "New note") }).content_width(400).build();
        let b = gtk::Box::new(gtk::Orientation::Vertical, 10);
        b.set_margin_top(4);
        b.set_margin_bottom(14);
        b.set_margin_start(14);
        b.set_margin_end(14);
        let view = gtk::TextView::builder().wrap_mode(gtk::WrapMode::WordChar).height_request(90).css_classes(["bilet-nou"]).build();
        if let Some(o) = &old {
            view.buffer().set_text(&o.text);
        }
        b.append(&view);
        b.append(
            &gtk::Label::builder()
                .label(tr(
                    lang,
                    "Enter salvează; Shift+Enter trece pe rândul următor. Biletul stă pe caiet până îl ștergi; îl poți trage oriunde.",
                    "Enter saves; Shift+Enter starts a new line. The note stays on the notebook until you delete it; you can drag it anywhere.",
                ))
                .wrap(true)
                .xalign(0.0)
                .css_classes(["dim-label", "caption"])
                .build(),
        );
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_halign(gtk::Align::End);
        let discard = gtk::Button::with_label(tr(lang, "Renunță", "Discard"));
        let save = gtk::Button::builder().label(tr(lang, "Salvează", "Save")).css_classes(["suggested-action"]).build();
        row.append(&discard);
        row.append(&save);
        b.append(&row);
        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&adw::HeaderBar::new());
        tv.set_content(Some(&b));
        d.set_child(Some(&tv));

        // Salvarea: o singură dată, la Enter, la „Salvează” sau la închiderea ferestrei.
        let done = Rc::new(Cell::new(false));
        let store = {
            let (weak, v, done) = (Rc::downgrade(self), view.clone(), done.clone());
            move || {
                if done.replace(true) {
                    return;
                }
                let Some(app) = weak.upgrade() else { return };
                let buf = v.buffer();
                let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).trim().to_string();
                if text.is_empty() {
                    return;
                }
                let res = app.journal.update_notes(|n| match which {
                    Some(i) if i < n.len() => n[i].text = text,
                    _ => n.push(Note::new(&text)),
                });
                match res {
                    Ok(_) => app.refresh_notes(),
                    Err(e) => app.error(&e),
                }
            }
        };
        let s1 = store.clone();
        d.connect_closed(move |_| s1());
        let dl = d.clone();
        save.connect_clicked(move |_| {
            dl.close();
        });
        let (dl, done2) = (d.clone(), done.clone());
        discard.connect_clicked(move |_| {
            done2.set(true);
            dl.close();
        });
        let keys = gtk::EventControllerKey::new();
        let dl = d.clone();
        keys.connect_key_pressed(move |_, key, _, mods| {
            if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) && !mods.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                dl.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        view.add_controller(keys);
        d.present(Some(&self.window));
        view.grab_focus();
    }
}
