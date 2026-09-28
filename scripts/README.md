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
