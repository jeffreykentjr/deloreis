#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Contract smoke coverage: wire names and schema emission.

use dli_serve::CONTRACT_VERSION;
use dli_serve::ops::Op;
use dli_serve::schema::contract_schemas;

const EXPECTED: &[(Op, &str)] = &[
    (Op::Version, "dli.version"),
    (Op::Check, "dli.check"),
    (Op::Query, "dli.query"),
    (Op::ForkNew, "dli.fork.new"),
    (Op::ForkList, "dli.fork.list"),
    (Op::ForkDiff, "dli.fork.diff"),
    (Op::ForkSuppose, "dli.fork.suppose"),
    (Op::ForkAdd, "dli.fork.add"),
    (Op::ForkSuppress, "dli.fork.suppress"),
    (Op::ForkRestrict, "dli.fork.restrict"),
    (Op::ForkPreflight, "dli.fork.preflight"),
    (Op::ForkPromote, "dli.fork.promote"),
    (Op::CheckProof, "dli.check-proof"),
    (Op::CheckDraft, "dli.check-draft"),
    (Op::SimulateStart, "dli.simulate.start"),
    (Op::SimulateStatus, "dli.simulate.status"),
    (Op::SimulateCancel, "dli.simulate.cancel"),
    (Op::TimelineShow, "dli.timeline.show"),
    (Op::TimelineOrder, "dli.timeline.order"),
    (Op::TimelineKnows, "dli.timeline.knows"),
    (Op::TimelineEpisodes, "dli.timeline.episodes"),
    (Op::TimelinePhases, "dli.timeline.phases"),
    (Op::ExportRun, "dli.export.run"),
    (Op::NameCheck, "dli.name.check"),
    (Op::AttestMissing, "dli.attest.missing"),
    (Op::AttestationSweep, "dli.attestation-sweep"),
    (Op::PayloadSubmit, "dli.payload-submit"),
    (Op::AuthorOpen, "author.open"),
    (Op::AuthorStatus, "author.status"),
    (Op::AuthorDiff, "author.diff"),
    (Op::AuthorDiscard, "author.discard"),
    (Op::AuthorMaterialize, "author.materialize"),
    (Op::AuthorAddEntity, "author.add_entity"),
    (Op::AuthorAddFact, "author.add_fact"),
    (Op::AuthorAddRelation, "author.add_relation"),
    (Op::AuthorAddEvent, "author.add_event"),
    (Op::AuthorAddBefore, "author.add_before"),
    (Op::AuthorOpenQuestion, "author.open_question"),
    (Op::AuthorAddOption, "author.add_option"),
    (Op::AuthorAddHypothesis, "author.add_hypothesis"),
    (Op::AuthorSuppose, "author.suppose"),
    (Op::AuthorQuery, "author.query"),
    (Op::AuthorCheck, "author.check"),
    (Op::AuthorWhy, "author.why"),
    (Op::AuthorFindings, "author.findings"),
    (Op::AuthorListWorkspaces, "author.list_workspaces"),
];

#[test]
fn every_op_serializes_to_its_wire_name() {
    assert_eq!(EXPECTED.len(), Op::ALL.len());
    for (op, wire) in EXPECTED {
        let json = serde_json::to_string(op).unwrap();
        assert_eq!(json, format!("\"{wire}\""));
    }
}

#[test]
fn contract_schema_emission_covers_every_op() {
    let schemas = contract_schemas();
    assert!(
        schemas.len() > 25,
        "expected more than 25 schema entries, got {}",
        schemas.len()
    );
    assert_eq!(schemas.len(), Op::ALL.len() * 2);
}

#[test]
fn request_schemas_close_every_declared_object() {
    fn assert_closed(value: &serde_json::Value, path: &str) {
        match value {
            serde_json::Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    assert_closed(item, &format!("{path}/{index}"));
                }
            }
            serde_json::Value::Object(map) => {
                if map.contains_key("properties") {
                    assert_eq!(
                        map.get("additionalProperties"),
                        Some(&serde_json::Value::Bool(false)),
                        "declared request object remains open at {path}"
                    );
                }
                for (key, child) in map {
                    assert_closed(child, &format!("{path}/{key}"));
                }
            }
            _ => {}
        }
    }

    for (name, schema) in contract_schemas()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".request"))
    {
        assert_closed(schema.as_value(), &name);
    }
}

#[test]
fn attestation_schema_matches_runtime_cardinality_rules() {
    let schema = contract_schemas()
        .into_iter()
        .find(|(name, _)| name == "dli.attestation-sweep.request")
        .expect("attestation request schema")
        .1;
    let value = schema.as_value();
    assert_eq!(
        value.pointer("/properties/records/minItems"),
        Some(&1.into())
    );
    assert_eq!(
        value.pointer("/$defs/Sampling/properties/max_tokens/minimum"),
        Some(&1.into())
    );
    assert_eq!(
        value.pointer("/$defs/Sampling/properties/samples/minimum"),
        Some(&1.into())
    );
}

#[test]
fn golden_dispatch_matches() {
    // Drive a golden fixture request through the real dispatch and assert the
    // response equals the fixture's expectation. The exhaustive multi-step and
    // negative/raw replays live in `tests/dispatch.rs`; this is the contract-side
    // smoke that the wire dispatch is live.
    use dli_serve::author::store::Store;
    use dli_serve::serve::respond;
    use dli_serve::session::Sessions;
    use serde_json::Value;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/dli-worker/fixtures/golden/check-basic-v1.json");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let step = &fixture["steps"][0];

    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let line = serde_json::to_string(&step["send"]).unwrap();
    let response = respond(&line, &mut sessions, &mut store).expect("a frame is answered");
    let got = serde_json::to_value(&response).unwrap();
    assert_eq!(got, step["expect"]);
}

#[test]
fn contract_version_is_consistent_across_registry_and_handshake() {
    use dli_serve::author::store::Store;
    use dli_serve::serve::respond;
    use dli_serve::session::Sessions;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry =
        std::fs::read_to_string(root.join("contracts/dli-worker/registry.yaml")).unwrap();
    let registry: serde_yaml_ng::Value = serde_yaml_ng::from_str(&registry).unwrap();
    assert_eq!(
        registry["contract_version"].as_str(),
        Some(CONTRACT_VERSION)
    );

    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let frame = serde_json::json!({
        "dli": CONTRACT_VERSION,
        "id": "version-consistency",
        "op": "dli.version",
        "params": {}
    });
    let response = respond(&frame.to_string(), &mut sessions, &mut store).unwrap();
    assert_eq!(
        response.result.unwrap()["contract_version"],
        CONTRACT_VERSION
    );
}
