//! Versioned caller-project loading for the standalone worker.
//!
//! A project manifest maps stable wire document ids to caller-owned files. All
//! paths are resolved beneath the manifest directory, sorted before loading, and
//! checked against symlink escape. The worker therefore starts from one explicit,
//! replayable project rather than discovering ambient files or falling back to a
//! bundled world.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use dli_ast::decl::Decl;
use dli_export::profile::Registry;
use dli_game::model::Ruleset;
use dli_syntax::parse_document;
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
    /// Optional rejected-lore token registry.
    pub killed: KilledTokens,
    /// Canonical source paths, retained to enforce one logical owner per file.
    source_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SourceIdentity {
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
    sources: Vec<PathBuf>,
    episodes: Option<PathBuf>,
    killed_tokens: Option<PathBuf>,
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
    killed: Vec<KilledRow>,
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
        let documents = load_documents(&root, manifest.documents)?;
        let games = load_games(&root, manifest.games, &documents)?;

        let evidence_dir = resolve_storage(&root, "paths.evidence", &manifest.paths.evidence)?;
        let cache_dir = resolve_storage(&root, "paths.cache", &manifest.paths.cache)?;
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
        let language_docs = documents
            .values()
            .flat_map(|document| document.pack.documents.iter().cloned())
            .collect::<Vec<_>>();
        let language = LanguageModel::from_documents(&language_docs).model;
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
            ("killed_tokens", entry.killed_tokens.as_deref()),
        ] {
            if let Some(path) = path {
                paths.push(resolve_existing(
                    root,
                    &format!("documents.{id}.{field}"),
                    path,
                )?);
            }
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
    let mut source_owners = BTreeMap::<SourceIdentity, (String, PathBuf)>::new();
    let mut fork_owners = BTreeMap::<String, String>::new();
    let mut dialect_owners = BTreeMap::<String, String>::new();
    for (id, entry) in entries {
        validate_id("document", &id)?;
        let document = load_document(root, &id, &entry)?;
        for path in &document.source_paths {
            let identity = source_identity(path, &format!("documents.{id}.sources"))?;
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
fn source_identity(path: &Path, field: &str) -> Result<SourceIdentity, ProjectError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path).map_err(|error| ProjectError::AssetPath {
        field: field.to_owned(),
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    Ok(SourceIdentity::Unix {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(not(unix))]
fn source_identity(path: &Path, field: &str) -> Result<SourceIdentity, ProjectError> {
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
            return Ok(SourceIdentity::Windows {
                volume_serial,
                file_index,
            });
        }
    }

    // Canonical paths still reject symlink aliases on targets where the
    // standard library exposes no stable file identity.
    Ok(SourceIdentity::Path(path.to_path_buf()))
}

fn load_document(
    root: &Path,
    id: &str,
    entry: &DocumentEntry,
) -> Result<ProjectDocument, ProjectError> {
    let (pack, source_paths) = load_pack(root, id, &entry.sources)?;
    compile(&pack).map_err(|error| ProjectError::Compile {
        document: id.to_owned(),
        message: error.to_string(),
    })?;
    let episodes = entry
        .episodes
        .as_deref()
        .map(|path| load_yaml(root, &format!("documents.{id}.episodes"), path))
        .transpose()?;
    let killed = entry
        .killed_tokens
        .as_deref()
        .map(|path| load_killed(root, &format!("documents.{id}.killed_tokens"), path))
        .transpose()?
        .unwrap_or_default();
    Ok(ProjectDocument {
        pack,
        episodes,
        killed,
        source_paths,
    })
}

fn load_pack(
    root: &Path,
    id: &str,
    sources: &[PathBuf],
) -> Result<(Pack, Vec<PathBuf>), ProjectError> {
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
    for path in &paths {
        let source = fs::read_to_string(path).map_err(|error| ProjectError::AssetPath {
            field: format!("documents.{id}.sources"),
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        let (document, diagnostics) = parse_document(&source);
        if let Some(diagnostic) = diagnostics.iter().find(|diag| diag.class.is_error()) {
            return Err(ProjectError::Syntax {
                document: id.to_owned(),
                path: path.display().to_string(),
                message: format!("{} ({})", diagnostic.message, diagnostic.class.code()),
            });
        }
        documents.push(document);
    }
    Ok((
        Pack {
            name: id.to_owned(),
            documents,
        },
        paths,
    ))
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
                "a Door-T fork must have one document owner",
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
        let profiles = entry
            .profiles
            .as_deref()
            .map(|path| load_profiles(root, &format!("games.{id}.profiles"), path))
            .transpose()?
            .map_or_else(Registry::builtin, |loaded| {
                Registry::builtin().merge(loaded)
            });
        games.insert(id, ProjectGame { ruleset, profiles });
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

fn resolve_storage(root: &Path, field: &str, path: &Path) -> Result<PathBuf, ProjectError> {
    validate_relative(field, path)?;
    let joined = root.join(path);
    let mut ancestor = joined.as_path();
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| ProjectError::AssetPath {
            field: field.to_owned(),
            path: path.display().to_string(),
            message: "the path has no existing ancestor".into(),
        })?;
    }
    let canonical = fs::canonicalize(ancestor).map_err(|error| ProjectError::AssetPath {
        field: field.to_owned(),
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    if !canonical.starts_with(root) {
        return Err(ProjectError::AssetPath {
            field: field.to_owned(),
            path: path.display().to_string(),
            message: "the path traverses a symlink outside the project directory".into(),
        });
    }
    if !ancestor.is_dir() {
        return Err(ProjectError::AssetPath {
            field: field.to_owned(),
            path: path.display().to_string(),
            message: "the nearest existing ancestor is not a directory".into(),
        });
    }
    let suffix = joined
        .strip_prefix(ancestor)
        .map_err(|_| ProjectError::AssetPath {
            field: field.to_owned(),
            path: path.display().to_string(),
            message: "the storage path could not be normalized".into(),
        })?;
    Ok(canonical.join(suffix))
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

fn load_profiles(root: &Path, field: &str, path: &Path) -> Result<Registry, ProjectError> {
    let resolved = resolve_existing(root, field, path)?;
    Registry::from_path(&resolved).map_err(|error| ProjectError::ParseAsset {
        path: resolved.display().to_string(),
        message: error.to_string(),
    })
}

fn load_killed(root: &Path, field: &str, path: &Path) -> Result<KilledTokens, ProjectError> {
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
    Ok(KilledTokens::new(tokens))
}
