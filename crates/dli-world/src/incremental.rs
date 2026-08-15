//! Incremental compilation: extending a compiled world without re-lowering it
//! (DL-0087, staged by DL-0088).
//!
//! [`compile`](crate::compile) is a whole-pack function. It re-lowers every
//! declaration, re-synthesizes the identity closure, re-stratifies, and
//! re-saturates from nothing, every time — so the cost of adding one fact to a
//! corpus is the cost of the corpus. This module is the seam that makes the
//! second compile cost the *delta* instead, **without** buying speed with a
//! different answer: the world [`compile_incremental`] produces is byte-identical
//! to the world [`compile`](crate::compile) produces for the union pack, and
//! `crates/dli-world/tests/incremental_compile_parity.rs` is the standing proof.
//!
//! # What is reused, and what is recomputed
//!
//! Reuse is at the granularity each step can *soundly* support, never finer:
//!
//! - **Per-declaration lowering is reused.** [`IncrementalLoad`] holds the
//!   loader's state before its whole-pack aggregation, so extending it lowers
//!   only the added documents. Everything the base documents produced — atoms,
//!   sorts, ground facts, rules, chronology edges and their rulings, fluent
//!   spans, the shared event-variable interner and time-point table (both
//!   append-only, never renumbered), pack provenance, the duplicate- and
//!   cross-pack bookkeeping, the temporal references still to resolve, the
//!   anonymous-proposal bundle counter, and the language layer's compiler state
//!   — is carried forward verbatim. This is the reuse that scales with corpus
//!   size.
//! - **The fixpoint is reused**, through PR 1's
//!   [`World::saturate_incremental`](dli_eval::world::World::saturate_incremental):
//!   only the strata the delta can disturb are re-saturated. That primitive
//!   refuses its own preconditions (the union program must *extend* the prior's
//!   facts, rules, and interner, and share its theory) and falls back to a full
//!   fixpoint on its own, so this call is always safe.
//! - **The whole-pack aggregation is recomputed in full**, from the
//!   un-aggregated state, every time. Temporal-reference resolution, cross-pack
//!   collision, finding dedup, entity materialization, identity-closure
//!   synthesis, and stratification are all genuinely global: a delta document can
//!   resolve a previously-dangling `after`, turn a single-pack name into a
//!   collision, or make a rule set unstratifiable that was fine before. These
//!   are linear (or near-linear) passes over the corpus, not the quadratic
//!   saturation, and recomputing them is what makes byte-identity a construction
//!   rather than a hope.
//! - **The ledger valuation is recomputed in full.** It is not append-only: a
//!   delta `ratify D-…` retro-rules a *base* atom's citation true, turning what
//!   was a `citation.unknown-decision` finding into a valuation entry. Its cost
//!   is one pass over the atoms, sorts, and declarations.
//! - **The check battery is not sliced.** [`IncrementalWorld::check`] re-runs
//!   [`check::run`](crate::check::run) over the whole resulting world; sliced
//!   checks are deferred by DL-0088.
//!
//! # The `same-as` case
//!
//! `synthesize_alias_rules` is always re-run over the union from the
//! un-synthesized state, because its per-predicate rewrite census spans *every*
//! predicate in the world: one added identity link re-synthesizes the whole
//! family. That has a consequence for the fixpoint half. The synthesized
//! `same-as:link` facts and alias rules are appended at the *end* of the
//! program, so when the **base** declares any identity, the prior world's facts
//! and rules are no longer a prefix of the union's (the base's synthesized rows
//! sit before the delta's declared ones in the prior and after them in the
//! union). `saturate_incremental` sees that, refuses reuse, and runs the full
//! fixpoint — the right answer, at full fixpoint cost. Loader reuse is
//! unaffected. When only the **delta** declares identity the prefix holds and
//! the fixpoint reuse path runs, though the alias rewrite rules typically leave
//! it few strata to skip. This is the expected shape (DL-0088); a finer split
//! needs the synthesized artifacts to be separable from the declared ones, which
//! is not a change this PR makes.
//!
//! # Why the reuse base is a type and not a `&CompiledWorld`
//!
//! [`IncrementalWorld`] is an invariant type in exactly the sense
//! [`CompiledWorld`] is: only [`compile_resumable`] and [`compile_incremental`]
//! construct one, so its loader state is always the state that produced its
//! compiled world. A bare `&CompiledWorld` cannot serve, because the loader's
//! aggregation is lossy — a `CompiledWorld` does not record which `(pack, kind,
//! name)` triples were declared, which temporal references are outstanding, or
//! where the anonymous-bundle counter stands, and reconstructing them by
//! guesswork would trade a wrong answer for a saved pass. Callers that hold only
//! a `CompiledWorld` have [`compile_reusing_fixpoint`], which re-lowers
//! everything and reuses the saturation alone.

use dli_ast::decl::Document;
use dli_eval::error::EvalError;
use dli_eval::world::{ReuseReport, World};

use crate::check::{CompiledWorld, WorldFinding};
use crate::loader::{IncrementalLoad, Loader};
use crate::model::WorldModel;
use crate::pack::Pack;

/// A compiled world together with everything needed to extend it without
/// re-lowering — the reuse base [`compile_incremental`] takes.
///
/// It carries three things: the [`CompiledWorld`] itself, the pack it was
/// compiled from (the ledger valuation reads the declarations, so the documents
/// have to survive the compile), and the loader state from *before* the
/// whole-pack aggregation. Holding the pre-aggregation state is what lets the
/// aggregation re-run from the same starting point a full load reaches, which is
/// what byte-identity rests on.
///
/// The two copies of the model — the aggregated, saturated one and the
/// un-aggregated loader state — cost roughly twice a `CompiledWorld` in memory.
/// That is the price of not re-lowering; a caller that would rather pay the time
/// than the space keeps a plain [`CompiledWorld`] and calls
/// [`compile_reusing_fixpoint`].
#[derive(Debug, Clone)]
pub struct IncrementalWorld {
    world: CompiledWorld,
    pack: Pack,
    load: IncrementalLoad,
}

impl IncrementalWorld {
    /// The compiled world. Byte-identical to
    /// [`compile`](crate::compile) over [`pack`](IncrementalWorld::pack).
    #[must_use]
    pub fn world(&self) -> &CompiledWorld {
        &self.world
    }

    /// The pack compiled so far — the base documents plus every document added
    /// by [`compile_incremental`], in the order they were added.
    #[must_use]
    pub fn pack(&self) -> &Pack {
        &self.pack
    }

    /// The compiled world, taken by value.
    #[must_use]
    pub fn into_world(self) -> CompiledWorld {
        self.world
    }

    /// Run the world-internal check battery over the compiled world.
    ///
    /// **Full, never sliced.** DL-0088 defers incremental checking: a delta can
    /// change the verdict on a claim it never mentions (a new `functional`
    /// declaration turns an old pair of values into an FD violation; a new
    /// `same-as` merges two subjects whose slots then clash), so nothing here
    /// tries to reason about which findings survived. This is a convenience over
    /// [`check::run`](crate::check::run), named so the discipline is stated at
    /// the call site rather than assumed.
    #[must_use]
    pub fn check(&self) -> Vec<WorldFinding> {
        crate::check::run(&self.world)
    }

    /// Falsify one artifact the loader state carries **forward** — the positive
    /// control the parity oracle needs to prove reuse actually happened.
    ///
    /// Returns a copy whose reused loader state names a pack that the documents
    /// never declared. A compile that genuinely reuses that state must carry the
    /// falsification into its output; one that quietly re-lowered everything
    /// cannot. `None` when there is nothing lowered to falsify.
    ///
    /// Deliberately produces a wrong world, and exists only so an oracle cannot
    /// go vacuous. It is not `cfg(test)` because that oracle is an integration
    /// test compiled against this crate from outside.
    #[doc(hidden)]
    #[must_use]
    pub fn falsify_carried_load_state(&self) -> Option<Self> {
        let mut copy = self.clone();
        copy.load.falsify_carried_state().then_some(copy)
    }

    /// Falsify one saturated row the fixpoint half **reuses** — the positive
    /// control for [`World::saturate_incremental`]'s share of the work.
    ///
    /// Picks a row whose predicate the delta does not affect (so the incremental
    /// path copies it forward verbatim rather than re-deriving it) and moves its
    /// grade, a falsification that survives the reuse preconditions: they check
    /// fact presence and support entailment, neither of which grade touches. A
    /// corruption the preconditions rejected would prove nothing, because the
    /// call would fall back and match for the wrong reason. `None` when the delta
    /// affected every predicate and there was nothing to reuse.
    ///
    /// Test-only in intent, for the same reason
    /// [`falsify_carried_load_state`](IncrementalWorld::falsify_carried_load_state)
    /// is.
    #[doc(hidden)]
    #[must_use]
    pub fn falsify_reused_row(&self, report: &CompileReuse) -> Option<Self> {
        let mut copy = self.clone();
        let victim = copy
            .world
            .model
            .world
            .derived
            .iter_mut()
            .find(|row| !report.fixpoint.affected.contains(&row.atom.pred))?;
        victim.grade = if victim.grade == dli_core::grade::Grade::Checked {
            dli_core::grade::Grade::Asserted
        } else {
            dli_core::grade::Grade::Checked
        };
        Some(copy)
    }
}

/// What one incremental compile actually reused.
///
/// Every parity assertion an oracle can make about this module is satisfied by
/// an implementation that reuses nothing — it would then be comparing a full
/// compile against itself, green and worthless, with nothing in the output to
/// say so. This is the channel that makes reuse observable, so a test can put a
/// floor under it. It is a report, not a verdict: no caller's behaviour depends
/// on it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompileReuse {
    /// Whether the loader state was reused at all (false only for a compile with
    /// no prior).
    pub loader_reused: bool,
    /// Declarations carried forward without being lowered again.
    pub reused_decls: usize,
    /// Declarations lowered by this call — the delta.
    pub lowered_decls: usize,
    /// What the fixpoint half did, from
    /// [`World::saturate_incremental_reported`](dli_eval::world::World::saturate_incremental_reported).
    pub fixpoint: ReuseReport,
}

impl CompileReuse {
    /// Whether the loader half genuinely reused work: it took the reuse path
    /// *and* had declarations to carry forward. A compile that reused an empty
    /// base did the full job with extra bookkeeping, and counting it as coverage
    /// would be fooling ourselves.
    #[must_use]
    pub fn loader_is_live(&self) -> bool {
        self.loader_reused && self.reused_decls > 0
    }

    /// Whether the fixpoint half genuinely reused work — it took the reuse path
    /// and skipped at least one stratum.
    #[must_use]
    pub fn fixpoint_is_live(&self) -> bool {
        self.fixpoint.is_live()
    }
}

/// Compile a pack into a resumable [`IncrementalWorld`].
///
/// [`world`](IncrementalWorld::world) is byte-identical to
/// [`compile`](crate::compile) over the same pack — the two share the whole
/// pipeline; this one keeps the loader state instead of dropping it.
///
/// # Errors
///
/// Returns [`EvalError::Unstratifiable`] on a rule set that admits no valid
/// stratification, exactly as [`compile`](crate::compile) does.
pub fn compile_resumable(pack: &Pack) -> Result<IncrementalWorld, EvalError> {
    let mut load = IncrementalLoad::new();
    load.extend(&pack.documents);
    let (model, _) = seal(load.finish().world, pack, None)?;
    Ok(IncrementalWorld {
        world: CompiledWorld::new(model),
        pack: pack.clone(),
        load,
    })
}

/// Extend a prior compile with more documents, reusing its lowering and its
/// fixpoint.
///
/// The result is byte-identical to [`compile`](crate::compile) over the union
/// pack — the prior's documents followed by `added` — and the returned
/// [`IncrementalWorld`] is itself a valid base for a further extension, so a
/// session's compiles chain. The [`CompileReuse`] says what was actually reused.
///
/// The prior's killed-token store rides along, so a guardrail registry attached
/// with [`with_killed_tokens`](CompiledWorld::with_killed_tokens) survives an
/// extension. (Byte-identity is against a `compile` of the union, which mints an
/// empty store, so a prior with an empty store — everything
/// [`compile_resumable`] produces — stays byte-identical.)
///
/// # Errors
///
/// Returns [`EvalError::Unstratifiable`] when the union's rules admit no valid
/// stratification — including when the base alone was fine and a delta rule
/// closed a negative cycle.
pub fn compile_incremental(
    prior: &IncrementalWorld,
    added: &[Document],
) -> Result<(IncrementalWorld, CompileReuse), EvalError> {
    let reused_decls = prior.load.lowered_decls();
    let mut load = prior.load.clone();
    load.extend(added);
    let lowered_decls = load.lowered_decls().saturating_sub(reused_decls);

    let mut pack = prior.pack.clone();
    pack.documents.extend_from_slice(added);

    let (model, fixpoint) = seal(load.finish().world, &pack, Some(&prior.world.model.world))?;
    let report = CompileReuse {
        loader_reused: true,
        reused_decls,
        lowered_decls,
        fixpoint,
    };
    let world = CompiledWorld::new(model).with_killed_tokens(prior.world.killed.clone());
    Ok((IncrementalWorld { world, pack, load }, report))
}

/// [`compile_incremental`], cross-checked against a full
/// [`compile`](crate::compile) of the union pack — the **verify-against-full
/// mode**, the compile-level twin of
/// [`World::saturate_incremental_verified`](dli_eval::world::World::saturate_incremental_verified).
///
/// Computes both worlds and **panics** when they disagree. It is strictly slower
/// than a full compile (it *is* a full compile, plus the incremental one), so it
/// buys no speed. What it buys is that a divergence surfaces at the seam that
/// caused it rather than as a wrong verdict downstream — the failure mode that
/// makes an incremental compiler worse than none at all. A divergence is a
/// broken kernel invariant, not a condition a caller can handle: there is no
/// recovering from a compile that does not reproduce its own full recompute,
/// because every verdict read from it stops being replayable, and routing it
/// through [`EvalError`] would tell the author their lore is broken when the
/// truth is that the compiler is.
///
/// # Errors
///
/// As [`compile_incremental`].
///
/// # Panics
///
/// If the incremental result differs from a full compile of the union pack.
pub fn compile_incremental_verified(
    prior: &IncrementalWorld,
    added: &[Document],
) -> Result<(IncrementalWorld, CompileReuse), EvalError> {
    let (incremental, report) = compile_incremental(prior, added)?;
    let full = crate::compile(&incremental.pack)?;
    assert!(
        incremental.world == full,
        "incremental compilation diverged from a full compile of the same pack \
         (this is a compiler bug, not a corpus error)"
    );
    Ok((incremental, report))
}

/// Compile a pack while reusing only a prior **fixpoint** — the smaller win, for
/// a caller that kept a [`CompiledWorld`] and not an [`IncrementalWorld`].
///
/// Every declaration is lowered again; what is saved is the saturation, which is
/// the quadratic half. `pack` is the *whole* pack to compile, not a delta, and
/// `prior` is a world compiled from some prefix of it. Nothing here trusts that
/// relationship: `saturate_incremental` checks that the union program extends
/// the prior's facts, rules, and interner and shares its theory, and falls back
/// to a full fixpoint when it does not — so an unrelated `prior` costs time and
/// never correctness. The result is byte-identical to
/// [`compile`](crate::compile) over `pack` either way.
///
/// # Errors
///
/// As [`compile`](crate::compile).
pub fn compile_reusing_fixpoint(
    prior: &CompiledWorld,
    pack: &Pack,
) -> Result<(CompiledWorld, CompileReuse), EvalError> {
    let model = Loader::load(&pack.documents).world;
    let (model, fixpoint) = seal(model, pack, Some(&prior.model.world))?;
    let report = CompileReuse {
        loader_reused: false,
        reused_decls: 0,
        lowered_decls: pack.documents.iter().map(|doc| doc.decls.len()).sum(),
        fixpoint,
    };
    Ok((
        CompiledWorld::new(model).with_killed_tokens(prior.killed.clone()),
        report,
    ))
}

/// The tail every compile shares: rule the ledger valuation, then saturate.
///
/// Both [`compile`](crate::compile) and the incremental entry points route
/// through here, so the ordering the ledger findings land in — every atom's,
/// then every sort's, then the language-layer declarations' — and the
/// stratify-then-saturate handoff cannot drift between the full and incremental
/// paths. That is not a tidiness argument: those two orderings are exactly what
/// byte-identity between the paths reduces to once the loader half is
/// order-identical by construction.
///
/// `prior` is the previously-saturated world to reuse, or `None` for a full
/// fixpoint. Passing `Some` is always safe — the primitive screens its own
/// preconditions and falls back.
pub(crate) fn seal(
    mut model: WorldModel,
    pack: &Pack,
    prior: Option<&World>,
) -> Result<(WorldModel, ReuseReport), EvalError> {
    let known_decisions = crate::declared_decisions(pack);
    let (valuation, findings) = crate::ledger_valuation(&model, pack, &known_decisions);
    model.world.valuation = valuation;
    model.world.findings.extend(findings);
    // Stratification is computed once and handed to the saturation, which is
    // what `World::fixpoint` does internally for the full path.
    let strata = model.world.stratify()?;
    let (saturated, report) = match prior {
        Some(prior) => model.world.saturate_incremental_reported(prior, &strata)?,
        None => (model.world.saturate_with(&strata)?, ReuseReport::fallback()),
    };
    model.world = saturated;
    Ok((model, report))
}
