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
        &[certain_fact("base-home", "home", &["lyra", "shallows"])],
    );
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    commit_facts(
        &store,
        "b",
        &[certain_fact("b-home", "home", &["lyra", "highlands"])],
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
    assert!(c.rendered.contains("lyra"), "names the subject");
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
        "scope canon\n\nquestion q-keeper \"Who keeps it?\"\n  option opt-fen\n  option opt-lyra\n";
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
    set_branch_valuation(&store, "b", &[("opt-lyra", true)]);

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
        bundle: None,
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
///
/// LS-9 replaced `RuleDecl.body: Vec<Clause>` with the typed `RuleBody`; the
/// parser (`structure_rule_body`) is the single producer of the pre-parsed
/// `Datalog`, so build the interior through it rather than by hand — otherwise
/// `datalog` is `None` and the rule never executes on recompile.
fn rule_decl(name: &str, body_text: &str) -> Decl {
    let name_ident = Ident::synthetic(name);
    let clauses = vec![Clause {
        text: body_text.to_owned(),
        span: Span::synthetic(),
    }];
    let body = dli_syntax::structure_rule_body(&clauses, &name_ident, &[]);
    Decl::Rule(RuleDecl {
        name: name_ident,
        domain: None,
        head: Vec::new(),
        body,
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
            body: dli_ast::language::RatifyBody::default(),
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
            body: dli_ast::language::RatifyBody::default(),
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
        &rule_decl("ra", "alive(lyra) :- not dead(lyra)"),
    );
    commit_decl(
        &store,
        "b",
        &rule_decl("rb", "dead(lyra) :- not alive(lyra)"),
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

// ════════════════════════════════════════════════════════════════════════════
// GH #75 — merge_base is O(M + E), the dry-run stall is gone, and a dry-run
// advances no ref. (DL-0086.)
// ════════════════════════════════════════════════════════════════════════════

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dli_lorestore::Storage;

/// The fast-forward shape the identity-authoring workload hits nearly every
/// merge: the branch is forked from an unmoved `main`, so the target head is an
/// (inclusive) ancestor of the source head. `merge_base` returns that head
/// directly — the ancestor short-circuit — for both argument orders.
#[test]
fn merge_base_short_circuits_when_one_head_is_an_ancestor() {
    let (store, _dir) = open_store();
    seed_main(&store, &[certain_fact("seed", "place", &["camelot"])]);
    let main_head = store.branch_head("main").expect("head").expect("main");
    store.create_branch("id", "main").expect("id");
    commit_facts(&store, "id", &[certain_fact("c1", "note", &["a"])]);
    commit_facts(&store, "id", &[certain_fact("c2", "note", &["b"])]);
    let id_head = store.branch_head("id").expect("head").expect("id");

    assert_eq!(
        store.merge_base(&main_head, &id_head).expect("mb"),
        Some(main_head.clone()),
        "target-as-ancestor: the LCA is the ancestor head itself"
    );
    assert_eq!(
        store.merge_base(&id_head, &main_head).expect("mb"),
        Some(main_head),
        "symmetric in the argument order"
    );
}

/// Genuine divergence — two branches each advance past the base, so **neither
/// head is an ancestor of the other** and the short-circuit provably cannot
/// fire. The result therefore exercises the topological domination pass, whose
/// correctness the near-always-firing short-circuit would otherwise mask
/// (reviewer note 1).
#[test]
fn merge_base_takes_the_topological_pass_on_genuine_divergence() {
    let (store, _dir) = open_store();
    seed_main(&store, &[certain_fact("seed", "place", &["camelot"])]);
    let base_head = store.branch_head("main").expect("head").expect("main");
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    commit_facts(&store, "a", &[certain_fact("a1", "note", &["a1"])]);
    commit_facts(&store, "a", &[certain_fact("a2", "note", &["a2"])]);
    commit_facts(&store, "b", &[certain_fact("b1", "note", &["b1"])]);
    let a_head = store.branch_head("a").expect("head").expect("a");
    let b_head = store.branch_head("b").expect("head").expect("b");

    // Precondition: the short-circuit cannot apply — neither head reaches the
    // other — so the topological pass is what computes the answer below.
    assert!(!store.ancestors(&a_head).expect("anc").contains(&b_head));
    assert!(!store.ancestors(&b_head).expect("anc").contains(&a_head));

    assert_eq!(
        store.merge_base(&a_head, &b_head).expect("mb"),
        Some(base_head),
        "the divergence LCA is the shared branch point"
    );
}

/// Criss-cross: two independent merges of the same two branch tips leave **two**
/// lowest common ancestors. `merge_base` must pick the smallest hash
/// deterministically (the documented tie-break), unchanged from the prior
/// formulation.
#[test]
fn merge_base_criss_cross_returns_the_smallest_hash() {
    let (store, _dir) = open_store();
    seed_main(&store, &[certain_fact("seed", "place", &["camelot"])]);
    store.create_branch("p", "main").expect("p");
    store.create_branch("q", "main").expect("q");
    commit_facts(&store, "p", &[certain_fact("p1", "note", &["alpha"])]);
    commit_facts(&store, "q", &[certain_fact("q1", "note", &["beta"])]);
    let p1 = store.branch_head("p").expect("head").expect("p");
    let q1 = store.branch_head("q").expect("head").expect("q");
    assert_ne!(p1, q1);

    // Snapshot p at P1, then merge the two tips in *opposite directions*:
    // M1 = merge(p<-q) has parent P1 / merge_parent Q1; M2 = merge(q<-p2) has
    // parent Q1 / merge_parent P1. The swapped parent order makes the two merge
    // commits distinct objects (a same-direction pair would be byte-identical
    // under content addressing), while both still reach exactly {P1, Q1}.
    store.create_branch("p2", "p").expect("p2");
    store.merge("p", "q").expect("p<-q");
    store.merge("q", "p2").expect("q<-p2");
    let m1 = store.branch_head("p").expect("head").expect("p");
    let m2 = store.branch_head("q").expect("head").expect("q");
    assert_ne!(m1, m2, "the two merge commits are distinct");

    let base = store.merge_base(&m1, &m2).expect("mb");
    assert!(
        base == Some(p1.clone()) || base == Some(q1.clone()),
        "an LCA is one of the two divergence tips: {base:?}"
    );
    assert_eq!(
        base,
        Some(std::cmp::min(p1, q1)),
        "with two LCAs the smallest hash is the deterministic pick"
    );
}

/// The GH #75 regression: on a deep, incrementally grown history, a dry-run merge of a freshly forked
/// identity branch must still answer. The stall was in `merge_base`, whose cost
/// grew with the *shared history* (|common|), so the guard grows that history
/// deep with cheap linear commits and threads in real identity branch-merges so
/// merge commits sit in the ancestry too. Under the prior O(M³) `merge_base` a
/// late preview over this DAG stalled past the client's 120s deadline; here it
/// returns within the ordinary harness. No wall-clock assertion — "the dry-run
/// returns at all" is the deterministic signal, and an O(M²⁺) regression would
/// blow the CI timeout instead of passing.
///
/// (The scan's own full-world recompiles are left O(world) by design — the
/// durable cure is incremental compilation, roadmap §6 Phase 3 — so this guard
/// keeps the *number of full merges* small and the linear spine large, isolating
/// the `merge_base` fix rather than the recompile slope.)
#[test]
fn merge_dry_run_stays_fast_on_a_deep_identity_history() {
    // A deep shared spine (cheap: linear commits, no merges → no recompiles) is
    // what stresses merge_base's |common|.
    const DEPTH: usize = 200;

    let (store, _dir) = open_store();
    seed_main(&store, &[certain_fact("seed", "place", &["camelot"])]);

    for i in 0..DEPTH {
        commit_facts(
            &store,
            "main",
            &[certain_fact(&format!("m{i}"), "note", &[&format!("v{i}")])],
        );
    }

    // A few real identity branch-merges, so merge commits also sit in the
    // ancestry (kept small — each real merge pays the O(world) recompile).
    for i in 0..3 {
        let branch = format!("id-{i}");
        store
            .create_branch(&branch, "main")
            .expect("fork identity branch");
        commit_facts(
            &store,
            &branch,
            &[certain_fact(&format!("c{i}"), "note", &[&format!("e{i}")])],
        );
        let preview = store
            .merge_with("main", &branch, false, None)
            .expect("dry-run preview must answer");
        assert!(preview.stratifies, "round {i} preview stratifies");
        store.merge("main", &branch).expect("accept: real merge");
    }

    // The #75 call: a dry-run preview over the fully-grown, deep DAG.
    store.create_branch("id-final", "main").expect("final fork");
    commit_facts(&store, "id-final", &[certain_fact("cf", "note", &["ef"])]);
    let preview = store
        .merge_with("main", "id-final", false, None)
        .expect("final preview must answer");
    assert!(preview.stratifies);
    // Fork-off-unmoved-main means base == target head, so the scan memoization
    // collapses a recompile — reviewer note 3's measurable signal.
    assert!(
        preview.recompiles_saved >= 1,
        "the coincident-head memoization fires on the fast-forward shape"
    );
}

// ── Q4: a dry-run advances no ref, even though it writes the merge object ─────

#[derive(Default)]
struct RefCounters {
    writes: AtomicUsize,
    cas: AtomicUsize,
}

/// A `Storage` that tallies object writes and ref CASes and delegates everything
/// to a real filesystem backend, so a test can prove exactly what a dry-run
/// touches on disk.
struct CountingStorage {
    inner: dli_lorestore::FilesystemStorage,
    counters: Arc<RefCounters>,
}

impl std::fmt::Debug for CountingStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CountingStorage").finish()
    }
}

impl Storage for CountingStorage {
    fn read_object(
        &self,
        hash: &ContentHash,
    ) -> Result<Option<Vec<u8>>, dli_lorestore::StoreError> {
        self.inner.read_object(hash)
    }
    fn write_object(&self, bytes: &[u8]) -> Result<ContentHash, dli_lorestore::StoreError> {
        self.counters.writes.fetch_add(1, Ordering::SeqCst);
        self.inner.write_object(bytes)
    }
    fn cas_ref(
        &self,
        name: &str,
        expected: Option<&str>,
        new: &dli_lorestore::Ref,
    ) -> Result<bool, dli_lorestore::StoreError> {
        self.counters.cas.fetch_add(1, Ordering::SeqCst);
        self.inner.cas_ref(name, expected, new)
    }
    fn load_ref(
        &self,
        name: &str,
    ) -> Result<Option<dli_lorestore::Ref>, dli_lorestore::StoreError> {
        self.inner.load_ref(name)
    }
    fn list_refs(&self) -> Result<Vec<String>, dli_lorestore::StoreError> {
        self.inner.list_refs()
    }
    fn delete_ref(&self, name: &str) -> Result<(), dli_lorestore::StoreError> {
        self.inner.delete_ref(name)
    }
    fn acquire_lock(&self) -> Result<std::fs::File, dli_lorestore::StoreError> {
        self.inner.acquire_lock()
    }
    fn walk_dag(
        &self,
        tip: &ContentHash,
        base: Option<&ContentHash>,
    ) -> Result<Vec<dli_lorestore::Commit>, dli_lorestore::StoreError> {
        self.inner.walk_dag(tip, base)
    }
    fn cleanup_orphans(&self) -> Result<usize, dli_lorestore::StoreError> {
        self.inner.cleanup_orphans()
    }
}

fn open_counting_store() -> (Lorestore, TempDir, Arc<RefCounters>) {
    let dir = TempDir::new().expect("tempdir");
    // Bootstrap the genesis commit without the wrapper, then re-open over it.
    let _ = Lorestore::open(dir.path().to_path_buf()).expect("bootstrap genesis");
    let counters = Arc::new(RefCounters::default());
    let inner = dli_lorestore::FilesystemStorage::new(dir.path().to_path_buf());
    let store = Lorestore::from_storage(Box::new(CountingStorage {
        inner,
        counters: Arc::clone(&counters),
    }));
    (store, dir, counters)
}

/// Q4: a `dry_run` merge writes the (content-addressed, unreferenced) merge
/// commit object but performs **no** ref-advancing CAS — so a worker killed
/// mid-scan leaves every ref byte-identical. A conflict-surfacing corpus proves
/// the scan does real work before the guarantee is checked. The following real
/// merge moves the target ref exactly once.
#[test]
fn dry_run_merge_writes_an_orphan_but_advances_no_ref() {
    let (store, _dir, counters) = open_counting_store();

    // An exclusivity corpus: one question, two options, ruled differently on the
    // two branches, so the dry-run's scan surfaces a real conflict.
    let source =
        "scope canon\n\nquestion q-keeper \"Who keeps it?\"\n  option opt-fen\n  option opt-lyra\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "base parses clean: {diagnostics:?}");
    let pack = dli_world::Pack {
        name: "canon".to_owned(),
        documents: vec![document],
    };
    store.import_pack(&pack).expect("import base with question");
    store.create_branch("a", "main").expect("a");
    store.create_branch("b", "main").expect("b");
    set_branch_valuation(&store, "a", &[("opt-fen", true)]);
    set_branch_valuation(&store, "b", &[("opt-lyra", true)]);

    let target_before = store
        .branch_ref("a")
        .expect("ref")
        .expect("branch a")
        .digest()
        .expect("digest");
    let writes_before = counters.writes.load(Ordering::SeqCst);
    let cas_before = counters.cas.load(Ordering::SeqCst);

    let report = store
        .merge_with("a", "b", false, None)
        .expect("dry-run preview");
    assert!(
        !report.conflicts.is_empty(),
        "the dry-run scan surfaced the exclusivity conflict"
    );
    assert_eq!(
        counters.cas.load(Ordering::SeqCst),
        cas_before,
        "a dry-run performs no ref-advancing CAS"
    );
    assert!(
        counters.writes.load(Ordering::SeqCst) > writes_before,
        "a dry-run still writes the unreferenced merge-commit object"
    );
    assert_eq!(
        store
            .branch_ref("a")
            .expect("ref")
            .expect("branch a")
            .digest()
            .expect("digest"),
        target_before,
        "the target ref is byte-identical after a dry-run"
    );

    // The accept decision — a real merge — moves the target ref exactly once.
    let cas_before_real = counters.cas.load(Ordering::SeqCst);
    store.merge("a", "b").expect("real merge");
    assert_eq!(
        counters.cas.load(Ordering::SeqCst),
        cas_before_real + 1,
        "the real merge CASes the target exactly once"
    );
    assert_ne!(
        store
            .branch_ref("a")
            .expect("ref")
            .expect("branch a")
            .digest()
            .expect("digest"),
        target_before,
        "the real merge moved the target ref"
    );
}

// ── Differential guard: the O(M+E) merge_base matches the O(M³) definition ────

/// A deterministic index in `0..len` drawn from the LCG, `as`-cast-free (the
/// crate denies truncating casts).
fn pick_index(rng: &mut Lcg, len: usize) -> usize {
    let n = u64::try_from(len).expect("len fits u64");
    usize::try_from(rng.next() % n).expect("index fits usize")
}

/// The reference lowest common ancestor — the pre-fix definition, computed with
/// `ancestors()` only: `common = anc(a) ∩ anc(b)`; the lowest common ancestors
/// are the common nodes that no *other* common node has as a proper ancestor;
/// the smallest hash wins (`None` when the histories are disjoint). This is the
/// exact behaviour the O(M+E) rewrite must reproduce.
fn brute_force_merge_base(
    store: &Lorestore,
    a: &ContentHash,
    b: &ContentHash,
) -> Option<ContentHash> {
    let anc_a = store.ancestors(a).expect("anc a");
    let anc_b = store.ancestors(b).expect("anc b");
    let common: Vec<ContentHash> = anc_a.intersection(&anc_b).cloned().collect();
    let mut lowest: Vec<ContentHash> = common
        .iter()
        .filter(|c| {
            !common
                .iter()
                .any(|c2| c2 != *c && store.ancestors(c2).expect("anc c2").contains(c))
        })
        .cloned()
        .collect();
    lowest.sort();
    lowest.into_iter().next()
}

/// Differential property test: over many randomized commit DAGs — linear
/// commits, forks, and merges in both parent orders — `Lorestore::merge_base`
/// must agree with the brute-force reference on every pair of heads, be
/// symmetric, and return a node in `anc(a) ∩ anc(b)`. The coverage counters
/// assert the corpus actually built merge commits and actually exercised the
/// topological pass (pairs where neither head is an ancestor of the other, so
/// the short-circuit provably cannot fire): a differential test that quietly
/// stopped building merges — or only hit the short-circuit — would false-pass,
/// so this guard refuses to be vacuous.
#[test]
fn merge_base_matches_the_reference_over_random_dags() {
    let mut merges_built = 0usize;
    let mut pairs_checked = 0usize;
    let mut topo_pass_pairs = 0usize;

    for seed in 0..8u64 {
        let (store, _dir) = open_store();
        seed_main(&store, &[certain_fact("seed", "note", &["root"])]);
        let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1));
        let mut branches: Vec<String> = vec!["main".to_owned()];
        let mut fresh = 0usize;

        for _ in 0..18 {
            // Fork (action 1) is always possible — even off `main` alone; only a
            // merge (action 2) needs two branches, else it falls back to a commit.
            let action = rng.next() % 3;
            if action == 1 {
                let idx = pick_index(&mut rng, branches.len());
                let src = branches[idx].clone();
                let name = format!("b{}-{}", seed, branches.len());
                store.create_branch(&name, &src).expect("create_branch");
                branches.push(name);
            } else if action == 2 && branches.len() >= 2 {
                let t = pick_index(&mut rng, branches.len());
                let s = pick_index(&mut rng, branches.len());
                if t != s {
                    // Interners are identical (the facts add no event variables),
                    // so a merge always reconciles.
                    store.merge(&branches[t], &branches[s]).expect("merge");
                    merges_built += 1;
                }
            } else {
                let idx = pick_index(&mut rng, branches.len());
                let b = branches[idx].clone();
                fresh += 1;
                commit_facts(
                    &store,
                    &b,
                    &[certain_fact(
                        &format!("c{fresh}"),
                        "note",
                        &[&format!("v{fresh}")],
                    )],
                );
            }
        }

        let heads: Vec<ContentHash> = branches
            .iter()
            .map(|b| store.branch_head(b).expect("head").expect("branch"))
            .collect();
        for i in 0..heads.len() {
            for j in i..heads.len() {
                let (a, b) = (&heads[i], &heads[j]);
                let got = store.merge_base(a, b).expect("merge_base");
                assert_eq!(
                    got,
                    brute_force_merge_base(&store, a, b),
                    "seed {seed}: merge_base disagrees with the O(M³) reference"
                );
                assert_eq!(
                    store.merge_base(b, a).expect("merge_base"),
                    got,
                    "seed {seed}: merge_base is not symmetric"
                );
                let anc_a = store.ancestors(a).expect("anc");
                let anc_b = store.ancestors(b).expect("anc");
                if let Some(m) = &got {
                    assert!(
                        anc_a.contains(m) && anc_b.contains(m),
                        "the LCA must be a common ancestor"
                    );
                }
                if !anc_a.contains(b) && !anc_b.contains(a) {
                    topo_pass_pairs += 1;
                }
                pairs_checked += 1;
            }
        }
    }

    // The guard must not be vacuous: the corpus built merges and exercised the
    // domination pass, not only the ancestor short-circuit.
    assert!(
        merges_built >= 10,
        "the corpus must build merge commits (built {merges_built})"
    );
    assert!(
        topo_pass_pairs > 0,
        "some pair must take the topological pass, not the short-circuit (got {topo_pass_pairs})"
    );
    assert!(
        pairs_checked > 150,
        "sanity: enough pairs checked (checked {pairs_checked})"
    );
}
