---
status: reference
title: "The standing differential-oracle harness (PROOF-2)"
verdict: "The legacy fantasy gates stand beside the kernel over one pinned corpus; verdicts are diffed and detection is proven separately from parity — a harness that only ever agrees is a null oracle."
---

# The standing differential-oracle harness

PROOF-2 is the shadow-read instrument: it runs the legacy fantasy oracle and the
De Lore Is kernel over the **same pinned inputs** and diffs their verdicts, so
that no check is ever declared at parity on faith. It is the standing companion
to MIG-1 (the `facts.yaml` read-through) and the runway to MIG-2's dual-run.

The load-bearing rule it enforces (`docs/integration/migration.md` §3): **a
harness that only ever reports agreement is a null oracle.** Detection is proven
*separately* from — and *before* — any parity claim counts.

## What it is (and is not)

- **Lab-side, non-gating, never CI.** The deloreis gate (`cargo xtask gate`) is
  deterministic, offline, and model-free. This harness clones another repo and
  drives its Python toolchain, so it is never wired into a gating lane. The one
  piece of it that *is* gate-run is the kernel read-through itself
  (`mig1_replay`), which is offline and pinned.
- **Two artifacts.**
  - `crates/dli-world/tests/mig1_replay.rs` — MIG-1: the `facts.yaml`
    status-vs-derived cross-check plus its in-code detection proof. Offline,
    committed, gate-run.
  - `scripts/differential-oracle.sh` — PROOF-2: the standing orchestration.
    Lab-side, writes untracked reports under `out/differential/`.

## MIG-1 — the facts.yaml read-through

Migration phase 1 reads the vendored `facts.yaml` directly and, for every
provenance-bearing row, cross-checks the stored `status:` field against the
standing the kernel derives from the row's `decisions:` under the wave-1
semantics (`dli_core::ann::Ann::status` over a support polynomial built exactly
as the loader builds it):

| stored `status:` | kernel support (default-true valuation) | derived standing |
|---|---|---|
| `ratified` / `provisional` / `decided` | ⋁ of cited decisions, each ruled true | `Certain` |
| the same, **citing no decision** | `P⁺ = 0` (fail-closed) | `Dead` — a finding |
| `proposed` | one open hypothesis variable, never ruled | `Possible` |
| `rejected` | `P⁺ = 0` | `Dead` |
| anything else | unresolvable to a provenance regime | a finding |

A **disagreement** is any row where the stored status and the derived standing
land in different cells. Every disagreement is a finding, never a silent fix —
if the first characterization run were nonzero, each case would be pinned in the
test with an explanation.

## PROOF-2 — the standing differential

`scripts/differential-oracle.sh` runs four phases:

- **A. Oracle** — `pinned-oracles.sh validate` + `world-build` over the pinned
  fantasy checkout. Skips cleanly when the oracle is not provisionable (no `uv`,
  or the SHA is absent — migration.md assumption A3: no oracle, no flip).
- **B. Our side** — the MIG-1 read-through (`cargo test … mig1_replay`), plus the
  optional CHECK-1 parity leg: the `check` test binary's `parity_*` cases
  (`compile()` + `check::run` over the vendored corpus), probed and skipped
  cleanly when absent. Overridable via `CHECK_TEST_BIN` / `CHECK_FILTER`.
- **C. Divergence report** — both sides' verdicts to `out/differential/`, with the
  triage classes and the known scope gap recorded.
- **D. Detection** — three named operators' data-layer analogues applied to a
  **temp copy** of `facts.yaml`; our side re-runs over each mutated corpus and
  must turn the clean read-through into a finding.

Run it:

```bash
scripts/differential-oracle.sh              # full run (oracle + kernel + detection)
scripts/differential-oracle.sh --no-oracle  # offline: skip phase A
scripts/differential-oracle.sh --keep-temp  # keep the mutated temp corpora
```

Exit codes: `0` detection proven and differential recorded; `1` detection
incomplete (investigate); `3` blocked (dli-world did not compile this cycle).

## Detection, proven separately from parity

The three seeded operators mirror the named seeds in
`worlds/tpth/mutation/canon.yaml` (EXT-F5), re-expressed against `facts.yaml`
because no corpus operator targets that file directly:

| operator | corpus analogue | the seeded change | why our side catches it |
|---|---|---|---|
| `flip-proposed-to-ratified` | same name | a `proposed` row → `ratified`, no citation added | ledger with no ruling derives `Dead`, not the claimed `Certain` |
| `strip-decisions-from-ratified` | `forged-promotion` | a `ratified` row loses its `decisions:` | same fail-closed `Dead` |
| `break-status-marker` | same name | a status token corrupted so it names no regime | unresolvable status is flagged directly |

The in-code proof lives in `mig1_replay.rs`'s `detects_*` tests (gate-run); the
shell harness additionally re-runs the read-through over a mutated temp corpus
file, exercising the real file-read path.

## Divergence triage (migration.md §3)

Every disagreement is assigned one class, with an owner:

| class | meaning | owner |
|---|---|---|
| `parity-match` | the two agree | — |
| `known-divergence` | the kernel is intentionally better | deloreis — documented as expected |
| `kernel-bug` | the kernel is wrong | deloreis — fix forward |
| `oracle-bug` | the legacy gate is wrong, the kernel is right | fantasy — noted; the kernel never regresses to match a legacy bug |
| `corpus-gap` | neither side has the data | fantasy — add a fixture |

### The standing scope gap (a documented `known-divergence`)

The oracle legs range over the **whole** vendored canon; MIG-1 ranges over
`facts.yaml` **only** (that is what migration phase 1 is). Their counts are
therefore *not* a row-level parity — the oracle's `world-build` census spans
`characters.yaml`, `timeline.yaml`, and the prose markers as well. Closing that
gap into a row-by-row diff is CHECK-1 / MIG-2 work; until then the two verdicts
are reported side by side and never reconciled into a single number.

## Current state — as of this run

- **Pinned SHA:** `40f66792229abf5d73d995a8167adc79152a64e7` (`fantasy-40f6679`).
- **Oracle (whole canon):** `validate` → `canon OK: 0 violations`; `world-build`
  → `world: 82 entities, 170 facts, 12 relations, 72 constraints` (`shadow: 4
  entities, 4 facts`, excluded from emission).
- **Kernel MIG-1 (facts.yaml):** 191 provenance rows cross-checked — 175
  `ratified`, 9 `provisional`, 7 `proposed` (3 constraints carry no status and
  are outside the cross-check). **0 stored-vs-derived disagreements** — the
  parity baseline.
- **Kernel CHECK-1 (`parity_*`):** present and clean this run — the clean-corpus
  verdict agrees with the oracle's `0 violations`.
- **Detection:** 3 / 3 seeded operators caught by the kernel read-through.
- **Harness verdict:** `exit 0` — detection proven, differential recorded.

The zero disagreement count is parity, not a null oracle: the same derivation
demonstrably catches all three seeded divergences.

The CHECK-1 `parity_*` leg (clean-corpus verdict, coordinated with the check
lane) is wired into phase B and consumed when its binary compiles. A row-level
verdict diff — every claim's `Supported`/`Contradicted`/`Unsupported` standing
diffed against the oracle's `world-build` — is MIG-2 dual-run work, built on
`dli_world::check::factcheck::factcheck() -> Vec<ClaimReport>`; until then MIG-1's
facts-layer cross-check plus the clean-corpus parity leg are the kernel's
standing shadow-read.
