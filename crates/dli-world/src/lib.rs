//! Deloreis Lore Interchange world model.
//!
//! A [`Pack`] of documents compiles into a [`CompiledWorld`] with provenance,
//! chronology, language and a symbolic fixpoint. Attestations can be checked
//! for staleness against the compiled world.

pub mod attest;
pub mod check;
pub mod draft;
pub mod episode;
pub mod episode_order;
mod era_order;
pub mod incremental;
pub mod language;
pub mod loader;
pub mod model;
pub mod pack;
pub mod predicate_identity;
pub mod scene_projection;

pub use attest::{Attestation, AttestationStore};
pub use check::{
    CompiledWorld, Severity, WorldFinding,
    factcheck::{ClaimReport, KilledToken, KilledTokens, MatchMode, RejectedClaim},
};
pub use draft::{DraftFinding, DraftMeta, DraftPayload};
pub use incremental::{CompileReuse, compile_reusing_fixpoint};
pub use language::{
    DialectDef, LangFinding, LanguageModel, LanguageOutcome, NameDef, RatifiedName,
};
pub use model::WorldModel;
pub use pack::Pack;
pub use predicate_identity::{
    PredicateArityMismatch, PredicateIdentityLink, PredicateIdentityState,
};

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
/// optional seam that reuses a prior fixpoint is [`compile_reusing_fixpoint`]. Both
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
    seal_loaded(model, pack)
}

/// Finish a world already produced by [`Loader::load`] with the ledger
/// valuation and symbolic fixpoint.
///
/// This pure phase boundary exists so non-gating release profiles can measure
/// loader work separately from fixpoint-and-seal work without duplicating the
/// compiler. Ordinary callers should prefer [`compile`].
///
/// # Errors
///
/// Returns [`EvalError::Unstratifiable`] under the same conditions as
/// [`compile`].
pub fn seal_loaded(model: WorldModel, pack: &Pack) -> Result<CompiledWorld, EvalError> {
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
            atom.provenance_span,
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
            sort.span,
        );
    }
    gate_language_citations(&mut nu, &mut findings, model, pack, known_decisions);
    findings.extend(unsupported_world_hypothesis_rulings(pack));
    (nu, findings)
}

/// Report text ratify assignments that cannot affect the referenced world claims.
/// The assignments currently change the language model's valuation only.
#[must_use]
pub fn unsupported_world_hypothesis_rulings(pack: &Pack) -> Vec<LowerFinding> {
    use dli_ast::annot::{Annotation, CondExpr};

    fn collect(expr: &CondExpr, names: &mut BTreeSet<String>) {
        match expr {
            CondExpr::Hyp(id) => {
                names.insert(id.text.clone());
            }
            CondExpr::Not(inner) => collect(inner, names),
            CondExpr::And(items) | CondExpr::Or(items) => {
                for item in items {
                    collect(item, names);
                }
            }
        }
    }
    fn annotation(annotation: Option<&Annotation>, names: &mut BTreeSet<String>) {
        if let Some(annotation) = annotation {
            if let Some(condition) = annotation.condition.as_ref() {
                collect(condition, names);
            } else if annotation.marker == dli_ast::annot::ProvenanceMarker::Proposed
                && !annotation.tags.is_empty()
            {
                names.insert(format!("h.{}", annotation.tags.join("+")));
            }
        }
    }

    let mut world_conditions = BTreeSet::new();
    for decl in pack.documents.iter().flat_map(|document| &document.decls) {
        let provenance = match decl {
            Decl::Entity(entity) => {
                for slot in &entity.slots {
                    annotation(slot.provenance.as_ref(), &mut world_conditions);
                }
                entity.provenance.as_ref()
            }
            Decl::Attr(d) => d.provenance.as_ref(),
            Decl::Fact(d) => d.provenance.as_ref(),
            Decl::State(d) => d.provenance.as_ref(),
            Decl::Before(d) => d.provenance.as_ref(),
            Decl::SameAs(d) => d.provenance.as_ref(),
            Decl::PredicateSameAs(d) => d.provenance.as_ref(),
            Decl::Functional(d) => d.provenance.as_ref(),
            Decl::Inertial(d) => d.provenance.as_ref(),
            Decl::Kind(d) => d.provenance.as_ref(),
            Decl::Rule(d) => d.provenance.as_ref(),
            Decl::Cycle(d) => d.provenance.as_ref(),
            Decl::Question(d) => d.provenance.as_ref(),
            Decl::Fork(d) => d.provenance.as_ref(),
            _ => None,
        };
        annotation(provenance, &mut world_conditions);
    }
    let known_decisions = declared_decisions(pack);
    let mut findings = Vec::new();
    for decl in pack.documents.iter().flat_map(|document| &document.decls) {
        if let Decl::Ratify(ratify) = decl {
            for line in &ratify.body.clauses {
                if let Some(assignment) = &line.assignment {
                    if known_decisions.contains(assignment.hyp.as_str()) {
                        findings.push(LowerFinding {
                            code: "ratify.decision-as-hypothesis".to_owned(),
                            message: format!("'{}' is a decision, not a hypothesis; use author.defeat to revise a decision", assignment.hyp),
                            span: Some(line.clause.span),
                        });
                    }
                    if !world_conditions.contains(&assignment.hyp) {
                        continue;
                    }
                    findings.push(LowerFinding {
                        code: "ratify.world-hypothesis-unsupported".to_owned(),
                        message: format!("`ratify` does not settle world claims that assume '{}'; leave the claim proposed or record a decision for it", assignment.hyp),
                        span: Some(line.clause.span),
                    });
                }
            }
        }
    }
    findings
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
/// recorded outside this corpus. Gating them would break a partial corpus that
/// points at its home ledger. Provenance is not assertability authority.
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
                for clause in proof.body.clauses() {
                    let citations = harvest_clause_citations(std::iter::once(clause));
                    gate_citations(
                        nu,
                        findings,
                        model,
                        known_decisions,
                        Provenance::Decided,
                        &citations,
                        Some(clause.span),
                    );
                }
            }
            Decl::Ratify(ratify) => {
                // The parser (`dli_syntax::structure_ratify_body`) already
                // harvested the body's cited decision ids into the typed interior
                // (LS-9), so the gate reads them without re-scanning clause text.
                let mut sources =
                    std::collections::BTreeMap::<String, std::collections::VecDeque<_>>::new();
                for line in &ratify.body.clauses {
                    for citation in harvest_clause_citations(std::iter::once(&line.clause)) {
                        sources
                            .entry(citation)
                            .or_default()
                            .push_back(line.clause.span);
                    }
                }
                for citation in &ratify.body.citations {
                    let span = sources
                        .get_mut(citation)
                        .and_then(std::collections::VecDeque::pop_front);
                    gate_citations(
                        nu,
                        findings,
                        model,
                        known_decisions,
                        Provenance::Decided,
                        std::slice::from_ref(citation),
                        span,
                    );
                }
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
                        Some(annotation.span),
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
                        Some(annotation.span),
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
                        Some(annotation.span),
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
    span: Option<dli_ast::span::Span>,
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
                span: span.filter(|span| span.end > span.start),
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
/// attestation was taken. The kernel calls no model here; a caller-side sweep
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
/// introduced name that revives a killed token each surface here.
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
