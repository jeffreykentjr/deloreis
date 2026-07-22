---
status: live
title: "The general/TPtH separation: leak inventory and migration DAG"
verdict: "dli is the generic engine; craft is togglable project config; TPtH is one world's data. This is the ordered plan to tease them apart, gate-green at every step. Authority: DL-0017."
---

# The general / TPtH separation

> ## ✅ COMPLETE (2026-07-21)
> The separation is done end to end. The engine (`dli-*` + `dli-game` +
> `deloreis`) is **provably story-agnostic** — the boundary CI guard passes with
> an **empty allowlist** (zero TPtH tokens in engine real code; the build fails if
> that regresses). TPtH is cleanly modularized as **`crates/tpth`** (a thin world
> crate) + **`worlds/tpth/`** (all its data: packs, turning, craft config,
> practices, episodes, vendor, mutation). Two public-domain teaching worlds ship
> (`examples/camelot.dli` Arthurian; `examples/games/french-deck/` a 52-card
> game), and the docs teach the tool generically with TPtH as a `docs/worlds/tpth/`
> worldbook. **Phases 0–5 all landed, every step gate-green.** What remains is
> optional polish (the `expert-card.md` genericization; wiring the `simulate`/
> `export` CLI stubs; a `Ruleset::cards()` memoization perf).

**Authority: [DL-0017](../decisions.md).** This note elaborates that ruling into
a concrete leak inventory and an ordered migration DAG. The ledger wins if they
ever disagree.

De Lore Is ships as a general-purpose lore tool. "To Pleach the Hedgeway" (TPtH)
is **one** world authored in it. The repo separates into **three tiers**:

| Tier | What | Where it lives |
|---|---|---|
| **Engine** | Generic mechanism only — algebra, loader/checker/forks, time, the practice *workbench*, the naming *oracle*, the season-rule *engine* + KB loader, the simulator, export core, server + Door T. Takes world content and craft config **as data**. | `crates/dli-*`, `crates/deloreis` |
| **Craft doctrine** | Togglable module library of generic craft wisdom (Sanderson's laws, kishōtenketsu, MICE, tryfail-before-triumph) as principle-record packs a **project** turns on/off. Not a world. | a craft-doctrine home (see open fork A) |
| **World content** | One fictional world's lore. For TPtH: the Turning, the Concordance / spoken-proof practice, cymric-fringe + names, the saltmarket corpus, banned-vocab rationale, the `D-nnnn` ledger. Loaded as data. | `worlds/tpth/` (+ a thin TPtH crate only where content is not yet pure data) |

**A project = a world + selected craft-doctrine modules + config.** The
**factory** (the consuming repo) maps a project's `D-nnnn` rulings → craft-KB
records; the engine only *loads* the resulting config. The engine never reads
`canon/decisions.md`.

**The boundary invariant.** No engine crate `src/` may hardcode a world's proper
nouns, a project's craft doctrine, or a consumer's `D-nnnn` ids outside a
doc-example explicitly marked as illustrative. This becomes a gate check
(§ *The boundary guard*).

---

## Target layout

```
crates/                     # ENGINE ONLY (generic mechanism)
  dli-ast dli-core dli-eval dli-syntax dli-world dli-ledger
  dli-time dli-sim dli-export dli-serve deloreis xtask
  (+ a genericized ruleset core: dli-sim keeps the PATTERN; the Turning leaves)

craft-doctrines/            # TIER 2 — togglable, world-independent packs (open fork A)
  sanderson-laws.dli  kishotenketsu.dli  mice-nesting.dli  audio-drama.dli ...
  (generic principle records; a project selects a subset. The engine loads them.)

worlds/tpth/                # TIER 3 — TPtH world content, as data
  packs/    saltmarket.dli cymric-fringe.dli true-naming.dli second-practice.dli
  turning/  lowlands.json (+ keep-stamped / salt-fringe variant deltas)
  episodes/ saltmarket-episodes.yaml  season/ saltmarket-season.yaml
  craft/    TPtH's factory-produced craft config (principles.yaml, thresholds.yaml,
            ten-battle-lexicon.yaml, material-categories.yaml) — PRODUCED by the factory
  vendor/   the pinned fantasy snapshot (fixtures/vendor/*) as provenance/regression
  mutation/ the detection corpus
  contract/ the TPtH-payload conformance fixtures (goldens carrying saltmarket/mara/…)
  (+ crates/tpth/ — thin world code only where not yet pure data: Turning invariants/study)

examples/                   # ENGINE — a NEW neutral generic teaching world (hello-world.dli)
contracts/dli-worker/       # ENGINE — the generic wire contract (renamed from fantasy-worker)
docs/                       # ENGINE MANUAL (generic) + a TPtH worldbook companion + integration/
```

---

## Leak inventory

Grouped by where each artifact belongs. Source: the 8-region separation audit
(2026-07-20). **HARD** = TPtH content baked into engine code (an extraction
target). **SOFT** = doc/test illustration (genericize on release, non-blocking).

### A. World content hardcoded in engine code (HARD)

> **Status:** the **Turning** rows (deck, type vocabulary, study, invariants,
> export-backend) are ✅ **RESOLVED** — Phase 4 (`a65077a`) moved the whole
> Turning to `crates/tpth/`. The **profile-registry** row is ✅ done (Phase 2,
> `b5dd94b`). Still ⏳: the saltmarket world built in Rust (`loader.rs::example_docs`)
> and `session.rs::builtin_pack` — folded into the PD-example-worlds effort.

| Leak | Location | Move |
|---|---|---|
| The entire lowlands Turning deck as Rust | `dli-sim/src/turning.rs` `lowlands_grounds/canonical_weathers/lowlands_figures/canonical_layouts`, `Ruleset::{lowlands_honest,keep_stamped,salt_fringe}`, `Variant::*` | → `worlds/tpth/turning/*.json` (the types already derive Serde+JsonSchema; `examples/the-turning-rules.json` is already the serialization). Load via a generic `Ruleset::from_rules`. |
| Turning-shaped **type vocabulary** | `dli-sim/src/turning.rs` `HandRole` (Taker/Keeper/Giver/Mender enum + `·4` arithmetic), `Weather/Ground/Figure/Leaf/Layer/Layout`, the `"reckoner"` hub id, `"weather-"` prefix, moon-count `grounds·4+weathers+1` | Genericize: declared roles/suits/conditions/hub as data on a parameterized `Ruleset`; or move the whole vocabulary to `crates/tpth/`. (Open fork B.) |
| The honest-vs-rigged **study** | `dli-sim/src/turning.rs` `honest_vs_rigged`, `StudyReport` (hardcodes `weather-debt`, `a-round`, `the-debt-square`) | → `crates/tpth/` experiments; composes the generic `TurningBattery`/`RunManifest`. |
| Turning-specific structural invariants | `dli-sim/src/turning.rs` `StructuralInvariant::{MoonCount,ReciprocityClosed}` | Generic invariant registry stays (`DeckIsASet`, `ClosedRing`); TPtH invariants registered from `crates/tpth/`. |
| The saltmarket world built in Rust | `dli-world/src/loader.rs::example_docs()` (`#[doc(hidden)] pub`, **ships in the lib**, served at runtime) | Replace with a **neutral** generic toy world loaded from a bundled `.dli` fixture; the real saltmarket lives in `worlds/tpth/`. |
| Saltmarket as the only servable corpus | `dli-serve/src/session.rs::builtin_pack` | Parameterize over a corpus directory (the doc-comment already says so); ship TPtH packs as one corpus. |
| Export game-backend bound to the Turning | `dli-export/src/games.rs` (entire module, `use dli_sim::turning::*`) | Move to `crates/tpth/` export, or define a generic deck/ruleset render target the TPtH ruleset feeds. Downstream of the Turning genericization. |
| ✅ Concrete export profiles as Rust literals | `dli-export/src/profile.rs::Registry::builtin` (cards/lowlands@3, games/ludii@1, video/articy@1) | ✅ **Done (b5dd94b):** the TPtH catalog moved to `worlds/tpth/export/profiles.json`, loaded by the registry; `builtin()` keeps only the neutral context-pack profile. |

### B. Craft doctrine hardcoded in engine code (HARD → Tier 2 / factory config)

> ✅ **RESOLVED — Phase 3 complete (Steps 1–5, commits `1d0511b`→`c2d3098`).** Every
> row below is landed: the engine ships a generic `craft-doctrines/` library (83
> records) + the mechanism; TPtH's overlays/pillars/policies/thresholds/lexicon/
> material-catalog/arc-shape are **data in `worlds/tpth/craft/`**; the 1512-line
> TPtH-fused const is gone; `apply(config, packs)` reconstructs the 107
> field-for-field (round-trip golden). Design: `craft-separation-design.md`.

| Leak | Location | Move |
|---|---|---|
| The 107-principle KB | `dli-world/src/craft/kb.rs::PRINCIPLES` (general craft fused with TPtH mission-filters, D-0128 pillars, `D-nnnn` sources, Enfys/Aalto/Turning-named principles, factory/judge binding names) | Split: the **generic** principle records → `craft-doctrines/*` packs; the **TPtH configuration** (which packs, mission-filters, pillars, citations) → `worlds/tpth/craft/principles.yaml`, **produced by the factory**. `by_id/coverage/checkable_debt/why/evaluate_judge` take a world-supplied `&[Principle]`. |
| The no-battle-ten lexicon | `dli-world/src/craft/season.rs::policy::{BATTLE_SINGLE,BATTLE_PAIRS}` (English combat words, enforces D-0052) | Per-world optional `ten-battle-lexicon.yaml`; empty list = check no-ops. Tokenizer stays in engine. |
| TPtH threshold numbers as engine defaults | `dli-world/src/craft/season.rs::DEFAULT_THRESHOLDS` + `policy::` consts | `Thresholds` struct stays generic; the TPtH values → `worlds/tpth/craft/thresholds.yaml`. |
| `D-nnnn` citations + host-narrator ban emitted from check code | `dli-world/src/craft/season.rs` `with_citations("D-0179"/…)`, `NarratorMode::Host→error`, `is_canon_ref`/`is_research_ref` | Carry citation + severity as world-supplied metadata on each rule/threshold. |
| The material-planning taxonomy | `dli-world/src/craft/outline.rs::MaterialUnknown` (closed 12-category enum, D-0128/D-0056) | World-supplied category set (`material-categories.yaml`), or move the whole opt-in review to the TPtH craft config. |
| Kishōtenketsu baked into the outline schema | `dli-world/src/craft/outline.rs::EpisodeOutline.{ki,sho,ten,ketsu}` + battle-scan of `ten` | Arc-shape-specific fields/checks become a world-selected arc module; the generic `ArcShape` enum stays. |
| ✅ Sayability cap + `D-0095` in the naming vet | `dli-world/src/language/derivation.rs::SAYABILITY_CAP` (const 6) + `LangFinding::SayabilityCap` message printing `(D-0095)` | ✅ **Done (b5dd94b):** per-dialect `sayability-max <n>` clause added (loaded into `Phonotactics`); cap is now a generic default; `D-0095` dropped; TPtH's cap lives in `cymric-fringe.dli`. |
| ✅ Banned-in-name message hardcodes the allegory rationale | `dli-world/src/language/derivation.rs::LangFinding::BannedInName` | ✅ **Done** (`2cd5878` + `bfcb758`): message genericized (drops the TPtH allegory rationale) and now surfaces the world's own `BanDef.reason`. |

### C. Data + contract (mostly clean; relocate / rename)

| Artifact | State | Move |
|---|---|---|
| `examples/*.dli/.yaml/.json` | All TPtH (saltmarket, cymric-fringe, true-naming, second-practice, the-turning-rules, saltmarket-episodes/season) | → `worlds/tpth/`. **Safe data move** (update test path refs). Add a NEW neutral `examples/hello-world.dli`. |
| `fixtures/vendor/fantasy-*` | Pinned fantasy snapshot, digest-isolated | → `worlds/tpth/vendor/`. Safe move (update MANIFEST/test refs). |
| `fixtures/mutation/*` | Ported fantasy detection corpus | → `worlds/tpth/mutation/`; the generic check-class taxonomy in its README → engine spec docs. |
| `contracts/fantasy-worker/` | Mechanism generic; **mis-named** for one consumer; goldens TPtH-payload-bound | Rename dir + `contract:` id → `dli-worker`; keep TPtH-payload goldens as the fantasy consumer's suite; add a small generic-world conformance set. |

### D. Docs (SOFT; teach-on-generic + a TPtH worldbook)

The specs are a genuine language manual and the engine is architecturally
story-agnostic, but specs 03/04/05/06/09 (and lightly 10/11) illustrate with
unexplained TPtH canon. A non-TPtH reader meets grounds/binders/Weathers/Frith
with no glossary. Split into (1) **engine manual** teaching on the neutral world;
(2) a **TPtH worldbook companion** holding the worked instances (the Concordance,
cymric-fringe/Enfys, the Turning export, the era spine); (3) **consumer
integration** (the whole `integration/` tree + the migration design-notes),
positioned as "a worked integration: the fantasy factory," not core manual. Each
spec keeps one cross-link to its worldbook counterpart. The machinery prose does
not change — only the fenced examples and a handful of framing sentences move.

### E. dli-ledger (practice workbench)

The **cleanest region**: the practice workbench (`PracticeDef`, the closed
resource algebra, L1–L7 classes, `check_practice`/`check_proof`,
`SceneProjection`, the premise pool) is fully generic — proof-magic, true-naming,
and the Turning all run the identical code path, and practices are loaded as data.
`true-naming.dli` / `second-practice.dli` are already relocated (Phase 0). Three
residues remain:

| Leak | Location | Move |
|---|---|---|
| ✅ **HARD:** ambient TPtH cycles in real logic | `dli-ledger/src/practice.rs:66` `KERNEL_CYCLES = ["tide","moon","season"]` — seeded every practice's cycle scope, so `tide` resolved in *any* world undeclared | ✅ **Done (b5dd94b):** `PracticeEnv` ambient cycles now **empty** (derive `Default`); cycles must be declared; `tide/moon/season` moved to `worlds/tpth/time/cycles.dli`. |
| Contract-frozen diegetic name | `ledger_check.rs` `ProofFinding::WrongHour` / `proof.wrong-hour` — TPtH's "hour" in a frozen seam | Rename to neutral `WrongPhase` / `proof.wrong-phase` as a deliberate contract migration (schema regen). Low priority; rides with the `dli-worker` work. |
| ✅ Un-externalized practice | the Concordance / spoken-proof practice existed only inline in `tests/proof.rs` | ✅ **Done** (`b2c5caa`): committed as `worlds/tpth/practices/spoken-proof.dli` (with its own declared `tide` cycle); `proof.rs` loads it, all 20 assertions preserved. |

Module doc "the Concordance-class engine" is doc-only (genericize on release).

---

## The migration DAG

Ordered so every step lands gate-green. **Phase 0** is pure data relocation and
additive docs (safe now). Later phases are code extractions gated on the open
forks; each is an individually-verified step, not a big-bang.

**Phase 0 — establish the location (safe now, no engine code changes)**
- ✅ `DL-0017`/`DL-0018` + this plan + the `worlds/tpth/` home (README).
- ✅ Relocate the TPtH example corpus: `examples/{saltmarket,cymric-fringe,true-naming,second-practice}.dli` → `worlds/tpth/packs/`, `the-turning-rules.json` → `worlds/tpth/turning/`, the episode/season yaml → `worlds/tpth/episodes|season/`; test path-helpers repointed.
- ✅ Add a neutral `examples/hello-world.dli` teaching world (a generic river town, no TPtH lore) — now the only pack in `examples/`.
- ✅ Moved (they lived outside `examples/`) `fixtures/vendor|mutation` → `worlds/tpth/vendor|mutation/`; test path-helpers, the pinned/differential/shadow-read scripts, and the corpus docs repointed. ⏳ Still to move: the TPtH-payload contract goldens (with the `dli-worker` rename, Phase 2).

**Phase 1 — the boundary guard**
- Add a gate step: engine crate `src/` grep-denies a TPtH proper-noun set (saltmarket, cymric, enfys, the-debt-square, weather-debt, concordance, spoken-proof, pleach, hedgeway, `D-\d{4}`) outside lines marked `// doc-example`. Fails closed. This freezes the boundary and turns every remaining leak into a tracked, guard-visible debt.

**Phase 2 — the clean code extractions (each its own commit)**
- ✅ Sayability cap → per-dialect `sayability-max`; drop `D-0095` (b5dd94b).
- ✅ Banned-in-name message → generic (2cd5878); ⏳ surface `BanDef.reason` (follow-on).
- ✅ `KERNEL_CYCLES` ambient cycles → empty default + `worlds/tpth/time/cycles.dli` (b5dd94b).
- ✅ `profile.rs::Registry::builtin` → data-loaded profiles from `worlds/tpth/export/profiles.json` (b5dd94b).
- ⏳ `loader.rs::example_docs` → a **PD example world** (not a bare toy — see *The generic corpus* below); `session.rs::builtin_pack` → corpus-dir.
- ✅ Contract rename `fantasy-worker` → `dli-worker` + consumer profile (touches registry + fixtures) — DL-0018, `contracts/dli-worker/`.

**Phase 3 — craft to config — ✅ DONE (Steps 1–5; Step 6 = the factory-interface spec)**
- ✅ Step 1 (`Kb` handle + config-as-argument) · ✅ Step 2 (`Cow`/serde data-loadable records) · ✅ Step 3 (season/outline policy → `config.rs`) · ✅ Step 4 (shipped `craft-doctrines/` library, 83 records) · ✅ Step 5 (author `worlds/tpth/craft/` + `apply()` + round-trip golden + flip source + strip the TPtH const; `dli-world` tests moved to `worlds/example`).
- **Step 6 — the factory interface (spec; implemented fantasy-side, not here).** The engine LOADS `worlds/tpth/craft/*` as data and `validate()`s it; it never reads a decision ledger. The **factory** (fantasy repo) produces those files by mapping `D-nnnn` rulings → `PrincipleOverlay`/`WorldPrinciple` records — a `fantasy craft compile` step wired into the fantasy gate, regenerating `worlds/tpth/craft/*` idempotently. The record shapes in `dli-world::craft::worldcfg` (`CraftConfig`/`PrincipleOverlay`/`WorldPrinciple`) are the versioned contract; the round-trip golden (`apply == 107`) is the acceptance test the factory output must satisfy.

**Phase 4 — the Turning — ✅ DONE (commits `a65077a`/`2ca7129`; design: `tpth-turning-extraction.md`)**
- ✅ Per DL-0018 fork B, the **whole Turning moved to a thin `crates/tpth/` world crate** (types + instance + study + invariants + the card/Ludii export): `dli-sim/src/turning.rs` → `tpth/src/turning.rs`; `dli-export/src/games.rs` → `tpth/src/export.rs` (+ a world-side `tpth::export::run`). `dli-sim` is now SIM-1-only; `dli-export` is the generic export core (drops its `dli-sim` dep), keeps the generic `Cards`/`LudiiGdl` format targets failing closed via `NoEngineRendererForTarget`. **No wire/contract change** — the card export never read the compiled world (fake coupling), and the only live consumers were the moved tests. Boundary proof: no engine crate references `tpth`, no Turning literals remain in `dli-sim`/`dli-export` `src/`.
- ✅ **Follow-on DONE (sim-genericization; design: `sim-genericization.md`):** a generic `crates/dli-game` engine (parameterized `Ruleset` — axes/layers/moves/layouts/invariants as data; neutral vocabulary) now hosts the Turning as **data + a thin binding** (`crates/tpth` 1469→301 lines, golden-guarded byte-identical) loaded from `worlds/tpth/turning/*.json`, and a **standard 52-card game** (`examples/games/french-deck/`) ships as a generic instance — the ranked-run complement to the Turning's rankless wheel. Commits `c993298` (engine), `7f3a045` (Turning migration), `6dc426b` (52-card game). Remaining niceties: wire the `deloreis simulate`/`export` CLI stubs to `dli-game`; memoize `Ruleset::cards()` (a per-call rebuild — a perf, not correctness, item).

**Phase 5 — docs split — ✅ DONE (`499e198`; design: `docs-split.md`)**
- ✅ Specs 03/04/05/06/09 (+ light 10/11) re-illustrated on the shipped generic worlds (`camelot.dli`, `french-deck`); the worked TPtH instances moved to a new **`docs/worlds/tpth/`** worldbook (6 chapters, mdBook-wired); `integration/` reframed as "a worked integration: the fantasy factory" (one consumer of the generic `dli-worker` contract). Machinery prose unchanged; docs-oracle 43 units parse.
- Remaining nicety (non-blocking): genericize `docs/cards/expert-card.md` (a sibling pass, flagged).

---

## The generic corpus (author's direction, 2026-07-21)

The standalone tool should ship example worlds drawn from **public-domain
literature and real games**, not TPtH content and not just a bare invented toy —
recognizable worlds prove dli's generality and give the sim/practice/export
features real non-TPtH exemplars. The current `examples/hello-world.dli`
(rivertown) is a placeholder to be superseded.

**The hard constraint is copyright.** Rules and ideas are not copyrightable;
fictional-world *content* is. So:
- **Real games with traditional rules — safe.** A **tarot** ruleset (Marseille /
  Rider-Waite are public domain) is the natural generic oracular-card-game
  exemplar — same ruleset/sim/Ludii-export machinery as the Turning, which
  becomes "one instance of the generic thing." (Bridge, go, hanafuda also work.)
- **Public-domain literature — safe.** Sherlock Holmes, Arthurian legend,
  Norse/Greek myth, Alice, Oz, Grimm, Frankenstein, Dracula — rich, famous
  lore/character/continuity to teach the lore engine, time model, and naming
  oracle on, with zero legal exposure.
- **Living authors' worlds as bundled content — avoid** (Mistborn, Middle-earth
  as shipped *data* = IP risk). Use PD equivalents or lightly-original homages.
- **Craft doctrine is the exception that's fine.** Sanderson's *Laws of Magic*,
  kishōtenketsu, MICE, save-the-cat are published *craft advice*; citing them as
  named, attributed principles in the `craft-doctrines/` library (Tier 2, DL-0018)
  is standard and legally clean — distinct from shipping any author's world.

**Proposed shipped corpus:** a **tarot** ruleset (sim/export exemplar), a
**public-domain literary world** (Arthurian or Sherlock — lore/time/naming
exemplar), a small **myth-based or original practice** (magic-workbench exemplar),
and the **craft-doctrines library** citing the real published laws/frameworks.

**How it threads the DAG:** the tarot exemplar pairs with Phase 4 (Turning →
`crates/tpth/`, generic card-game example stays in `examples/`); the craft
library is Phase 3's Tier-2 target; the PD literary world supersedes
`hello-world.dli` and becomes `example_docs`/`builtin_pack`'s default (Phase 2's
`loader`/`serve` item). Authoring these PD worlds is its own tracked strand.

---

## Forks (resolved — [DL-0018](../decisions.md))

1. **Craft-doctrine home.** ✅ **dli ships a generic craft library.** A curated
   `craft-doctrines/` set of togglable packs ships with the tool; a project
   enables a subset. TPtH's own selection/config is factory-produced under
   `worlds/tpth/craft/`.
2. **TPtH code not yet pure data.** ✅ **A thin `crates/tpth/` world crate now.**
   The Turning type vocabulary, study, and world-specific invariants move there;
   simulator genericization (everything-as-data) is a follow-on, not a blocker.
3. **Contract rename.** ✅ **`fantasy-worker` → `dli-worker`.** Generic contract +
   a fantasy consumer profile; TPtH-payload goldens become the fantasy suite.
4. **Neutral teaching world.** A `hello-world.dli` (invented neutral setting)
   becomes the engine's shipped example; saltmarket demotes to `worlds/tpth/`.
   *(Confirmed direction; landed with the Phase-0 relocation.)*

---

## The boundary guard

A `worlds/`-aware gate step is the durable enforcement: once the boundary holds,
CI refuses any new engine-`src/` reference to a world's proper nouns or a
project's `D-nnnn` citations. Until each leak is extracted it is listed as an
explicit, guard-acknowledged exception, so the debt is visible and shrinking
rather than silently reaccumulating.
