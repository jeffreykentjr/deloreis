---
status: live
title: "Reasoning: owned evaluation, borrowed proposers"
verdict: "evaluation and checking are owned; search and generation propose; nothing foreign certifies."
---

# Reasoning

De Lore Is answers a spread of reasoning tasks — assertability, continuity,
blame, timeline consistency, ledger proof checking, ruleset exploration. This
document inventories all of them, gives each its computational shape, and makes
the own-versus-borrow call for each under one rule: **evaluation and checking
are reimplemented, small and certifiable; search, optimization, and generative
enumeration are integrated out-of-process, as proposers and baselines, never as
certifiers.** That rule is koan 12 (own what certifies; borrow only baselines)
and DL-0005 (owned-first reasoning) applied task by task.

## 1. The kernel is solver-class software

There is no external solver process in any gating lane, and the kernel is still
solver-class software: it is a datalog query and reasoning engine with a
nonstandard annotation algebra. That is not a hedge to be walked back — it is
the committed center of the design, and building it seriously entails a named
bill:

- **Load-time stratification and range restriction.** The negation regime (see
  `02-semantics.md`) needs an acyclicity discipline on negative dependencies,
  exactly as stratified datalog does, and every rule must be range-restricted.
  Both are checked when a pack loads, before any evaluation runs.
- **Annotation-aware seminaive evaluation.** The delta semantics is not the
  textbook one: an atom re-enters the worklist when its support *strictly grows
  in the lattice*, not only when it is first derived. Termination is guaranteed
  by the finite-lattice ascending-chain condition on the annotation algebra.
- **Absorption normalization at every join.** Reducing each support polynomial
  to its antichain of minimal monomials at the moment it is formed is the
  performance linchpin, not an optional optimization. Without it, supports grow
  without bound; with it, they stay narrow.
- **Inverted variable→atom provenance indexes.** So that a valuation change
  touches only the atoms whose support mentions the changed event variable.
- **Serialized, replayable derivation forests.** Every judgment is a byte-stable
  artifact: same input, byte-identical judgment. This is the audit surface, the
  debugging story, and the differential-testing hookup at once.
- **The differential-testing story.** The kernel's own verdicts are checked
  against a reference: the legacy gates as reference oracle during migration
  (see `design-notes/sibling-lessons.md` and `integration/migration.md`), a
  Lean-specified judgment fragment later (§7). The engine must stay small enough
  to be *itself* checkable.

Anyone who has built a datalog engine will recognize this list. Planning it as
"a library" rather than "an engine" is how such projects go wrong; it is
planned as an engine.

**Why bespoke, and not a shelf datalog crate.** The engine is reimplemented not
out of not-invented-here but because no off-the-shelf datalog hosts the
condition-polynomial algebra, the grade lattice, and the keep-stamp visibility
filter without a fork — and a fork yields ownership in its worst form: owned
bugs, borrowed architecture (koan 12's good-crate decision, worked in the
charter). The governing rule is the beacon principle: **reimplement for
checkability wherever a verdict certifies canon; integrate wherever a component
proposes, enumerates at scale, or serves as a baseline.** A mature datalog crate
is welcome as a differential *baseline* over the shared fragment; it may not
*be* the certifying evaluator. The engine must stay small enough to be itself
checkable — differentially against the legacy gates now, against a
Lean-specified judgment fragment later (§7).

## 2. The state tree is never walked

The load-bearing architectural claim answers the obvious worry — sophisticated
reasoning over possible state trees. The possible-worlds space is exponential in
the number of open event variables, and it is **never enumerated**. It is
represented once, symbolically, in the condition polynomials; a world is a
valuation applied as a cheap evaluation *after* the fixpoint. The symbolic
representation is computed once; valuations come after.

This is the conditional-table / certain-answer semantics of the
incomplete-database literature, specialized to a corpus whose "uncertainty" is
*pending authorial decision* — which is exactly what makes it the right tool:
the variables are things the author rules on, and their number stays small
because rulings are few relative to facts. A possible world is a partial
valuation; ratification is a valuation event; blame is the polynomial support;
citations are a rendering of the same polynomial that gates assertion. None of
that requires enumerating worlds.

Search machinery is needed only where a question inverts that direction — *find
me* a world, an ordering, a repair, rather than *evaluate* this one — and those
inversions are enumerable and bounded at the corpus's scale. The inventory below
walks every reasoning task and shows that the certification path contains no
search at all.

## 3. The task inventory

Machinery legend: **EVAL** annotated seminaive fixpoint / lattice evaluation ·
**ENUM** finite exhaustive enumeration · **PROP** constraint propagation
(partial order / point algebra) · **SAT** bounded model search · **ASP**
answer-set-shaped enumeration (choice rules + constraints + defaults) · **SMT**
arithmetic theory solving · **CHECK** proof checking, linear in proof length ·
**MC** Monte Carlo + property testing · **MCTS** game-tree search.

| id | Task | Shape | Machinery | v1 | External |
|---|---|---|---|---|---|
| T1 | Assertability / status derivation | per-atom evaluation; ptime | EVAL | yes | no |
| T2 | Transitive closures (geography, `happens-before`) | recursive datalog, finite domain | EVAL | yes — the irreducible center | no; this row *is* the engine |
| T3 | Continuity checking (six classes) | non-recursive queries + FD checks over fold state | EVAL | yes | no |
| T4 | Factcheck verdicts | point lookups + FD + constraint scan (model extraction stays factory-side, never above `tested`) | EVAL | yes | no |
| T5 | Outline-vs-world soundness (check direction) | T3/T4 lifted to outline claims + promise bookkeeping | EVAL/CHECK | yes | no |
| T6 | Abduction of minimal fixes | single claim: read the support's minimal monomials — precomputed; cross-claim repair: hitting set, trivial at instance sizes | ENUM | yes | ASP power tool iff repair spaces reach hundreds of interacting hypotheses |
| T7 | Ratification pre-flight | one candidate-valuation evaluation + conflict scan | EVAL | yes | no |
| T8 | Joint valuation-space queries | bounded model search over event variables | ENUM→SAT | exact to ~20 free variables | SAT as proposer beyond; kernel re-evaluates any found valuation |
| T9 | Fork diff / merge conflicts | evaluation diff; constraint-join; FD/constraint scan | EVAL | yes | no |
| T10 | Blame / min-why / why-not | antichain read; defeater read | EVAL | yes | no |
| T11 | Keep-stamp unreachability | derivation-graph reachability + exhaustive spread enumeration | ENUM | yes | clingo as differential oracle |
| T12 | Timeline consistency | cycle detection over the before-graph + era spine | PROP | yes | no |
| T13 | State-at projection / anachronism | cautious event-calculus projection over a partial order; three-valued | EVAL/ENUM | yes | no |
| T14 | Episode-order CHECK (~200 eps) | linear scans against precedence + window constraints | PROP (trivial) | yes | no |
| T15 | Episode-order SEARCH / optimize | topological + earliest-slot while constraints stay precedence + windows (poly); NP only with disjunctive/soft constraints | PROP → owned searcher | deferred until the workflow exists | CP-SAT as proposer, if ever |
| T16 | Wrong-hour / phase checking | modular arithmetic; wheel-walk consistency | CHECK | yes | no |
| T17 | Turning legal moves; deck invariants under deltas | successor + finite invariant enumeration | ENUM | yes | no |
| T18 | Ruleset-space exploration | legality: sweep now, ASP-shaped iff generative; metrics: simulation | ENUM/ASP + MC | sweep + battery: yes | clingo when enumeration goes generative |
| T19 | Rank-emergence / thesis-alignment metrics | statistical estimation over simulated play | MC (+ MCTS) | battery: yes | ISMCTS lab-side; results `tested` forever |
| T20 | Proof-ledger checking | proof CHECKING — linear, decidable, no search | CHECK | yes | no — the paradigm reimplement case |
| T21 | Metatheorems | interactive proving, offline | Lean | no, by design | spec organ; badged lemmas |
| T22 | The proof layer itself (gate coverage) | seeded-violation operators × real gates | MC/ENUM | harness exists | no |

## 4. The three rows that need prose

Three rows carry the whole argument and deserve their honesty spelled out.

**T6 / T8 — abduction and valuation search.** The single-claim case is the
quiet triumph of the annotation design. "What would make this assertable?" is
not a search problem, because the answer *is* the atom's support, already in
minimal-monomial form: the provenance representation is a completed abduction.
The combinatorial cases are real but bounded. Minimal repair across many failing
claims is hitting-set, and joint valuation queries ("which combination of open
rulings keeps Act 1 sound?") are propositional search over however many *free*
variables the question leaves open. At corpus scale — a few hundred ledger
entries, exclusivity families of single-digit size — exhaustive enumeration is
exact and instant. The honest tripwire is roughly twenty free variables in one
query, past which a SAT library earns proposer status. What never changes: a
found valuation is *certified* by the kernel evaluating it — one pass, T1/T7 —
so the solver is structurally incapable of being load-bearing even when present.

**T14 / T15 — episode ordering, the one genuinely scheduling-shaped task.**
Roughly two hundred episodes against reveal-before-payoff rules, staging
constraints, an early-episode floor, and story-time monotonicity outside
declared flashbacks. Three regimes, analyzed honestly:

- *Checking* a given order is linear scans, and that is what the author's actual
  workflow needs almost always — because an ordering is a **creative artifact**.
  A solver-produced sequence has no narrative value; the machine's job is to
  check authored orders and explain violations, not to author them.
- *The useful middle regime is slack computation*: earliest and latest legal
  slot per episode under precedence and windows — a forward and backward pass,
  polynomial — surfaced as "this reveal can sit anywhere in episodes 8–14."
  That serves authoring better than any single "valid ordering" would, and it is
  committed alongside the checker.
- *Search* ("find me a valid ordering") stays polynomial as long as constraints
  remain precedence and windows. It goes NP only when disjunctive constraints,
  cardinality or pacing constraints, or soft optimization arrive. Building search
  machinery for a workflow that does not yet exist is the trap; the seat is
  vacated in favor of an owned searcher, and CP-SAT is named as the eventual
  proposer only if that owned searcher measurably fails (§5, §6).

**T18 — ruleset-space exploration.** Split it and it stops being mysterious. The
*metrics* half — play each variant, measure rank-emergence, thesis alignment,
honest-versus-rigged divergence — is Monte Carlo plus property testing:
statistical by nature, `tested` grade forever, embarrassingly parallel, with no
logical structure for a solver to exploit. ISMCTS enters only if competent-play
studies are wanted, as a lab-side player, never a verdict. The *legality* half
depends on who writes the variants. Today the deltas are author-designed and
few: a parameter sweep with the kernel's exact invariant checker covers it. If
the space is ever opened generatively — "enumerate all variant decks over these
substitution schemas satisfying invariants I1..In" — that is ASP-shaped in the
textbook sense, and the seat is handled by an owned enumerator pruned by the
exact checker (§5), with clingo retained only as a differential oracle.

**The two rows that anchor "reimplement."** T2 (transitive closures) is the
irreducible center: recursive datalog over a finite domain *is* the engine, and
there is nothing to integrate because this row is the thing being built. T20
(proof-ledger checking) is the paradigm reimplement case at the other end:
proof checking is linear in proof length, decidable, and contains no search at
all — a small first-order checker with explicit instantiation and linear
resource accounting, whose verdicts certify canon and therefore must be owned,
deterministic, and replayable. Between these two poles every certifying task is
evaluation or checking, and every proposing task is search or simulation held
behind the judgment boundary.

**The synthesis.** The certification path — everything that gates canon
assertion, continuity, the export pack, the proof ledger — is deterministic
evaluation and checking: polynomial per evaluation, terminating by construction,
no search anywhere. That is the crisp line under all twenty-two rows.

## 5. Owned-first: the seats, one by one

DL-0005 sets the default at every flip condition: **the first preference is a
small owned system sized to our instances; a foreign backend enters only when
the owned system measurably fails on real workloads — and then still as a
proposer, never a certifier.**

- **The CP-SAT seat is vacated in favor of an owned searcher.** If
  episode-ordering-as-search materializes (T15's disjunctive/soft regime), the
  first build is a small propagation-and-backtracking searcher over precedence,
  window, and disjunctive constraints: topological ordering with chronological
  backtracking and forward-checking on windows, on the order of one to two
  thousand lines. The instances are tiny and known. What the owned searcher
  deliberately lacks — clause learning, restarts, portfolio heuristics — is
  exactly what these instance sizes cannot need. Two properties make owning it
  safe: search output is self-certifying in the check direction (every found
  order re-passes the T14 linear checker), and the useful middle regime — slack
  computation — is already committed polynomial machinery that needed no solver.
- **The clingo generative seat is vacated; the oracle seat is retained.** If
  T18's legality half goes generative, the first build is a small owned
  choice-point enumerator over delta schemas: backtracking over substitution
  choice points, pruned at each node by the kernel's exact invariant checker —
  which already exists and is the hard half. Enumeration over a schema-defined
  space is the easy half; hand-rolling *propagation* is the thing to avoid, and
  pruning-by-checker avoids it. **clingo is retained in exactly one role: the
  standing differential oracle** — the same constraints encoded independently in
  ASP, enumerations diffed in lab runs, covering T11's spread space, T17's deck
  invariants, and the owned enumerator's own output. It is lab-side, never in a
  gating lane (DL-0013), never in the serving path. A baseline is cheap to rent
  precisely because nothing depends on it.
- **SMT: unchanged.** Nothing in scope is SMT-shaped; the door is closed and
  unlocked. The trigger is a ratified law introducing real arithmetic, and the
  decision is made then.
- **Lean: unchanged.** Offline spec organ; metatheorems import as badged lemmas;
  never in the serving path (§7).
- **Neural verification (α,β-CROWN-class): not applicable**, said plainly.

**The cost, accepted in writing.** Owned reimplementations must themselves be
tested: property tests, the mutation-operator corpus extended with seeded
searcher and enumerator violations, and the differential oracle diffing their
outputs. That is the price of checkability, paid deliberately — koan 12 commits
the kernel to this bill, and it extends the same bill to the satellites. Also
true and stated: an owned searcher is worse than CP-SAT in general. That is
irrelevant at our sizes, and the flip condition names exactly the regime where
it stops being irrelevant.

**What this buys in dependencies.** From the first release through the medium
term: **zero foreign reasoning processes** required to build, gate, or serve.
One — clingo — is optionally present lab-side as a baseline, never in
CI-required jobs, never in the serving path.

**Small parts, linked.** The kernel, the timeline module, the Turning engine,
and the searcher and enumerator when they earn existence are separate small
owned components composing at the **replayable-judgment boundary**: serialized
inputs and outputs, re-runnable, diffable. Composition by data, not by
embedding — which is also how one fabric (koan 10) stays small: one semantics,
several small parts.

## 6. Flip conditions, two-stage and falsifiable

Each condition discharges in two steps, and nothing breaks at either step
because proposers of either provenance bolt onto the judgment boundary the
kernel already exposes:

1. The condition fires in the author's actual workflow.
2. The small owned system is built.
3. The owned system measurably fails on real workloads.
4. The named foreign proposer lands behind the same judgment boundary.

Per seat:

- **Joint-valuation queries beyond ~20 free variables** → owned bounded
  enumeration first, a SAT library second (T8).
- **Repair spaces of hundreds of interacting hypotheses** → owned hitting-set
  enumeration first, ASP second (T6).
- **A real propose-orderings workflow with soft constraints** → the owned
  searcher first, CP-SAT second (T15).
- **Generative ruleset enumeration** → the owned choice-point enumerator first,
  clingo second (T18).
- **A ratified arithmetic law** → the SMT decision, made then.

## 7. The Lean posture

Lean is an offline spec organ, adopted later, never in the serving path. The
proof-ledger checker is a first-order object — atoms, explicit instantiation,
around nine forms, side conditions, and linear accounting — and a full CIC
embedding would supply dependent types the design never uses while lacking the
one thing it most needs, linearity. So Lean is not the checker; it is the spec
the checker is audited against.

Three triggers move it from deferred to built, any one sufficient: the judgment
forms freeze; the first metatheorem is wanted; checker correctness becomes
publication-load-bearing. When a trigger fires, the object is small — the nine
forms, the side conditions, and the conservation law specified in Lean — and a
**hull-style differential loop** stands up around it: the corpus of serialized
ledgers runs through both the Rust checker and the Lean spec's executable
elaboration, and their verdicts are diffed in CI. The serialized-judgment
requirement (§1) is what makes that loop nearly free.

Metatheorems — for example, "no legal sequence of plays increases total debt" as
a *theorem* rather than a Monte-Carlo observation — are proven in Lean against
the spec. Their authority reaches the runtime through the coherence evidence,
imported as **badged lemmas** the verdict lattice marks honestly (koan 6; see
`07-verdicts-and-llm-boundary.md`). The implementation stays authoritative at
runtime; Lean is the spec organ; coherence is a differential loop, never an
embedding.

## 8. The line, restated

Every task in the inventory falls on one side of a single line. On the certifying
side — assertability, continuity, factcheck, blame, timeline consistency, ledger
proof checking — the machinery is deterministic evaluation and checking, owned,
small, and replayable, with no search. On the proposing side — valuation-space
queries at scale, episode-order optimization, generative ruleset enumeration,
play metrics — the machinery may be integrated, out-of-process, behind the
judgment boundary, and its every output is re-checked or graded honestly before
it touches canon. Nothing foreign certifies; through the medium term, nothing
foreign is even required to build, gate, or serve. That is DL-0005 and koan 12,
discharged row by row.
