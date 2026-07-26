---
status: live
title: "Phase-3 craft separation: design and implementation DAG"
verdict: "The concrete plan to split the 107-principle KB into a shipped generic craft-doctrines library + factory-produced TPtH config, with a type-enforced boundary and a 5-step green-commit DAG. Authority: DL-0017/0018."
---

# Phase-3 Craft Separation — Design & Implementation Plan

**Authority:** DL-0017/DL-0018; `docs/design-notes/tpth-separation-plan.md`. **Repos:** engine = `/home/jeff/Documents/scratch/deloreis` (`crates/dli-world`); factory = `/home/jeff/Documents/scratch/fantasy`.

> **Implementer's note (adaptation).** This design was authored assuming write
> access to the fantasy repo. In practice the fantasy repo is **read-only** from
> here, so: TPtH's craft config (Step 5's `worlds/tpth/craft/*`) lives in
> **`deloreis/worlds/tpth/craft/`** — the established TPtH content home — as the
> committed *output* of the factory mapping. Step 6 (the `fantasy craft compile`
> D-nnnn→record mapping) is **specified as the fantasy-side interface** here but
> implemented in the fantasy repo separately, not by this workstream. Steps 1–4
> are entirely engine-side (deloreis) and unaffected.

## 0. The governing invariant

Every commit preserves this equality until Step 5 deliberately relocates ownership:

```
current_107_principles  ==  apply( tpth_craft_config , load(enabled_doctrine_packs) )
```

The engine ships **generic doctrine packs + the checking mechanism**; the factory (fantasy repo) **produces the TPtH config** by mapping D-nnnn → records; the engine **loads config as data and never reads a decision ledger**. Production blast radius is tiny — the craft engine is *not* wired to the CLI or `dli-serve`; the only non-test consumer is `payload.rs` (3 call sites). Everything else is tests + a source-scraping docgen.

---

## 1. THE LAYOUT

### 1.1 Shipped generic library — `crates/dli-world/craft-doctrines/*.toml`

Eleven fine-grained, togglable packs (`include_str!`-embedded so no runtime files needed), citing **real published frameworks** as attributed principles. Bucket tally across the 107: **26 pure-generic (G) / 57 generic+overlay (G+) / 24 TPtH-only (T)**. The 26 G + the *stripped* form of the 57 G+ ship here (= 83 `DoctrinePrinciple` records); the 24 T never ship.

| Pack | Frameworks cited |
|---|---|
| `sanderson-laws-of-magic` | Sanderson Zeroth/First/Second/Third Laws |
| `kishotenketsu` | classical four-act; Truby want/need |
| `mice-quotient` | Card 1990; Kowal MICE |
| `promise-and-plot` | Sanderson BYU; Writing Excuses |
| `try-fail-cycles` | Card/Kowal; Stone&Parker "therefore/but"; McKee |
| `scene-and-sequel` | Swain; Bickham |
| `character-craft` | Sanderson sliders; Truby; Austen/Brontë; Portis |
| `prose-craft` | Le Guin; Orwell; King; Leonard; Ira Glass "the Gap"; Tolkien *eucatastrophe* |
| `worldbuilding` | Sanderson iceberg; Wildbow modular |
| `serial-fiction` | Wildbow; Platt&Truant; Fox; Eyal |
| `audio-drama` | Toscan; Wayland; Urbina; Gill (SFWA); Wagner leitmotif; McKee/Mamet |
| `save-the-cat` *(stub)* | Snyder beat-sheet |

`save-the-cat` ships as a **declared empty stub** (the author named it; the corpus has no beat-sheet principle — do not fabricate one). A world enables packs individually. Full per-principle classification and the exact "bits to strip" list is the KB analysis §4 — the implementer works from it directly.

**Shipped record — `DoctrinePrinciple` (world-agnostic *by construction*):**
```
DoctrinePrinciple {
  id:             Cow<'static,str>,      // pack-namespaced slug
  pack:           Cow<'static,str>,
  family:         Family,                // Plot|Character|World|Prose|Serial|Audio — NEVER Pillar
  statement:      Cow<'static,str>,      // world-agnostic; TPtH tokens lint-forbidden here
  source:         Cow<'static,str>,      // a REAL published framework, attributed
  phases:         Cow<'static,[Phase]>,
  checkable_kind: CheckableKind,         // none | deterministic-kernel | deterministic-prose | judge
  kernel_rule:    Option<KernelRule>,    // only for deterministic-kernel; from the engine's closed set
  default_params: BTreeMap<Cow<'static,str>, Scalar>,   // e.g. said_tag_ratio=0.85, cast_per_scene=4
  finding_codes:  Vec<Cow<'static,str>>, // exact kernel findings governed by this principle
}
```
This type **cannot carry** a `mission_filter`, a citation, a binding symbol, or `Family::Pillar` — the boundary is type-enforced, not lint-enforced (see §2.4). A CI lint additionally scans shipped statements for forbidden world tokens (`Enfys|Aalto|Turning|proof-speech|Wyrd|Multitude|the-multitude`).

### 1.2 World craft-config — `fantasy/worlds/tpth/craft/*.toml` (factory output)

```
enabled-packs.toml      # exact shipped packs this project enables
overlays.toml           # 57 PrincipleOverlay — the D-nnnn / mission-filter / binding / pillar-tier bits
world-principles.toml   # 24 WorldPrinciple (T rows) incl. 5 pillars + wonder-amid-ruin
precedence.toml         # the precedence tier (D-0128) + the pillars-win standing rule
registries.toml         # factory_namespaces {Check,Style,Raise} + judge_kinds registry
thresholds.toml         # outline + season numeric bands, calibrated=true
finding-policy.toml      # finding-code -> {citations[], severity?}
narrator-policy.toml     # NarratorMode -> Disposition (Host,direct_address -> Error, D-0176)
provenance.toml          # D-nnnn / RR-nnn / docs-path patterns
material-catalog.toml    # the 12 categories + the REQUIRED set (D-0128/D-0056)
```

**Factory records (loaded by the engine as data):**
```
PrincipleOverlay {                       // rides on a DoctrinePrinciple by id
  principle_id:    String,               // must resolve to an enabled pack
  mission_filter:  MissionFilter,        // Pass | Adapted{note} | Tension{note,resolution}
  citations:       Vec<String>,          // D-nnnn / canon anchors
  binding:         Binding,              // Kernel{params} | Factory{ns,symbol} | Attest(kind) | Declaration
  in_precedence_tier: bool,              // promote this generic principle into the pillar tier?
  param_overrides: BTreeMap<String,Scalar>,
}
WorldPrinciple {  id, family(may be Pillar), statement(TPtH allowed),
                  source(canon/D-nnnn), phases, mission_filter, binding, citations }
CraftConfig { enabled_packs, precedence_tier, factory_namespaces, judge_kinds,
              overlays:[PrincipleOverlay], world_principles:[WorldPrinciple],
              outline_thresholds, season_thresholds, beat_lexicons, finding_policy,
              narrator_policy, provenance, material_catalog, arc_shapes }
```

### 1.3 Where the D-0128 pillars + mission-filters live

**In the fantasy repo, as data.** The four D-0128 pillars + `wonder-amid-ruin` are `WorldPrinciple`s in `world-principles.toml` sitting in the precedence tier; `pillars-win-standing-rule` is a world principle and `precedence.toml` carries the tier policy (it is *world policy, not doctrine*). Every mission-filter (`Adapted`/`Tension` note + resolution) is the `mission_filter` field of a `PrincipleOverlay` in `overlays.toml`. The engine ships only the **mechanism**: the `MissionFilter` enum and `why()`'s pillar-first precedence sort over whatever tier the config populates. Zero pillar/filter *content* in `dli-world`.

### 1.4 The engine's own test world — `crates/dli-world/worlds/example/craft/`

A minimal **synthetic** world (generic packs + a couple of toy overlays) that exercises load/apply/validate/round-trip in dli-world's tests. Never TPtH. This is what lets deloreis stay green with zero TPtH content after Step 5.

---

## 2. THE ENGINE API

### 2.1 The `Kb<'a>` handle (CRAFT-3)

The six static-reading query fns become borrow-generic methods; `evaluate_judge` already takes `&Principle` and stays a free fn.
```rust
pub struct Kb<'a> { principles: &'a [Principle] }
impl Kb<'static> { pub fn builtin() -> Self; }        // Step1: wraps PRINCIPLES const. Step4+: loads packs.
impl<'a> Kb<'a> {
  pub fn new(principles: &'a [Principle]) -> Self;
  pub fn principles(&self) -> &'a [Principle];
  pub fn by_id(&self, id:&str) -> Option<&'a Principle>;
  pub fn principles_binding(&self, r:KernelRule) -> Vec<&'a Principle>;
  pub fn checkable_debt(&self) -> Vec<&'a Principle>;
  pub fn coverage(&self) -> Coverage;
  pub fn why(&self, code:&str) -> Vec<&'a Principle>;
  pub fn audit(&self) -> Vec<KbError>;
}
```
Retain thin **deprecated free-fn shims** (`why(c)` → `Kb::builtin().why(c)`) so Step-1 churn is near-zero and tests migrate lazily.

### 2.2 The `&'static str` decision — **`Cow<'static, str>` + `Deserialize`, drop `Copy`**

`Principle` today is `Copy` with `&'static str` fields and derives `Serialize` but **not** `Deserialize` — it is emittable but not loadable. DL-0018's "the engine only LOADS craft config as data" plus shipping packs as TOML both require deserialization, which `&'static str` cannot satisfy without `Box::leak`/interning.

**Decision: change the string-bearing fields of `Principle`, `MissionFilter`, `Binding`, and the `EpisodePolicy` lexicon from `&'static str` → `Cow<'static, str>`, `phases` → `Cow<'static,[Phase]>`; add `Deserialize`; drop `Copy` (keep `Clone`).** Rationale: `Cow::Borrowed("…")` is `const`-constructible, so the literal `PRINCIPLES` table survives the type change unchanged (Step 2) and only disappears when packs are extracted (Step 4) — this decoupling is what keeps each step minimal and green. Loaded corpora deserialize into `Cow::Owned`. `Copy` removal is inert: every query already hands out `&Principle`/`Vec<&Principle>`. `KernelRule/Family/FactoryNs/Checkable/Phase` stay `Copy` closed enums.

**Rejected alternative:** keep `&'static str`, have the factory *codegen* a Rust const table (matching canon→Dialog codegen). Zero field churn, but it makes the world config compiled Rust rather than loaded data — contradicting "loads config as data," and the shipped TOML packs need `Deserialize` regardless. The `Kb<'a>` borrow-slice is correct under either path; we build toward `Cow`.

### 2.3 Season signatures (the CRAFT-1 seam CRAFT-2 already has)

`check_season_rules(ledger, &Thresholds)` is **already** world-parametric — leave it. Mirror it for CRAFT-1:
```rust
pub struct EpisodePolicy<'a> {           // = OutlineThresholds (scalars) + lexicons
  pub scene_cast_ceiling: usize, pub hook_seconds_ceiling: u32,
  pub slider_flat_ceiling: u8,   pub slider_low_floor: u8,
  pub battle_single: Cow<'a,[Cow<'a,str>]>,
  pub battle_pairs:  Cow<'a,[(Cow<'a,[Cow<'a,str>]>, Cow<'a,str>)]>,
}
pub const DEFAULT_EPISODE_POLICY: EpisodePolicy<'static> = /* current policy:: consts, empty lexicon */;
pub fn check_episode(ep:&EpisodeOutline, opts:CheckOptions, policy:&EpisodePolicy) -> Vec<WorldFinding>;
pub fn check_season (s:&SeasonOutline,  opts:CheckOptions, policy:&EpisodePolicy) -> Vec<WorldFinding>;
```
The end-state `CraftConfig` (§1.2) aggregates `{ kb, outline_thresholds(EpisodePolicy scalars), season_thresholds, beat_lexicons, finding_policy, narrator_policy, provenance, material_catalog, arc_shapes }`; `Kb` is its query handle. Early steps introduce the sub-objects; the fold to one `CraftConfig` lands when CLI/serve wiring does.

### 2.4 Two-type boundary — `DoctrinePrinciple` (shipped) vs `Principle` (runtime)

`apply(&CraftConfig, &[DoctrinePack]) -> Vec<Principle>` composes each enabled `DoctrinePrinciple` with its optional `PrincipleOverlay`, and appends the `WorldPrinciple`s. The shipped type physically cannot hold world content; the runtime type is today's `Principle` (now Cow-fielded). This makes the legal/architectural boundary compiler-checked rather than review-checked.

### 2.5 Every call site to update (from the census)

| File | Change |
|---|---|
| `crates/dli-world/src/craft/kb.rs` | add `Kb`; 6 fns → methods (`PRINCIPLES`→`self.principles`, `&'static`→`&'a`); `by_id` eq → `.as_ref()==id`; Cow/Deserialize/drop-Copy (Step 2); keep deprecated shims |
| `crates/dli-world/src/craft/season.rs` | add `EpisodePolicy`+`DEFAULT_EPISODE_POLICY`; replace `policy::*` reads with `policy.*`; add `policy` param to `check_episode`/`check_season`; **keep every `warn(...)`/`error(...)` literal code string byte-identical** |
| `crates/dli-world/src/craft/mod.rs` | export `Kb`,`EpisodePolicy`,`DEFAULT_EPISODE_POLICY`; keep shim re-exports |
| `crates/dli-world/src/payload.rs` | `BatteryContext` gains `kb: Kb<'static>` (default `builtin()`) + `with_kb`; lines 44–49 imports, 654 field, 844/1105 `kb::why`→`ctx.kb.why`; `outline_conformance` takes `ctx`/`kb`; `check_season_rules(&ledger,&ctx.thresholds)` unchanged |
| `crates/dli-world/tests/craft_rules.rs` | `kb::X(...)`→`Kb::builtin().X(...)`; `kb::PRINCIPLES`→`.principles()`; season sites unchanged |
| `crates/dli-world/tests/craft_outline.rs` | add `&DEFAULT_EPISODE_POLICY` to ~26 `check_episode` + 6 `check_season` (local `const P` alias) |
| `crates/dli-world/tests/payload_battery.rs` | unchanged **iff** the new `BatteryContext` field defaults via `::new` |
| `xtask/src/finding_families.rs` | **NO change** — it scrapes `season.rs` source text for `outline.*` literals; verify green (invariant: code strings untouched) |

---

## 3. THE SEASON PARAMETERIZATION

Default-off discipline throughout: **a dli install with no world config runs the structural battery + generic scalar defaults and emits zero world-specific findings.**

| # | Hardcoded item (today) | → Config field | Engine change | Generic default (unset) |
|---|---|---|---|---|
| 1 | `policy::BATTLE_SINGLE/PAIRS` (D-0052 no-battle-ten wordlist) | `EpisodePolicy.battle_*` / `beat_lexicons` | `reads_as_battle` → `scan_lexicon(text,&BeatLexicon)`; `check_ten` loops config lexicons over the config-named turn slot | empty ⇒ scan no-ops, `ten-reads-as-battle` never fires |
| 2 | `policy::SCENE_CAST_CEILING / HOOK_SECONDS_CEILING / SLIDER_FLAT_CEILING / SLIDER_LOW_FLOOR` | `EpisodePolicy` scalars | delete `policy::` consts; read `policy.*` | cast 4, hook 60s, flat 5, floor 4 (legit generic craft numbers, overridable) |
| 3 | `DEFAULT_THRESHOLDS` inline TPtH values (CRAFT-2 bands) | `season_thresholds: Thresholds` | none — seam exists; stop treating `DEFAULT_THRESHOLDS` as canonical | `DEFAULT_THRESHOLDS` uncalibrated (`calibrated:false` ⇒ bands ride at warn); `season_cast_budget:None`=off |
| 4 | inline `.with_citations(vec!["D-0179"/"D-0176"/…])` in check code | `finding_policy.citations[code]` | remove inline calls; central post-pass looks up citations by finding code | empty ⇒ finding fires with no citation (today's norm for most) |
| 5 | `NarratorMode::Host` → error; `direct_address` → error (D-0176) | `narrator_policy` dispositions | `check_audio_plan` reads `Disposition` (Allow/Warn/Error) per mode; severity from config not `error()` | every mode `Allow` ⇒ no narrator-framing findings |
| 6 | `is_canon_ref` (`^D-\d{4}$`), `is_research_ref` (`RR-\d{3}` / path / URL) | `provenance` patterns | replace hand-rolled fns with `matches_any(ref,&patterns)` | empty ⇒ accept any non-blank (dup + non-blank checks still run); ship a generic URL pattern |
| 7 | `MaterialUnknown` 12-variant enum + `REQUIRED` (D-0128/D-0056) | `material_catalog: [{id,required}]` | **biggest schema change**: enum + 12 named `Option<String>` fields → `answers:BTreeMap<CategoryId,String>` + `unknowns:BTreeSet`; completeness loops the catalog | empty ⇒ `material-assumption-incomplete` never fires; provenance guards still run on whatever keys exist |
| 8 | `EpisodeOutline.{ki,sho,ten,ketsu}` + hardcoded `ten` scan target/band | `arc_shapes` (`turn_slot`, `position_band`) | **keep the ki/sho/ten/ketsu named fields** (kishōtenketsu is a real framework, not world content); drive only *which slot is scanned* and *the band* off config | no `turn_slot` ⇒ no beat scanned, arc-band check no-ops; TPtH sets turn_slot=ten, band 0.55–0.80 |

Findings that turn on **ledger shape** (promise xor-carry, MICE interleave, scene-name clash, accomplishment-earned) keep their intrinsic severity — engine-owned, not tunable. Only *policy-driven* dispositions (item 5) and citation attachment (item 4) route through config.

---

## 4. THE FACTORY BOUNDARY

**Stated as an interface the fantasy repo implements against an engine-published schema.**

**Engine LOADS (data only — never a ledger, never a D-nnnn parse, never a canon read):**
```
dli_world::craft::load {
  fn doctrines() -> Vec<DoctrinePack>                       // include_str! shipped packs
  fn load_config(world_dir:&Path) -> Result<CraftConfig>    // serde over worlds/<w>/craft/*.toml
  fn apply(cfg:&CraftConfig, packs:&[DoctrinePack]) -> Vec<Principle>
  fn validate(cfg:&CraftConfig, packs:&[DoctrinePack]) -> Vec<ConfigError>
      // every overlay.principle_id resolves to an enabled pack; binding symbols non-empty;
      // precedence tier well-formed; material catalog keys unique; provenance patterns compile
}
```
The engine also **publishes versioned JSON Schemas** for `DoctrinePack`,
`DoctrinePrinciple`, `PrincipleOverlay`, `WorldPrinciple`, and `CraftConfig`
under `schemas/craft/*-v1.schema.json`; `cargo run -p xtask -- schema --check`
guards them against drift.

**Factory PRODUCES (fantasy repo owns the D-nnnn → record mapping):**
```
fantasy craft compile   # reads canon/decisions.md + canon craft source-of-truth,
                        # emits worlds/tpth/craft/*.toml conforming to the engine JsonSchema:
                        #   D-0052  -> overlay(mission_filter=Adapted, beat_lexicon)
                        #   D-0128  -> precedence.toml tier + judge_pillars binding
                        #   D-0176  -> narrator-policy Host/direct_address=Error
                        #   D-0179  -> finding-policy citations[sfx codes]
                        #   D-0056  -> material-catalog categories+required
                        #   ...one row per governing ruling
```
This command lives beside `fantasy propagate`/codegen and joins the fantasy gate: when a ruling lands, `fantasy craft compile` regenerates the world config, exactly the "factory maps ledger → records" of DL-0018(3). **The engine never invokes the factory.**

**The contract / golden test:** `apply(load_config("worlds/tpth/craft"), doctrines()) == the TPtH 107-principle corpus`. This faithfulness test lives in the **fantasy repo** (it owns both the config and the expectation, checked-in as a serialized snapshot). deloreis keeps only the `worlds/example` mechanism round-trip. This honors "TPtH content cleanly modularized OUT of the engine crates."

---

## 5. THE IMPLEMENTATION DAG

Each step is one green commit (or a small green series). Dependencies are strict; 3 and 4 parallelize after 2.

**Step 1 — Config-as-argument, zero behavior/format change.**
Add `Kb<'a>` (methods + shims); add `EpisodePolicy`+`DEFAULT_EPISODE_POLICY` and thread through `check_episode`/`check_season`; `BatteryContext.kb` + `with_kb`; update the 2 `kb::why` sites and tests. `PRINCIPLES` stays `&'static`; no data files; docgen untouched. *Green: identical findings; pure structural refactor.*

**Step 2 — Data-loadable records.**
`Cow<'static,str>` + `Deserialize` + drop `Copy` on `Principle`/`MissionFilter`/`Binding`/`phases`; add `load::load_config`/pack parsing. Builtin const stays literal via `Cow::Borrowed`. *Green: round-trip test — serialize builtin 107 → deserialize → equals builtin.* (Blocks 4 and 5.)

**Step 3 — Season-policy externalization** (independent of the KB split; 5 green sub-commits):
3a FindingPolicy (drop inline `.with_citations(D-nnnn)`) · 3b NarratorPolicy · 3c ProvenanceFormats · 3d **MaterialCatalog** (enum→keyed map, isolated) · 3e ArcShapeCatalog turn-slot/band (keep field names). *After Step 3, `season.rs`/`outline.rs` contain no D-nnnn and no TPtH ontology.*

**Step 4 — Extract generic doctrine packs.**
Author `craft-doctrines/*.toml` (83 records across 11 packs + `save-the-cat` stub); fix the two attribution gaps (`aphorism-as-acoustic-logo`→Wagner leitmotif; `music-as-learned-lexicon`→audio-branding lit) and reclassify `bar-raiser-taste-gap`/`wonder-amid-ruin` out of `Family::Pillar`; add `Kb::doctrines()`; add the world-token lint. `builtin()` unchanged yet. *Green: packs parse, cover expected ids, statements pass the token lint.* (Depends on 2.)

**Step 5 — Author TPtH config + prove round-trip + flip the default.**
In the fantasy repo, author `worlds/tpth/craft/*` (57 overlays + 24 world-principles incl. 5 pillars + wonder-amid-ruin + registries + policies). Add `apply()`; the fantasy golden test `apply==snapshot(107)`. Flip the TPtH consumer from the compiled 107 to `apply(...)`; remove TPtH-content const from `dli-world`; deloreis tests switch to `worlds/example`. *Green both sides: deloreis generic-only; fantasy composed.* (Depends on 3+4.)

**Step 6 — Factory interface.**
Formalize `fantasy craft compile` (D-nnnn→record regen of `worlds/tpth/craft/*`), wire into the fantasy gate; deloreis publishes the versioned JsonSchema. *Green: regen is idempotent; validate() clean; golden holds.* (Depends on 5.)

---

## 6. RISKS + THE HARDEST CALLS

1. **Drop `Copy` on `Principle` (`&'static`→`Cow`).** *Call taken:* adopt `Cow` + `Deserialize`; the const table survives via `Cow::Borrowed`, so Step 2 is decoupled from Step 4. *Risk:* inert — nothing downstream relied on `Copy` (queries return refs). Rejected the codegen-const alternative because it contradicts "loads config as data" and packs need `Deserialize` anyway.

2. **`MaterialUnknown` enum → keyed map (loses compile-time exhaustiveness).** *Call taken:* adopt the keyed map — the category *set* is world ontology per DL-0018 — and move exhaustiveness into `validate()`. Isolated as Step 3d. This is the single most invasive schema change; the trade is correct but must land alone.

3. **Kishōtenketsu in the schema.** *Call taken:* **keep** the `ki/sho/ten/ketsu` fields on `EpisodeOutline`; generalize only the *scan target* and *band*. Diverges from the fuller `BTreeMap<BeatSlot>` generalization in the season analysis — deferred as optional, because kishōtenketsu is a named framework (not a world leak), so separation-correctness never requires it; only a *second* arc vocabulary would, which no world needs today. Avoids massive test churn.

4. **The golden test crosses the repo boundary.** *Call taken:* the faithfulness golden (`apply==107`) lives in the **fantasy repo** (it owns config + expectation); deloreis keeps only a synthetic `worlds/example` mechanism test. *Risk:* engine record-schema drift silently breaks the factory — mitigated by publishing the versioned JsonSchema (Step 6) and running the fantasy golden in the fantasy gate.

5. **Content gaps that block shipping generic (Step 4).** Two principles are sourced only to canon and must gain real external attribution before shipping (leitmotif / audio-branding); two are misfiled under `Family::Pillar` and need family reclassification with only the D-0128 emphasis left as overlay. These are authoring tasks, not code — gate Step 4 on them.

6. **Citation strings are a hard leak, not a nicety.** `D-0179` etc. literally appear in `season.rs` `.with_citations`. FindingPolicy (Step 3a) is mandatory for "engine ships no TPtH content," not optional polish.

7. **Docgen invariant.** `xtask/finding_families.rs` scrapes `season.rs` source for `outline.*` literals. **Every `warn(...)`/`error(...)` code string stays byte-identical** through all refactors — moving a value out of a `policy::` const never touches an emitted string, so this holds if respected; call it out in every season commit.

8. **Cross-repo version contract.** Once the factory targets the serde/JsonSchema shape, that shape is a public contract. Version it; a breaking record-shape change is a coordinated two-repo change.

Key engine files: `/home/jeff/Documents/scratch/deloreis/crates/dli-world/src/craft/{kb.rs,season.rs,outline.rs,mod.rs}`, `.../src/payload.rs`, `.../tests/{craft_rules.rs,craft_outline.rs,payload_battery.rs}`, `.../xtask/src/finding_families.rs`. New: `.../craft-doctrines/*.toml`, `.../worlds/example/craft/*`, `.../src/craft/{config.rs,load.rs}`. Factory: `/home/jeff/Documents/scratch/fantasy/worlds/tpth/craft/*` + `src/fantasy/craft/` (`fantasy craft compile`).
