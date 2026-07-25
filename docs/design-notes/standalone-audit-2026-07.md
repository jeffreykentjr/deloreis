---
status: reference
verdict: "Standalone source-release audit at 6aa8726: the deterministic kernel is substantial and the gate is green, but the worker is an embedded-demo implementation with contract, durability, semantic, and assurance gaps. DL-0028 supersedes prior completion claims."
---

# Standalone source-release audit (2026-07)

This note distills the emergency handover, the July audit series, a fresh
repository review, and two independent command-running red teams. It is
evidence, not authority; DL-0027 through DL-0029 and the PLAN state matrix carry
the resulting rulings.

## Baseline and validation

The reviewed baseline was
`6aa87263a8a0d32e4e5a14a46b92f1d5e6c23c9e`, equal to `origin/main`.
`cargo xtask gate` passed 17 steps with 741 tests passed and 2 ignored. The gate
therefore proves the deterministic behavior it covers, but not the completeness
claims that were outside its census.

Additional probes established:

- The bundled multi-pack world fails whole-project checking because one cycle
  is declared in two packs.
- `cargo deny check` succeeds with unused-license and duplicate-dependency
  warnings.
- `serve` and `mcp` accept no caller project and resolve embedded Camelot,
  proof, and card fixtures.
- Opening an invented Door-T scope succeeds and exposes Camelot facts.
- MCP rejects the promised digest/idempotency fields while accepting unfenced
  mutations.
- Simulation starts and completes synchronously, then disappears on restart.
- Option labels and event-era semantics are lost by Door-T lowering.
- Question-body junk can be parsed and then deleted by `fmt`.
- `option ... assumes` is parsed and taught but has no semantics.
- A non-Datalog bare rule can pass checking while lowering to no rule.

## Release-blocking clusters

**Project isolation.** There is no versioned project input, reusable corpus
runtime, document resolver, or caller-session ownership boundary. The serving
surface is a deterministic demo over hard-coded assets.

**Contract honesty.** Capability negotiation advertises every operation even
when handlers are placeholders. Query, timeline, naming, export, payload,
attestation, authoring, and fork responses omit promised data or bypass richer
engines already present in the workspace.

**Durability.** Evidence mutations do not persist. Simulation is not an
asynchronous job API and does not use the checkpoint implementation for
restart, status, or cancellation.

**Language and evaluator correctness.** The formatter can lose accepted input;
some accepted declarations lower to nothing; why rendering leaks internal
variables; fork promotion leaks internal IDs; defeated-path and path-specific
grade semantics need their recorded ruling and tests.

**Assurance.** Thirty-five operations are exempt from framing fixtures while
the gate reports complete three-way coverage. Fixtures replay over cold demo
state rather than a real project. CI parity, source-excerpt verification,
finding severity ownership, and boundary parsing remain incomplete.

**Authority.** README and DL-0022 claimed completion while PLAN, live handlers,
and external/deferred oracles contradicted it. DL-0025 and DL-0026 lacked dates,
and DL-0025 was edited after minting.

## Closure discipline

The standalone remediation proceeds in dependency-ordered phases recorded by
the PLAN matrix. Each phase has a focused oracle, the full gate, an explicit
safe commit and SHA-based CI wait, then a fresh-context local red team. A phase
does not close with unresolved critical, high, or medium findings.

Fantasy cutover, consumer handshake fixtures, live Ludii comparison, the
authoring benchmark, human probe, grammar freeze, crate publication, and release
tagging are outside this source-release engineering round. They remain named as
deferred or external-blocked rather than being counted as complete.
