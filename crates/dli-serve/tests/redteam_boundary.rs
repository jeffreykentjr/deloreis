#![allow(clippy::unwrap_used, clippy::expect_used)]
//! RED-TEAM: the 1.9.0/1.10.0/1.11.0 additive boundary across the wire.
//!
//! From the rebase red team (RT#3): `author.as_of`/`author.record_decision`
//! (contract 1.9.0) and `author.predicates`/`author.entity_candidates` (contract
//! 1.10.0) each advertise and answer only from their own floor up, and the
//! 1.10.0 capability list is exactly the 1.9.0 list plus the two reads — no
//! reordering, no drop.

use serde_json::{Value, json};

use dli_serve::author::store::Store;
use dli_serve::session::Sessions;
use dli_serve::{ErrorCode, Op, Request};

fn req(dli: &str, id: &str, op: Op, params: Value) -> Request {
    Request {
        dli: dli.to_owned(),
        id: id.to_owned(),
        op,
        params,
        expected_digest: None,
        idempotency_key: None,
    }
}

fn caps(dli: &str) -> Vec<String> {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let resp = dli_serve::serve::dispatch(
        &req(dli, "v", Op::Version, json!({})),
        &mut sessions,
        &mut store,
    );
    resp.result.expect("version result")["capabilities"]
        .as_array()
        .expect("capabilities array")
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

fn dispatch_op(dli: &str, op: Op) -> dli_serve::Response {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    dli_serve::serve::dispatch(
        &req(
            dli,
            "x",
            op,
            json!({ "workspace": "nope", "surface": "s", "limit": 1 }),
        ),
        &mut sessions,
        &mut store,
    )
}

const ASOF: &str = "author.as_of";
const RECDEC: &str = "author.record_decision";
const PRED: &str = "author.predicates";
const CAND: &str = "author.entity_candidates";

#[test]
fn capability_matrix_across_the_1_9_1_10_1_11_boundary() {
    let c18 = caps("1.8.0");
    let c19 = caps("1.9.0");
    let c110 = caps("1.10.0");
    let c111 = caps("1.11.0");
    for op in [ASOF, RECDEC, PRED, CAND] {
        assert!(
            !c18.contains(&op.to_owned()),
            "1.8.0 must NOT advertise {op}: {c18:?}"
        );
    }
    assert!(
        c19.contains(&ASOF.to_owned()),
        "1.9.0 advertises as_of: {c19:?}"
    );
    assert!(
        c19.contains(&RECDEC.to_owned()),
        "1.9.0 advertises record_decision: {c19:?}"
    );
    assert!(
        !c19.contains(&PRED.to_owned()),
        "1.9.0 must NOT advertise predicates: {c19:?}"
    );
    assert!(
        !c19.contains(&CAND.to_owned()),
        "1.9.0 must NOT advertise entity_candidates: {c19:?}"
    );
    for op in [ASOF, RECDEC, PRED, CAND] {
        assert!(
            c110.contains(&op.to_owned()),
            "1.10.0 advertises {op}: {c110:?}"
        );
    }
    let mut expected = c19.clone();
    expected.push(PRED.to_owned());
    expected.push(CAND.to_owned());
    assert_eq!(
        c110, expected,
        "1.10.0 caps == 1.9.0 caps ++ [predicates, entity_candidates]"
    );
    assert_eq!(
        c111, c110,
        "1.11.0 changes entity-candidate semantics without changing capabilities"
    );
}

#[test]
fn dispatch_gating_matches_the_advertisement() {
    for op in [Op::AuthorAsOf, Op::AuthorRecordDecision] {
        let resp = dispatch_op("1.9.0", op);
        if let Some(err) = &resp.error {
            assert!(
                !(err.code == ErrorCode::UnknownOp && err.message.contains("not available before")),
                "{op:?} must NOT be version-gated at 1.9.0: {err:?}"
            );
        }
    }
    let asof18 = dispatch_op("1.8.0", Op::AuthorAsOf);
    let e = asof18.error.expect("as_of refused at 1.8.0");
    assert_eq!(
        e.code,
        ErrorCode::UnknownOp,
        "as_of@1.8.0 unknown-op: {e:?}"
    );
    for dli in ["1.8.0", "1.9.0"] {
        for op in [Op::AuthorPredicates, Op::AuthorEntityCandidates] {
            let resp = dispatch_op(dli, op);
            let e = resp.error.expect("refused");
            assert_eq!(
                e.code,
                ErrorCode::UnknownOp,
                "{op:?}@{dli} unknown-op: {e:?}"
            );
            assert!(
                e.message.contains("1.10.0"),
                "{op:?}@{dli} names 1.10.0: {}",
                e.message
            );
        }
    }
    for op in [Op::AuthorPredicates, Op::AuthorEntityCandidates] {
        let resp = dispatch_op("1.10.0", op);
        if let Some(err) = &resp.error {
            assert!(
                !(err.code == ErrorCode::UnknownOp && err.message.contains("not available before")),
                "{op:?} must NOT be version-gated at 1.10.0: {err:?}"
            );
        }
    }
}
