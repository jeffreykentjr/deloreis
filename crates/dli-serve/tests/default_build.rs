//! The 1.0-native worker: what the **default** build serves, and how it refuses
//! the rest (roadmap §LS-7).
//!
//! The LS-7 oracle is a build property — "the default build does not link
//! `dli_syntax`" — proven by `cargo xtask syntax-free`. This file proves the
//! consequence: that the build the oracle describes is *a working server*, not a
//! hollowed-out one, and that where it cannot serve a request it says so by
//! name.
//!
//! Every test here runs in **both** lanes and asserts the truth of the lane it
//! is in. That is the point: the two builds are one contract with one
//! feature-flagged edge, so a test that only ran in one lane would prove half of
//! it. `cfg!(feature = "syntax")` is the fork, and it appears only where the two
//! builds genuinely differ:
//!
//! | request | default build | `--features syntax` |
//! |---|---|---|
//! | `dli.query` with `goal_atom` | answered | answered |
//! | `dli.query` with string `goal` | `schema-violation` naming `goal_atom` | answered |
//! | `dli.fork.add` with `additions` | applied | applied |
//! | `dli.fork.add` with `declarations` | `schema-violation` naming `additions` | applied |
//! | `dli.check-draft` with a typed `continuity` row | checked | checked |
//! | `dli.check-draft` with a string `continuity` row | `payload.runtime-continuity-invalid` | checked |
//! | `author.*` mutation | staged; `rendered` empty | staged; `rendered` is the `.dli` line |
//! | `author.diff` rows | present | present |
//! | `author.materialize`, no lorestore | refused by name | the rendering IS the materialization |
//! | `.dli` project load | refused, naming the two ways forward | loaded |
//!
//! The demo corpora reach this build from committed fact objects, so the whole
//! read surface below is served without a parser in the process.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_serve::author::store::Store;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use serde_json::{Value, json};

/// Whether this build links the `.dli` front end.
const SYNTAX: bool = cfg!(feature = "syntax");

fn demo() -> (Sessions, Store) {
    let pack = dli_serve::session::builtin_pack("camelot").expect("the camelot demo is bundled");
    let store = Store::with_pack(pack).expect("the demo pack compiles");
    (Sessions::new(), store)
}

fn drive(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let line = serde_json::to_string(frame).unwrap();
    let response = respond(&line, sessions, store).expect("a response for every request frame");
    serde_json::to_value(&response).unwrap()
}

/// An `Ident` on the wire: the text plus the synthetic span every typed field
/// carries.
fn ident(text: &str) -> Value {
    json!({ "text": text, "span": { "file": 0, "start": 0, "end": 0 } })
}

fn request(id: &str, op: &str, params: &Value) -> Value {
    json!({
        "dli": "0.6.4",
        "id": id,
        "op": op,
        "params": params,
    })
}

/// The error body of a refused response, or a panic naming what came back
/// instead.
fn error(response: &Value) -> &Value {
    response
        .get("error")
        .unwrap_or_else(|| panic!("expected a refusal, got {response}"))
}

fn result(response: &Value) -> &Value {
    response
        .get("result")
        .unwrap_or_else(|| panic!("expected a result, got {response}"))
}

// -- the read surface, served from committed fact objects ---------------------

#[test]
fn the_demo_corpora_reconstruct_without_a_parser() {
    for document in dli_serve::demo::DEMO_DOCUMENTS {
        let pack = dli_serve::session::builtin_pack(document)
            .unwrap_or_else(|| panic!("demo `{document}` is bundled"));
        assert!(
            !pack.documents.is_empty(),
            "demo `{document}` reconstructed to an empty pack"
        );
    }
}

#[test]
fn a_typed_goal_is_answered() {
    let (mut sessions, mut store) = demo();
    let response = drive(
        &request(
            "q-typed",
            "dli.query",
            &json!({
                "document": "camelot",
                "goal": "",
                "goal_atom": {
                    "pred": ident("adjacent"),
                    "args": [{ "Sym": ident("camelot") }, { "Var": ident("to") }],
                },
            }),
        ),
        &mut sessions,
        &mut store,
    );
    let rows = result(&response)
        .get("rows")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("a typed query answers with rows: {response}"));
    assert!(
        !rows.is_empty(),
        "`adjacent(camelot, ?to)` over the camelot demo matches: {response}"
    );
}

#[test]
fn a_string_goal_needs_the_front_end() {
    let (mut sessions, mut store) = demo();
    let response = drive(
        &request(
            "q-string",
            "dli.query",
            &json!({ "document": "camelot", "goal": "adjacent(camelot, ?to)" }),
        ),
        &mut sessions,
        &mut store,
    );
    if SYNTAX {
        assert!(
            result(&response).get("rows").is_some(),
            "a syntax build still answers the string form: {response}"
        );
        return;
    }
    let message = error(&response)["message"].as_str().unwrap().to_owned();
    assert!(
        message.contains("goal_atom"),
        "the refusal must name the typed successor, got: {message}"
    );
    assert_eq!(error(&response)["code"], json!("schema-violation"));
}

// -- check-draft: the typed runtime-continuity row vs. the three strings ------

/// The `payload/v1` envelope every `dli.check-draft` below shares: an empty
/// payload over a one-word draft, so the only findings are the ones the
/// `continuity` context produces.
fn empty_payload() -> Value {
    json!({
        "meta": {
            "episode": "episode-1",
            "source": {
                "file": "draft.md",
                "sha256": "8ba8496a2525ae171ffd104d632dede6ef418d9b95962a9d88e2fcdbc8d48d24",
                "words": 1,
            },
            "extracted": "2026-07-25",
            "extractors": { "schema": "payload/v1" },
        },
        "claims": [],
        "deltas": [],
        "scenes": [],
        "beats": [],
        "promises": { "opened": [], "closed": [] },
        "proof_ledgers": [],
        "names": { "introduced": [] },
        "lexicon": { "banned_hits": [], "register_tokens": 0 },
    })
}

/// The finding codes a `dli.check-draft` response carries, in response order.
fn codes(response: &Value) -> Vec<String> {
    result(response)["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("check-draft answers with findings: {response}"))
        .iter()
        .map(|finding| finding["code"].as_str().unwrap().to_owned())
        .collect()
}

fn check_draft(id: &str, continuity: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    drive(
        &request(
            id,
            "dli.check-draft",
            &json!({
                "document": "camelot",
                "source": "baseline",
                "payload": empty_payload(),
                "continuity": continuity,
            }),
        ),
        sessions,
        store,
    )
}

/// `adjacent(camelot, excalibur)` as a typed atom. Canon ratifies
/// `adjacent(camelot, the-lake)`, so this atom is *contradicted* — which is what
/// makes it a proof rather than a smoke test: a build that ignored the typed
/// field would report the row unreadable, not wrong.
fn contradicting_atom() -> Value {
    json!({
        "pred": ident("adjacent"),
        "args": [{ "Sym": ident("camelot") }, { "Sym": ident("excalibur") }],
    })
}

#[test]
fn a_typed_runtime_continuity_fact_is_checked() {
    let (mut sessions, mut store) = demo();
    let response = check_draft(
        "cd-typed",
        &json!({
            "established": [{
                "predicate": "adjacent",
                "subject": "camelot",
                "value": "excalibur",
                "atom_expr": contradicting_atom(),
            }],
        }),
        &mut sessions,
        &mut store,
    );
    let codes = codes(&response);
    assert!(
        !codes
            .iter()
            .any(|code| code == "payload.runtime-continuity-invalid"),
        "the typed row needs no front end, so no build can call it unreadable: {response}"
    );
    assert!(
        codes
            .iter()
            .any(|code| code == "continuity.world-contradiction"),
        "the typed atom reached the fact-checker and lost to canon: {response}"
    );
}

#[test]
fn a_typed_runtime_continuity_change_is_checked() {
    let (mut sessions, mut store) = demo();
    let response = check_draft(
        "cd-typed-change",
        &json!({
            "changes": [{
                "predicate": "adjacent",
                "subject": "camelot",
                "from_value": "the-lake",
                "to_value": "excalibur",
                "to_atom_expr": contradicting_atom(),
            }],
        }),
        &mut sessions,
        &mut store,
    );
    let codes = codes(&response);
    assert!(
        !codes
            .iter()
            .any(|code| code == "payload.runtime-continuity-invalid"),
        "a change carries its target atom typed too: {response}"
    );
    assert!(
        codes
            .iter()
            .any(|code| code == "continuity.world-contradiction"),
        "the typed target atom reached the fact-checker: {response}"
    );
}

#[test]
fn a_string_runtime_continuity_fact_needs_the_front_end() {
    let (mut sessions, mut store) = demo();
    let response = check_draft(
        "cd-string",
        &json!({
            "established": [{
                "predicate": "adjacent",
                "subject": "camelot",
                "value": "excalibur",
            }],
        }),
        &mut sessions,
        &mut store,
    );
    let codes = codes(&response);
    if SYNTAX {
        assert!(
            codes
                .iter()
                .any(|code| code == "continuity.world-contradiction"),
            "a syntax build still assembles and parses the three strings: {response}"
        );
        return;
    }
    // The whole point of the typed successor: without it, the row is text this
    // build has no parser for, and it says so instead of checking nothing.
    assert_eq!(
        codes,
        vec!["payload.runtime-continuity-invalid"],
        "the untyped row is unreadable on a parser-free build: {response}"
    );
    let message = result(&response)["findings"][0]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        message.contains("atom_expr"),
        "the finding must name the typed successor, got: {message}"
    );
}

// -- forks: the typed additions vs. the text declarations ---------------------

fn open_fork(sessions: &mut Sessions, store: &mut Store) -> String {
    let response = drive(
        &request(
            "fork-new",
            "dli.fork.new",
            &json!({ "document": "camelot" }),
        ),
        sessions,
        store,
    );
    result(&response)["fork"].as_str().unwrap().to_owned()
}

#[test]
fn typed_additions_apply() {
    let (mut sessions, mut store) = demo();
    let fork = open_fork(&mut sessions, &mut store);
    let response = drive(
        &request(
            "fork-add-typed",
            "dli.fork.add",
            &json!({
                "fork": fork,
                "declarations": [],
                // A `GroundAtom` — the typed successor's shape: a predicate and
                // its already-ground argument renderings, no parse required.
                "additions": [{ "pred": "adjacent", "args": ["camelot", "lyonesse"] }],
            }),
        ),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        result(&response)["ok"],
        json!(true),
        "typed additions apply on every build: {response}"
    );
}

#[test]
fn text_declarations_need_the_front_end() {
    let (mut sessions, mut store) = demo();
    let fork = open_fork(&mut sessions, &mut store);
    let response = drive(
        &request(
            "fork-add-text",
            "dli.fork.add",
            &json!({ "fork": fork, "declarations": ["adjacent(camelot, lyonesse) proposed"] }),
        ),
        &mut sessions,
        &mut store,
    );
    if SYNTAX {
        assert_eq!(
            result(&response)["ok"],
            json!(true),
            "a syntax build still lowers the text form: {response}"
        );
        return;
    }
    let message = error(&response)["message"].as_str().unwrap().to_owned();
    assert!(
        message.contains("additions"),
        "the refusal must name the typed successor, got: {message}"
    );
    assert_eq!(error(&response)["code"], json!("schema-violation"));
}

// -- authoring: JSON-first, with `.dli` rendering as the opt-in ---------------

#[test]
fn authoring_stages_and_diffs_without_a_renderer() {
    let (mut sessions, mut store) = demo();
    let opened = drive(
        &request("ws-open", "author.open", &json!({ "scope": "camelot" })),
        &mut sessions,
        &mut store,
    );
    let workspace = result(&opened)["workspace"].as_str().unwrap().to_owned();

    // A relation moves an atom's standing, so it shows up as a structured row —
    // the review surface that needs no formatter.
    let added = drive(
        &request(
            "ws-add",
            "author.add_relation",
            &json!({
                "workspace": workspace,
                "relation": "adjacent",
                "from": "camelot",
                "to": "lyonesse",
                "provenance": { "kind": "proposed", "origin": "ls7-default-build" },
            }),
        ),
        &mut sessions,
        &mut store,
    );
    let rendered = result(&added)["rendered"].as_array().unwrap();
    if SYNTAX {
        assert_eq!(
            rendered,
            &vec![json!(
                "adjacent(camelot, lyonesse) proposed ls7-default-build"
            )],
            "a syntax build renders the canonical line: {added}"
        );
    } else {
        assert!(
            rendered.is_empty(),
            "the default build renders no `.dli`; the fact objects are the surface: {added}"
        );
    }

    // The LS-5 review surface is renderer-free on both builds: the structured
    // rows come from the standing lattice, never from a formatter.
    let diff = drive(
        &request("ws-diff", "author.diff", &json!({ "workspace": workspace })),
        &mut sessions,
        &mut store,
    );
    let rows = result(&diff)
        .get("rows")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("0.6.x carries structured diff rows: {diff}"));
    assert!(
        rows.iter().any(|row| row["atom"]
            .as_str()
            .is_some_and(|atom| atom.contains("lyonesse"))),
        "the staged relation shows up as a structured row: {diff}"
    );
    assert!(
        rows.iter()
            .all(|row| row.get("rendered").is_some_and(Value::is_string)),
        "every row renders itself from the atom, with no formatter: {diff}"
    );
}

#[test]
fn per_declaration_discard_is_refused_without_a_renderer() {
    if SYNTAX {
        // The rendered-line key exists on this build; `author.rs` covers it.
        return;
    }
    let (mut sessions, mut store) = demo();
    let opened = drive(
        &request("ws-open", "author.open", &json!({ "scope": "camelot" })),
        &mut sessions,
        &mut store,
    );
    let workspace = result(&opened)["workspace"].as_str().unwrap().to_owned();
    drive(
        &request(
            "ws-add",
            "author.add_entity",
            &json!({
                "workspace": workspace,
                "kind": "person",
                "id": "elaine",
                "name": "Elaine",
                "provenance": { "kind": "proposed", "origin": "ls7-default-build" },
            }),
        ),
        &mut sessions,
        &mut store,
    );
    let discarded = drive(
        &request(
            "ws-discard",
            "author.discard",
            &json!({ "workspace": workspace, "decl": "person elaine \"Elaine\"" }),
        ),
        &mut sessions,
        &mut store,
    );
    let message = error(&discarded)["message"].as_str().unwrap().to_owned();
    assert!(
        message.contains("rendered") && message.contains("syntax"),
        "the refusal must say the line cannot be named on this build, got: {message}"
    );
    // The workspace is untouched — a refusal is never a partial discard.
    let status = drive(
        &request(
            "ws-status",
            "author.status",
            &json!({ "workspace": workspace }),
        ),
        &mut sessions,
        &mut store,
    );
    assert!(
        result(&status)["findings"].is_array(),
        "the workspace is still open after the refusal: {status}"
    );
}

/// A demo-mode workspace (no lorestore) with one staged entity, and the
/// `author.materialize` response for it.
fn materialize_a_demo_workspace(sessions: &mut Sessions, store: &mut Store) -> Value {
    let opened = drive(
        &request("ws-open", "author.open", &json!({ "scope": "camelot" })),
        sessions,
        store,
    );
    let workspace = result(&opened)["workspace"].as_str().unwrap().to_owned();
    drive(
        &request(
            "ws-add",
            "author.add_entity",
            &json!({
                "workspace": workspace,
                "kind": "person",
                "id": "elaine",
                "name": "Elaine",
                "provenance": { "kind": "proposed", "origin": "ls7-default-build" },
            }),
        ),
        sessions,
        store,
    );
    drive(
        &request(
            "ws-materialize",
            "author.materialize",
            &json!({ "workspace": workspace }),
        ),
        sessions,
        store,
    )
}

#[test]
fn materialize_without_a_lorestore_is_refused_without_a_renderer() {
    let (mut sessions, mut store) = demo();
    let response = materialize_a_demo_workspace(&mut sessions, &mut store);
    if SYNTAX {
        // Demo mode: the rendering IS the materialization, and that is what the
        // 0.5.1/0.6.x author fixtures replay byte-for-byte.
        assert!(
            !result(&response)["rendered"].as_array().unwrap().is_empty(),
            "a syntax build renders the staged block: {response}"
        );
        return;
    }
    // A default build has neither half — nothing to commit to and nothing to
    // render — so an `ok` with an empty `findings` list would read to the author
    // as "materialized clean" when nothing was written anywhere.
    let message = error(&response)["message"].as_str().unwrap().to_owned();
    assert!(
        message.contains("lorestore") && message.contains("syntax"),
        "the refusal must name both ways forward, got: {message}"
    );
    assert_eq!(error(&response)["code"], json!("schema-violation"));
}

// -- contract 1.0: the field retirement, in both lanes ------------------------

/// LS-8 turns the LS-7 field registry live at contract `1.0.0`, and the switch
/// is a property of the *negotiated version*, not of the build: a string `goal`
/// is `killed-form` at 1.0 whether or not this build links a parser. Refusing
/// it for want of a front end (the LS-7 refusal above) and refusing it because
/// the contract retired it are different verdicts, and the second one outranks
/// the first — a caller who upgrades a parser-linked build must still be told
/// the form is gone.
#[test]
fn a_string_goal_is_a_killed_form_at_contract_1_0() {
    let (mut sessions, mut store) = demo();
    let response = drive(
        &json!({
            "dli": "1.0.0",
            "id": "q-1-0",
            "op": "dli.query",
            "params": { "document": "camelot", "goal": "adjacent(camelot, ?to)" },
        }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        error(&response)["code"],
        json!("killed-form"),
        "1.0 retires the string form in both lanes: {response}"
    );
    let message = error(&response)["message"].as_str().unwrap();
    assert!(
        message.contains("goal_atom") && message.contains("1.0.0"),
        "the refusal names the successor and the wire, got: {message}"
    );
}

/// The same request one version down is not a killed form. What it *is* differs
/// by lane — answered with the parser, refused for want of one without it — and
/// neither is the retirement, which is the point: LS-8 added a verdict at 1.0
/// and moved nothing below it.
#[test]
fn the_same_request_is_not_a_killed_form_at_0_6_4() {
    let (mut sessions, mut store) = demo();
    let response = drive(
        &json!({
            "dli": "0.6.4",
            "id": "q-0-6-4",
            "op": "dli.query",
            "params": { "document": "camelot", "goal": "adjacent(camelot, ?to)" },
        }),
        &mut sessions,
        &mut store,
    );
    if SYNTAX {
        assert!(
            result(&response).get("rows").is_some(),
            "0.6.4 still answers the string form: {response}"
        );
        return;
    }
    assert_eq!(error(&response)["code"], json!("schema-violation"));
    assert!(
        error(&response)["message"]
            .as_str()
            .unwrap()
            .contains("goal_atom"),
        "the parser-free refusal is LS-7's, not the retirement: {response}"
    );
}

/// At 1.0 the typed successor is sent ALONE — the retired field is no longer
/// required, which is what makes the retirement usable rather than a dead end.
/// This build has no parser, so answering it also proves the typed path never
/// touches one.
#[test]
fn a_typed_goal_is_answered_at_contract_1_0() {
    let (mut sessions, mut store) = demo();
    let response = drive(
        &json!({
            "dli": "1.0.0",
            "id": "q-typed-1-0",
            "op": "dli.query",
            "params": {
                "document": "camelot",
                "goal_atom": {
                    "pred": ident("adjacent"),
                    "args": [{ "Sym": ident("camelot") }, { "Var": ident("to") }],
                },
            },
        }),
        &mut sessions,
        &mut store,
    );
    let rows = result(&response)
        .get("rows")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("a 1.0 typed query answers with rows: {response}"));
    assert!(!rows.is_empty(), "{response}");
}

// -- projects -----------------------------------------------------------------

#[test]
fn a_dli_project_load_names_its_two_ways_forward() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/dli-worker/fixture-project");
    let loaded = dli_serve::project::Project::load(&source);
    if SYNTAX {
        assert!(
            loaded.is_ok(),
            "a syntax build loads the `.dli` fixture project: {:?}",
            loaded.err()
        );
        return;
    }
    let message = loaded
        .expect_err("a parser-free build cannot read `.dli`")
        .to_string();
    assert!(
        message.contains("syntax") && message.contains("fact objects"),
        "the refusal must name both ways forward, got: {message}"
    );
}
