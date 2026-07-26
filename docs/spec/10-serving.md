---
status: live
title: "Serving: one offline, deterministic worker surface"
verdict: "One project-backed JSON/MCP worker surface at contract 0.5.0, offline and deterministic. Typed responses, durable payload and attestation evidence, restart-resumable simulation jobs, and fixture closure are complete."
---

# Serving

`deloreis serve` is how a consuming application talks to the compiled world. It
is one JSON worker surface, offline and deterministic. The fantasy factory is
the first consumer, but nothing about the surface is factory-specific: every
consumer gets the same ops, the same envelope, and the same guarantees.

## 1. Process model

`deloreis serve --project <path>` speaks **JSON over stdio**.
`deloreis mcp --project <path>` exposes the same operations to MCP clients
without changing their semantics. `<path>` is a `deloreis.toml` manifest or its
containing directory. There is **no network, no model call, and no background
service** inside the worker — a determinism the gate depends on (DL-0013). The
consuming application keeps its own orchestration, its own UI, and all of its
LLM work; the worker keeps all evaluation and verdicts.

The MCP transport is a thin wrapper, not a second protocol: it registers the
same ops as MCP tools and forwards their arguments through the same request
envelope, so an MCP client and a stdio client exercise identical kernel paths
and identical fixtures. For a mutating Door-T tool, `expected_digest` and
`idempotency_key` appear as ordinary MCP tool arguments; the adapter lifts those
two reserved keys into the shared envelope before dispatch. There is one worker
surface; MCP is a way to reach it.

The surface serves **two op families** over the same process:

- **Batch operations** under the `dli.*` namespace — the query, check,
  simulate, fork, timeline, and export operations catalogued below.
- **Authoring tools** under the `author.*` namespace — the semantic Door T tools
  (DL-0014) that agents call to write lore, MCP-registered against the opaque
  `deloreis` binary.

The two families share one process and one envelope but have distinct normative
homes. The `author.*` tool contract is specified in `spec/11-agent-authoring.md`
and `integration/fantasy-contract.md` §3; this document specifies the `dli.*`
batch surface. The split is noted here so the process model is complete; the
authoring tools are not re-specified.

### 1.1 Caller projects

Production startup always names one versioned caller project. The v1 manifest is
strict TOML: unknown fields, versions other than `1`, duplicate sources,
ambiguous document/game ids, and unsafe paths stop startup.

```toml
version = 1
name = "nine-realms"

[paths]
evidence = ".dli-evidence"
cache = ".dli-cache"

[documents.norse]
sources = ["nine-realms.dli", "runes.dli", "seidr.dli"]
# episodes = "episodes.yaml"
# outlines = "outlines.yaml"
# craft_ledger = "craft-ledger.yaml"
# killed_tokens = "killed-tokens.yaml"
# attestation_questions = "attestation-questions.json"
# profiles = "export-profiles.json"

[games.rune-wheel]
rules = "games/rune-wheel/rules.json"
profiles = "games/rune-wheel/profiles.json"
```

The keys under `documents` and `games` are stable logical wire ids. A request's
`document` names one of those ids; it is not a filesystem path. Source files for
one logical document are canonicalized, sorted, parsed by the real front end,
and compiled together before the worker accepts traffic. Episode, killed-token,
document-profile, game, and game-profile assets are loaded as typed data.

Every manifest path is non-empty, normalized, relative to the manifest
directory, and may not contain `.` or `..` components. Existing assets and their
symlinks must resolve beneath that directory. Evidence and cache paths are
materialized and canonicalized before the worker accepts traffic. They must be
dedicated, non-overlapping project subdirectories with no symlinks anywhere in
either storage tree. Neither tree may contain or hard-link the manifest or any
manifest-referenced source, episode, killed-token, game, or profile input. A
project with duplicate fork question ids is rejected because a
Door-T fork must have one document owner. Dialect ids are also unique across the
project: the current `dli.name.check` shape has no document field and therefore
resolves its dialect against the project's combined language model.

Project mode never falls back to bundled content. An unknown logical id returns
`unknown-doc`, including when it happens to be a bundled demo id. The neutral
fixtures are available only through explicit `deloreis serve --demo` or
`deloreis mcp --demo`; that mode is for examples and contract replay, not caller
discovery.

## 2. The envelope

Every request is one JSON object per line: the protocol tag, a correlation id,
the operation, and the operation's params. A mutating `author.*` call may also
carry `expected_digest` (the optimistic-concurrency fence) and `idempotency_key`:

```json
{"dli": "0.5.0", "id": "<correlation>", "op": "dli.<name>", "params": {"…": "…"}}
```

Every response echoes the protocol tag and id and is either a result or a typed
error:

```json
{"dli": "0.5.0", "id": "<correlation>", "ok": true, "result": {"…": "…"}}
```
```json
{"dli": "0.5.0", "id": "<correlation>", "ok": false,
 "error": {"code": "<stable class>", "message": "<domain-spoken>"}}
```

An error response carries `"ok": false` and a **typed error** — a stable machine
`code` from the closed registry (`integration/fantasy-contract.md` §9) plus a
domain-spoken message (koan 9, errors are lore) — never a bare string. The
normative envelope shape (fields, the two optional fences, unknown-fields-are-
errors) is `integration/fantasy-contract.md` §2; this section is its
semantics-side elaboration and agrees with it byte-for-byte.

Two envelope properties are load-bearing:

- **Responses are timestamp-free by design.** A response is a pure function of
  the corpus digest, the contract version, and the params. Wall-clock time is not
  part of that function, so it is not in the response; **the response is the
  determinism domain.** Two runs against the same corpus digest produce
  byte-identical results — what makes replay meaningful and the gate reproducible.
- **Findings sort by stable keys.** Any list of findings, conflicts, or diffs is
  ordered by stable keys (atom identity, cue location, rule id), never by
  discovery order, so diffs between runs are meaningful.

The version handshake rides `dli.version`, not the envelope: a consumer calls
`dli.version` and reads the `contract`, `contract_version`, `server`, and
`capabilities` the running build reports (the consumer pins the contract vendored
and digest-checked, DL-0010). A frame that declares a contract version the build
does not speak is refused with a typed `version-mismatch` error, never silently
misinterpreted. Ops are **capability negotiated**: `dli.version`'s `capabilities`
lists the wire op names the build actually implements, so a consumer degrades
gracefully rather than calling an op the running build has not wired — an
unwired-but-recognized op returns a typed `not-wired` error, not a silent misread.

Contract `0.5.0` returns complete typed query rows, before/after fork changes,
named preflight valuations, typed timeline reports, and target-shaped export
emissions. The generated response schemas under `contracts/dli-worker/schemas/`
are the machine-readable authority for those shapes.

`dli.check` returns `{ok, grade, findings}`. Every finding is tagged by
`domain` (`world`, `payload`, or `attestation`) rather than flattened. Payload
findings retain the committed payload digest, episode, source filename, source
line, verbatim quote, principle, and citations. Configured attestation questions
produce a closed union of pass, fail, missing, or stale outcomes: pass/fail
imply current `yes`/`no` answers at grade `attested`; missing and stale detection
grades `checked`; every state except pass fixes severity `error` and makes `ok`
false. Each union branch fixes its code, grade, severity presence, and required
evidence fields in both Rust and JSON Schema. The result grade is the weakest
procedure grade actually consumed by the complete check. `ok` means no blocking
outcome; warning findings may still be present. Current and stale attestation
findings carry the head record id, supersession link, stored and current
fingerprints, and the previous typed answer when available.

> **Future gap (not shipped at v0).** An earlier draft of this section described a
> per-response `meta` block (versions, corpus commit/digest, a `replay` reference)
> and an `{op, contract_version, corpus, args}` request shape. The v0 server emits
> and accepts neither; the shipped wire is exactly the fields above. A
> response-metadata block is a candidate for a later contract version, tracked
> separately — it is not part of the current wire.

### 2.1 Typed errors

Every error carries a **stable machine class** and a **domain-spoken message**
(koan 9, errors are lore) — the two-channel error model, so neither the machine
consumer nor the human reviewer is sacrificed to the other. A malformed request
is a schema error caught against the op's JSON Schema before any evaluation
runs; a well-formed request over inconsistent lore returns a *finding*, not an
error, because a diagnosed conflict is a result the consumer is meant to act on,
not a failure of the call. The distinction is load-bearing: the worker fails
loudly on shapes it cannot parse and reports faithfully on worlds it can.

## 3. The op man-page

The full `dli.*` inventory is normatively specified in
`integration/fantasy-contract.md`, where each op carries its request/response
JSON Schema and its golden, negative, and framing fixtures. What follows is the
semantics-side description: what each op means and which spec organ owns its
behavior. The inventory, by family:

| Op | Meaning | Owning spec |
|---|---|---|
| `dli.version` | Worker contract version and capability handshake | `integration/fantasy-contract.md` |
| `dli.check` | Validate a corpus or scope; return findings | `02-semantics.md` |
| `dli.query` | Query the compiled world — status, reach, `--why`/blame | `02-semantics.md`, `08-reasoning.md` |
| `dli.fork.new` | Create a fork | `02-semantics.md` |
| `dli.fork.list` | Enumerate committed forks | `02-semantics.md` |
| `dli.fork.diff` | Derived-consequence diff of a fork against the base | `02-semantics.md` |
| `dli.fork.suppose` | Assume an option or event value in a fork | `02-semantics.md` |
| `dli.fork.add` | Add proposed ground facts or slots to a fork | `02-semantics.md` |
| `dli.fork.suppress` | Retract an assumption from a fork | `02-semantics.md` |
| `dli.fork.restrict` | Restrict a fork's valuation scope | `02-semantics.md` |
| `dli.fork.preflight` | Ratification pre-flight: candidate valuation + conflict scan | `02-semantics.md` |
| `dli.fork.promote` | Emit an RQ draft with generated affects-list (as data) | `02-semantics.md` |
| `dli.simulate.start` | Start a checkpointed practice or game-simulation job | `04-practices.md`, `09-exports.md` |
| `dli.simulate.status` | Poll a job; a completed job's response carries the final manifest and verdicts | `04-practices.md` |
| `dli.simulate.cancel` | Cancel a running job | — |
| `dli.check-proof` | Check a diegetic proof against the scene projection | `04-practices.md` |
| `dli.check-draft` | Run the merged payload battery over a draft | `06-craft-and-payloads.md` |
| `dli.timeline.*` | `show`, `order`, `knows`, `episodes`, `phases` | `03-time.md` |
| `dli.export.run` | Compile `context-pack`, `cards`, `ludii-gdl`, `articy`, `csv`, or `static-wiki` from the certain layer; optional `profile_name` selects a project profile | `09-exports.md` |
| `dli.name.check` | Vet a name — the kernel, world-true half | `05-language-layer.md` |
| `dli.attest.missing` | List open attestation questions checks demand | `07-verdicts-and-llm-boundary.md` |
| `dli.attestation-sweep` | Validate and record a sweep's attestation results | `07-verdicts-and-llm-boundary.md` |
| `dli.payload-submit` | Submit a `payload/v1` document for checking and storage | `06-craft-and-payloads.md` |

Every successful `dli.export.run` response carries the complete target-shaped
artifact and its manifest. Because the worker has no git dependency, the
manifest's historical `commit` field contains an order-independent
`content-sha256:<hex>` fingerprint of the selected document source set or game
rules file. The profile itself and artifact-shaping options are covered
separately by `profile_hash`.

`dli.suppose` is a **killed form**, superseded by `dli.fork.suppose`, and
`dli.check-ledger` is a killed form superseded by `dli.check-proof` (the
surface-v1 rename of the proof-block keyword `ledger`→`proof`, DL-0016); the
server refuses both, naming the successor. `deloreis report --min-grade` is a CLI
rendering over `dli.check` and `dli.query` results — it is not a worker op at v0.

Three worked scenarios ground the description below. All ride the JSON-Schema
worker protocol; the consumer validates each payload against the op schema
before sending.

### 3.1 Possible-worlds browse, edit, propose

The consumer lists forks, opens one, and requests the derived-consequence diff:

```json
→ {"op": "dli.fork.list", "params": {"document": "world"}}
← {"forks": [{"fork": "fork-7", "base": "fork-3",
    "assumes": {"rq-031-b": true, "p-214": true},
    "local_atoms": 40, "restrictions": 0}]}

→ {"op": "dli.fork.diff", "params": {"fork": "fork-7"}}
← {"newly_certain": [],
   "newly_possible": [
     {"atom": {"pred": "reachable", "args": ["harbour", "island"]},
      "before": {"status": "dead", "grade": "asserted"},
      "after": {"status": "possible", "grade": "asserted"},
      "rendered": "reachable(harbour, island): dead → possible"}],
   "newly_dead": [], "grade_changed": [], "conflicts": []}
```

The `rendered` string is the kernel's own deterministic citation rendering; a
consumer may additionally gloss a paragraph for a sidebar, clearly labeled as
gloss, but the structured verdict is the only thing downstream logic reads. The
consumer supposes an option, adds a proposed fact, and — when a conflict names
the exact pair of sources — resolves or accepts, then proposes for ratification:

```json
→ {"op": "dli.fork.promote", "params": {"fork": "fork-7"}}
← {"rq_draft": {"slug": "fork-7",
    "ask": "Ratify fork `fork-7`?",
    "affects": ["reachable(harbour, island)"],
    "diff": ["reachable(harbour, island): dead → possible"],
    "evidence": ["campaign-077"]}}
```

The kernel does evaluation, diff, blame, draft content, and the generated
affects-list. The consumer does rendering, glossing, file placement, and the
review queue. **Canon is untouched until the author rules** (koan 3).

### 3.2 Game simulation jobs

Contract `0.5.0` retains the storage-format-2 job behavior introduced at
`0.4.2` (DL-0036). The request's `document` field is the stable logical id of either a
manifest `documents` entry (its first declared practice) or a manifest `games`
entry (its loaded ruleset). A game entry pins the candidate delta as part of its
rules file; the default battery deals its first available layout, measures
`completed-rate`, and checks deck conservation and legal play. A game requiring
a registered Rust verdict is refused because a standalone project cannot supply
that code through data.

A job id is the SHA-256 address of its canonical specification (storage format,
asset id, battery, seed, trial count, and checkpoint cadence), so an exact retry
under the same contract reopens the same job. A prior contract's handle is not
reinterpreted under a new storage format.

A designer defines two candidate rule-deltas as data files; the consumer
enqueues one job per candidate:

```json
→ {"op": "dli.simulate.start",
   "params": {"document": "game", "seed": 41, "trials": 100000,
              "checkpoint_every": 10000}}
← {"job_id": "sha256:…", "status": "pending"}
```

The consumer polls `dli.simulate.status`; a pending or running poll after a
worker restart schedules continuation from the greatest verified checkpoint,
never trial zero. Checkpoints form one digest-linked immutable JCS chain under
the caller project's `.dli-evidence/simulations/` root. Every predecessor is
content- and chain-verified; the selected head is replayed from its battery,
seed, and trial count before use. Terminal cancellation and completion records
pin that head and retain the immutable specification after the live spec entry
is removed, so deleting a terminal record is corruption rather than rollback to
`pending`. A cancel races only at checkpoint publication boundaries and is
terminal once committed. On completion the manifest and verdicts carry
**honest grades**:

```json
← {"job_id": "sha256:…", "status": "complete",
   "trials_done": 100000, "trials_total": 100000,
   "manifest": {
     "battery_digest": "fnv1a64:…", "seed": 41, "trials": 100000,
     "checkpoint_lineage": [10000, 20000, 30000, 40000, 50000,
                            60000, 70000, 80000, 90000, 100000],
     "metrics": [
       {"metric": "completed-rate", "trials": 100000,
        "sum": 3100, "sum_of_squares": 3100,
        "mean": 0.031, "half_width": 0.0011, "grade": "tested"}],
     "invariants": [
       {"invariant": "deck-conserved",
        "outcome": {"outcome": "held"}, "grade": "checked"}]},
   "manifest_digest": "sha256:…"}
```

Deck conservation is `checked` (an exact sweep over every deal); the completion
rate is `tested` (a Monte Carlo estimate, never laundered into a proof).
The grade lattice (koan 6; see `07-verdicts-and-llm-boundary.md`) does its job in
the payload.

### 3.3 Diegetic proof checking

An episode draft carries premise and conclusion cues in its production metadata.
The consumer submits the formal encoding of the cues:

```json
→ {"op": "dli.check-proof", "scene": "e01-s04", "speaker": "protagonist",
   "proof": {"premises": ["…"], "steps": ["…"], "spends": ["p3"],
     "close": {"atom": "holds(ward(door))", "expect": "holds"}}}
← {"verdict": "invalid",
   "errors": [{"class": "L2", "name": "premise-not-available", "at": "p2",
     "message": "hour(slack-tide) is not established in e01-s04; last established hour is high-water",
     "cue": "line 118"}],
   "expected": "holds", "got": "blocked", "grade": "checked"}
```

The kernel projects the world as of that scene — *certain ∩ known-by-speaker ∩
present* — verifies each premise is available in that projection, runs the
steps, and does the linear accounting. The expected-verdict field cuts both
ways: a scene built on a character's flawed argument declares
`"expect": "fails-at: L3"`, and a wrong argument that accidentally validates is
as much a bug as a right one that fails. The consumer renders **three columns**
for review — prose with cue lines highlighted, encoding, verdict — and the
author approves the encoding or corrects the cues; approved proofs commit with
the episode.

## 4. The write surface

At contract `0.5.0`, each accepted fat payload and the exact draft source used
to verify it are written as one versioned RFC 8785 JCS record beneath
`.dli-evidence/payloads/<document>/<sha256>.json`. Retaining the source is
required: `dli.check` reloads every record and replays anchoring, staleness, and
the full payload battery after a restart or corpus change. Attestation sweeps
append JCS batches beneath
`.dli-evidence/attestations/<document>/batches/<sha256>.json`; batches form one
verified, unbranched parent chain while each question retains its supersession
history. A same-version retry of any committed batch returns that batch's
original digest, even after later appends. Protocol migrations do not
reinterpret an old request under a new wire version; durable readers
nevertheless recognize every committed batch-version-1 artifact and replay
legacy answers as current or re-attestation-required. Writers take a
project-wide advisory lock and publish immutable files from sibling temporaries
with file sync, atomic rename, and parent-directory sync for every newly created
path component. Reads verify canonical bytes, digest filenames, and chain
integrity before exposing evidence; payload submission verifies every prior
payload in that document's store before publishing another.

Attestation staleness fingerprints cover the saturated subject relation's atom
text, derived standing, grade, and resolved support/defeater conditions. A rule,
support, or ratification change therefore queues re-attestation even when a
source atom survives.

Rejected payloads and attestation records publish no evidence file. RQ drafts
are returned **as data** in the response (§3.1), and the worker never files
them. Simulation specifications, checkpoint-chain records, terminal records,
and final manifests use the same immutable publication path. The project lock
covers canonical bytes, content addresses, chain shape, terminal anchors,
accumulator shape, and publication. Replay of the selected immutable head and
all trial computation run outside it; publication reacquires the lock and
compares the observed head before appending.

The worker never writes canon, and it never writes a ruling-request file
directly. An open question surfaced by the worker is a debt the consumer files
and the author rules (koan 5).

The target write surface remains content-addressed evidence, derived cache
entries, and run manifests only. Canon and ruling-request files remain outside
the worker's authority.

## 5. Concurrency and jobs

Query and check ops are synchronous and cheap. At contract `0.5.0`,
`dli.simulate.start` durably publishes pending state before its response is
flushed; only then does the transport release a background worker.
`dli.simulate.status` verifies durable state and schedules an unfinished job
after restart, while `dli.simulate.cancel` commits terminal cancellation at a
checkpoint boundary. A worker process runs at most four simulation threads;
additional jobs remain queued until a later transport turn releases capacity.
The consumer still owns cross-process queue policy, retries, polling, and UI.

## 6. Postgres posture

There is no database in the serving path. When a dashboard eventually demands
paged, indexed queries a JSON worker is awkward for, a Postgres mirror may be
added — but only as a **read-only projection rebuilt from `.dli-cache/`
snapshots**, written by nothing but the mirror job, and **never a source of
truth**. The cache is derived from canon; the mirror is derived from the cache;
authority stays in the ledger.
