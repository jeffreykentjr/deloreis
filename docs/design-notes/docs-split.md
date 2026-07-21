---
status: live
title: "Phase 5 — the docs split: engine manual, TPtH worldbook, consumer integration"
verdict: "The specs teach the engine on the shipped generic worlds; the worked TPtH instances move to a docs/worlds/tpth/ worldbook; docs/integration/ is reframed as one worked consumer. Only fenced examples and a few framing lines move. Authority: DL-0017/DL-0018."
---

# Phase 5 — the docs split

**Authority: [DL-0017](../decisions.md) / [DL-0018](../decisions.md).** This note
elaborates the Phase-5 entry of
[`tpth-separation-plan.md`](tpth-separation-plan.md) §D (leak region D; the DAG's
Phase 5) into a concrete design and implementation plan. The ledger
(`../decisions.md`) wins if they ever disagree.

De Lore Is ships as a general-purpose lore tool; *To Pleach the Hedgeway* (TPtH)
is one world authored in it. The specs are a genuine language manual and the
engine is architecturally story-agnostic, but specs `03`/`04`/`05`/`06`/`09` (and
lightly `10`/`11`) illustrate with unexplained TPtH canon — a non-TPtH reader
meets grounds/binders/Weathers/Frith with no glossary. This split fixes that: the
manual teaches on the shipped generic worlds, the worked TPtH instances become a
separate companion, and the consumer wire is repositioned as one worked
integration. **The machinery prose does not change — only the fenced examples and
a handful of framing sentences move.**

---

## The gate that governs the whole plan

The one hard constraint is the **docs oracle**
([`xtask/src/docs_oracle.rs`](../../xtask/src/docs_oracle.rs)). It:

1. Recursively walks **`docs/**/*.md`** and parses every ` ```dli ` fenced block
   through the real front end; any **error-class** diagnostic fails the gate.
2. Parses **top-level `examples/*.dli` only** — a non-recursive `read_dir`,
   `.dli` extension only. So `examples/games/french-deck/*.json` is **not** seen
   by the oracle; only `examples/camelot.dli` is.
3. Requires `SKILL.md` to hold **≥ 3** dli blocks and the corpus **≥ 5** units
   total.

Four consequences drive every step below:

- **Only ` ```dli ` blocks are gated.** ` ```text `, ` ```json `, and ` ```yaml `
  blocks are invisible to the oracle — free to rewrite.
- **Moving a *parsing* dli block from a spec into a worldbook `.md` is
  parse-neutral** — both live under `docs/`, so the oracle re-parses it in its
  new home unchanged. Zero risk.
- **The only parse risk is newly authored generic dli.** Camelot excerpts are
  already proven (`examples/camelot.dli` is a gated example); french-deck is JSON
  (ungated). New generic dli blocks (a generic order-rule, a conserved-resource
  practice, a `banned` term, an agent-init bundle, a french-deck `variant`) must
  be parse-verified.
- **Worldbook prose must live under `docs/`** or its dli blocks escape the gate.

mdBook wiring: `book/src/*.md` are `{{#include ../../docs/...}}` stubs listed in
[`book/src/SUMMARY.md`](../../book/src/SUMMARY.md). Every new worldbook file needs
a stub plus a `SUMMARY.md` line, or the book omits it. Design-notes (this file
included) are **not** in `SUMMARY.md`, so this note is touched only by the
docs-oracle — which is why it carries no ` ```dli ` fences.

---

## The three homes

| Home | What | Where |
|---|---|---|
| **Engine manual** (generic) | The specs re-illustrated on the shipped generic worlds; the generic cards; charter; decisions; generated reference. | `docs/spec/00`–`11`, `docs/cards/`, `docs/charter.md`, `docs/decisions.md`, `docs/reference-generated/`, `docs/PLAN.md` |
| **TPtH worldbook** (new) | The extracted worked instances — the Concordance, cymric-fringe/Enfys, the Turning export, the era spine, the banned-vocab rationale — as diegetic prose plus the fenced blocks lifted verbatim from the specs. | **`docs/worlds/tpth/`** (new) |
| **Consumer integration** | The whole `integration/` tree, repositioned as *"a worked integration: the fantasy factory"* — one consumer of the generic `dli-worker` contract, not core manual. | `docs/integration/` (reframed) |

The **shipped generic worlds** that carry the re-illustration:

- [`examples/camelot.dli`](../../examples/camelot.dli) — the Matter of Britain
  (public domain): a totally-ordered era spine with a narration vantage, events
  with a before-fact and a deliberately unordered pair, a place-adjacency graph,
  people/things/facts, states with `since`/`until` across two eras, a spoken
  enchantment (`practice merlins-craft`, graded-monotone `breath`), a dialect
  (`old-speech`) and a derived name (`myrdin`), an open question and a hypothesis.
  It exercises the `03`/`04`/`05`/`06` teaching surface end to end.
- [`examples/games/french-deck/`](../../examples/games/french-deck/) — a standard
  52-card game as a generic ruleset (`rules.json`) plus export profiles
  (`profiles.json`: `cards/french-deck`, `games/french-deck-ludii`). The ranked-run
  complement to the Turning's rankless wheel; it carries the `09` export surface.

**Design-notes stay put.** `docs/design-notes/*` are engine
rationale/history, already status-framed. The TPtH-named ones
([`tpth-separation-plan.md`](tpth-separation-plan.md),
[`sim-genericization.md`](sim-genericization.md),
[`tpth-turning-extraction.md`](tpth-turning-extraction.md)) legitimately name TPtH
because extraction *is* their subject; they do **not** get genericized.

---

## Per-spec disposition

**Legend.** **→WB** = the block moves verbatim to the worldbook. **swap** =
replace with a generic re-illustration. **free** = a non-dli block (`text`/`json`/
`yaml`), rewrite in place (oracle-invisible). Every split spec keeps exactly
**one** cross-link footer to its worldbook counterpart.

### `03-time.md` → `docs/worlds/tpth/time.md`  (2 dli + 1 text)

| Locus | Now | Disposition |
|---|---|---|
| §6 block A `dli` — era spine `elder-cycles…the-narrators-era`, events `first-storehouse-lie`/`protagonists-taking`, `cycle tide(4)`/`moon(29)` + the leaf(29) note, `state gwern.water-status … D-0114` | TPtH | **→WB** + **swap** to camelot's shipped era spine (`uthers-time < arthurs-reign < after-camlann`), its events + `before` + the unordered pair, `narration vantage after-camlann`, and `state excalibur.keeper` `since`/`until` (`camelot.dli:33-86`). **Cycles gap:** camelot ships no `cycle`; tide/moon/season are public-domain natural cycles (only the Turning's *leaf(29)* is TPtH) — grow camelot with `cycle moon(29)` / `cycle season(4)` (Hard Call 6) so 03 excerpts a real shipped cycle. |
| §6 block B `dli` — `rule full-team-floor over order D-0162`, `no-deep-time-act-one D-0168`, `payoff-after-reveal` | TPtH | **→WB** + **swap** to a generic `rule … over order`. Camelot ships `rule show-the-price over season` (`camelot.dli:132`) but no `over order`; author one (parse-risk) or grow camelot. |
| §6 block C `text` — `episode e14 "The Drowned Field"` | TPtH | **free** — rewrite to a camelot episode; oracle-invisible. |
| §1(ii) prose — "the sabotage, the breach, the world-after" | TPtH-flavored | **swap** framing to neutral act language; keep `kishōtenketsu` (a Tier-2 craft lens). |

### `04-practices.md` → `docs/worlds/tpth/concordance.md`  (3 dli + 2 text)

| Locus | Now | Disposition |
|---|---|---|
| §3 `dli` — `practice spoken-proof … resource debt : linear conserved`, the nine error refinements, licensed-ignorance | TPtH | **→WB** (the nine-error + licensed-ignorance / rigged-deck treatment is the worldbook centerpiece). Spec §3 re-illustrates the *mechanism* — see Hard Call 1 for the generic conserved-resource exemplar. |
| §4 `dli` — `practice true-naming` (the arbitrariness proof; graded-monotone) | fixture, bundled under `worlds/tpth/packs/` | **swap** to camelot's shipped `practice merlins-craft` (`camelot.dli:114-129`) — graded-monotone `breath`, `knows()`, refines L4/L2/L3, dual cost; a field-for-field generic analogue. |
| §5 `dli` — `proof e01-s04 … in spoken-proof` | TPtH | **→WB** + **swap** to a generic `proof … in merlins-craft` ledger (author; parse-risk). |
| §3/§5 `text` — the `invalid: wrong-hour …` / `invalid: L2 …` verdicts | TPtH | **free** — rewrite to merlins-craft verdicts. |
| §1/§2 prose — "the Concordance — the Spoken Proof that opens the series", "the Turning's ring of twenty-nine leaves" | TPtH | **swap** framing to generic ("a spoken-enchantment practice"); cross-link the Concordance to the worldbook. |

### `05-language-layer.md` → `docs/worlds/tpth/language.md`  (4 dli + 1 text)

| Locus | Now | Disposition |
|---|---|---|
| §1 `dli` — `dialect cymric-fringe … name-part en/fys/wern/cors` | TPtH | **→WB** + **swap** to camelot `dialect old-speech` (`myr`/`din`, `camelot.dli:92-100`). `basis attested "Old Welsh / Brythonic"` fits camelot authentically (the Matter of Britain is Brythonic) — grow old-speech with it. |
| §2 `dli` — `name enfys "Enfys" … in cymric-fringe` | TPtH | **→WB** + **swap** to `name myrdin "Myrdin" … in old-speech` (`camelot.dli:102-104`). |
| §2 `text` — `invalid: dialect-fit at name gwyneth` | TPtH | **free** — rewrite to an old-speech failure. |
| §4 `dli` — `term hedgeway`/`the-plenty` + `banned machine`/`algorithm` | TPtH | **→WB** (the allegory rationale is worldbook material) + **swap** to a generic lexicon term + a neutral `banned` term (author; parse-risk). |
| §5 `dli` — agent-init `hypothesis h-cymric` + proposed `dialect cymric-fringe agent:…` + `ratify D-0195 { h-cymric := true }` | TPtH | **→WB** + **swap** to a generic agent-init bundle on old-speech (author; parse-risk). |
| §3 tables — `D-0095`, "deck-figure versus tarot" | TPtH-adjacent | keep the table *structure*; swap the two TPtH cell examples. |

### `06-craft-and-payloads.md` → `docs/worlds/tpth/craft.md`  (1 dli + 1 yaml)

| Locus | Now | Disposition |
|---|---|---|
| §5 `dli` — `principle tryfail-before-triumph source sanderson cites D-0128`; `rule accomplishment-earned`; `principle register-fit cites D-0166 tension "…the hedge register wins"`; `rule scene-name-clash` | mixed | **Keep the craft-doctrine names** (`tryfail-before-triumph`, `sanderson`, MICE — Tier-2 *published* craft, explicitly legal per DL-0018 §2). **Swap** only: the `D-0128`/`D-0166` cites → camelot `d-000n`, and the TPtH `register-fit` / "hedge register wins" tension → a generic register. The TPtH pillar configuration (the D-0128 pillars, the hedge tension) **→WB**. |
| §7.1 `yaml` — the payload (`enfys`/`apprentice`/`gwern-threshold`/`ward-the-door`/`the withy held`/`p-hound`) | TPtH | **free** (yaml, oracle-invisible) — rewrite to a camelot payload (cast `myrdin`/`the-enchanter`, scene at `camelot`). The full TPtH payload **→WB**. |

### `09-exports.md` → `docs/worlds/tpth/turning-export.md`  (1 dli + 3 json + 1 yaml)

| Locus | Now | Disposition |
|---|---|---|
| §2 `yaml` — profile `cards/lowlands@3` | TPtH | **free** — **swap** to `cards/french-deck` (ships in `examples/games/french-deck/profiles.json`). |
| §2 `json` — manifest `profile: cards/lowlands@3` | TPtH | **free** — swap to french-deck. |
| §5 prose — "the dli-hosted Turning rules", `games/ludii` | TPtH | **swap** to `games/french-deck-ludii` (ships); the GDL / differential machinery prose is unchanged. |
| §6 whole "worked card export" — deck anatomy `D-0148`, honest/keep-stamped/salt-fringe, `the-debt-square`; 3 json + 1 dli `variant turning/keep-stamped` | TPtH | **→WB** (the full Turning worked export is the worldbook centerpiece) + **swap** to a french-deck worked export. The dli `variant` block → a french-deck `variant` (author; parse-risk). **Variant-loss gap:** french-deck ships no variant; a public-domain **piquet** delta (32-card, drops 2–6, loses the `the-full-suit` layout) is the analogue of keep-stamped losing `the-debt-square` (Hard Call 4). |

### Lightly `10-serving.md` (0 dli, 8 json) and `11-agent-authoring.md` (1 dli, 27 json)

A light residue pass, not a full split; no worldbook counterpart required (a
cross-link is optional):

- `10-serving.md:166` — json atom `reachable(gwern, glass-weald)` + `D-0119`/
  `RQ-031`/`P-214` → **free** swap to `reachable(camelot, avalon)` with a camelot
  `d-000n`. No dli, so zero oracle risk.
- `11-agent-authoring.md:9` — "the coding agents that build *To Pleach the
  Hedgeway*" → **swap** to generic ("the coding agents that build a world");
  `:181` json `"decisions":["D-0104"]` → **free** swap. Verify the single dli
  block is already generic.

### Not split (confirmed generic — leave)

`00-overview`, `01-syntax`, `02-semantics`, `07-verdicts`, `08-reasoning`. A light,
**oracle-safe** residue exists — `01` uses `scope saltmarket` (its block parses),
`07` uses "is Enfys Welsh?", `08` names the Concordance/Turning — out of §D's
mandated split. Optional one-line swaps while in-tree; not required by Phase 5.

---

## The worldbook layout — `docs/worlds/tpth/`

```
docs/worlds/tpth/
  README.md          # what TPtH is (one paragraph); the index; links to worlds/tpth/ data
  time.md            # <-> spec 03 : era spine, the cycles + leaf(29), the water/enclosure
                     #               fluent case, the D-0104 narration frame
  concordance.md     # <-> spec 04 : the spoken-proof practice (linear-conserved debt, the nine
                     #               errors, licensed-ignorance / the rigged-deck reading),
                     #               the e01-s04 proof ledger, Frith conservation
  language.md        # <-> spec 05 : cymric-fringe, Enfys, the hedgeway/the-plenty lexicon, the
                     #               banned machine/algorithm rationale, the cymric agent-init
                     #               bundle, the "is Enfys Welsh?" attestation seam
  craft.md           # <-> spec 06 : the D-0128 pillars, the hedge-register tension, TPtH's
                     #               principle configuration + ten-battle lexicon, the ep-01 payload
  turning-export.md  # <-> spec 09 : the Turning deck anatomy (D-0148), the cards/lowlands profile,
                     #               keep-stamped/salt-fringe variants, the-debt-square, the ludii differential
```

Each file opens with a **banner** cross-linking its spec chapter and the
`worlds/tpth/` **data** it documents, then holds the **extracted fenced blocks
verbatim** (they already parse) plus the diegetic prose the spec should not carry.
Cross-references point three ways: **up** to the spec chapter, **out** to the
`worlds/tpth/` data (`packs/cymric-fringe.dli`, `turning/lowlands.json`,
`craft/principles.yaml`, `practices/spoken-proof.dli`, `time/cycles.dli`), and
**across** where lore interlocks (`concordance.md` ↔ `language.md` for
`kind binder from lexicon.loan-words`).

**Home decision — `docs/worlds/tpth/`, not beside `worlds/tpth/`.** Decisive
because the docs-oracle gates ` ```dli ` under `docs/` but does **not** scan
`worlds/` prose. Worked dli examples placed beside the data would silently escape
the parse gate (Hard Call 2).

---

## What must NOT move or change

- **`../decisions.md`** — the append-only ledger, the authority of record. Never
  move or renumber a `DL` entry.
- **The charter koans** (`../charter.md`) — the constitution; generic by
  construction. The koan-8 / koan-10 asides that name Frith and the Turning are
  *illustrations of a principle*, not a worked world — leave them, or trim to one
  neutral clause. Do not relocate koans.
- **The already-generic specs** `00`/`01`/`02`/`07`/`08`.
- **`../reference-generated/`** — emitted by `cargo xtask docgen`; never
  hand-edit.
- **`docs/design-notes/*`** — engine rationale/history; the TPtH-named ones are
  *about* extraction. Leave in place.
- **The craft-doctrine names** (`sanderson`, `kishotenketsu`,
  `tryfail-before-triumph`, MICE) — Tier-2 *published* craft, legal to name
  (DL-0018 §2). Do not treat them as TPtH content.

---

## The ordered green DAG

Invariants held at every step: every ` ```dli ` block parses; `SKILL.md` keeps
≥ 3 dli blocks and total units ≥ 5; `book/src/SUMMARY.md` includes every new
file; no dangling cross-link (a worldbook file exists **before** a spec links to
it). Verify each step with `cargo xtask` (the docs-oracle) plus a local
`mdbook build` (non-gating, but it catches a broken SUMMARY or link).

| Step | Work | Touches | Type | Green because |
|---|---|---|---|---|
| **0. Scaffold** | Create `docs/worlds/tpth/README.md` + the 6 chapter files, each holding the TPtH blocks **copied** (not yet removed) from the specs; add 6 `book/src/worldbook-*.md` include stubs; add a `# The TPtH worldbook` section to `SUMMARY.md`. | SUMMARY, stubs | prose-add | Copied blocks already parse; duplication is fine (the oracle does not dedupe); worldbook→spec links resolve; mdBook includes all. |
| **0b. (opt.) Grow camelot** | Add to `examples/camelot.dli`: `cycle moon(29)` / `cycle season(4)`; a `rule … over order`; `basis attested` on `old-speech`; a neutral `banned` term; a generic agent-init bundle; and **(Hard Call 1)** a `practice oath-binding` with `resource boon : linear conserved` + a licensed-ignorance-shaped error. | examples/ | dli-author | Each new construct is parse-verified once by the oracle (camelot is a gated example). Lets Steps 1–5 **excerpt** rather than inline-synthesize. |
| **1. Split 03** | Remove the TPtH dli/text from `03-time.md`; insert camelot excerpts (era spine, states, cycles, order-rule); rewrite the episode text block; add the worldbook footer link. | 03 only | prose-move + swap | Camelot excerpts proven; cycles/order-rule from 0b. No SUMMARY change. |
| **2. Split 04** | Remove spoken-proof / true-naming / proof dli; insert `merlins-craft` + (0b) `oath-binding`; rewrite the two `invalid:` text blocks; footer link. | 04 only | prose-move + swap | `merlins-craft` ships; the conserved practice comes from 0b (or a thin cross-link per Hard Call 1). |
| **3. Split 05** | Remove cymric / Enfys / lexicon / agent-init dli; insert `old-speech`/`myrdin` + (0b) banned/agent-init; rewrite the `gwyneth` text; footer link. | 05 only | prose-move + swap | `old-speech` ships; lexicon/agent-init from 0b. |
| **4. Split 06** | Keep the craft-doctrine names; swap the D-cites + the hedge tension to camelot; rewrite the payload yaml to a camelot cast; footer link. | 06 only | swap | yaml is oracle-invisible; the one dli block edits are cite-only, so it still parses. |
| **5. Split 09** | Swap the profile/manifest/deck json+yaml to french-deck; author a french-deck `variant` dli block (or a thin cross-link per Hard Call 4); rewrite the diff text; footer link. | 09 only | swap | french-deck profiles ship; the **new dli `variant` block is the parse-risk to verify**. |
| **6. (opt.) Light residue** | The `10`/`11` (and optionally `01`/`07`/`08`) prose/json swaps. | 10, 11 | prose-only | No dli change in `10`; `11`'s one dli block verified generic. |
| **7. Reframe integration** | Rename the `SUMMARY.md` section to `# A worked integration: the fantasy factory`; add a one-paragraph "one consumer of the generic `dli-worker` contract" framing header to `fantasy-contract.md` + `migration.md`. Coordinate with the in-flight `dli-worker` rename. | SUMMARY, integration | prose | No dli in `integration/`; heading + framing only. |
| **8. (opt.) Cards** | Genericize `expert-card.md` (13 TPtH tokens / 14 D-cites) + `authoring-loop.md` to camelot; `gentle-card`/`syntax-reference` are near-generic already. | cards | swap | A sibling to the split, not a blocker. |

Steps 1–6 are pure prose-moves + swaps and do **not** touch `SUMMARY.md`; only
Steps 0 and 7 do.

**Size estimate.** 5 specs split + 2 light; **~11 dli blocks relocated** to the
worldbook; **~6–8 new/rewritten generic blocks** — the parse-risk set (03 cycle +
order-rule, 04 conserved practice + proof, 05 banned + agent-init, 09 variant),
all absorbed into camelot/french-deck at Step 0b so they land as **excerpts**, not
inline synthetics; **6 new worldbook files + 6 stubs + 1 README**; `SUMMARY.md`
touched twice; `integration/` reframed. Roughly **a dozen edited files + ~13 new
files**.

---

## The hardest calls (recommendations adopted)

1. **`04`-§3 has no shipped generic `linear conserved` + licensed-ignorance
   exemplar.** `merlins-craft` is graded-*monotone*, so it maps to §4's
   arbitrariness proof, not §3's conserved-resource worked instance. **Adopted:**
   grow camelot (Step 0b) with a minimal `practice oath-binding` — a
   public-domain "a boon owed must balance" carrying `resource boon : linear
   conserved` and one licensed-ignorance-shaped L2 error — so §3 keeps teaching
   conservation + licensed-ignorance generically, while the full nine-error
   Concordance treatment lives in `concordance.md`. *Fallback:* teach §3's
   mechanism on `merlins-craft` and cross-link the conserved/licensed-ignorance
   depth to the worldbook (lighter, but the spec then carries no worked generic
   conserved example).

2. **Worldbook home → `docs/worlds/tpth/`** (decisive). The oracle gates dli under
   `docs/` but not `worlds/` prose; placing worked dli beside the data would
   escape the parse gate.

3. **`integration/` is all consumer-space, none worldbook.** The worldbook is
   world *content*; integration is the consumer *wire*. `migration.md` §10's
   worked TPtH Door-T scenario contains lore but *illustrates the consumer flow* —
   it stays in `integration/` as a worked consumer scenario, optionally
   cross-linking its lore terms to the worldbook.

4. **Deep re-illustration vs cross-link.** Rule: the spec teaches the **mechanism
   on a generic world**; the worldbook carries the **TPtH depth**. Deep where a
   shipped construct exists (`03`→camelot, `05`→old-speech, `09`→french-deck);
   thin cross-link where authoring a generic equivalent is heavy. **Adopted:** add
   the french-deck **piquet variant** (public-domain 32-card, drops 2–6, loses
   `the-full-suit`) so §6 keeps a *live* variant-loss demonstration parallel to
   keep-stamped losing `the-debt-square`.

5. **The cards.** Not in §D's mandated split — a sibling pass. `expert-card.md` is
   an engine-surface cheat-sheet, so genericize it to camelot (it references the
   *engine surface*, which camelot exercises), rather than relabel it worldbook.
   `gentle-card`/`syntax-reference` are near-generic (trivial swaps).

6. **Cycles and the Turning's leaf(29).** tide/moon/season are public-domain
   natural cycles; only *leaf(29)* is TPtH. Camelot ships no `cycle`, so `03`
   cannot currently excerpt one. **Adopted:** grow camelot with `cycle moon(29)` /
   `cycle season(4)` (Step 0b) — it also completes camelot as a full `03-time`
   exemplar. The "same cyclic kind as the Turning's leaf(29)" cross-ref moves to
   `time.md`.

**Meta-recommendation.** Do Step 0b (grow camelot + add the french-deck piquet
variant) so **every** spec example is an excerpt from one gate-parsed shipped
world rather than an inline synthetic block. It front-loads the only parse-risk
into the two example worlds — where the oracle proves them once — and makes Steps
1–5 pure, low-risk excerpt-and-swap.
