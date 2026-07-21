---
status: live
title: "TPtH worldbook — The Turning export"
verdict: "The worked Turning card export behind spec 09: the deck anatomy (D-0148), the cards/lowlands profile, the honest / salt-fringe / keep-stamped recompiles, the-debt-square falling out of the variant delta, and the Ludii differential."
---

# The Turning export

> **Worldbook.** This chapter faces [spec 09 · Exports](../../spec/09-exports.md)
> and documents the Turning ruleset at
> [`worlds/tpth/turning/the-turning-rules.json`](../../../worlds/tpth/turning/the-turning-rules.json)
> and its profiles at
> [`worlds/tpth/export/profiles.json`](../../../worlds/tpth/export/profiles.json).
> The spec teaches the export contract on the shipped
> [`examples/games/french-deck/`](../../../examples/games/french-deck/) and its
> piquet variant; the worked Turning export it used to carry lives here. The
> Turning's `leaf(29)` is the same cyclic kind as the moon in
> [Time](time.md).

The Turning is TPtH's in-kernel game — a rankless wheel of twenty-nine leaves —
and its export to a card content set is the worldbook's worked demonstration that
the variant machinery does real allegorical work.

## The profile

A profile is data, not a code fork — the cards/lowlands profile that renders the
honest deck:

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

Every emitted artifact carries a manifest tying it to the exact canon state that
produced it:

```json
{"compiled_from": {"commit": "3e1320a", "layer": "certain",
  "valuation": "ledger@head", "profile": "cards/lowlands@3"}}
```

## The worked card export

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

Three recompiles of the same profile demonstrate the variant machinery doing real
work:

- The **salt-fringe deck** compiles under the regional delta — the Tide and the
  Catch replacing Grounds, the Net-mender taking the Mender's face.
- The **keep-stamped deck** compiles under the D-0148 variant delta to
  twenty-six leaves, and its `layouts` list loses `the-debt-square`, because the
  reading the licensed decks cannot deal falls out of the compile rather than
  being hand-maintained.

The variant is **lore, not a profile edit** — koan 10's worked export decision. A
profile renders; it never defines content. The keep-stamped deck is a `variant`
declaration with status and citations, and the profile simply compiles it:

```dli
# grammar: v0 (unfrozen; AST normative)
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
deal, computed rather than asserted.

## The Ludii differential

The dli-hosted Turning rules must be trivially and soundly exportable to a
relevant GDL. A `games/ludii` export profile compiles the Turning's declared
ruleset — deck structure, legal-move relations, terminal conditions, variant
deltas — to Ludii's L-GDL, and soundness is verified differentially: the
`deloreis` simulator and the Ludii runtime replay the same seeded positions, and
their legal-move sets and terminal verdicts must agree over the battery. Ludii is
a rented baseline (DL-0002's one rented baseline) validating the owned Turning
engine — the cheapest external audit of the claim that the Turning's rules-as-lore
are really rules.
