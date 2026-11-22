#!/usr/bin/env bash
# E621 Cruiser Installer
# Builds the Rust binary (if needed) and installs binary, desktop entry, icons, and launcher.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if ! command -v cargo >/dev/null 2>&1; then
    echo "[ERROR] Cargo (Rust toolchain) was not found in PATH."
    echo "Install Rust from https://rustup.rs/ and re-run ./install.sh"
    exit 1
fi

echo "==> Building E621 Cruiser (release)..."
cargo build --release

BIN_SRC="$SCRIPT_DIR/target/release/e621-cruiser"
chmod +x "$SCRIPT_DIR/run.sh"

echo "==> Cleaning up any legacy grabber entries..."
rm -f "$HOME/.local/share/applications/e621-grabber.desktop"
rm -f "$HOME/Desktop/e621-grabber.desktop"
rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/e621-grabber.svg"
rm -f "$HOME/.local/share/icons/hicolor/256x256/apps/e621-grabber.png"
rm -f "$HOME/.local/bin/e621-grabber"

echo "==> Installing binary to ~/.local/bin..."
LOCAL_BIN="$HOME/.local/bin"
mkdir -p "$LOCAL_BIN"
install -m 755 "$BIN_SRC" "$LOCAL_BIN/e621-cruiser"
ln -sf "$LOCAL_BIN/e621-cruiser" "$LOCAL_BIN/e621cruiser"

echo "==> Installing application icons..."
ICON_SCALABLE="$HOME/.local/share/icons/hicolor/scalable/apps"
ICON_PNG="$HOME/.local/share/icons/hicolor/256x256/apps"
mkdir -p "$ICON_SCALABLE" "$ICON_PNG"

cp "$SCRIPT_DIR/assets/e621-cruiser.svg" "$ICON_SCALABLE/e621-cruiser.svg"
cp "$SCRIPT_DIR/assets/e621-cruiser.png" "$ICON_PNG/e621-cruiser.png"

echo "==> Installing desktop menu entry..."
APPS_DIR="$HOME/.local/share/applications"
mkdir -p "$APPS_DIR"

DESKTOP_FILE="$APPS_DIR/e621-cruiser.desktop"
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=E621 Cruiser
Comment=Browse and download posts, pools, and favorites from e621
Exec=$LOCAL_BIN/e621-cruiser
Icon=e621-cruiser
Terminal=false
Categories=Network;Graphics;Viewer;
Keywords=e621;art;booru;image;crawler;cruiser;gallery;
StartupNotify=true
StartupWMClass=e621-cruiser
EOF
chmod +x "$DESKTOP_FILE"

if [ -d "$HOME/Desktop" ]; then
    echo "==> Placing shortcut on Desktop..."
    cp "$DESKTOP_FILE" "$HOME/Desktop/e621-cruiser.desktop"
    chmod +x "$HOME/Desktop/e621-cruiser.desktop"
    if command -v gio >/dev/null 2>&1; then
        gio set "$HOME/Desktop/e621-cruiser.desktop" metadata::trusted true 2>/dev/null || true
    fi
fi

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPS_DIR" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
fi

echo ""
echo "=========================================================="
echo "  E621 Cruiser installed successfully!"
echo "  • Application menu: E621 Cruiser"
[ -d "$HOME/Desktop" ] && echo "  • Desktop shortcut: ~/Desktop/e621-cruiser.desktop"
echo "  • Binary:           $LOCAL_BIN/e621-cruiser"
echo "=========================================================="
