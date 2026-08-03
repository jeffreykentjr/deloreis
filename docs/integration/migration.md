---
status: live
verdict: "The plan for re-homing lore from fantasy into deloreis: scope and assumptions, the re-home work list, the shadow-read harness, three-phase source migration ending in deletion, the per-check flip rule, the STATUS handshake, coordination points CP-1..CP-10, ownership, and the worked TPtH tool-surface scenario."
---

# Migration: fantasy → deloreis

> **A worked integration: the fantasy factory.** This migration is one worked
> consumer's path onto the generic `dli-worker` contract — how the fantasy factory
> re-homes its lore into a `deloreis` kernel. The shadow-read harness, the
> three-phase source migration, and the per-check flip rule are the general shape
> any consumer follows; the fantasy specifics (its Python gates, its YAML sources)
> are this instance.

How lore moves from the fantasy repository's Python gates and YAML sources into
the De Lore Is kernel — safely, one check at a time, with the legacy oracle
standing beside the kernel until the kernel has earned the check.

## 1. Parties, scope, assumptions

**Parties.** deloreis (the kernel) and fantasy (the factory and its canon).

**Scope.** The reasoning that today lives in `src/fantasy/` Python gates and
`canon/data/*.yaml` sources re-homes into the kernel; prose canon, LLM calls,
orchestration, and rendering stay fantasy-side.

**Assumptions.** The migration is sound only while these hold:

- **A1.** `canon/decisions.md` remains the authoritative record of rulings. The
  kernel's ledger mirror indexes it; it never supersedes it.
- **A2.** The factory keeps every LLM call, all orchestration, and all
  rendering; the kernel stays offline and deterministic.
- **A3.** The legacy Python gates (`fantasy validate`, factcheck, continuity,
  `name vet`, `outline`) are pinnable and runnable as reference oracles
  throughout shadow-read.
- **A4.** The author is the single valuation authority; migration introduces no
  multi-authority valuation.

## 2. The re-home work list

Each capability names where it moves, what stays fantasy-side, and the oracle
that proves parity during the move.

| Capability | Moves to | Stays fantasy-side | Parity oracle |
|---|---|---|---|
| Validator core (assertability, status derivation) | CORE-2 / FRONT-1 | The prose V1–V5 markers policing markdown | `fantasy validate` |
| Continuity + factcheck | CHECK-1 | LLM extraction (encodings-class, never above `tested`) | `continuity.py` / factcheck |
| Context-pack builder | EXP-2 (the first export backend) | — | today's context-pack output |
| Naming world-half | LANG-1 / LANG-2 | External screens (ai-tics, franchise/pop collisions, English homophones, anachronism, portent, deck-figure) + LLM generation | `fantasy name vet` (kernel-half subset) |
| `ratified_names.yaml` mirror | Deleted; generated view (LANG-2) | — | the generated view equals the old mirror on the ratified names |
| Lexicon + banned vocabulary | LANG-1 (in-runtime, cited) | The prose scanner (consumes the compiled list) | the banned-vocabulary scan over the compiled list |
| Outline structural checks | CRAFT-1 | The 23 prose-text checks (`check.py`) | `outline.py` violation codes |
| Craft KB | CRAFT-3 | LLM judges (verdicts re-enter as attestations) | `config.py` invariants, now compile rules |
| Era precedence | TIME-1 / TIME-3 | — (Dialog emission becomes a consumer) | today's era-precedence answers |
| Magic / Concordance | PRAC-1 / PRAC-2 | Prose `magic-system.md` (status markers) | the spoken-proof scene set |
| Draft payloads | PAY-1 / PAY-2 / PAY-3 | The factory extractors (encodings-class) | the two-sided outline-conformance diff |

## 3. The shadow-read harness

The harness runs the legacy oracle and the kernel over the same input and
compares. Its three inputs are fantasy-provided:

- **Pinned oracles** (EXT-F1) — the legacy gates at a fixed SHA.
- **Vendored corpus snapshots** (EXT-F2) — canon at a pinned commit.
- **The mutation detection corpus** (EXT-F5) — seeded divergent cases.

**Detection is proven separately.** A harness that only ever reports agreement is
a null oracle. Before any parity run counts toward a flip, the harness must be
shown to **catch** the seeded divergences in the detection corpus. Parity means
nothing until detection is demonstrated.

**Triage classes** (every disagreement is assigned one, with an owner):

| Class | Meaning | Owner |
|---|---|---|
| `parity-match` | The two agree. | — (no action) |
| `known-divergence` | The kernel is intentionally better (e.g. `offset` → honest `unordered`). | deloreis — documented as expected |
| `kernel-bug` | The kernel is wrong. | deloreis — fix forward |
| `oracle-bug` | The legacy gate is wrong; the kernel is right. | fantasy — noted; the kernel never regresses to match a legacy bug |
| `corpus-gap` | Neither side has the data. | fantasy — add a fixture |

## 4. Three-phase source migration

`facts.yaml`, `timeline.yaml`, and `characters.yaml` each migrate in three
phases:

1. **Read-through.** The kernel reads the YAML directly. Each row's stored
   `status:` is cross-checked against what the kernel derives from `decisions:`,
   and any disagreement is a **finding**, never a silent fix.
2. **Dual-run.** The YAML stays the source; the shadow-read harness proves
   parity (§3, §5).
3. **Conversion and deletion.** At the grammar gate, a one-shot conversion emits
   `lore/*.dli`, the diff is reviewed, and the YAML source is **deleted** — not
   kept as a generated mirror, because two writable representations of the same
   facts is how drift starts.

Specifics:

- **`timeline.yaml`.** The `eras` list becomes the ordered spine. `offset` dies:
  offsets compile to chains of `before`-facts annotated with the adjacent events'
  citations. The act-eras leave the chronological spine for the narrative layer.
  The **offset→before migration and the act-era split ship as one batch ruling
  request**, so the intra-era order becomes something the author has actually
  ruled.
- **`characters.yaml`.** The registry rows migrate as above; the prose stays in
  `canon/characters/*.md`. The validator's bidirectional-sync check survives as:
  every name or alias the prose uses must resolve to a declared entity.

## 5. Dual-run and the per-check flip

A check runs in **dual-run** (oracle-authoritative, kernel shadowing) until it
earns authority. A check **flips** to kernel-authoritative only when all three
hold:

- **20 consecutive clean dual-runs** over the vendored corpus.
- **At least one real ruling propagated** through that check under dual-run —
  the check has been exercised by an actual canon change, not only by the frozen
  snapshot.
- **Detection parity** — the harness catches the check's seeded divergences.

**Rollback.** Any post-flip divergence rolls the check back to dual-run; the flip
is re-earned from zero.

## 6. The STATUS handshake

Coordination runs on **paired single-writer STATUS files**: this repo's
`docs/integration/STATUS.md` (written only by deloreis) and a mirror on the
fantasy side (written only by fantasy). Each side writes only its own file; the
pair, read together, is the handshake. A coordination point moves through the
states `proposed → provided → acknowledged → verified → closed`.

## 7. Coordination points

| CP | deloreis provides | fantasy provides | Author-blocked |
|---|---|---|---|
| **CP-1** — contract freeze | v0 schemas + the closed fixture registry | the vendored copy + a parity run | no |
| **CP-2** — pinned oracles | the shadow-read harness slot | the pinned legacy gates (EXT-F1) | no |
| **CP-3** — vendored corpus | the dual-run consumer | canon snapshots at a pinned commit (EXT-F2) | no |
| **CP-4** — detection corpus | detection proven separately | the seeded divergence corpus (EXT-F5) | no |
| **CP-5** — core shadow-read | dual-run parity (validator/continuity/factcheck) | the pinned oracles + corpus | no |
| **CP-6** — naming shadow-read | the LANG-2 kernel-half view | the legacy `name vet` + external screens | no |
| **CP-7** — the batch RQ | the offset→before compiler + the draft RQ | — | **yes** — the ruling |
| **CP-8** — source deletion | the conversion + the reviewed diff | the review + the deletion commit | no |
| **CP-9** — worker on the contract | `deloreis serve` + a tagged build | the fantasy worker on the vendored contract (EXT-F3) | no |
| **CP-10** — authoring-tools handshake | a tagged build with the `author.*` tools + the worked-scenario fixture | their agent exercises the scenario against the tagged build and posts the transcript (EXT-F4) | no |

**CP-10** is verified when the transcript matches the fixture's semantics.

## 8. Ownership of the three crossing artifacts

Three artifacts live in both repos and need a single owner each:

- **The contract.** deloreis owns it; fantasy vendors it pinned and
  digest-checked (`decisions.md` DL-0010).
- **The vendored corpus snapshots.** fantasy owns the canon source; deloreis owns
  the pinned snapshot in its own test tree.
- **The worked-scenario fixture.** deloreis owns the fixture; fantasy owns the
  transcript that must match it.

## 9. If an assumption fails

- **A1 fails** (decisions.md is no longer authoritative) — the mirror would
  become a second source of rulings. Halt: this violates the ledger-outranks
  rule; the mirror is re-derived from the ledger before anything proceeds.
- **A2 fails** (an LLM call is wanted inside the kernel) — the offline and
  deterministic guarantee breaks. Refused: the call stays factory-side and its
  output enters as one of the four declared classes.
- **A3 fails** (oracles cannot be pinned or run) — shadow-read has no reference,
  so **no check can flip**; affected checks stay in dual-run until the oracle is
  restored.
- **A4 fails** (multi-authority valuation is needed) — out of v1 scope
  (`PLAN.md` DEF-4). Migration does not attempt it; the single-authority limit is
  documented and the need is filed.

## 10. The worked scenario

The fantasy agent needs a new secondary character for episode 9. It authors
through the tool surface, gets graded findings at every step, and commits the materialized
`.dli` through fantasy review. Promotion to ratified canon happens later, via a
ruling request, if the character earns it. Rendered strings below are the
placeholder canonical surface (grammar unfrozen; the AST is normative —
`docs/spec/01-syntax.md`). Calls are shown in MCP `tool`/`args` framing with
the `workspace` arg and the worker frame fields (`dli`, `id`) elided for
brevity; the normative wire shape is the contract §2 and the generated
schemas.

**Open a workspace.**

```json transcript
→ {"tool": "author.open", "args": {"fork": "ep9-cast"}}
← {"workspace": "ep9-cast", "workspace_digest": "jcs:sha256:0a2c…"}
```

**Add the character (proposed).** The tool returns a graded finding with repair
data — the name is not yet vetted — and the rendered canonical line.

```json transcript
→ {"tool": "author.add_entity",
   "args": {"kind": "person", "id": "tegwen", "name": "Tegwen",
            "provenance": {"kind": "proposed", "origin": "agent:factory/ep9-cast"}}}
← {"findings": [
     {"grade": "asserted", "severity": "warn", "code": "name.unvetted",
      "message": "Tegwen has not been vetted against the name-space; the factory external screen (ai-tics, collisions, homophones) is a separate gate.",
      "repair": {"kind": "run-check", "op": "dli.name.check",
                 "params": {"name": "tegwen", "register": "cymric-fringe"}}}],
   "rendered": ["person tegwen \"Tegwen\"  proposed  agent:factory/ep9-cast"],
   "workspace_digest": "jcs:sha256:6b91…"}
```

**Vet the name (kernel half).**

```json transcript
→ {"op": "dli.name.check", "params": {"name": "tegwen", "register": "cymric-fringe"}}
← {"ok": true, "findings": []}
```

An empty findings list is the world-true pass (uniqueness, aural distinctness,
register fit, sayability). The factory's external screen — ai-tics, collisions,
homophones — is a separate, factory-side gate.

**Add a fact and a relation.** Each returns its own findings, rendered line, and
digest; the location is a real TPtH place (`gwern`).

```json transcript
→ {"tool": "author.add_fact",
   "args": {"entity": "tegwen", "key": "trade", "value": "net-mender",
            "provenance": {"kind": "proposed", "origin": "agent:factory/ep9-cast"}}}
← {"findings": [], "rendered": ["tegwen.trade = net-mender  proposed"],
   "workspace_digest": "jcs:sha256:9d40…"}

→ {"tool": "author.add_relation",
   "args": {"relation": "located_in", "from": "tegwen", "to": "gwern",
            "provenance": {"kind": "proposed", "origin": "agent:factory/ep9-cast"}}}
← {"findings": [], "rendered": ["located_in(tegwen, gwern)  proposed"],
   "workspace_digest": "jcs:sha256:1f77…"}
```

**Review the diff.** `author.diff` renders the whole canonical block and the full
findings, including the merely-possible status of everything proposed.

```json transcript
→ {"tool": "author.diff", "args": {"workspace": "ep9-cast"}}
← {"rendered": [
     "person tegwen \"Tegwen\"  proposed  agent:factory/ep9-cast",
     "tegwen.trade = net-mender  proposed",
     "located_in(tegwen, gwern)  proposed"],
   "findings": [
     {"grade": "asserted", "severity": "warn", "code": "name.unvetted",
      "message": "The name \"tegwen\" has not passed the naming oracle; it enters proposed and unvetted."}],
   "workspace_digest": "jcs:sha256:1f77…"}
```

**Materialize.** The tool returns the `.dli` file lines as **data** in
`rendered`; the kernel writes nothing. The agent chooses the fantasy-side path
(here `lore/staging/ep9-cast.dli`), writes the lines, and commits through
fantasy review.

```json transcript
→ {"tool": "author.materialize", "args": {"workspace": "ep9-cast"}}
← {"findings": [],
   "rendered": [
     "scope ep9-cast",
     "person tegwen \"Tegwen\"  proposed  agent:factory/ep9-cast",
     "tegwen.trade = net-mender  proposed",
     "located_in(tegwen, gwern)  proposed"],
   "workspace_digest": "jcs:sha256:1f77…"}
```

The agent commits `lore/staging/ep9-cast.dli` on the fantasy side. If Tegwen
should become ratified canon, promotion is a later `dli.fork.promote` →
`docs/rulings/` ruling request, ruled by the author, per the standing SOP. The
machine never wrote canon at any step.

The seed golden fixture (`contracts/dli-worker/fixtures/golden/author-session-v1.json`)
demonstrates this same tool flow against the example world. The AUTH-2 fixture
at `worlds/tpth/auth2/author-tegwen-v1.json` encodes this scenario against the
pinned-corpus-backed TPtH project and replays under
`cargo test -p dli-serve --test auth2`. It is CP-10's deloreis-side replay
evidence; the matching consumer transcript remains EXT-F4.
