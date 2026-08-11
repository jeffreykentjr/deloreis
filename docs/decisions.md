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

## DL-0031 — Contract 0.4.0 opens durable payload and attestation evidence

*Date: 2026-07-25*

**Ruling** (supersedes DL-0030's evidence capability state): contract `0.4.0`
advertises `dli.check-draft`, `dli.payload-submit`, `dli.attest.missing`, and
`dli.attestation-sweep`. Accepted fat payloads and attestation append batches
are RFC 8785 JCS documents, content-addressed by SHA-256 under the caller
project's `.dli-evidence/` root. Attestation batches form one verified,
unbranched parent chain, while supersession remains scoped to each configured
question. A project-wide advisory lock serializes writers; publication is
immutable, idempotent, file-synced, directory-synced, and atomically renamed.
Rejected evidence publishes no evidence file, and corrupt committed evidence
fails closed. The three `dli.simulate.*` operations remain capability-gated
until durable, restart-resumable jobs land.

This pre-v1 migration also makes RFC 8785 canonicalization real for Door-T
workspace digests and reserves the durable simulation request/response shapes;
the contract version therefore advances once from `0.3.0` to `0.4.0`.

**Rationale.** Content-addressed immutable evidence gives independent callers a
replayable cross-process artifact without granting the machine any canon write
authority. Advertising only the four operations whose complete storage and
validation semantics now exist preserves DL-0030's capability rule.

## DL-0032 — Evidence replay and payload acceptance fail closed across restarts

*Date: 2026-07-25*

**Ruling** (clarifies DL-0031 without changing the `0.4.0` wire version):
durable idempotency recognizes any previously committed attestation batch, not
only the current head. Every committed payload for a document is verified
before another payload is published. Attestation staleness fingerprints cover
the saturated subject relation, including derived standing and resolved
support, not source-atom spelling alone. Publication
temporaries live outside committed-only directories, newly created directory
entries are parent-synced, and stale interrupted temporaries are recoverable.

The advertised payload operations run every semantic family represented by
`payload/v1`: scene-keyed deltas are structurally and referentially checked;
phase observations are checked against declared cycles and contiguous walks;
the deterministic banned-word scan is independently recomputed; and a proof
shadow must match a declared proof and replay through its declared practice.
Optional outline and prior-season craft context is project-owned input declared
by `outlines` and `craft_ledger` manifest assets. The attestation schema now
states the positive cardinalities the runtime has always required, and dates
must be calendar-valid.

**Rationale.** Immediate-retry tests, clean-store tests, and shape-only payload
tests are not durability or semantic-completeness evidence. A general caller
must be able to retry after later writes, restart after an interrupted publish,
detect corruption elsewhere in the same store, and trust that no accepted
payload field was ignored.

## DL-0033 — Contract 0.4.1 makes evidence part of the document verdict

*Date: 2026-07-26*

**Ruling.** Contract `0.4.1` changes `dli.check` from a world-only finding list
to the complete replayed document verdict. The response carries `ok`, the
weakest procedure `grade`, and domain-tagged world, payload, and attestation
findings. Payload findings retain their source line, verbatim quote, principle,
citations, committed payload digest, episode, and source filename. Every
configured attestation question is consumed explicitly:
a current `yes` passes at grade `attested`; a current `no` fails at grade
`attested`; missing or stale evidence fails closed at grade `checked`. New wire
records use a closed Rust and JSON Schema `yes | no` type. The durable
batch-version-1 representation remains string-valued so existing `0.4.0`
evidence can be replayed; a legacy answer outside the closed set is stale and
requires re-attestation, never evidence corruption. Attestation result states
are a closed outcome union: each outcome fixes its code, severity, grade, and
required evidence fields rather than relying on correlated free fields.

This is a pre-v1 wire migration from `0.4.0` to `0.4.1`; it is not a silent
mutation of the earlier response schema. Request-level retries are therefore
version-scoped: an exact `0.4.0` request is not reinterpreted as `0.4.1`.
Durable readers still recognize and replay every committed batch-version-1
artifact, including batches written through `0.4.0`. Attestations remain
side-band check evidence and never become canon support.

**Rationale.** Persisting evidence without consuming it in the ordinary check
path leaves callers with a durable archive but no replayable verdict. Domain
tags prevent structurally overlapping finding shapes from deserializing into
the wrong variant, and retaining payload identity plus anchors preserves the
evidence a caller needs to render or repair the defect.

## DL-0034 — Contract 0.4.1 opens durable simulation jobs

*Date: 2026-07-26*

**Ruling** (supersedes DL-0031's remaining capability gate):
`dli.simulate.start`, `dli.simulate.status`, and `dli.simulate.cancel` are
advertised at contract `0.4.1`. A job id is the SHA-256 address of its canonical
specification. The specification, cadence checkpoints, cancellation marker, and
final manifest are project-owned evidence under `.dli-evidence/simulations/`.
Committed records are immutable and canonical; checkpoints are replayed from
their battery, seed, and trial count before use, and corruption fails closed.

Start publishes pending state before the transport flushes its response and
releases a background worker. Status verifies the durable head and schedules an
unfinished job after process restart. Cancellation is serialized by the
project evidence lock and takes effect at a checkpoint boundary. An exact start
retry reopens the same pending, running, cancelled, or completed job.

**Rationale.** A content-addressed job with replay-verified checkpoints can
survive a worker crash without treating process memory as evidence. Publishing
pending state before work begins makes acknowledgment honest, while
boundary-serialized cancellation preserves both resumability and an observable
cancel operation.

## DL-0035 — Simulation jobs cover project games and anchor terminal state

*Date: 2026-07-26*

**Ruling** (clarifies DL-0034 without changing contract `0.4.1`): the
`document` field of `dli.simulate.start` names either a project document's
declared practice or a project game ruleset. A generic game job deals the first
available layout and commits tested completion-rate evidence plus checked deck
conservation and legal-play sweeps. Rulesets requiring registered Rust verdict
predicates are refused through the standalone data-only door.

Checkpoint files form one digest-linked chain with the replay-verified head as
the only trusted accumulator. Terminal cancellation and completion records pin
that head and retain the immutable job specification after the live spec entry
is removed; deletion of a committed terminal record therefore fails closed
instead of reopening pending work. Trial computation runs outside the
project-wide publication lock, and each serve process runs at most four
simulation workers concurrently.

**Rationale.** Advertising a simulation capability that rejected manifest
games contradicted its ruleset contract. A standalone worker must also make
terminality observable after restart without turning process memory or an
unanchored marker into evidence, and a caller choosing checkpoint cadence must
not be able to monopolize unrelated evidence writes or spawn unbounded threads.

## DL-0036 — Contract 0.4.2 versions the durable job format

*Date: 2026-07-26*

**Ruling** (supersedes DL-0034 and clarifies DL-0035): contract `0.4.2` is the first contract for simulation
job storage format 2. The format version participates in the canonical job
specification and therefore in its content address. A `0.4.1` request or job
handle is not silently reinterpreted as `0.4.2`; exact retries are stable
within their declared contract.

Checkpoint replay and all trial folding run outside the project-wide evidence
lock. A locked read verifies canonical bytes, content addresses, chain shape,
terminal anchors, and accumulator shape; the selected immutable head is then
replayed after releasing the lock. A worker retains that verified state across
boundaries and publishes each successor under compare-and-swap, so cadence-one
work does not replay the growing prefix before every append.

**Rationale.** Changing a private format field inside a content-addressed
specification changed public job handles while the worker still claimed the
same contract. Versioning the wire makes that incompatibility explicit.
Separating structural verification from deterministic replay preserves
fail-closed evidence without allowing a long status read or trial prefix to
monopolize unrelated evidence writers.

## DL-0037 — Contract 0.5.0 completes the generic export family

*Date: 2026-07-26*

**Ruling.** Contract `0.5.0` adds `articy`, `csv`, and `static-wiki` to the
closed `dli.export.run` target and emission unions, and adds an optional
`profile_name` selector. A lore document may own a validated profile catalog
through its project manifest. Caller profiles override matching neutral
built-ins by qualified identity; unnamed resolution uses the caller's last
profile for that target. Duplicate identities, invalid templates, unavailable
kept fields, malformed target-specific atom shapes, and target mismatches fail
closed. Every successful artifact requires a non-empty source commit or content
fingerprint; project fingerprints are computed from the exact bytes parsed, not
a second filesystem read.

CSV and static-wiki targets share one deterministic flat projection of selected
certain-layer atoms. Articy honors exact sort/relation selection, entity id
templates, kept fact scopes, who-may-know scopes, and provenance mode.
Dev-metadata carries citations; ship profiles strip them. The CLI emits the
complete artifact and manifest rather than a summary. The operation's golden
fixture replays all six emission-union branches rather than treating one target
as evidence for the rest.

Game rulesets declare the relation names their profiles may select. Card and
Ludii profiles are checked against the concrete ruleset's axes, layers,
relations, template fields, and retained fields; selected card layers are exact,
and source or rendered-id collisions fail closed. A proposed materialized
ruleset cannot be emitted as certain-layer knowledge. Lore-world valuation
overlays have no defined meaning for a materialized game ruleset and are refused
rather than relabeling an unchanged artifact as a fork. Generic lore exports
likewise refuse region and variant selectors they do not materialize. A world
adapter may consume named selectors into a concrete ruleset, but must reject
unknown or incompatible values and clear the selectors before generic export.
Manifest profile hashes cover the resolved profile and options that actually
shape emission, never ignored selector text. Ludii renderers escape every
caller-controlled quoted string, including game ids, card ids, move endpoints,
and layout names.

This is a pre-v1 wire migration because existing consumers treat request and
response variants as closed. Simulation storage remains format 2: the export
expansion changes neither durable job specifications nor their content
addresses.

**Rationale.** Library-only target types and hardcoded neutral CLI mappings do
not satisfy a general caller-facing export contract. A project-owned,
data-driven profile path keeps world content outside the kernel while making
each artifact replayable from caller source, profile, valuation, and manifest.

## DL-0038 — The standalone source release closes its local scope

*Date: 2026-07-26*

**Ruling.** The release condition in DL-0028 is satisfied: the PLAN status
matrix has no local `open` nodes. The executable's built Clap command graph,
including generated help paths, now generates the CLI reference. The CLI
end-to-end suite exercises positive and negative timeline, proof, simulation,
fork, export, query, formatting, naming, fat-payload, craft, and report
workflows with verdict exit codes; the separate project suite exercises both
worker transports. The generic worker, durable evidence and jobs, Door T, and
all six export targets remain available to caller-owned projects without a
consumer-specific adapter. Proof bodies fail closed before formatting or replay:
unknown children, opaque custom-step text, undeclared rule verbs, promoted
assumptions, incomplete or ill-typed bindings, unknown bookkeeping operands,
ambiguous or non-final closes, unbalanced atoms, surplus proof-header text, and
surplus text on fixed-shape operations are findings. Quoted proof terms retain
embedded commas through semantic replay. Caller craft config
reaches every payload path with its
principles, season thresholds, finding citations, and severity overrides; its
outline policy is exposed through the same loaded value to library callers.
Generic defaults emit no consumer decision IDs or consumer
narrator/provenance/material policy, and repeated policy loads own and release
their allocations normally.

This closes the standalone source release only. Consumer/fantasy cutover and
the worked handshake remain external-blocked. The live Ludii differential,
multi-authority valuation, authoring benchmark, human probe, and grammar freeze
remain explicitly deferred; in particular, release closure does not claim a
frozen surface.

**Rationale.** Release status follows replayable caller-facing behavior, as
DL-0028 required. Deriving the reference from Clap removes the former
hand-maintained command inventory, while workflow tests prove dispatch and exit
semantics rather than treating help text or handler arms as implementation.

## DL-0039 — Contract 0.5.1 makes proof selection explicit and preserves quoted claims

*Date: 2026-07-26*

**Ruling.** Contract `0.5.1` adds an optional `proof` selector to
`dli.check-proof`. It may be omitted only when the selected logical document
declares exactly one proof; zero, multiple, missing, or duplicate matches fail
closed. The CLI follows the same rule through `--proof <name>` and no longer
offers a practice override that could only produce a practice-mismatch finding.

Proof claim identity removes insignificant whitespace only outside quoted
values. Quoted spaces, escaped quotes, comment markers, braces, parentheses,
and modifier words remain literal data through parsing, formatting, premise
splitting, and close comparison. `expectation_met` reports only verdict versus
expectation; `checks` additionally requires a structurally valid proof.
Craft-config directories reject every filename outside their closed registry,
and assurance scanning examines executable text after a block-comment closer.

This is a pre-v1 wire migration because request variants are closed. It changes
neither simulation storage format 2 nor durable job addresses.

**Rationale.** Selecting the first proof silently made later declarations
unreachable, while lossy quote normalization could certify two different claims
as equal. A general caller needs an unambiguous proof address and literal claim
identity; both are verdict semantics, not presentation details.

## DL-0040 — Runtime continuity context stays outside durable payload evidence

*Date: 2026-07-31*

**Ruling.** `dli.check-draft` accepts an optional caller-owned `continuity`
context containing the current episode's established facts, explicit changes and
references plus the folded prior fact/promise ledger. The context participates in
the deterministic continuity battery but is never stored as part of `payload/v1`;
`dli.payload-submit` remains replayable from the committed payload and project
assets alone.

The runtime battery reports world contradiction, proposed-as-fact, unknown
entity, intra-episode contradiction and prior contradiction over this structured
context. A change checks its declared `from_value` against the prior ledger; it
does not reject every legitimate transition merely because its target differs
from the old value. Closing a promise absent from the supplied open set is the
same prior-contradiction class.

**Rationale.** One immutable draft payload cannot honestly contain the mutable
"world as of episode N-1" supplied by its caller. Omitting that ledger left
cross-episode continuity library-tested but unreachable through the worker;
putting it inside durable payload evidence would make replay depend on a stale
snapshot. A typed request context preserves both boundaries.

## DL-0041 — The lorestore pivot: facts become the store; `.dli` becomes a rendering

*Date: 2026-08-01*

**Ruling.** deloreis's storage direction is the embedded fact store described
in `design-notes/lorestore-vision.md` and sequenced by
`design-notes/lorestore-roadmap.md` (the worked plan behind this entry, staged
as the `store` lane LS-0..LS-8): content-addressed, immutable JSON fact
objects under RFC 8785 canonicalization, refs with valuation overlays, an
append-only commit log, and — later — a provenance-aware merge engine. The
schemars-derived JSON Schema of the stored objects is the language definition.
The `.dli` text surface is demoted from canonical form to an optional
pretty-print rendering, opt-in on the tool surface and eventually
feature-gated out of the default build.

Two earlier rulings are superseded in part:

- **DL-0003**, the clause "the EBNF renders it; the formatter defines the one
  canonical surface." The schema-first half of DL-0003 stands and strengthens:
  the schema now describes the stored objects themselves. The formatter
  becomes one renderer among several; canonical form is the JCS byte
  serialization of the fact object.
- **DL-0029**, as applied to the `.dli` surface as a whole. Its guarantee
  relocates rather than repeals: retired surface forms, tool ops, and input
  shapes flow through the killed-form registry with a named successor —
  enforced at the server and schema layer once the parser leaves the default
  build. Nothing accepted ever silently disappears.

Consequential closures: SURF-4 (the grammar freeze) and BENCH-1/BENCH-2 (the
two-door authoring benchmark, deferred by DL-0022) are retired, not merely
deferred — the grammar was never frozen, and the door question is answered by
events: the surface door closes, Door T remains. The docs-oracle law is
re-aimed, never suspended: fenced examples stay machine-checked, with fenced
`json` validating against the fact-object schemas as fenced `.dli` recedes.

Boundary condition: the fantasy cutover (MIG-3/MIG-4, EXT-F3/EXT-F4) proceeds
on v1 terms — `.dli` sources and contract 0.5.1 — until EXT-F4 acknowledges
the handshake. Until then every lorestore change is additive and dual-mode
under the DL-0031..DL-0037 versioning discipline; no v1 path is removed on a
schedule.

**Rationale.** The `.dli` surface was the v1 answer to "how do humans and
agents put facts in and review what changed" — text in git, reviewed as
diffs. Both populations have moved: agents already author through Door T
(DL-0014) and never needed the text; human review of rendered structured
diffs is strictly more faithful than review of a rendering's serialization.
What the system actually is — the annotation algebra, derived standing,
graded verdicts, replayable judgments, the offline gate — never depended on
the syntax. Storing facts as canonical content-addressed objects makes the
digest a Merkle root, makes replay a fold over a committed log, and opens
deterministic provenance-aware merge, none of which git-over-text can
provide. The demotion is executed through the killed-form registry precisely
so the one law the surface era got right — accepted forms never silently
vanish — survives the surface itself.

## DL-0042 — The line-count budget and its gate lane are retired

*Date: 2026-08-01*

**Ruling.** The charter's line-count budget line (~15–25K, tripwire ~40K) is
removed, and the `loc` gate lane that enforced it is deleted from `xtask`.
No line-count guardrail replaces them. The budget's surviving lines (doors,
foreign certifiers, export targets) are unchanged and remain enforced by
review and this ledger, not by a counter.

**Rationale.** A line-count cap is a proxy metric: it punishes exactly the
work the lorestore roadmap requires — schema deepening (LS-9) and honest
test coverage add lines before they remove any — while catching nothing a
reviewer would miss. The sovereign judged the mechanic noise. The charter's
real budget content is architectural (fewer doors, fewer seats, fewer
parts), and those lines are judged in review, where the argument lives.

## DL-0043 — The door semantic is retired: one interface, authority boundaries

*Date: 2026-08-01*

**Ruling.** The Door H / Door T semantic — separate supported entry channels
per author population — is retired as a vocabulary and as an architecture.
There is **one interface**: the declarative tool surface (the `author.*` and
`dli.*` operations against the server). What distinguishes authors is
**authority** — who may propose, who may rule — never the entry channel, as
anticipated by `design-notes/multi-authority-knowledge-evolution.md`. Koan 11
is restated from "one door per author" to "one way in"; the charter's budget
line becomes an interface count of one. The machine-never-writes-canon law
(koan 3, DL-0014) is untouched: it was always an authority rule, and it now
reads as one.

Consequences for living documents: charter, `AGENTS.md`, `PLAN.md`,
`SKILL.md`, the spec suite, the cards, and the lorestore roadmap replace door
vocabulary with interface/authority vocabulary. Historical ledger entries,
dated audit notes, and vendored evidence keep their original wording — they
describe the era they were written in. Node ids (`AUTH-*`), fixture names,
and wire-visible strings (finding text, contract descriptions, generated
references) are **not** renamed by this entry; wire-visible renames ride the
LS-4 contract bump through the killed-form registry like every other rename.

**Rationale.** The door framing encoded a channel distinction that the
lorestore pivot dissolves: DL-0041 closed the text channel, and with one
channel remaining, "which door" carries no information — the load-bearing
distinction was always authority. Naming the boundary correctly matters for
what comes next: multi-authority evolution extends the authority model, and a
vocabulary that pins authorship to entry channels would fight that extension
at every step.

## DL-0044 — LS-4 write path: materialize commits to lorestore

*Date: 2026-08-01*

**Ruling.** The authoring `materialize` path commits staged declarations to the
lorestore as a child `Commit` on `main`, returning the new root hash as the
workspace digest. When no lorestore is attached (demo/test contexts), it falls
back to rendering `.dli` bytes as data — preserving backward compatibility.

The initial implementation takes the full-pack approach: `materialize` builds a
`Pack` from base documents + staged declarations, compiles it, lowers all atoms
to `FactObject`s via `lower_atoms()`, writes them to the lorestore (idempotent
CAS), and commits. `all_fact_hashes()` was deduplicated (via `BTreeSet`) to
handle the case where multiple commits reference the same fact objects.

Per-declaration lowering (`Decl` → `FactObject` on each `author.add_*`) is
deferred to LS-4b; the full-pack approach is simpler, reuses the tested
`import_pack` pipeline, and produces identical fact objects.

**Rationale.** This is the first step in making the lorestore the durable
source of truth for authored content (DL-0041). The full-pack approach is
chosen for safety: it exercises the complete `compile → lower_atoms → write_object
→ commit` chain exactly as `import_pack` does, producing identical `FactObject`s.
The CAS deduplication makes the "rewrite base facts" overhead zero — identical
objects produce identical hashes. The `all_fact_hashes()` dedup fix is required
for the read path's `compile()` to handle multi-commit chains correctly.

## DL-0045 — The LS-1..LS-5 audit: repairs land before the back half

*Date: 2026-08-02*

**Ruling.** The implementation audit
(`design-notes/lorestore-implementation-audit.md`) governs the store lane
until its repair sequence completes. Five rulings follow from it:

1. **LS-4 and LS-5 remain `open`.** The audit found the implemented write and
   query/review paths short of their roadmap deliverables: production serve
   never imports the base pack into the lorestore, `materialize` has no
   JSON-first return, no contract 0.6.0 exists, the review test proves
   row-count-plus-atom parity rather than byte-identical replay, and the
   structured-diff standing drops staged-fact provenance. The PLAN matrix
   carries the honest partial evidence; the nodes flip to `done` only when
   the audit's repair commits land.
2. **The `Commit` schema fields `decisions` and `pack_name` get their ledger
   home here.** Both rode the LS-3 read-path commits without a ledger entry;
   the roadmap's annotations mis-cited DL-0042 and DL-0043 (the line-count
   and door-semantic retirements) for them. `decisions` is the optional
   known-decision-id list on the root commit (additive, outside fact
   content-addressing); `pack_name` preserves the scope name so
   reconstruction mints proposal-bundle variables exactly as the loader
   does. The mis-citations in the roadmap are corrected in this commit.
3. **Per-declaration lowering supersedes DL-0044's deferral.** LS-4b —
   lowering each staged declaration to fact objects at `author.add_*` time —
   is implemented and is the taught path; DL-0044's full-pack materialize
   becomes the reconciliation description, not the mechanism. One clause of
   DL-0044 is amended: `materialize` does **not** return the commit hash as
   the workspace digest — the two digest regimes stay distinct (the
   workspace digest remains fence-valid; the commit hash travels in its own
   `root_hash` field), repairing the audit's fence-break finding.
4. **The contract lands as 0.6.0, dual-mode, after the code repairs**
   (owner decision, audit §"governing decisions"). The re-recorded v1
   fixture bytes are restored; 0.6.0 ships with its own golden/negative/
   framing fixtures under the DL-0031..DL-0037 template; an 0.5.1 request
   keeps receiving 0.5.1-shaped bytes until EXT-F4 completes. A gate check
   rides along: a fixture digest may not change while `contract_version`
   is unchanged.
5. **Lowering becomes total before LS-6** (owner decision): every
   declaration the AST can express lowers to fact objects — the expert tier
   as string-bodied `ExpertBody` per the v1 schema design — and
   `reconstruct_pack` refuses what it cannot faithfully round-trip. The
   merge engine builds on whole worlds or not at all.

**Rationale.** The lane's own laws demand this: grades are honest (DL-0006)
— a node is not `done` because a commit message says so; the ledger outranks
(DL-0011) — a ruling that claims byte-identical replay the oracle does not
prove would poison every doc that cites it; and the gate's replayability law
requires the advertised `(root_hash, valuation, query)` triple to actually
resolve, which today it does not. The audit's adversarially verified
findings, not this entry, carry the file-level detail.

**Revisit trigger.** When the audit's repair sequence completes (its rows
all `fixed`), LS-4/LS-5 flip to `done` and the 0.6.0 landing gets its own
entry.

## DL-0046 — Contract 0.6.0: dual-mode, JSON-first, replayable

*Date: 2026-08-02*

**Ruling.** The wire contract is **0.6.0**, served dual-mode: a `0.5.1`
request receives byte-identical `0.5.1`-shaped responses — every vendored v1
fixture replays unchanged against the dual-mode server, and stays in the
registry as the standing compatibility suite — until EXT-F4 completes on v1
terms. Nothing is removed, nothing reshaped (DL-0010, DL-0031..DL-0037).
The handshake reports the version the conversation negotiated.

The 0.6.0 surface closes the audit's wire findings
(`design-notes/lorestore-implementation-audit.md`):

1. **Digest regimes never share a field** (A4). `workspace_digest` is always
   the fence-valid `jcs:sha256:` workspace digest; the lorestore commit hash
   a materialize produced travels in its own `root_hash` field.
2. **`materialize` is a mutation and the accept decision** (B9): dispatched
   with idempotency-key replay, committing the staged delta, advancing the
   workspace's pinned commit, and answering an empty re-materialize as a
   no-op. `author.discard` (whole-workspace or per-declaration) is the
   reject decision, and a rejected declaration's durably staged objects are
   dropped from the pending commit. The JSON-first return is the committed
   fact objects; `.dli` rendering is the `rendered: true` opt-in.
3. **One meaning per `root_hash` field** (B8): `author.status`/`diff` return
   the commit the workspace was **pinned to at open** (advanced only by its
   own materialize); `author.query` returns the workspace digest (staged
   state, `jcs:` regime); `dli.query` returns a domain-distinct
   `world:sha256:` binding over the compiled world — facts, derived set,
   interner, the **full effective valuation with fork/as-of overlays
   folded in** (B6) — resolvable in no store by design.
4. **The wire valuation is the full ruled map** (B7): sorted
   `{atom, value}` entries, explicit falses included; absence means unruled.
5. **Review standing is never filtered** (B3): `AuthorDiffStanding.support`
   is the monomial-structured antichain with signed literals, resolved
   through each side's own interner — hypothesis and proposal-bundle gates
   included, so a gated claim never renders as unconditional.
6. **Released fixture bytes are frozen** (C4): the fixtures lane refuses a
   re-pinned or removed registry digest that does not ride a
   `contract_version` change.

The LS-5 oracle (`crates/dli-serve/tests/ls5_review.rs`) drives the seeded
change-set through the JSON contract alone and proves byte-identical replay:
recompiling the lorestore named by the materialize `root_hash` reproduces
the query rows byte-for-byte and the full valuation exactly. The framing
suite gains paired 0.6.0 cases for the ops with new response surface.

**Rationale.** This is the audit's repair sequence completing (DL-0045
rulings 1 and 4): LS-4 and LS-5 flip to `done` only with this entry, on real
evidence. One fold-order defect was found and fixed on the way — the store
now replays its commit log root-first, so multi-commit reconstruction
preserves declaration order and interner assignment.

**Revisit trigger.** EXT-F4: when the consumer completes the v1 handshake
and migrates, the 0.5.1 dual-mode paths retire through the killed-form
registry (LS-8), never silently.

## DL-0047 — LS-9 foundation: the typed-interior pattern, upcast seam, and no-parse gate

*Date: 2026-08-03*

**Ruling.** LS-9 (expert-tier structuring, roadmap §LS-9) begins with its
reusable foundation, and `RuleDecl` is converted end-to-end as the reference:

1. **Typed interior in the AST.** `RuleDecl` carries a typed `RuleClause`
   (pre-parsed `Datalog` head/body) in `dli-ast`; the parser (`dli_syntax`,
   the single producer) emits it, and `dli-world`'s loader and `dli-time`'s
   order/season guards **read the typed structure** — no `parse_rule_clause`
   on stored data. `dli-time` is now genuinely syntax-free: `dli-syntax` is a
   dev-dependency only.
2. **The v1→v2 upcast seam.** `SCHEMA_VERSION` is 2; `read_typed` probes the
   stored version and upcasts a v1 string-bodied rule carriage to its typed v2
   form at the load boundary (default reads of v2 are pure deserialization).
   Idempotency (`ensure_pack_imported`) compares on the version-independent
   identity projection (`FactObject::content_hash`), so a store persisted
   under v1 is recognized as the same lineage rather than refused — old stores
   remain usable, as the design requires.
3. **The no-parse acceptance ratchet.** A gate lane (`xtask syntax-free`)
   classifies each target crate's manifest and reports whether the parser is
   on its default path. It is robust to the dotted `[dependencies.dli-syntax]`
   form (a red-team-found false-negative, fixed).

The acceptance bar — *no crate outside `dli_syntax` parses stored data* — is
met **for `dli-time` only**. `dli-world` and `dli-ledger` remain coupled
through the other ten declaration types (`Practice`, `Proof`, `Dialect`,
`Name`, `Ratify`, and the five near-empty `Fork`/`Draft`/`Export`/`Variant`/
`Principle`), which the ratchet honestly reports as `still-coupled`. **LS-9
stays `open`**; this entry records the foundation, not completion.

**Rationale.** LS-9 is the roadmap's predicted bulk (~23k lines across
`dli-world`/`dli-ledger`/`dli-time`); the pattern proven here — typed interior
in `dli-ast`, lowering owned by `dli_syntax`, downstream as typed readers,
carriage upcast — is the unit each remaining type repeats. Landing the
foundation and the ratchet first makes the remaining conversions incremental
and keeps the acceptance bar honest at every step. The work was adversarially
red-teamed; the two confirmed defects (the gate's dotted-table false-negative
and the v1-store idempotency break) were fixed with fail-before-fix tests.

**Revisit trigger.** LS-9 completion: when the ratchet reports `dli-world`
and `dli-ledger` syntax-free (all eleven types converted), LS-9 flips to
`done` and unblocks LS-7 (the parser can leave the default build).

## DL-0048 — LS-6: the merge engine (commit DAG, 3-way merge, conflict detection)

*Date: 2026-08-03*

**Ruling.** The lorestore gains a **merge engine** (roadmap §LS-6), and LS-6
flips to `done`:

1. **Merge commits, additive.** `Commit` gains an optional `merge_parent`
   (`skip_serializing_if`), so a non-merge commit's JCS bytes are unchanged
   under the LS-9 v2 schema; a merge commit records `parent` = target head,
   `merge_parent` = merged-in head. The two-parent DAG walk and a merge-base
   (lowest-common-ancestor) generalize the single-parent chain; the linear
   walk stays byte-identical (LS-3/LS-4 replay parity preserved).
2. **Persistent branches** under `refs/heads/<name>` (`main` unqualified),
   with create/list/head on the `Lorestore` facade.
3. **3-way merge = union + recompile.** The union of both branches' fact
   sets since the merge base is committed; the polynomial join is the
   fixpoint's own `⊕` (`PosBool::or_`) on recompile. Interner reconciliation
   requires each parent's interner be a prefix of the merged (union) table.
4. **Conflict detection, three classes**, reusing `ConflictKind`: valuation
   disagreement (the sovereign decides — never auto-resolved); constraint
   violations (functional-dependency / exclusivity / certain-layer, detected
   by recompiling the merged world through `check::run`); stratified-negation
   effects (standing flips, and the unstratifiable-merge extreme). The
   `fork.merge` functional-dependency gap is closed.
5. **Tool surface** `author.branch` / `author.merge` at contract **0.6.1**
   (additive; 0.5.1/0.6.0 requests keep byte-identical responses).

**Oracles.** Merge is commutative and associative up to root-hash equality on
conflict-free corpora — proven on the **non-monotone** case (a negation-rule
base where each branch un-derives the other's conclusion) via a
defeater-inclusive world signature; a seeded-conflict corpus catches every
class with repair data; the unstratifiable path is seeded end to end.

**Two honest limitations, recorded so they are not rediscovered as bugs:**
- **Bare-`@proposed` facts are fold-order-sensitive at the byte level.** They
  mint positional gates `h.<pack>#N` that are re-numbered per merge fold order,
  so recompiled *bytes* can differ across merge order though the *observable
  world* is confluent. Root-hash-equality confluence therefore holds for
  ratified/cited facts; the oracles cite `D-1` to stay order-independent. The
  interner reconciler carries a pointer comment.
- **A merge conflict cannot be driven through pure wire ops.** Door T never
  mints certain content (agents only propose — DL-0014/DL-0043) and no wire op
  sets a branch valuation, and a contradiction is refused upstream by the
  workspace continuity check before it can conflict. So the conflict-bearing
  *golden fixture* asked for is not constructible in LS-6's scope: the
  conflict-report **shape** is pinned in the digest-checked contract
  (`author.merge.response.json`, `schema --check`), and a real conflict through
  `respond()` is proven by seeding divergence via the lorestore handle. A
  wire-driven conflict needs a future sovereign-ruling op or branch-scoped
  writes — out of LS-6 scope, and for agent-minted certainty, against DL-0014.

**Rationale.** This is the roadmap's genuinely new machinery, and the last
core store-lane stage. It falls out of the algebra (§4): the monotone core is
a grow-only join-semilattice, so union is confluent and the only conflicts are
the three non-monotone judgments the merge report enumerates — merge is
*simpler* than git's line-merge, not harder. The work was adversarially
red-teamed twice; the confluence oracle's gaps (a defeater-blind signature, a
positive-only commute corpus, an untested unstratifiable path) were closed,
and no correctness defect survived verification.

**Revisit trigger.** LS-8 (contract 1.0) and any wire-driven-merge surface —
a sovereign-ruling op would let a conflict be exercised end to end through the
wire.

## DL-0049 — LS-9 completion: all eleven declaration interiors typed; the residual is LS-7's

*Date: 2026-08-03*

**Ruling.** LS-9 (expert-tier structuring, roadmap §LS-9) is **complete** and
flips to `done`. The DL-0047 foundation extends to all eleven string-bodied
declaration types:

1. **Ten remaining interiors typed.** Beyond `RuleDecl` (DL-0047), the parser
   (`dli_syntax`, the single producer) now structures `Practice`, `Proof`,
   `Dialect`, `NamePart`, `Name`, `Ratify`, and the five near-empty
   `Fork`/`Draft`/`Export`/`Variant`/`Principle` bodies via
   `structure_<type>_body`, emitting typed interiors defined in `dli-ast`.
   `dli-world` (dialect/name/ratify) and `dli-ledger` (practice, proof) read the
   typed structure — no `parse_*` on a stored declaration body. The proof
   validator's former format-and-reparse round-trip (`validate_proof_decl`) is
   replaced by a **structural** check over the typed `ProofClause`/`ProofOp`
   interior. Each type keeps its normalized `Clause` text beside the typed
   payload, so the formatter round-trips byte-identically.
2. **`dli-ledger` and `dli-time` are syntax-free.** Both carry `dli-syntax` as a
   dev-dependency only; the ratchet (`xtask syntax-free`) reports **2/3** target
   crates clear.
3. **The residual is not a declaration body.** `dli-world` retains one
   `dli_syntax` coupling — `payload.rs`'s `parse_clause_atom` over payload-**claim**
   and runtime-**fact** surface atoms at the fact-checker boundary (`parse_claims`,
   `parse_runtime_fact`). This is a load/check-boundary surface parse, not one of
   the eleven declaration interiors; retiring it is LS-7's feature-gate-the-load-
   boundary work. DL-0047's revisit trigger phrased the bar as "`dli-world` **and**
   `dli-ledger` syntax-free," assuming the eleven types were `dli-world`'s only
   coupling; the completion audit found this twelfth, non-declaration coupling and
   assigns it to LS-7. The ratchet honestly reports `dli-world` as `still-coupled`
   for that one site.
4. **v1→v2 upcast covers all eleven.** `deepen_<type>_carriage`
   (`dli-lorestore/src/upcast.rs`) upcasts every v1 string-bodied carriage to its
   typed v2 form; `read_typed`'s schema-version probe routes below-current reads
   through it. A store persisted under v1 remains usable.

**Contract.** The v2 `SCHEMA_VERSION` participates in commit/object bytes, so two
commit-hash-bearing goldens re-hash: `author-merge-v1` (`merge_commit`/`merge_base`)
and `export-ludii-v1` (`manifest.commit`). Per the C4 guard the digest change rides
a `contract_version` bump **0.6.1 → 0.6.2**, and 0.6.1 joins
`ACCEPTED_CONTRACT_VERSIONS`. **No 0.5.1 or 0.6.0 golden shifts** — the only external
commitment (0.5.1) stays byte-identical, and 0.6.x has no external consumer yet (the
consumer is on 0.5.1 pending EXT-F4). The `author.branch`/`author.merge` capabilities
are re-gated on `version >= 0.6.1` (not `== current build`), so a 0.6.1 handshake
keeps them after the 0.6.2 bump.

**Oracles.** Per-type differential parity — a corpus compiled through each typed
interior yields byte-identical facts / fixpoint / findings / digests to the
string-bodied original across the vendored pack; the proof structural validator
replaces the format-and-reparse round-trip; the no-parse ratchet is a compiled gate
lane (2/3, with `dli-world`'s one residual tracked).

**Rationale.** LS-9 was the roadmap's predicted bulk; with the DL-0047 pattern
proven, the ten remaining conversions were mechanical repeats. Completing them makes
`dli_syntax` a pure front end for every declaration body — the precondition LS-7 needs
to feature-gate the parser out of the default build. The remaining load-boundary claim
parse is genuinely LS-7's, not a hidden LS-9 miss; recording it here keeps the
acceptance bar honest rather than declaring a clean sweep the ratchet contradicts.
Adversarially red-teamed: structural-parse equivalence and upcast fidelity confirmed
clean, proof faithfulness confirmed; the one low finding (`dli-ledger` clearable to a
dev-dependency, a stale ratchet comment) is applied in this change.

**Revisit trigger.** LS-7: when `payload.rs`'s claim/fact parse moves behind the
`syntax` feature, the ratchet reports `dli-world` clear (3/3) and the default build
links no `dli_syntax`.

## DL-0050 — LS-7: surface retirement — `dli_syntax` off the default build

*Date: 2026-08-03*

**Ruling.** LS-7 (surface retirement, roadmap §LS-7) is **complete** and flips to
`done`. `dli_syntax` is demoted to an opt-in `syntax` feature, **off by default**;
the default build of the server and CLI links **zero** `dli_syntax` (`cargo tree -p
dli-serve`/`-p deloreis -e normal` is empty), and is JSON / 1.0-native. The parser
crate is **kept** — full removal (the deprecation note's phase 4) stays deliberately
unscheduled. Six landings, each adversarially red-teamed:

1. **Docs-oracle re-aimed** (#11): fenced ` ```json <schema> ` blocks validate against
   the LS-1 fact-object schemas; bare ` ```json ` must be well-formed; ` ```dli `
   stays parsed. The docs-oracle law holds continuously; every fenced example is
   machine-checked.
2. **Typed successors + a dormant field-level killed-form registry** (#12 → contract
   0.6.3; #17 → 0.6.4). The text-on-wire remnants gain optional typed fields:
   `goal_atom` (dli.query/author.query), `additions` (fork.add, `Vec<GroundAtom>`),
   `Claim.atom_expr` and `RuntimeFact.atom_expr`/`RuntimeChange.to_atom_expr`
   (check-draft). A `KILLED_FIELDS` registry names each string field's typed successor
   at the decode seam, **dormant** — gated on contract major ≥ 1, so it never fires on
   any accepted version (it fires under 1.0 in LS-8). All additive: prior-version
   responses byte-identical, string forms still accepted under `--features syntax`.
3. **Demos served from committed fact objects** (#15): the `camelot`/`round-table`/
   `warding-vows` demo corpora are generated by a real `import_pack` and served at
   runtime via `reconstruct_pack` from committed bundles under `crates/dli-serve/demos/`
   — no runtime parse — with a drift-guard test regenerating them from source. LS-3
   parity keeps every demo response byte-identical.
4. **The feature-gate** (#16): `dli-syntax` is an optional dependency behind `syntax`
   across `dli-lorestore`, `dli-world`, `dli-serve`, `deloreis`; every default-path
   call site (`.dli` ingestion, the formatter, the string-parse fallbacks, the v1→v2
   upcast, the deloreis `.dli` CLI) is `#[cfg(feature = "syntax")]`-gated. A **two-build
   gate matrix** proves it: a default lane asserts no `dli_syntax` in the graph and
   exercises the default build; a `--features syntax` lane replays every fixture
   byte-identical. The no-parse ratchet is **3/3**.
5. **Reference cards reframed** (#18): the cards teach authoring as `author.*` tool
   calls over JSON and the canonical form as fact objects, keeping `.dli` as the
   readable **rendering** for concepts. All values are engine-true (captured from a
   running worker at 0.6.4); schema-validated json blocks went 6 → 13.

**The default build is 1.0-native.** It serves lorestore worlds, the migrated demos,
and typed (`goal_atom`/`additions`/`atom_expr`) requests. `.dli` project loading, string
goal/fork/claim parsing, the `.dli` formatter, v1→v2 upcast, and the deloreis `.dli`
CLI all require `--features syntax`. The **0.5.1/0.6.x consumer is served byte-identical
by a syntax-enabled build** (that is what production runs until EXT-F4); the default
build is the JSON-native surface the pivot targets.

**Rationale.** This is the roadmap's staged demotion (DL-0041): the schema is the
language, `.dli` is one rendering, and the parser need not be linked to serve the
canonical JSON surface. Keeping `dli_syntax` behind an opt-in feature — not deleting
it — preserves the pretty-printer and the legacy `.dli` path while making the default
build prove it needs neither. Every contract bump is an additive optional-field
addition; nothing the 0.5.1 consumer sends changes shape.

**Deferred to LS-8, recorded so they are not rediscovered as bugs:**
- **No per-declaration `author.*` tool for the expert declarations** (`rule`, `state`,
  `cycle`, `practice`, `proof`, `dialect`): they enter only when a corpus is loaded.
  The "one interface" (DL-0014/DL-0043) is incomplete for expert-tier lore — a missing
  tool, not a second interface.
- **No lorestore-backed *project* loader**: the default build loads demos and
  author-tool worlds from the store but still needs `--features syntax` for a `.dli`
  project. LS-8's "one storage model" closes this.
- `Decided`-with-citations reconstructs as `Ratified`, and fact spans reconstruct
  synthetic (`0..0`) — both inert on today's wire surface, both need a `SCHEMA_VERSION`
  bump to fix.
- Typed-atom predicate well-formedness is not re-validated (JSON-first, matching the
  `author.add_relation` precedent); `Ident::span` has no serde default, so a hand-built
  typed goal is verbose.
- `SKILL.md`'s `rendered`/`materialize` description is corrected in this change;
  `CLAUDE.md` §7 still carries the pre-LS-7 "rendered canonical lines" phrasing (left to
  the human owner of the instructions file).

**Revisit trigger.** LS-8 (contract 1.0 and consumer cutover): the field-level
killed-forms fire under 1.0; the expert-declaration tools and the lorestore project
loader are its natural close; and the 0.5.1 dual-mode paths retire through the
killed-form registry after EXT-F4 — never silently.

## DL-0051 — LS-8: contract 1.0, local prep — killed-forms activated; external-blocked on EXT-F4

*Date: 2026-08-04*

**Ruling.** LS-8 (contract 1.0, roadmap §LS-8) lands its **local half** and is marked
**`external-blocked`** on EXT-F4. `CONTRACT_VERSION` is `1.0.0`; `0.5.1` and
`0.6.0`–`0.6.4` stay accepted (multi-version dual-mode). Four moves:

1. **The field-level killed-forms fire under 1.0.** The dormant `KILLED_FIELDS`
   registry (DL-0050) is wired into `serve::dispatch` above the op fork and gated on
   contract major ≥ 1. A 1.0 request carrying a retired string field — `goal`
   (dli.query/author.query), `declarations` (fork.add), a payload claim's `atom`
   (payload-submit *and* check-draft) — is refused `killed-form` naming its typed
   successor (`goal_atom`/`additions`/`atom_expr`); the typed successor sent alone
   answers.
2. **Two-sided retirement off one registry.** A retired field cannot also be
   serde-`required` (a 1.0 typed request omits it), so the four fields became
   optional; the requirement is re-imposed **below 1.0** by `required_retired_field`,
   byte-exact with serde's own `missing field` verdict and its declaration-order
   precedence (`preceded_by`/`followed_by`). Below 1.0 the string form stays required;
   at 1.0 it is refused. No 0.x wire moved.
3. **The agent surface adopts 1.0, ergonomically** (operator's call). The MCP door
   frames at 1.0, so authoring agents send the typed `goal_atom`; a hand-written
   `Ident` codec accepts the terse spelling (`"serves"` for `{"text":"serves"}`) so a
   typed atom is not verbose. `SKILL.md` and the reference cards teach the terse form.
4. **A differential parity oracle guards 0.x, permanently.** `ls8_zero_x_parity`
   replays a 394-frame corpus captured from the pre-LS-8 build against the new one, in
   the gate; every pre-1.0 frame matches byte-for-byte except a handful of
   individually-recorded, mutation-verified deviations (framing version-echoes; a
   positional-array `Ident` derive-quirk the published schema never allowed).

**Rationale.** LS-8 is the roadmap's contract major: JSON-first, lorestore-backed,
`.dli` rendering opt-in. The local half proves 1.0 end to end — the killed-forms fire,
the typed surface is complete and ergonomic, and the whole 0.5.1..0.6.4 window replays
byte-identical — without touching the consumer. Two adversarial rounds found real 0.x
byte-identity breaks the fixture suite could not see (check-draft's shared-struct
`atom`, a nested-error mis-attribution, an `Ident` wording drift); each was fixed and
the parity oracle built so the class cannot recur.

**External-blocked.** The consumer cutover — the fantasy repo migrating from vendored
0.5.1 to 1.0 — is **EXT-F4**, consumer-owned; deloreis does not write the fantasy tree
(DL-0010). The 0.5.1/0.6.x dual-mode paths are **not deleted**; they retire through the
killed-form registry only after EXT-F4, never silently.

**Deferred, recorded so they are not rediscovered as bugs.** The runtime-continuity
`predicate`/`subject`/`value` string form is not yet a registered killed-form (a
multi-field retirement), so it survives into 1.0; `KILLED_FIELDS.followed_by` is
name-matched (the five registrations are audited disjoint from nested field names and
corpus-backstopped, but a future params field named `pred`/`args` would need a look);
and DL-0050's open items — the expert-declaration `author.*` tools and the
lorestore-backed project loader — remain.

**The back half.** With LS-8 `external-blocked`, the store-lane back half is locally
complete: LS-6 (DL-0048), LS-9 (DL-0049), LS-7 (DL-0050), LS-8 (this entry). Only
EXT-F4, consumer-owned, remains.

**Revisit trigger.** EXT-F4: when the consumer speaks 1.0 against the vendored,
digest-checked contract, the 0.5.1 dual-mode paths retire through the killed-form
registry, and the deferred runtime-continuity / expert-tool / project-loader items
close.

## DL-0052 — Governance: machine-minted ledger entries with attribution; supervision is async

*Date: 2026-08-04*

**Attribution:** human — this entry encodes the author's explicit words, quoted below.

**Ruling.** Ledger entries are minted by whoever decides, machine or human, and every
entry says which. Four parts:

1. **Machine agents may mint ledger entries.** A `DL-nnnn` entry in this repo, and a
   `D-nnnn` entry in the consuming fantasy repo, may be minted by a machine agent on
   its own judgment — not only by the human author. An agent that reaches a settled
   decision writes it down; it does not file a ruling request and wait.
2. **Every entry carries an `Attribution:` marker**, one of two values. `machine` — an
   agent decided it autonomously; the decision is live and binding on the tree, pending
   the author's async review. `human` — the entry encodes the human author's explicit
   words, quoted in the entry so the ruling can be checked against its source.
3. **Human supervision is async and lagging, and steers as repair.** The author reviews
   on his own cadence, after the fact, and corrects by minting a superseding entry — the
   ledger's ordinary append-only mechanism. Supervision is never a merge-blocking
   sign-off, and no work waits on a human sync point; there are no hard human sync
   points in this repo's procedures.
4. **The rest of ledger discipline is unchanged.** Append-only (never edit or renumber
   an entry; a decision that changes is superseded by a new entry naming the one it
   supersedes), single-writer (one agent writes the file per commit), and mint-and-use
   in one commit all stand exactly as DL-0011 set them. Machine minting widens *who*
   may write an entry; it loosens nothing about *how*.

This entry is where the `Attribution:` convention lives, and it stays here: `cargo
xtask authority` enforces append-only as strict byte-prefix extension, so the file's
preamble cannot be amended to carry a new discipline bullet — evolving discipline
lands as an entry, by design. `AGENTS.md` §2 restates it for cold-start agents.

**Rationale.** The author ruled it on 2026-08-04, in these words:

> i dont think i need hard human sync points of any kind. i'll review on a lagging
> basis and steer as repair, not as merge-blocking signoff. this is another area where
> the overall procedures need a rewrite.

and, on who writes the entries:

> just write them yourself with clear attribution on what was machine versus human. as
> discused, i'll mostly supervise on an async/lagging basis

The prior procedure — agents file ruling requests and wait for the author — made one
human the serializing bottleneck on a machine-paced build, and it produced entries
uniformly presented as the author's whether or not he had decided anything. The
attribution marker is what makes machine minting readable later: a reviewer can tell at
a glance which entries carry the author's own words and which are an agent's judgment
awaiting his lagging review. Append-only is what makes it safe: a wrong machine ruling
is superseded, never erased, and the correction costs no more than the mistake.

**Scope.** This rules the governance procedure, not the engine. The authority model on
the wire — attribution and decision state as machine-supplied data, no role gating
inside deloreis — is a separate author ruling, recorded in
`docs/integration/cutover-campaign.md` §1 and minted under its own entry.

## DL-0053 — Handshake delivery: the contract doc to 1.0, the tag, retirement through the registry

*Date: 2026-08-04*

**Attribution:** machine — an engineering decision by the orchestrating agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** The integration handshake ships as four decisions.

1. **The contract doc is brought to `1.0.0` truth.**
   `docs/integration/fantasy-contract.md` was a full major stale: it described
   `0.5.1`, 46 operations, and a non-mutating `author.materialize`. It now states the
   served reality — contract `1.0.0`; **48** operations (27 `dli.*` + 21 `author.*`,
   counted against `crates/dli-serve/src/ops.rs`), with `author.branch` and
   `author.merge` (added at `0.6.1`) present in the §3.2 inventory; and
   `author.materialize` as **a mutation and the accept decision** per DL-0046 — it
   commits the staged delta to the lorestore, advances the workspace's pinned commit,
   and answers JSON-first (`facts` plus `root_hash`) with `.dli` rendering behind
   `rendered: true`, while `author.discard` is the reject decision. The vendored
   contract is the consumer's map of this engine; a stale map is worse than none,
   because it is trusted.

2. **The accepted window, and who chooses within it.** The server accepts `0.5.1`,
   `0.6.0`–`0.6.4`, and `1.0.0`, and answers every call in the version that call
   declared. `1.0.0` was minted **before** the consumer cutover rather than at it: the
   engine's own pivot to fact objects (DL-0041, LS-8/DL-0051) is what earned the
   major, and withholding the number until the consumer moved would have shipped the
   pivot under a `0.x` label. The `0.x` window is consequently wider than an ordinary
   compatibility promise and closes at EXT-F4. **N / N-1 major support applies from
   `2.0.0` onward** — that is what contract §6 now states, in place of its superseded
   "the version bumps to v1 at the first cutover" plan.

3. **Retired versions answer through the killed-form registry.** After EXT-F4 each
   retired `0.x` version is registered with its successor and the ruling that retired
   it, and a frame declaring one receives a deterministic refusal naming both. A
   version is never dropped silently and never merely stops working. This is the
   killed-form law applied to the version axis, and as with every other killed form
   the registry entry outlives the code path behind it.

4. **The tag is the deliverable.** deloreis cuts an **annotated `contract/1.0.0` tag
   on the merge commit of this change**, so the tagged tree is self-consistent — the
   contract doc and the `contracts/` fixture-registry digests agree at one commit —
   and the tag message cites this entry. That tag is the deloreis side of CP-1, CP-9,
   and CP-10 (campaign SP-1). The STATUS rows advance to `provided`, carrying the tag
   name and SHA as evidence, only once the tag exists; this change does not advance
   them, because the ladder does not admit a rung whose evidence is a promise.

**Rationale.** The consumer's critical path runs through a document, not through
code: it vendors the contract, digest-checks it, and implements against it. Every
sentence that lags the engine is a defect the consumer discovers as a wire surprise,
so the doc rewrite precedes the tag rather than following it (contract §5 step 1).
Deciding the versioning story here, rather than leaving §6's obsolete plan standing,
matters because the plan it described did not happen: pretending otherwise would make
the first thing the consumer reads about compatibility a fiction. Retirement through
the registry is chosen over deletion for the same reason it always is — a refusal that
names its successor teaches, and a silence does not.

**Revisit trigger.** EXT-F4: when the consumer completes the handshake on `1.0.0`, the
`0.x` versions retire through the registry (campaign DM-6) and the accepted list
shrinks; the retirement lands its own entry.

## DL-0054 — Authority scrub: the engine records attribution and decision state as data; role enforcement is a consumer concern

*Date: 2026-08-04*

**Attribution:** human — this entry encodes the author's explicit ruling, quoted below.

**Ruling.** Deloreis stops enforcing **who may rule**. The engine verifies and records;
it does not police roles.

1. **Scrubbed, on the 1.x wire.** The Door-T refusal of citation-bearing provenance
   from a caller (`provenance.cited-refused`), `SKILL.md` §0's "you cannot cite" law,
   and DL-0043's framing of authority boundaries — "who may propose, who may rule" —
   as something the engine enforces are removed. Whether a caller is entitled to cite a
   ruling is not a question this engine can answer, and answering it anyway made the
   engine wrong about its own callers.

2. **Kept: referential integrity.** A supplied citation must resolve against the known
   decision set. `citation.unknown-decision` stands and stays fail-closed, on every
   wire: a forged or unknown decision id is refused. The engine verifies that a cited
   thing exists; it never invents one, and it no longer asks who is citing it.

3. **New: attribution and decision state are data.** A mutating call carries
   caller-supplied attribution and decision state, recorded in provenance as supplied.
   Review and approval workflow — proposing, ruling, cascading an approval downward —
   is the consuming application's pipeline. A human ruling reaches the engine the same
   way everything else does: a machine cites the consumer-ledger id that recorded it.

4. **Wire discipline.** The behavior change rides **contract ≥ 1.2.0** only. Every
   `0.x` wire keeps the `provenance.cited-refused` refusal **byte-exact** until version
   retirement, enforced by the `ls8_zero_x_parity` oracle. This entry rules the
   **direction**; the code lands in campaign phase DM-4 under this ruling, and until it
   ships the refusal remains the served behavior on every version, `1.0.0` included.

**Rationale.** The author ruled it on 2026-08-04, in these words:

> this is a misunderstanding that got threaded pretty deep in deloreis and needs get
> scrubbed out. basically all callers of deloreis are machines, typically agents or
> upstream agentically mediated. those agents are responsible for providing clear
> attribution and decision state... really can't have all this role stuff living at the
> deloreis level. i think it's all a higher level application (e.g. fantasy) concern. so
> this needs to be evolved to match that sanely.

The engine had encoded a social fact — that a human author, and only a human author,
mints citations — as a wire-level refusal. Every caller of deloreis is a machine, so the
refusal never distinguished a human's decision from an agent's; it only distinguished
which field the machine filled in, and taught agents to route human rulings around the
provenance system rather than through it. Attribution is something the caller knows and
the engine cannot derive. Recording what the caller states, and verifying the one thing
the engine genuinely can check — that the cited decision exists — keeps the engine
honest about the boundary of its own knowledge. This is the wire-level counterpart to
DL-0052, which made the same correction for the ledger's own procedure.

**Revisit trigger.** Contract 1.2.0 (campaign DM-4): the implementing entry records the
finding-code and provenance-shape changes, and the `0.x` parity evidence.

## DL-0055 — DM-2: store-backed projects, SCHEMA_VERSION 3, and the schema/contract coupling

*Date: 2026-08-04*

**Attribution:** machine — an engineering decision by the orchestrating agents, live and
binding on the tree, pending the author's async review (DL-0052).

**Ruling.** DM-2 lands in four parts.

1. **A document's source form may be a lorestore.** `documents.<id>` gains `store` (a
   project-root-relative store directory) and `head` (a store commit hash), and `sources`
   becomes optional. Exactly one source form is legal: both is refused by name, and
   neither is refused by name, because either would leave "what is this document" without
   an answer the loader may pick on its own. `head` without `store` is refused for the
   same reason. The store path is resolved with the discipline every other asset path
   gets — beneath the manifest directory, symlink escape checked. Additive: every existing
   `sources` manifest parses unchanged under `deny_unknown_fields`.

2. **Two modes, and the fingerprint regime.** `head` present is **pinned mode**: the
   document is that commit, so the project replays, and advancing canon is an explicit
   manifest edit. `head` absent is **branch mode**: the document follows the store's
   `main` tip, which is what an authoring server wants. The document's fingerprint is the
   resolved head — the store's **content-hash** regime (`sha256:`), never the JCS
   `workspace_digest` regime (`jcs:`). The two digest domains do not meet in a field or in
   a meaning (DL-0046 audit repair 1).

3. **A store-backed document attaches its origin store, not a copy.** The serve layer
   opens the store the project loaded from, so `author.materialize` advances the very
   store the next load reads — the durable authoring loop closes. It is deliberately *not*
   re-imported on attach: the pack was reconstructed from that store, so there is nothing
   to import, and importing a pinned document's older commit would write a new one, which
   would make serving a read path with a side effect. A pinned document does not follow
   the branch; nothing may advance it implicitly.

4. **`SCHEMA_VERSION` is 3**, and the stored shape changes twice over.

   **Source position leaves the identity projection.** A carriage embedded the whole
   span-bearing `Decl` as JSON, and `expert_body` is inside `content_hash`, so the same
   declaration at two source offsets was **two objects** — verified with a reproduction
   before any fix: `use harbour` at offset 0 and at 4096 hashed to `sha256:a4bd51d7…` and
   `sha256:87425712…`. That defeated dedup and broke the invariant `identity.rs` states
   for itself, that the same assertion reached on two forks is the same object so merge
   can be a union. The hashed body is now span-normalized. The located original is
   recorded in `provenance.located_body`, outside the hash, and reconstruction reads it —
   position is kept, not dropped (koan 4), it simply stops deciding identity.

   **The authored ledger marker is recorded.** `decided D-2`, `ratified D-2`, and
   `provisional D-2` all lower to the identical citation conjunction, so reverse lowering
   had to answer `Ratified` for all three and the distinction died on every round trip.
   The marker is now recorded in `provenance.marker` — **outside** the hash, because the
   three share one valuation and the loader treats them identically at the fixpoint;
   hashing it would make an existing store foreign. A v2 object records `None`, which
   reads as *not recorded* and never as `proposed`; the upcast invents nothing.

   A v2 store is the **same lineage**, by the mechanism already in place: `read_object`
   upcasts, the upcast normalizes the carriage body, and the lifted object's identity
   equals a freshly lowered v3 object's, which is what `ensure_pack_imported` compares.

5. **A `SCHEMA_VERSION` bump is a contract change.** `schema_version` is inside the stored
   bytes by design, so bumping it moves every lorestore commit hash — including two pinned
   in `golden/author-merge-v1.json`. Audit C4 (DL-0046 ruling 6) refuses a re-pinned
   fixture digest that does not ride a `contract_version` change, and it is right to: a
   vendored consumer must never see different bytes under one version label. DM-2 was
   specified as "no contract bump"; that was wrong, and the ruling is that **the contract
   version bumps to `1.1.0` here**. Schema-first (DL-0003 as amended by DL-0041) already
   says the stored fact-object schema *is* the language definition, so a change to its
   canonical bytes is a contract change whatever else moves. `1.1.0` joins the accepted
   list; no version is retired; no tag is cut in this change.

   **The minor ladder shifts with it**, recorded here so the next agents inherit it
   rather than rediscover it: DM-2 is `1.1.0` (this entry, no tag); DM-3's corpus
   exchange is `1.2.0`, tagged `contract/1.2.0` as the pivot deliverable; DM-4's
   wave-1 tools and the authority scrub are `1.3.0`, tagged; DM-5's validator
   governance carries **no bump** (it has no wire surface); DM-6's retirement is
   `2.0.0`, tagged. DL-0054's "contract ≥ 1.2.0" for the scrub therefore reads as
   1.3.0 under this ladder — the scrub still lands with the tools, in DM-4.

**Rationale.** The span defect is the kind that hides: nothing fails loudly, dedup simply
stops deduplicating and a merge that should be a union starts producing conflicts, in
proportion to how much unrelated editing has shifted offsets. Fixing it needed the
identity projection to say what a fact *is* rather than where it was typed, which is what
the projection discipline already said for `claim_id` and `schema_version`. Recording the
marker is the same shape of correction, from the other side: the store was deriving a
fact's ruling status from a polynomial that cannot carry it, so it guessed, and the guess
was wrong two times in three.

The version coupling is the honest surprise. Ordering the schema bump before any consumer
store exists was right, and the campaign plan's "no contract bump" was simply incompatible
with the freeze rule it also relies on. Bumping is the resolution that keeps both guards
intact rather than carving an exception into C4, which exists precisely to stop bytes
moving quietly under a vendored label.

**Recorded, so it is not rediscovered.** The `SCHEMA_VERSION`/`contract_version` coupling
is **structural**: every future schema bump moves every commit hash, so any released
fixture that pins one forces a contract bump with it. That is a real constraint on the
minor ladder, not a DM-2 accident. The alternative — keeping content-addressed values out
of released fixtures — is not free either, since pinning them is what makes the fixture
prove anything about the store.

**Revisit trigger.** The next `SCHEMA_VERSION` bump. If the forced contract bump proves
costly on the minor ladder, the cleanup to weigh then is whether a fixture that pins a
bare lorestore commit hash should key its freeze on `SCHEMA_VERSION` rather than
`contract_version` — the schema is what actually moves those bytes — or should assert
**structural properties** (the merge produced one commit, the base resolved, the conflict
set was empty) instead of absolute hashes. Both trade some replay strength for a ladder
that is not forced; neither should be adopted without saying which guarantee is being
given up.

## DL-0056 — DM-3: the corpus exchange carries interner and decisions, and the rendered refusal is loud

*Date: 2026-08-04*

**Attribution:** machine — an engineering decision by the implementing agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** Contract `1.2.0` adds the two corpus operations the consumer's pivot walks
through, and rules what a corpus *is* on the wire.

1. **`dli.corpus.import` and `dli.corpus.export` land at `1.2.0`**, gated on the
   negotiated version: only a `1.2.0` handshake advertises them, so every earlier
   version's capability list — and the golden fixture recording it — stays byte-identical
   (audit C4). Nothing is retired; the accepted window only grows.

2. **A corpus is objects *plus* interner *plus* decisions.** This is the ruling, and it
   was found by building the round trip and watching it lie. A stored fact records its
   support polynomial as `±<index>` into the commit's interner, and its standing is read
   against the commit's known-decision set. Exporting the fact objects alone and
   re-importing them is not lossless and does not fail — it silently rewrites provenance.
   Measured, on a corpus whose facts read `person wren decided d-1`: re-importing the
   objects alone reconstructed `person wren proposed assumes __decl`, because the receiving
   store rebuilt an interner from the objects' own atom names, a different index space
   entirely. Both tables therefore travel with the objects: `dli.corpus.export` returns
   `interner` and `decisions` beside `objects`, and `dli.corpus.import` requires
   `interner` with an `objects` payload. Carrying all three reproduces the source commit
   hash exactly, which is the property the pivot rehearsal asserts.

3. **Absence is refused, never guessed.** An `objects` import with no `interner` is a
   `schema-violation` naming the consequence, not a best-effort reconstruction: a wrong
   index space is indistinguishable from a right one at import time and shows up only as
   changed lore. An append whose `interner` is not an extension of the one the store
   already resolves against is refused the same way — the store enforces this a layer
   down, but as an `internal` error, and a caller mistake must read as a caller-facing
   refusal.

4. **`format: rendered` from a build with no renderer is a loud refusal** (`not-wired`)
   naming both ways forward — export `objects`, or use a `syntax` build. Never a silent
   empty array, which reads as "this corpus is empty". The JSON-first default is
   unchanged: `format` absent or `objects` answers with fact objects on either build, and
   only an explicit request for a rendering can refuse.

5. **Error-code discipline, restating what DM-3 implemented.** Build-capability refusals
   are `not-wired` (a `.dli` import, a rendered export, on a build that links no front
   end); call-precondition violations are `schema-violation` (`init` into a populated
   store, both or neither source form, a corpus op on a `.dli`-sourced document, a missing
   or regressing interner). A corpus op requires a store-backed document and refuses a
   `.dli`-sourced one by name rather than handing it an empty store — the manifest is
   where a document's source form is chosen. A commit referencing an object the store
   cannot produce is `evidence-corrupt`, never a silent skip.

6. **The default lane runs the store-backed and span-identity tests.** DM-2 landed both
   as default-build properties and neither was in the lane that proves it; the canonical
   gate now runs them, and `.github/workflows/ci.yml` runs the same argv — the CI-parity
   oracle caught the workflow lagging the gate. That oracle's own assertion is now
   component-wise rather than a frozen argv, so adding a default-lane test reads as the
   strengthening it is instead of a regression.

**Not done, and named rather than half-built.** Per-declaration `author.discard` still
refuses on a build that links no formatter. Re-keying it on the structural key is only
half the change: the key exists and is stable, but nothing surfaces it, so a default-build
caller has no way to *name* the declaration it wants dropped. Choosing that surface — a
staged-key field on `author.diff`, the staged fact-group hash, or something else — is a
wire decision, and inventing one inside this change would be guessing at the consumer's
review flow. It is deferred to DM-4, where the authoring tools and `SKILL.md` are rewritten
together. The current refusal is honest and names its way forward, so nothing is broken
meanwhile; what is missing is a capability, not a correction.

## DL-0057 — DM-5: the native repository validator's scope is closed at eleven codes, V9 absorbed

*Date: 2026-08-04*

**Attribution:** machine — an engineering decision by the implementing agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** The native repository validator (`dli_world::validate_repository`,
`crates/dli-world/src/repository.rs`, served as `deloreis validate-repo`) is ruled
**scope-closed at eleven codes: V1–V8 and V10–V12**. It gets a governance home —
PLAN node `VAL-1`, the design note `docs/design-notes/native-repository-validator.md`,
and a committed detection corpus — and its finding shape becomes a promise the
consuming project may build against.

1. **There is no V9, and its absence is a ruling rather than an omission.** The legacy
   V9 forbade a ratified claim from resting on a proposed one. Its native successors are
   two committed artifacts, and the honest grade is **different for each half**:

   - **The status-leak half is `checked`.** `crates/dli-core/src/derivation.rs:142`
     (`Derivation::support_of`) computes a derived atom's support as the conjunction of
     its rule's support with its premises' supports. A conclusion therefore *cannot* be
     more certain than what it rests on — the violation is not caught, it is
     unrepresentable. `docs/spec/02-semantics.md:78` states this as the theorem it is.
     This is the strongest grade in the ladder and it is earned: nothing is testing a
     behaviour, the type of the computation forbids the state.
   - **The row-discipline half is `tested`.** `crates/dli-world/tests/mig1_replay.rs`
     cross-checks every vendored row's stored `status:` against the standing the kernel
     derives, and proves the cross-check is not a null oracle with three detection legs —
     `detects_flip_proposed_to_ratified`, `detects_forged_promotion_strip_decisions`, and
     `detects_break_status_marker` — beside a pristine census that finds zero
     disagreements.
   - **Legacy equivalence is `attested`, and the caveat is stated rather than buried.**
     What V9 *was* is characterised from `02-semantics.md`'s account of it. The Python
     V9 input file was deleted by the consumer's D-0245 source flip, so this is a
     characterisation and **not a replay**: we cannot re-run the legacy check and diff it.
     Labelling that honestly is the whole difference between an absorption ruling and
     absorption by oral tradition. Under the weakest-grade rollup (DL-0006) the
     absorption claim as a whole is therefore `attested`, even though its structural
     half is `checked`.

   V9 accordingly gets no corpus operator, and the detection suite asserts its absence
   rather than leaving the gap silent.

2. **The finding shape is a promise, not an implementation detail.**
   `RepositoryFinding { path, line, code, message, severity }`, serialised as that JSON
   object; `severity` is always `"error"`; `path` is a relative POSIX path from the canon
   root; results are sorted by `(path, line, code, message)`. Unreadable files become
   findings and a missing root yields an empty list — the library never panics on input,
   and the CLI, not the library, decides that a missing root is a usage error. The design
   note is the normative statement, including the two line-numbering deviations found
   while building the corpus (V6/V7 report line 1 because the YAML reader keeps no
   position; V8 reports a body-relative line, not a file line). Those are recorded as
   they behave, not as they ought to; changing either is a behaviour change and needs its
   own entry, because the consuming project's shadow-validate harness is about to pin
   them.

3. **The detection corpus is deloreis-owned and is the validator's first real oracle.**
   `crates/dli-world/tests/data/validator-detection/` holds a baseline canon repository
   that validates to the empty finding list plus twenty-six seeded-divergence operators
   — at least one per emitted code, **eleven of eleven covered, with no honest miss and
   nothing declared `xfail`**. `crates/dli-world/tests/repository_detection.rs` walks it
   offline and deterministically over committed inputs and asserts the **exact** finding
   set for each operator: path, line, code, and a distinguishing message fragment. The
   pre-existing unit tests in `repository.rs` are not superseded, but they are not this:
   they assert that *some* finding matches a predicate, which cannot see a check that
   fires twice, fires against the wrong file, or reports the right code for the wrong
   reason. The message fragment is in the corpus because that last failure actually
   happened while it was being written — a malformed YAML append and a duplicate registry
   id both land on `data/characters.yaml:1 V6`, and a triple-only oracle blessed the
   wrong catch.

4. **The consumer-side items are consumer-owned, by name.** Their DP-008 **X6**
   direct-SQL lineage negatives and **X8** propagation/index parity are not deloreis
   work and no deloreis node claims them. This repository delivers SP-4v — the corpus,
   the mapping table, and the finding-shape promise — and their shadow-validate accrual
   runs against it on their side. The `native-lineage-negatives` branch, opened here to
   attempt X6 locally, was a dead end and is recorded as one: the negatives need a
   database this repository does not have and must not grow.

**Rationale.** The validator was functionally complete and governance-invisible: no node,
no entry, no note, and therefore nothing `cargo xtask authority` could hold it to. That
is a worse state than incomplete, because completeness with no oracle reads as
completeness. Two things follow from fixing it in one change rather than two.

The first is that closing the scope and proving detection are the same act. A ruling that
"there are eleven codes" is only worth the corpus that would notice a twelfth appearing or
one of the eleven going quiet, so the suite reads the code set out of `repository.rs`'s
own source and refuses to let `EMITTED_CODES` be a constant the corpus merely agrees with.
V9 returning would fail that assertion, which is the mechanical form of this entry's first
clause.

The second is that a baseline is worth as much as the operators. Every operator is a delta
from a repository that validates clean, so an operator's expected set is exactly what its
seed caused — and the baseline itself carries the negative controls that no operator can:
the exempt files that need no frontmatter, the `note` register that may carry no passage,
the `proposed` section that must cite nothing, the whole-file and section-scoped `affects`
targets that must *not* raise V11. A validator that reports on clean input is worse than
one that misses, because it teaches its readers to ignore it.

The corpus also surfaced a real divergence inside the validator, recorded rather than
smoothed away: `check_sections` parses a status marker strictly, rejecting the entire
marker when a citation is malformed, while `check_citations` scans the same line loosely
and never validates citation shape. A section citing `D-0001-forged` therefore draws both
V3 ("no well-formed status marker") and V4 ("cites unknown D-0001-forged"). Both readings
are defensible. The operator `v4-malformed-decision-reference` pins the pair, so a future
tidy-up that unified the two parsers would have to argue for the change instead of
shipping it as a cleanup.

**Not done, and named rather than half-built.** Two branches are reachable in production
and not from committed inputs: `check_frontmatter`'s `cannot read file` and `read_yaml`'s
`cannot read YAML` both require a path that exists and cannot be read, which git cannot
record portably. Seeding them would mean a `chmod` or a broken symlink and a
platform-conditional operator — a worse trade than a named gap, since both branches only
convert an I/O error into a finding and the property that matters, that they are findings
and not panics, is proven elsewhere. The corpus README names them; nothing claims they
are covered.

## DL-0058 — DM-4: the expert authoring tools, and the authority scrub lands at contract 1.3.0

*Date: 2026-08-05*

**Attribution:** machine — an engineering decision by the implementing agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** Contract `1.3.0` adds the expert authoring surface the consumer's corpus
needs, implements the authority scrub DL-0054 ruled, and closes the per-declaration
`author.discard` deferral DM-3 recorded.

1. **Seven tools land, in two tiers, and the tiers are a scope ruling.** Wave 1 is the
   language lane the consumer's corpus actually contains — `author.add_dialect`,
   `author.add_name_part`, `author.add_name`, `author.add_term`, `author.add_banned`.
   Tier 2 is `author.add_state` (time-varying fluents, their episode continuity) and
   `author.add_attest` (their question-ledger flow). Each is a typed constructor over an
   AST declaration that already existed, staged through the existing path: same durable
   lorestore write, same digest, same graded reply. **Tier 3 — `rule`, `practice`,
   `proof`, `cycle` — is parked behind a named trigger, first consumer use**, and tier 4
   (`word`, `narration`, `variant`, `principle`, `export`, `draft`, `use`, `kind`)
   indefinitely. This re-scopes DL-0050's deferred "the remaining declaration kinds have
   no tool" item: the gap is now a decision with a trigger rather than an omission. A
   tool built before a caller exists is a guess at a shape, and every shape here was read
   off a corpus that exists. `Use`/`Kind` remain unbuilt for the same reason — campaign
   honest-unknown 5 asked for proven need and no mapping table has shown one.

2. **Params carry the typed interiors minus spans.** The LS-9 interiors pair each typed
   `item` with the normalized clause text and span the parser captured for exact
   round-trip. A caller has no source to point into, so the wire carries the items alone
   and the lowering synthesizes both. `author.add_dialect` therefore takes `dli_ast`'s own
   `DialectItem` sequence — one type, not two kept in step. `author.add_name` is the one
   deliberate narrowing: of `NameItem`'s four variants it exposes `Composition` and
   `Bearer` as typed params, omits `Provenance` (a second spelling of the tool's own
   `provenance`, which the compiler's header-first precedence would then have to choose
   between) and omits `Other`, which is by definition a clause the grammar did not
   recognize and which a typed constructor cannot produce.

3. **The authority scrub lands at contract ≥ 1.3.0, superseding DL-0054's "≥ 1.2.0".**
   DL-0054 named 1.2.0 when it was written; the minor ladder moved past it before the code
   landed (1.1.0 took the storage schema, 1.2.0 the corpus exchange), so that number is
   corrected here rather than in DL-0054, which is append-only and stands as written. At
   1.3.0 and above a `cited` provenance lowers to a real `decided` annotation carrying its
   ids: the engine records the attribution the caller states and stops asking whether the
   caller was entitled to state it. **Every version below 1.3.0 keeps
   `provenance.cited-refused` byte-exact — permanently, not transitionally.** That freeze
   now covers `1.0.0`, `1.1.0`, and `1.2.0` as well as the whole `0.x` prefix: those three
   shipped with the refusal and released fixtures pin it. A caller opts into the scrub by
   negotiating up, exactly as it opts into every other version-gated change.

4. **Referential integrity is the half that is kept, and it is fail-closed on every
   wire.** A cited decision must resolve against the decisions the corpus declares — the
   `ratify` set, which is the same source `dli-eval` reads. An id that does not resolve
   leaves its decision variable **open**, so the claim resting on it can never read
   `certain`, and raises `citation.unknown-decision` at error severity, which bars
   materialize. The finding is now raised twice on purpose: once by the compiled staged
   world, as before, and once by the add itself, so a mutating call answers with the
   findings its own claim raised (spec 11 §4) instead of deferring them to the next
   `author.check`. Recording a citation is not believing it, and that distinction is what
   keeps the scrub from converting "the engine refuses citations" into "the engine accepts
   forged ones".

5. **`author.discard` names a declaration by a structural key from 1.3.0.** The key is
   `jcs:sha256:` over the declaration's canonical JCS bytes, listed per declaration in
   `author.status`'s new `staged` rows. This closes what DM-3 deferred with its reason
   recorded: the key existed but nothing surfaced it, so on a build with no formatter the
   per-declaration form had to refuse rather than match nothing. The internal fallback it
   replaces was `serde_json::to_string` of the declaration, which could not be promoted to
   a wire identifier — its bytes follow the order the struct declares its fields, so
   reordering two fields would have re-keyed every staged declaration of that kind without
   changing any of them. JCS fixes the encoding, so the key is a function of the
   declaration's serialized content under a spelling a caller can reproduce, and it is
   identical on a default build and a `syntax` build. Below 1.3.0 the rendered-line key and
   its refusal are unchanged.

   **What that stability is and is not**, recorded because the wire schema promises it to a
   consumer. Canonical encoding buys reorder-stability; it does not buy immunity to schema
   growth. A field that serializes is content and moves the key — including one that
   serializes as `null`, which is what an `Option` without `skip_serializing_if` does when
   absent, and that is every optional field on the declaration types as they stand. Only a
   field elided from the bytes entirely leaves the key alone. So a key is valid **within a
   contract version**, and the schema says so rather than implying a durability the digest
   does not have. An earlier draft of this entry and of the `StagedDecl::key` description
   claimed stability "across additive schema changes"; that was wrong in the direction that
   costs a consumer real work, and it is corrected here before either shipped.

6. **Every version gate is a floor, not an equality.** `capabilities()` gated the DM-3
   corpus ops on `matches!(dli, "1.2.0")`, which was correct exactly while 1.2.0 was the
   newest version. Adding 1.3.0 to the accepted set would have dropped
   `dli.corpus.import`/`export` from the newest wire's capability list with no test
   failing — the tier assertion in `ls8_contract_1_0` was written against the same exact
   match, so it would have agreed. Both are now floors stated independently of each other,
   and a second test asserts the floor property per gate over every accepted version, in
   both directions. This was verified by seeding the exact-match regression and observing
   both guards go red.

**Rationale.** The scrub is the substantive change, and it is a correction of the
engine's model of its own callers rather than a loosening. The old refusal encoded a
social fact — that a human author, and only a human author, mints citations — as a wire
rule. Every caller of deloreis is a machine, so the rule never distinguished a human's
decision from an agent's; it distinguished which field the machine filled in, and its
practical effect was to teach agents to route human rulings *around* the provenance
system rather than through it, because the one field designed to carry a ruling was the
one field they were forbidden to use. What the engine can actually check is whether a
cited decision exists, and that check is strictly stronger than the refusal it replaces:
the refusal never inspected the ids at all.

The version gate is doing real work here and is not ceremony. This is the first change in
the campaign that alters what an existing request *returns* rather than adding a request
that did not exist, so it is the first that could break a consumer mid-cutover — and the
consumer is mid-cutover. Gating it means the same binary answers the old callers and the
new one, and the evidence is mechanical rather than argued: the `ls8_zero_x_parity`
corpus reproduces every captured `0.x` response with exactly two deviations, both of them
the version list growing in a refusal whose whole purpose is to enumerate it, and neither
touching the scrub; three new goldens pin the refusal at `0.5.1`, `1.0.0`, and `1.2.0`;
and a test drives all nine frozen versions through a `cited` add and asserts the finding,
its message, and its repair verbatim.

The tier ruling deserves its own note because "we did not build it" and "we decided not to
build it" look identical in a diff and are opposite in a review. DL-0050 recorded the
missing tools as a deferred item, which reads as debt. Most of it is not debt: `word`,
`narration`, `variant`, and `principle` have no consumer, and building typed constructors
for them would freeze param shapes against a guess, on a wire where param shapes are the
contract. Naming the trigger — first consumer use — converts the gap into something a
future reader can act on, and keeps the tool surface a record of what is authored rather
than of what is declarable.

**Not done, and named rather than half-built.** `DialectItem::Other` and
`NameItem::Other` round-trip through the params but render as empty clause text, because
an unrecognized clause has no keyword to reconstruct — the grammar did not classify it,
so the lowering does not guess. They are reachable only by a caller that deliberately
sends them, and they carry no meaning to the compiler either way.

## DL-0059 — The corpus door: a commit records the scope its facts were minted under, an empty corpus is one holding no facts, and an imported claim id is checked

*Date: 2026-08-05*

**Attribution:** machine — an engineering decision by the implementing agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** Two corrections to the corpus door DL-0056 opened, both found by the
consumer's FW-6 pivot attempting the one thing the door exists for: seeding a store from
`.dli` text.

1. **The lowering walk uses the loader's scope name, empty fallback included.** A
   document with no `scope` header belongs to the **empty** pack — that is
   `dli_world::loader`'s own rule — and the store's per-declaration walk now says the
   same. It had fallen back to the *pack* name instead, so a scope-less document minted
   its proposal bundle as `h.<pack>#0` where the compiled model minted `h.#0`. The
   faithfulness guard did exactly its job and refused the import; the guard was right and
   the walk was wrong. This is a bug fix, not a decision: the loader defines the canonical
   interner and the walk's whole contract is to track it.

2. **A seeding commit records the documents' *scope*, never the wire document id.** These
   are different things and conflating them corrupted the round trip. The walk mints a
   proposal's bundle under the document's scope (`h.<scope>#N`); `reconstruct_pack`
   re-derives both that prefix and the scope header it re-emits from the commit's recorded
   pack name. Where the recorded name was the document id and the id differed from the
   scope, reconstruct looked for the wrong prefix, failed to recognize the bundle, and
   re-rendered it as a lore **tag**: `proposed` came back as `proposed #0`, under a `scope`
   header nobody wrote — on the served world and on the rendered export. DM-3 named exactly
   this hazard on the other write path ("re-render every committed fact's provenance under
   a scope it was never written in") and guarded it there; this arm was still taking the id.
   Recording the scope makes the two agree by construction, so reconstruct needs no special
   case, including for a scope-less corpus, whose empty name already suppresses the header.

3. **`init` refuses on a store that holds *facts*, and nothing weaker.** Two weaker tests
   were tried and both were wrong. `branch_head().is_some()` is wrong because opening a
   lorestore writes a genesis commit, so it is `Some` for every store the server can serve
   — which made `mode: init` unreachable over the wire, leaving `append` the only way to
   seed, which for a non-Rust consumer is no way at all. The committed *pack name* is wrong
   too, and worse: under ruling 2 a scope-less corpus records the empty name, which reads
   as unnamed, so a populated store would have answered "not seeded" and `init` would have
   imported straight over it. Only the facts answer the question being asked.

4. **An imported object's `claim_id` is re-derived and checked, from contract 1.3.0.** A
   claim id is a claim *identity* minted by `ClaimId::from_label` — `atom:<pred>(<args>)`
   for a fact, `decl:<Kind>:<name>` for a carried declaration — and deliberately not a
   content hash of the fact. That makes it the one field of an imported object nothing else
   can vouch for: the object's own content hash covers the bytes as sent, so a wrong id
   yields a perfectly valid object that `export` then hands back as truth. It was accepted
   verbatim, so a caller could store an identity disagreeing with its content, or give two
   atoms the same one; content-addressing was simply not enforced on that path. It is now
   checked, and a mismatch is refused naming the id the content mints.

   **Checked, not silently derived.** Deriving and ignoring the caller's value is the
   quieter fix and the wrong one: it discards what the caller stated without saying so —
   the same lossy-exchange pattern DL-0056 refused for a dropped interner — and it would
   hide a broken emitter instead of reporting it. The objection to checking, that it forces
   external emitters to reproduce the engine's canonicalization, does not apply: the label
   is a short documented string and the id is its SHA-256, with no JCS anywhere.

   **Gated at 1.3.0, and the mislabelled golden is deliberately left alone.**
   `contract/1.2.0` is a tagged version that really does accept a mislabelled object, and
   `golden/corpus-import-v1` records that it does — its `rowan` object carries
   `sha256("atom:person(wren)")`, copied from the object beside it. Correcting those bytes
   would falsify what a released version is on record as doing, and refusing on a `1.2.0`
   wire would change a released version's behavior. So the check is a floor at `1.3.0`, the
   golden keeps its bytes, and the same object is pinned twice more: accepted at `1.2.0` in
   `golden/corpus-import-claim-id-frozen-1-2-0-v1`, refused at `1.3.0` in
   `negative/corpus-import-mislabelled-claim-id`.

**Rationale.** Rulings 1 and 2 are bugs and the interesting part is why they survived. Both
committed corpus fixtures write a `scope` header, where walk and loader agree, and the
only scope-less case anyone had exercised was the *empty* document — which mints no
bundle at all and therefore cannot diverge. The suite held a case on each side of the
defect and none through it. A guard that fires correctly on the first real caller is the
system working; a test corpus whose every member dodges the same branch is the thing to
fix, so the round-trip test added here drives scope-less text with **two** proposals,
because one would not have caught a counter that failed to advance.

Ruling 2 is the sharper version of the same lesson, and it was found only because ruling 1
unblocked the path that reaches it. The round-trip test written for ruling 1 used a
*carried* declaration — one with a label, so `carriage_kind` is `Some` — which replays from
its stored carriage JSON and never touches the polynomial-to-annotation path at all. It
could not have failed. The defect lives on bare `Fact`/`Attr`/`Entity` declarations, and it
is not really about scope-less-ness either: a document with `scope tpth` served under
document id `beta` — a configuration DM-3 explicitly supports — corrupted the same way on
the `1.2.0` wire, before any of this work. Two tests, each written to cover the thing the
other missed, and the defect sat in the gap. The round-trip test now runs a table over the
shapes that differ *structurally in the store* — bare and carried, scoped and not, id equal
to scope and not — and asserts text in equals canonical text out.

Ruling 3 is a decision, which is why this entry exists at all rather than the fixes landing
bare. "Empty" had two defensible readings and the code picked the one that made a
documented wire mode dead: a consumer could not seed a fresh corpus through the interface,
only extend one that some out-of-band process had already created — which for a non-Rust
consumer means no path at all. Reading emptiness as *holds no facts* completes the wire
and costs nothing, because the guard it replaces was never protecting anything a caller
could reach. Choosing the reading that leaves an operation usable is not a loosening here;
the refusal it preserves is the one that was always meant — and the second attempt at that
predicate, the committed pack name, would have been a genuine loosening, which is why the
one that survived is the one that asks about facts.

**Not done, and named rather than half-built.** `LoweringDiverged` still maps to the
`internal` wire code rather than a caller-facing one. That is arguably correct — the walk
failing to track the loader is a server defect, not a caller mistake — and with this fix
no caller input is known to reach it, so re-grading it now would be a guess at a surface
nobody is standing on. It is named here so the next report against it is read as evidence
rather than as a fresh discovery.

## DL-0060 — The repository validator's V8 lines are corrected to file-absolute; V6/V7 line-1 and the strict/loose marker-parser split are ruled accepted

*Date: 2026-08-05*

**Attribution:** machine — an engineering decision by the implementing agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** DL-0057 §2 recorded two line-numbering deviations in the native repository
validator and held both recorded-not-fixed, on the ground that the consuming project is
about to pin this output and a silent renumbering inside a parity-support change would be
the drift the finding-shape promise exists to prevent. This entry is the promised own
entry. It fixes one, keeps the other, and rules a third disagreement — the strict/loose
marker parsers — accepted. It lands ahead of the consumer's FW-9, so FW-9 pins the
corrected output and does not re-pin.

1. **V8 now reports file-absolute lines; the body-relative numbering was a bug, and the fix
   is strictly more correct than a renumbering.** The banned-vocabulary scan
   (`check_lexicon`) numbered lines within the document *body*, and that body slice began at
   the frontmatter's *closing* `---`, which became line 1. So for a document whose
   frontmatter occupies `F` lines including both delimiters, a hit on file line `N` was
   reported as `N − F + 1`: a word on file line 15 under an eight-line frontmatter reported
   as line 8, and a word on file line 13 under a seven-line frontmatter reported as line 7.
   That is a genuine violation of the `RepositoryFinding` contract, whose `line` is
   documented as the one-based *file* line and which every other check (V1–V5, V10–V12)
   already honours; a consumer diffing the array against the file would land the caret seven
   lines high. The scan now enumerates over the whole source and skips the frontmatter block
   by *line count*, so the reported line is the real file line.

   The finding **shape** is untouched — `severity` is still always `"error"`, the list is
   still sorted by `(path, line, code, message)`, a missing root is still an empty list — but
   it would be false to say only the `line` value changed and the same words are caught. The
   old slice was computed by byte arithmetic (`offset += line.len() + 1` per line, then a
   substring), and replacing it with a line-count skip removes two spurious-scan bugs that
   rode along with the wrong numbering:

   - **CRLF documents.** `str::lines()` yields each line without its `\r`, so the old
     `len + 1` charged one byte for the terminator where a `\r\n` file spends two. The offset
     undercounted one byte per line and the slice began *before* the closing `---`, inside
     the last frontmatter field — so a banned word in the *metadata* was reported as a V8
     finding, on bytes the scan must never see. A regression test drives a ten-line CRLF
     frontmatter whose only banned word is its last field and whose body is clean: the fix
     emits no V8 finding, the byte scan emitted a false one.
   - **Unterminated frontmatter.** With no closing `---` the old loop consumed the whole
     file and the offset ran to (roughly) the end, leaving an arbitrary trailing slice
     defined only by the opening delimiter's length — nonsense input that happened to match a
     word only by accident. The fix skips every line waiting for a close that never comes and
     scans nothing; the unterminated block is still its own V1 finding.

   So the change to output is: V8 line values corrected, plus two classes of *false* V8
   findings removed. No legitimate catch is dropped or relabelled. The correction is pinned
   non-vacuously: both detection-corpus operators now assert the absolute lines (15 and 13,
   over frontmatters of different heights so a partial regression cannot hide); a unit test
   asserts a below-frontmatter hit at file line 11, where the old numbering said 5; and the
   CRLF and unterminated cases each have a pin. A regression to the byte scan reds them.

2. **V6 and V7 findings stay at `line: 1`, and this is accepted, not a bug — with one honest
   caveat.** They are file-level findings: the structural checks read a parsed YAML value
   that keeps no source position, and V7 is a whole-file presence check on the registry↔file
   join. The `message` names the offending id; line 1 is a file-level convention. This is a
   different kind from V8's defect: V8 had a real line and reported the wrong one. The caveat
   is that it is not literally true that these findings *have* no line to report — the
   **invalid-YAML** V6 branch parses through `serde_yaml_ng`, whose error carries a location,
   and that line is currently discarded and reported as 1 like the rest. Surfacing it is a
   tracked follow-up (below), out of scope for a V8 line-number fix; the convention is kept
   for now and pinned by two unit tests — one over the structural V6/V7 findings and one over
   the invalid-YAML branch specifically — so a later change to real line numbers needs its
   own entry rather than arriving as a side effect.

3. **The strict V3 and loose V4 marker parsers are allowed to disagree, by design — but the
   guarantee is the gate, not that V4 always names the reference.** V3's marker check uses
   the strict `parse_status_marker` (status in the enum, every reference well-formed); V4's
   citation resolution uses the looser `section_citations` (it lifts a parenthesised,
   `, `-separated reference list out of the first non-blank line under a heading). What holds
   is the **gate**: `parse_status_marker` rejects every ill-formed marker, so V3 fires on all
   of them and no malformed-marker section passes unflagged, and once a marker is well-formed
   V4 resolves its references reliably. What does **not** hold — an earlier draft of this
   entry wrongly claimed it did — is that V4 always names a dangling reference hidden behind
   a *malformed* marker. The loose extractor is best-effort and unreliable on malformed
   input, in two opposite ways, both now pinned:

   - **It can miss.** A trailing character after the closing paren
     (`> status: ratified (D-9999).`) defeats V4's `strip_suffix(')')`, so it lifts zero
     references and stays silent on the dangling `D-9999`; only V3 fires, and V3's message
     names no reference. That run reports *that the marker is malformed* but not *which id
     dangles*.
   - **It can invent.** References joined by ` and ` instead of `, ` are lifted as one
     mangled token, so V4 reports a phantom composite id (`D-0001 and D-0002`) as unknown
     even though both real ids resolve.

   This is ruled **accepted**, not fixed: hardening `section_citations` is out of scope for a
   V8 line-number change, and the gate — V3 flags every malformed marker — is what makes the
   fragility tolerable (a reader is always told the marker is wrong; V4's extra line, when it
   fires, is a best-effort bonus and, when it misfires, is bounded to malformed markers V3
   has already condemned). All three faces — miss, phantom, and the ordinary case where both
   fire on a nearly-well-formed marker — are pinned by one unit test.

**Rationale.** The three items look alike — all three are "the line/parse behaviour is not
what a naive reader expects" — and the ruling is that only one is a *defect against a
promise*. V8 was: the `line` field is contracted as the file line and V8 returned a
different number, so a shadow-validate harness comparing carets would silently misalign; and
because the wrong number came from byte arithmetic, fixing it also retired the CRLF and
unterminated false findings that arithmetic produced. V6/V7's line 1 is a named file-level
convention, honest about the one branch (invalid YAML) where a real line exists but is not
yet surfaced. The parser split is tolerable not because "both always fire" — they do not —
but because V3's gate condemns every malformed marker, so V4's unreliability is confined to
input already flagged and never touches well-formed markers. The discipline this entry tries
to hold is that a top-authority ledger states what is *tested*, not what is rhetorically
tidy: each claim above has a pin, including the unfavourable ones.

**Tracked follow-ups (named, not built here).** Two are registered so the next report against
them reads as known ground, not fresh discovery, and neither is touched by this V8 fix:

1. **V4's `section_citations` is a fragile parser.** It can miss a reference (a trailing
   character) and invent one (a non-`, ` separator). A hardening pass would share the strict
   `parse_status_marker` tokenizer so V3 and V4 agree on what a reference is; it is deferred
   because it changes V4's finding set and belongs with the consumer's marker-shape work, not
   a line-number fix.
2. **The invalid-YAML V6 line is available and discarded.** `serde_yaml_ng`'s parse error
   carries a location that could replace the `line: 1` convention on that one branch; doing so
   is a finding-shape change and needs its own entry.

**Docs.** `docs/design-notes/native-repository-validator.md` §3 now records one line-number
deviation (V6/V7) rather than two, notes V8 corrected here and the invalid-YAML caveat, and
frames the fix as removing spurious scans rather than merely renumbering; the detection
corpus's `operators.yaml` header and both V8 operator rationales are updated to the absolute
lines. The design note is the normative finding-shape statement the consumer compares
against, so this entry is the ledger evidence that reopens it (DL-0011).

## DL-0061 — Caller-supplied text in the authoring finding channel is control-char-sanitized at the serialization boundary

*Date: 2026-08-05*

**Attribution:** machine — an engineering decision by the implementing agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** The graded findings a mutating `author.*` tool returns are the LS-5 review
surface a reviewer reads. Their `message` field `format!`-s **caller-supplied** text —
entity ids, hypothesis and question ids, cited decision ids, event and point names,
attribute values — straight into the sentence the reviewer sees (`entity_unknown`,
`hypothesis_unknown`, `question_unknown`, `citation_unknown`, `order_cycle`,
`event_era_*`, `contradiction`, `name_unvetted`, and the world/compile-sourced
`world_finding`, `contradiction_finding`, `uncompilable`, which carry caller ids
transitively). Nothing escaped it. This is the exact forgery class DM-4 (DL-0058)
closed for the rendered `.dli` diff channel — a newline in a caller id forges a second
review line, a bidi override silently reverses one — still open on the finding-**message**
channel. A concrete repro: `author.suppose` with a hypothesis id `"ghost\nhypothesis
forged \"Forged\" decided d-9999"` returned `hypothesis.unknown` with a **raw** newline
in its message; the id is a bare param, so the `field.control-char` guard that bars
materialize does not even fire on it — the message escape is the only defense.

1. **The escape is applied once, at the `Finding` serialization boundary, reusing DM-4's
   sanitizer.** `dli_serve::finding::Finding.message` carries `#[serde(serialize_with)]`
   that runs the caller text through `author::render::escape_controls` — the very
   function DM-4 built for the diff channel (`field.control-char` / free-text machinery),
   not a new one. This covers the **class at a boundary**, not N builder sites: every
   `Finding`, from any builder, in any result struct (`AuthorMutationResult`,
   `AuthorStatusResult`, `AuthorDiffResult`, `AuthorCheckResult`, `AuthorFindingsResult`,
   `AuthorMergeResult`) and every future one, is escaped on the wire, and so are `dli.*`
   producers of the same type (`dli.name.check`'s `name.confusable`), since the boundary
   is the type's serialization, not one tool. `Finding` has no non-serde egress — no
   `Display`, no manual `Serialize`, no `to_string` — so serialization is the whole exit.

2. **The one copy that routes around the field is escaped consistently.**
   `finding_conflict` copies `finding.message` into `DiffConflict.rendered` — a separate
   `String` that bypasses the field's `serialize_with` — for `author.diff`'s consequence
   channel; it now escapes the copy with the same `escape_controls`. An exhaustive sweep
   found this to be the **only** such bypass; every other read of a finding message is
   either mapped back into `Finding.message` (covered) or internal-only (a dedup or sort
   key, never emitted).

3. **The finding SHAPE promise (DL-0060) is preserved.** Severity, code, grade, repair,
   citations, and the sort/dedup order are untouched; only the `message` field's bytes
   change, and only for the forbidden characters. `escape_controls` is a strict **no-op**
   on control-char-free text, so every well-formed message — including every released
   fixture's — serializes byte-identically, and the `0.x` wire parity the fixture lane
   pins is unaffected. A unit test asserts the clean-message identity; a second asserts a
   newline-and-bidi message serializes to its exact escapes; the `author.suppose` and
   `dli.name.check` repros assert the escape end-to-end in both build lanes. A regression
   to raw interpolation reds them.

4. **Scope: this closes the `dli_serve::Finding` type — the authoring finding channel —
   mirroring DM-4's diff-channel scope, and no more; the resulting asymmetry on the
   `dli.check` surface is stated, not papered over.** Because the escape rides the
   `Finding` **type**, it also escapes the one `dli.check` finding that *is* a `Finding`,
   `CheckFinding::World`, as a side effect — strictly safer (a no-op on clean text) but
   leaving that surface **internally inconsistent**: on the same `CheckResult.findings`
   enum, `CheckFinding::World` is now escaped while its siblings —
   `CheckFinding::Payload(PayloadFinding)` (`dli-world` `payload.rs`) and the four
   `Attestation*Finding.message` (`contract.rs`, doc-commented "domain-spoken rendering")
   — are their own types that never pass through `dli_serve::Finding` and still serialize
   their messages raw. This entry makes **no claim that those peers are safe**: no
   forgery repro was built against them, so their risk is **unestablished**, not cleared.

   What *is* established is the surface's documented role. `dli.check` is a
   validation/batch/gating op whose **machine** fields drive consumption — `ok` (no
   blocking error), the weakest procedure `grade`, and the stable finding `code` a
   benchmark or repair tool matches on (`docs/integration/fantasy-contract.md` §3.1;
   `docs/spec/10-serving.md`; `docs/reference-generated/finding-families.md`) — and it is
   explicitly *not* the LS-5 reviewer channel that `author.*` is. Whether a **human ever
   reads the `message` text** of a `dli.check` finding is **unestablished**: the docs
   hedge, and `finding-families.md` tells a reader to "read the message" of exactly these
   kernel families, so a reviewer-forgery path there cannot be ruled out either. Because
   the risk is unestablished on a cross-crate, different-lane surface, folding the peer
   types in here would widen a scoped change on unproven need; the asymmetry is **logged
   for separate routing**, exactly as DL-0060 logged V6/V7 rather than renumbering under a
   parity-support change.

## DL-0062 — De Lore Is is epistemic infrastructure; the archive destination is peer plus service

*Date: 2026-08-07*

**Attribution:** machine — an engineering and documentation decision made from
the author's supplied product direction. It is live and binding on the tree,
pending async review under DL-0052.

**Ruling.** De Lore Is is a deterministic **epistemic engine** for agents and
their consuming applications, not an agent, a model orchestrator, or a
fiction-only language. Fictional-world authoring, TPtH, and the existing domain
modules remain first-class reference applications. The engine's durable core is
the content-addressed fact store, provenance/defeater algebra, replayable
standing, and valuation overlays.

1. **Evidence first, owned elsewhere.** SubtleMemory, ClawArena,
   FlawedFictions, and ConStory are the current external evidence program in
   `dli-benchmarks`. Strong results across all four are an aspiration, not a
   local release gate or a result this repository may assert without that
   repository's scored records. Their traces may inform engine work but do not
   change semantics by themselves.

2. **The archive destination is a peer plus managed service.** The peer embeds
   the certifying evaluator and uses immutable, content-addressed local cache;
   it computes symbolic fixpoints, standing, explanations, and valuation
   overlays. The service serializes commits and refs, holds immutable objects
   and indexes, and supplies query-relevant provenance clusters. The first
   implementation is an in-process loopback seam; remote transport and object
   storage are later replacements behind that seam. The offline gate and the
   peer evaluator remain network-free.

3. **Supplementary Magic Sets are the service-side bounding algorithm.** Given
   a snapshot-pinned typed goal, the service uses supplementary Magic Sets and
   indexed base facts to construct a complete cluster. It does not produce the
   final standing or act as a stateful SLG/tabling session. A peer evaluating a
   returned cluster must match whole-corpus evaluation for the same root, goal,
   and valuation; otherwise the service returns no answer. This exactness oracle
   is the boundary's certification condition.

4. **Database analogies are constrained by the ledger.** Commits are the
   append-only log; refs plus valuation identify snapshots; caches, indexes, and
   materialized fixpoints are rebuildable readers. Compaction may pack or
   snapshot derived storage but never rewrite committed history. A defeater is
   a conditional proof-path invalidator, not a physical deletion tombstone.

5. **Consensus is an intentionally open product layer.** DLI already provides
   structured submissions, derived standing, conflicts, and explanations — the
   substrate for a deterministic referee. It does not yet define a
   winner-selection rule, trust weights, formal argumentation semantics,
   probability model, authorization policy, or escalation workflow. Each needs
   a later decision and a dedicated oracle. DL-0054/DL-0058 continue to govern
   authority: DLI records caller-supplied attribution and decision state and
   checks referential integrity; consuming applications own policy.

**Rationale.** The old fiction-first wording accurately described DLI's first
corpus but obscured the reusable mechanism. The managed-service destination
must also preserve the distinction between storage/routing and certification:
distributing I/O must not turn a remote heuristic or a model majority into a
verdict. This ruling names that boundary before implementation work begins.

**Revisit trigger.** A proposed public segment wire shape, a second valuation
context, or a consensus-selection policy requires a successor ruling with its
own compatibility and exactness oracle.
## DL-0063 — The `killed` registry predicate is exempt from the factcheck.contradicted killed-hit scan

*Date: 2026-08-06*

**Attribution:** machine — an engineering decision by the implementing agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** `dli_world::check::factcheck::classify` scans every asserted claim's
`args` + `pred` text for killed-token substrings (the `killed_hits` block in
`crates/dli-world/src/check/factcheck.rs`). A `killed` registry fact *records* a banned
token as its **payload**: embedding the token in the fact's value is the fact's purpose,
not a realized usage of a banned form. To make the killed-tokens registry expressible as
store fact objects (LS-9/lorestore pivot, DL-0041/44/45/46) the `killed_hits` scan
**skips** claims whose `pred` text is `killed`; the `rejected_hits` (contextual
rejected-claim) check at L95 is unchanged. This is a narrow, predicate-scoped exemption:
no other predicate is affected, and the matcher still rejects a killed token realized in
any non-registry fact.

**Rationale.** Without the exemption the registry fact that *records* a token is itself
contradicted by the checker that *enforces* the token — a circular contradiction that makes
the killed-tokens domain (`killed_tokens.yaml`, 20 rows) un-storeable, blocking the
`--canon` export's 6th parity domain from ever becoming store-derived. The exemption
breaks the cycle at the predicate boundary while preserving the guardrail for all real
lore. Concretely: a store-borne `fact <entity> killed {token,match,kind,scope,decision}`
is now materializable; the token (e.g. `doom-wight`, `goal`) remains refused everywhere
else under `factcheck.contradicted`.

**Revisit trigger.** Re-evaluated if the killed-tokens registry is re-encoded under a
non-`killed` predicate; otherwise the exemption stands.

**Affected code:** `crates/dli-world/src/check/factcheck.rs` — a guard around the
`killed_hits` scan reading `if claim.pred.text.as_str() != "killed" { … }`.

> **Scope correction / superseded.** DL-0064 supersedes this entry: it under-described
> the exemption set, listing only `crates/dli-world/src/check/factcheck.rs`. The
> `killed` registry predicate is exempt at **all three** scan sites — this
> `classify` `killed_hits` guard (`dli-world/factcheck.rs`), the
> `declaration_rejected_lore_findings` pre-scan (`dli-serve/src/author/tools.rs`),
> and the `certain_rejected_lore` scan (`dli-world/src/check/continuity.rs`). The
> `classify` guard was also revised from the simple `!= "killed"` predicate check to
> the `is_killed_registry_fact` disjunction (see DL-0064). DL-0063 is retained as a
> historical record; DL-0064 is the binding rule.

## DL-0064 — `killed` registry predicate exempt at all three scan sites (supersedes DL-0063)

*Date: 2026-08-07*

**Attribution:** machine — an engineering decision by the implementing agents. Live
and binding on the tree, pending the author's async review (DL-0052). DL-0064
supersedes DL-0063, whose "Affected code" listed only `crates/dli-world`; the full
exemption set spans **both** crates.

**Ruling.** A `killed` registry fact records a banned token as its **payload** — the
token's value is the fact's purpose, not a realized usage of a banned form. The
factcheck/contradiction machinery therefore **skips** the `killed` registry fact at
all three scan sites that would otherwise fire on its banned-token payload:

1. **Staging pre-scan** — `crates/dli-serve/src/author/tools.rs`
   `declaration_rejected_lore_findings` (L1108): returns `Vec::new()` for a
   `Decl::Attr` whose `attr.text == "killed"`, evaluated **before**
   `free_text_fields` → `rejected_lore_findings` → `realizes_killed_token` pre-scan.
   This stops the staging-time killed-token scan from ever staging a blocking
   finding for the registry record.

2. **`classify` killed-token scan** — `crates/dli-world/src/check/factcheck.rs`
   (L121-137): the guard is the `is_killed_registry_fact` disjunction, computed as
   `pred == "killed"` **or** (`pred == "declaration"` **and** `args[1] == "killed"`
   for both `Term::Sym` and `Term::Str`). Only when this is **false** does the
   `killed_hits` block run; the `rejected_hits` (contextual rejected-claim) check at
   L95-123 is untouched. DL-0063 described only the original `pred != "killed"`
   predicate check; the revised disjunction (handling the
   `declaration(subject, killed, value, source)` rendering) is binding here.

3. **Continuity rejected-lore scan** — `crates/dli-world/src/check/continuity.rs`
   `certain_rejected_lore` (L210): skips atoms whose `pred.text == "killed"`
   (`if atom.atom.pred.text == "killed" { continue; }`) after the `Status::Certain`
   guard, so the materialized registry fact is not run through
   `world.killed.hits(token)` → `continuity.rejected-lore`.

**Rationale.** DL-0063's `classify`-only exemption proved **insufficient** under the
live `author.materialize` probe (`/tmp/mcp_author_kill.py`): `classify` alone cleared
the `factcheck.contradicted` check, but `materialize` then surfaced
`continuity.rejected-lore` (scan-site #3 ran over the staged `killed` atom, matching
its embedded token). A second failure mode existed at site #1:
`declaration_rejected_lore_findings` staged a
`declaration(killed-token-registry, killed, {…token…})` claim whose free-text value
contained the banned token, tripping the `realizes_killed_token` pre-scan before
`classify` was ever reached. The three-scan exemption closes both failure modes at
their origin. Without it, the killed-tokens registry (20 rows) is un-storeable,
blocking the `--canon` export's killed-tokens domain (parity.json
`{killed:20, worker_projection:19, rejected_claims:5, excluded_ratified_surface_collisions:1}`) from ever becoming store-derived. The exemption breaks the cycle at the
predicate boundary while preserving the guardrail for all real lore: a store-borne
`fact <entity> killed {token,match,kind,scope,decision}` is now materializable; the
token (e.g. `doom-wight`, `goal`, `Audrey`) remains refused everywhere else under
`factcheck.contradicted`.

**Verification.** `mcp_author_kill.py` probe over `/tmp/s5b` (exact copy of
`worlds/tpth`): `[killed facts] 20/20 authored clean; 0 blocked`; `[materialize]
committed 22 objects; isError=False`; `[canon] killed predicate bucket: 20
objects`; `integrity` root_hash stable at the `/tmp/s5b` tip. `cargo xtask gate
--ci` → `PASSED — 21 steps green`, `GATE_EXIT=0`. This entry is the live binding
superseding DL-0063 (whose "Affected code" is corrected to note the `dli-serve` +
`continuity` sites).

**Revisit trigger.** Re-evaluated if the killed-tokens registry is re-encoded under
a non-`killed` predicate; otherwise the three-scan exemption stands.

## DL-0065 — The killed-tokens canon domain is store-projected via the `killed` predicate bucket

*Date: 2026-08-07*

**Attribution:** machine — a projection design decision by the implementing
agents. DL-0064 opens the `killed` registry fact for storage; DL-0065 settles
how that stored fact yields `parity.json`'s `killed-tokens.yaml` census without
re-touching the YAML. Live and binding on the tree, pending the author's async
review (DL-0052). Supersedes the open `(A1)`/`(A2)` design call text in
`docs/integration/STATUS.md` L154-161.

**Ruling.** The `--canon` export projects the killed-tokens domain (LS-7) purely
from the folded store fact objects at `head`, in four maps that byte-identify to
the committed `worlds/tpth/generated/parity.json` `killed-tokens.yaml` census:

- `killed` (20): every `fact killed-token-registry <json>` row, emitted as the
  verbatim registry-row object from the fact's `value` JSON — the full field set
  `{token, match, kind, scope, decision}` plus any sibling columns the source
  YAML flow-mapping parsed as null-valued keys.
- `worker_projection` (19): the killed rows whose `token` (casefolded) is **not**
  a ratified surface — the same fields minus `kind`.
- `excluded_ratified_surface_collisions` (1 = Frankpledge/D-0138): killed rows
  whose `token` (casefolded) **is** a ratified surface — full field set.
- `rejected_claims` (5): one per guardrail reject cue, expanded from the
  `factcheck-object-{key}` facts' `support` monomials through the interner.

Three empirical rules make the projection store-sourced and deterministic (the
kernel reads no clock, no model — DL-0007):

1. **`match` is data, not a function of the token.** It is read from the stored
   fact's `value` JSON (`token_obj["match"]`) verbatim; it is never re-derived.
   The prior `token.contains(' ')` derivation marked hyphenated tokens
   (`doom-wight`, `doom-ward`) as `"word"`; they are `"phrase"` in the source
   registry. Reading verbatim fixes both.
2. **Commit order is carried by `serialized_objects`.** `by_pred` is a `BTreeMap`
   (alphabetical by predicate) and would reorder the guardrail cue sequence; the
   per-domain lists iterate the commit-order `Vec` instead, so `rejected_claims`
   and the census rows follow registry row order.
3. **The Colophon scope split is reconstructed in-projection.** The store fact
   carries `scope` as a single flat string (the authoring flattened the YAML
   flow-mapping). The projection rebuilds the sibling columns YAML parsed as
   null-valued keys: `scope: a, b, c` → `scope = "a"`, `b = null`, `c = null` —
   today only the Colophon row (`scope: series title — clunky, mismatched,
   suspected AI-literary-vocabulary tic`). The other 19 rows have no `, ` in scope
   and pass through untouched. The `killed`/`excluded` census emits the
   reconstructed object as-is; `worker_projection` emits the 4-key filtered form
   `{token, match, decision, scope}` (scope = the split short form).

**Exclusions** (two), mirroring `killed_tokens()`:
- `gender` is absent from `rejected_claims` — it is ratified (`factcheck-assert-*`,
  no `factcheck-reject-*` cue keys it), so `rejected_keys` never contains `gender`.
- `mortality` is absent — ratified by `D-0120`, but its `(protagonist, mortality)`
  slot is a plain asserted `fact protagonist mortality <value>` in the source
  registry, so `asserted_slots` filters it out (parity L2726-2728 equivalent).

**Interner:** 0-based (`+N` → `interner[N]`); `+1` → `D-0108` (the ratified
batch-002 authoring decision — ALL 20 killed facts carry `support.monomials
[["+1"]]`). `ratified_surfaces` is derived from every `(actor, pred)` pair in the
folded buckets (entity ids + alias/name second-args, casefolded); this is
sufficient for the `Frankpledge` collision (D-0138) which is a real declared
entity.

**Affected code.** `crates/dli-serve/src/handlers.rs`
`canon_killed_tokens()` (and the `Canon => (Vec::new(), Some(canon_export(...)))`
branch in `handle_corpus_export`). Depends on the DL-0064 three-scan exemption
to make the `killed` fact materializable in the first place.

**Verification.** `canon_export{killed_tokens}` over `/tmp/s5b` (exact copy of
`worlds/tpth` + 20 authored killed facts + declaration) byte-identifies to
committed `worlds/tpth/generated/parity.json` (`json.dumps(..., sort_keys=True)`
equality): `killed: 20/20`, `worker_projection: 19/19`,
`rejected_claims: 5/5`, `excluded_ratified_surface_collisions: 1/1`, AND
`by_pred`/`interner`/`decisions` all match. `cargo xtask gate --ci` →
`PASSED — 21 steps green`, `GATE_EXIT=0`. Independent red-team (fresh-context
reconstruction of the four maps from raw objects + a planted divergence) confirms
the projection detects a store mutation → RED.

**Revisit trigger.** Re-evaluated if the `killed` fact's `value` JSON is no
longer a verbatim registry row, or if `compile_lore_dli.py`'s `killed_tokens()`
semantics change (parity.json becomes the stale artifact, per the
reconciled-empty gate on the fantasy side).

## DL-0066 — The facts sub-maps (relations, constraints) reproduce parity from the folded store

*Date: 2026-08-09*

**Attribution:** machine — the LS-7 facts sub-map projection (Path B), the
store-sourced sibling of DL-0065 (killed-tokens) and DL-0068 (the general
by_pred/YAML cutover rule). The projection code committed at `a51f3bd`
(`canon_facts()`) was authored without its own ledger entry; DL-0066 retroactively
records the ruling that the commit already encodes. Live and binding, pending
async review (DL-0052).

**Ruling.** `dli.corpus.export{format:"canon"}` projects the `relations` (12) and
`constraints` (3) predicate buckets from the folded store fact objects at `head`,
as `canonical.facts.{relations, constraints}`, each byte-identical to the
committed `parity.json`:

- `relations` (12 rows across 9 predicates: `adjacent_to`(3), `located_in`(2),
  `upstream_of`(2), `raised_by`(1), `trained_by`(1), `member_of`(1),
  `rigged_variant_of`(1), `reads`(1)) — the `(pred, args)` atoms from the folded
  `by_pred` buckets, emitted in commit order (`serialized_objects`).
- `constraints` (3: `constraint-kind`/`constraint-target`/`constraint-pattern`/
  `constraint-description`) — emitted with their raw-regex `pattern` verbatim
  (the `value` JSON is NOT re-parsed as a regex; the projection emits it as
  found).

Each row carries `decisions` (the `support` monomials expanded through the
interner) and `status` (`support.monomials[0]`'s `provenance.marker` lowercased:
`provisional`/`ratified`). The `support.monomials` literal `+N` is an interner
index (`+N` → `interner[N]`) into the canonical decision set (`interner` len 68).

**Affected code.** `crates/dli-serve/src/handlers.rs`
`canon_facts()` (joined into `canon_export` as `canonical.facts`); the
`compile_facts` re-point in `scripts/compile_lore_dli.py`.

**Verification.** `canon_export{facts}` byte-identifies to committed
`parity.json` (`json.dumps(..., sort_keys=True)` equality): `by_pred` relation
atoms match, constraint `pattern` strings match verbatim (no regex escape),
`interner`/`decisions` match. `compile_lore_dli.py --check` green in both canon
(`DELOREIS_BIN` set) and offline lanes; `git diff worlds/tpth/generated/` empty.
Gate 21/21 (`GATE_EXIT=0`).

**Revisit trigger.** Re-evaluated if a constraint `value` JSON stops being a
verbatim constraint row, or if `compile_facts` re-renders `constraint-pattern`
from a non-raw-regex source.

## DL-0067 — The chronology before-edges reproduce the committed timeline from the folded store

*Date: 2026-08-09*

**Attribution:** machine — the LS-7 chronology projection (Path C), the
store-sourced sibling of DL-0065 (killed-tokens), DL-0066 (facts), and DL-0068
(the general by_pred/YAML cutover rule). The projection code committed at
`7e6aa56` (`canon_timeline()`) was authored without its own ledger entry; DL-0067
retroactively records the ruling that the commit already encodes. Live and
binding, pending async review (DL-0052).

**Ruling.** `dli.corpus.export{format:"canon"}` projects the `before` predicate
bucket as `canonical.timeline`, in four maps that byte-identify to the committed
`parity.json` `timeline.dli` (flat, 4 edges):

- `before_edges` (4): every `(earlier, later)` atom from the folded `before`
  `by_pred` bucket, in commit order — `{before, after, decision, era, source,
  status}` rows. `decision` is singular (per row, the interner-expanded `support`
  monomial; all four `+7` → `interner[7]` → D-0072, the ratified batch-003 authoring
  decision).
- `era_provenance` (`{status:"ratified", decisions:[D-0040,D-0060,D-0062,D-0070,D-0072],
  source:"event-citation-union"}`): the ratified era-decision union.
- `events` (9) / `eras` (9): from the event/era atoms (rendered as era/offset lists
  in `timeline.dli`, the `era` and `source` reassembled from the YAML
  `timeline.yaml`'s era/offset grouping — the store carries no `era`/`source` in
  the atom).

The `line` rendering `before breaking-of-the-stones -> tithe-of-minds-sweeps
provisional D-0072` is reproduced verbatim; chronologically-inconsistent pairs
reverse the edge order and are rejected by the `fact.contradiction` guardrail, so
a reverse `before` plant fails (use chronologically-consistent pairs).

**Affected code.** `crates/dli-serve/src/handlers.rs`
`canon_timeline()` (joined into `canon_export` as `canonical.timeline`); the
`compile_timeline` re-point in `scripts/compile_lore_dli.py`.

**Verification.** `canon_export{timeline}` byte-identifies to committed
`timeline.dli`; `compile_lore_dli.py --check` green in both canon and offline
lanes; `git diff worlds/tpth/generated/` empty. Gate 21/21 (`GATE_EXIT=0`).
Independent fresh-context red-team (reconstruct the 4 `before` edges from raw
objects + plant a non-chronological `before` edge cited→`Decided`) confirms the
projection detects a store mutation → RED → restore → GREEN (4 edges).

**Revisit trigger.** Re-evaluated if the `before` fact atom gains an `era` or
`source` field, or if `compile_timeline` no longer reassembles `era`/`source`
from the YAML era/offset grouping.

## DL-0068 — LS-7 by_pred / YAML cutover: canon carries the structural by_pred
fields; ratified name surfaces stay YAML-sourced

*Date: 2026-08-09*

**Attribution:** machine — the LS-7 lorestore cutover split, decided across the
Path B (relations/constraints), Path C (before-edges), and Path E
(person/role/alias) canon projections. DL-0065 opens the killed-tokens domain;
DL-0068 settles the general rule governing every canon domain: which `by_pred`
fields project into `dli.corpus.export{format:"canon"}` and which human-ratified
display surfaces stay on the YAML fallback. Live and binding on the tree,
pending the author's async review (DL-0052).

**Ruling.** On the LS-7 cutover, the canon export projects the **structural
`by_pred` fields** that are fully `by_pred`/commit-order-sufficient — each row
carrying `decisions` (the `support` monomials expanded through the interner) and
`status` (`provenance.marker` lowercased):

- relations (`adjacent_to`, `located_in`, …), constraints, chronology `before`
  edges, `killed`-registry atoms, and the character sheet's `person`, `role`,
  and `alias` predicates (`canon_facts`, `canon_timeline`, `canon_characters`).

Non-canonical markers are refused fail-closed at projection time: the MCP
`author.*` tools emit `Decided`/`Proposed` markers (not the `.dli`-authored
`Provisional`/`Ratified`), so a planted `Decided` row makes the export signal
`ok=false` rather than silently rendering a non-canonical surface.

The **human-ratified display `name` surfaces** stay YAML-sourced — not
canon-projected:

- `entities.name` (the `person <id> "<display name>"` surface, eg. `Enfys Livia`)
  — `characters.yaml` supplies it; the store carries only the slug `name`
  by_pred atoms (3, ratified D-0132/D-0134), not the display surfaces.
- the guardrail `factcheck-alias` surfaces (the cue-checked alias surfaces eg.
  `the Anchor`) — rendered by `compile_facts` from `cue_source`
  (guardrail-facts.yaml), NOT from `canon.facts`. The `factcheck-alias` re-point
  (Path D) was evaluated and **rejected**: the canonical `factcheck-alias` by_pred
  (11 rows) is byte-identical to the committed `facts.dli` factcheck-alias lines,
  so re-rooting changes nothing but reverses the ratified-surface YAML convention
  for no byte-level gain.

`compile_lore_dli.py`'s per-domain normalizers consume the canon projection when
`DELOREIS_BIN` serves the lane; the matching YAML projection is the offline
fallback so the offline CI gate (no binary) still reproduces `parity.json`
byte-for-byte. Both emit identical output — `canon == parity`.

**Affected code.** `crates/dli-serve/src/handlers.rs`
`canon_characters()` (joined into `canon_export` as `canonical.characters`) and the
`compile_characters` re-point in `scripts/compile_lore_dli.py` (B-E Path E).

**Verification.** `canon_export{characters}` over `/tmp/s5b` projects `person`: 5,
`role`: 15, `alias`: 12 — byte-identical to the six core characters' committed
`characters.dli` (5 `person` declarations + 6 `role` + 12 `alias`; `hound` is a
declared entity and so has no `person` atom). `compile_lore_dli.py --check` is
green in BOTH canon (`DELOREIS_BIN` set) and offline lanes (exit 0); `git diff
worlds/tpth/generated/` is empty. `cargo xtask gate --ci` → `PASSED — 21 steps
green`, `GATE_EXIT=0`. Independent fresh-context red-team (reconstruct
`canon.characters` from raw objects + plant a bogus `protagonist alias`
cited→`Decided`) confirms fail-closed: export `ok=false`
(*"unknown person/role/alias provenance marker: Decided"*) → RED → restore →
GREEN (5/15/12).

**Revisit trigger.** Re-evaluated if display `name` surfaces are ever stored as
`by_pred` `name` atoms carrying the canonical display surface (currently the
store carries only slug `name` atoms, D-0132/D-0134), or if `compile_characters`
selects a different character set than the YAML `characters.yaml` registry.

## DL-0069 — Canon export is a contract 1.4.0 wire addition

*Date: 2026-08-10*

**Attribution:** machine — the contract versioning repair for the LS-7 canon
export. The implementation adds `CorpusExportFormat::Canon` and the optional
`canon` response field, so it must not ship as a second meaning of `1.3.0`.

**Ruling.** `dli.corpus.export{format: "canon"}` is introduced at contract
`1.4.0`. The server advertises and accepts `1.4.0` alongside every previously
accepted version. A request negotiated at `1.3.0` or below that names `canon`
receives the released decoder's schema refusal, byte-identically; all older
wire behavior remains frozen.

**Rationale.** A same-version response-shape change would make the contract
tag ambiguous and would let a pinned fantasy consumer receive different wire
surfaces from different commits. A minimal version bump keeps the canon
projection additive while preserving the multi-version dual-mode law.

**Verification.** The contract schemas, version echo, capability replay, and
the pre-1.4.0 canon-refusal fixture are checked by the gate; the refusal is
replayed against the `contract/1.3.0` build and the `1.4.0` build.

## DL-0070 — Contract 1.5 unifies standing and adds ordinary ruling data

*Date: 2026-08-11*

**Attribution:** machine — the contract 1.5 standing migration and authoring
surface. Human supervision remains asynchronous; these operations do not
pretend to be a human approval workflow.

**Ruling.** The stored standing vocabulary is `asserted` (no marker),
`proposed`, `provisional`, or `ratified`. The legacy `decided` marker remains
deserializable for one compatibility cycle, is registered as a killed form
whose successor is `provisional`, and is refused at the contract 1.5 import
boundary. Schema v4 upcasts old stored `decided` metadata to `provisional`.
Marker metadata remains outside the fact identity projection, so this upcast
does not change content addresses.

At contract `1.5.0`, cited authoring provenance carries a required
`state: "provisional" | "ratified"`; below `1.5.0`, the prior wire remains
byte-compatible and a cited call without state keeps its previous behavior.
`author.set_standing` promotes an existing provisional claim when its cited
decision is declared and occurs in every support path. `author.defeat` records
the new defeating decision and sets the claim's old supporting decision false
on the serving ref; defeated objects remain reachable in the append-only store.
Both transitions retain their decision evidence in provenance metadata without
changing the claim identity. They are ordinary data operations and do not block
on a human-signoff gate. Canon excludes claims whose support evaluates dead,
derives surviving status from the closed vocabulary, requires the commit's
decision list to equal the stored `Ratify` carriage set, and fails closed on
garbage markers or inconsistent decision evidence.

**Rationale.** A single status vocabulary prevents the retired entry door from
being mistaken for a standing value, while v4 metadata upcast preserves old
stores and stable identity hashes. Explicit caller-supplied state records the
data the consuming application needs; promotion and defeat belong to the
author interface without assigning the engine authority over who may rule.

**Verification.** Contract schemas, version gates, killed-form handling,
marker upcast, identity-hash stability, canonical status projection, durable
promotion, defeat valuation overlay, dead-claim exclusion, and decision-carriage
corruption are covered by the Contract 1.5 test lane.

## DL-0071 — A `state` becomes an evaluable fluent fact, and interval-overlap contradiction goes live (issues 47, 48)

*Date: 2026-08-11*

**Attribution:** machine — an engineering decision by the implementing agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** A declared `state` — the surface `state s.p = v [time]` and the
`author.add_state` wire op — was accepted, provenance-scrubbed, persisted, and
round-tripped, yet lowered to *nothing*: both the loader and the evaluator routed
`Decl::State` to a catch-all and the durable store emitted no fact object. That is
an inert declaration (koan 4, DL-0029) and a provenance gap (the store carried a
ruling for a claim no query could surface). It is closed:

1. **One lowering seam, three paths.** `StateDecl::atom` (in `dli-ast`) maps a
   state to its ground atom — dotted `s.p` with value `v` → `p(s, v)` (the same
   atom `s.p = v` would produce, so a state is queryable), dotted no-value →
   `p(s)`, undotted with value → `n(v)`, undotted no-value → none. The loader
   (`process_state`), the evaluator (`World::lower`), and the durable store
   (`lower_decl`) all lower through it, so their ground atoms cannot drift; the
   two-lowering convergence oracle and the byte-identity oracle both hold.

2. **The interval rides the fluent, not the atom.** A valued binary state also
   pushes a `Span` onto the slot's `FluentTimeline` on the compiled world, keyed
   by `(pred, subject)`, carrying its `since`/`until` anchors and the
   declaration's ruling. The timeless atom stays queryable; the interval does not
   collapse into the attribute layer. A state's temporal anchor is checked like an
   entity's (`since`/`until` name events, `after`/`during` eras), so an undeclared
   anchor surfaces `load.unknown-temporal-reference`.

3. **Interval-overlap contradiction is now a live path.** `dli-time`'s fluent
   machinery — built and tested but referenced nowhere — is wired into the check
   battery. `continuity::run` raises exactly one `continuity.fluent-contradiction`
   for a slot whose timeline holds two distinct values *certainly at once over
   overlapping spans*; a value in force over disjoint spans (`commons until e` /
   `enclosed since e`) is one truthful fluent and raises nothing. The
   claim-relative `intra-episode-contradiction` skips fluent slots (the flat
   single-valued reader would false-fire on disjoint spans), and the fact-checker
   never returns `contradicted` on a fluent slot — the two false positives that
   realizing states as flat facts would otherwise have introduced.

4. **Nothing silently dropped.** A residual state that names nothing evaluable
   (undotted, valueless) surfaces `load.inert-declaration` rather than vanishing.

**Rationale.** DL-0029 already ruled that accepted syntax may not lower to
nothing; a whole declaration kind that was accepted-and-vanished violated it. The
fix realizes the state as the queryable fact issue 47 asks for while threading the
interval through the fluent layer issue 48 asks for — and the two are one change,
because emitting the flat fact without the fluent routing would itself have
introduced the false disjoint-span contradiction issue 48 warns of. Contract
behavior is unchanged: `author.add_state` still stages a `Decl::State`; it now
lowers to something the world can answer.

**Verification.** `dli-world` loader tests (a state is a queryable fact; missing
provenance fails closed; an inert state is a finding), the check lane (disjoint
spans raise zero findings, overlapping spans raise exactly one, a disjoint fluent
value is never a fact-check contradiction, the existing single-valued and
multi-valued intra-episode cases still hold), the two-lowering convergence oracle,
the `dli-eval` state smoke test, the `dli-lorestore` total-lowering byte-identity
oracle, and an end-to-end `author.add_state` → `author.query` serve test.
## DL-0072 — Active teaching surfaces name citation integrity, not human sign-off

*Date: 2026-08-11*

**Attribution:** machine — the documentation scrub and retired-vocabulary lint
implement the authority model already ruled by DL-0054 and DL-0058.

**Ruling.** At contract 1.3.0 and above, a caller-supplied citation is recorded
when its consuming-ledger decision resolves; an unknown decision is a finding
that bars materialization. Authorization and review belong to the consuming
application. Accepted wires below 1.3.0 retain the byte-exact cited-provenance
refusal. Active specifications, cards, and the authoring skill must teach this
versioned split and must not describe the retired human-only or sign-off law.

**Affected surfaces.** `docs/spec/11-agent-authoring.md`, the cards, `SKILL.md`,
the author renderer comments, and the `xtask retired-vocabulary` gate.

**Verification.** The retired-vocabulary lint fails on each retired phrase in an
active teaching surface, permits the append-only historical record and the
spec/11 compatibility row, and passes after the scrub; docs-oracle, authority,
and the full gate remain the required follow-up checks.

## DL-0073 — Contradiction is judged on a normalized value key, and canon is checked against itself (issue 49)

*Date: 2026-08-11*

**Attribution:** machine — an engineering decision by the implementing agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** The functional-dependency detector compared values by **exact string**,
ran **one-sided** (claim-against-canon only), and skipped a whole staged value
form. Each of those is a way for a real contradiction to pass unremarked, which is
koan 4 (*nothing is silently dropped*) read on the check layer: a detector that
cannot see a clash drops it as surely as a parser that discards a line. Three of
issue 49's four sub-claims are closed here; the fourth was already closed by
DL-0071.

1. **One normalized key, wherever two values are compared.** The fold the
   killed-token guardrail already used — lowercase, hyphens and underscores to
   spaces, runs of whitespace collapsed — is promoted to `dli_world::check::normalize`
   and is now the key *every* FD comparison comes down to: the fact-checker's
   certain-value match and its `certain.len() == 1` single-valuedness test, the
   claim-relative `intra-episode-contradiction`, the data-driven
   functional-predicate inference, and the interactive staging path. `gold` and
   `Gold` are one value rather than a phantom two — and the phantom two was worse
   than a cosmetic wart, because a second value silently switched the
   single-valued detector *off* and took the real contradiction with it. The fold
   is cosmetic only: case and separators, never stems. `gold` and `golden` stay
   two values, because deciding that they are one is a ruling about the lore and
   the engine does not make those (koan 3). The raw spelling is kept beside the
   key so a finding still quotes what the author wrote.

2. **`continuity.fd-violation` — canon is checked against itself.** The
   contradiction battery only ever asked *does this claim clash with canon*, so a
   functional-dependency break sitting inside canon's own certain layer was
   invisible: a slot the store holds two ways was never checked against itself,
   only against an incoming claim. `check::run` now raises
   `continuity.fd-violation` for a slot whose own certain projection holds two or
   more distinct values on a predicate the author **declared** `functional`
   (DL-0074) — the read-side counterpart to the merge engine's `certain_conflicts`,
   closing the asymmetry that let a load accept what a join refuses. The class
   fires **only** on a declared predicate. Functionality is not inferred from
   canon's shape: a subject bearing several edges while others bear one each is a
   hub in a many-to-many relation (a place with several neighbours, R2/B) exactly
   as readily as a functional-dependency break, so guessing manufactures findings
   against lore that is merely unusual — and did, reddening two bundled corpora on
   the first cut. The author rules on functionality (koan 3); the engine never
   guesses. Fluent slots are skipped: their clashes are interval-sensitive and
   belong to `continuity.fluent-contradiction` (DL-0071), where a value held over
   disjoint spans is correctly no clash at all.

3. **The staged world is checked like a loaded one.** Because materialize gates on
   `check::run` over the *staged* world, the new world-internal class makes a
   conflict between two **sibling staged** claims block the accept decision, where
   before only a staged-versus-committed clash could — the staging path inherits
   the symmetry rather than growing a second rule to keep in step. Alongside it,
   the staging pre-check (`attr_findings`) now compares any ground value: a
   `Term::Str` slot value was previously not compared at all, so a quoted
   contradiction walked past the one check that was supposed to catch it.

4. **Timelessness was not the remaining defect.** Issue 49's "timeless" sub-claim
   is closed by DL-0071 (issues 47, 48): a time-varying property is a `state`,
   which lowers to a queryable fact *and* pushes its interval onto the slot's
   `FluentTimeline`, where overlap is judged with the intervals in hand. A flat
   fact or slot is timeless by construction, so the FD check is right to compare
   values without reasoning about time — and right to leave fluent slots to the
   check that owns their spans.

**Rationale.** All three fixes are one defect wearing three costumes: the detector
answered a narrower question than the one it was trusted to answer, and answered it
silently. Keying comparison on the FD value key stops cosmetic drift from reading as
distinctness — but that key folds only case, hyphen/underscore, and whitespace,
never other punctuation, so `2.5` and `2-5` stay two values: a fold that merged
genuinely distinct values would itself silently drop a contradiction (koan 4), the
very failure the issue names. Checking canon against itself removes the asymmetry
that let the store hold what a merge would refuse; comparing every ground value
removes the type-shaped hole. And functionality is the author's ruling, not the
engine's guess (koan 3, DL-0074): where the data cannot settle whether a relation
is single- or multi-valued, a declaration does — so the world-internal class fires
on a declaration, never on canon's shape.

**Verification.** The `dli-world` check lane covers the FD value key (`gold` and
`Gold` are one value; `gold`/`golden` and `2.5`/`2-5` remain two), the new class
(a declared `functional` predicate with a two-valued subject raises exactly one
finding; an undeclared predicate — including the hub-and-chain topology that reds
`camelot`/`saltmarket` if inferred — raises none), and the fluent skip (a value
held over disjoint spans is still no FD finding). The `dli-serve` staging tests
cover the sibling-staged conflict blocking materialize, the `Term::Str`
contradiction, and the new op's citation and control-character guards. The
finding-family catalogue is regenerated and coverage-checked against the emitting
source, so the code is registered rather than merely raised.

## DL-0074 — `functional <pred>` is a declaration, and `author.declare_functional` is a contract 1.6.0 wire addition (issue 49)

*Date: 2026-08-11*

**Attribution:** machine — an engineering decision by the implementing agents. It is
live and binding on the tree, pending the author's async review (DL-0052).

**Ruling.** Single-valuedness becomes something an author can **say**. The surface
gains one keyword — `functional <pred>` — and the tool surface gains one operation,
`author.declare_functional`, taking the closed operation set to 60. The declaration
marks a predicate a functional dependency: each subject holds at most one certain
value for it, so two distinct certain values are `continuity.fd-violation`
(DL-0073). It is introduced at contract **`1.6.0`**, a minor bump on the
additive-capability discipline DL-0069 (canon export at `1.4.0`) and DL-0070 (the
standing tools at `1.5.0`) already set: the op goes on the *end* of the
append-only operation list and is advertised only from `1.6.0` up, so every
earlier negotiation's capability list — and the released golden fixture recording
it — stays byte-identical and needs no re-pin (audit C4). `0.5.1`, `0.6.0`–`0.6.4`,
and `1.0.0`–`1.6.0` all remain accepted, each caller answered in the version it
declared.

The declaration is **carriage-only**. It lowers to no ground atom — there is no
fact "`ruler-of` is functional" to query — and it interns no provenance: the
loader records the predicate in the world model and never calls `compile_ann`, so
the durable lowering must not offer the annotation either, on pain of minting a
bundle variable the full compile never mints and desyncing every index after it
(the lock-step hazard DL-0071 named for `state`). Its provenance rides the
serialized carriage body instead, as `ratify`'s does. Because it lowers to no
atom it is not an inert declaration in the DL-0029 sense: it is consumed — by the
checker, which is the only reader a constraint needs.

**Rationale.** Without a declaration, functionality could only be *inferred* from
canon precedent, and inference is structurally blind exactly where an author most
needs the check: the first time a predicate is used. A first-seen predicate given
two values is either a slip or a genuinely multi-valued relation, and no amount of
data settles which — that is a ruling about the world, and the engine does not
make rulings (koan 3). So the author makes it, once, in one line, and the checker
enforces it from then on. A wire op rather than a text-only form, because agents
author through the one interface (DL-0014, DL-0043); a version bump rather than a
second meaning of `1.5.0`, because a pinned consumer must not receive different
surfaces from different commits.

**Verification.** The contract fixtures for `author.declare_functional` (framing,
golden, and the negative schema violation) are registered in the closed fixture
registry at `contract_version: 1.6.0`; the op inventory, capability replay, and
version echo are gate-checked, with the pre-`1.6.0` capability lists asserted
unmoved. The parser, formatter, and the three lowerings are covered by the
round-trip and byte-identity oracles, and the checker's use of the declared set is
covered by the `dli-world` check lane (DL-0073).

---

## DL-0075 — One functional-dependency primitive across the load, fork, and merge checks

*Date: 2026-08-12*

*Attribution: machine*

**Ruling.** The functional-dependency check — "does this certain layer set a
single-valued slot two ways?" — is asked in three places, and they are unified
behind one primitive so they cannot answer it differently. The primitive is
`dli_core::fd`: `fd_value_key` (the value-distinctness key), `fd_violations` (the
grouping that turns a stream of certain `(pred, subject, value)` slots plus a
functional-predicate set into violations), and `inferred_functional` (the
data-driven shape inference). The three call sites now render their own finding
or `Conflict` but share this logic:

- the load/read check, `dli_world::check::continuity`'s `fd_violations` and
  `predicate_is_functional`, over a compiled world's **authored source** facts;
- the in-memory fork merge, `dli_eval::fork`'s `certain_conflicts`, over the
  **joined saturated** relation of two forks;
- the durable-store merge, `dli_lorestore::merge`'s `certain_layer_conflicts`,
  over the **recompiled merged** world.

Three consequences settle the axes issue 59 named:

1. **Distinctness keys on `fd_value_key` everywhere.** Cosmetic drift
   (`gold`/`Gold`, `tide-bell`/`tide bell`) is one value on a fork merge and a
   store merge exactly as it already was on load — never a phantom conflict. The
   fork and merge paths formerly compared raw strings. The key folds a
   hyphen/underscore/whitespace run to a single space **only between two other
   characters**, and keeps a leading or trailing `-`/`_`: a separator's
   word-joining meaning holds only *between* words, so `-5` and `5`, `_gold` and
   `gold`, `5-` and `5` stay distinct. Folding them would silently drop a real
   contradiction (koan 4) — the one thing the key must never do — and unifying
   onto it made that latent edge-fold reach two paths that had compared raw
   strings, so the fold was tightened as part of this change (issue 59 red-team).

2. **The declared-functional set reaches the fork engine.** `functional_predicates`
   is lifted onto the evaluable `dli_eval::World` (a read-only set carrying no
   atom and shifting no interner), populated by `World::lower` and, on the
   compiled path, seeded by the `dli-world` loader — the fork base is
   `WorldModel::world`, so this is what carries a declaration into the in-memory
   merge. This **amends DL-0074's** "the loader only populates the world-model
   set": it now seeds the evaluable world's set too, and `functional` is no longer
   a total no-op on the eval core — it records the constraint (still no atom, no
   proposal index, no interned name, no finding). The functional-set *policy* is
   unchanged from DL-0074: the world-internal load check over
   canon-against-itself fires on **declared-only** (canon's shape cannot tell a
   first-seen slip from a genuine many-to-many hub, koan 3), while the
   delta-vs-base checks use `declared ∪ inferred-from-base`, because a merge is a
   delta against a base and inference is sound only as that base. Inference is the
   shared `inferred_functional`, so it too keys on `fd_value_key` — the fork/merge
   inference now folds cosmetic drift exactly as the load side's
   `predicate_is_functional` already did (a deliberate convergence, not a new
   behavior of its own).

3. **Where a declared break is reported differs by path, deliberately.** The load
   check reads **authored source** atoms; the fork and store merges read the
   **saturated derived** layer — what a merge derives once re-evaluated. That
   split is intended (source is what canon *asserts*; derived is what a merge
   *builds*), but the derived, flattened layer is coarser — it has erased fluent
   intervals and subject term kinds. So the two merge paths place the
   declared/inferred halves to keep each break reported by the lens that shares
   the load check's exclusions:
   - **Durable store merge.** `certain_layer_conflicts` is **inferred-only**. The
     recompile's `dli_world::check::run` battery already reports an
     author-**declared** break as `continuity.fd-violation` over the merged
     world's *source* layer — fluent-aware and symbol-subject-aware, exactly as on
     load. Re-adding declared to `certain_layer_conflicts` (an earlier cut of this
     change) double-reported it through the coarse derived path and, lacking those
     exclusions, raised a conflict for a declared *fluent* that loads clean and
     re-blamed a break the base already held. So declared stays with `check::run`;
     the two halves compose to `declared ∪ inferred` across the merge report
     without either diverging from the load.
   - **In-memory fork merge.** `certain_conflicts` has no findings channel, so it
     must carry `declared ∪ inferred` itself, and it is a **delta** check: a break
     the base already holds is the base's defect (the load check reports it), not
     the join's conflict, and is subtracted — which keeps two clean forks joinable
     over broken canon and neutralizes a flattened fluent whose two interval-values
     both sit in the base. Its residual coarseness is accepted and documented: a
     fork that *adds* a second interval-value to a declared-functional fluent, or a
     declared predicate whose subject is a non-symbol literal, can over-report on
     this what-if tool where the load check and the durable merge — which see
     intervals and term kinds — do not. Representative value spellings in a message
     are per-lens (each keeps the first raw spelling it meets in its own atom
     order); the key they are judged by is identical.

**Accepted consequences** (weighed against the alternatives; each documented here
rather than engineered away):

- **Inference keys on `fd_value_key`, like detection.** Because the shared
  `inferred_functional` folds cosmetic drift, a many-to-many predicate whose *sole*
  multi-valued subject is multi-valued only cosmetically (`tag(book, "Sea-Wolf")`
  and `tag(book, "sea wolf")`) is inferred functional, and the next genuine edge on
  that subject reads as a break the fork/merge report. This is narrow, and it is
  the coherent choice: raw-key inference paired with folded-key detection would be
  incoherent (it would call a slot functional and then refuse to fold its values,
  or the reverse). It matches the claim-relative `predicate_is_functional`, which
  already keyed this way. It is *not* convergence with the world-internal load check
  (`continuity.fd-violation`), which is declared-only and raises nothing for an
  undeclared predicate.
- **A declared break is raised in different *kinds* on the two merges.** On the
  fork merge it is a `Certain` `Conflict` (its only channel); on the durable merge
  it is an error-severity `continuity.fd-violation` **finding** from `check::run`,
  and `certain_layer_conflicts` stays silent on it. Both raise it, and the durable
  merge report carries both a conflict and a finding array, so a consumer reads the
  error finding as the blocking signal; the durable fact-set union is written
  regardless (the report is advisory the sovereign rules on). Making the durable
  path *also* emit a `Certain` conflict for declared breaks is exactly the earlier
  cut that reintroduced the coarse-path F2/F3/F5 divergences, so the finding — from
  the fluent- and symbol-aware lens — is the right home. This is the one residual
  "same question, phrased differently" between the two merges, and it is deliberate.

A separate, **pre-existing** inconsistency is noted for its own issue: a canon with
non-symbol subjects (`crest(5, …)`, `crest("mara", …)`) raises no
`continuity.fd-violation` on a direct `dli_world::compile` but two after a lorestore
round-trip, because `reconstruct_pack` rebuilds integer and quoted-string subjects
as bare symbols that `slot_of` then accepts. It reproduces on `main`, predates this
change, and is in tension with the LS-3 differential-parity promise; it is not
fixed here.

**Rationale.** The store's value proposition is that its certain projection is
internally consistent and a delta breaking a functional dependency is caught,
cited, and refused *the same way wherever it is checked*. Three hand-written
copies broke that promise: a cosmetic pair was one value on load but a conflict on
merge, and a declared dependency was honored on load but invisible to a fork
merge / ratification pre-flight. Issue 59's review also found the claim "the
lorestore merge already runs `check::run`, so it is consistent" was only half
true — merge runs `check::run` *and* its own raw-string `certain_layer_conflicts`.
One primitive both sides call is the only structural guarantee they cannot drift
again — but "one primitive" is the value-key and the grouping; the atom the check
reads and where declared functionality is reported remain per-path choices, made
above so no path answers the FD question differently from the load, within the
limits of what the flattened eval layer can represent.

**Scope.** This is an internal engine-consistency change: **no contract change**.
There is no new wire operation and no version bump — the `functional` declaration
and `author.declare_functional` stay at contract `1.6.0` (DL-0074) — so no golden
fixture, schema, or capability list is re-pinned. `dli_eval::World`'s new field is
`#[serde(default, skip_serializing_if)]` and empty for worlds that declare no
functional predicate, so no stored world serializes differently; the eval `World`
is not content-addressed.

**Verification.** The primitive carries unit tests in `dli-core`: cosmetic fold is
one value, genuinely-distinct values (including a leading/trailing `-`/`_`) never
merge, and inference excludes a many-to-many relation. The `dli-eval` fork lane
pins the merge folding cosmetic drift, honoring a declared first-seen dependency,
folding cosmetic drift *under* a declaration, and not re-blaming a base-inherited
break while still catching a merge-introduced one. The `dli-lorestore` merge lane
pins the durable path: cosmetic drift is one value, and a declared first-seen break
is raised on the merge as a `continuity.fd-violation` finding (the declared-owning
load lens), not a coarse derived-layer conflict. The revised `functional_decl_honesty`
differential in `dli-eval` pins that the declaration still touches nothing but the
functional set, and the unchanged `dli-world` FD lane confirms the load-side
refactor preserved its behavior exactly. The whole change is gate-green.
