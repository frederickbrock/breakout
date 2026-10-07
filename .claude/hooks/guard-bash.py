#!/usr/bin/env python3
"""PreToolUse Bash guard: blocks destructive commands for every agent role.

Claude Code runs this before each Bash tool call with the hook JSON on stdin
(`tool_input.command`, `cwd`). Exit 2 blocks the call; the one-line reason on
stderr is shown to the agent. Anything else (exit 0) lets the normal
permission rules decide.

Blocked (each part of a compound command is checked: &&, ||, ;, |, newlines):
- `git push` with --force, -f (also in a short-flag cluster) or a +refspec
- `git push --force-with-lease` / `--force-if-includes` to master/main, or
  from master/main when no refspec is given
- `git reset --hard`, `git clean -f…`, and whole-tree discards
  (`git checkout [--] .`, `git restore .`); single paths stay allowed
- `rm -r` on /, ~ / $HOME, any .git or .beads path, or anything outside the
  repo root and /tmp / $TMPDIR
- `br delete`

Stdlib only. Tests: `python3 -m unittest discover -s .claude/hooks`.
"""
from __future__ import annotations

import json
import os
import re
import shlex
import subprocess
import sys
from pathlib import Path

TRUNKS = {"master", "main"}


def split_commands(command: str) -> list[str]:
    """Split a shell line on &&, ||, ;, |, & and newlines outside quotes."""
    parts, buf, quote, i = [], [], None, 0
    while i < len(command):
        c = command[i]
        if quote:
            buf.append(c)
            if c == quote:
                quote = None
            elif c == "\\" and quote == '"' and i + 1 < len(command):
                buf.append(command[i + 1])
                i += 1
        elif c in "'\"":
            quote = c
            buf.append(c)
        elif c == "\\" and i + 1 < len(command):
            buf.append(command[i:i + 2])
            i += 1
        elif c in ";|&\n":
            parts.append("".join(buf))
            buf = []
        else:
            buf.append(c)
        i += 1
    parts.append("".join(buf))
    return [p.strip() for p in parts if p.strip()]


def words(part: str) -> list[str]:
    try:
        w = shlex.split(part, comments=True)
    except ValueError:
        w = part.split()
    # Drop leading env assignments (FOO=bar cmd) and wrappers that run the rest.
    while w and (re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", w[0]) or w[0] in ("env", "command", "nice", "time", "exec")):
        w = w[1:]
    return w


def git_args(w: list[str]) -> tuple[str | None, list[str], str | None]:
    """(subcommand, its args, -C dir) for a git invocation."""
    i, cdir = 1, None
    while i < len(w) and w[i].startswith("-"):
        if w[i] == "-C" and i + 1 < len(w):
            cdir = w[i + 1]
            i += 2
            continue
        if w[i] in ("-c",) and i + 1 < len(w):
            i += 2
            continue
        i += 1
    if i >= len(w):
        return None, [], cdir
    return w[i], w[i + 1:], cdir


def current_branch(cwd: str) -> str | None:
    try:
        out = subprocess.run(["git", "-C", cwd, "rev-parse", "--abbrev-ref", "HEAD"],
                             capture_output=True, text=True, timeout=5)
        if out.returncode != 0:
            return None
        return out.stdout.strip() or None
    except (OSError, subprocess.SubprocessError):
        return None


def repo_root(cwd: str) -> str:
    try:
        out = subprocess.run(["git", "-C", cwd, "rev-parse", "--show-toplevel"],
                             capture_output=True, text=True, timeout=5)
        if out.returncode == 0 and out.stdout.strip():
            return out.stdout.strip()
    except (OSError, subprocess.SubprocessError):
        pass
    return cwd


def check_git_push(args: list[str], cwd: str, branch_of) -> str | None:
    lease = False
    positional = []
    for a in args:
        if a in ("--force",) or (a.startswith("--force") and not a.startswith(("--force-with-lease", "--force-if-includes"))):
            return "git push --force rewrites remote history; never allowed (use --force-with-lease on a task branch)"
        if a.startswith(("--force-with-lease", "--force-if-includes")):
            lease = True
            continue
        if re.fullmatch(r"-[A-Za-z]+", a) and "f" in a[1:]:
            return "git push -f rewrites remote history; never allowed (use --force-with-lease on a task branch)"
        if a.startswith("-"):
            continue
        positional.append(a)
    refspecs = positional[1:]  # positional[0] is the remote
    for r in refspecs:
        if r.startswith("+"):
            return f"git push {r}: a +refspec force-pushes; never allowed"
    if lease:
        targets = [r.split(":", 1)[-1] for r in refspecs]
        targets = [t.removeprefix("refs/heads/") for t in targets]
        if not targets:
            b = branch_of(cwd)
            targets = [b] if b else []
            if not targets:
                return "git push --force-with-lease with no refspec and an unknown current branch"
        hit = [t for t in targets if t in TRUNKS or t == "HEAD" and branch_of(cwd) in TRUNKS]
        if hit:
            return f"git push --force-with-lease to {hit[0]}: force-pushing the trunk is never allowed"
    return None


def under(path: Path, root: Path) -> bool:
    return path == root or root in path.parents


def check_rm(args: list[str], cwd: str) -> str | None:
    flags = [a for a in args if a.startswith("-") and a != "--"]
    recursive = any(a in ("-r", "-R", "--recursive") or (not a.startswith("--") and re.search("[rR]", a[1:]))
                    for a in flags)
    if not recursive:
        return None
    targets = [a for a in args if not a.startswith("-")]
    home = Path(os.path.expanduser("~")).resolve()
    root = Path(repo_root(cwd)).resolve()
    safe_tmp = [Path("/tmp").resolve()]
    if os.environ.get("TMPDIR"):
        safe_tmp.append(Path(os.environ["TMPDIR"]).resolve())
    for t in targets:
        raw = t
        if raw in ("/", "~", "~/", "$HOME", "${HOME}", "/*", "~/*", "*", "."):
            if raw in ("*", "."):
                # The repo root itself (or everything in it).
                if Path(cwd).resolve() == root:
                    return f"rm -r {raw} at the repo root would delete the whole checkout"
                continue
            return f"rm -r {raw}: refusing to delete / or the home directory"
        expanded = os.path.expandvars(os.path.expanduser(raw))
        p = Path(expanded if os.path.isabs(expanded) else os.path.join(cwd, expanded))
        try:
            p = p.resolve()
        except OSError:
            pass
        parts = set(p.parts)
        if ".git" in parts or ".beads" in parts or p.name in (".git", ".beads"):
            return f"rm -r {raw}: .git and .beads are never deleted by agents"
        if p == home or p == Path("/") or p == root:
            return f"rm -r {raw}: refusing to delete the home directory, / or the repo root"
        if not (under(p, root) or any(under(p, s) for s in safe_tmp)):
            return f"rm -r {raw}: outside the repo and /tmp; not allowed"
    return None


def check_part(part: str, cwd: str, branch_of=current_branch) -> str | None:
    w = words(part)
    if not w:
        return None
    cmd = os.path.basename(w[0])
    if cmd == "git":
        sub, args, cdir = git_args(w)
        here = os.path.join(cwd, cdir) if cdir else cwd
        if sub == "push":
            return check_git_push(args, here, branch_of)
        if sub == "reset" and "--hard" in args:
            return "git reset --hard throws away work; never allowed (commit or stash with a name instead)"
        if sub == "clean" and any(a == "--force" or re.fullmatch(r"-[A-Za-z]*f[A-Za-z]*", a) for a in args):
            return "git clean -f deletes untracked files; never allowed"
        if sub in ("checkout", "restore"):
            paths = args[args.index("--") + 1:] if "--" in args else [a for a in args if not a.startswith("-")]
            if sub == "checkout" and "--" not in args:
                paths = [a for a in paths if a in (".", ":/", "*")]  # `git checkout <branch>` is fine
            if any(p in (".", ":/", "*", "./") for p in paths):
                return f"git {sub} {' '.join(args)}: discards every change in the tree; name the files instead"
        return None
    if cmd == "rm":
        return check_rm(w[1:], cwd)
    if cmd == "br" and len(w) > 1 and w[1] == "delete":
        return "br delete removes issues from the shared queue; close them instead (br close)"
    return None


def check_command(command: str, cwd: str, branch_of=current_branch) -> str | None:
    """The first reason to block `command`, or None to let it through."""
    here = cwd
    for part in split_commands(command):
        w = words(part)
        if w[:1] == ["cd"]:
            target = os.path.expanduser(w[1]) if len(w) > 1 else os.path.expanduser("~")
            here = target if os.path.isabs(target) else os.path.normpath(os.path.join(here, target))
            continue
        reason = check_part(part, here, branch_of)
        if reason:
            return reason
    return None


def main() -> int:
    try:
        data = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        return 0  # not our input: let the normal rules decide
    if data.get("tool_name") not in (None, "Bash"):
        return 0
    command = (data.get("tool_input") or {}).get("command") or ""
    cwd = data.get("cwd") or os.getcwd()
    reason = check_command(command, cwd)
    if reason:
        print(f"guard-bash: blocked — {reason}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
