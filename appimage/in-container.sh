#!/bin/sh
# Rulează în container (vezi construieste.sh): compilează Lanruj și adună AppDir-ul în /out/Lanruj.AppDir.
# Sursa e montată doar pentru citire în /src; compilarea se face în /work.
set -eu

ARCH=x86_64-linux-gnu
SYS=/usr/lib/$ARCH
APPDIR=/out/Lanruj.AppDir

# ---- Compilarea ----
mkdir -p /work
# target/ e montat din appimage/.target, ca a doua construire să nu compileze totul de la capăt
cp -a /src/Cargo.toml /src/Cargo.lock /src/crates /src/data /work/
cd /work
cargo build --release --locked
strip target/release/lanruj

# ---- AppDir: programul, lansatorul, iconițele, fișa AppStream ----
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib" "$APPDIR/usr/share"
install -m755 target/release/lanruj "$APPDIR/usr/bin/lanruj"
install -m755 /src/appimage/AppRun "$APPDIR/AppRun"
install -Dm644 data/io.github.ThinkRoot99.Lanruj.desktop "$APPDIR/usr/share/applications/io.github.ThinkRoot99.Lanruj.desktop"
install -Dm644 data/io.github.ThinkRoot99.Lanruj.metainfo.xml "$APPDIR/usr/share/metainfo/io.github.ThinkRoot99.Lanruj.appdata.xml"
cp -a data/icons "$APPDIR/usr/share/"
cp data/io.github.ThinkRoot99.Lanruj.desktop "$APPDIR/"
cp data/icons/hicolor/scalable/apps/io.github.ThinkRoot99.Lanruj.svg "$APPDIR/io.github.ThinkRoot99.Lanruj.svg"
ln -s io.github.ThinkRoot99.Lanruj.svg "$APPDIR/.DirIcon"

# ---- Modulele GTK și GIO ----
# Tipărirea (CUPS și „în fișier”) și încărcătoarele de imagini (SVG pentru iconițe).
PB=$(find /usr/lib -type d -path '*gtk-4.0/4.0.0/printbackends' | head -1)
# În Ubuntu 26.04 tipărirea e inclusă în libgtk-4 (care depinde direct de CUPS); modulele separate, doar dacă există.
mkdir -p "$APPDIR/usr/lib/gtk-4.0/4.0.0"
[ -z "$PB" ] || cp -a "$PB" "$APPDIR/usr/lib/gtk-4.0/4.0.0/"
PIXBUF=$(pkg-config --variable=gdk_pixbuf_moduledir gdk-pixbuf-2.0)
mkdir -p "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders" "$APPDIR/usr/lib/gio/modules"
cp -a "$PIXBUF"/*.so "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders/" 2>/dev/null || true
GDK_PIXBUF_MODULEDIR="$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders" $SYS/gdk-pixbuf-2.0/gdk-pixbuf-query-loaders \
    | sed "s#$APPDIR#@APPDIR@#g" > "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders.cache.in"
# dconf, ca setările GTK ale sistemului (tema, fontul) să fie citite
[ -f $SYS/gio/modules/libdconfsettings.so ] && cp $SYS/gio/modules/libdconfsettings.so "$APPDIR/usr/lib/gio/modules/"

# ---- Datele: schemele GSettings și tema de iconițe Adwaita (doar cele simbolice) ----
mkdir -p "$APPDIR/usr/share/glib-2.0/schemas"
cp /usr/share/glib-2.0/schemas/org.gtk.gtk4.*.xml "$APPDIR/usr/share/glib-2.0/schemas/"
glib-compile-schemas "$APPDIR/usr/share/glib-2.0/schemas"
mkdir -p "$APPDIR/usr/share/icons/Adwaita"
cp /usr/share/icons/Adwaita/index.theme "$APPDIR/usr/share/icons/Adwaita/"
cp -a /usr/share/icons/Adwaita/symbolic "$APPDIR/usr/share/icons/Adwaita/"
[ -d /usr/share/icons/Adwaita/scalable ] && cp -a /usr/share/icons/Adwaita/scalable "$APPDIR/usr/share/icons/Adwaita/"
cp /usr/share/icons/hicolor/index.theme "$APPDIR/usr/share/icons/hicolor/"

# ---- Bibliotecile ----
# Nu intră în pachet ce trebuie să vină din sistem: glibc, driverele grafice (GL, EGL, Vulkan, DRM, GBM),
# fontconfig și freetype (legate de configurarea fonturilor sistemului) și X11/xcb.
# Lista urmează excludelist-ul proiectului AppImage.
EXCLUDE='^(ld-linux|libc|libm|libdl|libpthread|librt|libresolv|libutil|libnsl|libanl|libmvec|libBrokenLocale|libthread_db|libGL|libGLX|libGLdispatch|libOpenGL|libEGL|libGLESv2|libvulkan|libdrm|libgbm|libfontconfig|libfreetype|libharfbuzz|libX11|libX11-xcb|libxcb|libxcb-.*|libxshmfence|libasound|libjack|libuuid|libcom_err|libgpg-error|libexpat|libz)\.so'

collect() {
    ldd "$1" | awk '/=> \//{print $3}' | while read -r lib; do
        name=$(basename "$lib")
        echo "$name" | grep -Eq "$EXCLUDE" && continue
        [ -e "$APPDIR/usr/lib/$name" ] && continue
        cp -L "$lib" "$APPDIR/usr/lib/$name"
        collect "$lib"
    done
}
collect "$APPDIR/usr/bin/lanruj"
find "$APPDIR/usr/lib/gtk-4.0" "$APPDIR/usr/lib/gdk-pixbuf-2.0" "$APPDIR/usr/lib/gio" -name '*.so' | while read -r m; do collect "$m"; done

# Verificare: nicio bibliotecă lipsă
missing=$(for f in "$APPDIR/usr/bin/lanruj" $(find "$APPDIR/usr/lib" -name '*.so*'); do
    LD_LIBRARY_PATH="$APPDIR/usr/lib" ldd "$f" 2>/dev/null | grep "not found"; done | sort -u)
[ -z "$missing" ] || { echo "Biblioteci lipsă:"; echo "$missing"; exit 1; }

# ---- Arhiva squashfs (runtime-ul AppImage se lipește în față, pe calculatorul gazdă) ----
desktop-file-validate "$APPDIR/io.github.ThinkRoot99.Lanruj.desktop"
rm -f /out/Lanruj.squashfs
mksquashfs "$APPDIR" /out/Lanruj.squashfs -root-owned -noappend -comp zstd -Xcompression-level 19 -quiet
echo "AppDir: $(du -sh "$APPDIR" | cut -f1), arhivă: $(du -h /out/Lanruj.squashfs | cut -f1), biblioteci: $(ls "$APPDIR/usr/lib" | grep -c '\.so')"
