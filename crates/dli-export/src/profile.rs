//! Export backend: the one-way profile core and manifest (`EXP-1`).
//!
//! [`export`] is the whole export contract in one function (spec 09 §2): it reads
//! the **certain layer** of a [`CompiledWorld`] by default (fail-closed on
//! proposed lore unless [`ExportOpts::fork`] opts in), resolves a [`ProfileSpec`]
//! from the [`Registry`], applies the shared keep-stamp visibility filter, hands
//! the visible certain atoms to the target emitter, and pins an
//! [`ExportManifest`] that traces the artifact to the exact canon state.
//!
//! **A profile is data.** [`ProfileSpec`] holds only *selection and renaming* — no
//! logic. Adding a target is a [`Registry`] entry (data), never kernel code (spec
//! 09 §2 clause 2). The [`ExportTarget`] family is the registry's key and is a
//! superset of the wire [`ExportProfile`] selector.
//!
//! **One-way, absolutely.** There is no import path — [`refuse_write_back`] is the
//! only answer a write-back request gets, and it is worked in full in the charter
//! (trial 3): the tool's field value is not an origin with authority (koan 1), the
//! machine never writes canon (koan 3), integration is by declaration not by API
//! (koan 10), and a studio's writers enter through Door H as `proposed`
//! declarations awaiting a ruling (koan 11).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use dli_core::ann::Status;
use dli_core::valuation::Valuation;
use dli_eval::fork::{KeepStamp, Visibility};
use dli_eval::world::GroundAtom;
use dli_world::CompiledWorld;
use dli_world::model::LoadedAtom;

use crate::ExportManifest;
use crate::ExportProfile;
use crate::articy::{self, ArticyExport};
use crate::context_pack::{self, ContextPack};
use crate::games::{self, CardDeck, LudiiExport};

/// The export target family — the [`Registry`]'s key.
///
/// A **superset** of the wire [`ExportProfile`] selector: it adds
/// [`ExportTarget::Articy`] (EXP-4), which has no wire variant yet.
/// Adding a wire variant is a contract change (it regenerates the golden schema);
/// adding a *target* here is additive and internal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ExportTarget {
    /// The audio-script context pack — export backend number one, in production.
    ContextPack,
    /// Card and board game content sets.
    Cards,
    /// Ludii L-GDL for the game simulator (the DL-0007 proof point).
    LudiiGdl,
    /// Articy-shaped video-game JSON (typed entities, relations, quest structure).
    Articy,
}

impl ExportTarget {
    /// The internal target a frozen wire [`ExportProfile`] selects.
    #[must_use]
    pub fn from_wire(profile: ExportProfile) -> Self {
        match profile {
            ExportProfile::ContextPack => Self::ContextPack,
            ExportProfile::Cards => Self::Cards,
            ExportProfile::LudiiGdl => Self::LudiiGdl,
        }
    }
}

/// The sorts and relations a profile selects out of the compiled world.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Selection {
    /// The entity sorts the target consumes.
    pub sorts: Vec<String>,
    /// The relations the target consumes.
    pub relations: Vec<String>,
}

/// How one selected sort renders: the id template and the fields kept.
///
/// This is *rendering only* — a template string and a keep-list. A profile that
/// needs to **compute** rather than render has smuggled rules out of the fabric
/// (spec 09 §5) and is a finding, not a feature.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RenderRule {
    /// The id template, e.g. `{role}-{ground}`.
    pub id: String,
    /// The fields carried onto the rendered leaf.
    pub keep: Vec<String>,
}

/// Whether cited provenance travels with the artifact.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceMode {
    /// Cited decisions ride as `meta.cited` dev-metadata (stripped for ship).
    #[default]
    DevMetadata,
    /// Provenance is stripped — the shipped artifact carries knowledge without the
    /// authoring apparatus.
    Stripped,
}

/// A versioned export profile — **data, not a code fork** (spec 09 §2 clause 2).
///
/// It names the target, the sorts and relations consumed, how each renders, and
/// whether provenance ships. It holds no logic beyond selection and renaming, so
/// adding a target is writing one of these, never touching the kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProfileSpec {
    /// The target this profile compiles for.
    pub target: ExportTarget,
    /// The profile name (e.g. `cards/lowlands`).
    pub name: String,
    /// The profile version — the `@n` in `cards/lowlands@3`.
    pub version: u32,
    /// The sorts and relations selected.
    pub selects: Selection,
    /// How each selected sort renders, keyed by sort name.
    pub renders: BTreeMap<String, RenderRule>,
    /// Whether cited provenance travels with the artifact.
    pub carries_provenance: ProvenanceMode,
}

impl ProfileSpec {
    /// The `name@version` identity used in a manifest's `profile` field.
    #[must_use]
    pub fn qualified(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

/// The profile registry: the set of known [`ProfileSpec`]s. Adding a target is
/// pushing one here — the mechanism that makes "adding a target is a profile,
/// never kernel code" literally true (spec 09 §2 clause 2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Registry {
    /// The registered profiles.
    pub profiles: Vec<ProfileSpec>,
}

impl Registry {
    /// The built-in profiles — one default per target, mirroring the spec's
    /// worked profile data (spec 09 §2, §6).
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            profiles: vec![
                ProfileSpec {
                    target: ExportTarget::ContextPack,
                    name: "context-pack/default".to_owned(),
                    version: 1,
                    selects: Selection::default(),
                    renders: BTreeMap::new(),
                    carries_provenance: ProvenanceMode::DevMetadata,
                },
                ProfileSpec {
                    target: ExportTarget::Cards,
                    name: "cards/lowlands".to_owned(),
                    version: 3,
                    selects: Selection {
                        sorts: vec![
                            "ground".to_owned(),
                            "hand".to_owned(),
                            "weather".to_owned(),
                            "reckoner".to_owned(),
                        ],
                        relations: vec!["rides-figure".to_owned(), "ring-next".to_owned()],
                    },
                    renders: BTreeMap::from([
                        (
                            "hand".to_owned(),
                            RenderRule {
                                id: "{role}-{ground}".to_owned(),
                                keep: vec![
                                    "role".to_owned(),
                                    "ground".to_owned(),
                                    "figure".to_owned(),
                                ],
                            },
                        ),
                        (
                            "weather".to_owned(),
                            RenderRule {
                                id: "weather-{name}".to_owned(),
                                keep: vec!["name".to_owned(), "ring_next".to_owned()],
                            },
                        ),
                    ]),
                    carries_provenance: ProvenanceMode::DevMetadata,
                },
                ProfileSpec {
                    target: ExportTarget::LudiiGdl,
                    name: "games/ludii".to_owned(),
                    version: 1,
                    selects: Selection {
                        sorts: vec!["ground".to_owned(), "hand".to_owned(), "weather".to_owned()],
                        relations: vec!["legal-move".to_owned(), "terminal".to_owned()],
                    },
                    renders: BTreeMap::new(),
                    carries_provenance: ProvenanceMode::Stripped,
                },
                ProfileSpec {
                    target: ExportTarget::Articy,
                    name: "video/articy".to_owned(),
                    version: 1,
                    selects: Selection {
                        sorts: vec!["person".to_owned(), "place".to_owned(), "thing".to_owned()],
                        relations: vec!["knows".to_owned(), "promise".to_owned()],
                    },
                    renders: BTreeMap::new(),
                    carries_provenance: ProvenanceMode::DevMetadata,
                },
            ],
        }
    }

    /// The profile registered under a `name` or `name@version` string.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&ProfileSpec> {
        let (base, version) = match name.split_once('@') {
            Some((base, ver)) => (base, ver.parse::<u32>().ok()),
            None => (name, None),
        };
        self.profiles
            .iter()
            .find(|p| p.name == base && version.is_none_or(|v| v == p.version))
    }

    /// The default profile for a target — the first registered for it.
    #[must_use]
    pub fn default_for(&self, target: ExportTarget) -> Option<&ProfileSpec> {
        self.profiles.iter().find(|p| p.target == target)
    }
}

/// The options that shape an export.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportOpts {
    /// The source commit to pin in the manifest. The kernel is offline and reads
    /// no git; the caller supplies the commit its canon was compiled from.
    pub source_commit: String,
    /// An explicit fork valuation to export under, applied over the ambient ledger
    /// (the overlay wins). `None` reads the certain layer under the ambient ledger
    /// — the fail-closed default (spec 09 §2 clause 1, `--fork`).
    pub fork: Option<Valuation>,
    /// Keep-stamp visibility restrictions applied before the emitter sees the
    /// atoms — the shared who-may-know-what filter (spec 09 §3). A `Removed`
    /// keep-stamp is how a lore variant delta's cut compiles (§6): the leaves it
    /// covers vanish, so a layout that read them no longer compiles.
    pub keep_stamps: Vec<KeepStamp>,
    /// A named profile override (`name` or `name@version`); `None` uses the
    /// target's default profile.
    pub profile_name: Option<String>,
    /// A regional delta selector (lore, not a profile edit): which region's
    /// substitutions a target compiler applies (e.g. `salt-fringe`). `None` is the
    /// base region.
    pub region: Option<String>,
    /// A variant delta selector (lore, not a profile edit): which named variant a
    /// target compiler applies (e.g. `keep-stamped`). `None` is the honest deck.
    pub variant: Option<String>,
}

/// The rendered, target-shaped artifact an export emits — knowledge, never
/// behavior (spec 09 §2 clause 3). Serialised untagged, so a consumer that pulls
/// one out gets the target's own shape with no envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExportArtifact {
    /// A context pack (EXP-2) — the certain layer as prompt-injection blocks.
    ContextPack(ContextPack),
    /// A card set (EXP-3, the §6 worked export) — the compiled deck.
    Cards(CardDeck),
    /// Ludii L-GDL (EXP-3) — the rendered program plus its differential status.
    LudiiGdl(LudiiExport),
    /// Articy-shaped video-game JSON (EXP-4 scaffold).
    Articy(ArticyExport),
}

impl ExportArtifact {
    /// The context pack, if this artifact is one.
    #[must_use]
    pub fn as_context_pack(&self) -> Option<&ContextPack> {
        match self {
            Self::ContextPack(pack) => Some(pack),
            _ => None,
        }
    }

    /// The card deck, if this artifact is one.
    #[must_use]
    pub fn as_cards(&self) -> Option<&CardDeck> {
        match self {
            Self::Cards(deck) => Some(deck),
            _ => None,
        }
    }
}

/// The outcome of an export: the artifact and the manifest that traces it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportOutcome {
    /// The rendered, target-shaped artifact.
    pub artifact: ExportArtifact,
    /// The provenance manifest pinning the artifact to the exact canon state.
    pub manifest: ExportManifest,
}

/// A failure of an export.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExportError {
    /// The named profile is not registered.
    #[error("no export profile named '{0}' is registered")]
    UnknownProfile(String),
    /// No default profile is registered for the requested target.
    #[error("no default profile is registered for target {0:?}")]
    NoDefaultProfile(ExportTarget),
    /// The resolved profile targets something other than what was requested.
    #[error("profile '{name}' targets {found:?}, not the requested {requested:?}")]
    ProfileTargetMismatch {
        /// The profile name.
        name: String,
        /// The target the profile declares.
        found: ExportTarget,
        /// The target that was requested.
        requested: ExportTarget,
    },
    /// A write-back was requested. There is no import path — exports are one-way,
    /// absolutely (charter koan 1/3/10/11).
    #[error(
        "export is one-way: there is no write-back path — a downstream change comes \
         home as a proposed declaration through an authoring door (charter trial 3)"
    )]
    WriteBackRefused,
}

/// The refusal a `--write-back` request gets: there is no import path, by
/// principle (spec 09 §2, charter koan 10 — one fabric refuses two truths).
///
/// This is a *function that returns the refusal*, not a stub that could grow an
/// import: there is deliberately nothing here to call to write canon.
///
/// ```
/// use dli_export::{ExportError, refuse_write_back};
/// assert_eq!(refuse_write_back(), ExportError::WriteBackRefused);
/// ```
#[must_use]
pub fn refuse_write_back() -> ExportError {
    ExportError::WriteBackRefused
}

/// Render a compiled world out under a target, returning its artifact and
/// manifest.
///
/// The pipeline is the four-clause contract (spec 09 §2): resolve the
/// [`ProfileSpec`] (default or [`ExportOpts::profile_name`]); pick the source
/// layer — the certain layer under the ambient ledger by default, or the fork
/// valuation under [`ExportOpts::fork`]; read the certain atoms; apply the shared
/// keep-stamp filter; hand the visible atoms to the target emitter; and pin the
/// manifest.
///
/// # Errors
///
/// [`ExportError::UnknownProfile`] / [`ExportError::NoDefaultProfile`] when no
/// profile resolves, and [`ExportError::ProfileTargetMismatch`] when a named
/// profile is for a different target.
///
/// ```
/// use dli_export::{ExportOpts, ExportTarget, export};
/// use dli_world::compile;
/// use dli_world::pack::Pack;
/// use dli_world::loader::example_docs;
///
/// let pack = Pack { name: "saltmarket".to_owned(), documents: example_docs() };
/// let world = compile(&pack).expect("compiles");
/// let outcome = export(ExportTarget::ContextPack, &world, &ExportOpts::default())
///     .expect("context pack exports");
/// // Fail-closed default: the certain layer under the ambient ledger.
/// assert_eq!(outcome.manifest.layer, "certain");
/// ```
pub fn export(
    target: ExportTarget,
    world: &CompiledWorld,
    opts: &ExportOpts,
) -> Result<ExportOutcome, ExportError> {
    let registry = Registry::builtin();
    let spec = match &opts.profile_name {
        Some(name) => registry
            .resolve(name)
            .ok_or_else(|| ExportError::UnknownProfile(name.clone()))?,
        None => registry
            .default_for(target)
            .ok_or(ExportError::NoDefaultProfile(target))?,
    };
    if spec.target != target {
        return Err(ExportError::ProfileTargetMismatch {
            name: spec.name.clone(),
            found: spec.target,
            requested: target,
        });
    }

    let (layer, valuation) = layer_valuation(world, opts);
    let certain = certain_atoms_under(world, &valuation);
    let visible = apply_keep_stamps(certain, &opts.keep_stamps);

    let artifact = match target {
        ExportTarget::ContextPack => {
            ExportArtifact::ContextPack(context_pack::build(spec, &layer, &visible))
        }
        ExportTarget::Cards => {
            let ruleset = games::ruleset_for(opts);
            ExportArtifact::Cards(games::compile_cards(&ruleset, spec, &layer, opts))
        }
        ExportTarget::LudiiGdl => {
            let ruleset = games::ruleset_for(opts);
            ExportArtifact::LudiiGdl(games::compile_ludii(&ruleset, spec, &layer))
        }
        ExportTarget::Articy => {
            ExportArtifact::Articy(articy::compile_scaffold(spec, &layer, &visible))
        }
    };

    let manifest = ExportManifest {
        commit: opts.source_commit.clone(),
        layer,
        valuation: digest_of(&valuation),
        profile_hash: digest_of(spec),
    };
    Ok(ExportOutcome { artifact, manifest })
}

/// The layer name and the valuation an export reads under.
///
/// `None` fork → the ambient ledger and the `"certain"` layer (the fail-closed
/// default). `Some` fork → the ambient ledger with the fork overlay laid on top
/// (the overlay wins, verbatim — no re-derivation) and the `"fork"` layer.
fn layer_valuation(world: &CompiledWorld, opts: &ExportOpts) -> (String, Valuation) {
    let ambient = &world.model().world.valuation;
    match &opts.fork {
        None => ("certain".to_owned(), ambient.clone()),
        Some(overlay) => {
            let mut valuation = ambient.clone();
            for (&atom, &value) in &overlay.0 {
                valuation.set(atom, value);
            }
            ("fork".to_owned(), valuation)
        }
    }
}

/// Every source atom that stands **certain** under the ambient ledger — the
/// default export layer. Proposed lore (merely possible) is excluded; this is the
/// fail-closed read (spec 09 §2 clause 1).
#[must_use]
pub fn certain_atoms(world: &CompiledWorld) -> Vec<&LoadedAtom> {
    certain_atoms_under(world, &world.model().world.valuation)
}

/// Every source atom that stands certain under an explicit valuation — the
/// `--fork` read (the fork's valuation may raise a shadow to certain).
#[must_use]
pub fn certain_atoms_under<'w>(
    world: &'w CompiledWorld,
    valuation: &Valuation,
) -> Vec<&'w LoadedAtom> {
    let model = world.model();
    let theory = &model.world.theory;
    model
        .atoms()
        .iter()
        .filter(|atom| atom.ann.status(valuation, theory) == Status::Certain)
        .collect()
}

/// Drop the keep-stamped-hidden atoms — the shared who-may-know-what visibility
/// filter (spec 09 §3), reusing the kernel's canonical [`KeepStamp`].
///
/// A `Removed` or `KeptAbsent` stamp hides every atom whose predicate or rendered
/// `pred(args)` is in its scope; `KeptPresent` never hides (it forces presence,
/// a no-op for a filter). The atom is still true in the store — this is
/// *visibility, not valuation* — it is only withheld from the artifact. The
/// honest-vs-keep-stamped diff of two exports is the allegory as data.
///
/// ```
/// use dli_eval::fork::{KeepStamp, Visibility};
/// use dli_eval::world::GroundAtom;
/// use dli_core::ann::Ann;
/// use dli_world::loader::Loader;
/// use dli_world::CompiledWorld;
/// use dli_export::profile::{apply_keep_stamps, certain_atoms};
/// use dli_world::loader::example_docs;
/// use dli_world::compile;
/// use dli_world::pack::Pack;
///
/// let pack = Pack { name: "saltmarket".to_owned(), documents: example_docs() };
/// let world = compile(&pack).expect("compiles");
/// let all = certain_atoms(&world);
/// // Hiding a predicate that appears in canon drops at least one atom.
/// let pred = all[0].atom.pred.text.clone();
/// let stamp = KeepStamp { scope: vec![pred], visibility: Visibility::Removed };
/// let visible = apply_keep_stamps(all.clone(), std::slice::from_ref(&stamp));
/// assert!(visible.len() < all.len());
/// ```
#[must_use]
pub fn apply_keep_stamps<'a>(
    atoms: Vec<&'a LoadedAtom>,
    stamps: &[KeepStamp],
) -> Vec<&'a LoadedAtom> {
    if stamps.is_empty() {
        return atoms;
    }
    atoms
        .into_iter()
        .filter(|atom| !hidden_by(atom, stamps))
        .collect()
}

/// Whether a keep-stamp with hiding visibility covers this atom, by predicate or
/// by rendered `pred(args)`.
fn hidden_by(atom: &LoadedAtom, stamps: &[KeepStamp]) -> bool {
    let rendered = GroundAtom::from_pattern(&atom.atom).map(|ground| ground.render());
    let pred = atom.atom.pred.text.as_str();
    stamps.iter().any(|stamp| {
        matches!(
            stamp.visibility,
            Visibility::Removed | Visibility::KeptAbsent
        ) && stamp
            .scope
            .iter()
            .any(|entry| entry == pred || rendered.as_deref() == Some(entry.as_str()))
    })
}

/// A `sha256:<hex>` digest over a value's canonical serialisation — the manifest
/// fingerprint domain (matching the golden fixture's `sha256:` form). Deterministic
/// `serde_json` encoding suffices: this engine both mints and compares the digest.
fn digest_of<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let hash = Sha256::digest(&bytes);
    let mut out = String::with_capacity(7 + hash.len() * 2);
    out.push_str("sha256:");
    for byte in hash {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
