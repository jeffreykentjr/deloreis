---
status: live
title: "Expert-tier reference card"
verdict: "The second card: the expert vocabulary — rules, forks, cycles, practices, proofs, dialects, states — in the same flat, one-claim-per-line shape as the first."
---

# The expert tier — reference card

This is the second card. It adds **vocabulary, never grammar** (spec
`../spec/01-syntax.md` §2–§3): every form below is the same flat, order-
insensitive, one-claim-per-line record you already know, and every claim still
ends by saying where it came from. The expert tier is not a second syntax regime
— it is more keywords in the one shape. Where a form has an interior, it is a
shallow, brace-`{ … }`-delimited block, never a deep tree.

Nothing on this card is needed to build a world; the gentle card
(`gentle-card.md`) is most of worldbuilding. Reach here when you want rules that
check, forks that explore, cyclic time, magic systems, or a constructed
language. Each lane has a full spec under `../spec/`; this card is the map.

---

## Rules — claims that check

A `rule` is a checkable constraint. A bare `rule` checks the world; `rule …
over order` constrains a timeline; `rule … over season` is a season-ledger
rule. All carry a severity and their origin — the domain is stated in the form,
not encoded in a separate keyword.

```dli
scope camelot

rule uthers-time-held-back  over order  severity error  decided d-0009
  : listener-learns(during uthers-time _) requires act > 1

rule show-the-price  over season  severity warn  proposed
  : listener-learns(cost magnitude >= major) requires season >= 2
```

## States — facts that hold over an interval

A `state` is a fact true across a stretch of time, until something ends it.

```dli
scope camelot

state excalibur.keeper = in-the-stone  until the-sword-drawn  decided d-0008
```

## Cycles — cyclic time that refuses rank

A `cycle` is a cyclic kind: it has `succ`, `pred`, `across`, and `distance`, but
**no first, no largest, no `<`** (charter koan 8). Asking for the "greatest"
season is a type error, not a silent tie-break.

```dli
scope camelot

cycle season(4) = spring | summer | autumn | winter   decided d-0009
cycle moon(29)                                         decided d-0009
```

## Forks — exploring an undecided world

A `fork` supposes an answer and explores its consequences over an unchanged
store; nothing a fork does changes canon (koan 2). Its arms are `option`s, like
a question's.

```dli
scope camelot

fork what-does-the-word-cost
  option a-year-of-life
  option a-truth-told
```

## Practices — magic systems as checked lore

A `practice` is a magic system: kinds, roles, resources, rules, conditions,
errors, and costs, captured in one shallow braced block. Its interior is
captured verbatim and checked by its lane; nothing is dropped (koan 4).

```dli
scope camelot

practice oath-binding  proposed {
  kind bond = fealty | troth | ransom
  role oathgiver : person   requires standing(oathgiver)
  resource boon   : linear conserved
  rule pledge(bond, oathgiver)
  condition witnessed requires witness within earshot
  error unowed         refines L2 premise-not-available where absent
  error wrong-moon     refines L3 side-condition [timing] over cycle moon
  cost social | bodily | material | epistemic
  }
```

## Proofs — a scene's reasoning, step by step

A `proof` records a bounded piece of reasoning — a premise, the steps taken, a
spend, and what it closes — inside a practice.

```dli
scope camelot

proof lancelots-oath  speaker lancelot  in oath-binding {
  premise p1  standing(lancelot)  cited d-0007  cue "Lancelot comes to court"
  premise p2  consent(arthur)     cited d-0006
  step   s1  instantiate pledge  with bond = fealty, oathgiver = lancelot
  spend  p1
  close  pledged(lancelot, fealty)  expect holds
  }
```

## Dialects and names — a constructed language

A `dialect` is a language variety with its inventory and name-parts; `name`
composes a proper name from parts; `term` and `banned` govern in-world
vocabulary; a `hypothesis` conditions speculative claims, and `ratify` records
the ruling that settles one.

```dli
scope camelot

dialect old-speech  "the old speech"  proposed {
  vowels a e i o u y
  name-part myr   "sea"        role given-proto     proposed
  }

name myrdin "Myrdin"  person.given in old-speech
  = given-proto myr + given-deutero din
  bearer the-enchanter   proposed

term the-drawing "the freeing of Excalibur from the stone"  decided d-0008
banned cannon   d-0010  "the Matter of Britain keeps to sword and spear — no gunpowder anachronism"

hypothesis h-old-loan "the old speech took a hero-name from the tongue over the water"

dialect over-the-water  proposed agent:factory/naming-run-0714  assumes h-old-loan {
  inventory placeholder
  }

ratify d-0011 { h-old-loan := true }
```

---

## The one rule that still holds

Everything here obeys the first card's discipline: **a claim carries its
origin**, and nothing is true until the author rules it so (koan 1, koan 3). The
expert vocabulary buys reach — checkable rules, cyclic time, forks, magic — but
not a way past provenance. When you materialize, the formatter renders all of it
into the one canonical surface, reviewed as surface like every other line.
