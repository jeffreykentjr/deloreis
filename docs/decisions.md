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
