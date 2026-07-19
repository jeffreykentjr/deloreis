#!/usr/bin/env bash
# greengate.sh — run the FULL gate against a committed tree, immune to
# workspace pollution.
#
# Multiple agents share this checkout, so running the gate in the live tree
# mixes YOUR commit with THEIR uncommitted WIP. This checks out HEAD (or a given
# ref) into a throwaway `git worktree`, so ONLY committed state is gated. A
# shared CARGO_TARGET_DIR keeps rebuilds cheap; the live checkout is untouched.
#
# Usage:  scripts/greengate.sh [ref]        # default ref: HEAD
# Exit:   0 on all-green, 1 on any gate failure. The worktree is always removed.

set -euo pipefail

REF="${1:-HEAD}"

REPO_ROOT="$(git rev-parse --show-toplevel)"
SHA="$(git -C "$REPO_ROOT" rev-parse --verify "${REF}^{commit}")"
SHORT_SHA="$(git -C "$REPO_ROOT" rev-parse --short "$SHA")"

export CARGO_TARGET_DIR="${DELOREIS_GREENGATE_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/deloreis-greengate-target}"

TMP_PARENT="$(mktemp -d "${TMPDIR:-/tmp}/deloreis-greengate.XXXXXX")"
WORKTREE="$TMP_PARENT/wt"

cleanup() {
  git -C "$REPO_ROOT" worktree remove --force "$WORKTREE" >/dev/null 2>&1 || true
  rm -rf "$TMP_PARENT"
  git -C "$REPO_ROOT" worktree prune >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "greengate: testing committed tree $SHORT_SHA ($SHA)"
echo "greengate: worktree $WORKTREE  target $CARGO_TARGET_DIR"
git -C "$REPO_ROOT" worktree add --detach "$WORKTREE" "$SHA" >/dev/null

if (cd "$WORKTREE" && cargo run -p xtask --locked -- gate --ci --receipt "$REPO_ROOT/out/greengate/$SHA"); then
  echo
  echo "greengate: PASSED — commit $SHORT_SHA is green (full gate, clean tree)"
else
  echo
  echo "greengate: FAILED — the COMMITTED tree $SHORT_SHA is red — do not push." >&2
  echo "greengate: fix forward, commit, and re-run scripts/greengate.sh before pushing." >&2
  exit 1
fi
