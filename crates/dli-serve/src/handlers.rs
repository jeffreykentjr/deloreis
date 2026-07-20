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
//! Ops that are capability-gated (`dli.check-proof`'s rich `ProofReport` awaits
//! the scene projection, `dli.export.run`, `dli.simulate.*`, `dli.timeline.*`,
//! and `dli.fork.add`'s declaration parser) are answered with the typed
//! [`ErrorCode::NotWired`] capability error rather than a panic — honest
//! capability negotiation (`10-serving.md` §2), with a domain-spoken message
//! naming the op and its pending milestone.

// The contract module is one flat table of param/result structs; a scoped glob
// is clearer than a 40-line import list, exactly as `schema.rs` does it.
#[allow(clippy::wildcard_imports)]
use crate::contract::*;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use dli_core::atom::{Interner, Rail};
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;

use dli_eval::EvalError;
use dli_eval::fork::{AtomChange, ForkDelta, ForkError, KeepStamp, Visibility};
use dli_eval::query::{Query, QueryOpts};
use dli_eval::world::GroundAtom;

use dli_ast::ident::Ident;
use dli_ast::term::{AtomExpr, Term};

use dli_world::check::{Severity as WorldSeverity, WorldFinding};
use dli_world::draft::DraftFinding;
use dli_world::language::{LangFinding, LanguageModel, view};
use dli_world::model::WorldModel;
use dli_world::{check, check_draft, sweep};

use crate::envelope::{ErrorBody, ErrorCode, Request, Response};
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
        Op::NameCheck => handle_name_check(request),
        Op::AttestMissing => handle_attest_missing(request, sessions),
        Op::AttestationSweep => handle_attestation_sweep(request),
        // Recognized, but capability-gated. Params are still validated first, so a
        // malformed request is a schema error (caught before the capability gate),
        // never masked by it.
        // check-proof's wire result is the rich `ProofReport` (per-step L1-L7
        // verdicts + weakest-link grade); building it needs the scene projection
        // (the payload lane, wave 4). The thin ledger-replay is not its contract.
        Op::CheckProof => unwired::<CheckProofParams>(request),
        // fork.add's declarations are full `.dli` declaration lines, not rendered
        // atoms; parsing them faithfully needs the syntax layer the worker does
        // not link yet, so the op is capability-gated rather than half-parsed.
        Op::ForkAdd => unwired::<ForkAddParams>(request),
        Op::ExportRun => unwired::<ExportRunParams>(request),
        Op::SimulateStart => unwired::<SimulateStartParams>(request),
        Op::SimulateStatus => unwired::<SimulateStatusParams>(request),
        Op::SimulateCancel => unwired::<SimulateCancelParams>(request),
        Op::TimelineShow => unwired::<TimelineShowParams>(request),
        Op::TimelineOrder => unwired::<TimelineOrderParams>(request),
        Op::TimelineKnows => unwired::<TimelineKnowsParams>(request),
        Op::TimelineEpisodes => unwired::<TimelineEpisodesParams>(request),
        Op::TimelinePhases => unwired::<TimelinePhasesParams>(request),
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
        | Op::AuthorSuppose => Err((
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
            contract: "fantasy-worker".to_owned(),
            contract_version: "0.1.0".to_owned(),
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
        Some(handle) => Some(fork_valuation(sessions, handle)?),
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
        Some(handle) => Some(
            sessions
                .fork_handle(handle)
                .ok_or_else(|| unknown_fork(handle))?
                .1,
        ),
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

fn handle_name_check(request: &Request) -> Result<Response, Fault> {
    let p: NameCheckParams = params(request)?;
    // The wire carries a `dialect` but no corpus reference; the kernel half vets
    // the candidate against an empty language model. Dialect-scoped vetting over a
    // loaded corpus is the future seam (`05-language-layer.md`).
    let _ = &p.dialect;
    let findings = view::vet_candidate(&LanguageModel::default(), &p.name);
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
fn fork_valuation(sessions: &mut Sessions, handle: &str) -> Result<Valuation, Fault> {
    let (id, scope) = fork_scope(sessions, handle)?;
    scope.forks.valuation(id).map_err(fork_fault)
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

/// Validate a not-yet-wired op's params, then report it capability-gated. A
/// malformed request is a schema error caught before the capability gate
/// (`10-serving.md` §2.1), so an unwired op never masks a bad request.
fn unwired<T: DeserializeOwned + Serialize>(request: &Request) -> Result<Response, Fault> {
    let _validated: T = params(request)?;
    Err(not_wired(request.op))
}

/// The recognized-but-unwired capability error for an op whose engine is pending.
/// `not-wired` is a permanent-until-wired refusal, distinct from an `internal`
/// server bug (`fantasy-contract.md` §9).
fn not_wired(op: Op) -> Fault {
    (
        ErrorCode::NotWired,
        format!(
            "operation '{}' is recognized but its engine is not wired in this build; \
             it is capability-gated until the engine lands",
            wire_name(op)
        ),
    )
}

/// The wire op names this build actually implements — every op the dispatch
/// answers with a real result. The capability-gated ops (whose engines have not
/// landed) are omitted, so a consumer can degrade rather than call one
/// (`10-serving.md` §2). [`GATED`] must mirror the capability-gated match arms
/// in [`handle`].
fn capabilities() -> Vec<String> {
    /// The ops whose engine has not landed — mirrors the `unwired` match arms.
    const GATED: [Op; 11] = [
        Op::CheckProof,
        Op::ForkAdd,
        Op::ExportRun,
        Op::SimulateStart,
        Op::SimulateStatus,
        Op::SimulateCancel,
        Op::TimelineShow,
        Op::TimelineOrder,
        Op::TimelineKnows,
        Op::TimelineEpisodes,
        Op::TimelinePhases,
    ];
    Op::ALL
        .iter()
        .copied()
        .filter(|op| !GATED.contains(op))
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
