//! The session-cache parity oracle: a cached `compile_at` is a full recompute
//! (DL-0089 and DL-0102).
//!
//! [`Lorestore::compile_at`] is memoized by commit hash
//! ([`dli_lorestore::cache`]). The claim this file makes falsifiable is the only
//! one that matters: **for every commit in a randomized DAG, the answer the
//! cache serves is byte-identical to the answer a cold store computes.** Not
//! "equivalent", not "equal on the fields we thought to check" —
//! `CompiledWorld: Eq`, and that is what is asserted, with a rendered signature
//! compared first only so a failure is legible.
//!
//! # Why the assertion needs teeth, and where they are
//!
//! A parity oracle over a cache is the easiest vacuous test there is. An
//! implementation whose cache never hits satisfies every assertion below — it
//! would be comparing a full recompute against itself, green and worthless. So
//! three separate anti-vacuity guards run alongside the parity sweep, and each
//! is itself a failing test if the cache stops working:
//!
//! 1. **A hit floor.** Every DAG is swept twice. The second sweep must be served
//!    entirely from the cache — `world_hits` has to rise by at least one per
//!    distinct head — and the run-wide total has to clear
//!    [`MIN_TOTAL_WORLD_HITS`]. A cache that quietly recomputed everything reads
//!    zero here.
//! 2. **A positive control.** `falsify_cached_world` corrupts one memoized
//!    world; `compile_at` must then return the corrupted answer and *disagree*
//!    with `compile_at_uncached`. That proves the warm path genuinely reads the
//!    cache, so the parity assertion is capable of failing, and it proves the
//!    parity assertion notices the kind of difference a real bug would produce.
//! 3. **Eviction and cache-off sweeps.** The same parity holds at capacity 1
//!    (with evictions proved to have happened) and at capacity 0 (with hits
//!    proved to be zero) — so neither a full cache nor an empty one is the only
//!    configuration that works.
//!
//! # Confluence
//!
//! The same fact set committed in different cross-branch orders and merged must
//! compile to the same world. That is the store's own CALM property (roadmap §4)
//! rather than the cache's, but it is asserted here through the cached path, so
//! a cache that keyed on anything order-dependent — a ref, a sequence number, a
//! branch name — would fail it.
//!
//! All randomness is a deterministic LCG (the `state_fold_invariant` idiom) — no
//! clock, no OS entropy, gate-safe.

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
use dli_lorestore::{ContentHash, FactObject, Lorestore};
use tempfile::TempDir;

/// The one ruled decision every fact cites, so it compiles certain and its
/// support reconstructs order-independently. A bare-`@proposed` fact re-mints an
/// order-dependent `h.<pack>#N` positional gate, which would make even a
/// confluent history's recompiled *bytes* depend on commit order — the caveat
/// `merge::reconcile_interners` documents. The oracles stay in the
/// order-independent regime deliberately.
const DECISION: &str = "D-1";

/// Seeds swept by the randomized-DAG oracles.
const SEEDS: u64 = 24;

/// Script steps per seed. Enough for branches to fork, accumulate, and merge —
/// and small enough that a DAG's distinct commits stay well inside the default
/// cache capacity, which the hit floor depends on.
const STEPS: usize = 14;

/// The run-wide floor on genuine cache hits. Well under what the sweep actually
/// produces (see the assertion's message, which reports the real number), and
/// far above what an implementation that never cached could reach: zero.
const MIN_TOTAL_WORLD_HITS: u64 = 200;

// ── deterministic RNG (the `state_fold_invariant` idiom) ────────────────────

struct Lcg(u64);

impl Lcg {
    fn seeded(seed: u64) -> Self {
        // Spread adjacent seeds apart before the first draw, so seed 0 and seed 1
        // do not script near-identical DAGs.
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5DEE_CE66_D1B1_4A2D)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n as u64)).unwrap()
    }
}

// ── declaration fixtures (shared with the LS-6 merge oracles) ───────────────

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

/// A `@ratified D-1` rule, built through the parser so its `datalog` interior is
/// populated and the rule actually executes on recompile (LS-9).
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

/// The seed pack: three seated entities and the **variable negation rule**
/// `ward-holds($g) :- seated($g), not breached($g)`.
///
/// The negation matters. A cache over a purely monotone world could key on
/// almost anything and still look right; a `breached` fact committed later
/// *un-derives* a `ward-holds` that was certain, so a stale or mis-keyed entry
/// shows up as a standing flip rather than as a missing row.
fn ward_pack() -> dli_world::Pack {
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

// ── store helpers ───────────────────────────────────────────────────────────

fn certain_fact(label: &str, pred: &str, args: &[&str]) -> FactObject {
    common::test_fact(label, pred, args, PosBool::pos(Atom(0)), Grade::Asserted)
}

fn branch_state(store: &Lorestore, branch: &str) -> (Vec<String>, Vec<String>) {
    let head = store.branch_head(branch).unwrap().unwrap();
    let commit: dli_lorestore::Commit = store.read_object(&head).unwrap().unwrap();
    (commit.interner, commit.decisions)
}

fn commit_facts(store: &Lorestore, branch: &str, facts: &[FactObject]) -> ContentHash {
    let mut hashes = Vec::new();
    for fo in facts {
        hashes.push(store.write_object(fo).unwrap());
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
        .unwrap()
}

/// The `n`th scripted fact. Every third one breaches a seated entity — touching
/// the negated stratum — and the rest are inert additions.
fn scripted_fact(n: usize) -> FactObject {
    const SEATED: [&str; 3] = ["oak", "elm", "ash"];
    let label = format!("f{n}");
    if n.is_multiple_of(3) {
        certain_fact(&label, "breached", &[SEATED[(n / 3) % 3]])
    } else {
        let arg = format!("fen-{n}");
        certain_fact(&label, "keeps", &[&arg])
    }
}

/// A store plus every commit a scripted history produced.
struct Dag {
    store: Lorestore,
    /// Distinct commits in creation order — the heads the parity sweep visits.
    heads: Vec<ContentHash>,
    /// How many of the script's steps were merges (reported so a seed that
    /// happened to script none is visible rather than silently weaker).
    merges: usize,
    _dir: TempDir,
}

/// Script a random commit DAG: branch off, commit facts, merge back.
///
/// Every fact cites `D-1` and mints no event variable, so `reconcile_interners`
/// never has cause to refuse and the script always completes — the oracle
/// measures the cache, not the merge engine's refusal paths, which the LS-6
/// oracles already own.
fn build_dag(seed: u64, steps: usize) -> Dag {
    let dir = TempDir::new().unwrap();
    let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
    store.import_pack(&ward_pack()).unwrap();

    let mut rng = Lcg::seeded(seed);
    let mut branches = vec!["main".to_owned()];
    let mut heads = vec![store.branch_head("main").unwrap().unwrap()];
    let mut minted = 0usize;
    let mut facts_written = 0usize;
    let mut merges = 0usize;

    for _ in 0..steps {
        match rng.next() % 10 {
            // Commit one or two facts to a random branch.
            0..=5 => {
                let branch = branches[rng.below(branches.len())].clone();
                let count = 1 + rng.below(2);
                let facts: Vec<FactObject> = (0..count)
                    .map(|_| {
                        facts_written += 1;
                        scripted_fact(facts_written)
                    })
                    .collect();
                heads.push(commit_facts(&store, &branch, &facts));
            }
            // Fork a new branch off a random existing one.
            6 | 7 => {
                let from = branches[rng.below(branches.len())].clone();
                minted += 1;
                let name = format!("b{minted}");
                store.create_branch(&name, &from).unwrap();
                branches.push(name);
            }
            // Merge one branch into another.
            _ => {
                if branches.len() < 2 {
                    continue;
                }
                let target = branches[rng.below(branches.len())].clone();
                let source = branches[rng.below(branches.len())].clone();
                if target == source {
                    continue;
                }
                let report = store.merge(&target, &source).unwrap();
                merges += 1;
                heads.push(report.merge_commit);
            }
        }
    }

    // Distinct commits only: two identical commits on the same parent are the
    // same content-addressed object, and counting one twice would inflate the
    // hit floor without adding coverage.
    let mut distinct = std::collections::BTreeSet::new();
    heads.retain(|h| distinct.insert(h.clone()));

    Dag {
        store,
        heads,
        merges,
        _dir: dir,
    }
}

/// A legible rendering of a compiled world — compared before the full equality
/// so a divergence reads as a diff of rows rather than as a wall of JSON.
fn signature(world: &dli_world::CompiledWorld) -> Vec<String> {
    let mut rows = common::render_facts(&world.model.world);
    rows.extend(common::render_derived(&world.model.world));
    rows.extend(common::render_findings(world.model.findings()));
    rows
}

/// The parity assertion itself: cached answer == cold answer, at one commit.
fn assert_parity(store: &Lorestore, head: &ContentHash, context: &str) {
    let warm = store.compile_at(head).unwrap();
    let cold = store.compile_at_uncached(head).unwrap();
    assert_eq!(
        signature(&warm),
        signature(&cold),
        "{context}: the cached world at {head} differs from a full recompute"
    );
    assert!(
        warm == cold,
        "{context}: the cached world at {head} is not byte-identical to a full \
         recompute (the rendered signatures matched, so the difference is in a \
         field the signature does not render)"
    );
    // The pack half is memoized separately and has its own parity.
    assert_eq!(
        store.reconstruct_at(head).unwrap(),
        store.reconstruct_at_uncached(head).unwrap(),
        "{context}: the cached pack at {head} differs from a full reconstruction"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 1 — PARITY OVER RANDOMIZED DAGS, WITH A HIT FLOOR
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn cached_compile_at_matches_a_full_recompute_over_random_dags() {
    let mut total_hits = 0u64;
    let mut total_heads = 0usize;
    let mut total_merges = 0usize;

    for seed in 0..SEEDS {
        let dag = build_dag(seed, STEPS);
        assert!(
            dag.heads.len() <= dli_lorestore::DEFAULT_CACHE_CAPACITY,
            "seed {seed} scripted {} distinct commits, past the default cache \
             capacity — the second-sweep hit floor below assumes no eviction",
            dag.heads.len()
        );

        // Sweep 1 — cold on entry to each head; warms the cache as it goes.
        for head in &dag.heads {
            assert_parity(&dag.store, head, &format!("seed {seed}, first sweep"));
        }

        // Sweep 2 — every head is memoized now. Parity must still hold, and the
        // cache must be the thing answering.
        let before = dag.store.cache_stats();
        for head in &dag.heads {
            assert_parity(&dag.store, head, &format!("seed {seed}, second sweep"));
        }
        let after = dag.store.cache_stats();

        let sweep_hits = after.world_hits - before.world_hits;
        assert!(
            sweep_hits >= dag.heads.len() as u64,
            "seed {seed}: the second sweep over {} commits produced only \
             {sweep_hits} cache hits — every head was already memoized, so a \
             shortfall means `compile_at` is not consulting the cache and this \
             oracle is comparing a full recompute against itself",
            dag.heads.len()
        );
        assert_eq!(
            after.evictions, 0,
            "seed {seed}: nothing should have been evicted under the default \
             capacity at this DAG size"
        );

        total_hits += after.world_hits;
        total_heads += dag.heads.len();
        total_merges += dag.merges;
    }

    assert!(
        total_merges > 0,
        "the scripted DAGs produced no merges at all — the sweep never covered a \
         two-parent commit"
    );
    assert!(
        total_hits >= MIN_TOTAL_WORLD_HITS,
        "the whole sweep produced {total_hits} cache hits over {total_heads} \
         commits, under the floor of {MIN_TOTAL_WORLD_HITS}"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 2 — THE POSITIVE CONTROL: THE PARITY ASSERTION CAN FAIL
// ════════════════════════════════════════════════════════════════════════════

/// Corrupt one memoized world and require the store to hand it back.
///
/// This is what stops test 1 from being vacuous, in both directions at once. If
/// `compile_at` did not read the cache, the corrupted entry would be invisible
/// and this test would fail. If the parity assertion could not see a
/// single-grade difference, it would also fail. Passing means test 1 is
/// comparing two genuinely different code paths with an assertion sharp enough
/// to tell them apart.
#[test]
fn a_corrupted_cache_entry_is_visible_and_breaks_parity() {
    let dag = build_dag(7, STEPS);
    let head = dag.heads.last().unwrap();

    let clean = dag.store.compile_at(head).unwrap();
    assert!(
        dag.store.falsify_cached_world(head),
        "there must have been a memoized world with a derived row to corrupt"
    );

    let served = dag.store.compile_at(head).unwrap();
    assert_ne!(
        served, clean,
        "`compile_at` returned the uncorrupted world — it is not reading the \
         cache, which makes the parity oracle vacuous"
    );
    assert_ne!(
        served,
        dag.store.compile_at_uncached(head).unwrap(),
        "the parity assertion could not see the corruption — it lacks the teeth \
         to catch a real divergence"
    );

    // And the corruption is confined to the cache: clearing it restores the
    // store's answer, which is the whole point of a derived reader.
    dag.store.clear_cache();
    assert_eq!(
        dag.store.compile_at(head).unwrap(),
        clean,
        "clearing the cache must restore the recomputed answer"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 3 — PARITY UNDER EVICTION, AND WITH THE CACHE OFF
// ════════════════════════════════════════════════════════════════════════════

/// Capacity 1: every read but the last evicts. Parity is unaffected, because an
/// evicted entry is a rebuildable derived reader and a miss is just a recompute.
#[test]
fn parity_survives_an_evicting_cache() {
    let dag = build_dag(11, STEPS);
    assert!(
        dag.heads.len() > 2,
        "the eviction sweep needs more commits than the capacity"
    );
    dag.store.set_cache_capacity(1);

    for head in &dag.heads {
        assert_parity(&dag.store, head, "capacity 1");
    }

    let stats = dag.store.cache_stats();
    assert!(
        stats.evictions > 0,
        "sweeping {} commits at capacity 1 evicted nothing",
        dag.heads.len()
    );
}

/// Capacity 0: the cache is off entirely and the store still answers correctly.
/// Together with test 1 this brackets the cache — neither a full one nor an
/// absent one changes a single verdict.
#[test]
fn parity_holds_with_the_cache_disabled() {
    let dag = build_dag(13, STEPS);
    dag.store.set_cache_capacity(0);
    dag.store.clear_cache();

    let before = dag.store.cache_stats();
    for head in &dag.heads {
        assert_parity(&dag.store, head, "capacity 0");
    }
    let after = dag.store.cache_stats();

    assert_eq!(
        after.world_hits, before.world_hits,
        "a disabled cache must serve nothing"
    );
    assert_eq!(
        after.pack_hits, before.pack_hits,
        "a disabled cache must serve nothing"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 4 — CONFLUENCE THROUGH THE CACHED PATH
// ════════════════════════════════════════════════════════════════════════════

/// The same facts, committed in two different cross-branch interleavings and
/// merged, compile to the same world.
///
/// This is the store's CALM property, asserted here through the cached read
/// path: a cache keyed on anything order-dependent — a ref name, a sequence
/// number, a branch — would key the two histories differently and diverge.
#[test]
fn confluence_holds_through_the_cache() {
    /// Build the same content two ways: `interleave = false` finishes branch `a`
    /// before starting `b`; `interleave = true` alternates between them. Per
    /// branch the commit order is identical; only the cross-branch interleaving
    /// differs.
    fn built(interleave: bool) -> (Lorestore, ContentHash, TempDir) {
        let dir = TempDir::new().unwrap();
        let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
        store.import_pack(&ward_pack()).unwrap();
        store.create_branch("a", "main").unwrap();
        store.create_branch("b", "main").unwrap();

        let a: Vec<FactObject> = (1..=3).map(|n| scripted_fact(n * 3)).collect();
        let b: Vec<FactObject> = (1..=3).map(|n| scripted_fact(n * 3 + 1)).collect();

        if interleave {
            for (fa, fb) in a.iter().zip(b.iter()) {
                commit_facts(&store, "a", std::slice::from_ref(fa));
                commit_facts(&store, "b", std::slice::from_ref(fb));
            }
        } else {
            for fa in &a {
                commit_facts(&store, "a", std::slice::from_ref(fa));
            }
            for fb in &b {
                commit_facts(&store, "b", std::slice::from_ref(fb));
            }
        }

        let merged = store.merge("a", "b").unwrap();
        (store, merged.merge_commit, dir)
    }

    let (s1, h1, _d1) = built(false);
    let (s2, h2, _d2) = built(true);

    // Warm both through the cache, then confirm each is its own cold recompute.
    assert_parity(&s1, &h1, "confluence, sequential order");
    assert_parity(&s2, &h2, "confluence, interleaved order");

    assert_eq!(
        signature(&s1.compile_at(&h1).unwrap()),
        signature(&s2.compile_at(&h2).unwrap()),
        "the same facts committed in two cross-branch orders compiled to \
         different worlds"
    );
}

/// The second blocker: a child commit that declares a new decision does not
/// merely *append* to its parent's declarations — it changes declaration zero.
///
/// `reconstruct_pack` emits the ratify prelude at the **front** of the document,
/// one synthesized `ratify` per known decision that no stored carriage object
/// already replays. The base pack here was imported, so its `ratify D-1` is a
/// *carriage* object and the prelude is empty at the parent. Add `D-2` — a
/// decision with no carriage — and the prelude becomes `[ratify D-2]`, which
/// lands ahead of every declaration the parent had. The parent's declarations
/// are not a prefix of the child's at any index.
///
/// So even once the pack-provenance blocker above is fixed, a parent-delta path
/// needs an explicit precondition guard (`child.decisions == parent.decisions`,
/// plus the same `pack_name`) with a fallback to the full compile: the delta is
/// not a suffix, and reconstructing it as one would compile a different world.
#[test]
fn a_new_decision_breaks_the_parent_prefix() {
    let dir = TempDir::new().unwrap();
    let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
    store.import_pack(&ward_pack()).unwrap();
    let parent = store.branch_head("main").unwrap().unwrap();

    // A child that adds a fact AND declares a second decision. `D-2` has no
    // carriage object, so it is synthesized into the prelude — at the very
    // front of the document.
    let fact = store.write_object(&scripted_fact(3)).unwrap();
    let (interner, _) = branch_state(&store, "main");
    let child = store
        .commit_on_branch(
            "main",
            interner,
            vec![fact],
            Vec::new(),
            Vec::new(),
            vec![DECISION.to_owned(), "D-2".to_owned()],
            "canon".to_owned(),
        )
        .unwrap();

    let parent_decls = store.reconstruct_at(&parent).unwrap().documents[0]
        .decls
        .clone();
    let child_decls = store.reconstruct_at(&child).unwrap().documents[0]
        .decls
        .clone();

    assert!(
        !parent_decls
            .iter()
            .zip(child_decls.iter())
            .all(|(a, b)| a == b),
        "a new decision must break the parent-prefix property — if it no longer \
         does, the parent-delta guard this test justifies can be relaxed"
    );
    assert!(
        matches!(&child_decls[0], Decl::Ratify(r) if r.decision.text == "D-2"),
        "the new decision is synthesized into the prelude at position 0, ahead \
         of every declaration the parent had — the delta is not a suffix"
    );
    assert!(
        matches!(&parent_decls[0], Decl::Ratify(r) if r.decision.text == DECISION),
        "the parent's own declaration zero is its carried `ratify D-1`, which the \
         child has displaced to position 1"
    );
    assert_eq!(
        child_decls[1], parent_decls[0],
        "the displacement is a shift, not a rewrite: everything the parent \
         declared survives, one position later"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 6 — THE MERGE SCAN READS AND FEEDS THE SESSION CACHE
// ════════════════════════════════════════════════════════════════════════════

/// A merge scan compiles three pre-merge worlds and the merged one. All four go
/// through the session cache, so a second merge over the same heads costs
/// nothing, and the merged head is memoized for the read that normally follows a
/// merge report.
#[test]
fn the_merge_scan_shares_the_session_cache() {
    let dir = TempDir::new().unwrap();
    let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
    store.import_pack(&ward_pack()).unwrap();
    store.create_branch("a", "main").unwrap();
    store.create_branch("b", "main").unwrap();
    commit_facts(&store, "a", &[scripted_fact(3)]);
    commit_facts(&store, "b", &[scripted_fact(6)]);

    // A DRY RUN, not a real merge: a real merge advances `a` to the merge
    // commit, so the "same two heads" the rescan below is about would no longer
    // be the same two heads — it would be a fresh merge of a moved target, and
    // the rescan assertion would be measuring nothing. `advance = false` writes
    // the identical content-addressed merge commit and runs the identical scan
    // while leaving both refs where they are, which is exactly the WS-3
    // dry-run-then-merge shape this is here to pin.
    let report = store.merge_with("a", "b", false, None).unwrap();

    // The merge commit's own world was memoized by the scan, so the read that
    // normally follows a merge report is a hit, not a recompile.
    let before = store.cache_stats();
    let merged = store.compile_at(&report.merge_commit).unwrap();
    let after = store.cache_stats();
    assert_eq!(
        after.world_hits,
        before.world_hits + 1,
        "reading the merged head after a merge must be served from the cache"
    );
    assert_eq!(
        merged,
        store.compile_at_uncached(&report.merge_commit).unwrap(),
        "the world the merge scan memoized must be the one a cold recompute \
         produces"
    );

    // A dry-run rescan of the same two heads recompiles nothing.
    //
    // Asserting only `world_misses == before` would be too weak to mean
    // anything: the scan compiles the merge commit *inline* (it must, to tell an
    // unstratifiable merge apart from any other failure), and an inline compile
    // is invisible to the miss counter. So a scan that recompiled the merged
    // world every time would satisfy a misses-only assertion while doing the
    // full work. The hit floor is what actually pins it — the scan must ASK the
    // cache for all four worlds and be answered.
    let before = store.cache_stats();
    store.merge_with("a", "b", false, None).unwrap();
    let after = store.cache_stats();
    assert_eq!(
        after.world_misses, before.world_misses,
        "a second scan over already-compiled heads recompiled a world"
    );
    assert!(
        after.world_hits - before.world_hits >= 4,
        "a rescan must be served the target, source, base, and MERGED worlds \
         from the cache — got {} hits. A shortfall means the scan is doing a \
         full compile somewhere the miss counter cannot see.",
        after.world_hits - before.world_hits
    );
}
