---
status: live
verdict: "A magic system is a practice with laws; the checker is generic, the systems are data."
title: "Practices: magic systems, games, and any declared law-set"
---

# Practices

A **practice** is a declared system of laws — a magic system, a divination
game, a rhetorical proof discipline, a craft with rules — expressed entirely
as data in the fabric. The kernel supplies one generic checker and one generic
evaluator; a practice supplies the kinds it reasons over, the laws as annotated
rules, the resources it spends, the side conditions its castings must satisfy,
the ways it fails, and the currencies those failures and successes cost. From
that declaration the kernel derives a checker for castings, proofs, readings,
and workings in that practice — with citations, expected verdicts, and honesty
grades, exactly as for any other lore.

The same practice and proof machinery can check different declared systems.
A practice describes its rules as data, and `check-proof` checks a proof
against that declaration. DLI does not ship a practice simulator.

## 1. The claim, precisely

**A magic system is a practice with laws, and the laws are data.** A
spoken-enchantment practice, a vow discipline, a card game — none is a component
of the kernel. Each is one *practice instance* of a generic machine, one `.dli`
scope among others. Nothing about a practice's own kinds, its resources, its side
conditions, or its timing failure is written into any Rust type or evaluator
branch. Point the same machinery at a wholly unrelated art — a true-naming
enchantment, a card game, a smith's temper-craft — and it runs with zero kernel
change, because everything that distinguishes one practice from another lives in
the practice's declaration.

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

**Typed finite kinds, including the cyclic `cycle`.** A practice declares its
own kinds — bonds, grounds, roles — as ordinary finite kinds
([01-syntax](01-syntax.md)). The cyclic `cycle` kind ([03-time](03-time.md),
[koan 8](../charter.md)) is available on the same footing: tide, moon, and
season are cycles, and so is any rankless wheel of a game's turns. A cycle
carries `succ`, `pred`, `across`, and `distance` and refuses `<`, sort order,
and maximum — the same rank-refusing kind the timing checker and the game
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
the world as projected into a scene: states that hold there, cycle phases in
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
| L4 | bad-instantiation — a form is instantiated against arguments its kinds forbid |
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

Everything else — which kinds, which laws, which resources, which conditions,
which failures, which costs — is the practice's own declaration.

## 3. A conserved-resource practice, worked

A vow is a practice with laws. **Oath-binding** — a public-domain "a boon owed
must balance" — declares a `linear conserved` resource and the failures that
attend it, entirely as data:

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

practice oath-binding  proposed {
  use chronology.cycles                        # the pledge seats its hour on the moon
  kind bond = fealty | troth | ransom
  role oathgiver  : person   requires standing(oathgiver)
  role oathholder : person   requires consent(oathholder)
  resource boon   : linear conserved           # a boon owed must balance — the debt law
  resource strain : affine bounded body-limit  # the swearer's body caps the draw
  rule pledge(bond, oathgiver)
  rule give-and-take                           # every boon revealed, assigned, or released balances
  condition witnessed requires witness within earshot
  error unowed        refines L2 premise-not-available where absent
                      # a pledge drawn on a boon never granted
  error forsworn      refines L2 premise-not-available where contradicted signals dishonour
                      # a boon the ledger contradicts recoils on the swearer
  error wrong-moon    refines L3 side-condition [timing] over cycle moon
                      # the same oath, sworn at the wrong turn of the moon
  error overpledged   refines L6 resource-violation on strain signals collapse
                      # pledging more strain than the body-limit allows
  error hollow-vow    refines L2 where available-in(licensed-pool) and contradicted-in(certain)
                      # seats-and-lies: true against a licensed pool, false against the certain
  cost social | bodily | material | epistemic
  }
```

Three design beats carry the weight of the whole approach.

**Refinements, not a parallel taxonomy.** The declared failures are not a
second error system bolted beside the checker's seven classes — each is a
refinement of one class. Unowed and Forsworn are both L2, split by the
discriminant *absent* versus *contradicted*: an unowed pledge draws on a boon
never granted, a forsworn one on a boon the ledger actively contradicts, and
forsworn signals dishonour while unowed signals nothing. Wrong-moon is the timing
side condition over the kernel's own cycles — an oath that fails for being sworn
at the wrong turn of the moon is one declared line, not a special case.
Overpledged is the affine bound on `strain`. A ledger reading `in oath-binding`
speaks the story's language back:

```text
invalid: wrong-moon (L3 [timing] at s2) — the moon is at the wane in the pledge;
         the oath allows only the waxing turn
```

and a scene that declares `expect fails-at wrong-moon` reads as drama, not
plumbing.

**Licensed ignorance is the failure only this kernel can state.** It is a
provenance failure: the vow validates against a *licensed pool* — a set of
premises with certain facts withheld — while its conclusion is contradicted in
the *certain* projection. The checker states it as a **differential between two
premise pools**: certain, and the licensed subset. The verdict is "seats and
lies" — an oath true against what the swearer is licensed to know and false
against what is certain (oath-binding's `hollow-vow`). A proof checker without an
annotation layer beneath it cannot even express the two pools, let alone diff
them; here it is one declared error refining L2 with the discriminant
*available-in(licensed-pool) and contradicted-in(certain)*.

**Cost vocabulary is declared, not computed.** A practice can name cost
currencies such as social, bodily, material, or epistemic in a `cost` line.
The current ledger checks declared resource premises; it does not produce a
per-currency cost report from that vocabulary.

## 4. The arbitrariness proof

A practice is only a genuine parameterization if a second system, disjoint from
the first on every declared axis, hosts with zero kernel code. **Merlin's craft**
is that proof — a spoken enchantment with nothing in common with a conserved-debt
vow:

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

practice merlins-craft  proposed {      # the arbitrariness proof: nothing in common with oath-binding
  use language.registers                # its primitives ARE the naming layer (05-language-layer)
  resource breath(word) : graded monotone full > spent > silent
  rule speak(bearer) requires
      knows(caster, true-name(bearer)),                     # a knowledge state
      breath(true-name(bearer)) >= spent,
      derivation(true-name(bearer)) checks in register-of(bearer)
  effect speak degrades breath by 1     # a word spoken thins the breath behind it
  condition unheard requires no witness within earshot
  error mis-spoken     refines L4 bad-instantiation
                       # a mispronunciation IS a failed dialect derivation
  error stale-name     refines L2 where premise-outdated
                       # the bearer has changed since the name was learned
  error word-overheard refines L3 side-condition [unheard] signals degrade(+1)
  cost vital | epistemic
  }
```

Nothing here resembles a conserved-debt vow. The resource is a graded-monotone
decay, not a conserved linear multiset — nothing is spent, the breath only
*wears*. Failure is degradation and staleness against states, not typed
proof-step errors. The premises are names known, not boons owed. And the practice
consumes the language layer wholesale: a true-name is a dialect derivation, so
a mispronunciation is literally a grammar error (L4), and learning a name is a
knowledge state. Three layers — magic, language, and story-time — meet in a
dozen declared lines and zero kernel code.

That is [koan 10](../charter.md) demonstrated rather than argued. The one honest
cost this practice exacts is `graded monotone`: hosting merlins-craft required
adding a member to the resource algebra. Had it needed more than one new
algebra member, the parameterization would have failed — which is exactly what
makes this example load-bearing.

**Standing role.** The camelot scope is the workbench's acceptance test. It
ships as [`examples/camelot.dli`](../../examples/camelot.dli) and is compiled by
the gate: if a future kernel change makes merlins-craft require per-system code,
the example stops compiling with zero kernel code and the regression is caught at
the gate, not in review.

## 5. Ledger checking

A **ledger** is one casting written out for the checker: its premises, its
steps, its spends, and its close, run `in <practice>`. The ledger step forms
are a small closed set — roughly nine — and the checker instantiates each
explicitly rather than searching for a derivation:

- `premise` — an atom the casting rests on, drawn from the projection, with its
  cue and, where it is an observation, its source (`from-scene`, `observed`).
- `instantiate` — bind a declared form's kinds to concrete arguments.
- `chain` — join prior premises and steps by a named law to a conclusion.
- `assume` — take a fork-local supposition, marked as such.
- `spend` — draw a resource unit against a named premise or step.
- `release` — return a `linear conserved` unit to the multiset.
- `redistribute` — move a conserved unit between accounts without minting.
- `reveal` — surface a conserved unit already implied by the accounts.
- `close` — the casting's conclusion, with its expected verdict.

Proof bodies are fail-closed syntax. Only those child keywords may open a proof
line; fixed-shape operations reject missing fields and surplus tokens before the
document can be formatted or replayed. `instantiate` and `chain` are parsed to
their complete shapes. `assume` always contributes grade `asserted`; a promoted
or conflicting grade is malformed input. An instantiation binds every declared
parameter exactly once, and a value for a declared finite kind must be one of
that kind's members. A practice may also invoke a declared rule directly as
`step <label> <rule> [<value>, ...]`; those positional terms must match the
rule's exact arity and finite kinds. Opaque custom-step text is not a proof.
`reveal` and `redistribute` must name an existing resource premise even though
their accounting delta is zero. A proof header carries exactly one role binding
(`speaker`, or a role declared by the practice) and one `in <practice>` target,
and every proof ends with exactly one final `close`; the checker revalidates
header layout, header roles, close layout, and the complete canonical
surface-to-parser replay for direct AST callers. Relation atoms must have
balanced parentheses in every proof position; nested claims use the same grammar
in premises, chain conclusions, and closes. Binding and positional values are
tokenized terms, so a comma inside a quoted value never becomes a separator.
Consequently `check`, `check-proof`, and `fmt` cannot silently turn a malformed
or unsupported proof into a smaller valid one.

When a CLI proof file or worker document declares more than one proof, the
caller selects one explicitly with `--proof <name>` or the wire `proof` field.
The selector may be omitted only for a single-proof input; ambiguity, an unknown
name, or duplicate names fail closed. The practice always comes from the
selected proof's `in <practice>` header.

A ledger is written as data and checked, never authored by the machine:

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

proof scene-04  speaker merlin  in merlins-craft {
  premise p1  knows(merlin, true-name(the-enchanter))  cited d-0006  cue "line 112"
  premise p2  breath(true-name(the-enchanter))  from-scene  cue "line 118"
  step   s1  instantiate speak  with bearer = the-enchanter
  step   s2  chain from p1, s1  by speak  conclusion named(the-enchanter)
  spend  p2
  close  named(the-enchanter)  expect holds
  }
```

The kernel projects the world as of the scene — *certain ∩ known-by-speaker ∩
present* — verifies each premise is actually available in that projection, runs
the steps, does the **linear accounting** over every resource named in the
practice, and matches the close against the declared expectation:

```text
invalid: L2 premise-not-available at p2 — breath(true-name(the-enchanter)) is
         spent in scene-04; the caster has no breath left to speak the name
         (episode-01, facts_established); cue "line 118"
expected: holds   got: blocked   grade: checked
```

**The expected verdict cuts both ways.** A scene built on a character's flawed
argument declares `expect fails-at stale-name`, and the checker confirms the
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

Conservation is therefore a fold invariant plus the checker's
resource-multiset bookkeeping — an affine discipline as a side condition, not a
linear type theory bolted onto the surface language.

Because a proof ledger produces a derived atom with real support, everything
downstream inherits it: a ruling that weakens a cited premise flips every
ledger that rested on it, at the next proof check, as a finding.

## 6. Practice well-formedness

`deloreis check` gains three well-formedness obligations for practice
declarations, checked at compile time before any casting is run:

- **Every declared error refines a structural class.** An `error` that names no
  L-class, or names one outside L1–L7, does not compile. The discriminant and
  the optional on-mic signal are checked against the class's shape.
- **Resources use only the closed algebra.** A `resource` whose kind is not
  `linear`, `linear conserved`, `affine bounded <cap>`, or `graded monotone
  <order>` is a compile error — there is no open resource vocabulary.
- **Rules type-check against the practice's kinds.** A law that instantiates a
  form against arguments outside the declared kinds, or references a role or
  condition the practice does not declare, fails to compile.

A ledger gains one clause — `in <practice>` — and loses nothing: the same
ledger checker serves oath-binding, merlins-craft, and any declared game, because
the practice it runs against is data all the way down.
