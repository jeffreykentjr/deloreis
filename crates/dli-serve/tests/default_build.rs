//! The parser-free worker: what the **no-default-features** build serves, and how it refuses
//! the rest (roadmap §LS-7).
//!
//! The LS-7 oracle is a build property — "the build without default features does not link
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
//! | request | `--no-default-features` | default build |
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
        "dli": "2.0.0",
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

/// The finding-message escape is on the `Finding` TYPE, so it covers the
/// `dli.*` surface exactly as it covers `author.*` — the boundary is the whole
/// class, not one channel. A `dli.name.check` candidate mixes a Cyrillic
/// homoglyph (which trips `name.confusable`, embedding the raw candidate in the
/// message) with a newline; the finding that returns over the wire must carry
/// the newline as its escape, never a raw split that forges a review line
/// (DL-0061). Runs in both build lanes — `name.check`'s confusable screen needs
/// no parser.
#[test]
fn dli_name_check_finding_message_escapes_control_chars() {
    let (mut sessions, mut store) = demo();
    // Cyrillic 'а' (U+0430) among Latin letters trips the confusable screen; the
    // embedded newline is the forgery the message escape must neutralize.
    let name = "Mar\u{0430}\nx";
    let response = drive(
        &request(
            "1",
            "dli.name.check",
            &json!({ "name": name, "dialect": "" }),
        ),
        &mut sessions,
        &mut store,
    );
    let findings = result(&response)["findings"]
        .as_array()
        .expect("name.check answers with findings");
    let confusable = findings
        .iter()
        .find(|finding| finding["code"] == json!("name.confusable"))
        .expect("the Cyrillic homoglyph raises name.confusable");
    let message = confusable["message"].as_str().expect("a message");
    assert!(
        !message.contains('\n'),
        "a raw newline in a dli.* finding message forges a review line: {message:?}"
    );
    assert!(
        message.contains("\\n"),
        "the caller's newline must survive as its escape: {message:?}"
    );
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
    assert_eq!(error(&response)["code"], json!("killed-form"));
    assert!(
        error(&response)["message"]
            .as_str()
            .unwrap()
            .contains("additions")
    );
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
            "the parser-free build renders no `.dli`; the fact objects are the surface: {added}"
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

/// A rendered line is not a structural key, so rejecting it must leave the
/// staged declaration in place on either build.
#[test]
fn per_declaration_discard_rejects_a_rendered_line() {
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
        message.contains("no staged declaration matching"),
        "the refusal must name the missing structural key, got: {message}"
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
    // A parser-free build has neither half — nothing to commit to and nothing to
    // render — so an `ok` with an empty `findings` list would read to the author
    // as "materialized clean" when nothing was written anywhere.
    let message = error(&response)["message"].as_str().unwrap().to_owned();
    assert!(
        message.contains("lorestore") && message.contains("syntax"),
        "the refusal must name both ways forward, got: {message}"
    );
    assert_eq!(error(&response)["code"], json!("schema-violation"));
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

/// From contract 1.3.0, a staged declaration carries a **structural key** — the
/// JCS digest of the declaration itself — which `author.status` lists and
/// `author.discard` accepts. This closes the deferral DM-3 recorded: the key
/// existed internally, but nothing surfaced it, so the per-declaration form had
/// to refuse here rather than match nothing.
///
/// The whole point is that this runs in the default lane. The key is derived
/// from the declaration's canonical bytes rather than from a rendering, so it
/// exists on a build that links no formatter — which is the build the consumer's
/// authoring server is. Rejecting one declaration out of several, without
/// discarding the session, was simply not available there before.
// A single end-to-end walk, asserted step by step: splitting it into helpers
// would hide the sequence that is the thing under test.
#[allow(clippy::too_many_lines)]
#[test]
fn per_declaration_discard_names_the_structural_key_from_1_3_0() {
    let (mut sessions, mut store) = demo();
    let at_1_3_0 = |id: &str, op: &str, params: &Value| json!({ "dli": "2.0.0", "id": id, "op": op, "params": params });
    let opened = drive(
        &at_1_3_0("ws-open", "author.open", &json!({ "scope": "camelot" })),
        &mut sessions,
        &mut store,
    );
    let workspace = result(&opened)["workspace"].as_str().unwrap().to_owned();
    for (id, name) in [("elaine", "Elaine"), ("bors", "Bors")] {
        drive(
            &at_1_3_0(
                "ws-add",
                "author.add_entity",
                &json!({
                    "workspace": workspace,
                    "kind": "person",
                    "id": id,
                    "name": name,
                    "provenance": { "kind": "proposed", "origin": "dm4-default-build" },
                }),
            ),
            &mut sessions,
            &mut store,
        );
    }

    let status = drive(
        &at_1_3_0(
            "ws-status",
            "author.status",
            &json!({ "workspace": workspace }),
        ),
        &mut sessions,
        &mut store,
    );
    let staged = result(&status)["staged"]
        .as_array()
        .unwrap_or_else(|| panic!("1.3.0 status lists the staged declarations: {status}"))
        .clone();
    assert_eq!(staged.len(), 2, "both declarations are listed: {status}");
    for row in &staged {
        let key = row["key"].as_str().expect("every row carries a key");
        assert!(
            key.starts_with("jcs:sha256:"),
            "the key is a JCS digest, not a rendering: {key}"
        );
        assert_eq!(row["kind"], "person", "the row names its kind: {row}");
        if !SYNTAX {
            assert!(
                row.get("rendered").is_none(),
                "a build with no formatter offers no rendered line: {row}"
            );
        }
    }
    // Two different declarations must not collide, or the key names a group
    // rather than a declaration and `discard` would drop the wrong one.
    assert_ne!(
        staged[0]["key"], staged[1]["key"],
        "distinct declarations take distinct keys: {status}"
    );

    let key = staged[0]["key"].as_str().unwrap().to_owned();
    let discarded = drive(
        &at_1_3_0(
            "ws-discard",
            "author.discard",
            &json!({ "workspace": workspace, "decl": key }),
        ),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        result(&discarded)["discarded"],
        json!(true),
        "the keyed declaration is dropped: {discarded}"
    );

    let after = drive(
        &at_1_3_0(
            "ws-after",
            "author.status",
            &json!({ "workspace": workspace }),
        ),
        &mut sessions,
        &mut store,
    );
    let left = result(&after)["staged"].as_array().unwrap().clone();
    assert_eq!(
        left.len(),
        1,
        "exactly one declaration was removed: {after}"
    );
    assert_eq!(
        left[0]["key"], staged[1]["key"],
        "the OTHER declaration is the one that survived: {after}"
    );

    // An unknown key is a refusal, never a fall-through to nuking the workspace.
    let missing = drive(
        &at_1_3_0(
            "ws-missing",
            "author.discard",
            &json!({ "workspace": workspace, "decl": "jcs:sha256:0000" }),
        ),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        error(&missing)["code"],
        "unknown-doc",
        "an unmatched key refuses: {missing}"
    );
    let still = drive(
        &at_1_3_0(
            "ws-still",
            "author.status",
            &json!({ "workspace": workspace }),
        ),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        result(&still)["staged"].as_array().unwrap().len(),
        1,
        "the refusal discarded nothing: {still}"
    );
}

/// The structural key is a **published function of the declaration's content**,
/// and both halves of that matter.
///
/// *Published*: the key `author.status` hands out is exactly
/// `digest_of(&declaration)` — the same JCS-over-SHA-256 the contract already
/// names as its digest domain. A caller can recompute it from the fact objects
/// it holds instead of treating it as an engine secret it must round-trip.
///
/// *Of the content*: JCS sorts object keys and fixes the encoding, so the key
/// depends on what the declaration says and on nothing else about how this
/// binary happened to serialize it. That is why the raw `serde_json::to_string`
/// this replaced could not be promoted to a wire identifier — see the assertion
/// below, which shows the two serializations of one identical declaration
/// disagreeing byte-for-byte while their JCS digests agree.
#[test]
fn the_structural_key_is_a_recomputable_digest_of_the_declaration() {
    use dli_ast::decl::{Decl, DialectDecl, TermDecl};
    use dli_ast::ident::Ident;
    use dli_ast::language::DialectBody;
    use dli_serve::author::digest::digest_of;

    let decl = Decl::Term(TermDecl {
        name: Ident::synthetic("hedgerow"),
        label: Some("the pleached boundary road".to_owned()),
        provenance: None,
    });

    // 1. Order-insensitive. `to_string` of the struct writes fields in
    //    DECLARATION order; `to_string` of the same value re-serialized through
    //    `Value` writes them in the map's order. One declaration, two byte
    //    strings — and the old key was whichever one serde happened to produce,
    //    so reordering two fields in the struct would have silently re-keyed
    //    every staged declaration of that kind. The digests are equal.
    let as_struct = serde_json::to_string(&decl).unwrap();
    let as_value = serde_json::to_string(&serde_json::to_value(&decl).unwrap()).unwrap();
    assert_ne!(
        as_struct, as_value,
        "this assertion is the hazard: if serde ever agreed on one ordering, the \
         rest of this test would be proving nothing"
    );
    assert_eq!(
        digest_of(&decl),
        digest_of(&serde_json::to_value(&decl).unwrap()),
        "the key must not depend on which serialization produced the bytes"
    );

    // 2. Additive-schema tolerance, stated honestly. A field that serializes to
    //    NOTHING cannot move the key: `DialectDecl::body` carries
    //    `skip_serializing_if`, so an empty body is absent from the bytes and
    //    the key equals that of the same declaration with no `body` key at all.
    //    This is the invariant that lets the AST grow: a declaration that does
    //    not use a new optional field keeps its name. (A new field that IS
    //    present changes the content and so changes the key — that is content
    //    addressing working, not a defect.)
    let dialect = Decl::Dialect(DialectDecl {
        name: Ident::synthetic("elder-tongue-fringe"),
        label: None,
        body: DialectBody::default(),
        provenance: None,
    });
    let serialized = serde_json::to_value(&dialect).unwrap();
    assert!(
        serialized
            .get("Dialect")
            .and_then(|d| d.get("body"))
            .is_none(),
        "an empty typed body is absent from the bytes, not present-and-empty: {serialized}"
    );
    assert_eq!(
        digest_of(&dialect),
        digest_of(&serialized),
        "a field that serializes to nothing does not participate in the key"
    );

    // 3. Distinct declarations take distinct keys — a key that collided would
    //    name a group, and `discard` would drop something the caller did not ask
    //    to drop.
    let other = Decl::Term(TermDecl {
        name: Ident::synthetic("ford"),
        label: Some("the pleached boundary road".to_owned()),
        provenance: None,
    });
    assert_ne!(digest_of(&decl), digest_of(&other));

    // 4. And the wire really hands out this value, rather than something merely
    //    shaped like it.
    let (mut sessions, mut store) = demo();
    let at_1_3_0 = |id: &str, op: &str, params: &Value| json!({ "dli": "2.0.0", "id": id, "op": op, "params": params });
    let opened = drive(
        &at_1_3_0("k-open", "author.open", &json!({ "scope": "camelot" })),
        &mut sessions,
        &mut store,
    );
    let workspace = result(&opened)["workspace"].as_str().unwrap().to_owned();
    drive(
        &at_1_3_0(
            "k-add",
            "author.add_term",
            &json!({
                "workspace": workspace,
                "id": "hedgerow",
                "label": "the pleached boundary road",
                "provenance": { "kind": "proposed", "origin": "dm4-key" },
            }),
        ),
        &mut sessions,
        &mut store,
    );
    let status = drive(
        &at_1_3_0(
            "k-status",
            "author.status",
            &json!({ "workspace": workspace }),
        ),
        &mut sessions,
        &mut store,
    );
    let staged = &result(&status)["staged"][0];
    // The staged declaration carries the tool's provenance, so rebuild the same
    // declaration rather than reusing `decl` above, which carries none.
    let with_provenance = Decl::Term(TermDecl {
        name: Ident::synthetic("hedgerow"),
        label: Some("the pleached boundary road".to_owned()),
        provenance: Some(dli_ast::annot::Annotation {
            marker: dli_ast::annot::ProvenanceMarker::Proposed,
            citations: Vec::new(),
            condition: None,
            tags: vec!["dm4-key".to_owned()],
            span: dli_ast::span::Span::synthetic(),
        }),
    });
    assert_eq!(
        staged["key"].as_str().unwrap(),
        digest_of(&with_provenance),
        "the key on the wire is recomputable from the declaration: {status}"
    );
}

// -- author.why's typed `goal_atom`: supporting_decisions, reachable syntax-free

/// `author.why`'s `supporting_decisions` (contract 1.9.0, DL-0083) was dead in a
/// syntax-free build: the op only accepted a rendered `atom` string, and parsing
/// it needs the `.dli` front end this build does not link. The typed `goal_atom`
/// successor (DL-0085) mirrors the sibling `author.query`'s `goal_atom` verbatim
/// and carries the atom structurally, so the auditability affordance — confirm a
/// fact reached certainty through a recorded decision — is finally reachable with
/// no parser in the process.
///
/// The walk is the whole taught recipe, wire-only: record a baseline decision,
/// cite it from a fact, then ask `author.why` with the TYPED goal and read the
/// decision back out of `supporting_decisions`. Every call is a typed `author.*`
/// mutation or read — no rendered string is parsed anywhere. It runs in both
/// lanes; the point is that it passes in the default one.
#[test]
fn author_why_typed_goal_atom_names_the_supporting_decision_syntax_free() {
    let (mut sessions, mut store) = demo();
    // `record_decision` writes a durable `ratify`, so its base needs a branch.
    // Attaching to the camelot demo imports the already-compiled fact objects —
    // no parser is involved in the import, so this also holds on a parser-free build.
    let root = tempfile::tempdir().expect("temporary lorestore root");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .expect("attach a lorestore to the camelot demo");

    let at = |id: &str, op: &str, params: &Value| json!({ "dli": "2.0.0", "id": id, "op": op, "params": params });

    let opened = drive(
        &at("open", "author.open", &json!({ "scope": "camelot" })),
        &mut sessions,
        &mut store,
    );
    let workspace = result(&opened)["workspace"].as_str().unwrap().to_owned();

    // The wire way to certainty: record the baseline decision, no `.dli ratify`.
    // Recorded first — it reloads the workspace base from the new head — so all
    // staging below happens over the post-record base.
    let recorded = drive(
        &at(
            "record",
            "author.record_decision",
            &json!({ "workspace": workspace, "decision": "d-1" }),
        ),
        &mut sessions,
        &mut store,
    );
    assert!(
        result(&recorded)["findings"].as_array().unwrap().is_empty(),
        "recording a fresh decision is clean: {recorded}"
    );

    // A typed entity declaration — the `knight` this fact will hang on.
    drive(
        &at(
            "add-entity",
            "author.add_entity",
            &json!({
                "workspace": workspace,
                "kind": "person",
                "id": "knight",
                "name": "Knight",
                "provenance": { "kind": "proposed", "origin": "dl0085-default-build" },
            }),
        ),
        &mut sessions,
        &mut store,
    );

    // A single fact citing the recorded decision — cited + ratified reads certain,
    // so the atom is derived and its support carries the decision literal.
    let added = drive(
        &at(
            "add-fact",
            "author.add_fact",
            &json!({
                "workspace": workspace,
                "entity": "knight",
                "key": "rank",
                "value": "captain",
                "provenance": { "kind": "cited", "decisions": ["d-1"], "state": "ratified" },
            }),
        ),
        &mut sessions,
        &mut store,
    );
    let add_codes: Vec<String> = result(&added)["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|finding| finding["code"].as_str().map(str::to_owned))
        .collect();
    assert!(
        !add_codes.contains(&"citation.unknown-decision".to_owned()),
        "the cited decision resolves — it was recorded on the wire: {added}"
    );

    // The affordance: ask `author.why` with the TYPED goal — no rendered `atom`,
    // so nothing to parse — and read the recorded decision back out.
    let why = drive(
        &at(
            "why",
            "author.why",
            &json!({
                "workspace": workspace,
                "goal_atom": {
                    "pred": ident("rank"),
                    "args": [{ "Sym": ident("knight") }, { "Sym": ident("captain") }],
                },
            }),
        ),
        &mut sessions,
        &mut store,
    );
    let supporting: Vec<String> = result(&why)["supporting_decisions"]
        .as_array()
        .unwrap_or_else(|| panic!("author.why answers with supporting_decisions: {why}"))
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    assert!(
        supporting.contains(&"d-1".to_owned()),
        "the typed goal_atom reaches supporting_decisions with no parser: {why}"
    );
}
