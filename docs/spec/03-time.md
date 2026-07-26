---
status: live
title: "Story-time"
verdict: "production order is the listener's dimension; story-time is the world's."
---

# Story-time

This document specifies how De Lore Is models time. There are two times, and the
whole design turns on keeping them apart: **production order** — the sequence in
which a listener meets the material — is the listener's dimension; **story-time**
— when things were so in the world — is the world's. There remains exactly one
*meta*-time, the ledger, and there is no calendar; story-time is object data with
structure, not a second transaction dimension.

The model has four separated components, a state layer for facts with validity
spans, a disciplined composition with the annotation algebra of
`02-semantics.md`, and a set of capabilities that fall out once the pieces are
separated. It deliberately excludes metric time and the heavy temporal-logic
machinery; §7 says why.

The decision ledger (`../decisions.md`) outranks this document.

---

## 1. The time model: four components, separated

Time is not one axis. Collapsing world chronology, narrative structure, the
narrator's vantage, and cyclic phase into a single ladder is the failure mode
this model exists to prevent. The four are separated by type.

**(i) World chronology — an era spine plus events as points.** A totally-ordered
**era spine**, and **events** that carry era membership. *Within* an era, events
are ordered only by explicit `before(e1, e2)` constraint facts — a **partial
order, honestly partial.** There is no `offset`: a per-event integer fakes a
total order nobody ruled and invites arithmetic on a calendar the world forbids.
Absent a before-fact, two same-era events are **unordered**, and a query answers
`unordered` as a **first-class verdict** — never a silent tie-break. Precedence
is a kernel rule (`happens-before` = era-order lifting composed with intra-era
before-closure), exercised under CI; consistency is cycle detection, and a cycle
is reported as **blame** — the exact before-facts that close the loop.

**(ii) Narrative structure — its own layer.** Acts and arcs are **not eras.**
The opening act, the turn, the world-after are narrative-layer objects (a handful
of acts, a couple hundred episodes; the kishōtenketsu labels attach *here*), each
**referencing** a chronological anchor rather than *being* one. This is precisely
why those act-shaped "eras" sit empty of events in a flattened model — they were
never chronology. When acts are planned in earnest, their macro events (an act's
climax landing, a reveal breaking) enter *chronology* as events, and may,
**by ruling**, initiate genuinely new world eras. Narrative structure references
the spine; it never masquerades as it.

**(iii) Narration-time — a second vantage, inert.** One distinguished point,
`narration-now`, sits inside `the-narrators-era`, centuries out from
story-present. It is **inert by construction** (D-0104): declaring an event in
the narrator's era is a **type error**, no state dynamics exist there, and no
machinery accrues to it. What the vantage buys is the **knowledge asymmetry**:
the narrator is retrospectively omniscient over story-present — their premise
pool is the certain layer as-of `narration-now` — while cast knowledge is
evaluated as-of scene position. Facts whose validity *begins after*
story-present — how the story came to be remembered, a later vindication —
are markable `narration-frame`, and premise selection **structurally excludes**
them from any cast premise pool: a narrators-era fact can never be cast
knowledge. That is the entire apparatus; the ruling forbids more, and the model
agrees with the ruling.

**(iv) Cyclic time — the cycles.** Tide, moon, and season are small **modular
sorts**, orthogonal to the linear spine: `cycle tide(4)`, `cycle moon(29)`, the
festival cycle. A cycle carries **succ, pred, across, distance** and refuses
**`<`, sort, and max** — asking for the "greatest" season or sorting the moon's
nights is a **type error**, not a silent tie-break (koan 8, *the wheel refuses
rank*). The **cycle type lives once in the kernel**, shared by the game engine
(the Turning's `leaf(29)`, one leaf for each night of the moon) and by the timing
checker — canon itself says the same turning that seats a ward runs the Turning's
leaves. Crucially there is **no epoch**: phase coordinates are *local*
declarations, and consistency is checked only as **legal walks along declared
contiguity** — within an episode, or an explicitly declared contiguous span —
never as absolute positions joined across unrelated scenes, which would be a
calendar through the back door
(see [the TPtH world · Turning](../worlds/tpth/turning-export.md) for the worked cycle).

The four edge operations are pinned (ruling M6), so the cycle's laws are stated
rather than accidental:

- **`across` is the half-turn forward step, `floor(n/2)`.** On an **even** cycle
  it is a true antipode and an involution (`across ∘ across = id`). On an **odd**
  cycle there is no exact opposite spoke, so `across` is deliberately
  *non-involutive* — `across ∘ across = pred` — because inventing an antipode the
  ring does not have would be a lie, and the cycle does not lie about its shape.
- **`distance` is directional** — the count of `succ` steps from one position to
  another, with `distance(a,b) + distance(b,a) = n`. The symmetric shorter-arc
  reading is a separate operation, **`separation`** (`= min(distance(a,b),
  distance(b,a))`); the two are named apart so a timing check says which it means.
- **A position index `≥ n` is read modulo `n`** — a documented normalization, not
  an error, because a phase coordinate is a congruence class with no epoch. The
  strict boundary is opt-in: a checked constructor rejects an out-of-range index
  at the point a raw number enters, but a walk already underway never faults.

---

## 2. Fluents: facts acquire validity spans

A fact may carry a span anchored to chronology: `since <event>`, `until
<event>`, `during <era>`, or a combination.

**The migration default makes the entire existing corpus valid unchanged.** An
**unanchored fact means `since era(the-beleaguered-present)`, open-ended** — which
is exactly what the continuity checker already implicitly assumes of every
atemporal row. Adding spans to the model therefore requires **zero edits** to the
current corpus; a fact acquires a span only when a writer has a reason to give it
one.

The semantics is **event-calculus-shaped**: events initiate and terminate
states, and inertia holds a state between its initiator and its terminator.
Because the event order is *partial*, projection is **cautious and
three-valued**:

- **holds-certainly** — an initiator precedes the query point and no possibly-
  intervening terminator does,
- **holds-possibly** — an initiator may precede and a terminator may intervene,
  depending on undeclared order,
- **order-indeterminate** — the answer depends on an order no one has declared.

The water-status case resolves concretely: a fact that the parish water was held
in common *until* the enclosure and enclosed *since* it (D-0114) lets a
pre-enclosure flashback check clean *and* the story-present episode check clean.
The false contradiction a flashback would otherwise raise against the
story-present enclosure fact is gone — not suppressed, but structurally absent,
because the two claims hold over disjoint spans.

---

## 3. Composition with the annotation algebra

Time goes in the atom; provenance stays the annotation; **they must not
entangle.** A state is `atom × span`, and its **support** answers a strictly
separate question: *which authorial events back this claim about that span.*

The two kinds of indeterminacy are different in kind and must stay
distinguishable:

- **Temporal indeterminacy** — unordered events — is **in-world incompleteness**,
  repaired by declaring a `before`-fact. That is an act of world-modeling, owned
  at the continuity surface.
- **Authorial indeterminacy** — open variables — is **meta-level**, repaired by a
  ruling. That is an act of authorship, owned at the ledger.

Collapsing them would make *"the tide during an unordered scene"* indistinguishable
from *"an unratified fact"* — but they call for different repairs, by different
owners, answering different queries. So they **compose factorially**: every
temporal query returns a `(temporal verdict × support verdict)` pair —
*"holds-certainly, provisional (D-0072)"*; *"order-indeterminate under every
valuation."*

And the `before`-facts **themselves carry annotations**, like any fact:
**declaring an order is a ruling.** So the meta-level uniformly governs the object
level without leaking into it. One meta-time is preserved intact; story-time is
object data with structure, never a second transaction dimension over the store.

Because an order is a ruling, **precedence itself is read through standing.** The
raw graph reachability — *is there a path of `before`-facts?* — is standing-blind:
it answers `Before` for a merely-proposed edge exactly as for a ratified one. That
is the wrong answer for any checker that must not let an unratified order harden a
conclusion (the season slack, an anachronism verdict). So the standing-aware read
returns one of three: **certain-order**, when every `before`-fact on the connecting
path is certain; **possible-order**, when a path exists but rests on at least one
proposed edge — it holds only if that ruling lands; and **unordered-under-uncertainty**,
when no path relates the pair under any standing. A merely-proposed `before`-fact
thus yields *possibly-before*, never a hard *before* — the same discipline the
projection of §2 already applies to fluents, now applied to the order relation
that anchors them.

---

## 4. What it unlocks: eight capabilities

The doctrine that organizes all eight: **production order is the listener's
dimension; story-time is the world's.** The episode fold — the threaded,
order-dependent computation that resolves last-writer-wins slots and opens and
closes promises — still runs in **production order**, untouched. What changes is
that folded atoms now carry story-time spans, so **story-time becomes a
projection dimension over the same store.** A flashback episode writes its
*reveals* at its production slot (that is when the listener learns) and its
*facts* at its story anchor (that is when the world was so): two checks, two
dimensions, both on existing machinery.

1. **Episode-to-story-time binding.** The episode schema gains an `at` anchor
   (era, optional after/before event anchors, phase bindings), **distinct from
   `order`**. An unanchored episode follows its predecessor in
   `the-beleaguered-present` — the current behavior, preserved. Scenes may override:
   a flashback scene carries its own anchor.
2. **Facts with spans.** Per §2; the default rule makes migration free.
3. **Who-knew-what-when.** `knowledge` rows become **knowledge states**
   initiated at the episode's story position; query `knows(who, fact) as-of
   <era | event | episode>`. The **listener** is a distinguished knower whose
   ledger runs on **production order** — which is exactly where the staging
   constraints (the full-team floor, the no-deep-time rule, the reveal-placement
   constraints) attach.
4. **Anachronism checking.** An era-anchored episode or scene projects
   `state-at(anchor)` and runs the continuity classes against *that projection*
   instead of the fold head. An `order-indeterminate` result surfaces as its own
   violation class — *"depends on an undeclared order; declare `before(…)` or
   accept"* — which **doubles as a ruling-request generator.**
5. **The partial-order checker over a full season.** Ordering rules are data —
   payoff-requires-prior-reveal, no-deep-time-in-the-opening-act, the full-team
   floor, story-time monotonicity outside declared flashbacks — checked by linear
   scans, with **slack reporting**: the earliest and latest legal slot for each
   episode. Check-first, search-later (the search posture is `08-reasoning.md`,
   tasks T14/T15).
6. **Intra-era precedence, real and kernel-queryable.** `before`-facts are kernel
   facts and `happens-before` is a kernel rule under CI; a downstream toolchain
   emission becomes a **consumer** of this layer instead of its only expression.
7. **Two story-times explicit, narration inert.** The vantage of §1(iii) exists,
   carries the knowledge asymmetry and the `narration-frame` guardrail, and is
   forbidden dynamics **by type** rather than by remembering.
8. **Cyclic time checkable.** A timing premise resolves against the scene's phase
   bindings instead of being taken on faith: a ward requiring the slack tide,
   spoken in a flood-bound scene, is a **checker error**; cycle-walk consistency
   holds within declared contiguity; and phase arc continuity — the moon cannot
   jump mid-episode — comes free from the walk check.

---

## 5. The timeline as a design tool

This is where the model pays the author directly. A factory surface renders,
over the same store:

- an **era-lane timeline** — eras as lanes, events as points with their
  partial-order edges drawn (and unordered pairs *visibly* unordered),
  episode and scene placements as markers, state spans as horizontal bars;
- a **two-track view** — the story-time track over the production-order track,
  in which a flashback is a **visible crossing edge** rather than an invisible
  convention;
- a **who-knows-what matrix** — characters against facts, the listener included,
  at any cursor position;
- **violations in place** — a before-cycle, an anachronism, a staging breach,
  each rendered where it occurs rather than in a separate report.

The **scrubbable cursor is the tool**: drag through story-time and watch
world-state and every character's knowledge change. The subcommands:

- `deloreis timeline show [--at <era | event | episode>]`
- `deloreis timeline order` — the partial-order report: cycles, plus every
  unordered pair. Consumers may correlate these with check findings; the
  chronology report itself does not claim that every pair is load-bearing;
- `deloreis timeline knows --at <pos> [--who <id>]`
- `deloreis timeline episodes --check` — ordering constraints and slack;
  `--propose` is **reserved**, gated on the T15 flip (`08-reasoning.md`);
- `deloreis timeline phases <episode>`

---

## 6. Surface forms

The chronology layer — the era spine, events and their before-facts, the inert
narration vantage, the cycles, and states with spans:

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

era uthers-time < arthurs-reign < after-camlann   decided d-0001
# acts are NOT eras — narrative structure lives in its own layer and
# references chronology; the spine gains eras only by ruling

event the-sword-drawn         during uthers-time     decided d-0002
event the-round-table-founded during arthurs-reign   decided d-0002
event the-grail-quest         during arthurs-reign   decided d-0003
event the-battle-of-camlann   during arthurs-reign   proposed
before the-round-table-founded -> the-battle-of-camlann   decided d-0003
# no `offset`; the-grail-quest shares arthurs-reign with the-round-table-founded
# and no before-fact orders them, so happens-before(x, y) answers `unordered`

narration vantage after-camlann
# inert: declaring an event in this vantage era is a type error [narration-dynamics]

cycle moon(29)                                        decided d-0009
cycle season(4) = spring | summer | autumn | winter   decided d-0009
# a rankless wheel: succ/pred/across/distance exist; `<`, sort, max are type
# errors — no epoch, no calendar

state excalibur.keeper = in-the-stone until the-sword-drawn   decided d-0008
state excalibur.keeper = the-king     since the-sword-drawn   decided d-0008
# an interregnum flashback checks clean AND the reign checks clean — the two
# claims hold over disjoint spans; unanchored facts default to the present era
```

Ordering rules are declarations too — data the season checker reads, not code:

```dli
# grammar: v0 (unfrozen; AST normative)
rule uthers-time-held-back over order   decided d-0009
  : listener-learns(during uthers-time _) requires act > 1
# a floor on when the deepest era may first be shown, held as data the season
# checker reads rather than as code — inert until an episode order binds it
```

An episode binds a production `order` and a story-time `at` anchor separately, and
a scene may carry its own anchor to cross into story-time — a flashback. The
episode schema is defined normatively in `06-craft-and-payloads.md`; the
anchoring it adds is shown here for the two-dimension picture (the anchoring
keywords sit in the episode block, not among the top-level chronology keywords):

```text
episode e12 "The Last Battle" order 12 {
  at after-camlann after e11                # default if omitted: follows prior
  phase season := autumn -> winter          # a walk, checked; not a timestamp
  scene s3 flashback {
    at arthurs-reign after the-round-table-founded before the-battle-of-camlann
    # s3's world checks run against state-at(this anchor), not the fold head;
    # its reveals still land at production slot 12 on the listener ledger
  }
}
```

---

## 7. What stays out, and why

The model is deliberately narrow. Each exclusion answers a specific temptation.

- **Metric time and calendars.** The world has no ratified calendar, and the
  cycles' no-epoch rule keeps the back door shut. Time is qualitative by ruling,
  not by omission.
- **DatalogMTL / temporal Vadalog machinery.** That literature solves *metric*
  temporal reasoning and pays for it in complexity up to EXPSPACE-hardness. This
  time is causal-ordinal by ruling — finitely many named point events,
  qualitative precedence, era membership — so the whole need is plain datalog over
  a finite partial order, free on machinery already built.
- **Full Allen interval algebra.** Thirteen relations and NP-hard general
  consistency buy nothing here: spans are anchored to *named events*, so every
  interval relation reduces to point precedence — the tractable point algebra,
  complete under simple propagation.
- **Event-calculus solvers.** The model takes the event-calculus *shape*
  (initiate / terminate / inertia) and none of the solving: events are
  **declared, never abduced.** No circumscription, no narrative abduction.
- **Bitemporal storage.** There is one meta-time, the ledger; ratification is a
  valuation, not a clock. No second transaction dimension.
- **Narration-time dynamics.** The ticking narrator's clock is rejected, and the
  model makes that rejection **structural**: one inert vantage, a knowledge
  asymmetry, and nothing else, ever.

---

**Worldbook.** The worked TPtH chronology — the elder-cycles→the-narrators-era
spine, the water/enclosure fluent (D-0114), the inert narration frame (D-0104),
and the Turning's leaf(29) — lives in
[the TPtH worldbook · Time](../worlds/tpth/time.md).
