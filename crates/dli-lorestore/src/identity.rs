//! Three-way identity for stored fact objects.
//!
//! The lorestore pivot splits identity into three concerns, each with its own
//! type, so no single pointer serves (and conflates) all three (DL-0041,
//! roadmap §LS-1):
//!
//! 1. **Authority distinctness** — the provenance polynomial and [`Authority`]
//!    are part of the object and its hash, so the same proposition asserted by
//!    two authorities is already two distinct objects. No parent hash needed.
//! 2. **Claim continuity** — a stable [`ClaimId`] tracks "this claim through its
//!    revisions" (the Wikibase entity-vs-datom split). A parent chain serves
//!    this badly: hashes change on every revision and a Merkle chain re-hashes
//!    every descendant.
//! 3. **Supersession / retcon history as domain knowledge** —
//!    [`LineageEdge`] carries its own `(P⁺, P⁻)`, because in a multi-authority
//!    world *whether* B supersedes A is itself contested and
//!    valuation-dependent.
//!
//! Only the bare parent pointer stays out of the content hash: it delivers none
//! of the three and costs cross-branch convergence — the same assertion
//! reached on two forks must be the same object for merge to be union.
//! Storage-level ancestry lives in the commit DAG (§3), as in git.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION;
use crate::canonical::CanonicalError;
use dli_core::posbool::PosBool;

/// A `sha256:<hex>` content digest of a JCS-serialized object.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ContentHash(pub String);

impl ContentHash {
    /// The canonical `"sha256:<hex>"` form as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse a `sha256:<hex>` string into a [`ContentHash`].
    ///
    /// Returns `None` if the string does not carry the `sha256:` prefix — the
    /// only validation this type performs; full JCS round-trip integrity is
    /// checked on read by [`validate_content_address`](crate::validate_content_address).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        s.strip_prefix("sha256:").map(|_| Self(s.to_owned()))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for ContentHash {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// A stable minted claim identifier.
///
/// Unlike the [`ContentHash`], a `ClaimId` is not derived from the fact's
/// content — it is minted once and survives revisions, so lineage and
/// cross-version references remain stable even when the fact's text changes.
/// The content hash does not reference it, so minting a `ClaimId` never
/// destabilizes addressing.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ClaimId(pub String);

impl ClaimId {
    /// Mint a stable claim id from a deterministic label. The id encodes a
    /// content hash of the label so it is reproducible across builds (v1
    /// sovereign deployments need no RNG).
    ///
    /// The label is typically a path like `<pack>/<kind>/<subject>/<predicate>`
    /// — enough to disambiguate within one continuity. A `ClaimId` is never a
    /// content hash of the *fact* itself; it is a *claim identity*, minted
    /// from addressing metadata.
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        let digest = ContentHash(format!("sha256:{}", crate::canonical::sha256_hex(label)));
        Self(digest.0)
    }

    /// Create a claim id from an existing `sha256:` string.
    #[must_use]
    pub fn from_hash(hash: ContentHash) -> Self {
        Self(hash.0)
    }
}

impl fmt::Display for ClaimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The authority that asserted an object.
///
/// Exists on every stored object; stays `None` in single-sovereign
/// deployments (DEF-4 unchanged). In multi-authority configurations, the
/// authority tag distinguishes who proposed or ruled the object — and because
/// it participates in the content hash, the same proposition asserted by two
/// authorities is two distinct objects, never conflated by dedup.
#[derive(
    Debug, Clone, PartialEq, Eq, Default, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct Authority(pub Option<String>);

impl Authority {
    /// No authority set — the single-sovereign default.
    #[must_use]
    pub fn sovereign() -> Self {
        Self(None)
    }

    /// A named authority.
    #[must_use]
    pub fn named(name: &str) -> Self {
        Self(Some(name.to_owned()))
    }

    /// Whether this is a sovereign (single-authority) object.
    #[must_use]
    pub fn is_sovereign(&self) -> bool {
        self.0.is_none()
    }
}

/// A first-class lineage edge: the record that B supersedes A.
///
/// In a multi-authority world, *whether* B supersedes A is itself contested
/// and valuation-dependent — one continuity accepts the retcon, another
/// rejects it. The edge must therefore cite the ruling it happened under,
/// carried as its own `(P⁺, P⁻)` support/defeater pair. A bare parent hash
/// inside B cannot do this: it hard-codes one true lineage.
///
/// The edge's content hash does not include any parent pointer of its own; it
/// hashes its support/defeater polynomials, its from/to pair, its authority,
/// and its `schema_version`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LineageEdge {
    /// Schema version; bumped when the stored shape changes.
    pub schema_version: u32,
    /// The authority behind this lineage judgment (unset in sovereign
    /// deployments).
    pub authority: Authority,
    /// The claim this edge annotates (continuity across revisions).
    pub claim_id: ClaimId,
    /// The fact being superseded (content hash of the `from` fact).
    pub from: ContentHash,
    /// The fact doing the superseding (content hash of the `to` fact).
    pub to: ContentHash,
    /// When the supersession holds: the support polynomial.
    /// Absorption-normalized irredundant monotone DNF with sorted monomials.
    pub support: PosBool,
    /// What could defeat the supersession: the defeater polynomial.
    pub defeaters: PosBool,
}

impl LineageEdge {
    /// Construct a lineage edge with the given identities and polynomials.
    #[must_use]
    pub fn new(
        authority: Authority,
        claim_id: ClaimId,
        from: ContentHash,
        to: ContentHash,
        support: PosBool,
        defeaters: PosBool,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            authority,
            claim_id,
            from,
            to,
            support,
            defeaters,
        }
    }

    /// The content hash of this edge — over all fields except any parent or
    /// commit pointer.
    pub fn content_hash(&self) -> Result<ContentHash, CanonicalError> {
        crate::canonical::content_hash(self)
    }
}
