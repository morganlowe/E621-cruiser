#!/usr/bin/env bash
set -euo pipefail
export PYTHONPATH="/usr/share/e621-cruiser${PYTHONPATH:+:$PYTHONPATH}"
exec python3 -m e621cruiser "$@"
