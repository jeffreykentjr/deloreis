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

use std::collections::{BTreeMap, BTreeSet};

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

use dli_syntax::parse_document;
use dli_time::chronology::{TemporalVerdict, TimePoint};

use dli_export::profile::ExportTarget;
use dli_export::{ExportOpts, ExportProfile, export_wire};
use dli_ledger::ledger_check::check_proof;
use dli_ledger::practice::{PracticeDef, compile_practices};
use dli_sim::{Battery, GenBounds, Invariant, InvariantSpec, Metric, Observable};

use dli_world::check;
use dli_world::check::{Severity as PayloadSeverity, Severity as WorldSeverity, WorldFinding};
use dli_world::episode::{Episode, Knower, fold};
use dli_world::episode_order::{order_integrity, season_slack};
use dli_world::language::{LangFinding, LanguageModel, vet_candidate_in_dialect, view};
use dli_world::payload::{BatteryContext, check_payload};
use dli_world::scene_projection::project_scene;

use crate::CONTRACT_VERSION;
use crate::envelope::{ErrorBody, ErrorCode, Request, Response};
use crate::evidence::{self, EvidenceError};
use crate::finding::{Finding, Severity};
use crate::ops::Op;
use crate::session::{Scope, Sessions, SimJob};

/// A handler failure: a typed wire code and a domain-spoken message.
type Fault = (ErrorCode, String);

/// Route a `dli.*` request to its handler and build the response envelope. The
/// serve loop routes `author.*` ops elsewhere (the Door T seam); a stray
/// `author.*` op reaching here is an internal routing bug, reported as such.
#[must_use]
pub fn handle(request: &Request, sessions: &mut Sessions) -> Response {
    if !is_implemented(request.op) {
        if let Err((code, message)) = validate_gated_params(request) {
            return error_response(request, code, message);
        }
        return error_response(
            request,
            ErrorCode::NotWired,
            format!(
                "{} is defined by contract {CONTRACT_VERSION} but this build does not yet \
                 implement its promised semantics",
                wire_name(request.op)
            ),
        );
    }
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
        | Op::AuthorListWorkspaces => Err((
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
    Ok(ok_response(
        request,
        &VersionResult {
            contract: "dli-worker".to_owned(),
            contract_version: CONTRACT_VERSION.to_owned(),
            server: concat!("deloreis ", env!("CARGO_PKG_VERSION")).to_owned(),
            capabilities: capabilities(),
        },
    ))
}

fn handle_check(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: CheckParams = params(request)?;
    let scope = resolve(sessions, &p.document)?;
    let findings = check::run(&scope.world);
    Ok(ok_response(
        request,
        &CheckResult {
            ok: findings.is_empty(),
            findings: findings.iter().map(world_finding).collect(),
        },
    ))
}

fn handle_query(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: QueryParams = params(request)?;
    let goal = parse_atom(&p.goal)?;
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
            }
        })
        .collect();
    let result = QueryResult { rows };
    Ok(ok_response(request, &result))
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
    // Parse the declaration lines through the syntax layer and lower them to
    // ground facts BEFORE touching session state, so a malformed decl is a schema
    // error caught up front, never a half-applied fork.
    let additions = additions_from_declarations(&p.declarations)?;
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

/// Parse a run of `.dli` declaration lines and lower them to fork-local
/// additions. Each lowered ground fact or slot becomes an [`Addition`] asserted by fiat within
/// the fork (`dli.fork.add`, *propose a fact* — the same fork-local assertion the
/// CLI's `fork add` makes): the fork is a what-if, so a proposed line is supposed
/// true inside it and its consequences are read off the diff. A parse error or an
/// unground fact is a clean [`ErrorCode::SchemaViolation`], never a panic.
/// Declarations that are not facts or slots are refused instead of being
/// accepted and silently dropped.
fn additions_from_declarations(declarations: &[String]) -> Result<Vec<Addition>, Fault> {
    let source = declarations.join("\n");
    let (document, diags) = parse_document(&source);
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
    }
    let findings = check_payload(&scope.world, &p.payload, &context);
    let accepted = findings
        .iter()
        .all(|finding| finding.severity != PayloadSeverity::Error);
    let payload_digest = if accepted {
        Some(
            evidence::submit_payload(&project, &p.document, &p.payload)
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
    let proof: &ProofDecl = pack
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .find_map(|decl| match decl {
            Decl::Proof(proof) => Some(proof),
            _ => None,
        })
        .ok_or_else(|| {
            (
                ErrorCode::SchemaViolation,
                format!(
                    "document '{}' declares no proof block to replay",
                    p.document
                ),
            )
        })?;
    let practices: Vec<PracticeDef> = pack.documents.iter().flat_map(compile_practices).collect();
    let practice_name = header_value(&proof.head, "in").ok_or_else(|| {
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
    let speaker = header_value(&proof.head, "speaker");
    let scope = resolve(sessions, &p.document)?;
    let scene = project_scene(&scope.world, speaker.as_deref());
    let report = check_proof(practice, proof, &scene);
    Ok(ok_response(request, &CheckProofResult { report }))
}

fn handle_export_run(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ExportRunParams = params(request)?;
    match p.profile {
        // The context pack renders a compiled lore world's certain layer.
        ExportProfile::ContextPack => {
            let scope = resolve(sessions, &p.document)?;
            let outcome = export_wire(
                ExportProfile::ContextPack,
                &scope.world,
                &ExportOpts::default(),
            )
            .map_err(|err| {
                (
                    ErrorCode::Internal,
                    format!("the context-pack export failed: {err}"),
                )
            })?;
            Ok(ok_response(
                request,
                &ExportRunResult {
                    emission: match outcome.artifact {
                        dli_export::ExportArtifact::ContextPack(pack) => {
                            ExportEmission::ContextPack(pack)
                        }
                        dli_export::ExportArtifact::Articy(_) => {
                            return Err((
                                ErrorCode::Internal,
                                "the context-pack profile emitted an articy artifact".to_owned(),
                            ));
                        }
                    },
                    manifest: outcome.manifest,
                },
            ))
        }
        // Cards / Ludii render a card ruleset; the document must name a game, not a
        // lore corpus (which carries no ruleset). A lore doc-id is refused, never
        // half-rendered.
        ExportProfile::Cards | ExportProfile::LudiiGdl => {
            let (ruleset, registry) = sessions.ruleset(&p.document).ok_or_else(|| {
                if sessions.pack(&p.document).is_some() {
                    (
                        ErrorCode::SchemaViolation,
                        format!(
                            "the requested profile renders a card ruleset; document '{}' names no \
                             card game (a lore corpus carries no ruleset)",
                            p.document
                        ),
                    )
                } else {
                    (
                        ErrorCode::UnknownDoc,
                        format!("no document or game named '{}' is loaded", p.document),
                    )
                }
            })?;
            let target = if matches!(p.profile, ExportProfile::Cards) {
                ExportTarget::Cards
            } else {
                ExportTarget::LudiiGdl
            };
            let export = dli_game::export::run(&registry, target, &ruleset, &ExportOpts::default())
                .map_err(|err| (ErrorCode::Internal, format!("the export failed: {err}")))?;
            Ok(ok_response(
                request,
                &ExportRunResult {
                    emission: match export.artifact {
                        dli_game::export::GameArtifact::Cards(deck) => ExportEmission::Cards(deck),
                        dli_game::export::GameArtifact::LudiiGdl(gdl) => {
                            ExportEmission::LudiiGdl(gdl)
                        }
                    },
                    manifest: export.manifest,
                },
            ))
        }
    }
}

fn handle_simulate_start(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: SimulateStartParams = params(request)?;
    // Source the first declared practice from the document (the same corpus the
    // scope compiles); `trials` is the trial count.
    let pack = sessions.pack(&p.document).ok_or_else(|| {
        (
            ErrorCode::UnknownDoc,
            format!("no document named '{}' is loaded", p.document),
        )
    })?;
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
    let battery = Battery {
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
    };
    let manifest = battery.run();
    let job_id = sessions.register_job(SimJob {
        status: SimulateJobStatus::Complete,
        trials_done: p.trials,
        trials_total: p.trials,
        manifest,
    });
    Ok(ok_response(
        request,
        &SimulateStartResult {
            job_id,
            status: SimulateJobStatus::Complete,
        },
    ))
}

fn handle_simulate_status(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: SimulateStatusParams = params(request)?;
    let job = sessions.job(&p.job_id).ok_or_else(|| {
        (
            ErrorCode::UnknownDoc,
            format!("no simulation job named '{}' is running", p.job_id),
        )
    })?;
    Ok(ok_response(
        request,
        &SimulateStatusResult {
            job_id: p.job_id,
            status: job.status,
            trials_done: job.trials_done,
            trials_total: job.trials_total,
            manifest: Some(job.manifest.clone()),
            manifest_digest: Some(job.manifest.digest()),
        },
    ))
}

fn handle_simulate_cancel(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: SimulateCancelParams = params(request)?;
    if sessions.cancel_job(&p.job_id) {
        Ok(ok_response(
            request,
            &SimulateCancelResult {
                job_id: p.job_id,
                status: SimulateJobStatus::Cancelled,
            },
        ))
    } else {
        Err((
            ErrorCode::UnknownDoc,
            format!("no simulation job named '{}' is running", p.job_id),
        ))
    }
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

/// Read the value following a header keyword (`in <practice>`, `speaker <who>`)
/// from a proof declaration's flat header token list.
fn header_value(head: &[String], key: &str) -> Option<String> {
    head.iter()
        .position(|token| token == key)
        .and_then(|i| head.get(i + 1))
        .cloned()
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
    let appended =
        evidence::append_attestations(&project, &p.document, &p.records, &scope.world.model)
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
    let mut knowers = BTreeSet::from([Knower::Listener]);
    knowers.extend(
        folded
            .knowledge_records()
            .iter()
            .map(|record| record.knower.clone()),
    );
    let knowers = knowers
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

/// Map a language-layer finding onto the wire [`Finding`]. The kernel vetting is
/// deterministic, so it grades `checked`; a name defect warns rather than blocks.
fn lang_finding(finding: &LangFinding) -> Finding {
    Finding {
        severity: Severity::Warn,
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
    let typed: T = serde_json::from_value(request.params.clone()).map_err(|e| {
        (
            ErrorCode::SchemaViolation,
            format!("params do not satisfy the operation schema: {e}"),
        )
    })?;
    let normalized = serde_json::to_value(&typed).map_err(|e| {
        (
            ErrorCode::Internal,
            format!("params could not be normalized: {e}"),
        )
    })?;
    if let Some(field) = first_unknown_field(&request.params, &normalized) {
        return Err((
            ErrorCode::SchemaViolation,
            format!("unknown field '{field}' is not defined by the operation schema"),
        ));
    }
    Ok(typed)
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

/// Parse a rendered goal atom through the shared syntax-backed wire adapter.
fn parse_atom(text: &str) -> Result<AtomExpr, Fault> {
    crate::query_goal::parse(text).ok_or_else(|| {
        (
            ErrorCode::SchemaViolation,
            format!(
                "'{text}' is not a query atom; write a rendered goal like \
                 serves(lancelot, ?who)"
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
        EvidenceError::Conflict(_) => ErrorCode::Conflict,
        EvidenceError::Corrupt(_) => ErrorCode::EvidenceCorrupt,
        EvidenceError::Invalid(_) => ErrorCode::SchemaViolation,
        EvidenceError::Io { .. } => ErrorCode::Internal,
    };
    (code, error.to_string())
}

/// Whether this build satisfies the operation's documented semantics.
///
/// The gated evidence and simulation operations retain typed schemas and
/// fixtures, but are not advertised until their persistence and restart
/// guarantees land.
fn is_implemented(op: Op) -> bool {
    !matches!(
        op,
        Op::SimulateStart | Op::SimulateStatus | Op::SimulateCancel
    )
}

/// Validate a gated operation's request shape without executing its unfinished
/// semantics. Capability gating must not weaken the contract's field checks.
fn validate_gated_params(request: &Request) -> Result<(), Fault> {
    match request.op {
        Op::CheckDraft => params::<CheckDraftParams>(request).map(drop),
        Op::SimulateStart => params::<SimulateStartParams>(request).map(drop),
        Op::SimulateStatus => params::<SimulateStatusParams>(request).map(drop),
        Op::SimulateCancel => params::<SimulateCancelParams>(request).map(drop),
        Op::AttestMissing => params::<AttestMissingParams>(request).map(drop),
        Op::AttestationSweep => params::<AttestationSweepParams>(request).map(drop),
        Op::PayloadSubmit => params::<PayloadSubmitParams>(request).map(drop),
        _ => Ok(()),
    }
}

/// The wire op names this build currently implements.
fn capabilities() -> Vec<String> {
    Op::ALL
        .iter()
        .copied()
        .filter(|op| is_implemented(*op))
        .map(wire_name)
        .collect()
}

/// The op's wire identifier (its serde encoding).
fn wire_name(op: Op) -> String {
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
