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

use dli_ast::annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
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

// ── per-declaration lowering ────────────────────────────────────────────

/// Lower a single [`Decl`] into [`FactObject`]s, without a full world compilation.
///
/// This is the LS-4b write path: each `author.add_*` call lowers its
/// declaration here and writes the resulting `FactObject` to the lorestore
/// immediately, so fact objects are durable before `materialize` commits them.
///
/// The `interner` must be seeded with the **base pack's** variable names (in
/// order) so that:
/// - citation variables (`D-nnnn`) and hypothesis names resolve to the same
///   `Atom` indices the full compilation would assign, producing identical
///   content hashes.
/// - fresh proposal-bundle variables (`h.<pack>#N`) are appended sequentially,
///   matching the loader's `bundle_counter`.
///
/// **Interner fidelity:** every declaration — not just the four fact-producing
/// types — is run through provenance interning to keep the interner's Atom
/// indices in lock-step with the full loader. Without this, citation
/// variables from expert-tier declarations (e.g. a `ratified` rule decl) would
/// shift the bundle counter and desync all subsequent indices.
///
/// Only the four fact-producing gentle-subset declarations yield objects:
/// `Entity`, `Attr` (slot), `Fact` (relation), and `Before`. Returns an empty
/// vector for all others.
///
/// **Must produce byte-identical `FactObject`s to [`lower_atoms`] given the
/// same interner state** — the total-lowering oracle verifies this.
pub fn lower_decl(
    decl: &Decl,
    pack_name: &str,
    interner: &mut Interner,
    bundle_counter: &mut u32,
) -> Vec<FactObject> {
    // Question and Fork declarations don't process their own provenance in the
    // loader — only their options (which intern names and produce implied-atom
    // fact objects). Handle them specially.
    if let Decl::Question(q) = decl {
        return lower_options(&q.options, pack_name, interner);
    }
    if let Decl::Fork(f) = decl {
        return lower_options(&f.options, pack_name, interner);
    }

    // Hypothesis declarations intern their name as a variable — no fact object
    // is produced, but the variable must be interned to keep the interner in
    // sync (it may be referenced later by `suppose` or `assumes` conditions).
    if let Decl::Hypothesis(h) = decl {
        let _ = interner.intern(&h.name.text);
        return Vec::new();
    }

    // Always intern the decl's provenance variables. For fact-producing
    // decls, the resulting Ann is reused below.
    let provenance = decl_provenance(decl);
    let label = decl_label(decl);
    let ann = provenance.map(|p| compile_ann(Some(p), &label, pack_name, interner, bundle_counter));

    let (atom_expr, _) = decl_to_atom(decl);
    let Some(atom_expr) = atom_expr else {
        return Vec::new(); // hypotheses, use-decls, questions-without-options
    };

    let ann = ann.unwrap_or_else(|| dli_core::ann::Ann::supported(PosBool::FALSE));
    let ground = atom_expr_to_ground(&atom_expr);
    let conditions = cond_pairs(&ann);

    let claim_label = format!("decl:{}", ground.render());
    let mut objects = vec![FactObject::new(
        ClaimId::from_label(&claim_label),
        ground,
        ann.support,
        ann.defeaters,
        conditions,
        Grade::Asserted,
    )];

    // Entity slots — the loader lowers each slot as an attr atom with
    // provenance inherited from the entity when the slot carries none
    // (`process_attr`'s `inherit` parameter), each through its own
    // `compile_ann` call so bundle-counter side effects stay in lock-step.
    if let Decl::Entity(e) = decl {
        for slot in &e.slots {
            let provenance = slot.provenance.as_ref().or(e.provenance.as_ref());
            let label = format!("slot '{}.{}'", slot.subject.text, slot.attr.text);
            let slot_ann = compile_ann(provenance, &label, pack_name, interner, bundle_counter);
            let atom = AtomExpr {
                pred: slot.attr.clone(),
                args: vec![Term::Sym(slot.subject.clone()), slot.value.clone()],
            };
            let ground = atom_expr_to_ground(&atom);
            let conditions = cond_pairs(&slot_ann);
            let claim_label = format!("decl:{}", ground.render());
            objects.push(FactObject::new(
                ClaimId::from_label(&claim_label),
                ground,
                slot_ann.support,
                slot_ann.defeaters,
                conditions,
                Grade::Asserted,
            ));
        }
    }
    objects
}

/// Convert an `Ann`'s per-monomial condition pairs into stored `CondPair`s.
fn cond_pairs(ann: &dli_core::ann::Ann) -> Vec<CondPair> {
    ann.conditions
        .iter()
        .map(|(monomial, poison)| {
            CondPair::new(
                PosBool(std::iter::once(monomial.clone()).collect()),
                poison.clone(),
            )
        })
        .collect()
}

/// Lower a question/fork's option clauses — mirrors the loader's
/// `process_options`: each option name is interned as a hypothesis variable,
/// and each `implies` atom becomes a `FactObject` with option-gated support.
fn lower_options(
    options: &[dli_ast::decl::OptionClause],
    pack_name: &str,
    interner: &mut Interner,
) -> Vec<FactObject> {
    let mut objects = Vec::new();
    for option in options {
        // Intern the option name — same side-effect as `self.intern(...)`.
        let var = interner.intern(&option.name.text);
        if let Some(implied) = &option.implies {
            let ground = atom_expr_to_ground(implied);
            let support = PosBool::pos(var);
            let claim_label = format!("option-implies:{}", ground.render());
            objects.push(FactObject::new(
                ClaimId::from_label(&claim_label),
                ground,
                support,
                PosBool::FALSE,
                Vec::new(),
                Grade::Asserted,
            ));
        }
    }
    // Suppress unused `pack_name` warning — retained for API symmetry.
    let _ = pack_name;
    objects
}

/// Extract the provenance annotation from the decl types the world loader
/// actually routes through its `compile_ann` (loader `process_decl`:
/// Entity, Attr, Fact, Before, Kind, Rule, Cycle — Question/Fork/Hypothesis
/// are special-cased in `lower_decl`). Every other declaration kind carries
/// its provenance to the *language* model's separate interner, so interning
/// it here would shift world-interner indices against a full compile.
fn decl_provenance(decl: &Decl) -> Option<&Annotation> {
    match decl {
        Decl::Entity(d) => d.provenance.as_ref(),
        Decl::Attr(d) => d.provenance.as_ref(),
        Decl::Fact(d) => d.provenance.as_ref(),
        Decl::Before(d) => d.provenance.as_ref(),
        Decl::Kind(d) => d.provenance.as_ref(),
        Decl::Rule(d) => d.provenance.as_ref(),
        Decl::Cycle(d) => d.provenance.as_ref(),
        _ => None,
    }
}

/// A surface label for the decl — used by `compile_ann` for diagnostics (which
/// we discard in `lower_decl`), but the label string itself does not affect
/// the interned variable names or the `FactObject` content hash.
fn decl_label(decl: &Decl) -> String {
    match decl {
        Decl::Entity(e) => format!("entity '{}'", e.name.text),
        Decl::Attr(a) => format!("slot '{}.{}'", a.subject.text, a.attr.text),
        Decl::Fact(f) => format!("fact '{}'", f.atom.pred.text),
        Decl::Before(b) => format!("ordering '{} -> {}'", b.earlier.text, b.later.text),
        other => format!("{other:?}"),
    }
}

/// Extract the `AtomExpr` (if any) from a decl — mirrors the loader's
/// per-declaration atom construction.
fn decl_to_atom(decl: &Decl) -> (Option<AtomExpr>, String) {
    match decl {
        Decl::Entity(e) => {
            let sort = e.kind.keyword();
            let atom = AtomExpr {
                pred: Ident {
                    text: sort.to_owned(),
                    span: e.name.span,
                },
                args: vec![Term::Sym(e.name.clone())],
            };
            let label = format!("entity '{}'", e.name.text);
            (Some(atom), label)
        }
        Decl::Attr(a) => {
            let atom = AtomExpr {
                pred: a.attr.clone(),
                args: vec![Term::Sym(a.subject.clone()), a.value.clone()],
            };
            let label = format!("slot '{}.{}'", a.subject.text, a.attr.text);
            (Some(atom), label)
        }
        Decl::Fact(f) => {
            let label = format!("fact '{}'", f.atom.pred.text);
            (Some(f.atom.clone()), label)
        }
        Decl::Before(b) => {
            let atom = AtomExpr {
                pred: Ident {
                    text: "before".to_owned(),
                    span: b.earlier.span,
                },
                args: vec![Term::Sym(b.earlier.clone()), Term::Sym(b.later.clone())],
            };
            let label = format!("ordering '{} -> {}'", b.earlier.text, b.later.text);
            (Some(atom), label)
        }
        // Questions, hypotheses, use-decls, and expert-tier forms do not lower
        // to extensional fact objects — they affect the workspace index only.
        _ => (None, String::new()),
    }
}

/// Compile an `Annotation` into an `Ann`, mirroring the loader's `compile_ann`.
///
/// On `None` provenance (required=false), returns a fail-closed annotation —
/// matches `Compiled::fail_closed()`.
fn compile_ann(
    provenance: Option<&Annotation>,
    _label: &str,
    pack: &str,
    interner: &mut Interner,
    bundle_counter: &mut u32,
) -> dli_core::ann::Ann {
    let Some(annotation) = provenance else {
        return dli_core::ann::Ann::supported(PosBool::FALSE);
    };

    // A citation-requiring marker with no citations fails closed, exactly as
    // the loader's compile_ann does — the durable object must never record
    // support the full compile would refuse.
    if annotation.marker.requires_citation() && annotation.citations.is_empty() {
        return dli_core::ann::Ann::supported(PosBool::FALSE);
    }

    let mut support = if annotation.marker == ProvenanceMarker::Proposed {
        // A proposal is gated by an open hypothesis. With an explicit
        // `assumes` condition the truth base lets the condition stand alone;
        // otherwise it enters under a fresh bundle variable — the `h.` prefix
        // applies to the tag key too, matching the loader.
        if annotation.condition.is_some() {
            PosBool::truth()
        } else {
            let key = if annotation.tags.is_empty() {
                let key = format!("{pack}#{}", *bundle_counter);
                *bundle_counter += 1;
                key
            } else {
                annotation.tags.join("+")
            };
            let var = interner.intern(&format!("h.{key}"));
            PosBool::pos(var)
        }
    } else {
        // Ledger claims: conjunct all cited decision variables into one
        // monomial, in citation order.
        let mut acc = PosBool::truth();
        for citation in &annotation.citations {
            let var = interner.intern(citation.id.text.trim());
            acc = acc.and_(&PosBool::pos(var));
        }
        acc
    };

    if let Some(condition) = &annotation.condition {
        match cond_to_posbool(condition, false, interner) {
            Ok(cond) => support = support.and_(&cond),
            // A malformed condition fails closed, never open.
            Err(_) => return dli_core::ann::Ann::supported(PosBool::FALSE),
        }
    }

    if support.is_false() {
        return dli_core::ann::Ann::supported(PosBool::FALSE);
    }

    dli_core::ann::Ann::supported(support)
}

/// Compile a `CondExpr` into a `PosBool`, interning named hypotheses — a
/// verbatim port of the loader's `cond_to_posbool`: negation is pushed to the
/// literals via De Morgan, so `not (a and b)` becomes a disjunction.
fn cond_to_posbool(
    cond: &CondExpr,
    negated: bool,
    interner: &mut Interner,
) -> Result<PosBool, String> {
    match cond {
        CondExpr::Hyp(id) => {
            let text = id.text.trim();
            if text.is_empty() {
                return Err("a hypothesis in the condition has no name".to_owned());
            }
            let atom = interner.intern(text);
            Ok(if negated {
                PosBool::neg(atom)
            } else {
                PosBool::pos(atom)
            })
        }
        CondExpr::Not(inner) => cond_to_posbool(inner, !negated, interner),
        CondExpr::And(list) => cond_group(list, negated, negated, interner),
        CondExpr::Or(list) => cond_group(list, negated, !negated, interner),
    }
}

/// Compile an `and`/`or` group — the loader's `cond_group`, ported verbatim:
/// `effective_or` is the connective *after* De Morgan, and an empty group is
/// an error, never a vacuous truth/false.
fn cond_group(
    list: &[CondExpr],
    negated: bool,
    effective_or: bool,
    interner: &mut Interner,
) -> Result<PosBool, String> {
    if list.is_empty() {
        return Err("a condition group has no members".to_owned());
    }
    let mut children = Vec::with_capacity(list.len());
    for member in list {
        children.push(cond_to_posbool(member, negated, interner)?);
    }
    let mut acc = if effective_or {
        PosBool::FALSE
    } else {
        PosBool::truth()
    };
    for child in &children {
        acc = if effective_or {
            acc.or_(child)
        } else {
            acc.and_(child)
        };
    }
    Ok(acc)
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
