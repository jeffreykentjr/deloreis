---
status: live
verdict: "The executor's guide for the fiction-release reset (DL-0101): ordered tasks T1–T16 with goal, files, steps, acceptance, and verification. Each task mirrors one GitHub issue labelled release-1. The ledger outranks this file."
---

# Handover: the fiction-release reset

**Read first:** [`north-star.md`](north-star.md) (what we're building toward),
then [`assessment-2026-11.md`](assessment-2026-11.md) (the verified state, with
evidence). DL-0101 in `../decisions.md` is the ruling behind all of this. The
tracking issue is labelled `release-1`, and each task below has its own issue.

## Owner rulings you must not re-litigate (DL-0101)

1. **Fiction only.** DLI is a continuity engine for fictional worlds. You may
   call it agent-ready for authors and agents generating fiction. You may not
   call it general agent memory or "epistemic infrastructure".
2. **Cut what we don't use.** Delete it; don't archive it piece by piece. Git
   keeps the history at the tag `pre-fiction-reset`. CI must pay nothing for
   abandoned features. Keep code that still fits, and tidy it.
3. **former private consumer leaves this repo completely.** Code, worlds, fixtures, docs and
   lints all go. If a mechanism is generic but its tests use former private consumer data, keep
   the mechanism and re-author the data on Camelot, Norse, or the new world.
   If the former private consumer or its pipeline is the only user, delete it.
   The former consumer repository pins `pre-fiction-reset`; never edit it here.
4. **`.dli` is revived** as a first-class, hardened authoring and reading
   surface. The lorestore stays canonical. #84 is cancelled.
5. **Contract 2.0.0** is the only served contract after the scrub. Earlier
   versions and deleted ops go into the killed-form registry, naming 2.0.0.

## Working rules (this repo's, condensed)

- Build and test with a **short target dir**:
  `CARGO_TARGET_DIR=/home/jeff/tg-rel CI=1 cargo xtask gate`. A long path
  breaks a Unix-socket test. Don't symlink the target dir. After moving it,
  run `cargo clean`.
- Run the **gate once per PR**, not in a loop. Docs-only PRs run
  `cargo xtask docs-oracle` and `cargo xtask authority`. Regenerating
  `cli.md` needs `cargo xtask docgen` with `--features syntax` *before* `--`.
- **Stage by explicit filename.** Never use `git add -A`, `git add .` or
  `commit -a`. No AI co-author trailers; the `.githooks/commit-msg` hook
  rejects them. The CI `no-ai-authorship` lane
  (`scripts/no-ai-authorship.sh:34`) also fails any commit message that
  contains a standalone `ai` or `llm`, or a tool name such as `claude`,
  `codex` or `gpt-4`. Write "benchmark-era", not "LLM benchmark", and keep
  squash-merge bodies clean too.
- **main requires a PR**: squash-merge on green CI. Keep PRs few and large:
  WS1 is 2–3 PRs, not one per file. Each non-trivial PR gets one
  clean-context red-team review of the *committed* diff.
- **Ledger:** mint the next `DL-nnnn` in `docs/decisions.md` only, in the
  same commit that uses it. Each entry needs `*Date: YYYY-MM-DD*`,
  `**Attribution:** machine|human`, and a `**Ruling.**` line. Run
  `cargo xtask authority`. To correct an unmerged entry, amend the commit; a
  follow-up commit fails `authority`.
- **Every doc** carries `status` and `verdict` frontmatter. Fenced `.dli`
  blocks in docs must parse, because docs-oracle runs them.
- Pushing: `GIT_SSH_COMMAND="ssh -F /dev/null" git push`. If you're
  sandboxed, network is off and `git fetch` silently no-ops.

## Order

    WS0  T1 → T2 → tag
    WS1  T3 → T4 → T5 ; T6, T7 alongside T4 ; T8 last in WS1
    WS2  T9 → T10 → T11            (can start after T4)
    WS3  T12, T13, T14             (after T5)
    WS4  T15 → T16                 (after WS1–3)
    WS5  owner-only (repo public, binaries, docs site, blog)

---

## WS0: Unblock

### T1: Get main CI green
- **Goal:** cargo-deny passes; the gate is green on main.
- **Files:** root `Cargo.toml:40` (serde_yaml_ng), `deny.toml`, and whatever
  crates use serde_yaml_ng (`git grep serde_yaml`).
- **Steps:** Prefer replacing serde_yaml_ng with a maintained YAML crate
  that doesn't pull unsafe-libyaml, or drop YAML entirely if its only users
  are former private consumer or craft (check first; T4 may delete them). Failing both, add a
  justified `[advisories] ignore = ["RUSTSEC-2024-0449"]` with a comment and
  a removal trigger.
- **Acceptance:** `cargo deny check` is clean locally and the CI `deny` job
  is green on the PR. If CI jobs die in about 4s before starting, that is the
  owner's GitHub billing; report it and don't debug it.
- **Verify:** `cargo deny check`; `gh pr checks <n>`.

### T2: Baseline gate run and tag
- **Goal:** record engine test health before cutting anything, and tag the
  last former-consumer-bearing commit.
- **Steps:** On green main, run the gate once with the short target dir and
  note the pass/fail counts in the T2 issue. Then
  `git tag -a pre-fiction-reset -m "last commit with former private consumer, multi-version contract"`
  and push the tag.
- **Acceptance:** the gate result is posted to the issue and the tag exists
  on the remote.

## WS1: Cut

### T3: former private consumer inventory (one comment, no code)
- **Goal:** a complete removal list before deleting anything.
- **Steps:**
  - Run `git grep -liE` over the world tokens in
    `xtask/src/boundary.rs` `WORLD_TOKENS`, plus the retired identifier, former consumer references, `D-0`,
    `craft`, `factory`, `payload-submit`, `check-draft`, `attestation`,
    `simulate`.
  - Group the hits by path and give each a disposition: **delete**,
    **keep and re-author data**, or **keep and reword**. Apply the
    DL-0101 rule. Default dispositions are in the assessment §3.
  - For every op to delete, list its schemas
    (`contracts/dli-worker/schemas/`), fixtures, `ops.rs` rows and tests.
  - Also decide on the episode fold (`crates/dli-world/src/episode.rs`) and
    the naming/dialect layer. Both are expected to be generic; keep them and
    re-author their data.
- **Acceptance:** the inventory is posted as a comment on the T3 issue.
  Anything ambiguous is flagged for the owner rather than guessed.

### T4: Execute the former private consumer scrub
- **Goal:** no former private consumer in the tree.
- **Steps:**
  - Cut in 2–3 PRs: (a) code and crates: the former consumer crate, the craft port,
    `dli-sim`, `dli-game`, the Ludii export, the pipeline ops, and the
    workspace members in `Cargo.toml`; (b) worlds, fixtures and examples:
    `worlds/`, `examples/games`, re-authoring test data that generic
    mechanisms need on Camelot or Norse, and renaming character-named
    fixtures; (c) docs: the former private world documentation, `docs/integration/`,
    `book/src/worldbook-*` and `integration-*`, the craft half of spec 06,
    the former-consumer-only design notes, the spec footers that cross-link worlds,
    and the AGENTS §6 integration section.
  - Turn `xtask/src/docs_tokens.rs` and `boundary.rs` into one **repo-wide
    guard**: no world token or former private consumer or former consumer reference in any tracked file
    except the guard's own token list. Drop the allowlists.
  - Remove the CI steps and xtask subcommands that existed only for the
    deleted crates and ops.
- **Acceptance:** a tracked-file search for the retired identifier and every `WORLD_TOKENS` entry returns nothing outside
  the guard, and a review of a search for former consumer references finds no hits that refer
  to the former consumer repository or consumer (the genre word is fine). This includes the
  `docs/release/` files themselves: reword their former private consumer mentions to "the
  former private consumer", or remove them. The guard runs inside `cargo xtask gate`. The gate is
  green, and CI wall-time drops (note before and after in the PR).
- **Verify:** the gate; the git grep above.

### T5: Contract 2.0.0
- **Goal:** one served contract.
- **Steps:**
  - Mint a DL entry. Set `CONTRACT_VERSION` to `2.0.0`
    (`crates/dli-serve/src/lib.rs:120`) and remove the multi-version
    dispatch.
  - Register each retired version range (0.5.1, 0.6.x, 1.x) and each
    deleted op in the killed-form registry with successor "contract 2.0.0".
    The server must refuse each one by name.
  - Delete the per-version golden fixtures, and re-pin the 2.0.0 fixtures
    under neutral names.
  - Regenerate the schemas and the reference docs.
- **Acceptance:** an old-version frame gets a killed-form refusal naming
  2.0.0. MCP still works. The gate is green.

### T6: Benchmark-era code cruft
- **Steps:**
  - Delete `compile_incremental` and `compile_incremental_verified`
    (`crates/dli-world/src/incremental.rs:249-403`, `lib.rs:34-35`) and
    `tests/incremental_compile_parity.rs`. **Keep**
    `World::saturate_incremental`.
  - Delete `scripts/differential-oracle.sh`, `shadow-read.sh` and
    `pinned-oracles.sh`.
  - Reword the "benchmark"/"SubtleMemory" doc comments
    (`check/mod.rs:110`, `attest.rs:342`, `model.rs:296`,
    `derivation.rs:202`, `xtask/src/finding_families.rs:5`) and the op
    summaries in `ops.rs`.
  - Re-describe `author.predicates`, `author.entity_candidates` and
    `author.assert_predicate_same_as` in fiction terms: reuse a character's
    existing id, and merge two names for one character.
- **Acceptance:** a case-insensitive grep for `benchmark` or `subtlememory`
  in `crates/` and `xtask/` returns nothing. The gate is green.

### T7: Docs cut
- **Steps:**
  - Delete the root EVALUATION-*, EXTENSIONS-*, SPECULATIVE-* and
    STEERING-REVIEW-* docs.
  - Delete these design notes: `benchmark-program`, `bench-deferral`,
    `baselines-why-not`, `differential-harness`, the `audit-*` notes,
    `standalone-audit-2026-07`, `followups-2026-07`, the cutover notes, and
    anything T3 marked former-consumer-only.
  - Keep the notes that describe live mechanisms (inertia, lorestore
    vision/roadmap/indexing, decision recording, multi-authority, conceptual
    comparison). Reword their benchmark motivation to a single line.
  - Rewrite spec 00 for fiction, starting from
    `git show 3318022^:docs/spec/00-overview.md`, and drop its "Evaluation,
    scale, and consensus" section.
  - Demote spec 12 to a parked design note.
  - In PLAN, drop BENCH-*/SURF-4/DIST-* or mark them retired or parked, and
    resolve #110.
  - Keep `cargo xtask authority` green; PLAN's node registry and status
    matrix must still match.
- **Acceptance:** docs-oracle and authority are green, and no doc pitches
  benchmarks or general agent memory.

### T8: Ledger reset (high judgment; one red-team)
- **Goal:** a ledger a newcomer can read, with no former private consumer in it.
- **Steps:**
  - Write a new `docs/decisions.md`. It opens with the discipline header and
    one entry, numbered from the current next ID, titled "Standing rulings
    carried forward from `pre-fiction-reset`".
  - That entry is a terse list. Each still-binding *engine* ruling gets its
    old ID, a one-line statement, and the pointer
    `git show pre-fiction-reset:docs/decisions.md`.
  - Drop every entry that only governs the former consumer, the benchmarks, the
    cutover campaign, or the retired contract versions.
  - Update `xtask/src/authority.rs` only if the sequence check needs a start
    offset, and keep its tests.
- **Acceptance:** authority is green. A red-team reviewer confirms that no
  live engine ruling was dropped, by checking the old ledger's surviving
  semantics against spec 02/03 and the code.

## WS2: `.dli` revival and hardening

### T9: Ship `.dli` by default
- **Steps:**
  - Make `syntax` a default feature in `crates/deloreis/Cargo.toml:11-17`
    and `crates/dli-serve/Cargo.toml`.
  - Mint a DL entry; DL-0101 already authorizes it.
  - Add `deloreis init <dir>`, which writes the store directory and a minimal
    `deloreis.toml`, and `deloreis import <file.dli>`, which loads text into
    a store-backed project.
  - Make the wire `rendered: true` format work on the release build.
  - Regenerate `docs/reference-generated/cli.md`.
  - Retire or adjust the `syntax_free` xtask lane to match.
- **Acceptance:** a fresh release build runs `deloreis check
  examples/camelot.dli`, and `deloreis init` followed by `import` and then
  `mcp` serves the imported world.

### T10: `.dli` correctness bugs
- **Steps:**
  - Lift `during <era>` events into the chronology
    (`crates/dli-world/src/loader.rs:455-506`) per spec 03 §1.
  - Make CLI `query --as-of` and the exports project fluents through the same
    code path as `author.as_of` (`crates/dli-eval/src/query.rs:242-248` vs
    `crates/dli-serve/src/author/reads.rs:124-133`).
  - Make entity-sort predicates queryable, and emit an atom for every
    era-chain member (`world.rs:1208-1211`, `loader.rs:474-487`).
- **Acceptance:** a regression test for each bug. `deloreis timeline order
  --corpus examples/camelot.dli` orders sword-drawn before grail-quest.

### T11: `.dli` robustness for outside users
- **Steps:**
  - Add a warning for an undeclared entity in a fact, with a did-you-mean.
  - Do a diagnostics pass: every error has a span and a suggestion, checked
    against `docs/reference-generated/error-codes.md`.
  - Add an fmt round-trip test over every example, and a property or fuzz
    test showing that the parser never panics on arbitrary input.
  - Make the gentle card the language tutorial.
- **Acceptance:** the new tests pass inside the gate, and a misspelled
  entity in a fact yields a warning.

## WS3: Engine and wire fixes

### T12: Wire bugs
- **(a) Stale base after materialize.** Refresh the base and snapshot after
  materialize (`crates/dli-serve/src/serve.rs:689-727`,
  `author/store.rs:495-561`), and add a test: materialize, open a second
  workspace, query, and see the new canon.
- **(b) `name.unvetted`.** Make it quiet unless a dialect is configured, and
  fix its repair to include `dialect` (`author/tools.rs:1623-1628,
  2414-2429`).
- **(c) `root_hash`.** Include the base commit (`author/workspace.rs:467-486`).
- **(d) claim_id read.** Add a read, or a field on query rows, that exposes a
  fact's claim_id so a retcon doesn't need the materialize output.
- **Acceptance:** a test for each fix; the gate is green.

### T13: Warn on conflicting drafts
- **Goal:** a first-time author staging two proposed values for a
  declared-functional slot sees a warning.
- **Steps:** Add a warn-level `continuity.pending-conflict` finding, and amend
  spec 02 §4 and charter trial 2 to match. Mint a DL entry.
- **Acceptance:** the warning fires on two proposed values and does not block
  materialize.

### T14: Core MCP tool profile
- **Steps:**
  - `deloreis mcp` advertises about 20 fiction-core ops by default. A
    starting list: open, add_entity, add_fact, add_era, add_event,
    declare_functional, declare_inertial, record_decision, open_question,
    add_option, check, query, why, as_of, defeat, set_standing, fork or
    suppose, materialize, discard, entity_candidates.
  - `--tools all` advertises everything.
  - Write the classification into SKILL.md.
- **Acceptance:** tools/list is under about 60 KB by default, and an MCP test
  covers both profiles.

## WS4: Example world and docs

### T15: The example world, "The Hollow"
- **Goal:** the blog's running world, kept honest by CI.
- **Steps:**
  - Build it per the brief in `north-star.md`: `examples/hollow/hollow.dli`,
    `examples/hollow/deloreis.toml`, and a store-backed project.
  - Write a driver test (Rust, in `crates/dli-serve/tests/` or
    `crates/deloreis/tests/`) that runs beats 1–6 over MCP stdio and writes
    the transcript to `examples/hollow/transcript.md`.
  - The test fails if the transcript would change, so update it with an
    explicit bless env var.
  - Everything is original and uses no former private consumer names.
- **Acceptance:** the driver runs in the gate, and the transcript shows the
  fd-violation, materialize-blocked, contested `possible` rows, the retcon
  and its restoration, the what-if diff, and a replay after restart.

### T16: Release docs
- **Steps:** Write the target docs set in `north-star.md`:
  - `README.md`, rewritten from `git show 3318022^:README.md` and the north
    star;
  - `docs/quickstart.md`, with a `claude mcp add` snippet verified in Claude
    Code;
  - `docs/concepts.md`;
  - the authoring-loop card, re-captured from the T15 transcript;
  - the examples-walkthrough card, with its numbers fixed;
  - `SKILL.md`, rewritten for 2.0.0 with core tools first, fiction examples
    and no DL IDs;
  - `CHANGELOG.md`, absorbing AGENTS §7's version history;
  - AGENTS.md and the charter with former private consumer removed (AGENTS §7 cut to about 10
    lines);
  - the mdBook restructured users-first, with frontmatter stripped from
    rendered pages.
- **Acceptance:** a newcomer following only the README and quickstart on a
  clean machine reaches the contradiction catch within 10 minutes. Test this
  with a fresh-context agent given only those two files. docs-oracle and
  authority are green.

## WS5: Owner-only (do not start)

- Make the repo public.
- Decide crate names and publishing.
- Add a release workflow with prebuilt binaries.
- Put the mdBook on Pages.
- Write the launch post in the mesite repo from `north-star.md`.
- Archive the dli-benchmarks repo.
- Pin the former consumer repository to `pre-fiction-reset`.
