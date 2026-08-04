---
status: live
verdict: "The committed progress log for the cross-repo cutover campaign: what has cleared at each stage boundary and the evidence for it. Orchestrator single-writer, deloreis side; appended to, never rewritten."
---

# Cutover campaign — progress log

This is the running record of the **cross-repo cutover campaign**: what has
actually cleared, stage by stage, and the evidence that says so. It lives in the
deloreis tree and is committed here, but it logs **both** repositories' progress,
because the campaign is one plan run across two checkouts. Fantasy-side work is
cited by PR number and by `D-nnnn` ledger id; deloreis work by PR number and
`DL-nnnn`.

It is **orchestrator single-writer** — only the orchestrating agent appends to
it, the same discipline `STATUS.md` carries. Unlike `STATUS.md`, this file makes
no claim on the fantasy repository's own records; it is a log, not a handshake.

The plan it logs against is `docs/integration/cutover-campaign.md` (deloreis
half) and the consumer's `docs/proposals/DP-013-deloreis-full-cutover.md`
(fantasy half). Where this log disagrees with `docs/decisions.md`, the ledger
wins. A stage is appended here **after** it closes, with its evidence; a stage
in flight appears only under "In progress".

## Stage 0a — Governance rewrite (cleared)

Both repositories replaced request-and-wait supervision with machine minting
plus attribution, so that no later stage can block on a human's availability.

| Repo | Landed | Ruling |
|---|---|---|
| deloreis | PR #26, merged | DL-0052 — machine agents mint their own entries; every entry carries `machine` or `human` attribution; supervision is async and steers as repair |
| fantasy | PR #8, merged | D-0247 — the consumer-side counterpart |

Both PRs were red-teamed and came back clean. This stage is a precondition for
every later one: the campaign's sync points are machine-to-machine, and this is
the ruling that makes that legitimate rather than merely convenient.

## Stage 1 — DM-1 handshake and the first tag (cleared; SP-1 delivered)

The consumer's vendored map of this engine was a full major stale. Stage 1
brought it to truth and then cut the artifact the consumer pulls from.

- **deloreis PR #27, merged at `76888052`** — "DM-1: integration contract to
  1.0; mint DL-0053, DL-0054". `docs/integration/fantasy-contract.md` moves to
  `1.0.0` truth: 48 operations counted against `crates/dli-serve/src/ops.rs`
  (27 `dli.*` plus 21 `author.*`, with `author.branch` and `author.merge` now
  in the inventory), and `author.materialize` corrected to its DL-0046
  semantics — a mutation and the accept decision, answering JSON-first with
  `.dli` rendering behind `rendered: true`.
- **DL-0053** (attribution `machine`) — handshake delivery: the doc to 1.0, the
  tag, and retirement of prior versions through the killed-form registry rather
  than silently.
- **DL-0054** (attribution `human`) — the authority scrub: the engine records
  attribution and decision state as data and keeps referential integrity; role
  enforcement is a consumer concern. Ruled here, implemented on the wire at
  contract 1.2.0 (DM-4), so the code lands under an existing ruling.
- **Annotated tag `contract/1.0.0`** cut on that merge commit, so the tagged
  tree is self-consistent — contract doc and `contracts/` registry digests
  agree. Tag object `e239322c`; target `76888052`; present on the remote.
  Cutting it was **SP-1**, the campaign's highest-leverage single action: it
  unblocks consumer vendoring (CP-1), their real-binary CI lane (CP-9), and the
  EXT-F4 run (CP-10).
- **STATUS advance**: CP-1, CP-9, and CP-10 moved `proposed` → `provided`, one
  rung, with evidence filled in the same commit. CP-10 additionally cites the
  frozen AUTH-2 worked-scenario fixture under `worlds/tpth/auth2/`, replayed by
  `cargo test -p dli-serve --test auth2` — the bytes the consumer's transcript
  is compared against, deliberately left pinned at contract `0.5.1` in all
  fourteen frames so their EXT-F4 run is decoupled from their 1.0 wire upgrade.

The three rows stop at `provided` by the ladder rule. `acknowledged` requires
the consumer's vendored SHA and digest-check output; `verified` requires their
replay artifact. Neither is evidence deloreis can produce.

## In progress

- **FW-1** (fantasy) — pin consolidation on the consumer side.
- **DM-2** (deloreis) — the lorestore-backed project loader and
  `SCHEMA_VERSION` 2 → 3, which must land before any consumer store exists.
