---
status: live
verdict: "The generic worker contract at 1.9.0. Caller-project isolation, typed response semantics, durable evidence and simulation jobs, expert authoring, corpus and canon exchange, authority-scrubbed provenance, durable standing transitions, functional-dependency declarations, defeasible coreference assertions, datalog rule authoring, temporal close-out findings, an inertial-persistence declaration (a value asserted `since` holds forward until a later distinct certain value defeats it, bounded by the continuity check), the one inertia-aware read (`author.as_of`, projecting that same boundary at a query point), and complete paired framing replay are implemented across the accepted legacy window."
---

# The dli-worker contract

> **A worked integration: the fantasy factory.** This is one consumer of the
> generic `dli-worker` contract — the fantasy repository's audio-drama factory,
> the first tree to wire itself to a `deloreis` server. The contract *shape*
> (parties, transport, the `dli.*`/`author.*` inventories, the closed fixture
> registry, versioning) is generic and engine-owned; the fantasy factory is the
> worked instance that exercises it. A second consumer would wire to the same
> contract, not a new one.

This document defines the boundary between **De Lore Is** and the **fantasy**
repository's worker layer. It is owned by `deloreis` (`decisions.md` DL-0010) and
vendored, pinned and digest-checked, by fantasy. Where any reimplementation on
either side disagrees with this document, this document wins.

> **Implementation status:** the contract is at `1.9.0` (`decisions.md` DL-0051,
> DL-0053, DL-0055, DL-0056, DL-0058, DL-0069, DL-0070, DL-0074, DL-0077,
> DL-0080, DL-0081, DL-0082). It carries typed responses for all 64 operations,
> strict caller projects, complete golden/negative/paired-framing coverage, and
> a replayable tool surface. Durable fat-payload, attestation, and
> restart-resumable simulation evidence is implemented. `CON-1`, `AUTH-1`, and
> `SERVE-1` are closed. The server runs **multi-version dual-mode**: it accepts
> `0.5.1`, `0.6.0`–`0.6.4`, and `1.0.0`–`1.9.0`, and answers in the
> version the caller declared (§6), so a vendored `0.5.1` consumer keeps
> byte-identical responses. `EXT-F4` — the consumer's worked-scenario transcript
> against a tagged build — is **complete**: the posted transcript is byte-parity
> verified against the deloreis-owned AUTH-2 fixture, so `CP-10` stands
> `verified` on the STATUS board. What remains is the consumer's own corpus
> pivot, not a deloreis deliverable.

## 1. Parties

- **deloreis** — the opaque `deloreis` binary and its server. It compiles,
  checks, queries, simulates, exports, and hosts the tool surface. It is deterministic and
  offline; it never calls a model.
- **fantasy** — the Elixir factory (Phoenix/LiveView, Ash, Oban). It owns every
  LLM call, all orchestration, and all rendering. It reaches lore only through
  this contract.

**The boundary sentence:** *the factory may say anything; the kernel stores only
typed things with provenance; the author alone makes things true; and every
model utterance is one of four declared classes, or it does not enter.*

## 2. Transport and envelope

- **Transport.** JSON over stdio, carried on the MCP transport. Batch `dli.*`
  operations and the `author.*` tools are both registered against the server.
- **No timestamps.** The envelope carries no wall-clock time. Determinism and
  replay do not admit a clock; version is explicit (§6), not inferred from time.
- **Unknown fields are errors.** An envelope or payload carrying a field the
  contract does not define is a `schema` error (§9), never a silently ignored
  extra. This is "nothing is silently dropped" at the wire.
- **Two declared optional request fields.** A post-open mutating `author.*` call
  may carry `expected_digest` — the caller's last-seen workspace digest, for
  optimistic concurrency: a mismatch is a `stale-fence` error (§9), so a stale
  caller never clobbers a moved fence — and `idempotency_key` — a retry-safe key,
  so a re-sent mutation is applied at most once. Read tools, `author.open`,
  `author.materialize`, `author.branch`, `author.merge`, and batch `dli.*`
  operations do not accept either field — `materialize` is a mutation (§3.2)
  whose fence is the workspace's own pinned commit, and a re-materialize with
  nothing newly staged is an idempotent no-op without a key. Both are **declared**
  fields; "unknown fields are errors" still stands. Native stdio carries them in
  the request envelope. MCP exposes them as reserved arguments on those ten
  author mutation tools and lifts them into that same envelope before shared
  dispatch.
- **Every tool-surface mutation returns immediately.** No mutating `author.*`
  operation defers its verdict; graded findings, the structured result, and the
  workspace digest come back on the same call. The `.dli` rendering rides along
  as an opt-in view of that result, never as the result itself (§3.2). Stateful
  `dli.fork.*` operations return their typed consequence delta or
  ruling-request draft.

## 3. Operation inventory

### 3.1 Batch operations — `dli.*`

The table states the semantics every advertised operation satisfies.

| Op | Purpose | Returns |
|---|---|---|
| `dli.version` | Report the worker contract version and this build's op capabilities. | `{contract, contract_version, server, capabilities}` |
| `dli.check` | Compile and check a scope or corpus, replaying its committed evidence. | `ok`, weakest procedure `grade`, and domain-tagged findings with durable evidence identity |
| `dli.query` | Query the compiled world (`--layer/--as-of/--fork/--why`). | every matching row: atom, bindings, status, support, grade, citations, optional why, rendered |
| `dli.fork.new` | Create a fork. | fork handle |
| `dli.fork.list` | List forks. | forks with base/assumes/counts |
| `dli.fork.diff` | Derived-consequence diff of a fork. | typed before/after atom changes and conflicts, each rendered |
| `dli.fork.suppose` | Assume an option or event in a fork. | diff delta |
| `dli.fork.add` | Add proposed ground facts or slots to a fork. | `ok` + diff delta |
| `dli.fork.suppress` | Retract an assumption from a fork. | diff delta |
| `dli.fork.restrict` | Restrict a fork's valuation scope. | diff delta |
| `dli.fork.preflight` | Ratification pre-flight: candidate valuation + conflict scan. | named candidate valuation + typed conflicts + affected atoms |
| `dli.fork.promote` | Emit a ruling-request draft from a fork. | `rq_draft` (slug, ask, affects, diff, evidence) |
| `dli.check-proof` | Check a selected diegetic proof `in <practice>`; `proof` may be omitted only when the document declares one. | verdict + errors (structural classes) + grade |
| `dli.check-draft` | Run the full battery over a draft payload. | one merged report |
| `dli.simulate.start` | Start a durable battery over a logical practice document or game ruleset/delta. | `job_id` + status |
| `dli.simulate.status` | Poll a battery. | progress, or final manifest + metrics + invariant verdicts |
| `dli.simulate.cancel` | Cancel a battery. | status |
| `dli.timeline.show` | The timeline at a position (`--at`). | topological layers, cycle blame, cursor relations, placements |
| `dli.timeline.order` | Partial-order report. | cycles + every unordered pair |
| `dli.timeline.knows` | Who-knows-what matrix at a position. | characters × facts (listener included) |
| `dli.timeline.episodes` | Episode-order check + slack. | episode summaries, loop ledger, integrity findings, earliest/latest slots |
| `dli.timeline.phases` | Phase-walk verdict for an episode. | resolved anchors + consistency findings |
| `dli.export.run` | Run an export target and profile. | target-shaped emission + manifest |
| `dli.name.check` | Vet a name (the kernel half only). | kernel verdict (world-true) |
| `dli.attest.missing` | List the open attestation questions checks demand. | question records |
| `dli.attestation-sweep` | Validate and append an attestation sweep's results to the store. | accepted count + immutable batch digest |
| `dli.payload-submit` | Submit a `payload/v1` document: validate, quote-anchor-check, store. | findings + payload digest |
| `dli.corpus.import` | **≥ `1.2.0`.** Seed or extend a store-backed document's corpus. `document`; `mode` `init`/`append`; exactly one of `objects` (fact objects, any build) or `dli_documents` (syntax builds only). An `objects` payload also carries `interner`, and `decisions` unless the store has none. | `{root_hash, imported_count, findings}` |
| `dli.corpus.export` | **≥ `1.2.0`; `format: canon` ≥ `1.4.0`.** The folded live facts of a store-backed document — state, not the commit log. `document`; optional `at` (a store commit hash, default the served head); `format` `objects` (default), `rendered` (syntax builds only), or `canon` (the LS-7 normalized projection). | `{root_hash, objects, interner, decisions, rendered?, canon?}` |

`dli.fork.suppose` **supersedes** `dli.suppose`, which is a **killed form** — it
is registered as such and the server refuses it with a `refused` error naming
the successor. `dli.check-proof` likewise **supersedes** `dli.check-ledger`,
renamed by `decisions.md` DL-0016 (the surface-v1 pass that renamed the
in-practice proof block keyword `ledger`→`proof`); the old op name is a killed
form the server refuses, naming its successor.

**A corpus is `objects` *and* `interner` *and* `decisions`** (DL-0056). A stored
fact records its support polynomial as `±<index>` into the commit's interner,
and its standing is read against the commit's known-decision set — so the three
travel together or the exchange is lossy in a way that does not announce itself:
re-importing the objects alone reconstructs a different world, `decided d-1`
coming back as `proposed`. `dli.corpus.export` emits all three; an `objects`
import without its `interner` is refused rather than reconstructed, and an
append may only extend what the store already holds. Feed back what the export
returned and the round trip reproduces the source commit hash. An append also
preserves the store's committed scope: `document` is a wire identifier, so
serving a store under a different id is supported and never renames it.

**Three `1.3.0` refinements to that door** (DL-0059), each stated with the
version it applies from, because two of them are floors rather than blanket
rules and a consumer reading only this paragraph must not over-apply them.

- **An imported `claim_id` is checked — from `1.3.0` only.** A claim id is the
  SHA-256 of the claim's own identity, minted from a short documented label:
  `atom:<pred>(<args>)` for a fact, `decl:<Kind>:<name>` for a carried
  declaration. An external emitter can therefore compute it without replicating
  any canonicalization. At `1.3.0` and above `dli.corpus.import` re-derives it
  and **refuses a mismatch** with a `schema-violation` naming the id the content
  mints; it is neither taken on trust nor silently rewritten, because rewriting
  would discard what the caller stated without saying so.
  **This is a floor, not a retroactive rule.** `1.2.0` and every version below
  still **accept** a mislabelled object, and that stays true permanently —
  released behavior is frozen. The pair is pinned in the registry so neither
  half can drift: `golden/corpus-import-claim-id-frozen-1-2-0-v1` replays the
  acceptance at `1.2.0`, `negative/corpus-import-mislabelled-claim-id` replays
  the refusal at `1.3.0`. A consumer on `1.2.0` gains nothing here until it
  negotiates `1.3.0`.
- **`mode: init` means "holds no facts", and now actually reaches an empty
  store.** Emptiness is measured by the facts a store holds, not by whether it
  has a commit: opening a lorestore writes a genesis commit, so a
  head-based test called every servable store "seeded" and left `append` as the
  only way in. `init` now seeds a store that exists but is empty — the
  seed-from-nothing path a consumer's first import needs. The refusal it keeps
  is the one always intended: `init` into a store that already holds facts is
  still refused, naming `append`.
- **The store directory must already exist.** The project loader resolves
  `documents.<id>.store` against the filesystem and fails to load a manifest
  naming a directory that is not there — it does not create one. So
  "seed from nothing" means *from an empty store*, not *from no store*:
  something outside this wire has to create the directory first. That is a real
  gap in wire completeness for a consumer with no other way to touch the
  filesystem, and it is recorded here rather than left to be discovered.

All 64 operation names have request/response schemas, dispatch arms, and golden
+ negative + paired-framing fixtures. The complete fixture oracle passes
against the real front end and a digest-pinned replay project. This closes
contract shape and replay coverage (`CON-1`) and the standalone worker runtime
(`SERVE-1`).

At `1.0.0`, `dli.export.run.profile` is the closed target selector:
`context-pack`, `cards`, `ludii-gdl`, `articy`, `csv`, or `static-wiki`.
Optional `profile_name` selects a qualified entry from the caller project's
validated catalog. Lore-document catalogs are declared by
`documents.<id>.profiles`; game catalogs remain under `games.<id>.profiles`.
The response manifest's historical `commit` field is non-empty even though the
worker never reads git: it carries an order-independent
`content-sha256:<hex>` fingerprint of the selected document source set or game
rules file. `profile_hash` independently covers the resolved profile and
artifact-shaping options.

### 3.2 Authoring tools — `author.*` (the tool surface)

These are the curated semantic tools agents author through (`decisions.md`
DL-0014). Every tool that **stages or commits a declaration** returns the same
immediate-feedback shape (`author.branch` and `author.merge` act on branches
rather than declarations and return their own typed shapes, below):

```
{ findings:  [ { severity: error|warn, code, message, grade, repair, citations } ],
  workspace_digest: "<RFC 8785 JCS digest of the workspace>",
  rendered:  [ .dli lines — the opt-in view; empty when no formatter is linked ],
  root_hash: "<lorestore commit hash — author.materialize only>",
  facts:     [ the committed fact objects — author.materialize only ] }
```

`findings` and `workspace_digest` are always present. `root_hash` and `facts`
appear from `0.6.0` up and only where the table says so; a `0.5.1` caller sees
neither, and its `rendered` array keeps the pre-`0.6.0` bytes exactly (§6). The
two digest regimes never share a field: `workspace_digest` is always the
fence-valid `jcs:sha256:` workspace digest, and a lorestore commit hash always
travels in `root_hash` (DL-0046).

A finding's `severity` (`error` | `warn`) is first-class and **gates
materialize**: an `error`-class finding blocks `author.materialize`;
`warn`-class findings pass. Severity is orthogonal to `grade` — `grade` is
evidence strength (the `checked > tested > attested > asserted` lattice), never
a gate. A finding's `code` is a **finding code** — dotted `family.item` form
(`name.unvetted`, `entity.unknown`, `consequence.changed`), disjoint from the
§9 error-code registry (which names transport/op failures, not lore verdicts).
Each check family's finding codes are pinned by its fixtures as the family
lands; `repair` carries the fix data; `citations` the support.

**Provenance over the tool surface depends on the negotiated version** — this is
the authority scrub (DL-0054, implemented at `1.3.0` by DL-0058), and it is the
one place where what an agent may assert changes with the version it speaks.

**Below `1.3.0`**, provenance is `proposed` or `hypothesis` only (B1). A `cited`
provenance (`{"kind":"cited","decisions":[…]}`) supplied to any `author.*` tool is
refused with the legacy cited-provenance refusal finding. Every `0.x` wire keeps that
refusal byte-exact until version retirement, and `1.0.0`–`1.2.0` keep it too.

**At `1.3.0` and above**, the engine stops asking *who* is citing and starts
recording *what* is cited:

- **A citation of a *declared* decision is recorded, and the claim reads
  certain.** This is the scrub's purpose, not a side effect. A `cited` provenance
  lowers to a real `decided` marker carrying the cited ids, and because the
  corpus declares that decision its variable evaluates true — so the claim stands
  as decided rather than being flattened. That flattening is exactly what the
  pre-scrub behavior did, and why an agent could not previously record a ruling
  it had every right to cite. Every caller of this engine is a machine; a human
  ruling reaches it by a machine citing the consumer-ledger id that recorded the
  ruling. Review and approval workflow belongs to the consuming application, not
  here.
- **A citation of an *undeclared* decision is caught, and that is the guardrail.**
  A citation naming a decision the corpus does not declare raises
  `citation.unknown-decision` — an **error-severity finding**, graded `checked`,
  returned by the mutating call itself and raised again over the compiled staged
  world. It is **not** a transport-level refusal: the add answers, the finding is
  in that answer, and `check::run`'s error severity is what bars materialize. **A
  consumer harness that watches only for a refused frame will sail straight past
  it** — read the findings.
- **The guarantee sits one layer below the finding.** `dli-eval` sets a
  decision's variable true only when the corpus declares that decision, so a
  forged id leaves the variable **open** and the claim can never evaluate
  certain, whatever the caller stated. An agent cannot cite a non-existent
  decision into a believed fact; the worst it can do is stage a claim that reads
  as merely possible and cannot be materialized.

**Naming one staged declaration.** `author.status` returns
`staged: [{key, kind, rendered?}]` — `key` is the structural key,
`jcs:sha256:<hex>` over the declaration's canonical JCS bytes, and from `1.3.0`
`author.discard`'s `decl` accepts it. `kind` is enough to read the list without a
formatter; `rendered` is present only on a build that links one, which is why the
key exists at all — on a default build it is the only name a declaration has.

> **How long a key is good for, stated plainly.** JCS fixes the *encoding*: keys
> sort, number and string forms are specified, so the key does not depend on
> field declaration order, on which serializer produced the bytes, or on which
> build is running — a default build and a `syntax` build compute the same key
> for the same declaration, and any holder of the declaration can recompute it.
> It is **not** stable across a schema change that adds a field which serializes
> — including a field that serializes as `null`, which is what an `Option`
> without `skip_serializing_if` does when absent, and the declaration types do
> not carry that attribute on their optional fields today. A key is therefore
> valid **within a contract version**: hold it across a call, not across an
> upgrade, and re-read `author.status` after negotiating a new version rather
> than persisting keys through one.

| Tool | Purpose | Mutating |
|---|---|---|
| `author.open` | Open a workspace (a staging fork) over the world; `branch` (**≥ `1.7.0`**) opens against a named branch head instead of `main`. | yes (creates) |
| `author.status` | The workspace's staged declarations and open findings. | no |
| `author.diff` | The structured review diff — per-fact `rows` with before/after standing, from `0.6.0` — with the full findings and derived consequences, including merely-possible status; the `.dli` rendering is a view of it. | no |
| `author.discard` | Discard the workspace, or one staged declaration — the **reject** decision. | yes |
| `author.materialize` | Commit the workspace's staged delta to the lorestore — the **accept** decision; returns the committed fact objects and the new `root_hash`. | yes (commits) |
| `author.add_entity` | Add a proposed entity (kind: person/place/thing/era). Events are not entities — chronology points go through `author.add_event`. | yes |
| `author.add_fact` | Add a proposed fact on an entity. | yes |
| `author.add_relation` | Add a proposed relation between entities. | yes |
| `author.add_event` | Add a chronology event. | yes |
| `author.add_before` | Add a before-fact (an ordering; itself a ruling-shaped claim). | yes |
| `author.open_question` | Open a question — a tracked debt. | yes |
| `author.add_option` | Add an option to an open question. | yes |
| `author.add_hypothesis` | Add a hypothesis variable (one per proposal bundle). | yes |
| `author.suppose` | Pin minted hypotheses true within the workspace (fork-only). | yes |
| `author.query` | Query the compiled world (entity lookup, relation traversal, timeline). | read |
| `author.check` | Run the full check battery over the workspace and return findings. | read |
| `author.why` | Explain why a fact holds: provenance chain + grade derivation. | read |
| `author.findings` | Return all outstanding findings for the workspace. | read |
| `author.list_workspaces` | List open workspaces and their summary status; each row's `branch` (**≥ `1.7.0`**) names the branch it targets, if any. | read |
| `author.branch` | Create a persistent lorestore branch, list them, or (**≥ `1.7.0`**) `delete` one; `create`/`delete` accept an `expected_ref` fence and the result carries per-branch `heads` (added at `0.6.1`). | yes (`create`, `delete`) |
| `author.merge` | Merge a named branch into the target branch (default `main`); returns the merge commit, the merge base, structural conflicts, and findings. **≥ `1.7.0`**: `dry_run` inspects the merge without committing it (the same content-addressed commit a later real merge produces), and `expected_ref` fences the target (added at `0.6.1`). | yes |
| `author.add_dialect` | **≥ `1.3.0`.** Declare a dialect — the explicit register `dli.name.check` vets against. | yes |
| `author.add_name_part` | **≥ `1.3.0`.** Declare a name part (a morpheme a name is built from). | yes |
| `author.add_name` | **≥ `1.3.0`.** Declare a name, optionally from declared parts, in a declared dialect. | yes |
| `author.add_term` | **≥ `1.3.0`.** Declare a term of art with its gloss. | yes |
| `author.add_banned` | **≥ `1.3.0`.** Declare a banned word, with the reason it is banned. | yes |
| `author.add_state` | **≥ `1.3.0`.** Declare a time-varying fluent — the shape episode continuity is read over. | yes |
| `author.add_attest` | **≥ `1.3.0`.** Raise an attestation request against a claim (the question-ledger door). | yes |
| `author.set_standing` | **≥ `1.5.0`.** Promote a provisional stored claim to ratified; `decision` must be declared and support every path. | yes (commits) |
| `author.defeat` | **≥ `1.5.0`.** Record a new defeating decision and rule the named old supporting decision false; the claim remains stored but evaluates dead. | yes (commits) |
| `author.declare_functional` | **≥ `1.6.0`.** Declare a predicate single-valued (a functional dependency), so two distinct certain values for one subject raise `continuity.fd-violation` — including for a first-seen predicate canon has no precedent for. | yes |
| `author.assert_same_as` | **≥ `1.7.0`.** Assert two entity ids name the same bearer; the evaluable world closes the merge symmetric-transitively, gated on the citing claim's own standing (defeasible: defeating the citation un-merges every derived consequence). | yes |
| `author.add_rule` | **≥ `1.7.0`.** Declare one datalog derivation rule (a typed `head :- body` clause); every fact it derives carries this rule's own provenance, defeasible on the rule's own citation like any other claim. Refuses to stage on a bad rule shape or a stratification failure, so a bad rule never reaches the workspace. | yes |
| `author.declare_inertial` | **≥ `1.8.0`.** Declare a predicate's fluent values persistent (inertial): a value asserted `since` a point holds forward until a later, distinct value defeats it. Stages a carriage-only declaration exactly like `author.declare_functional`; the `continuity` check reads it to bound an inertial value's forward persistence at the next distinct certain value (a silent transition, a `continuity.fluent-order-undeclared` warn, or a `continuity.fluent-contradiction` error). | yes |
| `author.as_of` | **≥ `1.9.0`.** Project the inertially-current fluent value(s) of `{pred, subject}` at a query point `{at}` — the one inertia-aware read; kernel-side via `check::slot_state_at`, never the eval query path `author.query` answers over (`author.query` still returns the raw extensional value(s) unmodified). | read |

**`author.materialize` is a mutation, and it is the accept decision**
(DL-0046). It commits the workspace's staged delta to the lorestore and
advances the workspace's pinned commit, answering with the committed fact
objects (`facts`) and the commit hash (`root_hash`); the `.dli` rendering is
the `rendered: true` opt-in, not the return. `author.discard` — whole-workspace
or per-declaration — is the matching **reject** decision, and a rejected
declaration's durably staged objects are dropped from the pending commit.
Re-materializing with nothing newly staged writes no new commit and returns the
pinned one.

The kernel still never writes the fantasy repo: the commit lands in the
lorestore the workspace was opened over, which is deloreis-side storage. What
crosses the boundary is JSON. The commit is version-independent, but the
**return** is not: a `0.5.1` caller receives the pre-`0.6.0` shape — the
rendered `.dli` lines, byte-identical, with no `facts` and no `root_hash` — and
so keeps its file-writing flow until `EXT-F4` (§6).

## 4. The fixture registry

The fixture registry is **closed**: *an operation or tool without golden,
negative, and framing fixtures does not exist.* A new op or tool is added to the
contract only together with its three fixtures. This completeness rule binds
today: contract `1.9.0` enforces it for all 64 operations.

- **Golden** — a valid call and its expected verdict.
- **Negative** — a malformed or refused call and its expected error.
- **Framing** — a paired control/attack replay from independent cold state.
  Exact changed request paths are declared; required verdict and safety
  projections must match; tainted strings may appear only in declared data
  paths. Mutating cases probe post-state, and materialized output must reparse.
- **Precondition.** A fixture whose call needs prior state — an opened workspace,
  a created fork, staged entities — declares its setup as a `precondition` array
  of send-frames, driven through the server **cold** before the fixture's own
  frames and **not themselves asserted**. A single-frame `author.*` fixture, for
  example, declares `[author.open {scope: …}]` so the harness-opened `ws-1` is
  reproducible from the vendored contract rather than assumed by an in-repo
  harness only. Tainted-state reads use independent control and attack
  preconditions so the read request can be identical while staged data differs.
- **Expected outcome.** A fixture declares an `expected_outcome` so replay is
  unambiguous: `replay` (the default — each frame's response equals its `expect`),
  or `gated` (reserved for an operation unavailable under the loaded project or
  current implementation).
- **Cold replay.** Every registered fixture — golden, negative, framing, and
  author — is driven through the server **cold**, from a fresh session, with its
  declared precondition, and the declared outcome is asserted (`fixture_replay`
  in `dli-serve`; the fantasy mirror remains consumer-owned integration work).
  Paired fixtures use the strict, digest-pinned `fixture-project`; unknown ids
  are refused exactly as they are in project mode. A fixture that pins a
  verdict the running engine does not produce **fails the gate**: the manifest
  cannot silently document a fiction.
- **Verdict parity.** The same fixture input must yield the same verdict on the
  deloreis side and the fantasy side. Deloreis replay is closed here; cross-tree
  parity remains blocked on consumer integration.
- **Digest domains.** Fixture files are pinned by **sha256 over exact file
  bytes** in `contracts/dli-worker/registry.yaml` — the registry IS the
  manifest, and the gate verifies it against the fixtures on both sides.
  Digests over *payloads and workspaces* (e.g. `workspace_digest`) use the
  **RFC 8785 JSON Canonicalization Scheme (JCS)**, so those are stable across
  serializers and languages.

## 5. Authority and the sync protocol

`deloreis` is the sole author of this contract; fantasy consumes it vendored,
pinned, and digest-checked. A contract change follows four steps, in order:

1. **Rule and revise.** deloreis lands the change — a `DL` entry in
   `decisions.md` if it is a decision — bumps the contract version (§6), and
   updates the affected fixtures and the `MANIFEST`.
2. **Tag.** deloreis tags a build carrying the new contract and its fixtures.
3. **Re-vendor.** fantasy re-vendors the contract at the new digest and re-runs
   the fixtures in its own tree for verdict parity.
4. **Acknowledge.** The corresponding coordination point moves to `verified` on
   the `STATUS.md` board (single-writer, deloreis side); the change is closed.

No unilateral reimplementation on either side; the contract document, not the
code, is the authority.

## 6. Versioning

The major arrived **before** the consumer cutover, not at it (DL-0051,
DL-0053). The earlier plan — bump to v1 at the first per-check flip — was
overtaken: the engine's own pivot to fact objects (DL-0041) is what earned the
major, and holding `1.0.0` back until the consumer moved would have meant
shipping the pivot under a `0.x` label. So the version and the cutover were
decoupled, and the compatibility window carries the consumer across.

- **The accepted window.** The server accepts `0.5.1`, `0.6.0`, `0.6.1`,
  `0.6.2`, `0.6.3`, `0.6.4`, and `1.0.0` through `1.9.0`. A frame declaring
  anything else is refused `version-mismatch` (§9), never reinterpreted.
- **The caller picks; the server echoes.** Every response is shaped for, and
  labelled with, the version the caller declared. A vendored `0.5.1` consumer
  sees `0.5.1` bytes from a `1.0.0` build — the `ls8_zero_x_parity` oracle
  replays a captured pre-`1.0` frame corpus in the gate and holds every `0.x`
  response byte-identical. `1.0.0` is opt-in by negotiation, exactly like every
  version before it.
- **What `1.0.0` changes.** The field-level killed-form registry activates at
  major ≥ 1: a `1.0.0` request carrying a retired string field (`goal`,
  `declarations`, a payload claim's `atom`) is refused `killed-form` naming its
  typed successor (`goal_atom`, `additions`, `atom_expr`). Below `1.0.0` those
  fields stay required and the string form still answers.
- **What `1.1.0` changes.** Storage `SCHEMA_VERSION` 3 (DL-0055): stored fact
  objects carry provenance metadata, their identity projection no longer
  includes source position, and the `decided`/`ratified`/`provisional` status
  distinction now survives a store round trip. **No new wire operations** —
  `1.1.0` added none, and the only wire-visible change is that it joins the
  accepted list. The version moves because the stored canonical bytes did: under
  schema-first the fact-object schema *is* part of this contract, so a change to
  its bytes is a contract change even when no operation moves.
- **What `1.2.0` changes.** The corpus exchange (DL-0056): `dli.corpus.import`
  and `dli.corpus.export` join the inventory, taking it from 48 operations to
  50. They are gated on the **negotiated version**, not on the build — only a
  `1.2.0` handshake lists them in `dli.version`, so every earlier version's
  capability list, and the golden fixture recording it, stays byte-identical.
  Nothing is retired; the accepted window only widens. A build that links no
  renderer answers `format: rendered` with a loud `not-wired` refusal naming the
  ways forward, never a silent empty array.
- **What `1.3.0` changes.** Two things, both additive (DL-0058). **Seven expert
  authoring tools** join the surface, taking the inventory from 50 operations to
  57: `author.add_dialect`, `add_name_part`, `add_name`, `add_term`, `add_banned`
  (the wave the consumer's corpus actually needs) plus `add_state` and
  `add_attest`. Like every op added since `0.6.1` they are gated on the
  **negotiated version**, so an earlier handshake's capability list — and the
  golden fixture recording it — stays byte-identical. And **the authority scrub**
  lands: at `1.3.0` a mutating call may carry citation-bearing provenance,
  recorded as data, with referential integrity kept fail-closed (§3.2). Below
  `1.3.0` the legacy cited-provenance refusal behavior is unchanged, on every version,
  permanently.
- **What `1.4.0` changes.** The canon-normalized `dli.corpus.export` format
  (DL-0069) joins the existing `objects` and `rendered` forms. It returns the
  store-derived `canon` projection alongside the unchanged corpus fields, and
  is gated on the negotiated version: `1.3.0` and earlier retain the released
  schema refusal for `format: canon`.
- **What `1.5.0` changes.** Schema v4 retires stored `Decided` standing in
  favor of `Provisional`, with v3 objects upcast on read and claim identities
  unchanged. Cited authoring requires an explicit `provisional` or `ratified`
  state. `author.set_standing` and `author.defeat` join the negotiated surface,
  taking the inventory to 59 operations. Promotion records its ratifying
  decision; defeat records a new decision and rules the old supporting
  decision false. Canon excludes dead claims and refuses corrupt commits whose
  decision list disagrees with stored `Ratify` objects.
- **What `1.6.0` changes.** One additive operation,
  `author.declare_functional` (DL-0074), taking the inventory to 60. It declares
  a predicate single-valued, so canon holding two distinct certain values for
  one subject raises `continuity.fd-violation` (DL-0073) — the read-side twin of
  the merge engine's conflict check, and the only way to judge a *first-seen*
  predicate, which precedent-based inference cannot. The op is advertised only
  from `1.6.0` up, so a `1.5.0`-or-earlier caller's capability list is
  byte-identical to what it was; nothing else on the wire moves.
- **`1.7.0` — complete.** The wire-surface-completion campaign window
  (`decisions.md` DL-0077) opened the version. The scaffolding step added
  **zero new operations** — the inventory stayed at 60, and a `1.7.0`
  handshake's capability list was byte-identical to `1.6.0`'s — but the
  version was accepted, and the three floor predicates the campaign's
  additions gate on (`has_identity_assertions`, `has_rule_authoring`,
  `has_branch_writes`) existed from that commit. Every pre-`1.7.0` wire stays
  byte-identical.

  **Branch-scoped writes (WS-3) landed**, params-only — the inventory stayed
  at 60 ops for this step. `author.open` gained `branch: Option<String>`: it
  routes the workspace's reads and every subsequent write (`materialize`,
  `set_standing`, `defeat`) to that branch's own ref instead of `main`, and
  echoes it back on the result and on `author.list_workspaces` rows, all
  gated `has_branch_writes` (≥ `1.7.0`; a schema violation below it).
  `author.merge` gained `dry_run: Option<bool>` (writes the same
  content-addressed merge commit a real merge would and runs the full
  conflict scan, but skips the target-ref CAS — a later real merge of the
  same two heads produces the identical commit hash) and
  `expected_ref: Option<String>` (fences the target branch's whole-ref
  digest). `author.branch` gained the `delete` action (pointer removal only;
  refuses `main` and refuses while any open workspace still targets the
  branch) and an `expected_ref` fence on `create`/`delete`; its result gained
  `heads: Option<Vec<{name, commit, ref_digest}>>`. `author.branch`/
  `author.merge` now accept the envelope's `expected_digest`/
  `idempotency_key` fences from `1.7.0` up (a retry with the same
  idempotency key replays instead of re-executing). The certain-class
  conflict that previously needed raw lorestore access to construct is now
  wire-only: `author.defeat` on a branch, citing a new decision, rules the
  base's supporting decision false on that branch's own ref valuation;
  citing the new decision for a rival value and materializing succeeds
  there; merging surfaces the certain-layer conflict.

  **`1.7.0` adds `author.assert_same_as`** (DL-0078, the identity kernel;
  DL-0080, the wire exposure) and **`author.add_rule`** (DL-0080, rule
  authoring), taking the inventory from 60 to 62 operations, each gated on
  its own floor predicate (`has_identity_assertions`,
  `has_rule_authoring`) rather than an exact version match, so every
  `1.6.0`-or-earlier negotiation's capability list stays byte-identical; a
  `1.7.0` handshake differs from `1.6.0`'s by exactly these two ops, in
  append order. `author.add_rule` derives every fact through a declared
  `head :- body` clause with the rule's own provenance riding into each
  derivation (defeasible on the rule's own citation, exactly like any other
  claim); a functional-dependency break visible only once rules saturate
  surfaces as `continuity.derived-fd-violation` on `author.check` and blocks
  `author.materialize` the same as the pre-saturation `continuity.fd-violation`
  does. Also `1.7.0` (DL-0080, no op, no contract-version-visible surface
  beyond what is named here): **the `Finding.data` structured-payload
  rider** — every wire `Finding` gains an optional `data` field
  (additive, `skip_serializing_if`, so a pre-`1.7.0` response's bytes are
  unchanged), populated only on a `≥1.7.0` negotiation and only for the
  finding classes documented to carry one (`continuity.fd-violation` and its
  derived sibling, `continuity.fluent-contradiction`,
  `continuity.fluent-order-undeclared`) — enough for a consumer to
  reconstruct the verdict without parsing the message; and **two temporal
  close-out findings** over the existing chronology/fluent machinery:
  `state.anchor-kind` (error, at add time) when a state's temporal anchor
  names a KNOWN entity of the wrong sort, with a typed repair naming valid
  alternatives, and `continuity.fluent-order-undeclared` (warn,
  non-blocking) when a fluent slot's ambiguity would resolve if an
  `author.add_before` declared the order between two named anchors — the
  contradiction check itself still never fires on undeclared order; this
  only names the gap. This closes the campaign: `1.7.0`'s surface is now
  complete at 62 operations.
- **What `1.8.0` changes.** One additive operation, `author.declare_inertial`
  (`decisions.md` DL-0081), taking the inventory to 63. It declares a
  predicate's fluent values persistent (inertial) — a value asserted `since`
  a point is meant to hold forward until a later, distinct, live value on
  the same slot defeats it — mirroring `author.declare_functional`'s
  carriage-only shape exactly. The op is advertised only from `1.8.0` up
  (`has_inertia`), so a `1.7.0`-or-earlier caller's capability list stays
  byte-identical. The declaration stages, materializes, and carries through
  forks/merges/the lorestore, and the `continuity` check reads it: an open
  inertial value's forward persistence is bounded at the next distinct certain
  value, composing into a silent transition, a
  `continuity.fluent-order-undeclared` warn, or a
  `continuity.fluent-contradiction` error. The finding enrichment rides the
  existing `has_finding_data` floor, so this adds no new wire surface beyond
  the one op.
- **What `1.9.0` changes.** One additive operation, `author.as_of`
  (`decisions.md` DL-0082), taking the inventory to 64. It is the one
  inertia-aware read: `{workspace, pred, subject, at}` projects the value(s)
  the slot holds at the query point directly — kernel-side, via
  `check::slot_state_at` (reusing `state_at_inertial`/`state_at` and the
  continuity check's own canon-grouping and `inertial_derived_until`
  boundary derivation), never `dli_eval::world::World::query_with` or the
  saturated derived layer, so a `(pred, subject)` with no fluent timeline
  answers empty `holdings` rather than falling back to the raw extensional
  fact. This **lifts** DL-0081's v1 read boundary ("no read is
  inertia-aware"): `author.query`/`dli.query` are unchanged and still return
  the raw extensional value(s), so the non-leak invariant holds for them
  specifically, but a silent transition's derived boundary is no longer
  observable only through `continuity.fluent-*` `Finding.data` — this read
  is the direct answer. The op is advertised only from `1.9.0` up
  (`has_as_of_read`), so a `1.8.0`-or-earlier caller's capability list stays
  byte-identical, and it routes through the read seam (never `mutate`), so
  it stages nothing and carries no idempotency fence.
- **Retirement runs through the registry.** After `EXT-F4` — the consumer's
  move onto `1.0.0` — the `0.x` versions retire the way killed forms always do:
  each retired version is registered with its successor and the ruling that
  retired it, and a frame declaring one gets a deterministic refusal naming
  both. A version is never dropped silently, and the registry entry outlives
  the code path behind it.
- **N / N-1 compatibility applies from `2.0.0` onward** — the server supports
  the current and the immediately previous major, so a re-vendor is never a
  flag day. It does not describe the `0.x → 1.0.0` window above, which is
  wider by design and closes at `EXT-F4`.

Because the envelope carries no timestamp, the version is always explicit in
the frame, in `dli.version`, and in the fixture manifest — never inferred.

## 7. What deloreis guarantees

- **Deterministic and offline.** Every verdict replays from committed inputs;
  the kernel never calls a model, opens a socket, or reads a clock.
- **Immediate graded feedback.** Every mutating `author.*` call returns findings
  (graded, with repair data), the structured result, and the workspace digest on
  the same call, as required by DL-0014; the `.dli` rendering is the opt-in view
  of that result (§3.2).
- **Fail-closed exports.** An export reads the certain layer and never emits
  proposed lore by accident.
- **Provenance in, provenance out.** Nothing enters without an origin; every
  finding renders its support and its grade.
- **The contract outranks the code.** A server behavior that contradicts this
  document is a server bug (`internal`, §9), not a new rule.

## 8. What fantasy must do

- **Class-declared LLM calls.** Every factory LLM call declares which of the four
  classes its output is — **attestation**, **candidate**, **encoding**, or
  **gloss** — and reaches the kernel only through that class's single input
  surface. There is no unclassified path.
- **Quote anchors on encodings.** Every encoded item carries a verbatim quote and
  line; the kernel verifies the quote appears at the claimed position.
- **Job orchestration.** The consumer owns queue policy, retries, and UI around
  worker-owned durable jobs. Exact start retries are content-addressed and
  idempotent; status polling resumes pending or running work after restart.
- **Digest-checked vendoring.** The contract copy is pinned to a digest and the
  gate fails if it drifts.
- **Authorship through the tool surface.** Agents author lore through the `author.*` tools,
  not by editing `.dli` text. Raw `.dli` editing is a measured fallback, never the
  taught path.

## 9. Error taxonomy

Every non-success return has **two layers**: a **class** and a **code**. The
class is one of four; the code is drawn from a closed registry and pinpoints the
cause.

**The four classes:**

| Class | Meaning | Example |
|---|---|---|
| `schema` | The call is malformed — an envelope or payload violation, an unknown field, a wrong type. | a `dli.check` payload missing `document` |
| `refused` | The operation is refused by principle. | `export --write-back`; calling the killed `dli.suppose`; asserting proposed lore |
| `finding` | A graded domain finding — the normal "your lore has a problem" path. | `attestation-missing`; a wrong-hour proof error; a `beat-unaccounted` conformance finding |
| `internal` | A server defect. Never a lore verdict; always a bug to fix. | an evaluator panic |

`finding` is the ordinary channel and carries the full graded shape (§3.2);
`schema` and `refused` are the call's own fault; `internal` is deloreis's fault
and is never dressed up as a lore verdict.

**The closed code registry.** Every error carries exactly one `code` from this
closed set, and each code maps to a class:

| Code | Class | Cause |
|---|---|---|
| `bad-frame` | schema | a malformed envelope |
| `unknown-op` | schema | an operation or tool the contract does not define |
| `schema-violation` | schema | a payload that violates its op/tool schema |
| `unknown-doc` | schema | a reference to a workspace, fork, or entity that does not exist |
| `unstratifiable` | finding | a negative-dependency cycle caught at load (FRONT-1) |
| `contradiction` | finding | a claim that contradicts the projection |
| `stale-fence` | refused | `expected_digest` did not match the current fence |
| `materialize-blocked` | refused | `author.materialize` called with error-class findings open; the blocking findings ride in the error data |
| `killed-form` | refused | a killed or renamed op/tool (e.g. `dli.suppose`, `dli.check-ledger`); the message names the successor |
| `not-wired` | refused | a recognized op whose engine is not wired in this build — capability-gated (see `dli.version` capabilities); permanent-until-wired, not a defect |
| `version-mismatch` | refused | a well-formed frame declaring a contract version this build does not speak |
| `conflict` | refused | an `idempotency_key` reused with a *different* payload — the key is bound to its first payload (distinct from `stale-fence`, which means the digest moved) |
| `project-required` | refused | a durable operation was called without a caller-owned project |
| `evidence-corrupt` | internal | committed evidence is malformed, non-canonical, misaddressed, or has broken lineage |
| `unknown-job` | schema | a durable simulation job handle does not exist |
| `internal` | internal | a server defect |

## 10. Change control

- Contract changes go through the four-step sync (§5); a change that is a
  decision also lands a `DL` entry in `decisions.md`, which outranks this file.
- Killed forms (like `dli.suppose`) are registered and refused, never quietly
  removed — the server names the successor.
- No coordination point advances without its evidence link on `STATUS.md`; no
  new op or tool exists without its three fixtures (§4).
