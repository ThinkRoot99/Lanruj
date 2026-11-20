// SPDX-License-Identifier: GPL-3.0-or-later
//! Nucleul Lanruj: formatul fișierelor zilei, citirea și scrierea lor.
//! Nu știe nimic despre ferestre; interfața doar îl folosește.

pub mod day;
pub mod journal;
pub mod lang;
pub mod migrate;
pub mod month;

pub use day::{parse_quick, Day, Entry, Item, Kind, Priority, TaskState};
pub use journal::Journal;
pub use lang::Lang;
pub use migrate::Decision;
pub use month::{CalLine, Month};

/// O pagină cu intrări: o zi sau lista de sarcini a unei luni.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageRef {
    Day(chrono::NaiveDate),
    Month { year: i32, month: u32 },
}
