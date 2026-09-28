#!/usr/bin/env bash
# Launch the native game for an agent, exclusively and identifiably.
#
#   scripts/native-run.sh [--label <text>] [--wait <secs>] [-- <extra cargo args>]
#
# - Takes an exclusive lock on the X display ($DISPLAY) for the whole run, so
#   two agents never drive the same display at once. When the display is busy
#   it fails fast (or waits up to --wait seconds) and names the current holder.
# - Gives the window a unique title, "Breakout [<actor> <label>]", and prints
#   it plus the X window id, so drivers target that exact window by id.
# - Runs cargo and the game in their own process group, and kills the whole
#   group on exit, Ctrl-C, SIGTERM or `timeout`. Nothing is left reparented to
#   init, and the lock is released.
#
# See scripts/README.md for driving and screenshotting the window.

set -euo pipefail

usage() {
    sed -n '4p' "$0" | sed 's/^#  *//'
}

# die <message> [exit code]
die() {
    echo "native-run: $1" >&2
    exit "${2:-1}"
}

label=""
wait_secs=0
extra=()
while (($#)); do
    case "$1" in
    --label)
        [[ $# -ge 2 ]] || die "--label needs a value" 2
        label="$2"
        shift 2
        ;;
    --wait)
        [[ $# -ge 2 && $2 =~ ^[0-9]+$ ]] || die "--wait needs a number of seconds" 2
        wait_secs="$2"
        shift 2
        ;;
    --)
        shift
        extra=("$@")
        break
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *) die "unknown argument: $1 (usage: $(usage))" 2 ;;
    esac
done

[[ -n ${DISPLAY:-} ]] || die "DISPLAY is not set"
command -v xdotool >/dev/null || die "xdotool is not installed (needed to find the window id)"
command -v flock >/dev/null || die "flock is not installed"

actor="${AGENT_ACTOR:-$(id -un)@$(basename "$PWD")}"
title="Breakout [${actor}${label:+ $label}]"

lock_dir="${XDG_RUNTIME_DIR:-/tmp}"
lock="${lock_dir}/breakout-display-${DISPLAY//[^A-Za-z0-9]/_}.lock"
holder="${lock}.holder"

# fd 9 holds the lock for the life of this script. The lock file itself is
# never deleted (that would race with other waiters); only the holder record
# changes.
exec 9>>"$lock"
if ((wait_secs > 0)); then
    lock_args=(-w "$wait_secs")
else
    lock_args=(-n)
fi
if ! flock "${lock_args[@]}" 9; then
    busy_by="$(cat "$holder" 2>/dev/null || echo "unknown holder")"
    die "display ${DISPLAY} busy: ${busy_by}" 75
fi
printf '%s (%s) pid %s since %s\n' "$actor" "${label:-no label}" "$$" "$(date +%T)" >"$holder"

game_pgid=""
search_pid=""
cleanup() {
    trap - EXIT INT TERM
    [[ -n $search_pid ]] && kill "$search_pid" 2>/dev/null || true
    if [[ -n $game_pgid ]] && kill -0 -- "-$game_pgid" 2>/dev/null; then
        kill -TERM -- "-$game_pgid" 2>/dev/null || true
        for _ in 1 2 3 4 5 6 7 8 9 10; do
            kill -0 -- "-$game_pgid" 2>/dev/null || break
            sleep 0.3
        done
        kill -KILL -- "-$game_pgid" 2>/dev/null || true
    fi
    [[ -n ${id_file:-} ]] && rm -f "$id_file"
    : >"$holder"
    flock -u 9
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# setsid: cargo (and the game it spawns) lead their own process group, so the
# whole tree can be killed at once. From a non-interactive script the
# background job isn't a group leader, so setsid doesn't fork and $! is the
# group id.
BREAKOUT_WINDOW_TITLE="$title" setsid cargo run "${extra[@]}" &
game_pgid=$!
echo "native-run: window title: ${title}"

# Look for the window in the background, so signals are still handled while
# cargo is building; poll for the game exiting and the window id showing up.
id_file="$(mktemp)"
pattern="^$(printf '%s' "$title" | sed 's/[][\.*^$(){}+?|/]/\\&/g')\$"
xdotool search --sync --limit 1 --name "$pattern" >"$id_file" 2>/dev/null &
search_pid=$!

while kill -0 "$game_pgid" 2>/dev/null; do
    if [[ -n $search_pid ]] && ! kill -0 "$search_pid" 2>/dev/null; then
        echo "native-run: window id: $(head -n1 "$id_file")"
        search_pid=""
    fi
    sleep 0.5
done

status=0
wait "$game_pgid" || status=$?
exit "$status"
