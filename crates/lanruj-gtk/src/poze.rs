// SPDX-License-Identifier: GPL-3.0-or-later
//! Pozele și fișierele din pagini: poza singură (cu nume și mărime), galeria (mai multe
//! poze una după alta, ca pătrățele egale), rândul de fișier și fereastra de mărire.

use crate::page::open_uri;
use crate::{App, tr};
use adw::prelude::*;
use gtk::{gdk, glib};
use lanruj_core::attach::{Attachment, IMAGE_SIZES};
use lanruj_core::{Item, Lang, PageRef};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// O poză din pagină, pentru fereastra de mărire.
#[derive(Clone)]
pub struct Shown {
    pub abs: PathBuf,
    pub name: String,
}

/// Pozele unei pagini, în ordine (pentru ← → în fereastra de mărire).
pub fn page_images(app: &App, r: PageRef, items: &[Item]) -> Vec<(usize, Shown)> {
    items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| match it {
            Item::Other(l) => match lanruj_core::attach::parse(l) {
                Some(Attachment::Image { name, path, .. }) => Some((i, Shown { abs: app.journal.attachment_abs(r, &path), name })),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Poza micșorată exact la lățimea `w` (înălțimea după proporții), făcută o singură dată.
fn sized(abs: &Path, w: i32) -> Option<gdk::Texture> {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<(PathBuf, i32), gdk::Texture>> = Default::default();
    }
    let key = (abs.to_path_buf(), w);
    if let Some(t) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Some(t);
    }
    let pb = gtk::gdk_pixbuf::Pixbuf::from_file(abs).ok()?;
    let pb = pb.apply_embedded_orientation().unwrap_or(pb);
    let h = (w as f64 * pb.height() as f64 / pb.width().max(1) as f64).round().max(1.0) as i32;
    let small = pb.scale_simple(w, h, gtk::gdk_pixbuf::InterpType::Bilinear)?;
    #[allow(deprecated)]
    let tex = gdk::Texture::for_pixbuf(&small);
    CACHE.with(|c| c.borrow_mut().insert(key, tex.clone()));
    Some(tex)
}

/// Miniatura pătrată (mijlocul pozei), de `size` pixeli.
fn square(abs: &Path, size: i32) -> Option<gdk::Texture> {
    let pb = gtk::gdk_pixbuf::Pixbuf::from_file(abs).ok()?;
    let pb = pb.apply_embedded_orientation().unwrap_or(pb);
    let (w, h) = (pb.width(), pb.height());
    let side = w.min(h);
    let sub = pb.new_subpixbuf((w - side) / 2, (h - side) / 2, side, side);
    let small = sub.scale_simple(size, size, gtk::gdk_pixbuf::InterpType::Bilinear)?;
    #[allow(deprecated)]
    let tex = gdk::Texture::for_pixbuf(&small);
    Some(tex)
}

fn missing(lang: Lang, rel: &str) -> gtk::Widget {
    gtk::Label::builder()
        .label(format!("{} {rel}", tr(lang, "Poza lipsește:", "Missing picture:")))
        .xalign(0.0)
        .css_classes(["gol"])
        .build()
        .upcast()
}

/// Clic = fereastra de mărire, la poza `pos`; clic dreapta = meniul pozei.
fn hook_image(app: &Rc<App>, w: &gtk::Widget, holder: &gtk::Box, r: PageRef, index: usize, images: Rc<Vec<(usize, Shown)>>) {
    w.set_cursor_from_name(Some("zoom-in"));
    let click = gtk::GestureClick::builder().button(1).build();
    let weak = Rc::downgrade(app);
    let imgs = images.clone();
    click.connect_released(move |_, _, _, _| {
        let Some(app) = weak.upgrade() else { return };
        let pos = imgs.iter().position(|(i, _)| *i == index).unwrap_or(0);
        let list: Vec<Shown> = imgs.iter().map(|(_, s)| s.clone()).collect();
        glib::idle_add_local_once(move || viewer(&app, list, pos));
    });
    w.add_controller(click);
    image_menu(app, holder, r, index);
}

/// O poză singură, la mărimea ei, cu numele dedesubt.
pub fn single(app: &Rc<App>, r: PageRef, index: usize, a: &Attachment, images: Rc<Vec<(usize, Shown)>>) -> gtk::Widget {
    let Attachment::Image { name, width, path } = a else { return file_row(app, r, index, a) };
    let abs = app.journal.attachment_abs(r, path);
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 2);
    holder.set_halign(gtk::Align::Start);
    holder.set_margin_top(4);
    holder.set_margin_bottom(4);
    // Cel mult „Mare”, ca o lățime scrisă în alt program să nu iasă din pagină.
    let w = width.unwrap_or(IMAGE_SIZES[1].2).min(IMAGE_SIZES[2].2) as i32;
    match sized(&abs, w) {
        Some(tex) => {
            // Poza are exact lățimea aleasă (și înălțimea după proporții): nu ocupă mai mult loc decât se vede.
            let pic = gtk::Picture::for_paintable(&tex);
            pic.set_can_shrink(false);
            pic.set_halign(gtk::Align::Start);
            pic.set_valign(gtk::Align::Start);
            pic.add_css_class("poza");
            holder.append(&pic);
            hook_image(app, pic.upcast_ref(), &holder, r, index, images);
        }
        None => holder.append(&missing(app.lang, path)),
    }
    if !name.is_empty() {
        // Numele are lățimea pozei: un nume care se rupe pe rânduri nu mai lasă gol sub poză
        // (GTK măsura înălțimea etichetei pentru o lățime foarte mică).
        let cap = gtk::Label::builder().label(name.as_str()).xalign(0.0).wrap(true).css_classes(["poza-nume"]).build();
        cap.set_size_request(w, -1);
        cap.set_max_width_chars((w / 7).max(8));
        holder.append(&cap);
    }
    // Într-un rând orizontal: pagina măsoară poza la lățimea ei, nu la lățimea paginii
    // (altfel GTK îi rezerva înălțimea pentru toată lățimea și lăsa un gol sub ea).
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.append(&holder);
    row.upcast()
}

/// Galeria: mai multe poze una după alta, ca pătrățele egale, cu numele dedesubt.
pub fn gallery(app: &Rc<App>, r: PageRef, run: &[(usize, Attachment)], images: Rc<Vec<(usize, Shown)>>) -> gtk::Widget {
    const TILE: i32 = 128;
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(8)
        .column_spacing(8)
        .row_spacing(8)
        .homogeneous(false)
        .css_classes(["galerie"])
        .build();
    flow.set_margin_top(4);
    flow.set_margin_bottom(4);
    for (index, a) in run {
        let Attachment::Image { name, path, .. } = a else { continue };
        let abs = app.journal.attachment_abs(r, path);
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 2);
        holder.set_halign(gtk::Align::Start);
        holder.set_valign(gtk::Align::Start);
        holder.set_size_request(TILE, -1);
        match square(&abs, TILE * 2) {
            Some(tex) => {
                // Pătrățele egale: mijlocul pozei, tăiat pătrat.
                let pic = gtk::Image::from_paintable(Some(&tex));
                pic.set_pixel_size(TILE);
                pic.add_css_class("poza");
                pic.set_overflow(gtk::Overflow::Hidden);
                holder.append(&pic);
                hook_image(app, pic.upcast_ref(), &holder, r, *index, images.clone());
            }
            None => holder.append(&missing(app.lang, path)),
        }
        if !name.is_empty() {
            holder.append(
                &gtk::Label::builder()
                    .label(name.as_str())
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .max_width_chars(16)
                    .css_classes(["poza-nume"])
                    .build(),
            );
        }
        flow.append(&holder);
    }
    flow.upcast()
}

/// Un fișier (nu poză): iconița și numele; clic = îl deschide.
pub fn file_row(app: &Rc<App>, r: PageRef, index: usize, a: &Attachment) -> gtk::Widget {
    let lang = app.lang;
    let abs = app.journal.attachment_abs(r, a.path());
    let name = match a {
        Attachment::File { name, .. } | Attachment::Image { name, .. } => name.clone(),
    };
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.set_margin_top(2);
    holder.set_margin_bottom(2);
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    b.add_css_class("fisier");
    let (ctype, _) = gtk::gio::content_type_guess(Some(&abs), None);
    let img = gtk::Image::from_gicon(&gtk::gio::content_type_get_icon(&ctype));
    img.set_pixel_size(24);
    b.append(&img);
    b.append(&gtk::Label::builder().label(name.as_str()).xalign(0.0).wrap(true).css_classes(["text"]).build());
    b.set_tooltip_text(Some(tr(lang, "Clic: deschide; clic dreapta: leagă sau scoate", "Click: open; right-click: link or remove")));
    b.set_cursor_from_name(Some("pointer"));
    let click = gtk::GestureClick::builder().button(1).build();
    click.connect_released(move |g, _, _, _| {
        if let Some(w) = g.widget() {
            open_uri(&w, &format!("file://{}", abs.display()));
        }
    });
    b.add_controller(click);
    holder.append(&b);
    image_menu(app, &holder, r, index);
    holder.upcast()
}

/// Meniul de la clic dreapta pe o poză sau un fișier: numele, mărimea (poze), „Leagă de…”, „Scoate”.
fn image_menu(app: &Rc<App>, holder: &gtk::Box, r: PageRef, index: usize) {
    let lang = app.lang;
    let pop = gtk::Popover::new();
    pop.set_parent(holder);
    let p = pop.clone();
    holder.connect_destroy(move |_| p.unparent());
    let weak = Rc::downgrade(app);
    let p2 = pop.clone();
    // Conținutul se face la deschidere (citește rândul și intrările de acum ale paginii).
    let fill = move || {
        let Some(app) = weak.upgrade() else { return };
        let Ok(day) = app.journal.load_ref(r) else { return };
        let Some(Item::Other(line)) = day.items.get(index) else { return };
        let Some(a) = lanruj_core::attach::parse(line) else { return };
        let b = gtk::Box::new(gtk::Orientation::Vertical, 6);
        b.set_margin_top(4);
        b.set_margin_bottom(4);
        b.set_margin_start(4);
        b.set_margin_end(4);
        if let Attachment::Image { name, width, path } = &a {
            b.append(&gtk::Label::builder().label(tr(lang, "Numele pozei", "Picture name")).xalign(0.0).css_classes(["heading"]).build());
            let e = gtk::Entry::builder().text(name.as_str()).width_chars(24).css_classes(["poza-nume-camp"]).build();
            let (w2, pa, wd, pp) = (Rc::downgrade(&app), path.clone(), *width, p2.clone());
            e.connect_activate(move |en| {
                let n = en.text().trim().replace(['[', ']', '|'], "");
                pp.popdown();
                if let Some(app) = w2.upgrade() {
                    let a = Attachment::Image { name: n, width: wd, path: pa.clone() };
                    glib::idle_add_local_once(move || app.set_attachment(r, index, a));
                }
            });
            b.append(&e);
            b.append(&gtk::Label::builder().label(tr(lang, "Mărimea în pagină", "Size on the page")).xalign(0.0).css_classes(["heading"]).build());
            let sizes = gtk::Box::builder().css_classes(["linked"]).build();
            for (ro, en, w) in IMAGE_SIZES {
                let t = gtk::ToggleButton::builder().label(if lang == Lang::Ro { ro } else { en }).active(width.unwrap_or(IMAGE_SIZES[1].2) == w).build();
                let (w2, n, pa, pp) = (Rc::downgrade(&app), name.clone(), path.clone(), p2.clone());
                t.connect_clicked(move |_| {
                    pp.popdown();
                    if let Some(app) = w2.upgrade() {
                        let a = Attachment::Image { name: n.clone(), width: Some(w), path: pa.clone() };
                        glib::idle_add_local_once(move || app.set_attachment(r, index, a));
                    }
                });
                sizes.append(&t);
            }
            b.append(&sizes);
            b.append(
                &gtk::Label::builder()
                    .label(tr(lang, "Mai multe poze una după alta se arată ca galerie.", "Several pictures in a row show as a gallery."))
                    .xalign(0.0)
                    .wrap(true)
                    .css_classes(["mic", "dim-label"])
                    .build(),
            );
        }
        // Leagă de o intrare: lista intrărilor de primul nivel din pagină.
        b.append(&gtk::Label::builder().label(tr(lang, "Leagă de…", "Link to…")).xalign(0.0).css_classes(["heading"]).build());
        let mut targets: Vec<Option<usize>> = vec![None];
        let mut names: Vec<String> = vec![tr(lang, "(nicio intrare, la sfârșitul paginii)", "(no entry, end of page)").to_string()];
        for (i, it) in day.items.iter().enumerate() {
            if let Item::Entry(e) = it
                && e.depth == 0
            {
                let t: String = lanruj_core::colors::plain(&e.text).chars().take(40).collect();
                targets.push(Some(i));
                names.push(t);
            }
        }
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let dd = gtk::DropDown::from_strings(&refs);
        dd.set_hexpand(true);
        row.append(&dd);
        let go = gtk::Button::builder().label(tr(lang, "Leagă", "Link")).build();
        let (w2, pp, dd2) = (Rc::downgrade(&app), p2.clone(), dd.clone());
        go.connect_clicked(move |_| {
            let Some(t) = targets.get(dd2.selected() as usize).copied() else { return };
            pp.popdown();
            if let Some(app) = w2.upgrade() {
                glib::idle_add_local_once(move || app.move_attachment(r, index, t));
            }
        });
        row.append(&go);
        b.append(&row);
        let rm = gtk::Button::builder().label(tr(lang, "Scoate din pagină", "Remove from the page")).css_classes(["flat", "error"]).build();
        let (w2, pp) = (Rc::downgrade(&app), p2.clone());
        rm.connect_clicked(move |_| {
            pp.popdown();
            if let Some(app) = w2.upgrade() {
                glib::idle_add_local_once(move || app.remove_line(r, index));
            }
        });
        b.append(&rm);
        p2.set_child(Some(&b));
    };
    let right = gtk::GestureClick::builder().button(3).build();
    let p = pop.clone();
    right.connect_pressed(move |g, _, _, _| {
        fill();
        p.popup();
        g.set_state(gtk::EventSequenceState::Claimed);
    });
    holder.add_controller(right);
}

/// Fereastra de mărire: poza mare, zoom (+ − 0, Ctrl+rotița), ← → pentru celelalte poze, Esc închide.
pub fn viewer(app: &Rc<App>, images: Vec<Shown>, start: usize) {
    if images.is_empty() {
        return;
    }
    let lang = app.lang;
    let dialog = adw::Dialog::builder().content_width(1000).content_height(720).build();
    let header = adw::HeaderBar::new();
    let title = adw::WindowTitle::new("", "");
    header.set_title_widget(Some(&title));
    let prev = gtk::Button::from_icon_name("go-previous-symbolic");
    prev.set_tooltip_text(Some(tr(lang, "Poza de dinainte (←)", "Previous picture (←)")));
    let next = gtk::Button::from_icon_name("go-next-symbolic");
    next.set_tooltip_text(Some(tr(lang, "Poza următoare (→)", "Next picture (→)")));
    let zin = gtk::Button::from_icon_name("zoom-in-symbolic");
    zin.set_tooltip_text(Some(tr(lang, "Mărește (+)", "Zoom in (+)")));
    let zout = gtk::Button::from_icon_name("zoom-out-symbolic");
    zout.set_tooltip_text(Some(tr(lang, "Micșorează (−)", "Zoom out (−)")));
    let fit = gtk::Button::from_icon_name("zoom-fit-best-symbolic");
    fit.set_tooltip_text(Some(tr(lang, "Potrivește în fereastră (0)", "Fit to window (0)")));
    header.pack_start(&prev);
    header.pack_start(&next);
    header.pack_end(&fit);
    header.pack_end(&zin);
    header.pack_end(&zout);
    let pic = gtk::Picture::builder().can_shrink(true).content_fit(gtk::ContentFit::Contain).build();
    let scroll = gtk::ScrolledWindow::builder().child(&pic).hexpand(true).vexpand(true).build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    dialog.set_child(Some(&view));

    // Starea: poza arătată, zoomul (0 = potrivită în fereastră) și poza întreagă, citită o dată.
    struct St {
        pos: std::cell::Cell<usize>,
        zoom: std::cell::Cell<f64>,
        full: std::cell::RefCell<Option<gtk::gdk_pixbuf::Pixbuf>>,
        imgs: Vec<Shown>,
    }
    let state = Rc::new(St { pos: std::cell::Cell::new(start.min(images.len() - 1)), zoom: std::cell::Cell::new(0.0), full: Default::default(), imgs: images });
    let load = {
        let state = state.clone();
        move || {
            let s = &state.imgs[state.pos.get()];
            let pb = gtk::gdk_pixbuf::Pixbuf::from_file(&s.abs).ok().map(|p| p.apply_embedded_orientation().unwrap_or(p));
            *state.full.borrow_mut() = pb;
        }
    };
    let show = {
        let (pic, title, state, prev, next) = (pic.clone(), title.clone(), state.clone(), prev.clone(), next.clone());
        move || {
            let i = state.pos.get();
            let s = &state.imgs[i];
            let z = state.zoom.get();
            let full = state.full.borrow();
            if let Some(pb) = full.as_ref() {
                if z > 0.0 {
                    // Zoom: poza pregătită exact la mărimea cerută; mai mare decât fereastra = se derulează.
                    let (w, h) = ((pb.width() as f64 * z).round() as i32, (pb.height() as f64 * z).round() as i32);
                    let scaled = pb.scale_simple(w.max(1), h.max(1), gtk::gdk_pixbuf::InterpType::Bilinear);
                    #[allow(deprecated)]
                    let tex = scaled.as_ref().map(gdk::Texture::for_pixbuf);
                    pic.set_paintable(tex.as_ref());
                    pic.set_can_shrink(false);
                    pic.set_halign(gtk::Align::Center);
                    pic.set_valign(gtk::Align::Center);
                } else {
                    #[allow(deprecated)]
                    let tex = gdk::Texture::for_pixbuf(pb);
                    pic.set_paintable(Some(&tex));
                    pic.set_can_shrink(true);
                    pic.set_halign(gtk::Align::Fill);
                    pic.set_valign(gtk::Align::Fill);
                }
            } else {
                pic.set_paintable(gdk::Paintable::NONE);
            }
            let name = if s.name.is_empty() { s.abs.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string() } else { s.name.clone() };
            title.set_title(&name);
            let zt = if z > 0.0 { format!(" · {}%", (z * 100.0).round()) } else { String::new() };
            title.set_subtitle(&format!("{} / {}{zt}", i + 1, state.imgs.len()));
            prev.set_sensitive(i > 0);
            next.set_sensitive(i + 1 < state.imgs.len());
        }
    };
    let show = Rc::new(show);
    // Zoom față de mărimea arătată acum; din „potrivit” pornește de la raportul poză/fereastră.
    let zoom_by = {
        let (state, show, scroll) = (state.clone(), show.clone(), scroll.clone());
        move |f: f64| {
            let base = {
                let full = state.full.borrow();
                let Some(pb) = full.as_ref() else { return };
                let z = state.zoom.get();
                if z > 0.0 {
                    z
                } else {
                    (scroll.width() as f64 / pb.width() as f64).min(scroll.height() as f64 / pb.height().max(1) as f64).min(1.0)
                }
            };
            // Cel mult 4× și cel mult 8000 de pixeli pe latură.
            let max = {
                let full = state.full.borrow();
                full.as_ref().map(|pb| 4.0f64.min(8000.0 / pb.width().max(pb.height()) as f64)).unwrap_or(4.0)
            };
            state.zoom.set((base * f).clamp(0.05, max));
            show();
        }
    };
    let zoom_by = Rc::new(zoom_by);
    let go = {
        let (state, show, load) = (state.clone(), show.clone(), load.clone());
        move |d: i32| {
            let n = state.imgs.len() as i32;
            let i = (state.pos.get() as i32 + d).clamp(0, n - 1);
            if i as usize != state.pos.get() {
                state.pos.set(i as usize);
                state.zoom.set(0.0);
                load();
                show();
            }
        }
    };
    let go = Rc::new(go);
    let g = go.clone();
    prev.connect_clicked(move |_| g(-1));
    let g = go.clone();
    next.connect_clicked(move |_| g(1));
    let z = zoom_by.clone();
    zin.connect_clicked(move |_| z(1.25));
    let z = zoom_by.clone();
    zout.connect_clicked(move |_| z(0.8));
    let (st, sh) = (state.clone(), show.clone());
    fit.connect_clicked(move |_| {
        st.zoom.set(0.0);
        sh();
    });
    // Tastele și rotița se prind înaintea altor părți ale ferestrei.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let (g, z, st, sh) = (go.clone(), zoom_by.clone(), state.clone(), show.clone());
    keys.connect_key_pressed(move |_, key, _, _| {
        match key {
            gdk::Key::Left => g(-1),
            gdk::Key::Right => g(1),
            gdk::Key::plus | gdk::Key::KP_Add | gdk::Key::equal => z(1.25),
            gdk::Key::minus | gdk::Key::KP_Subtract => z(0.8),
            gdk::Key::_0 | gdk::Key::KP_0 => {
                st.zoom.set(0.0);
                sh();
            }
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    });
    dialog.add_controller(keys);
    // Rotița: cu Ctrl sau fără, mărește și micșorează (derularea rămâne pe bare și pe trackpad cu două degete).
    let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
    let z = zoom_by.clone();
    wheel.connect_scroll(move |_, _, dy| {
        if dy != 0.0 {
            z(if dy < 0.0 { 1.15 } else { 1.0 / 1.15 });
        }
        glib::Propagation::Stop
    });
    scroll.add_controller(wheel);
    load();
    show();
    dialog.present(Some(&app.window_widget()));
}
