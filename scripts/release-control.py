#!/usr/bin/env python3
"""Check that a manually requested archive belongs to its draft release."""

import json
import os
import re
import subprocess
import sys
import tomllib


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
    if release.get("tagName") != tag or not release.get("isDraft"):
        raise ValueError("archive builds require the matching draft release")
    if bool(release.get("isPrerelease")) != rc:
        raise ValueError("the draft's prerelease flag must match the tag")


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


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
