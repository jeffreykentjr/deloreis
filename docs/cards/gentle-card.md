---
status: live
title: "Gentle-subset reference card"
verdict: "The first card: the whole gentle subset on one page — entities, facts, timeline, questions, and where every claim comes from — as the intents you name to the author.* tools and the lines they render back. No expert word appears here."
---

# The gentle subset — reference card

This is the first card, and for most worldbuilding it is the only one you need.
It shows the **whole** gentle subset: entities, facts and relations, timeline
points, questions, and the one word that says where a claim comes from.

Two things are true at once here, and the card is built around both:

- **You add lore by naming an intent to a tool.** "A place called Avalon,
  proposed" is a small JSON request to the `deloreis` server's `author.*` tools,
  and the tools do the rest — check it, store it, and tell you what they made of
  it. You do not type the lines below into a file. There is no file to type them
  into.
- **You read lore in the lines below.** They are the **rendering**: the short,
  sentence-like view the tools offer on request, so that what the store keeps as
  a dozen JSON objects can be taken in at a glance. Read every line as notes, not
  as English — **what the rendering shows is exactly what the world holds** (spec
  `../spec/01-syntax.md` §4). There is no lenient variant grammar hiding behind
  it.

So: this card teaches you the *vocabulary* — what a world is made of — through
the form that is easiest to read. `authoring-loop.md` walks the calls that
produce it.

The card supersedes `SKILL.md` §4 as the canonical statement of the gentle
subset; where they differ, this card is right.

---

## The shape

One claim per line. A leading word names what the line is; a reference names a
thing by its own name, never by position; and **every claim ends by saying where
it came from**. Where a thing has parts — an entity's slots, a question's
options — they sit indented under it, one level, never a deeper tree.

None of the layout is yours to get right, because you never produce these lines.
The renderer produces them, the same way every time, from what the world
actually holds — so spacing, alignment, and the order claims appear in are its
problem, not yours (spec §1).

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

## The one tool per word

Each of those words is something you *ask for*, and one tool asks for it. This
is the whole gentle interface:

| What you want to add | The call | What it reads back as |
|---|---|---|
| a place, person, thing, or era | `author.add_entity` | `place avalon "Avalon" proposed` |
| a detail of one of them | `author.add_fact` | `arthur.seat = camelot decided d-0006` |
| a fact holding between two | `author.add_relation` | `adjacent(camelot, the-lake) decided d-0004` |
| a point in time | `author.add_event` | `event the-battle-of-camlann "…" proposed` |
| an ordering between two events | `author.add_before` | `before the-sword-drawn -> camlann decided d-0003` |
| an undecided choice | `author.open_question`, then `author.add_option` | `question …` with its `option`s |

A whole call is small. This one adds Avalon, and says the claim is yours and
unruled:

```json
{
  "op": "author.add_entity",
  "params": {
    "workspace": "ws-1",
    "kind": "place",
    "id": "avalon",
    "name": "Avalon",
    "provenance": { "kind": "proposed", "origin": "session-7c9" }
  }
}
```

Three things about it are worth naming. The **`id`** is how everything else will
refer to Avalon — never its position, never its label. The **`name`** is only
what a reader sees. And the **`provenance`** is not optional and not decoration:
it is the claim's truth condition. `proposed` is the only kind this card needs;
a cited decision must resolve in the consuming ledger, and an unknown citation
is refused as an error finding rather than staged. Nothing you send makes anything true
(charter koan 3).

---

## A first world

A complete small world fits in the gentle subset. This is how it reads:

```dli
# the rendering of one scope; the stored fact objects are the definition
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

Ask the world how it stands and it answers as data — `author.findings` over your
workspace:

```json
{
  "op": "author.findings",
  "params": { "workspace": "ws-1" }
}
```

Findings come back in two piles. **`errors`** bar the change from being committed
until you fix them; **`warnings`** do not. Each carries a `message` written to be
read straight off, in the register you are working in:

> *`avalon` has no decision behind it — fine as notes; it stays proposed and*
> *out of canon until you decide.*

A proposed claim is never an error. Being unruled is the normal condition of new
lore, not a fault — which is why `materializable` can be `true` with a page of
warnings still open.

---

## What is actually kept

The line is the view. What the store keeps is one JSON object per claim, and its
content is its name — the same claim, made twice, anywhere, is the same object.
Here is `adjacent(camelot, the-lake)` from the Camelot corpus, exactly as it sits
in the store:

```json fact-object-v1
{
  "schema_version": 2,
  "authority": null,
  "claim_id": "sha256:9d9e29f77df8844ee871ec4f5fd9e81f0646419e4a0ca9d81ffbf66b3703e9b8",
  "atom": { "pred": "adjacent", "args": ["camelot", "the-lake"] },
  "support": { "monomials": [["+5"]] },
  "defeaters": { "monomials": [] },
  "grade": "asserted"
}
```

You do not have to be able to write that. You should be able to see one thing in
it: `support` is where the origin went. The `+5` is a slot in the world's list of
decisions — slot 5 in this corpus is `d-0004` — so the rendered tail `decided
d-0004` and the stored `["+5"]` are two spellings of the same fact. Notice what
is *not* stored: whether the claim is true. Certain, proposed, or ruled out is
worked out from the origin every time it is asked, which is why supposing a
different answer costs nothing (koan 2).

---

## When you outgrow this card

You can work for a long time entirely here and still be using the full
provenance semantics underneath. The rest of the language — the power a writer
reaches for only occasionally — is on the second card. You will not need it to
build a world; you will know when you want it.
