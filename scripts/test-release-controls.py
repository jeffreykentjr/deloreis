#!/usr/bin/env python3
"""Exercise release tag, draft and target controls without a network."""

import importlib.util
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("release_control", Path(__file__).with_name("release-control.py"))
control = importlib.util.module_from_spec(spec)
spec.loader.exec_module(control)


class ReleaseControls(unittest.TestCase):
    def setUp(self):
        self.commit = "a" * 40
        self.rc = {"tagName": "v0.1.0-rc.1", "isDraft": True, "isPrerelease": True}
        self.stable = {"tagName": "v0.1.0", "isDraft": True, "isPrerelease": False}

    def check(self, tag, release, target, *, head=None, tagged=None, ancestor=True):
        control.validate(tag, self.commit, "0.1.0", head or self.commit,
                         tagged or self.commit, ancestor, release, target)

    def test_linux_accepts_the_matching_stable_or_rc_draft(self):
        for tag, release in [("v0.1.0", self.stable), ("v0.1.0-rc.1", self.rc)]:
            self.check(tag, release, "x86_64-unknown-linux-gnu")

    def test_mac_accepts_only_a_matching_rc_draft(self):
        self.check("v0.1.0-rc.1", self.rc, "aarch64-apple-darwin")
        with self.assertRaises(ValueError):
            self.check("v0.1.0", self.stable, "aarch64-apple-darwin")
        with self.assertRaises(ValueError):
            self.check("v0.1.0-rc.1", dict(self.rc, isPrerelease=False), "aarch64-apple-darwin")

    def test_rejects_published_wrong_commit_and_unsupported_architectures(self):
        for release in [dict(self.rc, isDraft=False), dict(self.rc, tagName="v0.1.0-rc.2")]:
            with self.assertRaises(ValueError):
                self.check("v0.1.0-rc.1", release, "x86_64-unknown-linux-gnu")
        for kwargs in [{"head": "b" * 40}, {"tagged": "b" * 40}, {"ancestor": False}]:
            with self.assertRaises(ValueError):
                self.check("v0.1.0-rc.1", self.rc, "x86_64-unknown-linux-gnu", **kwargs)
        for target in ["aarch64-unknown-linux-gnu", "x86_64-apple-darwin"]:
            with self.assertRaises(ValueError):
                self.check("v0.1.0-rc.1", self.rc, target)

    def test_macos_job_is_manual_and_environment_protected(self):
        root = Path(__file__).resolve().parent.parent
        workflow = (root / ".github/workflows/release-macos-rc.yml").read_text()
        self.assertIn("workflow_dispatch:", workflow)
        self.assertIn("environment: macos-rc", workflow)
        self.assertIn("RELEASE_TARGET: aarch64-apple-darwin", workflow)
        self.assertIn("if: github.ref == 'refs/heads/main'", workflow)
        self.assertIn("path: control", workflow)
        self.assertIn("python3 ../control/scripts/release-control.py", workflow)
        self.assertNotIn("schedule:", workflow)
        self.assertNotIn("push:", workflow)
        self.assertNotIn("pull_request:", workflow)

    def test_environment_requires_owner_and_disallows_bypass(self):
        protected = {
            "name": "macos-rc", "can_admins_bypass": False,
            "protection_rules": [{"type": "required_reviewers", "reviewers": [
                {"type": "User", "reviewer": {"id": 2244907}}
            ]}],
        }
        control.validate_macos_environment(protected, 2244907)
        for broken in [
            dict(protected, can_admins_bypass=True),
            dict(protected, protection_rules=[]),
            dict(protected, name="other"),
            dict(protected, can_admins_bypass=None),
            dict(protected, protection_rules=[{"type": "required_reviewers", "reviewers": [
                {"type": "User", "reviewer": {"id": 2244907}},
                {"type": "User", "reviewer": {"id": 123}},
            ]}]),
        ]:
            with self.assertRaises(ValueError):
                control.validate_macos_environment(broken, 2244907)

    def test_only_a_published_stable_release_can_publish_crates(self):
        release = {"tagName": "v0.1.0", "isDraft": False,
                   "isPrerelease": False, "publishedAt": "2026-11-03T12:00:00Z"}
        control.validate_published_release("v0.1.0", release)
        for broken in [dict(release, isPrerelease=True), dict(release, isDraft=True),
                       dict(release, publishedAt=None), dict(release, tagName="v0.1.1"),
                       dict(release, isPrerelease=None)]:
            with self.assertRaises(ValueError):
                control.validate_published_release("v0.1.0", broken)


if __name__ == "__main__":
    unittest.main()
