---
status: live
title: "Exports: one-way compiles from the certain layer"
verdict: "One-way, profile-driven compiles from the certain layer to context packs, cards, Ludii, articy JSON, CSV, and static Markdown wikis; the live GDL differential remains external."
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

`deloreis export --profile <target> --corpus <path> [--profiles <catalog>]
[--profile-name <name@version>]` compiles a lore world for a target. Card and
Ludii exports use `--game` in place of `--corpus`. The contract has four
clauses:

1. **Source layer.** An export reads the **certain layer by default**; a fork's
   valuation only when a library caller supplies an explicit valuation. The
   shipped CLI and worker expose certain-layer exports only; neither accepts a
   fork selector. Proposed lore never exports by accident — it fails closed,
   exactly as the context-pack builder already behaves.
2. **Profile.** A **versioned mapping file** — which sorts, relations, and keys
   the target consumes, and how they render. Profiles are **data, not code
   forks**; adding a mapping for an existing target means writing a profile,
   never touching the kernel. A genuinely new emission format still requires an
   explicit target implementation and wire-contract migration.
3. **Emission.** Target-shaped data: JSON, CSV, whatever the consumer eats.
   Exports carry **knowledge** — entities, facts, relations, who-knows-what
   scopes, variant deltas — **never behavior**.
4. **Manifest.** Source revision or content fingerprint, layer, valuation, and
   profile hash. The field remains named `commit` on the pre-v1 wire: the CLI
   uses an explicit `--commit` when supplied and otherwise derives
   `content-sha256:<hex>` from the loaded source-byte set; the project worker,
   which never reads git, always derives that content fingerprint. Every
   exported artifact is traceable to the exact canon state that produced it; a
   wrong card is diagnosable to a profile bug or a canon state, never a mystery.

A profile is an ordinary data file — the thing that makes adding a mapping for
an existing target data rather than a code branch. It names the sorts and
relations the target consumes and how each renders; it holds no logic beyond
selection and renaming:

```yaml
# cards/french-deck@1 — a profile is data, not a code fork
target: cards
version: 1
selects:
  sorts: [card]
  relations: [run-successor]
renders:
  card: {id: "{rank}-of-{suit}", keep: [rank, suit, name]}
carries_provenance: dev-metadata   # cited decisions as meta.cited; stripped for ship
```

Catalogs are validated before rendering. Profile names are versioned and unique;
unknown object fields, duplicate selectors, duplicate identities, target
mismatches, unknown kept fields, and unavailable template placeholders fail
closed. Empty `sorts` and `relations` together mean all certain predicates; once
either list is non-empty, their union is exact. Generic flat renderers name
positional fields `arg0`, `arg1`, and so on. A caller catalog overrides a
matching neutral built-in by qualified identity, and an explicit
`--profile-name` removes default-selection ambiguity.

Game rulesets declare the sort and relation names their profiles may select.
Their axes and card layers form the sort namespace; their `relations` array is
the relation namespace. Card and Ludii profiles are validated against that
concrete namespace and the card fields available to each render rule. Explicit
sort selection emits only selected card layers, missing fields and rendered-id
collisions fail, and Ludii emits legal moves or terminals only when those
relations are selected. A proposed ruleset is not certain-layer knowledge and
cannot export. A lore-world `fork` valuation is not a game delta and is refused
by the game library; callers materialize and ratify a ruleset variant first.
Generic lore renderers likewise refuse `region` and `variant` selectors because
they do not materialize world deltas. A world adapter may consume known selectors
into a concrete ruleset before generic export; unknown and incompatible values
fail, and consumed selectors are cleared rather than hashed as if they had an
additional artifact effect.

An export without a non-empty source revision or content fingerprint is refused.
The project worker fingerprints the same source bytes it parsed, so a concurrent
filesystem change cannot relabel an already compiled artifact. Target-specific
projections are total over selected atoms: unsupported arity, a non-ground
argument, or a field the target cannot preserve is an error, never a dropped
row. Provenance mode changes metadata only; stripped and development exports
contain the same selected knowledge.

Every emitted artifact carries a manifest that ties it to the exact canon state
that produced it — the field that turns a wrong card from a mystery into a
diagnosis:

```json
{
  "artifact": {"profile": "cards/french-deck@1", "layer": "certain"},
  "manifest": {
    "commit": "content-sha256:…",
    "layer": "certain",
    "valuation": "sha256:…",
    "profile_hash": "sha256:…"
  }
}
```

**One-way, absolutely.** No export is ever read back as authority. A change
wanted downstream is a change made in lore and re-exported. Two-way sync between
a game database and an authority is how a project acquires two truths; koan 10
(one fabric) refuses it by principle. The write-back refusal is worked in full in
`charter.md` under "The trials" (trial 3): a studio asking for
`deloreis export --write-back` is answered by four koans in one paragraph — the tool's current field value is
not an origin with authority (koan 1), the engine cannot decide the studio's
authorization policy (koan 3), integration is by declaration not by API (koan
10), and if the studio's writers need in, they come through the same tool
surface with the attribution and review process their application requires
(koan 11).

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
- **Free riders** — RFC 4180 CSV for producers and a static Markdown wiki bundle
  for collaborators. Both use the same sorted flat projection of selected
  certain-layer atoms. CSV carries one row per atom with rendered id, kind,
  predicate, retained positional arguments, canonical claim, and optional
  citations; cells beginning with spreadsheet formula sigils are prefixed with
  an apostrophe. The wiki emits `index.md` plus one HTML-escaped,
  path-component-encoded page per predicate. A `stripped` profile removes
  citations from both.

## 4. What we refuse to become

Games-as-data is a mature genre — the Card Game Description Language, generic
JSON card-engine schemas, Ludii's ludeme trees, Stanford's GDL. Their existence
is precisely why the fabric does not grow in that direction: the target shapes
are stable, documented, and someone else's problem to execute. The boundary,
stated exactly: **rules-as-lore live in the kernel; play-runtimes live outside,
fed by exports.**

The french-deck engine is in-kernel not because the fabric is a game platform but
because its rules are ratified canon whose invariants are checked lore claims
(koan 10). A video game consumes exports; it does not call the kernel at runtime.
If a Ludii- or GDL-shaped need ever arrives, it arrives as an export profile —
never as kernel growth.

## 5. The GDL proof point (DL-0007)

One committed exception exists as a proof point. While the fabric will not
become a game description language, the soundness bar is adopted: the dli-hosted
french-deck rules must be **trivially and soundly exportable** to a relevant GDL.

A `games/french-deck-ludii` export profile compiles the french-deck's declared
ruleset — deck structure, legal-move relations, terminal conditions, variant
deltas — to Ludii's **L-GDL**, chosen over Stanford GDL because Ludii natively
models card
and board games and ships a maintained runtime. Soundness is verified
**differentially**: the `deloreis` simulator and the Ludii runtime replay the
same seeded positions, and their legal-move sets and terminal verdicts must
agree over the battery.

Every ruleset string emitted inside an L-GDL quoted literal is escaped by the
renderer: quotes, backslashes, line separators, tabs, and control characters
cannot terminate a component, move endpoint, game id, or layout name and inject
a ludeme.

This is koan 12 working **in reverse**: the GDL runtime is a rented baseline
validating the owned engine and its rules-as-lore:

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

Canon in: the deck anatomy from the french-deck ruleset — four suits, thirteen
ranks, fifty-two cards; the run-successor ordering; and the two dealing layouts (a
five-card run and the full thirteen-card suit). Profile `cards/french-deck`,
standard deck. Output, abridged:

```json
{
  "artifact": {
    "deck": "french-deck",
    "layer": "certain",
    "profile": "cards/french-deck@1",
    "cited": ["DL-0017", "DL-0018"],
    "card_count": 52,
    "cards": [
      {"id": "ace-of-spades", "layer": "card",
       "kept": {"suit": "spades", "rank": "ace", "name": "Ace"}}
    ],
    "layouts": ["a-run", "the-full-suit"]
  },
  "manifest": {
    "commit": "content-sha256:…",
    "layer": "certain",
    "valuation": "fnv1a64:…",
    "profile_hash": "fnv1a64:…"
  }
}
```

A recompile of the same profile demonstrates the variant machinery doing real
work:

- The **piquet deck** compiles under the low-card delta — the twos through sixes
  cut from the ruleset — to thirty-two cards, and its `layouts` list loses
  `the-full-suit`, because the thirteen-card layout the shortened deck cannot deal
  falls out of the compile rather than being hand-maintained.

The variant is **lore, not a profile edit** — koan 10's worked export decision.
A profile renders; it never defines content. The piquet deck begins as a
`variant` declaration with status and citations:

```dli
# grammar: v0 (unfrozen; AST normative)
# the piquet deck is lore — a variant declaration, not a JSON hand-edit
variant french-deck/piquet
  cite d-0018
  proposed
  practice french-deck
```

That declaration is proposed and therefore cannot be exported as certain-layer
knowledge. Once authorially ratified, the caller materializes the resulting
ruleset and passes that concrete ruleset to the same generic profile. The
recompile, abridged, shows the loss falling out of the compile:

```json
{
  "deck": "french-deck-piquet",
  "layer": "certain",
  "profile": "cards/french-deck@1",
  "card_count": 32,
  "layouts": ["a-run"]
}
```

`the-full-suit` is absent not because a profile author deleted it but because the
cards it reads were cut by the variant delta, so the layout no longer compiles.
The diff between the standard and piquet exports is itself a compiled artifact:

```text
- "card_count": 52                          + "card_count": 32
- "layouts": [… "the-full-suit"]            + "layouts": [ (no the-full-suit) ]
```

That diff is the variant machinery, shippable as data — what the shortened deck
cannot deal, computed rather than asserted. The consumer's problems remain the
consumer's — art, layout, rules-text styling; canon's contribution is names,
structure, variant deltas, and provenance. Because every output carries its
manifest (§2), a wrong card in either recompile is diagnosable to a profile bug or
a canon state, never a mystery.

## 7. What this does not promise

- **No game engine.** No runtime integration beyond data — no shipped game
  queries the kernel.
- **No engine bindings maintained here.** Importers are thin and target-owned.
- **No bidirectional sync, ever.** When a downstream tool's users want changes,
  they enter through the authoring interface with application-supplied
  attribution and decision state; DLI never treats an export as authoritative.
- **Exports are not canon.** A bad profile is a compiler bug, diagnosable via the
  manifest — never a lore event.
- **Generality is a compiler surface, not an essay.** Every future generality
  claim is a claim about a profile, testable per target. Export targets earn
  maintained profiles only from demonstrated consumers (DL-0002).
