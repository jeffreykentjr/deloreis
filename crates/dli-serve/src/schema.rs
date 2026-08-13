//! Per-op contract schema emission.

use schemars::{Schema, schema_for};
use serde_json::Value;

// schema.rs references nearly every contract type; a scoped glob is clearer and
// less error-prone than a 60-line explicit import list.
#[allow(clippy::wildcard_imports)]
use crate::contract::*;

/// The per-op request/response JSON schemas.
///
/// Each entry is keyed `"<wire-op>.request"` or `"<wire-op>.response"`; the key
/// is the emitted schema file's stem. This is the authority the fixture
/// registry validates against. Ops whose response reuses a shared shape
/// ([`DiffDelta`], [`AuthorMutationResult`]) emit that
/// shape under each op's file name.
// One flat entry per op per direction; a table, not logic.
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn contract_schemas() -> Vec<(String, Schema)> {
    let mut schemas = vec![
        ("dli.version.request".to_owned(), schema_for!(VersionParams)),
        (
            "dli.version.response".to_owned(),
            schema_for!(VersionResult),
        ),
        ("dli.check.request".to_owned(), schema_for!(CheckParams)),
        ("dli.check.response".to_owned(), schema_for!(CheckResult)),
        ("dli.query.request".to_owned(), schema_for!(QueryParams)),
        ("dli.query.response".to_owned(), schema_for!(QueryResult)),
        (
            "dli.fork.new.request".to_owned(),
            schema_for!(ForkNewParams),
        ),
        (
            "dli.fork.new.response".to_owned(),
            schema_for!(ForkNewResult),
        ),
        (
            "dli.fork.list.request".to_owned(),
            schema_for!(ForkListParams),
        ),
        (
            "dli.fork.list.response".to_owned(),
            schema_for!(ForkListResult),
        ),
        (
            "dli.fork.diff.request".to_owned(),
            schema_for!(ForkDiffParams),
        ),
        ("dli.fork.diff.response".to_owned(), schema_for!(DiffDelta)),
        (
            "dli.fork.suppose.request".to_owned(),
            schema_for!(ForkSupposeParams),
        ),
        (
            "dli.fork.suppose.response".to_owned(),
            schema_for!(DiffDelta),
        ),
        (
            "dli.fork.add.request".to_owned(),
            schema_for!(ForkAddParams),
        ),
        (
            "dli.fork.add.response".to_owned(),
            schema_for!(ForkAddResult),
        ),
        (
            "dli.fork.suppress.request".to_owned(),
            schema_for!(ForkSuppressParams),
        ),
        (
            "dli.fork.suppress.response".to_owned(),
            schema_for!(DiffDelta),
        ),
        (
            "dli.fork.restrict.request".to_owned(),
            schema_for!(ForkRestrictParams),
        ),
        (
            "dli.fork.restrict.response".to_owned(),
            schema_for!(DiffDelta),
        ),
        (
            "dli.fork.preflight.request".to_owned(),
            schema_for!(ForkPreflightParams),
        ),
        (
            "dli.fork.preflight.response".to_owned(),
            schema_for!(ForkPreflightResult),
        ),
        (
            "dli.fork.promote.request".to_owned(),
            schema_for!(ForkPromoteParams),
        ),
        (
            "dli.fork.promote.response".to_owned(),
            schema_for!(ForkPromoteResult),
        ),
        (
            "dli.check-proof.request".to_owned(),
            schema_for!(CheckProofParams),
        ),
        (
            "dli.check-proof.response".to_owned(),
            schema_for!(CheckProofResult),
        ),
        (
            "dli.check-draft.request".to_owned(),
            schema_for!(CheckDraftParams),
        ),
        (
            "dli.check-draft.response".to_owned(),
            schema_for!(CheckDraftResult),
        ),
        (
            "dli.simulate.start.request".to_owned(),
            schema_for!(SimulateStartParams),
        ),
        (
            "dli.simulate.start.response".to_owned(),
            schema_for!(SimulateStartResult),
        ),
        (
            "dli.simulate.status.request".to_owned(),
            schema_for!(SimulateStatusParams),
        ),
        (
            "dli.simulate.status.response".to_owned(),
            schema_for!(SimulateStatusResult),
        ),
        (
            "dli.simulate.cancel.request".to_owned(),
            schema_for!(SimulateCancelParams),
        ),
        (
            "dli.simulate.cancel.response".to_owned(),
            schema_for!(SimulateCancelResult),
        ),
        (
            "dli.timeline.show.request".to_owned(),
            schema_for!(TimelineShowParams),
        ),
        (
            "dli.timeline.show.response".to_owned(),
            schema_for!(TimelineShowResult),
        ),
        (
            "dli.timeline.order.request".to_owned(),
            schema_for!(TimelineOrderParams),
        ),
        (
            "dli.timeline.order.response".to_owned(),
            schema_for!(TimelineOrderResult),
        ),
        (
            "dli.timeline.knows.request".to_owned(),
            schema_for!(TimelineKnowsParams),
        ),
        (
            "dli.timeline.knows.response".to_owned(),
            schema_for!(TimelineKnowsResult),
        ),
        (
            "dli.timeline.episodes.request".to_owned(),
            schema_for!(TimelineEpisodesParams),
        ),
        (
            "dli.timeline.episodes.response".to_owned(),
            schema_for!(TimelineEpisodesResult),
        ),
        (
            "dli.timeline.phases.request".to_owned(),
            schema_for!(TimelinePhasesParams),
        ),
        (
            "dli.timeline.phases.response".to_owned(),
            schema_for!(TimelinePhasesResult),
        ),
        (
            "dli.export.run.request".to_owned(),
            schema_for!(ExportRunParams),
        ),
        (
            "dli.export.run.response".to_owned(),
            schema_for!(ExportRunResult),
        ),
        (
            "dli.name.check.request".to_owned(),
            schema_for!(NameCheckParams),
        ),
        (
            "dli.name.check.response".to_owned(),
            schema_for!(NameCheckResult),
        ),
        (
            "dli.attest.missing.request".to_owned(),
            schema_for!(AttestMissingParams),
        ),
        (
            "dli.attest.missing.response".to_owned(),
            schema_for!(AttestMissingResult),
        ),
        (
            "dli.attestation-sweep.request".to_owned(),
            schema_for!(AttestationSweepParams),
        ),
        (
            "dli.attestation-sweep.response".to_owned(),
            schema_for!(AttestationSweepResult),
        ),
        (
            "dli.payload-submit.request".to_owned(),
            schema_for!(PayloadSubmitParams),
        ),
        (
            "dli.payload-submit.response".to_owned(),
            schema_for!(PayloadSubmitResult),
        ),
        (
            "dli.corpus.import.request".to_owned(),
            schema_for!(CorpusImportParams),
        ),
        (
            "dli.corpus.import.response".to_owned(),
            schema_for!(CorpusImportResult),
        ),
        (
            "dli.corpus.export.request".to_owned(),
            schema_for!(CorpusExportParams),
        ),
        (
            "dli.corpus.export.response".to_owned(),
            schema_for!(CorpusExportResult),
        ),
        (
            "author.open.request".to_owned(),
            schema_for!(AuthorOpenParams),
        ),
        (
            "author.open.response".to_owned(),
            schema_for!(AuthorOpenResult),
        ),
        (
            "author.status.request".to_owned(),
            schema_for!(AuthorStatusParams),
        ),
        (
            "author.status.response".to_owned(),
            schema_for!(AuthorStatusResult),
        ),
        (
            "author.diff.request".to_owned(),
            schema_for!(AuthorDiffParams),
        ),
        (
            "author.diff.response".to_owned(),
            schema_for!(AuthorDiffResult),
        ),
        (
            "author.discard.request".to_owned(),
            schema_for!(AuthorDiscardParams),
        ),
        (
            "author.discard.response".to_owned(),
            schema_for!(AuthorDiscardResult),
        ),
        (
            "author.materialize.request".to_owned(),
            schema_for!(AuthorMaterializeParams),
        ),
        (
            "author.materialize.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_entity.request".to_owned(),
            schema_for!(AuthorAddEntityParams),
        ),
        (
            "author.add_entity.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_fact.request".to_owned(),
            schema_for!(AuthorAddFactParams),
        ),
        (
            "author.add_fact.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_relation.request".to_owned(),
            schema_for!(AuthorAddRelationParams),
        ),
        (
            "author.add_relation.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_event.request".to_owned(),
            schema_for!(AuthorAddEventParams),
        ),
        (
            "author.add_event.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_before.request".to_owned(),
            schema_for!(AuthorAddBeforeParams),
        ),
        (
            "author.add_before.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.open_question.request".to_owned(),
            schema_for!(AuthorOpenQuestionParams),
        ),
        (
            "author.open_question.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_option.request".to_owned(),
            schema_for!(AuthorAddOptionParams),
        ),
        (
            "author.add_option.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_hypothesis.request".to_owned(),
            schema_for!(AuthorAddHypothesisParams),
        ),
        (
            "author.add_hypothesis.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.suppose.request".to_owned(),
            schema_for!(AuthorSupposeParams),
        ),
        (
            "author.suppose.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.query.request".to_owned(),
            schema_for!(AuthorQueryParams),
        ),
        (
            "author.query.response".to_owned(),
            schema_for!(AuthorQueryResult),
        ),
        (
            "author.as_of.request".to_owned(),
            schema_for!(AuthorAsOfParams),
        ),
        (
            "author.as_of.response".to_owned(),
            schema_for!(AuthorAsOfResult),
        ),
        (
            "author.record_decision.request".to_owned(),
            schema_for!(AuthorRecordDecisionParams),
        ),
        (
            "author.record_decision.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.check.request".to_owned(),
            schema_for!(AuthorCheckParams),
        ),
        (
            "author.check.response".to_owned(),
            schema_for!(AuthorCheckResult),
        ),
        (
            "author.why.request".to_owned(),
            schema_for!(AuthorWhyParams),
        ),
        (
            "author.why.response".to_owned(),
            schema_for!(AuthorWhyResult),
        ),
        (
            "author.findings.request".to_owned(),
            schema_for!(AuthorFindingsParams),
        ),
        (
            "author.findings.response".to_owned(),
            schema_for!(AuthorFindingsResult),
        ),
        (
            "author.list_workspaces.request".to_owned(),
            schema_for!(AuthorListWorkspacesParams),
        ),
        (
            "author.list_workspaces.response".to_owned(),
            schema_for!(AuthorListWorkspacesResult),
        ),
        (
            "author.branch.request".to_owned(),
            schema_for!(AuthorBranchParams),
        ),
        (
            "author.branch.response".to_owned(),
            schema_for!(AuthorBranchResult),
        ),
        (
            "author.merge.request".to_owned(),
            schema_for!(AuthorMergeParams),
        ),
        (
            "author.merge.response".to_owned(),
            schema_for!(AuthorMergeResult),
        ),
        // The DM-4 expert authoring surface (contract 1.3.0). Every one of these
        // stages through the shared path, so each response is the shared
        // `AuthorMutationResult` under its own op's file name.
        (
            "author.add_dialect.request".to_owned(),
            schema_for!(AuthorAddDialectParams),
        ),
        (
            "author.add_dialect.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_name_part.request".to_owned(),
            schema_for!(AuthorAddNamePartParams),
        ),
        (
            "author.add_name_part.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_name.request".to_owned(),
            schema_for!(AuthorAddNameParams),
        ),
        (
            "author.add_name.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_term.request".to_owned(),
            schema_for!(AuthorAddTermParams),
        ),
        (
            "author.add_term.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_banned.request".to_owned(),
            schema_for!(AuthorAddBannedParams),
        ),
        (
            "author.add_banned.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_state.request".to_owned(),
            schema_for!(AuthorAddStateParams),
        ),
        (
            "author.add_state.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_attest.request".to_owned(),
            schema_for!(AuthorAddAttestParams),
        ),
        (
            "author.add_attest.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.set_standing.request".to_owned(),
            schema_for!(AuthorSetStandingParams),
        ),
        (
            "author.set_standing.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.defeat.request".to_owned(),
            schema_for!(AuthorDefeatParams),
        ),
        (
            "author.defeat.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.declare_functional.request".to_owned(),
            schema_for!(AuthorDeclareFunctionalParams),
        ),
        (
            "author.declare_functional.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.assert_same_as.request".to_owned(),
            schema_for!(AuthorAssertSameAsParams),
        ),
        (
            "author.assert_same_as.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.add_rule.request".to_owned(),
            schema_for!(AuthorAddRuleParams),
        ),
        (
            "author.add_rule.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.declare_inertial.request".to_owned(),
            schema_for!(AuthorDeclareInertialParams),
        ),
        (
            "author.declare_inertial.response".to_owned(),
            schema_for!(AuthorMutationResult),
        ),
        (
            "author.predicates.request".to_owned(),
            schema_for!(AuthorPredicatesParams),
        ),
        (
            "author.predicates.response".to_owned(),
            schema_for!(AuthorPredicatesResult),
        ),
        (
            "author.entity_candidates.request".to_owned(),
            schema_for!(AuthorEntityCandidatesParams),
        ),
        (
            "author.entity_candidates.response".to_owned(),
            schema_for!(AuthorEntityCandidatesResult),
        ),
    ];
    for (name, schema) in &mut schemas {
        if name.ends_with(".request") {
            close_declared_objects(schema.as_object_mut());
        } else if name == "dli.check.response" {
            close_attestation_outcomes(schema.as_object_mut());
        }
    }
    schemas
}

/// Match the runtime's recursive unknown-field rejection in every published
/// request schema. Map-shaped objects already carry `additionalProperties`
/// and remain open by design; struct-shaped objects have `properties` and are
/// closed here even when their owning type lives in another crate.
fn close_declared_objects(root: Option<&mut serde_json::Map<String, Value>>) {
    let Some(root) = root else {
        return;
    };
    close_map(root);
}

fn close_value(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                close_value(item);
            }
        }
        Value::Object(map) => {
            close_map(map);
        }
        _ => {}
    }
}

fn close_map(map: &mut serde_json::Map<String, Value>) {
    for child in map.values_mut() {
        close_value(child);
    }
    if map.contains_key("properties") && !map.contains_key("additionalProperties") {
        map.insert("additionalProperties".to_owned(), Value::Bool(false));
    }
}

/// Close the composed attestation finding without breaking its flattened tagged
/// union. `additionalProperties: false` is not safe here; draft 2020-12
/// `unevaluatedProperties` at the outer object accounts for both the `domain`
/// discriminator and every field selected by the outcome branch.
fn close_attestation_outcomes(root: Option<&mut serde_json::Map<String, Value>>) {
    fn visit(value: &mut Value) {
        match value {
            Value::Array(items) => items.iter_mut().for_each(visit),
            Value::Object(map) => {
                let attestation_finding = map
                    .get("properties")
                    .and_then(Value::as_object)
                    .and_then(|properties| properties.get("domain"))
                    .and_then(Value::as_object)
                    .and_then(|domain| domain.get("const"))
                    .and_then(Value::as_str)
                    == Some("attestation")
                    && map.contains_key("oneOf");
                for child in map.values_mut() {
                    visit(child);
                }
                if attestation_finding {
                    map.insert("unevaluatedProperties".to_owned(), Value::Bool(false));
                }
            }
            _ => {}
        }
    }
    if let Some(root) = root {
        for child in root.values_mut() {
            visit(child);
        }
    }
}
