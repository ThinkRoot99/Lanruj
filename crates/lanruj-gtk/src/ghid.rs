// SPDX-License-Identifier: GPL-3.0-or-later
//! Ghidul (F1): toate instrucțiunile, pe capitole, într-o fereastră peste caiet.

use crate::{App, tr};
use adw::prelude::*;
use lanruj_core::Lang;
use std::rc::Rc;

/// (titlu, text) în română și în engleză. Textul e markup Pango (<b>, <i>, <tt>).
const CAPITOLE: [(&str, &str, &str, &str); 13] = [
    (
        "Pe scurt",
        "Lanruj e un jurnal în stilul metodei Bullet Journal: scrii scurt, cu semne, și muți ce n-ai terminat.\n\n\
         • <b>Pagina zilei</b> - ce faci azi; alături, ziua de ieri.\n\
         • <b>Pagina săptămânii</b> - planul săptămânii și cele 7 zile pe scurt.\n\
         • <b>Pagina lunii</b> - calendarul lunii și sarcinile ei.\n\
         • <b>Planuri</b> - lunile care vin.\n\
         • <b>Index</b> - zilele și lucrurile de care vrei să-ți amintești.\n\n\
         Totul se salvează imediat, în fișiere Markdown obișnuite.",
        "Short version",
        "Lanruj is a journal in the Bullet Journal style: write short, with signs, and move what you didn't finish.\n\n\
         • <b>Day</b> - what you do today; next to it, yesterday.\n\
         • <b>Week</b> - the week's plan and the 7 days at a glance.\n\
         • <b>Month</b> - the month's calendar and tasks.\n\
         • <b>Future log</b> - the coming months.\n\
         • <b>Index</b> - the days and things you want to remember.\n\n\
         Everything is saved at once, in plain Markdown files.",
    ),
    (
        "Scrierea rapidă",
        "În câmpul de jos scrii un rând și apeși <b>Enter</b>. Semnul de la început spune ce e:\n\n\
         <tt>. </tt> sarcină, de exemplu <tt>. Sună la bancă</tt>\n\
         <tt>o </tt> eveniment, cu ora dacă vrei: <tt>o 10:00 Dentist</tt>\n\
         <tt>- </tt> notă: <tt>- A plouat toată ziua</tt>\n\
         fără semn: tot notă\n\n\
         După semn: <tt>*</tt> pentru important (★), <tt>!</tt> pentru idee, de exemplu <tt>. * Trimite raportul</tt>.\n\
         În Planuri, ziua stă la început: <tt>. 15 Revizia mașinii</tt>.",
        "Quick writing",
        "Type a line in the field at the bottom and press <b>Enter</b>. The sign at the start says what it is:\n\n\
         <tt>. </tt> task, for example <tt>. Call the bank</tt>\n\
         <tt>o </tt> event, with a time if you like: <tt>o 10:00 Dentist</tt>\n\
         <tt>- </tt> note: <tt>- Rained all day</tt>\n\
         no sign: also a note\n\n\
         After the sign: <tt>*</tt> for important (★), <tt>!</tt> for an idea, for example <tt>. * Send the report</tt>.\n\
         In the future log, the day goes first: <tt>. 15 Car service</tt>.",
    ),
    (
        "Semnele",
        "• sarcină de făcut   × făcută   ○ eveniment   – notă\n\
         &gt; mutată pe pagina următoare   &lt; programată în altă parte   <s>tăiată</s> renunțată\n\
         ★ importantă   ! idee\n\n\
         Un clic pe semnul unei sarcini o bifează. <b>↻2</b> arată de câte ori a fost mutată; de la a treia \
         mutare apare <b>„mai merită?”</b>, ca să nu amâni la nesfârșit.",
        "Signs",
        "• task to do   × done   ○ event   – note\n\
         &gt; moved to the next page   &lt; scheduled elsewhere   <s>crossed out</s> dropped\n\
         ★ important   ! idea\n\n\
         Click a task's sign to check it. <b>↻2</b> shows how many times it was moved; from the third move on, \
         <b>\"still worth it?\"</b> appears, so you don't postpone forever.",
    ),
    (
        "Paginile",
        "<b>Pagina zilei</b>: săgețile din antet (Alt+← →) merg de la o zi la alta; <b>Azi</b> (Alt+Home) te aduce înapoi. \
         Calendarul mic din bara laterală deschide orice zi; zilele scrise sunt colorate.\n\n\
         <b>Pagina săptămânii</b>: în stânga planul (ce vrei să faci săptămâna asta), în dreapta zilele pe scurt; clic pe o zi o deschide.\n\n\
         <b>Pagina lunii</b>: calendarul (dublu clic pe o zi ca să scrii ce e atunci) și sarcinile lunii.\n\n\
         <b>Planuri</b>: lunile care vin, două câte două. Pe 1 ale lunii, ce ai scris acolo trece singur în pagina lunii.\n\n\
         <b>Index</b>: rândurile tale (+ Adaugă în index sau Ctrl+I din orice zi) și, automat, lunile și etichetele. \
         Fiecare rând din index are pagina lui (de exemplu „Concediu”): ce scrii acolo, iar rândurile care încep cu o dată \
         (<tt>26.11 Delfinariu</tt>) apar și în ziua aceea. Pe zilele din interval apare banda „Concediu · ziua 1 din 8”.",
        "Pages",
        "<b>Day</b>: the arrows in the header (Alt+← →) go from day to day; <b>Today</b> (Alt+Home) brings you back. \
         The small calendar in the sidebar opens any day; written days are colored.\n\n\
         <b>Week</b>: the plan on the left (what you want to do this week), the days at a glance on the right; click a day to open it.\n\n\
         <b>Month</b>: the calendar (double-click a day to write what's on) and the month's tasks.\n\n\
         <b>Future log</b>: the coming months, two at a time. On the 1st, what you wrote there moves to the month's page.\n\n\
         <b>Index</b>: your lines (+ Add to index, or Ctrl+I from any day) and, automatically, months and tags. \
         Each index line has its own page (for example \"Holiday\"); lines there that start with a date \
         (<tt>26.11 Dolphinarium</tt>) also show on that day, and the days in range show \"Holiday · day 1 of 8\".",
    ),
    (
        "Migrarea",
        "Seara (de la ora din setări, implicit 21:00) Lanruj îți arată pe rând sarcinile neterminate ale zilei. Pentru fiecare alegi:\n\n\
         <b>M</b> sau Enter - mâine   <b>P</b> - programează (o zi, lista unei luni, Planuri, planul săptămânii)\n\
         <b>R</b> - renunț   <b>L</b> - o las azi   ↑ ↓ - altă sarcină   Esc - mai târziu\n\n\
         La fel, la început de lună pentru lista lunii și lunea pentru planul săptămânii. Oricând: butonul <b>Migrează</b> (Ctrl+M). \
         Te poți răzgândi: alegi din nou și copia de pe pagina nouă dispare.",
        "Migration",
        "In the evening (from the hour in preferences, 9 pm by default) Lanruj shows the day's unfinished tasks one by one. For each you choose:\n\n\
         <b>M</b> or Enter - tomorrow   <b>P</b> - schedule (a day, a month's list, the future log, a week's plan)\n\
         <b>R</b> - drop   <b>L</b> - keep today   ↑ ↓ - another task   Esc - later\n\n\
         The same at the start of a month for its list, and on Mondays for the week's plan. Any time: the <b>Migrate</b> button (Ctrl+M). \
         You can change your mind: choose again and the copy on the new page goes away.",
    ),
    (
        "Editare, mutare, ștergere",
        "<b>Dublu clic</b> pe o intrare (sau ↑ ↓ și Enter) o editează; Enter salvează, Esc renunță. \
         Schimbând semnul de la început, o notă devine sarcină sau eveniment.\n\n\
         <b>Clic dreapta</b> pe o intrare: Editează, <b>Mută…</b> (azi, mâine, lista lunii, planul săptămânii, Planuri, o zi anume), \
         Culoare…, Adaugă poze sau fișiere…, Șterge (cu <b>Anulează</b> în notificare).\n\n\
         Din tastatură: ↑ din câmpul de jos alege ultima intrare; Ctrl+Enter bifează; Delete șterge.",
        "Edit, move, delete",
        "<b>Double-click</b> an entry (or ↑ ↓ and Enter) to edit it; Enter saves, Esc cancels. \
         Changing the leading sign turns a note into a task or an event.\n\n\
         <b>Right-click</b> an entry: Edit, <b>Move…</b> (today, tomorrow, a month's list, a week's plan, future log, a specific day), \
         Color…, Add pictures or files…, Delete (with <b>Undo</b> in the notification).\n\n\
         From the keyboard: ↑ from the bottom field selects the last entry; Ctrl+Enter checks; Delete deletes.",
    ),
    (
        "Culori și legături",
        "La editare, sub câmp apare bara de culori: selectezi o bucată de text și alegi <b>A</b> colorat (textul) sau un pătrat (marker). \
         Fără selecție se colorează tot textul; butonul cu ✕ scoate culorile. Clic dreapta → Culoare… colorează toată intrarea.\n\n\
         <b>Legături</b>: o adresă scrisă (<tt>https://…</tt>) sau de e-mail devine singură legătură. \
         Pe un cuvânt: selectezi și apeși 🔗 (Ctrl+K), apoi lipești adresa. Clic pe legătură o deschide.",
        "Colors and links",
        "While editing, the color bar appears under the field: select some text and pick a colored <b>A</b> (text) or a square (marker). \
         With no selection, all the text is colored; the ✕ button removes colors. Right-click → Color… colors the whole entry.\n\n\
         <b>Links</b>: a web address (<tt>https://…</tt>) or an e-mail address becomes a link by itself. \
         On a word: select it, press 🔗 (Ctrl+K) and paste the address. Click a link to open it.",
    ),
    (
        "Poze și fișiere",
        "Agrafa din câmpul de jos adaugă poze și fișiere; le poți și trage în pagină. Trase peste o intrare (sau clic dreapta pe ea → \
         Adaugă poze…) se leagă de ea și o urmează când o muți.\n\n\
         Clic dreapta pe o poză: numele, mărimea (mică, medie, mare), Leagă de…, Scoate din pagină. \
         Mai multe poze una după alta se arată ca galerie. Clic pe o poză o mărește: + − sau rotița pentru zoom, ← → pentru celelalte, Esc închide.\n\n\
         Pozele mari se micșorează la copiere; originalul tău rămâne neatins.",
        "Pictures and files",
        "The paperclip in the bottom field adds pictures and files; you can also drag them onto the page. Dropped on an entry \
         (or right-click it → Add pictures…) they belong to it and follow it when you move it.\n\n\
         Right-click a picture: name, size (small, medium, large), Link to…, Remove from the page. \
         Several pictures in a row show as a gallery. Click a picture to enlarge it: + − or the wheel to zoom, ← → for the others, Esc closes.\n\n\
         Big pictures are scaled down when copied; your original stays untouched.",
    ),
    (
        "Etichete și nota zilei",
        "Sub fiecare zi: <b>+ etichetă</b> (mai multe, despărțite prin virgulă; clic pe una o scoate) și <b>nota zilei</b>, \
         cinci puncte de la 1 la 5. Etichetele apar în Index, fiecare cu zilele ei.",
        "Tags and day rating",
        "Under each day: <b>+ tag</b> (several, comma-separated; click one to remove it) and the <b>day rating</b>, \
         five dots from 1 to 5. Tags appear in the Index, each with its days.",
    ),
    (
        "Căutarea",
        "<b>Ctrl+F</b> sau lupa caută în tot jurnalul: zile, liste, planuri, pagini din index, etichete și calendarul lunilor. \
         Literele mari și diacriticele nu contează („sedinta” găsește „Ședință”). Filtrele aleg sarcinile, evenimentele, notițele, ★ sau !. \
         Clic pe un rezultat deschide pagina lui; Esc închide căutarea.",
        "Search",
        "<b>Ctrl+F</b> or the magnifier searches the whole journal: days, lists, plans, index pages, tags and month calendars. \
         Case and accents don't matter. Filters pick tasks, events, notes, ★ or !. \
         Click a result to open its page; Esc closes the search.",
    ),
    (
        "Tastele",
        "Toate tastele sunt în <b>☰ → Scurtături de taste</b> (Ctrl+?). Cele mai folosite: Ctrl+F căutare, Ctrl+M migrare, Ctrl+I în index, \
         Alt+← → zile, Alt+Home azi, F9 bara laterală, F11 fereastra mare, F10 meniul, Ctrl+Q ieșire, F1 ghidul acesta.",
        "Keys",
        "All keys are in <b>☰ → Keyboard Shortcuts</b> (Ctrl+?). The most used: Ctrl+F search, Ctrl+M migrate, Ctrl+I index, \
         Alt+← → days, Alt+Home today, F9 sidebar, F11 maximize, F10 menu, Ctrl+Q quit, F1 this guide.",
    ),
    (
        "Unde stă jurnalul",
        "Totul stă în dosarul ascuns <tt>~/.lanruj</tt>, în fișiere Markdown pe care le poți citi și fără aplicație: \
         <tt>2026/11/2026-11-21.md</tt> (o zi), <tt>2026/11/luna.md</tt>, <tt>viitor.md</tt>, <tt>index.md</tt>, \
         <tt>pagini/</tt>, iar pozele lângă ziua lor.\n\n\
         <b>Pe mai multe calculatoare</b>: instalează Syncthing pe fiecare, leagă calculatoarele între ele (Adaugă dispozitiv, cu codul lui) \
         și partajează dosarul <tt>~/.lanruj</tt>. Se trimite doar ce e nou.\n\n\
         <b>Copii de siguranță</b>: copiază dosarul <tt>~/.lanruj</tt> pe alt disc sau stick.",
        "Where the journal lives",
        "Everything lives in the hidden folder <tt>~/.lanruj</tt>, in Markdown files you can read without the app: \
         <tt>2026/11/2026-11-21.md</tt> (a day), <tt>2026/11/luna.md</tt>, <tt>viitor.md</tt>, <tt>index.md</tt>, \
         <tt>pagini/</tt>, with pictures next to their day.\n\n\
         <b>On several computers</b>: install Syncthing on each, link the computers (Add device, with its ID) \
         and share the folder <tt>~/.lanruj</tt>. Only what's new is sent.\n\n\
         <b>Backups</b>: copy the <tt>~/.lanruj</tt> folder to another disk or a USB stick.",
    ),
    (
        "Setările",
        "<b>☰ → Setări</b> (Ctrl+,): modelul și culoarea hârtiei, noaptea, migrarea (singură sau doar cu butonul, ora de seară), \
         limba (fereastra se redeschide) și systray-ul (la închidere, Lanruj rămâne în bara de sus; Ctrl+Q îl oprește).",
        "Preferences",
        "<b>☰ → Preferences</b> (Ctrl+,): paper pattern and color, dark mode, migration (automatic or only with the button, evening hour), \
         language (the window reopens) and the system tray (when closed, Lanruj stays in the top bar; Ctrl+Q quits).",
    ),
];

pub fn show(app: &Rc<App>) {
    let lang = app.lang;
    let dialog = adw::Dialog::builder().content_width(760).content_height(700).title(tr(lang, "Ghid", "Guide")).build();
    let header = adw::HeaderBar::new();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    body.set_margin_top(12);
    body.set_margin_bottom(24);
    body.set_margin_start(18);
    body.set_margin_end(18);
    // Cuprinsul: un buton pentru fiecare capitol.
    let toc = adw::WrapBox::builder().child_spacing(6).line_spacing(6).build();
    body.append(&toc);
    let mut heads: Vec<gtk::Label> = Vec::new();
    for (tro, bro, ten, ben) in CAPITOLE {
        let (title, text) = if lang == Lang::Ro { (tro, bro) } else { (ten, ben) };
        let h = gtk::Label::builder().label(title).xalign(0.0).css_classes(["title-3"]).build();
        let t = gtk::Label::builder().label(text).use_markup(true).wrap(true).xalign(0.0).selectable(true).css_classes(["ghid-text"]).build();
        let sec = gtk::Box::new(gtk::Orientation::Vertical, 6);
        sec.append(&h);
        sec.append(&t);
        body.append(&sec);
        heads.push(h);
    }
    let clamp = adw::Clamp::builder().maximum_size(700).child(&body).build();
    let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&clamp).build();
    for (i, (tro, _, ten, _)) in CAPITOLE.iter().enumerate() {
        let b = gtk::Button::builder().label(if lang == Lang::Ro { *tro } else { *ten }).css_classes(["ghid-capitol"]).build();
        let (sc, h) = (scroll.clone(), heads[i].clone());
        b.connect_clicked(move |_| {
            // Derulează până la titlul capitolului.
            if let Some(p) = h.compute_point(&sc.child().expect("conținut"), &gtk::graphene::Point::new(0.0, 0.0)) {
                sc.vadjustment().set_value((p.y() as f64 - 8.0).max(0.0));
            }
        });
        toc.append(&b);
    }
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    dialog.set_child(Some(&view));
    dialog.present(Some(&app.window_widget()));
}
