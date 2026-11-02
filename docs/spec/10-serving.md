---
status: live
title: "Serving: one offline, deterministic worker surface"
verdict: "The JSON and MCP server for fictional worlds; contract 2.0.0 is the only served version."
---

# Serving

`deloreis serve --project <path>` serves newline-delimited JSON over standard
input and output. `deloreis mcp --project <path>` exposes the same operations
as MCP tools. The project path names a `deloreis.toml` manifest or its
containing directory. The server performs deterministic local evaluation; it
calls no model or network service.

Contract `2.0.0` is the only served version (DL-0101, DL-0103). Previously
served versions and retired operations are refused as killed forms, with the
successor named. The generated schemas and replay fixtures in
`contracts/dli-worker/` define the request and response shapes.

## Projects

A project manifest names logical documents. Each document uses either one or
more `.dli` source files on a syntax-enabled build, or a lorestore directory.
A store-backed document may pin a commit with `head`; without it the document
follows the main branch. This example names a store-backed document:

```toml
version = 1
name = "storybook"

[documents.world]
store = "lorestore/world"
```

A wire request names the logical document ID, not a filesystem path. The
manifest may also select episode data, killed-token and attestation-question
registries, and export profiles. Paths are validated within the project; the
server rejects unknown manifest fields and unsafe or overlapping storage paths.
`--demo` instead selects the bundled examples explicitly.

## Envelope and errors

Each request is one JSON object on one line:

```json
{"dli": "2.0.0", "id": "request-1", "op": "dli.version", "params": {}}
```

A response echoes the correlation ID and carries either `ok: true` with a
`result`, or `ok: false` with a typed `error` containing a stable `code` and a
human-readable `message`. Malformed frames are refused; blank lines and
non-object log noise are skipped. Responses contain no timestamps.
`dli.version` reports the contract and its available operations. Unknown
operations are refused; registered killed forms name their successor.

A mutating `author.*` request may carry `expected_digest` and
`idempotency_key` in the envelope. The MCP adapter exposes these as tool
arguments and forwards them into the same envelope. A stale digest is refused;
a retry with the same key and request can replay its result without applying a
second mutation. See [agent authoring](11-agent-authoring.md) for workspaces,
findings, structured diffs, and commit behavior.

## Operations

The `dli.*` family includes document checking and queries; fork creation,
assumptions, diff, preflight, and promotion; proof checking; timeline reads;
name checking; attestation demand; corpus import and export; and one-way
exports. Lore export profiles include context packs, Articy-shaped JSON, CSV,
and static wiki output. The generated schemas and the `dli.version`
capabilities list are the complete operation inventory.

`dli.check` returns an `ok` verdict, the weakest procedure grade used, and
findings tagged as world or attestation findings. An error finding blocks the
check; a warning may accompany a successful result. Attestation findings
report whether configured evidence is current, missing, stale, or negative.

`dli.query` accepts a typed `goal_atom` and can select a layer, chronology
point, or fork. `dli.fork.add` accepts typed ground additions. The retired
string forms are refused with their typed successors named. `dli.check-proof`
checks a named declared proof against its practice and scene projection; when
there is exactly one proof the selection may be omitted.

## State and replay

Sessions cache compiled document scopes and hold process-local fork handles.
Author workspaces stage declarations over a base; `author.materialize`
checks the proposed result and commits accepted fact objects to the lorestore.
A named branch or pinned commit supplies its own snapshot. Content-addressed
objects, committed decisions, and conditional attacks preserve history so
that the same committed inputs can reproduce a verdict and its explanation.
Authorization and review policy belong to the consuming application.
