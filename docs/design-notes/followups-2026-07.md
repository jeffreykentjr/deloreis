---
status: reference
verdict: "The engineering follow-ups surfaced by the July audit, including closure records for items resolved by later ledger decisions. Open release work is authoritative in PLAN.md."
---

# Outstanding follow-ups (2026-07)

This note exists so the small engineering follow-ups surfaced during the
items-2/3/4 build and the Tier-A round are **tracked in a committed file**, not
only in ephemeral red-team task outputs. The later standalone audit found
release blockers outside that round; those live in the PLAN matrix and
`standalone-audit-2026-07.md`. Each item below records its current disposition.

The **larger roadmap is tracked elsewhere** — this note does not duplicate it:

- **Tier B (needs an author ruling)** and **Tier D (parked behind a flip
  condition)** — `docs/PLAN.md` node registry (BENCH-1/2, SURF-4, DEF-1..4,
  EXT-A1/A2) + `docs/decisions.md` (DL-0005, DL-0022) + `bench-deferral.md`.
- **Tier C (the cross-repo fantasy cutover)** — `docs/integration/STATUS.md`
  (CP-1…CP-10, all `proposed`), `docs/integration/fantasy-contract.md`, and the
  MIG-3/4 / EXT-F1/F3/F4 nodes in `PLAN.md`.
- **The three pending craft mutation operators** — `docs/design-notes/mutation-coverage.md`.
- **The Ludii trace differential** — `DifferentialStatus::Pending` in
  `crates/dli-game/src/export.rs`, gated by DL-0013 (Ludii not provisioned offline).
- **Ledger revisit triggers** (async simulate; real corpus-passing channel; the
  craft-library pack set on a 2nd world; post-freeze surface migrations) —
  `docs/decisions.md` DL-0021 / DL-0018 / DL-0016.

## The follow-ups

### F1 — negation blame names the proposal variable, not the atom (LOW)
`crates/dli-eval/src/why.rs:320` (`render_why_not`, the `"at risk from"` /
`"defeated by"` lines at :339/:341). For a certain-at-risk or defeated conclusion,
the blame line renders the defeater as its anonymous bundle variable (e.g.
`h.nine-realms#2`) instead of the atom that poisoned it (`stormed(asgard)`), and
the `#N` index shifts as other proposed facts are added earlier. The **assertion**
surface (`status`/`at_risk`, fork diffs) is correct; only the human-readable "why"
is opaque. Fix: thread the inverted provenance index into `render_why_not` so the
defeater renders to its atom name. Found by R-M1 F4 / R-A3 (verify pass).

### F2 — `fork.promote` renders the internal `ForkId` (RESOLVED)
Fork creation now names the evaluator fork with its registered wire handle.
Promotion therefore renders `fork-1` rather than the internal zero-based id;
the dispatch test and golden fixture pin both the slug and prompt. Found by
R-A1; closed by the standalone-runtime remediation.

### F3 — L2-contradicted residuals (LOW/by-design)
`crates/dli-world/src/scene_projection.rs:233` (`denied_by_exclusivity`). A3 made
`forsworn`/rebound reachable via option-exclusivity denials, but two edges remain:
- the "certain **negative** fact" contradiction sub-case is **not** implemented —
  the language has no stored negative ground-fact primitive (negation lives in
  rule bodies / defeaters per `spec/02-semantics.md §4`); exclusivity is the only
  concrete contradiction primitive today.
- a cross-family predicate-reuse hazard: when two different questions share an
  outcome atom that is certain, siblings in *both* families are denied. Defensible
  under "an option whose outcome is certain is the settled answer," but authors
  reusing a predicate across questions should know. Found by R-A3.

### F4 — `fork.add` asserts any parseable fact by fiat (INFO/by-design)
`crates/dli-serve/src/handlers.rs:341` (`additions_from_declarations`,
`Ann::unconditional()` at :354). A forked add discards the decl's declared status
and its own annotation, strips a dangling citation yet still asserts the fact
certain, and admits a fact referencing an undeclared symbol. This is the intended
what-if semantics (a fork is session-local; `promote` yields only a *draft*, never
canon), so the certain-canon trust boundary holds — but `fork.add` cannot
introduce a merely-*possible* fact and bypasses the B1 citation-authority check the
base `check` enforces. Revisit only if forks need graded/provenanced additions.
Found by R-A1.

### F5 — `name.check` honest subset + server `camelot` id-collision (PARTIAL)
- The wire/schema description now says this is the kernel-half dialect and
  phonotactic vet, matching the operation contract; it no longer claims the
  complete naming battery.
- `crates/dli-serve/src/session.rs:220`: the `"camelot"` builtin resolves the
  hand-built `example_docs()` (no proof block), so `dli.check-proof {document:"camelot"}`
  can't check a proof — only the `round-table` / `warding-vows` demo worlds carry
  one. A v0 resolver limitation (the wire ops carry only `{document}`); folds away
  when the worker gains a real corpus-passing channel (DL-0021 revisit trigger).
  Found by R-BC F5 / R-A1.

### F6 — `parse_caps` / `simulate --cap` minor input handling (LOW, mostly hardened)
`crates/deloreis/src/cmd/drafts.rs` (`parse_caps`). R-A3 hardened this: negative
and empty-resource caps now error. Remaining by-design: a duplicate
`--cap res=3 --cap res=9` silently last-wins (BTreeMap), no diagnostic; a cap for a
resource the practice does not declare `affine bounded` is simply unused (stays
deferred). Neither is wrong; a warning would be friendlier. Found by R-A3.

### F7 — grade_under ignores per-monomial defeat (LOW/pre-existing asymmetry)
`crates/dli-eval/src/world.rs` (`grade_under`). A defeated Checked monomial
still lends its grade — the status and grade channels are asymmetric. Closing
this requires a design ruling on whether defeated paths should lend grade.
Found by audit-validation-2026-07 §7.2.1(b).

### F8 — option.assumes semantic wiring (RESOLVED by DL-0029)
The inert AST field and surface form were retired. The parser now reports
`dli::syntax::killed-form` with a claim-level hypothesis-provenance repair;
examples that intended the condition express it on an explicit claim. The AST,
schema, formatter, specs, examples, and negative tests were updated together.
Found by audit-validation-2026-07 §7.3; closed 2026-07-25.

### F9 — join_body precision under path (B) conservative (LOW)
`crates/dli-eval/src/world.rs` (`join_body`). Uses `weakest_entry()` (the row's
weakest stored grade) instead of the specific monomial's grade. Under-claims
for multi-path bodies; guarded by `multi_path_body_no_over_claim` test.
Found by audit-close plan, path (B) registered.

### F10 — question-body junk clause filter_map drop (RESOLVED by DL-0029)
Question parsing now emits one error-class diagnostic for every non-option
child, malformed and bare options are diagnosed, and `fmt` refuses the source.
Fork command bodies remain intentionally preserved. Found by
audit-validation-2026-07 §7.3; closed 2026-07-25.

### F11 — ci_parity residuals (RESOLVED)
`xtask/tests/ci_parity.rs` now pins push and pull-request triggers to `main`,
rejects path/branch bypass filters, and rejects job- and step-level
`continue-on-error` or conditions on the gate. Found by
audit-validation-2026-07 §7.3; closed by the standalone release polish.

### F12 — F5S-5 finding-family severity verification (RESOLVED)
`xtask/src/finding_families.rs` derives severity observations from direct
constructors, Door-T `Finding` structs, craft-rule call sites, and the language
finding enum across nine families and fails closed on unsupported source shapes.
The five uniform-severity families have an explicit closed contract
(`attest.pass` is the sole non-error row). Five data-driven outline codes are
enumerated and pinned by runtime severity assertions; no additional code can
enter that exception set silently.
Found by audit-2026-07 §3.10; closed by the standalone release hardening.

### F13 — block-comment `/* #[cfg(test)] */` boundary poisoning (RESOLVED)
The boundary scanner now recognizes `#[cfg(test)]` only outside block/line
comments and string/character literals. A regression proves that the commented
attribute cannot exempt a following world-token leak. Found by
audit-validation-2026-07 §7.3; closed by the standalone release polish.
