#!/usr/bin/env bash
# Build (if needed) and launch E621 Cruiser.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
BIN="$SCRIPT_DIR/target/release/e621-cruiser"
if [ ! -x "$BIN" ]; then
    cargo build --release
fi
exec "$BIN" "$@"
