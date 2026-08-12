//! The authoring seam's digest (`AUTH-1`): the workspace fence.
//!
//! Every mutating `author.*` call answers with a `workspace_digest`, and a
//! caller may fence its next call with `expected_digest`; a mismatch is a
//! `stale-fence` refusal (spec 11 §2). The digest is a content hash over the
//! staged state, so it moves iff the staged state moves.
//!
//! The contract names **RFC 8785 JCS** as the digest domain (fantasy-contract
//! §3.2/§4). Every whole-value digest therefore uses a real JCS serializer:
//! object keys sort by UTF-16 code units, strings and numbers use the RFC's
//! canonical form, and insignificant whitespace is absent. Independent callers
//! can reproduce the exact bytes rather than trusting an engine-private field
//! order.

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
    let bytes = serde_json_canonicalizer::to_vec(snapshot)
        .expect("workspace digest inputs are valid JSON values");
    digest_bytes(&bytes)
}

/// Compute `jcs:sha256:<hex>` over raw bytes. The workspace fence hashes the
/// canonical `{"base_name":…,"decls":[…],"mode":…,"suppositions":…}` string this
/// way; it does so incrementally (a streaming SHA-256 that absorbs each staged
/// declaration once — the incremental reseal, R3/H4/B4), but the bytes hashed are
/// exactly this string, so the digest value is identical to hashing it whole.
///
/// ```
/// use dli_serve::author::digest::{digest_bytes, digest_of};
/// assert_eq!(digest_bytes(b"[\"one\",\"two\"]"), digest_of(&vec!["one", "two"]));
/// ```
#[must_use]
pub fn digest_bytes(bytes: &[u8]) -> String {
    digest_hash(Sha256::digest(bytes).as_slice())
}

/// Render an already-computed SHA-256 hash as `jcs:sha256:<hex>` — the tail the
/// incremental reseal shares with [`digest_bytes`] once it has finalized its
/// streaming hasher, so both paths mint identically-shaped fences.
#[must_use]
pub fn digest_hash(hash: &[u8]) -> String {
    let mut out = String::with_capacity(DOMAIN.len() + hash.len() * 2);
    out.push_str(DOMAIN);
    for byte in hash {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::{digest_bytes, digest_of};

    #[derive(Serialize)]
    struct OutOfOrder<'a> {
        z: &'a str,
        a: f64,
    }

    #[test]
    fn digest_is_over_rfc_8785_bytes() {
        let value = OutOfOrder { z: "last", a: 1.0 };
        assert_eq!(digest_of(&value), digest_bytes(br#"{"a":1,"z":"last"}"#));
    }
}
