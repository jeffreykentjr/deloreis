//! `LS-1` total-lowering oracle.
//!
//! This integration test closes the loop the roadmap (doc/PLAN.md § LS-1,
//! `docs/design-notes/lorestore-roadmap.md` §LS-1) demands:
//!
//! > every AST construct lowers to fact objects, and lowering the vendored
//! > corpus then compiling reproduces identical fixpoint, findings, and digests
//!
//! Flow — shadow-read harness idiom (same as the MIG/PROOF lanes in
//! `crates/dli-world/tests/check.rs`):
//!
//! 1.  Bridge the vendored `facts.yaml` into `Decl`s (same `corpus_pack`
//!     logic the MIG lane uses).
//! 2.  Compile via `dli_world::compile` → **reference A**.
//! 3.  Lower A's `LoadedAtom`s to `FactObject`s and compute their
//!     content-address digests (verifying determinism by hashing twice).
//! 4.  Reconstruct source `Decl`s from those `FactObject`s — deriving each
//!     `Annotation` from the support polynomial via the committed interner.
//! 5.  Recompile the reconstructed documents → **reference B**.
//! 6.  Compare A and B: rendered fixpoint (`World::derived`), findings, and
//!     the set of `FactObject` digests.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
use common::{corpus_pack, load_corpus, render_derived, render_facts, render_findings};

use dli_ast::decl::Decl;

use dli_core::posbool::PosBool;

use dli_lorestore::identity::ClaimId;
use dli_lorestore::lower::{declared_decisions, derive_annotation, lower_atoms, reconstruct_pack};
use dli_lorestore::object::FactObject;

use dli_world::compile;

// ── comparison helpers are shared via `common/mod.rs` ───────────────────

// ── the oracle ────────────────────────────────────────────────────────

#[test]
#[allow(clippy::too_many_lines, clippy::print_stdout)]
fn total_lowering_reproduces_fixpoint_findings_and_digests() {
    // 1 — bridge the vendored corpus.
    let pack = corpus_pack(&load_corpus());

    // 2 — compile reference A.
    let compiled_a = compile(&pack).expect("reference A compiles");
    let model_a = compiled_a.model();

    // 3 — lower A's LoadedAtoms to FactObjects.
    //    Using LoadedAtom (not just World::facts) captures entity declarations
    //    too, which is what the loader interns as proposal-bundle variables.
    let fact_objects = lower_atoms(model_a.atoms());

    //    Totality: every loaded atom lowers to exactly one FactObject.
    assert_eq!(
        fact_objects.len(),
        model_a.atoms().len(),
        "every LoadedAtom must lower to exactly one FactObject (DL-0041)"
    );

    //    Determinism: hashing the objects twice must yield identical digests.
    let digests_a: Vec<String> = fact_objects
        .iter()
        .map(|fo| fo.content_hash().expect("canonical encoding").to_string())
        .collect();
    let digests_a_again: Vec<String> = fact_objects
        .iter()
        .map(|fo| fo.content_hash().expect("canonical encoding").to_string())
        .collect();
    assert_eq!(
        digests_a, digests_a_again,
        "FactObject digests must be deterministic over canonical bytes"
    );

    //    No two FactObjects in the same pack may share a digest.
    let mut unique = std::collections::HashSet::new();
    for d in &digests_a {
        assert!(
            unique.insert(d.clone()),
            "duplicate FactObject digest — content-address collision: {d}"
        );
    }

    // 4 — collect known decisions (minted via Decl::Ratify) so the
    //    reconstructed pack can re-emit them.
    let known_decisions = declared_decisions(&pack);

    //    Derive annotations from FactObject support polynomials and verify
    //    that every citation resolves to a known decision.
    let interner_a = model_a.interner();
    for fo in &fact_objects {
        if let Some(ann) = derive_annotation(&fo.support, interner_a, &known_decisions, &pack.name)
            .expect("annotation derives")
        {
            for cite in &ann.citations {
                assert!(
                    known_decisions.contains(&cite.id.text),
                    "reconstructed citation {} was not a declared decision",
                    cite.id.text
                );
            }
        }
    }

    // 5 — reconstruct source Decls from the FactObjects.
    let reconstructed = reconstruct_pack(&fact_objects, interner_a, &known_decisions, &pack.name)
        .expect("the lowered corpus reconstructs");

    //    The reconstructed documents carry ratify decls first, then entity decls,
    //    then fact decls — matching the original pack's ordering.
    let ratify_count = reconstructed
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .filter(|d| matches!(d, Decl::Ratify(_)))
        .count();
    assert_eq!(ratify_count, known_decisions.len());

    // 6 — compile the reconstructed pack (reference B).
    let compiled_b = compile(&reconstructed).expect("reference B compiles");
    let model_b = compiled_b.model();

    // ---- fidelity checks ---------------------------------------------------

    // (a) extensional facts match (by surface form + support name-strings).
    let facts_a = render_facts(&model_a.world);
    let facts_b = render_facts(&model_b.world);
    assert_eq!(
        facts_a, facts_b,
        "extensional facts must be identical after round-trip"
    );

    // (b) fixpoint (derived relation) is identical.
    let derived_a = render_derived(&model_a.world);
    let derived_b = render_derived(&model_b.world);
    assert_eq!(
        derived_a, derived_b,
        "fixpoint must be identical after round-trip"
    );
    assert_eq!(
        model_a.world.derived.len(),
        model_b.world.derived.len(),
        "derived relation row count must match"
    );

    // (c) load findings are identical (the clean corpus must be clean on both
    //     sides — charter koan 4).
    assert_eq!(
        render_findings(model_a.findings()),
        render_findings(model_b.findings()),
        "load findings must be identical after round-trip"
    );

    // (d) valuation (truth assignment over resolved names) matches.
    let names_a = model_a.interner().names().to_vec();
    let names_b = model_b.interner().names().to_vec();
    assert_eq!(names_a, names_b, "interned variable sets must match");
    for (&atom_idx, val_a) in &model_a.world.valuation.0 {
        let name = model_a.interner().resolve(atom_idx).expect("atom has name");
        let b_idx = dli_core::atom::Atom(
            u32::try_from(
                names_b
                    .iter()
                    .position(|n| n == name)
                    .expect("atom exists in B interner"),
            )
            .expect("fits in u32"),
        );
        let val_b = model_b.world.valuation.get(b_idx).unwrap_or(false);
        assert_eq!(
            *val_a, val_b,
            "valuation disagree on variable '{name}': A={val_a}, B={val_b}"
        );
    }

    // (e) FactObject digests from A and B must match — the interner is
    //     rebuilt identically (ratify-first ordering) so atom indices line up
    //     and canonical bytes are byte-for-byte stable.
    let fact_objects_b = lower_atoms(model_b.atoms());
    let digests_b: Vec<String> = fact_objects_b
        .iter()
        .map(|fo| fo.content_hash().expect("canonical encoding").to_string())
        .collect();
    assert_eq!(
        digests_a, digests_b,
        "FactObject digests must match after round-trip (interner-stable)"
    );

    println!(
        "oracle passed: {} atoms, {} facts, {} derived, {} findings, {} digests identical",
        fact_objects.len(),
        facts_a.len(),
        derived_a.len(),
        render_findings(model_a.findings()).len(),
        digests_a.len(),
    );
}

#[test]
fn fact_object_is_content_addressed_and_upcast_safe() {
    // A FactObject's ContentHash must depend only on its canonical bytes,
    // never on ClaimId (the three-way split — DL-0041).
    let atom = dli_eval::world::GroundAtom {
        pred: "place".to_owned(),
        args: vec!["camelot".to_owned()],
    };
    let support = PosBool::pos(dli_core::atom::Atom(0));

    let fo = FactObject::new(
        ClaimId::from_label("test/a"),
        atom.clone(),
        support.clone(),
        PosBool::FALSE,
        Vec::new(),
        dli_core::grade::Grade::Asserted,
    );
    let h = fo.content_hash().expect("canonical encoding").to_string();

    // Same content, different claim_id → same hash (claim_id is not in the
    // canonical content bytes).
    let mut fo_renamed = fo.clone();
    fo_renamed.claim_id = ClaimId::from_label("test/a-renamed");
    assert_eq!(
        fo_renamed
            .content_hash()
            .expect("canonical encoding")
            .to_string(),
        h,
        "content hash must be stable regardless of ClaimId label"
    );

    // Different atoms produce different hashes.
    let mut fo_diff = fo.clone();
    fo_diff.atom.args.push("guild".to_owned());
    assert_ne!(
        fo_diff
            .content_hash()
            .expect("canonical encoding")
            .to_string(),
        h,
        "different atoms must produce different hashes"
    );
}

#[test]
fn lower_decl_reproduces_lower_atoms() {
    // Verify that lower_decl (per-declaration, LS-4b) produces byte-identical
    // FactObjects to lower_atoms (full-compilation, LS-1) when given the same
    // interner state. This is the parity guarantee the write path depends on.
    let pack = corpus_pack(&load_corpus());
    let compiled = compile(&pack).expect("corpus compiles");
    let model = compiled.model();

    // lower_atoms over the full compiled model — reference set.
    let reference = lower_atoms(model.atoms());
    let ref_keys: Vec<(String, String)> = reference
        .iter()
        .map(|fo| {
            (
                fo.content_hash().expect("canonical encoding").to_string(),
                fo.claim_id.to_string(),
            )
        })
        .collect();

    // Re-lower each decl using lower_decl, starting with an empty interner
    // and bundle_counter=0 — the same state the loader starts from when
    // compiling the full pack. lower_decl must produce identical FactObjects
    // to lower_atoms on the atom layer; carriage objects (expert_body) carry
    // declarations lower_atoms cannot see and are excluded from the atom
    // comparison.
    let mut interner = dli_core::atom::Interner::new();
    let mut bundle_counter = 0u32;
    let pack_name_str = &pack.name;

    let mut decl_keys: Vec<(String, String)> = Vec::new();
    for doc in &pack.documents {
        for decl in &doc.decls {
            let objects = dli_lorestore::lower::lower_decl(
                decl,
                pack_name_str,
                &mut interner,
                &mut bundle_counter,
            );
            for fo in objects.iter().filter(|fo| fo.expert_body.is_none()) {
                decl_keys.push((
                    fo.content_hash().expect("canonical encoding").to_string(),
                    fo.claim_id.to_string(),
                ));
            }
        }
    }

    assert_eq!(
        ref_keys.len(),
        decl_keys.len(),
        "lower_decl must produce one atom FactObject per fact-producing decl,\n\
         reference has {} objects, decl-lowering produced {}",
        ref_keys.len(),
        decl_keys.len()
    );

    // Both sets are in declaration order (the loader pushes atoms in the same
    // order lower_decl processes them), so direct position comparison is
    // valid — not just set equality. Claim ids must agree too (audit A3): the
    // same fact reaches the same claim id through every write path.
    for (i, (ref_key, decl_key)) in ref_keys.iter().zip(decl_keys.iter()).enumerate() {
        assert_eq!(
            ref_key, decl_key,
            "FactObject at position {i} differs (hash, claim_id):\n  lower_atoms: {ref_key:?}\n  lower_decl:  {decl_key:?}"
        );
    }
}

/// Parity on the shapes the vendored corpus never exercises — each was a live
/// divergence between `lower_decl` and the loader before the audit repairs:
/// tag-gated proposals (the `h.` prefix), negated condition groups (De
/// Morgan), citation-requiring markers with no citations (fail-closed), a
/// malformed `assumes` (fail-closed, never fail-open), and entity slots
/// (inherited provenance, per-slot bundle vars).
#[test]
#[allow(clippy::too_many_lines)]
fn lower_decl_parity_on_gated_and_malformed_shapes() {
    use dli_ast::annot::{Annotation, CondExpr, ProvenanceMarker};
    use dli_ast::decl::{AttrDecl, Document, EntityDecl, EntityKind, FactDecl, ScopeHeader};
    use dli_ast::ident::Ident;
    use dli_ast::span::Span;
    use dli_ast::term::{AtomExpr, Term};
    use dli_world::pack::Pack;

    let id = |text: &str| Ident {
        text: text.to_owned(),
        span: Span::synthetic(),
    };
    let ann = |marker: ProvenanceMarker, tags: Vec<&str>, condition: Option<CondExpr>| Annotation {
        marker,
        citations: Vec::new(),
        condition,
        tags: tags.into_iter().map(str::to_owned).collect(),
        span: Span::synthetic(),
    };
    let fact = |pred: &str, subject: &str, object: &str, provenance: Option<Annotation>| {
        Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred: id(pred),
                args: vec![Term::Sym(id(subject)), Term::Sym(id(object))],
            },
            triple: true,
            provenance,
        })
    };

    let decls = vec![
        // A plain proposal: fresh bundle var `h.parity#0`.
        fact(
            "allied",
            "keep",
            "mill",
            Some(ann(ProvenanceMarker::Proposed, Vec::new(), None)),
        ),
        // A tag-gated proposal: must intern `h.origin-a`, not bare `origin-a`.
        fact(
            "allied",
            "keep",
            "forge",
            Some(ann(ProvenanceMarker::Proposed, vec!["origin-a"], None)),
        ),
        // A negated condition group: De Morgan must flip the connective —
        // `not (ha and hb)` is the disjunction `-ha | -hb`.
        fact(
            "allied",
            "mill",
            "forge",
            Some(ann(
                ProvenanceMarker::Proposed,
                Vec::new(),
                Some(CondExpr::Not(Box::new(CondExpr::And(vec![
                    CondExpr::Hyp(id("ha")),
                    CondExpr::Hyp(id("hb")),
                ])))),
            )),
        ),
        // A citation-requiring marker with zero citations: fail-closed.
        fact(
            "allied",
            "keep",
            "gate",
            Some(ann(ProvenanceMarker::Ratified, Vec::new(), None)),
        ),
        // A malformed `assumes` (empty hypothesis name): fail-closed, never
        // truth support.
        fact(
            "allied",
            "mill",
            "gate",
            Some(ann(
                ProvenanceMarker::Proposed,
                Vec::new(),
                Some(CondExpr::Hyp(id("  "))),
            )),
        ),
        // An entity with a slot: the slot inherits the entity's provenance and
        // mints its own bundle variable.
        Decl::Entity(EntityDecl {
            kind: EntityKind::Place,
            name: id("keep"),
            label: None,
            time: None,
            slots: vec![AttrDecl {
                subject: id("keep"),
                attr: id("standing"),
                value: Term::Sym(id("fortified")),
                provenance: None,
            }],
            provenance: Some(ann(ProvenanceMarker::Proposed, Vec::new(), None)),
        }),
    ];

    let pack = Pack {
        name: "parity".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("parity"),
                span: Span::synthetic(),
            }),
            decls: decls.clone(),
        }],
    };

    let compiled = compile(&pack).expect("parity pack compiles");
    let reference = lower_atoms(compiled.model().atoms());
    let ref_digests: Vec<String> = reference
        .iter()
        .map(|fo| fo.content_hash().expect("canonical encoding").to_string())
        .collect();

    let mut interner = dli_core::atom::Interner::new();
    let mut bundle_counter = 0u32;
    let mut decl_digests: Vec<String> = Vec::new();
    for decl in &decls {
        for fo in
            dli_lorestore::lower::lower_decl(decl, "parity", &mut interner, &mut bundle_counter)
                .iter()
                .filter(|fo| fo.expert_body.is_none())
        {
            decl_digests.push(fo.content_hash().expect("canonical encoding").to_string());
        }
    }

    assert_eq!(
        ref_digests, decl_digests,
        "lower_decl must be byte-identical to lower_atoms on gated and malformed shapes"
    );

    // The interners must agree too — a name-set divergence would poison every
    // later index even where the hashes happen to coincide.
    assert_eq!(
        compiled.model().interner().names().to_vec(),
        interner.names().to_vec(),
        "per-decl lowering must keep the interner in lock-step with the loader"
    );
}
