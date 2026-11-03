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
    (Op::TimelineShow, "dli.timeline.show"),
    (Op::TimelineOrder, "dli.timeline.order"),
    (Op::TimelineKnows, "dli.timeline.knows"),
    (Op::TimelineEpisodes, "dli.timeline.episodes"),
    (Op::TimelinePhases, "dli.timeline.phases"),
    (Op::ExportRun, "dli.export.run"),
    (Op::NameCheck, "dli.name.check"),
    (Op::AttestMissing, "dli.attest.missing"),
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
    (Op::AuthorBranch, "author.branch"),
    (Op::AuthorMerge, "author.merge"),
    (Op::CorpusImport, "dli.corpus.import"),
    (Op::CorpusExport, "dli.corpus.export"),
    (Op::AuthorAddDialect, "author.add_dialect"),
    (Op::AuthorAddNamePart, "author.add_name_part"),
    (Op::AuthorAddName, "author.add_name"),
    (Op::AuthorAddTerm, "author.add_term"),
    (Op::AuthorAddBanned, "author.add_banned"),
    (Op::AuthorAddState, "author.add_state"),
    (Op::AuthorAddAttest, "author.add_attest"),
    (Op::AuthorSetStanding, "author.set_standing"),
    (Op::AuthorDefeat, "author.defeat"),
    (Op::AuthorDeclareFunctional, "author.declare_functional"),
    (Op::AuthorAssertSameAs, "author.assert_same_as"),
    (Op::AuthorAddRule, "author.add_rule"),
    (Op::AuthorDeclareInertial, "author.declare_inertial"),
    (Op::AuthorAsOf, "author.as_of"),
    (Op::AuthorRecordDecision, "author.record_decision"),
    (Op::AuthorPredicates, "author.predicates"),
    (Op::AuthorEntityCandidates, "author.entity_candidates"),
    (
        Op::AuthorAssertPredicateSameAs,
        "author.assert_predicate_same_as",
    ),
    (Op::AuthorRecordDecisions, "author.record_decisions"),
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
#[allow(clippy::too_many_lines)]
fn check_finding_domains_round_trip() {
    use dli_core::Grade;
    use dli_serve::contract::{
        AttestationCheckFinding, AttestationPassCode, AttestationPassFinding,
        AttestedProcedureGrade, CheckFinding, CheckResult,
    };
    use dli_serve::{Finding, Severity};
    use dli_world::attest::{ContentHash, Question};

    let result = CheckResult {
        ok: false,
        grade: Grade::Checked,
        findings: vec![
            CheckFinding::World(Finding {
                severity: Severity::Warn,
                code: "world.advisory".to_owned(),
                message: "advisory".to_owned(),
                grade: Grade::Checked,
                repair: None,
                citations: Vec::new(),
                data: None,
            }),
            CheckFinding::Attestation(AttestationCheckFinding::Pass(Box::new(
                AttestationPassFinding {
                    code: AttestationPassCode::Value,
                    grade: AttestedProcedureGrade::Attested,
                    message: "yes".to_owned(),
                    question: Question {
                        kind: "basis".to_owned(),
                        atom: "basis(example)".to_owned(),
                        subject: "example".to_owned(),
                    },
                    record_id: format!("sha256:{}", "a".repeat(64)),
                    supersedes: None,
                    previous_answer: None,
                    stored_fingerprint: ContentHash("fnv1a64:1111111111111111".to_owned()),
                    current_fingerprint: ContentHash("fnv1a64:1111111111111111".to_owned()),
                },
            ))),
        ],
    };
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["findings"][0]["domain"], "world");
    assert_eq!(value["findings"][1]["domain"], "attestation");
    assert_eq!(
        value["findings"][1]["record_id"],
        format!("sha256:{}", "a".repeat(64))
    );
    assert_eq!(
        value["findings"][1]["current_fingerprint"],
        "fnv1a64:1111111111111111"
    );
    let schema = contract_schemas()
        .into_iter()
        .find(|(name, _)| name == "dli.check.response")
        .expect("check response schema")
        .1;
    let validator = jsonschema::validator_for(schema.as_value()).unwrap();
    assert!(validator.is_valid(&value));

    let mut contradictory = value.clone();
    contradictory["findings"][1]["code"] = "attest.fail".into();
    contradictory["findings"][1]["grade"] = "checked".into();
    contradictory["findings"][1]["severity"] = "error".into();
    assert!(
        !validator.is_valid(&contradictory),
        "a pass cannot claim fail/error/checked fields"
    );
    let mut extra = value.clone();
    extra["findings"][1]["severity"] = "error".into();
    assert!(
        !validator.is_valid(&extra),
        "the schema must reject a field serde rejects on the pass branch"
    );
    assert!(
        serde_json::from_value::<CheckResult>(extra).is_err(),
        "the schema and serde must agree on pass-branch closure"
    );
    assert_eq!(
        serde_json::from_value::<CheckResult>(value).unwrap(),
        result
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
fn export_golden_covers_every_closed_emission_branch() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/dli-worker/fixtures/golden/export-lore-v1.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let profiles = fixture["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["send"]["params"]["profile"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        profiles,
        std::collections::BTreeSet::from(["articy", "context-pack", "csv", "static-wiki",])
    );
}

#[test]
fn contract_version_is_consistent_across_registry_and_handshake() {
    use dli_serve::author::store::Store;
    use dli_serve::serve::respond;
    use dli_serve::session::Sessions;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry =
        std::fs::read_to_string(root.join("contracts/dli-worker/registry.yaml")).unwrap();
    let registry: serde_json::Value = serde_saphyr::from_str(&registry).unwrap();
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
