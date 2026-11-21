// SPDX-License-Identifier: GPL-3.0-or-later
//! Exportul: pagina arătată ca PDF (cum arată în aplicație), mai multe zile ca PDF,
//! tipărirea acelorași pagini și tot jurnalul ca arhivă .zip.

use crate::page::Page;
use crate::{App, View, tr};
use adw::prelude::*;
use chrono::{Datelike, Duration, NaiveDate};
use gtk::{gio, glib, gsk};
use lanruj_core::Lang;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// O pagină de exportat: desenul ei și mărimea (în pixeli de ecran).
pub struct Shot {
    node: gsk::RenderNode,
    width: f64,
    height: f64,
}

fn node_of(w: &impl IsA<gtk::Widget>) -> Option<gsk::RenderNode> {
    let w = w.as_ref();
    let paintable = gtk::WidgetPaintable::new(Some(w));
    let snap = gtk::Snapshot::new();
    paintable.snapshot(&snap, w.width() as f64, w.height() as f64);
    snap.to_node()
}

/// O pagină a caietului desenată întreagă, chiar dacă pe ecran trebuie derulată:
/// fundalul (hârtia, titlul, subsolul) și separat lista intrărilor.
struct PagePic {
    bg: gsk::RenderNode,
    list: Option<gsk::RenderNode>,
    width: f64,
    height: f64,
    /// Unde începe și cât de înaltă e partea derulabilă.
    top: f64,
    view: f64,
    /// Unde stă lista în pagină și cât e de înaltă cu tot cu margini.
    list_x: f64,
    list_y: f64,
    content: f64,
}

impl PagePic {
    fn of(p: &Page) -> Option<PagePic> {
        let (width, height) = (p.root.width() as f64, p.root.height() as f64);
        if width < 1.0 || height < 1.0 || !p.root.is_visible() {
            return None;
        }
        let at = |w: &gtk::Widget| w.compute_point(&p.root, &gtk::graphene::Point::new(0.0, 0.0)).map(|pt| (pt.x() as f64, pt.y() as f64));
        let (_, top) = at(p.scroll.upcast_ref())?;
        let (list_x, list_y) = at(p.list.upcast_ref())?;
        // Fundalul fără listă: GTK trebuie lăsat să redeseneze după fiecare schimbare.
        p.list.set_opacity(0.0);
        settle(12);
        let bg = node_of(&p.root);
        p.list.set_opacity(1.0);
        settle(12);
        Some(PagePic {
            bg: bg?,
            list: node_of(&p.list),
            width,
            height,
            top,
            view: p.scroll.height() as f64,
            list_x,
            list_y,
            content: (p.list.height() + p.list.margin_top() + p.list.margin_bottom()) as f64,
        })
    }

    /// Cât trebuie lungită pagina ca să încapă toată lista.
    fn extra(&self) -> f64 {
        (self.content - self.view).max(0.0)
    }

    /// Desenează pagina lungită cu `extra` pixeli; hârtia din mijloc se repetă
    /// în benzi de 22 px (pasul punctelor), ca modelul să nu se rupă.
    fn draw(&self, snap: &gtk::Snapshot, x: f64, extra: f64) {
        let rect = |y: f64, h: f64| gtk::graphene::Rect::new(x as f32, y as f32, self.width as f32, h as f32);
        let band = ((self.view / 22.0).floor() * 22.0).max(22.0);
        let middle = self.view + extra;
        let paint = |clip: gtk::graphene::Rect, dy: f64| {
            snap.push_clip(&clip);
            snap.save();
            snap.translate(&gtk::graphene::Point::new(x as f32, dy as f32));
            snap.append_node(&self.bg);
            snap.restore();
            snap.pop();
        };
        paint(rect(0.0, self.top), 0.0);
        let mut done = 0.0;
        while done < middle {
            let h = band.min(middle - done);
            paint(rect(self.top + done, h), done);
            done += band;
        }
        paint(rect(self.top + middle, self.height - self.top - self.view), extra);
        if let Some(list) = &self.list {
            snap.save();
            snap.translate(&gtk::graphene::Point::new((x + self.list_x) as f32, self.list_y as f32));
            snap.append_node(list);
            snap.restore();
        }
    }
}

/// Lasă GTK să deseneze ce s-a schimbat (după ce aplicația a trecut pe altă pagină).
fn settle(rounds: u32) {
    let ctx = glib::MainContext::default();
    for _ in 0..rounds {
        while ctx.pending() {
            ctx.iteration(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
}

/// Ascunde (sau arată la loc) ce nu ține de jurnal: câmpurile de scris și „+ etichetă”.
fn hide_tools(w: &gtk::Widget, hide: bool, hidden: &mut Vec<gtk::Widget>) {
    // Paginile se desenează de la început, nu de unde sunt derulate.
    if let Some(sw) = w.downcast_ref::<gtk::ScrolledWindow>() {
        sw.vadjustment().set_value(0.0);
    }
    if hide {
        let tool = w.has_css_class("rapid") || (w.has_css_class("adauga") && w.is::<gtk::MenuButton>());
        if tool && w.is_visible() {
            w.set_visible(false);
            hidden.push(w.clone());
            return;
        }
    }
    let mut c = w.first_child();
    while let Some(ch) = c {
        hide_tools(&ch, hide, hidden);
        c = ch.next_sibling();
    }
}

/// Scrie paginile într-un PDF: fiecare pagină are mărimea desenului (1 px = 0,75 pt).
fn write_pdf(shots: &[Shot], path: &Path) -> Result<(), String> {
    const PT: f64 = 0.75;
    let first = shots.first().ok_or("nimic de exportat")?;
    let surface = cairo::PdfSurface::new(first.width * PT, first.height * PT, path).map_err(|e| e.to_string())?;
    let cr = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
    for (i, s) in shots.iter().enumerate() {
        if i > 0 {
            surface.set_size(s.width * PT, s.height * PT).map_err(|e| e.to_string())?;
        }
        cr.save().ok();
        cr.scale(PT, PT);
        s.node.draw(&cr);
        cr.restore().ok();
        cr.show_page().map_err(|e| e.to_string())?;
    }
    surface.finish();
    Ok(())
}

impl App {
    /// Pregătește caietul pentru desen, face `f`, apoi îl lasă cum era.
    fn clean<T>(self: &Rc<Self>, f: impl FnOnce(&Rc<Self>) -> T) -> T {
        self.exporting.set(true);
        self.refresh();
        let mut hidden = Vec::new();
        hide_tools(self.caiet.upcast_ref(), true, &mut hidden);
        settle(50);
        let out = f(self);
        for w in hidden {
            w.set_visible(true);
        }
        self.exporting.set(false);
        self.refresh();
        out
    }

    /// Ce se vede acum în caiet (cele două pagini sau una), întreg, ca o singură pagină.
    fn page_shot(&self) -> Option<Shot> {
        let pages: Vec<(PagePic, f64)> = [&self.left, &self.right]
            .into_iter()
            .filter_map(|p| {
                let pic = PagePic::of(p)?;
                let x = p.root.compute_point(&self.caiet, &gtk::graphene::Point::new(0.0, 0.0))?.x() as f64;
                Some((pic, x))
            })
            .collect();
        if pages.is_empty() {
            return None;
        }
        // Luna stă una sub alta pe ferestrele înguste: atunci fiecare pagină e separată.
        if self.caiet.orientation() == gtk::Orientation::Vertical || pages.len() == 1 {
            let (p, _) = pages.last()?;
            return Some(Self::one(p));
        }
        let extra = pages.iter().map(|(p, _)| p.extra()).fold(0.0, f64::max);
        let snap = gtk::Snapshot::new();
        // Cotorul și umbra caietului, de sus până jos.
        if let Some(bg) = node_of(&self.caiet) {
            let h = self.caiet.height() as f64;
            let w = self.caiet.width() as f32;
            snap.push_clip(&gtk::graphene::Rect::new(0.0, 0.0, w, (h + extra) as f32));
            snap.append_node(&bg);
            snap.pop();
        }
        let x0 = pages.first().map(|(_, x)| *x).unwrap_or(0.0);
        let width = pages.iter().map(|(p, x)| x + p.width).fold(0.0, f64::max) - x0;
        for (p, x) in &pages {
            p.draw(&snap, x - x0, extra);
        }
        let height = pages.iter().map(|(p, _)| p.height).fold(0.0, f64::max) + extra;
        Some(Shot { node: snap.to_node()?, width, height })
    }

    fn one(p: &PagePic) -> Shot {
        let snap = gtk::Snapshot::new();
        let extra = p.extra();
        p.draw(&snap, 0.0, extra);
        let node = snap.to_node().expect("pagina are desen");
        Shot { node, width: p.width, height: p.height + extra }
    }

    /// Pagina arătată acum, curată și întreagă.
    fn current_shot(self: &Rc<Self>) -> Option<Shot> {
        self.clean(|app| app.page_shot())
    }

    /// Fiecare zi din interval, ca pagina din dreapta a caietului (zilele de la `start` la `end`).
    fn day_shots(self: &Rc<Self>, start: NaiveDate, end: NaiveDate) -> Vec<Shot> {
        let (view, shown) = (self.view.get(), self.shown.get());
        let out = self.clean(|app| {
            let mut out = Vec::new();
            let mut d = start;
            while d <= end {
                app.go(d);
                let mut hidden = Vec::new();
                hide_tools(app.caiet.upcast_ref(), true, &mut hidden);
                settle(50);
                if let Some(p) = PagePic::of(&app.right) {
                    out.push(Self::one(&p));
                }
                for w in hidden {
                    w.set_visible(true);
                }
                d += Duration::days(1);
            }
            out
        });
        // Înapoi unde era utilizatorul.
        self.view.set(view);
        self.shown.set(shown);
        self.refresh();
        out
    }

    /// Exportul nu are sens în migrare sau în căutare: acolo caietul arată altceva.
    fn can_export(&self) -> bool {
        if self.migrating() || self.searching.get() {
            self.toast(tr(self.lang, "Termină întâi migrarea sau căutarea.", "Finish the migration or the search first."));
            return false;
        }
        true
    }

    /// Alege unde se salvează fișierul (`name` propus) și apoi face `then(cale)`.
    fn save_dialog(self: &Rc<Self>, title: &str, name: &str, then: impl FnOnce(&Rc<App>, PathBuf) + 'static) {
        let dialog = gtk::FileDialog::builder().title(title).initial_name(name).modal(true).build();
        let weak = Rc::downgrade(self);
        dialog.save(Some(&self.window), gio::Cancellable::NONE, move |res| {
            let (Some(app), Ok(file)) = (weak.upgrade(), res) else { return };
            if let Some(p) = file.path() {
                then(&app, p);
            }
        });
    }

    fn done_toast(&self, path: &Path) {
        let msg = match self.lang {
            Lang::Ro => format!("Salvat: {}", path.display()),
            Lang::En => format!("Saved: {}", path.display()),
        };
        self.toast(&msg);
    }

    /// Numele propus pentru fișier, după pagina arătată.
    fn export_name(&self) -> String {
        match self.view.get() {
            View::Day => format!("lanruj-{}", self.shown.get().format("%Y-%m-%d")),
            View::Week(y, w) => format!("lanruj-{y}-S{w:02}"),
            View::Month(y, m) => format!("lanruj-{y}-{m:02}"),
            View::Future(y, m) => format!("lanruj-planuri-{y}-{m:02}"),
            View::Index => "lanruj-index".into(),
            View::Topic(d, _) => format!("lanruj-pagina-{}", d.format("%Y-%m-%d")),
            View::Tracking(y, m) => format!("lanruj-serviciu-{y}-{m:02}"),
            View::Diary(d) => format!("lanruj-jurnal-{}", d.format("%Y-%m-%d")),
        }
    }

    /// Pentru probe: pagina arătată, direct în fișier.
    pub fn test_export_current(self: &Rc<Self>, path: &Path) -> Result<(), String> {
        write_pdf(&[self.current_shot().ok_or("nimic")?], path)
    }

    /// Pentru probe: ultimele 7 zile, direct în fișier.
    pub fn test_export_days(self: &Rc<Self>, path: &Path) -> Result<(), String> {
        let end = self.shown.get();
        write_pdf(&self.day_shots(end - Duration::days(6), end), path)
    }

    /// ☰ → Exportă pagina ca PDF.
    pub fn export_pdf_current(self: &Rc<Self>) {
        if !self.can_export() {
            return;
        }
        let name = format!("{}.pdf", self.export_name());
        // Monitorizarea are foile ei de tipar (A4), nu o captură a ecranului.
        if let View::Tracking(y, m) = self.view.get() {
            self.save_dialog(tr(self.lang, "Luna ca PDF", "The month as PDF"), &name, move |app, path| {
                match crate::tiparire::Sheet::new(app, y, m).write_pdf(&path) {
                    Ok(()) => app.done_toast(&path),
                    Err(e) => app.error(&std::io::Error::other(e)),
                }
            });
            return;
        }
        self.save_dialog(tr(self.lang, "Pagina ca PDF", "Page as PDF"), &name, |app, path| {
            let res = app.current_shot().ok_or_else(|| "nimic de exportat".to_string()).and_then(|s| write_pdf(&[s], &path));
            match res {
                Ok(()) => app.done_toast(&path),
                Err(e) => app.error(&std::io::Error::other(e)),
            }
        });
    }

    /// ☰ → Mai multe zile ca PDF: alegi intervalul, apoi fișierul.
    pub fn export_pdf_days(self: &Rc<Self>) {
        if !self.can_export() {
            return;
        }
        self.range_dialog(tr(self.lang, "Mai multe zile ca PDF", "Several days as PDF"), |app, start, end| {
            let name = format!("lanruj-{}-{}.pdf", start.format("%Y-%m-%d"), end.format("%Y-%m-%d"));
            app.save_dialog(tr(app.lang, "Mai multe zile ca PDF", "Several days as PDF"), &name, move |app, path| {
                let shots = app.day_shots(start, end);
                match write_pdf(&shots, &path) {
                    Ok(()) => app.done_toast(&path),
                    Err(e) => app.error(&std::io::Error::other(e)),
                }
            });
        });
    }

    /// Fereastra de alegere a intervalului: de la, până la.
    fn range_dialog(self: &Rc<Self>, title: &str, then: impl Fn(&Rc<App>, NaiveDate, NaiveDate) + 'static) {
        let lang = self.lang;
        let dialog = adw::Dialog::builder().title(title).content_width(640).build();
        let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
        b.set_margin_top(6);
        b.set_margin_bottom(12);
        b.set_margin_start(12);
        b.set_margin_end(12);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_homogeneous(true);
        let col = |label: &str, d: NaiveDate| {
            let v = gtk::Box::new(gtk::Orientation::Vertical, 4);
            v.append(&gtk::Label::builder().label(label).xalign(0.0).css_classes(["heading"]).build());
            let c = gtk::Calendar::new();
            if let Ok(dt) = glib::DateTime::from_local(d.year(), d.month() as i32, d.day() as i32, 12, 0, 0.0) {
                c.select_day(&dt);
            }
            v.append(&c);
            (v, c)
        };
        let shown = self.shown.get();
        let (v1, c1) = col(tr(lang, "De la", "From"), shown - Duration::days(6));
        let (v2, c2) = col(tr(lang, "Până la", "To"), shown);
        row.append(&v1);
        row.append(&v2);
        b.append(&row);
        let go = gtk::Button::builder().label(tr(lang, "Continuă", "Continue")).css_classes(["suggested-action"]).halign(gtk::Align::End).build();
        b.append(&go);
        let view = adw::ToolbarView::new();
        view.add_top_bar(&adw::HeaderBar::new());
        view.set_content(Some(&b));
        dialog.set_child(Some(&view));
        let to_date = |c: &gtk::Calendar| {
            let dt = c.date();
            NaiveDate::from_ymd_opt(dt.year(), dt.month() as u32, dt.day_of_month() as u32)
        };
        let (weak, dl) = (Rc::downgrade(self), dialog.clone());
        go.connect_clicked(move |_| {
            let (Some(app), Some(a), Some(z)) = (weak.upgrade(), to_date(&c1), to_date(&c2)) else { return };
            let (a, z) = if a <= z { (a, z) } else { (z, a) };
            if (z - a).num_days() > 366 {
                app.toast(tr(app.lang, "Alege cel mult un an.", "Pick at most one year."));
                return;
            }
            dl.close();
            then(&app, a, z);
        });
        dialog.present(Some(&self.window));
    }

    /// ☰ → Tipărește (Ctrl+P): pagina arătată, prin fereastra de tipărire a sistemului.
    pub fn print_current(self: &Rc<Self>) {
        if !self.can_export() {
            return;
        }
        if let View::Tracking(y, m) = self.view.get() {
            self.print_tracking(y, m);
            return;
        }
        let Some(s) = self.current_shot() else { return };
        self.print_shots(vec![s]);
    }

    /// ☰ → Tipărește mai multe zile.
    pub fn print_days(self: &Rc<Self>) {
        if !self.can_export() {
            return;
        }
        self.range_dialog(tr(self.lang, "Tipărește mai multe zile", "Print several days"), |app, start, end| {
            let shots = app.day_shots(start, end);
            app.print_shots(shots);
        });
    }

    fn print_shots(self: &Rc<Self>, shots: Vec<Shot>) {
        if shots.is_empty() {
            return;
        }
        let op = gtk::PrintOperation::new();
        op.set_job_name("Lanruj");
        op.set_n_pages(shots.len() as i32);
        op.set_embed_page_setup(true);
        let shots = Rc::new(shots);
        op.connect_draw_page(move |_, ctx, n| {
            let Some(s) = shots.get(n as usize) else { return };
            let cr = ctx.cairo_context();
            // Pagina încape în foaie, cu proporțiile ei.
            let scale = (ctx.width() / s.width).min(ctx.height() / s.height);
            cr.save().ok();
            cr.scale(scale, scale);
            s.node.draw(&cr);
            cr.restore().ok();
        });
        if let Err(e) = op.run(gtk::PrintOperationAction::PrintDialog, Some(&self.window)) {
            self.error(&std::io::Error::other(e.to_string()));
        }
    }

    /// ☰ → Tot jurnalul ca arhivă .zip: întâi alegi parola (jurnalul criptat are mereu), apoi fișierul.
    pub fn export_zip(self: &Rc<Self>) {
        let lang = self.lang;
        let encrypted = self.journal.key().is_some();
        let dialog = adw::Dialog::builder().title(tr(lang, "Tot jurnalul ca arhivă zip", "The whole journal as zip")).content_width(480).build();
        let b = gtk::Box::new(gtk::Orientation::Vertical, 10);
        b.set_margin_top(6);
        b.set_margin_bottom(18);
        b.set_margin_start(18);
        b.set_margin_end(18);
        let info = gtk::Label::builder().wrap(true).xalign(0.0).build();
        info.set_label(if encrypted {
            tr(lang, "Jurnalul e criptat, deci și arhiva are parolă. Se deschide cu parola în 7-Zip sau în managerul de arhive.", "The journal is encrypted, so the archive has a password too. It opens with the password in 7-Zip or the archive manager.")
        } else {
            tr(lang, "Toate paginile și pozele, într-o singură arhivă. Cu parolă, arhiva se deschide doar cu ea (în 7-Zip sau în managerul de arhive).", "All pages and pictures in one archive. With a password, it opens only with it (in 7-Zip or the archive manager).")
        });
        b.append(&info);
        let with = gtk::CheckButton::with_label(tr(lang, "Cu parolă", "With a password"));
        let without = gtk::CheckButton::with_label(tr(lang, "Fără parolă", "Without a password"));
        without.set_group(Some(&with));
        if encrypted {
            with.set_active(true);
        } else {
            without.set_active(true);
            b.append(&without);
            b.append(&with);
        }
        let p1 = gtk::PasswordEntry::builder().placeholder_text(tr(lang, "Parola arhivei", "Archive password")).show_peek_icon(true).build();
        let p2 = gtk::PasswordEntry::builder().placeholder_text(tr(lang, "Încă o dată parola", "The password again")).show_peek_icon(true).build();
        for p in [&p1, &p2] {
            with.bind_property("active", p, "visible").sync_create().build();
            b.append(p);
        }
        let err = gtk::Label::builder().css_classes(["error"]).xalign(0.0).wrap(true).visible(false).build();
        b.append(&err);
        let go = gtk::Button::builder().label(tr(lang, "Alege unde se salvează…", "Choose where to save…")).css_classes(["suggested-action", "pill"]).halign(gtk::Align::End).build();
        b.append(&go);
        let view = adw::ToolbarView::new();
        view.add_top_bar(&adw::HeaderBar::new());
        view.set_content(Some(&b));
        dialog.set_child(Some(&view));
        let (weak, dl) = (Rc::downgrade(self), dialog.clone());
        go.connect_clicked(move |_| {
            let Some(app) = weak.upgrade() else { return };
            let password = if with.is_active() {
                let (a, c) = (p1.text().to_string(), p2.text().to_string());
                let msg = if a.chars().count() < 8 {
                    Some(tr(lang, "Parola trebuie să aibă cel puțin 8 caractere.", "The password needs at least 8 characters."))
                } else if a != c {
                    Some(tr(lang, "Cele două parole nu sunt la fel.", "The two passwords don't match."))
                } else {
                    None
                };
                if let Some(m) = msg {
                    err.set_label(m);
                    err.set_visible(true);
                    return;
                }
                Some(a)
            } else {
                None
            };
            dl.close();
            let name = format!("lanruj-{}.zip", app.today.get().format("%Y-%m-%d"));
            app.save_dialog(tr(lang, "Tot jurnalul ca arhivă", "The whole journal as an archive"), &name, move |app, path| {
                let (root, key) = (app.journal.root().to_path_buf(), app.journal.key());
                let weak = Rc::downgrade(app);
                let password = password.clone();
                app.toast(tr(app.lang, "Se face arhiva…", "Making the archive…"));
                glib::spawn_future_local(async move {
                    let p2 = path.clone();
                    let res = gio::spawn_blocking(move || {
                        let j = lanruj_core::Journal::new(&root);
                        j.set_key(key);
                        write_zip_readable(&j, &p2, password.as_deref())
                    })
                    .await;
                    let Some(app) = weak.upgrade() else { return };
                    match res {
                        Ok(Ok(n)) => {
                            let msg = match app.lang {
                                Lang::Ro => format!("Arhiva are {n} fișiere: {}", path.display()),
                                Lang::En => format!("The archive has {n} files: {}", path.display()),
                            };
                            app.toast(&msg);
                        }
                        Ok(Err(e)) => app.error(&e),
                        Err(_) => {}
                    }
                });
            });
        });
        dialog.present(Some(&self.window));
    }
}

/// Arhiva de export: fișierele jurnalului decriptate (dacă e cazul), cu parolă AES-256 sau fără.
pub fn write_zip_readable(j: &lanruj_core::Journal, out: &Path, password: Option<&str>) -> std::io::Result<usize> {
    let file = std::fs::File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let mut opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    if let Some(p) = password {
        opts = opts.with_aes_encryption(zip::AesMode::Aes256, p);
    }
    let root = j.root();
    let mut count = 0;
    for p in j.all_files() {
        if p == out {
            continue;
        }
        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
        let data = j.read_bytes(&p)?;
        zip.start_file(format!("lanruj/{rel}"), opts).map_err(std::io::Error::other)?;
        zip.write_all(&data)?;
        count += 1;
    }
    zip.finish().map_err(std::io::Error::other)?;
    Ok(count)
}

/// Pune în arhivă tot dosarul jurnalului (fără fișierele ascunse de stare și setări).
pub fn write_zip(root: &Path, out: &Path) -> std::io::Result<usize> {
    let file = std::fs::File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut count = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&dir)?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name.ends_with(".tmp") {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() && p != out {
                let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                zip.start_file(format!("lanruj/{rel}"), opts).map_err(std::io::Error::other)?;
                zip.write_all(&std::fs::read(&p)?)?;
                count += 1;
            }
        }
    }
    zip.finish().map_err(std::io::Error::other)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn arhiva_cu_parola() {
        use std::io::Read;
        let root = std::env::temp_dir().join(format!("lanruj-zipp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let j = lanruj_core::Journal::new(&root);
        let (key, _) = lanruj_core::crypt::create(&root, "parola-jurnal").unwrap();
        j.set_key(Some(std::sync::Arc::new(key)));
        j.write_text(&root.join("2026/11/2026-11-21.md"), "# zi\n\n- Secret\n").unwrap();
        let out = std::env::temp_dir().join(format!("lanruj-zipp-{}.zip", std::process::id()));
        assert_eq!(super::write_zip_readable(&j, &out, Some("parola-arhiva")).unwrap(), 1, "fără fișierele cheii");
        let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        assert!(z.by_index(0).is_err(), "fără parolă nu se deschide");
        let mut text = String::new();
        z.by_index_decrypt(0, b"parola-arhiva").unwrap().read_to_string(&mut text).unwrap();
        assert_eq!(text, "# zi\n\n- Secret\n", "în arhivă e text, nu conținut age");
        std::fs::remove_dir_all(root).unwrap();
        if let Ok(keep) = std::env::var("LANRUJ_PASTREAZA_ZIP") {
            std::fs::copy(&out, keep).unwrap();
        }
        std::fs::remove_file(out).unwrap();
    }

    #[test]
    fn arhiva_jurnalului() {
        let root = std::env::temp_dir().join(format!("lanruj-zip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("2026/11/2026-11-21")).unwrap();
        std::fs::write(root.join("2026/11/2026-11-21.md"), "# zi\n").unwrap();
        std::fs::write(root.join("2026/11/2026-11-21/poza.jpg"), b"jpg").unwrap();
        std::fs::write(root.join("index.md"), "# Index\n").unwrap();
        std::fs::write(root.join(".setari.ini"), "x").unwrap();
        let out = std::env::temp_dir().join(format!("lanruj-zip-{}.zip", std::process::id()));
        assert_eq!(super::write_zip(&root, &out).unwrap(), 3, "fără fișierele ascunse");
        let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let mut names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().to_string()).collect();
        names.sort();
        assert_eq!(names, vec!["lanruj/2026/11/2026-11-21.md", "lanruj/2026/11/2026-11-21/poza.jpg", "lanruj/index.md"]);
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_file(out).unwrap();
    }
}
