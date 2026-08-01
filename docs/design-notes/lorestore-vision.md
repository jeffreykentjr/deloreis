---
status: accepted
verdict: "Architecture vision: Lorestore as deloreis's embedded, deterministic storage engine (like SQLite), replacing the git+text+evidence-v1 model for new deployments after fantasy cutover completes."
---

# Lorestore: deloreis's deterministic storage layer

> **A post-cutover architectural evolution, not a migration blocker.** This document
> captures the long-term vision for deloreis persistent storage. The current v1
> model (git + `.dli` text + `.dli-evidence/`) remains in force until the fantasy
> cutover is complete; this direction is realized only in Phase 2 or later.

## Vision

Today, deloreis stores facts in `.dli` text files in git, and evidence in
`.dli-evidence/` as immutable content-addressed JSON. This "git + text files"
approach is the **v1 implementation** — sufficient for the migration but not for
general deployment.

**Lorestore** is the vision for deloreis's next storage layer: a deterministic,
immutable, content-addressed fact store with git-like merge semantics, embedded
directly as deloreis's storage engine (like SQLite is an application's storage
engine, not a separate deployment).

The consuming application (e.g. fantasy) embeds deloreis, which embeds
lorestore. Fantasy provides **where** lorestore stores data (a path, a cloud
bucket, an IPFS repo); lorestore owns **how** the data is structured, merged,
and indexed.

## What lorestore solves that git + text does not

1. **Structured concurrency** — Multiple agents working on forks
   simultaneously, with deterministic merge semantics that understand
   provenance polynomials, not just line-based diffs.

2. **Truly O(1) fork creation** — Branch + valuation overlay only. No full
   tree copy or ref scan.

3. **Provenance-aware merging** — Git merges lines; lorestore merges *facts
   with their support polynomial dependencies*. A merge can detect that two
   forks contradict only on a proposed hypothesis, and produce the right
   three-valued status.

4. **Deterministic conflict detection** — The three conflict types
   (valuation disagreement, exclusivity violation, certain-layer
   contradiction) are computed structurally on write, not discovered by
   re-parsing text.

5. **Atomic, immutable writes** — Every fact write is content-addressed and
   atomic. The consuming app never sees partial state.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Consuming Application (e.g. fantasy)                   │
│  Provides: storage path / cloud bucket / IPFS           │
└────────────┬────────────────────────────────────────────┘
             │ embeds
┌────────────▼────────────────────────────────────────────┐
│  deloreis binary (MCP server + all dli-* crates)         │
│  - AST types, evaluator, fork manager, export engine     │
│  - Handles all MCP tool calls (author.*, dli.*)         │
└────────────┬────────────────────────────────────────────┘
             │ embeds
┌────────────▼────────────────────────────────────────────┐
│  lorestore (deloreis's storage engine, like SQLite)      │
│  - Content-addressed immutable fact store                 │
│  - Git-like branches with valuation overlays              │
│  - Provenance-aware merge engine                         │
│  - Secondary indices (entity→facts, ruling→facts, etc.)  │
│  - Evidence subsystem (attestations, simulations)        │
└─────────────────────────────────────────────────────────┘
```

## Lorestore's data model

Every fact in lorestore is a **deterministic, content-addressed object**:

```json
{
  "id": "sha256:...",
  "type": "fact",
  "predicate": "person",
  "arguments": ["<entity-id>"],
  "support": {"monomials": [["+<decision-id>", "+<hypothesis-id>"]]},
  "defeaters": {"monomials": [["+<defeater-id>"]]},
  "provenance": {
    "kind": "proposed",
    "origin": "agent:<origin-tag>"
  },
  "grade": "asserted",
  "parent": "sha256:..."
}
```

**Branches** are named pointers to root commit objects, with attached
**valuation overlays** (which ruling variables are true/false):

```
refs/heads/main → {root: sha256:abc, valuation: {<decision-id>: true, <hypothesis-id>: false}}
refs/heads/fork/<branch-name> → {root: sha256:def, valuation: {<decision-id>: true, <hypothesis-id>: true, <bundle-id>: true}}
```

**Merges** are 3-way merges computed over the provenance polynomial structure,
not over text lines.

## The MCP flow (post-lorestore)

```
Agent: author.add_entity(<entity-id>, person, "<Label>", proposed)
  →
deloreis serve:
  1. Validate against AST schema, compile provenance polynomial
  2. Write fact to lorestore (content-addressed)
  3. Update fork branch with new root + digest
  4. Return: {findings, rendered_lines, workspace_digest}
  →
Agent gets immediate feedback; lorestore is updated atomically
```

No separate agent layer needed to "write the rendered lines to disk" —
deloreis writes directly to lorestore as part of the tool call.

## What lorestore needs (technical requirements)

1. **Append-only, content-addressed object store** — SHA-256 addressed blobs,
   immutable. RFC 8785 JCS canonicalization for all structured data.

2. **Branch + valuation model** — Git-like refs, but each branch carries a
   valuation (partial assignment of event variables to true/false). Forks are
   lightweight branches with overlaid valuations.

3. **Merge engine** — 3-way merge over fact graphs, understanding:
   - Support polynomial conflicts (two facts whose support monomials are
     exclusivity-family violations when conjoined)
   - Valuation overlay conflicts (two forks ruling the same variable differently)
   - Certain-layer contradiction detection (functional dependency violations)

4. **Secondary indices** — Efficient lookup by:
   - Entity name → all facts about that entity
   - Predicate → all facts with that predicate
   - Ruling id → all facts citing that ruling
   - Hypothesis → all facts gated on that hypothesis

5. **Evidence subsystem** — Same as today's `.dli-evidence/` but integrated:
   attestation batches, payload records, simulation manifests, all
   content-addressed and append-only.

6. **Transaction model** — Single-writer per lorestore instance (serialized
   through the advisory lock, like today). Atomic ref updates
   (compare-and-swap on branch roots).

## Comparison: git + text vs. lorestore

| Capability | Git + `.dli` + `.dli-evidence/` | Lorestore |
|---|---|---|
| Fact storage | Text files parsed by deloreis | Content-addressed fact objects |
| Fork creation | `git branch` (O(1) but full tree) | Branch + valuation overlay (true O(1)) |
| Merge semantics | Line-based diff | Provenance-polynomial-aware 3-way merge |
| Conflict detection | Parse + re-load to find conflicts | Structural conflict detection on write |
| Query efficiency | Re-parse + re-load | Indices over structured facts |
| Agent-mediated writes | Yes (agent writes rendered lines) | No (deloreis writes directly) |
| Content addressing | File-level only (git blobs) | Fact-level + evidence-level |
| Concurrency | File locking, git merge conflicts | Advisory lock per writer, structural merge |

## Deployment model

Lorestore is **embedded** in the deloreis binary. The consuming application
configures:

- **Storage backend**: a filesystem path, or a cloud bucket (via a storage trait),
  or an IPFS repo
- **Concurrency model**: single-process (simplest) or multi-process with
  advisory file locking
- **Evidence path**: where attestation/payload/simulation evidence lives
  (defaults to `<store>/evidence/`)

There is **no separate lorestore server or deployment**. It is a library crate
that deloreis links to, providing the deterministic persistence layer.

## Phased realization

**Phase 1 (Current — v1)** — Git + `.dli` text + `.dli-evidence/`. Sufficient
for the fantasy migration. No changes needed.

**Phase 2 (Post-cutover)** — Introduce lorestore as an alternative backend.
deloreis can run with either the git+text backend (for existing projects) or
the lorestore backend (for new deployments). Configuration-driven.

**Phase 3 (Post-migration stabilization)** — Make lorestore the default and sole
backend. The git+text path becomes a migration tool (import existing `.dli`
into lorestore format).

## Why this is "barely a separate project"

Lorestore is deloreis's storage engine — like SQLite is Firefox's storage
backend. You don't deploy "a SQLite instance" separately; you ship the Firefox
binary and it has embedded storage.

Similarly:
- You deploy `deloreis serve` with a configured storage path
- deloreis embeds lorestore internally
- A consumer just tells deloreis where to store its facts
- All the merge semantics, indexing, and provenance tracking live in lorestore
  code

The only "external" aspect is the **storage path configuration** — everything
else is deloreis's internal business.

## Relationship to existing deloreis subsystems

| Existing subsystem | Becomes part of lorestore | Notes |
|---|---|---|
| `ForkStore` (`dli-eval/fork.rs`) | Branch management + valuation overlays | The in-memory fork model maps to lorestore branches |
| `.dli-evidence/` writer (`dli-serve/evidence.rs`) | Evidence subsystem | Content-addressed, append-only, becomes a lorestore module |
| `.dli-cache/` compiled world cache | Replaced entirely | Lorestore stores structured facts, not serialized fixpoints |
| `dli_world::compile` (loader + fixpoint) | Reads from lorestore instead of `.dli` files | The compile pipeline stays, but input source changes |
| The formatter | Fact canonicalization layer | Renders structured facts to canonical `.dli` for human review |

## Open questions

1. **Index design** — What secondary indices are needed for efficient queries?
   (Entity lookups, predicate scans, citation traversal, timeline queries)

2. **Merge complexity** — Is full git-like merge overkill, or do we need it
   for concurrent agent workflows? Simpler "latest write wins" with conflict
   detection might suffice for single-agent scenarios.

3. **Storage backends** — Do we support only filesystem, or also object storage
   (S3-compatible) and IPFS? What is the storage trait interface?

4. **Evidence migration** — Do existing `.dli-evidence/` contents migrate to
   lorestore format, or do they coexist?

5. **Offline operation** — Does lorestore need to work in air-gapped
   environments (like the deterministic offline gate requires)?

6. **Size/bandwidth** — For large lore corpora, is full fact materialization
   always needed, or do we support lazy/partial loading?
