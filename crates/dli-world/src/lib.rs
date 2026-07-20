//! Deloreis Lore Interchange world model.
//!
//! A [`Pack`] of documents [`compile`]s into a [`CompiledWorld`]: the loader's
//! status-gated [`WorldModel`] with its ledger valuation set and its symbolic
//! fixpoint computed, plus the compiled language layer, craft rules, and the
//! season ledger. Drafts arrive as [`DraftPayload`]s and are fact-checked with
//! [`check_draft`]; [`Attestation`]s are recomputed for staleness with [`sweep`].

pub mod attest;
pub mod check;
pub mod draft;
pub mod episode;
pub mod language;
pub mod loader;
pub mod model;
pub mod pack;

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

use dli_core::grade::Grade;
use dli_core::valuation::Valuation;
use dli_eval::error::EvalError;

use crate::loader::Loader;
use crate::model::Provenance;

/// Compile a pack into a [`CompiledWorld`].
///
/// This is the single lowering the whole fabric shares (the `CHECK-1` carry
/// item): [`Loader::load`] gates every claim on its ratification status, then
/// compile adds the two things a raw load lacks — a **ledger valuation** that
/// rules every cited decision variable true (so ledger-marked claims read
/// certain, not merely possible), and the **symbolic fixpoint** over the loader's
/// own status-gated facts. Reconciliation is by *reuse*, not re-lowering: the
/// loader already pushes each fact's gated support into `world.world.facts`, so
/// the evaluator saturates the very same atoms. After compile, a claim's standing
/// read off the loader's atoms and read off the evaluated world agree exactly for
/// the same document and valuation.
///
/// # Errors
///
/// Returns [`EvalError::Unstratifiable`] if the world's rules admit no valid
/// stratification. Malformed inputs are never errors — they are load
/// [`findings`](WorldModel::findings) held fail-closed.
pub fn compile(pack: &Pack) -> Result<CompiledWorld, EvalError> {
    let mut model = Loader::load(&pack.documents).world;
    model.world.valuation = ledger_valuation(&model);
    // Saturate the loader's status-gated facts; the clone in `fixpoint` carries
    // the valuation through, so the compiled world evaluates under the ledger.
    model.world = model.world.fixpoint()?;
    Ok(CompiledWorld::new(model))
}

/// The ledger valuation: every decision variable cited by a `decided`,
/// `ratified`, or `provisional` claim, ruled true. This matches
/// `dli_eval`'s `lower_fact_annotation` (citing a ruling asserts the ruling
/// exists), so the loader and evaluator representations converge.
fn ledger_valuation(model: &WorldModel) -> Valuation {
    let mut nu = Valuation::new();
    let mut rule_true = |kind: Provenance, citations: &[String]| {
        if matches!(
            kind,
            Provenance::Decided | Provenance::Ratified | Provenance::Provisional
        ) {
            for citation in citations {
                if let Some(var) = model.var_atom(citation.trim()) {
                    nu.set(var, true);
                }
            }
        }
    };
    for atom in model.atoms() {
        rule_true(atom.kind, &atom.citations);
    }
    for sort in model.sorts() {
        rule_true(sort.kind, &sort.citations);
    }
    nu
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
