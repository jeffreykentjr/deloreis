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
//! logic. Adding a mapping for an existing target is a [`Registry`] entry;
//! introducing a new emission format requires an explicit renderer and contract
//! migration (spec 09 §2 clause 2). The [`ExportTarget`] family is the registry
//! key shared by the wire and CLI selectors.
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
use crate::csv::{self, CsvExport};
use crate::static_wiki::{self, StaticWikiExport};

/// The export target family — the [`Registry`]'s key.
///
/// The internal target selected by the wire and CLI surfaces.
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
    /// Producer-facing flat CSV.
    Csv,
    /// A static Markdown wiki bundle.
    StaticWiki,
}

impl ExportTarget {
    /// The internal target a frozen wire [`ExportProfile`] selects.
    #[must_use]
    pub fn from_wire(profile: ExportProfile) -> Self {
        match profile {
            ExportProfile::ContextPack => Self::ContextPack,
            ExportProfile::Cards => Self::Cards,
            ExportProfile::LudiiGdl => Self::LudiiGdl,
            ExportProfile::Articy => Self::Articy,
            ExportProfile::Csv => Self::Csv,
            ExportProfile::StaticWiki => Self::StaticWiki,
        }
    }
}

impl std::fmt::Display for ExportTarget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::ContextPack => "context-pack",
            Self::Cards => "cards",
            Self::LudiiGdl => "ludii-gdl",
            Self::Articy => "articy",
            Self::Csv => "csv",
            Self::StaticWiki => "static-wiki",
        };
        formatter.write_str(name)
    }
}

/// The sorts and relations a profile selects out of the compiled world.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
/// adding a mapping for an existing target never touches the kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileSpec {
    /// The target this profile compiles for.
    pub target: ExportTarget,
    /// The profile name (e.g. `context-pack/default`, or a world's `cards/<region>`).
    pub name: String,
    /// The profile version — the `@n` in a qualified name like `cards/<region>@3`.
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

/// The profile registry: the set of known [`ProfileSpec`]s. Adding a mapping for
/// an existing target is pushing one here (spec 09 §2 clause 2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// The registered profiles.
    pub profiles: Vec<ProfileSpec>,
}

impl Registry {
    /// The built-in profiles — the world-agnostic base catalog.
    ///
    /// This is the engine's neutral catalog. Each generic world target names no
    /// sorts, relations, or world, so its empty selection means all certain
    /// predicates. Concrete,
    /// world-specific catalogs (the Turning card deck, the Ludii ruleset export,
    /// a world's `video/articy` profile) are **data**, not kernel code (spec 09 §2 clause 2):
    /// they live under `worlds/<world>/export/profiles.json` and are loaded with
    /// [`Registry::from_path`] and merged in with [`Registry::merge`]. A profile
    /// is data, not a code fork — so no world's profile catalog is hardcoded here.
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            profiles: vec![
                neutral_profile(ExportTarget::ContextPack, "context-pack/default"),
                neutral_profile(ExportTarget::Articy, "video/articy"),
                neutral_profile(ExportTarget::Csv, "tabular/csv"),
                neutral_profile(ExportTarget::StaticWiki, "wiki/static"),
            ],
        }
    }

    /// Load a profile catalog (a serialised [`Registry`]) from a JSON file — the
    /// mechanism that makes "a profile is data, not a code fork" literal: a world
    /// ships its export profiles as data under `worlds/<world>/export/profiles.json`
    /// and the engine reads them, never hardcodes them.
    ///
    /// The loaded catalog is typically merged onto [`Registry::builtin`] with
    /// [`Registry::merge`] so the neutral base and the world's profiles both
    /// resolve.
    ///
    /// # Errors
    ///
    /// [`RegistryLoadError::Read`] if the file cannot be read, and
    /// [`RegistryLoadError::Parse`] if its contents are not a valid serialised
    /// [`Registry`].
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, RegistryLoadError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| RegistryLoadError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let registry: Self =
            serde_json::from_slice(&bytes).map_err(|source| RegistryLoadError::Parse {
                path: path.display().to_string(),
                source,
            })?;
        registry
            .validate()
            .map_err(|message| RegistryLoadError::Validate {
                path: path.display().to_string(),
                message,
            })?;
        Ok(registry)
    }

    /// Merge another catalog onto this one, with the later catalog taking
    /// precedence for an identical qualified identity.
    ///
    /// Defaults are resolved from the last profile registered for a target, so a
    /// caller catalog naturally overrides a neutral built-in target.
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        let incoming = other
            .profiles
            .iter()
            .map(ProfileSpec::qualified)
            .collect::<std::collections::BTreeSet<_>>();
        self.profiles
            .retain(|existing| !incoming.contains(&existing.qualified()));
        self.profiles.extend(other.profiles);
        self
    }

    /// Validate profile identities and selector structure.
    ///
    /// Validation runs at export time as well as file-load time so catalogs
    /// assembled in memory cannot bypass it.
    pub fn validate(&self) -> Result<(), String> {
        let mut identities = std::collections::BTreeSet::new();
        for profile in &self.profiles {
            if profile.name.trim().is_empty() || profile.name.contains('@') {
                return Err("profile names must be non-empty and must not contain `@`".to_owned());
            }
            if profile.version == 0 {
                return Err(format!(
                    "profile `{}` must use a version of at least 1",
                    profile.name
                ));
            }
            if !identities.insert(profile.qualified()) {
                return Err(format!(
                    "profile identity `{}` is registered more than once",
                    profile.qualified()
                ));
            }
            reject_duplicates(
                &profile.selects.sorts,
                &format!("profile `{}` sorts", profile.qualified()),
            )?;
            reject_duplicates(
                &profile.selects.relations,
                &format!("profile `{}` relations", profile.qualified()),
            )?;
            if let Some(name) = profile
                .selects
                .sorts
                .iter()
                .find(|name| profile.selects.relations.contains(name))
            {
                return Err(format!(
                    "profile `{}` selects `{name}` as both a sort and relation",
                    profile.qualified()
                ));
            }
            for (predicate, rule) in &profile.renders {
                if predicate.trim().is_empty() {
                    return Err(format!(
                        "profile `{}` has an empty render-rule key",
                        profile.qualified()
                    ));
                }
                reject_duplicates(
                    &rule.keep,
                    &format!(
                        "profile `{}` render rule `{predicate}` keep-list",
                        profile.qualified()
                    ),
                )?;
                let wildcard =
                    profile.selects.sorts.is_empty() && profile.selects.relations.is_empty();
                let selected = profile.selects.sorts.contains(predicate)
                    || profile.selects.relations.contains(predicate);
                if !wildcard && !selected {
                    return Err(format!(
                        "profile `{}` has a render rule for unselected predicate `{predicate}`",
                        profile.qualified()
                    ));
                }
                validate_render_rule(profile, predicate, rule)?;
            }
        }
        Ok(())
    }

    /// The profile registered under a `name` or `name@version` string.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&ProfileSpec> {
        let (base, version) = match name.split_once('@') {
            Some((base, ver)) => (base, Some(ver.parse::<u32>().ok()?)),
            None => (name, None),
        };
        self.profiles
            .iter()
            .filter(|profile| {
                profile.name == base && version.is_none_or(|value| value == profile.version)
            })
            .max_by_key(|profile| profile.version)
    }

    /// The default profile for a target — the last registered for it.
    #[must_use]
    pub fn default_for(&self, target: ExportTarget) -> Option<&ProfileSpec> {
        self.profiles
            .iter()
            .rev()
            .find(|profile| profile.target == target)
    }

    /// Resolve and target-check a profile after validating the full registry.
    pub fn resolve_for(
        &self,
        target: ExportTarget,
        name: Option<&str>,
    ) -> Result<&ProfileSpec, ExportError> {
        self.validate()
            .map_err(ExportError::InvalidProfileRegistry)?;
        let spec = match name {
            Some(name) => self
                .resolve(name)
                .ok_or_else(|| ExportError::UnknownProfile(name.to_owned()))?,
            None => self
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
        Ok(spec)
    }
}

fn neutral_profile(target: ExportTarget, name: &str) -> ProfileSpec {
    ProfileSpec {
        target,
        name: name.to_owned(),
        version: 1,
        selects: Selection::default(),
        renders: BTreeMap::new(),
        carries_provenance: ProvenanceMode::DevMetadata,
    }
}

fn reject_duplicates(values: &[String], label: &str) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    if let Some(value) = values.iter().find(|value| !seen.insert(value.as_str())) {
        return Err(format!("{label} contains duplicate `{value}`"));
    }
    Ok(())
}

fn validate_render_rule(
    profile: &ProfileSpec,
    predicate: &str,
    rule: &RenderRule,
) -> Result<(), String> {
    let placeholders = template_placeholders(&rule.id).map_err(|detail| {
        format!(
            "profile `{}` render rule `{predicate}` {detail}",
            profile.qualified()
        )
    })?;
    match profile.target {
        ExportTarget::ContextPack if !profile.renders.is_empty() => {
            return Err(format!(
                "profile `{}` targets context-pack, which does not accept render rules",
                profile.qualified()
            ));
        }
        ExportTarget::Articy => {
            reject_unknown_fields(profile, predicate, &rule.keep, |field| {
                matches!(field, "id" | "sort" | "facts")
            })?;
            reject_unknown_fields(profile, predicate, &placeholders, |field| {
                matches!(field, "id" | "sort") || positional_field(field)
            })?;
        }
        ExportTarget::Csv | ExportTarget::StaticWiki => {
            reject_unknown_fields(profile, predicate, &rule.keep, positional_field)?;
            reject_unknown_fields(profile, predicate, &placeholders, |field| {
                matches!(field, "predicate" | "claim") || positional_field(field)
            })?;
        }
        ExportTarget::Cards | ExportTarget::LudiiGdl | ExportTarget::ContextPack => {}
    }
    Ok(())
}

fn reject_unknown_fields(
    profile: &ProfileSpec,
    predicate: &str,
    fields: &[String],
    allowed: impl Fn(&str) -> bool,
) -> Result<(), String> {
    if let Some(field) = fields.iter().find(|field| !allowed(field)) {
        return Err(format!(
            "profile `{}` render rule `{predicate}` names unavailable field `{field}`",
            profile.qualified()
        ));
    }
    Ok(())
}

fn positional_field(field: &str) -> bool {
    field
        .strip_prefix("arg")
        .is_some_and(|index| !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit()))
}

fn template_placeholders(template: &str) -> Result<Vec<String>, String> {
    let mut placeholders = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        if rest[..open].contains('}') {
            return Err(format!("template `{template}` has an unmatched `}}`"));
        }
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('}') else {
            return Err(format!("template `{template}` has an unmatched `{{`"));
        };
        let field = &after_open[..close];
        if field.is_empty() || field.contains('{') {
            return Err(format!("template `{template}` has an invalid placeholder"));
        }
        placeholders.push(field.to_owned());
        rest = &after_open[close + 1..];
    }
    if rest.contains('}') {
        return Err(format!("template `{template}` has an unmatched `}}`"));
    }
    Ok(placeholders)
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
    /// Keep-stamp visibility restrictions applied before a lore-world emitter
    /// sees the atoms — the shared who-may-know-what filter (spec 09 §3).
    /// Materialized game exporters reject this option.
    pub keep_stamps: Vec<KeepStamp>,
    /// A named profile override (`name` or `name@version`); `None` uses the
    /// target's default profile.
    pub profile_name: Option<String>,
    /// A regional delta selector for a world adapter (lore, not a profile edit).
    /// Generic lore and game exporters reject it because they do not know how to
    /// materialize world deltas.
    pub region: Option<String>,
    /// A variant delta selector for a world adapter (lore, not a profile edit).
    /// Generic lore and game exporters reject it because they do not know how to
    /// materialize world deltas.
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
    /// Articy-shaped video-game JSON (EXP-4): typed entities with fact scopes,
    /// who-may-know facts, relations, and quest flow.
    Articy(ArticyExport),
    /// Producer-facing flat CSV.
    Csv(CsvExport),
    /// Static Markdown wiki bundle.
    StaticWiki(StaticWikiExport),
}

impl ExportArtifact {
    /// The context pack, if this artifact is one.
    #[must_use]
    pub fn as_context_pack(&self) -> Option<&ContextPack> {
        match self {
            Self::ContextPack(pack) => Some(pack),
            Self::Articy(_) | Self::Csv(_) | Self::StaticWiki(_) => None,
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
    /// The caller did not identify the source state used for the export.
    #[error("an export requires a non-empty source commit or content fingerprint")]
    MissingSourceAnchor,
    /// A materialized source is still proposed and therefore cannot be exported
    /// as certain-layer knowledge.
    #[error("target {target} cannot export a proposed source as certain-layer knowledge")]
    UnassertableSource {
        /// Target that refused the proposed source.
        target: ExportTarget,
    },
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
    /// The caller assembled an ambiguous or malformed registry.
    #[error("invalid export profile registry: {0}")]
    InvalidProfileRegistry(String),
    /// One profile cannot be interpreted by its target renderer.
    #[error("invalid export profile '{profile}': {detail}")]
    InvalidProfile {
        /// Qualified profile identity.
        profile: String,
        /// Target-specific validation detail.
        detail: String,
    },
    /// A target renderer could not encode a selected value.
    #[error("the {target} export could not be rendered: {detail}")]
    Render {
        /// Target name.
        target: String,
        /// Encoding detail.
        detail: String,
    },
    /// An option has no defined semantics for the requested target.
    #[error("target {target} does not support export option `{option}`")]
    UnsupportedOption {
        /// Target that refused the option.
        target: ExportTarget,
        /// Refused option name.
        option: &'static str,
    },
    /// The requested target has no engine-side renderer: its renderer lives in a
    /// world crate (the Turning's card/Ludii render moved out of the engine),
    /// reached through that crate's own export entry point, never through this
    /// generic export.
    #[error(
        "target {0:?} has no engine-side renderer; it is rendered by a world crate, \
         through that crate's own export entry point"
    )]
    NoEngineRendererForTarget(ExportTarget),
    /// A write-back was requested. There is no import path — exports are one-way,
    /// absolutely (charter koan 1/3/10/11).
    #[error(
        "export is one-way: there is no write-back path — a downstream change comes \
         home as a proposed declaration through an authoring door (charter trial 3)"
    )]
    WriteBackRefused,
}

/// A failure loading a profile catalog from disk with [`Registry::from_path`].
#[derive(Debug, Error)]
pub enum RegistryLoadError {
    /// The catalog file could not be read.
    #[error("reading profile catalog '{path}': {source}")]
    Read {
        /// The path that failed to read.
        path: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// The catalog file was not a valid serialised [`Registry`].
    #[error("parsing profile catalog '{path}': {source}")]
    Parse {
        /// The path that failed to parse.
        path: String,
        /// The underlying deserialisation error.
        source: serde_json::Error,
    },
    /// The decoded catalog violates registry invariants.
    #[error("validating profile catalog '{path}': {message}")]
    Validate {
        /// The path that failed validation.
        path: String,
        /// Invariant violation.
        message: String,
    },
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
/// let pack = Pack { name: "camelot".to_owned(), documents: example_docs() };
/// let world = compile(&pack).expect("compiles");
/// let opts = ExportOpts {
///     source_commit: "content-sha256:example".to_owned(),
///     ..ExportOpts::default()
/// };
/// let outcome = export(ExportTarget::ContextPack, &world, &opts)
///     .expect("context pack exports");
/// // Fail-closed default: the certain layer under the ambient ledger.
/// assert_eq!(outcome.manifest.layer, "certain");
/// ```
pub fn export(
    target: ExportTarget,
    world: &CompiledWorld,
    opts: &ExportOpts,
) -> Result<ExportOutcome, ExportError> {
    export_with(&Registry::builtin(), target, world, opts)
}

/// [`export`] against an explicit [`Registry`] — the same pipeline, sourcing its
/// profiles from a caller-supplied catalog instead of [`Registry::builtin`].
///
/// This is the seam that keeps profiles *data*: a world loads its catalog with
/// [`Registry::from_path`] (optionally `.merge`d onto the builtin base) and exports
/// through it. The mechanism — layer, certain atoms, keep-stamp filter, manifest —
/// is unchanged; only where the [`ProfileSpec`] comes from differs.
///
/// # Errors
///
/// [`ExportError::UnknownProfile`] / [`ExportError::NoDefaultProfile`] when no
/// profile resolves, and [`ExportError::ProfileTargetMismatch`] when a named
/// profile is for a different target.
pub fn export_with(
    registry: &Registry,
    target: ExportTarget,
    world: &CompiledWorld,
    opts: &ExportOpts,
) -> Result<ExportOutcome, ExportError> {
    if opts.source_commit.trim().is_empty() {
        return Err(ExportError::MissingSourceAnchor);
    }
    if opts.region.is_some() {
        return Err(ExportError::UnsupportedOption {
            target,
            option: "region",
        });
    }
    if opts.variant.is_some() {
        return Err(ExportError::UnsupportedOption {
            target,
            option: "variant",
        });
    }
    let spec = registry.resolve_for(target, opts.profile_name.as_deref())?;

    let (layer, valuation) = layer_valuation(world, opts);
    let certain = certain_atoms_under(world, &valuation);
    let visible = apply_keep_stamps(certain, &opts.keep_stamps);
    let selected = select_atoms(spec, visible.clone());
    if let Some(predicate) = spec.renders.keys().find(|predicate| {
        !selected
            .iter()
            .any(|atom| atom.atom.pred.text.as_str() == predicate.as_str())
    }) {
        return Err(ExportError::InvalidProfile {
            profile: spec.qualified(),
            detail: format!(
                "render rule `{predicate}` has no selected certain-layer atom to render"
            ),
        });
    }

    let artifact = match target {
        ExportTarget::ContextPack => {
            if !spec.renders.is_empty() {
                return Err(ExportError::InvalidProfile {
                    profile: spec.qualified(),
                    detail: "context-pack profiles select and carry provenance but do not define row render rules"
                        .to_owned(),
                });
            }
            ExportArtifact::ContextPack(context_pack::build(spec, &layer, &selected))
        }
        ExportTarget::Articy => ExportArtifact::Articy(articy::compile(spec, &layer, &visible)?),
        ExportTarget::Csv => ExportArtifact::Csv(csv::compile(spec, &layer, &selected)?),
        ExportTarget::StaticWiki => {
            ExportArtifact::StaticWiki(static_wiki::compile(spec, &layer, &selected)?)
        }
        // The card/Ludii render is world content, not engine code: it lives in a
        // world crate that builds its ruleset directly and needs no compiled
        // world. Fail closed here.
        ExportTarget::Cards | ExportTarget::LudiiGdl => {
            return Err(ExportError::NoEngineRendererForTarget(target));
        }
    };

    let manifest = ExportManifest {
        commit: opts.source_commit.clone(),
        layer,
        valuation: digest_of(&valuation),
        profile_hash: digest_of(&(spec, &opts.keep_stamps)),
    };
    Ok(ExportOutcome { artifact, manifest })
}

fn select_atoms<'a>(spec: &ProfileSpec, atoms: Vec<&'a LoadedAtom>) -> Vec<&'a LoadedAtom> {
    if spec.selects.sorts.is_empty() && spec.selects.relations.is_empty() {
        return atoms;
    }
    atoms
        .into_iter()
        .filter(|atom| {
            let predicate = &atom.atom.pred.text;
            spec.selects.sorts.contains(predicate) || spec.selects.relations.contains(predicate)
        })
        .collect()
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
/// let pack = Pack { name: "camelot".to_owned(), documents: example_docs() };
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

/// Fingerprint an order-independent set of source byte strings.
///
/// Each member is hashed first, then the sorted member digests are framed by a
/// domain tag and count. This preserves duplicate members without relying on a
/// delimiter that could also occur in source bytes.
#[must_use]
pub fn source_fingerprint<'a>(contents: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut content_digests = contents.into_iter().map(Sha256::digest).collect::<Vec<_>>();
    content_digests.sort();

    let mut digest = Sha256::new();
    digest.update(b"dli-source-set-v1\0");
    digest.update((content_digests.len() as u64).to_be_bytes());
    for content_digest in content_digests {
        digest.update(content_digest);
    }
    format!("content-sha256:{:x}", digest.finalize())
}
