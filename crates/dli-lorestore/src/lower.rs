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

use std::collections::{BTreeMap, BTreeSet};

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
use crate::object::{CondPair, ExpertBody, ExpertDeclKind, FactObject};

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

// ── claim identity ─────────────────────────────────────────────────────

/// Mint the stable claim id for a ground atom — the **one** minting scheme
/// every write path shares (audit A3). Claim continuity is keyed by the
/// proposition's surface form, never by a positional index, so inserting or
/// removing an earlier declaration renumbers nothing, and the same fact
/// reaches the same claim id through `lower_atoms`, `lower_decl`, and
/// `lower_facts` alike.
fn claim_for(ground: &dli_eval::world::GroundAtom) -> ClaimId {
    ClaimId::from_label(&format!("atom:{}", ground.render()))
}

/// Mint the stable claim id for a carried declaration.
fn claim_for_decl(kind: ExpertDeclKind, name: &str) -> ClaimId {
    ClaimId::from_label(&format!("decl:{kind:?}:{name}"))
}

/// The claim id a stored object **must** carry, re-derived from the object
/// itself — the check side of the two minting functions above.
///
/// A `ClaimId` is a claim *identity*, minted by [`ClaimId::from_label`] from
/// addressing metadata, and deliberately not a content hash of the fact
/// (`identity.rs`). That makes it the one field of an imported object that
/// nothing else can vouch for: the object's own content hash covers the bytes a
/// caller sent, so a wrong id hashes to a perfectly valid object and travels as
/// truth. Re-deriving it here is what lets the import refuse one.
///
/// The label is exactly what the write paths mint: `decl:<Kind>:<name>` for a
/// carried declaration, `atom:<pred>(<args>)` for a bare fact. Both live beside
/// the minting functions on purpose — a checker that restated the format would
/// eventually disagree with it, and then the check would be the bug.
#[must_use]
pub fn expected_claim_id(object: &FactObject) -> ClaimId {
    match &object.expert_body {
        Some(body) => claim_for_decl(body.kind, &body.name),
        None => claim_for(&object.atom),
    }
}

// ── declaration carriage ───────────────────────────────────────────────

/// Which declarations are carried whole (audit A2, DL-0045 ruling 5):
/// everything not fully captured by its extensional ground atoms. Returns the
/// carriage kind and display name, or `None` for a gentle declaration whose
/// atom objects alone reconstruct it faithfully (`Attr`, `Fact`, and a bare
/// `Entity` with no label, time clause, or slots).
fn carriage_kind(decl: &Decl) -> Option<(ExpertDeclKind, String)> {
    use ExpertDeclKind as K;
    match decl {
        Decl::Attr(_) | Decl::Fact(_) => None,
        Decl::Entity(e) => (e.label.is_some() || e.time.is_some() || !e.slots.is_empty())
            .then(|| (K::Entity, e.name.text.clone())),
        Decl::Before(b) => Some((K::Before, format!("{}->{}", b.earlier.text, b.later.text))),
        Decl::SameAs(s) => Some((K::SameAs, format!("{}={}", s.left.text, s.right.text))),
        Decl::PredicateSameAs(s) => Some((
            K::PredicateSameAs,
            format!("{}={}", s.left.text, s.right.text),
        )),
        Decl::Rule(d) => Some((K::Rule, d.name.text.clone())),
        Decl::Fork(d) => Some((K::Fork, d.question.text.clone())),
        Decl::Practice(d) => Some((K::Practice, d.name.text.clone())),
        Decl::Dialect(d) => Some((K::Dialect, d.name.text.clone())),
        Decl::Draft(d) => Some((K::Draft, d.name.text.clone())),
        Decl::Export(d) => Some((K::Export, d.profile.text.clone())),
        Decl::Name(d) => Some((K::Name, d.name.text.clone())),
        Decl::Variant(d) => Some((K::Variant, d.path.text.clone())),
        Decl::Ratify(d) => Some((K::Ratify, d.decision.text.clone())),
        Decl::Principle(d) => Some((K::Principle, d.name.text.clone())),
        Decl::Proof(d) => Some((K::Proof, d.name.text.clone())),
        Decl::Kind(d) => Some((K::Kind, d.name.text.clone())),
        Decl::State(d) => Some((K::State, d.name.text.clone())),
        Decl::Functional(d) => Some((K::Functional, d.pred.text.clone())),
        Decl::Inertial(d) => Some((K::Inertial, d.pred.text.clone())),
        Decl::Cycle(d) => Some((K::Cycle, d.name.text.clone())),
        Decl::Question(d) => Some((K::Question, d.name.text.clone())),
        Decl::Hypothesis(d) => Some((K::Hypothesis, d.name.text.clone())),
        Decl::Use(d) => Some((K::Use, d.target.text.clone())),
        Decl::Word(d) => Some((K::Word, d.name.text.clone())),
        Decl::NamePart(d) => Some((K::NamePart, d.name.text.clone())),
        Decl::Attest(d) => Some((K::Attest, d.question.text.clone())),
        Decl::Term(d) => Some((K::Term, d.name.text.clone())),
        Decl::Banned(d) => Some((K::Banned, d.word.text.clone())),
        Decl::Narration(d) => Some((K::Narration, d.head.join(" "))),
    }
}

/// Replace every source span in a serialized AST value with the synthetic
/// span, in place.
///
/// A [`Span`] serializes as exactly three unsigned-integer fields — `file`,
/// `start`, `end` — so that shape, and only that shape, is normalized: an
/// object with those three keys, no others, all non-negative integers. The
/// match is structural rather than typed because the walk has to reach spans
/// nested anywhere inside any of the carried declaration types (and inside the
/// typed interiors LS-9 introduced) without a hand-written visitor per variant
/// that would silently rot as the AST grows.
///
/// Over-matching cannot lose information: the located original is retained
/// verbatim in [`FactProvenance::located_body`] and is what reconstruction
/// reads. Normalization decides *identity* only.
pub(crate) fn normalize_spans(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let is_span = map.len() == 3
                && ["file", "start", "end"]
                    .iter()
                    .all(|key| map.get(*key).is_some_and(|v| v.as_u64().is_some()));
            if is_span {
                map.insert("file".to_owned(), serde_json::json!(0));
                map.insert("start".to_owned(), serde_json::json!(0));
                map.insert("end".to_owned(), serde_json::json!(0));
                return;
            }
            for (_, child) in map.iter_mut() {
                normalize_spans(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                normalize_spans(item);
            }
        }
        _ => {}
    }
}

/// The located and span-normalized JSON encodings of a declaration.
///
/// Both are serialized from a [`serde_json::Value`], never one from the `Decl`
/// and the other from a `Value`: those two routes can order keys differently,
/// and then the pair differs even when no span moved. Comparing like with like
/// is what lets the caller read "these differ" as "this declaration carries a
/// real source position" (DL-0055).
pub(crate) fn decl_json_pair(decl: &Decl) -> (String, String) {
    let value = serde_json::to_value(decl).expect("the AST serializes to JSON");
    let located = serde_json::to_string(&value).expect("a JSON value re-serializes");
    let mut normalized_value = value;
    normalize_spans(&mut normalized_value);
    let normalized = serde_json::to_string(&normalized_value).expect("a JSON value re-serializes");
    (located, normalized)
}

/// Build the carriage object for a declaration: the whole `Decl` as canonical
/// JSON inside an [`ExpertBody`], under a synthetic `__decl` atom. Support is
/// truth and defeaters false — the declaration's *compiled* provenance is
/// re-derived on recompile; the carriage binds the source form only.
///
/// The hashed body is **span-normalized**: source position is not part of what
/// a declaration asserts, so two identical declarations at different offsets
/// must be one object, or dedup and cross-branch merge convergence break
/// (DL-0055). The located original rides along in `provenance.located_body`,
/// outside the hash, so positions survive the round trip.
fn carriage_object(decl: &Decl, kind: ExpertDeclKind, name: &str) -> FactObject {
    let (located, body) = decl_json_pair(decl);
    let ground = dli_eval::world::GroundAtom {
        pred: "__decl".to_owned(),
        args: vec![format!("{kind:?}"), name.to_owned()],
    };
    let object = FactObject::with_expert_body(
        claim_for_decl(kind, name),
        ground,
        PosBool::truth(),
        PosBool::FALSE,
        Vec::new(),
        Grade::Asserted,
        ExpertBody::new(kind, name, Vec::new(), vec![body.clone()]),
    );
    // Only record the located body when normalization actually moved
    // something: a declaration with synthetic spans throughout would otherwise
    // carry a byte-identical duplicate for nothing.
    object.with_provenance(crate::object::FactProvenance {
        marker: None,
        located_body: (located != body).then_some(located),
        ..Default::default()
    })
}

/// The rendered ground atoms a carried declaration regenerates on recompile —
/// the reconstruction walk skips exactly these if they follow the carriage
/// object in the stream. Identity-based (not count-based) because commit-chain
/// deduplication can elide an atom that already exists in an earlier commit.
#[must_use]
#[allow(clippy::match_same_arms)]
pub fn carried_atom_renders(decl: &Decl) -> Vec<String> {
    let implies_renders = |options: &[dli_ast::decl::OptionClause]| {
        options
            .iter()
            .filter_map(|o| o.implies.as_ref())
            .map(|implied| atom_expr_to_ground(implied).render())
            .collect::<Vec<_>>()
    };
    match decl {
        Decl::Entity(e) => {
            let mut renders = Vec::with_capacity(1 + e.slots.len());
            renders.push(
                dli_eval::world::GroundAtom {
                    pred: e.kind.keyword().to_owned(),
                    args: vec![e.name.text.clone()],
                }
                .render(),
            );
            for slot in &e.slots {
                let atom = AtomExpr {
                    pred: slot.attr.clone(),
                    args: vec![Term::Sym(slot.subject.clone()), slot.value.clone()],
                };
                renders.push(atom_expr_to_ground(&atom).render());
            }
            renders
        }
        Decl::Before(b) => vec![
            dli_eval::world::GroundAtom {
                pred: "before".to_owned(),
                args: vec![b.earlier.text.clone(), b.later.text.clone()],
            }
            .render(),
        ],
        Decl::SameAs(s) => vec![
            dli_eval::world::GroundAtom {
                pred: "same-as".to_owned(),
                args: vec![s.left.text.clone(), s.right.text.clone()],
            }
            .render(),
        ],
        Decl::PredicateSameAs(_) => Vec::new(),
        Decl::Question(q) => implies_renders(&q.options),
        Decl::Fork(f) => implies_renders(&f.options),
        // A state is carried whole *and* regenerates its mirrored attribute
        // atom on recompile (issues 47/48) — the same seam the loader and the
        // evaluator lower through, so the skip stays exact.
        Decl::State(s) => s
            .atom()
            .map(|atom| vec![atom_expr_to_ground(&atom).render()])
            .unwrap_or_default(),
        // Attr/Fact are never carried; other declarations regenerate no atoms.
        _ => Vec::new(),
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
/// Only the fact-producing declarations yield atom objects: `Entity`, `Attr`
/// (slot), `Fact` (relation), `Before`, and `State` (the mirrored attribute of
/// a fluent — issues 47/48). Returns an empty vector for all others.
///
/// **Must produce byte-identical `FactObject`s to [`lower_atoms`] given the
/// same interner state** — the total-lowering oracle verifies this.
pub fn lower_decl(
    decl: &Decl,
    pack_name: &str,
    interner: &mut Interner,
    bundle_counter: &mut u32,
) -> Vec<FactObject> {
    // Carried declarations emit their carriage object FIRST, before any atom
    // objects they imply — reconstruction relies on that order to skip the
    // regenerated atoms. Carriage emission has no interner side effect, so
    // interner order stays in lock-step with the loader regardless.
    let mut objects: Vec<FactObject> = Vec::new();
    if let Some((kind, name)) = carriage_kind(decl) {
        objects.push(carriage_object(decl, kind, &name));
    }

    // Question and Fork declarations don't process their own provenance in the
    // loader — only their options (which intern names and produce implied-atom
    // fact objects). Handle them specially.
    if let Decl::Question(q) = decl {
        objects.extend(lower_options(&q.options, pack_name, interner));
        return objects;
    }
    if let Decl::Fork(f) = decl {
        objects.extend(lower_options(&f.options, pack_name, interner));
        return objects;
    }

    // Hypothesis declarations intern their name as a variable — the carriage
    // object preserves the declaration; the interning keeps `suppose` and
    // `assumes` references resolvable.
    if let Decl::Hypothesis(h) = decl {
        let _ = interner.intern(&h.name.text);
        return objects;
    }

    // Always intern the decl's provenance variables. For fact-producing
    // decls, the resulting Ann is reused below.
    let provenance = decl_provenance(decl);
    let label = decl_label(decl);
    let ann = provenance.map(|p| compile_ann(Some(p), &label, pack_name, interner, bundle_counter));

    let (atom_expr, _) = decl_to_atom(decl);
    let Some(atom_expr) = atom_expr else {
        return objects; // carried declarations with no extensional atom
    };

    let ann = ann.unwrap_or_else(|| dli_core::ann::Ann::supported(PosBool::FALSE));
    let ground = atom_expr_to_ground(&atom_expr);
    let conditions = cond_pairs(&ann);

    // The authored marker is recorded beside the fact, not inside its hash
    // (DL-0055): `decided`, `ratified`, and `provisional` lower to the same
    // citation conjunction, so without this the round trip cannot tell them
    // apart and reverse lowering has to guess.
    objects.push(
        FactObject::new(
            claim_for(&ground),
            ground,
            ann.support,
            ann.defeaters,
            conditions,
            Grade::Asserted,
        )
        .with_provenance(crate::object::FactProvenance {
            marker: provenance.map(|p| p.marker),
            located_body: None,
            ..Default::default()
        }),
    );

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
            objects.push(
                FactObject::new(
                    claim_for(&ground),
                    ground,
                    slot_ann.support,
                    slot_ann.defeaters,
                    conditions,
                    Grade::Asserted,
                )
                .with_provenance(crate::object::FactProvenance {
                    marker: provenance.map(|p| p.marker),
                    located_body: None,
                    ..Default::default()
                }),
            );
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
            objects.push(FactObject::new(
                claim_for(&ground),
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
/// Entity, Attr, Fact, Before, Kind, Rule, Cycle, State — Question/Fork/Hypothesis
/// are special-cased in `lower_decl`). Every other declaration kind carries
/// its provenance to the *language* model's separate interner, so interning
/// it here would shift world-interner indices against a full compile.
fn decl_provenance(decl: &Decl) -> Option<&Annotation> {
    match decl {
        Decl::Entity(d) => d.provenance.as_ref(),
        Decl::Attr(d) => d.provenance.as_ref(),
        Decl::Fact(d) => d.provenance.as_ref(),
        Decl::Before(d) => d.provenance.as_ref(),
        Decl::SameAs(d) => d.provenance.as_ref(),
        Decl::PredicateSameAs(d) => d.provenance.as_ref(),
        Decl::Kind(d) => d.provenance.as_ref(),
        Decl::Rule(d) => d.provenance.as_ref(),
        Decl::Cycle(d) => d.provenance.as_ref(),
        // A state that names nothing evaluable is inert: the loader records the
        // finding and returns *before* its `compile_ann`, so interning the
        // provenance here would mint a bundle variable the full compile never
        // does and desync every index after it.
        Decl::State(d) => d.atom().and(d.provenance.as_ref()),
        // `inertial <pred>` (DL-0081) is carriage-only, but — unlike
        // `functional` — the loader's `process_inertial` DOES call `compile_ann`
        // (its declaration is a defeasible claim whose standing gates capping).
        // So its provenance must be interned here too, or `import_pack` would
        // see the walk mint fewer variables than the full compile and refuse the
        // pack as `LoweringDiverged`.
        Decl::Inertial(d) => d.provenance.as_ref(),
        // Carriage-only declarations intern no provenance here. A
        // `functional <pred>` declaration (issue 49) is one: the loader records
        // the predicate but never calls `compile_ann`, so returning its
        // provenance here would mint a bundle variable the full compile never does
        // and desync every index after it. Its provenance rides the serialized
        // carriage body instead, keeping the three lowerings byte-identical.
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
        Decl::SameAs(s) => format!("identity '{} = {}'", s.left.text, s.right.text),
        Decl::PredicateSameAs(s) => {
            format!("predicate identity '{} = {}'", s.left.text, s.right.text)
        }
        Decl::State(s) => format!("state '{}'", s.name.text),
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
        Decl::SameAs(s) => {
            let atom = AtomExpr {
                pred: Ident {
                    text: "same-as".to_owned(),
                    span: s.left.span,
                },
                args: vec![Term::Sym(s.left.clone()), Term::Sym(s.right.clone())],
            };
            let label = format!("identity '{} = {}'", s.left.text, s.right.text);
            (Some(atom), label)
        }
        // A state mirrors an attribute so it becomes queryable (issues 47/48):
        // it lowers through the one shared seam `StateDecl::atom`, the same the
        // loader and the evaluator use, so the three cannot drift. An undotted
        // name with no value names nothing evaluable — the carriage object
        // alone carries it, as in the loader.
        Decl::State(s) => (s.atom(), format!("state '{}'", s.name.text)),
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
        .map(|la| {
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
            FactObject::new(
                claim_for(&ground),
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
        .map(|fact| {
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
            FactObject::new(
                claim_for(&fact.atom),
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

/// Turn a positive-Boolean polynomial back into the `CondExpr` DNF that
/// compiles to it: each monomial becomes an `And` of hypotheses (negative
/// rails wrapped in `Not`), the polynomial an `Or` of monomials. Recompiling
/// through `cond_to_posbool` reproduces the polynomial exactly, because the
/// stored form is already absorption-normalized DNF.
fn polynomial_to_condexpr(p: &PosBool, interner: &Interner) -> Result<CondExpr, String> {
    let mut monomials: Vec<CondExpr> = Vec::new();
    for m in p.monomials() {
        let mut lits: Vec<CondExpr> = Vec::new();
        for lit in m.lits() {
            let name = interner
                .resolve(lit.atom)
                .ok_or_else(|| format!("condition variable atom{} has no name", lit.atom.0))?;
            let hyp = CondExpr::Hyp(Ident::synthetic(name));
            lits.push(match lit.rail {
                Rail::Pos => hyp,
                Rail::Neg => CondExpr::Not(Box::new(hyp)),
            });
        }
        monomials.push(if lits.len() == 1 {
            lits.pop().expect("one element")
        } else {
            CondExpr::And(lits)
        });
    }
    Ok(if monomials.len() == 1 {
        monomials.pop().expect("one element")
    } else {
        CondExpr::Or(monomials)
    })
}

/// Derive an [`Annotation`] from a support polynomial.
///
/// The support polynomial is the canonical encoding of provenance, factored
/// back as follows:
///
/// - `FALSE` — fail-closed: no annotation at all.
/// - Citations: positive known-decision variables present in **every**
///   monomial (a ledger claim conjoins its citations). Marker `Ratified` —
///   the loader treats `Decided`/`Ratified`/`Provisional` identically for
///   valuation, so the fixpoint is unaffected.
/// - After stripping citations, a residual single positive `h.<pack>#N`
///   literal is a positional proposal bundle (re-minted on recompile, so no
///   tags); a residual single positive `h.<key>` literal is a tag-gated
///   proposal with `tags = key.split('+')`.
/// - Any other residual is an `assumes` condition, factored back to
///   `CondExpr` DNF (each monomial an `And`, the polynomial an `Or`).
/// - A residual that *mixes* `h.`-gates with condition variables cannot be
///   factored — that shape is refused, never guessed (DL-0045 ruling 5).
#[allow(clippy::too_many_lines)]
pub fn derive_annotation(
    support: &PosBool,
    interner: &Interner,
    known_decisions: &BTreeSet<String>,
    pack_name: &str,
) -> Result<Option<Annotation>, String> {
    if support.is_false() {
        return Ok(None); // fail-closed: no provenance (spec 02 §3)
    }

    let annotation = |marker, citations: Vec<String>, condition, tags| {
        Some(Annotation {
            marker,
            citations: citations
                .into_iter()
                .map(|c| CitationRef {
                    id: Ident::synthetic(c),
                })
                .collect(),
            condition,
            tags,
            span: Span::synthetic(),
        })
    };

    // Citations: positive known-decision names present in every monomial.
    let monomial_names: Vec<Vec<(String, Rail)>> = support
        .monomials()
        .map(|m| {
            m.lits()
                .map(|lit| {
                    interner
                        .resolve(lit.atom)
                        .map(|n| (n.to_owned(), lit.rail))
                        .ok_or_else(|| format!("support variable atom{} has no name", lit.atom.0))
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .collect::<Result<Vec<_>, String>>()?;

    let is_citation =
        |(name, rail): &(String, Rail)| *rail == Rail::Pos && known_decisions.contains(name);
    let citations: Vec<String> = monomial_names
        .first()
        .map(|first| {
            first
                .iter()
                .filter(|l| is_citation(l))
                .filter(|l| monomial_names.iter().all(|m| m.contains(l)))
                .map(|(n, _)| n.clone())
                .collect()
        })
        .unwrap_or_default();

    // The residual polynomial: every monomial minus the shared citations.
    let residual: Vec<Vec<(String, Rail)>> = monomial_names
        .iter()
        .map(|m| {
            m.iter()
                .filter(|l| !(is_citation(l) && citations.contains(&l.0)))
                .cloned()
                .collect()
        })
        .collect();
    let residual_empty = residual.iter().all(Vec::is_empty);
    let marker = if citations.is_empty() {
        ProvenanceMarker::Proposed
    } else {
        ProvenanceMarker::Ratified
    };

    if residual_empty {
        return Ok(annotation(marker, citations, None, Vec::new()));
    }

    // A single residual literal that is an `h.` gate: positional bundle or tags.
    let bundle_prefix = format!("h.{pack_name}#");
    if residual.len() == 1 && residual[0].len() == 1 && residual[0][0].1 == Rail::Pos {
        let name = &residual[0][0].0;
        if let Some(counter) = name.strip_prefix(&bundle_prefix)
            && counter.parse::<u32>().is_ok()
        {
            // Positional proposal bundle — re-minted on recompile.
            return Ok(annotation(
                ProvenanceMarker::Proposed,
                citations,
                None,
                Vec::new(),
            ));
        }
        if let Some(key) = name.strip_prefix("h.") {
            return Ok(annotation(
                ProvenanceMarker::Proposed,
                citations,
                None,
                key.split('+').map(str::to_owned).collect(),
            ));
        }
    }

    // Any `h.` gate mixed into a wider residual is unfactorable — refuse.
    if residual
        .iter()
        .flatten()
        .any(|(name, _)| name.starts_with("h."))
    {
        return Err(format!(
            "support mixes proposal gates with condition variables and cannot be factored: {residual:?}"
        ));
    }

    // The residual is an `assumes` condition in DNF.
    let residual_poly = {
        let mut acc = PosBool::FALSE;
        for m in &residual {
            let mut mono = PosBool::truth();
            for (name, rail) in m {
                let atom = interner
                    .names()
                    .iter()
                    .position(|n| n == name)
                    .map(|i| dli_core::atom::Atom(u32::try_from(i).expect("interner fits u32")))
                    .ok_or_else(|| format!("condition variable `{name}` not in interner"))?;
                let lit_poly = match rail {
                    Rail::Pos => PosBool::pos(atom),
                    Rail::Neg => PosBool::neg(atom),
                };
                mono = mono.and_(&lit_poly);
            }
            acc = acc.or_(&mono);
        }
        acc
    };
    let condition = polynomial_to_condexpr(&residual_poly, interner)?;
    Ok(annotation(marker, citations, Some(condition), Vec::new()))
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

/// A fact-object stream that cannot be faithfully reconstructed. Loud by
/// design (DL-0045 ruling 5): the store must refuse what it cannot round-trip,
/// never compile a silently different world.
#[derive(Debug, thiserror::Error)]
pub enum ReconstructError {
    /// A carried declaration failed to decode.
    #[error("carried declaration `{name}` does not decode: {reason}")]
    Carriage {
        /// The carriage object's display name.
        name: String,
        /// Why decoding failed.
        reason: String,
    },
    /// A base fact object carries state the declaration forms cannot express.
    #[error("cannot reconstruct `{atom}`: {reason}")]
    Unfaithful {
        /// The rendered ground atom.
        atom: String,
        /// What could not be expressed.
        reason: String,
    },
}

type CarriedFactKey = (crate::identity::ContentHash, ClaimId);
type MarkerIndex = BTreeMap<CarriedFactKey, (Option<ProvenanceMarker>, bool)>;

/// Preserve current metadata for the cited entities whose generated fact has
/// a stable identity independent of positional proposal numbering.
struct CitedEntityReplay<'a> {
    objects: &'a [FactObject],
    interner: &'a Interner,
    pack_name: &'a str,
    replay_interner: Option<Interner>,
    markers: Option<MarkerIndex>,
}

impl CitedEntityReplay<'_> {
    fn prepare(
        &mut self,
        decl: &mut Decl,
        name: &str,
    ) -> Result<Option<CarriedFactKey>, ReconstructError> {
        let eligible = matches!(decl, Decl::Entity(entity)
            if entity.slots.is_empty() && entity.provenance.as_ref()
                .is_some_and(|ann| ann.marker.requires_citation()));
        if !eligible {
            return Ok(None);
        }
        let replay = self
            .replay_interner
            .get_or_insert_with(|| self.interner.clone());
        let initial_names = replay.names().len();
        let generated = lower_decl(decl, self.pack_name, replay, &mut 0);
        if replay.names().len() != initial_names {
            return Err(ReconstructError::Carriage {
                name: name.to_owned(),
                reason: "entity provenance is absent from the committed interner".to_owned(),
            });
        }
        let atom = generated
            .iter()
            .find(|object| object.expert_body.is_none())
            .ok_or_else(|| ReconstructError::Carriage {
                name: name.to_owned(),
                reason: "entity did not regenerate an atom".to_owned(),
            })?;
        let key = (
            atom.content_hash()
                .map_err(|error| ReconstructError::Carriage {
                    name: name.to_owned(),
                    reason: error.to_string(),
                })?,
            atom.claim_id.clone(),
        );
        let markers = self
            .markers
            .get_or_insert_with(|| marker_index(self.objects));
        if let Some((_, true)) = markers.get(&key) {
            return Err(ReconstructError::Carriage {
                name: name.to_owned(),
                reason: "conflicting standing metadata for the same entity fact".to_owned(),
            });
        }
        if let Some((Some(marker), false)) = markers.get(&key)
            && marker.requires_citation()
            && let Decl::Entity(entity) = decl
            && let Some(annotation) = &mut entity.provenance
        {
            annotation.marker = *marker;
        }
        Ok(Some(key))
    }
}

fn marker_index(objects: &[FactObject]) -> MarkerIndex {
    let mut markers: MarkerIndex = BTreeMap::new();
    for object in objects.iter().filter(|object| object.expert_body.is_none()) {
        let Ok(hash) = object.content_hash() else {
            continue;
        };
        let marker = object.provenance.as_ref().and_then(|p| p.marker);
        markers
            .entry((hash, object.claim_id.clone()))
            .and_modify(|(prior, conflict)| *conflict |= *prior != marker)
            .or_insert((marker, false));
    }
    markers
}

/// Re-emit known decisions not already present as carried ratifications.
fn missing_ratify_decls(
    fact_objects: &[FactObject],
    known_decisions: &BTreeSet<String>,
) -> Vec<Decl> {
    let mut decls: Vec<Decl> = Vec::new();

    // Ratify prelude — one synthesized `ratify` per known decision that no
    // carried `ratify` object already replays. A pure-prelude stream (no
    // carriage at all) still gets a prelude for every decision, and a
    // pure-carriage stream (every decision carried, e.g. a `defeat` chain) gets
    // none — both unchanged. The load-bearing case is the MIX: a stream that
    // carries one `ratify` (an `author.record_decision`/`defeat` carriage) over
    // a base whose earlier decisions were only ever in the commit's `decisions`
    // list. Under the old all-or-nothing rule the first carriage suppressed the
    // whole prelude, silently un-declaring every list-only decision and making
    // every fact resting on one uncertain (DL-0083, the synthetic-prelude /
    // carriage byte-identity risk). Covering exactly the uncarried decisions
    // keeps all of them declared, byte-identically, whether by the list or by
    // carriage.
    let carried: BTreeSet<&str> = fact_objects
        .iter()
        .filter_map(|fo| fo.expert_body.as_ref())
        .filter(|body| body.kind == ExpertDeclKind::Ratify)
        .map(|body| body.name.as_str())
        .collect();
    for decision in known_decisions {
        if !carried.contains(decision.as_str()) {
            decls.push(Decl::Ratify(RatifyDecl {
                decision: Ident::synthetic(decision),
                body: dli_ast::language::RatifyBody::default(),
            }));
        }
    }

    decls
}

/// Reconstruct a [`Pack`] from a set of [`FactObject`]s, ready for
/// recompilation.
///
/// Carried declarations (`expert_body` present) decode back to their exact
/// source `Decl` and **skip** the atom objects they regenerate on recompile
/// ([`carried_atom_renders`]). Plain atom objects reconstruct as
/// `Entity`/`Fact` declarations with the `Annotation` derived from the stored
/// support polynomial — citations, tags, and `assumes` conditions included.
/// Anything that cannot be reconstructed faithfully is a
/// [`ReconstructError`], never a silently different world.
///
/// Streams lowered from `LoadedAtom`s alone (no carriage objects — the
/// compile-side oracle path) get a `Ratify` prelude from `known_decisions`,
/// preserving the historical interner growth order.
///
/// The `pack_name` seeds the proposal-bundle variable naming — it must match
/// the original pack's scope header.
pub fn reconstruct_pack(
    fact_objects: &[FactObject],
    interner: &Interner,
    known_decisions: &BTreeSet<String>,
    pack_name: &str,
) -> Result<Pack, ReconstructError> {
    let mut decls = missing_ratify_decls(fact_objects, known_decisions);

    let mut cited_entities = CitedEntityReplay {
        objects: fact_objects,
        interner,
        pack_name,
        replay_interner: None,
        markers: None,
    };
    let mut index = 0;
    while index < fact_objects.len() {
        let fo = &fact_objects[index];

        if let Some(body) = &fo.expert_body {
            // The located body when there is one, else the normalized body —
            // reconstruction is a fidelity path (DL-0055).
            let json = fo
                .carriage_json()
                .ok_or_else(|| ReconstructError::Carriage {
                    name: body.name.clone(),
                    reason: "carriage body is empty".to_owned(),
                })?;
            let mut decl: Decl =
                serde_json::from_str(json).map_err(|e| ReconstructError::Carriage {
                    name: body.name.clone(),
                    reason: e.to_string(),
                })?;
            let cited_entity_identity = cited_entities.prepare(&mut decl, &body.name)?;
            // Skip the atom objects this declaration regenerates on recompile
            // — by identity, in order, tolerating absences (commit-chain
            // dedup can elide an atom already present in an earlier commit).
            index += 1;
            for expected in carried_atom_renders(&decl) {
                if fact_objects.get(index).is_some_and(|next| {
                    next.expert_body.is_none()
                        && next.atom.render() == expected
                        && cited_entity_identity
                            .as_ref()
                            .is_none_or(|(hash, claim_id)| {
                                next.claim_id == *claim_id
                                    && next.content_hash().ok().as_ref() == Some(hash)
                            })
                }) {
                    index += 1;
                }
            }
            decls.push(decl);
            continue;
        }

        // Base atoms must not carry state the declaration forms cannot say.
        if !fo.defeaters.is_false() {
            return Err(ReconstructError::Unfaithful {
                atom: fo.atom.render(),
                reason: "a base fact with a non-false defeater polynomial".to_owned(),
            });
        }
        let annotation = derive_annotation(&fo.support, interner, known_decisions, pack_name)
            .map_err(|reason| ReconstructError::Unfaithful {
                atom: fo.atom.render(),
                reason,
            })?;
        // The polynomial cannot distinguish `decided` / `ratified` /
        // `provisional` — all three are the same citation conjunction — so
        // `derive_annotation` answers `Ratified` for any cited claim. When the
        // object recorded what it was actually authored as, that wins
        // (DL-0055). A v2 object records nothing and keeps the derived answer.
        let annotation = match (annotation, fo.provenance.as_ref().and_then(|p| p.marker)) {
            (Some(mut annotation), Some(marker))
                if marker != ProvenanceMarker::Proposed
                    && annotation.marker != ProvenanceMarker::Proposed =>
            {
                annotation.marker = marker;
                Some(annotation)
            }
            (annotation, _) => annotation,
        };

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
        index += 1;
    }

    let scope = if pack_name.is_empty() {
        None
    } else {
        Some(ScopeHeader {
            name: Ident::synthetic(pack_name),
            span: Span::synthetic(),
        })
    };

    let pack = dli_world::pack::Pack {
        name: pack_name.to_owned(),
        documents: vec![Document { scope, decls }],
    };
    ensure_positional_proposals_survive(fact_objects, interner, &pack, pack_name)?;
    Ok(pack)
}

/// Compare anonymous proposal gates by name after reconstruction. The source
/// form remints these gates positionally, so omitted declarations or collapsed
/// scopes can silently attach a fact to a different hypothesis.
fn ensure_positional_proposals_survive(
    objects: &[FactObject],
    interner: &Interner,
    pack: &Pack,
    pack_name: &str,
) -> Result<(), ReconstructError> {
    fn positional(name: &str) -> bool {
        name.strip_prefix("h.")
            .and_then(|value| value.rsplit_once('#'))
            .is_some_and(|(_, number)| {
                !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
            })
    }
    type ProposalSignature = (dli_eval::world::GroundAtom, Vec<Vec<(String, Rail)>>);
    fn collect(
        objects: &[FactObject],
        interner: &Interner,
    ) -> Result<BTreeSet<ProposalSignature>, ReconstructError> {
        let mut signatures = BTreeSet::new();
        for object in objects.iter().filter(|object| object.expert_body.is_none()) {
            let mut paths = Vec::new();
            let mut has_positional = false;
            for monomial in object.support.monomials() {
                let mut literals = Vec::new();
                for lit in monomial.lits() {
                    let name =
                        interner
                            .resolve(lit.atom)
                            .ok_or_else(|| ReconstructError::Unfaithful {
                                atom: object.atom.render(),
                                reason: "proposal support references an unknown variable"
                                    .to_owned(),
                            })?;
                    has_positional |= positional(name);
                    literals.push((name.to_owned(), lit.rail));
                }
                literals.sort();
                paths.push(literals);
            }
            if has_positional {
                paths.sort();
                signatures.insert((object.atom.clone(), paths));
            }
        }
        Ok(signatures)
    }
    let source = collect(objects, interner)?;
    // A carried declaration can regenerate a proposal even when no matching
    // fact object survives in the stream. Audit those too rather than assuming
    // an empty source signature means there is no proposal to change.
    if source.is_empty() && !pack.documents.iter().flat_map(|doc| &doc.decls).any(|decl| {
        decl_provenance(decl).is_some_and(|annotation| annotation.marker == ProvenanceMarker::Proposed)
            || matches!(decl, Decl::Entity(entity) if entity.slots.iter().any(|slot| slot.provenance.as_ref().is_some_and(|annotation| annotation.marker == ProvenanceMarker::Proposed)))
    }) {
        return Ok(());
    }
    let prefix = format!("h.{pack_name}#");
    if source
        .iter()
        .flat_map(|(_, monomials)| monomials)
        .flatten()
        .any(|(name, _)| positional(name) && !name.starts_with(&prefix))
    {
        return Err(ReconstructError::Unfaithful {
            atom: "proposal".to_owned(),
            reason: "anonymous proposal belongs to a different source scope".to_owned(),
        });
    }
    let (regenerated, names) = lower_pack(pack);
    if collect(&regenerated, &Interner::from_names(names))? != source {
        return Err(ReconstructError::Unfaithful {
            atom: "proposal".to_owned(),
            reason: "anonymous proposal identities change during reconstruction".to_owned(),
        });
    }
    Ok(())
}

/// Lower a whole pack per-declaration, in document order — the store's write
/// path. Uses each document's scope-header name — falling back to the **empty**
/// name, exactly as the loader does — for proposal-bundle minting. Returns the
/// lowered objects and the interner name table the walk built; callers refuse
/// the result when that table diverges from the compiled model's (loud
/// totality — the walk must track the loader or the pack does not import).
#[must_use]
pub fn lower_pack(pack: &Pack) -> (Vec<FactObject>, Vec<String>) {
    let mut interner = Interner::new();
    // One bundle counter across the whole pack — the loader's counter is a
    // LoadCtx field that never resets between documents, even though the
    // minted prefix follows each document's scope name.
    let mut bundle_counter = 0u32;
    let mut objects: Vec<FactObject> = Vec::new();
    for doc in &pack.documents {
        // A document with no `scope` header belongs to the **empty** pack, not
        // to the store's pack name. That is the loader's own rule
        // (`dli_world::loader::Loader::load` falls back to `String::new()`),
        // and this walk exists to track the loader exactly — where the two
        // disagree the store refuses the import as `LoweringDiverged`.
        //
        // Falling back to `pack.name` here is what made
        // `dli.corpus.import { dli_documents }` refuse **every** scope-less
        // corpus: the walk minted `h.<pack>#0` while the compiled model minted
        // `h.#0`, diverging at index 0 on the first proposal. It went unnoticed
        // because every committed fixture writes a `scope` header, where the
        // two agree, and because an empty document mints no bundle at all.
        let scope_name = doc.scope.as_ref().map_or("", |s| s.name.text.as_str());
        for decl in &doc.decls {
            objects.extend(lower_decl(
                decl,
                scope_name,
                &mut interner,
                &mut bundle_counter,
            ));
        }
    }
    (objects, interner.names().to_vec())
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
