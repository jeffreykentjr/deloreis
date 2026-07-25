---
status: live
verdict: "The pre-v1 generic worker contract and intended guarantees. Caller-project isolation is implemented; semantic, durability, and assurance closure remain open under DL-0028, and consumer integration remains blocked."
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

> **Implementation status:** this is still a pre-release contract. All operation
> names have schemas and dispatch arms, and both transports load strict caller
> projects. Several handlers remain incomplete or non-durable, and the fixture
> census has framing exemptions.
> The PLAN matrix therefore keeps CON-1, SERVE-1, and AUTH-1 open. Nothing in
> this document is evidence that consumer cutover is ready.

## 1. Parties

- **deloreis** — the opaque `deloreis` binary and its server. It compiles,
  checks, queries, simulates, exports, and hosts Door T. It is deterministic and
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
- **Two declared optional request fields.** A mutating call may carry
  `expected_digest` — the caller's last-seen workspace or fork digest, for
  optimistic concurrency: a mismatch is a `stale-fence` error (§9), so a stale
  caller never clobbers a moved fence — and `idempotency_key` — a retry-safe key,
  so a re-sent mutating `author.*` call is applied at most once. Both are
  **declared** fields; "unknown fields are errors" still stands — these are now
  known. Native stdio carries them in the request envelope. MCP exposes them as
  reserved tool arguments and lifts them into that same envelope before shared
  dispatch.
- **Every mutating call returns immediately.** No mutating operation defers its
  verdict; the graded findings, the rendered canonical lines, and the workspace
  or fork digest come back on the same call.

## 3. Operation inventory

### 3.1 Batch operations — `dli.*`

| Op | Purpose | Returns |
|---|---|---|
| `dli.version` | Report the contract/grammar versions and this build's op capabilities. | `{contract, contract_version, server, capabilities}` |
| `dli.check` | Compile and check a scope or corpus. | graded findings + rendered lines |
| `dli.query` | Query the compiled world (`--layer/--as-of/--fork/--why`). | atoms + support + rendered |
| `dli.fork.new` | Create a fork. | fork handle |
| `dli.fork.list` | List forks. | forks with base/assumes/counts |
| `dli.fork.diff` | Derived-consequence diff of a fork. | `newly_certain/possible/dead/grade_changed/conflicts`, each rendered |
| `dli.fork.suppose` | Assume an option or event in a fork. | diff delta |
| `dli.fork.add` | Add proposed declarations to a fork. | `ok` + diff delta |
| `dli.fork.suppress` | Retract an assumption from a fork. | diff delta |
| `dli.fork.restrict` | Restrict a fork's valuation scope. | diff delta |
| `dli.fork.preflight` | Ratification pre-flight: candidate valuation + conflict scan. | conflicts + affected atoms |
| `dli.fork.promote` | Emit a ruling-request draft from a fork. | `rq_draft` (slug, ask, affects, diff, evidence) |
| `dli.check-proof` | Check a diegetic proof `in <practice>`. | verdict + errors (structural classes) + grade |
| `dli.check-draft` | Run the full battery over a draft payload. | one merged report |
| `dli.simulate.start` | Start a battery over a ruleset/delta. | `job_id` + status |
| `dli.simulate.status` | Poll a battery. | progress, or final manifest + metrics + invariant verdicts |
| `dli.simulate.cancel` | Cancel a battery. | status |
| `dli.timeline.show` | The timeline at a position (`--at`). | lanes, events, spans, placements |
| `dli.timeline.order` | Partial-order report. | cycles + load-bearing unordered pairs (ready-made RQs) |
| `dli.timeline.knows` | Who-knows-what matrix at a position. | characters × facts (listener included) |
| `dli.timeline.episodes` | Episode-order check + slack. | findings + earliest/latest legal slots |
| `dli.timeline.phases` | Phase-walk verdict for an episode. | walk consistency |
| `dli.export.run` | Run an export target and profile. | target-shaped emission + manifest |
| `dli.name.check` | Vet a name (the kernel half only). | kernel verdict (world-true) |
| `dli.attest.missing` | List the open attestation questions checks demand. | question records |
| `dli.attestation-sweep` | Validate and record an attestation sweep's results into the store. | accepted records + staleness report |
| `dli.payload-submit` | Submit a `payload/v1` document: validate, quote-anchor-check, store. | findings + payload digest |

`dli.fork.suppose` **supersedes** `dli.suppose`, which is a **killed form** — it
is registered as such and the server refuses it with a `refused` error naming
the successor. `dli.check-proof` likewise **supersedes** `dli.check-ledger`,
renamed by `decisions.md` DL-0016 (the surface-v1 pass that renamed the
in-practice proof block keyword `ledger`→`proof`); the old op name is a killed
form the server refuses, naming its successor.

All 46 operation names have request/response schemas and dispatch arms.
That census is not a readiness claim. Caller-project isolation now loads a
strict versioned manifest and refuses demo fallback, but `capabilities()` still
advertises operations whose full Returns semantics are open; simulation is
synchronous and session-local; payload and attestation mutations do not store
evidence; and several responses omit promised semantics. CON-1 closes only when
the handlers satisfy these Returns columns and the complete fixture oracle
passes.

### 3.2 Authoring tools — `author.*` (Door T)

These are the curated semantic tools agents author through (`decisions.md`
DL-0014). Every **mutating** tool returns the same immediate-feedback shape:

```
{ findings:  [ { severity: error|warn, code, message, grade, repair, citations } ],
  rendered:  [ canonical .dli lines the call produced ],
  workspace_digest: "<RFC 8785 JCS digest of the workspace>" }
```

A finding's `severity` (`error` | `warn`) is first-class and **gates
materialize**: an `error`-class finding blocks `author.materialize`;
`warn`-class findings pass. Severity is orthogonal to `grade` — `grade` is
evidence strength (the `checked > tested > attested > asserted` lattice), never
a gate. A finding's `code` is a **finding code** — dotted `family.item` form
(`name.unvetted`, `entity.unknown`, `consequence.changed`), disjoint from the
§9 error-code registry (which names transport/op failures, not lore verdicts).
Each check family's finding codes are pinned by its fixtures as the family
lands; `repair` carries the fix data; `citations` the support.

**Provenance over Door T is `proposed` or `hypothesis` only** (B1). An agent
cannot mint a citation — a ruling lives in the ledger, not a tool call — so a
`cited` provenance (`{"kind":"cited","decisions":[…]}`) supplied to any `author.*`
tool is refused with the `provenance.cited-refused` finding. Human authors cite
through Door H; the agent proposes and the human confirms the citation in review.

| Tool | Purpose | Mutating |
|---|---|---|
| `author.open` | Open a workspace (a staging fork) over the world. | yes (creates) |
| `author.status` | The workspace's staged declarations and open findings. | no |
| `author.diff` | The rendered canonical block + the full findings, including merely-possible status. | no |
| `author.discard` | Discard the workspace, or one staged declaration. | yes |
| `author.materialize` | Render the workspace to `.dli` file contents **as data**; the caller commits through its own review. | no (returns contents) |
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
| `author.list_workspaces` | List open workspaces and their summary status. | read |

`author.materialize` returns the `.dli` contents as data; the kernel does not
write the fantasy repo. The agent writes the contents to a fantasy-side staging
path and commits through fantasy review.

## 4. The fixture registry

The fixture registry is **closed**: *an operation or tool without golden,
negative, and framing fixtures does not exist.* A new op or tool is added to the
contract only together with its three fixtures. This completeness rule binds
from contract **v1** (the first cutover, `CON-1`); the v0 seed set covers the
flow families and every declared error code, not yet every op.

- **Golden** — a valid call and its expected verdict.
- **Negative** — a malformed or refused call and its expected error.
- **Framing** — an adversarially framed call (a prompt-injection-shaped payload,
  a coaxing narrative) whose expected verdict is **unchanged** by the framing.
- **Precondition.** A fixture whose call needs prior state — an opened workspace,
  a created fork, staged entities — declares its setup as a `precondition` array
  of send-frames, driven through the server **cold** before the fixture's own
  frames and **not themselves asserted**. A single-frame `author.*` fixture, for
  example, declares `[author.open {scope: …}]` so the harness-opened `ws-1` is
  reproducible from the vendored contract rather than assumed by an in-repo
  harness only.
- **Expected outcome.** A fixture declares an `expected_outcome` so replay is
  unambiguous: `replay` (the default — each frame's response equals its `expect`),
  or `gated` (reserved for an operation unavailable under the loaded project or
  current implementation).
- **Cold replay.** Every registered fixture — golden, negative, framing, and
  author — is driven through the server **cold**, from a fresh session, with its
  declared precondition, and the declared outcome is asserted (`fixture_replay` in
  `dli-serve`, mirrored on the fantasy side). A fixture that pins a verdict the
  running engine does not produce **fails the gate**: the manifest cannot silently
  document a fiction.
- **Verdict parity.** The same fixture input yields the same verdict on the
  deloreis side and the fantasy side. Parity is checked in both trees.
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

- **v0** — the pre-cutover contract. Breaking changes are allowed; no
  compatibility is promised. This is the state through the build up to the first
  cutover.
- **v0 → v1** — the version bumps to v1 at the **first cutover** (the first
  per-check flip that deletes a legacy source, `migration.md` §5).
- **From v1** — **N / N-1 compatibility**: the server supports the current and
  the immediately previous major contract, so a re-vendor is never a flag day.
  Because the envelope carries no timestamp, the version is always explicit in
  `dli.version` and in the fixture manifest.

## 7. What deloreis guarantees

- **Deterministic and offline.** Every verdict replays from committed inputs;
  the kernel never calls a model, opens a socket, or reads a clock.
- **Immediate graded feedback.** Every mutating `dli.*`/`author.*` call returns
  findings (graded, with repair data), the rendered canonical lines, and the
  workspace or fork digest.
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
- **Job wrapping.** Long operations (`dli.simulate.*`, batch checks) run as
  wrapped, checkpointed jobs, so a killed job resumes rather than restarts.
- **Digest-checked vendoring.** The contract copy is pinned to a digest and the
  gate fails if it drifts.
- **Authorship through Door T.** Agents author lore through the `author.*` tools,
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
| `internal` | internal | a server defect |

## 10. Change control

- Contract changes go through the four-step sync (§5); a change that is a
  decision also lands a `DL` entry in `decisions.md`, which outranks this file.
- Killed forms (like `dli.suppose`) are registered and refused, never quietly
  removed — the server names the successor.
- No coordination point advances without its evidence link on `STATUS.md`; no
  new op or tool exists without its three fixtures (§4).
