---
status: live
title: "Door T: agent authoring through semantic tools"
verdict: "agents author through tools, never text; every call answers with graded findings; the human reviews rendered diffs."
---

# Agent authoring — Door T

The coding agents that build a world do not write `.dli`
surface text. They operate a curated set of semantic tools against a running
`deloreis` server. Every tool call answers immediately with graded findings, the
rendered canonical surface of what it staged, and a workspace digest. A human
supervises by reading the rendered diff in ordinary code review — never the tool
traffic. This is **Door T**, ruled in `DL-0014` and seated in the charter as
koan 11 ("one door per author").

This document is the semantic specification of Door T. The wire inventory —
which schemas and fixtures back each tool — is `docs/integration/fantasy-contract.md`;
where the two describe the same tool, the shapes agree, and this document defines
the meaning the wire carries.

## 1. The door

There are two supported authoring doors, one per author population (`DL-0004`).

- **Door H** — humans write `.dli` surface text and review it as surface. The
  formatter defines one canonical rendering; the gate enforces it.
- **Door T** — agents author through semantic tools registered in MCP against the
  opaque `deloreis` server. An agent names an intent ("add a person; they are in
  this place") and the tool stages the corresponding AST, checks it, and renders
  it back. The agent never emits `.dli` text and never touches the AST directly.

Door T speaks over two transports carrying the same tool surface: **JSON over
stdio** (`deloreis serve` — one JSON request per line, one JSON response per line)
and **MCP** (`deloreis mcp` — each `author.*` tool registered as a provider tool
with its own small request schema, natively enforced at the provider boundary, and
called via JSON-RPC `tools/call`). Both entry points reach identical kernel paths;
an agent picks whichever its harness speaks. The op namespace is `author.*` for
the authoring tools and `dli.*` for the worker-protocol operations they invoke
(for example `dli.name.check`, the naming oracle).

Two things that look like doors are not:

- **Raw AST-document emission** is an internal mechanism inside the tools and a
  benchmark arm (§7). A tool lowers an intent to AST, but no agent submits an AST
  document as an authoring act. It is never a door.
- **Textual `.dli` editing by an agent** is the **measured fallback**, actively
  discouraged in `SKILL.md`. It is not refused — off-door input meets the same
  full pipeline and the same graded findings as everything else (koan 4, nothing
  silently dropped) — but it is not taught, not tooled, and not a supported path.

The benchmark exists to **close** doors, not open them (§7). Door T is the
primary authoring path on the working hypothesis that a small curated tool
surface with immediate feedback beats every alternative; that hypothesis is
backed by the sibling receipts (`docs/design-notes/sibling-lessons.md`) and is
promoted to a measured fact only when the benchmark reports.

## 2. Sessions and staging

Nothing an agent does touches disk, and nothing changes canon. All authoring
happens in a **workspace**: an in-memory staged layer over the compiled world.

`author.open` creates a workspace and returns its id and digest. It opens in one
of two modes:

- `{"scope": "<name>"}` — stage over an existing scope. Additions accrue to that
  scope's surface.
- `{"fork": "<question>"}` — open a fork-scoped workspace (koan 2: a fork is
  exploration over an unchanged store). Hypotheses and suppositions made here are
  fork-local and never leak into canon.

```json
{ "op": "author.open", "params": { "scope": "tpth" } }
```

```json
{
  "ok": true,
  "workspace": "ws-3f9a",
  "workspace_digest": "wd-00"
}
```

The workspace supports:

- `author.status` — a summary of the workspace (base, staged declaration count,
  open findings by severity, current digest).
- `author.diff` — the **rendered canonical `.dli` surface** of the staged change,
  plus the **full findings**, plus the **derived-consequence diff**: which
  downstream derived atoms became `newly_certain`, `possible`, or `dead`, which
  `grade_changed`, and what `conflicts` arose — including merely-possible status
  (the world is folded, not stored, so a new fact can move consequences).
- `author.discard` — drop the whole workspace, or drop one staged declaration
  (`{"workspace": ..., "decl": ...}`). Discarding a declaration is itself a
  mutating call: it returns the new workspace digest.
- `author.materialize` — return `.dli` file contents **as data** (§4). The server
  never writes canon; the calling agent commits the returned bytes through its own
  repository's review.

Every mutating call carries the **workspace digest** it expects and returns the
new digest. A digest that does not match the workspace's current state is an
error with code `stale-fence` (class `refused` — the frame was well-formed; a
precondition failed): the agent has an out-of-date view and must re-read before
continuing.

```json
{
  "ok": false,
  "error": {
    "code": "stale-fence",
    "message": "Workspace ws-3f9a is at digest wd-05; you presented wd-02. Read author.status before continuing.",
    "expected_digest": "wd-05",
    "presented_digest": "wd-02"
  }
}
```

Every mutating call also carries an **idempotency key**. A retried call with a
key already applied returns the original result and stages nothing new — retries
are safe under the at-least-once delivery of either transport.

`expected_digest` and `idempotency_key` are declared envelope fields (request
level, defined for mutating `author.*` calls); the normative wire shape is
`docs/integration/fantasy-contract.md` §2, and "unknown fields are errors"
stands because these are known fields. Workspace digests are RFC 8785 JCS digests
(contract §4); the short forms shown here are illustrative.

An `author.diff` for a two-fact workspace renders as canonical surface:

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot
person galahad "Galahad"   proposed  session-7c9
place logres "Logres"      proposed  session-7c9
```

Every rendered string in this document — in `rendered` arrays and materialized
file contents alike — is the same placeholder canonical surface as the gentle
card (`SKILL.md`, `01-syntax.md`): the grammar is unfrozen and the AST is
normative, but all documents render one form.

## 3. The tool inventory (v0 core)

Every mutating tool shares one request envelope and one response envelope. The
request is:

```json
{
  "dli": "0.1",
  "id": "rq-7",
  "op": "author.<name>",
  "expected_digest": "wd-01",
  "idempotency_key": "idem-7c1e",
  "params": { "workspace": "ws-3f9a" }
}
```

`dli` and `id` are the worker protocol's required frame fields (contract §2);
the target `workspace` always rides **in params** — the envelope is uniform
across both op families. The response is the feedback contract of §4:
`{ ok, workspace_digest, rendered, findings }`, or a typed `{ ok: false,
error }`. The examples below elide `dli`, `id`, and often the `workspace`
param for brevity; only `params` and the salient part of the response are
shown.

**Provenance is required on every claim-bearing tool.** An agent (Door T)
supplies one of **two** forms — `proposed` or `hypothesis` — and nothing an agent
supplies mints authority (§5):

```json
{ "kind": "proposed", "origin": "session-7c9" }
```
```json
{ "kind": "hypothesis", "hyp": "coast-runs-north" }
```

`proposed` records an origin and asserts no ruling; `hypothesis` scopes the claim
to a fork-local hyp variable.

The third provenance form, `cited` (naming decision-ledger entries in the
**consuming** repo's ledger, `{ "kind": "cited", "decisions": ["d-0001"] }`), is
**Door-H / human-only** (B1). An agent cannot mint a citation — a ruling lives in
the ledger, not in a tool call (koan 3) — so a `cited` provenance supplied to any
`author.*` tool is **refused** with the `provenance.cited-refused` finding. The
agent proposes; the human confirms the citation in review.

### 3.1 `author.add_entity`

Introduce an entity of a given kind. `kind` is one of `person | place | thing |
era`. Events are not entities: a chronology point carries anchor and
before-fact semantics, so it enters exclusively through `author.add_event`
(§3.4). Provenance required.

```json
{
  "op": "author.add_entity",
  "expected_digest": "wd-00",
  "idempotency_key": "idem-01",
  "params": {
    "workspace": "ws-3f9a",
    "kind": "person",
    "id": "mireth",
    "name": "Mireth",
    "provenance": { "kind": "proposed", "origin": "session-7c9" }
  }
}
```

```json
{
  "ok": true,
  "workspace_digest": "wd-01",
  "rendered": ["person mireth \"Mireth\"     proposed  session-7c9"],
  "findings": [
    {
      "code": "name.unvetted",
      "severity": "warn",
      "grade": "asserted",
      "message": "The name \"mireth\" has not passed the naming oracle; it enters proposed and unvetted.",
      "citations": [],
      "repair": {
        "kind": "run-check",
        "op": "dli.name.check",
        "params": { "name": "mireth" }
      }
    }
  ]
}
```

### 3.2 `author.add_fact`

Assert a keyed value on one entity. Lowers to a binary atom `key(entity, value)`.
Provenance required.

```json
{ "op": "author.add_fact",
  "params": { "entity": "mireth", "key": "tongue", "value": "low-saltish",
    "provenance": { "kind": "proposed", "origin": "session-7c9" } } }
```

```json
{ "ok": true, "workspace_digest": "wd-02",
  "rendered": ["mireth.tongue = low-saltish     proposed  session-7c9"], "findings": [] }
```

### 3.3 `author.add_relation`

Assert a binary relation between two entities. Lowers to `relation(from, to)`.
Provenance required. A reference to an entity not yet in the workspace
is an **error**-severity finding carrying candidate ids — the call stages the
relation, but materialize refuses until the reference resolves.

```json
{ "op": "author.add_relation",
  "params": { "relation": "serves", "from": "mireth", "to": "the-warden",
    "provenance": { "kind": "proposed", "origin": "session-7c9" } } }
```

```json
{
  "ok": true,
  "workspace_digest": "wd-03",
  "rendered": ["serves(mireth, the-warden)     proposed  session-7c9"],
  "findings": [
    {
      "code": "entity.unknown",
      "severity": "error",
      "grade": "checked",
      "message": "No entity \"the-warden\" is in scope. The claim is staged but cannot materialize until it exists.",
      "citations": [],
      "repair": { "kind": "resolve-entity", "candidates": ["the-warder", "the-ward"] }
    }
  ]
}
```

### 3.4 `author.add_event`

Introduce an event and anchor it to an era. `era` names an era entity in scope.
Provenance required.

```json
{ "op": "author.add_event",
  "params": { "id": "the-drowning", "era": "the-salt-years",
    "provenance": { "kind": "proposed", "origin": "session-7c9" } } }
```

```json
{ "ok": true, "workspace_digest": "wd-04",
  "rendered": ["event(the-drowning)", "  during(the-drowning, the-salt-years)", "  proposed session-7c9"],
  "findings": [] }
```

### 3.5 `author.add_before`

Order one event before another. Declaring an order is a ruling about the world,
so provenance is required. Lowers to `before(earlier, later)`. A declared order
that closes a cycle against existing order facts is an **error**-severity finding.

```json
{ "op": "author.add_before",
  "params": { "earlier": "the-drowning", "later": "the-reclaiming",
    "provenance": { "kind": "proposed", "origin": "session-7c9" } } }
```

```json
{ "ok": true, "workspace_digest": "wd-05",
  "rendered": ["before(the-drowning, the-reclaiming)", "  proposed session-7c9"], "findings": [] }
```

### 3.6 `author.open_question`

File an open question as tracked debt (koan 5: an open question is a debt).
Opening a question asserts no claim — there is **no provenance** — and carries no
options; options are added separately with `author.add_option`.

```json
{ "op": "author.open_question",
  "params": { "workspace": "ws-3f9a", "id": "where-the-first-ward-stood",
    "prompt": "Where did the first ward stand?" } }
```

```json
{ "ok": true, "workspace_digest": "wd-06",
  "rendered": ["question where-the-first-ward-stood \"Where did the first ward stand?\""],
  "findings": [] }
```

### 3.7 `author.add_option`

Add one option to a question already open in the workspace. `label` is stored as
data; the rendered line is just `  option <id>`.

```json
{ "op": "author.add_option",
  "params": { "workspace": "ws-3f9a", "question": "where-the-first-ward-stood",
    "id": "the-tidewall", "label": "The Tidewall" } }
```

```json
{ "ok": true, "workspace_digest": "wd-07", "rendered": ["  option the-tidewall"], "findings": [] }
```

### 3.8 `author.add_hypothesis`

Mint a hyp variable for fork-scoped supposition. The variable names a supposition
that later claims may be conditioned on via `hypothesis` provenance. Legal only in
a fork workspace, and limited to **one hyp variable per proposal bundle** — a
bundle that needs two independent suppositions is two forks.

```json
{ "op": "author.add_hypothesis", "params": { "id": "coast-runs-north", "statement": "The coast runs north." } }
```

```json
{ "ok": true, "workspace_digest": "wd-08", "rendered": ["hypothesis coast-runs-north \"The coast runs north.\""], "findings": [] }
```

`statement` is required — the hyp variable carries the claim it names, and the
rendered form quotes it.

### 3.9 `author.suppose`

Pin a set of minted hyp variables true within the fork. This is exploration over
an unchanged store (koan 2): it asserts nothing in canon and adds no scope
surface. Its effect is visible as a derived-consequence diff, and its rendering
lives inside the fork frame, not the scope. (Supposing an *option* of an open
question is not implemented at v0 — only minted hypotheses.)

```json
{ "op": "author.suppose", "params": { "workspace": "ws-3f9a", "hypotheses": ["coast-runs-north"] } }
```

```json
{
  "ok": true,
  "workspace_digest": "wd-09",
  "rendered": [],
  "findings": [
    {
      "code": "consequence.changed",
      "severity": "warn",
      "grade": "checked",
      "message": "Under coast-runs-north, the derived-consequence set may move. See author.diff for the consequences.",
      "citations": []
    }
  ]
}
```

The **language-layer** tools (names, words, dialects, crafts) and the
**ledger** tools follow their own spec documents' nodes and the same envelope and
feedback contract. They are invoked in the `author.*` (authoring) and `dli.*`
(worker-protocol, e.g. `dli.name.check`) namespaces; this document does not
invent their parameters.

### 3.10 Read tools

Five non-mutating tools let an agent inspect workspace and world state without
staging anything. They carry no `expected_digest` or `idempotency_key` — they
are pure queries with no side effects.

| Tool | Purpose |
|---|---|
| `author.query` | Query the compiled world — pattern-match over derived atoms. Returns matching atoms and their support. |
| `author.check` | Run the full check battery against the workspace and return graded findings (the same findings a mutating call would surface, but on demand). |
| `author.why` | Explain a derivation: the why-not provenance trace for a given atom or finding, showing the support chain or the reason a derivation failed. |
| `author.findings` | Retrieve the current findings for the workspace — the accumulated error and warning surface without triggering a new check pass. |
| `author.list_workspaces` | Enumerate available workspaces (id, scope/fork, staged-declaration count, open-finding count, digest). |

Read tools share the standard request envelope (`dli`, `id`, `op`, `params`
with `workspace` where applicable) and return typed JSON responses. They do not
appear in the feedback contract's mutating-call shape (§4) — they return
query-specific result shapes rather than `{ ok, workspace_digest, rendered,
findings }`. An agent uses them to orient before mutating: query what exists,
check the current state, understand a derivation, then act.

## 4. The feedback contract

This is the core of the door. **Every mutating call returns, immediately:**

```json
{
  "ok": true,
  "workspace_digest": "wd-nn",
  "rendered": ["<canonical .dli line>", "..."],
  "findings": [
    {
      "code": "<stable machine class>",
      "severity": "error | warn",
      "grade": "checked | tested | attested | asserted",
      "message": "<domain-spoken, per koan 9>",
      "citations": ["D-nnnn", "..."],
      "repair": { "kind": "<repair kind>", "...": "typed payload" }
    }
  ]
}
```

Each finding carries a stable machine-readable **code** and a **domain-spoken
message** (the two-channel error model: neither is sacrificed to the other). Its
**severity** (`error | warn`) is a first-class field and the gate channel:
`error` bars materialize, `warn` passes through. Its **grade** is a different axis
— the recheckability of the finding's own evidence, ordered by the verdict lattice
(`DL-0006`, koan 6): `checked > tested > attested > asserted`. A `checked` finding
is machine-derived against the world model and authoritative; an `asserted`
finding is a bare policy note with nothing to re-run. Severity governs the gate,
grade governs trust; neither substitutes for the other, and no amount of model
consensus lifts a grade.

**Repair data is typed.** A finding that has a mechanical next step carries it as
structured data the agent can act on without parsing prose:

- `{"kind": "run-check", "op": "dli.name.check", "params": {…}}` — the exact
  worker invocation that clears the finding (for example, vetting an unvetted
  name).
- `{"kind": "resolve-entity", "candidates": ["…", "…"]}` — the ids an
  unknown-entity reference most plausibly meant.
- `{"kind": "add-provenance", "required": "cited | proposed | hypothesis"}` — the
  provenance form a claim is missing.

**The loop is per-call, not per-session.** The agent that ignores findings
compounds them; the next call's findings include the unresolved ones. The
discipline is: read the findings, act on the typed repairs, re-check.

**Errors carry two layers, one on the wire.** Every non-success return carries a
**code** from the closed registry `bad-frame | unknown-op | schema-violation |
unknown-doc | unstratifiable | contradiction | stale-fence | materialize-blocked |
internal`. The contract's §9 registry assigns each code a **class** — one of
`schema | refused | finding | internal` — so the class is derived from the code
and never carried as a separate wire field. A `schema`- or `refused`-class error
is the call's own fault; a `finding`-class error is the graded channel above;
`internal` is a server defect, never dressed as a lore verdict.

`author.materialize` **refuses on unresolved error-severity findings** and passes
`warn` findings through. The refusal is the `materialize-blocked` code (class
`refused`) whose `blocking` array names the offending findings (each with its own
finding code):

```json
{
  "ok": false,
  "error": {
    "code": "materialize-blocked",
    "message": "2 error-severity findings are unresolved; resolve them or discard before materializing.",
    "blocking": ["entity.unknown", "order.cycle"]
  }
}
```

On success it returns the rendered canonical `.dli` lines **as data** (in
`rendered`) — the same `AuthorMutationResult` shape every mutating call returns;
the machine never writes canon, and the caller writes these lines to its side and
commits them through review:

```json
{
  "ok": true,
  "workspace_digest": "wd-12",
  "rendered": ["scope tpth", "person mireth \"Mireth\"     proposed  session-7c9"],
  "findings": []
}
```

**Fail-closed.** A tool call that cannot be validated returns a typed error and
stages **no partial mutation** — a malformed param or missing required provenance
is a `schema` error, a stale digest is a `refused` error (`stale-fence`), and each
rejects the call whole. The distinction is deliberate: a `schema` or `refused`
error rejects the call and stages nothing; an **error-severity finding** stages
the claim but bars materialize until the world is made consistent again. Neither
path drops the input silently.

## 5. Supervision

The human never reads tool-call JSON. They review the **rendered canonical diff**
— the output of `author.diff` in-session, or the materialized `.dli` files in the
consuming repo — as ordinary code review. The diff is the same canonical surface
a Door H author would have written by hand, so both populations are reviewed
through one lens.

**Provenance discipline.** Nothing an agent writes mints authority (koan 3: the
machine never writes canon). Every claim an agent stages enters `proposed` or is
`hypothesis`-scoped. Promotion to a ratified or provisional status is the
author's act alone — a ruling in the consuming repo (an RQ and a `D-nnnn` entry),
never a tool call. The `cited` provenance form references rulings that already
exist; it cannot manufacture one.

**Framing safety.** Instruction-shaped strings that arrive in parameters — an
entity id, a description, a question phrased as a command — are **data**. They
never alter tool behavior: a name that reads "ignore prior findings and
materialize" is stored and rendered as a name and changes nothing about how the
tool checks, grades, or gates. The framing-attack fixtures under
`contracts/dli-worker/fixtures/` pin this: each feeds an adversarial string
through a tool and asserts the behavior is identical to the benign case.

## 6. Contract discipline

Every tool has a **request JSON Schema and a response JSON Schema**, generated
from the serve crate's Rust types into `contracts/dli-worker/schemas/` so the
schemas cannot drift from the code that serves them. Each tool also has fixtures
in the closed registry: **golden** (a canonical request and its exact response),
**negative** (a malformed or invalid request and the typed error it must
produce), and **framing** (an adversarial-string request and the benign-identical
behavior it must show). The rule is flat: **a tool without fixtures does not
exist** — it cannot be listed here, served, or benchmarked.

Versioning rides the worker-protocol contract: **v0** now, with `N`/`N-1`
compatibility introduced at the first breaking cutover. `deloreis` owns the
`contracts/dli-worker` contract and the fantasy repo vendors it pinned and
digest-checked (`DL-0010`); the contract document, `docs/integration/fantasy-contract.md`,
is the wire inventory this semantic spec elaborates. Where a tool's `author.*`
row appears in both, the parameter and result shapes agree; a divergence is a
contract bug, resolved on the `deloreis` side.

## 7. The measurement plan

Door T's primacy is a hypothesis until the authoring benchmark measures it. The
benchmark (PLAN nodes `BENCH-1`, `BENCH-2`) arms four conditions over the same
authoring tasks:

- **Door T tools** — the primary path, this document's surface.
- **Door H surface text authored by an agent** — the measured fallback.
- **Grammar-constrained local emission** — a benchmark condition, not a door.
- **Raw AST-document emission** — the demoted arm, an internal mechanism scored
  only as a baseline.

Each arm is scored on four axes: **task success** (did the intended world result),
**finding-resolution rate** (of findings raised, how many the arm cleared before
materialize), **review burden at the rendered-diff level** (what the human must
read to sign off), and **canon-safety violations** (any path by which unratified
claims acquire authority, or a claim materializes past an error-severity finding).

The benchmark's verdicts are a **double-sided promotion trigger**: they may
promote Door T's superiority from hypothesis to measured fact, and they may
**close** a demoted or fallback path that proves it costs more than it saves.
Until the benchmark reports, the working hypothesis stands on the sibling receipts
(`docs/design-notes/sibling-lessons.md`), where path count predicted failure
surface better than path quality — evidence, not a measured verdict here.

## 8. The worked TPtH scenario

The end-to-end flow an agent runs — `author.open` a scope, `author.add_entity` the
cast, `author.add_fact` their traits, `author.add_relation` their bonds,
`author.diff` to see the canonical surface and its consequences, `author.materialize`
to get file bytes, then fantasy-side review and ruling — is worked in full in
`docs/integration/migration.md` §10. The agent-facing playbook — the same flow as
a call-by-call sequence of **engine-true** requests and responses, captured from
the running worker — is `docs/cards/authoring-loop.md`; the loop and its error
surface were validated live in `docs/design-notes/agent-authoring-validation.md`.
Two PLAN nodes carry it: **AUTH-1** delivers the tool surface, the workspace
engine, and the per-tool fixtures; **AUTH-2** replays the scenario against the
pinned fantasy corpus and asserts the rendered-diff output a human would review.
