//! The `Lorestore` facade: durable, content-addressed persistence for fact
//! objects (DL-0041, roadmap §LS-2).
//!
//! A `Lorestore` wraps a [`Storage`] backend and provides the high-level
//! operations the rest of the system needs: writing fact objects, constructing
//! commits, managing refs, and walking the commit DAG. The storage layer
//! (loose objects, advisory locks, crash-safe writes) lives in [`crate::storage`];
//! this module composes it into a transactional commit model.
//!
//! ## The commit model
//!
//! Storage-level ancestry lives in the commit DAG, as in git — not in any fact
//! object's content hash (the three-way identity split, roadmap §LS-1). A
//! [`Commit`] names the fact objects it introduces, the lineage edges, the
//! evidence envelopes, and the event-variable interner (the name table that
//! makes `Atom(u32)` indices in support polynomials self-describing). The
//! `sha256:` digest of the head commit is the **workspace digest** — the
//! Merkle root over the entire workspace state.
//!
//! There is one ref: `main`. Forks-as-valuation-overlays are introduced in
//! LS-6 (the merge engine), not here. The MVP is single-branch.
//!
//! ## `state ≡ fold(log)`
//!
//! The store's state — the set of all objects reachable from the head ref — is
//! exactly the union of objects named by all commits in the chain from root to
//! head. This invariant is a gate-checked property test (see
//! `tests/state_fold_invariant.rs`), not a structural guarantee of the type
//! system.

use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::identity::ContentHash;
use crate::storage::{self, Storage, StoreError};
use crate::store::{Commit, Ref};

/// The name of the default branch ref.
pub const MAIN_REF: &str = "main";

/// The ref namespace for named branches (LS-6). `main` stays **unqualified**
/// for back-compat with LS-2 stores that predate branches; every other branch
/// is a ref under this prefix (`refs/heads/<name>`), reusing the storage layer's
/// existing support for `/` in ref names.
pub const HEADS_PREFIX: &str = "refs/heads/";

/// The storage ref name for a branch: `main` unqualified, any other branch under
/// [`HEADS_PREFIX`]. This is the one place the back-compat asymmetry lives.
#[must_use]
pub fn branch_ref_name(branch: &str) -> String {
    if branch == MAIN_REF {
        MAIN_REF.to_owned()
    } else {
        format!("{HEADS_PREFIX}{branch}")
    }
}

/// A durable lorestore: the append-only commit log over a [`Storage`] backend.
///
/// Construct with [`Lorestore::open`].
#[derive(Debug)]
pub struct Lorestore {
    storage: Box<dyn Storage>,
}

impl Lorestore {
    /// Open (or initialise) a store at `root`.
    ///
    /// If no `main` ref exists, a genesis (empty) commit is written and `main`
    /// is pointed at it. The returned `Lorestore` is ready to accept writes.
    pub fn open(root: PathBuf) -> Result<Self, StoreError> {
        let storage: Box<dyn Storage> = Box::new(crate::storage::FilesystemStorage::new(root));

        // The whole first-open sequence runs under the advisory lock: two
        // processes racing the genesis both observe-and-write atomically
        // (audit A11), and crash garbage is swept while no writer can be
        // mid-flight.
        {
            let _lock = storage.acquire_lock()?;
            let _ = storage.cleanup_orphans()?;
            let head = storage.load_ref(MAIN_REF)?;
            if head.is_none() {
                let genesis = Commit::root(
                    Vec::new(),    // empty interner at genesis
                    Vec::new(),    // no facts
                    Vec::new(),    // no lineage
                    Vec::new(),    // no evidence
                    Vec::new(),    // no decisions
                    String::new(), // no pack name at genesis
                );
                let genesis_hash = storage::write_typed(&*storage, &genesis)?;
                let genesis_ref = Ref::new(MAIN_REF, genesis_hash);
                let written = storage.cas_ref(MAIN_REF, None, &genesis_ref)?;
                // Under the lock a lost CAS can only mean another process
                // initialised the identical deterministic genesis between our
                // lock release and re-acquire paths — accept it.
                if !written && storage.load_ref(MAIN_REF)?.is_none() {
                    return Err(StoreError::Lock(
                        "genesis CAS failed on an empty store".to_owned(),
                    ));
                }
            }
        }

        Ok(Self { storage })
    }

    /// Construct a `Lorestore` from an already-opened `Storage` backend, without
    /// initialising the genesis commit.
    ///
    /// **Test / advanced use only.** The normal entry point is [`Lorestore::open`],
    /// which creates the genesis commit if absent. This constructor skips that
    /// step, allowing callers to wrap the storage with custom middleware (e.g.
    /// fault-injection wrappers for crash-safety testing).
    #[doc(hidden)]
    pub fn from_storage(storage: Box<dyn Storage>) -> Self {
        Self { storage }
    }

    /// Write a typed object to the store. Returns its content hash.
    ///
    /// Idempotent: if an object with the same hash already exists, the write
    /// is a no-op.
    pub fn write_object<T: Serialize>(&self, obj: &T) -> Result<ContentHash, StoreError> {
        storage::write_typed(&*self.storage, obj)
    }

    /// Read and deserialize a typed object from the store by content hash.
    pub fn read_object<T: DeserializeOwned + Serialize>(
        &self,
        hash: &ContentHash,
    ) -> Result<Option<T>, StoreError> {
        storage::read_typed(&*self.storage, hash)
    }

    /// The content hash of the current head commit, or `None` if the store is
    /// empty (which should never happen after `open`).
    ///
    /// This is the **workspace digest** — the Merkle root over the entire
    /// workspace state.
    pub fn root_hash(&self) -> Result<Option<ContentHash>, StoreError> {
        match self.storage.load_ref(MAIN_REF)? {
            Some(r) => Ok(Some(r.commit.clone())),
            None => Ok(None),
        }
    }

    /// Load the current head `Ref`.
    pub fn head_ref(&self) -> Result<Option<Ref>, StoreError> {
        self.storage.load_ref(MAIN_REF)
    }

    /// The head commit's interner name table — the table every fact in the
    /// chain resolves against, and the seed for any state that must extend it
    /// append-only (audit A6).
    pub fn head_interner(&self) -> Result<Vec<String>, StoreError> {
        let Some(head) = self.root_hash()? else {
            return Ok(Vec::new());
        };
        self.interner_at(&head)
    }

    /// The interner name table recorded on a given commit — the table the facts
    /// folded at that commit resolve their support polynomials against. An
    /// export at `at` must carry this alongside the objects; the indices are
    /// meaningless without it.
    pub fn interner_at(&self, at: &ContentHash) -> Result<Vec<String>, StoreError> {
        Ok(self.commit_at(at)?.interner)
    }

    /// The decision ids known at a given commit. Standing is read against this
    /// set — the same fact reads `decided d-1` where `d-1` is known and
    /// `proposed assumes d-1` where it is not — so an export carries it beside
    /// the objects and the interner.
    pub fn decisions_at(&self, at: &ContentHash) -> Result<Vec<String>, StoreError> {
        Ok(self.commit_at(at)?.decisions)
    }

    /// The pack name the chain is committed under, if it has a head. This is the
    /// lore scope, not the caller's document id: an append must carry it
    /// forward rather than rename the chain out from under its facts.
    pub fn head_pack_name(&self) -> Result<Option<String>, StoreError> {
        let Some(head) = self.root_hash()? else {
            return Ok(None);
        };
        // The genesis commit every store opens with is deliberately unnamed;
        // report that as "no name yet" rather than as an empty one.
        Ok(Some(self.commit_at(&head)?.pack_name).filter(|name| !name.is_empty()))
    }

    fn commit_at(&self, at: &ContentHash) -> Result<Commit, StoreError> {
        storage::read_typed(&*self.storage, at)?
            .ok_or_else(|| StoreError::NotFound(format!("commit missing: {at}")))
    }

    /// Atomically update the `main` ref from `expected` to `new`.
    ///
    /// `expected` is the current ref's whole-ref digest (`Ref::digest()`),
    /// covering the valuation overlay as well as the commit pointer.
    /// Acquires the store lock, so callers do not need to hold it separately.
    /// Returns `true` if the CAS succeeded, `false` if it lost the race.
    pub fn cas_main_ref(&self, expected: Option<&str>, new: &Ref) -> Result<bool, StoreError> {
        let _lock = self.storage.acquire_lock()?;
        self.storage.cas_ref(MAIN_REF, expected, new)
    }

    // -- branches (LS-6) -----------------------------------------------------

    /// The head commit of a branch, or `None` if the branch does not exist.
    pub fn branch_head(&self, branch: &str) -> Result<Option<ContentHash>, StoreError> {
        Ok(self
            .storage
            .load_ref(&branch_ref_name(branch))?
            .map(|r| r.commit))
    }

    /// The full [`Ref`] of a branch (commit pointer + valuation overlay).
    pub fn branch_ref(&self, branch: &str) -> Result<Option<Ref>, StoreError> {
        self.storage.load_ref(&branch_ref_name(branch))
    }

    /// Create a new branch `name` at the current head of `from` (default is
    /// `main`). The new branch inherits `from`'s commit **and** its valuation
    /// overlay — a branch is a divergent line of writes, and its point of
    /// departure is whatever the source ruled. Refuses if `name` already exists
    /// or `from` does not.
    ///
    /// Returns the commit the new branch points at.
    pub fn create_branch(&self, name: &str, from: &str) -> Result<ContentHash, StoreError> {
        let _lock = self.storage.acquire_lock()?;
        let ref_name = branch_ref_name(name);
        if self.storage.load_ref(&ref_name)?.is_some() {
            return Err(StoreError::BranchExists {
                name: name.to_owned(),
            });
        }
        let source = self
            .storage
            .load_ref(&branch_ref_name(from))?
            .ok_or_else(|| StoreError::NotFound(format!("branch `{from}` does not exist")))?;
        let new_ref =
            Ref::with_valuation(ref_name.clone(), source.commit.clone(), source.valuation);
        // `expected: None` means create-only; under the lock a lost race means a
        // concurrent create of the same name — surface it as BranchExists.
        if !self.storage.cas_ref(&ref_name, None, &new_ref)? {
            return Err(StoreError::BranchExists {
                name: name.to_owned(),
            });
        }
        Ok(source.commit)
    }

    /// Every branch name in the store, sorted with `main` first. Only `main` and
    /// `refs/heads/<name>` refs are branches; other refs (fork overlays, say) are
    /// not surfaced.
    pub fn list_branches(&self) -> Result<Vec<String>, StoreError> {
        let mut names = Vec::new();
        let mut has_main = false;
        for r in self.storage.list_refs()? {
            if r == MAIN_REF {
                has_main = true;
            } else if let Some(branch) = r.strip_prefix(HEADS_PREFIX) {
                names.push(branch.to_owned());
            }
        }
        names.sort();
        names.dedup();
        if has_main {
            names.insert(0, MAIN_REF.to_owned());
        }
        Ok(names)
    }

    /// Atomically update a branch ref from `expected` to `new` (the branch analog
    /// of [`cas_main_ref`](Lorestore::cas_main_ref)).
    pub fn cas_branch_ref(
        &self,
        branch: &str,
        expected: Option<&str>,
        new: &Ref,
    ) -> Result<bool, StoreError> {
        let _lock = self.storage.acquire_lock()?;
        self.storage
            .cas_ref(&branch_ref_name(branch), expected, new)
    }

    /// Commit a new `Commit` to the `main` branch.
    ///
    /// Verifies that every referenced fact, lineage-edge, and evidence-envelope
    /// object exists in the store before writing the commit. The new commit's
    /// parent is the current `main` head (CAS on `main`).
    ///
    /// Returns the content hash of the new commit.
    pub fn commit(
        &self,
        interner: Vec<String>,
        fact_hashes: Vec<ContentHash>,
        lineage: Vec<ContentHash>,
        evidence: Vec<ContentHash>,
        decisions: Vec<String>,
        pack_name: String,
    ) -> Result<ContentHash, StoreError> {
        self.commit_on_branch(
            MAIN_REF,
            interner,
            fact_hashes,
            lineage,
            evidence,
            decisions,
            pack_name,
        )
    }

    /// Commit a new `Commit` to an arbitrary branch (LS-6). The parent is that
    /// branch's current head; the CAS fences on that branch's whole-ref digest.
    /// The interner-append-only discipline is checked against the branch's own
    /// head — the same guard `commit` runs on `main`.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_on_branch(
        &self,
        branch: &str,
        interner: Vec<String>,
        fact_hashes: Vec<ContentHash>,
        lineage: Vec<ContentHash>,
        evidence: Vec<ContentHash>,
        decisions: Vec<String>,
        pack_name: String,
    ) -> Result<ContentHash, StoreError> {
        let _lock = self.storage.acquire_lock()?;
        let ref_name = branch_ref_name(branch);

        // Verify all referenced objects exist before writing the commit.
        for hash in &fact_hashes {
            if self.storage.read_object(hash)?.is_none() {
                return Err(StoreError::NotFound(format!(
                    "fact object not in store: {hash}"
                )));
            }
        }
        for hash in &lineage {
            if self.storage.read_object(hash)?.is_none() {
                return Err(StoreError::NotFound(format!(
                    "lineage edge not in store: {hash}"
                )));
            }
        }
        for hash in &evidence {
            if self.storage.read_object(hash)?.is_none() {
                return Err(StoreError::NotFound(format!(
                    "evidence envelope not in store: {hash}"
                )));
            }
        }

        // The parent is the branch's current head commit; the CAS token is the
        // whole head ref's digest (commit + valuation overlay).
        let head_ref = self.storage.load_ref(&ref_name)?;
        let expected: Option<String> = head_ref.as_ref().map(|r| r.commit.0.clone());
        let expected_token: Option<String> = match &head_ref {
            Some(r) => Some(r.digest()?.0),
            None => None,
        };
        let branch_valuation = head_ref.map(|r| r.valuation).unwrap_or_default();

        // Three fields on a commit replace, rather than add to, what the head
        // holds — and `reconstruct` reads all three off the head. A child that
        // changes any of them silently rewrites what earlier commits mean, so
        // they are checked together, here, at the one seam every write path
        // funnels through.
        if let Some(parent_hash) = &expected {
            let parent: Commit =
                storage::read_typed(&*self.storage, &ContentHash(parent_hash.clone()))?
                    .ok_or_else(|| {
                        StoreError::NotFound(format!("parent commit missing: {parent_hash}"))
                    })?;
            check_chain_invariants(&parent, &interner, &decisions, &pack_name)?;
        }

        let commit = match &expected {
            Some(parent_hash) => Commit::child(
                ContentHash(parent_hash.clone()),
                interner,
                fact_hashes,
                lineage,
                evidence,
                decisions,
                pack_name,
            ),
            None => Commit::root(
                interner,
                fact_hashes,
                lineage,
                evidence,
                decisions,
                pack_name,
            ),
        };

        let commit_hash = storage::write_typed(&*self.storage, &commit)?;

        // Build the new ref with the commit as its target, preserving the
        // branch's valuation overlay.
        let new_ref = match &expected {
            Some(_) => Ref::with_valuation(ref_name.clone(), commit_hash.clone(), branch_valuation),
            None => Ref::new(ref_name.clone(), commit_hash.clone()),
        };

        let written = self
            .storage
            .cas_ref(&ref_name, expected_token.as_deref(), &new_ref)?;
        if !written {
            return Err(StoreError::CasConflict {
                name: ref_name,
                expected,
                found: self
                    .storage
                    .load_ref(&branch_ref_name(branch))?
                    .map(|r| r.commit.0),
            });
        }

        Ok(commit_hash)
    }

    /// Walk the commit DAG from `tip` back to `base` (inclusive), returning
    /// each `Commit` in order (tip first, base last).
    pub fn walk_dag(
        &self,
        tip: &ContentHash,
        base: Option<&ContentHash>,
    ) -> Result<Vec<Commit>, StoreError> {
        self.storage.walk_dag(tip, base)
    }

    /// Collect all unique fact-object hashes named by commits in the chain,
    /// in **fold order: root first, head last** — reconstruction replays the
    /// log the way it was written, so declaration order (and with it interner
    /// index assignment) is preserved across multi-commit chains. Deduplicates
    /// (the same fact hash may appear in multiple commits) keeping the
    /// first-written occurrence.
    pub fn all_fact_hashes(&self) -> Result<Vec<ContentHash>, StoreError> {
        let Some(head) = self.root_hash()? else {
            return Ok(Vec::new());
        };
        self.fact_hashes_from(&head)
    }

    /// Collect the unique fact-object hashes reachable from `head` in **fold
    /// order (root first, head last)**. For a merge commit the walk follows both
    /// parents, so the returned set is the **union** of the two merged branches'
    /// facts — the polynomial join then happens for free on recompile
    /// (`PosBool::or_` re-joins alternative supports at the fixpoint). Dedup keeps
    /// the first-written occurrence, exactly as the single-branch path does.
    pub fn fact_hashes_from(&self, head: &ContentHash) -> Result<Vec<ContentHash>, StoreError> {
        // walk_dag returns HEAD → … → ROOT; the fold runs ROOT → … → HEAD.
        let mut chain = self.storage.walk_dag(head, None)?;
        chain.reverse();
        let mut seen = std::collections::BTreeSet::new();
        let mut unique = Vec::new();
        for c in chain {
            for h in c.facts {
                if seen.insert(h.clone()) {
                    unique.push(h);
                }
            }
        }
        Ok(unique)
    }

    /// The set of commit hashes reachable from `head` (inclusive), following both
    /// the linear parent and any merge parent.
    pub fn ancestors(
        &self,
        head: &ContentHash,
    ) -> Result<std::collections::BTreeSet<ContentHash>, StoreError> {
        let chain = self.storage.walk_dag(head, None)?;
        let mut set = std::collections::BTreeSet::new();
        for c in &chain {
            set.insert(c.content_hash()?);
        }
        Ok(set)
    }

    /// The **merge base** (lowest common ancestor) of two commits over the
    /// two-parent DAG, or `None` if they share no history. The LCA is the common
    /// ancestor that is not itself a proper ancestor of any other common
    /// ancestor. A criss-cross history can leave several; the smallest hash is
    /// chosen deterministically (an honest limitation for the first cut — simple
    /// branch-and-merge topologies have exactly one).
    pub fn merge_base(
        &self,
        a: &ContentHash,
        b: &ContentHash,
    ) -> Result<Option<ContentHash>, StoreError> {
        let anc_a = self.ancestors(a)?;
        let anc_b = self.ancestors(b)?;
        let common: Vec<ContentHash> = anc_a.intersection(&anc_b).cloned().collect();
        if common.is_empty() {
            return Ok(None);
        }
        // Precompute each common node's ancestor set, then keep only nodes that
        // no *other* common node has as a proper ancestor (the lowest ones).
        let mut lowest: Vec<ContentHash> = Vec::new();
        for c in &common {
            let dominated = common.iter().try_fold(false, |acc, c2| {
                if acc || c2 == c {
                    return Ok::<bool, StoreError>(acc);
                }
                // c is dominated if c2 is a descendant of c, i.e. c ∈ ancestors(c2).
                Ok(self.ancestors(c2)?.contains(c))
            })?;
            if !dominated {
                lowest.push(c.clone());
            }
        }
        lowest.sort();
        Ok(lowest.into_iter().next())
    }

    /// Import a compiled pack into the store: lower every `LoadedAtom` to a
    /// `FactObject`, persist each one, and write a new commit that names them
    /// all. The commit's `interner` is the compiled world's atom-name table
    /// (so support polynomials remain resolvable). The `main` ref advances to
    /// point at the new commit.
    ///
    /// Returns the new commit's content hash (= the workspace digest after this
    /// import).
    ///
    /// This is the **`.dli → lorestore`** bridge: it reuses `dli_world::compile`
    /// for the evaluation and `crate::lower::lower_atoms` for the object mapping.
    /// The stored fact objects are byte-identical to the compiled world model
    /// (the total-lowering oracle on LS-1 guarantees this).
    pub fn import_pack(&self, pack: &dli_world::Pack) -> Result<ContentHash, StoreError> {
        // 1 — lower the pack per-declaration (carriage objects interleaved in
        //     document order) and refuse it if the walk cannot track the
        //     loader.
        let (objects, interner) = Self::lower_pack_checked(pack)?;

        // 2 — persist the objects.
        let mut fact_hashes: Vec<ContentHash> = Vec::with_capacity(objects.len());
        for obj in &objects {
            let hash = self.write_object(obj)?;
            fact_hashes.push(hash);
        }

        // 3 — known decisions (append-only name list).
        let decisions = crate::lower::declared_decisions(pack).into_iter().collect();

        // 4 — commit, chaining from the current head.
        self.commit(
            interner,
            fact_hashes,
            Vec::new(),
            Vec::new(),
            decisions,
            pack.name.clone(),
        )
    }

    /// Lower a pack per-declaration and verify the walk's interner matches the
    /// compiled model's exactly — the loud-totality gate every import passes
    /// through. Returns the objects (carriage + atoms, document order) and the
    /// verified interner name table.
    fn lower_pack_checked(
        pack: &dli_world::Pack,
    ) -> Result<(Vec<crate::FactObject>, Vec<String>), StoreError> {
        let compiled = dli_world::compile(pack).map_err(|e| StoreError::Io {
            path: "compile".to_owned(),
            source: std::io::Error::other(e.to_string()),
        })?;
        let model_interner: Vec<String> = compiled.model().interner().names().to_vec();
        let (objects, walk_interner) = crate::lower::lower_pack(pack);
        if walk_interner != model_interner {
            let index = walk_interner
                .iter()
                .zip(model_interner.iter())
                .take_while(|(a, b)| a == b)
                .count();
            return Err(StoreError::LoweringDiverged {
                pack: pack.name.clone(),
                index,
                walk: walk_interner.get(index).cloned(),
                model: model_interner.get(index).cloned(),
            });
        }
        Ok((objects, model_interner))
    }

    /// Import a base pack **idempotently**: if the store's head state already
    /// covers the pack (every lowered fact object present, the pack's interner
    /// a prefix of the head interner), no commit is written and the current
    /// head hash is returned — so re-attaching at every serve start does not
    /// stack duplicate commits.
    ///
    /// A store whose head carries a different, non-empty lineage refuses the
    /// import (`ForeignBase`): a single-branch store holds exactly one pack
    /// lineage, and silently mixing two would desync the interner chain.
    pub fn ensure_pack_imported(&self, pack: &dli_world::Pack) -> Result<ContentHash, StoreError> {
        let (objects, interner) = Self::lower_pack_checked(pack)?;

        // Two hash domains, kept apart (roadmap §LS-9, schema-versioning):
        //
        // - **Storage address** — the full-object JCS hash, which INCLUDES
        //   `schema_version` (`object.rs`, no skip). This is what `write_object`
        //   returns and what a commit's fact list holds.
        // - **Identity projection** — `FactObject::content_hash`, which EXCLUDES
        //   `schema_version` (and `claim_id`). This is the version-independent
        //   fact identity.
        //
        // Idempotency must be judged on *identity*, not storage address: a store
        // persisted under an older schema (e.g. v1 string-bodied `rule`
        // carriages) has different storage addresses than the freshly-lowered v2
        // objects, even though the facts are identical. Comparing storage
        // addresses would refuse such a store as a `ForeignBase` at serve start —
        // defeating the upcast machinery's promise that old stores stay usable.
        // So we compare content hashes, reading the stored side through
        // `read_object` (which upcasts an old carriage to the current shape, whose
        // identity then equals the freshly-lowered one). The read cost is O(stored
        // facts) at attach time — acceptable at the atelier tier, which already
        // reads every object to compile.
        let mut pack_hashes: Vec<ContentHash> = Vec::with_capacity(objects.len());
        let mut pack_identities: std::collections::BTreeSet<ContentHash> =
            std::collections::BTreeSet::new();
        for obj in &objects {
            pack_hashes.push(crate::canonical::content_hash(obj)?);
            pack_identities.insert(obj.content_hash()?);
        }

        let head = self
            .root_hash()?
            .ok_or_else(|| StoreError::NotFound("store has no head commit".to_owned()))?;
        let chain = self.storage.walk_dag(&head, None)?;
        let head_commit = chain.first().expect("walk_dag returns at least one commit");

        // The stored side, read through the upcasting read path and projected to
        // its identity hash — a v1 carriage lifts to its v2 identity here.
        let mut existing_identities: std::collections::BTreeSet<ContentHash> =
            std::collections::BTreeSet::new();
        for hash in self.all_fact_hashes()? {
            if let Some(obj) = self.read_object::<crate::FactObject>(&hash)? {
                existing_identities.insert(obj.content_hash()?);
            }
        }
        let interner_covered = head_commit.interner.len() >= interner.len()
            && head_commit.interner[..interner.len()] == interner[..];
        if interner_covered
            && pack_identities
                .iter()
                .all(|h| existing_identities.contains(h))
        {
            return Ok(head);
        }

        // Not covered: only a fresh store (empty genesis lineage) may accept a
        // base import.
        if !existing_identities.is_empty() || !head_commit.interner.is_empty() {
            return Err(StoreError::ForeignBase {
                store_pack: head_commit.pack_name.clone(),
                importing: pack.name.clone(),
            });
        }

        for obj in &objects {
            self.write_object(obj)?;
        }
        let decisions = crate::lower::declared_decisions(pack).into_iter().collect();
        self.commit(
            interner,
            pack_hashes,
            Vec::new(),
            Vec::new(),
            decisions,
            pack.name.clone(),
        )
    }

    /// Read path: reconstruct a `CompiledWorld` from stored fact objects
    /// (roadmap §LS-3).
    ///
    /// Walks the commit chain from `main` back to the root, collecting the
    /// interner name table, known decisions, and all fact object hashes. Then
    /// reads the fact objects, reconstructs a `Pack` via `reconstruct_pack`,
    /// and compiles it through `dli_world::compile` — producing the identical
    /// `WorldModel`, findings, and digests as if the original `.dli` files had
    /// been loaded directly.
    ///
    /// This is the **`.dli → lorestore → .dli` round-trip**: the oracle for
    /// LS-3 is differential parity — the same corpus loaded from `.dli` and
    /// from its lorestore import compiles to identical results.
    pub fn compile(&self) -> Result<dli_world::CompiledWorld, StoreError> {
        let head = self
            .root_hash()?
            .ok_or_else(|| StoreError::NotFound("store has no head commit".to_owned()))?;
        self.compile_at(&head)
    }

    /// Reconstruct the `Pack` at an arbitrary `head` commit — walk the chain
    /// (both parents), collect the head's interner / decisions / pack name, read
    /// every reachable fact object, and rebuild the pack. Shared by
    /// [`compile_at`](Lorestore::compile_at) and the merge engine, which needs to
    /// reconstruct branch heads and the merge commit that are not `main`.
    pub fn reconstruct_at(&self, head: &ContentHash) -> Result<dli_world::Pack, StoreError> {
        let chain = self.storage.walk_dag(head, None)?;
        // walk_dag returns commits tip-first; the head carries the current
        // interner, decisions, and pack name (the most recent state).
        let head_commit = chain.first().expect("walk_dag returns at least one commit");

        let interner_names = &head_commit.interner;
        let known_decisions: std::collections::BTreeSet<String> =
            head_commit.decisions.iter().cloned().collect();

        let fact_hashes = self.fact_hashes_from(head)?;
        let mut fact_objects: Vec<crate::FactObject> = Vec::with_capacity(fact_hashes.len());
        for hash in &fact_hashes {
            let obj: crate::FactObject = self
                .read_object(hash)?
                .ok_or_else(|| StoreError::NotFound(format!("fact object missing: {hash}")))?;
            fact_objects.push(obj);
        }

        // A stream that cannot round-trip is a typed refusal, never a silently
        // different world.
        let interner = dli_core::Interner::from_names(interner_names.clone());
        crate::lower::reconstruct_pack(
            &fact_objects,
            &interner,
            &known_decisions,
            &head_commit.pack_name,
        )
        .map_err(StoreError::from)
    }

    /// Compile the world at an arbitrary `head` commit (LS-3 read path,
    /// generalized for LS-6 to any branch head or merge commit).
    ///
    /// This is the **`.dli → lorestore → .dli` round-trip**: the oracle for
    /// LS-3 is differential parity — the same corpus loaded from `.dli` and
    /// from its lorestore import compiles to identical results.
    pub fn compile_at(&self, head: &ContentHash) -> Result<dli_world::CompiledWorld, StoreError> {
        let pack = self.reconstruct_at(head)?;
        dli_world::compile(&pack).map_err(|e| StoreError::Io {
            path: "compile".to_owned(),
            source: std::io::Error::other(e.to_string()),
        })
    }

    /// Merge branch `source` into branch `target` (LS-6, the merge engine).
    ///
    /// The whole operation runs under the store lock. It reconciles the two
    /// heads' interners (refusing a merge whose branches minted colliding event
    /// variables — [`crate::merge::reconcile_interners`]), unions the reachable
    /// fact-hash sets, writes a **merge commit** (`parent` = target head,
    /// `merge_parent` = source head) naming that union, and advances `target` to
    /// it. The polynomial join happens for free on recompile: the union of two
    /// branches' facts re-joins alternative supports at the fixpoint.
    ///
    /// The fact-set union is confluent (CALM, roadmap §4), so the commit is
    /// always written; the returned [`MergeReport`](crate::merge::MergeReport)
    /// carries the structural conflicts (valuation disagreement, constraint
    /// violation, stratified-negation effect) for the sovereign to rule on.
    /// A variable the two branches ruled differently keeps the **target**'s value
    /// on the merged ref and is reported — never silently reconciled.
    pub fn merge(
        &self,
        target: &str,
        source: &str,
    ) -> Result<crate::merge::MergeReport, StoreError> {
        let _lock = self.storage.acquire_lock()?;

        let target_ref = self
            .storage
            .load_ref(&branch_ref_name(target))?
            .ok_or_else(|| StoreError::NotFound(format!("branch `{target}` does not exist")))?;
        let source_ref = self
            .storage
            .load_ref(&branch_ref_name(source))?
            .ok_or_else(|| StoreError::NotFound(format!("branch `{source}` does not exist")))?;
        let target_head = target_ref.commit.clone();
        let source_head = source_ref.commit.clone();

        let target_commit: Commit = storage::read_typed(&*self.storage, &target_head)?
            .ok_or_else(|| StoreError::NotFound(format!("target head missing: {target_head}")))?;
        let source_commit: Commit = storage::read_typed(&*self.storage, &source_head)?
            .ok_or_else(|| StoreError::NotFound(format!("source head missing: {source_head}")))?;

        // Interner reconciliation — prefix-preservation against BOTH parents.
        let merged_interner =
            crate::merge::reconcile_interners(&target_commit.interner, &source_commit.interner)?;

        // Union the reachable fact-hash sets (fold order: target first, then
        // source-only), so a linear fast-forward degenerates to the target set.
        let mut union = self.fact_hashes_from(&target_head)?;
        let mut seen: std::collections::BTreeSet<ContentHash> = union.iter().cloned().collect();
        for h in self.fact_hashes_from(&source_head)? {
            if seen.insert(h.clone()) {
                union.push(h);
            }
        }

        // Union of known decisions (append-only over target then source).
        let mut merged_decisions = target_commit.decisions.clone();
        let mut dseen: std::collections::BTreeSet<String> =
            merged_decisions.iter().cloned().collect();
        for d in &source_commit.decisions {
            if dseen.insert(d.clone()) {
                merged_decisions.push(d.clone());
            }
        }

        let base = self.merge_base(&target_head, &source_head)?;

        // Write the merge commit — its facts are already stored, so it is a pure
        // pointer object over the union.
        let merge_commit = Commit::merge(
            target_head.clone(),
            source_head.clone(),
            merged_interner.clone(),
            union,
            Vec::new(),
            Vec::new(),
            merged_decisions,
            target_commit.pack_name.clone(),
        );
        let merge_hash = storage::write_typed(&*self.storage, &merge_commit)?;

        // Advance the target branch to the merge commit, carrying the union
        // overlay (target wins on disagreement — reported, never silent).
        let merged_valuation =
            crate::merge::union_valuations(&target_ref.valuation, &source_ref.valuation);
        let new_ref = Ref::with_valuation(
            branch_ref_name(target),
            merge_hash.clone(),
            merged_valuation.clone(),
        );
        let expected_token = Some(target_ref.digest()?.0);
        if !self.storage.cas_ref(
            &branch_ref_name(target),
            expected_token.as_deref(),
            &new_ref,
        )? {
            return Err(StoreError::CasConflict {
                name: branch_ref_name(target),
                expected: Some(target_head.0.clone()),
                found: self
                    .storage
                    .load_ref(&branch_ref_name(target))?
                    .map(|r| r.commit.0),
            });
        }

        // Detect the three conflict classes over the merged state.
        let scan = crate::merge::scan(
            self,
            &merge_hash,
            &target_head,
            &source_head,
            base.as_ref(),
            &target_ref.valuation,
            &source_ref.valuation,
            &merged_valuation,
            &merged_interner,
        )?;

        Ok(crate::merge::MergeReport {
            merge_commit: merge_hash,
            merge_base: base,
            conflicts: scan.conflicts,
            findings: scan.findings,
            stratifies: scan.stratifies,
            standing_changes: scan.standing_changes,
        })
    }
}

/// The head-replaced fields, checked against the parent commit.
///
/// `compile()`/`reconstruct` resolve the whole chain against the **head**
/// commit's interner, decision set, and pack name — so these three are the ones
/// a child can change to alter what its ancestors mean. Each has its own shape:
///
/// - **interner** — prefix-preserving. Support polynomials are `±<index>` into
///   it, so dropping or reordering a name misresolves earlier facts.
/// - **decisions** — containment. Standing is read against the set, so dropping
///   one demotes every fact resting on it; order is meaningless (the merge path
///   unions them).
/// - **pack name** — equality, once the chain has a name. The scope and the
///   positional-proposal bundle prefix `h.<pack_name>#N` come from it, so a
///   rename re-renders committed provenance under a name it was never written
///   in. Genesis is deliberately unnamed, so the first commit names a chain
///   rather than renaming it.
///
/// Everything else a commit carries is additive (facts, lineage, evidence) or
/// structural (parents), and cannot rewrite an ancestor.
///
/// Both write paths above this seam preserve the name rather than relying on
/// the refusal: `dli.corpus.import` and `author.materialize` each take their
/// scope from the store's head when it has one, and pass their caller's name
/// only when seeding. The refusal is the backstop, not the mechanism — a
/// document id is a wire identifier decoupled from lore scope, so a differing
/// one is a supported configuration and must not fail.
fn check_chain_invariants(
    parent: &Commit,
    interner: &[String],
    decisions: &[String],
    pack_name: &str,
) -> Result<(), StoreError> {
    let diverges_at = parent
        .interner
        .iter()
        .zip(interner.iter())
        .take_while(|(a, b)| a == b)
        .count();
    if diverges_at < parent.interner.len() {
        return Err(StoreError::InternerRegression {
            parent_len: parent.interner.len(),
            diverges_at,
        });
    }
    if let Some(dropped) = parent
        .decisions
        .iter()
        .find(|known| !decisions.contains(*known))
    {
        return Err(StoreError::DecisionRegression {
            parent_len: parent.decisions.len(),
            dropped: dropped.clone(),
        });
    }
    // An unnamed parent is the genesis commit, which every store opens with:
    // the chain has no name yet, so the first real commit names it rather than
    // renaming it.
    if !parent.pack_name.is_empty() && pack_name != parent.pack_name {
        return Err(StoreError::PackRename {
            parent: parent.pack_name.clone(),
            candidate: pack_name.to_owned(),
        });
    }
    Ok(())
}
