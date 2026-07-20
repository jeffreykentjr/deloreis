//! Door T digest (`AUTH-1`): the workspace fence.
//!
//! Every mutating `author.*` call answers with a `workspace_digest`, and a
//! caller may fence its next call with `expected_digest`; a mismatch is a
//! `stale-fence` refusal (spec 11 §2). The digest is a content hash over the
//! staged state, so it moves iff the staged state moves.
//!
//! The contract names **RFC 8785 JCS** as the digest domain (fantasy-contract
//! §3.2/§4). Here we take the deterministic-serialization subset that domain
//! needs for a *self-consistent* fence: `serde_json` serializes struct fields in
//! declaration order and arrays in element order, so the encoding of a given
//! staged state is stable across runs and processes. Full canonicalization
//! (number/whitespace normalization, cross-serializer key ordering) is only
//! load-bearing when two independent serializers must agree byte-for-byte on the
//! *same* digest; the fence only ever compares a digest this engine minted
//! against one it minted earlier, so deterministic serialization suffices.

use std::fmt::Write as _;

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The digest domain prefix every workspace digest carries.
pub const DOMAIN: &str = "jcs:sha256:";

/// Compute the workspace digest over a serializable snapshot of the staged
/// state: `jcs:sha256:<hex>` of the deterministic `serde_json` encoding.
///
/// ```
/// use dli_serve::author::digest::digest_of;
/// let a = digest_of(&vec!["one", "two"]);
/// // Deterministic: the same state hashes to the same digest.
/// assert_eq!(a, digest_of(&vec!["one", "two"]));
/// assert!(a.starts_with("jcs:sha256:"));
/// // Sensitive: a different state moves the fence.
/// assert_ne!(a, digest_of(&vec!["two", "one"]));
/// ```
#[must_use]
pub fn digest_of<T: Serialize>(snapshot: &T) -> String {
    digest_bytes(&serde_json::to_vec(snapshot).unwrap_or_default())
}

/// Compute `jcs:sha256:<hex>` over raw bytes. The workspace fence uses this to
/// hash a snapshot it assembles from cached per-declaration encodings, so a
/// mutation never re-serializes the unchanged declarations (the incremental
/// reseal — R3/H4); the bytes it hashes are byte-identical to
/// [`digest_of`]'s snapshot encoding, so the digest value is unchanged.
///
/// ```
/// use dli_serve::author::digest::{digest_bytes, digest_of};
/// assert_eq!(digest_bytes(b"[\"one\",\"two\"]"), digest_of(&vec!["one", "two"]));
/// ```
#[must_use]
pub fn digest_bytes(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut out = String::with_capacity(DOMAIN.len() + hash.len() * 2);
    out.push_str(DOMAIN);
    for byte in hash {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
