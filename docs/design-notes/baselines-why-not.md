---
status: reference
title: "Baselines: why not Ash, why not Python + clingo"
verdict: "why this exists: the two nearest alternatives, honestly accounted, once."
---

# Baselines: the two nearest alternatives

This note records, once, why De Lore Is is a bespoke Rust language rather than
either of the two realistic off-the-shelf routes. Both were worked honestly, not
as strawmen. This is `status: reference`: it is the evidence behind the build
decision, not a live proposal, and it is **not re-litigated without new evidence
entering the ledger**. The full accounting is in the fantasy repo,
`docs/research/RR-018-lore-language-fifth-pass.md` §10.

## Baseline 1 — build it in the Elixir factory

The factory (Phoenix/LiveView + Ash + Oban) is a real system with proven idioms,
so this baseline earns a straight accounting.

**What Ash gives nearly free:** entities, relations, and facts as resources with
citation join tables; status as an enum column; functional dependencies as
identities; LiveView reads over all of it with zero protocol work; Oban for
simulation jobs; Ash policies that can express "the machine never writes canon"
as an actual authorization rule. That is most of the possible-worlds *browser*
and all of the sim *orchestration* — genuinely more UI, sooner, than the kernel
plan produces.

**What has no precedent there, and lands in the untyped half:** the annotation
machinery — the Boolean expression of citations and assumptions that travels
with every derived fact — has no Ash or Ecto idiom. It becomes JSONB columns
manipulated by hand-rolled Elixir with no compile-time help and no property
tests. Three consequences are decisive:

- **Derived status would be stored.** The factory's existing derived-readiness
  pattern *stores* a derived flag via a Postgres trigger — the exact opposite of
  this design's core commitment that status is computed, never stored, so it can
  never go stale against its own citations. Storing it is the anti-pattern the
  language exists to prevent.
- **Forks smear across the codebase.** A fork becomes row-copying or a `fork_id`
  threaded through every query — a cross-cutting concern smeared across
  application code rather than absorbed by an evaluator.
- **The fixture wall dies.** DP-004 (a live proposal in the consuming fantasy
  repo, `docs/proposals/`, assigning Ash the storage-mirror and UI/orchestration
  seat) has an architecture that sandboxes workers behind
  a JSON-Schema boundary with golden/negative/framing fixtures. Implementing
  lore *inside* the factory deletes that wall for the most consequential worker
  of all: there is no longer any separation between "the factory misrendered it"
  and "the kernel misjudged it," and reasoning bugs share a runtime, a deploy
  cadence, and a failure domain with the control plane.

The fair one-liner: **Ash is the right home for the corpus's storage mirror and
the UI/orchestration surface — which is exactly what DP-004 already assigns it.**
The annotated-deduction core is bespoke and hard in either language; the real
question is whether it lives in the factory's untyped half on the factory's
release cadence, or behind the protocol as its own typed, separately-testable
thing. It lives behind the protocol, and this baseline, honestly worked, is why.

## Baseline 2 — Python + clingo + pydantic

Not a strawman; parts of this would be genuinely fast.

**What it delivers quickly:** clingo is a real, mature answer-set solver. Encode
facts and rules as ASP, open RQ options as choice rules, rejected lore as
integrity constraints, defaults as negation-as-failure, and enumeration of
consistent possible worlds *works today*, with cautious/brave reasoning (what
holds in all worlds versus some world — precisely the certain/possible
distinction) built in. Pydantic is already the house style. A working
forks-and-what-if demonstrator is plausibly quick.

**What it doesn't deliver:**

- **No compile-time anything.** Every discipline is a runtime convention holding
  only at call sites that remembered it.
- **No language artifact.** Statuses and citations stay library discipline; the
  compiler-as-agent-feedback-loop never exists.
- **Per-fact support reconstructed by hand.** ASP text becomes a second language
  of truth: clingo tells you *which worlds are consistent*, not *why each fact
  holds in them*. Per-fact support ("cites D-0119, assumes RQ-031(b)") must be
  hand-encoded into every rule — clunky and invasive, and it is exactly the part
  that must not be clunky, because **citations are the product**.
- **Two-language debugging.** Grounding-error diagnostics are notoriously opaque,
  now across a Python/ASP surface.
- **The author has exited Python for this system.**

**The narrow honest claim, preserved:** if the dominant workload turned out to be
heavy *combinatorial search* over candidate worlds and rulesets rather than
provenance-carrying deduction, clingo would be the fastest route to that one
capability, and nothing in the kernel replicates a competitive solver. That is a
**design input** for the reasoning inventory (`spec/08-reasoning.md`) — which
concluded the search-shaped tasks are peripheral and proposer-shaped — **not a
reason to stay.** The first release discharges everything it needs without any
solver, and if enumeration-at-scale ever becomes load-bearing, clingo embeds
from Rust behind the kernel boundary as a differential oracle (DL-0005). Even
the one capability this baseline is genuinely best at does not require adopting
the rest of it.
