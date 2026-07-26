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
//! All 46 operations (27 `dli.*` + 19 `author.*`) are fully dispatched through
//! [`handle`]; [`ErrorCode::NotWired`] is never constructed. The capability-gated
//! regime described in earlier iterations of this comment is historical — the
//! DL-0021 and ec92efd wiring waves completed the dispatch table.

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
use dli_eval::fork::{Addition, AtomChange, ForkDelta, ForkError, KeepStamp, Visibility};
use dli_eval::query::{Query, QueryOpts};
use dli_eval::world::{GroundAtom, World};

use dli_ast::decl::{Decl, ProofDecl};
use dli_ast::ident::Ident;
use dli_ast::term::{AtomExpr, Term};

use dli_syntax::parse_document;
use dli_time::chronology::{TemporalVerdict, TimePoint};

use dli_export::profile::ExportTarget;
use dli_export::{ExportOpts, ExportProfile, export_wire};
use dli_ledger::ledger_check::check_proof;
use dli_ledger::practice::{PracticeDef, compile_practices};
use dli_sim::{Battery, GenBounds, Invariant, InvariantSpec, Metric, Observable};

use dli_world::check::{Severity as WorldSeverity, WorldFinding};
use dli_world::draft::DraftFinding;
use dli_world::episode::{Episode, Knower, fold};
use dli_world::language::{LangFinding, LanguageModel, vet_candidate_in_dialect, view};
use dli_world::model::WorldModel;
use dli_world::scene_projection::project_scene;
use dli_world::{check, check_draft, sweep};

use crate::CONTRACT_VERSION;
use crate::envelope::{ErrorBody, ErrorCode, Request, Response};
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
        Op::AttestationSweep => handle_attestation_sweep(request),
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
    let result = match answer.rows.first() {
        Some(row) => QueryResult {
            support: row.verdict.support.clone(),
            grade: row.verdict.grade,
            citations: citations_of(&row.verdict.support, interner),
        },
        // A well-formed query that matched nothing is not an error (§2.1): it is a
        // claim with no support, graded by fiat's empty procedure.
        None => QueryResult {
            support: PosBool::FALSE,
            grade: Grade::Asserted,
            citations: Vec::new(),
        },
    };
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
    Ok(ok_response(request, &ForkNewResult { fork: handle }))
}

fn handle_fork_list(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: ForkListParams = params(request)?;
    resolve(sessions, &p.document)?;
    Ok(ok_response(
        request,
        &ForkListResult {
            forks: sessions.forks_of(&p.document),
        },
    ))
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
    for target in &p.assume {
        scope.forks.suppose(id, target, true).map_err(fork_fault)?;
    }
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
            conflicts: pre.conflicts.iter().map(|c| c.rendered.clone()).collect(),
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
/// additions. Each lowered fact becomes an [`Addition`] asserted by fiat within
/// the fork (`dli.fork.add`, *propose a fact* — the same fork-local assertion the
/// CLI's `fork add` makes): the fork is a what-if, so a proposed line is supposed
/// true inside it and its consequences are read off the diff. A parse error or an
/// unground fact is a clean [`ErrorCode::SchemaViolation`], never a panic. A
/// non-fact declaration (an entity, an era edge) carries no ground fact and adds
/// nothing — the honest empty delta.
fn additions_from_declarations(declarations: &[String]) -> Result<Vec<Addition>, Fault> {
    let source = declarations.join("\n");
    let (document, diags) = parse_document(&source);
    if let Some(diag) = diags.iter().find(|diag| diag.class.is_error()) {
        return Err((
            ErrorCode::SchemaViolation,
            format!("the declaration is not well-formed .dli: {}", diag.message),
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
    let scope = resolve(sessions, &p.payload.meta.scope)?;
    Ok(ok_response(
        request,
        &CheckDraftResult {
            findings: check_draft(&scope.world, &p.payload),
        },
    ))
}

fn handle_payload_submit(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: PayloadSubmitParams = params(request)?;
    let scope = resolve(sessions, &p.payload.meta.scope)?;
    let defects = check_draft(&scope.world, &p.payload);
    Ok(ok_response(
        request,
        &PayloadSubmitResult {
            accepted: defects.is_empty(),
            findings: defects.iter().map(draft_finding).collect(),
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
                    manifest: export.manifest,
                },
            ))
        }
    }
}

fn handle_simulate_start(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: SimulateStartParams = params(request)?;
    // Source the first declared practice from the document (the same corpus the
    // scope compiles); `turns` is the trial count.
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
        trials: u64::from(p.turns),
        metrics: default_metrics(&pack),
        invariants: default_invariants(&pack),
        affine_caps: std::collections::BTreeMap::new(),
    };
    let manifest = battery.run();
    let trace = trace_lines(&battery);
    let manifest_json = serde_json::to_value(&manifest).map_err(|err| {
        (
            ErrorCode::Internal,
            format!("manifest could not be serialized: {err}"),
        )
    })?;
    let job_id = sessions.register_job(SimJob {
        status: "complete".to_owned(),
        trace,
        manifest: manifest_json,
    });
    Ok(ok_response(
        request,
        &SimulateStartResult {
            job_id,
            status: "complete".to_owned(),
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
            status: job.status.clone(),
            trace: job.trace.clone(),
            manifest: Some(job.manifest.clone()),
        },
    ))
}

fn handle_simulate_cancel(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: SimulateCancelParams = params(request)?;
    if sessions.cancel_job(&p.job_id) {
        Ok(ok_response(
            request,
            &SimulateCancelResult {
                status: "cancelled".to_owned(),
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

/// One rendered line per simulated turn — whether that trial's casting broke any
/// declared resource law.
fn trace_lines(battery: &Battery) -> Vec<String> {
    (0..battery.trials)
        .map(|index| {
            let outcome = battery.account_trial(&battery.generate_trial(index));
            let verdict = if outcome.any_violation() {
                "a law was broken"
            } else {
                "all laws held"
            };
            format!("turn {index}: {verdict}")
        })
        .collect()
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
    resolve(sessions, &p.document)?;
    // No wired check currently demands an attestation, so the open-question set is
    // empty. When a check family posts attestation demands, this enumerates them.
    Ok(ok_response(
        request,
        &AttestMissingResult {
            questions: Vec::new(),
        },
    ))
}

fn handle_attestation_sweep(request: &Request) -> Result<Response, Fault> {
    let p: AttestationSweepParams = params(request)?;
    // The wire carries the store but no corpus reference; staleness is recomputed
    // against an empty model (the honest "no corpus supplied" reading). A record
    // whose stored fingerprint no longer matches the current content is stale.
    let model = WorldModel::default();
    Ok(ok_response(
        request,
        &AttestationSweepResult {
            stale: sweep(&p.store, &model),
        },
    ))
}

// -- dli.timeline.* ----------------------------------------------------------
//
// Two times meet here and stay apart (`03-time.md`): `show`/`order` read the
// resolved world's chronology (an era spine and events as partially-ordered
// points); `knows`/`episodes`/`phases` read the episode fold (production order —
// the listener's dimension) over the document's season. Each renders the same
// domain lines the CLI's `deloreis timeline` reads print, mapped onto the shared
// [`TimelineResult`]. Story-time is honestly partial: `order` reports unordered
// pairs as first-class, never a silent tie-break (`TemporalVerdict::Unordered`).

fn handle_timeline_show(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: TimelineShowParams = params(request)?;
    let scope = resolve(sessions, &p.document)?;
    let world = &scope.world.model.world;
    let points = time_universe(world);
    let mut findings = Vec::new();
    if points.is_empty() {
        findings.push("no chronology in this corpus".to_owned());
        return Ok(ok_response(request, &TimelineResult { findings }));
    }
    match world.chronology.topological_layers(&points) {
        Ok(layers) => {
            for (rank, layer) in layers.iter().enumerate() {
                let names: Vec<String> = layer.iter().map(|pt| point_name(world, *pt)).collect();
                findings.push(format!("layer {rank}: {}", names.join(", ")));
            }
        }
        Err(blame) => {
            findings.push(format!(
                "the chronology has a cycle over {} edge(s):",
                blame.cycle.len()
            ));
            for edge in &blame.cycle {
                findings.push(format!(
                    "  {} -> {}",
                    point_name(world, edge.before),
                    point_name(world, edge.after)
                ));
            }
            return Ok(ok_response(request, &TimelineResult { findings }));
        }
    }
    if let Some(at) = &p.at {
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
        let mut before = Vec::new();
        let mut after = Vec::new();
        for &other in &points {
            if other == cursor {
                continue;
            }
            match world.chronology.precedes(other, cursor) {
                TemporalVerdict::Before => before.push(point_name(world, other)),
                TemporalVerdict::After => after.push(point_name(world, other)),
                TemporalVerdict::Unordered => {}
            }
        }
        findings.push(format!(
            "at {at}: after [{}], before [{}]",
            before.join(", "),
            after.join(", ")
        ));
    }
    Ok(ok_response(request, &TimelineResult { findings }))
}

fn handle_timeline_order(request: &Request, sessions: &mut Sessions) -> Result<Response, Fault> {
    let p: TimelineOrderParams = params(request)?;
    let scope = resolve(sessions, &p.document)?;
    let world = &scope.world.model.world;
    let mut findings = Vec::new();
    if let Some(blame) = world.chronology.find_cycle(&[]) {
        findings.push(format!(
            "cycle in the chronology over {} edge(s):",
            blame.cycle.len()
        ));
        for edge in &blame.cycle {
            findings.push(format!(
                "  {} -> {}",
                point_name(world, edge.before),
                point_name(world, edge.after)
            ));
        }
        return Ok(ok_response(request, &TimelineResult { findings }));
    }
    let points = time_universe(world);
    let mut unordered = 0usize;
    for (i, &a) in points.iter().enumerate() {
        for &b in &points[i + 1..] {
            if world.chronology.precedes(a, b) == TemporalVerdict::Unordered {
                findings.push(format!(
                    "unordered: {} ? {}",
                    point_name(world, a),
                    point_name(world, b)
                ));
                unordered += 1;
            }
        }
    }
    findings.push(format!("chronology acyclic; {unordered} unordered pair(s)"));
    Ok(ok_response(request, &TimelineResult { findings }))
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
    let facts = folded.knowledge_at(slot, &Knower::Listener);
    let mut findings = Vec::new();
    if facts.is_empty() {
        findings.push(format!("at slot {slot}, the listener knows nothing"));
    } else {
        findings.push(format!("at slot {slot}, the listener knows:"));
        for fact in &facts {
            findings.push(format!("  {}", fact.0));
        }
    }
    Ok(ok_response(request, &TimelineResult { findings }))
}

fn handle_timeline_episodes(request: &Request, sessions: &Sessions) -> Result<Response, Fault> {
    let p: TimelineEpisodesParams = params(request)?;
    let episodes = resolve_season(sessions, &p.document)?;
    let folded = fold(&episodes);
    let mut ordered: Vec<&Episode> = episodes.iter().collect();
    ordered.sort_by_key(|ep| ep.production_order);
    let mut findings = Vec::new();
    for ep in &ordered {
        let anchor = ep
            .story_anchor
            .era
            .as_ref()
            .map_or_else(|| "unanchored".to_owned(), |era| era.0.clone());
        findings.push(format!(
            "slot {}: {} @ {anchor}",
            ep.production_order, ep.id
        ));
    }
    let loops = folded.open_loops();
    for question in &loops.open {
        findings.push(format!("open loop never resolved: {}", question.0));
    }
    findings.push(format!(
        "{} episode(s); {} loop(s) resolved, {} still open",
        episodes.len(),
        loops.closed.len(),
        loops.open.len()
    ));
    Ok(ok_response(request, &TimelineResult { findings }))
}

fn handle_timeline_phases(request: &Request, sessions: &Sessions) -> Result<Response, Fault> {
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
    let mut findings = Vec::new();
    for phase in &episode.story_anchor.phases {
        findings.push(format!(
            "episode anchor: {} = index {}",
            phase.cycle, phase.index
        ));
    }
    for scene in &episode.scenes {
        if let Some(anchor) = &scene.scene_anchor {
            for phase in &anchor.phases {
                findings.push(format!(
                    "scene {} anchor: {} = index {}",
                    scene.id, phase.cycle, phase.index
                ));
            }
        }
    }
    if findings.is_empty() {
        findings.push(format!("episode {} pins no cyclic phase", p.episode));
    }
    Ok(ok_response(request, &TimelineResult { findings }))
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

// -- engine → wire mappings --------------------------------------------------

/// Map an engine [`ForkDelta`] onto the wire [`DiffDelta`]: each status-change
/// list becomes its atoms rendered `pred(a, b)`; conflicts become their rendered
/// one-liners.
fn diff_delta(delta: &ForkDelta) -> DiffDelta {
    let render = |changes: &[AtomChange]| changes.iter().map(|c| c.atom.render()).collect();
    DiffDelta {
        newly_certain: render(&delta.newly_certain),
        newly_possible: render(&delta.newly_possible),
        newly_dead: render(&delta.newly_dead),
        grade_changed: render(&delta.grade_changed),
        conflicts: delta.conflicts.iter().map(|c| c.rendered.clone()).collect(),
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

/// Map a draft defect onto the wire [`Finding`]. A draft defect blocks acceptance,
/// so it carries error severity; its evidence grade is the checker's.
fn draft_finding(finding: &DraftFinding) -> Finding {
    Finding {
        severity: Severity::Error,
        code: "draft.defect".to_owned(),
        message: format!("{}: {}", finding.claim, finding.message),
        grade: finding.grade,
        repair: None,
        citations: Vec::new(),
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
/// A missing or mistyped field fails typed deserialization; an unknown extra
/// field survives it (serde ignores unknowns) but is caught by comparing the
/// input against the normalized re-serialization — the frozen contract types
/// carry no `deny_unknown_fields`, so this round-trip is the dependency-free
/// realization of the contract's "unknown fields are errors" discipline
/// (`integration/fantasy-contract.md` §2).
fn params<T: DeserializeOwned + Serialize>(request: &Request) -> Result<T, Fault> {
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

/// The first input field (dotted path) absent from the normalized re-serialization
/// — an unknown field the schema does not define. A `null` input value is treated
/// as an absent optional, never an unknown field, so an explicit `field: null`
/// never trips a false positive. Shared with the envelope-level check in
/// [`crate::serve`].
pub(crate) fn first_unknown_field(input: &Value, normalized: &Value) -> Option<String> {
    match (input, normalized) {
        (Value::Object(given), Value::Object(kept)) => {
            for (key, value) in given {
                match kept.get(key) {
                    None if !value.is_null() => return Some(key.clone()),
                    None => {}
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

/// Parse a rendered goal atom (`pred(a, b)`) into an [`AtomExpr`]; arguments are
/// taken as ground symbols.
fn parse_atom(text: &str) -> Result<AtomExpr, Fault> {
    let (pred, args) = split_atom(text)?;
    Ok(AtomExpr {
        pred: Ident::synthetic(&pred),
        args: args
            .into_iter()
            .map(|a| Term::Sym(Ident::synthetic(&a)))
            .collect(),
    })
}

/// Split `pred(a, b, c)` into its predicate and trimmed argument renderings.
fn split_atom(text: &str) -> Result<(String, Vec<String>), Fault> {
    let text = text.trim();
    let malformed = || {
        (
            ErrorCode::SchemaViolation,
            format!("'{text}' is not a pred(args) atom"),
        )
    };
    let open = text.find('(').ok_or_else(malformed)?;
    if !text.ends_with(')') {
        return Err(malformed());
    }
    let pred = text[..open].trim();
    if pred.is_empty() {
        return Err(malformed());
    }
    let inner = text[open + 1..text.len() - 1].trim();
    let args = if inner.is_empty() {
        Vec::new()
    } else {
        inner.split(',').map(|a| a.trim().to_owned()).collect()
    };
    Ok((pred.to_owned(), args))
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

/// The wire op names this build implements — every `dli.*` op the dispatch
/// answers with a real result. Every engine has now landed (fork.add's declaration
/// parser and the `timeline.*` chronology/episode reads were the last gated ops),
/// so the full op table is advertised and a consumer never needs to degrade
/// (`10-serving.md` §2).
fn capabilities() -> Vec<String> {
    Op::ALL.iter().copied().map(wire_name).collect()
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
