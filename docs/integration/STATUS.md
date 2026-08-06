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

**Open gate (not fabricated):** §5 ≥1-real-ruling reproduction needs a real canon edit
recompiled through `compile_lore_pack` (pack digest visibly changes mid-streak) — a
canon-authoring step per AGENTS.md DL-0014/DL-0043 via the `author.*` MCP tools, absent
here. Phases 3–4 blocked until the FW-6 flip lands + EXT-F4 verified.

