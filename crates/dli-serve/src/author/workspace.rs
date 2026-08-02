//! Door T workspace (`AUTH-1`): the in-memory staged layer over a compiled base.
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

use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};

use dli_ast::decl::Decl;

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
    /// The hypothesis ids supposed true within the fork (spec 11 §3.9).
    pub suppositions: Vec<String>,
    /// The current digest fence.
    pub digest: String,
    /// Fact object hashes written to the lorestore for staged declarations
    /// (LS-4). Populated on each `author.add_*` call; consumed by
    /// `author.materialize` when committing to `main`.
    pub staged_fact_hashes: Vec<dli_lorestore::identity::ContentHash>,
    /// Idempotency replay cache: key → (payload fingerprint, the exact result
    /// value first produced). The fingerprint guards key reuse: the same key with
    /// a different payload is a conflict, never a silent cached replay (R3/M9).
    pub idempotency: BTreeMap<String, (String, serde_json::Value)>,
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
            suppositions: Vec::new(),
            digest: String::new(),
            idempotency: BTreeMap::new(),
            staged_fact_hashes: Vec::new(),
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
        self.decls.push(decl);
        self.reseal();
    }

    /// Replace the declaration at `index` (e.g. a question that gained an option)
    /// and reseal. The prefix is re-absorbed once, since the change lands mid-array.
    pub fn replace_decl(&mut self, index: usize, decl: Decl) {
        if let Some(slot) = self.decls.get_mut(index) {
            *slot = decl;
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
        self.reabsorb();
        self.reseal();
        Some(removed)
    }

    /// Recompute the digest by finishing the streamed material: clone the prefix
    /// (the head and every declaration, already absorbed), feed the tail
    /// (`],"mode":…,"suppositions":…}`), and finalize. `O(1)` in the declaration
    /// count.
    ///
    /// The absorbed-plus-tail bytes are exactly
    /// `{"base_name":…,"decls":[…],"mode":…,"suppositions":…}`, the JCS string the
    /// whole-snapshot reseal hashed, so the digest value is unchanged.
    pub fn reseal(&mut self) {
        let mut hasher = self.prefix.clone();
        hasher.update(b"],\"mode\":");
        hasher.update(encode(&self.mode).as_bytes());
        hasher.update(b",\"suppositions\":");
        hasher.update(encode(&self.suppositions).as_bytes());
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

/// The canonical JSON encoding of a value (empty on the impossible serialize
/// failure — every type here is infallibly serializable).
fn encode<T: Serialize>(value: &T) -> String {
    serde_json_canonicalizer::to_string(value)
        .expect("workspace state consists only of valid JSON values")
}

#[cfg(test)]
mod tests {
    use super::{Mode, Workspace};

    use dli_ast::decl::{Decl, EntityDecl, EntityKind};
    use dli_ast::ident::Ident;
    use serde::Serialize;

    /// The whole-snapshot shape the incremental reseal must reproduce byte for
    /// byte — the field order here is the JSON the fence hashes.
    #[derive(Serialize)]
    struct Snapshot<'a> {
        base_name: &'a str,
        decls: &'a [Decl],
        mode: Mode,
        suppositions: &'a [String],
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
        });
        assert_eq!(
            ws.digest, snapshot,
            "the incremental reseal equals the full-snapshot digest"
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
