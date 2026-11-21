// SPDX-License-Identifier: GPL-3.0-or-later
//! Prima pornire: o fereastră cu trei pași (limba, hârtia, criptarea), doar când nu există
//! încă nici jurnal, nici setări. La sfârșit scrie setările, creează jurnalul cu un mic
//! exemplu în ziua de azi și deschide caietul.

use crate::setari::{self, Paper, PaperColor, Settings};
use crate::tr;
use adw::prelude::*;
use gtk::glib;
use lanruj_core::crypt::{self, Key};
use lanruj_core::{Journal, Lang};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

/// E nevoie de prima pornire? Nu există setări, iar dosarul jurnalului lipsește sau e gol.
pub fn needed(root: &Path) -> bool {
    if setari::exists() {
        return false;
    }
    match std::fs::read_dir(root) {
        Err(_) => true,
        Ok(rd) => !rd.flatten().any(|e| !e.file_name().to_string_lossy().starts_with('.')),
    }
}

/// Ce se face după prima pornire (deschiderea caietului, cu cheia, dacă e cazul).
type Then = dyn Fn(&adw::Application, Option<Arc<Key>>);

struct Choice {
    lang: Lang,
    paper: Paper,
    color: PaperColor,
    encrypt: bool,
    password: String,
    /// Cheia și codul de recuperare, generate când se ajunge la pasul cheii.
    key: Option<(Arc<Key>, String)>,
}

struct Wizard {
    window: adw::ApplicationWindow,
    root: PathBuf,
    choice: RefCell<Choice>,
    step: Cell<u32>,
    /// Ultimele 4 caractere ale cheii de recuperare au fost scrise corect.
    key_ok: Cell<bool>,
    then: Box<Then>,
}

fn label(s: &str, classes: &[&str]) -> gtk::Label {
    let l = gtk::Label::builder().label(s).wrap(true).xalign(0.0).use_markup(true).build();
    for c in classes {
        l.add_css_class(c);
    }
    l
}

/// Fereastra primei porniri; `then` deschide caietul la sfârșit.
pub fn show(app: &adw::Application, root: PathBuf, then: impl Fn(&adw::Application, Option<Arc<Key>>) + 'static) {
    let window = adw::ApplicationWindow::builder().application(app).title("Lanruj").default_width(760).default_height(620).build();
    window.add_css_class("lanruj");
    let choice = Choice { lang: Lang::from_env(), paper: Paper::Dots, color: PaperColor::Warm, encrypt: false, password: String::new(), key: None };
    let w = Rc::new(Wizard { window: window.clone(), root, choice: RefCell::new(choice), step: Cell::new(0), key_ok: Cell::new(false), then: Box::new(then) });
    let style = adw::StyleManager::default();
    let set_night = {
        let win = window.clone();
        move |s: &adw::StyleManager| if s.is_dark() { win.add_css_class("noapte") } else { win.remove_css_class("noapte") }
    };
    set_night(&style);
    style.connect_dark_notify(set_night);
    w.apply_paper();
    w.render();
    window.present();

    // Doar la probe: LANRUJ_PRIMA_PAS arată direct un pas; LANRUJ_PRIMA_CRIPT=parolă alege criptarea;
    // LANRUJ_PRIMA_GATA termină singur (cu cheia de recuperare confirmată).
    #[cfg(debug_assertions)]
    {
        if let Ok(p) = std::env::var("LANRUJ_PRIMA_CRIPT") {
            let mut c = w.choice.borrow_mut();
            c.encrypt = true;
            c.password = p;
        }
        if let Some(n) = std::env::var("LANRUJ_PRIMA_PAS").ok().and_then(|n| n.parse::<u32>().ok()) {
            if n == 3 {
                w.make_key();
            }
            w.step.set(n);
            w.render();
        }
        if let Ok(p) = std::env::var("LANRUJ_PNG_PRIMA") {
            let win = window.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || {
                crate::save_png(&win, &p);
                if let Some(app) = win.application() {
                    app.quit();
                }
            });
        }
        if std::env::var_os("LANRUJ_PRIMA_GATA").is_some() {
            let w2 = w.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
                if w2.choice.borrow().encrypt {
                    w2.make_key();
                }
                w2.finish();
            });
        }
    }
}

impl Wizard {
    fn lang(&self) -> Lang {
        self.choice.borrow().lang
    }

    fn last_step(&self) -> u32 {
        if self.choice.borrow().encrypt { 3 } else { 2 }
    }

    /// Hârtia aleasă se vede pe loc, în mostra din pasul al doilea.
    fn apply_paper(&self) {
        for c in self.window.css_classes() {
            if c.starts_with("hartie-") || c.starts_with("culoare-") {
                self.window.remove_css_class(&c);
            }
        }
        let s = Settings { paper: self.choice.borrow().paper, color: self.choice.borrow().color, ..Settings::default() };
        for c in s.css_classes() {
            self.window.add_css_class(&c);
        }
    }

    fn make_key(&self) {
        let mut c = self.choice.borrow_mut();
        if c.key.is_none() {
            let (k, code) = crypt::generate();
            c.key = Some((Arc::new(k), code));
        }
    }

    /// Refac conținutul ferestrei pentru pasul de acum (și în limba aleasă).
    fn render(self: &Rc<Self>) {
        let lang = self.lang();
        let step = self.step.get();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 14);
        body.set_margin_top(18);
        body.set_margin_bottom(18);
        body.set_margin_start(32);
        body.set_margin_end(32);
        match step {
            0 => self.page_language(&body),
            1 => self.page_paper(&body),
            2 => self.page_encryption(&body),
            _ => self.page_key(&body),
        }
        let err = gtk::Label::builder().css_classes(["error"]).xalign(0.0).wrap(true).visible(false).build();
        body.append(&err);

        // Jos: pașii (puncte) și butoanele Înapoi / Mai departe.
        let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        bottom.set_margin_start(24);
        bottom.set_margin_end(24);
        bottom.set_margin_bottom(18);
        let back = gtk::Button::with_label(tr(lang, "Înapoi", "Back"));
        back.set_visible(step > 0);
        bottom.append(&back);
        let dots = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        dots.set_hexpand(true);
        dots.set_halign(gtk::Align::Center);
        for i in 0..=self.last_step() {
            let d = gtk::Box::builder().css_classes(["punct-pas"]).valign(gtk::Align::Center).build();
            if i == step {
                d.add_css_class("acum");
            }
            dots.append(&d);
        }
        bottom.append(&dots);
        let last = step >= self.last_step();
        let next = gtk::Button::builder()
            .label(if last { tr(lang, "Începe", "Start") } else { tr(lang, "Mai departe", "Next") })
            .css_classes(["suggested-action", "pill"])
            .build();
        bottom.append(&next);

        let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&body).build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&scroll);
        content.append(&bottom);
        let view = adw::ToolbarView::new();
        view.add_top_bar(&adw::HeaderBar::builder().show_title(false).build());
        let clamp = adw::Clamp::builder().maximum_size(680).child(&content).build();
        view.set_content(Some(&clamp));
        self.window.set_content(Some(&view));

        let w = self.clone();
        back.connect_clicked(move |_| {
            w.step.set(w.step.get().saturating_sub(1));
            w.render();
        });
        let w = self.clone();
        next.connect_clicked(move |_| {
            if let Some(m) = w.check(step) {
                err.set_label(&m);
                err.set_visible(true);
                return;
            }
            if step >= w.last_step() {
                w.finish();
            } else {
                if step + 1 == 3 {
                    w.make_key();
                }
                w.step.set(step + 1);
                w.render();
            }
        });
    }

    /// `Some(mesaj)` dacă pasul nu e complet.
    fn check(&self, step: u32) -> Option<String> {
        let lang = self.lang();
        let c = self.choice.borrow();
        match step {
            2 if c.encrypt && c.password.chars().count() < 8 => Some(
                tr(
                    lang,
                    "Parola trebuie să aibă cel puțin 8 caractere și să fie scrisă la fel în ambele câmpuri.",
                    "The password needs at least 8 characters and must be the same in both fields.",
                )
                .into(),
            ),
            3 if !self.key_ok.get() => Some(tr(lang, "Scrie ultimele 4 caractere ale cheii de recuperare.", "Type the last 4 characters of the recovery key.").into()),
            _ => None,
        }
    }

    fn page_language(self: &Rc<Self>, b: &gtk::Box) {
        let lang = self.lang();
        let icon = gtk::Image::from_icon_name(crate::APP_ID);
        icon.set_pixel_size(96);
        icon.set_halign(gtk::Align::Center);
        b.append(&icon);
        let t = label(tr(lang, "Bun venit în Lanruj", "Welcome to Lanruj"), &["title-1"]);
        t.set_xalign(0.5);
        b.append(&t);
        let s = label(
            tr(
                lang,
                "Un jurnal de tip „bullet journal”, pe Linux. Trei alegeri scurte și poți începe; toate se pot schimba oricând din Setări.",
                "A bullet journal app for Linux. Three short choices and you can start; all of them can be changed any time in Preferences.",
            ),
            &["dim-label"],
        );
        s.set_xalign(0.5);
        s.set_justify(gtk::Justification::Center);
        b.append(&s);
        b.append(&label(tr(lang, "Limba aplicației", "App language"), &["heading"]));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_homogeneous(true);
        let ro = gtk::ToggleButton::builder().label("Română").css_classes(["alegere-mare"]).active(lang == Lang::Ro).build();
        let en = gtk::ToggleButton::builder().label("English").css_classes(["alegere-mare"]).group(&ro).active(lang == Lang::En).build();
        row.append(&ro);
        row.append(&en);
        b.append(&row);
        for (btn, l) in [(ro, Lang::Ro), (en, Lang::En)] {
            let w = self.clone();
            btn.connect_toggled(move |x| {
                if x.is_active() && w.lang() != l {
                    w.choice.borrow_mut().lang = l;
                    // Textele se refac în limba aleasă, după ce se termină clicul.
                    let w2 = w.clone();
                    glib::idle_add_local_once(move || w2.render());
                }
            });
        }
    }

    fn page_paper(self: &Rc<Self>, b: &gtk::Box) {
        let lang = self.lang();
        b.append(&label(tr(lang, "Hârtia caietului", "Your paper"), &["title-2"]));
        b.append(&label(tr(lang, "Alege modelul și culoarea paginilor. Noaptea, hârtia se întunecă singură, după sistem.", "Choose the pattern and colour of the pages. At night the paper darkens by itself, following the system."), &["dim-label"]));
        // Mostra: o bucată de pagină cu câteva rânduri.
        let sample = gtk::Box::new(gtk::Orientation::Vertical, 4);
        sample.add_css_class("pagina");
        sample.add_css_class("singura");
        sample.add_css_class("mostra-hartie");
        let lines = match lang {
            Lang::Ro => ["•  Sună la bancă", "○  10:00 Dentist", "–  Prima zăpadă a anului", "★  Trimite raportul"],
            Lang::En => ["•  Call the bank", "○  10:00 Dentist", "–  First snow of the year", "★  Send the report"],
        };
        for l in lines {
            sample.append(&gtk::Label::builder().label(l).xalign(0.0).css_classes(["text-mostra"]).build());
        }
        b.append(&sample);

        b.append(&label(tr(lang, "Modelul", "Pattern"), &["heading"]));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_homogeneous(true);
        let mut first: Option<gtk::ToggleButton> = None;
        let cur = self.choice.borrow().paper;
        for (p, _, ro, en) in setari::PAPERS {
            let t = gtk::ToggleButton::builder().label(tr(lang, ro, en)).active(p == cur).build();
            if let Some(f) = &first {
                t.set_group(Some(f));
            } else {
                first = Some(t.clone());
            }
            let w = self.clone();
            t.connect_toggled(move |x| {
                if x.is_active() {
                    w.choice.borrow_mut().paper = p;
                    w.apply_paper();
                }
            });
            row.append(&t);
        }
        b.append(&row);

        b.append(&label(tr(lang, "Culoarea", "Colour"), &["heading"]));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_homogeneous(true);
        let mut first: Option<gtk::ToggleButton> = None;
        let cur = self.choice.borrow().color;
        for (c, key, ro, en) in setari::COLORS {
            let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            inner.set_halign(gtk::Align::Center);
            inner.append(&gtk::Box::builder().css_classes(["patrat-culoare", &format!("c-{key}")]).valign(gtk::Align::Center).build());
            inner.append(&gtk::Label::new(Some(tr(lang, ro, en))));
            let t = gtk::ToggleButton::builder().child(&inner).active(c == cur).build();
            if let Some(f) = &first {
                t.set_group(Some(f));
            } else {
                first = Some(t.clone());
            }
            let w = self.clone();
            t.connect_toggled(move |x| {
                if x.is_active() {
                    w.choice.borrow_mut().color = c;
                    w.apply_paper();
                }
            });
            row.append(&t);
        }
        b.append(&row);
    }

    fn page_encryption(self: &Rc<Self>, b: &gtk::Box) {
        let lang = self.lang();
        b.append(&label(tr(lang, "Criptarea jurnalului", "Encrypting the journal"), &["title-2"]));
        b.append(&label(
            tr(
                lang,
                "Jurnalul stă în fișiere Markdown obișnuite, pe calculatorul tău. Le poți cripta cu o parolă, ca nimeni altcineva să nu le poată citi.",
                "Your journal is kept as plain Markdown files on your computer. You can encrypt them with a password, so nobody else can read them.",
            ),
            &["dim-label"],
        ));
        let no = gtk::CheckButton::builder().label(tr(lang, "Nu, mulțumesc", "No, thanks")).active(!self.choice.borrow().encrypt).build();
        no.add_css_class("alegere-cripta");
        b.append(&no);
        b.append(&label(tr(lang, "Fișierele se pot citi și cu alte programe. Criptarea se poate porni oricând din Setări.", "The files can be read by other programs too. Encryption can be turned on any time in Preferences."), &["dim-label", "sub-alegere"]));
        let yes = gtk::CheckButton::builder().label(tr(lang, "Da, criptează jurnalul cu o parolă", "Yes, encrypt the journal with a password")).group(&no).active(self.choice.borrow().encrypt).build();
        yes.add_css_class("alegere-cripta");
        b.append(&yes);
        b.append(&label(
            tr(
                lang,
                "Lanruj îți cere parola la fiecare pornire și îți dă o cheie de recuperare, pentru cazul în care o uiți. Fără parolă și fără cheie, jurnalul nu mai poate fi citit de nimeni.",
                "Lanruj asks for the password at every start and gives you a recovery key in case you forget it. Without the password and the key, nobody can read the journal again.",
            ),
            &["dim-label", "sub-alegere"],
        ));
        let p1 = gtk::PasswordEntry::builder().placeholder_text(tr(lang, "Parola (cel puțin 8 caractere)", "Password (at least 8 characters)")).show_peek_icon(true).build();
        let p2 = gtk::PasswordEntry::builder().placeholder_text(tr(lang, "Încă o dată parola", "The password again")).show_peek_icon(true).build();
        let pw = self.choice.borrow().password.clone();
        p1.set_text(&pw);
        p2.set_text(&pw);
        for p in [&p1, &p2] {
            p.set_margin_start(28);
            yes.bind_property("active", p, "visible").sync_create().build();
            b.append(p);
        }
        let w = self.clone();
        yes.connect_toggled(move |x| {
            w.choice.borrow_mut().encrypt = x.is_active();
            let w2 = w.clone();
            glib::idle_add_local_once(move || w2.render());
        });
        // Parola se ține doar dacă e scrisă la fel de două ori.
        let save = {
            let (w, p1, p2) = (self.clone(), p1.clone(), p2.clone());
            move || {
                let (a, c) = (p1.text().to_string(), p2.text().to_string());
                w.choice.borrow_mut().password = if a == c { a } else { String::new() };
            }
        };
        let s1 = save.clone();
        p1.connect_changed(move |_| s1());
        p2.connect_changed(move |_| save());
    }

    fn page_key(self: &Rc<Self>, b: &gtk::Box) {
        let lang = self.lang();
        let code = self.choice.borrow().key.as_ref().map(|(_, c)| c.clone()).unwrap_or_default();
        b.append(&label(tr(lang, "Cheia de recuperare", "Recovery key"), &["title-2"]));
        b.append(&label(
            tr(
                lang,
                "Cu ea îți deschizi jurnalul dacă uiți parola. Se arată <b>o singură dată</b>: scrie-o pe hârtie sau copiaz-o și păstreaz-o în alt loc decât calculatorul.",
                "It opens your journal if you forget the password. It is shown <b>only once</b>: write it on paper or copy it and keep it somewhere other than the computer.",
            ),
            &[],
        ));
        let shown = gtk::Label::builder().label(&code).selectable(true).css_classes(["title-3", "monospace"]).build();
        shown.set_margin_top(6);
        shown.set_margin_bottom(6);
        b.append(&shown);
        let copy = gtk::Button::builder().label(tr(lang, "Copiază", "Copy")).halign(gtk::Align::Center).build();
        let (c, win) = (code.clone(), self.window.clone());
        copy.connect_clicked(move |_| win.clipboard().set_text(&c));
        b.append(&copy);
        b.append(&label(tr(lang, "Ca să știm că ai păstrat-o, scrie <b>ultimele 4 caractere</b> ale cheii, apoi apasă Începe.", "To be sure you kept it, type the <b>last 4 characters</b> of the key, then press Start."), &[]));
        let last = gtk::Entry::builder().max_length(4).width_chars(6).halign(gtk::Align::Start).css_classes(["monospace"]).build();
        b.append(&last);
        let tail: String = crypt::clean_code(&code).chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        let w = self.clone();
        last.connect_changed(move |e| w.key_ok.set(crypt::clean_code(&e.text()) == tail));
        self.key_ok.set(false);
    }

    /// Scrie setările, creează jurnalul (cu exemplul din prima zi) și deschide caietul.
    fn finish(self: &Rc<Self>) {
        let lang = self.lang();
        let c = self.choice.borrow();
        let mut s = Settings::load();
        s.lang = Some(c.lang);
        s.paper = c.paper;
        s.color = c.color;
        if let Err(e) = s.save() {
            eprintln!("lanruj: setările: {e}");
        }
        let j = Journal::new(&self.root);
        let mut key = None;
        if c.encrypt
            && let Some((k, code)) = &c.key
        {
            if let Err(e) = crypt::store(&self.root, k, &c.password, code) {
                eprintln!("lanruj: cheia: {e}");
            } else {
                j.set_key(Some(k.clone()));
                key = Some(k.clone());
            }
        }
        let today = chrono::Local::now().date_naive();
        let example = match lang {
            Lang::Ro => [
                "- [ ] Bine ai venit! Aceasta e o sarcină: apasă pe punctul din stânga ca s-o bifezi",
                "- o 10:00 Acesta e un eveniment, cu oră",
                "- Aceasta e o notă: un gând, o întâmplare, ceva de ținut minte",
                "- [ ] * O sarcină importantă, cu steluță",
                "- ! O idee, cu semnul exclamării",
                "- Scrie în câmpul de jos „. ” pentru sarcină, „o ” pentru eveniment și „- ” pentru notă. Exemplele acestea le poți șterge oricând.",
            ],
            Lang::En => [
                "- [ ] Welcome! This is a task: click the dot on the left to tick it",
                "- o 10:00 This is an event, with a time",
                "- This is a note: a thought, something that happened, something to remember",
                "- [ ] * An important task, with a star",
                "- ! An idea, with an exclamation mark",
                "- In the field below, type \". \" for a task, \"o \" for an event and \"- \" for a note. You can delete these examples any time.",
            ],
        };
        let text = format!("# {}\n\n{}\n", lang.day_title(today), example.join("\n"));
        if let Err(e) = j.write_text(&j.day_path(today), &text) {
            eprintln!("lanruj: exemplul: {e}");
        }
        drop(c);
        if let Some(app) = self.window.application().and_downcast::<adw::Application>() {
            (self.then)(&app, key);
        }
        self.window.destroy();
    }
}
