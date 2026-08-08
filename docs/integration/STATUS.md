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

The coordination points and what each side owes are defined in `migration.md`
§7. This board tracks their state and links the evidence.

| CP | What deloreis provides | What fantasy provides | State | Evidence |
|---|---|---|---|---|
| **CP-1** — contract freeze | v0 schemas + the closed fixture registry | the vendored copy + a parity run | provided | tag `contract/1.0.0` at `76888052`; `docs/integration/fantasy-contract.md` at 1.0.0 truth on that commit |
| **CP-2** — pinned oracles | the shadow-read harness slot | the pinned legacy gates (EXT-F1) | proposed | — |
| **CP-3** — vendored corpus | the dual-run consumer | canon snapshots at a pinned commit (EXT-F2) | proposed | — |
| **CP-4** — detection corpus | detection proven separately | the seeded divergence corpus (EXT-F5) | proposed | — |
| **CP-5** — core shadow-read | dual-run parity (validator/continuity/factcheck) | the pinned oracles + corpus | proposed | — |
| **CP-6** — naming shadow-read | the LANG-2 kernel-half view | the legacy `name vet` + external screens | proposed | — |
| **CP-7** — the batch RQ | the offset→before compiler + the draft RQ | the author's ruling | proposed | — |
| **CP-8** — source deletion | the conversion + the reviewed diff | the review + the deletion commit | proposed | — |
| **CP-9** — worker on the contract | `deloreis serve` + a tagged build | the fantasy worker on the vendored contract (EXT-F3) | provided | tag `contract/1.0.0` at `76888052` |
| **CP-10** — authoring-tools handshake | a tagged build with the `author.*` tools + the worked-scenario fixture | their agent exercises the scenario against the tagged build + posts the transcript (EXT-F4) | verified | tag `contract/1.0.0` at `76888052`; the AUTH-2 worked-scenario fixture under `worlds/tpth/auth2/`, replayed by `cargo test -p dli-serve --test auth2`. Acknowledged and verified together at the Stage-3 close: the consumer ran EXT-F4 against the tagged build and their posted transcript is byte-parity verified against that fixture (fantasy `e4a51fa2`) |

The source tree currently implements contract `1.3.0`, served **multi-version
dual-mode**: it accepts `0.5.1`, `0.6.0`–`0.6.4`, `1.0.0`, `1.1.0`, `1.2.0`, and
`1.3.0`, answering each caller in the version it declared, so a `0.5.1` request
still receives byte-identical `0.5.1`-shaped responses and every vendored v1
fixture remains in the registry as the standing compatibility suite (DL-0046,
DL-0051, DL-0053, DL-0055, DL-0056, DL-0058). All six export emission branches
are implemented.

Engineering readiness became delivery when the annotated tag `contract/1.0.0`
was cut on merge commit `76888052`, the commit whose tree carries the contract
doc at 1.0.0 truth. That tag is the deloreis deliverable named by CP-1, CP-9,
and CP-10, so those three rows stood at `provided`. Two of them still do:
`acknowledged` needs the consumer's vendored SHA and digest-check output, and
`verified` needs their replay artifact — neither is evidence deloreis can
produce for itself.

**CP-10 is now `verified`.** The consumer ran EXT-F4 against the tagged build and
posted the transcript; it is byte-parity verified against the AUTH-2 fixture this
side owns. The row moves from `provided` straight to `verified` because both
rungs were earned at once and are recorded together — the acknowledgement is in
the evidence cell rather than lost. Later tags (`contract/1.2.0`, `contract/1.3.0`)
do not restate CP-10: the fixture is deliberately frozen at `0.5.1` terms, so the
transcript that verified it is not re-run per tag.

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

**deloreis canonical projection (committed via DL-0063/DL-0064):**
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

**killed-tokens blocker — RESOLVED by DL-0063 + DL-0064.** The circular
contradiction (storing a banned token fires the checker that bans tokens) is
broken at the predicate boundary: DL-0063 exempts the `killed` registry fact at
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

**Colophon scope split (DL-0064):** the store fact carries `scope` as a single
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

