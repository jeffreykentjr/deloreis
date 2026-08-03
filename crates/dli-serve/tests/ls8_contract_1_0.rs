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
/// separately: Door T is a different decode seam behind a different branch, and
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
/// dotted field path)` pair and 1.0 refuses nothing by association — which is
/// two claims, one a safety property and one an honest gap, and both are pinned
/// here rather than left to be discovered.
///
/// **Not by name.** `author.why` takes a top-level `atom` string. The registry
/// retires `atom` only as `dli.payload-submit`'s `payload.claims.atom`, so
/// `author.why` is untouched at 1.0. A registry keyed on the field name alone
/// would have broken it; this is the test that would catch that.
///
/// **The gaps.** Two text-on-wire spellings that LS-7 gave typed successors
/// were never entered in `KILLED_FIELDS`, so they survive into 1.0 and are
/// answered: `dli.check-draft`'s runtime-continuity `predicate`/`subject`/
/// `value` rows (successors `atom_expr`/`to_atom_expr`, contract `0.6.4`), and
/// a claim's `atom` when it arrives on `dli.check-draft` rather than on
/// `dli.payload-submit` — the same payload shape, retired on one op and not the
/// other. Neither is a mechanism failure; both are registry content, and
/// closing them is a ledger decision, not a silent edit. Pinned so the choice
/// stays visible.
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
        "author.why's `atom` is not dli.payload-submit's claim atom: {why}"
    );

    // Gap 1: the runtime-continuity strings were never registered.
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

    // Gap 2: the same claim shape is retired on dli.payload-submit and not on
    // dli.check-draft, because only the former carries a registry entry.
    let draft_claim = send(
        "1.0.0",
        "dli.check-draft",
        &json!({
            "document": "saltmarket",
            "source": "the marker holds\n",
            "payload": payload(&json!([
                { "atom": "marker(\"control\")", "quote": "the marker holds", "line": 1 }
            ])),
        }),
        &[],
    );
    assert_ne!(
        draft_claim.pointer("/error/code").and_then(Value::as_str),
        Some("killed-form"),
        "dli.check-draft has no registry entry, so its claim atom is not retired: {draft_claim}"
    );
}

// -- negotiation --------------------------------------------------------------

/// A 1.0 handshake gets the whole surface, and an older one is unchanged. The
/// capability list is gated on the *negotiated version*, never on "is this the
/// newest build", so a new major inherits every op instead of resetting the
/// list to what the build happens to be.
#[test]
fn a_1_0_handshake_gets_the_full_capability_surface() {
    let (_temp, mut sessions, mut store) = fixture_project();
    let mut newest: Option<Value> = None;
    for version in dli_serve::ACCEPTED_CONTRACT_VERSIONS {
        let response = drive(
            &json!({ "dli": version, "id": "v", "op": "dli.version", "params": {} }),
            &mut sessions,
            &mut store,
        );
        assert_eq!(
            response["result"]["contract_version"], **version,
            "the handshake reports the version this conversation speaks: {response}"
        );
        let caps = response["result"]["capabilities"].clone();
        assert!(
            caps.as_array().is_some_and(|caps| !caps.is_empty()),
            "{version} advertises capabilities: {response}"
        );
        if matches!(*version, "0.5.1" | "0.6.0") {
            continue;
        }
        match &newest {
            None => newest = Some(caps),
            Some(expected) => assert_eq!(
                &caps, expected,
                "{version} must advertise the same surface as every 0.6.1+ version"
            ),
        }
    }
    assert!(newest.is_some(), "a 0.6.1+ version was negotiated");
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
