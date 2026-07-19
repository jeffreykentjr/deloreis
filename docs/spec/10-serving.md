---
status: live
title: "Serving: one offline, deterministic worker surface"
verdict: "one JSON worker surface, offline and deterministic; every consumer gets the same ops the fantasy factory gets."
---

# Serving

`deloreis serve` is how a consuming application talks to the compiled world. It
is one JSON worker surface, offline and deterministic. The fantasy factory is
the first consumer, but nothing about the surface is factory-specific: every
consumer gets the same ops, the same envelope, and the same guarantees.

## 1. Process model

`deloreis serve` speaks **JSON over stdio**. An MCP transport wrapper exposes the
same operations to MCP clients without changing their semantics. There is **no
network, no model call, and no background service** inside the worker — a
determinism the gate depends on (DL-0013). The consuming application keeps its
own orchestration, its own UI, and all of its LLM work; the worker keeps all
evaluation and verdicts.

The MCP transport is a thin wrapper, not a second protocol: it registers the
same ops as MCP tools and forwards their JSON payloads unchanged, so an MCP
client and a stdio client exercise identical kernel paths and identical
fixtures. There is one worker surface; MCP is a way to reach it.

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

## 2. The envelope

Every request carries the operation, the contract version, a reference to the
corpus, and the operation's arguments:

```json
{"op": "dli.<name>", "contract_version": "1", "corpus": "<ref>", "args": {"…": "…"}}
```

Every response is either a result or a typed error, plus metadata:

```json
{
  "ok": true,
  "result": {"…": "…"},
  "meta": {
    "versions": {"contract": "1", "grammar": "…", "kernel": "…"},
    "corpus": {"commit": "…", "digest": "…"},
    "replay": "<judgment ref>"
  }
}
```

An error response carries `"ok": false` and a **typed error** — a stable machine
class plus a domain-spoken message (koan 9, errors are lore) — never a bare
string.

Two envelope properties are load-bearing:

- **Responses are timestamp-free by design.** A response is a pure function of
  the corpus digest, the contract version, and the arguments. Wall-clock time is
  not part of that function, so it is not in the response; **the response is the
  determinism domain.** Two runs against the same corpus digest produce
  byte-identical results, which is what makes the `replay` judgment reference
  meaningful and the gate reproducible.
- **Findings sort by stable keys.** Any list of findings, conflicts, or diffs is
  ordered by stable keys (atom identity, cue location, rule id), never by
  discovery order, so diffs between runs are meaningful.

The `versions` block is a **handshake**, not decoration. A consumer sends the
`contract_version` it was built against; the worker answers with the contract,
grammar, and kernel versions it is actually running. A consumer pinned to
contract 1 talking to a worker that only speaks contract 2 gets a typed
version-mismatch error, never a silently misinterpreted payload — the contract
is vendored pinned and digest-checked on the consuming side (DL-0010), so a
mismatch is a real deployment fact worth surfacing. Ops are **capability
negotiated**: the worker advertises which ops it implements, so a consumer can
degrade gracefully rather than call an op the running build does not yet wire.

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
| `dli.version` | Contract/grammar version handshake | `integration/fantasy-contract.md` |
| `dli.check` | Validate a corpus or pack; return findings | `02-semantics.md` |
| `dli.query` | Query the compiled world — status, reach, `--why`/blame | `02-semantics.md`, `08-reasoning.md` |
| `dli.fork.new` | Create a fork | `02-semantics.md` |
| `dli.fork.list` | Enumerate committed forks | `02-semantics.md` |
| `dli.fork.diff` | Derived-consequence diff of a fork against the base | `02-semantics.md` |
| `dli.fork.suppose` | Assume an option or event value in a fork | `02-semantics.md` |
| `dli.fork.add` | Add proposed declarations to a fork | `02-semantics.md` |
| `dli.fork.suppress` | Retract an assumption from a fork | `02-semantics.md` |
| `dli.fork.restrict` | Restrict a fork's valuation scope | `02-semantics.md` |
| `dli.fork.preflight` | Ratification pre-flight: candidate valuation + conflict scan | `02-semantics.md` |
| `dli.fork.promote` | Emit an RQ draft with generated affects-list (as data) | `02-semantics.md` |
| `dli.simulate.start` | Start a checkpointed practice-simulation job | `04-practices.md` |
| `dli.simulate.status` | Poll a job; a completed job's response carries the final manifest and verdicts | `04-practices.md` |
| `dli.simulate.cancel` | Cancel a running job | — |
| `dli.check-ledger` | Check a diegetic proof ledger against the scene projection | `04-practices.md` |
| `dli.check-draft` | Run the merged payload battery over a draft | `06-craft-and-payloads.md` |
| `dli.timeline.*` | `show`, `order`, `knows`, `episodes`, `phases` | `03-time.md` |
| `dli.export.run` | Compile a target profile from the certain layer | `09-exports.md` |
| `dli.name.check` | Vet a name — the kernel, world-true half | `05-language-layer.md` |
| `dli.attest.missing` | List open attestation questions checks demand | `07-verdicts-and-llm-boundary.md` |
| `dli.attestation-sweep` | Validate and record a sweep's attestation results | `07-verdicts-and-llm-boundary.md` |
| `dli.payload-submit` | Submit a `payload/v1` document for checking and storage | `06-craft-and-payloads.md` |

`dli.suppose` is a **killed form**, superseded by `dli.fork.suppose`; the server
refuses it. `deloreis report --min-grade` is a CLI rendering over `dli.check`
and `dli.query` results — it is not a worker op at v0.

Three worked scenarios ground the description below. All ride the JSON-Schema
worker protocol; the consumer validates each payload against the op schema
before sending.

### 3.1 Possible-worlds browse, edit, propose

The consumer lists forks, opens one, and requests the derived-consequence diff:

```json
→ {"op": "dli.fork.list"}
← {"forks": [{"name": "ep7-planning", "base": "3e1320a",
    "assumes": ["RQ-031=b", "P-214"], "local_atoms": 40,
    "created_by": "campaign-077", "updated": "2026-07-14"}]}

→ {"op": "dli.fork.diff", "fork": "ep7-planning"}
← {"newly_certain": [],
   "newly_possible": [
     {"atom": "reachable(gwern, glass-weald)",
      "support": [{"cites": ["D-0119"], "assumes": ["RQ-031(b)", "P-214"]}],
      "rendered": "possible — D-0119, assuming RQ-031(b) and P-214"}],
   "newly_dead": [], "grade_changed": [], "conflicts": []}
```

The `rendered` string is the kernel's own deterministic citation rendering; a
consumer may additionally gloss a paragraph for a sidebar, clearly labeled as
gloss, but the structured verdict is the only thing downstream logic reads. The
consumer supposes an option, adds a proposed fact, and — when a conflict names
the exact pair of sources — resolves or accepts, then proposes for ratification:

```json
→ {"op": "dli.fork.promote", "fork": "ep7-planning"}
← {"rq_draft": {"slug": "hedge-road-and-weald",
    "ask": "Adopt RQ-031(b) + the P-214 Glass Weald bundle?",
    "affects": ["canon/geography/regions.md#The Glass Weald", "…"],
    "diff": {"…": "the full derived-consequence diff"},
    "evidence": ["campaign-077"]}}
```

The kernel does evaluation, diff, blame, draft content, and the generated
affects-list. The consumer does rendering, glossing, file placement, and the
review queue. **Canon is untouched until the author rules** (koan 3).

### 3.2 Turning simulation jobs

A designer defines two candidate rule-deltas as data files; the consumer
enqueues one job per candidate:

```json
→ {"op": "dli.simulate.start", "ruleset": "turning@main",
   "delta": "lore/variants/salt-fringe.dli", "games": 100000,
   "seed": 41, "checkpoint_every": 10000,
   "metrics": ["game_length", "debt_spread", "forced_loss_rate"]}
← {"job_id": "sim-0042", "status": "running"}
```

The consumer polls `dli.simulate.status`; checkpoints mean a killed job resumes
rather than restarts. On completion the manifest and verdicts carry **honest
grades**:

```json
← {"job_id": "sim-0042", "games": 100000, "seed": 41,
   "manifest": "lab/campaign-081/runs/sim-0042.manifest.json",
   "invariants": {
     "debt_conservation": {"verdict": "checked"},
     "no-forced-loss-from-opening": {"verdict": "tested",
       "violations": 0, "note": "100000 games, 0 hits"}},
   "metrics": {"game_length": {"mean": 34.2, "p50": 33, "p95": 51},
     "forced_loss_rate": 0.031, "debt_spread": {"mean": 2.4, "max": 7}}}
```

Debt conservation is `checked` (exact bookkeeping over every game); the
no-forced-loss claim is `tested` (Monte Carlo evidence, never laundered into a
proof). The grade lattice (koan 6; see `07-verdicts-and-llm-boundary.md`) does
its job in the payload.

### 3.3 Diegetic proof checking

An episode draft carries premise and conclusion cues in its production metadata.
The consumer submits the formal encoding of the cues:

```json
→ {"op": "dli.check_ledger", "scene": "e01-s04", "speaker": "protagonist",
   "ledger": {"premises": ["…"], "steps": ["…"], "spends": ["p3"],
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
author approves the encoding or corrects the cues; approved ledgers commit with
the episode.

## 4. The write surface

The worker's write surface is exactly three things, and nothing else:

- **The derived-world cache** (`.dli-cache/`) — compiled world snapshots,
  content-hash keyed, gitignored, never a source of anything.
- **Run manifests** — written under a caller-designated output directory (sim
  manifests: spec hash, seed, counts, verdicts).
- **RQ drafts** — returned **as data** in the response (§3.1). The worker never
  files them; the calling consumer writes the ruling-request file under its own
  docs contract and its own review.

The worker never writes canon, and it never writes a ruling-request file
directly. An open question surfaced by the worker is a debt the consumer files
and the author rules (koan 5).

Because responses are a pure function of the corpus digest and the arguments
(§2), the cache is **content-hash keyed** and safe to share: two consumers
asking the same question of the same corpus state get the same cached compiled
world, and a corpus that has not changed does not recompile. Determinism is what
makes the cache correct rather than a source of staleness — there is no
timestamp to invalidate against, only the digest.

## 5. Concurrency and jobs

Query and check ops are synchronous and cheap. `dli.simulate.*` jobs are
**checkpointed and resumable**: a killed job resumes from its last checkpoint
rather than restarting. Long jobs run detached, and **orchestration is the
consumer's** — the worker exposes start/status/result, and the consumer owns the
queue, the progress UI, and the retry policy. The worker holds no job state
across process lifetime beyond what the checkpoint files record.

## 6. Postgres posture

There is no database in the serving path. When a dashboard eventually demands
paged, indexed queries a JSON worker is awkward for, a Postgres mirror may be
added — but only as a **read-only projection rebuilt from `.dli-cache/`
snapshots**, written by nothing but the mirror job, and **never a source of
truth**. The cache is derived from canon; the mirror is derived from the cache;
authority stays in the ledger.
