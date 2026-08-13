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

use dli_ast::decl::{Decl, Document, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::Status;
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
    AuthorQueryResult, AuthorWhyParams, AuthorWhyResult, DiffConflict, DiffDelta, EntityCandidate,
    PredicateEntry, ValuationEntry, WorkspaceDigest, WorkspaceMode,
};
use crate::envelope::{ErrorBody, ErrorCode};
use crate::finding::{Finding, Severity};

use super::store::Store;
use super::tools::{nearest_by_edit_distance, workspace_findings};
use super::workspace::{Mode, Workspace};

// -- author.query ------------------------------------------------------------

/// Query the staged world (base + staged declarations). A goal argument written
/// `?` or `?name` is a query variable that enumerates matches; a bare symbol is a
/// ground constant. An unknown predicate over the staged world is not a fault — it
/// is "asked and answered, nothing matched" (spec 10 §2.1), returned as no rows.
pub(super) fn query(
    store: &Store,
    params: &AuthorQueryParams,
    v2: bool,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    // The 0.6.3 typed successor wins when the caller sent it; the rendered string
    // is parsed only when it did not (`goal` is a registered field-level killed
    // form whose retirement lands at contract 1.0 — dormant here).
    let goal = match &params.goal_atom {
        Some(atom) => atom.clone(),
        None => parse_goal(&params.goal)?,
    };
    let world = staged_world(store, ws).map_err(|err| eval_error(&err))?;
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
    let (root_hash, valuation) = if v2 {
        (
            Some(ws.digest.clone()),
            Some(valuation_entries(
                &world.model.world.valuation,
                world.model.interner(),
            )),
        )
    } else {
        (None, None)
    };
    Ok(value(&AuthorQueryResult {
        root_hash,
        valuation,
        rows: answer.rows,
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
pub(super) fn as_of(
    store: &Store,
    params: &AuthorAsOfParams,
    v2: bool,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let world = staged_world(store, ws).map_err(|err| eval_error(&err))?;
    let at = resolve_time_point(&world, &params.at)?;
    let projected = check::slot_state_at(&world, &params.pred, &params.subject, at);
    let root_hash = v2.then(|| ws.digest.clone());
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
    v2: bool,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let world = staged_world(store, ws).map_err(|err| eval_error(&err))?;
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
    let root_hash = v2.then(|| ws.digest.clone());
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

/// Existing entities near a surface form, nearest first, bounded by the caller's
/// `limit` (capped at [`ENTITY_CANDIDATE_CEILING`]). Entities are folded through
/// the SAME identity canon the FD/continuity check uses ([`check::identity_canon`],
/// exposed for exactly this) BEFORE scoring, so an aliased class collapses to its
/// one canonical representative — the budget is never spent twice on one class,
/// and a surfaced candidate keys the way `continuity.fd-violation` would.
pub(super) fn entity_candidates(
    store: &Store,
    params: &AuthorEntityCandidatesParams,
    v2: bool,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let world = staged_world(store, ws).map_err(|err| eval_error(&err))?;
    let canon = check::identity_canon(&world);
    // Each declared entity's sort, and the canonical-representative set folded
    // through the identity map so aliases dedupe to one candidate before scoring.
    let mut sort_of: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut reps: BTreeSet<String> = BTreeSet::new();
    for entity in world.model.entities() {
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
    let root_hash = v2.then(|| ws.digest.clone());
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
pub(super) fn check(
    store: &Store,
    params: &AuthorCheckParams,
    carries_data: bool,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let findings = workspace_report(store, ws, carries_data).findings;
    let ok = findings
        .iter()
        .all(|finding| finding.severity != Severity::Error);
    Ok(value(&AuthorCheckResult { ok, findings }))
}

/// The full staged-world report shared by status, diff, and check.
pub(super) struct WorkspaceReport {
    /// Every deduplicated finding over base plus staged declarations.
    pub findings: Vec<Finding>,
    /// The base-to-staged derived-consequence diff.
    pub consequences: DiffDelta,
    /// Structured per-fact diff rows with before/after standing (0.6.0 review
    /// surface; computed always, emitted version-dependently).
    pub diff_rows: Vec<AuthorDiffRow>,
}

/// Compile and check one workspace without mutating it. `carries_data` gates
/// whether a raised [`WorldFinding`]'s structured payload rides across as
/// `Finding.data` (WS-2, `≥1.7.0` only — [`crate::has_finding_data`]).
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
    let decisions = super::tools::workspace_decisions(store, &ws.id);
    let mut findings = workspace_findings(ws, base, &decisions);
    match staged_world(store, ws) {
        Ok(world) => {
            findings.extend(
                check::run(&world)
                    .iter()
                    .map(|finding| world_finding(finding, carries_data)),
            );
            for report in factcheck(&world, &staged_claims(&ws.decls)) {
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

// -- author.why --------------------------------------------------------------

/// Explain a staged atom's standing: the blocking rulings and defeater (why
/// certain / why dead / why possible), plus the support antichain when the world
/// derives it. The agent asks "why can't this be certain yet?" and gets the exact
/// blocker — an unruled hypothesis, an unknown entity, or a defeater.
pub(super) fn why(
    store: &Store,
    params: &AuthorWhyParams,
    records_decisions: bool,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let atom = parse_goal(&params.atom)?;
    let world = staged_world(store, ws).map_err(|err| eval_error(&err))?;
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
    // confirm it reached certainty the designed way. Floor-gated: below 1.9.0
    // the field stays empty and, being `skip_serializing_if = Vec::is_empty`,
    // absent — so pre-1.9.0 `author.why` responses stay byte-identical.
    let supporting_decisions = match (records_decisions, derived) {
        (true, Some(derived)) => {
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
    Ok(value(&AuthorWhyResult {
        why_not,
        why,
        supporting_decisions,
    }))
}

// -- author.findings ---------------------------------------------------------

/// The workspace's open findings, partitioned by severity — the standing
/// pre-flight of what `author.materialize` would refuse on, each carrying its
/// typed repair. This is the exact set the materialize gate reads, so
/// `materializable` answers "could I materialize now?".
pub(super) fn findings(
    store: &Store,
    params: &AuthorFindingsParams,
    carries_data: bool,
) -> Result<Value, ErrorBody> {
    let ws = store.get(&params.workspace).ok_or_else(gone)?;
    let all = workspace_report(store, ws, carries_data).findings;
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
/// `branch_writes` gates the row's `branch` field (WS-3, ≥ 1.7.0): this list
/// can surface workspaces a DIFFERENT caller opened, so — unlike
/// `author.open`'s own result, where a pre-1.7.0 caller could never have set
/// `branch` on its own workspace in the first place — a 1.6.0-or-earlier
/// caller here needs the field actively suppressed, not just naturally absent.
pub(super) fn list_workspaces(store: &Store, branch_writes: bool) -> Value {
    let workspaces = store
        .workspaces()
        .map(|ws| WorkspaceDigest {
            workspace: ws.id.clone(),
            base: ws.base_name.clone(),
            mode: wire_mode(ws.mode),
            staged: ws.decls.len(),
            workspace_digest: ws.digest.clone(),
            branch: branch_writes.then(|| ws.target_branch.clone()).flatten(),
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
fn staged_world(store: &Store, ws: &Workspace) -> Result<CompiledWorld, EvalError> {
    let mut documents = store
        .workspace_base_docs(&ws.id)
        .unwrap_or_default()
        .to_vec();
    documents.push(Document {
        scope: Some(ScopeHeader {
            name: Ident::synthetic(&ws.base_name),
            span: Span::synthetic(),
        }),
        decls: ws.decls.clone(),
    });
    let mut world = compile(&Pack {
        name: ws.base_name.clone(),
        documents,
    })?;
    // The workspace is anchored to the store ref selected during project load.
    // Apply that complete overlay before its local suppositions; explicit falses
    // are as meaningful as trues and must not be reconstructed as absence.
    for (atom, value) in &ws.base_valuation.0 {
        world.model.world.valuation.0.insert(*atom, *value);
    }
    for hypothesis in &ws.suppositions {
        if let Some(index) = world
            .model
            .world
            .interner
            .names()
            .iter()
            .position(|name| name == hypothesis)
            .and_then(|index| u32::try_from(index).ok())
        {
            world
                .model
                .world
                .valuation
                .set(dli_core::atom::Atom(index), true);
        }
    }
    let killed = store
        .workspace_base(&ws.id)
        .map(|base| base.killed.clone())
        .unwrap_or_default();
    Ok(world.with_killed_tokens(killed))
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
/// (WS-2, `≥1.7.0` — [`crate::has_finding_data`]).
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
fn uncompilable(err: &EvalError) -> Finding {
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
