---
status: settled
verdict: "A design-level comparison of De Lore Is against mainstream lore-wiki backing technology: SWM, graphdb-backed wikis, Cargo-style dependency systems, Datomic, and other Datalog-family solutions. Covers query patterns, data storage, schema evolution, invariant enforcement, runtime perf, and the single-authority valuation model. Updated for the lorestore (LS-1/LS-2/LS-3): the three-way identity split, content-addressed fact objects, and commit DAG."
---

# Conceptual comparison: De Lore Is vs. mainstream lore backing technology

## What De Lore Is is

De Lore Is is a **knowledge language** for fictional worlds — a declarative, Datalog-derived dialect that records *what is true in a world, since when, on whose say-so, and what breaks if it changes.* It is not a play-time engine (not Inform/TADS), not a choice-flow language (not Ink/Twine). It occupies the third family of narrative language:

| Family | Models | Runs at | Exemplars |
|---|---|---|---|
| Simulation | world state | play | Inform, TADS, Dialog |
| Flow | discourse and choice | play | Ink, Twine, ChoiceScript |
| **Knowledge** | **what is true, on whose say-so** | **authoring** | articy (commercial, no provenance semantics); **De Lore Is** |

It compiles `.dli` source files into an **annotated fixpoint** — a saturated set of ground atoms, each carrying its **support polynomial** (a positive-Boolean formula over event variables) and its **defeater polynomial** (what could override it). From these two channels, every claim's **standing** (certain / possible / dead) and **grade** (checked > tested > attested > asserted) are *derived*, never stored.

The architecture is layered across ~17 Rust crates: `dli-ast` (typed AST + schemars-derived JSON Schema), `dli-syntax` (hand-rolled recursive-descent parser + formatter), `dli-core` (the annotation algebra), `dli-eval` (the annotated seminaive fixpoint + fork manager), `dli-world` (loader, fact-checking, timeline, practices, language, craft), `dli-export` (export backends), and `dli-serve` (MCP/JSON-over-stdio server).

---

## Data model and storage

### Source files

Source files (`.dli`) live in git, organized by `scope` (a `scope <name>` header per file). Claims are annotated with provenance:

- `proposed <origin>` — agent's draft, structurally unassertable (gates on an open hypothesis variable)
- `decided <d-id>` — grounded in a recorded decision
- `ratified <d-id>` — a ratified ruling
- `provisional <d-id>` — a provisional ruling, open to revision

The **decision ledger** (`decisions.md` — or the consuming project's `canon/decisions.md`) is the authority. `ratify <d-id>` declarations in `.dli` files mint the event variables that the polynomial algebra indexes into.

### Compiled cache and evidence store

- `.dli-cache/` — gitignored, content-hash-keyed; never a source of truth, only a rebuild target.
- `.dli-evidence/` — immutable, RFC 8785 JCS-canonical JSON documents (attestation batches, payload records, simulation manifests) addressed by SHA-256. Writers take a project-wide advisory lock and publish atomically. Write-once, append-only.

The machine's entire durable write surface is bounded to these two directories. It **never** writes `canon/` or ledger content that does not cite an existing ruling.

### The lorestore (LS-1/LS-2/LS-3)

The lorestore is the **durable, content-addressed persistence layer** that
replaces the loose cache/evidence directories with a single transactional
abstraction. It is built on three design invariants (DL-0041):

**The three-way identity split.** Fact objects, commits, and refs have
*independent* identity:

- **Fact objects** are content-addressed (`sha256:<64-hex>` over JCS-canonical
  JSON). Their hash embeds *no* ancestry — a fact's identity is its assertion,
  not its history. This is the "three-way split": a fact lives in its own
  address space, separate from commits and refs.
- **Commits** carry storage-level ancestry (git-style parent pointers). A
  `Commit` names the fact objects it introduces, the lineage edges, evidence
  envelopes, the event-variable interner (the name table that makes `Atom(u32)`
  indices in support polynomials self-describing), the known `Decl::Ratify`
  decision ids, and the pack/scope name. The commit's hash absorbs all fields.
- **Refs** are named pointers to commits, carrying a valuation overlay. Creating
  a ref is O(1) — a pointer + an overlay, exactly the `dli_eval::ForkStore`
  model made durable. The `main` ref always exists; fork refs
  (`fork/option-x-true`) come in LS-6.

**Storage layout** (LooseObject, not a graph):
```
objects/<2-hex>/<remaining-62-hex>   ← content-addressed JSON blobs
refs/<name>                          ← named ref → commit hash
root/.lock                           ← advisory fs lock (fs2)
```

**The read path** (`Lorestore::compile()`, LS-3) walks the commit chain from
`main` → root, reads the head commit's interner + decisions + pack name,
reads every named fact object, calls `lower::reconstruct_pack` to rebuild a
`Pack`, then `dli_world::compile` produces the identical `CompiledWorld` as
if the `.dli` files had been loaded directly. This is verified by a
**differential parity oracle** (`tests/differential_parity.rs`): the same
vendored corpus loaded from `.dli` and from its lorestore import compiles to
identical fixpoint, findings, and digests (206 atoms, 183 facts, 183
derived, 0 findings — all matching).

This is not a graph database. There are no nodes, no edges, no maintained
indices, no traversal. A fact object is a flat JSON blob addressed by its
hash. The "graph" emerges at query time from the support/defeater
polynomials, which the symbolic fixpoint saturates once.

### Schema evolution

The AST's JSON Schema is normative (DL-0003). The formatter is the canonical-to-canonical migration. The **killed-form registry** ensures retired keywords are refused at parse time with a `renamed-to` diagnostic — never silently dropped, never accepted as unknown. Supersession is **revaluation**, not rewrite: changing a ruling revalues a variable, and every claim's history stays intact.

This is fundamentally different from a graph database backing. A graphdb stores triples/nodes/edges as first-class entities; De Lore Is compiles the entire corpus into one **annotated Datalog program** where the support/defeater polynomials are the only data structure.

---

## Query patterns

There is exactly one query surface: `dli.query` (CLI) and `author.query` / `dli.query` (server), with `--layer`, `--fork`, `--as-of`, `--why`.

The query is a **pattern-match against the saturated relation** — every ground atom already has its support computed. What changes per query is the **valuation** (which event variables are true/false), not the fixpoint. This is the critical architectural decision: **the symbolic fixpoint is computed once, and all standing/grade computations are post-hoc evaluations over it.** A fork's evaluation reuses the same precomputed supports, applying a valuation overlay in a single pass.

The **read path** through the lorestore is a compile-time concern, not a query-time one: `Lorestore::compile()` reconstructs a `Pack` from stored fact objects (via `reconstruct_pack`), then runs the existing `dli_world::compile`. Queries never touch storage directly — they operate on the in-memory `CompiledWorld`, which is `Arc`-shared across sessions.

Compare to a graphdb query (Cypher/SPARQL), where each query traverses the graph from scratch against maintained indices. De Lore Is is closer to **Datalog engines** (fixpoint once, then query), but with the critical addition of **provenance polynomials** that make the fixpoint *symbolic* — a single saturation serves infinitely many valuations.

---

## Invariant enforcement

Six continuity classes are checked at compile time:

1. **Status-derivation** — `proposed` facts gate on open hypothesis variables; `ratified`/`decided` facts gate on declared decision variables; citations to undeclared decisions fail closed (`P⁺ = FALSE`).
2. **Provenance** — claims without provenance are fail-closed.
3. **Range-restriction** — every rule variable must appear in a positive body literal (Datalog safety).
4. **Stratification** — negative-dependency cycles are load-time errors.
5. **Temporal** — era-order chains and `before`-facts must be cycle-free.
6. **Rejected-lore** — banned tokens, killed forms, conflict constraints.

The **grade lattice** (`checked > tested > attested > asserted`) uses weakest-link rollup over the *procedure chain*, never over citations. Authority rides the support channel, not the grade channel (DL-0006). An `asserted` premise under a `checked` procedure keeps the procedure `checked`.

The **fork pre-flight** is a free query: before valuating an option, it scans for three conflict kinds (valuation disagreement, exclusivity violation, certain-layer contradiction).

---

## Runtime performance and concurrency

**Fork creation is O(1).** A fork is a valuation overlay + local additions, not a copy of the world. The base world is saturated once; every fork evaluates over that single symbolic fixpoint with a valuation passed as a parameter.

Concurrency: the server runs at most 4 simulation workers concurrently (hard limit), with an advisory lock on evidence writes. Forks and queries are **pure functions** over `(World, Valuation, Query)` — the evaluator has no stateful `Compiler` struct (DL-0009). The compiled world is behind `Arc` in the session layer, so it's shareable across threads. Fork evaluation is embarrassingly parallel.

This contrasts with a graphdb: Neo4j is single-writer, multi-reader. Queries run against maintained indices but re-traverse per query. There's no concept of "fork" in a graphdb — you'd model a what-if as a separate database instance (expensive) or application-level versioning.

The lorestore itself (the storage backend) is **offline and deterministic** — no network, ever, in a gating lane (DL-0013). The gate is `cargo xtask gate`: 17 steps (rustfmt, clippy, nextest, doc build, doctests, build, dli-fmt, schema --check, docgen --check, fixtures, docs-oracle, authority, boundary, docs-tokens, cargo-deny). All 17 run in the deterministic offline lane and block merges.

---

## The export architecture (One Fabric vs. Satellite Semantics)

Koan 10 ("one fabric") is the defining architectural constraint: **all world knowledge — geography, magic, names, story-time, games — lives in one runtime under one semantics.** Exports are **one-way compiles to targets** (context-pack, card set, Ludii GDL, Articy JSON, CSV, static wiki). Every export reads the **certain layer** and fails closed on proposed lore.

---

## Comparison with mainstream technology

### MediaWiki / SMW (the dominant wiki backing)

Most fan-media wikis today use **MediaWiki** (PHP + MySQL) or **Semantic MediaWiki** (SMW), which adds typed properties, semantic queries (SPARQL), and some constraint checking.

| Dimension | MediaWiki / SMW | De Lore Is |
|---|---|---|
| Data model | Pages with templates; semi-structured at best | Annotated Datalog: ground atoms with (P⁺, P⁻) polynomials |
| Schema | Whatever templates happen to contain; OWL via SMW (optional, often unenforced) | AST JSON Schema is normative; kinds with slots as functional deps |
| Time model | Edit history / revision IDs | Partial order over eras/events; fluents with since/until/during; cycles |
| Query | Full-text + category links; SMW queries / SPARQL | Pattern match over precomputed symbolic fixpoint |
| Provenance | Edit history (social convention); SMW named graphs | Logical provenance polynomials over event variables |
| What-if | Separate draft namespaces (unstructured) | Forks are O(1) valuation overlays |
| Invariant enforcement | Optional constraints, often not checked | Six continuity classes, fail-closed |
| Grade/trust | Editorial tags, manual | `checked > tested > attested > asserted` lattice with weakest-link rollup |
| Runtime | PHP + MySQL/Elasticsearch/Redis | Rust, pure functions, thread-safe |
| Storage | Files in git + database | `.dli` in git + lorestore (content-addressed objects + commit DAG) |

The key gap: in a wiki, every edit is immediately visible and (barring protection) authoritative. There is no clean separation between "proposed" and "ratified" content. In De Lore Is, `proposed` is **structurally unassertable** — a proposed fact gates on an open hypothesis variable that no one can rule true except the sovereign author.

### Graphdb-backed wikis (Neo4j, ArangoDB)

| Dimension | Graphdb | De Lore Is |
|---|---|---|
| Data model | Labeled property graph (nodes, edges, properties) | Annotated Datalog (pos polynomials over event variables) |
| Schema | Node labels + relationship types; constraints (optional) | AST JSON Schema normative; kinds with slots |
| Query | Cypher / AQL pattern matching | Pattern match over precomputed symbolic fixpoint |
| Provenance | None (or app-level as separate nodes) | First-class: every atom carries (P⁺, P⁻) |
| Time | App-level (version nodes) or none | Partial order + fluents + cycles |
| What-if | Separate DB, transactions (expensive) | O(1) valuation overlay |
| Invariant enforcement | Structural constraints only | Six continuity classes, fail-closed |
| Grade/trust | App-level node properties | Formal grade lattice |
| Concurrency | Single writer, multi-reader clusters | Pure functions over shared fixpoint |
| Storage | Nodes/edges in graphdb engine; maintained indices | Loose JSON objects, content-addressed; commit DAG for ancestry |

A graphdb can store "this entity has this property" with a "source" property. It cannot express "this fact holds if ratification D-0208 AND hypothesis H-003 are both true, OR if D-0146 is true AND breached is false" — and it certainly cannot apply a valuation change in O(derived atoms) without traversing the graph again.

### Cargo/npm-style dependency systems

Cargo and npm solve **dependency resolution** and **deterministic builds** — which transitive versions to compile against. They don't model provenance of *facts* within a single corpus. De Lore Is borrows the **deterministic-offline** principle (DL-0013) and the **single-authority** principle, but applies them to lore facts rather than code packages. Where Cargo has a `Cargo.lock` pinning exact versions, De Lore Is has a `decisions.md` ledger pinning exact rulings, and every citation is verified at compile time.

### Datomic and Datalog-family solutions

Datomic is the closest professional prior art: a fact-based, immutable database with Datalog queries and explicit transaction-time.

| Dimension | Datomic | De Lore Is |
|---|---|---|
| Data model | Datoms `[e a v t]` | Ground atoms with (P⁺, P⁻) annotation polynomials |
| Schema | Typed attributes (string, ref, cardinality) | AST JSON Schema normative; kinds with slots |
| Time model | Transaction time (linear, ordered) | Partial order + fluents + cycles |
| Query | Datalog `(q [...])` over latest/as-of view | Pattern match over precomputed symbolic fixpoint |
| Provenance | Transactional (who did what when) | Logical provenance polynomials (why is this true) |
| What-if | `with` queries (in-memory, not persisted) | O(1) valuation overlay, pre-flight conflict scan |
| Certainty | Binary: true as-of-transaction-T | Three-valued: certain / possible / dead |
| Grade | None (app-level attributes) | `checked > tested > attested > asserted` lattice |
| Schema evolution | Retractions/replacements | Killed-form registry; supersession-as-revaluation |
| Concurrency | Single transactor (writes) | Pure functions, thread-safe shared fixpoint |
| Determinism | Sequential transactions; queries may use set semantics | Fully deterministic offline gate; no clock |
| Storage | DynamoDB/PG/SQL backend | `.dli` in git + lorestore (content-addressed objects + commit DAG) |

#### Key architectural divergences from Datomic

**1. Provenance granularity.** Datomic's provenance is *transactional* — "this fact was asserted in transaction T by user U." De Lore Is provenance is *logical* — "this atom holds if (D-0208 ∧ H-003) ∨ (D-0146 ∧ ¬breached)." Datomic can say who added a fact and when. De Lore Is can say what logical conditions must hold for a fact to be true, and what breaks if any condition changes.

**2. Three-valued standing vs. binary truth.** In Datomic, a datom is true or false (as-of time T). A "proposed" fact lives in a staging transaction. Once in the database, it's true. De Lore Is has **three-valued standing** derived from the annotation polynomial: a `proposed` fact is *possible* — structurally unassertable because its support variable is open. No Datalog query over Datomic can produce this distinction.

**3. Grade as inferred procedure strength.** De Lore Is grades derive *through* derivation: a `checked` procedure over `asserted` premises stays `checked` (DL-0006). Datomic has no concept of procedure recheckability — facts are equally true regardless of how they were derived.

**4. Symbolic fixpoint + valuation overlay.** Datomic materializes views *as of* a transaction — it scans the log. De Lore Is computes one symbolic fixpoint over positive polynomials, then applies valuations as cheap post-hoc evaluations. A fork costs O(derived atoms), not O(rewalk the transaction log).

**5. One fabric.** Datomic is general-purpose — your world entities, game rules, and audio script are separate concerns sharing a transactor. De Lore Is has **Koan 10**: all world knowledge — geography, magic, names, story-time, games — lives in one runtime under one semantics.

#### Other Datalog-family solutions

- **Soufflé / bddengine:** Bottom-up fixpoint Datalog. Closer to De Lore Is's evaluator than Datomic, but lacks provenance polynomials, three-valued logic, graded trust, and the fork/valuation mechanism (you'd re-run the fixpoint for each what-if).
- **SWI-Prolog with tabling:** Top-down SLD resolution (query-driven), not bottom-up fixpoint. Fundamentally different evaluation strategy.
- **RDF triplestores with reasoning (Jena, GraphDB):** Essentially Datalog-over-RDF. Support rule-based inference but lack provenance polynomials, grade lattice, and three-valued standing.

---

## What this actually solves (that wikis don't)

The core problem De Lore Is addresses: **the difference between "I wrote this down" and "this is true in the world."**

In a wiki, every edit is immediately visible and (barring protection) authoritative. There's no clean separation between proposed and ratified content. In De Lore Is, `proposed` is *structurally unassertable* — a proposed fact gates on an open hypothesis variable that no one can rule true except the sovereign author. It can never accidentally become "certain" in a query unless someone explicitly ratifies it.

The **three-valued standing** (certain/possible/dead) is derived from the polynomial algebra, not a status field someone forgot to update. And "what breaks if I change this" is answered by a single fork+diff: the support index tells you exactly which atoms depend on which event variables, so changing a ruling ripples through with perfect precision.

This is provenance tracking of the kind that **IPFS/IPLD** aspires to for data, or **DBSan + provenance** does for databases, but applied to *fictional world knowledge* with a full Datalog evaluator as the engine. It's not a wiki with better templates — it's a database with the provenance model of a research database (SemanTEq, Trio/Gradual provenance) but with a Datalog dialect as the query/derivation language and a Rust engine treating the whole thing as a pure, deterministic, offline artifact.

---

## The single-authority valuation model (and why wikis don't have it)

De Lore Is explicitly models **single-authority valuation** (charter koan 1). The machine may propose, check, render, and explain — but it may never make a thing true (koan 3). Multi-authority valuation (writers' rooms, shared universes, wiki canon) is **explicitly out of v1** (DEF-4 in the PLAN; DL-0002's budget).

This is the fundamental mismatch with fan wikis: fan wikis are *collaborative by default* — many authors, no sovereign. De Lore Is is a tool for *dictator-mode* collaborative fiction — the author who is the final arbiter of truth, assisted by agents that propose but cannot decide. Every claim asks "on whose say-so" because there is exactly one say-so that matters: the sovereign author's ruling.
