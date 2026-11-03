---
status: reference
verdict: "Historical roadmap for the lorestore cutover; DL-0101 supersedes the text-surface deprecation and benchmark-era priorities. The current release direction is in docs/release/north-star.md and the decision ledger."
---

# The lorestore roadmap

This is a historical plan for the storage pivot. Its discussion of demoting or
removing `.dli`, former integrations, contract versions, and archive-scale
priorities records the earlier direction; it is not the current release plan.
DL-0101 restores `.dli` as a first-class surface and establishes the fiction
scope. The [decision ledger](../decisions.md) and
[release north star](../release/north-star.md) govern current work.

Stages are written in the `PLAN.md` idiom: a deliverable, its needs, what it
produces, and a falsifiable oracle. No dates. The stage ids (`LS-0` … `LS-8`)
are the node ids for the `store` lane in `PLAN.md`.

## 1. What is invariant, and what dies

The pivot is a surface change, not a semantics change. The system's DNA — the
part every comparison note identifies as the thing no other system has — is
untouched:

- **The annotation algebra.** Every fact carries a support/defeater pair
  `(P⁺, P⁻)` of positive-Boolean polynomials over event variables minted by the
  decision ledger. Standing (certain / possible / dead) is *derived* from
  provenance under a valuation, never stored.
- **Saturate once, valuate many.** One symbolic fixpoint serves every fork,
  what-if, and ratification pre-flight as a cheap post-hoc valuation.
- **Grades are honest** (DL-0006), rollup is weakest-grade over the procedure
  chain, and judgments replay from committed inputs.
- **The gate is offline and deterministic** (DL-0013). Lorestore reads no
  clock, calls no model, touches no network.
- **The engine never decides canon** (DL-0054, DL-0058). Callers supply
  attribution and decision state; provenance gates assertability structurally
  and the engine checks the decision references it can resolve.

What dies is the claim that `.dli` text is the canonical form of any of that:

- Storage moves from `.dli` files in git to content-addressed JSON fact
  objects in an embedded store (the SQLite posture: a library the `deloreis`
  binary links, not a deployment).
- The formatter stops defining "the one canonical surface" (the surviving half
  of DL-0003 is the schema: **the schemars-derived JSON Schema is the
  definition**; that half gets stronger, since the schema now describes the
  stored objects themselves, not an AST that text is parsed into).
- Human review moves from git diffs of text to structured diffs of fact
  objects, served as JSON and rendered by whatever client the consumer
  wires up — deloreis is an MCP-addressable engine and ships no review UI.
- `.dli` becomes one renderer among several — a pretty-print for humans who
  want the sentence-like view — behind an explicit opt-in.

## 2. Sequencing constraints (why the stages are ordered as they are)

1. **The ledger outranks everything.** Two standing decisions collide head-on
   with this pivot: DL-0003's formatter-canonical clause, and DL-0029
   ("accepted syntax may not disappear or become an inert declaration").
   Nothing in this roadmap is authoritative until DL-0041 supersedes those
   clauses explicitly. DL-0029 is not repealed but *relocated*: retired surface
   flows through the killed-form registry with a named successor, exactly as a
   retired keyword always has — the successor is now a tool call or a JSON
   shape rather than a replacement keyword.
2. **The former cutover is retired.** Contract 2.0.0 is the only served wire; deleted versions and operations are
   registered as killed forms. Golden, negative, and framing fixtures remain required.
3. **The local frontier is empty** (DL-0038 closed the standalone release's
   local scope). Lorestore is therefore the natural next local programme and
   can begin immediately as a leaf crate with zero blast radius.
4. **A lucky break: the grammar was never frozen.** DL-0022 deferred SURF-4
   and the BENCH two-route study. There is no freeze to unwind — the milestone
   is retired, not reversed, and the route question (text surface vs. tools)
   is answered by events: the text surface closes, and DL-0043 retires the
   door semantic itself.

## 3. The stages

### LS-0 — Ratify the pivot

*Deliverable:* DL-0041 in the ledger; `AGENTS.md` laws updated; `PLAN.md`
gains the `store` lane; the four source notes marked as consolidated by this
roadmap.
*Needs:* nothing.
*Produces:* the authority every later stage cites.
*Oracle:* `cargo xtask authority` and the docs-oracle are green; no doc
contradicts the ledger; DL-0041 names the superseded clauses of DL-0003 and
DL-0029 explicitly.

### LS-1 — The fact-object model (the new language definition)

*Deliverable:* Rust types + schemars JSON Schemas for the stored object kinds:
`FactObject`, `Commit`, `Ref` (with valuation overlay), `EvidenceEnvelope` —
plus the canonical-bytes rule (RFC 8785 JCS) and the SHA-256 identity rule.
*Needs:* LS-0.
*Produces:* the normative schema every later stage stores, serves, and
validates against.
*Oracle:* **total lowering** — every construct the current AST can express
lowers to fact objects, and lowering the whole vendored corpus then compiling
reproduces the identical fixpoint, findings, and digests (shadow-read parity,
the MIG/PROOF idiom re-aimed).

**The stored shapes, as they serialize.** Each block below is validated in CI
against the schema its fence names — the docs-oracle's `json` path (LS-7,
`xtask/src/docs_oracle.rs`) runs `dli_lorestore::schemas()` over it — so a
worked example here cannot drift from the type it documents. The objects are
a real import of three gentle-subset declarations into the `camelot` teaching
pack.

A fact object. `support` names interner index `0` on the positive rail; the
commit's `interner` resolves it to the ruling variable `h.session-7c9`.
`defeaters` is the empty polynomial — this atom is positive-only, so
`conditions` is absent too:

```json fact-object-v1
{
  "schema_version": 2,
  "authority": null,
  "claim_id": "sha256:ea0c222b30c84ca5d23509d461c8e77dfb66ff8d63a488a7bd25f1bf97ce7b92",
  "atom": {
    "pred": "serves",
    "args": ["bedivere", "the-steward"]
  },
  "support": { "monomials": [["+0"]] },
  "defeaters": { "monomials": [] },
  "grade": "asserted"
}
```

The same atom once a later ruling can defeat it. `defeaters` is the
atom-global `P⁻`; `conditions` pins the poison to the one support monomial it
travels with, which is what keeps the per-path reading exact when a fact is
derived along several routes:

```json fact-object-v1
{
  "schema_version": 2,
  "authority": null,
  "claim_id": "sha256:ea0c222b30c84ca5d23509d461c8e77dfb66ff8d63a488a7bd25f1bf97ce7b92",
  "atom": {
    "pred": "serves",
    "args": ["bedivere", "the-steward"]
  },
  "support": { "monomials": [["+0"]] },
  "defeaters": { "monomials": [["+1"]] },
  "conditions": [
    {
      "path": { "monomials": [["+0"]] },
      "poison": { "monomials": [["+1"]] }
    }
  ],
  "grade": "asserted"
}
```

The commit that introduced them. `parent` is *commit* ancestry — the field
that is deliberately absent from every fact object's content hash — and
`interner` is the flat name table that makes `"+0"` resolvable:

```json commit-v1
{
  "schema_version": 2,
  "authority": null,
  "parent": "sha256:be78d4f6522725ad26e08555610344a380a3ef586a6301aa80420fe58288dce2",
  "facts": [
    "sha256:c683f5a192155f254faee16a4a307e808f18c0115c61ce200a21cae7e38e6c82",
    "sha256:056a2db800878f58335d4468fdcfe0147caa1e1ac52f92c997170d5c9c1fef91",
    "sha256:38451bf53df17159d2c4e2a97ae8d5bae7cf6794427297050d7c03417b0384b6",
    "sha256:4c13431ad3453808339ddd0fd1d1299f5b7b2c581fcb06f9555bbeee9b3ce1cd"
  ],
  "lineage": [],
  "evidence": [],
  "interner": ["h.session-7c9"],
  "pack_name": "camelot"
}
```

A ref: a named pointer plus a valuation overlay. `main` carries the empty
overlay; a fork ref is the same object with event variables ruled in it,
which is why creating a fork is O(1):

```json ref-v1
{
  "schema_version": 2,
  "authority": null,
  "name": "main",
  "commit": "sha256:abc5b8afc2d11d19d46925752e08b200dd6840636de212451fa1b1895eee6821",
  "valuation": {}
}
```

A lineage edge — supersession as a first-class object carrying its own
`(P⁺, P⁻)`, so *whether* the retcon holds is itself valuation-dependent:

```json lineage-edge-v1
{
  "schema_version": 2,
  "authority": null,
  "claim_id": "sha256:ea0c222b30c84ca5d23509d461c8e77dfb66ff8d63a488a7bd25f1bf97ce7b92",
  "from": "sha256:4c13431ad3453808339ddd0fd1d1299f5b7b2c581fcb06f9555bbeee9b3ce1cd",
  "to": "sha256:38451bf53df17159d2c4e2a97ae8d5bae7cf6794427297050d7c03417b0384b6",
  "support": { "monomials": [["+1"]] },
  "defeaters": { "monomials": [] }
}
```

An evidence envelope. The payload is opaque to the store — the envelope is
the integrity layer, and `payload_digest` is a second, independent check that
the payload has not drifted inside it. No floats anywhere, per the storage
rule below:

```json evidence-envelope-v1
{
  "schema_version": 2,
  "authority": null,
  "subject": "camelot",
  "kind": "attestation",
  "payload": {
    "question": "is the steward served?",
    "verdict": "holds",
    "trials": 1
  },
  "payload_digest": "sha256:1f77b0d1e5f7d2c3b4a59687e0c1d2a3b4c5d6e7f80912a3b4c5d6e7f8091a2b"
}
```

Design rules, each load-bearing (see §4 for the theory):

- **Three identities, kept apart.** The vision note's object sketch embeds a
  bare `parent` hash inside the fact. That conflates three needs the design
  genuinely has, and serves none of them well:
  1. *Authority distinctness* (canon vs. fanon, one continuity vs. another)
     needs no parent — the provenance polynomial and `authority` field are
     part of the object and its hash, so the "same" proposition asserted by
     two authorities is already two distinct objects. Dedup can never
     conflate them; it only merges facts identical in provenance too.
  2. *Claim continuity* ("this claim, through its revisions" — statement
     identity in the Wikibase sense; lineage-aware manipulation) gets a
     **stable minted claim id** alongside the content hash — the
     entity-vs-datom split. A parent chain serves this badly: hashes change
     on every revision regardless, and a Merkle chain makes amending
     anything early re-hash every descendant.
  3. *Supersession and retcon history as domain knowledge* gets
     **first-class lineage edge objects carrying their own `(P⁺, P⁻)`** —
     because in a multi-authority world, *whether* B supersedes A is itself
     contested and valuation-dependent (one continuity accepts the retcon,
     another rejects it), and the edge must cite the ruling it happened
     under. A bare hash inside B hard-codes one true lineage and cannot
     cite anything.
  Only the bare parent pointer stays out of the content hash: it delivers
  none of the three, and it costs cross-branch convergence — the same
  assertion reached on two forks must be the same object for merge to be
  union (§4). Storage-level ancestry (which write produced which state)
  lives in the commit DAG, as in git.
- **Canonical polynomial form.** `(P⁺, P⁻)` are stored as
  absorption-normalized irredundant monotone DNF in a fixed total order:
  literals within a monomial ordered by interned atom id (positive rail
  first), monomials ordered by the same literal order. Irredundant monotone
  DNF is *unique* for a monotone Boolean function, so equal provenance means
  equal bytes means equal hash — this is the invariant that makes
  content-addressing sound. The evaluator already absorption-normalizes at
  every join; the storage rule just freezes that as the on-disk form. (The
  order is the evaluator's own — numeric by interned id, not lexicographic
  over wire tokens; an independent implementation must intern identically to
  hash identically.)
- **No floats anywhere in stored objects.** JCS inherits ECMAScript number
  serialization; the safe rule is integers and strings only. Future
  confidence values (multi-authority layers) enter as decimal strings.
- **`schema_version` on every object, upcast on read.** The
  formatter-as-migrator law dies with the surface; its replacement is typed
  upcasts — old versions remain readable forever, and a version bump ships
  with its upcast, in the killed-form spirit.
- **The `authority` field exists and stays unset** in single-sovereign
  deployments (DEF-4 unchanged).
- **The expert tier is named, versioned debt.** The code audit
  (`design-notes/dli-syntax-coupling-audit.md`) found the AST only half
  structured: eleven declaration types carry `body: Vec<Clause>` where a
  clause is whitespace-normalized raw source text, re-parsed downstream at
  semantic-analysis time. Fact-object schema v1 may carry these bodies as
  formatter-canonicalized strings — deterministic to hash, but semantically
  opaque — and the `schema_version` + upcast machinery is how they later
  deepen to typed interiors (LS-9) without rewriting stored facts. The v1
  schema must mark these fields as the expert tier explicitly, so nobody
  mistakes an embedded micro-syntax string for a structured interior.

### LS-2 — The lorestore crate (single-branch MVP)

*Deliverable:* the `lorestore` crate: append-only loose-object store
(git-style fan-out directories), refs with valuation overlays, an append-only
commit log, compare-and-swap ref updates behind the existing advisory lock,
tmp-file + rename atomicity with explicit fsync discipline, and a storage
trait with exactly one backend: the filesystem.
*Needs:* LS-1.
*Produces:* durable, deterministic persistence for fact objects.
*Oracle:* cross-platform determinism (the same operation sequence yields the
same root hash on every platform) and crash-injection (killing the process
between object write and ref update never produces a torn store — recovery
replays or discards, deterministically).

Three deliberate refusals, recorded so they are not re-litigated by accident:

- **Index-free at the atelier tier — a sequencing choice, not a scale
  ceiling.** The cutover's consumer lives at 10³–10⁵ facts, where full
  in-memory saturation is milliseconds and a persistent index is pure
  failure surface. The archive tier (§6) — encyclopedic multi-continuity
  corpora at 10⁶–10⁸ facts — is a designed-for destination whose index
  track (the indexing note's Phases 2–4) enters on measured triggers after
  the cutover. What makes the deferral safe is the MVP's contract: **the
  log is the truth; every index is a derived cache; `state ≡ fold(commit
  log)` is a property test in the gate.** The tier boundary is crossed by
  adding readers, never by migrating stored facts.
- **No branches beyond `main`.** Forks-as-valuation-overlays already work
  in memory and are cheap; persistent divergent *fact sets* are the only
  thing that needs real branches, and that need arrives with the merge
  engine (LS-6), not before. Single-branch keeps the MVP honest and small.
- **No remote backends.** S3/IPFS wait for a real remote deployment (flip
  condition, §6). The gate is offline; the first backend is a directory.

One unification this stage buys for free: **the workspace digest becomes the
root commit hash.** Digest fencing (optimistic concurrency for agent
sessions) becomes exact, O(1), and Merkle-verifiable instead of a recomputed
summary.

### LS-3 — The read path (dual backend)

*Deliverable:* `dli_world::compile` accepts a lorestore source; a one-way
`.dli → lorestore` import tool; configuration selects the backend.
*Needs:* LS-2.
*Produces:* a world compiled from stored fact objects; the migration path for
every existing corpus.
*Oracle:* differential parity — the same corpus loaded from `.dli` files and
from its lorestore import compiles to the identical world model, findings,
and digests.

The fixpoint, valuation, and checking machinery does not change in this
stage. Only the front of the loader changes: instead of parse → lower →
compile, the store path is fetch → validate-against-schema → compile. The
*file* parser leaves the critical path here, but the parser itself does not:
expert-tier clause bodies are still re-parsed during semantic analysis
(`dli-world`, `dli-ledger`, `dli-time` — see the coupling audit), and that
dependency falls only with LS-9.

The `Commit` schema's `decisions` and `pack_name` fields ride this stage;
their ledger home is DL-0045 (ruling 2). Implementation state lives in the
PLAN store-lane evidence, per the docs discipline.

### LS-4 — The write path (author.* writes the store; JSON-first contract)

*Deliverable:* mutating `author.*` tools write fact objects directly
(validate → compile polynomial → object write → CAS ref → return graded
findings + digest); `author.materialize` returns JSON by default with `.dli`
rendering opt-in; contract **0.6.0**, additive and dual-mode.
*Needs:* LS-3.
*Produces:* the tool surface backed by lorestore; the agent loop loses its
write-rendered-lines-to-disk step.
*Oracle:* golden/negative/framing fixtures re-recorded and green; framing
attacks leave verdicts unchanged; the vendored-contract compatibility suite
still passes for a 0.5.1 consumer (nothing removed, nothing reshaped).

The killed-form registry moves up a layer here: a retired tool op or a
retired input shape is refused *by the server* with its successor named,
exactly as the parser refused retired keywords. DL-0029's guarantee — nothing
accepted ever silently vanishes — survives the death of the syntax it was
written about.

This stage is smaller than it looks: the code audit found the tool surface already
JSON-first on input — all twenty `author.*` tools take typed
`deny_unknown_fields` structs, the workspace stages typed declarations in
memory, and the digest is already a SHA-256 over canonical JSON. The `.dli`
coupling is output-side only (`materialize` and the rendered diff), plus two
text-on-wire remnants (`fork.add`, wire query goals) that retire in LS-7.

Write-path implementation is tracked in the PLAN store-lane evidence; the
governing rulings are DL-0044 as amended by DL-0045 (ruling 3: per-decl
lowering is the taught path; the workspace digest and the commit hash stay
distinct fields).

### LS-5 — The query and review contract

*Deliverable:* a formal JSON query-response schema — every row carries its
bindings, its provenance antichain (the minimal-witness monomials), its
grade, and the pair `(root_hash, valuation)` it was evaluated under — plus
the structured-diff/review contract: MCP-served JSON describing pending
change-sets, per-fact diffs with provenance, and accept/reject semantics
(the tool-surface read-tool idiom, DL-0023).
*Needs:* LS-4 (write path produces the diffs worth reviewing).
*Produces:* the replayable verdict format agents and humans both consume; the
review data that replaces git-diff-of-text.
*Oracle:* a stored query result replays byte-identical from its
`(root_hash, valuation, query)` triple; a golden review-flow fixture drives
a seeded change-set to the same accept/reject decisions through the JSON
contract alone.

The scope discipline: **deloreis ships no review UI — not web, not TUI.** It
is an MCP-addressable engine; consumers wire the contract to whatever
surface they want (a web app, an editor plugin, or plain agent
interaction). This dissolves the deprecation note's biggest flagged
dependency — a web UI — cleanly: it was never deloreis's to build. What
deloreis owes is a contract complete enough that *any* client can render a
faithful review, which is exactly what the golden fixture proves.

### LS-6 — The merge engine (the genuinely new machinery)

*Deliverable:* a commit DAG with merge commits; 3-way merge over fact sets
with structural conflict detection; branch creation/merge exposed through
the tool surface.
*Needs:* LS-5; this is deliberately the *last* core stage.
*Produces:* persistent divergent worlds with deterministic reconciliation —
the capability git-of-text never actually gave us.
*Oracle:* property tests — merge is commutative and associative up to root-
hash equality on conflict-free corpora; a seeded-conflict corpus (mutation-
harness idiom) demonstrates every conflict class below is caught, named, and
carries repair data.

The merge design falls out of the algebra (§4 for the full argument):

- Facts are immutable and the fact set is append-only, so the store's
  monotone core is a grow-only set with a per-fact polynomial join
  (`⊕` = alternative support). Union of monotone accumulations is a
  join-semilattice merge: confluent, order-independent, coordination-free.
  **Modify/modify conflicts — the entire hard case of text merging — cannot
  exist.** Merge here is *simpler* than git's, not harder.
- Conflict is confined to exactly three non-monotone places, and those are
  the merge report:
  1. **Valuation disagreement** — two branches supply different values for the
     same event variable. Never auto-resolved; the consuming application decides
     its policy and records the result.
  2. **Constraint violations in the merged world** — functional-dependency,
     exclusivity-family, certain-layer contradiction — detected by
     recompiling the merged result; surfaced as ordinary graded findings
     with repair data.
  3. **Stratified-negation effects** — fact additions are not monotone
     across strata (a new fact can kill a higher-stratum derivation), so
     merged worlds recompute affected strata; the diff names atoms whose
     standing changed.

### LS-9 — Expert-tier structuring (the hidden prerequisite)

*Deliverable:* typed interiors for the eleven expert-tier declaration bodies
(`RuleDecl`, `ForkDecl`, `PracticeDecl`, `DialectDecl`, `DraftDecl`,
`ExportDecl`, `NameDecl`, `VariantDecl`, `RatifyDecl`, `PrincipleDecl`,
`ProofDecl`) — the downstream semantic parses in `dli-world`, `dli-ledger`,
and `dli-time` become constructors of typed AST, and stored facts upcast from
the v1 string-bodied schema.
*Needs:* LS-1 (the schema and upcast machinery it deepens).
*Produces:* an AST that is actually AST all the way down; the condition under
which the parser can leave the default build at all.
*Oracle:* no crate outside `dli_syntax` calls a parse function on stored
data; `validate_proof_decl`'s format-and-reparse round-trip is replaced by a
structural validator; upcasted corpora compile identically to their
string-bodied originals.

The id is out of sequence because this stage was found by the code audit
(`design-notes/dli-syntax-coupling-audit.md`) after LS-0..LS-8 shipped; its
DAG position is between LS-1 and LS-7. It is the real bulk of the retirement
— roughly, restructuring interiors across ~23k lines of `dli-world`/
`dli-ledger`/`dli-time` versus deleting ~7.5k lines of `dli-syntax` — and it
can proceed incrementally, one declaration type at a time, behind the upcast.

### LS-7 — Surface retirement

*Deliverable:* `dli_syntax` demoted to an optional pretty-printer crate
(feature-gated out of the default build); reference cards rewritten around
tool calls and JSON shapes; the docs-oracle re-aimed: fenced `json` examples
validate against the LS-1 schemas, fenced `.dli` checked only in the docs
that still deliberately teach the rendering (67 fenced blocks across 21 files
at audit time); the two text-on-wire remnants (`fork.add` declaration
strings, wire query goals) retired through the killed-form registry with
typed successors.
*Needs:* LS-4 (agents), LS-5 (humans — the structured-diff contract replaces
the forgery-proofed rendered-lines diff as the supervision input; rendering
is consumer-owned), LS-9 (the parser has no remaining semantic caller).
*Produces:* the deprecation note's phases 2–3, done.
*Oracle:* the default build does not link `dli_syntax`; the docs-oracle law —
**every fenced example in the docs is machine-checked** — holds continuously
through the swap; killed-form entries exist for every retired surface form.

*Landed so far:* the docs-oracle's `json` path. Every fenced `json` block now
carries its check class in the fence itself, so none is silently unchecked:
` ```json <schema-name> ` parses and validates against that LS-1 stored-object
schema (the shapes worked above); a bare ` ```json ` must be well-formed JSON;
` ```json transcript ` marks the elided `→`/`←` wire sketches, whose markers
and bracket balance are themselves checked so the tag cannot become a blanket
escape hatch. An unrecognized tag is a hard failure, never a silent downgrade,
and the oracle refuses to run with fewer than three schema-validated blocks —
the path cannot rot into a no-op. Fenced `.dli` is untouched and still checked
wherever the docs deliberately teach the rendering.

Full removal (the deprecation note's phase 4) is deliberately unscheduled:
an optional pretty-printer crate is cheap to keep and expensive for early
adopters to lose. Park it behind a flip condition (§6).

### LS-8 — Contract 1.0 and consumer cutover

*Deliverable:* the post-pivot contract major version: JSON-first tool
surface, lorestore-backed, `.dli` rendering opt-in; the former consumer's
migration from vendored 0.5.1.
*Needs:* LS-4..LS-7 internally; **EXT-F4 externally** — the v1 handshake
completes on v1 terms before v1 is retired.
*Produces:* one storage model, one interchange format, one review surface.
*Oracle:* the consumer speaks 1.0 against the vendored, digest-checked copy;
the 0.5.1 dual-mode paths are then retired through the killed-form registry,
not deleted silently.

## 4. Theory appendix — why the design is shaped this way

This section records the academic grounding so future arguments start from
the literature, not from scratch.

**Provenance semirings.** The `(P⁺, P⁻)` pair is a dual **PosBool[X]**
provenance annotation in the sense of Green, Karvoulyrakis & Tannen
("Provenance Semirings", PODS 2007). PosBool[X] — positive Boolean
expressions over a variable set, quotiented by absorption — is the classic
why-provenance semiring: `⊕` is alternative derivation, `⊗` is joint
derivation, and the minimal-witness antichain (what `--why` renders) is
exactly the set of minimal monomials. Two consequences the roadmap leans on:
(1) the fixpoint's absorption normalization is not an optimization but the
semiring's congruence, and (2) irredundant monotone DNF is a *unique* normal
form for monotone functions, which is what makes hashing annotated facts
well-defined (LS-1). The defeater channel is the same algebra run for
override conditions; standing is a function of both under a valuation.

**CALM and confluent merge.** The CALM theorem (Consistency As Logical
Monotonicity — Hellerstein & Alvaro) says monotone programs are eventually
consistent without coordination. The lorestore write model is engineered to
live inside that theorem: the fact set only grows, polynomials only gain
alternative support, and supersession is *revaluation* (an event variable
flips) rather than retraction. Merge on the monotone core is therefore a
join-semilattice least-upper-bound — a grow-only-set CRDT with a per-element
join — and needs no coordination to be deterministic. Everything
non-monotone is quarantined into three named places (valuations, constraint
checks, stratified negation), and that quarantine *is* the merge report
(LS-6). This is the formal version of "git merges lines; lorestore merges
facts": git must reconcile arbitrary edits; lorestore only ever reconciles
growth plus a small, enumerable set of non-monotone judgments.

**Incremental view maintenance, mostly avoided.** The classic difficulty of
incremental Datalog (DRed, counting algorithms, DBSP/differential dataflow)
is *deletion* — retracting a fact and un-deriving its consequences. The
architecture dodges it: facts are never deleted, and a ruling change is a
valuation flip that does not touch the symbolic fixpoint at all — the
polynomials were computed for *all* valuations at once. The only incremental
work that ever exists is *addition*, and additions are handled by ordinary
semi-naive deltas (already the evaluator's core), with recomputation above
the affected stratum as the sole negation correction. Record this insight
now so the indexing note's Phase 3 ("Medium-High lift") is not built at
generic-IVM strength when the workload needs a fraction of it. DL-0009's
salsa-readiness posture (pure phases, no stateful compiler) is exactly the
shape a later memoization drop-in wants.

**Event sourcing, honestly applied.** The commit log is the system of
record; every index, cache, and materialization is a fold over it. The
gate-checked invariant `state ≡ fold(log)` (LS-2) is what makes "rebuildable"
a theorem instead of a hope, and it is the property that keeps replayable
judgments replayable when storage stops being git.

**Archive scale.** Eager full saturation is the *atelier-tier* optimum, not
a universal law. At encyclopedic scale (§6) the derived set explodes —
appearance-style relations (character × issue) dominate, and transitive
rules multiply them — so the archive tier pairs a saturated hot core with
goal-directed evaluation (magic sets / sideways information passing) over
indexed base facts for the long tail. Provenance follows the same split:
derived polynomials are kept as shared circuits with the k-minimal-witness
antichain materialized, exact DNF on demand. The canonical-DNF hashing
invariant is untouched by any of this — it binds *stored base facts*, whose
polynomials stay small; derived provenance is computed, never
content-addressed. Same algebra at both tiers; only the evaluation strategy
changes.

**Honest asymptotics.** "O(1) fork" is two claims and the docs should state
both: branch/fork *creation* is O(1) (a ref plus an overlay); fork
*evaluation* is O(dependent slice) through the support index. The grades law
(DL-0006) applies to performance claims too.

**Crash consistency.** Append-only object stores get atomicity from
write-to-temp + fsync + rename, and refs get it from compare-and-swap; the
window between object write and ref update must be recoverable in either
direction. Orphaned objects are garbage, never corruption; a ref must never
point at an object that is not fully durable. This ordering discipline is
cheap and must be property-tested (crash injection, LS-2), because it is the
one place "deterministic and immutable" meets a kernel page cache that is
neither.

## 5. Risk register

1. **Ledger collision handled out of order.** If any stage lands before
   DL-0041, the tree contradicts the ledger. LS-0 gates everything;
   discipline is the mitigation.
2. **The former cutover.** The private consumer pins the pre-reset tag; contract 2.0.0 serves the fiction release.
3. **Hash-identity mistakes are forever.** `parent` inside the fact, float
   serialization, unsorted monomials — each silently poisons content
   addressing, and repair after real facts exist means rewriting history.
   Mitigation: the LS-1 design rules are gate-checked before the first
   durable store ships.
4. **Review-surface coupling.** The deprecation note's review story rests on
   a web UI that does not exist. Mitigation: it was never deloreis's to
   build — deloreis is an MCP-addressable engine, and LS-5's deliverable is
   the review JSON contract; consumers own rendering, on their own side of the wire.
5. **Docs and cards churn.** The teaching surface (`docs/cards/`) teaches a
   syntax that is being demoted. Mitigation: the docs-oracle law is never
   suspended — the oracle is *re-aimed* (LS-7), and cards are rewritten in
   the same stage that feature-gates the parser, not before.
6. **Scope discipline in both directions.** The archive-tier index track
   (§6) is *not* scope creep — encyclopedic multi-authority corpora are a
   designed-for destination, and under-building for them is the opposite
   failure. What stays gated: the reasoning-model extensions (argumentation,
   uncertainty, fuzzy entity resolution) are PhD-scale programmes the
   exploring-status note is explicit about; they enter only by ledger
   entry.
7. **Evidence double-home.** During dual-backend operation, evidence lives
   in `.dli-evidence/` and in lorestore's evidence module. Mitigation: both
   use the same JCS + SHA-256 addressing, so content addresses are identical
   and migration is a move, not a rewrite; the digest is the check.
8. **Merge-engine ambition.** LS-6 is the only stage with genuinely novel
   machinery. Mitigation: it is last, it is not on the cutover's critical
   path (single-branch operation covers the consuming project's needs), and
   its formal skeleton (§4) is written down before its code.
9. **The expert tier is the real retirement cost.** Eleven declaration
   bodies are raw clause strings re-parsed downstream; without LS-9 the
   parser never actually leaves the build, and fact objects smuggle
   micro-syntax as data. Mitigation: the v1 schema names the debt
   explicitly, and the upcast machinery lets it retire one declaration type
   at a time (see `design-notes/dli-syntax-coupling-audit.md`).

## 6. Scale tiers, the archive track, and parked work

The engine has **two designed-for populations**, orders of magnitude apart,
and index decisions only make sense stated against them:

- **The atelier tier** — one world, one-to-few authorities, 10³–10⁵ facts:
  the consuming audio-drama project, a novelist, a game studio. Full
  in-memory saturation runs in milliseconds; persistent indexes add failure
  modes and buy nothing. The cutover (LS-1..LS-8) targets this tier because
  the consuming project lives in it.
- **The archive tier** — encyclopedic, multi-continuity, multi-authority
  corpora, 10⁶–10⁸ facts: a full creature-catalogue with moves, forms, and
  per-episode appearances runs 10⁵–10⁶ facts; a comics multiverse, where
  character-by-issue appearance relations dominate, runs 10⁷ and up before
  fan authorities multiply it. **This tier is a designed-for destination,
  not a hypothetical.** The indexing note's Phases 2–4 are its committed
  track — sequenced after the cutover, entered on measured triggers.

What breaks at archive scale, and the committed remedy for each:

| What breaks | At roughly | Committed remedy |
|---|---|---|
| Full-scan query | ~10⁵ facts | Indexing note Phase 2: entity/predicate/citation/variable posting lists — sorted runs and bitmap-style structures, because appearance-heavy relations are IR/OLAP-shaped, not B-tree-shaped |
| Whole-corpus recompile | ~10⁵–10⁶ facts, or continuous ingestion | Phase 3 incremental compile, kept simple by §4's no-retraction property: semi-naive deltas for additions, recompute above the affected stratum, no DRed machinery. **Entered now, ahead of Phase 2, on the continuous-ingestion condition (DL-0087).** |
| Eager full saturation | ~10⁶–10⁷ facts | Hot-core saturation + goal-directed evaluation (magic sets / SIP) for the long tail; partition by authority and scope, composing with branches (§4, "Archive scale") |
| Cross-authority query and conflict scan | archive multi-authority work | Phase 4 cross-branch indexes as per-authority visibility maps, bitmap-composable over the one shared fact store |

The thresholds are engineering triggers inside a committed direction — they
say *when* each remedy enters the critical path, not *whether*. The atelier
tier never pays for them: `state ≡ fold(log)` means every index is a derived
reader that bolts on without touching stored facts.

Parked work, PLAN idiom — named, owned by a trigger, not built before it:

| Track | Trigger |
|---|---|
| Compaction / packfiles | Store size or object count degrades clone/load measurably |
| Peer/service loopback, then remote backends | A measured workload or archive deployment needs bounded remote retrieval (DL-0062) |
| `.dli` full removal (deprecation phase 4) | The pretty-printer crate's maintenance cost exceeds its use |
| Review rendering | None — consumer-owned by design (LS-5); deloreis ships the JSON contract, never a UI |
| Worker cancellation + bounded diagnostic | A second captured trace shows any handler exceed an interactive budget for a reason other than an accidental-complexity bug (DL-0086). The `serve` worker is synchronous with no cancellation and the gate law bars the kernel from reading a clock, so a client-side deadline against a slow handler presents as an undiagnosed stall; a genuine slow path (not a fixable blunder) is the trigger to design a bounded/streaming answer |
| Multi-authority valuations (DEF-4) | The first real second-authority corpus; the archive tier's authority model builds on the `authority` field and per-authority valuations |
| Argumentation / uncertainty / fuzzy-ER (indexing note Phases 5–7) | Exploring, not committed; a ledger entry is the only way in |

**Logged data points toward the triggers.** GH **#75** (an incremental-canonicalization pilot and DL-0062 evidence trace) surfaced an
`author.merge` dry-run stalling past the client's deadline on a growing identity
world. Its dominant cause was an **accidental** O(M³) merge base, fixed in place
to O(M+E) and orthogonal to this track (DL-0086) — a bug, not a threshold. But it
also exercised two patterns this section owns, at a scale far below their
thresholds, and is logged here so the aggregate is visible when the real
threshold arrives: the merge scan's whole-world recompiles (**Phase 3**
incremental compile — the pilot is *continuous ingestion*, the named condition)
and merge-consequence/ancestry work over the commit DAG (**Phase 4** cross-branch
+ branch-ancestry indexes, the archive-tier form of the `merge_base` the fix
computes by hand). #75 does not trip a *size* trigger — but it does satisfy the
*continuous-ingestion* condition, which is what moves Phase 3.

**Phase 3 is now entered — ahead of Phase 2 (DL-0087).** The historical pilot used
continuous ingestion (recompiles scale with mentions, not facts),
so incremental compilation is the one archive-track item with a live trigger and a
near-term performance payoff; secondary indexes (Phase 2) wait. The v1 scope is an
in-process, rebuildable compiled-world cache (a bounded LRU; `state ≡ fold(log)`
makes eviction safe) plus an incremental fixpoint that reuses the prior saturation
and recomputes above the affected stratum — **not** the persistent on-disk indexes
(deferred) and **not** a sliced check battery (deferred). Its safety contract is a
permanent gate-resident oracle: incremental compile must equal full recompile
byte-identically under every addition ordering, with the full recompile kept as the
runtime fallback. This supersedes the indexing note's "start with Phase 2"
recommendation for the continuous-ingestion trigger.

**Corrected payoff (DL-0088).** PR 1 established that the sound reuse unit is the
dependency-closed stratum (atom-level reuse is unsound under the defeater-conjoining
`or_`), so the fixpoint win is delta-position dependent and **~1.0× for `same-as`
deltas** — identity assertions synthesize alias rules over every predicate, marking
the whole world affected. The fixpoint was never the pilot workload’s cost driver,
though: the dominant term is `merge::scan`'s 3–8× whole-world recompiles per
`author.merge` (× ~7 harness workers per mention), which the compiled-world cache and
incremental loader attack directly. So Phase 3's payoff for that workload is a
**several-fold constant factor, not O(delta)**; beating ~1.0× for `same-as` needs
per-derivation provenance (its own research lever), and the whole-world check battery
is the predicted next bottleneck (instrumented in PR 3, sliced later).

## 7. Effect on standing documents

- `docs/PLAN.md` — gains the `store` lane (LS-0..LS-8); SURF-4/BENCH-1/
  BENCH-2 are retired (not deferred) per DL-0041.
- `AGENTS.md` — §3 (AST-first restated schema-first; docs-oracle law
  re-aimed), §5 (commit ritual simplified), §7 (the tool surface returns JSON by
  default).
- `docs/spec/01-syntax.md` and `docs/cards/` — stale after LS-7; rewritten in
  that stage; until then they document the opt-in rendering.
- The four source notes — remain as reasoning; each gains a pointer to this
  roadmap as the consolidated sequence.
- `docs/integration/` — contract evolution lands as 0.6.0 (LS-4) then 1.0
  (LS-8) under the DL-0031..DL-0037 fixture discipline.
