# Mutation detection corpus (EXT-F5)

Data seeds for wave-3 **PROOF-1**: the ported fantasy mutation-operator corpus.
Each operator seeds one deliberate violation into a pristine world and names the
deloreis kernel **check class** that must flag it. This directory is DATA only —
no Rust, no gate wiring. PROOF-1 consumes these records to prove the deloreis
kernel catches every violation the legacy engine did (and inherits its one honest
hole, openly).

## Provenance

Ported from the fantasy repository at pinned SHA
`099e04cdbbbaf644842a2f4b7a23b2235f752311` (the same pin the vendored corpus is
cut from — `worlds/tpth/vendor/fantasy-099e04cd/`). Sources:

- `src/fantasy/mutation/operators.py` — the 33 seeded-violation operators.
- `docs/reference/lore-engine-gate-coverage.md` — the operator×gate coverage matrix.

Refresh procedure and the pin rationale: `docs/design-notes/vendored-corpus.md`.

## Files

| File | Target | Records |
|---|---|---|
| `canon.yaml` | canon-target | 9 |
| `draft.yaml` | draft-target | 20 |
| `episode.yaml` | episode-target | 4 |

**33 operators total.** In the source engine, **32/33** are caught by ≥1 gate;
**1** is a known hole (`paraphrased-contradiction`). The `expected_catch` field
is the **proposed** end-state mapping onto the deloreis kernel; PROOF-1 (the
`dli-world` `mutation_harness` suite) **verifies** it against the real gates and
records the wave-3 answer in each record's `kernel_outcome:` block.

**Verified wave-3 census** (`docs/design-notes/mutation-coverage.md` has the full
operator×gate matrix):

| `kernel_outcome.status` | count | meaning |
|---|---|---|
| `caught` | 22 | the expected kernel check fires with the right class |
| `known-divergence` | 1 | the class *dissolves* in deloreis (`registry-file-desync` — no mirror to desync) |
| `corpus-gap` | 3 | flagged as a defect, but the precise rejected-lore class needs a vendored constraint (`contradict-protagonist-immortal`, `assert-musical-magic`, `assert-narrator-ticking-clock` — D-0067/D-0002/D-0104) |
| `factory` | 3 | correct no-catch: factory-side by law (`slop-cluster`, `allegory-wink`, **and `killed-name-elara`** — see below) |
| `pending-gate` | 3 | the catching check lands in a later PLAN node (the three craft operators → CRAFT-2/PAY-2) |
| `miss` | 1 | the honest deterministic hole (`paraphrased-contradiction`) |

`cite-resolved-question` was a fourth `pending-gate` until the **B1
citation-existence gate** landed: a citation to an id with no backing declaration
(a resolved question is not a live ruling) now mints no authority — the claim fails
closed and `citation.unknown-decision` is raised — so it flipped `pending-gate →
caught`. A genuine detection gain from the security fix; the `xfail-strict` record
did exactly what it was written to do.

Two deviations from the proposed mapping the harness surfaced (findings, not
silent fixes — migration.md §3):

- **`killed-name-elara` is factory-side, not kernel.** Elara is not in the vendored
  `killed_tokens.yaml`; it lives only in `ai_tics.yaml`, and ai-tics stay
  factory-side by law (migration.md §2 work-list; spec 05 §3). Registry membership
  — not "is it an AI-tic" — is the kernel/factory ground truth: `killed-name-silas`
  re-homes to the kernel because Silas *is* in the registry (Q-0015).
- **Three rejected-lore operators are `corpus-gap`s.** The vendored `constraints:`
  section carries only `no-auditor-concept`/`no-singular-demon-king`/`no-monster-of-the-week`;
  the D-0067/D-0002/D-0104 rejections are not vendored, so the kernel flags those
  claims as *unsupported* (never silently passes them) but cannot cite the precise
  rejection. Vendoring the missing constraints upgrades them to clean contradictions.

## Record shape

```yaml
operator: <id>                 # stable operator name (matches the fantasy corpus)
description: <what it violates>
target: canon | draft | episode
seeded_change:                 # the DATA seed — exact edit/append/synthetic
  kind: append | edit | edits | write | synthetic
  ...
fantasy_gates: [validator, …]  # SOURCE ground truth: which fantasy gate caught it
fantasy_codes: [V3, …]         # source validator/continuity code, where applicable
expected_catch: [<class>, …]   # PROPOSED deloreis kernel check class(es) — PROOF-1 confirms
catch_discriminant: <sub-case> # for continuity/craft: which sub-case
side: kernel | factory         # per the spec 05 §3 vet split
kernel_gated: false            # present only when the kernel does NOT gate it
expected_miss: true            # present only for the honest hole
rationale: <why this class>
```

`expected_catch` is a **proposed** mapping onto deloreis kernel check classes,
grounded in the specs (cited per-record in `rationale`). It is the hypothesis
PROOF-1 verifies against the real kernel checks as they land; the fantasy
`fantasy_gates`/`fantasy_codes` are the immutable source-of-truth for what the
legacy engine actually caught.

## Check-class taxonomy

Deloreis kernel check classes (and the two factory-side screens), with the
fantasy gates that map onto each:

| deloreis check class | side | fantasy gate(s) | spec basis |
|---|---|---|---|
| `provenance` | kernel | validator (V3, V11), pack | 01 §1 (no status slot in the AST); 07 |
| `assertability` | kernel | pack, continuity (PROPOSED_AS_FACT) | 07 §4 (candidates structurally unassertable) |
| `reference` | kernel | validator (V5 dangling), continuity (UNKNOWN_ENTITY) | 03 §1; 02 |
| `uniqueness` | kernel | validator (V5 duplicate) | 05 §3 (intra-canon uniqueness) |
| `citation` | kernel | validator (V4) | lore SOP (cite a live question) |
| `registry-consistency` | kernel | validator (V7) | 05 §6 (the mirror dies → query-consistency) |
| `lexicon-ban` | kernel | validator (V8), allegory (modern-vocab) | 05 §4 (bans are cited lexicon facts) |
| `contradiction` | kernel | factcheck, continuity (WORLD/PRIOR) | 03; 04 §5; error code `contradiction` |
| `register-fit` | kernel | naming (wrong-register) | 05 §2 (register-fit is a compile verdict) |
| `killed-token` | kernel | naming (killed-name-*) | 01 §7; 05 §5 (killed-form registry) |
| `craft-payload` | kernel | craft | 06 (payload battery) |
| `style-slop` | **factory** | style | 05 §3 (AI-tics screen — our world, non-gating) |
| `allegory-explicitness` | **factory** | allegory (winking) | 05 §3 principle (genre-taste judgment) |

**The kernel/factory split matters for PROOF-1.** A check re-homes into the
kernel iff its ground truth is a fact about the *story's* world; it stays
factory-side iff it is a fact about *ours* (spec 05 §3). So `modern-vocab-*`
(a cited diction ban — story-world) is kernel `lexicon-ban`, while `slop-cluster`
(a machine-generation tell — our world) and `allegory-wink` (a genre-taste
judgment) are factory screens that **must never gate assertability**. PROOF-1
asserts a kernel catch for the 30 kernel-gated operators only; the 2 factory-side
operators are recorded so the corpus is complete, not so the kernel is expected
to flag them.

## Operator → expected_catch (index)

**canon** — `break-status-marker` → provenance+assertability · `flip-proposed-to-ratified`
→ provenance · `malformed-override-leak` → provenance+assertability · `dangling-link`
→ reference · `duplicate-id` → uniqueness · `banned-word-into-diegetic` → lexicon-ban
· `cite-resolved-question` → citation · `registry-file-desync` → registry-consistency
· `forged-promotion` → provenance

**draft** — `contradict-anchor-gender`, `contradict-protagonist-immortal`,
`assert-auditor-identity`, `assert-demon-king`, `assert-musical-magic`,
`assert-narrator-ticking-clock` → contradiction · `contradict-hound-mundane` →
contradiction+assertability · `modern-vocab-algorithm`, `modern-vocab-carbon` →
lexicon-ban · `killed-name-elara`, `killed-name-silas`, `killed-name-irondale`,
`killed-name-doom-wight` → killed-token · `wrong-register-name` → register-fit ·
`crowd-scene-cast`, `as-you-know-bob-leak`, `ornate-tag-cluster` → craft-payload ·
`slop-cluster` → style-slop *(factory)* · `allegory-wink` → allegory-explicitness
*(factory)* · `paraphrased-contradiction` → contradiction *(known miss)*

**episode** — `continuity-world-contradiction`, `continuity-prior-contradiction`
→ contradiction · `continuity-unknown-entity` → reference ·
`continuity-proposed-as-fact` → assertability

## The honest hole

`paraphrased-contradiction` (`expected_miss: true`) contradicts a fact by
paraphrase that matches no surface cue. The deterministic kernel cannot catch it;
only the opt-in cross-family semantic judge can, and that judge is a non-gating,
model-in-the-loop factory lane — the deloreis gate is offline and model-free by
law. It is carried as the corpus's one xfail-strict analog: a miss that is proven
and named, never hidden.
