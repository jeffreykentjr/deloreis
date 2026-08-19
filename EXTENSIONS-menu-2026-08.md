---
status: delivered
verdict: "Internal decision doc: a principled, prioritized menu of high-leverage feature extensions to the engine surface, with code-feasibility anchors, external precedent, demand evidence, cost shapes, and triggers. Companion to EVALUATION-directions-2026-08.md (which ranks directions; this ranks features). Non-canonical; the ledger outranks this file."
---

# EXTENSIONS — the high-leverage feature menu (2026-08)

**Authorship: Claude (Anthropic), Claude Code session, 2026-08-18.** Internal
decision document for Jeff. Non-canonical: `docs/decisions.md` outranks it, and
per the house law every semantic change proposed here needs its own DL entry
before it is anything more than a proposal. Method: nine parallel evidence
agents — five reading deloreis + dli-benchmarks (every claim anchored
`file:line`), four external sweeps (every claim carrying a URL or marked
**[ASSESSMENT]**) — synthesized against the verified state in
`EVALUATION-directions-2026-08.md` (§1–§6 of that doc are assumed, not
re-derived). A bounded fresh-context verification pass ran before delivery:
all 17 load-bearing code anchors confirmed against source, all 12
spot-checked external claims confirmed at their URLs, and a skeptic's ten
findings (four major: an interface-budget collision in the MCP item, an
underspecified expiry item, an omitted GH #92 prerequisite in the
incrementality item, and a sequencing conflict with Experiment-1-first)
are folded into the text below. Items are tagged **[new]** or
**[contemplated: source]** so it is honest about what the repo's own roadmap
MDs already imagine — per the brief, the menu is not constrained by them.

Relation to the standing plan: nothing here displaces the EVALUATION §8
sequence (Experiment 1 → publish negatives → benchmark → vertical → MCP).
This menu says what to *build on the engine surface*, and §5 interleaves it
with that sequence — several items exist precisely to make Experiment 1 and
the benchmark cheaper.

---

## §1 The surface today, in three structural facts

Full inventory: 68 wire ops at contract 1.12.0 (`crates/dli-serve/src/ops.rs:248-317`),
29 `dli.*` + 39 `author.*`. What matters for extensions is not the list but
three structural facts the evidence sweep established:

**Fact 1 — the valuation is a parameter, end to end.** Saturation produces
`Derived` rows holding support polynomials + per-monomial defeaters; **status
is never stored** (`crates/dli-eval/src/world.rs:215-238`). Standing is
computed by `eval_under(&Valuation, &Query)` — a linear sweep evaluating
stored polynomials under an explicit assignment (`world.rs:1121`,
`ann.rs:312-329`). Decision ids are interned event variables
(`world.rs:846-864`); `author.defeat` is literally `valuation.set(atom,
false)` on the branch ref (`author/tools.rs:3322-3351`). Fork overlays
(`ForkStore::suppose/suppress/diff`, `fork.rs:420-535`) already evaluate the
same saturation under modified assignments. Consequence: every
counterfactual, impact, and diff read in this menu is a *valuation sweep over
already-computed labels*, not new engine math. The one comic gap:
`dli.fork.suppose` on the wire hard-codes `value=true`
(`handlers.rs:465-471`) while the engine's `suppose` takes `value: bool`
(`fork.rs:420`) — previewing a defeat is one boolean away.

**Fact 2 — every author read recompiles the world; the store can already
read at any commit.** All author reads and the materialize gate recompile
base + staged decls per call (`author/reads.rs:522-566`,
`author/tools.rs:2554`); the 64-entry LRU caches per-*commit* compiles only
(`dli-lorestore/src/cache.rs:62`). Meanwhile `reconstruct_at`/`compile_at`
accept **any commit hash** and are cached (`lorestore.rs:965-1106`) — the
library half of time travel exists; no wire op consumes it. One honest
caveat that shapes the history pack: **valuations are unversioned** — branch
refs are overwritten in place via CAS with no reflog, and defeats ride the
ref valuation (`tools.rs:3322-3351`), so historical fact sets are exactly
recoverable and historical *rulings* are not.

**Fact 3 — the store retains everything and shows only the latest.** Every
standing change writes a revised fact object + a commit; the fold keeps
last-writer-wins per identity hash while all revisions stay reachable
(`lorestore.rs:623-646`). Per-claim *revision* history (markers,
standing_decision, defeated_by, citing decisions, introducing commits) is
fully present and has no API — with the Fact-2 caveat carried forward:
standing *at* a historical commit also needs the ruling in force then,
which the unversioned ref valuation does not preserve. Likewise `LineageEdge` (supersession edges carrying their own
(P⁺,P⁻)) and `EvidenceEnvelope` types, schemas, and commit lanes exist and
**every production write path passes `Vec::new()`** (`identity.rs:144-221`;
e.g. `tools.rs:3313-3314`) — built, dead capacity.

**Constitution (extensions must not violate).** No clock, network, or model
in the kernel or gating lanes; koan 3 (callers declare identity/canon, the
engine computes); koans 7/11 and the charter budget's **one interface** — no
synonym surfaces, the declarative tool surface is the single way in
(DL-0043); DL-0054 (authorization/review policy is consumer-owned — bars
in-engine RBAC, trust weights, winner-selection); koan 10 (exports one-way,
no write-back); no floats in fact objects (LS-1); append-only, defeat is a
valuation flip never a deletion; killed forms retire loudly; benchmark
results never change semantics by themselves
(`docs/spec/12-scale-and-distribution.md:98-109`, `docs/PLAN.md:450-457`,
`docs/spec/03-time.md:396-419` — metric time and calendars are excluded *by
ruling*, which shapes the expiry item below).

---

## §2 What the evidence says (cross-cutting findings)

**F1 — Adoption in versioned/immutable stores clusters on five feature
families, and DLI has the substrate for all five.** Dolt's own testimony:
"most people who say Git for Data want log, branch, merge, and diff"; every
named paying customer runs a branch+diff+merge *human-review loop*, and
"many people... really want the GitHub experience with data"
([dolthub.com/blog/2024-09-24](https://www.dolthub.com/blog/2024-09-24-git-for-data/),
[2024-10-15](https://www.dolthub.com/blog/2024-10-15-dolt-use-cases/)).
Datomic's sticky trio is `db.with` speculative writes (community:
"superpower"), the pull API, and audit-framed history — the feature set that
won Nubank, the regulated fintech that chose Datomic for its "first class
concept of time"
([datomic.com/nubanks-story](https://www.datomic.com/nubanks-story.html))
and by 2020 ran 2000+ Datomic servers and acquired Cognitect (acquisition
coverage, July 2020).
Negative space is equally instructive: raw as-of time travel alone reads as
oops-recovery (Snowflake/BigQuery) — the sticky forms are *audit-framed
history* and *diff-framed comparison*; bespoke query languages kill adoption
(XTDB v2 dropped Datalog for SQL explicitly "to aid familiarity"); generic
versioned infra without a workload-shaped workflow dies (Noms, DoltHub
bounties, TerminusDB's pivots); and server-side user-programmable
projections are a documented production hazard (EventStoreDB) — ship a
deterministic changefeed, never a hosted projection engine.

**F2 — The agent-memory demand center moved onto DLI's write path, 2025–26.**
Both poles of LLM-arbitrated conflict resolution failed in production: Mem0
v1/v2 silently deleted needed memories, so v3 went ADD-only (2026-04), which
now accumulates stale contradictions
([#4956](https://github.com/mem0ai/mem0/issues/4956)); Graphiti's LLM
invalidation judge stamped 41% of one production graph `invalid_at` with
silent collateral retirements
([#1728](https://github.com/getzep/graphiti/issues/1728)). Practitioners in
the #4956 thread spec the missing primitive nearly verbatim as DLI's
existing machinery: "a small provenance/supersession layer... source-tag
each memory update... explicit supersession as a first-class write, rather
than trying to patch stale memory at retrieval time... whether promotion
[to] durable/core memory requires human ratification." Zep deprecated its LLM-assigned Fact Ratings entirely
(Feb 2026) and steers users to *custom ontology* — the vendor field is
independently converging on declared vocabulary. Point-in-time attribute
state is an open unserved gap (Graphiti
[#1166](https://github.com/getzep/graphiti/issues/1166)); batch/structured
ingest is explicitly requested (mem0
[#3761](https://github.com/mem0ai/mem0/issues/3761), Graphiti #1299/#1262);
memory-poisoning attacks (EchoLeak CVE-2025-32711, MemGhost) make
write-review a security control. The one demand that cuts *against* the
store: GDPR-grade erasure (F7).

**F3 — The harnesses are a measured demand list.** Every place a benchmark
harness built scaffolding around the engine is a candidate feature with a
cost number attached: the client-composed identity workflow is ~330 lines,
10 ops and 2 worker sessions per accepted match, and produced two real bugs
the lifecycle audit confirmed (proposal facts leaking to main; leaked
`identity-*` refs); the scoped-valuation gap is recorded *inside every
sealed round-2 artifact* ("the wire has no scoped-valuation parameter",
`subtlememory_round2.py:521-563`); a full `author.predicates` census is read
before **every** mention (335 full reads per persona run); 1,288 claims cost
thousands of per-object calls; recovery re-authors whole worlds claim-by-
claim because no snapshot-restore path is packaged; control flow
string-matches `"materialize-blocked"` out of error text; store stats come
from globbing `.dli-lorestore` internals. The Gate-1 prereg states the
thesis outright: "at scale this loop belongs inside the engine: candidate
retrieval should be an engine service, resolution should be a caller-supplied
policy, and aliases should be first-class claims."

**F4 — The incremental wall stands, and now has a licensed side door.** The
first systematic IVM study over semirings *without additive inverse* that
this evaluation is aware of — "first" is our characterization, not the
paper's own claim — (Chmielewski/Draghici/Olteanu/Zhang,
[arXiv:2606.07795](https://arxiv.org/pdf/2606.07795), June 2026) confirms
the parked position: deletes/negation/recursion are untouched for
absorptive payloads — DLI's absorption-destroys-the-delta finding is a
documented dichotomy boundary, not a solvable gap. But the wall is
specifically about *defeater-carrying* deltas; insert-only maintenance is
sound and now formally characterized — which licenses a commit-type-directed
fast path (§3-G). Separately: **no shipped system offers derived-fact change
subscriptions without a resident dataflow daemon** (Materialize/Feldera/
Noria pay for a daemon; Datomic/Supabase/Electric expose base rows only) —
a deterministic commit-scoped engine gets the derived-fact changefeed for
the price of a diff. That is open product surface with no exact precedent.

**F5 — KR precedent maps three semantics extensions onto known-tractable
ground.** Denial constraints subsume FDs and are P-time to check when
declared (the exponential cost in that literature is *mining*, not
checking); SHACL core is the field-tested taxonomy of what practitioners
actually declare (cardinality, type/domain, exclusivity, inclusion,
property-pair) — and DLI already ships two hard-coded instances (the FD
continuity class; `citation.unknown-decision` referential integrity).
Grounded argumentation semantics is the unique mainstream reinstatement
semantics that is simultaneously deterministic (unique extension) and
P-time, and Caminada's in/out/undec labelling is structurally
certain/dead/possible (Dvořák & Dunne complexity survey; Maher 2001 proves
the defeasible-rule variant linear-time). Why-not provenance and minimal
repair sets are shipped by **no commercial system** (RDFox and s(CASP) stop
at why-trees) — and DLI is unusually cheap there because the per-fact
polynomial already localizes the repair structure that makes bare-database
repair coNP-hard. Bonner's wall disciplines the what-if surface: embedded
(rule-level) hypotheticals are PSPACE — assumptions must stay flat,
query-level.

**F6 — Nothing below reopens a closed line.** No item performs similarity
merging, tunes resolution thresholds, or revives open-domain
canonicalization (EVALUATION §G). The identity-workflow op (§3-D4) executes
*caller-supplied* identity decisions — the engine still never proposes a
merge.

**F7 — One procurement blocker needs an answer, not a feature.** Regulated
buyers ask about erasure on day one; content-addressing makes byte-level
deletion structurally hostile; Datomic's excision precedent is "costly, use
very infrequently, and record that you forgot." §3-H3 scopes this as a
design note + documented key-indirection pattern first, code later if ever.

---

## §3 The menu

Grouped into packs. Each item: what, evidence, feasibility anchor, cost,
risks, and tag. Cost words mean: **S** = wire op + schemas + fixtures over
existing machinery; **M** = new read/plumbing path or schema field, no new
semantics; **L** = new semantics or subsystem, DL entry mandatory.

### Pack A — What-if & impact (the reads the architecture already paid for)

**A1. `suppose`-false on the wire** — permit assuming a decision/hypothesis
**false** in fork overlays (and an `author.*` mirror), previewing a defeat
before `author.defeat` commits it. *Evidence:* Datomic `db.with` is the
community-designated "superpower"; ATMS context switching is the engine's
own ancestry ("all retraction is avoided"). *Feasibility:* engine takes the
bool today (`fork.rs:420`); the wire hard-codes true (`handlers.rs:465-471`).
*Cost:* **S** — one params field + version gate. *Risks:* callers confusing
explicit-false with unruled; document loudly. **[new]**

**A2. `author.impact` — decision blast radius.** Given a decision id (or a
hypothetical assignment over several), return every fact whose support or
defeater mentions its variable, with the standing each would take under the
flip — "what changes if D-0147 is revoked" as one read. *Evidence:* the
killer compliance query, and the indexing note already names the O(k)
reader for it (polynomial-variable posting lists,
`lorestore-indexing-strategy.md:99-126`). *Feasibility:* linear sweep +
`eval_under` at atelier tier; polynomials are stored per fact
(`object.rs:270-279`). *Cost:* **S** at current scale; Phase-2 index is the
archive-tier upgrade, already designed. *Risks:* none structural — this is
the architecture's home turf. **[contemplated as an index use-case:
indexing-strategy §Phase 2; the op itself new]**

**A3. `author.what_if` — flat assumption-scoped query.** One read taking
`{assume: [{decision, value}], goal_atom | scope}` and returning standing
(optionally a diff against actual) under that assignment. Assumptions are
**flat by construction** — query-level, never rule-embedded — which is what
keeps it P-time (Bonner: embedded hypotheticals are PSPACE). Hypothetical
*facts* ride the existing staged-workspace path, not this op. *Cost:* **S**
for decision flips (a valuation sweep); staged-fact what-ifs already exist
as workspaces + `author.diff`. *Risks:* scope creep toward nested
hypotheticals is the complexity cliff; refuse it in the schema. **[new]**

**A4. Speculative branches as a taught pattern, not new ops.** O(1) branch +
dry-run merge (writes the identical content-addressed commit, moves no ref,
`lorestore.rs:1186-1220`) + pointer delete is ~90% of a what-if-branch
product already; the missing piece is documentation (SKILL.md recipe) and
GC honesty (discarded what-ifs orphan objects — DL-0077 defers GC).
*Cost:* **S** (docs) — and it feeds Dolt's "consensus-by-identical-diff"
agent pattern once C1 lands. **[contemplated: machinery shipped; the
pattern surface new]**

### Pack B — Read surface (context assembly for agents)

**B1. Scoped-valuation parameter on `author.query`.** `valuation: none |
scoped | full` where `scoped` returns only the atoms in the support/defeater
closure of returned rows + findings, plus a digest of the full valuation.
*Evidence:* the only engine gap the harness records inside every sealed
artifact; forces a client regex projection and a 64KB prompt guard.
*Feasibility:* the closure is computed for `why=true` already
(`reads.rs:98-104` returns full today). *Cost:* **S**. *Risks:* the closure
must include defeater atoms or grades lie — state it in the contract.
**[new; demand recorded in dli-benchmarks artifacts]**

**B2. `author.about` — entity dossier (pull-shaped).** Everything the world
holds about entity X in one bounded call: sort, alias class, every atom
mentioning it with standing/grade/support, fluent slots, open questions.
*Evidence:* Datomic pull is the workhorse read of that ecosystem; the
harness's substitute is planner + N single-atom queries + a full census per
mention; agent-memory context-pack assembly is this call. *Feasibility:*
enumeration + identity-canon fold already written inside
`predicates()`/`entity_candidates()` (`reads.rs:219-310`). *Cost:* **S–M**
(result bound needed). *Risks:* XTDB's lesson — keep the pattern language
closed and schema-defined, no query-language creep. **[new]**

**B3. Census/candidates efficiency.** (a) `author.predicates` delta mode
(If-None-Match by root_hash) — the harness reads the full census before
every mention; (b) `entity_candidates` v3: return alias sets and per-pred
claim counts (identity canon is already computed in the read,
`reads.rs:279-291`), over an indexed surface when DL-0091's measurement
fires. Caveat on that trigger: it names a *SubtleMemory* measurement, and
no further SM work is authorized without Jeff's explicit sign-off — the
trigger likely needs re-basing onto whatever workload Experiment 1 runs.
*Cost:* **S** (delta) / **M** (index, behind the re-based trigger).
**[contemplated: DL-0091; delta mode new]**

**B4. `dli.*` as-of read without a workspace.** DL-0082's recorded boundary
with the trigger "a consumer needing the as-of read without an authoring
session" — any read-mostly consumer hits it immediately. Kernel projection
exists (`check::slot_state_at`). *Cost:* **S**. **[contemplated: DL-0082]**

**B5. Conjunctive goals by server-side desugaring.** Accept a
`body: [BodyLiteral]` goal, synthesize an anonymous rule + head for one
compile, reuse `query_with` — the staged-rule workaround proves the
semantics today, minus the digest churn. *Cost:* **M**. *Risks:*
deterministic naming of the transient predicate; result bounds. **[new]**

### Pack C — History, diff, changefeed (the audit spine)

**C0. Valuation versioning — the pack's real price tag.** Snapshot the ref
valuation into each commit (append-only lane, upcast, DL entry): **L** by
this doc's own legend, and it is the substrate C3/C4 need to deserve the
audit framing. The C1–C4 cost labels below assume option (a) of the honesty
condition (ship with the fact-set-only caveat stated in the contract); a
reader summing the pack must add C0 if the audit story is the point. **[new]**

**C1. Commit-pair / branch-pair standing diff.** Generalize the LS-5 diff
off the workspace anchor: two refs in, `AuthorDiffRow`s out (before/after
standing + grade, introduced/retracted, attribution). *Evidence:* the
single strongest adoption signal in the external sweep (Dolt: diff is what
everyone uses); ClawArena's adapter needs belief-state deltas per round;
supervision already reviews exactly these rows. *Feasibility:* pure
composition — `compile_at` any commit (cached) + `classify_standings`
(`reads.rs:568-665`) + existing row schema. *Cost:* **M**. *Risks:*
pagination; a stated rule for cross-schema-version diffs. **[new]**

**C2. `author.changes_since` — the pull-cursor changefeed.** Cursor = the
ref/commit digest the caller last saw (callers already hold these as fence
tokens); response = standing transitions to head in commit order. This is
C1 iterated along first-parent — deterministic, no daemon, no push, gate-
compatible. *Evidence:* the no-precedent gap (F4); Convex/Materialize prove
reactivity sells; Datomic's tx-report-queue is the commit-scoped shape;
Supabase's documented can't-subscribe-to-views limitation is the market
hole. Three-valued transitions generalize `mz_diff ±1` cleanly. *Cost:*
**S once C1 exists**. *Risks:* the EventStore trap — never host consumer
logic; long-gap diffs need pagination. **[new]**

**C3. Time-travel reads** — `at: commit` on `author.query`/`why`/`as_of`/
`check` (and `dli.query` already has era/event `as_of` + fork overlay;
`author.query` hard-codes both to None, `reads.rs:86-91`). *Evidence:*
audit-framed history is what won Nubank; EU AI Act Art-12 language is
"reconstruct what the agent knew"; but see the honesty condition below.
*Feasibility:* `compile_at` + cache exist; project head-pinning proves the
pattern (`project.rs:820-856`). *Cost:* **M**. **[new at the wire; library
contemplated]**

**C4. `author.history` — per-claim standing dossier.** For one claim: every
revision in commit order with marker, standing_decision, defeated_by,
introducing commit, cited decisions. The revision data is fully retained
and invisible (Fact 3) — this turns the store's strongest property into a
product feature, with Fact 3's caveat inherited: the dossier is exact about
recorded revisions; reconstructing *standing* at each historical point
additionally needs C0. The ledger already reserves the name
(`docs/decisions.md:561-562`). *Cost:* **M** (O(commits) walk now; Phase-2
citation index later). **[contemplated by name; unbuilt]**

**Honesty condition for the whole pack:** valuations are unversioned
(Fact 2). A time-travel read is exact for the *fact set* and not for
historical *rulings*. Either (a) ship with the caveat stated in the
contract (fact-set-at-commit + current-or-caller-supplied valuation), or
(b) do the small design first: snapshot the ref valuation into each commit
(append-only lane, upcast, DL entry). (b) is the right substrate for C3/C4
to deserve the audit framing — without it, "what did we believe at C" is a
half-truth the grades law shouldn't tolerate silently. **This valuation-
versioning decision is the one real design job in the pack and should lead
it.**

### Pack D — Ingestion & wire ergonomics (what Experiment 1 and every pilot touch)

**D1. `author.add_batch` — ordered typed declarations, one call.** Server
loops the existing single `stage()` entry (`tools.rs:1402-1426`), returns
per-decl graded findings + one digest. *Evidence:* 1,288 claims = thousands
of calls; replay re-authors entire worlds; Mem0 #3761 / Graphiti #1262
show the demand is generic. *Cost:* **S–M**; the design work is
partial-failure semantics (collect-and-report beats fail-fast — findings
are already per-decl). *Risks:* keep the per-add object model (DL-0045);
never a monolithic document. **[new]**

**D2. `author.declare_schema` — the vocabulary manifest.** One call
declaring a vertical's vocabulary: predicates with functional/inertial
flags, entity sorts, rules, dialects/banned words — i.e., the shareable
"schema pack" artifact the declared-vocabulary strategy wants to version
and hand to Experiment 1, direction C's onboarding, and the benchmark's
domains. Strict subset of D1 (declaration kinds only) — ship as a
restricted batch, don't ship both shapes separately. *Cost:* **S** given
D1. **[new]**

**D3. Ergonomics batch (small independent PRs).** Typed
`materialize-blocked` results instead of string-matched error text; uniform
digest/idempotency envelope on materialize/branch/merge; `author.stats`
(commit/object/branch counts) so harnesses stop globbing store internals;
snapshot export→import documented as the blessed replay/recovery path
(`corpus.export`/`import` already exist and are healthy); fix spec 11 §3.18
(stale: says no wire op routes writes to a branch — 1.7.0 did). *Cost:*
**S each**. **[new; all documented warts]**

**D4. `author.apply_identity` — the composite identity transaction.** One
op that executes a caller-supplied identity decision atomically: record/cite
decision, stage carried facts, assert (entity|predicate) same-as, check,
merge-consequence preview, then commit+merge or reject+clean. (Named
*apply*, not *propose* — the caller decided; the op executes and reports.)
*Evidence:* the harness's version is ~330 lines, 10 ops, 2 sessions per
match, and produced two real bugs; the prereg states "the loop belongs in
the engine." *Cost:* **L**, honestly — the hard part being moved
server-side is cross-branch failure-path atomicity, which is new semantics
by this doc's legend and needs its own DL entry; the two harness bugs it
cites as demand are precisely atomicity failures. *Risks:* koan 3
discipline; does not reopen canonicalization (F6). **[new; demand recorded
in prereg + Gate-1 artifacts]**

### Pack E — Semantics (each needs its own DL entry)

**E1. `author.declare_constraint` — denial constraints as declared
vocabulary.** Generalize the two shipped instances (FD continuity;
citation referential integrity) into the SHACL-shaped practitioner classes:
cardinality, exclusivity/xone, inclusion/referential, type/domain,
two-row comparison denials — violations as graded findings with repair
data, `check::run` barring materialize on error severity, exactly the house
pattern. *Evidence:* P-time when declared; the multi-authority note rates
its Layer-2 twin **Low lift** ("constraints compile to support polynomials;
the engine already handles unsatisfiable polynomials"); DL-0092's arity
check is the first hard-coded instance with "a declared predicate-shape or
type system" as its recorded un-lift trigger; RelationalAI's
`ic`-with-counterexamples is the commercial result shape to copy.
*Cost:* **M–L**. *Risks:* semantics under three-valued standing (violated
by certain only, or possible too — encode in severity); weak/cost-ranked
constraints tempt optimization semantics — refuse, severity grades instead.
**[contemplated: multi-authority note Layer 2 + DL-0092 trigger; the
unified surface new]**

**E2. Declared value domains & incompatibility.** Per-predicate value
vocabulary: enum/scale membership, declared-incompatible pairs
(`old ≁ young`), declared-legitimate transitions. FD then fires on
*declared-incompatible* values instead of any distinct strings. *Evidence:*
the FF ceiling report names the incompatibility gate as one of two
load-bearing components the pipeline had to build outside the engine — the
direct reason dli-only recall was 0.18. Koan-compatible: callers declare,
the engine computes. *Cost:* **M** (a declaration kind + check extension +
Finding.data rider). *Risks:* vocabulary sprawl — per-predicate, killed-form
discipline applies. **[new; demand measured in
`flawedfictions-realeval-ceiling.md:55-67`]** — E1 and E2 together are the
"constraint pack" and should share one DL entry and schema shape.

**E3. Reinstatement.** (a) `author.reinstate` — the missing inverse of
defeat: record a decision, flip the defeated decision's atom back true,
clear `defeated_by`. Today defeat is a one-way door (the flip is not gated
on the defeating decision's variable; no op re-rules true;
`set_standing` refuses non-provisional claims) — a genuine expressiveness
hole in an otherwise defeasible engine, and the algebra already treats a
dead defeater as inert (`ann.rs:367-370`), so valuation-level reinstatement
is semantically coherent now. (b) The deeper form — defeats gated on their
own variable, so a defeat is itself defeasible — is what `LineageEdge`'s
own (P⁺,P⁻) was designed for and is currently dead code. Grounded
labelling is the only reinstatement semantics compatible with the
determinism + P-time walls (unique extension, P-time; in/out/undec ≅
certain/dead/possible), with Turnip/Data61's regtech program as the shipped
precedent shape. *Evidence for demand:* appeals-shaped legal workflows;
retcon-of-retcon; a correction later found wrong (agent memory). *Cost:*
(a) **S–M** — mirror of `durable_standing_change`; (b) **L** — ledger
ruling, lowering change, upcast for existing defeats. Ship (a); park (b)
with E3's DL entry naming it. **[new; (b)'s machinery contemplated by
LineageEdge design]**

**E4. `why_not` completion: repair sets + `min_why` + abductive mode.**
(a) Wire the existing `Why::min_why` (smallest witness — computed, never
consumed; `why.rs:101-105`). (b) Repair data on why/why_not: minimal
decision assignments that would flip standing, read off the per-fact
polynomial — spec 08-reasoning T6 *pre-authorizes exactly this* (owned-
first hitting set, exact to ~20 free vars, certify by one eval pass).
(c) Bounded rule-local abductive why-not for underivable atoms (today:
`known:false`, dead end — `why.rs:173-179`): which rules could derive it,
first failing premise each, depth-capped. *Evidence:* no commercial system
ships why-not or repair (F5) — open surface where DLI is structurally
cheap; agents iterate on "why didn't X follow" loops. *Cost:* (a) **S**,
(b) **S–M**, (c) **M–L** (new eval-side bounded backward search). *Risks:*
repair output must stay data, never advice (koan 3); caps surfaced, never
silent (grades law). **[contemplated: spec T6 rows; unbuilt]**

**E5. Evidence/source-span attachment.** Optional evidence member (quote,
span ref, or content hash) on a fact's provenance, returned by
query/why. Today evidence text never enters the engine, so the store is
not self-contained for audit — the harness joins `why.rendered` provenance
ids against its own quote dicts at read time. Hash-only mode for privacy-
constrained corpora. *Cost:* **M** (schema v5 field + read plumbing).
**[new; gap documented in harness architecture]**

**E6. Derived-fluent inertia (DL-0081 boundary).** Both prerequisites
shipped in one release window (add_rule 1.7.0, declare_inertial 1.8.0);
the recorded un-lift trigger — a consumer authoring rules that produce
fluent values needing persistence — is the natural next consumer ask.
*Cost:* **M**. **[contemplated: DL-0081]**

### Pack F — Memory-product packaging (rides direction C; no kernel change)

**F1. The memory facade.** `remember / supersede / why / recall` mapped
onto existing ops with source-tag grades (user-said / agent-inferred /
imported / policy → asserted/proposed + provenance), supersession =
defeat, promotion = provisional→ratified. *Evidence:* mem0 #4956 is close
to a spec of this; three startups are pitching fragments of it in-thread;
none has replayable labels. *Cost:* **S–M** (adapter + docs). *Risks:*
buyers expect embedding recall too — scope honestly as the write-path
arbiter + deterministic reads; pair with a retrieval layer, don't fake one.
**[new]**

**F2. MCP memory-server compatibility bridge — client-side only.** A
separate adapter process speaking the official MCP memory server's verbs
(create_entities/create_relations/add_observations/read_graph/search) and
*emitting `author.*` calls* — never a second engine-side surface. The
charter budget is one interface (koans 7/11, DL-0043); a synonym verb set
served by the engine would violate it, so the compliant shape is the same
one the charter's own form-editor ruling prescribes: a client of the tool
surface. *Evidence:* the official server's loudest issues are all
storage-path/persistence reliability —
[#1018](https://github.com/modelcontextprotocol/servers/issues/1018) (23
reactions), [#692](https://github.com/modelcontextprotocol/servers/issues/692)
(15), [#220](https://github.com/modelcontextprotocol/servers/issues/220)
(13) — a large install base wanting the same trivial verbs on a real store.
*Cost:* **S–M** (adapter + a default vocabulary bundle shipped as
caller-owned declarations, framed as such per koan 3). **[new]**

**F3. Expiry: package what exists; do not price what has no denotation.**
The honest version of this item is smaller than the demand suggests.
Per-fact bounded validity is *shipped*: `add_state` takes
`time {kind: until, point}` and `as_of` holdings carry the derived
boundary — the memory facade should document and use it. But the demanded
form (mem0 #5330, 17 comments; incumbent clock bugs #6931) is **wall-clock
TTL**, which is excluded by ruling (metric time, spec 03) — that demand
does not transfer; a caller wanting dates must model them as declared
timeline points. And a per-predicate *default* expiry declaration has no
obvious qualitative-time denotation (a default cannot name a timeline
point in advance, and "expires at the next distinct certain value" is
`declare_inertial`, already shipped). So: docs + facade wiring now
(**S**); `declare_expiry` is an open design question, unpriced, entering
only through a DL entry that supplies a denotation. **[substrate shipped;
the additive half unresolved]**

**F4. Review-queue surface (poisoning defense).** List-pending /
promote / defeat over proposed-standing claims + the structured diff as
the review artifact — the write-review security control MemGhost-class
attacks argue for. Must stay data-shaped workflow (DL-0054; and per Jeff's
standing ruling, nothing hangs on proposed-vs-ratified — this is optional
consumer workflow, never an engine gate). *Cost:* **S** (query surface +
SKILL.md). **[new; machinery shipped]**

### Pack G — Performance lane (measured triggers; mostly keep parked)

**G1. Commit-type-directed incrementality (insert-only fast path).**
Classify each staged delta: pure-monotone commits (asserts only, touching
no negation stratum, no defeat/retract/rule change) continue semi-naive
saturation from the cached fixpoint; anything defeater-carrying falls back
to full resaturation. *Evidence:* the 2026 dichotomy paper formally
licenses insert-only maintenance in absorptive semirings; DLI's own
root-cause note names defeater deltas specifically. *Two disclosures the
build decision needs:* (1) this is the **one item adjacent to a closed
line** — EVALUATION §G closed "`compile_incremental` wiring into the
lorestore" as it existed; the licensing here is a materially different
mechanism (a monotone-only classifier under new theory), which is exactly
the §G re-entry standard, but say so out loud. (2) DL-0089 §2 records the
engineering blocker: the parent-delta path is byte-identity-blocked by
construction (PackProvenance diverges 2-docs-vs-1) and is deferred to
GH #92, which requires an append-to-the-last-document loader entry point
and a byte-identical reconstruct-delta primitive — those primitives are a
prerequisite and belong in the price. *Discipline (Jeff's review
philosophy):* measure the assumption first — count what fraction of real
session commits are monotone; if defeats interleave everywhere, the path
rarely fires. The classifier must check *strata*, not op types (a new fact
in a negated subgoal's relation is non-monotone for consumers above it).
*Cost:* **M–L** after the measurement, GH #92 primitives included. **[new
mechanism; adjacent machinery contemplated in DL-0089/GH #92]**

**G2. Proof-height annotations for cheap lazy why.** One small int per
derived fact (min proof height = iteration number), maintained through
commits; scope why-reconstruction to update-affected facts — the
Zhao/Subotić TPLP-2025 shape, the literature's only working answer to
provenance-after-update. *Cost:* **S–M**. **[new]**

**G3. Affected-cone sketches.** Predicate-level static cone from the rule
graph (nearly free) + per-commit touched-key sketches (IMP, EDBT 2026) to
bound partial resaturation and pre-filter changefeed scopes. Cones must
follow negative edges both ways. *Cost:* **M**. **[new]**

**G4. Keep parked, triggers as recorded:** per-derivation provenance
(GH #89 — the measured next lever but High cost, research-shaped);
verdict-only Z-set lane (**L**, justify only if changefeed latency
measurements demand it; results must be contract-marked verdict-only);
Phase-2 posting lists (behind DL-0091's named measurement); DIST-1 seam
(its trigger is half-fired — SubtleMemory traces exist — but wants the
recorded trace artifact first); compaction/packfiles (measured-degradation
trigger; archive-never-delete); worker cancellation (second slow-path
trace). **[all contemplated; listed for completeness]**

### Pack H — Interop & trust

**H1. JSON-LD/RDF export target.** `ProjectionRow` is already
triple-shaped (`projection.rs:41-58`); the export discipline prices a new
format explicitly. Design work is the polynomial→reification mapping (or
certain-layer-only, honestly lossy). *Cost:* **S–M**. Earn it from a
demonstrated consumer per DL-0002 — don't build speculatively.
**[contemplated: export discipline; target new]**

**H2. Signed commit provenance (Fluree-shaped).** Caller-supplied JWS over
canonical commit bytes, stored as data, verified offline. Procurement
strengthener for the compliance story; weakest daily-use pull in the menu.
*Cost:* **M**. **[new; defer until a partner asks]**

**H3. Erasure: write the design note now, build nothing yet.** The honest
menu: (a) key-indirection pattern documented (sensitive values in a side
store, hashes in DLI) — available today, zero code; (b) standing-level
tombstone (defeat + killed-form-style naming) — mostly exists; (c)
byte-level redaction/crypto-shred with tombstoned hashes — **L**, collides
with replayable-judgments and needs its own DL ruling with a degraded-grade
story for verdicts whose support was erased. DLI's labels can compute the
exact derived-fact cascade ("zombie memory" set) that competitors
hand-wave — worth saying in any compliance conversation even before (c)
exists. **[new; blocking question for direction A procurement]**

---

## §4 Non-extensions (asked for by the market or tempting; refused by the constitution or the record)

- **In-engine similarity/canonicalization, resolution thresholds, "smart
  merge."** Closed by measurement (EVALUATION §G); koan 3. `entity_candidates`
  stays edit-distance over *declared* entities.
- **Trust weights, winner-selection, in-engine review gates, RBAC.**
  DL-0054/DL-0062: policy is consumer-owned. The consensus *referee* read
  (report conflicts + what each valuation concludes) is fine; choosing is not.
- **Wall-clock TTL, calendars, metric time.** Excluded by ruling
  (spec 03 §exclusions). F3's expiry is timeline-point-anchored.
- **Push daemons / server-side user-programmable projections.** Offline
  law + the EventStoreDB negative precedent. Changefeed is pull-cursor only.
- **A query language.** XTDB's lesson: named ops + closed pull patterns.
- **Probabilistic/confidence lattices via floats.** LS-1 bars floats;
  grades + three-valued standing are the uncertainty surface.
- **Write-back/bidirectional export sync.** Koan 10.
- **Eager incremental label maintenance under negation.** Proven wall
  (F4); only G1's monotone side door and G4's parked research shapes.

---

## §5 Prioritization

**[ASSESSMENT — the whole section.]**

The strategy doc's sequence stands: Experiment 1 gates everything. The
feature menu slots in around it in four moves:

**Move 1 — Experiment 1 launches first; only its cost reducers ride
alongside.** The strategy doc's "run this before anything else" stands
un-softened. Two lanes:

*1a — licensed alongside the experiment, because they are in its critical
path:* **D2 + D1** (declare_schema manifest + batch add — the
vocabulary-on arm *is* a schema pack + corpus lowering; these shrink the
experiment's own harness and become the vertical's onboarding artifact)
and **B1 + D3** (scoped valuation + ergonomics batch — recorded harness
warts, days each, all touched by the experiment's adapter).

*1b — bounded-regret fill-in; must not delay the experiment:* **A1 + A3 +
A2** (suppose-false, what_if, impact — the highest leverage-to-cost ratio
in the menu, the engine's unique capability surfaced for one boolean and
two thin ops) and **E4(a,b)** (min_why wiring + T6 repair sets;
abstention-with-witness → abstention-with-*repair* is the demo line nobody
else can say). Their stated payoffs (B's counterfactual splits, A's
demos) sit behind the Experiment-1 gate — the reason they may be built
early anyway is bounded regret: per the kill-condition analysis below they
survive an Experiment-1 failure (they serve B-as-negative-result and the
fiction consumer), and they are S-cost. But if scheduling forces a choice,
the experiment wins.

**Move 2 — with the benchmark build (B of the strategy doc): the audit
spine.** Lead with the **valuation-versioning design decision** (Pack C's
honesty condition), then **C1 → C2 → C3/C4** in that order — diff first
(Dolt's lesson: diff is the sticky form), changefeed as its cursor
iteration, time-travel + history once rulings are versioned. C1/C2 are
also what ClawArena and MemSyco adapters need, so the benchmark work
double-pays. **B2** (dossier) lands here as the context-pack read.

**Move 3 — with the vertical/design-partner motion (A): the semantics
pack.** **E1+E2** (constraints + value domains) is the biggest
engine-side win for the vertical thesis — it converts the FF ceiling's
"external incompatibility gate" into declared vocabulary and is rated
low-lift by the repo's own design note; **E3(a)** (reinstate) because
regulated workflows are appeals-shaped; **E5** (evidence spans) because
audit without the quote is half an audit; **D4** (identity op) when any
pilot accepts aliases at volume; **H3** (erasure note) *before* the first
procurement conversation.

**Move 4 — with direction C (MCP distribution): the memory product.**
**F1–F4** package what exists; they are positioning work riding B's
credibility, exactly as the strategy doc prices C. G-lane items fire only
on their recorded measurements (G1's monotone-commit census is the one
cheap measurement worth running early, since a positive result compounds
every ingestion workload).

**What not to do first:** anything in G4's parked list ahead of its
trigger; H1/H2 without a consumer; E3(b) without its ledger ruling; C3
marketed as audit before the valuation-versioning decision; and no
capability that lets the engine *choose* — every op above returns data and
findings, and the caller stays sovereign.

**Kill conditions.** If Experiment 1 fails, Packs D/E lose their vertical
payer but A/B/C survive (they serve the benchmark-as-negative-result and
the fiction consumer). F1/F3/F4 die with C's kill condition (market
refuses vocabulary declaration); **F2 does not** — it exists precisely for
users who never declare vocabulary and is the surviving hedge if C's kill
fires; its own kill condition is the compat install base refusing to move
stores even for the reliability it is asking for. G1 dies if the
monotone-commit census comes back low, and may not enter at all without
its GH #92 prerequisites priced. The changefeed (C2) dies only if C1's
diff sees no consumer — watch whether ClawArena's adapter actually
consumes it, since that is the first real changefeed customer in hand.
