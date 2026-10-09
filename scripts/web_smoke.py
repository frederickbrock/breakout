#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["playwright==1.55.0"]
# ///
"""Web smoke test: load the wasm build in headless Chromium and check it starts.

  uv run scripts/web_smoke.py <dist-dir> [--prefix /breakout/] [--out DIR]

Serves <dist-dir> under <prefix> (the same --public-url the build used) on
127.0.0.1, opens it with software WebGL2 (SwiftShader: CI runners have no GPU)
and runs two phases:

1. **menu:** wait for the game's `steelbreak-ready` (the splash gets `.done`;
   `.failed` or the timeout fails), then screenshot `menu.png`.
2. **level:** press Enter (the focused Start button), wait for the wasm-only
   `steelbreak-level-started` event, let it run a few seconds, then
   screenshot `level.png`.

Throughout, any console error, any console message containing "panic", any
uncaught exception or unhandled rejection, and any failed request (HTTP >= 400
or a network error) fails the run, unless it matches a regex in
`scripts/web-smoke.allow`. The screenshots and `console.log` are written to
--out even when the run fails. Exit 0 = pass, 1 = fail.

Chromium: $CHROMIUM (a path, or `bundled`), else /usr/bin/chromium if present,
else Playwright's own (install it with
`uv run --with playwright==1.55.0 playwright install chromium`).
"""
from __future__ import annotations

import argparse
import functools
import http.server
import os
import re
import sys
import tempfile
import threading
import time
from pathlib import Path

from playwright.sync_api import Error as PlaywrightError
from playwright.sync_api import sync_playwright

HERE = Path(__file__).resolve().parent
ALLOW_FILE = HERE / "web-smoke.allow"
CHROMIUM_ARGS = [
    "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader",
    "--ignore-gpu-blocklist",
    "--use-gl=angle",
]
WATCH_EVENTS = """
window.__smoke = {ready: false, levelStarted: 0};
window.addEventListener('steelbreak-ready', () => { window.__smoke.ready = true; });
window.addEventListener('steelbreak-level-started', () => { window.__smoke.levelStarted += 1; });
"""


def load_allowlist(path: Path) -> list[re.Pattern]:
    if not path.is_file():
        return []
    out = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            out.append(re.compile(line))
    return out


class Quiet(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map,
                      ".wasm": "application/wasm", ".js": "text/javascript", ".ron": "text/plain"}

    def log_message(self, *args):
        pass


def serve(dist: Path, prefix: str):
    """Serve dist at http://127.0.0.1:<port><prefix> (a temp dir holding a
    symlink named after the prefix). Returns (server, base url, temp dir)."""
    root = Path(tempfile.mkdtemp(prefix="web-smoke-"))
    name = prefix.strip("/")
    target = root / name if name else root
    if name:
        target.symlink_to(dist.resolve(), target_is_directory=True)
    handler = functools.partial(Quiet, directory=str(root if name else dist.resolve()))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{server.server_address[1]}/{name + '/' if name else ''}"
    return server, base, root


def pick_chromium() -> str | None:
    """$CHROMIUM (a path, or `bundled` for Playwright's own), else
    /usr/bin/chromium if present, else Playwright's own."""
    choice = os.environ.get("CHROMIUM", "")
    if choice == "bundled":
        return None
    if choice:
        return choice
    return "/usr/bin/chromium" if Path("/usr/bin/chromium").exists() else None


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dist", type=Path)
    ap.add_argument("--prefix", default="/breakout/")
    ap.add_argument("--out", type=Path, default=Path("web-smoke-out"))
    ap.add_argument("--menu-timeout", type=float, default=90.0)
    ap.add_argument("--level-timeout", type=float, default=60.0)
    ap.add_argument("--level-seconds", type=float, default=5.0)
    args = ap.parse_args(argv)
    if not (args.dist / "index.html").is_file():
        print(f"web-smoke: no index.html in {args.dist}", file=sys.stderr)
        return 1
    args.out.mkdir(parents=True, exist_ok=True)
    allow = load_allowlist(ALLOW_FILE)
    log: list[str] = []
    problems: list[str] = []
    t0 = time.monotonic()

    def note(kind: str, text: str):
        line = f"[{time.monotonic() - t0:6.1f}s] {kind}: {text}"
        log.append(line)
        bad = kind in ("pageerror", "requestfailed", "http") or (
            kind.startswith("console") and (kind == "console.error" or "panic" in text.lower()))
        if bad and not any(p.search(text) for p in allow):
            problems.append(line)

    server, base, root = serve(args.dist, args.prefix)
    phase = "start"
    try:
        with sync_playwright() as pw:
            browser = pw.chromium.launch(headless=True, executable_path=pick_chromium(), args=CHROMIUM_ARGS)
            page = browser.new_page(viewport={"width": 1280, "height": 720})
            page.on("console", lambda m: note(f"console.{m.type}", m.text))
            page.on("pageerror", lambda e: note("pageerror", str(e)))
            page.on("requestfailed", lambda r: note("requestfailed", f"{r.url} ({r.failure})"))
            page.on("response", lambda r: r.status >= 400 and note("http", f"{r.status} {r.url}"))
            page.add_init_script(WATCH_EVENTS)
            try:
                phase = "menu"
                page.goto(base, wait_until="load")
                page.wait_for_function(
                    "() => { const s = document.getElementById('splash');"
                    " return s && (s.classList.contains('done') || s.classList.contains('failed')); }",
                    timeout=args.menu_timeout * 1000)
                if page.evaluate("document.getElementById('splash').classList.contains('failed')"):
                    text = page.evaluate("document.getElementById('splash-error-text')?.textContent || ''")
                    problems.append(f"menu: the splash failed: {text}")
                page.wait_for_timeout(500)
                page.screenshot(path=str(args.out / "menu.png"))
                note("phase", f"menu reached in {time.monotonic() - t0:.1f}s")
                if not problems:
                    phase = "level"
                    page.locator("canvas").first.focus()
                    page.keyboard.press("Enter")
                    page.wait_for_function("() => window.__smoke.levelStarted > 0",
                                           timeout=args.level_timeout * 1000)
                    page.wait_for_timeout(args.level_seconds * 1000)
                    page.screenshot(path=str(args.out / "level.png"))
                    note("phase", f"level started and ran {args.level_seconds:g}s")
            except PlaywrightError as e:
                problems.append(f"{phase}: {str(e).splitlines()[0]}")
                try:
                    page.screenshot(path=str(args.out / f"{phase}-failure.png"))
                except PlaywrightError:
                    pass
            finally:
                browser.close()
    finally:
        server.shutdown()
        (args.out / "console.log").write_text("\n".join(log) + "\n")
        try:
            (root / args.prefix.strip("/")).unlink(missing_ok=True)
            root.rmdir()
        except OSError:
            pass
    if problems:
        print(f"web-smoke: FAIL ({phase}) after {time.monotonic() - t0:.1f}s", file=sys.stderr)
        for p in problems:
            print(f"  {p}", file=sys.stderr)
        print(f"  screenshots + console.log in {args.out}", file=sys.stderr)
        return 1
    print(f"web-smoke: pass in {time.monotonic() - t0:.1f}s — {args.out}/menu.png, level.png, console.log")
    return 0


if __name__ == "__main__":
    sys.exit(main())
