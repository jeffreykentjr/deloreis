//! Batch `dli.*` op handlers (`SERVE-1`): wire params → engine calls → wire
//! results.
//!
//! Engine types never leak past this module. A `dli_eval::fork::ForkDelta`
//! becomes a [`DiffDelta`], a `dli_eval::fork::Preflight` becomes a
//! [`ForkPreflightResult`], a `dli_world::check::WorldFinding` becomes a
//! [`Finding`] — the mapping lives here so the wire contract and the engine can
//! evolve independently. [`handle`] is the frozen dispatch seam the serve loop
//! (and `CLI-1`) call; each op has one small typed handler.
//!
//! Every operation has a typed dispatch arm, but operations whose documented
//! durability semantics have not landed remain capability-gated. `dli.version`
//! advertises only operations that currently satisfy their contract.

use std::collections::BTreeMap;

// The contract module is one flat table of param/result structs; a scoped glob
// is clearer than a 40-line import list, exactly as `schema.rs` does it.
#[allow(clippy::wildcard_imports)]
use crate::contract::*;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use dli_core::ann::Ann;
use dli_core::atom::{Interner, Rail};
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;

use dli_eval::EvalError;
use dli_eval::fork::{Addition, ForkDelta, ForkError, KeepStamp, Visibility};
use dli_eval::query::{Query, QueryOpts};
use dli_eval::world::{GroundAtom, World};

use dli_ast::decl::{Decl, ProofDecl};
use dli_ast::term::AtomExpr;

use dli_time::chronology::{TemporalVerdict, TimePoint};

use dli_export::profile::ExportTarget;
use dli_export::{ExportOpts, ExportProfile};
use dli_game::model::{Verdict, VerdictRule};
use dli_game::{DealBattery, ReadingInvariant, ReadingMetric, ReadingObservable};
use dli_ledger::ledger_check::check_proof;
use dli_ledger::practice::{PracticeDef, compile_practices};
use dli_sim::{Battery, GenBounds, Invariant, InvariantSpec, Metric, Observable};

use dli_world::check;
use dli_world::check::{Severity as PayloadSeverity, Severity as WorldSeverity, WorldFinding};
use dli_world::episode::{Episode, fold, knower_universe};
use dli_world::episode_order::{order_integrity, season_slack};
use dli_world::language::{LangFinding, LanguageModel, vet_candidate_in_dialect, view};
use dli_world::payload::{BatteryContext, check_payload};
use dli_world::scene_projection::project_scene;

use crate::envelope::{ErrorBody, ErrorCode, Request, Response};
use crate::evidence::{self, EvidenceError};
use crate::finding::{Finding, Severity};
use crate::ops::Op;
use crate::session::{Scope, Sessions};

/// A handler failure: a typed wire code and a domain-spoken message.
type Fault = (ErrorCode, String);

/// Route a `dli.*` request to its handler and build the response envelope. The
/// serve loop routes `author.*` ops elsewhere (the Door T seam); a stray
/// `author.*` op reaching here is an internal routing bug, reported as such.
#[must_use]
pub fn handle(request: &Request, sessions: &mut Sessions) -> Response {
    let outcome = match request.op {
        Op::Version => handle_version(request),
        Op::Check => handle_check(request, sessions),
        Op::Query => handle_query(request, sessions),
        Op::ForkNew => handle_fork_new(request, sessions),
        Op::ForkList => handle_fork_list(request, sessions),
        Op::ForkDiff => handle_fork_diff(request, sessions),
        Op::ForkSuppose => handle_fork_suppose(request, sessions),
        Op::ForkSuppress => handle_fork_suppress(request, sessions),
        Op::ForkRestrict => handle_fork_restrict(request, sessions),
        Op::ForkPreflight => handle_fork_preflight(request, sessions),
        Op::ForkPromote => handle_fork_promote(request, sessions),
        Op::CheckDraft => handle_check_draft(request, sessions),
        Op::PayloadSubmit => handle_payload_submit(request, sessions),
        Op::CorpusImport => handle_corpus_import(request, sessions),
        Op::CorpusExport => handle_corpus_export(request, sessions),
        Op::NameCheck => handle_name_check(request, sessions),
        Op::AttestMissing => handle_attest_missing(request, sessions),
        Op::AttestationSweep => handle_attestation_sweep(request, sessions),
        // check-proof replays the document's proof block against its practice and
        // the certain-layer scene projection, returning the rich `ProofReport`.
        Op::CheckProof => handle_check_proof(request, sessions),
        Op::ExportRun => handle_export_run(request, sessions),
        Op::SimulateStart => handle_simulate_start(request, sessions),
        Op::SimulateStatus => handle_simulate_status(request, sessions),
        Op::SimulateCancel => handle_simulate_cancel(request, sessions),
        // fork.add parses full `.dli` declaration lines through the syntax layer
        // and adds the lowered facts to the fork; the timeline reads project the
        // resolved world's chronology and the episode fold. Params are validated
        // first, so a malformed request is a schema error either way.
        Op::ForkAdd => handle_fork_add(request, sessions),
        Op::TimelineShow => handle_timeline_show(request, sessions),
        Op::TimelineOrder => handle_timeline_order(request, sessions),
        Op::TimelineKnows => handle_timeline_knows(request, sessions),
        Op::TimelineEpisodes => handle_timeline_episodes(request, sessions),
        Op::TimelinePhases => handle_timeline_phases(request, sessions),
        // author.* is routed through the Door T seam before reaching here.
        Op::AuthorOpen
        | Op::AuthorStatus
        | Op::AuthorDiff
        | Op::AuthorDiscard
        | Op::AuthorMaterialize
        | Op::AuthorAddEntity
        | Op::AuthorAddFact
        | Op::AuthorAddRelation
        | Op::AuthorAddEvent
        | Op::AuthorAddBefore
        | Op::AuthorOpenQuestion
        | Op::AuthorAddOption
        | Op::AuthorAddHypothesis
        | Op::AuthorSuppose
        | Op::AuthorQuery
        | Op::AuthorCheck
        | Op::AuthorWhy
        | Op::AuthorFindings
        | Op::AuthorListWorkspaces
        | Op::AuthorBranch
        | Op::AuthorMerge => Err((
            ErrorCode::Internal,
            "author.* op reached the dli handler; it should route through Door T".to_owned(),
        )),
    };
    match outcome {
        Ok(response) => response,
        Err((code, message)) => error_response(request, code, message),
    }
}

// -- per-op handlers ---------------------------------------------------------

fn handle_version(request: &Request) -> Result<Response, Fault> {
    let _: VersionParams = params(request)?;
    // Dual-mode: the handshake reports the version THIS conversation is
    // speaking — a `0.5.1` caller keeps its byte-identical `0.5.1` handshake
    // until it opts into 0.6.0 (DL-0045 ruling 4).
    Ok(ok_response(
        request,
        &VersionResult {
            contract: "dli-worker".to_owned(),
            contract_version: request.dli.clone(),
            server: concat!("deloreis ", env!("CARGO_PKG_VERSION")).to_owned(),
            capabilities: capabilities(&request.dli),
        },
    ))
}

fn handle_check(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: CheckParams = params(request)?;
    let project = sessions.project_handle();
    let pack = sessions.pack(&p.document);
    let scope = resolve(sessions, &p.document)?;
    let world_findings = check::run(&scope.world);
    let mut procedure_grades = vec![Grade::Checked];
    procedure_grades.extend(world_findings.iter().map(|finding| finding.grade));
    let mut findings = world_findings
        .iter()
        .map(|finding| CheckFinding::World(world_finding(finding)))
        .collect::<Vec<_>>();
    if let Some(project) = project {
        let pack = pack.ok_or_else(|| {
            (
                ErrorCode::UnknownDoc,
                format!("no document named '{}' is loaded", p.document),
            )
        })?;
        let document = project.document(&p.document).ok_or_else(|| {
            (
                ErrorCode::UnknownDoc,
                format!("no document named '{}' is loaded", p.document),
            )
        })?;
        let records = evidence::load_payloads(&project, &p.document)
            .map_err(|error| evidence_fault(&error))?;
        for loaded in records {
            let record = loaded.record;
            let mut context = BatteryContext::new(&record.source).with_documents(&pack.documents);
            if let Some(outlines) = &document.outlines {
                context = context.with_outlines(&outlines.episodes);
            }
            if let Some(season) = &document.craft_ledger {
                context = context.with_season(season);
            }
            if let Some(principles) = &document.craft_principles {
                context = context.with_kb(dli_world::craft::Kb::new(principles.principles()));
            }
            if let Some(thresholds) = document.craft_thresholds {
                context = context.with_thresholds(thresholds);
            }
            if let Some(policy) = &document.craft_policy {
                context = context.with_episode_policy(policy.clone());
            }
            let payload_findings = check_payload(&scope.world, &record.payload, &context);
            procedure_grades.extend(payload_findings.iter().map(|finding| finding.grade));
            let episode = record.payload.meta.episode.clone();
            let source_file = record.payload.meta.source.file.clone();
            findings.extend(payload_findings.into_iter().map(|finding| {
                CheckFinding::Payload(PayloadCheckFinding {
                    payload_digest: loaded.digest.clone(),
                    episode: episode.clone(),
                    source_file: source_file.clone(),
                    finding,
                })
            }));
        }
        let checked = evidence::check_attestations(&project, &p.document, &scope.world.model)
            .map_err(|error| evidence_fault(&error))?;
        for attestation in checked {
            let finding = attestation_check_finding(attestation);
            procedure_grades.push(finding.procedure_grade());
            findings.push(CheckFinding::Attestation(finding));
        }
    }
    findings.sort_by(|left, right| check_finding_key(left).cmp(&check_finding_key(right)));
    let ok = findings.iter().all(check_finding_passes);
    let grade = complete_check_grade(procedure_grades);
    Ok(ok_response(
        request,
        &CheckResult {
            ok,
            grade,
            findings,
        },
    ))
}

fn complete_check_grade(grades: impl IntoIterator<Item = Grade>) -> Grade {
    Grade::weakest(grades)
}

fn check_finding_passes(finding: &CheckFinding) -> bool {
    match finding {
        CheckFinding::World(finding) => finding.severity != Severity::Error,
        CheckFinding::Payload(finding) => finding.finding.severity != PayloadSeverity::Error,
        CheckFinding::Attestation(finding) => finding.passes(),
    }
}

fn check_finding_key(finding: &CheckFinding) -> (u8, &str, &str) {
    match finding {
        CheckFinding::World(finding) => (0, &finding.code, &finding.message),
        CheckFinding::Payload(finding) => (1, &finding.finding.code, &finding.finding.message),
        CheckFinding::Attestation(finding) => (2, finding.code(), finding.message()),
    }
}

fn handle_query(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: QueryParams = params(request)?;
    let goal = goal_atom(p.goal_atom.as_ref(), &p.goal)?;
    // Resolve an optional fork handle to its valuation overlay first, so its
    // (owned) valuation outlives the query scope borrow (H9, `--fork`).
    let fork = match &p.fork {
        Some(handle) => Some(fork_valuation(sessions, handle, &p.document)?),
        None => None,
    };
    let opts = QueryOpts {
        layer: p.layer.unwrap_or_default(),
        as_of: p.as_of.clone(),
        fork,
        why: p.why,
    };
    let scope = resolve(sessions, &p.document)?;
    let answer = scope
        .world
        .model
        .world
        .query_with(&Query { goal }, &opts)
        .map_err(eval_fault)?;
    let interner = scope.world.model.interner();
    // 0.6.0 replay context: bind the compiled world (facts, derived set,
    // interner) together with the EFFECTIVE valuation — the ambient rulings
    // with any fork overlay folded in — and the as-of cut. The digest is
    // domain-prefixed `world:sha256:` — a binding of compiled state, never a
    // loadable store address (audit B6). The wire valuation is the full
    // ruled map, explicit falses included (audit B7).
    let context = if crate::speaks_v2(&request.dli) {
        let mut effective = scope.world.model.world.valuation.clone();
        if let Some(overlay) = &opts.fork {
            for (atom, value) in &overlay.0 {
                effective.0.insert(*atom, *value);
            }
        }
        let entries = crate::author::reads::valuation_entries(&effective, interner);
        let digest = world_binding_digest(&scope.world, &effective, p.as_of.as_deref());
        Some((digest, entries))
    } else {
        None
    };
    let rows = answer
        .rows
        .into_iter()
        .map(|row| {
            let citations = citations_of(&row.verdict.support, interner);
            let status = match row.status {
                dli_core::ann::Status::Dead => "dead",
                dli_core::ann::Status::Possible => "possible",
                dli_core::ann::Status::Certain => "certain",
            };
            QueryResultRow {
                rendered: format!("{status}  {}  ({})", row.atom, row.verdict.grade),
                atom: row.atom,
                bindings: row.bindings,
                status: row.status,
                support: row.verdict.support,
                grade: row.verdict.grade,
                citations,
                why: row.why,
                root_hash: context.as_ref().map(|(digest, _)| digest.clone()),
                valuation: context.as_ref().map(|(_, entries)| entries.clone()),
            }
        })
        .collect();
    let result = QueryResult {
        root_hash: context.as_ref().map(|(digest, _)| digest.clone()),
        valuation: context.map(|(_, entries)| entries),
        rows,
    };
    Ok(ok_response(request, &result))
}

/// The `world:sha256:` binding of a compiled world under an effective
/// valuation and as-of cut: extensional facts with their support polynomials,
/// the derived set, the interner name table, the full valuation, and the cut.
/// Two worlds differing in any of these bind differently; the digest resolves
/// in no store by design.
fn world_binding_digest(
    world: &dli_world::CompiledWorld,
    effective: &Valuation,
    as_of: Option<&str>,
) -> String {
    use sha2::{Digest, Sha256};
    let model = &world.model;
    let interner = model.interner();
    let inner = &model.world;
    let mut sections: Vec<String> = Vec::new();
    let mut facts: Vec<String> = inner
        .facts
        .iter()
        .map(|fact| {
            format!(
                "{}|{}",
                fact.atom.render(),
                dli_lorestore::lower::render_polynomial(&fact.ann.support, interner)
            )
        })
        .collect();
    facts.sort();
    sections.push(format!("facts:{}", facts.join(";")));
    let mut derived: Vec<String> = inner.derived.iter().map(|d| d.atom.render()).collect();
    derived.sort();
    sections.push(format!("derived:{}", derived.join(";")));
    sections.push(format!("interner:{}", interner.names().join(";")));
    let mut ruled: Vec<String> = effective
        .0
        .iter()
        .filter_map(|(atom, value)| {
            interner
                .resolve(*atom)
                .map(|name| format!("{name}={value}"))
        })
        .collect();
    ruled.sort();
    sections.push(format!("valuation:{}", ruled.join(";")));
    sections.push(format!("as_of:{}", as_of.unwrap_or("")));
    let mut hasher = Sha256::new();
    for section in &sections {
        hasher.update(section.as_bytes());
        hasher.update([0u8]);
    }
    format!("world:sha256:{:x}", hasher.finalize())
}

fn handle_fork_new(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ForkNewParams = params(request)?;
    let base = match &p.base {
        Some(handle) => Some(fork_id_for_document(sessions, handle, &p.document)?),
        None => None,
    };
    let scope = resolve(sessions, &p.document)?;
    let fork = match base {
        Some(parent) => scope.forks.fork_from(parent).map_err(fork_fault)?,
        None => scope.forks.fork(),
    };
    let handle = sessions.register_fork(&p.document, fork);
    let (_, scope) = fork_scope(sessions, &handle)?;
    scope
        .forks
        .set_name(fork, handle.clone())
        .map_err(fork_fault)?;
    Ok(ok_response(request, &ForkNewResult { fork: handle }))
}

fn handle_fork_list(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ForkListParams = params(request)?;
    let handles = sessions.fork_records(&p.document);
    let handle_by_id: BTreeMap<_, _> = handles
        .iter()
        .map(|(handle, id)| (*id, handle.clone()))
        .collect();
    let scope = resolve(sessions, &p.document)?;
    let mut forks = Vec::new();
    for (handle, id) in handles {
        let state = scope.forks.get(id).map_err(fork_fault)?;
        let assumes = state
            .overlay
            .iter()
            .map(|(atom, value)| {
                let name = scope
                    .forks
                    .base()
                    .interner
                    .resolve(*atom)
                    .map_or_else(|| format!("var#{}", atom.0), ToOwned::to_owned);
                (name, *value)
            })
            .collect();
        forks.push(ForkSummary {
            fork: handle,
            base: state
                .parent
                .and_then(|parent| handle_by_id.get(&parent).cloned()),
            assumes,
            local_atoms: state.additions.len(),
            restrictions: state.keep_stamps.len(),
        });
    }
    Ok(ok_response(request, &ForkListResult { forks }))
}

fn handle_fork_diff(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ForkDiffParams = params(request)?;
    let (id, scope) = fork_scope(sessions, &p.fork)?;
    let delta = scope.forks.diff(id).map_err(fork_fault)?;
    Ok(ok_response(request, &diff_delta(&delta)))
}

fn handle_fork_suppose(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ForkSupposeParams = params(request)?;
    let (id, scope) = fork_scope(sessions, &p.fork)?;
    scope
        .forks
        .suppose_all(id, p.assume.iter().map(String::as_str), true)
        .map_err(fork_fault)?;
    let delta = scope.forks.diff(id).map_err(fork_fault)?;
    Ok(ok_response(request, &diff_delta(&delta)))
}

fn handle_fork_suppress(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ForkSuppressParams = params(request)?;
    let (id, scope) = fork_scope(sessions, &p.fork)?;
    scope
        .forks
        .suppress(id, &p.assumption)
        .map_err(fork_fault)?;
    let delta = scope.forks.diff(id).map_err(fork_fault)?;
    Ok(ok_response(request, &diff_delta(&delta)))
}

fn handle_fork_restrict(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ForkRestrictParams = params(request)?;
    let (id, scope) = fork_scope(sessions, &p.fork)?;
    // The wire carries only the scope; a restriction hides the scoped atoms from
    // the fork's presented consequences (visibility, not valuation — spec 02 §5).
    scope
        .forks
        .restrict(
            id,
            KeepStamp {
                scope: p.scope.clone(),
                visibility: Visibility::KeptAbsent,
            },
        )
        .map_err(fork_fault)?;
    let delta = scope.forks.diff(id).map_err(fork_fault)?;
    Ok(ok_response(request, &diff_delta(&delta)))
}

fn handle_fork_preflight(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ForkPreflightParams = params(request)?;
    let (id, scope) = fork_scope(sessions, &p.fork)?;
    let pre = scope.forks.preflight(id).map_err(fork_fault)?;
    Ok(ok_response(
        request,
        &ForkPreflightResult {
            candidate: named_valuation(&pre.candidate, &scope.forks.base().interner),
            conflicts: pre.conflicts.iter().map(diff_conflict).collect(),
            affected: pre.affected.iter().map(GroundAtom::render).collect(),
        },
    ))
}

fn handle_fork_promote(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ForkPromoteParams = params(request)?;
    let (id, scope) = fork_scope(sessions, &p.fork)?;
    let rq = scope.forks.promote(id).map_err(fork_fault)?;
    Ok(ok_response(
        request,
        &ForkPromoteResult {
            rq_draft: RqDraft {
                slug: rq.slug,
                ask: rq.ask,
                affects: rq.affects,
                diff: rq.diff,
                evidence: rq.evidence,
            },
        },
    ))
}

fn handle_fork_add(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ForkAddParams = params(request)?;
    // Prefer the 0.6.3 typed successor; otherwise parse the declaration lines
    // through the syntax layer and lower them to ground facts. Either way the
    // additions are built BEFORE touching session state, so a malformed decl is a
    // schema error caught up front, never a half-applied fork.
    let additions = match &p.additions {
        Some(atoms) => additions_from_atoms(atoms),
        None => additions_from_declarations(&p.declarations)?,
    };
    let (id, scope) = fork_scope(sessions, &p.fork)?;
    scope.forks.add(id, additions).map_err(fork_fault)?;
    // The consequence delta runs the fork's incremental fixpoint over the base.
    let delta = scope.forks.diff(id).map_err(fork_fault)?;
    Ok(ok_response(
        request,
        &ForkAddResult {
            ok: true,
            delta: diff_delta(&delta),
        },
    ))
}

/// Lift typed ground atoms to fork-local additions — the 0.6.3 typed successor
/// to [`additions_from_declarations`], reaching the same additions without a
/// parse. Each atom is asserted by fiat with an unconditional annotation,
/// exactly as the lowered text path does, so the two forms are
/// indistinguishable downstream. No lexical validation is imposed on a typed
/// atom, matching the `author.add_relation` precedent for structured atoms on
/// the wire.
fn additions_from_atoms(atoms: &[GroundAtom]) -> Vec<Addition> {
    atoms
        .iter()
        .map(|atom| Addition::asserted(atom.clone(), Ann::unconditional()))
        .collect()
}

/// Parse a run of `.dli` declaration lines and lower them to fork-local
/// additions. Each lowered ground fact or slot becomes an [`Addition`] asserted by fiat within
/// the fork (`dli.fork.add`, *propose a fact* — the same fork-local assertion the
/// CLI's `fork add` makes): the fork is a what-if, so a proposed line is supposed
/// true inside it and its consequences are read off the diff. A parse error or an
/// unground fact is a clean [`ErrorCode::SchemaViolation`], never a panic.
/// Declarations that are not facts or slots are refused instead of being
/// accepted and silently dropped.
///
/// LS-7 gates the parse: a build without the `syntax` feature refuses the whole
/// text form by naming [`ForkAddParams::additions`], the 0.6.3 typed successor
/// that reaches the same additions.
#[cfg(feature = "syntax")]
fn additions_from_declarations(declarations: &[String]) -> Result<Vec<Addition>, Fault> {
    let source = declarations.join("\n");
    let (document, diags) = dli_syntax::parse_document(&source);
    if let Some(diag) = diags.iter().find(|diag| diag.class.is_error()) {
        return Err((
            ErrorCode::SchemaViolation,
            format!("the declaration is not well-formed .dli: {}", diag.message),
        ));
    }
    if document
        .decls
        .iter()
        .any(|decl| !matches!(decl, Decl::Fact(_) | Decl::Attr(_)))
    {
        return Err((
            ErrorCode::SchemaViolation,
            "fork.add accepts only ground facts and slot declarations".to_owned(),
        ));
    }
    let world = World::lower(&document).map_err(eval_fault)?;
    Ok(world
        .facts
        .iter()
        .map(|fact| Addition::asserted(fact.atom.clone(), Ann::unconditional()))
        .collect())
}

/// The default (parser-free) build's answer to the text form: a refusal that
/// names the typed successor. Never a silent drop — the fork is left untouched.
#[cfg(not(feature = "syntax"))]
fn additions_from_declarations(_declarations: &[String]) -> Result<Vec<Addition>, Fault> {
    Err((
        ErrorCode::SchemaViolation,
        "this build links no `.dli` front end, so `declarations` cannot be read; send the typed \
         `additions` successor (ground atoms) instead"
            .to_owned(),
    ))
}

fn handle_check_draft(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: CheckDraftParams = params(request)?;
    let project = sessions.project_handle();
    let pack = sessions.pack(&p.document).ok_or_else(|| {
        (
            ErrorCode::UnknownDoc,
            format!("no document named '{}' is loaded", p.document),
        )
    })?;
    let scope = resolve(sessions, &p.document)?;
    let mut context = BatteryContext::new(&p.source).with_documents(&pack.documents);
    if let Some(continuity) = &p.continuity {
        context = context.with_runtime_continuity(continuity);
    }
    if let Some(document) = project
        .as_ref()
        .and_then(|project| project.document(&p.document))
    {
        if let Some(outlines) = &document.outlines {
            context = context.with_outlines(&outlines.episodes);
        }
        if let Some(season) = &document.craft_ledger {
            context = context.with_season(season);
        }
        if let Some(principles) = &document.craft_principles {
            context = context.with_kb(dli_world::craft::Kb::new(principles.principles()));
        }
        if let Some(thresholds) = document.craft_thresholds {
            context = context.with_thresholds(thresholds);
        }
        if let Some(policy) = &document.craft_policy {
            context = context.with_episode_policy(policy.clone());
        }
    }
    Ok(ok_response(
        request,
        &CheckDraftResult {
            findings: check_payload(&scope.world, &p.payload, &context),
        },
    ))
}

fn handle_payload_submit(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: PayloadSubmitParams = params(request)?;
    let project = require_project(sessions)?;
    let pack = sessions.pack(&p.document).ok_or_else(|| {
        (
            ErrorCode::UnknownDoc,
            format!("no document named '{}' is loaded", p.document),
        )
    })?;
    let scope = resolve(sessions, &p.document)?;
    let mut context = BatteryContext::new(&p.source).with_documents(&pack.documents);
    if let Some(document) = project.document(&p.document) {
        if let Some(outlines) = &document.outlines {
            context = context.with_outlines(&outlines.episodes);
        }
        if let Some(season) = &document.craft_ledger {
            context = context.with_season(season);
        }
        if let Some(principles) = &document.craft_principles {
            context = context.with_kb(dli_world::craft::Kb::new(principles.principles()));
        }
        if let Some(thresholds) = document.craft_thresholds {
            context = context.with_thresholds(thresholds);
        }
        if let Some(policy) = &document.craft_policy {
            context = context.with_episode_policy(policy.clone());
        }
    }
    let findings = check_payload(&scope.world, &p.payload, &context);
    let accepted = findings
        .iter()
        .all(|finding| finding.severity != PayloadSeverity::Error);
    let payload_digest = if accepted {
        Some(
            evidence::submit_payload(&project, &p.document, &p.payload, &p.source)
                .map_err(|error| evidence_fault(&error))?,
        )
    } else {
        None
    };
    Ok(ok_response(
        request,
        &PayloadSubmitResult {
            accepted,
            findings,
            payload_digest,
        },
    ))
}

fn handle_check_proof(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: CheckProofParams = params(request)?;
    // The document's source declarations carry both the proof block and the
    // practices; they resolve deterministically from the same corpus the scope
    // compiles, so the report is projected against exactly the checked world.
    let pack = sessions.pack(&p.document).ok_or_else(|| {
        (
            ErrorCode::UnknownDoc,
            format!("no document named '{}' is loaded", p.document),
        )
    })?;
    let proofs = pack
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .filter_map(|decl| match decl {
            Decl::Proof(proof) => Some(proof),
            _ => None,
        })
        .collect::<Vec<_>>();
    let proof: &ProofDecl = match p.proof.as_deref() {
        Some(name) => {
            let matches = proofs
                .iter()
                .copied()
                .filter(|proof| proof.name.text == name)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [proof] => proof,
                [] => {
                    return Err((
                        ErrorCode::SchemaViolation,
                        format!("document '{}' declares no proof named '{name}'", p.document),
                    ));
                }
                _ => {
                    return Err((
                        ErrorCode::SchemaViolation,
                        format!(
                            "document '{}' declares proof '{name}' more than once",
                            p.document
                        ),
                    ));
                }
            }
        }
        None => match proofs.as_slice() {
            [proof] => proof,
            [] => {
                return Err((
                    ErrorCode::SchemaViolation,
                    format!(
                        "document '{}' declares no proof block to replay",
                        p.document
                    ),
                ));
            }
            _ => {
                let names = proofs
                    .iter()
                    .map(|proof| proof.name.text.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err((
                    ErrorCode::SchemaViolation,
                    format!(
                        "document '{}' declares multiple proofs ({names}); pass `proof` to select one",
                        p.document
                    ),
                ));
            }
        },
    };
    let practices: Vec<PracticeDef> = pack.documents.iter().flat_map(compile_practices).collect();
    let practice_name = proof.practice().ok_or_else(|| {
        (
            ErrorCode::SchemaViolation,
            "the proof names no practice — its header carries no `in <practice>`".to_owned(),
        )
    })?;
    let practice = practices
        .iter()
        .find(|practice| practice.name.text == practice_name)
        .ok_or_else(|| {
            (
                ErrorCode::SchemaViolation,
                format!("the document declares no practice named '{practice_name}'"),
            )
        })?;
    let speaker = proof.actor().map(ToOwned::to_owned);
    let scope = resolve(sessions, &p.document)?;
    let scene = project_scene(&scope.world, speaker.as_deref());
    let report = check_proof(practice, proof, &scene);
    Ok(ok_response(request, &CheckProofResult { report }))
}

fn handle_export_run(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ExportRunParams = params(request)?;
    let source_commit = sessions.export_source(&p.document).ok_or_else(|| {
        (
            ErrorCode::UnknownDoc,
            format!("no document or game named '{}' is loaded", p.document),
        )
    })?;
    match p.profile {
        ExportProfile::ContextPack
        | ExportProfile::Articy
        | ExportProfile::Csv
        | ExportProfile::StaticWiki => export_lore(request, sessions, &p, source_commit),
        ExportProfile::Cards | ExportProfile::LudiiGdl => {
            export_game(request, sessions, &p, source_commit)
        }
    }
}

fn export_lore(
    request: &Request,
    sessions: &mut Sessions,
    params: &ExportRunParams,
    source_commit: String,
) -> Result<Response, Fault> {
    let registry = sessions.export_registry(&params.document).ok_or_else(|| {
        if sessions.ruleset(&params.document).is_some() {
            (
                ErrorCode::SchemaViolation,
                format!(
                    "the requested profile renders a lore document; '{}' names a card game",
                    params.document
                ),
            )
        } else {
            (
                ErrorCode::UnknownDoc,
                format!("no document or game named '{}' is loaded", params.document),
            )
        }
    })?;
    let scope = resolve(sessions, &params.document)?;
    let outcome = dli_export::export_with(
        &registry,
        ExportTarget::from_wire(params.profile),
        &scope.world,
        &ExportOpts {
            source_commit,
            profile_name: params.profile_name.clone(),
            ..ExportOpts::default()
        },
    )
    .map_err(|err| {
        (
            ErrorCode::SchemaViolation,
            format!("the export profile was refused: {err}"),
        )
    })?;
    let emission = match outcome.artifact {
        dli_export::ExportArtifact::ContextPack(pack) => ExportEmission::ContextPack(pack),
        dli_export::ExportArtifact::Articy(artifact) => ExportEmission::Articy(artifact),
        dli_export::ExportArtifact::Csv(artifact) => ExportEmission::Csv(artifact),
        dli_export::ExportArtifact::StaticWiki(artifact) => ExportEmission::StaticWiki(artifact),
    };
    Ok(ok_response(
        request,
        &ExportRunResult {
            emission,
            manifest: outcome.manifest,
        },
    ))
}

fn export_game(
    request: &Request,
    sessions: &Sessions,
    params: &ExportRunParams,
    source_commit: String,
) -> Result<Response, Fault> {
    let (ruleset, registry) = sessions.ruleset(&params.document).ok_or_else(|| {
        if sessions.pack(&params.document).is_some() {
            (
                ErrorCode::SchemaViolation,
                format!(
                    "the requested profile renders a card ruleset; document '{}' names no \
                     card game (a lore corpus carries no ruleset)",
                    params.document
                ),
            )
        } else {
            (
                ErrorCode::UnknownDoc,
                format!("no document or game named '{}' is loaded", params.document),
            )
        }
    })?;
    let target = if matches!(params.profile, ExportProfile::Cards) {
        ExportTarget::Cards
    } else {
        ExportTarget::LudiiGdl
    };
    let export = dli_game::export::run(
        &registry,
        target,
        &ruleset,
        &ExportOpts {
            source_commit,
            profile_name: params.profile_name.clone(),
            ..ExportOpts::default()
        },
    )
    .map_err(|err| {
        (
            ErrorCode::SchemaViolation,
            format!("the export profile was refused: {err}"),
        )
    })?;
    let emission = match export.artifact {
        dli_game::export::GameArtifact::Cards(deck) => ExportEmission::Cards(deck),
        dli_game::export::GameArtifact::LudiiGdl(gdl) => ExportEmission::LudiiGdl(gdl),
    };
    Ok(ok_response(
        request,
        &ExportRunResult {
            emission,
            manifest: export.manifest,
        },
    ))
}

fn handle_simulate_start(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: SimulateStartParams = params(request)?;
    if p.checkpoint_every == 0 {
        return Err((
            ErrorCode::SchemaViolation,
            "`checkpoint_every` must be at least 1".to_owned(),
        ));
    }
    let project = require_project(sessions)?;
    let battery = if let Some(pack) = sessions.pack(&p.document) {
        let practice = pack
            .documents
            .iter()
            .flat_map(compile_practices)
            .next()
            .ok_or_else(|| {
                (
                    ErrorCode::SchemaViolation,
                    format!("document '{}' declares no practice to simulate", p.document),
                )
            })?;
        crate::simulation::JobBattery::Practice(Battery {
            practice,
            bounds: GenBounds {
                max_steps: SIM_MAX_STEPS,
                max_magnitude: SIM_MAX_MAGNITUDE,
            },
            seed: p.seed,
            trials: p.trials,
            metrics: default_metrics(&pack),
            invariants: default_invariants(&pack),
            affine_caps: std::collections::BTreeMap::new(),
        })
    } else if let Some((ruleset, _)) = sessions.ruleset(&p.document) {
        if let VerdictRule::Registered { name } = &ruleset.verdict {
            return Err((
                ErrorCode::SchemaViolation,
                format!(
                    "game '{}' needs registered verdict predicate '{name}', which a standalone \
                     project cannot supply; use a data-defined completed verdict",
                    p.document
                ),
            ));
        }
        let layout = ruleset
            .available_layouts()
            .first()
            .map(|layout| layout.name.clone())
            .ok_or_else(|| {
                (
                    ErrorCode::SchemaViolation,
                    format!("game '{}' has no dealable layout to simulate", p.document),
                )
            })?;
        crate::simulation::JobBattery::Game(DealBattery {
            ruleset,
            layout,
            seed: p.seed,
            trials: p.trials,
            metrics: vec![ReadingMetric {
                name: "completed-rate".to_owned(),
                observable: ReadingObservable::VerdictIs {
                    verdict_id: Verdict::COMPLETED.to_owned(),
                },
            }],
            invariants: vec![
                ReadingInvariant::DeckConserved,
                ReadingInvariant::OnlyLegalPlays,
            ],
        })
    } else {
        return Err((
            ErrorCode::UnknownDoc,
            format!("no document or game named '{}' is loaded", p.document),
        ));
    };
    let job = crate::simulation::start_job(&project, &p.document, battery, p.checkpoint_every)
        .map_err(|error| evidence_fault(&error))?;
    if matches!(
        job.status,
        SimulateJobStatus::Pending | SimulateJobStatus::Running
    ) {
        sessions.queue_simulation_worker(job.job_id.clone());
    }
    Ok(ok_response(
        request,
        &SimulateStartResult {
            job_id: job.job_id,
            status: job.status,
        },
    ))
}

fn handle_simulate_status(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: SimulateStatusParams = params(request)?;
    let project = require_project(sessions)?;
    let job = crate::simulation::status_job(&project, &p.job_id)
        .map_err(|error| evidence_fault(&error))?;
    if matches!(
        job.status,
        SimulateJobStatus::Pending | SimulateJobStatus::Running
    ) {
        sessions.queue_simulation_worker(job.job_id.clone());
    }
    Ok(ok_response(
        request,
        &SimulateStatusResult {
            job_id: job.job_id,
            status: job.status,
            trials_done: job.trials_done,
            trials_total: job.trials_total,
            manifest: job.manifest,
            manifest_digest: job.manifest_digest,
        },
    ))
}

fn handle_simulate_cancel(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: SimulateCancelParams = params(request)?;
    let project = require_project(sessions)?;
    let job = crate::simulation::cancel_job(&project, &p.job_id)
        .map_err(|error| evidence_fault(&error))?;
    Ok(ok_response(
        request,
        &SimulateCancelResult {
            job_id: job.job_id,
            status: job.status,
        },
    ))
}

/// The generator bound on deltas drawn per resource per trial for the default
/// `simulate.start` battery.
const SIM_MAX_STEPS: u32 = 6;
/// The generator bound on a single delta's magnitude for the default battery.
const SIM_MAX_MAGNITUDE: u32 = 3;

/// The synthesized default metrics for a `simulate.start` battery: an
/// `any-violation` rate plus a per-resource `<res>-violated` rate.
fn default_metrics(pack: &dli_world::Pack) -> Vec<Metric> {
    let mut metrics = vec![Metric {
        name: "any-violation".to_owned(),
        observable: Observable::AnyViolation,
    }];
    for name in practice_resource_names(pack) {
        metrics.push(Metric {
            name: format!("{name}-violated"),
            observable: Observable::Violated {
                resource: name.clone(),
            },
        });
    }
    metrics
}

/// The synthesized default invariants for a `simulate.start` battery: an
/// `all-laws-hold` sweep plus a per-resource `<res>-never-violated` sweep.
fn default_invariants(pack: &dli_world::Pack) -> Vec<InvariantSpec> {
    let mut invariants = vec![InvariantSpec {
        name: "all-laws-hold".to_owned(),
        predicate: Invariant::AllLawsHold,
    }];
    for name in practice_resource_names(pack) {
        invariants.push(InvariantSpec {
            name: format!("{name}-never-violated"),
            predicate: Invariant::NeverViolated {
                resource: name.clone(),
            },
        });
    }
    invariants
}

/// The declared resource names of the document's first practice, in declaration
/// order — the axis the default metrics and invariants range over.
fn practice_resource_names(pack: &dli_world::Pack) -> Vec<String> {
    pack.documents
        .iter()
        .flat_map(compile_practices)
        .next()
        .map(|practice| {
            practice
                .resources
                .iter()
                .map(|resource| resource.name.text.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn handle_name_check(request: &Request, sessions: &Sessions) -> Result<Response, Fault> {
    let p: NameCheckParams = params(request)?;
    // The op carries no document id, so a named dialect resolves against the
    // selected project's combined language model (or the explicit demo model).
    // An empty dialect keeps the register-free default.
    let dialect = p.dialect.trim();
    let findings = if dialect.is_empty() {
        view::vet_candidate(&LanguageModel::default(), &p.name)
    } else {
        vet_candidate_in_dialect(&sessions.language_model(), &p.name, dialect)
    };
    Ok(ok_response(
        request,
        &NameCheckResult {
            ok: findings.is_empty(),
            findings: findings.iter().map(lang_finding).collect(),
        },
    ))
}

fn handle_attest_missing(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: AttestMissingParams = params(request)?;
    let project = require_project(sessions)?;
    let scope = resolve(sessions, &p.document)?;
    let questions = evidence::missing_attestations(&project, &p.document, &scope.world.model)
        .map_err(|error| evidence_fault(&error))?
        .into_iter()
        .map(|demand| AttestationDemand {
            question: demand.question,
            fingerprint: demand.fingerprint,
            reason: match demand.reason {
                "missing" => AttestationDemandReason::Missing,
                "stale" => AttestationDemandReason::Stale,
                _ => unreachable!("evidence emits a closed reason set"),
            },
        })
        .collect();
    Ok(ok_response(request, &AttestMissingResult { questions }))
}

fn handle_attestation_sweep(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: AttestationSweepParams = params(request)?;
    let project = require_project(sessions)?;
    let scope = resolve(sessions, &p.document)?;
    let records = p
        .records
        .into_iter()
        .map(AttestationRecord::into_stored)
        .collect::<Vec<_>>();
    let appended =
        evidence::append_attestations(&project, &p.document, &records, &scope.world.model)
            .map_err(|error| evidence_fault(&error))?;
    Ok(ok_response(
        request,
        &AttestationSweepResult {
            accepted: appended.accepted,
            batch_digest: appended.batch_digest,
        },
    ))
}

// -- dli.timeline.* ----------------------------------------------------------
//
// Two times meet here and stay apart (`03-time.md`): `show`/`order` read the
// resolved world's chronology (an era spine and events as partially-ordered
// points); `knows`/`episodes`/`phases` read the episode fold (production order —
// the listener's dimension) over the document's season. Each renders the same
// domain lines the CLI's `deloreis timeline` reads print, mapped onto typed
// operation-specific results. Story-time is honestly partial: `order` reports
// unordered pairs as first-class, never a silent tie-break
// (`TemporalVerdict::Unordered`).

fn handle_timeline_show(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: TimelineShowParams = params(request)?;
    let placements = sessions
        .episodes(&p.document)
        .map(|episodes| fold(&episodes).placements().to_vec())
        .unwrap_or_default();
    let scope = resolve(sessions, &p.document)?;
    let world = &scope.world.model.world;
    let points = time_universe(world);
    let (layers, cycle) = match world.chronology.topological_layers(&points) {
        Ok(layers) => (
            layers
                .iter()
                .enumerate()
                .map(|(rank, layer)| TimelineLayer {
                    rank: u32::try_from(rank).unwrap_or(u32::MAX),
                    points: layer
                        .iter()
                        .map(|point| point_name(world, *point))
                        .collect(),
                })
                .collect(),
            Vec::new(),
        ),
        Err(blame) => (
            Vec::new(),
            blame
                .cycle
                .iter()
                .map(|edge| TimelineEdge {
                    before: point_name(world, edge.before),
                    after: point_name(world, edge.after),
                })
                .collect(),
        ),
    };
    let cursor = if let Some(at) = &p.at {
        let cursor = points
            .iter()
            .copied()
            .find(|pt| &point_name(world, *pt) == at)
            .ok_or_else(|| {
                (
                    ErrorCode::SchemaViolation,
                    format!("unknown timeline point '{at}'"),
                )
            })?;
        if cycle.is_empty() {
            let mut before = Vec::new();
            let mut after = Vec::new();
            let mut unordered = Vec::new();
            for &other in &points {
                if other == cursor {
                    continue;
                }
                match world.chronology.precedes(other, cursor) {
                    TemporalVerdict::Before => before.push(point_name(world, other)),
                    TemporalVerdict::After => after.push(point_name(world, other)),
                    TemporalVerdict::Unordered => unordered.push(point_name(world, other)),
                }
            }
            Some(TimelineCursor {
                at: at.clone(),
                before,
                after,
                unordered,
            })
        } else {
            None
        }
    } else {
        None
    };
    Ok(ok_response(
        request,
        &TimelineShowResult {
            layers,
            cycle,
            cursor,
            placements,
        },
    ))
}

fn handle_timeline_order(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: TimelineOrderParams = params(request)?;
    let scope = resolve(sessions, &p.document)?;
    let world = &scope.world.model.world;
    if let Some(blame) = world.chronology.find_cycle(&[]) {
        return Ok(ok_response(
            request,
            &TimelineOrderResult {
                acyclic: false,
                cycle: blame
                    .cycle
                    .iter()
                    .map(|edge| TimelineEdge {
                        before: point_name(world, edge.before),
                        after: point_name(world, edge.after),
                    })
                    .collect(),
                unordered: Vec::new(),
            },
        ));
    }
    let points = time_universe(world);
    let mut unordered = Vec::new();
    for (i, &a) in points.iter().enumerate() {
        for &b in &points[i + 1..] {
            if world.chronology.precedes(a, b) == TemporalVerdict::Unordered {
                unordered.push(TimelinePair {
                    left: point_name(world, a),
                    right: point_name(world, b),
                });
            }
        }
    }
    Ok(ok_response(
        request,
        &TimelineOrderResult {
            acyclic: true,
            cycle: Vec::new(),
            unordered,
        },
    ))
}

fn handle_timeline_knows(request: &Request, sessions: &Sessions) -> Result<Response, Fault> {
    let p: TimelineKnowsParams = params(request)?;
    let episodes = resolve_season(sessions, &p.document)?;
    let folded = fold(&episodes);
    let slot = match &p.at {
        Some(at) => at.parse::<u32>().map_err(|_| {
            (
                ErrorCode::SchemaViolation,
                format!("'{at}' is not a production slot (a whole number)"),
            )
        })?,
        None => max_slot(&episodes),
    };
    let knowers = knower_universe(&episodes)
        .into_iter()
        .map(|knower| KnowledgeRow {
            facts: folded
                .knowledge_at(slot, &knower)
                .into_iter()
                .map(|fact| fact.0)
                .collect(),
            knower,
        })
        .collect();
    Ok(ok_response(
        request,
        &TimelineKnowsResult { at: slot, knowers },
    ))
}

fn handle_timeline_episodes(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: TimelineEpisodesParams = params(request)?;
    let episodes = resolve_season(sessions, &p.document)?;
    let folded = fold(&episodes);
    let mut ordered: Vec<&Episode> = episodes.iter().collect();
    ordered.sort_by_key(|ep| ep.production_order);
    let summaries = ordered
        .into_iter()
        .map(|episode| EpisodeSummary {
            episode: episode.id.clone(),
            production_order: episode.production_order,
            story_anchor: episode.story_anchor.clone(),
        })
        .collect();
    let integrity = order_integrity(&episodes);
    let scope = resolve(sessions, &p.document)?;
    let slack = season_slack(
        &episodes,
        &[],
        &BTreeMap::new(),
        &scope.world.model.world.valuation,
        &scope.world.model.world.theory,
    )
    .slots;
    Ok(ok_response(
        request,
        &TimelineEpisodesResult {
            episodes: summaries,
            loops: folded.open_loops(),
            integrity,
            slack,
        },
    ))
}

fn handle_timeline_phases(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: TimelinePhasesParams = params(request)?;
    let episodes = resolve_season(sessions, &p.document)?;
    let episode = episodes
        .iter()
        .find(|ep| ep.id == p.episode)
        .ok_or_else(|| {
            (
                ErrorCode::SchemaViolation,
                format!("no episode '{}' in the season", p.episode),
            )
        })?;
    let cycles = resolve(sessions, &p.document)?
        .world
        .model
        .world
        .cycles
        .clone();
    let mut anchors = Vec::new();
    let mut findings = Vec::new();
    append_phase_anchors(
        &mut anchors,
        &mut findings,
        "episode",
        &episode.story_anchor.phases,
        &cycles,
    );
    for scene in &episode.scenes {
        if let Some(anchor) = &scene.scene_anchor {
            append_phase_anchors(
                &mut anchors,
                &mut findings,
                &format!("scene:{}", scene.id),
                &anchor.phases,
                &cycles,
            );
        }
    }
    if anchors.is_empty() {
        findings.push(format!("episode {} pins no cyclic phase", p.episode));
    }
    Ok(ok_response(
        request,
        &TimelinePhasesResult {
            episode: p.episode,
            consistent: anchors.iter().all(|anchor| anchor.valid),
            anchors,
            findings,
        },
    ))
}

/// The season for a document, or a typed fault when none is loaded. Episodes are
/// serde payloads the compiled world does not carry, so the session resolves
/// them beside the document pack: from the caller manifest in project mode, or
/// from the neutral fixture registry in explicit demo mode.
fn resolve_season(sessions: &Sessions, document: &str) -> Result<Vec<Episode>, Fault> {
    sessions.episodes(document).ok_or_else(|| {
        (
            ErrorCode::UnknownDoc,
            format!("no episode season is loaded for document '{document}'"),
        )
    })
}

/// The highest production slot a season reaches — the default `knows` cursor (the
/// whole season played).
fn max_slot(episodes: &[Episode]) -> u32 {
    episodes
        .iter()
        .map(|ep| ep.production_order)
        .max()
        .unwrap_or(0)
}

/// The name of a time point in the world's time-point table, or a stable fallback.
fn point_name(world: &World, point: TimePoint) -> String {
    world
        .time_points
        .names()
        .get(point.0 as usize)
        .cloned()
        .unwrap_or_else(|| format!("point#{}", point.0))
}

/// The full universe of interned time points in a compiled world — every era and
/// event, so a point with no `before`-fact still appears (it lands in layer 0).
fn time_universe(world: &World) -> Vec<TimePoint> {
    (0..world.time_points.names().len())
        .filter_map(|index| u32::try_from(index).ok())
        .map(TimePoint)
        .collect()
}

/// Resolve a candidate valuation's interned variables for the wire.
fn named_valuation(valuation: &Valuation, interner: &Interner) -> BTreeMap<String, bool> {
    valuation
        .0
        .iter()
        .map(|(atom, value)| {
            (
                interner
                    .resolve(*atom)
                    .map_or_else(|| format!("var#{}", atom.0), ToOwned::to_owned),
                *value,
            )
        })
        .collect()
}

/// Resolve episode/scene phase pins against the compiled world's cycle table.
fn append_phase_anchors(
    anchors: &mut Vec<PhaseAnchor>,
    findings: &mut Vec<String>,
    owner: &str,
    positions: &[dli_time::cycle::CyclePos],
    cycles: &[dli_time::cycle::Cycle],
) {
    for position in positions {
        let cycle = cycles.iter().find(|cycle| cycle.name == position.cycle);
        let spoke = cycle.and_then(|cycle| {
            cycle
                .in_range(position)
                .then(|| cycle.spoke(position))
                .flatten()
                .map(ToOwned::to_owned)
        });
        let valid = spoke.is_some();
        if !valid {
            findings.push(match cycle {
                Some(cycle) => format!(
                    "{owner} pins {} index {}, outside its {} spokes",
                    position.cycle,
                    position.index,
                    cycle.len()
                ),
                None => format!("{owner} pins undeclared cycle '{}'", position.cycle),
            });
        }
        anchors.push(PhaseAnchor {
            owner: owner.to_owned(),
            position: position.clone(),
            spoke,
            valid,
        });
    }
}

// -- engine → wire mappings --------------------------------------------------

/// Map an engine [`ForkDelta`] onto the wire [`DiffDelta`] without dropping
/// before/after standings, typed conflicts, or deterministic renderings.
fn diff_delta(delta: &ForkDelta) -> DiffDelta {
    DiffDelta {
        newly_certain: delta.newly_certain.clone(),
        newly_possible: delta.newly_possible.clone(),
        newly_dead: delta.newly_dead.clone(),
        grade_changed: delta.grade_changed.clone(),
        conflicts: delta.conflicts.iter().map(diff_conflict).collect(),
    }
}

fn diff_conflict(conflict: &dli_eval::fork::Conflict) -> DiffConflict {
    DiffConflict {
        code: format!("fork.{}", conflict.kind.label().replace(' ', "-")),
        kind: Some(conflict.kind),
        blame: vec![conflict.blame.0.clone(), conflict.blame.1.clone()],
        rendered: conflict.rendered.clone(),
    }
}

/// Map a kernel [`WorldFinding`] onto the wire [`Finding`].
fn world_finding(finding: &WorldFinding) -> Finding {
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
    }
}

fn attestation_check_finding(checked: evidence::CheckedAttestation) -> AttestationCheckFinding {
    let evidence::CheckedAttestation {
        question,
        fingerprint,
        previous,
        state,
    } = checked;
    let key = question.key();
    let previous_answer = previous
        .as_ref()
        .and_then(|record| AttestationAnswer::try_from(record.answer.as_str()).ok());
    match state {
        evidence::AttestationState::Missing => {
            AttestationCheckFinding::Missing(Box::new(AttestationMissingFinding {
                code: AttestationMissingCode::Value,
                severity: AttestationErrorSeverity::Error,
                grade: CheckedProcedureGrade::Checked,
                message: format!(
                    "the question '{key}' has no attestation; the kernel will not guess an answer"
                ),
                question,
                current_fingerprint: fingerprint,
            }))
        }
        evidence::AttestationState::Stale(attestation) => {
            let reason = if AttestationAnswer::try_from(attestation.answer.as_str()).is_err() {
                "uses an answer outside the current yes/no contract"
            } else {
                "was posed against different question data or world content"
            };
            AttestationCheckFinding::Stale(Box::new(AttestationStaleFinding {
                code: AttestationStaleCode::Value,
                severity: AttestationErrorSeverity::Error,
                grade: CheckedProcedureGrade::Checked,
                message: format!(
                    "the stored judgment for question '{key}' {reason}; re-attestation is required"
                ),
                question,
                record_id: attestation.record_id().to_owned(),
                supersedes: attestation.supersedes.clone(),
                previous_answer,
                stored_fingerprint: attestation.fingerprint.clone(),
                current_fingerprint: fingerprint,
            }))
        }
        evidence::AttestationState::Current(attestation) => {
            let answer = AttestationAnswer::try_from(attestation.answer.as_str())
                .expect("evidence classifies only yes/no records as current");
            let record_id = attestation.record_id().to_owned();
            let message = match (&attestation.supersedes, previous_answer) {
                (Some(previous_id), Some(previous)) => format!(
                    "stored judgment '{record_id}' answers '{answer}' to question '{key}', \
                     superseding '{previous_id}' which answered '{previous}'"
                ),
                _ => {
                    format!("stored judgment '{record_id}' answers '{answer}' to question '{key}'")
                }
            };
            if answer == AttestationAnswer::Yes {
                AttestationCheckFinding::Pass(Box::new(AttestationPassFinding {
                    code: AttestationPassCode::Value,
                    grade: AttestedProcedureGrade::Attested,
                    message,
                    question,
                    record_id,
                    supersedes: attestation.supersedes.clone(),
                    previous_answer,
                    stored_fingerprint: attestation.fingerprint.clone(),
                    current_fingerprint: fingerprint,
                }))
            } else {
                AttestationCheckFinding::Fail(Box::new(AttestationFailFinding {
                    code: AttestationFailCode::Value,
                    severity: AttestationErrorSeverity::Error,
                    grade: AttestedProcedureGrade::Attested,
                    message,
                    question,
                    record_id,
                    supersedes: attestation.supersedes.clone(),
                    previous_answer,
                    stored_fingerprint: attestation.fingerprint.clone(),
                    current_fingerprint: fingerprint,
                }))
            }
        }
    }
}

/// Map a language-layer finding onto the wire [`Finding`]. The kernel vetting is
/// deterministic, so it grades `checked`; preserve whether the kernel classified
/// the defect as blocking or advisory.
fn lang_finding(finding: &LangFinding) -> Finding {
    Finding {
        severity: match finding.severity() {
            dli_world::check::Severity::Error => Severity::Error,
            dli_world::check::Severity::Warn => Severity::Warn,
        },
        code: finding.code().to_owned(),
        message: finding.message(),
        grade: Grade::Checked,
        repair: None,
        citations: Vec::new(),
    }
}

/// The decision citations a support formula rests on: the positive literals whose
/// interned name reads as a ledger decision id (`D-nnnn`/`d-nnnn`), deduplicated
/// and sorted. Hypothesis and bundle variables are not citations.
fn citations_of(support: &PosBool, interner: &Interner) -> Vec<String> {
    let mut out = std::collections::BTreeSet::new();
    for monomial in support.monomials() {
        for lit in monomial.lits() {
            if lit.rail == Rail::Pos
                && let Some(name) = interner.resolve(lit.atom)
                && is_decision_id(name)
            {
                out.insert(name.to_owned());
            }
        }
    }
    out.into_iter().collect()
}

/// Whether an interned name reads as a ledger decision id: `D-`/`d-` then digits.
fn is_decision_id(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("D-").or_else(|| name.strip_prefix("d-")) else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

// -- request plumbing --------------------------------------------------------

/// Deserialize the op's params, rejecting unknown fields.
///
/// A missing or mistyped field fails typed deserialization. Structs owned by
/// this crate reject extras directly; the recursive normalized comparison also
/// catches extras inside imported nested types.
fn params<T: DeserializeOwned + Serialize>(request: &Request) -> Result<T, Fault> {
    if !request.params.is_object() {
        return Err((
            ErrorCode::SchemaViolation,
            "params do not satisfy the operation schema: expected an object".to_owned(),
        ));
    }
    let decoded = serde_json::from_value::<T>(request.params.clone());
    // The pre-1.0 half of the field retirement (LS-8). Contract 1.0 makes the
    // retired string forms optional so a caller can send the typed successor
    // alone; on every earlier wire they are still required, and this restores
    // the verbatim `missing field` verdict serde gave when they were.
    //
    // It runs HERE — between the decode and its error, and before
    // `first_unknown_field` — because that is exactly where serde used to
    // answer. Later would let a missing field declared *after* the retired one
    // (a claim's `quote`) or a post-decode unknown-field report answer first,
    // where serde named the retired field; earlier would let it pre-empt an
    // unknown key. The two guards keep it in its place: `preceded_by` yields to
    // a missing field serde would have named sooner, and `is_missing_field_error`
    // yields to every error serde raised while the map was still being visited.
    if let Some(field) = crate::serve::required_retired_field(
        &request.dli,
        request.op,
        &request.params,
        decoded.as_ref().err(),
    ) {
        return Err((
            ErrorCode::SchemaViolation,
            format!(
                "params do not satisfy the operation schema: missing field `{}`",
                crate::serve::field_leaf(field)
            ),
        ));
    }
    let typed: T = decoded.map_err(|e| {
        // A field this build no longer declares fails HERE, on `deny_unknown_fields`
        // — it never reaches `first_unknown_field` below. So the killed-field
        // registry is consulted on both decode failures, or a retired form would
        // be refused anonymously (LS-7). Belt to `dispatch`'s braces since LS-8:
        // a retired field that IS still declared is refused before routing, so
        // what survives to here is the shape 1.0 will have once the field is
        // removed from the struct outright.
        killed_field_fault(request).unwrap_or((
            ErrorCode::SchemaViolation,
            format!("params do not satisfy the operation schema: {e}"),
        ))
    })?;
    let normalized = serde_json::to_value(&typed).map_err(|e| {
        (
            ErrorCode::Internal,
            format!("params could not be normalized: {e}"),
        )
    })?;
    if let Some(field) = first_unknown_field(&request.params, &normalized) {
        return Err(killed_field_fault(request).unwrap_or((
            ErrorCode::SchemaViolation,
            format!("unknown field '{field}' is not defined by the operation schema"),
        )));
    }
    Ok(typed)
}

/// The `killed-form` fault for a retired params field the request still carries,
/// or `None` when nothing registered applies. Consulted only after a params
/// decode has already failed, so it can never refuse a request the contract
/// would otherwise accept — it renames an anonymous schema violation into a
/// refusal that names the successor field, exactly as `refuse_op` does for a
/// retired op.
fn killed_field_fault(request: &Request) -> Option<Fault> {
    crate::serve::killed_field_successor(&request.dli, request.op, &request.params).map(
        |(field, successor)| {
            (
                ErrorCode::KilledForm,
                crate::serve::killed_field_message(&request.dli, field, successor),
            )
        },
    )
}

/// The first input field (dotted path) absent from the normalized
/// re-serialization. Unknown fields are rejected even when their value is
/// `null`; only a field actually declared by the typed schema may use `null`.
/// Shared with the envelope-level check in [`crate::serve`].
pub(crate) fn first_unknown_field(input: &Value, normalized: &Value) -> Option<String> {
    match (input, normalized) {
        (Value::Object(given), Value::Object(kept)) => {
            for (key, value) in given {
                match kept.get(key) {
                    None => return Some(key.clone()),
                    Some(kept_value) => {
                        if let Some(deep) = first_unknown_field(value, kept_value) {
                            return Some(format!("{key}.{deep}"));
                        }
                    }
                }
            }
            None
        }
        (Value::Array(given), Value::Array(kept)) => given
            .iter()
            .zip(kept)
            .find_map(|(a, b)| first_unknown_field(a, b)),
        _ => None,
    }
}

/// Resolve a query goal from the dual-mode pair `(goal_atom, goal)`: the 0.6.3
/// typed successor wins when the caller sent it, and the rendered string is
/// parsed only when it did not. The string form is a registered field-level
/// killed form (`serve::KILLED_FIELDS`) whose retirement lands at
/// contract 1.0; until then both are accepted and the text path is unchanged.
fn goal_atom(typed: Option<&AtomExpr>, text: &str) -> Result<AtomExpr, Fault> {
    match typed {
        Some(atom) => Ok(atom.clone()),
        None => parse_atom(text),
    }
}

/// Parse a rendered goal atom through the shared syntax-backed wire adapter. A
/// build without the front end resolves nothing here and the refusal names
/// `goal_atom` (`query_goal::UNTYPED_GOAL`).
fn parse_atom(text: &str) -> Result<AtomExpr, Fault> {
    crate::query_goal::parse(text).ok_or_else(|| {
        (
            ErrorCode::SchemaViolation,
            format!(
                "'{text}' is not a query atom; {}",
                crate::query_goal::UNTYPED_GOAL
            ),
        )
    })
}

/// Resolve a document to its compiled scope, mapping the typed error to a fault.
fn resolve<'a>(sessions: &'a mut Sessions, document: &str) -> Result<&'a mut Scope, Fault> {
    sessions.scope(document).map_err(|code| {
        let message = match code {
            ErrorCode::UnknownDoc => format!("no document named '{document}' is loaded"),
            _ => format!("document '{document}' could not be compiled"),
        };
        (code, message)
    })
}

/// Resolve a fork handle to its fork id and owning scope.
fn fork_scope<'a>(
    sessions: &'a mut Sessions,
    handle: &str,
) -> Result<(dli_eval::fork::ForkId, &'a mut Scope), Fault> {
    let (document, id) = sessions
        .fork_handle(handle)
        .ok_or_else(|| unknown_fork(handle))?;
    let scope = sessions.scope(&document).map_err(|code| {
        (
            code,
            format!("the scope '{document}' behind fork '{handle}' could not be loaded"),
        )
    })?;
    Ok((id, scope))
}

/// Resolve a fork handle to its valuation overlay — the seam `dli.query --fork`
/// consumes (the theory pass is baked in by [`ForkStore::valuation`]).
fn fork_valuation(
    sessions: &mut Sessions,
    handle: &str,
    document: &str,
) -> Result<Valuation, Fault> {
    fork_id_for_document(sessions, handle, document)?;
    let (id, scope) = fork_scope(sessions, handle)?;
    scope.forks.valuation(id).map_err(fork_fault)
}

fn fork_id_for_document(
    sessions: &Sessions,
    handle: &str,
    document: &str,
) -> Result<dli_eval::fork::ForkId, Fault> {
    let (owner, id) = sessions
        .fork_handle(handle)
        .ok_or_else(|| unknown_fork(handle))?;
    if owner != document {
        return Err((
            ErrorCode::SchemaViolation,
            format!("fork '{handle}' belongs to document '{owner}', not '{document}'"),
        ));
    }
    Ok(id)
}

fn unknown_fork(handle: &str) -> Fault {
    (
        ErrorCode::UnknownDoc,
        format!("no fork named '{handle}' is open"),
    )
}

/// Map a fork-store error onto a typed fault.
fn fork_fault(error: ForkError) -> Fault {
    match error {
        ForkError::UnknownFork(id) => (ErrorCode::UnknownDoc, format!("unknown fork {id:?}")),
        ForkError::UnknownTarget(target) => (
            ErrorCode::SchemaViolation,
            format!("'{target}' is not a supposable variable of this world"),
        ),
        ForkError::Eval(eval) => eval_fault(eval),
    }
}

/// Map an evaluation error onto its typed fault (the frozen [`ErrorCode`] mapping
/// plus the error's domain-spoken message).
fn eval_fault(error: EvalError) -> Fault {
    let message = error.to_string();
    (ErrorCode::from(error), message)
}

fn require_project(sessions: &Sessions) -> Result<std::sync::Arc<crate::project::Project>, Fault> {
    sessions.project_handle().ok_or_else(|| {
        (
            ErrorCode::ProjectRequired,
            "durable evidence operations require `deloreis serve --project <path>`; demo mode has no evidence owner"
                .to_owned(),
        )
    })
}

fn evidence_fault(error: &EvidenceError) -> Fault {
    let code = match error {
        EvidenceError::NotFound(_) => ErrorCode::UnknownJob,
        EvidenceError::Conflict(_) => ErrorCode::Conflict,
        EvidenceError::Corrupt(_) => ErrorCode::EvidenceCorrupt,
        EvidenceError::Invalid(_) => ErrorCode::SchemaViolation,
        EvidenceError::Io { .. } => ErrorCode::Internal,
    };
    (code, error.to_string())
}

/// The wire op names this build implements at the negotiated contract version.
/// Ops added in `0.6.1` (the branch/merge surface) are omitted for a `0.5.1` or
/// `0.6.0` handshake, so an older negotiation's capability list — and the golden
/// fixture recording it — stays byte-identical. Every version at or beyond
/// `0.6.1` (through the current `1.0.0`) advertises them, so the capability is
/// gated on the version, not on being the newest build.
fn capabilities(dli: &str) -> Vec<String> {
    let has_branch_merge = !matches!(dli, "0.5.1" | "0.6.0");
    // The corpus ops arrived at `1.2.0`. Gating on the negotiated version — not
    // on being the newest build — is what keeps an older caller's capability
    // list, and the golden fixture recording it, byte-identical (audit C4).
    let has_corpus = matches!(dli, "1.2.0");
    Op::ALL
        .iter()
        .copied()
        .filter(|op| has_branch_merge || !op.added_in_0_6_1())
        .filter(|op| has_corpus || !op.added_in_1_2_0())
        .map(wire_name)
        .collect()
}

/// The op's wire identifier (its serde encoding).
pub(crate) fn wire_name(op: Op) -> String {
    serde_json::to_value(op)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

// -- response builders -------------------------------------------------------

/// A success response echoing the request's protocol tag and id.
fn ok_response(request: &Request, result: &impl Serialize) -> Response {
    match serde_json::to_value(result) {
        Ok(value) => Response {
            dli: request.dli.clone(),
            id: request.id.clone(),
            ok: true,
            result: Some(value),
            error: None,
        },
        Err(e) => error_response(
            request,
            ErrorCode::Internal,
            format!("result could not be serialized: {e}"),
        ),
    }
}

/// A typed error response echoing the request's protocol tag and id.
fn error_response(request: &Request, code: ErrorCode, message: impl Into<String>) -> Response {
    Response {
        dli: request.dli.clone(),
        id: request.id.clone(),
        ok: false,
        result: None,
        error: Some(ErrorBody {
            code,
            message: message.into(),
            data: None,
        }),
    }
}

// -- dli.corpus.* (DM-3, contract 1.2.0) -------------------------------------

/// The lorestore a corpus op acts on: the document's **origin** store.
///
/// Only a store-backed document has one. A `.dli`-sourced document is refused
/// by name rather than given an empty store to import into — the manifest is
/// where a document's source form is chosen, and a corpus op is not a way to
/// change it behind the manifest's back.
fn document_store(sessions: &Sessions, document: &str) -> Result<dli_lorestore::Lorestore, Fault> {
    let project = require_project(sessions)?;
    let entry = project
        .documents()
        .find(|(id, _)| *id == document)
        .map(|(_, doc)| doc)
        .ok_or((
            ErrorCode::UnknownDoc,
            format!("unknown document `{document}`"),
        ))?;
    let origin = entry.store_origin.as_ref().ok_or((
        ErrorCode::SchemaViolation,
        format!(
            "document `{document}` is `.dli`-sourced, so it has no store to exchange a corpus \
             through; give it a `store` in the manifest (DL-0055)"
        ),
    ))?;
    dli_lorestore::Lorestore::open(origin.clone())
        .map_err(|error| (ErrorCode::Internal, error.to_string()))
}

fn handle_corpus_import(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: CorpusImportParams = params(request)?;
    let store = document_store(sessions, &p.document)?;

    let head = store
        .branch_head(dli_lorestore::MAIN_REF)
        .map_err(|e| (ErrorCode::Internal, e.to_string()))?;
    if matches!(p.mode, CorpusImportMode::Init) && head.is_some() {
        return Err((
            ErrorCode::SchemaViolation,
            format!(
                "document `{}` already holds a corpus; `init` seeds an empty store — use \
                 `append` to extend this one",
                p.document
            ),
        ));
    }

    let root = match (&p.objects, &p.dli_documents) {
        (Some(_), Some(_)) => {
            return Err((
                ErrorCode::SchemaViolation,
                "an import carries `objects` or `dli_documents`, never both".to_owned(),
            ));
        }
        (None, None) => {
            return Err((
                ErrorCode::SchemaViolation,
                "an import needs `objects` (any build) or `dli_documents` (syntax builds)"
                    .to_owned(),
            ));
        }
        (Some(objects), None) => {
            let interner = p.interner.as_ref().ok_or((
                ErrorCode::SchemaViolation,
                "an `objects` import needs the exporting store's `interner`; a stored fact \
                 records its support as `±<index>` into that table, so importing the objects \
                 without it would resolve them against a different table and rewrite their \
                 provenance (DL-0056)"
                    .to_owned(),
            ))?;
            import_objects(&store, objects, interner, &p.decisions, &p.document)?
        }
        (None, Some(documents)) => import_dli_documents(&store, documents, &p.document)?,
    };

    let count = store
        .fact_hashes_from(&root)
        .map_err(|e| (ErrorCode::Internal, e.to_string()))?
        .len();
    Ok(ok_response(
        request,
        &CorpusImportResult {
            root_hash: root.as_str().to_owned(),
            imported_count: count,
            findings: Vec::new(),
        },
    ))
}

/// Import the build-independent form: write each object, then commit them
/// against the **caller's** interner.
///
/// The interner is the caller's, not one reconstructed here, and that is the
/// whole point: a stored fact's support polynomial is `±<index>` into the
/// exporting store's name table. Rebuilding a table from the objects' own atom
/// names produces a different index space, so every support literal silently
/// resolves to the wrong variable — a `decided d-1` fact reads back as
/// `proposed assumes __decl`. Carrying the table keeps the exchange lossless
/// (DL-0056).
fn import_objects(
    store: &dli_lorestore::Lorestore,
    objects: &[serde_json::Value],
    interner: &[String],
    decisions: &[String],
    document: &str,
) -> Result<dli_lorestore::ContentHash, Fault> {
    // Appending into a populated store may only extend the table it already
    // resolves against. The store enforces this too, but as an `internal`
    // error; caught here it is a caller-facing refusal that names the cause.
    let head_interner = store
        .head_interner()
        .map_err(|e| (ErrorCode::Internal, e.to_string()))?;
    if !interner.starts_with(&head_interner) {
        return Err((
            ErrorCode::SchemaViolation,
            format!(
                "the supplied `interner` is not an extension of the one this store already \
                 resolves against ({} name(s)); every earlier fact's support indexes into that \
                 table, so replacing it would rewrite their provenance — export the store, \
                 extend that interner, and import the whole corpus",
                head_interner.len()
            ),
        ));
    }

    // The same guard on the other carried table. Standing is read against the
    // *tip* commit's decision set, so a commit that lands without a decision the
    // store already knew demotes every fact resting on it — `decided d-1`
    // reconstructs as `proposed`, and nothing the caller sent said so. Note this
    // catches omission as well as regression: `decisions` defaults to empty on
    // the wire, so "I forgot the field" and "I meant to drop them all" arrive
    // identically and neither may proceed silently.
    let head_decisions = store
        .head_decisions()
        .map_err(|e| (ErrorCode::Internal, e.to_string()))?;
    if let Some(dropped) = head_decisions
        .iter()
        .find(|known| !decisions.contains(*known))
    {
        return Err((
            ErrorCode::SchemaViolation,
            format!(
                "the supplied `decisions` drops `{dropped}`, which this store already knows; \
                 standing is read against the newest commit's decision set, so committing \
                 without it would demote every fact resting on that decision from `decided` to \
                 `proposed` — send the store's decisions plus any new ones"
            ),
        ));
    }

    let mut hashes = Vec::with_capacity(objects.len());
    for (index, value) in objects.iter().enumerate() {
        let object: dli_lorestore::FactObject =
            serde_json::from_value(value.clone()).map_err(|error| {
                (
                    ErrorCode::SchemaViolation,
                    format!("object {index} is not a stored fact object: {error}"),
                )
            })?;
        hashes.push(
            store
                .write_object(&object)
                .map_err(|e| (ErrorCode::Internal, e.to_string()))?,
        );
    }
    store
        .commit(
            interner.to_vec(),
            hashes,
            Vec::new(),
            Vec::new(),
            decisions.to_vec(),
            document.to_owned(),
        )
        .map_err(|e| (ErrorCode::Internal, e.to_string()))
}

/// Import `.dli` text. The front end is the one thing a default build does not
/// have, so this arm refuses by name there rather than pretending.
#[cfg(feature = "syntax")]
fn import_dli_documents(
    store: &dli_lorestore::Lorestore,
    documents: &[CorpusDocument],
    document: &str,
) -> Result<dli_lorestore::ContentHash, Fault> {
    let mut parsed = Vec::with_capacity(documents.len());
    for entry in documents {
        let (ast, diagnostics) = dli_syntax::parse_document(&entry.text);
        if let Some(diagnostic) = diagnostics.iter().find(|d| d.class.is_error()) {
            return Err((
                ErrorCode::SchemaViolation,
                format!("`{}` does not parse: {}", entry.id, diagnostic.message),
            ));
        }
        parsed.push(ast);
    }
    let pack = dli_world::Pack {
        name: document.to_owned(),
        documents: parsed,
    };
    // The same structural refusals the `objects` arm catches by hand reach this
    // arm through the store, which raises them as `internal`. A caller whose
    // `.dli` append does not extend what the store already holds made a caller's
    // mistake, and must read it as one (DL-0056 rulings 3 and 5).
    store.import_pack(&pack).map_err(|error| match error {
        dli_lorestore::StoreError::InternerRegression { parent_len, .. } => (
            ErrorCode::SchemaViolation,
            format!(
                "these `dli_documents` compile to a name table that does not extend the one this \
                 store already resolves against ({parent_len} name(s)); every earlier fact's \
                 support indexes into that table, so committing them would rewrite its \
                 provenance — send the whole corpus, not just the new documents"
            ),
        ),
        dli_lorestore::StoreError::ForeignBase {
            store_pack,
            importing,
        } => (
            ErrorCode::SchemaViolation,
            format!(
                "this store holds the `{store_pack}` lineage and these documents are `{importing}`; \
                 a store carries one pack lineage, so importing a foreign base would replace the \
                 corpus rather than extend it"
            ),
        ),
        other => (ErrorCode::Internal, other.to_string()),
    })
}

#[cfg(not(feature = "syntax"))]
fn import_dli_documents(
    _store: &dli_lorestore::Lorestore,
    _documents: &[CorpusDocument],
    _document: &str,
) -> Result<dli_lorestore::ContentHash, Fault> {
    Err((
        ErrorCode::NotWired,
        "this build links no `.dli` front end, so it cannot import `dli_documents`; send the \
         corpus as `objects`, or run the one-shot import on a `syntax`-enabled build"
            .to_owned(),
    ))
}

fn handle_corpus_export(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: CorpusExportParams = params(request)?;
    let store = document_store(sessions, &p.document)?;

    let head = match &p.at {
        Some(at) => dli_lorestore::ContentHash::parse(at).ok_or((
            ErrorCode::SchemaViolation,
            format!("`at` is `{at}`, which is not a `sha256:<hex>` store commit hash"),
        ))?,
        None => store
            .branch_head(dli_lorestore::MAIN_REF)
            .map_err(|e| (ErrorCode::Internal, e.to_string()))?
            .ok_or((
                ErrorCode::UnknownDoc,
                format!("document `{}` has no commit to export", p.document),
            ))?,
    };

    // The folded live facts at that commit — the state, not the commit log.
    let hashes = store
        .fact_hashes_from(&head)
        .map_err(|e| (ErrorCode::Internal, e.to_string()))?;
    let mut objects = Vec::with_capacity(hashes.len());
    for hash in &hashes {
        // A hash the commit references but the store cannot produce is
        // corruption, not an empty result — say so rather than skipping it.
        let object: dli_lorestore::FactObject = store
            .read_object(hash)
            .map_err(|e| (ErrorCode::EvidenceCorrupt, e.to_string()))?
            .ok_or((
                ErrorCode::EvidenceCorrupt,
                format!("commit references object {hash}, which the store does not hold"),
            ))?;
        objects
            .push(serde_json::to_value(&object).map_err(|e| (ErrorCode::Internal, e.to_string()))?);
    }

    let rendered = match p.format {
        CorpusExportFormat::Objects => Vec::new(),
        CorpusExportFormat::Rendered => render_export(&store, &head)?,
    };

    // The name table those objects' support polynomials index into. Exported
    // beside them because the objects alone are not the corpus (DL-0056).
    let interner = store
        .interner_at(&head)
        .map_err(|e| (ErrorCode::Internal, e.to_string()))?;
    let decisions = store
        .decisions_at(&head)
        .map_err(|e| (ErrorCode::Internal, e.to_string()))?;

    Ok(ok_response(
        request,
        &CorpusExportResult {
            root_hash: head.as_str().to_owned(),
            objects,
            interner,
            decisions,
            rendered,
        },
    ))
}

/// Render an export as `.dli`. Asking for a rendering from a build that links no
/// renderer is a **loud refusal** naming the ways forward — never a silent empty
/// array, which would read as "this corpus is empty" (DL-0056).
#[cfg(feature = "syntax")]
fn render_export(
    store: &dli_lorestore::Lorestore,
    head: &dli_lorestore::ContentHash,
) -> Result<Vec<String>, Fault> {
    let pack = store
        .reconstruct_at(head)
        .map_err(|e| (ErrorCode::Internal, e.to_string()))?;
    Ok(pack
        .documents
        .iter()
        .map(dli_syntax::format_document)
        .collect())
}

#[cfg(not(feature = "syntax"))]
fn render_export(
    _store: &dli_lorestore::Lorestore,
    _head: &dli_lorestore::ContentHash,
) -> Result<Vec<String>, Fault> {
    Err((
        ErrorCode::NotWired,
        "`format: rendered` needs a linked renderer and this build has none; export `objects` \
         (the canonical form), or run the export on a `syntax`-enabled build"
            .to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use dli_ast::span::Span;
    use dli_world::language::LangFinding;
    use dli_world::payload::PayloadFinding;

    use super::*;

    #[test]
    fn document_ok_ignores_warnings_but_not_errors() {
        let world_warn = CheckFinding::World(Finding {
            severity: Severity::Warn,
            code: "entity.unknown".to_owned(),
            message: "advisory".to_owned(),
            grade: Grade::Checked,
            repair: None,
            citations: Vec::new(),
        });
        let payload_warn = CheckFinding::Payload(PayloadCheckFinding {
            payload_digest: format!("sha256:{}", "0".repeat(64)),
            episode: "e01".to_owned(),
            source_file: "draft.md".to_owned(),
            finding: PayloadFinding {
                code: "fact.contradiction".to_owned(),
                severity: PayloadSeverity::Warn,
                grade: Grade::Checked,
                message: "advisory".to_owned(),
                line: None,
                quote: None,
                principle: None,
                citations: Vec::new(),
            },
        });
        assert!(check_finding_passes(&world_warn));
        assert!(check_finding_passes(&payload_warn));

        let world_error = CheckFinding::World(Finding {
            severity: Severity::Error,
            code: "fact.contradiction".to_owned(),
            message: "blocking".to_owned(),
            grade: Grade::Checked,
            repair: None,
            citations: Vec::new(),
        });
        assert!(!check_finding_passes(&world_error));
    }

    #[test]
    fn document_grade_is_the_weakest_consumed_procedure() {
        assert_eq!(complete_check_grade([Grade::Checked]), Grade::Checked);
        assert_eq!(
            complete_check_grade([Grade::Checked, Grade::Tested, Grade::Attested]),
            Grade::Attested
        );
    }

    #[test]
    fn language_findings_preserve_kernel_severity() {
        let error = LangFinding::RegisterFit {
            name: "candidate".to_owned(),
            register: "missing".to_owned(),
            reason: "unknown dialect".to_owned(),
            span: Span::synthetic(),
        };
        assert_eq!(lang_finding(&error).severity, Severity::Error);

        let warn = LangFinding::SayabilityCap {
            name: "candidate".to_owned(),
            syllables: 9,
            cap: 6,
            span: Span::synthetic(),
        };
        assert_eq!(lang_finding(&warn).severity, Severity::Warn);
    }
}
