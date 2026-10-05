"""Tests for scripts/pr-merge against fake `gh` and `br` (no network).

Run from the repo root: python3 -m unittest discover -s scripts -p 'test_*.py'
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "pr-merge"
SHA = "a" * 40
MERGE_SHA = "b" * 40
URL = "https://github.com/o/r/pull/7"

# A fake `gh`: answers from $FAKE_DIR/{pr,threads}.json, records merges.
FAKE_GH = r'''#!/usr/bin/env python3
import json, os, sys
d = os.environ["FAKE_DIR"]
a = sys.argv[1:]
def load(n): return json.load(open(os.path.join(d, n)))
if a[:2] == ["repo", "view"]:
    print("o/r"); sys.exit(0)
if a[:2] == ["api", "graphql"]:
    if os.path.exists(os.path.join(d, "api_fail")): sys.exit(1)
    print(json.dumps(load("threads.json"))); sys.exit(0)
if a[:2] == ["pr", "merge"]:
    open(os.path.join(d, "merge_args"), "w").write(" ".join(a))
    if os.path.exists(os.path.join(d, "merge_fail")): sys.exit(1)
    open(os.path.join(d, "merged"), "w").write("1"); sys.exit(0)
if a[:2] == ["pr", "view"]:
    if os.path.exists(os.path.join(d, "view_fail")): sys.exit(1)
    pr = load("pr.json")
    merged = os.path.exists(os.path.join(d, "merged"))
    if "mergeCommit" in a: print(MERGE := "%s" if merged else ""); sys.exit(0)
    if "-q" in a and ".state" in a: print("MERGED" if merged else pr["state"]); sys.exit(0)
    print(json.dumps(pr)); sys.exit(0)
sys.exit(3)
''' % MERGE_SHA

FAKE_BR = r'''#!/usr/bin/env python3
import json, os, sys
if sys.argv[1:2] == ["show"]:
    print(open(os.path.join(os.environ["FAKE_DIR"], "issue.json")).read()); sys.exit(0)
sys.exit(3)
'''

WORKFLOW = """version: 1
remote: origin
trunk: master                  # default PR base
gates:
  spec_approval: true
  merge: {merge}                  # human | auto
max_bounces: 2
"""


def check(name, conclusion="SUCCESS", status="COMPLETED"):
    return {"name": name, "status": status, "conclusion": conclusion}


def green_pr(**over):
    pr = {
        "number": 7, "state": "OPEN", "isDraft": False, "baseRefName": "master",
        "headRefOid": SHA, "mergeable": "MERGEABLE", "reviewDecision": None,
        "title": "feat(sim-abc.1): do the thing", "url": URL,
        "statusCheckRollup": [check("native"), check("test"), check("web"),
                              check("deploy-pages", "SKIPPED")],
    }
    pr.update(over)
    return pr


class PrMerge(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="pr-merge-test-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        bin_ = self.dir / "bin"
        bin_.mkdir()
        for name, body in (("gh", FAKE_GH), ("br", FAKE_BR)):
            (bin_ / name).write_text(body)
            (bin_ / name).chmod(0o755)
        self.env = {**os.environ, "FAKE_DIR": str(self.dir),
                    "PATH": f"{bin_}{os.pathsep}{os.environ['PATH']}",
                    "PR_MERGE_WORKFLOW": str(self.dir / "workflow.yaml")}
        self.set(pr=green_pr(), merge="auto",
                 threads={"data": {"repository": {"pullRequest": {
                     "reviewThreads": {"totalCount": 0, "nodes": []},
                     "latestReviews": {"nodes": []}}}}},
                 issue=[{"id": "sim-abc.1", "status": "open", "labels": ["stage:pr"],
                         "external_ref": URL}])

    def set(self, pr=None, merge=None, threads=None, issue=None):
        if pr is not None:
            (self.dir / "pr.json").write_text(json.dumps(pr))
        if merge is not None:
            (self.dir / "workflow.yaml").write_text(WORKFLOW.format(merge=merge))
        if threads is not None:
            (self.dir / "threads.json").write_text(json.dumps(threads))
        if issue is not None:
            (self.dir / "issue.json").write_text(json.dumps(issue))

    def run_it(self, pr="7"):
        return subprocess.run([str(SCRIPT), pr], env=self.env, capture_output=True, text=True)

    def assert_refused(self, reason):
        r = self.run_it()
        self.assertEqual(r.returncode, 1, r.stderr)
        self.assertIn(f"pr-merge: refusing #7 — {reason}", r.stderr)
        self.assertFalse((self.dir / "merge_args").exists(), "must not try to merge")

    def test_merges_a_green_pr_and_prints_the_merge_sha(self):
        r = self.run_it()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.strip(), MERGE_SHA)
        args = (self.dir / "merge_args").read_text()
        self.assertIn("--squash", args)
        self.assertIn("--delete-branch", args)
        self.assertIn(f"--match-head-commit {SHA}", args)

    def test_refuses_a_draft(self):
        self.set(pr=green_pr(isDraft=True))
        self.assert_refused("PR is a draft")

    def test_refuses_a_closed_pr(self):
        self.set(pr=green_pr(state="CLOSED"))
        self.assert_refused("PR is CLOSED")

    def test_refuses_a_failing_check(self):
        self.set(pr=green_pr(statusCheckRollup=[check("native"), check("test", "FAILURE"), check("web")]))
        self.assert_refused("checks not green: test")

    def test_refuses_a_pending_check(self):
        self.set(pr=green_pr(statusCheckRollup=[check("native"), check("test"), check("web", None, "IN_PROGRESS")]))
        self.assert_refused("checks still running: web")

    def test_refuses_a_missing_required_check(self):
        self.set(pr=green_pr(statusCheckRollup=[check("native"), check("web")]))
        self.assert_refused("required check 'test' has not passed")

    def test_refuses_conflicts(self):
        self.set(pr=green_pr(mergeable="CONFLICTING"))
        self.assert_refused("GitHub says mergeable=CONFLICTING")

    def threads(self, *threads):
        """Review threads as (resolved, first comment body)."""
        nodes = [{"isResolved": r, "comments": {"nodes": [{"body": b}]}} for r, b in threads]
        self.set(threads={"data": {"repository": {"pullRequest": {
            "reviewThreads": {"totalCount": len(nodes), "nodes": nodes},
            "latestReviews": {"nodes": []}}}}})

    def test_refuses_an_unresolved_blocking_thread(self):
        self.threads((True, "blocking: fixed"), (False, "  Blocking: the merge conflicts"))
        self.assert_refused("1 unresolved blocking review thread(s)")

    def test_an_unresolved_nit_thread_does_not_block(self):
        self.threads((False, "nit: rename this"), (False, "a plain question"))
        r = self.run_it()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.strip(), MERGE_SHA)

    def test_a_resolved_blocking_thread_does_not_block(self):
        self.threads((True, "blocking: fixed in abc123"))
        self.assertEqual(self.run_it().returncode, 0)

    def test_refuses_when_threads_cannot_all_be_seen(self):
        self.set(threads={"data": {"repository": {"pullRequest": {
            "reviewThreads": {"totalCount": 150, "nodes": [{"isResolved": True, "comments": {"nodes": []}}]},
            "latestReviews": {"nodes": []}}}}})
        self.assert_refused("too many threads to check")

    def test_refuses_changes_requested(self):
        self.set(pr=green_pr(reviewDecision="CHANGES_REQUESTED"))
        self.assert_refused("a review requests changes")
        self.set(pr=green_pr(), threads={"data": {"repository": {"pullRequest": {
            "reviewThreads": {"totalCount": 0, "nodes": []},
            "latestReviews": {"nodes": [{"state": "CHANGES_REQUESTED"}]}}}}})
        self.assert_refused("1 review(s) request changes")

    def test_refuses_a_non_trunk_base(self):
        self.set(pr=green_pr(baseRefName="exp/foo"))
        self.assert_refused("base 'exp/foo' is not trunk 'master'")

    def test_refuses_when_the_merge_gate_is_human(self):
        self.set(merge="human")
        self.assert_refused("gates.merge is 'human', not auto")

    def test_refuses_an_issue_not_at_stage_pr_or_needing_a_human(self):
        self.set(issue=[{"id": "sim-abc.1", "status": "open", "labels": ["stage:test"], "external_ref": URL}])
        self.assert_refused("sim-abc.1: issue is not at stage:pr (stage:test)")
        self.set(issue=[{"id": "sim-abc.1", "status": "open", "labels": ["stage:pr", "needs-human"],
                         "external_ref": URL}])
        self.assert_refused("sim-abc.1: issue is needs-human")
        self.set(issue=[{"id": "sim-abc.1", "status": "open", "labels": ["stage:pr"],
                         "external_ref": "https://github.com/o/r/pull/8"}])
        self.assert_refused("sim-abc.1: issue external_ref is")

    def test_refuses_a_title_without_an_issue(self):
        self.set(pr=green_pr(title="Update README"))
        self.assert_refused("title 'Update README' doesn't name a beads issue")

    def test_fails_closed_on_api_errors(self):
        (self.dir / "view_fail").write_text("1")
        self.assert_refused("can't read the PR from GitHub")
        (self.dir / "view_fail").unlink()
        (self.dir / "api_fail").write_text("1")
        self.assert_refused("can't read review threads")

    def test_a_failed_merge_is_an_error_unless_the_pr_really_merged(self):
        (self.dir / "merge_fail").write_text("1")
        r = self.run_it()
        self.assertEqual(r.returncode, 1)
        self.assertIn("gh pr merge failed", r.stderr)

    def test_rejects_a_non_numeric_argument(self):
        r = self.run_it("7; rm -rf /")
        self.assertEqual(r.returncode, 2)
        self.assertFalse((self.dir / "merge_args").exists())


if __name__ == "__main__":
    unittest.main()
