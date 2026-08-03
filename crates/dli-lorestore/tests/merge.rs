//! LS-6 merge-engine oracles.
//!
//! Two families of falsifiable property, per the roadmap §LS-6 oracle:
//!
//! 1. **Confluence & associativity** on conflict-free corpora: `merge(A, B)` and
//!    `merge(B, A)` recompile to root-hash-equal merged worlds, and
//!    `merge(merge(A, B), C) == merge(A, merge(B, C))` up to that equality. The
//!    grow-only fact set makes merge a join-semilattice least-upper-bound
//!    (CALM), so order cannot matter.
//! 2. **Seeded-conflict corpus** (one seed per class): valuation disagreement,
//!    a functional-dependency / certain-layer violation, an exclusivity-family
//!    violation, and a stratified-negation standing flip — each proved *caught*,
//!    *named* (with its `ConflictKind` where it has one), and carrying repair
//!    data; and a conflict-free merge proved to produce an **empty** conflict
//!    report.
//!
//! Facts cite a single ruled decision `D-1` so they compile *certain* and their
//! support reconstructs order-independently (a bare-proposed fact re-mints an
//! order-dependent `h.`-bundle gate, which would make even a confluent merge's
//! recompiled bytes depend on merge order). All randomness is a deterministic
//! LCG (the `state_fold_invariant` idiom) — no clock, no OS entropy.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

mod common;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{Decl, Document, FactDecl, RatifyDecl, RuleDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::atom::Atom;
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;
use dli_eval::fork::ConflictKind;
use dli_lorestore::{ContentHash, FactObject, Lorestore};
use tempfile::TempDir;

// ── deterministic RNG (state_fold_invariant idiom) ──────────────────────────

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

/// The one ruled decision every fact cites so it compiles certain.
const DECISION: &str = "D-1";

// ── store / branch helpers ──────────────────────────────────────────────────

fn open_store() -> (Lorestore, TempDir) {
    let dir = TempDir::new().expect("tempdir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open lorestore");
    (store, dir)
}

/// A fact object citing `D-1` (support `{+D-1}`), so it compiles certain and its
/// support reconstructs to a stable `@ratified D-1` — order-independent.
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

/// Overwrite a branch's valuation overlay, fencing on its current ref digest.
fn set_branch_valuation(store: &Lorestore, branch: &str, rulings: &[(&str, bool)]) {
    let head = store
        .branch_head(branch)
        .expect("branch head")
        .expect("branch exists");
    let (interner, _) = branch_state(store, branch);
    let mut valuation = Valuation::new();
    for (name, value) in rulings {
        let idx = interner
            .iter()
            .position(|n| n == name)
            .unwrap_or_else(|| panic!("event variable `{name}` not in interner {interner:?}"));
        valuation.set(Atom(u32::try_from(idx).unwrap()), *value);
    }
    let old = store
        .branch_ref(branch)
        .expect("branch ref")
        .expect("branch exists");
    let token = old.digest().expect("digest").0;
    let new =
        dli_lorestore::Ref::with_valuation(dli_lorestore::branch_ref_name(branch), head, valuation);
    let ok = store
        .cas_branch_ref(branch, Some(&token), &new)
        .expect("cas branch ref");
    assert!(ok, "branch valuation CAS must land");
}

/// A canonical, interner-order-independent signature of the world compiled at a
/// commit: the name-resolved derived relation, sorted. Equal signatures mean
/// root-hash-equal merged worlds (the confluence oracle's comparison).
fn world_signature(store: &Lorestore, head: &ContentHash) -> Vec<String> {
    let compiled = store.compile_at(head).expect("compile merged world");
    common::render_derived(&compiled.model().world)
}

// ════════════════════════════════════════════════════════════════════════════
// 0 — THE SIGNATURE HAS TEETH IN THE DEFEATER CHANNEL
// ════════════════════════════════════════════════════════════════════════════

/// The confluence signature must distinguish two worlds that differ **only** in
/// the defeater channel (`P⁻`) — otherwise a non-monotone (negation) divergence
/// between `merge(A,B)` and `merge(B,A)` could false-green. Both worlds share the
/// same atom, support, and grade; only the defeater variable differs (`d-y` vs
/// `d-z`). The support+grade-only signature would render them identical; the
/// defeater-inclusive signature (red-team item 1) must not.
#[test]
fn signature_distinguishes_a_defeater_only_difference() {
    use dli_core::ann::Ann;
    use dli_eval::world::{GroundAtom, World};

    fn world_with_defeater(defeater_name: &str) -> World {
        let mut w = World::new();
        let support = w.interner.intern("d-x");
        let defeater = w.interner.intern(defeater_name);
        w.assert_fact(
            GroundAtom {
                pred: "keeps".to_owned(),
                args: vec!["fen".to_owned()],
            },
            Ann::new(PosBool::pos(support), PosBool::pos(defeater)),
        );
        w.fixpoint().expect("stratifies")
    }

    let a = common::render_derived(&world_with_defeater("d-y"));
    let b = common::render_derived(&world_with_defeater("d-z"));

    // The support and grade are identical; a support-only signature would tie.
    assert!(
        a.iter().all(|row| row.contains("sup={+d-x}")),
        "support is the shared part: {a:?}"
    );
    assert_ne!(
        a, b,
        "a defeater-only difference must change the signature (item 1 teeth)"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 1 — CONFLUENCE & ASSOCIATIVITY
// ════════════════════════════════════════════════════════════════════════════

/// merge(A, B) and merge(B, A) recompile to the identical merged world.
#[test]
fn merge_is_commutative_up_to_root_hash() {
    for seed in [1_u64, 7, 42, 1999] {
        let mut rng = Lcg(seed);
        let a_facts: Vec<FactObject> = (0..2)
            .map(|i| {
                certain_fact(
                    &format!("s{seed}/a/{i}"),
                    "kept",
                    &[&format!("a-{seed}-{i}")],
                )
            })
            .collect();
        let b_facts: Vec<FactObject> = (0..2)
            .map(|i| {
                certain_fact(
                    &format!("s{seed}/b/{i}"),
                    "kept",
                    &[&format!("b-{seed}-{i}")],
                )
            })
            .collect();
        let base_count = 1 + (rng.next() % 2) as usize;
        let base: Vec<FactObject> = (0..base_count)
            .map(|i| {
                certain_fact(
                    &format!("s{seed}/base/{i}"),
                    "place",
                    &[&format!("p-{seed}-{i}")],
                )
            })
            .collect();

        // Store 1: merge B into A.
        let (s1, _d1) = open_store();
        seed_main(&s1, &base);
        s1.create_branch("a", "main").expect("branch a");
        s1.create_branch("b", "main").expect("branch b");
        commit_facts(&s1, "a", &a_facts);
        commit_facts(&s1, "b", &b_facts);
        let r_ab = s1.merge("a", "b").expect("merge a<-b");
        assert!(
            r_ab.conflicts.is_empty(),
            "conflict-free merge, seed {seed}"
        );
        let sig_ab = world_signature(&s1, &r_ab.merge_commit);

        // Store 2: merge A into B (opposite order, same contents).
        let (s2, _d2) = open_store();
        seed_main(&s2, &base);
        s2.create_branch("a", "main").expect("branch a");
        s2.create_branch("b", "main").expect("branch b");
        commit_facts(&s2, "a", &a_facts);
        commit_facts(&s2, "b", &b_facts);
        let r_ba = s2.merge("b", "a").expect("merge b<-a");
        let sig_ba = world_signature(&s2, &r_ba.merge_commit);

        assert_eq!(
            sig_ab, sig_ba,
            "seed {seed}: merge(A,B) and merge(B,A) must recompile equal worlds"
        );
        assert!(
            sig_ab.iter().any(|r| r.contains("a-")) && sig_ab.iter().any(|r| r.contains("b-")),
            "the union carries both branches' facts"
        );
    }
}

/// merge(merge(A, B), C) and merge(A, merge(B, C)) recompile equal.
#[test]
fn merge_is_associative_up_to_root_hash() {
    let a = [
        certain_fact("a0", "kept", &["a-0"]),
        certain_fact("a1", "kept", &["a-1"]),
    ];
    let b = [certain_fact("b0", "kept", &["b-0"])];
    let c = [
        certain_fact("c0", "kept", &["c-0"]),
        certain_fact("c1", "kept", &["c-1"]),
    ];
    let base = [certain_fact("base0", "place", &["camelot"])];

    // Left-associated: (A ← B) ← C.
    let (s1, _d1) = open_store();
    seed_main(&s1, &base);
    for br in ["a", "b", "c"] {
        s1.create_branch(br, "main").expect("branch");
    }
    commit_facts(&s1, "a", &a);
    commit_facts(&s1, "b", &b);
    commit_facts(&s1, "c", &c);
    s1.merge("a", "b").expect("a<-b");
    let left = s1.merge("a", "c").expect("a<-c");
    let sig_left = world_signature(&s1, &left.merge_commit);

    // Right-associated: A ← (B ← C).
    let (s2, _d2) = open_store();
    seed_main(&s2, &base);
    for br in ["a", "b", "c"] {
        s2.create_branch(br, "main").expect("branch");
    }
    commit_facts(&s2, "a", &a);
    commit_facts(&s2, "b", &b);
    commit_facts(&s2, "c", &c);
    s2.merge("b", "c").expect("b<-c");
    let right = s2.merge("a", "b").expect("a<-b");
    let sig_right = world_signature(&s2, &right.merge_commit);

    assert_eq!(
        sig_left, sig_right,
        "merge associativity must hold up to root-hash equality"
    );
}

/// A conflict-free merge produces an empty conflict report; the merge commit is
/// a genuine two-parent commit naming the union.
#[test]
fn conflict_free_merge_has_empty_report() {
    let (store, _dir) = open_store();
    seed_main(&store, &[certain_fact("base", "keeps", &["camelot"])]);
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    commit_facts(&store, "a", &[certain_fact("a", "kept", &["alpha"])]);
    commit_facts(&store, "b", &[certain_fact("b", "kept", &["beta"])]);

    let report = store.merge("a", "b").expect("merge");
    assert!(report.conflicts.is_empty(), "no conflicts");
    assert!(report.stratifies, "merged world stratifies");
    assert!(
        report.standing_changes.is_empty(),
        "monotone growth introduces no downgrades"
    );
    let commit: dli_lorestore::Commit = store
        .read_object(&report.merge_commit)
        .expect("read")
        .expect("commit");
    assert!(
        commit.merge_parent.is_some(),
        "merge commit has two parents"
    );
    let signature = world_signature(&store, &report.merge_commit);
    assert!(signature.iter().any(|r| r.contains("alpha")));
    assert!(signature.iter().any(|r| r.contains("beta")));
    assert!(signature.iter().any(|r| r.contains("camelot")));
}

// ════════════════════════════════════════════════════════════════════════════
// 2 — SEEDED-CONFLICT CORPUS (one seed per class)
// ════════════════════════════════════════════════════════════════════════════

/// Class 1 — valuation disagreement. Two branches rule the same event variable
/// differently; caught as `ConflictKind::Valuation`, never auto-resolved.
#[test]
fn seed_valuation_disagreement() {
    let (store, _dir) = open_store();
    seed_main(&store, &[certain_fact("base", "keeps", &["fen"])]);
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    set_branch_valuation(&store, "a", &[(DECISION, true)]);
    set_branch_valuation(&store, "b", &[(DECISION, false)]);

    let report = store.merge("a", "b").expect("merge");
    let valuation: Vec<_> = report
        .conflicts
        .iter()
        .filter(|c| c.kind == ConflictKind::Valuation)
        .collect();
    assert_eq!(
        valuation.len(),
        1,
        "one valuation disagreement: {:?}",
        report.conflicts
    );
    let c = valuation[0];
    assert!(c.rendered.contains(DECISION), "names the variable");
    assert!(
        c.blame.0.contains("true") && c.blame.1.contains("false"),
        "repair data carries both rulings: {:?}",
        c.blame
    );
}

/// Class 2a — functional-dependency / certain-layer violation. A predicate the
/// base holds functional carries two distinct certain values for one subject
/// after the merge; caught as `ConflictKind::Certain` with a graded finding.
#[test]
fn seed_functional_dependency_violation() {
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
        &[certain_fact("b-home", "home", &["mara", "highlands"])],
    );

    let report = store.merge("a", "b").expect("merge");
    let certain: Vec<_> = report
        .conflicts
        .iter()
        .filter(|c| c.kind == ConflictKind::Certain)
        .collect();
    assert_eq!(certain.len(), 1, "one FD conflict: {:?}", report.conflicts);
    let c = certain[0];
    assert!(c.rendered.contains("home"), "names the predicate");
    assert!(c.rendered.contains("mara"), "names the subject");
    let blame = format!("{} {}", c.blame.0, c.blame.1);
    assert!(
        blame.contains("shallows") && blame.contains("highlands"),
        "repair data carries both clashing values: {:?}",
        c.blame
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "merge.functional-dependency"),
        "FD violation is a graded finding: {:?}",
        report.findings.iter().map(|f| &f.code).collect::<Vec<_>>()
    );
}

/// Class 2b — exclusivity-family violation. Two options of one question are
/// ruled true across the two branches; the merged valuation violates the family.
#[test]
fn seed_exclusivity_violation() {
    let source =
        "scope canon\n\nquestion q-keeper \"Who keeps it?\"\n  option opt-fen\n  option opt-mara\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "base parses clean: {diagnostics:?}");
    let pack = dli_world::Pack {
        name: "canon".to_owned(),
        documents: vec![document],
    };
    let (store, _dir) = open_store();
    store.import_pack(&pack).expect("import base with question");
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    set_branch_valuation(&store, "a", &[("opt-fen", true)]);
    set_branch_valuation(&store, "b", &[("opt-mara", true)]);

    let report = store.merge("a", "b").expect("merge");
    let exclusivity: Vec<_> = report
        .conflicts
        .iter()
        .filter(|c| c.kind == ConflictKind::Exclusivity)
        .collect();
    assert_eq!(
        exclusivity.len(),
        1,
        "one exclusivity conflict: {:?}",
        report.conflicts
    );
    let c = exclusivity[0];
    assert!(
        c.blame.0.contains("opt-") && c.blame.1.contains("opt-"),
        "repair data names both options: {:?}",
        c.blame
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "merge.exclusivity"),
        "exclusivity is a graded finding"
    );
}

/// Class 3 — stratified-negation standing flip. The base has a negation rule
/// (`ward-holds :- seated, not breached`); branch B introduces `breached`, which
/// un-derives `ward-holds` that was certain on branch A. The merge names the
/// atom whose standing changed vs. the pre-merge branch.
#[test]
fn seed_stratified_negation_flip() {
    let (store, _dir) = open_store();
    store
        .import_pack(&ward_base_pack())
        .expect("import ward base");
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    // Branch B breaches the oak — the override the rule negates over.
    commit_facts(
        &store,
        "b",
        &[certain_fact("breached", "breached", &["oak"])],
    );

    let report = store.merge("a", "b").expect("merge");
    let flip = report
        .standing_changes
        .iter()
        .find(|s| s.atom.contains("ward-holds(oak)"))
        .unwrap_or_else(|| {
            panic!(
                "the stratified-negation flip must name ward-holds(oak): {:?}",
                report.standing_changes
            )
        });
    assert_eq!(flip.before, "certain", "certain on branch A");
    assert_ne!(flip.after, "certain", "no longer certain after the merge");
    assert!(report.stratifies, "the merged rule set still stratifies");
}

/// A `@ratified D-1` provenance annotation.
fn ratified() -> Annotation {
    Annotation {
        marker: ProvenanceMarker::Ratified,
        citations: vec![CitationRef {
            id: Ident::synthetic(DECISION),
        }],
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

/// A `@ratified D-1` fact declaration `pred(arg)`.
fn fact_decl(pred: &str, arg: &str) -> Decl {
    Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: Ident::synthetic(pred),
            args: vec![Term::Sym(Ident::synthetic(arg))],
        },
        triple: false,
        provenance: Some(ratified()),
    })
}

/// A `@ratified D-1` rule declaration from its clause text.
fn rule_decl(name: &str, body_text: &str) -> Decl {
    Decl::Rule(RuleDecl {
        name: Ident::synthetic(name),
        domain: None,
        head: Vec::new(),
        body: vec![Clause {
            text: body_text.to_owned(),
            span: Span::synthetic(),
        }],
        provenance: Some(ratified()),
    })
}

fn pack_of(decls: Vec<Decl>) -> dli_world::Pack {
    dli_world::Pack {
        name: "canon".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: Ident::synthetic("canon"),
                span: Span::synthetic(),
            }),
            decls,
        }],
    }
}

/// A base pack with `seated(oak)` and the negation rule that derives
/// `ward-holds(oak)` while `oak` is un-breached.
fn ward_base_pack() -> dli_world::Pack {
    pack_of(vec![
        Decl::Ratify(RatifyDecl {
            decision: Ident::synthetic(DECISION),
            body: Vec::new(),
        }),
        fact_decl("seated", "oak"),
        rule_decl("ward", "ward-holds(oak) :- seated(oak), not breached(oak)"),
    ])
}

/// A base pack with several `seated` entities and the *variable* negation rule
/// `ward-holds($g) :- seated($g), not breached($g)`. Branches that add `breached`
/// facts touch the negated stratum, so the merged fixpoint is genuinely
/// non-monotone (a fact from one branch un-derives the other's `ward-holds`).
fn ward_multi_pack() -> dli_world::Pack {
    pack_of(vec![
        Decl::Ratify(RatifyDecl {
            decision: Ident::synthetic(DECISION),
            body: Vec::new(),
        }),
        fact_decl("seated", "oak"),
        fact_decl("seated", "elm"),
        fact_decl("seated", "ash"),
        rule_decl("ward", "ward-holds($g) :- seated($g), not breached($g)"),
    ])
}

/// Commit an expert-tier declaration (e.g. a rule) to a branch, lowering it to
/// carriage fact objects — the store's own write path (`lower::lower_decl`), the
/// only way to put a rule on a branch without a `.dli` file.
fn commit_decl(store: &Lorestore, branch: &str, decl: &Decl) -> ContentHash {
    let (interner_names, decisions) = branch_state(store, branch);
    let mut interner = dli_core::atom::Interner::from_names(interner_names);
    let mut counter = 0u32;
    let objects = dli_lorestore::lower::lower_decl(decl, "canon", &mut interner, &mut counter);
    let mut hashes = Vec::new();
    for fo in &objects {
        hashes.push(store.write_object(fo).expect("write carriage object"));
    }
    store
        .commit_on_branch(
            branch,
            interner.names().to_vec(),
            hashes,
            Vec::new(),
            Vec::new(),
            decisions,
            "canon".to_owned(),
        )
        .expect("commit decl on branch")
}

// ════════════════════════════════════════════════════════════════════════════
// 2b — CONFLUENCE ON THE NON-MONOTONE (NEGATION) CASE
// ════════════════════════════════════════════════════════════════════════════

/// Confluence must hold even when the merge is genuinely non-monotone: the base
/// carries a negation rule, and each branch breaches a *different* entity, so the
/// union downgrades atoms that were certain on the opposite branch. The
/// defeater-inclusive signature (red-team item 1) must still match across orders.
#[test]
fn merge_is_commutative_on_the_negation_stratum() {
    // Store 1: merge B into A.
    let (s1, _d1) = open_store();
    s1.import_pack(&ward_multi_pack())
        .expect("import ward base");
    s1.create_branch("a", "main").expect("a");
    s1.create_branch("b", "main").expect("b");
    commit_facts(&s1, "a", &[certain_fact("a-br", "breached", &["oak"])]);
    commit_facts(&s1, "b", &[certain_fact("b-br", "breached", &["elm"])]);
    let ab = s1.merge("a", "b").expect("a<-b");
    let sig_ab = world_signature(&s1, &ab.merge_commit);

    // Store 2: merge A into B (opposite order).
    let (s2, _d2) = open_store();
    s2.import_pack(&ward_multi_pack())
        .expect("import ward base");
    s2.create_branch("a", "main").expect("a");
    s2.create_branch("b", "main").expect("b");
    commit_facts(&s2, "a", &[certain_fact("a-br", "breached", &["oak"])]);
    commit_facts(&s2, "b", &[certain_fact("b-br", "breached", &["elm"])]);
    let ba = s2.merge("b", "a").expect("b<-a");
    let sig_ba = world_signature(&s2, &ba.merge_commit);

    assert_eq!(
        sig_ab, sig_ba,
        "non-monotone merge must be confluent (defeater channel included)"
    );
    // The merge genuinely touched the negated stratum: ward-holds atoms appear,
    // and the merge reports non-monotone standing downgrades vs the branches.
    assert!(
        sig_ab.iter().any(|r| r.contains("ward-holds")),
        "the negation-derived predicate is present: {sig_ab:?}"
    );
    assert!(
        !ab.standing_changes.is_empty(),
        "the merge downgraded ward-holds atoms vs the pre-merge branches"
    );
}

/// Associativity on the same non-monotone base: a third branch breaches `ash`.
#[test]
fn merge_is_associative_on_the_negation_stratum() {
    fn build(store: &Lorestore) {
        store
            .import_pack(&ward_multi_pack())
            .expect("import ward base");
        for br in ["a", "b", "c"] {
            store.create_branch(br, "main").expect("branch");
        }
        commit_facts(store, "a", &[certain_fact("a-br", "breached", &["oak"])]);
        commit_facts(store, "b", &[certain_fact("b-br", "breached", &["elm"])]);
        commit_facts(store, "c", &[certain_fact("c-br", "breached", &["ash"])]);
    }

    let (s1, _d1) = open_store();
    build(&s1);
    s1.merge("a", "b").expect("a<-b");
    let left = s1.merge("a", "c").expect("a<-c");
    let sig_left = world_signature(&s1, &left.merge_commit);

    let (s2, _d2) = open_store();
    build(&s2);
    s2.merge("b", "c").expect("b<-c");
    let right = s2.merge("a", "b").expect("a<-b");
    let sig_right = world_signature(&s2, &right.merge_commit);

    assert_eq!(
        sig_left, sig_right,
        "non-monotone merge must associate (defeater channel included)"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 2c — CLASS 3 (EXTREME): AN UNSTRATIFIABLE MERGE
// ════════════════════════════════════════════════════════════════════════════

/// The union of two branches closes a negative-dependency cycle
/// (`alive :- not dead`, `dead :- not alive`), so the merged rule set is
/// unstratifiable. The stratify seam surfaces it: the report flags
/// `stratifies == false`, names the reason, and carries a `merge.unstratifiable`
/// finding plus a conflict. (New coverage of an existing early-return path.)
#[test]
fn merge_that_closes_a_negation_cycle_is_unstratifiable() {
    let (store, _dir) = open_store();
    // A base carrying only the D-1 decision (no facts, no rules yet).
    seed_main(&store, &[]);
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    commit_decl(
        &store,
        "a",
        &rule_decl("ra", "alive(mara) :- not dead(mara)"),
    );
    commit_decl(
        &store,
        "b",
        &rule_decl("rb", "dead(mara) :- not alive(mara)"),
    );

    let report = store
        .merge("a", "b")
        .expect("merge still writes the union commit");
    assert!(
        !report.stratifies,
        "the merged rule set must be unstratifiable: {:?}",
        report.conflicts
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "merge.unstratifiable"),
        "a merge.unstratifiable finding is surfaced: {:?}",
        report.findings.iter().map(|f| &f.code).collect::<Vec<_>>()
    );
    let conflict = report
        .conflicts
        .iter()
        .find(|c| c.rendered.contains("stratif"))
        .unwrap_or_else(|| {
            panic!(
                "a stratification conflict is reported: {:?}",
                report.conflicts
            )
        });
    assert!(
        conflict.rendered.contains("cycle") || conflict.blame.1.contains("cycle"),
        "the conflict names the negative-dependency cycle: {conflict:?}"
    );
    assert!(
        report.standing_changes.is_empty(),
        "an unstratifiable merge yields no standing diff (the world does not compile)"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 3 — INTERNER RECONCILIATION, MERGE BASE, BRANCHES
// ════════════════════════════════════════════════════════════════════════════

/// The merge commit's interner must extend BOTH parents append-only; branches
/// that minted colliding event variables are refused (interner regression).
#[test]
fn merge_refuses_divergent_interners() {
    let (store, _dir) = open_store();
    seed_main(&store, &[certain_fact("base", "place", &["camelot"])]);
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    // Each branch appends a *different* event variable at the same index.
    let ha = store
        .write_object(&certain_fact("fa", "kept", &["x"]))
        .expect("w");
    store
        .commit_on_branch(
            "a",
            vec![DECISION.to_owned(), "D-A".to_owned()],
            vec![ha],
            Vec::new(),
            Vec::new(),
            vec![DECISION.to_owned(), "D-A".to_owned()],
            "canon".to_owned(),
        )
        .expect("commit a");
    let hb = store
        .write_object(&certain_fact("fb", "kept", &["y"]))
        .expect("w");
    store
        .commit_on_branch(
            "b",
            vec![DECISION.to_owned(), "D-B".to_owned()],
            vec![hb],
            Vec::new(),
            Vec::new(),
            vec![DECISION.to_owned(), "D-B".to_owned()],
            "canon".to_owned(),
        )
        .expect("commit b");

    let err = store
        .merge("a", "b")
        .expect_err("divergent interners refused");
    assert!(
        matches!(err, dli_lorestore::StoreError::InternerRegression { .. }),
        "got {err:?}"
    );
}

/// The merge base is the lowest common ancestor over the two-parent DAG.
#[test]
fn merge_base_is_lowest_common_ancestor() {
    let (store, _dir) = open_store();
    seed_main(&store, &[certain_fact("base", "place", &["camelot"])]);
    let base_head = store.branch_head("main").expect("head").expect("main");
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    commit_facts(&store, "a", &[certain_fact("a", "kept", &["alpha"])]);
    commit_facts(&store, "b", &[certain_fact("b", "kept", &["beta"])]);
    let a_head = store.branch_head("a").expect("h").expect("a");
    let b_head = store.branch_head("b").expect("h").expect("b");

    let base = store.merge_base(&a_head, &b_head).expect("merge base");
    assert_eq!(base, Some(base_head), "LCA is the common branch point");
}

/// Branch create / list / head resolution round-trips.
#[test]
fn branch_create_list_head() {
    let (store, _dir) = open_store();
    seed_main(&store, &[certain_fact("base", "place", &["camelot"])]);
    store.create_branch("feature", "main").expect("create");
    assert!(
        matches!(
            store.create_branch("feature", "main"),
            Err(dli_lorestore::StoreError::BranchExists { .. })
        ),
        "re-create refused"
    );
    let branches = store.list_branches().expect("list");
    assert_eq!(branches, vec!["main".to_owned(), "feature".to_owned()]);
    assert_eq!(
        store.branch_head("feature").expect("head"),
        store.branch_head("main").expect("head"),
        "a fresh branch starts at its source head"
    );
}
