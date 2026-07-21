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
#[must_use]
pub fn ruleset_for(opts: &ExportOpts) -> Ruleset {
    match (opts.region.as_deref(), opts.variant.as_deref()) {
        (Some("salt-fringe"), _) => salt_fringe(),
        (_, Some("keep-stamped")) => keep_stamped(),
        _ => the_turning(),
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
/// for a different target, and [`ExportError::NoEngineRendererForTarget`] for a
/// non-card/Ludii target (the context pack and articy scaffold are rendered by the
/// world-atom export, not here).
pub fn run(
    registry: &Registry,
    target: ExportTarget,
    opts: &ExportOpts,
) -> Result<GameExport, ExportError> {
    let ruleset = ruleset_for(opts);
    dli_game::export::run(registry, target, &ruleset, opts)
}
