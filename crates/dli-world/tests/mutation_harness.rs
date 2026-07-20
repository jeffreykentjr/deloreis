#![allow(clippy::unwrap_used, clippy::expect_used)]
//! PROOF-1 — the mutation harness: seeded-violation operators × the real gates.
//!
//! For every operator in `fixtures/mutation/{canon,draft,episode}.yaml` this suite
//! applies the operator's **deloreis-analog** mutation in memory and runs the REAL
//! wave-3 kernel gate over it — [`compile`] + [`check::run`], [`check_draft`],
//! [`check::continuity`], [`check::factcheck`], or the language battery
//! ([`language::check`] / [`language::vet_candidate`]) — then asserts the outcome.
//!
//! # Why the mutations are re-realized, not replayed
//!
//! The fantasy operators mutate **prose/YAML** and are caught by fantasy's Python
//! gates. The deloreis kernel is offline and consumes **structured lore** (packs of
//! typed declarations, drafts of typed claims), never prose. So each operator is
//! re-realized as the structured-lore mutation whose ground truth is a fact about
//! the *story's* world (spec 05 §3, the vet split) — exactly the mapping
//! `fixtures/mutation/README.md` proposes. This suite is where that proposed
//! mapping is *verified* against the real checks; the outcomes it records are the
//! honest wave-3 answer, and `docs/design-notes/mutation-coverage.md` is their
//! written-up matrix.
//!
//! # The five outcome classes (charter: honest grades, `xfail-strict`)
//!
//! - **Caught** — the expected kernel check fires with the right class. Most
//!   operators.
//! - **Factory** — the kernel correctly does *not* gate: the screen is a fact about
//!   *our* world (AI-tics, genre taste), factory-side by law (05 §3). Asserted as a
//!   *no-catch* so a future kernel that wrongly gated it would fail here.
//! - **Corpus-gap** — the kernel flags the claim as a defect (never silently
//!   passes it) but cannot reach the *precise* rejected-lore class because the
//!   guardrail is not in the vendored snapshot (`migration.md` §3 `corpus-gap`).
//!   Asserted as: flagged, but no citation of the rejecting ruling.
//! - **Pending-gate** — the catching check lands in a *later* PLAN node
//!   (craft-payload → CRAFT-2/PAY-2). Asserted `xfail-strict`: the wave-3 kernel
//!   does not yet catch it, so if a gate lands the assertion flips and the record
//!   is upgraded. (`cite-resolved-question` was such a record until the B1
//!   citation-existence gate landed and flipped it to *caught* — the flip working
//!   as designed.)
//! - **Miss (deterministic)** — the one honest hole: a paraphrase matching no
//!   surface cue. The offline kernel cannot reach it; asserted `xfail-strict`.
//!
//! Every operator id in the three fixtures is exercised by exactly one test here;
//! [`census_every_operator_is_exercised`] proves it (no operator silently unrun).

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, EntityDecl, EntityKind, FactDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_syntax::parse_document;

use dli_world::check::factcheck::{KilledToken, KilledTokens, MatchMode, Verdict, factcheck};
use dli_world::check::{self, CompiledWorld};
use dli_world::draft::{DraftMeta, DraftPayload};
use dli_world::language::{self, LanguageModel, vet_candidate};
use dli_world::model::LoadFinding;
use dli_world::pack::Pack;
use dli_world::{check_draft, compile};

// ===========================================================================
// Builders (shared with the CHECK-1 suite's conventions).
// ===========================================================================

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn marker(marker: ProvenanceMarker, cites: &[&str]) -> Annotation {
    Annotation {
        marker,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn ratified(cite: &str) -> Annotation {
    marker(ProvenanceMarker::Ratified, &[cite])
}

fn proposed() -> Annotation {
    marker(ProvenanceMarker::Proposed, &[])
}

fn atom(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: args.iter().map(|a| Term::Sym(id(a))).collect(),
    }
}

fn fact(pred: &str, args: &[&str], provenance: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, args),
        triple: false,
        provenance,
    })
}

fn entity(kind: EntityKind, name: &str, provenance: Option<Annotation>) -> Decl {
    Decl::Entity(EntityDecl {
        kind,
        name: id(name),
        label: None,
        time: None,
        slots: Vec::new(),
        provenance,
    })
}

fn pack(name: &str, decls: Vec<Decl>) -> Pack {
    Pack {
        name: name.to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id(name),
                span: Span::synthetic(),
            }),
            decls,
        }],
    }
}

/// A draft carrying claims and/or introduced names — the payload `check_draft`
/// reads.
fn draft(claims: Vec<AtomExpr>, names: &[&str]) -> DraftPayload {
    DraftPayload {
        meta: DraftMeta::default(),
        claims,
        names: names.iter().map(|n| (*n).to_owned()).collect(),
        ..DraftPayload::default()
    }
}

// ===========================================================================
// The vendored corpus bridge (facts.yaml → a deloreis pack), the killed-token
// registry, and the rejected-ledger constraints — the C-store's two sources.
// ===========================================================================

fn vendor_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/vendor/fantasy-40f6679")
}

#[derive(Debug, Deserialize)]
struct FactsDoc {
    #[serde(default)]
    entities: Vec<EntityRow>,
    #[serde(default)]
    relations: Vec<RelRow>,
    #[serde(default)]
    facts: Vec<FactRow>,
    #[serde(default)]
    constraints: Vec<Constraint>,
}

#[derive(Debug, Deserialize)]
struct EntityRow {
    id: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RelRow {
    subject: String,
    kind: String,
    object: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FactRow {
    entity: String,
    key: String,
    value: serde_yaml_ng::Value,
    #[serde(default)]
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

/// A machine-checkable guardrail compiled from a rejected decision (the
/// `constraints:` section of the vendored `facts.yaml`).
#[derive(Debug, Deserialize)]
struct Constraint {
    #[serde(default)]
    decisions: Vec<String>,
    #[serde(default)]
    description: String,
    machine_check: MachineCheck,
}

#[derive(Debug, Deserialize)]
struct MachineCheck {
    kind: String,
    pattern: String,
}

fn load_corpus() -> FactsDoc {
    let path = vendor_root().join("canon/data/facts.yaml");
    let text = std::fs::read_to_string(&path).expect("read facts.yaml");
    serde_yaml_ng::from_str(&text).expect("parse facts.yaml")
}

fn render_value(value: &serde_yaml_ng::Value) -> String {
    match value {
        serde_yaml_ng::Value::String(s) => s.clone(),
        serde_yaml_ng::Value::Bool(b) => b.to_string(),
        serde_yaml_ng::Value::Number(n) => n.to_string(),
        other => serde_yaml_ng::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_owned(),
    }
}

fn provenance(status: &str, decisions: &[String]) -> Option<Annotation> {
    let marker = match status {
        "ratified" => ProvenanceMarker::Ratified,
        "provisional" => ProvenanceMarker::Provisional,
        "rejected" => return None, // guardrails are not asserted lore
        _ => ProvenanceMarker::Proposed,
    };
    let cites: Vec<&str> = if marker == ProvenanceMarker::Proposed {
        Vec::new()
    } else {
        decisions.iter().map(String::as_str).collect()
    };
    Some(self::marker(marker, &cites))
}

fn map_kind(kind: &str) -> EntityKind {
    match kind {
        "place" => EntityKind::Place,
        "era" => EntityKind::Era,
        "event" => EntityKind::Event,
        "person" | "character" => EntityKind::Person,
        _ => EntityKind::Thing,
    }
}

/// Bridge the vendored `facts.yaml` into a deloreis pack: entities become `entity`
/// declarations, facts and relations become flat-triple `fact`s (the same bridge
/// the CHECK-1 shadow-parity suite uses).
fn corpus_pack(facts: &FactsDoc) -> Pack {
    let mut decls: Vec<Decl> = Vec::new();
    // B1: declare every decision the corpus cites (the ledger mirror of
    // decisions.md), so a citation carries real authority instead of minting it.
    let mut decisions = BTreeSet::new();
    for row in &facts.entities {
        decisions.extend(row.decisions.iter().cloned());
    }
    for row in &facts.facts {
        decisions.extend(row.decisions.iter().cloned());
    }
    for row in &facts.relations {
        decisions.extend(row.decisions.iter().cloned());
    }
    for decision in &decisions {
        decls.push(Decl::Ratify(RatifyDecl {
            decision: id(decision),
            body: Vec::new(),
        }));
    }
    let mut seen = BTreeSet::new();
    for row in &facts.entities {
        if seen.insert(row.id.clone()) {
            decls.push(entity(
                map_kind(&row.kind),
                &row.id,
                provenance(&row.status, &row.decisions),
            ));
        }
    }
    for row in &facts.facts {
        decls.push(fact(
            &row.key,
            &[&row.entity, &render_value(&row.value)],
            provenance(&row.status, &row.decisions),
        ));
    }
    for row in &facts.relations {
        decls.push(fact(
            &row.kind,
            &[&row.subject, &row.object],
            provenance(&row.status, &row.decisions),
        ));
    }
    Pack {
        name: "canon".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("canon"),
                span: Span::synthetic(),
            }),
            decls,
        }],
    }
}

fn corpus_world() -> CompiledWorld {
    compile(&corpus_pack(&load_corpus())).expect("vendored corpus compiles")
}

/// The killed-token registry, read from the vendored `killed_tokens.yaml` — the
/// D-killed names/titles the ledger struck down (the C-store's first source).
fn vendored_killed_tokens() -> Vec<KilledToken> {
    #[derive(Deserialize)]
    struct KilledDoc {
        killed: Vec<KilledRow>,
    }
    #[derive(Deserialize)]
    struct KilledRow {
        token: String,
        #[serde(rename = "match", default)]
        mode: String,
        decision: String,
        #[serde(default)]
        scope: String,
    }
    let path = vendor_root().join("src/fantasy/propagate/data/killed_tokens.yaml");
    let doc: KilledDoc =
        serde_yaml_ng::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    doc.killed
        .into_iter()
        .map(|row| KilledToken {
            token: row.token,
            mode: match row.mode.as_str() {
                "phrase" => MatchMode::Phrase,
                "substring" | "regex" => MatchMode::Substring,
                _ => MatchMode::Word,
            },
            decision: row.decision,
            scope: row.scope,
        })
        .collect()
}

/// The rejected-ledger constraints, read from the vendored `facts.yaml`
/// `constraints:` section — "machine-checkable guardrails compiled from rejected
/// decisions" (the C-store's second source). Only the `name_ban`/`vocabulary`
/// checks yield a token; a `structure` check (monster-of-the-week) is not a
/// lexical guardrail.
fn vendored_rejected_constraints() -> Vec<KilledToken> {
    load_corpus()
        .constraints
        .into_iter()
        .filter_map(|c| {
            let token = token_from_pattern(&c.machine_check.pattern);
            let mode = match c.machine_check.kind.as_str() {
                "name_ban" => MatchMode::Word,
                "vocabulary" => {
                    if token.contains(' ') {
                        MatchMode::Phrase
                    } else {
                        MatchMode::Word
                    }
                }
                _ => return None, // structure/other: not a lexical guardrail
            };
            Some(KilledToken {
                token,
                mode,
                decision: c.decisions.first().cloned().unwrap_or_default(),
                scope: c.description,
            })
        })
        .collect()
}

/// Strip a simple `\b…\b` word-boundary regex to its bare token.
fn token_from_pattern(pattern: &str) -> String {
    pattern.replace("\\b", "").trim().to_owned()
}

/// The full C-store the consuming repo compiles from its rejected ledger: the
/// D-killed names/titles PLUS the rejected-decision guardrail constraints. The
/// kernel reads no files, so this is supplied as data (spec: `with_killed_tokens`).
fn rejected_lore_store() -> KilledTokens {
    let mut tokens = vendored_killed_tokens();
    tokens.extend(vendored_rejected_constraints());
    KilledTokens::new(tokens)
}

// ===========================================================================
// Small assertion helpers.
// ===========================================================================

fn world_codes(findings: &[check::WorldFinding]) -> Vec<String> {
    findings.iter().map(|f| f.code.clone()).collect()
}

fn draft_messages(findings: &[dli_world::draft::DraftFinding]) -> Vec<String> {
    findings.iter().map(|f| f.message.clone()).collect()
}

// ===========================================================================
// CANON operators (fixtures/mutation/canon.yaml) — 9.
// ===========================================================================

/// `break-status-marker` → provenance + assertability (CAUGHT). Strip the citation
/// off a ratified corpus fact: the ledger marker can no longer resolve, so the
/// loader fails it closed and the claim it bore is no longer certain.
#[test]
fn op_break_status_marker() {
    let mut pack = corpus_pack(&load_corpus());
    for decl in &mut pack.documents[0].decls {
        if let Decl::Fact(f) = decl
            && f.atom.pred.text == "gender"
            && matches!(f.atom.args.first(), Some(Term::Sym(s)) if s.text == "anchor")
        {
            f.provenance = Some(marker(ProvenanceMarker::Ratified, &[])); // ratified, no citation
        }
    }
    let world = compile(&pack).expect("compiles");
    // provenance: the loader fails it closed.
    assert!(
        world
            .model()
            .findings()
            .iter()
            .any(|f| matches!(f, LoadFinding::MalformedProvenance { .. })),
        "break-status-marker: expected malformed-provenance, got {:?}",
        world.model().findings()
    );
    assert!(check::run(&world).iter().any(|f| f.family() == "load"));
    // assertability: the fact is no longer certain, so the claim is not supported.
    let reports = factcheck(&world, &[atom("gender", &["anchor", "male"])]);
    assert_ne!(reports[0].verdict, Verdict::Supported);
}

/// `flip-proposed-to-ratified` → provenance (CAUGHT). Promote a proposed claim to
/// ratified with no citation: a bare `ratified` with no ruling has nowhere to
/// stand and fails closed.
#[test]
fn op_flip_proposed_to_ratified() {
    let world = compile(&pack(
        "canon",
        vec![fact(
            "origin",
            &["apprentice", "halspetra"],
            Some(marker(ProvenanceMarker::Ratified, &[])), // promoted, no citation
        )],
    ))
    .expect("compiles");
    assert!(
        world
            .model()
            .findings()
            .iter()
            .any(|f| matches!(f, LoadFinding::MalformedProvenance { .. })),
        "flip-proposed-to-ratified: expected malformed-provenance, got {:?}",
        world.model().findings()
    );
}

/// `malformed-override-leak` → provenance + assertability (CAUGHT). A malformed
/// ratified override on a proposed-texture fact fails closed and never lets the
/// proposed texture into the certain layer.
#[test]
fn op_malformed_override_leak() {
    // The proposed texture, plus a malformed ratified override (no citation).
    let world = compile(&pack(
        "canon",
        vec![fact(
            "texture-secret",
            &["anchor", "hidden"],
            Some(marker(ProvenanceMarker::Ratified, &[])), // malformed override
        )],
    ))
    .expect("compiles");
    // provenance: the override is held fail-closed.
    assert!(
        world
            .model()
            .findings()
            .iter()
            .any(|f| matches!(f, LoadFinding::MalformedProvenance { .. })),
        "malformed-override-leak: expected malformed-provenance, got {:?}",
        world.model().findings()
    );
    // assertability: the texture never reads certain.
    let reports = factcheck(&world, &[atom("texture-secret", &["anchor", "hidden"])]);
    assert_ne!(
        reports[0].verdict,
        Verdict::Supported,
        "malformed-override-leak: proposed texture must not become certain"
    );
}

/// `dangling-link` → reference (CAUGHT). A [[wiki-link]] to a nonexistent id
/// re-homes as a *claim*-level reference: the loader observes every symbol a fact
/// names, so the honest dangling signal is a delta referencing a symbol canon
/// never even observed (see `check::continuity` module docs).
#[test]
fn op_dangling_link() {
    let world = corpus_world();
    let findings =
        check::continuity::check_claims(&world, &[atom("mentions", &["ghost-entity", "x"])]);
    assert!(
        world_codes(&findings).contains(&"continuity.unknown-entity".to_owned()),
        "dangling-link: expected unknown-entity, got {:?}",
        world_codes(&findings)
    );
}

/// `duplicate-id` → uniqueness (CAUGHT). A second declaration of an existing entity
/// id collides within the pack.
#[test]
fn op_duplicate_id() {
    let mut pack = corpus_pack(&load_corpus());
    pack.documents[0]
        .decls
        .push(entity(EntityKind::Place, "gwern", Some(ratified("D-0111"))));
    let world = compile(&pack).expect("compiles");
    assert!(
        world_codes(&check::run(&world)).contains(&"load.duplicate-declaration".to_owned()),
        "duplicate-id: expected duplicate-declaration, got {:?}",
        world_codes(&check::run(&world))
    );
}

/// `banned-word-into-diegetic` → lexicon-ban (CAUGHT). The prose scanner stays
/// factory-side; the kernel owns the *compiled ban list* (a cited lexicon fact,
/// 05 §4), which catches the banned token when it enters structured lore — here,
/// as a compiled name.
#[test]
fn op_banned_word_into_diegetic() {
    let src = "scope language\n\
        banned algorithm D-0004 \"technology register is barred in-world\"\n\
        name algo \"Algorithm\" concept in none ratified D-0004\n";
    let (doc, _diags) = parse_document(src);
    let model = LanguageModel::from_documents(&[doc]).model;
    let codes: Vec<&str> = language::check_language(&model)
        .iter()
        .map(language::LangFinding::code)
        .collect();
    assert!(
        codes.contains(&"name.banned-in-name"),
        "banned-word-into-diegetic: expected name.banned-in-name, got {codes:?}"
    );
}

/// `cite-resolved-question` → citation (CAUGHT by B1). A resolved-question id cited
/// as a ruling is not a *declared* decision (no `ratify Q-0002`), so the ledger
/// valuation leaves its variable open: the claim fails closed to non-certain and a
/// `citation.unknown-decision` finding is raised. This is the flip the pending-gate
/// record anticipated — before B1 the kernel ruled every cited id true and the
/// forgery slipped through; now the forged authority mints nothing, matching the
/// corpus `expected_catch: [citation]`.
#[test]
fn op_cite_resolved_question() {
    // A claim citing Q-0002 as if it were a ruling, with no `ratify Q-0002` to back
    // it. B1 treats the citation as forged authority.
    let world = compile(&pack(
        "canon",
        vec![fact(
            "origin",
            &["apprentice", "halspetra"],
            Some(marker(ProvenanceMarker::Ratified, &["Q-0002"])),
        )],
    ))
    .expect("compiles");
    // The forged citation surfaces as a finding.
    assert!(
        world_codes(&check::run(&world)).contains(&"citation.unknown-decision".to_owned()),
        "cite-resolved-question: expected citation.unknown-decision, got {:?}",
        world_codes(&check::run(&world))
    );
    // And the claim fails closed — a citation to an undeclared id mints no authority.
    let reports = factcheck(&world, &[atom("origin", &["apprentice", "halspetra"])]);
    assert_ne!(
        reports[0].verdict,
        Verdict::Supported,
        "cite-resolved-question must not read as supported: a resolved question is not a ruling",
    );
}

/// `registry-file-desync` → registry-consistency (KNOWN-DIVERGENCE: dissolved).
/// The hand-synced mirror dies (`migration.md` §2): the compiled world *is* the
/// name-space, so there is no second registry to fall out of sync with. The class
/// dissolves; what remains is reference-integrity, which the kernel does hold — a
/// claim on a genuinely-unobserved id is caught, a declared-and-referenced entity
/// stays consistent.
#[test]
fn op_registry_file_desync() {
    let world = corpus_world();
    // The residual guarantee: an entity canon actually declares (gwern) is known,
    // so nothing "desyncs"; a symbol canon never saw is the only reference finding.
    let known: BTreeSet<&str> = world
        .model()
        .entities()
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(
        known.contains("gwern"),
        "registry-file-desync dissolves: the compiled world is the single registry"
    );
    let findings =
        check::continuity::check_claims(&world, &[atom("role", &["no-such-registry-row", "x"])]);
    assert!(
        world_codes(&findings).contains(&"continuity.unknown-entity".to_owned()),
        "the residual reference-integrity still fires on a truly-absent id: {:?}",
        world_codes(&findings)
    );
}

/// `forged-promotion` → provenance (CAUGHT). A ratified claim stripped of its
/// authorizing ruling fails closed. Note: the strict V11 nuance (a *real* D-id
/// whose section-affects set does not name this claim) has no deloreis analog — the
/// per-claim citation model has no sections and no affects-set; a claim either
/// cites a live ruling or it fails closed. This is the faithful deloreis
/// realization (mirrors the MIG-1 strip-decisions detection).
#[test]
fn op_forged_promotion() {
    let mut pack = corpus_pack(&load_corpus());
    // Strip the authorizing ruling from a ratified fact (a "promotion" with no live
    // decision behind it).
    for decl in &mut pack.documents[0].decls {
        if let Decl::Fact(f) = decl
            && f.atom.pred.text == "given-name"
        {
            f.provenance = Some(marker(ProvenanceMarker::Ratified, &[]));
        }
    }
    let world = compile(&pack).expect("compiles");
    assert!(
        world
            .model()
            .findings()
            .iter()
            .any(|f| matches!(f, LoadFinding::MalformedProvenance { .. })),
        "forged-promotion: a ratified claim with no live ruling must fail closed, got {:?}",
        world.model().findings()
    );
}

// ===========================================================================
// DRAFT operators (fixtures/mutation/draft.yaml) — 20.
// ===========================================================================

/// `contradict-anchor-gender` → contradiction (CAUGHT). A draft asserting the
/// anchor is a woman clashes with certain canon (male, D-0096).
#[test]
fn op_contradict_anchor_gender() {
    let world = corpus_world();
    let findings = check_draft(
        &world,
        &draft(vec![atom("gender", &["anchor", "female"])], &[]),
    );
    assert!(
        draft_messages(&findings)
            .iter()
            .any(|m| m.contains("canon asserts male")),
        "contradict-anchor-gender: expected a gender contradiction, got {:?}",
        draft_messages(&findings)
    );
}

/// `contradict-protagonist-immortal` → contradiction (CORPUS-GAP). Immortality is
/// rejected as the protagonist's depth source (D-0067), but that rejected-ledger
/// constraint is NOT in the vendored `constraints:` snapshot. The kernel therefore
/// flags the claim as an *unsupported* defect (never silently passes it) but cannot
/// reach the precise rejected-lore class with the D-0067 citation. Adding the
/// constraint to the vendored corpus upgrades this to a clean contradiction.
#[test]
fn op_contradict_protagonist_immortal() {
    let world = corpus_world().with_killed_tokens(rejected_lore_store());
    let findings = check_draft(
        &world,
        &draft(vec![atom("mortality", &["protagonist", "immortal"])], &[]),
    );
    assert!(
        !findings.is_empty(),
        "corpus-gap: the claim is still flagged as a defect, never silently passed"
    );
    assert!(
        findings.iter().all(|f| !f.message.contains("D-0067")),
        "corpus-gap confirmed: no vendored constraint carries the D-0067 rejection, \
         so the precise rejected-lore citation is absent. If a constraint now exists, \
         upgrade this record. Got {:?}",
        draft_messages(&findings)
    );
}

/// `contradict-hound-mundane` → contradiction + assertability (CAUGHT). The hound's
/// nature is an open (shadow) question; asserting it settled ("just a dog") states
/// merely-possible lore as fact.
#[test]
fn op_contradict_hound_mundane() {
    // A world where the hound's nature rests only on proposed (shadow) lore.
    let world = compile(&pack(
        "canon",
        vec![fact("nature", &["hound", "moor-spirit"], Some(proposed()))],
    ))
    .expect("compiles");
    let findings =
        check::continuity::check_claims(&world, &[atom("nature", &["hound", "mundane"])]);
    assert!(
        world_codes(&findings).contains(&"continuity.proposed-as-fact".to_owned()),
        "contradict-hound-mundane: expected proposed-as-fact, got {:?}",
        world_codes(&findings)
    );
}

/// `assert-auditor-identity` → contradiction (CAUGHT, rejected-lore). The vendored
/// constraint `no-auditor-concept` (D-0009) compiles into the C-store; a claim
/// naming the rejected `auditor` identity is contradicted with its citation.
#[test]
fn op_assert_auditor_identity() {
    let world = corpus_world().with_killed_tokens(rejected_lore_store());
    let reports = factcheck(&world, &[atom("identity", &["protagonist", "auditor"])]);
    assert_eq!(reports[0].verdict, Verdict::Contradicted);
    assert!(
        reports[0].citations.contains(&"D-0009".to_owned()),
        "assert-auditor-identity: expected the D-0009 rejection citation, got {:?}",
        reports[0].citations
    );
}

/// `assert-demon-king` → contradiction (CAUGHT, rejected-lore). The vendored
/// constraint `no-singular-demon-king` (D-0010) compiles into the C-store as a
/// phrase; a claim seating a demon king is contradicted.
#[test]
fn op_assert_demon_king() {
    let world = corpus_world().with_killed_tokens(rejected_lore_store());
    let reports = factcheck(&world, &[atom("ruler", &["empire", "demon-king"])]);
    assert_eq!(reports[0].verdict, Verdict::Contradicted);
    assert!(
        reports[0].citations.contains(&"D-0010".to_owned()),
        "assert-demon-king: expected the D-0010 rejection citation, got {:?}",
        reports[0].citations
    );
}

/// `assert-musical-magic` → contradiction (CORPUS-GAP). The musical/acoustic magic
/// mechanic is rejected (D-0002), but no vendored constraint carries it, so the
/// kernel flags the claim as unsupported without the precise rejected-lore class.
#[test]
fn op_assert_musical_magic() {
    let world = corpus_world().with_killed_tokens(rejected_lore_store());
    let findings = check_draft(
        &world,
        &draft(vec![atom("mechanic", &["magic", "musical"])], &[]),
    );
    assert!(
        !findings.is_empty(),
        "corpus-gap: the claim is still flagged, never silently passed"
    );
    assert!(
        findings.iter().all(|f| !f.message.contains("D-0002")),
        "corpus-gap: no vendored constraint carries the D-0002 rejection. Got {:?}",
        draft_messages(&findings)
    );
}

/// `assert-narrator-ticking-clock` → contradiction (CORPUS-GAP). A ticking clock in
/// the narrator's frame is rejected (D-0104); no vendored constraint carries it, so
/// the kernel flags but cannot cite the rejection.
#[test]
fn op_assert_narrator_ticking_clock() {
    let world = corpus_world().with_killed_tokens(rejected_lore_store());
    let findings = check_draft(
        &world,
        &draft(vec![atom("frame", &["narrator", "ticking-clock"])], &[]),
    );
    assert!(
        !findings.is_empty(),
        "corpus-gap: the claim is still flagged, never silently passed"
    );
    assert!(
        findings.iter().all(|f| !f.message.contains("D-0104")),
        "corpus-gap: no vendored constraint carries the D-0104 rejection. Got {:?}",
        draft_messages(&findings)
    );
}

/// `modern-vocab-algorithm` → lexicon-ban (CAUGHT). A draft-introduced term is
/// vetted against the compiled ban list (the kernel half of the naming oracle);
/// `algorithm` is a cited lexicon ban.
#[test]
fn op_modern_vocab_algorithm() {
    let model = ban_model("algorithm");
    let codes: Vec<&str> = vet_candidate(&model, "algorithm")
        .iter()
        .map(language::LangFinding::code)
        .collect();
    assert!(
        codes.contains(&"name.banned-in-name"),
        "modern-vocab-algorithm: expected name.banned-in-name, got {codes:?}"
    );
}

/// `modern-vocab-carbon` → lexicon-ban (CAUGHT). A modern-ecology term barred by the
/// compiled ban list.
#[test]
fn op_modern_vocab_carbon() {
    let model = ban_model("carbon");
    let codes: Vec<&str> = vet_candidate(&model, "carbon")
        .iter()
        .map(language::LangFinding::code)
        .collect();
    assert!(
        codes.contains(&"name.banned-in-name"),
        "modern-vocab-carbon: expected name.banned-in-name, got {codes:?}"
    );
}

/// A language model whose only lore is a single cited lexical ban.
fn ban_model(word: &str) -> LanguageModel {
    let src = format!("scope language\nbanned {word} D-0004 \"modern register barred\"\n");
    let (doc, _diags) = parse_document(&src);
    LanguageModel::from_documents(&[doc]).model
}

/// `killed-name-elara` → killed-token (FACTORY — reclassified). Elara is NOT in the
/// vendored killed registry (`killed_tokens.yaml`); it lives only in `ai_tics.yaml`,
/// which stays factory-side by law (`migration.md` §2: ai-tics are an external
/// screen, not re-homed to the kernel; spec 05 §3). So the kernel correctly does
/// NOT catch it — the AI-tic screen is a fact about *our* world. The corpus record
/// is corrected from `side: kernel` to `side: factory`.
#[test]
fn op_killed_name_elara() {
    let world = corpus_world().with_killed_tokens(rejected_lore_store());
    let findings = check_draft(&world, &draft(vec![], &["Elara"]));
    assert!(
        findings.iter().all(|f| !f.message.contains("killed token")),
        "killed-name-elara is factory-side: Elara is an ai-tic, not a D-killed token, \
         so the offline kernel must not gate it. If it now does, the C-store source \
         changed — revisit the vet split (05 §3). Got {:?}",
        draft_messages(&findings)
    );
}

/// `killed-name-silas` → killed-token (CAUGHT). Silas is a registered killed token
/// (Q-0015) in the vendored registry; the kernel refuses its return.
#[test]
fn op_killed_name_silas() {
    assert_killed_name_caught("Silas");
}

/// `killed-name-irondale` → killed-token (CAUGHT). Irondale (D-0101, franchise
/// collision) is in the vendored registry.
#[test]
fn op_killed_name_irondale() {
    assert_killed_name_caught("Irondale");
}

/// `killed-name-doom-wight` → killed-token (CAUGHT). Doom-wight (D-0097, portent
/// cliche) is in the vendored registry as a phrase.
#[test]
fn op_killed_name_doom_wight() {
    assert_killed_name_caught("Doom-wight");
}

/// A draft introducing `name` is flagged for reviving a killed token.
fn assert_killed_name_caught(name: &str) {
    let world = corpus_world().with_killed_tokens(rejected_lore_store());
    let findings = check_draft(&world, &draft(vec![], &[name]));
    assert!(
        findings.iter().any(|f| f.message.contains("killed token")),
        "killed name '{name}': expected a killed-token finding, got {:?}",
        draft_messages(&findings)
    );
}

/// `wrong-register-name` → register-fit (CAUGHT). An imperial-register form placed
/// in the crags register fails the register's phonotactic frame — the imperial
/// cluster `qu` marks the surface as not of the crag tongue (register-fit is a
/// compile verdict, 05 §2).
#[test]
fn op_wrong_register_name() {
    let src = "scope language\n\
        dialect crags \"the crag tongue\" ratified D-0111 {\n\
        vowels a e i o u\n\
        foreign qu x z\n\
        cluster-max 3\n\
        name-kind place = place-qualifier place-generic\n\
        name-part aqui \"spring\" role place-qualifier ratified D-0111\n\
        name-part lium \"hall\" role place-generic ratified D-0111\n\
        }\n\
        name aquilium \"Aquilium\" place in crags = place-qualifier aqui + place-generic lium ratified D-0111\n";
    let (doc, _diags) = parse_document(src);
    let model = LanguageModel::from_documents(&[doc]).model;
    let codes: Vec<&str> = language::check_language(&model)
        .iter()
        .map(language::LangFinding::code)
        .collect();
    assert!(
        codes.contains(&"name.phonotactics"),
        "wrong-register-name: expected register-fit (name.phonotactics) on the \
         imperial 'qu' cluster, got {codes:?}"
    );
}

/// `crowd-scene-cast` → craft-payload (PENDING-GATE). The cast-per-scene rule is a
/// season-battery craft check (spec 06) that lands at CRAFT-2/PAY-2 (wave 4); no
/// craft gate exists in the wave-3 kernel. Asserted `xfail-strict`.
#[test]
fn op_crowd_scene_cast() {
    assert_no_craft_gate_yet();
}

/// `as-you-know-bob-leak` → craft-payload (PENDING-GATE, CRAFT-2/PAY-2).
#[test]
fn op_as_you_know_bob_leak() {
    assert_no_craft_gate_yet();
}

/// `ornate-tag-cluster` → craft-payload (PENDING-GATE, CRAFT-2/PAY-2).
#[test]
fn op_ornate_tag_cluster() {
    assert_no_craft_gate_yet();
}

/// The craft-payload battery has no wave-3 home: `WorldModel::craft_rules` is an
/// empty table and no `craft.*` finding class exists. This proves the gate is
/// genuinely absent (so the three craft operators are honestly pending, not
/// silently passed); it flips the day CRAFT-2 lands a `craft.*` finding.
fn assert_no_craft_gate_yet() {
    let world = corpus_world();
    assert!(
        world.model().craft_rules.is_empty(),
        "pending-gate: no craft rules compile at wave 3 (CRAFT-2/PAY-2 own them)"
    );
    assert!(
        check::run(&world).iter().all(|f| f.family() != "craft"),
        "pending-gate: no craft.* finding class exists at wave 3"
    );
}

/// `slop-cluster` → style-slop (FACTORY). An AI-slop screen is a fact about OUR
/// world's machine-generation tells; it stays factory-side and must never gate
/// assertability (05 §3). The kernel emits no finding for slop prose.
#[test]
fn op_slop_cluster() {
    // The slop cluster carries no structured lore violation: a clean draft with no
    // contradicting claim and no killed/ banned token. The kernel is silent.
    let world = corpus_world().with_killed_tokens(rejected_lore_store());
    let findings = check_draft(
        &world,
        &draft(vec![atom("mood", &["apprentice", "afraid"])], &[]),
    );
    assert!(
        findings.iter().all(|f| !f.message.contains("killed token")),
        "slop-cluster is factory-side: the kernel must not gate machine-tell prose. Got {:?}",
        draft_messages(&findings)
    );
}

/// `allegory-wink` → allegory-explicitness (FACTORY). Winking at the allegory is a
/// genre-taste judgment about our expectations, not a story-world fact; it is a
/// factory-side screen, never a kernel assertability gate (05 §3 principle).
#[test]
fn op_allegory_wink() {
    let world = corpus_world();
    // "the system is rigged" is a coherence/taste judgment. Framed as a structured
    // claim about a *known* canon subject (the-plenty, the imperial framework), it
    // maps to no kernel-gated violation: not a contradiction, not shadow-as-settled.
    let findings =
        check::continuity::check_claims(&world, &[atom("theme", &["the-plenty", "rigged"])]);
    assert!(
        findings.is_empty(),
        "allegory-wink is factory-side: the kernel must not gate genre taste. Got {:?}",
        world_codes(&findings)
    );
    // And there is no allegory finding class at all — factory by law (05 §3).
    assert!(
        check::run(&world).iter().all(|f| f.family() != "allegory"),
        "no allegory.* kernel class exists: winking is a factory-side screen"
    );
}

/// `paraphrased-contradiction` → contradiction (MISS, deterministic). A paraphrase
/// that matches no fact cue slips past the deterministic cue-miner; the offline,
/// model-free kernel cannot reach it. The opt-in cross-family semantic judge (a
/// non-gating factory lane) is the only closer. Asserted `xfail-strict`: the day the
/// gating kernel catches a cue-less paraphrase, this fails and the record upgrades.
#[test]
fn op_paraphrased_contradiction() {
    // The hound's nature is open (shadow). A paraphrase ("a clever herding beast")
    // shares no symbol with any fact cue, so the deterministic checker finds nothing
    // to bind against — no contradiction, no proposed-as-fact.
    let world = compile(&pack(
        "canon",
        vec![fact("nature", &["hound", "moor-spirit"], Some(proposed()))],
    ))
    .expect("compiles");
    // The paraphrase shares no cue with the shadow `nature` slot: it speaks of the
    // hound (a known subject) in different words (`herds`, `sheep`), so the
    // deterministic checker finds nothing to bind against — no contradiction, no
    // proposed-as-fact. Semantically it denies the open nature; structurally it is
    // indistinguishable from legitimate new lore. That gap is the honest miss.
    let findings = check::continuity::check_claims(&world, &[atom("herds", &["hound", "sheep"])]);
    assert!(
        findings.is_empty(),
        "paraphrased-contradiction is the honest deterministic miss: a cue-less \
         paraphrase is unreachable offline. If the kernel now catches it, upgrade \
         the record. Got {:?}",
        world_codes(&findings)
    );
}

// ===========================================================================
// EPISODE operators (fixtures/mutation/episode.yaml) — 4.
// ===========================================================================

/// `continuity-world-contradiction` → contradiction (CAUGHT). An episode delta sets
/// a fact to a value the certain layer denies.
#[test]
fn op_continuity_world_contradiction() {
    let world = corpus_world();
    let findings = check_draft(
        &world,
        &draft(vec![atom("gender", &["protagonist", "male"])], &[]),
    );
    assert!(
        draft_messages(&findings)
            .iter()
            .any(|m| m.contains("canon asserts female")),
        "continuity-world-contradiction: expected a world contradiction, got {:?}",
        draft_messages(&findings)
    );
}

/// `continuity-prior-contradiction` → contradiction (CAUGHT). An episode changes a
/// fact from the value the prior episode established.
#[test]
fn op_continuity_prior_contradiction() {
    let prior = [(
        ("gender".to_owned(), "anchor".to_owned()),
        "male".to_owned(),
    )]
    .into();
    let findings =
        check::continuity::prior_contradictions(&[atom("gender", &["anchor", "female"])], &prior);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, "continuity.prior-contradiction");
}

/// `continuity-unknown-entity` → reference (CAUGHT). An episode references an id
/// canon does not define.
#[test]
fn op_continuity_unknown_entity() {
    let world = corpus_world();
    let findings = check::continuity::check_claims(&world, &[atom("gender", &["ghost", "female"])]);
    assert!(
        world_codes(&findings).contains(&"continuity.unknown-entity".to_owned()),
        "continuity-unknown-entity: expected unknown-entity, got {:?}",
        world_codes(&findings)
    );
}

/// `continuity-proposed-as-fact` → assertability (CAUGHT). An episode asserts
/// proposed/shadow lore as established fact.
#[test]
fn op_continuity_proposed_as_fact() {
    let world = compile(&pack(
        "canon",
        vec![fact("origin", &["anchor", "swartby"], Some(proposed()))],
    ))
    .expect("compiles");
    let findings =
        check::continuity::check_claims(&world, &[atom("origin", &["anchor", "swartby"])]);
    assert!(
        world_codes(&findings).contains(&"continuity.proposed-as-fact".to_owned()),
        "continuity-proposed-as-fact: expected proposed-as-fact, got {:?}",
        world_codes(&findings)
    );
}

// ===========================================================================
// The coverage census: every operator id in the three fixtures is exercised.
// ===========================================================================

/// Every operator this suite exercises, tagged with the wave-3 kernel outcome it
/// asserts. The census cross-checks the id set against the fixtures so no operator
/// is silently unrun and no stale id lingers.
const EXERCISED: &[(&str, &str)] = &[
    // canon
    ("break-status-marker", "caught"),
    ("flip-proposed-to-ratified", "caught"),
    ("malformed-override-leak", "caught"),
    ("dangling-link", "caught"),
    ("duplicate-id", "caught"),
    ("banned-word-into-diegetic", "caught"),
    ("cite-resolved-question", "caught"),
    ("registry-file-desync", "known-divergence"),
    ("forged-promotion", "caught"),
    // draft
    ("contradict-anchor-gender", "caught"),
    ("contradict-protagonist-immortal", "corpus-gap"),
    ("contradict-hound-mundane", "caught"),
    ("assert-auditor-identity", "caught"),
    ("assert-demon-king", "caught"),
    ("assert-musical-magic", "corpus-gap"),
    ("assert-narrator-ticking-clock", "corpus-gap"),
    ("modern-vocab-algorithm", "caught"),
    ("modern-vocab-carbon", "caught"),
    ("killed-name-elara", "factory"),
    ("killed-name-silas", "caught"),
    ("killed-name-irondale", "caught"),
    ("killed-name-doom-wight", "caught"),
    ("wrong-register-name", "caught"),
    ("crowd-scene-cast", "pending-gate"),
    ("as-you-know-bob-leak", "pending-gate"),
    ("ornate-tag-cluster", "pending-gate"),
    ("slop-cluster", "factory"),
    ("allegory-wink", "factory"),
    ("paraphrased-contradiction", "miss"),
    // episode
    ("continuity-world-contradiction", "caught"),
    ("continuity-prior-contradiction", "caught"),
    ("continuity-unknown-entity", "caught"),
    ("continuity-proposed-as-fact", "caught"),
];

#[derive(Debug, Deserialize)]
struct OperatorFile {
    operators: Vec<OperatorRecord>,
}

#[derive(Debug, Deserialize)]
struct OperatorRecord {
    operator: String,
}

fn fixture_operator_ids() -> BTreeSet<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/mutation");
    let mut ids = BTreeSet::new();
    for file in ["canon.yaml", "draft.yaml", "episode.yaml"] {
        let text = std::fs::read_to_string(root.join(file)).expect("read fixture");
        let doc: OperatorFile = serde_yaml_ng::from_str(&text).expect("parse fixture");
        for record in doc.operators {
            assert!(
                ids.insert(record.operator.clone()),
                "duplicate operator id across fixtures: {}",
                record.operator
            );
        }
    }
    ids
}

#[test]
fn census_every_operator_is_exercised() {
    let fixtures = fixture_operator_ids();
    let exercised: BTreeSet<String> = EXERCISED.iter().map(|(id, _)| (*id).to_owned()).collect();

    assert_eq!(
        exercised.len(),
        EXERCISED.len(),
        "the EXERCISED table has a duplicate id"
    );
    let unrun: Vec<&String> = fixtures.difference(&exercised).collect();
    assert!(
        unrun.is_empty(),
        "operators in the fixtures with no test in this suite (silently unrun): {unrun:?}"
    );
    let stale: Vec<&String> = exercised.difference(&fixtures).collect();
    assert!(
        stale.is_empty(),
        "tests naming operators absent from the fixtures (stale ids): {stale:?}"
    );
    assert_eq!(fixtures.len(), 33, "the ported corpus has 33 operators");
}

#[test]
fn census_outcome_tally_matches_the_coverage_doc() {
    // The census tally the coverage doc reports. If an operator's asserted outcome
    // changes, this fails until the doc and the tally agree.
    let mut caught = 0;
    let mut factory = 0;
    let mut corpus_gap = 0;
    let mut pending = 0;
    let mut miss = 0;
    let mut known_divergence = 0;
    for (_, outcome) in EXERCISED {
        match *outcome {
            "caught" => caught += 1,
            "factory" => factory += 1,
            "corpus-gap" => corpus_gap += 1,
            "pending-gate" => pending += 1,
            "miss" => miss += 1,
            "known-divergence" => known_divergence += 1,
            other => panic!("unknown outcome tag: {other}"),
        }
    }
    // `cite-resolved-question` flipped pending-gate → caught when the B1
    // citation-existence gate landed (a forged citation now raises
    // `citation.unknown-decision`), so caught is 22 and pending is 3.
    assert_eq!(caught, 22, "clean kernel catches");
    assert_eq!(
        known_divergence, 1,
        "dissolved class (registry-file-desync)"
    );
    assert_eq!(corpus_gap, 3, "flagged but needs a vendored constraint");
    assert_eq!(factory, 3, "correct factory-side no-catch");
    assert_eq!(pending, 3, "gate lands in a later node");
    assert_eq!(miss, 1, "the honest deterministic hole");
    assert_eq!(
        caught + known_divergence + corpus_gap + factory + pending + miss,
        33
    );
}
