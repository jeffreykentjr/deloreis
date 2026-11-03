---
status: live
verdict: DL-0081, DL-0082
---

# Inertia: derived fluent persistence

This is the harmonized feature spec behind `author.declare_inertial` (contract
`1.8.0`). The ruling is `docs/decisions.md` **DL-0081**; the spec section is
`docs/spec/03-time.md` (the inertia subsection); this note carries the worked
semantics, interactions, wire surface, and v1 scope boundaries behind them. The
ledger outranks this note (DL-0011): where the two disagree, DL-0081 wins.

## Context — why this feature

The engine used to require callers to supply a validity interval
(`since`/`until`) for every fluent value; `continuity.fluent-contradiction` then
adjudicated overlap-vs-disjoint over intervals it had been *handed*.
FlawedFictions Round 3 measured that this misplaces the judgment: an LLM assigned
each value an interval from textual cues and the engine deterministically
relabeled it, so the bounding call *was* the transition-vs-clash judgment — the
engine only looked like it adjudicated, while the weak link was the LLM's
decomposition (inter-LLM consistency 0.706; documented failure modes:
over-applying `until` to fixed properties, defaulting `since` on relational
slots).

Inertia moves that judgment inside. A fluent value asserted at a point persists
forward until a later, distinct value defeats it. Intervals become **derived**,
not supplied; a later assertion becomes a defeater of the earlier value's
continued persistence. This is the frame problem's standard event-calculus
resolution and a logical extension of shipped machinery — fluent timelines with
per-span provenance (`Ann`), defeat as a valuation flip, and three-valued
temporal projection that reports undeclared ordering rather than inventing a
clash.

## A.1 Core semantics (v1)

- **Declaration.** `author.declare_inertial(pred)` declares predicate `pred`
  **inertial**: its fluent (state) values persist forward from their `since`
  anchor until defeated. It is **per-predicate** (author-ruled), mirroring
  `declare_functional`, and it is a defeasible claim — provenance and standing,
  can be cited, can be defeated. It is carriage-only: it mints no ground atom.

  ```dli
  scope camelot

  inertial water-status                       ratified D-0104
  ```

- **The point assertion.** On an inertial predicate, a value asserted `since t`
  with **no authored `until`** means "asserted at `t`, persists forward." The
  `since` is the only anchor the caller supplies; the `until` is derived. An
  **authored `until` overrides** derivation for that value (an explicit interval
  wins, so existing interval callers are byte-unaffected). A predicate never
  declared inertial keeps today's exact behavior: `since t` with no `until` is
  open-ended-forever.

- **The defeater — a later distinct value.** A value `vᵢ since tᵢ` is defeated
  from `tⱼ` forward by a **later, distinct, live** value `vⱼ since tⱼ` on the
  same slot (`tᵢ before tⱼ`). Declaring a predicate inertial declares it
  **single-valued-over-time**; a new distinct value *transitions* from the old.
  This stays koan-3 safe: persistence is *declared* (via `declare_inertial`), and
  "a later distinct value transitions" is inertia's *definition*, not an
  inference from data shape. Re-asserting the *same* value never punches a
  boundary; no explicit-defeater op is required.

- **The three outcomes** (composing with the existing three-valued projection —
  no fourth outcome):
  - **Transition (silent).** `tᵢ before tⱼ` certain ⇒ `vᵢ` holds `[tᵢ, tⱼ)`,
    `vⱼ` holds `[tⱼ, …)`. Provably disjoint ⇒ no finding.
  - **Contested (warn).** `tᵢ`/`tⱼ` order undeclared ⇒ neither value is certainly
    current downstream ⇒ `continuity.fluent-order-undeclared` (warn,
    non-blocking), whose repair names the `tᵢ / tⱼ` pair a `before`-fact would
    settle.
  - **Clash (error).** Two distinct values certainly co-hold and inertia cannot
    linearize them — the same assertion point (`tᵢ == tⱼ`), or authored
    overlapping intervals ⇒ `continuity.fluent-contradiction` (error), which bars
    materialize.

  Two *distinct* points that are `Unordered` are always settleable (the
  chronology carries no constraint beyond reachability), so "unordered" only ever
  means "contested" for a genuinely distinct pair. The single exception is the
  *same* point, where precedence also answers `Unordered` but the true verdict is
  simultaneity — a clash, never a manufactured `before(e, e)` repair candidate.

- **Returning value (a named case, not emergent).** A value may return after
  being defeated: `commons since dawn`, `enclosed since noon`, `commons since
  dusk` (dawn < noon < dusk). Because a defeater must be *distinct*, the third
  `commons` does **not** re-extend or reopen the first `commons` span — the first
  stays capped at `noon` by `enclosed`, and the third starts a **new** span
  `[dusk, ∞)`. Result: `commons [dawn, noon)`, `enclosed [noon, dusk)`,
  `commons [dusk, ∞)` — a clean return, no gap, no clash (each point has exactly
  one certain value).

- **Unbounded persistence.** The last value in a chain (no later distinct
  defeater) holds `[tᵢ, ∞)` — persists unbounded.

- **Backward persistence: none (v1 boundary).** An asserted value implies nothing
  before its `since`. Forward-only.

- **Koan 4 — nothing silently dropped.** A value on an inertial predicate with
  **no `since` anchor** cannot produce a persistence interval, so it raises
  `continuity.inertia-unanchored` (warn), naming the slot. The declaration is
  honored or it complains — never silently inert.

## A.2 Interactions (confirmed against the code)

- **`declare_functional`.** A `state`-asserted slot is already a fluent, so it is
  *excluded* from the FD checks and judged only by the fluent projection. Inertia
  refines that projection: on an inertial slot a later distinct value is a
  transition (silent) when its derived span is provably disjoint from the prior,
  and a `fluent-contradiction` only when they certainly co-hold. Declaring a
  predicate both functional and inertial is consistent (both say single-valued);
  the FD check keeps deferring to fluent reasoning.

- **Defeasible identity (`assert_same_as`).** Merged slots are canon-grouped into
  one scratch timeline; inertia's derived spans ride that grouping. Two inertial
  values merged from different surfaces that certainly co-hold ⇒
  `fluent-contradiction` whose support **names the identity assertion**.
  Defeating the merge un-persists correctly because the canon map is rebuilt from
  *certain* links under the current valuation — defeat is a valuation flip, so
  the next projection sees them un-merged.

- **Branches.** Derivation is branch-local because it is computed inside
  `dli_world::compile(reconstruct_at(branch_ref.commit))`; any pre-image read
  uses `head_ref.commit`, never `root_hash()`. Persistence derived in one branch
  is invisible to base and siblings.

## A.3 Architecture — a valuation-lazy projection, not span-clipping

**Inertia is derived at projection time, never baked into the stored span**
(DL-0081, plan B.0). A span's standing is read through `ann.status(valuation,
theory)` lazily at projection time, and the support algebra is positive (PosBool
DNF — no negation). A derived `until` clipped in at compile time could never be
*retracted* by defeating a defeater — positive support cannot express "re-extend
when the defeater dies" — and would break the fork-suppose and
differential-parity oracles that flip valuation over an already-compiled world.

So the open spans are stored exactly as authored (byte-unaffected), a declared
`inertial_predicates` map (predicate → the OR of its declaring provenance
`Ann`s) is carried alongside them, and the effective boundary is derived at each
projection from which competitors are live. `FluentTimeline::state_at_inertial`
is the projection the **continuity check** consults, and — since DL-0082,
contract `1.9.0` — the projection `check::slot_state_at` (`crates/dli-world/
src/check/continuity.rs`) also consults directly, dispatching the identical
`state_at_inertial`/`state_at` choice over the same canon-grouped scratch
timeline the check builds. **DL-0082 lifts the v1 read boundary this note
originally recorded here**: `author.as_of {workspace, pred, subject, at}` is
now the one inertia-aware read, answering with the value(s) live at a query
point — including a capped value's **derived** `until` and its certain
defeater — without waiting for a `continuity.fluent-*` finding to carry it. A
`dli.query` / `author.query` are UNCHANGED by DL-0082: they still return the
raw extensional value(s), never the derived interval, so the eval-path
non-leak invariant below holds for them exactly as it always did — `author.as_of`
is a new, separate, additive read, not a change to `query_with`'s own
projection. The v1 boundary DL-0082 leaves in its place is narrower: **no
`dli.*` mirror of this read over a materialized corpus without opening a
workspace** (trigger: a consumer needing the as-of read without an authoring
session). **Capping is active only while both the
inertia declaration and the competing value are certain**: a defeated or merely
proposed declaration — and a non-certain declaration never caps at all,
consistent with the standing model that a non-certain claim takes no certain
effect — reverts the projection byte-for-byte to plain `state_at`, so no value
ever silently vanishes. Every
defeat case then falls out of the ordinary standing read with **no recompile**:

- defeat the inertia declaration ⇒ projection reverts to plain `state_at` (the
  open spans clash exactly as before, the value never vanishes);
- defeat a defeater ⇒ it stops capping, the earlier value re-extends;
- defeat the originating assertion ⇒ its span drops, others stand.

**Hard invariant:** derived intervals never leak into the EVAL QUERY PATH. A
`dli.query` / `author.query` still returns the raw extensional value(s) —
`slot_state_at` never touches `dli_eval::world::World::query_with` or the
saturated derived layer, it reads `world.model.fluents` and
`inertial_predicates` directly, so a `(pred, subject)` with no fluent
timeline answers empty `holdings` rather than falling back to the
extensional fact. Since DL-0082 the derived boundary is no longer a
continuity-check concept ONLY — `author.as_of` is the one inertia-aware
read — but it is reachable through exactly that one op, never through the
eval query path. This keeps the three lowering paths agreeing on the open
spans plus the declared map, with no timeline synthesis forced onto the
evaluator.

## A.4 Wire surface

`author.declare_inertial { workspace, pred, provenance }` at contract **1.8.0**;
originally added at a version floor with earlier wires unchanged. Guarded for control-chars / killed-tokens and citation integrity
like every other declaration. The verdict is reachable wire-only:
`author.declare_inertial` → `author.add_state` (assert values at points) →
`author.check` (findings) → `author.materialize`.

`continuity.fluent-contradiction` and `continuity.fluent-order-undeclared`
carry, for an inertial slot, an enriched `Finding.data`: `"inertial": true` and
each value's **derived** `since`/`until` (the defeater's `since`, or `null` for
an unbounded value), keeping the order-undeclared `repair.candidates`. The data
is included in the current contract's structured findings.

```json
{
  "pred": "water-status",
  "subject": "river",
  "values": ["commons", "enclosed"],
  "inertial": true,
  "spans": [
    { "value": "commons", "since": "dawn", "until": "noon" },
    { "value": "enclosed", "since": "noon", "until": null }
  ],
  "witness_points": ["noon"]
}
```

## A.5 v1 scope boundaries (each with an un-lift trigger)

Following the temporal-identity / domain-rule precedent (a boundary recorded with
no trigger becomes permanent by default, DL-0011):

- **Backward persistence** — off. Trigger: a consumer requirement for
  retro-persistence with stated semantics.
- **Per-slot (per-subject) declaration** — off; per-predicate only. Trigger: a
  consumer needing one subject inertial and another not on the same predicate.
- **Inertia over *derived* (rule-produced) fluent values** — off; only *asserted*
  `state` values persist. Trigger: a consumer authoring rules that produce fluent
  values needing persistence.
- **Explicit persistence-defeater op** — not added; the later-distinct-value rule
  covers v1. Trigger: a consumer needing to end persistence without a successor
  value.
