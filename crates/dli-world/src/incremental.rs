//! Shared compilation tail and optional fixpoint reuse.

use dli_eval::error::EvalError;
use dli_eval::world::{ReuseReport, World};

use crate::check::CompiledWorld;
use crate::loader::Loader;
use crate::model::WorldModel;
use crate::pack::Pack;

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

/// Compile a pack while reusing only a prior **fixpoint** — the smaller win, for
/// a caller that kept a [`CompiledWorld`].
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
    model.inferred_era_edges = crate::era_order::infer(&model);
    model.era_inference_revision = crate::era_order::INFERENCE_REVISION;
    Ok((model, report))
}
