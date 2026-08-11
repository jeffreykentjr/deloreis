---
status: live
title: "Semantics"
verdict: "status is never stored; support and grade are two evaluations of one annotation."
---

# Semantics

This document specifies what a `.dli` claim *means*: the annotation algebra that
carries provenance through every derivation, the two evaluations of that
annotation that yield a claim's standing and its strength, the honest places
where the algebra does not reach and something else carries the weight, and the
cost of computing all of it. It is the deductive core of the fabric; every other
spec document reasons over the objects defined here.

The organizing commitment, stated once and then earned: **no atom stores a
status.** Standing is *derived*, always — and it is derived twice, from one
annotation, along two orthogonal channels.

The decision ledger (`../decisions.md`) outranks this document.

---

## 1. The annotation structure

Every **atom** — an entity's existence, a relation, a fact — and every **rule**
carries an annotation:

```text
ann(a) = (P⁺, P⁻)
```

a pair of **absorptive positive polynomials in PosBool[X]** — the
conditional-table algebra, not full counting-provenance. `P⁺` is the atom's
**support**: the Boolean combination of authorial events under which the atom
holds. `P⁻` is its **defeater polynomial**: the combination under which a
default the atom rests on is overridden (§4).

`X` is a set of **event variables at ruling-clause grain**. One variable is
minted per:

- separable clause of a decision entry (so a partial supersession is
  expressible — one variable per clause, not one per decision),
- option of an open question,
- coherent proposal bundle,
- simulation-run export (origin: the run manifest),
- episode-acceptance.

Each variable carries metadata **outside the algebra** — the algebra sees only a
Boolean variable, never its label:

- **kind** — `ratified | provisional | option | bundle | sim | episode`,
- **origin** — who, when, fork lineage, run manifest,
- **scope** — the variable's affects-set (the attachment side condition of §3).

Three pieces of **global state** complete the picture:

- **ν** — a partial valuation, the ledger: which variables are ruled true, which
  false, which still open.
- **Θ** — a small constraint theory: option-exclusivity families (the options of
  one question cannot both be true), supersession links.
- **C** — the **negative-constraint store**: the guardrails, the content that
  must never be present (§3).

Per fork there is additionally a valuation extension, a set of fresh variables,
and an optional keep-stamp visibility restriction (§5).

**Rules are annotated atoms too.** A rule's own variable multiplies into every
derivation that passes through it. Three consequences fall out structurally,
with no special machinery:

- A **proposed** rule yields only *possible* consequences — a rules-as-proposed
  practice needs no policing, because its conclusions carry its own unratified
  variable and can never evaluate certain.
- **Fork-local rules** need nothing special: a rule tagged with a fork variable
  is symbolically present everywhere and evaluates dead outside the fork.
- The rule that anything resting on a proposed entity is itself merely possible
  (the legacy validator's **V9**) becomes a **theorem** — a derived relation
  multiplies in its endpoints' existence polynomials, so it cannot be more
  certain than the entities it connects.

**Why PosBool and not the full counting semiring N[X].** Every evaluation this
system performs is **idempotent** — layer membership, the ratified-over-
provisional grade, minimal-witness blame, world membership. Nothing counts
derivations; the one counting discipline in scope, linear debts, lives elsewhere
by design (§6). PosBool is exponentially smaller under absorption, and —
decisive for recursion — it is a **finite lattice over finitely many
variables**, so the closure predicates (`within`, `downstream`, `reachable`)
reach fixpoints with no continuity machinery. The one property worth keeping
from counting-provenance is kept: **evaluation commutes with specialization.**
The fixpoint is therefore computed **once, symbolically**; valuations — the
ledger, any fork, any what-if — are applied *afterward* as cheap homomorphisms.
Precomputed provenance, valuated on demand, is the load-bearing architecture,
not an optimization.

---

## 2. Status as evaluation

Under a valuation and constraint theory `(ν, Θ)`, each atom's support `P⁺`
yields a **three-valued standing**:

- **certain** (assertable): some monomial of `P⁺` is entirely ν-true — the atom
  is supported by resolved events alone. The all-open-goes-false completion is
  always admissible (an author may park or reject everything, forever), which
  pins certainty to *exactly* the assertion gate.
- **possible** (shadow): not certain, but some monomial is Θ-satisfiable — no
  ν-false variable, and no two options of one exclusivity family together.
- **dead**: every monomial is Θ-unsatisfiable. Split by `C`: **dead-by-rejection**
  (the killing event also wrote a C-constraint) versus **dead-by-supersession**
  (stranded, not anti-canon). The algebra alone cannot tell these apart; `C`
  does.

**Disjunctive certainty does not count.** A fact implied by *every* option of an
open question is still **unassertable** until the ruling — because the
all-open-goes-false completion, always admissible, makes it false. This is a
deliberate position: a claim is certain when resolved events *alone* force it,
not when it would follow under any resolution.

**Grade is a second evaluation on the same polynomial**, not a level in the
support lattice. The grade of a monomial is **stored at derivation time** as the weakest-link
of the deriving rule's grade and the body atoms' grades; the grade of an atom
under a valuation ν is the **join** over its ν-true monomials' stored grades.
Join-of-meets over a finite chain is monotone and polynomial-time (annotated
deduction). This is strictly stronger than a flat per-record status:
**provisionality propagates through derivation** — anything whose every live
support touches a provisional premise is itself provisional, which a stored
status field cannot express.

The two channels are orthogonal and must not be confused: **support** is what a
claim rests on; **grade** is how well the procedure that produced it was
verified. The grade channel's full lattice, its weakest-link rollup, and its
boundary against model evidence are specified in `07-verdicts-and-llm-boundary.md`
(two-channel semantics; DL-0006). This document owns support; grade rides the
same polynomial.

**Supersession without non-monotonicity.** Nothing is ever retracted. The
symbolic fixpoint stands; superseding a clause **revaluates a variable**, and
evaluation re-runs on unchanged polynomials. What lets this survive a real
ledger is the clause grain: partial supersessions ("supersedes D-0208, the
episode-arithmetic clause only") that one-variable-per-decision cannot express,
one-variable-per-clause expresses exactly. **Promotion after supersession is
extension, never rewrite:** the new clause's variable attaches as a *new*
monomial; the old monomials go dead and persist as lineage. History is additive.

---

## 3. The four honest residues, and fail-closed

The collapse of five stored statuses into support × grade is elegant, and it is
honest about the four places it does not reach. Each residue is carried by a
mechanism *outside* the polynomial, on purpose.

1. **Rejection of never-present content.** A banned figure, a banned in-world
   vocabulary word — there is no atom to make dead, because the content was never
   admitted. This lives in `C`, checked against the certain layer and against
   drafts, never part of the fixpoint.
2. **The shadow layer's must-not-contradict duty.** The obligation that possible
   content not contradict certain content is a property of the *evaluation
   regime* (§4), not a truth value an atom can carry.
3. **V11 as an attachment side condition.** Attaching a monomial containing
   variable `d` to atom `a` requires `a ∈ scope(d)`. The algebra cannot see
   *which* atom a polynomial decorates; the type system can, and enforces it at
   attachment. A bonus falls out: **derived atoms cannot be hand-annotated at
   all**, so forged promotion of derived lore is **impossible by construction**,
   not merely forbidden by rule.
4. **Defaults need `P⁻`.** A default with proposed exceptions is not expressible
   in support alone; the defeater polynomial carries it (§4).

**Fail-closed is a theorem, not a policy.** An annotation that fails to parse or
fails to attach yields `P⁺ = 0` — not certain, not even possible, excluded from
every layer except the error report. The context-pack builder's rule that a
malformed provenance marker excludes its claim from generation is therefore not
a guard bolted on; it is what `P⁺ = 0` *means*.

---

## 4. Negation, defaults, and the conflict duty

Raw negation-as-failure is banned; `proposed` is genuinely three-valued. The
regime is one store, one symbolic fixpoint, with **layers as evaluations, never
as sub-stores.**

A negative body literal `¬b` does **not** block derivation. It **poisons**: all
non-contradictory, theory-respecting monomials of `b`'s support `P⁺` enter the
conclusion's defeater polynomial `P⁻`. This is **valuation-neutral
all-admissible-monomials poison** (DL-0024): the fixpoint includes every
monomial that *could* become true under *some* valuation — never filtered by
the active `ν` — so the symbolic saturation is identical regardless of which
valuation is set at fixpoint time. Evaluation commutes with specialization: a
fork's `eval_under(ν)` reads the precomputed supports and defeaters without a
re-fixpoint and gets the same answer a fresh fixpoint under `ν` would give
(TQ-N3 property).

The consequence of poison for status:

- A **certain** `b` (some monomial of `b`'s `P⁺` is ν-true): its monomials
  enter `P⁻`, and — because they are themselves ν-certain — the conclusion is
  **present-but-Possible (defeated)**. The conclusion's support monomial is
  poisoned by a live defeater, so it cannot evaluate as Certain. No derivation
  is blocked; no row vanishes — the conclusion exists with degraded standing.
- A **merely-possible** `b` (no monomial ν-certain, but some ν-live): its open
  monomials likewise enter `P⁻`. Under the current valuation the defeater is
  not yet certain (so the conclusion stays Certain if its support is clean), but
  the risk is recorded — a future ruling that makes `b` certain will defeat it.
  The conclusion is **certain-at-risk**.
- A **dead** `b` (every monomial ν-killed): its monomials are still in `P⁻`
  syntactically but evaluate dead, so the poison is inert and the conclusion is
  unaffected. A clean support evaluates to Certain without obstacle.

An atom that is **certain but carries a Θ-live `P⁻`** is **certain-at-risk**:
assertable now, with the pending fork that could defeat it surfaced —
*"assertable; awaits q-00nn."* This single mechanism *is* three things at once:
regional defaults with proposed exceptions (the exception rides in `P⁻`), the
shadow layer's must-not-contradict duty (discharged as surfaced risk, not as a
block), and why-not provenance.

Worked, in the surface. A ward holds where its ground is seated and not breached:

```dli
# grammar: v0 (unfrozen; AST normative)
scope wards

seated(ground-oak)     ratified d-0146    # certain
breached(ground-oak)   proposed           # a shadow: possible, not certain

rule ward-holds  ratified d-0146
  ward-holds($g) :- seated($g), not breached($g)
```

`breached(ground-oak)` is merely proposed — its monomial is ν-live but not
ν-certain. The negation `not breached($g)` poisons: `breached`'s open monomial
enters `ward-holds`'s `P⁻`. Because the defeater is not yet certain,
`ward-holds(ground-oak)` evaluates as **Certain** (its support from `seated` is
ν-certain and the defeater does not yet fire). It is **certain-at-risk**:
assertable now, flagged as defeasible should `breached` ever be ruled true. Were
`breached` *certain* instead, its monomial in `P⁻` would be ν-certain too —
poisoning the support monomial — and `ward-holds` would evaluate as
**present-but-Possible** (defeated), not absent. The derivation still exists;
only its standing degrades.

**Slots are functional dependencies.** A slot (`entity.key`) admits one value;
the continuity checker already treats them so. A certain value `v` together with
a possible claim `v′ ≠ v` raises a **pending-conflict** for the author — nothing
is dropped, nothing is auto-resolved.

**Which predicates are functional, and what a break is called.** A predicate is
functional when the author declared it so (`functional <pred>`, spec/01 §3). For
a claim-relative check — a draft setting one slot two ways — a predicate canon
already witnesses as single-valued everywhere it appears also counts, and the
clash is `continuity.intra-episode-contradiction`. But the world-internal check —
asked of canon *itself*, `continuity.fd-violation` — fires **only** on a
**declared** functional predicate: canon's own shape cannot settle functionality.
A subject bearing several edges while others bear one each is a hub in a
many-to-many relation (a place with several neighbours) exactly as readily as a
functional-dependency break, so inferring functionality from degree manufactures
findings against lore that is merely unusual. The author rules on it (koan 3); the
declaration is what arms the class. This is the read-side counterpart to merge's
blame-pair check (§5), closing the asymmetry that let a load accept a doubly-set
slot a join refuses. Distinctness of values is judged on the **FD value key** —
case, hyphen/underscore, and whitespace folded, so `gold` and `Gold` are one
value — and on nothing else: other punctuation survives (`2.5` and `2-5` stay two
values) and stems are never folded (`gold` and `golden` remain two), because
merging genuinely distinct values would silently drop a contradiction (koan 4).
Fluent slots are excluded here and judged over their intervals instead
(`continuity.fluent-contradiction`, spec/03 §2), where a value held over disjoint
spans is no clash at all.

**Ratification pre-flight is a free query.** Before valuating an option, evaluate
the would-be valuation and verify that the certain layer stays FD-consistent and
C-consistent; report blame as the exact conflicting monomial pair. What would
otherwise be discovered *after* a decision propagates becomes a question asked
*before* it.

The continuity classes and the four fact-check verdicts are all recovered as
evaluations over this regime — contradiction is an FD-versus-certain clash or a
C-match with blame; asserting-the-merely-possible is asserting a possible-not-
certain atom; unknown is absence from certain ∪ possible. No new mechanisms are
introduced to check a draft; checking is querying.

---

## 5. Forks, merges, promotion, and the keep-stamp

A **fork** is a named condition context: a valuation extension, a set of fresh
variables, an optional visibility restriction, and local atoms and rules tagged
with fork variables. Creation is **O(1)** — no store is copied; only the local
additions run the incremental fixpoint.

The operations:

```dli
# grammar: v0 (unfrozen; AST normative)
fork arthur-drew-the-sword
  suppose q-0001 = arthur
  add fact drawn-by(excalibur, arthur)
```

- **suppose** — valuate an open variable within the fork.
- **add** — introduce a fork-local atom or rule.
- **suppress** — valuate a bundle false within the fork.
- **restrict** — apply a keep-stamp (below).

The interesting output of a fork is the **derived-consequence diff**: the set of
atoms whose `(support, grade)` changes between the base valuation and the fork —
newly-certain, newly-dead, grade-changed, newly-in-conflict. A fork is a lens on
consequence, not a copy of the world.

**Merge is a constraint-join.** Two forks conflict iff they valuate some variable
differently, *or* jointly violate Θ, *or* their joined certain layers break a
functional dependency or a C-constraint. The third kind is semantic and reports
its **blame pair** — the exact two monomials that cannot both hold.

**Promotion is an application decision recorded as data.** A fork does not
become active merely because it exists. The consuming application records the
decision state and supplies it with the affected scope; DLI verifies the
referenced decision and re-evaluates the valuation. The affected atom set can
therefore be generated, origins persist as lineage, and forged or unknown
citations fail closed. DLI does not decide which caller was entitled to make the
decision (DL-0054, DL-0058). A sim-born variable carries its run manifest, so a
promoted simulation finding renders its provenance in full.

**The keep-stamp is visibility, not valuation.** It is a thin second dimension —
`kept-present | kept-absent | removed` — applied *after* evaluation and composing
as a filter over the results. This is what keeps **licensed ignorance
mechanically distinct from falsity**: a fact hidden from a deck, an axiom kept
out of a proof, a character not permitted to know something are all *present and
true and filtered from a view*, never *false*. The distinction holds at every
level the censorship operates.

---

## 6. Linearity stays out: object versus meta

Two accounting systems live in the fabric, and conflating them would be a
category error.

**Debt conservation and linear resources govern the object language.** A debt
token spent twice inside an in-world proof is an in-world error, checked by the
resource discipline of `04-practices.md`. This is a fact about the *world*.

**Annotations govern the meta language.** A polynomial records *which authorial
events support a fact*. This is a fact about the *authoring*.

If the two were one system, an authorial what-if — supposing a variable to
explore a fork — could perturb an in-world proof's resource accounting, which is
nonsense. They are kept apart, touching at **exactly two points**:

1. The proof checker's **premise selection** reads the certain projection of the
   store (certain ∩ known-by-speaker ∩ present-in-scene).
2. A checker's verdict is **itself a derived atom** whose `P⁺` multiplies its
   premises' polynomials — so a proof's validity **degrades automatically** when
   a premise later loses support. A proof that rested on a since-superseded fact
   stops being certain the moment the fact does, with no re-checking of the proof
   itself.

Resource conservation remains a fold invariant plus the checker's resource-
multiset bookkeeping — an affine side condition, not a linear type theory
intruding into the surface language.

---

## 7. Cost

Evaluation is **annotated semi-naive** with **aggressive absorption at every
join**, which keeps supports narrow — monomial widths and alternative counts
stay small at corpus scale, because absorption discards dominated monomials as
they form. Absorption is the performance linchpin, not a cleanup pass.

The policy that keeps recursion tractable:

- **Recursive predicates recompute** — the certain evaluation is stored together
  with a coarsened possible-approximation, rather than a full symbolic form.
- **Non-recursive predicates store symbolically** — computed once, valuated on
  demand.
- **Exact-on-demand queries** — minimal-witness blame, ratification-survival —
  run over an **inverted variable-to-atom index**, so a question about one
  variable's reach does not walk the whole store.

The concrete numbers — a ledger on the order of hundreds of clauses, atoms on the
order of thousands to tens of thousands, monomial widths in the single digits, a
symbolic fixpoint in milliseconds against a CI gate measured in seconds — are an
**engineering envelope**, the scale the design is built to sit comfortably
inside, not a guarantee to be quoted as a benchmark. The claim they support is
qualitative: **the state tree is never walked.** Provenance is computed once;
worlds are valuations applied afterward; search enters only where a question
inverts the fixpoint, and those inversions are enumerable (see
`08-reasoning.md`).

---

## 8. What this buys, in five lines

- **Certain answers *are* the assertability gate** — fail-closed and the V9 rule
  are theorems, not guards.
- **Blame is polynomial support** — minimal-witness on demand, from the same
  object that gates assertion.
- **What-if is a valuation** — forks, supersession, ratification pre-flight, and
  episode cuts are all changes of ν over unchanged polynomials; nothing is
  retracted, so monotonicity is never at stake.
- **Citation is a rendering of the polynomial that gates assertion** — the
  citation shown to a reader and the support that admits the claim to canon are
  the same object, so they cannot drift apart.
- **Status is a view.** The legacy five statuses collapse into support × grade,
  with exactly the four honest residues of §3 carried outside the algebra —
  `C`, the conflict duty, V11 attachment, and the threaded episode fold
  (`03-time.md`, `06-craft-and-payloads.md`).
