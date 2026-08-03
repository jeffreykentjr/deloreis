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
use dli_core::valuation::Valuation;

/// The name of the default branch ref.
pub const MAIN_REF: &str = "main";

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
        let commit: Commit = storage::read_typed(&*self.storage, &head)?
            .ok_or_else(|| StoreError::NotFound(format!("head commit missing: {head}")))?;
        Ok(commit.interner)
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

    /// Commit a new `Commit` to the log.
    ///
    /// Verifies that every referenced fact, lineage-edge, and evidence-envelope
    /// object exists in the store before writing the commit. The new commit's
    /// parent is the current head (CAS on `main`).
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
        let _lock = self.storage.acquire_lock()?;

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

        // The parent is the current head commit; the CAS token is the whole
        // head ref's digest (commit + valuation overlay).
        let head_ref = self.storage.load_ref(MAIN_REF)?;
        let expected: Option<String> = head_ref.as_ref().map(|r| r.commit.0.clone());
        let expected_token: Option<String> = match &head_ref {
            Some(r) => Some(r.digest()?.0),
            None => None,
        };

        // The interner is append-only across the chain: `compile()` resolves
        // every commit's facts against the *head* interner, so a child whose
        // interner drops or reorders a parent name would silently misresolve
        // earlier facts' support polynomials. Refuse loudly instead.
        if let Some(parent_hash) = &expected {
            let parent: Commit =
                storage::read_typed(&*self.storage, &ContentHash(parent_hash.clone()))?
                    .ok_or_else(|| {
                        StoreError::NotFound(format!("parent commit missing: {parent_hash}"))
                    })?;
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

        // Build the new ref with the commit as its target.
        let new_ref = match &expected {
            Some(_) => Ref::with_valuation(
                MAIN_REF.to_owned(),
                commit_hash.clone(),
                self.current_valuation(),
            ),
            None => Ref::new(MAIN_REF, commit_hash.clone()),
        };

        let written = self
            .storage
            .cas_ref(MAIN_REF, expected_token.as_deref(), &new_ref)?;
        if !written {
            return Err(StoreError::CasConflict {
                name: MAIN_REF.to_string(),
                expected,
                found: self.storage.load_ref(MAIN_REF)?.map(|r| r.commit.0),
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
        // walk_dag returns HEAD → … → ROOT; the fold runs ROOT → … → HEAD.
        let mut chain = self.storage.walk_dag(&head, None)?;
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

    /// The current valuation overlay on the head ref.
    fn current_valuation(&self) -> Valuation {
        self.storage
            .load_ref(MAIN_REF)
            .ok()
            .flatten()
            .map(|r| r.valuation)
            .unwrap_or_default()
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
        // 1 — read the head commit; walk the chain to the root.
        let head = self
            .root_hash()?
            .ok_or_else(|| StoreError::NotFound("store has no head commit".to_owned()))?;
        let chain = self.storage.walk_dag(&head, None)?;
        // walk_dag returns commits ordered HEAD → … → ROOT; the head carries
        // the current interner, decisions, and pack name (the most recent state).
        let head_commit = chain.first().expect("walk_dag returns at least one commit");

        // 2 — collect the interner name table and known decisions from the head.
        let interner_names = &head_commit.interner;
        let known_decisions: std::collections::BTreeSet<String> =
            head_commit.decisions.iter().cloned().collect();

        // 3 — read all fact objects from the store.
        let fact_hashes = self.all_fact_hashes()?;
        let mut fact_objects: Vec<crate::FactObject> = Vec::with_capacity(fact_hashes.len());
        for hash in &fact_hashes {
            let obj: crate::FactObject = self
                .read_object(hash)?
                .ok_or_else(|| StoreError::NotFound(format!("fact object missing: {hash}")))?;
            fact_objects.push(obj);
        }

        // 4 — reconstruct the Pack; a stream that cannot round-trip is a
        //     typed refusal, never a silently different world.
        let interner = dli_core::Interner::from_names(interner_names.clone());
        let pack = crate::lower::reconstruct_pack(
            &fact_objects,
            &interner,
            &known_decisions,
            &head_commit.pack_name,
        )?;

        // 5 — compile via the existing path.
        dli_world::compile(&pack).map_err(|e| StoreError::Io {
            path: "compile".to_owned(),
            source: std::io::Error::other(e.to_string()),
        })
    }
}
