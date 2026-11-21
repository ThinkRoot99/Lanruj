// SPDX-License-Identifier: GPL-3.0-or-later
//! Fonturile care vin cu aplicația (toate sub SIL Open Font License, licențele în `data/fonts`):
//! Inter, Noto Sans, Caveat, Patrick Hand, Lora, Courier Prime, Nunito.
//! Sunt puse în program, deci merg oriunde e instalat Lanruj, fără să fie instalate în sistem.

use gtk::glib;
use pango::prelude::*;

/// (cheia din setări, familia, fișierul, conținutul, numele în română, în engleză, cât se mărește textul).
pub struct Font {
    pub key: &'static str,
    pub family: &'static str,
    /// Fișierele fontului: (numele, conținutul).
    files: &'static [(&'static str, &'static [u8])],
    pub ro: &'static str,
    pub en: &'static str,
    /// Scrisul de mână e mai mic la aceeași mărime; se mărește puțin ca să se citească ușor.
    pub scale: f64,
}

pub const FONTS: [Font; 7] = [
    Font { key: "inter", family: "Inter Variable", files: &[("InterVariable.ttf", include_bytes!("../../../data/fonts/InterVariable.ttf"))], ro: "Inter", en: "Inter", scale: 1.0 },
    Font {
        key: "noto",
        family: "Noto Sans",
        files: &[("NotoSans-Regular.ttf", include_bytes!("../../../data/fonts/NotoSans-Regular.ttf")), ("NotoSans-Bold.ttf", include_bytes!("../../../data/fonts/NotoSans-Bold.ttf"))],
        ro: "Noto Sans",
        en: "Noto Sans",
        scale: 1.0,
    },
    Font { key: "caveat", family: "Caveat", files: &[("Caveat-wght.ttf", include_bytes!("../../../data/fonts/Caveat-wght.ttf"))], ro: "Caveat (scris de mână)", en: "Caveat (handwriting)", scale: 1.3 },
    Font { key: "patrick", family: "Patrick Hand", files: &[("PatrickHand-Regular.ttf", include_bytes!("../../../data/fonts/PatrickHand-Regular.ttf"))], ro: "Patrick Hand (scris de mână)", en: "Patrick Hand (handwriting)", scale: 1.12 },
    Font { key: "lora", family: "Lora", files: &[("Lora-wght.ttf", include_bytes!("../../../data/fonts/Lora-wght.ttf"))], ro: "Lora (de carte)", en: "Lora (book)", scale: 1.0 },
    Font { key: "courier", family: "Courier Prime", files: &[("CourierPrime-Regular.ttf", include_bytes!("../../../data/fonts/CourierPrime-Regular.ttf"))], ro: "Courier Prime (mașină de scris)", en: "Courier Prime (typewriter)", scale: 1.0 },
    Font { key: "nunito", family: "Nunito", files: &[("Nunito-wght.ttf", include_bytes!("../../../data/fonts/Nunito-wght.ttf"))], ro: "Nunito (rotunjit)", en: "Nunito (rounded)", scale: 1.0 },
];

pub fn by_key(key: &str) -> Option<&'static Font> {
    FONTS.iter().find(|f| f.key == key)
}

/// Pune fonturile la îndemâna aplicației: le scrie o dată în dosarul de cache și le dă lui Pango.
pub fn register() {
    let dir = glib::user_cache_dir().join("lanruj").join("fonturi");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let map = pangocairo::FontMap::default();
    for f in &FONTS {
        for (file, data) in f.files {
            let path = dir.join(file);
            let same = std::fs::metadata(&path).map(|m| m.len() as usize == data.len()).unwrap_or(false);
            if !same && std::fs::write(&path, data).is_err() {
                continue;
            }
            if let Err(e) = map.add_font_file(&path) {
                eprintln!("lanruj: fontul {}: {e}", f.family);
            }
        }
    }
}
