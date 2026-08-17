//! Deloreis Lore Interchange world model.
//!
//! A [`Pack`] of documents [`compile`]s into a [`CompiledWorld`]: the loader's
//! status-gated [`WorldModel`] with its ledger valuation set and its symbolic
//! fixpoint computed, plus the compiled language layer, craft rules, and the
//! season ledger. Drafts arrive as fat [`Payload`] shadows and are checked with
//! [`payload::check_payload`]; [`Attestation`]s are recomputed for staleness with
//! [`sweep`]. The older [`DraftPayload`] projection remains available only for
//! legacy library callers.

pub mod attest;
pub mod check;
pub mod craft;
pub mod draft;
pub mod episode;
pub mod episode_order;
pub mod incremental;
pub mod language;
pub mod loader;
pub mod model;
pub mod pack;
pub mod payload;
pub mod predicate_identity;
pub mod repository;
pub mod scene_projection;

pub use attest::{Attestation, AttestationStore};
pub use check::{
    CompiledWorld, Severity, WorldFinding,
    factcheck::{ClaimReport, KilledToken, KilledTokens, MatchMode, RejectedClaim},
};
pub use draft::{DraftFinding, DraftMeta, DraftPayload};
pub use incremental::{
    CompileReuse, IncrementalWorld, compile_incremental, compile_incremental_verified,
    compile_resumable, compile_reusing_fixpoint,
};
pub use language::{
    DialectDef, LangFinding, LanguageModel, LanguageOutcome, NameDef, RatifiedName,
};
pub use model::{CraftRule, SeasonLedger, WorldModel};
pub use pack::Pack;
pub use payload::{Payload, PayloadFinding};
pub use predicate_identity::{
    PredicateArityMismatch, PredicateIdentityLink, PredicateIdentityState,
};
pub use repository::{RepositoryFinding, validate_repository};

use std::collections::BTreeSet;

use dli_ast::decl::Decl;
use dli_core::grade::Grade;
use dli_core::valuation::Valuation;
use dli_eval::error::EvalError;
use dli_eval::world::LowerFinding;

use crate::loader::Loader;
use crate::model::Provenance;

/// Compile a pack into a [`CompiledWorld`].
///
/// This is the single lowering the whole fabric shares (the `CHECK-1` carry
/// item): [`Loader::load`] gates every claim on its ratification status, then
/// compile adds the two things a raw load lacks — a **ledger valuation** and the
/// **symbolic fixpoint** over the loader's own status-gated facts. Reconciliation
/// is by *reuse*, not re-lowering: the loader already pushes each fact's gated
/// support into `world.world.facts`, so the evaluator saturates the very same
/// atoms. After compile, a claim's standing read off the loader's atoms and read
/// off the evaluated world agree exactly for the same document and valuation.
///
/// **Authority is declared, not minted (`B1`).** The ledger valuation rules a
/// cited decision variable true only when the corpus actually declares that
/// decision (`ratify <d-id>`); a citation to an undeclared decision is a forged
/// (or typo'd) authority claim — its variable is left open, so the claim can never
/// evaluate certain, and a `citation.unknown-decision` finding is recorded. This
/// matches `dli_eval`'s `lower_fact_annotation` exactly, so the loader and
/// evaluator representations stay convergent, and it closes the Door-H path where
/// a citation alone would otherwise assert a ruling into existence (charter
/// koan 1).
///
/// This is the **whole-pack** compile: every declaration is lowered afresh. The
/// incremental seam that reuses a prior compile instead — byte-identically —
/// is [`compile_incremental`]. Both
/// route through one shared tail (`incremental::seal`), so the ledger valuation
/// and the stratify-then-saturate handoff cannot drift between them.
///
/// # Errors
///
/// Returns [`EvalError::Unstratifiable`] if the world's rules admit no valid
/// stratification. Malformed inputs are never errors — they are load
/// [`findings`](WorldModel::findings) held fail-closed.
pub fn compile(pack: &Pack) -> Result<CompiledWorld, EvalError> {
    let model = Loader::load(&pack.documents).world;
    // Saturate the loader's status-gated facts; the clone inside the saturation
    // carries the valuation and findings through, so the compiled world
    // evaluates under the ledger.
    let (model, _) = incremental::seal(model, pack, None)?;
    Ok(CompiledWorld::new(model))
}

/// The decision ids the corpus actually declares via `ratify <d-id>` — the
/// authoritative decision set a citation must land in to carry authority
/// (`B1`). This is the tightest authoritative source available in a pack today;
/// the ledger mirror of `decisions.md` is exactly such a set of `ratify` decls.
pub(crate) fn declared_decisions(pack: &Pack) -> BTreeSet<&str> {
    pack.documents
        .iter()
        .flat_map(|document| &document.decls)
        .filter_map(|decl| match decl {
            Decl::Ratify(ratify) => Some(ratify.decision.text.as_str()),
            _ => None,
        })
        .collect()
}

/// The ledger valuation and its findings: each decision variable cited by a
/// `decided`, `ratified`, or `provisional` claim is ruled true **iff** the corpus
/// declares that decision (`known_decisions`); a citation to an undeclared
/// decision leaves its variable open and raises a `citation.unknown-decision`
/// finding (`B1`). This mirrors `dli_eval`'s `lower_fact_annotation`, so the two
/// lowerings converge.
///
/// The certain-layer atoms and sorts carry their citations as typed
/// [`LoadedAtom`](crate::model::LoadedAtom)/[`SortEntry`](crate::model::SortEntry)
/// provenance; a `term`, a `proof`'s premises, a `ratify … { }` block, and a
/// `rule`'s own provenance (WS-2) cite decisions too, but the loader does not
/// lift any of them into `atoms()`/`sorts()` (a rule is carriage-only — it
/// lowers to no atom of its own), so [`gate_language_citations`] closes that
/// hole over the raw declarations. Without it, a forged decision id on any of
/// these would mint authority the corpus never declared (`B1`) and escape the
/// invariant.
pub(crate) fn ledger_valuation(
    model: &WorldModel,
    pack: &Pack,
    known_decisions: &BTreeSet<&str>,
) -> (Valuation, Vec<LowerFinding>) {
    let mut nu = Valuation::new();
    let mut findings = Vec::new();
    for atom in model.atoms() {
        gate_citations(
            &mut nu,
            &mut findings,
            model,
            known_decisions,
            atom.kind,
            &atom.citations,
        );
    }
    for sort in model.sorts() {
        gate_citations(
            &mut nu,
            &mut findings,
            model,
            known_decisions,
            sort.kind,
            &sort.citations,
        );
    }
    gate_language_citations(&mut nu, &mut findings, model, pack, known_decisions);
    (nu, findings)
}

/// Extend the citation guard over the ledger-authority declarations the loader
/// keeps outside `atoms()`/`sorts()` — closing the `B1` hole the red team surfaced
/// (a forged authority claim escaping the "authority is declared, not minted"
/// invariant) — WITHOUT over-reaching into bibliographic provenance:
///
/// - a **`proof`**'s premises cite the certain ground they rest on
///   (`premise … cited <d-id>`), harvested from the captured body clauses; a forged
///   id would mint a diegetic proof's authority, so it is gated;
/// - a **`ratify … { }`** block mints its own decision (already declared), but any
///   decision its body clauses *cite* is gated the same way;
/// - a **`rule`**'s own provenance (WS-2) cites the ruling its derivations rest
///   on, but a rule is carriage-only — `process_rule` never calls `push_atom`,
///   so it has no entry in `model.atoms()` for the ordinary gate to see. Left
///   unclosed, a rule's interned citation variable (`compile_ann` interns it
///   unconditionally into the rule's own support polynomial, gated or not)
///   would NEVER be ruled true by this pass, so every wire-authored rule's
///   citation would be dead weight — riding along only when some OTHER atom
///   happens to cite the same decision, never gating anything on its own. This
///   arm is what makes `fire_rules`' multiplication of `rule.ann` into every
///   derivation mean something, and what `author.defeat` on a rule's own
///   citation has to flip.
///
/// **Not gated:** a `term`/`dialect`/`word`/`name-part`'s `decided <d-id>` is
/// *language-layer provenance* — a bibliographic pointer that may name a decision
/// recorded OUTSIDE this corpus. The vendored `TPtH` packs legitimately cite external
/// canon ids (e.g. `D-0181`) this way; gating them would break a partial/vendored
/// corpus that points at its home ledger. Provenance is not assertability authority.
///
/// Each gated id is routed through [`gate_citations`] exactly as an atom's would
/// be, so a declared id rules its variable and an undeclared one raises
/// `citation.unknown-decision`.
fn gate_language_citations(
    nu: &mut Valuation,
    findings: &mut Vec<LowerFinding>,
    model: &WorldModel,
    pack: &Pack,
    known_decisions: &BTreeSet<&str>,
) {
    for decl in pack.documents.iter().flat_map(|doc| &doc.decls) {
        match decl {
            Decl::Proof(proof) => {
                let citations = harvest_clause_citations(proof.body.clauses());
                gate_citations(
                    nu,
                    findings,
                    model,
                    known_decisions,
                    Provenance::Decided,
                    &citations,
                );
            }
            Decl::Ratify(ratify) => {
                // The parser (`dli_syntax::structure_ratify_body`) already
                // harvested the body's cited decision ids into the typed interior
                // (LS-9), so the gate reads them without re-scanning clause text.
                gate_citations(
                    nu,
                    findings,
                    model,
                    known_decisions,
                    Provenance::Decided,
                    &ratify.body.citations,
                );
            }
            Decl::Rule(rule) => {
                if let Some(annotation) = &rule.provenance {
                    let citations: Vec<String> = annotation
                        .citations
                        .iter()
                        .map(|citation| citation.id.text.clone())
                        .collect();
                    gate_citations(
                        nu,
                        findings,
                        model,
                        known_decisions,
                        Provenance::from_marker(annotation.marker),
                        &citations,
                    );
                }
            }
            // `inertial <pred>` is carriage-only — like a rule, it never reaches
            // `model.atoms()`, so the ordinary atom gate cannot see its
            // citation. Gate it here so an `inertial pred ratified D-…` reads
            // certain (capping active, DL-0081) when the decision is declared,
            // and raises `citation.unknown-decision` when it is not — the load-
            // path twin of the stage-time `decl_annotation` guard.
            Decl::Inertial(inertial) => {
                if let Some(annotation) = &inertial.provenance {
                    let citations: Vec<String> = annotation
                        .citations
                        .iter()
                        .map(|citation| citation.id.text.clone())
                        .collect();
                    gate_citations(
                        nu,
                        findings,
                        model,
                        known_decisions,
                        Provenance::from_marker(annotation.marker),
                        &citations,
                    );
                }
            }
            // Predicate identity is structured carriage-only, like `inertial`:
            // its citation never reaches `model.atoms()`, but it still gates
            // the identity link's evaluator support and must rule its decision
            // variable true (or surface the unknown citation) here.
            Decl::PredicateSameAs(identity) => {
                if let Some(annotation) = &identity.provenance {
                    let citations: Vec<String> = annotation
                        .citations
                        .iter()
                        .map(|citation| citation.id.text.clone())
                        .collect();
                    gate_citations(
                        nu,
                        findings,
                        model,
                        known_decisions,
                        Provenance::from_marker(annotation.marker),
                        &citations,
                    );
                }
            }
            _ => {}
        }
    }
}

/// The decision ids a run of captured body clauses cite: the token after each
/// `cited`/`decided`/`ratified`/`provisional` keyword read at top level, OUTSIDE any
/// string literal. Two disciplines guard it: paren depth (a keyword inside a nested
/// atom `p(cited)` is not a citation) AND string state (a keyword or paren inside a
/// `"…"` cue/label is inert). Without the string guard a cue like `"the king decided
/// to grant it"` would mint a spurious citation, and an unbalanced `(` inside a cue
/// string would hide a real forged `cited` after it.
fn harvest_clause_citations<'a>(
    clauses: impl IntoIterator<Item = &'a dli_ast::clause::Clause>,
) -> Vec<String> {
    const CITE_KEYWORDS: &[&str] = &["cited", "decided", "ratified", "provisional"];
    let mut out = Vec::new();
    for clause in clauses {
        let tokens: Vec<&str> = clause.text.split_whitespace().collect();
        let mut depth: i32 = 0;
        let mut in_str = false;
        let mut index = 0;
        while index < tokens.len() {
            let token = tokens[index];
            // A keyword counts only at top level and outside a string literal.
            if depth == 0
                && !in_str
                && CITE_KEYWORDS.contains(&token)
                && let Some(id) = tokens.get(index + 1)
            {
                out.push((*id).to_owned());
                index += 2;
                continue;
            }
            // Walk the token's characters, toggling string state on each quote; a
            // paren only moves the depth when we are NOT inside a string.
            for ch in token.chars() {
                match ch {
                    '"' => in_str = !in_str,
                    '(' if !in_str => depth += 1,
                    ')' if !in_str => depth = (depth - 1).max(0),
                    _ => {}
                }
            }
            index += 1;
        }
    }
    out
}

/// Rule each of a ledger claim's cited decisions true when it is declared,
/// otherwise record a forged-authority finding. Non-ledger provenance rules
/// nothing.
fn gate_citations(
    nu: &mut Valuation,
    findings: &mut Vec<LowerFinding>,
    model: &WorldModel,
    known_decisions: &BTreeSet<&str>,
    kind: Provenance,
    citations: &[String],
) {
    if !matches!(
        kind,
        Provenance::Decided | Provenance::Ratified | Provenance::Provisional
    ) {
        return;
    }
    for citation in citations {
        let id = citation.trim();
        if known_decisions.contains(id) {
            if let Some(var) = model.var_atom(id) {
                nu.set(var, true);
            }
        } else {
            findings.push(LowerFinding {
                code: "citation.unknown-decision".to_owned(),
                message: format!("cited decision '{id}' is not a declared decision"),
            });
        }
    }
}

/// Recompute which stored attestations are stale against the current model —
/// the re-attestation queue.
///
/// A pure read over the store and the world: for each question's head record,
/// recompute the current fingerprint of the world's facts about its subject and
/// return the record iff it no longer matches what was fingerprinted when the
/// attestation was taken. The kernel calls no model here; a sweep in the factory
/// sense answers this queue by re-posing the questions and committing records.
pub fn sweep(store: &AttestationStore, model: &WorldModel) -> Vec<Attestation> {
    store
        .heads()
        .into_iter()
        .filter(|head| {
            let current = model.subject_fingerprint(head.subject());
            head.is_stale(&current)
        })
        .cloned()
        .collect()
}

/// Fact-check a draft payload against the compiled world — the merged `CHECK-1`
/// battery (v0).
///
/// Runs the fact-checker over the draft's claims and the structural continuity
/// classes over the same claims, plus the killed-token guardrail over introduced
/// names, and returns one merged list of defects (supported claims are silent). A
/// contradicted or unsupported claim, a claim on an unknown entity, a claim that
/// states shadow lore as settled, a claim set that sets one slot two ways, and an
/// introduced name that revives a killed token each surface here. The deeper
/// season battery (outline-conformance, craft rules, proof checks) lands in
/// `PAY-2`.
#[must_use]
pub fn check_draft(world: &CompiledWorld, draft: &DraftPayload) -> Vec<DraftFinding> {
    let mut out = Vec::new();
    for report in check::factcheck::factcheck(world, &draft.claims) {
        if report.is_defect() {
            out.push(report.to_draft_finding());
        }
    }
    for finding in check::continuity::check_claims(world, &draft.claims) {
        out.push(DraftFinding {
            claim: finding.code.clone(),
            grade: finding.grade,
            message: finding.message,
        });
    }
    for name in &draft.names {
        let hits = world.killed.hits(name).collect::<Vec<_>>();
        if !hits.is_empty() {
            let rejected = hits
                .iter()
                .map(|hit| format!("'{}' ({}): {}", hit.token, hit.decision, hit.scope))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join("; ");
            out.push(DraftFinding {
                claim: name.clone(),
                grade: Grade::Checked,
                message: format!(
                    "introduced name '{name}' realizes killed token ruling(s): {rejected}"
                ),
            });
        }
    }
    out
}
