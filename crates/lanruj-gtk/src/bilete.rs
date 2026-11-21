// SPDX-License-Identifier: GPL-3.0-or-later
//! Biletele pe ecran: foi mici, galbene, într-un colț al caietului, deasupra paginilor.
//! Se văd pe fiecare pagină și dispar doar când le ștergi (×). Nu intră în textul paginilor.

use crate::{App, tr};
use adw::prelude::*;
use std::rc::Rc;

impl App {
    /// Reface biletele din colț, din `bilete.md`.
    pub fn refresh_notes(self: &Rc<Self>) {
        while let Some(c) = self.notes_box.first_child() {
            self.notes_box.remove(&c);
        }
        let notes = self.journal.load_notes().unwrap_or_default();
        self.notes_scroll.set_visible(!notes.is_empty());
        let lang = self.lang;
        for (i, text) in notes.iter().enumerate() {
            let note = gtk::Box::new(gtk::Orientation::Vertical, 2);
            note.add_css_class("bilet");
            // Ușor strâmbe, ca niște foi lipite de mână.
            note.add_css_class(["inclinat-a", "inclinat-b", "inclinat-c"][i % 3]);
            let top = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            top.append(&gtk::Box::builder().hexpand(true).build());
            let del = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .css_classes(["flat", "circular", "sterge-bilet"])
                .tooltip_text(tr(lang, "Șterge biletul", "Delete the note"))
                .build();
            let weak = Rc::downgrade(self);
            del.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    match app.journal.update_notes(|n| {
                        if i < n.len() {
                            n.remove(i);
                        }
                    }) {
                        Ok(_) => app.refresh_notes(),
                        Err(e) => app.error(&e),
                    }
                }
            });
            top.append(&del);
            note.append(&top);
            note.append(&gtk::Label::builder().label(text).wrap(true).wrap_mode(gtk::pango::WrapMode::WordChar).xalign(0.0).max_width_chars(22).css_classes(["text-bilet"]).build());
            self.notes_box.append(&note);
        }
    }

    /// Becul din antet (Ctrl+N): o fereastră mică în care scrii biletul.
    pub fn quick_idea(self: &Rc<Self>) {
        let lang = self.lang;
        let d = adw::Dialog::builder().title(tr(lang, "Bilet nou", "New note")).content_width(420).build();
        let b = gtk::Box::new(gtk::Orientation::Vertical, 10);
        b.set_margin_top(6);
        b.set_margin_bottom(16);
        b.set_margin_start(16);
        b.set_margin_end(16);
        let view = gtk::TextView::builder().wrap_mode(gtk::WrapMode::WordChar).height_request(90).css_classes(["bilet-nou"]).build();
        b.append(&view);
        b.append(
            &gtk::Label::builder()
                .label(tr(
                    lang,
                    "Biletul stă lipit într-un colț al caietului, pe fiecare pagină, până îl ștergi. Enter îl pune; Shift+Enter trece pe rândul următor.",
                    "The note stays stuck in a corner of the notebook, on every page, until you delete it. Enter adds it; Shift+Enter starts a new line.",
                ))
                .wrap(true)
                .xalign(0.0)
                .css_classes(["dim-label", "caption"])
                .build(),
        );
        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&adw::HeaderBar::new());
        tv.set_content(Some(&b));
        d.set_child(Some(&tv));
        let keys = gtk::EventControllerKey::new();
        let (weak, dl, v) = (Rc::downgrade(self), d.clone(), view.clone());
        keys.connect_key_pressed(move |_, key, _, mods| {
            if !matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) || mods.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                return gtk::glib::Propagation::Proceed;
            }
            let buf = v.buffer();
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).trim().to_string();
            dl.close();
            if let (Some(app), false) = (weak.upgrade(), text.is_empty()) {
                match app.journal.update_notes(|n| n.push(text)) {
                    Ok(_) => app.refresh_notes(),
                    Err(e) => app.error(&e),
                }
            }
            gtk::glib::Propagation::Stop
        });
        view.add_controller(keys);
        d.present(Some(&self.window));
        view.grab_focus();
    }
}
