#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Contract smoke coverage: wire names and schema emission.

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
    (Op::CheckLedger, "dli.check-ledger"),
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
#[ignore = "enabled at SERVE-1"]
fn golden_dispatch_matches() {
    // Once dispatch is real, drive each golden fixture request through it and
    // assert the response equals the fixture's expectation.
    todo!("SERVE-1: golden request -> response dispatch")
}
