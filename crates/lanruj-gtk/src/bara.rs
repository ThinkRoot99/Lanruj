// SPDX-License-Identifier: GPL-3.0-or-later
//! Bara laterală: lățime fixă, cu numele paginilor și calendarul; strânsă, rămân
//! doar iconițele. Se strânge cu butonul ei, cu F9 și singură, când fereastra e prea mică.

use adw::prelude::*;
use std::cell::Cell;

/// Lățimea barei întinse (aleasă de utilizator: calendarul încape întreg).
pub const WIDTH: i32 = 288;
/// Lățimea barei strânse (doar iconițe).
pub const COMPACT_WIDTH: i32 = 46;

pub struct Bara {
    /// Bara însăși, cu lățimea ei.
    pub side: gtk::Widget,
    /// Ce dispare când bara e strânsă: numele paginilor, numerele, calendarul, căutarea.
    pub hide_when_compact: Vec<gtk::Widget>,
    /// Rândurile paginilor, centrate când rămân doar iconițele.
    pub row_boxes: Vec<gtk::Box>,
    pub header: adw::HeaderBar,
    /// Butonul de strângere: în stânga antetului când bara e întinsă, în mijloc când e strânsă
    /// (în stânga, antetul ar păstra încă atâta loc gol în dreapta, ca să rămână centrat).
    pub toggle: gtk::Button,
    pub title: gtk::Widget,
    toggle_centered: Cell<bool>,
    /// Strânsă la alegerea utilizatorului (butonul, F9).
    pub user_compact: Cell<bool>,
    /// Strânsă pentru că fereastra e prea mică.
    pub forced: Cell<bool>,
}

impl Bara {
    pub fn new(
        side: gtk::Widget,
        hide_when_compact: Vec<gtk::Widget>,
        row_boxes: Vec<gtk::Box>,
        header: adw::HeaderBar,
        toggle: gtk::Button,
        title: gtk::Widget,
        compact: bool,
    ) -> Bara {
        Bara {
            side,
            hide_when_compact,
            row_boxes,
            header,
            toggle,
            title,
            toggle_centered: Cell::new(false),
            user_compact: Cell::new(compact),
            forced: Cell::new(false),
        }
    }

    pub fn compact(&self) -> bool {
        self.user_compact.get() || self.forced.get()
    }

    /// Așază bara după starea ei: întinsă (cu nume și calendar) sau strânsă (doar iconițe).
    pub fn apply(&self) {
        let c = self.compact();
        for w in &self.hide_when_compact {
            w.set_visible(!c);
        }
        for b in &self.row_boxes {
            b.set_halign(if c { gtk::Align::Center } else { gtk::Align::Fill });
        }
        if c && !self.toggle_centered.get() {
            self.header.remove(&self.toggle);
            self.header.set_title_widget(Some(&self.toggle));
            self.toggle_centered.set(true);
        } else if !c && self.toggle_centered.get() {
            self.header.set_title_widget(Some(&self.title));
            self.header.pack_start(&self.toggle);
            self.toggle_centered.set(false);
        }
        // Strânsă, marginile din jurul iconițelor se micșorează (vezi .stransa în style.css).
        if c { self.side.add_css_class("stransa") } else { self.side.remove_css_class("stransa") }
        self.side.set_size_request(if c { COMPACT_WIDTH } else { WIDTH }, -1);
    }

    /// Butonul sau F9: strânge ori întinde bara (când fereastra are loc pentru ea).
    pub fn flip(&self) {
        if self.forced.get() {
            return;
        }
        self.user_compact.set(!self.user_compact.get());
        self.apply();
    }
}
