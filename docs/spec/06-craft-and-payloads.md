---
status: live
verdict: "Structured objects go to the kernel; prose stays outside and re-enters only as attestations or payload fields."
title: "Craft rules and the fat-payload pipeline"
---

# Craft and payloads

The craft layer answers the owner's ask for lots of deterministic reasoning at
the outline level. The consuming repo's outline checker is already a pure
structural prover over typed data — the closest thing in that codebase to a
kernel rule set — and its natural home is the fabric's narrative layer. The two
episode schemas merge there; craft becomes rules-as-data over the merged
objects, sharing one evaluator with the story-time order-rules of
[03-time](03-time.md). What changes is not the doctrine but the horizon: the
legacy checker sees one outline, the kernel sees the whole season ledger.

The split that governs the layer, stated once: **structured objects — outlines,
payloads — go to the kernel; prose goes to factory-side checks and judges, whose
outputs re-enter the kernel only as attestations or payload fields.**

## 1. The merged episode schema

`EpisodeOutline` and `EpisodeState` unify into one schema. Promises, beats,
scenes, sliders, and knowledge become first-class objects carrying both a
**story-time anchor** and a **production anchor** ([03-time](03-time.md)), and
they are visible **season-wide** rather than one outline at a time:

- **Promises** — setup/payoff obligations to the listener, on the production
  axis, with kind and an opening quote.
- **Beats** — tries, fails, and accomplishments, each with a `thread` identity
  and a `magnitude`, anchored to a scene and a production slot.
- **Scenes** — cast, presence (including off-mic), location, and phase
  observations.
- **Sliders** — per-character competence and disposition, read as states on
  the production axis.
- **Knowledge** — who knows what, initiated at the episode's story position,
  with the listener as a distinguished knower on the production axis.

Because beats and promises are season-wide ledger rows rather than
outline-local fields, a rule may join across episodes — the move that makes §3's
checks possible at all.

## 2. The port inventory

The entire legacy outline inventory ports with its violation codes intact. The
old checker serves as the **parity oracle** during the port — every ported rule
must reproduce its verdict on the outline fixture corpus — and then retires,
exactly as the continuity checker does for the world checks. Ported unchanged:

- promise **payoff-XOR-carry**, `promise-made-in-unknown`, `dangling-payoff`,
  and the season promise ledger;
- **MICE LIFO** nesting (threads close in reverse of their opening order);
- **try/fail typing** with `yes-and` terminal;
- character **sliders and compensation**;
- scene **goal/conflict/disaster** chains;
- **cast-over-four** (no scene carries more than four active speakers);
- the **audio-plan policy family**, porting as *policy leaf tables* in the
  board-game-variant style — substitutable data, not code;
- the **material-assumptions** provenance ledger;
- **loop open/close** discipline;
- **hook-late**.

Two ports change status rather than behavior. The `ten-reads-as-battle` regex
ports as a lexicon-table rule (its combat wordlist becomes data) and is
**demoted to a backstop** by the structural arc-shape rule in §3 — the regex no
longer decides arc shape, it only catches what the structural rule misses. The
magic-pedagogy prerequisite DAG becomes **kernel facts plus rules over order**:
a working taught before its prerequisite is an ordering violation on the story
axis, the same machinery as every other `rule … over order`.

## 3. Season-ledger rules

The merge makes newly checkable exactly the questions the one-outline checker
could not answer. Take the owner's example, worked: *is this large
accomplishment preceded by one or more previous failures?* Today unanswerable,
because the failures live two episodes back and the checker sees one outline. In
the kernel, beats are season-ledger rows with thread identities and production
anchors, and the rule is a join.

- **Accomplishment-earned.** An accomplishment beat of magnitude ≥ major
  requires at least one prior beat on the **same thread** with outcome in
  {yes-but, no-but, no-and}, anywhere earlier on the production axis — an
  **error** if absent. A climax-class accomplishment requires **two** — a
  **warn** if only one. `yes-and` becomes terminal *per thread across the
  season*, not per outline: once a thread lands its unqualified win, it may not
  re-open for another.

- **Competence-consistency.** Sliders are states on the production axis. A
  major unassisted accomplishment by a character whose competence as of that
  episode is below threshold requires an intervening growth, aid, or price beat
  — a **warn**. The legacy flat-slider check generalizes to arc-flat
  trajectories, and compensation rules apply at season scope.

- **Reveal pacing against the listener ledger.** The listener is a distinguished
  knower on the production axis ([03-time](03-time.md)), which makes fairness
  *checkable*. `payoff-after-reveal` generalizes; a kishōtenketsu turn's
  reframing fact must sit on the listener ledger at least N episodes before the
  turn — fair-play as a rule, not a review note. An open mystery promise
  untouched for K consecutive episodes is **starved**. Dramatic-irony spans
  (listener-knows ∧ character-does-not) become queryable and budgetable.

- **Marketability and clarity.** Hook ≤ 60s ports as-is. Cold-open discipline:
  a cold open must open a loop, carry cast ≤ 2, and hold no deep-time in act 1
  (a join with the `no-deep-time-act-one` rule over order). Cast-introduction rate:
  new named speakers per episode inside a band, against a season named-cast
  budget. **Scene-scoped aural distinctness**: join the naming layer's
  confusability metric ([05-language-layer](05-language-layer.md)) with scene
  cast lists — two names below the distinctness floor may coexist in the series
  but not share a scene. The global distinctness check stays the oracle's; the
  scene-scoped one is a join only a kernel holding both naming data and cast
  lists can run. Promise-density bands by season position (opens exceed closes
  early, the reverse late; a finale carries only declared carries). Arc-shape
  conformance: an episode labeled kishōtenketsu must place its *ten* in the
  declared band, and the *ten* must not be a conflict-resolution beat —
  structural, replacing the regex as primary. Audio-load trajectory: no N
  consecutive high-orientation-load episodes.

## 4. Thresholds are policy data

Every threshold in §3 — the magnitude scale, the distinctness floor, the density
bands, the constants N and K — is **policy data**, not code. A project may supply
calibrated leaf tables and citation-bearing severity overrides; uncalibrated
generic tables remain warning-grade. Tuning a threshold is a caller-config diff,
never a checker change: the rule that reads the table is stable.

The same ownership rule covers finding citations, narrator dispositions,
provenance formats, and material categories. Generic defaults contain no project
decision IDs, apply no project-specific narrator ban, recognize no project-local
decision/reference convention, and require no project material taxonomy.
`finding-policy.toml` is a closed map over emitted `outline.*` and `craft.*`
codes: unknown or duplicate codes make the craft config invalid.

Doctrine selection controls evaluation, not only explanation. Each shipped
principle declares the exact `outline.*` and `craft.*` finding codes it governs.
A loaded project config enables only codes governed by its selected packs (plus
its explicitly configured lexical rules); an empty selection emits no craft
verdicts. The unconfigured library constants retain the all-rules mechanism for
callers that deliberately use the bare checker without project config.

## 5. The knowledge base as policy data

The knowledge base ports as `principle` declarations with every field intact —
`{id, family, statement, source, mission_filter, tension_note, resolution,
phases, checkable, check_ref}` — **107 principles** in the pinned corpus (the
doctrine's round-number *79* names the documented subset; the table, not the
number, is the source of truth). Its invariants stop being test assertions and
become **compile rules**:

- a `tension` requires a `resolution`;
- a `checkable` principle that binds a nonexistent rule is a **compile error** —
  the legacy `check_ref_resolves` test becomes the reference checker.

Bindings follow the principle's checkability:

- `deterministic` → `-> rule <name>`;
- `judge` → `-> attest <question-kind>` — the principle's check consumes stored
  attestations and reports grade `attested`
  ([07-verdicts-and-llm-boundary](07-verdicts-and-llm-boundary.md) §3);
- `none` → declaration-only, surfaced by brief- and phase-queries, never a
  verdict.

Every craft finding cites its principle; every principle cites its source and
its mission-filter resolution. `deloreis craft why <finding>` renders the
statement, the tension, and the resolution in place — and the **pillars win**
(d-0001) where a mission-specific pillar overrides generic advice. Coverage
becomes a query: the principles whose checkability is `none` or `judge` are the
standing **checkable-debt report** — the roadmap of rules not yet written, kept
honest rather than hidden.

Composition retains caller provenance citations, merges only declared parameter
overrides with doctrine defaults, and returns the applied precedence and
factory/judge registries beside the principle corpus. Empty, duplicate,
undeclared, type-changing, or unsupported records fail validation rather than
being silently ignored. Factory namespaces and material-category identifiers are
open caller-owned identifiers; bindings must resolve through the supplied
registries.

Grade rollup applies to craft like everything else: an "episode passes craft"
claim that includes judge dimensions is honestly `attested` overall. Judge
outcomes use the stable codes `craft.judge-attested` and
`craft.attestation-missing`; the caller-owned principle id remains in the
finding message rather than becoming an unbounded code suffix, and
`deloreis report --min-grade` filters to what rides on stronger evidence.

```dli
# grammar: v0 (unfrozen; AST normative)
scope craft

principle tryfail-before-triumph  family plot  mission pass
  source sanderson  cites d-0001
  checkable deterministic -> accomplishment-earned

rule accomplishment-earned over season severity error
  : beat $b kind accomplishment magnitude >= major
    requires beat $f  thread($f) = thread($b),
             outcome($f) in {yes-but, no-but, no-and},
             produced-before($f, $b)
  # season-wide join over the beat ledger; the old checker saw one outline

principle register-fit  family prose  mission adapted  cites d-0002
  tension "generic 'strong voice' advice; the chronicle register wins"
  checkable judge -> attest register-fit
  # consuming checks report grade attested (07-verdicts-and-llm-boundary §3)

rule scene-name-clash over season severity error
  : scene $s casts $a, $b  where $a != $b
    requires aural-distance($a, $b) >= floor(scene)
  # joins the naming layer's confusability table with scene cast lists —
  # only checkable where name vetting and casts share a kernel
```

## 6. What does not move

The prose-text checks stay factory-side. The 23 checks that read prose —
said-ratio, abstraction density, SFX-cue heuristics, the stylometry delegations
— need the style parser and remain factory-invoked. The style lint and the
exemplar-anchored judge stay. LLM judges remain judges.

But their verdicts can be recorded as **attestations**, and then season-level
rules reason over them deterministically. "No episode ships below attested
register-fit 3.5" is a kernel rule over attestation data
([07](07-verdicts-and-llm-boundary.md) §3): the judgment is made once, in the
factory, by a model; the *reasoning over* the judgment is deterministic and
replayable in the kernel. The line, once more: structured objects go to the
kernel; prose goes to factory checks and factory judges, whose outputs re-enter
the kernel only as attestations or payload fields.

## 7. The fat-payload pipeline

A draft is prose; prose never enters the kernel. What enters is the draft's
**structured shadow** — the payload — and the shadow is replayable evidence. The
flow: a draft lands → a factory extraction job produces the payload → `deloreis
check-draft <payload>` runs the full battery → one merged typed report. The
extraction job is **encodings-class** ([07](07-verdicts-and-llm-boundary.md) §5):
schema-constrained, fixtured, and quote-anchored, produced by a factory-side
model; agents that author lore from the draft do so through the tool surface
([11-agent-authoring](11-agent-authoring.md)), never by hand-writing the shadow.

### 7.1 The payload

```yaml
# episodes/s01/episode-01.payload.yaml — the draft's structured shadow
meta:
  episode: e01
  source: {file: episode-01.md, sha256: "9f3a…", words: 5210}
  extracted: 2026-07-19
  extractors:
    schema: payload/v1
    fields:
      beats:  {model: claude-fable-5, prompt: "sha256:b41c…"}
      claims: {model: claude-fable-5, prompt: "sha256:77e0…"}
claims:                                # factcheck atoms
  - {atom: "knows(merlin, true-name(the-enchanter))",
     quote: "he knew the old name, and the shape of its saying", line: 112}
deltas:                                # knowledge / possession / location / injury
  - {kind: knowledge, who: arthur, fact: names-bind-power, scene: s04, line: 129}
scenes:
  - {id: s04, cast: [merlin, arthur], location: camelot,
     phases: {season: spring}, off_mic: [morgan]}
beats:
  - {id: b7, scene: s04, kind: accomplishment, thread: speak-the-name,
     magnitude: minor, by: merlin, assisted: false,
     quote: "the word held", line: 131}
promises:
  opened: [{id: p-sword, kind: mystery, quote: "something waited in the stone…", line: 44}]
  closed: []
proof_ledgers:
  - scene: s04
    ledger:
      premises: ['premise p1 knows(merlin, true-name(the-enchanter)) cited d-1 cue "line 112"']
      steps: []
      close: {atom: "knows(merlin, true-name(the-enchanter))", expect: holds}
names: {introduced: []}
lexicon: {banned_hits: [], register_tokens: 61}   # deterministic scans ride along
```

The top-level keys are `meta` (source hash and extractor manifest), `claims`,
`deltas`, `scenes` (cast, presence, location, phase observations), `beats`
(tries/fails/accomplishments with threads and magnitudes), `promises`,
`proof_ledgers`, `names`, and `lexicon`. The lexicon and banned-vocabulary rows
are **deterministic scans, no model involved** — they ride in the payload so the
report is one artifact, but they are not encodings and carry no extractor.
Proof `premises` and `steps` are the canonical interior clause lines of the
declared `proof <episode>-<scene>` (or `proof <scene>`) block. The payload shadow
must match that declaration before the kernel replays it; an invented or stale
shadow fails closed.

Payload metadata is part of acceptance, not commentary. The episode and source
file are non-empty, the extraction date is a real calendar date, the source
word count is recomputed from the draft, and the schema is exactly
`payload/v1`. Every non-empty model-extracted field carries a non-empty model
name and a full `sha256:` prompt digest. Deterministic register-token counts may
not exceed the draft's lexical-token count. Delta subjects resolve to a
declared entity, the listener, or a name introduced by the payload.

### 7.2 The battery

The kernel runs everything it has against the one artifact — eight check
families into one merged report:

1. **Continuity** — six classes over claims, plus structural and referential
   validation of scene-keyed state deltas.
2. **Factcheck** — claims resolve to supported / contradicted / unsupported with
   D-citations; rejected-lore constraints enforced. A killed token rejects its
   appearance globally, while a contextual rejected claim rejects only the exact
   predicate/subject/value slot named by its ruling.
3. **Outline-conformance** — §7.3.
4. **Craft rules** — §3's battery, with the payload's beats merged into the
   season ledger.
5. **Proof checks** — each shadow must match a project declaration, then that
   declaration runs through the practice checker with the payload scene's phase
   projection ([04-practices](04-practices.md)).
6. **Naming** — introduced names must be oracle-vetted or hypothesis-carried;
   `scene-name-clash` over the payload's casts
   ([05-language-layer](05-language-layer.md)).
7. **Phase walks** — observed phases resolve to declared cycles and spokes;
   adjacent observed scenes may retain a spoke or advance by one successor
   ([03-time](03-time.md)).
8. **Lexicon verdicts** — the kernel independently scans the source against the
   compiled ban list and rejects both real hits and a mismatched carried scan.

Every finding carries its rule/check id and grade; findings that turn on an
anchored extraction also carry the draft line and verbatim quote, while
scene-level structural findings name their scene. Craft findings carry their
principle citation when one is configured. The payload commits with the
episode; the report is derived and regenerated by the gate.

The worker loads optional reviewed `outlines`, a prior `craft_ledger`, and a
caller-owned `craft_config` directory from the project document entry. The CLI
accepts the same inputs through `--outlines`, `--craft-ledger`, and
`--craft-config`. Their absence disables only the checks that require those
optional inputs or selects the generic builtin craft corpus; supplied assets
are always consumed. The config composes enabled principles and overlays.
`dli.check-draft` additionally accepts optional caller-owned `continuity`
context. Its `established`, `changes`, and `references` rows describe the current
episode state; `prior_facts`, `prior_open_promises`, and `closed_promises`
describe the already-folded predecessor ledger and the closures to validate.
This context runs the cross-episode continuity classes but is deliberately not a
top-level `payload/v1` field and is never committed by `dli.payload-submit`: the
payload is immutable draft evidence, while the prior ledger is runtime state.
An explicit change compares its `from_value` with the prior slot, so a legitimate
transition to a new value is not misclassified as a reversal.
Payload checking consumes its season thresholds plus finding citations and
severity overrides. The same loaded value exposes narrator policy, provenance
formats, material categories, and outline policy to callers of the
outline-checking library API; those fields do not silently imply a payload check
that has no reviewed outline input. Loaded policy owns its data, so repeated
library/project loads do not accumulate leaked config allocations. Config file
names, fields, overlays, and finding-code rows are closed and fail on unknown or
duplicate entries.

The craft-config directory is a closed interface of eleven TOML file shapes.
Each accepted filename has its own versioned JSON Schema under
`schemas/craft/`, derived from the exact serde type `load_config` consumes.
`cargo run -p xtask -- schema --check` validates the committed example and TPtH
TOML files against those per-file schemas as well as checking schema drift.
`craft-config-v1.schema.json` remains the aggregate `CraftConfig` library shape;
it is not the schema for any one TOML file.

### 7.3 Two-sided outline-conformance

Extractors are encodings-class and quote-anchored, so an extractor cannot invent
what it reports; but an extractor can **miss** — completeness is not provable.
The mitigation is structural: outline-conformance is a **two-sided diff**, and
each finding states both readings so neither is silently assumed.

- The outline promised beat *b* and the payload lacks it → **`beat-unaccounted`**:
  *either the draft dropped the beat or extraction missed it* — both readings
  stated, both deserving a look.
- The payload contains a beat the outline never declared → **`beat-undeclared`**:
  *draft drift, or a stale outline.*

The same two-sided diff runs over promises and scene casts. The current
`EpisodeOutline` schema carries no proof-cue list, so proof conformance instead
uses the declared proof block as the second side of that comparison. A
silent extraction miss is thereby converted into a visible conformance finding
everywhere the outline made a promise — and the outline is exactly the artifact
the author already reviews. What remains uncovered is content neither promised
nor extracted, invisible to both sides; the backstop is the periodic
**re-extraction sweep** — a new extractor version re-runs all drafts and the
payload diffs are reviewed, cheap because payloads diff like code.

### 7.4 Anchoring and staleness

**Quote anchoring is a hard error.** Every extracted item carries a verbatim
quote and a line; the kernel mechanically verifies the quote appears at the
claimed position. A quote that does not appear verbatim at the claimed line
fails the check — so the extractor cannot hallucinate anchors, and every
downstream finding renders real text from the real draft.

**Staleness is by source hash.** The payload's `meta.source.sha256` pins the
draft it shadows. Editing the draft raises a **staleness finding** — the same
family as the propagation staleness linter — and queues the payload for
re-extraction. A payload can never silently describe a draft that has moved
underneath it.

### 7.5 The performance claim, as engineering envelope

Extraction costs seconds and cents, factory-side, once per draft revision. The
battery is pure evaluation over the compiled world plus one payload: at canon
scale it runs in single-digit milliseconds per draft, and a full-season re-check
sits well under a second — so the battery lives in the CI gate.

The consequence that matters more than the number: **when a ruling lands, every
committed payload re-checks for free.** A new D-entry that breaks episode 3's
proof ledger or contradicts episode 5's claim surfaces in the next
`deloreis check`, not in an author reread. That is fast deterministic reasoning
delivered at both altitudes — outline objects natively, drafts via their shadows
— with a model needed exactly **once per draft revision, never once per
question**. The committed evidence record therefore retains both the structured
payload and the exact source prose used to verify its anchors and source hash;
`dli.check` verifies and replays those records rather than relying on process
memory or caller resubmission.
