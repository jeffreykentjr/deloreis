#![allow(dead_code)]

//! Shared helpers for LS-1/LS-2/LS-3 lorestore oracle tests.
//!
//! The corpus-bridge section mirrors `crates/dli-world/tests/check.rs:corpus_pack`.

use serde::Deserialize;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, EntityDecl, EntityKind, FactDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_lorestore::ClaimId;
use dli_lorestore::FactObject;
use dli_lorestore::lower::{render_monomial, render_polynomial};
use dli_world::pack::Pack;

// ── fact-object fixture ────────────────────────────────────────────────

/// Build a simple `FactObject` for testing with a positive support polynomial.
pub fn test_fact(
    claim_label: &str,
    pred: &str,
    args: &[&str],
    support: PosBool,
    grade: Grade,
) -> FactObject {
    FactObject::new(
        ClaimId::from_label(claim_label),
        dli_eval::world::GroundAtom {
            pred: pred.to_owned(),
            args: args.iter().map(ToString::to_string).collect(),
        },
        support,
        PosBool::FALSE,
        Vec::new(),
        grade,
    )
}

// ── test corpus bridge ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct FactsDoc {
    #[serde(default)]
    pub entities: Vec<EntityRow>,
    #[serde(default)]
    pub relations: Vec<RelRow>,
    #[serde(default)]
    pub facts: Vec<FactRow>,
}

#[derive(Debug, Deserialize)]
pub struct EntityRow {
    pub id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub decisions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RelRow {
    pub subject: String,
    pub kind: String,
    pub object: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub decisions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct FactRow {
    pub entity: String,
    pub key: String,
    pub value: serde_yaml_ng::Value,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub decisions: Vec<String>,
}

pub fn id(text: &str) -> Ident {
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
        body: dli_ast::language::RatifyBody::default(),
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

/// Bridge the test corpus into a `Pack`.
pub fn corpus_pack(facts: &FactsDoc) -> Pack {
    use std::collections::BTreeSet;
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

/// A small original corpus covering asserted, proposed, and ruled lore.
pub fn load_corpus() -> FactsDoc {
    let source = r"
entities:
  - {id: wren, kind: person, status: ratified, decisions: [D-1]}
  - {id: thornfield, kind: place, status: ratified, decisions: [D-1]}
  - {id: ashby, kind: place, status: proposed}
facts:
  - {entity: wren, key: home, value: thornfield, status: ratified, decisions: [D-1]}
  - {entity: wren, key: trade, value: courier, status: proposed}
relations:
  - {subject: thornfield, kind: borders, object: ashby, status: provisional, decisions: [D-2]}
";
    serde_yaml_ng::from_str(source).expect("the original test corpus parses")
}

// ── comparison helpers ─────────────────────────────────────────────────

/// Render a `World`'s derived relation as a sorted list of canonical strings.
///
/// The row spans **both** annotation channels — support `P⁺`, the atom-global
/// defeater `P⁻`, the per-monomial defeater conditions, and the per-monomial
/// grades — every part name-resolved through the interner, so the projection is
/// interner-index-independent. Rendering the defeater channel is load-bearing
/// for the merge confluence oracle: two merged worlds that differ *only* in
/// defeaters (the negation channel) must produce different signatures, or a
/// non-monotone divergence could false-green (red-team item 1).
pub fn render_derived(world: &dli_eval::world::World) -> Vec<String> {
    let interner = &world.interner;
    let mut rows: Vec<String> = world
        .derived
        .iter()
        .map(|d| {
            let atom = serde_json::to_string(&d.atom).expect("ground atom serializes");
            let support = render_polynomial(&d.support, interner);
            let defeaters = render_polynomial(&d.defeaters, interner);
            let mut conditions: Vec<String> = d
                .conditions
                .iter()
                .map(|(m, poly)| {
                    format!(
                        "{}=>{}",
                        render_monomial(m, interner),
                        render_polynomial(poly, interner)
                    )
                })
                .collect();
            conditions.sort();
            let mut mono_grades: Vec<String> = d
                .monomial_grades
                .iter()
                .map(|(m, g)| format!("{}={g:?}", render_monomial(m, interner)))
                .collect();
            mono_grades.sort();
            format!(
                "{atom}|sup={support}|def={defeaters}|cond=[{}]|mg=[{}]|grade={:?}",
                conditions.join("; "),
                mono_grades.join("; "),
                d.grade
            )
        })
        .collect();
    rows.sort();
    rows
}

/// Render a `World`'s extensional facts as canonical strings (sorted).
pub fn render_facts(world: &dli_eval::world::World) -> Vec<String> {
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

/// Render findings as comparable strings (sorted).
pub fn render_findings(findings: &[dli_world::model::LoadFinding]) -> Vec<String> {
    let mut rows: Vec<String> = findings
        .iter()
        .map(|f| format!("{}:{}", f.code(), f.message()))
        .collect();
    rows.sort();
    rows
}
