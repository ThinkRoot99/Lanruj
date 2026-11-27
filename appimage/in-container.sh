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
mkdir -p "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders"
cp -a "$PIXBUF"/*.so "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders/" 2>/dev/null || true
GDK_PIXBUF_MODULEDIR="$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders" $SYS/gdk-pixbuf-2.0/gdk-pixbuf-query-loaders \
    | sed "s#$APPDIR#@APPDIR@#g" > "$APPDIR/usr/lib/gdk-pixbuf-2.0/loaders.cache.in"

# ---- Datele: schemele GSettings și tema de iconițe Adwaita (doar cele simbolice) ----
mkdir -p "$APPDIR/usr/share/glib-2.0/schemas"
cp /usr/share/glib-2.0/schemas/org.gtk.gtk4.*.xml "$APPDIR/usr/share/glib-2.0/schemas/"
glib-compile-schemas "$APPDIR/usr/share/glib-2.0/schemas"
mkdir -p "$APPDIR/usr/share/icons/Adwaita"
cp /usr/share/icons/Adwaita/index.theme "$APPDIR/usr/share/icons/Adwaita/"
cp -a /usr/share/icons/Adwaita/symbolic "$APPDIR/usr/share/icons/Adwaita/"
[ -d /usr/share/icons/Adwaita/scalable ] && cp -a /usr/share/icons/Adwaita/scalable "$APPDIR/usr/share/icons/Adwaita/"
cp /usr/share/icons/hicolor/index.theme "$APPDIR/usr/share/icons/hicolor/"

# ---- Rezerve pentru sistemele fără ele: configurația tastaturii (XKB) și a fonturilor ----
[ -d /usr/share/X11/xkb ] && { mkdir -p "$APPDIR/usr/share/X11"; cp -aL /usr/share/X11/xkb "$APPDIR/usr/share/X11/"; }
[ -d /etc/fonts ] && { mkdir -p "$APPDIR/etc"; cp -aL /etc/fonts "$APPDIR/etc/"; }

# ---- Bibliotecile ----
# Intră toate, inclusiv glibc și încărcătorul ei: pachetul nu depinde de versiunile din sistem.
# libGL, libEGL și celelalte din libglvnd intră și ele (GTK e legat de ele), dar sunt doar dispecerii care
# aleg driverul plăcii video; driverul nu se încarcă, pentru că AppRun oprește folosirea plăcii video.
EXCLUDE='^$'
cp -L /lib64/ld-linux-x86-64.so.2 "$APPDIR/usr/lib/ld-linux-x86-64.so.2"

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
find "$APPDIR/usr/lib/gtk-4.0" "$APPDIR/usr/lib/gdk-pixbuf-2.0" -name '*.so' | while read -r m; do collect "$m"; done

# fontconfig cercetează și lista de configurații „disponibile” a sistemului (calea e scrisă în bibliotecă), pe care
# un fontconfig mai vechi n-o înțelege: avertismente fără folos. Calea devine una de aceeași lungime, care nu există.
FC=$(ls "$APPDIR"/usr/lib/libfontconfig.so.* | head -1)
n=$(grep -c -a "/usr/share/fontconfig/conf.avail" "$FC" || true)
perl -pi -e 's#/usr/share/fontconfig/conf\.avail#/usr/share/fontconfig/conf.nonex#g' "$FC"
echo "fontconfig: $n locuri schimbate"

# Verificare: nicio bibliotecă lipsă
missing=$(for f in "$APPDIR/usr/bin/lanruj" $(find "$APPDIR/usr/lib" -name '*.so*'); do
    LD_LIBRARY_PATH="$APPDIR/usr/lib" ldd "$f" 2>/dev/null | grep "not found"; done | sort -u)
[ -z "$missing" ] || { echo "Biblioteci lipsă:"; echo "$missing"; exit 1; }

# ---- Arhiva squashfs (runtime-ul AppImage se lipește în față, pe calculatorul gazdă) ----
desktop-file-validate "$APPDIR/io.github.ThinkRoot99.Lanruj.desktop"
rm -f /out/Lanruj.squashfs
mksquashfs "$APPDIR" /out/Lanruj.squashfs -root-owned -noappend -comp zstd -Xcompression-level 19 -quiet
echo "AppDir: $(du -sh "$APPDIR" | cut -f1), arhivă: $(du -h /out/Lanruj.squashfs | cut -f1), biblioteci: $(ls "$APPDIR/usr/lib" | grep -c '\.so')"
