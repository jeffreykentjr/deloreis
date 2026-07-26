---
status: reference
title: "Agent-authoring validation — MCP/tool ergonomics audit + acceptance transcript"
verdict: "A cold agent CAN author + self-check a non-trivial world through the author.* tools alone, guided only by SKILL.md and the tools' responses; every mutation tool and every error is agent-legible. Four gaps found: the MCP CLI entry point (G3) and the stale spec §3 param shapes (G4) are already fixed this wave; generic MCP tool descriptions (G1) and an incomplete name.unvetted repair (G2) remain with agent-tools."
---

# Agent-authoring validation

This note is the wave-4 validation artifact for the agent-facing tooling
(`skill-mcp` lane). Its oracle: **a fresh reader, simulating a cold agent, can
author and self-check a non-trivial artifact using only `SKILL.md` and the tools'
own responses — no Rust, no hand-written `.dli`.** Everything below was captured
by driving the real `deloreis serve` worker (`target/debug/deloreis`) over its
JSON-over-stdio transport, and the MCP surface by driving the same
`dli_serve::mcp::handle_rpc` seam the MCP tests use. Requests and responses are
verbatim (whitespace trimmed).

## Verdict

**PASS, with four routed gaps.** The fourteen mutation tools are complete and
self-explanatory: each returns `{findings, rendered, workspace_digest}`, every
finding carries a stable code + a domain-spoken message + typed repair data, and
every deliberate mistake produced a typed, actionable error or finding that names
its own fix. A cold agent following `SKILL.md` reaches a materialized world
without ever reading Rust or writing `.dli`. The gaps are in the *discovery* and
*self-service* layer, not the core loop, and none blocks authoring.

## Part A — the successful end-to-end transcript

Seven frames, one persistent worker, one workspace. This is the happy path
`docs/cards/authoring-loop.md` teaches.

```text
→ author.open        {scope: saltmarket}
← ok  workspace=ws-1  digest=jcs:sha256:4b8f5ec2…

→ author.add_entity  {ws-1, person, mara, "Mara of the Weirs", proposed session-7c9}
← ok  rendered=[person mara "Mara of the Weirs" proposed session-7c9]
      findings=[name.unvetted / warn / asserted / repair:run-check dli.name.check]

→ author.add_fact    {ws-1, mara.trade = eel-fisher, proposed session-7c9}
← ok  rendered=[mara.trade = eel-fisher proposed session-7c9]  findings=[]

→ author.add_relation {ws-1, serves(mara → the-warden), proposed session-7c9}
← ok  rendered=[serves(mara, the-warden) proposed session-7c9]
      findings=[entity.unknown / error / checked / repair:resolve-entity candidates:[]]
      (staged, but this error-severity finding will bar materialize until resolved)

→ author.add_entity  {ws-1, person, the-warden, "The Warden", proposed session-7c9}
← ok  rendered=[person the-warden "The Warden" proposed session-7c9]
      findings=[name.unvetted / warn]   (the relation now resolves — full-state recheck)

→ author.diff        {ws-1}
← ok  rendered=[4 lines]   (no error-severity finding remains → materialize will pass)

→ author.materialize {ws-1}
← ok  findings=[]  rendered=[scope saltmarket / "" / person mara … / mara.trade … /
                             serves(mara, the-warden) … / person the-warden …]
```

The materialized bytes are the canonical `.dli` a human reviews — identical to
what a Door H author would have hand-written. The kernel wrote nothing; the agent
commits the bytes through its own repo review.

## Part B — the deliberate-mistake battery (error legibility)

Each row: the mistake, and whether the response is self-explanatory enough for an
agent to recover **without external help**.

| Mistake | Response | Agent-legible? |
|---|---|---|
| Call a tool on `ws-99` (never opened) | `unknown-doc`: "no authoring workspace \"ws-99\" is open" | ✅ names the cause |
| `cited` provenance on `add_entity` | `provenance.cited-refused` finding, `rendered:[]`, repair `add-provenance:{required:proposed}` | ✅ refuses + names the fix; nothing staged |
| `add_relation` to two unknown ids | two `entity.unknown` findings, one per argument | ✅ one finding per bad reference |
| `materialize` with an error finding open | `materialize-blocked`: "1 error-severity finding(s)…", `data.blocking:[entity.unknown]` | ✅ names the blocking code |
| `expected_digest` that doesn't match | `stale-fence`, carries both digests + "Read author.status before continuing." | ✅ names the recovery call |
| `add_hypothesis` in a scope (not fork) | `hypothesis.not-in-fork`: "open with {\"fork\": …}." | ✅ names the fix |
| `add_option` to an unopened question | `question.unknown`: "open it before adding options." | ✅ names the fix |
| `suppose` an unstaged hypothesis | `hypothesis.unknown`: "No hypothesis \"…\" is staged…" | ✅ names the cause |
| Bad enum (`kind: "persom"`) | `schema-violation`: "unknown variant `persom`, expected one of `person`, `place`, `thing`, `era`" | ✅ lists valid values |
| `discard` a decl that matches nothing | `unknown-doc`: "no staged declaration matching \"…\"" | ✅ never nukes the workspace |

Every error and finding in the battery is actionable from its own text. **No
opaque errors were found in the mutation surface.**

## Part C — the MCP surface

Driving `initialize → tools/list → tools/call`:

- `initialize` returns `serverInfo.name = "deloreis"` and the `tools` capability. ✅
- `tools/list` registers every op, each with a real `inputSchema` (`required`
  arrays, per-field descriptions, and the `EntityKind` / `Provenance` enums fully
  documented with their variants and examples). ✅ The inputSchema is genuinely
  good. (Audited at 41 ops / 14 `author.*`; the 5 read tools of Part D since
  landed bring it to 19 `author.*`, and they inherit the same generic tool-level
  description — G1.)
- `tools/call author.open` forwards through the shared dispatch and wraps the
  worker response as `{content:[{type:text, text:"<wire JSON>"}], isError:false}`.
  A worker `ok:false` surfaces as `isError:true`, not a JSON-RPC error — the call
  succeeded; the lore verdict is the content. ✅

## Part D — the self-check read loop (the new capability)

The AUTH-2 read/self-check tools landed this wave, and the loop now **closes
through `author.*` alone** — an agent authors *and* fully self-checks with zero raw
`.dli` and zero `dli.*` batch ops. Driven live over `deloreis serve`, staging the
same `serves(mara, the-warden)` relation to an as-yet-unknown entity:

| Tool | Call | Result (self-check use) |
|---|---|---|
| `author.findings` | `{workspace}` | `{errors, warnings, materializable}` — with the reference open: `materializable:false`, `errors:[entity.unknown]`. The go/no-go pre-flight. ✅ |
| `author.query` | `{workspace, goal:"serves(mara, ?who)", why:true}` | `rows:[{atom, bindings:{who:"the-warden"}, status:"possible", verdict:{grade:"asserted"}}]` — a proposed claim reads `possible`, never `certain`. ✅ |
| `author.why` | `{workspace, atom:"serves(mara, the-warden)"}` | `why_not:{known:true, status:"possible", defeater:null, rendered:["awaits ruling: …"]}` — names why it can't be certain yet. ✅ |
| `author.check` | `{workspace}` | `{ok:true, findings:[name.unvetted warnings]}` after the entity is added — full battery, contradictions only (proposed ≠ "unsupported" noise). ✅ |
| `author.list_workspaces` | `{}` | `workspaces:[{workspace:"ws-1", base:"saltmarket", mode:"scope", staged:3, workspace_digest}]` — context recovery. ✅ |

After the missing entity is added, `author.findings` flips to `materializable:true`
(only `warn`s remain) and `author.check` returns `ok:true` — the workspace is
clear to materialize. Each read tool rejects an unknown workspace (`unknown-doc`)
and unknown fields (`schema-violation`), and treats a command-shaped `goal` string
as a plain atom (framing-safe). The end-to-end acceptance test
`crates/dli-serve/tests/author_read.rs::end_to_end_self_check_session` proves the
full loop closes through `author.*` alone; it passes.

## Gaps found — routed to owning lanes

**G1 · Generic MCP tool descriptions → `agent-tools`.** Every `author.*` tool's
`tools/list` `description` is the generic template `"The `author.add_entity`
worker operation."` (from `tool_descriptors()` in `crates/dli-serve/src/mcp.rs`).
The one-line description is exactly what an agent uses to *choose* a tool and
understand its role in the loop, and it carries zero information. The
`inputSchema` compensates for *how to call* a tool once chosen, but not *which*
tool or *why*. Fix: a per-tool description table (purpose + where it sits in the
open→add→self-check→materialize loop). This is the only thing standing between the
current surface and full tool-selection legibility from `tools/list` alone.

**G2 · Incomplete `name.unvetted` repair → `agent-tools`.** The finding's repair
is `{kind:run-check, op:dli.name.check, params:{name}}`, but `dli.name.check`
**requires** a `dialect` (`NameCheckParams` has no default). An agent that follows
the typed repair verbatim gets `schema-violation: missing field dialect`. The
error is legible, so recovery is possible — but a *typed repair* should be
complete. Fix: carry the scope/dialect in the repair payload, or make `dialect`
optional. (Related open work: task "Item 3: dialect vet (name check --dialect)".)

**G3 · MCP not reachable through the shipped binary — FIXED this wave (CLI-1).**
When first audited, `dli_serve::mcp::run_mcp()` existed, was public, and worked
(covered by `crates/dli-serve/tests/mcp.rs`), but no subcommand invoked it —
`deloreis serve` ran only the stdio worker, so an agent could not reach MCP through
the binary despite the two-transport promise in spec 11 §1. The CLI-1 lane landed
the entry point: `main.rs` now routes `Command::Mcp` → `cmd::serve::run_mcp()` →
`dli_serve::mcp::run_mcp()`. **Both transports are now real from the binary:**
`deloreis serve` (stdio) and `deloreis mcp` (MCP JSON-RPC). Verified in source here
and runtime-verified by the lead (`deloreis mcp` + an `initialize` frame returns a
proper MCP response); SKILL.md, `docs/cards/authoring-loop.md`, and spec 11 §1 now
name `deloreis mcp` as the MCP entry point.

**G4 · Stale `§3` param shapes in spec 11 — FIXED this wave (contract-close).**
The wire-table examples in `docs/spec/11-agent-authoring.md` §3 had drifted from
`contract.rs`: `author.open_question` is `{workspace, id, prompt}` (spec showed
`options[]` + `provenance`); `author.add_option` is `{workspace, question, id,
label}` (spec showed `{question, option}`, and `label` is stored but not
rendered); `author.suppose` is `{workspace, hypotheses:[…]}` (spec showed a
`{hyp/option, value}` form — the "assume an option" path is unimplemented, and the
`fantasy-contract` §3.2 row overclaimed it too). Contract-close corrected all §3
request/response JSON blocks and the fantasy-contract row against the captured
shapes; this lane reconciled the narrative prose, including the two message-wording
strings (§3.3 entity.unknown "The claim is staged…", §3.9 consequence.changed's
generic form). Spec 11 is now engine-true across both owners' sections.

## Reproducing this

Build with `TMPDIR=$HOME/.cache/tmp cargo build -p deloreis`, then feed newline-
delimited request frames to the worker:

```text
target/debug/deloreis serve < frames.jsonl
```

Each line is one `{"dli":"0.2.0","id":"…","op":"author.…","params":{…}}` frame;
responses come back one per line, in order, over a single persistent workspace
store. The MCP surface is now reachable directly with `target/debug/deloreis mcp`
(JSON-RPC over stdio: `initialize`, then `tools/list` / `tools/call`); it was
audited here through the `dli_serve::mcp::handle_rpc` seam that
`crates/dli-serve/tests/mcp.rs` exercises, and both paths reach identical kernel
code.
