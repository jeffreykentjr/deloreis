---
status: live
verdict: "The four-suite benchmark program is external, non-gating evidence for DLI's epistemic-engine direction. Protocols, baselines, costs, and results are owned by the dli-benchmarks repository."
---

# The benchmark program

The first evidence program for De Lore Is as agent infrastructure is maintained
outside this repository at `~/Documents/scratch/dli-benchmarks`. That repository
owns pinned upstream inputs, adapters, scoring, spend controls, run records, and
any claim about comparative results. This repository owns the deterministic
engine those adapters exercise.

## The four relevant suites

| Suite | Relevant DLI capability |
|---|---|
| **SubtleMemory** | Durable multi-session state, source/session provenance, and explicit contradiction handling. |
| **ClawArena** | Revisable operational state across rounds, tool-workflow consequences, and machine-checkable conflict reporting. |
| **FlawedFictions** | Contradiction detection and localization with preserved textual provenance. |
| **ConStory** | Long-horizon narrative/world consistency, where DLI addresses the factual, world, and timeline portions of the task. |

These are an evidence portfolio, not one synthetic metric. A result must retain
the upstream scoring boundary and matched baseline needed to interpret it. A
smoke, adapter test, or deterministic DLI diagnostic proves integration only;
it is not a benchmark win.

## What this repository may claim

DLI's documented mechanisms are hypotheses to test: structured claims can make
state handoffs more inspectable, provenance can localize contradictions, and
valuation/defeater semantics can expose the consequences of a changed premise.
The program aspires to strong results across all four suites, but no benchmark
result gates this repository and no outcome is inferred from design alone.

The program informs the archive and consensus tracks by producing real
ingestion, query, update, and explanation traces. It does not authorize a
change to core semantics; that still requires a decision-ledger entry and the
usual deterministic oracle.
