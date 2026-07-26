---
status: live
verdict: "The append-only decision ledger. It outranks every other document in this repo. DL-IDs are minted here and nowhere else."
---

# Decision ledger

This file is the authority of record for De Lore Is. Every settled decision is a
`DL-nnnn` entry below.

**This ledger outranks all other docs.** When any document — charter, plan,
spec, README, contract, design note — disagrees with an entry here, this file
wins and the other document is stale until fixed. A spec is an elaboration of
the decisions; it is never a decision.

Discipline:

- **Append-only.** Never edit or renumber an existing entry. A decision that
  changes is superseded by a new entry that names the one it supersedes.
- **DL-IDs are minted here only.** No other file invents a `DL-nnnn`. Mint the
  next number and use it in the same commit — an ID that exists but decides
  nothing, or a decision cited before its entry lands, is a process error.
- **Single writer.** One agent writes this file per commit. Concurrent edits do
  not merge; they collide.

Each entry carries an id, a date, the ruling, a short rationale, and a revisit
trigger where one applies.

---

## DL-0001 — The name: De Lore Is

*Date: 2026-07-19*

**Ruling.** The language is **De Lore Is**. Source files carry the `.dli`
extension. The compiler-and-server binary is `deloreis`. Worker-protocol
operations live under the `dli.*` namespace. The derived-world cache is
`.dli-cache/`. The name is a double reading: a woman's name — *Delores* — and
the opening of every declarative line the language exists to finish, *the lore
is …*. It is explicitly **not** a reference to any time-traveling automobile.

**Rationale.** Every line in the language is a declarative claim about what the
world holds; the name states exactly that. One memorable name fixes the file
extension, the binary, and the operation namespace together.

## DL-0002 — The charter is adopted

*Date: 2026-07-19*

**Ruling.** The twelve-koan charter (`charter.md`) is adopted as the
constitution of the language: four triads — truth, loss, form, growth. When
koans deadlock, the tiebreak is **truth > loss > form > growth**; within a
deadlock the resolution preserving the author's sole authority wins first, and
at equal authority the resolution that adds less wins. The budget has named
lines: total size ~15–25K lines of code (tripwire ~40K), **two** authoring
doors, **zero** foreign certifiers with **one** rented baseline, and export
targets earn maintained profiles only from demonstrated consumers.

**Rationale.** A koan earns its place only if two designers' hands decide novel
questions identically; the growth triad closes the class of expansion questions
(doors, owned parts, served targets) the earlier charter left silent.

**Revisit trigger.** A new koan is added only on demonstrated need — see the
parked candidate recorded in the charter.

## DL-0003 — AST-first normativity

*Date: 2026-07-19*

**Ruling.** The language is defined **AST-first**: the AST's JSON Schema is the
normative definition of the language; the two-page EBNF is a rendering of it,
published in the same versioned document. There is **exactly one** canonical
surface rendering, defined by the formatter and enforced by the gate;
non-canonical text is always legal *input* and never legal *repo state*. The
grammar is versioned; a version bump ships a canonical-to-canonical migration,
and **the formatter is the migrator**.

**Rationale.** Deriving the schema from the definition, the surface from the
schema, and the migration from the formatter removes every seam where the
definition, the parser, and the API surface could drift apart.

## DL-0004 — Two doors

*Date: 2026-07-19*

**Ruling.** There are two supported authoring doors, one per author population.
**Door H:** humans write surface text, reviewed as surface. **Door T:** agents
author through semantic MCP tools against the `deloreis` server. Raw
AST-document emission is demoted to an internal mechanism inside Door T's tools
and to a benchmark arm — it is not a door. Textual agent editing of `.dli`
files is a **measured fallback**, not a door. Grammar-constrained local emission
is a **benchmark condition**, not a door. The benchmark exists to close doors,
not to open them.

**Rationale.** In the sibling projects the count of supported paths predicted
maintenance load and failure surface better than any path's individual quality.
One door per population bounds what is *supported* without narrowing what the
gate *accepts* — off-door input meets the same pipeline and graded errors.

## DL-0005 — Owned-first reasoning

*Date: 2026-07-19*

**Ruling.** Anything whose verdict certifies canon is owned, small,
deterministic, and replayable. The CP-SAT seat and the clingo generative seat
are **vacated** in favor of an owned searcher and an owned enumerator. clingo is
retained in exactly one role: a standing **differential oracle**, lab-side,
never in a gating lane or the serving path. Through the medium term there are
**zero foreign reasoning processes** required to build, gate, or serve. Flip
conditions discharge in two stages: the small owned system is built first, and a
named foreign proposer lands only when the owned system measurably fails on real
workloads — and then as a proposer behind the judgment boundary, never as a
certifier.

**Rationale.** The certification path must stay small enough to be itself
checkable. Foreign components may propose candidates or serve as outrun
baselines; they may never certify.

**Revisit trigger.** Per reasoning task, the two-stage flip: episode-order
search with soft/disjunctive constraints (owned searcher, then CP-SAT);
generative ruleset enumeration (owned enumerator, then ASP); a ratified
arithmetic law (the SMT decision, made then).

## DL-0006 — The verdict lattice

*Date: 2026-07-19*

**Ruling.** The grade lattice is **checked > tested > attested > asserted**,
carried over two orthogonal channels: a **support** channel (the Boolean
expression over citations and assumptions — what a claim rests on) and a
**grade** channel (how the procedure stacked on that ground was verified).
Rollup is weakest-grade, taken as the minimum over the *procedure chain*, never
over the citations. **Attestations are check-evidence** — side-band on the
checks that consume them — and are **never canon support**. No amount of model
consensus lifts a grade.

**Rationale.** The lattice ranks the recheckability of evidence; the ledger
ranks authority. `asserted` is the strongest word and the weakest evidence: it
is grounded in will, and there is nothing to re-run.

## DL-0007 — The GDL proof point

*Date: 2026-07-19*

**Ruling.** The dli-hosted Turning ruleset must export **soundly** to Ludii's
L-GDL. Soundness is verified **differentially**: the `deloreis` simulator and
the Ludii runtime replay the same seeded battery, and their legal-move sets and
terminal verdicts must agree. Ludii is a **rented baseline**; any disagreement
is a finding against the export profile or the kernel, both worth having.

**Rationale.** This is the cheapest external audit of the claim that the
Turning's rules-as-lore are really rules — if the export is not trivial, the
rules were entangled with the kernel; if it is not sound, the simulator was
wrong. Either finding pays for the profile.

## DL-0008 — Tooling

*Date: 2026-07-19*

**Ruling.** The build uses: a **hand-rolled recursive-descent** parser (the
grammar is two pages, with per-line error recovery); **miette** from day zero,
because diagnostics are the product; **schemars 1.x**, so the JSON Schema is
derived from the Rust types and cannot drift from them; **insta** for diagnostic
snapshots, because the errors are an API; **proptest** for property tests;
**nextest** as the test runner; **cargo-deny** for the dependency gate. Not
adopted, each with its inline promotion trigger: **salsa** (trigger: measured
rebuild latency breaching the CI envelope — see DL-0009), **rowan** (trigger:
the grammar outgrows two pages), **petgraph**.

**Rationale.** Small, deterministic, offline tools whose output is the product
surface. Parser generators and incremental frameworks earn adoption only at a
stated, measurable size or latency trigger — not in advance.

## DL-0009 — Salsa-readiness without salsa

*Date: 2026-07-19*

**Ruling.** The kernel is written salsa-**ready** without adopting salsa: pure
phases, per-invocation environments, and no stateful `Compiler` struct that
holds derived state across calls.

**Rationale.** Purity keeps incremental recomputation a later drop-in rather
than a rewrite; the framework is deferred until measurement demands it.

**Revisit trigger.** Adopt salsa when measured incremental-rebuild latency
breaches the CI envelope.

## DL-0010 — Contract authority

*Date: 2026-07-19*

**Ruling.** `deloreis` owns the `contracts/fantasy-worker` contract. The fantasy
repo vendors it **pinned and digest-checked**. The contract document outranks
any reimplementation of it on either side.

**Rationale.** One authoritative contract, vendored under a digest, prevents two
copies of the boundary from drifting.

## DL-0011 — Docs discipline

*Date: 2026-07-19*

**Ruling.** Frontmatter carrying `status` and `verdict` is **mandatory** on
every document under `docs/`, and on `SKILL.md`. The repo-root guides
(`README.md`, `AGENTS.md`) are exempt: they carry an in-body status line
instead, matching the root-guide convention of the consuming repo. The decision
ledger outranks all other docs (DL-0011 restates DL-0002's constitution for the
docs layer specifically). The docs tree is **flat** — no generated `INDEX`
files, no index generators. DL-IDs are minted **single-writer** in the ledger,
mint-and-use in one commit.

**Rationale.** A stale doc reads as current. Mandatory frontmatter plus a single
outranking ledger make authority legible without an index-generation apparatus
to maintain and keep honest.

## DL-0012 — Migration protocol

*Date: 2026-07-19*

**Ruling.** Source migration runs **shadow-read first**: the pinned legacy
oracles run beside the kernel over vendored corpus snapshots, with a **mutation
detection corpus**, and **detection is proven separately** from parity — a
harness that only ever agrees is a null oracle until it is shown to catch seeded
divergence. A **per-check flip** requires 20 consecutive clean dual-runs, at
least one real ruling propagated through the check under dual-run, and detection
parity. Source migration is **three-phase and ends in deletion** of the legacy
source — not in a generated mirror. Post-flip divergence rolls the check back.

**Rationale.** Parity without proven detection certifies nothing. Deletion —
rather than a second writable representation of the same facts — is what
prevents drift after cutover.

## DL-0013 — CI quarantine

*Date: 2026-07-19*

**Ruling.** The language gate is **deterministic offline lanes only**. Models,
the Ludii baseline, and benchmarks run in **non-gating** lanes.

**Rationale.** The gate must reproduce with no network and no model in the loop;
anything flaky or live is quarantined out of the verdict that blocks a merge.

## DL-0014 — The tool door

*Date: 2026-07-19*

**Ruling.** Agents author lore through **Door T** — a curated set of semantic
tools registered in MCP against the opaque `deloreis` binary: `author.open`,
`author.add_entity`, `author.add_fact`, `author.add_relation`,
`author.add_event`, `author.add_before`, `author.open_question`,
`author.add_option`, `author.add_hypothesis`, `author.suppose`, `author.status`,
`author.diff`, `author.discard`, `author.materialize`. Every mutating call
returns, immediately: graded **findings** (with repair data), the **rendered
canonical lines**, and the **workspace digest**. `author.materialize` returns
`.dli` file contents **as data**; the calling agent commits them through its own
review. Raw `.dli` editing by agents is discouraged in the skill.

**Rationale.** A prior in-house mutation-to-meaning attempt via raw structural
editing shipped unmeasured and failed; a small curated MCP tool surface
succeeded. Per-tool small schemas are natively enforced by the providers, and
immediate graded feedback replaces guessing.

## DL-0015 — xtask gate machinery

*Date: 2026-07-19*

**Ruling.** *(Recorded on the infra agent's behalf.)* The CI-parity gate is an
**xtask** — compiled Rust run as `cargo xtask gate` — rather than a `gate.py`
script; the full gate runs as a Rust test. Bash is retained only for the three
git-orchestration scripts (`safe-commit.sh`, `greengate.sh`, `ci-wait.sh`).

**Rationale.** A compiled gate is itself testable and shares the project
toolchain; a scripting layer is kept only where it orchestrates git, which is
not the language.

**Revisit trigger.** Add a general scripting layer only if the repo goes
polyglot.

## DL-0016 — Surface v1 naming — the does-what-it-says pass

*Date: 2026-07-20*

**Ruling.** The surface keyword set is renamed so every reserved word says what
the line does in the plainest word a working fiction writer already owns
(`docs/surface-v1-naming.md`, status resolved). The rename set, approved in
full: `fluent`→`state`, `wheel`→`cycle`, the in-practice proof block
`ledger`→`proof`, `morpheme`→`name-part`, `lexeme`→`word`, `register`→`dialect`,
`sort`→`kind`. The file header `pack` is renamed to `scope` (the header stays).
The rule family **unifies** under one `rule` keyword with its domain stated in
the form — `rule … over order` (was `order-rule`), `rule … over season` (was
`craft`), and a bare `rule` for a world rule — retiring `order-rule` and `craft`
as keywords. Indentation stays with an explicit **autoformat guarantee**: the
parser is indent-tolerant, the formatter owns all layout, and no diagnostic ever
asks a writer to fix indentation. Every retired word becomes a **killed form**:
the parser refuses it with a domain-spoken *renamed-to* diagnostic
(`error: `fluent` was renamed — write `state``) rather than accepting it or
reading it as an unknown keyword.

**Rationale.** The author's first surface review found the keyword terminology
incoherent: `fluent`, `wheel`, `craft` are non-obvious; `morpheme`/`lexeme` are
linguistics terms of art; `register` is borderline; `pack` is unjustified; the
lone compound `order-rule` is out of character. The whole syntax should read
"does what it says" — no terms of art on the writer path, and no fantasy flavor
in the machinery (the flavor belongs to the world being described). The rename
is cheap while the grammar is unfrozen and expensive after.

**Revisit trigger.** The grammar freeze (SURF-4); after it, any surface change is
a versioned canonical-to-canonical migration, not an edit.

## DL-0017 — The general/TPtH separation: engine, craft-doctrine config, world content

*Date: 2026-07-20*

**Ruling.** De Lore Is ships as a general-purpose standalone tool; "To Pleach the
Hedgeway" (TPtH) is **one** world authored in it. In the author's words: *"you
need to begin to tease apart the general purpose aspects of dli that could be
used with any story and the tpth ones. i will eventually be releasing dli as a
general purpose standalone tool used by many different fictional worlds. now is
the time to start to put the tpth stuff in a clearly organized modular
location."* The repo separates into **three tiers**:

1. **The engine** (the `dli-*` crates + the `deloreis` bin) — generic mechanism
   only: the annotation algebra, the loader/checker/fork engine, the time model,
   the practice **workbench**, the language **oracle**, the season-ledger rule
   **engine** and KB-loader, the simulator, the export core, the server + Door T.
   It is story-agnostic **and** doctrine-agnostic: it takes world content and
   craft configuration **as data**. An engine crate's `src/` may not hardcode any
   world's proper nouns, any project's craft doctrine, or any consumer's decision
   ids (`D-nnnn`) outside a doc-example explicitly marked as illustrative.

2. **Craft doctrine** — *"for all of the craft stuff, it needs to be differently
   modular than the tpth stuff. basically a config of some sort of what sort of
   things to turn on or off. e.g. sanderson's laws of magic, kishotenketsu, etc.
   are all project specific. it doesnt correspond to a world but rather the
   optional bits of writing craft the user chooses to enable."* Craft is a
   **togglable module library** — generic, published craft wisdom (Sanderson's
   laws, kishōtenketsu, MICE, tryfail-before-triumph) as principle-record packs a
   **project** enables a subset of. It is **not** world content: kishōtenketsu is
   a craft lens any world's project may turn on. The current 107-principle
   `kb::PRINCIPLES` static is really *TPtH's chosen configuration* of these packs
   (its mission-filters, the D-0128 pillars, `D-nnnn` citations), frozen into
   engine Rust — it comes out.

3. **World content** — one fictional world's lore: for TPtH, the Turning card
   game, the Concordance / spoken-proof practice, the cymric-fringe dialect and
   its names, the saltmarket corpus, the banned-vocabulary rationale, the
   `D-nnnn` authorial ledger. It lives in a clearly organized modular location
   and is loaded as data.

**A project = a world + a selected set of craft-doctrine modules + config.** TPtH
is one such project.

**The factory owns the decision→record mapping.** *"the factory side code should
be mostly responsible for mapping decisions to records in the craft kb."* The
engine holds the generic craft-rule evaluator, the principle-record schema, and
the KB loader; it consumes a craft KB as data. The act of **mapping a project's
`D-nnnn` rulings into KB records** — which doctrines are on, the mission-filters,
the pillars, the citations, the thresholds and rule severities — is a
**factory-side** (consuming-repo) responsibility, not dli engine code. The
engine never reads `canon/decisions.md`; the factory produces the config the
engine loads.

**Homes.** TPtH world content moves to a modular location (`worlds/tpth/` data
tree, and a thin TPtH crate only where content cannot yet be pure data — e.g. the
Turning's world-specific invariants/study). Craft-doctrine packs get their own
togglable-module home, distinct from any world. The engine keeps only mechanism
plus a **neutral generic teaching world** (no TPtH names) so a standalone user
can learn the surface without absorbing one customer's lore. The wire contract is
generic and gets a generic name (`dli-worker`), with the fantasy factory as one
consumer profile.

**Migration is incremental and gate-green at each step.** The full leak inventory
and the ordered move/extraction DAG live in the migration plan (a design
proposal); this entry is the authority the plan elaborates. Safe data-relocations
land first; the code-extractions (the Turning genericization, the KB→data move,
the season-rule parameterization, the contract rename, the doc split) follow as
tracked, individually-verified steps.

**Rationale.** A standalone dli must not assume TPtH lore or TPtH's craft
choices; a new world's author supplies their own world data, selects their own
craft doctrines, and — through their own factory — maps their own decisions into
KB records. Teasing this apart is cheap now, while the engine is young, and grows
expensive as more world content and doctrine bake into the crates.

**Revisit trigger.** The standalone release (the first non-TPtH world authored in
dli); the boundary invariant becomes a gate check at that point if not before.

## DL-0018 — Separation layout: a shipped craft library, a thin TPtH crate, a generic contract

*Date: 2026-07-21*

**Ruling.** Three layout forks under [DL-0017](#dl-0017--the-generaltpth-separation-engine-craft-doctrine-config-world-content), resolved by the author:

1. **dli ships a generic craft-doctrine library.** The togglable packs
   (Sanderson's laws, kishōtenketsu, MICE, tryfail-before-triumph) are curated
   generic craft wisdom that ships **with the tool** so "turn craft on/off" works
   out of the box; a project enables a subset. A world's own *selection and
   configuration* of them (mission-filters, pillars, citations, thresholds) is
   separate and, for TPtH, factory-produced under `worlds/tpth/craft/`.

2. **TPtH code that is not yet pure data goes to a thin `crates/tpth/` crate
   now.** The Turning's type vocabulary, its honest-vs-rigged study, and its
   world-specific invariants move into a small TPtH world crate that depends on
   the engine's generic seams — full modular separation immediately. Genericizing
   the simulator so the whole Turning becomes pure data is a **follow-on**, not a
   blocker.

3. **The wire contract is renamed `fantasy-worker` → `dli-worker`.** The generic
   contract owns the story-agnostic surface (schemas, registry, transport
   fixtures); the fantasy factory becomes one **consumer profile** on top, keeping
   the TPtH-payload conformance goldens as its own suite.

**Rationale.** Each choice takes the fastest path to a clean, standalone-honest
boundary without a large upfront refactor: a shipped starter library makes the
craft feature demonstrable immediately; a thin world crate modularizes TPtH code
now and defers the harder generics work; the rename makes the contract's already
world-agnostic surface honest about it.

**Revisit trigger.** Simulator genericization (fork 2's follow-on) may later empty
`crates/tpth/` of the Turning code in favor of pure data; the craft library's pack
set is revisited when a second world exercises it.

## DL-0019 — Rename the timing finding code `proof.wrong-hour` → `proof.wrong-phase`

*Date: 2026-07-21*

**Ruling.** The kernel proof-checker's L3 timing finding is renamed from
`proof.wrong-hour` / `ProofFinding::WrongHour` to `proof.wrong-phase` /
`WrongPhase`. "Phase" is the neutral, cross-world term for a position within a
cyclic coordinate (spec `03-time.md §1` speaks of "cyclic phase"); "hour" was a
TPtH-flavored word that leaked into a generic engine finding code. This renames
**only the kernel finding code** — a world's own diegetic error name (e.g.
`error wrong-hour refines L3` in TPtH's `spoken-proof`, or `wrong-moon` in
camelot) is world content and is untouched; the two are decoupled at
`failed_at()`/`refining_error()`.

**Rationale.** Pre-authorized by the TPtH/general separation
([DL-0017](#dl-0017--the-generaltpth-separation-engine-craft-doctrine-config-world-content),
[DL-0018](#dl-0018--separation-layout-a-shipped-craft-library-a-thin-tpth-crate-a-generic-contract))
as "a deliberate contract migration" — the engine's finding codes must be
story-agnostic. The finding code appears in no fixture or schema, so the migration
is a lockstep change of the `code()` string, the docgen source, and the
regenerated `finding-families.md` (the `families_are_covered_against_source` test
holds documented == emitted). No wire/registry regeneration is required.

**Revisit trigger.** None; the neutral term is stable. Any future world wanting an
"hour"-flavored timing error declares it as its own diegetic error name, never as
the kernel code.

## DL-0020 — Rule-body negation: open bare world rules, with `not` and certain-layer semantics

*Date: 2026-07-21*

**Ruling.** Bare world `rule` declarations are an **open authoring door**: the
loader lowers them into the typed world model (previously rule bodies were
captured text and dropped). A negated body literal is written with the keyword
**`not`** — `rule ward-holds($g) : seated($g), not breached($g)`. The evaluation
semantics are those already specified in `02-semantics.md §4`: a negated literal
`¬b` tests the **certain layer only** (a merely-`proposed` `b` never blocks the
derivation) and **poisons** — the open-variable monomials of `b`'s positive
support enter the conclusion's defeater polynomial `P⁻`, so the conclusion is
*certain-at-risk*, never negation-as-failure. Two load-time disciplines guard the
form: **range restriction** — every variable in the head or in a negated literal
must also appear in a positive body atom (a negated literal binds nothing) — and
**stratification**, which refuses a negative-dependency cycle
(`p :- not q / q :- not p`) as a load finding rather than evaluate it.

**Rationale.** The author ruled `not` (over `unless`/`absent`) as the negated-body
keyword — datalog-standard, least surprising, and what the engine's own tests
already assumed — and chose a full build (open the door + implement the evaluator
semantics) over deferring. The semantics were pinned by the spec before any code;
this ruling records the surface keyword and the decision to open bare world rules
as an authoring door, which flips the prior "rule bodies are inert text" contract.

**Revisit trigger.** A future need for disjunction or existential negation in rule
bodies; a `requires`-style guarded body form (currently the parser leaves such
bodies unlowered). Multi-authority or arithmetic negation stays out of v1
(DEF-3/DEF-4).

## DL-0021 — Wire check-proof / simulate / export; migrate `dli.check-proof` to the rich `ProofReport`

*Date: 2026-07-21*

**Ruling.** The three honest not-yet commands are wired to their (already-built)
engines at BOTH the CLI and the server, and their server ops leave the gated set.
This carries a **frozen-contract migration**: `dli.check-proof`'s
`CheckProofResult.report` changes from the placeholder thin `LedgerReport`
(`{ violations: [] }`) to the rich `dli_ledger::ProofReport` (steps, findings,
verdict, expectation, grade), with the response schema regenerated and the golden
fixture rewritten to the real report shape. `check-proof` builds its scene via the
new `CompiledWorld → SceneProjection` builder (DL-0019's companion); `export`
renders ContextPack from the certain layer and Cards/Ludii from a game ruleset;
`simulate` runs a practice or deal battery to completion and returns an honest
manifest. `GATED` shrinks from 11 ops to 6 (only `fork.add` + `timeline.*` remain
unwired).

**Rationale.** The author ruled item 2 as "CLI + full serve un-gate" — the engines
(dli-ledger proof checker, dli-sim, dli-game, dli-export) had all shipped, so the
honest not-yet stubs were stale; wiring them is the intended completion. The
`ProofReport` migration is the one frozen-contract change and was pre-enabled by
adding the wire-derives (DL-0019's commit). The server ops carry only `{document}`,
so the worker resolves a small set of builtin demo worlds (a `spoken-proof`
practice corpus and a `french-deck` ruleset) rather than churn the cross-lane
camelot fixtures — a lane-local resolver extension, not a contract change.

**Revisit trigger.** If the worker gains a real corpus-passing channel, the
builtin demo resolver is replaced by the passed world; `simulate` becomes
asynchronous if a cross-process job store lands (today it runs synchronously and
`status`/`cancel` read/drop an in-session job).

## DL-0022 — Defer the authoring benchmark (BENCH-1 / BENCH-2)

*Date: 2026-07-21*

**Ruling.** BENCH-1 (the authoring-benchmark rig) and BENCH-2 (the two-door run
that yields the "which door to keep shut" verdict) are **deferred** — parked, not
dropped. No rig code, no run. The engine build is otherwise complete (the core,
plateau, integration, and stretch lanes, the separation, and now the CLI/serve
wiring and rule-body negation all landed and gate-green), so the door-decision
benchmark is the remaining dli-internal piece — and it is not pure engine work.

**Rationale.** Two reasons it waits. (1) It informs SURF-4 (the grammar-gate
freeze), which the plan holds open pending the **human-writer probe** — the
author's own act; running the benchmark before that probe would freeze a surface
on machine evidence alone. (2) It is **live-LLM** work: non-gating, CI-quarantined,
and requiring an authorized run with a hard spend cap (Bedrock-only, proven
`flukeball-bedrock`; never OpenRouter/xAI). Deferring keeps the engine's offline
credential-free gate intact and avoids spend on a verdict the author is not yet
ready to consume.

**Flip condition.** Build BENCH-1 then run BENCH-2 when BOTH the human-writer
probe is prioritized AND a live-LLM door-decision run is authorized with a spend
cap. Details: `design-notes/bench-deferral.md`; the PLAN nodes are marked
`*(deferred.)*`.

## DL-0023 — Door T read tools: 5 non-mutating `author.*` queries

*Date: 2026-07-22*

**Ruling.** Door T's served `author.*` set (DL-0014's 14 mutating tools) is
extended with **5 read tools** — non-mutating queries that let an agent inspect
workspace and world state without staging anything:

- `author.query` — query the compiled world (pattern-match over derived atoms).
- `author.check` — run the full check battery and return graded findings.
- `author.why` — explain a derivation (the why-not provenance trace).
- `author.findings` — retrieve the current findings for the workspace.
- `author.list_workspaces` — enumerate available workspaces.

The served `author.*` tool set is now **19 tools total** (14 mutating + 5 read).
These tools were implemented during the DL-0021 wiring wave but never ledgered;
this entry records them.

**Rationale.** An agent that cannot inspect the world it is authoring into must
guess at context, provenance, and derivation state. The read tools close the
inspect-before-you-act loop: query discovers what exists, check reveals what is
wrong, why explains how a conclusion was reached, findings retrieves the current
error surface, and list_workspaces orients a cold-starting session. None of them
mutate the workspace; none carry `expected_digest` or `idempotency_key`.

**Revisit trigger.** Additional read tools (e.g. `author.schema`, `author.history`)
land as new ledger entries if the tool set grows beyond 19.

## DL-0024 — Shipped negation semantics: valuation-neutral poison, present-but-Possible defeat

*Date: 2026-07-23*

**Ruling** (supersedes DL-0020 mechanics wording): the evaluator's negation
semantics are valuation-neutral all-admissible-monomials poison. A negated atom
`¬b` contributes every non-contradictory, theory-respecting monomial of `b`'s
support `P⁺` to the conclusion's defeater polynomial `P⁻`; the conclusion is
present-but-Possible (defeated) rather than absent. No derivation is blocked;
no row vanishes.

**Rationale:** the fork subsystem evaluates by valuation overlay with no
re-fixpoint (`fork.rs:7`; O(1) forks, koan 2, ENG-2). Blocking semantics are
only sound if every fork evaluation re-runs the fixpoint; without that the
blocked-conclusion path forges certainty through `fork.suppose`/`diff`/
`preflight`. Valuation-neutral poison is the semantics that survived two
adversarial red-teams (b8243cc, 13605cd) and keeps why-blocked explanations
alive (koans 4 and 9).

**TQ-N2 over-approximation acknowledged:** defeat is not propagated through
negation; the defeater set is a superset of the minimal. This is a conservative
over-approximation (never forges certainty, may over-report risk). Follow-up
guarded by the TQ-N3 property test.

**Revisit:** when TQ-N3 property demonstrates full commutation, the
conditioned-support refinement of TQ-N2 may narrow the defeater set.

## DL-0025 — Canonical `.dli` carries no comments; teaching commentary lives in docs

**Ruling (1):** The formatter's comment-free canonical form is ratified. `.dli`
files committed under `examples/` and `worlds/` carry no `#` comments; the
`deloreis fmt --check` gate step enforces this. Comment-preservation is not a
pre-freeze grammar feature (deferred to SURF-4 as an explicit author question).

**Ruling (2):** Historical teaching commentary is migrated to the docs channel
(`docs/cards/examples-walkthrough.md`), not dropped. Fenced `.dli` blocks in
that doc are gate-checked by the docs-oracle. Declarations that need human
labels use the AST `label` field, not comments.

**Rationale:** comments-in-AST would either pollute the JSON Schema (DL-0003:
the schema is the language definition) or become second-class trivia with
special-case plumbing. Agents author through Door T and review rendered diffs;
a comment channel Door T cannot read is drift waiting to happen. The docs
channel is gate-checked and staleness is partially caught.

**Scope:** generic-world commentary (camelot, norse) lives in
`docs/cards/examples-walkthrough.md`; TPtH pack commentary lives in
`docs/worlds/tpth/packs.md` (worldbook channel, oracle-checked). The
french-deck example is a JSON ruleset corpus — no `.dli` commentary existed.

## DL-0026 — Token-lint file-exemption policy; DL-0023 erratum

**Ruling (1):** File-level token-lint exemption scope. Design notes, the
charter, and audit/validation docs may be file-exempt from the `docs_tokens`
lint (they discuss or quote the world by nature). Normative teaching surfaces
(`docs/spec/`) may **never** be file-exempt — they are the generic surface
DL-0017 protects and consumer #2 will read. World-token mentions in specs use
the worldbook-footer paragraph mechanism (a paragraph containing a
`docs/worlds/tpth/` link self-exempts) or are genericized.

**Ruling (2) — DL-0023 erratum:** The 19 `author.*` tools comprise 11 mutating
ops, 3 read-only state ops (status, diff, materialize), and 5 read queries
(query, check, why, findings, list_workspaces). The contract §3.2 table is
authoritative for the mutating/read classification; DL-0023's loose "14
mutating" was editorial shorthand, not a count. Propagated copies corrected.

## DL-0027 — Ledger integrity is checked without rewriting history

*Date: 2026-07-25*

**Ruling.** Existing decision entries remain byte-for-byte immutable. Missing
metadata and accidental post-mint edits are corrected only by a later entry,
and the compiled gate checks entry sequence, dates, rulings, and append-only
history whenever the comparison revision is available.

*Corrected date: DL-0025 = 2026-07-23*

*Corrected date: DL-0026 = 2026-07-23*

DL-0025 was minted by `01b4957c3f95288053285fdad267c837392f8aca` without a
date. DL-0026 was minted by
`f51b819095f03ed799a3a23a25e8945e834ef5f3` without a date. Commit
`6aa87263a8a0d32e4e5a14a46b92f1d5e6c23c9e` later appended a scope paragraph
inside DL-0025 instead of recording it as a new decision. Those bytes are now
historical evidence; this entry records the defects rather than hiding them.

**Rationale.** An append-only authority cannot repair its own history by
silently rewriting it. A replayable checker plus an explicit corrigendum makes
the integrity failure visible and prevents a recurrence.

## DL-0028 — The standalone source release is open

*Date: 2026-07-25*

**Ruling.** The sentence in DL-0022 calling the engine build otherwise complete
is superseded. The kernel and domain engines are substantial, but the
standalone source release remains open until the PLAN status matrix closes its
local `open` nodes: the worker must load caller projects rather than embedded
demos, advertised operations must implement their contracts, evidence and jobs
must have their promised durability, and the assurance gate must test those
behaviors honestly. Consumer cutover, live Ludii comparison, the authoring
benchmark, the human probe, and grammar freeze remain separately deferred or
external-blocked; they are not release-completion evidence.

**Rationale.** A green gate over demo-bound handlers and exempt fixtures is not
evidence that a general caller can use the release. Status must follow
replayable behavior, not the existence of dispatch arms.

## DL-0029 — Accepted syntax may not disappear or become an inert declaration

*Date: 2026-07-25*

**Ruling.** Every parsed declaration or child clause either has defined
semantics or produces an error-class diagnostic before formatting or loading.
Question bodies accept only well-formed options; fork command bodies remain
part of the defined fork surface. A bare world rule must
be valid Datalog; checker-style `requires` clauses remain legal only in their
defined domain and practice contexts. The semantically inert
`option ... assumes` form is retired and registered as a killed form whose
repair directs authors to claim-level hypothesis provenance.

**Rationale.** Silently deleting source during canonicalization and silently
accepting declarations that lower to nothing both violate the loss and truth
triads. Keeping an unimplemented field in the normative AST would falsely make
it part of the language.

## DL-0030 — Capability claims follow complete runtime semantics

*Date: 2026-07-25*

**Ruling** (supersedes DL-0021's server-wiring and gated-set status): a worker
operation is advertised only when it implements its full documented semantics,
including required durability and restart behavior. Contract `0.3.0` therefore
capability-gates `dli.check-draft`, `dli.attest.missing`,
`dli.attestation-sweep`, `dli.payload-submit`, and all three `dli.simulate.*`
operations. Their typed reservation schemas remain available, and valid calls
return `not-wired`; malformed calls still fail schema validation first.
`dli.check-proof`, `dli.export.run`, `dli.fork.add`, and all five
`dli.timeline.*` operations remain advertised because their documented
semantics are implemented.

**Rationale.** Dispatching to a synchronous or in-memory approximation does not
implement a durable evidence or resumable-job contract. Capability negotiation
must describe behavior a general caller can rely on, not merely the existence
of a handler arm.
