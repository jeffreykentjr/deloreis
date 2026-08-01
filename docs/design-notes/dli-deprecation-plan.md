---
status: proposed
verdict: "The .dli text format is evolving from primary storage/interchange format to an optional pretty-print convenience. Human review shifts to web UI over JSON query results; agents consume JSON APIs. This document records the deprecation plan and staged cutover path."
---

# Deprecating `.dli` Files: From Canonical Format to Pretty-Print Convenience

> **Ratified and sequenced.** DL-0041 accepted this evolution; the staged plan
> is `design-notes/lorestore-roadmap.md` (this note's phases map onto LS-4,
> LS-5, and LS-7, with the web-UI dependency deliberately taken off the
> critical path). The roadmap wins where they differ.

## Current State

Today, `.dli` files serve three roles:
1. **Primary storage** — Facts stored as `.dli` text files in git
2. **Interchange format** — `author.materialize` returns `.dli` bytes as data
3. **Human review surface** — Humans review PR diffs of `.dli` file changes

These roles are artifact of the **v1 architecture**: human authors writing text files, agents generating text, review happening via git diffs of text.

## The Evolution

As deloreis evolves toward the lorestore model:
- **Storage** moves to content-addressed fact objects in lorestore
- **Agent interaction** happens via MCP tool calls with JSON payloads
- **Human review** happens via web UI calling query APIs
- **Everything** becomes a JSON interface

In this model, `.dli` files have **no essential role**:
- Not needed for storage (lorestore handles this)
- Not needed for agent interaction (JSON over MCP)
- Not needed for human review (web UI renders JSON)

## What `.dli` Becomes

A **pretty-print convenience** — one of several possible renderings of fact objects:
- `.dli` rendering (compact, sentence-like, familiar to current users)
- JSON rendering (structured, machine-friendly)
- HTML rendering (for web UI)
- Markdown/GrammarViz/Gateway/etc (for specific integrations)

The **canonical form** is the structured fact object in lorestore. `.dli` becomes an optional serialization, like how a database might offer CSV export — useful sometimes, but not the storage format.

## Staged Cutover Plan

### Phase 1: Dual-Mode (Current → Transition)
Both `.dli` files and lorestore operate simultaneously:
- `author.materialize` returns JSON by default, `.dli` as opt-in
- Web UI reads JSON query results, renders diffs for human review
- Existing `.dli` files still accepted as import source
- Fantasy migrates to JSON-based review flows

**Exit criterion**: No `.dli` files committed as primary world source

### Phase 2: JSON-Only Default
- `author.materialize` returns only JSON
- `.dli` rendering becomes an explicit opt-in (`--format dli`)
- Web UI is the only human review surface
- New projects use lorestore directly, no `.dli` import

**Exit criterion**: No code path requires `.dli` parsing for primary operations

### Phase 3: `.dli` as Pure Convenience
- `.dli` support moves to an optional crate/plugin
- `dli_syntax` crate becomes a "pretty printer" library
- No part of the core engine, MCP API, or web UI requires `.dli`
- `.dli` import/export available via CLI flag (`deloreis convert --from-dli`)

**Exit criterion**: `dli_syntax` is not linked by default

### Phase 4: Removal (Deferred)
- Remove `.dli` support entirely from core repos
- Maintain `.dli` tooling in a separate "legacy" repository
- All interfaces are JSON-based

## Impact Analysis

### Human Review Workflow Change

**Current (v1):**
```
Agent → author.materialize → .dli bytes → commit to git → human reviews text diff in PR
```

**Evolved:**
```
Agent → author.add_entity(...) → JSON response → fact written to lorestore
Human → web UI → query API → JSON diff → approve/reject in UI
```

### Agent Interface Change

**Current (v1):**
```
Agent → author.materialize → .dli bytes → write to filesystem
```

**Evolved:**
```
Agent → author.add_entity(...) → JSON response (direct write)
```

Agents never see `.dli` text. They call tools, get JSON responses, facts go directly to lorestore.

### Import/Export Implications

Since nothing else speaks `.dli`, we have flexibility:
- **Import**: Can accept JSON, CSV, RDF/Turtle, or other formats
- **Export**: Can emit JSON, CSV, or other formats (including `.dli` as one option)
- **Migration**: Fantasy moves from `.dli` import to JSON/lorestore import

## What Humans Review Instead

Humans review changes via web UI showing structured diffs:

```json
{
  "change_type": "fact_added",
  "fact": {
    "subject": "arthur",
    "predicate": "standing",
    "value": "king",
    "provenance": {"kind": "ratified", "decision": "d-0012"}
  },
  "diff": {
    "before": null,
    "after": {"standing": "king"}
  }
}
```

The web UI renders this into human-friendly representations (tables, lists, graphs) without requiring a text syntax.

## Query Interface Shape

Query responses return structured JSON that serves both humans and agents:

```json
{
  "query": "standing(arthur, X)",
  "results": [
    {
      "bindings": {"X": "king"},
      "provenance": {"kind": "ratified", "decision": "d-0012"},
      "grade": "attested",
      "human_readable": "Arthur Pendragon is King"
    }
  ]
}
```

This gives humans a readable summary and agents structured data — no `.dli` generation needed.

## Risks and Mitigations

### Risk 1: Loss of Familiarity
The `.dli` syntax is familiar to early adopters. Removing it as a default may cause friction.

**Mitigation**: Keep `.dli` as an opt-in rendering during transition. Document the equivalent JSON queries.

### Risk 2: Web UI Development Burden
The cutover depends on a functional web UI for human review.

**Mitigation**: The web UI can start simple (JSON display with expand/collapse) and evolve. The MCP tools already return JSON — the web UI is primarily a visualization layer over the same API.

### Risk 3: Reference Card Obsolescence
`docs/cards/` teach `.dli` syntax. If `.dli` becomes optional, these become less relevant.

**Mitigation**: Reference cards evolve to show equivalent JSON/MCP tool calls. The `.dli` card becomes "pretty-print mode."

## Relationship to AGENTS.md Principles

This evolution preserves the core principles:
- **AST-first** (DL-0003): The JSON schema remains normative; `.dli` was just one rendering
- **Deterministic offline gate** (DL-0013): Unchanged — gate checks JSON serialization, not `.dli`
- **Replayable judgments** (DL-0007): Enhanced — JSON queries are more directly replayable

It challenges some current conventions:
- **"Every fenced `.dli` example gate-checked"** (AGENTS.md §3): Needs updating — fence `json` instead
- **Formatter canonicalization** (AGENTS.md §2): Becomes one of many renderers, not THE canonical one

## Open Questions

1. **What's the canonical JSON schema for fact objects?** Need to formalize this before removing `.dli` as default
2. **What's the query API contract?** Need to define query response shapes precisely
3. **How does the web UI authenticate/authorize?** RBAC model for human review
4. **Migration story for existing `.dli` projects?** Import tool path, not just export
