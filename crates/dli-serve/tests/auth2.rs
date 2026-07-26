//! AUTH-2: the migration guide's worked Door-T scenario against the pinned
//! `TPtH` corpus. The golden exchange is world-owned under `worlds/tpth/auth2`;
//! the consumer-owned CP-10 transcript remains EXT-F4.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, EntityDecl, EntityKind, FactDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_serve::author::store::Store;
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_syntax::{format_document, parse_document};
use dli_world::model::{LoadedAtom, Provenance};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const FIXTURE_NAME: &str = "worlds/tpth/auth2/author-tegwen-v1";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tpth_root() -> PathBuf {
    repo_root().join("worlds/tpth")
}

fn fixture_path() -> PathBuf {
    tpth_root().join("auth2/author-tegwen-v1.json")
}

fn copy_project() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("create isolated AUTH-2 project");
    fs::create_dir_all(temp.path().join("auth2")).unwrap();
    fs::create_dir_all(temp.path().join("packs")).unwrap();
    fs::copy(
        tpth_root().join("auth2-project.toml"),
        temp.path().join("deloreis.toml"),
    )
    .unwrap();
    fs::copy(
        tpth_root().join("auth2/pinned-facts.dli"),
        temp.path().join("auth2/pinned-facts.dli"),
    )
    .unwrap();
    fs::copy(
        tpth_root().join("auth2/pinned-constraints.dli"),
        temp.path().join("auth2/pinned-constraints.dli"),
    )
    .unwrap();
    fs::copy(
        tpth_root().join("auth2/fork.dli"),
        temp.path().join("auth2/fork.dli"),
    )
    .unwrap();
    fs::copy(
        tpth_root().join("auth2/pinned-guardrails.yaml"),
        temp.path().join("auth2/pinned-guardrails.yaml"),
    )
    .unwrap();
    fs::copy(
        tpth_root().join("packs/cymric-fringe.dli"),
        temp.path().join("packs/cymric-fringe.dli"),
    )
    .unwrap();
    temp
}

fn runtime() -> (tempfile::TempDir, Sessions, Store) {
    let temp = copy_project();
    let project = Arc::new(Project::load(temp.path()).expect("AUTH-2 project loads"));
    let packs = project.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    let sessions = Sessions::from_project(Arc::clone(&project));
    let store = Store::with_packs(packs).expect("AUTH-2 project compiles");
    (temp, sessions, store)
}

fn requests() -> Vec<Value> {
    vec![
        json!({
            "dli": "0.5.1",
            "id": "1",
            "op": "author.open",
            "params": {"fork": "ep9-cast"}
        }),
        json!({
            "dli": "0.5.1",
            "id": "2",
            "op": "author.add_entity",
            "params": {
                "workspace": "ws-1",
                "kind": "person",
                "id": "tegwen",
                "name": "Tegwen",
                "provenance": {
                    "kind": "proposed",
                    "origin": "agent:factory/ep9-cast"
                }
            }
        }),
        json!({
            "dli": "0.5.1",
            "id": "3",
            "op": "dli.name.check",
            "params": {"name": "tegwen", "dialect": "cymric-fringe"}
        }),
        json!({
            "dli": "0.5.1",
            "id": "4",
            "op": "author.add_fact",
            "params": {
                "workspace": "ws-1",
                "entity": "tegwen",
                "key": "trade",
                "value": "net-mender",
                "provenance": {
                    "kind": "proposed",
                    "origin": "agent:factory/ep9-cast"
                }
            }
        }),
        json!({
            "dli": "0.5.1",
            "id": "5",
            "op": "author.add_relation",
            "params": {
                "workspace": "ws-1",
                "relation": "located_in",
                "from": "tegwen",
                "to": "gwern",
                "provenance": {
                    "kind": "proposed",
                    "origin": "agent:factory/ep9-cast"
                }
            }
        }),
        json!({
            "dli": "0.5.1",
            "id": "6",
            "op": "author.diff",
            "params": {"workspace": "ws-1"}
        }),
        json!({
            "dli": "0.5.1",
            "id": "7",
            "op": "author.materialize",
            "params": {"workspace": "ws-1"}
        }),
    ]
}

fn drive(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let response = respond(&frame.to_string(), sessions, store)
        .unwrap_or_else(|| panic!("AUTH-2 request was not answered: {frame}"));
    serde_json::to_value(response).expect("serialize AUTH-2 response")
}

fn build_fixture() -> Value {
    let (_temp, mut sessions, mut store) = runtime();
    let steps = requests()
        .into_iter()
        .map(|send| {
            let expect = drive(&send, &mut sessions, &mut store);
            json!({"send": send, "expect": expect})
        })
        .collect::<Vec<_>>();
    json!({
        "fixture": FIXTURE_NAME,
        "version": 1,
        "project": "worlds/tpth/auth2-project.toml",
        "pinned_corpus": "worlds/tpth/vendor/fantasy-40f6679/canon/data/facts.yaml",
        "note": "The migration.md section 10 Door-T scenario over the pinned TPtH base. Gwern comes from the ratified vendored corpus; only Tegwen, her trade, and her location relation are staged.",
        "steps": steps
    })
}

fn pinned_facts_path() -> PathBuf {
    tpth_root().join("vendor/fantasy-40f6679/canon/data/facts.yaml")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FactsDoc {
    #[serde(default)]
    entities: Vec<EntityRow>,
    #[serde(default)]
    relations: Vec<RelationRow>,
    #[serde(default)]
    facts: Vec<FactRow>,
    #[serde(default)]
    constraints: Vec<ConstraintRow>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntityRow {
    id: String,
    kind: String,
    name: String,
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationRow {
    subject: String,
    kind: String,
    object: String,
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FactRow {
    entity: String,
    key: String,
    value: serde_yaml_ng::Value,
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstraintRow {
    id: String,
    description: String,
    decisions: Vec<String>,
    machine_check: MachineCheckRow,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MachineCheckRow {
    kind: String,
    pattern: String,
    target: String,
}

fn pinned_facts() -> FactsDoc {
    serde_yaml_ng::from_str(&fs::read_to_string(pinned_facts_path()).expect("read pinned facts"))
        .expect("parse pinned facts")
}

fn ident(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn annotation(status: &str, decisions: &[String]) -> Annotation {
    let marker = match status {
        "ratified" => ProvenanceMarker::Ratified,
        "provisional" => ProvenanceMarker::Provisional,
        "proposed" => ProvenanceMarker::Proposed,
        other => panic!("unsupported pinned status {other}"),
    };
    Annotation {
        marker,
        citations: decisions
            .iter()
            .map(|decision| CitationRef {
                id: ident(decision),
            })
            .collect(),
        condition: None,
        tags: matches!(marker, ProvenanceMarker::Proposed)
            .then(|| vec!["imported:fantasy-40f6679".to_owned()])
            .unwrap_or_default(),
        span: Span::synthetic(),
    }
}

fn entity_kind(kind: &str) -> EntityKind {
    match kind {
        "place" => EntityKind::Place,
        "era" => EntityKind::Era,
        "event" => EntityKind::Event,
        "person" | "character" => EntityKind::Person,
        "creature" | "faction" | "order" | "concept" => EntityKind::Thing,
        other => panic!("unsupported pinned entity kind {other}"),
    }
}

fn symbol_or_string(value: &str) -> Term {
    let is_symbol = !value.is_empty()
        && !value.bytes().all(|byte| byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        });
    if is_symbol {
        Term::Sym(ident(value))
    } else {
        Term::Str(value.to_owned())
    }
}

fn term(value: &serde_yaml_ng::Value) -> Term {
    match value {
        serde_yaml_ng::Value::Bool(value) => Term::Sym(ident(&value.to_string())),
        serde_yaml_ng::Value::Number(value) => value
            .as_i64()
            .map_or_else(|| Term::Str(value.to_string()), Term::Int),
        serde_yaml_ng::Value::String(value) => symbol_or_string(value),
        other => Term::Str(
            serde_yaml_ng::to_string(other)
                .expect("render pinned value")
                .trim()
                .to_owned(),
        ),
    }
}

fn pinned_projection() -> String {
    let facts = pinned_facts();
    let mut decisions = std::collections::BTreeSet::new();
    for row in &facts.entities {
        decisions.extend(row.decisions.iter().cloned());
    }
    for row in &facts.relations {
        decisions.extend(row.decisions.iter().cloned());
    }
    for row in &facts.facts {
        decisions.extend(row.decisions.iter().cloned());
    }
    let mut decls = decisions
        .into_iter()
        .map(|decision| {
            Decl::Ratify(RatifyDecl {
                decision: ident(&decision),
                body: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    decls.extend(facts.entities.iter().map(|row| {
        Decl::Entity(EntityDecl {
            kind: entity_kind(&row.kind),
            name: ident(&row.id),
            label: Some(row.name.clone()),
            time: None,
            slots: Vec::new(),
            provenance: Some(annotation(&row.status, &row.decisions)),
        })
    }));
    decls.extend(facts.entities.iter().map(|row| {
        Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred: ident("source-kind"),
                args: vec![Term::Sym(ident(&row.id)), Term::Sym(ident(&row.kind))],
            },
            triple: true,
            provenance: Some(annotation(&row.status, &row.decisions)),
        })
    }));
    decls.extend(facts.entities.iter().filter_map(|row| {
        row.note.as_ref().map(|note| {
            Decl::Fact(FactDecl {
                atom: AtomExpr {
                    pred: ident("source-note"),
                    args: vec![Term::Sym(ident(&row.id)), Term::Str(note.clone())],
                },
                triple: true,
                provenance: Some(annotation(&row.status, &row.decisions)),
            })
        })
    }));
    decls.extend(facts.relations.iter().map(|row| {
        Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred: ident(&row.kind),
                args: vec![
                    Term::Sym(ident(&row.subject)),
                    Term::Sym(ident(&row.object)),
                ],
            },
            triple: true,
            provenance: Some(annotation(&row.status, &row.decisions)),
        })
    }));
    decls.extend(facts.facts.iter().map(|row| {
        Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred: ident(&row.key),
                args: vec![Term::Sym(ident(&row.entity)), term(&row.value)],
            },
            triple: true,
            provenance: Some(annotation(&row.status, &row.decisions)),
        })
    }));
    format_document(&Document {
        scope: Some(ScopeHeader {
            name: ident("tpth-pinned-facts"),
            span: Span::synthetic(),
        }),
        decls,
    })
}

fn pinned_constraints_projection() -> String {
    let facts = pinned_facts();
    let mut decisions = facts
        .constraints
        .iter()
        .flat_map(|row| row.decisions.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|decision| {
            Decl::Ratify(RatifyDecl {
                decision: ident(&decision),
                body: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    decisions.extend(facts.constraints.iter().flat_map(constraint_decls));
    format_document(&Document {
        scope: Some(ScopeHeader {
            name: ident("tpth-pinned-constraints"),
            span: Span::synthetic(),
        }),
        decls: decisions,
    })
}

fn constraint_decls(row: &ConstraintRow) -> Vec<Decl> {
    let provenance = || Some(annotation("ratified", &row.decisions));
    [
        (
            "constraint-status",
            vec![Term::Sym(ident(&row.id)), Term::Sym(ident("rejected"))],
        ),
        (
            "constraint-description",
            vec![
                Term::Sym(ident(&row.id)),
                Term::Str(row.description.clone()),
            ],
        ),
        (
            "constraint-kind",
            vec![
                Term::Sym(ident(&row.id)),
                Term::Sym(ident(&row.machine_check.kind)),
            ],
        ),
        (
            "constraint-pattern",
            vec![
                Term::Sym(ident(&row.id)),
                Term::Str(row.machine_check.pattern.clone()),
            ],
        ),
        (
            "constraint-target",
            vec![
                Term::Sym(ident(&row.id)),
                Term::Sym(ident(&row.machine_check.target)),
            ],
        ),
    ]
    .into_iter()
    .map(|(predicate, args)| {
        Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred: ident(predicate),
                args,
            },
            triple: true,
            provenance: provenance(),
        })
    })
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SemanticRow {
    predicate: String,
    arguments: Vec<String>,
    provenance: String,
    citations: Vec<String>,
}

fn term_key(term: &Term) -> String {
    match term {
        Term::Var(id) => format!("var:{}", id.text),
        Term::Sym(id) => format!("sym:{}", id.text),
        Term::Str(value) => format!("str:{value}"),
        Term::Int(value) => format!("int:{value}"),
        Term::Time(value) => format!("time:{value:?}"),
    }
}

fn marker_key(marker: ProvenanceMarker) -> &'static str {
    match marker {
        ProvenanceMarker::Proposed => "proposed",
        ProvenanceMarker::Decided => "decided",
        ProvenanceMarker::Ratified => "ratified",
        ProvenanceMarker::Provisional => "provisional",
    }
}

fn provenance_key(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::Proposed => "proposed",
        Provenance::Decided => "decided",
        Provenance::Ratified => "ratified",
        Provenance::Provisional => "provisional",
        Provenance::Unprovenanced => "unprovenanced",
    }
}

fn semantic_row(
    atom: &AtomExpr,
    provenance: impl Into<String>,
    citations: impl IntoIterator<Item = String>,
) -> SemanticRow {
    SemanticRow {
        predicate: atom.pred.text.clone(),
        arguments: atom.args.iter().map(term_key).collect(),
        provenance: provenance.into(),
        citations: citations.into_iter().collect(),
    }
}

fn expected_projection_rows(facts: &FactsDoc) -> Vec<SemanticRow> {
    let mut rows = Vec::new();
    for row in &facts.entities {
        rows.push(semantic_row(
            &AtomExpr {
                pred: ident(entity_kind(&row.kind).keyword()),
                args: vec![Term::Sym(ident(&row.id))],
            },
            row.status.clone(),
            row.decisions.clone(),
        ));
        rows.push(semantic_row(
            &AtomExpr {
                pred: ident("source-kind"),
                args: vec![Term::Sym(ident(&row.id)), Term::Sym(ident(&row.kind))],
            },
            row.status.clone(),
            row.decisions.clone(),
        ));
        if let Some(note) = &row.note {
            rows.push(semantic_row(
                &AtomExpr {
                    pred: ident("source-note"),
                    args: vec![Term::Sym(ident(&row.id)), Term::Str(note.clone())],
                },
                row.status.clone(),
                row.decisions.clone(),
            ));
        }
    }
    rows.extend(facts.relations.iter().map(|row| {
        semantic_row(
            &AtomExpr {
                pred: ident(&row.kind),
                args: vec![
                    Term::Sym(ident(&row.subject)),
                    Term::Sym(ident(&row.object)),
                ],
            },
            row.status.clone(),
            row.decisions.clone(),
        )
    }));
    rows.extend(facts.facts.iter().map(|row| {
        semantic_row(
            &AtomExpr {
                pred: ident(&row.key),
                args: vec![Term::Sym(ident(&row.entity)), term(&row.value)],
            },
            row.status.clone(),
            row.decisions.clone(),
        )
    }));
    rows.extend(facts.constraints.iter().flat_map(|row| {
        constraint_decls(row).into_iter().map(|decl| match decl {
            Decl::Fact(fact) => semantic_row(&fact.atom, "ratified", row.decisions.clone()),
            _ => unreachable!("constraint_decls emits facts only"),
        })
    }));
    rows.sort();
    rows
}

fn parsed_projection_rows(document: &Document) -> Vec<SemanticRow> {
    let mut rows = document
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Entity(entity) => {
                let provenance = entity.provenance.as_ref().expect("entity provenance");
                Some(semantic_row(
                    &AtomExpr {
                        pred: ident(entity.kind.keyword()),
                        args: vec![Term::Sym(ident(&entity.name.text))],
                    },
                    marker_key(provenance.marker),
                    provenance
                        .citations
                        .iter()
                        .map(|citation| citation.id.text.clone()),
                ))
            }
            Decl::Fact(fact) => {
                let provenance = fact.provenance.as_ref().expect("fact provenance");
                Some(semantic_row(
                    &fact.atom,
                    marker_key(provenance.marker),
                    provenance
                        .citations
                        .iter()
                        .map(|citation| citation.id.text.clone()),
                ))
            }
            Decl::Ratify(_) => None,
            other => panic!("unexpected declaration in pinned projection: {other:?}"),
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn loaded_projection_rows(atoms: &[LoadedAtom]) -> Vec<SemanticRow> {
    let mut rows = atoms
        .iter()
        .filter(|atom| atom.pack.starts_with("tpth-pinned-"))
        .map(|atom| {
            semantic_row(
                &atom.atom,
                provenance_key(atom.kind),
                atom.citations.clone(),
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn loaded_constraint_rows() -> Vec<SemanticRow> {
    let temp = copy_project();
    let project = Project::load(temp.path()).expect("AUTH-2 project loads");
    let constraints = project
        .document("tpth-constraints")
        .expect("AUTH-2 project exposes the pinned constraint document");
    let world = dli_world::compile(&constraints.pack).expect("pinned constraints compile");
    loaded_projection_rows(world.model.atoms())
}

#[derive(Serialize)]
struct GuardrailFile {
    killed: Vec<GuardrailRow>,
}

#[derive(Serialize)]
struct GuardrailRow {
    token: String,
    #[serde(rename = "match")]
    mode: &'static str,
    decision: String,
    scope: String,
}

fn token_from_pattern(pattern: &str) -> String {
    pattern.replace("\\b", "").trim().to_owned()
}

fn pinned_guardrails() -> String {
    let killed = pinned_facts()
        .constraints
        .into_iter()
        .flat_map(|constraint| {
            let token = token_from_pattern(&constraint.machine_check.pattern);
            let mode = match constraint.machine_check.kind.as_str() {
                "vocabulary" if token.contains(' ') => "phrase",
                "name_ban" | "vocabulary" => "word",
                _ => return Vec::new(),
            };
            constraint
                .decisions
                .into_iter()
                .map(|decision| GuardrailRow {
                    token: token.clone(),
                    mode,
                    decision,
                    scope: constraint.description.clone(),
                })
                .collect()
        })
        .collect();
    serde_yaml_ng::to_string(&GuardrailFile { killed }).expect("render pinned guardrails")
}

fn assert_constraints_are_preserved(facts: &FactsDoc) {
    let ids = facts
        .constraints
        .iter()
        .map(|constraint| {
            assert!(!constraint.description.trim().is_empty());
            assert!(!constraint.decisions.is_empty());
            assert!(!constraint.machine_check.kind.trim().is_empty());
            assert!(!constraint.machine_check.pattern.trim().is_empty());
            assert!(!constraint.machine_check.target.trim().is_empty());
            constraint.id.as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            "no-auditor-concept",
            "no-singular-demon-king",
            "no-monster-of-the-week"
        ],
        "the pinned constraint inventory must remain closed"
    );
}

fn parse_projection(source: &str) -> Document {
    let (document, diagnostics) = parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
    document
}

fn assert_guardrail_rejected(
    value: &str,
    citations: &[&str],
    workspace: &str,
    sessions: &mut Sessions,
    store: &mut Store,
) {
    let id = citations.join("-");
    let added = drive(
        &json!({
            "dli": "0.5.1",
            "id": format!("guardrail-add-{id}"),
            "op": "author.add_fact",
            "params": {
                "workspace": workspace,
                "entity": "protagonist",
                "key": "identity",
                "value": value,
                "provenance": {
                    "kind": "proposed",
                    "origin": "agent:redteam/auth2"
                }
            }
        }),
        sessions,
        store,
    );
    assert!(
        added["result"]["findings"]
            .as_array()
            .is_some_and(|findings| findings.iter().any(|finding| {
                finding["code"] == "factcheck.contradicted"
                    && citations.iter().all(|citation| {
                        finding["citations"]
                            .as_array()
                            .is_some_and(|actual| actual.iter().any(|value| value == citation))
                    })
            })),
        "lexical guardrail `{value}` was not returned immediately: {added:#}"
    );
    let rejected = drive(
        &json!({
            "dli": "0.5.1",
            "id": format!("guardrail-check-{id}"),
            "op": "author.check",
            "params": {"workspace": workspace}
        }),
        sessions,
        store,
    );
    assert!(
        rejected["result"]["findings"]
            .as_array()
            .is_some_and(|findings| findings.iter().any(|finding| {
                finding["code"] == "factcheck.contradicted"
                    && citations.iter().all(|citation| {
                        finding["citations"]
                            .as_array()
                            .is_some_and(|actual| actual.iter().any(|value| value == citation))
                    })
            })),
        "lexical guardrail `{value}` did not execute: {rejected:#}"
    );
    let findings = drive(
        &json!({
            "dli": "0.5.1",
            "id": format!("guardrail-findings-{id}"),
            "op": "author.findings",
            "params": {"workspace": workspace}
        }),
        sessions,
        store,
    );
    assert_eq!(
        findings["result"]["materializable"],
        json!(false),
        "{findings:#}"
    );
    let materialized = drive(
        &json!({
            "dli": "0.5.1",
            "id": format!("guardrail-materialize-{id}"),
            "op": "author.materialize",
            "params": {"workspace": workspace}
        }),
        sessions,
        store,
    );
    assert_eq!(
        materialized["error"]["code"], "materialize-blocked",
        "{materialized:#}"
    );
}

fn assert_entity_guardrail_rejected(
    id: &str,
    name: &str,
    citation: &str,
    workspace: &str,
    sessions: &mut Sessions,
    store: &mut Store,
) {
    let added = drive(
        &json!({
            "dli": "0.5.1",
            "id": format!("guardrail-entity-{id}"),
            "op": "author.add_entity",
            "params": {
                "workspace": workspace,
                "kind": "person",
                "id": id,
                "name": name,
                "provenance": {
                    "kind": "proposed",
                    "origin": "agent:redteam/auth2"
                }
            }
        }),
        sessions,
        store,
    );
    assert!(
        added["result"]["findings"]
            .as_array()
            .is_some_and(|findings| findings.iter().any(|finding| {
                finding["code"] == "factcheck.contradicted"
                    && finding["citations"]
                        .as_array()
                        .is_some_and(|citations| citations.iter().any(|value| value == citation))
            })),
        "killed entity `{id}` / `{name}` was not rejected immediately: {added:#}"
    );
}

fn assert_guardrail_finding(response: &Value, citations: &[&str]) {
    assert!(
        response["result"]["findings"]
            .as_array()
            .is_some_and(|findings| findings.iter().any(|finding| {
                finding["code"] == "factcheck.contradicted"
                    && citations.iter().all(|citation| {
                        finding["citations"]
                            .as_array()
                            .is_some_and(|actual| actual.iter().any(|value| value == citation))
                    })
            })),
        "guardrail finding missing from response: {response:#}"
    );
}

fn pinned_gwern_row() -> serde_yaml_ng::Value {
    let path = tpth_root().join("vendor/fantasy-40f6679/canon/data/facts.yaml");
    let document: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&fs::read_to_string(path).expect("read pinned facts"))
            .expect("parse pinned facts");
    document["entities"]
        .as_sequence()
        .expect("entities sequence")
        .iter()
        .find(|row| row["id"].as_str() == Some("gwern"))
        .cloned()
        .expect("pinned corpus declares gwern")
}

#[test]
fn fixture_base_preserves_every_pinned_atomic_row() {
    let facts = pinned_facts();
    let row = pinned_gwern_row();
    assert_eq!(row["kind"].as_str(), Some("place"));
    assert_eq!(row["name"].as_str(), Some("Gwern"));
    assert_eq!(row["status"].as_str(), Some("ratified"));
    let decisions = row["decisions"]
        .as_sequence()
        .expect("gwern decisions")
        .iter()
        .filter_map(serde_yaml_ng::Value::as_str)
        .collect::<Vec<_>>();
    assert_eq!(decisions, ["D-0108", "D-0111"]);

    let projection = fs::read_to_string(tpth_root().join("auth2/pinned-facts.dli")).unwrap();
    assert_eq!(projection, pinned_projection());
    let constraints = fs::read_to_string(tpth_root().join("auth2/pinned-constraints.dli")).unwrap();
    assert_eq!(constraints, pinned_constraints_projection());
    assert!(projection.contains("ratify D-0108"));
    assert!(projection.contains("ratify D-0111"));
    let document = parse_projection(&projection);
    let constraint_document = parse_projection(&constraints);
    let gwern = document
        .decls
        .iter()
        .find_map(|decl| match decl {
            Decl::Entity(entity) if entity.name.text == "gwern" => Some(entity),
            _ => None,
        })
        .expect("projection declares gwern");
    assert_eq!(gwern.kind, EntityKind::Place);
    assert_eq!(gwern.label.as_deref(), Some("Gwern"));
    let provenance = gwern.provenance.as_ref().expect("gwern has provenance");
    assert_eq!(provenance.marker, ProvenanceMarker::Ratified);
    assert_eq!(
        provenance
            .citations
            .iter()
            .map(|citation| citation.id.text.as_str())
            .collect::<Vec<_>>(),
        ["D-0108", "D-0111"]
    );

    let entity_rows = document
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Entity(entity) => Some((
                entity.name.text.as_str(),
                entity.kind,
                entity.label.as_deref(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    for source in &facts.entities {
        assert!(
            entity_rows.contains(&(
                source.id.as_str(),
                entity_kind(&source.kind),
                Some(source.name.as_str())
            )),
            "entity row was not preserved: {}",
            source.id
        );
    }
    assert_eq!(entity_rows.len(), facts.entities.len());
    let mut parsed_rows = parsed_projection_rows(&document);
    parsed_rows.extend(parsed_projection_rows(&constraint_document));
    parsed_rows.sort();
    assert_eq!(
        parsed_rows,
        expected_projection_rows(&facts),
        "the parser must preserve every projected entity, relation, fact, constraint, source kind, and note"
    );

    let projected_decisions = document
        .decls
        .iter()
        .chain(&constraint_document.decls)
        .filter_map(|decl| match decl {
            Decl::Ratify(ratify) => Some(ratify.decision.text.as_str()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let expected_decisions = facts
        .entities
        .iter()
        .flat_map(|row| &row.decisions)
        .chain(facts.relations.iter().flat_map(|row| &row.decisions))
        .chain(facts.facts.iter().flat_map(|row| &row.decisions))
        .chain(facts.constraints.iter().flat_map(|row| &row.decisions))
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(projected_decisions, expected_decisions);

    assert_constraints_are_preserved(&facts);
    assert_eq!(
        fs::read_to_string(tpth_root().join("auth2/pinned-guardrails.yaml")).unwrap(),
        pinned_guardrails(),
        "the executable lexical guardrails must be regenerated from the pinned constraints"
    );
}

#[test]
fn fixture_project_observes_the_full_pinned_corpus() {
    let facts = pinned_facts();
    assert_eq!(facts.entities.len(), 23);
    assert_eq!(facts.relations.len(), 12);
    assert_eq!(facts.facts.len(), 156);
    assert_eq!(facts.constraints.len(), 3);

    let (_temp, mut sessions, mut store) = runtime();
    let opened = drive(&requests()[0], &mut sessions, &mut store);
    let workspace = opened["result"]["workspace"].as_str().unwrap();
    let base = store.workspace_base(workspace).expect("opened AUTH-2 base");
    let mut loaded_rows = loaded_projection_rows(base.model.atoms());
    loaded_rows.extend(loaded_constraint_rows());
    loaded_rows.sort();
    assert_eq!(
        loaded_rows,
        expected_projection_rows(&facts),
        "the real project loader must retain every projected semantic row"
    );
    let contradiction = drive(
        &json!({
            "dli": "0.5.1",
            "id": "probe",
            "op": "author.add_fact",
            "params": {
                "workspace": workspace,
                "entity": "gwern",
                "key": "kind",
                "value": "mountain",
                "provenance": {
                    "kind": "proposed",
                    "origin": "agent:redteam/auth2"
                }
            }
        }),
        &mut sessions,
        &mut store,
    );
    assert!(
        contradiction["result"]["findings"]
            .as_array()
            .is_some_and(|findings| findings
                .iter()
                .any(|finding| finding["code"] == "fact.contradiction")),
        "{contradiction:#}"
    );

    assert_guardrail_rejected("auditor", &["D-0009"], workspace, &mut sessions, &mut store);
    assert_guardrail_rejected(
        "demon king",
        &["D-0010", "D-0033"],
        workspace,
        &mut sessions,
        &mut store,
    );
}

#[test]
fn fixture_project_rejects_killed_entity_ids_and_labels() {
    let (_temp, mut sessions, mut store) = runtime();
    let opened = drive(&requests()[0], &mut sessions, &mut store);
    let workspace = opened["result"]["workspace"].as_str().unwrap();

    assert_entity_guardrail_rejected(
        "auditor",
        "Harmless Label",
        "D-0009",
        workspace,
        &mut sessions,
        &mut store,
    );
    assert_entity_guardrail_rejected(
        "harmless-id",
        "The Auditor",
        "D-0009",
        workspace,
        &mut sessions,
        &mut store,
    );

    let findings = drive(
        &json!({
            "dli": "0.5.1",
            "id": "guardrail-entity-findings",
            "op": "author.findings",
            "params": {"workspace": workspace}
        }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(findings["result"]["materializable"], json!(false));
    assert!(
        findings["result"]["errors"]
            .as_array()
            .is_some_and(|errors| errors
                .iter()
                .any(|finding| finding["code"] == "factcheck.contradicted")),
        "{findings:#}"
    );
}

#[test]
fn fixture_project_rejects_killed_question_option_and_hypothesis_text() {
    let (_temp, mut sessions, mut store) = runtime();
    let opened = drive(&requests()[0], &mut sessions, &mut store);
    let workspace = opened["result"]["workspace"].as_str().unwrap();

    let question = drive(
        &json!({
            "dli": "0.5.1",
            "id": "guardrail-question",
            "op": "author.open_question",
            "params": {
                "workspace": workspace,
                "id": "protagonist-role",
                "prompt": "Could the protagonist be an Auditor?"
            }
        }),
        &mut sessions,
        &mut store,
    );
    assert_guardrail_finding(&question, &["D-0009"]);

    let clean_question = drive(
        &json!({
            "dli": "0.5.1",
            "id": "guardrail-clean-question",
            "op": "author.open_question",
            "params": {
                "workspace": workspace,
                "id": "antagonist-shape",
                "prompt": "What shape does the antagonist take?"
            }
        }),
        &mut sessions,
        &mut store,
    );
    assert!(
        clean_question["result"]["findings"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "{clean_question:#}"
    );
    let option = drive(
        &json!({
            "dli": "0.5.1",
            "id": "guardrail-option",
            "op": "author.add_option",
            "params": {
                "workspace": workspace,
                "question": "antagonist-shape",
                "id": "singular-foe",
                "label": "Demon King"
            }
        }),
        &mut sessions,
        &mut store,
    );
    assert_guardrail_finding(&option, &["D-0010", "D-0033"]);

    let hypothesis = drive(
        &json!({
            "dli": "0.5.1",
            "id": "guardrail-hypothesis",
            "op": "author.add_hypothesis",
            "params": {
                "workspace": workspace,
                "id": "protagonist-office",
                "statement": "The protagonist serves as the auditor."
            }
        }),
        &mut sessions,
        &mut store,
    );
    assert_guardrail_finding(&hypothesis, &["D-0009"]);

    let findings = drive(
        &json!({
            "dli": "0.5.1",
            "id": "guardrail-text-findings",
            "op": "author.findings",
            "params": {"workspace": workspace}
        }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(findings["result"]["materializable"], json!(false));
    let materialized = drive(
        &json!({
            "dli": "0.5.1",
            "id": "guardrail-text-materialize",
            "op": "author.materialize",
            "params": {"workspace": workspace}
        }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(materialized["error"]["code"], "materialize-blocked");
}

#[test]
fn worked_tpth_fixture_replays_exactly() {
    let fixture: Value =
        serde_json::from_str(&fs::read_to_string(fixture_path()).expect("read AUTH-2 fixture"))
            .expect("parse AUTH-2 fixture");
    assert_eq!(fixture["fixture"], FIXTURE_NAME);
    assert_eq!(fixture["project"], "worlds/tpth/auth2-project.toml");
    assert_eq!(
        fixture["pinned_corpus"],
        "worlds/tpth/vendor/fantasy-40f6679/canon/data/facts.yaml"
    );

    let (_temp, mut sessions, mut store) = runtime();
    for step in fixture["steps"].as_array().expect("fixture steps") {
        let actual = drive(&step["send"], &mut sessions, &mut store);
        assert_eq!(
            actual, step["expect"],
            "AUTH-2 response drift at request {}",
            step["send"]["id"]
        );
    }

    let relation = &fixture["steps"][4]["expect"]["result"];
    assert_eq!(relation["findings"], json!([]));
    assert_eq!(
        relation["rendered"],
        json!(["located_in(tegwen, gwern) proposed agent:factory/ep9-cast"])
    );
    let materialized = fixture["steps"][6]["expect"]["result"]["rendered"]
        .as_array()
        .expect("materialized lines");
    assert!(
        materialized.iter().all(|line| line
            .as_str()
            .is_none_or(|line| !line.starts_with("place gwern "))),
        "materialize returns staged declarations, not the pinned base"
    );
}

#[test]
#[ignore = "run explicitly to regenerate the AUTH-2 golden fixture"]
fn regenerate_worked_tpth_fixture() {
    let mut text = serde_json::to_string_pretty(&build_fixture()).expect("serialize fixture");
    text.push('\n');
    fs::write(fixture_path(), text).expect("write AUTH-2 fixture");
}

#[test]
#[ignore = "run explicitly to regenerate the committed pinned facts projection"]
fn regenerate_pinned_facts_projection() {
    fs::write(
        tpth_root().join("auth2/pinned-facts.dli"),
        pinned_projection(),
    )
    .expect("write pinned facts projection");
    fs::write(
        tpth_root().join("auth2/pinned-constraints.dli"),
        pinned_constraints_projection(),
    )
    .expect("write pinned constraints projection");
    fs::write(
        tpth_root().join("auth2/pinned-guardrails.yaml"),
        pinned_guardrails(),
    )
    .expect("write pinned guardrails");
}
