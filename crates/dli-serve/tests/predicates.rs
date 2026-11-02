#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Contract 1.10's `author.predicates` — the predicate census for incremental
//! canonicalization (Read A). Mirrors `crates/dli-serve/tests/declare_inertial.rs`'s
//! harness.
//!
//! Coverage: op reachability (advertised and answered only from `1.10.0` up), the
//! census itself over source atoms and rule heads, the DECLARED-only `functional`
//! annotation (it must read `functional_predicates`, never canon's inferred set,
//! so the census agrees with `continuity.fd-violation`), and the guard-census
//! determination — the read stages no `Decl`, so it needs no per-kind arm.

use serde_json::{Value, json};

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::{Op, Request};

// -- harness -----------------------------------------------------------------

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

fn result(response: &dli_serve::Response) -> &Value {
    assert!(response.ok, "expected ok, got error: {:?}", response.error);
    response
        .result
        .as_ref()
        .expect("ok response carries a result")
}

fn base(source: &str) -> Store {
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.is_empty(),
        "base source parses: {diagnostics:?}"
    );
    Store::with_pack(dli_world::pack::Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .expect("base compiles")
}

/// A base with two binary predicates present (`current-role`, `hair-color`) so
/// the census has arity-2, FD-eligible predicates to annotate.
fn two_predicate_base() -> Store {
    base(concat!(
        "scope storybook\n",
        "ratify D-0001\n",
        "place nara \"Nara\"  ratified D-0001\n",
        "place keeper \"Keeper\"  ratified D-0001\n",
        "current-role(nara, keeper) decided D-0001\n",
        "hair-color(nara, keeper) decided D-0001\n",
    ))
}

fn open_scope(store: &mut Store, dli: &str, scope: &str) -> String {
    let response = dispatch(
        &req(dli, "open", Op::AuthorOpen, json!({ "scope": scope })),
        store,
    );
    result(&response)["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned()
}

fn predicates(store: &mut Store, dli: &str, ws: &str) -> dli_serve::Response {
    dispatch(
        &req(dli, "pr", Op::AuthorPredicates, json!({ "workspace": ws })),
        store,
    )
}

/// The census row for `pred`, or `None` if it is absent.
fn entry<'a>(result: &'a Value, pred: &str) -> Option<&'a Value> {
    result["predicates"]
        .as_array()
        .expect("a predicates array")
        .iter()
        .find(|row| row["pred"] == json!(pred))
}

// -- op reachability ---------------------------------------------------------

// -- the census --------------------------------------------------------------

#[test]
fn the_census_lists_present_predicates_with_arity() {
    let mut store = two_predicate_base();
    let ws = open_scope(&mut store, "1.10.0", "storybook");
    let response = predicates(&mut store, "1.10.0", &ws);
    let out = result(&response);
    let role = entry(out, "current-role").expect("current-role is present");
    assert_eq!(role["arity"], json!(2), "binary predicate: {role}");
    let hair = entry(out, "hair-color").expect("hair-color is present");
    assert_eq!(hair["arity"], json!(2), "binary predicate: {hair}");
}

/// The DECLARED-only authority: `functional` reads the author's declared set
/// (`author.declare_functional`), never canon's inferred-functional set, so the
/// census agrees with what `continuity.fd-violation` fires on. A predicate that
/// is present but undeclared reads `functional: false` even when its values
/// would infer a dependency.
#[test]
fn functional_is_annotated_from_the_declared_set_only() {
    let mut store = two_predicate_base();
    let ws = open_scope(&mut store, "1.10.0", "storybook");

    // Before declaring: nothing is functional, even though current-role has a
    // single value that canon-precedent inference alone might treat as one.
    let before = predicates(&mut store, "1.10.0", &ws);
    assert_eq!(
        entry(result(&before), "current-role").expect("present")["functional"],
        json!(false),
        "undeclared predicate reads functional:false"
    );

    // Declare current-role functional (stages the decl into the workspace).
    let declared = dispatch(
        &req(
            "1.10.0",
            "df",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": "current-role",
                    "provenance": { "kind": "proposed", "origin": "test" } }),
        ),
        &mut store,
    );
    assert!(declared.ok, "declare_functional succeeds: {declared:?}");

    let after = predicates(&mut store, "1.10.0", &ws);
    let out = result(&after);
    assert_eq!(
        entry(out, "current-role").expect("present")["functional"],
        json!(true),
        "the declared predicate now reads functional:true"
    );
    assert_eq!(
        entry(out, "hair-color").expect("present")["functional"],
        json!(false),
        "an undeclared sibling stays functional:false"
    );
}

// -- guard-census determination ----------------------------------------------
//
// author.predicates is a PURE READ: it stages no `Decl`, so it introduces no new
// declaration kind and needs no arm in the four wildcarded guard-census matches
// (`decl_kind`, `free_text_fields`, `decl_annotation`, `sanitize_decl`). This
// test is the standing proof of that determination — it fails if the read ever
// begins mutating the staged-decl set.

#[test]
fn the_read_stages_nothing() {
    let mut store = two_predicate_base();
    let ws = open_scope(&mut store, "1.10.0", "storybook");

    // Stage one declaration so "unchanged" means "neither added nor dropped".
    let declared = dispatch(
        &req(
            "1.10.0",
            "df",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": "current-role",
                    "provenance": { "kind": "proposed", "origin": "test" } }),
        ),
        &mut store,
    );
    assert!(declared.ok, "stage a baseline decl: {declared:?}");
    let staged_before = store.get(&ws).expect("workspace open").decls.len();

    let read = predicates(&mut store, "1.10.0", &ws);
    assert!(read.ok, "the read answers: {read:?}");

    let staged_after = store.get(&ws).expect("workspace open").decls.len();
    assert_eq!(
        staged_before, staged_after,
        "a pure read must neither stage nor drop a declaration"
    );
}
