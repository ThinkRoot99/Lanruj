// SPDX-License-Identifier: GPL-3.0-or-later
//! Bara laterală: lățime fixă, cu numele paginilor și calendarul; strânsă, rămân
//! doar iconițele. Se strânge cu butonul ei, cu F9 și singură, când fereastra e prea mică.

use adw::prelude::*;
use std::cell::Cell;

/// Lățimea barei întinse (aleasă de utilizator: calendarul încape întreg).
pub const WIDTH: i32 = 288;
/// Lățimea barei strânse (doar iconițe).
pub const COMPACT_WIDTH: i32 = 58;

pub struct Bara {
    /// Bara însăși, cu lățimea ei.
    pub side: gtk::Widget,
    /// Ce dispare când bara e strânsă: numele paginilor, numerele, calendarul, căutarea.
    pub hide_when_compact: Vec<gtk::Widget>,
    /// Rândurile paginilor, centrate când rămân doar iconițele.
    pub row_boxes: Vec<gtk::Box>,
    pub header: adw::HeaderBar,
    /// Strânsă la alegerea utilizatorului (butonul, F9).
    pub user_compact: Cell<bool>,
    /// Strânsă pentru că fereastra e prea mică.
    pub forced: Cell<bool>,
}

impl Bara {
    pub fn new(side: gtk::Widget, hide_when_compact: Vec<gtk::Widget>, row_boxes: Vec<gtk::Box>, header: adw::HeaderBar, compact: bool) -> Bara {
        Bara { side, hide_when_compact, row_boxes, header, user_compact: Cell::new(compact), forced: Cell::new(false) }
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
        self.header.set_show_title(!c);
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
