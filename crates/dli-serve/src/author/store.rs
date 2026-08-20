//! The authoring seam's store (`AUTH-1`): each workspace bound to one loaded document base.
//!
//! A project store owns one compiled base per logical document. `author.open`
//! resolves a scope or declared fork to exactly one of those bases and records
//! that ownership for every subsequent read and mutation. The fallback base
//! exists only for focused unit tests and explicit demo fixtures.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use dli_ast::decl::{Decl, Document};
use dli_core::valuation::{NamedAtomSet, NamedValuation, ValuationRemapError};
use dli_eval::error::EvalError;
use dli_world::pack::Pack;
use dli_world::{CompiledWorld, compile};
use thiserror::Error;

use super::workspace::{Mode, Workspace};

/// The workspace store: loaded document bases plus every open workspace.
///
/// Each base keeps both its **compiled** world (what mutation-time findings check
/// against) and its **source documents**, so a read tool can
/// recompile the staged world — the base documents plus the workspace's staged
/// declarations — through the one real [`compile`] loader (spec 11: the reads
/// evaluate `base + staged`). A store built from only a compiled base
/// ([`with_base`](Store::with_base)) carries no source, so its reads see the
/// staged declarations alone; [`with_pack`](Store::with_pack) is the explicit
/// demo constructor and [`with_packs`](Store::with_packs) is the strict project
/// constructor.
#[derive(Debug)]
pub struct Store {
    fallback: Option<Base>,
    bases: BTreeMap<String, Base>,
    fork_bases: BTreeMap<String, String>,
    strict_forks: bool,
    workspaces: BTreeMap<String, Workspace>,
    workspace_bases: BTreeMap<String, String>,
    /// Per-workspace compiled base for a branch-mode workspace (WS-3), keyed
    /// by workspace id rather than branch name: two workspaces opened against
    /// the same branch compile it independently (each is a point-in-time read
    /// of the branch head at its own open), so the base lives with the
    /// workspace that read it, not with the branch. Populated only by a
    /// `branch`-carrying `open`; consulted first by
    /// [`workspace_base`](Store::workspace_base),
    /// [`workspace_base_docs`](Store::workspace_base_docs), and
    /// [`parts_mut`](Store::parts_mut); cleared by [`discard`](Store::discard).
    branch_bases: BTreeMap<String, Base>,
    discard_replays: BTreeMap<String, (String, String, serde_json::Value)>,
    discard_replay_order: VecDeque<String>,
    next_id: u32,
    /// The durable lorestore backends, one per base — empty in test/demo
    /// contexts without a project directory. Each base pack has its own
    /// single-branch store (one store carries one pack lineage), attached and
    /// idempotently seeded by [`attach_lorestore`](Store::attach_lorestore).
    lorestores: BTreeMap<String, dli_lorestore::Lorestore>,
    /// The root path the lorestores were opened under (one subdirectory per
    /// base), so new packs can be imported on-demand.
    lorestore_root: Option<std::path::PathBuf>,
    /// Origin lorestore directories for store-backed documents, keyed by base.
    /// These are attached in place of an import copy (DL-0055).
    origins: BTreeMap<String, std::path::PathBuf>,
    /// Bases the manifest pinned to an explicit commit, which the served world
    /// must not follow off.
    pinned_bases: BTreeSet<String>,
    /// The immutable commit-and-valuation pair selected by project loading for
    /// each store-backed base. This is deliberately distinct from the live
    /// lorestore handle: an external CAS after load must not mix its state into
    /// an already loaded project.
    snapshots: BTreeMap<String, StoreSnapshot>,
    /// Deterministic operation counters for the durable base-refresh paths.
    /// They are diagnostic only; no verdict depends on them.
    base_update_stats: BaseUpdateStats,
    /// Unit-test-only scheduling seam for a ref movement after materialize has
    /// prepared its exact candidate but before it enters the fenced commit.
    #[cfg(test)]
    materialize_pre_cas_hook: Option<fn(&mut Store, &str)>,
}

/// Diagnostic counts for workspace-base preparation and installation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BaseUpdateStats {
    /// Full candidate compiles prepared before a bulk commit.
    pub prepared_compiles: usize,
    /// Legacy reconstruct/compile reloads.
    pub reloads: usize,
    /// Infallible installs of a successfully committed prepared base.
    pub prepared_installs: usize,
}

/// A fully compiled base and the exact durable standing snapshot applied to it.
///
/// Keeping the structural world and its named overlay together prevents an
/// infallible post-CAS install from accidentally reusing workspace-stale
/// standing state.
#[derive(Debug)]
pub struct PreparedWorkspaceBase {
    pub(super) pack: Pack,
    pub(super) world: CompiledWorld,
    pub(super) base_valuation: NamedValuation,
    pub(super) base_unassigned: NamedAtomSet,
}

const MAX_DISCARD_REPLAYS: usize = 1024;

#[derive(Debug)]
struct Base {
    world: CompiledWorld,
    documents: Vec<Document>,
    /// The pack name the base was compiled under — the scope identity a
    /// lorestore import must reproduce to yield identical fact objects.
    pack_name: String,
}

#[derive(Debug, Clone)]
struct StoreSnapshot {
    commit: dli_lorestore::ContentHash,
    valuation: NamedValuation,
    unassigned: NamedAtomSet,
    ref_digest: Option<String>,
}

/// A caller-project store cannot be constructed unambiguously.
#[derive(Debug, Error)]
pub enum StoreError {
    /// One project document failed to compile.
    #[error(transparent)]
    Compile(#[from] EvalError),
    /// Two supplied packs used the same logical document id.
    #[error("project document `{0}` was supplied more than once")]
    DuplicateDocument(String),
    /// A fork question was declared by more than one project document.
    #[error("fork `{fork}` is declared by both project documents `{first}` and `{second}`")]
    DuplicateFork {
        /// The duplicated fork question id.
        fork: String,
        /// The first owning document.
        first: String,
        /// The second owning document.
        second: String,
    },
    /// A store ref valuation could not be resolved into its compiled document.
    #[error("store ref valuation is corrupt: {0}")]
    ValuationRemap(#[from] ValuationRemapError),
}

/// Why [`Store::open`] refused to open a workspace — distinct wire error codes
/// for each cause (WS-3's `author.open` `branch` validation: `author.open`
/// needs `UnknownDoc` for the pre-1.7.0 shape unchanged, and three new
/// branch-specific outcomes the caller can tell apart).
#[derive(Debug, Error)]
pub enum OpenError {
    /// No project document (or, for `fork` mode, no fork owner) matches —
    /// the pre-1.7.0 failure. Message text unchanged (byte-pinned): earlier
    /// builds constructed this string inline at the call site, so a
    /// pre-1.7.0 fixture's wording must survive the move verbatim.
    #[error("no project document named \"{0}\" is loaded")]
    UnknownDoc(String),
    /// A `branch` was supplied but the resolved base carries no attached
    /// lorestore (a durable operation attempted in explicit demo mode).
    #[error("branch-scoped opens need a durable lorestore; run against a project store")]
    ProjectRequired,
    /// A `branch` was supplied but no branch of that name exists.
    #[error("no branch named \"{0}\" exists")]
    UnknownBranch(String),
    /// Reconstructing or compiling the branch head failed — a store-integrity
    /// failure, not a caller mistake: every branch was, by construction,
    /// compilable when created.
    #[error("reconstructing branch head: {0}")]
    BranchCompile(String),
    /// The branch ref's numeric valuation cannot be resolved through its
    /// durable source names or into the freshly compiled receiving world.
    #[error("branch ref valuation is corrupt: {0}")]
    ValuationRemap(#[from] ValuationRemapError),
}

impl Default for Store {
    fn default() -> Self {
        Self {
            fallback: Some(Base {
                world: CompiledWorld::default(),
                documents: Vec::new(),
                pack_name: String::new(),
            }),
            bases: BTreeMap::new(),
            fork_bases: BTreeMap::new(),
            strict_forks: false,
            workspaces: BTreeMap::new(),
            workspace_bases: BTreeMap::new(),
            branch_bases: BTreeMap::new(),
            discard_replays: BTreeMap::new(),
            discard_replay_order: VecDeque::new(),
            next_id: 0,
            lorestores: BTreeMap::new(),
            lorestore_root: None,
            origins: BTreeMap::new(),
            pinned_bases: BTreeSet::new(),
            snapshots: BTreeMap::new(),
            base_update_stats: BaseUpdateStats::default(),
            #[cfg(test)]
            materialize_pre_cas_hook: None,
        }
    }
}

impl Store {
    /// A test store over a given compiled base. The base carries no
    /// re-compilable source, so read tools
    /// over this store evaluate the staged declarations alone; prefer
    /// [`with_pack`](Store::with_pack) when the source pack is available.
    #[must_use]
    pub fn with_base(base: CompiledWorld) -> Self {
        Self {
            fallback: Some(Base {
                world: base,
                documents: Vec::new(),
                pack_name: String::new(),
            }),
            bases: BTreeMap::new(),
            fork_bases: BTreeMap::new(),
            strict_forks: false,
            workspaces: BTreeMap::new(),
            workspace_bases: BTreeMap::new(),
            branch_bases: BTreeMap::new(),
            discard_replays: BTreeMap::new(),
            discard_replay_order: VecDeque::new(),
            next_id: 0,
            lorestores: BTreeMap::new(),
            lorestore_root: None,
            origins: BTreeMap::new(),
            pinned_bases: BTreeSet::new(),
            snapshots: BTreeMap::new(),
            base_update_stats: BaseUpdateStats::default(),
            #[cfg(test)]
            materialize_pre_cas_hook: None,
        }
    }

    /// A store over a source pack: compile it for the mutation-time gate **and**
    /// keep its documents, so the read tools recompile `base + staged` through the
    /// real loader. This is the read-complete constructor.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] if the pack does not compile (an unstratifiable
    /// program); malformed claims are never errors — they compile to fail-closed
    /// findings.
    pub fn with_pack(pack: Pack) -> Result<Self, EvalError> {
        let base = compile(&pack)?;
        let fork_bases = fork_owners(&pack.name, &pack.documents)
            .into_iter()
            .collect();
        let mut bases = BTreeMap::new();
        bases.insert(
            pack.name.clone(),
            Base {
                world: base,
                documents: pack.documents,
                pack_name: pack.name,
            },
        );
        Ok(Self {
            fallback: None,
            bases,
            fork_bases,
            strict_forks: false,
            workspaces: BTreeMap::new(),
            workspace_bases: BTreeMap::new(),
            branch_bases: BTreeMap::new(),
            discard_replays: BTreeMap::new(),
            discard_replay_order: VecDeque::new(),
            next_id: 0,
            lorestores: BTreeMap::new(),
            lorestore_root: None,
            origins: BTreeMap::new(),
            pinned_bases: BTreeSet::new(),
            snapshots: BTreeMap::new(),
            base_update_stats: BaseUpdateStats::default(),
            #[cfg(test)]
            materialize_pre_cas_hook: None,
        })
    }

    /// A strict multi-document store for a caller project.
    ///
    /// Duplicate logical ids and fork questions are rejected because either
    /// would make `author.open` ownership depend on insertion order.
    pub fn with_packs(
        packs: impl IntoIterator<Item = (String, Pack, dli_world::KilledTokens)>,
    ) -> Result<Self, StoreError> {
        let mut bases = BTreeMap::new();
        let mut fork_bases = BTreeMap::new();
        for (id, pack, killed) in packs {
            if bases.contains_key(&id) {
                return Err(StoreError::DuplicateDocument(id));
            }
            let world = compile(&pack)?.with_killed_tokens(killed);
            for (fork, owner) in fork_owners(&id, &pack.documents) {
                if let Some(first) = fork_bases.insert(fork.clone(), owner.clone()) {
                    return Err(StoreError::DuplicateFork {
                        fork,
                        first,
                        second: owner,
                    });
                }
            }
            bases.insert(
                id,
                Base {
                    world,
                    documents: pack.documents,
                    pack_name: pack.name,
                },
            );
        }
        Ok(Self {
            fallback: None,
            bases,
            fork_bases,
            strict_forks: true,
            workspaces: BTreeMap::new(),
            workspace_bases: BTreeMap::new(),
            branch_bases: BTreeMap::new(),
            discard_replays: BTreeMap::new(),
            discard_replay_order: VecDeque::new(),
            next_id: 0,
            lorestores: BTreeMap::new(),
            lorestore_root: None,
            origins: BTreeMap::new(),
            pinned_bases: BTreeSet::new(),
            snapshots: BTreeMap::new(),
            base_update_stats: BaseUpdateStats::default(),
            #[cfg(test)]
            materialize_pre_cas_hook: None,
        })
    }

    /// Every open workspace, in id order.
    pub fn workspaces(&self) -> impl Iterator<Item = &Workspace> {
        self.workspaces.values()
    }

    /// Open a new empty workspace and return it. Ids are minted `ws-1`, `ws-2`, …
    /// in open order.
    ///
    /// `branch`, when supplied, opens the workspace against a named branch
    /// head instead of the project document's main lorestore ref (WS-3): the
    /// branch's head is reconstructed and compiled fresh (a per-workspace read,
    /// not shared with any other open workspace on the same branch), and every
    /// subsequent read/write against this workspace id sees that compiled
    /// world and routes to that branch — never `main` — until the workspace is
    /// discarded. `branch: None` is the entire pre-1.7.0 path, untouched below:
    /// same fields, same order, same fallbacks.
    ///
    /// # Errors
    ///
    /// [`OpenError::UnknownDoc`] if `mode`/`base_name` name no project document
    /// (or, for [`Mode::Fork`], no fork owner) — the pre-1.7.0 failure, same
    /// message as before. [`OpenError::ProjectRequired`] if `branch` is
    /// supplied but the resolved base has no attached lorestore.
    /// [`OpenError::UnknownBranch`] if `branch` is supplied but no branch of
    /// that name exists. [`OpenError::BranchCompile`] if the branch head fails
    /// to reconstruct or compile (a store-integrity failure, not a caller
    /// mistake — every branch was, by construction, compilable when created).
    /// [`OpenError::ValuationRemap`] if the branch ref's persisted valuation
    /// cannot be resolved by name into the reconstructed receiving world.
    #[allow(clippy::too_many_lines)]
    pub fn open(
        &mut self,
        mode: Mode,
        base_name: String,
        branch: Option<String>,
    ) -> Result<&Workspace, OpenError> {
        let base_key = match mode {
            Mode::Scope => base_name.clone(),
            Mode::Fork => self
                .fork_bases
                .get(&base_name)
                .cloned()
                .or_else(|| {
                    (!self.strict_forks && self.bases.len() == 1)
                        .then(|| self.bases.keys().next().cloned())
                        .flatten()
                })
                .or_else(|| (!self.strict_forks).then(|| base_name.clone()))
                .ok_or_else(|| OpenError::UnknownDoc(base_name.clone()))?,
        };
        let base = self
            .base_named(&base_key)
            .ok_or_else(|| OpenError::UnknownDoc(base_name.clone()))?;

        if let Some(branch_name) = branch {
            // Branch mode reads a point-in-time compile of the branch head —
            // deliberately NOT sharing the pinned/snapshot machinery below,
            // which is about the project's chosen MAIN commit. This is its own
            // third path.
            let killed = base.world.killed.clone();
            let ls = self
                .lorestores
                .get(&base_key)
                .ok_or(OpenError::ProjectRequired)?;
            let branch_ref = ls
                .branch_ref(&branch_name)
                .map_err(|error| OpenError::BranchCompile(error.to_string()))?
                .ok_or_else(|| OpenError::UnknownBranch(branch_name.clone()))?;
            let head = branch_ref.commit.clone();
            // Both halves go through the store's session cache (Phase 3 PR 3):
            // opening the same branch head twice — the common shape when a
            // session opens, materializes, and reopens — reconstructs and
            // compiles once. The answers are identical either way; a commit hash
            // names immutable content.
            let pack = ls
                .reconstruct_at(&head)
                .map_err(|error| OpenError::BranchCompile(error.to_string()))?;
            let mut world = ls
                .compile_at(&head)
                .map_err(|error| OpenError::BranchCompile(error.to_string()))?;
            world.killed = killed;
            let branch_names = ls
                .interner_at(&head)
                .map_err(|error| OpenError::BranchCompile(error.to_string()))?;

            self.next_id += 1;
            let id = format!("ws-{}", self.next_id);
            let mut workspace = Workspace::new(id.clone(), mode, base_name);
            let mut bundle_counter = 0u32;
            let prefix = format!("h.{}", workspace.base_name);
            for name in &branch_names {
                let Some(n_str) = name.strip_prefix(&format!("{prefix}#")) else {
                    continue;
                };
                if let Ok(n) = n_str.parse::<u32>() {
                    bundle_counter = bundle_counter.max(n + 1);
                }
            }
            let effective = ls
                .effective_valuation_at(&head, &branch_ref.valuation)
                .map_err(|error| match error {
                    dli_lorestore::StoreError::ValuationRemap(error) => {
                        OpenError::ValuationRemap(error)
                    }
                    other => OpenError::BranchCompile(other.to_string()),
                })?;
            let named_valuation = NamedValuation::from_indexed(&effective.valuation, &branch_names)
                .map_err(OpenError::ValuationRemap)?;
            let named_unassigned = NamedAtomSet::from_indexed(&effective.unassigned, &branch_names)
                .map_err(OpenError::ValuationRemap)?;
            let declared_decisions: BTreeSet<&str> = pack
                .documents
                .iter()
                .flat_map(|document| &document.decls)
                .filter_map(|decl| match decl {
                    Decl::Ratify(ratify) => Some(ratify.decision.text.as_str()),
                    _ => None,
                })
                .collect();
            for name in named_valuation
                .0
                .keys()
                .chain(named_unassigned.0.iter())
                .filter(|name| declared_decisions.contains(name.as_str()))
            {
                world.model.world.interner.intern(name);
            }
            let target_interner = world.model.interner().clone();
            named_valuation
                .apply_to(&target_interner, &mut world.model.world.valuation)
                .map_err(OpenError::ValuationRemap)?;
            named_unassigned
                .remove_from(&target_interner, &mut world.model.world.valuation)
                .map_err(OpenError::ValuationRemap)?;
            workspace.base_valuation = named_valuation;
            workspace.base_unassigned = named_unassigned;
            workspace.interner = dli_core::atom::Interner::from_names(branch_names);
            workspace.bundle_counter = bundle_counter;
            workspace.base_root_hash = Some(head.to_string());
            workspace.base_ref_digest = branch_ref.digest().ok().map(|digest| digest.0);
            workspace.target_branch = Some(branch_name);
            workspace.refresh_resolution(&world, &pack.documents);
            // `Workspace::new` already sealed the initial digest with
            // `target_branch: None` (it has no way to know the branch yet at
            // construction). Reseal now that it is set, or the fence never
            // reflects it at all.
            workspace.reseal();

            self.branch_bases.insert(
                id.clone(),
                Base {
                    world,
                    documents: pack.documents,
                    pack_name: pack.name,
                },
            );
            self.workspace_bases.insert(id.clone(), base_key);
            return Ok(self.workspaces.entry(id.clone()).or_insert(workspace));
        }

        // A pinned replay serves its manifest commit but explicitly authors to
        // the live origin. Capture that origin ref once at open for both its
        // interner and the write fence. (WS-3 branch-mode opens never reach
        // this point — they return above, before any of this pinned/snapshot
        // machinery, which is about the project's chosen MAIN commit.)
        let pinned_origin_ref = self.is_pinned(&base_key).then(|| {
            self.lorestores
                .get(&base_key)
                .and_then(|ls| ls.head_ref().ok().flatten())
        });
        let base_names = match (
            self.lorestores.get(&base_key),
            self.snapshots.get(&base_key),
            pinned_origin_ref.as_ref().and_then(Option::as_ref),
        ) {
            // A pinned project replays the selected pack and valuation, but its
            // documented authoring path appends to the live origin branch. Keep
            // that branch's interner seed so an intervening append cannot cause
            // an avoidable interner-regression refusal on materialize.
            (Some(ls), Some(_), Some(origin_ref)) => ls
                .interner_at(&origin_ref.commit)
                .unwrap_or_else(|_| base.world.model().interner().names().to_vec()),
            (Some(ls), Some(snapshot), _) => ls
                .interner_at(&snapshot.commit)
                .unwrap_or_else(|_| base.world.model().interner().names().to_vec()),
            _ => base.world.model().interner().names().to_vec(),
        };
        let next_id = self.next_id + 1;
        let id = format!("ws-{next_id}");
        let mut workspace = Workspace::new(id.clone(), mode, base_name);
        // The seeded names give `lower_decl` (LS-4b) matching Atom indices.
        // Also compute the bundle counter continuation.
        let mut bundle_counter = 0u32;
        let prefix = format!("h.{}", workspace.base_name);
        for name in &base_names {
            let Some(n_str) = name.strip_prefix(&format!("{prefix}#")) else {
                continue;
            };
            if let Ok(n) = n_str.parse::<u32>() {
                bundle_counter = bundle_counter.max(n + 1);
            }
        }
        workspace.interner = dli_core::atom::Interner::from_names(base_names);
        workspace.bundle_counter = bundle_counter;
        // Pin the project-selected commit and valuation together. For local
        // demo stores no snapshot exists, preserving their previous behavior.
        if let Some(snapshot) = self.snapshots.get(&base_key) {
            workspace.base_root_hash = Some(snapshot.commit.to_string());
            workspace.base_valuation = snapshot.valuation.clone();
            workspace.base_unassigned.clone_from(&snapshot.unassigned);
            if let Some(origin_ref) = pinned_origin_ref.flatten() {
                workspace.base_ref_digest = origin_ref.digest().ok().map(|digest| digest.0);
            } else {
                workspace.base_ref_digest.clone_from(&snapshot.ref_digest);
            }
        } else {
            workspace.base_root_hash = self
                .lorestores
                .get(&base_key)
                .and_then(|ls| ls.root_hash().ok().flatten())
                .map(|h| h.to_string());
        }
        workspace.refresh_resolution(&base.world, &base.documents);
        self.next_id = next_id;
        self.workspace_bases.insert(id.clone(), base_key);
        Ok(self.workspaces.entry(id.clone()).or_insert(workspace))
    }

    /// A shared reference to a workspace by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Workspace> {
        self.workspaces.get(id)
    }

    /// The base and a mutable workspace together, borrowing disjoint fields so a
    /// tool can compute findings against the base while staging into the
    /// workspace. A branch-mode workspace's own compiled branch base
    /// (`branch_bases`, keyed by workspace id — WS-3) is consulted first, so a
    /// branch's world, not the project document's main world, is what
    /// findings and staging compute against.
    pub fn parts_mut(&mut self, id: &str) -> Option<(&CompiledWorld, &mut Workspace)> {
        let base = if let Some(base) = self.branch_bases.get(id) {
            &base.world
        } else {
            let base_name = self.workspace_bases.get(id)?.clone();
            if let Some(base) = self.bases.get(&base_name) {
                &base.world
            } else {
                &self.fallback.as_ref()?.world
            }
        };
        self.workspaces.get_mut(id).map(|ws| (base, ws))
    }

    /// The compiled base selected by a workspace. A branch-mode workspace's
    /// own compiled branch base is consulted first (WS-3).
    #[must_use]
    pub fn workspace_base(&self, id: &str) -> Option<&CompiledWorld> {
        if let Some(base) = self.branch_bases.get(id) {
            return Some(&base.world);
        }
        let base_name = self.workspace_bases.get(id)?;
        self.base_named(base_name).map(|base| &base.world)
    }

    /// The source documents selected by a workspace. A branch-mode
    /// workspace's own reconstructed branch documents are consulted first
    /// (WS-3).
    #[must_use]
    pub fn workspace_base_docs(&self, id: &str) -> Option<&[Document]> {
        if let Some(base) = self.branch_bases.get(id) {
            return Some(base.documents.as_slice());
        }
        let base_name = self.workspace_bases.get(id)?;
        self.base_named(base_name)
            .map(|base| base.documents.as_slice())
    }

    /// Reconstruct a workspace's base from its target-branch head and install it
    /// as the workspace's own base (WS-3's `branch_bases` slot, consulted first),
    /// so a durable write that grew the branch — an `author.record_decision`
    /// `ratify`, DL-0083 — becomes visible to every subsequent staged read
    /// (`workspace_decisions`, `staged_world`, `author.check`) on the same
    /// workspace. Without it a recorded decision would sit in the lorestore but
    /// stay invisible to the static scope base the workspace opened over, so a
    /// fact citing it would read `citation.unknown-decision` rather than certain.
    /// The workspace's interner and bundle counter are re-aligned to the new head
    /// so subsequent staging mints byte-identical hashes. Idempotent.
    ///
    /// # Errors
    ///
    /// Returns the failure message if the workspace has no attached lorestore,
    /// its target branch has no ref, or the reconstructed pack fails to
    /// recompile.
    pub fn reload_workspace_base(&mut self, ws_id: &str) -> Result<(), String> {
        self.base_update_stats.reloads += 1;
        let base_key = self
            .workspace_bases
            .get(ws_id)
            .cloned()
            .ok_or_else(|| format!("no base is bound to workspace `{ws_id}`"))?;
        let killed = self
            .base_named(&base_key)
            .map(|base| base.world.killed.clone())
            .unwrap_or_default();
        let target = self
            .workspaces
            .get(ws_id)
            .and_then(|ws| ws.target_branch.clone())
            .unwrap_or_else(|| dli_lorestore::MAIN_REF.to_owned());
        let ls = self
            .lorestores
            .get(&base_key)
            .ok_or_else(|| format!("workspace `{ws_id}` has no attached lorestore"))?;
        let branch_ref = ls
            .branch_ref(&target)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("branch `{target}` has no ref"))?;
        let head = branch_ref.commit.clone();
        let effective_valuation = ls
            .effective_valuation_at(&head, &branch_ref.valuation)
            .map_err(|error| error.to_string())?;
        // Through the session cache, as branch-open above: a reload after a
        // materialize that did not advance the target branch is a pure repeat,
        // and the reload that follows one that did lands on a commit the merge
        // scan or the materialize itself has usually already compiled.
        let pack = ls
            .reconstruct_at(&head)
            .map_err(|error| error.to_string())?;
        let interner = ls.interner_at(&head).map_err(|error| error.to_string())?;
        let named_valuation =
            NamedValuation::from_indexed(&effective_valuation.valuation, &interner)
                .map_err(|error| error.to_string())?;
        let named_unassigned =
            NamedAtomSet::from_indexed(&effective_valuation.unassigned, &interner)
                .map_err(|error| error.to_string())?;
        let mut world = ls.compile_at(&head).map_err(|error| error.to_string())?;
        for name in named_valuation.0.keys().chain(named_unassigned.0.iter()) {
            world.model.world.interner.intern(name);
        }
        let target_interner = world.model.interner().clone();
        named_valuation
            .apply_to(&target_interner, &mut world.model.world.valuation)
            .map_err(|error| error.to_string())?;
        named_unassigned
            .remove_from(&target_interner, &mut world.model.world.valuation)
            .map_err(|error| error.to_string())?;
        world.killed = killed;
        if let Some(ws) = self.workspaces.get_mut(ws_id) {
            let prefix = format!("h.{}#", ws.base_name);
            let mut bundle = 0u32;
            for name in &interner {
                if let Some(rest) = name.strip_prefix(&prefix)
                    && let Ok(value) = rest.parse::<u32>()
                {
                    bundle = bundle.max(value + 1);
                }
            }
            ws.interner = dli_core::atom::Interner::from_names(interner);
            ws.bundle_counter = bundle;
            ws.base_root_hash = Some(head.to_string());
            ws.base_ref_digest = branch_ref.digest().ok().map(|digest| digest.0);
            ws.base_valuation = named_valuation;
            ws.base_unassigned = named_unassigned;
        }
        let refreshed = Base {
            world,
            documents: pack.documents,
            pack_name: pack.name,
        };
        let structural_changed = self.workspace_base(ws_id).is_none_or(|current| {
            current.killed != refreshed.world.killed
                || self
                    .workspace_base_docs(ws_id)
                    .is_none_or(|documents| documents != refreshed.documents)
        });
        if let Some(ws) = self.workspaces.get_mut(ws_id) {
            if structural_changed {
                ws.refresh_structure(&refreshed.world, &refreshed.documents);
            } else {
                // Attack-only commits and ref-standing movement keep the exact
                // same symbolic documents. Refresh the derived add-time index,
                // but retain the saturate-once compile.
                ws.refresh_resolution(&refreshed.world, &refreshed.documents);
            }
        }
        self.branch_bases.insert(ws_id.to_owned(), refreshed);
        Ok(())
    }

    /// Refresh only a workspace's durable valuation overlay and resolution
    /// index, keeping its current base documents intact. Standing changes use
    /// this path because already-materialized declarations remain in the
    /// workspace's staged document set until its ordinary session lifecycle
    /// clears them; reconstructing the new durable head here would therefore
    /// install a second copy beside those staged declarations.
    pub fn refresh_workspace_valuation(&mut self, ws_id: &str) -> Result<(), String> {
        let base_key = self
            .workspace_bases
            .get(ws_id)
            .cloned()
            .ok_or_else(|| format!("no base is bound to workspace `{ws_id}`"))?;
        let named = self
            .workspaces
            .get(ws_id)
            .map(|workspace| workspace.base_valuation.clone())
            .ok_or_else(|| format!("workspace `{ws_id}` is not open"))?;
        let unassigned = self
            .workspaces
            .get(ws_id)
            .map(|workspace| workspace.base_unassigned.clone())
            .ok_or_else(|| format!("workspace `{ws_id}` is not open"))?;
        let current = self
            .branch_bases
            .get(ws_id)
            .or_else(|| self.base_named(&base_key))
            .ok_or_else(|| format!("workspace `{ws_id}` has no compiled base"))?;
        let mut refreshed = Base {
            world: current.world.clone(),
            documents: current.documents.clone(),
            pack_name: current.pack_name.clone(),
        };
        let target_interner = refreshed.world.model.interner().clone();
        // This is a projection onto the base half, not the fail-closed
        // full-world boundary: a decision introduced only by a staged
        // declaration is absent here but present when `staged_world` applies
        // the complete overlay. Such a name cannot affect any base atom.
        let target_names: BTreeSet<&str> =
            target_interner.names().iter().map(String::as_str).collect();
        let base_named = NamedValuation(
            named
                .0
                .into_iter()
                .filter(|(name, _)| target_names.contains(name.as_str()))
                .collect(),
        );
        let base_unassigned = NamedAtomSet(
            unassigned
                .0
                .into_iter()
                .filter(|name| target_names.contains(name.as_str()))
                .collect(),
        );
        base_named
            .apply_to(&target_interner, &mut refreshed.world.model.world.valuation)
            .map_err(|error| error.to_string())?;
        base_unassigned
            .remove_from(&target_interner, &mut refreshed.world.model.world.valuation)
            .map_err(|error| error.to_string())?;
        if let Some(workspace) = self.workspaces.get_mut(ws_id) {
            workspace.refresh_resolution(&refreshed.world, &refreshed.documents);
        }
        self.branch_bases.insert(ws_id.to_owned(), refreshed);
        Ok(())
    }

    /// Compile a complete candidate workspace base before a durable batch write.
    /// The returned pack/world pair is ready for an infallible install after the
    /// branch CAS succeeds.
    pub fn prepare_workspace_base(
        &mut self,
        ws_id: &str,
        pack: Pack,
        base_valuation: NamedValuation,
        base_unassigned: NamedAtomSet,
    ) -> Result<PreparedWorkspaceBase, StoreError> {
        let killed = self
            .workspace_base(ws_id)
            .map(|base| base.killed.clone())
            .unwrap_or_default();
        let mut world = compile(&pack)?;
        world.killed = killed;
        debug_assert!(self.workspaces.contains_key(ws_id));
        for name in base_valuation.0.keys().chain(base_unassigned.0.iter()) {
            world.model.world.interner.intern(name);
        }
        dli_core::valuation::apply_named_overlay(
            &base_valuation,
            &base_unassigned,
            &world.model.world.interner,
            &mut world.model.world.valuation,
        )?;
        self.base_update_stats.prepared_compiles += 1;
        Ok(PreparedWorkspaceBase {
            pack,
            world,
            base_valuation,
            base_unassigned,
        })
    }

    /// Install a base that was compiled before its commit/CAS. Every fallible
    /// operation has already succeeded; this method only swaps owned in-memory
    /// state, so a successful durable batch cannot be followed by a refresh
    /// failure that reports an ambiguous outcome.
    pub fn install_prepared_workspace_base(
        &mut self,
        ws_id: &str,
        prepared: PreparedWorkspaceBase,
        interner: Vec<String>,
        commit: &dli_lorestore::ContentHash,
        ref_digest: &dli_lorestore::ContentHash,
    ) -> String {
        let mut bundle = 0u32;
        let prefix = self
            .workspaces
            .get(ws_id)
            .map(|ws| format!("h.{}#", ws.base_name))
            .unwrap_or_default();
        for name in &interner {
            if let Some(rest) = name.strip_prefix(&prefix)
                && let Ok(value) = rest.parse::<u32>()
            {
                bundle = bundle.max(value + 1);
            }
        }
        let PreparedWorkspaceBase {
            pack,
            world,
            base_valuation,
            base_unassigned,
        } = prepared;
        let refreshed = Base {
            world,
            documents: pack.documents,
            pack_name: pack.name,
        };
        let ws = self
            .workspaces
            .get_mut(ws_id)
            .expect("prepared base belongs to a live workspace");
        ws.base_root_hash = Some(commit.to_string());
        ws.base_ref_digest = Some(ref_digest.to_string());
        ws.interner = dli_core::atom::Interner::from_names(interner);
        ws.bundle_counter = bundle;
        ws.base_valuation = base_valuation;
        ws.base_unassigned = base_unassigned;
        ws.refresh_structure(&refreshed.world, &refreshed.documents);
        let digest = ws.digest.clone();
        self.branch_bases.insert(ws_id.to_owned(), refreshed);
        self.base_update_stats.prepared_installs += 1;
        digest
    }

    /// Infallibly install a successfully materialized base and reconcile the
    /// staged bookkeeping in the same owned-state transition. All validation,
    /// object reads, and compilation must precede the durable branch CAS; this
    /// method is the immediate post-CAS continuation.
    pub fn install_materialized_workspace_base(
        &mut self,
        ws_id: &str,
        prepared: PreparedWorkspaceBase,
        interner: Vec<String>,
        commit: &dli_lorestore::ContentHash,
        ref_digest: &dli_lorestore::ContentHash,
        materialized_keys: &BTreeSet<String>,
    ) -> String {
        {
            let workspace = self
                .workspaces
                .get_mut(ws_id)
                .expect("the prepared base belongs to this live workspace");
            // Reconcile before the base refresh so its derived resolution index
            // replays only the still-pending delta atop the newly installed
            // durable world. Both mutations are infallible and remain strictly
            // after the successful CAS.
            workspace.mark_materialized(materialized_keys);
            workspace.staged_fact_hashes.clear();
        }
        self.install_prepared_workspace_base(ws_id, prepared, interner, commit, ref_digest)
    }

    /// Diagnostic operation counts used by deterministic scale tests.
    #[must_use]
    pub const fn base_update_stats(&self) -> BaseUpdateStats {
        self.base_update_stats
    }

    #[cfg(test)]
    pub(super) fn run_materialize_pre_cas_hook(&mut self, ws_id: &str) {
        if let Some(hook) = self.materialize_pre_cas_hook.take() {
            hook(self, ws_id);
        }
    }

    /// The lorestore backing a base, if one is attached (LS-4). Each base has
    /// its own single-branch store — one store, one pack lineage.
    #[must_use]
    pub fn lorestore_for(&self, base_key: &str) -> Option<&dli_lorestore::Lorestore> {
        self.lorestores.get(base_key)
    }

    /// The lorestore backing the base a workspace was opened over, if any.
    #[must_use]
    pub fn workspace_lorestore(&self, ws_id: &str) -> Option<&dli_lorestore::Lorestore> {
        let base_key = self.workspace_bases.get(ws_id)?;
        self.lorestores.get(base_key)
    }

    /// Conditional decision attacks folded at the exact commit this workspace
    /// currently reads. Demo/in-memory workspaces have no durable attack lane
    /// and therefore return an empty set.
    pub fn workspace_decision_attacks(
        &self,
        ws_id: &str,
    ) -> Result<Vec<dli_lorestore::DecisionAttack>, String> {
        let Some(workspace) = self.workspaces.get(ws_id) else {
            return Err(format!("no workspace named `{ws_id}` is open"));
        };
        let Some(lorestore) = self.workspace_lorestore(ws_id) else {
            return Ok(Vec::new());
        };
        let Some(head) = workspace
            .base_root_hash
            .as_deref()
            .and_then(dli_lorestore::ContentHash::parse)
        else {
            return Ok(Vec::new());
        };
        lorestore
            .decision_attacks_from(&head)
            .map_err(|error| error.to_string())
    }

    /// Mutable access to the lorestore backing a workspace's base.
    pub fn workspace_lorestore_mut(
        &mut self,
        ws_id: &str,
    ) -> Option<&mut dli_lorestore::Lorestore> {
        let base_key = self.workspace_bases.get(ws_id)?.clone();
        self.lorestores.get_mut(&base_key)
    }

    /// Attach lorestores under the given root — one subdirectory per base —
    /// and idempotently import each base pack, so the store's folded state
    /// always contains the world the findings are graded against (audit A1).
    /// Called by `serve.rs` when a project directory is available.
    ///
    /// Re-attaching is cheap: `ensure_pack_imported` writes nothing when the
    /// head already covers the base.
    pub fn attach_lorestore(
        &mut self,
        root: std::path::PathBuf,
    ) -> Result<(), dli_lorestore::StoreError> {
        for (key, base) in &self.bases {
            if self.lorestores.contains_key(key) {
                continue;
            }
            // A store-backed document attaches its **origin** store, not an
            // import copy under the server root, so a materialize advances the
            // very store the project loads from — that is what closes the
            // durable authoring loop (DL-0055). It is also not re-imported:
            // the pack was reconstructed *from* this store, so there is
            // nothing to import, and importing a pinned document's older
            // commit would write a new one — a read path with a side effect.
            if let Some(origin) = self.origins.get(key) {
                let ls = dli_lorestore::Lorestore::open(origin.clone())?;
                self.lorestores.insert(key.clone(), ls);
                continue;
            }
            let ls = dli_lorestore::Lorestore::open(root.join(key))?;
            let pack = Pack {
                name: base.pack_name.clone(),
                documents: base.documents.clone(),
            };
            ls.ensure_pack_imported(&pack)?;
            self.lorestores.insert(key.clone(), ls);
        }
        self.lorestore_root = Some(root);
        Ok(())
    }

    /// Register the origin lorestore of a store-backed document, before
    /// [`attach_lorestore`](Self::attach_lorestore) runs.
    ///
    /// `pinned` records that the manifest fixed the document at an explicit
    /// commit. A pinned document still authors — the branch advances as
    /// materialize writes — but the *served* world stays the pinned commit
    /// until the manifest says otherwise, which is what makes a pinned project
    /// replayable.
    #[allow(clippy::too_many_arguments)]
    pub fn register_store_origin(
        &mut self,
        key: String,
        origin: std::path::PathBuf,
        pinned: bool,
        commit: dli_lorestore::ContentHash,
        valuation: NamedValuation,
        unassigned: NamedAtomSet,
        ref_digest: Option<String>,
    ) -> Result<(), StoreError> {
        self.origins.insert(key.clone(), origin);
        if let Some(base) = self.bases.get_mut(&key) {
            for name in valuation.0.keys().chain(unassigned.0.iter()) {
                base.world.model.world.interner.intern(name);
            }
            let target_interner = base.world.model.interner().clone();
            valuation.apply_to(&target_interner, &mut base.world.model.world.valuation)?;
            unassigned.remove_from(&target_interner, &mut base.world.model.world.valuation)?;
        }
        self.snapshots.insert(
            key.clone(),
            StoreSnapshot {
                commit,
                valuation,
                unassigned,
                ref_digest,
            },
        );
        if pinned {
            self.pinned_bases.insert(key);
        }
        Ok(())
    }

    /// Whether this base is served at a manifest-pinned commit.
    #[must_use]
    pub fn is_pinned(&self, key: &str) -> bool {
        self.pinned_bases.contains(key)
    }

    /// LS-4b: lower a staged decl to `FactObject`s and write them to the
    /// lorestore immediately (durable staging). Updates the workspace's
    /// `staged_fact_hashes`, the live interner, and `bundle_counter` in place.
    /// Does nothing if no lorestore is attached.
    ///
    /// Fail-closed: a write error advances **no** workspace state — no hash is
    /// recorded, the interner and bundle counter stay untouched — and the
    /// error propagates so the caller refuses the add. Objects written before
    /// the failure are orphans, not corruption: a retry rewrites them
    /// idempotently (CAS dedup) and only then commits the state advance.
    pub fn stage_lorestore(
        &mut self,
        ws_id: &str,
        decl: &Decl,
    ) -> Result<(), dli_lorestore::StoreError> {
        let Some(base_key) = self.workspace_bases.get(ws_id).cloned() else {
            return Ok(());
        };
        let Some(ws) = self.workspaces.get_mut(ws_id) else {
            return Ok(());
        };
        let Some(ls) = self.lorestores.get(&base_key) else {
            return Ok(());
        };
        let pack_name = ws.base_name.clone();
        let interner_checkpoint = ws.interner.len();
        let bundle_checkpoint = ws.bundle_counter;
        let objects = dli_lorestore::lower::lower_decl(
            decl,
            &pack_name,
            &mut ws.interner,
            &mut ws.bundle_counter,
        );

        let mut hashes = Vec::with_capacity(objects.len());
        for fo in &objects {
            match ls.write_object(fo) {
                Ok(hash) => hashes.push(hash),
                Err(error) => {
                    // Lowering only appends names. Restore the exact prefix and
                    // proposal-bundle counter so a failed durable write advances
                    // no workspace state. Objects written before the failure are
                    // harmless CAS orphans, as before.
                    ws.interner = dli_core::atom::Interner::from_names(
                        ws.interner.names()[..interner_checkpoint].to_vec(),
                    );
                    ws.bundle_counter = bundle_checkpoint;
                    return Err(error);
                }
            }
        }
        let key = super::tools::decl_key(decl);
        ws.staged_fact_hashes.push((key, hashes));
        Ok(())
    }

    /// Cache a mutating call's result under its idempotency key together with the
    /// payload fingerprint that produced it, so a retry with the *same* payload
    /// replays and stages nothing, while the same key with a *different* payload
    /// is caught as a conflict (spec 11 §2; R3/M9). A whole-workspace discard
    /// retains a compact tombstone because the successful operation removes the
    /// live workspace before its result can be cached there.
    pub fn remember(
        &mut self,
        id: &str,
        key: String,
        fingerprint: String,
        result: serde_json::Value,
    ) {
        if let Some(ws) = self.workspaces.get_mut(id) {
            ws.idempotency.insert(key, (fingerprint, result));
            return;
        }

        while self.discard_replays.len() >= MAX_DISCARD_REPLAYS {
            let Some(oldest) = self.discard_replay_order.pop_front() else {
                break;
            };
            self.discard_replays.remove(&oldest);
        }
        self.discard_replay_order.push_back(id.to_owned());
        self.discard_replays
            .insert(id.to_owned(), (key, fingerprint, result));
    }

    /// Find a live-workspace replay entry or the compact tombstone left by a
    /// whole-workspace discard.
    #[must_use]
    pub fn replay(&self, id: &str, key: &str) -> Option<(&str, &serde_json::Value)> {
        if let Some((fingerprint, result)) = self
            .workspaces
            .get(id)
            .and_then(|workspace| workspace.idempotency.get(key))
        {
            return Some((fingerprint, result));
        }
        self.discard_replays
            .get(id)
            .and_then(|(cached_key, fingerprint, result)| {
                (cached_key == key).then_some((fingerprint.as_str(), result))
            })
    }

    /// Discard a whole workspace, returning whether one was present. Clears
    /// the workspace's compiled branch base too (WS-3), if it had one — a
    /// fresh `author.open` against the same branch reconstructs it again from
    /// the (possibly since-advanced) branch head rather than reusing a stale
    /// compile.
    pub fn discard(&mut self, id: &str) -> bool {
        self.workspace_bases.remove(id);
        self.branch_bases.remove(id);
        self.workspaces.remove(id).is_some()
    }

    /// The base key (project document / lorestore id) a workspace was opened
    /// against — the key `workspace_lorestore` resolves through. Used by
    /// `author.branch`'s `delete` action to scope its "any open workspace
    /// still targets this branch" guard to the SAME lorestore the branch
    /// lives in (branch names are scoped per lorestore, so a same-named
    /// branch in a different document's store is unrelated).
    #[must_use]
    pub fn base_key_for(&self, id: &str) -> Option<&str> {
        self.workspace_bases.get(id).map(String::as_str)
    }

    /// Whether any currently open workspace targets `branch` within
    /// `base_key`'s lorestore (WS-3). `author.branch`'s `delete` action
    /// refuses while this holds — deleting the ref out from under a live
    /// authoring session would strand its next read/write.
    #[must_use]
    pub fn branch_has_open_workspace(&self, base_key: &str, branch: &str) -> bool {
        self.workspaces.values().any(|ws| {
            ws.target_branch.as_deref() == Some(branch)
                && self
                    .workspace_bases
                    .get(&ws.id)
                    .is_some_and(|key| key == base_key)
        })
    }

    fn base_named(&self, name: &str) -> Option<&Base> {
        self.bases.get(name).or(self.fallback.as_ref())
    }
}

fn fork_owners(document: &str, sources: &[Document]) -> Vec<(String, String)> {
    sources
        .iter()
        .flat_map(|source| &source.decls)
        .filter_map(|decl| match decl {
            Decl::Fork(fork) => Some((fork.question.text.clone(), document.to_owned())),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use dli_ast::annot::{Annotation, ProvenanceMarker};
    use dli_ast::decl::{AttrDecl, Decl};
    use dli_ast::ident::Ident;
    use dli_ast::span::Span;
    use dli_ast::term::Term;
    use dli_lorestore::storage::{Storage, StoreError};
    use dli_lorestore::{Commit, ContentHash, FilesystemStorage, Lorestore, Ref};
    use serde_json::json;

    use super::Store;
    use crate::author::workspace::Mode;

    #[derive(Debug)]
    struct WriteFails;

    #[derive(Debug, Default)]
    struct BatchStorageControl {
        reads_after_cas: AtomicUsize,
        writes: AtomicUsize,
        cas: AtomicUsize,
        cas_succeeded: AtomicBool,
        fail_write_at: AtomicUsize,
        fail_cas: AtomicBool,
    }

    #[derive(Debug)]
    struct CountingFaultStorage {
        inner: FilesystemStorage,
        control: Arc<BatchStorageControl>,
    }

    impl Storage for CountingFaultStorage {
        fn read_object(&self, hash: &ContentHash) -> Result<Option<Vec<u8>>, StoreError> {
            if self.control.cas_succeeded.load(Ordering::SeqCst) {
                self.control.reads_after_cas.fetch_add(1, Ordering::SeqCst);
            }
            self.inner.read_object(hash)
        }

        fn write_object(&self, bytes: &[u8]) -> Result<ContentHash, StoreError> {
            let call = self.control.writes.fetch_add(1, Ordering::SeqCst) + 1;
            if self.control.fail_write_at.load(Ordering::SeqCst) == call {
                return Err(injected_failure());
            }
            self.inner.write_object(bytes)
        }

        fn cas_ref(
            &self,
            name: &str,
            expected: Option<&str>,
            new: &Ref,
        ) -> Result<bool, StoreError> {
            self.control.cas.fetch_add(1, Ordering::SeqCst);
            if self.control.fail_cas.swap(false, Ordering::SeqCst) {
                return Err(injected_failure());
            }
            let swapped = self.inner.cas_ref(name, expected, new)?;
            if swapped {
                self.control.cas_succeeded.store(true, Ordering::SeqCst);
            }
            Ok(swapped)
        }

        fn load_ref(&self, name: &str) -> Result<Option<Ref>, StoreError> {
            self.inner.load_ref(name)
        }

        fn list_refs(&self) -> Result<Vec<String>, StoreError> {
            self.inner.list_refs()
        }

        fn delete_ref(&self, name: &str) -> Result<(), StoreError> {
            self.inner.delete_ref(name)
        }

        fn acquire_lock(&self) -> Result<File, StoreError> {
            self.inner.acquire_lock()
        }

        fn walk_dag(
            &self,
            tip: &ContentHash,
            base: Option<&ContentHash>,
        ) -> Result<Vec<Commit>, StoreError> {
            self.inner.walk_dag(tip, base)
        }

        fn cleanup_orphans(&self) -> Result<usize, StoreError> {
            self.inner.cleanup_orphans()
        }
    }

    fn injected_failure() -> StoreError {
        StoreError::Io {
            path: "injected".to_owned(),
            source: std::io::Error::other("injected write failure"),
        }
    }

    impl Storage for WriteFails {
        fn read_object(&self, _hash: &ContentHash) -> Result<Option<Vec<u8>>, StoreError> {
            Err(injected_failure())
        }

        fn write_object(&self, _bytes: &[u8]) -> Result<ContentHash, StoreError> {
            Err(injected_failure())
        }

        fn cas_ref(
            &self,
            _name: &str,
            _expected: Option<&str>,
            _new: &Ref,
        ) -> Result<bool, StoreError> {
            Err(injected_failure())
        }

        fn load_ref(&self, _name: &str) -> Result<Option<Ref>, StoreError> {
            Err(injected_failure())
        }

        fn list_refs(&self) -> Result<Vec<String>, StoreError> {
            Err(injected_failure())
        }

        fn delete_ref(&self, _name: &str) -> Result<(), StoreError> {
            Err(injected_failure())
        }

        fn acquire_lock(&self) -> Result<File, StoreError> {
            Err(injected_failure())
        }

        fn walk_dag(
            &self,
            _tip: &ContentHash,
            _base: Option<&ContentHash>,
        ) -> Result<Vec<Commit>, StoreError> {
            Err(injected_failure())
        }

        fn cleanup_orphans(&self) -> Result<usize, StoreError> {
            Err(injected_failure())
        }
    }

    fn proposed_fact(origin: &str, value: &str) -> Decl {
        Decl::Attr(AttrDecl {
            subject: Ident::synthetic("world"),
            attr: Ident::synthetic("holds"),
            value: Term::Str(value.to_owned()),
            provenance: Some(Annotation {
                marker: ProvenanceMarker::Proposed,
                citations: Vec::new(),
                condition: None,
                tags: if origin.is_empty() {
                    Vec::new()
                } else {
                    vec![origin.to_owned()]
                },
                span: Span::synthetic(),
            }),
        })
    }

    fn workspace(store: &mut Store) -> String {
        store
            .open(Mode::Scope, "saltmarket".to_owned(), None)
            .expect("open workspace")
            .id
            .clone()
    }

    fn batch_store() -> (tempfile::TempDir, Store, String, Arc<BatchStorageControl>) {
        let root = tempfile::tempdir().expect("temporary lorestore");
        let (document, diagnostics) =
            dli_syntax::parse_document("scope demo\nperson mara \"Mara\" proposed seed\n");
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.class.is_error())
        );
        let pack = dli_world::pack::Pack {
            name: "demo".to_owned(),
            documents: vec![document],
        };
        let mut store = Store::with_pack(pack).expect("base compiles");
        store
            .attach_lorestore(root.path().join("lorestores"))
            .expect("attach lorestore");
        let ws = store
            .open(Mode::Scope, "demo".to_owned(), None)
            .expect("open workspace")
            .id
            .clone();
        let base_key = store.workspace_bases[&ws].clone();
        let path = root.path().join("lorestores").join(&base_key);
        let control = Arc::new(BatchStorageControl::default());
        store.lorestores.insert(
            base_key,
            Lorestore::from_storage(Box::new(CountingFaultStorage {
                inner: FilesystemStorage::new(path),
                control: Arc::clone(&control),
            })),
        );
        (root, store, ws, control)
    }

    fn bulk_request(ws: &str, count: usize) -> crate::Request {
        crate::Request {
            dli: "1.13.0".to_owned(),
            id: "bulk".to_owned(),
            op: crate::Op::AuthorRecordDecisions,
            params: json!({
                "workspace": ws,
                "records": (0..count)
                    .map(|index| json!({"decision": format!("d-{index:05}")}))
                    .collect::<Vec<_>>()
            }),
            expected_digest: None,
            idempotency_key: None,
        }
    }

    fn semantic_workspace_state(store: &Store, ws: &str) -> serde_json::Value {
        let workspace = store.get(ws).expect("workspace");
        json!({
            "digest": workspace.digest,
            "root": workspace.base_root_hash,
            "ref": workspace.base_ref_digest,
            "valuation": workspace.base_valuation.0,
            "unassigned": workspace.base_unassigned.0,
            "interner": workspace.interner.names(),
            "decisions": workspace.known_decisions(),
        })
    }

    fn advance_main_ref_before_materialize_cas(store: &mut Store, ws: &str) {
        let lorestore = store.workspace_lorestore(ws).expect("lorestore");
        let current = lorestore
            .head_ref()
            .expect("head read")
            .expect("main exists");
        let token = current.digest().expect("ref digest").to_string();
        let mut valuation = current.valuation.clone();
        let atom = dli_core::Atom(0);
        valuation.set(atom, !valuation.get(atom).unwrap_or(false));
        let moved = Ref::with_valuation(dli_lorestore::MAIN_REF, current.commit, valuation);
        assert!(
            lorestore
                .cas_main_ref(Some(&token), &moved)
                .expect("injected ref movement")
        );
    }

    #[test]
    fn bulk_decision_write_and_cas_failures_leave_ref_and_workspace_unchanged() {
        for fail_cas in [false, true] {
            let (_root, mut store, ws, control) = batch_store();
            let before_ref = store
                .workspace_lorestore(&ws)
                .expect("lorestore")
                .root_hash()
                .expect("head read");
            let before_workspace = semantic_workspace_state(&store, &ws);
            if fail_cas {
                control.fail_cas.store(true, Ordering::SeqCst);
            } else {
                control.fail_write_at.store(1, Ordering::SeqCst);
            }
            let response = crate::author::dispatch(&bulk_request(&ws, 3), &mut store);
            assert!(!response.ok, "injected failure must refuse: {response:?}");
            assert_eq!(
                store
                    .workspace_lorestore(&ws)
                    .expect("lorestore")
                    .root_hash()
                    .expect("head read"),
                before_ref
            );
            assert_eq!(semantic_workspace_state(&store, &ws), before_workspace);
            assert_eq!(store.base_update_stats.prepared_installs, 0);
        }
    }

    #[test]
    fn materialize_reads_every_response_object_before_the_successful_cas() {
        let (_root, mut store, ws, control) = batch_store();
        let added = crate::author::dispatch(
            &crate::Request {
                dli: "1.13.0".to_owned(),
                id: "add".to_owned(),
                op: crate::Op::AuthorAddFact,
                params: json!({
                    "workspace": ws,
                    "entity": "mara",
                    "key": "rank",
                    "value": "silver",
                    "provenance": {"kind": "proposed", "origin": "atomicity-tooth"}
                }),
                expected_digest: None,
                idempotency_key: None,
            },
            &mut store,
        );
        assert!(added.ok, "staging succeeds: {added:?}");

        let cas_before = control.cas.load(Ordering::SeqCst);
        let materialized = crate::author::dispatch(
            &crate::Request {
                dli: "1.13.0".to_owned(),
                id: "materialize".to_owned(),
                op: crate::Op::AuthorMaterialize,
                params: json!({"workspace": ws, "rendered": false}),
                expected_digest: None,
                idempotency_key: None,
            },
            &mut store,
        );
        assert!(materialized.ok, "materialize succeeds: {materialized:?}");
        assert_eq!(control.cas.load(Ordering::SeqCst), cas_before + 1);
        assert!(control.cas_succeeded.load(Ordering::SeqCst));
        assert_eq!(
            control.reads_after_cas.load(Ordering::SeqCst),
            0,
            "the successful CAS must flow directly into the infallible prepared-base install"
        );
        assert_eq!(store.base_update_stats.prepared_installs, 1);
    }

    #[test]
    fn ordinary_main_materialize_fences_its_prepared_target_snapshot() {
        let (_root, mut store, ws, _control) = batch_store();
        let added = crate::author::dispatch(
            &crate::Request {
                dli: "1.13.0".to_owned(),
                id: "add-race".to_owned(),
                op: crate::Op::AuthorAddFact,
                params: json!({
                    "workspace": ws,
                    "entity": "mara",
                    "key": "rank",
                    "value": "silver",
                    "provenance": {"kind": "proposed", "origin": "cas-race-tooth"}
                }),
                expected_digest: None,
                idempotency_key: None,
            },
            &mut store,
        );
        assert!(added.ok, "staging succeeds: {added:?}");
        let lorestore = store.workspace_lorestore(&ws).expect("lorestore");
        let head_before = lorestore
            .head_ref()
            .expect("head read")
            .expect("main exists");
        let root_before = head_before.commit.clone();
        let ref_digest_before = head_before.digest().expect("ref digest");
        let workspace_before = semantic_workspace_state(&store, &ws);

        store.materialize_pre_cas_hook = Some(advance_main_ref_before_materialize_cas);
        let refused = crate::author::dispatch(
            &crate::Request {
                dli: "1.13.0".to_owned(),
                id: "materialize-race".to_owned(),
                op: crate::Op::AuthorMaterialize,
                params: json!({"workspace": ws, "rendered": false}),
                expected_digest: None,
                idempotency_key: None,
            },
            &mut store,
        );
        assert!(!refused.ok, "the injected CAS race refuses: {refused:?}");
        assert_eq!(
            refused.error.as_ref().map(|error| error.code),
            Some(crate::ErrorCode::StaleFence),
            "the prepared target's whole-ref fence names the race"
        );
        let head_after = store
            .workspace_lorestore(&ws)
            .expect("lorestore")
            .head_ref()
            .expect("head read")
            .expect("main exists");
        assert_eq!(head_after.commit, root_before);
        assert_ne!(
            head_after.digest().expect("ref digest"),
            ref_digest_before,
            "the injected valuation-only movement must change whole-ref authority"
        );
        assert_eq!(semantic_workspace_state(&store, &ws), workspace_before);
        assert_eq!(store.base_update_stats.prepared_installs, 0);
    }

    #[test]
    fn bulk_decision_storage_counts_are_constant_per_batch() {
        let (_root, mut store, ws, control) = batch_store();
        let response = crate::author::dispatch(&bulk_request(&ws, 1024), &mut store);
        assert!(response.ok, "bulk succeeds: {response:?}");
        // One Ratify carriage per record plus one Commit object; one branch CAS.
        assert_eq!(control.writes.load(Ordering::SeqCst), 1025);
        assert_eq!(control.cas.load(Ordering::SeqCst), 1);
        assert_eq!(store.base_update_stats.prepared_compiles, 1);
        assert_eq!(store.base_update_stats.prepared_installs, 1);
        assert_eq!(store.base_update_stats.reloads, 0);
    }

    /// A durable write failure after lowering restores every piece of mutable
    /// workspace state. The rollback may rebuild the prefix on this exceptional
    /// path; successful staging never does.
    #[test]
    fn durable_stage_failure_rolls_back_live_interner_and_counter() {
        let mut store = Store::default();
        let ws_id = workspace(&mut store);
        let base_key = store.workspace_bases[&ws_id].clone();
        store
            .lorestores
            .insert(base_key, Lorestore::from_storage(Box::new(WriteFails)));
        let before = store.get(&ws_id).expect("workspace");
        let names = before.interner.names().to_vec();
        let counter = before.bundle_counter;
        let hashes = before.staged_fact_hashes.clone();
        let digest = before.digest.clone();

        let error = store
            .stage_lorestore(&ws_id, &proposed_fact("", "one"))
            .expect_err("the injected write fails");
        assert!(error.to_string().contains("injected write failure"));

        let after = store.get(&ws_id).expect("workspace");
        assert_eq!(after.interner.names(), names);
        assert_eq!(after.bundle_counter, counter);
        assert_eq!(after.staged_fact_hashes, hashes);
        assert_eq!(after.digest, digest);
        assert!(after.decls.is_empty());
    }

    /// Mutating the live interner produces the exact names, counter, and object
    /// hashes the former reconstruct-lower-copy path produced.
    #[test]
    fn live_interner_lowering_is_byte_identical_to_reconstruction() {
        let root = tempfile::tempdir().expect("temporary lorestore");
        let mut store = Store::default();
        let ws_id = workspace(&mut store);
        let base_key = store.workspace_bases[&ws_id].clone();
        store.lorestores.insert(
            base_key,
            Lorestore::open(root.path().join("store")).expect("open lorestore"),
        );

        for (origin, value) in [("session-a", "one"), ("session-b", "two")] {
            let decl = proposed_fact(origin, value);
            let before = store.get(&ws_id).expect("workspace");
            let mut reference = dli_core::Interner::from_names(before.interner.names().to_vec());
            let mut reference_counter = before.bundle_counter;
            let objects = dli_lorestore::lower::lower_decl(
                &decl,
                &before.base_name,
                &mut reference,
                &mut reference_counter,
            );
            let expected_hashes = {
                let ls = store
                    .workspace_lorestore(&ws_id)
                    .expect("attached lorestore");
                objects
                    .iter()
                    .map(|object| ls.write_object(object).expect("reference write"))
                    .collect::<Vec<_>>()
            };

            store
                .stage_lorestore(&ws_id, &decl)
                .expect("live staging succeeds");
            let after = store.get(&ws_id).expect("workspace");
            assert_eq!(after.interner.names(), reference.names());
            assert_eq!(after.bundle_counter, reference_counter);
            assert_eq!(
                after
                    .staged_fact_hashes
                    .last()
                    .expect("staged hash group")
                    .1,
                expected_hashes
            );
        }
    }
}
