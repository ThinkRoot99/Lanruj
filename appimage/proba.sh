#!/bin/sh
# Proba AppImage-ului pe distribuții goale (doar imaginea oficială, fără niciun pachet în plus), într-un
# compozitor Wayland fără ecran (mutter --headless). Pentru fiecare: pornește 10 s, apoi verifică lansatorul din meniu.
# Folosire, din rădăcina proiectului: dbus-run-session -- sh appimage/proba.sh
set -u
cd "$(dirname "$0")/.."
A=$PWD/appimage
IMG=$(ls "$A"/Lanruj-*-x86_64.AppImage | tail -1)
P="podman --cgroup-manager=cgroupfs --storage-driver=btrfs --root=$A/.podman/root --runroot=${XDG_RUNTIME_DIR:-/tmp}/lanruj-podman"
DISTROS="${*:-docker.io/library/debian:13 docker.io/library/ubuntu:24.04 docker.io/library/fedora:latest docker.io/opensuse/leap:16.0 docker.io/library/archlinux:latest}"

before=$(ls "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$')
mutter --headless --wayland --no-x11 --virtual-monitor 1400x900 >/dev/null 2>&1 &
M=$!
sleep 3
W=$(ls "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$' | grep -vxF "$before" | head -1)

for d in $DISTROS; do
    out=$($P run --rm -v "$IMG:/Lanruj.AppImage:ro" -v "$XDG_RUNTIME_DIR/$W:/run/w/wayland-0" \
        -e XDG_RUNTIME_DIR=/run/w -e WAYLAND_DISPLAY=wayland-0 -e APPIMAGE_EXTRACT_AND_RUN=1 -e HOME=/root \
        "$d" sh -c '. /etc/os-release; echo "== $PRETTY_NAME, $(ldd --version 2>/dev/null | head -1 | grep -o "[0-9.]*$" | sed "s/^/glibc /")"
            cd /tmp; timeout 10 /Lanruj.AppImage > /tmp/log 2>&1; c=$?
            [ $c = 124 ] && echo "PORNEȘTE: a rulat 10 s" || { echo "EȘUAT (cod $c)"; }
            grep -v "^$" /tmp/log | tail -6
            ls /root/.local/share/applications/io.github.ThinkRoot99.Lanruj.desktop >/dev/null 2>&1 && echo "MENIU: lansator pus" || echo "MENIU: lipsește"' 2>&1)
    echo "$out"
done
kill $M
