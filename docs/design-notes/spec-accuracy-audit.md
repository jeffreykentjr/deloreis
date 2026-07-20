---
status: reference
verdict: "The wave-4 spec-vs-implementation accuracy audit: a per-doc verdict for every settled spec (00–11) and the integration docs, the corrections made, the drifts routed to owning lanes, and the cross-reference/mdBook/docs-oracle results. The decision ledger outranks this note."
---

# Spec-vs-implementation accuracy audit (wave 4)

A systematic pass over the settled specification: read each doc, verify every
concrete claim against what shipped (op tables against `dli_serve::Op`, the
error taxonomy against `ErrorCode`, CLI docs against the clap surface, keyword
lists against `line_open_keywords()`, and fenced `dli` examples against the
parser/loader), and either **correct** a provably-stale settled claim in place or
**flag** it to the lane still building it.

## The calibration that governs every "stale" call

DL-0016 (the does-what-it-says pass) renamed the **surface keywords**:
`fluent→state`, `wheel→cycle`, `ledger→proof` (the in-practice proof block),
`morpheme→name-part`, `lexeme→word`, `register→dialect`, `sort→kind`,
`pack→scope`, and unified the rule family under `rule … over <domain>`.

It did **not** rename the charter's concept vocabulary. Koan 8 is literally
titled *"The wheel refuses rank"* and the charter uses "wheel" and "sort" as
living metaphor throughout (`charter.md` §8). So:

- A **surface reference** using an old spelling — a keyword inside a `dli` block,
  a CLI command, a wire op name, or "the `X` keyword" — is stale and corrected.
- **Concept/metaphor prose** using "wheel", "sort", "ledger", or "register" as a
  word (as the charter does) is *not* stale and is left intact. Correcting it
  would put the specs out of step with the constitution they answer to.

This is why the majority of the raw "soft-stale" flags a keyword grep surfaces
are **not** defects: the `dli` examples in specs 03/04/05 already use the
post-rename spellings (`cycle`, `state`, `proof`, `kind`, `dialect`), and only
prose/koan taglines retain the metaphor — correctly.

## Per-doc verdict

| Doc | Verdict | Notes |
|---|---|---|
| 00 · overview | **corrected** | `deloreis check-ledger`→`check-proof` (CLI, §"seven programs" item 5) and the op-inventory `check-ledger`→`check-proof`. Concept prose ("a ledger checker", "declarative-ledger checker") left intact. |
| 01 · syntax | **corrected** | "roughly twenty-five domain keywords"→"roughly thirty" (the real set is **32**, `LINE_OPEN_KEYWORDS`; the generated `grammar-keywords.md` is authoritative). Keyword examples already use `cycle`. AST-first/schemars/no-status-slot/killed-form claims all verified true. |
| 02 · semantics | **accurate** | PosBool annotation pair, variable kinds (6), three-valued standing, weakest-grade rollup, six continuity classes + four fact-check verdicts, fork ops (`suppose/add/suppress/restrict`), keep-stamp (3 states) all match the evaluator. |
| 03 · time | **accurate** | `cycle` ops (`succ/pred/across/distance`; `across=floor(n/2)`, even⇒involution, odd⇒pred; `distance(a,b)+distance(b,a)=n`; `separation=min`) match `cycle.rs` exactly. `dli` examples use `cycle`/`state`. Koan-8 "wheel" tagline and the "Fluents" concept heading are metaphor, not stale (module is `fluent.rs`, surface keyword is `state`). |
| 04 · practices | **accurate** | Seven structural classes L1–L7, the closed 4-member resource algebra, nine failure modes, ~nine ledger step forms, and the `proof`/`kind`/`practice` examples all verified. §5–6 prose says "ledger" as the concept word; the examples correctly use `proof` — not corrected (concept vocabulary). |
| 05 · language layer | **accurate** | `dialect`/`name-part`/`name`/`term`/`banned`/`hypothesis`/`ratify` keywords, the 8 in-kernel + 6 factory-side vet checks, and the mirror-death query path all verified. §1 heading "Registers as declarations" is the linguistics concept; the finding code `name.register-fit` legitimately keeps "register" (see `finding-families.md`). |
| 06 · craft and payloads | **flagged (lane building)** | CRAFT-2/CRAFT-3/PAY. §5 "**79**-principle KB" is now provably **107** (`kb.rs` says so itself); §7.2 "eight check-families merged" and §7.3 `beat-unaccounted`/`beat-undeclared` are named but not yet executed in `check_draft` (PAY-2). Routed to the craft/payload lanes rather than rewritten mid-build. Outline schema (CRAFT-1) and quote-anchoring (PAY-1) verified landed. |
| 07 · verdicts + LLM boundary | **accurate (one soft note)** | Grade lattice `checked>tested>attested>asserted` (4, `Grade::ALL`), weakest-grade rollup, `attestation-missing`, ratification-exit⇒`checked` all verified. Soft: the §3 example question-kind `register-real-basis`/template `naming/register-basis@v3` reads `dialect-real-basis` in `attest.rs`; left as an illustrative free-text kind string (noted, not corrected). |
| 08 · reasoning | **accurate** | T1–T22 machinery map; six continuity classes (T3), ~nine proof forms (T20/§7), no Lean/α,β-CROWN deps, clingo lab-side only — all consistent. T16 "wheel-walk consistency" is metaphor prose for the cyclic-position walk (`cycle.rs`), not a surface reference — left intact per the calibration above. |
| 09 · exports | **pending wave-4 landing** | EXP-1..4 landed (`export_wire`, profile registry, context-pack, cards, articy). The spec's stub-framing is being overtaken by the landing; **do not treat as stale**. Caveat: `dli-export` does **not currently compile** in the shared tree (`games.rs`/`profile.rs` errors) — flagged to the export lane; re-verify 09's worked numbers once it compiles. |
| 10 · serving | **accurate envelope; §3 worked shapes routed to contract-close** | Envelope, the 13-code closed `ErrorCode` registry, the 41-op inventory (27 `dli.*` + 14 `author.*`), killed forms (`dli.check-ledger`→`dli.check-proof` correctly named), capability negotiation, and the self-aware "future gap" callout all verified. **Drift:** the §3.1 `dli.fork.list`/`dli.fork.diff` worked JSON shows richer objects than the wired `ForkListResult{forks:Vec<String>}` / `DiffDelta{…:Vec<String>}` (golden fixtures confirm the string shapes); the §3.2/§3.3 `simulate.*`/`check-proof` worked shapes describe **capability-gated (unwired)** ops. Routed to contract-close (wire ownership). |
| 11 · agent authoring | **out of scope** | Owned by the skill-mcp lane this wave; audited for cross-refs only (all resolve). |
| integration/fantasy-contract, migration, STATUS | **cross-refs verified** | Wire tables owned by contract-close. Note: `migration.md` uses an example finding `code: "merely-possible"` that no implementation emits — an illustrative wire example, flagged for contract-close, not a shipped code. |

## Corrections made (settled docs, in place)

1. `docs/spec/00-overview.md` — `deloreis check-ledger in <practice>` → `check-proof` (program 5).
2. `docs/spec/00-overview.md` — op inventory `check-ledger` → `check-proof`.
3. `docs/spec/01-syntax.md` — "roughly twenty-five domain keywords" → "roughly thirty" (real count 32).
4. `docs/PLAN.md` — PRAC-2 node: `check-ledger in <practice>` → `check-proof in <practice>`.
5. `docs/PLAN.md` — CLI-1 node op list: `check-ledger` → `check-proof`.

Untouched, deliberately: every "wheel"/"sort"/"ledger"/"register" that is concept
or koan vocabulary (charter-consistent), and `docs/surface-v1-naming.md` +
`fantasy-contract.md`, which legitimately record the killed spellings as the
killed-form corpus.

## Flags routed to owning lanes

- **craft/payload lane** — spec 06 §5 "79"→"107"; §7.2 eight-family battery and
  §7.3 `beat-unaccounted`/`beat-undeclared` are PAY-2 pending, not yet in
  `check_draft`.
- **contract-close** — spec 10 §3.1 `fork.list`/`fork.diff` worked shapes vs the
  wired structs; §3.2/§3.3 unwired-op worked shapes; `migration.md`'s
  `merely-possible` example code.
- **export lane** — `dli-export` compile errors block a full gate; re-verify 09
  after it compiles.

## Cross-reference, mdBook, and docs-oracle results

- **DL citations:** every `DL-nnnn` cited across `docs/**` and `book/**` resolves
  in `decisions.md` (defined range DL-0001…DL-0016; zero dangling).
- **doc→doc links + `{{#include}}`:** all relative `.md` links and mdBook
  includes resolve to existing files (zero dead references).
- **koan references:** every "koan N" is in the charter's range 1–12.
- **docs-oracle:** the xtask docs-oracle passes — every fenced `dli` block in the
  docs and cards validates.
- **mdBook:** `cargo xtask book` builds. SUMMARY now covers every spec (00–11),
  every integration doc, and the **five** generated reference pages (ops,
  error-codes, and the newly-added grammar-keywords, cli, and finding-families).

## The new generated reference

`docs/reference-generated/finding-families.md` is added this wave: the catalogue
of every stable dotted `family.item` finding code (113 codes across 9 families
plus the `proof.resource-violation` sub-codes), generated from the emitting crate
sources and coverage-checked against them (`xtask/src/finding_families.rs`). The
nine settled families are drift-checked strictly; the two still-landing families
(`name`, `outline`) are covered no-phantom so an in-flight lane edit does not gate
on the page.
