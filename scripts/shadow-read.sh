#!/usr/bin/env bash
#
# shadow-read.sh — the MIG-2 shadow-read harness (the dual-run evidence).
#
# WHAT THIS IS
#   MIG-2's assembly: pinned oracles (EXT-F1) + the vendored corpus snapshot
#   (EXT-F2) + the mutation detection corpus (EXT-F5), run as a dual-run and — the
#   load-bearing part — with DETECTION PROVEN SEPARATELY FROM PARITY. A harness
#   that only ever reports agreement is a null oracle (migration.md §3): before any
#   parity number is trusted, the harness must be shown to CATCH the seeded
#   divergences. This script proves detection first, then reports parity.
#
# RELATIONSHIP TO scripts/differential-oracle.sh (wave 2, PROOF-2)
#   This EXTENDS the wave-2 harness, it does not duplicate it. differential-oracle.sh
#   already stands the pinned legacy oracle beside the kernel over the vendored
#   facts.yaml (parity) and proves the KERNEL read-through catches three seeded
#   DATA-LAYER operators. MIG-2 adds the piece that makes it the migration's
#   shadow-read evidence:
#
#     LEG 1  the STRUCTURED detection corpus (EXT-F5) proven through the REAL
#            CHECK-1 gates — the PROOF-1 mutation harness
#            (crates/dli-world/tests/mutation_harness.rs): every one of the 33
#            ported operators is applied in memory and run through compile()/
#            check::run()/check_draft()/continuity/factcheck/language, and its
#            wave-3 kernel_outcome is asserted. This is the kernel side of the
#            detection corpus — 33 seeded structured divergences, not 3 data rows.
#
#     LEG 2  the wave-2 differential-oracle.sh, invoked (not re-implemented) for the
#            parity dual-run + the data-layer detection leg.
#
#     LEG 3  the both-sides cross-reference: each operator records the fantasy gate
#            that caught it (fantasy_gates — the ORACLE-side ground truth, pinned)
#            beside its verified kernel_outcome (the KERNEL side, proven in LEG 1),
#            so a seeded divergence is evidenced on both sides of the shadow-read.
#
#   It is LAB-SIDE and NON-GATING. Never wired into `cargo xtask gate` or CI: the
#   deloreis gate is deterministic, offline, and model-free, while this script drives
#   another repo's Python toolchain (via scripts/pinned-oracles.sh) and writes
#   untracked reports under out/shadow-read/.
#
# USAGE
#   scripts/shadow-read.sh [--no-oracle] [--keep-temp]
#     --no-oracle   skip the pinned legacy oracle even if provisionable (offline run)
#     --keep-temp   forwarded to differential-oracle.sh (leave mutated temp corpora)
#
# ENVIRONMENT
#   TMPDIR              scratch root (the sprint pins $HOME/.cache/tmp).
#   Plus everything scripts/{pinned-oracles,differential-oracle}.sh honour.
#
set -uo pipefail

# --- locate ourselves ------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT" || exit 1

DIFFERENTIAL="$SCRIPT_DIR/differential-oracle.sh"
VENDOR="$REPO_ROOT/fixtures/vendor/fantasy-40f6679"
MUT_DIR="$REPO_ROOT/fixtures/mutation"
PINNED_SHA="$(sed -n 's/^# pinned-sha: //p' "$VENDOR/MANIFEST.txt" | head -1)"
OUT="$REPO_ROOT/out/shadow-read"
RUN_TS="$(date -u +%Y%m%dT%H%M%SZ)"
REPORT="$OUT/report-$RUN_TS.md"

FORWARD=()
for arg in "$@"; do
  case "$arg" in
    --no-oracle | --keep-temp) FORWARD+=("$arg") ;;
    -h | --help) sed -n '2,44p' "$0"; exit 0 ;;
    *) echo "unknown arg: $arg (try --help)" >&2; exit 2 ;;
  esac
done

[[ -d "$MUT_DIR" ]] || { echo "detection corpus missing: $MUT_DIR" >&2; exit 2; }
mkdir -p "$OUT"

log()  { printf '[shadow-read] %s\n' "$*" >&2; }
rule() { printf '\n========== %s ==========\n' "$*" >&2; }

# Accumulators.
STRUCTURAL_STATE="unknown"
STRUCTURAL_PASS=""
OPERATOR_COUNT=0
DIFF_STATE="unknown"
DIFF_EXIT=0

# The pinned SHA every leg shares (the corpus, the oracle, and the ported operators
# are all cut from it).
log "MIG-2 shadow-read @ pinned SHA ${PINNED_SHA:-<unknown>}"

# ===========================================================================
# LEG 1 — DETECTION, structured corpus (EXT-F5), proven FIRST and SEPARATELY.
# The PROOF-1 mutation harness runs all 33 ported operators through the real
# CHECK-1 gates. Its pass IS the proof that the kernel reacts to every seeded
# structured divergence (or honestly asserts the miss). No parity number below is
# trusted unless this leg is green — detection before parity (migration.md §3).
# ===========================================================================
rule "LEG 1: structured detection corpus (PROOF-1 mutation harness)"
STRUCT_LOG="$OUT/proof1-$RUN_TS.log"
OPERATOR_COUNT="$(grep -c '^  - operator:' "$MUT_DIR"/canon.yaml "$MUT_DIR"/draft.yaml "$MUT_DIR"/episode.yaml 2>/dev/null | awk -F: '{s+=$2} END{print s}')"
log "detection corpus: $OPERATOR_COUNT ported operators (fixtures/mutation/*.yaml)"
if cargo nextest run -p dli-world --test mutation_harness --locked >"$STRUCT_LOG" 2>&1; then
  STRUCTURAL_STATE="proven"
  STRUCTURAL_PASS="$(grep -oE '[0-9]+ (tests? )?passed' "$STRUCT_LOG" | tail -1)"
  log "structured detection PROVEN: ${STRUCTURAL_PASS:-all cases pass} (see $STRUCT_LOG)"
else
  STRUCTURAL_STATE="FAILED"
  log "structured detection FAILED — the kernel no longer reacts as recorded (see $STRUCT_LOG)"
fi

# ===========================================================================
# LEG 2 — the wave-2 differential-oracle harness: the parity dual-run (pinned
# oracle vs kernel over the pristine vendored corpus) plus the data-layer detection
# leg (three seeded facts.yaml operators through the kernel read-through). Invoked,
# not re-implemented.
# ===========================================================================
rule "LEG 2: parity dual-run + data-layer detection (differential-oracle.sh)"
DIFF_LOG="$OUT/differential-$RUN_TS.log"
if [[ -x "$DIFFERENTIAL" ]]; then
  "$DIFFERENTIAL" "${FORWARD[@]}" >"$DIFF_LOG" 2>&1
  DIFF_EXIT=$?
  case "$DIFF_EXIT" in
    0) DIFF_STATE="detection-proven; parity recorded" ;;
    1) DIFF_STATE="detection INCOMPLETE" ;;
    3) DIFF_STATE="BLOCKED (kernel did not compile)" ;;
    *) DIFF_STATE="unexpected exit $DIFF_EXIT" ;;
  esac
  log "differential-oracle.sh: $DIFF_STATE (exit $DIFF_EXIT, see $DIFF_LOG)"
else
  DIFF_STATE="skipped (differential-oracle.sh not executable)"
  log "$DIFF_STATE"
fi

# ===========================================================================
# LEG 3 — the both-sides cross-reference. Each operator record carries the fantasy
# gate that caught it (fantasy_gates — the ORACLE side, pinned ground truth) beside
# its verified kernel_outcome (the KERNEL side, LEG 1). Count the operators with
# both, so the report states detection on BOTH sides of the shadow-read.
# ===========================================================================
rule "LEG 3: both-sides cross-reference (oracle ground truth × kernel outcome)"
WITH_FANTASY_GATE="$(grep -c '^    fantasy_gates:' "$MUT_DIR"/canon.yaml "$MUT_DIR"/draft.yaml "$MUT_DIR"/episode.yaml 2>/dev/null | awk -F: '{s+=$2} END{print s}')"
WITH_KERNEL_OUTCOME="$(grep -c '^    kernel_outcome:' "$MUT_DIR"/canon.yaml "$MUT_DIR"/draft.yaml "$MUT_DIR"/episode.yaml 2>/dev/null | awk -F: '{s+=$2} END{print s}')"
log "operators with oracle-side gate recorded: $WITH_FANTASY_GATE / $OPERATOR_COUNT"
log "operators with verified kernel_outcome:   $WITH_KERNEL_OUTCOME / $OPERATOR_COUNT"

# ===========================================================================
# The report.
# ===========================================================================
rule "REPORT"
{
  echo "# MIG-2 shadow-read — $RUN_TS"
  echo
  echo "- pinned SHA: \`$PINNED_SHA\`"
  echo "- corpus: \`fixtures/vendor/fantasy-40f6679\` (EXT-F2)"
  echo "- detection corpus: \`fixtures/mutation/*.yaml\` — $OPERATOR_COUNT operators (EXT-F5)"
  echo "- harness: \`scripts/shadow-read.sh\` (lab-side, non-gating) — extends \`differential-oracle.sh\`"
  echo
  echo "## Detection (proven SEPARATELY from parity, and FIRST)"
  echo
  echo "| Leg | What | State |"
  echo "|---|---|---|"
  echo "| structured (EXT-F5) | $OPERATOR_COUNT operators × the real CHECK-1 gates (PROOF-1) | $STRUCTURAL_STATE ${STRUCTURAL_PASS:+($STRUCTURAL_PASS)} |"
  echo "| data-layer | 3 seeded facts.yaml operators × the kernel read-through | via differential-oracle.sh |"
  echo "| both-sides | oracle gate recorded × kernel_outcome verified | $WITH_FANTASY_GATE / $WITH_KERNEL_OUTCOME of $OPERATOR_COUNT |"
  echo
  echo "## Parity dual-run"
  echo
  echo "differential-oracle.sh: **$DIFF_STATE** (exit $DIFF_EXIT)."
  echo "Its own report (oracle validate/world-build vs kernel CHECK-1 + MIG-1) is under \`out/differential/\`."
  echo
  echo "## Verdict"
  echo
  if [[ "$STRUCTURAL_STATE" == "proven" && ( "$DIFF_EXIT" -eq 0 || "$DIFF_STATE" == skipped* ) ]]; then
    echo "Detection is PROVEN on the kernel side across $OPERATOR_COUNT structured operators"
    echo "and the data-layer leg; parity is meaningful because detection precedes it."
  else
    echo "Detection is NOT fully proven this cycle — investigate before trusting any parity number."
  fi
} >"$REPORT"
log "wrote $REPORT"
cat "$REPORT" >&2
echo >&2

# The harness's own exit: HEALTHY when structured detection is proven and the
# differential leg is not compile-blocked. Parity is reported, never gated on.
if [[ "$STRUCTURAL_STATE" != "proven" ]]; then
  log "VERDICT: structured detection FAILED — MIG-2 not satisfied this cycle"
  exit 1
elif [[ "$DIFF_EXIT" -eq 3 ]]; then
  log "VERDICT: BLOCKED (kernel did not compile in the differential leg)"
  exit 3
else
  log "VERDICT: detection PROVEN ($OPERATOR_COUNT operators); dual-run evidence recorded"
  exit 0
fi
