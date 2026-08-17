---
status: live
verdict: "The deprecation-audit table WS-6 named: dispositions for the thirteen `.dli`-only Decl forms, the `over order`/`over season` domain-rule micro-grammars, the remaining text-on-wire remnant, and `ForkStore::merge`'s dormant status. Evidence, not authority on its own — the ruling of record is the DL entry this doc's ledger companion mints; this doc is what that entry cites."
---

# The `.dli`-only forms audit

Twenty-eight `Decl` variants exist. Fifteen are reachable through a typed `author.*`
wire tool (the exhaustive accounting is `carriage_of` in `dli-lorestore/src/lower.rs`,
the only totality check in the tree that catches a #47-shaped gap by construction).
The remaining thirteen are authorable only through `.dli` text or
`dli.corpus.import { dli_documents }` — both syntax-gated, both licensed-but-
unscheduled for removal (WS-6, campaign plan §2.6). This doc rules, per form, what
happens to it and under what trigger the ruling reopens. It does not itself carry
authority — the consolidated DL entry this campaign mints is the verdict of record;
this doc is the evidence that entry cites, kept separately so the ledger stays an
append-only record of rulings, not a rehearsal of the reasoning behind them.

## 1. The table

| Decl form | Disposition | Trigger to reopen |
|---|---|---|
| `Rule` | **Reimplemented at contract 1.7.0** (bare world rules only; `over order`/`over season` explicitly excluded — see §2). | First consumer ask for typed order/season authoring (see §2's own trigger — narrower than Rule's own, now satisfied). |
| `Cycle` | Open, corpus-load-only. DL-0058 tier 3 (parked behind first consumer use). | First consumer use. |
| `Practice` | Open, corpus-load-only. DL-0058 tier 3. | First consumer use. |
| `Proof` | Open, corpus-load-only. DL-0058 tier 3. | First consumer use. |
| `Kind` | Open, corpus-load-only. DL-0058 tier 4 (indefinite). No tool built; DL-0058 already records "no proven need shown." | First consumer use with a demonstrated mapping. |
| `Word` | Open, corpus-load-only. DL-0058 tier 4. | First consumer use. |
| `Principle` | Open, corpus-load-only. DL-0058 tier 4. | First consumer use. |
| `Use` | **Intentional no-op**, ruled here (§3.1). | First consumer need for real cross-scope import/resolution. |
| `Draft` | **Intentional no-op**, ruled here (§3.2). | A consumer need for in-kernel-evaluable draft content (distinct from the live `check-draft`/payload pipeline, which already exists and does not use this decl). |
| `Export` | **Intentional no-op**, ruled here (§3.3). | A consumer need for this decl to configure export behavior (distinct from the live `dli.corpus.export` wire op and the `deloreis export` CLI, neither of which reads it). |
| `Narration` | **Intentional no-op for the core inert-vantage semantic** (D-0104); the surrounding machinery is an **open, unbuilt gap** (§3.5). | First consumer need for narrator-era knowledge-asymmetry enforcement (premise exclusion, event type-checking against the vantage). |
| `Variant` | No-op, but **not** ruled intentionally-inert-by-design — unbuilt, deferred (§3.4). | First consumer ask for lore-variant deltas. |
| Fork (`Decl::Fork`) | Open, corpus-load-only. **Not previously ruled** — DL-0058's tiers do not name it; this entry is the first to record its status, alongside the twelve DL-0058 already tiered. | First consumer use. Note: distinct from the batch `dli.fork.*` ops and `AuthorOpenParams`'s `fork` mode, which operate on authoring-workspace forks, not this AST declaration. |
| `Ratify` | **Partial by design.** No `author.declare_ratify`-shaped op exists or is planned: a `Ratify` object is minted only as a side effect of `author.set_standing`/`author.defeat`. Who may rule is the consuming application's call (DL-0054), not this engine's. | None — this is a permanent architectural position, not a parked gap. |

Thirteen rows above the `Ratify` line; `Ratify` itself is the fourteenth `.dli`-only
form and is listed separately because its disposition is not "open" or "inert" but a
settled design position with no trigger at all.

## 2. `over order` / `over season` — open gap, named trigger (user pin)

These are not `Decl` variants; they are micro-grammar clauses inside the rule surface
that the WS-2 rule-authoring wire op (`author.add_rule`, contract 1.7.0) deliberately
excludes. `Rule` expressivity at 1.7.0 is bare world rules only — typed datalog
head/body, recursion, stratified negation, range restriction. `over order` and
`over season` domain rules never enter `World.rules`; they are text-only clauses
owned by `dli-time`/`dli-ledger`, and their only authoring path is `.dli` text.

**This is an open gap, not a closed one, and removing `.dli` text removes their only
authoring path entirely** — there is no typed wire successor. Two live consumer
artifacts already depend on it: `worlds/tpth/time/cycles.dli` and
`worlds/tpth/practices/spoken-proof.dli`. **Trigger to reopen:** the first consumer
ask for typed order/season authoring — fantasy's FW-10 cutover is the likely real
trigger, since those two artifacts are exactly the shape FW-10 would need to migrate.

## 3. The five inert forms, ruled

PR-0c (Decl exhaustiveness) left `Use`, `Draft`, `Export`, `Variant`, and `Narration`
as behavior-neutral no-op arms in both `dli-world/src/loader.rs`'s and
`dli-eval/src/world.rs`'s now-exhaustive `Decl` matches, and deferred the per-form
finding disposition to this audit — flagging all five error-severity in the first
pass regressed the shipped round-table/camelot reference corpus (`use language`,
`use chronology`, `narration vantage after-camlann` all appear in
`examples/camelot.dli`), taking a clean `dli.check` from zero findings to three
(SEV-A, caught by red team; pinned now by `decl_exhaustiveness` tests in both
`dli-world` and `dli-eval` asserting these five decls lower to zero findings and
zero facts).

The question for each form is the one issue #47 (`Decl::State`, DL-0071) posed:
does it parse, persist, and round-trip while nothing ever reads it — a genuine
defect relative to what an author reasonably expects — or is its non-evaluability
the intended shape?

### 3.1 `Use`

`UseDecl { target }`. Doc comment: "import another scope's surface" — a name that
implies real import semantics. No code anywhere in `crates/` resolves `target` into
anything: multi-document project assembly (`project.rs`) discovers documents from an
explicit manifest, never by walking a document's `use` declarations, and no other
consumer exists. DL-0058 already recorded, of the whole unbuilt tier, "`Use`/`Kind`
remain unbuilt for the same reason — campaign honest-unknown 5 asked for proven need
and no mapping table has shown one." Camelot's own two `use` lines
(`use language`, `use chronology`) function today as self-documenting headers, not
as anything the loader acts on, and the corpus is correct with or without them
resolving to anything.

**Ruling: intentional no-op**, standing on DL-0058's existing "no proven need"
finding rather than reopening it. Raising `load.inert-declaration` here would fire
on every shipped corpus that documents its own dependencies this way, for no
present benefit. **Un-lift trigger:** a real consumer need for cross-scope
import/resolution — at that point `use` gets built, not flagged.

### 3.2 `Draft`

`DraftDecl { name, label, body: Vec<Clause>, provenance }`. No code reads it beyond
storage carriage. Critically, this decl is **not** the same thing as the live
`check-draft` / payload pipeline (`dli.check-draft`, `dli-world/src/payload.rs`,
`deloreis check-draft`): that pipeline's own doc comment states "a draft is prose;
prose never enters the kernel — what enters is the draft's structured shadow," and
it is keyed by a `SourceRef` pointing at a prose file on disk, never by
`Decl::Draft`. `syntax-reference.md` files `draft` under "records of process" as a
payload anchor, consistent with it being a named reference point for the
out-of-band prose the real pipeline actually checks, not a container for
in-kernel-evaluable content itself.

**Ruling: intentional no-op** — this is the same shape as `Ratify`'s carriage-only
role, an anchor rather than a claim. **Un-lift trigger:** a demonstrated need for
this decl specifically (not the existing `check-draft` pipeline) to carry
in-kernel-evaluable content.

### 3.3 `Export`

`ExportDecl { profile, label, body: Vec<Clause>, provenance }`. Confirmed
independent of two same-named, unrelated, and very much live mechanisms: the
`dli.corpus.export` wire op (`format: objects|rendered|canon`, reads folded fact
objects from the lorestore by commit hash — never matches `Decl::Export`) and the
`deloreis export` CLI (`ExportArgs`, `dli-export::profile::Registry`, its own
`--profile` flag typed as `ProfileArg` — also never matches `Decl::Export`). Three
"export" concepts share one English word in this codebase; only one of them is the
AST decl under audit, and it is read by neither of the other two.

**Ruling: intentional no-op**, same anchor/reference shape as `Draft`.
**Un-lift trigger:** a demonstrated need for this decl to configure real export
behavior, distinct from the two mechanisms that already own that name's live
meaning.

### 3.4 `Variant`

`VariantDecl { path, body: Vec<Clause>, provenance }`. Doc comment: "a lore variant
of an artifact"; `syntax-reference.md` files it as "(ruleset delta)" — language
that, unlike `Draft`'s and `Export`'s framing, implies real intended effect rather
than a documented anchor role. No shipped or vendored `.dli` corpus exercises it, so
raising a finding here carries no corpus-regression risk the way `Use`'s or
`Narration`'s would.

**Ruling: no-op, but explicitly NOT ruled intentionally-inert-by-design.** Unlike
§3.1–3.3, nothing in the design record argues `Variant`'s silence is the intended
shape — it reads as unbuilt, parked in DL-0058's tier 4 alongside `Use`/`Kind` for
the same "no proven need" reason, not as a settled anchor role. Recorded as an open
gap rather than a closed ruling so a future reader does not mistake "no finding
today" for "ruled inert." **Un-lift trigger:** the first consumer ask for
lore-variant deltas — at that point this either gets typed rule-modification
semantics or an explicit inert ruling, whichever the ask actually needs.

### 3.5 `Narration`

`NarrationDecl { head: Vec<String>, provenance }`. Two things are true at once here,
and they pull toward different findings:

- The vendored consumer canon already rules the narrator's era's own dynamics
  abstract (D-0104, `docs/spec/03-time.md` §(iii): narration-time is "inert by
  construction" — declaring an event inside `the-narrators-era` is a type error, no
  state dynamics exist there, and no machinery is meant to accrue to it). That core
  inertness is **intentional**, not a gap.
- The spec also describes real consumer machinery this vantage is meant to enable —
  knowledge asymmetry (the narrator retrospectively omniscient over story-present;
  cast knowledge evaluated as-of scene position) and a `narration-frame` guardrail
  excluding not-yet-true facts from a cast's premise pool. **None of this is
  implemented anywhere in `crates/`** — grepped for `narration-frame`, narrator-era
  type-checking, and the `narration-dynamics` error class; the only hits are AST
  field names, tests, and doc comments. This is a real gap relative to what the
  spec says the declaration is for.

Camelot's own `narration vantage after-camlann` line, and two other shipped
corpora, exercise this form; raising `load.inert-declaration` today would repeat
the same SEV-A regression PR-0c's red team caught.

**Ruling: intentional no-op for the core inert-vantage semantic** (D-0104 already
rules the vantage carries no dynamics of its own — nothing to fix there). **The
surrounding machinery — narration-frame premise exclusion and narrator-era
event type-checking — is recorded as an open, unbuilt gap**, distinct from an
inertness ruling: it is roadmap, not a defect in the marker declaration itself, and
building it would not make `Decl::Narration`'s OWN lowering evaluable — the
type-check and exclusion machinery would live in the checker, reading the
declaration's presence, not in the loader emitting a fact from it. **Un-lift
trigger:** the first consumer need for narrator-era knowledge-asymmetry
enforcement.

## 4. Text-on-wire remnants

- **`KILLED_FIELDS`** — the registered retired-string-field set (`serve::KILLED_FIELDS`)
  is covered by its own gate lane and DL-0050/DL-0051; nothing further for this audit
  to add.
- **The DL-0051-deferred runtime-continuity string form** — `dli.check-draft`'s
  `continuity` rows still carry the legacy `predicate`/`subject`/`value` string
  triple alongside the 0.6.4 typed successors (`atom_expr`, `to_atom_expr`).
  DL-0051 recorded explicitly that this multi-field form "is not yet a registered
  killed-form," deferred to EXT-F4. It remains deferred today — recorded here so it
  is not rediscovered as an oversight. Trigger: EXT-F4, per DL-0051.

## 5. `ForkStore::merge` — dormant by choice

`dli_eval::fork::ForkStore::merge` (fork-vs-fork, including certain-FD detection)
exists and is tested, but no wire op calls it — `dli.fork.*` stays the lightweight
session-only in-memory valuation overlay WS-3 explicitly kept that way (branch
bundle ruling, campaign plan §2.5(a)). WS-3's durable `author.merge` `dry_run` mode
now covers the durable analog (branch-vs-branch conflict inspection with no
commitment), which is what makes `ForkStore::merge` dormant **by choice**, not by
gap: the durable path was built to not need it. No trigger is named because no gap
is open — this is a settled architectural position, recorded so its dormancy reads
as deliberate rather than as an unfinished wire-up.

## 6. `.dli` phase-4 removal

Licensed but unscheduled, per campaign plan §2.6: no dated commitment against a
consumer timeline this repo does not control. Named trigger: fantasy's FW-10.
Removing `.dli` text removes the only authoring path for every open-gap form in
this audit (§1's open rows, §2's `over order`/`over season`) — that dependency is
exactly why phase-4 stays gated on a real consumer signal rather than an internal
schedule.

## 7. Predicate identity adds no text form

The contract `1.12.0` predicate-identity declaration is deliberately a typed
wire-only addition: `author.assert_predicate_same_as` has no new parser keyword,
formatter line, or `.dli`-only `Decl` form. This is a concrete deprecation-audit
data point, not an omission. No current consumer requires predicate identity to
round-trip through `.dli`; the supported interchange and review surfaces are
the structured authoring result, fact-object JSON, staged diff, and query/why
data.

The engine still records the caller's identity decision, evaluates its certain
consequences, and carries its support through FD/fluent/rule derivations. If a
consumer later demonstrates a need for textual round-trip, FW-10 is the trigger
to reopen this audit and add a typed consumer-owned rendering requirement; it
does not authorize adding a retiring `.dli` form now.
