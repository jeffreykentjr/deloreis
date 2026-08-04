---
status: live
verdict: "The deloreis-side campaign plan for the full consumer cutover: EXT-F4 on 0.5.1 terms, the JSON-native serving and authoring capabilities the consumer needs (loader, corpus ops, expert tools), the authority-model scrub, validator governance, and the post-EXT-F4 retirement to contract 2.0.0 — with per-phase exit criteria, red teams, and the cross-repo orchestration rules."
---

# Cutover campaign — deloreis side

This is the deloreis half of a two-repo campaign plan. The fantasy repository
carries the matching consumer half (`docs/proposals/DP-013-deloreis-full-cutover.md`
in its own tree); the `STATUS.md` pair remains the live handshake. Neither repo
writes the other's tree; this doc governs only deloreis work. It is written for
one orchestrating agent running an agent team across both checkouts concurrently.

Authored 2026-08-04 from a verified two-repo survey. Where this doc disagrees
with `docs/decisions.md`, the ledger wins; the campaign's rulings are minted as
DL entries at their landing PRs (provisional numbers below — mint order is merge
order, single-writer, mint-and-use; the numbers shifted by one once DL-0052 took
the governance ruling). Those entries are minted by the agents that decide them,
each carrying its `machine`/`human` attribution (DL-0052).

## 0. Preconditions and first acts

- **Sync the checkout.** The long-lived local checkout has sat on
  `native-lineage-negatives`, strictly behind `origin/main` with zero unique
  commits (verified); local `main` is stale. Every work branch cuts from
  `origin/main` (`dccece5` at survey time). Delete `native-lineage-negatives`;
  the work its name suggests (direct-SQL lineage negatives) is consumer-side
  (their DP-008 X6), not ours.
- **Ground truth at survey time**: contract `1.0.0`; accepted versions
  `0.5.1`, `0.6.0`–`0.6.4`, `1.0.0`; the server echoes the caller's version;
  the field-level killed forms fire at major ≥ 1; the 394-frame
  `ls8_zero_x_parity` oracle guards every 0.x wire byte-exact in the gate.
- **No git tag exists yet** (`git tag -l` empty) — yet CP-1, CP-9, and CP-10
  all name "a tagged build" as the deloreis deliverable, and the contract §5
  sync protocol requires one. Cutting the first tag is the highest-leverage
  single action in the campaign.
- **EXT-F4 completes on 0.5.1 terms.** The AUTH-2 worked-scenario fixture
  (under `worlds/tpth/auth2/`, replayed by `cargo test -p dli-serve --test
  auth2`) pins `"dli": "0.5.1"` in every frame. Its bytes are the CP-10
  comparison artifact and are frozen (C4): the fixture is **not** rewritten to
  1.0. Consequence: the 1.0.0 tag can be cut with **no code change**, and the
  consumer's EXT-F4 transcript run is decoupled from their 1.0 wire upgrade.

## 1. The authority-model scrub (author ruling, 2026-08-04)

A doctrinal correction from the author, minted as a DL entry in phase P1 and
implemented on the 1.x wire in phase P5:

> All callers of deloreis are machines — agents, or upstream agentically
> mediated pipelines. Those callers are responsible for providing clear
> attribution and decision state. A machine files things as proposed while they
> await human review; once the human rules, the machine is still the thing that
> cascades the approval down. Role enforcement ("who may rule") cannot live at
> the deloreis level — it is a higher-level application concern.

What this scrubs, and what it keeps:

- **Scrubbed**: the Door-T refusal of citation-bearing provenance from agents
  (`provenance.cited-refused`, `author/tools.rs`), `SKILL.md` §0's "you cannot
  cite" law, and the DL-0043 framing of authority boundaries ("who may propose,
  who may rule") as something the engine enforces.
- **Kept**: referential integrity. A supplied citation must resolve against the
  known-decision set (`citation.unknown-decision` stands, fail-closed on forged
  ids). The engine verifies and records; it never invents and never polices
  roles.
- **New**: mutating calls carry machine-supplied **attribution and decision
  state as data**, recorded in provenance. Review/approval workflow is the
  consumer application's pipeline, not an engine mode.
- **Wire discipline**: the behavior change rides contract ≥ 1.2.0 only. Every
  0.x wire keeps the old refusal byte-exact until version retirement (P7) — the
  `ls8_zero_x_parity` oracle enforces this.

## 2. Campaign overview (shared with the consumer doc)

Phases P0–P7. Deloreis milestones DM-1..DM-6; consumer milestones FW-1..FW-10
(detailed in their DP-013). Contract bumps serialize: `1.0.0` (tag only) →
`1.1.0` → `1.2.0` → `2.0.0`. Each phase ends with an adversarial red team
(§6) and its exit criteria green before the next phase's dependent work lands.

| Phase | Deloreis | Consumer (their doc) | Sync point out |
|---|---|---|---|
| P0 Ground zero | sync checkout; drop dead branch; land this doc | pin consolidation; delete their null-oracle parity script; STATUS mirror; land DP-013 | — |
| P1 Handshake | DM-1: integration docs to 1.0 truth; authority-scrub DL entry; annotated tag `contract/1.0.0` | vendor the contract digest-checked (CP-1); pin file; real-binary CI lane (CP-9) | SP-1: tag + 1.0 contract doc |
| P2 Wire + EXT-F4 | verify the posted transcript; CP-10 → `verified` | 1.0 wire jump; EXT-F4 transcript at 0.5.1 terms | SP-6: EXT-F4 done; LS-8 unblocks |
| P3 Store serving | DM-2 loader + `SCHEMA_VERSION` 3; DM-3 corpus ops + rendered loud-refusal; tag `contract/1.1.0` | pack-emitter prep, mapping tables | SP-2/SP-3: tag 1.1.0 |
| P4 Corpus pivot | support/verify | corpus to a committed lorestore; §5 dual-run; serving flip to the default build | consumer corpus store-served |
| P5 Authoring | DM-4 wave-1 + tier-2 expert tools; authority-scrub code; SKILL rewrite; tag `contract/1.2.0` | MCP authoring adoption; staged data expansion | SP-4: tag 1.2.0 |
| P6 Validator (concurrent from P1) | DM-5 governance home + detection corpus | shadow-validate harness; §5 accrual; CI flip to `validate-repo` | SP-4v: corpus + mapping table |
| P7 Closeout | DM-6: 1.x parity capture; version retirement; contract `2.0.0`; STATUS closed | authorship flip; legacy deletion | SP-5 → SP-7: final re-vendor |

**Hard order inside P7**: the consumer stops sending 0.x → a canary window
confirms zero 0.x frames → deloreis retires the dual-mode paths → the consumer
deletes its legacy. Each step reverts independently.

## 3. Deloreis workstream

### DM-1 — Handshake close: truthful docs, the scrub ruling, the first tag

The integration docs are a full major stale and are the consumer's vendored
authority; contract §5 step 1 requires the doc rewrite before the tag.

- `docs/integration/fantasy-contract.md`: 46 → 48 operations (add
  `author.branch`/`author.merge`); `author.materialize` corrected to DL-0046's
  semantics (a mutation and the accept decision, JSON-first, rendering behind
  `rendered: true`); §6 versioning rewritten as a decision — the major arrived
  before the cutover; retired versions will answer through the killed-form
  registry, never silently; N/N-1 applies from 2.0.0.
- `docs/integration/STATUS.md`: fix the "implements contract 0.6.0" paragraph;
  after the tag exists, advance CP-1/CP-9/CP-10 `proposed → provided` with
  evidence cells filled in the same commit (tag name + SHA; fixture paths).
- `docs/integration/migration.md` §10: **annotate, don't rewrite** — the worked
  scenario is historically correct at 0.5.1 and matches the frozen AUTH-2
  bytes; add a note that materialize semantics differ from 0.6.0 up.
- Stale-text sweep: `AGENTS.md` §7 (still "contract 0.6.0"), `SKILL.md` lines
  72/217, `docs/spec/10-serving.md`, `00-overview.md`, `11-agent-authoring.md`,
  the `handlers.rs` capability comment, `PLAN.md`'s Cutover-milestone gloss.
- **Mint the authority-scrub ruling** (§1) as its own DL entry here, so P5's
  code lands under an existing ruling.
- Tag mechanics: land the docs PR, then annotated tag `contract/1.0.0` on the
  merge commit so the tagged tree is self-consistent (doc + `contracts/`
  registry digests). Tag message cites the DL entry. Verify tag pushes clear
  branch protection before relying on them.

Exit: `cargo xtask docs-oracle` + `authority` green; the tag visible on the
remote; STATUS rows `provided` with evidence; `auth2` green at the tag.
Ledger: DL-0053 (handshake delivery + tag policy), DL-0054 (authority scrub).

### DM-2 — Lorestore-backed project loader + SCHEMA_VERSION 3

Closes DL-0050's "no lorestore-backed project loader". Design decisions:

- Manifest: `DocumentEntry` gains `store` (project-root-relative lorestore
  directory) and `head` (a store commit hash), mutually exclusive with
  `sources` — refuse both-or-neither. Documents stay the unit of compilation;
  sibling assets (killed tokens, craft config, episodes) remain valid beside
  either source form; additive, every existing manifest stays valid.
- Two modes: `head` present = replayable pinned mode (advancing canon is an
  explicit manifest edit); `head` absent = branch mode following the tip, for
  authoring servers. The project fingerprint folds the head hash — store
  content-hash regime, never the JCS `workspace_digest` regime.
- Loader: the non-syntax arm reconstructs the pack from the store and feeds the
  existing compile path; the `.dli` arm stays feature-gated with its current
  refusal.
- Serve attach: for store-backed documents, attach the **origin** store rather
  than an import copy, closing the durable authoring loop (materialize advances
  the store the project loads from); a pinned project deliberately does not
  auto-follow.
- **`SCHEMA_VERSION` 2 → 3 lands here, before any consumer store exists**
  (hard ordering — afterwards it would be a live-store migration): persist the
  `Decided`-with-citations status distinction; rule on spans (recommendation:
  spans out of the identity projection, carried as provenance metadata; a short
  spike first confirms whether spans currently sit inside the identity bytes).
  v2 stores migrate by re-fold, recognized as the same lineage.

Exit: gate green including the syntax-free lane; a default build loads a
store-backed project and answers the consumer's five read ops; materialize →
restart → reload sees the new head; a v2→v3 re-fold migration test; the 0.x
parity oracle untouched. No contract bump (server config, not wire).
Ledger: DL-0055 (store-backed projects + SCHEMA_VERSION 3 rulings).

### DM-3 — Corpus exchange on the wire + rendered loud-refusal (contract 1.1.0)

- `dli.corpus.import`: `document`; exactly one of `objects` (fact-object
  array, any build) or `dli_documents` (syntax builds only; loud refusal
  elsewhere naming the one-shot syntax-build path); `mode` init/append (init
  into a non-empty store refuses). Returns `root_hash`, count, graded
  findings. Wraps the existing library import path. This is the door the
  consumer's corpus pivot walks through.
- `dli.corpus.export`: `document`; optional `at` (content hash, default served
  head); `format` objects (default) or rendered (syntax builds only). Returns
  the folded live fact objects at head — folded state, not the raw commit log
  (the log stays a library concern; parked, named).
- `rendered: true` with no renderer linked becomes a **loud refusal** naming
  the ways forward — never a silent empty array. Per-declaration
  `author.discard` re-keys on the structural key so it works on a default
  build.

Exit: gate green; version echo `1.1.0`; golden + negative + framing fixtures
for both ops plus the rendered-refusal negative replay cold; an end-to-end
rehearsal: import a multi-document corpus on a syntax build → serve from the
store on a default build → export returns object-identical facts. Tag
`contract/1.1.0` (bundling DM-2 in the tagged tree).
Ledger: DL-0056 (corpus exchange + loud-refusal rule).

### DM-4 — Expert authoring wave 1 + tier 2, and the scrub code (contract 1.2.0)

Tool scope per the author's tier ruling (2026-08-04):

- **Wave 1 (consumer's actual corpus)**: `author.add_dialect`,
  `author.add_name_part`, `author.add_name`, `author.add_term`,
  `author.add_banned`. Params reuse the LS-9 typed interiors minus spans,
  through the terse `Ident` codec; each tool is a typed constructor plus
  findings on the existing staging path, not new machinery.
- **Tier 2 (this campaign)**: `author.add_state` (time-varying fluents — the
  consumer's episode-continuity states), `author.add_attest` (attestation
  requests for their question-ledger flow). `Use`/`Kind` only if the consumer's
  mapping tables prove the need.
- **Tier 3 parked behind a named trigger** ("first consumer use"):
  `rule`, `practice`, `proof`, `cycle`. A DL entry re-scopes DL-0050's deferred
  item accordingly. **Tier 4 parked indefinitely** (`word`, `narration`,
  `variant`, `principle`, `export`, `attest`-decl beyond the tool above,
  `draft`, `use`, `kind`), with the fork-declaration and draft-declaration
  corpus forms flagged as eventual killed-form candidates once session ops
  fully supersede them.
- **Authority-scrub implementation** (§1): at contract ≥ 1.2.0, mutating calls
  accept citation-bearing provenance and attribution/decision-state fields,
  recorded as data; citations verified resolvable (fail-closed on unknown
  ids); no role gating anywhere in the engine. Every 0.x wire keeps the
  refusal byte-exact. Ratification enters as data the same way — the caller
  cascades a human decision by citing the consumer-ledger id that recorded it.
- `SKILL.md` §0 rewritten (the "you cannot cite" law is replaced by the
  attribution discipline); reference cards updated; docs-oracle covers the new
  examples.

Exit: gate green; version echo `1.2.0`; three fixtures per new tool replay
cold; a scenario test stages dialect + name-part + name + banned via tools,
materializes, reloads, and the language model sees them; 0.x parity green.
Tag `contract/1.2.0`.
Ledger: DL-0057 (wave-1/tier-2 tools + scrub implementation + tier re-scoping).

### DM-5 — Validator governance, detection corpus, parity support (concurrent)

The native repository validator (V1–V12, `crates/dli-world/src/repository.rs`)
is functionally complete but has **no governance home** — no PLAN node, no DL
entry, no design note. There is deliberately no V9: it closed by absorption
(its status-leak half is a theorem in `dli-core` derivation support; its
row-discipline half is MIG-1 replay; its input file was deleted by the
consumer's ruled source flip and their boundary guard forbids its return).

- Governance PR: a ledger entry ruling the scope closed as V1–V8 + V10–V12
  native, recording V9's closure-by-absorption with citations, declaring the
  consumer-side items (their DP-008 X6 database negatives, X8
  propagation/index parity) consumer-owned by name; a PLAN node with
  needs/produces/oracle; a design note with the legacy↔native mapping table
  and a **documented finding-shape compatibility promise**
  (`{path, line, code, message, severity}`, deterministic sort) — the stable
  surface the consumer's shadow harness compares against.
- **Detection corpus, deloreis-owned**: seeded single-defect trees or mutation
  operators with ≥ 1 operator per V-code — all twelve (the consumer's RR-014
  records zero operators today for V1/V2/V6/V10) — each with its expected
  finding recorded; honest misses xfail-strict; walked by a gate-legal test
  (offline, deterministic, committed inputs).
- Stale-text sweep: the four "first slice V1/V2" comments and the CLI
  reference line; regenerate.

Exit: gate green with the detection-corpus test asserting twelve-code
coverage; authority + docs-oracle green with the new note; no "first slice"
text remains. No wire surface, no contract bump. Fully parallel with DM-1..4.
Ledger: DL-0058 (validator governance + scope + corpus ownership).

### DM-6 — Post-EXT-F4 retirement and closeout (contract 2.0.0; strictly last)

Preconditions: the EXT-F4 transcript is `verified` (P2) **and** the consumer
has confirmed on the STATUS evidence trail that nothing production-side pins
0.x (SP-5).

- **Capture before retiring**: record `ls8_one_x_parity` — the 394-frame
  corpus re-expressed in 1.0 typed forms, captured against the pre-retirement
  build and replayed in the gate. Behavior coverage transfers to the living
  wire before the 0.x wire changes meaning.
- **Retire versions through the registry**: a `KILLED_VERSIONS` table beside
  the existing registries — each 0.x version maps to successor + DL id;
  dispatch refuses pre-decode, deterministically, naming both. The accepted
  list becomes 1.x + 2.0.0. Then the dual-mode code dies honestly (the
  version-switch collapses; the below-1.0 required-field shim is removed) —
  the **registry** is what is never deleted, not the dead branches behind it.
- **Re-baseline the 0.x oracle without losing it**: the captured corpus file
  stays byte-untouched; the test's expectation becomes a function of
  retirement state — version-pinned frames expect the computed killed-version
  refusal (itself golden-pinned); a wholesale rewrite of expectations is
  rejected. The 0.x corpus then proves forever that no 0.x frame silently
  succeeds and the refusal surface never drifts; behavior regression lives in
  the 1.x twin.
- **Runtime-continuity trio**: extend the field registry with a field-group
  entry retiring `predicate`/`subject`/`value` together (successors
  `atom_expr`/`to_atom_expr`) at major ≥ 2, with the same two-sided mechanics
  below 2.0. The from-value remains untyped by design (DL-0050) — restated,
  not revisited.
- Fixture registry re-baseline (0.x-pinned fixtures get 2.0 twins or expected
  refusal outcomes; the largest mechanical diff — budget review time); a
  schema-walk test asserting no params field is named `pred`/`args` (the
  followed-by collision audit); STATUS rows to `closed` with evidence; PLAN
  milestone flips; `AGENTS.md` §7 final text. Tag `contract/2.0.0`.

Exit: gate green with both parity oracles (0.x in refusal mode, 1.x
byte-identity); version echo `2.0.0` with the shrunk accepted list; no
dual-mode branch survives (grep-asserted); all STATUS rows closed.
Ledger: DL-0059 (the retirement).

## 4. Sync points (deloreis delivers → consumer proceeds)

Every sync point below is machine-to-machine: one repo's agents wait on an
artifact the other repo's agents produce — a tag, a transcript, a STATUS row
with evidence. **None of them is a human sync point**, and no phase waits on the
author's review (DL-0052). Human steering arrives async, on its own cadence, and
lands as a repair — a superseding ledger entry — not as a gate anything blocks
on.

| # | Deloreis delivers | Unblocks |
|---|---|---|
| SP-1 | `contract/1.0.0` tag + contract doc at 1.0 | vendoring (CP-1), real-binary CI (CP-9), EXT-F4 run |
| SP-2 | store-backed project loader (in the 1.1.0 tag) | corpus pivot serving |
| SP-3 | corpus import/export ops (1.1.0 tag) | corpus pivot import |
| SP-4 | wave-1 + tier-2 tools, scrub code (1.2.0 tag) | authoring adoption |
| SP-4v | validator detection corpus + mapping table + finding-shape promise | their shadow-validate accrual |
| SP-5 | — (consumer delivers: "nothing pins 0.x") | DM-6 may land |
| SP-6 | — (consumer delivers: EXT-F4 transcript) | CP-10 verified; LS-8 unblocks; DM-6 licensed |
| SP-7 | `contract/2.0.0` tag | final re-vendor and closeout |

## 5. Concurrency

DM-1 first (consumer critical path, docs-only). Then DM-2 ∥ DM-4-code ∥ DM-5;
DM-3 after DM-2; contract bumps serialize (1.1.0 → 1.2.0 → 2.0.0) because the
version constants and registry are single files. The consumer's EXT-F4 runs on
the 1.0.0 tag concurrently with DM-2..DM-5. DM-6 strictly last, gated on
SP-5 + SP-6. Every code PR runs `cargo xtask gate`; docs-only PRs run
`docs-oracle` + `authority`.

## 6. Red teams (deloreis-gated phases)

Shared idiom, both repos: N dimension reviewers (one adversarial lens each) →
every finding independently re-verified by a skeptic prompted to refute it →
the orchestrator ground-truth-checks any finding lacking a runnable
reproduction (sub-agents fabricate citations; three real wire breaks were
caught only by this chain in the prior campaign). **A finding without a
reproduction command and observed output cannot block.** Confirmed findings
are fixed inside the phase, before its gate/merge. Severity: **SEV-A**
wire/data truth (blocks), **SEV-B** process/authority (blocks), **SEV-C**
guard erosion (blocks), **SEV-D** drift (fix before phase close).

- **P1 (docs/tag)**: board-vs-tree truth (every STATUS sentence checked
  against the tagged tree — the "0.6.0 dual-mode" paragraph is a pre-seeded
  finding); tag integrity (annotated tag equals the commit the gate receipt
  ran on; reachable from main); digest lie-hunting (recompute hashes over the
  vendored bytes; corrupt a byte, confirm it is caught); worked-scenario
  semantic drift (every fenced transcript in the integration docs validates
  against the current schemas); ladder legality (no rung skipped; evidence in
  the same commit).
- **P3 (loader/ops)**: round-trip byte identity (export → import → re-export
  JCS-identical; a corrupted byte refused by content addressing); loader
  parity (store-loaded project equals the `.dli`-loaded world); grade
  laundering (no output graded above its weakest input); store side-effect
  census (read ops write nothing; author ops' exact write set enumerated);
  determinism (no network, no clock; replay twice byte-identical);
  killed-form conservation (new schemas resurrect no retired field);
  schema-first drift (schemas derived, never hand-edited).
- **P5 (tools/scrub)**: scrub regression (0.x refusal bytes unmoved — the
  parity oracle plus targeted replays); referential integrity fail-closed
  (forged citation ids refused on every wire); attribution recorded not
  invented; tier-scope honesty (no tier-3/4 tool snuck in without its
  trigger).
- **P6 (validator, deloreis half)**: the detection corpus kills — every seeded
  divergence caught or xfail-strict; coverage matrix complete (every legacy
  check maps to a native successor or a **ruled** absorption — absorption by
  oral tradition is SEV-C); the finding-shape promise is tested, not prose.
- **P7 (retirement)**: killed-version completeness (every 0.x frame refused
  naming successor + ruling — the old corpus as refusal suite); no dual-mode
  survivor (grep + parity); fixture re-baseline honesty (no digest re-pinned
  without the 2.0.0 coupling); ledger/PLAN/STATUS truth (`cargo xtask
  authority`; every row closed with evidence); the consumer's
  factory-never-stopped audit passes on their side before the retirement PR
  merges.

**Global invariants, asserted at every phase close** (identical list in the
consumer doc): both gates green; every diff touches exactly one repo and both
worktrees are porcelain-clean after every agent; ledgers append-only, ids
mint-and-use, every entry attributed `machine` or `human` (DL-0052); machine
agents mint their own rulings in both ledgers — deloreis `DL` entries stay
orchestrator-only for single-writer reasons, consumer `D` entries are minted by
the consumer's own agents — and no entry waits on a human sign-off, so a phase
never closes on an author's availability; additive-only on the wire until EXT-F4
is verified; every fenced doc example machine-checked; no silent removal —
every retirement lands in a registry naming its successor; the consumer's
factory keeps shipping episodes throughout; a flipped check never silently
reverts; no lock files, store objects, or build dirs committed; **every
parity/evidence harness carries a standing seeded-divergence case that goes
red**, re-run whenever the harness changes.

## 7. Orchestration rules (cross-repo)

- One git worktree per agent per repo, named `<phase>-<lane>`; no two agents
  share a checkout. Unique `CARGO_TARGET_DIR` per agent (shared dirs caused
  stale-rlib failures; never split the target dir *inside* one gate run — the
  lanes share it by design).
- Every sub-agent prompt names exactly one writable repo root; the sibling
  appears only as an explicit read-only path. After every agent completes, the
  orchestrator runs a porcelain check in **both** repos; any dirt in the
  non-assigned repo rejects the work wholesale — never patched into
  compliance.
- Artifacts cross the boundary only by pull from tags (the consumer's
  vendor-script pattern); a deloreis agent never pushes bytes into the
  consumer tree.
- STATUS rows advance orchestrator-only, evidence in the same commit:
  `provided` = tag + gate receipt; `acknowledged` = consumer's vendored SHA +
  digest-check output; `verified` = the replay/diff artifact; `closed` = both
  ledgers cite it. Before any flip or retirement, read both STATUS files —
  disagreement means stop, not improvise.
- Deloreis git law: branch from `origin/main`; stage by explicit filename;
  plain messages, no AI trailers (hook-enforced — instruct every sub-agent
  explicitly); docs-only PRs run `docs-oracle` + `authority`, code PRs run the
  gate; PR to main, squash-merge, green CI.

## 8. Risk register (campaign-wide; mirrored in the consumer doc)

1. §5 flip shortcuts under schedule pressure → append-only dual-run logs with
   corpus hashes; the flip PR links the log; no log, no flip.
2. A stale 0.5.1 literal survives in a consumer fixture (CI green against a
   fiction) → post-upgrade grep gate with an explicit allowlist; mocks
   regenerated from captured real frames.
3. Hallucinated lore enters canon through extraction (including REJECTED
   ledger entries read as facts) → per-fact citation requirement; skeptic
   re-derivation of samples; one confirmed invention audits the whole stage.
4. Stale projection consumers read dead data post-pivot → seeded canary
   (mutate the new authority only; every consumer must see it); tombstone the
   old file; fail loudly, never fall back.
5. Store lock/object files dirty the consumer tree → gitignore in the same PR
   that introduces the store; porcelain checks after every run.
6. Authoring bypasses review → store ref history reconciled against session
   transcripts and structured-diff review records; orphan mutations block.
7. A sub-agent writes the sibling repo → the both-repos porcelain sweep;
   wholesale rejection.
8. Tag/digest drift after a re-tag → re-vendoring is a named step of every tag
   advancement; `acknowledged` requires the digest-check output.
9. Hallucinated red-team findings prompting harmful "fixes" → no repro, no
   block.
10. A flip retroactively fails shipped episodes → run the episode gate suite
    over the existing corpus before/after each flip; grandfathering is a minted
    ruling carrying its attribution, never a weakened gate.
11. Cross-repo deadlock → a STATUS row stuck more than one cycle is broken by
    the orchestrating agents, who mint the resolving ruling `machine`-attributed
    (DL-0052) and leave it for the author's lagging review; nobody blocks on a
    reply. Agents still never improvise *around* the ladder — the row advances
    by a recorded decision or not at all.
12. Evidence scripts that structurally cannot fail (the consumer's parity
    script was one: comparisons a tripwire guarantees empty, no-op check
    loops, delegation to deleted scripts) → seed-and-expect-red probes before
    any harness output counts as evidence; a harness self-test lands with
    every harness change.

## 9. Honest unknowns (resolve at the named phase, record in the landing DL)

1. Whether `fixture_replay` runs in the syntax-free lane (placement of the
   rendered-refusal fixture) — check at DM-3 start.
2. Whether spans sit inside the fact-object identity bytes (shapes the
   SCHEMA_VERSION 3 span ruling) — spike at DM-2 start.
3. The store object-enumeration-at-head read for `dli.corpus.export` does not
   exist yet — small library addition, shape verified at DM-3.
4. Whether tag pushes clear branch protection — verify at DM-1 before the
   docs PR merges.
5. Whether `Use`/`Kind` tools are needed — decided by the consumer's mapping
   tables at P3/P4; build only on proven need.
