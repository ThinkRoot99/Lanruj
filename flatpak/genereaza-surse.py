#!/usr/bin/env python3
"""Sursele Rust pentru construirea Flatpak fără internet, din Cargo.lock.

Fiecare pachet de pe crates.io devine o arhivă (adresa și sumă de control din Cargo.lock),
despachetată în cargo/vendor/<nume>-<versiune>, cu fișierul .cargo-checksum.json cerut de Cargo,
plus configurarea care îi spune lui Cargo să ia pachetele de acolo.

Folosire (din rădăcina proiectului): python3 flatpak/genereaza-surse.py > flatpak/cargo-sources.json
"""
import json
import sys
import tomllib

CRATES = "registry+https://github.com/rust-lang/crates.io-index"

with open("Cargo.lock", "rb") as f:
    lock = tomllib.load(f)

sources = []
for p in lock["package"]:
    if p.get("source") != CRATES:
        continue  # pachetele proiectului (lanruj, lanruj-core)
    if "checksum" not in p:
        sys.exit(f"lipsește suma de control pentru {p['name']} {p['version']}")
    name, ver, sha = p["name"], p["version"], p["checksum"]
    dest = f"cargo/vendor/{name}-{ver}"
    sources.append({
        "type": "archive",
        "archive-type": "tar-gzip",
        "url": f"https://static.crates.io/crates/{name}/{name}-{ver}.crate",
        "sha256": sha,
        "dest": dest,
    })
    sources.append({
        "type": "inline",
        "contents": json.dumps({"package": sha, "files": {}}),
        "dest": dest,
        "dest-filename": ".cargo-checksum.json",
    })
sources.append({
    "type": "inline",
    "contents": '[source.vendored-sources]\ndirectory = "cargo/vendor"\n\n[source.crates-io]\nreplace-with = "vendored-sources"\n',
    "dest": "cargo",
    "dest-filename": "config.toml",
})
json.dump(sources, sys.stdout, indent=2)
sys.stdout.write("\n")
