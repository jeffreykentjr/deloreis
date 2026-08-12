//! The authoring seam's store (`AUTH-1`): each workspace bound to one loaded document base.
//!
//! A project store owns one compiled base per logical document. `author.open`
//! resolves a scope or declared fork to exactly one of those bases and records
//! that ownership for every subsequent read and mutation. The fallback base
//! exists only for focused unit tests and explicit demo fixtures.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use dli_ast::decl::{Decl, Document};
use dli_core::valuation::Valuation;
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
    valuation: Valuation,
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
            let pack = ls
                .reconstruct_at(&head)
                .map_err(|error| OpenError::BranchCompile(error.to_string()))?;
            let mut world = dli_world::compile(&pack)
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
            workspace.interner_names = branch_names;
            workspace.bundle_counter = bundle_counter;
            workspace.base_root_hash = Some(head.to_string());
            workspace.base_valuation = branch_ref.valuation.clone();
            workspace.base_ref_digest = branch_ref.digest().ok().map(|digest| digest.0);
            workspace.target_branch = Some(branch_name);
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
        self.next_id += 1;
        let id = format!("ws-{}", self.next_id);
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
        workspace.interner_names = base_names;
        workspace.bundle_counter = bundle_counter;
        // Pin the project-selected commit and valuation together. For local
        // demo stores no snapshot exists, preserving their previous behavior.
        if let Some(snapshot) = self.snapshots.get(&base_key) {
            workspace.base_root_hash = Some(snapshot.commit.to_string());
            workspace.base_valuation = snapshot.valuation.clone();
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
    pub fn register_store_origin(
        &mut self,
        key: String,
        origin: std::path::PathBuf,
        pinned: bool,
        commit: dli_lorestore::ContentHash,
        valuation: Valuation,
        ref_digest: Option<String>,
    ) {
        self.origins.insert(key.clone(), origin);
        if let Some(base) = self.bases.get_mut(&key) {
            for (atom, value) in &valuation.0 {
                base.world.model.world.valuation.0.insert(*atom, *value);
            }
        }
        self.snapshots.insert(
            key.clone(),
            StoreSnapshot {
                commit,
                valuation,
                ref_digest,
            },
        );
        if pinned {
            self.pinned_bases.insert(key);
        }
    }

    /// Whether this base is served at a manifest-pinned commit.
    #[must_use]
    pub fn is_pinned(&self, key: &str) -> bool {
        self.pinned_bases.contains(key)
    }

    /// LS-4b: lower a staged decl to `FactObject`s and write them to the
    /// lorestore immediately (durable staging). Updates the workspace's
    /// `staged_fact_hashes`, `interner_names`, and `bundle_counter` in place.
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
        let pack_name = ws.base_name.clone();
        let interner_names = ws.interner_names.clone();
        let bundle_counter = ws.bundle_counter;

        let Some(ls) = self.lorestores.get(&base_key) else {
            return Ok(());
        };
        let mut interner = dli_core::atom::Interner::from_names(interner_names);
        let mut counter = bundle_counter;
        let objects =
            dli_lorestore::lower::lower_decl(decl, &pack_name, &mut interner, &mut counter);

        let mut hashes = Vec::with_capacity(objects.len());
        for fo in &objects {
            hashes.push(ls.write_object(fo)?);
        }
        let key = super::tools::decl_key(decl);
        let ws = self
            .workspaces
            .get_mut(ws_id)
            .expect("workspace existed above");
        ws.staged_fact_hashes.push((key, hashes));
        ws.interner_names = interner.names().to_vec();
        ws.bundle_counter = counter;
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
