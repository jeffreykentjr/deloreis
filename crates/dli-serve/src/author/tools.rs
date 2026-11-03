//! The authoring seam's tools (`AUTH-1`): the fourteen `author.*` tools and their immediate
//! graded findings.
//!
//! Each tool lowers an agent intent to typed AST, stages it over the workspace's
//! base, checks the staged layer, and answers with the feedback contract of spec
//! 11 §4: graded findings (data, never instructions), the rendered canonical
//! surface of what it staged, and the new workspace digest. Nothing an agent
//! supplies mints authority — every claim enters `proposed` or is
//! `hypothesis`-scoped (§5) — and instruction-shaped strings in parameters are
//! stored and rendered as data, never interpreted (§5, framing safety).
//!
//! Fail-closed (§4): a malformed call rejects whole (a `schema`/`refused` wire
//! error, staging nothing); an **error-severity finding** stages the claim but
//! bars `author.materialize` until the world is made consistent again.

use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use serde_json::{Value, json};

use dli_ast::annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{
    AttestDecl, AttrDecl, BannedDecl, BeforeDecl, Decl, DialectDecl, Document, EntityDecl,
    EntityKind as AstEntityKind, FactDecl, FunctionalDecl, HypothesisDecl, InertialDecl, NameDecl,
    NamePartDecl, OptionClause, PredicateSameAsDecl, QuestionDecl, RatifyDecl, RuleDecl,
    SameAsDecl, ScopeHeader, StateDecl, TermDecl, TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::language::{
    CompositionStep, DialectBody, DialectClause, DialectItem, NameBody, NameClause, NameItem,
    NameKindSpec,
};
use dli_ast::rule::{Datalog, RuleBody, RuleClause};
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::grade::Grade;
use dli_eval::EvalError;
use dli_world::CompiledWorld;
use dli_world::check::factcheck::{Verdict, factcheck};
use dli_world::compile;
use dli_world::pack::Pack;

use crate::contract::{
    AuthorAddAttestParams, AuthorAddBannedParams, AuthorAddBeforeParams, AuthorAddDialectParams,
    AuthorAddEntityParams, AuthorAddEventParams, AuthorAddFactParams, AuthorAddHypothesisParams,
    AuthorAddNameParams, AuthorAddNamePartParams, AuthorAddOptionParams, AuthorAddRelationParams,
    AuthorAddRuleParams, AuthorAddStateParams, AuthorAddTermParams,
    AuthorAssertPredicateSameAsParams, AuthorAssertSameAsParams, AuthorBranchParams,
    AuthorBranchResult, AuthorDeclareFunctionalParams, AuthorDeclareInertialParams,
    AuthorDefeatParams, AuthorDiffParams, AuthorDiffResult, AuthorMaterializeParams,
    AuthorMergeParams, AuthorMergeResult, AuthorMutationResult, AuthorOpenParams,
    AuthorOpenQuestionParams, AuthorOpenResult, AuthorRecordDecisionParams,
    AuthorRecordDecisionsParams, AuthorRecordDecisionsResult, AuthorSetStandingParams,
    AuthorStatusParams, AuthorStatusResult, AuthorSupposeParams, BranchAction, BranchHead,
    DiffConflict, EntityKind as WireEntityKind, MergeStandingChange, StagedDecl, StateTimeKind,
};
use crate::envelope::{ErrorBody, ErrorCode};
use crate::finding::{Finding, Provenance, ProvenanceState, Severity};
use dli_eval::fork::Conflict;
use dli_world::check::{Severity as WorldSeverity, WorldFinding, fd_value_key};

use super::reads;
use super::render::{render_decls, render_option, render_scoped};
use super::store::Store;
use super::workspace::{Mode, Workspace};

// -- author.open -------------------------------------------------------------

/// Open a workspace over the store's base, in `scope` or `fork` mode (spec 11
/// §2). The `scope` param stages over an existing scope (a pack, in DL-0016's
/// renamed surface); `fork` opens a fork-scoped workspace.
///
/// `branch` (WS-3, ≥ contract 1.7.0 — `branch_writes` gates it) opens the
/// workspace against a named branch head instead of the project document's
/// main lorestore ref: it requires `scope` (branch + fork is refused — there
/// is no third meaning of "fork"), an attached lorestore
/// ([`ErrorCode::ProjectRequired`]), and an existing branch
/// ([`ErrorCode::UnknownDoc`], naming the branch). Below 1.7.0, supplying
/// `branch` at all is a schema violation — the same "explicit version check
/// in the open arm" shape the provenance.state gate in `precheck` already
/// uses.
pub(super) fn open(
    store: &mut Store,
    params: &AuthorOpenParams,
    branch_writes: bool,
) -> Result<Value, ErrorBody> {
    if params.branch.is_some() && !branch_writes {
        return Err(schema_error(
            "field `branch` is defined only by contract 1.7.0 branch-scoped writes",
        ));
    }
    let (mode, base_name) = match (&params.scope, &params.fork) {
        (Some(scope), None) => (Mode::Scope, scope.clone()),
        (None, Some(fork)) => (Mode::Fork, fork.clone()),
        _ => {
            return Err(schema_error(
                "author.open needs exactly one of `scope` or `fork`",
            ));
        }
    };
    if params.branch.is_some() && matches!(mode, Mode::Fork) {
        return Err(schema_error(
            "author.open's `branch` requires `scope`; a fork workspace does not target a branch",
        ));
    }
    let ws = store
        .open(mode, base_name, params.branch.clone())
        .map_err(|err| open_error(&err))?;
    Ok(value(&AuthorOpenResult {
        workspace: ws.id.clone(),
        workspace_digest: ws.digest.clone(),
        branch: branch_writes.then(|| ws.target_branch.clone()).flatten(),
    }))
}

/// Map a [`super::store::OpenError`] to its wire error. `UnknownDoc` keeps
/// the pre-1.7.0 message byte-for-byte (the error type's `Display` impl
/// carries it verbatim); the three branch-specific outcomes are new (WS-3).
fn open_error(err: &super::store::OpenError) -> ErrorBody {
    use super::store::OpenError as OE;
    let message = err.to_string();
    let code = match err {
        OE::UnknownDoc(_) | OE::UnknownBranch(_) => ErrorCode::UnknownDoc,
        OE::ProjectRequired => ErrorCode::ProjectRequired,
        OE::BranchCompile(_) => ErrorCode::Internal,
        OE::ValuationRemap(_) => ErrorCode::EvidenceCorrupt,
    };
    ErrorBody {
        code,
        message,
        data: None,
    }
}

// -- author.status / author.diff (read-only) ---------------------------------

/// The workspace's staged canonical surface and current digest. On a 0.6.0
/// request the result also carries the pinned base commit and the structured
/// diff rows (the review surface).
pub(super) fn status(
    store: &Store,
    params: &AuthorStatusParams,
    v2: bool,
    structural_keys: bool,
    carries_data: bool,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let report = reads::workspace_report(store, ws, carries_data);
    Ok(value(&AuthorStatusResult {
        rendered: render_decls(&ws.decls),
        root_hash: if v2 { ws.base_root_hash.clone() } else { None },
        diff_rows: v2.then_some(report.diff_rows),
        staged: structural_keys.then(|| ws.decls.iter().map(staged_decl).collect()),
        findings: report.findings,
        workspace_digest: ws.digest.clone(),
    }))
}

/// One staged declaration's wire row: its structural key, its kind, and its
/// rendered line where a formatter exists to produce one.
fn staged_decl(decl: &Decl) -> StagedDecl {
    StagedDecl {
        key: decl_key(decl),
        kind: decl_kind(decl).to_owned(),
        rendered: rendered_key(decl),
    }
}

/// A declaration's kind word — the `.dli` keyword that introduces it.
fn decl_kind(decl: &Decl) -> &'static str {
    match decl {
        Decl::Entity(e) => e.kind.keyword(),
        Decl::Attr(_) => "attr",
        Decl::Fact(_) => "fact",
        Decl::Question(_) => "question",
        Decl::Before(_) => "before",
        Decl::SameAs(_) => "same-as",
        Decl::PredicateSameAs(_) => "predicate-same-as",
        Decl::Hypothesis(_) => "hypothesis",
        Decl::Use(_) => "use",
        Decl::Kind(_) => "kind",
        Decl::Rule(_) => "rule",
        Decl::State(_) => "state",
        Decl::Functional(_) => "functional",
        Decl::Inertial(_) => "inertial",
        Decl::Cycle(_) => "cycle",
        Decl::Fork(_) => "fork",
        Decl::Practice(_) => "practice",
        Decl::Dialect(_) => "dialect",
        Decl::Word(_) => "word",
        Decl::NamePart(_) => "name-part",
        Decl::Draft(_) => "draft",
        Decl::Attest(_) => "attest",
        Decl::Export(_) => "export",
        Decl::Name(_) => "name",
        Decl::Term(_) => "term",
        Decl::Banned(_) => "banned",
        Decl::Narration(_) => "narration",
        Decl::Variant(_) => "variant",
        Decl::Ratify(_) => "ratify",
        Decl::Principle(_) => "principle",
        Decl::Proof(_) => "proof",
    }
}

/// The rendered canonical block, findings, and consequences of the staged change.
/// On a 0.6.0 request the result also carries the pinned base commit and the
/// structured per-fact rows.
pub(super) fn diff(
    store: &Store,
    params: &AuthorDiffParams,
    v2: bool,
    carries_data: bool,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let report = reads::workspace_report(store, ws, carries_data);
    Ok(value(&AuthorDiffResult {
        rendered: render_decls(&ws.decls),
        root_hash: if v2 { ws.base_root_hash.clone() } else { None },
        rows: v2.then_some(report.diff_rows),
        findings: report.findings,
        consequences: report.consequences,
        workspace_digest: ws.digest.clone(),
    }))
}

// -- author.discard ----------------------------------------------------------

/// Discard a workspace (R3/C3). With no `decl`, drops the whole workspace and
/// answers `{discarded}`. With a `decl` — the rendered canonical line of one
/// staged declaration — drops only that declaration, leaving the workspace open,
/// and answers `{discarded, workspace_digest}`; a `decl` that matches nothing is
/// an `unknown-doc` refusal (it never falls through to nuking the workspace).
///
/// # Naming one declaration, by version
///
/// **At contract ≥ 1.3.0** a declaration is named by its **structural key** —
/// the JCS digest [`decl_key`] computes — which `author.status` lists beside
/// each staged declaration. The key exists on every build, so this form works
/// on a default build for the first time (the DM-3 deferral, closed by
/// DL-0058). A `syntax` build additionally still accepts the rendered line, so
/// a caller that negotiates up loses nothing it could do before.
///
/// **Below 1.3.0** only the rendered line is accepted, and a build that renders
/// no `.dli` (LS-7) refuses by name rather than matching nothing and answering
/// `unknown-doc` — which would read as "that declaration is not staged" when the
/// truth is "this build cannot name it". Those bytes are frozen: that refusal
/// shipped on every wire through 1.2.0.
pub(super) fn discard(
    store: &mut Store,
    ws_id: &str,
    decl: Option<&str>,
    structural_keys: bool,
) -> Result<Value, ErrorBody> {
    let Some(decl_id) = decl else {
        return Ok(json!({ "discarded": store.discard(ws_id) }));
    };
    if !structural_keys && !super::render::renders_dli() {
        return Err(ErrorBody {
            code: ErrorCode::SchemaViolation,
            message: "per-declaration `discard` names a staged declaration by its rendered `.dli` \
                      line, and this build links no formatter; discard the whole workspace, or \
                      use a `syntax`-enabled build"
                .to_owned(),
            data: None,
        });
    }
    let (_, ws) = parts_mut(store, ws_id)?;
    let index = ws.decls.iter().position(|staged| {
        let by_structure = structural_keys && decl_key(staged) == decl_id;
        by_structure || rendered_key(staged).is_some_and(|line| line == decl_id)
    });
    match index {
        Some(i) => {
            // Take the key from the declaration being removed rather than from
            // the caller's string: the two are the same only when the caller
            // named it structurally, and the durable bookkeeping is keyed
            // structurally either way.
            let key = ws.decls.get(i).map(decl_key);
            ws.remove_decl(i);
            // The reject drops the durably staged group too: a rejected claim
            // never reaches a commit (LS-5 accept/reject; the object itself
            // becomes an orphan, garbage never corruption).
            if let Some(pos) = key.and_then(|key| {
                ws.staged_fact_hashes
                    .iter()
                    .position(|(staged, _)| *staged == key)
            }) {
                ws.staged_fact_hashes.remove(pos);
            }
            // A structural change invalidates every cached replay: a later retry
            // must re-execute against the new state, not serve a stale result for
            // a declaration that is no longer present (R3/B5).
            ws.idempotency.clear();
            Ok(json!({ "discarded": true, "workspace_digest": ws.digest }))
        }
        None => Err(unknown_decl(decl_id)),
    }
}

// -- author.materialize ------------------------------------------------------

/// Materialize the workspace: commit the staged delta to the lorestore and
/// return the committed fact objects **as data** — the kernel writes no files
/// (spec 11 §4). Refuses with `materialize-blocked` when any error-severity
/// finding is open; the blocking finding codes ride in the error data.
///
/// Digest regimes stay separate (audit A4, DL-0045 ruling 3): the returned
/// `workspace_digest` is the fence-valid `jcs:sha256:` workspace digest —
/// materialize advances its base commit, so it returns the newly sealed digest
/// to use as the next call's fence. The lorestore commit hash travels in its
/// own `root_hash` field.
///
/// Re-materializing with nothing newly staged is an idempotent no-op: no new
/// commit is written (audit B9) and the pinned commit is returned.
///
/// A workspace with **no lorestore** (demo/test contexts) materializes as the
/// rendering alone — and a build that renders no `.dli` (LS-7) therefore has
/// nothing to return, so it refuses by name rather than answering an empty
/// success. That refusal is the default build's only divergence here; the
/// durable, lorestore-backed path is identical on both builds.
#[allow(clippy::too_many_lines)]
pub(super) fn materialize(
    store: &mut Store,
    params: &AuthorMaterializeParams,
    v2: bool,
    records_decisions: bool,
) -> Result<Value, ErrorBody> {
    // The `decision` param (accept the whole committed set under a named,
    // recorded, defeasible decision) is defined only by contract 1.9.0
    // (DL-0083); supplying it on an earlier accepted version is a schema
    // violation, floor-gated the same way `state`/`branch` params are.
    if params.decision.is_some() && !records_decisions {
        return Err(schema_error(
            "field `decision` is defined only by contract 1.9.0 decision-stamped materialize",
        ));
    }
    // Without a lorestore the rendering IS the materialization (demo/test
    // contexts): version-independent legacy shape.
    if store.workspace_lorestore(&params.workspace).is_none() {
        let ws = store.get(&params.workspace).ok_or_else(gone)?;
        let blocking = dedup(
            reads::workspace_report(store, ws, false)
                .findings
                .into_iter()
                .filter(|finding| finding.severity == Severity::Error)
                .map(|finding| finding.code),
        );
        if !blocking.is_empty() {
            return Err(ErrorBody {
                code: ErrorCode::MaterializeBlocked,
                message: format!(
                    "{} error-severity finding(s) are unresolved; resolve them or discard before materializing.",
                    blocking.len()
                ),
                data: Some(json!({ "blocking": blocking })),
            });
        }
        // …and a build that renders no `.dli` has neither half: nothing to
        // commit and nothing to render. Answering `{findings: [], rendered:
        // []}` would be a success with no content — the author would read an
        // empty `findings` list as "materialized clean" when in truth nothing
        // was durably written anywhere. Refuse by name instead, exactly as
        // per-declaration `author.discard` does above (LS-7): the build says
        // what it cannot do and names the two ways forward.
        if !super::render::renders_dli() {
            return Err(ErrorBody {
                code: ErrorCode::SchemaViolation,
                message: "this workspace has no lorestore attached, so `materialize` has nothing \
                          to commit to, and this build links no formatter, so it has no `.dli` \
                          rendering to return in place of one; attach a lorestore, or use a \
                          `syntax`-enabled build"
                    .to_owned(),
                data: None,
            });
        }
        return Ok(value(&AuthorMutationResult {
            findings: Vec::new(),
            rendered: render_scoped(&ws.base_name, &ws.decls),
            workspace_digest: ws.digest.clone(),
            root_hash: None,
            facts: None,
        }));
    }

    let (
        base_name,
        staged_decls,
        staged_fact_hashes,
        ws_interner,
        ws_digest,
        pinned,
        base_ref_digest,
        target_ref,
    ) = {
        let ws = store.get(&params.workspace).ok_or_else(gone)?;
        (
            ws.base_name.clone(),
            ws.decls.clone(),
            ws.staged_fact_hashes
                .iter()
                .flat_map(|(_, hashes)| hashes.iter().cloned())
                .collect::<Vec<_>>(),
            ws.interner.names().to_vec(),
            ws.digest.clone(),
            ws.base_root_hash.clone(),
            ws.base_ref_digest.clone(),
            target_ref_name(ws).to_owned(),
        )
    };

    // JSON-first (0.6.0): the rendered lines are the opt-in, the fact objects
    // the default. A 0.5.1 request always renders.
    let rendered = if v2 && !params.rendered {
        Vec::new()
    } else {
        render_scoped(&base_name, &staged_decls)
    };

    // Idempotent empty delta: nothing staged since the last commit.
    if staged_fact_hashes.is_empty() {
        // Some deliberately non-durable authoring state (notably a hypothesis
        // outside a fork) has no fact object. It must still pass the standing
        // materialize oracle before the empty-object fast path answers success.
        // There is no candidate fold or CAS in this arm, so the workspace's
        // current base is the exact base relevant to these workspace-only
        // findings.
        if !staged_decls.is_empty() {
            let ws = store.get(&params.workspace).ok_or_else(gone)?;
            let blocking = dedup(
                reads::workspace_report(store, ws, false)
                    .findings
                    .into_iter()
                    .filter(|finding| finding.severity == Severity::Error)
                    .map(|finding| finding.code),
            );
            if !blocking.is_empty() {
                return Err(ErrorBody {
                    code: ErrorCode::MaterializeBlocked,
                    message: format!(
                        "{} error-severity finding(s) are unresolved; resolve them or discard before materializing.",
                        blocking.len()
                    ),
                    data: Some(json!({ "blocking": blocking })),
                });
            }
        }
        return Ok(value(&AuthorMutationResult {
            findings: Vec::new(),
            rendered,
            workspace_digest: ws_digest,
            root_hash: if v2 { pinned } else { None },
            facts: v2.then(Vec::new),
        }));
    }

    // Snapshot the exact target ref that will parent this materialize. Ordinary
    // main workspaces intentionally carry no open-time ref fence, so they bind
    // to the current main here; branch and pinned workspaces keep their stricter
    // open-time fence. In both cases the snapshot's whole-ref digest fences the
    // later CAS, making preparation and commit one coherent view.
    let (
        target_snapshot,
        target_ref_digest,
        target_names,
        mut decisions,
        target_parent_pack,
        mut target_parent_world,
        target_valuation,
        target_unassigned,
    ) = {
        let ls = store
            .workspace_lorestore(&params.workspace)
            .expect("lorestore presence checked above");
        let target = ls
            .branch_ref(&target_ref)
            .map_err(|error| store_fault(&error))?
            .ok_or_else(|| schema_error(format!("branch `{target_ref}` has no ref")))?;
        let digest = target
            .digest()
            .map_err(|error| schema_error(error.to_string()))?
            .to_string();
        if let Some(expected) = base_ref_digest.as_deref()
            && expected != digest
        {
            return Err(ErrorBody {
                code: ErrorCode::StaleFence,
                message: format!(
                    "ref snapshot conflict on `{target_ref}`: expected {expected}, found {digest}"
                ),
                data: None,
            });
        }
        let names = ls
            .interner_at(&target.commit)
            .map_err(|error| store_fault(&error))?;
        let decisions = ls
            .decisions_at(&target.commit)
            .map_err(|error| store_fault(&error))?;
        let pack = ls
            .reconstruct_at(&target.commit)
            .map_err(|error| store_fault(&error))?;
        let world = ls
            .compile_at(&target.commit)
            .map_err(|error| store_fault(&error))?;
        let effective = ls
            .effective_valuation_at(&target.commit, &target.valuation)
            .map_err(|error| store_fault(&error))?;
        let valuation =
            dli_core::valuation::NamedValuation::from_indexed(&effective.valuation, &names)
                .map_err(|error| valuation_corrupt(&error))?;
        let unassigned =
            dli_core::valuation::NamedAtomSet::from_indexed(&effective.unassigned, &names)
                .map_err(|error| valuation_corrupt(&error))?;
        (
            target, digest, names, decisions, pack, world, valuation, unassigned,
        )
    };
    target_parent_world.killed = store
        .workspace_base(&params.workspace)
        .map(|base| base.killed.clone())
        .unwrap_or_default();
    for name in target_valuation.0.keys().chain(target_unassigned.0.iter()) {
        target_parent_world.model.world.interner.intern(name);
    }
    dli_core::valuation::apply_named_overlay(
        &target_valuation,
        &target_unassigned,
        &target_parent_world.model.world.interner,
        &mut target_parent_world.model.world.valuation,
    )
    .map_err(|error| valuation_corrupt(&error))?;
    let pack_name = target_parent_pack.name.clone();

    // Append staged Ratify declarations to the target commit's decision prefix.
    // Starting with the target list preserves the append-only chain invariant
    // even when a sibling materialized after this ordinary workspace opened.
    let staged_pack = dli_world::pack::Pack {
        name: base_name.clone(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: ident(&base_name),
                span: Span::synthetic(),
            }),
            decls: staged_decls,
        }],
    };
    for decision in dli_lorestore::lower::declared_decisions(&staged_pack) {
        if !decisions.contains(&decision) {
            decisions.push(decision);
        }
    }
    // Decision-stamped materialize (DL-0083): accept the whole committed set
    // under a named, recorded, defeasible decision — the honest replacement for
    // `fixture-authority ... true`. Record it as a known supporting decision of
    // this commit (appended to the decisions list, so `reconstruct_pack`
    // re-emits its `ratify` as a synthesized prelude beside any carried one) and
    // ensure the commit interner carries its atom, so a committed fact that
    // cites it reads certain by the ordinary algebra. The fact objects are then
    // re-stamped `Ratified` under it below. `records_decisions` is guaranteed
    // true here — a pre-1.9.0 `decision` was refused at the top.
    let mut merged_interner = dli_core::Interner::from_names(target_names.clone());
    for name in &ws_interner {
        merged_interner.intern(name);
    }
    if let Some(decision) = params.decision.as_deref() {
        if !decisions.iter().any(|known| known == decision) {
            decisions.push(decision.to_owned());
        }
        merged_interner.intern(decision);
    }
    let commit_interner = merged_interner.names().to_vec();

    // Accept the whole staged set under `d` (DL-0083): each committed **fact**
    // has its SUPPORT rested on `d`, so it reads certain-and-ratified through the
    // ordinary `ledger_valuation` algebra once `d`'s synthetic `Ratify` is
    // recorded — the honest certainty-conferral, not a cosmetic marker the
    // valuation ignores (`derive_annotation` reads certainty from `support`, not
    // from the marker, so a marker with a proposal-gated support round-trips back
    // to `Proposed` and never reads certain). Three rules:
    //   * A declaration/carriage object (functional, inertial, ratify, `__decl`)
    //     is machinery, not a claim — it does not rest on a standing decision, so
    //     it is committed byte-identical, never stamped.
    //   * A fact that already cites a decision keeps its own deliberate citation
    //     and standing — the accept never clobbers a chosen provenance.
    //   * A proposed/uncited fact becomes `cited d ratified`: its support is
    //     replaced by `d` and the promoting decision recorded. This is the
    //     batch-accept point — the honest replacement for `fixture-authority true`.
    let commit_fact_hashes = if ws_interner == commit_interner && params.decision.is_none() {
        staged_fact_hashes.clone()
    } else {
        let ls = store
            .workspace_lorestore(&params.workspace)
            .expect("checked above");
        let target_interner = dli_core::Interner::from_names(commit_interner.clone());
        let decision_support = params.decision.as_deref().map(|decision| {
            dli_core::PosBool::pos(
                target_interner
                    .lookup(decision)
                    .expect("the decision is interned in the commit interner"),
            )
        });
        // Known decisions (base-declared plus `d`): a positive support literal
        // naming one is a deliberate citation the accept must preserve.
        let known: std::collections::BTreeSet<&str> =
            decisions.iter().map(String::as_str).collect();
        let cites_a_decision = |support: &dli_core::PosBool| {
            support.monomials().any(|monomial| {
                monomial.lits().any(|lit| {
                    lit.rail == dli_core::Rail::Pos
                        && commit_interner
                            .get(lit.atom.0 as usize)
                            .is_some_and(|name| known.contains(name.as_str()))
                })
            })
        };
        let mut rewritten = Vec::with_capacity(staged_fact_hashes.len());
        for hash in &staged_fact_hashes {
            let mut object: dli_lorestore::FactObject = ls
                .read_object(hash)
                .map_err(|error| store_fault(&error))?
                .ok_or_else(|| schema_error(format!("staged object {hash} is missing")))?;
            remap_fact_object(&mut object, &ws_interner, &target_interner)?;
            // A carriage/declaration does not rest on a standing decision — commit
            // it unchanged, never stamped (its `standing_decision` stays absent).
            if object.expert_body.is_none()
                && !cites_a_decision(&object.support)
                && let (Some(decision), Some(support)) =
                    (params.decision.as_deref(), decision_support.as_ref())
            {
                // A proposed/uncited fact is accepted under `d`: rest its support
                // on `d` and record the promoting decision.
                object.support.clone_from(support);
                let provenance = object.provenance.get_or_insert_with(Default::default);
                provenance.marker = Some(ProvenanceMarker::Ratified);
                provenance.standing_decision = Some(decision.to_owned());
            }
            rewritten.push(
                ls.write_object(&object)
                    .map_err(|error| store_fault(&error))?,
            );
        }
        rewritten
    };

    // Prepare the exact post-commit world before the branch CAS. The candidate
    // fold mirrors lorestore replacement-by-content-identity, then reconstructs
    // from the very objects that will be committed (including decision-stamped
    // support). Every fallible read/reconstruction/compile therefore completes
    // while the durable ref is still unchanged.
    let prepared = {
        let ls = store
            .workspace_lorestore(&params.workspace)
            .expect("lorestore presence checked above");
        let parent = &target_snapshot.commit;
        let mut candidate_hashes = ls
            .fact_hashes_from(parent)
            .map_err(|error| store_fault(&error))?;
        let mut identities = BTreeMap::new();
        for (index, hash) in candidate_hashes.iter().enumerate() {
            let object: dli_lorestore::FactObject = ls
                .read_object(hash)
                .map_err(|error| store_fault(&error))?
                .ok_or_else(|| schema_error(format!("base object {hash} is missing")))?;
            identities.insert(
                object
                    .content_hash()
                    .map_err(|error| schema_error(error.to_string()))?,
                index,
            );
        }
        for hash in &commit_fact_hashes {
            let object: dli_lorestore::FactObject = ls
                .read_object(hash)
                .map_err(|error| store_fault(&error))?
                .ok_or_else(|| schema_error(format!("staged object {hash} is missing")))?;
            let identity = object
                .content_hash()
                .map_err(|error| schema_error(error.to_string()))?;
            if let Some(index) = identities.get(&identity).copied() {
                candidate_hashes[index] = hash.clone();
            } else {
                identities.insert(identity, candidate_hashes.len());
                candidate_hashes.push(hash.clone());
            }
        }
        let mut objects = Vec::with_capacity(candidate_hashes.len());
        for hash in candidate_hashes {
            objects.push(
                ls.read_object::<dli_lorestore::FactObject>(&hash)
                    .map_err(|error| store_fault(&error))?
                    .ok_or_else(|| schema_error(format!("candidate object {hash} is missing")))?,
            );
        }
        let known = decisions.iter().cloned().collect();
        let candidate = dli_lorestore::lower::reconstruct_pack(
            &objects,
            &dli_core::Interner::from_names(commit_interner.clone()),
            &known,
            &pack_name,
        )
        .map_err(|error| ErrorBody {
            code: ErrorCode::Internal,
            message: format!("reconstructing the materialize candidate: {error}"),
            data: None,
        })?;
        store
            .prepare_workspace_base(
                &params.workspace,
                candidate,
                target_valuation,
                target_unassigned,
            )
            .map_err(|error| ErrorBody {
                code: ErrorCode::Internal,
                message: format!("compiling the materialize candidate: {error}"),
                data: None,
            })?
    };

    // The opening report was checked against the workspace's old base. Re-run
    // the complete blocking oracle over the exact target-parent fold that was
    // just prepared; sibling state can turn two individually clean staged
    // changes into one invalid candidate.
    let blocking = dedup(
        reads::materialize_candidate_findings(
            store.get(&params.workspace).ok_or_else(gone)?,
            &target_parent_world,
            &target_parent_pack.documents,
            &prepared.world,
            false,
        )
        .into_iter()
        .filter(|finding| finding.severity == Severity::Error)
        .map(|finding| finding.code),
    );
    if !blocking.is_empty() {
        return Err(ErrorBody {
            code: ErrorCode::MaterializeBlocked,
            message: format!(
                "{} error-severity finding(s) are unresolved; resolve them or discard before materializing.",
                blocking.len()
            ),
            data: Some(json!({ "blocking": blocking })),
        });
    }

    // Prepare the JSON-first return from the object-addressed bytes before the
    // branch CAS. These objects were already written during staging (or while
    // stamping above), so reading them after the ref moved would create an
    // avoidable error window in which the durable commit succeeded but the
    // prepared in-memory base was not installed.
    let facts = if v2 {
        let ls = store
            .workspace_lorestore(&params.workspace)
            .expect("checked above");
        let mut list = Vec::with_capacity(commit_fact_hashes.len());
        for hash in &commit_fact_hashes {
            let fo: dli_lorestore::FactObject = ls
                .read_object(hash)
                .map_err(|e| ErrorBody {
                    code: ErrorCode::Internal,
                    message: format!("reading committed fact object: {e}"),
                    data: None,
                })?
                .ok_or_else(|| ErrorBody {
                    code: ErrorCode::Internal,
                    message: format!("committed fact object missing: {hash}"),
                    data: None,
                })?;
            list.push(serde_json::to_value(&fo).unwrap_or(Value::Null));
        }
        Some(list)
    } else {
        None
    };
    let materialized_keys: BTreeSet<String> = store
        .get(&params.workspace)
        .ok_or_else(gone)?
        .staged_fact_hashes
        .iter()
        .map(|(key, _)| key.clone())
        .collect();

    #[cfg(test)]
    store.run_materialize_pre_cas_hook(&params.workspace);

    let receipt = store
        .workspace_lorestore(&params.workspace)
        .expect("checked above")
        .commit_on_branch_at_ref(
            &target_ref,
            Some(&target_ref_digest),
            commit_interner.clone(),
            commit_fact_hashes.clone(),
            Vec::new(),
            Vec::new(),
            decisions,
            pack_name,
        )
        .map_err(|error| {
            // A structural refusal is a caller-facing verdict, not a server
            // defect — the same discipline `corpus_commit_fault` applies on the
            // corpus arm (DL-0056 rulings 3 and 5).
            let code = match &error {
                dli_lorestore::StoreError::InternerRegression { .. }
                | dli_lorestore::StoreError::DecisionRegression { .. }
                | dli_lorestore::StoreError::PackRename { .. } => ErrorCode::SchemaViolation,
                dli_lorestore::StoreError::RefSnapshotConflict { .. }
                | dli_lorestore::StoreError::CasConflict { .. } => ErrorCode::StaleFence,
                _ => ErrorCode::Internal,
            };
            ErrorBody {
                code,
                message: format!("committing to lorestore: {error}"),
                data: None,
            }
        })?;

    // Advance the workspace immediately after the successful CAS: everything
    // between preparation and this point is either the CAS itself or infallible
    // owned-state installation. A retry then commits nothing.
    let commit_hash = receipt.commit;
    let workspace_digest = store.install_materialized_workspace_base(
        &params.workspace,
        prepared,
        commit_interner,
        &commit_hash,
        &receipt.ref_digest,
        &materialized_keys,
    );

    Ok(value(&AuthorMutationResult {
        findings: Vec::new(),
        rendered,
        workspace_digest,
        root_hash: v2.then(|| commit_hash.to_string()),
        facts,
    }))
}

// -- the claim-bearing mutations ---------------------------------------------

/// Add an entity of a given kind (person/place/thing/era). Provenance required;
/// the display name is stored verbatim as data.
pub(super) fn add_entity(
    store: &mut Store,
    params: &AuthorAddEntityParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Entity(EntityDecl {
        kind: map_kind(params.kind),
        name: ident(&params.id),
        label: Some(params.name.clone()),
        time: None,
        slots: Vec::new(),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Add a keyed value on one entity — lowers to `key(entity, value)`.
pub(super) fn add_fact(
    store: &mut Store,
    params: &AuthorAddFactParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Attr(AttrDecl {
        subject: ident(&params.entity),
        attr: ident(&params.key),
        value: author_value(&params.value),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Add a binary relation between two entities — lowers to `relation(from, to)`.
pub(super) fn add_relation(
    store: &mut Store,
    params: &AuthorAddRelationParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: ident(&params.relation),
            args: vec![Term::Sym(ident(&params.from)), Term::Sym(ident(&params.to))],
        },
        triple: false,
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Add a chronology event (an event is not an entity; §3.4). Provenance required.
pub(super) fn add_event(
    store: &mut Store,
    params: &AuthorAddEventParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Entity(EntityDecl {
        kind: AstEntityKind::Event,
        name: ident(&params.id),
        label: params.name.clone(),
        time: Some(TimeClause::During(ident(&params.era))),
        slots: Vec::new(),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Order one event before another — declaring an order is a ruling, so
/// provenance is required. A declared order that closes a cycle is an error.
pub(super) fn add_before(
    store: &mut Store,
    params: &AuthorAddBeforeParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Before(BeforeDecl {
        earlier: ident(&params.earlier),
        later: ident(&params.later),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Assert two entity ids name the same bearer (WS-1, the identity-closure
/// kernel's wire surface). Stages a `Decl::SameAs`, which the compiled world
/// closes symmetric-transitively (`dli_eval::world::synthesize_alias_rules`)
/// and merges every other predicate's tuples across `left` and `right` — a
/// plain `author.add_relation {pred: "same-as", ...}` does not merge anything
/// (DL-0078, koan 3). The merge's standing is carried entirely by this
/// claim's own provenance: defeating the citing decision un-merges every
/// consequence algebraically, with no recompile.
pub(super) fn assert_same_as(
    store: &mut Store,
    params: &AuthorAssertSameAsParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::SameAs(SameAsDecl {
        left: ident(&params.left),
        right: ident(&params.right),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Assert two predicate names denote the same property (contract 1.12.0).
/// This constructs the dedicated structured AST declaration. It must never be
/// lowered through `Decl::SameAs`: that channel merges entity bearers.
pub(super) fn assert_predicate_same_as(
    store: &mut Store,
    params: &AuthorAssertPredicateSameAsParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::PredicateSameAs(PredicateSameAsDecl {
        left: ident(&params.left),
        right: ident(&params.right),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Declare one datalog derivation rule (WS-2): the wire surface for the
/// rule-as-claim kernel enrichment (`dli_eval::world::World::
/// define_rule_annotated`) — `fire_rules` already multiplies a rule's own
/// annotation into every derivation it fires, so this is what gives that
/// multiplication a real, cited rule to work with; defeating the rule's own
/// citing decision kills every derivation through it, exactly as defeating
/// any other claim's citation does.
///
/// **Stricter than every other `author.add_*` tool.** An error-severity
/// finding here refuses the whole add and stages nothing — never the usual
/// "stage it, block materialize" shape — because a bad rule staged even
/// once BRICKS every later read: `staged_world` recompiles the whole
/// workspace on every call, and an unstratifiable or range-unsafe rule set
/// makes that recompile fail outright, collapsing every subsequent
/// query/check/why/diff/materialize to the single opaque
/// `world.uncompilable` finding and losing whatever else that call would
/// have reported. So the check runs BEFORE `stage()` ever sees the
/// declaration (the `add_option` return-findings-without-push shape): first
/// the ordinary census (`field.control-char`, `citation.unknown-decision`)
/// plus the rule's own shape (`rule.unbound-variable`, `rule.duplicate`,
/// all in [`rule_findings`]) — any error there refuses immediately — then,
/// only once that is clean, the stratification pre-flight
/// ([`rule_stratifies`]): a tentative compile of the candidate rule
/// alongside everything already staged, refusing with `rule.unstratifiable`
/// on failure. Only a rule that survives every one of those reaches
/// `stage()`.
pub(super) fn add_rule(
    store: &mut Store,
    params: &AuthorAddRuleParams,
) -> Result<Value, ErrorBody> {
    let datalog = Datalog {
        head: params.head.clone(),
        body: params.body.clone(),
    };
    let decl = Decl::Rule(RuleDecl {
        name: ident(&params.name),
        domain: None,
        head: Vec::new(),
        body: RuleBody {
            clauses: vec![RuleClause {
                clause: Clause {
                    text: datalog.render(),
                    span: Span::synthetic(),
                },
                datalog: Some(datalog),
            }],
            inline: None,
        },
        provenance: Some(annotation(&params.provenance)),
    });

    let findings = {
        let (base, ws) = parts_mut(store, &params.workspace)?;
        decl_findings(&decl, ws, base)
    };
    if findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        let (_, ws) = parts_mut(store, &params.workspace)?;
        return Ok(mutation(findings, Vec::new(), &ws.digest));
    }

    if let Err(err) = rule_stratifies(store, &params.workspace, &decl) {
        let mut findings = findings;
        findings.push(rule_unstratifiable(&params.name, &err));
        let (_, ws) = parts_mut(store, &params.workspace)?;
        return Ok(mutation(findings, Vec::new(), &ws.digest));
    }

    stage(store, &params.workspace, decl)
}

/// Open a question — a tracked debt (§3.6). No claim is asserted by opening one.
pub(super) fn open_question(
    store: &mut Store,
    params: &AuthorOpenQuestionParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Question(QuestionDecl {
        name: ident(&params.id),
        prompt: Some(params.prompt.clone()),
        options: Vec::new(),
        provenance: None,
    });
    stage(store, &params.workspace, decl)
}

/// Add one option to a question already open in the workspace (§3.7).
pub(super) fn add_option(
    store: &mut Store,
    params: &AuthorAddOptionParams,
) -> Result<Value, ErrorBody> {
    if params
        .implies
        .as_ref()
        .is_some_and(|atom| atom.args.iter().any(|term| matches!(term, Term::Var(_))))
    {
        return Err(schema_error("an option outcome must be a ground atom"));
    }
    let existing_names: BTreeSet<String> = store
        .workspace_base_docs(&params.workspace)
        .into_iter()
        .flatten()
        .flat_map(|document| &document.decls)
        .flat_map(option_and_hypothesis_names)
        .collect();
    let (base, ws) = parts_mut(store, &params.workspace)?;
    let mut findings: Vec<Finding> = [("id", params.id.as_str()), ("label", params.label.as_str())]
        .into_iter()
        .filter_map(|(field, text)| field_control(field, text))
        .collect();
    let index = ws
        .decls
        .iter()
        .position(|decl| matches!(decl, Decl::Question(q) if q.name.text == params.question));
    let Some(i) = index else {
        findings.push(question_unknown(&params.question));
        return Ok(mutation(findings, Vec::new(), &ws.digest));
    };
    // Rebuild the question with the appended option, then swap it in — the
    // immutable borrow ends before the cache-maintaining replace.
    let (updated, old_key) = if let Some(Decl::Question(q)) = ws.decls.get(i) {
        let old_key = decl_key(&ws.decls[i]);
        if !ws.is_pending_decl(i) {
            return Err(schema_error(
                "options can be added only before the question is materialized",
            ));
        }
        if existing_names.contains(&params.id)
            || ws
                .pending_decl_refs()
                .flat_map(option_and_hypothesis_names)
                .any(|name| name == params.id)
        {
            return Err(schema_error("the option id is already used in this world"));
        }
        let mut question = q.clone();
        question.options.push(OptionClause {
            name: ident(&params.id),
            label: Some(params.label.clone()),
            implies: params.implies.clone(),
        });
        let decl = Decl::Question(question);
        findings.extend(control_char_findings(&decl));
        findings.extend(declaration_rejected_lore_findings(base, &decl));
        (decl, old_key)
    } else {
        return Err(schema_error("question disappeared while adding an option"));
    };
    // Durably stage the replacement before changing the workspace. Otherwise
    // materialize commits the earlier question without the newly added option.
    store
        .stage_lorestore(&params.workspace, &updated)
        .map_err(|error| ErrorBody {
            code: ErrorCode::Internal,
            message: format!("durable staging failed; the option was not added: {error}"),
            data: None,
        })?;
    let (_, ws) = parts_mut(store, &params.workspace)?;
    if let Some(index) = ws
        .staged_fact_hashes
        .iter()
        .position(|(key, _)| *key == old_key)
    {
        ws.staged_fact_hashes.remove(index);
    }
    ws.replace_decl(i, updated);
    let rendered = render_option(
        &params.question,
        &params.id,
        &params.label,
        params.implies.as_ref(),
    );
    Ok(mutation(findings, rendered, &ws.digest))
}

fn option_and_hypothesis_names(decl: &Decl) -> Vec<String> {
    match decl {
        Decl::Question(question) => question
            .options
            .iter()
            .map(|option| option.name.text.clone())
            .collect(),
        Decl::Fork(fork) => fork
            .options
            .iter()
            .map(|option| option.name.text.clone())
            .collect(),
        Decl::Hypothesis(hypothesis) => vec![hypothesis.name.text.clone()],
        _ => Vec::new(),
    }
}

/// Mint a hypothesis variable for fork-scoped supposition (§3.8): legal only in
/// a fork workspace and limited to one per proposal bundle.
pub(super) fn add_hypothesis(
    store: &mut Store,
    params: &AuthorAddHypothesisParams,
) -> Result<Value, ErrorBody> {
    let existing_names: BTreeSet<String> = store
        .workspace_base_docs(&params.workspace)
        .into_iter()
        .flatten()
        .flat_map(|document| &document.decls)
        .flat_map(option_and_hypothesis_names)
        .collect();
    let (base, ws) = parts_mut(store, &params.workspace)?;
    if existing_names.contains(&params.id)
        || ws
            .pending_decl_refs()
            .flat_map(option_and_hypothesis_names)
            .any(|name| name == params.id)
    {
        return Err(schema_error(
            "the hypothesis id is already used by an option or hypothesis",
        ));
    }
    let decl = Decl::Hypothesis(HypothesisDecl {
        name: ident(&params.id),
        statement: Some(params.statement.clone()),
    });
    let mut findings = control_char_findings(&decl);
    findings.extend(declaration_rejected_lore_findings(base, &decl));
    if ws.mode != Mode::Fork {
        findings.push(hypothesis_not_in_fork());
    }
    let existing = hypothesis_count(ws);
    if existing >= 1 {
        findings.push(hypothesis_bundle_limit(existing + 1));
    }
    let rendered = render_decls(std::slice::from_ref(&decl));
    ws.push_decl(decl);
    Ok(mutation(findings, rendered, &ws.digest))
}

/// Suppose one or more staged hypotheses true within the fork (§3.9): exploration
/// over an unchanged store — it asserts nothing in canon and adds no pack surface.
pub(super) fn suppose(store: &mut Store, params: &AuthorSupposeParams) -> Result<Value, ErrorBody> {
    let (_, ws) = parts_mut(store, &params.workspace)?;
    let staged: BTreeSet<String> = ws
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Hypothesis(h) => Some(h.name.text.clone()),
            _ => None,
        })
        .collect();
    let mut findings: Vec<Finding> = params
        .hypotheses
        .iter()
        .filter_map(|hyp| field_control("hypothesis", hyp))
        .collect();
    let mut supposed = Vec::new();
    for hyp in &params.hypotheses {
        if staged.contains(hyp) {
            supposed.push(hyp.clone());
        } else {
            findings.push(hypothesis_unknown(hyp));
        }
    }
    for hyp in &supposed {
        if !ws.suppositions.contains(hyp) {
            ws.suppositions.push(hyp.clone());
        }
    }
    if !supposed.is_empty() {
        findings.push(consequence_changed(&supposed));
    }
    ws.reseal();
    Ok(mutation(findings, Vec::new(), &ws.digest))
}

// -- the DM-4 expert authoring surface (contract 1.3.0) ----------------------
//
// Wave 1 is the consumer's actual corpus — the language lane: a dialect
// register, the morphemes it composes from, the names composed out of them, the
// glossary, and the ban list. Tier 2 is their episode-continuity and
// question-ledger flow. Every one of these is a typed constructor plus findings
// on the *existing* staging path: same durable lorestore write, same digest,
// same graded reply. No new machinery (DL-0058).
//
// Tier 3 (`rule`, `practice`, `proof`, `cycle`) is parked behind first consumer
// use, and tier 4 indefinitely. That is a scope ruling, not an oversight: a tool
// built before a caller exists is a guess at a shape, and the shapes here were
// read off the consumer's corpus.

/// Declare a dialect register: the phonology and name grammar a naming
/// population draws on. The typed interior clauses arrive span-free and each
/// gains the normalized clause text the formatter round-trips through.
pub(super) fn add_dialect(
    store: &mut Store,
    params: &AuthorAddDialectParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Dialect(DialectDecl {
        name: ident(&params.id),
        label: params.label.clone(),
        body: DialectBody {
            clauses: params.body.iter().map(dialect_clause).collect(),
        },
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Declare a name-part morpheme: the raw material composed names are built from.
pub(super) fn add_name_part(
    store: &mut Store,
    params: &AuthorAddNamePartParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::NamePart(NamePartDecl {
        name: ident(&params.id),
        gloss: params.gloss.clone(),
        head: params.head.clone(),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Compose a name from morphemes, optionally binding it to the entity that bears
/// it. The composition and bearer clauses are built in that order, which is the
/// order the name compiler's header-first precedence expects to read them.
pub(super) fn add_name(
    store: &mut Store,
    params: &AuthorAddNameParams,
) -> Result<Value, ErrorBody> {
    let mut clauses = Vec::new();
    if !params.composition.is_empty() {
        clauses.push(name_clause(
            composition_text(&params.composition),
            NameItem::Composition(params.composition.clone()),
        ));
    }
    if let Some(bearer) = &params.bearer {
        clauses.push(name_clause(
            format!("bearer {bearer}"),
            NameItem::Bearer {
                entity: Some(bearer.clone()),
                // The declaration-level provenance is the one provenance this
                // name carries; duplicating it onto the bearer line would give
                // the compiler's header-first precedence two candidates to
                // choose between for a single authored intent.
                provenance: None,
            },
        ));
    }
    let decl = Decl::Name(NameDecl {
        name: ident(&params.id),
        label: params.label.clone(),
        head: params.head.clone(),
        body: NameBody { clauses },
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Introduce a glossary term.
pub(super) fn add_term(
    store: &mut Store,
    params: &AuthorAddTermParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Term(TermDecl {
        name: ident(&params.id),
        label: params.label.clone(),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Ban a word from the lore surface. The reason is stored verbatim as data — it
/// is what a later refusal quotes back.
pub(super) fn add_banned(
    store: &mut Store,
    params: &AuthorAddBannedParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Banned(BannedDecl {
        word: ident(&params.word),
        reason: params.reason.clone(),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Assert a time-varying state. Unlike `add_fact`, a state holds over an
/// interval rather than forever, so it lowers to `Decl::State` and carries its
/// temporal clause rather than being folded into the timeless attribute layer.
pub(super) fn add_state(
    store: &mut Store,
    params: &AuthorAddStateParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::State(StateDecl {
        name: ident(&params.id),
        value: params.value.as_deref().map(author_value),
        time: params.time.as_ref().map(|time| {
            let point = ident(&time.point);
            match time.kind {
                StateTimeKind::After => TimeClause::After(point),
                StateTimeKind::During => TimeClause::During(point),
                StateTimeKind::Since => TimeClause::Since(point),
                StateTimeKind::Until => TimeClause::Until(point),
            }
        }),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Declare a predicate single-valued (a functional dependency). It stages a
/// carriage-only `Decl::Functional`, so the FD/contradiction check treats a slot
/// on `pred` as single-valued even for a first-seen predicate canon has no
/// precedent for — the two-distinct-certain-values case then surfaces
/// `continuity.fd-violation` (issue 49, sub-claim 4; contract 1.6.0).
pub(super) fn declare_functional(
    store: &mut Store,
    params: &AuthorDeclareFunctionalParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Functional(FunctionalDecl {
        pred: ident(&params.pred),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Declare a predicate's fluent values persistent (inertial): mirrors
/// [`declare_functional`] exactly. Stages the carriage-only declaration and
/// carries it through every lowering; the continuity check then reads it to
/// bound an inertial value's forward persistence at the next distinct certain
/// value (DL-0081).
pub(super) fn declare_inertial(
    store: &mut Store,
    params: &AuthorDeclareInertialParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Inertial(InertialDecl {
        pred: ident(&params.pred),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

/// Request an attestation for an open question — the ledger entry a later
/// attestation sweep answers.
pub(super) fn add_attest(
    store: &mut Store,
    params: &AuthorAddAttestParams,
) -> Result<Value, ErrorBody> {
    let decl = Decl::Attest(AttestDecl {
        question: ident(&params.question),
        provenance: Some(annotation(&params.provenance)),
    });
    stage(store, &params.workspace, decl)
}

// -- typed-interior lowering -------------------------------------------------
//
// The AST pairs each typed interior `item` with the normalized `Clause` text the
// parser captured, retained purely so the formatter round-trips exactly. A
// caller supplies the item; these synthesize the text. The compiler reads
// `item`, so a synthesized text that differed from what the parser would have
// captured costs rendering fidelity, never meaning.

fn dialect_clause(item: &DialectItem) -> DialectClause {
    DialectClause {
        clause: clause(dialect_item_text(item)),
        item: item.clone(),
    }
}

fn name_clause(text: String, item: NameItem) -> NameClause {
    NameClause {
        clause: clause(text),
        item,
    }
}

fn clause(text: String) -> Clause {
    Clause {
        text,
        span: Span::synthetic(),
    }
}

/// The `.dli` clause text a typed dialect item was written as.
fn dialect_item_text(item: &DialectItem) -> String {
    match item {
        DialectItem::Basis { attested, text } => {
            let attested = if *attested { "attested " } else { "" };
            let text = text
                .as_deref()
                .map_or_else(String::new, |text| format!("\"{text}\""));
            format!("basis {attested}{text}").trim_end().to_owned()
        }
        DialectItem::Vowels(words) => keyword_list("vowels", words),
        DialectItem::Endings(words) => keyword_list("endings", words),
        DialectItem::Foreign(words) => keyword_list("foreign", words),
        DialectItem::ClusterMax(n) => keyword_count("cluster-max", *n),
        DialectItem::SayabilityMax(n) => keyword_count("sayability-max", *n),
        DialectItem::Stress(keyword) => {
            format!("stress {}", keyword.as_deref().unwrap_or_default())
                .trim_end()
                .to_owned()
        }
        DialectItem::NameKind(spec) => spec
            .as_ref()
            .map_or_else(|| "name-kind".to_owned(), name_kind_text),
        DialectItem::NamePart(part) => {
            let gloss = part
                .gloss
                .as_deref()
                .map_or_else(String::new, |gloss| format!(" \"{gloss}\""));
            let head = if part.head.is_empty() {
                String::new()
            } else {
                format!(" {}", part.head.join(" "))
            };
            format!("name-part {}{head}{gloss}", part.name.text)
        }
        // An unrecognized clause has no keyword to reconstruct — by definition
        // the grammar did not classify it. It renders as nothing rather than as
        // a guess.
        DialectItem::Other => String::new(),
    }
}

fn keyword_list(keyword: &str, words: &[String]) -> String {
    if words.is_empty() {
        keyword.to_owned()
    } else {
        format!("{keyword} {}", words.join(" "))
    }
}

fn keyword_count(keyword: &str, count: Option<usize>) -> String {
    count.map_or_else(|| keyword.to_owned(), |n| format!("{keyword} {n}"))
}

fn name_kind_text(spec: &NameKindSpec) -> String {
    let patterns = spec
        .patterns
        .iter()
        .map(|roles| roles.join(" "))
        .collect::<Vec<_>>()
        .join(" | ");
    if patterns.is_empty() {
        format!("name-kind {}", spec.kind)
    } else {
        format!("name-kind {} = {patterns}", spec.kind)
    }
}

/// The `= <role> <morpheme> [+ <role> <morpheme> …]` composition line.
fn composition_text(steps: &[CompositionStep]) -> String {
    let body = steps
        .iter()
        .map(|step| format!("{} {}", step.role, step.morpheme))
        .collect::<Vec<_>>()
        .join(" + ");
    format!("= {body}")
}

// -- staging + the checked layer ---------------------------------------------

/// Stage a world declaration: check the candidate against the current staged
/// state and the base, render it, push it, and reseal the digest. The check runs
/// *before* the push, so a reference resolves against what is already present.
/// If a lorestore is attached (LS-4b), the decl is also lowered to a
/// `FactObject` and written immediately, with the hash tracked in the
/// workspace for `materialize` to commit.
fn stage(store: &mut Store, ws_id: &str, decl: Decl) -> Result<Value, ErrorBody> {
    // Findings read the workspace's derived base-plus-staged resolution index;
    // no historical declaration or base-atom census belongs on this hot path.
    let (findings, rendered) = {
        let (base, ws) = parts_mut(store, ws_id)?;
        let findings = decl_findings(&decl, ws, base);
        let rendered = render_decls(std::slice::from_ref(&decl));
        (findings, rendered)
    };

    // LS-4b: lower the decl and write fact objects to the lorestore
    // immediately (durable staging), tracking hashes for materialize. A write
    // failure refuses the add — the durable staged set and the in-memory
    // workspace must never diverge silently (the graded-findings law).
    store.stage_lorestore(ws_id, &decl).map_err(|e| ErrorBody {
        code: ErrorCode::Internal,
        message: format!("durable staging failed; the declaration was not added: {e}"),
        data: None,
    })?;

    let (_, ws) = parts_mut(store, ws_id)?;
    ws.push_decl(decl);
    Ok(mutation(findings, rendered, &ws.digest))
}

/// The findings a declaration raises against the staged state and the base.
/// Every declaration is first scanned for control characters in its free-text
/// fields (R3/C1); the world declarations then add their resolution findings.
fn decl_findings(decl: &Decl, ws: &Workspace, base: &CompiledWorld) -> Vec<Finding> {
    let mut out = control_char_findings(decl);
    out.extend(declaration_rejected_lore_findings(base, decl));
    out.extend(citation_findings(decl, ws.known_decisions()));
    let dialect = base
        .model
        .language
        .registers
        .iter()
        .map(|register| register.name.as_str())
        .chain(ws.pending_decl_refs().filter_map(|decl| match decl {
            Decl::Dialect(dialect) => Some(dialect.name.text.as_str()),
            _ => None,
        }))
        .min();
    match decl {
        Decl::Entity(e) => {
            if e.kind == AstEntityKind::Event {
                out.extend(event_findings(e, ws));
            } else if let Some(dialect) = dialect {
                out.push(name_unvetted(&e.name.text, dialect, &ws.id));
            }
        }
        Decl::Attr(a) => out.extend(attr_findings(a, ws)),
        Decl::Fact(f) => out.extend(relation_findings(f, ws)),
        Decl::Before(b) => out.extend(before_findings(b, ws, base)),
        // The DM-4 expert declarations resolve nothing beyond their own text:
        // a dialect, morpheme, term, or banned word introduces a name rather
        // than referring to one. The two that *do* refer get their reference
        // checked, on the same footing as `add_before`'s endpoints.
        Decl::State(s) => out.extend(state_findings(s, ws)),
        Decl::Attest(a) => out.extend(attest_findings(a, ws)),
        Decl::Name(n) => out.extend(name_findings(n, ws, dialect)),
        Decl::SameAs(s) => out.extend(same_as_findings(s, ws)),
        Decl::PredicateSameAs(s) => out.extend(predicate_same_as_findings(s)),
        Decl::Rule(r) => out.extend(rule_findings(r, ws, base)),
        _ => {}
    }
    out
}

fn declaration_rejected_lore_findings(base: &CompiledWorld, decl: &Decl) -> Vec<Finding> {
    // The `killed` registry predicate records a banned token as its payload; the
    // token is the fact's subject matter, not a realized usage, so the registry
    // attr is exempt from the killed-token scan (DL-0063, refined by DL-0064).
    // Its free-text `value` would otherwise trip `realizes_killed_token` and the
    // downstream `classify` `killed_hits` substring scan, blocking the registry
    // fact itself. The rejected_hits (contextual rejected-claim) path is skipped
    // wholesale here because a registry attr carries no contextual slot to reject.
    if matches!(decl, Decl::Attr(a) if a.attr.text == "killed") {
        return Vec::new();
    }
    let args = free_text_fields(decl)
        .into_iter()
        .map(|(_, text)| Term::Str(text))
        .collect::<Vec<_>>();
    if args.is_empty() {
        return Vec::new();
    }
    rejected_lore_findings(
        base,
        AtomExpr {
            pred: ident("declaration"),
            args,
        },
    )
}

fn rejected_lore_findings(base: &CompiledWorld, claim: AtomExpr) -> Vec<Finding> {
    let realizes_killed_token = claim
        .args
        .iter()
        .filter_map(|term| match term {
            Term::Sym(id) => Some(id.text.as_str()),
            Term::Str(text) => Some(text.as_str()),
            Term::Var(_) | Term::Int(_) | Term::Time(_) => None,
        })
        .chain(std::iter::once(claim.pred.text.as_str()))
        .any(|text| base.killed.hits(text).next().is_some());
    if !realizes_killed_token {
        return Vec::new();
    }
    factcheck(base, &[claim])
        .into_iter()
        .filter(|report| report.verdict == Verdict::Contradicted)
        .map(|report| Finding {
            severity: Severity::Error,
            code: "factcheck.contradicted".to_owned(),
            message: format!("{}: {}", report.claim, report.reason),
            grade: Grade::Checked,
            repair: None,
            citations: report.citations,
            data: None,
        })
        .collect()
}

fn event_findings(entity: &EntityDecl, ws: &Workspace) -> Vec<Finding> {
    let Some(TimeClause::During(era)) = &entity.time else {
        return vec![event_era_missing(&entity.name.text)];
    };
    let known = ws.known_entities();
    if !known.contains(&era.text) {
        return vec![entity_unknown(&era.text, known)];
    }
    match ws.entity_sort(&era.text) {
        Some("era") => Vec::new(),
        Some(sort) => vec![event_era_kind(&entity.name.text, &era.text, sort)],
        None => vec![event_era_kind(&entity.name.text, &era.text, "untyped")],
    }
}

/// `add_fact` findings: an unknown subject (error) and a slot that contradicts a
/// certain base value (error).
fn attr_findings(attr: &AttrDecl, ws: &Workspace) -> Vec<Finding> {
    let known = ws.known_entities();
    let mut out = Vec::new();
    if !known.contains(&attr.subject.text) {
        out.push(entity_unknown(&attr.subject.text, known));
    }
    // Compare the staged value against the base's certain value on the normalized
    // key, for any ground value — a `Term::Str` slot contradicts just as a
    // `Term::Sym` one does, and cosmetic case/hyphen drift is not a distinct value
    // (issue 49, sub-claim 4's Term::Str bypass and sub-claim 1's exact-string).
    if !matches!(attr.value, Term::Var(_)) {
        let value_text = term_text(&attr.value);
        let value_key = fd_value_key(&value_text);
        if let Some(certain) = ws.certain_base_values(&attr.attr.text, &attr.subject.text) {
            let certain_keys: BTreeSet<String> = certain.iter().map(|v| fd_value_key(v)).collect();
            if certain_keys.len() == 1
                && !certain_keys.contains(&value_key)
                && let Some(canon) = certain.iter().next()
            {
                out.push(contradiction(
                    &attr.subject.text,
                    &attr.attr.text,
                    &value_text,
                    canon,
                ));
            }
        }
    }
    out
}

/// `add_relation` findings: each argument that names no known entity is an error,
/// carrying candidate ids (§3.3).
fn relation_findings(fact: &FactDecl, ws: &Workspace) -> Vec<Finding> {
    let known = ws.known_entities();
    fact.atom
        .args
        .iter()
        .filter_map(|arg| match arg {
            Term::Sym(id) if !known.contains(&id.text) => Some(entity_unknown(&id.text, known)),
            _ => None,
        })
        .collect()
}

/// `add_before` findings: an unknown endpoint (error) and an order that closes a
/// cycle against existing orderings (error; §3.5).
fn before_findings(before: &BeforeDecl, ws: &Workspace, base: &CompiledWorld) -> Vec<Finding> {
    let known = ws.known_entities();
    let mut out = Vec::new();
    if !known.contains(&before.earlier.text) {
        out.push(entity_unknown(&before.earlier.text, known));
    }
    if !known.contains(&before.later.text) {
        out.push(entity_unknown(&before.later.text, known));
    }
    if closes_cycle(&before.earlier.text, &before.later.text, ws, base) {
        out.push(order_cycle(&before.earlier.text, &before.later.text));
    }
    out
}

/// `assert_same_as` findings (WS-1): an unknown endpoint (error, both sides
/// checked against known entities, same as `add_before`'s); a self-merge
/// (warn — `left == right` names one bearer twice, staged anyway as a
/// harmless no-op, never an error); an endpoint naming a declared era or
/// event (error — the stage-time counterpart to the kernel's world-check
/// `continuity.identity-temporal-unsupported`, DL-0078's v1 boundary); and a
/// sort mismatch between the two declared entity kinds (warn — an honest
/// signal an author may still mean, never a block).
fn same_as_findings(same_as: &SameAsDecl, ws: &Workspace) -> Vec<Finding> {
    let known = ws.known_entities();
    let mut out = Vec::new();
    if !known.contains(&same_as.left.text) {
        out.push(entity_unknown(&same_as.left.text, known));
    }
    if !known.contains(&same_as.right.text) {
        out.push(entity_unknown(&same_as.right.text, known));
    }
    if same_as.left.text == same_as.right.text {
        out.push(identity_self_merge(&same_as.left.text));
    }
    // One finding even when both endpoints are temporal — mirrors the
    // kernel's own `identity_temporal_unsupported`, which also stops at the
    // first hit.
    for endpoint in [same_as.left.text.as_str(), same_as.right.text.as_str()] {
        if let Some(sort @ ("era" | "event")) = ws.entity_sort(endpoint) {
            out.push(identity_temporal_unsupported(
                &same_as.left.text,
                &same_as.right.text,
                endpoint,
                sort,
            ));
            break;
        }
    }
    if let (Some(left_sort), Some(right_sort)) = (
        ws.entity_sort(&same_as.left.text),
        ws.entity_sort(&same_as.right.text),
    ) && left_sort != right_sort
    {
        out.push(identity_sort_mismatch(
            &same_as.left.text,
            left_sort,
            &same_as.right.text,
            right_sort,
        ));
    }
    out
}

/// Predicate identity has no entity endpoint lookup: predicate names are open
/// vocabulary and their declaration is the sole source of merge authority.
/// A self-merge is retained for provenance but is a harmless no-op, matching
/// the entity identity contract while keeping the finding data structured.
fn predicate_same_as_findings(identity: &PredicateSameAsDecl) -> Vec<Finding> {
    (identity.left.text == identity.right.text)
        .then(|| predicate_identity_self_merge(&identity.left.text))
        .into_iter()
        .collect()
}

/// `add_state` findings: the subject the dotted state name is about must be a
/// known entity, and a temporal anchor must name one too. A state is a claim
/// *about* something, so an unresolvable subject is the same error class as
/// `add_fact`'s. A KNOWN anchor of the wrong sort — `since`/`until` naming an
/// era, `after`/`during` naming an event — is `state.anchor-kind` (WS-4): the
/// consumer's pinned xfail (an LLM anchoring `since`/`until` at an era) turns
/// from a check-time dead end into an add-time repairable finding, ahead of
/// the load-side `load.unknown-temporal-reference` backstop (unchanged) that
/// still catches an anchor this workspace never even declares.
fn state_findings(state: &StateDecl, ws: &Workspace) -> Vec<Finding> {
    let known = ws.known_entities();
    let mut out = Vec::new();
    // `joren.water-status` is a state on `joren`; an undotted name names no
    // subject and so has nothing to resolve.
    if let Some((subject, _)) = state.name.text.split_once('.')
        && !known.contains(subject)
    {
        out.push(entity_unknown(subject, known));
    }
    if let Some(time) = &state.time {
        let (point, expected_kind) = match time {
            TimeClause::Since(p) | TimeClause::Until(p) => (p, "event"),
            TimeClause::After(p) | TimeClause::During(p) => (p, "era"),
            // `order` is not reachable from `author.add_state` (the wire form
            // carries a single point), but the AST allows it, so it resolves
            // every point rather than silently checking none. It carries no
            // kind expectation of its own (chronology order, not an anchor).
            TimeClause::Order(points) => {
                for point in points {
                    if !known.contains(&point.text) {
                        out.push(entity_unknown(&point.text, known));
                    }
                }
                return out;
            }
        };
        if !known.contains(&point.text) {
            out.push(entity_unknown(&point.text, known));
        } else if let Some(actual_kind) = ws.entity_sort(&point.text)
            && actual_kind != expected_kind
        {
            out.push(state_anchor_kind(
                &point.text,
                expected_kind,
                actual_kind,
                &ws.entities_of_kind(expected_kind),
            ));
        }
    }
    out
}

/// The known entities of exactly one sort (`era`, `event`, …), for a
/// `state.anchor-kind` repair's `alternatives` — the valid anchors an author
/// could have meant instead.
/// `add_attest` findings: the attested question must be open. An attestation
/// request against a question nobody asked is tracked debt pointing at nothing.
fn attest_findings(attest: &AttestDecl, ws: &Workspace) -> Vec<Finding> {
    let staged = ws
        .decls
        .iter()
        .any(|decl| matches!(decl, Decl::Question(q) if q.name.text == attest.question.text));
    if staged {
        Vec::new()
    } else {
        vec![question_unknown(&attest.question.text)]
    }
}

/// `add_name` findings: a bound bearer must be a known entity, and the name
/// itself has not passed the naming oracle — the same unvetted warning
/// `add_entity` raises, for the same reason.
fn name_findings(name: &NameDecl, ws: &Workspace, dialect: Option<&str>) -> Vec<Finding> {
    let mut out = dialect
        .map(|dialect| vec![name_unvetted(&name.name.text, dialect, &ws.id)])
        .unwrap_or_default();
    let known = ws.known_entities();
    for clause in &name.body.clauses {
        if let NameItem::Bearer {
            entity: Some(entity),
            ..
        } = &clause.item
            && !known.contains(entity)
        {
            out.push(entity_unknown(entity, known));
        }
    }
    out
}

/// Every finding open over the pending workspace delta — the materialize gate
/// reads its error-severity subset. Each pending declaration is re-checked
/// against the full base-plus-workspace resolution state, so a later-added
/// target clears an earlier reference's block while accepted history is not
/// revalidated as if it were newly staged.
///
/// The read tools (`author.findings`, `author.check`) reuse this so the standing
/// aggregate is exactly what materialize refuses on.
pub(super) fn workspace_findings(ws: &Workspace, base: &CompiledWorld) -> Vec<Finding> {
    let mut out = Vec::new();
    for decl in ws.pending_decl_refs() {
        out.extend(decl_findings(decl, ws, base));
    }
    let hyps = hypothesis_count(ws);
    if hyps > 0 && ws.mode != Mode::Fork {
        out.push(hypothesis_not_in_fork());
    }
    if hyps > 1 {
        out.push(hypothesis_bundle_limit(hyps));
    }
    out
}

// -- entity/order resolution -------------------------------------------------

/// Whether adding `earlier -> later` closes a cycle against the base's and the
/// workspace's existing orderings.
fn closes_cycle(earlier: &str, later: &str, ws: &Workspace, base: &CompiledWorld) -> bool {
    if earlier == later {
        return true;
    }
    let edges = before_edges(ws, base);
    reachable(later, earlier, &edges)
}

/// Every `before` edge the base and the workspace already hold.
fn before_edges(ws: &Workspace, base: &CompiledWorld) -> Vec<(String, String)> {
    let mut edges = Vec::new();
    for atom in base.model.atoms() {
        if atom.atom.pred.text == "before"
            && let [Term::Sym(a), Term::Sym(b)] = atom.atom.args.as_slice()
        {
            edges.push((a.text.clone(), b.text.clone()));
        }
    }
    for decl in &ws.decls {
        if let Decl::Before(b) = decl {
            edges.push((b.earlier.text.clone(), b.later.text.clone()));
        }
    }
    edges
}

/// Whether `to` is reachable from `from` following the directed edges.
fn reachable(from: &str, to: &str, edges: &[(String, String)]) -> bool {
    let mut frontier = vec![from.to_owned()];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    while let Some(node) = frontier.pop() {
        if node == to {
            return true;
        }
        if !seen.insert(node.clone()) {
            continue;
        }
        for (a, b) in edges {
            if a == &node {
                frontier.push(b.clone());
            }
        }
    }
    false
}

/// Candidate ids an unknown reference most plausibly meant: known entities within
/// a small edit distance, nearest first.
fn candidates(target: &str, known: &BTreeSet<String>) -> Vec<String> {
    // The five best results contain every name within distance 3 that the old
    // full sort could have returned: all closer names precede every farther
    // name, and ties are lexical. Filtering after the bounded ranking is
    // therefore byte-identical to the old `filter → sort → take` path.
    nearest_by_edit_distance(target, known.iter().cloned(), 5)
        .into_iter()
        .filter(|(distance, _)| *distance <= 3)
        .map(|(_, name)| name)
        .collect()
}

/// Known `names` ranked by edit distance to `surface`, nearest first and lexical
/// on ties, each paired with its distance, keeping at most `limit`. The shared
/// core of the unknown-reference suggester ([`candidates`]) and the
/// `author.entity_candidates` read (Read B); `limit` bounds the result so
/// neither can turn into a full dump.
pub(super) fn nearest_by_edit_distance(
    surface: &str,
    names: impl IntoIterator<Item = String>,
    limit: usize,
) -> Vec<(usize, String)> {
    if limit == 0 {
        return Vec::new();
    }

    let surface: Vec<char> = surface.chars().collect();
    // `BinaryHeap` keeps the largest tuple at the top. Since the final order
    // is ascending by `(distance, name)`, the top entry is exactly the worst
    // retained candidate and can be evicted when a better one arrives.
    let mut best: BinaryHeap<(usize, String)> = BinaryHeap::with_capacity(limit);
    for name in names {
        let cutoff = (best.len() == limit).then(|| {
            best.peek()
                .expect("a full candidate heap has a worst entry")
                .0
        });
        let Some(distance) = levenshtein_bounded(&surface, &name, cutoff) else {
            continue;
        };
        let candidate = (distance, name);
        if best.len() < limit {
            best.push(candidate);
        } else if candidate
            < *best
                .peek()
                .expect("a full candidate heap has a worst entry")
        {
            best.pop();
            best.push(candidate);
        }
    }

    let mut scored = best.into_vec();
    scored.sort_unstable();
    scored
}

/// The exact Levenshtein distance, optionally stopping once a candidate is
/// provably worse than the current heap cutoff.
///
/// The minimum value in each dynamic-programming row is non-decreasing. Once
/// every cell exceeds `cutoff`, no later row can recover a distance at or below
/// it, so returning `None` cannot discard a candidate that belongs in the
/// deterministic top-k result. Equality is retained so lexical tie-breaking
/// remains exact.
fn levenshtein_bounded(a: &[char], b: &str, cutoff: Option<usize>) -> Option<usize> {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        if cutoff.is_some_and(|max| cur.iter().all(|distance| *distance > max)) {
            return None;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    let distance = prev[b.len()];
    cutoff.is_none_or(|max| distance <= max).then_some(distance)
}

// -- control-character defense (R3/C1) ---------------------------------------

/// The `field.control-char` findings a declaration's free-text fields raise: one
/// per field carrying a control character. A newline or tab smuggled into an
/// origin, value, name, prompt, or option would forge a second declaration line
/// in the review diff, so it enters as an error-severity finding that bars
/// materialization (the display is separately sanitized in `render`).
fn control_char_findings(decl: &Decl) -> Vec<Finding> {
    free_text_fields(decl)
        .iter()
        .filter_map(|(field, text)| field_control(field, text))
        .collect()
}

/// A `field.control-char` finding for one field, if its text carries a control
/// character.
fn field_control(field: &'static str, text: &str) -> Option<Finding> {
    text.chars()
        .any(super::render::is_forbidden_char)
        .then(|| control_char(field))
}

/// The named free-text fields a declaration renders — everything a control
/// character could hide inside.
// One arm per declaration kind, each a flat enumeration of that kind's fields.
// It runs long by construction and splitting it would only scatter the census
// this function exists to be complete about.
#[allow(clippy::too_many_lines)]
fn free_text_fields(decl: &Decl) -> Vec<(&'static str, String)> {
    let mut fields = Vec::new();
    match decl {
        Decl::Entity(e) => {
            fields.push(("id", e.name.text.clone()));
            if let Some(label) = &e.label {
                fields.push(("name", label.clone()));
            }
            if let Some(time) = &e.time {
                match time {
                    TimeClause::After(point)
                    | TimeClause::During(point)
                    | TimeClause::Since(point)
                    | TimeClause::Until(point) => {
                        fields.push(("time", point.text.clone()));
                    }
                    TimeClause::Order(points) => {
                        fields.extend(points.iter().map(|point| ("time", point.text.clone())));
                    }
                }
            }
            annotation_fields(&mut fields, e.provenance.as_ref());
        }
        Decl::Attr(a) => {
            fields.push(("entity", a.subject.text.clone()));
            fields.push(("key", a.attr.text.clone()));
            fields.push(("value", term_text(&a.value)));
            annotation_fields(&mut fields, a.provenance.as_ref());
        }
        Decl::Fact(f) => {
            fields.push(("relation", f.atom.pred.text.clone()));
            for arg in &f.atom.args {
                fields.push(("argument", term_text(arg)));
            }
            annotation_fields(&mut fields, f.provenance.as_ref());
        }
        Decl::Before(b) => {
            fields.push(("earlier", b.earlier.text.clone()));
            fields.push(("later", b.later.text.clone()));
            annotation_fields(&mut fields, b.provenance.as_ref());
        }
        Decl::SameAs(s) => {
            fields.push(("left", s.left.text.clone()));
            fields.push(("right", s.right.text.clone()));
            annotation_fields(&mut fields, s.provenance.as_ref());
        }
        Decl::PredicateSameAs(s) => {
            fields.push(("left", s.left.text.clone()));
            fields.push(("right", s.right.text.clone()));
            annotation_fields(&mut fields, s.provenance.as_ref());
        }
        Decl::Question(q) => {
            fields.push(("id", q.name.text.clone()));
            if let Some(prompt) = &q.prompt {
                fields.push(("prompt", prompt.clone()));
            }
            for option in &q.options {
                fields.push(("option", option.name.text.clone()));
                if let Some(label) = &option.label {
                    fields.push(("option-label", label.clone()));
                }
                if let Some(atom) = &option.implies {
                    fields.push(("option-predicate", atom.pred.text.clone()));
                    for arg in &atom.args {
                        fields.push(("option-argument", term_text(arg)));
                    }
                }
            }
            annotation_fields(&mut fields, q.provenance.as_ref());
        }
        Decl::Hypothesis(h) => {
            fields.push(("id", h.name.text.clone()));
            if let Some(statement) = &h.statement {
                fields.push(("statement", statement.clone()));
            }
        }
        // The DM-4 expert declarations (contract 1.3.0). Every free-text field
        // each one renders is enumerated here. This list is not optional
        // bookkeeping: a field missing from it is a field a control character
        // can hide in, and the `field.control-char` error is what stops a
        // newline in a gloss or a ban reason from forging a second declaration
        // line in the review diff (R3/C1).
        Decl::Dialect(d) => {
            fields.push(("id", d.name.text.clone()));
            if let Some(label) = &d.label {
                fields.push(("label", label.clone()));
            }
            for clause in &d.body.clauses {
                fields.push(("clause", clause.clause.text.clone()));
                dialect_item_fields(&mut fields, &clause.item);
            }
            annotation_fields(&mut fields, d.provenance.as_ref());
        }
        Decl::NamePart(n) => {
            name_part_fields(&mut fields, n);
        }
        Decl::Name(n) => {
            fields.push(("id", n.name.text.clone()));
            if let Some(label) = &n.label {
                fields.push(("label", label.clone()));
            }
            fields.extend(n.head.iter().map(|token| ("head", token.clone())));
            for clause in &n.body.clauses {
                fields.push(("clause", clause.clause.text.clone()));
                match &clause.item {
                    NameItem::Composition(steps) => {
                        for step in steps {
                            fields.push(("role", step.role.clone()));
                            fields.push(("morpheme", step.morpheme.clone()));
                        }
                    }
                    NameItem::Bearer {
                        entity,
                        provenance: bearer_provenance,
                    } => {
                        if let Some(entity) = entity {
                            fields.push(("bearer", entity.clone()));
                        }
                        annotation_fields(&mut fields, bearer_provenance.as_ref());
                    }
                    NameItem::Provenance(annot) => annotation_fields(&mut fields, Some(annot)),
                    NameItem::Other => {}
                }
            }
            annotation_fields(&mut fields, n.provenance.as_ref());
        }
        Decl::Term(t) => {
            fields.push(("id", t.name.text.clone()));
            if let Some(label) = &t.label {
                fields.push(("label", label.clone()));
            }
            annotation_fields(&mut fields, t.provenance.as_ref());
        }
        Decl::Banned(b) => {
            fields.push(("word", b.word.text.clone()));
            if let Some(reason) = &b.reason {
                fields.push(("reason", reason.clone()));
            }
            annotation_fields(&mut fields, b.provenance.as_ref());
        }
        Decl::State(s) => {
            fields.push(("id", s.name.text.clone()));
            if let Some(value) = &s.value {
                fields.push(("value", term_text(value)));
            }
            if let Some(time) = &s.time {
                match time {
                    TimeClause::After(point)
                    | TimeClause::During(point)
                    | TimeClause::Since(point)
                    | TimeClause::Until(point) => fields.push(("time", point.text.clone())),
                    TimeClause::Order(points) => {
                        fields.extend(points.iter().map(|point| ("time", point.text.clone())));
                    }
                }
            }
            annotation_fields(&mut fields, s.provenance.as_ref());
        }
        Decl::Functional(f) => {
            fields.push(("pred", f.pred.text.clone()));
            annotation_fields(&mut fields, f.provenance.as_ref());
        }
        Decl::Inertial(f) => {
            fields.push(("pred", f.pred.text.clone()));
            annotation_fields(&mut fields, f.provenance.as_ref());
        }
        Decl::Attest(a) => {
            fields.push(("question", a.question.text.clone()));
            annotation_fields(&mut fields, a.provenance.as_ref());
        }
        Decl::Rule(r) => {
            fields.push(("id", r.name.text.clone()));
            fields.extend(r.head.iter().map(|token| ("head", token.clone())));
            for rc in &r.body.clauses {
                fields.push(("clause", rc.clause.text.clone()));
            }
            if let Some(rc) = &r.body.inline {
                fields.push(("clause", rc.clause.text.clone()));
            }
            annotation_fields(&mut fields, r.provenance.as_ref());
        }
        // `author.record_decision` is the first op to stage a `ratify` from
        // untrusted wire input (DL-0083): its caller-named decision id and any
        // provenance citations are free text, so a control character smuggled
        // into either would forge a second line in the review diff unless it is
        // scanned here (guard census — a control char in the id refuses the
        // record). `RatifyBody`'s citations live in `body.citations`, its own
        // typed interior (LS-9), so both are enumerated.
        Decl::Ratify(r) => {
            fields.push(("decision", r.decision.text.clone()));
            fields.extend(r.body.citations.iter().map(|id| ("cited", id.clone())));
        }
        _ => {}
    }
    fields
}

/// The free-text a typed dialect clause carries beyond its normalized text —
/// every string the item payload holds, since the compiler reads the payload
/// and the formatter may render either.
fn dialect_item_fields(fields: &mut Vec<(&'static str, String)>, item: &DialectItem) {
    match item {
        DialectItem::Basis { text, .. } => {
            if let Some(text) = text {
                fields.push(("basis", text.clone()));
            }
        }
        DialectItem::Vowels(words) | DialectItem::Endings(words) | DialectItem::Foreign(words) => {
            fields.extend(words.iter().map(|word| ("token", word.clone())));
        }
        DialectItem::Stress(Some(keyword)) => fields.push(("stress", keyword.clone())),
        DialectItem::NameKind(Some(spec)) => {
            fields.push(("name-kind", spec.kind.clone()));
            for pattern in &spec.patterns {
                fields.extend(pattern.iter().map(|role| ("role", role.clone())));
            }
        }
        DialectItem::NamePart(part) => name_part_fields(fields, part),
        DialectItem::Stress(None)
        | DialectItem::NameKind(None)
        | DialectItem::ClusterMax(_)
        | DialectItem::SayabilityMax(_)
        | DialectItem::Other => {}
    }
}

/// A name-part's free-text fields. Shared because a `name-part` is both a
/// top-level declaration and a legal dialect interior clause, and both spellings
/// must be scanned identically.
fn name_part_fields(fields: &mut Vec<(&'static str, String)>, part: &NamePartDecl) {
    fields.push(("id", part.name.text.clone()));
    if let Some(gloss) = &part.gloss {
        fields.push(("gloss", gloss.clone()));
    }
    fields.extend(part.head.iter().map(|token| ("head", token.clone())));
    annotation_fields(fields, part.provenance.as_ref());
}

/// The free-text fields a provenance annotation carries (origin tag, cited
/// decision ids, hypothesis condition).
fn annotation_fields(fields: &mut Vec<(&'static str, String)>, annot: Option<&Annotation>) {
    let Some(annot) = annot else {
        return;
    };
    for tag in &annot.tags {
        fields.push(("origin", tag.clone()));
    }
    for citation in &annot.citations {
        fields.push(("decision", citation.id.text.clone()));
    }
    if let Some(CondExpr::Hyp(hyp)) = &annot.condition {
        fields.push(("hypothesis", hyp.text.clone()));
    }
}

/// The **structural key** naming one staged declaration: the JCS digest of the
/// declaration itself, `jcs:sha256:<hex>`.
///
/// This is the DM-3 deferral closed (DL-0058). The key used to be the rendered
/// `.dli` line, with the declaration's raw `serde_json::to_string` as the
/// fallback on a build that links no formatter — and that fallback could not be
/// promoted to a wire identifier, which is why per-declaration `author.discard`
/// refused outright there. Raw serde output is not a safe identifier: its bytes
/// follow the order the struct happens to declare its fields, so reordering two
/// fields would have re-keyed every staged declaration of that kind without
/// changing a single one of them. JCS fixes the encoding — keys sort, the
/// number and string forms are specified — so the key is a function of the
/// declaration's *serialized content* under a spelling nobody has to guess at.
///
/// Being build-independent is the point: the same declaration has the same key
/// on a default build and a `syntax` build, so the durable
/// `staged_fact_hashes` bookkeeping no longer means two different things
/// depending on which binary is running.
///
/// **The limit, stated because the wire schema states it too.** Canonical
/// encoding buys reorder-stability, not immunity to schema growth. A field that
/// serializes is content and moves the key — including one serializing as
/// `null`, which is what an `Option` without `skip_serializing_if` does when
/// absent, and that is every optional field on the declaration types today.
/// Only a field elided from the bytes entirely leaves the key alone. The key is
/// therefore valid within a contract version, which is what
/// [`crate::contract::StagedDecl::key`] tells the caller.
pub(super) fn decl_key(decl: &Decl) -> String {
    super::digest::digest_of(decl)
}

/// The canonical rendered line for a staged declaration, or `None` on a build
/// that links no formatter (LS-7) — the pre-1.3.0 `author.discard` key, kept
/// because that is what every wire below 1.3.0 accepts.
fn rendered_key(decl: &Decl) -> Option<String> {
    super::render::renders_dli().then(|| {
        render_decls(std::slice::from_ref(decl))
            .into_iter()
            .next()
            .unwrap_or_default()
    })
}

// -- finding constructors ----------------------------------------------------

fn name_unvetted(id: &str, dialect: &str, workspace: &str) -> Finding {
    Finding {
        severity: Severity::Warn,
        code: "name.unvetted".to_owned(),
        message: format!(
            "The name \"{id}\" has not passed the naming oracle; it enters proposed and unvetted."
        ),
        grade: Grade::Asserted,
        repair: Some(json!({
            "kind": "run-check",
            "op": "dli.name.check",
            "params": { "name": id, "dialect": dialect, "workspace": workspace }
        })),
        citations: Vec::new(),
        data: None,
    }
}

fn entity_unknown(id: &str, known: &BTreeSet<String>) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "entity.unknown".to_owned(),
        message: format!(
            "No entity \"{id}\" is in scope. The claim is staged but cannot materialize until it exists."
        ),
        grade: Grade::Checked,
        repair: Some(json!({ "kind": "resolve-entity", "candidates": candidates(id, known) })),
        citations: Vec::new(),
        data: None,
    }
}

fn contradiction(subject: &str, key: &str, value: &str, canon: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "fact.contradiction".to_owned(),
        message: format!(
            "claims {subject}.{key} = {value}, but the base asserts {canon} as certain."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn order_cycle(earlier: &str, later: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "order.cycle".to_owned(),
        message: format!(
            "before({earlier}, {later}) closes a cycle against the existing orderings; the timeline would have no consistent order."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn consequence_changed(supposed: &[String]) -> Finding {
    Finding {
        severity: Severity::Warn,
        code: "consequence.changed".to_owned(),
        message: format!(
            "Under {}, the derived-consequence set may move. See author.diff for the consequences.",
            supposed.join(", ")
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn hypothesis_not_in_fork() -> Finding {
    Finding {
        severity: Severity::Error,
        code: "hypothesis.not-in-fork".to_owned(),
        message: "A hypothesis is legal only in a fork workspace; open with {\"fork\": …}."
            .to_owned(),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn hypothesis_bundle_limit(count: usize) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "hypothesis.bundle-limit".to_owned(),
        message: format!(
            "A proposal bundle carries at most one hypothesis; this workspace now stages {count}. A second supposition is a second fork."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn hypothesis_unknown(id: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "hypothesis.unknown".to_owned(),
        message: format!("No hypothesis \"{id}\" is staged in this workspace to suppose."),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn question_unknown(id: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "question.unknown".to_owned(),
        message: format!(
            "No question \"{id}\" is open in this workspace; open it before adding options."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn event_era_missing(event: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "event.era-missing".to_owned(),
        message: format!(
            "Event \"{event}\" has no era anchor; every event must occur during an era."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn event_era_kind(event: &str, era: &str, actual: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "event.era-kind".to_owned(),
        message: format!(
            "Event \"{event}\" names \"{era}\" as its era, but that entity is {actual}; choose an era entity."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

/// `state.anchor-kind` (WS-4): a state's temporal anchor names a KNOWN entity
/// of the wrong sort — `since`/`until` need an event, `after`/`during` need
/// an era. Typed repair names both kinds, the offending point, and up to five
/// known entities of the expected kind the author could anchor to instead.
fn state_anchor_kind(
    point: &str,
    expected_kind: &str,
    actual_kind: &str,
    alternatives: &[String],
) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "state.anchor-kind".to_owned(),
        message: format!(
            "the state anchors at \"{point}\", which is {actual_kind}, but this time clause needs {expected_kind}; choose an {expected_kind} entity."
        ),
        grade: Grade::Checked,
        repair: Some(json!({
            "kind": "resolve-anchor",
            "expected_kind": expected_kind,
            "actual_kind": actual_kind,
            "point": point,
            "alternatives": alternatives.iter().take(5).collect::<Vec<_>>(),
        })),
        citations: Vec::new(),
        data: None,
    }
}

fn identity_self_merge(id: &str) -> Finding {
    Finding {
        severity: Severity::Warn,
        code: "identity.self-merge".to_owned(),
        message: format!(
            "same-as {id} = {id} names one bearer twice; staged anyway as a harmless no-op."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn predicate_identity_self_merge(id: &str) -> Finding {
    Finding {
        severity: Severity::Warn,
        code: "predicate-identity.self-merge".to_owned(),
        message: format!(
            "predicate identity {id} = {id} names one property twice; staged anyway as a harmless no-op."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: Some(json!({"left": id, "right": id, "effect": "no-op"})),
    }
}

fn identity_temporal_unsupported(left: &str, right: &str, endpoint: &str, sort: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "identity.temporal-unsupported".to_owned(),
        message: format!(
            "same-as {left} = {right} names \"{endpoint}\", a {sort}; identity over eras/events is not supported (v1 boundary) — point unification would half-merge the chronology."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn identity_sort_mismatch(left: &str, left_sort: &str, right: &str, right_sort: &str) -> Finding {
    Finding {
        severity: Severity::Warn,
        code: "identity.sort-mismatch".to_owned(),
        message: format!(
            "same-as {left} = {right} merges a {left_sort} with a {right_sort} — an honest signal, not a block."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

// -- author.add_rule (WS-2) ---------------------------------------------------

/// `add_rule`'s findings, computed WITHOUT staging (WS-2's stricter gate):
/// unlike every other declaration, an error here refuses the whole add rather
/// than staging alongside a blocking finding. A bad rule must never reach
/// `ws.decls` — staging one BRICKS every subsequent read, because
/// `staged_world` recompiles on every call and an unstratifiable or
/// range-unsafe rule set makes that recompile fail outright, collapsing every
/// read to the single opaque `world.uncompilable` finding and losing every
/// other finding the same call would otherwise report. So this function
/// covers a rule's *shape* — range restriction and a duplicate name — and the
/// caller runs it, and the stratification pre-flight compile, BEFORE staging;
/// the generic census (`field.control-char`, `citation.unknown-decision`) that
/// `decl_findings` already runs for every declaration kind is error-severity
/// too and gates the same way, once `free_text_fields`/`decl_annotation`
/// carry a `Decl::Rule` arm (done above).
fn rule_findings(rule: &RuleDecl, ws: &Workspace, base: &CompiledWorld) -> Vec<Finding> {
    let mut out = Vec::new();
    if let Some(datalog) = rule_datalog(rule) {
        if datalog.body.is_empty() {
            // rt-features SEV-C: an empty body would otherwise stage clean and
            // derive the head unconditionally (fact-as-rule), but its render
            // (`head :- `) reparses via the `.dli` text path to `datalog: None`
            // — `parse_rule_clause` requires at least one body atom after the
            // separator — which the loader then refuses as `MalformedWorldRule`
            // (koan 4's own body-less handling, `dli-world/src/loader.rs`). A
            // wire-staged rule that the reimport path would refuse is a
            // migration-parity break, so refuse it here too, before it ever
            // stages.
            out.push(rule_empty_body(&rule.name.text));
        } else if let Some(variable) =
            dli_eval::world::unbound_variable(&datalog.head, &datalog.body)
        {
            out.push(rule_unbound_variable(&rule.name.text, &variable));
        }
    }
    if rule_name_taken(rule, ws, base) {
        out.push(rule_duplicate(&rule.name.text));
    }
    out
}

/// The typed datalog a `RuleDecl` carries, from whichever of the two shapes
/// `structure_rule_body` can produce holds it — a single block clause (the
/// shape every wire-authored rule uses) or the inline `name : head :- body`
/// header form a text-authored corpus can use instead.
fn rule_datalog(rule: &RuleDecl) -> Option<&Datalog> {
    rule.body
        .clauses
        .first()
        .and_then(|rc| rc.datalog.as_ref())
        .or_else(|| rule.body.inline.as_ref().and_then(|rc| rc.datalog.as_ref()))
}

/// Whether `current`'s name already names a DIFFERENT rule — staged in this
/// workspace, or already in the compiled base (a corpus-imported or
/// previously materialized rule).
///
/// This runs at two different times against the same workspace: at add time,
/// on a freshly built candidate not yet in `ws.decls`, where any staged rule
/// sharing the name is a genuine duplicate; and again at every later
/// `author.check`, over each pending declaration in turn
/// ([`workspace_findings`]'s pending-ref re-validation), where `current` IS one
/// of the elements being scanned. Comparing by pointer
/// excludes exactly that self-match — the one case a plain name scan cannot
/// tell apart from a real duplicate — so a rule never reports itself as
/// duplicating itself on every subsequent check.
fn rule_name_taken(current: &RuleDecl, ws: &Workspace, base: &CompiledWorld) -> bool {
    let staged = ws.decls.iter().any(|decl| match decl {
        Decl::Rule(other) => !std::ptr::eq(other, current) && other.name.text == current.name.text,
        _ => false,
    });
    let based = base
        .model
        .world
        .rules
        .iter()
        .any(|r| r.name == current.name.text);
    staged || based
}

/// Tentatively compile the workspace's staged declarations plus one candidate
/// rule — the stratification pre-flight WS-2 requires. Mirrors
/// [`reads::staged_world`]'s own compile shape (base docs + staged decls
/// under the workspace's scope) but does not touch `ws.decls`: a caller uses
/// the `Err` to refuse the add without ever staging the rule that caused it,
/// which is the whole point — a rule that makes this fail must never reach
/// the workspace it would brick.
fn rule_stratifies(store: &Store, ws_id: &str, candidate: &Decl) -> Result<(), EvalError> {
    let Some(ws) = store.get(ws_id) else {
        // Unreachable in practice — `precheck` confirms the workspace exists
        // before any tool runs — but a vanished workspace is not this
        // function's stratification question to answer either way.
        return Ok(());
    };
    let mut documents = store
        .workspace_base_docs(ws_id)
        .unwrap_or_default()
        .to_vec();
    let mut decls = ws.pending_decls();
    decls.push(candidate.clone());
    documents.push(Document {
        scope: Some(ScopeHeader {
            name: Ident::synthetic(&ws.base_name),
            span: Span::synthetic(),
        }),
        decls,
    });
    compile(&Pack {
        name: ws.base_name.clone(),
        documents,
    })
    .map(|_| ())
}

fn rule_unbound_variable(name: &str, variable: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "rule.unbound-variable".to_owned(),
        message: format!(
            "rule '{name}' is not range-restricted: variable '{variable}' is not bound by any positive body atom"
        ),
        grade: Grade::Checked,
        repair: Some(json!({ "kind": "bind-variable", "variable": variable })),
        citations: Vec::new(),
        data: None,
    }
}

fn rule_empty_body(name: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "rule.empty-body".to_owned(),
        message: format!(
            "rule '{name}' has an empty body; a body-less rule derives its head unconditionally on the wire but the reimport path refuses it as malformed — add at least one body literal."
        ),
        grade: Grade::Checked,
        repair: Some(json!({ "kind": "add-body-literal" })),
        citations: Vec::new(),
        data: None,
    }
}

fn rule_duplicate(name: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "rule.duplicate".to_owned(),
        message: format!(
            "a rule named '{name}' already exists; rule names are unique so `author.defeat` can cite one unambiguously — choose a different name."
        ),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn rule_unstratifiable(name: &str, reason: &EvalError) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "rule.unstratifiable".to_owned(),
        message: format!("rule '{name}' would leave the world unstratifiable: {reason}"),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn control_char(field: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "field.control-char".to_owned(),
        message: format!(
            "The {field} field carries a control or bidi/zero-width character; it could forge or reorder a declaration in the review diff. Remove it before materializing."
        ),
        grade: Grade::Checked,
        repair: Some(json!({ "kind": "remove-control-chars", "field": field })),
        citations: Vec::new(),
        data: None,
    }
}

/// The per-add referential-integrity finding for a citation naming a decision
/// the base does not declare (the authority scrub's kept half, DL-0054 clause 2).
///
/// The compiled staged world raises this too — `dli-eval`'s lowering leaves an
/// unknown decision's variable open and `check::run` surfaces the finding, which
/// is what actually bars materialize. Raising it *here* as well is not
/// redundancy: a mutating tool call must answer with the findings its own claim
/// raised (spec 11 §4), so an agent that only reads the add's reply learns the
/// citation is unresolvable at the moment it cites, rather than at the next
/// `author.check`. Both spellings carry the same code and message, so
/// `dedup_findings` collapses them in the aggregate reads.
fn citation_unknown(id: &str, known: &BTreeSet<String>) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "citation.unknown-decision".to_owned(),
        message: format!("cited decision '{id}' is not a declared decision"),
        grade: Grade::Checked,
        repair: Some(json!({
            "kind": "resolve-citation",
            "candidates": candidates(id, known),
        })),
        citations: Vec::new(),
        data: None,
    }
}

// -- lowering + wire helpers -------------------------------------------------

/// Lower a wire [`Provenance`] to an AST [`Annotation`]. `proposed` records an
/// origin tag; `hypothesis` scopes the claim to a fork-local hyp condition.
///
/// A cited claim carries its recorded decisions; referential integrity is
/// checked when the claim is staged and again before materialization.
fn annotation(provenance: &Provenance) -> Annotation {
    match provenance {
        Provenance::Cited { decisions, state } => Annotation {
            marker: match state {
                Some(ProvenanceState::Provisional) => ProvenanceMarker::Provisional,
                Some(ProvenanceState::Ratified) => ProvenanceMarker::Ratified,
                None => ProvenanceMarker::Decided,
            },
            citations: decisions
                .iter()
                .map(|id| CitationRef { id: ident(id) })
                .collect(),
            condition: None,
            tags: Vec::new(),
            span: Span::synthetic(),
        },
        Provenance::Proposed { origin } => Annotation {
            marker: ProvenanceMarker::Proposed,
            citations: Vec::new(),
            condition: None,
            tags: vec![origin.clone()],
            span: Span::synthetic(),
        },
        Provenance::Hypothesis { hyp } => Annotation {
            marker: ProvenanceMarker::Proposed,
            citations: Vec::new(),
            condition: Some(CondExpr::Hyp(Ident::synthetic(hyp))),
            tags: Vec::new(),
            span: Span::synthetic(),
        },
    }
}

/// Map the wire entity kind to the AST entity kind.
fn map_kind(kind: WireEntityKind) -> AstEntityKind {
    match kind {
        WireEntityKind::Person => AstEntityKind::Person,
        WireEntityKind::Place => AstEntityKind::Place,
        WireEntityKind::Thing => AstEntityKind::Thing,
        WireEntityKind::Era => AstEntityKind::Era,
    }
}

/// The ground string form of a term (mirrors the loader's rendering).
fn term_text(term: &Term) -> String {
    match term {
        Term::Var(id) | Term::Sym(id) => id.text.clone(),
        Term::Str(text) => text.clone(),
        Term::Int(value) => value.to_string(),
        Term::Time(dli_ast::term::TimeRef::Era(id)) => format!("era:{}", id.text),
        Term::Time(dli_ast::term::TimeRef::Event(id)) => format!("event:{}", id.text),
    }
}

fn ident(text: &str) -> Ident {
    Ident::synthetic(text)
}

/// Preserve a compact token as a symbol; render every other caller value as a
/// quoted string so the authoring seam can never claim malformed text is canonical.
fn author_value(text: &str) -> Term {
    let symbol = !text.is_empty()
        && !text.starts_with('$')
        && !text.bytes().all(|byte| byte.is_ascii_digit())
        && text.bytes().all(|byte| {
            !byte.is_ascii_control()
                && !byte.is_ascii_whitespace()
                && !matches!(byte, b'(' | b')' | b',' | b'=' | b'{' | b'}' | b'"' | b'#')
        });
    if symbol {
        Term::Sym(ident(text))
    } else {
        Term::Str(text.to_owned())
    }
}

fn hypothesis_count(ws: &Workspace) -> usize {
    ws.decls
        .iter()
        .filter(|decl| matches!(decl, Decl::Hypothesis(_)))
        .count()
}

/// A mutation result value: findings, rendered lines, and the new digest.
fn mutation(findings: Vec<Finding>, rendered: Vec<String>, digest: &str) -> Value {
    value(&AuthorMutationResult {
        findings,
        rendered,
        workspace_digest: digest.to_owned(),
        root_hash: None,
        facts: None,
    })
}

/// Promote a stored provisional claim or defeat it through the durable ref.
/// These are ordinary data operations: neither operation opens an approval
/// gate, and the decision evidence is represented in the store itself.
pub(super) fn set_standing(
    store: &mut Store,
    params: &AuthorSetStandingParams,
) -> Result<Value, ErrorBody> {
    durable_standing_change(
        store,
        &params.workspace,
        &params.claim_id,
        &params.decision,
        None,
        false,
    )
}

/// Record a defeating decision and rule the claim's existing supporting
/// decision false on the current main ref. The defeated object remains
/// reachable in the append-only store.
pub(super) fn defeat(
    store: &mut Store,
    params: &AuthorDefeatParams,
    conditional: bool,
) -> Result<Value, ErrorBody> {
    durable_standing_change(
        store,
        &params.workspace,
        &params.claim_id,
        &params.decision,
        Some(&params.defeated_decision),
        conditional,
    )
}

/// Record a baseline ratified (supporting) decision on the workspace's branch
/// (contract 1.9.0, DL-0083).
///
/// This is the `adds_decision` half of [`durable_standing_change`], extracted
/// into its own op **minus** the claim lookup, the support-cites-decision guard,
/// the marker mutation, and the defeated-atom false-flip: it writes a
/// `Decl::Ratify` carriage for `decision`, grows the commit's known-decision
/// list (idempotent on a known id), interns the id eagerly so a later
/// `author.defeat` can target it, and commits on the workspace's target branch.
/// There is **no** `Ref::with_valuation` write — the certain-bit is free: any
/// fact that cites `decision` reads certain on the next compile through the
/// ordinary ledger valuation (`ledger_valuation` rules the decision's atom true
/// for a citing fact once the corpus declares it). This closes the bootstrap gap
/// the out-of-band `fixture-authority` binary used to fill, and it does not move
/// the DL-0054 boundary: it records the caller's claim that a ruling was made,
/// as data, never deciding *who* may rule.
#[allow(clippy::too_many_lines)]
pub(super) fn record_decision(
    store: &mut Store,
    params: &AuthorRecordDecisionParams,
) -> Result<Value, ErrorBody> {
    let decision = params.decision.as_str();
    let ws0 = store.get(&params.workspace).ok_or_else(gone)?;
    let digest = ws0.digest.clone();
    // Routed to the WORKSPACE's own target branch (WS-3), read while `ws0` is
    // still a live immutable borrow, then owned so it survives the mutable
    // lorestore borrows below — never `main`, never `root_hash()` (#68).
    let target_branch = target_ref_name(ws0).to_owned();
    if store.workspace_lorestore(&params.workspace).is_none() {
        return Err(schema_error(
            "recording a decision requires an attached lorestore",
        ));
    }
    // The `ratify` carriage this op will write. Any `cited` provenance names the
    // prior decisions this ruling itself rests on; they ride in the body's typed
    // interior (`body.citations`) so the citation gate reads them the way it
    // reads a text `ratify … { cited … }`.
    let cited: Vec<String> = match &params.provenance {
        Some(Provenance::Cited { decisions, .. }) => decisions.clone(),
        _ => Vec::new(),
    };
    let ratify = Decl::Ratify(RatifyDecl {
        decision: ident(decision),
        body: dli_ast::language::RatifyBody {
            clauses: Vec::new(),
            citations: cited,
        },
    });

    // Pre-image read against the target branch's own head (never `root_hash()`,
    // which is hardcoded to `main` — a branch-only decision would otherwise leak
    // to base/siblings, #68 SEV-A).
    let (head, head_ref, pack_name, mut interner, mut decisions) = {
        let ls = store
            .workspace_lorestore(&params.workspace)
            .expect("lorestore presence checked above");
        let head_ref = ls
            .branch_ref(&target_branch)
            .map_err(|error| store_fault(&error))?
            .ok_or_else(|| {
                schema_error("the lorestore has no ref for this workspace's target branch")
            })?;
        let head = head_ref.commit.clone();
        let pack_name = ls
            .head_pack_name()
            .map_err(|error| store_fault(&error))?
            .unwrap_or_default();
        let interner = ls.interner_at(&head).map_err(|error| store_fault(&error))?;
        let decisions = ls
            .decisions_at(&head)
            .map_err(|error| store_fault(&error))?;
        (head, head_ref, pack_name, interner, decisions)
    };
    {
        let ls = store
            .workspace_lorestore(&params.workspace)
            .expect("lorestore presence checked above");
        preflight_ref_valuation(ls, &head_ref, &interner)?;
    }

    // Guard census (DL-0083): a control character in the id or rationale, and a
    // provenance citation of an unknown decision, refuse the record rather than
    // committing a tainted or dangling `ratify`. The id/citation scan runs
    // through the same `free_text_fields`/`citation_findings` census every
    // staged declaration does — `record_decision` is the first op to carry a
    // `ratify` from the wire, so those censuses grew a `Ratify` arm for it.
    let known: BTreeSet<String> = decisions.iter().cloned().collect();
    let mut findings = control_char_findings(&ratify);
    if let Some(rationale) = params.rationale.as_deref()
        && let Some(finding) = field_control("rationale", rationale)
    {
        findings.push(finding);
    }
    findings.extend(citation_findings(&ratify, &known));
    if findings.iter().any(|f| f.severity == Severity::Error) {
        return Ok(value(&AuthorMutationResult {
            findings,
            rendered: Vec::new(),
            workspace_digest: digest,
            root_hash: Some(head.to_string()),
            facts: None,
        }));
    }

    // Idempotent on a known id: the decision is already recorded, so there is
    // nothing to write and the current head IS the result.
    if decisions.iter().any(|known| known == decision) {
        return Ok(value(&AuthorMutationResult {
            findings: Vec::new(),
            rendered: Vec::new(),
            workspace_digest: digest,
            root_hash: Some(head.to_string()),
            facts: None,
        }));
    }

    // Grow the known-decision list and intern the id eagerly (as `defeat` does),
    // so a later `defeat` can target a decision that has no other carrier yet.
    decisions.push(decision.to_owned());
    if !interner.iter().any(|name| name == decision) {
        interner.push(decision.to_owned());
    }

    let commit = {
        let ls = store
            .workspace_lorestore(&params.workspace)
            .expect("lorestore presence checked above");
        let mut bundle_counter = 0;
        let mut lower_interner = dli_core::Interner::from_names(interner.clone());
        let fact_hashes = dli_lorestore::lower::lower_decl(
            &ratify,
            &pack_name,
            &mut lower_interner,
            &mut bundle_counter,
        )
        .iter()
        .map(|object| ls.write_object(object).map_err(|error| store_fault(&error)))
        .collect::<Result<Vec<_>, _>>()?;
        let receipt = ls
            .commit_on_branch_at_ref(
                &target_branch,
                Some(
                    &head_ref
                        .digest()
                        .map_err(|error| schema_error(error.to_string()))?
                        .to_string(),
                ),
                interner,
                fact_hashes,
                Vec::new(),
                Vec::new(),
                decisions,
                pack_name,
            )
            .map_err(|error| store_fault(&error))?;
        receipt.commit
    };

    store.mark_main_base_dirty(&params.workspace);

    // Advance the workspace base to the new commit so subsequent staged reads
    // (add_fact → author.check) see the recorded decision as base — the same
    // tail `durable_standing_change` runs after a defeat.
    let current_ref = store
        .workspace_lorestore(&params.workspace)
        .and_then(|ls| ls.branch_ref(&target_branch).ok().flatten())
        .ok_or_else(|| {
            schema_error("the lorestore ref for this workspace's target branch disappeared")
        })?;
    let new_ref_digest = current_ref
        .digest()
        .map_err(|error| schema_error(error.to_string()))?
        .to_string();
    let effective_valuation = store
        .workspace_lorestore(&params.workspace)
        .expect("lorestore presence checked above")
        .effective_valuation_at(&commit, &current_ref.valuation)
        .map_err(|error| store_fault(&error))?;
    let source_names = store
        .workspace_lorestore(&params.workspace)
        .expect("lorestore presence checked above")
        .interner_at(&current_ref.commit)
        .map_err(|error| store_fault(&error))?;
    let named_valuation = dli_core::valuation::NamedValuation::from_indexed(
        &effective_valuation.valuation,
        &source_names,
    )
    .map_err(|error| ErrorBody {
        code: ErrorCode::EvidenceCorrupt,
        message: error.to_string(),
        data: None,
    })?;
    let named_unassigned = dli_core::valuation::NamedAtomSet::from_indexed(
        &effective_valuation.unassigned,
        &source_names,
    )
    .map_err(|error| ErrorBody {
        code: ErrorCode::EvidenceCorrupt,
        message: error.to_string(),
        data: None,
    })?;
    if let Some((_, ws)) = store.parts_mut(&params.workspace) {
        ws.set_base_root_hash(Some(commit.to_string()));
        ws.base_ref_digest = Some(new_ref_digest);
        ws.base_valuation = named_valuation;
        ws.base_unassigned = named_unassigned;
    }
    // Reconstruct the workspace's base from the new head so the recorded `ratify`
    // is visible to staged reads (`workspace_decisions`, `author.check`): unlike
    // a `defeat`, whose effect rides the valuation overlay, a recorded decision
    // is a decl the static scope base never carried — without this refresh a
    // fact citing it would read `citation.unknown-decision`, not certain.
    store
        .reload_workspace_base(&params.workspace)
        .map_err(|message| ErrorBody {
            code: ErrorCode::Internal,
            message: format!("refreshing the workspace after recording the decision: {message}"),
            data: None,
        })?;
    let workspace_digest = store
        .get(&params.workspace)
        .ok_or_else(gone)?
        .digest
        .clone();
    Ok(value(&AuthorMutationResult {
        findings: Vec::new(),
        rendered: Vec::new(),
        workspace_digest,
        root_hash: Some(commit.to_string()),
        facts: None,
    }))
}

/// Atomically record several baseline supporting decisions in one branch
/// commit. Validation and the single candidate compile complete before any
/// object write; after the CAS succeeds, installing that prepared base is an
/// infallible in-memory swap.
#[allow(clippy::too_many_lines)]
pub(super) fn record_decisions(
    store: &mut Store,
    params: &AuthorRecordDecisionsParams,
) -> Result<Value, ErrorBody> {
    if params.records.is_empty() {
        return Err(schema_error(
            "author.record_decisions requires at least one record",
        ));
    }
    let ws0 = store.get(&params.workspace).ok_or_else(gone)?;
    let digest = ws0.digest.clone();
    let target_branch = target_ref_name(ws0).to_owned();
    if store.workspace_lorestore(&params.workspace).is_none() {
        return Err(schema_error(
            "recording decisions requires an attached lorestore",
        ));
    }

    // One immutable pre-image read for the entire request. `reconstruct_at`
    // gives the exact target-branch pack to which the new Ratify carriages will
    // be appended, including branch-only and pinned-origin state.
    let (
        head,
        head_ref,
        mut interner,
        mut decisions,
        mut candidate_pack,
        named_valuation,
        named_unassigned,
    ) = {
        let ls = store
            .workspace_lorestore(&params.workspace)
            .expect("lorestore presence checked above");
        let head_ref = ls
            .branch_ref(&target_branch)
            .map_err(|error| store_fault(&error))?
            .ok_or_else(|| {
                schema_error("the lorestore has no ref for this workspace's target branch")
            })?;
        let head = head_ref.commit.clone();
        let interner = ls.interner_at(&head).map_err(|error| store_fault(&error))?;
        let effective = ls
            .effective_valuation_at(&head, &head_ref.valuation)
            .map_err(|error| store_fault(&error))?;
        let named_valuation =
            dli_core::valuation::NamedValuation::from_indexed(&effective.valuation, &interner)
                .map_err(|error| ErrorBody {
                    code: ErrorCode::EvidenceCorrupt,
                    message: error.to_string(),
                    data: None,
                })?;
        let named_unassigned =
            dli_core::valuation::NamedAtomSet::from_indexed(&effective.unassigned, &interner)
                .map_err(|error| ErrorBody {
                    code: ErrorCode::EvidenceCorrupt,
                    message: error.to_string(),
                    data: None,
                })?;
        let decisions = ls
            .decisions_at(&head)
            .map_err(|error| store_fault(&error))?;
        let pack = ls
            .reconstruct_at(&head)
            .map_err(|error| store_fault(&error))?;
        (
            head,
            head_ref,
            interner,
            decisions,
            pack,
            named_valuation,
            named_unassigned,
        )
    };

    let existing: BTreeSet<String> = decisions.iter().cloned().collect();
    let mut citation_universe = existing.clone();
    citation_universe.extend(params.records.iter().map(|record| record.decision.clone()));

    // Build every declaration and validate against the complete batch universe.
    // That makes forward references legal. Ratify citations are referential
    // metadata, so cycles remain legal exactly as in the existing world loader.
    let mut declarations = Vec::with_capacity(params.records.len());
    let mut findings = Vec::new();
    let mut first_index = BTreeMap::<String, usize>::new();
    for (index, record) in params.records.iter().enumerate() {
        if let Some(first) = first_index.get(&record.decision).copied() {
            findings.push(Finding {
                severity: Severity::Error,
                code: "decision.duplicate-in-batch".to_owned(),
                message: format!(
                    "decision '{}' occurs more than once in this batch",
                    record.decision
                ),
                grade: Grade::Checked,
                repair: None,
                citations: Vec::new(),
                data: Some(json!({
                    "record_index": index,
                    "first_index": first,
                    "decision": record.decision,
                })),
            });
        } else {
            first_index.insert(record.decision.clone(), index);
        }
        let cited = match &record.provenance {
            Some(Provenance::Cited { decisions, .. }) => decisions.clone(),
            _ => Vec::new(),
        };
        let ratify = Decl::Ratify(RatifyDecl {
            decision: ident(&record.decision),
            body: dli_ast::language::RatifyBody {
                clauses: Vec::new(),
                citations: cited,
            },
        });
        let mut record_findings = control_char_findings(&ratify);
        if let Some(rationale) = record.rationale.as_deref()
            && let Some(finding) = field_control("rationale", rationale)
        {
            record_findings.push(finding);
        }
        record_findings.extend(citation_findings(&ratify, &citation_universe));
        for finding in &mut record_findings {
            finding.data = Some(json!({
                "record_index": index,
                "decision": record.decision,
            }));
        }
        findings.extend(record_findings);
        declarations.push(ratify);
    }

    let already_known = params
        .records
        .iter()
        .filter(|record| existing.contains(&record.decision))
        .count();
    if findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        return Ok(value(&AuthorRecordDecisionsResult {
            mutation: AuthorMutationResult {
                findings,
                rendered: Vec::new(),
                workspace_digest: digest,
                root_hash: Some(head.to_string()),
                facts: None,
            },
            requested: params.records.len(),
            already_known,
            recorded: 0,
        }));
    }

    let new_declarations: Vec<Decl> = params
        .records
        .iter()
        .zip(declarations)
        .filter_map(|(record, decl)| (!existing.contains(&record.decision)).then_some(decl))
        .collect();
    if new_declarations.is_empty() {
        return Ok(value(&AuthorRecordDecisionsResult {
            mutation: AuthorMutationResult {
                findings: Vec::new(),
                rendered: Vec::new(),
                workspace_digest: digest,
                root_hash: Some(head.to_string()),
                facts: None,
            },
            requested: params.records.len(),
            already_known,
            recorded: 0,
        }));
    }

    // Append names in request order, exactly as successful repeated singles do.
    for record in &params.records {
        if existing.contains(&record.decision) {
            continue;
        }
        decisions.push(record.decision.clone());
        if !interner.iter().any(|name| name == &record.decision) {
            interner.push(record.decision.clone());
        }
    }

    // Candidate pack preparation is the one full compile. Ratify declarations
    // have no proposal bundle side effects, and appending them to the current
    // reconstructed document matches their durable carriage order on reopen.
    if let Some(document) = candidate_pack.documents.first_mut() {
        document.decls.extend(new_declarations.iter().cloned());
    } else {
        candidate_pack.documents.push(Document {
            scope: (!candidate_pack.name.is_empty()).then(|| ScopeHeader {
                name: Ident::synthetic(&candidate_pack.name),
                span: Span::synthetic(),
            }),
            decls: new_declarations.clone(),
        });
    }
    let prepared = store
        .prepare_workspace_base(
            &params.workspace,
            candidate_pack,
            named_valuation,
            named_unassigned,
        )
        .map_err(|error| ErrorBody {
            code: ErrorCode::Internal,
            message: format!("compiling the bulk decision candidate: {error}"),
            data: None,
        })?;

    let receipt = {
        let ls = store
            .workspace_lorestore(&params.workspace)
            .expect("lorestore presence checked above");
        let mut lower_interner = dli_core::Interner::from_names(interner.clone());
        let mut bundle_counter = 0;
        let mut fact_hashes = Vec::new();
        for declaration in &new_declarations {
            for object in dli_lorestore::lower::lower_decl(
                declaration,
                &prepared.pack.name,
                &mut lower_interner,
                &mut bundle_counter,
            ) {
                fact_hashes.push(
                    ls.write_object(&object)
                        .map_err(|error| store_fault(&error))?,
                );
            }
        }
        ls.commit_on_branch_at_ref(
            &target_branch,
            Some(
                &head_ref
                    .digest()
                    .map_err(|error| schema_error(error.to_string()))?
                    .to_string(),
            ),
            interner.clone(),
            fact_hashes,
            Vec::new(),
            Vec::new(),
            decisions,
            prepared.pack.name.clone(),
        )
        .map_err(|error| store_fault(&error))?
    };

    let workspace_digest = store.install_prepared_workspace_base(
        &params.workspace,
        prepared,
        interner,
        &receipt.commit,
        &receipt.ref_digest,
    );
    Ok(value(&AuthorRecordDecisionsResult {
        mutation: AuthorMutationResult {
            findings: Vec::new(),
            rendered: Vec::new(),
            workspace_digest,
            root_hash: Some(receipt.commit.to_string()),
            facts: None,
        },
        requested: params.records.len(),
        already_known,
        recorded: new_declarations.len(),
    }))
}

#[allow(clippy::too_many_lines)]
fn durable_standing_change(
    store: &mut Store,
    workspace: &str,
    claim_id: &str,
    decision: &str,
    defeated_decision: Option<&str>,
    conditional: bool,
) -> Result<Value, ErrorBody> {
    let defeating = defeated_decision.is_some();
    let ws0 = store.get(workspace).ok_or_else(gone)?;
    let digest = ws0.digest.clone();
    // Captured once, owned: every write site below used to hardcode `main`
    // (`dli_lorestore::MAIN_REF`); this is WS-3's routing, `target_ref_name`
    // read while `ws0` is still a live immutable borrow of `store`, then held
    // as an owned `String` so it survives the mutable `workspace_lorestore`
    // borrows further down.
    let target_branch = target_ref_name(ws0).to_owned();
    let Some(_) = store.workspace_lorestore(workspace) else {
        return Err(schema_error(
            "standing operations require an attached lorestore",
        ));
    };

    let (head, head_ref, pack_name, mut interner, mut decisions, mut target) = {
        let ls = store
            .workspace_lorestore(workspace)
            .expect("lorestore presence checked above");
        // The pre-image this whole read step is against must be the WORKSPACE's
        // own target branch, not `main` — `root_hash()` is hardcoded to `main`
        // (WS-3 leakage: a branch-authored claim would read `unknown-doc`, and a
        // branch-only ratified decision would read `citation.unknown-decision`,
        // both because the lookup below would walk main's fact set instead of
        // the branch's). `head_ref` already reads the correct branch; derive
        // `head` from ITS commit rather than a second, main-only read.
        let head_ref = ls
            .branch_ref(&target_branch)
            .map_err(|error| store_fault(&error))?
            .ok_or_else(|| {
                schema_error("the lorestore has no ref for this workspace's target branch")
            })?;
        let head = head_ref.commit.clone();
        let pack_name = ls
            .head_pack_name()
            .map_err(|error| store_fault(&error))?
            .unwrap_or_default();
        let interner = ls.interner_at(&head).map_err(|error| store_fault(&error))?;
        let decisions = ls
            .decisions_at(&head)
            .map_err(|error| store_fault(&error))?;
        let hashes = ls
            .fact_hashes_from(&head)
            .map_err(|error| store_fault(&error))?;
        let mut target = None;
        for hash in &hashes {
            let object: dli_lorestore::FactObject = ls
                .read_object(hash)
                .map_err(|error| store_fault(&error))?
                .ok_or_else(|| schema_error(format!("store object {hash} is missing")))?;
            if object.claim_id.0 != claim_id {
                continue;
            }
            // A claim can have multiple stored support objects. Select the one
            // that actually carries the requested decision; taking the first
            // object can mistake an earlier proposal for the later ruling.
            let matches = match defeated_decision {
                Some(defeated) => {
                    let ratify_carrier = conditional
                        && object.expert_body.as_ref().is_some_and(|body| {
                            body.kind == dli_lorestore::ExpertDeclKind::Ratify
                                && body.name == defeated
                        });
                    ratify_carrier || support_cites_decision(&object, &interner, defeated)
                }
                None => {
                    object.provenance.as_ref().and_then(|p| p.marker)
                        == Some(ProvenanceMarker::Provisional)
                        && support_cites_decision(&object, &interner, decision)
                }
            };
            if matches {
                target = Some((hash.clone(), object));
                break;
            }
            if target.is_none() {
                target = Some((hash.clone(), object));
            }
        }
        (head, head_ref, pack_name, interner, decisions, target)
    };
    {
        let ls = store
            .workspace_lorestore(workspace)
            .expect("lorestore presence checked above");
        preflight_ref_valuation(ls, &head_ref, &interner)?;
    }

    let Some((subject_fact_hash, mut object)) = target.take() else {
        return Err(ErrorBody {
            code: ErrorCode::UnknownDoc,
            message: format!("no stored claim has claim_id `{claim_id}`"),
            data: None,
        });
    };

    if !defeating && !decisions.iter().any(|known| known == decision) {
        let known = decisions.iter().cloned().collect::<BTreeSet<_>>();
        return Ok(value(&AuthorMutationResult {
            findings: vec![citation_unknown(decision, &known)],
            rendered: Vec::new(),
            workspace_digest: digest,
            root_hash: Some(head.to_string()),
            facts: None,
        }));
    }

    if defeating {
        let defeated_decision = defeated_decision.expect("defeat carries a supporting decision");
        if defeated_decision == decision && !conditional {
            return Err(schema_error(
                "author.defeat requires distinct defeating and defeated decisions",
            ));
        }
        if !decisions.iter().any(|known| known == defeated_decision) {
            let known = decisions.iter().cloned().collect::<BTreeSet<_>>();
            return Ok(value(&AuthorMutationResult {
                findings: vec![citation_unknown(defeated_decision, &known)],
                rendered: Vec::new(),
                workspace_digest: digest,
                root_hash: Some(head.to_string()),
                facts: None,
            }));
        }
        let ratify_carrier = conditional
            && object.expert_body.as_ref().is_some_and(|body| {
                body.kind == dli_lorestore::ExpertDeclKind::Ratify && body.name == defeated_decision
            });
        if !ratify_carrier && !support_cites_decision(&object, &interner, defeated_decision) {
            return Err(schema_error(format!(
                "claim `{claim_id}` does not rest on decision `{defeated_decision}` in every support path"
            )));
        }
        if !conditional {
            object
                .provenance
                .get_or_insert_with(Default::default)
                .defeated_by = Some(decision.to_owned());
        }
    } else {
        let marker = object
            .provenance
            .as_ref()
            .and_then(|provenance| provenance.marker);
        if marker != Some(ProvenanceMarker::Provisional) {
            return Err(schema_error(
                "author.set_standing requires a provisional stored claim",
            ));
        }
        if !support_cites_decision(&object, &interner, decision) {
            return Err(schema_error(format!(
                "claim `{claim_id}` does not rest on decision `{decision}` in every support path"
            )));
        }
        let provenance = object.provenance.get_or_insert_with(Default::default);
        provenance.marker = Some(ProvenanceMarker::Ratified);
        provenance.standing_decision = Some(decision.to_owned());
        provenance.defeated_by = None;
    }
    let adds_decision = defeating && !decisions.iter().any(|known| known == decision);
    if adds_decision {
        decisions.push(decision.to_owned());
    }
    if defeating && !interner.iter().any(|name| name == decision) {
        interner.push(decision.to_owned());
    }
    let defeat_atom = match (defeated_decision, conditional) {
        (Some(defeated_decision), false) => Some(
            interner
                .iter()
                .position(|name| name == defeated_decision)
                .and_then(|index| u32::try_from(index).ok())
                .map(dli_core::Atom)
                .ok_or_else(|| {
                    schema_error(format!(
                        "defeated decision `{defeated_decision}` has no interner atom"
                    ))
                })?,
        ),
        _ => None,
    };

    let (commit, ref_digest) = {
        let ls = store
            .workspace_lorestore(workspace)
            .expect("lorestore presence checked above");
        let mut fact_hashes = if adds_decision {
            let mut bundle_counter = 0;
            let ratify = Decl::Ratify(RatifyDecl {
                decision: ident(decision),
                body: dli_ast::language::RatifyBody::default(),
            });
            let mut lower_interner = dli_core::Interner::from_names(interner.clone());
            dli_lorestore::lower::lower_decl(
                &ratify,
                &pack_name,
                &mut lower_interner,
                &mut bundle_counter,
            )
            .iter()
            .map(|object| ls.write_object(object).map_err(|error| store_fault(&error)))
            .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        if !conditional {
            fact_hashes.push(
                ls.write_object(&object)
                    .map_err(|error| store_fault(&error))?,
            );
        }
        let attack_hashes = if conditional {
            let attack = dli_lorestore::DecisionAttack::new(
                object.claim_id.clone(),
                subject_fact_hash,
                decision,
                defeated_decision.expect("conditional defeat carries a target"),
            );
            vec![
                ls.write_object(&attack)
                    .map_err(|error| store_fault(&error))?,
            ]
        } else {
            Vec::new()
        };
        let receipt = ls
            .commit_on_branch_with_attacks_at_ref(
                &target_branch,
                Some(
                    &head_ref
                        .digest()
                        .map_err(|error| schema_error(error.to_string()))?
                        .to_string(),
                ),
                interner,
                fact_hashes,
                Vec::new(),
                Vec::new(),
                attack_hashes,
                decisions,
                pack_name,
            )
            .map_err(|error| store_fault(&error))?;
        (receipt.commit, receipt.ref_digest)
    };

    // The first CAS has already advanced the ref, even if a legacy valuation
    // CAS or later workspace refresh fails.
    store.mark_main_base_dirty(workspace);

    if defeating && !conditional {
        let ls = store
            .workspace_lorestore(workspace)
            .expect("lorestore presence checked above");
        let current = ls
            .branch_ref(&target_branch)
            .map_err(|error| store_fault(&error))?
            .ok_or_else(|| {
                schema_error("the lorestore ref for this workspace's target branch disappeared")
            })?;
        let mut valuation = current.valuation;
        valuation.set(defeat_atom.expect("defeat carries an atom"), false);
        let updated = dli_lorestore::Ref::with_valuation(
            dli_lorestore::branch_ref_name(&target_branch),
            commit.clone(),
            valuation,
        );
        if !ls
            .cas_branch_ref(&target_branch, Some(ref_digest.as_str()), &updated)
            .map_err(|error| store_fault(&error))?
        {
            return Err(ErrorBody {
                code: ErrorCode::StaleFence,
                message:
                    "the lorestore changed while applying the defeat; retry from author.status"
                        .to_owned(),
                data: None,
            });
        }
    }

    let current_ref = store
        .workspace_lorestore(workspace)
        .and_then(|ls| ls.branch_ref(&target_branch).ok().flatten())
        .ok_or_else(|| {
            schema_error("the lorestore ref for this workspace's target branch disappeared")
        })?;
    let new_ref_digest = current_ref
        .digest()
        .map_err(|error| schema_error(error.to_string()))?
        .to_string();
    // A standing promotion can follow a conditional defeat. Recompute the
    // attack overlay for promotions too, or the open workspace would briefly
    // resurrect a defeated decision until it is reopened.
    let effective_valuation = if conditional || !defeating {
        store
            .workspace_lorestore(workspace)
            .expect("lorestore presence checked above")
            .effective_valuation_at(&commit, &current_ref.valuation)
            .map_err(|error| store_fault(&error))?
    } else {
        dli_lorestore::EffectiveValuation {
            valuation: current_ref.valuation.clone(),
            unassigned: BTreeSet::new(),
        }
    };
    let source_names = store
        .workspace_lorestore(workspace)
        .expect("lorestore presence checked above")
        .interner_at(&current_ref.commit)
        .map_err(|error| store_fault(&error))?;
    let named_valuation = dli_core::valuation::NamedValuation::from_indexed(
        &effective_valuation.valuation,
        &source_names,
    )
    .map_err(|error| ErrorBody {
        code: ErrorCode::EvidenceCorrupt,
        message: error.to_string(),
        data: None,
    })?;
    let named_unassigned = dli_core::valuation::NamedAtomSet::from_indexed(
        &effective_valuation.unassigned,
        &source_names,
    )
    .map_err(|error| ErrorBody {
        code: ErrorCode::EvidenceCorrupt,
        message: error.to_string(),
        data: None,
    })?;
    if let Some((_, ws)) = store.parts_mut(workspace) {
        ws.set_base_root_hash(Some(commit.to_string()));
        ws.base_ref_digest = Some(new_ref_digest);
        ws.base_valuation = named_valuation;
        ws.base_unassigned = named_unassigned;
    }
    // Both legacy false bits and 1.13 grounded attacks change only the named
    // standing overlay. Conditional attacks may carry a new Ratify object to
    // establish their endpoint, but that attack-only name is interned and
    // registered dynamically; rebuilding the symbolic documents would violate
    // saturate-once/valuate-many for every defeat and nested restoration.
    store
        .refresh_workspace_valuation(workspace)
        .map_err(|message| ErrorBody {
            code: ErrorCode::Internal,
            message: format!("refreshing the workspace after changing standing: {message}"),
            data: None,
        })?;
    let workspace_digest = store.get(workspace).ok_or_else(gone)?.digest.clone();
    Ok(value(&AuthorMutationResult {
        findings: Vec::new(),
        rendered: Vec::new(),
        workspace_digest,
        root_hash: Some(commit.to_string()),
        facts: None,
    }))
}

/// Whether `decision` supports every path of a stored object's claim.
///
/// **Expert carriage** is the class that cannot answer this the ordinary way:
/// `carriage_object` stores every carried declaration's `support` as
/// `PosBool::truth()` — a structural constant, hashed content, never touched —
/// so the declaration's actual citation lives only in its carried `Decl`, not
/// in the polynomial. `object.expert_body.body` holds that whole declaration
/// as span-normalized canonical JSON (`carriage_object`'s `decl_json_pair`);
/// deserializing it and reading `decl_annotation`'s citations directly is the
/// only way to ask whether it cites `decision`, and it needs no `dli_syntax` —
/// a `Decl` round-trips through plain `serde_json`. Plain atom claims keep
/// reading their gated `support` polynomial exactly as before.
fn support_cites_decision(
    object: &dli_lorestore::FactObject,
    interner: &[String],
    decision: &str,
) -> bool {
    if let Some(body) = &object.expert_body {
        return declaration_carriage_cites_decision(body, decision);
    }
    let Some(index) = interner.iter().position(|name| name == decision) else {
        return false;
    };
    let Ok(index) = u32::try_from(index) else {
        return false;
    };
    let atom = dli_core::Atom(index);
    let mut monomials = object.support.monomials().peekable();
    monomials.peek().is_some()
        && monomials.all(|monomial| {
            monomial
                .lits()
                .any(|lit| lit.atom == atom && lit.rail == dli_core::atom::Rail::Pos)
        })
}

/// Whether a carried declaration cites `decision`. A malformed or
/// legacy carriage (a v1 store predating LS-9's typed body, say) falls
/// closed to `false` rather than panicking — `support_cites_decision`'s
/// caller then reports the ordinary "does not support" refusal, never a
/// crash on a stored object this build cannot parse.
fn declaration_carriage_cites_decision(
    body: &dli_lorestore::object::ExpertBody,
    decision: &str,
) -> bool {
    let Some(text) = body.body.first() else {
        return false;
    };
    let Ok(decl) = serde_json::from_str::<Decl>(text) else {
        return false;
    };
    decl_annotation(&decl)
        .is_some_and(|annot| annot.citations.iter().any(|cite| cite.id.text == decision))
}

fn store_fault(error: &dli_lorestore::StoreError) -> ErrorBody {
    ErrorBody {
        code: ErrorCode::Internal,
        message: error.to_string(),
        data: None,
    }
}

/// Validate a persisted ref overlay against both its source commit table and
/// the independently compiled receiving table before a mutating author call
/// writes any object, commit, or ref.
fn preflight_ref_valuation(
    store: &dli_lorestore::Lorestore,
    reference: &dli_lorestore::Ref,
    source_names: &[String],
) -> Result<(), ErrorBody> {
    let named =
        dli_core::valuation::NamedValuation::from_indexed(&reference.valuation, source_names)
            .map_err(|error| valuation_corrupt(&error))?;
    let compiled = store
        .compile_at(&reference.commit)
        .map_err(|error| store_fault(&error))?;
    named
        .to_indexed(compiled.model.interner())
        .map_err(|error| valuation_corrupt(&error))?;
    Ok(())
}

fn valuation_corrupt(error: &dli_core::valuation::ValuationRemapError) -> ErrorBody {
    ErrorBody {
        code: ErrorCode::EvidenceCorrupt,
        message: error.to_string(),
        data: None,
    }
}

/// Rewrite every interner-local polynomial in a staged fact object by exact
/// name. Staged objects were lowered against the workspace interner; a sibling
/// may have appended a different name at the same numeric position before an
/// ordinary-main materialize snapshots its current parent.
fn remap_fact_object(
    object: &mut dli_lorestore::FactObject,
    source_names: &[String],
    target: &dli_core::Interner,
) -> Result<(), ErrorBody> {
    object.support = remap_posbool(&object.support, source_names, target)?;
    object.defeaters = remap_posbool(&object.defeaters, source_names, target)?;
    for condition in &mut object.conditions {
        condition.path = remap_posbool(&condition.path, source_names, target)?;
        condition.poison = remap_posbool(&condition.poison, source_names, target)?;
    }
    Ok(())
}

fn remap_posbool(
    formula: &dli_core::PosBool,
    source_names: &[String],
    target: &dli_core::Interner,
) -> Result<dli_core::PosBool, ErrorBody> {
    let mut monomials = BTreeSet::new();
    for monomial in formula.monomials() {
        let mut literals = Vec::new();
        for literal in monomial.lits() {
            let name = source_names
                .get(literal.atom.0 as usize)
                .ok_or_else(|| ErrorBody {
                    code: ErrorCode::EvidenceCorrupt,
                    message: format!(
                        "staged support atom {} is outside its source interner of length {}",
                        literal.atom.0,
                        source_names.len()
                    ),
                    data: None,
                })?;
            let atom = target.lookup(name).ok_or_else(|| ErrorBody {
                code: ErrorCode::EvidenceCorrupt,
                message: format!("staged support name `{name}` is absent from the commit interner"),
                data: None,
            })?;
            literals.push(dli_core::Lit {
                atom,
                rail: literal.rail,
            });
        }
        monomials.insert(dli_core::posbool::Monomial::from_lits(literals));
    }
    Ok(dli_core::PosBool(monomials).absorb())
}

/// Serialize a typed result to a wire value.
fn value<T: serde::Serialize>(result: &T) -> Value {
    serde_json::to_value(result).unwrap_or(Value::Null)
}

/// Deduplicate finding codes, preserving first-seen order.
fn dedup(codes: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    codes
        .into_iter()
        .filter(|code| seen.insert(code.clone()))
        .collect()
}

/// The decision ids in scope for a workspace — what a citation must resolve
/// against. The engine verifies that a cited thing exists; it never invents one
/// (DL-0054 clause 2).
///
/// A decision is declared by a `ratify` declaration, which is the same source
/// `dli-eval` reads when it decides whether to rule a cited decision's variable
/// true. Both the base documents and the staged layer are scanned, so a `ratify`
/// staged in this workspace counts — the reference resolves against what is
/// present, exactly as entity references do.
pub(super) fn workspace_decisions(store: &Store, ws_id: &str) -> BTreeSet<String> {
    store
        .get(ws_id)
        .map(|ws| ws.known_decisions().clone())
        .unwrap_or_default()
}

/// The referential-integrity findings a declaration's provenance raises: one per
/// cited decision that is not declared in scope. Empty for every provenance that
/// carries no citation.
fn citation_findings(decl: &Decl, decisions: &BTreeSet<String>) -> Vec<Finding> {
    // A recorded decision (`author.record_decision`, DL-0083) may itself rest on
    // prior decisions — its `provenance` citations, kept in the `ratify` body's
    // typed interior (`body.citations`, LS-9), never in an `Annotation`. So
    // `decl_annotation` cannot reach them; this arm is what makes
    // `citation.unknown-decision` integrity run on a `ratify` staged from the
    // wire (guard census), gating a dangling citation exactly as a fact's is.
    if let Decl::Ratify(ratify) = decl {
        return ratify
            .body
            .citations
            .iter()
            .map(|id| id.trim())
            .filter(|id| !decisions.contains(*id))
            .map(|id| citation_unknown(id, decisions))
            .collect();
    }
    let Some(annot) = decl_annotation(decl) else {
        return Vec::new();
    };
    annot
        .citations
        .iter()
        .map(|citation| citation.id.text.trim())
        .filter(|id| !decisions.contains(*id))
        .map(|id| citation_unknown(id, decisions))
        .collect()
}

/// The provenance annotation a declaration carries, if its kind carries one.
fn decl_annotation(decl: &Decl) -> Option<&Annotation> {
    match decl {
        Decl::Entity(e) => e.provenance.as_ref(),
        Decl::Attr(a) => a.provenance.as_ref(),
        Decl::Fact(f) => f.provenance.as_ref(),
        Decl::Before(b) => b.provenance.as_ref(),
        Decl::Question(q) => q.provenance.as_ref(),
        Decl::Dialect(d) => d.provenance.as_ref(),
        Decl::NamePart(n) => n.provenance.as_ref(),
        Decl::Name(n) => n.provenance.as_ref(),
        Decl::Term(t) => t.provenance.as_ref(),
        Decl::Banned(b) => b.provenance.as_ref(),
        Decl::State(s) => s.provenance.as_ref(),
        Decl::Functional(f) => f.provenance.as_ref(),
        Decl::Inertial(f) => f.provenance.as_ref(),
        Decl::Attest(a) => a.provenance.as_ref(),
        Decl::SameAs(s) => s.provenance.as_ref(),
        Decl::PredicateSameAs(s) => s.provenance.as_ref(),
        Decl::Rule(r) => r.provenance.as_ref(),
        _ => None,
    }
}

/// The base and a mutable workspace, or `internal` if the workspace vanished
/// (dispatch checks existence before calling a tool, so this is unreachable in
/// practice — it fails closed rather than panicking).
fn parts_mut<'a>(
    store: &'a mut Store,
    id: &str,
) -> Result<(&'a CompiledWorld, &'a mut Workspace), ErrorBody> {
    store.parts_mut(id).ok_or_else(gone)
}

fn schema_error(message: impl Into<String>) -> ErrorBody {
    ErrorBody {
        code: ErrorCode::SchemaViolation,
        message: message.into(),
        data: None,
    }
}

fn gone() -> ErrorBody {
    ErrorBody {
        code: ErrorCode::Internal,
        message: "the workspace vanished mid-call".to_owned(),
        data: None,
    }
}

/// The refusal for an `author.discard` naming a declaration the workspace does
/// not hold — never a fall-through to discarding the whole workspace (R3/C3).
fn unknown_decl(decl_id: &str) -> ErrorBody {
    ErrorBody {
        code: ErrorCode::UnknownDoc,
        message: format!("no staged declaration matching \"{decl_id}\" in this workspace"),
        data: None,
    }
}

// -- author.branch / author.merge (LS-6, contract 0.6.1) ---------------------

/// `author.branch`: create a persistent branch, list them, or delete one
/// (`delete`, WS-3, ≥ contract 1.7.0). Operates on the lorestore backing the
/// workspace's base — branches are durable divergent lines of writes, so a
/// store without a lorestore refuses (demo mode).
pub(super) fn branch(
    store: &Store,
    params: &AuthorBranchParams,
    branch_writes: bool,
) -> Result<Value, ErrorBody> {
    if !branch_writes {
        if params.expected_ref.is_some() {
            return Err(schema_error(
                "field `expected_ref` is defined only by contract 1.7.0 branch-scoped writes",
            ));
        }
        if matches!(params.action, BranchAction::Delete) {
            return Err(schema_error(
                "action `delete` is defined only by contract 1.7.0 branch-scoped writes",
            ));
        }
    }
    let base_key = store.base_key_for(&params.workspace).map(str::to_owned);
    let lorestore = store
        .workspace_lorestore(&params.workspace)
        .ok_or_else(no_lorestore)?;
    let created = match params.action {
        BranchAction::List => None,
        BranchAction::Create => {
            let name = params.name.as_deref().ok_or_else(|| ErrorBody {
                code: ErrorCode::SchemaViolation,
                message: "author.branch `create` requires a `name`".to_owned(),
                data: None,
            })?;
            let from = params.from.as_deref().unwrap_or("main");
            if let Some(expected_ref) = &params.expected_ref {
                fence_branch_ref(lorestore, from, expected_ref)?;
            }
            lorestore
                .create_branch(name, from)
                .map_err(|e| store_error(&e))?;
            Some(name.to_owned())
        }
        BranchAction::Delete => {
            let name = params.name.as_deref().ok_or_else(|| ErrorBody {
                code: ErrorCode::SchemaViolation,
                message: "author.branch `delete` requires a `name`".to_owned(),
                data: None,
            })?;
            if name == dli_lorestore::MAIN_REF {
                return Err(schema_error("the `main` branch cannot be deleted"));
            }
            if let Some(base_key) = &base_key
                && store.branch_has_open_workspace(base_key, name)
            {
                return Err(ErrorBody {
                    code: ErrorCode::Conflict,
                    message: format!(
                        "branch `{name}` has an open workspace targeting it; discard or materialize it first"
                    ),
                    data: None,
                });
            }
            lorestore
                .delete_branch(name, params.expected_ref.as_deref())
                .map_err(|e| store_error(&e))?;
            None
        }
    };
    let branches = lorestore.list_branches().map_err(|e| store_error(&e))?;
    let heads = branch_writes.then(|| branch_heads(lorestore, &branches));
    to_value(&AuthorBranchResult {
        branches,
        created,
        heads,
    })
}

/// Every listed branch's head commit and ref digest (WS-3) — a branch that
/// vanished or lost its ref between the list and this read is skipped rather
/// than failing the whole call; the list itself is the authority on what
/// exists.
fn branch_heads(lorestore: &dli_lorestore::Lorestore, branches: &[String]) -> Vec<BranchHead> {
    branches
        .iter()
        .filter_map(|name| {
            let r = lorestore.branch_ref(name).ok().flatten()?;
            let digest = r.digest().ok()?;
            Some(BranchHead {
                name: name.clone(),
                commit: r.commit.to_string(),
                ref_digest: digest.0,
            })
        })
        .collect()
}

/// Fence `branch`'s current whole-ref digest against `expected_ref` (WS-3):
/// `author.branch`'s `create` fences its FROM branch this way (`delete`
/// fences its own target through `Lorestore::delete_branch` instead, since
/// that check has to run under the same lock as the removal).
fn fence_branch_ref(
    lorestore: &dli_lorestore::Lorestore,
    branch: &str,
    expected_ref: &str,
) -> Result<(), ErrorBody> {
    let current = lorestore
        .branch_ref(branch)
        .map_err(|e| store_error(&e))?
        .ok_or_else(|| ErrorBody {
            code: ErrorCode::UnknownDoc,
            message: format!("no branch named \"{branch}\" exists"),
            data: None,
        })?;
    let digest = current.digest().map_err(|e| schema_error(e.to_string()))?.0;
    if digest != expected_ref {
        return Err(ErrorBody {
            code: ErrorCode::StaleFence,
            message: format!(
                "branch `{branch}` is at ref digest {digest}; you presented {expected_ref}"
            ),
            data: None,
        });
    }
    Ok(())
}

/// `author.merge`: merge a named branch into the current one (default `main`),
/// returning the merge commit hash and the structural conflict report.
///
/// `dry_run` (WS-3, ≥ contract 1.7.0) writes the same content-addressed merge
/// commit a real merge would and runs the full conflict scan, but skips the
/// target-ref CAS — the derived-layer conflict inspection, with no
/// commitment. `expected_ref` fences the TARGET's whole-ref digest either way.
pub(super) fn merge(
    store: &mut Store,
    params: &AuthorMergeParams,
    branch_writes: bool,
    carries_data: bool,
) -> Result<Value, ErrorBody> {
    if !branch_writes {
        if params.dry_run.is_some() {
            return Err(schema_error(
                "field `dry_run` is defined only by contract 1.7.0 branch-scoped writes",
            ));
        }
        if params.expected_ref.is_some() {
            return Err(schema_error(
                "field `expected_ref` is defined only by contract 1.7.0 branch-scoped writes",
            ));
        }
    }
    let lorestore = store
        .workspace_lorestore(&params.workspace)
        .ok_or_else(no_lorestore)?;
    let target = params.target.as_deref().unwrap_or("main");
    let advance = params.dry_run != Some(true);
    let report = lorestore
        .merge_with(
            target,
            &params.source,
            advance,
            params.expected_ref.as_deref(),
        )
        .map_err(|e| store_error(&e))?;
    if advance {
        store.mark_ref_dirty(&params.workspace, target);
    }
    let result = AuthorMergeResult {
        merge_commit: report.merge_commit.to_string(),
        merge_base: report.merge_base.map(|h| h.to_string()),
        conflicts: report.conflicts.iter().map(conflict_to_wire).collect(),
        findings: report
            .findings
            .iter()
            .map(|finding| world_finding_to_wire(finding, carries_data))
            .collect(),
        stratifies: report.stratifies,
        standing_changes: report
            .standing_changes
            .iter()
            .map(|s| MergeStandingChange {
                atom: s.atom.clone(),
                branch: s.branch.clone(),
                before: s.before.clone(),
                after: s.after.clone(),
            })
            .collect(),
    };
    to_value(&result)
}

/// The branch a workspace's writes route to (WS-3): `"main"` for every
/// workspace opened without a `branch`, or the branch name itself for one
/// opened WITH a branch (`author.open`'s `branch` param). The one seam every
/// write site that used to hardcode `dli_lorestore::MAIN_REF` now threads
/// through, so a branch workspace's existing `base_ref_digest` fence gives
/// branch fencing for free — no new fence-checking code, just the right
/// branch at each call.
///
/// This is the BARE branch name — what `Lorestore::branch_ref`,
/// `commit_on_branch_at_ref`, and `cas_branch_ref` all call their `branch`
/// parameter and qualify internally via `dli_lorestore::branch_ref_name`.
/// The one exception is constructing a [`dli_lorestore::Ref`] directly
/// (its `name` field IS the qualified storage path, matching how
/// `Lorestore::merge_impl` builds one) — call
/// `dli_lorestore::branch_ref_name(target_ref_name(ws))` there.
fn target_ref_name(ws: &Workspace) -> &str {
    ws.target_branch
        .as_deref()
        .unwrap_or(dli_lorestore::MAIN_REF)
}

/// The refusal when branches/merge are called against a store with no durable
/// lorestore (the demo store) — a durable operation in explicit demo mode.
fn no_lorestore() -> ErrorBody {
    ErrorBody {
        code: ErrorCode::ProjectRequired,
        message: "branches and merge need a durable lorestore; run against a project store"
            .to_owned(),
        data: None,
    }
}

/// Map a lorestore error to a typed wire error.
fn store_error(err: &dli_lorestore::StoreError) -> ErrorBody {
    use dli_lorestore::StoreError as SE;
    let code = match err {
        SE::NotFound(_) => ErrorCode::UnknownDoc,
        SE::BranchExists { .. } | SE::InternerRegression { .. } | SE::CasConflict { .. } => {
            ErrorCode::Conflict
        }
        // WS-3: `author.merge`'s `expected_ref` and `author.branch`
        // `delete`'s `expected_ref` both surface as this variant when a
        // caller's snapshot has gone stale — the same code `materialize`'s
        // own `RefSnapshotConflict` mapping already uses.
        SE::RefSnapshotConflict { .. } => ErrorCode::StaleFence,
        SE::ValuationRemap(_) => ErrorCode::EvidenceCorrupt,
        _ => ErrorCode::Internal,
    };
    ErrorBody {
        code,
        message: err.to_string(),
        data: None,
    }
}

/// A merge-engine [`Conflict`] on the wire as a [`DiffConflict`]. The code comes
/// from [`ConflictKind::merge_code`] — no literal lives here, so the merge
/// conflict codes are not graded materialize-gating findings of the author
/// family.
fn conflict_to_wire(conflict: &Conflict) -> DiffConflict {
    DiffConflict {
        code: conflict.kind.merge_code().to_owned(),
        kind: Some(conflict.kind),
        blame: vec![conflict.blame.0.clone(), conflict.blame.1.clone()],
        rendered: conflict.rendered.clone(),
    }
}

/// A `dli-world` [`WorldFinding`] on the wire as the contract [`Finding`].
/// `data` rides across when `carries_data` is set.
fn world_finding_to_wire(finding: &WorldFinding, carries_data: bool) -> Finding {
    Finding {
        severity: match finding.severity {
            WorldSeverity::Error => Severity::Error,
            WorldSeverity::Warn => Severity::Warn,
        },
        code: finding.code.clone(),
        message: finding.message.clone(),
        grade: finding.grade,
        repair: None,
        citations: finding.citations.clone(),
        data: carries_data.then(|| finding.data.clone()).flatten(),
    }
}

/// Serialize a tool result to a wire `Value`, or an internal error.
fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, ErrorBody> {
    serde_json::to_value(value).map_err(|e| ErrorBody {
        code: ErrorCode::Internal,
        message: e.to_string(),
        data: None,
    })
}

#[cfg(test)]
mod tests {
    use dli_ast::term::Term;

    use super::{author_value, levenshtein_bounded, nearest_by_edit_distance};

    #[test]
    fn author_values_preserve_their_term_kind_when_reparsed() {
        assert!(matches!(author_value("river-warden"), Term::Sym(_)));
        for value in ["river warden", "$keeper", "42", "name#comment", "\0"] {
            assert!(
                matches!(author_value(value), Term::Str(_)),
                "{value:?} must render as a string"
            );
        }
    }

    #[test]
    fn bounded_levenshtein_keeps_cutoff_ties_and_rejects_worse_distances() {
        let source: Vec<char> = "kitten".chars().collect();
        assert_eq!(levenshtein_bounded(&source, "sitting", Some(3)), Some(3));
        assert_eq!(levenshtein_bounded(&source, "sitting", Some(2)), None);
        assert_eq!(levenshtein_bounded(&source, "kitten", Some(0)), Some(0));
        assert_eq!(levenshtein_bounded(&source, "sitting", None), Some(3));
    }

    #[test]
    fn heap_ranking_matches_exhaustive_distance_ordering() {
        let names = ["nara", "marc", "mira", "moor", "zzzzzz", "Mára", "河守"];
        let surface = "nara";
        let chars: Vec<char> = surface.chars().collect();
        let mut expected: Vec<(usize, String)> = names
            .iter()
            .map(|name| {
                (
                    levenshtein_bounded(&chars, name, None).expect("uncut distance"),
                    (*name).to_owned(),
                )
            })
            .collect();
        expected.sort_unstable();

        for limit in 0..=names.len() + 2 {
            let actual = nearest_by_edit_distance(
                surface,
                names.iter().map(|name| (*name).to_owned()),
                limit,
            );
            assert_eq!(
                actual,
                expected.iter().take(limit).cloned().collect::<Vec<_>>()
            );
        }
    }
}
