#!/usr/bin/env bash
# E621 Cruiser Uninstaller
# Removes desktop entries, icons, and launcher links.
set -euo pipefail

echo "==> Removing E621 Cruiser shortcuts and icons..."
rm -f "$HOME/.local/share/applications/e621-cruiser.desktop"
rm -f "$HOME/Desktop/e621-cruiser.desktop"
rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/e621-cruiser.svg"
rm -f "$HOME/.local/share/icons/hicolor/256x256/apps/e621-cruiser.png"
rm -f "$HOME/.local/bin/e621-cruiser"
rm -f "$HOME/.local/bin/e621cruiser"

# Also clean legacy grabber entries if present
rm -f "$HOME/.local/share/applications/e621-grabber.desktop"
rm -f "$HOME/Desktop/e621-grabber.desktop"
rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/e621-grabber.svg"
rm -f "$HOME/.local/share/icons/hicolor/256x256/apps/e621-grabber.png"
rm -f "$HOME/.local/bin/e621-grabber"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
fi

echo "E621 Cruiser desktop shortcuts and launcher removed."
