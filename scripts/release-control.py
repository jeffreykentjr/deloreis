#!/usr/bin/env python3
"""Check that a manually requested archive belongs to its draft release."""

import json
import os
import re
import subprocess
import sys
import tomllib
import urllib.request


def validate(tag, commit, version, head, tagged, ancestor, release, target):
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("commit must be a full SHA")
    stable = f"v{version}"
    rc = re.fullmatch(re.escape(stable) + r"-rc\.[1-9][0-9]*", tag) is not None
    if target == "x86_64-unknown-linux-gnu":
        if tag != stable and not rc:
            raise ValueError("tag must match the workspace version or its RC")
    elif target == "aarch64-apple-darwin":
        if not rc:
            raise ValueError("macOS builds are limited to release candidates")
    else:
        raise ValueError("unsupported release target")
    if head != commit or tagged != commit or not ancestor:
        raise ValueError("checkout, tag and main ancestry must match the commit")
    if release.get("tagName") != tag or release.get("isDraft") is not True:
        raise ValueError("archive builds require the matching draft release")
    if release.get("isPrerelease") is not rc:
        raise ValueError("the draft's prerelease flag must match the tag")


def validate_published_release(tag, release):
    if (
        release.get("tagName") != tag
        or release.get("isDraft") is not False
        or release.get("isPrerelease") is not False
        or not release.get("publishedAt")
    ):
        raise ValueError("crate publication requires the matching stable published release")


def validate_macos_environment(environment, owner_id):
    if environment.get("name") != "macos-rc" or environment.get("can_admins_bypass") is not False:
        raise ValueError("macos-rc must prevent administrator bypass")
    for rule in environment.get("protection_rules", []):
        if rule.get("type") == "required_reviewers":
            reviewers = rule.get("reviewers", [])
            if (len(reviewers) == 1
                and reviewers[0].get("type") == "User"
                and reviewers[0].get("reviewer", {}).get("id") == owner_id):
                return
    raise ValueError("macos-rc must require only the owner's review")


def main():
    tag = os.environ["RELEASE_TAG"]
    commit = os.environ["RELEASE_COMMIT"]
    target = os.environ["RELEASE_TARGET"]
    version = tomllib.load(open("Cargo.toml", "rb"))["workspace"]["package"]["version"]

    def git(*args):
        return subprocess.check_output(["git", *args], text=True).strip()

    # Reject malformed input before putting it in a Git ref or API request.
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+(?:-rc\.[1-9][0-9]*)?", tag):
        raise ValueError("invalid release tag")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("commit must be a full SHA")
    head = git("rev-parse", "HEAD")
    tagged = git("rev-parse", f"refs/tags/{tag}^{{commit}}")
    ancestor = subprocess.run(
        ["git", "merge-base", "--is-ancestor", commit, "origin/main"], check=False
    ).returncode == 0
    release = json.loads(
        subprocess.check_output(
            ["gh", "release", "view", tag, "--json", "tagName,isDraft,isPrerelease"],
            text=True,
        )
    )
    validate(tag, commit, version, head, tagged, ancestor, release, target)
    if target == "aarch64-apple-darwin":
        # The repository is public; this read needs no elevated token. Fail
        # closed before allocating a macOS runner if the gate was removed.
        repository = os.environ["GITHUB_REPOSITORY"]
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
            raise ValueError("invalid repository name")
        request = urllib.request.Request(
            f"https://api.github.com/repos/{repository}/environments/macos-rc",
            headers={"Accept": "application/vnd.github+json", "User-Agent": "deloreis-release"},
        )
        with urllib.request.urlopen(request, timeout=10) as response:
            environment = json.load(response)
        validate_macos_environment(environment, 2244907)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
