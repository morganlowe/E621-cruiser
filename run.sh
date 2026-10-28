#!/usr/bin/env bash
# Launch E621 Cruiser. Creates a venv on first run (reuses system PySide6/requests if present).
set -euo pipefail
cd "$(dirname "$0")"
if [ ! -x .venv/bin/python ]; then
    python3 -m venv --system-site-packages .venv
    .venv/bin/python -c "import PySide6, requests" 2>/dev/null || .venv/bin/pip install -r requirements.txt
fi
exec .venv/bin/python -m e621cruiser "$@"
