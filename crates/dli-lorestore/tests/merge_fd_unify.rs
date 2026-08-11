//! Issue 59: the durable merge path asks the **one** functional-dependency
//! question — `dli_core::fd` — and answers it the way every other FD surface does.
//!
//! `certain_layer_conflicts` used to key value distinctness on the raw string and
//! to infer functionality from the base's shape alone. Both halves moved onto the
//! shared primitive (DL-0075):
//!
//! 1. **Distinctness keys on [`dli_core::fd::fd_value_key`]** — cosmetic drift
//!    (case, interior hyphen/underscore/whitespace) is one value, not two, so a
//!    branch that respells a value raises no conflict. Before the fix `shallows`
//!    vs `Shallows` was a spurious FD break on the merge path while the load check
//!    called it clean — an author told the same slot was fine on one surface and
//!    broken on another.
//! 2. **Declared ∪ inferred, but split across two lenses.** The coarse
//!    derived-layer `certain_layer_conflicts` owns the **inferred** half. The
//!    author's `functional <pred>` declaration is owned by the recompile's
//!    `dli_world::check::run` battery, whose `continuity.fd-violation` is
//!    fluent-aware and subject-kind-aware over the source layer — so a declared
//!    break is raised once, by the lens that shares the load check's exclusions,
//!    and never double-reported through a coarser path (issue 59 red-team).
//!
//! The probes below pin each half on the durable store's own 3-way merge, not on
//! the primitive in isolation: the primitive has its own unit tests, and what can
//! regress here is the *wiring* — which set the merge builds and which key it
//! compares by. The scaffolding mirrors `merge.rs` (a `D-1`-citing certain layer,
//! so facts compile certain and support reconstructs order-independently).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, FactDecl, FunctionalDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::atom::Atom;
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_eval::fork::ConflictKind;
use dli_lorestore::{ContentHash, FactObject, Lorestore};
use dli_world::CompiledWorld;
use tempfile::TempDir;

/// The one ruled decision every fact cites, so it compiles certain (merge.rs).
const DECISION: &str = "D-1";

/// The predicate the author *declares* functional while the base holds no fact of
/// it — inference alone cannot see it, so only the declared set can catch a break.
const DECLARED: &str = "crest";

// ── store / branch helpers (mirrored from `merge.rs`; test files share none) ──

fn open_store() -> (Lorestore, TempDir) {
    let dir = TempDir::new().expect("tempdir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open lorestore");
    (store, dir)
}

/// A fact object citing `D-1` (support `{+D-1}`), so it compiles certain.
fn certain_fact(label: &str, pred: &str, args: &[&str]) -> FactObject {
    common::test_fact(label, pred, args, PosBool::pos(Atom(0)), Grade::Asserted)
}

/// The interner names and known decisions at a branch's head.
fn branch_state(store: &Lorestore, branch: &str) -> (Vec<String>, Vec<String>) {
    let head = store
        .branch_head(branch)
        .expect("branch head")
        .expect("branch exists");
    let commit: dli_lorestore::Commit = store.read_object(&head).expect("read").expect("commit");
    (commit.interner, commit.decisions)
}

/// Seed `main` with a root commit carrying the `D-1` interner + decision and the
/// given facts. `main` is the branch point for every test branch.
fn seed_main(store: &Lorestore, facts: &[FactObject]) {
    let mut hashes = Vec::new();
    for fo in facts {
        hashes.push(store.write_object(fo).expect("write fact"));
    }
    store
        .commit_on_branch(
            "main",
            vec![DECISION.to_owned()],
            hashes,
            Vec::new(),
            Vec::new(),
            vec![DECISION.to_owned()],
            "canon".to_owned(),
        )
        .expect("seed main");
}

/// Commit a run of fact objects to a branch, carrying its interner + decisions
/// forward (the facts below add no event variables).
fn commit_facts(store: &Lorestore, branch: &str, facts: &[FactObject]) -> ContentHash {
    let mut hashes = Vec::new();
    for fo in facts {
        hashes.push(store.write_object(fo).expect("write fact"));
    }
    let (interner, decisions) = branch_state(store, branch);
    store
        .commit_on_branch(
            branch,
            interner,
            hashes,
            Vec::new(),
            Vec::new(),
            decisions,
            "canon".to_owned(),
        )
        .expect("commit on branch")
}

// ── pack builders for the declared-functional base ───────────────────────────

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

/// A `@ratified D-1` provenance annotation.
fn ratified() -> Annotation {
    Annotation {
        marker: ProvenanceMarker::Ratified,
        citations: vec![CitationRef { id: id(DECISION) }],
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn fact_decl(pred: &str, args: &[&str]) -> Decl {
    Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id(pred),
            args: args.iter().map(|a| Term::Sym(id(a))).collect(),
        },
        triple: false,
        provenance: Some(ratified()),
    })
}

/// The base pack for the declared cases: it **declares** `functional crest` and
/// holds no `crest` fact whatsoever — the shape where inference is blind and only
/// the author's declaration can establish functionality. `home(mara, shallows)`
/// rides along as an unrelated, genuinely inferred-functional slot, so the base is
/// not degenerate.
fn declared_functional_pack() -> dli_world::Pack {
    dli_world::Pack {
        name: "canon".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("canon"),
                span: Span::synthetic(),
            }),
            decls: vec![
                Decl::Ratify(RatifyDecl {
                    decision: id(DECISION),
                    body: dli_ast::language::RatifyBody::default(),
                }),
                Decl::Functional(FunctionalDecl {
                    pred: id(DECLARED),
                    provenance: Some(ratified()),
                }),
                fact_decl("home", &["mara", "shallows"]),
            ],
        }],
    }
}

/// Import the declared-functional base onto `main`, branch `a`/`b` off it, and
/// assert the premise the declared cases rest on: `crest` is in the base model's
/// declared set, and the base certain layer holds **no** `crest` slot, so
/// `inferred_functional` over the base cannot contribute it.
fn open_declared_base() -> (Lorestore, TempDir) {
    let (store, dir) = open_store();
    store
        .import_pack(&declared_functional_pack())
        .expect("import declared-functional base");
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");

    let base = compile_branch(&store, "main");
    assert!(
        base.model.functional_predicates.contains(DECLARED),
        "the base declares {DECLARED} functional: {:?}",
        base.model.functional_predicates
    );
    assert!(
        !derived_preds(&base).iter().any(|pred| pred == DECLARED),
        "the base holds no {DECLARED} fact, so inference alone is blind to it: {:?}",
        derived_preds(&base)
    );
    (store, dir)
}

// ── world inspection ─────────────────────────────────────────────────────────

fn compile_branch(store: &Lorestore, branch: &str) -> CompiledWorld {
    let head = store
        .branch_head(branch)
        .expect("branch head")
        .expect("branch exists");
    store.compile_at(&head).expect("compile branch")
}

fn derived_preds(world: &CompiledWorld) -> Vec<String> {
    world
        .model
        .world
        .derived
        .iter()
        .map(|d| d.atom.pred.clone())
        .collect()
}

/// Every derived atom of the world at a commit, rendered `pred(a, b)`.
fn rendered_atoms(store: &Lorestore, head: &ContentHash) -> Vec<String> {
    let compiled = store.compile_at(head).expect("compile merged world");
    let mut out: Vec<String> = compiled
        .model
        .world
        .derived
        .iter()
        .map(|d| d.atom.render())
        .collect();
    out.sort();
    out
}

fn certain_conflicts(report: &dli_lorestore::MergeReport) -> Vec<&dli_eval::fork::Conflict> {
    report
        .conflicts
        .iter()
        .filter(|c| c.kind == ConflictKind::Certain)
        .collect()
}

fn fd_findings(report: &dli_lorestore::MergeReport) -> Vec<&str> {
    report
        .findings
        .iter()
        .filter(|f| f.code == "merge.functional-dependency")
        .map(|f| f.message.as_str())
        .collect()
}

/// The world-internal `continuity.fd-violation` findings the recompile's
/// `dli_world::check::run` battery raises over the merged world — the load-check
/// lens, which owns author-*declared* functional dependencies (fluent-aware and
/// subject-kind-aware).
fn continuity_fd_findings(report: &dli_lorestore::MergeReport) -> Vec<&str> {
    report
        .findings
        .iter()
        .filter(|f| f.code == "continuity.fd-violation")
        .map(|f| f.message.as_str())
        .collect()
}

// ════════════════════════════════════════════════════════════════════════════
// 1 — COSMETIC DRIFT IS ONE VALUE (acceptance criterion a)
// ════════════════════════════════════════════════════════════════════════════

/// The base's `home(mara, shallows)` makes `home` inferred-functional; a branch
/// then commits `home(mara, Shallows)` — a **cosmetic-only** respelling. Under
/// `fd_value_key` that is one value, so the merged certain layer breaks nothing.
/// Before the fix the merge path compared raw strings and raised a spurious
/// `ConflictKind::Certain` here, while the load check (already keyed) called the
/// same layer clean.
#[test]
fn merge_folds_cosmetic_value_drift_to_one_value() {
    let (store, _dir) = open_store();
    seed_main(
        &store,
        &[certain_fact("base-home", "home", &["mara", "shallows"])],
    );
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    commit_facts(
        &store,
        "b",
        &[certain_fact("b-home", "home", &["mara", "Shallows"])],
    );

    let report = store.merge("a", "b").expect("merge");

    // The probe has teeth only if both spellings genuinely survive into the
    // merged world as separate atoms — a store that deduped them would pass
    // vacuously.
    let atoms = rendered_atoms(&store, &report.merge_commit);
    assert!(
        atoms.iter().any(|a| a.contains("shallows")),
        "the base spelling survives the merge: {atoms:?}"
    );
    assert!(
        atoms.iter().any(|a| a.contains("Shallows")),
        "the branch spelling survives the merge as its own atom: {atoms:?}"
    );

    assert!(
        certain_conflicts(&report).is_empty(),
        "cosmetic drift is one value, not an FD break: {:?}",
        certain_conflicts(&report)
    );
    assert!(
        fd_findings(&report).is_empty(),
        "and no merge.functional-dependency finding is emitted: {:?}",
        fd_findings(&report)
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 2 — THE DECLARED SET REACHES THE MERGE CHECK (acceptance criterion b)
// ════════════════════════════════════════════════════════════════════════════

/// `crest` is declared functional in the base and held by **no** base fact, so
/// `inferred_functional` over the base certain layer cannot see it (asserted in
/// `open_declared_base`). Two branches then add different certain values for the
/// same subject. The merge must still raise the break — and it does, through the
/// recompile's `dli_world::check::run` battery, whose `continuity.fd-violation`
/// owns author-declared functional dependencies over the merged world's source
/// layer (fluent- and subject-kind-aware). The coarser derived-layer
/// `certain_layer_conflicts` deliberately stays inferred-only, so declared breaks
/// are reported once, by the lens that shares the load check's exclusions (DL-0075,
/// issue 59 red-team F2/F3/F5). "Raised on the merge" is what acceptance (b) asks;
/// it is raised as an error-severity finding, not a `Certain` conflict.
#[test]
fn merge_honors_declared_functional_on_a_first_seen_predicate() {
    let (store, _dir) = open_declared_base();
    commit_facts(
        &store,
        "a",
        &[certain_fact("a-crest", DECLARED, &["mara", "gold"])],
    );
    commit_facts(
        &store,
        "b",
        &[certain_fact("b-crest", DECLARED, &["mara", "silver"])],
    );

    let report = store.merge("a", "b").expect("merge");

    let findings = continuity_fd_findings(&report);
    assert_eq!(
        findings.len(),
        1,
        "the declared break is raised on the merge as a continuity.fd-violation: {:?}",
        report.findings.iter().map(|f| &f.code).collect::<Vec<_>>()
    );
    let message = findings[0];
    assert!(
        message.contains(DECLARED) && message.contains("mara"),
        "names the predicate and subject: {message}"
    );
    assert!(
        message.contains("gold") && message.contains("silver"),
        "carries both clashing values: {message}"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 3 — THE TWO HALVES COMPOSE
// ════════════════════════════════════════════════════════════════════════════

/// Distinctness keys on `fd_value_key` regardless of *how* functionality was
/// established: the same declared-functional `crest`, whose branch values differ
/// only cosmetically, is one value and breaks nothing. This is the pair of the
/// probe above — same base, same predicate, same declared set; only the values'
/// spelling changes, and that alone must decide the verdict.
///
/// Honest grading: unlike the two probes above this one does **not** discriminate
/// the pre-fix code, which passed it vacuously (the declared set never reached the
/// check, so `crest` raised nothing whatever its values). Its teeth face forward,
/// at the half-fix a future edit could land — unioning the declared set while
/// keying distinctness on the raw string, which raises a spurious break here.
#[test]
fn merge_declared_functional_still_folds_cosmetic_drift() {
    let (store, _dir) = open_declared_base();
    commit_facts(
        &store,
        "a",
        &[certain_fact("a-crest", DECLARED, &["mara", "gold"])],
    );
    commit_facts(
        &store,
        "b",
        &[certain_fact("b-crest", DECLARED, &["mara", "Gold"])],
    );

    let report = store.merge("a", "b").expect("merge");

    let atoms = rendered_atoms(&store, &report.merge_commit);
    assert!(
        atoms.iter().any(|a| a.contains("gold")) && atoms.iter().any(|a| a.contains("Gold")),
        "both spellings survive as separate atoms: {atoms:?}"
    );
    assert!(
        certain_conflicts(&report).is_empty(),
        "a declared-functional slot still folds cosmetic drift: {:?}",
        certain_conflicts(&report)
    );
    assert!(
        fd_findings(&report).is_empty(),
        "and emits no merge.functional-dependency finding: {:?}",
        fd_findings(&report)
    );
    assert!(
        continuity_fd_findings(&report).is_empty(),
        "and the declared-owning load lens also folds it: {:?}",
        continuity_fd_findings(&report)
    );
}
