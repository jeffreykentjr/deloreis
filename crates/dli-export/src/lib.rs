//! Deloreis Lore Interchange export profiles.
//!
//! A compiled world can be rendered under an [`ExportProfile`] as a context
//! pack, articy-shaped JSON, CSV, or a static wiki; game rulesets use the same
//! profile vocabulary for cards and Ludii GDL. Every export carries an
//! [`ExportManifest`] pinning the commit, model layer, valuation, profile, and
//! artifact-shaping options.
//!
//! # The four-clause contract (spec 09 §2)
//!
//! Exports go **one direction only, out of the certain layer**. The core is
//! [`export`], which reads the [`CompiledWorld`]:
//!
//! 1. **Source layer.** The **certain layer by default** — proposed lore never
//!    exports by accident, it fails closed, exactly as the context-pack builder
//!    behaves. A fork's valuation exports only under an explicit
//!    [`ExportOpts::fork`].
//! 2. **Profile.** A [`ProfileSpec`] is **data, not a code fork**: it names the
//!    sorts and relations a target consumes and how each renders. Adding a
//!    mapping for an existing target is data; a new emission format requires a
//!    renderer and contract migration.
//! 3. **Emission.** Target-shaped data ([`ExportArtifact`]): a context pack, a
//!    card set, GDL. Exports carry **knowledge** — entities, facts, relations,
//!    who-may-know scopes, variant deltas — **never behavior**.
//! 4. **Manifest.** [`ExportManifest`] pins source commit, layer, valuation
//!    fingerprint, and profile hash, so a wrong artifact is a *diagnosis*, never a
//!    mystery.
//!
//! **One-way, absolutely.** There is no import path; [`refuse_write_back`] is the
//! only answer a `--write-back` request gets (charter koan 1/3/10/11).
//!
//! # The wire types are schema-controlled
//!
//! [`ExportProfile`] and [`ExportManifest`] are part of the `dli.export.run`
//! wire contract. Their JSON Schema is generated into
//! `contracts/dli-worker/schemas/` and diffed by `xtask schema --check`; changing
//! either requires an explicit contract migration.

pub mod articy;
pub mod context_pack;
pub mod csv;
pub mod profile;
pub mod projection;
pub mod static_wiki;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_world::CompiledWorld;

pub use articy::ArticyExport;
pub use context_pack::{ContextPack, PackedUnit};
pub use csv::CsvExport;
pub use profile::{
    ExportArtifact, ExportError, ExportOpts, ExportOutcome, ExportTarget, ProfileSpec,
    ProvenanceMode, Registry, RegistryLoadError, RenderRule, Selection, export, export_with,
    refuse_write_back, source_fingerprint,
};
pub use static_wiki::{StaticWikiExport, WikiPage};

/// The export target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ExportProfile {
    /// A context pack for downstream generation.
    ContextPack,
    /// Printable reference cards.
    Cards,
    /// Ludii Game Description Language for the simulator.
    LudiiGdl,
    /// Articy-shaped video-game JSON.
    Articy,
    /// Producer-facing flat CSV.
    Csv,
    /// A static Markdown wiki bundle.
    StaticWiki,
}

/// Provenance for an exported artifact.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportManifest {
    /// The source commit the export was built from.
    pub commit: String,
    /// The model layer exported (e.g. ratified-only).
    pub layer: String,
    /// A fingerprint of the valuation the export was taken under.
    pub valuation: String,
    /// A hash of the export profile and its options.
    pub profile_hash: String,
}

/// Render a compiled world out under a profile, returning its artifact and
/// manifest.
///
/// This is the thin façade the wire selector calls: it maps the frozen
/// [`ExportProfile`] to its internal [`ExportTarget`] and delegates to the export
/// core, [`profile::export`], which reads the certain layer and pins the
/// manifest.
///
/// ```
/// use dli_export::{ExportOpts, ExportProfile, export_wire};
/// use dli_world::compile;
/// use dli_world::pack::Pack;
/// use dli_world::loader::example_docs;
///
/// let pack = Pack { name: "camelot".to_owned(), documents: example_docs() };
/// let world = compile(&pack).expect("compiles");
/// let opts = ExportOpts {
///     source_commit: "content-sha256:example".to_owned(),
///     ..ExportOpts::default()
/// };
/// let outcome = export_wire(ExportProfile::ContextPack, &world, &opts)
///     .expect("context pack always exports");
/// // The manifest pins the exact canon state (spec 09 §2 clause 4).
/// assert_eq!(outcome.manifest.layer, "certain");
/// assert!(outcome.manifest.valuation.starts_with("sha256:"));
/// ```
pub fn export_wire(
    profile: ExportProfile,
    world: &CompiledWorld,
    opts: &ExportOpts,
) -> Result<ExportOutcome, ExportError> {
    export(ExportTarget::from_wire(profile), world, opts)
}
