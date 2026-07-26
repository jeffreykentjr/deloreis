//! The Turning's card / Ludii L-GDL export (`EXP-3`) — a **thin wrapper over the
//! generic [`dli_game::export`] engine**.
//!
//! Both renders are a *straight structural render* of the loaded Turning
//! [`Ruleset`] — the `DL-0007` triviality bar: *if the export is not trivial, the
//! rules were entangled with the kernel.* Since the whole deck is now data compiled
//! by the generic engine, this module keeps only the **world-side selection**: it
//! maps the export options' region/variant selectors onto the honest, keep-stamped,
//! or salt-fringe ruleset ([`ruleset_for`]) and hands that to
//! [`dli_game::export::run`]. The card render honors the world's declared profile
//! render rules (`worlds/tpth/export/profiles.json`); the Ludii render enumerates
//! every leaf, the static [`Ruleset::follows`] successor edges, and the terminal
//! layouts.
//!
//! The variant selection is **lore** (`region`/`variant` selectors), never a
//! profile edit — the keep-stamped deck loses `the-debt-square` by the compile (its
//! required `weather-debt` card is cut), not by a profile author's hand.

use dli_export::profile::{ExportError, ExportOpts, ExportTarget, Registry};

use dli_game::model::Ruleset;

pub use dli_game::export::{
    CardDeck, DeckDiff, DifferentialStatus, GameArtifact, GameExport, LudiiExport, RenderedCard,
    compile_cards, compile_ludii, diff_decks, follow_edge_count,
};

use crate::turning::{keep_stamped, salt_fringe, the_turning};

/// Select the ruleset a set of options compiles: the salt-fringe regional deck, the
/// keep-stamped variant, or the honest lowlands deck (the default). The variant is
/// **lore** (`region`/`variant` selectors), never a profile edit.
///
/// # Errors
///
/// [`ExportError::UnsupportedOption`] when a selector value is unknown or when
/// both region and variant are supplied; no combined materialization is defined.
pub fn ruleset_for(target: ExportTarget, opts: &ExportOpts) -> Result<Ruleset, ExportError> {
    if opts
        .region
        .as_deref()
        .is_some_and(|region| region != "salt-fringe")
    {
        return Err(ExportError::UnsupportedOption {
            target,
            option: "region",
        });
    }
    if opts
        .variant
        .as_deref()
        .is_some_and(|variant| variant != "keep-stamped")
    {
        return Err(ExportError::UnsupportedOption {
            target,
            option: "variant",
        });
    }
    match (opts.region.as_deref(), opts.variant.as_deref()) {
        (None, None) => Ok(the_turning()),
        (Some("salt-fringe"), None) => Ok(salt_fringe()),
        (None, Some("keep-stamped")) => Ok(keep_stamped()),
        (Some("salt-fringe"), Some("keep-stamped")) => Err(ExportError::UnsupportedOption {
            target,
            option: "variant",
        }),
        _ => unreachable!("selectors were validated above"),
    }
}

/// Render the Turning under a card or Ludii target, returning its artifact and
/// manifest. The **world-side dispatch**: it builds the ruleset from the
/// region/variant selectors ([`ruleset_for`]) and delegates the whole structural
/// render to [`dli_game::export::run`].
///
/// # Errors
///
/// [`ExportError::UnknownProfile`] / [`ExportError::NoDefaultProfile`] when no
/// profile resolves, [`ExportError::ProfileTargetMismatch`] when a named profile is
/// for a different target, [`ExportError::UnsupportedOption`] when selectors are
/// unknown or incompatible, and [`ExportError::NoEngineRendererForTarget`] for a
/// non-card/Ludii target.
pub fn run(
    registry: &Registry,
    target: ExportTarget,
    opts: &ExportOpts,
) -> Result<GameExport, ExportError> {
    let ruleset = ruleset_for(target, opts)?;
    let mut generic_opts = opts.clone();
    generic_opts.region = None;
    generic_opts.variant = None;
    dli_game::export::run(registry, target, &ruleset, &generic_opts)
}
