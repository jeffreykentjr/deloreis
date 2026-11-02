---
status: reference
title: "Sibling lessons: the receipts behind the laws"
verdict: "laws in the charter and AGENTS.md were paid for elsewhere; here are the receipts, condensed."
---

# Sibling lessons

Several laws in `charter.md`, `decisions.md`, and `AGENTS.md` were not reasoned
from first principles here — they were paid for in the author's other projects.
This note is the receipt for each, condensed. It is `status: reference`: the
lessons are settled, not up for re-argument. The earlier private research is preserved in git history.

## The path-count lesson → koan 11, DL-0004

**Chelis** ran a dual-syntax design whose syntax-redesign budget was consumed by
churn between surfaces; it shipped structural editing without ever measuring it
against text editing; and it recorded **12–31% bust rates in unmanaged
multi-agent authoring.** Across Chelis and **FlukeBall**, one relationship held:
**the count of supported paths predicted maintenance load and failure surface
better than any path's individual quality.** Every additional supported path is a
support surface (docs, examples, skill guidance, CI checks), a drift surface
(parallel artifacts kept equivalent), and a bust-rate multiplier (each path fails
its own way, and paths interact). The remedy is a law: **one door per author
population** (koan 11, DL-0004). Two populations exist — humans and agents — so
two doors are supported, and the benchmark exists to *close* doors, never to open
them.

## The agent-syntax history → DL-0004

The honest residue of the sibling agent-authoring work is what is still
*unknown*: no frontier-model measurement of the textual repair loop exists, and
the local-model evidence is one model class. What the record does establish is
which levers are reproducible: **grammar regularity and canonical form** reduce
agent failure, and **task complexity dominates failure variance**. Those levers,
not faith, justify the AST-first design (DL-0003) and the demotion of raw AST
emission to a benchmark arm. The benchmark's job is a verdict about which door to
keep shut, defined by the real gate's pass — not a menu of channels.

## The hull pattern → DL-0012, migration

**hull** self-hosts an executable spec that differentially tests the real
compiler over a deterministic generated corpus, with **detection proven
separately from agreement** — 10,000 of 10,000 agree, zero unsound, and the
harness is shown *able* to fail before its agreement counts. That split is the
whole migration protocol (DL-0012): the legacy gates are the reference oracle,
the mutation operators are the generated corpus, and a parity harness that only
ever agrees is a null oracle until it catches seeded divergence. It also shapes
the Lean posture (`spec/08-reasoning.md` §7): the serialized-ledger corpus runs
through both the Rust checker and the Lean spec, verdicts diffed in CI.

## The beacon posture → koan 12, DL-0005

**beacon** reimplements the ideas of the CROWN verification literature so its
verdicts are *certified* rather than float-computed, and keeps the external
tools — α,β-CROWN itself, which it outruns roughly 150× in its target regime — as
comparison **baselines, never dependencies.** Stated as law: **reimplement for
checkability rather than rent for coverage** (koan 12). It is why the certifying
evaluator is owned, why clingo is retained only as a lab-side differential oracle,
and why zero foreign reasoning processes sit in any gating or serving path
(DL-0005). The companion project **LaCaDiLE** — a mechanized Lean spec running
ahead of implementation, machine-audited against prose overclaim (around 1,460
theorems, zero sorries, roughly 60 negative theorems pinning the boundary from
outside) — is the pattern the deferred Lean spec organ would follow. And
**latent-waite**, a deterministic knowledge-base application that worked cleanly,
is the standing existence proof that a checkable lore knowledge base is not
speculative — its validated patterns are prior art for the fabric.

## The serial-collision lesson → DL-0011

Concurrent minting of identifiers into a shared file collides and does not merge.
The remedy is **single-writer ID minting**: `DL-nnnn` IDs are minted only in
`decisions.md`, one writer per commit, mint-and-use in a single commit, rebase
before push (DL-0011). The whole docs layer inherits the discipline — the ledger
is append-only and single-writer because that is the shape that survives many
agents at once.

## The flaky-factory-CI lesson → DL-0013

Letting model calls, live baselines, or benchmarks into the gate makes the gate
non-reproducible and lets its flakiness block unrelated merges. The remedy:
**the language gate is deterministic offline lanes only**, and anything flaky or
live — models, the Ludii baseline, the authoring benchmark, the clingo oracle —
runs in **non-gating** lanes (DL-0013). The verdict that blocks a merge must
reproduce with no network and no model in the loop.

## Salsa-readiness as law → DL-0009

Incremental-recomputation frameworks are a rewrite if retrofitted and a
premature dependency if adopted early. The remedy is to write the kernel
**salsa-ready without salsa**: pure phases, per-invocation environments,
content-hash keys, and no stateful compiler struct holding derived state across
calls (DL-0009). Purity keeps incrementality a later drop-in rather than a
rearchitecture, and the adoption trigger is **measured, not anticipated** — salsa
lands only when measured incremental-rebuild latency breaches the CI envelope.
