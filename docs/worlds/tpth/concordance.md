---
status: live
title: "TPtH worldbook — The Concordance"
verdict: "The spoken-proof practice behind spec 04: the linear-conserved debt, the nine failure modes, licensed-ignorance and the rigged-deck reading, the e01-s04 proof ledger, and Frith conservation."
---

# The Concordance — the spoken-proof practice

> **Worldbook.** This chapter faces [spec 04 · Practices](../../spec/04-practices.md)
> and documents the practice data at
> [`worlds/tpth/practices/spoken-proof.dli`](../../../worlds/tpth/practices/spoken-proof.dli).
> The spec teaches the generic practice machinery on camelot's `oath-binding` and
> `merlins-craft`; the worked Concordance instance it used to carry lives here.
> The binder layer interlocks with [Language](language.md) — `kind binder from
> lexicon.loan-words`.

The Concordance — the Spoken Proof that opens the series — is not a component of
the kernel. It is the *spoken-proof instance* of a generic practice, one `.dli`
scope among others: grounds, binders, wards, debt, and the wrong-hour failure are
declarations, not Rust. This is the TPtH depth behind the mechanism the spec
teaches generically.

## The practice, worked

The Concordance as a practice, from the ratified direction vocabulary:

```dli
# grammar: v0 (unfrozen; AST normative)
scope magic

practice spoken-proof  ratified D-0146 D-0148 {
  kind ground = water | field | ore | tide | hearth | boundary   # the Grounds
  kind binder from lexicon.loan-words          # the Binder layer IS the lexicon's
                                               # attested loan-words: magic consuming
                                               # the language layer as an ordinary kind
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
  error wrong-hour         refines L3 side-condition [timing] over cycle tide, moon
  error coerced-party      refines L3 side-condition [consent] signals refusal
  error broken-chorus      refines L5 invalid-chain where chorus fails
  error overload           refines L6 resource-violation on strain signals collapse
  error licensed-ignorance refines L2 where available-in(licensed-pool)
                           and contradicted-in(certain)

  cost bodily | cognitive | material | temporal | social | epistemic
     | scope | opportunity
  }
```

## The nine failure modes

The nine diegetic failures are not a second error system bolted beside the
checker's seven classes — each is a refinement of one class. Fizzle and Rebound
are both L2, split by the discriminant *absent* versus *contradicted*: a fizzle
draws on a premise merely unestablished, a rebound on one the projection actively
denies, and a rebound signals a bodily hit while a fizzle signals nothing.
Wrong-hour is the timing side condition over the kernel's own cycles — the
series-opening beat that a ward can fail for being made at the wrong hour is one
declared line, not a special case. Overload is the affine bound on `strain`. A
ledger reading `in spoken-proof` speaks the story's language back:

```text
invalid: wrong-hour (L3 [timing] at s2) — tide is flood in e01-s04;
         ward-form allows slack
```

and a scene that declares `expect fails-at wrong-hour` reads as drama, not
plumbing.

## Licensed ignorance and the rigged-deck reading

Licensed ignorance is the failure only this kernel can state. It is a provenance
failure: the proof validates against a *licensed pool* — a deck with its Weathers
removed, an archive with rows struck — while its conclusion is contradicted in the
*certain* projection. The checker states it as a **differential between two
premise pools**: certain, and the licensed subset. The verdict is "seats and
lies" — the soothing-but-false result that powers the rigged-deck reading. A proof
checker without an annotation layer beneath it cannot even express the two pools,
let alone diff them; here it is one declared error refining L2 with the
discriminant *available-in(licensed-pool) and contradicted-in(certain)*. This is
the allegory made checkable: what the licensed decks cannot deal, computed rather
than asserted.

## The e01-s04 proof ledger

A **ledger** is one casting written out for the checker. The opening episode's
door-ward, run `in spoken-proof`:

```dli
# grammar: v0 (unfrozen; AST normative)
scope magic

proof e01-s04  speaker protagonist  in spoken-proof {
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
present* — verifies each premise, runs the steps, does the **linear accounting**
over every resource named in the practice, and matches the close against the
declared expectation. When the hour is wrong, the reading speaks it back:

```text
invalid: L2 premise-not-available at p2 — hour(slack-tide) is not established
         in e01-s04; last established hour is high-water
         (episode-01, facts_established); cue "line 118"
expected: holds   got: blocked   grade: checked
```

## Frith conservation

Frith conservation is a fold invariant plus the checker's resource-multiset
bookkeeping — an affine discipline as a side condition, not a linear type theory
bolted onto the surface language. Conservation and linear resources govern the
*object language*: a debt token spent twice inside a casting is an in-world error.
The annotation algebra governs the *meta language*. The two languages touch at
exactly two points: the checker's premise selection reads the certain projection,
and a checker verdict is itself a derived atom whose support multiplies its
premises' polynomials — so a proof's validity degrades automatically when a
premise later loses support, with no re-run. A ruling that weakens a cited premise
flips every ledger that rested on it, at the next `deloreis check`.
