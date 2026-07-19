#!/usr/bin/env bash
#
# differential-oracle.sh — the standing differential-oracle harness (PROOF-2).
#
# WHAT THIS IS
#   The shadow-read harness that stands the legacy fantasy oracle beside the
#   deloreis kernel over the SAME pinned inputs, diffs their verdicts, and — the
#   load-bearing part — proves the kernel side CATCHES seeded divergence
#   separately from any parity claim (docs/integration/migration.md §3: a harness
#   that only ever reports agreement is a null oracle).
#
#   It is LAB-SIDE and NON-GATING. It is never wired into `cargo xtask gate` or
#   CI: the deloreis gate is deterministic, offline, and model-free, while this
#   script drives another repo's Python toolchain (via scripts/pinned-oracles.sh)
#   and writes untracked reports under out/differential/.
#
# THE FOUR PHASES
#   A. ORACLE   — the pinned legacy gates over the vendored corpus:
#                 `pinned-oracles.sh validate` + `world-build`. Skips cleanly if
#                 the oracle cannot be provisioned (no uv / SHA absent).
#   B. OUR SIDE — the kernel read-through over the same corpus:
#                 `cargo test -p dli-world --test mig1_replay` (MIG-1, facts.yaml
#                 status cross-check). The optional CHECK-1 parity leg is invoked
#                 if its test is present (probe: MIG1_CHECK_TEST).
#   C. DIVERGE  — both verdicts written to out/differential/, with the triage
#                 classes and the KNOWN scope gap (oracle = whole canon;
#                 mig1_replay = facts.yaml only) recorded, never papered over.
#   D. DETECT   — detection proven separately from parity: for three named
#                 operators (fixtures/mutation/canon.yaml), apply the data-layer
#                 analogue to a TEMP copy of facts.yaml and re-run OUR side,
#                 asserting the clean read-through turns into a finding.
#
# USAGE
#   scripts/differential-oracle.sh [--no-oracle] [--keep-temp]
#     --no-oracle   skip phase A even if the oracle is provisionable (offline run)
#     --keep-temp   leave the mutated temp corpora under out/differential/ for
#                   inspection instead of deleting them
#
# ENVIRONMENT
#   TMPDIR              scratch root (the sprint pins $HOME/.cache/tmp).
#   CHECK_TEST_BIN      the CHECK-1 parity test binary to probe (default "check").
#   CHECK_FILTER        the parity case filter within it (default "parity_").
#   Plus everything scripts/pinned-oracles.sh honours (FANTASY_PINNED_SHA, …).
#
set -uo pipefail

# --- locate ourselves ------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT" || exit 1

ORACLE="$SCRIPT_DIR/pinned-oracles.sh"
VENDOR="$REPO_ROOT/fixtures/vendor/fantasy-40f6679"
FACTS="$VENDOR/canon/data/facts.yaml"
MUT="$REPO_ROOT/fixtures/mutation/canon.yaml"
PINNED_SHA="$(sed -n 's/^# pinned-sha: //p' "$VENDOR/MANIFEST.txt" | head -1)"
OUT="$REPO_ROOT/out/differential"
RUN_TS="$(date -u +%Y%m%dT%H%M%SZ)"
REPORT="$OUT/report-$RUN_TS.md"
: "${CHECK_TEST_BIN:=check}"
: "${CHECK_FILTER:=parity_}"

WITH_ORACLE=1
KEEP_TEMP=0
for arg in "$@"; do
  case "$arg" in
    --no-oracle) WITH_ORACLE=0 ;;
    --keep-temp) KEEP_TEMP=1 ;;
    -h | --help) sed -n '2,52p' "$0"; exit 0 ;;
    *) echo "unknown arg: $arg (try --help)" >&2; exit 2 ;;
  esac
done

[[ -f "$MUT" ]] || { echo "detection corpus missing: $MUT" >&2; exit 2; }

mkdir -p "$OUT"
TMPROOT="$(mktemp -d "${TMPDIR:-/tmp}/differential.XXXXXX")"
# shellcheck disable=SC2329  # invoked via the EXIT trap below
cleanup() { [[ "$KEEP_TEMP" -eq 1 ]] || rm -rf "$TMPROOT"; }
trap cleanup EXIT

log()  { printf '[differential] %s\n' "$*" >&2; }
rule() { printf '\n========== %s ==========\n' "$*" >&2; }

# Accumulators for the final verdict.
ORACLE_STATE="skipped"
ORACLE_VALIDATE=""
ORACLE_WORLD=""
OURS_STATE="unknown"
OURS_DISAGREEMENTS=""
CHECK_STATE="not-present"
DETECT_TOTAL=0
DETECT_CAUGHT=0
DETECT_LINES=""

# ===========================================================================
# Phase A — the pinned legacy oracle over the vendored corpus.
# ===========================================================================
rule "PHASE A: pinned legacy oracle"
if [[ "$WITH_ORACLE" -eq 0 ]]; then
  log "phase A skipped (--no-oracle)"
elif ! command -v uv >/dev/null 2>&1; then
  log "phase A skipped: uv not on PATH — oracle not provisionable (migration.md A3)"
elif ! "$ORACLE" info >/dev/null 2>&1; then
  log "phase A skipped: pinned-oracles.sh info failed"
else
  log "provisioning / confirming the pinned oracle at $PINNED_SHA"
  if "$ORACLE" provision >>"$OUT/oracle-provision-$RUN_TS.log" 2>&1; then
    ORACLE_VALIDATE="$("$ORACLE" validate 2>/dev/null | tail -1)"
    ORACLE_WORLD="$("$ORACLE" world-build 2>/dev/null | grep -E '^world:' | head -1)"
    log "oracle validate : ${ORACLE_VALIDATE:-<no output>}"
    log "oracle world    : ${ORACLE_WORLD:-<no output>}"
    ORACLE_STATE="ran"
  else
    log "phase A skipped: provisioning failed (see oracle-provision-$RUN_TS.log)"
    ORACLE_STATE="provision-failed"
  fi
fi

# ===========================================================================
# Phase B — our side: the MIG-1 read-through (and CHECK-1 parity if present).
# ===========================================================================
rule "PHASE B: kernel side (MIG-1 read-through)"
MIG1_LOG="$OUT/mig1-$RUN_TS.log"
log "building the mig1_replay test binary"
if ! cargo test -p dli-world --test mig1_replay --no-run --locked >"$MIG1_LOG" 2>&1; then
  log "BLOCKED: dli-world does not compile — cannot run our side (see $MIG1_LOG)"
  OURS_STATE="blocked-compile"
else
  if cargo test -p dli-world --test mig1_replay --locked -- --nocapture >"$MIG1_LOG" 2>&1; then
    OURS_STATE="clean"
  else
    OURS_STATE="findings-or-fail"
  fi
  OURS_DISAGREEMENTS="$(grep -oE '[0-9]+ disagreement' "$MIG1_LOG" | head -1)"
  log "mig1_replay: $OURS_STATE (${OURS_DISAGREEMENTS:-count unknown})"

  # Optional CHECK-1 parity leg — the `check` test binary's `parity_*` cases
  # (compile()+check::run over the vendored corpus; coordinated with the check
  # lane). Invoked only if the binary is present and compiles.
  if cargo test -p dli-world --test "$CHECK_TEST_BIN" --no-run --locked \
       >"$OUT/check-probe-$RUN_TS.log" 2>&1; then
    log "CHECK-1 parity leg present (--test $CHECK_TEST_BIN) — running ${CHECK_FILTER}*"
    if cargo test -p dli-world --test "$CHECK_TEST_BIN" --locked "$CHECK_FILTER" \
         >"$OUT/check-$RUN_TS.log" 2>&1; then
      CHECK_STATE="clean"
    else
      CHECK_STATE="findings-or-fail"
    fi
  else
    log "CHECK-1 parity leg (--test $CHECK_TEST_BIN) not present/compiling yet — skipped"
  fi
fi

# ===========================================================================
# Phase D — detection proven SEPARATELY from parity (before it counts).
# Applies three named operators' data-layer analogues to a TEMP copy of
# facts.yaml and re-runs the read-through, asserting each becomes a finding.
# ===========================================================================
rule "PHASE D: detection proof (seeded operators over a temp corpus)"
PRISTINE_TEST="pristine_facts_have_zero_status_disagreements"

detect_op() {
  # $1 = operator name, $2 = sed program applied to the temp facts.yaml
  local op="$1" sed_prog="$2" log_file
  local tmp="$TMPROOT/$op.facts.yaml"
  log_file="$OUT/detect-$op-$RUN_TS.log"
  cp "$FACTS" "$tmp"
  sed -i "$sed_prog" "$tmp"
  DETECT_TOTAL=$((DETECT_TOTAL + 1))
  # Our side re-run over the mutated corpus MUST now fail (the catch). A nonzero
  # exit from a *compile* failure is NOT a catch — it would be a false positive —
  # so the log is checked for a compile error before counting the failure.
  if MIG1_FACTS_YAML="$tmp" cargo test -p dli-world --test mig1_replay --locked \
       "$PRISTINE_TEST" >"$log_file" 2>&1; then
    DETECT_LINES+="  MISS   $op — the read-through still passed (harness did NOT catch)\n"
    log "MISS   $op (NOT caught)"
  elif grep -qE 'error\[E[0-9]+\]|could not compile' "$log_file"; then
    DETECT_LINES+="  BLOCKED $op — the tree re-broke mid-run; not a catch (compile error)\n"
    log "BLOCKED $op (compile error mid-run — not counted as a catch)"
  else
    DETECT_CAUGHT=$((DETECT_CAUGHT + 1))
    DETECT_LINES+="  CAUGHT $op — the mutated corpus turned the clean read-through into a finding\n"
    log "CAUGHT $op"
  fi
}

if [[ "$OURS_STATE" == "blocked-compile" ]]; then
  log "detection skipped: dli-world does not compile (phase B was blocked)"
  DETECT_LINES="  BLOCKED — dli-world did not compile; detection not run this cycle\n"
else
  # 1) flip-proposed-to-ratified: promote a proposed row to ratified, no citation.
  detect_op "flip-proposed-to-ratified" '0,/status: proposed}/s//status: ratified}/'
  # 2) forged-promotion (strip-decisions): a ratified row loses its ruling.
  detect_op "strip-decisions-from-ratified" '0,/status: ratified, decisions: \[[^]]*\]}/s//status: ratified}/'
  # 3) break-status-marker: corrupt a status so it resolves to no provenance regime.
  detect_op "break-status-marker" '0,/status: ratified/s//status: ratified-broken/'
  log "detection: $DETECT_CAUGHT/$DETECT_TOTAL operators caught"
fi

# ===========================================================================
# Phase C — the divergence report (written last, over all phases' verdicts).
# ===========================================================================
rule "PHASE C: divergence report"
{
  echo "# Differential-oracle run — $RUN_TS"
  echo
  echo "- pinned SHA: \`$PINNED_SHA\`"
  echo "- corpus: \`fixtures/vendor/fantasy-40f6679\` (facts.yaml + full canon)"
  echo "- harness: \`scripts/differential-oracle.sh\` (lab-side, non-gating)"
  echo
  echo "## Verdicts"
  echo
  echo "| Leg | State | Verdict |"
  echo "|---|---|---|"
  echo "| oracle \`validate\` (whole canon) | $ORACLE_STATE | ${ORACLE_VALIDATE:-—} |"
  echo "| oracle \`world-build\` (whole canon) | $ORACLE_STATE | ${ORACLE_WORLD:-—} |"
  echo "| kernel MIG-1 (facts.yaml) | $OURS_STATE | ${OURS_DISAGREEMENTS:-—} |"
  echo "| kernel CHECK-1 parity | $CHECK_STATE | — |"
  echo
  echo "## Detection (proven separately from parity)"
  echo
  echo "Operators mirror the named seeds in \`fixtures/mutation/canon.yaml\` (EXT-F5)."
  echo
  echo "$DETECT_CAUGHT of $DETECT_TOTAL seeded operators caught by the kernel read-through:"
  echo
  printf '%b' "$DETECT_LINES"
  echo
  echo "## Triage"
  echo
  echo "- **Scope gap (known-divergence, deloreis).** The oracle legs range over the"
  echo "  whole vendored canon; MIG-1 ranges over \`facts.yaml\` only (migration phase 1"
  echo "  is the facts read-through). A row-level parity diff is CHECK-1 / MIG-2 work,"
  echo "  not this cycle — the two counts are not comparable and are not treated as one."
  echo "- Any real disagreement surfaced by either side is a finding, assigned a class"
  echo "  from migration.md §3 (parity-match / known-divergence / kernel-bug /"
  echo "  oracle-bug / corpus-gap), never silently reconciled."
} >"$REPORT"
log "wrote $REPORT"

# ===========================================================================
# Final verdict.
# ===========================================================================
rule "SUMMARY"
cat "$REPORT" >&2
echo >&2

# The harness's own exit code: it is HEALTHY when detection is proven (every
# seeded operator caught) and our side is not compile-blocked. Parity numbers are
# reported, never gated on — this script is a lab instrument, not a gate.
if [[ "$OURS_STATE" == "blocked-compile" ]]; then
  log "VERDICT: BLOCKED (dli-world did not compile this cycle)"
  exit 3
elif [[ "$DETECT_TOTAL" -gt 0 && "$DETECT_CAUGHT" -eq "$DETECT_TOTAL" ]]; then
  log "VERDICT: detection PROVEN ($DETECT_CAUGHT/$DETECT_TOTAL); differential recorded"
  exit 0
else
  log "VERDICT: detection INCOMPLETE ($DETECT_CAUGHT/$DETECT_TOTAL caught) — investigate"
  exit 1
fi
