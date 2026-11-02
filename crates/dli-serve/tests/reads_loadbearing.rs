#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_pass_by_value
)]
//! RED TEAM: the contract 1.10.0 reads (`author.predicates`, `author.entity_candidates`)
//! must be LOAD-BEARING — their answers derive from the store's compiled staged
//! world (`staged_world(store, ws) -> world.model`), never from a side channel
//! (a request field smuggling data, or a stand-in).
//!
//! Four properties, each attacked empirically over the real wire:
//!   1. store-derived, not caller-passed — the only request fields are
//!      `workspace`/`surface`/`limit`; none carries predicate or entity DATA;
//!   2. the answer genuinely depends on store state — staging makes a thing
//!      appear, not staging omits it, and two workspaces with different state
//!      answer differently for the identical request shape;
//!   3. fault-injection analog — an empty store degrades to empty results
//!      rather than fabricating;
//!   4. agreement with the FD check — `predicates.functional` reads the same
//!      DECLARED set `continuity.fd-violation` fires on, and an
//!      `entity_candidates` canonical representative keys the way the FD check's
//!      `identity_canon`/`canon_slot` does.
//!
//! Mirrors `crates/dli-serve/tests/predicates.rs`'s harness.

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

fn candidates(
    store: &mut Store,
    dli: &str,
    ws: &str,
    surface: &str,
    limit: u64,
) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "ec",
            Op::AuthorEntityCandidates,
            json!({ "workspace": ws, "surface": surface, "limit": limit }),
        ),
        store,
    )
}

fn check(store: &mut Store, dli: &str, ws: &str) -> Value {
    result(&dispatch(
        &req(dli, "ck", Op::AuthorCheck, json!({ "workspace": ws })),
        store,
    ))
    .clone()
}

/// The names present in a predicates census.
fn pred_names(result: &Value) -> Vec<String> {
    result["predicates"]
        .as_array()
        .expect("a predicates array")
        .iter()
        .map(|row| row["pred"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// The `functional` flag for `pred`, or `None` if the predicate is absent.
fn functional_of(result: &Value, pred: &str) -> Option<bool> {
    result["predicates"]
        .as_array()
        .expect("a predicates array")
        .iter()
        .find(|row| row["pred"] == json!(pred))
        .map(|row| row["functional"].as_bool().unwrap_or(false))
}

/// The names present in a candidates result.
fn candidate_names(result: &Value) -> Vec<String> {
    result["candidates"]
        .as_array()
        .expect("a candidates array")
        .iter()
        .map(|row| row["name"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// The finding codes in an `author.check` result.
fn codes(result: &Value) -> Vec<String> {
    result["findings"]
        .as_array()
        .map(|findings| {
            findings
                .iter()
                .map(|f| f["code"].as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Stage an attribute fact `<entity>.<key> = <value>` with the given provenance.
fn add_fact(
    store: &mut Store,
    dli: &str,
    ws: &str,
    entity: &str,
    key: &str,
    value: &str,
    prov: Value,
) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "add",
            Op::AuthorAddFact,
            json!({ "workspace": ws, "entity": entity, "key": key, "value": value, "provenance": prov }),
        ),
        store,
    )
}

fn cited(decision: &str) -> Value {
    json!({ "kind": "cited", "decisions": [decision], "state": "ratified" })
}

fn proposed() -> Value {
    json!({ "kind": "proposed", "origin": "rt" })
}

// ===========================================================================
// PROPERTY 1 + 2: store-derived, and answers track the SPECIFIC workspace's
// staged state — not the (identical) request shape.
// ===========================================================================

/// Two workspaces on ONE store, identical request shape, different staged
/// state -> different census. If the answer came from the request rather than
/// the store, both would answer alike. `beta-rel` is staged only in `ws_a`, so
/// it appears there and is absent in `ws_b`; the base's `alpha-rel` appears in
/// both.
#[test]
fn predicates_track_the_specific_workspace_not_the_request() {
    let mut store = base(concat!(
        "scope storybook\n",
        "ratify D-0001\n",
        "place nara \"Nara\"  ratified D-0001\n",
        "place keeper \"Keeper\"  ratified D-0001\n",
        "alpha-rel(nara, keeper)  ratified D-0001\n",
    ));
    let ws_a = open_scope(&mut store, "1.10.0", "storybook");
    let ws_b = open_scope(&mut store, "1.10.0", "storybook");

    // Stage a NEW predicate only in ws_a.
    let staged = add_fact(
        &mut store,
        "1.10.0",
        &ws_a,
        "nara",
        "beta-rel",
        "keeper",
        proposed(),
    );
    assert!(staged.ok, "stage beta-rel in ws_a: {staged:?}");

    let a = predicates(&mut store, "1.10.0", &ws_a);
    let a = pred_names(result(&a));
    let b = predicates(&mut store, "1.10.0", &ws_b);
    let b = pred_names(result(&b));

    assert!(
        a.contains(&"alpha-rel".to_owned()),
        "base predicate in ws_a: {a:?}"
    );
    assert!(
        a.contains(&"beta-rel".to_owned()),
        "staged predicate appears in ws_a: {a:?}"
    );
    assert!(
        b.contains(&"alpha-rel".to_owned()),
        "base predicate in ws_b: {b:?}"
    );
    assert!(
        !b.contains(&"beta-rel".to_owned()),
        "a predicate staged in ANOTHER workspace must not leak into ws_b: {b:?}"
    );
}

/// The same for entities: an entity staged only in `ws_a` is a candidate there
/// and absent in `ws_b`, for the byte-identical `surface`/`limit`. This is the
/// decisive no-side-channel proof — the surface string is fixed, so any
/// difference is store state, not request data.
#[test]
fn entity_candidates_track_the_specific_workspace_not_the_request() {
    let mut store = base(concat!(
        "scope storybook\n",
        "ratify D-0001\n",
        "place keeper \"Keeper\"  ratified D-0001\n",
    ));
    let ws_a = open_scope(&mut store, "1.10.0", "storybook");
    let ws_b = open_scope(&mut store, "1.10.0", "storybook");

    // Before staging: "marabel" is nobody in either workspace.
    let pre = candidates(&mut store, "1.10.0", &ws_a, "marabel", 5);
    assert!(
        !candidate_names(result(&pre)).contains(&"marabel".to_owned()),
        "an entity nobody staged is not a candidate: {:?}",
        candidate_names(result(&pre))
    );

    // Stage the entity only in ws_a.
    let staged = dispatch(
        &req(
            "1.10.0",
            "ae",
            Op::AuthorAddEntity,
            json!({ "workspace": ws_a, "kind": "place", "id": "marabel", "name": "Marabel",
                    "provenance": proposed() }),
        ),
        &mut store,
    );
    assert!(staged.ok, "stage entity in ws_a: {staged:?}");

    let a = candidates(&mut store, "1.10.0", &ws_a, "marabel", 5);
    assert!(
        candidate_names(result(&a)).contains(&"marabel".to_owned()),
        "the staged entity is now a candidate in ws_a: {:?}",
        candidate_names(result(&a))
    );

    let b = candidates(&mut store, "1.10.0", &ws_b, "marabel", 5);
    assert!(
        !candidate_names(result(&b)).contains(&"marabel".to_owned()),
        "an entity staged in ANOTHER workspace must not leak into ws_b: {:?}",
        candidate_names(result(&b))
    );
}

/// The surface form is a QUERY, not a data source: a surface that matches no
/// stored entity is never itself returned as a fabricated candidate, and every
/// name returned is an actual store entity.
#[test]
fn the_surface_form_is_never_a_fabricated_candidate() {
    let mut store = base(concat!(
        "scope storybook\n",
        "ratify D-0001\n",
        "place keeper \"Keeper\"  ratified D-0001\n",
    ));
    let ws = open_scope(&mut store, "1.10.0", "storybook");

    let out = candidates(&mut store, "1.10.0", &ws, "zzzzzzzzzzzz", 5);
    let names = candidate_names(result(&out));
    assert!(
        !names.contains(&"zzzzzzzzzzzz".to_owned()),
        "the surface string must never surface as its own candidate: {names:?}"
    );
    // Every candidate is a real store entity (here, only `keeper`).
    assert!(
        names.iter().all(|n| n == "keeper"),
        "every candidate is an actual stored entity: {names:?}"
    );
}

/// There is no request field to smuggle answer-data through: both params types
/// are `deny_unknown_fields`, so a caller cannot even PARSE a request that
/// carries a `predicates`/`candidates` payload. This shuts the side channel at
/// the wire boundary, before any handler logic.
#[test]
fn the_reads_refuse_a_smuggled_data_field() {
    let mut store = base(concat!(
        "scope storybook\n",
        "ratify D-0001\n",
        "place keeper \"Keeper\"  ratified D-0001\n",
    ));
    let ws = open_scope(&mut store, "1.10.0", "storybook");

    // A forged predicate census smuggled alongside the legitimate workspace id.
    let smuggled = dispatch(
        &req(
            "1.10.0",
            "pr",
            Op::AuthorPredicates,
            json!({ "workspace": ws,
                    "predicates": [{ "pred": "forged", "arity": 2, "functional": true, "inertial": false }] }),
        ),
        &mut store,
    );
    assert!(
        !smuggled.ok,
        "an unknown data field must be refused, not ignored-and-answered: {smuggled:?}"
    );

    // A forged candidate list smuggled alongside surface/limit.
    let smuggled = dispatch(
        &req(
            "1.10.0",
            "ec",
            Op::AuthorEntityCandidates,
            json!({ "workspace": ws, "surface": "keeper", "limit": 5,
                    "candidates": [{ "name": "forged", "distance": 0 }] }),
        ),
        &mut store,
    );
    assert!(
        !smuggled.ok,
        "an unknown data field must be refused, not ignored-and-answered: {smuggled:?}"
    );
}

// ===========================================================================
// PROPERTY 3: fault-injection analog — an empty store degrades to empty,
// never fabricates.
// ===========================================================================

/// A staged world with no facts and no rules has an EMPTY predicate census —
/// the read reports the world, not a stand-in.
#[test]
fn an_empty_staged_world_has_an_empty_predicate_census() {
    let mut store = base("scope storybook\n");
    let ws = open_scope(&mut store, "1.10.0", "storybook");
    let out = predicates(&mut store, "1.10.0", &ws);
    assert!(
        pred_names(result(&out)).is_empty(),
        "an empty world fabricates no predicates: {:?}",
        pred_names(result(&out))
    );
}

/// A staged world with no entities yields NO candidates, for any surface — the
/// read tracks the (empty) world, not the (non-empty) request.
#[test]
fn an_empty_staged_world_yields_no_entity_candidates() {
    let mut store = base("scope storybook\n");
    let ws = open_scope(&mut store, "1.10.0", "storybook");
    for surface in ["nara", "keeper", "anything-at-all"] {
        let out = candidates(&mut store, "1.10.0", &ws, surface, 25);
        assert!(
            candidate_names(result(&out)).is_empty(),
            "empty store, surface {surface:?}: no fabricated candidates: {:?}",
            candidate_names(result(&out))
        );
    }
}

// ===========================================================================
// PROPERTY 4: agreement with the FD check.
// ===========================================================================

/// The census `functional` flag reads exactly the DECLARED set
/// `continuity.fd-violation` fires on — in BOTH directions:
///   - `rank` is declared functional AND carries two values -> census
///     `functional:true` AND check fires `continuity.fd-violation` on `rank`;
///   - `hobby` is undeclared AND carries two values -> census
///     `functional:false` AND the check fires NO fd-violation on `hobby`.
///
/// A disagreement would be a census that flagged a predicate the check ignores
/// (or vice versa). There is none.
#[test]
fn predicates_functional_flag_agrees_with_fd_violation_both_directions() {
    let mut store = base(concat!(
        "scope storybook\n",
        "ratify D-0001\n",
        "person nara \"Nara\"  ratified D-0001\n",
    ));
    let ws = open_scope(&mut store, "1.10.0", "storybook");

    // Declare `rank` functional (staged); leave `hobby` undeclared.
    let declared = dispatch(
        &req(
            "1.10.0",
            "df",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": "rank", "provenance": proposed() }),
        ),
        &mut store,
    );
    assert!(declared.ok, "declare rank functional: {declared:?}");

    // Two distinct CERTAIN values for each predicate on `nara`.
    add_fact(
        &mut store,
        "1.10.0",
        &ws,
        "nara",
        "rank",
        "captain",
        cited("D-0001"),
    );
    add_fact(
        &mut store,
        "1.10.0",
        &ws,
        "nara",
        "rank",
        "admiral",
        cited("D-0001"),
    );
    add_fact(
        &mut store,
        "1.10.0",
        &ws,
        "nara",
        "hobby",
        "chess",
        cited("D-0001"),
    );
    add_fact(
        &mut store,
        "1.10.0",
        &ws,
        "nara",
        "hobby",
        "go",
        cited("D-0001"),
    );

    // The census.
    let census = predicates(&mut store, "1.10.0", &ws);
    let census = result(&census);
    assert_eq!(
        functional_of(census, "rank"),
        Some(true),
        "declared predicate reads functional:true: {census}"
    );
    assert_eq!(
        functional_of(census, "hobby"),
        Some(false),
        "undeclared predicate reads functional:false even with two values: {census}"
    );

    // The check.
    let checked = check(&mut store, "1.10.0", &ws);
    let fd: Vec<&Value> = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"] == json!("continuity.fd-violation"))
        .collect();
    assert_eq!(fd.len(), 1, "exactly one fd-violation, on rank: {checked}");
    assert_eq!(
        fd[0]["data"]["pred"],
        json!("rank"),
        "the fd-violation names the declared predicate: {}",
        fd[0]
    );
    // No fd-violation ever names the undeclared predicate.
    assert!(
        !fd.iter().any(|f| f["data"]["pred"] == json!("hobby")),
        "the undeclared predicate never triggers fd-violation: {checked}"
    );

    // The coupling, stated directly: predicate flagged functional by the census
    // <=> a two-valued slot on it is an fd-violation the check fires.
    assert_eq!(functional_of(census, "rank"), Some(true));
    assert!(codes(&checked).contains(&"continuity.fd-violation".to_owned()));
}

/// A declared-functional predicate with NO facts is absent from the census
/// (nothing is "present") AND fires no fd-violation — the degenerate case still
/// agrees. This pins the edge so a future census that started fabricating
/// declared-but-factless predicates (or a check that fired on them) would be
/// caught.
#[test]
fn a_declared_functional_predicate_with_no_facts_agrees_by_absence() {
    let mut store = base(concat!(
        "scope storybook\n",
        "ratify D-0001\n",
        "person nara \"Nara\"  ratified D-0001\n",
    ));
    let ws = open_scope(&mut store, "1.10.0", "storybook");
    let declared = dispatch(
        &req(
            "1.10.0",
            "df",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": "ghost-pred", "provenance": proposed() }),
        ),
        &mut store,
    );
    assert!(declared.ok, "declare ghost-pred functional: {declared:?}");

    let census = predicates(&mut store, "1.10.0", &ws);
    assert_eq!(
        functional_of(result(&census), "ghost-pred"),
        None,
        "a declared-but-factless predicate is not 'present': {:?}",
        pred_names(result(&census))
    );

    let checked = check(&mut store, "1.10.0", &ws);
    assert!(
        !codes(&checked).contains(&"continuity.fd-violation".to_owned()),
        "no facts -> no fd-violation: {checked}"
    );
}

/// The entity-candidate canonical representative keys the SAME way
/// `continuity.fd-violation`'s subject does. A certain `same-as nara = sora`
/// merges two conflicting `rank` facts into one class; the FD check names the
/// class by its canonical representative (`data.subject`), and
/// `entity_candidates` for the aliased surface `sora` folds to that SAME
/// representative — never the raw alias. If the read canonicalized differently
/// than the check, the surfaced name would disagree with `data.subject`.
#[test]
fn entity_candidates_canonical_rep_matches_the_fd_violation_subject() {
    let mut store = base(concat!(
        "scope storybook\n",
        "ratify D-0001\n",
        "person nara \"Nara\"  ratified D-0001\n",
        "person sora \"Sora\"  ratified D-0001\n",
        "rank(nara, gold)  ratified D-0001\n",
        "rank(sora, silver)  ratified D-0001\n",
    ));
    let ws = open_scope(&mut store, "1.10.0", "storybook");

    // Declare rank functional and merge the two people (certain merge).
    let declared = dispatch(
        &req(
            "1.10.0",
            "df",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": "rank", "provenance": proposed() }),
        ),
        &mut store,
    );
    assert!(declared.ok, "declare rank functional: {declared:?}");
    let merged = dispatch(
        &req(
            "1.10.0",
            "sa",
            Op::AuthorAssertSameAs,
            json!({ "workspace": ws, "left": "nara", "right": "sora", "provenance": cited("D-0001") }),
        ),
        &mut store,
    );
    assert!(merged.ok, "assert same-as: {merged:?}");

    // The check names the class by its canonical subject.
    let checked = check(&mut store, "1.10.0", &ws);
    let fd = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == json!("continuity.fd-violation"))
        .unwrap_or_else(|| panic!("expected a post-merge fd-violation: {checked}"));
    let fd_subject = fd["data"]["subject"]
        .as_str()
        .expect("fd-violation carries data.subject at 1.10.0")
        .to_owned();

    // The read, asked under the ALIAS spelling, folds to the same rep.
    let out = candidates(&mut store, "1.10.0", &ws, "sora", 5);
    let names = candidate_names(result(&out));
    assert!(
        names.contains(&fd_subject),
        "the surfaced candidate keys the way the FD check does: fd subject {fd_subject:?} vs candidates {names:?}"
    );
    assert!(
        !names.contains(&"sora".to_owned()),
        "the aliased member never surfaces separately from its rep: {names:?}"
    );
    // Nail it down: the canonical representative is `nara` (lexicographic min).
    assert_eq!(fd_subject, "nara", "the class's canonical subject");
}
