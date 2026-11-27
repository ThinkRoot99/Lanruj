#!/bin/sh
# Construiește Lanruj-<versiune>-x86_64.AppImage în appimage/, într-un container Ubuntu 26.04 (podman).
#
# Nevoie: podman și runtime-ul AppImage, pus de mână în appimage/unelte/runtime-x86_64
#   (https://github.com/AppImage/type2-runtime/releases, fișierul runtime-x86_64).
# Folosire, din rădăcina proiectului: sh appimage/construieste.sh
set -eu

cd "$(dirname "$0")/.."
ROOT=$PWD
A=$ROOT/appimage
VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
RUNTIME=$A/unelte/runtime-x86_64

# Pe btrfs, podman nu poate folosi overlay: imaginile stau în appimage/.podman, cu driverul btrfs.
P="podman --storage-driver=btrfs --root=$A/.podman/root --runroot=${XDG_RUNTIME_DIR:-/tmp}/lanruj-podman"
IMG=localhost/lanruj-appimage:ubuntu-26.04

# Imaginea cu uneltele se face o singură dată. (Fără „podman build”: are nevoie de overlay.)
if ! $P image exists $IMG; then
    $P rm -f lanruj-pregatire >/dev/null 2>&1 || true
    $P run --name lanruj-pregatire -v "$A:/a:ro" docker.io/library/ubuntu:26.04 sh /a/pregateste.sh
    $P commit -q --change 'ENV PATH=/usr/lib/rust-1.97/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin' lanruj-pregatire $IMG >/dev/null
    $P rm lanruj-pregatire >/dev/null
fi
mkdir -p "$A/build" "$A/.cargo" "$A/.target"
$P run --rm \
    -v "$ROOT:/src:ro" -v "$A/build:/out" -v "$A/.cargo:/root/.cargo" -v "$A/.target:/work/target" \
    $IMG sh /src/appimage/in-container.sh

[ -f "$RUNTIME" ] || { echo "Arhiva e gata, dar lipsește $RUNTIME (vezi începutul scriptului)."; exit 1; }
OUT=$A/Lanruj-$VER-x86_64.AppImage
cat "$RUNTIME" "$A/build/Lanruj.squashfs" > "$OUT"
chmod +x "$OUT"
echo "Gata: $OUT ($(du -h "$OUT" | cut -f1))"
