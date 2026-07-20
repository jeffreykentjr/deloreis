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
scope craft

rule accomplishment-earned  over season  severity error
  : beat $b kind accomplishment magnitude >= major
    requires beat $f  thread($f) = thread($b)

rule full-team-floor  over order  ratified D-0162
  : listener-has(configuration full-team) requires slot >= 10
```

## States — facts that hold over an interval

A `state` is a fact true across a stretch of time, until something ends it.

```dli
scope chronology

state gwern.water-status = commons  until enclosure-of-the-parish-water  ratified D-0114
```

## Cycles — cyclic time that refuses rank

A `cycle` is a cyclic kind: it has `succ`, `pred`, `across`, and `distance`, but
**no first, no largest, no `<`** (charter koan 8). Asking for the "greatest"
season is a type error, not a silent tie-break.

```dli
scope chronology

cycle tide(4) = ebb | slack | flood | high        ratified D-0146
cycle moon(29)                                     ratified D-0148
```

## Forks — exploring an undecided world

A `fork` supposes an answer and explores its consequences over an unchanged
store; nothing a fork does changes canon (koan 2). Its arms are `option`s, like
a question's.

```dli
scope saltmarket

fork what-does-a-name-cost
  option a-memory
  option a-year-of-service
```

## Practices — magic systems as checked lore

A `practice` is a magic system: kinds, roles, resources, rules, conditions,
errors, and costs, captured in one shallow braced block. Its interior is
captured verbatim and checked by its lane; nothing is dropped (koan 4).

```dli
scope magic

practice spoken-proof  ratified D-0146 D-0148 {
  kind ground = water | field | ore | tide | hearth | boundary
  role witness : entity    requires standing(witness), consent(witness)
  resource debt   : linear conserved
  rule ward-form(ground, fibre)  ratified D-0146 :- ...
  condition seated requires phase(tide) in allowed-hours(rule)
  error fizzle             refines L2 premise-not-available where absent
  cost bodily | cognitive | material | temporal
  }
```

## Proofs — a scene's reasoning, step by step

A `proof` records a bounded piece of reasoning — a premise, the steps taken, a
spend, and what it closes — inside a practice.

```dli
scope magic

proof e01-s04  speaker protagonist  in spoken-proof {
  premise p1  holds(enfys, consent(household, door))  cited D-0113  cue "line 112"
  step   s1  instantiate ward-form  with ground = door, fibre = withy
  spend  p3
  close  holds(ward(door))  expect holds
  }
```

## Dialects and names — a constructed language

A `dialect` is a language variety with its inventory and name-parts; `name`
composes a proper name from parts; `term` and `banned` govern in-world
vocabulary; a `hypothesis` conditions speculative claims, and `ratify` records
the ruling that settles one.

```dli
scope language

dialect cymric-fringe  "the fringe tongue"  provisional D-0181 {
  vowels a e i o u w y
  name-part en   "spirit"        role given-proto     ratified D-0134
  }

name enfys "Enfys"  person.given in cymric-fringe
  = given-proto en + given-deutero fys
  bearer protagonist   ratified D-0134

term hedgeway "the hedgeway"  ratified D-0143
banned machine   D-0056  "the allegory is structural, never lexical"

hypothesis h-cymric "the western fringe speaks a Brythonic-shaped tongue"

dialect speculative  proposed agent:factory/naming-run-0712  assumes h-cymric {
  inventory placeholder
  }

ratify D-0195 { h-cymric := true }
```

---

## The one rule that still holds

Everything here obeys the first card's discipline: **a claim carries its
origin**, and nothing is true until the author rules it so (koan 1, koan 3). The
expert vocabulary buys reach — checkable rules, cyclic time, forks, magic — but
not a way past provenance. When you materialize, the formatter renders all of it
into the one canonical surface, reviewed as surface like every other line.
