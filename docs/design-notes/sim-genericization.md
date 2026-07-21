---
status: live
title: "Sim genericization: a generic dli-game ruleset, the Turning as data, a 52-card exemplar"
verdict: "Lift the card-deal paradigm into a new engine crate (dli-game) atop dli-sim's rng/manifest substrate; the Turning reduces to worlds/tpth/turning/*.json plus a thin binding (counsel verdict, reciprocity invariant, the study); a standard 52-card French deck ships as the generic sim/export exemplar. Authority: DL-0017, DL-0018 (fork 2's deferred follow-on)."
---

# Sim genericization — the deferred `Ruleset` parameterization

**Authority: [DL-0017](../decisions.md#dl-0017--the-generaltpth-separation-engine-craft-doctrine-config-world-content)
and [DL-0018](../decisions.md#dl-0018--separation-layout-a-shipped-craft-library-a-thin-tpth-crate-a-generic-contract).**
This note elaborates DL-0018 fork 2's explicitly **deferred** follow-on into a
concrete design. The ledger wins if they ever disagree.

DL-0018 fork 2 moved the whole Turning to a thin `crates/tpth/` world crate
([the extraction design](tpth-turning-extraction.md)) and ruled that *"genericizing
the simulator so the whole Turning becomes pure data is a **follow-on**, not a
blocker."* That follow-on is now on the critical path: the author wants a **standard
52-card game** to ship with dli as a generic sim/export exemplar, and that
**requires** this genericization — the Turning extraction moved the entire `Ruleset`
type into `crates/tpth`, so the engine currently has no generic ruleset machinery at
all. This is the design that supplies it, gate-green at every step.

---

## The load-bearing discoveries

Two facts in the current code shape the whole design.

### 1. The Turning uses only `rng` + `manifest` from `dli-sim`

`dli-sim` owns **two** things that are easy to conflate: the *practice-resource*
simulator (`Battery`/`checkpoint` over a `dli_ledger::practice::PracticeDef` — see
`crates/dli-sim/src/battery.rs:26`, `use dli_ledger::practice`) and the world-agnostic
*honesty substrate* (`rng`, `manifest`). The Turning's deal engine
(`tpth::turning::TurningBattery`, `crates/tpth/src/turning.rs:1091`) is a **separate,
parallel** simulator: it reuses `dli_sim::rng::{SplitMix64, stream_value}` and
`dli_sim::manifest::{RunManifest, MetricEstimate, InvariantVerdict, digest_json}` and
**nothing else** — never `Battery`, never `checkpoint`. It re-implements its own
deal/observe/fold loop.

So there are already **two simulator paradigms**: practice-resource (ledger-bound)
and card-deal (ad-hoc, in `tpth`). Genericization is not "extend the practice
battery"; it is **lift the card-deal paradigm into its own engine home**, atop the
shared `rng`/`manifest` substrate the honesty oracle lives in.

### 2. `ProfileSpec.renders` is a generic render DSL the card export ignores

`dli_export::profile::ProfileSpec` (`crates/dli-export/src/profile.rs:113`) already
carries a per-sort render DSL — `renders: BTreeMap<String, RenderRule>` where
`RenderRule { id: "{role}-{ground}", keep: [...] }` is a template plus a keep-list
(`profile.rs:88`). It was built for exactly this: "a profile that needs to *compute*
rather than render has smuggled rules out of the fabric" (spec 09 §5). But
`tpth::export::compile_cards` (`crates/tpth/src/export.rs:202`) **ignores it** and
hardcodes the ground/hand/weather/reckoner layer iteration. Genericizing the card
export means *actually honoring the profile's declared render rules* over a generic
ruleset — the DL-0007 "render, don't compute" bar finally made literal, with no new
machinery invented.

---

## The generic `Ruleset` model

Everything the Turning bakes in reduces to **declared axes → card families → a
positional/facet move relation → declared layouts and invariants**, with only the
world's *verdict predicate* and one *structural invariant* remaining as code. The
model lives in a new `dli-game` crate (see [Where it lives](#where-it-lives-a-new-dli-game-crate)).

```rust
// A named ordered set of values a card family ranges over. Order matters (ranked
// axes: A < 2 < … < K). Replaces the hardcoded HandRole enum + the flat weather list.
struct Axis { id: String, values: Vec<AxisValue> }
struct AxisValue { id: String, name: String, attrs: BTreeMap<String, AttrValue> }
// attrs carry world facts as data: a weather value carries {kind, reciprocal, ring_next, hard}.

// A card family = a product of 0+ axes with an id template. Zero axes = a singleton
// (the hub). Replaces the "reckoner" hub id and the "hand-/weather-/ground-" prefixes.
struct Layer {
    id: String,            // "hand" | "weather" | "reckoner" | "card"
    axes: Vec<String>,     // ["ground","role"] (outer→inner) | ["weather"] | [] | ["suit","rank"]
    id_template: String,   // "hand-{role}-{ground}" | "weather-{weather}" | "reckoner" | "{rank}-of-{suit}"
    dealt: bool,           // false for the hub (the Reckoner is never dealt)
}

struct Card { id: String, layer: String, bindings: BTreeMap<String, String> }

enum Ordering { LayerProduct }  // the wheel: layers in decl order, each in axis-product order

// The follow-predicate — a CLOSED declared enum, the same discipline SIM-1 applies to
// Observable/Invariant/ResourceKind. The opening play is always free; this governs follows().
enum MoveRule {
    CyclicReach { reach: u32 },                              // the Turning wheel (rankless ring)
    Ascending { axis: String, step: Option<u32>, same: Vec<String> }, // French run: same suit, rank+1
    Free,                                                    // pure draw
}

struct Layout { name: String, cards_dealt: u32, requires_cards: Vec<String> }
// the debt-square is Layout { requires_cards: ["weather-debt"], .. }

enum VerdictRule { Completed, Registered { name: String } }  // Turning → Registered { "turning-counsel" }
enum StructuralInvariant {
    DeckIsASet, CountMatches, ClosedRing,                    // generic, built-in
    Registered { name: String },                            // world binding → "reciprocity-closed"
}

struct Ruleset {
    game_id: String, region: String,
    axes: Vec<Axis>, layers: Vec<Layer>, ordering: Ordering,
    moves: MoveRule, layouts: Vec<Layout>,
    verdict: VerdictRule, invariants: Vec<StructuralInvariant>,
    decorations: Vec<Decoration>,   // figures, generalized (see below)
    provenance: Provenance,         // Standing / cites — lifted from turning.rs unchanged
}

impl Ruleset {
    fn from_rules(json: &str) -> Result<Ruleset, _>;  // the north star: load worlds/tpth/turning/*.json
    fn cards(&self) -> Vec<Card>;                     // enumerate all layer products (the deck)
    fn leaves(&self) -> Vec<Card>;                    // cards() in Ordering order (the wheel)
    fn follows(&self, prev: &str, next: &str) -> bool;// dispatch on MoveRule
    fn legal_moves(&self, r: &Reading) -> Vec<String>;
    fn is_terminal(&self, r: &Reading) -> bool;
    fn verdict(&self, r: &Reading) -> Verdict;        // generic, or Registered → world predicate
}
```

Move-relation dispatch is the seam: `CyclicReach` reads `Ordering` positions (the
wheel); `Ascending` reads axis attrs (rank/suit). Both are pure
`(ruleset, prev, next) -> bool`. A genuinely novel relation is a new enum arm — a
`dli-game` PR, never world data. That is the honest closed-set boundary SIM-1 already
draws for `Observable`/`Invariant`/`ResourceKind`, and its explicit purpose here is to
**refuse the reinvent-Ludii-in-the-ruleset trap**: Ludii L-GDL is the general
game-description language dli *exports to*, not one to re-implement inside `Ruleset`.

**Figures** (`Figure`/`FigureSeat`, `crates/tpth/src/turning.rs:216`) generalize to a
`decorations` table (a card selector → attribute overlay); the card export's
`keep: ["figure"]` reads it; a French deck's table is empty. **Variants** lift generic
unchanged in shape (`Variant`/`DeltaOp`/`fork`/`ratify`, `turning.rs:816`), with the
ops neutralized: `CutWeather` → `CutAxisValue { axis, value }` (drops the bound cards
and any layout that required them), `RenameGround` → `RenameAxisValue`, `RefaceHand` →
a generic `SetDecoration`.

The **deal battery is fully generic — no binding**. `TurningBattery` /
`ReadingObservable` (`turning.rs:995`) / `ReadingInvariant` (`turning.rs:1030`) are
already generic in shape; rename leaf/reading → card and make `ContradictionRate` the
generic `VerdictIs { "contradiction" }` (the verdict is a declared string now). The
`deal`/`run`/`RunManifest`-folding lifts verbatim into `dli-game`.

### The invariant count-reduction proof

The clean evidence that the model is the right abstraction is what happens to the
Turning's four `StructuralInvariant`s (`crates/tpth/src/turning.rs:864`):

| Turning invariant | Fate under the generic model |
|---|---|
| `DeckIsASet` | **generic built-in** — already world-agnostic (every card id unique). |
| `ClosedRing` | **generic built-in** — the ordering is one cycle; declared only for cyclic games. |
| `MoonCount` | **collapses into the generic `CountMatches`.** The `grounds·4 + weathers + 1` arithmetic *is*, generically, "the declared count (Σ of layer-product sizes) equals the distinct card ids." No world binding. |
| `ReciprocityClosed` | **the one world binding** — it reads the `reciprocal` attr, genuinely Turning-specific. |

Four invariants → three generic + one thin binding. Invariants are a **declared
list** on the ruleset (loaded from data), so the Turning declares
`[DeckIsASet, ClosedRing, CountMatches, Registered{"reciprocity-closed"}]` and a
French deck declares `[DeckIsASet, CountMatches]`.

---

## Where it lives: a new `crates/dli-game` crate

**Recommendation: a new `dli-game` engine crate, not a `dli-sim` extension.** The
card-deal paradigm is distinct from the resource-practice paradigm `dli-sim` owns
(and which drags in `dli-ledger`); merging them muddies SIM-1. `dli-game` reuses the
shared substrate by *depending on* `dli-sim` for `rng` + `manifest` only.

```
dli-core                                    (Grade)
dli-sim   → dli-ledger, dli-core            SIM-1 unchanged (practice battery + rng + manifest)
dli-game  → dli-sim, dli-export, dli-core   NEW: generic Ruleset, deck, moves, deal battery,
                                                  generic card + Ludii export (honors ProfileSpec.renders)
crates/tpth → dli-game, dli-sim, dli-core   the Turning: data + a thin binding
```

The dependency graph is acyclic and boundary-honest: **no engine crate depends on
`tpth`** (verified — the only `tpth` mentions in engine crates are doc-comment
strings in `dli-serve`/`dli-world`). `dli-game/src` is scanned by the boundary guard
(`xtask/src/boundary.rs`, whose `WORLD_TOKENS` includes `reckoner`, `weather-debt`,
`the-turning`, `lowlands`), so it **must** be TPtH-token-free — which is precisely the
proof genericization succeeded: the crate speaks only neutral vocabulary (axis, layer,
card, hub, `CyclicReach`).

**The generic card/Ludii render moves into `dli-game::export`**, generalized from
`tpth::export` to drive `ProfileSpec.renders`/`selects` over the generic ruleset's
declared cards and facets. It depends on `dli_export::profile` for
`ProfileSpec`/`Registry`/`ExportTarget`/`ExportOpts`/`ExportManifest`/`ExportError`.
`dli-export` stays world-atom-only (context-pack / articy over `CompiledWorld`); its
`ExportError::NoEngineRendererForTarget` for `Cards`/`LudiiGdl` can now be genuinely
*satisfied* by `dli-game` rather than fail closed. (Alternative, if the
`dli-game → dli-export` edge is judged wrong: split the profile types into a tiny
`dli-export-core`. Not recommended — scope creep for one clean edge.)

---

## The Turning as an instance: data + a thin binding

After migration `crates/tpth` is a few hundred lines, not 1470.

**Data** (`worlds/tpth/turning/`): `the-turning-rules.json` rewritten in the generic
schema —

- `axes: [ ground, role, weather ]` (role carries the four Frith values; weather
  values carry `{kind, reciprocal, ring_next, hard}` attrs);
- `layers: [ hand(ground×role) "hand-{role}-{ground}", weather(weather)
  "weather-{weather}", reckoner() "reckoner" dealt:false ]`;
- `ordering: LayerProduct`, `moves: CyclicReach { reach: 6 }`;
- `layouts` including the debt-square with `requires_cards: ["weather-debt"]`;
- `invariants: [DeckIsASet, ClosedRing, CountMatches, Registered{"reciprocity-closed"}]`;
- `verdict: Registered { "turning-counsel" }`; `provenance` (ratified, `[D-0147, D-0148]`).

Plus `variants/keep-stamped.json` and `variants/salt-fringe.json` as generic
`DeltaOp` lists. **Card ids stay byte-identical** (`hand-mender-field`, `weather-debt`,
`reckoner`) by choosing id templates that reproduce them — the migration invariant.

**Thin Rust binding** (`crates/tpth/src/turning.rs`): the two genuinely-code
predicates plus the study —

- `turning_counsel_verdict(ruleset, reading) -> Verdict` (reciprocal-pair →
  `Contradiction`, filled → `Question`, unfilled → `MissingPremise`), registered under
  `"turning-counsel"`;
- `reciprocity_closed(ruleset) -> Option<breach>`, registered under
  `"reciprocity-closed"`;
- a `turning_registry()` that hands `dli-game` those two named predicates;
- `honest_vs_rigged(seed, trials) -> StudyReport` — composes two generic deal
  batteries over the loaded honest and keep-stamped rulesets;
- convenience ctors `Ruleset::the_turning() / keep_stamped() / salt_fringe()` = load
  JSON + apply the variant delta + ratify.

What genuinely cannot be data: the two registered predicates and the study.
Everything structural is data. This is the honest floor of "the Turning as pure data"
— see [hardest call 1](#the-hardest-calls).

---

## The standard 52-card exemplar

Ships at `examples/games/french-deck/{rules.json,profiles.json}` (parallel to
`worlds/tpth/turning/`; `examples/` today holds only `camelot.dli`, a lore world). A
generic `Ruleset` instance:

- `axes: [ suit{♠♥♦♣}, rank{A,2..10,J,Q,K} ]`;
  `layers: [ card(suit×rank) "{rank}-of-{suit}" ]` → 52 distinct cards;
- `moves: Ascending { axis: "rank", step: Some(1), same: ["suit"] }` — a straight-flush
  run: the next card is the same suit, one rank up. This is the **same deal loop** as
  the Turning (opening free → follow the last-laid → no repeats → terminal when filled
  or jammed), so it exercises `legal_moves`/`is_terminal`/`verdict` on the identical
  code path;
- `layouts: [ "a-run" deals 5, "the-full-suit" deals 13 ]`;
  `invariants: [DeckIsASet, CountMatches]`; `verdict: Completed` (generic — no binding);
- exercises: delta-sensitive legal-move enumeration, terminals, generic invariants, a
  seeded deal battery → `RunManifest`, and **both** exports (card render
  `{rank}-of-{suit}`; Ludii render of 52 components + the same-suit rank+1 successor
  edges + terminal layouts).

**Why a French deck specifically:** it has a *total rank order*; the Turning wheel is
*rankless*. One engine spanning `Ascending` (linear, ranked) **and** `CyclicReach`
(cyclic, rankless) is the strongest possible genericity proof — the Turning becomes
"one instance of the generic thing." It is single-actor (no players, hands, or
scoring), which matches the engine's single-deck-deal model as it stands; a
trick-taking or shedding game would need a later player/hand layer (out of scope here,
noted as a future extension). A **tarot** ruleset (Marseille / Rider-Waite, public
domain) is a trivial *second* instance — the same shape as the Turning — if an
oracular exemplar is also wanted.

**The rules are uncopyrightable.** Game mechanics and rules are not subject to
copyright (the Baker v. Selden line; only specific creative expression — text, art,
unique names — is protectable). A standard 52-card French deck and generic draw/run
rules are firmly public domain.

---

## The ordered implementation DAG

Each step lands gate-green (`cargo fmt --check`, `cargo clippy`, `cargo nextest`,
`cargo xtask gate` including the boundary guard). Two steps are real work; the rest
are mechanical or additive.

- **Step 0 — the skeleton (mechanical).** Create `crates/dli-game` (deps: `dli-sim`,
  `dli-export`, `dli-core`, serde/serde_json/schemars) with a crate-doc-only
  `lib.rs`. Workspace `members = ["crates/*"]` auto-includes it. Green: the empty
  crate builds.

- **Step 1 — the generic engine (THE design step; a pure add).** Implement the model,
  the generic card + Ludii export driven by `ProfileSpec.renders`, and the
  `Registered` verdict/invariant registry seam in `dli-game`; unit-test it against a
  tiny synthetic ruleset. It touches no existing code, so it **cannot** break the
  Turning. This is the one genuinely new-design step.

- **Step 2 — migrate the Turning to data + binding (the second real refactor;
  `crates/tpth` only).** Write the new generic `the-turning-rules.json`; rewrite
  `tpth/src/turning.rs` to load it, register `turning-counsel` + `reciprocity-closed`,
  and keep `honest_vs_rigged`; reduce `tpth/src/export.rs` to a thin `run` over
  `dli_game::export::run`; update `tpth/tests/{turning,export}.rs` to assert the **same
  facts** (29/26 leaf counts, debt-square foreclosure, `hand-mender-field` → the
  Pleacher, `weather-debt` ring_next `weather-fallow`, the honest-vs-rigged difference,
  the Ludii render containing every leaf) through the generic path. Isolated to
  `crates/tpth`, so the rest of the workspace stays green. **Guard the migration** with
  a golden asserting the generic `leaves()` id-list equals the pre-migration list
  *before* deleting the old code, then delete the ~1200 lines of hardcoded
  vocabulary/data/invariants.

- **Step 3 — the 52-card exemplar (additive).** Author `examples/games/french-deck/*`;
  add a `dli-game` test that loads it and drives legal moves, terminals, generic
  invariants, a deal battery, and the card + Ludii render. Pure add.

- **Step 4 — repoint the CLI (mechanical, optional).** With `Cards`/`LudiiGdl` now
  having a generic engine renderer, wire the honest "not yet" `deloreis simulate` /
  `export` stubs (`crates/deloreis/src/cmd/drafts.rs`) to `dli-game`. Optional.

- **Step 5 — docs (docs-as-you-go).** Flip the Phase-4 follow-on row in
  [`tpth-separation-plan.md`](tpth-separation-plan.md) to done; cross-link this note.
  Docs-only, green.

**Risk.** The Turning's *observable behavior* must stay green through Step 2 — the only
risky step. Digests are safe to change: no committed golden pins a Turning
battery/manifest digest (the Ludii golden
`contracts/dli-worker/fixtures/golden/export-ludii-v1.json` carries
`expected_outcome: "gated"` and never dispatches to a renderer; the attestation-sweep
golden is turning-free), and the Turning tests assert only determinism and
equality *within* a run (`same_seed_same_manifest`, `the_study_is_deterministic`). The
migration's real hazard is card-id or wheel-order drift, which the pre-delete golden
catches.

---

## The hardest calls

1. **How far to push "Turning as pure data."** *Recommended, adopted:* structure +
   moves + layouts + generic invariants = data; the counsel **verdict**, the
   **reciprocity invariant**, and the **study** stay as thin registered code in
   `crates/tpth`. Do **not** build a declared predicate mini-language for verdicts —
   that is the reinvent-Ludii trap, and the world-authored predicate would still be
   code-shaped. Fork for the author: accept the thin binding (recommended) vs. invest
   later in a verdict DSL.

2. **`dli-sim` extension vs. a new `dli-game` crate.** *Recommended, adopted:* a new
   `dli-game` crate — the card-deal paradigm is distinct from the ledger-bound
   resource-practice paradigm; keeping them separate keeps SIM-1 clean, and `dli-game`
   reuses `rng`/`manifest` by dependency. Fork: a fewer-crates `dli-sim` extension vs.
   the paradigm-clean new crate (recommended).

3. **Move-relation breadth.** *Recommended, adopted:* a closed declared enum
   (`CyclicReach` / `Ascending` / `Free`), extensible only by an engine PR — matching
   SIM-1's closed-enum discipline. Fork: the closed enum (recommended) vs. an open
   predicate DSL (over-engineering).

4. **Render location and dependency direction.** *Recommended, adopted:* `dli-game`
   owns the generic card + Ludii render and depends on `dli_export::profile`;
   `dli-export` stays world-atom-only. Minor fork: that edge vs. a `dli-export-core`
   split for the profile types (not recommended).
