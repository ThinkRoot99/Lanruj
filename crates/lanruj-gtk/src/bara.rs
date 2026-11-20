// SPDX-License-Identifier: GPL-3.0-or-later
//! Bara laterală: se trage de margine ca să-și schimbe lățimea; strânsă, rămân
//! doar iconițele (fără nume și fără calendar). Se strânge cu butonul ei, cu F9,
//! când e trasă foarte îngust și singură, când fereastra e prea mică.

use adw::prelude::*;
use std::cell::Cell;

/// Lățimea barei strânse (doar iconițe).
pub const COMPACT_WIDTH: i32 = 58;
/// Sub această lățime, trasă de mână, bara se strânge.
const SNAP_WIDTH: i32 = 170;
/// Lățimea cea mai mică a barei întinse.
pub const MIN_WIDTH: i32 = 200;

pub struct Bara {
    pub paned: gtk::Paned,
    /// Ce dispare când bara e strânsă: numele paginilor, numerele, calendarul, căutarea.
    pub hide_when_compact: Vec<gtk::Widget>,
    /// Rândurile paginilor, centrate când rămân doar iconițele.
    pub row_boxes: Vec<gtk::Box>,
    pub header: adw::HeaderBar,
    /// Strânsă la alegerea utilizatorului (butonul, F9, trasă îngust).
    pub user_compact: Cell<bool>,
    /// Strânsă pentru că fereastra e prea mică.
    pub forced: Cell<bool>,
    /// Lățimea barei întinse, ținută minte.
    pub width: Cell<i32>,
    busy: Cell<bool>,
}

impl Bara {
    pub fn new(
        paned: gtk::Paned,
        hide_when_compact: Vec<gtk::Widget>,
        row_boxes: Vec<gtk::Box>,
        header: adw::HeaderBar,
        width: i32,
        compact: bool,
    ) -> Bara {
        Bara {
            paned,
            hide_when_compact,
            row_boxes,
            header,
            user_compact: Cell::new(compact),
            forced: Cell::new(false),
            width: Cell::new(width.max(MIN_WIDTH)),
            busy: Cell::new(false),
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
        self.header.set_show_title(!c);
        self.busy.set(true);
        self.paned.set_position(if c { COMPACT_WIDTH } else { self.width.get() });
        self.busy.set(false);
    }

    /// Butonul sau F9: strânge ori întinde bara.
    pub fn flip(&self) {
        if self.forced.get() {
            // Fereastra e prea mică pentru bara întinsă; o strângem doar la alegere.
            return;
        }
        self.user_compact.set(!self.user_compact.get());
        self.apply();
    }

    /// Bara trasă de margine: îngustă de tot = strânsă; lărgită din nou = întinsă.
    pub fn dragged(&self) {
        if self.busy.get() || self.forced.get() {
            return;
        }
        let pos = self.paned.position();
        if self.user_compact.get() {
            if pos > COMPACT_WIDTH + 40 {
                self.user_compact.set(false);
                self.width.set(pos.max(MIN_WIDTH));
                self.apply();
            } else if pos != COMPACT_WIDTH {
                self.apply();
            }
        } else if pos < SNAP_WIDTH {
            self.user_compact.set(true);
            self.apply();
        } else {
            self.width.set(pos.max(MIN_WIDTH));
        }
    }
}
