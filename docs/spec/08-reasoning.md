---
status: live
title: "Reasoning: owned evaluation, borrowed proposers"
verdict: "evaluation and checking are owned; search and generation propose; nothing foreign certifies."
---

# Reasoning

De Lore Is answers a spread of reasoning tasks — assertability, continuity,
blame, timeline consistency, and ledger proof checking. This
document inventories all of them, gives each its computational shape, and makes
the own-versus-borrow call for each under one rule: **evaluation and checking
are reimplemented, small and certifiable; any search or external baselines
would act as proposers, never as
certifiers.** That rule is koan 12 (own what certifies; borrow only baselines)
and DL-0005 (owned-first reasoning) applied task by task.

The inventory distinguishes implemented work from deferred possibilities. It
does not promise that a future solver or service is included in the release.

## 1. The kernel is solver-class software

There is no external solver process in any gating lane, and the kernel is still
solver-class software: it is a datalog query and reasoning engine with a
nonstandard annotation algebra. That is not a hedge to be walked back — it is
the committed center of the design, and building it seriously entails a named
bill:

- **Load-time rule shape, stratification, and range restriction.** A bare
  top-level world rule must be valid Datalog; malformed or body-less forms are
  error findings, never empty lowerings. The negation regime (see
  `02-semantics.md`) also needs an acyclicity discipline on negative
  dependencies, exactly as stratified datalog does, and every rule must be
  range-restricted. All three are checked when a pack loads, before any
  evaluation runs. Domain checker rules and practice-internal
  `rule ... requires` forms remain in their owning lanes.
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
- **Differential testing.** Retained tests compare independent evaluation paths,
  including incremental and full saturation. The engine must stay small enough
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
checkable through its tests; an independent formal specification remains a
possible future aid (§7).

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
**MC** property-based and mutation testing.

| id | Task | Shape | Machinery | v1 | External |
|---|---|---|---|---|---|
| T1 | Assertability / status derivation | per-atom evaluation; ptime | EVAL | yes | no |
| T2 | Transitive closures (geography, `happens-before`) | recursive datalog, finite domain | EVAL | yes — the irreducible center | no; this row *is* the engine |
| T3 | Continuity checking (six classes) | non-recursive queries + FD checks over fold state | EVAL | yes | no |
| T4 | Factcheck verdicts | point lookups + FD + constraint scan (model extraction stays caller-side, never above `tested`) | EVAL | yes | no |
| T5 | Draft claims against canon | continuity and fact checks over draft claims | EVAL/CHECK | yes | no |
| T6 | Explain one claim | read its support paths and blockers | EVAL | yes | no |
| T7 | Ratification pre-flight | one candidate-valuation evaluation + conflict scan | EVAL | yes | no |
| T8 | Joint valuation search | search over event variables | ENUM or SAT | deferred | any proposal must be rechecked |
| T9 | Fork diff / merge conflicts | evaluation diff; constraint-join; FD/constraint scan | EVAL | yes | no |
| T10 | Blame / min-why / why-not | antichain read; defeater read | EVAL | yes | no |
| T11 | Keep-stamp visibility | deterministic scope filter over exported or forked facts | CHECK | yes | no |
| T12 | Timeline consistency | cycle detection over the before-graph + era spine | PROP | yes | no |
| T13 | State-at projection | cautious event-calculus projection over a partial order; three-valued | EVAL/ENUM | yes | no |
| T14 | Episode-order CHECK (~200 eps) | linear scans against precedence + window constraints | PROP (trivial) | yes | no |
| T15 | Episode-order SEARCH / optimize | topological + earliest-slot while constraints stay precedence + windows (poly); NP only with disjunctive/soft constraints | PROP → owned searcher | deferred until the workflow exists | CP-SAT as proposer, if ever |
| T16 | Wrong-hour / phase checking | modular arithmetic; wheel-walk consistency | CHECK | yes | no |
| T20 | Proof-ledger checking | proof CHECKING — linear, decidable, no search | CHECK | yes | no — the paradigm reimplement case |
| T21 | Metatheorems | interactive proving, offline | Lean | no, by design | spec organ; badged lemmas |
| T22 | The proof layer itself (gate coverage) | seeded-violation operators × real gates | MC/ENUM | harness exists | no |

## 4. Reasoning choices that need prose

Some rows deserve their tradeoffs spelled out.

**T6 / T8 — explanation and valuation search.** The engine can explain a
single claim in terms of its support and blockers. Searching for a joint repair
across claims or enumerating valuations is not a shipped capability. If a
future proposer supplies a valuation, the kernel must re-evaluate its result
before it is used as a judgment.

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

**The two rows that anchor "reimplement."** T2 (transitive closures) is the
irreducible center: recursive datalog over a finite domain *is* the engine, and
there is nothing to integrate because this row is the thing being built. T20
(proof-ledger checking) is the paradigm reimplement case at the other end:
proof checking is linear in proof length, decidable, and contains no search at
all — a small first-order checker with explicit instantiation and linear
resource accounting, whose verdicts certify canon and therefore must be owned,
deterministic, and replayable. Between these two poles every certifying task is
evaluation or checking, and every proposing task stays behind the judgment boundary.

**The synthesis.** The certification path — everything that gates canon
assertion, continuity, the export pack, the proof ledger — is deterministic
evaluation and checking: polynomial per evaluation, terminating by construction,
no search anywhere in that certification path.

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
- **SMT: unchanged.** Nothing in scope is SMT-shaped; the door is closed and
  unlocked. The trigger is a ratified law introducing real arithmetic, and the
  decision is made then.
- **Lean: unchanged.** Offline spec organ; metatheorems import as badged lemmas;
  never in the serving path (§7).
- **Neural verification (α,β-CROWN-class): not applicable**, said plainly.

**The cost, accepted in writing.** Owned reimplementations must themselves be
tested: property tests, the mutation-operator corpus extended with seeded
searcher and enumerator violations, and differential checks of their outputs. That is the price of checkability, paid deliberately — koan 12 commits
the kernel to this bill, and it extends the same bill to the satellites. Also
true and stated: an owned searcher is worse than CP-SAT in general. That is
irrelevant at our sizes, and the flip condition names exactly the regime where
it stops being irrelevant.

**What this buys in dependencies.** From the first release through the medium
term: **zero foreign reasoning processes** required to build, gate, or serve.
Optional external baselines are never required in the serving path.

**Small parts, linked.** The kernel and timeline module compose at the
**replayable-judgment boundary**: serialized inputs and outputs are rerunnable
and diffable. Components share one semantics while remaining independently
testable.

## 6. Flip conditions, two-stage and falsifiable

Each condition discharges in two steps, and nothing breaks at either step
because proposers of either provenance bolt onto the judgment boundary the
kernel already exposes:

1. The condition fires in the author's actual workflow.
2. The small owned system is built.
3. The owned system measurably fails on real workloads.
4. The named foreign proposer lands behind the same judgment boundary.

Per seat:

- **A demonstrated joint-valuation search need** → evaluate a bounded approach
  first; any proposed valuation must be rechecked (T8).
- **A demonstrated cross-claim repair need** → assess an owned repair search
  first; the current single-claim explanation does not implement it (T6).
- **A real propose-orderings workflow with soft constraints** → the owned
  searcher first, CP-SAT second (T15).
- **A ratified arithmetic law** → the SMT decision, made then.

## 7. The Lean posture

Lean is a possible future offline specification tool, never part of the serving
path. The
proof-ledger checker is a first-order object — atoms, explicit instantiation,
around nine forms, side conditions, and linear accounting — and a full CIC
embedding would supply dependent types the design never uses while lacking the
one thing it most needs, linearity. It is not the checker; an offline specification could be used to audit it.

Three triggers move it from deferred to built, any one sufficient: the judgment
forms freeze; the first metatheorem is wanted; checker correctness becomes
publication-load-bearing. When a trigger fires, the object is small — the nine
forms, the side conditions, and the conservation law specified in Lean — and a
**hull-style differential loop** stands up around it: the corpus of serialized
ledgers runs through both the Rust checker and the Lean spec's executable
elaboration, and their verdicts are diffed in CI. The serialized-judgment
requirement (§1) is what makes that loop nearly free.

Metatheorems could be proven in Lean against the spec. Their authority would
reach the runtime through coherence evidence, imported as **badged lemmas**
the verdict lattice marks honestly (koan 6; see
`07-verdicts-and-llm-boundary.md`). The implementation stays authoritative at
runtime; Lean is the spec organ; coherence is a differential loop, never an
embedding.

## 8. The line, restated

Every task in the inventory falls on one side of a single line. On the certifying
side — assertability, continuity, factcheck, blame, timeline consistency, ledger
proof checking — the machinery is deterministic evaluation and checking, owned,
small, and replayable, with no search. On the proposing side — valuation-space
queries at scale and episode-order optimization — machinery could be integrated,
out-of-process, behind the
judgment boundary, and its every output is re-checked or graded honestly before
it touches canon. Nothing foreign certifies; through the medium term, nothing
foreign is even required to build, gate, or serve. That is DL-0005 and koan 12,
discharged row by row.

## 9. Archive evaluation keeps the line

Whole-corpus saturation is the current implementation, not a semantic
requirement. At archive scale, a future service could use supplementary Magic
Sets to bound a snapshot-pinned provenance cluster, then an embedded peer could
run this same owned bottom-up evaluator over that cluster. The service performs
indexed I/O and binding propagation; it does not certify standing, choose a
consensus, or replace the evaluator with a remote solver.

The required oracle is stronger than a latency benchmark: for the same root,
typed goal, and valuation, peer evaluation over a valid returned cluster must
produce the same bindings, standing, and provenance as whole-corpus evaluation.
An incomplete or unbounded cluster is a failure, not an approximate answer.
The loopback-first peer/service work is deferred in `PLAN.md` as DIST-1 through
DIST-4; [scale and distribution](../design-notes/scale-and-distribution.md) defines the
boundary.
