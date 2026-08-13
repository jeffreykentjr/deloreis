---
status: live
verdict: "DL-0083"
---

# Decision recording on the wire

The feature spec behind `author.record_decision`, decision-stamped
`author.materialize`, and `author.why → supporting_decisions`, shipped at
contract **1.9.0**. This note states the bootstrap gap it closes, restates the
DL-0054 authority boundary it does *not* move, fixes the semantics, and audits
every certainty-gated check the feature makes reachable through the taught tool
surface. The DL id in the frontmatter is minted by the implementer against
`docs/decisions.md`.

## 1. The bootstrap gap

The provenance triad on the tool surface is lopsided:

- `author.set_standing` and `cited` provenance **consume** a pre-existing
  decision — they promote a claim, or attach a citation, to a decision that must
  already be known.
- `author.defeat` **introduces** a decision, but only as the *defeater* of an
  existing one: it records a new ruling whose whole job is to rule an older
  supporting decision false.
- **Nothing records the first, baseline, *supporting* decision.** No tool mints
  the ratified ruling that a fact can rest on to read certain.

The one path that ever created a baseline ratified decision was `.dli`'s
`ratify d-1`, and that surface is dead — text authoring is a measured fallback,
not the taught way in (DL-0014/DL-0043). So the only route to certainty left to
a wire caller runs entirely around the ledger: the `fixture-authority <store>
main true` binary (it lives in the *benchmark* repo, `dli-benchmarks`), which
force-flips **every** `h.*` proposal-gate variable in the reference's valuation
overlay to true. It records no decision, cites nothing, and reaches around the
whole known-decision set; it simply asserts "everything is certain now."

The consequence is that **contradiction detection is unreachable through the
taught surface.** Every world-internal contradiction class the engine computes —
FD, derived-FD, fluent, fluent-order, inertia-capping, `same-as` identity merge,
and the fact-checker's world-contradiction — is gated on *certain* facts. Two
proposed conflicting values produce no finding: both read `possible`, and the
engine correctly says nothing. The engine is *correct over a precondition no
wire caller can establish.* An agent that only knows the `author.*` tools cannot
reach a single one of these findings without the out-of-band `fixture-authority`
hack.

There is already standing proof the algebra itself works: the benchmark's
`designed_usage.py:demo_cited_certainty` fires `continuity.fd-violation` with
`used_fixture_authority: False` — but *only* because it writes `ratify d-1` into
a `.dli` file on disk and points a `sources` corpus at it, so the facts can cite
`d-1`. This feature moves that one `ratify` onto the wire. Nothing about the
valuation algebra changes; a caller gains the one op that lets it record the
baseline decision it was always expected to supply.

## 2. What this does NOT move — the DL-0054 restatement

DL-0054 draws a boundary: **who may rule belongs to the consuming application.**
The engine does not gate on authority. A mutating call may carry citation-bearing
provenance; caller-supplied attribution and decision state are recorded as data;
review and approval workflow live in the consumer, not the kernel.

**This feature does not move that boundary, and it is written down here because
the op will look like it does.** `record_decision` does not decide *whether* a
decision is legitimate, *who* was entitled to make it, or *whether the ruling is
right*. It records the caller's claim that a ruling was made, exactly as
`cited`, `set_standing`, and `defeat` already record caller-supplied decision
state as data. What it fills is a **bootstrap hole**: the surface let callers
*consume* and *defeat* decisions but never *introduce a baseline supporting one*,
so the decision half of the provenance model — the half DL-0054 always expected
the caller to supply — had no entry point. This op is that entry point and
nothing more.

What the engine keeps is **referential integrity**, unchanged: a cited decision
must resolve. Citing a decision no `record_decision` (or text `ratify`) has
introduced still raises the `citation.unknown-decision` error-severity finding,
`check::run` still bars materialize on it, and the decision's variable is left
open so the claim can never read certain. Recording a decision is what makes a
subsequent citation *resolve*; it is not what makes the citation *authorized*.

## 3. Semantics (contract 1.9.0)

### 3.1 `author.record_decision {workspace, decision, rationale?, provenance?}`

Writes a `Decl::Ratify` carriage object for `decision` and grows the commit's
`decisions` list, so `decision` enters the known-decision set. On the next
compile, the ledger valuation (`ledger_valuation`) sets the decision atom true
for any fact that cites it — so facts that `cite` the decision, and claims that
`set_standing` promotes to it, read `certain` **through the ordinary ledger
valuation, with no valuation force-flip.** The true-bit is free: recording the
decision is a decl write, not a write of the reference's valuation overlay.

- **Caller-supplied ids, matched by exact equality.** The caller names the id
  (`"d-1"`), exactly as `cited`, `defeat`, and `set_standing` take ids today.
  Re-recording a known id is **idempotent**. This is predictable for replay and
  adds zero new identity machinery.
- **Defeasible via `author.defeat`.** A recorded decision is a claim that a
  ruling was *made*, which is distinct from the ruling being *right*. `defeat`
  rules it false through the existing valuation-flip; every fact resting on it
  reverts to `possible` (it is not deleted). `record_decision` **interns the id
  eagerly**, the same way `defeat` does, so a later `defeat` can target a
  decision that has no other carrier yet.
- **A mutation.** It accepts author fences and returns graded findings, the
  structured result, and the workspace digest, like every other mutating tool.
  `rationale` is human metadata. `provenance` citations — a decision resting on
  prior decisions — are integrity-gated: an unresolved citation raises
  `citation.unknown-decision`.
- **Reuses `Ratify`; no new `Decl` kind.** It is the `adds_decision` path
  `author.defeat` already runs, minus the claim lookup, the
  support-cites-decision guard, the marker mutation, and the defeated-atom
  false-flip.

### 3.2 `author.materialize {…, decision?}`

When `decision` is supplied, materialize injects a synthetic `Decl::Ratify` for
it into the committed decl set **and** rests each committed **fact**'s SUPPORT on
that decision — an accepted proposed/uncited fact is rewritten to `cited d
ratified`, its support polynomial replaced by the decision variable. The accepted
**facts** then read `certain` under the decision by the ordinary algebra — and
**only** the facts. The conferral is facts-only: the code carve-out is
`object.expert_body.is_some()`, which skips every carriage claim. Moving the
support, not merely the marker, is the whole point: `derive_annotation` derives
certainty from a fact's `support`, so a `Ratified` marker left over a
proposal-gated support is inert — it round-trips back to `Proposed` and the fact
stays possible. Two carve-outs: a declaration/carriage object — anything with an
`expert_body` (functional, inertial, `same-as`, a rule, `ratify`, `__decl`) — is
never stamped or rested on a decision, so a `same-as` link, a rule, and an
inertial declaration each stay `possible` under `materialize {decision}`. Their
verdict-affecting standing comes from their embedded decl provenance, not from
fact support, so it is `record_decision` + a **cited declaration** that makes
them certain, never `materialize {decision}`. And a fact that already cites a
decision keeps its own citation.

This means "**accept this staged set's facts under decision D**," which is the
honest replacement for `fixture-authority ... true`. It is emphatically *not*
"force every support variable true": it rests exactly the facts this materialize
commits on a named, recorded, defeasible decision — not the whole valuation
overlay — so two conflicting accepted facts fire `continuity.fd-violation` on the
next check. (The honest *subset* — accepting only some staged facts under `d` —
is `record_decision` + selective `cited`/`set_standing`, not `materialize
{decision}`.) Floor-gated at 1.9.0; supplying `decision` on an earlier accepted
version is a schema violation.

### 3.3 `author.why → supporting_decisions: Vec<String>`

`why` gains a `supporting_decisions` field: the decisions in a certain fact's
support antichain, obtained by filtering the support-monomial claim ids to the
known-decision set (the same `ann_citations` filter the continuity findings use).
It is the auditability affordance — it lets a caller confirm a fact reached
certainty *the designed way*, through recorded decisions, rather than through a
side channel. Floor-gated with skip-serialize, so pre-1.9.0 `why` responses stay
byte-identical.

## 4. Deprecation audit

Every certainty-gated check below is, on `origin/main` `9b28866` (contract
1.8.0), reachable through the wire **only** by running `fixture-authority ...
true` — force-flipping every proposal gate out-of-band. After this feature each
is reachable through the taught surface: record the supporting decision, then
author/cite the claims to it (`record_decision` + `cited`/`set_standing`) — and,
for the **facts** in the set, `materialize {decision}` accepts them under it in
one call. `materialize {decision}` confers certainty on facts only: a `same-as`
link, a rule, and an inertial declaration are carriage claims it never stamps, so
their rows below reach certainty via `record_decision` + a cited declaration, not
`materialize`. Certain-gate sites are cited against `crates/dli-world/src/check/`
at `9b28866`.

| Check (finding) | Certain-gate site (verified @ 9b28866) | Certain facts today only via | Reachable wire-only post-feature via |
|---|---|---|---|
| `continuity.fd-violation` | `continuity.rs` `fd_violations` → `certain_binary_slots` filter `atom_status == Status::Certain`; claim side `factcheck.rs` `certain_values` → `continuity.rs certain_values` (`atom_status == Certain`) | `fixture-authority true` | `record_decision d` + cite the two facts, or `materialize {decision: d}` |
| `continuity.derived-fd-violation` | `continuity.rs` `derived_fd_violations` → `certain_derived_binary_slots` filter `derived.ann().status == Status::Certain` | `fixture-authority true` | `record_decision d` + cite the base facts and the rule — the rule is a carriage claim, so `materialize {decision}` (facts-only) cannot confer its certainty |
| `continuity.fluent-contradiction` | `continuity.rs` `fluent_contradictions` → `fluent_clash` sweep over `state.certain_values()`; certain-span basis (`status == Certain`) | `fixture-authority true` | `record_decision d` + cite (or `materialize {decision: d}`) the competing fluent states |
| `continuity.fluent-order-undeclared` | `continuity.rs` `fluent_order_gap` certain-span filter `span.ann.status == Status::Certain` | `fixture-authority true` | same as fluent-contradiction; the ordering edge is staged at check-only as today |
| inertia capping (DL-0081) | `continuity.rs` `fluent_finding_data`/`state_at_inertial` `capping = inertial.status == Status::Certain`; `inertial_derived_until` competitor filter `status == Certain` | `fixture-authority true` (declaration **and** states must read certain) | `record_decision d` + cite the `declare_inertial` decl and the point states — the inertial decl is a carriage claim, so `materialize {decision}` stamps the states but not the decl; cite it |
| `same-as` identity merge (WS-1) | `continuity.rs` `identity_canon` union-find over links with `link.ann.status == Status::Certain` | `fixture-authority true` | `record_decision d` + cite the `assert_same_as` link — a carriage claim `materialize {decision}` (facts-only) never stamps |
| `continuity.world-contradiction` | `factcheck.rs` `Verdict::Contradicted` requires one certain value differing from the claim, via `certain_values` | `fixture-authority true` (a certain canon value) | `record_decision d` + cite (or `materialize {decision: d}`) the canon fact; the contradicting claim then reads contradicted |
| `continuity.rejected-lore` | `continuity.rs` `certain_rejected_lore` filter `atom_status == Status::Certain` | `fixture-authority true` (a certain fact realizing a killed token) | `record_decision d` + cite (or `materialize {decision: d}`) the offending fact |
| `continuity.identity-temporal-unsupported` | `continuity.rs` `identity_temporal_unsupported` — **NOT certainty-gated**; fires on any declared `same-as` naming an era/event *regardless of the link's standing* | already reachable via `assert_same_as` (proposed) — no flip needed | already wire-reachable today; **unchanged** by this feature |

**One honest correction to the plan's A.3 list.** Eight of the nine rows are
genuinely certainty-gated and were wire-unreachable without the flip.
`continuity.identity-temporal-unsupported` is the exception: its emitter iterates
declared links with no status filter and its own doc comment states it "fires
regardless of the link's own standing — even a merely *proposed* `same-as` over
events." It is therefore already reachable on the wire via `author.assert_same_as`
alone, and this feature neither blocks it nor is needed for it. It is listed here
for completeness and to record that it is *not* part of what the bootstrap fills.

### 4.1 The retired shortcuts and their wire replacements

- **`fixture-authority` binary** (`dli-benchmarks`, `tools/fixture-authority/`) —
  `... main true` force-flips every `h.*` gate. Replaced by
  `author.materialize {decision}` (whole committed set certain under a named
  decision) or `record_decision` + `cited`/`set_standing` (selective certainty).
  Its `... main false h.X` mode (targeted supersession of one support variable)
  is replaced by `author.defeat`. Once the benchmark migration converts every
  call site (Part C), the binary is dead.
- **`.dli` `ratify d-1`** — the dead text form that once introduced a baseline
  ratified decision. Replaced directly by `author.record_decision`.

## 5. v1 boundaries and their un-lift triggers

Each is deliberately off for v1; each names the concrete consumer need that
would justify lifting it.

- **Standalone batch-ratify** — off. `materialize {decision}` already accepts a
  whole staged set's **facts** under one decision, which covers batch-accept of
  facts.
  **Un-lift trigger:** a consumer needing to bulk-ratify a large set that is
  *already materialized* (so no staging pass is available to stamp), e.g. via a
  future `author.ratify_all {decision}`.
- **Carriage-claim stamping by `materialize {decision}`** — off. The
  `object.expert_body.is_some()` carve-out means `materialize {decision}` confers
  certainty on **facts** only; `same-as` links, rules, and inertial declarations
  keep their embedded decl provenance and stay `possible`, made certain by
  `record_decision` + a cited declaration.
  **Un-lift trigger:** a consumer needing a one-call batch-accept of a staged set
  that *contains* `same-as`/rule/inertial declarations under a single decision —
  extend the carve-out to stamp carriage claims (e.g. by resting an
  `expert_body`'s standing on the decision) rather than only facts.
- **Engine-minted / content-addressed ids** — off; ids are caller-supplied and
  matched by exact equality.
  **Un-lift trigger:** a consumer that needs collision-free machine-minted ids
  rather than naming its own.
- **`dli.*` (session-corpus) decision recording** — off; recording is
  `author.*`, workspace-scoped only.
  **Un-lift trigger:** a consumer needing to record a decision over a
  materialized corpus with no workspace open over it.

## 6. Branch scoping (a correctness requirement, not a boundary)

`record_decision` and materialize-stamping route to the workspace's target
branch via `target_ref_name` and read the pre-image from `head_ref.commit`,
never `root_hash()`. A decision recorded in a branch is invisible to the base and
to sibling branches; facts certain under it read `possible` in the base. This is
the #68 SEV-A branch-isolation requirement, and a decision recorded in a branch
leaking into base/siblings is a red-team failure mode, not an accepted behavior.
