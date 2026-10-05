# Agent scripts

## Running the native game: `native-run.sh`

Every agent runs the desktop build **only** through this script, never with a bare `cargo run`. The whole agent team shares one X display (WSLg `DISPLAY=:0`). Before this script, concurrent runs from different roles drove and killed each other's windows, and left orphaned `sim` processes (sim-x2s).

```bash
scripts/native-run.sh --label <issue-id>              # fail fast if the display is busy
scripts/native-run.sh --label <issue-id> --wait 120   # or wait up to 120 s for it
scripts/native-run.sh --label <issue-id> -- --release # extra args go to cargo run
```

What the script does:

- **Exclusive display.** It holds a `flock` on `${XDG_RUNTIME_DIR:-/tmp}/breakout-display-<DISPLAY>.lock` for the whole run. If another agent holds it, the script exits with **75** and names the holder:
  `native-run: display :0 busy: tester@tester (sim-rdl.2) pid 1234 since 23:10:05`
  A duplicate role instance gets this refusal instead of silently cross-talking.
- **Unique window.** The window is titled `Breakout [<actor> <label>]` (`$AGENT_ACTOR`, or `user@worktree`). The game reads the title from `BREAKOUT_WINDOW_TITLE`, native only. The script prints:
  ```
  native-run: window title: Breakout [tester@tester sim-rdl.2]
  native-run: window id: 8388610
  ```
- **Clean teardown.** The game runs in its own process group. Closing the window, Ctrl-C (exit 130), SIGTERM (143) or `timeout` (124) kills the whole group and releases the lock. Afterwards `pgrep -x sim` is empty.

## Driving and screenshotting

Always target the **printed window id**. Never search for a window named plain "Breakout": another agent's window may match.

```bash
WID=8388610                                     # from "native-run: window id:"
xdotool key --window "$WID" Return              # e.g. Start on the main menu
xdotool key --window "$WID" p                   # pause
xdotool mousemove --window "$WID" 450 300       # cursor to window coords
xdotool click --window "$WID" 1                 # left click
import -window "$WID" shot.png                  # screenshot (ImageMagick); or: xwd -id "$WID" -out shot.xwd
scripts/window-close.py "$WID"                  # close like the title-bar button (clean exit 0)
```

WSLg has no EWMH window manager, so `wmctrl -l` / `wmctrl -c` don't work there, and `xdotool windowclose` destroys the window rather than asking it to close. Use `scripts/window-close.py` to test "closing the window".

## Who launches the game

Only the **tester** runs the native game, and only through `native-run.sh`. The **coder** does not launch it: its checks are `cargo build`, `cargo clippy`, `cargo test` and `trunk build`. The one exception is verifying changes to this tooling, and then only through `native-run.sh`.

## Merging a finished PR: `pr-merge`

`scripts/pr-merge <pr-number>` is the only way an agent (the **pr-manager**) merges a code PR;
plain `gh pr merge` stays denied. It squash-merges with `--delete-branch` and prints the merge
commit SHA, or refuses with `pr-merge: refusing #N — <reason>` (exit 1) unless **all** of these hold:

- `gates.merge: auto` in `.claude/workflow.yaml`
- the PR is open, not a draft, and its base is the trunk
- the required checks `native`, `web` and `test` passed, and nothing else is pending or failing
- GitHub reports it mergeable
- no review requests changes, and no unresolved review thread starts with `blocking:` (`nit:` threads
  stay open by design and don't block)
- the beads issue in the title (`<type>(<id>): ...`) is open, at `stage:pr`, not `needs-human`,
  and its `external_ref` is this PR

It merges with `--match-head-commit` set to the head it checked, so a push in between is refused
by GitHub. Any API error is a refusal. Install: `ln -s <main>/scripts/pr-merge ~/.local/bin/pr-merge`.
Tests (fake `gh`/`br`, no network): `python3 -m unittest discover -s scripts -p 'test_*.py'`.

## Web smoke test: `web-smoke.sh`

`scripts/web-smoke.sh [dist-dir]` loads the wasm build in headless Chromium (software WebGL2)
and fails on anything that would break the web version:
- **Phase 1, menu:** waits for the game's `steelbreak-ready` (splash `.done`), or fails on
  `.failed` or a 90 s timeout. Screenshot `menu.png`.
- **Phase 2, level:** presses Enter on the focused Start button, waits for the wasm-only
  `steelbreak-level-started` event, runs 5 s. Screenshot `level.png`.
- **Throughout:** any console error, any message containing "panic", uncaught exceptions or
  rejections, and failed requests (HTTP >= 400) fail the run. The exception is entries in
  `scripts/web-smoke.allow`: one narrow regex per known-harmless message, each commented.

Without a dist dir it first runs `trunk build --release --public-url /breakout/` (like CI). Output
goes to `$WEB_SMOKE_OUT` or a temp dir and is printed at the end. It uses `/usr/bin/chromium` and
gets Playwright through `uv run` (nothing installed globally); `CHROMIUM=<path>` or
`CHROMIUM=bundled` picks another browser. CI runs it as the `web-smoke` job after `web`, and
`deploy-pages` waits for it.
