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

## Novel benchmark continuation exception (2026-08-20)

The external `dli-benchmarks` novel benchmark is continuing past the first
large Slice 1 timeout under an explicit exception. The raw DLI timeout rows
remain authoritative evidence and are never rewritten as successful engine
observations. The continuation may add separately named proxy rows derived
from the deterministic theory reference solely to keep later slices moving;
those rows are excluded from primary DLI correctness and property scores and
are reported as `engine-timeout-proxy`/asterisked data.

The observed failure is tracked in [GH #99](https://github.com/jeffreyksmithjr/deloreis/issues/99).
At revision `dfffbb0fcd0349293af53699e0f6467f60010948`, authoring 65,536 facts
one at a time left the worker CPU-bound at about 99% and produced seven
900-second client-deadline failures. The suspected path is
`author.add_fact` → `stage` → `attr_findings` → `known_entities`, where the
last function scans all staged declarations on every fact. The resulting
upper-bound estimate is 2,147,516,416 declaration inspections. This is an
engine-performance failure, not a semantic result, and the benchmark's
continuation must not be cited as proof that DLI handled those theories.

The exception also records that Slice 2 and later indicative runs are not a
clean preregistered continuation: any result relying on the proxy is
conditional, the engine-timeout count is reported separately, and the clean
line remains pending an engine fix and rerun. No contract or core semantic
change is authorized by this note.
