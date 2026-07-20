---
status: live
title: "The general/TPtH separation: leak inventory and migration DAG"
verdict: "dli is the generic engine; craft is togglable project config; TPtH is one world's data. This is the ordered plan to tease them apart, gate-green at every step. Authority: DL-0017."
---

# The general / TPtH separation

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

| Leak | Location | Move |
|---|---|---|
| The entire lowlands Turning deck as Rust | `dli-sim/src/turning.rs` `lowlands_grounds/canonical_weathers/lowlands_figures/canonical_layouts`, `Ruleset::{lowlands_honest,keep_stamped,salt_fringe}`, `Variant::*` | → `worlds/tpth/turning/*.json` (the types already derive Serde+JsonSchema; `examples/the-turning-rules.json` is already the serialization). Load via a generic `Ruleset::from_rules`. |
| Turning-shaped **type vocabulary** | `dli-sim/src/turning.rs` `HandRole` (Taker/Keeper/Giver/Mender enum + `·4` arithmetic), `Weather/Ground/Figure/Leaf/Layer/Layout`, the `"reckoner"` hub id, `"weather-"` prefix, moon-count `grounds·4+weathers+1` | Genericize: declared roles/suits/conditions/hub as data on a parameterized `Ruleset`; or move the whole vocabulary to `crates/tpth/`. (Open fork B.) |
| The honest-vs-rigged **study** | `dli-sim/src/turning.rs` `honest_vs_rigged`, `StudyReport` (hardcodes `weather-debt`, `a-round`, `the-debt-square`) | → `crates/tpth/` experiments; composes the generic `TurningBattery`/`RunManifest`. |
| Turning-specific structural invariants | `dli-sim/src/turning.rs` `StructuralInvariant::{MoonCount,ReciprocityClosed}` | Generic invariant registry stays (`DeckIsASet`, `ClosedRing`); TPtH invariants registered from `crates/tpth/`. |
| The saltmarket world built in Rust | `dli-world/src/loader.rs::example_docs()` (`#[doc(hidden)] pub`, **ships in the lib**, served at runtime) | Replace with a **neutral** generic toy world loaded from a bundled `.dli` fixture; the real saltmarket lives in `worlds/tpth/`. |
| Saltmarket as the only servable corpus | `dli-serve/src/session.rs::builtin_pack` | Parameterize over a corpus directory (the doc-comment already says so); ship TPtH packs as one corpus. |
| Export game-backend bound to the Turning | `dli-export/src/games.rs` (entire module, `use dli_sim::turning::*`) | Move to `crates/tpth/` export, or define a generic deck/ruleset render target the TPtH ruleset feeds. Downstream of the Turning genericization. |
| Concrete export profiles as Rust literals | `dli-export/src/profile.rs::Registry::builtin` (cards/lowlands@3, games/ludii@1, video/articy@1) | Become data: a loader reads profiles from a file; engine keeps a neutral example profile. Makes "a profile is data, not a code fork" literally true. |

### B. Craft doctrine hardcoded in engine code (HARD → Tier 2 / factory config)

| Leak | Location | Move |
|---|---|---|
| The 107-principle KB | `dli-world/src/craft/kb.rs::PRINCIPLES` (general craft fused with TPtH mission-filters, D-0128 pillars, `D-nnnn` sources, Enfys/Aalto/Turning-named principles, factory/judge binding names) | Split: the **generic** principle records → `craft-doctrines/*` packs; the **TPtH configuration** (which packs, mission-filters, pillars, citations) → `worlds/tpth/craft/principles.yaml`, **produced by the factory**. `by_id/coverage/checkable_debt/why/evaluate_judge` take a world-supplied `&[Principle]`. |
| The no-battle-ten lexicon | `dli-world/src/craft/season.rs::policy::{BATTLE_SINGLE,BATTLE_PAIRS}` (English combat words, enforces D-0052) | Per-world optional `ten-battle-lexicon.yaml`; empty list = check no-ops. Tokenizer stays in engine. |
| TPtH threshold numbers as engine defaults | `dli-world/src/craft/season.rs::DEFAULT_THRESHOLDS` + `policy::` consts | `Thresholds` struct stays generic; the TPtH values → `worlds/tpth/craft/thresholds.yaml`. |
| `D-nnnn` citations + host-narrator ban emitted from check code | `dli-world/src/craft/season.rs` `with_citations("D-0179"/…)`, `NarratorMode::Host→error`, `is_canon_ref`/`is_research_ref` | Carry citation + severity as world-supplied metadata on each rule/threshold. |
| The material-planning taxonomy | `dli-world/src/craft/outline.rs::MaterialUnknown` (closed 12-category enum, D-0128/D-0056) | World-supplied category set (`material-categories.yaml`), or move the whole opt-in review to the TPtH craft config. |
| Kishōtenketsu baked into the outline schema | `dli-world/src/craft/outline.rs::EpisodeOutline.{ki,sho,ten,ketsu}` + battle-scan of `ten` | Arc-shape-specific fields/checks become a world-selected arc module; the generic `ArcShape` enum stays. |
| Sayability cap + `D-0095` in the naming vet | `dli-world/src/language/derivation.rs::SAYABILITY_CAP` (const 6) + `LangFinding::SayabilityCap` message printing `(D-0095)` | Per-dialect `sayability-max <n>` clause (sibling of `cluster-max`), loaded into `Phonotactics`; drop the hardcoded `D-0095`. |
| Banned-in-name message hardcodes the allegory rationale | `dli-world/src/language/derivation.rs::LangFinding::BannedInName` | Genericize the message; surface `BanDef.reason` (already carried). |

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

The practice **workbench** (`PracticeDef`, the closed resource algebra, L1–L7
classes, `check_practice`/`check_proof`, `SceneProjection`, the premise pool) is
generic; a magic system "is a practice with laws." The Concordance / spoken-proof
/ true-naming practices are **data** (`examples/true-naming.dli`,
`second-practice.dli`) → move to `worlds/tpth/packs/`. *(Region re-audited
separately; update this section if a hardcoded practice leak surfaces.)*

---

## The migration DAG

Ordered so every step lands gate-green. **Phase 0** is pure data relocation and
additive docs (safe now). Later phases are code extractions gated on the open
forks; each is an individually-verified step, not a big-bang.

**Phase 0 — establish the location (safe now, no engine code changes)**
- `DL-0017` + this plan + the `worlds/tpth/` and craft-doctrine homes (READMEs). *(this commit)*
- Relocate the TPtH corpus: `examples/*` → `worlds/tpth/packs|turning|episodes|season/`, `fixtures/vendor|mutation` → `worlds/tpth/vendor|mutation/`, updating test path references. *(after the wave-4 payload tail lands, so in-flight tests keep their inputs)*
- Add a neutral `examples/hello-world.dli` teaching world.

**Phase 1 — the boundary guard**
- Add a gate step: engine crate `src/` grep-denies a TPtH proper-noun set (saltmarket, cymric, enfys, the-debt-square, weather-debt, concordance, spoken-proof, pleach, hedgeway, `D-\d{4}`) outside lines marked `// doc-example`. Fails closed. This freezes the boundary and turns every remaining leak into a tracked, guard-visible debt.

**Phase 2 — the clean code extractions (each its own commit)**
- Sayability cap → per-dialect `sayability-max`; drop `D-0095` (small).
- Banned-in-name message → generic + `BanDef.reason` (small).
- `loader.rs::example_docs` → neutral toy from a bundled fixture; `session.rs::builtin_pack` → corpus-dir (medium).
- `profile.rs::Registry::builtin` → data-loaded profiles (medium).
- Contract rename `fantasy-worker` → `dli-worker` + consumer profile (medium; touches registry + fixtures).

**Phase 3 — craft to config (gated on fork A + the factory)**
- Extract `kb::PRINCIPLES` → generic `craft-doctrines/*` packs + a factory-produced `worlds/tpth/craft/principles.yaml`; engine queries take `&[Principle]`.
- Parameterize `season.rs` (battle-lexicon, thresholds, citations, material-categories, arc-shape) as world/project config.
- The factory-side decision→record mapping is built in the consuming (fantasy) repo, not here.

**Phase 4 — the Turning (gated on fork B)**
- Turning instance → `worlds/tpth/turning/*.json` behind a generic `Ruleset::from_rules`.
- Genericize or relocate the Turning type vocabulary + study + invariants; repoint `dli-export/games.rs`.

**Phase 5 — docs split**
- Engine manual on the neutral world; the TPtH worldbook companion; reposition `integration/` as a worked consumer integration.

---

## Open forks for the author

1. **Craft-doctrine home.** Where do the togglable packs live, and does dli
   **ship** a generic doctrine library (Sanderson, kishōtenketsu, MICE) with the
   tool, or does each project supply its own? Recommendation: ship a small
   curated generic library under `craft-doctrines/` that projects enable a subset
   of — it makes "turn craft on/off" real out of the box.
2. **TPtH code that is not yet pure data** (the Turning type vocabulary, the
   study, world-specific invariants): a thin `crates/tpth/` world crate, or push
   harder to genericize the sim so *everything* is data? Recommendation: a thin
   `crates/tpth/` now (fast, fully modular), with sim-genericization as a
   follow-on — don't block the separation on a hard generics refactor.
3. **Contract rename** `fantasy-worker` → `dli-worker`: confirm the name and that
   the fantasy factory becomes one consumer profile. (Low-risk; recommended.)
4. **Neutral teaching world**: confirm a `hello-world.dli` (invented neutral
   setting) as the engine's shipped example, with saltmarket demoted to
   `worlds/tpth/`.

---

## The boundary guard

A `worlds/`-aware gate step is the durable enforcement: once the boundary holds,
CI refuses any new engine-`src/` reference to a world's proper nouns or a
project's `D-nnnn` citations. Until each leak is extracted it is listed as an
explicit, guard-acknowledged exception, so the debt is visible and shrinking
rather than silently reaccumulating.
