#!/usr/bin/env bash
# no-ai-authorship.sh — fail if any commit in range carries an AI-authorship
# marker in its author, committer, or message.
#
# Usage:  scripts/no-ai-authorship.sh <event_name> <base_sha> <head_sha> <before_sha> <sha>
#   pull_request : scans base_sha..head_sha
#   push         : scans before_sha..sha, falling back to sha~20..sha when
#                  before_sha is the zero SHA (a new branch)

set -euo pipefail

EVENT_NAME="${1:-}"
BASE_SHA="${2:-}"
HEAD_SHA="${3:-}"
BEFORE_SHA="${4:-}"
SHA="${5:-HEAD}"

ZERO="0000000000000000000000000000000000000000"

commits() {
  if [[ "$EVENT_NAME" == "pull_request" ]]; then
    git rev-list "${BASE_SHA}..${HEAD_SHA}"
  elif [[ -z "$BEFORE_SHA" || "$BEFORE_SHA" == "$ZERO" ]]; then
    if git rev-parse --verify --quiet "${SHA}~20" >/dev/null; then
      git rev-list "${SHA}~20..${SHA}"
    else
      git rev-list "$SHA"
    fi
  else
    git rev-list "${BEFORE_SHA}..${SHA}"
  fi
}

PATTERN='(claude|anthropic|kiro|openai|codex|chatgpt|gpt-[0-9]|copilot|gemini|cursor|windsurf|devin|aider|[^a-z]ai[^a-z]|[^a-z]llm[^a-z])'

FAILED=0
while IFS= read -r commit; do
  [[ -n "$commit" ]] || continue
  identity="$(git log -1 --format='%an %ae %cn %ce' "$commit")"
  body="$(git log -1 --format='%B' "$commit")"
  if printf '%s' "$identity" | grep -qiE "$PATTERN"; then
    echo "no-ai-authorship: FAIL $commit — author/committer field matches AI marker" >&2
    FAILED=1
  fi
  if printf '%s' "$body" | grep -qiE "$PATTERN"; then
    echo "no-ai-authorship: FAIL $commit — commit message matches AI marker" >&2
    FAILED=1
  fi
done < <(commits)

if ((FAILED)); then
  echo "no-ai-authorship: remove AI-tool authorship from the offending commits." >&2
  exit 1
fi

echo "no-ai-authorship: clean — no AI-authorship markers in range."
