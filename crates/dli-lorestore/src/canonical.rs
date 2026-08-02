//! JCS canonical bytes and SHA-256 content addressing.
//!
//! This module reimplements the idiom from `dli_serve::evidence`
//! (`canonical_bytes`, `digest_bytes`, `validate_content_address`) so that
//! `dli-lorestore` is self-contained for the content-addressing rule (DL-0041,
//! roadmap §LS-1). Every stored object serializes to RFC 8785 JCS bytes, and
//! its address is `sha256:<hex>` of those bytes — the canonical form that
//! makes content-addressing sound (equal provenance → equal bytes → equal
//! hash, because `PosBool` absorption-normalized DNF is unique).
//!
//! **No floats anywhere in stored objects.** JCS inherits ECMAScript number
//! serialization; the safe rule is integers and strings only.

use std::path::Path;

use serde::Serialize;
use serde_json_canonicalizer;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::identity::ContentHash;

/// One canonical-bytes or content-address computation failed.
#[derive(Debug, Error)]
pub enum CanonicalError {
    /// The value could not be encoded as JCS (a NaN or infinite float, a
    /// non-string key, or similar) — or it violates the storage rule that
    /// stored objects are integers and strings only. Note the JCS encoder
    /// itself accepts *finite* floats; the no-floats rule is enforced by the
    /// typed object constructors (no float-bearing fields) and by
    /// `EvidenceEnvelope::new` for opaque payloads.
    #[error("value cannot be encoded as JCS: {0}")]
    Encode(String),
    /// An on-disk object's filename does not match its content hash.
    #[error("{path}: content address mismatch — expected {expected}, got {actual}")]
    AddressMismatch {
        /// The path being accessed.
        path: String,
        /// The hash encoded in the filename.
        expected: String,
        /// The hash computed from the file's bytes.
        actual: String,
    },
}

/// Canonical JCS bytes for a serializable value.
///
/// Uses `serde_json_canonicalizer` (the same crate `dli_serve::evidence`
/// uses) so stored objects are byte-identical to evidence records.
pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CanonicalError> {
    serde_json_canonicalizer::to_vec(value)
        .map_err(|error| CanonicalError::Encode(error.to_string()))
}

/// A `sha256:<hex>` digest over raw bytes.
///
/// This is the low-level hash primitive — the same function used by
/// `dli_serve::evidence::digest_bytes`, reimplemented here so the store crate
/// is self-contained.
pub fn digest_bytes(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut out = String::with_capacity(71);
    out.push_str("sha256:");
    for byte in hash {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The `sha256:<hex>` content hash of a value's JCS canonical serialization.
///
/// This is the content-addressing identity: the hash of an object's content
/// fields, never of any parent pointer or metadata. Two objects with the same
/// content produce the same hash; the hash is the store key.
pub fn content_hash<T: Serialize>(value: &T) -> Result<ContentHash, CanonicalError> {
    let bytes = canonical_bytes(value)?;
    Ok(ContentHash(digest_bytes(&bytes)))
}

/// Validate that a file's on-disk bytes match the content hash encoded in
/// its filename.
///
/// Mirrors `dli_serve::evidence::validate_content_address`: the file's
/// bytes are JCS-decoded to the value type (structural check), the hash is
/// recomputed from the raw bytes (content-address check), and the two are
/// compared. A mismatch is corruption — the object was tampered with or
/// the filename is wrong.
pub fn validate_content_address<T: Serialize + serde::de::DeserializeOwned>(
    path: &Path,
    bytes: &[u8],
    expected: &ContentHash,
) -> Result<(), CanonicalError> {
    let actual = digest_bytes(bytes);
    if actual != expected.0 {
        return Err(CanonicalError::AddressMismatch {
            path: path.display().to_string(),
            expected: expected.0.clone(),
            actual,
        });
    }
    // Structural check: the bytes must deserialize to the claimed type.
    // (A content hash collision on adversarial input is negligible; this is
    // the belt-and-suspenders check for accidental corruption.)
    let _ = serde_json::from_slice::<T>(bytes)
        .map_err(|error| CanonicalError::Encode(format!("malformed JSON: {error}")))?;
    Ok(())
}

/// Compute the hex (no prefix) SHA-256 digest of a string — used to mint
/// reproducible `ClaimId`s without an RNG (v1 sovereign deployments).
pub(crate) fn sha256_hex(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}
