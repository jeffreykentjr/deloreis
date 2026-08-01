//! `LS-1` total-lowering oracle.
//!
//! This integration test closes the loop the roadmap (doc/PLAN.md § LS-1,
//! `docs/design-notes/lorestore-roadmap.md` §LS-1) demands:
//!
//! > every AST construct lowers to fact objects, and lowering the vendored
//! > corpus then compiling reproduces identical fixpoint, findings, and digests
//!
//! Flow — shadow-read harness idiom (same as the MIG/PROOF lanes in
//! `crates/dli-world/tests/check.rs`):
//!
//! 1.  Bridge the vendored `facts.yaml` into `Decl`s (same `corpus_pack`
//!     logic the MIG lane uses).
//! 2.  Compile via `dli_world::compile` → **reference A**.
//! 3.  Lower A's `LoadedAtom`s to `FactObject`s and compute their
//!     content-address digests (verifying determinism by hashing twice).
//! 4.  Reconstruct source `Decl`s from those `FactObject`s — deriving each
//!     `Annotation` from the support polynomial via the committed interner.
//! 5.  Recompile the reconstructed documents → **reference B**.
//! 6.  Compare A and B: rendered fixpoint (`World::derived`), findings, and
//!     the set of `FactObject` digests.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, EntityDecl, EntityKind, FactDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::posbool::PosBool;

use dli_lorestore::identity::ClaimId;
use dli_lorestore::lower::{
    declared_decisions, derive_annotation, lower_atoms, reconstruct_pack, render_polynomial,
};
use dli_lorestore::object::FactObject;

use dli_world::compile;
use dli_world::pack::Pack;

// ── vendored corpus bridge (mirrors check.rs:corpus_pack) ──────────────

fn vendor_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../worlds/tpth/vendor/fantasy-099e04cd")
}

#[derive(Debug, Deserialize)]
struct FactsDoc {
    #[serde(default)]
    entities: Vec<EntityRow>,
    #[serde(default)]
    relations: Vec<RelRow>,
    #[serde(default)]
    facts: Vec<FactRow>,
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

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn an_marker(marker: ProvenanceMarker, cites: &[&str]) -> Annotation {
    Annotation {
        marker,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn ratify_decl(decision: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(decision),
        body: Vec::new(),
    })
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

fn provenance(status: &str, decisions: &[String]) -> Option<Annotation> {
    let marker = match status {
        "ratified" => ProvenanceMarker::Ratified,
        "provisional" => ProvenanceMarker::Provisional,
        "rejected" => return None,
        _ => ProvenanceMarker::Proposed,
    };
    let cites: Vec<&str> = if matches!(marker, ProvenanceMarker::Proposed) {
        Vec::new()
    } else {
        decisions.iter().map(String::as_str).collect()
    };
    Some(an_marker(marker, &cites))
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

fn atom(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: args.iter().map(|a| Term::Sym(id(a))).collect(),
    }
}

fn fact_decl(pred: &str, args: &[&str], prov: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, args),
        triple: false,
        provenance: prov,
    })
}

fn corpus_pack(facts: &FactsDoc) -> Pack {
    let mut decls: Vec<Decl> = Vec::new();
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
        decls.push(ratify_decl(decision));
    }
    let mut seen = BTreeSet::new();
    for entity in &facts.entities {
        if seen.insert(entity.id.clone()) {
            decls.push(Decl::Entity(EntityDecl {
                kind: map_kind(&entity.kind),
                name: id(&entity.id),
                label: None,
                time: None,
                slots: Vec::new(),
                provenance: provenance(&entity.status, &entity.decisions),
            }));
        }
    }
    for row in &facts.facts {
        decls.push(fact_decl(
            &row.key,
            &[&row.entity, &render_value(&row.value)],
            provenance(&row.status, &row.decisions),
        ));
    }
    for row in &facts.relations {
        decls.push(fact_decl(
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

fn load_corpus() -> FactsDoc {
    let path = vendor_root().join("canon/data/facts.yaml");
    let text = std::fs::read_to_string(&path).expect("read facts.yaml");
    serde_yaml_ng::from_str(&text).expect("parse facts.yaml")
}

// ── comparison helpers ─────────────────────────────────────────────────

/// Render a `World`'s derived relation as a sorted list of canonical strings.
fn render_derived(world: &dli_eval::world::World) -> Vec<String> {
    let interner = &world.interner;
    let mut rows: Vec<String> = world
        .derived
        .iter()
        .map(|d| {
            let support = render_polynomial(&d.support, interner);
            let atom = serde_json::to_string(&d.atom).expect("ground atom serializes");
            format!("{atom}|{support}|{:?}", d.grade)
        })
        .collect();
    rows.sort();
    rows
}

/// Render a `World`'s extensional facts as canonical strings (sorted).
fn render_facts(world: &dli_eval::world::World) -> Vec<String> {
    let interner = &world.interner;
    let mut rows: Vec<String> = world
        .facts
        .iter()
        .map(|f| {
            let support = render_polynomial(&f.ann.support, interner);
            let atom = serde_json::to_string(&f.atom).expect("ground atom serializes");
            format!("{atom}|{support}|{:?}", f.grade)
        })
        .collect();
    rows.sort();
    rows
}

/// Render findings as comparable strings.
fn render_findings(findings: &[dli_world::model::LoadFinding]) -> Vec<String> {
    let mut rows: Vec<String> = findings
        .iter()
        .map(|f| format!("{}:{}", f.code(), f.message()))
        .collect();
    rows.sort();
    rows
}

// ── the oracle ────────────────────────────────────────────────────────

#[test]
#[allow(clippy::too_many_lines, clippy::print_stdout)]
fn total_lowering_reproduces_fixpoint_findings_and_digests() {
    // 1 — bridge the vendored corpus.
    let pack = corpus_pack(&load_corpus());

    // 2 — compile reference A.
    let compiled_a = compile(&pack).expect("reference A compiles");
    let model_a = compiled_a.model();

    // 3 — lower A's LoadedAtoms to FactObjects.
    //    Using LoadedAtom (not just World::facts) captures entity declarations
    //    too, which is what the loader interns as proposal-bundle variables.
    let fact_objects = lower_atoms(model_a.atoms());

    //    Totality: every loaded atom lowers to exactly one FactObject.
    assert_eq!(
        fact_objects.len(),
        model_a.atoms().len(),
        "every LoadedAtom must lower to exactly one FactObject (DL-0041)"
    );

    //    Determinism: hashing the objects twice must yield identical digests.
    let digests_a: Vec<String> = fact_objects
        .iter()
        .map(|fo| fo.content_hash().expect("canonical encoding").to_string())
        .collect();
    let digests_a_again: Vec<String> = fact_objects
        .iter()
        .map(|fo| fo.content_hash().expect("canonical encoding").to_string())
        .collect();
    assert_eq!(
        digests_a, digests_a_again,
        "FactObject digests must be deterministic over canonical bytes"
    );

    //    No two FactObjects in the same pack may share a digest.
    let mut unique = std::collections::HashSet::new();
    for d in &digests_a {
        assert!(
            unique.insert(d.clone()),
            "duplicate FactObject digest — content-address collision: {d}"
        );
    }

    // 4 — collect known decisions (minted via Decl::Ratify) so the
    //    reconstructed pack can re-emit them.
    let known_decisions = declared_decisions(&pack);

    //    Derive annotations from FactObject support polynomials and verify
    //    that every citation resolves to a known decision.
    let interner_a = model_a.interner();
    for fo in &fact_objects {
        if let Some(ann) = derive_annotation(&fo.support, interner_a, &known_decisions) {
            for cite in &ann.citations {
                assert!(
                    known_decisions.contains(&cite.id.text),
                    "reconstructed citation {} was not a declared decision",
                    cite.id.text
                );
            }
        }
    }

    // 5 — reconstruct source Decls from the FactObjects.
    let reconstructed = reconstruct_pack(&fact_objects, interner_a, &known_decisions, &pack.name);

    //    The reconstructed documents carry ratify decls first, then entity decls,
    //    then fact decls — matching the original pack's ordering.
    let ratify_count = reconstructed
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .filter(|d| matches!(d, Decl::Ratify(_)))
        .count();
    assert_eq!(ratify_count, known_decisions.len());

    // 6 — compile the reconstructed pack (reference B).
    let compiled_b = compile(&reconstructed).expect("reference B compiles");
    let model_b = compiled_b.model();

    // ---- fidelity checks ---------------------------------------------------

    // (a) extensional facts match (by surface form + support name-strings).
    let facts_a = render_facts(&model_a.world);
    let facts_b = render_facts(&model_b.world);
    assert_eq!(
        facts_a, facts_b,
        "extensional facts must be identical after round-trip"
    );

    // (b) fixpoint (derived relation) is identical.
    let derived_a = render_derived(&model_a.world);
    let derived_b = render_derived(&model_b.world);
    assert_eq!(
        derived_a, derived_b,
        "fixpoint must be identical after round-trip"
    );
    assert_eq!(
        model_a.world.derived.len(),
        model_b.world.derived.len(),
        "derived relation row count must match"
    );

    // (c) load findings are identical (the clean corpus must be clean on both
    //     sides — charter koan 4).
    assert_eq!(
        render_findings(model_a.findings()),
        render_findings(model_b.findings()),
        "load findings must be identical after round-trip"
    );

    // (d) valuation (truth assignment over resolved names) matches.
    let names_a = model_a.interner().names().to_vec();
    let names_b = model_b.interner().names().to_vec();
    assert_eq!(names_a, names_b, "interned variable sets must match");
    for (&atom_idx, val_a) in &model_a.world.valuation.0 {
        let name = model_a.interner().resolve(atom_idx).expect("atom has name");
        let b_idx = dli_core::atom::Atom(
            u32::try_from(
                names_b
                    .iter()
                    .position(|n| n == name)
                    .expect("atom exists in B interner"),
            )
            .expect("fits in u32"),
        );
        let val_b = model_b.world.valuation.get(b_idx).unwrap_or(false);
        assert_eq!(
            *val_a, val_b,
            "valuation disagree on variable '{name}': A={val_a}, B={val_b}"
        );
    }

    // (e) FactObject digests from A and B must match — the interner is
    //     rebuilt identically (ratify-first ordering) so atom indices line up
    //     and canonical bytes are byte-for-byte stable.
    let fact_objects_b = lower_atoms(model_b.atoms());
    let digests_b: Vec<String> = fact_objects_b
        .iter()
        .map(|fo| fo.content_hash().expect("canonical encoding").to_string())
        .collect();
    assert_eq!(
        digests_a, digests_b,
        "FactObject digests must match after round-trip (interner-stable)"
    );

    println!(
        "oracle passed: {} atoms, {} facts, {} derived, {} findings, {} digests identical",
        fact_objects.len(),
        facts_a.len(),
        derived_a.len(),
        render_findings(model_a.findings()).len(),
        digests_a.len(),
    );
}

#[test]
fn fact_object_is_content_addressed_and_upcast_safe() {
    // A FactObject's ContentHash must depend only on its canonical bytes,
    // never on ClaimId (the three-way split — DL-0041).
    let atom = dli_eval::world::GroundAtom {
        pred: "place".to_owned(),
        args: vec!["camelot".to_owned()],
    };
    let support = PosBool::pos(dli_core::atom::Atom(0));

    let fo = FactObject::new(
        ClaimId::from_label("test/a"),
        atom.clone(),
        support.clone(),
        PosBool::FALSE,
        Vec::new(),
        dli_core::grade::Grade::Asserted,
    );
    let h = fo.content_hash().expect("canonical encoding").to_string();

    // Same content, different claim_id → same hash (claim_id is not in the
    // canonical content bytes).
    let mut fo_renamed = fo.clone();
    fo_renamed.claim_id = ClaimId::from_label("test/a-renamed");
    assert_eq!(
        fo_renamed
            .content_hash()
            .expect("canonical encoding")
            .to_string(),
        h,
        "content hash must be stable regardless of ClaimId label"
    );

    // Different atoms produce different hashes.
    let mut fo_diff = fo.clone();
    fo_diff.atom.args.push("guild".to_owned());
    assert_ne!(
        fo_diff
            .content_hash()
            .expect("canonical encoding")
            .to_string(),
        h,
        "different atoms must produce different hashes"
    );
}
