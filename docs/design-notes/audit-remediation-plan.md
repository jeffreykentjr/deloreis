---
status: active
verdict: "Implementation plan for the 2026-07 audit remediation. Evidence, not authority — mints no DL ids itself; DL entries minted during execution."
---

# Audit remediation plan — 2026-07

> Tracks the systematic fix of all 18 MAJOR and 18 MINOR findings from
> `docs/design-notes/audit-2026-07.md`. Four milestones, each ending with a
> red-team checkpoint.

## Milestone 1 — Doc-resync + Guard Holes

### Task 1: Doc-resync commit (§3.1 items 1–9 + §3.2)

Bring all 9 hand-written surfaces into agreement with shipped code, record the
5 read tools.

- Update `docs/PLAN.md:538-553` — parser is live, zero `todo!()`, 46 ops,
  `golden_dispatch_matches` un-ignored.
- Update `docs/integration/fantasy-contract.md:102-106` — remove
  capability-gated claims; all 46 wired.
- Update `docs/integration/fantasy-contract.md:135-150` §3.2 table — add 5
  rows for read tools.
- Update `AGENTS.md:51-55` and `docs/PLAN.md:80-82` — describe parser-backed
  docs-oracle.
- Fix `docs/integration/migration.md:262` — replace `pack` with `scope`; fix
  `merely-possible` cite.
- Fix `crates/dli-serve/src/handlers.rs:11-16` doc-comment — remove dead
  `NotWired` narrative.
- Fix `crates/deloreis/src/cmd/serve.rs:12-16` doc-comment — remove
  "unimplemented until AUTH-1".
- Remove "SPIKE ONLY" clippy allows from `Cargo.toml:56-59`.
- Update `README.md:13` — replace "Spike scaffold."
- Add staleness banner to `docs/design-notes/spec-accuracy-audit.md:51`.
- Mint DL entry for the 5 read tools.
- Add read-tools section to `docs/spec/11-agent-authoring.md`.

### Task 2: Fix docs-oracle recursive walk (§3.4 — F3T-4)

In `xtask/src/docs_oracle.rs`, replace `fs::read_dir(&examples_dir)` with
`WalkDir::new(&examples_dir)` to collect all `*.dli` recursively. Mirror fix
in `crates/dli-syntax/tests/corpus.rs` and `xtask/tests/docs_oracle.rs`.

### Task 3: Re-anchor craft xfail-strict guard (§3.5 — F4T-1)

Replace `assert_no_craft_gate_yet` with `assert_craft_gate_landed` that builds
a payload with a known craft violation and asserts `check_payload` returns a
`craft.*` finding. Upgrade the 3 pending records.

### Task 4: Red-team checkpoint 1

Red-team agent tries to fool each fixed guard. Full gate green.

## Milestone 2 — Core Semantics

### Task 5: Implement per-valuation grade semantics (TESTQ-CORE-1)

Introduce `VarKind` metadata; implement per-valuation grade (join over ν-true
monomials, meet of variable kinds). Fix grade-laundering counterexample.

### Task 6: Align negation semantics (TQ-N1, TQ-N2)

Implement certain-layer-only negation with open-variable poisoning. Fix
chained negation. Extend props.rs generator for negation (TQ-N3). Mint DL
entry.

### Task 7: Fix expert-tier parser silent drops (§3.3 — F3T-1, F3T-2)

Add AST homes for expert-header content (labels, provenance markers). Add
`expect_consumed` to naked arms. Fix WordDecl.name_parts. Fix formatter
braced-block parenthesization.

### Task 8: Red-team checkpoint 2

Adversarial tests on grade, negation, and parser safety.

## Milestone 3 — Craft Infrastructure

### Task 9: Fix finding-families page (§3.6)

Add `craft.*` family to `FAMILIES` array. Fix substring scraper. Regenerate.

### Task 10: Build CraftConfig→EpisodePolicy bridge (§3.8)

Implement `build_episode_policy(cfg)`. Move D-nnnn defaults to
`worlds/tpth/craft/*.toml`. Mint DL entry.

### Task 11: Wire killed-token ingestion channel (F8-2)

Add `--killed <path>` to check-draft. Update serve path.

### Task 12: Red-team checkpoint 3

## Milestone 4 — Guard Hardening + MINOR Sweep

### Task 13: Unify token lists (§3.7)

One canonical superset. Fix spaced-prose matching. Fix `#[cfg(test)]` mention
poisoning.

### Task 14: Add .dli canonicality gate step (F3T-3)

Add `deloreis fmt --check` gate step for all committed `.dli` files.

### Task 15: Proof-gap closures (MINOR)

TQ-N3, TQ-N4, TESTQ-CORE-2/3/4, F4T-2/3/4, F3T-5, F5S-5.

### Task 16: Staleness/hygiene sweep (MINOR)

F3T-6/7, F5S-6/7/9, F8-4, F7B-4/5.

### Task 17: Red-team checkpoint 4 (final)

Re-execute all "fooling edits" from §5. Full gate green. Update audit doc with
remediation SHA.

## Process notes

- Each task commits via `scripts/safe-commit.sh "<msg>" <files...>`
- Push on green gate
- Agent team uses separate worktrees per cluster within a milestone
- Red-team checkpoints are coarse-grained (per milestone, not per task)
- Policy questions §7 deferred: LOC band, factory integration, craft
  thresholds ownership, D-nnnn scan exemption — not in scope for this pass

## Deferred (not in scope)

- LOC band DL entry (stale/irrelevant per author)
- Full factory integration (worlds/tpth ownership clarity)
- Engine-default craft thresholds ownership (integration-time)
- D-nnnn scan exemption scope (immaterial)
