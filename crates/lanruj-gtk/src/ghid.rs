// SPDX-License-Identifier: GPL-3.0-or-later
//! Ghidul (F1): toate instrucțiunile, pe capitole, într-o fereastră peste caiet.

use crate::{App, tr};
use adw::prelude::*;
use lanruj_core::Lang;
use std::rc::Rc;

/// (titlu, text) în română și în engleză. Textul e markup Pango (<b>, <i>, <tt>).
const CAPITOLE: [(&str, &str, &str, &str); 21] = [
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
         mutare (se poate schimba în Setări) apare <b>„mai merită?”</b>, ca să nu amâni la nesfârșit.",
        "Signs",
        "• task to do   × done   ○ event   – note\n\
         &gt; moved to the next page   &lt; scheduled elsewhere   <s>crossed out</s> dropped\n\
         ★ important   ! idea\n\n\
         Click a task's sign to check it. <b>↻2</b> shows how many times it was moved; from the third move on (changeable in Preferences), \
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
        "Indexul",
        "<b>Adaugi</b> un rând cu <b>+ Adaugă în index</b> sau, din orice zi, cu <b>Ctrl+I</b>: scrii titlul (de exemplu „Concediu”), \
         alegi ziua și, dacă bifezi „Mai multe zile”, ziua de sfârșit.\n\
         <b>Clic</b> pe rând deschide <b>pagina lui</b>: în stânga scrii ce ține de el (ce vizitezi, unde te cazezi), în dreapta vezi zilele lui pe scurt.\n\
         <b>Clic dreapta</b> pe rând: <b>Editează</b> (titlul și zilele) sau <b>Scoate din index</b>.\n\
         Rândurile din pagina lui care încep cu o dată (<tt>26.11 Delfinariu</tt>, <tt>26.11.2026 …</tt> sau doar <tt>26 …</tt>) \
         apar și în ziua aceea, sub banda „Concediu · ziua 4 din 8”.\n\
         În dreapta Indexului, automat: lunile cu zilele scrise și etichetele cu zilele lor.\n\n\
         <b>Calendarul lunii</b> (în Pagina lunii): dublu clic pe o zi și scrii ce e atunci; mai multe lucruri pe aceeași zi \
         le desparți cu <tt>;</tt>; dacă golești textul, rândul zilei dispare. Clic pe număr deschide ziua.",
        "The Index",
        "<b>Add</b> a line with <b>+ Add to index</b> or, from any day, with <b>Ctrl+I</b>: type the title (for example \"Holiday\"), \
         pick the day and, if you tick \"Several days\", the last day.\n\
         <b>Click</b> the line to open <b>its page</b>: on the left write what belongs to it (what you visit, where you stay), on the right see its days.\n\
         <b>Right-click</b> the line: <b>Edit</b> (title and days) or <b>Remove from index</b>.\n\
         Lines on its page that start with a date (<tt>26.11 Dolphinarium</tt>, <tt>26.11.2026 …</tt> or just <tt>26 …</tt>) \
         also show on that day, under the band \"Holiday · day 4 of 8\".\n\
         On the right of the Index, automatically: months with written days and tags with their days.\n\n\
         <b>Month calendar</b> (Month page): double-click a day and write what's on; separate several things on the same day \
         with <tt>;</tt>; empty the text to remove the day's line. Click the number to open the day.",
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
        "Monitorizarea",
        "<b>Monitorizare</b>, din bara laterală, are trei file, lună de lună. Săgețile din antet (sau Alt+← →) schimbă luna.\n\n\
         <b>Activități</b>: scrii sus ce vrei să urmărești și alegi felul: <b>cu bifă</b> (cafea, făcut patul) sau <b>cu număr</b> (flotări; apă, cu unitatea „pahare”), apoi Enter. \
         În fiecare zi bifezi sau scrii câte ai făcut. În dreapta vezi în câte zile ai făcut fiecare și, la cele cu număr, totalul și media pe zi. \
         Lista trece singură în lunile următoare; clic dreapta pe un nume îl scoate.\n\n\
         <b>Somn</b>: pentru fiecare zi scrii cât ai dormit noaptea și cât ziua (7:30, 7,5 sau 45m), apoi Enter. \
         În dreapta, graficul lunii se face singur: noaptea cu albastru, ziua cu ocru deasupra, plus mediile.\n\n\
         <b>Serviciu</b>: clic pe o zi pentru <b>Serviciu</b> (24 de ore), <b>Rezervă</b>, <b>Concediu</b> sau <b>Liber</b>; tot în meniu scrii o <b>notă</b> pentru zi \
         și adaugi <b>ore suplimentare</b> (de exemplu 08:00-12:00), cu notă dacă vrei. Cu tastatura, pe ziua aleasă: S, R, C, O, Delete. \
         În dreapta: serviciile, rezervele, orele suplimentare, zilele de concediu și totalul orelor lucrate. \
         Ce ai trecut aici apare și în pagina zilei, în zilele săptămânii și în calendarul lunii; un clic pe casetă te aduce înapoi aici.\n\n\
         Luna stă în <tt>monitorizare/2026-11.md</tt>, ca text simplu.",
        "Tracking",
        "<b>Tracking</b>, in the sidebar, has three tabs, month by month. The arrows in the header (or Alt+← →) change the month.\n\n\
         <b>Activities</b>: type at the top what you want to follow and choose its kind: <b>tick</b> (coffee, making the bed) or <b>count</b> (push-ups; water, with the unit \"glasses\"), then Enter. \
         Each day, tick it or type how many. On the right you see on how many days you did each one and, for counted ones, the total and the daily average. \
         The list carries over to the next months; right-click a name to remove it.\n\n\
         <b>Sleep</b>: for each day, type how long you slept at night and during the day (7:30, 7.5 or 45m), then Enter. \
         On the right, the month's chart draws itself: night in blue, daytime sleep in ochre on top, plus the averages.\n\n\
         <b>Work</b>: click a day for <b>Duty</b> (24 hours), <b>Standby</b>, <b>Leave</b> or <b>Free</b>; in the same menu write a <b>note</b> for the day \
         and add <b>overtime</b> (for example 08:00-12:00), with a note if you like. With the keyboard, on the chosen day: D, S, L, O, Delete. \
         On the right: duties, standbys, overtime, days of leave and total hours worked. \
         What you enter here also appears on the day page, on the week's days and in the month calendar; clicking it brings you back here.\n\n\
         The month is stored in <tt>monitorizare/2026-11.md</tt> as plain text.",
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
        "Exportul și tipărirea",
        "În <b>☰ → Exportă și tipărește</b>:\n\n\
         <b>Pagina ca PDF</b> (Ctrl+Shift+E): ce vezi acum (ieri și azi alăturate, săptămâna, luna, planurile, indexul), \
         cu hârtia și culorile tale, întreagă chiar dacă pe ecran trebuie derulată.\n\
         <b>Mai multe zile ca PDF</b>: alegi de la ce zi până la ce zi (cel mult un an); fiecare zi devine o pagină.\n\
         <b>Tipărește</b> (Ctrl+P): aceleași pagini, prin fereastra de tipărire a calculatorului (de acolo se poate salva și ca PDF).\n\
         <b>Tot jurnalul ca arhivă zip</b>: toate fișierele Markdown și pozele, bune ca copie de siguranță sau ca să le muți pe alt calculator. Alegi „fără parolă” sau „cu parolă” (cel puțin 8 caractere; arhiva se deschide cu parola în 7-Zip sau în managerul de arhive). Un jurnal criptat dă mereu o arhivă cu parolă.\n\n\
         În PDF nu apar câmpul de scris, „+ etichetă” și intrarea aleasă. În timpul migrării sau al căutării exportul așteaptă să le termini.",
        "Export and print",
        "In <b>☰ → Export and print</b>:\n\n\
         <b>Page as PDF</b> (Ctrl+Shift+E): what you see now (yesterday and today side by side, the week, month, plans, index), \
         with your paper and colors, whole even if it needs scrolling on screen.\n\
         <b>Several days as PDF</b>: pick the first and last day (at most a year); each day becomes a page.\n\
         <b>Print</b> (Ctrl+P): the same pages, through the computer's print window (which can also save as PDF).\n\
         <b>The whole journal as zip</b>: every Markdown file and picture, good as a backup or to move to another computer. Choose \"without a password\" or \"with a password\" (at least 8 characters; the archive opens with it in 7-Zip or the archive manager). An encrypted journal always gives a password-protected archive.\n\n\
         The writing field, \"+ tag\" and the selected entry don't appear in the PDF. During a migration or a search, export waits until you finish.",
    ),
    (
        "Tastele",
        "Toate tastele sunt în <b>☰ → Scurtături de taste</b> (Ctrl+?). Cele mai folosite: Ctrl+F căutare, Ctrl+M migrare, Ctrl+I în index, Ctrl+G mergi la data, Ctrl+P tipărește, \
         Tab sub intrarea de deasupra, Ctrl+K legătură (la editare), Alt+← → zile, Alt+Home azi, F9 bara laterală, F11 fereastra mare, F10 meniul, Ctrl+Q ieșire, F1 ghidul acesta.\n\n\
         <b>Taste ca în Vim</b> (se pornesc din Setări → Caietul): în afara câmpului de scris, <b>j</b> / <b>k</b> aleg intrarea de dedesubt / de deasupra, \
         <b>gg</b> / <b>G</b> prima / ultima, <b>h</b> / <b>l</b> pagina de dinainte / de după, <b>t</b> azi, <b>x</b> bifează, <b>i</b> editează, \
         <b>o</b> scrie o intrare nouă, <b>dd</b> șterge, <b>&gt;&gt;</b> / <b>&lt;&lt;</b> pune sub cea de deasupra / scoate, <b>/</b> caută. \
         <b>Esc</b> te scoate din câmpul de scris înapoi la intrări.",
        "Keys",
        "All keys are in <b>☰ → Keyboard Shortcuts</b> (Ctrl+?). The most used: Ctrl+F search, Ctrl+M migrate, Ctrl+I index, Ctrl+G go to date, Ctrl+P print, \
         Tab under the entry above, Ctrl+K link (while editing), Alt+← → days, Alt+Home today, F9 sidebar, F11 maximize, F10 menu, Ctrl+Q quit, F1 this guide.\n\n\
         <b>Vim keys</b> (turned on in Preferences → Notebook): outside the writing field, <b>j</b> / <b>k</b> select the entry below / above, \
         <b>gg</b> / <b>G</b> the first / last, <b>h</b> / <b>l</b> the previous / next page, <b>t</b> today, <b>x</b> check, <b>i</b> edit, \
         <b>o</b> write a new entry, <b>dd</b> delete, <b>&gt;&gt;</b> / <b>&lt;&lt;</b> indent / outdent, <b>/</b> search. \
         <b>Esc</b> takes you out of the writing field back to the entries.",
    ),
    (
        "Unde stă jurnalul",
        "Totul stă în dosarul ascuns <tt>~/.lanruj</tt>, în fișiere Markdown pe care le poți citi și fără aplicație: \
         <tt>2026/11/2026-11-21.md</tt> (o zi), <tt>2026/11/luna.md</tt>, <tt>viitor.md</tt>, <tt>index.md</tt>, \
         <tt>pagini/</tt>, iar pozele lângă ziua lor.\n\n\
         <b>Pe mai multe calculatoare</b>: instalează Syncthing pe fiecare, leagă calculatoarele între ele (Adaugă dispozitiv, cu codul lui) \
         și partajează dosarul <tt>~/.lanruj</tt>. Se trimite doar ce e nou.\n\n\
         <b>Copii de siguranță</b>: Lanruj face singur copii zip (vezi Setările); pentru siguranță deplină, ține dosarul copiilor pe alt disc sau stick.",
        "Where the journal lives",
        "Everything lives in the hidden folder <tt>~/.lanruj</tt>, in Markdown files you can read without the app: \
         <tt>2026/11/2026-11-21.md</tt> (a day), <tt>2026/11/luna.md</tt>, <tt>viitor.md</tt>, <tt>index.md</tt>, \
         <tt>pagini/</tt>, with pictures next to their day.\n\n\
         <b>On several computers</b>: install Syncthing on each, link the computers (Add device, with its ID) \
         and share the folder <tt>~/.lanruj</tt>. Only what's new is sent.\n\n\
         <b>Backups</b>: Lanruj makes zip backups by itself (see Preferences); to be really safe, keep the backup folder on another disk or USB stick.",
    ),
    (
        "Fișierele",
        "Jurnalul se poate citi și scrie și cu alt program (Obsidian, Typora, un editor de text). Cum arată în fișier:\n\n\
         <tt>- [ ] Sună la bancă</tt>  sarcină     <tt>- [x] …</tt> făcută     <tt>- [&gt;] …</tt> mutată     <tt>- [&lt;] …</tt> programată     <tt>- [-] …</tt> renunțată\n\
         <tt>- o 10:00 Dentist</tt>  eveniment     <tt>- Prima zăpadă</tt>  notă     <tt>- [ ] * …</tt> important     <tt>- ! …</tt> idee\n\
         <tt>  - frig, dar senin</tt>  notiță sub intrarea de deasupra (două spații)\n\
         <tt>^k3f9</tt> la sfârșit: semnul unei sarcini mutate (nu-l șterge)\n\
         <tt>==text==</tt> marker galben, <tt>&lt;span style=\"color:#c01c28\"&gt;text&lt;/span&gt;</tt> text colorat\n\
         <tt>[text](https://…)</tt> legătură     <tt>![Nume|220](2026-11-21/poza.jpg)</tt> poză     <tt>[bilet.pdf](…)</tt> fișier\n\
         <tt>Etichete: carte, film</tt> și <tt>Ziua: 4/5</tt> la sfârșitul zilei\n\n\
         Modifică fișierele doar când Lanruj e închis, ca să nu scrie amândouă în același timp.",
        "Files",
        "The journal can also be read and written with another program (Obsidian, Typora, a text editor). How it looks in the file:\n\n\
         <tt>- [ ] Call the bank</tt>  task     <tt>- [x] …</tt> done     <tt>- [&gt;] …</tt> moved     <tt>- [&lt;] …</tt> scheduled     <tt>- [-] …</tt> dropped\n\
         <tt>- o 10:00 Dentist</tt>  event     <tt>- First snow</tt>  note     <tt>- [ ] * …</tt> important     <tt>- ! …</tt> idea\n\
         <tt>  - cold but clear</tt>  note under the entry above (two spaces)\n\
         <tt>^k3f9</tt> at the end: the mark of a moved task (don't delete it)\n\
         <tt>==text==</tt> yellow marker, <tt>&lt;span style=\"color:#c01c28\"&gt;text&lt;/span&gt;</tt> colored text\n\
         <tt>[text](https://…)</tt> link     <tt>![Name|220](2026-11-21/pic.jpg)</tt> picture     <tt>[ticket.pdf](…)</tt> file\n\
         <tt>Tags: book, film</tt> and <tt>Day: 4/5</tt> at the end of a day\n\n\
         Edit the files only while Lanruj is closed, so the two don't write at the same time.",
    ),
    (
        "Setările",
        "<b>☰ → Setări</b> (Ctrl+,):\n\n\
         • <b>Hârtia</b>: modelul, culoarea și noaptea.\n\
         • <b>Migrarea</b>: singură sau doar cu butonul, ora de seară și după câte mutări apare „mai merită?” (3 la început).\n\
         • <b>Fereastra</b>: systray-ul (la închidere, Lanruj rămâne în bara de sus; Ctrl+Q îl oprește).\n\
         • <b>Caietul</b>: mărimea textului (mic, normal, mare, foarte mare), pagina arătată la pornire \
         și <b>o singură pagină</b> (doar ziua arătată, fără ziua dinainte alături).\n\
         • <b>Copii de siguranță</b>: o dată pe zi sau pe săptămână (la început: pe săptămână), Lanruj pune tot jurnalul într-o arhivă zip \
         în dosarul <tt>Documente/Lanruj - copii</tt> sau în altul ales de tine, și păstrează ultimele 10. <b>Fă o copie acum</b> o face pe loc.\n\
         • <b>Limba</b>: fereastra se redeschide în limba aleasă.",
        "Preferences",
        "<b>☰ → Preferences</b> (Ctrl+,):\n\n\
         • <b>Paper</b>: pattern, color and dark mode.\n\
         • <b>Migration</b>: automatic or only with the button, the evening hour, and after how many moves \"still worth it?\" appears (3 at first).\n\
         • <b>Window</b>: the system tray (when closed, Lanruj stays in the top bar; Ctrl+Q quits).\n\
         • <b>Notebook</b>: text size (small, normal, large, larger), the page shown at startup \
         and <b>a single page</b> (only the shown day, without the day before next to it).\n\
         • <b>Backups</b>: once a day or once a week (weekly at first), Lanruj puts the whole journal in a zip archive \
         in the folder <tt>Documents/Lanruj - copii</tt> or another one you choose, and keeps the last 10. <b>Back up now</b> makes one right away.\n\
         • <b>Language</b>: the window reopens in the chosen language.",
    ),
    (
        "Criptarea",
        "În <b>☰ → Setări → Criptarea → Criptează…</b> alegi o parolă (cel puțin 8 caractere). Lanruj îți arată apoi \
         <b>cheia de recuperare</b> (6 grupuri de câte 5 caractere): tipărește-o sau scrie-o pe hârtie, apoi scrie ultimele ei 4 caractere ca să mergi mai departe. \
         Întâi se face o copie a jurnalului, apoi fiecare pagină și fiecare poză se criptează separat (numele fișierelor rămân aceleași).\n\n\
         De atunci, la fiecare pornire Lanruj cere parola. Dacă ai uitat-o: <b>Am uitat parola</b>, scrii cheia de recuperare și alegi o parolă nouă.\n\n\
         De știut: fișierele nu se mai pot citi cu alte programe (Obsidian, Markor); copiile de siguranță și arhiva zip rămân criptate; \
         <b>fără parolă și fără cheia de recuperare jurnalul e pierdut</b>. Tot din Setări schimbi parola sau oprești criptarea (fișierele redevin text).",
        "Encryption",
        "In <b>☰ → Preferences → Encryption → Encrypt…</b> you choose a password (at least 8 characters). Lanruj then shows the \
         <b>recovery key</b> (6 groups of 5 characters): print it or write it on paper, then type its last 4 characters to continue. \
         A backup is made first, then every page and every picture is encrypted separately (file names stay the same).\n\n\
         From then on Lanruj asks for the password at every start. If you forgot it: <b>I forgot the password</b>, type the recovery key and choose a new password.\n\n\
         Good to know: other programs (Obsidian, Markor) can't read the files any more; backups and the zip archive stay encrypted; \
         <b>without the password and the recovery key the journal is lost</b>. Also in Preferences you change the password or turn encryption off (the files become text again).",
    ),
    (
        "Încuierea",
        "În <b>☰ → Setări → Încuierea</b>:\n\n\
         • <b>La deschidere cere</b>: nimic, o parolă sau un PIN de 4 cifre (se trimite singur la a patra cifră). \
         La un jurnal necriptat, încuietoarea doar ascunde fereastra: fișierele se pot citi în continuare cu alte programe.\n\
         • <b>Se încuie la blocarea ecranului</b>: când blochezi calculatorul, Lanruj se ascunde și cere parola sau PIN-ul. \
         Un jurnal <b>criptat</b> se încuie mereu; după blocarea ecranului cere PIN-ul (dacă ai pus unul) sau parola jurnalului.\n\
         • <b>Ține minte parola pe acest calculator</b> (jurnal criptat): parola stă în portofelul de chei GNOME. \
         Portofelul se deschide odată cu sesiunea (la autentificarea automată, în fereastra mică de după pornire), iar Lanruj nu mai cere parola: \
         fereastra lui de parolă se închide singură.\n\
         • <b>Pornește odată cu calculatorul</b>.\n\n\
         Dacă închizi fereastra „Lanruj e încuiat”, Lanruj se oprește, iar jurnalul rămâne încuiat.",
        "Lock",
        "In <b>☰ → Preferences → Lock</b>:\n\n\
         • <b>When opening, ask for</b>: nothing, a password or a 4-digit PIN (sent by itself at the fourth digit). \
         For an unencrypted journal the lock only hides the window: other programs can still read the files.\n\
         • <b>Lock when the screen locks</b>: when you lock the computer, Lanruj hides and asks for the password or PIN. \
         An <b>encrypted</b> journal always locks; after the screen locks it asks for the PIN (if you set one) or the journal password.\n\
         • <b>Remember the password on this computer</b> (encrypted journal): the password lives in the GNOME keyring. \
         The keyring opens with your session (with automatic login, in the small window after startup), and Lanruj no longer asks: \
         its password window closes by itself.\n\
         • <b>Start with the computer</b>.\n\n\
         Closing the \"Lanruj is locked\" window quits Lanruj; the journal stays locked.",
    ),
    (
        "Sfaturi utile",
        "• <b>Nu trebuie să salvezi nimic</b>: fiecare schimbare se scrie pe loc. Dacă ai șters ceva din greșeală, apasă Anulează în notificarea de jos.\n\
         • <b>Mergi la data…</b> (Ctrl+G) te duce la orice zi; <b>Azi</b> (Alt+Home) te aduce înapoi.\n\
         • <b>F9</b> strânge bara laterală la iconițe (și o deschide); pe o fereastră mică se strânge singură. <b>F11</b> mărește fereastra.\n\
         • Sarcinile mutate de mai multe ori primesc <b>↻</b>; când apare „mai merită?”, gândește-te dacă mai merită.\n\
         • Lucrurile mari (un concediu, un proiect) pune-le în <b>Index</b>: au o pagină a lor, iar rândurile cu dată apar și în zilele lor.\n\
         • Rândurile din fișiere sunt Markdown obișnuit: le poți deschide și cu Obsidian, Typora sau orice editor de text.\n\
         • Cu <b>systray</b> pornit (Setări → Fereastra), închiderea doar ascunde fereastra; Ctrl+Q oprește aplicația.\n\
         • Totul despre taste: Ctrl+? (Scurtături de taste).",
        "Useful tips",
        "• <b>No need to save</b>: every change is written at once. If you deleted something by mistake, press Undo in the notification at the bottom.\n\
         • <b>Go to date…</b> (Ctrl+G) takes you to any day; <b>Today</b> (Alt+Home) brings you back.\n\
         • <b>F9</b> collapses the sidebar to icons (and back); on a small window it collapses by itself. <b>F11</b> maximizes the window.\n\
         • Tasks moved several times get <b>↻</b>; when \"still worth it?\" appears, think it over.\n\
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
    // Căutarea în ghid: rămân doar capitolele care conțin toate cuvintele.
    let search = gtk::SearchEntry::builder().placeholder_text(tr(lang, "Caută în ghid", "Search the guide")).build();
    body.append(&search);
    // Cuprinsul: un buton pentru fiecare capitol.
    let toc = adw::WrapBox::builder().child_spacing(6).line_spacing(6).build();
    body.append(&toc);
    let mut heads: Vec<gtk::Label> = Vec::new();
    let mut sections: Vec<(gtk::Box, String)> = Vec::new();
    for (tro, bro, ten, ben) in CAPITOLE {
        let (title, text) = if lang == Lang::Ro { (tro, bro) } else { (ten, ben) };
        let h = gtk::Label::builder().label(title).xalign(0.0).css_classes(["title-3"]).build();
        let t = gtk::Label::builder().label(text).use_markup(true).wrap(true).xalign(0.0).selectable(true).css_classes(["ghid-text"]).build();
        let sec = gtk::Box::new(gtk::Orientation::Vertical, 6);
        sec.append(&h);
        sec.append(&t);
        body.append(&sec);
        heads.push(h);
        // Textul fără etichete de markup, pentru căutare.
        let plain = gtk::pango::parse_markup(text, '\0').map(|(_, t, _)| t.to_string()).unwrap_or_else(|_| text.to_string());
        sections.push((sec.clone(), lanruj_core::search::fold(&format!("{title} {plain}"))));
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
    let tc = toc.clone();
    search.connect_search_changed(move |e| {
        let words: Vec<String> = lanruj_core::search::fold(&e.text()).split_whitespace().map(str::to_string).collect();
        for (sec, text) in &sections {
            sec.set_visible(words.iter().all(|w| text.contains(w.as_str())));
        }
        tc.set_visible(words.is_empty());
    });
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    dialog.set_child(Some(&view));
    dialog.present(Some(&app.window_widget()));
    search.grab_focus();
}
