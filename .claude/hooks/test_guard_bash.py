"""Tests for guard-bash.py and the committed .claude/settings.json.

Run from the repo root: python3 -m unittest discover -s .claude/hooks -p 'test_*.py'
"""
from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import tempfile
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "guard-bash.py"
SETTINGS = HERE.parent / "settings.json"
spec = importlib.util.spec_from_file_location("guard_bash", SCRIPT)
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)

# The hook resolves rm targets against the repo root (`git rev-parse`), so the
# checks run "in" this checkout (nothing is executed). It isn't under /tmp,
# which the hook treats as safe.
REPO = HERE.parent.parent


def on(branch):
    return lambda _cwd: branch


BLOCK = [
    # force pushes, any spelling
    ("git push --force origin feat/x", "feature"),
    ("git push origin feat/x --force", "feature"),
    ("git push -f origin feat/x", "feature"),
    ("git push -uf origin feat/x", "feature"),
    ("git push --force=origin", "feature"),
    ("git push origin +feat/x", "feature"),
    ("git push origin +HEAD:feat/x", "feature"),
    # force-with-lease to the trunk
    ("git push --force-with-lease origin master", "feature"),
    ("git push --force-with-lease origin HEAD:main", "feature"),
    ("git push --force-with-lease", "master"),
    ("git push --force-if-includes --force-with-lease origin refs/heads/master", "feature"),
    # discards
    ("git reset --hard", "feature"),
    ("git reset --hard origin/master", "feature"),
    ("git clean -fd", "feature"),
    ("git clean -xdf", "feature"),
    ("git clean --force", "feature"),
    ("git checkout -- .", "feature"),
    ("git checkout .", "feature"),
    ("git restore .", "feature"),
    ("git restore --staged --worktree :/", "feature"),
    # rm -r on dangerous targets
    ("rm -rf /", "feature"),
    ("rm -rf ~", "feature"),
    ("rm -r $HOME", "feature"),
    ("rm -rf .git", "feature"),
    ("rm -rf .beads", "feature"),
    ("rm -rf src/../.git/hooks", "feature"),
    ("rm -rf /etc/foo", "feature"),
    ("rm -R ../other-repo", "feature"),
    # the queue
    ("br delete sim-123", "feature"),
    # compound commands: any part blocks
    ("cargo test && git push --force origin feat/x", "feature"),
    ("cd /tmp && git reset --hard", "feature"),
    ("git status; rm -rf ~", "feature"),
    ("echo ok | git clean -f", "feature"),
    ("FOO=1 git push -f origin x", "feature"),
    ("git -C /some/dir reset --hard", "feature"),
]

ALLOW = [
    ("git push --force-with-lease origin feat/sim-1-thing", "feature"),
    ("git push --force-with-lease", "feat/sim-1-thing"),
    ("git push -u origin feat/x", "feature"),
    ("git push origin master", "master"),
    ("git checkout -- .beads/issues.jsonl", "master"),
    ("git checkout feat/x", "feature"),
    ("git checkout -b feat/new origin/master", "feature"),
    ("git restore src/main.rs", "feature"),
    ("git reset --soft HEAD~1", "feature"),
    ("git reset HEAD src/main.rs", "feature"),
    ("git clean -n", "feature"),
    ("git status && git diff --stat", "feature"),
    ("cargo build && cargo clippy --all-targets -- -D warnings && cargo test", "feature"),
    ("br update sim-1 --claim --actor coder@coder", "feature"),
    ("br close sim-1", "feature"),
    ("gh pr view 12 --json state", "feature"),
    ("rm -rf target/debug", "feature"),
    ("rm -rf /tmp/scratch-dir", "feature"),
    ("rm -f stray.txt", "feature"),
    ("rm -r scripts/__pycache__", "feature"),
    ("echo 'git push --force' > notes.txt", "feature"),
    ("git commit -m 'do not git reset --hard'", "feature"),
]


class Rules(unittest.TestCase):
    def test_blocks(self):
        for cmd, branch in BLOCK:
            with self.subTest(cmd=cmd):
                reason = guard.check_command(cmd, str(REPO), branch_of=on(branch))
                self.assertIsNotNone(reason, f"should block: {cmd}")
                self.assertNotIn("\n", reason, "one-line reason")

    def test_allows(self):
        for cmd, branch in ALLOW:
            with self.subTest(cmd=cmd):
                self.assertIsNone(guard.check_command(cmd, str(REPO), branch_of=on(branch)), cmd)


class HookProtocol(unittest.TestCase):
    def run_hook(self, payload):
        return subprocess.run([sys.executable, str(SCRIPT)], input=json.dumps(payload),
                              capture_output=True, text=True)

    def test_a_blocked_command_exits_2_with_a_one_line_reason(self):
        r = self.run_hook({"tool_name": "Bash", "cwd": str(REPO),
                           "tool_input": {"command": "git reset --hard"}})
        self.assertEqual(r.returncode, 2)
        self.assertTrue(r.stderr.startswith("guard-bash: blocked — git reset --hard"), r.stderr)
        self.assertEqual(r.stderr.count("\n"), 1)

    def test_an_ordinary_command_passes(self):
        r = self.run_hook({"tool_name": "Bash", "cwd": str(REPO),
                           "tool_input": {"command": "cargo test"}})
        self.assertEqual((r.returncode, r.stderr), (0, ""))

    def test_other_tools_and_garbage_pass(self):
        self.assertEqual(self.run_hook({"tool_name": "Read", "tool_input": {}}).returncode, 0)
        r = subprocess.run([sys.executable, str(SCRIPT)], input="not json", capture_output=True, text=True)
        self.assertEqual(r.returncode, 0)


class Settings(unittest.TestCase):
    def setUp(self):
        self.s = json.loads(SETTINGS.read_text())  # parses: the CI check

    def test_denies_gh_pr_merge_and_has_no_force_push_globs(self):
        perms = self.s["permissions"]
        self.assertIn("Bash(gh pr merge*)", perms["deny"])
        everything = perms["allow"] + perms["deny"]
        self.assertFalse([r for r in everything if "--force" in r or " -f" in r],
                         "force pushes are the hook's job (a deny glob also blocks --force-with-lease)")
        self.assertFalse([r for r in perms["allow"] if "pr-merge" in r or "gh pr merge" in r],
                         "merging is the pr-manager's local rule only")

    def hook_command(self):
        return self.s["hooks"]["PreToolUse"][0]["hooks"][0]["command"]

    def run_configured(self, project_dir, command):
        payload = json.dumps({"tool_name": "Bash", "cwd": str(REPO), "tool_input": {"command": command}})
        return subprocess.run(["sh", "-c", self.hook_command()], input=payload, capture_output=True, text=True,
                              env={**os.environ, "CLAUDE_PROJECT_DIR": str(project_dir)})

    def test_the_configured_command_blocks_when_the_hook_is_present(self):
        r = self.run_configured(REPO, "git reset --hard")
        self.assertEqual(r.returncode, 2, r.stderr)
        self.assertEqual(self.run_configured(REPO, "cargo test").returncode, 0)

    def test_a_missing_hook_file_does_not_block_every_command(self):
        # A branch or worktree without the hook (or a renamed file) must not
        # turn into "every Bash call refused": python3 on a missing file exits 2.
        with tempfile.TemporaryDirectory() as empty:
            r = self.run_configured(empty, "cargo test")
            self.assertEqual(r.returncode, 0, r.stderr)

    def test_registers_the_guard_hook(self):
        hooks = self.s["hooks"]["PreToolUse"]
        bash = [h for h in hooks if h.get("matcher") == "Bash"]
        self.assertTrue(bash)
        cmds = [x["command"] for h in bash for x in h["hooks"]]
        self.assertTrue(any("guard-bash.py" in c for c in cmds), cmds)


if __name__ == "__main__":
    unittest.main()
