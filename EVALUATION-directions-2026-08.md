---
status: delivered
verdict: "Internal decision doc: verified state of engine + experimental record, prior-art and market review, and the canonical menu of directions with evidence, triggers, and kill conditions. Non-canonical; the ledger outranks this file."
---

# EVALUATION — the canonical menu of directions (2026-08)

**Authorship: Claude (Anthropic), Claude Code session, 2026-08-18.** Internal
decision document for Jeff. Non-canonical: `docs/decisions.md` outranks it.
Numbers trace to a committed artifact, a cited external source, or the
delivered probe files (named as such). Sections marked **[ASSESSMENT]** are
interpretation; bottom-lines, theses, and kill conditions throughout are
likewise synthesis, not verified fact. A fresh-context red team verified the
§1 rows against their artifacts and spot-checked eight external claims; its
corrections are folded in.

Scope: what De Lore Is can and cannot do as evolved; what is known to be true
of systems and approaches like it; the current state of agent knowledge-
reasoning research; and a canonical menu of directions with supporting evidence
(market, literature, and direct code proof), each with triggers and kill
conditions, ending in a ranked recommendation.

---

## §1 Verified state

### §1.0 Lineage: what happened to the original conception

The founding session conceived DLI as: `.dli` text files in git as the
persistence layer; canon requiring a single sovereign human's ratification
("the machine never writes canon"); 19 `author.*` tools; fiction-first
positioning with three go-to-market ideas — don't compete with Fandom for
eyeballs, act as a **"Knowledge Refinery"** (scrape contradiction-heavy fan
wikitext, ingest as proposed/attested, let the compiler scrub contradictions),
and position as the **"Headless Lore Engine for AI agents"** (AI storytellers,
Discord bots, RPG companions, game engines).

Each original bet has a verdict now:

| Original bet | What happened | Authority |
|---|---|---|
| `.dli` text in git is the canonical surface | **Reversed by ruling.** Facts became the store; `.dli` is an opt-in rendering; the default build links no parser | DL-0041, LS-7 |
| "The machine never writes canon" — human ratification structurally required | **Reversed by ruling.** Attribution and decision state are caller-supplied data; the engine checks referential integrity only; agents mint rulings, supervision is async | DL-0043, DL-0052, DL-0054, DL-0058 |
| 19 authoring tools | Grew to **68 operations** at contract 1.12.0 | DL-0058→DL-0092 |
| Provenance algebra, three-valued standing, O(1) forks, total gate | **Survived intact** — and §2 shows the saturate-once/valuate-many core is the theoretically strongest part | — |
| GTM 2: "Knowledge Refinery" over messy open-domain text | **Falsified by measurement.** This is exactly the open-domain ingestion the 2026 benchmark campaign killed: the compiler can only scrub contradictions that land in a shared slot, and §1.2 shows they don't | the whole §1.2 record |
| GTM 1: don't compete for human eyeballs | Still correct; never contested | — |
| GTM 3: "Headless Lore Engine for AI agents" (storytellers, RPG companions, game engines) | **Never tested.** The fiction-consumer integration (TPtH) works in production, but the external market for it was never probed | §6 |

**[ASSESSMENT]** The evolution has a consistent direction: every reversal
removed a place where the engine presumed to know something about the caller
(who may rule, what surface they write) and kept the epistemic core. The one
market idea the campaign actually falsified is the refinery; the original
headless-engine idea was never run.

### §1.1 Code reality (deloreis @ `b6bf9cf`, 2026-08-17)

**IMPLEMENTED and serving.** ~80K source lines (~56K code) of Rust across 14 crates. Contract
**1.12.0** served, accepting every version 0.5.1–1.12.0
(`crates/dli-serve/src/lib.rs:115-163`). 68 operations on the wire (verified
live via `dli.version`). Every non-deferred PLAN node is `done` except LS-10
(`open`) and the consumer-owned external-blocked halves
(`docs/PLAN.md:729-806`): annotated seminaive fixpoint with support/
defeater polynomials (`dli-eval`, 4.8K LOC), the lorestore (JCS + SHA-256
content addressing, commit DAG, branches, O(M+E) merge-base, schema-v4 upcast;
`dli-lorestore`, 6.3K LOC), six continuity classes, three-valued standing,
graded findings with typed repair data, the killed-form registry, the mutation
harness, and the compiled-world cache (10.7×–124.6× on `author.merge` scan,
DL-0090). The default build links no `.dli` parser (LS-7): the released binary
serves `craft`, `validate-repo`, `serve`, `mcp` only — the text surface is
genuinely retired, not deprecated-in-name.

**PARTIAL.** `compile_incremental` exists at the loader level but measures
**0.97×–0.98×** — a net loss — on the identity workload
(`docs/design-notes/incremental-compile-baseline.md`). Root cause is
structural, not a bug: atom-level delta reuse is unsound under negation
(absorption `d ∧ (d ∨ poison) = d` silently destroys the delta's defeater),
the sound fallback is dependency-closed stratum granularity, and `same_as`
synthesizes alias rewrites over every predicate so an identity delta marks the
whole world affected. The named research step is per-derivation provenance;
parked with a trigger (DL-0089). The check battery is refuted as the next
bottleneck (0–1% of residual, falling — same note).

**DESIGN-ONLY.** The entire distribution lane: peer/transactor split,
supplementary-Magic-Set segment router, content-addressed peer cache, managed
transport (DIST-1..4, all `deferred`; `docs/spec/12-scale-and-distribution.md`,
DL-0062). Verified zero code: every "magic"/"transactor" hit in `crates/` is
the fiction domain (magic *practices*), not Magic Sets. The design is specific
(exactness oracle: a routed cluster must evaluate byte-identically to
whole-corpus evaluation), but nothing is built, and the triggers ("a benchmark
adapter records a representative query/update trace, or an archive deployment
needs the seam") have not fired.

**Consumer integration is real.** The fantasy/TPtH cutover completed the
authority transfer: the engine is the canon producer (parity.json generated
from the lorestore export with a provenance header and a required offline CI
check; migrated authority fallbacks deleted; FW-8 end-to-end `author.*`
session committed — `docs/integration/STATUS.md`). One production-shaped
consumer exists and it is the fiction one.

### §1.2 The experimental record (dli-benchmarks, 4 worktrees)

The campaign's verdicts, each verified against its committed artifact:

| Result | Verdict | Artifact |
|---|---|---|
| FF Round 1: assisted 0.377 vs direct 0.312 / control 0.338, long split | The one preregistered positive; stands, with its own caveat: the run's inference is exploratory, not confirmatory (no CI on the preregistered endpoint) | `reports/flawedfictions-realeval-fourarm.md` (the four-arm run; `full-v3.md` is the earlier direct/dli-only study) |
| FF length crossover ~30 sentences | Sharpens R1; standard-split loss masks longest-tercile win | `reports/flawedfictions-realeval.md` |
| FF structural ceiling: both GT sides extracted 78.0%, shared slot 18.1%, firable 14.8% | Extraction competent; key agreement fails | `reports/flawedfictions-realeval-ceiling.md` |
| R2 genuine margin over normalized hashmap: **4/10**, both precision-going | Red-team cut from 7/11; functional gate net-negative held-out (0.106→0.063, 0.039→0.000) | `reports/flawedfictions-round2-*.md` |
| R3: 1.7.0 mechanisms constructible, margin unchanged, $0 | Identity reproduces LLM coreference exactly; gold oracle doesn't move it | `reports/flawedfictions-round3-*.md` |
| Canonicalization: **NOT-SUPPORTED** | 0/160 judge-genuine firings (64% false merge, 32% paraphrase); pos/neg firing ratio 1.06; metric scored on resolver's own labels | `reports/flawedfictions-canon-audit.md` |
| Engine-gated merge rerun | 1,716 previews, FD consequence **clash rate 1.0 on positives AND negatives** — the engine ratifies every merge because merged slots always clash | `reports/flawedfictions-canon-engine-gated-v1.md` |
| Resolution calibration (480 items, 7 arms, 3 judges) | Entity: llm zero-false-merge only at recall **0.176**. Predicate: **no method reaches any non-trivial zero-false-merge operating point at any threshold** | `dli-benchmarks-side`, `reports/resolution-calibration-final.md` |
| SubtleMemory Gate 1 @ 1.11.0 | Retrieval fixed (8.00 usable rows/read, 98.98% coverage) — and the resolver still rejects **97.03%**; predicate collapse 0.5287, minting unstable (34–64/100 claims); zero functional predicates declared → zero FD findings possible | `dli-benchmarks-sm-rerun`, `reports/subtlememory-gate1-1.11-provider.md` |
| **ER-policy terminal experiment** (2026-08, post-orientation-doc) | **stop-negative.** $175.10, six preregistered policy variants over 1,280 sealed decisions incl. deterministic pronoun pre-pass; none passed the gate (best variant `richer-evidence`: merge rate 0.106 with 7/100 sampled false merges; `deterministic-pronouns` moved `I` 3→43 but overall gate still failed). Per prereg: "incremental canonicalization is closed, and no further SubtleMemory work is authorized." | committed at `dli-benchmarks-sm-rerun` `reports/subtlememory-entity-resolution-policy-final.md` (commit `b573bf1`, branch `subtlememory-gate1-1.11`); the `runs/` copy is gitignored |

**What this record establishes [ASSESSMENT of the rows above].** Two
independent corpora, two failure directions, six resolution policies, and a
seven-arm calibration agree: on open-domain, schema-free input, LLM-extracted
claims fragment at the (entity, predicate) key, and no post-hoc resolution
method reaches an operating point that moves the live corpus. Precisely: the
calibration's entity llm arm did meet its own preregistered bar (zero
held-out false merges at recall 0.176 ≥ 0.10) — but that recall is too low to
matter, as the terminal ER-policy run then showed on the live decisions (97%
rejection, stop-negative); on predicates no method reached any non-trivial
zero-false-merge point at all. The engine's two genuine discriminative
mechanisms (the functional gate, interval/inertia reasoning) are *precision*
mechanisms and lose on recall-scored benchmarks that the direct model already
saturates.

**What this record does NOT establish.** Nothing has ever been run against
the input distribution the engine was designed for — a declared vocabulary
with a horizon past context. That distribution has no published benchmark
(checked again in §4). The record is a scope statement, not a capability
refutation.

### §1.3 Direct code proof: the declared-vocabulary path works end-to-end

Feasibility probe run 2026-08-18 against the released binary (default
syntax-free build, contract 1.12.0, empty store-backed project): a micro
credit-agreement corpus driven wire-only through `deloreis serve`.
**37/37 checks passed**, one session, 40 wire calls, zero engine defects.
The driver script (`probe.py`) and full request/response transcript
(`probe-transcript.jsonl`) are delivered alongside this report — they are
**delivered files, not committed artifacts**; commit them beside this doc if
it is promoted. One honest gap: the post-defeat un-derivation check lacks a
pre-defeat control query in the same second workspace, so it is one control
query short of fully clean (the un-derivation itself is shown: rows=[]).

The arc, in order, all through `author.*`:

1. `record_decision` ×2 (executed agreement; amendment) — wire-only certainty,
   no fixture-authority anywhere.
2. Entities, an era, two events, one `add_before` — the temporal spine.
3. `declare_functional governing-law`, `declare_inertial interest-margin` —
   the vocabulary is *declared*, koan-3 clean.
4. Facts + two fluent states (350bps since closing; 400bps since amendment),
   each citing its decision.
5. `add_rule breach-triggers-default` (`in-default(f) :- breaches(f,c)`)
   citing the agreement.
6. A **proposed** breach relation (an agent's unratified monitoring claim) —
   derived `in-default(term-loan-a)` reads **`possible`**, not certain, and
   `author.why` names the open hypothesis blocking it. This is the abstention
   property, live.
7. `record_decision d-breach-confirmed` + cited re-assertion → derived
   `in-default` flips to **`certain`**; `why.supporting_decisions =
   ["d-agreement-executed","d-breach-confirmed"]` — the replayable audit
   answer.
8. `as_of` at closing: 350bps `holds-certainly` **with the derived cap** (the
   defeater names the amendment's value and decision); at amendment: 400bps.
9. A side-letter `governing-law = english-law` under a second decision →
   `continuity.fd-violation`, grade `checked`, typed data naming slot, subject,
   both values, both citing decisions; `materialize` **refuses**
   (`materialize-blocked`); `discard` by structural key repairs; clean
   materialize commits 24 fact objects + root hash.
10. `author.defeat` (waiver) on the stored breach claim → derived `in-default`
    un-derives. The full promote/defeat lifecycle, wire-only.

**[ASSESSMENT]** The gap between §1.2 and §1.3 is the whole evaluation: the
engine performs its designed function without defect the moment the vocabulary
is *given*; every measured failure is upstream of the engine, in the
open-domain naming step the design explicitly refuses to guess at (koan 3).
The product question is therefore not "does the engine work" but "where does a
declared vocabulary come from, and who pays for what it enables."

---

## §2 What is known to be true of systems like this (prior art)

**Bottom line.** Every individual mechanism has deep prior art, and the
literature has already mapped both the walls DLI hit and the walls it would
hit next. The combination — content-addressed versioned storage + provenance-
carrying Datalog + defeasible valuation — has **no existing system**; every
pairwise combination has a neighbor, the triple does not. The corollary cuts
both ways: the novelty is real, and the risks are all inherited and known.

- **"Saturate once, valuate many" is the engine's most solidly grounded
  claim** — it is the conjunction of the provenance-semiring homomorphism
  theorem ([Green/Karvounarakis/Tannen, PODS 2007](https://web.cs.ucdavis.edu/~green/papers/pods07.pdf))
  with knowledge compilation ([Darwiche & Marquis](https://arxiv.org/abs/1106.1819)).
  No reviewer should be able to push back on it.
- **Support/defeater polynomials are ATMS labels in modern dress** (de Kleer
  1986; minimal environments = prime implicants over PosBool(X)). The
  identification is mathematically immediate but *culturally unclaimed* — no
  paper headlines "ATMS = Datalog provenance in PosBool[X]"; soften any doc
  claim that it is a recognized equivalence. The canonical ATMS scaling
  failure is label explosion (worst-case exponential;
  [Darwiche](https://arxiv.org/pdf/cs/9806101)), and the field's mitigation —
  the focused ATMS / consumer architecture (Forbus & de Kleer, AAAI-88) — is
  structurally the Magic-Sets-cluster idea with weaker theory. Dressler's
  nonmonotonic ATMS out-assumptions are direct prior art for
  defeat-as-assumption.
- **The `compile_incremental` wall is a proven impossibility, not a bug.**
  Semiring provenance cannot extend to difference while preserving expected
  equivalences for some semirings — a class that includes the label algebras
  used here ([Amsterdamer/Deutch/Tannen 2011](https://arxiv.org/pdf/1105.2255);
  the paper does not itself name the absorptive class), and
  incrementalization algebraically requires subtraction — a commutative
  group — which absorption destroys (DBSP,
  [Budiu et al., VLDB 2023](https://www.vldb.org/pvldb/vol16/p1601-budiu.pdf)).
  DLI's absorption-destroys-the-delta finding
  (`incremental-compile-baseline.md`) is a local rediscovery of a global
  barrier, documented from both sides. The parked position (full resaturation;
  valuation overlays for what-ifs) is exactly what the combined literature
  prescribes. For the parked research step: incremental *verdicts* without
  labels are solvable off the shelf (DBSP Z-sets, Lean-verified, stratified
  negation included, shipped as Feldera); incremental *label maintenance*
  under negation is solved **nowhere** — and the most promising published
  shape is not sound polynomial deltas but Soufflé's cheap-witness /
  lazy-proof-reconstruction pattern
  ([Zhao/Subotić/Scholz, TOPLAS 2020](https://dl.acm.org/doi/10.1145/3379446)).
- **Magic Sets: feasible, industrial, one new theorem required.** Soundness/
  completeness for answers is settled (Beeri & Ramakrishnan,
  [On the Power of Magic](https://dl.acm.org/doi/10.1145/28659.28689));
  the named pitfall — the rewrite can unstratify a stratified program — has
  known fixes, sharpest being demand transformation with stratified negation
  ([Tekle & Liu](https://arxiv.org/abs/1909.08246)); LogicBlox/Soufflé/DLV
  deployed it industrially. But the DIST-1 exactness oracle needs something
  *stronger* than answer-equivalence: a shipped cluster must reproduce the
  same **labels**, which requires an all-derivations closure (magic may prune
  derivations that add no answers but do change the polynomial). That
  label-fidelity theorem is DLI's to state and prove — or the oracle relaxes
  to "same verdicts, possibly different polynomials."
- **Defeat sits on the tractable side of a hard complexity line, and should
  say so.** Grounded semantics is P-time; preferred/stable are
  NP/Π2p-complete ([Dvořák & Dunne](https://www.dbai.tuwien.ac.at/staff/dvorak/files/HOFA-complexity-authorsversion.pdf)).
  Per-atom polynomial-time valuation flips can only realize the
  grounded/defeasible-logic family — document that as a feature, not a
  limitation. One genuine gap: defeasible-logic variants disagree on standard
  examples along the ambiguity-blocking/propagating and team-defeat axes
  ([Governatori et al., JLC 2004](https://www.researchgate.net/publication/29456460_Argumentation_Semantics_for_Defeasible_Logic));
  DLI has not self-located on that map and a trial suite of the
  discriminating examples would close it. DeLP servers (warrant + dialectic
  explanation over a wire) are prior art for the serve surface.
- **The inertia semantics has the wrong citation.** "Open value persists until
  next distinct certain value" is not Kowalski–Sergot event-calculus inertia
  (which is event-driven via initiates/terminates); its real ancestors are
  Dean–McDermott persistence ([AIJ 1987](https://www.sciencedirect.com/science/article/abs/pii/0004370287900610))
  and temporal-DB last-known-value semantics. Defensible, nonstandard,
  should be documented as a design choice with the honest lineage. Two
  literature-backed validations: keeping `as_of` projection outside the
  eval/defeat path is the right firewall against Hanks–McDermott anomalous
  models, and per-predicate `declare_inertial` is the same move as EC's
  `ReleasedAt` (one inertia law never fits all fluents).
- **Datomic precedent: validated, with known limits.** Peer/transactor is
  production-proven; Jepsen confirmed both the consistency story and the two
  limits DLI would inherit — single-transactor write ceiling and stale peer
  reads ([Jepsen on Datomic Pro](https://jepsen.io/analyses/datomic-pro-1.0.7075)).
  `state ≡ fold(log)` is established doctrine (Kleppmann, event sourcing).
- **Nearest-neighbor systems.** ORCHESTRA (Penn, SIGMOD 2008) is the single
  most instructive: semiring-annotated facts, provenance-carrying update
  exchange, trust policies filtering by provenance — a valuation overlay in
  all but name — and its practical cost center was **provenance storage
  growth** ([update exchange](https://www.cs.ucdavis.edu/~green/papers/updateexchange.pdf)).
  FactDAG, Fluree, TerminusDB, Dolt, XTDB each hold one or two corners of the
  triple; Drools holds TMS + defeasibility with no versioned store.

**[ASSESSMENT]** Net: the theory under the engine is sound and in two places
(saturate-once; the delta wall) *provably* the right call. The three debts the
literature names are cheap to pay: re-cite inertia to Dean–McDermott,
self-locate defeat in the Governatori taxonomy with discriminating tests, and
state the label-fidelity requirement in the DIST-2 oracle. The one inherited
cost nobody has engineered away is ATMS label growth at scale — ORCHESTRA's
experience says provenance storage, not fixpoint time, is where an archive
tier would first hurt.

## §3 The identity problem: the literature already ran this experiment

**Bottom line.** Both calibration results have published twins, and the field's
fifty-year conclusion vindicates the engine's design choice. The slot problem
is not a DLI implementation failure; it is the measured state of the art, and
treating identity as a caller-supplied, defeasible, retractable claim is where
data integration independently landed.

- **The entity operating point is the published frontier.** Fellegi–Sunter
  institutionalized the three-zone rule (link / *clerical review* / non-link)
  precisely because a band of pairs is undecidable without a human; threshold
  placement is "left entirely to the practitioner"
  ([framework limits](https://www.zingg.ai/post/fellegi-sunter-model-limitations-modern-entity-resolution)).
  Modern SOTA (Ditto [arXiv:2004.00584](https://arxiv.org/abs/2004.00584);
  WDC Products [arXiv:2301.09521](https://arxiv.org/abs/2301.09521); GPT-4
  zero-shot F1 76–98 by corpus,
  [EDBT 2025](https://openproceedings.org/2025/conf/edbt/paper-81.pdf))
  reports best-F1 operating points with material false merges; published
  evaluations essentially never report a non-trivial zero-false-merge point.
  Fine-tuned matchers collapse 36–56 F1 on unseen entities. Practice routes the
  uncertain band to humans (Magellan, Falcon, CloudMatcher) or accepts partial
  resolution ordered by benefit (pay-as-you-go ER, TKDE 2013).
- **The predicate result has a near-exact twin.** OAEI 2013 property alignment
  (Cheatham & Hitzler, [OM 2014](https://ceur-ws.org/Vol-1317/om2014_Tpaper2.pdf)):
  classes average P/R/F 0.79/0.60/0.68 vs properties **0.36/0.18/0.21**;
  the only property precision-1.00 systems ran at recall **0.20–0.26** —
  the same frontier shape as our calibration, on a *cleaner* corpus (hand-built
  OWL with domain/range declarations). Several systems scored property F = 0.00;
  three OAEI 2024 matchers still don't attempt properties. The error anatomy
  matches ours exactly: false positives are high-string-similarity pairs with
  different domain/range; false negatives are true synonyms with no lexical
  overlap. OAEI's hard tracks are plateaued (anatomy best-F 0.941 in 2021 *and*
  2025; conference best F1 0.64 in 2024); LLM-era matchers are candidate
  bootstrappers that retain human verification
  ([VLDB-W 2024](https://arxiv.org/abs/2407.11852),
  [Matchmaker](https://arxiv.org/abs/2410.24105)).
- **"No ground truth for predicate identity" is measured, not speculative.**
  The OAEI re-issued its Conference gold standard as an *agreement
  distribution* over twenty judges
  ([ISWC 2014](https://link.springer.com/chapter/10.1007/978-3-319-11915-1_3));
  experts demonstrably disagree more on property matches than class matches;
  machine precision empirically saturates at the inter-judge agreement ceiling.
  Halpin et al. ("When owl:sameAs Isn't the Same",
  [ISWC 2010](https://link.springer.com/chapter/10.1007/978-3-642-17746-0_20)):
  three KR experts confirmed **73 vs 132 vs 181** of the same 250 sameAs links;
  large-scale closure studies over the 558M-link sameAs.cc corpus discard a
  material fraction as erroneous; transitive closure fused 177K names
  into single false equivalence classes
  ([survey, arXiv:1907.10528](https://arxiv.org/abs/1907.10528)). Open-KB
  canonicalization concedes it outright: "for relations gold linking is not
  available" (CESI, [WWW 2018](https://arxiv.org/abs/1902.00172)). Nuance to
  keep: indeterminacy is *partial* — a settled core plus a contested band —
  which argues for exactly a defeasible-claim mechanism, not full automation or
  full agnosticism.
- **The design is the field's conclusion.** Data integration formally abandoned
  automate-up-front around 2005–09: dataspaces / pay-as-you-go
  (Franklin/Halevy/Maier), probabilistic schema mappings motivated by "in some
  domains it is not clear what the mappings should be"
  ([VLDB 2007](https://www.vldb.org/conf/2007/papers/research/p687-dong.pdf)),
  and "in most schema matching scenarios, there is a human in the loop"
  ([Ten Years Later, VLDB 2011](https://vldb.org/pvldb/vol4/p695-bernstein_madhavan_rahm.pdf)).
  Production practice agrees: Knowledge Vault kept calibrated probabilities
  rather than asserting facts; Amazon's AutoKnow learns synonyms from behavior
  logs; **Wikidata's property lifecycle — proposal, consensus, deletion
  discussions, audit trail — is structurally DLI's model** run by humans
  ([Help:Properties](https://www.wikidata.org/wiki/Help:Properties)). The
  sameAs literature's central pathology (hard identity propagating through
  closure, expensive to undo) is precisely what wire-level defeasible defeat
  addresses; the academic contextual-identity proposals never reached
  production tooling.

**[ASSESSMENT]** Consequence for the roadmap: no plausible amount of resolver
engineering rescues open-domain predicate alignment — the field has been at
that wall for two decades with cleaner inputs. `assert_same_as` /
`assert_predicate_same_as` as recorded, defeasible, retractable decisions is
the correct primitive and its honest value is "make bad merges cheap to undo,"
not "enable detection." The calibration study itself is ahead of the published
literature (nobody else has drawn the LLM-extraction operating curve) and is
publishable as a negative result.

## §4 Agent-memory research: where the field is, and the gap

**Bottom line.** The field is converging on DLI's problem statement from two
directions while lacking its machinery; DLI's measured weakness is universal
and mostly unspoken; and the benchmark DLI needs does not exist — but its
construction method does.

- **Systems.** Two mechanism families: LLM-curated fact stores over vectors
  (Mem0, LangMem, Memobase, ChatGPT/Claude memory) and LLM-built knowledge
  graphs (Zep/Graphiti, Cognee, HippoRAG). Closest shipped mechanism to DLI:
  Zep/Graphiti's bitemporal edge invalidation
  ([arXiv:2501.13956](https://arxiv.org/abs/2501.13956)) — two-valued,
  LLM-adjudicated at ingestion, no defeaters, no derivation replay. **No
  surveyed system offers typed claims + explicit defeaters + three-valued
  standing + replayable derivations.** Telling 2026 reversals: Mem0 v3
  *abandoned* LLM-arbitrated UPDATE/DELETE for ADD-only accumulation
  ([Mem0 2026 report](https://mem0.ai/blog/state-of-ai-agent-memory-2026));
  OpenAI's "Dreaming" consolidation trades audit trail for recall
  ([coverage](https://www.techtimes.com/articles/317840/20260605/chatgpt-memory-dreaming-update-openai-rewrites-personalization-engine-limits-audit-trail.htm)).
- **Fragmentation is universal and unquantified.** Graphiti ships three-tier
  entity dedup and still leaks duplicates
  ([issue #875](https://github.com/getzep/graphiti/issues/875)); Cognee's docs
  name the pathology verbatim ("car manufacturer"/"automobile maker"
  fragmentation) and offer supplied-ontology grounding as the fix, with no
  numbers ([Cognee](https://www.cognee.ai/blog/deep-dives/grounding-ai-memory));
  the 2026 TOKI preprint independently names entity/predicate normalization as
  its *blocking limitation*
  ([arXiv:2606.06240](https://arxiv.org/pdf/2606.06240)). Nobody publishes
  fragmentation rates. Our §1.2 numbers are ahead of the literature.
- **Deterministic conflict machinery wins where tried.** Moving conflict
  resolution from LLM judgment to deterministic code yields 94.8% single-hop
  and **30.2% multi-hop vs ~7% best published** on MemoryAgentBench-style
  conflict tasks ([arXiv:2606.01435](https://arxiv.org/html/2606.01435v1)) —
  by *assuming slot alignment away* with numbered facts. The field's best
  conflict numbers without that assumption: ≈60% single-hop, ≈7% multi-hop
  (MemoryAgentBench, [arXiv:2507.05257](https://arxiv.org/abs/2507.05257),
  ICLR 2026).
- **The key gap, confirmed: no published benchmark tests long-horizon
  consequence tracking over a declared vocabulary.** The canon (LoCoMo,
  LongMemEval, MemBench, MemoryAgentBench) is all schema-free conversational
  prose. Nearest neighbors prove the construction method: **ACPBench**
  generates provably-correct consequence questions *from formal PDDL domains*
  ([arXiv:2410.05669](https://arxiv.org/abs/2410.05669), AAAI 2025) but is
  single-shot; **τ-bench** has declared policy + typed DB state but tests task
  execution, not epistemic QA; RuleArena tests single-episode rule application.
  The 2026 successor wave (LongMemEval-V2, MINTEval, WorldLines, A-TMA, BEAM)
  is crowding the *long-horizon prose* lane monthly — none covers declared
  vocabulary, defeat, or provenance-conditioned answers.
- **Abstention: counter-cyclical and unrewarded.** AbstentionBench (NeurIPS
  2025, [arXiv:2506.09038](https://arxiv.org/abs/2506.09038)): abstention is
  unsolved, scale-insensitive, and **reasoning fine-tuning degrades it 24%**.
  No benchmark rewards abstention-with-witness (naming the fact that would
  settle the question) — DLI's most distinctive property. In a
  declared-vocabulary setting the settling fact is *computable*, so the
  benchmark gap above and the abstention gap are **the same missing
  benchmark**.
- **Schema-guided extraction supports the vocabulary bet, directionally.**
  Ontology-grounded extraction "greatly surpasses" native LLM identifier
  grounding (SPIRES,
  [Bioinformatics 2024](https://academic.oup.com/bioinformatics/article/40/3/btae104/7612230));
  Text2KGBench measures conformance to a supplied ontology (imperfect, not
  free); constrained decoding guarantees vocabulary *membership*, converting
  silent slot-miss into visible forced choice. **The decisive experiment —
  same corpus, vocabulary on/off, measure contradiction-slot landing rate —
  appears unpublished, and our no-vocabulary arm is already measured.**

## §5 Market demand

**Bottom line.** Demand is strong for the *problem* and unnamed for the
*mechanism*: memory corruption, non-determinism, and audit are documented
buyer pain, but nobody yet pays for replayable derivation as such. The money
is in verticals; the distribution is MCP; the wedge that makes the mechanism
legible is the benchmark.

- **Agent-memory market: real, small-check, cheap.** Mem0 $24M A
  ([TechCrunch](https://techcrunch.com/2025/10/28/mem0-raises-24m-from-yc-peak-xv-and-basis-set-to-build-the-memory-layer-for-ai-apps/),
  41K stars, $19–249/mo); Letta $10M seed; Cognee $7.5M; Zep ~$500K + Graphiti,
  killed its free/self-hosted tiers in 2026. Documented pain: an independent
  audit found **97.8% of 10,134 production Mem0 entries were junk**, including
  808 entries asserting one false preference
  ([mem0 #4573](https://github.com/mem0ai/mem0/issues/4573)); a Rust memory
  engine advertising heuristic contradiction detection produced 54 false
  positives in 60 flags — HN converged on "can't adjudicate without
  context/vocabulary" ([HN](https://news.ycombinator.com/item?id=47767119)).
  Zep sells "temporal contradiction handling" at the marketing level, but the
  ecosystem's own literature names the residual gap precisely: "nothing says
  whose assertion won the conflict, or why" — provenance is "the missing who"
  ([Mnemoverse](https://mnemoverse.com/docs/library/provenance-in-agent-memory)).
  An HN post asking for exactly DLI's feature list (provenance, contradictions,
  audit) drew zero comments — the demand is nameable but not yet loud.
- **Verticals are where verifiability is a purchase criterion.** Harvey $200M
  at **$11B** ([CNBC](https://www.cnbc.com/2026/03/25/legal-ai-startup-harvey-raises-200-million-at-11-billion-valuation.html));
  Blue J $122M Series D selling "defensible tax answers"
  ([BusinessWire](https://www.businesswire.com/news/home/20250804963246/en/));
  Hebbia's "auditable trail for every data point"; 9fin/Ontra selling covenant
  analysis with citations; **Norm Ai $120M Series C at $1.2B whose cap table
  is its customer list** (Blackstone, Vanguard, Citi — regulated buyers funding
  their compliance plumbing). Forcing function in legal: 1,598 court cases
  involving fabricated citations, sanctions to $55K
  ([audit](https://www.haqq.ai/blog/ai-legal-hallucination-audit)). Honest
  caveat: what buyers verifiably pay for is *span citations + logs*, not
  replayable inference — DLI's guarantee is stronger than the market knows how
  to ask for.
- **Regulatory: tailwind, not trigger.** EU AI Act Art 12 logging slipping
  Aug 2026 → Dec 2027; the Fed's SR 26-2 *explicitly excludes* generative and
  agentic AI from model-risk scope
  ([SR 26-2](https://www.federalreserve.gov/supervisionreg/srletters/SR2602.htm));
  FINRA undecided whether a prompt is a record. Real duties exist (Circular 230
  due diligence on AI-generated positions, PCAOB documentation) but are
  satisfiable with logs + human review. The regulatory language converges on
  *reconstruction*; replay is the cheapest way to make reconstruction
  trustworthy — an argument, not a mandate, with a 12–24 month runway.
- **Infrastructure: the complaint is DLI-shaped; the spend goes elsewhere.**
  70% of leaders name non-deterministic outputs the #1 production-readiness
  barrier; 88% of agent pilots fail to reach production
  ([LangChain State of Agent Engineering via VentureBeat](https://venturebeat.com/orchestration/enterprise-ai-is-entering-an-evaluation-gap-agents-are-gaining-autonomy-faster-than-companies-can-verify-them)).
  But when enterprises act on it they buy evals/observability (Braintrust $80M
  at $800M; Arize $70M) — watching the nondeterminism, not removing it. MCP is
  free distribution: ~10K public servers, ~97M monthly SDK downloads, every
  major client speaks it.
- **Honest negatives.** Platform bundling absorbs the commodity layer
  (Anthropic memory tool + Managed Agents memory; AWS AgentCore memory
  advertising "compliance and audit trails"); willingness-to-pay for standalone
  memory is $19–249/mo developer-tool pricing; "your Postgres is enough"
  sentiment is durable; the successor-benchmark lane is crowding at roughly
  one per month. Bundling kills thin-wrapper memory APIs — which is not this
  engine's business — but nothing in the sweep shows anyone paying a premium
  for determinism as such yet.

## §6 The original scope: fiction-management technology

**Bottom line.** Nothing in the fiction stack does what the engine does — the
gap is real, quantified, and now legible to other founders — but the
human-hobbyist segment demonstrably does not pay for enforced consistency,
and the buyer whose shape matches the engine's design is a *machine* author.
(Internal seed: `docs/design-notes/architecture-comparison.md`; note it is
pre-pivot in places — "the machine never writes canon" is superseded by
DL-0054/DL-0058, and git-as-persistence predates the lorestore.)

- **Fan wikis: DLI's function, performed socially, for free, at scale.** The
  largest canon operations (Wookieepedia's Canon/Legends split, Memory
  Alpha's precedence rules, AWOIAF's "the novels always trump") enforce canon
  tiers, retcons, and citation discipline entirely by policy, templates, and
  admin judgment — SMW/Cargo store infobox data for *querying*, never
  constraints ([canon policy](https://starwars.fandom.com/wiki/Wookieepedia:Canon_policy);
  [Cargo](https://www.mediawiki.org/wiki/Extension:Cargo)). Documented pain:
  a formal Avatar Wiki war-room debate on re-splitting canon from fanon,
  curated lists of canon edit wars. A retcon is a manual rewrite across N
  pages with no propagation guarantee. Twenty years of this proves the work
  is valued — and that communities do it unpaid.
- **Worldbuilding tools ("lorebases"): zero consistency checking anywhere.**
  World Anvil, LegendKeeper, Kanka, Campfire, Plottr, Obsidian vaults,
  Fantasia Archive — linked note stores with presentational timelines; the
  category's whole continuity answer is "keep it in one place." Obsidian's
  Dataview is the closest to facts-as-data: query, not constraint. Authors
  learn of continuity errors from reader email. The revealed economics are
  damning: World Anvil claims 3.5M+ users yet is a ~16-person company with no
  outside funding and sub-$1M third-party revenue estimates
  ([Tracxn](https://tracxn.com/d/companies/world-anvil/__o2wnIpwqT81OG4AoPIFv2EyAiYO9ARu0MbIaAOzTrWc));
  the deepest structured system in class (Novelcrafter Codex) is dinged for
  setup friction while freeform tools win adoption. A 2025–26 startup wave
  (Urdr's "Consistency Engine," Novarrium's "Logic-Locking") now says
  "contradiction detection" out loud — all RAG/LLM-probabilistic, all
  pre-traction.
- **AI-fiction lore systems: injection, never verification.** NovelAI
  Lorebook, SillyTavern World Info, AI Dungeon memory, Sudowrite Story Bible,
  Novelcrafter Codex all share one mechanism — get lore *into* the prompt
  (keyword triggers or always-present bible); nothing shipping checks
  generated output *against* the lore. Documented drift: a 70K-word Sudowrite
  test lost character eye color and backstory by ~chapter 15 despite the
  Story Bible ([review](https://ucstrategies.com/news/sudowrite-review-i-tested-the-22-month-ai-against-chatgpt-across-70000-words/)).
  Research now quantifies it: **40–68% fact-conflict rates across models in
  long interactive narratives** (NCP-Bench,
  [arXiv:2608.08160](https://arxiv.org/html/2608.08160); survival measured at
  20 turns); consistency bugs
  concentrate in factual/temporal dimensions and peak mid-narrative
  (ConStory-Bench, [Findings of ACL 2026](https://aclanthology.org/2026.findings-acl.410/));
  and LLM checkers are themselves unreliable and degrade with length
  (FlawedFictions, [arXiv:2504.11900](https://arxiv.org/abs/2504.11900)) —
  which undercuts the "AI checks AI" competitors and is the precise argument
  for a deterministic replayable kernel. (Two of these three are the
  campaign's own suites, independently rediscovered from the market side.)
- **Studios: the workflow exists, mechanized by a librarian.** Lucasfilm's
  Holocron — 55,000+ entries in FileMaker Pro, one keeper, licensed products
  manually vetted against it, and in 2014 the entire tracked EU demoted to
  "Legends" by decree ([Holocron](https://starwars.fandom.com/wiki/Holocron_continuity_database)).
  ESO employs a salaried Loremaster with an unreliable-narrator doctrine for
  absorbing contradictions. articy:draft is a content database (entities,
  variables, branching, localization) with no canon grading, provenance, or
  contradiction detection; narrative/localization QA is manual
  playthrough-plus-spreadsheet. Studios run exactly the canon-vetting
  workflow DLI mechanizes — with human reading as the checking step. The
  wedge is scale pressure: live-service content velocity, or generative NPC
  dialogue that must not contradict canon at runtime.
- **Solo-RPG / AI GMs: the loudest consumer complaint, the smallest market.**
  Analog GM emulators (Mythic GME 2e) delegate all continuity to the
  player's journal. AI GMs made memory the competitive axis because
  forgetting/contradicting the campaign is the top user complaint (AI Dungeon
  sells 16× context as its premium tier; a rival markets itself on "why AI
  game masters forget your campaign — and how we don't"); shipped answers are
  all bigger-context-plus-RAG, documented to drift on long campaigns. Paying
  consumers, $10–30/mo, indie two-person vendors, no visible funding: a
  proving ground, not a market.

**[ASSESSMENT]** Three reads, in order of confidence. (1) The original GTM
intuition "don't compete for eyeballs" was right, and its unstated corollary
is now measured: human fiction authors won't pay for discipline — they buy
presentation, friction-reduction, and memory-as-effect. (2) The problem the
engine was built for is real at scale and getting independently named (wiki
labor, loremaster salaries, LQA cost, 40–68% conflict rates, a startup wave
saying "consistency engine") — and every entrant is building the
probabilistic version whose failure modes the research already documents.
(3) The buyer whose shape matches the design — tolerates typed claims,
graded standing, declared vocabulary — is a machine author: generative
fiction pipelines, AI GMs, live-service narrative teams feeding generation.
That is direction C specialized to fiction, with TPtH as its worked
production integration, and it is a bet on the 2026–28 generative-fiction
production stack, not on today's World Anvil user.

---

## §7 The menu

Each direction: thesis, evidence for and against, code-feasibility proof,
cost shape, and the condition that would kill it. Directions are not mutually
exclusive; §8 sequences them.

### A. The declared-vocabulary vertical build (credit / tax / regulated workflow state)

**Thesis.** The slot problem dissolves when the application declares the
vocabulary, and regulated verticals have exactly what the engine wants: a
stable professional vocabulary, genuinely defeasible reasoning (waivers defeat
breaches; amendments defeat terms; a ratified position defeats a proposed
one), long horizons, and money attached to "show me why."

- **For.** §1.3: the entire arc runs today, wire-only, zero engine defects —
  the direct code proof. §5: verifiability is the *stated* purchase criterion
  of every vertical winner (Harvey $11B, Blue J "defensible tax answers,"
  Hebbia's auditable trail, Norm Ai's investor-as-buyer pattern); Circular 230
  imposes real due-diligence duties on AI-generated positions. §4: ontology-
  grounded extraction demonstrably stabilizes identifiers; constrained
  decoding guarantees vocabulary membership by construction.
- **Against.** §5's honest caveat: what buyers verifiably pay for today is
  span citations + logs, which incumbents deliver without an epistemic engine;
  the regulatory forcing functions slipped 12–24 months; this is a build with
  a sales cycle, not an experiment; it needs a design partner with a real
  corpus. And the load-bearing bet — that a *declared* vocabulary makes
  LLM lowering land contradictions in shared slots at usable accuracy — is
  supported directionally by the literature but **unproven at the operating
  point that matters** (§4: the decisive experiment is unpublished).
- **Code proof.** §1.3 end-to-end; `declare_functional`/`declare_inertial`/
  `add_rule`/`record_decision`/`defeat`/`as_of` all live at 1.12.0. Missing:
  nothing engine-side for a pilot; everything missing is corpus, lowering
  harness, and packaging.
- **Kill condition.** Run the vocabulary-on/off experiment (§8, experiment 1).
  If declared-vocabulary lowering still fragments or forced choice substitutes
  mis-mapping errors at a rate that erases the FD signal, the vertical thesis
  loses its mechanism and this direction dies with it.

### B. Author the missing benchmark (declared vocabulary × long horizon × defeat × abstention-with-witness)

**Thesis.** The benchmark that would test DLI's thesis does not exist — §4
confirmed it twice (no declared-vocabulary consequence tracking; no
abstention-with-witness scoring). Its construction method exists: ACPBench
proves consequence questions can be generated from a formal domain with
provably correct answers; MemoryAgentBench proves the incremental-conflict
protocol and supplies the incumbent numbers to beat (≈60% single-hop, ≈7%
multi-hop conflict resolution). The engine's replayable kernel is a
ready-made deterministic grader.

- **For.** Cheapest direction ($0 engine work); fastest credibility; the
  market measures exactly this pain (70% non-determinism blocker, 88% pilot
  failure) and spends on evals; establishes *both* differentiators (the
  functional/interval margin AND abstention) in a venue that rewards them
  instead of punishing them; the 2026 successor-benchmark wave is crowding
  the long-horizon-prose lane monthly but **none** covers declared vocabulary,
  defeat, or provenance-conditioned answers — the niche is open and claimable.
- **Against.** Benchmarks don't monetize directly. Vendor-built benchmarks
  draw justified skepticism (the MnemeBrain 100%-vs-0% cautionary tale), so it
  must be engine-neutral: runnable by Mem0/Zep/plain-Postgres adapters, graded
  by an open deterministic scorer, with DLI entered as one contestant. The
  three standing preflight traps apply with full force (the baseline must be a
  competent schema-aware store, the arms must demonstrably differ, and the
  scoring must never route through the treatment's own output).
- **Code proof.** The grader path exists: ground truth = the kernel's own
  fixpoint over the generated corpus (§1.3 pattern); `worlds/` fixtures and
  the dli-benchmarks harness architecture (receipts, checkpointing, sealed
  artifacts, preregistration discipline) carry over intact.
- **Kill condition.** A credible third-party benchmark covering the same axes
  appears first (watch the successor wave monthly), or the Phase-0 experiment
  below shows the declared-vocabulary setting doesn't discriminate — in which
  case the result publishes as a negative and the thesis itself is answered.

### C. Schema-supplied agent-memory middleware (the MCP epistemic-memory server)

**Thesis.** Ship the engine as an MCP memory layer where the application
declares its vocabulary and the engine supplies standing, defeat, why, and
abstention — the schema-first lane Memobase quietly proved viable, upgraded
with actual epistemics. Distribution is free: ~10K MCP servers, every major
client speaks the protocol, and the server exists (§1.1, 68 ops).

- **For.** §5: memory corruption at scale is documented (97.8% junk audit);
  the ecosystem names its own gap in DLI's vocabulary ("provenance is the
  missing who"); heuristic contradiction detection without a vocabulary
  visibly fails (54/60 false flags, HN consensus "can't adjudicate without
  context"); platform bundling kills thin wrappers but none of the platforms
  ship epistemics. §4: the field's own trajectory (Mem0's ADD-only retreat,
  Zep's two-valued invalidation) is converging on needing what the engine
  already has.
- **Against.** Willingness-to-pay is $19–249/mo developer pricing; the
  determinism complaint currently converts into observability spend, not
  substrate spend; declaring a vocabulary is real onboarding friction against
  drop-in memory; mindshare belongs to Mem0/Zep. As a *business* this is weak
  standalone; as OSS distribution riding B's credibility it is nearly free.
- **Code proof.** §1.3 ran against the released binary over an empty store
  through `deloreis serve`; `deloreis mcp` serves the identical surface. The
  missing work is packaging and DX (quickstarts, adapters, a hosted story),
  not capability.
- **Kill condition.** After honest DX investment and a B-shaped demo, no
  adoption — i.e., the market confirms it will not declare vocabularies even
  when shown the margin.

### D. Sell the audit layer ("version control for beliefs") — a positioning, not a build

**Thesis.** Replayable why, recorded defeasible rulings, and
abstention-naming-blockers as the epistemic audit substrate for agent systems.

**[ASSESSMENT]** The evidence (§5) says D is not a standalone direction: the
regulatory language converges on *reconstruction*, incumbents satisfy it with
logs, the mandates slipped, and nobody yet pays for replay as such. D is the
correct **marketing frame for A and C** — "replay is the cheapest trustworthy
reconstruction" — and the correct paper framing for B/H. Fold it into those;
do not staff it separately. Revisit trigger: EU Art-12 harmonized technical
standards work starts naming *how* traceability is satisfied (the standards
are unwritten — a reference implementation could shape them), or a Norm-Ai-
shaped partner asks for replay explicitly.

### E. Engine-ahead-of-evidence tracks (keep parked, sharpen triggers, pay the cheap debts)

- **E1 — per-derivation provenance / incremental labels.** §2: the naive goal
  (sound polynomial deltas) is *provably* unattainable under absorption;
  incremental label maintenance under negation is solved nowhere; the
  promising shape is Soufflé's cheap-witness + lazy-reconstruction. Keep
  parked on the DL-0089 trigger; if ever entered, enter with that design and
  treat it as publishable research, not a perf ticket.
- **E2 — DIST peer/transactor + Magic Sets.** §2: answer-soundness is
  buyable off the shelf (Tekle & Liu demand transformation); the exactness
  oracle as written requires an unproven **label-fidelity theorem**
  (all-derivations closure) — either prove it or weaken the oracle to
  same-verdicts. ORCHESTRA's lesson: provenance *storage growth*, not
  fixpoint time, is where the archive tier will first hurt. Triggers stand
  (no representative trace exists; no archive deployment needs the seam).
- **E3 — multi-authority / argumentation.** Grounded-family placement is
  complexity-forced and correct; the full layer stays parked
  (`multi-authority-knowledge-evolution.md` remains `exploring`).
- **E4 — cheap debts worth paying now (doc/test-only, near-zero cost).**
  (1) Re-cite the inertia semantics to Dean–McDermott persistence rather than
  event-calculus inertia. (2) Self-locate defeat in the Governatori taxonomy
  (ambiguity blocking vs propagating, team defeat) with the discriminating
  examples as a trial suite. (3) Add the label-fidelity requirement (or its
  explicit waiver) to the DIST-2 oracle text. (4) Soften any "ATMS ≡
  provenance polynomials is recognized" phrasing to "mathematically immediate,
  culturally unclaimed." Each closes a real review vulnerability named by the
  literature.

### F. Fiction: the corpus in hand, and the machine-author market

**Thesis (revised by §6).** Split F into two halves with different verdicts.

- **F1 — the worldbook as corpus: keep, and use inside B.** TPtH is a real
  declared-vocabulary long-horizon corpus in production against the engine
  (§1.1) — a legitimate second domain for B's corpus diversity and for
  Experiment 1, while remaining skeptic-discounted as *external* evidence
  because the engine was built against it. Cost ~zero; no separate direction
  needed.
- **F2 — the fiction-tools market: do not chase the human segment.** §6 is
  unambiguous: hobbyists and authors don't pay for enforced consistency
  (World Anvil's 3.5M-users/16-employees/<$1M economics; freeform beats
  structure everywhere; consistency-engine startups all pre-traction). The
  original "Headless Lore Engine" idea survives only in machine-author form:
  generative-fiction pipelines, AI GMs, and live-service/generative-NPC
  narrative teams — where 40–68% fact-conflict rates are measured, LLM
  checkers degrade with length, and the checking step today is a salaried
  human (§6). That is direction C with a fiction vertical vocabulary, and it
  is a timing bet on the generative-fiction production stack maturing.
- **Kill condition for F2.** Same as C's, plus: if the AI-GM / generative-
  narrative segment stays at two-person-indie scale through 2027, there is no
  buyer of infrastructure there regardless of fit.

### G. Closed lines — do not reopen without new evidence

Stated so no future agent reopens them by accident:

- **Open-domain incremental canonicalization** — closed by the terminal
  stop-negative (§1.2). Reopening requires a materially different mechanism,
  not a better resolver prompt: six policies, seven methods, and an
  engine-FD gate have all been tried.
- **FlawedFictions paid execution** — retired pending a materially new
  evidence-generation hypothesis (dli-benchmarks README).
- **Check-battery slicing** — refuted by measurement (0–1% of residual,
  DL-0089).
- **`compile_incremental` wiring into the lorestore** — 0.4ms of a 280ms
  compile; dead for present purposes, and §2 shows the ceiling is structural.
- **Predicate-resolution threshold tuning** — §3: no operating point exists;
  the literature has been at that wall for two decades with cleaner inputs.

### H. Publish the negative results (new, emerged from the sweeps)

**Thesis.** The calibration study (the operating curve nobody else has drawn),
the two-corpus fragmentation measurements (FF: 78.0% both-sides-extracted vs
18.1% shared-slot, committed in `realeval-ceiling.md`; SM: 306/377 = 81.2% vs
30/377 = 8.0% from the Gate-2 coverage analysis — **re-derive this pair from
the sealed artifacts before publication; it is in the campaign record but was
not located in a committed report by the red team**), and the engine-gated
1.0/1.0 result are **ahead of the published literature**: no memory system publishes fragmentation rates, TOKI
names the problem as its blocking limitation without quantifying it, and the
matching literature's nearest numbers (OAEI property alignment) are on far
cleaner corpora. A short, honest negative-results paper (arXiv + workshop)
establishes priority, hardens B's design rationale, and costs days, not weeks.

- **Against.** Publication effort competes with build time; negative results
  draw less attention without the constructive companion (B).
- **Kill condition.** None needed — this is cheap and severable; the only
  failure mode is letting it block B.

## §8 Ranked recommendation and sequencing

**[ASSESSMENT] — the whole section.**

**The centerpiece observation: one experiment serves three directions.** The
decisive unpublished experiment from §4 — same corpus, vocabulary on/off,
measure where contradictions land — is simultaneously: B's Phase 0 (does the
declared-vocabulary setting discriminate?), A's gating experiment (does the
vertical mechanism exist?), and F's internal validation (run one arm over the
worldbook). The no-vocabulary arm is *already measured* (§1.2). Run this
before anything else.

1. **Experiment 1 — declared-vocabulary lowering (run next).** Default
   corpus: a **credit-agreement set** (or another vertical corpus with a
   natural vocabulary) — not SubtleMemory. Reusing SM's sealed corpora is
   attractive (corpora and receipts exist) but its prereg's stop rule says
   "no further SubtleMemory work is authorized"; a different-mechanism
   carve-out must not authorize itself — **SM reuse requires Jeff's explicit
   sign-off in the new prereg**. Protocol: run BOTH arms (vocabulary-on with
   constrained decoding, vocabulary-off) **contemporaneously under one
   engine-scored protocol** — do not compare a fresh engine-scored arm
   against the §1.2 label-scored historical baselines (that is a
   cross-instrument comparison: different scorer, older contracts, and the
   SM baseline declared zero functional predicates so its FD precision is
   structurally undefined). Measure contradiction-slot landing rate and
   FD-finding precision via `author.check` findings in both arms. Preflight
   the three traps explicitly: competent baseline (vocabulary-off uses the
   deployed pipeline, not a store-blind strawman), arms verified to differ
   (count vocabulary-constrained emissions), engine-side scoring (never
   string comparison on the treatment's own labels).
2. **H immediately, in parallel** — the negative-results write-up requires no
   new measurement.
3. **If Experiment 1 clears: B** — build the benchmark (ACPBench-style
   generation over 2–3 declared domains including one vertical and the
   worldbook; MemoryAgentBench-style incremental protocol; conflict, defeat,
   consequence, and abstention-with-witness splits; engine-neutral adapters;
   open deterministic grader). Publish alongside H.
4. **A follows B's artifact** — use the benchmark + probe as the design-
   partner demo in credit or tax; sell under a vertical player (the Norm Ai
   pattern), not against Harvey/Hebbia.
5. **C rides along** — package `deloreis mcp` with quickstarts once B exists
   to point at; treat as OSS distribution, not revenue.
6. **E4 debts now** (days); E1–E3 stay parked with §7-E's sharpened triggers.
7. **G stays closed.**
8. **If Experiment 1 fails** — the vocabulary bet itself is falsified at the
   operating point that matters. Then the honest menu shrinks to: H plus B-as-
   negative-result (publish that even a supplied vocabulary doesn't feed the
   engine — a genuine contribution), the TPtH/fiction integration as the one
   proven consumer, and a fundamental rethink of the commercial reframe. That
   outcome is informative, not wasted.
