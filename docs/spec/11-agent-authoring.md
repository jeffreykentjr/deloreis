---
status: live
title: "the tool surface: agent authoring through semantic tools"
verdict: "agents author through tools, never text; every call answers with graded findings and structured data; supervision reviews consumer-rendered structured diffs, with .dli an opt-in view."
---

# Agent authoring — the tool surface

Agents that build a declared corpus do not write `.dli` surface text. They
operate a curated set of semantic tools against a running `deloreis` server.
Every tool call answers immediately with graded findings, structured fact data,
and a workspace digest; `.dli` rendering is opt-in. A consuming application
supervises by rendering the structured diff for its reviewers. This is **the
tool surface**, ruled in `DL-0014` and seated in the charter as koan 11 ("one
way in").

This document is the semantic specification of the tool surface. The wire inventory —
which schemas and fixtures back each tool — is `docs/integration/fantasy-contract.md`;
where the two describe the same tool, the shapes agree, and this document defines
the meaning the wire carries.

## 1. The interface

There is one supported authoring interface (`DL-0014`, `DL-0043`): the
semantic tools against the opaque `deloreis` server. An author names an intent
("add a person; they are in this place") and the tool stages the corresponding
AST, checks it, and renders it back. No author emits `.dli` text as the
supported path, and none touches the AST directly. Attribution is data, not a
channel distinction: callers declare the provenance and decision state their
application permits, while DLI checks schema and cited-decision integrity. The
application, not DLI, decides who may authorize that declaration.

*Historical note.* `DL-0004` originally defined two doors — a manual-text path (humans
writing `.dli` surface) and Door T (these tools). `DL-0041` demoted the text
surface to an opt-in rendering; `DL-0043` retired the door semantic entirely.
`.dli` text survives as a rendering and as the measured fallback below.

The interface speaks over two transports carrying the same tools: **JSON over
stdio** (`deloreis serve --project <path>` — one JSON request per line, one JSON
response per line) and **MCP** (`deloreis mcp --project <path>` — each
`author.*` tool registered as a provider tool with its own small request schema,
natively enforced at the provider boundary, and called via JSON-RPC
`tools/call`). Both entry points load the same strict `deloreis.toml` project and
reach identical kernel paths; an agent picks whichever its harness speaks. The
op namespace is `author.*` for the authoring tools and `dli.*` for the
worker-protocol operations they invoke (for example `dli.name.check`, the naming
oracle).

Two things that look like second interfaces are not:

- **Raw AST-document emission** is an internal mechanism inside the tools and a
  benchmark arm (§7). A tool lowers an intent to AST, but no agent submits an AST
  document as an authoring act. It is never an interface.
- **Textual `.dli` editing by an agent** is the **measured fallback**, actively
  discouraged in `SKILL.md`. It is not refused — off-interface input meets the same
  full pipeline and the same graded findings as everything else (koan 4, nothing
  silently dropped) — but it is not taught, not tooled, and not a supported path.

The tool surface is the primary authoring path on the working hypothesis that
a small curated tool surface with immediate feedback beats every alternative;
that hypothesis is backed by the sibling receipts
(`docs/design-notes/sibling-lessons.md`). The benchmark that would have
measured the alternative is retired with the two-door semantic (DL-0041,
DL-0043).

## 2. Sessions and staging

All authoring happens through a **workspace**: an in-memory staged layer over
the compiled world with a pinned lorestore commit and ref. Add calls may write
immutable fact objects, but they do not advance the serving ref.
`author.materialize` commits the accepted staged delta; Contract 1.5 standing
operations commit metadata revisions and valuation changes directly. The
workspace digest remains the fence over staged declarations and suppositions;
durable commit identity travels separately as `root_hash`.

`author.open` creates a workspace and returns its id and digest. It opens in one
of two modes:

- `{"scope": "<name>"}` — stage over a logical document declared by the loaded
  project. Additions accrue to that document's surface.
- `{"fork": "<question>"}` — open a fork-scoped workspace over the one project
  document that declares that question (koan 2: a fork is exploration over an
  unchanged store). Hypotheses and suppositions made here are fork-local and
  never leak into canon.

Unknown scopes, unknown forks, and fork ids declared by more than one document
are refused. Once opened, a workspace remains bound to that one document base
for mutation findings, read tools, staged recompilation, and materialization; it
cannot observe another document or the bundled demo world.

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

- `author.status` — the rendered staged declarations, full findings, and current
  workspace digest.
- `author.diff` — the **rendered canonical `.dli` surface** of the staged change,
  plus the **full findings**, plus the **derived-consequence diff**: which
  downstream derived atoms became `newly_certain`, `possible`, or `dead`, which
  `grade_changed`, and what `conflicts` arose — including merely-possible status
  (the world is folded, not stored, so a new fact can move consequences).
- `author.discard` — drop the whole workspace, or drop one staged declaration
  (`{"workspace": ..., "decl": ...}`). Discarding a declaration is itself a
  mutating call: it returns the new workspace digest.
- `author.materialize` — commit the workspace's staged delta to the lorestore and
  return the committed fact objects **as data**, with `.dli` rendering the
  `rendered: true` opt-in (§4; `decisions.md` DL-0046). It is a mutation, and it
  records the accept/reject of the staged DLI mutation — `author.discard` is the
  reject. The server never writes the consuming repository; what crosses the
  boundary is JSON, which the caller takes through its own review. A `0.5.1`
  caller instead receives the pre-`0.6.0` return, the rendered lines as data.
  From contract `1.9.0` an optional `decision` accepts the committed **facts**
  under a named, recorded, defeasible decision so they read `certain`. The
  conferral is **facts-only**: carriage claims — rules, `same-as` links, and
  inertial declarations — are never stamped and stay `possible`; their standing
  comes from their embedded declaration provenance, so they reach certainty
  through `author.record_decision` plus a **cited declaration**, not
  `author.materialize {decision}`.

A workspace mutation after `author.open` may carry the **workspace digest** it
expects and returns the new digest. (`author.open` creates the first digest, so
there is no prior fence to present.) When supplied, a digest that does not match
the workspace's current state is an error with code `stale-fence` (class
`refused` — the frame was well-formed; a precondition failed): the agent has an
out-of-date view and must re-read before continuing.

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

A workspace mutation after `author.open` may also carry an **idempotency key**.
When supplied, a retried call with a key already applied returns the original
result and stages nothing new — including a retry of a whole-workspace discard.
Callers that need retry safety under at-least-once delivery supply the key. Live
workspaces retain their applied-key cache for their lifetime. After a whole
workspace discard, the worker retains compact tombstones for the 1,024 most
recently discarded workspaces; an older retry is refused as `unknown-doc` and
can never recreate or mutate the discarded workspace.

`expected_digest` and `idempotency_key` are declared envelope fields (request
level, defined for mutating `author.*` calls); the normative wire shape is
`docs/integration/fantasy-contract.md` §2, and "unknown fields are errors"
stands because these are known fields. Workspace digests are RFC 8785 JCS digests
(contract §4); the short forms shown here are illustrative.

On MCP, both fields are exposed as reserved top-level tool arguments beside the
tool's semantic parameters. The adapter removes those two keys from `params` and
lifts them into the shared worker envelope before dispatch, preserving the same
stale-fence and idempotency behavior as native stdio.

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
  "dli": "1.6.0",
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

**Provenance is required on every claim-bearing tool.** A caller supplies
`proposed`, `hypothesis`, or, on the authority-scrubbed wires, `cited`:

```json
{ "kind": "proposed", "origin": "session-7c9" }
```
```json
{ "kind": "hypothesis", "hyp": "coast-runs-north" }
```

`proposed` records an origin and asserts no ruling; `hypothesis` scopes the claim
to a fork-local hyp variable.

At `1.3.0` and above, `cited` names declared decisions in the consuming
application’s ledger and is recorded as caller-supplied data. At `1.5.0`, it
also requires `state: "provisional" | "ratified"`. An unknown decision raises
`citation.unknown-decision` and bars materialization; DLI checks referential
integrity but does not decide who may rule. Wires below `1.3.0` retain the
released legacy cited-provenance refusal behavior.

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
Provenance required. A compact token renders as a symbol; whitespace or surface
delimiter characters make the value a quoted string, so arbitrary caller data
always materializes as parseable canonical text.

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
  "rendered": ["event the-drowning during the-salt-years proposed session-7c9"],
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
data in the AST and canonical surface.

```json
{ "op": "author.add_option",
  "params": { "workspace": "ws-3f9a", "question": "where-the-first-ward-stood",
    "id": "the-tidewall", "label": "The Tidewall" } }
```

```json
{ "ok": true, "workspace_digest": "wd-07",
  "rendered": ["  option the-tidewall \"The Tidewall\""], "findings": [] }
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

### 3.10 Standing transitions

Contract `1.5.0` adds two durable operations over stored claims:

- `author.set_standing {workspace, claim_id, decision}` promotes a provisional
  claim to ratified only when `decision` is declared and occurs in every
  support path. The object records `standing_decision`; its claim identity is
  unchanged.
- `author.defeat {workspace, claim_id, decision, defeated_decision}` records
  the new defeating decision, verifies that `defeated_decision` supports every
  path, and rules that old supporting decision false on the serving ref. The
  object records `defeated_by`, remains reachable, and evaluates dead.

Both return the new lorestore `root_hash` and the unchanged staged-state
`workspace_digest`. Canon export excludes dead claims and refuses
`evidence-corrupt` when the commit decision list differs from stored `Ratify`
carriage objects.

### 3.11 Constraint declarations

Contract `1.6.0` adds one declaration over predicates:

- `author.declare_functional {workspace, pred, provenance}` declares `pred` a
  functional dependency — each subject holds at most one certain value for it,
  so two distinct certain values raise `continuity.fd-violation` (DL-0073).

It stages like any other add and commits with `author.materialize`. Undeclared,
the checker infers single-valuedness from canon's precedent, which cannot judge a
predicate's *first* use; whether a relation admits one value or many is a ruling
about the world, not one the engine makes (koan 3), so the caller declares it.
The declaration lowers to no atom — there is no fact to query — and rides, with
its provenance, on the stored declaration (DL-0074).

### 3.12 Coreference declarations

Contract `1.7.0` adds one declaration merging identity:

- `author.assert_same_as {workspace, left, right, provenance}` asserts that
  `left` and `right` name the same bearer. The evaluable world closes the
  merge symmetric-transitively over every predicate the two ids appear in
  (DL-0078); the merge's standing is carried entirely by this claim's own
  provenance — defeating the citing decision (`author.defeat`) un-merges
  every derived consequence on the next read, with no re-fixpoint.

Three findings are stage-time, not deferred to check: `identity.self-merge`
(warn) when `left` and `right` are the same id — a harmless no-op merge,
staged anyway; `identity.temporal-unsupported` (error) when either endpoint
is a declared era or event — identity over the chronology spine is refused
in v1, regardless of the claim's own standing (DL-0078); and
`identity.sort-mismatch` (warn) when the two endpoints' declared kinds
differ. A merge that would create a duplicate certain value for a
functionally-declared predicate (§3.11) surfaces the existing
`continuity.fd-violation` on `author.check` and blocks `author.materialize`
the same as it would for any other write — no separate mechanism.

### 3.13 Rule authoring

Contract `1.7.0` adds one declaration over derivation rules:

- `author.add_rule {workspace, name, head, body, provenance}` declares one
  datalog derivation rule — a typed `head :- body` clause. `head` is the
  already-typed `goal_atom` shape `author.query` shares; `body` is a list of
  body literals (`{atom, negated}`), a positive atom or a `not`-negated one.
  One clause per call: an N-clause rule is N `author.add_rule` calls under
  distinct names — equivalent to one N-clause rule, and finer-grained for
  `author.defeat`, which cites a rule by name. Every fact the rule derives
  carries this claim's own provenance (`fire_rules` multiplies a rule's
  annotation into every derivation it fires): defeating the rule's citing
  decision kills every derivation through it, with no re-fixpoint, exactly
  as defeating any other claim's citation does.

  **Stricter than every other `author.add_*` tool.** An error-severity
  finding refuses the whole add and stages nothing — not the usual
  stage-then-block-materialize shape — because a bad rule staged even once
  can BRICK every later read: the staged world recompiles on every call, and
  an unstratifiable or range-unsafe rule set makes that recompile fail
  outright. Checked before staging: `rule.unbound-variable` (a head or
  negated-literal variable no positive body atom binds — datalog range
  restriction); `rule.duplicate` (the name is already staged or already in
  the compiled base); and, only once those pass, a stratification
  pre-flight — a tentative compile of the candidate rule alongside
  everything already staged — refusing with `rule.unstratifiable` on
  failure. The ordinary per-declaration census still applies
  (`field.control-char`, `citation.unknown-decision`).

  A functional-dependency break a rule's own derivation creates — visible
  only once the world is saturated, not in the pre-saturation authored
  layer `continuity.fd-violation` reads — surfaces as
  `continuity.derived-fd-violation` on `author.check` and blocks
  `author.materialize` the same way. No separate wire mechanism: it is the
  same check battery, reading a second, saturated lens.

### 3.14 Inertial persistence declarations

Contract `1.8.0` adds one declaration over fluent predicates:

- `author.declare_inertial {workspace, pred, provenance}` declares `pred`
  inertial: a fluent value asserted `since t` with no authored `until` is
  meant to persist forward until a later, distinct, live value on the same
  slot defeats it. Mirrors `author.declare_functional` (§3.11) exactly — a
  carriage-only declaration, no ground atom, provenance riding the stored
  declaration.

  **The projection.** The op stages and materializes the declaration and
  carries it through every lowering (the compiled world, forks, merges, and
  the durable lorestore), and the `continuity` check reads it: an open
  inertial value's forward persistence is bounded at the next distinct certain
  value on the slot. This composes into three outcomes — an ordered-distinct
  pair is a **silent transition**; an undeclared order is a
  `continuity.fluent-order-undeclared` **warn** (with a settling before-fact
  repair); a certain co-hold (the same anchor, or authored overlap) is a
  `continuity.fluent-contradiction` **error**. An inertial value with no
  `since` anchor raises `continuity.inertia-unanchored` (warn). Capping is
  active only while both the declaration and the competing value are certain
  (`docs/decisions.md` DL-0081 records the full ruling).

### 3.15 Read tools

Six non-mutating tools let an agent inspect workspace and world state without
staging anything. They carry no `expected_digest` or `idempotency_key` — they
are pure queries with no side effects.

| Tool | Purpose |
|---|---|
| `author.query` | Query the compiled world — pattern-match over derived atoms. Returns matching atoms and their support. |
| `author.check` | Run the full check battery against the workspace and return graded findings (the same findings a mutating call would surface, but on demand). |
| `author.why` | Explain a derivation: the why-not provenance trace for a given atom or finding, showing the support chain or the reason a derivation failed. It also returns `supporting_decisions` (the recorded decisions a certain fact rests on), the auditability affordance. The atom may be given rendered (`atom`, parsed by the `.dli` front end) or as a typed `goal_atom` (the same shape `author.query` takes, `{"pred": …, "args": [{"Sym": …}, {"Var": …}]}`); the typed form is what makes `supporting_decisions` reachable in a syntax-free build. |
| `author.findings` | Retrieve the current findings for the workspace — the accumulated error and warning surface without triggering a new check pass. |
| `author.list_workspaces` | Enumerate available workspaces (id, scope/fork, staged-declaration count, digest). |
| `author.as_of {workspace, pred, subject, at}` | Contract `1.9.0`. Project the inertially-current fluent value(s) of `pred(subject)` at a query point — the one inertia-aware read (DL-0082): `check::slot_state_at` kernel-side, never `author.query`'s eval path. Returns each holding's projection, temporal cover, support, `since`/`until` (a capped inertial value's `until` is the DERIVED boundary), citations, and — when capped — the certain defeater that bounds it. |

Read tools share the standard request envelope (`dli`, `id`, `op`, `params`
with `workspace` where applicable) and return typed JSON responses. They do not
appear in the feedback contract's mutating-call shape (§4) — they return
query-specific result shapes rather than `{ ok, workspace_digest, rendered,
findings }`. An agent uses them to orient before mutating: query what exists,
check the current state, understand a derivation, then act.

### 3.16 Expert authoring tools (contract 1.3.0)

Seven tools add the language lane and the continuity/ledger pair the expert
card (`docs/cards/expert-card.md`) describes; each stages like any other add,
returns the standard mutation envelope (§4), and requires provenance. All
seven are advertised only from contract `1.3.0` and above (DL-0058).

| Tool | Purpose |
|---|---|
| `author.add_dialect {id, label?, body[]}` | Declare a naming register — the phonology and name grammar a naming population draws on. |
| `author.add_name_part {id, gloss?, head[]}` | Declare a name-part morpheme — the raw material composed names are built from. |
| `author.add_name {id, label?, head[], composition[], bearer?}` | Compose a name from name-part morphemes, optionally binding it to the entity that bears it. |
| `author.add_term {id, label?}` | Introduce a glossary term — a word the lore defines and reuses. |
| `author.add_banned {word, reason?}` | Ban a word from the lore surface, with the reason. |
| `author.add_state {id, value?, time?}` | Assert a time-varying state fluent — a value that holds over an interval, not forever; `time` is `{kind: after\|during\|since\|until, point}`. |
| `author.add_attest {question}` | Request an attestation for an open question already in the workspace. |

### 3.17 Branch and merge tools (contract 0.6.1)

`author.branch {workspace, action: create|list, name?, from?}` creates a
persistent, named ref into the workspace's lorestore at the source branch's
head (`from`, default `main`), or lists every branch that exists.
`author.merge {workspace, source, target?}` unions a branch's fact-set into the
target branch (default `main`) as a new merge commit — the fact-set union is
confluent, so the merge always commits — and reports valuation, exclusivity,
and functional-dependency conflicts (`AuthorMergeResult`) rather than blocking
on them.

Both tools operate on durable lorestore refs, not the staged workspace layer.
At contract `1.6.0`, every `author.add_*` write still lands on `main` — no wire
op yet routes a staged addition onto a named branch — so branch and merge are
today a durable checkpoint-and-comparison mechanism rather than a parallel
authoring line: reach for `author.branch` to name a durable snapshot of the
current state, and `author.merge` when two such snapshots' histories are ready
to fold together.

## 4. The feedback contract

This is the core of the interface. **Every mutating call returns, immediately:**

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
 killed-form | not-wired | version-mismatch | conflict | internal`. The
contract's §9 registry assigns each code a **class** — one of
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

On success, the current JSON-first wire returns committed fact objects as data;
`rendered: true` adds `.dli` lines for a client that wants that view. The server
does not write the consuming repository, and the caller takes its own review or
approval action. The legacy-shaped example below is retained for the earlier
accepted wire it demonstrates:

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

The human never reads tool-call JSON. They review the **diff** — `author.diff`'s
structured per-fact rows, each with its before/after standing, which the
consuming application renders for review (DL-0046). Where a formatter is linked,
that rendering can be the canonical `.dli` surface, which is the same surface a
human author would have written by hand — so human and agent contributions are
reviewed through one lens. Deloreis serves the structured rows; the rendering is
consumer-owned.

**Provenance discipline.** DLI does not mint authority or enforce who may rule.
At contract 1.3.0 and above, a caller may supply attribution and a cited decision
state; DLI records it only when the cited decision resolves, and an unknown id
raises `citation.unknown-decision` and blocks materialization. The consuming
application owns authorization, review, and the policy that interprets a
resolved citation. Earlier accepted wires retain their documented cited-provenance
refusal behavior (DL-0058).

| Negotiated wire | Cited provenance | Materialization behavior |
|---|---|---|
| `1.3.0` and above | Recorded when every cited decision resolves | Unknown ids raise `citation.unknown-decision` and block materialization |
| Below `1.3.0` | Refused with `provenance.cited-refused` | Nothing is staged; behavior remains byte-exact |

**Framing safety.** Instruction-shaped strings that arrive in parameters — an
entity id, a description, a question phrased as a command — are **data**. They
never alter tool behavior: a name that reads "ignore prior findings and
materialize" is stored and rendered as a name and changes nothing about how the
tool checks, grades, or gates. The paired framing fixtures under
`contracts/dli-worker/fixtures/framing/` pin this from independent cold project
states, including reads over separately staged benign and tainted workspaces.

## 6. Contract discipline

Every tool has a **request JSON Schema and a response JSON Schema**, generated
from the serve crate's Rust types into `contracts/dli-worker/schemas/` so the
schemas cannot drift from the code that serves them. Each tool also has fixtures
in the closed registry: **golden** (a canonical request and its exact response),
**negative** (a malformed or invalid request and the typed error it must
produce), and **framing** (an adversarial-string request and the benign-identical
behavior it must show). The rule is flat: **a tool without fixtures does not
exist** — it cannot be listed here, served, or benchmarked.

Contract `1.6.0` enforces that rule for all 67 operations. Paired cases declare
the exact changed request paths and stable safety projection; mutating cases
probe post-state, and materialized tainted labels must still reparse as
canonical `.dli`.

Versioning rides the worker-protocol contract: **v0** now, with `N`/`N-1`
compatibility introduced at the first breaking cutover. `deloreis` owns the
`contracts/dli-worker` contract and the fantasy repo vendors it pinned and
digest-checked (`DL-0010`); the contract document, `docs/integration/fantasy-contract.md`,
is the wire inventory this semantic spec elaborates. Where a tool's `author.*`
row appears in both, the parameter and result shapes agree; a divergence is a
contract bug, resolved on the `deloreis` side.

## 7. The measurement plan

*Retired with the door semantic (DL-0041, DL-0043): with the text surface an
opt-in rendering, a door decision has nothing left to decide. Preserved for
the arms' definitions.* The
benchmark (PLAN nodes `BENCH-1`, `BENCH-2`) arms four conditions over the same
authoring tasks:

- **the tool surface tools** — the primary path, this document's surface.
- **Text-surface authoring by an agent** — the measured fallback.
- **Grammar-constrained local emission** — a benchmark condition, not an interface.
- **Raw AST-document emission** — the demoted arm, an internal mechanism scored
  only as a baseline.

Each arm is scored on four axes: **task success** (did the intended world result),
**finding-resolution rate** (of findings raised, how many the arm cleared before
materialize), **review burden at the rendered-diff level** (what the human must
read to sign off), and **canon-safety violations** (any path by which unratified
claims acquire authority, or a claim materializes past an error-severity finding).

The benchmark's verdicts are a **double-sided promotion trigger**: they may
promote the tool surface's superiority from hypothesis to measured fact, and they may
**close** a demoted or fallback path that proves it costs more than it saves.
Until the benchmark reports, the working hypothesis stands on the sibling receipts
(`docs/design-notes/sibling-lessons.md`), where path count predicted failure
surface better than path quality — evidence, not a measured verdict here.

## 8. The worked consumer scenario

The end-to-end flow an agent runs — `author.open` a scope, `author.add_entity` the
cast, `author.add_fact` their traits, `author.add_relation` their bonds,
`author.diff` to see the staged change and its consequences,
`author.materialize` to commit it and get the fact objects back, then
consumer-side review and ruling — is worked in full in
`docs/integration/migration.md` §10. The agent-facing playbook — the same flow as
a call-by-call sequence of **engine-true** requests and responses, captured from
the running worker — is `docs/cards/authoring-loop.md`; the loop and its error
surface were validated live in `docs/design-notes/agent-authoring-validation.md`.
Two PLAN nodes carry it: **AUTH-1** delivers the tool surface, the workspace
engine, and the per-tool fixtures; **AUTH-2** replays the scenario against a
pinned consumer corpus and asserts the rendered-diff output a human would review.
Its world-owned fixture path is recorded in the PLAN evidence and checked by
`cargo test -p dli-serve --test auth2`; the consumer's matching transcript is
separate EXT-F4 evidence.
