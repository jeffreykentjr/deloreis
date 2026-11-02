---
status: live
verdict: "The decision ledger: current rulings and the discipline for new entries."
---

# Decision ledger

This ledger outranks the other project documents. Append new decisions with the
next sequential `DL` number, a date, an attribution (`machine` or `human`), and
a ruling, a short rationale, and a revisit trigger when applicable. Corrections are new entries. The reset below is the one authorized
exception to the earlier append-only history; subsequent entries are append-only.

## DL-0104 — Standing rulings carried forward from `pre-fiction-reset`

*Date: 2026-11-02*

**Attribution:** machine.

**Ruling.** DL-0101 authorizes replacing the old ledger with this readable
continuation. The still-applicable engine and project obligations of the
rulings listed below remain binding, including their qualifications and
exceptions, except where a later ruling supersedes them. These summaries are
navigation, not permission to discard a surviving detail. The earlier text is
available with `git show pre-fiction-reset:docs/decisions.md`; DL-0102 and
DL-0103 were recorded later and are available with
`git show 8c73720:docs/decisions.md`. Retired consumer-specific work, obsolete
contract-version promises, the abandoned evaluation program, and the retired
cutover are not carried forward.

- **DL-0001:** The project is De Lore Is; text uses `.dli`, the executable is `deloreis`, and worker operations use `dli.*`.
- **DL-0002:** The charter is constitutional; resolve koan conflicts by truth, loss, form, then growth.
- **DL-0003, as amended by DL-0041 and DL-0101:** Schemas derive from Rust types; fact objects and their canonical JCS bytes are authoritative, with supported text as a rendering and authoring surface.
- **DL-0005:** Canon-certifying judgments are owned, deterministic, offline, and replayable; foreign tools do not silently certify canon.
- **DL-0006:** Grades are `checked > tested > attested > asserted`; support and grade remain distinct, and chains roll up at their weakest grade.
- **DL-0008:** Keep the hand-built parser, typed schemas, domain diagnostics, and the project's chosen small tooling stack.
- **DL-0009:** Keep phases pure and invocation-local; do not make a stateful compiler a prerequisite for incremental evaluation.
- **DL-0011:** The ledger outranks documents; docs and the skill have `status` and `verdict`, IDs are minted in the ledger, and the documentation tree stays flat.
- **DL-0013:** Gating lanes are deterministic and offline; external or nondeterministic experiments do not block the language gate.
- **DL-0014, as amended by DL-0043 and DL-0054:** Agents author lore using the server's semantic authoring tools; authority and review policy belong to the caller.
- **DL-0015:** `cargo xtask gate` is the canonical compiled CI-parity gate.
- **DL-0016:** Surface names describe their behavior; retired forms are registered and refused with their successor.
- **DL-0019:** The cyclic timing proof finding is `proof.wrong-phase`.
- **DL-0020 and DL-0024:** World rules support typed bodies and stratified `not`; negation carries valuation-neutral, theory-respecting defeat rather than treating an open counterclaim as absent.
- **DL-0023:** Authoring reads inspect the staged world without mutating it and report structured findings.
- **DL-0025:** The canonical formatter is comment-free; teaching commentary lives outside canonical text.
- **DL-0029, as qualified by DL-0079:** Accepted syntax has semantics or an honest diagnostic; explicitly ruled inert forms retain their recorded dispositions.
- **DL-0030:** Advertise a capability only when its documented runtime and durability semantics are implemented.
- **DL-0037, as amended by DL-0101:** Keep the generic context, articy, CSV, and static-wiki exports, with validated profiles and containment of hostile text.
- **DL-0039:** Proof selection is explicit when ambiguous, and quoted claims retain their intended term form.
- **DL-0041:** The content-addressed lorestore is canonical; deterministic reconstruction, schemas, offline checks, and checked documentation examples remain part of the contract.
- **DL-0042:** The old line-count budget and its gate are retired; the remaining architectural constraints still apply.
- **DL-0043, as amended by DL-0054 and DL-0101:** Semantic tools provide the common agent interface; text is supported, and the engine does not decide caller authorization.
- **DL-0044 and DL-0045:** Stage declarations as fact objects per mutation; materialize is acceptance, reconstruction is total or refuses, and workspace and commit digests remain distinct.
- **DL-0046:** Structured objects and graded findings are the primary mutation result; optional text rendering, fencing, and deterministic replay remain available.
- **DL-0047 and DL-0049:** Stored declaration interiors are typed; old schemas upcast at the boundary without changing identity, and the kernel need not parse stored data.
- **DL-0048:** Branches and two-parent commits merge via a common base, union, and recompile; valuation, constraint, and negation conflicts remain explicit and deterministic.
- **DL-0052:** Humans and machines may mint attributed ledger rulings; supervision is asynchronous, with correction by a later ruling.
- **DL-0054:** Attribution and decision state are caller-supplied data; citations must resolve, and unknown decisions fail closed.
- **DL-0055:** Store-backed projects replay the selected head or follow a branch as configured; storage schema changes and wire claims stay coupled and checked.
- **DL-0056:** Corpus exchange preserves interner names and decision data; unsupported rendering is refused explicitly, never returned as an empty success.
- **DL-0058:** Supported expert authoring declarations use the same staged, checked, durable path and record citations as data.
- **DL-0059:** Preserve the original pack scope through commits; an empty corpus has no facts, and imported claim IDs are checked against their contents.
- **DL-0061:** Escape caller-supplied controls in finding messages at the serialization boundary so content cannot forge a second finding.
- **DL-0064:** The `killed` registry predicate records prohibited tokens as data and is excluded from killed-token usage scans.
- **DL-0065, DL-0066, DL-0067, DL-0068, and DL-0069, as amended by DL-0101:** Canon exports derive their retained domains and provenance from the folded store; private-consumer parity targets do not bind the fiction release.
- **DL-0070:** Standing is `asserted`, `proposed`, `provisional`, or `ratified`; legacy stored values upcast honestly and current citation state is explicit.
- **DL-0071:** Valued states lower to evaluable facts and temporal fluent spans; continuity checks use their intervals.
- **DL-0072:** Teaching material describes citation integrity and caller-owned authorization, not a required human sign-off.
- **DL-0073:** Functional contradictions compare normalized values, inspect canon against itself, and retain explicit blame.
- **DL-0074 and DL-0075:** Functional dependencies are declared, persisted, and checked by the same primitive at load, fork, and merge boundaries.
- **DL-0078:** Entity identity is explicit, defeasible, symmetric, and transitive; era/event identity is refused and ordinary facts do not silently activate identity.
- **DL-0079, as amended by DL-0101:** `use`, `draft`, and `export` declarations are intentionally inert; narration and variants retain their recorded limits; text removal is cancelled.
- **DL-0080:** Typed identity and rule authoring preserve provenance, temporal reference findings, and structured finding data.
- **DL-0081:** Inertial persistence is a valuation-lazy projection; only a certain declaration and later distinct certain value cap an open span, and defeating that support can restore it.
- **DL-0082:** `author.as_of` uses the continuity checker’s canonical slot projection, reports uncertainty and supporting boundaries, and does not fall back to unrelated raw facts.
- **DL-0083:** Recording decisions is durable; decision-stamped materialization and why explanations retain their recorded scope and provenance.
- **DL-0084 and DL-0091:** Predicate and entity-candidate reads are bounded and deterministic; candidates come from declared entities and use the specified exact ranking.
- **DL-0085:** `author.why` accepts a typed goal; decision-stamped materialization applies to facts, not arbitrary carriage declarations.
- **DL-0086:** Merge-base work is linear in the reachable commit graph, and dry-run merge leaves refs unchanged.
- **DL-0087, as amended by DL-0102:** Retain permanent, non-vacuous incremental-evaluator differential and confluence coverage for negation, defeater-only growth, identity, and strata, with full recompilation fallback; retire the unused loader-reuse compiler and its workstream.
- **DL-0089 and DL-0102:** Retain the bounded compiled-world cache and incremental evaluator saturation; retire the unused loader-reuse compiler.
- **DL-0092:** Predicate identity is an explicit, defeasible, symmetric-transitive claim, never inferred from spelling.
- **DL-0093:** Resolution indexes and the live lowering interner are derived workspace state, with results equivalent to a fresh rebuild.
- **DL-0094 and DL-0095:** Persisted valuations cross independently compiled worlds by exact variable name, never by local atom index; corruption fails closed.
- **DL-0096:** Defeats are durable conditional attacks; defeating an attacker can restore its target, while cycles remain undecided.
- **DL-0097:** Explanations distinguish active minimal support paths from blocked alternatives and report deterministic direct blockers.
- **DL-0098:** Decision batches validate once and commit atomically; failure cannot leave a partial batch.
- **DL-0100:** Materialize honors the standard workspace digest fence and idempotency key.
- **DL-0101 (human):** The release serves fiction, removes the former private consumer, retains the lorestore, ships `.dli` by default, and cuts unused work rather than archiving it in-tree.
- **DL-0102:** Remove the unused loader-reuse compiler while retaining safe full compilation and the independently checked evaluator reuse path.
- **DL-0103:** Serve only contract `2.0.0`; register each retired version and removed operation with its successor, preserving current replay and defeat semantics.

**Rationale.** The release needs a readable authoritative record while preserving
its surviving engineering laws and their historical evidence.
