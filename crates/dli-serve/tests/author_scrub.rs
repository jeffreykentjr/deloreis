//! The authority scrub (DL-0054, versioned and implemented by DL-0058).
//!
//! DL-0054 ruled that deloreis stops enforcing **who may rule**. Every caller of
//! this engine is a machine; whether that machine is entitled to cite a ruling
//! is a question about the consuming application's review pipeline, and the
//! engine had encoded a guess at the answer as a wire refusal. It records what
//! the caller states and verifies the one thing it genuinely can — that the
//! cited decision exists.
//!
//! Three properties, and the third is the one that makes the first two safe:
//!
//! 1. **At ≥ 1.3.0 a citation is recorded**, not refused: the claim carries a
//!    `decided` marker and its ids.
//! 2. **Below 1.3.0 the old refusal is served unchanged**, so every released
//!    wire is frozen. The `ls8_zero_x_parity` corpus covers `0.x`; this covers
//!    the `1.0.0`–`1.2.0` band, which that corpus does not reach.
//! 3. **Referential integrity stays fail-closed.** Recording a citation is not
//!    the same as believing it. A cited decision the corpus does not declare
//!    leaves its variable *open*, so the claim it supports can never read
//!    `certain` — and the finding that says so is error-severity, which bars
//!    materialize. Without this, the scrub would have turned "the engine
//!    refuses citations" into "the engine accepts forged ones".

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::{Value, json};

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::{Op, Request};

/// Every version this build accepts, below the scrub floor.
const FROZEN: &[&str] = &[
    "0.5.1", "0.6.0", "0.6.1", "0.6.2", "0.6.3", "0.6.4", "1.0.0", "1.1.0", "1.2.0",
];

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

fn result(response: &dli_serve::Response) -> Value {
    assert!(response.ok, "expected ok, got error: {:?}", response.error);
    response.result.clone().expect("ok carries a result")
}

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

/// A demo-store workspace over the built-in scope.
fn workspace(dli: &str) -> (Store, String) {
    let mut store = Store::default();
    let opened = dispatch(
        &req(dli, "open", Op::AuthorOpen, json!({ "scope": "demo" })),
        &mut store,
    );
    let id = result(&opened)["workspace"]
        .as_str()
        .expect("author.open answers a workspace id")
        .to_owned();
    (store, id)
}

fn add_entity(store: &mut Store, dli: &str, provenance: &Value) -> Value {
    let ws = store
        .workspaces()
        .next()
        .map(|ws| ws.id.clone())
        .expect("a workspace is open");
    let response = dispatch(
        &req(
            dli,
            "1",
            Op::AuthorAddEntity,
            json!({
                "workspace": ws,
                "kind": "person",
                "id": "mireth",
                "name": "Mireth",
                "provenance": provenance,
            }),
        ),
        store,
    );
    result(&response)
}

#[test]
fn at_the_scrub_version_a_citation_is_recorded_as_data() {
    let (mut store, _ws) = workspace("1.3.0");
    let result = add_entity(
        &mut store,
        "1.3.0",
        &json!({ "kind": "cited", "decisions": ["d-1"] }),
    );
    assert!(
        !codes(&result).contains(&"provenance.cited-refused".to_owned()),
        "the refusal is scrubbed at 1.3.0: {result}"
    );
    // The claim staged: something is in the workspace to review.
    let staged = store
        .workspaces()
        .next()
        .map(|ws| ws.decls.len())
        .unwrap_or_default();
    assert_eq!(
        staged, 1,
        "the cited claim is staged, not dropped: {result}"
    );
}

#[test]
fn below_the_scrub_version_every_accepted_wire_still_refuses_a_citation() {
    for dli in FROZEN {
        let (mut store, _ws) = workspace(dli);
        let result = add_entity(
            &mut store,
            dli,
            &json!({ "kind": "cited", "decisions": ["d-1"] }),
        );
        assert!(
            codes(&result).contains(&"provenance.cited-refused".to_owned()),
            "{dli} must still refuse a citation: {result}"
        );
        assert_eq!(
            result["rendered"],
            json!([]),
            "{dli}: the refusal stages nothing: {result}"
        );
        let staged = store
            .workspaces()
            .next()
            .map(|ws| ws.decls.len())
            .unwrap_or_default();
        assert_eq!(staged, 0, "{dli}: nothing reached the workspace: {result}");
    }
}

/// The finding text is part of the frozen surface, not just its code: a caller
/// below 1.3.0 reads the message and the repair, and both shipped.
#[test]
fn the_frozen_refusal_keeps_its_message_and_repair_verbatim() {
    let (mut store, _ws) = workspace("1.2.0");
    let result = add_entity(
        &mut store,
        "1.2.0",
        &json!({ "kind": "cited", "decisions": ["d-1"] }),
    );
    let finding = &result["findings"][0];
    assert_eq!(finding["code"], "provenance.cited-refused");
    assert_eq!(finding["severity"], "error");
    assert_eq!(finding["grade"], "checked");
    assert_eq!(
        finding["message"],
        "Door T authors `proposed` or `hypothesis` only; an agent cannot cite a ruling to mint \
         canon authority. Citing a decision is a human act in review — resubmit with proposed \
         provenance."
    );
    assert_eq!(
        finding["repair"],
        json!({ "kind": "add-provenance", "required": "proposed" })
    );
}

/// **The scrub's load-bearing guard.** Recording a citation must not mean
/// believing it. A forged id draws an error-severity finding, which is what
/// `author.materialize` refuses on — so a claim resting on an invented ruling
/// cannot be committed.
#[test]
fn a_forged_citation_is_refused_on_every_wire_that_records_citations() {
    let (mut store, ws) = workspace("1.3.0");
    let result = add_entity(
        &mut store,
        "1.3.0",
        &json!({ "kind": "cited", "decisions": ["d-9999-forged"] }),
    );
    let found = result["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|f| f["code"] == "citation.unknown-decision")
        .unwrap_or_else(|| panic!("a forged citation is caught at the add: {result}"));
    assert_eq!(
        found["severity"], "error",
        "an unresolvable citation bars materialize: {result}"
    );
    assert!(
        found["message"]
            .as_str()
            .is_some_and(|m| m.contains("d-9999-forged")),
        "the finding names the id it could not resolve: {result}"
    );

    // And materialize actually refuses, rather than the finding merely existing.
    let blocked = dispatch(
        &req(
            "1.3.0",
            "2",
            Op::AuthorMaterialize,
            json!({ "workspace": ws }),
        ),
        &mut store,
    );
    assert!(
        !blocked.ok,
        "materialize must refuse while the forged citation stands: {blocked:?}"
    );
    let error = blocked.error.expect("a refusal carries an error");
    assert_eq!(error.code, dli_serve::ErrorCode::MaterializeBlocked);
    assert!(
        error
            .data
            .as_ref()
            .and_then(|d| d.get("blocking"))
            .and_then(Value::as_array)
            .is_some_and(|blocking| blocking.iter().any(|c| c == "citation.unknown-decision")),
        "the blocking code is named in the refusal data: {error:?}"
    );
}

/// The gate is the negotiated version, not the newest build: a caller that has
/// not opted in sees no behavior change at all, which is what "every tagged
/// prior contract is frozen" means operationally.
#[test]
fn the_two_behaviors_differ_only_by_the_negotiated_version() {
    let forged: Value = json!({ "kind": "cited", "decisions": ["d-9999-forged"] });

    let (mut old, _) = workspace("1.2.0");
    let old_result = add_entity(&mut old, "1.2.0", &forged);

    let (mut new, _) = workspace("1.3.0");
    let new_result = add_entity(&mut new, "1.3.0", &forged);

    assert_eq!(
        codes(&old_result),
        vec!["provenance.cited-refused".to_owned()],
        "1.2.0 answers with the refusal alone"
    );
    assert!(
        codes(&new_result).contains(&"citation.unknown-decision".to_owned())
            && !codes(&new_result).contains(&"provenance.cited-refused".to_owned()),
        "1.3.0 answers with referential integrity instead: {new_result}"
    );
}

/// A `proposed` claim is untouched by the scrub on both sides of the gate. The
/// change is scoped to `cited`; if it had leaked, this is where it would show.
#[test]
fn proposed_provenance_is_identical_across_the_gate() {
    let proposed: Value = json!({ "kind": "proposed", "origin": "session-scrub" });

    let (mut old, _) = workspace("1.2.0");
    let old_result = add_entity(&mut old, "1.2.0", &proposed);

    let (mut new, _) = workspace("1.3.0");
    let new_result = add_entity(&mut new, "1.3.0", &proposed);

    assert_eq!(
        old_result["rendered"], new_result["rendered"],
        "a proposed claim renders identically across the gate"
    );
    assert_eq!(
        codes(&old_result),
        codes(&new_result),
        "a proposed claim raises the same findings across the gate"
    );
    assert_eq!(
        old_result["workspace_digest"], new_result["workspace_digest"],
        "a proposed claim reaches the same staged state across the gate"
    );
}

// -- the two legs, on standing rather than on findings ------------------------
//
// These need a corpus that actually ratifies something, so they run against the
// fixture project (whose `saltmarket` document declares `ratify d-1`) and are
// therefore syntax-lane. `author-scrub-cited-recorded-v1` replays the same pair
// cold; this states the property by name, so a failure says which leg broke
// instead of printing a byte diff.

#[cfg(feature = "syntax")]
mod standing {
    use std::sync::Arc;

    use dli_serve::author::store::Store;
    use dli_serve::project::Project;
    use dli_serve::serve::respond;
    use dli_serve::session::Sessions;
    use serde_json::{Value, json};

    fn fixture_project() -> (tempfile::TempDir, Sessions, Store) {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/dli-worker/fixture-project");
        let temp = tempfile::tempdir().unwrap();
        copy_tree(&source, temp.path());
        let project = Arc::new(Project::load(temp.path()).unwrap());
        let packs = project
            .documents()
            .map(|(id, d)| (id.to_owned(), d.pack.clone(), d.killed.clone()));
        let store = Store::with_packs(packs).unwrap();
        (temp, Sessions::from_project(Arc::clone(&project)), store)
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

    fn drive(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
        let line = serde_json::to_string(frame).unwrap();
        serde_json::to_value(respond(&line, sessions, store).unwrap()).unwrap()
    }

    /// Stage `rel(baseline, baseline)` citing `decision`, then read the claim's
    /// standing back off the staged world. Returns `(add_result, status)`.
    fn cite_then_read(decision: &str, rel: &str) -> (Value, String) {
        let (_temp, mut sessions, mut store) = fixture_project();
        let opened = drive(
            &json!({"dli":"2.0.0","id":"o","op":"author.open",
                    "params":{"scope":"saltmarket"}}),
            &mut sessions,
            &mut store,
        );
        let ws = opened["result"]["workspace"].as_str().unwrap().to_owned();
        let added = drive(
            &json!({"dli":"2.0.0","id":"1","op":"author.add_relation","params":{
                "workspace": ws, "relation": rel, "from":"baseline", "to":"baseline",
                "provenance":{"kind":"cited","decisions":[decision],"state":"ratified"}}}),
            &mut sessions,
            &mut store,
        );
        let queried = drive(
            &json!({"dli":"2.0.0","id":"2","op":"author.query","params":{
                "workspace": ws, "why": true,
                "goal_atom":{"pred": rel, "args":[{"Sym":"baseline"},{"Sym":"baseline"}]}}}),
            &mut sessions,
            &mut store,
        );
        let status = queried["result"]["rows"][0]["status"]
            .as_str()
            .unwrap_or_else(|| panic!("the staged claim is derived: {queried}"))
            .to_owned();
        (added["result"].clone(), status)
    }

    /// **Leg 1.** A citation to a decision the corpus declares is recorded *and
    /// believed*: the decision's variable is ruled true, so the claim reads
    /// `certain` and its support antichain resolves to the cited id.
    #[test]
    fn a_declared_citation_is_recorded_and_rules_the_claim_certain() {
        let (added, status) = cite_then_read("d-1", "serves");
        assert_eq!(
            added["findings"],
            json!([]),
            "a resolvable citation raises nothing: {added}"
        );
        assert_eq!(
            added["rendered"],
            json!(["serves(baseline, baseline) ratified d-1"]),
            "the claim records the marker and the id the caller supplied: {added}"
        );
        assert_eq!(
            status, "certain",
            "a claim resting on a DECLARED decision reads certain"
        );
    }

    /// **Leg 2, and the one that makes leg 1 safe.** A citation to a decision
    /// nothing declares is still *recorded* — the engine writes down what the
    /// caller stated — but it is not *believed*. The variable is left open, so
    /// the claim can never read `certain`, and the error-severity finding bars
    /// materialize. Referential integrity (DL-0054 clause 2) is preserved by the
    /// scrub, not scrubbed by it.
    #[test]
    fn an_undeclared_citation_still_fails_closed_at_the_scrub_version() {
        let (added, status) = cite_then_read("d-9999", "guards");
        let finding = added["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["code"] == "citation.unknown-decision")
            .unwrap_or_else(|| panic!("the forged id is caught at the add: {added}"));
        assert_eq!(finding["severity"], "error", "it bars materialize: {added}");
        assert_ne!(
            status, "certain",
            "a claim resting on an UNDECLARED decision must never read certain — \
             the scrub records the attribution, it does not believe it"
        );
        assert_eq!(
            status, "possible",
            "the variable is left open, not ruled false"
        );
    }
}

// -- the review diff is forgery-proof, including through time clauses ---------

/// A staged declaration renders as **exactly one line**, whatever its free text
/// contains. The review diff is the human's supervision surface, so a newline
/// smuggled into any field must never split one declaration into two and forge a
/// second the reviewer reads as real (R3/C1).
///
/// This is a regression test for a hole a red-team probe found against
/// `author.add_state`: `render`'s sanitizer rebuilt every field of a declaration
/// except its **time clause**, which it cloned through untouched, so `during
/// <era>` rendered the anchor verbatim. `state x during "the-flood\nstate forged
/// during now"` came back as two `state` lines. The `Decl::Entity` half of the
/// same omission predates DM-4 and is reachable through `author.add_event`'s
/// `era`; both are covered below, along with the multi-point `order` form.
///
/// The `field.control-char` finding always fired on these and always barred
/// materialize, so no forged claim could reach canon — the defect was that the
/// *display* could lie while the module's contract said it could not. Both
/// defenses are asserted here, because either one alone is a single point of
/// failure.
#[cfg(feature = "syntax")]
#[test]
fn a_newline_in_a_time_anchor_cannot_forge_a_second_rendered_line() {
    use dli_ast::decl::{Decl, EntityDecl, EntityKind, StateDecl, TimeClause};
    use dli_ast::ident::Ident;
    use dli_serve::author::render::render_decls;

    let forged = "first-tides\nperson forged-by-newline \"Forged\"";
    let cases: Vec<(&str, Decl)> = vec![
        (
            "entity/during",
            Decl::Entity(EntityDecl {
                kind: EntityKind::Thing,
                name: Ident::synthetic("the-drowning"),
                label: None,
                time: Some(TimeClause::During(Ident::synthetic(forged))),
                slots: Vec::new(),
                provenance: None,
            }),
        ),
        (
            "state/during",
            Decl::State(StateDecl {
                name: Ident::synthetic("lancelot.oath-status"),
                value: None,
                time: Some(TimeClause::During(Ident::synthetic(forged))),
                provenance: None,
            }),
        ),
        (
            "state/until",
            Decl::State(StateDecl {
                name: Ident::synthetic("lancelot.oath-status"),
                value: None,
                time: Some(TimeClause::Until(Ident::synthetic(forged))),
                provenance: None,
            }),
        ),
        (
            "entity/order",
            Decl::Entity(EntityDecl {
                kind: EntityKind::Era,
                name: Ident::synthetic("arthurs-reign"),
                label: None,
                time: Some(TimeClause::Order(vec![
                    Ident::synthetic("first-light"),
                    Ident::synthetic(forged),
                ])),
                slots: Vec::new(),
                provenance: None,
            }),
        ),
    ];

    for (label, decl) in cases {
        let lines = render_decls(std::slice::from_ref(&decl));
        assert_eq!(
            lines.len(),
            1,
            "{label}: one staged declaration rendered as {} lines — the review \
             diff can be forged: {lines:#?}",
            lines.len()
        );
        assert!(
            !lines[0].contains('\n'),
            "{label}: the rendered line still carries a raw newline: {lines:#?}"
        );
        // The independent defense: the finding that bars materialize. A staged
        // declaration carrying this must never be committable, whatever the
        // renderer does with it.
        let mut store = Store::default();
        let opened = dispatch(
            &req(
                "1.3.0",
                "open",
                Op::AuthorOpen,
                json!({ "scope": "camelot" }),
            ),
            &mut store,
        );
        let ws = result(&opened)["workspace"].as_str().unwrap().to_owned();
        let added = dispatch(
            &req(
                "1.3.0",
                "1",
                Op::AuthorAddState,
                json!({
                    "workspace": ws,
                    "id": "lancelot.oath-status",
                    "time": { "kind": "during", "point": forged },
                    "provenance": { "kind": "proposed", "origin": "forgery-probe" },
                }),
            ),
            &mut store,
        );
        assert!(
            codes(&result(&added)).contains(&"field.control-char".to_owned()),
            "{label}: a control character in a time anchor must bar materialize: {:?}",
            result(&added)
        );
    }
}

// -- sanitization completeness, by walking rather than by listing -------------

/// **Every free-text field of every stageable declaration, mechanically.**
///
/// The `field.control-char` census and the render sanitizer are both
/// hand-maintained lists over a growing set of declaration kinds, and a
/// hand-maintained list is exactly the thing that silently stops being complete:
/// the time-clause gap above sat behind a catch-all whose comment said the authoring
/// seam never staged those kinds, which was true right up until these tools did.
///
/// So this does not enumerate fields. It takes one valid call per mutating tool,
/// walks the params for **every string leaf**, and re-sends the call with a
/// newline injected at that position — so a field added tomorrow is covered the
/// day it appears, without anyone remembering to add it here.
///
/// The invariant asserted at each position is the disjunction that is actually
/// true, rather than a stronger one that would be false for enum-valued fields:
///
/// - the call is **refused whole** (a `schema-violation` — nothing staged,
///   nothing rendered), **or**
/// - it stages, and then *both* defenses hold: the rendering is **at most one
///   line** (no forged second declaration in the review diff) **and**
///   `field.control-char` is among the findings (materialize is barred).
///
/// A field that is neither refused nor caught is a hole, and the failure names
/// the exact JSON pointer.
// The bulk is the fifteen call templates — a table, not logic. Splitting it
// would separate the templates from the invariant they are fed through.
#[allow(clippy::too_many_lines)]
#[test]
fn no_free_text_field_of_any_staged_declaration_can_forge_a_line() {
    let provenance = json!({ "kind": "proposed", "origin": "sanitize-sweep" });
    // One valid call per mutating tool. `workspace` is filled in per case.
    let cases: Vec<(Op, Value)> = vec![
        (
            Op::AuthorAddEntity,
            json!({"kind":"person","id":"elaine","name":"Elaine","provenance":provenance}),
        ),
        (
            Op::AuthorAddFact,
            json!({"entity":"elaine","key":"trade","value":"knight","provenance":provenance}),
        ),
        (
            Op::AuthorAddRelation,
            json!({"relation":"serves","from":"elaine","to":"bors","provenance":provenance}),
        ),
        (
            Op::AuthorAddEvent,
            json!({"id":"the-vigil","name":"The Vigil","era":"arthurs-reign","provenance":provenance}),
        ),
        (
            Op::AuthorAddBefore,
            json!({"earlier":"the-vigil","later":"the-quest","provenance":provenance}),
        ),
        (
            Op::AuthorOpenQuestion,
            json!({"id":"who-holds-the-ford","prompt":"Who holds the ford?"}),
        ),
        (
            Op::AuthorAddOption,
            json!({"question":"who-holds-the-ford","id":"the-tidewall","label":"The Tidewall"}),
        ),
        (
            Op::AuthorAddHypothesis,
            json!({"id":"coast-runs-north","statement":"The coast runs north."}),
        ),
        (
            Op::AuthorAddDialect,
            json!({"id":"cymric-fringe","label":"Cymric fringe","provenance":provenance,
                   "body":[{"Basis":{"attested":true,"text":"a real tongue"}},
                           {"Vowels":["a","e"]},{"Endings":["wyn"]},{"Foreign":["zh"]},
                           {"Stress":"initial"},
                           {"NameKind":{"kind":"person.given","patterns":[["given-proto"]]}},
                           {"NamePart":{"name":"wyn","gloss":"fair","head":["role","given-proto"]}}]}),
        ),
        (
            Op::AuthorAddNamePart,
            json!({"id":"wyn","gloss":"fair","head":["role","given-proto"],"provenance":provenance}),
        ),
        (
            Op::AuthorAddName,
            json!({"id":"gwenwyn","label":"Gwenwyn","head":["person.given","in","cymric-fringe"],
                   "composition":[{"role":"given-proto","morpheme":"wyn"}],
                   "bearer":"elaine","provenance":provenance}),
        ),
        (
            Op::AuthorAddTerm,
            json!({"id":"hedgeway","label":"the boundary road","provenance":provenance}),
        ),
        (
            Op::AuthorAddBanned,
            json!({"word":"sword","reason":"none here","provenance":provenance}),
        ),
        (
            Op::AuthorAddState,
            json!({"id":"elaine.oath-status","value":"sworn",
                   "time":{"kind":"during","point":"arthurs-reign"},"provenance":provenance}),
        ),
        (
            Op::AuthorAddAttest,
            json!({"question":"who-holds-the-ford","provenance":provenance}),
        ),
    ];

    let mut positions = 0;
    for (op, template) in &cases {
        // The uncorrupted call's rendering: the baseline every injected variant
        // is measured against.
        let clean_lines = {
            let (mut store, ws) = workspace("1.3.0");
            let _ = dispatch(
                &req(
                    "1.3.0",
                    "q",
                    Op::AuthorOpenQuestion,
                    json!({"workspace": ws, "id": "who-holds-the-ford",
                           "prompt": "Who holds the ford?"}),
                ),
                &mut store,
            );
            let mut params = template.clone();
            params["workspace"] = json!(ws);
            let response = dispatch(&req("1.3.0", "0", *op, params), &mut store);
            response
                .result
                .as_ref()
                .and_then(|r| r["rendered"].as_array())
                .map_or(0, Vec::len)
        };
        for pointer in string_leaves(template, "") {
            positions += 1;
            let (mut store, ws) = workspace("1.3.0");
            // `add_option`/`add_attest` need their question open first; opening
            // one always succeeds and is not the call under test.
            let _ = dispatch(
                &req(
                    "1.3.0",
                    "q",
                    Op::AuthorOpenQuestion,
                    json!({"workspace": ws, "id": "who-holds-the-ford",
                           "prompt": "Who holds the ford?"}),
                ),
                &mut store,
            );

            let mut params = template.clone();
            inject_newline(&mut params, &pointer);
            params["workspace"] = json!(ws);
            let response = dispatch(&req("1.3.0", "1", *op, params.clone()), &mut store);

            let where_ = format!("{op:?} at {pointer}");
            if !response.ok {
                // Refused whole: nothing staged, so nothing to forge.
                continue;
            }
            let result = response.result.clone().expect("ok carries a result");
            let lines = result["rendered"].as_array().cloned().unwrap_or_default();
            // A declaration may legitimately render as several lines — `dialect`
            // and `name` are blocks with an interior. So the forgery check is not
            // "one line"; it is that injecting text into a field cannot ADD a
            // line to the rendering, measured against the same call made clean.
            assert!(
                lines.len() <= clean_lines,
                "{where_}: injecting a newline grew the rendering from {clean_lines} to {} \
                 lines — the extra one is a declaration the author never made: {lines:#?}",
                lines.len()
            );
            for line in &lines {
                assert!(
                    !line.as_str().unwrap_or_default().contains('\n'),
                    "{where_}: a raw newline survived into the rendered line: {line}"
                );
            }
            // Defense two: materialize must be barred. Usually that is
            // `field.control-char` from the free-text census, but not always —
            // `add_option` with a corrupted `question` never reaches the census
            // because the question resolves to nothing first, and
            // `question.unknown` bars materialize instead. Either is a real bar;
            // demanding one specific code here would assert the mechanism rather
            // than the property.
            let errors: Vec<&Value> = result["findings"]
                .as_array()
                .map(|f| f.iter().filter(|f| f["severity"] == "error").collect())
                .unwrap_or_default();
            assert!(
                !errors.is_empty(),
                "{where_}: a control character was neither refused nor caught by any \
                 error-severity finding — materialize would accept it: {result}"
            );
            // Where the declaration actually staged AND rendered, the census is
            // the only thing standing between the control character and the
            // review diff, so there it must be the census that fired.
            if !lines.is_empty() {
                assert!(
                    codes(&result).contains(&"field.control-char".to_owned()),
                    "{where_}: a rendered declaration carried a control character that the \
                     free-text census did not name: {result}"
                );
            }
        }
    }
    // Non-vacuity: if the walker stopped finding leaves, every assertion above
    // would pass by never running.
    assert!(
        positions >= 70,
        "the walk covered only {positions} string positions; it reaches 80 across the \
         fifteen mutating tools, so a number far below that means the walker stopped \
         descending and every assertion above passed by never running"
    );
}

/// Every string leaf in a params value, as JSON pointers. `workspace` is skipped
/// — it names the target rather than carrying authored text, and corrupting it
/// only produces an unknown-workspace refusal.
fn string_leaves(value: &Value, prefix: &str) -> Vec<String> {
    match value {
        Value::String(_) => vec![prefix.to_owned()],
        Value::Array(items) => items
            .iter()
            .enumerate()
            .flat_map(|(i, item)| string_leaves(item, &format!("{prefix}/{i}")))
            .collect(),
        Value::Object(map) => map
            .iter()
            .filter(|(key, _)| key.as_str() != "workspace")
            .flat_map(|(key, item)| string_leaves(item, &format!("{prefix}/{key}")))
            .collect(),
        _ => Vec::new(),
    }
}

/// Splice a newline plus a plausible forged declaration into the string at
/// `pointer`. The payload is shaped like a real declaration precisely so that a
/// renderer which lets it through produces something a reviewer would believe.
fn inject_newline(value: &mut Value, pointer: &str) {
    if let Some(Value::String(text)) = value.pointer_mut(pointer) {
        *text = format!("{text}\nperson forged-by-sweep \"Forged\"");
    }
}
