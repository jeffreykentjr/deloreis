#!/usr/bin/env bash
# safe-commit.sh — stage named files, gate the commit, push.
#
#   1. Hold a repository-identity delivery lock (multi-agent safety).
#   2. Assert a branch and a fast-forward base against origin.
#   3. ABORT if the staging area is not empty (nothing rides along).
#   4. Stage ONLY the files you name; show and commit them.
#   5. Run scripts/greengate.sh on the new commit; require a green receipt.
#   6. Push only if green.
#   7. Print the SHA for CI verification — do NOT block on ci-wait.sh
#      (that eats minutes; verify manually with `scripts/ci-wait.sh <sha>`).
#
# Usage:  scripts/safe-commit.sh "<commit message>" <file> [file...]
# Exit:   0 after the commit is pushed; remote CI is the caller's responsibility.

set -euo pipefail

if (($# < 2)); then
  echo "usage: scripts/safe-commit.sh \"<commit message>\" <file> [file...]" >&2
  exit 1
fi

MSG="$1"
shift

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

repository_lock_id() {
  local identity
  identity="$(git remote get-url --push origin 2>/dev/null || true)"
  if [[ -n "$identity" ]]; then
    identity="origin:$identity"
  else
    identity="git-common-dir:$(git rev-parse --path-format=absolute --git-common-dir)"
  fi
  printf '%s' "$identity" | sha256sum | awk '{print $1}'
}

acquire_delivery_lock() {
  local lock_root lock_timeout
  lock_root="${DELOREIS_DELIVERY_LOCK_DIR:-${XDG_RUNTIME_DIR:-/tmp}/deloreis-delivery-locks}"
  lock_timeout="${DELOREIS_DELIVERY_LOCK_TIMEOUT:-1800}"
  if [[ ! "$lock_timeout" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
    echo "safe-commit: invalid DELOREIS_DELIVERY_LOCK_TIMEOUT '$lock_timeout'" >&2
    exit 1
  fi
  umask 077
  mkdir -p "$lock_root"
  chmod 700 "$lock_root"
  REPOSITORY_LOCK_ID="$(repository_lock_id)"
  DELIVERY_LOCK_PATH="$lock_root/$REPOSITORY_LOCK_ID.lock"
  exec 9>>"$DELIVERY_LOCK_PATH"
  echo "safe-commit: waiting up to ${lock_timeout}s for repository delivery lock $REPOSITORY_LOCK_ID"
  if ! flock -w "$lock_timeout" 9; then
    echo "safe-commit: TIMEOUT waiting for repository delivery lock $REPOSITORY_LOCK_ID" >&2
    if [[ -s "$DELIVERY_LOCK_PATH" ]]; then
      sed 's/^/safe-commit: lock holder: /' "$DELIVERY_LOCK_PATH" >&2
    fi
    exit 1
  fi
  : >"$DELIVERY_LOCK_PATH"
  printf 'pid=%s\nstarted=%s\nworktree=%s\ncommand=safe-commit\n' \
    "$$" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$REPO_ROOT" >&9
  export DELOREIS_DELIVERY_LOCK_ID="$REPOSITORY_LOCK_ID"
  echo "safe-commit: acquired repository delivery lock $REPOSITORY_LOCK_ID"
}

acquire_delivery_lock

BRANCH="$(git symbolic-ref --quiet --short HEAD || true)"
if [[ -z "$BRANCH" ]]; then
  echo "safe-commit: detached HEAD — check out a branch first." >&2
  exit 1
fi

echo "safe-commit: fetching origin/$BRANCH and verifying the delivery base"
if ! git fetch --quiet origin "$BRANCH"; then
  echo "safe-commit: ABORT — cannot fetch origin/$BRANCH." >&2
  exit 1
fi
UPSTREAM_SHA="$(git rev-parse "refs/remotes/origin/$BRANCH")"
BASE_SHA="$(git rev-parse HEAD)"
if ! git merge-base --is-ancestor "$UPSTREAM_SHA" "$BASE_SHA"; then
  echo "safe-commit: ABORT — stale base: HEAD is not a fast-forward of origin/$BRANCH." >&2
  echo "safe-commit: refresh/rebase before committing; nothing was staged." >&2
  exit 1
fi

# --- staging area must be EMPTY (nothing rides along) -----------------------
if ! git diff --cached --quiet; then
  echo "safe-commit: ABORT — the staging area is not empty:" >&2
  git diff --cached --name-status >&2
  echo "safe-commit: unstage with 'git restore --staged .' then re-run." >&2
  exit 1
fi

# --- stage only the named files ---------------------------------------------
git add -- "$@"
if git diff --cached --quiet; then
  echo "safe-commit: ABORT — named files have no changes to commit." >&2
  exit 1
fi

echo "safe-commit: committing on '$BRANCH' with message: $MSG"
git diff --cached --stat
git commit -m "$MSG"
SHA="$(git rev-parse HEAD)"
SHORT_SHA="$(git rev-parse --short "$SHA")"

# --- full gate on the COMMITTED tree ----------------------------------------
echo
echo "safe-commit: running greengate on $SHORT_SHA (full gate, clean worktree)"
if ! "$SCRIPT_DIR/greengate.sh" "$SHA"; then
  echo "safe-commit: RED — $SHORT_SHA fails the gate. NOT pushing (commit is local)." >&2
  exit 1
fi

RECEIPT="$REPO_ROOT/out/greengate/$SHA/receipt.json"
if ! grep -Eq '"result"[[:space:]]*:[[:space:]]*"green"' "$RECEIPT" 2>/dev/null; then
  echo "safe-commit: ABORT — greengate receipt is not green: $RECEIPT" >&2
  exit 1
fi

# --- push (green only) ------------------------------------------------------
echo
echo "safe-commit: green — pushing $SHORT_SHA to origin/$BRANCH"
if ! git push origin "$BRANCH"; then
  echo "safe-commit: push failed (probably behind origin/$BRANCH)." >&2
  echo "safe-commit: pull --rebase, re-run greengate, push, then ci-wait." >&2
  exit 1
fi

# --- CI verification is manual (by SHA) -------------------------------------
echo
echo "safe-commit: pushed $SHORT_SHA to origin/$BRANCH"
echo "safe-commit: verify CI manually: scripts/ci-wait.sh $SHORT_SHA"
