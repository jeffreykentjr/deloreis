//! The authoring seam (`AUTH-1`): the authoring workspace engine and the `author.*` tools.
//!
//! The frozen serve seam: `serve.rs` routes every `author.*` op through
//! [`dispatch`] — one call, one store. The workspace digest is the wire fence
//! (`expected_digest` / stale-fence in the envelope), and every mutating call
//! answers immediately with graded findings, the rendered canonical surface, and
//! the new digest (spec 11 §4).
//!
//! ```
//! use dli_serve::author::{dispatch, store::Store};
//! use dli_serve::{Op, Request};
//! use serde_json::json;
//!
//! let mut store = Store::default();
//! let open = Request {
//!     dli: "0.5.1".to_owned(),
//!     id: "1".to_owned(),
//!     op: Op::AuthorOpen,
//!     params: json!({ "scope": "demo" }),
//!     expected_digest: None,
//!     idempotency_key: None,
//! };
//! let opened = dispatch(&open, &mut store);
//! assert!(opened.ok);
//! ```

// The error carrier threaded through dispatch is the wire `Response` — a large
// but deliberate type: an early return IS a built response. Boxing it would add
// deref noise to every arm for no wire benefit.
#![allow(clippy::result_large_err)]

pub mod digest;
pub mod reads;
pub mod render;
pub mod store;
pub mod tools;
pub mod workspace;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::contract::{
    AuthorAddAttestParams, AuthorAddBannedParams, AuthorAddBeforeParams, AuthorAddDialectParams,
    AuthorAddEntityParams, AuthorAddEventParams, AuthorAddFactParams, AuthorAddHypothesisParams,
    AuthorAddNameParams, AuthorAddNamePartParams, AuthorAddOptionParams, AuthorAddRelationParams,
    AuthorAddRuleParams, AuthorAddStateParams, AuthorAddTermParams, AuthorAsOfParams,
    AuthorAssertPredicateSameAsParams, AuthorAssertSameAsParams, AuthorBranchParams,
    AuthorCheckParams, AuthorDeclareFunctionalParams, AuthorDeclareInertialParams,
    AuthorDefeatParams, AuthorDiffParams, AuthorDiscardParams, AuthorEntityCandidatesParams,
    AuthorFindingsParams, AuthorListWorkspacesParams, AuthorMaterializeParams, AuthorMergeParams,
    AuthorOpenParams, AuthorOpenQuestionParams, AuthorPredicatesParams, AuthorQueryParams,
    AuthorRecordDecisionParams, AuthorRecordDecisionsParams, AuthorSetStandingParams,
    AuthorStatusParams, AuthorSupposeParams, AuthorWhyParams,
};
use crate::envelope::{ErrorBody, ErrorCode, Request, Response};
use crate::ops::Op;

/// The authoring seam's dispatch (frozen verbatim in the wave-3 sockets): route an
/// `author.*` request against the workspace store and answer with graded
/// findings, rendered lines, and the new workspace digest.
// A flat routing table over the closed op set — one arm per tool, so it runs long
// by construction; splitting it would only scatter the table.
#[allow(clippy::too_many_lines)]
pub fn dispatch(request: &Request, store: &mut store::Store) -> Response {
    match request.op {
        Op::AuthorOpen => match parse::<AuthorOpenParams>(request) {
            Ok(params) => settle(request, tools::open(store, &params)),
            Err(response) => response,
        },
        Op::AuthorStatus => read(request, store, |store, request| {
            let params: AuthorStatusParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::status(store, &params)))
        }),
        Op::AuthorDiff => read(request, store, |store, request| {
            let params: AuthorDiffParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::diff(store, &params)))
        }),
        // materialize advances the lorestore ref: a mutation, dispatched
        // through the mutating wrapper so idempotency-key replay works
        // (audit B9).
        Op::AuthorMaterialize => mutate(request, store, |store, request| {
            let params: AuthorMaterializeParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::materialize(store, &params)))
        }),
        Op::AuthorDiscard => mutate(request, store, |store, request| {
            let params: AuthorDiscardParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            let decl = request.params.get("decl").and_then(Value::as_str);
            Ok((
                params.workspace.clone(),
                tools::discard(store, &params.workspace, decl),
            ))
        }),
        Op::AuthorAddEntity => mutate(request, store, |store, request| {
            let params: AuthorAddEntityParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_entity(store, &params)))
        }),
        Op::AuthorAddFact => mutate(request, store, |store, request| {
            let params: AuthorAddFactParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_fact(store, &params)))
        }),
        Op::AuthorAddRelation => mutate(request, store, |store, request| {
            let params: AuthorAddRelationParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::add_relation(store, &params),
            ))
        }),
        Op::AuthorAddEvent => mutate(request, store, |store, request| {
            let params: AuthorAddEventParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_event(store, &params)))
        }),
        Op::AuthorAddBefore => mutate(request, store, |store, request| {
            let params: AuthorAddBeforeParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_before(store, &params)))
        }),
        Op::AuthorOpenQuestion => mutate(request, store, |store, request| {
            let params: AuthorOpenQuestionParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::open_question(store, &params),
            ))
        }),
        Op::AuthorAddOption => mutate(request, store, |store, request| {
            let params: AuthorAddOptionParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_option(store, &params)))
        }),
        Op::AuthorAddHypothesis => mutate(request, store, |store, request| {
            let params: AuthorAddHypothesisParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::add_hypothesis(store, &params),
            ))
        }),
        Op::AuthorSuppose => mutate(request, store, |store, request| {
            let params: AuthorSupposeParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::suppose(store, &params)))
        }),
        // The DM-4 expert authoring surface (contract 1.3.0). Ordinary staging
        // mutations: same wrapper, same fences, same graded reply as the tools
        // above — they differ only in which declaration they construct.
        Op::AuthorAddDialect => mutate(request, store, |store, request| {
            let params: AuthorAddDialectParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_dialect(store, &params)))
        }),
        Op::AuthorAddNamePart => mutate(request, store, |store, request| {
            let params: AuthorAddNamePartParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::add_name_part(store, &params),
            ))
        }),
        Op::AuthorAddName => mutate(request, store, |store, request| {
            let params: AuthorAddNameParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_name(store, &params)))
        }),
        Op::AuthorAddTerm => mutate(request, store, |store, request| {
            let params: AuthorAddTermParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_term(store, &params)))
        }),
        Op::AuthorAddBanned => mutate(request, store, |store, request| {
            let params: AuthorAddBannedParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_banned(store, &params)))
        }),
        Op::AuthorAddState => mutate(request, store, |store, request| {
            let params: AuthorAddStateParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_state(store, &params)))
        }),
        Op::AuthorAddAttest => mutate(request, store, |store, request| {
            let params: AuthorAddAttestParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_attest(store, &params)))
        }),
        Op::AuthorSetStanding => mutate(request, store, |store, request| {
            let params: AuthorSetStandingParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::set_standing(store, &params),
            ))
        }),
        Op::AuthorDefeat => mutate(request, store, |store, request| {
            let params: AuthorDefeatParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::defeat(store, &params, true),
            ))
        }),
        Op::AuthorDeclareFunctional => mutate(request, store, |store, request| {
            let params: AuthorDeclareFunctionalParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::declare_functional(store, &params),
            ))
        }),
        Op::AuthorAssertSameAs => mutate(request, store, |store, request| {
            let params: AuthorAssertSameAsParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::assert_same_as(store, &params),
            ))
        }),
        Op::AuthorAssertPredicateSameAs => mutate(request, store, |store, request| {
            let params: AuthorAssertPredicateSameAsParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::assert_predicate_same_as(store, &params),
            ))
        }),
        Op::AuthorAddRule => mutate(request, store, |store, request| {
            let params: AuthorAddRuleParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::add_rule(store, &params)))
        }),
        Op::AuthorDeclareInertial => mutate(request, store, |store, request| {
            let params: AuthorDeclareInertialParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::declare_inertial(store, &params),
            ))
        }),
        // The baseline-decision recorder (contract 1.9.0, DL-0083): writes a
        // `ratify` carriage to the workspace branch. A mutation like `defeat`,
        // dispatched through `mutate` so idempotency-key replay works and no
        // side channel reaches certainty (`_ => UnknownOp` in the batch seam is
        // a runtime miss, so this arm is added by hand).
        Op::AuthorRecordDecision => mutate(request, store, |store, request| {
            let params: AuthorRecordDecisionParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                tools::record_decision(store, &params),
            ))
        }),
        Op::AuthorRecordDecisions => mutate(request, store, |store, request| {
            let params: AuthorRecordDecisionsParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            precheck_record_decisions(request, &params)?;
            Ok((
                params.workspace.clone(),
                tools::record_decisions(store, &params),
            ))
        }),
        // The read/self-check tools (AUTH-2): each answers over the STAGED world
        // (base + staged) and stages nothing, so they route through `read` — no
        // idempotency caching, only the tool's result or a typed error.
        Op::AuthorQuery => read(request, store, |store, request| {
            let params: AuthorQueryParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), reads::query(store, &params)))
        }),
        // The one inertia-aware read (contract 1.9.0, DL-0082): a workspace
        // read like `author.query`, never a mutation — it stages nothing, so
        // it routes through `read`, not `mutate`, and carries no idempotency
        // fence.
        Op::AuthorAsOf => read(request, store, |store, request| {
            let params: AuthorAsOfParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), reads::as_of(store, &params)))
        }),
        Op::AuthorCheck => read(request, store, |store, request| {
            let params: AuthorCheckParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), reads::check(store, &params)))
        }),
        Op::AuthorWhy => read(request, store, |store, request| {
            let params: AuthorWhyParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                reads::why(store, &params, true, true),
            ))
        }),
        Op::AuthorFindings => read(request, store, |store, request| {
            let params: AuthorFindingsParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), reads::findings(store, &params)))
        }),
        // No workspace param, so no precheck: a store-level read of the open set.
        Op::AuthorListWorkspaces => read(request, store, |store, request| {
            let _params: AuthorListWorkspacesParams = parse(request)?;
            Ok((String::new(), Ok(reads::list_workspaces(store))))
        }),
        // The incremental-canonicalization reads (contract 1.10.0): both answer
        // over the STAGED world and stage nothing, so they route through `read`.
        Op::AuthorPredicates => read(request, store, |store, request| {
            let params: AuthorPredicatesParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), reads::predicates(store, &params)))
        }),
        Op::AuthorEntityCandidates => read(request, store, |store, request| {
            let params: AuthorEntityCandidatesParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((
                params.workspace.clone(),
                reads::entity_candidates(store, &params),
            ))
        }),
        // Branch/merge operate on the workspace's durable lorestore (LS-6/WS-3).
        // Routed through `mutate`, not `read` (WS-3): `delete` removes a ref and
        // a real merge can advance one, and idempotency-key replay only ever
        // gets CACHED by the `mutate` wrapper — `precheck`'s replay lookup was
        // already live here, but nothing populated it while these used `read`,
        // so a retry re-ran the mutation instead of replaying (the gap WS-3
        // closes; see the `ls10_branch_writes.rs` idempotent-replay test).
        Op::AuthorBranch => mutate(request, store, |store, request| {
            let params: AuthorBranchParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::branch(store, &params)))
        }),
        Op::AuthorMerge => mutate(request, store, |store, request| {
            let params: AuthorMergeParams = parse(request)?;
            precheck(store, request, &params.workspace)?;
            Ok((params.workspace.clone(), tools::merge(store, &params)))
        }),
        // Every non-`author.*` op is the batch worker's; the frozen seam only
        // ever receives `author.*`, so anything else is an unknown op here.
        _ => error(
            request,
            ErrorBody {
                code: ErrorCode::UnknownOp,
                message: format!(
                    "{:?} is not an author.* op; the author dispatch handles only Door T tools",
                    request.op
                ),
                data: None,
            },
        ),
    }
}

/// A per-op step: parse, precheck, and run a tool; any early return is a fully
/// built error [`Response`], and success carries the workspace id (for
/// idempotency caching) and the tool outcome.
type Step = Result<(String, Result<Value, ErrorBody>), Response>;

/// Run a read-only op: no idempotency caching (nothing is staged), just the
/// tool's result or error.
fn read(
    request: &Request,
    store: &mut store::Store,
    step: impl FnOnce(&mut store::Store, &Request) -> Step,
) -> Response {
    match step(store, request) {
        Ok((_, outcome)) => settle(request, outcome),
        Err(response) => response,
    }
}

/// Run a mutating op: on success, cache the result under any idempotency key so a
/// retry replays it and stages nothing.
fn mutate(
    request: &Request,
    store: &mut store::Store,
    step: impl FnOnce(&mut store::Store, &Request) -> Step,
) -> Response {
    debug_assert!(
        request.op.is_post_open_author_mutation(),
        "every author::dispatch arm routed through mutate must be registered as a post-open mutation: {:?}",
        request.op
    );
    match step(store, request) {
        Ok((workspace, Ok(result))) => {
            if let Some(key) = &request.idempotency_key {
                store.remember(
                    &workspace,
                    key.clone(),
                    fingerprint(request),
                    result.clone(),
                );
            }
            ok(request, result)
        }
        Ok((_, Err(body))) => error(request, body),
        Err(response) => response,
    }
}

/// Turn a tool outcome into a response.
fn settle(request: &Request, outcome: Result<Value, ErrorBody>) -> Response {
    match outcome {
        Ok(result) => ok(request, result),
        Err(body) => error(request, body),
    }
}

/// The existence, idempotency-replay, and stale-fence gate every workspace-scoped
/// op runs before staging. Returns `Err(response)` to short-circuit — an
/// idempotency replay is a success response, a missing workspace or moved fence a
/// typed error.
fn precheck(store: &store::Store, request: &Request, ws_id: &str) -> Result<(), Response> {
    if let Some(key) = &request.idempotency_key
        && let Some((cached_fingerprint, cached_result)) = store.replay(ws_id, key)
    {
        // A retry with the identical payload replays; the same key with a
        // different payload is a conflict, never a silent cached replay (R3/M9).
        if cached_fingerprint == fingerprint(request) {
            return Err(ok(request, cached_result.clone()));
        }
        return Err(error(
            request,
            ErrorBody {
                code: ErrorCode::Conflict,
                message: format!(
                    "idempotency key \"{key}\" was already applied to a different request; a reused key must carry the identical payload."
                ),
                data: None,
            },
        ));
    }
    let Some(ws) = store.get(ws_id) else {
        return Err(error(
            request,
            ErrorBody {
                code: ErrorCode::UnknownDoc,
                message: format!("no authoring workspace \"{ws_id}\" is open"),
                data: None,
            },
        ));
    };
    if let Some(expected) = &request.expected_digest
        && expected != &ws.digest
    {
        return Err(error(
            request,
            ErrorBody {
                code: ErrorCode::StaleFence,
                message: format!(
                    "workspace {ws_id} is at digest {}; you presented {expected}. Read author.status before continuing.",
                    ws.digest
                ),
                data: Some(serde_json::json!({
                    "expected_digest": ws.digest,
                    "presented_digest": expected,
                })),
            },
        ));
    }
    if let Some(message) = provenance_contract_error(&request.dli, &request.params["provenance"]) {
        return Err(error(
            request,
            ErrorBody {
                code: ErrorCode::SchemaViolation,
                message: message.to_owned(),
                data: None,
            },
        ));
    }
    Ok(())
}

/// Apply the ordinary negotiated-provenance fence to every nested record in a
/// bulk decision request. `precheck` handles only the common top-level
/// `params.provenance` shape; without this explicit descent, a cited record at
/// 1.5+ could omit its required state and silently lower as legacy `Decided`.
fn precheck_record_decisions(
    request: &Request,
    params: &AuthorRecordDecisionsParams,
) -> Result<(), Response> {
    for (record_index, record) in params.records.iter().enumerate() {
        let Some(provenance) = request.params["records"][record_index].get("provenance") else {
            continue;
        };
        if let Some(message) = provenance_contract_error(&request.dli, provenance) {
            return Err(error(
                request,
                ErrorBody {
                    code: ErrorCode::SchemaViolation,
                    message: message.to_owned(),
                    data: Some(serde_json::json!({
                        "record_index": record_index,
                        "decision": record.decision,
                    })),
                },
            ));
        }
    }
    Ok(())
}

/// The cited-provenance rule shared by single and bulk authoring calls.
fn provenance_contract_error(_dli: &str, provenance: &serde_json::Value) -> Option<&'static str> {
    if provenance["kind"] == serde_json::json!("cited") && provenance.get("state").is_none() {
        return Some("cited provenance requires state `provisional` or `ratified`");
    }
    None
}

/// A stable fingerprint of a request's mutating content (its op and params), so
/// a reused idempotency key with a different payload is caught as a conflict
/// rather than replaying the cached result (R3/M9).
fn fingerprint(request: &Request) -> String {
    digest::digest_of(&(request.op, &request.params))
}

/// Deserialize op params, or a `schema-violation` error response. When the
/// contract param types carry `deny_unknown_fields` (contract-close owns that),
/// an unknown top-level or nested field fails this deserialize and is reported
/// here as a `schema-violation` (R3/H2).
fn parse<T: DeserializeOwned>(request: &Request) -> Result<T, Response> {
    if !request.params.is_object() {
        return Err(error(
            request,
            ErrorBody {
                code: ErrorCode::SchemaViolation,
                message: "params must be an object".to_owned(),
                data: None,
            },
        ));
    }
    let decoded = serde_json::from_value(request.params.clone());
    decoded.map_err(|err| {
        // The authoring seam's twin of the batch seam's check: a retired params field the
        // struct no longer declares fails `deny_unknown_fields` here, and the
        // killed-field registry renames the anonymous schema violation into a
        // refusal that names its successor (LS-7). Since LS-8 a retired field
        // that is still declared never reaches here — `serve::dispatch` refuses
        // it above the routing fork — so this covers the shape 1.0 will have
        // once the field leaves the struct outright.
        let (code, message) =
            crate::serve::killed_field_successor(&request.dli, request.op, &request.params)
                .map_or_else(
                    || (ErrorCode::SchemaViolation, err.to_string()),
                    |(field, successor)| {
                        (
                            ErrorCode::KilledForm,
                            crate::serve::killed_field_message(&request.dli, field, successor),
                        )
                    },
                );
        error(
            request,
            ErrorBody {
                code,
                message,
                data: None,
            },
        )
    })
}

fn ok(request: &Request, result: Value) -> Response {
    Response {
        dli: request.dli.clone(),
        id: request.id.clone(),
        ok: true,
        result: Some(result),
        error: None,
    }
}

fn error(request: &Request, body: ErrorBody) -> Response {
    Response {
        dli: request.dli.clone(),
        id: request.id.clone(),
        ok: false,
        result: None,
        error: Some(body),
    }
}
