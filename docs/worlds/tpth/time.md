---
status: live
title: "TPtH worldbook — Time"
verdict: "The TPtH chronology worked instances behind spec 03: the elder-cycles→the-narrators-era spine, the tide/moon cycles and the Turning's leaf(29), the water/enclosure fluent (D-0114), and the inert narration vantage (D-0104)."
---

# Time — the TPtH chronology

> **Worldbook.** This chapter faces [spec 03 · Story-time](../../spec/03-time.md)
> and documents the TPtH time data at
> [`worlds/tpth/time/cycles.dli`](../../../worlds/tpth/time/cycles.dli). The spec
> teaches the four-component time model on the shipped
> [`examples/camelot.dli`](../../../examples/camelot.dli); the worked TPtH
> instances it used to carry live here.

TPtH runs a six-era spine from the deep past to the frame from which the story is
told, a partial intra-era order, two natural cycles that a ward seats its hour
on, and fluents whose validity spans let a flashback and the story-present check
clean at once.

## The chronology surface

The chronology layer — the era spine, events and their before-facts, the inert
narration vantage, the cycles, and states with spans — exactly as it was worked
in spec 03 §6:

```dli
# grammar: v0 (unfrozen; AST normative)
scope chronology

era elder-cycles < the-equilibrium < the-first-enclosure
    < the-long-creep < the-beleaguered-present < the-narrators-era
# acts are NOT eras — narrative structure lives in its own layer and
# references chronology; the spine gains eras only by ruling

event first-storehouse-lie    during the-first-enclosure   provisional D-0072
event protagonists-taking     during the-long-creep        ratified    D-0060
event breaking-of-the-stones  during the-long-creep        provisional D-0072
before breaking-of-the-stones -> tithe-of-minds-sweeps     provisional D-0072
before tithe-of-minds-sweeps  -> protagonists-taking       provisional D-0072
# no `offset`; an undeclared pair is honestly unordered:
# happens-before(x, y) may answer `unordered`

narration vantage the-narrators-era
# inert (D-0104): declaring an event in this era is error T2 [narration-dynamics]

cycle tide(4) = ebb | slack | flood | high        ratified D-0146
cycle moon(29)                                     ratified D-0148
# the same cyclic kind as the Turning's leaf(29): succ/pred/across/distance
# exist; `<`, sort, max are type errors — no epoch, no calendar

state gwern.water-status = commons  until enclosure-of-the-parish-water  ratified D-0114
state gwern.water-status = enclosed since enclosure-of-the-parish-water  ratified D-0114
# unanchored facts default to `since era(the-beleaguered-present)` — the
# migration rule; the entire existing facts corpus is valid unchanged
```

Ordering rules are declarations too — data the season checker reads, not code —
the TPtH staging constraints in full:

```dli
# grammar: v0 (unfrozen; AST normative)
rule full-team-floor over order       ratified D-0162
  : listener-has(configuration full-team) requires slot >= 10
rule no-deep-time-act-one over order  ratified D-0168
  : listener-learns(during elder-cycles _) requires act > 1
rule payoff-after-reveal over order
  : payoff($p) at slot $j requires reveal($p) at slot $i, $i < $j
```

An episode binds a production `order` and a story-time `at` anchor separately, and
a scene may carry its own anchor to cross into story-time — a flashback:

```text
episode e14 "The Drowned Field" order 14 {
  at the-beleaguered-present after e13      # default if omitted: follows prior
  phase tide := ebb -> slack                # a walk, checked; not a timestamp
  scene s3 flashback {
    at the-long-creep after protagonists-taking before tithe-of-minds-sweeps
    # s3's world checks run against state-at(this anchor), not the fold head;
    # its reveals still land at production slot 14 on the listener ledger
  }
}
```

## The water/enclosure fluent (D-0114)

The water-status case is TPtH's canonical fluent. A fact that the parish water was
held in common *until* the enclosure and enclosed *since* it (D-0114) lets a
pre-enclosure flashback check clean *and* the story-present episode check clean.
The false contradiction a flashback would otherwise raise against the
story-present enclosure fact is gone — not suppressed, but structurally absent,
because the two claims hold over disjoint spans. The enclosure of the parish water
is the allegory's founding injury, and the model lets the story move across it
without lying about either side.

## The narration frame (D-0104)

`the-narrators-era` sits centuries out from story-present, and `narration-now`
sits inside it. It is **inert by construction** (D-0104): declaring an event in
the narrator's era is a type error, no state dynamics exist there, and no
machinery accrues to it. What the vantage buys TPtH is the knowledge asymmetry —
the narrator is retrospectively omniscient over story-present while the cast is
evaluated as-of scene position — and the `narration-frame` guardrail: facts whose
validity begins after story-present (how the story came to be remembered, a later
vindication) are structurally excluded from any cast premise pool. A
narrators-era fact can never be cast knowledge.

## The cycles and the Turning's leaf(29)

Tide, moon, and season are TPtH's cyclic sorts, orthogonal to the linear spine.
Only the Turning's `leaf(29)` — one leaf for each night of the moon — is TPtH's
own; tide/moon/season are public-domain natural cycles the world merely declares.
The **same cyclic kind** lives once in the kernel, shared by the game engine and
the timing checker: canon itself says the same turning that seats a ward runs the
Turning's leaves ([the Turning export](turning-export.md)). A ward requiring the
slack tide, spoken in a flood-bound scene, is a checker error; cycle-walk
consistency holds within declared contiguity; and the moon cannot jump
mid-episode. The cross-reference to the deck's `leaf(29)` is what makes "the
turning that seats a ward" and "the Turning that deals the leaves" the same word,
honestly.
