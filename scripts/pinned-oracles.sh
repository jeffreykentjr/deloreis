#!/usr/bin/env bash
#
# pinned-oracles.sh — run the fantasy repo's oracles at a PINNED commit.
#
# WHAT THIS IS
#   A lab-side, NON-GATING convenience wrapper (EXT-F1). It provisions a
#   pristine checkout of the consuming fantasy repo at one pinned SHA and
#   exposes its `uv run fantasy ...` oracles as subcommands, so the deloreis
#   kernel can be dual-run / parity-checked against the legacy engine on the
#   SAME pinned inputs the vendored corpus (fixtures/vendor/) was cut from.
#
#   This is NOT part of `cargo xtask gate`. The deloreis gate is deterministic,
#   offline, and model-free (CLAUDE.md §3); this script clones another repo and
#   runs its Python toolchain. It must never be wired into a gating lane.
#
# HOW IT PINS
#   The fantasy checkout is materialized with `git archive <SHA> | tar -x` from
#   the source repo's OBJECT STORE — never from its working tree. The fantasy
#   tree is a shared live checkout that other agents dirty; archiving from the
#   committed object at the pinned SHA is what makes this reproducible. The
#   pinned SHA is the same one recorded in
#   fixtures/vendor/fantasy-40f6679/MANIFEST.txt and docs/design-notes/vendored-corpus.md.
#
# SUBCOMMANDS
#   provision            Materialize + `uv sync` the pinned checkout (idempotent).
#   info                 Print the pinned SHA, source repo, and checkout path.
#   validate             `fantasy validate`      — canon corpus validation.
#   world-build [args]   `fantasy world build`   — compile the typed world model.
#   factcheck <draft>    `fantasy factcheck …`   — claim-check a draft vs the world.
#   Any trailing args after the subcommand are forwarded to the fantasy CLI.
#
# ENVIRONMENT
#   FANTASY_PINNED_SHA    Override the pinned SHA (default: the recorded pin).
#   FANTASY_SOURCE_REPO   Path to the fantasy git repo providing the objects
#                         (default: /home/jeff/Documents/scratch/fantasy).
#   DELOREIS_ORACLE_DIR   Cache root for provisioned checkouts
#                         (default: $HOME/.cache/deloreis-oracles).
#
# CAVEATS
#   * First provision needs `uv` on PATH. Dependency install is offline
#     (uv.lock), but uv may fetch the pinned CPython on a cold machine.
#   * `factcheck`/`world-build` need only the vendored canon, which the pinned
#     archive already contains; no AWS/OpenRouter credentials are required for
#     these three oracles (they are deterministic, model-free paths).
#   * The cache is keyed by short SHA. `rm -rf "$CHECKOUT"` (or delete the
#     .oracle-ready marker) to force a clean re-provision.
#
set -euo pipefail

PINNED_SHA="${FANTASY_PINNED_SHA:-40f66792229abf5d73d995a8167adc79152a64e7}"
SOURCE_REPO="${FANTASY_SOURCE_REPO:-/home/jeff/Documents/scratch/fantasy}"
ORACLE_DIR="${DELOREIS_ORACLE_DIR:-$HOME/.cache/deloreis-oracles}"
SHORT_SHA="${PINNED_SHA:0:7}"
CHECKOUT="$ORACLE_DIR/fantasy-$SHORT_SHA"
READY_MARKER="$CHECKOUT/.oracle-ready"

log() { printf '[pinned-oracles] %s\n' "$*" >&2; }
die() { printf '[pinned-oracles] error: %s\n' "$*" >&2; exit 1; }

provision() {
  if [[ -f "$READY_MARKER" ]]; then
    log "checkout ready: $CHECKOUT (marker present)"
    return 0
  fi
  command -v uv >/dev/null 2>&1 || die "uv not found on PATH — required to provision the oracle checkout"
  [[ -d "$SOURCE_REPO/.git" ]] || die "source repo has no .git: $SOURCE_REPO"
  git -C "$SOURCE_REPO" cat-file -e "${PINNED_SHA}^{commit}" 2>/dev/null \
    || die "pinned SHA $PINNED_SHA not present in $SOURCE_REPO objects (fetch it first)"

  log "materializing pinned tree $SHORT_SHA into $CHECKOUT (git archive; working tree ignored)"
  rm -rf "$CHECKOUT"
  mkdir -p "$CHECKOUT"
  git -C "$SOURCE_REPO" archive "$PINNED_SHA" | tar -x -C "$CHECKOUT"

  log "uv sync in pinned checkout (offline deps from uv.lock)"
  ( cd "$CHECKOUT" && uv sync )

  printf 'pinned-sha: %s\nsource-repo: %s\nprovisioned-by: scripts/pinned-oracles.sh\n' \
    "$PINNED_SHA" "$SOURCE_REPO" > "$READY_MARKER"
  log "provisioned: $CHECKOUT"
}

run_fantasy() {
  provision
  log "run: (cd $CHECKOUT && uv run fantasy $*)"
  ( cd "$CHECKOUT" && uv run fantasy "$@" )
}

cmd="${1:-help}"
[[ $# -gt 0 ]] && shift || true

case "$cmd" in
  provision)
    provision
    ;;
  info)
    printf 'pinned-sha:   %s\n' "$PINNED_SHA"
    printf 'source-repo:  %s\n' "$SOURCE_REPO"
    printf 'oracle-dir:   %s\n' "$ORACLE_DIR"
    printf 'checkout:     %s\n' "$CHECKOUT"
    printf 'ready:        %s\n' "$([[ -f "$READY_MARKER" ]] && echo yes || echo no)"
    ;;
  validate)
    run_fantasy validate "$@"
    ;;
  world-build)
    run_fantasy world build "$@"
    ;;
  factcheck)
    [[ $# -ge 1 ]] || die "factcheck needs a draft path: pinned-oracles.sh factcheck <draft>"
    run_fantasy factcheck "$@"
    ;;
  help | -h | --help)
    sed -n '2,40p' "$0"
    ;;
  *)
    die "unknown subcommand '$cmd' (try: provision | info | validate | world-build | factcheck | help)"
    ;;
esac
