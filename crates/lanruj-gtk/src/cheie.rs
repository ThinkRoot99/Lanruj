// SPDX-License-Identifier: GPL-3.0-or-later
//! Criptarea, partea care se vede: fereastra cu parola la pornire (și „Am uitat parola”),
//! pornirea criptării (cu cheia de recuperare), schimbarea parolei și oprirea ei.

use crate::{App, tr};
use adw::prelude::*;
use gtk::{gio, glib};
use lanruj_core::crypt::{self, Key, Unlock};
use lanruj_core::{Journal, Lang};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// Cea mai scurtă parolă primită.
const MIN_LEN: usize = 8;

fn page_box() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
    b.set_margin_top(12);
    b.set_margin_bottom(24);
    b.set_margin_start(24);
    b.set_margin_end(24);
    b
}

fn text(label: &str, classes: &[&str]) -> gtk::Label {
    let l = gtk::Label::builder().label(label).wrap(true).xalign(0.0).use_markup(true).build();
    for c in classes {
        l.add_css_class(c);
    }
    l
}

fn password(placeholder: &str) -> gtk::PasswordEntry {
    gtk::PasswordEntry::builder().placeholder_text(placeholder).show_peek_icon(true).build()
}

fn error_label() -> gtk::Label {
    let l = text("", &["error"]);
    l.set_visible(false);
    l
}

fn show_error(l: &gtk::Label, msg: &str) {
    l.set_label(msg);
    l.set_visible(true);
}

/// Verifică parola nouă (și repetarea ei); `Some(mesaj)` = nu e bună.
fn check_new(lang: Lang, a: &str, b: &str) -> Option<String> {
    if a.chars().count() < MIN_LEN {
        return Some(match lang {
            Lang::Ro => format!("Parola trebuie să aibă cel puțin {MIN_LEN} caractere."),
            Lang::En => format!("The password needs at least {MIN_LEN} characters."),
        });
    }
    (a != b).then(|| tr(lang, "Cele două parole nu sunt la fel.", "The two passwords don't match.").to_string())
}

/// Fereastra de la pornire, când jurnalul e criptat: parola sau cheia de recuperare.
/// După deschidere cheamă `then` cu cheia și se închide.
pub fn unlock_window(app: &adw::Application, root: PathBuf, lang: Lang, then: impl Fn(&adw::Application, Arc<Key>) + 'static) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder().application(app).title("Lanruj").default_width(460).default_height(420).build();
    let stack = gtk::Stack::builder().transition_type(gtk::StackTransitionType::SlideLeftRight).vhomogeneous(false).build();

    // Parola.
    let b = page_box();
    let icon = gtk::Image::from_icon_name(crate::APP_ID);
    icon.set_pixel_size(72);
    b.append(&icon);
    let title = text(tr(lang, "Jurnalul e blocat", "The journal is locked"), &["title-2"]);
    title.set_xalign(0.5);
    b.append(&title);
    let hint = text(tr(lang, "Scrie parola ca să-l deschizi.", "Type the password to open it."), &["dim-label"]);
    hint.set_xalign(0.5);
    b.append(&hint);
    let pass = password(tr(lang, "Parola", "Password"));
    b.append(&pass);
    let err = error_label();
    b.append(&err);
    let open = gtk::Button::builder().label(tr(lang, "Deschide", "Open")).css_classes(["suggested-action", "pill"]).halign(gtk::Align::Center).build();
    b.append(&open);
    let forgot = gtk::Button::builder().label(tr(lang, "Am uitat parola", "I forgot the password")).css_classes(["flat"]).halign(gtk::Align::Center).build();
    b.append(&forgot);
    stack.add_named(&b, Some("parola"));

    // Cheia de recuperare și o parolă nouă.
    let r = page_box();
    r.append(&text(tr(lang, "Cu cheia de recuperare", "With the recovery key"), &["title-3"]));
    r.append(&text(
        tr(
            lang,
            "Scrie cheia de recuperare (6 grupuri de câte 5 caractere; nu contează dacă literele sunt mari sau mici și nici liniuțele) și alege o parolă nouă.",
            "Type the recovery key (6 groups of 5 characters; capitals and dashes don't matter) and choose a new password.",
        ),
        &["dim-label"],
    ));
    let code = gtk::Entry::builder().placeholder_text("ABCDE-FGHJK-…").css_classes(["monospace"]).build();
    r.append(&code);
    let new1 = password(tr(lang, "Parola nouă", "New password"));
    let new2 = password(tr(lang, "Încă o dată parola nouă", "The new password again"));
    r.append(&new1);
    r.append(&new2);
    let err2 = error_label();
    r.append(&err2);
    let rec = gtk::Button::builder()
        .label(tr(lang, "Deschide și pune parola nouă", "Open and set the new password"))
        .css_classes(["suggested-action", "pill"])
        .halign(gtk::Align::Center)
        .build();
    r.append(&rec);
    let back = gtk::Button::builder().label(tr(lang, "Înapoi la parolă", "Back to the password")).css_classes(["flat"]).halign(gtk::Align::Center).build();
    r.append(&back);
    stack.add_named(&r, Some("recuperare"));

    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::builder().show_title(false).build());
    let clamp = adw::Clamp::builder().maximum_size(400).child(&stack).valign(gtk::Align::Center).build();
    view.set_content(Some(&clamp));
    window.set_content(Some(&view));

    let (s, p) = (stack.clone(), pass.clone());
    back.connect_clicked(move |_| {
        s.set_visible_child_name("parola");
        p.grab_focus();
    });
    let (s, c) = (stack.clone(), code.clone());
    forgot.connect_clicked(move |_| {
        s.set_visible_child_name("recuperare");
        c.grab_focus();
    });

    let then = Rc::new(then);
    // Deschiderea durează o clipă (scrypt), deci se face pe alt fir.
    let try_open = {
        let (window, pass, err, open, root, then) = (window.clone(), pass.clone(), err.clone(), open.clone(), root.clone(), then.clone());
        move || {
            let secret = pass.text().to_string();
            if secret.is_empty() {
                return;
            }
            open.set_sensitive(false);
            open.set_label(tr(lang, "Se deschide…", "Opening…"));
            let (window, pass, err, open, root, then) = (window.clone(), pass.clone(), err.clone(), open.clone(), root.clone(), then.clone());
            glib::spawn_future_local(async move {
                let r0 = root.clone();
                let res = gio::spawn_blocking(move || crypt::unlock(&r0, &secret)).await;
                open.set_sensitive(true);
                open.set_label(tr(lang, "Deschide", "Open"));
                match res {
                    Ok(Ok(key)) => {
                        // Ținută minte: parola (poate schimbată în altă parte) intră din nou în portofel.
                        if crate::setari::Settings::load().remember {
                            let (r, p) = (root.clone(), pass.text().to_string());
                            glib::spawn_future_local(async move {
                                let _ = crate::incuiere::keyring_set(r, p).await;
                            });
                        }
                        if let Some(app) = window.application().and_downcast::<adw::Application>() {
                            then(&app, Arc::new(key));
                        }
                        window.destroy();
                    }
                    Ok(Err(Unlock::Wrong)) => {
                        show_error(&err, tr(lang, "Parola nu e corectă.", "Wrong password."));
                        pass.select_region(0, -1);
                        pass.grab_focus();
                    }
                    Ok(Err(Unlock::Io(e))) => show_error(&err, &e.to_string()),
                    Err(_) => show_error(&err, "?"),
                }
            });
        }
    };
    let f = try_open.clone();
    open.connect_clicked(move |_| f());
    let f = try_open.clone();
    pass.connect_activate(move |_| f());

    let try_recover = {
        let (window, err2, rec, root, then) = (window.clone(), err2.clone(), rec.clone(), root.clone(), then.clone());
        let (code, new1, new2) = (code.clone(), new1.clone(), new2.clone());
        move || {
            if let Some(m) = check_new(lang, &new1.text(), &new2.text()) {
                show_error(&err2, &m);
                return;
            }
            let (c, newp) = (code.text().to_string(), new1.text().to_string());
            rec.set_sensitive(false);
            let (window, err2, rec, root, then) = (window.clone(), err2.clone(), rec.clone(), root.clone(), then.clone());
            glib::spawn_future_local(async move {
                let res = gio::spawn_blocking(move || {
                    let key = crypt::recover(&root, &c)?;
                    crypt::set_password(&root, &key, &newp).map_err(Unlock::Io)?;
                    Ok::<Key, Unlock>(key)
                })
                .await;
                rec.set_sensitive(true);
                match res {
                    Ok(Ok(key)) => {
                        if let Some(app) = window.application().and_downcast::<adw::Application>() {
                            then(&app, Arc::new(key));
                        }
                        window.destroy();
                    }
                    Ok(Err(Unlock::Wrong)) => show_error(&err2, tr(lang, "Cheia de recuperare nu e corectă.", "Wrong recovery key.")),
                    Ok(Err(Unlock::Io(e))) => show_error(&err2, &e.to_string()),
                    Err(_) => show_error(&err2, "?"),
                }
            });
        }
    };
    let f = try_recover.clone();
    rec.connect_clicked(move |_| f());
    let f = try_recover.clone();
    new2.connect_activate(move |_| f());

    window.present();
    pass.grab_focus();

    // Doar la probe: captura ferestrei cu parola (LANRUJ_PNG_PAROLA), apoi ieșire.
    #[cfg(debug_assertions)]
    if let Ok(p) = std::env::var("LANRUJ_PNG_PAROLA") {
        let w = window.clone();
        let wait = std::env::var("LANRUJ_ASTEAPTA").ok().and_then(|v| v.parse().ok()).unwrap_or(900);
        glib::timeout_add_local_once(std::time::Duration::from_millis(wait), move || {
            crate::save_png(&w, &p);
            if let Some(app) = w.application() {
                app.quit();
            }
        });
    }

    // Doar la probe: cheia de recuperare din LANRUJ_CHEIE și parola nouă LANRUJ_PAROLA_NOUA.
    #[cfg(debug_assertions)]
    if let (Ok(c), Ok(p)) = (std::env::var("LANRUJ_CHEIE"), std::env::var("LANRUJ_PAROLA_NOUA")) {
        stack.set_visible_child_name("recuperare");
        code.set_text(&c);
        new1.set_text(&p);
        new2.set_text(&p);
        glib::timeout_add_local_once(std::time::Duration::from_millis(600), try_recover);
    }
    // Doar la probe: parola din LANRUJ_PAROLA, scrisă și trimisă singură.
    #[cfg(debug_assertions)]
    if let Ok(p) = std::env::var("LANRUJ_PAROLA") {
        pass.set_text(&p);
        glib::timeout_add_local_once(std::time::Duration::from_millis(600), try_open);
    }
    window
}

/// Tipărește un text simplu (cheia de recuperare), prin fereastra de tipărire.
fn print_text(parent: &impl IsA<gtk::Window>, title: &str, body: &str) {
    let op = gtk::PrintOperation::new();
    op.set_job_name(title);
    op.set_n_pages(1);
    let (title, body) = (title.to_string(), body.to_string());
    op.connect_draw_page(move |_, ctx, _| {
        let cr = ctx.cairo_context();
        let layout = ctx.create_pango_layout();
        layout.set_width((ctx.width() * gtk::pango::SCALE as f64) as i32);
        layout.set_markup(&format!(
            "<span size=\"x-large\" weight=\"bold\">{}</span>\n\n{}",
            glib::markup_escape_text(&title),
            body
        ));
        cr.move_to(0.0, 0.0);
        pangocairo::functions::show_layout(&cr, &layout);
    });
    let _ = op.run(gtk::PrintOperationAction::PrintDialog, Some(parent.as_ref()));
}

/// Un dialog cu titlu și o pagină de conținut.
fn dialog(title: &str, content: &gtk::Box) -> adw::Dialog {
    let d = adw::Dialog::builder().title(title).content_width(520).build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(content));
    d.set_child(Some(&view));
    d
}

impl App {
    /// Setări → Criptarea → Pornește: parola, cheia de recuperare, apoi criptarea fișierelor.
    pub fn encrypt_dialog(self: &Rc<Self>, after: impl Fn() + 'static) {
        let lang = self.lang;
        let b = page_box();
        b.append(&text(
            tr(
                lang,
                "Cu criptarea pornită, fișierele jurnalului (paginile și pozele) se pot citi <b>doar cu Lanruj și cu parola</b>: \
                 nu și cu Obsidian, Markor sau alt editor. Căutarea poate fi puțin mai lentă. \
                 Fără parolă și fără cheia de recuperare, jurnalul nu mai poate fi citit de nimeni, nici de tine.",
                "With encryption on, the journal files (pages and pictures) can be read <b>only with Lanruj and the password</b>: \
                 not with Obsidian, Markor or another editor. Search may be a bit slower. \
                 Without the password and the recovery key, nobody can read the journal again, not even you.",
            ),
            &[],
        ));
        let p1 = password(tr(lang, "Parola", "Password"));
        let p2 = password(tr(lang, "Încă o dată parola", "The password again"));
        b.append(&p1);
        b.append(&p2);
        let err = error_label();
        b.append(&err);
        let next = gtk::Button::builder().label(tr(lang, "Continuă", "Continue")).css_classes(["suggested-action", "pill"]).halign(gtk::Align::End).build();
        b.append(&next);
        let d = dialog(tr(lang, "Criptează jurnalul", "Encrypt the journal"), &b);
        let after = Rc::new(after);
        let (weak, dl) = (Rc::downgrade(self), d.clone());
        let go = move || {
            let Some(app) = weak.upgrade() else { return };
            if let Some(m) = check_new(lang, &p1.text(), &p2.text()) {
                show_error(&err, &m);
                return;
            }
            let pass = p1.text().to_string();
            dl.close();
            app.recovery_dialog(pass, after.clone());
        };
        let go = Rc::new(go);
        let g = go.clone();
        next.connect_clicked(move |_| g());
        p2_activate(&b, go);
        d.present(Some(&self.window));
    }

    /// Cheia de recuperare: arătată o singură dată; mergi mai departe după ultimele 4 caractere.
    fn recovery_dialog(self: &Rc<Self>, pass: String, after: Rc<dyn Fn()>) {
        let lang = self.lang;
        let (key, code) = crypt::generate();
        let b = page_box();
        b.append(&text(
            tr(
                lang,
                "Aceasta e <b>cheia de recuperare</b>. Cu ea deschizi jurnalul dacă uiți parola. \
                 Se arată <b>o singură dată</b>: tipărește-o sau scrie-o pe hârtie și păstreaz-o în alt loc decât calculatorul.",
                "This is the <b>recovery key</b>. It opens the journal if you forget the password. \
                 It is shown <b>only once</b>: print it or write it on paper and keep it somewhere other than the computer.",
            ),
            &[],
        ));
        let shown = gtk::Label::builder().label(&code).selectable(true).wrap(true).css_classes(["title-3", "monospace"]).build();
        shown.set_margin_top(8);
        shown.set_margin_bottom(8);
        b.append(&shown);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_halign(gtk::Align::Center);
        let copy = gtk::Button::with_label(tr(lang, "Copiază", "Copy"));
        let print = gtk::Button::with_label(tr(lang, "Tipărește", "Print"));
        row.append(&copy);
        row.append(&print);
        b.append(&row);
        b.append(&text(tr(lang, "Ca să știm că ai păstrat-o, scrie <b>ultimele 4 caractere</b> ale cheii:", "To be sure you kept it, type the <b>last 4 characters</b> of the key:"), &[]));
        let last = gtk::Entry::builder().max_length(4).width_chars(6).halign(gtk::Align::Start).css_classes(["monospace"]).build();
        b.append(&last);
        let go = gtk::Button::builder().label(tr(lang, "Criptează jurnalul", "Encrypt the journal")).css_classes(["suggested-action", "pill"]).halign(gtk::Align::End).sensitive(false).build();
        b.append(&go);
        let d = dialog(tr(lang, "Cheia de recuperare", "Recovery key"), &b);
        d.set_can_close(true);

        let (c, w) = (code.clone(), self.window.clone());
        copy.connect_clicked(move |_| w.clipboard().set_text(&c));
        let (c, w) = (code.clone(), self.window.clone());
        print.connect_clicked(move |_| {
            let body = match lang {
                Lang::Ro => format!(
                    "<span font_family=\"monospace\" size=\"xx-large\">{c}</span>\n\nCheia de recuperare a jurnalului Lanruj. Cu ea deschizi jurnalul dacă uiți parola: la pornire, „Am uitat parola”. Păstreaz-o într-un loc sigur."
                ),
                Lang::En => format!(
                    "<span font_family=\"monospace\" size=\"xx-large\">{c}</span>\n\nThe recovery key of the Lanruj journal. It opens the journal if you forget the password: at startup, \"I forgot the password\". Keep it somewhere safe."
                ),
            };
            print_text(&w, "Lanruj", &body);
        });
        let tail: String = crypt::clean_code(&code).chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        let g = go.clone();
        last.connect_changed(move |e| g.set_sensitive(crypt::clean_code(&e.text()) == tail));

        let key = Rc::new(std::cell::RefCell::new(Some(key)));
        let (weak, dl) = (Rc::downgrade(self), d.clone());
        go.connect_clicked(move |btn| {
            let (Some(app), Some(key)) = (weak.upgrade(), key.borrow_mut().take()) else { return };
            btn.set_sensitive(false);
            dl.set_can_close(false);
            btn.set_label(tr(lang, "Se criptează… Nu închide aplicația.", "Encrypting… Don't close the app."));
            let (pass, code) = (pass.clone(), code.clone());
            let key = Arc::new(key);
            let root = app.journal.root().to_path_buf();
            let backup = app.settings.borrow().backup_folder();
            let today = app.today.get();
            let (dl, after, k2) = (dl.clone(), after.clone(), key.clone());
            glib::spawn_future_local(async move {
                let res = gio::spawn_blocking(move || {
                    // Întâi o copie a jurnalului așa cum e, apoi cheia, apoi fișierele.
                    crate::copii::make(&root, &backup, today, 1000)?;
                    crypt::store(&root, &k2, &pass, &code)?;
                    let j = Journal::new(&root);
                    j.set_key(Some(k2));
                    j.convert(true, |_, _| {})
                })
                .await;
                dl.set_can_close(true);
                dl.close();
                match res {
                    Ok(Ok(n)) => {
                        app.journal.set_key(Some(key.clone()));
                        let msg = match app.lang {
                            Lang::Ro => format!("Jurnalul e criptat ({n} fișiere). Copiile vechi, necriptate, au rămas în dosarul copiilor."),
                            Lang::En => format!("The journal is encrypted ({n} files). Older unencrypted backups stay in the backup folder."),
                        };
                        app.toast(&msg);
                    }
                    Ok(Err(e)) => app.error(&e),
                    Err(_) => app.error(&std::io::Error::other("criptarea s-a oprit")),
                }
                // Ce s-a apucat de criptat se citește oricum (cu cheia de acum).
                if app.journal.key().is_none() && crypt::enabled(app.journal.root()) {
                    app.journal.set_key(Some(key));
                }
                app.refresh();
                after();
            });
        });
        d.present(Some(&self.window));
    }

    /// Cere parola jurnalului; `then(app, parola)` dacă e bună, `cancel()` dacă fereastra se închide fără ea.
    pub fn ask_password_then(self: &Rc<Self>, title: &str, intro: &str, button: &str, then: impl Fn(&Rc<App>, String) + 'static, cancel: impl Fn() + 'static) {
        let done = Rc::new(std::cell::Cell::new(false));
        let d2 = done.clone();
        let d = self.ask_password(title, intro, None, button, move |app, secret, _| {
            d2.set(true);
            then(app, secret);
        });
        d.connect_closed(move |_| {
            if !done.get() {
                cancel();
            }
        });
    }

    /// Cere parola de acum, apoi face `then` (pentru schimbarea parolei și oprirea criptării).
    fn ask_password(self: &Rc<Self>, title: &str, intro: &str, extra: Option<(gtk::PasswordEntry, gtk::PasswordEntry)>, button: &str, then: impl Fn(&Rc<App>, String, Option<String>) + 'static) -> adw::Dialog {
        let lang = self.lang;
        let b = page_box();
        b.append(&text(intro, &[]));
        let cur = password(tr(lang, "Parola de acum", "Current password"));
        b.append(&cur);
        if let Some((a, c)) = &extra {
            b.append(a);
            b.append(c);
        }
        let err = error_label();
        b.append(&err);
        let go = gtk::Button::builder().label(button).css_classes(["suggested-action", "pill"]).halign(gtk::Align::End).build();
        b.append(&go);
        let d = dialog(title, &b);
        let (weak, dl) = (Rc::downgrade(self), d.clone());
        let then = Rc::new(then);
        go.connect_clicked(move |btn| {
            let Some(app) = weak.upgrade() else { return };
            let newp = match &extra {
                Some((a, c)) => match check_new(lang, &a.text(), &c.text()) {
                    Some(m) => {
                        show_error(&err, &m);
                        return;
                    }
                    None => Some(a.text().to_string()),
                },
                None => None,
            };
            let secret = cur.text().to_string();
            let root = app.journal.root().to_path_buf();
            btn.set_sensitive(false);
            let (btn, err, dl, then, newp) = (btn.clone(), err.clone(), dl.clone(), then.clone(), newp.clone());
            glib::spawn_future_local(async move {
                let s2 = secret.clone();
                let ok = gio::spawn_blocking(move || crypt::unlock(&root, &s2).is_ok()).await.unwrap_or(false);
                btn.set_sensitive(true);
                if !ok {
                    show_error(&err, tr(lang, "Parola nu e corectă.", "Wrong password."));
                    return;
                }
                then(&app, secret, newp);
                dl.close();
            });
        });
        d.present(Some(&self.window));
        d
    }

    /// Setări → Criptarea → Schimbă parola.
    pub fn change_password_dialog(self: &Rc<Self>) {
        let lang = self.lang;
        let extra = (password(tr(lang, "Parola nouă", "New password")), password(tr(lang, "Încă o dată parola nouă", "The new password again")));
        self.ask_password(
            tr(lang, "Schimbă parola", "Change the password"),
            tr(lang, "Cheia de recuperare rămâne aceeași; fișierele nu se schimbă.", "The recovery key stays the same; the files don't change."),
            Some(extra),
            tr(lang, "Schimbă parola", "Change the password"),
            |app, _, newp| {
                let (Some(key), Some(newp)) = (app.journal.key(), newp) else { return };
                let root = app.journal.root().to_path_buf();
                let weak = Rc::downgrade(app);
                let newp2 = newp.clone();
                glib::spawn_future_local(async move {
                    let res = gio::spawn_blocking(move || crypt::set_password(&root, &key, &newp)).await;
                    let Some(app) = weak.upgrade() else { return };
                    match res {
                        Ok(Ok(())) => {
                            app.remember_password(newp2);
                            app.toast(tr(app.lang, "Parola e schimbată.", "The password is changed."));
                        }
                        Ok(Err(e)) => app.error(&e),
                        Err(_) => {}
                    }
                });
            },
        );
    }

    /// Setări → Criptarea → Oprește: fișierele redevin text obișnuit.
    pub fn decrypt_dialog(self: &Rc<Self>, after: impl Fn() + 'static) {
        let lang = self.lang;
        let after = Rc::new(after);
        self.ask_password(
            tr(lang, "Oprește criptarea", "Turn off encryption"),
            tr(
                lang,
                "Fișierele jurnalului vor fi din nou text obișnuit, pe care îl poate citi oricine are acces la calculator. Întâi se face o copie (criptată) a jurnalului.",
                "The journal files will be plain text again, readable by anyone with access to the computer. An encrypted backup is made first.",
            ),
            None,
            tr(lang, "Oprește criptarea", "Turn off encryption"),
            move |app, _, _| {
                let Some(key) = app.journal.key() else { return };
                let root = app.journal.root().to_path_buf();
                let backup = app.settings.borrow().backup_folder();
                let today = app.today.get();
                let (weak, after) = (Rc::downgrade(app), after.clone());
                app.toast(tr(lang, "Se decriptează jurnalul…", "Decrypting the journal…"));
                glib::spawn_future_local(async move {
                    let res = gio::spawn_blocking(move || {
                        crate::copii::make(&root, &backup, today, 1000)?;
                        let j = Journal::new(&root);
                        j.set_key(Some(key));
                        let n = j.convert(false, |_, _| {})?;
                        crypt::remove(&root)?;
                        Ok::<usize, std::io::Error>(n)
                    })
                    .await;
                    let Some(app) = weak.upgrade() else { return };
                    match res {
                        Ok(Ok(n)) => {
                            app.journal.set_key(None);
                            app.settings.borrow_mut().remember = false;
                            app.apply_settings();
                            glib::spawn_future_local(crate::incuiere::keyring_clear(app.journal.root().to_path_buf()));
                            let msg = match app.lang {
                                Lang::Ro => format!("Criptarea e oprită ({n} fișiere decriptate)."),
                                Lang::En => format!("Encryption is off ({n} files decrypted)."),
                            };
                            app.toast(&msg);
                        }
                        Ok(Err(e)) => app.error(&e),
                        Err(_) => {}
                    }
                    app.refresh();
                    after();
                });
            },
        );
    }
}

/// Enter în ultimul câmp de parolă trimite formularul.
fn p2_activate(b: &gtk::Box, go: Rc<impl Fn() + 'static>) {
    let mut last: Option<gtk::PasswordEntry> = None;
    let mut c = b.first_child();
    while let Some(w) = c {
        if let Some(p) = w.downcast_ref::<gtk::PasswordEntry>() {
            last = Some(p.clone());
        }
        c = w.next_sibling();
    }
    if let Some(p) = last {
        p.connect_activate(move |_| go());
    }
}
