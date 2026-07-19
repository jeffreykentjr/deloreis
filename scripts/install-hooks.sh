#!/usr/bin/env bash
# install-hooks.sh — point git at the tracked hooks in .githooks/.
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

git config core.hooksPath .githooks
echo "install-hooks: core.hooksPath = .githooks"
echo "install-hooks: commit-msg hook active (AI-authorship markers are rejected)."
