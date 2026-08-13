//! LS-8 contract 1.0: the field retirement, both sides of it.
//!
//! LS-7 registered four text-carrying params fields against their typed
//! successors and left the registry **dormant** — no accepted version reached
//! the retirement major, and `dispatch` did not consult the registry at all.
//! LS-8 turns it live at contract `1.0.0` and wires it into `dispatch`, above
//! the `author.*` / `dli.*` routing fork so one check covers both seams.
//!
//! The retirement is a hinge, not a deletion, and this file asserts both faces
//! of it on the wire:
//!
//! | | `0.5.1`–`0.6.4` | `1.0.0` |
//! |---|---|---|
//! | string form present | answered | `killed-form`, naming the successor |
//! | string form absent | `schema-violation: missing field` | answered |
//! | typed successor | answered | answered |
//!
//! The right column is new; the left column is what LS-8 must not have moved.
//! It is asserted here per-version because "additive" is a claim about every
//! wire this build still speaks, not only the newest one.
//!
//! This file needs the `.dli` front end: the *dormant* half is only a real
//! claim where the string form is genuinely answered, and without the parser a
//! `0.x` string goal is refused for an unrelated reason (LS-7, no front end).
//! The lane-independent half — that the refusal fires at `1.0` in the
//! 1.0-native default build too — lives in `default_build.rs`.
#![cfg(feature = "syntax")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use dli_serve::author::store::Store;
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use serde_json::{Value, json};

/// Every accepted version that predates the retirement — the wires LS-8 must
/// leave exactly as it found them.
fn pre_1_0() -> Vec<&'static str> {
    dli_serve::ACCEPTED_CONTRACT_VERSIONS
        .iter()
        .copied()
        .filter(|version| !version.starts_with("1."))
        .collect()
}

fn drive(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let line = serde_json::to_string(frame).unwrap();
    let response = respond(&line, sessions, store).expect("a response for every request frame");
    serde_json::to_value(&response).unwrap()
}

fn fixture_project() -> (tempfile::TempDir, Sessions, Store) {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/dli-worker/fixture-project");
    let temp = tempfile::tempdir().unwrap();
    copy_tree(&source, temp.path());
    let project = Arc::new(Project::load(temp.path()).unwrap());
    let packs = project.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    let store = Store::with_packs(packs).unwrap();
    let sessions = Sessions::from_project(Arc::clone(&project));
    (temp, sessions, store)
}

fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let to = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir(&to).unwrap();
            copy_tree(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// An `Ident` on the wire: the text plus the synthetic span typed fields carry.
fn ident(text: &str) -> Value {
    json!({ "text": text, "span": { "file": 0, "start": 0, "end": 0 } })
}

/// `marker(baseline)` as a typed atom — the fact the fixture project's
/// `saltmarket` document ratifies, so a query for it answers with a row.
fn marker_baseline() -> Value {
    json!({ "pred": ident("marker"), "args": [{ "Sym": ident("baseline") }] })
}

fn error(response: &Value) -> &Value {
    response
        .get("error")
        .unwrap_or_else(|| panic!("expected a refusal, got {response}"))
}

fn code(response: &Value) -> &str {
    error(response)["code"].as_str().unwrap()
}

fn message(response: &Value) -> &str {
    error(response)["message"].as_str().unwrap()
}

/// The empty `payload/v1` envelope, with `claims` supplied by the caller: the
/// only field under test is a claim's `atom` / `atom_expr`.
fn payload(claims: &Value) -> Value {
    json!({
        "meta": {
            "episode": "episode-1",
            "source": {
                "file": "draft.md",
                "sha256": "056d80e18f378bfb3d48d8533338ba9e6e997ceb8c11e8fd5c5f58cbb5c981b0",
                "words": 3,
            },
            "extracted": "2026-07-25",
            "extractors": { "schema": "payload/v1" },
        },
        "claims": claims.clone(),
        "deltas": [],
        "scenes": [],
        "beats": [],
        "promises": { "opened": [], "closed": [] },
        "proof_ledgers": [],
        "names": { "introduced": [] },
        "lexicon": { "banned_hits": [], "register_tokens": 0 },
    })
}

/// One registered retirement, as the wire sees it: the request that carries the
/// string form, the request that carries the typed successor alone, and the
/// successor's name the refusal must speak.
struct Retirement {
    op: &'static str,
    successor: &'static str,
    /// Frames that must succeed first (open a workspace, open a fork).
    precondition: Vec<(&'static str, Value)>,
    string_form: Value,
    typed_form: Value,
    /// The same typed request as a pre-1.0 wire must spell it: the retired
    /// field is still required there, so it rides along carrying nothing. The
    /// typed successor wins in both, which is what makes the two comparable.
    typed_form_below_1_0: Value,
}

fn retirements() -> Vec<Retirement> {
    vec![
        Retirement {
            op: "dli.query",
            successor: "goal_atom",
            precondition: Vec::new(),
            string_form: json!({ "document": "saltmarket", "goal": "marker(baseline)" }),
            typed_form: json!({ "document": "saltmarket", "goal_atom": marker_baseline() }),
            typed_form_below_1_0: json!({
                "document": "saltmarket",
                "goal": "",
                "goal_atom": marker_baseline(),
            }),
        },
        Retirement {
            op: "author.query",
            successor: "goal_atom",
            precondition: vec![("author.open", json!({ "scope": "saltmarket" }))],
            string_form: json!({ "workspace": "ws-1", "goal": "marker(baseline)" }),
            typed_form: json!({ "workspace": "ws-1", "goal_atom": marker_baseline() }),
            typed_form_below_1_0: json!({
                "workspace": "ws-1",
                "goal": "",
                "goal_atom": marker_baseline(),
            }),
        },
        Retirement {
            op: "dli.fork.add",
            successor: "additions",
            precondition: vec![("dli.fork.new", json!({ "document": "saltmarket" }))],
            string_form: json!({ "fork": "fork-1", "declarations": ["note(\"x\") proposed"] }),
            typed_form: json!({
                "fork": "fork-1",
                "additions": [{ "pred": "note", "args": ["x"] }],
            }),
            typed_form_below_1_0: json!({
                "fork": "fork-1",
                "declarations": [],
                "additions": [{ "pred": "note", "args": ["x"] }],
            }),
        },
        Retirement {
            op: "dli.payload-submit",
            successor: "atom_expr",
            precondition: Vec::new(),
            string_form: json!({
                "document": "saltmarket",
                "source": "the marker holds\n",
                "payload": payload(&json!([
                    { "atom": "marker(\"control\")", "quote": "the marker holds", "line": 1 }
                ])),
            }),
            typed_form: json!({
                "document": "saltmarket",
                "source": "the marker holds\n",
                "payload": payload(&json!([
                    {
                        "atom_expr": { "pred": ident("marker"), "args": [{ "Str": "control" }] },
                        "quote": "the marker holds",
                        "line": 1,
                    }
                ])),
            }),
            typed_form_below_1_0: json!({
                "document": "saltmarket",
                "source": "the marker holds\n",
                "payload": payload(&json!([
                    {
                        "atom": "",
                        "atom_expr": { "pred": ident("marker"), "args": [{ "Str": "control" }] },
                        "quote": "the marker holds",
                        "line": 1,
                    }
                ])),
            }),
        },
    ]
}

/// Run a retirement's preconditions and then one request, from a fresh project.
fn send(version: &str, op: &str, params: &Value, precondition: &[(&str, Value)]) -> Value {
    let (_temp, mut sessions, mut store) = fixture_project();
    for (index, (pre_op, pre_params)) in precondition.iter().enumerate() {
        let response = drive(
            &json!({ "dli": version, "id": format!("pre-{index}"), "op": pre_op, "params": pre_params }),
            &mut sessions,
            &mut store,
        );
        assert_eq!(
            response["ok"],
            Value::Bool(true),
            "{version} precondition {pre_op} failed: {response}"
        );
    }
    drive(
        &json!({ "dli": version, "id": "1", "op": op, "params": params }),
        &mut sessions,
        &mut store,
    )
}

// -- the new side: contract 1.0 refuses the string form -----------------------

/// Every registered retirement fires at `1.0.0`, on the live path — not at the
/// decode seam, where a still-declared field could never arrive. The refusal is
/// `killed-form` and it names the successor field, exactly as a killed *op* is
/// refused naming its successor op.
#[test]
fn every_registered_string_form_is_refused_at_1_0() {
    for case in retirements() {
        let response = send("1.0.0", case.op, &case.string_form, &case.precondition);
        assert_eq!(
            code(&response),
            "killed-form",
            "{} string form at 1.0.0: {response}",
            case.op
        );
        assert!(
            message(&response).contains(case.successor),
            "{}: the refusal must name '{}', got: {}",
            case.op,
            case.successor,
            message(&response)
        );
        assert_eq!(
            response["dli"], "1.0.0",
            "the refusal echoes the negotiated wire: {response}"
        );
        assert_eq!(response["id"], "1", "the refusal echoes the id: {response}");
    }
}

/// The refusal reaches `author.*` as well as `dli.*`. It has to be checked
/// separately: the authoring seam is a different decode seam behind a different branch, and
/// the check earns both only by sitting above the fork in `dispatch`.
#[test]
fn the_refusal_covers_the_author_seam_too() {
    let response = send(
        "1.0.0",
        "author.query",
        &json!({ "workspace": "ws-1", "goal": "marker(baseline)" }),
        &[("author.open", json!({ "scope": "saltmarket" }))],
    );
    assert_eq!(code(&response), "killed-form", "{response}");
    assert!(message(&response).contains("goal_atom"), "{response}");
}

/// A string form sent with its typed successor beside it is still refused. The
/// retirement is of the *field*, not of the ambiguity: 1.0 does not quietly
/// prefer the typed one and drop the retired key on the floor.
#[test]
fn carrying_both_forms_is_still_refused_at_1_0() {
    let response = send(
        "1.0.0",
        "dli.query",
        &json!({
            "document": "saltmarket",
            "goal": "marker(baseline)",
            "goal_atom": marker_baseline(),
        }),
        &[],
    );
    assert_eq!(code(&response), "killed-form", "{response}");
    assert!(message(&response).contains("goal_atom"), "{response}");
}

/// The typed successor is answered at 1.0, sent **alone** — which is the whole
/// point of the retirement, and the reason the string form had to stop being a
/// required field. A 1.0 caller has a way to say the thing.
#[test]
fn the_typed_successor_is_answered_at_1_0() {
    for case in retirements() {
        let response = send("1.0.0", case.op, &case.typed_form, &case.precondition);
        assert_eq!(
            response["ok"],
            Value::Bool(true),
            "{} typed form at 1.0.0 must be answered: {response}",
            case.op
        );
    }
}

/// A 1.0 answer is the 0.6.4 answer. The major bump retires a spelling; it does
/// not reshape a result, so the same typed request differs only in the echoed
/// protocol tag and in whether the retired field had to ride along empty.
#[test]
fn a_1_0_result_equals_its_0_6_4_twin() {
    for case in retirements() {
        let older = send(
            "0.6.4",
            case.op,
            &case.typed_form_below_1_0,
            &case.precondition,
        );
        let newer = send("1.0.0", case.op, &case.typed_form, &case.precondition);
        assert_eq!(older["dli"], "0.6.4");
        assert_eq!(newer["dli"], "1.0.0");
        assert_eq!(
            older["ok"],
            Value::Bool(true),
            "{}: the 0.6.4 twin must be answered: {older}",
            case.op
        );
        assert_eq!(
            older["result"], newer["result"],
            "{}: 1.0 reshaped a result",
            case.op
        );
    }
}

// -- the old side: every 0.x wire is where LS-8 found it ----------------------

/// The registry is dormant below 1.0 on every accepted version: the string form
/// is answered, not refused. This is the claim "additive" actually makes, and
/// it is checked per-version rather than on the newest 0.x alone.
#[test]
fn every_string_form_is_still_answered_below_1_0() {
    for version in pre_1_0() {
        for case in retirements() {
            let response = send(version, case.op, &case.string_form, &case.precondition);
            assert_eq!(
                response["ok"],
                Value::Bool(true),
                "{} string form at {version} must still be answered: {response}",
                case.op
            );
        }
    }
}

/// The other half of the hinge: below 1.0 the string form is still REQUIRED.
/// Making the field optional is what lets 1.0 omit it, and it must not have
/// loosened the wires below — a `0.x` request that omits it is the same
/// `schema-violation` it always was, with serde's own wording.
#[test]
fn the_string_form_is_still_required_below_1_0() {
    for version in pre_1_0() {
        let response = send(
            version,
            "dli.query",
            &json!({ "document": "saltmarket", "goal_atom": marker_baseline() }),
            &[],
        );
        assert_eq!(
            code(&response),
            "schema-violation",
            "{version} still requires the string goal: {response}"
        );
        assert_eq!(
            message(&response),
            "params do not satisfy the operation schema: missing field `goal`",
            "the pre-1.0 message is the one serde gave before the field was optional"
        );

        let author = send(
            version,
            "author.query",
            &json!({ "workspace": "ws-1", "goal_atom": marker_baseline() }),
            &[("author.open", json!({ "scope": "saltmarket" }))],
        );
        assert_eq!(code(&author), "schema-violation", "{version}: {author}");
        assert_eq!(message(&author), "missing field `goal`");
    }
}

/// Requiring the retired field did not steal the first word from serde: a
/// request missing several fields still reports the one serde reported, in
/// declaration order, so no `0.x` message moved.
#[test]
fn serde_still_names_the_first_missing_field() {
    for version in pre_1_0() {
        let response = send(version, "dli.query", &json!({}), &[]);
        assert_eq!(code(&response), "schema-violation", "{response}");
        assert_eq!(
            message(&response),
            "params do not satisfy the operation schema: missing field `document`",
            "{version}: the first missing field is still serde's to name"
        );
    }
}

/// A claims array is required element-wise below 1.0. `field_present` (any) is
/// what refuses at 1.0 and `field_carried` (all) is what requires below it; a
/// mixed array is the case that tells them apart, and the strict reading is
/// what the params struct enforced before the field became optional.
#[test]
fn a_partly_typed_claims_array_is_refused_below_1_0() {
    let mixed = json!({
        "document": "saltmarket",
        "source": "the marker holds\n",
        "payload": payload(&json!([
            { "atom": "marker(\"control\")", "quote": "the marker holds", "line": 1 },
            {
                "atom_expr": { "pred": ident("marker"), "args": [{ "Str": "control" }] },
                "quote": "the marker holds",
                "line": 1,
            },
        ])),
    });
    let response = send("0.6.4", "dli.payload-submit", &mixed, &[]);
    assert_eq!(code(&response), "schema-violation", "{response}");
    assert_eq!(
        message(&response),
        "params do not satisfy the operation schema: missing field `atom`"
    );
    // At 1.0 the same array is refused for the opposite reason: one claim still
    // carries the retired spelling.
    let response = send("1.0.0", "dli.payload-submit", &mixed, &[]);
    assert_eq!(code(&response), "killed-form", "{response}");
    assert!(message(&response).contains("atom_expr"), "{response}");
}

/// Only what is REGISTERED is retired. The registry is keyed on the `(op,
/// dotted field path)` pair, so 1.0 refuses nothing by association.
///
/// **Not by name.** `author.why` takes a top-level `atom` string. The registry
/// retires `atom` only as a *claim's* `payload.claims.atom`, so `author.why` is
/// untouched at 1.0. A registry keyed on the field name alone would have broken
/// it; this is the test that would catch that.
///
/// **The remaining gap.** `dli.check-draft`'s runtime-continuity rows —
/// `predicate`/`subject`/`value`, whose typed successors `atom_expr` and
/// `to_atom_expr` arrived at `0.6.4` — were never entered in `KILLED_FIELDS`,
/// so the string spelling survives into 1.0 and is answered. That is registry
/// content, not a mechanism failure, and closing it is a ledger decision rather
/// than a silent edit. Pinned so the choice stays visible.
///
/// (A claim's `atom` was a second such gap until the `0.x` parity oracle showed
/// it was worse than a gap: `Claim` is shared, so leaving `dli.check-draft`
/// unregistered also un-required the field there. Both ops are registered now,
/// and `a_shared_params_type_is_retired_on_every_op_that_carries_it` is what
/// holds them together.)
#[test]
fn nothing_is_refused_by_association_at_1_0() {
    let checked = send(
        "1.0.0",
        "dli.check",
        &json!({ "document": "saltmarket" }),
        &[],
    );
    assert_eq!(checked["ok"], Value::Bool(true), "{checked}");

    // A same-named field on an unregistered (op, path) pair is not retired.
    let why = send(
        "1.0.0",
        "author.why",
        &json!({ "workspace": "ws-1", "atom": "marker(baseline)" }),
        &[("author.open", json!({ "scope": "saltmarket" }))],
    );
    assert_ne!(
        why.pointer("/error/code").and_then(Value::as_str),
        Some("killed-form"),
        "author.why's `atom` is not a claim's atom: {why}"
    );

    // The runtime-continuity strings were never registered, so they survive.
    let continuity = send(
        "1.0.0",
        "dli.check-draft",
        &json!({
            "document": "saltmarket",
            "source": "baseline",
            "payload": payload(&json!([])),
            "continuity": {
                "established": [
                    { "predicate": "marker", "subject": "baseline", "value": "control" }
                ]
            },
        }),
        &[],
    );
    assert_eq!(
        continuity["ok"],
        Value::Bool(true),
        "an unregistered text field is not retired at 1.0: {continuity}"
    );
}

/// A params *type* can be shared by several ops, and a retirement that lands on
/// only one of them is not half-done — it is broken in both directions.
///
/// `Claim` is carried by `dli.payload-submit` and by `dli.check-draft`. Making
/// `atom` optional is what lets a 1.0 caller send `atom_expr` alone, but it is a
/// property of the TYPE, so it un-required the field on both ops at once. With
/// only `dli.payload-submit` registered, `dli.check-draft` got the relaxation
/// and none of the re-imposition: below 1.0 a claim missing `atom` went from
/// refused to accepted, and at 1.0 the retired spelling was still accepted.
/// Both ops are registered, and this asserts the symmetry directly.
#[test]
fn a_shared_params_type_is_retired_on_every_op_that_carries_it() {
    let string_claim = json!([
        { "atom": "marker(\"control\")", "quote": "the marker holds", "line": 1 }
    ]);
    let typed_claim = json!([
        {
            "atom_expr": { "pred": ident("marker"), "args": [{ "Str": "control" }] },
            "quote": "the marker holds",
            "line": 1,
        }
    ]);
    for op in ["dli.payload-submit", "dli.check-draft"] {
        let params = |claims: &Value| {
            json!({
                "document": "saltmarket",
                "source": "the marker holds\n",
                "payload": payload(claims),
            })
        };
        let refused = send("1.0.0", op, &params(&string_claim), &[]);
        assert_eq!(
            code(&refused),
            "killed-form",
            "{op}: a claim's string atom is retired at 1.0: {refused}"
        );
        assert!(message(&refused).contains("atom_expr"), "{op}: {refused}");

        let answered = send("1.0.0", op, &params(&typed_claim), &[]);
        assert_eq!(
            answered["ok"],
            Value::Bool(true),
            "{op}: the typed claim is answered at 1.0: {answered}"
        );

        // ... and below 1.0 the string form is required on BOTH, which is the
        // half that the shared type silently dropped.
        for version in pre_1_0() {
            let missing = send(
                version,
                op,
                &params(&json!([{ "quote": "the marker holds", "line": 1 }])),
                &[],
            );
            assert_eq!(
                code(&missing),
                "schema-violation",
                "{op} at {version}: a claim without `atom` is still refused: {missing}"
            );
            assert_eq!(
                message(&missing),
                "params do not satisfy the operation schema: missing field `atom`",
                "{op} at {version}: with serde's own wording"
            );
        }
    }
}

/// The retired field keeps the PRECEDENCE it had when its struct declared it
/// required. Making it optional moved *when* serde notices it is absent, and
/// without care that reorders which field a malformed request is told about.
///
/// Two directions, and the registry's `preceded_by` is what tells them apart:
///
/// - `Claim` declares `atom` FIRST, before the still-required `quote`. A claim
///   missing both must report `atom` — serde would have, and after the
///   relaxation it fails on `quote` instead unless the retirement pre-empts it.
/// - `QueryParams` declares `document` BEFORE `goal`. A request missing both
///   must still report `document` — here the retirement must NOT pre-empt.
///
/// A nested registration additionally out-ranks the outer struct's missing
/// fields, because a nested decode fails while the outer map is still being
/// visited: a submission missing `document` AND a claim's `atom` reports
/// `atom`.
#[test]
fn the_retired_field_keeps_its_declaration_precedence_below_1_0() {
    let submission = |params: Value| send("0.6.4", "dli.payload-submit", &params, &[]);
    let claims = |claims: Value| {
        json!({
            "document": "saltmarket",
            "source": "the marker holds\n",
            "payload": payload(&claims),
        })
    };
    let missing = |response: &Value| {
        message(response)
            .strip_prefix("params do not satisfy the operation schema: ")
            .unwrap_or_else(|| panic!("a schema violation: {response}"))
            .to_owned()
    };

    // atom-first: the retired field out-ranks a field declared after it.
    assert_eq!(
        missing(&submission(claims(json!([{ "quote": "q", "line": 1 }])))),
        "missing field `atom`"
    );
    assert_eq!(
        missing(&submission(claims(json!([{ "line": 1 }])))),
        "missing field `atom`",
        "`atom` is declared before `quote`, so `atom` is what serde named"
    );
    // ... and yields to a field declared before it, within the claim.
    assert_eq!(
        missing(&submission(claims(json!([
            { "atom": "marker(\"control\")", "line": 1 }
        ])))),
        "missing field `quote`",
        "the retirement has nothing to say when the retired field is present"
    );
    // A nested decode beats the outer struct's end-of-map check.
    assert_eq!(
        missing(&submission(json!({
            "source": "the marker holds\n",
            "payload": payload(&json!([{ "quote": "q", "line": 1 }])),
        }))),
        "missing field `atom`",
        "the claim fails while the outer map is still being visited"
    );

    // document-first: the retirement must NOT steal serde's earlier word.
    let query = |params: Value| send("0.6.4", "dli.query", &params, &[]);
    assert_eq!(
        missing(&query(json!({}))),
        "missing field `document`",
        "`document` is declared before `goal`"
    );
    assert_eq!(
        missing(&query(json!({ "goal_atom": marker_baseline() }))),
        "missing field `document`"
    );
    assert_eq!(
        missing(&query(json!({ "document": "saltmarket" }))),
        "missing field `goal`"
    );
}

/// The retirement yields to every error serde raised while the map was still
/// being visited — an unknown key at the top level, and an unknown key nested
/// inside a typed successor, which the server reports only *after* a successful
/// decode. Both used to be answered by serde's `missing field` on the retired
/// field, and both must still be.
#[test]
fn an_unknown_key_still_out_ranks_the_retired_field_below_1_0() {
    let query = |params: Value| send("0.6.4", "dli.query", &params, &[]);

    // Top-level: `deny_unknown_fields` errors during map iteration, so serde
    // never reached its end-of-map missing-field check.
    let top = query(json!({ "document": "saltmarket", "bogus": 1 }));
    assert_eq!(code(&top), "schema-violation");
    assert!(
        message(&top).contains("bogus"),
        "an unknown key answers first: {top}"
    );

    // Nested inside the typed successor: the decode SUCCEEDS (the retired field
    // now defaults), and the unknown key surfaces from the post-decode
    // normalized round-trip. The retirement must answer before that, because
    // serde's missing-field answered before it.
    let nested = query(json!({
        "document": "saltmarket",
        "goal_atom": {
            "pred": ident("marker"),
            "args": [{ "Sym": ident("baseline") }],
            "bogus": 1,
        },
    }));
    assert_eq!(
        message(&nested),
        "params do not satisfy the operation schema: missing field `goal`",
        "the retired field is still what serde named: {nested}"
    );
}

/// A decode that failed **deeper** is never the retired field's to answer. A
/// malformed typed successor fails on the successor's own required fields —
/// `AtomExpr` wants `pred` and `args`, a `GroundAtom` the same — and that error
/// is raised while the params struct's map is still being visited, so it
/// out-ranked the retired field when the field was required and still does.
///
/// This is the distinction `followed_by` draws that `preceded_by` could not:
/// "the decode failed with SOME missing field" says nothing about which struct
/// the field belongs to.
#[test]
fn a_malformed_typed_successor_answers_for_itself_below_1_0() {
    let missing = |response: &Value| {
        message(response)
            .strip_prefix("params do not satisfy the operation schema: ")
            .unwrap_or_else(|| message(response))
            .to_owned()
    };
    for version in pre_1_0() {
        // `goal` is absent AND the typed successor is malformed. The nested
        // `pred` answers, not the retired `goal`.
        let response = send(
            version,
            "dli.query",
            &json!({ "document": "saltmarket", "goal_atom": { "args": [] } }),
            &[],
        );
        assert_eq!(
            missing(&response),
            "missing field `pred`",
            "{version}: the nested failure answers for itself"
        );

        let response = send(
            version,
            "dli.query",
            &json!({ "document": "saltmarket", "goal_atom": { "pred": "marker" } }),
            &[],
        );
        assert_eq!(missing(&response), "missing field `args`", "{version}");

        // The same for a fork addition's typed ground atom ...
        let response = send(
            version,
            "dli.fork.add",
            &json!({ "fork": "fork-1", "additions": [{ "args": ["x"] }] }),
            &[("dli.fork.new", json!({ "document": "saltmarket" }))],
        );
        assert_eq!(missing(&response), "missing field `pred`", "{version}");

        // ... and for a claim's, on both ops that carry `Claim`.
        for op in ["dli.payload-submit", "dli.check-draft"] {
            let response = send(
                version,
                op,
                &json!({
                    "document": "saltmarket",
                    "source": "the marker holds\n",
                    "payload": payload(&json!([
                        { "atom_expr": { "args": [] }, "quote": "q", "line": 1 }
                    ])),
                }),
                &[],
            );
            assert_eq!(
                missing(&response),
                "missing field `pred`",
                "{op} at {version}"
            );
        }
    }
}

/// The `Ident` widening moved what is ACCEPTED, not what a rejection says. A
/// wrong-typed identifier reports the derive's own wording, because the
/// visitor's `expecting` is the derive's own wording — the widening lives in
/// `visit_str` alone.
#[test]
fn a_wrong_typed_identifier_keeps_the_derives_wording_below_1_0() {
    for version in pre_1_0() {
        let response = send(
            version,
            "dli.query",
            &json!({ "document": "saltmarket", "goal_atom": { "pred": 7, "args": [] } }),
            &[],
        );
        assert_eq!(
            message(&response),
            "params do not satisfy the operation schema: invalid type: integer `7`, \
             expected struct Ident",
            "{version}: the rejection is the one the derive gave"
        );
    }
}

// -- negotiation --------------------------------------------------------------

/// The capability list is gated on the *negotiated version*, never on "is this
/// the newest build", so an older conversation keeps the surface it was promised
/// while a newer one inherits every op. Four tiers, each an exact set:
///
/// - `0.5.1`/`0.6.0` — no `author.branch`/`author.merge`, no corpus ops, no
///   expert authoring.
/// - `0.6.1`..=`1.1.0` — the branch/merge surface, still no corpus ops.
/// - `1.2.0` — that surface plus `dli.corpus.import`/`dli.corpus.export`.
/// - `1.3.0` — that surface plus the DM-4 expert authoring tools.
/// - `1.5.0` — that surface plus durable standing promotion and defeat.
/// - `1.6.0` — that surface plus the `functional` single-valuedness declaration.
/// - `1.7.0` — that surface plus `author.assert_same_as` (WS-1, the
///   identity-closure kernel's wire surface) and `author.add_rule` (WS-2,
///   rule authoring) — the wire-surface-completion campaign's two appended
///   ops, in that order.
/// - `1.8.0` — that surface plus `author.declare_inertial` (the inertia
///   declaration, DL-0081).
/// - `1.9.0` — that surface plus `author.as_of` (the inertia-aware point
///   read, DL-0082) and `author.record_decision` (the baseline-decision
///   recorder, DL-0083).
///
/// Asserting the *differences* rather than a frozen list keeps this honest as
/// ops are added: a new op must land in a tier deliberately, and an op leaking
/// into an older tier — the C4 violation this guards — fails here.
///
/// **Each tier is a floor**, expressed here as `version >= introduced_at`, and
/// that is the whole point of the rewrite this test got in DM-4. Its previous
/// shape asked `version != "1.2.0"` to decide whether the corpus ops belonged,
/// which mirrored the exact-match gate in `capabilities()` — so when `1.3.0`
/// joined the accepted set, both would have agreed that a `1.3.0` caller gets no
/// corpus ops, and this test would have passed while DM-3's surface silently
/// vanished from the newest wire. A test that reproduces the implementation's
/// mistake cannot catch it; a floor stated independently can.
#[test]
fn a_1_0_handshake_gets_the_full_capability_surface() {
    let (_temp, mut sessions, mut store) = fixture_project();
    // Each tier: the ops it introduced, and the version that introduced them.
    let tiers: &[(&str, &[&str])] = &[
        ("0.6.1", &["author.branch", "author.merge"]),
        ("1.2.0", &["dli.corpus.import", "dli.corpus.export"]),
        (
            "1.3.0",
            &[
                "author.add_dialect",
                "author.add_name_part",
                "author.add_name",
                "author.add_term",
                "author.add_banned",
                "author.add_state",
                "author.add_attest",
            ],
        ),
        ("1.5.0", &["author.set_standing", "author.defeat"]),
        ("1.6.0", &["author.declare_functional"]),
        ("1.7.0", &["author.assert_same_as", "author.add_rule"]),
        ("1.8.0", &["author.declare_inertial"]),
        ("1.9.0", &["author.as_of", "author.record_decision"]),
        ("1.10.0", &["author.predicates", "author.entity_candidates"]),
    ];

    let surface = |version: &str, sessions: &mut _, store: &mut _| -> Vec<String> {
        let response = drive(
            &json!({ "dli": version, "id": "v", "op": "dli.version", "params": {} }),
            sessions,
            store,
        );
        assert_eq!(
            response["result"]["contract_version"], *version,
            "the handshake reports the version this conversation speaks: {response}"
        );
        let caps: Vec<String> = response["result"]["capabilities"]
            .as_array()
            .unwrap_or_else(|| panic!("{version} advertises capabilities: {response}"))
            .iter()
            .map(|value| value.as_str().unwrap_or_default().to_owned())
            .collect();
        assert!(!caps.is_empty(), "{version} advertises capabilities");
        caps
    };

    // Version ordering, spelled out here rather than imported, so the test does
    // not inherit whatever comparison the server used to build the answer.
    let key = |version: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = version.split('.').map(|p| p.parse().unwrap()).collect();
        (parts[0], parts[1], parts[2])
    };

    // The newest accepted version is the reference surface: it must carry every
    // tier's ops, or a later tier has quietly displaced an earlier one.
    let newest = dli_serve::ACCEPTED_CONTRACT_VERSIONS
        .iter()
        .max_by_key(|version| key(version))
        .expect("at least one accepted version");
    let full = surface(newest, &mut sessions, &mut store);
    for (introduced_at, ops) in tiers {
        for op in *ops {
            assert!(
                full.contains(&(*op).to_owned()),
                "{newest} advertises `{op}` (introduced at {introduced_at}): {full:?}"
            );
        }
    }

    for version in dli_serve::ACCEPTED_CONTRACT_VERSIONS {
        let caps = surface(version, &mut sessions, &mut store);
        let mut expected = full.clone();
        for (introduced_at, ops) in tiers {
            if key(version) < key(introduced_at) {
                expected.retain(|op| !ops.contains(&op.as_str()));
            }
        }
        assert_eq!(
            caps, expected,
            "{version} advertises exactly its tier's surface"
        );
    }
}

/// The version gates are floors, and a floor is what keeps a released surface
/// from evaporating at the next bump.
///
/// This asserts the property directly, one predicate at a time, so it fails at
/// the gate itself rather than only through the handshake: every accepted
/// version at or above a tier's introduction answers `true`, and every version
/// below answers `false`. An exact-match gate (`matches!(dli, "1.2.0")`) passes
/// the second half and fails the first the moment a newer version exists.
/// One version gate under test: its name, the version that introduced it, and
/// the predicate the server asks.
type Gate = (&'static str, &'static str, fn(&str) -> bool);

#[test]
fn every_capability_gate_is_a_floor_not_an_exact_match() {
    let key = |version: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = version.split('.').map(|p| p.parse().unwrap()).collect();
        (parts[0], parts[1], parts[2])
    };
    let gates: &[Gate] = &[
        ("branch/merge", "0.6.1", dli_serve::has_branch_merge),
        ("corpus ops", "1.2.0", dli_serve::has_corpus_ops),
        ("canon export", "1.4.0", dli_serve::has_canon_export),
        ("expert authoring", "1.3.0", dli_serve::has_expert_authoring),
        ("standing tools", "1.5.0", dli_serve::has_standing_tools),
        ("functional decls", "1.6.0", dli_serve::has_functional_decls),
        ("citation recording", "1.3.0", dli_serve::records_citations),
        (
            "identity assertions",
            "1.7.0",
            dli_serve::has_identity_assertions,
        ),
        ("rule authoring", "1.7.0", dli_serve::has_rule_authoring),
        ("branch writes", "1.7.0", dli_serve::has_branch_writes),
    ];
    for (name, floor, gate) in gates {
        let mut at_or_above = 0;
        for version in dli_serve::ACCEPTED_CONTRACT_VERSIONS {
            let expected = key(version) >= key(floor);
            assert_eq!(
                gate(version),
                expected,
                "the {name} gate must answer {expected} for {version} (floor {floor})"
            );
            at_or_above += usize::from(expected);
        }
        // Non-vacuous in both directions: a gate that answered a constant would
        // satisfy the loop above only if every accepted version fell on one side.
        assert!(
            at_or_above > 0,
            "the {name} gate never fires — no accepted version reaches {floor}"
        );
        assert!(
            at_or_above < dli_serve::ACCEPTED_CONTRACT_VERSIONS.len(),
            "the {name} gate always fires — it gates nothing"
        );
    }
}

/// `1.0.0` is accepted, and an unspoken version is still refused with
/// `version-mismatch` — the new major widened the accepted set, it did not
/// open it.
#[test]
fn an_unspoken_version_is_still_refused() {
    let (_temp, mut sessions, mut store) = fixture_project();
    let response = drive(
        &json!({ "dli": "9.9.9", "id": "1", "op": "dli.version", "params": {} }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(code(&response), "version-mismatch", "{response}");
    assert!(
        message(&response).contains("1.0.0"),
        "the refusal lists 1.0.0 among the versions this worker speaks: {response}"
    );
}
