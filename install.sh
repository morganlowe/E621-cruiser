#!/usr/bin/env bash
# E621 Cruiser Installer
# Installs application menu entry, desktop shortcut, icons, and command line launcher.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "==> Setting up environment..."
chmod +x "$SCRIPT_DIR/run.sh"

if [ ! -x .venv/bin/python ]; then
    echo "Creating virtual environment..."
    python3 -m venv --system-site-packages .venv
    .venv/bin/python -c "import PySide6, requests" 2>/dev/null || .venv/bin/pip install -r requirements.txt
fi

echo "==> Cleaning up any legacy grabber entries..."
rm -f "$HOME/.local/share/applications/e621-grabber.desktop"
rm -f "$HOME/Desktop/e621-grabber.desktop"
rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/e621-grabber.svg"
rm -f "$HOME/.local/share/icons/hicolor/256x256/apps/e621-grabber.png"
rm -f "$HOME/.local/bin/e621-grabber"

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
Exec=$SCRIPT_DIR/run.sh
Icon=e621-cruiser
Terminal=false
Categories=Network;Graphics;Viewer;
Keywords=e621;art;booru;image;crawler;cruiser;gallery;
StartupNotify=true
StartupWMClass=e621cruiser
Path=$SCRIPT_DIR
EOF
chmod +x "$DESKTOP_FILE"

# Desktop shortcut if ~/Desktop exists
if [ -d "$HOME/Desktop" ]; then
    echo "==> Placing shortcut on Desktop..."
    cp "$DESKTOP_FILE" "$HOME/Desktop/e621-cruiser.desktop"
    chmod +x "$HOME/Desktop/e621-cruiser.desktop"
    if command -v gio >/dev/null 2>&1; then
        gio set "$HOME/Desktop/e621-cruiser.desktop" metadata::trusted true 2>/dev/null || true
    fi
fi

# Command-line link in ~/.local/bin
LOCAL_BIN="$HOME/.local/bin"
mkdir -p "$LOCAL_BIN"
ln -sf "$SCRIPT_DIR/run.sh" "$LOCAL_BIN/e621-cruiser"
ln -sf "$SCRIPT_DIR/run.sh" "$LOCAL_BIN/e621cruiser"

# Update desktop & icon caches if tools exist
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
echo "  • Command line:     e621-cruiser (via ~/.local/bin)"
echo "=========================================================="
