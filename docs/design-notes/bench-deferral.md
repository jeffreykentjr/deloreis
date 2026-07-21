---
status: reference
verdict: "BENCH-1 (the authoring-benchmark rig) and BENCH-2 (the two-door run) are deferred: the dli engine build is otherwise complete, and the door-decision benchmark is the remaining, human-probe-gated, live-LLM piece."
---

# Deferring the authoring benchmark (BENCH-1 / BENCH-2)

## What is deferred

- **BENCH-1** — the authoring-benchmark *rig*: two authoring doors (Door H, the
  `.dli` surface; Door T, the `author.*` tools) × models, plus the two demoted
  arms (raw text editing, raw AST emission). Pass is defined by the real gate's
  verdict on the artifact each arm produces.
- **BENCH-2** — the *run and report*: a two-door benchmark over the example world
  and vendored corpus that yields a single verdict — **which authoring door to
  keep shut** — never a menu.

No rig code and no run exist as of this note; both nodes are parked (see
`../PLAN.md`, marked `*(deferred.)*`).

## Why deferred (not dropped)

The dli engine build is otherwise complete: the core spine, the plateau (queries,
forks, checks, time, practices, language, attestations), the integration layer
(server, Door T, CLI, proof checker, simulator), the stretch lanes (Turning
engine, exports, craft rules, payloads), and the TPtH/general separation have all
landed and are gate-green; the honest not-yet CLI/serve commands are now wired
([DL-0021](../decisions.md)) and rule-body negation is implemented
([DL-0020](../decisions.md)). What remains inside dli's own control, beyond
polish, is the **door-decision benchmark** — and it is the one piece that is not
pure engine construction:

1. **It needs a human input first.** BENCH informs **SURF-4** (the grammar-gate
   freeze), which the plan holds open pending the **human-writer probe**
   (`../cards/probe-protocol.md`) — the author's own act, not a bot's. Running
   the benchmark before that probe would freeze a surface on machine evidence
   alone.
2. **It is live-LLM work.** The arms call real models; per the CI-quarantine
   discipline it is non-gating, never in CI, and needs an authorized run with a
   hard spend cap (Bedrock-only — the proven `flukeball-bedrock` profile;
   `opus`/`sonnet`/`haiku` model ids; `fable-5` is account-blocked; **never**
   OpenRouter/xAI).

Deferring keeps the engine's offline, credential-free gate intact and avoids
spending on a run whose verdict the author is not yet ready to consume.

## Flip condition ([DL-0022](../decisions.md))

Build BENCH-1, then run BENCH-2, when **both**: (a) the human-writer probe is
prioritized (so SURF-4 has its human evidence), and (b) a live-LLM door-decision
run is explicitly authorized with a spend cap. BENCH-2's output is a design-note
(status: reference) carrying the door verdict + a per-call cost log, and it
unblocks SURF-4.
