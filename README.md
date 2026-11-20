# Lanruj

Un jurnal cu cerneală, zi de zi. Aplicație pentru Linux (GTK4 + libadwaita), inspirată de metoda Bullet Journal, cu jurnalul păstrat în fișiere Markdown obișnuite, în `~/.lanruj`.

Stadiu: început. Merge ecranul „Azi”: ieri și azi alăturate, scrierea rapidă (`.` sarcină, `o` eveniment, `-` notă), bifarea sarcinilor, etichetele și nota zilei.

## Rulare din cod

```
cargo run --release
```

Pentru probe, fără a atinge jurnalul adevărat: `LANRUJ_DIR=/un/dosar/de/proba cargo run`.

## Licență

GPL-3.0-or-later.
