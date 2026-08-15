//! The session compile cache: an in-process, content-addressed memo of the
//! store's read path (DL-0087/DL-0088, Phase 3 PR 3).
//!
//! # Why this is sound without an invalidation protocol
//!
//! [`Lorestore::compile_at`](crate::Lorestore::compile_at) is a **pure function
//! of a commit hash**. The commit names an immutable object graph — its
//! interner, its decision list, its pack name, and, through the DAG walk, every
//! fact object reachable from it — and content addressing means a hash's content
//! can never change. So a world compiled at commit `c` and a world freshly
//! compiled at commit `c` are equal *by construction*, not by a checked
//! coincidence. There is no invalidation to get wrong: the key already names the
//! content.
//!
//! That is also why eviction is safe at any moment. Every entry is a
//! **rebuildable derived reader** over the log (`state ≡ fold(log)`); dropping
//! one costs a recompute and nothing else. The cache is never a source of truth,
//! never persisted, and never consulted for a verdict — only for a value the
//! store could have recomputed itself.
//!
//! # What is cached
//!
//! Both halves of the read path, independently:
//!
//! - the reconstructed [`Pack`] — two DAG walks, a read of every reachable fact
//!   object, and `reconstruct_pack`; and
//! - the [`CompiledWorld`] compiled from it — the loader pass and the symbolic
//!   fixpoint.
//!
//! They share one entry because they share one key, but each fills in lazily: a
//! caller that only wants the pack (the author read paths need
//! `pack.documents`) never pays for a compile, and a later `compile_at` on the
//! same commit reuses the pack it left behind.
//!
//! # Bound
//!
//! [`DEFAULT_CACHE_CAPACITY`] entries, least-recently-used evicted past the cap.
//! A compiled world is not small — it carries the saturated relation — so an
//! unbounded memo over a pilot's hundreds of commits per persona is a leak with
//! a nicer name. The cap is per-store and adjustable
//! ([`Lorestore::set_cache_capacity`](crate::Lorestore::set_cache_capacity)) so
//! a host that knows its working set can tune it, and so a test can pin it to 1
//! and exercise eviction.

use std::collections::BTreeMap;
use std::sync::Arc;

use dli_world::CompiledWorld;
use dli_world::pack::Pack;

use crate::identity::ContentHash;

/// How many commits' reconstructions the session cache keeps before evicting the
/// least recently used.
///
/// Sized for the working set the read paths actually touch — a merge scan wants
/// three commits (target, source, base) plus the merge commit, and an authoring
/// session walks a short recency window of branch heads — with generous headroom,
/// not for a whole pilot's history. Compiled worlds are the largest values this
/// crate holds in memory, so headroom past the working set buys nothing but
/// resident bytes.
pub const DEFAULT_CACHE_CAPACITY: usize = 64;

/// What the session cache did, over the life of one [`Lorestore`](crate::Lorestore).
///
/// A report, not a verdict: nothing in the store's behaviour depends on these
/// counters. They exist because a cache that silently never hits is
/// indistinguishable — from the outside — from one that works, and a parity
/// oracle over a cache with no hits is a green test proving nothing. This is the
/// channel that lets a test put a floor under the reuse.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// `reconstruct_at` calls served from the cache.
    pub pack_hits: u64,
    /// `reconstruct_at` calls that had to walk the DAG and read the objects.
    pub pack_misses: u64,
    /// `compile_at` calls served from the cache.
    pub world_hits: u64,
    /// `compile_at` calls that had to run the loader and the fixpoint.
    pub world_misses: u64,
    /// Entries dropped to stay under the capacity bound.
    pub evictions: u64,
}

impl CacheStats {
    /// Total cache hits across both halves.
    #[must_use]
    pub const fn hits(&self) -> u64 {
        self.pack_hits + self.world_hits
    }

    /// Total misses across both halves.
    #[must_use]
    pub const fn misses(&self) -> u64 {
        self.pack_misses + self.world_misses
    }
}

/// One commit's memoized reconstruction. Either half may be absent — they are
/// filled by whichever call arrives first.
#[derive(Debug, Default)]
struct Entry {
    pack: Option<Arc<Pack>>,
    world: Option<Arc<CompiledWorld>>,
    /// The logical clock reading at this entry's last use; the LRU victim is the
    /// entry with the smallest one.
    used: u64,
}

/// The bounded LRU memo behind the store's read path.
///
/// Hand-rolled rather than pulled from a crate: at this capacity the eviction
/// scan is a linear pass over at most a few dozen entries, which costs
/// immeasurably less than the compile it is protecting, and a dependency whose
/// whole value is an intrusive linked list would be more code to audit than the
/// forty lines below.
#[derive(Debug)]
pub(crate) struct CompileCache {
    entries: BTreeMap<ContentHash, Entry>,
    capacity: usize,
    /// Monotone use counter — the recency order. Never wraps in any realistic
    /// session, and a wrap would degrade eviction order, not correctness.
    clock: u64,
    stats: CacheStats,
}

impl Default for CompileCache {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CACHE_CAPACITY)
    }
}

impl CompileCache {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            capacity,
            clock: 0,
            stats: CacheStats::default(),
        }
    }

    pub(crate) const fn stats(&self) -> CacheStats {
        self.stats
    }

    /// Resize the bound, evicting immediately if the new cap is smaller. A cap of
    /// 0 disables the cache (and drops everything it held).
    pub(crate) fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
        self.evict_to_capacity();
    }

    /// Drop every entry. Correct at any moment — see the module docs — and used
    /// by oracles that need a cold recompute to compare against.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    /// The memoized pack at `head`, if present. Counts a hit or a miss.
    pub(crate) fn get_pack(&mut self, head: &ContentHash) -> Option<Arc<Pack>> {
        self.clock += 1;
        let clock = self.clock;
        let found = self.entries.get_mut(head).and_then(|entry| {
            entry.used = clock;
            entry.pack.clone()
        });
        if found.is_some() {
            self.stats.pack_hits += 1;
        } else {
            self.stats.pack_misses += 1;
        }
        found
    }

    /// The memoized world at `head`, if present. Counts a hit or a miss.
    pub(crate) fn get_world(&mut self, head: &ContentHash) -> Option<Arc<CompiledWorld>> {
        self.clock += 1;
        let clock = self.clock;
        let found = self.entries.get_mut(head).and_then(|entry| {
            entry.used = clock;
            entry.world.clone()
        });
        if found.is_some() {
            self.stats.world_hits += 1;
        } else {
            self.stats.world_misses += 1;
        }
        found
    }

    /// Memoize a pack. Overwrites any prior value, which is a no-op in content:
    /// the same key names the same reconstruction.
    pub(crate) fn put_pack(&mut self, head: &ContentHash, pack: Arc<Pack>) {
        self.touch(head).pack = Some(pack);
        self.evict_to_capacity();
    }

    /// Memoize a compiled world.
    pub(crate) fn put_world(&mut self, head: &ContentHash, world: Arc<CompiledWorld>) {
        self.touch(head).world = Some(world);
        self.evict_to_capacity();
    }

    fn touch(&mut self, head: &ContentHash) -> &mut Entry {
        self.clock += 1;
        let clock = self.clock;
        let entry = self.entries.entry(head.clone()).or_default();
        entry.used = clock;
        entry
    }

    fn evict_to_capacity(&mut self) {
        while self.entries.len() > self.capacity {
            let Some(victim) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(hash, _)| hash.clone())
            else {
                break;
            };
            self.entries.remove(&victim);
            self.stats.evictions += 1;
        }
    }

    /// Corrupt the world memoized at `head` — the positive control a parity
    /// oracle needs so it cannot go vacuous.
    ///
    /// Moves one derived row's grade in the cached copy. A read path that
    /// genuinely serves from the cache must carry the falsification into its
    /// answer; one that quietly recomputed cannot. Returns whether there was a
    /// cached world with a row to corrupt.
    ///
    /// Deliberately produces a wrong world. It is not `cfg(test)` because the
    /// oracle is an integration test compiled against this crate from outside.
    pub(crate) fn falsify_world(&mut self, head: &ContentHash) -> bool {
        let Some(entry) = self.entries.get_mut(head) else {
            return false;
        };
        let Some(world) = entry.world.as_ref() else {
            return false;
        };
        let mut copy = (**world).clone();
        let Some(row) = copy.model.world.derived.first_mut() else {
            return false;
        };
        row.grade = if row.grade == dli_core::grade::Grade::Checked {
            dli_core::grade::Grade::Asserted
        } else {
            dli_core::grade::Grade::Checked
        };
        entry.world = Some(Arc::new(copy));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{CompileCache, DEFAULT_CACHE_CAPACITY};
    use crate::identity::ContentHash;
    use std::sync::Arc;

    fn key(n: u8) -> ContentHash {
        ContentHash(format!("sha256:{n:064x}"))
    }

    fn pack() -> Arc<dli_world::pack::Pack> {
        Arc::new(dli_world::pack::Pack {
            name: "canon".to_owned(),
            documents: Vec::new(),
        })
    }

    /// The store's `Send + Sync` is a documented property that `dli-serve`
    /// relies on, and adding an interior-mutability field is exactly how a crate
    /// loses it by accident. Assert it rather than trust that today's field
    /// choice survives the next one.
    #[test]
    fn lorestore_stays_send_and_sync() {
        const fn require<T: Send + Sync>() {}
        require::<crate::Lorestore>();
    }

    #[test]
    fn eviction_drops_the_least_recently_used() {
        let mut cache = CompileCache::with_capacity(2);
        cache.put_pack(&key(1), pack());
        cache.put_pack(&key(2), pack());
        // Touch 1, so 2 becomes the least recently used.
        assert!(cache.get_pack(&key(1)).is_some());
        cache.put_pack(&key(3), pack());

        assert!(
            cache.get_pack(&key(1)).is_some(),
            "1 was used most recently"
        );
        assert!(cache.get_pack(&key(3)).is_some(), "3 was just inserted");
        assert!(cache.get_pack(&key(2)).is_none(), "2 was the LRU victim");
        assert_eq!(cache.stats().evictions, 1);
    }

    #[test]
    fn shrinking_the_capacity_evicts_immediately() {
        let mut cache = CompileCache::with_capacity(DEFAULT_CACHE_CAPACITY);
        for n in 0..5 {
            cache.put_pack(&key(n), pack());
        }
        cache.set_capacity(1);
        assert_eq!(cache.stats().evictions, 4);
        // Capacity 0 empties it and keeps it empty.
        cache.set_capacity(0);
        cache.put_pack(&key(9), pack());
        assert!(
            cache.get_pack(&key(9)).is_none(),
            "a 0-cap cache holds nothing"
        );
    }

    /// The two halves are independent: memoizing a pack must not fabricate a
    /// world at the same key, or `compile_at` would serve a `None` it never
    /// computed.
    #[test]
    fn the_pack_and_world_halves_fill_separately() {
        let mut cache = CompileCache::with_capacity(4);
        cache.put_pack(&key(1), pack());
        assert!(cache.get_pack(&key(1)).is_some());
        assert!(cache.get_world(&key(1)).is_none());

        let stats = cache.stats();
        assert_eq!(stats.pack_hits, 1);
        assert_eq!(stats.world_misses, 1);
    }
}
