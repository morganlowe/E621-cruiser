#!/usr/bin/env bash
# E621 Grabber Installer
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

echo "==> Installing application icons..."
ICON_SCALABLE="$HOME/.local/share/icons/hicolor/scalable/apps"
ICON_PNG="$HOME/.local/share/icons/hicolor/256x256/apps"
mkdir -p "$ICON_SCALABLE" "$ICON_PNG"

cp "$SCRIPT_DIR/assets/e621-grabber.svg" "$ICON_SCALABLE/e621-grabber.svg"
cp "$SCRIPT_DIR/assets/e621-grabber.png" "$ICON_PNG/e621-grabber.png"

echo "==> Installing desktop menu entry..."
APPS_DIR="$HOME/.local/share/applications"
mkdir -p "$APPS_DIR"

DESKTOP_FILE="$APPS_DIR/e621-grabber.desktop"
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=E621 Grabber
Comment=Browse and download posts, pools, and favorites from e621
Exec=$SCRIPT_DIR/run.sh
Icon=e621-grabber
Terminal=false
Categories=Network;Graphics;Viewer;
Keywords=e621;art;booru;image;crawler;grabber;
StartupNotify=true
StartupWMClass=e621grabber
Path=$SCRIPT_DIR
EOF
chmod +x "$DESKTOP_FILE"

# Desktop shortcut if ~/Desktop exists
if [ -d "$HOME/Desktop" ]; then
    echo "==> Placing shortcut on Desktop..."
    cp "$DESKTOP_FILE" "$HOME/Desktop/e621-grabber.desktop"
    chmod +x "$HOME/Desktop/e621-grabber.desktop"
    if command -v gio >/dev/null 2>&1; then
        gio set "$HOME/Desktop/e621-grabber.desktop" metadata::trusted true 2>/dev/null || true
    fi
fi

# Command-line link in ~/.local/bin
LOCAL_BIN="$HOME/.local/bin"
mkdir -p "$LOCAL_BIN"
ln -sf "$SCRIPT_DIR/run.sh" "$LOCAL_BIN/e621-grabber"

# Update desktop & icon caches if tools exist
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPS_DIR" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
fi

echo ""
echo "=========================================================="
echo "  E621 Grabber installed successfully!"
echo "  • Application menu: E621 Grabber"
[ -d "$HOME/Desktop" ] && echo "  • Desktop shortcut: ~/Desktop/e621-grabber.desktop"
echo "  • Command line:     e621-grabber (via ~/.local/bin)"
echo "=========================================================="
