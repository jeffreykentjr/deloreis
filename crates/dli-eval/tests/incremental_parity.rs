#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The permanent incremental-parity oracle (DL-0087).
//!
//! [`World::saturate_incremental`] reuses a prior fixpoint instead of
//! recomputing it. The claim that buys is **byte-identity**: for the same
//! program it must produce the world [`World::saturate_with`] produces, down to
//! the serialized bytes — not "the same answers", not "the same rows modulo
//! ordering". This file is the standing proof of that claim, and it is gate-
//! resident on purpose: an incremental evaluator that silently diverges from its
//! own full recompute is worse than no incrementality at all, because every
//! verdict downstream of it stops being replayable.
//!
//! The named failure mode is **divergence under some ordering** of the delta, so
//! the oracle never trusts one ordering. Each generated trial permutes its
//! additions several ways and demands that every ordering agree with full
//! recompute *and* with every other ordering.
//!
//! Generation is a deterministic LCG over a fixed seed range: no clock, no RNG
//! from the environment, so a failure here reproduces exactly (`SEEDS` and the
//! seed printed in the assertion message locate it).
//!
//! The coverage counters at the bottom are asserted against **floors**, not
//! against `> 0`. A generator that drifts until it stops producing negation
//! across strata, or stops producing defeater-only growth, would still pass a
//! `> 0` check while testing nothing; a floor turns that drift into a red test.

use std::collections::{BTreeMap, BTreeSet};

use dli_ast::ident::Ident;
use dli_ast::literal::BodyLiteral;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::{Ann, DefeaterConditions};
use dli_core::atom::Atom;
use dli_core::grade::Grade;
use dli_core::posbool::{Monomial, PosBool};

use dli_eval::Strata;
use dli_eval::world::{
    Derived, GroundAtom, ReuseReport, World, incremental_divergence, synthesize_alias_rules,
};

// ---------------------------------------------------------------------------
// Deterministic generation
// ---------------------------------------------------------------------------

/// A 64-bit linear congruential generator — the repo's deterministic-generator
/// idiom (`dli-lorestore/tests/merge.rs`). No clock, no thread RNG.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        // Scramble the seed so adjacent seeds do not produce adjacent programs.
        Lcg(seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0x1234_5678_9ABC_DEF0))
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 17
    }

    /// A value in `0..n` (`n` must be non-zero).
    fn below(&mut self, n: usize) -> usize {
        let n = u64::try_from(n).unwrap();
        usize::try_from(self.next_u64() % n).unwrap()
    }

    /// True with probability `numerator / 16`.
    fn chance(&mut self, numerator: u64) -> bool {
        self.next_u64() % 16 < numerator
    }

    /// Fisher–Yates, in place.
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i + 1));
        }
    }
}

// ---------------------------------------------------------------------------
// The generated program shape
// ---------------------------------------------------------------------------

/// The entity universe. Small on purpose: the alias-rewrite rules synthesized
/// for every predicate make the relation grow with the square of this, and the
/// oracle must stay inside an ordinary `cargo test` run.
const ENTITIES: usize = 3;

/// Event variables interned up front, so no addition ever has to intern — the
/// interner stays a prefix of itself and the reuse precondition is exercised the
/// way a real fork exercises it (supposing variables the base already knows).
const EVENT_VARS: usize = 6;

/// The two decision variables (`EVENT_VARS - 2`, `EVENT_VARS - 1`) are ruled
/// true in the ambient valuation, standing in for cited decisions.
const DECISIONS: usize = 2;

fn ent(i: usize) -> String {
    format!("e{i}")
}

fn var(name: &str) -> Term {
    Term::Var(Ident::synthetic(name))
}

fn pattern(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: Ident::synthetic(pred),
        args: args.iter().map(|a| var(a)).collect(),
    }
}

fn ground(pred: &str, args: &[String]) -> GroundAtom {
    GroundAtom {
        pred: pred.to_owned(),
        args: args.to_vec(),
    }
}

/// One thing the delta adds. Facts and rules land in different vectors, so a
/// permutation of the additions permutes each vector's contents independently —
/// which is exactly the ordering freedom the fixpoint must be blind to.
#[derive(Debug, Clone)]
enum Addition {
    Fact {
        atom: GroundAtom,
        ann: Ann,
        grade: Grade,
    },
    Rule {
        name: String,
        head: AtomExpr,
        body: Vec<BodyLiteral>,
        ann: Ann,
        grade: Grade,
    },
}

impl Addition {
    fn apply(&self, world: &mut World) {
        match self {
            Addition::Fact { atom, ann, grade } => {
                world.assert_fact_graded(atom.clone(), ann.clone(), *grade);
            }
            Addition::Rule {
                name,
                head,
                body,
                ann,
                grade,
            } => {
                world
                    .define_rule_annotated(
                        name.clone(),
                        head.clone(),
                        body.clone(),
                        ann.clone(),
                        *grade,
                    )
                    .unwrap();
            }
        }
    }
}

/// The always-present rule skeleton: eight rules over six intensional
/// predicates, laying out **seven dependency strata** with stratified negation
/// crossing two of the boundaries (`safe` over `blocked`, `crown` over `rival`).
/// `reach` is recursive, so at least one stratum is a genuine clique.
fn skeleton_rules(world: &mut World, rng: &mut Lcg) {
    let mut rule = |name: &str, head: AtomExpr, body: Vec<BodyLiteral>, rng: &mut Lcg| {
        // Rules-as-claims: some rules rest on an event variable of their own, so
        // a derivation through them carries that variable in its support.
        let ann = if rng.chance(4) {
            Ann::supported(PosBool::pos(Atom(
                u32::try_from(rng.below(EVENT_VARS)).unwrap(),
            )))
        } else {
            Ann::unconditional()
        };
        world
            .define_rule_annotated(name, head, body, ann, Grade::Asserted)
            .unwrap();
    };
    rule(
        "reach-base",
        pattern("reach", &["X", "Y"]),
        vec![BodyLiteral::pos(pattern("edge", &["X", "Y"]))],
        rng,
    );
    rule(
        "reach-step",
        pattern("reach", &["X", "Z"]),
        vec![
            BodyLiteral::pos(pattern("edge", &["X", "Y"])),
            BodyLiteral::pos(pattern("reach", &["Y", "Z"])),
        ],
        rng,
    );
    rule(
        "blocked-mark",
        pattern("blocked", &["X"]),
        vec![BodyLiteral::pos(pattern("mark", &["X"]))],
        rng,
    );
    rule(
        "blocked-reach",
        pattern("blocked", &["X"]),
        vec![
            BodyLiteral::pos(pattern("reach", &["X", "Y"])),
            BodyLiteral::pos(pattern("mark", &["Y"])),
        ],
        rng,
    );
    // Negation across a stratum boundary.
    rule(
        "safe-node",
        pattern("safe", &["X"]),
        vec![
            BodyLiteral::pos(pattern("node", &["X"])),
            BodyLiteral::not(pattern("blocked", &["X"])),
        ],
        rng,
    );
    rule(
        "rival-reach",
        pattern("rival", &["X"]),
        vec![
            BodyLiteral::pos(pattern("reach", &["X", "Y"])),
            BodyLiteral::pos(pattern("safe", &["Y"])),
        ],
        rng,
    );
    // A second negation, two strata higher: this is the one a delta down in
    // `edge`/`mark` has to re-poison all the way up.
    rule(
        "crown-safe",
        pattern("crown", &["X"]),
        vec![
            BodyLiteral::pos(pattern("safe", &["X"])),
            BodyLiteral::not(pattern("rival", &["X"])),
        ],
        rng,
    );
    rule(
        "heir-crown",
        pattern("heir", &["X"]),
        vec![
            BodyLiteral::pos(pattern("crown", &["X"])),
            BodyLiteral::pos(pattern("owner", &["X", "P"])),
        ],
        rng,
    );
}

/// The optional rules a seed may put in the base or hand to the delta. Each
/// preserves stratifiability (none closes a negative cycle).
fn optional_rule(which: usize) -> Addition {
    let (name, head, body) = match which {
        0 => (
            "reach-tie",
            pattern("reach", &["X", "Y"]),
            vec![BodyLiteral::pos(pattern("tie", &["X", "Y"]))],
        ),
        1 => (
            "blocked-self-tie",
            pattern("blocked", &["X"]),
            vec![BodyLiteral::pos(pattern("tie", &["X", "X"]))],
        ),
        2 => (
            "heir-rival",
            pattern("heir", &["X"]),
            vec![
                BodyLiteral::pos(pattern("rival", &["X"])),
                BodyLiteral::pos(pattern("owner", &["X", "P"])),
            ],
        ),
        _ => (
            "safe-unmarked",
            pattern("safe", &["X"]),
            vec![
                BodyLiteral::pos(pattern("node", &["X"])),
                BodyLiteral::not(pattern("mark", &["X"])),
            ],
        ),
    };
    Addition::Rule {
        name: name.to_owned(),
        head,
        body,
        ann: Ann::unconditional(),
        grade: Grade::Asserted,
    }
}

const OPTIONAL_RULES: usize = 4;

/// A random annotation over the interned event variables: unconditional, a
/// single hypothesis, a conjunction of two, a disjunction of two, or a decision
/// citation (a variable the ambient valuation rules true).
fn random_ann(rng: &mut Lcg) -> Ann {
    let pick = |rng: &mut Lcg| Atom(u32::try_from(rng.below(EVENT_VARS - DECISIONS)).unwrap());
    let decision =
        |rng: &mut Lcg| Atom(u32::try_from(EVENT_VARS - DECISIONS + rng.below(DECISIONS)).unwrap());
    match rng.below(5) {
        0 => Ann::unconditional(),
        1 => Ann::supported(PosBool::pos(pick(rng))),
        2 => Ann::supported(PosBool::pos(pick(rng)).and_(&PosBool::pos(pick(rng)))),
        3 => Ann::supported(PosBool::pos(pick(rng)).or_(&PosBool::pos(pick(rng)))),
        _ => Ann::supported(PosBool::pos(decision(rng))),
    }
}

fn random_grade(rng: &mut Lcg) -> Grade {
    match rng.below(4) {
        0 => Grade::Checked,
        1 => Grade::Tested,
        2 => Grade::Attested,
        _ => Grade::Asserted,
    }
}

/// Every ground atom the generator may assert, in a fixed order.
fn atom_universe() -> Vec<GroundAtom> {
    let mut atoms = Vec::new();
    for i in 0..ENTITIES {
        atoms.push(ground("node", &[ent(i)]));
        atoms.push(ground("mark", &[ent(i)]));
        for j in 0..ENTITIES {
            atoms.push(ground("edge", &[ent(i), ent(j)]));
            atoms.push(ground("tie", &[ent(i), ent(j)]));
            // `owner` is the FD-bearing predicate (declared functional below):
            // one owner per subject in the base, which a delta may violate.
            atoms.push(ground("owner", &[ent(i), ent(j)]));
        }
    }
    // Facts asserted straight onto intensional predicates, so the delta can
    // land extensional rows on a rule head as well as on pure EDB.
    for i in 0..ENTITIES {
        for j in 0..ENTITIES {
            atoms.push(ground("reach", &[ent(i), ent(j)]));
        }
    }
    atoms.extend(high_atom_universe());
    atoms
}

/// The atoms whose predicates sit **high in the dependency order** — little or
/// nothing reads them, so adding one leaves most strata untouched.
///
/// This split exists because of a measurement, not a hunch. Drawing deltas
/// uniformly from the full universe put 148 of 192 ordering-checks on
/// predicates at the *bottom* of the order (`edge`, `tie`, `mark`), and a delta
/// there transitively affects every level: reuse takes its path, skips nothing,
/// and degenerates into a full recompute with extra bookkeeping. The parity
/// assertions still passed — which is the point, and the danger. The oracle was
/// exercising the reuse *machinery* while barely exercising reuse. Biasing half
/// the delta draws here is what keeps `live_reuse` meaningful; the other half
/// still comes from the bottom, because that is where the negation-repoisoning
/// and defeater-growth coverage comes from.
fn high_atom_universe() -> Vec<GroundAtom> {
    let mut atoms = Vec::new();
    for i in 0..ENTITIES {
        atoms.push(ground("safe", &[ent(i)]));
        atoms.push(ground("rival", &[ent(i)]));
        atoms.push(ground("crown", &[ent(i)]));
        atoms.push(ground("heir", &[ent(i)]));
    }
    atoms
}

/// One generated trial: a base program (unsaturated) plus the additions that
/// form its delta.
struct Trial {
    base: World,
    delta: Vec<Addition>,
    aliased: bool,
}

fn generate(seed: u64) -> Trial {
    let mut rng = Lcg::new(seed);
    let mut base = World::new();

    // Intern every event variable up front, in order, so `Atom(i)` is stable and
    // no later addition can renumber the interner.
    for i in 0..EVENT_VARS {
        base.interner.intern(&format!("v{i}"));
    }
    // The two decision variables are ruled true: a cited decision reads certain.
    for i in 0..DECISIONS {
        base.valuation.set(
            Atom(u32::try_from(EVENT_VARS - DECISIONS + i).unwrap()),
            true,
        );
    }
    // An exclusivity family over two hypotheses, so `Theory::respects` filters
    // real monomials out of the negation poison.
    base.theory.add_exclusive([Atom(0), Atom(1)]);
    // The declared functional dependency (carried, not derived).
    base.functional_predicates.insert("owner".to_owned());

    skeleton_rules(&mut base, &mut rng);

    // A random slice of the optional rules starts in the base; the rest are
    // candidates for the delta.
    let mut in_base = [false; OPTIONAL_RULES];
    for slot in &mut in_base {
        *slot = rng.chance(6);
    }
    for (which, present) in in_base.iter().enumerate() {
        if *present {
            optional_rule(which).apply(&mut base);
        }
    }

    // Base facts: a random subset of the universe, each with its own annotation
    // and grade. `owner` is kept single-valued here so the FD holds in the base.
    let universe = atom_universe();
    let high = high_atom_universe();
    let mut used: BTreeSet<GroundAtom> = BTreeSet::new();
    let mut owned: BTreeSet<String> = BTreeSet::new();
    for atom in &universe {
        if !rng.chance(6) {
            continue;
        }
        if atom.pred == "owner" && !owned.insert(atom.args[0].clone()) {
            continue;
        }
        used.insert(atom.clone());
        let ann = random_ann(&mut rng);
        let grade = random_grade(&mut rng);
        base.assert_fact_graded(atom.clone(), ann, grade);
    }

    // Identity: about half the seeds declare `same-as` links, which synthesizes
    // the closure rules over every predicate in the census.
    let aliased = rng.chance(8);
    if aliased {
        let pairs = 1 + rng.below(2);
        for _ in 0..pairs {
            let left = ent(rng.below(ENTITIES));
            let right = ent(rng.below(ENTITIES));
            if left != right {
                base.assert_same_as(left, right, random_ann(&mut rng));
            }
        }
        synthesize_alias_rules(&mut base);
    }

    // The delta: one to four additions.
    let mut delta = Vec::new();
    let count = 1 + rng.below(4);
    for _ in 0..count {
        // An added rule, but only one the base does not already carry.
        if rng.chance(4) {
            let which = rng.below(OPTIONAL_RULES);
            if !in_base[which] {
                in_base[which] = true;
                delta.push(optional_rule(which));
                continue;
            }
        }
        // A new `same-as:link` row: the delta changes the identity closure
        // itself. Only meaningful once the closure rules exist.
        if aliased && rng.chance(10) {
            let left = ent(rng.below(ENTITIES));
            let right = ent(rng.below(ENTITIES));
            let atom = ground("same-as:link", &[left, right]);
            if used.insert(atom.clone()) {
                delta.push(Addition::Fact {
                    atom,
                    ann: random_ann(&mut rng),
                    grade: random_grade(&mut rng),
                });
                continue;
            }
        }
        // Half the draws come from high in the dependency order, so reuse has
        // strata left to skip; the rest from the whole universe, so the delta
        // still lands at the bottom often enough to drive poison across strata.
        // See `high_atom_universe` for why this split is not cosmetic.
        let pool: &[GroundAtom] = if rng.chance(8) { &high } else { &universe };
        for _ in 0..8 {
            let atom = pool[rng.below(pool.len())].clone();
            if used.insert(atom.clone()) {
                delta.push(Addition::Fact {
                    atom,
                    ann: random_ann(&mut rng),
                    grade: random_grade(&mut rng),
                });
                break;
            }
        }
    }

    Trial {
        base,
        delta,
        aliased,
    }
}

// ---------------------------------------------------------------------------
// Comparison helpers
// ---------------------------------------------------------------------------

/// Canonical bytes for a world: `serde_json` emits struct fields in declaration
/// order and every map in this type is a `BTreeMap`, so the encoding is
/// deterministic and byte-comparison is the strongest available identity check —
/// strictly stronger than `World`'s structural `PartialEq`, since it also sees
/// the skipped-when-empty fields.
fn canonical(world: &World) -> Vec<u8> {
    serde_json::to_vec(world).unwrap()
}

/// The saturation half of a world, keyed by atom — what must agree across
/// orderings of the same delta (the `facts` vector itself does not, because the
/// ordering *is* the difference).
fn rows(world: &World) -> BTreeMap<GroundAtom, Derived> {
    world
        .derived
        .iter()
        .map(|row| (row.atom.clone(), row.clone()))
        .collect()
}

/// A copy of `prior` with one **reused** row falsified — a row whose predicate
/// the delta does not affect, so the incremental path copies it forward verbatim
/// instead of re-deriving it. `None` when the delta affects everything and there
/// is nothing to reuse.
///
/// The falsification is a grade change, chosen because it survives
/// `looks_saturated`: that screen checks fact-atom presence and support
/// entailment, neither of which grade touches. A corruption the preconditions
/// rejected would prove nothing — the call would fall back and the result would
/// match for the wrong reason.
fn corrupt_a_reused_row(prior: &World, report: &ReuseReport) -> Option<World> {
    let mut corrupt = prior.clone();
    let victim = corrupt
        .derived
        .iter_mut()
        .find(|row| !report.affected.contains(&row.atom.pred))?;
    victim.grade = if victim.grade == Grade::Checked {
        Grade::Asserted
    } else {
        Grade::Checked
    };
    Some(corrupt)
}

/// The positive control for one ordering-check: prove the reuse path actually
/// **reused** something, rather than quietly falling back.
///
/// Every equality assertion in the oracle passes trivially if
/// `saturate_incremental` fell back to `saturate_with` on every case — it would
/// then be comparing a full recompute against itself, green and worthless, with
/// nothing in the output to say so. This falsifies a row the delta does not
/// affect and demands the result change. If the row was reused, it must; if the
/// call had fallen back, the corruption is ignored and this fires.
///
/// Returns whether a control was actually run (there is nothing to corrupt when
/// the delta affects every predicate).
fn prove_reuse_was_live(
    program: &World,
    prior: &World,
    strata: &Strata,
    full: &World,
    report: &ReuseReport,
    seed: u64,
    index: usize,
) -> bool {
    let Some(corrupt) = corrupt_a_reused_row(prior, report) else {
        return false;
    };
    let poisoned = program.saturate_incremental(&corrupt, strata).unwrap();
    assert_ne!(
        &poisoned, full,
        "seed {seed} ordering {index}: corrupting a reused prior row changed nothing — \
         the reuse path did not actually reuse it"
    );
    true
}

/// Apply a delta ordering to the base program and return the program.
fn program_with(base: &World, delta: &[Addition]) -> World {
    let mut program = base.clone();
    for addition in delta {
        addition.apply(&mut program);
    }
    program
}

/// The strata level index of each predicate; predicates that head no rule (pure
/// extensional ones) map to `None`.
fn level_of(strata: &Strata) -> BTreeMap<String, usize> {
    let mut index = BTreeMap::new();
    for (i, level) in strata.levels.iter().enumerate() {
        for pred in level {
            index.insert(pred.clone(), i);
        }
    }
    index
}

// ---------------------------------------------------------------------------
// Coverage
// ---------------------------------------------------------------------------

/// Trial counts for the situations this oracle exists to cover. Asserted against
/// floors so generator drift is a red test, not a quietly weaker suite.
#[derive(Debug, Default)]
struct Coverage {
    trials: usize,
    /// (a) A delta re-poisoned an atom that already existed: some row kept in
    /// both saturations came out of the delta carrying different defeater
    /// conditions. This is negation crossing a stratum boundary — the case a
    /// retained row would swallow.
    repoison: usize,
    /// (b) Defeater-only growth: a row whose support is byte-identical before
    /// and after, whose *conditions* moved. The `merge_into` re-enqueue gate used
    /// to be blind to exactly this.
    defeater_only: usize,
    /// (c) The identity closure itself changed: `same-as` / `same-as:link` rows
    /// were added or re-supported by the delta.
    same_as_closure: usize,
    /// (d) The delta's effect spanned two or more strata levels.
    multi_stratum: usize,
}

fn score(coverage: &mut Coverage, before: &World, after: &World, strata: &Strata) {
    coverage.trials += 1;
    let (prior, next) = (rows(before), rows(after));
    let levels = level_of(strata);
    let mut touched_levels: BTreeSet<Option<usize>> = BTreeSet::new();
    let mut same_as_moved = false;
    let mut repoisoned = false;
    let mut defeater_only = false;

    for (atom, row) in &next {
        let changed = match prior.get(atom) {
            None => true,
            Some(was) => was != row,
        };
        if !changed {
            continue;
        }
        touched_levels.insert(levels.get(&atom.pred).copied());
        if atom.pred == "same-as" || atom.pred == "same-as:link" {
            same_as_moved = true;
        }
        if let Some(was) = prior.get(atom)
            && was.conditions != row.conditions
        {
            repoisoned = true;
            defeater_only |= was.support == row.support;
        }
    }

    coverage.repoison += usize::from(repoisoned);
    coverage.defeater_only += usize::from(defeater_only);
    coverage.same_as_closure += usize::from(same_as_moved);
    coverage.multi_stratum += usize::from(touched_levels.len() >= 2);
}

// ---------------------------------------------------------------------------
// The oracle
// ---------------------------------------------------------------------------

/// The floors that keep this file from going quietly vacuous, in one place:
/// how much of the delta space was covered, and how much reuse was genuinely
/// live. Split out from the oracle body so the assertions read as a checklist
/// rather than as a tail on a long loop.
fn assert_floors(
    coverage: &Coverage,
    checked_orderings: usize,
    live_reuse: usize,
    positive_controls: usize,
) {
    assert_eq!(
        checked_orderings,
        usize::try_from(SEEDS).unwrap() * ORDERINGS,
        "every seed must contribute every ordering"
    );

    // THE ANTI-VACUITY FLOOR. Every parity assertion above is satisfiable by a
    // reuse path that never reuses anything, so this is the assertion that makes
    // the rest of the file mean something. `live_reuse` counts ordering-checks
    // where reuse ran AND skipped at least one stratum; `positive_controls`
    // counts those where corrupting a reused row provably changed the result.
    // Observed for this generator: **112 live of 192** ordering-checks, all 112
    // with a passing positive control. The floor sits well under that but far
    // above the 44 the generator produced before its delta draws were rebalanced
    // (see `high_atom_universe`), so it discriminates both failures: reuse
    // silently ceasing to engage, and the generator drifting back to deltas at
    // the bottom of the dependency order that leave nothing to skip. Either way
    // this goes red while every equality assertion in the file still passes.
    assert!(
        live_reuse >= 70,
        "anti-vacuity floor: only {live_reuse} of {checked_orderings} ordering-checks actually \
         reused a stratum — a parity oracle that always falls back proves nothing"
    );
    assert_eq!(
        positive_controls, live_reuse,
        "every live reuse must be provable by corrupting a reused row: {positive_controls} of \
         {live_reuse} were"
    );

    // Floors, tied to the generator config above: `SEEDS = 48` programs over
    // `ENTITIES = 3` entities, half of them (`rng.chance(8)`) declaring identity
    // links, each with a 1..4 addition delta drawn from `atom_universe()`. The
    // counts this configuration actually produces are repoison 32, defeater-only
    // 16, same-as 14, multi-stratum 42 of 48 trials. The floors sit at roughly
    // two thirds of those: low enough that benign churn in the generator does not
    // fail the build, high enough that a case going uncovered does. A count under
    // a floor means the generator drifted off the behaviour it exists to
    // exercise — fix the generator, do not lower the floor.
    assert_eq!(coverage.trials, usize::try_from(SEEDS).unwrap());
    assert!(
        coverage.repoison >= 18,
        "coverage floor: negation re-poisoning across strata, got {} of {} trials",
        coverage.repoison,
        coverage.trials
    );
    assert!(
        coverage.defeater_only >= 9,
        "coverage floor: defeater-only growth (support unchanged, conditions moved), got {} of {} trials",
        coverage.defeater_only,
        coverage.trials
    );
    assert!(
        coverage.same_as_closure >= 9,
        "coverage floor: same-as closure change, got {} of {} trials",
        coverage.same_as_closure,
        coverage.trials
    );
    assert!(
        coverage.multi_stratum >= 28,
        "coverage floor: multi-stratum deltas, got {} of {} trials",
        coverage.multi_stratum,
        coverage.trials
    );
}

/// How many generated programs the oracle runs.
const SEEDS: u64 = 48;
/// How many orderings of each delta are checked. The failure mode is divergence
/// under *some* ordering, so one is never enough.
const ORDERINGS: usize = 4;

#[test]
fn incremental_saturation_is_byte_identical_to_full_recompute() {
    let mut coverage = Coverage::default();
    let mut checked_orderings = 0usize;
    let mut live_reuse = 0usize;
    let mut positive_controls = 0usize;

    for seed in 0..SEEDS {
        let trial = generate(seed);
        assert!(
            !trial.delta.is_empty(),
            "seed {seed}: generated an empty delta"
        );
        let base_saturated = trial.base.fixpoint().unwrap();

        let mut rng = Lcg::new(seed ^ 0xDEAD_BEEF);
        let mut orderings: Vec<Vec<Addition>> = Vec::new();
        orderings.push(trial.delta.clone());
        for _ in 1..ORDERINGS {
            let mut permuted = trial.delta.clone();
            rng.shuffle(&mut permuted);
            orderings.push(permuted);
        }

        let mut reference: Option<(BTreeMap<GroundAtom, Derived>, World)> = None;
        for (index, ordering) in orderings.iter().enumerate() {
            let program = program_with(&trial.base, ordering);
            let strata = program.stratify().unwrap();
            let full = program.saturate_with(&strata).unwrap();
            let (incremental, report) = program
                .saturate_incremental_reported(&base_saturated, &strata)
                .unwrap();

            assert_eq!(
                incremental, full,
                "seed {seed} ordering {index}: incremental saturation diverged from full recompute"
            );
            assert_eq!(
                canonical(&incremental),
                canonical(&full),
                "seed {seed} ordering {index}: incremental saturation is not byte-identical to full recompute"
            );
            checked_orderings += 1;
            if report.is_live() {
                live_reuse += 1;
                positive_controls += usize::from(prove_reuse_was_live(
                    &program,
                    &base_saturated,
                    &strata,
                    &full,
                    &report,
                    seed,
                    index,
                ));
            }

            // Confluence: every ordering of the same delta reaches the same
            // saturation. Only the saturation is compared — the `facts` vector
            // legitimately differs, since its order is the permutation itself.
            match &reference {
                None => {
                    score(&mut coverage, &base_saturated, &full, &strata);
                    reference = Some((rows(&full), full));
                }
                Some((reference_rows, reference_world)) => {
                    assert_eq!(
                        rows(&full),
                        *reference_rows,
                        "seed {seed} ordering {index}: full recompute is not confluent across delta orderings"
                    );
                    assert_eq!(
                        full.support_index, reference_world.support_index,
                        "seed {seed} ordering {index}: support index diverged across delta orderings"
                    );
                    assert_eq!(
                        full.var_grades, reference_world.var_grades,
                        "seed {seed} ordering {index}: var grades diverged across delta orderings"
                    );
                }
            }
        }
    }

    assert_floors(&coverage, checked_orderings, live_reuse, positive_controls);
}

/// The generator must actually build the world it claims to: at least three
/// strata, stratified negation, an identity closure, and an FD-bearing
/// predicate. Without this, the oracle above could pass over programs too thin
/// to test anything.
#[test]
fn the_generator_builds_the_world_it_claims() {
    let mut deepest = 0;
    let mut with_alias = 0;
    let mut negation_seen = false;
    for seed in 0..SEEDS {
        let trial = generate(seed);
        let strata = trial.base.stratify().unwrap();
        deepest = deepest.max(strata.levels.len());
        with_alias += usize::from(trial.aliased);
        negation_seen |= trial
            .base
            .rules
            .iter()
            .any(|rule| rule.body.iter().any(|lit| lit.negated));
        assert!(
            trial.base.functional_predicates.contains("owner"),
            "seed {seed}: the FD-bearing predicate is not declared"
        );
        // Stratified negation must survive every generated rule set.
        let saturated = trial.base.fixpoint().unwrap();
        assert!(
            !saturated.derived.is_empty() || trial.base.facts.is_empty(),
            "seed {seed}: a world with facts saturated to nothing"
        );
    }
    assert!(
        deepest >= 3,
        "generated programs are only {deepest} strata deep"
    );
    assert!(negation_seen, "no generated rule set used negation");
    assert!(with_alias >= 6, "only {with_alias} seeds declared identity");
}

// ---------------------------------------------------------------------------
// The two traps, hand-built
// ---------------------------------------------------------------------------

fn hyp(i: u32) -> PosBool {
    PosBool::pos(Atom(i))
}

fn mono(i: u32) -> Monomial {
    hyp(i).monomials().next().unwrap().clone()
}

/// Trap 1 — **defeater-only growth must leave the row it lands on.**
///
/// `p` and `q` are one recursive clique, so a change to `p` has to reach `q`
/// inside the stratum. `p(a)` is seeded already conditioned on `v2`, and a
/// second derivation reaches the same support monomial conditioned on `v1`
/// instead; merging them conjoins the defeaters to `v1·v2`, which is a strictly
/// weaker condition and a **real change to the row** — with the support
/// monomials byte-identical throughout. Gating the re-enqueue on support alone
/// stranded that change: `q` kept the stale `v2` and read defeated in worlds
/// where the derivation is clean.
#[test]
fn defeater_only_growth_propagates_inside_a_stratum() {
    let mut world = World::new();
    for i in 0..3 {
        world.interner.intern(&format!("v{i}"));
    }
    // `bad1(a)` rests on v1, so `not bad1` poisons with v1.
    world.assert_fact(ground("b1", &[ent(0)]), Ann::supported(hyp(1)));
    // The seeded `p(a)`: support v0, already conditioned on v2.
    let mut conditions = DefeaterConditions::new();
    conditions.set(mono(0), hyp(2));
    world.assert_fact(ground("p", &[ent(0)]), Ann::conditioned(hyp(0), conditions));
    world.assert_fact(ground("z", &[ent(0)]), Ann::supported(hyp(0)));
    world
        .define_rule(
            "bad1-b1",
            pattern("bad1", &["X"]),
            vec![BodyLiteral::pos(pattern("b1", &["X"]))],
        )
        .unwrap();
    // The second route to p(a)'s support monomial, poisoned by v1 instead of v2.
    world
        .define_rule(
            "p-z",
            pattern("p", &["X"]),
            vec![
                BodyLiteral::pos(pattern("z", &["X"])),
                BodyLiteral::not(pattern("bad1", &["X"])),
            ],
        )
        .unwrap();
    // The clique: q consumes p, p consumes q.
    world
        .define_rule(
            "q-p",
            pattern("q", &["X"]),
            vec![BodyLiteral::pos(pattern("p", &["X"]))],
        )
        .unwrap();
    world
        .define_rule(
            "p-q",
            pattern("p", &["X"]),
            vec![BodyLiteral::pos(pattern("q", &["X"]))],
        )
        .unwrap();

    let strata = world.stratify().unwrap();
    assert!(
        strata.levels.iter().any(|level| level.len() > 1),
        "p and q must share a stratum for this trap to bite: {strata:?}"
    );
    let saturated = world.fixpoint().unwrap();
    let row = |world: &World, pred: &str| {
        world
            .derived
            .iter()
            .find(|row| row.atom.pred == pred)
            .unwrap_or_else(|| panic!("{pred} was never derived"))
            .clone()
    };

    let p = row(&saturated, "p");
    let q = row(&saturated, "q");
    let expected = hyp(1).and_(&hyp(2));
    assert_eq!(
        p.conditions.defeater_of(&mono(0)),
        &expected,
        "p(e0) must carry both routes' defeaters conjoined"
    );
    assert_eq!(
        q.conditions.defeater_of(&mono(0)),
        &expected,
        "q(e0) must see p's weakened defeater, not the stale one it was first derived with"
    );
    assert_eq!(
        p.support, q.support,
        "the support never moved; only the poison did"
    );
}

/// Trap 2 — **a delta must be able to poison a stratum above it.**
///
/// `h(a)` is derived clean while `bad` is underivable. The delta makes `bad(a)`
/// derivable, which adds a defeater to `h(a)` **without touching its support**.
/// Retaining `h`'s prior row and folding the re-derivation in would compute
/// `FALSE ∧ (FALSE ∨ v1) = FALSE` and lose the poison outright — the atom would
/// keep reading certain where a full recompute defeats it. This pins the
/// incremental path to the full one on exactly that shape.
#[test]
fn a_delta_poisons_the_stratum_above_it() {
    let mut base = World::new();
    for i in 0..2 {
        base.interner.intern(&format!("v{i}"));
    }
    base.assert_fact(ground("x", &[ent(0)]), Ann::unconditional());
    base.define_rule(
        "p-x",
        pattern("p", &["X"]),
        vec![BodyLiteral::pos(pattern("x", &["X"]))],
    )
    .unwrap();
    base.define_rule(
        "bad-b",
        pattern("bad", &["X"]),
        vec![BodyLiteral::pos(pattern("b", &["X"]))],
    )
    .unwrap();
    base.define_rule(
        "h-p",
        pattern("h", &["X"]),
        vec![
            BodyLiteral::pos(pattern("p", &["X"])),
            BodyLiteral::not(pattern("bad", &["X"])),
        ],
    )
    .unwrap();

    let base_saturated = base.fixpoint().unwrap();
    let find = |world: &World, pred: &str| {
        world
            .derived
            .iter()
            .find(|row| row.atom.pred == pred)
            .cloned()
    };
    let before = find(&base_saturated, "h").expect("h(e0) is derived in the base");
    assert!(
        before.conditions.is_empty(),
        "h(e0) starts clean: nothing derives bad yet"
    );

    // The delta: `b(e0)` on hypothesis v1, which makes `bad(e0)` derivable.
    let mut program = base.clone();
    program.derived.clear();
    program.support_index.clear();
    program.assert_fact(ground("b", &[ent(0)]), Ann::supported(hyp(1)));

    let strata = program.stratify().unwrap();
    let full = program.saturate_with(&strata).unwrap();
    let incremental = program
        .saturate_incremental(&base_saturated, &strata)
        .unwrap();

    let after = find(&full, "h").expect("h(e0) survives the delta");
    assert_eq!(
        after.support, before.support,
        "the delta must move only the poison, not the support"
    );
    assert!(
        !after.conditions.is_empty(),
        "the newly derivable bad(e0) must poison h(e0)"
    );
    assert_eq!(
        incremental, full,
        "incremental saturation lost the delta's poison"
    );
    assert_eq!(
        canonical(&incremental),
        canonical(&full),
        "incremental saturation is not byte-identical on the poison-propagation shape"
    );
}

/// The reuse preconditions are refusals, not silent wrong answers: a prior that
/// does not extend into `self`, whose theory differs, or that was never
/// saturated, falls back to the full fixpoint and still returns the right world.
///
/// **Scope:** these are the three refusals exercised here, on one generated
/// program — not a general claim that every unusable prior is caught. The
/// preconditions are a screen with known gaps; `looks_saturated`'s note states
/// them, and `a_saturated_then_appended_prior_is_refused` /
/// `a_never_saturated_prior_with_a_ground_rule_is_refused` cover the two that
/// were reachable in release.
#[test]
fn an_unusable_prior_falls_back_to_the_full_fixpoint() {
    let trial = generate(7);
    let program = program_with(&trial.base, &trial.delta);
    let strata = program.stratify().unwrap();
    let full = program.saturate_with(&strata).unwrap();

    // An unsaturated prior: the program itself, with no `derived`.
    assert_eq!(
        program.saturate_incremental(&trial.base, &strata).unwrap(),
        full,
        "an unsaturated prior must fall back, not reuse nothing"
    );
    // A prior that is not a prefix: the delta's own program, saturated, used as
    // the base for itself minus the delta.
    let unrelated = World::new().fixpoint().unwrap();
    assert_eq!(
        program.saturate_incremental(&unrelated, &strata).unwrap(),
        full,
        "an empty prior must fall back to a full recompute"
    );
    // A prior whose theory differs.
    let mut retheoried = trial.base.fixpoint().unwrap();
    retheoried.theory.add_exclusive([Atom(2), Atom(3)]);
    assert_eq!(
        program.saturate_incremental(&retheoried, &strata).unwrap(),
        full,
        "a prior with a different theory must fall back"
    );
}

/// One generated case set up for the corruption control: the program, its
/// strata, an honest prior, and the same prior with a **reused** row falsified.
struct CorruptionCase {
    program: World,
    strata: Strata,
    honest: World,
    corrupt: World,
}

/// Build a corruption control from the first seed that leaves anything reusable
/// — a prior that is a legitimate reuse base by every structural precondition,
/// with one row the delta does not affect falsified. Reuse copies that row
/// forward verbatim, exactly as designed, so this is the shape the
/// verify-against-full mode exists to catch.
///
/// **Not seed-pinned, deliberately.** An earlier version hard-coded a seed and
/// corrupted every row; rebalancing the generator's delta draws then made that
/// seed's delta affect every predicate, so the corruption was dropped along with
/// the rows and the control silently stopped controlling anything. Scanning for
/// a seed with a genuinely reused row keeps the test about the verifier rather
/// than about one seed's shape — and the `panic!` at the end means "no seed
/// reuses anything" fails loudly instead of passing vacuously.
fn corruption_case() -> CorruptionCase {
    for seed in 0..SEEDS {
        let trial = generate(seed);
        let program = program_with(&trial.base, &trial.delta);
        let strata = program.stratify().unwrap();
        let honest = trial.base.fixpoint().unwrap();
        let (_, report) = program
            .saturate_incremental_reported(&honest, &strata)
            .unwrap();
        if let Some(corrupt) = corrupt_a_reused_row(&honest, &report) {
            return CorruptionCase {
                program,
                strata,
                honest,
                corrupt,
            };
        }
    }
    panic!("no generated seed leaves a row reusable — the corruption control cannot be built");
}

/// A prior that was saturated and then **appended to** passes every structural
/// precondition — its `derived` is non-empty, its facts extend cleanly into the
/// new program — while carrying rows that no longer reflect its own facts. It is
/// not a hypothetical shape: it is what a cache holds the moment something adds
/// a fact without re-running the fixpoint, which is precisely what PR3 builds.
///
/// Here `a(e1)` is appended after saturation, so the prior has no `p(e1)` row.
/// The delta then touches only `b`, leaving `p`'s stratum unaffected and
/// therefore **skipped** — so without a screen the reused saturation silently
/// lacks `p(e1)` that a full recompute derives. `looks_saturated` must refuse
/// this prior and fall back.
#[test]
fn a_saturated_then_appended_prior_is_refused() {
    let mut base = World::new();
    base.assert_fact(ground("a", &[ent(0)]), Ann::unconditional());
    base.define_rule(
        "p-a",
        pattern("p", &["X"]),
        vec![BodyLiteral::pos(pattern("a", &["X"]))],
    )
    .unwrap();
    base.define_rule(
        "q-b",
        pattern("q", &["X"]),
        vec![BodyLiteral::pos(pattern("b", &["X"]))],
    )
    .unwrap();

    // Saturate, THEN append — the row for `a(e1)` (and so `p(e1)`) never exists.
    let mut stale = base.fixpoint().unwrap();
    stale.assert_fact(ground("a", &[ent(1)]), Ann::unconditional());
    assert!(
        !stale
            .derived
            .iter()
            .any(|row| row.atom.pred == "p" && row.atom.args == vec![ent(1)]),
        "the stale prior must be missing p(e1) — that is the whole shape"
    );

    // The delta touches `b` only, so `p`'s stratum is unaffected and skippable.
    let mut program = stale.clone();
    program.derived.clear();
    program.support_index.clear();
    program.assert_fact(ground("b", &[ent(0)]), Ann::unconditional());

    let strata = program.stratify().unwrap();
    let full = program.saturate_with(&strata).unwrap();
    let (incremental, report) = program
        .saturate_incremental_reported(&stale, &strata)
        .unwrap();

    assert!(
        !report.reused,
        "a saturated-then-appended prior must be refused, not reused: {report:?}"
    );
    assert_eq!(
        incremental, full,
        "the refusal must fall back to a correct full recompute"
    );
    assert!(
        full.derived
            .iter()
            .any(|row| row.atom.pred == "p" && row.atom.args == vec![ent(1)]),
        "the full recompute derives p(e1) — the row the stale prior would have dropped"
    );
}

/// A prior with rules but no rows at all is refused too. A pure-negative-body
/// rule with a ground head derives without any fact, so an empty `derived` is
/// indistinguishable from a fixpoint that never ran, and reuse would skip the
/// stratum that should have produced the row.
#[test]
fn a_never_saturated_prior_with_a_ground_rule_is_refused() {
    let mut base = World::new();
    base.define_rule(
        "p-unless-q",
        AtomExpr {
            pred: Ident::synthetic("p"),
            args: vec![Term::Sym(Ident::synthetic("e0"))],
        },
        vec![BodyLiteral::not(AtomExpr {
            pred: Ident::synthetic("q"),
            args: vec![Term::Sym(Ident::synthetic("e0"))],
        })],
    )
    .unwrap();

    let never_saturated = base.clone(); // no `derived`, no facts, but a live rule
    let mut program = base.clone();
    program.assert_fact(ground("r", &[ent(0)]), Ann::unconditional());

    let strata = program.stratify().unwrap();
    let full = program.saturate_with(&strata).unwrap();
    let (incremental, report) = program
        .saturate_incremental_reported(&never_saturated, &strata)
        .unwrap();

    assert!(
        !report.reused,
        "a rules-but-no-rows prior must be refused: {report:?}"
    );
    assert_eq!(incremental, full, "the refusal must fall back correctly");
    assert!(
        full.derived.iter().any(|row| row.atom.pred == "p"),
        "the ground pure-negative rule really does derive p(e0) from nothing"
    );
}

/// An added rule that **merges two previously separate strongly-connected
/// components** into one. The affected-set closure and the level skipping are
/// both computed against the *new* stratification, so a delta that reshapes the
/// component graph is the case where reusing rows computed under the old shape
/// could go wrong.
#[test]
fn a_delta_rule_that_merges_two_sccs_stays_byte_identical() {
    let mut base = World::new();
    base.assert_fact(ground("a", &[ent(0)]), Ann::unconditional());
    base.assert_fact(ground("b", &[ent(1)]), Ann::supported(hyp(0)));
    base.interner.intern("v0");
    base.define_rule(
        "p-a",
        pattern("p", &["X"]),
        vec![BodyLiteral::pos(pattern("a", &["X"]))],
    )
    .unwrap();
    base.define_rule(
        "q-b",
        pattern("q", &["X"]),
        vec![BodyLiteral::pos(pattern("b", &["X"]))],
    )
    .unwrap();
    base.define_rule(
        "p-q",
        pattern("p", &["X"]),
        vec![BodyLiteral::pos(pattern("q", &["X"]))],
    )
    .unwrap();

    let before = base.stratify().unwrap();
    let level_of_p = before
        .levels
        .iter()
        .position(|level| level.contains(&"p".to_owned()));
    let level_of_q = before
        .levels
        .iter()
        .position(|level| level.contains(&"q".to_owned()));
    assert_ne!(
        level_of_p, level_of_q,
        "p and q must start in separate components for this to be the merging case"
    );

    let base_saturated = base.fixpoint().unwrap();

    // `q(X) :- p(X)` closes the cycle: {p} and {q} become one component.
    let mut program = base.clone();
    program.derived.clear();
    program.support_index.clear();
    program
        .define_rule(
            "q-p",
            pattern("q", &["X"]),
            vec![BodyLiteral::pos(pattern("p", &["X"]))],
        )
        .unwrap();

    let strata = program.stratify().unwrap();
    assert!(
        strata
            .levels
            .iter()
            .any(|level| level.contains(&"p".to_owned()) && level.contains(&"q".to_owned())),
        "the added rule must have merged p and q into one component: {strata:?}"
    );

    let full = program.saturate_with(&strata).unwrap();
    let incremental = program
        .saturate_incremental(&base_saturated, &strata)
        .unwrap();
    assert_eq!(
        incremental, full,
        "a delta that merges two SCCs must still reproduce the full recompute"
    );
    assert_eq!(
        canonical(&incremental),
        canonical(&full),
        "SCC-merging delta is not byte-identical"
    );
}

/// The verify-against-full mode has to be able to **fail**, or it is decoration:
/// a guard that never fires is indistinguishable from no guard.
///
/// This checks the value-level half, [`incremental_divergence`] — that it stays
/// silent on an honest prior, reports on a corrupted one, and that the
/// corruption genuinely reaches the unverified primitive's result (otherwise
/// catching it would prove nothing).
#[test]
fn the_verifier_reports_a_prior_whose_saturation_is_wrong() {
    let case = corruption_case();
    let full = case.program.saturate_with(&case.strata).unwrap();

    let reused = case
        .program
        .saturate_incremental(&case.honest, &case.strata)
        .unwrap();
    assert_eq!(
        incremental_divergence(&reused, &full),
        None,
        "an honest prior must report no divergence"
    );
    assert_eq!(
        case.program
            .saturate_incremental_verified(&case.honest, &case.strata)
            .unwrap(),
        full,
        "an honest prior must survive the asserting form"
    );

    let corrupted_reuse = case
        .program
        .saturate_incremental(&case.corrupt, &case.strata)
        .unwrap();
    assert_ne!(
        corrupted_reuse, full,
        "the corruption must actually reach the reused saturation"
    );
    let report = incremental_divergence(&corrupted_reuse, &full)
        .expect("a corrupted prior must be reported, not copied forward");
    // The report names the first atom that disagrees in atom order, which may be
    // a *consequence* of the injected fault rather than the injected row itself —
    // it says what diverged, not what caused it.
    assert!(
        report.contains("disagrees") || report.contains("derived"),
        "the divergence must name an atom, got: {report}"
    );
}

/// The asserting half: [`World::saturate_incremental_verified`] panics rather
/// than returning a divergence, because there is no recovering from a saturation
/// that does not reproduce its own full recompute.
#[test]
#[should_panic(expected = "incremental saturation diverged from a full recompute")]
fn the_verifier_panics_on_a_prior_whose_saturation_is_wrong() {
    let case = corruption_case();
    let _ = case
        .program
        .saturate_incremental_verified(&case.corrupt, &case.strata);
}
