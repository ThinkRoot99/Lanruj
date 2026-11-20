// SPDX-License-Identifier: GPL-3.0-or-later
//! Setările aplicației, pe calculatorul acesta: `~/.config/lanruj/setari.ini`.

use gtk::glib;
use lanruj_core::Lang;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paper {
    Dots,
    Grid,
    Lines,
    Plain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaperColor {
    White,
    Warm,
    Yellow,
    Blue,
    Brown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `None` = limba sistemului.
    pub lang: Option<Lang>,
    pub paper: Paper,
    pub color: PaperColor,
    pub scheme: Scheme,
    /// Ora de la care migrarea de seară pornește singură.
    pub evening_hour: u32,
    /// Migrările pornesc singure (seara, la început de lună și de săptămână).
    pub auto_migrate: bool,
    /// La închidere, Lanruj rămâne în systray.
    pub tray: bool,
    /// Bara laterală strânsă (doar iconițe).
    pub sidebar_compact: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { lang: None, paper: Paper::Dots, color: PaperColor::Warm, scheme: Scheme::System, evening_hour: 21, auto_migrate: true, tray: false, sidebar_compact: false }
    }
}

pub const PAPERS: [(Paper, &str, &str, &str); 4] = [
    (Paper::Dots, "puncte", "Puncte", "Dots"),
    (Paper::Grid, "patratele", "Pătrățele", "Grid"),
    (Paper::Lines, "dungi", "Dungi", "Lines"),
    (Paper::Plain, "alba", "Fără model", "Plain"),
];

pub const COLORS: [(PaperColor, &str, &str, &str); 5] = [
    (PaperColor::White, "alb", "Alb", "White"),
    (PaperColor::Warm, "cald", "Alb cald", "Warm white"),
    (PaperColor::Yellow, "galbui", "Gălbui", "Pale yellow"),
    (PaperColor::Blue, "albastru", "Albastru pal", "Pale blue"),
    (PaperColor::Brown, "maro", "Maro pal", "Pale brown"),
];

pub const SCHEMES: [(Scheme, &str, &str, &str); 3] = [
    (Scheme::System, "sistem", "Ca sistemul", "Follow system"),
    (Scheme::Light, "zi", "Mereu zi", "Always light"),
    (Scheme::Dark, "noapte", "Mereu noapte", "Always dark"),
];

pub const LANGS: [(Option<Lang>, &str, &str, &str); 3] = [
    (None, "auto", "Ca sistemul", "Follow system"),
    (Some(Lang::Ro), "ro", "Română", "Română"),
    (Some(Lang::En), "en", "English", "English"),
];

fn path() -> PathBuf {
    // La probe (LANRUJ_DIR), setările stau lângă jurnalul de probă.
    if let Some(d) = std::env::var_os("LANRUJ_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join(".setari.ini");
    }
    glib::user_config_dir().join("lanruj").join("setari.ini")
}

fn find<T: Copy + PartialEq, const N: usize>(table: &[(T, &str, &str, &str); N], key: &str, default: T) -> T {
    table.iter().find(|(_, k, _, _)| *k == key).map(|(v, ..)| *v).unwrap_or(default)
}

fn key_of<T: Copy + PartialEq, const N: usize>(table: &[(T, &'static str, &'static str, &'static str); N], v: T) -> &'static str {
    table.iter().find(|(x, ..)| *x == v).map(|(_, k, ..)| *k).unwrap_or("")
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let kf = glib::KeyFile::new();
        if kf.load_from_file(path(), glib::KeyFileFlags::NONE).is_err() {
            return s;
        }
        let get = |k: &str| kf.string("lanruj", k).ok().map(|v| v.to_string());
        if let Some(v) = get("limba") {
            s.lang = find(&LANGS, &v, None);
        }
        if let Some(v) = get("hartie") {
            s.paper = find(&PAPERS, &v, s.paper);
        }
        if let Some(v) = get("culoare") {
            s.color = find(&COLORS, &v, s.color);
        }
        if let Some(v) = get("noapte") {
            s.scheme = find(&SCHEMES, &v, s.scheme);
        }
        if let Ok(h) = kf.integer("lanruj", "ora-migrarii")
            && (0..24).contains(&h)
        {
            s.evening_hour = h as u32;
        }
        if let Ok(b) = kf.boolean("lanruj", "migrare-automata") {
            s.auto_migrate = b;
        }
        if let Ok(b) = kf.boolean("lanruj", "systray") {
            s.tray = b;
        }
        if let Ok(b) = kf.boolean("lanruj", "bara-stransa") {
            s.sidebar_compact = b;
        }
        s
    }

    pub fn save(&self) -> std::io::Result<()> {
        let kf = glib::KeyFile::new();
        kf.set_string("lanruj", "limba", key_of(&LANGS, self.lang));
        kf.set_string("lanruj", "hartie", key_of(&PAPERS, self.paper));
        kf.set_string("lanruj", "culoare", key_of(&COLORS, self.color));
        kf.set_string("lanruj", "noapte", key_of(&SCHEMES, self.scheme));
        kf.set_integer("lanruj", "ora-migrarii", self.evening_hour as i32);
        kf.set_boolean("lanruj", "migrare-automata", self.auto_migrate);
        kf.set_boolean("lanruj", "systray", self.tray);
        kf.set_boolean("lanruj", "bara-stransa", self.sidebar_compact);
        let p = path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        kf.save_to_file(&p).map_err(std::io::Error::other)
    }

    pub fn lang(&self) -> Lang {
        self.lang.unwrap_or_else(Lang::from_env)
    }

    /// Clasele CSS ale ferestrei pentru hârtie: „hartie-puncte”, „culoare-cald”.
    pub fn css_classes(&self) -> [String; 2] {
        [format!("hartie-{}", key_of(&PAPERS, self.paper)), format!("culoare-{}", key_of(&COLORS, self.color))]
    }
}
