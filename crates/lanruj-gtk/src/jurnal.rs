// SPDX-License-Identifier: GPL-3.0-or-later
//! Jurnalul propriu-zis: o pagină mare de caiet pentru fiecare zi, în care scrii liber și pe larg.
//! Scrisul se vede direct (îngroșat, cursiv, culori…), iar pe disc rămâne Markdown
//! (`jurnal/AAAA/AAAA-LL-ZZ.md`, vezi `lanruj_core::diary`). Se salvează singur, la o clipă după scris.
//!
//! Etichetele din câmp: `s:bold`, `s:italic`, `s:strike`, `s:underline`; culorile și legăturile
//! ca în pagini (`f#…`, `b#…`, `l…`); felul rândului: `r:ora`, `r:titlu`, `r:citat`.

use crate::page::{color_tag, link_tag, open_uri};
use crate::{App, View, tr};
use adw::prelude::*;
use chrono::{Datelike, NaiveDate};
use gtk::glib;
use lanruj_core::diary::{Line, LineKind, Span, Style, Target};
use lanruj_core::{Kind, PageRef, TaskState};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct Diary {
    pub root: gtk::Box,
    heading: gtk::Label,
    words: gtk::Label,
    pub view: gtk::TextView,
    /// Rândurile din alte zile ale jurnalului care trimit aici.
    mentions: gtk::Box,
    date: Cell<Option<NaiveDate>>,
    loading: Cell<bool>,
    save_id: RefCell<Option<glib::SourceId>>,
    /// Ce s-a scris adineauri (poziția și câte caractere), ca să primească scrisul potrivit.
    inserted: Cell<Option<(i32, i32)>>,
    /// Scrisul ales fără selecție (B, I, culoare…), valabil cât cursorul stă pe loc.
    typing: RefCell<Option<(i32, Style)>>,
    /// Felul ales pentru un rând încă gol (titlu, citat).
    pending_line: Cell<Option<(i32, &'static str)>>,
    night: Cell<bool>,
    lang: lanruj_core::Lang,
    buttons: Vec<(&'static str, gtk::Button)>,
    bar: adw::WrapBox,
}

const INLINE: [(&str, &str); 4] = [("s:bold", "bold"), ("s:italic", "italic"), ("s:strike", "strike"), ("s:underline", "underline")];

fn is_inline(name: &str) -> bool {
    name.starts_with("s:") || name.starts_with("f#") || name.starts_with("b#") || name.starts_with('l')
}

fn tag(buf: &gtk::TextBuffer, name: &str) -> gtk::TextTag {
    if let Some(t) = buf.tag_table().lookup(name) {
        return t;
    }
    let t = gtk::TextTag::new(Some(name));
    match name {
        "s:bold" => t.set_weight(700),
        "s:italic" => t.set_style(gtk::pango::Style::Italic),
        "s:strike" => t.set_strikethrough(true),
        "s:underline" => t.set_underline(gtk::pango::Underline::Single),
        "r:ora" => {
            t.set_weight(700);
            t.set_pixels_above_lines(12);
            t.set_foreground(Some("#1c71d8"));
        }
        "r:titlu" => {
            t.set_weight(700);
            t.set_scale(1.3);
            t.set_pixels_above_lines(10);
        }
        "r:citat" => {
            t.set_style(gtk::pango::Style::Italic);
            t.set_left_margin(56);
            t.set_foreground(Some("#6b6b6b"));
        }
        _ => {}
    }
    buf.tag_table().add(&t);
    t
}

fn line_tag_name(k: LineKind) -> Option<&'static str> {
    match k {
        LineKind::Time => Some("r:ora"),
        LineKind::Title => Some("r:titlu"),
        LineKind::Quote => Some("r:citat"),
        LineKind::Text => None,
    }
}

/// Stilul caracterului de la `it`.
fn style_at(it: &gtk::TextIter) -> Style {
    let mut st = Style::default();
    for t in it.tags() {
        let n = t.name().map(|n| n.to_string()).unwrap_or_default();
        match n.as_str() {
            "s:bold" => st.bold = true,
            "s:italic" => st.italic = true,
            "s:strike" => st.strike = true,
            "s:underline" => st.underline = true,
            _ => {
                if let Some(h) = n.strip_prefix('f') {
                    st.color = Some(h.to_string());
                } else if let Some(h) = n.strip_prefix('b') {
                    st.marker = Some(h.to_string());
                } else if let Some(u) = n.strip_prefix('l') {
                    st.link = Some(u.to_string());
                }
            }
        }
    }
    st
}

fn style_tags(buf: &gtk::TextBuffer, st: &Style, night: bool) -> Vec<gtk::TextTag> {
    let mut v = Vec::new();
    for (on, name) in [(st.bold, "s:bold"), (st.italic, "s:italic"), (st.strike, "s:strike"), (st.underline, "s:underline")] {
        if on {
            v.push(tag(buf, name));
        }
    }
    if let Some(c) = &st.color {
        v.push(color_tag(buf, false, c, night));
    }
    if let Some(m) = &st.marker {
        v.push(color_tag(buf, true, m, night));
    }
    if let Some(l) = &st.link {
        v.push(link_tag(buf, l, night));
    }
    v
}

fn set_style(buf: &gtk::TextBuffer, a: &gtk::TextIter, b: &gtk::TextIter, st: &Style, night: bool) {
    let mut all = Vec::new();
    buf.tag_table().foreach(|t| {
        if t.name().is_some_and(|n| is_inline(&n)) {
            all.push(t.clone());
        }
    });
    for t in &all {
        buf.remove_tag(t, a, b);
    }
    for t in style_tags(buf, st, night) {
        buf.apply_tag(&t, a, b);
    }
}

fn line_bounds(buf: &gtk::TextBuffer, line: i32) -> Option<(gtk::TextIter, gtk::TextIter)> {
    let a = buf.iter_at_line(line)?;
    let mut b = a;
    if !b.ends_line() {
        b.forward_to_line_end();
    }
    Some((a, b))
}

fn line_kind(buf: &gtk::TextBuffer, line: i32) -> Option<&'static str> {
    let (mut a, b) = line_bounds(buf, line)?;
    while a.offset() < b.offset() {
        for n in ["r:ora", "r:titlu", "r:citat"] {
            if let Some(t) = buf.tag_table().lookup(n)
                && a.has_tag(&t)
            {
                return Some(n);
            }
        }
        a.forward_char();
    }
    None
}

fn set_line_kind(buf: &gtk::TextBuffer, line: i32, kind: Option<&str>) {
    let Some((a, b)) = line_bounds(buf, line) else { return };
    for n in ["r:ora", "r:titlu", "r:citat"] {
        buf.remove_tag(&tag(buf, n), &a, &b);
    }
    if let Some(k) = kind {
        buf.apply_tag(&tag(buf, k), &a, &b);
    }
}

fn line_text(buf: &gtk::TextBuffer, line: i32) -> String {
    line_bounds(buf, line).map(|(a, b)| buf.text(&a, &b, false).to_string()).unwrap_or_default()
}

/// Semnul de listă de la începutul rândului: (lungimea în caractere, semnul pentru rândul următor).
fn list_prefix(text: &str) -> Option<(usize, String)> {
    let indent = text.len() - text.trim_start_matches(' ').len();
    let rest = &text[indent..];
    let pad = " ".repeat(indent);
    for s in ["• ", "☐ ", "☑ "] {
        if rest.starts_with(s) {
            let next = if s == "☑ " { "☐ " } else { s };
            return Some((indent + 2, format!("{pad}{next}")));
        }
    }
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() && rest[digits.len()..].starts_with(". ") {
        let n: u32 = digits.parse().ok()?;
        return Some((indent + digits.len() + 2, format!("{pad}{}. ", n + 1)));
    }
    None
}

impl Diary {
    pub fn new(lang: lanruj_core::Lang) -> Rc<Diary> {
        let view = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::WordChar)
            .vexpand(true)
            .left_margin(36)
            .right_margin(36)
            .top_margin(8)
            .bottom_margin(40)
            .pixels_inside_wrap(2)
            .pixels_below_lines(4)
            .css_classes(["jurnal-text"])
            .build();
        view.set_has_tooltip(true);
        let scroll = gtk::ScrolledWindow::builder().child(&view).hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build();

        let heading = gtk::Label::builder().xalign(0.0).wrap(true).css_classes(["titlu-pagina"]).build();
        let words = gtk::Label::builder().xalign(0.0).css_classes(["eticheta-pagina"]).build();
        let head = gtk::Box::new(gtk::Orientation::Vertical, 2);
        head.set_margin_top(22);
        head.set_margin_start(36);
        head.set_margin_end(36);
        head.append(&words);
        head.append(&heading);

        // Bara de scris.
        // Pe fereastră îngustă, uneltele trec pe rândul următor.
        let bar = adw::WrapBox::builder().child_spacing(2).line_spacing(2).build();
        bar.add_css_class("bara-jurnal");
        bar.set_margin_start(30);
        bar.set_margin_end(30);
        bar.set_margin_top(8);
        bar.set_margin_bottom(4);
        let mut buttons: Vec<(&'static str, gtk::Button)> = Vec::new();
        let mk = |id: &'static str, icon: Option<&str>, label: Option<&str>, tip: &str| {
            let b = gtk::Button::builder().focus_on_click(false).css_classes(["flat", "unealta-jurnal"]).tooltip_text(tip).build();
            match (icon, label) {
                (Some(i), _) => b.set_icon_name(i),
                (None, Some(l)) => b.set_label(l),
                _ => {}
            }
            (id, b)
        };
        let groups: Vec<Vec<(&'static str, gtk::Button)>> = vec![
            vec![
                mk("bold", Some("format-text-bold-symbolic"), None, tr(lang, "Îngroșat (Ctrl+B)", "Bold (Ctrl+B)")),
                mk("italic", Some("format-text-italic-symbolic"), None, tr(lang, "Cursiv (Ctrl+I)", "Italic (Ctrl+I)")),
                mk("underline", Some("format-text-underline-symbolic"), None, tr(lang, "Subliniat (Ctrl+U)", "Underline (Ctrl+U)")),
                mk("strike", Some("format-text-strikethrough-symbolic"), None, tr(lang, "Tăiat (Ctrl+D)", "Strikethrough (Ctrl+D)")),
            ],
            vec![
                mk("titlu", None, Some("T"), tr(lang, "Titlu", "Heading")),
                mk("citat", None, Some("„“"), tr(lang, "Citat", "Quote")),
                mk("lista", Some("view-list-bullet-symbolic"), None, tr(lang, "Listă", "List")),
                mk("numere", Some("view-list-ordered-symbolic"), None, tr(lang, "Listă numerotată", "Numbered list")),
                mk("casute", None, Some("☐"), tr(lang, "Listă cu căsuțe", "Checklist")),
            ],
            vec![
                mk("ora", Some("preferences-system-time-symbolic"), None, tr(lang, "Ora de acum (Ctrl+Shift+O): o scriere nouă în aceeași zi", "The time now (Ctrl+Shift+O): a new entry on the same day")),
                mk("leaga", Some("x-office-calendar-symbolic"), None, tr(lang, "Leagă de o zi, un rând, o pagină sau Monitorizare (Ctrl+L)", "Link to a day, an entry, a page or Tracking (Ctrl+L)")),
            ],
            vec![mk("curata", Some("edit-clear-all-symbolic"), None, tr(lang, "Fără formatare", "Clear formatting"))],
        ];
        for (k, g) in groups.into_iter().enumerate() {
            if k > 0 {
                bar.append(&gtk::Separator::new(gtk::Orientation::Vertical));
            }
            for (id, b) in g {
                bar.append(&b);
                buttons.push((id, b));
            }
            if k == 0 {
                // Culorile și markerul, ca în pagini (același meniu).
                bar.append(&gtk::Separator::new(gtk::Orientation::Vertical));
            }
        }
        let paper = gtk::Box::new(gtk::Orientation::Vertical, 0);
        paper.add_css_class("pagina");
        paper.add_css_class("singura");
        paper.add_css_class("foaie-jurnal");
        paper.append(&head);
        paper.append(&bar);
        paper.append(&scroll);
        let mentions = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).visible(false).css_classes(["pomeniri"]).build();
        mentions.set_margin_start(36);
        mentions.set_margin_end(36);
        mentions.set_margin_bottom(14);
        paper.append(&mentions);
        let clamp = adw::Clamp::builder().maximum_size(980).tightening_threshold(700).child(&paper).build();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("caiet");
        root.append(&clamp);
        clamp.set_vexpand(true);
        Rc::new(Diary {
            root,
            heading,
            words,
            view,
            mentions,
            date: Cell::new(None),
            loading: Cell::new(false),
            save_id: RefCell::new(None),
            inserted: Cell::new(None),
            typing: RefCell::new(None),
            pending_line: Cell::new(None),
            night: Cell::new(false),
            lang,
            buttons,
            bar,
        })
    }

    fn button(&self, id: &str) -> gtk::Button {
        self.buttons.iter().find(|(i, _)| *i == id).map(|(_, b)| b.clone()).expect("buton")
    }

    fn buf(&self) -> gtk::TextBuffer {
        self.view.buffer()
    }

    /// Pune textul zilei în câmp.
    fn load(&self, md: &str) {
        let buf = self.buf();
        let night = self.night.get();
        self.loading.set(true);
        buf.set_text("");
        for (k, line) in lanruj_core::diary::parse_doc(md).iter().enumerate() {
            if k > 0 {
                let mut end = buf.end_iter();
                buf.insert(&mut end, "\n");
            }
            let start = buf.end_iter().offset();
            for sp in &line.spans {
                let s = buf.end_iter().offset();
                let mut end = buf.end_iter();
                buf.insert(&mut end, &sp.text);
                set_style(&buf, &buf.iter_at_offset(s), &buf.end_iter(), &sp.style, night);
            }
            if let Some(n) = line_tag_name(line.kind) {
                buf.apply_tag(&tag(&buf, n), &buf.iter_at_offset(start), &buf.end_iter());
            }
        }
        buf.place_cursor(&buf.end_iter());
        self.typing.replace(None);
        self.loading.set(false);
        self.count_words();
    }

    /// Textul din câmp, în Markdown.
    pub fn serialize(&self) -> String {
        let buf = self.buf();
        let mut lines = Vec::new();
        for l in 0..buf.line_count() {
            let Some((mut a, b)) = line_bounds(&buf, l) else { continue };
            let kind = match line_kind(&buf, l) {
                Some("r:ora") => LineKind::Time,
                Some("r:titlu") => LineKind::Title,
                Some("r:citat") => LineKind::Quote,
                _ => LineKind::Text,
            };
            let mut spans: Vec<Span> = Vec::new();
            while a.offset() < b.offset() {
                let st = style_at(&a);
                let c = a.char();
                match spans.last_mut() {
                    Some(s) if s.style == st => s.text.push(c),
                    _ => spans.push(Span { text: c.to_string(), style: st }),
                }
                a.forward_char();
            }
            lines.push(Line { kind, spans });
        }
        lanruj_core::diary::to_doc(&lines)
    }

    fn count_words(&self) {
        let buf = self.buf();
        let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
        let n = text.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count();
        let ro = self.lang == lanruj_core::Lang::Ro;
        self.words.set_label(&match n {
            0 => String::new(),
            1 if ro => "1 cuvânt".to_string(),
            1 => "1 word".to_string(),
            n if !ro => format!("{n} words"),
            n if n % 100 >= 20 || n % 100 == 0 => format!("{n} de cuvinte"),
            n => format!("{n} cuvinte"),
        });
    }

    /// Stilul pentru scrisul de la cursor: cel ales cu butoanele sau cel al textului dinainte.
    fn style_here(&self) -> Style {
        let buf = self.buf();
        let at = buf.iter_at_mark(&buf.get_insert());
        if let Some((o, st)) = self.typing.borrow().as_ref()
            && *o == at.offset()
        {
            return st.clone();
        }
        let mut prev = at;
        if prev.backward_char() && prev.char() != '\n' {
            let mut st = style_at(&prev);
            // La capătul unei legături nu se scrie mai departe în ea.
            if st.link.is_some() && style_at(&at).link != st.link {
                st.link = None;
            }
            return st;
        }
        Style::default()
    }

    /// Aprinde butoanele după scrisul de la cursor.
    fn show_state(&self) {
        let buf = self.buf();
        let st = match buf.selection_bounds() {
            Some((a, _)) => style_at(&a),
            None => self.style_here(),
        };
        for (id, on) in [("bold", st.bold), ("italic", st.italic), ("underline", st.underline), ("strike", st.strike)] {
            let b = self.button(id);
            if on { b.add_css_class("aprins") } else { b.remove_css_class("aprins") }
        }
        let line = buf.iter_at_mark(&buf.get_insert()).line();
        let kind = line_kind(&buf, line);
        for (id, n) in [("titlu", "r:titlu"), ("citat", "r:citat")] {
            let b = self.button(id);
            if kind == Some(n) { b.add_css_class("aprins") } else { b.remove_css_class("aprins") }
        }
    }
}

impl App {
    pub fn go_diary(self: &Rc<Self>, d: NaiveDate) {
        self.flush_diary();
        self.leave_search();
        if self.migrating() {
            self.end_migration(false);
        }
        self.editing.set(None);
        self.selected.set(None);
        self.view.set(View::Diary(d));
        self.refresh();
    }

    /// Salvează pe loc ce așteaptă să fie salvat.
    pub fn flush_diary(self: &Rc<Self>) {
        if let Some(id) = self.diary.save_id.borrow_mut().take() {
            id.remove();
            self.save_diary_now();
        }
    }

    fn save_diary_now(self: &Rc<Self>) {
        let Some(d) = self.diary.date.get() else { return };
        let md = self.diary.serialize();
        if let Err(e) = self.journal.save_diary(d, &md, self.lang) {
            self.error(&e);
        }
        self.diary_links.replace(None);
        self.mark_calendar();
    }

    fn schedule_diary_save(self: &Rc<Self>) {
        if self.diary.loading.get() {
            return;
        }
        if let Some(id) = self.diary.save_id.borrow_mut().take() {
            id.remove();
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
            if let Some(app) = weak.upgrade() {
                app.diary.save_id.replace(None);
                app.save_diary_now();
            }
        });
        self.diary.save_id.replace(Some(id));
    }

    /// Arată jurnalul zilei `d` (îl citește din nou doar dacă e altă zi).
    pub fn refresh_diary(self: &Rc<Self>, d: NaiveDate) {
        let night = self.window_night();
        if self.diary.date.get() != Some(d) || self.diary.night.get() != night {
            self.flush_diary();
            self.diary.night.set(night);
            self.diary.date.set(Some(d));
            match self.journal.load_diary(d) {
                Ok(md) => self.diary.load(&md),
                Err(e) => {
                    self.diary.load("");
                    self.error(&e);
                }
            }
        }
        self.diary.heading.set_label(&self.lang.day_title(d));
        self.title.set_title(tr(self.lang, "Jurnal", "Journal"));
        self.title.set_subtitle(&self.lang.day_title_short(d));
        self.fill_diary_mentions(d);
    }

    /// „Pomenit în jurnal”: legăturile din alte zile spre ziua aceasta (sau spre jurnalul ei).
    fn fill_diary_mentions(self: &Rc<Self>, d: NaiveDate) {
        let b = &self.diary.mentions;
        while let Some(c) = b.first_child() {
            b.remove(&c);
        }
        let links: Vec<_> = self
            .all_diary_links()
            .into_iter()
            .filter(|l| l.diary != d && matches!(&l.target, Target::Diary(x) if *x == d))
            .collect();
        b.set_visible(!links.is_empty());
        if links.is_empty() {
            return;
        }
        b.append(&gtk::Label::builder().label(tr(self.lang, "Pomenită în jurnalul altor zile", "Mentioned on other days")).xalign(0.0).css_classes(["eticheta-pagina"]).build());
        for l in links {
            b.append(&self.mention_button(l.diary, &l.text));
        }
    }

    fn mention_button(self: &Rc<Self>, diary: NaiveDate, text: &str) -> gtk::Button {
        let short: String = text.chars().take(60).collect();
        let b = gtk::Button::builder()
            .label(format!("{} · „{short}”", self.lang.day_title_short(diary)))
            .css_classes(["flat", "pomenire"])
            .halign(gtk::Align::Start)
            .tooltip_text(tr(self.lang, "Deschide jurnalul zilei", "Open that day's journal"))
            .build();
        let weak = Rc::downgrade(self);
        b.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.go_diary(diary);
            }
        });
        b
    }

    /// Toate legăturile din jurnal (ținute minte până la următoarea salvare).
    pub fn all_diary_links(&self) -> Vec<lanruj_core::diary::DiaryLink> {
        if self.diary_links.borrow().is_none() {
            self.diary_links.replace(Some(self.journal.diary_links()));
        }
        self.diary_links.borrow().clone().unwrap_or_default()
    }

    /// Zilele de jurnal care pomenesc rândul cu semnul `id` din pagina `r`.
    pub fn entry_mentions(&self, r: PageRef, id: &str) -> Vec<(NaiveDate, String)> {
        self.all_diary_links()
            .into_iter()
            .filter(|l| matches!(&l.target, Target::Entry(rr, i) if *rr == r && i == id))
            .map(|l| (l.diary, l.text))
            .collect()
    }

    /// Legăturile din jurnal spre Monitorizarea unei luni.
    pub fn tracking_mentions(&self, y: i32, m: u32) -> Vec<(NaiveDate, String)> {
        self.all_diary_links()
            .into_iter()
            .filter(|l| matches!(&l.target, Target::Tracking { year, month, .. } if *year == y && *month == m))
            .map(|l| (l.diary, l.text))
            .collect()
    }

    /// „Pomenit în jurnal” pentru Monitorizare: o cutie de pus sub pagină.
    pub fn tracking_mentions_box(self: &Rc<Self>, y: i32, m: u32) -> Option<gtk::Box> {
        let links = self.tracking_mentions(y, m);
        if links.is_empty() {
            return None;
        }
        let b = gtk::Box::new(gtk::Orientation::Vertical, 2);
        b.add_css_class("pomeniri");
        b.append(&gtk::Label::builder().label(tr(self.lang, "Pomenită în jurnal", "Mentioned in the journal")).xalign(0.0).css_classes(["eticheta-pagina"]).build());
        for (d, t) in links {
            b.append(&self.mention_button(d, &t));
        }
        Some(b)
    }

    fn go_page(self: &Rc<Self>, r: PageRef) {
        match r {
            PageRef::Day(d) => self.go(d),
            PageRef::Week { year, week } => self.go_week(year, week),
            PageRef::Month { year, month } => self.go_month(year, month),
            PageRef::Future { year, month } => self.go_future(year, month),
            PageRef::Topic { date, n } => {
                self.leave_search();
                self.editing.set(None);
                self.selected.set(None);
                self.view.set(View::Topic(date, n));
                self.refresh();
            }
        }
    }

    /// Deschide o legătură din jurnal: în Lanruj (`lanruj:`) sau în programul potrivit.
    pub fn open_link(self: &Rc<Self>, url: &str) {
        let Some(t) = Target::parse(url) else {
            open_uri(&self.window, url);
            return;
        };
        match t {
            Target::Page(r) => {
                self.flush_diary();
                self.go_page(r);
            }
            Target::Entry(r, id) => {
                self.flush_diary();
                let pos = self.journal.load_ref(r).ok().and_then(|d| d.entries().position(|e| e.id.as_deref() == Some(id.as_str())));
                match pos {
                    Some(i) => self.open_result(r, i),
                    None => {
                        self.go_page(r);
                        self.toast(tr(self.lang, "Rândul nu mai e pe pagina aceasta (poate a fost mutat sau șters).", "The entry is no longer on that page (it may have been moved or deleted)."));
                    }
                }
            }
            Target::Diary(d) => self.go_diary(d),
            Target::Tracking { year, month, tab } => {
                self.flush_diary();
                self.tracking_tab.set(match tab.as_deref() {
                    Some("somn") => crate::monitorizare::Tab::Sleep,
                    Some("serviciu") => crate::monitorizare::Tab::Work,
                    _ => crate::monitorizare::Tab::Activities,
                });
                self.tracking_focus.set(None);
                self.go_tracking(year, month);
            }
            Target::Index => {
                self.flush_diary();
                self.go_index();
            }
        }
    }

    /// Din meniul unui rând: „Scrie în jurnal despre asta”. Deschide jurnalul de azi și pune la
    /// sfârșit o legătură spre rând, gata de scris mai departe.
    pub fn write_about_entry(self: &Rc<Self>, r: PageRef, index: usize) {
        let text = match self.journal.load_ref(r) {
            Ok(d) => d.entries().nth(index).map(|e| lanruj_core::colors::plain(&e.text)).unwrap_or_default(),
            Err(e) => return self.error(&e),
        };
        let id = match self.journal.entry_anchor(r, index, self.lang) {
            Ok(Some(id)) => id,
            Ok(None) => return,
            Err(e) => return self.error(&e),
        };
        self.go_diary(self.today.get());
        let buf = self.diary.buf();
        let mut end = buf.end_iter();
        let has_text = end.offset() > 0;
        if has_text {
            buf.insert(&mut end, "\n\n");
        }
        let s = buf.end_iter().offset();
        let mut end = buf.end_iter();
        let label = if text.trim().is_empty() { tr(self.lang, "rândul acesta", "this entry").to_string() } else { text };
        buf.insert(&mut end, &label);
        let st = Style { link: Some(Target::Entry(r, id).to_url()), ..Style::default() };
        set_style(&buf, &buf.iter_at_offset(s), &buf.end_iter(), &st, self.diary.night.get());
        let mut end = buf.end_iter();
        buf.insert(&mut end, " ");
        set_style(&buf, &buf.iter_at_offset(end.offset() - 1), &buf.end_iter(), &Style::default(), self.diary.night.get());
        buf.place_cursor(&buf.end_iter());
        self.diary.view.grab_focus();
        self.schedule_diary_save();
    }

    /// Leagă selecția (sau, fără selecție, un text nou) de `target`.
    fn diary_link_to(self: &Rc<Self>, sel: Option<(i32, i32)>, target: &Target, label: &str) {
        let buf = self.diary.buf();
        let night = self.diary.night.get();
        let url = target.to_url();
        let (a, b) = match sel {
            Some((a, b)) if a != b => (a, b),
            _ => {
                let mut at = buf.iter_at_mark(&buf.get_insert());
                let s = at.offset();
                buf.insert(&mut at, label);
                (s, s + label.chars().count() as i32)
            }
        };
        let (ia, ib) = (buf.iter_at_offset(a), buf.iter_at_offset(b));
        let mut links = Vec::new();
        buf.tag_table().foreach(|t| {
            if t.name().is_some_and(|n| n.starts_with('l')) {
                links.push(t.clone());
            }
        });
        for t in &links {
            buf.remove_tag(t, &ia, &ib);
        }
        buf.apply_tag(&link_tag(&buf, &url, night), &ia, &ib);
        buf.place_cursor(&buf.iter_at_offset(b));
        self.diary.view.grab_focus();
        self.schedule_diary_save();
    }

    /// Fereastra „Leagă de…”: o zi din calendar și, pentru ea, rândurile, paginile și Monitorizarea.
    fn diary_link_dialog(self: &Rc<Self>) {
        let lang = self.lang;
        let buf = self.diary.buf();
        let sel = buf.selection_bounds().map(|(a, b)| (a.offset(), b.offset()));
        let d0 = self.diary.date.get().unwrap_or(self.today.get());
        let dlg = adw::Dialog::builder().title(tr(lang, "Leagă de…", "Link to…")).content_width(560).content_height(620).build();
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        body.set_margin_top(6);
        body.set_margin_bottom(14);
        body.set_margin_start(14);
        body.set_margin_end(14);
        let cal = gtk::Calendar::new();
        cal.set_valign(gtk::Align::Start);
        if let Ok(dt) = glib::DateTime::from_local(d0.year(), d0.month() as i32, d0.day() as i32, 12, 0, 0.0) {
            cal.select_day(&dt);
        }
        let left = gtk::Box::new(gtk::Orientation::Vertical, 8);
        left.append(&cal);
        left.append(
            &gtk::Label::builder()
                .label(if sel.is_some_and(|(a, b)| a != b) {
                    tr(lang, "Textul selectat devine legătura.", "The selected text becomes the link.")
                } else {
                    tr(lang, "Fără selecție, se scrie numele locului, ca legătură.", "With no selection, the place's name is written as a link.")
                })
                .wrap(true)
                .max_width_chars(24)
                .xalign(0.0)
                .css_classes(["dim-label", "caption"])
                .build(),
        );
        body.append(&left);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let scroll = gtk::ScrolledWindow::builder().child(&list).hexpand(true).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build();
        body.append(&scroll);
        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&adw::HeaderBar::new());
        tv.set_content(Some(&body));
        dlg.set_child(Some(&tv));

        let fill: Rc<dyn Fn(NaiveDate)> = {
            let (weak, list, dlg) = (Rc::downgrade(self), list.clone(), dlg.clone());
            Rc::new(move |d: NaiveDate| {
                let Some(app) = weak.upgrade() else { return };
                while let Some(c) = list.first_child() {
                    list.remove(&c);
                }
                let section = |t: &str| {
                    let l = gtk::Label::builder().label(t).xalign(0.0).css_classes(["heading"]).build();
                    l.set_margin_top(10);
                    l
                };
                let row = |label: &str, target: Target, name: String| {
                    let b = gtk::Button::builder().css_classes(["flat", "rand-legatura"]).build();
                    let l = gtk::Label::builder().label(label).xalign(0.0).wrap(true).wrap_mode(gtk::pango::WrapMode::WordChar).build();
                    b.set_child(Some(&l));
                    let (weak, dlg) = (Rc::downgrade(&app), dlg.clone());
                    b.connect_clicked(move |_| {
                        dlg.close();
                        if let Some(app) = weak.upgrade() {
                            app.diary_link_to(sel, &target, &name);
                        }
                    });
                    b
                };
                // Rândurile: ziua, planul săptămânii, lista lunii.
                let wk = lanruj_core::week_of(d);
                let pages = [
                    (PageRef::Day(d), format!("{} · {}", tr(lang, "Pagina zilei", "Day page"), lang.day_title_short(d))),
                    (wk, tr(lang, "Planul săptămânii", "Week plan").to_string()),
                    (PageRef::Month { year: d.year(), month: d.month() }, format!("{} · {}", tr(lang, "Lista lunii", "Month list"), lang.month(d.month()))),
                ];
                for (r, title) in pages {
                    let Ok(day) = app.journal.load_ref(r) else { continue };
                    let entries: Vec<_> = day.entries().cloned().collect();
                    if entries.is_empty() {
                        continue;
                    }
                    list.append(&section(&title));
                    for (i, e) in entries.iter().enumerate() {
                        let sign = match e.kind {
                            Kind::Task(TaskState::Done) => "✕",
                            Kind::Task(_) => "•",
                            Kind::Event => "○",
                            Kind::Note => "–",
                        };
                        let plain = lanruj_core::colors::plain(&e.text);
                        let time = e.time.as_deref().map(|t| format!("{t} ")).unwrap_or_default();
                        let b = gtk::Button::builder().css_classes(["flat", "rand-legatura"]).build();
                        let l = gtk::Label::builder()
                            .label(format!("{}{sign}  {time}{plain}", "    ".repeat(e.depth)))
                            .xalign(0.0)
                            .wrap(true)
                            .wrap_mode(gtk::pango::WrapMode::WordChar)
                            .build();
                        b.set_child(Some(&l));
                        let (weak, dlg) = (Rc::downgrade(&app), dlg.clone());
                        b.connect_clicked(move |_| {
                            dlg.close();
                            let Some(app) = weak.upgrade() else { return };
                            match app.journal.entry_anchor(r, i, app.lang) {
                                Ok(Some(id)) => app.diary_link_to(sel, &Target::Entry(r, id), &plain),
                                Ok(None) => {}
                                Err(e) => app.error(&e),
                            }
                        });
                        list.append(&b);
                    }
                }
                list.append(&section(tr(lang, "Pagini", "Pages")));
                let (y, m) = (d.year(), d.month());
                for (label, t, name) in [
                    (tr(lang, "Pagina zilei", "Day page"), Target::Page(PageRef::Day(d)), lang.day_title_short(d)),
                    (tr(lang, "Jurnalul zilei", "That day's journal"), Target::Diary(d), lang.day_title_short(d)),
                    (tr(lang, "Planul săptămânii", "Week plan"), Target::Page(wk), tr(lang, "planul săptămânii", "the week plan").to_string()),
                    (tr(lang, "Pagina lunii", "Month page"), Target::Page(PageRef::Month { year: y, month: m }), lang.month(m).to_lowercase()),
                    (tr(lang, "Planuri, luna aceasta", "Future log, this month"), Target::Page(PageRef::Future { year: y, month: m }), tr(lang, "planuri", "plans").to_string()),
                    ("Index", Target::Index, "index".to_string()),
                ] {
                    list.append(&row(label, t, name));
                }
                list.append(&section(&format!("{} · {}", tr(lang, "Monitorizare", "Tracking"), lang.month(m))));
                for (label, tab) in [(tr(lang, "Activități", "Activities"), "activitati"), (tr(lang, "Somn", "Sleep"), "somn"), (tr(lang, "Serviciu", "Work"), "serviciu")] {
                    list.append(&row(label, Target::Tracking { year: y, month: m, tab: Some(tab.into()) }, label.to_lowercase()));
                }
            })
        };
        fill(d0);
        let f2 = fill.clone();
        cal.connect_day_selected(move |c| {
            let dt = c.date();
            if let Some(d) = NaiveDate::from_ymd_opt(dt.year(), dt.month() as u32, dt.day_of_month() as u32) {
                f2(d);
            }
        });
        dlg.present(Some(&self.window));
    }

    /// Leagă câmpul și bara de restul aplicației (după ce `App` există).
    pub fn wire_diary(self: &Rc<Self>) {
        let dy = &self.diary;
        let buf = dy.buf();
        let lang = self.lang;
        // Culorile, markerul și legătura web: aceleași unelte ca în pagini.
        let colors = crate::page::color_bar(lang, crate::page::ColorTarget::Field(dy.view.clone(), self.window_night()));
        // Fără selecție, culoarea se pune pe ce urmează să scrii (nu pe tot textul).
        colors.add_css_class("bara-jurnal-culori");
        let pos = dy.buttons.iter().position(|(id, _)| *id == "strike").expect("buton");
        let after = dy.buttons[pos].1.next_sibling().expect("separator");
        dy.bar.insert_child_after(&colors, Some(&after));

        let ins = Rc::downgrade(self);
        buf.connect_insert_text(move |_, it, text| {
            if let Some(app) = ins.upgrade()
                && !app.diary.loading.get()
            {
                app.diary.inserted.set(Some((it.offset(), text.chars().count() as i32)));
            }
        });
        let weak = Rc::downgrade(self);
        buf.connect_changed(move |buf| {
            let Some(app) = weak.upgrade() else { return };
            let dy = &app.diary;
            if dy.loading.get() {
                return;
            }
            if let Some((o, n)) = dy.inserted.take() {
                // Scrisul nou ia stilul ales sau pe cel al textului dinainte.
                let at = buf.iter_at_offset(o);
                let st = {
                    let typed = dy.typing.borrow().as_ref().filter(|(off, _)| *off == o).map(|(_, s)| s.clone());
                    typed.unwrap_or_else(|| {
                        let mut prev = at;
                        if prev.backward_char() && prev.char() != '\n' {
                            let mut st = style_at(&prev);
                            if st.link.is_some() && style_at(&buf.iter_at_offset(o + n)).link != st.link {
                                st.link = None;
                            }
                            st
                        } else {
                            Style::default()
                        }
                    })
                };
                dy.loading.set(true);
                set_style(buf, &at, &buf.iter_at_offset(o + n), &st, dy.night.get());
                // Felul rândului (oră, titlu, citat) se întinde pe tot rândul.
                let line = buf.iter_at_offset(o + n).line();
                let pending = dy.pending_line.get().filter(|(l, _)| *l == line).map(|(_, k)| k);
                if let Some(k) = pending.or_else(|| line_kind(buf, line)) {
                    set_line_kind(buf, line, Some(k));
                }
                dy.pending_line.set(None);
                dy.loading.set(false);
                if dy.typing.borrow().is_some() {
                    dy.typing.replace(Some((o + n, st)));
                }
            }
            dy.count_words();
            app.schedule_diary_save();
        });
        let weak = Rc::downgrade(self);
        buf.connect_apply_tag(move |_, _, _, _| {
            if let Some(app) = weak.upgrade() {
                app.schedule_diary_save();
            }
        });
        let weak = Rc::downgrade(self);
        buf.connect_remove_tag(move |_, _, _, _| {
            if let Some(app) = weak.upgrade() {
                app.schedule_diary_save();
            }
        });
        let weak = Rc::downgrade(self);
        buf.connect_mark_set(move |buf, it, mark| {
            let Some(app) = weak.upgrade() else { return };
            if *mark != buf.get_insert() {
                return;
            }
            let stale = app.diary.typing.borrow().as_ref().is_some_and(|(o, _)| *o != it.offset());
            if stale {
                app.diary.typing.replace(None);
            }
            app.diary.show_state();
        });

        for (id, _) in INLINE.iter().map(|(n, id)| (*id, *n)) {
            let b = dy.button(id);
            let weak = Rc::downgrade(self);
            b.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.diary_toggle(id);
                }
            });
        }
        for id in ["titlu", "citat"] {
            let weak = Rc::downgrade(self);
            dy.button(id).connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.diary_line_kind(if id == "titlu" { "r:titlu" } else { "r:citat" });
                }
            });
        }
        for (id, sign) in [("lista", "• "), ("numere", "1. "), ("casute", "☐ ")] {
            let weak = Rc::downgrade(self);
            dy.button(id).connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.diary_list(sign);
                }
            });
        }
        let weak = Rc::downgrade(self);
        dy.button("ora").connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.diary_time();
            }
        });
        let weak = Rc::downgrade(self);
        dy.button("leaga").connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.diary_link_dialog();
            }
        });
        let weak = Rc::downgrade(self);
        dy.button("curata").connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                let buf = app.diary.buf();
                match buf.selection_bounds() {
                    Some((a, b)) => {
                        set_style(&buf, &a, &b, &Style::default(), false);
                    }
                    None => {
                        let at = buf.iter_at_mark(&buf.get_insert()).offset();
                        app.diary.typing.replace(Some((at, Style::default())));
                        app.diary.show_state();
                    }
                }
                app.diary.view.grab_focus();
            }
        });

        // Tastele: B, I, U, D, L, Shift+O; Enter în liste; Tab în liste.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, mods| {
            use gtk::gdk::{Key, ModifierType};
            let Some(app) = weak.upgrade() else { return glib::Propagation::Proceed };
            let ctrl = mods.contains(ModifierType::CONTROL_MASK);
            let shift = mods.contains(ModifierType::SHIFT_MASK);
            if ctrl {
                let k = key.to_lower();
                let id = match k {
                    Key::b => Some("bold"),
                    Key::i => Some("italic"),
                    Key::u => Some("underline"),
                    Key::d => Some("strike"),
                    _ => None,
                };
                if let Some(id) = id {
                    app.diary_toggle(id);
                    return glib::Propagation::Stop;
                }
                if k == Key::l {
                    app.diary_link_dialog();
                    return glib::Propagation::Stop;
                }
                if k == Key::o && shift {
                    app.diary_time();
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }
            let buf = app.diary.buf();
            let cur = buf.iter_at_mark(&buf.get_insert());
            let line = cur.line();
            let text = line_text(&buf, line);
            if matches!(key, Key::Return | Key::KP_Enter) && !shift && buf.selection_bounds().is_none() {
                if let Some((len, next)) = list_prefix(&text) {
                    let content = text.chars().skip(len).collect::<String>();
                    if content.trim().is_empty() {
                        // Enter pe un rând de listă gol: lista se termină.
                        let (a, _) = line_bounds(&buf, line).expect("rând");
                        let mut b = a;
                        b.forward_chars(len as i32);
                        let (mut a, mut b) = (a, b);
                        buf.delete(&mut a, &mut b);
                    } else {
                        let mut at = cur;
                        buf.insert(&mut at, &format!("\n{next}"));
                    }
                    return glib::Propagation::Stop;
                }
                // După un titlu sau o oră, rândul următor e text obișnuit.
                if line_kind(&buf, line).is_some() && cur.ends_line() {
                    let mut at = cur;
                    buf.insert(&mut at, "\n");
                    set_line_kind(&buf, line + 1, None);
                    return glib::Propagation::Stop;
                }
            }
            if matches!(key, Key::Tab | Key::ISO_Left_Tab) && list_prefix(&text).is_some() {
                let (mut a, _) = line_bounds(&buf, line).expect("rând");
                if key == Key::ISO_Left_Tab || shift {
                    if text.starts_with("  ") {
                        let mut b = a;
                        b.forward_chars(2);
                        buf.delete(&mut a, &mut b);
                    }
                } else {
                    buf.insert(&mut a, "  ");
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        dy.view.add_controller(keys);

        // Clicurile: Ctrl+clic deschide legătura; clic pe ☐/☑ o bifează.
        let click = gtk::GestureClick::builder().button(1).build();
        let weak = Rc::downgrade(self);
        click.connect_released(move |g, _, x, y| {
            let Some(app) = weak.upgrade() else { return };
            let v = &app.diary.view;
            let buf = v.buffer();
            if buf.selection_bounds().is_some() {
                return;
            }
            let (bx, by) = v.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
            let Some(it) = v.iter_at_location(bx, by) else { return };
            let ctrl = g.current_event_state().contains(gtk::gdk::ModifierType::CONTROL_MASK);
            if ctrl && let Some(url) = style_at(&it).link {
                let app2 = app.clone();
                glib::idle_add_local_once(move || app2.open_link(&url));
                return;
            }
            let c = it.char();
            if c == '☐' || c == '☑' {
                let before = line_text(&buf, it.line()).chars().take(it.line_offset() as usize).collect::<String>();
                if before.trim().is_empty() {
                    let (mut a, mut b) = (it, it);
                    b.forward_char();
                    buf.delete(&mut a, &mut b);
                    buf.insert(&mut a, if c == '☐' { "☑" } else { "☐" });
                }
            }
        });
        dy.view.add_controller(click);
        let lang2 = lang;
        dy.view.connect_query_tooltip(move |v, x, y, _, tip| {
            let (bx, by) = v.window_to_buffer_coords(gtk::TextWindowType::Widget, x, y);
            let Some(it) = v.iter_at_location(bx, by) else { return false };
            let Some(url) = style_at(&it).link else { return false };
            let what = match Target::parse(&url) {
                Some(Target::Entry(..)) => tr(lang2, "un rând din jurnal", "a journal entry").to_string(),
                Some(Target::Diary(d)) => format!("{} {}", tr(lang2, "jurnalul din", "the journal of"), lang2.day_title_short(d)),
                Some(Target::Tracking { .. }) => tr(lang2, "Monitorizare", "Tracking").to_string(),
                Some(Target::Page(_)) | Some(Target::Index) => tr(lang2, "o pagină din jurnal", "a journal page").to_string(),
                None => url.clone(),
            };
            tip.set_text(Some(&format!("{what}\n{}", tr(lang2, "Ctrl+clic: deschide", "Ctrl+click: open"))));
            true
        });
        // Mouse-ul arată mâna peste legături când Ctrl e apăsat (altfel cursorul de text).
        let motion = gtk::EventControllerMotion::new();
        let v = dy.view.clone();
        motion.connect_motion(move |m, x, y| {
            let (bx, by) = v.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
            let on_link = v.iter_at_location(bx, by).is_some_and(|it| style_at(&it).link.is_some());
            let ctrl = m.current_event_state().contains(gtk::gdk::ModifierType::CONTROL_MASK);
            v.set_cursor_from_name(Some(if on_link && ctrl { "pointer" } else { "text" }));
        });
        dy.view.add_controller(motion);
    }

    /// B, I, U, tăiat: pe selecție (toată are → se scoate) sau pe ce urmează să scrii.
    fn diary_toggle(self: &Rc<Self>, id: &str) {
        let dy = &self.diary;
        let buf = dy.buf();
        let name = INLINE.iter().find(|(_, i)| *i == id).map(|(n, _)| *n).expect("stil");
        let t = tag(&buf, name);
        if let Some((a, b)) = buf.selection_bounds() {
            let mut it = a;
            let mut all = true;
            while it.offset() < b.offset() {
                if it.char() != '\n' && !it.has_tag(&t) {
                    all = false;
                    break;
                }
                it.forward_char();
            }
            if all { buf.remove_tag(&t, &a, &b) } else { buf.apply_tag(&t, &a, &b) }
        } else {
            let at = buf.iter_at_mark(&buf.get_insert()).offset();
            let mut st = dy.style_here();
            match id {
                "bold" => st.bold = !st.bold,
                "italic" => st.italic = !st.italic,
                "underline" => st.underline = !st.underline,
                _ => st.strike = !st.strike,
            }
            dy.typing.replace(Some((at, st)));
        }
        dy.show_state();
        dy.view.grab_focus();
    }

    /// Titlu sau citat pentru rândurile selectate (sau rândul cursorului); încă o dată îl scoate.
    fn diary_line_kind(self: &Rc<Self>, kind: &'static str) {
        let dy = &self.diary;
        let buf = dy.buf();
        let (a, b) = buf.selection_bounds().unwrap_or_else(|| {
            let i = buf.iter_at_mark(&buf.get_insert());
            (i, i)
        });
        let on = line_kind(&buf, a.line()) != Some(kind);
        for l in a.line()..=b.line() {
            if line_text(&buf, l).is_empty() && on {
                dy.pending_line.set(Some((l, kind)));
            }
            set_line_kind(&buf, l, on.then_some(kind));
        }
        self.schedule_diary_save();
        dy.show_state();
        dy.view.grab_focus();
    }

    /// Listă cu buline, numerotată sau cu căsuțe, pe rândurile selectate; încă o dată o scoate.
    fn diary_list(self: &Rc<Self>, sign: &str) {
        let dy = &self.diary;
        let buf = dy.buf();
        let (a, b) = buf.selection_bounds().unwrap_or_else(|| {
            let i = buf.iter_at_mark(&buf.get_insert());
            (i, i)
        });
        let (first, last) = (a.line(), b.line());
        let same = |t: &str| {
            let r = t.trim_start_matches(' ');
            match sign {
                "1. " => r.chars().take_while(|c| c.is_ascii_digit()).count() > 0 && list_prefix(t).is_some() && !r.starts_with(['•', '☐', '☑']),
                "☐ " => r.starts_with("☐ ") || r.starts_with("☑ "),
                _ => r.starts_with(sign),
            }
        };
        let remove = same(&line_text(&buf, first));
        let mut n = 1;
        for l in first..=last {
            let text = line_text(&buf, l);
            let indent = text.len() - text.trim_start_matches(' ').len();
            let old = list_prefix(&text).map(|(len, _)| len).unwrap_or(indent);
            let (mut x, _) = line_bounds(&buf, l).expect("rând");
            x.forward_chars(indent as i32);
            let mut y = x;
            y.forward_chars((old - indent) as i32);
            buf.delete(&mut x, &mut y);
            if !remove {
                let s = if sign == "1. " { format!("{n}. ") } else { sign.to_string() };
                buf.insert(&mut x, &s);
                set_style(&buf, &buf.iter_at_offset(x.offset() - s.chars().count() as i32), &x, &Style::default(), false);
                n += 1;
            }
        }
        dy.view.grab_focus();
    }

    /// Ora de acum, ca început al unei scrieri noi în aceeași zi.
    fn diary_time(self: &Rc<Self>) {
        let dy = &self.diary;
        let buf = dy.buf();
        let mut at = buf.iter_at_mark(&buf.get_insert());
        if !at.ends_line() {
            at.forward_to_line_end();
        }
        let line = at.line();
        let empty_line = line_text(&buf, line).trim().is_empty();
        let prev_empty = line == 0 || line_text(&buf, line - 1).trim().is_empty();
        let before = if buf.char_count() == 0 || (empty_line && prev_empty) {
            ""
        } else if empty_line {
            "\n"
        } else {
            "\n\n"
        };
        let t = chrono::Local::now().format("%H:%M").to_string();
        buf.insert(&mut at, before);
        let s = at.offset();
        buf.insert(&mut at, &t);
        let hline = buf.iter_at_offset(s).line();
        set_style(&buf, &buf.iter_at_offset(s), &at, &Style::default(), false);
        buf.insert(&mut at, "\n");
        set_line_kind(&buf, hline, Some("r:ora"));
        set_line_kind(&buf, hline + 1, None);
        buf.place_cursor(&at);
        dy.typing.replace(Some((at.offset(), Style::default())));
        dy.view.grab_focus();
        dy.view.scroll_to_mark(&buf.get_insert(), 0.1, false, 0.0, 0.0);
    }

    #[cfg(debug_assertions)]
    pub fn diary_test_toggle(self: &Rc<Self>, id: &str) {
        self.diary_toggle(id);
    }
    #[cfg(debug_assertions)]
    pub fn diary_test_time(self: &Rc<Self>) {
        self.diary_time();
    }
    #[cfg(debug_assertions)]
    pub fn diary_test_list(self: &Rc<Self>, sign: &str) {
        self.diary_list(sign);
    }
    /// Enter, ca de la tastatură (cu continuarea listei).
    #[cfg(debug_assertions)]
    pub fn diary_test_enter(self: &Rc<Self>) {
        let buf = self.diary.buf();
        let cur = buf.iter_at_mark(&buf.get_insert());
        let text = line_text(&buf, cur.line());
        let mut at = cur;
        match list_prefix(&text) {
            Some((_, next)) => buf.insert(&mut at, &format!("\n{next}")),
            None => buf.insert(&mut at, "\n"),
        }
    }
    /// Leagă cuvântul „piață” de primul rând al zilei `d`.
    #[cfg(debug_assertions)]
    pub fn diary_test_link_entry(self: &Rc<Self>, d: NaiveDate) {
        let buf = self.diary.buf();
        let all = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
        let Some(byte) = all.find("piață") else { return };
        let a = all[..byte].chars().count() as i32;
        let id = self.journal.entry_anchor(PageRef::Day(d), 0, self.lang).ok().flatten().expect("semn");
        self.diary_link_to(Some((a, a + 5)), &Target::Entry(PageRef::Day(d), id), "piață");
    }
}
