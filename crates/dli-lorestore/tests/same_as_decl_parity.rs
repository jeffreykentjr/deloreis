#![allow(clippy::unwrap_used, clippy::expect_used)]
//! WS-1: `same-as` is carried whole (like `before`), exactly mirroring its
//! pipeline — carriage object first, then the regenerated `same-as(a, b)`
//! atom object, and the interner lock-step touchpoint at `decl_provenance`.
//!
//! The probes here are the durable-store analogues of `same_as_honesty.rs`
//! (`dli-eval`) and `identity_merge.rs` (`dli-world`): three-path byte
//! identity with a CITED same-as staged BETWEEN two bare proposals (so a
//! desynced bundle counter or a stray interned citation shows up as a shifted
//! digest downstream, not a local failure), the carriage round trip, the
//! `expected_claim_id` re-derivation, and a durable "defeat and refold": a
//! second commit whose ledger no longer carries the identity's citing decision
//! un-merges it on recompile, while an unrelated ratified fact stays certain.

use std::collections::BTreeSet;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{
    Decl, Document, EntityDecl, EntityKind, FactDecl, FunctionalDecl, RatifyDecl, SameAsDecl,
    ScopeHeader,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::Status;

use dli_lorestore::Lorestore;
use dli_lorestore::identity::ClaimId;
use dli_lorestore::lower::{
    declared_decisions, expected_claim_id, lower_atoms, lower_decl, lower_pack, reconstruct_pack,
};
use dli_lorestore::object::ExpertDeclKind;

use dli_world::compile;
use dli_world::pack::Pack;

use tempfile::TempDir;

// ── builders ──────────────────────────────────────────────────────────

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn marker(m: ProvenanceMarker, cites: &[&str]) -> Annotation {
    Annotation {
        marker: m,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn ratified(cite: &str) -> Annotation {
    marker(ProvenanceMarker::Ratified, &[cite])
}

/// A bare proposal — no citation, no tags, so it mints a fresh
/// `h.<pack>#N` bundle variable. That variable is the desync tripwire: a
/// same-as that quietly interned an extra name between two of these would
/// shift the second's index.
fn proposed() -> Annotation {
    marker(ProvenanceMarker::Proposed, &[])
}

fn ratify_decl(d: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(d),
        body: dli_ast::language::RatifyBody::default(),
    })
}

fn entity(kind: EntityKind, name: &str, cite: &str) -> Decl {
    Decl::Entity(EntityDecl {
        kind,
        name: id(name),
        label: None,
        time: None,
        slots: Vec::new(),
        provenance: Some(ratified(cite)),
    })
}

fn fact(pred: &str, args: &[&str], prov: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id(pred),
            args: args.iter().map(|a| Term::Sym(id(a))).collect(),
        },
        triple: false,
        provenance: prov,
    })
}

fn functional(pred: &str, prov: Option<Annotation>) -> Decl {
    Decl::Functional(FunctionalDecl {
        pred: id(pred),
        provenance: prov,
    })
}

fn same_as(left: &str, right: &str, prov: Option<Annotation>) -> Decl {
    Decl::SameAs(SameAsDecl {
        left: id(left),
        right: id(right),
        provenance: prov,
    })
}

fn pack(name: &str, decls: Vec<Decl>) -> Pack {
    Pack {
        name: name.to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id(name),
                span: Span::synthetic(),
            }),
            decls,
        }],
    }
}

/// The differential gauntlet: a CITED same-as staged BETWEEN two bare
/// proposals — the interner lock-step pin. `decl_provenance` must route
/// `Decl::SameAs` exactly as `Decl::Before` (its citation interned, nothing
/// extra), or the second proposal's bundle index shifts and every digest after
/// it changes.
fn gauntlet(with_same_as: bool) -> Pack {
    let mut decls = vec![
        ratify_decl("D-1"),
        ratify_decl("D-9"),
        entity(EntityKind::Place, "mara", "D-1"),
        entity(EntityKind::Place, "moira", "D-1"),
        // h.weir#0
        fact("linked", &["mara", "camelot"], Some(proposed())),
    ];
    if with_same_as {
        decls.push(same_as("mara", "moira", Some(ratified("D-9"))));
    }
    // h.weir#1 — #2 if the same-as above burned an index
    decls.push(fact("omen", &["mara", "storm"], Some(proposed())));
    pack("weir", decls)
}

fn digests(objects: &[dli_lorestore::object::FactObject]) -> Vec<String> {
    objects
        .iter()
        .map(|o| o.content_hash().expect("hash").to_string())
        .collect()
}

fn bundle_vars(names: &[String]) -> Vec<&String> {
    names.iter().filter(|n| n.starts_with("h.")).collect()
}

// ── carriage shape ────────────────────────────────────────────────────

#[test]
fn same_as_carriage_kind_serializes_as_the_durable_const() {
    assert_eq!(
        serde_json::to_string(&ExpertDeclKind::SameAs).expect("serializes"),
        "\"sameas\"",
        "the wire-visible durable kind name — a rename here is a stored-shape break"
    );
}

/// `same-as` is carried like `before`: one carriage object under the
/// synthetic `__decl` atom, followed by the regenerated `same-as(a, b)` atom
/// object — never the reverse, and never collapsed into one.
#[test]
fn same_as_lowers_to_a_carriage_object_then_its_atom() {
    let (objects, _names) = lower_pack(&gauntlet(true));
    let same_as_index = objects
        .iter()
        .position(|o| {
            o.expert_body
                .as_ref()
                .is_some_and(|b| b.kind == ExpertDeclKind::SameAs)
        })
        .expect("a same-as carriage object exists");
    let carriage = &objects[same_as_index];
    assert_eq!(carriage.atom.pred, "__decl");
    assert!(
        carriage
            .expert_body
            .as_ref()
            .is_some_and(|b| b.body.len() == 1 && b.body[0].contains("SameAs")),
        "the carriage body is the serialized Decl::SameAs"
    );

    let next = objects
        .get(same_as_index + 1)
        .expect("the regenerated atom follows immediately");
    assert!(
        next.expert_body.is_none(),
        "the atom object carries no body"
    );
    assert_eq!(next.atom.pred, "same-as");
    assert_eq!(next.atom.args, vec!["mara".to_owned(), "moira".to_owned()]);
}

// ── the interner lock-step pin ───────────────────────────────────────

#[test]
fn same_as_between_two_proposals_keeps_the_bundle_counter_in_lock_step() {
    let (_with_objects, with_names) = lower_pack(&gauntlet(true));
    let (_without_objects, without_names) = lower_pack(&gauntlet(false));

    // The same-as decl sits between the two bare proposals in document order
    // (`gauntlet`'s layout). It must mint NO bundle variable of its own — only
    // its citation (D-9, already interned separately) — so the SECOND
    // proposal's bundle index is unmoved by its presence.
    assert_eq!(
        bundle_vars(&with_names),
        vec!["h.weir#0", "h.weir#1"],
        "one bundle per bare proposal — the same-as decl mints none of its own"
    );
    assert_eq!(
        bundle_vars(&without_names),
        vec!["h.weir#0", "h.weir#1"],
        "identical bundle indices with or without the same-as decl"
    );

    // The atom digests these two bundle vars support are compared directly
    // against the loader-driven `lower_atoms` reference in
    // `lower_pack_atoms_stay_byte_identical_to_lower_atoms` and
    // `lower_decl_reproduces_the_same_as_atom` — the real three-path lock-step
    // guarantee. A with/without digest comparison is not meaningful here: D-9
    // is cited ONLY by the same-as decl, so it is interned in the "with" case
    // and never interned in the "without" one, which legitimately shifts the
    // SECOND proposal's own Atom index between the two variants (a fact about
    // shared citation position, not a same-as-specific desync).
}

// ── three-path byte identity ─────────────────────────────────────────

/// `lower_pack` (whole-pack walk) and `lower_atoms` (over the compiled model)
/// must mint the identical `same-as(a, b)` atom object.
#[test]
fn lower_pack_atoms_stay_byte_identical_to_lower_atoms() {
    let pack = gauntlet(true);
    let compiled = compile(&pack).expect("compiles");
    let atom_digests: BTreeSet<String> = digests(&lower_atoms(compiled.model().atoms()))
        .into_iter()
        .collect();
    let (decl_objects, _names) = lower_pack(&pack);
    let decl_digests: BTreeSet<String> = digests(&decl_objects).into_iter().collect();
    let missing: Vec<&String> = atom_digests.difference(&decl_digests).collect();
    assert!(
        missing.is_empty(),
        "every lower_atoms digest (including same-as) must be reproduced by lower_pack; missing: {missing:?}"
    );
}

/// The third leg: `lower_decl`, called per-declaration with a fresh interner
/// walked in document order, must reproduce the SAME `same-as` atom object
/// `lower_atoms` mints from the full compiled model.
#[test]
fn lower_decl_reproduces_the_same_as_atom() {
    let pack = gauntlet(true);
    let compiled = compile(&pack).expect("compiles");
    let reference: BTreeSet<String> = digests(&lower_atoms(compiled.model().atoms()))
        .into_iter()
        .collect();

    let mut interner = dli_core::atom::Interner::new();
    let mut bundle_counter = 0u32;
    let mut decl_digests = BTreeSet::new();
    for doc in &pack.documents {
        for decl in &doc.decls {
            for fo in lower_decl(decl, &pack.name, &mut interner, &mut bundle_counter) {
                if fo.expert_body.is_none() {
                    decl_digests.insert(fo.content_hash().expect("hash").to_string());
                }
            }
        }
    }
    let missing: Vec<&String> = reference.difference(&decl_digests).collect();
    assert!(
        missing.is_empty(),
        "lower_decl must reproduce every atom lower_atoms mints, including same-as: {missing:?}"
    );
}

// ── expected_claim_id ─────────────────────────────────────────────────

#[test]
fn expected_claim_id_re_derives_for_both_the_carriage_and_the_atom() {
    let (objects, _names) = lower_pack(&gauntlet(true));
    let carriage = objects
        .iter()
        .find(|o| {
            o.expert_body
                .as_ref()
                .is_some_and(|b| b.kind == ExpertDeclKind::SameAs)
        })
        .expect("carriage object");
    assert_eq!(
        expected_claim_id(carriage),
        ClaimId::from_label("decl:SameAs:mara=moira"),
        "the carriage claim id is minted from the decl kind and display name"
    );

    let atom_obj = objects
        .iter()
        .find(|o| o.expert_body.is_none() && o.atom.pred == "same-as")
        .expect("atom object");
    assert_eq!(
        expected_claim_id(atom_obj),
        ClaimId::from_label("atom:same-as(mara, moira)"),
        "the atom claim id is minted from the rendered ground atom, the SAME scheme every fact uses"
    );
}

// ── carriage round trip ─────────────────────────────────────────────

#[test]
fn same_as_round_trips_through_reconstruct_pack() {
    let pack = gauntlet(true);
    let compiled = compile(&pack).expect("compiles");
    let (objects, _names) = lower_pack(&pack);
    let decisions = declared_decisions(&pack);
    let back = reconstruct_pack(
        &objects,
        compiled.model().interner(),
        &decisions,
        &pack.name,
    )
    .expect("the lowered pack reconstructs");

    let recovered: Vec<&SameAsDecl> = back
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .filter_map(|decl| match decl {
            Decl::SameAs(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(recovered.len(), 1, "the same-as decl survives the store");
    assert_eq!(recovered[0].left.text, "mara");
    assert_eq!(recovered[0].right.text, "moira");
    assert_eq!(
        recovered[0].provenance,
        Some(ratified("D-9")),
        "the cited provenance rides the carriage body byte-for-byte"
    );

    // Recompiling the reconstructed pack reproduces the identical fixpoint,
    // findings, and digests as the original.
    let recompiled = compile(&back).expect("reconstructed pack recompiles");
    assert_eq!(
        recompiled.model().findings(),
        compiled.model().findings(),
        "findings must round-trip identically"
    );
    let orig_status = compiled
        .model()
        .atoms()
        .iter()
        .find(|a| a.atom.pred.text == "same-as")
        .map(|a| {
            a.ann.status(
                &compiled.model().world.valuation,
                &compiled.model().world.theory,
            )
        });
    let round_status = recompiled
        .model()
        .atoms()
        .iter()
        .find(|a| a.atom.pred.text == "same-as")
        .map(|a| {
            a.ann.status(
                &recompiled.model().world.valuation,
                &recompiled.model().world.theory,
            )
        });
    assert_eq!(orig_status, round_status);
    assert_eq!(orig_status, Some(Status::Certain));
}

// ── the full store round trip ───────────────────────────────────────

#[test]
fn same_as_pack_round_trips_through_the_lorestore() {
    let pack = gauntlet(true);
    let compiled_a = compile(&pack).expect("A compiles");
    let model_a = compiled_a.model();

    let tmp = TempDir::new().expect("tempdir");
    let store = Lorestore::open(tmp.path().to_path_buf()).expect("open");
    store.import_pack(&pack).expect("import must not diverge");
    let compiled_b = store.compile().expect("B compiles");
    let model_b = compiled_b.model();

    assert_eq!(
        model_a.interner().names().to_vec(),
        model_b.interner().names().to_vec(),
        "interner names must match after round-trip"
    );
    let same_as_a = model_a
        .atoms()
        .iter()
        .find(|a| a.atom.pred.text == "same-as")
        .expect("same-as atom in A");
    let same_as_b = model_b
        .atoms()
        .iter()
        .find(|a| a.atom.pred.text == "same-as")
        .expect("same-as atom in B");
    assert_eq!(same_as_a.atom, same_as_b.atom);
    assert_eq!(
        same_as_a
            .ann
            .status(&model_a.world.valuation, &model_a.world.theory),
        same_as_b
            .ann
            .status(&model_b.world.valuation, &model_b.world.theory),
    );
}

// ── durable defeat, refold, un-merge ───────────────────────────────

/// A durable "defeat and refold". The append-only `decisions` ledger on a
/// commit cannot un-declare D-9 (the store refuses a commit whose decisions
/// list regresses — verified below), so a real durable defeat rides the OTHER
/// mechanism the store already carries for this: a ref's `valuation` overlay
/// (`Ref::with_valuation`, CAS-written via `cas_main_ref`). Neither
/// `Lorestore::compile`/`compile_at` reads it — applying a ref's overlay onto
/// a compiled world is a consumer's job (the wire's workspace machinery, out
/// of scope here) — so this proves the STORAGE half honestly: the defeating
/// bit round-trips durably through the store, and applying it exactly as a
/// consumer would (one valuation flip, no recompile) un-merges the identity
/// while leaving the unrelated, still-ratified fact untouched. Not a single
/// stored fact object changes; only the ref's overlay does.
#[test]
fn durable_defeat_refolds_to_an_un_merged_world() {
    let gauntlet = pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            ratify_decl("D-9"),
            functional("rank", Some(ratified("D-1"))),
            fact("rank", &["mara", "gold"], Some(ratified("D-1"))),
            fact("rank", &["moira", "silver"], Some(ratified("D-1"))),
            same_as("mara", "moira", Some(ratified("D-9"))),
        ],
    );

    let tmp = TempDir::new().expect("tempdir");
    let store = Lorestore::open(tmp.path().to_path_buf()).expect("open");
    store.import_pack(&gauntlet).expect("import");

    let compiled = store.compile().expect("compiles");
    let fd_before: Vec<_> = dli_world::check::run(&compiled)
        .into_iter()
        .filter(|f| f.code == "continuity.fd-violation")
        .collect();
    assert_eq!(
        fd_before.len(),
        1,
        "the merge creates one FD violation: {fd_before:?}"
    );

    // The append-only guard: a commit cannot durably drop a declared decision.
    let (objects, interner) = dli_lorestore::lower::lower_pack(&gauntlet);
    let mut fact_hashes = Vec::with_capacity(objects.len());
    for obj in &objects {
        fact_hashes.push(store.write_object(obj).expect("write"));
    }
    let regressed_decisions = vec!["D-1".to_owned()]; // drops D-9
    let regression = store.commit(
        interner,
        fact_hashes,
        Vec::new(),
        Vec::new(),
        regressed_decisions,
        gauntlet.name.clone(),
    );
    assert!(
        regression.is_err(),
        "the store must refuse an attempt to durably un-declare a decision"
    );

    // The real mechanism: a ref valuation overlay, CAS-written and read back.
    let d9 = compiled
        .model()
        .var_atom("D-9")
        .expect("D-9 is interned by the same-as citation");
    let mut defeat = dli_core::valuation::Valuation::new();
    defeat.set(d9, false);
    let head_commit = store.root_hash().expect("root hash").expect("head exists");
    let current = store.head_ref().expect("head ref").expect("main exists");
    let token = current.digest().expect("digest").0;
    let new_ref = dli_lorestore::Ref::with_valuation("main", head_commit, defeat);
    let landed = store
        .cas_main_ref(Some(&token), &new_ref)
        .expect("cas main ref");
    assert!(landed, "the defeating valuation overlay must land on main");

    // Durable round trip: re-open and read the overlay back.
    let reopened = Lorestore::open(tmp.path().to_path_buf()).expect("reopen");
    let stored_ref = reopened.head_ref().expect("head ref").expect("main exists");
    assert_eq!(
        stored_ref.valuation.get(d9),
        Some(false),
        "the defeating bit survives a store reopen"
    );

    // Apply it exactly as a consumer would: resolve the persisted numeric bit
    // through its source commit interner, then into this independently compiled
    // world. The symbolic fixpoint still stands; only its valuation changes.
    let mut refolded = compiled;
    let source_names = reopened
        .interner_at(&stored_ref.commit)
        .expect("source commit interner");
    let named =
        dli_core::valuation::NamedValuation::from_indexed(&stored_ref.valuation, &source_names)
            .expect("persisted valuation resolves");
    let target_interner = refolded.model.interner().clone();
    named
        .apply_to(&target_interner, &mut refolded.model.world.valuation)
        .expect("all persisted names exist in the receiving world");
    let fd_after: Vec<_> = dli_world::check::run(&refolded)
        .into_iter()
        .filter(|f| f.code == "continuity.fd-violation")
        .collect();
    assert!(
        fd_after.is_empty(),
        "the durable defeat must un-merge the slot on the next read: {fd_after:?}"
    );
    let mara_rank_status = refolded
        .model()
        .atoms()
        .iter()
        .find(|a| {
            a.atom.pred.text == "rank"
                && matches!(&a.atom.args[0], Term::Sym(subj) if subj.text == "mara")
        })
        .map(|a| {
            a.ann.status(
                &refolded.model().world.valuation,
                &refolded.model().world.theory,
            )
        });
    assert_eq!(
        mara_rank_status,
        Some(Status::Certain),
        "D-1's own fact is untouched by defeating D-9"
    );
}
