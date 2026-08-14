---
status: proposed
verdict: "Characterizes the indexing strategy from embedded fixpoint materialization to persistent archive readers. Supplementary Magic Sets bound peer clusters under DL-0062; argumentation, uncertainty, and trust remain exploratory."
---

# Lorestore Indexing Strategy: From In-Memory Fixpoints to Persistent Indices

> **Sequenced by `design-notes/lorestore-roadmap.md` (DL-0041).** This note's
> Phases 2–4 are the **committed archive-tier track** — load-bearing for the
> encyclopedic multi-continuity corpora the engine is aimed at (creature
> catalogues, comics multiverses, canon-and-fanon authorities) — entered on
> the measured triggers in the roadmap's §6. The cutover itself starts at
> Phase 1 only because the consuming project is atelier-tier (10³–10⁵ facts),
> where in-memory saturation outruns any index; the tier boundary is crossed
> by adding index readers, never by migrating stored facts. Phases 5–7 remain
> exploratory. The roadmap wins where they differ.

## Current State: Precomputed Fixpoint + Sparse Indexes

The current implementation uses an **eager materialization** strategy for query
performance. During `dli_world::compile`, the system computes the complete
fixpoint — all derived facts, their statuses, grades, and support polynomials —
and stores the result in memory.

### What Gets Precomputed

1. **Fixpoint materialization** — The complete closure of all Datalog rules
   over the asserted facts, with three-valued status and grade assignment for
   each derived atom. This is stored in `WorldModel::derived`.

2. **Support index** — Maps each atom (fact) to:
   - Which clauses/rules can derive it (forward dependency)
   - Which atoms depend on it (backward dependency)
   - Enables **slice-based recomputation** when decision variables change

3. **Atom interning** — All entity names interned to `u32` indices via
   `Interner`, making polynomial operations fast (integer ops vs string matching).

### How This Enables O(1) Forking

When `World::fork()` is called:
1. Copy the `World` struct (cheap — mostly references)
2. Overlay a new `Valuation` (partial map of decision variables)
3. Mark the `support_index` for changed variables as "dirty"
4. Only the slice of derived facts that depend on those variables is recomputed

This is **not truly O(1)** — it's O(dependent_slice) — but the dependent slice
is typically small because most facts don't depend on most decision variables.

### Query Engine Current State

Very basic. The `query` function in `dli-eval/src/lib.rs` does pattern matching
over the materialized fixpoint — no indexes beyond the fixpoint materialization
itself. Query is essentially:
```rust
pub fn query(world: &World, patterns: &[Pattern]) -> Vec<BindingSet> {
    // Just scans the derived set for matches
}
```

## The Fundamental Shift: From Memory to Persistence

Moving to lorestore fundamentally changes indexing because:

1. **Facts live in object storage**, not in-memory maps
2. **Queries must span forks/revisions** — not just a single compiled world
3. **Multiple readers** need to access the same store without locking
4. **Incremental writes** need to update persistent indexes

The current approach of "compute everything at compile time" doesn't work when
facts persist across sessions and can be queried before full recompilation.

## Lorestore Indexing Roadmap

### Phase 1: Basic Persistent Storage (MVP)

**Goal:** Store and retrieve individual facts by content hash, with basic
branch management.

**Indexes:**
- **Object index**: `sha256(content)` → serialized fact object (content-addressed)
- **Branch refs**: `refs/heads/<name>` → root commit hash
- **Commit log**: Append-only log of fact writes per branch

**Write path:**
1. Serialize fact deterministically (JCS canonical JSON)
2. Compute content hash
3. Append to object store
4. Update branch ref (compare-and-swap)

**Query path:**
1. Read branch ref → root hash
2. Fetch commits from log
3. Fetch individual fact objects

No secondary indexes yet — queries require full scan. This is sufficient for
basic fact retrieval but not for efficient query processing.

### Phase 2: Essential Secondary Indexes

**Goal:** Enable efficient query processing for the current Datalog workload.

**Indexes:**
- **Entity index**: `entity_id` → sorted list of fact hashes (per branch)
- **Predicate index**: `predicate_name` → sorted list of fact hashes
- **Citation index**: `decision_id` → list of citing fact hashes
- **Polynomial variable index**: `event_variable` → list of fact hashes mentioning it

**Write path additions:**
- For each fact, update all relevant secondary indexes atomically
- Index entries are sorted for efficient range scans
- Indexes are versioned per branch (fork = new index view)

**Query path improvements:**
- Entity lookup: O(log n) instead of O(n) full scan
- Predicate scan: O(log n + k) where k = matching facts
- Impact analysis ("what changes if D-0147 flips"): O(k) via polynomial index

**Value delivered:** Makes lorestore viable as a persistent replacement for the
current git+text model. Enables O(log n) entity and predicate lookups, which is
sufficient for most DLI queries.

**Dev lift: Low-Medium.** Indexes are straightforward B-trees or LSM trees over
the basic object store. The challenge is keeping them consistent with branch
operations.

### Phase 3: Incremental Compilation Indexes

> **Status: entered (DL-0087), ahead of Phase 2, on the continuous-ingestion
> trigger.** v1 is an in-process rebuildable compiled-world cache + an incremental
> fixpoint reusing the prior saturation (recompute above the affected stratum); the
> **persistent on-disk** indexes below and a **sliced check battery** are deferred.
> Safety is a permanent gate-resident oracle: incremental compile equals full
> recompile byte-identically under every addition ordering.

**Goal:** Avoid recomputing the entire fixpoint when new facts are added or
decision variables change.

**Indexes:**
- **Dependency graph index**: `fact_hash` → {forward_deps, backward_deps}
- **Stratification index**: Facts organized by Datalog strata
- **Dirty set tracking**: Which atoms are affected by a branch valuation change

**Write path additions:**
- When a fact is written, compute its dependency footprint
- Mark dependent facts as "needs recompile" in the dirty set
- Compile only the dirty slice (reuses current `support_index` logic)

**Query path improvements:**
- Only recompile affected facts — rest of fixpoint stays materialized
- O(|affected_slice|) instead of O(|entire_fixpoint|)

**Value delivered:** Makes large lorestores practical. Without this, every new
fact triggers a full recompilation.

**Dev lift: Medium-High.** Requires porting the current in-memory
`support_index` logic to persistent storage, and handling the complexity of
incremental fixpoint maintenance across branch boundaries.

### Phase 4: Cross-Branch Query Indexes

**Goal:** Efficient queries that span multiple forks/branches simultaneously.

**Indexes:**
- **Branch ancestry index**: Parent-child relationships between branch heads
- **Cross-branch fact resolution**: For a given entity+predicate, all values
  across all visible branches
- **Conflict detection index**: Facts that differ between branches on the same
  entity+predicate

**Value delivered:** Enables "compare these two forks" and "merge these branches"
operations efficiently.

**Dev lift: High.** Requires a new query evaluation model that doesn't assume
a single valuation context.

### Phase 5: Argumentation Framework Indexes (Aspirational)

**Goal:** Support ASPIC+-style argumentation with attack relations.

**Indexes:**
- **Argument index**: `argument_id` → {claim, supporting_facts, attacking_arguments}
- **Attack relation index**: `(arg_a, arg_b)` → {condition, strength}
- **Preference ordering index**: Authority trust weights and preference relations
- **Acceptability set index**: Precomputed grounded/preferred extensions

**Value delivered:** Enables contested canon, fan theory debates, multi-authority
conflict resolution.

**Dev lift: Very High.** Requires rethinking the evaluation model from
"true/false/dead" to "in/out/undecided" over argument graphs.

### Phase 6: Uncertainty Propagation Indexes (Aspirational)

**Goal:** Support probabilistic/confidence-weighted assertions.

**Indexes:**
- **Confidence interval index**: Per-fact [low, high] confidence bounds
- **Authority trust weight index**: Per-authority trust scores
- **Joint distribution index**: Correlation structure over related facts
- **Sensitivity analysis index**: How confidence propagates through the network

**Value delivered:** Realistic modeling for messy, contested knowledge.

**Dev lift: Very High.** Requires extending the truth value space beyond
boolean and modifying the join/sup operations in the fixpoint.

### Phase 7: Fuzzy Entity Resolution Indexes (Aspirational)

**Goal:** Handle probabilistic entity matching across sources.

**Indexes:**
- **Entity similarity index**: All pairwise similarity scores between entities
- **Equivalence class index**: Clustering of entities under probabilistic merging
- **Counterfactual cluster index**: What if we merged entity A with entity B?

**Value delivered:** Handles the reality that different sources use different
names for the same entity.

**Dev lift: Very High.** Requires approximate nearest neighbor structures and
probabilistic equivalence relation reasoning.

## Prior Art in Persistent Datalog Indexing

### Differential Datalog (DDLog)
The industry standard for incremental view maintenance. Instead of
recomputing entire fixpoints, DDLog computes only the *delta* when inputs change.
This directly informs Phase 3 — our current `support_index` is a hand-rolled
version of DDLog's differential work.

**Key insight:** Study DDLog's indexing strategies for incremental evaluation
before building our own.

### Semi-Naive Evaluation with SIP
"Sideways Information Passing" strategies optimize join processing in
Datalog evaluation. These techniques are relevant for the query engine in
Phases 2-3.

### Materialized View Maintenance (Academic Literature)
- "Incremental Maintenance of Views with Duplicates" (BRJS 1996)
- "The Differential Fixpoint" (Botangui et al.)
- "Lattice-based Quantifier Elimination" (Beyene et al.)

These provide formal foundations for the incremental compilation index (Phase 3).

### MVCC for Immutable Systems
Traditional MVCC assumes row updates. For immutable, content-addressed stores:
- **Delta encoding** between versions
- **CRDT-inspired merge** for concurrent writes
- **Version vectors** for causality

Git's delta compression and IPFS's Merkle DAG techniques are directly relevant.

## What This Preserves

Despite evolving the indexing strategy, three principles remain:

1. **O(1) fork creation** — Branch creation must remain cheap regardless of
   index sophistication
2. **Polynomial provenance** — Every fact still has a support polynomial;
   indexes serve the polynomial model, not replace it
3. **Possible worlds without recompile** — Forks create lightweight views,
   not independent database instances

## Adjacent Topics to Understand

1. **IPLD Schema** — How typed schemas layer on immutable content-addressed
   data (relevant for lorestore's type evolution)

2. **CRDTs for knowledge merge** — Commutative replicated data types for
   multi-writer scenarios (relevant for Phase 4+)

3. **Probabilistic Data Structures** — Bloom filters, Count-Min Sketch,
   HyperLogLog for memory-efficient approximate indexes (relevant for large
   lorestores)

4. **Graph Database Indexing** — Adjacency list vs matrix tradeoffs, subgraph
   isomorphism indexing (relevant since our fact model is a labeled property graph)

5. **Columnar Storage for Facts** — Parquet/Family stores for efficient
   analytical queries over fact attributes (predicate, grade, status)

## Recommendation: Start with Phase 2

> **Superseded for the continuous-ingestion trigger (DL-0087).** This recommendation
> assumed a full-scan-query pressure (Phase 2's trigger) would arrive first. The
> SubtleMemory pilot's continuous ingestion made Phase 3's trigger live *first* — the
> real-world usage pattern this section asked to wait for — so **Phase 3 is entered
> ahead of Phase 2**. Phase 2 remains the next item after it. The rest of this
> section stands for the general case.

Phase 2 (essential secondary indexes) delivers the most value for the least
complexity. It makes lorestore viable as a persistent store and enables
efficient queries that the current git+text model can't provide.

Phases 3+ should be deferred until we have real-world usage patterns to inform
the indexing decisions. Premature optimization on indexing strategies is
expensive and likely to be wrong.

The aspirational phases (5-7) are legitimate future directions but represent
entirely new engineering efforts — argumentation frameworks, probabilistic
reasoning, and entity resolution are each PhD-level problems. They should be
tracked as future work, not planned for near-term implementation.
