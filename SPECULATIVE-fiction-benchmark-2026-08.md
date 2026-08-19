---
status: exploring
verdict: "Speculative direction doc: a TTRPG/fiction lore-continuity benchmark built in the generation direction (engine-authored world, rendered narrative, fixpoint-computed ground truth). Maps what such a benchmark must test onto shipped capability vs the extensions menu, against the campaign record and the 2020–2026 literature. Authorizes no spend. Non-canonical; the ledger outranks this file."
---

# SPECULATIVE — the fiction-continuity benchmark (2026-08)

**Authorship: Claude (Anthropic), Claude Code session, 2026-08-19.** Internal
speculative-direction document for Jeff — a "what if we built the benchmark
around the *original* fiction use cases" analysis, explicitly downstream of
`EVALUATION-directions-2026-08.md` (whose §7B proposes authoring the missing
benchmark and whose §G closes several lines this doc must not reopen) and
`EXTENSIONS-menu-2026-08.md` (whose packs price every capability this doc
would pull on). Evidence: six parallel agents (three over deloreis /
dli-benchmarks / the fantasy repo, three cited literature sweeps) plus a
commissioned external deep-research survey of narrative-consistency and
long-document QA benchmarks (PDF, 2026-08; cited below as **[DR]**). Claims
carry `file:line`, a URL, or **[ASSESSMENT]**. Nothing here authorizes
spend; if pursued, this direction gets its own preregistration first.

**One-paragraph thesis.** Every fiction-consistency benchmark published
2020–2026 builds its ground truth by extraction from existing prose, human
annotation, flaw injection, or a prompted-LLM checker — and the field's own
validation numbers show why that ceiling is low (professional writers catch
17.1% of known injected consistency errors; the best automated checker
reaches F1 0.678; LLM judges are measured-unreliable in the adjacent
role-play domain). Nobody authors the formal world *first* and renders the
narrative *from* it, though that construction is standard in planning
benchmarks and is exactly what this engine does for a living. The empty
cell — multi-session campaign horizon × independent formal world model as
grader × GM-side canon consistency × retcon-as-defeat (history stays
queryable) × scored abstention-with-witness — survives contact with ~40
surveyed benchmarks and the commissioned survey's own gap analysis. A v1
benchmark covering five of the strongest task families requires **zero
engine extensions**; the remaining families each name their price in the
extensions menu.

---

## §1 The governing fork, and the closed lines this does not reopen

**Extraction vs generation.** The campaign's fiction record is a sustained
measurement of the *extraction* direction — prose in, an LLM lowers claims,
the engine detects — and it is dead in that direction: dli-only as sole
classifier scored 4.83% recall on FlawedFictions with the failure
decomposed as 59.9% alignment failure and 22.0% extraction miss
(`dli-benchmarks` `reports/flawedfictions-realeval-full-v3` record); the
structural ceiling found both ground-truth sides extracted 78.0% of the
time but landing in a shared slot only 18.1% (firable 14.8%)
(`reports/flawedfictions-realeval-ceiling.md`); canonicalization audited
NOT-SUPPORTED (0/160 judge-genuine firings) and the terminal ER-policy
experiment ran stop-negative ($175.10). The *generation* direction inverts
the pipeline: author the formal world first, evolve it, render narrative
from it, and quiz systems against the engine's own fixpoint. Slot
alignment then **never exists as a problem** — violations are planted by
perturbing slots the generator already knows.

**The retirement clause, honestly.** FlawedFictions paid execution is
retired "pending a materially new evidence-generation hypothesis"
(dli-benchmarks `README.md:116-118`), operationalized as dev targets
(≥70% conflict-evidence precision, ≤10% negative-story firing, ≥25%
positive coverage, `REMEDIATION.md:35-39`). Three findings from the record
read [**ASSESSMENT** in the last two]:

1. A generation-direction benchmark is a materially different
   evidence-generation hypothesis *in the literal sense* — ground truth is
   computed by deterministic fixpoint over authored slots, not extracted
   from prose by a resolver, so §G's "materially different mechanism, not
   a better resolver prompt" bar is met by construction. But it is not an
   FF *resumption* and must not be framed as one: it is a different
   benchmark, and the FF-specific dev targets never arise.
2. The extraction failure reappears wherever prose must be mapped back
   onto formal slots. If DLI enters its own benchmark as a contestant via
   open-domain lowering, that re-runs the measured-dead line. The declared
   vocabulary must be **task input** (the campaign bible/rulebook declares
   the vocabulary — natural in the TTRPG frame), which makes DLI-as-
   contestant exactly the Experiment-1 question (§7 below).
3. The generation direction has a **new validity threat FF never had**:
   renderer fidelity. If the rendered narrative doesn't faithfully entail
   the authored facts, engine ground truth is wrong *about the narrative* —
   judge circularity in a new form (trusting the renderer about what the
   prose says). This gets its own workstream in §5; Shadow-Loom's
   re-extraction audit loop ("miracle-step" detection,
   [arXiv:2605.02475](https://arxiv.org/html/2605.02475v1)) is the prior
   art to improve on.

**Closed lines honored.** Nothing below performs similarity merging, tunes
resolution thresholds, or resumes FF paid execution. Identity in the
benchmark is *declared canon by construction* (aliases planted by the
generator as `assert_same_as` claims), which turns the closed
canonicalization line into a design asset: the benchmark tests whether
*systems under test* honor declared identity, never whether a resolver
guesses it.

---

## §2 What the benchmark must test, mapped to capability

Twelve candidate task families, each mapped against contract 1.12.0
(68 ops) and the extensions menu (pack letters and S/M/L costs from
`EXTENSIONS-menu-2026-08.md` §3). Full op-level detail is in the mapping
agent's record; this is the decision table.

| # | Task family | Shipped at 1.12.0 | Needs from menu | Genuinely missing |
|---|---|---|---|---|
| T2 | Long-horizon consequence QA (derived facts after N sessions; engine as grader) | **Complete** — the §1.3 probe arc (37/37) is this task's grader path | D1/D2/B1 shrink the harness (S–M) | nothing |
| T6 | Abstention-with-witness (canon genuinely open; answer = "unresolved, blocked on X") | **Complete for decision-blocked case** — `possible` standing + `why` names the open variable (probe step 6) | E4(a) min_why wiring (S); E4(b) repair sets (S–M) | E4(c) abductive why-not for never-derivable atoms (M–L) — defer those items |
| T3 | Retcon propagation (defeat mid-campaign; downstream consequences un-derive) | **Complete** — `defeat` un-derives with no re-fixpoint (probe step 10) | A2 impact (S) turns ripple-set assembly into one read; C1 commit-pair diff (M) for per-session deltas | retcon-of-retcon needs E3(a) reinstate (S–M) — v2 split |
| T7 | Timeline/ordering (partial order, unordered-as-verdict, fluent intervals, inertia) | **Complete, richest machinery** — spec 03 surface + `as_of` | nothing | nothing (metric time excluded *by ruling* — tasks must be event/era-shaped, which is the distinctive part) |
| T8 | Alias/identity discipline (declared same-as; systems must honor it) | **Complete** — `assert_same_as`/`assert_predicate_same_as` | B3 efficiency (S) | aliased events/eras refused (DL-0078) — alias tasks target persons/places/things/predicates only |
| T9 | Counterfactual what-if ("if the duke had died…") | Hypothetical-fact forks/workspaces shipped | A1 suppose-false (S, one boolean) for "if the ruling had never landed"; A3 what_if (S) | nothing (flat assumptions only — Bonner PSPACE wall) |
| T5 | Rules adjudication with exceptions (house rules, waivers, which-rule-governs) | add_rule w/ stratified negation + defeat chains shipped | E3(a) reinstate for waiver-later-voided chains (S–M) | engine-computed rule *priority* — constitutionally parked (DL-0054; encode precedence as harness-performed defeat data) |
| T4 | Session-boundary epistemic state ("what did the party know at session k") | **Story-time: complete** (`as_of`, `dli.timeline.knows` per-knower ledgers) | commit-time: C3 time-travel (M) + honestly C0 valuation versioning (L) | v1 asks story-time questions; reconstruct session states by replaying the authoring script (corpus.import/export) |
| T1 | Continuity-violation detection in rendered prose | check battery (16 continuity codes) scores *claims*, never prose — detection is the contestant's job, the engine is the oracle | E2 value domains/incompatibility (M) if value-clash violations are a headline split | prose-reading is nobody's job but the contestant's (koan 3) — v1 plants FD, inertial-overlap, order/anachronism, referential classes only |
| T12 | Harness/scoring infra | corpus import/export, diff, fences, fixtures | D1 batch, D2 schema manifest, B1 scoped valuation, C1 diff — the Move-1/Move-2 items | nothing blocking; every gap has a measured workaround |
| T11 | Campaign-scale cost | per-session recompile is linear and fine at probe scale | G1 census is the cheap measurement to run over generated session commits | only large-N campaigns gate on G-pack |
| T10 | Multi-source canon precedence (core book vs supplement vs table ruling) | — | — | **out of scope v1, stated in the card**: DEF-4 deferred, winner-selection constitutionally refused; source tiers encoded as data, precedence performed by the harness as defeats |

**The headline** [**ASSESSMENT**]: a v1 benchmark built from
**T2 + T6 + T3 + T7 + T8 requires zero engine extensions** — the entire
scoring path is shipped and probe-proven. T9 and T5 each add one S-cost
item (A1; E3a). T1 as a headline split buys E2 (M). T4-at-commit and T10
are the two families that genuinely cannot be scored honestly at 1.12.0.
Composite ranking by shipped-coverage × distinctiveness:
**T2 ≈ T6 > T3 > T7 > T8 > T9 ≈ T5 > T4 > T1**, with T12 as enabler, T11
as measurement, T10 out of scope.

---

## §3 What the literature offers, and the empty cell

### §3.1 Fiction-consistency benchmarks: four ways to build truth, all with measured ceilings

The field constructs ground truth four ways
([ASSESSMENT over ~40 surveyed benchmarks + [DR]]):

1. **Expert human annotation** (NovelQA, DetectiveQA, InfiniteBench,
   CharacterBench, NoCha) — expensive, static, and for *this* error class
   demonstrably weak: ConStory-Bench's validation measured professional
   web-novel writers at **F1 0.281 (17.1% recall)** on 1,000 known
   injected errors ([arXiv:2603.05890](https://arxiv.org/html/2603.05890v1)).
   Human annotation systematically undercounts continuity errors.
2. **Controlled flaw injection** (FlawedFictions,
   [arXiv:2504.11900](https://arxiv.org/abs/2504.11900)) — the strongest
   current trick (known locations → judge-free detection scoring), and its
   results set the difficulty floor: detection near-random at 1,200–4,000
   words; LLM summarization adds >50% and generation >100% more plot
   holes than human originals.
3. **LLM-checker pipelines** (ConStory-Checker F1 0.678; NCP-Bench's
   Gemini auditor with mandatory secondary confirmation; SCORE; FactTrack)
   — load-bearing judges spot-validated at small n, against direct
   measured counter-evidence that LLM evaluators are unreliable in the
   adjacent role-play domain (PersonaEval,
   [arXiv:2508.10014](https://arxiv.org/pdf/2508.10014)).
4. **Programmatic engines** — deterministic, but confined to game
   *mechanics* and planning: TALES/TextWorld/Jericho scores, TextQuests
   checkpoints, ByteSized32-State-Prediction's engine-computed next-state
   quiz ([arXiv:2406.06485](https://arxiv.org/abs/2406.06485)), ACPBench's
   PDDL synthesis. Never lore.

The **[DR]** survey's own conclusions converge from outside: the gap is
"integration" (no benchmark ties long-range story memory + contradiction
detection + stable character behavior in one narrative setting) and
"dynamic story-state tracking" (updating facts "in the right way without
breaking earlier constraints"); the Park et al. 2026 survey it cites calls
for "evidence-grounded oracles and task-conditioned verification." Its
nearest rule-based construction — BOOKAGENT's 16 multi-page visual stories
with 40+ declared rule groups checked programmatically — is story-local,
visual-domain, and revision-free: the nearest neighbor, not an occupant.

### §3.2 The RPG/game-state lane: engine-graded means player-side, so far

Every benchmark with unimpeachable ground truth grades the model as
**player** (task success against a game engine): TALES (122 games;
Claude 3.5 Sonnet 95.5% on synthetic TextWorld vs **9.5% on human-written
Jericho**; Zork1 fails despite likely training exposure — contamination
does not confer competence,
[arXiv:2504.14128](https://arxiv.org/html/2504.14128v3)), TextQuests,
ByteSized32-SP (single-step). The GM side has exactly two occupants:
RPGBench ([arXiv:2502.00595](https://arxiv.org/abs/2502.00595)), where the
LLM is *its own* engine (programmatic checks verify internal validity, no
external canon), and NCP-Bench
([arXiv:2608.08160](https://arxiv.org/html/2608.08160)) — the nearest
published neighbor and the reviewer's first question: 100 movie-derived
environments, extracted-then-expert-reviewed specs, LLM-audited
conformance, ≤100 turns in one continuous session, best model **42%
conflict-free survival at 20 turns**, fact-conflict rates 40–68%. The
best gold-state D&D data (FIREBALL, ~25K Avrae combat sessions with true
engine state) is turn-level with last-5-messages context — it never tests
session memory — and both it and CRD3 are public-web/HF-hosted since
2023, i.e. contamination-compromised as evaluation targets
([ASSESSMENT]; use them only to calibrate narrative register). "Setting
the DC" (UCSD) pairs LLMs with a D&D rules engine against 2,000+ human
players — programmatic, but combat-mechanics scope.

The framing wedge this hands us [**ASSESSMENT**]: score the model as
**keeper of canon**, not player. The practitioner AI-GM lane has already
converged on "LLM narrates, deterministic engine adjudicates" (AI
Dungeon's Memory Bank/Story Cards; ChatRPG's Archivist state tools, whose
N=12 study found agentic state significantly improved perceived
coherence) and evaluates it by vibes — a canon-consistency benchmark with
a deterministic grader names that lane's pain in its own vocabulary.

### §3.3 Defeasibility and retcons: the construction proof and the replacement-semantics twin

**BoardgameQA** ([arXiv:2306.07934](https://arxiv.org/abs/2306.07934),
NeurIPS 2023) is the existence proof that generation-direction
construction works *specifically for defeasible content*: backward story
generation to target depth, conflicts injected at tunable density, labels
{proved, disproved, **unknown**} computed by a defeasible symbolic
solver, proofs tracked at generation. Its measured lessons are our
generator's difficulty knobs for free: accuracy degrades monotonically
with conflict density; Type-2 conflicts (winning requires proving the
defeater's body unprovable) are harder than preference-resolved Type-1;
models predict correct labels with **wrong proofs** — so score the
support chain, which `author.why` makes cheap where BoardgameQA needed
manual verification. BBEH's harder variant shows defeasible conflict
resolution remains unsolved in 2025 (best reasoning model 54.2% overall,
[arXiv:2502.19187](https://arxiv.org/pdf/2502.19187)).

**Knowledge-editing ripple benchmarks are the retcon twin — with the
wrong semantics.** MQuAKE
([arXiv:2305.14795](https://arxiv.org/abs/2305.14795)) and RippleEdits
([arXiv:2307.12976](https://arxiv.org/abs/2307.12976)) formalize
post-edit consequence propagation (parametric editors collapse to 7%
multi-hop accuracy; external-memory approaches — MeLLo, in-context
editing — win, which means a fiction-retcon benchmark should quiz
frozen models against canon-in-context/store, never grade parametric
editing). Their semantics is *replacement*: the old triple ceases to
exist. A fiction retcon **defeats** old canon while the old text remains
as history — "what did session 3 establish?" vs "what is canon now?" vs
"as of session 12, what stood?" exists in no surveyed benchmark and maps
directly onto three-valued standing + defeat records + `as_of`.
RippleEdits' six criteria (especially **Preservation** — unrelated facts
must NOT change, the anti-over-rippling control) port over as the retcon
question taxonomy. And the cautionary tale is precise: MQuAKE-Remastered
found **33–76% of MQuAKE corrupted**, chiefly by conflicting edits
interacting within batches
([ICLR 2025 spotlight](https://openreview.net/forum?id=m9wG6ai2Xk)) —
(a) validate every instance by re-running the engine over committed
inputs (the replayable-judgments law *is* this defense), and (b)
conflicting retcons, which corrupt a replacement-semantics benchmark,
are well-defined *content* under defeat semantics: the twin's failure
mode becomes a question type here.

NoCha ([arXiv:2406.16264](https://arxiv.org/abs/2406.16264)) bounds the
difficulty from the fiction side: human-authored claim pairs over recent
novels put GPT-4o at 55.8% and open-weight models at chance — worst on
speculative fiction with heavy world-building — but it is unscalable
(annotators read each book) and has no ground-truth engine. AbstentionBench
([arXiv:2506.09038](https://arxiv.org/html/2506.09038v1)) supplies the
headline-generator for the abstention split: reasoning fine-tuning
*degrades* abstention ~24%, so frontier reasoning models should
measurably regress on engine-labeled open-canon questions.

### §3.4 The empty cell, stated precisely

No published benchmark combines: **(a)** an independently-computed formal
world model as grader, **(b)** generation-direction fiction surface,
**(c)** retcon/belief-revision with defeat-not-replacement semantics,
**(d)** multi-session campaign horizon with incremental canon accrual,
**(e)** scored abstention-with-witness on genuinely-open canon
[**ASSESSMENT**, triangulated by three independent sweeps + [DR]]. Every
axis has a near-neighbor occupying the *other* cells: TextQuests
(engine + long, but player-side single-episode), ByteSized32-SP
(engine + state quiz, single-step), NCP-Bench (GM-side multi-turn,
annotated truth + LLM grader, one session), RPGBench (GM-side
programmatic, no external canon), LoCoMo/LongMemEval (multi-session,
prose-only), BoardgameQA (defeasible + solver-labeled, one-shot static),
MQuAKE/RippleEdits (ripple taxonomy, replacement semantics, 2–4 hops),
BOOKAGENT (declared rules checked programmatically, story-local visual),
NoCha (long fiction, human-bound). This also sharpens EVALUATION §7B's
kill-condition watch: only an *engine-graded GM-side* entrant counts as
the niche being taken.

---

## §4 The substrate in hand

- **Sessions map onto episodes natively — the engine's strongest fit to
  the TTRPG frame.** Episodes bind production order and story anchor
  separately; `fold()` accumulates per-knower knowledge ledgers with the
  Listener (= the table) as a distinguished production-axis knower
  (`crates/dli-world/src/episode.rs`); order rules + slack compute legal
  reveal windows; `dli.timeline.knows` serves the who-knew-what matrix.
  "What did the party know at session k," anachronism detection, and
  reveal-fairness tasks have engine-computed answers today.
- **Ground truth is essentially free.** Every authoring step returns a
  content-addressed root hash; the FW-8 transcript
  (`fantasy/worlds/tpth/ext-f8/fw8-authoring-session.jsonl`) walks
  add → materialize → promote → **defeat** → export with a root per step —
  a ready-made construction template for retcon items: replay the
  sequence, quiz "what is true / who knows what" at each root.
- **TPtH seeds but cannot carry.** The worldbook is ~588 objects, 223
  distinct predicates of which **184 are singletons**, with **zero
  functional declarations and exactly one inertial predicate**
  (promise-status) — the declared-vocabulary features the benchmark
  exists to test are nearly unexercised in it, and it stays
  skeptic-discounted (EVALUATION §7-F1) because the engine was built
  against it. Its arc-01 trace (12 episodes, 24 open/close promise
  states on the one inertial predicate) is, however, exactly a
  session-horizon obligation ledger — the naturalness anchor. Synthetic
  generated worlds must carry the discriminating load.
- **The generator is the one big build.** Nothing in either repo
  procedurally generates entities, timelines, or retcon schedules —
  `dli-sim` is a practice-battery simulator, not a world evolver. But it
  is the *house pattern to copy*: pure function of (seed, index),
  checkpointed manifests, checked-vs-tested honesty grades; and the
  `arc-01-states.yaml → compile_states → states.dli` path is a working
  template for turning a generated episode trace into an engine-checkable
  corpus.
- **Rendering is half-built and deliberately outside the engine.**
  Context-pack/static-wiki exports emit deterministic, citation-carrying,
  token-budgeted atom renderings with a manifest pinning the exact store
  root — a leak-resistant hidden-ground-truth boundary. The prose
  narration step (formal world → session prose) is new benchmark
  construction work, consumer-owned by design; FIREBALL's
  state-conditioned-narration result and Ludii's rules-to-manual
  generation ([arXiv:2109.09507](https://arxiv.org/pdf/2109.09507))
  de-risk the step itself.
- **A supplementary rules track exists.** The Turning ruleset (29 cards,
  4 layouts, 2 variants) has deterministic, seed-replayable
  legal-move/verdict ground truth via `dli.simulate.start` — "was this
  play legal / which variant removes this layout" quiz families need no
  Ludii provisioning (DEF-5 stays deferred). Small volume; a track, not a
  corpus.

---

## §5 The sketch (what v1 would look like)

**[ASSESSMENT — the whole section; a prereg would harden every choice.]**

**Construction pipeline.** (1) *Generate* a campaign world under a
declared vocabulary — entities, era spine, functional + inertial
predicates, rules with exceptions, planted aliases — via a seeded,
checkpointed generator (dli-sim pattern), difficulty parameterized by
BoardgameQA's validated knobs (conflict density, Type-1/Type-2 defeater
mix, distractor lore, derivation depth). (2) *Evolve* it session by
session: each session is a commit carrying new facts, rulings, and a
scheduled retcon stream (defeats, supersessions, alias declarations).
(3) *Render* each session's prose from the context-pack export of the
visible layer, with multiple narrator styles and held-out phrasings
(BoardgameQA's template discipline). (4) *Quiz* contestants at session
checkpoints; ground truth is the fixpoint at each session's root hash.
(5) *Validate* every instance by replaying the engine over committed
inputs before release (the MQuAKE-Remastered lesson).

**Splits** (mapped to §2): consequence QA (T2); *now vs then vs history*
retcon questions with propagation AND preservation scoring (T3, RippleEdits
taxonomy); open-canon abstention where the correct answer names the
blocking open decision (T6); qualitative-order/anachronism (T7);
declared-alias honoring (T8); which-rule-governs adjudication (T5, v1
without reinstatement chains); optional counterfactual (T9, behind A1) and
planted-violation detection (T1, engine-scoreable classes only).

**Scoring.** Three-valued (certain/possible/dead), never binary; witness
required on abstention items; support-chain scoring on a sampled subset
(the BoardgameQA right-label-wrong-proof lesson); results reported as
consistency-decay curves over session depth (NCP-Bench's survival-curve
presentation), with an item-taxonomy crosswalk to ConStory's five
dimensions and NCP's conflict types for comparability. Difficulty does not
require huge token counts — detection is near-random at 1,200–4,000 words
already — so horizon scales in *facts, sessions, and revision depth*, which
keeps evaluation cheap.

**Contestants.** Engine-neutral by the §7B rule: frozen models with
canon-in-context, RAG, memory products (Mem0/Zep-class), agentic
state-trackers, and DLI-backed agents as *one* contestant among them —
with the open deterministic scorer standing apart from every contestant
(the MnemeBrain vendor-benchmark caution). The ChatRPG result predicts
tool-augmented systems beat bare models; publishing that spread is the
benchmark's point.

**The renderer-fidelity workstream (the new validity threat).** A
faithfulness audit must show the rendered prose entails exactly the facts
the checkpoint exposes — no more (leakage), no less (starvation), no
contradiction (renderer error). Design: two-sided checks in the spirit of
the craft layer's outline-conformance discipline
(`docs/spec/06-craft-and-payloads.md`), a re-extraction audit sample
(Shadow-Loom's miracle-step loop), and human spot-audit at release-gate
scale only. This is where LLM/human checking legitimately remains — on
the *corpus construction* side, never in contestant scoring. The three
standing preflight traps apply in full (competent baseline; arms
demonstrably differ; scoring never routes through the treatment's own
output).

---

## §6 What it pulls from the extensions menu

| When | Items | Why |
|---|---|---|
| v1 core | *(none required)* | T2/T6/T3/T7/T8 score on shipped ops |
| v1 harness | D1 batch add (S–M), D2 declare_schema (S), B1 scoped valuation (S), D3 ergonomics (S) | the Move-1 items; the generator/harness is exactly the demand instrument EXTENSIONS priced them for |
| v1 scoring | C1 commit-pair standing diff (M) → C2 changefeed (S) | per-session belief-state deltas; also what ClawArena/MemSyco need — double-pays |
| v1 nice | A2 impact (S), A1 suppose-false (S), E4(a/b) min_why + repair sets (S–S–M) | one-read ripple ground truth; counterfactual split; abstention-with-repair |
| v2 gated | E3(a) reinstate (S–M) → retcon-of-retcon split; E2 value domains (M) → value-clash violations as headline; E4(c) (M–L) → never-derivable why-not | each named in the benchmark card as a versioned split, not silently absent |
| honesty-gated | C3 time-travel (M) + C0 valuation versioning (L) → commit-time epistemic questions | v1 sidesteps via story-time questions + replay-to-fresh-store |
| measurement | G1 monotone-commit census over generated session commits | prices G1 and characterizes the benchmark's own cost curve in one move |

NCP-Bench's commitment taxonomy (invariants / ordering constraints /
achievement goals) independently maps onto E1/E2/E3/E4 — external
confirmation that benchmark authoring pulls Pack E forward, exactly as
EXTENSIONS Move-3 anticipated.

---

## §7 Costs, risks, kill conditions, and where this sits

**Relation to Experiment 1 — the load-bearing nuance.** The benchmark
*artifact* (generator, corpus, grader) does not depend on Experiment 1:
ground truth is authored-then-computed, no lowering involved. What
Experiment 1 forecasts is **DLI's own competitiveness as a contestant** —
a DLI-backed agent must lower session prose into the store under the
supplied vocabulary, which is precisely the vocabulary-on arm. So the
sequencing stands: Experiment 1 first; this doc changes nothing there. If
Experiment 1 clears, the fiction frame is a strong candidate for B's
headline domain; if it fails, the benchmark survives as
B-as-negative-result infrastructure (it still measures the field's gap,
and the abstention/retcon results generate findings with or without a
competitive DLI entry).

**Honest costs.** The two real builds are the **world/episode generator**
(new, but pattern-anchored) and the **renderer + fidelity audit** (new
validity surface, the direction's biggest research risk). Everything else
is harness assembly over shipped ops plus S/M menu items. TPtH
seed-authoring (declaring vocabulary over it) is Experiment-1-shaped work
and inherits its discount.

**Risks.** (1) Renderer circularity — mitigated but not eliminated by
§5's audit; if faithful rendering at scale proves unreliable, the
benchmark degrades to formal-surface questions (still novel via retcon +
abstention axes, but far less fiction-flavored). (2) Vendor-benchmark
skepticism — engine-neutral adapters, open deterministic scorer, DLI as
one contestant, preregistered. (3) The crowding watch — the long-horizon
prose-memory lane crowds monthly; the sharpened filter is that only an
engine-graded GM-side entrant occupies this cell (NCP-Bench does not).
(4) Fiction framing may read as unserious to some reviewers —
counter-positioning: the identical machinery is the declared-vocabulary
consequence benchmark of §7B; fiction is the *demonstration domain* where
the empty cell is sharpest, with a regulated-vertical domain as the
co-equal second corpus.

**Kill conditions.** A credible third-party benchmark covering (a)+(c) or
(a)+(d) of §3.4's cell appears (watch the wave monthly); the renderer
fidelity audit cannot reach a preregistered faithfulness bar; or
Experiment 1 fails *and* no external demand for the artifact
materializes — in which case the generator work is salvaged as the
worked-scenario/teaching corpus and the direction closes.

**Verdict [ASSESSMENT].** This is a *specialization of direction B*, not a
new direction: the strongest concrete candidate for B's shape when B's
turn comes in the standing sequence (after Experiment 1, alongside H). Its
distinct contribution to the decision record is threefold: the empty cell
is confirmed sharpest in the fiction/GM frame; the v1 core needs zero
engine extensions; and the two genuinely new workstreams (generator,
renderer audit) are now named with their prior art. Recommended next
concrete step *if and when pursued*: a one-week generator spike (seeded
world + 3 sessions + 2 retcons + rendered prose + 20 engine-graded
questions end-to-end) and a prereg that fixes the renderer-fidelity bar
before any paid run.

---

*Housekeeping noted in passing: `CLAUDE.md`'s agent-interface section
still says 64 ops at 1.9.0 (the surface is 68 at 1.12.0 per
`crates/dli-serve/src/ops.rs:248`), and spec 11 §3.18's branch-writes
sentence remains stale (already flagged as EXTENSIONS D3). Both are doc
debts for whoever next touches those files, not this doc's scope.*
