//! The store's structural types: the append-only commit log and the valuated
//! reference (DL-0041, roadmap §LS-1 / §LS-2).
//!
//! A [`Commit`] is the atomic unit of the append-only log: it names the fact
//! objects it introduces, the lineage edges, the evidence envelopes, and the
//! event-variable interner (so support polynomials are self-describing). The
//! parent pointer on a commit is a *commit* hash — deliberately excluded from
//! every fact object's content hash (the three-way identity split).
//!
//! A [`Ref`] is a named pointer to a commit, carrying a valuation overlay.
//! Forks-as-valuation-overlays already work in memory (`dli_eval::ForkStore`);
//! the store makes them durable — a ref is O(1) to create (a pointer + an
//! overlay), evaluation is O(dependent slice) through the support index.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION;
use crate::identity::{Authority, ContentHash};
use dli_core::valuation::Valuation;

/// An append-only commit: one atomic write to the store.
///
/// The commit DAG carries **storage-level ancestry** (which write produced which
/// state) — as in git, ancestry is a commit-level concern, never a fact-level
/// one. The fact object's content hash embeds no parent pointer of any kind.
///
/// The interner is a flattened name list (`Vec<String>`) — same serialization
/// as `dli_core::Interner` — so support polynomials using `Atom(u32)` indices
/// are resolvable without a separate lookup. It is committed at the root
/// commit; subsequent commits may extend it (append-only — atoms are never
/// renumbered, only added).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Commit {
    /// Schema version; bumped when the stored shape changes.
    pub schema_version: u32,
    /// The authority behind this commit (unset in sovereign deployments).
    pub authority: Authority,
    /// The parent commit hash — storage-level ancestry only. This field is
    /// NOT and NEVER will be part of any fact object's content hash.
    pub parent: Option<ContentHash>,
    /// The fact objects introduced by this commit, as content hashes.
    pub facts: Vec<ContentHash>,
    /// The lineage edge objects introduced by this commit.
    pub lineage: Vec<ContentHash>,
    /// The evidence envelope hashes introduced by this commit.
    pub evidence: Vec<ContentHash>,
    /// The event-variable name table at this commit — a flat list so that
    /// `Atom(u32)` indices in support polynomials resolve deterministically.
    /// Present on the root commit; extended by append-only growth only.
    pub interner: Vec<String>,
    /// The known decision ids (`Decl::Ratify`) — needed to reconstruct
    /// `Decl::Ratify` declarations when importing back from store to `Pack`.
    /// Stored on the root commit; child commits may extend it. Optional on
    /// read (empty default); wire-compatible with v1 commits that lack it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<String>,
}

impl Commit {
    /// Construct a root commit (no parent).
    #[must_use]
    pub fn root(
        interner: Vec<String>,
        facts: Vec<ContentHash>,
        lineage: Vec<ContentHash>,
        evidence: Vec<ContentHash>,
        decisions: Vec<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            authority: Authority::sovereign(),
            parent: None,
            facts,
            lineage,
            evidence,
            interner,
            decisions,
        }
    }

    /// Construct a child commit branching from `parent`.
    #[must_use]
    pub fn child(
        parent: ContentHash,
        interner: Vec<String>,
        facts: Vec<ContentHash>,
        lineage: Vec<ContentHash>,
        evidence: Vec<ContentHash>,
        decisions: Vec<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            authority: Authority::sovereign(),
            parent: Some(parent),
            facts,
            lineage,
            evidence,
            interner,
            decisions,
        }
    }

    /// The content hash of this commit — over all fields. Used as the ref
    /// target and as the `parent` of the next commit.
    pub fn content_hash(&self) -> Result<ContentHash, crate::canonical::CanonicalError> {
        crate::canonical::content_hash(self)
    }
}

/// A named reference to a commit, carrying a valuation overlay.
///
/// The valuation overlay is how forks-as-valuation-overlays work on the
/// persistent store: creating a ref is O(1) (a pointer + an overlay), and
/// evaluating it is O(dependent slice) through the support index. The base
/// commit's fact set is unchanged; the overlay just re-values event variables
/// at query time — exactly the `dli_eval::ForkStore` model, made durable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ref {
    /// Schema version; bumped when the stored shape changes.
    pub schema_version: u32,
    /// The authority owning this ref (unset in sovereign deployments).
    pub authority: Authority,
    /// The ref name — e.g. `main`, `fork/option-x-true`.
    pub name: String,
    /// The commit this ref points to.
    pub commit: ContentHash,
    /// The valuation overlay: event variables ruled true or false by this ref.
    /// Applied over the commit's base valuation at evaluation time.
    pub valuation: Valuation,
}

impl Ref {
    /// Construct a ref pointing at `commit` with no valuation override.
    #[must_use]
    pub fn new(name: impl Into<String>, commit: ContentHash) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            authority: Authority::sovereign(),
            name: name.into(),
            commit,
            valuation: Valuation::new(),
        }
    }

    /// Construct a ref with a valuation overlay (a fork).
    #[must_use]
    pub fn with_valuation(
        name: impl Into<String>,
        commit: ContentHash,
        valuation: Valuation,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            authority: Authority::sovereign(),
            name: name.into(),
            commit,
            valuation,
        }
    }

    /// The `sha256:` digest of this ref's canonical serialization — used for
    /// digest fencing and optimistic concurrency on agent sessions.
    pub fn digest(&self) -> Result<ContentHash, crate::canonical::CanonicalError> {
        crate::canonical::content_hash(self)
    }
}
