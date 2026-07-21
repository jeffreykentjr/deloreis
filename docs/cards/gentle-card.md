---
status: live
title: "Gentle-subset reference card"
verdict: "The first card: the whole gentle subset on one page — entities, facts, timeline, questions, and where every claim comes from. No expert word appears here."
---

# The gentle subset — reference card

This is the first card, and for most worldbuilding it is the only one you need.
It shows the **whole** gentle subset: entities, facts and relations, timeline
points, questions, and the one word that says where a claim comes from. Read
every line as notes, not as English — **what parses is exactly what this card
shows** (spec `../spec/01-syntax.md` §4). There is no lenient variant grammar
hiding behind it.

The card supersedes `SKILL.md` §4 as the canonical statement of the gentle
subset; where they differ, this card is right.

---

## The shape

One claim per line. A leading word names what the line is; a reference names a
thing by its own name, never by position; and **every claim ends by saying where
it came from**. Where a thing has parts — an entity's slots, a question's
options — they sit indented under it, one level, never a deeper tree.

You type loosely and save; saving produces the one canonical form (spec §1), so
spacing and alignment are never yours to get right.

---

## The gentle words

The whole subset is these leading words and nothing else:

- **`scope`** — opens a world (a namespace for its claims).
- **`place`**, **`person`**, **`thing`** — the entities of the world.
- **`era`**, **`event`**, **`before`** — points and orderings in time.
- **`adjacent`** and other named relations — facts of the form `pred(a, b)`.
- **`question`** with its **`option`**s — an undecided choice, kept as a debt.
- a slot fact — `subject.slot = value` — an entity's own detail.

And the words that give a claim its origin:

- **`decided d-nnnn`** — a recorded ruling stands behind this claim.
- **`proposed`** — a claim awaiting one: legal, kept, visibly out of canon.

That is the entire card. Everything else lives on the expert card, and no word
of it is needed here.

---

## A first world

A complete small world fits in the gentle subset:

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

era uthers-time   "Uther's Time"    decided d-0001
era arthurs-reign "Arthur's Reign"  after uthers-time

place camelot   "Camelot"    decided d-0004
place the-lake  "The Lake"    decided d-0004
place avalon    "Avalon"      proposed

adjacent(camelot, the-lake)  decided d-0004
```

An entity carries its own details as indented slot facts, each with its origin:

```dli
person arthur "Arthur, the King"    decided d-0006
  arthur.seat  = camelot            decided d-0006
  arthur.blade = excalibur          proposed

thing excalibur "Excalibur"         decided d-0008
  excalibur.keeper = the-king       decided d-0008
```

Time is a line of eras and orderings of events:

```dli
event the-round-table-founded "The Round Table Founded"  decided d-0002
event the-battle-of-camlann   "The Battle of Camlann"    proposed
before the-round-table-founded -> the-battle-of-camlann  decided d-0003
```

An undecided choice is not a blank — it is a `question` carrying its `option`s,
a debt the world keeps until you rule on it:

```dli
question who-holds-excalibur "Who holds Excalibur after Camlann?"
  option arthur   implies keeps(arthur, excalibur)
  option the-lake implies keeps(the-lake, excalibur)
```

---

## Where a claim comes from

Every claim ends in its origin, and this is the one discipline the gentle subset
asks of you (spec §4; charter koan 1):

- `decided d-0004` — a ruling you recorded (your decision list is just a file of
  what you chose, and when).
- `proposed` — a claim with no ruling yet. `avalon` and `arthur.blade` above
  are proposed: fine as notes, out of canon until you decide.

A claim with **no** origin is not wrong — it is read as a sketch, kept, and
flagged so the surface says plainly that it has none. Nothing you jot is ever
lost (koan 4), and choosing truth stays a separate, later act (koan 5).

When `deloreis check` answers, it answers in the same plain voice you wrote in:

> *camelot: 10 claims certain, 3 proposed, 1 open question (who-holds-excalibur).*
> *`avalon` has no decision behind it — fine as notes; it stays proposed and*
> *out of canon until you decide.*

---

## When you outgrow this card

You can work for a long time entirely here and still be using the full
provenance semantics underneath. The rest of the language — the power a writer
reaches for only occasionally — is on the second card. You will not need it to
build a world; you will know when you want it.
