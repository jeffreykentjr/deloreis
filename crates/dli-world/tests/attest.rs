#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The attestation store and the attested-grade discipline (ATT-1).
//!
//! These tests prove the four ORACLE properties: the store is append-only per
//! question (superseding retains the chain), staleness is mechanical, a check
//! lacking its attestation reports the finding rather than guessing, and no
//! amount of model consensus lifts an attestation above `attested`.

use dli_core::atom::Atom;
use dli_core::posbool::PosBool;
use dli_core::{Grade, Verdict};

use dli_world::attest::{
    Attestation, AttestationStore, ContentHash, ModelRef, Policy, PromptRef, Question, QuestionKey,
    Sampling, fingerprint_facts, ratification_exit, require_attestation,
};
use dli_world::loader::{Loader, example_docs};
use dli_world::sweep;

/// A well-formed attestation record over one bounded question. The provenance
/// fields carry representative committed values; the `atom`, `subject`, and
/// `fingerprint` are what each test varies.
fn attestation(atom: &str, subject: &str, fingerprint: ContentHash) -> Attestation {
    Attestation {
        question: Question {
            kind: "register-real-basis".to_owned(),
            atom: atom.to_owned(),
            subject: subject.to_owned(),
        },
        answer: "yes".to_owned(),
        rationale_hash: format!("sha256:{atom}"),
        model: ModelRef {
            provider: "bedrock".to_owned(),
            id: "claude-fable-5".to_owned(),
            as_of: "2026-07-19".to_owned(),
        },
        prompt: PromptRef {
            template: "naming/register-basis@v3".to_owned(),
            hash: "sha256:88d1".to_owned(),
        },
        params: Sampling {
            temperature: "0.0".to_owned(),
            max_tokens: 400,
            samples: 3,
            agreement: "3/3".to_owned(),
        },
        policy: Policy {
            reattest: "on-model-change".to_owned(),
        },
        supersedes: None,
        sweep: "lab/campaign-082/runs/attest-0007.manifest.json".to_owned(),
        fingerprint,
    }
}

#[test]
fn superseding_appends_and_retains_the_chain() {
    let mut store = AttestationStore::new();
    let key = QuestionKey("register-real-basis::name-basis(enfys, welsh)".to_owned());

    let first = attestation(
        "name-basis(enfys, welsh)",
        "enfys",
        ContentHash("a".to_owned()),
    );
    let mut second = attestation(
        "name-basis(enfys, welsh)",
        "enfys",
        ContentHash("b".to_owned()),
    );
    second.answer = "no".to_owned();
    second.rationale_hash = "sha256:re-attested".to_owned();
    second.supersedes = Some(first.rationale_hash.clone());

    store.append(first.clone());
    store.append(second.clone());

    // A superseding answer becomes the head, but nothing is overwritten.
    assert_eq!(store.head(&key), Some(&second));
    let chain = store.chain(&key);
    assert_eq!(chain.len(), 2, "the chain is retained and readable");
    assert_eq!(chain[0], &first, "oldest first");
    assert_eq!(chain[1], &second, "head last");
    assert_eq!(store.records().len(), 2);

    // The supersedes link points at the record it replaced.
    assert_eq!(
        store.head(&key).unwrap().supersedes.as_ref(),
        Some(&first.rationale_hash)
    );
    // One distinct question, one head.
    assert_eq!(store.question_keys(), vec![key]);
}

#[test]
fn is_stale_flips_when_the_fingerprint_changes() {
    let taken = ContentHash("fnv1a64:aaaaaaaaaaaaaaaa".to_owned());
    let record = attestation("name-basis(enfys, welsh)", "enfys", taken.clone());

    assert!(!record.is_stale(&taken), "unchanged content is fresh");
    assert!(
        record.is_stale(&ContentHash("fnv1a64:bbbbbbbbbbbbbbbb".to_owned())),
        "a changed fingerprint is stale"
    );
}

#[test]
fn fingerprint_is_order_independent_and_content_sensitive() {
    let out = Loader::load(&example_docs());
    let model = &out.world;

    let seat: Vec<_> = model.facts_about("camelot");
    let forward = fingerprint_facts(seat.iter().map(|loaded| &loaded.atom));
    let reversed = fingerprint_facts(seat.iter().rev().map(|loaded| &loaded.atom));
    assert_eq!(
        forward, reversed,
        "iteration order does not change the digest"
    );

    let lake = fingerprint_facts(
        model
            .facts_about("the-lake")
            .iter()
            .map(|loaded| &loaded.atom),
    );
    assert_ne!(forward, lake, "different content fingerprints differently");
}

#[test]
fn sweep_returns_exactly_the_stale_heads() {
    let out = Loader::load(&example_docs());
    let model = &out.world;

    // Fresh: the head's fingerprint equals the world's current facts about the
    // subject, recomputed exactly as sweep will.
    let current_excalibur = model.subject_fingerprint("excalibur");
    let fresh = attestation("name-basis(excalibur, x)", "excalibur", current_excalibur);
    store_with_fresh_and_stale(model, fresh);
}

fn store_with_fresh_and_stale(model: &dli_world::WorldModel, fresh: Attestation) {
    let mut store = AttestationStore::new();
    store.append(fresh);

    // Stale: fingerprints that no longer match the current facts.
    let stale_seat = attestation(
        "name-basis(camelot, x)",
        "camelot",
        ContentHash("stale".to_owned()),
    );
    let stale_lake = attestation(
        "name-basis(the-lake, x)",
        "the-lake",
        ContentHash("stale".to_owned()),
    );
    store.append(stale_seat.clone());
    store.append(stale_lake.clone());

    let queue = sweep(&store, model);

    assert_eq!(queue.len(), 2, "exactly the stale heads, not the fresh one");
    assert!(queue.contains(&stale_seat));
    assert!(queue.contains(&stale_lake));
    assert!(
        !queue.iter().any(|record| record.subject() == "excalibur"),
        "a fresh head is never queued"
    );
}

#[test]
fn require_attestation_reports_missing_never_guesses() {
    let mut store = AttestationStore::new();
    let key = QuestionKey("register-real-basis::name-basis(enfys, welsh)".to_owned());

    let finding = require_attestation(&store, &key).expect_err("absent must be a finding");
    assert_eq!(finding.code(), "attest.missing");
    assert!(finding.message().contains("no attestation"));

    // Once a sweep has committed the record, the same lookup resolves.
    store.append(attestation(
        "name-basis(enfys, welsh)",
        "enfys",
        ContentHash("a".to_owned()),
    ));
    assert!(require_attestation(&store, &key).is_ok());
}

#[test]
fn no_consensus_lifts_the_grade() {
    // Consuming N agreeing attestations through either combinator never exceeds
    // Attested — the grade changes only when the procedure class changes.
    let agreeing = [Grade::Attested, Grade::Attested, Grade::Attested];
    assert_eq!(Grade::weakest(agreeing), Grade::Attested);
    assert_eq!(Grade::join_alt(agreeing), Grade::Attested);

    let hundred = std::iter::repeat_n(Grade::Attested, 100);
    assert_eq!(
        Grade::join_alt(hundred),
        Grade::Attested,
        "a hundred agreeing models is still attested"
    );
}

#[test]
fn checked_step_chained_with_an_attestation_rolls_up_attested() {
    // A check that chains a Checked procedure step with an Attested attestation
    // rolls up to Attested via weakest-link (Verdict::chain), while the support
    // channel accumulates both citations untouched.
    let checked = Verdict::new(PosBool::pos(Atom(0)), Grade::Checked);
    let attested = Verdict::new(PosBool::pos(Atom(1)), Grade::Attested);

    let rolled = checked.chain(&attested);
    assert_eq!(rolled.grade, Grade::Attested);
    assert_eq!(
        rolled.support,
        PosBool::pos(Atom(0)).and_(&PosBool::pos(Atom(1))),
        "support is the conjunction of both steps' citations"
    );
}

#[test]
fn ratification_exit_grades_checked_and_retains_the_chain() {
    let mut store = AttestationStore::new();
    let key = QuestionKey("register-real-basis::name-basis(enfys, welsh)".to_owned());

    let first = attestation(
        "name-basis(enfys, welsh)",
        "enfys",
        ContentHash("a".to_owned()),
    );
    let mut second = attestation(
        "name-basis(enfys, welsh)",
        "enfys",
        ContentHash("b".to_owned()),
    );
    second.rationale_hash = "sha256:re-attested".to_owned();
    store.append(first);
    store.append(second);

    let head = store.head(&key).expect("head present");
    // Exit without a ruling is impossible (empty and blank refused).
    assert!(ratification_exit(head, "").is_none());
    assert!(ratification_exit(head, "  ").is_none());
    let fact = ratification_exit(head, "D-0146").expect("a real ruling exits");

    // The ruling converts judgment to fact: the grade becomes checked (a lookup
    // is exact), fiat rides in the support/ruling, and the chain is retained.
    assert_eq!(fact.grade, Grade::Checked);
    assert_eq!(fact.ruling, "D-0146");
    assert_eq!(fact.answer, "yes");
    assert_eq!(fact.attested_history, key);

    assert_eq!(
        store.chain(&key).len(),
        2,
        "the attestation chain is retained as history"
    );
}
