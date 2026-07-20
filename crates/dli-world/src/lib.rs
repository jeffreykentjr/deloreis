//! Deloreis Lore Interchange world model.
//!
//! A [`Pack`] of documents [`compile`]s into a [`CompiledWorld`]: the loader's
//! status-gated [`WorldModel`] with its ledger valuation set and its symbolic
//! fixpoint computed, plus the compiled language layer, craft rules, and the
//! season ledger. Drafts arrive as [`DraftPayload`]s and are fact-checked with
//! [`check_draft`]; [`Attestation`]s are recomputed for staleness with [`sweep`].

pub mod attest;
pub mod check;
pub mod craft;
pub mod draft;
pub mod episode;
pub mod episode_order;
pub mod language;
pub mod loader;
pub mod model;
pub mod pack;
pub mod payload;

pub use attest::{Attestation, AttestationStore};
pub use check::{
    CompiledWorld, Severity, WorldFinding,
    factcheck::{ClaimReport, KilledToken, KilledTokens, MatchMode},
};
pub use draft::{DraftFinding, DraftMeta, DraftPayload};
pub use language::{
    DialectDef, LangFinding, LanguageModel, LanguageOutcome, NameDef, RatifiedName,
};
pub use model::{CraftRule, SeasonLedger, WorldModel};
pub use pack::Pack;

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
/// # Errors
///
/// Returns [`EvalError::Unstratifiable`] if the world's rules admit no valid
/// stratification. Malformed inputs are never errors — they are load
/// [`findings`](WorldModel::findings) held fail-closed.
pub fn compile(pack: &Pack) -> Result<CompiledWorld, EvalError> {
    let known_decisions = declared_decisions(pack);
    let mut model = Loader::load(&pack.documents).world;
    let (valuation, findings) = ledger_valuation(&model, &known_decisions);
    model.world.valuation = valuation;
    model.world.findings.extend(findings);
    // Saturate the loader's status-gated facts; the clone in `fixpoint` carries
    // the valuation and findings through, so the compiled world evaluates under
    // the ledger.
    model.world = model.world.fixpoint()?;
    Ok(CompiledWorld::new(model))
}

/// The decision ids the corpus actually declares via `ratify <d-id>` — the
/// authoritative decision set a citation must land in to carry authority
/// (`B1`). This is the tightest authoritative source available in a pack today;
/// the ledger mirror of `decisions.md` is exactly such a set of `ratify` decls.
fn declared_decisions(pack: &Pack) -> BTreeSet<&str> {
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
fn ledger_valuation(
    model: &WorldModel,
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
    (nu, findings)
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
            let current = crate::attest::fingerprint_facts(
                model
                    .facts_about(head.subject())
                    .into_iter()
                    .map(|loaded| &loaded.atom),
            );
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
        if let Some(hit) = world.killed.hit(name) {
            out.push(DraftFinding {
                claim: name.clone(),
                grade: Grade::Checked,
                message: format!(
                    "introduced name '{name}' realizes killed token '{}' ({}): {}",
                    hit.token, hit.decision, hit.scope
                ),
            });
        }
    }
    out
}
