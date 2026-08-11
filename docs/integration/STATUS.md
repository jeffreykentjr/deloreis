---
status: live
verdict: "The deloreis-side coordination board for the fantasy integration. Single-writer (deloreis). Every coordination point starts at proposed; the paired fantasy-side file is the other half of the handshake."
---

# Integration STATUS — deloreis side

This is the **deloreis-side** coordination board for the fantasy integration. It
is **single-writer**: only deloreis writes this file. The fantasy repository
keeps its own mirror, written only by fantasy. Read together, the pair is the
handshake — neither side edits the other's file, so there is no write race.

**States**, in order:

- **proposed** — the coordination point is defined; no artifact exchanged yet.
- **provided** — deloreis has delivered its side (a tagged build, a fixture, a
  harness slot).
- **acknowledged** — fantasy has received and vendored/consumed it.
- **verified** — both sides' fixtures/transcripts agree; the point holds.
- **closed** — verified and no longer active work.

## Current cutover snapshot — 2026-08-11

The execution log below is retained as historical evidence. This snapshot is the
active coordination truth after the Contract 1.5 cutover:

- deloreis `main` is `63d5a3c` (PR #55), with the retired-authority vocabulary
  scrub and its `retired-vocabulary` gate landed. Contract `1.5.0` is immutable
  at `8c0641ac`; the corrective store/display-name hotfix is
  `contract/1.5.0-hotfix.1` at `dd75afaa`.
- Fantasy `main` is `de22b111` (PR #31, following PR #30). It pins the
  corrective Contract 1.5 build, produces `parity.json` from the lorestore
  export, carries the parity-chain check as a required offline path, and
  records the bounded seeded canary and FW-8 authoring proof. The migrated
  authority fallbacks are deleted on that main.
- The current Fantasy projection is bound to tag
  `contract/1.5.0-hotfix.1`, store head
  `sha256:9deb4d7b39fa5e5f6843519b7e3462448d5baf3aa0681bbe5429d51729677df2`,
  and projection digest `sha256:dfff1a9fdbbf846a1a9d778fb0b47a9d3094065446ed82f8bc2a714979b0db5d`.
- FW-6's killed-token, facts, timeline, and character projection work, plus
  authority-fallback deletion, is complete. CP-5's bounded seeded canary and
  CP-10's FW-8 authoring proof are committed on Fantasy main; the Contract 1.5
  engine-side standing tests and the real MCP transcript cover cited-provisional
  canon reach, promotion, defeat, and object retention.

The rows below record the current handoff state; older paragraphs later in this
file are evidence logs and must not be read as a reversion to their old pins or
version claims.

The coordination points and what each side owes are defined in `migration.md`
§7. This board tracks their state and links the evidence.

| CP | What deloreis provides | What fantasy provides | State | Evidence |
|---|---|---|---|---|
| **CP-1** — contract freeze | v0 schemas + the closed fixture registry | the vendored copy + a parity run | provided | current deliverable tag `contract/1.5.0` at `8c0641ac`; corrective tag `contract/1.5.0-hotfix.1` at `dd75afaa` |
| **CP-2** — pinned oracles | the shadow-read harness slot | the pinned legacy gates (EXT-F1) | provided | `scripts/shadow-read.sh` and `scripts/pinned-oracles.sh` remain the lab-side slot; the historical oracle pin needs re-pin before this row can close |
| **CP-3** — vendored corpus | the dual-run consumer | canon snapshots at a pinned commit (EXT-F2) | acknowledged | Fantasy `main` `de22b111` consumes the Contract 1.5 lorestore pack and projection; the current store head and tag are recorded above |
| **CP-4** — detection corpus | detection proven separately | the seeded divergence corpus (EXT-F5) | acknowledged | Fantasy's 33-operator mutation corpus is present and exercised by its offline test gate; the post-projection harness repair is present on `de22b111` |
| **CP-5** — core shadow-read | dual-run parity (validator/continuity/factcheck) | the pinned oracles + corpus | verified | Fantasy's bounded seeded canary records clean parity, five own-tag mutation reds, and rollback rehearsal; the broad advisory predicate sweep remains non-gating |
| **CP-6** — naming shadow-read | the LANG-2 kernel-half view | the legacy `name vet` + external screens | provided | Contract 1.5 character/name projection is live; the consumer retains its external naming screen, with full naming parity still a separate follow-up |
| **CP-7** — the batch RQ | the offset→before compiler + the draft RQ | the author's ruling | acknowledged | machine-attributed D-ledger entries are now permitted by the consuming governance ruling; the former synchronous author block is no longer a release blocker |
| **CP-8** — source deletion | the conversion + the reviewed diff | the review + the deletion commit | verified | Fantasy PR #27 deleted the four migrated authority fallbacks after the projection producer flip; its required Python and Factory checks passed |
| **CP-9** — worker on the contract | `deloreis serve` + a tagged build | the fantasy worker on the vendored contract (EXT-F3) | verified | Fantasy `main` `de22b111` pins `contract/1.5.0-hotfix.1`; contract pin, projection provenance, boundary checks, and the full local gate are green |
| **CP-10** — authoring-tools handshake | a tagged build with the `author.*` tools + the worked-scenario fixture | their agent exercises the scenario against the tagged build + posts the transcript (EXT-F4) | verified | historical AUTH-2 replay remains verified; Contract 1.5 engine tests additionally cover cited provisional canon reach, standing promotion, defeat, and append-only object retention |

The source tree currently implements contract `1.5.0`, served **multi-version
dual-mode**: it accepts `0.5.1`, `0.6.0`–`0.6.4`, `1.0.0`, `1.1.0`, `1.2.0`,
`1.3.0`, `1.4.0`, and `1.5.0`, answering each caller in the version it
declared. A `0.5.1` request still receives byte-identical `0.5.1`-shaped
responses; the compatibility fixtures remain the standing suite. Contract 1.5
adds the closed standing vocabulary, cited-state boundary, and ordinary
`author.set_standing`/`author.defeat` data operations (DL-0046, DL-0051,
DL-0053, DL-0055, DL-0056, DL-0058, DL-0070, DL-0071).

The original `contract/1.0.0` milestone at `76888052` remains historical
evidence for the frozen AUTH-2 fixture. The active deliverable is the Contract
1.5 tag pair recorded above; the consumer's current pin and projection digest
are the evidence for the present handshake. CP-10's historical transcript is
deliberately not re-run merely because the fixture remains frozen at `0.5.1`
terms; the Contract 1.5 engine tests are the current standing-transition proof.

When a point advances, deloreis updates its **State** and fills **Evidence** with
a link (a fixture path, a tagged build, a transcript). A point never advances
without its evidence.

## Cutover execution log

### 2026-08-06 — Phase 0 closed (both lanes); §5 accrual proven on a real binary

**deloreis (`origin/main` = `6a626e1`, DL-0060 + DL-0061):** the `finding-sanitize` lane
(DL-0061) is committed, gate 21/21 green, red-team cleared (single-serde-egress bypass
sweep; non-vacuity pins `message_is_control_char_escaped_on_the_wire` +
`a_clean_message_serializes_unchanged`; 0 schema drift; DL-0061 honest-deferral), and
auto-squash-merged. Merged tree ≣ `daff4fe`. CI green post-merge (deny/gate/no-ai-authorship/
default-build/docs-oracle `success`). Evidence: `docs/decisions.md#dl-0061`;
`crates/dli-serve/src/finding.rs` (`Finding.message` `#[serde(serialize_with=serialize_message)]`,
reusing `author::render::escape_controls`); 25 regen'd schema `description`s.

**fantasy (fw6-flip `b519115a`, green NOT yet merged):**
- Store census reconciled: `558 = 556` claim-bearing facts `+ 2` system (seed sentinel
  `b4/9f784…` + root manifest `db80a5a3…` ≡ `lorestore-head.txt`); `543` distinct claim_ids
  (13 `decl:ratify` dupes); content-addressed `558/558`. OPEN #1 closed.
- 5-mutation battery audited; `KNOWN_DIVERGENCES = {}`. Non-vacuity **13/13 offline green**
  (`tests/test_lore_pivot_dual_serve.py` + `tests/test_shadow_lore_live.py`, incl.
  `structural-only → 5` not `0`).
- §5 accrual, real binary `contract/1.3.0`+DL-0061: `lore_dual_run.sh` full pass green —
  **217 predicates answered identically / 6 refused / 0 divergences** (parity at both
  ends); 5 mutations RED at own tags; pack reproduces from `.dli`; store root `db80a5a3…`
  reproduced **exactly**; rollback rehearsal clean. Plus `shadow_lore.py --repeat 20` →
  AGREE×20, 0 DIVERGENT, worktree clean.

**Phase 1 re-scoped (two stale-plan landmines removed):** `detect_parity.py` was already
deleted at FW-1 (`b9947240`, ancestor of the flip base — confirmed via
`git merge-base --is-ancestor`); `shadow_validate.py` already exists as
`scripts/shadow_factcheck.py` (strict `{code,line,quote,severity}` + `PROMPT_DIGEST` +
`--repeat`, non-vacuity proven green 13/13). Phase 1 = VERIFY, not BUILD.

**CP state (honest):** deloreis-side evidence for CP-2/CP-4/CP-5 is committed (`origin/main`);
fantasy-side evidence (the shadow harness/factories in fw6-flip) is green but **unmerged** —
so CP-2/CP-4/CP-5 remain `proposed` until the fw6-flip lane merges (no point advances on
one side's evidence alone). Do not hand-fabricate; the `compile_lore_pack --check` ≣ committed
pack already rules out a canned corpus.

**§5 ≥1-real-ruling — CLOSED (scratch-store proof; real author.* MCP):** A real canon ruling
was authored via `author.*` MCP (`deloreis mcp --project`) on a throwaway copy of the
tpth store (`/tmp/s5`, NOT the committed `worlds/tpth`). `author.open→add_entity→materialize`
staged the rule (workspace digest `9db59135…→6deb725d…`); the engine surfaced an honest
`name.unvetted` finding (grade `asserted`, repair `dli.name.check {name:"s5witness"}`) —
it reported a consequence instead of silently accepting an unvetted name; `materialize`
committed it (`ok:true`, `__decl Entity s5witness`, claim_id `a7fb7833…`). Root shifted
`sha256:db80a5a3…→ba63c029…`, persisted (re-read by a fresh `deloreis mcp --project` process).
`dli.check` held `checked`/clean (0 findings); `alias(?v0,?v1)` ×20 → AGREE (12 rows, 1
distinct frame). Committed `worlds/tpth` byte-identical (`db80a5a3`). §5 accrual re-verified
on the restored dm4fix binary: `lore_dual_run.sh` PASSED @ `69dbc1ea…`/`db80a5a3…` (217
identical / 6 refused / 0 divergence, 5 mutations RED, rollback PASSED); `shadow_lore --repeat 5`
→ AGREE (274×5). §5 no longer blocks; Phases 3–4 blocked only on the FW-6 flip + EXT-F4.
Evidence: the lane's `fw6-flip-verification-log-2026-08-06.md` §5-closure section. NOTE: the
handoff's `/home/jeff/tg-findingsanitize/debug/deloreis` was clobbered by a deloreis-main
rebuild (no store surface) — FW-6/MCP work uses the dm4fix binary at
`/home/jeff/tg-dm4fix/debug/deloreis` (contract/1.3.0).

### 2026-08-07 — Phase 2 path A1 CLOSED: killed-tokens canon domain verified (gate 21/21 GREEN)

**deloreis canonical projection (committed via DL-0064/DL-0065):**
- `CorpusExportFormat` gained `Canon`; `CorpusExportResult` gained `canon: Option<serde_json::Value>`.
- `canon_export()` (`handlers.rs`) folds the live `FactObject`s at head into a
  `BTreeMap<String, Vec<Value>>` keyed by `atom.pred`, returning `{by_pred, interner, decisions, killed_tokens}`.
- Schemas regenerated (`xtask schema`, 2 drift files: `dli.corpus.export.{request,response}.json`).
- Live probe over a throwaway copy (`/tmp/s5b`) with the rebuilt dm4fix: `--canon` returns all
  74 predicates bucketed (`__decl`=149, `source-kind`=23, `name-note`=20, `person`=5,
  `place`=8, `thing`=15, `event`=9, `era`, `before`, `name`, `name-part`, `dialect`,
  `factcheck-alias`=11, plus ~50 lore fact-predicates). `decisions` census = 68 ratified ids;
  `interner` (68 symbols, 0-based) carries decision ids, so `support` monomials
  (`+N` → `interner[N]`) ARE resolvable — byte-identical projection is real, not fantasy.

**killed-tokens blocker — RESOLVED by DL-0064 + DL-0065.** The circular
contradiction (storing a banned token fires the checker that bans tokens) is
broken at the predicate boundary: DL-0064 exempts the `killed` registry fact at
all three scan sites, so `fact killed-token-registry <json>` materializes; the
token remains refused everywhere else under `factcheck.contradicted`.
`canon_export` then projects the killed-tokens domain purely from the folded
objects, byte-identical to committed `worlds/tpth/generated/parity.json`:

- `killed` (20): `fact killed-token-registry <json>` rows emitted as the verbatim
  registry-row object. `match` is read verbatim from the stored JSON (not
  re-derived — `doom-wight`/`doom-ward` are `"phrase"` despite no space).
- `worker_projection` (19): killed rows whose token is **not** a ratified
  surface — 4-key `{token, match, decision, scope}` (no `kind`).
- `excluded_ratified_surface_collisions` (1 = Frankpledge/D-0138): killed rows
  whose token **is** a ratified surface (Frankpledge is a real declared entity);
  full field set.
- `rejected_claims` (5): `factcheck-object-{key}` facts expanded through the
  interner via `support` monomials, in commit order. `gender` is excluded
  (ratified → no `factcheck-reject-*` cue); `mortality` is excluded (its slot is
  owned by a plain asserted `fact`).

**Colophon scope split (DL-0065):** the store fact carries `scope` as a single
flat string; the projection `split(", ")`s it and rebuilds the sibling columns
YAML parsed as null-valued keys (`scope: a, b, c` → `scope="a"`, `b=null`,
`c=null`) — today only the Colophon row. Commit order is carried by
`serialized_objects` (the ordered Vec), not the `by_pred` BTreeMap.

**Verification.** `canon_export{killed_tokens}` over `/tmp/s5b`
(root_hash `sha256:6a165d81…`, 578 objects, interner 68, decisions 68)
byte-identifies to committed `parity.json` (`sort_keys=True` equality):
`killed: 20/20`, `worker_projection: 19/19`, `rejected_claims: 5/5`,
`excluded: 1/1`, AND `by_pred`/`interner`/`decisions` match. `cargo xtask gate
--ci` → `PASSED — 21 steps green`. Independent red-team (fresh-context
reconstruction from the raw `objects` export + a planted divergence) passes GREEN
before the plant and RED after. Full empirical derivation: `docs/design-notes/LS-7-killed-tokens-canon.md`.

### 2026-08-07 — Phase 2 store-migration gate CLOSED (committed re-seed `b19b0740`)

**Committed-store re-seed, reversible.** Authored the 20 killed-token registry
facts (cited D-0108) into the committed `worlds/tpth` lorestore via `author.*`,
re-rooting `db80a5a3` (556 objects) → `6a165d81` (578 objects; +22 = registry
`thing`+`__decl` decl + 20 `killed`). The killed-tokens canon domain is now
reproducible against the **committed** lane: `canon_export{format:"canon"}` on the
committed manifest yields `killed=20 / worker_projection=19 / rejected_claims=5 /
excluded=1`, byte-identical to committed `generated/{parity.json,killed-tokens.yaml}`
(`git diff` empty); `check-lore-cutover-boundaries.sh` stays green. Non-vacuous
red-team (plant divergence on `/tmp/s5b` ≡ committed `6a165d81` → RED → restore →
GREEN) passed. Gate: `cargo xtask gate --ci` → 21/21 green. The canonical lane is
no longer `/tmp/s5b`-only; `/tmp/s5b` is re-synced to `6a165d81`.

**Paths B–E feasibility (byte-exact fold probes against `by_pred`):**
- `facts.factcheck_aliases` (11) — 1:1 `factcheck-alias` args `[subject,surface]` →
  `{subject,surface}`. **Byte-exact. Fully canon-derivable.** (mechanism validated)
- `facts.relations` (12, 8 predicates) — `{subject, kind←predicate, object, decisions←support→interner
  sorted, status:'ratified'}`. Fold located (decl/interner join); byte-exact confirmation pending the
  ordered commit-order match (the field mapping is exact).
- `facts.entities` (23) — `id`/`kind`/`decisions`/`note` recoverable from
  `source-kind`+`name-note`, but the `name` surface for places/concepts
  (e.g. `The Keep-Stamped Deck`, `Whitecombe`) is **not a by_pred arg** (only the
  lowercased id + a `Name` decl survive) — a human-ratified surface → **not fully
  canon-derivable** without a name-source.
- `characters`/`names` — name surfaces likewise not all present as by_pred args
  → **YAML-sourced** (ratified registries), the projection carries only the
  by_pred-sufficient fields.
- `dialects.registers` — phonotactics live in `src/fantasy/naming/data/registers.yaml`
  (external), **not by_pred** → YAML-sourced (per LS-7).

**Remaining Phase 2 (B–E):** a *per-domain by_pred-sufficiency analysis*, not a
uniform port. `canon_export` will emit the by_pred-sufficient sub-maps
(`factcheck_aliases`, `relations`, event/era edges, entity decls,…) as canonical
projections; the human-ratified surfaces (`names`, `dialects.registers`, entity
`name` for concepts/places) stay as explicit YAML fallback sources in
`compile_lore_dli.py products()`. A ledger entry for the committed-store re-seed
should be minted once the contended single-writer `docs/decisions.md` is read back
in full (it is being appended by the parallel `finding-sanitize` lane — not
clobbered mid-flight).

## FW-6 flip checkpoint — Phase 2 B–E complete (canon_characters, DL-0068)

All three canon projections are committed, gated, and red-TEAM'd (fresh-context
reconstruction of the canon sub-maps from raw objects + planted divergence →
RED → restore → GREEN):

- **Path A** `canon_killed_tokens` (killed=20 / worker=19 / rejected=5 / excluded=1):
  deloreis `2a3730e` (DL-0065). Re-seed `db80a5a3`→`6a165d81` (578 objects).
- **Path B** `canon_facts` (relations=12, constraints=3): deloreis `a51f3bd`;
  fw6-flip `7189c9a2`. **DL-0066** (minted on `integration-status`, retrospectively
  recording the ruling the `canon_facts()` commit already encodes).
- **Path C** `canon_timeline` (before=4): deloreis `7e6aa56`; fw6-flip `65e6dd34`.
  **DL-0067** (minted on `integration-status`; before-edges `+7`→`interner[7]→D-0072`).
- **Path E** `canon_characters` (person=5, role=15, alias=12): deloreis `4c9afeb`;
  fw6-flip `c37b6490`. **DL-0068** settles the general rule: structural `by_pred`
  fields (person/role/alias, relations, constraints, before, killed) → canon;
  the human-ratified display `name` surfaces (`entities.name`, the guardrail
  `factcheck-alias` surfaces) stay YAML-sourced from `characters.yaml`/
  `cue_source`.

**finding-sanitize lane — resolved stalled.** The contending single-writer lane
on `docs/decisions.md` during paths B-E is **stalled**: its worktree is absent
from `git worktree list` and no orphaned commit holds DL-0066/0067 (`git log
--all --grep` clean) — the `agent-a1b5e029059455d70`/ls9-integration tree is a
separate BEHIND lane (DL-0049, not appending this ledger). DL-0066/0067 were
therefore minted retrospectively to keep the ledger's strict sequence
(`DL-0065→0066→0067→0068`; `cargo xtask authority` valid). DL-0065's re-seed
ledger entry remains live. **Path D** (factcheck-alias re-point onto canon) stays
**rejected** — byte-neutral already (canon `factcheck-alias` 11 rows == committed
11 lines; L657 keeps it YAML-sourced), DO NOT redo.

**Per-domain red-teams (all GREEN/RED/restore/GREEN):**
- facts: GREEN(12 rels)→PLANT `keep-stamped-deck adjacent_to bogus-redteam-realm`→RED(13, --check STALE)→restore→GREEN(12).
- timeline: GREEN(4 before)→PLANT `prosperous-cycles before story-opening` cited→Decided→RED("unknown before-atom provenance marker: Decided")→restore→GREEN(4).
- characters: GREEN(5/15/12)→PLANT `fact protagonist alias "bogus redteam alias"` cited→Decided→RED("unknown person/role/alias provenance marker: Decided")→restore→GREEN(5/15/12).

**Gate / check:** `cargo xtask gate --ci` (tg-findingsanitize target) →
`PASSED — 21 steps green`, `GATE_EXIT=0`. `compile_lore_dli.py --check` green in
BOTH canon (`DELOREIS_BIN=/home/jeff/tg-dm4fix/debug/deloreis`, default
`CANON_PROJECT` → committed `6a165d81`) and offline (`DELOREIS_BIN` unset) lanes;
`git diff worlds/tpth/generated/` empty. Serving binary `dm4fix` rebuilt
(`BUILD_EXIT=0`); `dm4fix` is NOT re-built after the clippy `#[allow]` (no-op) attr.

**Dialects / names YAML-only (LS-7 confirmed):** `names.dli`←`ratified_names.yaml`,
`dialects.dli`←`registers.yaml` — no `canon_<domain>` needed (these surfaces are
absent from `by_pred`; `person <id> "<name>"` is the YAML name surface, `name`
by_pred is the 3 ratified slug atoms). `cargo xtask docs-oracle` ("71 dli units
parse; 86 json blocks") + `cargo xtask authority` green.

**Phase 3/4 §5 authoring over `/tmp/s5`:** **blocked** on EXT-F4 / FW-6 flip land.
The authored canon surface (B-E) is in place; the `author.*` authoring session over
`/tmp/s5` is deferred until the flip lands. `docs/integration/migration.md` is the
in-flight handoff for that lane.
