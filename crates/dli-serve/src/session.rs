//! Serve sessions (`SERVE-1`): doc-id to loaded-scope state, and the fork registry.
//!
//! A [`Sessions`] resolves a wire `document` id to a compiled [`Scope`] exactly
//! once and caches it — the in-memory shape of the derived-world cache
//! (`docs/spec/10-serving.md` §4): a second request for the same corpus reuses
//! the compiled world rather than recompiling. Forks are session state:
//! `dli.fork.new` mints a stable wire handle bound to `(document, ForkId)`, and
//! the later fork ops resolve that handle back to the owning scope's
//! [`ForkStore`].
//!
//! The corpus resolver is deliberately small at v0: the built-in `saltmarket`
//! corpus (the crate's canonical test world, `dli_world::loader::example_docs`)
//! resolves hermetically, and any other id is [`ErrorCode::UnknownDoc`]. Pointing
//! the resolver at a corpus directory — parse each `<id>.dli` through
//! `dli_syntax::parse_document` and compile it — is the single seam that opens the
//! surface to arbitrary corpora; the cache, handle registry, and every handler
//! above are already corpus-agnostic.

use std::collections::BTreeMap;

use dli_eval::fork::{ForkId, ForkStore};
use dli_world::loader::example_docs;
use dli_world::{CompiledWorld, Pack, compile};

use crate::envelope::ErrorCode;

/// A loaded corpus scope: the compiled world every read op queries, and the fork
/// store every fork op layers over its saturated base.
#[derive(Debug)]
pub struct Scope {
    /// The compiled, evaluable world.
    pub world: CompiledWorld,
    /// The forks opened over this scope. Its base is a clone of the compiled
    /// world's saturated program, so a fork never disturbs the read surface.
    pub forks: ForkStore,
}

/// The session store shared across one serve process: the doc-id→scope cache and
/// the global fork-handle registry. One `Sessions` lives for the life of the
/// `serve` loop; nothing here is persisted (the worker holds no cross-process job
/// state — `10-serving.md` §5).
#[derive(Debug, Default)]
pub struct Sessions {
    scopes: BTreeMap<String, Scope>,
    /// Wire fork handle → the scope and fork it names.
    fork_handles: BTreeMap<String, (String, ForkId)>,
    /// Monotonic handle counter, so handles (`fork-1`, `fork-2`, …) are stable
    /// and never reused within a process.
    next_handle: u64,
}

impl Sessions {
    /// An empty session store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve a document id to its compiled scope, compiling and caching on first
    /// use. Only the built-in `saltmarket` corpus is wired at v0; every other id is
    /// [`ErrorCode::UnknownDoc`] until the resolver is pointed at a corpus root.
    ///
    /// A compile failure maps through the frozen [`ErrorCode`] conversion — an
    /// unstratifiable or contradictory corpus surfaces as its typed error, never a
    /// panic.
    pub fn scope(&mut self, document: &str) -> Result<&mut Scope, ErrorCode> {
        if !self.scopes.contains_key(document) {
            let documents = match document {
                "saltmarket" => example_docs(),
                _ => return Err(ErrorCode::UnknownDoc),
            };
            let pack = Pack {
                name: document.to_owned(),
                documents,
            };
            let world = compile(&pack).map_err(ErrorCode::from)?;
            let forks = ForkStore::new(world.model.world.clone());
            self.scopes
                .insert(document.to_owned(), Scope { world, forks });
        }
        Ok(self
            .scopes
            .get_mut(document)
            .expect("scope was just resolved into the cache"))
    }

    /// Mint a fresh, stable wire handle bound to a fork within a document.
    pub fn register_fork(&mut self, document: &str, fork: ForkId) -> String {
        self.next_handle += 1;
        let handle = format!("fork-{}", self.next_handle);
        self.fork_handles
            .insert(handle.clone(), (document.to_owned(), fork));
        handle
    }

    /// Resolve a wire fork handle to the document and fork it names.
    #[must_use]
    pub fn fork_handle(&self, handle: &str) -> Option<(String, ForkId)> {
        self.fork_handles.get(handle).cloned()
    }

    /// The fork handles registered for a document, in handle order.
    #[must_use]
    pub fn forks_of(&self, document: &str) -> Vec<String> {
        self.fork_handles
            .iter()
            .filter(|(_, (doc, _))| doc == document)
            .map(|(handle, _)| handle.clone())
            .collect()
    }
}
