---
status: proposed
verdict: "The canonical staged roadmap for the lorestore cutover: facts become content-addressed JSON objects in an embedded deterministic store; .dli is demoted from canonical surface to optional rendering. Consolidates lorestore-vision, dli-deprecation-plan, lorestore-indexing-strategy, and multi-authority-knowledge-evolution into one dependency-ordered plan with oracles, a theory appendix, and a risk register. Authority flows from DL-0041; this note is the worked plan behind it."
---

# The lorestore roadmap

This is the working plan for the storage pivot: deloreis stops being a language
with a text surface and becomes a database with an algebra. The four notes that
argued the direction — `lorestore-vision.md`, `dli-deprecation-plan.md`,
`lorestore-indexing-strategy.md`, `multi-authority-knowledge-evolution.md` —
remain the reasoning; this note is the sequence. Where they disagree with each
other or with this note, this note wins; where this note disagrees with the
ledger, the ledger wins (DL-0041 is the ratifying entry).

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
- **The machine never writes canon** (DL-0014). Agents propose through tools;
  provenance gates assertability structurally.

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
2. **The fantasy cutover is frozen on v1.** MIG-3/MIG-4 and EXT-F3/EXT-F4 are
   consumer-owned and still run against `.dli` + contract 0.5.1. Until EXT-F4
   acknowledges the Door-T handshake, every lorestore change is **additive**:
   dual-mode, digest-vendored, nothing removed. The contract discipline of
   DL-0031..DL-0037 (version bumps, golden + negative + framing fixtures per
   op) is the template.
3. **The local frontier is empty** (DL-0038 closed the standalone release's
   local scope). Lorestore is therefore the natural next local programme and
   can begin immediately as a leaf crate with zero blast radius.
4. **A lucky break: the grammar was never frozen.** DL-0022 deferred SURF-4
   and the BENCH two-door study. There is no freeze to unwind — the milestone
   is retired, not reversed, and the two-door question (surface vs. tools) is
   answered by events: the surface door closes.

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
  absorption-normalized irredundant monotone DNF with monomials and variables
  in lexicographic order. Irredundant monotone DNF is *unique* for a monotone
  Boolean function, so equal provenance means equal bytes means equal hash —
  this is the invariant that makes content-addressing sound. The evaluator
  already absorption-normalizes at every join; the storage rule just freezes
  that as the on-disk form.
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

- **No secondary indexes.** At the designed corpus scale (10³–10⁵ facts) a
  full load is microseconds to milliseconds, and the compile-then-query
  architecture materializes everything in memory anyway. This *refines* the
  indexing note's "start at Phase 2" recommendation down to "start at
  Phase 1": the index phases are real but parked behind flip conditions
  (§6). The store's contract makes them safe to add later: **the log is the
  truth; every index is a derived cache; `state ≡ fold(commit log)` is a
  property test in the gate.** Anything rebuildable is droppable.
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

### LS-4 — The write path (author.* writes the store; JSON-first contract)

*Deliverable:* mutating `author.*` tools write fact objects directly
(validate → compile polynomial → object write → CAS ref → return graded
findings + digest); `author.materialize` returns JSON by default with `.dli`
rendering opt-in; contract **0.6.0**, additive and dual-mode.
*Needs:* LS-3.
*Produces:* Door T backed by lorestore; the agent loop loses its
write-rendered-lines-to-disk step.
*Oracle:* golden/negative/framing fixtures re-recorded and green; framing
attacks leave verdicts unchanged; the vendored-contract compatibility suite
still passes for a 0.5.1 consumer (nothing removed, nothing reshaped).

The killed-form registry moves up a layer here: a retired tool op or a
retired input shape is refused *by the server* with its successor named,
exactly as the parser refused retired keywords. DL-0029's guarantee — nothing
accepted ever silently vanishes — survives the death of the syntax it was
written about.

This stage is smaller than it looks: the code audit found Door T already
JSON-first on input — all twenty `author.*` tools take typed
`deny_unknown_fields` structs, the workspace stages typed declarations in
memory, and the digest is already a SHA-256 over canonical JSON. The `.dli`
coupling is output-side only (`materialize` and the rendered diff), plus two
text-on-wire remnants (`fork.add`, wire query goals) that retire in LS-7.

### LS-5 — The query and review contract

*Deliverable:* a formal JSON query-response schema — every row carries its
bindings, its provenance antichain (the minimal-witness monomials), its
grade, and the pair `(root_hash, valuation)` it was evaluated under — plus
the structured-diff/review contract: MCP-served JSON describing pending
change-sets, per-fact diffs with provenance, and accept/reject semantics
(the Door-T read-tool idiom, DL-0023).
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
Door T.
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
  1. **Valuation disagreement** — two branches rule the same event variable
     differently. Never auto-resolved; the sovereign decides (koan-1
     territory).
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

Full removal (the deprecation note's phase 4) is deliberately unscheduled:
an optional pretty-printer crate is cheap to keep and expensive for early
adopters to lose. Park it behind a flip condition (§6).

### LS-8 — Contract 1.0 and consumer cutover

*Deliverable:* the post-pivot contract major version: JSON-first tool
surface, lorestore-backed, `.dli` rendering opt-in; the fantasy consumer's
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
provenance annotation in the sense of Green, Karvounarakis & Tannen
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
2. **The fantasy freeze.** A breaking contract change before EXT-F4 strands
   the consumer mid-cutover. Mitigation: additive-only until the handshake;
   dual-backend and dual-mode keep v1 alive indefinitely if fantasy stalls —
   the roadmap has no step that *requires* v1's death on a schedule.
3. **Hash-identity mistakes are forever.** `parent` inside the fact, float
   serialization, unsorted monomials — each silently poisons content
   addressing, and repair after real facts exist means rewriting history.
   Mitigation: the LS-1 design rules are gate-checked before the first
   durable store ships.
4. **Review-surface coupling.** The deprecation note's review story rests on
   a web UI that does not exist. Mitigation: it was never deloreis's to
   build — deloreis is an MCP-addressable engine, and LS-5's deliverable is
   the review JSON contract; consumers own rendering, exactly as the fantasy
   worker owns its side of the wire.
5. **Docs and cards churn.** The teaching surface (`docs/cards/`) teaches a
   syntax that is being demoted. Mitigation: the docs-oracle law is never
   suspended — the oracle is *re-aimed* (LS-7), and cards are rewritten in
   the same stage that feature-gates the parser, not before.
6. **Aspirational scope creep.** Argumentation frameworks, uncertainty
   propagation, and fuzzy entity resolution are PhD-scale programmes the
   exploring-status note is explicit about. Mitigation: they appear in this
   roadmap only as flip-conditioned deferrals (§6); none is greenlit by the
   pivot.
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

## 6. Deferred tracks and flip conditions

The PLAN idiom: parked work is named, owned by a flip condition, and not
built before its trigger fires.

| Track | Flip condition |
|---|---|
| Secondary indexes (indexing note Phase 2) | Measured query latency exceeds interactive budget, or a real corpus approaches ~10⁶ facts |
| Persistent incremental compile (Phase 3) | Full recompile time measurably degrades the authoring loop |
| Cross-branch query indexes (Phase 4) | The merge engine (LS-6) is in real multi-branch use |
| Compaction / packfiles | Store size or object count degrades clone/load measurably |
| Remote backends (S3/IPFS) | A real deployment needs storage the filesystem cannot host |
| `.dli` full removal (deprecation phase 4) | The pretty-printer crate's maintenance cost exceeds its use |
| Live web review UI | The CLI/TUI + static-HTML surface measurably limits review throughput |
| Multi-authority valuations (DEF-4) | A real second sovereign exists (writers' room, shared universe) |
| Argumentation / uncertainty / fuzzy-ER | Each keeps its "exploring, not committed" standing; a ledger entry is the only door in |

## 7. Effect on standing documents

- `docs/PLAN.md` — gains the `store` lane (LS-0..LS-8); SURF-4/BENCH-1/
  BENCH-2 are retired (not deferred) per DL-0041.
- `AGENTS.md` — §3 (AST-first restated schema-first; docs-oracle law
  re-aimed), §5 (commit ritual simplified), §7 (Door T returns JSON by
  default).
- `docs/spec/01-syntax.md` and `docs/cards/` — stale after LS-7; rewritten in
  that stage; until then they document the opt-in rendering.
- The four source notes — remain as reasoning; each gains a pointer to this
  roadmap as the consolidated sequence.
- `docs/integration/` — contract evolution lands as 0.6.0 (LS-4) then 1.0
  (LS-8) under the DL-0031..DL-0037 fixture discipline.
