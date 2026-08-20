//! The authoring seam's workspace (`AUTH-1`): the in-memory staged layer over a compiled base.
//!
//! Nothing an agent does touches disk and nothing changes canon (spec 11 §2). A
//! [`Workspace`] is a staged layer of typed AST declarations over an unchanged
//! compiled base, its fork-local suppositions, an idempotency replay cache, and
//! the current digest fence. The fence is a content hash over the staged state
//! ([`digest`](crate::author::digest)), so it moves exactly when the staged
//! state moves.
//!
//! The fence is resealed **incrementally** (R3/H4/B4). The digest hashes one
//! canonical byte string — `{"base_name":…,"decls":[…],"mode":…,"suppositions":…}`
//! — but reseal no longer re-renders and re-hashes the whole string on every
//! mutation (the `O(n²)` a full build would cost). Instead the workspace holds a
//! streaming SHA-256 `prefix` that has already absorbed the fixed head and every
//! declaration's JSON in order, up to but not including the `]` that closes the
//! declarations array. Staging one declaration feeds only that declaration's
//! bytes into `prefix` (`O(1)` in the declaration count), so a full `n`-declaration
//! build is linear. `reseal` clones `prefix`, feeds the small tail
//! (`],"suppositions":…}`), and finalizes — `O(1)` in the declaration count. A
//! replace or discard, both rare, re-absorb the head and declarations once.
//!
//! Because the absorbed bytes are exactly the string the old whole-snapshot reseal
//! hashed, the digest **value is unchanged** — the optimization is invisible on
//! the wire, and every recorded fence and fixture still matches.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use sha2::{Digest, Sha256};

use dli_ast::decl::{Decl, Document};
use dli_ast::term::Term;
use dli_core::ann::Status;
use dli_core::atom::Interner;
use dli_core::valuation::{NamedAtomSet, NamedValuation};
use dli_world::CompiledWorld;

use crate::author::digest::digest_hash;

/// Which authoring door a workspace was opened in (spec 11 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// `{"scope": …}` — stage over an existing scope (a pack, in DL-0016's
    /// renamed surface); additions accrue to that scope's surface.
    Scope,
    /// `{"fork": …}` — a fork-scoped workspace; hypotheses and suppositions are
    /// fork-local and never leak into canon (koan 2).
    Fork,
}

/// One authoring workspace.
#[derive(Debug, Clone)]
pub struct Workspace {
    /// The workspace id (`ws-<n>`).
    pub id: String,
    /// The door the workspace was opened in.
    pub mode: Mode,
    /// The pack or fork name given at open — the base the staging is over, and
    /// the `scope` header `author.materialize` emits.
    pub base_name: String,
    /// The staged declarations, in the order they were added (typed, AST-level).
    /// Mutate only through [`push_decl`](Workspace::push_decl),
    /// [`replace_decl`](Workspace::replace_decl), and
    /// [`remove_decl`](Workspace::remove_decl), which keep the digest cache in
    /// step.
    pub decls: Vec<Decl>,
    /// Whether each declaration is already represented by the installed durable
    /// base. Kept parallel to `decls` so the public workspace digest/history can
    /// remain stable across materialize while structural reads compile only the
    /// still-pending suffix/delta.
    materialized_decls: Vec<bool>,
    /// The hypothesis ids supposed true within the fork (spec 11 §3.9).
    pub suppositions: Vec<String>,
    /// The current digest fence.
    pub digest: String,
    /// Fact object hashes written to the lorestore for staged declarations
    /// (LS-4). Populated on each `author.add_*` call; consumed by
    /// `author.materialize` when committing to `main`.
    /// One entry per durably staged declaration: the declaration's canonical
    /// key (its rendered line — the same key `author.discard` addresses) and
    /// the fact-object hashes it staged. A per-declaration reject removes its
    /// group, so a rejected claim never reaches a commit.
    pub staged_fact_hashes: Vec<(String, Vec<dli_lorestore::identity::ContentHash>)>,
    /// The live interner seeded from the base pack and grown by per-declaration
    /// lowering. Keeping the reverse index alive avoids rebuilding it from the
    /// complete name list on every staged mutation.
    pub interner: Interner,
    /// The bundle counter for staged proposal-bundle variables — continues
    /// from the base pack's count so `h.<pack>#N` indices are stable.
    pub bundle_counter: u32,
    /// The lorestore commit this workspace is relative to — pinned at open,
    /// advanced only by this workspace's own `materialize`, never the moving
    /// global head (audit B8). `None` without a lorestore.
    pub base_root_hash: Option<String>,
    /// The persisted valuation paired with [`base_root_hash`](Self::base_root_hash)
    /// when the workspace opened. It is applied before local suppositions.
    pub base_valuation: NamedValuation,
    /// Stored decisions whose compiler-default assignment is explicitly open,
    /// keyed by stable name so recompilation cannot retarget them.
    pub base_unassigned: NamedAtomSet,
    /// Whole-ref fence for the branch the workspace writes to. Branch-mode
    /// workspaces inherit their project snapshot; pinned replays capture the
    /// live origin ref at open while continuing to serve their pinned world.
    pub base_ref_digest: Option<String>,
    /// The named branch this workspace was opened against and writes to
    /// (WS-3, `author.open`'s `branch` param, ≥ 1.7.0). `None` is the
    /// pre-1.7.0 shape: every write routes to `main`, exactly as before this
    /// field existed. Absorbed into the digest fence ONLY when `Some` (see
    /// [`reseal`](Workspace::reseal)), so a branchless workspace's digest is
    /// byte-identical to every fixture recorded before this field was added.
    pub target_branch: Option<String>,
    /// Idempotency replay cache: key → (payload fingerprint, the exact result
    /// value first produced). The fingerprint guards key reuse: the same key with
    /// a different payload is a conflict, never a silent cached replay (R3/M9).
    pub idempotency: BTreeMap<String, (String, serde_json::Value)>,
    /// Derived resolution state for add-time and aggregate findings. It is not
    /// part of the workspace digest or any wire/storage shape.
    resolution: ResolutionIndex,
    /// Monotonic identity of the source declarations compiled by staged reads.
    /// Dynamic valuation, grounded attacks, explicit unassignment, ref movement,
    /// and suppositions deliberately do not participate.
    structural_revision: u64,
    /// One bounded structural compile per workspace. Reads clone the cached
    /// world before applying request-local standing and suppositions.
    compile_cache: WorkspaceCompileCache,
    /// Deterministic request-work counters used by cache acceptance and release
    /// profiling. They are observational only and never enter a verdict.
    read_work: ReadWorkCounters,
    /// The streaming digest state: a SHA-256 that has absorbed the canonical head
    /// (`{"base_name":…,"decls":[`) and every staged declaration's JCS bytes in
    /// order, up to but not including the closing `]`. An append feeds one more
    /// declaration; [`reseal`](Workspace::reseal) clones this and finishes the tail,
    /// so neither cost grows with the declaration count.
    prefix: Sha256,
}

impl Workspace {
    /// Open an empty workspace with its initial digest sealed.
    #[must_use]
    pub fn new(id: String, mode: Mode, base_name: String) -> Self {
        let mut workspace = Self {
            prefix: Sha256::new(),
            id,
            mode,
            base_name,
            decls: Vec::new(),
            materialized_decls: Vec::new(),
            suppositions: Vec::new(),
            digest: String::new(),
            idempotency: BTreeMap::new(),
            staged_fact_hashes: Vec::new(),
            interner: Interner::new(),
            bundle_counter: 0,
            base_root_hash: None,
            base_valuation: NamedValuation::default(),
            base_unassigned: NamedAtomSet::default(),
            base_ref_digest: None,
            target_branch: None,
            resolution: ResolutionIndex::default(),
            structural_revision: 0,
            compile_cache: WorkspaceCompileCache::default(),
            read_work: ReadWorkCounters::default(),
        };
        workspace.absorb_head();
        workspace.reseal();
        workspace
    }

    /// Stage a declaration, feeding its canonical JSON into the streaming prefix
    /// and resealing. Append is `O(1)` in the declaration count.
    pub fn push_decl(&mut self, decl: Decl) {
        // The declarations array joins its members with commas, so every member
        // past the first carries a leading separator.
        if !self.decls.is_empty() {
            self.prefix.update(b",");
        }
        self.prefix.update(encode(&decl).as_bytes());
        self.resolution.add_staged(&decl);
        self.decls.push(decl);
        self.materialized_decls.push(false);
        self.bump_structural_revision();
        self.reseal();
    }

    /// Replace the declaration at `index` (e.g. a question that gained an option)
    /// and reseal. The prefix is re-absorbed once, since the change lands mid-array.
    pub fn replace_decl(&mut self, index: usize, decl: Decl) {
        if let Some(slot) = self.decls.get_mut(index) {
            *slot = decl;
            if let Some(materialized) = self.materialized_decls.get_mut(index) {
                *materialized = false;
            }
            self.resolution.rebuild_staged(&self.decls);
            self.bump_structural_revision();
            self.reabsorb();
            self.reseal();
        }
    }

    /// Discard the single staged declaration at `index`, returning it, and reseal.
    pub fn remove_decl(&mut self, index: usize) -> Option<Decl> {
        if index >= self.decls.len() {
            return None;
        }
        let removed = self.decls.remove(index);
        self.materialized_decls.remove(index);
        self.resolution.rebuild_staged(&self.decls);
        self.bump_structural_revision();
        self.reabsorb();
        self.reseal();
        Some(removed)
    }

    /// Replace the base half of the derived resolution index, then replay the
    /// staged declarations over it. Used at open and after a branch/base reload.
    pub fn refresh_resolution(&mut self, base: &CompiledWorld, documents: &[Document]) {
        self.resolution.rebase(base, documents, &self.decls);
        // Conditional-attack commits may introduce a Ratify carriage solely to
        // establish an attack endpoint. Those decision variables are dynamic
        // standing inputs, not symbolic declarations worth recompiling. The
        // durable named overlay is nevertheless sufficient referential proof,
        // so keep them in the resolution census by exact name.
        self.resolution.known_decisions.extend(
            self.base_valuation
                .0
                .keys()
                .chain(self.base_unassigned.0.iter())
                .cloned(),
        );
    }

    /// Refresh derived resolution state and invalidate the symbolic compile
    /// because the base documents or killed-token guardrails changed.
    pub fn refresh_structure(&mut self, base: &CompiledWorld, documents: &[Document]) {
        self.refresh_resolution(base, documents);
        self.bump_structural_revision();
    }

    /// Declarations not yet represented by the installed durable base.
    pub(super) fn pending_decls(&self) -> Vec<Decl> {
        self.decls
            .iter()
            .zip(&self.materialized_decls)
            .filter_map(|(decl, materialized)| (!materialized).then_some(decl.clone()))
            .collect()
    }

    /// Reconcile every currently staged durable-object group into the newly
    /// installed base without changing the caller's digest/history surface.
    /// Structural invalidation is owned by the base install and occurs once.
    pub(super) fn mark_materialized(&mut self, keys: &BTreeSet<String>) {
        for (decl, materialized) in self.decls.iter().zip(&mut self.materialized_decls) {
            if keys.contains(&crate::author::tools::decl_key(decl)) {
                *materialized = true;
            }
        }
    }

    /// Return the cached structural world for this declaration revision.
    /// Counters are diagnostic only and use relaxed ordering: no verdict relies
    /// on an inter-thread ordering between observations.
    pub(super) fn cached_structural_world(&self) -> Option<Arc<CompiledWorld>> {
        if !self.compile_cache.enabled.load(Ordering::Relaxed) {
            self.compile_cache.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        let cached = self
            .compile_cache
            .entry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = cached.as_ref()
            && entry.revision == self.structural_revision
        {
            self.compile_cache.hits.fetch_add(1, Ordering::Relaxed);
            return Some(Arc::clone(&entry.world));
        }
        self.compile_cache.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Install a successful structural compile for subsequent reads. Racing
    /// identical misses may compile twice; the first matching entry wins and
    /// both worlds have already passed the same deterministic compiler.
    pub(super) fn cache_structural_world(&self, world: CompiledWorld) {
        self.compile_cache.compiles.fetch_add(1, Ordering::Relaxed);
        if !self.compile_cache.enabled.load(Ordering::Relaxed) {
            return;
        }
        let mut cached = self
            .compile_cache
            .entry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cached
            .as_ref()
            .is_some_and(|entry| entry.revision == self.structural_revision)
        {
            return;
        }
        if cached.is_some() {
            self.compile_cache.evictions.fetch_add(1, Ordering::Relaxed);
        }
        *cached = Some(CachedStructuralWorld {
            revision: self.structural_revision,
            world: Arc::new(world),
        });
    }

    /// Deterministic diagnostic counts for the request-side symbolic cache.
    #[must_use]
    pub fn compile_cache_stats(&self) -> CompileCacheStats {
        CompileCacheStats {
            compiles: self.compile_cache.compiles.load(Ordering::Relaxed),
            hits: self.compile_cache.hits.load(Ordering::Relaxed),
            misses: self.compile_cache.misses.load(Ordering::Relaxed),
            evictions: self.compile_cache.evictions.load(Ordering::Relaxed),
        }
    }

    /// Deterministic work counts for dynamic read overlays and explanations.
    #[must_use]
    pub fn read_work_stats(&self) -> ReadWorkStats {
        self.read_work.snapshot()
    }

    pub(super) fn note_reconstruction(&self) {
        self.read_work
            .reconstructions
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn note_loader_and_seal(&self) {
        self.read_work.loader_runs.fetch_add(1, Ordering::Relaxed);
        self.read_work
            .fixpoint_seals
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn note_valuation(&self, copied: u64, suppositions: u64) {
        self.read_work.valuations.fetch_add(1, Ordering::Relaxed);
        self.read_work
            .valuation_copies
            .fetch_add(copied, Ordering::Relaxed);
        self.read_work
            .supposition_probes
            .fetch_add(suppositions, Ordering::Relaxed);
    }

    pub(super) fn note_query(&self) {
        self.read_work.queries.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn note_why(&self) {
        self.read_work.whys.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn note_attack_explanation(&self, endpoints: u64, incidences: u64, copied: u64) {
        self.read_work
            .attack_endpoint_resolutions
            .fetch_add(endpoints, Ordering::Relaxed);
        self.read_work
            .attack_incidence_visits
            .fetch_add(incidences, Ordering::Relaxed);
        self.read_work
            .valuation_copies
            .fetch_add(copied, Ordering::Relaxed);
    }

    /// Force cache bypass for deterministic differential tests.
    #[doc(hidden)]
    pub fn set_compile_cache_enabled(&self, enabled: bool) {
        self.compile_cache.enabled.store(enabled, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub(super) fn corrupt_compile_cache(&self) {
        let mut cached = self
            .compile_cache
            .entry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *cached = Some(CachedStructuralWorld {
            revision: self.structural_revision,
            world: Arc::new(CompiledWorld::default()),
        });
    }

    fn bump_structural_revision(&mut self) {
        self.structural_revision = self.structural_revision.wrapping_add(1);
        if self.structural_revision == 0 {
            let mut cached = self
                .compile_cache
                .entry
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *cached = None;
        }
    }

    /// Every entity declared or observed in the selected base or staged layer.
    #[must_use]
    pub fn known_entities(&self) -> &BTreeSet<String> {
        &self.resolution.known_entities
    }

    /// The current entity sort: latest staged declaration first, otherwise the
    /// first matching entity entry in the compiled base (the pre-index semantics).
    #[must_use]
    pub fn entity_sort(&self, name: &str) -> Option<&str> {
        self.resolution
            .entity_sorts
            .get(name)
            .and_then(|sort| sort.as_deref())
    }

    /// Sorted entity names that have at least one declaration of `kind` in the
    /// base or staged layer. Duplicate declarations of different kinds remain in
    /// both sets, matching the former union scan used for repair alternatives.
    #[must_use]
    pub fn entities_of_kind(&self, kind: &str) -> Vec<String> {
        self.resolution
            .entities_by_kind
            .get(kind)
            .map(|names| names.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Decision ids declared by the selected base or staged layer.
    #[must_use]
    pub fn known_decisions(&self) -> &BTreeSet<String> {
        &self.resolution.known_decisions
    }

    /// The base's distinct certain values for one `(predicate, subject)` slot.
    /// Staged values deliberately do not participate: this is the same base-only
    /// contradiction check the former atom scan implemented.
    #[must_use]
    pub fn certain_base_values(&self, pred: &str, subject: &str) -> Option<&BTreeSet<String>> {
        self.resolution.certain_base_values.get(pred)?.get(subject)
    }

    /// Recompute the digest by finishing the streamed material: clone the prefix
    /// (the head and every declaration, already absorbed), feed the tail
    /// (`],"mode":…,"suppositions":…}`), and finalize. `O(1)` in the declaration
    /// count.
    ///
    /// The absorbed-plus-tail bytes are exactly
    /// `{"base_name":…,"decls":[…],"mode":…,"suppositions":…}`, the JCS string the
    /// whole-snapshot reseal hashed, so the digest value is unchanged — UNLESS
    /// [`target_branch`](Self::target_branch) is `Some`, in which case one more
    /// key is appended (`"target_branch"` sorts after `"suppositions"` in JCS
    /// key order, so it belongs at the very end of the tail). A `None`
    /// `target_branch` emits nothing here, which is what keeps a branchless
    /// workspace's digest byte-identical to every fixture recorded before this
    /// field existed.
    pub fn reseal(&mut self) {
        let mut hasher = self.prefix.clone();
        hasher.update(b"],\"mode\":");
        hasher.update(encode(&self.mode).as_bytes());
        hasher.update(b",\"suppositions\":");
        hasher.update(encode(&self.suppositions).as_bytes());
        if let Some(branch) = &self.target_branch {
            hasher.update(b",\"target_branch\":");
            hasher.update(encode(branch).as_bytes());
        }
        hasher.update(b"}");
        self.digest = digest_hash(hasher.finalize().as_slice());
    }

    /// Seed the streaming prefix with the fixed head that precedes the first
    /// declaration: `{"base_name":<base>,"decls":[`. This is JCS key order;
    /// `mode` and `suppositions` follow the declarations array in the tail.
    fn absorb_head(&mut self) {
        self.prefix.update(b"{\"base_name\":");
        self.prefix.update(encode(&self.base_name).as_bytes());
        self.prefix.update(b",\"decls\":[");
    }

    /// Rebuild the streaming prefix from scratch — the head then every staged
    /// declaration in order — after a mid-array edit that the append-only stream
    /// cannot patch in place. Linear in the declaration count; the mutating calls
    /// that need it (replace, discard) are rare.
    fn reabsorb(&mut self) {
        self.prefix = Sha256::new();
        self.absorb_head();
        for (i, decl) in self.decls.iter().enumerate() {
            if i > 0 {
                self.prefix.update(b",");
            }
            self.prefix.update(encode(decl).as_bytes());
        }
    }
}

#[derive(Debug, Clone)]
struct CachedStructuralWorld {
    revision: u64,
    world: Arc<CompiledWorld>,
}

#[derive(Debug)]
struct WorkspaceCompileCache {
    entry: Mutex<Option<CachedStructuralWorld>>,
    enabled: AtomicBool,
    compiles: AtomicU64,
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

impl Default for WorkspaceCompileCache {
    fn default() -> Self {
        Self {
            entry: Mutex::new(None),
            enabled: AtomicBool::new(true),
            compiles: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }
}

impl Clone for WorkspaceCompileCache {
    fn clone(&self) -> Self {
        let entry = self
            .entry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        Self {
            entry: Mutex::new(entry),
            enabled: AtomicBool::new(self.enabled.load(Ordering::Relaxed)),
            compiles: AtomicU64::new(self.compiles.load(Ordering::Relaxed)),
            hits: AtomicU64::new(self.hits.load(Ordering::Relaxed)),
            misses: AtomicU64::new(self.misses.load(Ordering::Relaxed)),
            evictions: AtomicU64::new(self.evictions.load(Ordering::Relaxed)),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Deterministic counts for one workspace's bounded structural-compile cache.
pub struct CompileCacheStats {
    /// Successful symbolic compiler invocations, including racing duplicates.
    pub compiles: u64,
    /// Reads served from the matching structural entry.
    pub hits: u64,
    /// Reads that had to invoke the compiler (or deliberately bypassed it).
    pub misses: u64,
    /// Stale structural entries replaced after an invalidating mutation.
    pub evictions: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Deterministic phase/work counts for one workspace's author reads.
pub struct ReadWorkStats {
    /// Structural document reconstructions performed on cache misses.
    pub reconstructions: u64,
    /// Loader phase invocations.
    pub loader_runs: u64,
    /// Fixpoint-and-seal phase invocations paired with loader runs.
    pub fixpoint_seals: u64,
    /// Request-local dynamic valuation overlays applied.
    pub valuations: u64,
    /// `author.query` calls observed.
    pub queries: u64,
    /// `author.why` calls observed.
    pub whys: u64,
    /// Full target-interner name scans on the overlay hit path.
    pub full_name_scans: u64,
    /// Temporary target remap maps built on the overlay hit path.
    pub remap_builds: u64,
    /// Exact-name supposition lookups.
    pub supposition_probes: u64,
    /// Exact-name attack endpoint lookups.
    pub attack_endpoint_resolutions: u64,
    /// Active target-to-support-path incidences visited.
    pub attack_incidence_visits: u64,
    /// Request-bounded valuation copies, including the one read clone and the
    /// optional one shared explanation counterfactual.
    pub valuation_copies: u64,
}

#[derive(Debug, Default)]
struct ReadWorkCounters {
    reconstructions: AtomicU64,
    loader_runs: AtomicU64,
    fixpoint_seals: AtomicU64,
    valuations: AtomicU64,
    queries: AtomicU64,
    whys: AtomicU64,
    supposition_probes: AtomicU64,
    attack_endpoint_resolutions: AtomicU64,
    attack_incidence_visits: AtomicU64,
    valuation_copies: AtomicU64,
}

impl ReadWorkCounters {
    fn snapshot(&self) -> ReadWorkStats {
        ReadWorkStats {
            reconstructions: self.reconstructions.load(Ordering::Relaxed),
            loader_runs: self.loader_runs.load(Ordering::Relaxed),
            fixpoint_seals: self.fixpoint_seals.load(Ordering::Relaxed),
            valuations: self.valuations.load(Ordering::Relaxed),
            queries: self.queries.load(Ordering::Relaxed),
            whys: self.whys.load(Ordering::Relaxed),
            // The implementation has no operation that performs either form of
            // work; keeping explicit zeros makes regressions an asserted tooth.
            full_name_scans: 0,
            remap_builds: 0,
            supposition_probes: self.supposition_probes.load(Ordering::Relaxed),
            attack_endpoint_resolutions: self.attack_endpoint_resolutions.load(Ordering::Relaxed),
            attack_incidence_visits: self.attack_incidence_visits.load(Ordering::Relaxed),
            valuation_copies: self.valuation_copies.load(Ordering::Relaxed),
        }
    }
}

impl Clone for ReadWorkCounters {
    fn clone(&self) -> Self {
        let stats = self.snapshot();
        Self {
            reconstructions: AtomicU64::new(stats.reconstructions),
            loader_runs: AtomicU64::new(stats.loader_runs),
            fixpoint_seals: AtomicU64::new(stats.fixpoint_seals),
            valuations: AtomicU64::new(stats.valuations),
            queries: AtomicU64::new(stats.queries),
            whys: AtomicU64::new(stats.whys),
            supposition_probes: AtomicU64::new(stats.supposition_probes),
            attack_endpoint_resolutions: AtomicU64::new(stats.attack_endpoint_resolutions),
            attack_incidence_visits: AtomicU64::new(stats.attack_incidence_visits),
            valuation_copies: AtomicU64::new(stats.valuation_copies),
        }
    }
}

/// A rebuildable, non-authoritative census used by the authoring seam's
/// referential checks. Base state is retained separately so the rare mid-array
/// edit can rebuild only the staged overlay without rescanning the compiled
/// world. Ordered collections preserve finding and repair ordering.
#[derive(Debug, Clone, Default)]
struct ResolutionIndex {
    base_entities: BTreeSet<String>,
    base_entity_sorts: BTreeMap<String, Option<String>>,
    base_entities_by_kind: BTreeMap<String, BTreeSet<String>>,
    base_decisions: BTreeSet<String>,
    known_entities: BTreeSet<String>,
    entity_sorts: BTreeMap<String, Option<String>>,
    entities_by_kind: BTreeMap<String, BTreeSet<String>>,
    known_decisions: BTreeSet<String>,
    certain_base_values: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    #[cfg(test)]
    staged_rebuild_visits: usize,
    #[cfg(test)]
    base_atom_visits: usize,
}

impl ResolutionIndex {
    fn rebase(&mut self, base: &CompiledWorld, documents: &[Document], decls: &[Decl]) {
        self.base_entities.clear();
        self.base_entity_sorts.clear();
        self.base_entities_by_kind.clear();
        self.base_decisions.clear();
        self.certain_base_values.clear();

        for entity in base.model.entities() {
            self.base_entities.insert(entity.name.clone());
            self.base_entity_sorts
                .entry(entity.name.clone())
                .or_insert_with(|| entity.sort.clone());
            if let Some(kind) = &entity.sort {
                self.base_entities_by_kind
                    .entry(kind.clone())
                    .or_default()
                    .insert(entity.name.clone());
            }
        }
        self.base_decisions.extend(
            documents
                .iter()
                .flat_map(|document| document.decls.iter())
                .filter_map(|decl| match decl {
                    Decl::Ratify(ratify) => Some(ratify.decision.text.clone()),
                    _ => None,
                }),
        );
        for atom in base.model.atoms() {
            #[cfg(test)]
            {
                self.base_atom_visits += 1;
            }
            if atom
                .ann
                .status(&base.model.world.valuation, &base.model.world.theory)
                != Status::Certain
                || atom.atom.args.len() != 2
            {
                continue;
            }
            if let [Term::Sym(subject), value] = atom.atom.args.as_slice() {
                self.certain_base_values
                    .entry(atom.atom.pred.text.clone())
                    .or_default()
                    .entry(subject.text.clone())
                    .or_default()
                    .insert(term_text(value));
            }
        }
        self.rebuild_staged(decls);
    }

    fn rebuild_staged(&mut self, decls: &[Decl]) {
        #[cfg(test)]
        {
            self.staged_rebuild_visits += decls.len();
        }
        self.known_entities.clone_from(&self.base_entities);
        self.entity_sorts.clone_from(&self.base_entity_sorts);
        self.entities_by_kind
            .clone_from(&self.base_entities_by_kind);
        self.known_decisions.clone_from(&self.base_decisions);
        for decl in decls {
            self.add_staged(decl);
        }
    }

    fn add_staged(&mut self, decl: &Decl) {
        match decl {
            Decl::Entity(entity) => {
                let name = entity.name.text.clone();
                let kind = entity.kind.keyword().to_owned();
                self.known_entities.insert(name.clone());
                self.entity_sorts.insert(name.clone(), Some(kind.clone()));
                self.entities_by_kind.entry(kind).or_default().insert(name);
            }
            Decl::Ratify(ratify) => {
                self.known_decisions.insert(ratify.decision.text.clone());
            }
            _ => {}
        }
    }
}

fn term_text(term: &Term) -> String {
    match term {
        Term::Var(id) | Term::Sym(id) => id.text.clone(),
        Term::Str(text) => text.clone(),
        Term::Int(value) => value.to_string(),
        Term::Time(dli_ast::term::TimeRef::Era(id)) => format!("era:{}", id.text),
        Term::Time(dli_ast::term::TimeRef::Event(id)) => format!("event:{}", id.text),
    }
}

/// The canonical JSON encoding of a value (empty on the impossible serialize
/// failure — every type here is infallibly serializable).
fn encode<T: Serialize>(value: &T) -> String {
    serde_json_canonicalizer::to_string(value)
        .expect("workspace state consists only of valid JSON values")
}

#[cfg(test)]
mod tests {
    use super::{Mode, ResolutionIndex, Workspace};

    use dli_ast::decl::{AttrDecl, Decl, EntityDecl, EntityKind, RatifyDecl};
    use dli_ast::ident::Ident;
    use dli_ast::language::RatifyBody;
    use dli_ast::term::Term;
    use serde::Serialize;

    /// The whole-snapshot shape the incremental reseal must reproduce byte for
    /// byte — the field order here is the JSON the fence hashes.
    #[derive(Serialize)]
    struct Snapshot<'a> {
        base_name: &'a str,
        decls: &'a [Decl],
        mode: Mode,
        suppositions: &'a [String],
        #[serde(skip_serializing_if = "Option::is_none")]
        target_branch: Option<&'a str>,
    }

    fn person(name: &str) -> Decl {
        Decl::Entity(EntityDecl {
            kind: EntityKind::Person,
            name: Ident::synthetic(name),
            label: Some(name.to_owned()),
            time: None,
            slots: Vec::new(),
            provenance: None,
        })
    }

    fn entity(name: &str, kind: EntityKind) -> Decl {
        Decl::Entity(EntityDecl {
            kind,
            name: Ident::synthetic(name),
            label: Some(name.to_owned()),
            time: None,
            slots: Vec::new(),
            provenance: None,
        })
    }

    fn ratify(id: &str) -> Decl {
        Decl::Ratify(RatifyDecl {
            decision: Ident::synthetic(id),
            body: RatifyBody {
                clauses: Vec::new(),
                citations: Vec::new(),
            },
        })
    }

    fn fact(index: usize) -> Decl {
        Decl::Attr(AttrDecl {
            subject: Ident::synthetic("world"),
            attr: Ident::synthetic("holds"),
            value: Term::Str(format!("value-{index}")),
            provenance: None,
        })
    }

    fn indexed_base() -> (dli_world::CompiledWorld, Vec<dli_ast::decl::Document>) {
        let source = "scope saltmarket\nratify d-1\nperson world \"World\" decided d-1\nworld.holds = old decided d-1\n";
        let (document, diagnostics) = dli_syntax::parse_document(source);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.class.is_error()),
            "indexed base parses: {diagnostics:?}"
        );
        let documents = vec![document];
        let world = dli_world::compile(&dli_world::pack::Pack {
            name: "saltmarket".to_owned(),
            documents: documents.clone(),
        })
        .expect("indexed base compiles");
        (world, documents)
    }

    fn alternate_base() -> (dli_world::CompiledWorld, Vec<dli_ast::decl::Document>) {
        let source = "scope alternate\nratify d-2\nplace world \"World\" decided d-2\nworld.holds = newer decided d-2\n";
        let (document, diagnostics) = dli_syntax::parse_document(source);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.class.is_error()),
            "alternate base parses: {diagnostics:?}"
        );
        let documents = vec![document];
        let world = dli_world::compile(&dli_world::pack::Pack {
            name: "alternate".to_owned(),
            documents: documents.clone(),
        })
        .expect("alternate base compiles");
        (world, documents)
    }

    fn assert_resolution_matches_rebuild(
        ws: &Workspace,
        base: &dli_world::CompiledWorld,
        documents: &[dli_ast::decl::Document],
    ) {
        let mut rebuilt = ResolutionIndex::default();
        rebuilt.rebase(base, documents, &ws.decls);
        let actual = &ws.resolution;
        assert_eq!(actual.base_entities, rebuilt.base_entities);
        assert_eq!(actual.base_entity_sorts, rebuilt.base_entity_sorts);
        assert_eq!(actual.base_entities_by_kind, rebuilt.base_entities_by_kind);
        assert_eq!(actual.base_decisions, rebuilt.base_decisions);
        assert_eq!(actual.known_entities, rebuilt.known_entities);
        assert_eq!(actual.entity_sorts, rebuilt.entity_sorts);
        assert_eq!(actual.entities_by_kind, rebuilt.entities_by_kind);
        assert_eq!(actual.known_decisions, rebuilt.known_decisions);
        assert_eq!(actual.certain_base_values, rebuilt.certain_base_values);
    }

    fn generated_decl(seed: u32) -> Decl {
        match seed % 4 {
            0 => entity(&format!("entity-{}", (seed >> 8) % 7), EntityKind::Person),
            1 => entity(&format!("entity-{}", (seed >> 8) % 7), EntityKind::Place),
            2 => ratify(&format!("decision-{}", (seed >> 8) % 11)),
            _ => fact(usize::try_from(seed).expect("u32 test seed fits usize")),
        }
    }

    fn scope(base: &str) -> Workspace {
        Workspace::new("ws-1".to_owned(), Mode::Scope, base.to_owned())
    }

    /// The load-bearing invariant: the incrementally-streamed digest is byte-for-
    /// byte the whole-snapshot digest, so every recorded fence and fixture stays
    /// valid across the optimization. Exercised after an append, a supposition,
    /// and a discard so the streamed and re-absorbed paths both prove it.
    #[test]
    fn incremental_reseal_equals_whole_snapshot() {
        let mut ws = scope("saltmarket");
        ws.push_decl(person("mara"));
        ws.push_decl(person("the-warden"));
        ws.suppositions.push("h".to_owned());
        ws.reseal();
        ws.remove_decl(0);

        let snapshot = crate::author::digest::digest_of(&Snapshot {
            base_name: &ws.base_name,
            decls: &ws.decls,
            mode: ws.mode,
            suppositions: &ws.suppositions,
            target_branch: None,
        });
        assert_eq!(
            ws.digest, snapshot,
            "the incremental reseal equals the full-snapshot digest"
        );
    }

    /// A `target_branch` moves the digest fence when present, and — because
    /// it is only ever emitted when `Some` — a branchless workspace's digest
    /// is byte-identical to every fixture recorded before this field existed
    /// (pinned here, not just by omission: the `None` snapshot below matches
    /// the pre-1.7.0 `Snapshot` shape exactly).
    #[test]
    fn target_branch_moves_the_fence_and_is_absent_when_none() {
        let mut branchless = scope("saltmarket");
        branchless.push_decl(person("mara"));

        let mut branched = scope("saltmarket");
        branched.push_decl(person("mara"));
        branched.target_branch = Some("rival".to_owned());
        branched.reseal();

        assert_ne!(
            branchless.digest, branched.digest,
            "a target branch moves the fence"
        );

        let branched_snapshot = crate::author::digest::digest_of(&Snapshot {
            base_name: &branched.base_name,
            decls: &branched.decls,
            mode: branched.mode,
            suppositions: &branched.suppositions,
            target_branch: branched.target_branch.as_deref(),
        });
        assert_eq!(
            branched.digest, branched_snapshot,
            "the incremental reseal with a branch equals the full-snapshot digest"
        );

        let branchless_snapshot = crate::author::digest::digest_of(&Snapshot {
            base_name: &branchless.base_name,
            decls: &branchless.decls,
            mode: branchless.mode,
            suppositions: &branchless.suppositions,
            target_branch: None,
        });
        assert_eq!(
            branchless.digest, branchless_snapshot,
            "a branchless workspace's digest carries no `target_branch` key at all"
        );
    }

    /// The incremental stream — including the re-absorb a discard triggers —
    /// lands on the exact digest a from-scratch build of the same final state
    /// produces. This is the property the fence needs: the digest is a function
    /// of the staged state, never of the path taken to it.
    #[test]
    fn incremental_reseal_equals_from_scratch() {
        let mut built = scope("saltmarket");
        built.push_decl(person("mara"));
        built.push_decl(person("the-warden"));
        built.push_decl(person("kes"));
        built.remove_decl(1); // drop the-warden → [mara, kes]

        let mut fresh = scope("saltmarket");
        fresh.push_decl(person("mara"));
        fresh.push_decl(person("kes"));

        assert_eq!(built.decls.len(), 2);
        assert_eq!(built.digest, fresh.digest);
        assert!(built.digest.starts_with("jcs:sha256:"));
    }

    /// A replace re-absorbs the declarations, matching a build that staged the
    /// replacement value in place.
    #[test]
    fn replace_matches_from_scratch() {
        let mut edited = scope("saltmarket");
        edited.push_decl(person("mara"));
        edited.push_decl(person("the-warden"));
        edited.replace_decl(0, person("kes")); // [kes, the-warden]

        let mut fresh = scope("saltmarket");
        fresh.push_decl(person("kes"));
        fresh.push_decl(person("the-warden"));

        assert_eq!(edited.digest, fresh.digest);
    }

    /// The fence is deterministic and sensitive: identical state hashes the same,
    /// and declaration order, the base, and a supposition each move it.
    #[test]
    fn reseal_is_deterministic_and_sensitive() {
        let mut a = scope("saltmarket");
        a.push_decl(person("mara"));
        a.push_decl(person("the-warden"));

        let mut same = scope("saltmarket");
        same.push_decl(person("mara"));
        same.push_decl(person("the-warden"));
        assert_eq!(a.digest, same.digest, "same state → same fence");

        let mut reordered = scope("saltmarket");
        reordered.push_decl(person("the-warden"));
        reordered.push_decl(person("mara"));
        assert_ne!(a.digest, reordered.digest, "order moves the fence");

        let mut other_base = scope("the-shallows");
        other_base.push_decl(person("mara"));
        other_base.push_decl(person("the-warden"));
        assert_ne!(a.digest, other_base.digest, "the base moves the fence");

        let mut supposed = scope("saltmarket");
        supposed.push_decl(person("mara"));
        supposed.push_decl(person("the-warden"));
        supposed.suppositions.push("h-1".to_owned());
        supposed.reseal();
        assert_ne!(a.digest, supposed.digest, "a supposition moves the fence");
    }

    /// An empty workspace still seals a stable, domain-shaped fence.
    #[test]
    fn empty_workspace_seals_a_stable_fence() {
        let a = scope("saltmarket");
        let b = scope("saltmarket");
        assert_eq!(a.digest, b.digest);
        assert!(a.digest.starts_with("jcs:sha256:"));
    }

    /// The resolution census is a derived cache, never an alternate semantics:
    /// append updates, duplicate-sort fallback, mid-array rebuild, and decision
    /// removal all reproduce the former base-plus-staged scans.
    #[test]
    fn resolution_index_tracks_append_replace_remove_and_base_state() {
        let (base, documents) = indexed_base();
        let mut ws = scope("saltmarket");
        ws.refresh_resolution(&base, &documents);

        assert!(ws.known_entities().contains("world"));
        assert_eq!(ws.entity_sort("world"), Some("person"));
        assert!(ws.known_decisions().contains("d-1"));
        assert_eq!(
            ws.certain_base_values("holds", "world")
                .expect("certain base slot")
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["old"]
        );

        ws.push_decl(entity("mara", EntityKind::Person));
        ws.push_decl(entity("mara", EntityKind::Place));
        ws.push_decl(ratify("d-stage"));
        assert_eq!(ws.entity_sort("mara"), Some("place"));
        assert_eq!(ws.entities_of_kind("person"), vec!["mara", "world"]);
        assert_eq!(ws.entities_of_kind("place"), vec!["mara"]);
        assert!(ws.known_decisions().contains("d-stage"));

        ws.remove_decl(1);
        assert_eq!(ws.entity_sort("mara"), Some("person"));
        assert!(ws.entities_of_kind("place").is_empty());

        ws.replace_decl(0, entity("mara", EntityKind::Thing));
        assert_eq!(ws.entity_sort("mara"), Some("thing"));
        assert!(!ws.entities_of_kind("person").contains(&"mara".to_owned()));

        ws.remove_decl(1);
        assert!(!ws.known_decisions().contains("d-stage"));
        assert!(ws.known_decisions().contains("d-1"));
    }

    /// Deterministic mutation-property oracle: incremental append state and the
    /// rare edit/reload rebuild paths equal a fresh base-plus-staged census after
    /// every operation. The fixed generator makes failures exactly replayable.
    #[test]
    fn resolution_index_matches_rebuild_across_mutation_sequences() {
        let (base_a, documents_a) = indexed_base();
        let (base_b, documents_b) = alternate_base();
        let mut use_alternate = false;
        let mut ws = scope("saltmarket");
        ws.refresh_resolution(&base_a, &documents_a);
        let mut seed = 0xcafe_f00d_u32;
        let mut appends = 0usize;
        let mut replaces = 0usize;
        let mut removals = 0usize;
        let mut reloads = 0usize;

        for step in 0..256usize {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            match step % 4 {
                2 if !ws.decls.is_empty() => {
                    let index =
                        usize::try_from(seed).expect("u32 test seed fits usize") % ws.decls.len();
                    ws.replace_decl(index, generated_decl(seed.rotate_left(17)));
                    replaces += 1;
                }
                3 if !ws.decls.is_empty() => {
                    let index =
                        usize::try_from(seed).expect("u32 test seed fits usize") % ws.decls.len();
                    ws.remove_decl(index);
                    removals += 1;
                }
                _ => {
                    ws.push_decl(generated_decl(seed));
                    appends += 1;
                }
            }
            if step % 31 == 30 {
                use_alternate = !use_alternate;
                let (base, documents) = if use_alternate {
                    (&base_b, documents_b.as_slice())
                } else {
                    (&base_a, documents_a.as_slice())
                };
                ws.refresh_resolution(base, documents);
                reloads += 1;
            }
            let (base, documents) = if use_alternate {
                (&base_b, documents_b.as_slice())
            } else {
                (&base_a, documents_a.as_slice())
            };
            assert_resolution_matches_rebuild(&ws, base, documents);
        }

        assert!(appends >= 128, "append coverage is anti-vacuous");
        assert!(replaces >= 50, "replace coverage is anti-vacuous");
        assert!(removals >= 50, "remove coverage is anti-vacuous");
        assert!(reloads >= 8, "base-reload coverage is anti-vacuous");
    }

    /// Gate-resident complexity oracle: a fact-only append sequence neither
    /// rebuilds the staged census nor revisits base atoms. Wall time is not the
    /// verdict; the exact historical-visit budget is zero.
    #[test]
    fn fact_appends_do_no_historical_resolution_work() {
        const FACTS: usize = 16_384;
        let (base, documents) = indexed_base();
        let mut ws = scope("saltmarket");
        ws.refresh_resolution(&base, &documents);
        let staged_visits = ws.resolution.staged_rebuild_visits;
        let base_visits = ws.resolution.base_atom_visits;

        for index in 0..FACTS {
            ws.push_decl(fact(index));
            assert!(ws.known_entities().contains("world"));
            assert!(ws.known_decisions().contains("d-1"));
            assert!(ws.certain_base_values("holds", "world").is_some());
        }

        assert_eq!(ws.decls.len(), FACTS);
        assert_eq!(ws.resolution.staged_rebuild_visits, staged_visits);
        assert_eq!(ws.resolution.base_atom_visits, base_visits);
    }

    /// The reseal is linear, not quadratic: staging the second half of a large
    /// workspace costs no more than staging the first half (each append is `O(1)`
    /// in the declaration count, where a whole-snapshot reseal would make the
    /// second half ~3× the first). Wall-clock, so `#[ignore]`d against CI noise —
    /// run with `--ignored` to measure.
    #[test]
    #[ignore = "wall-clock perf check; run with --ignored"]
    fn reseal_stays_linear_over_a_large_build() {
        use std::time::Instant;

        const HALF: usize = 2500;
        let mut ws = scope("saltmarket");

        let first_start = Instant::now();
        for i in 0..HALF {
            ws.push_decl(person(&format!("person-{i}")));
        }
        let first = first_start.elapsed();

        let second_start = Instant::now();
        for i in HALF..(2 * HALF) {
            ws.push_decl(person(&format!("person-{i}")));
        }
        let second = second_start.elapsed();

        assert_eq!(ws.decls.len(), 2 * HALF);
        assert!(
            second < first * 4,
            "reseal looks super-linear: first {HALF} appends took {first:?}, \
             second {HALF} took {second:?}"
        );
    }
}
