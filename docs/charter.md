---
status: live
verdict: "The twelve-koan charter: four triads (truth, loss, form, growth), the tiebreak and budget, the tensions, and the trials that test the koans. The decision ledger outranks this file."
---

# The charter

Twelve koans in four triads. The triads govern what the language believes
(**truth**), what it refuses to lose (**loss**), what it looks like (**form**),
and what it is allowed to become (**growth**).

**The test a koan must pass.** A koan earns its place only if it decides novel
questions **identically in two designers' hands**. A principle that merely
describes the build is documentation wearing a koan's clothes; it is not a koan.
Each koan below is stated, given its meaning, shown what it settles, and — where
the koan is load-bearing on questions the build has not yet met — worked against
a genuinely novel decision. The worked decisions are the test, run.

The decision ledger (`decisions.md`) outranks this charter. Where they
disagree, the ledger wins.

---

## Triad I — truth

### 1. Claims come from somewhere.

**Meaning.** A claim's authority is its origin — authored by whom, ruled by
what, assumed under which question. Nothing is true by being written down, and
no origin can be manufactured after the fact.

**It settles.** There is no status field in the AST; a bare `ratified` cannot
parse; derived atoms cannot be hand-annotated; citations render from the same
polynomial that gates assertion.

**A novel decision it makes — the wiki migration.** When an old worldbuilding
wiki is imported, its ten thousand claims arrive `proposed` with origin
`imported-from(wiki, page, rev)`. An import records where claims came from and
cannot mint authority, however trusted the source, because "trusted" is a
valuation only a ruling can make. The same answer holds for claims an LLM
extracts from old drafts.

**Why here.** Provenance drift is the repo pathology, and accumulated
consequence is what the world's Wyrd already is; the language believes what the
canon believes.

### 2. Nothing changes but a ruling.

**Meaning.** The world changes only when the author rules; every other operation
— forks, sims, imports, repairs, tools — is exploration over an unchanged store.
Koan 2 bounds what change *is*; koan 3 bounds *who* may cause it.

**It settles.** There is no delete opcode; supersession keeps lineage; forks are
O(1); cutting an episode is a valuation flip plus a re-fold, not an erasure.

**A novel decision it makes — the rename request.** A contributor asks for
`deloreis rename glass-weald glaswold` as an in-place operation across the
corpus. The koan refuses the shape and dictates the correct one: a rename is a
**supersession** — the new name ruled in, the old name dying into lineage and
the killed-token registry, every claim's history intact. There is no edit that
changes the past quietly; there is only a ruling that changes the future
visibly.

**Why here.** The author's practice is already massively forked; the koan turns
that workflow into the semantics.

### 3. The machine never writes canon.

**Meaning.** The machine may propose, check, render, and explain; it may never
make a thing true. Truth is the author's act alone.

**It settles.** The auto-fill trial (the machine offering to complete a claim
does not assert it); the entire agent surface writes proposals and evidence,
never canon.

**Why here.** This is the constitution of the repo and the moral center of the
allegory: the honest deck — a tool for the mind, never instead of it.

---

## Triad II — loss

### 4. Nothing is silently dropped.

**Meaning.** No input is discarded without a record. Off-door input, unknown
keywords, unhandled cases — each surfaces as a graded finding, never a silent
loss.

**It settles.** Forward-compatibility against unknown-keyword tolerance: an
unrecognized construct is a diagnosed finding, not a swallowed line. A closed
door (koan 11) bounds what is *supported*; it never authorizes a silent drop.

**Why here.** The gate is total by design; what the language refuses to lose is
the first thing the loss triad protects.

### 5. An open question is a debt.

**Meaning.** An undecided fork is not a blank; it is an obligation the system
carries and surfaces until it is ruled or retired.

**It settles.** `question`/`option` declarations are tracked debts; a waiting
candidate is filed as a ruling request and surfaced for the author, never held
in memory.

**Why here.** This is the koan where the language's ethics and the world's Frith
are literally the same law: a debt named is a debt that cannot be quietly
forgotten.

### 6. A verdict never outranks its evidence.

**Meaning.** A conclusion is only as strong as the weakest procedure it rests
on. A grade is a claim about recheckability, and it cannot be laundered upward.

**It settles.** The verdict lattice (checked > tested > attested > asserted);
weakest-grade rollup over the procedure chain; replayable judgments. A
Monte-Carlo result never reports `checked`; a model attestation never reports
`tested`.

**Why here.** Grading honesty is what keeps the whole apparatus trustworthy;
this koan governs grading and rollup, and its growth-side cousin — reimplement
for checkability, borrow only baselines — lives in koan 12.

---

## Triad III — form

### 7. One way to write it.

**Meaning.** For each thing sayable, there is exactly one canonical way to say
it. No synonyms, no positional shorthands, no second syntax regime waiting in
the expert tier.

**It settles.** The closed keyword vocabulary; the single canonical rendering,
formatter-defined and gate-enforced; non-canonical text legal as input, never as
repo state.

**Why here.** Koan 7 governs the text; its door-counting cousin, koan 11,
governs the paths in. Seven keeps the writing predictable; eleven keeps the ways
in few.

### 8. The wheel refuses rank.

**Meaning.** Some sorts are cyclic, and a cycle has no first and no largest. The
type system honors that: a wheel sort carries succ, pred, across, and distance,
but no `<`, no sort order, and no maximum.

**It settles.** The single `wheel` sort shared by cyclic time (tide, moon,
season) and the Turning's twenty-nine leaves; asking for the "greatest" season
or sorting the moon's nights is a type error, not a silent tie-break.

**Why here.** The world has wheels — the same turning that seats a ward runs the
Turning's leaves — and a language that faked a linear order over them would be
lying about the world it models.

### 9. Errors are lore.

**Meaning.** A diagnostic is part of the world's record, spoken in the world's
terms. Errors are first-class output, authored with the same care as any other
surface.

**It settles.** Diagnostics speak the domain, never PL-speak — a missing ruling,
a ward made at the wrong hour, a season with no first — and each is a designed,
snapshot-tested artifact.

**Why here.** The error ladder is most of what a writer meets day to day; if the
errors are noise, the language is hostile, and the loss triad has failed at the
surface.

---

## Triad IV — growth

Truth, loss, and form govern what the language *is*; growth governs what it is
allowed to *become* — which doors open, which components are owned, which
targets are served.

### 10. One fabric.

**Meaning.** All world knowledge — geography, magic, names, story-time, games —
lives in one runtime under one semantics. No satellite lore systems; integration
by declaration, not by API. If a thing is knowledge about the world, it is
declarations in the fabric; if it is not, it consumes exports.

**It settles.** The Turning engine is in-kernel because its rules are ratified
lore whose invariants are checked claims, while a video game is outside because a
runtime is not knowledge — one line, drawn once, by principle instead of
per-case argument; the timeline is a module of the fabric, not a service; the
naming oracle's ratified-names view becomes generated output of the compiled
world, not a hand-maintained sibling authority.

**A novel decision it makes — the export variant.** When a card export earns
users, someone will want variants maintained in the profile — "just add the
festival deck in the JSON." Refused, with the correct path attached: variants
are lore (`variant` declarations with status and citations); profiles only
render. The moment a profile defines content there are two fabrics, and one of
them is unratified.

**Why here.** The repo has lived the alternative — hand-maintained mirrors that
drift, prose shadowing data, an apparatus built to chase divergence. One fabric
is that lesson as law: divergence is prevented by having nothing to diverge.

### 11. One door per author.

**Meaning.** Each author population gets exactly one supported way into the
fabric; benchmarks exist to close doors, not open them. Population, not tool — a
nicer keyboard is not a new door.

**It settles.** Humans write surface (Door H); agents author through **Door T**,
the semantic tools against the server; textual agent editing is a measured
fallback, not a door; grammar-constrained local emission is a benchmark
condition, not a door; and **raw AST emission is a mechanism inside Door T's
tools, not a door**.

**A novel decision it makes — the form-editor question.** The factory UI will
someday want click-to-edit forms over lore entities. A new door? No: the UI
serves the human population, so it is Door H with a different keyboard — its
output is surface text, reviewed as surface, like editor snippets; it may not
emit past review as a second door for a population that already has one.

**Why here.** The sibling projects paid for it — path count predicted failure
surface better than path quality.

### 12. Own what certifies; borrow only baselines.

**Meaning.** Anything whose verdict gates canon is small, owned, deterministic,
and replayable; foreign components may propose candidates or serve as outrun
baselines, never certify. Growth proceeds by small owned parts linked at the
judgment boundary, and every owned part inherits the testing bill — property
tests, differential oracles — as the price of checkability.

**It settles.** The vacated CP-SAT and clingo seats and the two-stage flip
conditions; clingo's retained role as a lab-side differential oracle; zero
foreign reasoning processes through the medium term.

**A novel decision it makes — the good-crate question.** A mature Rust datalog
crate appears that could replace the evaluator at half the code. Adopt? No for
the certifying evaluator — a foreign fixpoint cannot certify, and forking it to
host the annotation algebra yields ownership in the worst form: owned bugs,
borrowed architecture. Yes as a **baseline** — run it as a differential oracle
over the shared datalog fragment. One paragraph decides it, which is what having
the koan is for.

**Why here.** The demonstrated ecosystem ethic of the sibling projects — a
verifier posture, a differential pattern — stated as law rather than habit.

---

## The tiebreak, and the budget

Every mechanism argues from a demonstrated lore requirement; the tripwires are
contractual. When koans genuinely deadlock, the order is **truth outranks loss
outranks form outranks growth** — form exists to prevent loss, loss-prevention
exists to protect authorship, and growth exists to keep all three enforceable
over time, never to enlarge the system for its own sake. Within a deadlock, the
resolution preserving the author's sole authority wins first; at equal
authority, the resolution that **adds less** wins — less language, and
explicitly fewer doors, fewer seats, fewer parts.

The budget has named lines:

- **Door count** — two.
- **Foreign certifiers** — zero, with one rented baseline.
- **Export targets** — earn maintained profiles only from demonstrated
  consumers.

## Principles in tension

The charter names its costs; a charter that names none has not been used.

- **Single-form versus sugar.** One canonical way to write it (koan 7) versus
  the ergonomics a writer would gain from shorthands. Resolved toward the single
  form: the formatter absorbs sloppiness on save, so the writer's convenience is
  paid at input time, not by a second surface.
- **The budget versus the timeline machinery.** The size budget pushes features
  out; first-class story-time pulls a partial-order module, fluents, and wheels
  in. Resolved by demonstrated requirement: each timeline feature answers a
  named corpus failure, and nothing speculative rides along.
- **Strictness versus the sketching writer.** A total gate versus a writer who
  wants to jot incomplete notes. Resolved by `proposed`-first authoring:
  incompleteness is a first-class state (proposed, open question) rather than an
  error, so sketching is legal and never silently lost.
- **Domain errors versus machine output.** Errors that speak the domain (koan 9)
  versus errors that a machine emitter or a benchmark can parse. Resolved by the
  two-channel error model: findings carry a stable machine-readable class *and* a
  domain-spoken message; neither is sacrificed to the other.
- **One fabric versus a small system.** One fabric pulls all knowledge into one
  runtime; the budget pushes everything out. Resolved by scope: one fabric
  governs *semantics* — all knowledge, one meaning, one store — not code shape.
  The kernel, the timeline module, the Turning engine, and any future searcher
  are separate small parts sharing the judgment boundary (koan 12). A monolith is
  not what one fabric means; satellite *semantics* is not what small parts means.
- **One door per author versus nothing silently dropped.** Closing doors must not
  create silent drops. Input arriving off-door — an agent's textual edit — is
  neither refused nor ignored; it meets the same full pipeline and graded errors
  as everything else. Doors bound what is *supported* (taught, tooled, measured);
  the gate remains total. A closed door is a support decision, never a data-loss
  decision.

## The trials

Three worked decisions test the charter end to end.

1. **Bare `ratified` cannot parse.** A status with no origin is a claim from
   nowhere; koan 1 refuses it at the grammar. The AST has no status slot to
   fill, so the refusal is structural, not a lint.
2. **A proposed conflict queues as a debt.** Two proposed facts contradict.
   Nothing is dropped (koan 4), nothing changes without a ruling (koan 2), so the
   conflict is filed as an open question (koan 5) and surfaced, awaiting the
   author.
3. **The write-back refusal.** A studio consuming the video-game export asks for
   `deloreis export --write-back`, so their tool can push edited entity fields
   home. Koan 10: integration is by declaration — an external tool's database is
   not the fabric. Koan 1: "the tool's current field value" is not an origin with
   authority. Koan 3: the machine never writes canon. Koan 11: if the studio's
   writers need in, they are humans — Door H, their edits entering as `proposed`
   declarations citing their session, awaiting ruling like every claim from
   outside the sovereign's hand. Four koans, one paragraph, no committee: the
   flag is refused and the honest path is offered.

## A parked koan candidate

Noted for a future charter revision, not adopted here: *"The model's word is
evidence about the work, never support for the world."* It states the boundary
that keeps attestations out of the support polynomials (`decisions.md` DL-0006).
The charter holds twelve koans; this candidate waits for its demonstrated need,
per the budget.
