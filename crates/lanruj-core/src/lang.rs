// SPDX-License-Identifier: GPL-3.0-or-later
//! Limba aplicației: română sau engleză. Datele reale vin din numele
//! fișierelor, deci schimbarea limbii nu strică nimic din jurnal.

use chrono::{Datelike, NaiveDate, Weekday};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Ro,
    En,
}

const ZILE_RO: [&str; 7] = ["Luni", "Marți", "Miercuri", "Joi", "Vineri", "Sâmbătă", "Duminică"];
const ZILE_EN: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const LUNI_RO: [&str; 12] = [
    "ianuarie", "februarie", "martie", "aprilie", "mai", "iunie", "iulie", "august", "septembrie",
    "octombrie", "noiembrie", "decembrie",
];
const LUNI_EN: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
];

impl Lang {
    /// Limba sistemului: română dacă LANG începe cu „ro”, altfel engleză.
    pub fn from_env() -> Lang {
        let v = std::env::var("LC_ALL")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("LANG").ok())
            .unwrap_or_default();
        if v.starts_with("ro") { Lang::Ro } else { Lang::En }
    }

    pub fn weekday(self, w: Weekday) -> &'static str {
        let i = w.num_days_from_monday() as usize;
        match self {
            Lang::Ro => ZILE_RO[i],
            Lang::En => ZILE_EN[i],
        }
    }

    pub fn month(self, m: u32) -> &'static str {
        let i = (m - 1) as usize;
        match self {
            Lang::Ro => LUNI_RO[i],
            Lang::En => LUNI_EN[i],
        }
    }

    /// „Vineri, 20 noiembrie 2026” / „Friday, 20 November 2026”
    pub fn day_title(self, d: NaiveDate) -> String {
        format!("{}, {} {} {}", self.weekday(d.weekday()), d.day(), self.month(d.month()), d.year())
    }

    /// „Vineri, 20 noiembrie” (fără an), pentru antetul ferestrei.
    pub fn day_title_short(self, d: NaiveDate) -> String {
        format!("{}, {} {}", self.weekday(d.weekday()), d.day(), self.month(d.month()))
    }

    /// „Noiembrie 2026”
    pub fn month_title(self, year: i32, month: u32) -> String {
        let m = self.month(month);
        let mut c = m.chars();
        let first = c.next().map(|f| f.to_uppercase().collect::<String>()).unwrap_or_default();
        format!("{}{} {}", first, c.as_str(), year)
    }

    pub fn tags_label(self) -> &'static str {
        match self {
            Lang::Ro => "Etichete",
            Lang::En => "Tags",
        }
    }

    pub fn rating_label(self) -> &'static str {
        match self {
            Lang::Ro => "Ziua",
            Lang::En => "Day",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titluri() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        assert_eq!(Lang::Ro.day_title(d), "Vineri, 20 noiembrie 2026");
        assert_eq!(Lang::En.day_title(d), "Friday, 20 November 2026");
        assert_eq!(Lang::Ro.month_title(2026, 11), "Noiembrie 2026");
    }
}
