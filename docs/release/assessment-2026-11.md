---
status: live
verdict: "Verified state of De Lore Is at c1ef2a9 (2026-11-02), measured against its original fiction-lore goal: what works, what is broken or incomplete, and what the benchmark era and the TPtH integration left behind. HANDOVER.md tasks cite this file; the ledger outranks it."
---

# Assessment: De Lore Is at `c1ef2a9` (2026-11-02)

**Method.** Seven parallel assessors covered the engine core, the wire
surface, syntax/export/sim, docs, benchmark cruft, an end-to-end run, and
release resources. An adversarial verifier then checked every bug or
incomplete claim, and refuted claims are excluded. The wire-surface and
release assessors built the default binary and drove it over MCP stdio. The
syntax assessor built with `--features syntax` and ran the tests for the
syntax, ast, export, sim, game and tpth crates, all green. Tests for the
engine crates (dli-core, dli-eval, dli-world, dli-serve) were **not** run in
this pass; HANDOVER T2 runs them. Every `file:line` below is at `c1ef2a9`.

## 1. What is good

The reasoning core is coherent, small where it matters, and nearly free of
debt markers. A grep over the core crates finds one TODO
(`crates/dli-eval/src/world.rs:963`), no `todo!`/`unimplemented!`, and no
`#[ignore]` outside deliberate measurement tests.

| Capability | Mechanism | Evidence |
|---|---|---|
| Provenance and standing | Every atom carries an absorptive PosBool support/defeater pair. Standing (certain/possible/dead) is derived under a valuation and never stored. | `docs/spec/02-semantics.md:29-37`; `crates/dli-core/src/ann.rs:82-93` |
| Contested accounts | Open questions with mutually exclusive option families. Disjunctive certainty does not count as canon. | `crates/dli-core/src/ann.rs:33-71`; spec 02:114-118 |
| Contradiction catch | A declared-functional slot raises `continuity.fd-violation` with `{pred, subject, values}` plus citations, and materialize refuses. | `crates/dli-world/src/check/continuity.rs:1438-1480`; `crates/dli-serve/src/mcp.rs:209-211`; `crates/dli-serve/tests/record_decision.rs:151-187`; observed live |
| Non-destructive retcon | Defeat revaluates and keeps lineage. At 1.13, durable conditional attacks restore a claim when its defeater is defeated, and cycles stay undecided. | spec 02:137-144; DL-0096; `crates/dli-lorestore/src/attack.rs`; `crates/dli-serve/tests/conditional_defeat.rs:296-330` |
| Explanations | `author.why` returns status, supporting decisions, active paths, blocking defeats, and why-not with at-risk. | `crates/dli-eval/src/why.rs:173-228,337-343`; `crates/dli-serve/src/author/reads.rs:472,555`; observed live |
| Cheap what-ifs | Saturate once, then valuate many times. A fork re-saturates only its additions. | `crates/dli-eval/src/fork.rs:815-831` |
| Defeasible identity | The same-as closure carries the identity claim's own support, so defeating the identity un-merges. | `crates/dli-eval/src/world.rs:1240-1358` |
| Story time | A partial-order chronology where Unordered is a real answer. Fluent projection is three-valued. Standing-aware order rules. | `crates/dli-time/src/chronology.rs:37-43`; `fluent.rs:29-39`; `order_rules.rs:656-723` |
| Magic-system proofs | A diegetic proof checker with failure classes L1–L7. | `crates/dli-ledger/src/ledger_check.rs:1-31` |
| Determinism | A content-addressed JCS store with a genesis commit on open. Responses carry no timestamps, and golden fixtures replay byte-exact. | `crates/dli-lorestore/src/lorestore.rs:108-123`; spec 10:174-180 |
| Readable `.dli` | With `--features syntax`, `check`, `fmt --check`, `query --why`, `check-proof` and `name check` all pass on `examples/camelot.dli` and the Norse packs. | syntax assessor's run |
| MCP adapter | tools/list, tools/call and isError are honest. MCP always speaks the newest contract, so a user never sees version negotiation. | `crates/dli-serve/src/mcp.rs:198` |

## 2. Broken or incomplete (verified, ranked)

1. **Main CI is red.** The 2026-10-12 scheduled run failed cargo-deny on
   RUSTSEC-2024-0449 (unsafe-libyaml is unmaintained and comes in through
   serde_yaml_ng, root `Cargo.toml:40`). The 10-19 and 10-26 runs died in
   about 4s before any job started, which looks like a billing or account
   problem. The last green run was 2026-10-05. → T1
2. **No way in for outsiders.** The repo is private, every crate is
   `publish = false`, and there is no release workflow. No doc has an
   install, quickstart, or MCP config snippet. The store-backed manifest form
   is documented only in code (`crates/dli-serve/src/project.rs:221-232,
   1039-1045, 1188-1194`), and the store directory must already exist. Every
   on-disk example is `.dli`-sourced and does not load on the default build;
   only `--demo` works. → T9, T15, T16
3. **Stale base after materialize.** In the same server process, a workspace
   opened after a materialize reads the old canon, and `add_fact` raises
   `entity.unknown` for an entity that was just committed. The base snapshot
   is registered once at load (`crates/dli-serve/src/serve.rs:689-727`;
   `author/store.rs:495-561, 1018-1050`; `project.rs:74-86`). This matters for
   long-lived MCP hosts such as Claude Code and Desktop. Observed live. → T12
4. **`during <era>` events never reach the chronology.** `During` is used
   only as a reference check (`crates/dli-world/src/loader.rs:455-463,
   497-506`). As a result Camelot's timeline reports sword-drawn ? grail-quest
   as unordered, contradicting spec 03 §1. → T10
5. **CLI `query --as-of` and the exports flatten fluents.** Both values come
   back as certain (`crates/dli-eval/src/query.rs:242-248`). The MCP
   `author.as_of` is correct (`author/reads.rs:124-133`). → T10
6. **Entity-sort predicates can't be queried.** `person($p)` and `era($e)`
   fail as unknown, and an `era a < b < c` chain emits only `era(a)`
   (`crates/dli-eval/src/world.rs:1208-1211`; `loader.rs:474-487`). → T10
7. **`name.unvetted` noise.** It fires on every `add_entity`, and its typed
   repair omits the required `dialect`, so the repair fails schema
   validation (`crates/dli-serve/src/author/tools.rs:1623-1628, 2414-2429`).
   → T12b
8. **Two conflicting draft claims are silent.** The FD and fluent checks read
   certain slots only (`continuity.rs:77-101, 491-515`). Spec 02 §4 and
   charter trial 2 promise a finding; the ledger says silence is correct.
   → T13
9. **Misspelled entities in `.dli` facts pass `check` silently.** → T11
10. **`root_hash` does not identify the store commit.** Two different heads
    can report the same hash (`author/reads.rs:101-111`;
    `author/workspace.rs:467-486`). → T12c
11. **Retcons need a claim_id that only materialize output exposes**
    (`tests/conditional_defeat.rs:133-164`). → T12d
12. **tools/list advertises all 69 tools (~166 KB).** → T14
13. **Magic-system practices and proofs have no `author.*` tools**; they are
    reachable through `.dli` only (DL-0079). This is acceptable once `.dli`
    ships by default (T9).
14. **Stale agent and teaching docs.**
    - SKILL.md says the contract is 1.6.0, says there are no rule tools
      (`add_rule` has existed since 1.7.0), and says nothing touches disk
      (SKILL.md:79, 93-94, 104, 106, 118, 343-344).
    - The authoring-loop card was captured at 0.6.4.
    - The examples-walkthrough support variables are off by one
      (`docs/cards/examples-walkthrough.md:226-252`); transcript blocks are
      not gate-checked.
    - cli.md does not mark which commands need the syntax feature.
    → T16

## 3. Cruft

**Benchmark era.** It did little damage to code. CI has no benchmark lane:
all five jobs are deterministic (`.github/workflows/ci.yml:28-117`). What it
left:

- **Positioning.** README.md:3-11, `book/src/introduction.md`, spec 00
  (:14-83), PLAN "External evidence priority", spec 12, design note
  `benchmark-program.md`, and DL-0062. All come from PR #42 (`3318022`); the
  fiction README before it is `git show 3318022^:README.md`. DL-0101 now
  supersedes this.
- **Dead code.** `dli_world::compile_incremental` /
  `compile_incremental_verified` (`crates/dli-world/src/incremental.rs:249-403`,
  re-exported at `lib.rs:34-35`) and the 1,420-line
  `tests/incremental_compile_parity.rs`. No production caller (GH #92).
  `World::saturate_incremental` is live through the fork path; keep it.
- **Benchmark vocabulary** in doc comments and op summaries:
  `check/mod.rs:110`, `attest.rs:342`, `model.rs:296`, `derivation.rs:202`,
  `xtask/src/finding_families.rs:5`, and `ops.rs` summaries such as
  "fixture-authority".
- **Benchmark-born ops** (`author.predicates`, `author.entity_candidates`,
  `author.assert_predicate_same_as`). These are useful for an authoring agent
  reusing names. Keep them, but leave them out of the core profile.
- **Root strategy docs:** EVALUATION-*, EXTENSIONS-*, SPECULATIVE-*,
  STEERING-REVIEW-*.
- **Issues** #73, #86, #89, #92 and #116 (closed by DL-0101).

**TPtH** (to be scrubbed under DL-0101):

- `crates/tpth`.
- `worlds/tpth`: 156 files, including the vendored fantasy corpus.
- `docs/worlds/tpth` and the `book/src/worldbook-*` pages.
- `docs/integration/*`.
- The dli-world craft port: `crates/dli-world/src/craft/` (about 6.6K lines),
  `crates/dli-world/craft-doctrines/`, `schemas/craft/`, and the craft half of
  spec 06.
- The oracular-deck simulator: `crates/dli-sim`, `crates/dli-game`, the
  Ludii/GDL export, and `examples/games`.
- The pipeline ops that serve the fantasy factory: `dli.payload-submit`,
  `check-draft`, `attestation-sweep`, `simulate`, and the TPtH export
  profiles.
- Golden fixtures named after TPtH characters.
- The world-token lints, which become a repo-wide guard.
- Migration-era scripts: `scripts/differential-oracle.sh`, `shadow-read.sh`
  and `pinned-oracles.sh`. They point at a vendored corpus directory that no
  longer exists.
- References in AGENTS.md §6–7, the charter, PLAN, and about 25 design notes.

**Multi-version contract layer.** The server accepts 0.5.1 through 1.13.0,
with golden fixtures per version. Its only consumer is the fantasy repo, so it
retires with the scrub (contract 2.0.0, T5).

## 4. Release resources inventory

- **Usable now:** the Camelot and Norse examples and the gentle,
  authoring-loop and examples-walkthrough cards. Their worlds are
  public-domain; the content is stale but the structure is sound.
- **Missing:** a release workflow, binaries, crate metadata, a docs site (the
  mdBook wraps internal docs and renders raw frontmatter, `book/README.md:24-28`),
  any DLI page on the website repos, and an example world written for release.
- **Blog home:** the mesite repo (jeffreykent.dev). The post is written last,
  from `north-star.md`.
