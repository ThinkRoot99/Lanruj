// SPDX-License-Identifier: GPL-3.0-or-later
//! Încuierea: parola sau PIN-ul de deschidere, încuierea la blocarea ecranului,
//! parola jurnalului criptat în portofelul de chei și pornirea odată cu calculatorul.

use crate::setari::{self, Lock};
use crate::{App, tr};
use adw::prelude::*;
use gtk::{gio, glib};
use lanruj_core::{Lang, crypt};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

thread_local! {
    /// Fereastra de încuiere deschisă acum (cât timp Lanruj e încuiat).
    static LOCK_WINDOW: RefCell<Option<adw::ApplicationWindow>> = const { RefCell::new(None) };
}

/// Lanruj e încuiat acum? Atunci o nouă pornire aduce în față fereastra de încuiere.
pub fn present_lock() -> bool {
    LOCK_WINDOW.with(|w| match w.borrow().as_ref() {
        Some(win) => {
            win.present();
            true
        }
        None => false,
    })
}

// ---- Încuietoarea (parola sau PIN-ul de deschidere) ----

/// Pune o încuietoare nouă (sau o scoate, cu `None`).
pub fn set_lock(secret: Option<&str>) -> std::io::Result<()> {
    let p = setari::lock_file();
    match secret {
        Some(s) => {
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d)?;
            }
            std::fs::write(&p, crypt::seal(s, b"lanruj")?)
        }
        None => match std::fs::remove_file(&p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
}

/// E bună parola sau PIN-ul? (durează o clipă: scrypt)
pub fn check_lock(secret: &str) -> bool {
    std::fs::read(setari::lock_file()).ok().and_then(|d| crypt::unseal(secret, &d)).is_some_and(|d| d == b"lanruj")
}

// ---- Portofelul de chei ----

fn attributes(root: &Path) -> std::collections::HashMap<&'static str, String> {
    [("application", crate::APP_ID.to_string()), ("jurnal", root.display().to_string())].into_iter().collect()
}

/// Parola jurnalului din portofelul de chei (îl deblochează, dacă trebuie: fereastra GNOME).
pub async fn keyring_get(root: PathBuf) -> Option<String> {
    let k = oo7::Keyring::new().await.ok()?;
    let _ = k.unlock().await;
    let items = k.search_items(&attributes(&root)).await.ok()?;
    let item = items.first()?;
    let _ = item.unlock().await;
    let secret = item.secret().await.ok()?;
    String::from_utf8(secret.as_bytes().to_vec()).ok()
}

pub async fn keyring_set(root: PathBuf, password: String) -> Result<(), String> {
    let k = oo7::Keyring::new().await.map_err(|e| e.to_string())?;
    let _ = k.unlock().await;
    k.create_item("Lanruj", &attributes(&root), oo7::Secret::text(password), true).await.map_err(|e| e.to_string())
}

pub async fn keyring_clear(root: PathBuf) {
    if let Ok(k) = oo7::Keyring::new().await {
        let _ = k.delete(&attributes(&root)).await;
    }
}

// ---- Pornirea odată cu calculatorul ----

fn autostart_file() -> PathBuf {
    // La probe (LANRUJ_DIR), pornirea automată rămâne în dosarul de probă, nu în sistem.
    if let Some(d) = std::env::var_os("LANRUJ_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join(".pornire-automata.desktop");
    }
    glib::user_config_dir().join("autostart").join(format!("{}.desktop", crate::APP_ID))
}

pub fn autostart() -> bool {
    autostart_file().is_file()
}

pub fn set_autostart(on: bool) -> std::io::Result<()> {
    let p = autostart_file();
    if !on {
        return match std::fs::remove_file(&p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let exe = std::env::current_exe()?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(
        &p,
        format!(
            "[Desktop Entry]\nType=Application\nName=Lanruj\nExec=\"{}\"\nIcon={}\nX-GNOME-Autostart-enabled=true\n",
            exe.display(),
            crate::APP_ID
        ),
    )
}

// ---- Fereastra de încuiere ----

/// Ce verifică fereastra: încuietoarea (parolă sau PIN) sau parola jurnalului criptat.
#[derive(Clone)]
pub enum Check {
    Lock(Lock),
    Journal(PathBuf),
}

impl Check {
    fn pin(&self) -> bool {
        matches!(self, Check::Lock(Lock::Pin))
    }
    fn ok(&self, secret: &str) -> bool {
        match self {
            Check::Lock(_) => check_lock(secret),
            Check::Journal(root) => crypt::unlock(root, secret).is_ok(),
        }
    }
}

/// Fereastra „Lanruj e încuiat”; după secretul bun cheamă `then` și se închide.
pub fn lock_window(app: &adw::Application, lang: Lang, check: Check, then: impl Fn(&adw::Application) + 'static) {
    let window = adw::ApplicationWindow::builder().application(app).title("Lanruj").default_width(420).default_height(360).build();
    let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
    b.set_margin_top(12);
    b.set_margin_bottom(24);
    b.set_margin_start(24);
    b.set_margin_end(24);
    let icon = gtk::Image::from_icon_name(crate::APP_ID);
    icon.set_pixel_size(72);
    b.append(&icon);
    b.append(&gtk::Label::builder().label(tr(lang, "Lanruj e încuiat", "Lanruj is locked")).css_classes(["title-2"]).build());
    let hint = match &check {
        Check::Lock(Lock::Pin) => tr(lang, "Scrie PIN-ul.", "Type the PIN."),
        Check::Lock(_) => tr(lang, "Scrie parola de deschidere.", "Type the opening password."),
        Check::Journal(_) => tr(lang, "Scrie parola jurnalului.", "Type the journal password."),
    };
    b.append(&gtk::Label::builder().label(hint).css_classes(["dim-label"]).build());
    let entry = gtk::PasswordEntry::builder().show_peek_icon(!check.pin()).build();
    if check.pin() {
        entry.set_halign(gtk::Align::Center);
        entry.set_width_chars(6);
        entry.add_css_class("title-2");
    }
    b.append(&entry);
    let err = gtk::Label::builder().css_classes(["error"]).visible(false).build();
    b.append(&err);
    let open = gtk::Button::builder().label(tr(lang, "Deschide", "Open")).css_classes(["suggested-action", "pill"]).halign(gtk::Align::Center).build();
    b.append(&open);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::builder().show_title(false).build());
    let clamp = adw::Clamp::builder().maximum_size(380).child(&b).valign(gtk::Align::Center).build();
    view.set_content(Some(&clamp));
    window.set_content(Some(&view));

    let then = Rc::new(then);
    let busy = Rc::new(std::cell::Cell::new(false));
    let try_open = {
        let (window, entry, err, open, check, then, busy) = (window.clone(), entry.clone(), err.clone(), open.clone(), check.clone(), then.clone(), busy.clone());
        move || {
            let secret = entry.text().to_string();
            if secret.is_empty() || busy.get() {
                return;
            }
            busy.set(true);
            open.set_sensitive(false);
            let (window, entry, err, open, check, then, busy) = (window.clone(), entry.clone(), err.clone(), open.clone(), check.clone(), then.clone(), busy.clone());
            glib::spawn_future_local(async move {
                let ok = gio::spawn_blocking(move || check.ok(&secret)).await.unwrap_or(false);
                busy.set(false);
                open.set_sensitive(true);
                if ok {
                    LOCK_WINDOW.with(|w| w.borrow_mut().take());
                    if let Some(app) = window.application().and_downcast::<adw::Application>() {
                        then(&app);
                    }
                    window.destroy();
                } else {
                    err.set_label(tr(lang, "Nu e bună. Mai încearcă.", "That's not it. Try again."));
                    err.set_visible(true);
                    entry.set_text("");
                    entry.grab_focus();
                }
            });
        }
    };
    let f = try_open.clone();
    open.connect_clicked(move |_| f());
    let f = try_open.clone();
    entry.connect_activate(move |_| f());
    // PIN-ul se trimite singur la a patra cifră (și primește doar cifre).
    if check.pin() {
        let f = try_open.clone();
        entry.connect_changed(move |e| {
            let t = e.text().to_string();
            let digits: String = t.chars().filter(|c| c.is_ascii_digit()).take(4).collect();
            if digits != t {
                e.set_text(&digits);
                return;
            }
            if digits.len() == 4 {
                f();
            }
        });
    }
    // Închiderea ferestrei de încuiere oprește Lanruj (jurnalul rămâne încuiat).
    window.connect_close_request(|w| {
        LOCK_WINDOW.with(|x| x.borrow_mut().take());
        if let Some(app) = w.application() {
            app.quit();
        }
        glib::Propagation::Proceed
    });
    LOCK_WINDOW.with(|w| *w.borrow_mut() = Some(window.clone()));
    window.present();
    entry.grab_focus();

    #[cfg(debug_assertions)]
    {
        if let Ok(p) = std::env::var("LANRUJ_PNG_INCUIAT") {
            let w = window.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(900), move || crate::save_png(&w, &p));
        }
        if let Ok(p) = std::env::var("LANRUJ_SECRET") {
            let e = entry.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(1300), move || {
                e.set_text(&p);
                e.emit_activate();
            });
        }
    }
}

/// Dialogul pentru o încuietoare nouă: parola (de 2 ori, cel puțin 4 caractere) sau PIN-ul (4 cifre, de 2 ori).
pub fn new_lock_dialog(app: &Rc<App>, kind: Lock, done: impl Fn(bool) + 'static) {
    let lang = app.lang;
    let pin = kind == Lock::Pin;
    let title = if pin { tr(lang, "PIN de deschidere", "Opening PIN") } else { tr(lang, "Parola de deschidere", "Opening password") };
    let d = adw::Dialog::builder().title(title).content_width(440).build();
    let b = gtk::Box::new(gtk::Orientation::Vertical, 10);
    b.set_margin_top(6);
    b.set_margin_bottom(18);
    b.set_margin_start(18);
    b.set_margin_end(18);
    b.append(
        &gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .label(if app.journal.key().is_some() {
                tr(
                    lang,
                    "Le ceri după blocarea ecranului, în locul parolei mari a jurnalului (care rămâne pentru pornire).",
                    "Asked after the screen locks, instead of the journal's big password (still used at startup).",
                )
            } else {
                tr(
                    lang,
                    "Lanruj o cere la deschidere. Jurnalul nu e criptat: încuietoarea ascunde fereastra, dar fișierele se pot citi cu alte programe.",
                    "Lanruj asks for it when opening. The journal isn't encrypted: the lock hides the window, but other programs can still read the files.",
                )
            })
            .build(),
    );
    let mk = |ph: &str| {
        gtk::PasswordEntry::builder().placeholder_text(ph).show_peek_icon(true).build()
    };
    let (p1, p2) = if pin {
        (mk(tr(lang, "PIN (4 cifre)", "PIN (4 digits)")), mk(tr(lang, "Încă o dată PIN-ul", "The PIN again")))
    } else {
        (mk(tr(lang, "Parola", "Password")), mk(tr(lang, "Încă o dată parola", "The password again")))
    };
    b.append(&p1);
    b.append(&p2);
    let err = gtk::Label::builder().css_classes(["error"]).xalign(0.0).visible(false).build();
    b.append(&err);
    let go = gtk::Button::builder().label(tr(lang, "Pune încuietoarea", "Set the lock")).css_classes(["suggested-action", "pill"]).halign(gtk::Align::End).build();
    b.append(&go);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&b));
    d.set_child(Some(&view));
    let done = Rc::new(done);
    let ok = Rc::new(std::cell::Cell::new(false));
    let (dn, okc) = (done.clone(), ok.clone());
    d.connect_closed(move |_| {
        if !okc.get() {
            dn(false);
        }
    });
    let (weak, dl) = (Rc::downgrade(app), d.clone());
    go.connect_clicked(move |btn| {
        let Some(app) = weak.upgrade() else { return };
        let (a, c) = (p1.text().to_string(), p2.text().to_string());
        let bad = if pin {
            (a.len() != 4 || !a.chars().all(|x| x.is_ascii_digit())).then(|| tr(lang, "PIN-ul are 4 cifre.", "The PIN has 4 digits."))
        } else {
            (a.chars().count() < 4).then(|| tr(lang, "Parola trebuie să aibă cel puțin 4 caractere.", "The password needs at least 4 characters."))
        };
        let bad = bad.or_else(|| (a != c).then(|| tr(lang, "Nu sunt la fel.", "They don't match.")));
        if let Some(m) = bad {
            err.set_label(m);
            err.set_visible(true);
            return;
        }
        btn.set_sensitive(false);
        let (dl, done, ok, weak) = (dl.clone(), done.clone(), ok.clone(), Rc::downgrade(&app));
        glib::spawn_future_local(async move {
            let res = gio::spawn_blocking(move || set_lock(Some(&a))).await;
            let Some(app) = weak.upgrade() else { return };
            match res {
                Ok(Ok(())) => {
                    ok.set(true);
                    dl.close();
                    done(true);
                }
                Ok(Err(e)) => {
                    app.error(&e);
                    dl.close();
                }
                Err(_) => {
                    dl.close();
                }
            }
        });
    });
    d.present(Some(&app.window));
}

impl App {
    /// Încuie Lanruj acum: fereastra jurnalului se ascunde, apare fereastra de încuiere.
    pub fn lock_now(self: &Rc<Self>) {
        if LOCK_WINDOW.with(|w| w.borrow().is_some()) {
            return;
        }
        let s = self.settings.borrow().clone();
        let encrypted = self.journal.key().is_some();
        let check = match (s.lock, encrypted) {
            (Lock::None, true) => Check::Journal(self.journal.root().to_path_buf()),
            (Lock::None, false) => return,
            (l, _) => Check::Lock(l),
        };
        let Some(app) = self.window.application().and_downcast::<adw::Application>() else { return };
        // Fără fereastra jurnalului, aplicația trebuie ținută pornită.
        let hold = app.hold();
        self.window.set_visible(false);
        let w = self.window.clone();
        let hold = RefCell::new(Some(hold));
        lock_window(&app, self.lang, check, move |_| {
            w.present();
            hold.borrow_mut().take();
        });
    }

    /// Ascultă blocarea ecranului (GNOME și restul, prin D-Bus).
    pub fn watch_screen_lock(self: &Rc<Self>) {
        let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else { return };
        for (name, path) in [("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"), ("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver")] {
            let weak = Rc::downgrade(self);
            #[allow(deprecated)]
            let id = bus.signal_subscribe(Some(name), Some(name), Some("ActiveChanged"), Some(path), None, gio::DBusSignalFlags::NONE, move |_, _, _, _, _, params| {
                let active = params.get::<(bool,)>().map(|(a,)| a).unwrap_or(false);
                let Some(app) = weak.upgrade() else { return };
                let s = app.settings.borrow().clone();
                if active && (app.journal.key().is_some() || (s.lock_on_screen && s.lock != Lock::None)) {
                    app.lock_now();
                }
            });
            // Abonarea ține cât fereastra.
            let bus2 = bus.clone();
            let id = RefCell::new(Some(id));
            self.window.connect_destroy(move |_| {
                if let Some(i) = id.borrow_mut().take() {
                    #[allow(deprecated)]
                    bus2.signal_unsubscribe(i);
                }
            });
        }
    }

    /// După deschiderea jurnalului criptat cu parola: o pune în portofel, dacă e cerut.
    pub fn remember_password(self: &Rc<Self>, password: String) {
        if !self.settings.borrow().remember {
            return;
        }
        let (root, weak) = (self.journal.root().to_path_buf(), Rc::downgrade(self));
        glib::spawn_future_local(async move {
            if let (Err(e), Some(app)) = (keyring_set(root, password).await, weak.upgrade()) {
                app.toast(&format!("{} {e}", tr(app.lang, "Portofelul de chei:", "Keyring:")));
            }
        });
    }
}
