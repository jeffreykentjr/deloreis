//! The authoring seam's read/self-check tools (`AUTH-2`): the five `author.*` reads that let an
//! agent author **and fully self-check** its work without ever writing `.dli`
//! text or falling back to the batch `dli.*` ops.
//!
//! Where the mutating tools ([`super::tools`]) stage a layer and answer per-call,
//! these read that layer as a whole:
//!
//! - `author.query` — query the **staged** world (base + staged declarations):
//!   what holds about an entity now, over work in progress.
//! - `author.check` — the full check battery (continuity + factcheck +
//!   rejected-lore) over the staged world: the pre-materialize verify of the
//!   whole draft.
//! - `author.why` — the blame/explanation of a staged atom: why it can't be
//!   certain yet (the unruled ruling, the unknown entity, the defeater),
//!   rendered.
//! - `author.findings` — the workspace's open findings by severity, each with its
//!   typed repair: what `author.materialize` would refuse on, and how to fix it.
//! - `author.list_workspaces` — the open workspaces and their digests, for
//!   context recovery.
//!
//! The reads evaluate `base + staged` by recompiling the base's source documents
//! together with the workspace's staged declarations through the one real
//! [`compile`] loader — the same lowering the whole fabric shares — so a proposed
//! staged claim reads exactly as it would once materialized (gated on an open
//! hypothesis: [`Possible`](dli_core::ann::Status::Possible), never `Certain`).
//! Findings are data, never instructions (spec 11 §4), and instruction-shaped
//! goal strings are parsed as atoms, never interpreted.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use thiserror::Error;

use dli_ast::decl::{Decl, Document, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::Status;
use dli_core::atom::Rail;
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_eval::error::EvalError;
use dli_eval::fork::{AtomChange, Standing};
use dli_eval::query::{Answer, Query, QueryOpts};
use dli_eval::why::{Why, why_not as compute_why_not};
use dli_eval::world::GroundAtom;
use dli_time::chronology::TimePoint;
use dli_world::check::factcheck::{ClaimReport, Verdict, factcheck};
use dli_world::check::{self, Severity as WorldSeverity, WorldFinding};
use dli_world::pack::Pack;
use dli_world::{CompiledWorld, compile};

use crate::contract::{
    AsOfDefeater, AsOfHolding, AuthorAsOfParams, AuthorAsOfResult, AuthorCheckParams,
    AuthorCheckResult, AuthorDiffRow, AuthorDiffStanding, AuthorEntityCandidatesParams,
    AuthorEntityCandidatesResult, AuthorFindingsParams, AuthorFindingsResult,
    AuthorListWorkspacesResult, AuthorPredicatesParams, AuthorPredicatesResult, AuthorQueryParams,
    AuthorQueryResult, AuthorQueryRow, AuthorWhyParams, AuthorWhyResult, BlockedSupport,
    BlockingDefeat, DiffConflict, DiffDelta, EntityCandidate, PredicateEntry, ValuationEntry,
    WorkspaceDigest, WorkspaceMode,
};
use crate::envelope::{ErrorBody, ErrorCode};
use crate::finding::{Finding, Severity};

use super::store::Store;
use super::tools::{nearest_by_edit_distance, workspace_findings};
use super::workspace::{Mode, Workspace};

/// Language model of a workspace, including staged declarations.
pub(crate) fn workspace_language(
    store: &Store,
    workspace: &str,
) -> Result<dli_world::language::LanguageModel, String> {
    let ws = store
        .get(workspace)
        .ok_or_else(|| format!("unknown workspace `{workspace}`"))?;
    staged_world(store, ws)
        .map(|world| world.model.language)
        .map_err(|err| err.to_string())
}

// -- author.query ------------------------------------------------------------

/// Query the staged world (base + staged declarations). A goal argument written
/// `?` or `?name` is a query variable that enumerates matches; a bare symbol is a
/// ground constant. An unknown predicate over the staged world is not a fault — it
/// is "asked and answered, nothing matched" (spec 10 §2.1), returned as no rows.
pub(super) fn query(store: &Store, params: &AuthorQueryParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    ws.note_query();
    // The 0.6.3 typed successor wins when the caller sent it; the rendered string
    // is parsed only when it did not (`goal` is a registered field-level killed
    // form whose retirement lands at contract 1.0 — dormant here).
    let goal = match &params.goal_atom {
        Some(atom) => atom.clone(),
        None => parse_goal(&params.goal)?,
    };
    let world = staged_world(store, ws).map_err(|err| staged_error(&err))?;
    let opts = QueryOpts {
        layer: params.layer.unwrap_or_default(),
        as_of: None,
        fork: None,
        why: params.why,
    };
    let answer = match world.model.world.query_with(&Query { goal }, &opts) {
        Ok(answer) => answer,
        Err(EvalError::UnknownPredicate { .. }) => Answer::default(),
        Err(err) => return Err(eval_error(&err)),
    };
    // 0.6.0: the replay context. The staged world is workspace state, so its
    // anchor is the workspace digest (jcs regime); the valuation is the full
    // ruled map — explicit falses included.
    let root_hash = Some(ws.digest.clone());
    let valuation = Some(valuation_entries(
        &world.model.world.valuation,
        world.model.interner(),
    ));
    let claim_ids = if !answer.rows.is_empty()
        && let (Some(ls), Some(root)) = (store.workspace_lorestore(&ws.id), &ws.base_root_hash)
    {
        if let Some(cached) = ws.cached_claim_index(root) {
            Some(cached)
        } else {
            let hashes = ls
                .fact_hashes_from(&dli_lorestore::ContentHash(root.clone()))
                .map_err(|err| ErrorBody {
                    code: ErrorCode::Internal,
                    message: err.to_string(),
                    data: None,
                })?;
            let mut index: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for hash in hashes {
                let object: dli_lorestore::FactObject = ls
                    .read_object(&hash)
                    .map_err(|err| ErrorBody {
                        code: ErrorCode::Internal,
                        message: err.to_string(),
                        data: None,
                    })?
                    .ok_or_else(|| ErrorBody {
                        code: ErrorCode::Internal,
                        message: format!("missing fact object {hash}"),
                        data: None,
                    })?;
                index
                    .entry(object.atom.render())
                    .or_default()
                    .insert(object.claim_id.0);
            }
            Some(ws.cache_claim_index(root.clone(), index))
        }
    } else {
        None
    };
    let rows = answer
        .rows
        .into_iter()
        .map(|row| AuthorQueryRow {
            claim_ids: claim_ids
                .as_ref()
                .and_then(|index| index.get(&row.atom))
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            row,
        })
        .collect();
    Ok(value(&AuthorQueryResult {
        root_hash,
        valuation,
        rows,
    }))
}

// -- author.as_of (contract 1.9.0, DL-0082) -----------------------------------

/// Project the inertially-current fluent value(s) of one `(pred, subject)`
/// slot at a query point over the staged world. The ONE inertia-aware read
/// (`docs/design-notes/inertia-fluent-persistence.md` A.3, DL-0082): unlike
/// [`query`], this NEVER calls `world.model.world.query_with` — it reads
/// `world.model.fluents` and `world.model.inertial_predicates` directly
/// through [`check::slot_state_at`], the same kernel projection the
/// continuity check's `continuity.fluent-contradiction`/`continuity.fluent-
/// order-undeclared` findings already derive their boundary from, so this
/// read and those findings can never disagree.
pub(super) fn as_of(store: &Store, params: &AuthorAsOfParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let world = staged_world(store, ws).map_err(|err| staged_error(&err))?;
    let at = resolve_time_point(&world, &params.at)?;
    let projected = check::slot_state_at(&world, &params.pred, &params.subject, at);
    let root_hash = Some(ws.digest.clone());
    Ok(value(&AuthorAsOfResult {
        pred: projected.pred,
        subject: projected.subject,
        at: params.at.clone(),
        inertial: projected.inertial,
        capping: projected.capping,
        contested: projected.contested,
        inertial_citations: projected.inertial_citations,
        holdings: projected
            .holdings
            .iter()
            .map(|holding| wire_holding(&world, holding))
            .collect(),
        root_hash,
    }))
}

/// Map a kernel [`check::SlotHolding`] onto its wire [`AsOfHolding`], resolving
/// every [`TimePoint`] to its interned name.
fn wire_holding(world: &CompiledWorld, holding: &check::SlotHolding) -> AsOfHolding {
    AsOfHolding {
        value: holding.value.clone(),
        projection: holding.projection,
        temporal: holding.temporal,
        support: holding.support,
        since: holding.since.map(|point| point_name(world, point)),
        until: holding.until.map(|point| point_name(world, point)),
        citations: holding.citations.clone(),
        defeater: holding.defeater.as_ref().map(|defeater| AsOfDefeater {
            value: defeater.value.clone(),
            since: defeater.since.map(|point| point_name(world, point)),
            citations: defeater.citations.clone(),
        }),
    }
}

/// Resolve a rendered timeline point name to its interned [`TimePoint`], or a
/// `schema-violation` naming the unknown point — mirrors
/// `handlers::handle_timeline_show`'s `at` resolution (the batch seam's
/// twin), so an unknown point is refused identically on both seams.
fn resolve_time_point(world: &CompiledWorld, name: &str) -> Result<TimePoint, ErrorBody> {
    world
        .model
        .world
        .time_points
        .names()
        .iter()
        .position(|point| point == name)
        .and_then(|index| u32::try_from(index).ok())
        .map(TimePoint)
        .ok_or_else(|| ErrorBody {
            code: ErrorCode::SchemaViolation,
            message: format!("unknown timeline point '{name}'"),
            data: None,
        })
}

/// The name of a time point in the world's time-point table, or a stable
/// fallback — the authoring seam's twin of `handlers::point_name`.
fn point_name(world: &CompiledWorld, point: TimePoint) -> String {
    world
        .model
        .world
        .time_points
        .names()
        .get(point.0 as usize)
        .cloned()
        .unwrap_or_else(|| format!("point#{}", point.0))
}

// -- author.predicates -------------------------------------------------------

/// The distinct predicates present in the staged world (base + staged), each
/// annotated with its observed arity and whether it was **declared** functional
/// or inertial. Enumerates the source atoms and rule heads over the same staged
/// [`CompiledWorld`] [`query`] reads. The `functional` flag reads the declared
/// `functional_predicates` set the load-check consults (never canon's inferred
/// set), so the census agrees with `continuity.fd-violation` by construction. A
/// `BTreeMap` keys the census by predicate, so the answer is deterministically
/// name-sorted.
pub(super) fn predicates(
    store: &Store,
    params: &AuthorPredicatesParams,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let world = staged_world(store, ws).map_err(|err| staged_error(&err))?;
    // pred -> arity at first occurrence; source atoms then rule heads.
    let mut arity: BTreeMap<String, usize> = BTreeMap::new();
    for atom in world.model.atoms() {
        arity
            .entry(atom.atom.pred.text.clone())
            .or_insert(atom.atom.args.len());
    }
    for rule in &world.model.world.rules {
        arity
            .entry(rule.head.pred.text.clone())
            .or_insert(rule.head.args.len());
    }
    let predicates = arity
        .into_iter()
        .map(|(pred, arity)| PredicateEntry {
            functional: world.model.functional_predicates.contains(&pred),
            inertial: world.model.inertial_predicates.contains_key(&pred),
            pred,
            arity,
        })
        .collect();
    let root_hash = Some(ws.digest.clone());
    Ok(value(&AuthorPredicatesResult {
        predicates,
        root_hash,
    }))
}

// -- author.entity_candidates ------------------------------------------------

/// The hard ceiling on `author.entity_candidates` results: a caller's `limit` is
/// capped here so an over-large value can never turn the read into a full entity
/// dump (the entity set grows without bound). Bounded-by-design is a property of
/// the engine, not a hope about caller input.
const ENTITY_CANDIDATE_CEILING: usize = 100;

/// Entities near a surface form, nearest first, bounded by the caller's `limit`
/// (capped at [`ENTITY_CANDIDATE_CEILING`]). Fact-only symbols are filtered
/// before ranking. The selected universe is then folded through the same identity
/// canon the FD/continuity check uses ([`check::identity_canon`]) BEFORE scoring, so an
/// aliased class collapses to its one canonical representative — the budget is
/// never spent twice on one class, and a surfaced candidate keys the way
/// `continuity.fd-violation` would.
pub(super) fn entity_candidates(
    store: &Store,
    params: &AuthorEntityCandidatesParams,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let world = staged_world(store, ws).map_err(|err| staged_error(&err))?;
    let canon = check::identity_canon(&world);
    // Each declared entity's sort, and the canonical-representative set folded
    // through the identity map so aliases dedupe to one candidate before scoring.
    let mut sort_of: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut reps: BTreeSet<String> = BTreeSet::new();
    for entity in world
        .model
        .entities()
        .iter()
        .filter(|entity| entity.sort.is_some())
    {
        sort_of.insert(entity.name.clone(), entity.sort.clone());
        reps.insert(dli_core::alias::canon(&canon, &entity.name).to_owned());
    }
    let limit = params.limit.min(ENTITY_CANDIDATE_CEILING);
    let candidates = nearest_by_edit_distance(&params.surface, reps, limit)
        .into_iter()
        .map(|(distance, name)| {
            let sort = sort_of.get(&name).cloned().flatten();
            EntityCandidate {
                name,
                sort,
                distance,
            }
        })
        .collect();
    let root_hash = Some(ws.digest.clone());
    Ok(value(&AuthorEntityCandidatesResult {
        candidates,
        root_hash,
    }))
}

// -- author.check ------------------------------------------------------------

/// Run the check battery over the staged world: the standing workspace findings
/// (what materialize gates on) merged with the engine's world-internal continuity
/// and rejected-lore battery ([`check::run`]) and the fact-checker's
/// contradictions ([`factcheck`]) over the staged claims. A new proposed claim is
/// legitimately *unsupported*, so only *contradictions* surface here; unsupported
/// is silent (it is candidate lore, not a defect). Findings are deduplicated.
pub(super) fn check(store: &Store, params: &AuthorCheckParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let findings = workspace_report(store, ws, true).findings;
    let ok = findings
        .iter()
        .all(|finding| finding.severity != Severity::Error);
    Ok(value(&AuthorCheckResult { ok, findings }))
}

/// The full staged-world report shared by status, diff, and check.
pub(super) struct WorkspaceReport {
    /// Every deduplicated finding over the base plus pending declarations.
    pub findings: Vec<Finding>,
    /// The base-to-staged derived-consequence diff.
    pub consequences: DiffDelta,
    /// Structured per-fact diff rows with before/after standing (0.6.0 review
    /// surface; computed always, emitted version-dependently).
    pub diff_rows: Vec<AuthorDiffRow>,
}

/// Compile and check one workspace without mutating it. `carries_data` gates
/// whether a raised [`WorldFinding`]'s structured payload rides across as
/// `Finding.data`.
pub(super) fn workspace_report(
    store: &Store,
    ws: &Workspace,
    carries_data: bool,
) -> WorkspaceReport {
    let Some(base) = store.workspace_base(&ws.id) else {
        return WorkspaceReport {
            findings: vec![Finding {
                severity: Severity::Error,
                code: "workspace.base-missing".to_owned(),
                message: "the workspace's document base is no longer loaded".to_owned(),
                grade: Grade::Checked,
                repair: None,
                citations: Vec::new(),
                data: None,
            }],
            consequences: DiffDelta::default(),
            diff_rows: Vec::new(),
        };
    };
    let mut findings = workspace_findings(ws, base);
    match staged_world(store, ws) {
        Ok(world) => {
            findings.extend(
                check::run(&world)
                    .iter()
                    .map(|finding| world_finding(finding, carries_data)),
            );
            for report in factcheck(&world, &staged_claims(&ws.pending_decls())) {
                if report.verdict == Verdict::Contradicted {
                    findings.push(contradiction_finding(&report));
                }
            }
            let findings = dedup_findings(findings);
            let (consequences, diff_rows) = classify_standings(base, &world, &findings);
            WorkspaceReport {
                findings,
                consequences,
                diff_rows,
            }
        }
        Err(err) => {
            findings.push(uncompilable(&err));
            let findings = dedup_findings(findings);
            WorkspaceReport {
                consequences: DiffDelta {
                    conflicts: findings
                        .iter()
                        .filter(|finding| finding.severity == Severity::Error)
                        .map(finding_conflict)
                        .collect(),
                    ..DiffDelta::default()
                },
                findings,
                diff_rows: Vec::new(),
            }
        }
    }
}

/// Re-run the complete materialize blocking oracle against the exact durable
/// target-parent plus staged candidate. An ordinary main workspace may have
/// opened before a sibling commit, so its earlier staged-world report is not a
/// verdict about the fold the CAS will actually create.
pub(super) fn materialize_candidate_findings(
    ws: &Workspace,
    target_parent: &CompiledWorld,
    target_documents: &[Document],
    candidate: &CompiledWorld,
    carries_data: bool,
) -> Vec<Finding> {
    // Rebuild the declaration-resolution census against the candidate rather
    // than consulting the caller workspace's stale base index. Workspace has
    // intentionally non-cloneable caches/counters, so replay only declarations
    // not already represented by the installed base into the throwaway view.
    let mut exact = Workspace::new(ws.id.clone(), ws.mode, ws.base_name.clone());
    for declaration in ws.pending_decl_refs() {
        exact.push_decl(declaration.clone());
    }
    exact.suppositions.clone_from(&ws.suppositions);
    exact.base_valuation.clone_from(&ws.base_valuation);
    exact.base_unassigned.clone_from(&ws.base_unassigned);
    exact.refresh_resolution(target_parent, target_documents);

    let mut findings = workspace_findings(&exact, target_parent);
    findings.extend(
        check::run(candidate)
            .iter()
            .map(|finding| world_finding(finding, carries_data)),
    );
    for report in factcheck(candidate, &staged_claims(&exact.decls)) {
        if report.verdict == Verdict::Contradicted {
            findings.push(contradiction_finding(&report));
        }
    }
    dedup_findings(findings)
}

// -- author.why --------------------------------------------------------------

/// Explain a staged atom's standing: the blocking rulings and defeater (why
/// certain / why dead / why possible), plus the support antichain when the world
/// derives it. The agent asks "why can't this be certain yet?" and gets the exact
/// blocker — an unruled hypothesis, an unknown entity, or a defeater.
pub(super) fn why(store: &Store, params: &AuthorWhyParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    ws.note_why();
    // The typed successor wins when the caller sent it; the rendered string is
    // parsed only when it did not (parsing needs the `.dli` front end, so the
    // typed form is what makes `supporting_decisions` reachable syntax-free).
    let atom = match &params.goal_atom {
        Some(a) => a.clone(),
        None => parse_goal(&params.atom)?,
    };
    let world = staged_world(store, ws).map_err(|err| staged_error(&err))?;
    let inner = &world.model.world;
    let why_not = compute_why_not(inner, &atom);
    let derived = GroundAtom::from_pattern(&atom)
        .and_then(|ground| inner.derived.iter().find(|derived| derived.atom == ground));
    let why = derived.map(|derived| Why::of_support(&derived.support, &inner.interner));
    // `supporting_decisions` (contract 1.9.0, DL-0083): the recorded decisions
    // in the atom's support antichain — the support monomials' claim ids
    // filtered to the workspace's known-decision set, the same filter the
    // continuity findings' `ann_citations` uses. The auditability affordance:
    // it names which recorded decisions a certain fact rests on, so a caller can
    // confirm it reached certainty the designed way.
    let supporting_decisions = match derived {
        Some(derived) => {
            let known = super::tools::workspace_decisions(store, &params.workspace);
            let mut out = Vec::new();
            for monomial in derived.support.monomials() {
                for lit in monomial.lits() {
                    if let Some(name) = inner.interner.resolve(lit.atom) {
                        let name = name.to_owned();
                        if known.contains(&name) && !out.contains(&name) {
                            out.push(name);
                        }
                    }
                }
            }
            out
        }
        _ => Vec::new(),
    };
    let (active_supporting_decision_paths, blocking_defeats, blocked_supports) = {
        let known = super::tools::workspace_decisions(store, &params.workspace);
        let attacks = store
            .workspace_decision_attacks(&params.workspace)
            .map_err(|message| ErrorBody {
                code: ErrorCode::EvidenceCorrupt,
                message,
                data: None,
            })?;
        let (active, blocked, symbolic_path_count, attack_blocked_path_count) = derived
            .map_or_else(
                || (Vec::new(), Vec::new(), 0, 0),
                |derived| {
                    let symbolic_path_count = derived.support.monomials().count();
                    let (active, blocked, attack_blocked_path_count, work) =
                        explain_support_paths(inner, derived, &known, &attacks);
                    ws.note_attack_explanation(
                        work.endpoint_resolutions,
                        work.incidence_visits,
                        work.valuation_copies,
                    );
                    (
                        active,
                        blocked,
                        symbolic_path_count,
                        attack_blocked_path_count,
                    )
                },
            );
        // The conclusion itself is attack-blocked only when every symbolic
        // support monomial is accounted for by a structured blocked entry.
        // A merely open/possible or otherwise-dead alternative prevents a
        // direct attack from being reported as the global cause.
        let blockers = if active.is_empty()
            && symbolic_path_count > 0
            && attack_blocked_path_count == symbolic_path_count
        {
            let mut blockers: Vec<_> = blocked
                .iter()
                .flat_map(|support| support.blocking_defeats.iter().cloned())
                .collect();
            blockers.sort();
            blockers.dedup();
            blockers
        } else {
            Vec::new()
        };
        (Some(active), Some(blockers), Some(blocked))
    };
    Ok(value(&AuthorWhyResult {
        why_not,
        why,
        supporting_decisions,
        active_supporting_decision_paths,
        blocking_defeats,
        blocked_supports,
    }))
}

/// Partition the symbolic antichain into currently active paths and paths made
/// inactive specifically by durable decision attacks. Path membership is read
/// from the evaluator's own conditioned annotation; the server contributes only
/// exact-name storage edges and wire shaping.
fn explain_support_paths(
    world: &dli_eval::world::World,
    derived: &dli_eval::world::Derived,
    known: &BTreeSet<String>,
    attacks: &[dli_lorestore::DecisionAttack],
) -> (
    Vec<Vec<String>>,
    Vec<BlockedSupport>,
    usize,
    AttackExplanationWork,
) {
    let ann = derived.ann();
    let mut active = Vec::new();
    let mut blocked = Vec::new();
    let mut attack_blocked_path_count = 0;
    let mut by_target = BTreeMap::<dli_core::Atom, Vec<BlockingDefeat>>::new();
    let mut restored = world.valuation.clone();
    let mut work = AttackExplanationWork {
        endpoint_resolutions: u64::try_from(attacks.len()).unwrap_or(u64::MAX) * 2,
        valuation_copies: 1,
        ..AttackExplanationWork::default()
    };

    // Resolve each stored edge exactly once per endpoint, then index only the
    // currently active attacks by target atom. Support paths visit their own
    // incident blockers; they never rescan the complete attack set.
    for attack in attacks {
        let attacker = world.interner.lookup(&attack.attacker);
        let target = world.interner.lookup(&attack.target);
        let (Some(attacker), Some(target)) = (attacker, target) else {
            continue;
        };
        if world.valuation.get(attacker) == Some(true) && world.valuation.get(target) == Some(false)
        {
            restored.set(target, true);
            by_target.entry(target).or_default().push(BlockingDefeat {
                defeated_decision: attack.target.clone(),
                defeating_decision: attack.attacker.clone(),
            });
        }
    }
    for blockers in by_target.values_mut() {
        blockers.sort();
        blockers.dedup();
    }

    for monomial in derived.support.monomials() {
        let path = named_decision_path(monomial, &world.interner, known);
        if ann.support_path_is_active(monomial, &world.valuation, &world.theory) {
            active.push(path);
            continue;
        }

        let mut path_blockers = Vec::new();
        for literal in monomial.lits() {
            if literal.rail == Rail::Pos
                && let Some(incident) = by_target.get(&literal.atom)
            {
                work.incidence_visits = work
                    .incidence_visits
                    .saturating_add(u64::try_from(incident.len()).unwrap_or(u64::MAX));
                path_blockers.extend(incident.iter().cloned());
            }
        }
        path_blockers.sort();
        path_blockers.dedup();
        if !path_blockers.is_empty()
            && ann.support_path_is_active(monomial, &restored, &world.theory)
        {
            attack_blocked_path_count += 1;
            blocked.push(BlockedSupport {
                supporting_decision_path: path,
                blocking_defeats: path_blockers,
            });
        }
    }

    active.sort();
    active.dedup();
    blocked.sort();
    blocked.dedup();
    (active, blocked, attack_blocked_path_count, work)
}

#[derive(Debug, Default)]
struct AttackExplanationWork {
    endpoint_resolutions: u64,
    incidence_visits: u64,
    valuation_copies: u64,
}

fn named_decision_path(
    monomial: &dli_core::posbool::Monomial,
    interner: &dli_core::atom::Interner,
    known: &BTreeSet<String>,
) -> Vec<String> {
    let mut names: Vec<_> = monomial
        .lits()
        .filter_map(|literal| interner.resolve(literal.atom))
        .filter(|name| known.contains(*name))
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    names
}

// -- author.findings ---------------------------------------------------------

/// The workspace's open findings, partitioned by severity — the standing
/// pre-flight of what `author.materialize` would refuse on, each carrying its
/// typed repair. This is the exact set the materialize gate reads, so
/// `materializable` answers "could I materialize now?".
pub(super) fn findings(store: &Store, params: &AuthorFindingsParams) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let all = workspace_report(store, ws, true).findings;
    let (errors, warnings): (Vec<Finding>, Vec<Finding>) = all
        .into_iter()
        .partition(|finding| finding.severity == Severity::Error);
    Ok(value(&AuthorFindingsResult {
        materializable: errors.is_empty(),
        errors,
        warnings,
    }))
}

// -- author.list_workspaces --------------------------------------------------

/// The open authoring workspaces and their digests, in id order — enough for an
/// agent to recover which workspaces it has open and where each stands. Total: a
/// store-level read that never fails.
///
/// List open workspaces and their target branches.
pub(super) fn list_workspaces(store: &Store) -> Value {
    let workspaces = store
        .workspaces()
        .map(|ws| WorkspaceDigest {
            workspace: ws.id.clone(),
            base: ws.base_name.clone(),
            mode: wire_mode(ws.mode),
            staged: ws.decls.len(),
            workspace_digest: ws.digest.clone(),
            branch: ws.target_branch.clone(),
        })
        .collect();
    value(&AuthorListWorkspacesResult { workspaces })
}

// -- the staged world --------------------------------------------------------

/// Compile the staged world: the base's source documents plus the workspace's
/// staged declarations, lowered through the one real [`compile`] loader, with the
/// base's killed-token guardrail store re-attached (compile alone leaves it
/// empty). The staged declarations enter under the workspace's own scope header so
/// they merge into the base pack and entity references resolve across both.
#[derive(Debug, Error)]
enum StagedWorldError {
    #[error(transparent)]
    Eval(#[from] EvalError),
    #[error("persisted valuation is corrupt: {0}")]
    Valuation(#[from] dli_core::valuation::ValuationRemapError),
}

fn staged_world(store: &Store, ws: &Workspace) -> Result<CompiledWorld, StagedWorldError> {
    let mut world = if let Some(cached) = ws.cached_structural_world() {
        (*cached).clone()
    } else {
        ws.note_reconstruction();
        let mut documents = store
            .workspace_base_docs(&ws.id)
            .unwrap_or_default()
            .to_vec();
        documents.push(Document {
            scope: Some(ScopeHeader {
                name: Ident::synthetic(&ws.base_name),
                span: Span::synthetic(),
            }),
            decls: ws.pending_decls(),
        });
        let killed = store
            .workspace_base(&ws.id)
            .map(|base| base.killed.clone())
            .unwrap_or_default();
        ws.note_loader_and_seal();
        let structural = compile(&Pack {
            name: ws.base_name.clone(),
            documents,
        })?
        .with_killed_tokens(killed);
        ws.cache_structural_world(structural.clone());
        structural
    };
    // The workspace is anchored to the store ref selected during project load.
    // Apply that complete overlay before its local suppositions; explicit falses
    // are as meaningful as trues and must not be reconstructed as absence.
    // Recorded attack-only decisions need not occur in any compiled support.
    // Append their stable names without disturbing existing positions, then
    // apply both assignments and explicit open states through exact names.
    for name in ws.base_valuation.0.keys() {
        world.model.world.interner.intern(name);
    }
    for name in &ws.base_unassigned.0 {
        world.model.world.interner.intern(name);
    }
    dli_core::valuation::apply_named_overlay(
        &ws.base_valuation,
        &ws.base_unassigned,
        &world.model.world.interner,
        &mut world.model.world.valuation,
    )?;
    for hypothesis in &ws.suppositions {
        if let Some(atom) = world.model.world.interner.lookup(hypothesis) {
            world.model.world.valuation.set(atom, true);
        }
    }
    ws.note_valuation(1, u64::try_from(ws.suppositions.len()).unwrap_or(u64::MAX));
    Ok(world)
}

fn classify_standings(
    base: &CompiledWorld,
    staged: &CompiledWorld,
    findings: &[Finding],
) -> (DiffDelta, Vec<AuthorDiffRow>) {
    let before = standings(base);
    let after = standings(staged);
    let mut atoms: BTreeSet<&GroundAtom> = before.keys().collect();
    atoms.extend(after.keys());
    let mut delta = DiffDelta::default();
    let mut diff_rows: Vec<AuthorDiffRow> = Vec::new();
    // Each side resolves its support through ITS OWN interner: staged-only
    // variables (hypothesis and proposal-bundle vars) do not exist in the
    // base table, and resolving through the wrong one silently drops exactly
    // the provenance the review surface exists to show (audit B3).
    let before_interner = base.model.interner();
    let after_interner = staged.model.interner();
    for atom in atoms {
        let before_standing = before.get(atom).cloned();
        let after_standing = after.get(atom).cloned();
        let before_present = before_standing.is_some();
        let after_present = after_standing.is_some();
        let (before_status, before_grade, before_support) =
            before_standing.unwrap_or((Status::Dead, Grade::Asserted, PosBool::FALSE));
        let (after_status, after_grade, after_support) =
            after_standing.unwrap_or((Status::Dead, Grade::Asserted, PosBool::FALSE));
        let was_dead = before_status == Status::Dead;
        let is_dead = after_status == Status::Dead;
        let rendered = format!(
            "{}: {} ({before_grade}) -> {} ({after_grade})",
            atom.render(),
            status_word(before_status),
            status_word(after_status)
        );
        if before_status == after_status {
            if after_status != Status::Dead
                && (before_grade != after_grade || before_support != after_support)
            {
                if before_grade != after_grade {
                    delta.grade_changed.push(atom_change(
                        atom,
                        before_status,
                        before_grade,
                        after_status,
                        after_grade,
                    ));
                }
                diff_rows.push(AuthorDiffRow {
                    atom: atom.render(),
                    before: Some(standing_row(
                        before_interner,
                        before_status,
                        before_grade,
                        &before_support,
                    )),
                    after: Some(standing_row(
                        after_interner,
                        after_status,
                        after_grade,
                        &after_support,
                    )),
                    introduced: false,
                    retracted: false,
                    rendered,
                });
            }
            continue;
        }
        let change = atom_change(atom, before_status, before_grade, after_status, after_grade);
        match after_status {
            Status::Certain => delta.newly_certain.push(change),
            Status::Possible => delta.newly_possible.push(change),
            Status::Dead => delta.newly_dead.push(change),
        }
        diff_rows.push(AuthorDiffRow {
            atom: atom.render(),
            before: before_present.then(|| {
                standing_row(
                    before_interner,
                    before_status,
                    before_grade,
                    &before_support,
                )
            }),
            after: after_present
                .then(|| standing_row(after_interner, after_status, after_grade, &after_support)),
            introduced: was_dead && !is_dead,
            retracted: !was_dead && is_dead,
            rendered,
        });
    }
    delta.conflicts = findings
        .iter()
        .filter(|finding| finding.severity == Severity::Error)
        .map(finding_conflict)
        .collect();
    (delta, diff_rows)
}

fn standings(world: &CompiledWorld) -> BTreeMap<GroundAtom, (Status, Grade, PosBool)> {
    let inner = &world.model.world;
    inner
        .derived
        .iter()
        .map(|derived| {
            (
                derived.atom.clone(),
                (
                    derived.ann().status(&inner.valuation, &inner.theory),
                    inner.grade_under(derived, &inner.valuation),
                    derived.ann().support.clone(),
                ),
            )
        })
        .collect()
}

/// Build an `AuthorDiffStanding`: the support antichain as one inner list per
/// monomial, each literal signed `+name`/`-name`, resolved through the side's
/// OWN interner and **never filtered** — a gated claim must never render as
/// unconditional (audit B3).
fn standing_row(
    interner: &dli_core::atom::Interner,
    status: Status,
    grade: Grade,
    support: &PosBool,
) -> AuthorDiffStanding {
    let support: Vec<Vec<String>> = support
        .monomials()
        .map(|m| {
            m.lits()
                .map(|lit| {
                    let sign = match lit.rail {
                        dli_core::atom::Rail::Pos => '+',
                        dli_core::atom::Rail::Neg => '-',
                    };
                    let name = interner
                        .resolve(lit.atom)
                        .map_or_else(|| format!("atom{}", lit.atom.0), str::to_owned);
                    format!("{sign}{name}")
                })
                .collect()
        })
        .collect();
    AuthorDiffStanding {
        support,
        grade,
        status: status_word(status).to_owned(),
    }
}

/// Render a valuation as the full sorted wire form — every ruled atom with
/// its boolean, explicit falses included (audit B7: a true-only list is lossy
/// and disagrees with any hash computed over the full map).
pub(crate) fn valuation_entries(
    valuation: &dli_core::valuation::Valuation,
    interner: &dli_core::atom::Interner,
) -> Vec<ValuationEntry> {
    let mut entries: Vec<ValuationEntry> = valuation
        .0
        .iter()
        .filter_map(|(atom, value)| {
            interner.resolve(*atom).map(|name| ValuationEntry {
                atom: name.to_owned(),
                value: *value,
            })
        })
        .collect();
    entries.sort_by(|a, b| a.atom.cmp(&b.atom));
    entries
}

fn atom_change(
    atom: &GroundAtom,
    before_status: Status,
    before_grade: Grade,
    after_status: Status,
    after_grade: Grade,
) -> AtomChange {
    AtomChange {
        atom: atom.clone(),
        before: Standing {
            status: before_status,
            grade: before_grade,
        },
        after: Standing {
            status: after_status,
            grade: after_grade,
        },
        rendered: format!(
            "{}: {} ({before_grade}) -> {} ({after_grade})",
            atom.render(),
            status_word(before_status),
            status_word(after_status)
        ),
    }
}

fn status_word(status: Status) -> &'static str {
    match status {
        Status::Dead => "dead",
        Status::Possible => "possible",
        Status::Certain => "certain",
    }
}

fn finding_conflict(finding: &Finding) -> DiffConflict {
    DiffConflict {
        code: finding.code.clone(),
        kind: None,
        blame: finding.citations.clone(),
        // `rendered` copies the finding message into the diff surface, so it
        // carries the same caller text and needs the same escape the message
        // gets at its serialization boundary — a raw copy here would reopen the
        // forgery on the `author.diff` consequence channel (DL-0061).
        rendered: super::render::escape_controls(&finding.message),
    }
}

/// The binary claim atoms a workspace's relations and facts assert — the input to
/// the fact-checker. A relation stages its atom verbatim; an attribute
/// `entity.key = value` lowers to the binary atom `key(entity, value)`, matching
/// the loader's own lowering so the fact-checker reads the same fact.
fn staged_claims(decls: &[Decl]) -> Vec<AtomExpr> {
    decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Fact(fact) => Some(fact.atom.clone()),
            Decl::Attr(attr) => Some(AtomExpr {
                pred: attr.attr.clone(),
                args: vec![Term::Sym(attr.subject.clone()), attr.value.clone()],
            }),
            _ => None,
        })
        .collect()
}

// -- goal parsing ------------------------------------------------------------

/// Parse a rendered goal atom (`pred(a, b)`) into an [`AtomExpr`]. An argument
/// written `?` or `?name` is a query variable; every other argument is a ground
/// symbol. A string that is not a `pred(args)` atom is a self-explanatory
/// `schema-violation` — never a bare error. A build without the front end
/// resolves nothing here and the refusal names `goal_atom`
/// (`query_goal::UNTYPED_GOAL`).
fn parse_goal(text: &str) -> Result<AtomExpr, ErrorBody> {
    crate::query_goal::parse(text).ok_or_else(|| ErrorBody {
        code: ErrorCode::SchemaViolation,
        message: format!(
            "'{text}' is not a pred(args) atom; {}",
            crate::query_goal::UNTYPED_GOAL
        ),
        data: None,
    })
}

// -- finding mapping ---------------------------------------------------------

/// Map a kernel [`WorldFinding`] onto the wire [`Finding`] (the same mapping the
/// batch `dli.check` handler uses; the read surface carries no repair for a
/// world-internal finding). `data` rides across only when `carries_data`
/// (WS-2).
fn world_finding(finding: &WorldFinding, carries_data: bool) -> Finding {
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

/// Map a fact-checker contradiction onto the wire [`Finding`]. A contradiction
/// against a certain fact (or a realized killed token) bars materialize, so it is
/// error-severity; the checker re-derives exactly, so it grades `checked`.
fn contradiction_finding(report: &ClaimReport) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "factcheck.contradicted".to_owned(),
        message: format!("{}: {}", report.claim, report.reason),
        grade: Grade::Checked,
        repair: None,
        citations: report.citations.clone(),
        data: None,
    }
}

/// An error-severity finding for a staged world that does not compile — a
/// self-contained defect the agent must resolve before materialize.
fn uncompilable(err: &StagedWorldError) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "world.uncompilable".to_owned(),
        message: format!("the staged world does not compile: {err}"),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
        data: None,
    }
}

fn staged_error(err: &StagedWorldError) -> ErrorBody {
    match err {
        StagedWorldError::Eval(error) => eval_error(error),
        StagedWorldError::Valuation(_) => ErrorBody {
            code: ErrorCode::EvidenceCorrupt,
            message: err.to_string(),
            data: None,
        },
    }
}

/// Deduplicate findings by `(code, message)`, preserving first-seen order — the
/// author findings and the engine battery can raise the same defect.
fn dedup_findings(findings: Vec<Finding>) -> Vec<Finding> {
    let mut seen = BTreeSet::new();
    findings
        .into_iter()
        .filter(|finding| seen.insert((finding.code.clone(), finding.message.clone())))
        .collect()
}

// -- wire helpers ------------------------------------------------------------

/// Serialize a typed result to a wire value.
fn value<T: serde::Serialize>(result: &T) -> Value {
    serde_json::to_value(result).unwrap_or(Value::Null)
}

/// Map the internal workspace [`Mode`] to its wire-local [`WorkspaceMode`], so the
/// contract layer stays independent of the workspace engine's internals.
fn wire_mode(mode: Mode) -> WorkspaceMode {
    match mode {
        Mode::Scope => WorkspaceMode::Scope,
        Mode::Fork => WorkspaceMode::Fork,
    }
}

/// Map a compile/eval failure of the staged world onto its wire error code.
fn eval_error(err: &EvalError) -> ErrorBody {
    let code = match err {
        EvalError::Unstratifiable { .. } => ErrorCode::Unstratifiable,
        EvalError::Contradiction { .. } => ErrorCode::Contradiction,
        EvalError::UnknownPredicate { .. } => ErrorCode::SchemaViolation,
    };
    ErrorBody {
        code,
        message: err.to_string(),
        data: None,
    }
}

/// The fail-closed error when a workspace vanishes mid-call (dispatch checks
/// existence first, so this is unreachable in practice).
fn gone() -> ErrorBody {
    ErrorBody {
        code: ErrorCode::Internal,
        message: "the workspace vanished mid-call".to_owned(),
        data: None,
    }
}

#[cfg(test)]
mod explanation_tests {
    use super::*;
    use dli_core::ann::Ann;

    #[test]
    fn unconditional_support_is_one_explicit_empty_path() {
        let atom = GroundAtom {
            pred: "unconditional".to_owned(),
            args: vec!["lyra".to_owned()],
        };
        let mut program = dli_eval::world::World::new();
        program.assert_fact(atom.clone(), Ann::unconditional());
        let world = program.fixpoint().expect("unconditional program saturates");
        let derived = world
            .derived
            .iter()
            .find(|derived| derived.atom == atom)
            .expect("unconditional fact is derived");

        let (active, blocked, attack_blocked_path_count, _) = explain_support_paths(
            &world,
            derived,
            &BTreeSet::new(),
            &Vec::<dli_lorestore::DecisionAttack>::new(),
        );
        assert_eq!(active, vec![Vec::<String>::new()]);
        assert!(blocked.is_empty());
        assert_eq!(attack_blocked_path_count, 0);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod cache_tests {
    use std::sync::{Arc, Barrier};

    use dli_ast::decl::{Decl, EntityDecl, EntityKind, HypothesisDecl};

    use super::*;
    use crate::author::workspace::CompileCacheStats;

    fn store() -> Store {
        Store::with_pack(Pack {
            name: "cache-test".to_owned(),
            documents: Vec::new(),
        })
        .expect("empty cache test pack compiles")
    }

    fn hypothesis(name: &str) -> Decl {
        Decl::Hypothesis(HypothesisDecl {
            name: Ident::synthetic(name),
            statement: Some("cache overlay probe".to_owned()),
        })
    }

    fn person(name: &str) -> Decl {
        Decl::Entity(EntityDecl {
            kind: EntityKind::Person,
            name: Ident::synthetic(name),
            label: Some(name.to_owned()),
            time: None,
            slots: Vec::new(),
            provenance: None,
        })
    }

    fn missing_goal() -> AtomExpr {
        AtomExpr {
            pred: Ident::synthetic("missing"),
            args: Vec::new(),
        }
    }

    #[test]
    fn interleaved_query_and_why_share_one_structural_compile() {
        let mut store = store();
        store
            .open(Mode::Scope, "cache-test".to_owned(), None)
            .expect("workspace opens");
        let query_params = AuthorQueryParams {
            workspace: "ws-1".to_owned(),
            goal: String::new(),
            goal_atom: Some(missing_goal()),
            layer: None,
            why: false,
        };
        let why_params = AuthorWhyParams {
            workspace: "ws-1".to_owned(),
            atom: String::new(),
            goal_atom: Some(missing_goal()),
        };

        query(&store, &query_params).expect("first query");
        why(&store, &why_params).expect("why hit");
        query(&store, &query_params).expect("second query hit");
        why(&store, &why_params).expect("second why hit");

        assert_eq!(
            store.get("ws-1").expect("workspace").compile_cache_stats(),
            CompileCacheStats {
                compiles: 1,
                hits: 3,
                misses: 1,
                evictions: 0,
            }
        );
        assert_eq!(
            store.get("ws-1").expect("workspace").read_work_stats(),
            crate::author::workspace::ReadWorkStats {
                reconstructions: 1,
                loader_runs: 1,
                fixpoint_seals: 1,
                valuations: 4,
                queries: 2,
                whys: 2,
                full_name_scans: 0,
                remap_builds: 0,
                valuation_copies: 4,
                ..crate::author::workspace::ReadWorkStats::default()
            }
        );
    }

    #[test]
    fn suppositions_are_request_local_and_structural_edits_invalidate_once() {
        let mut store = store();
        store
            .open(Mode::Fork, "cache-test".to_owned(), None)
            .expect("workspace opens");
        store
            .parts_mut("ws-1")
            .expect("workspace exists")
            .1
            .push_decl(hypothesis("h-cache"));

        let first = staged_world(&store, store.get("ws-1").expect("workspace")).expect("compile");
        let atom = first
            .model
            .world
            .interner
            .names()
            .iter()
            .position(|name| name == "h-cache")
            .and_then(|index| u32::try_from(index).ok())
            .map(dli_core::atom::Atom)
            .expect("hypothesis is interned");
        assert_eq!(first.model.world.valuation.get(atom), None);

        let ws = store.parts_mut("ws-1").expect("workspace exists").1;
        ws.suppositions.push("h-cache".to_owned());
        ws.reseal();
        let supposed = staged_world(&store, store.get("ws-1").expect("workspace")).expect("hit");
        assert_eq!(supposed.model.world.valuation.get(atom), Some(true));

        let ws = store.parts_mut("ws-1").expect("workspace exists").1;
        ws.suppositions.clear();
        ws.reseal();
        let restored = staged_world(&store, store.get("ws-1").expect("workspace")).expect("hit");
        assert_eq!(restored.model.world.valuation.get(atom), None);

        store
            .parts_mut("ws-1")
            .expect("workspace exists")
            .1
            .push_decl(person("lyra"));
        staged_world(&store, store.get("ws-1").expect("workspace")).expect("recompile");
        staged_world(&store, store.get("ws-1").expect("workspace")).expect("second hit");
        assert_eq!(
            store.get("ws-1").expect("workspace").compile_cache_stats(),
            CompileCacheStats {
                compiles: 2,
                hits: 3,
                misses: 2,
                evictions: 1,
            }
        );
        let work = store.get("ws-1").expect("workspace").read_work_stats();
        assert_eq!(work.reconstructions, 2);
        assert_eq!(work.loader_runs, 2);
        assert_eq!(work.fixpoint_seals, 2);
        assert_eq!(work.valuations, 5);
        assert_eq!(work.full_name_scans, 0);
        assert_eq!(work.remap_builds, 0);
        assert_eq!(work.supposition_probes, 1);
        assert_eq!(work.valuation_copies, 5);
    }

    #[test]
    fn disabled_cache_is_byte_equivalent_and_corrupt_hit_is_observable() {
        for mode in [Mode::Scope, Mode::Fork] {
            let mut store = store();
            store
                .open(mode, "cache-test".to_owned(), None)
                .expect("workspace opens");
            store
                .parts_mut("ws-1")
                .expect("workspace exists")
                .1
                .push_decl(person("lyra"));
            let ws = store.get("ws-1").expect("workspace");
            let enabled = staged_world(&store, ws).expect("enabled compile");
            let query_params = AuthorQueryParams {
                workspace: "ws-1".to_owned(),
                goal: String::new(),
                goal_atom: Some(missing_goal()),
                layer: None,
                why: true,
            };
            let why_params = AuthorWhyParams {
                workspace: "ws-1".to_owned(),
                atom: String::new(),
                goal_atom: Some(missing_goal()),
            };
            let enabled_query = query(&store, &query_params).expect("enabled query");
            let enabled_why = why(&store, &why_params).expect("enabled why");

            ws.set_compile_cache_enabled(false);
            let disabled = staged_world(&store, ws).expect("disabled compile");
            assert_eq!(enabled, disabled, "{mode:?}");
            assert_eq!(
                enabled_query,
                query(&store, &query_params).expect("disabled query"),
                "{mode:?} query"
            );
            assert_eq!(
                enabled_why,
                why(&store, &why_params).expect("disabled why"),
                "{mode:?} why"
            );

            ws.set_compile_cache_enabled(true);
            ws.corrupt_compile_cache();
            let corrupted = staged_world(&store, ws).expect("forced hit");
            assert_ne!(
                enabled, corrupted,
                "the mutation tooth proves a hit supplies the cached structural world"
            );
            assert!(ws.compile_cache_stats().hits >= 1);
        }
    }

    #[test]
    fn concurrent_identical_reads_are_safe_and_bounded() {
        let mut store = store();
        store
            .open(Mode::Scope, "cache-test".to_owned(), None)
            .expect("workspace opens");
        store
            .parts_mut("ws-1")
            .expect("workspace exists")
            .1
            .push_decl(person("lyra"));
        let store = Arc::new(store);
        let barrier = Arc::new(Barrier::new(8));
        let mut threads = Vec::new();
        for _ in 0..8 {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                staged_world(&store, store.get("ws-1").expect("workspace")).expect("racing compile")
            }));
        }
        let worlds: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().expect("read thread did not panic"))
            .collect();
        for world in &worlds[1..] {
            assert_eq!(world, &worlds[0]);
        }
        let stats = store.get("ws-1").expect("workspace").compile_cache_stats();
        assert!((1..=8).contains(&stats.compiles));
        assert_eq!(stats.misses + stats.hits, 8);
        assert_eq!(stats.evictions, 0, "one cache entry is retained");
    }

    #[test]
    fn add_replace_remove_and_base_refresh_have_exact_invalidation_counts() {
        let mut store = store();
        store
            .open(Mode::Scope, "cache-test".to_owned(), None)
            .expect("workspace opens");
        staged_world(&store, store.get("ws-1").unwrap()).unwrap();

        store.parts_mut("ws-1").unwrap().1.push_decl(person("lyra"));
        staged_world(&store, store.get("ws-1").unwrap()).unwrap();
        store
            .parts_mut("ws-1")
            .unwrap()
            .1
            .replace_decl(0, person("perin"));
        staged_world(&store, store.get("ws-1").unwrap()).unwrap();
        store.parts_mut("ws-1").unwrap().1.remove_decl(0);
        staged_world(&store, store.get("ws-1").unwrap()).unwrap();

        let base = store.workspace_base("ws-1").unwrap().clone();
        let documents = store.workspace_base_docs("ws-1").unwrap().to_vec();
        store
            .parts_mut("ws-1")
            .unwrap()
            .1
            .refresh_resolution(&base, &documents);
        staged_world(&store, store.get("ws-1").unwrap()).unwrap();
        let after_dynamic = store.get("ws-1").unwrap().compile_cache_stats();
        assert_eq!(after_dynamic.compiles, 4);
        assert_eq!(after_dynamic.hits, 1);
        assert_eq!(after_dynamic.evictions, 3);

        store
            .parts_mut("ws-1")
            .unwrap()
            .1
            .refresh_structure(&base, &documents);
        staged_world(&store, store.get("ws-1").unwrap()).unwrap();
        assert_eq!(
            store.get("ws-1").unwrap().compile_cache_stats(),
            CompileCacheStats {
                compiles: 5,
                hits: 1,
                misses: 5,
                evictions: 4,
            }
        );
    }

    #[test]
    #[ignore = "non-gating release profile; run with --release --ignored --nocapture"]
    #[allow(clippy::print_stderr, clippy::similar_names, clippy::too_many_lines)]
    fn release_profile_reports_size_query_and_snapshot_counter_matrix() {
        use std::fmt::Write as _;
        use std::time::Instant;

        for claim_count in [0usize, 100, 1_000, 10_000] {
            let root = tempfile::tempdir().expect("profile tempdir");
            let mut source = concat!(
                "scope cache-profile\n",
                "ratify d1\n",
                "ratify d10\n",
                "ratify d11\n",
                "person lyra \"Lyra\" ratified d11\n",
                "p(lyra) ratified d1\n",
                "rule p-to-q ratified d10\n",
                "  q(lyra) :- p(lyra)\n",
            )
            .to_owned();
            for index in 0..claim_count {
                writeln!(&mut source, "noise-{index}(lyra) ratified d11")
                    .expect("writing a String is infallible");
            }
            let (document, diagnostics) = dli_syntax::parse_document(&source);
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| !diagnostic.class.is_error()),
                "profile fixture parses: {diagnostics:?}"
            );
            let mut store = Store::with_pack(Pack {
                name: "cache-profile".to_owned(),
                documents: vec![document],
            })
            .expect("profile fixture compiles");
            store
                .attach_lorestore(root.path().join("lorestore"))
                .expect("profile lorestore attaches");
            store
                .open(Mode::Scope, "cache-profile".to_owned(), None)
                .expect("workspace opens");

            let carrier = |store: &Store, ratify: bool, name: &str| {
                let lorestore = store
                    .workspace_lorestore("ws-1")
                    .expect("profile lorestore");
                let head = lorestore.root_hash().unwrap().unwrap();
                lorestore
                    .fact_hashes_from(&head)
                    .unwrap()
                    .into_iter()
                    .find_map(|hash| {
                        let object = lorestore
                            .read_object::<dli_lorestore::FactObject>(&hash)
                            .unwrap()?;
                        if ratify {
                            object
                                .expert_body
                                .as_ref()
                                .is_some_and(|body| {
                                    body.kind == dli_lorestore::ExpertDeclKind::Ratify
                                        && body.name == name
                                })
                                .then_some(object.claim_id.0)
                        } else {
                            (object.atom.pred == name).then_some(object.claim_id.0)
                        }
                    })
                    .expect("decision has a real carrier")
            };
            let snapshot = |store: &Store| {
                let workspace = store.get("ws-1").unwrap();
                (
                    workspace.base_root_hash.clone().unwrap(),
                    workspace.base_valuation.clone(),
                    workspace.base_unassigned.clone(),
                )
            };
            let live = snapshot(&store);
            let d1_claim = carrier(&store, false, "p");
            let defeat =
                |store: &mut Store, id: &str, claim_id: &str, attacker: &str, target: &str| {
                    let response = crate::author::dispatch(
                        &crate::Request {
                            dli: "1.13.0".to_owned(),
                            id: id.to_owned(),
                            op: crate::Op::AuthorDefeat,
                            params: serde_json::json!({
                                "workspace": "ws-1",
                                "claim_id": claim_id,
                                "decision": attacker,
                                "defeated_decision": target,
                            }),
                            expected_digest: None,
                            idempotency_key: None,
                        },
                        store,
                    );
                    assert!(response.ok, "profile defeat succeeds: {response:?}");
                };
            defeat(&mut store, "direct", &d1_claim, "d2", "d1");
            let direct = snapshot(&store);
            let d2_claim = carrier(&store, true, "d2");
            defeat(&mut store, "nested", &d2_claim, "d3", "d2");
            let nested = snapshot(&store);
            let snapshots = [live, direct, nested];

            for logical_queries in [1usize, 3, 15] {
                for snapshot_count in [2usize, 3] {
                    let base = store.workspace_base("ws-1").unwrap().clone();
                    let base_documents = store.workspace_base_docs("ws-1").unwrap().to_vec();
                    store
                        .parts_mut("ws-1")
                        .unwrap()
                        .1
                        .refresh_structure(&base, &base_documents);
                    let before_cache = store.get("ws-1").unwrap().compile_cache_stats();
                    let before_work = store.get("ws-1").unwrap().read_work_stats();

                    let reconstruction_started = Instant::now();
                    let pack = Pack {
                        name: "cache-profile".to_owned(),
                        documents: base_documents.clone(),
                    };
                    let reconstruction = reconstruction_started.elapsed();
                    let loader_started = Instant::now();
                    let loaded = dli_world::loader::Loader::load(&pack.documents).world;
                    let loader = loader_started.elapsed();
                    let seal_started = Instant::now();
                    dli_world::seal_loaded(loaded, &pack).expect("profile seal succeeds");
                    let fixpoint_seal = seal_started.elapsed();

                    let query_params = AuthorQueryParams {
                        workspace: "ws-1".to_owned(),
                        goal: String::new(),
                        goal_atom: Some(AtomExpr {
                            pred: Ident::synthetic("q"),
                            args: vec![Term::Sym(Ident::synthetic("lyra"))],
                        }),
                        layer: None,
                        why: false,
                    };
                    let why_params = AuthorWhyParams {
                        workspace: "ws-1".to_owned(),
                        atom: String::new(),
                        goal_atom: query_params.goal_atom.clone(),
                    };
                    let mut query_time = std::time::Duration::ZERO;
                    let mut why_time = std::time::Duration::ZERO;
                    for (root_hash, valuation, unassigned) in snapshots.iter().take(snapshot_count)
                    {
                        let ws = store.parts_mut("ws-1").expect("workspace exists").1;
                        ws.set_base_root_hash(Some(root_hash.clone()));
                        ws.base_valuation = valuation.clone();
                        ws.base_unassigned = unassigned.clone();
                        for _ in 0..logical_queries {
                            let started = Instant::now();
                            query(&store, &query_params).expect("profile query");
                            query_time += started.elapsed();
                            let started = Instant::now();
                            why(&store, &why_params).expect("profile why");
                            why_time += started.elapsed();
                        }
                    }
                    let valuation_started = Instant::now();
                    staged_world(&store, store.get("ws-1").unwrap())
                        .expect("profile valuation hit");
                    let valuation = valuation_started.elapsed();

                    let after_cache = store.get("ws-1").unwrap().compile_cache_stats();
                    let after_work = store.get("ws-1").unwrap().read_work_stats();
                    let reads = u64::try_from(snapshot_count * logical_queries * 2)
                        .expect("small profile matrix");
                    assert_eq!(after_cache.compiles - before_cache.compiles, 1);
                    assert_eq!(after_cache.misses - before_cache.misses, 1);
                    assert_eq!(after_cache.hits - before_cache.hits, reads);
                    assert_eq!(after_work.reconstructions - before_work.reconstructions, 1);
                    assert_eq!(after_work.loader_runs - before_work.loader_runs, 1);
                    assert_eq!(after_work.fixpoint_seals - before_work.fixpoint_seals, 1);
                    assert_eq!(after_work.valuations - before_work.valuations, reads + 1);
                    assert_eq!(after_work.queries - before_work.queries, reads / 2);
                    assert_eq!(after_work.whys - before_work.whys, reads / 2);
                    assert_eq!(after_work.full_name_scans, 0);
                    assert_eq!(after_work.remap_builds, 0);
                    assert_eq!(
                        after_work.attack_endpoint_resolutions
                            - before_work.attack_endpoint_resolutions,
                        if snapshot_count == 2 {
                            u64::try_from(logical_queries * 2).unwrap()
                        } else {
                            u64::try_from(logical_queries * 6).unwrap()
                        }
                    );
                    assert_eq!(
                        after_work.attack_incidence_visits - before_work.attack_incidence_visits,
                        u64::try_from(logical_queries).unwrap()
                    );
                    assert_eq!(
                        after_work.valuation_copies - before_work.valuation_copies,
                        reads + 1 + reads / 2
                    );
                    eprintln!(
                        "claims={claim_count} logical_queries={logical_queries} snapshots={snapshot_count} \
                         compiles=1 hits={reads} misses=1 reconstruction={reconstruction:?} \
                         loader={loader:?} fixpoint_seal={fixpoint_seal:?} valuation={valuation:?} \
                         query={query_time:?} why={why_time:?}"
                    );
                }
            }
        }
    }
}
