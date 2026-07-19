#!/usr/bin/env bash
# ci-wait.sh — wait for the GitHub Actions runs OF A SPECIFIC COMMIT and report
# their conclusion.
#
# `gh run list` right after a push races the webhook and can return the PREVIOUS
# commit's run. This resolves runs BY headSha, polls until they register, then
# until they all complete. Success iff every run for the SHA concluded success.
#
# Usage:  scripts/ci-wait.sh [sha-or-ref]   # default: HEAD
# Exit:   0 only if all runs concluded "success"; 1 otherwise.

set -euo pipefail

REF="${1:-HEAD}"
if SHA="$(git rev-parse --verify --quiet "${REF}^{commit}")"; then
  :
elif [[ "$REF" =~ ^[0-9a-f]{7,40}$ ]]; then
  SHA="$REF"
else
  echo "ci-wait: cannot resolve '$REF' to a commit" >&2
  exit 1
fi

APPEAR_TIMEOUT="${DELOREIS_CI_WAIT_APPEAR_TIMEOUT:-900}"
COMPLETE_TIMEOUT="${DELOREIS_CI_WAIT_TIMEOUT:-1500}"
POLL_INTERVAL="${DELOREIS_CI_WAIT_POLL_INTERVAL:-15}"

echo "ci-wait: waiting for CI runs of ${SHA}"

count_runs() {
  gh run list --commit "$SHA" --json databaseId --jq 'length'
}

# Wait for at least one run to register.
deadline=$((SECONDS + APPEAR_TIMEOUT))
while :; do
  n="$(count_runs || echo 0)"
  [[ "$n" =~ ^[0-9]+$ ]] || n=0
  if ((n > 0)); then
    break
  fi
  if ((SECONDS >= deadline)); then
    echo "ci-wait: TIMEOUT — no run registered for $SHA after ${APPEAR_TIMEOUT}s." >&2
    echo "ci-wait: was this SHA actually pushed as the branch tip?" >&2
    exit 1
  fi
  sleep "$POLL_INTERVAL"
done

# Wait for every run of this SHA to complete.
deadline=$((SECONDS + COMPLETE_TIMEOUT))
while :; do
  incomplete="$(gh run list --commit "$SHA" --json status --jq '[.[] | select(.status != "completed")] | length')"
  [[ "$incomplete" =~ ^[0-9]+$ ]] || incomplete=1
  if ((incomplete == 0)); then
    break
  fi
  if ((SECONDS >= deadline)); then
    echo "ci-wait: TIMEOUT — runs for $SHA still incomplete after ${COMPLETE_TIMEOUT}s." >&2
    exit 1
  fi
  echo "ci-wait: $incomplete run(s) still in progress ..."
  sleep "$POLL_INTERVAL"
done

failed="$(gh run list --commit "$SHA" --json conclusion --jq '[.[] | select(.conclusion != "success")] | length')"
if [[ "$failed" == "0" ]]; then
  echo "ci-wait: GREEN — all runs for $SHA passed CI"
  exit 0
fi

echo "ci-wait: RED — $failed run(s) for $SHA did not pass CI." >&2
echo "ci-wait: red main is a drop-everything event: fix forward or revert NOW." >&2
gh run list --commit "$SHA" --json name,conclusion,url \
  --jq '.[] | select(.conclusion != "success") | "  \(.name): \(.conclusion) \(.url)"' >&2 || true
exit 1
