---
status: live
title: "Exports: one-way compiles from the certain layer"
verdict: "one-way compiles from the certain layer; profiles are data; the GDL differential is the external audit."
---

# Exports

The lore fabric feeds audio scripts today and must be able to fill out card
games, board games, and video games of arbitrary size tomorrow. The export layer
delivers that knowledge-level portability: a family of thin compiler backends
that read the compiled world and emit target-shaped data. Exports go one
direction only, out of the certain layer, and are never read back as authority.

## 1. The seat: the missing authoritative layer

The pattern has a commercial proof. **articy:draft X** is a
narrative-production database — a central store of entities, dialogues, quests,
locations, and variables, exported as JSON/XML and consumed by Unity and Unreal
importers with a re-import iteration loop. Studios buy it because neither the
simulation languages nor the flow languages of the fiction lineage hold
*knowledge*; without a database, the world lives in wikis, spreadsheets, and the
lead writer's head. **CastleDB**-style game databases sit in the same seat for
their engines.

De Lore Is occupies that seat with the one difference that is the entire point:
articy's database has no provenance semantics. Its truth is whatever was last
typed — no ratification, no lineage, no assumptions, no "what breaks if this
changes." The fabric is **not a competitor** to such tools; it is the missing
authoritative layer that feeds them, the same relationship CastleDB has to its
engine, upgraded with the provenance machinery those tools lack. That difference
is the product.

## 2. The four-clause contract

`deloreis export <target> --profile <p> [--fork <f>]` compiles the world for a
target. The contract has four clauses:

1. **Source layer.** An export reads the **certain layer by default**; a fork's
   valuation only under an explicit `--fork` flag. Proposed lore never exports by
   accident — it fails closed, exactly as the context-pack builder already
   behaves.
2. **Profile.** A **versioned mapping file** — which sorts, relations, and keys
   the target consumes, and how they render. Profiles are **data, not code
   forks**; adding a target means writing a profile, never touching the kernel.
3. **Emission.** Target-shaped data: JSON, CSV, whatever the consumer eats.
   Exports carry **knowledge** — entities, facts, relations, who-knows-what
   scopes, variant deltas — **never behavior**.
4. **Manifest.** Source commit, layer, valuation, and profile hash. Every
   exported artifact is traceable to the exact canon state that produced it; a
   wrong card is diagnosable to a profile bug or a canon state, never a mystery.

A profile is an ordinary data file — the thing that makes "adding a target is a
profile, never kernel code" literally true. It names the sorts and relations the
target consumes and how each renders; it holds no logic beyond selection and
renaming:

```yaml
# cards/lowlands@3 — a profile is data, not a code fork
target: cards
version: 3
selects:
  sorts: [ground, hand, weather, reckoner]
  relations: [rides-figure, ring-next]
renders:
  hand: {id: "{role}-{ground}", keep: [role, ground, figure]}
  weather: {id: "weather-{name}", keep: [name, ring_next]}
carries_provenance: dev-metadata   # cited D-entries as meta.cited; stripped for ship
```

Every emitted artifact carries a manifest that ties it to the exact canon state
that produced it — the field that turns a wrong card from a mystery into a
diagnosis:

```json
{"compiled_from": {"commit": "3e1320a", "layer": "certain",
  "valuation": "ledger@head", "profile": "cards/lowlands@3"}}
```

**One-way, absolutely.** No export is ever read back as authority. A change
wanted downstream is a change made in lore and re-exported. Two-way sync between
a game database and an authority is how a project acquires two truths; koan 10
(one fabric) refuses it by principle. The write-back refusal is worked in full in
`charter.md` under "The trials" (trial 3): a studio asking for
`deloreis export --write-back` is answered by four koans in one paragraph — the tool's current field value is
not an origin with authority (koan 1), the machine never writes canon (koan 3),
integration is by declaration not by API (koan 10), and if the studio's writers
need in, they are humans using Door H, their edits entering as `proposed`
declarations awaiting a ruling like every claim from outside the sovereign's
hand (koan 11).

## 3. The target family

- **Audio-script context packs** — today's product, recognized as what it
  already is: **export backend number one, in production**. The certain-layer
  default and fail-closed behavior are the pack builder's behavior, generalized.
- **Card and board game content sets** — §6.
- **Video-game data** — articy-shaped JSON: typed entities, relations,
  dialogue-relevant facts with knowledge scopes, and quest-shaped structure. Two
  mappings make this more than a data dump. First, the **keep-stamp visibility
  dimension** — which claims a given faction, region, or character is licensed to
  know — exports directly as **who-may-know-what**, the exact gate a dialogue
  system needs to decide what a speaker can reference. Second, the
  **promise-ledger and debt machinery** map onto quest logs with almost no
  translation: a promise is a hook, an open debt is an objective, a discharge is
  a completion. Provenance citations travel as dev-metadata and are **stripped
  for ship builds**, so the shipped artifact carries the knowledge without the
  authoring apparatus.
- **Free riders** — CSV for producers, a static wiki render for collaborators;
  both fall out of the same compiled world for near-zero cost, because the
  compiled world already holds everything a flat projection needs.

## 4. What we refuse to become

Games-as-data is a mature genre — the Card Game Description Language, generic
JSON card-engine schemas, Ludii's ludeme trees, Stanford's GDL. Their existence
is precisely why the fabric does not grow in that direction: the target shapes
are stable, documented, and someone else's problem to execute. The boundary,
stated exactly: **rules-as-lore live in the kernel; play-runtimes live outside,
fed by exports.**

The Turning engine is in-kernel not because the fabric is a game platform but
because the Turning's rules are ratified canon whose invariants are checked lore
claims (koan 10). A video game consumes exports; it does not call the kernel at
runtime. If a Ludii- or GDL-shaped need ever arrives, it arrives as an export
profile — never as kernel growth.

## 5. The GDL proof point (DL-0007)

One committed exception exists as a proof point. While the fabric will not
become a game description language, the soundness bar is adopted: the dli-hosted
Turning rules must be **trivially and soundly exportable** to a relevant GDL.

A `games/ludii` export profile compiles the Turning's declared ruleset — deck
structure, legal-move relations, terminal conditions, variant deltas — to
Ludii's **L-GDL**, chosen over Stanford GDL because Ludii natively models card
and board games and ships a maintained runtime. Soundness is verified
**differentially**: the `deloreis` simulator and the Ludii runtime replay the
same seeded positions, and their legal-move sets and terminal verdicts must
agree over the battery.

This is koan 12 working **in reverse** — the GDL runtime is a rented baseline
validating the owned engine — and it is the cheapest external audit of the claim
that the Turning's rules-as-lore are really rules:

- If the export is **not trivial**, the rules were entangled with the kernel.
- If it is **not sound**, the simulator was wrong.

Either finding pays for the profile, and both are worth having. The two halves
are separately testable: **triviality** is a property of the profile — a straight
structural compile of declared leaves, relations, and terminal conditions, with
no kernel logic leaking into the mapping — and **soundness** is the trace
agreement over the battery. A profile that needs to *compute* rather than
*render* has smuggled rules out of the fabric and is the finding, not the fix.

Any disagreement over the battery is filed as a finding against the profile or
the kernel. Ludii is a rented baseline (DL-0002's one rented baseline), runs in a
non-gating lane (DL-0013), and the profile lands once the ratified ruleset exists
to export.

## 6. A worked card export

Canon in: the deck anatomy from D-0148 — four Grounds, sixteen Hands, twelve
Weathers in a closed ring, the Reckoner: twenty-nine leaves; region-variable
Ground sets; folk-figures riding Hands by region; and the keep-stamped variant
with the hard Weathers cut. Profile `cards/lowlands`, honest deck. Output,
abridged:

```json
{
  "deck": "turning-lowlands-honest",
  "compiled_from": {"commit": "…", "layer": "certain", "profile": "cards/lowlands@3"},
  "leaf_count": 29,
  "leaves": [
    {"id": "ground-hearth", "layer": "ground", "name": "The Hearth"},
    {"id": "hand-mender-field", "layer": "hand", "role": "mender",
     "ground": "field", "figure": "the-pleacher",
     "meta": {"cited": ["D-0147", "D-0148"]}},
    {"id": "weather-debt", "layer": "weather", "name": "The Debt",
     "ring_next": "weather-fallow"},
    {"id": "reckoner", "layer": "reckoner", "name": "The Reckoner"}
  ],
  "layouts": ["a-round", "the-whole-round", "the-ford", "the-debt-square"]
}
```

Three recompiles of the same profile demonstrate the variant machinery doing
real work:

- The **salt-fringe deck** compiles under the regional delta — the Tide and the
  Catch replacing Grounds, the Net-mender taking the Mender's face.
- The **keep-stamped deck** compiles under the D-0148 variant delta to
  twenty-six leaves, and its `layouts` list loses `the-debt-square`, because the
  reading the licensed decks cannot deal falls out of the compile rather than
  being hand-maintained.

The variant is **lore, not a profile edit** — koan 10's worked export decision.
A profile renders; it never defines content. The keep-stamped deck is a `variant`
declaration with status and citations, and the profile simply compiles it:

```dli
# syntax-pending: grammar unfrozen; AST is normative
# the keep-stamped deck is lore — a variant declaration, not a JSON hand-edit
variant turning/keep-stamped
  cite D-0148
  proposed
  practice the-turning
```

The keep-stamped recompile, abridged, shows the loss falling out of the compile:

```json
{
  "deck": "turning-lowlands-keep-stamped",
  "compiled_from": {"commit": "…", "layer": "certain",
    "profile": "cards/lowlands@3", "variant": "turning/keep-stamped"},
  "leaf_count": 26,
  "layouts": ["a-round", "the-whole-round", "the-ford"]
}
```

`the-debt-square` is absent not because a profile author deleted it but because
the leaves it reads were cut by the variant delta, so the layout no longer
compiles. The diff between the honest and keep-stamped exports is itself a
compiled artifact:

```text
- "leaf_count": 29                          + "leaf_count": 26
- "layouts": [… "the-debt-square"]          + "layouts": [ (no the-debt-square) ]
```

That diff is the allegory, shippable as data — what the licensed decks cannot
deal, computed rather than asserted. The consumer's problems remain the
consumer's — art, layout, rules-text styling; canon's contribution is names,
structure, figure assignments, variant deltas, and provenance. Because every
output carries its manifest (§2), a wrong card in any of the three recompiles is
diagnosable to a profile bug or a canon state, never a mystery.

## 7. What this does not promise

- **No game engine.** No runtime integration beyond data — no shipped game
  queries the kernel.
- **No engine bindings maintained here.** Importers are thin and target-owned.
- **No bidirectional sync, ever.** When a downstream tool's users want changes,
  their edits come home as `proposed` declarations through an authoring door,
  awaiting a ruling like every claim from outside the sovereign's hand.
- **Exports are not canon.** A bad profile is a compiler bug, diagnosable via the
  manifest — never a lore event.
- **Generality is a compiler surface, not an essay.** Every future generality
  claim is a claim about a profile, testable per target. Export targets earn
  maintained profiles only from demonstrated consumers (DL-0002).
