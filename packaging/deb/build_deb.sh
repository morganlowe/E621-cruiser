#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-1.1.0}"
VERSION="${VERSION#v}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${2:-$ROOT_DIR/dist}"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

PKG_DIR="$STAGE/e621-cruiser_${VERSION}_all"
mkdir -p \
    "$PKG_DIR/DEBIAN" \
    "$PKG_DIR/usr/bin" \
    "$PKG_DIR/usr/share/e621-cruiser/e621cruiser" \
    "$PKG_DIR/usr/share/e621-cruiser/assets" \
    "$PKG_DIR/usr/share/applications" \
    "$PKG_DIR/usr/share/icons/hicolor/scalable/apps" \
    "$PKG_DIR/usr/share/icons/hicolor/256x256/apps" \
    "$PKG_DIR/usr/share/doc/e621-cruiser"

install -m 644 "$ROOT_DIR"/e621cruiser/*.py "$PKG_DIR/usr/share/e621-cruiser/e621cruiser/"
install -m 644 "$ROOT_DIR"/assets/e621-cruiser.png "$PKG_DIR/usr/share/e621-cruiser/assets/"
install -m 644 "$ROOT_DIR"/assets/e621-cruiser.svg "$PKG_DIR/usr/share/e621-cruiser/assets/"
install -m 755 "$ROOT_DIR/packaging/e621-cruiser.sh" "$PKG_DIR/usr/bin/e621-cruiser"
ln -sf e621-cruiser "$PKG_DIR/usr/bin/e621cruiser"
install -m 644 "$ROOT_DIR/packaging/e621-cruiser.desktop" "$PKG_DIR/usr/share/applications/e621-cruiser.desktop"
install -m 644 "$ROOT_DIR/assets/e621-cruiser.svg" "$PKG_DIR/usr/share/icons/hicolor/scalable/apps/e621-cruiser.svg"
install -m 644 "$ROOT_DIR/assets/e621-cruiser.png" "$PKG_DIR/usr/share/icons/hicolor/256x256/apps/e621-cruiser.png"
install -m 644 "$ROOT_DIR/LICENSE" "$PKG_DIR/usr/share/doc/e621-cruiser/copyright"

INSTALLED_SIZE="$(du -sk "$PKG_DIR/usr" | awk '{print $1}')"

cat > "$PKG_DIR/DEBIAN/control" <<EOF
Package: e621-cruiser
Version: ${VERSION}
Section: graphics
Priority: optional
Architecture: all
Installed-Size: ${INSTALLED_SIZE}
Depends: python3 (>= 3.10), python3-pyside6.qtcore, python3-pyside6.qtgui, python3-pyside6.qtwidgets, python3-requests
Maintainer: Morgan Lowe <https://github.com/morganlowe/E621-cruiser>
Homepage: https://github.com/morganlowe/E621-cruiser
Description: Desktop browser and downloader for e621
 E621 Cruiser is a PySide6 desktop application to browse, filter, favorite,
 and download posts, pools, artists, and favorites from e621.
EOF

find "$PKG_DIR" -type d -exec chmod 755 {} +
chmod 644 "$PKG_DIR/DEBIAN/control"

mkdir -p "$OUT_DIR"
dpkg-deb --build --root-owner-group "$PKG_DIR" "$OUT_DIR/e621-cruiser_${VERSION}_all.deb"
echo "Built: $OUT_DIR/e621-cruiser_${VERSION}_all.deb"
