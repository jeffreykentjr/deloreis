---
status: live
verdict: "A magic system is a practice with laws; the checker is generic, the systems are data."
title: "Practices: magic systems, games, and any declared law-set"
---

# Practices

A **practice** is a declared system of laws — a magic system, a divination
game, a rhetorical proof discipline, a craft with rules — expressed entirely
as data in the fabric. The kernel supplies one generic checker and one generic
evaluator; a practice supplies the sorts it reasons over, the laws as annotated
rules, the resources it spends, the side conditions its castings must satisfy,
the ways it fails, and the currencies those failures and successes cost. From
that declaration the kernel derives a checker for castings, proofs, readings,
and workings in that practice — with citations, expected verdicts, and honesty
grades, exactly as for any other lore.

Three of the seven programs named in [00-overview](00-overview.md) — the
magic-system workbench, the ledger checker, and the practice simulator — are
one machine pointed at different declared systems. The workbench authors a
practice; the ledger checker runs a single casting against it; the simulator
runs batteries. None of the three knows anything about a particular practice
that the practice's own declaration does not tell it.

## 1. The claim, precisely

**A magic system is a practice with laws, and the laws are data.** The
Concordance — the Spoken Proof that opens the series — is not a component of
the kernel. It is the *spoken-proof instance* of a generic practice, one
`.dli` pack among others. Nothing about grounds, binders, wards, debt, or the
wrong-hour failure is written into any Rust type or evaluator branch. Point the
same machinery at a wholly unrelated art — true-naming, a card game, a smith's
temper-craft — and it runs with zero kernel change, because everything that
distinguishes one practice from another lives in the practice's declaration.

This is [koan 10 — one fabric](../charter.md) enforced at the level of magic:
a practice is ordinary lore, so the whole existing apparatus applies to it for
free. `variant` deltas give a practice regional forms; a fork may suppose a
candidate change to a law and report which ratified castings break; a proposed
practice is structurally unassertable until ruled, like any lore. There is no
per-system engine, and there is no seam through which one could enter: a
second magic system that needed kernel support would be a bug in the
parameterization, not a feature request.

## 2. What is generic

The following exist once in the kernel and cost zero lines per practice. They
are the closed vocabulary a practice declaration draws on; a practice that
needs something outside this vocabulary is telling the workbench that the
parameterization is incomplete, which is a finding worth having.

**Typed finite sorts, including the cyclic `wheel`.** A practice declares its
own sorts — grounds, binders, roles — as ordinary finite sorts
([01-syntax](01-syntax.md)). The cyclic `wheel` sort ([03-time](03-time.md),
[koan 8](../charter.md)) is available on the same footing: tide, moon, and
season are wheels, and so is the Turning's ring of twenty-nine leaves. A wheel
carries `succ`, `pred`, `across`, and `distance` and refuses `<`, sort order,
and maximum — the same rank-refusing sort the timing checker and the game
engine share.

**Annotated facts and rules.** A practice's laws are annotated rules
([02-semantics](02-semantics.md)): each rule carries a citation-and-hypothesis
annotation polynomial, a derived status, and behaves under forks as a
valuation. Proposed rules yield merely-possible consequences; a fork-local
rule is free; the invariant that derived atoms cannot be hand-annotated (V9)
is a theorem of the algebra, not a check bolted on. A magic law is a rule; a
casting's conclusion is a derived atom.

**The closed resource algebra.** A practice spends resources drawn from a
fixed, small algebra — nothing else is a resource:

- `linear` — spent exactly once; the ledger must account for every unit.
- `linear conserved` — additionally, no ledger step may mint or burn the
  resource; a casting may reveal, assign, redistribute, or release it, but the
  multiset balances across every close or the ledger does not check.
- `affine bounded <cap>` — spent at most up to a declared capacity; overspend
  is a failure, underspend is legal.
- `graded monotone <order>` — an ordered grade that only ever moves downward
  along its declared order; nothing is spent, but the grade wears.

**Side conditions over the scene projection.** A condition is a predicate over
the world as projected into a scene: fluents that hold there, wheel phases in
force, cast presence, consent facts, scope containment. Conditions are how a
practice reaches into story-time and world-state without special-casing
either — the timing of a casting, the standing of a witness, the boundary a
claim may not cross are all conditions over the same projection.

**Error classes as refinements of the seven structural classes.** The checker
has exactly seven structural error classes, and every diegetic failure a
practice declares is a *refinement* of one of them:

| Class | Structural meaning |
|---|---|
| L1 | unknown-form — the casting names a form the practice does not declare |
| L2 | premise-not-available — a required premise is absent or contradicted in the projection |
| L3 | side-condition — a declared condition fails (timing, consent, scope, soundness) |
| L4 | bad-instantiation — a form is instantiated against arguments its sorts forbid |
| L5 | invalid-chain — a step joins premises the law does not license |
| L6 | resource-violation — a spend breaks the resource's algebra |
| L7 | close-mismatch — the close does not follow from the accounted ledger |

A declared error names its structural class, adds a **discriminant** (which
sub-case of that class it is), and may add an **on-mic signal** — the audible
consequence a performer voices when it fires. The discriminant is what lets
two failures share a structural class and still read as different drama.

**The expected-verdict discipline and the verdict lattice.** Every checked
casting declares the verdict it expects; a scene built on a character's flawed
argument expects a failure, and the checker confirms it fails for the declared
reason. Verdicts carry grades from the lattice in
[07-verdicts-and-llm-boundary](07-verdicts-and-llm-boundary.md).

**The premise-pool projection.** A casting's available premises are exactly
**certain ∩ known-by-speaker ∩ present-in-scene** — the certain projection of
the world at that scene, intersected with what the speaker knows as of that
scene, intersected with what the scene puts on stage. This one projection is
the seam between a practice and the rest of the fabric, and it is where
licensed ignorance becomes checkable (§3).

Everything else — which sorts, which laws, which resources, which conditions,
which failures, which costs — is the practice's own declaration.

## 3. The spoken-proof instance, worked

The Concordance as a practice, from the ratified direction vocabulary:

```dli
# syntax-pending: grammar unfrozen; AST is normative
pack magic

practice spoken-proof  ratified D-0146 D-0148 {
  sort ground = water | field | ore | tide | hearth | boundary   # the Grounds
  sort binder from lexicon.loan-words          # the Binder layer IS the lexicon's
                                               # attested loan-words: magic consuming
                                               # the language layer as an ordinary sort
  role witness : entity    requires standing(witness), consent(witness)
  role holder  : artifact  requires sound(holder)     # fiber, leaf, record, cord
  role seat    : place

  resource debt   : linear conserved
  # THE conservation law, declared: no ledger step mints or burns debt.
  # A casting may reveal, assign, redistribute, release — the multiset balances
  # across every close, or the ledger does not check.
  resource strain : affine bounded body-limit(caster)   # Overload's substrate

  rule ward-form(ground, fibre)  ratified D-0146 :- ...
  rule give-and-take             ratified D-0146 :- ...

  condition seated requires phase(tide) in allowed-hours(rule),
                            scope(claim) within scope(consent)
  condition chorus requires each voice in chain has standing, trust(voice, next)

  # the nine failure modes, each refining a structural class:
  error fizzle             refines L2 premise-not-available where absent
  error rebound            refines L2 premise-not-available where contradicted
                           signals bodily-hit
  error mis-scope          refines L3 side-condition [scope] at close
  error bad-holder         refines L3 side-condition [holder-soundness]
  error wrong-hour         refines L3 side-condition [timing] over wheel tide, moon
  error coerced-party      refines L3 side-condition [consent] signals refusal
  error broken-chorus      refines L5 invalid-chain where chorus fails
  error overload           refines L6 resource-violation on strain signals collapse
  error licensed-ignorance refines L2 where available-in(licensed-pool)
                           and contradicted-in(certain)

  cost bodily | cognitive | material | temporal | social | epistemic
     | scope | opportunity
  }
```

Three design beats carry the weight of the whole approach.

**Refinements, not a parallel taxonomy.** The nine diegetic failures are not a
second error system bolted beside the checker's seven classes — each is a
refinement of one class. Fizzle and Rebound are both L2, split by the
discriminant *absent* versus *contradicted*: a fizzle draws on a premise merely
unestablished, a rebound on one the projection actively denies, and a rebound
signals a bodily hit while a fizzle signals nothing. Wrong-hour is the timing
side condition over the kernel's own wheels — the series-opening beat that a
ward can fail for being made at the wrong hour is one declared line, not a
special case. Overload is the affine bound on `strain`. A ledger reading
`in spoken-proof` speaks the story's language back:

```text
invalid: wrong-hour (L3 [timing] at s2) — tide is flood in e01-s04;
         ward-form allows slack
```

and a scene that declares `expect fails-at wrong-hour` reads as drama, not
plumbing.

**Licensed ignorance is the failure only this kernel can state.** It is a
provenance failure: the proof validates against a *licensed pool* — a deck with
its Weathers removed, an archive with rows struck — while its conclusion is
contradicted in the *certain* projection. The checker states it as a
**differential between two premise pools**: certain, and the licensed subset.
The verdict is "seats and lies" — the soothing-but-false result that powers the
rigged-deck reading. A proof checker without an annotation layer beneath it
cannot even express the two pools, let alone diff them; here it is one declared
error refining L2 with the discriminant *available-in(licensed-pool) and
contradicted-in(certain)*.

**Cost vocabulary is typed; the cost schedule is proposed lore.** The `cost`
line fixes the *currencies* a casting may spend in — bodily, cognitive,
material, temporal, social, epistemic, scope, opportunity — so a ledger records
spends by kind and the craft layer can ask what an episode's magic cost and in
which currencies. What it does not fix is the cost *schedule*: how much strain a
given chain draws is a rule set that enters as `proposed` lore only when the
author rules it. The workbench holds the slot open with a typed vocabulary and
declines to invent the numbers — the schedule is reserved extension work, and a
practice with no schedule yet still checks everything else.

## 4. The arbitrariness proof

A practice is only a genuine parameterization if a second system, disjoint from
the first on every declared axis, hosts with zero kernel code. **True-naming**
is that proof — fifteen declared lines, nothing in common with proof-magic:

```dli
# syntax-pending: grammar unfrozen; AST is normative
pack magic

practice true-naming  proposed {        # a fixture, not canon: the arbitrariness proof
  use language.registers                # its primitives ARE the naming layer (05-language-layer)
  resource acquaintance(name) : graded monotone whole > worn > husk
  rule invoke(bearer) requires
      knows(caster, true-name(bearer)),                     # a knowledge fluent
      acquaintance(true-name(bearer)) >= worn,
      derivation(true-name(bearer)) checks in register-of(bearer)
  effect invoke degrades acquaintance by 1     # speaking a name wears it
  condition unheard requires no witness within earshot
  error slipped-name   refines L4 bad-instantiation
                       # a mispronunciation IS a failed register derivation
  error stale-name     refines L2 where premise-outdated
                       # the bearer has changed since the name was learned
  error name-overheard refines L3 side-condition [unheard] signals degrade(+1)
  cost social | epistemic
  }
```

Nothing here resembles proof-magic. The resource is a graded-monotone decay,
not a conserved linear multiset — nothing is spent, knowing only *wears*.
Failure is degradation and staleness against fluents, not typed proof-step
errors. The premises are names known, not facts derived. And the practice
consumes the language layer wholesale: a true-name is a register derivation, so
a mispronunciation is literally a grammar error (L4), and learning a name is a
knowledge fluent. Three layers — magic, language, and story-time — meet in
fifteen declared lines and zero kernel code.

That is [koan 10](../charter.md) demonstrated rather than argued. The one honest
cost the fixture exacts is `graded monotone`: hosting true-naming required
adding a member to the resource algebra. Had it needed more than one new
algebra member, the parameterization would have failed — which is exactly what
makes this fixture load-bearing.

**Standing role.** The true-naming pack is the workbench's acceptance test. It
is kept in the tree beside the mutation corpus and compiled by the gate: if a
future kernel change makes true-naming require per-system code, the fixture
stops compiling with zero kernel code and the regression is caught at the gate,
not in review.

## 5. Ledger checking

A **ledger** is one casting written out for the checker: its premises, its
steps, its spends, and its close, run `in <practice>`. The ledger step forms
are a small closed set — roughly nine — and the checker instantiates each
explicitly rather than searching for a derivation:

- `premise` — an atom the casting rests on, drawn from the projection, with its
  cue and, where it is an observation, its source (`from-scene`, `observed`).
- `instantiate` — bind a declared form's sorts to concrete arguments.
- `chain` — join prior premises and steps by a named law to a conclusion.
- `assume` — take a fork-local supposition, marked as such.
- `spend` — draw a resource unit against a named premise or step.
- `release` — return a `linear conserved` unit to the multiset.
- `redistribute` — move a conserved unit between accounts without minting.
- `reveal` — surface a conserved unit already implied by the accounts.
- `close` — the casting's conclusion, with its expected verdict.

A ledger is written as data and checked, never authored by the machine:

```dli
# syntax-pending: grammar unfrozen; AST is normative
pack magic

ledger e01-s04  speaker protagonist  in spoken-proof {
  premise p1  holds(enfys, consent(household, door))  cited D-0113  cue "line 112"
  premise p2  hour(slack-tide)  from-scene  cue "line 118"
  premise p3  debt(1)  drawn-against p1
  step   s1  instantiate ward-form  with ground = door, fibre = withy
  step   s2  chain from p1, s1  by give-and-take  conclusion bound(ward, door)
  spend  p3
  close  holds(ward(door))  expect holds
  }
```

The kernel projects the world as of the scene — *certain ∩ known-by-speaker ∩
present* — verifies each premise is actually available in that projection, runs
the steps, does the **linear accounting** over every resource named in the
practice, and matches the close against the declared expectation:

```text
invalid: L2 premise-not-available at p2 — hour(slack-tide) is not established
         in e01-s04; last established hour is high-water
         (episode-01, facts_established); cue "line 118"
expected: holds   got: blocked   grade: checked
```

**The expected verdict cuts both ways.** A scene built on a character's flawed
argument declares `expect fails-at wrong-hour`, and the checker confirms the
casting fails at the declared step for the declared reason. A wrong argument
that *accidentally validates* is as much a bug as a right one that fails — the
`fails-at` expectation is xfail-for-drama, and a silent success where the story
needs a failure is a caught regression. Grades attach to checks: a ledger over
D-cited premises checks at grade `checked`
([07](07-verdicts-and-llm-boundary.md)).

**Linearity stays object-level — the two contact points.** Conservation and
linear resources govern the *object language*: a debt token spent twice inside
a casting is an in-world error. The annotation algebra governs the *meta
language*: which authorial events support a fact. Conflating them would let an
authorial what-if perturb in-world proof accounting, which is nonsense. The
two languages touch at exactly two points, and nowhere else:

1. The checker's premise **selection** reads the certain projection — so the
   set of premises a casting may draw on is governed by support.
2. A checker verdict is **itself a derived atom** whose positive support
   multiplies its premises' polynomials — so a proof's validity degrades
   automatically when a premise later loses support, with no re-run.

Frith conservation is therefore a fold invariant plus the checker's
resource-multiset bookkeeping — an affine discipline as a side condition, not a
linear type theory bolted onto the surface language.

Because a proof ledger produces a derived atom with real support, everything
downstream inherits it: a ruling that weakens a cited premise flips every
ledger that rested on it, at the next `deloreis check`, as a finding
([06](06-craft-and-payloads.md) §7 — the payload battery consumes exactly this).

## 6. Practice well-formedness

`deloreis check` gains three well-formedness obligations for practice
declarations, checked at compile time before any casting is run:

- **Every declared error refines a structural class.** An `error` that names no
  L-class, or names one outside L1–L7, does not compile. The discriminant and
  the optional on-mic signal are checked against the class's shape.
- **Resources use only the closed algebra.** A `resource` whose kind is not
  `linear`, `linear conserved`, `affine bounded <cap>`, or `graded monotone
  <order>` is a compile error — there is no open resource vocabulary.
- **Rules type-check against the practice's sorts.** A law that instantiates a
  form against arguments outside the declared sorts, or references a role or
  condition the practice does not declare, fails to compile.

A ledger gains one clause — `in <practice>` — and loses nothing: the same
ledger checker serves the Concordance, true-naming, and the Turning, because
the practice it runs against is data all the way down.
