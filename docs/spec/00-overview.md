---
status: live
verdict: "The spec entry point: De Lore Is is a deterministic epistemic engine for agents; its core evaluates structured claims, provenance, defeaters, and valuations, while fiction and TPtH remain first-class reference domains."
---

# Spec 00 — overview

The entry point to the specification. It states what the engine computes,
separates the reusable core from its domain modules, and points each reader at
the next document. The decision ledger (`../decisions.md`) outranks this file.

## What De Lore Is is

De Lore Is is a **deterministic epistemic engine** for agents and the
applications that supervise them. It stores typed claims as immutable fact
objects, records their provenance and explicit defeaters, and derives each
claim's standing — `certain`, `possible`, or `dead` — plus an explanation that
can be replayed from committed inputs.

DLI is infrastructure, not an agent. It does not call a model, decide an
application's policy, or decide who is entitled to approve a claim. Callers
supply attribution and decision state; DLI records them and verifies the
referential integrity it can verify, including that a cited decision exists.
The consuming application owns authorization, review, escalation, and any
meaning of "canon."

The `.dli` text surface survives as an opt-in rendering. The normative language
definition is the schemars-derived schema of lorestore fact objects and their
JCS-canonical bytes (DL-0041).

## The epistemic core

The reusable core answers questions that a retrieval system or an unstructured
agent transcript cannot answer deterministically:

- What claims hold under this committed snapshot and valuation?
- What support and defeaters make them hold, remain open, or fail?
- What changes when an application supplies a different decision or
  hypothetical valuation?
- Which exact constraints or derivation paths conflict?

The engine computes one symbolic fixpoint, then applies valuations over that
result. It therefore supports replayable `why` answers and cheap local
what-if evaluation without treating a model's prose or a majority vote as a
verdict. This is a semantic boundary, not a promise that DLI independently
settles factual disputes: structured evidence, rules, and decision policy still
come from the application.

## Domain modules and reference applications

The core is deliberately general. The current repository also ships domain
modules that exercised it in fictional-world authoring and remain supported:

| Area | Role |
|---|---|
| [Time](03-time.md), [practices](04-practices.md), [language](05-language-layer.md), and [craft](06-craft-and-payloads.md) | Optional domain models over the common fact/provenance substrate. |
| [Exports](09-exports.md) | One-way projections from evaluated knowledge to consuming tools. |
| TPtH | A first-class, worked reference corpus and integration, not a limit on the engine's use. |
| [Serving](10-serving.md) and [agent authoring](11-agent-authoring.md) | The JSON/MCP interface through which applications and agents interact with the engine. |

Fiction remains valuable because canon, retcons, alternatives, and authored
rules make provenance visible. The same machinery applies to an operational,
research, or multi-agent corpus when its inputs are made explicit.

## Evaluation, scale, and consensus

The current external evidence program is maintained in the sibling
`dli-benchmarks` repository. Its relevant suites are SubtleMemory, ClawArena,
FlawedFictions, and ConStory. Their protocols, costs, baselines, and results
belong there; they are non-gating evidence for this repository, not claims made
by this specification.

At atelier scale, DLI runs embedded and saturates a whole corpus locally. Its
archive destination is a local peer plus managed service: the service bounds an
immutable query-relevant subgraph, and the peer performs exact symbolic
evaluation and overlays from its local cache. [Scale and distribution](12-scale-and-distribution.md)
specifies that planned boundary.

This also supplies the deterministic substrate for future multi-agent debate
and consensus products: agents can submit structured claims and receive
standing, conflict maps, and explanations. Winner-selection, trust weights,
argumentation semantics, and human escalation policy are not yet DLI
semantics; applications own them until a later decision specifies otherwise.

## Reading order, per role

- **A contributor:** the charter (`../charter.md`) → this file → the relevant
  spec → `../PLAN.md` → `../decisions.md`.
- **An integrator or agent-system builder:** this file →
  [semantics](02-semantics.md) → [reasoning](08-reasoning.md) →
  [serving](10-serving.md) → [agent authoring](11-agent-authoring.md).
- **A domain author:** this file → the relevant domain module → the TPtH
  worldbook or another consuming project's corpus.
- **A scale/distribution implementer:** this file →
  [scale and distribution](12-scale-and-distribution.md) → `../PLAN.md` → the
  lorestore roadmap.
