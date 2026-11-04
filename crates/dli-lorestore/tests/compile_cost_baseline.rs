//! The compile-cost baseline: where the time actually goes, once compiles are
//! cached.
//!
//! The whole-world [`dli_world::check::run`] battery may become the residual
//! `O(world)` term once redundant recompiles are gone. This test measures that
//! cost so decisions about splitting the battery can be based on evidence.
//!
//! # Not in the gate, and why it may read a clock
//!
//! Every test here is `#[ignore]`d. `cargo nextest run` — the gate's test
//! runner — does not run ignored tests without `--run-ignored`, and no gate step
//! passes it, so nothing here executes in a gating lane. That is what licenses
//! the clock: the offline-and-deterministic law (DL-0013) binds the lanes that
//! block a merge, and a measurement that never runs in one cannot make a verdict
//! depend on wall time. Nothing in this file asserts on a duration.
//!
//! # Running it
//!
//! ```text
//! cargo test -p dli-lorestore --all-features --release \
//!     --test compile_cost_baseline -- --ignored --nocapture
//! ```
//!
//! **`--release` matters.** A debug-profile timing of a symbolic fixpoint is
//! noise with units; the committed baseline is a release measurement and a
//! debug one is not comparable to it.
//!
//! The committed result — the numbers, the machine, the commit, and the
//! parameters — is `docs/design-notes/incremental-compile-baseline.md`. Re-run
//! this to refresh it; do not hand-edit the tables.
//!
//! # The workload
//!
//! A synthetic **identity world** (DL-0087 §1): each
//! persona arrives as its own commit carrying a handful of mention/role facts
//! and one `same-as` identity assertion. Identity is deliberately the *worst*
//! case for fixpoint reuse — `same-as` synthesizes alias-rewrite rules over
//! every predicate, so any identity assertion marks the whole world affected
//! — which is exactly why the interesting question is what the
//! *other* phases cost.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::time::{Duration, Instant};

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{Decl, FactDecl, RatifyDecl, RuleDecl, SameAsDecl};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_lorestore::{ContentHash, Lorestore};
use tempfile::TempDir;

const DECISION: &str = "D-1";

/// Persona counts swept by the phase-split baseline. Each persona is one commit
/// carrying five declarations, so the largest world is a few hundred
/// declarations over a few dozen commits — the shape a pilot persona reaches,
/// not a synthetic million.
const PERSONA_SWEEP: &[usize] = &[8, 16, 32, 64, 128];

/// Median of this many repetitions per measurement.
const REPS: usize = 5;

// ── fixtures ────────────────────────────────────────────────────────────────

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn ratified() -> Annotation {
    Annotation {
        marker: ProvenanceMarker::Ratified,
        citations: vec![CitationRef { id: id(DECISION) }],
        condition: None,
        bundle: None,
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

fn rule_decl(name: &str, body_text: &str) -> Decl {
    let name_ident = id(name);
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

/// The seed commit: the ruled decision, and two rules that give the fixpoint
/// real work to do across the mention graph.
fn seed_decls() -> Vec<Decl> {
    vec![
        Decl::Ratify(RatifyDecl {
            decision: id(DECISION),
            body: dli_ast::language::RatifyBody::default(),
        }),
        rule_decl("appears", "appears($p, $s) :- mentions($p, $s)"),
        rule_decl(
            "co-mentioned",
            "co-mentioned($p, $q) :- appears($p, $s), appears($q, $s)",
        ),
    ]
}

/// One persona's arrival: four facts and the identity assertion that binds its
/// alias — a representative matched mention.
fn persona_decls(i: usize) -> Vec<Decl> {
    let p = format!("p{i}");
    let alias = format!("a{i}");
    let scene = format!("s{i}");
    let shared = format!("s{}", i % 8);
    vec![
        fact_decl("mentions", &[&p, &scene]),
        fact_decl("role", &[&p, "narrator"]),
        fact_decl("mentions", &[&alias, &shared]),
        fact_decl("role", &[&alias, "witness"]),
        Decl::SameAs(SameAsDecl {
            left: id(&p),
            right: id(&alias),
            provenance: Some(ratified()),
        }),
    ]
}

// ── store construction ──────────────────────────────────────────────────────

/// Builds the store one commit at a time, exactly as the authoring tools do —
/// through `lower_decl`, carrying the interner and the anonymous-bundle counter
/// forward.
struct Builder {
    store: Lorestore,
    interner: dli_core::atom::Interner,
    counter: u32,
    _dir: TempDir,
}

impl Builder {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
        Self {
            store,
            interner: dli_core::atom::Interner::from_names(Vec::new()),
            counter: 0,
            _dir: dir,
        }
    }

    fn commit(&mut self, decls: &[Decl]) -> ContentHash {
        let mut hashes = Vec::new();
        for decl in decls {
            for object in dli_lorestore::lower::lower_decl(
                decl,
                "canon",
                &mut self.interner,
                &mut self.counter,
            ) {
                hashes.push(self.store.write_object(&object).unwrap());
            }
        }
        self.store
            .commit_on_branch(
                "main",
                self.interner.names().to_vec(),
                hashes,
                Vec::new(),
                Vec::new(),
                vec![DECISION.to_owned()],
                "canon".to_owned(),
            )
            .unwrap()
    }
}

/// A store holding `personas` personas, one commit each, on top of the seed.
fn build(personas: usize) -> (Builder, ContentHash) {
    let mut builder = Builder::new();
    let mut head = builder.commit(&seed_decls());
    for i in 0..personas {
        head = builder.commit(&persona_decls(i));
    }
    (builder, head)
}

// ── timing ──────────────────────────────────────────────────────────────────

/// Median of `REPS` runs, plus the last value produced. Median rather than mean
/// so one scheduler hiccup does not move the number.
fn timed<T>(mut f: impl FnMut() -> T) -> (Duration, T) {
    let mut samples = Vec::with_capacity(REPS);
    let mut last = None;
    for _ in 0..REPS {
        let start = Instant::now();
        let value = f();
        samples.push(start.elapsed());
        last = Some(value);
    }
    samples.sort_unstable();
    (samples[samples.len() / 2], last.unwrap())
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn pct(part: Duration, whole: Duration) -> f64 {
    if whole.is_zero() {
        0.0
    } else {
        100.0 * part.as_secs_f64() / whole.as_secs_f64()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// THE PHASE SPLIT — does check::run dominate once compiles are cached?
// ════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore = "measurement, not a verdict: reads a clock, so it never runs in a gating lane"]
fn baseline_phase_split() {
    println!("\n### Phase split per cold `compile_at`, by world size\n");
    println!(
        "| personas | commits | decls | facts | derived | reconstruct | loader | fixpoint+seal | check::run | cold total | warm `compile_at` |"
    );
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");

    for &personas in PERSONA_SWEEP {
        let (builder, head) = build(personas);
        let store = &builder.store;

        // 1 — reconstruct: two DAG walks, a read of every reachable fact object,
        // and `reconstruct_pack`.
        let (t_reconstruct, pack) = timed(|| store.reconstruct_at_uncached(&head).unwrap());

        // 2 — loader: per-declaration lowering plus the whole-pack aggregation,
        // stopping short of the ledger valuation and the fixpoint.
        let (t_loader, _) = timed(|| {
            let mut load = dli_world::loader::IncrementalLoad::new();
            load.extend(&pack.documents);
            load.finish()
        });

        // 3 — the whole compile, and the check battery over its result.
        let (t_compile, world) = timed(|| dli_world::compile(&pack).unwrap());
        let (t_check, findings) = timed(|| dli_world::check::run(&world));

        // 4 — the cached read, for scale.
        store.clear_cache();
        let _ = store.compile_at(&head).unwrap();
        let (t_warm, _) = timed(|| store.compile_at(&head).unwrap());

        // The fixpoint half is what the compile does beyond the loader: the
        // ledger valuation, stratification, and the saturation. It is not
        // separately callable from outside the crate, so it is measured by
        // difference — which is why the loader is timed against the identical
        // documents rather than estimated.
        let t_fixpoint = t_compile.saturating_sub(t_loader);
        let cold_total = t_reconstruct + t_compile + t_check;

        println!(
            "| {personas} | {} | {} | {} | {} | {:.1} ms ({:.0}%) | {:.1} ms ({:.0}%) | {:.1} ms ({:.0}%) | **{:.1} ms ({:.0}%)** | {:.1} ms | {:.2} ms |",
            personas + 1,
            pack.documents.iter().map(|d| d.decls.len()).sum::<usize>(),
            world.model.world.facts.len(),
            world.model.world.derived.len(),
            ms(t_reconstruct),
            pct(t_reconstruct, cold_total),
            ms(t_loader),
            pct(t_loader, cold_total),
            ms(t_fixpoint),
            pct(t_fixpoint, cold_total),
            ms(t_check),
            pct(t_check, cold_total),
            ms(cold_total),
            ms(t_warm),
        );
        assert!(findings.len() < usize::MAX, "keep the findings alive");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// THE CACHE — what the session cache removes from a merge-shaped workload
// ════════════════════════════════════════════════════════════════════════════

/// Fork two branches off the built head and give each one extra persona, so a
/// merge between them is a genuine two-parent scan and not a fast-forward.
fn fork_two_branches(builder: &Builder, personas: usize) {
    let store = &builder.store;
    store.create_branch("a", "main").unwrap();
    store.create_branch("b", "main").unwrap();

    let mut interner = builder.interner.clone();
    let mut counter = builder.counter;
    for (branch, i) in [("a", personas), ("b", personas + 1)] {
        let mut hashes = Vec::new();
        for decl in persona_decls(i) {
            for object in
                dli_lorestore::lower::lower_decl(&decl, "canon", &mut interner, &mut counter)
            {
                hashes.push(store.write_object(&object).unwrap());
            }
        }
        store
            .commit_on_branch(
                branch,
                interner.names().to_vec(),
                hashes,
                Vec::new(),
                Vec::new(),
                vec![DECISION.to_owned()],
                "canon".to_owned(),
            )
            .unwrap();
    }
}

/// The direct measurement of Stage A's win on the shape DL-0087 §1 names: an
/// `author.merge` scan that recompiles the whole world 3–8× per call.
///
/// Every scan is a **dry run**, so the target ref never moves and the cold and
/// warm scans are genuinely over the same two heads. A real merge would advance
/// the target, making the second scan a different merge and the comparison
/// meaningless.
#[test]
#[ignore = "measurement, not a verdict: reads a clock, so it never runs in a gating lane"]
fn baseline_merge_scan_cache() {
    println!("\n### `author.merge` scan cost, cache cold vs warm\n");
    println!(
        "| personas | scan (cold cache) | scan (warm cache) | speedup | worlds compiled cold | warm |"
    );
    println!("|---:|---:|---:|---:|---:|---:|");

    for &personas in PERSONA_SWEEP {
        let (builder, _head) = build(personas);
        let store = &builder.store;
        fork_two_branches(&builder, personas);

        // Cold: nothing memoized.
        store.clear_cache();
        let cold_start = store.cache_stats();
        let cold = Instant::now();
        store.merge_with("a", "b", false, None).unwrap();
        let t_cold = cold.elapsed();
        let cold_misses = store.cache_stats().world_misses - cold_start.world_misses;

        // Warm: the same scan again, everything already memoized — including the
        // merge commit, whose hash is identical because a dry run writes the same
        // content-addressed object a real merge would.
        let warm_start = store.cache_stats();
        let warm = Instant::now();
        store.merge_with("a", "b", false, None).unwrap();
        let t_warm = warm.elapsed();
        let warm_misses = store.cache_stats().world_misses - warm_start.world_misses;

        println!(
            "| {personas} | {:.1} ms | {:.1} ms | {:.2}× | {cold_misses} | {warm_misses} |",
            ms(t_cold),
            ms(t_warm),
            t_cold.as_secs_f64() / t_warm.as_secs_f64().max(f64::MIN_POSITIVE),
        );
    }
}

/// **The residual per-`author.merge` cost split, measured directly.**
///
/// With the session cache in effect the redundant recompiles collapse and each
/// merge pays only for what is genuinely new: the merge commit itself, which no
/// cache can have seen before. So the honest steady-state question is what
/// *that* residual is made of, and specifically whether `check::run` has become
/// its dominant term.
///
/// The three pre-merge worlds are warmed before the clock starts — that is the
/// post-Stage-A steady state, not a trick — and the scan is then timed over a
/// merge commit nothing has compiled. The residual is split into the merge
/// commit's own reconstruct / loader / fixpoint / `check::run`, plus the scan's
/// **own** detection work (exclusivity, FD grouping, standing downgrades over
/// every derived row), which is not a compile at all and which nothing in this
/// campaign has measured before.
#[test]
#[ignore = "measurement, not a verdict: reads a clock, so it never runs in a gating lane"]
fn baseline_merge_residual_split() {
    println!("\n### Residual per-`author.merge` cost, three pre-merge worlds already cached\n");
    println!(
        "| personas | derived rows | residual scan | reconstruct | loader | fixpoint+seal | check::run | scan detection |"
    );
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|");

    for &personas in PERSONA_SWEEP {
        let (builder, _head) = build(personas);
        let store = &builder.store;
        fork_two_branches(&builder, personas);

        let target = store.branch_head("a").unwrap().unwrap();
        let source = store.branch_head("b").unwrap().unwrap();
        let base = store.merge_base(&target, &source).unwrap();

        // Median of REPS scans, each preceded by an UNTIMED warm of exactly the
        // three pre-merge worlds — the state Stage A leaves behind — and an
        // untimed clear, so the merge commit is genuinely uncompiled every rep.
        let mut samples = Vec::with_capacity(REPS);
        let mut merge_commit = None;
        for _ in 0..REPS {
            store.clear_cache();
            store.compile_at(&target).unwrap();
            store.compile_at(&source).unwrap();
            if let Some(b) = &base {
                store.compile_at(b).unwrap();
            }
            let start = Instant::now();
            let report = store.merge_with("a", "b", false, None).unwrap();
            samples.push(start.elapsed());
            merge_commit = Some(report.merge_commit);
        }
        samples.sort_unstable();
        let t_residual = samples[samples.len() / 2];
        let merge_commit = merge_commit.unwrap();

        // The same merge commit's phases, measured apart from the scan.
        let (t_reconstruct, pack) = timed(|| store.reconstruct_at_uncached(&merge_commit).unwrap());
        let (t_loader, _) = timed(|| {
            let mut load = dli_world::loader::IncrementalLoad::new();
            load.extend(&pack.documents);
            load.finish()
        });
        let (t_compile, world) = timed(|| dli_world::compile(&pack).unwrap());
        let (t_check, _) = timed(|| dli_world::check::run(&world));
        let t_fixpoint = t_compile.saturating_sub(t_loader);
        // Whatever the scan spends beyond compiling and checking the merge
        // commit: the conflict detectors and the standing-downgrade sweep.
        //
        // This is a DIFFERENCE OF TWO INDEPENDENTLY TIMED QUANTITIES, and it is
        // printed signed on purpose. Once the fixpoint reaches seconds, the
        // run-to-run variance of a single compile exceeds the quantity being
        // subtracted out, and the column goes negative — which is the honest
        // report that the method has hit its noise floor, not a measurement of
        // zero. Clamping it to 0 would dress that up as "detection is free".
        let detect_ms = ms(t_residual) - ms(t_reconstruct + t_compile + t_check);

        println!(
            "| {personas} | {} | {:.1} ms | {:.1} ms ({:.0}%) | {:.1} ms ({:.0}%) | {:.1} ms ({:.0}%) | **{:.1} ms ({:.0}%)** | {detect_ms:+.1} ms ({:+.0}%) |",
            world.model.world.derived.len(),
            ms(t_residual),
            ms(t_reconstruct),
            pct(t_reconstruct, t_residual),
            ms(t_loader),
            pct(t_loader, t_residual),
            ms(t_fixpoint),
            pct(t_fixpoint, t_residual),
            ms(t_check),
            pct(t_check, t_residual),
            100.0 * detect_ms / ms(t_residual).max(f64::MIN_POSITIVE),
        );
    }
}
