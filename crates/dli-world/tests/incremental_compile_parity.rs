#![allow(clippy::unwrap_used, clippy::expect_used)]
// The corpus builders below return `Option<Annotation>` on purpose: every AST
// `provenance` field is one, and keeping the shapes interchangeable with
// `random_provenance` — which genuinely has a `None` arm, the claim-from-nowhere
// case — is what lets a call site vary its provenance without varying its shape.
#![allow(clippy::unnecessary_wraps)]
//! The permanent incremental-**compile** parity oracle (DL-0087, staged by
//! DL-0088).
//!
//! [`compile_incremental`] extends a prior compile instead of redoing it: it
//! reuses the loader's per-declaration lowering and the evaluator's prior
//! saturation. The claim that buys is **byte-identity** — for the union pack it
//! must produce the world [`compile`] produces, down to the serialized bytes,
//! not "the same answers" and not "the same rows modulo ordering". This file is
//! the standing proof of that claim, and it is gate-resident on purpose: an
//! incremental compiler that silently diverges from its own full recompile is
//! worse than no incrementality at all, because every verdict downstream of it
//! stops being replayable.
//!
//! It is the compile-level twin of `dli-eval/tests/incremental_parity.rs`, and
//! it inherits that file's two disciplines.
//!
//! **Never trust one ordering.** The named failure mode is divergence under
//! *some* arrangement of the delta, so every trial permutes its delta documents
//! several ways. Each ordering is checked against its own full compile
//! (byte-identity), and all orderings are checked against each other over the
//! order-independent projection (confluence) — `atoms`, `facts`, and `sorts` are
//! *sequences in document order*, so those legitimately differ between
//! orderings and the projection excludes them.
//!
//! **Never let the oracle go vacuous.** Every equality assertion here is
//! satisfied by an implementation that reuses nothing and recompiles everything,
//! which would be green and worthless with nothing in the output to say so. Two
//! positive controls close that: one falsifies an artifact the *loader* carries
//! forward and demands the result change, the other falsifies a saturated row
//! the *fixpoint* copies forward and demands the same. Both are counted, both
//! are floored, and the floors are thresholds rather than `> 0`.
//!
//! Generation is a deterministic LCG over a fixed seed range — no clock, no
//! environment RNG — so a failure reproduces exactly from the seed printed in
//! its assertion message.
//!
//! # Why the canonical encoding is YAML and not JSON
//!
//! [`CompiledWorld`] contains `WorldModel::fluents`, a `BTreeMap` keyed by the
//! `(pred, subject)` slot **tuple**. JSON has no encoding for a non-string map
//! key, so `serde_json::to_vec` of a compiled world errors the moment a corpus
//! declares one `state` — and this generator declares several. `serde_yaml_ng`
//! (already a dependency of this crate) encodes it faithfully and
//! deterministically, every map in the type being a `BTreeMap` and every struct
//! emitting its fields in declaration order. The evaluable half is *also*
//! compared as canonical JSON, since `World` carries no tuple-keyed map and that
//! is the exact encoding the PR 1 oracle pins.

use std::collections::{BTreeMap, BTreeSet};

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{
    Decl, Document, EntityDecl, EntityKind, FactDecl, FunctionalDecl, HypothesisDecl, InertialDecl,
    KindDecl, OptionClause, QuestionDecl, RatifyDecl, RuleDecl, SameAsDecl, ScopeHeader, StateDecl,
    TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::literal::BodyLiteral;
use dli_ast::rule::{Datalog, RuleBody, RuleClause};
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_eval::world::{Derived, GroundAtom, World};

use dli_world::check::CompiledWorld;
use dli_world::incremental::{CompileReuse, IncrementalWorld};
use dli_world::pack::Pack;
use dli_world::{compile, compile_incremental, compile_resumable};

// ---------------------------------------------------------------------------
// Deterministic generation
// ---------------------------------------------------------------------------

/// A 64-bit linear congruential generator — the repo's deterministic-generator
/// idiom, identical to the one the PR 1 oracle uses. No clock, no thread RNG.
struct Lcg(u64);

impl Lcg {
    /// A generator seeded from `seed`, scrambled so adjacent seeds do not
    /// produce adjacent corpora.
    fn new(seed: u64) -> Self {
        Lcg(seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0x1234_5678_9ABC_DEF0))
    }

    /// The next raw draw.
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
// Corpus vocabulary
// ---------------------------------------------------------------------------

/// The entity universe. Small on purpose: the alias-rewrite rules synthesized
/// for every predicate make the relation grow with the square of this, and the
/// oracle must stay inside an ordinary `cargo test` run.
const ENTITIES: usize = 3;

/// The scopes (packs) a document may carry. Three, so a delta can land in a pack
/// the base never declared as well as in one it did.
const PACKS: [&str; 3] = ["realm", "march", "fen"];

/// Decision ids. `D-0001`/`D-0002` are declared in the base; `D-0003` is cited
/// by the base but declared only by a delta that chooses to (the retro-ruling
/// case the ledger valuation cannot compute incrementally); `D-0004` is cited
/// and never declared, so its `citation.unknown-decision` finding is permanent.
const DECLARED_IN_BASE: [&str; 2] = ["D-0001", "D-0002"];
/// The decision a delta may declare, retro-ruling a base citation true.
const RETRO_DECISION: &str = "D-0003";
/// The decision nothing ever declares — a permanently forged citation.
const FORGED_DECISION: &str = "D-0004";

/// Proposal bundle tags. Every `proposed` annotation the generator mints carries
/// one, so its gating variable is `h.<tag>` — a name, not the loader's
/// order-dependent anonymous `#<counter>`. That keeps the interner a function of
/// the delta's *content* rather than its arrangement, which is what makes
/// confluence across orderings a meaningful question rather than a doomed one.
const TAGS: [&str; 2] = ["tide", "guild"];

/// An entity symbol.
fn ent(i: usize) -> String {
    format!("e{i}")
}

/// A synthetic identifier.
fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

/// A ground symbol term.
fn sym(text: &str) -> Term {
    Term::Sym(id(text))
}

/// A rule-pattern atom over variables.
fn pat(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: args.iter().map(|a| Term::Var(id(a))).collect(),
    }
}

/// A ground atom expression.
fn ground(pred: &str, args: &[String]) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: args.iter().map(|a| sym(a)).collect(),
    }
}

/// A provenance annotation.
fn annotation(marker: ProvenanceMarker, cites: &[&str], tags: &[&str]) -> Annotation {
    Annotation {
        marker,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
        tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        span: Span::synthetic(),
    }
}

/// `ratified <d-id>`.
fn ratified(decision: &str) -> Option<Annotation> {
    Some(annotation(ProvenanceMarker::Ratified, &[decision], &[]))
}

/// `proposed <tag>` — gated by the named hypothesis bundle.
fn proposed(tag: &str) -> Option<Annotation> {
    Some(annotation(ProvenanceMarker::Proposed, &[], &[tag]))
}

/// One of the four provenance shapes a generated claim may carry: two ledger
/// citations, a named proposal, and (rarely) nothing at all, which the loader
/// holds fail-closed with a `load.missing-provenance` finding.
fn random_provenance(rng: &mut Lcg) -> Option<Annotation> {
    match rng.below(8) {
        0..=2 => ratified(DECLARED_IN_BASE[0]),
        3..=4 => ratified(DECLARED_IN_BASE[1]),
        5 => ratified(RETRO_DECISION),
        6 => proposed(TAGS[rng.below(TAGS.len())]),
        _ => None,
    }
}

/// A document with a scope header.
fn document(pack: &str, decls: Vec<Decl>) -> Document {
    Document {
        scope: Some(ScopeHeader {
            name: id(pack),
            span: Span::synthetic(),
        }),
        decls,
    }
}

/// A ground `fact` declaration.
fn fact(pred: &str, args: &[String], provenance: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: ground(pred, args),
        triple: false,
        provenance,
    })
}

/// A bare world `rule` carrying its typed `head :- body` datalog.
fn rule(
    name: &str,
    head: AtomExpr,
    body: Vec<BodyLiteral>,
    provenance: Option<Annotation>,
) -> Decl {
    let datalog = Datalog { head, body };
    Decl::Rule(RuleDecl {
        name: id(name),
        domain: None,
        head: Vec::new(),
        body: RuleBody {
            clauses: vec![RuleClause {
                clause: Clause {
                    text: datalog.render(),
                    span: Span::synthetic(),
                },
                datalog: Some(datalog),
            }],
            inline: None,
        },
        provenance,
    })
}

/// An entity declaration with no slots.
fn entity(
    kind: EntityKind,
    name: &str,
    time: Option<TimeClause>,
    prov: Option<Annotation>,
) -> Decl {
    Decl::Entity(EntityDecl {
        kind,
        name: id(name),
        label: None,
        time,
        slots: Vec::new(),
        provenance: prov,
    })
}

// ---------------------------------------------------------------------------
// The rule skeleton
// ---------------------------------------------------------------------------

/// The always-present rule skeleton: eight rules over six intensional
/// predicates, laying out **seven dependency strata** with stratified negation
/// crossing two of the boundaries (`safe` over `blocked`, `crown` over `rival`).
/// `reach` is recursive, so at least one stratum is a genuine clique. The shape
/// is deliberately the PR 1 oracle's, so a divergence that oracle would catch at
/// the saturation seam is one this one can catch at the compile seam.
fn skeleton_rules(rng: &mut Lcg) -> Vec<Decl> {
    let mut out = Vec::new();
    let mut add = |name: &str, head: AtomExpr, body: Vec<BodyLiteral>, rng: &mut Lcg| {
        // Rules-as-claims (WS-2): a rule's own citation multiplies into every
        // derivation through it, so some rules rest on a decision of their own.
        out.push(rule(name, head, body, random_provenance(rng)));
    };
    add(
        "reach-base",
        pat("reach", &["X", "Y"]),
        vec![BodyLiteral::pos(pat("edge", &["X", "Y"]))],
        rng,
    );
    add(
        "reach-step",
        pat("reach", &["X", "Z"]),
        vec![
            BodyLiteral::pos(pat("edge", &["X", "Y"])),
            BodyLiteral::pos(pat("reach", &["Y", "Z"])),
        ],
        rng,
    );
    add(
        "blocked-mark",
        pat("blocked", &["X"]),
        vec![BodyLiteral::pos(pat("mark", &["X"]))],
        rng,
    );
    add(
        "blocked-reach",
        pat("blocked", &["X"]),
        vec![
            BodyLiteral::pos(pat("reach", &["X", "Y"])),
            BodyLiteral::pos(pat("mark", &["Y"])),
        ],
        rng,
    );
    // Negation across a stratum boundary.
    add(
        "safe-node",
        pat("safe", &["X"]),
        vec![
            BodyLiteral::pos(pat("node", &["X"])),
            BodyLiteral::not(pat("blocked", &["X"])),
        ],
        rng,
    );
    add(
        "rival-reach",
        pat("rival", &["X"]),
        vec![
            BodyLiteral::pos(pat("reach", &["X", "Y"])),
            BodyLiteral::pos(pat("safe", &["Y"])),
        ],
        rng,
    );
    // A second negation, two strata higher: this is the one a delta down in
    // `edge`/`mark` has to re-poison all the way up.
    add(
        "crown-safe",
        pat("crown", &["X"]),
        vec![
            BodyLiteral::pos(pat("safe", &["X"])),
            BodyLiteral::not(pat("rival", &["X"])),
        ],
        rng,
    );
    add(
        "heir-crown",
        pat("heir", &["X"]),
        vec![
            BodyLiteral::pos(pat("crown", &["X"])),
            BodyLiteral::pos(pat("owner", &["X", "P"])),
        ],
        rng,
    );
    out
}

/// How many optional rules a delta may draw from.
const OPTIONAL_RULES: usize = 4;

/// The optional rules a delta may add. Each preserves stratifiability (none
/// closes a negative cycle) and each lands its head in a different part of the
/// dependency order.
fn optional_rule(which: usize, prov: Option<Annotation>) -> Decl {
    match which {
        0 => rule(
            "reach-tie",
            pat("reach", &["X", "Y"]),
            vec![BodyLiteral::pos(pat("tie", &["X", "Y"]))],
            prov,
        ),
        1 => rule(
            "blocked-self-tie",
            pat("blocked", &["X"]),
            vec![BodyLiteral::pos(pat("tie", &["X", "X"]))],
            prov,
        ),
        2 => rule(
            "heir-rival",
            pat("heir", &["X"]),
            vec![
                BodyLiteral::pos(pat("rival", &["X"])),
                BodyLiteral::pos(pat("owner", &["X", "P"])),
            ],
            prov,
        ),
        _ => rule(
            "safe-unmarked",
            pat("safe", &["X"]),
            vec![
                BodyLiteral::pos(pat("node", &["X"])),
                BodyLiteral::not(pat("mark", &["X"])),
            ],
            prov,
        ),
    }
}

// ---------------------------------------------------------------------------
// The generated corpus
// ---------------------------------------------------------------------------

/// Every ground atom the generator may assert **low** in the dependency order —
/// the extensional predicates every stratum above eventually reads.
fn low_atoms() -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for i in 0..ENTITIES {
        out.push(("node".to_owned(), vec![ent(i)]));
        out.push(("mark".to_owned(), vec![ent(i)]));
        for j in 0..ENTITIES {
            out.push(("edge".to_owned(), vec![ent(i), ent(j)]));
            out.push(("tie".to_owned(), vec![ent(i), ent(j)]));
            // `owner` is the FD-bearing predicate (declared `functional`): one
            // owner per subject in the base, which a delta may violate.
            out.push(("owner".to_owned(), vec![ent(i), ent(j)]));
        }
    }
    out
}

/// The atoms whose predicates sit **high in the dependency order** — little or
/// nothing reads them, so adding one leaves most strata untouched.
///
/// The PR 1 oracle learned this the hard way: drawing deltas uniformly put most
/// of them at the bottom of the order, where a delta transitively affects every
/// level and reuse degenerates into a full recompute with extra bookkeeping. The
/// parity assertions still passed — which is the point, and the danger. Biasing
/// part of the delta here is what keeps the live-reuse floor meaningful; the
/// rest still comes from the bottom, because that is where the negation
/// re-poisoning coverage comes from.
/// `safe` and `rival` appear twice, which is a weighting and not a mistake: a
/// draw is uniform over this vector, and those two are the only high predicates
/// a *negated* body literal reads (`crown :- safe, not rival`), so they are what
/// carries the defeater-propagation coverage. `heir` and `primed` head or feed
/// nothing above them and buy liveness rather than coverage.
fn high_atoms() -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for i in 0..ENTITIES {
        out.push(("safe".to_owned(), vec![ent(i)]));
        out.push(("safe".to_owned(), vec![ent(i)]));
        out.push(("rival".to_owned(), vec![ent(i)]));
        out.push(("rival".to_owned(), vec![ent(i)]));
        out.push(("crown".to_owned(), vec![ent(i)]));
        out.push(("heir".to_owned(), vec![ent(i)]));
        out.push(("primed".to_owned(), vec![ent(i)]));
    }
    out
}

/// One generated trial: the base documents and the delta documents that extend
/// them, plus what the generator knows it built.
///
/// The flags are a record of what this seed happened to produce, read only by
/// the coverage scorer — four of them is the point, not a smell.
#[allow(clippy::struct_excessive_bools)]
struct Trial {
    /// The base pack's documents.
    base: Vec<Document>,
    /// The delta documents, whose orderings are what the oracle permutes.
    delta: Vec<Document>,
    /// The base declares at least one `same-as` link.
    base_aliased: bool,
    /// The delta declares at least one `same-as` link.
    delta_aliased: bool,
    /// The delta declares `D-0003`, retro-ruling the base's citation of it.
    retro_ratifies: bool,
    /// How many distinct packs the delta's documents carry.
    delta_packs: usize,
    /// The delta was drawn **shallow**: only from predicates high in the
    /// dependency order, with no identity link and no rule heading a low
    /// predicate. See [`generate`] for why this split is not cosmetic.
    shallow: bool,
}

/// The base preamble: the vocabulary every later document leans on.
///
/// This document also does the interner's work up front. Every event variable a
/// delta can reach — the four decision ids and both proposal bundles — is
/// interned here, in a fixed order, so no permutation of the delta can renumber
/// the interner. That is not a convenience: support polynomials index the
/// interner by position, so a delta that interned a fresh name would make two
/// orderings of the same delta produce genuinely different (though equally
/// correct) worlds, and confluence would be untestable rather than false.
#[expect(clippy::too_many_lines)]
fn preamble() -> Document {
    let mut decls = vec![
        Decl::Ratify(RatifyDecl {
            decision: id(DECLARED_IN_BASE[0]),
            body: dli_ast::language::RatifyBody::default(),
        }),
        Decl::Ratify(RatifyDecl {
            decision: id(DECLARED_IN_BASE[1]),
            body: dli_ast::language::RatifyBody::default(),
        }),
        Decl::Kind(KindDecl {
            name: id("place"),
            provenance: ratified(DECLARED_IN_BASE[0]),
        }),
        Decl::Kind(KindDecl {
            name: id("person"),
            provenance: ratified(DECLARED_IN_BASE[1]),
        }),
    ];
    // The proposal bundles, interned by name so a `proposed <tag>` claim in any
    // later document — base or delta — gates on an already-known variable.
    for tag in TAGS {
        decls.push(Decl::Hypothesis(HypothesisDecl {
            name: id(&format!("h.{tag}")),
            statement: Some(format!("the {tag} reading holds")),
        }));
    }
    // The timeline. Eras and events are declared ONLY here: the time-point table
    // is positional too, so a delta that declared a fresh era would renumber it.
    decls.push(entity(
        EntityKind::Era,
        "age-of-stone",
        Some(TimeClause::Order(vec![id("age-of-iron")])),
        ratified(DECLARED_IN_BASE[0]),
    ));
    decls.push(entity(
        EntityKind::Era,
        "age-of-iron",
        None,
        ratified(DECLARED_IN_BASE[0]),
    ));
    decls.push(entity(
        EntityKind::Event,
        "the-flood",
        None,
        ratified(DECLARED_IN_BASE[0]),
    ));
    decls.push(entity(
        EntityKind::Event,
        "the-fire",
        None,
        ratified(DECLARED_IN_BASE[0]),
    ));
    decls.push(Decl::Before(dli_ast::decl::BeforeDecl {
        earlier: id("the-flood"),
        later: id("the-fire"),
        provenance: ratified(DECLARED_IN_BASE[0]),
    }));
    // The declared constraints.
    decls.push(Decl::Functional(FunctionalDecl {
        pred: id("owner"),
        provenance: None,
    }));
    decls.push(Decl::Inertial(InertialDecl {
        pred: id("holds"),
        provenance: ratified(DECLARED_IN_BASE[1]),
    }));
    // An open question: its options are an exclusivity family in the theory, and
    // the theory must match between prior and union for fixpoint reuse to
    // engage, so no delta ever declares one.
    decls.push(Decl::Question(QuestionDecl {
        name: id("q-succession"),
        prompt: Some("who holds the march".to_owned()),
        options: vec![
            OptionClause {
                name: id("q-succession.elder"),
                label: None,
                implies: Some(ground("claimant", &[ent(0)])),
            },
            OptionClause {
                name: id("q-succession.younger"),
                label: None,
                implies: Some(ground("claimant", &[ent(1)])),
            },
        ],
        provenance: ratified(DECLARED_IN_BASE[1]),
    }));
    // The two citations that intern the retro-ruled and the forged decision.
    // Both read `citation.unknown-decision` in the base; a delta that ratifies
    // `D-0003` silently converts the first into a ruled variable, which is the
    // reason the ledger valuation is recomputed whole rather than extended.
    decls.push(fact(
        "primed",
        &["retro".to_owned()],
        ratified(RETRO_DECISION),
    ));
    decls.push(fact(
        "primed",
        &["forged".to_owned()],
        ratified(FORGED_DECISION),
    ));
    for i in 0..ENTITIES {
        decls.push(entity(
            EntityKind::Place,
            &ent(i),
            None,
            ratified(DECLARED_IN_BASE[0]),
        ));
    }
    decls.push(entity(
        EntityKind::Person,
        "the-steward",
        None,
        ratified(DECLARED_IN_BASE[1]),
    ));
    // A fluent on the inertial predicate, anchored on a declared event.
    decls.push(Decl::State(StateDecl {
        name: id("e0.holds"),
        value: Some(sym("calm")),
        time: Some(TimeClause::Since(id("the-flood"))),
        provenance: ratified(DECLARED_IN_BASE[1]),
    }));
    document(PACKS[0], decls)
}

/// Build one trial from a seed.
#[expect(clippy::too_many_lines)]
fn generate(seed: u64) -> Trial {
    let mut rng = Lcg::new(seed);
    let low = low_atoms();
    let high = high_atoms();

    // --- base -----------------------------------------------------------
    let mut base = vec![preamble(), document(PACKS[0], skeleton_rules(&mut rng))];

    // Base facts, in the first pack. `owner` is kept single-valued so the
    // declared FD holds in the base and only a delta can break it.
    let mut used: BTreeSet<(String, Vec<String>)> = BTreeSet::new();
    let mut owned: BTreeSet<String> = BTreeSet::new();
    let mut base_facts = Vec::new();
    for (pred, args) in &low {
        if !rng.chance(9) {
            continue;
        }
        if pred == "owner" && !owned.insert(args[0].clone()) {
            continue;
        }
        used.insert((pred.clone(), args.clone()));
        base_facts.push(fact(pred, args, random_provenance(&mut rng)));
    }
    // Identity in the base. When present, the prior's synthesized `same-as:link`
    // facts and alias rules sit at the end of its program, so the union's
    // declared rows are no longer appended after them and fixpoint reuse must
    // refuse — see the module docs on `dli_world::incremental`.
    let base_aliased = rng.chance(5);
    if base_aliased {
        base_facts.push(Decl::SameAs(SameAsDecl {
            left: id(&ent(0)),
            right: id(&ent(1)),
            provenance: ratified(DECLARED_IN_BASE[0]),
        }));
    }
    base.push(document(PACKS[0], base_facts));

    // A second base pack. It re-declares `kind place` and one entity name, so
    // the union always carries cross-pack collisions for `finish` to find.
    let mut march = vec![
        Decl::Kind(KindDecl {
            name: id("place"),
            provenance: ratified(DECLARED_IN_BASE[0]),
        }),
        entity(
            EntityKind::Place,
            &ent(1),
            None,
            ratified(DECLARED_IN_BASE[0]),
        ),
        Decl::State(StateDecl {
            name: id("e1.holds"),
            value: Some(sym("stirred")),
            time: Some(TimeClause::Until(id("the-fire"))),
            provenance: ratified(DECLARED_IN_BASE[1]),
        }),
    ];
    for (pred, args) in &low {
        if !rng.chance(3) {
            continue;
        }
        if pred == "owner" && !owned.insert(args[0].clone()) {
            continue;
        }
        if !used.insert((pred.clone(), args.clone())) {
            continue;
        }
        march.push(fact(pred, args, random_provenance(&mut rng)));
    }
    base.push(document(PACKS[1], march));

    // --- delta ----------------------------------------------------------
    // Each delta document is self-contained: no document's meaning depends on
    // another's position, which is what lets the oracle permute them.
    //
    // **The shallow/deep split is not cosmetic.** A delta that touches anything
    // at the bottom of the dependency order — an `edge` fact, a rule heading
    // `reach`, an identity link (whose rewrite family spans every predicate) —
    // transitively affects every stratum, so fixpoint reuse takes its path,
    // skips nothing, and degenerates into a full recompute with extra
    // bookkeeping. Drawing every delta that way made 15 of 96 ordering-checks
    // live: the oracle was exercising the reuse *machinery* while barely
    // exercising reuse, and every parity assertion still passed, which is the
    // danger. Deciding shallow-or-deep once per trial (rather than per fact, so
    // one unlucky draw cannot sink a whole delta) is what keeps the live-reuse
    // floor meaningful; the deep half is where the negation re-poisoning,
    // alias-resynthesis, and multi-stratum coverage comes from.
    let shallow = rng.chance(9);
    let mut delta = Vec::new();
    let mut delta_aliased = false;
    let mut retro_ratifies = false;
    let mut rules_taken = [false; OPTIONAL_RULES];
    let mut slots_taken: BTreeSet<String> = BTreeSet::new();
    let count = 2 + rng.below(3);
    for index in 0..count {
        let pack = PACKS[rng.below(PACKS.len())];
        let mut decls = Vec::new();

        // A retro-ratification, at most once per trial.
        if !retro_ratifies && rng.chance(7) {
            retro_ratifies = true;
            decls.push(Decl::Ratify(RatifyDecl {
                decision: id(RETRO_DECISION),
                body: dli_ast::language::RatifyBody::default(),
            }));
        }
        // A new derivation rule, never one already taken. A shallow delta may
        // only take `heir-rival`, the one whose head sits at the very top.
        if rng.chance(5) {
            let which = if shallow {
                2
            } else {
                rng.below(OPTIONAL_RULES)
            };
            if !rules_taken[which] {
                rules_taken[which] = true;
                decls.push(optional_rule(which, random_provenance(&mut rng)));
            }
        }
        // Identity declared by the delta. The base's links (if any) already
        // forced full re-synthesis; this is the case where the delta forces it.
        if !shallow && !delta_aliased && rng.chance(10) {
            delta_aliased = true;
            decls.push(Decl::SameAs(SameAsDecl {
                left: id(&ent(1)),
                right: id(&ent(2)),
                provenance: ratified(DECLARED_IN_BASE[1]),
            }));
        }
        // A re-declared entity: a duplicate inside its pack, a cross-pack
        // collision outside it. Never an era or event — the time-point table is
        // positional and a fresh one would renumber it.
        if rng.chance(4) {
            decls.push(entity(
                EntityKind::Place,
                &ent(rng.below(ENTITIES)),
                None,
                random_provenance(&mut rng),
            ));
        }
        // A fluent on a slot nothing else touches, so the timeline it lands on
        // holds exactly one span whatever the document order.
        if rng.chance(4) {
            let subject = format!("s{index}");
            if slots_taken.insert(subject.clone()) {
                decls.push(Decl::State(StateDecl {
                    name: id(&format!("{subject}.holds")),
                    value: Some(sym("risen")),
                    time: Some(TimeClause::Since(id("the-flood"))),
                    provenance: random_provenance(&mut rng),
                }));
            }
        }
        // Facts. A shallow delta draws only from high in the dependency order,
        // so reuse has strata left to skip; a deep one draws from the bottom
        // too, so the delta drives negation poison up through the strata.
        let facts = 1 + rng.below(3);
        for _ in 0..facts {
            let pool = if shallow || rng.chance(6) {
                &high
            } else {
                &low
            };
            for _ in 0..8 {
                let (pred, args) = pool[rng.below(pool.len())].clone();
                if used.insert((pred.clone(), args.clone())) {
                    decls.push(fact(&pred, &args, random_provenance(&mut rng)));
                    break;
                }
            }
        }
        if !decls.is_empty() {
            delta.push(document(pack, decls));
        }
    }

    let delta_packs = delta
        .iter()
        .filter_map(|doc| doc.scope.as_ref().map(|s| s.name.text.clone()))
        .collect::<BTreeSet<_>>()
        .len();

    Trial {
        base,
        delta,
        base_aliased,
        delta_aliased,
        retro_ratifies,
        delta_packs,
        shallow,
    }
}

// ---------------------------------------------------------------------------
// Comparison helpers
// ---------------------------------------------------------------------------

/// A pack over a document run. The pack *name* never reaches the compiled world
/// (only the per-document `scope` headers do), so one name serves every trial.
fn pack_of(documents: Vec<Document>) -> Pack {
    Pack {
        name: "oracle".to_owned(),
        documents,
    }
}

/// The base pack followed by one ordering of the delta.
fn union(base: &[Document], delta: &[Document]) -> Pack {
    let mut documents = base.to_vec();
    documents.extend_from_slice(delta);
    pack_of(documents)
}

/// Canonical bytes for a compiled world.
///
/// YAML, not JSON: `WorldModel::fluents` is keyed by the `(pred, subject)`
/// tuple, which JSON cannot encode as a map key at all. Every map in the type is
/// a `BTreeMap` and `serde` emits struct fields in declaration order, so the
/// encoding is deterministic and byte-comparison is a strictly stronger identity
/// check than the derived `PartialEq` — it also sees the skipped-when-empty
/// fields.
fn canonical(world: &CompiledWorld) -> String {
    serde_yaml_ng::to_string(world).unwrap()
}

/// Canonical JSON bytes for the evaluable half — the exact encoding the PR 1
/// saturation oracle pins, applied here to the world a full compile produces.
fn canonical_eval(world: &CompiledWorld) -> Vec<u8> {
    serde_json::to_vec(&world.model.world).unwrap()
}

/// The saturation, keyed by atom.
fn rows(world: &World) -> BTreeMap<GroundAtom, Derived> {
    world
        .derived
        .iter()
        .map(|row| (row.atom.clone(), row.clone()))
        .collect()
}

/// Render a serializable value to a canonical string, for order-insensitive
/// multiset comparison.
fn render<T: serde::Serialize>(value: &T) -> String {
    serde_yaml_ng::to_string(value).unwrap()
}

/// A sorted multiset of rendered items — the shape a sequence takes when its
/// order is document order and therefore not part of the world's meaning.
fn multiset<T: serde::Serialize>(items: &[T]) -> Vec<String> {
    let mut out: Vec<String> = items.iter().map(render).collect();
    out.sort();
    out
}

/// The **order-independent projection** of a compiled world: everything two
/// orderings of the same delta must agree on.
///
/// `atoms`, `sorts`, `facts`, `rules`, and the load findings are sequences in
/// *document order*, so two orderings legitimately differ there and comparing
/// them would test the permutation rather than the compiler. What must not
/// differ is the meaning: the saturation, the ledger valuation, the derived
/// index, the entity and pack registries, the declared constraints, the fluent
/// timelines, and every finding taken as a multiset.
fn projection(world: &CompiledWorld) -> Vec<String> {
    let eval = &world.model.world;
    let mut out = vec![
        render(&rows(eval)),
        render(&eval.valuation),
        render(&eval.support_index),
        render(&eval.var_grades),
        render(&eval.theory),
        render(&eval.chronology),
        render(&eval.time_points),
        render(&eval.interner),
        render(&eval.functional_predicates),
        render(&eval.inertial_predicates),
        render(&world.model.entities),
        render(&world.model.packs),
        render(&world.model.fluents),
        render(&world.model.language),
        render(&dli_world::check::run(world)),
    ];
    out.extend(multiset(&eval.findings));
    out.push("--load-findings--".to_owned());
    out.extend(multiset(&world.model.findings));
    out.push("--sorts--".to_owned());
    out.extend(multiset(&world.model.sorts));
    out.push("--same-as--".to_owned());
    out.extend(multiset(&eval.same_as_links));
    out
}

// ---------------------------------------------------------------------------
// Coverage
// ---------------------------------------------------------------------------

/// Trial counts for the situations this oracle exists to cover. Asserted against
/// **floors**, not against `> 0`: a generator that drifts until it stops
/// producing (say) cross-pack deltas would still pass a `> 0` check while
/// testing nothing, whereas a floor turns that drift into a red test.
#[derive(Debug, Default)]
struct Coverage {
    /// Trials scored.
    trials: usize,
    /// (a) The delta added an identity link, so the whole alias-rewrite family
    /// is re-synthesized over every predicate in the union census.
    alias_resynth: usize,
    /// (b) The base already declared identity, so the prior's synthesized rows
    /// break the append-only precondition and the fixpoint half must refuse
    /// reuse — the known limitation, pinned so that fixing it is a red test that
    /// asks for the doc comment to be updated.
    alias_forced_full_fixpoint: usize,
    /// (c) The delta's effect spanned two or more stratum levels.
    multi_stratum: usize,
    /// (d) The delta's documents landed in two or more packs.
    cross_pack: usize,
    /// (e) Negation/defeater propagation: a row that survived the delta came out
    /// of it carrying different defeater conditions. This is the case a retained
    /// row would swallow.
    defeater_propagation: usize,
    /// (f) The delta declared a decision the base had only cited, retro-ruling a
    /// base claim certain — the reason the ledger valuation cannot be extended.
    retro_ratification: usize,
    /// How many trials drew a shallow delta (diagnostic for the floors below).
    shallow: usize,
}

/// The stratum level of each predicate; predicates heading no rule map to
/// nothing.
fn level_of(world: &World) -> BTreeMap<String, usize> {
    let mut index = BTreeMap::new();
    if let Ok(strata) = dli_eval::stratify(world) {
        for (i, level) in strata.levels.iter().enumerate() {
            for pred in level {
                index.insert(pred.clone(), i);
            }
        }
    }
    index
}

/// Score one trial from its base world and the world the union compiles to.
fn score(coverage: &mut Coverage, trial: &Trial, base: &CompiledWorld, full: &CompiledWorld) {
    coverage.trials += 1;
    coverage.alias_resynth += usize::from(trial.delta_aliased);
    coverage.cross_pack += usize::from(trial.delta_packs >= 2);
    coverage.retro_ratification += usize::from(trial.retro_ratifies);
    coverage.shallow += usize::from(trial.shallow);

    let (prior, next) = (rows(&base.model.world), rows(&full.model.world));
    let levels = level_of(&full.model.world);
    let mut touched: BTreeSet<Option<usize>> = BTreeSet::new();
    let mut repoisoned = false;
    for (atom, row) in &next {
        let was = prior.get(atom);
        if was == Some(row) {
            continue;
        }
        touched.insert(levels.get(&atom.pred).copied());
        if let Some(was) = was
            && was.conditions != row.conditions
        {
            repoisoned = true;
        }
    }
    coverage.multi_stratum += usize::from(touched.len() >= 2);
    coverage.defeater_propagation += usize::from(repoisoned);
}

// ---------------------------------------------------------------------------
// The oracle
// ---------------------------------------------------------------------------

/// How many generated corpora the oracle runs.
const SEEDS: u64 = 40;
/// How many orderings of each delta are checked. The failure mode is divergence
/// under *some* ordering, so one is never enough.
const ORDERINGS: usize = 3;

/// The floors that keep this file from going quietly vacuous, in one place.
///
/// Split out from the oracle body so the assertions read as a checklist rather
/// than as a tail on a long loop.
fn assert_floors(
    coverage: &Coverage,
    checked: usize,
    loader_live: usize,
    loader_controls: usize,
    fixpoint_live: usize,
    fixpoint_controls: usize,
) {
    assert_eq!(
        checked,
        usize::try_from(SEEDS).unwrap() * ORDERINGS,
        "every seed must contribute every ordering"
    );
    // THE ANTI-VACUITY FLOORS. Every equality assertion in this file is
    // satisfiable by a `compile_incremental` that quietly recompiles everything,
    // so these are the assertions that make the rest of the file mean something.
    //
    // The loader half reuses on every ordering-check by construction (the base
    // is never empty), so its floor is the full count — anything less means the
    // reuse path stopped engaging. The control is what proves the reuse is real:
    // it falsifies a pack name the loader carries forward and demands the
    // compiled world change, which it can only do if that state was reused.
    assert_eq!(
        loader_live, checked,
        "anti-vacuity: only {loader_live} of {checked} ordering-checks reused the loader state"
    );
    assert_eq!(
        loader_controls, loader_live,
        "every live loader reuse must be provable by falsifying carried state: {loader_controls} \
         of {loader_live} were"
    );

    // The fixpoint half only engages when the union program *appends* to the
    // prior's, which the base's own identity closure defeats (see the
    // `dli_world::incremental` module docs), and is only *live* when it also had
    // a stratum to skip. Observed for this generator: **48 of 120**
    // ordering-checks, every one with a passing control. The floor sits under
    // that and far above the 15 the generator produced before its delta draws
    // were split shallow/deep (see `generate`), so it discriminates both
    // failures — reuse silently ceasing to engage, and the generator drifting
    // back to deltas at the bottom of the dependency order that leave nothing to
    // skip. Either way this goes red while every equality assertion in the file
    // still passes.
    assert!(
        fixpoint_live >= 32,
        "anti-vacuity floor: only {fixpoint_live} of {checked} ordering-checks actually reused a \
         stratum — a parity oracle that always falls back proves nothing"
    );
    assert_eq!(
        fixpoint_controls, fixpoint_live,
        "every live fixpoint reuse must be provable by falsifying a reused row: \
         {fixpoint_controls} of {fixpoint_live} were"
    );

    // Coverage floors, tied to the generator configuration above: `SEEDS = 40`
    // corpora over `ENTITIES = 3` entities and three packs, each with a 2..4
    // document delta. The counts this configuration actually produces are alias
    // re-synthesis 15, base-alias fallback 12, multi-stratum 40, cross-pack 33,
    // defeater propagation 28, and retro-ratification 33 of 40 trials (23 of
    // which drew a shallow delta). The floors sit at roughly two thirds of
    // those: low enough that benign churn in the generator does not fail the
    // build, high enough that a case going uncovered does. A count under a floor
    // means the generator drifted off the behaviour it exists to exercise — fix
    // the generator, do not lower the floor.
    assert_eq!(coverage.trials, usize::try_from(SEEDS).unwrap());
    assert!(
        coverage.alias_resynth >= 10,
        "coverage floor: same-as-triggered alias re-synthesis, got {} of {} trials",
        coverage.alias_resynth,
        coverage.trials
    );
    assert!(
        coverage.alias_forced_full_fixpoint >= 8,
        "coverage floor: a base-declared identity closure forcing the full fixpoint, got {} of {} \
         trials. If this dropped because the fixpoint half learned to reuse across a synthesized \
         closure, that is a fix — update the `dli_world::incremental` module docs and this floor \
         together",
        coverage.alias_forced_full_fixpoint,
        coverage.trials
    );
    assert!(
        coverage.multi_stratum >= 26,
        "coverage floor: multi-stratum delta effects, got {} of {} trials",
        coverage.multi_stratum,
        coverage.trials
    );
    assert!(
        coverage.cross_pack >= 22,
        "coverage floor: cross-pack deltas, got {} of {} trials",
        coverage.cross_pack,
        coverage.trials
    );
    assert!(
        coverage.defeater_propagation >= 18,
        "coverage floor: defeater/negation propagation, got {} of {} trials",
        coverage.defeater_propagation,
        coverage.trials
    );
    assert!(
        coverage.retro_ratification >= 22,
        "coverage floor: a delta retro-ratifying a decision the base cited, got {} of {} trials",
        coverage.retro_ratification,
        coverage.trials
    );
}

/// Prove the loader half genuinely reused: falsify a pack name it carries
/// forward and demand the compiled world change.
///
/// If the state was reused, the falsification rides into `model.atoms`; if
/// `compile_incremental` had quietly re-lowered every document, the falsified
/// copy would be overwritten and this would fire. Returns whether a control ran.
fn prove_loader_reuse(
    prior: &IncrementalWorld,
    ordering: &[Document],
    full: &CompiledWorld,
) -> bool {
    let Some(falsified) = prior.falsify_carried_load_state() else {
        return false;
    };
    let (poisoned, _) = compile_incremental(&falsified, ordering).unwrap();
    poisoned.world() != full
}

/// Prove the fixpoint half genuinely reused: falsify a saturated row the delta
/// does not affect and demand the compiled world change.
fn prove_fixpoint_reuse(
    prior: &IncrementalWorld,
    ordering: &[Document],
    full: &CompiledWorld,
    report: &CompileReuse,
) -> bool {
    let Some(falsified) = prior.falsify_reused_row(report) else {
        return false;
    };
    let (poisoned, _) = compile_incremental(&falsified, ordering).unwrap();
    poisoned.world() != full
}

/// The oracle: an incremental compile is byte-identical to a full compile of the
/// union pack, under every ordering of the delta, and every ordering agrees with
/// every other on the world's meaning.
#[test]
fn incremental_compile_is_byte_identical_to_full_compile() {
    let mut coverage = Coverage::default();
    let mut checked = 0usize;
    let mut loader_live = 0usize;
    let mut loader_controls = 0usize;
    let mut fixpoint_live = 0usize;
    let mut fixpoint_controls = 0usize;

    for seed in 0..SEEDS {
        let trial = generate(seed);
        assert!(!trial.delta.is_empty(), "seed {seed}: generated no delta");

        let base_pack = pack_of(trial.base.clone());
        let prepared = compile_resumable(&base_pack).unwrap();
        let base_full = compile(&base_pack).unwrap();
        // The resumable compile is the ordinary compile, plus the state that
        // makes it extendable. If that ever stops holding, nothing below means
        // anything.
        assert_eq!(
            canonical(prepared.world()),
            canonical(&base_full),
            "seed {seed}: compile_resumable diverged from compile on the base pack"
        );

        let mut rng = Lcg::new(seed ^ 0xDEAD_BEEF);
        let mut orderings = vec![trial.delta.clone()];
        for _ in 1..ORDERINGS {
            let mut permuted = trial.delta.clone();
            rng.shuffle(&mut permuted);
            orderings.push(permuted);
        }

        let mut reference: Option<Vec<String>> = None;
        for (index, ordering) in orderings.iter().enumerate() {
            let full = compile(&union(&trial.base, ordering)).unwrap();
            let (incremental, report) = compile_incremental(&prepared, ordering).unwrap();

            assert!(
                incremental.world() == &full,
                "seed {seed} ordering {index}: incremental compile diverged from full compile"
            );
            assert_eq!(
                canonical(incremental.world()),
                canonical(&full),
                "seed {seed} ordering {index}: incremental compile is not byte-identical to full \
                 compile"
            );
            assert_eq!(
                canonical_eval(incremental.world()),
                canonical_eval(&full),
                "seed {seed} ordering {index}: the evaluable world is not byte-identical"
            );
            // `IncrementalWorld::check` is the full, unsliced battery (DL-0088);
            // it must agree with the battery over a full compile.
            assert_eq!(
                incremental.check(),
                dli_world::check::run(&full),
                "seed {seed} ordering {index}: the check battery disagreed"
            );
            checked += 1;

            if report.loader_is_live() {
                loader_live += 1;
                loader_controls += usize::from(prove_loader_reuse(&prepared, ordering, &full));
            }
            if report.fixpoint_is_live() {
                fixpoint_live += 1;
                fixpoint_controls +=
                    usize::from(prove_fixpoint_reuse(&prepared, ordering, &full, &report));
            }
            if trial.base_aliased && !report.fixpoint.reused {
                coverage.alias_forced_full_fixpoint += usize::from(index == 0);
            }

            // Confluence: every ordering of the same delta reaches the same
            // world *meaning*. Only the order-independent projection is
            // compared — the document-order sequences legitimately differ.
            match &reference {
                None => {
                    score(&mut coverage, &trial, &base_full, &full);
                    reference = Some(projection(&full));
                }
                Some(expected) => assert_eq!(
                    &projection(&full),
                    expected,
                    "seed {seed} ordering {index}: compiling is not confluent across delta \
                     orderings"
                ),
            }
        }
    }

    assert_floors(
        &coverage,
        checked,
        loader_live,
        loader_controls,
        fixpoint_live,
        fixpoint_controls,
    );
}

/// A chain of extensions is the same world as one compile of everything.
///
/// The single-step oracle above never re-uses an `IncrementalWorld` that was
/// itself produced incrementally, and that is exactly what a session does — one
/// compile per authored declaration, each on top of the last. A state that was
/// subtly wrong only after the *second* extension would pass everything above.
#[test]
fn chained_extensions_equal_one_full_compile() {
    for seed in 0..SEEDS {
        let trial = generate(seed);
        let mut prepared = compile_resumable(&pack_of(trial.base.clone())).unwrap();
        for document in &trial.delta {
            let (next, _) = compile_incremental(&prepared, std::slice::from_ref(document)).unwrap();
            prepared = next;
        }
        let full = compile(&union(&trial.base, &trial.delta)).unwrap();
        assert_eq!(
            canonical(prepared.world()),
            canonical(&full),
            "seed {seed}: a chain of one-document extensions diverged from one full compile"
        );
        assert_eq!(
            prepared.pack().documents.len(),
            trial.base.len() + trial.delta.len(),
            "seed {seed}: the carried pack lost documents"
        );
    }
}

/// The fixpoint-only fallback — for a caller holding a `CompiledWorld` and no
/// resumable state — is byte-identical too, and refuses an unrelated prior
/// rather than trusting it.
#[test]
fn fixpoint_only_reuse_is_byte_identical() {
    let mut reused = 0usize;
    for seed in 0..SEEDS {
        let trial = generate(seed);
        let base_pack = pack_of(trial.base.clone());
        let base = compile(&base_pack).unwrap();
        let union_pack = union(&trial.base, &trial.delta);

        let (world, report) = dli_world::compile_reusing_fixpoint(&base, &union_pack).unwrap();
        let full = compile(&union_pack).unwrap();
        assert_eq!(
            canonical(&world),
            canonical(&full),
            "seed {seed}: fixpoint-only reuse diverged from a full compile"
        );
        reused += usize::from(report.fixpoint.reused);

        // An unrelated prior — the union offered as a base for the *base* pack,
        // which is a truncation rather than an extension — must be refused, not
        // trusted. The answer stays right either way; that is the point.
        let (narrowed, refusal) = dli_world::compile_reusing_fixpoint(&full, &base_pack).unwrap();
        assert_eq!(
            canonical(&narrowed),
            canonical(&base),
            "seed {seed}: a truncating prior produced a wrong world"
        );
        assert!(
            !refusal.fixpoint.reused,
            "seed {seed}: a truncating prior was accepted as a reuse base"
        );
    }
    assert!(
        reused >= 16,
        "only {reused} of {SEEDS} fixpoint-only reuses engaged at all"
    );
}

/// The verify-against-full mode agrees with the plain one, and says so by not
/// panicking.
#[test]
fn verified_mode_agrees_with_the_plain_one() {
    for seed in 0..4 {
        let trial = generate(seed);
        let prepared = compile_resumable(&pack_of(trial.base.clone())).unwrap();
        let (verified, _) =
            dli_world::compile_incremental_verified(&prepared, &trial.delta).unwrap();
        let (plain, _) = compile_incremental(&prepared, &trial.delta).unwrap();
        assert_eq!(canonical(verified.world()), canonical(plain.world()));
    }
}

/// The generator must actually build the corpus it claims to: several packs,
/// three or more strata with stratified negation, an identity closure, an
/// FD-bearing predicate, an inertial fluent, a chronology, and citations that
/// both resolve and do not.
#[test]
fn the_generator_builds_the_corpus_it_claims() {
    let mut deepest = 0usize;
    let mut aliased = 0usize;
    let mut packs: BTreeSet<String> = BTreeSet::new();
    let mut saw_forged_citation = false;
    let mut saw_negation = false;

    for seed in 0..SEEDS {
        let trial = generate(seed);
        let pack = union(&trial.base, &trial.delta);
        for document in &pack.documents {
            if let Some(scope) = &document.scope {
                packs.insert(scope.name.text.clone());
            }
        }
        let world = compile(&pack).unwrap();
        let model = &world.model;

        deepest = deepest.max(
            dli_eval::stratify(&model.world)
                .map(|strata| strata.levels.len())
                .unwrap_or_default(),
        );
        aliased += usize::from(trial.base_aliased || trial.delta_aliased);
        saw_negation |= model
            .world
            .rules
            .iter()
            .any(|rule| rule.body.iter().any(|lit| lit.negated));
        saw_forged_citation |= model
            .world
            .findings
            .iter()
            .any(|finding| finding.code == "citation.unknown-decision");

        assert!(
            model.functional_predicates.contains("owner"),
            "seed {seed}: the FD-bearing predicate is not declared"
        );
        assert!(
            model.inertial_predicates.contains_key("holds"),
            "seed {seed}: the inertial predicate is not declared"
        );
        assert!(
            !model.fluents.is_empty(),
            "seed {seed}: no fluent timeline was built"
        );
        assert!(
            !model.world.chronology.edges.is_empty(),
            "seed {seed}: no chronology edge was built"
        );
        assert!(
            !model.world.theory.exclusive.is_empty(),
            "seed {seed}: no exclusivity family was built"
        );
        assert!(
            model
                .findings
                .iter()
                .any(|finding| finding.code() == "load.cross-pack-collision"),
            "seed {seed}: no cross-pack collision was built"
        );
        assert!(
            !model.world.derived.is_empty(),
            "seed {seed}: the corpus saturated to nothing"
        );
    }

    assert!(
        deepest >= 3,
        "generated corpora are only {deepest} strata deep"
    );
    assert!(saw_negation, "no generated rule set used negation");
    assert!(saw_forged_citation, "no forged citation was ever generated");
    assert!(aliased >= 12, "only {aliased} seeds declared identity");
    assert_eq!(
        packs.len(),
        PACKS.len(),
        "the generator never reached every pack: {packs:?}"
    );
}
