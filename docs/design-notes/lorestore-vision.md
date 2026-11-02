---
status: accepted
verdict: "Lorestore is deloreis's embedded, deterministic storage engine; distribution research is parked."
---

# Lorestore: deloreis's deterministic storage layer

> **Ratified and sequenced.** DL-0041 accepted this direction; the staged,
> dependency-ordered plan — including design refinements such as splitting
> fact identity three ways (content-hash versions, minted claim ids, and
> provenance-bearing lineage edges in place of a bare `parent` hash) — is
> `design-notes/lorestore-roadmap.md`, which consolidates this note and wins
> where they differ.

> **Embedded core shipped; distribution is deferred.** Lorestore's local
> content-addressed core is live. The former consumer cutover is retired;
> `scale-and-distribution.md` records the later peer/service
> destination and does not alter the current offline worker.

## Vision

Lorestore stores structured facts as immutable, content-addressed objects.
Legacy `.dli` inputs and evidence directories remain compatibility and consumer
cutover concerns; they are not the normative store model.

**Lorestore** is the vision for deloreis's next storage layer: a deterministic,
immutable, content-addressed fact store with git-like merge semantics, embedded
directly as deloreis's storage engine (like SQLite is an application's storage
engine, not a separate deployment).

The consuming application embeds deloreis, which embeds
lorestore. The application provides **where** lorestore stores data (a path, a cloud
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
│  Consuming Application                                  │
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
   immutable. RFC 8785 JCS canonicalization for all structured data. Facts stored
   as structured JSON objects, not serialized `.dli` text.

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
   - Polynomial variable → all facts mentioning that variable (for impact
     analysis when decisions change)

   > **See `lorestore-indexing-strategy.md`** for the phased roadmap of index
   > evolution from basic content-addressed storage to incremental compilation
   > and cross-branch query indexes, with aspirational targets for argumentation
   > frameworks and uncertainty propagation.

5. **Evidence subsystem** — Same as today's `.dli-evidence/` but integrated:
   attestation batches, payload records, simulation manifests, all
   content-addressed and append-only.

6. **Transaction model** — Single-writer per lorestore instance (serialized
   through the advisory lock, like today). Atomic ref updates
   (compare-and-swap on branch roots).

7. **Authority tagging** — While the current v1 model assumes single sovereign
   (DL-0002), lorestore's fact objects include an `authority` field to support
   the multi-authority evolution described in
   `docs/design-notes/multi-authority-knowledge-evolution.md`. Single-authority
   deployments leave this field unset; multi-authority setups populate it with
   authority identifiers and trust weights.

> **Important note on `.dli` files:** The lorestore-vision document originally
> described `.dli` as the human review surface. This is evolving — see
> `docs/design-notes/dli-deprecation-plan.md`. In the lorestore model, `.dli`
> files are not required for storage, agent interaction, or human review.
> Facts live as structured JSON objects; query APIs return JSON; human review
> happens via web UI. `.dli` becomes an optional pretty-print rendering, like
> CSV export from a database. The phased cutover plan is documented in that
> note.## Comparison: git + text vs. lorestore

| Capability | Git + `.dli` + `.dli-evidence/` | Lorestore |
|---|---|---|
| Fact storage | Text files parsed by deloreis | Content-addressed JSON fact objects |
| Fork creation | `git branch` (O(1) but full tree) | Branch + valuation overlay (true O(1)) |
| Merge semantics | Line-based diff | Provenance-polynomial-aware 3-way merge |
| Conflict detection | Parse + re-load to find conflicts | Structural conflict detection on write |
| Query efficiency | Re-parse + re-load | Indices over structured facts |
| Agent writes | Agent writes `.dli` files to disk | deloreis writes directly to lorestore |
| Human review | Git diff of `.dli` text files | Web UI over JSON query results |
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
for the former migration. This phase is historical.

**Phase 2 (Post-cutover)** — Introduce lorestore as an alternative backend.
deloreis can run with either the git+text backend (for existing projects) or
the lorestore backend (for new deployments). Configuration-driven. Query APIs
return JSON alongside `.dli` rendering. Web UI available for human review
alongside git-diff review.

**Phase 3 (Post-migration stabilization)** — Make lorestore the default and sole
backend. The git+text path becomes a migration tool (import existing `.dli`
into lorestore format). Query APIs return JSON by default; `.dli` becomes
opt-in. Web UI is the primary review surface.

**Phase 4 (DLI deprecation)** — `.dli` files become optional pretty-print
convenience only. `author.materialize` returns JSON by default. `.dli`
rendering available via explicit opt-in. See
`docs/design-notes/dli-deprecation-plan.md` for the full staged cutover plan.

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
| The formatter | Fact rendering engine | Renders structured facts to `.dli` as one of multiple possible output formats (JSON, HTML, etc.); `.dli` is no longer the canonical view — see `docs/design-notes/dli-deprecation-plan.md` |

## Open questions

1. **Storage backends** — Do we support only filesystem, or also object storage
   (S3-compatible) and IPFS? What is the storage trait interface?

2. **Offline operation** — Does lorestore need to work in air-gapped
   environments (like the deterministic offline gate requires)?

3. **Size/bandwidth** — For large lore corpora, is full fact materialization
   always needed, or do we support lazy/partial loading?

> Detailed treatment of index design, phased evolution, and prior art is in
> `docs/design-notes/lorestore-indexing-strategy.md`. The deprecation plan for
> `.dli` files as storage/interchange format is in `docs/design-notes/dli-deprecation-plan.md`.
