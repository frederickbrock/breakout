#!/usr/bin/env bash
# web-smoke.sh [dist-dir] — run the web smoke test locally (see scripts/web_smoke.py).
# Without a dist dir it builds one like CI does: trunk build --release --public-url /breakout/.
# Output (menu.png, level.png, console.log) goes to $WEB_SMOKE_OUT, default a temp dir.
set -euo pipefail
cd "$(dirname "$0")/.."
dist=${1:-}
if [[ -z $dist ]]; then
  trunk build --release --public-url /breakout/
  dist=dist
fi
out=${WEB_SMOKE_OUT:-$(mktemp -d -t web-smoke-XXXXXX)}
exec uv run scripts/web_smoke.py "$dist" --prefix /breakout/ --out "$out"
