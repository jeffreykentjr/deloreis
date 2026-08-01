---
status: live
verdict: "Conceptual comparison of De Lore Is against wiki structured-data backends (SMW, Cargo, Wikibase) and datalog fact stores (Datomic, XTDB, DataScript) across storage, querying, schema evolution, invariants, concurrency, provenance, inference, multi-user, and LLM agent integration."
---

# Architectural comparison: De Lore Is vs structured-data backends

This note locates De Lore Is in the broader design space of systems that store
structured knowledge about fictional (or curated) worlds. The comparison covers
the wiki-native structured-data extensions that power real fan wikis today, and
— as technical triangulation — the datalog-backed immutable fact stores that
share architectural DNA with deloreis but serve different populations.

The comparison is conceptual: no benchmarks, no code samples, no claim about
runtime speed beyond what falls out of the architecture. The decision ledger
outranks this document.

---

## 1. Architecture sketches

### Semantic MediaWiki (SMW)

SMW layers an RDF-like triple store on top of MediaWiki's page model. Each wiki
page can annotate its prose with typed property–value pairs (via `[[Has
population::12000]]` inline markup); properties and categories are themselves
wiki pages whose definitions constitute the schema. The triples land in MySQL
(or a SPARQL store backend) and are queryable via SMW's ASK language — a
simplified, wiki-friendly subset of SPARQL. Inference is limited to transitive
subcategory/subproperty closure; there is no rule engine. Provenance is page
revision history — who edited the page, when — with no per-triple citation
model.

### Cargo

Cargo is a MediaWiki extension that sidesteps the triple model entirely: it
declares relational tables *inside* wiki templates, and every page that
transcribes the template populates a row. Storage is plain MySQL tables sitting
beside the wiki's own page tables. Queries use a SQL-like syntax
(`{{#cargo_query:}}`), giving template authors direct access to joins, filters,
and aggregates. Schema is the template declaration — adding a column means
editing a template and running a `recreateData` maintenance script. There is no
inference, no provenance beyond page history, and no constraint enforcement
beyond MySQL column types and nullability.

### Wikibase (Wikidata model)

Wikibase is the software behind Wikidata: a purpose-built entity store where
each item carries statements (property–value pairs), each statement carries
qualifiers (sub-properties) and references (source citations). The data model is
a labeled property graph with first-class citation support. Querying is full
SPARQL over a Blazegraph (or Qlever) endpoint. Schema is emergent — properties
are entities with datatype constraints, but there is no closed-world schema
enforcer; any property can appear on any item. Multi-user editing with per-edit
provenance. No built-in inference engine, but SPARQL property paths and
federation enable ad-hoc reasoning.

### Datomic family (Datomic, XTDB, DataScript)

These are general-purpose immutable fact stores built around Datalog.  Storage
is an append-only log of EAV (entity–attribute–value) datoms, each stamped with
a transaction id. The log is the database — there are no updates, only
assertions and retractions whose history is retained forever. Queries are
Datalog (with negation, aggregation, and pull expressions). Time-travel is
native: `as-of`, `since`, and `history` queries read the database at any past
transaction. Schema is data — attributes are themselves entities with type,
cardinality, and uniqueness constraints. Datomic uses a peer architecture
(query runs in the application process against a shared immutable storage
service); XTDB is a standalone node; DataScript is an in-memory
ClojureScript/JS implementation. All support recursive rules; none provide a
purpose-built provenance algebra or domain-typed invariant enforcement.

### De Lore Is

De Lore Is is a knowledge language for fiction: a typed record of what a world
holds and, for every claim, where it came from and on whose ruling. Source
files are flat, order-insensitive, line-oriented `.dli` declarations living in
a git repository — git is the persistence layer. There is no database process;
the `deloreis` binary compiles source into a symbolic fixpoint (an annotated
Datalog saturation) in memory, answers queries, and writes derived artifacts.
Evidence (attestation batches, payload records) is stored as content-addressed
immutable JSON under `.dli-evidence/`. A derived cache (`.dli-cache/`,
gitignored) holds compiled snapshots.

The core intellectual machinery is a provenance algebra: every atom carries a
pair of positive-boolean polynomials `(P⁺, P⁻)` — support and defeater — over
event variables minted per ruling clause in an append-only decision ledger.
Status (certain / possible / dead) is *derived* from provenance, never stored.
The symbolic fixpoint is computed once; forks, what-ifs, and ratification
pre-flights are cheap valuations applied *afterward* over unchanged polynomials.
The system is single-threaded, deterministic, offline, and explicitly
single-authority in v1.

---

## 2. Axis-by-axis comparison

### 2.1 Storage model & persistence

| System | Storage shape | Persistence | Mutability |
|---|---|---|---|
| SMW | RDF triples in MySQL (or SPARQL store) | RDBMS on wiki server | Mutable; old triples overwritten on page save |
| Cargo | Relational rows in MySQL | RDBMS on wiki server | Mutable; rows rebuilt from template transclusion |
| Wikibase | Entity-statement graph in Blazegraph + MySQL | Graph DB + RDBMS | Mutable with full edit history per-entity |
| Datomic | EAV datoms in append-only log | Immutable storage service (DynamoDB / Postgres / local) | Append-only; retractions are new datoms |
| De Lore Is | Flat `.dli` text + content-addressed JSON evidence | Git repository (no DB process) | Append-only by discipline; supersession replaces deletion |

**Strengths and weaknesses:**

- **SMW/Cargo** gain free hosting on commodity LAMP stacks and zero-setup
  persistence — the wiki's MySQL *is* the database. The cost is that structured
  data is a second-class citizen bolted alongside page blobs; bulk export, backup
  granularity, and offline use all route through the wiki's own maintenance
  tooling.
- **Wikibase** dedicates a purpose-built graph store to the structured layer,
  gaining flexible multi-typed statements at the cost of operational complexity
  (Blazegraph/Qlever, WDQS, entity serialization). Offline use is possible only
  via periodic JSON dumps.
- **Datomic** trades MySQL simplicity for immutability and time-travel: every
  past state is retrievable, and readers never block writers. The cost is
  operational — a storage service, a transactor, and (for Datomic Cloud) vendor
  lock-in. XTDB reduces this by running standalone.
- **De Lore Is** eliminates the database entirely: source is text in git, which
  gives versioning, branching, merge, and offline use for free. The cost is that
  every query requires compiling the source into a fixpoint first (milliseconds
  at corpus scale, but not the sub-millisecond random-access a warm RDBMS
  gives), and there is no server to leave running between queries — the worker
  process is spun up per session.

### 2.2 Query patterns & expressiveness

| System | Query language | Recursion | Provenance in results |
|---|---|---|---|
| SMW | ASK (wiki inline) | No | No (page name only) |
| Cargo | SQL-like (`#cargo_query`) | No | No |
| Wikibase | SPARQL 1.1 | Property paths (transitive closure) | References retrievable per-statement |
| Datomic | Datalog (+ pull expressions, aggregates) | Yes (recursive rules) | Transaction metadata queryable |
| De Lore Is | Annotated Datalog (`dli.query --layer/--as-of/--fork/--why`) | Yes (stratified, range-restricted) | Full polynomial support returned per row |

**Strengths and weaknesses:**

- **SMW's ASK** is approachable for wiki editors — it reads almost like prose —
  but lacks joins beyond property chains, has no aggregation, and cannot express
  recursive reachability (e.g. "all places reachable from Camelot via
  adjacency").
- **Cargo** gives wiki editors real SQL power (joins, GROUP BY, HAVING) at the
  cost of requiring SQL literacy and coupling queries to the template-defined
  schema. No recursion.
- **Wikibase/SPARQL** is the most expressive of the wiki options: full SPARQL
  1.1 with property paths covers transitive closure, optional patterns, and
  federation. The cost is a steep learning curve and a heavy query service.
- **Datomic** brings full recursive Datalog with aggregation, pull patterns for
  tree-shaped results, and the ability to query transaction history. Expressively
  comparable to deloreis's Datalog fragment, but without the annotation algebra.
- **De Lore Is** queries are Datalog goals evaluated against the symbolic
  fixpoint. Every answer row carries its full provenance polynomial, so a query
  answers not just *what* is true but *under which authorial events* and *at
  what grade of evidence*. The `--why` flag renders minimal-witness blame. The
  `--fork` flag evaluates under a hypothetical valuation with no re-computation
  of the fixpoint. The cost: no ad-hoc SQL-style aggregation, no full-text
  search, and the Datalog fragment is deliberately restricted (stratified,
  range-restricted, no function symbols) to keep the fixpoint finite.

### 2.3 Schema evolution & migration

| System | Schema definition | Migration story | Breaking changes |
|---|---|---|---|
| SMW | Property/category wiki pages | Edit the page; old data silently loses its type | Data may become untyped silently |
| Cargo | Template field declarations | Edit template + `recreateData.php` | Columns vanish; data must be re-populated from pages |
| Wikibase | Properties as entities (datatype, constraints) | Add new properties freely; constraints are soft | No enforced breaking changes possible — anything goes |
| Datomic | Attributes as entities (type, cardinality) | New attributes are free; existing attributes are immutable | Cannot change attribute type; must add new + migrate |
| De Lore Is | AST JSON Schema (generated from Rust types) | Formatter is the migrator (canonical→canonical rewrite) | Grammar version bump; old text re-formatted into new form |

**Strengths and weaknesses:**

- **SMW/Cargo** schema changes are easy (edit a template or property page) but
  dangerous — a type change can silently orphan existing data. There is no
  migration tooling; the wiki simply stops recognizing old-shaped data.
- **Wikibase** avoids the problem by never enforcing schema: any item can carry
  any property. This is maximally flexible but means "schema evolution" is
  really "nobody ever has to evolve" — there is no coherent schema to evolve
  *from*. Constraint violations are advisory, not blocking.
- **Datomic** takes the append-only position: you can never change an
  attribute's type; you add a new attribute and write a migration that copies
  data. This is safe but operationally heavy for large schema changes.
- **De Lore Is** makes the formatter the migrator: a grammar version bump is
  shipped as a canonical-to-canonical surface rewrite, so upgrading a corpus is
  running the new formatter over the old files. *Data* never migrates — facts are
  never rewritten; they are superseded (new claims added, old claims die into
  lineage). The wire contract is explicitly versioned with digest-checked
  vendoring. The cost is that surface-level changes require a new formatter
  release, and there is no ad-hoc schema extension by end users — the grammar is
  closed.

### 2.4 Invariant enforcement & consistency guarantees

| System | Invariants | Enforcement point | Failure mode |
|---|---|---|---|
| SMW | None beyond MySQL types | — | Inconsistent data silently persists |
| Cargo | MySQL column constraints (type, NOT NULL) | On `recreateData` rebuild | Violations possible between rebuilds |
| Wikibase | Soft constraints (property constraints, SHACL) | Advisory (shown in UI, not blocking) | Violations flagged but stored |
| Datomic | Attribute types + transaction functions | At transaction time (ACID) | Transaction rejected |
| De Lore Is | Total gate: FD checks, cycle detection, C-constraints, range restriction, status algebra | At compile/check time (before anything is assertable) | Finding raised; claim cannot reach "certain" standing |

**Strengths and weaknesses:**

- **SMW/Cargo** rely on the underlying RDBMS for type safety and have no
  domain-level invariant checking. A wiki page can assert contradictory facts
  and nothing blocks the save. The wiki community polices consistency manually
  or via bots that flag violations after the fact.
- **Wikibase** adds constraint definitions as data (e.g. "instance of: human"
  should not also be "instance of: city"), but enforcement is advisory — the UI
  shows a warning, the edit is never blocked. Constraint violations are
  addressed socially, not mechanically.
- **Datomic** provides ACID transactions with custom transaction functions that
  can enforce arbitrary invariants at write time — a rejected transaction never
  lands. This is strong but requires the invariant to be expressible in Clojure
  transaction functions, which is procedural rather than declarative.
- **De Lore Is** enforces invariants declaratively at the algebra level. A
  fact's standing (certain / possible / dead) is a *theorem* of its provenance
  polynomial under the current valuation and constraint theory. Functional
  dependencies on slots, timeline acyclicity, negative-constraint membership,
  and the range restriction are all checked as properties of the compiled
  fixpoint. Nothing is "saved and checked later" — the gate is total, and a
  claim with `P⁺ = 0` is structurally excluded from every layer. The cost is
  rigidity: you cannot jot a half-formed thought without it being at least
  `proposed` (though `proposed` is a legal incomplete state).

### 2.5 Concurrency, scaling & runtime performance

| System | Concurrency model | Typical scale | Bottleneck |
|---|---|---|---|
| SMW | Web-request parallelism (PHP + MySQL) | Millions of triples (e.g. large fandom wikis) | MySQL query performance; job queue for rebuilds |
| Cargo | Same as SMW | Tens of thousands of rows per table typical | MySQL; `recreateData` is a batch lock |
| Wikibase | Blazegraph + MediaWiki parallel readers | Billions of triples (Wikidata: ~17B) | SPARQL query timeouts; single-writer Blazegraph |
| Datomic | Peer reads + single transactor writes | Billions of datoms | Transactor throughput (~10K writes/sec) |
| De Lore Is | Single-threaded, deterministic, offline | Thousands to tens of thousands of atoms | Fixpoint computation (milliseconds); no parallelism |

**Strengths and weaknesses:**

- **SMW/Cargo** inherit LAMP-stack concurrency: many readers, one writer
  (MySQL), with Apache/nginx handling parallel HTTP requests. This scales well
  for read-heavy wiki workloads. Write contention is rare because wiki edits
  are infrequent relative to reads.
- **Wikibase** scales to Wikidata's billions by separating the SPARQL query
  service from the editing path. But the query service has a hard timeout
  (60 seconds on Wikidata), and Blazegraph is a known operational burden.
- **Datomic** separates reads from writes architecturally: peers cache and
  query locally, the transactor serializes writes. This gives excellent read
  scalability but bounds write throughput to a single serialized transactor.
- **De Lore Is** is designed for a corpus of hundreds of decisions and thousands
  to tens of thousands of atoms — a single fiction world, not a collaborative
  encyclopedia. It is *deliberately* single-threaded and deterministic (no async,
  no parallelism, no network) because the gate depends on reproducibility. The
  fixpoint runs in milliseconds at this scale. The cost is obvious: it does not
  scale to millions of concurrent editors or billions of facts. It is not
  trying to. The workspace-digest fencing provides optimistic concurrency for
  multiple agent sessions against one server process, but the server itself is
  sequential.

### 2.6 Change management & provenance

| System | Change tracking | Provenance granularity | Undo/revert model |
|---|---|---|---|
| SMW | MediaWiki page revisions | Per-page (all triples on a page share one edit) | Page rollback (all-or-nothing per page) |
| Cargo | Page revisions (data rebuilt from templates) | Per-page | Page rollback + `recreateData` |
| Wikibase | Per-statement edit history | Per-claim (each statement edit is tracked) | Per-edit undo; granular |
| Datomic | Transaction log (every datom stamped) | Per-datom, per-transaction | `as-of` time-travel; excision for GDPR |
| De Lore Is | Git history + append-only decision ledger + polynomial support | Per-clause of a decision entry | Supersession (old claims die into lineage; nothing is deleted) |

**Strengths and weaknesses:**

- **SMW/Cargo** conflate structured-data provenance with page-edit provenance.
  If a template adds ten facts, they share one revision entry. Attributing a
  specific claim to a specific editorial decision is manual forensics on the
  page diff.
- **Wikibase** is the strongest of the wiki tools here: each statement carries
  its own references (source citations), and each edit to a statement is a
  separate revision. Provenance is *modeled* in the data (references) and
  *tracked* in the history (revisions). The weakness is that references are
  optional and socially enforced — there is no structural mechanism preventing
  unsourced claims.
- **Datomic** gives per-datom transaction attribution and full time-travel, so
  any past state is recoverable. But "provenance" here means "which transaction
  asserted this datom" — it does not model *why* or *on whose authority*. That
  semantic layer must be built on top.
- **De Lore Is** makes provenance the core data structure. A claim's support
  polynomial literally *is* its provenance — the combination of authorial
  events under which it holds. Status is derived from provenance, so the
  citation shown to a reader and the formula that gates assertability are the
  same object and cannot drift apart. Supersession preserves full lineage: the
  old claim is dead but visible in history, the new claim cites what it
  supersedes. The cost is ceremony — every claim needs a provenance marker, and
  "just write it down quickly" means at minimum writing it as `proposed`.

### 2.7 Inference & discovery of new implications

| System | Inference capability | Discovery pattern |
|---|---|---|
| SMW | Transitive category/subproperty only | Manual SPARQL-style querying |
| Cargo | None | SQL queries; manual discovery |
| Wikibase | SPARQL property paths (transitive closure) | Federated SPARQL; community queries |
| Datomic | Full recursive Datalog rules | Rules as data; recursive queries |
| De Lore Is | Annotated Datalog fixpoint + fork diffs | Suppose → diff shows all newly-certain/dead atoms |

**Strengths and weaknesses:**

- **SMW/Cargo** offer no real inference. If you want "all places reachable from
  X," you write a bot that computes transitive closure and stores the results
  as explicit triples. Discovery is manual querying.
- **Wikibase** provides transitive closure via SPARQL property paths (`wdt:P31/wdt:P279*`),
  which covers type hierarchies and part-of chains. But general recursive rules
  are not supported, and there is no mechanism to propagate implications when
  data changes — SPARQL is pull-only.
- **Datomic** supports full recursive Datalog rules, so derived facts are
  computed at query time. Rules can express reachability, transitive closure,
  and arbitrary fixed-point computations. Discovery is writing rules and
  querying. The weakness: rules are not annotated with provenance, so a derived
  fact does not automatically carry *why* it was derived.
- **De Lore Is** computes a full symbolic fixpoint at compile time: every
  derivable consequence is materialized with its full provenance polynomial.
  The fork-diff mechanism is the discovery tool — suppose a variable true and
  the system shows exactly which atoms become certain, which die, and which
  change grade. Ratification pre-flight answers "what would break if I ruled
  this?" before the ruling happens. The annotation algebra means derived facts
  automatically carry the provenance of every rule and premise they passed
  through — a consequence of a proposed rule is itself merely possible, as a
  theorem, not as a hand-maintained annotation. The cost: the fixpoint is
  computed eagerly (not lazily on query), which is fine at corpus scale but
  would not scale to billions of facts.

### 2.8 Multi-user / multi-authority suitability

| System | Authority model | Conflict resolution | Access control |
|---|---|---|---|
| SMW | Wiki consensus (any editor, any page) | Edit conflicts resolved by last-write-wins | Wiki permissions (page protection, user groups) |
| Cargo | Same as SMW | Same | Same |
| Wikibase | Wiki consensus + bot patrols | Edit-level undo/revert; community governance | Autoconfirmed, admin, etc. |
| Datomic | Application-defined | ACID transactions; application logic resolves | Application-level (Datomic has no user model) |
| De Lore Is | Single sovereign authority (v1) | No multi-user conflict — one authority rules | Single writer per commit; multi-authority is DEF-4 |

**Strengths and weaknesses:**

- **Wiki systems (SMW, Cargo, Wikibase)** are built for collaborative editing.
  Thousands of users can contribute simultaneously, with social norms and
  automated patrols (bots, ORES) enforcing quality. The weakness is that
  "consensus" is informal — there is no formal mechanism for declaring whose
  word is authoritative on a given claim. Edit wars are resolved socially, not
  algebraically.
- **Datomic** is agnostic — it provides transactions and the application builds
  whatever authority model it needs. This is maximally flexible but provides
  nothing out of the box.
- **De Lore Is** is explicitly single-authority in v1: one sovereign whose
  valuations are the world. The annotation algebra *could* support
  multi-authority (per-author event variables, merge policies over valuations),
  and this is explicitly deferred to DEF-4. The cost for wiki-style use is
  clear: collaborative worldbuilding — writers' rooms, shared universes — is
  not supported without extending the valuation regime. The benefit is equally
  clear: with one authority there are no edit wars, no consensus disputes, and
  the provenance chain is unambiguous.

### 2.9 LLM / agent integration

| System | Agent surface | Machine authorship model | Safety boundary |
|---|---|---|---|
| SMW | MediaWiki API (edit pages) | No distinction from human edits | Bot flags; page protection |
| Cargo | MediaWiki API | Same | Same |
| Wikibase | Wikibase API + bot framework | Bot edits flagged but structurally identical | Bot flags; ORES quality scoring |
| Datomic | Application code (Clojure/Java) | Application-defined | Application-defined |
| De Lore Is | Door T: 19 MCP-registered semantic tools | Agents propose only; machine never writes canon | Four-class output typing; graded findings; digest fencing |

**Strengths and weaknesses:**

- **Wiki systems** have no semantic boundary between human and machine edits. A
  bot's edit is structurally identical to a human's; the "bot" flag is metadata,
  not an enforcement mechanism. An LLM writing via the MediaWiki API can assert
  anything a human can — there is no "proposed" staging layer or mandatory
  review gate. The community must trust the bot operator.
- **Wikibase** adds quality-scoring (ORES) and structured references, which
  helps bots add well-sourced data, but the enforcement is still social — a bot
  can add an unreferenced claim and nothing structurally prevents it from
  becoming "accepted."
- **Datomic** provides no agent surface at all; everything is application code.
  An LLM integration would be bespoke per application.
- **De Lore Is** is architecturally designed for LLM agents as first-class
  co-authors with a hard safety boundary. Door T provides semantic tools (not
  raw text editing) — the agent names an *intent* ("add a person," "set a
  trait") and the server handles lowering, checking, and rendering. Every agent
  contribution enters as `proposed` and cannot mint authority. Model output is
  classified into four declared types (attestation, candidate, encoding, gloss),
  each with a specific kernel surface and verification story. The workspace
  digest provides optimistic concurrency fencing so multiple agents cannot
  silently clobber each other. The cost: the agent cannot do anything the tool
  surface does not expose, so novel operations require extending the tool set.
  The benefit: the machine *structurally cannot* write canon, and every
  agent-authored claim carries graded evidence of how well it was verified.

---

## 3. Suitability assessment

### Media lore wikis (Wookieepedia, Memory Alpha, Tolkien Gateway)

**Best fit: Wikibase** (or SMW/Cargo for smaller wikis). These are
collaborative, high-traffic, open-contribution projects where:
- Thousands of editors contribute; multi-user is non-negotiable.
- Data scale reaches millions of claims.
- "Good enough" provenance (edit history + optional references) suffices.
- The web presentation layer is integral, not orthogonal.
- Inference needs are modest (category hierarchies, property inheritance).

**De Lore Is** is unsuitable here in v1: single-authority, single-threaded, no
web layer, no multi-user. Its provenance algebra and inference engine are
dramatically stronger than what these wikis need or use, and its scaling
characteristics are wrong for the workload.

**Datomic** could technically serve this role (and has been used in content
management) but lacks the wiki social layer entirely — you would be building
from scratch.

### Cards and games (MTG databases, board-game variant tracking)

**Best fit: Cargo or Datomic**, depending on scale. Cargo is pragmatic for
wiki-hosted card databases (simple relational tables, SQL queries for deck
searches). Datomic suits applications that need variant tracking with
time-travel (historical card rulings, ban-list evolution).

**De Lore Is** is strong here if the game is *authored* rather than *catalogued*.
Its practice system models a game's laws, costs, and failure modes as data,
with invariant checking (resource conservation, rule legality) enforced at the
algebra level. Fork diffs show what changes under a house rule. The weakness is
that it does not serve as a card-search database for thousands of users — it is
an authoring and verification tool for the game's *design*, not a lookup service
for its *players*.

### Single-author worldbuilding (a novelist, a showrunner, a game designer)

**Best fit: De Lore Is.** This is its designed-for use case. One sovereign
authority, a corpus of hundreds to thousands of claims, provenance tracking
against editorial decisions, fork-based exploration of what-if scenarios,
LLM agents as co-authors under review, and a total gate that catches
contradictions before they propagate. The single-threaded deterministic model
is a *feature* here — reproducibility, offline use, no infrastructure.

**Datomic/DataScript** could serve this role if you build the domain layer
yourself, but you would be reimplementing the provenance algebra, the grade
lattice, the fork-diff mechanism, and the agent boundary from scratch.

**Wiki tools** are overkill in infrastructure and underkill in semantics — they
give you a web UI and multi-user you do not need, while lacking the invariant
enforcement and provenance you do.

### Collaborative worldbuilding (writers' rooms, shared universes, TTRPGs)

**Best fit: Wikibase** (for large open collaborations) or **Datomic with a
custom authority layer** (for smaller structured teams).

**De Lore Is** acknowledges this gap explicitly (DEF-4): the annotation algebra
is *indifferent* to multi-authority — you could mint per-author event variables
and define merge policies over valuations — but the valuation regime and
promotion workflow are built for one sovereign. The extension is architecturally
possible but not implemented. A TTRPG with one GM fits today (the GM is the
single authority); a shared universe with multiple equal authors does not.

---

## 4. Synthesis: where De Lore Is sits in the design space

De Lore Is occupies a position no existing system fills:

```
                    ┌─────────────────────────────────────────────────┐
                    │          provenance-first reasoning             │
                    │          (derived status, not stored)            │
                    ├─────────────────────────────────────────────────┤
  Datomic family:   │  immutable log, datalog, time-travel,           │
  shares →          │  append-only, EAV                               │
                    ├─────────────────────────────────────────────────┤
  Wiki backends:    │  multi-user, web-native, schema-light,          │
  shares →          │  domain-typed structured data for fiction        │
                    ├─────────────────────────────────────────────────┤
  Unique to DLI:    │  annotation algebra (PosBool provenance),       │
                    │  status-as-derivation, agent safety boundary,   │
                    │  fork-as-valuation, the machine never writes    │
                    │  canon, single-authority by design               │
                    └─────────────────────────────────────────────────┘
```

**What it trades away:**
- Multi-user / collaborative editing (v1)
- Web presentation layer
- Horizontal scaling / high-throughput concurrent access
- Ad-hoc schema extension by end users
- SQL-style aggregation and full-text search
- Operational simplicity of a hosted wiki

**What it gains:**
- Provenance is the data structure, not an afterthought — status cannot drift
  from citation
- Invariants are theorems, not guards — fail-closed is structural
- Inference propagates provenance automatically — a derived fact carries its
  full derivation history
- What-if exploration is O(1) — forks are valuations, not store copies
- LLM agents have a hard safety boundary — the machine structurally cannot
  write canon
- Schema evolution is a formatter run, not a data migration
- The entire system is deterministic, offline, and reproducible — no
  infrastructure, no network, no clock
- Change management is append-only supersession with full lineage — nothing is
  ever silently lost

**The architectural bet:** that for fiction worldbuilding — where one authority
rules, provenance matters, contradictions must not propagate silently, and
LLM agents are increasingly doing the drafting — trading away multi-user scale
for algebraic rigor is the right trade. The wiki tools made the opposite bet,
and they are right for their population. De Lore Is is the third family: not a
runtime, not a flow engine, not a wiki — the authoritative knowledge layer that
feeds all of them.
