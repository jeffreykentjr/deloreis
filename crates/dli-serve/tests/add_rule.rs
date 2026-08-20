#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Contract 1.7.0's `author.add_rule` end to end (WS-2, rule authoring): the
//! declaration reaches the staged world, every fact it derives carries the
//! rule's own provenance (defeasible like any other claim), the pre-stage
//! checks refuse WITHOUT staging — this op's stricter gate, since staging a
//! bad rule even once bricks every later read — the derived-FD check
//! surfaces on `check` and blocks `materialize`, and `author.defeat` on the
//! rule's own citation kills every derivation through it, durably, through
//! the real wire.
//!
//! Also here: the version gate, the `1.7.0` capability list (now `1.6.0`'s
//! plus BOTH campaign ops), the idempotency/digest fences, the brick-proof
//! stratification pre-flight, and a migration-parity check against
//! `dli.corpus.import`'s text path — the same logical rule, authored two
//! ways, answers the same query the same way.

use serde_json::{Value, json};

use dli_ast::clause::Clause;
use dli_ast::decl::{Decl, Document, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::language::RatifyBody;
use dli_ast::span::Span;
use dli_serve::author::store::Store;
use dli_serve::author::{dispatch, workspace::Mode};
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_serve::{Op, Request};
use dli_world::pack::Pack;

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

fn ident(text: &str) -> Ident {
    Ident {
        text: text.to_owned(),
        span: Span::synthetic(),
    }
}

/// A wire `Term`: `$x` is a variable, anything else a symbol. `Ident`'s
/// custom deserializer accepts a bare string (synthetic span), so the
/// wrapping object needs no `span`/`text` split.
fn term(text: &str) -> Value {
    text.strip_prefix('$')
        .map_or_else(|| json!({ "Sym": text }), |var| json!({ "Var": var }))
}

/// A wire `AtomExpr`: `pred(args...)`.
fn atom(pred: &str, args: &[&str]) -> Value {
    json!({ "pred": pred, "args": args.iter().map(|a| term(a)).collect::<Vec<_>>() })
}

/// A positive wire `BodyLiteral`.
fn lit(pred: &str, args: &[&str]) -> Value {
    json!({ "atom": atom(pred, args), "negated": false })
}

/// A negated wire `BodyLiteral` (`not <atom>`).
fn not_lit(pred: &str, args: &[&str]) -> Value {
    json!({ "atom": atom(pred, args), "negated": true })
}

/// A base declaring `D-1`, a person `mara` with a certain `rank(mara, gold)`
/// on a `functional` predicate, plus an unrelated marker fact `special(mara)`
/// — enough for a harmless rule, an FD-breaking one, and a range-restriction
/// probe.
fn rule_base() -> Store {
    let source = "scope tpth\n\
        ratify D-1\n\
        person mara \"Mara\"  ratified D-1\n\
        functional rank  ratified D-1\n\
        rank(mara, gold)  ratified D-1\n\
        special(mara)  ratified D-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.is_empty(),
        "base source parses: {diagnostics:?}"
    );
    Store::with_pack(Pack {
        name: "tpth".to_owned(),
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

#[allow(clippy::too_many_arguments)]
fn add_rule(
    store: &mut Store,
    dli: &str,
    ws: &str,
    name: &str,
    head: &Value,
    body: &Value,
    provenance: &Value,
) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "add-rule",
            Op::AuthorAddRule,
            json!({
                "workspace": ws, "name": name, "head": head, "body": body,
                "provenance": provenance
            }),
        ),
        store,
    )
}

fn proposed(origin: &str) -> Value {
    json!({ "kind": "proposed", "origin": origin })
}

fn cited(decision: &str) -> Value {
    json!({ "kind": "cited", "decisions": [decision], "state": "ratified" })
}

fn check_codes(store: &mut Store, dli: &str, ws: &str) -> Vec<String> {
    let response = dispatch(
        &req(dli, "check", Op::AuthorCheck, json!({ "workspace": ws })),
        store,
    );
    codes(result(&response))
}

fn query_status(store: &mut Store, dli: &str, ws: &str, goal: &str) -> Option<String> {
    let response = dispatch(
        &req(
            dli,
            "q",
            Op::AuthorQuery,
            json!({ "workspace": ws, "goal": goal, "layer": "all" }),
        ),
        store,
    );
    result(&response)["rows"]
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(|row| row["status"].as_str())
        .map(ToOwned::to_owned)
}

// -- the happy path: staging, rendering, derivation ---------------------------

#[test]
fn add_rule_stages_cleanly_and_derives_through_the_staged_query() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    let response = add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "endorsement",
        &atom("endorsed", &["$x"]),
        &json!([lit("rank", &["$x", "gold"])]),
        &cited("D-1"),
    );
    assert!(
        codes(result(&response)).is_empty(),
        "{:?}",
        result(&response)
    );
    let rendered = result(&response)["rendered"].as_array().expect("rendered");
    assert_eq!(rendered.len(), 2, "{rendered:?}");
    assert!(
        rendered[0]
            .as_str()
            .unwrap()
            .starts_with("rule endorsement")
    );
    assert!(
        rendered[1]
            .as_str()
            .unwrap()
            .contains("endorsed($x) :- rank($x, gold)"),
        "{rendered:?}"
    );
    assert_eq!(
        query_status(&mut store, "1.7.0", &ws, "endorsed(mara)").as_deref(),
        Some("certain"),
        "the rule's own derivation is visible over the staged world immediately"
    );
}

#[test]
fn the_declaration_stages_a_rule_decl() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "endorsement",
        &atom("endorsed", &["$x"]),
        &json!([lit("rank", &["$x", "gold"])]),
        &proposed("s"),
    );
    let workspace = store.get(&ws).expect("workspace is open");
    assert_eq!(workspace.mode, Mode::Scope);
    assert_eq!(workspace.decls.len(), 1, "{:?}", workspace.decls);
    assert!(
        matches!(workspace.decls[0], Decl::Rule(_)),
        "{:?}",
        workspace.decls[0]
    );
}

#[test]
fn negation_lowers_and_derives() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    let response = add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "unbadged",
        &atom("unbadged", &["$x"]),
        &json!([lit("special", &["$x"]), not_lit("badged", &["$x"])]),
        &cited("D-1"),
    );
    assert!(
        codes(result(&response)).is_empty(),
        "{:?}",
        result(&response)
    );
    assert_eq!(
        query_status(&mut store, "1.7.0", &ws, "unbadged(mara)").as_deref(),
        Some("certain"),
        "no badged(mara) fact exists, so the negated literal never poisons"
    );
}

// -- pre-stage checks: refuse WITHOUT staging ---------------------------------

#[test]
fn an_unbound_head_variable_is_refused_without_staging() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    let response = add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "unsafe-rule",
        &atom("risky", &["$x"]),
        &json!([lit("special", &["$y"])]),
        &proposed("s"),
    );
    let cs = codes(result(&response));
    assert_eq!(cs, vec!["rule.unbound-variable".to_owned()], "{cs:?}");
    assert!(
        result(&response)["rendered"]
            .as_array()
            .expect("rendered")
            .is_empty(),
        "refused without staging: nothing rendered"
    );
    let workspace = store.get(&ws).expect("workspace is open");
    assert!(
        workspace.decls.is_empty(),
        "the bad rule never reached the workspace: {:?}",
        workspace.decls
    );
}

/// rt-features SEV-C: an empty body would otherwise stage clean and derive
/// its head unconditionally (fact-as-rule) — but its render (`head :- `)
/// reparses via the `.dli` text path to `datalog: None` (`parse_rule_clause`
/// requires at least one body atom after the separator), which the loader
/// then refuses as `MalformedWorldRule`. A wire-staged rule the reimport path
/// would refuse is a migration-parity break, so this refuses before staging,
/// mirroring every other pre-stage rule-shape check.
#[test]
fn an_empty_body_is_refused_without_staging() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    let response = add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "vacuous-rule",
        &atom("always", &["mara"]),
        &json!([]),
        &proposed("s"),
    );
    let cs = codes(result(&response));
    assert_eq!(cs, vec!["rule.empty-body".to_owned()], "{cs:?}");
    assert!(
        result(&response)["rendered"]
            .as_array()
            .expect("rendered")
            .is_empty(),
        "refused without staging: nothing rendered"
    );
    let workspace = store.get(&ws).expect("workspace is open");
    assert!(
        workspace.decls.is_empty(),
        "the empty-bodied rule never reached the workspace: {:?}",
        workspace.decls
    );
}

/// rt-features SEV-C's migration-parity claim, checked directly: a non-empty
/// rule's rendered clause line, fed back through the same `.dli` text parser
/// the loader uses on reimport (`dli_syntax::parse_rule_clause`), structures
/// to the identical head and body the wire call submitted. This is the
/// property the empty-body case above violates and the new guard restores.
#[test]
fn a_non_empty_rules_render_round_trips_through_the_text_parser() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    let head = atom("endorsed", &["$x"]);
    let body = json!([lit("rank", &["$x", "gold"])]);
    let response = add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "endorsement",
        &head,
        &body,
        &cited("D-1"),
    );
    assert!(
        codes(result(&response)).is_empty(),
        "{:?}",
        result(&response)
    );
    let rendered = result(&response)["rendered"].as_array().expect("rendered");
    let clause_line = rendered[1].as_str().expect("clause line").trim();
    let clause = Clause {
        text: clause_line.to_owned(),
        span: Span::synthetic(),
    };
    let (parsed_head, parsed_body) =
        dli_syntax::parse_rule_clause(&clause).expect("the rendered clause reparses as a rule");
    // Reparsing mints real spans where the submitted wire params carried
    // synthetic ones; the round-trip claim is about structure (predicate and
    // variable/symbol names), not span provenance, so strip `span` before
    // comparing.
    assert_eq!(
        strip_spans(serde_json::to_value(&parsed_head).unwrap()),
        strip_spans(head),
        "the reparsed head matches what was submitted"
    );
    assert_eq!(
        strip_spans(serde_json::to_value(&parsed_body).unwrap()),
        strip_spans(body),
        "the reparsed body matches what was submitted"
    );
}

/// Recursively drop every `span` key so two ASTs that differ only in span
/// provenance (synthetic vs. freshly parsed) compare equal on structure. Also
/// collapses a span-stripped `Ident` (`{"text": "x"}`) down to the bare
/// string `"x"`, matching the shape the wire helpers in this file submit —
/// `Ident`'s deserializer accepts either, but serialization always produces
/// the full object.
fn strip_spans(mut value: Value) -> Value {
    match &mut value {
        Value::Object(map) => {
            map.remove("span");
            for v in map.values_mut() {
                *v = strip_spans(v.take());
            }
            if map.len() == 1
                && let Some(text) = map.get("text").and_then(Value::as_str)
            {
                return Value::String(text.to_owned());
            }
        }
        Value::Array(items) => {
            for v in items.iter_mut() {
                *v = strip_spans(v.take());
            }
        }
        _ => {}
    }
    value
}

#[test]
fn a_duplicate_rule_name_is_refused_without_staging() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    let first = add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "endorsement",
        &atom("endorsed", &["$x"]),
        &json!([lit("rank", &["$x", "gold"])]),
        &proposed("s"),
    );
    assert!(codes(result(&first)).is_empty(), "{:?}", result(&first));
    let second = add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "endorsement",
        &atom("endorsed2", &["$x"]),
        &json!([lit("special", &["$x"])]),
        &proposed("s"),
    );
    let cs = codes(result(&second));
    assert_eq!(cs, vec!["rule.duplicate".to_owned()], "{cs:?}");
    let workspace = store.get(&ws).expect("workspace is open");
    assert_eq!(
        workspace.decls.len(),
        1,
        "only the first rule ever staged: {:?}",
        workspace.decls
    );
}

/// The brick-proof test (WS-2): a stratification failure refuses WITHOUT
/// staging, so a bad rule can never brick every subsequent read. Two mutually
/// negative-dependent rules (`p :- not q`, `q :- not p`) admit no valid
/// stratification.
#[test]
fn an_unstratifiable_rule_is_refused_without_staging_and_reads_stay_healthy() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "p-rule",
        &atom("p", &[]),
        &json!([not_lit("q", &[])]),
        &cited("D-1"),
    );
    let second = add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "q-rule",
        &atom("q", &[]),
        &json!([not_lit("p", &[])]),
        &proposed("s"),
    );
    let cs = codes(result(&second));
    assert_eq!(cs, vec!["rule.unstratifiable".to_owned()], "{cs:?}");
    let workspace = store.get(&ws).expect("workspace is open");
    assert_eq!(
        workspace.decls.len(),
        1,
        "only p-rule ever staged; q-rule never reached the workspace: {:?}",
        workspace.decls
    );
    // The brick-proof property itself: a read after the refused add still
    // answers normally rather than collapsing to `world.uncompilable`.
    let check = check_codes(&mut store, "1.7.0", &ws);
    assert!(
        !check.contains(&"world.uncompilable".to_owned()),
        "the refused add left the workspace healthy: {check:?}"
    );
    assert_eq!(
        query_status(&mut store, "1.7.0", &ws, "p()").as_deref(),
        Some("certain"),
        "p-rule alone (negating a predicate nothing derives) still evaluates fine"
    );
}

#[test]
fn a_citation_to_an_undeclared_decision_refuses_without_staging() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    let response = add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "endorsement",
        &atom("endorsed", &["$x"]),
        &json!([lit("rank", &["$x", "gold"])]),
        &cited("D-999"),
    );
    let cs = codes(result(&response));
    assert_eq!(cs, vec!["citation.unknown-decision".to_owned()], "{cs:?}");
    let workspace = store.get(&ws).expect("workspace is open");
    assert!(workspace.decls.is_empty(), "{:?}", workspace.decls);
}

// -- the post-merge / post-derivation FD check --------------------------------

#[test]
fn a_rule_derived_fd_violation_surfaces_on_check_and_blocks_materialize() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    let response = add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "silver-if-special",
        &atom("rank", &["$x", "silver"]),
        &json!([lit("special", &["$x"])]),
        &cited("D-1"),
    );
    assert!(
        codes(result(&response)).is_empty(),
        "{:?}",
        result(&response)
    );

    let check = check_codes(&mut store, "1.7.0", &ws);
    assert!(
        check.contains(&"continuity.derived-fd-violation".to_owned()),
        "author.check surfaces the rule-derived FD break: {check:?}"
    );

    let blocked = dispatch(
        &req(
            "1.7.0",
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws }),
        ),
        &mut store,
    );
    assert!(!blocked.ok, "materialize is refused: {blocked:?}");
    let error = blocked.error.expect("blocked carries an error");
    assert_eq!(error.code, dli_serve::ErrorCode::MaterializeBlocked);
    let blocking = error.data.expect("blocking data");
    assert_eq!(
        blocking["blocking"],
        json!(["continuity.derived-fd-violation"])
    );
}

// -- author.diff shows the staged rule ----------------------------------------

#[test]
fn author_diff_rows_show_the_staged_rule_declaration() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    add_rule(
        &mut store,
        "1.7.0",
        &ws,
        "endorsement",
        &atom("endorsed", &["$x"]),
        &json!([lit("rank", &["$x", "gold"])]),
        &proposed("origin-tag"),
    );
    let diff = dispatch(
        &req("1.7.0", "d", Op::AuthorDiff, json!({ "workspace": ws })),
        &mut store,
    );
    let rendered = result(&diff)["rendered"]
        .as_array()
        .expect("rendered lines");
    assert_eq!(rendered.len(), 2, "{rendered:?}");
    assert!(
        rendered[0]
            .as_str()
            .unwrap_or_default()
            .contains("rule endorsement"),
        "{rendered:?}"
    );
}

// -- the version gate ----------------------------------------------------

#[test]
fn add_rule_is_unknown_before_contract_1_7_0() {
    for dli in ["0.5.1", "1.0.0", "1.3.0", "1.5.0", "1.6.0"] {
        let mut sessions = Sessions::new();
        let mut store = Store::default();
        let response = dli_serve::serve::dispatch(
            &req(
                dli,
                "add-rule",
                Op::AuthorAddRule,
                json!({
                    "workspace": "ws-1", "name": "r", "head": atom("p", &[]), "body": [],
                    "provenance": { "kind": "proposed", "origin": "s" }
                }),
            ),
            &mut sessions,
            &mut store,
        );
        assert!(!response.ok, "{dli} must refuse the op: {response:?}");
        let error = response.error.expect("refusal carries an error");
        assert_eq!(
            error.code,
            dli_serve::ErrorCode::UnknownOp,
            "{dli} refuses as unknown-op"
        );
        assert!(
            error.message.contains("1.7.0"),
            "{dli}'s refusal names the version that introduced it: {}",
            error.message
        );
    }
}

#[test]
fn the_1_7_0_capability_list_is_1_6_0_plus_both_campaign_ops() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let cap = |dli: &str, sessions: &mut Sessions, store: &mut Store| -> Vec<String> {
        let response = respond(
            &serde_json::to_string(
                &json!({"dli": dli, "id": "v", "op": "dli.version", "params": {}}),
            )
            .unwrap(),
            sessions,
            store,
        )
        .unwrap();
        response.result.unwrap()["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect()
    };
    let at_1_6_0 = cap("1.6.0", &mut sessions, &mut store);
    let at_1_7_0 = cap("1.7.0", &mut sessions, &mut store);
    assert_eq!(
        at_1_7_0.len(),
        at_1_6_0.len() + 2,
        "{at_1_6_0:?} vs {at_1_7_0:?}"
    );
    let mut expected = at_1_6_0.clone();
    expected.push("author.assert_same_as".to_owned());
    expected.push("author.add_rule".to_owned());
    assert_eq!(
        at_1_7_0, expected,
        "both new ops land at the END of the 1.6.0 list, in append order"
    );
}

// -- idempotency and digest fences --------------------------------------------

#[test]
fn a_stale_digest_fence_refuses() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    let mut stale = req(
        "1.7.0",
        "add-rule",
        Op::AuthorAddRule,
        json!({
            "workspace": ws, "name": "endorsement", "head": atom("endorsed", &["$x"]),
            "body": [lit("rank", &["$x", "gold"])], "provenance": proposed("s")
        }),
    );
    stale.expected_digest = Some("jcs:sha256:deadbeef".to_owned());
    let response = dispatch(&stale, &mut store);
    assert!(!response.ok);
    assert_eq!(
        response.error.expect("error").code,
        dli_serve::ErrorCode::StaleFence
    );
}

#[test]
fn an_idempotent_retry_replays_and_stages_nothing() {
    let mut store = rule_base();
    let ws = open_scope(&mut store, "1.7.0", "tpth");
    let mut call = req(
        "1.7.0",
        "add-rule",
        Op::AuthorAddRule,
        json!({
            "workspace": ws, "name": "endorsement", "head": atom("endorsed", &["$x"]),
            "body": [lit("rank", &["$x", "gold"])], "provenance": proposed("s")
        }),
    );
    call.idempotency_key = Some("idem-add-rule-1".to_owned());
    let first = dispatch(&call, &mut store);
    let second = dispatch(&call, &mut store);
    assert_eq!(result(&second), result(&first));

    let diff = dispatch(
        &req("1.7.0", "d", Op::AuthorDiff, json!({ "workspace": ws })),
        &mut store,
    );
    assert_eq!(
        result(&diff)["rendered"].as_array().map(Vec::len),
        Some(2),
        "the retry staged nothing new: {:?}",
        result(&diff)
    );
}

// -- migration parity: wire rule vs corpus.import text ------------------------

/// The wire's typed `author.add_rule` and `dli.corpus.import`'s `.dli` text
/// path both reach the loader's same `process_rule`; the same logical rule
/// authored either way must answer the same query the same way (WS-2).
#[test]
fn a_wire_authored_rule_matches_a_corpus_imported_one_on_the_same_query() {
    // Path A: author.add_rule on a fresh workspace over the shared base.
    let mut wire_store = rule_base();
    let wire_ws = open_scope(&mut wire_store, "1.7.0", "tpth");
    let staged = add_rule(
        &mut wire_store,
        "1.7.0",
        &wire_ws,
        "endorsement",
        &atom("endorsed", &["$x"]),
        &json!([lit("rank", &["$x", "gold"])]),
        &cited("D-1"),
    );
    assert!(codes(result(&staged)).is_empty(), "{:?}", result(&staged));
    let wire_status = query_status(&mut wire_store, "1.7.0", &wire_ws, "endorsed(mara)");

    // Path B: the identical logical corpus, rule included as `.dli` text,
    // imported through `dli.corpus.import`'s syntax-build text path. Corpus
    // import is a durable-evidence operation (`ProjectRequired`), so it needs
    // a real project: a manifest naming a store-backed document, with the
    // store directory pre-created ("seed from nothing" means from an EMPTY
    // store, never from no store at all).
    let root = tempfile::tempdir().expect("temporary project root");
    std::fs::create_dir_all(root.path().join("lorestore/tpth")).expect("pre-create store dir");
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"migration-parity\"\n\n[documents.tpth]\nstore = \"lorestore/tpth\"\n",
    )
    .expect("write project manifest");
    let project =
        std::sync::Arc::new(dli_serve::project::Project::load(root.path()).expect("load project"));
    let mut sessions = Sessions::from_project(std::sync::Arc::clone(&project));
    let mut import_store = Store::default();
    let imported = dli_serve::serve::dispatch(
        &req(
            "1.7.0",
            "import",
            Op::CorpusImport,
            json!({
                "document": "tpth",
                "mode": "init",
                "dli_documents": [{
                    "id": "tpth",
                    "text": "scope tpth\n\
                        ratify D-1\n\
                        person mara \"Mara\"  ratified D-1\n\
                        functional rank  ratified D-1\n\
                        rank(mara, gold)  ratified D-1\n\
                        special(mara)  ratified D-1\n\
                        rule endorsement  ratified D-1\n\
                        \x20 endorsed($x) :- rank($x, gold)\n"
                }],
            }),
        ),
        &mut sessions,
        &mut import_store,
    );
    assert!(imported.ok, "{imported:?}");
    drop(import_store);

    // `dli.corpus.import`'s own store never re-reads what it just wrote (the
    // session that imports is not the session that authors, in general) — a
    // FRESH project load picks up the new commit, mirroring exactly how the
    // real server builds a store-backed document's author store
    // (`serve::project_author_store`): `Store::with_packs` over each
    // document's compiled pack, `register_store_origin` for its lorestore
    // origin, then `attach_lorestore` over the project's own store root.
    let reread = dli_serve::project::Project::load(root.path()).expect("reread project");
    let packs = reread.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    let mut import_store = Store::with_packs(packs).expect("compile the imported corpus");
    for (id, document) in reread.documents() {
        if let Some(origin) = &document.store_origin {
            import_store
                .register_store_origin(
                    id.to_owned(),
                    origin.clone(),
                    document.store_pinned,
                    document
                        .store_commit
                        .clone()
                        .expect("a store-backed document carries its selected commit"),
                    document.store_valuation.clone(),
                    document.store_unassigned.clone(),
                    document.store_ref_digest.clone(),
                )
                .expect("store valuation maps by name");
        }
    }
    import_store
        .attach_lorestore(root.path().to_path_buf())
        .expect("attach the imported lorestore");

    let import_ws = open_scope(&mut import_store, "1.7.0", "tpth");
    let import_status = query_status(&mut import_store, "1.7.0", &import_ws, "endorsed(mara)");

    assert_eq!(
        wire_status, import_status,
        "the wire-authored and corpus-imported rules answer the same query the same way"
    );
    assert_eq!(wire_status.as_deref(), Some("certain"));
}

// -- the durable-defeat-un-merges wire test (rule edition) --------------------
//
// The rule-authoring twin of `assert_same_as.rs`'s durable-defeat test:
// `support_cites_decision`'s expert_body branch (WS-2) deserializes the
// carried `Decl::Rule` to find its citation — the rule carriage's own
// `support` is a structural `truth()` constant, never touched, so this is
// the only place that citation is recoverable. `author.defeat` on the
// rule's own citing decision must kill every derivation through it, durably.

fn ledger_pack() -> Pack {
    fn id(text: &str) -> Ident {
        ident(text)
    }
    Pack {
        name: "demo".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("demo"),
                span: Span::synthetic(),
            }),
            decls: vec![
                Decl::Ratify(RatifyDecl {
                    decision: id("D-1"),
                    body: RatifyBody::default(),
                }),
                Decl::Ratify(RatifyDecl {
                    decision: id("D-2"),
                    body: RatifyBody::default(),
                }),
                Decl::Ratify(RatifyDecl {
                    decision: id("D-3"),
                    body: RatifyBody::default(),
                }),
            ],
        }],
    }
}

fn ledger_request(id: &str, op: Op, params: Value) -> Request {
    req("1.7.0", id, op, params)
}

fn ledger_result(response: dli_serve::Response) -> Value {
    assert!(response.ok, "request failed: {response:?}");
    response.result.expect("successful request has a result")
}

#[test]
#[allow(clippy::too_many_lines)]
fn durable_defeat_of_a_rule_claim_kills_every_derivation_through_it() {
    let root = tempfile::tempdir().expect("temporary store root");
    let lorestore_path = root.path().join("lorestore");
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"contract-1-7-rule\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\n",
    )
    .expect("write project manifest");
    let mut store = Store::with_pack(ledger_pack()).expect("compile demo pack");
    store
        .attach_lorestore(lorestore_path.clone())
        .expect("attach demo lorestore");

    let opened = ledger_result(dispatch(
        &ledger_request("open", Op::AuthorOpen, json!({"scope": "demo"})),
        &mut store,
    ));
    let workspace = opened["workspace"].as_str().unwrap().to_owned();

    ledger_result(dispatch(
        &ledger_request(
            "mara",
            Op::AuthorAddEntity,
            json!({
                "workspace": workspace, "kind": "person", "id": "mara", "name": "Mara",
                "provenance": {"kind": "cited", "decisions": ["D-1"], "state": "ratified"}
            }),
        ),
        &mut store,
    ));
    ledger_result(dispatch(
        &ledger_request(
            "special",
            Op::AuthorAddFact,
            json!({
                "workspace": workspace, "entity": "mara", "key": "special", "value": "yes",
                "provenance": {"kind": "cited", "decisions": ["D-1"], "state": "ratified"}
            }),
        ),
        &mut store,
    ));
    // The rule cites D-3 — a DIFFERENT decision than the base fact (D-1) —
    // so defeating the rule alone leaves mara's own facts untouched.
    let rule_added = ledger_result(dispatch(
        &ledger_request(
            "rule",
            Op::AuthorAddRule,
            json!({
                "workspace": workspace, "name": "endorsement",
                "head": atom("endorsed", &["$x"]),
                "body": [lit("special", &["$x", "yes"])],
                "provenance": {"kind": "cited", "decisions": ["D-3"], "state": "ratified"}
            }),
        ),
        &mut store,
    ));
    assert!(
        rule_added["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["severity"] != "error"),
        "{rule_added:?}"
    );

    let materialized = ledger_result(dispatch(
        &ledger_request(
            "materialize",
            Op::AuthorMaterialize,
            json!({"workspace": workspace, "rendered": false}),
        ),
        &mut store,
    ));
    let rule_claim_id = materialized["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| {
            fact["expert_body"]["kind"] == "rule" && fact["expert_body"]["name"] == "endorsement"
        })
        .and_then(|fact| fact["claim_id"].as_str())
        .expect("the durably stored rule's claim id")
        .to_owned();

    let before = ledger_result(dispatch(
        &ledger_request(
            "q1",
            Op::AuthorQuery,
            json!({ "workspace": workspace, "goal": "endorsed(mara)", "layer": "all" }),
        ),
        &mut store,
    ));
    let before_rows = before["rows"].as_array().expect("rows");
    assert!(
        !before_rows.is_empty() && before_rows[0]["status"] == "certain",
        "before defeat, the rule's derivation is certain through the wire: {before:?}"
    );

    let defeated = ledger_result(dispatch(
        &ledger_request(
            "defeat",
            Op::AuthorDefeat,
            json!({
                "workspace": workspace,
                "claim_id": rule_claim_id,
                "decision": "D-2",
                "defeated_decision": "D-3"
            }),
        ),
        &mut store,
    ));
    assert_eq!(defeated["findings"], json!([]), "{defeated:?}");

    let after = ledger_result(dispatch(
        &ledger_request(
            "q2",
            Op::AuthorQuery,
            json!({ "workspace": workspace, "goal": "endorsed(mara)", "layer": "all" }),
        ),
        &mut store,
    ));
    assert_eq!(
        after["rows"],
        json!([]),
        "the durable defeat kills the rule's derivation through the wire: {after:?}"
    );

    // The surface fact is untouched: mara's own `special` fact is still there.
    let surface = ledger_result(dispatch(
        &ledger_request(
            "q3",
            Op::AuthorQuery,
            json!({ "workspace": workspace, "goal": "special(mara, yes)", "layer": "all" }),
        ),
        &mut store,
    ));
    let surface_rows = surface["rows"].as_array().expect("rows");
    assert!(
        !surface_rows.is_empty() && surface_rows[0]["status"] == "certain",
        "mara's own fact never depended on the rule's citation: {surface:?}"
    );
}
