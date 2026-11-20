// SPDX-License-Identifier: GPL-3.0-or-later
//! Nucleul Lanruj: formatul fișierelor zilei, citirea și scrierea lor.
//! Nu știe nimic despre ferestre; interfața doar îl folosește.

pub mod day;
pub mod future;
pub mod journal;
pub mod lang;
pub mod migrate;
pub mod month;

pub use day::{parse_quick, Day, Entry, Item, Kind, Priority, TaskState};
pub use journal::Journal;
pub use lang::Lang;
pub use migrate::Decision;
pub use future::Future;
pub use month::{CalLine, Month};

/// O pagină cu intrări: o zi sau lista de sarcini a unei luni.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageRef {
    Day(chrono::NaiveDate),
    Month { year: i32, month: u32 },
    /// Secțiunea unei luni din jurnalul viitor.
    Future { year: i32, month: u32 },
    /// Planul unei săptămâni (an și săptămână ISO).
    Week { year: i32, week: u32 },
}

/// Lunea săptămânii ISO.
pub fn week_monday(year: i32, week: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Mon).expect("săptămână validă")
}

/// Săptămâna ISO a unei zile.
pub fn week_of(d: chrono::NaiveDate) -> PageRef {
    use chrono::Datelike;
    let w = d.iso_week();
    PageRef::Week { year: w.year(), week: w.week() }
}
