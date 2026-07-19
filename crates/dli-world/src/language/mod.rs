//! The language layer (`LANG-1`/`LANG-2`): registers, morphemes, names, terms,
//! and the derivation checker.
//!
//! Names of people, places, and things — and the world's language generally —
//! are world knowledge, and world knowledge lives in one fabric
//! (05-language-layer). A register is a claim about a linguistic zone of the
//! world; a ratified name is canon; the allegory's lexical bans are cited lexicon
//! facts. This layer is therefore not a sibling tool bolted to the fabric's side
//! — it is packs *in* the fabric, statused and forkable like any other lore.
//!
//! The pipeline is three seams:
//!
//! 1. [`compile`] lifts parsed documents into a [`LanguageModel`] — the kernel's
//!    name-space (the mirror's successor, [`view`]).
//! 2. [`check`] runs the derivation battery ([`derivation`]): the three
//!    register-fit obligations plus the world-true naming screens (uniqueness,
//!    aural distinctness, sayability, gender, banned-in-name).
//! 3. [`vet_candidate`] is the kernel half of the oracle's two-layer verdict — it
//!    vets a bare candidate against the settled name-space, leaving the
//!    world-safe screens to the consuming repo's factory.
//!
//! The **vet split** governs what re-homes here: a check enters the kernel iff
//! its ground truth is a fact about the *story's* world, and stays factory-side
//! iff it is a fact about *ours* (05 §3). Every [`LangFinding`] is a claim about
//! the story's world.

pub mod derivation;
pub mod name;
pub mod register;
pub mod view;

pub use derivation::{LangFinding, aural_clash, check_language, phonetic_key};
pub use name::{Composition, LexemeDef, NameDef};
pub use register::{
    BanDef, Gender, Hypothesis, LanguageModel, LanguageOutcome, MorphemeDef, NameKind,
    Phonotactics, RegisterDef, Stress, TermDef,
};
pub use view::{RatifiedName, certain_surfaces, ratified_names, vet_candidate, vet_in_set};

/// Compile a set of parsed documents into the language registry and its
/// compile-time findings.
///
/// A thin alias for [`LanguageModel::from_documents`] naming the layer's compile
/// seam.
#[must_use]
pub fn compile(docs: &[dli_ast::decl::Document]) -> LanguageOutcome {
    LanguageModel::from_documents(docs)
}

/// Run the naming derivation battery over a compiled registry, returning every
/// world-true naming finding as a unified [`WorldFinding`](crate::check::WorldFinding).
///
/// This is the language layer's contribution to the world-check aggregator
/// ([`check::run`](crate::check::run)): each world-true [`LangFinding`] is lifted
/// into the aggregator's finding type, so `dli.name.check` and the AUTH tools
/// consume one shape. Callers that need the rich, typed findings use
/// [`check_language`].
#[must_use]
pub fn check(model: &LanguageModel) -> Vec<crate::check::WorldFinding> {
    check_language(model)
        .iter()
        .map(LangFinding::to_world_finding)
        .collect()
}
