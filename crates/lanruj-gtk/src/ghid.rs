// SPDX-License-Identifier: GPL-3.0-or-later
//! Ghidul (F1): toate instrucțiunile, pe capitole, într-o fereastră peste caiet.

use crate::{App, tr};
use adw::prelude::*;
use lanruj_core::Lang;
use std::rc::Rc;

/// (titlu, text) în română și în engleză. Textul e markup Pango (<b>, <i>, <tt>).
const CAPITOLE: [(&str, &str, &str, &str); 15] = [
    (
        "Cum începi",
        "O zi obișnuită cu Lanruj:\n\n         <b>1.</b> Deschizi aplicația: ești pe ziua de azi, cu cursorul în câmpul de jos.\n         <b>2.</b> Scrii ce ai de făcut, câte un rând, și apeși Enter după fiecare: <tt>. Sună la bancă</tt>,          <tt>o 18:00 Cină cu familia</tt>, <tt>- Prima zăpadă</tt>.\n         <b>3.</b> Când termini o sarcină, dai clic pe punctul din fața ei: devine ×.\n         <b>4.</b> Ca să adaugi o notiță sub o intrare (de exemplu „frig, dar senin” sub „Plimbare”), o scrii jos,          apoi o alegi cu ↑ și apeși <b>Tab</b>: trece sub intrarea de deasupra.\n         <b>5.</b> Vrei să schimbi ceva? Dublu clic pe intrare (sau clic dreapta → Editează).\n         <b>6.</b> Seara, Lanruj îți arată ce n-ai terminat: alegi mâine, altă zi, renunț sau o las (vezi Migrarea).\n         <b>7.</b> Sub zi pui etichete și nota zilei; cu agrafa din câmpul de jos, poze.\n\n         Pentru lucruri care nu țin de o zi anume: planul săptămânii, lista lunii, Planurile pentru lunile care vin          și Indexul pentru ce vrei să găsești ușor mai târziu.",
        "Getting started",
        "A usual day with Lanruj:\n\n         <b>1.</b> Open the app: you are on today, with the cursor in the field at the bottom.\n         <b>2.</b> Write what you have to do, one line at a time, pressing Enter after each: <tt>. Call the bank</tt>,          <tt>o 18:00 Family dinner</tt>, <tt>- First snow</tt>.\n         <b>3.</b> When you finish a task, click the dot in front of it: it becomes ×.\n         <b>4.</b> To add a note under an entry (for example \"cold but clear\" under \"Walk\"), write it at the bottom,          select it with ↑ and press <b>Tab</b>: it goes under the entry above.\n         <b>5.</b> Want to change something? Double-click the entry (or right-click → Edit).\n         <b>6.</b> In the evening, Lanruj shows what you didn't finish: choose tomorrow, another day, drop or keep (see Migration).\n         <b>7.</b> Under the day add tags and the day rating; with the paperclip in the bottom field, pictures.\n\n         For things that don't belong to a day: the week's plan, the month's list, the future log for the coming months,          and the Index for what you want to find easily later.",
    ),
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
         După semn: <tt>*</tt> pentru important (★), <tt>!</tt> pentru idee, de exemplu <tt>. * Trimite raportul</tt>.\n\n\
         <b>Unde scrii</b>: în Pagina zilei, în câmpul de jos din dreapta; în Pagina săptămânii, în câmpul planului (stânga); \
         în Pagina lunii, în câmpul sarcinilor (dreapta), iar în calendar cu dublu clic pe o zi; în Planuri, fiecare lună are \
         câmpul ei, iar ziua stă la început: <tt>. 15 Revizia mașinii</tt>; în pagina unui rând din index, în câmpul din stânga.\n\n\
         <b>Notițe sub o intrare</b>: alegi intrarea cu ↑ ↓ (sau o editezi) și apeși <b>Tab</b>; trece sub cea de deasupra. \
         <b>Shift+Tab</b> o scoate înapoi. La fel din clic dreapta.",
        "Quick writing",
        "Type a line in the field at the bottom and press <b>Enter</b>. The sign at the start says what it is:\n\n\
         <tt>. </tt> task, for example <tt>. Call the bank</tt>\n\
         <tt>o </tt> event, with a time if you like: <tt>o 10:00 Dentist</tt>\n\
         <tt>- </tt> note: <tt>- Rained all day</tt>\n\
         no sign: also a note\n\n\
         After the sign: <tt>*</tt> for important (★), <tt>!</tt> for an idea, for example <tt>. * Send the report</tt>.\n\
         <b>Where to write</b>: on the Day page, in the bottom field on the right; on the Week page, in the plan's field (left); \
         on the Month page, in the tasks field (right), and in the calendar by double-clicking a day; in the future log, each month \
         has its own field, with the day first: <tt>. 15 Car service</tt>; on an index line's page, in the field on the left.\n\n\
         <b>Notes under an entry</b>: select the entry with ↑ ↓ (or edit it) and press <b>Tab</b>; it goes under the one above. \
         <b>Shift+Tab</b> takes it back out. The same from the right-click menu.",
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
        "<b>Cum editezi o intrare</b>\n\
         <b>1.</b> Dublu clic pe ea (sau o alegi cu ↑ ↓ și apeși Enter; sau clic dreapta → Editează).\n\
         <b>2.</b> Rândul devine un câmp, cu semnul de la început: <tt>. </tt> sarcină, <tt>o </tt> eveniment, <tt>- </tt> notă.\n\
         <b>3.</b> Schimbi textul. Schimbând semnul, o notă devine sarcină sau eveniment (<tt>- dentist</tt> → <tt>. dentist</tt>).\n\
         <b>4.</b> <b>Enter</b> salvează, <b>Esc</b> renunță.\n\n\
         <b>Clic dreapta</b> pe o intrare deschide meniul ei: Editează, <b>Mută…</b>, Culoare…, Adaugă poze sau fișiere…, \
         Sub intrarea de deasupra (Tab), Scoate de sub ea (Shift+Tab), Șterge. Dacă intrarea are legături, sus apar și \
         „Deschide …” / „Scrie la …”.\n\n\
         <b>Mută…</b> duce intrarea pe altă pagină: azi, mâine, planul săptămânii aceasteia sau viitoare, lista lunii aceasteia \
         sau viitoare, o lună din Planuri ori o zi anume din calendar. O sarcină lasă urma ei (&lt;); notele și evenimentele pleacă de tot, \
         cu tot ce au dedesubt.\n\n\
         <b>Șterge</b> scoate intrarea cu notițele și pozele ei; jos apare <b>Anulează</b>, dacă te-ai răzgândit.\n\n\
         <b>Din tastatură</b>: ↑ din câmpul de jos alege ultima intrare; ↑ ↓ între intrări; Enter editează; Ctrl+Enter bifează; \
         Tab / Shift+Tab mută sub / scoate; Delete șterge; Esc te întoarce în câmpul de jos.",
        "Edit, move, delete",
        "<b>How to edit an entry</b>\n\
         <b>1.</b> Double-click it (or select it with ↑ ↓ and press Enter; or right-click → Edit).\n\
         <b>2.</b> The line becomes a field, with the leading sign: <tt>. </tt> task, <tt>o </tt> event, <tt>- </tt> note.\n\
         <b>3.</b> Change the text. Changing the sign turns a note into a task or an event (<tt>- dentist</tt> → <tt>. dentist</tt>).\n\
         <b>4.</b> <b>Enter</b> saves, <b>Esc</b> cancels.\n\n\
         <b>Right-click</b> an entry for its menu: Edit, <b>Move…</b>, Color…, Add pictures or files…, \
         Under the entry above (Tab), Out from under it (Shift+Tab), Delete. If the entry has links, \"Open …\" / \"Write to …\" appear on top.\n\n\
         <b>Move…</b> takes the entry to another page: today, tomorrow, this or next week's plan, this or next month's list, \
         a future log month or a specific day. A task leaves its trace (&lt;); notes and events leave entirely, with everything under them.\n\n\
         <b>Delete</b> removes the entry with its notes and pictures; <b>Undo</b> appears at the bottom if you change your mind.\n\n\
         <b>Keyboard</b>: ↑ from the bottom field selects the last entry; ↑ ↓ between entries; Enter edits; Ctrl+Enter checks; \
         Tab / Shift+Tab indent / outdent; Delete deletes; Esc goes back to the bottom field.",
    ),
    (
        "Culori și legături",
        "<b>Cum colorezi o bucată de text</b>\n\
         <b>1.</b> Intri în editare (dublu clic pe intrare).\n\
         <b>2.</b> Selectezi cu mouse-ul (sau cu Shift+săgeți) cuvintele pe care le vrei colorate.\n\
         <b>3.</b> În bara de sub câmp apeși un <b>A</b> colorat (culoarea textului) sau un pătrat colorat (marker, ca fundal).\n\
         <b>4.</b> Enter salvează.\n\
         Fără selecție, culoarea se pune pe tot textul. Butonul cu ✕ scoate culorile de pe selecție (sau de pe tot).\n\n\
         <b>Toată intrarea deodată</b>: clic dreapta → Culoare… și alegi.\n\n\
         <b>Legături</b>\n\
         • O adresă scrisă (<tt>https://…</tt>) sau o adresă de e-mail devine singură legătură.\n\
         • Pe un cuvânt: în editare îl selectezi, apeși 🔗 din bară (sau <b>Ctrl+K</b>), lipești adresa și Enter. \
         Poți scrie scurt, <tt>exemplu.ro</tt>; Lanruj adaugă <tt>https://</tt>.\n\
         • „Scoate legătura” din aceeași fereastră o scoate de pe textul selectat.\n\
         • Clic pe o legătură o deschide în browser (sau în programul de e-mail).\n\n\
         Noaptea, culorile se luminează singure, ca să se vadă pe hârtia întunecată.",
        "Colors and links",
        "<b>How to color some text</b>\n\
         <b>1.</b> Start editing (double-click the entry).\n\
         <b>2.</b> Select the words with the mouse (or Shift+arrows).\n\
         <b>3.</b> In the bar under the field, press a colored <b>A</b> (text color) or a colored square (marker, as background).\n\
         <b>4.</b> Enter saves.\n\
         With no selection, the color goes on all the text. The ✕ button removes colors from the selection (or from everything).\n\n\
         <b>The whole entry at once</b>: right-click → Color… and pick.\n\n\
         <b>Links</b>\n\
         • A written web address (<tt>https://…</tt>) or e-mail address becomes a link by itself.\n\
         • On a word: while editing, select it, press 🔗 in the bar (or <b>Ctrl+K</b>), paste the address and press Enter. \
         You can type it short, <tt>example.com</tt>; Lanruj adds <tt>https://</tt>.\n\
         • \"Remove link\" in the same window removes it from the selected text.\n\
         • Click a link to open it in the browser (or the e-mail program).\n\n\
         In dark mode, colors brighten by themselves so they show on dark paper.",
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
        "Toate tastele sunt în <b>☰ → Scurtături de taste</b> (Ctrl+?). Cele mai folosite: Ctrl+F căutare, Ctrl+M migrare, Ctrl+I în index, Ctrl+G mergi la data, \
         Tab sub intrarea de deasupra, Ctrl+K legătură (la editare), Alt+← → zile, Alt+Home azi, F9 bara laterală, F11 fereastra mare, F10 meniul, Ctrl+Q ieșire, F1 ghidul acesta.",
        "Keys",
        "All keys are in <b>☰ → Keyboard Shortcuts</b> (Ctrl+?). The most used: Ctrl+F search, Ctrl+M migrate, Ctrl+I index, Ctrl+G go to date, \
         Tab under the entry above, Ctrl+K link (while editing), Alt+← → days, Alt+Home today, F9 sidebar, F11 maximize, F10 menu, Ctrl+Q quit, F1 this guide.",
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
    (
        "Sfaturi utile",
        "• <b>Nu trebuie să salvezi nimic</b>: fiecare schimbare se scrie pe loc. Dacă ai șters ceva din greșeală, apasă Anulează în notificarea de jos.\n\
         • <b>Mergi la data…</b> (Ctrl+G) te duce la orice zi; <b>Azi</b> (Alt+Home) te aduce înapoi.\n\
         • <b>F9</b> strânge bara laterală la iconițe (și o deschide); pe o fereastră mică se strânge singură. <b>F11</b> mărește fereastra.\n\
         • Sarcinile mutate de mai multe ori primesc <b>↻</b>; de la a treia mutare, gândește-te dacă mai merită.\n\
         • Lucrurile mari (un concediu, un proiect) pune-le în <b>Index</b>: au o pagină a lor, iar rândurile cu dată apar și în zilele lor.\n\
         • Rândurile din fișiere sunt Markdown obișnuit: le poți deschide și cu Obsidian, Typora sau orice editor de text.\n\
         • Cu <b>systray</b> pornit (Setări → Fereastra), închiderea doar ascunde fereastra; Ctrl+Q oprește aplicația.\n\
         • Totul despre taste: Ctrl+? (Scurtături de taste).",
        "Useful tips",
        "• <b>No need to save</b>: every change is written at once. If you deleted something by mistake, press Undo in the notification at the bottom.\n\
         • <b>Go to date…</b> (Ctrl+G) takes you to any day; <b>Today</b> (Alt+Home) brings you back.\n\
         • <b>F9</b> collapses the sidebar to icons (and back); on a small window it collapses by itself. <b>F11</b> maximizes the window.\n\
         • Tasks moved several times get <b>↻</b>; from the third move on, think whether it's still worth it.\n\
         • Put big things (a holiday, a project) in the <b>Index</b>: they get a page of their own, and dated lines show on their days.\n\
         • The lines in the files are plain Markdown: you can open them with Obsidian, Typora or any text editor.\n\
         • With the <b>system tray</b> on (Preferences → Window), closing only hides the window; Ctrl+Q quits the app.\n\
         • Everything about keys: Ctrl+? (Keyboard Shortcuts).",
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
