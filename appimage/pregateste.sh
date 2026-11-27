#!/bin/sh
# Uneltele de construire din Ubuntu 26.04 LTS (glibc 2.43, GTK 4.22, libadwaita 1.9, Rust 1.97).
# Rulat o singură dată, de construieste.sh; rezultatul devine imaginea localhost/lanruj-appimage.
set -eu
apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates build-essential pkg-config file squashfs-tools desktop-file-utils \
    rustc-1.97 cargo-1.97 \
    libgtk-4-dev libadwaita-1-dev \
    libgtk-4-1 librsvg2-common adwaita-icon-theme hicolor-icon-theme libglib2.0-bin \
    xkb-data fontconfig-config
rm -rf /var/lib/apt/lists/*
