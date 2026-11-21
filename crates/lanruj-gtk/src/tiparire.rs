// SPDX-License-Identifier: GPL-3.0-or-later
//! Monitorizarea pe hârtie: două foi A4 desenate special (nu o captură de ecran).
//! Foaia 1: activitățile și somnul, pe zile, cu graficul somnului. Foaia 2: serviciul,
//! cu calendarul, totalurile și notele lunii. Zilele care n-au venit rămân goale,
//! ca foaia să se poată completa de mână.

use crate::{App, tr};
use adw::prelude::*;
use chrono::{Datelike, Duration, NaiveDate};
use gtk::cairo;
use gtk::pango;
use lanruj_core::Lang;
use lanruj_core::monitor::{Shift, Tracking, WorkOnDay, duration, seconds_text};
use std::path::Path;
use std::rc::Rc;

/// A4 în puncte.
pub const W: f64 = 595.0;
pub const H: f64 = 842.0;
const M: f64 = 36.0;
const INK: (f64, f64, f64) = (0.12, 0.23, 0.37);
const MUTED: (f64, f64, f64) = (0.42, 0.47, 0.53);
const OCHRE: (f64, f64, f64) = (0.79, 0.54, 0.07);
const RED: (f64, f64, f64) = (0.70, 0.26, 0.18);
const LINE: (f64, f64, f64) = (0.80, 0.78, 0.74);

pub struct Sheet {
    lang: Lang,
    y: i32,
    m: u32,
    t: Tracking,
}

fn text(cr: &cairo::Context, x: f64, y: f64, s: &str, size: f64, bold: bool, color: (f64, f64, f64), width: Option<f64>, center: bool) -> f64 {
    let layout = pangocairo::functions::create_layout(cr);
    let mut fd = pango::FontDescription::from_string("Sans");
    fd.set_absolute_size(size * pango::SCALE as f64);
    if bold {
        fd.set_weight(pango::Weight::Bold);
    }
    layout.set_font_description(Some(&fd));
    if let Some(w) = width {
        layout.set_width((w * pango::SCALE as f64) as i32);
        layout.set_wrap(pango::WrapMode::WordChar);
    }
    if center {
        layout.set_alignment(pango::Alignment::Center);
    }
    layout.set_text(s);
    cr.set_source_rgb(color.0, color.1, color.2);
    cr.move_to(x, y);
    pangocairo::functions::show_layout(cr, &layout);
    layout.pixel_size().1 as f64
}

fn line(cr: &cairo::Context, x1: f64, y1: f64, x2: f64, y2: f64, c: (f64, f64, f64), w: f64) {
    cr.set_source_rgb(c.0, c.1, c.2);
    cr.set_line_width(w);
    cr.move_to(x1, y1);
    cr.line_to(x2, y2);
    let _ = cr.stroke();
}

impl Sheet {
    pub fn new(app: &Rc<App>, y: i32, m: u32) -> Sheet {
        let t = app.journal.load_tracking(y, m).unwrap_or_default();
        Sheet { lang: app.lang, y, m, t }
    }

    pub const PAGES: i32 = 2;

    fn days(&self) -> Vec<NaiveDate> {
        let mut d = NaiveDate::from_ymd_opt(self.y, self.m, 1).expect("lună validă");
        let mut out = Vec::new();
        while d.month() == self.m {
            out.push(d);
            d += Duration::days(1);
        }
        out
    }

    fn head(&self, cr: &cairo::Context, what: &str) -> f64 {
        let title = self.lang.month_title(self.y, self.m);
        text(cr, M, M, &format!("{} · {what}", tr(self.lang, "Monitorizare", "Tracking")).to_uppercase(), 8.0, true, MUTED, None, false);
        text(cr, M, M + 12.0, &title, 18.0, true, INK, None, false);
        line(cr, M, M + 40.0, W - M, M + 40.0, INK, 1.2);
        M + 50.0
    }

    /// Desenează foaia `n` (0 sau 1) pe o suprafață de W × H puncte.
    pub fn draw(&self, cr: &cairo::Context, n: i32) {
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().ok();
        if n == 0 { self.activities_and_sleep(cr) } else { self.work(cr) }
        text(cr, M, H - M + 8.0, "Lanruj", 7.0, false, MUTED, None, false);
    }

    fn activities_and_sleep(&self, cr: &cairo::Context) {
        let lang = self.lang;
        let top = self.head(cr, tr(lang, "Activități și somn", "Activities and sleep"));
        let acts = &self.t.activities;
        let days = self.days();
        // Coloanele: ziua, activitățile, somnul (noaptea, ziua).
        let day_w = 44.0;
        let sleep_w = 46.0;
        let avail = W - 2.0 * M - day_w - 2.0 * sleep_w;
        let act_w = if acts.is_empty() { 0.0 } else { (avail / acts.len() as f64).min(70.0) };
        let head_h = 30.0;
        let row_h = 15.0;
        let mut x = M + day_w;
        for a in acts {
            let name = match (&a.unit, a.timed) {
                (_, true) => format!("{}\n(min:s)", a.name),
                (Some(u), _) => format!("{}\n({u})", a.name),
                _ => a.name.clone(),
            };
            text(cr, x + 2.0, top, &name, 7.5, true, INK, Some(act_w - 4.0), true);
            x += act_w;
        }
        text(cr, x + 2.0, top, &format!("{}\n{}", tr(lang, "Somn", "Sleep"), tr(lang, "noaptea", "night")), 7.5, true, INK, Some(sleep_w - 4.0), true);
        text(cr, x + sleep_w + 2.0, top, &format!("{}\n{}", tr(lang, "Somn", "Sleep"), tr(lang, "ziua", "day")), 7.5, true, INK, Some(sleep_w - 4.0), true);
        let table_right = x + 2.0 * sleep_w;
        let mut y = top + head_h;
        line(cr, M, y, table_right, y, INK, 0.8);
        let names = match lang {
            Lang::Ro => ["Lu", "Ma", "Mi", "Jo", "Vi", "Sâ", "Du"],
            Lang::En => ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
        };
        for d in &days {
            let weekend = d.weekday().num_days_from_monday() >= 5;
            if weekend {
                cr.set_source_rgb(0.97, 0.95, 0.91);
                cr.rectangle(M, y, table_right - M, row_h);
                let _ = cr.fill();
            }
            let wd = names[d.weekday().num_days_from_monday() as usize];
            text(cr, M + 2.0, y + 2.5, &format!("{wd} {}", d.day()), 8.0, false, if weekend { RED } else { INK }, None, false);
            let done = self.t.done.get(&d.day());
            let mut x = M + day_w;
            for a in acts {
                let v = done.and_then(|s| s.get(&a.name)).copied().unwrap_or(0);
                if v > 0 {
                    let s = if a.counted { a.value_text(v) } else { "✓".to_string() };
                    text(cr, x, y + 2.0, &s, 8.5, !a.counted, INK, Some(act_w), true);
                }
                line(cr, x, y, x, y + row_h, LINE, 0.4);
                x += act_w;
            }
            let s = self.t.sleep.get(&d.day()).copied().unwrap_or_default();
            for (i, v) in [s.night, s.day].into_iter().enumerate() {
                let sx = x + i as f64 * sleep_w;
                line(cr, sx, y, sx, y + row_h, LINE, 0.4);
                if let Some(v) = v {
                    text(cr, sx, y + 2.0, &duration(v), 8.0, false, INK, Some(sleep_w), true);
                }
            }
            y += row_h;
            line(cr, M, y, table_right, y, LINE, 0.4);
        }
        // Totalurile activităților, sub coloane.
        let mut x = M + day_w;
        text(cr, M + 2.0, y + 3.0, tr(lang, "Total", "Total"), 7.5, true, INK, None, false);
        for (a, n, sum) in self.t.activity_counts() {
            let s = if a.timed {
                seconds_text(sum)
            } else if a.counted {
                sum.to_string()
            } else {
                match lang {
                    Lang::Ro => format!("{n} zile"),
                    Lang::En => format!("{n} days"),
                }
            };
            text(cr, x, y + 3.0, &s, 7.5, true, INK, Some(act_w), true);
            x += act_w;
        }
        let (night, day) = self.t.sleep_average();
        let avg = |v: Option<u32>| v.map(duration).unwrap_or_default();
        text(cr, x, y + 3.0, &format!("⌀ {}", avg(night)), 7.5, true, INK, Some(sleep_w), true);
        text(cr, x + sleep_w, y + 3.0, &format!("⌀ {}", avg(day)), 7.5, true, INK, Some(sleep_w), true);
        y += 22.0;

        // Graficul somnului.
        let chart_top = y + 20.0;
        let chart_h = (H - M - 20.0 - chart_top).clamp(60.0, 150.0);
        text(cr, M, chart_top - 24.0, tr(lang, "Somnul lunii (noaptea jos, ziua deasupra)", "Sleep this month (night below, day above)"), 8.0, true, MUTED, None, false);
        let left = M + 22.0;
        let width = W - M - left;
        let max = 12.0 * 60.0;
        let y_of = |min: f64| chart_top + chart_h * (1.0 - (min / max).min(1.0));
        for hrs in [4.0, 8.0, 12.0] {
            let yy = y_of(hrs * 60.0);
            line(cr, left, yy, W - M, yy, LINE, 0.5);
            text(cr, M, yy - 5.0, &format!("{hrs:.0}h"), 7.0, false, MUTED, None, false);
        }
        let slot = width / days.len() as f64;
        let bar = slot * 0.65;
        for (i, d) in days.iter().enumerate() {
            let s = self.t.sleep.get(&d.day()).copied().unwrap_or_default();
            let (n, z) = (s.night.unwrap_or(0) as f64, s.day.unwrap_or(0) as f64);
            let x = left + slot * i as f64 + (slot - bar) / 2.0;
            cr.set_source_rgb(INK.0, INK.1, INK.2);
            cr.rectangle(x, y_of(n), bar, y_of(0.0) - y_of(n));
            let _ = cr.fill();
            if z > 0.0 {
                cr.set_source_rgb(OCHRE.0, OCHRE.1, OCHRE.2);
                cr.rectangle(x, y_of(n + z), bar, y_of(n) - y_of(n + z));
                let _ = cr.fill();
            }
            if d.day() == 1 || d.day() % 5 == 0 {
                text(cr, x - 2.0, y_of(0.0) + 2.0, &d.day().to_string(), 6.5, false, MUTED, None, false);
            }
        }
        line(cr, left, y_of(0.0), W - M, y_of(0.0), INK, 0.6);
    }

    fn work(&self, cr: &cairo::Context) {
        let lang = self.lang;
        let mut y = self.head(cr, tr(lang, "Serviciu", "Work"));
        // Calendarul: 7 coloane, câte o căsuță pe zi.
        let cw = (W - 2.0 * M) / 7.0;
        let ch = 52.0;
        let names = match lang {
            Lang::Ro => ["Luni", "Marți", "Miercuri", "Joi", "Vineri", "Sâmbătă", "Duminică"],
            Lang::En => ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
        };
        for (i, n) in names.iter().enumerate() {
            text(cr, M + cw * i as f64, y, n, 7.5, true, if i >= 5 { RED } else { MUTED }, Some(cw), true);
        }
        y += 14.0;
        let days = self.days();
        let offset = days[0].weekday().num_days_from_monday() as usize;
        let rows = (offset + days.len()).div_ceil(7);
        for (k, d) in days.iter().enumerate() {
            let i = offset + k;
            let (cx, cy) = (M + cw * (i % 7) as f64, y + ch * (i / 7) as f64);
            let items = self.t.work_on(d.day());
            let fill = items.iter().find_map(|it| match it {
                WorkOnDay::Day(Shift::Service, _) => Some((0.89, 0.91, 0.95)),
                WorkOnDay::Day(Shift::Reserve, _) => Some((0.93, 0.90, 0.96)),
                WorkOnDay::Day(Shift::Leave, _) => Some((0.90, 0.95, 0.91)),
                _ => None,
            });
            if let Some(c) = fill {
                cr.set_source_rgb(c.0, c.1, c.2);
                cr.rectangle(cx, cy, cw, ch);
                let _ = cr.fill();
            }
            cr.set_source_rgb(LINE.0, LINE.1, LINE.2);
            cr.set_line_width(0.5);
            cr.rectangle(cx, cy, cw, ch);
            let _ = cr.stroke();
            text(cr, cx + 4.0, cy + 3.0, &d.day().to_string(), 9.0, true, if i % 7 >= 5 { RED } else { INK }, None, false);
            let mut ty = cy + 17.0;
            for it in &items {
                let (s, c) = match it {
                    WorkOnDay::Day(Shift::Service, _) => (tr(lang, "Serviciu", "Duty").to_string(), INK),
                    WorkOnDay::Day(Shift::Reserve, _) => (tr(lang, "Rezervă", "Standby").to_string(), (0.48, 0.31, 0.63)),
                    WorkOnDay::Day(Shift::Leave, _) => (tr(lang, "Concediu", "Leave").to_string(), (0.18, 0.55, 0.34)),
                    WorkOnDay::Overtime(span, _) => (format!("+{}", span.text()), OCHRE),
                };
                ty += text(cr, cx + 4.0, ty, &s, 7.5, true, c, Some(cw - 8.0), false);
            }
        }
        y += ch * rows as f64 + 16.0;

        // Totalurile.
        let tot = self.t.totals();
        let h = |m: u32| if m % 60 == 0 { format!("{} h", m / 60) } else { format!("{} h {} min", m / 60, m % 60) };
        let rows = [
            (tr(lang, "Servicii", "Duties"), format!("{} ({} h)", tot.shifts, tot.shifts * Shift::HOURS)),
            (tr(lang, "Rezerve", "Standbys"), tot.reserve.to_string()),
            (tr(lang, "Ore suplimentare", "Overtime"), h(tot.overtime)),
            (tr(lang, "Zile de concediu", "Days of leave"), tot.leave.to_string()),
            (tr(lang, "Total ore lucrate", "Total hours worked"), h(tot.worked())),
        ];
        let half = (W - 2.0 * M) / 2.0;
        for (i, (k, v)) in rows.iter().enumerate() {
            let (x, yy) = (M + half * (i % 2) as f64, y + 16.0 * (i / 2) as f64);
            text(cr, x, yy, k, 9.0, false, INK, None, false);
            text(cr, x + half - 110.0, yy, v, 9.0, true, INK, Some(100.0), true);
        }
        y += 16.0 * 3.0 + 10.0;

        // Notele lunii.
        let notes = self.t.work_notes_list();
        if !notes.is_empty() {
            line(cr, M, y, W - M, y, INK, 0.8);
            y += 6.0;
            text(cr, M, y, tr(lang, "Notele lunii", "This month's notes"), 9.0, true, MUTED, None, false);
            y += 16.0;
            for (d, it) in notes {
                if y > H - M - 20.0 {
                    break;
                }
                let (what, note) = match it {
                    WorkOnDay::Day(Shift::Service, n) => (tr(lang, "Serviciu", "Duty").to_string(), n),
                    WorkOnDay::Day(Shift::Reserve, n) => (tr(lang, "Rezervă", "Standby").to_string(), n),
                    WorkOnDay::Day(Shift::Leave, n) => (tr(lang, "Concediu", "Leave").to_string(), n),
                    WorkOnDay::Overtime(span, n) => (format!("{} {}", tr(lang, "Ore suplimentare", "Overtime"), span.text()), n),
                };
                text(cr, M, y, &format!("{d} · {what}"), 8.5, true, INK, Some(150.0), false);
                y += text(cr, M + 160.0, y, &note, 8.5, false, INK, Some(W - 2.0 * M - 160.0), false).max(12.0) + 4.0;
            }
        }
    }

    /// Scrie foile într-un PDF.
    pub fn write_pdf(&self, path: &Path) -> Result<(), String> {
        let surface = cairo::PdfSurface::new(W, H, path).map_err(|e| e.to_string())?;
        let cr = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
        for n in 0..Self::PAGES {
            self.draw(&cr, n);
            cr.show_page().map_err(|e| e.to_string())?;
        }
        surface.finish();
        Ok(())
    }
}

impl App {
    /// Monitorizarea lunii, la tipar (fereastra de tipărire a sistemului).
    pub fn print_tracking(self: &Rc<Self>, y: i32, m: u32) {
        let sheet = Rc::new(Sheet::new(self, y, m));
        let op = gtk::PrintOperation::new();
        op.set_job_name("Lanruj");
        op.set_n_pages(Sheet::PAGES);
        op.set_use_full_page(false);
        let s = sheet.clone();
        op.connect_draw_page(move |_, ctx, n| {
            let cr = ctx.cairo_context();
            let scale = (ctx.width() / W).min(ctx.height() / H);
            cr.save().ok();
            cr.scale(scale, scale);
            s.draw(&cr, n);
            cr.restore().ok();
        });
        if let Err(e) = op.run(gtk::PrintOperationAction::PrintDialog, Some(&self.window)) {
            self.error(&std::io::Error::other(e.to_string()));
        }
    }
}
