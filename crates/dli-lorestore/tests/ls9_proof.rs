//! LS-9 proof-interior oracle.
//!
//! A schema-v1 **string-bodied** `proof` carriage — the shape stored before LS-9
//! structured the proof interior — must, on read, upcast to the v2 typed carriage
//! byte-identically, and the reconstructed `ProofDecl` must check to the same
//! `ProofReport` as the directly-parsed proof. Two levels of proof:
//!
//! 1. *Object fidelity* — a proof carriage downgraded to its v1 `Vec<Clause>`
//!    string body upcasts (via `deepen_proof_carriage`) to a fact object
//!    byte-identical (by content hash) to the v2 carriage the front end now
//!    produces, and its embedded declaration deepens to the typed `ProofBody`.
//!    The naive read (no upcast) is shown to fail, so the upcast is load-bearing.
//! 2. *Checked fidelity* — the `check_proof` verdict, findings, grade, and digest
//!    over the v1-upcast-reconstructed proof equal those over the direct proof.
//!    This is the proof-specific parity the whole-pack compile oracle cannot
//!    reach: world compilation loads a proof but does not run the ledger checker.
//!
//! Both fail against the pre-fix state: pre-LS-9 there is no typed `ProofBody`,
//! so a string body is the only shape and nothing upcasts.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::Value;

use dli_ast::decl::{Decl, ProofDecl};
use dli_ast::proof::ProofOp;
use dli_ledger::ledger_check::check_proof;
use dli_ledger::practice::compile_practices;
use dli_ledger::premise::SceneProjection;
use dli_lorestore::SCHEMA_VERSION;
use dli_lorestore::lower::lower_pack;
use dli_lorestore::object::{ExpertDeclKind, FactObject};
use dli_lorestore::upcast::upcast_value;
use dli_world::Pack;

/// A self-contained pack whose one proof exercises every proof form: a world
/// premise, a timing premise, a resource draw, an instantiate step, a chain
/// step, a spend, and a close.
const PROOF_SRC: &str = "\
scope magic
cycle tide(4) = ebb | slack | flood | high  proposed
practice spoken-proof proposed {
  kind ground = door | field | ore
  resource debt : linear conserved
  rule ward-form(ground, fibre)
  rule give-and-take
  error fizzle refines L2 premise-not-available where absent
  error wrong-hour refines L3 side-condition [timing] over cycle tide
  cost material
}
proof e01-s04 speaker protagonist in spoken-proof {
  premise p1 holds(enfys, consent(household, door)) cited D-0113 cue \"line 112\"
  premise p2 phase(tide, slack) from-scene cue \"line 118\"
  premise p3 debt(1) drawn-against p1
  step s1 instantiate ward-form with ground = door, fibre = withy
  step s2 chain from p1, s1 by give-and-take conclusion bound(ward, door)
  spend p3
  close bound(ward, door) expect holds
}
";

fn proof_pack() -> Pack {
    let (document, diagnostics) = dli_syntax::parse_document(PROOF_SRC);
    assert!(
        diagnostics.is_empty(),
        "pack must parse clean: {diagnostics:?}"
    );
    Pack {
        name: "magic".to_owned(),
        documents: vec![document],
    }
}

fn is_proof_carriage(fo: &FactObject) -> bool {
    fo.expert_body
        .as_ref()
        .is_some_and(|b| b.kind == ExpertDeclKind::Proof)
}

/// The scene the worked casting is spoken in: the consent premise seated and the
/// tide at slack, so `e01-s04` reaches a clean hold.
fn scene() -> SceneProjection {
    SceneProjection::new()
        .seat("holds(enfys, consent(household, door))")
        .at_phase("tide", "slack")
}

/// Rewrite a v2 proof-carriage fact object into the schema-v1 JSON a pre-LS-9
/// store held: `schema_version` 1, and the embedded declaration's body reverted
/// from the typed `ProofBody` to the old `Vec<Clause>` string body (exactly the
/// `ProofClause::clause` values, in order).
fn downgrade_to_v1(fo: &FactObject) -> Value {
    let mut value = serde_json::to_value(fo).expect("fact object serializes");
    value["schema_version"] = Value::from(1);

    let embedded = value["expert_body"]["body"][0]
        .as_str()
        .expect("carriage body is a JSON string")
        .to_owned();
    let decl_v2: Value = serde_json::from_str(&embedded).expect("embedded decl is JSON");
    let proof = decl_v2.get("Proof").expect("carriage is a proof decl");

    let v1_body: Vec<Value> = proof
        .get("body")
        .and_then(|b| b.get("items"))
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|item| item["clause"].clone()).collect())
        .unwrap_or_default();

    let mut proof_v1 = proof.clone();
    proof_v1["body"] = Value::Array(v1_body);
    let decl_v1 = serde_json::json!({ "Proof": proof_v1 });
    value["expert_body"]["body"][0] =
        Value::from(serde_json::to_string(&decl_v1).expect("v1 decl serializes"));
    value
}

fn embedded_proof(fo: &FactObject) -> ProofDecl {
    // `carriage_json` is the reconstruction path: the located body when the
    // object has one, so source positions survive the store (DL-0055). Reading
    // `expert_body.body` here instead would rebuild the proof from the
    // span-normalized bytes and quietly zero every span in the report.
    let body = fo.carriage_json().expect("a carriage object has a body");
    match serde_json::from_str(body).expect("embedded decl decodes") {
        Decl::Proof(proof) => proof,
        _ => panic!("carriage is a proof"),
    }
}

#[test]
fn v1_proof_carriage_upcasts_to_the_exact_v2_object() {
    let pack = proof_pack();
    let (objects, _names) = lower_pack(&pack);
    let v2_carriage = objects
        .iter()
        .find(|fo| is_proof_carriage(fo))
        .expect("the pack carries a proof");

    let v1_value = downgrade_to_v1(v2_carriage);

    // The v1 embedded declaration must NOT decode as a current `Decl` — the
    // string body (a JSON array) is not the typed `ProofBody` (a struct). This is
    // what makes the upcast load-bearing rather than cosmetic.
    let v1_embedded = v1_value["expert_body"]["body"][0].as_str().unwrap();
    assert!(
        serde_json::from_str::<Decl>(v1_embedded).is_err(),
        "a v1 string-bodied proof decl must not decode as a typed v2 Decl without upcast"
    );

    // Upcast v1 → current, then decode: it must equal the v2 carriage exactly
    // (same content address), and its interior must be the typed `ProofBody`.
    let lifted = upcast_value(1, v1_value).expect("v1 carriage upcasts");
    let lifted_fo: FactObject = serde_json::from_value(lifted).expect("lifted carriage decodes");

    assert_eq!(
        lifted_fo.content_hash().expect("hash"),
        v2_carriage.content_hash().expect("hash"),
        "the upcast v1 proof carriage must be byte-identical to the v2 carriage"
    );
    assert_eq!(lifted_fo.schema_version, SCHEMA_VERSION);

    // The interior is typed after upcast: the first premise pre-parses its atom,
    // and the close carries its parsed claim and verdict.
    let proof = embedded_proof(&lifted_fo);
    let first = &proof.body.items[0];
    assert!(
        matches!(&first.op, Some(ProofOp::Premise(p)) if p.label == "p1"),
        "the first op is the typed premise p1 after upcast"
    );
    assert!(
        proof
            .body
            .items
            .iter()
            .any(|item| matches!(&item.op, Some(ProofOp::Close(c)) if c.verdict.as_deref() == Some("holds"))),
        "the close is structured with its expect verdict after upcast"
    );
}

#[test]
fn a_v1_proof_carriage_checks_identically_after_upcast() {
    let pack = proof_pack();

    // reference A: the directly-parsed proof, checked against its scene.
    let document = &pack.documents[0];
    let practices = compile_practices(document);
    let practice = practices
        .iter()
        .find(|p| p.name.text == "spoken-proof")
        .expect("the spoken-proof practice compiles");
    let direct = document
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Proof(p) => Some(p),
            _ => None,
        })
        .expect("one proof");
    let report_a = check_proof(practice, direct, &scene());
    assert!(
        report_a.checks,
        "the worked proof checks clean: {:?}",
        report_a
            .findings
            .iter()
            .map(dli_ledger::ledger_check::ProofFinding::message)
            .collect::<Vec<_>>()
    );

    // reference B: the same proof stored v1, upcast on read, then checked.
    let (objects, _names) = lower_pack(&pack);
    let v2_carriage = objects.iter().find(|fo| is_proof_carriage(fo)).unwrap();
    let lifted = upcast_value(1, downgrade_to_v1(v2_carriage)).expect("upcast");
    let lifted_fo: FactObject = serde_json::from_value(lifted).expect("decode");
    let reconstructed = embedded_proof(&lifted_fo);
    let report_b = check_proof(practice, &reconstructed, &scene());

    // Byte-identical reports: the whole verdict, findings, grade, and shape.
    assert_eq!(
        serde_json::to_string(&report_a).unwrap(),
        serde_json::to_string(&report_b).unwrap(),
        "the v1-upcast proof must check to a byte-identical ProofReport"
    );
}
