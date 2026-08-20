//! Versioned caller-project loading for the standalone worker.
//!
//! A project manifest maps stable wire document ids to caller-owned files. All
//! paths are resolved beneath the manifest directory, sorted before loading, and
//! checked against symlink escape. The worker therefore starts from one explicit,
//! replayable project rather than discovering ambient files or falling back to a
//! bundled world.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};

use dli_ast::decl::{Decl, Document};
use dli_core::valuation::{NamedAtomSet, NamedValuation, Valuation};
use dli_export::profile::Registry;
use dli_export::source_fingerprint;
use dli_game::model::Ruleset;
use dli_world::attest::Question;
use dli_world::craft::{
    AppliedCraft, CONFIG_FILES, CraftLedger, EpisodePolicy, SeasonOutline, Thresholds, apply,
    build_episode_policy, doctrines, load_config, validate,
};
use dli_world::episode::Episode;
use dli_world::language::LanguageModel;
use dli_world::{KilledToken, KilledTokens, MatchMode, Pack, compile};
use serde::Deserialize;
use thiserror::Error;

/// The only project-manifest version this build accepts.
pub const PROJECT_VERSION: u32 = 1;

/// A fully loaded caller project.
#[derive(Debug, Clone)]
pub struct Project {
    name: String,
    root: PathBuf,
    documents: BTreeMap<String, ProjectDocument>,
    games: BTreeMap<String, ProjectGame>,
    language: LanguageModel,
    evidence_dir: PathBuf,
    cache_dir: PathBuf,
}

/// One logical document and its optional companion data.
#[derive(Debug, Clone)]
pub struct ProjectDocument {
    /// Parsed source documents, compiled together under the logical id.
    pub pack: Pack,
    /// Optional production/story-time episode data.
    pub episodes: Option<Vec<Episode>>,
    /// Optional reviewed outline set used by payload conformance.
    pub outlines: Option<SeasonOutline>,
    /// Optional committed season craft state merged into payload checks.
    pub craft_ledger: Option<CraftLedger>,
    /// Caller-owned craft principles composed from the project's selected
    /// doctrine packs and overlays.
    pub craft_principles: Option<AppliedCraft>,
    /// Caller-owned season thresholds from the same craft configuration.
    pub craft_thresholds: Option<Thresholds>,
    /// Caller-owned outline and per-finding policy.
    pub craft_policy: Option<EpisodePolicy<'static>>,
    /// Optional rejected-lore token registry.
    pub killed: KilledTokens,
    /// Bounded model-judgment questions demanded by this document's checks.
    pub attestation_questions: Vec<Question>,
    /// Built-in neutral profiles plus project-owned lore export profiles.
    pub profiles: Registry,
    /// Content fingerprint of the caller-owned source files. For a
    /// store-backed document this is the resolved store head — the store's
    /// content-hash regime, never the JCS workspace-digest regime (DL-0055).
    pub source_digest: String,
    /// Canonical source paths, retained to enforce one logical owner per file.
    source_paths: Vec<PathBuf>,
    /// The lorestore this document was loaded from, when it is store-backed.
    ///
    /// The serve layer attaches **this** directory rather than an import copy,
    /// so `author.materialize` advances the very store the project reads from
    /// and the durable authoring loop closes (DL-0055).
    pub store_origin: Option<PathBuf>,
    /// The commit selected while loading a store-backed document.
    ///
    /// Branch mode records the commit from the one `Ref` read that also supplied
    /// [`store_valuation`](Self::store_valuation); pinned mode records the
    /// manifest's exact commit. The serve layer must use this snapshot rather
    /// than re-read a moving store head.
    pub store_commit: Option<dli_lorestore::ContentHash>,
    /// The selected store ref's complete valuation overlay.
    ///
    /// This is empty for a manifest-pinned commit: a pin is a replay of that
    /// commit, not an instruction to borrow the valuation of whatever ref happens
    /// to point elsewhere when the project starts.
    pub store_valuation: NamedValuation,
    /// Store decisions explicitly left unassigned by grounded attack cycles,
    /// carried by exact name across independent compilation interners.
    pub store_unassigned: NamedAtomSet,
    /// Digest of the selected branch ref, including its valuation overlay.
    ///
    /// Only branch mode has a selected ref. The authoring write path uses this
    /// whole-ref fence so it cannot silently parent staged work on an external
    /// CAS that arrived after project load.
    pub store_ref_digest: Option<String>,
    /// Whether the manifest pinned the store to an explicit `head`.
    ///
    /// A pinned document is a replay: it must not follow the branch as writes
    /// land, so nothing here may advance it implicitly.
    pub store_pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum FileIdentity {
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
    #[cfg(windows)]
    Windows { volume_serial: u32, file_index: u64 },
    #[cfg(not(unix))]
    Path(PathBuf),
}

/// One project game and its export profile registry.
#[derive(Debug, Clone)]
pub struct ProjectGame {
    /// Rules-as-data for the game.
    pub ruleset: Ruleset,
    /// Built-in neutral profiles plus project-owned profiles.
    pub profiles: Registry,
    /// Content fingerprint of the caller-owned rules file.
    pub source_digest: String,
}

/// A project manifest or one of its referenced assets is invalid.
#[derive(Debug, Error)]
pub enum ProjectError {
    /// The manifest path cannot be read or resolved.
    #[error("cannot resolve project manifest {path}: {source}")]
    ManifestPath {
        /// User-supplied path.
        path: String,
        /// Filesystem failure.
        source: std::io::Error,
    },
    /// The manifest text cannot be read.
    #[error("cannot read project manifest {path}: {source}")]
    ReadManifest {
        /// Resolved manifest path.
        path: String,
        /// Filesystem failure.
        source: std::io::Error,
    },
    /// TOML decoding failed.
    #[error("project manifest {path} is malformed: {source}")]
    ParseManifest {
        /// Resolved manifest path.
        path: String,
        /// TOML parse failure.
        source: toml::de::Error,
    },
    /// The manifest uses an unsupported version.
    #[error("project manifest version {found} is unsupported; this build accepts version 1")]
    Version {
        /// Version read from the manifest.
        found: u32,
    },
    /// A manifest-level semantic invariant failed.
    #[error("project manifest is invalid: {0}")]
    Invalid(String),
    /// A referenced path is unsafe or unavailable.
    #[error("project asset `{field}` at {path} is invalid: {message}")]
    AssetPath {
        /// Manifest field naming the asset.
        field: String,
        /// Supplied relative path.
        path: String,
        /// Domain-spoken reason.
        message: String,
    },
    /// A `.dli` source has an error-class syntax diagnostic.
    #[error("project document `{document}` source {path} is invalid: {message}")]
    Syntax {
        /// Logical document id.
        document: String,
        /// Source path.
        path: String,
        /// First error diagnostic.
        message: String,
    },
    /// A logical document does not compile.
    #[error("project document `{document}` does not compile: {message}")]
    Compile {
        /// Logical document id.
        document: String,
        /// Compiler failure.
        message: String,
    },
    /// A companion data file is malformed.
    #[error("project asset {path} is malformed: {message}")]
    ParseAsset {
        /// Resolved path.
        path: String,
        /// Parse failure.
        message: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    name: String,
    documents: BTreeMap<String, DocumentEntry>,
    #[serde(default)]
    games: BTreeMap<String, GameEntry>,
    #[serde(default)]
    paths: StoragePaths,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentEntry {
    /// `.dli` source files. Mutually exclusive with [`store`](Self::store):
    /// a document has exactly one source form (DL-0055).
    #[serde(default)]
    sources: Vec<PathBuf>,
    /// A project-root-relative lorestore directory to load this document from.
    /// The syntax-free source form — nothing on this path parses.
    store: Option<PathBuf>,
    /// A lorestore commit hash to pin the store at.
    ///
    /// Present: **pinned mode** — the document is exactly that commit, so the
    /// project replays, and advancing canon is an explicit manifest edit.
    /// Absent: **branch mode** — the document follows the store's `main` tip,
    /// which is what an authoring server wants. Only meaningful with `store`.
    head: Option<String>,
    episodes: Option<PathBuf>,
    outlines: Option<PathBuf>,
    craft_ledger: Option<PathBuf>,
    craft_config: Option<PathBuf>,
    killed_tokens: Option<PathBuf>,
    attestation_questions: Option<PathBuf>,
    profiles: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GameEntry {
    rules: PathBuf,
    profiles: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct StoragePaths {
    evidence: PathBuf,
    cache: PathBuf,
}

impl Default for StoragePaths {
    fn default() -> Self {
        Self {
            evidence: PathBuf::from(".dli-evidence"),
            cache: PathBuf::from(".dli-cache"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KilledFile {
    #[serde(default)]
    killed: Vec<KilledRow>,
    #[serde(default)]
    rejected_claims: Vec<RejectedClaimRow>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KilledRow {
    token: String,
    #[serde(rename = "match", default)]
    mode: MatchMode,
    decision: String,
    #[serde(default)]
    scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RejectedClaimRow {
    predicate: String,
    subject: String,
    value: String,
    #[serde(rename = "match", default)]
    mode: MatchMode,
    decision: String,
    #[serde(default)]
    scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttestationQuestionsFile {
    questions: Vec<Question>,
}

impl Project {
    /// Load a project from a `deloreis.toml` path or a directory containing it.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let supplied = path.as_ref();
        let manifest_candidate = if supplied.is_dir() {
            supplied.join("deloreis.toml")
        } else {
            supplied.to_path_buf()
        };
        let manifest_path =
            fs::canonicalize(&manifest_candidate).map_err(|source| ProjectError::ManifestPath {
                path: manifest_candidate.display().to_string(),
                source,
            })?;
        let root = manifest_path
            .parent()
            .ok_or_else(|| ProjectError::Invalid("the manifest has no parent directory".into()))?
            .to_path_buf();
        let text =
            fs::read_to_string(&manifest_path).map_err(|source| ProjectError::ReadManifest {
                path: manifest_path.display().to_string(),
                source,
            })?;
        let manifest: Manifest =
            toml::from_str(&text).map_err(|source| ProjectError::ParseManifest {
                path: manifest_path.display().to_string(),
                source,
            })?;
        if manifest.version != PROJECT_VERSION {
            return Err(ProjectError::Version {
                found: manifest.version,
            });
        }
        if manifest.name.trim().is_empty() {
            return Err(ProjectError::Invalid("`name` must not be empty".into()));
        }
        if manifest.documents.is_empty() {
            return Err(ProjectError::Invalid(
                "`documents` must define at least one logical document".into(),
            ));
        }

        let input_paths =
            collect_input_paths(&root, &manifest.documents, &manifest.games, &manifest_path)?;
        let evidence_dir =
            validate_storage_path(&root, "paths.evidence", &manifest.paths.evidence)?;
        let cache_dir = validate_storage_path(&root, "paths.cache", &manifest.paths.cache)?;
        if evidence_dir == root || cache_dir == root {
            return Err(ProjectError::Invalid(
                "evidence and cache paths must be dedicated project subdirectories".into(),
            ));
        }
        if evidence_dir == cache_dir
            || evidence_dir.starts_with(&cache_dir)
            || cache_dir.starts_with(&evidence_dir)
        {
            return Err(ProjectError::Invalid(
                "evidence and cache paths must be distinct, non-overlapping directories".into(),
            ));
        }
        if let Some(input) = input_paths
            .iter()
            .find(|input| input.starts_with(&evidence_dir) || input.starts_with(&cache_dir))
        {
            return Err(ProjectError::Invalid(format!(
                "evidence and cache paths must not contain project input `{}`; machine-written storage must be dedicated",
                input.display()
            )));
        }
        let input_identities = input_paths
            .iter()
            .map(|path| {
                file_identity(path, "manifest input").map(|identity| (identity, path.clone()))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        validate_storage_tree("paths.evidence", &evidence_dir, &input_identities)?;
        validate_storage_tree("paths.cache", &cache_dir, &input_identities)?;

        let documents = load_documents(&root, manifest.documents)?;
        let games = load_games(&root, manifest.games, &documents)?;
        let language_docs = documents
            .values()
            .flat_map(|document| document.pack.documents.iter().cloned())
            .collect::<Vec<_>>();
        let language = LanguageModel::from_documents(&language_docs).model;
        materialize_storage(&root, "paths.evidence", &evidence_dir)?;
        materialize_storage(&root, "paths.cache", &cache_dir)?;
        Ok(Self {
            name: manifest.name,
            root,
            documents,
            games,
            language,
            evidence_dir,
            cache_dir,
        })
    }

    /// Caller-facing project name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Canonical directory containing `deloreis.toml`.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a logical document id.
    #[must_use]
    pub fn document(&self, id: &str) -> Option<&ProjectDocument> {
        self.documents.get(id)
    }

    /// Resolve a game id.
    #[must_use]
    pub fn game(&self, id: &str) -> Option<&ProjectGame> {
        self.games.get(id)
    }

    /// Project-wide language model used by the current name-check wire shape.
    #[must_use]
    pub fn language(&self) -> &LanguageModel {
        &self.language
    }

    /// Documents in stable logical-id order.
    pub fn documents(&self) -> impl Iterator<Item = (&str, &ProjectDocument)> {
        self.documents
            .iter()
            .map(|(id, document)| (id.as_str(), document))
    }

    /// Validated evidence directory, reserved for durable stores.
    #[must_use]
    pub fn evidence_dir(&self) -> &Path {
        &self.evidence_dir
    }

    /// Validated derived-cache directory.
    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

fn collect_input_paths(
    root: &Path,
    documents: &BTreeMap<String, DocumentEntry>,
    games: &BTreeMap<String, GameEntry>,
    manifest_path: &Path,
) -> Result<Vec<PathBuf>, ProjectError> {
    let mut paths = vec![manifest_path.to_path_buf()];
    for (id, entry) in documents {
        for source in &entry.sources {
            paths.push(resolve_existing(
                root,
                &format!("documents.{id}.sources"),
                source,
            )?);
        }
        for (field, path) in [
            ("episodes", entry.episodes.as_deref()),
            ("outlines", entry.outlines.as_deref()),
            ("craft_ledger", entry.craft_ledger.as_deref()),
            ("killed_tokens", entry.killed_tokens.as_deref()),
            (
                "attestation_questions",
                entry.attestation_questions.as_deref(),
            ),
            ("profiles", entry.profiles.as_deref()),
        ] {
            if let Some(path) = path {
                paths.push(resolve_existing(
                    root,
                    &format!("documents.{id}.{field}"),
                    path,
                )?);
            }
        }
        if let Some(path) = &entry.craft_config {
            paths.extend(craft_config_paths(
                root,
                &format!("documents.{id}.craft_config"),
                path,
            )?);
        }
    }
    for (id, entry) in games {
        paths.push(resolve_existing(
            root,
            &format!("games.{id}.rules"),
            &entry.rules,
        )?);
        if let Some(profiles) = &entry.profiles {
            paths.push(resolve_existing(
                root,
                &format!("games.{id}.profiles"),
                profiles,
            )?);
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn load_documents(
    root: &Path,
    entries: BTreeMap<String, DocumentEntry>,
) -> Result<BTreeMap<String, ProjectDocument>, ProjectError> {
    let mut loaded = BTreeMap::new();
    let mut source_owners = BTreeMap::<FileIdentity, (String, PathBuf)>::new();
    let mut fork_owners = BTreeMap::<String, String>::new();
    let mut dialect_owners = BTreeMap::<String, String>::new();
    for (id, entry) in entries {
        validate_id("document", &id)?;
        let document = load_document(root, &id, &entry)?;
        for path in &document.source_paths {
            let identity = file_identity(path, &format!("documents.{id}.sources"))?;
            if let Some((first, first_path)) =
                source_owners.insert(identity, (id.clone(), path.clone()))
            {
                return Err(ProjectError::Invalid(format!(
                    "source `{}` aliases `{}` and is owned by both `{first}` and `{id}`; each source file must belong to one logical document",
                    path.display(),
                    first_path.display()
                )));
            }
        }
        reject_duplicate_decls(&id, &document.pack, &mut fork_owners, &mut dialect_owners)?;
        loaded.insert(id, document);
    }
    Ok(loaded)
}

#[cfg(unix)]
fn file_identity(path: &Path, field: &str) -> Result<FileIdentity, ProjectError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path).map_err(|error| ProjectError::AssetPath {
        field: field.to_owned(),
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    Ok(FileIdentity::Unix {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(not(unix))]
fn file_identity(path: &Path, field: &str) -> Result<FileIdentity, ProjectError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        let metadata = fs::metadata(path).map_err(|error| ProjectError::AssetPath {
            field: field.to_owned(),
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        if let (Some(volume_serial), Some(file_index)) =
            (metadata.volume_serial_number(), metadata.file_index())
        {
            return Ok(FileIdentity::Windows {
                volume_serial,
                file_index,
            });
        }
    }

    // Canonical paths still reject symlink aliases on targets where the
    // standard library exposes no stable file identity.
    Ok(FileIdentity::Path(path.to_path_buf()))
}

fn validate_store_standing(
    id: &str,
    pack: &Pack,
    valuation: &NamedValuation,
    unassigned: &NamedAtomSet,
) -> Result<(), ProjectError> {
    let mut compiled = compile(pack).map_err(|error| ProjectError::Compile {
        document: id.to_owned(),
        message: error.to_string(),
    })?;
    let declared_decisions: BTreeSet<&str> = pack
        .documents
        .iter()
        .flat_map(|document| &document.decls)
        .filter_map(|decl| match decl {
            Decl::Ratify(ratify) => Some(ratify.decision.text.as_str()),
            _ => None,
        })
        .collect();
    for name in valuation
        .0
        .keys()
        .chain(unassigned.0.iter())
        .filter(|name| declared_decisions.contains(name.as_str()))
    {
        compiled.model.world.interner.intern(name);
    }
    valuation
        .to_indexed(compiled.model.interner())
        .map_err(|error| ProjectError::Compile {
            document: id.to_owned(),
            message: format!("store ref valuation does not map to its compiled document: {error}"),
        })?;
    unassigned
        .to_indexed(compiled.model.interner())
        .map_err(|error| ProjectError::Compile {
            document: id.to_owned(),
            message: format!("store open decisions do not map to the compiled document: {error}"),
        })?;
    Ok(())
}

fn load_document(
    root: &Path,
    id: &str,
    entry: &DocumentEntry,
) -> Result<ProjectDocument, ProjectError> {
    let (
        pack,
        source_paths,
        source_digest,
        store_origin,
        store_commit,
        store_valuation,
        store_unassigned,
        store_ref_digest,
    ) = load_source(root, id, entry)?;
    validate_store_standing(id, &pack, &store_valuation, &store_unassigned)?;
    let episodes = entry
        .episodes
        .as_deref()
        .map(|path| load_yaml(root, &format!("documents.{id}.episodes"), path))
        .transpose()?;
    let outlines = entry
        .outlines
        .as_deref()
        .map(|path| load_yaml(root, &format!("documents.{id}.outlines"), path))
        .transpose()?;
    let craft_ledger = entry
        .craft_ledger
        .as_deref()
        .map(|path| load_yaml(root, &format!("documents.{id}.craft_ledger"), path))
        .transpose()?;
    let (craft_principles, craft_thresholds, craft_policy) = entry
        .craft_config
        .as_deref()
        .map(|path| load_craft_config(root, &format!("documents.{id}.craft_config"), path))
        .transpose()?
        .map_or((None, None, None), |(principles, thresholds, policy)| {
            (Some(principles), thresholds, Some(policy))
        });
    let killed = entry
        .killed_tokens
        .as_deref()
        .map(|path| load_killed(root, &format!("documents.{id}.killed_tokens"), path))
        .transpose()?
        .unwrap_or_default();
    let attestation_questions = entry
        .attestation_questions
        .as_deref()
        .map(|path| {
            load_attestation_questions(root, &format!("documents.{id}.attestation_questions"), path)
        })
        .transpose()?
        .unwrap_or_default();
    let profiles = entry
        .profiles
        .as_deref()
        .map(|path| load_profiles(root, &format!("documents.{id}.profiles"), path))
        .transpose()?
        .map_or_else(Registry::builtin, |loaded| {
            Registry::builtin().merge(loaded)
        });
    Ok(ProjectDocument {
        pack,
        episodes,
        outlines,
        craft_ledger,
        craft_principles,
        craft_thresholds,
        craft_policy,
        killed,
        attestation_questions,
        profiles,
        source_digest,
        source_paths,
        store_origin: store_origin.clone(),
        store_commit,
        store_valuation,
        store_unassigned,
        store_ref_digest,
        store_pinned: store_origin.is_some() && entry.head.is_some(),
    })
}

fn craft_config_paths(root: &Path, field: &str, path: &Path) -> Result<Vec<PathBuf>, ProjectError> {
    let directory = resolve_existing(root, field, path)?;
    if !directory.is_dir() {
        return Err(ProjectError::AssetPath {
            field: field.to_owned(),
            path: path.display().to_string(),
            message: "craft_config must name a directory".into(),
        });
    }
    let mut files = vec![directory];
    for name in CONFIG_FILES {
        let relative = path.join(name);
        if root
            .join(&relative)
            .try_exists()
            .map_err(|error| ProjectError::AssetPath {
                field: field.to_owned(),
                path: relative.display().to_string(),
                message: error.to_string(),
            })?
        {
            files.push(resolve_existing(root, field, &relative)?);
        }
    }
    Ok(files)
}

fn load_craft_config(
    root: &Path,
    field: &str,
    path: &Path,
) -> Result<(AppliedCraft, Option<Thresholds>, EpisodePolicy<'static>), ProjectError> {
    let files = craft_config_paths(root, field, path)?;
    let directory = resolve_existing(root, field, path)?;
    let packs = doctrines();
    let config = load_config(&directory).map_err(|error| ProjectError::ParseAsset {
        path: directory.display().to_string(),
        message: error.to_string(),
    })?;
    let faults = validate(&config, &packs);
    if !faults.is_empty() {
        return Err(ProjectError::Invalid(format!(
            "{field} is invalid: {}",
            faults
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }
    debug_assert!(
        files.iter().all(|file| file.starts_with(&directory)),
        "validated craft files stay under their config directory"
    );
    let thresholds = config.season_thresholds;
    let policy = build_episode_policy(&config);
    Ok((apply(&config, &packs), thresholds, policy))
}

/// Load a document from whichever source form its manifest entry declares.
///
/// A document has exactly one source form: `.dli` `sources`, or a lorestore
/// `store` (DL-0055). Both is ambiguous — two answers to "what is this
/// document" with no rule for which wins — and neither leaves nothing to
/// compile, so both are refused by name rather than by a later confusing
/// failure.
#[allow(clippy::type_complexity)]
fn load_source(
    root: &Path,
    id: &str,
    entry: &DocumentEntry,
) -> Result<
    (
        Pack,
        Vec<PathBuf>,
        String,
        Option<PathBuf>,
        Option<dli_lorestore::ContentHash>,
        NamedValuation,
        NamedAtomSet,
        Option<String>,
    ),
    ProjectError,
> {
    match (entry.sources.is_empty(), entry.store.as_deref()) {
        (false, Some(_)) => Err(ProjectError::Invalid(format!(
            "document `{id}` declares both `sources` and `store`; a document has one source form \
             — remove whichever is not the document's origin"
        ))),
        (true, None) => Err(ProjectError::Invalid(format!(
            "document `{id}` declares neither `sources` nor `store`; it has nothing to compile"
        ))),
        (true, Some(store)) => {
            let (pack, paths, digest, commit, valuation, unassigned, ref_digest) =
                load_pack_from_store(root, id, store, entry.head.as_deref())?;
            let origin = paths.first().cloned();
            Ok((
                pack,
                paths,
                digest,
                origin,
                Some(commit),
                valuation,
                unassigned,
                ref_digest,
            ))
        }
        (false, None) => {
            if entry.head.is_some() {
                return Err(ProjectError::Invalid(format!(
                    "document `{id}` sets `head` without `store`; a commit pin only means \
                     something for a store-backed document"
                )));
            }
            let (pack, paths, digest) = load_pack(root, id, &entry.sources)?;
            Ok((
                pack,
                paths,
                digest,
                None,
                None,
                NamedValuation::default(),
                NamedAtomSet::default(),
                None,
            ))
        }
    }
}

/// Load a document from a lorestore directory — the syntax-free source form.
///
/// Nothing here parses: the store's fact objects are read and reconstructed
/// into the same [`Pack`] the `.dli` arm produces, and then handed to the
/// existing compile path, so a store-backed document and a `.dli`-backed
/// document reach identical machinery from the first compile onward.
///
/// The fingerprint is the resolved head's **content hash** — the store's own
/// addressing regime. It is deliberately not the JCS `workspace_digest`
/// regime: the two never share a field or a meaning (DL-0046 audit repair 1).
#[allow(clippy::type_complexity)]
fn load_pack_from_store(
    root: &Path,
    id: &str,
    store: &Path,
    head: Option<&str>,
) -> Result<
    (
        Pack,
        Vec<PathBuf>,
        String,
        dli_lorestore::ContentHash,
        NamedValuation,
        NamedAtomSet,
        Option<String>,
    ),
    ProjectError,
> {
    let field = format!("documents.{id}.store");
    let dir = resolve_existing(root, &field, store)?;
    let lorestore =
        dli_lorestore::Lorestore::open(dir.clone()).map_err(|error| ProjectError::AssetPath {
            field: field.clone(),
            path: store.display().to_string(),
            message: error.to_string(),
        })?;

    // Branch mode reads the complete ref once. Its commit and valuation are one
    // inseparable project snapshot: reading the commit first and valuation later
    // can mix two external CAS states.
    let (resolved, valuation, ref_digest) = match head {
        // Pinned: the manifest names the commit, and that is the document.
        Some(pin) => (
            dli_lorestore::ContentHash::parse(pin).ok_or_else(|| {
                ProjectError::Invalid(format!(
                    "document `{id}` pins `head` to `{pin}`, which is not a `sha256:<hex>` store \
                     commit hash"
                ))
            })?,
            Valuation::new(),
            None,
        ),
        // Branch mode: follow the store's main tip.
        None => lorestore
            .branch_ref(dli_lorestore::MAIN_REF)
            .map_err(|error| ProjectError::AssetPath {
                field: field.clone(),
                path: store.display().to_string(),
                message: error.to_string(),
            })?
            .ok_or_else(|| {
                ProjectError::Invalid(format!(
                    "document `{id}` follows the store's `{}` branch, which has no commit yet; \
                     import a corpus into the store, or pin `head` to an existing commit",
                    dli_lorestore::MAIN_REF
                ))
            })
            .and_then(|reference| {
                reference
                    .digest()
                    .map(|digest| (reference.commit, reference.valuation, Some(digest.0)))
                    .map_err(|error| ProjectError::Compile {
                        document: id.to_owned(),
                        message: format!("store ref cannot be fingerprinted: {error}"),
                    })
            })?,
    };

    let effective = lorestore
        .effective_valuation_at(&resolved, &valuation)
        .map_err(|error| ProjectError::Compile {
            document: id.to_owned(),
            message: format!("store head `{resolved}` has invalid decision standing: {error}"),
        })?;
    let source_names = lorestore
        .interner_at(&resolved)
        .map_err(|error| ProjectError::Compile {
            document: id.to_owned(),
            message: format!("store head `{resolved}` has no valid interner: {error}"),
        })?;
    let valuation =
        NamedValuation::from_indexed(&effective.valuation, &source_names).map_err(|error| {
            ProjectError::Compile {
                document: id.to_owned(),
                message: format!("store ref valuation cannot be resolved: {error}"),
            }
        })?;
    let unassigned =
        NamedAtomSet::from_indexed(&effective.unassigned, &source_names).map_err(|error| {
            ProjectError::Compile {
                document: id.to_owned(),
                message: format!("store open decisions cannot be resolved: {error}"),
            }
        })?;

    let pack = lorestore
        .reconstruct_at(&resolved)
        .map_err(|error| ProjectError::Compile {
            document: id.to_owned(),
            message: format!("store head `{resolved}` does not reconstruct: {error}"),
        })?;

    Ok((
        Pack {
            name: id.to_owned(),
            documents: pack.documents,
        },
        vec![dir],
        resolved.as_str().to_owned(),
        resolved,
        valuation,
        unassigned,
        ref_digest,
    ))
}

fn load_pack(
    root: &Path,
    id: &str,
    sources: &[PathBuf],
) -> Result<(Pack, Vec<PathBuf>, String), ProjectError> {
    if sources.is_empty() {
        return Err(ProjectError::Invalid(format!(
            "document `{id}` has no sources"
        )));
    }
    let field = format!("documents.{id}.sources");
    let mut paths = sources
        .iter()
        .map(|path| resolve_existing(root, &field, path))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    if paths.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProjectError::Invalid(format!(
            "document `{id}` lists the same source more than once"
        )));
    }
    if let Some(path) = paths
        .iter()
        .find(|path| path.extension().and_then(|ext| ext.to_str()) != Some("dli"))
    {
        return Err(ProjectError::AssetPath {
            field,
            path: path.display().to_string(),
            message: "source files must use the `.dli` extension".into(),
        });
    }

    let mut documents = Vec::with_capacity(paths.len());
    let mut contents = Vec::with_capacity(paths.len());
    for path in &paths {
        let source = fs::read_to_string(path).map_err(|error| ProjectError::AssetPath {
            field: format!("documents.{id}.sources"),
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        let document = parse_source(id, path, &source)?;
        documents.push(document);
        contents.push(source.into_bytes());
    }
    let source_digest = source_fingerprint(contents.iter().map(Vec::as_slice));
    Ok((
        Pack {
            name: id.to_owned(),
            documents,
        },
        paths,
        source_digest,
    ))
}

/// Read one `.dli` project source into a typed [`Document`], refusing on the
/// first error-class diagnostic.
///
/// LS-7 gates the parse. A build without the `syntax` feature has no front end
/// to read `.dli` text with, so a `.dli`-sourced project refuses at load with a
/// message that names the two ways forward — a `syntax`-enabled build, or a
/// corpus served from committed fact objects. It never loads an empty pack and
/// pretends the project was fine.
#[cfg(feature = "syntax")]
fn parse_source(id: &str, path: &Path, source: &str) -> Result<Document, ProjectError> {
    let (document, diagnostics) = dli_syntax::parse_document(source);
    if let Some(diagnostic) = diagnostics.iter().find(|diag| diag.class.is_error()) {
        return Err(ProjectError::Syntax {
            document: id.to_owned(),
            path: path.display().to_string(),
            message: format!("{} ({})", diagnostic.message, diagnostic.class.code()),
        });
    }
    Ok(document)
}

#[cfg(not(feature = "syntax"))]
fn parse_source(id: &str, path: &Path, _source: &str) -> Result<Document, ProjectError> {
    Err(ProjectError::Syntax {
        document: id.to_owned(),
        path: path.display().to_string(),
        message: "this build links no `.dli` front end; load this project with a `syntax`-enabled \
                  build, or serve the corpus from committed fact objects"
            .to_owned(),
    })
}

fn reject_duplicate_decls(
    id: &str,
    pack: &Pack,
    fork_owners: &mut BTreeMap<String, String>,
    dialect_owners: &mut BTreeMap<String, String>,
) -> Result<(), ProjectError> {
    for decl in pack.documents.iter().flat_map(|document| &document.decls) {
        let (name, owners, kind, reason) = match decl {
            Decl::Fork(fork) => (
                &fork.question.text,
                &mut *fork_owners,
                "fork",
                "a tool-surface fork must have one document owner",
            ),
            Decl::Dialect(dialect) => (
                &dialect.name.text,
                &mut *dialect_owners,
                "dialect",
                "name checking resolves dialect ids project-wide",
            ),
            _ => continue,
        };
        if let Some(first) = owners.insert(name.clone(), id.to_owned()) {
            return Err(ProjectError::Invalid(format!(
                "{kind} `{name}` is declared by both `{first}` and `{id}`; {reason}"
            )));
        }
    }
    Ok(())
}

fn load_games(
    root: &Path,
    entries: BTreeMap<String, GameEntry>,
    documents: &BTreeMap<String, ProjectDocument>,
) -> Result<BTreeMap<String, ProjectGame>, ProjectError> {
    let mut games = BTreeMap::new();
    for (id, entry) in entries {
        validate_id("game", &id)?;
        if documents.contains_key(&id) {
            return Err(ProjectError::Invalid(format!(
                "`{id}` is both a document and a game; wire asset ids must be unambiguous"
            )));
        }
        let rules_path = resolve_existing(root, &format!("games.{id}.rules"), &entry.rules)?;
        let rules_text =
            fs::read_to_string(&rules_path).map_err(|error| ProjectError::AssetPath {
                field: format!("games.{id}.rules"),
                path: rules_path.display().to_string(),
                message: error.to_string(),
            })?;
        let ruleset =
            Ruleset::from_rules(&rules_text).map_err(|error| ProjectError::ParseAsset {
                path: rules_path.display().to_string(),
                message: error.to_string(),
            })?;
        let source_digest = source_fingerprint([rules_text.as_bytes()]);
        let loaded_profiles = entry
            .profiles
            .as_deref()
            .map(|path| {
                let field = format!("games.{id}.profiles");
                let resolved = resolve_existing(root, &field, path)?;
                let registry =
                    Registry::from_path(&resolved).map_err(|error| ProjectError::ParseAsset {
                        path: resolved.display().to_string(),
                        message: error.to_string(),
                    })?;
                Ok::<_, ProjectError>((registry, resolved))
            })
            .transpose()?;
        let profile_path = loaded_profiles
            .as_ref()
            .map_or_else(|| rules_path.clone(), |(_, path)| path.clone());
        let profiles = loaded_profiles.map_or_else(Registry::builtin, |(loaded, _)| {
            Registry::builtin().merge(loaded)
        });
        for profile in &profiles.profiles {
            if matches!(
                profile.target,
                dli_export::ExportTarget::Cards | dli_export::ExportTarget::LudiiGdl
            ) {
                dli_game::export::validate_game_profile(&ruleset, profile).map_err(|error| {
                    ProjectError::ParseAsset {
                        path: profile_path.display().to_string(),
                        message: error.to_string(),
                    }
                })?;
            }
        }
        games.insert(
            id,
            ProjectGame {
                ruleset,
                profiles,
                source_digest,
            },
        );
    }
    Ok(games)
}

fn validate_id(kind: &str, id: &str) -> Result<(), ProjectError> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ProjectError::Invalid(format!(
            "{kind} id `{id}` must use lowercase ASCII letters, digits, and hyphens"
        )));
    }
    Ok(())
}

fn validate_relative(field: &str, path: &Path) -> Result<(), ProjectError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(ProjectError::AssetPath {
            field: field.to_owned(),
            path: path.display().to_string(),
            message: "paths must be non-empty, normalized, relative, and contain no `..` or `.` components"
                .into(),
        });
    }
    Ok(())
}

fn resolve_existing(root: &Path, field: &str, path: &Path) -> Result<PathBuf, ProjectError> {
    validate_relative(field, path)?;
    let resolved = fs::canonicalize(root.join(path)).map_err(|error| ProjectError::AssetPath {
        field: field.to_owned(),
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    if !resolved.starts_with(root) {
        return Err(ProjectError::AssetPath {
            field: field.to_owned(),
            path: path.display().to_string(),
            message: "the resolved path escapes the project directory".into(),
        });
    }
    Ok(resolved)
}

fn validate_storage_path(root: &Path, field: &str, path: &Path) -> Result<PathBuf, ProjectError> {
    validate_relative(field, path)?;
    let joined = root.join(path);
    let mut current = root.to_path_buf();
    for component in path.components() {
        let Component::Normal(component) = component else {
            unreachable!("validate_relative accepts only normal components");
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ProjectError::AssetPath {
                    field: field.to_owned(),
                    path: path.display().to_string(),
                    message: format!(
                        "machine-written storage must not traverse symlinks; `{}` is a symlink",
                        current.display()
                    ),
                });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(ProjectError::AssetPath {
                    field: field.to_owned(),
                    path: path.display().to_string(),
                    message: format!(
                        "storage path component `{}` is not a directory",
                        current.display()
                    ),
                });
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(ProjectError::AssetPath {
                    field: field.to_owned(),
                    path: path.display().to_string(),
                    message: error.to_string(),
                });
            }
        }
    }
    Ok(joined)
}

fn materialize_storage(root: &Path, field: &str, path: &Path) -> Result<(), ProjectError> {
    let relative = path
        .strip_prefix(root)
        .expect("validated storage paths are beneath the project root");
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(ProjectError::AssetPath {
                    field: field.to_owned(),
                    path: path.display().to_string(),
                    message: format!(
                        "storage path component `{}` is not a regular directory",
                        current.display()
                    ),
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let parent = current
                    .parent()
                    .expect("a project storage directory has a parent");
                match fs::create_dir(&current) {
                    Ok(()) => sync_project_directory(field, path, parent)?,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        let metadata = fs::symlink_metadata(&current).map_err(|error| {
                            ProjectError::AssetPath {
                                field: field.to_owned(),
                                path: path.display().to_string(),
                                message: error.to_string(),
                            }
                        })?;
                        if !metadata.is_dir() || metadata.file_type().is_symlink() {
                            return Err(ProjectError::AssetPath {
                                field: field.to_owned(),
                                path: path.display().to_string(),
                                message: format!(
                                    "storage path component `{}` is not a regular directory",
                                    current.display()
                                ),
                            });
                        }
                    }
                    Err(error) => {
                        return Err(ProjectError::AssetPath {
                            field: field.to_owned(),
                            path: path.display().to_string(),
                            message: error.to_string(),
                        });
                    }
                }
            }
            Err(error) => {
                return Err(ProjectError::AssetPath {
                    field: field.to_owned(),
                    path: path.display().to_string(),
                    message: error.to_string(),
                });
            }
        }
    }
    let canonical = fs::canonicalize(path).map_err(|error| ProjectError::AssetPath {
        field: field.to_owned(),
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    if canonical != path {
        return Err(ProjectError::AssetPath {
            field: field.to_owned(),
            path: path.display().to_string(),
            message: "storage path changed while the project was loading".into(),
        });
    }
    Ok(())
}

fn sync_project_directory(field: &str, storage: &Path, path: &Path) -> Result<(), ProjectError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| ProjectError::AssetPath {
            field: field.to_owned(),
            path: storage.display().to_string(),
            message: format!("cannot sync storage parent `{}`: {error}", path.display()),
        })
}

fn validate_storage_tree(
    field: &str,
    root: &Path,
    inputs: &BTreeMap<FileIdentity, PathBuf>,
) -> Result<(), ProjectError> {
    if !root.exists() {
        return Ok(());
    }
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| ProjectError::AssetPath {
            field: field.to_owned(),
            path: directory.display().to_string(),
            message: error.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| ProjectError::AssetPath {
                field: field.to_owned(),
                path: directory.display().to_string(),
                message: error.to_string(),
            })?;
            let path = entry.path();
            let metadata =
                fs::symlink_metadata(&path).map_err(|error| ProjectError::AssetPath {
                    field: field.to_owned(),
                    path: path.display().to_string(),
                    message: error.to_string(),
                })?;
            if metadata.file_type().is_symlink() {
                return Err(ProjectError::Invalid(format!(
                    "machine-written storage must not contain symlinks; `{}` is a symlink",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                let identity = file_identity(&path, field)?;
                if let Some(input) = inputs.get(&identity) {
                    return Err(ProjectError::Invalid(format!(
                        "machine-written storage file `{}` aliases project input `{}`; storage must be dedicated",
                        path.display(),
                        input.display()
                    )));
                }
            } else {
                return Err(ProjectError::Invalid(format!(
                    "machine-written storage may contain only regular files and directories; `{}` is a special filesystem node",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn load_yaml<T: for<'de> Deserialize<'de>>(
    root: &Path,
    field: &str,
    path: &Path,
) -> Result<T, ProjectError> {
    let resolved = resolve_existing(root, field, path)?;
    let text = fs::read_to_string(&resolved).map_err(|error| ProjectError::AssetPath {
        field: field.to_owned(),
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    serde_yaml_ng::from_str(&text).map_err(|error| ProjectError::ParseAsset {
        path: resolved.display().to_string(),
        message: error.to_string(),
    })
}

fn load_json<T: for<'de> Deserialize<'de>>(
    root: &Path,
    field: &str,
    path: &Path,
) -> Result<T, ProjectError> {
    let resolved = resolve_existing(root, field, path)?;
    let text = fs::read_to_string(&resolved).map_err(|error| ProjectError::AssetPath {
        field: field.to_owned(),
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    serde_json::from_str(&text).map_err(|error| ProjectError::ParseAsset {
        path: resolved.display().to_string(),
        message: error.to_string(),
    })
}

fn load_profiles(root: &Path, field: &str, path: &Path) -> Result<Registry, ProjectError> {
    let resolved = resolve_existing(root, field, path)?;
    Registry::from_path(&resolved).map_err(|error| ProjectError::ParseAsset {
        path: resolved.display().to_string(),
        message: error.to_string(),
    })
}

fn load_killed(root: &Path, field: &str, path: &Path) -> Result<KilledTokens, ProjectError> {
    use dli_world::RejectedClaim;

    let file: KilledFile = load_yaml(root, field, path)?;
    let mut tokens = Vec::with_capacity(file.killed.len());
    for row in file.killed {
        if row.token.trim().is_empty() {
            return Err(ProjectError::Invalid(
                "killed-token entries must name a non-empty token".into(),
            ));
        }
        if row.decision.trim().is_empty() {
            return Err(ProjectError::Invalid(format!(
                "killed token `{}` must name the decision that killed it",
                row.token
            )));
        }
        tokens.push(KilledToken {
            token: row.token,
            mode: row.mode,
            decision: row.decision,
            scope: row.scope,
        });
    }
    let mut rejected_claims = Vec::with_capacity(file.rejected_claims.len());
    for row in file.rejected_claims {
        if row.predicate.trim().is_empty()
            || row.subject.trim().is_empty()
            || row.value.trim().is_empty()
        {
            return Err(ProjectError::Invalid(
                "rejected-claim entries must name a predicate, subject, and value".into(),
            ));
        }
        if row.decision.trim().is_empty() {
            return Err(ProjectError::Invalid(format!(
                "rejected claim `{}({}, {})` must name its decision",
                row.predicate, row.subject, row.value
            )));
        }
        rejected_claims.push(RejectedClaim {
            predicate: row.predicate,
            subject: row.subject,
            value: row.value,
            mode: row.mode,
            decision: row.decision,
            scope: row.scope,
        });
    }
    Ok(KilledTokens::new(tokens).with_rejected_claims(rejected_claims))
}

fn load_attestation_questions(
    root: &Path,
    field: &str,
    path: &Path,
) -> Result<Vec<Question>, ProjectError> {
    let file: AttestationQuestionsFile = load_json(root, field, path)?;
    let mut keys = BTreeSet::new();
    for question in &file.questions {
        if question.kind.trim().is_empty()
            || question.atom.trim().is_empty()
            || question.subject.trim().is_empty()
        {
            return Err(ProjectError::Invalid(
                "attestation questions require non-empty `kind`, `atom`, and `subject`".into(),
            ));
        }
        if !keys.insert(question.key()) {
            return Err(ProjectError::Invalid(format!(
                "attestation question `{}` is declared more than once",
                question.key()
            )));
        }
    }
    Ok(file.questions)
}
