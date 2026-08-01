//! Total lowering: the bridge between the in-memory world model and the
//! stored fact-object form (DL-0041, roadmap §LS-1).
//!
//! This module implements the two-way conversion that the total-lowering
//! oracle (`tests/total_lowering.rs`) depends on:
//!
//! - **Forward lowering** ([`lower_atoms`]): each extensional [`LoadedAtom`]
//!   in a compiled world becomes a [`FactObject`], carrying its annotation
//!   (`support`, `defeaters`, per-monomial `conditions`), grade, and a minted
//!   `ClaimId`.  This is the stored representation — the schema that
//!   `schema_for!` emits and the gate checks for drift.
//!
//! - **Reverse lowering** ([`reconstruct_pack`]): each [`FactObject`] is turned
//!   back into a source [`Decl`], with the `Annotation` *derived* from the
//!   support polynomial (not stored separately — the polynomial is the
//!   canonical provenance encoding).  Cited decision variables are recovered
//!   via the committed interner, so `ratify` declarations can be re-emitted
//!   and the ledger valuation is restored on recompilation.
//!
//! The oracle verifies that lowering the vendored corpus, then recompiling the
//! reconstructed documents via `dli_world::compile`, reproduces an identical
//! fixpoint, findings, and digests.

use std::collections::BTreeSet;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, EntityDecl, EntityKind, FactDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::atom::{Interner, Lit, Rail};
use dli_core::grade::Grade;
use dli_core::posbool::{Monomial, PosBool};
use dli_eval::world::Fact;
use dli_world::model::LoadedAtom;
use dli_world::pack::Pack;

use crate::identity::ClaimId;
use crate::object::{CondPair, FactObject};

// ── rendering ─────────────────────────────────────────────────────────

/// Render a `Term` to its ground string form — mirrors the loader's private
/// `render_term` so the `GroundAtom` produced here matches `world.facts`.
fn render_term(term: &Term) -> String {
    match term {
        Term::Var(id) | Term::Sym(id) => id.text.clone(),
        Term::Str(text) => text.clone(),
        Term::Int(value) => value.to_string(),
        Term::Time(dli_ast::term::TimeRef::Era(id)) => format!("era:{}", id.text),
        Term::Time(dli_ast::term::TimeRef::Event(id)) => format!("event:{}", id.text),
    }
}

/// Convert an `AtomExpr` to a `GroundAtom` — mirrors the loader's private
/// `ground_atom`.
fn atom_expr_to_ground(atom: &AtomExpr) -> dli_eval::world::GroundAtom {
    dli_eval::world::GroundAtom {
        pred: atom.pred.text.clone(),
        args: atom.args.iter().map(render_term).collect(),
    }
}

/// Render a single literal as `+<name>` or `-<name>` using the interner.
pub fn render_lit(lit: &Lit, interner: &Interner) -> String {
    let sign = match lit.rail {
        Rail::Pos => '+',
        Rail::Neg => '-',
    };
    let name = interner
        .resolve(lit.atom)
        .map_or_else(|| format!("atom{}", lit.atom.0), str::to_owned);
    format!("{sign}{name}")
}

/// Render a monomial as `{{+a, +b}}` using the interner — the canonical
/// comparison form for one support path.
pub fn render_monomial(m: &Monomial, interner: &Interner) -> String {
    let lits: Vec<String> = m.lits().map(|lit| render_lit(&lit, interner)).collect();
    format!("{{{}}}", lits.join(", "))
}

/// Render a polynomial as a disjunction of rendered monomials — the canonical
/// comparison form for a support or defeater polynomial.
pub fn render_polynomial(p: &PosBool, interner: &Interner) -> String {
    let monomials: Vec<String> = p
        .monomials()
        .map(|m| render_monomial(m, interner))
        .collect();
    if monomials.is_empty() {
        "FALSE".to_owned()
    } else {
        monomials.join(" | ")
    }
}

// ── forward lowering ──────────────────────────────────────────────────

/// Lower a set of [`LoadedAtom`]s into [`FactObject`]s.
///
/// `LoadedAtom` is the loader's record of every source atom — entities, slots,
/// relations, before-clauses — so this captures the complete extensional
/// layer, not just the subset that lives in `World::facts`.
///
/// Each atom becomes exactly one object, carrying the full annotation
/// (`support`, `defeaters`, per-monomial `conditions`) and a deterministic
/// `ClaimId` minted from a label that encodes the atom's surface form.
pub fn lower_atoms(atoms: &[LoadedAtom]) -> Vec<FactObject> {
    atoms
        .iter()
        .enumerate()
        .map(|(index, la)| {
            let ground = atom_expr_to_ground(&la.atom);
            let conditions: Vec<CondPair> = la
                .ann
                .conditions
                .iter()
                .map(|(monomial, poison)| {
                    CondPair::new(
                        PosBool(std::iter::once(monomial.clone()).collect()),
                        poison.clone(),
                    )
                })
                .collect();
            let label = format!("atom#{index}:{}", ground.render());
            FactObject::new(
                ClaimId::from_label(&label),
                ground,
                la.ann.support.clone(),
                la.ann.defeaters.clone(),
                conditions,
                Grade::Asserted,
            )
        })
        .collect()
}

/// Lower a set of extensional [`Fact`]s into [`FactObject`]s.
///
/// Each fact becomes exactly one object, carrying the full annotation
/// (`support`, `defeaters`, per-monomial `conditions`), the grade, and a
/// deterministic `ClaimId` minted from a label that encodes the fact's
/// surface form.
pub fn lower_facts(facts: &[Fact]) -> Vec<FactObject> {
    facts
        .iter()
        .enumerate()
        .map(|(index, fact)| {
            let conditions: Vec<CondPair> = fact
                .ann
                .conditions
                .iter()
                .map(|(monomial, poison)| {
                    CondPair::new(
                        PosBool(std::iter::once(monomial.clone()).collect()),
                        poison.clone(),
                    )
                })
                .collect();
            let label = format!(
                "fact#{index}:{}({})",
                fact.atom.pred,
                fact.atom.args.join(",")
            );
            FactObject::new(
                ClaimId::from_label(&label),
                fact.atom.clone(),
                fact.ann.support.clone(),
                fact.ann.defeaters.clone(),
                conditions,
                fact.grade,
            )
        })
        .collect()
}

// ── reverse lowering ──────────────────────────────────────────────────

/// The entity-kind keywords that the loader recognises (spec 05 §3).
const ENTITY_KEYWORDS: &[&str] = &["place", "era", "event", "person", "thing"];

/// Is `pred` one of the entity-kind keywords?
fn is_entity_kind(pred: &str) -> bool {
    ENTITY_KEYWORDS.contains(&pred)
}

/// Map an entity-kind keyword back to `EntityKind`.
fn entity_kind_from_keyword(pred: &str) -> EntityKind {
    match pred {
        "place" => EntityKind::Place,
        "era" => EntityKind::Era,
        "event" => EntityKind::Event,
        "person" => EntityKind::Person,
        _ => EntityKind::Thing,
    }
}

/// Derive an [`Annotation`] from a support polynomial.
///
/// The support polynomial is the canonical encoding of provenance: decision
/// variables (`D-XXXX`) in the support are citations; all-positive supports
/// gated only on hypothesis variables (`h.*`) are proposals; `FALSE` means
/// fail-closed (no annotation at all).
///
/// This function uses the committed `interner` to resolve `Atom` indices to
/// variable names and `known_decisions` to distinguish citations from
/// hypotheses. The marker for all ledger-gated claims is reconstructed as
/// `Ratified` — the loader treats `Decided`, `Ratified`, and `Provisional`
/// identically for valuation setting, so the fixpoint is unaffected.
pub fn derive_annotation(
    support: &PosBool,
    interner: &Interner,
    known_decisions: &BTreeSet<String>,
) -> Option<Annotation> {
    if support.is_false() {
        return None; // fail-closed: no provenance (spec 02 §3)
    }

    // Collect positive variable names that are known decisions (citations).
    let citations: Vec<String> = support
        .monomials()
        .flat_map(dli_core::posbool::Monomial::lits)
        .filter(|lit| {
            lit.rail == Rail::Pos && {
                interner
                    .resolve(lit.atom)
                    .is_some_and(|name| known_decisions.contains(name))
            }
        })
        .filter_map(|lit| interner.resolve(lit.atom).map(String::from))
        .collect();

    if !citations.is_empty() {
        // Ledger-gated: every cited decision is a citation.
        Some(Annotation {
            marker: ProvenanceMarker::Ratified,
            citations: citations
                .into_iter()
                .map(|c| CitationRef {
                    id: Ident::synthetic(c),
                })
                .collect(),
            condition: None,
            tags: Vec::new(),
            span: Span::synthetic(),
        })
    } else if support.is_true() {
        // Unconditional proposal (no gating variable).
        Some(Annotation {
            marker: ProvenanceMarker::Proposed,
            citations: Vec::new(),
            condition: None,
            tags: Vec::new(),
            span: Span::synthetic(),
        })
    } else {
        // Hypothesis-gated proposal: the support variables are hypothesis names.
        Some(Annotation {
            marker: ProvenanceMarker::Proposed,
            citations: Vec::new(),
            condition: None,
            tags: Vec::new(),
            span: Span::synthetic(),
        })
    }
}

/// Build an `AtomExpr` from a `GroundAtom`, choosing a term shape that
/// round-trips through the loader.
///
/// For `before(p, q)` we emit `Term::Sym` for both args — the same form the
/// corpus uses (the loader's `render_term` produces a plain string for both
/// `Term::Sym` and `Term::Time(Era)`, so a `GroundAtom` cannot distinguish
/// them, and `Sym` is the lossless default for the grounded form).
fn atom_expr_from_ground(ground: &dli_eval::world::GroundAtom) -> AtomExpr {
    AtomExpr {
        pred: Ident::synthetic(&ground.pred),
        args: ground
            .args
            .iter()
            .map(|a| Term::Sym(Ident::synthetic(a)))
            .collect(),
    }
}

/// Reconstruct a [`Pack`] from a set of [`FactObject`]s, ready for
/// recompilation.
///
/// The reconstruction derives each fact's `Annotation` from its support
/// polynomial (using the committed `interner` to resolve `Atom` indices to
/// variable names) and re-emits `Decl::Ratify` declarations for every cited
/// decision so the loader can restore the ledger valuation on recompile.
///
/// Entity-kind predicates (`place`, `era`, …) are reconstructed as
/// `Decl::Entity` so the loader's entity registry and proposal counter advance
/// in exactly the same order as the original — preserving `Atom` index
/// assignments and keeping content-address digests stable across the round-trip.
///
/// The `pack_name` seeds the proposal-bundle variable naming — it must match
/// the original pack's scope header.
pub fn reconstruct_pack(
    fact_objects: &[FactObject],
    interner: &Interner,
    known_decisions: &BTreeSet<String>,
    pack_name: &str,
) -> Pack {
    let mut decls: Vec<Decl> = Vec::new();

    // Ratify first: this interns every decision-id variable before any fact,
    // matching the loader's interner growth order exactly.
    for decision in known_decisions {
        decls.push(Decl::Ratify(RatifyDecl {
            decision: Ident::synthetic(decision),
            body: Vec::new(),
        }));
    }

    // Then facts/entities in lowered order — proposal-bundle counters advance
    // in the same sequence as the original pack.
    for fo in fact_objects {
        let annotation = derive_annotation(&fo.support, interner, known_decisions);

        if is_entity_kind(&fo.atom.pred) && fo.atom.args.len() == 1 {
            // Reconstruct as Decl::Entity — entity facts always have exactly one
            // argument (the entity name).  A predicate that happens to share an
            // entity-kind keyword but has two args (e.g. `era(narrator,
            // centuries-after)`) is a regular fact, not an entity declaration.
            let kind = entity_kind_from_keyword(&fo.atom.pred);
            let name = Ident::synthetic(&fo.atom.args[0]);
            decls.push(Decl::Entity(EntityDecl {
                kind,
                name,
                label: None,
                time: None,
                slots: Vec::new(),
                provenance: annotation,
            }));
        } else {
            let atom_expr = atom_expr_from_ground(&fo.atom);
            decls.push(Decl::Fact(FactDecl {
                atom: atom_expr,
                triple: false,
                provenance: annotation,
            }));
        }
    }

    let scope = if pack_name.is_empty() {
        None
    } else {
        Some(ScopeHeader {
            name: Ident::synthetic(pack_name),
            span: Span::synthetic(),
        })
    };

    dli_world::pack::Pack {
        name: pack_name.to_owned(),
        documents: vec![Document { scope, decls }],
    }
}

/// Extract the `Decl::Ratify` decision ids from a pack — mirrors the private
/// `declared_dedecisions` in `dli_world::compile`.
pub fn declared_decisions(pack: &dli_world::pack::Pack) -> BTreeSet<String> {
    pack.documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .filter_map(|decl| match decl {
            Decl::Ratify(ratify) => Some(ratify.decision.text.trim().to_owned()),
            _ => None,
        })
        .collect()
}
