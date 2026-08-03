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
//! | `author.*` mutation | staged; `rendered` empty | staged; `rendered` is the `.dli` line |
//! | `author.diff` rows | present | present |
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
        "dli": "0.6.3",
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
