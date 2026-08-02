---
status: live
verdict: "Quality pass over the LS-1..LS-5 implementation (commits 6a34d22..99f82bc plus the uncommitted LS-5 working tree), run against the lorestore roadmap's stage specs and adversarially verified finding-by-finding. The storage core is sound; the center is hollow: the store never holds the base world in production, lowering covers only the extensional gentle subset, and the newest ledger draft rules things the code does not do. Tracked repair list; rows flip to fixed as repair commits land. Evidence for the repair programme, not authority."
---

# Lorestore implementation audit (LS-1..LS-5)

A quality pass over the lorestore cutover as implemented, run before the back
half (LS-6, LS-9, LS-7, LS-8) builds on it. Method: five review dimensions
(model, storage, read/write paths, contract, ledger/docs) against the stage
specs in `lorestore-roadmap.md`, every finding independently re-verified
against the code by an adversarial pass; findings that did not survive were
dropped or downgraded. File:line references are against the working tree at
audit time (HEAD `99f82bc` + uncommitted LS-5 work).

Two governing decisions, made by the project owner at audit time:

1. **Contract**: repair the code first while the wire changes stay
   uncommitted; then land one coherent **0.6.0** commit — v1 fixture bytes
   restored, v2 fixtures added, 0.5.1 requests still answered (dual-mode, per
   the roadmap and DL-0031..DL-0037 discipline).
2. **Totality**: lowering becomes total **before LS-6**, via string-bodied
   `ExpertBody` objects (the v1 schema design); `reconstruct_pack` refuses
   what it cannot round-trip; LS-9 later deepens strings to typed interiors.

The repair sequence lives in the PLAN store-lane evidence; this note is the
finding-by-finding record. Severities are post-verification.

## A. Committed defects (LS-1..LS-4)

| # | Sev | Where | Finding | Repair direction | Status |
|---|---|---|---|---|---|
| A1 | high | `crates/dli-serve/src/serve.rs:199-217` | Production serve never calls `import_pack`: the store holds only staged deltas atop an empty genesis, so `Lorestore::compile()` yields a world missing the entire base and every advertised root hash names a world that never existed. Only the LS-5 test imports manually. | Import base packs at attach time (idempotent via CAS dedup); serve-level test with no manual pre-import. | fixed — 8f3b8db |
| A2 | high | `crates/dli-lorestore/src/lower.rs:119,300-303` | Lowering is not total: only Entity/Attr/Fact/Before produce objects; rules, kinds, cycles, states and all eleven expert-tier declaration types produce zero objects; `ExpertBody`/`with_expert_body` has zero callers; the "total lowering" oracle counts LoadedAtoms so it cannot see the loss. | Emit string-bodied `ExpertBody` objects (decision 2); totality assertion walks Decls, not LoadedAtoms. | fixed — 80cfa7d |
| A3 | high | `crates/dli-lorestore/src/lower.rs:433,173,468` | ClaimId violates its own stability rule: three minting schemes (positional `atom#index`, `decl:`, `fact#index`); inserting an earlier declaration renumbers every later claim id; parity tests compare content hashes only (claim id excluded from the hash) so the divergence is invisible. | One stable minting function shared by all three paths, from addressing metadata, never positional; oracle asserts claim-id equality across paths. Safe now: no production store exists yet (A1). | fixed — 80cfa7d |
| A4 | high | `crates/dli-serve/src/author/tools.rs:268-272`, `mod.rs:270-287` | Digest-fence break: materialize returns the commit hash (`sha256:`) as `workspace_digest`, but the stale fence compares the `jcs:sha256:` workspace digest, which materialize never updates — an agent fencing with the returned digest is always refused. | Reseal the workspace digest on success; return the commit hash in a separate `root_hash` field. | fixed — DL-0046 |
| A5 | med | `crates/dli-lorestore/src/lower.rs:609-669` | `reconstruct_pack` is partial and silent: rules/kinds/cycles are never stored; stored before-atoms come back as plain facts (chronology edges lost); `derive_annotation` always emits empty condition/tags and has no defeater channel. | Refuse (typed error) anything that cannot round-trip faithfully; restore chronology; carry conditions/tags. | fixed — 80cfa7d |
| A6 | med | `crates/dli-lorestore/src/lorestore.rs:333-336,142-150` | Commit interners are whole-replacement while `compile()` resolves all facts against the head commit's interner only; the documented append-only extension is unenforced — sequential workspaces can silently misresolve earlier facts' support polynomials. | Validate prefix-preserving extension in `Lorestore::commit`; seed workspace interners from the store head. | fixed — 8f3b8db |
| A7 | med | `crates/dli-lorestore/tests/differential_parity.rs:26` | The LS-3 parity oracle runs one YAML-bridged gentle-subset world — never any of the ten vendored `.dli` packs — and never compares digests; the corpus exercises no defeaters, conditions, or tags, so round-trip parity is claimed beyond what is tested. | Iterate the vendored packs (parse → import → compile), compare digests, extend the corpus. | fixed — 80cfa7d |
| A8 | med | `crates/dli-lorestore/src/storage.rs:244-248` | Concurrent writers of the same object race through a shared deterministic temp filename (unconditional remove-then-write): one process can rename another's half-written temp into the content-addressed path — a torn object at a named path. Object writes are not under the advisory lock. | Unique temp names (pid + nonce); drop the unconditional remove. | fixed — 8d19b2f |
| A9 | med | `crates/dli-lorestore/src/identity.rs:200-203` | `LineageEdge::content_hash` hashes the whole struct including claim id and schema version — contradicting its own doc, the FactObject projection discipline, and the rule that minting a claim id never destabilizes addressing. Also: no code anywhere constructs a LineageEdge — the third leg of the identity split exists on paper only. | Same hash-projection discipline as FactObject; one end-to-end edge test; first production emission belongs to LS-6. | fixed — 8d19b2f |
| A10 | med | `crates/dli-lorestore/src/evidence.rs:68` | The no-floats rule is unenforced at the one door where arbitrary JSON enters: `EvidenceEnvelope.payload` is an unconstrained JSON value and the JCS crate serializes finite floats happily, while `canonical.rs:26-29` falsely documents that floats fail to encode. | Reject non-integer numbers in `EvidenceEnvelope::new`; fix the error doc. | fixed — 8d19b2f |
| A11 | low | storage, misc | Genesis-open race (ref check before lock; CAS loser panics); ref CAS compares the commit hash only, so valuation-overlay-only updates are unfenced (`Ref::digest()` is documented for fencing but has zero callers); the state-equals-fold test is a fixed three-round example whose comment overclaims; crash injection fires only at operation boundaries (the roadmap's stated window is covered); the cross-platform claim runs only on Linux and `sync_dir` cannot work on Windows; upcast-on-read is absent even as a stub (schema_version written, never read). | Lock before ref check; CAS on the full ref digest; property-test the fold invariant; scope the platform claim; add the v1 upcast stub. | fixed — 8d19b2f |
| A12 | low | `crates/dli-core/src/posbool.rs:304-338` vs roadmap | Stored polynomial ordering is numeric-by-interned-atom-id, not lexicographic-over-wire-tokens as the roadmap's sentence implies. Deterministic and unique — the code is fine; the written rule would make an independent implementation compute different hashes. | Fix the roadmap wording to state the actual total order. | fixed — 4f819ce |

## B. The uncommitted LS-5/LS-4b working tree (must not land as-is)

| # | Sev | Where | Finding | Repair direction | Status |
|---|---|---|---|---|---|
| B1 | high | `contracts/dli-worker/` | Twelve new unconditionally-required properties across four response schemas; eight fixture digests re-pinned in place; `contract_version: 0.5.1` unchanged and the server rejects any other version — so 0.5.1 now labels two different byte sets and no 0.6.0 exists to opt into. The AUTH-2 worked-scenario golden under `worlds/tpth/auth2/` likewise records 0.6-shaped responses under 0.5.1 framing (deloreis-owned, so mutating it is legal — the version labeling is not). | Decision 1: restore v1 bytes; land 0.6.0 dual-mode with v2 fixtures. | fixed — DL-0046 (0.6.0 dual-mode; v1 fixtures byte-frozen and green) |
| B2 | high | `docs/decisions.md` (uncommitted draft DL-0045) | The draft entry's central ruling is false as implemented: the query root hash is not a lorestore commit hash, `Lorestore::compile()` cannot load by hash, and the review test proves only row-count plus atom-string parity where the entry claims byte-identical replay. Its date (2026-08-01) predates the work it describes. | Rewrite before committing (it is uncommitted text): rule only what holds, date honestly. | fixed — 4f819ce |
| B3 | high | `crates/dli-serve/src/author/reads.rs:297,452-473` | `AuthorDiffStanding.support` is empty or wrong for exactly the staged facts under review: staged-world support is resolved through the base world's interner (staged-only variables silently dropped), then filtered to decision-id prefixes (dropping every hypothesis/bundle variable), and the antichain is flattened to a single list — a gated claim renders identically to an unconditional one. | Resolve through the staged interner; drop the prefix filter; represent support as a list of monomials. | fixed — DL-0046 |
| B4 | high | `crates/dli-serve/src/author/store.rs:331-337` | `stage_lorestore` silently swallows write errors and still advances the interner-name and bundle counters; materialize later commits the incomplete delta with success findings — the durable record diverges invisibly from the in-memory workspace. | Return `Result`; surface failure as an error-severity finding; no counter advance on failure. | fixed — f39cf12 |
| B5 | high | `crates/dli-lorestore/src/lower.rs:342-352` | Per-declaration lowering fails open where the loader fails closed: tag-gated proposal variables are interned without the loader's `h.` prefix (content-hash divergence against a full compile); a malformed assumes-condition is silently dropped leaving truth support; a citation-requiring marker with zero citations yields unconditional truth instead of false support. The parity corpus never sets tags, so it cannot catch any of this. | Mirror the loader exactly (prefix, fail-closed, citation check); tagged-proposal parity fixture. | fixed — f39cf12 |
| B6 | high | `crates/dli-serve/src/handlers.rs:1589-1611` | The `dli.query` replay context is unsound: its root hash is an ad-hoc digest over interner names plus valuation — blind to the fact set, rules, and grades, resolvable in no store, prefix-indistinguishable from commit hashes — and fork/as-of overlays that change the answer are excluded from the reported pair. | Bind the fact set and fold overlays, under a domain-distinct prefix — or omit the fields until store-backed. | fixed — DL-0046 |
| B7 | med | `handlers.rs:1622`, `reads.rs:586` | The wire valuation renders only atoms ruled true, dropping explicit-false rulings the engine distinguishes from unruled — while the root-hash computation hashes the full map including falses: the two halves of the replay pair disagree about what the valuation is. | Carry the full valuation (sorted atom/value pairs; absence means unruled) in both places identically. | fixed — DL-0046 |
| B8 | med | `reads.rs:73`, `tools.rs:83-107`, `contract.rs:1226-1228` | `root_hash` means three incompatible things across the surface: an ad-hoc state hash (`dli.query`), the `jcs:sha256:` workspace digest (`author.query` — pre-materialize it names nothing loadable), and the live store head (`author.status`/`diff` — whose doc claims "base commit this workspace is relative to", false: it moves when any other workspace materializes). | One meaning (the lorestore commit hash); pin the base commit at open; keep the workspace digest in its own named field. | fixed — DL-0046 |
| B9 | med | `crates/dli-serve/src/author/mod.rs:74` | `author.materialize` now mutates (advances the main ref) but is dispatched through the read wrapper: no idempotency caching, staged state never cleared — a retry writes duplicate commits (the folded world survives via dedup; the DAG accumulates junk and the mutation law is violated). | Route through `mutate`; clear staged state and reseal on success. | fixed — DL-0046 |
| B10 | med | `docs/PLAN.md`, `AGENTS.md:122` | PLAN marks LS-4/LS-5 done against deliverables that do not exist: no JSON-first materialize (the params struct has no format switch; rendered text is returned unconditionally — AGENTS.md §7 claims otherwise), no 0.6.0, no byte-identical replay oracle. The LS-4 evidence cites DL-0044 for per-add lowering that DL-0044 explicitly defers; a stray checkmark decorates the LS-5 registry line (status lives in the matrix). | Honest states and evidence (repair D2); ledger home for the supersession. | fixed — 4f819ce |
| B11 | med | `crates/dli-serve/tests/ls5_review.rs:283-292` | The replay test compares row count and atom string only; bindings, support antichain, grade, and status are never compared; the replayed answer bypasses the wire row shape; the accept path (materialize) and reject path (discard) are not exercised. | Byte-equality over full wire rows; drive both accept and reject. | fixed — DL-0046 (`tests/ls5_review.rs` rebuilt) |
| B12 | low | `contracts/dli-worker/fixtures/framing/paired-all-ops-v1.json` | Roughly 95% of the 2,221-line diff is pretty-print reformatting hiding a six-case semantic change, and the four add-op taint allowances are inert (those ops never return the allowed path). The framing property itself still holds. | Re-record for 0.6.0 without reformatting; drop the inert allowances. | fixed — DL-0046 (v2 cases appended without reformatting; inert allowances never landed) |
| B13 | low | `crates/dli-lorestore/src/lower.rs:380-401,217-276` | The copied condition compiler violates De Morgan under negation and accepts empty groups the loader rejects (unreachable today — only bare hypothesis conditions flow in); declaration provenance is interned for kinds the loader never routes through annotation compilation while entity slots the loader does lower are dropped; `materialize_to_lorestore` is dead code with a false justification comment. | Port the loader's condition compiler verbatim; align the interning scope; delete the dead code. | fixed — f39cf12 |

## C. Ledger and docs discipline

| # | Sev | Finding | Repair direction | Status |
|---|---|---|---|---|
| C1 | high | The roadmap's LS-3 annotations (committed in 99f82bc) cite "DL-0042 (refined)" and "DL-0043" for the `decisions`/`pack_name` commit fields — but DL-0042 is the line-count-budget retirement and DL-0043 the door-semantic retirement. Fake ledger authority; the fields have no ledger home at all. | Correct citations; give the fields their ledger home in the rewritten DL-0045. | fixed — 4f819ce |
| C2 | med | The roadmap gained implementation-changelog paragraphs inside a proposed design note (docs discipline: notes are settled reasoning; implementation state lives in PLAN matrix evidence) — and one is already stale against the working tree. | Strip to one-line pointers at the correct ledger entry. | fixed — 4f819ce |
| C3 | low | The roadmap frontmatter is still `status: proposed` although DL-0041 ratified it (the sibling vision note is `accepted` with a ratification banner). | Flip to accepted, citing DL-0041. | fixed — 4f819ce |
| C4 | low | Gate gap: no lane asserts that released fixture bytes are frozen — re-recording v1 fixtures under an unchanged contract version passed all seventeen gate steps. The breach would have surfaced only on the consumer's digest check. | Fixtures lane fails when any registry digest changes while the contract version is unchanged against HEAD (the ledger's append-only-check idiom). | fixed — DL-0046 (fixtures-lane guard) |

## Verified sound — recorded so it is not re-litigated

- **LS-2 storage core.** Temp-file + fsync + rename with parent- and
  grandparent-directory fsync (`storage.rs` `publish_atomic`); a real
  read-verify-write ref CAS under an `fs2` advisory lock; object-before-ref
  ordering with existence-plus-hash verification; an append-only
  content-addressed commit DAG in which nothing is overwritten in place.
- **Identity discipline where it was designed.** No parent hash in the
  FactObject content bytes; claim id and schema version correctly excluded
  from the FactObject hash projection; no floats in any Rust type or
  generated schema (A10's payload door excepted); `authority` present and
  optional on all five object kinds; the same JCS implementation as
  `dli-serve/src/evidence.rs`, so evidence content addresses carry over.
- **Schema-first held.** All five stored-object schemas are
  schemars-generated (`dli_lorestore::schemas()`) and drift-checked by
  `xtask schema --check` inside the gate; the commit and ref schemas match
  the structs — the LS-3 field additions did not drift.
- **Accept/reject substantially exists.** `author.materialize` is the accept
  decision and `author.discard` (with its per-declaration parameter) is the
  reject decision. What is missing is the golden fixture exercising both and
  a false doc-reference to a nonexistent accept mechanism at
  `contract.rs:1273` — not the semantics.
- **The cross-platform determinism test pins a hard-coded root hash** — the
  strong form of the oracle (its platform coverage, not its design, is A11's
  complaint).
- **The framing property held** through the fixture re-record: verdict paths
  are still compared control-vs-attack and taint stays confined (B12 is
  about reviewability and inert allowances, not the property).

## Repair sequencing

Seven commits, gate-green before each push, tracked in the PLAN store lane:

1. This audit (docs only).
2. Ledger and docs truth restoration: DL-0045 rewritten while uncommitted;
   roadmap citations and frontmatter; PLAN honest states; AGENTS.md §7.
3. Staging integrity, fail-closed lowering (B4, B5, B13).
4. Store soundness: base import at attach, interner extension discipline,
   materialize as a real mutation (A1, A4, A6, B9).
5. Totality: expert-tier lowering, stable claim ids, loud reconstruction,
   corpus-wide parity (A2, A3, A5, A7 — decision 2).
6. Storage hardening (A8, A9, A10, A11).
7. Contract 0.6.0 dual-mode with repaired wire semantics and the strengthened
   replay oracle (B1, B3, B6, B7, B8, B11, B12 — decision 1); C4's gate check
   rides along. LS-4/LS-5 flip to done with real evidence only after this.
