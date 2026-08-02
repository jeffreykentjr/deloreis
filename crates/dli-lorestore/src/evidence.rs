//! Evidence envelopes — durable, content-addressed evidence stored alongside
//! fact objects (DL-0041, roadmap §LS-1, risk register §7).
//!
//! During the dual-backend cutover (LS-0..LS-8), evidence lives in both
//! `.dli-evidence/` and the lorestore. The mitigation recorded in the risk
//! register is that **both use the same JCS + SHA-256 addressing**, so content
//! addresses are identical and migration is a move, not a rewrite. This module
//! provides the lorestore-side envelope type that implements that addressing.
//!
//! An [`EvidenceEnvelope`] wraps an evidence payload (an attestation record,
//! a simulation manifest, a proof transcript) with authority metadata and a
//! content digest. The payload is carried as opaque JSON — the envelope is the
//! integrity layer, not the schema of the evidence itself.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION;
use crate::identity::{Authority, ContentHash};

/// The kind of evidence carried in an [`EvidenceEnvelope`].
///
/// Named so the store can route envelopes to the right verifier without
/// re-parsing the payload. Each kind corresponds to a durable evidence class
/// in the existing `dli-serve` evidence store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKind {
    /// A recorded model judgment on a bounded question (`Attestation`).
    Attestation,
    /// A draft payload shadow (`Payload`).
    Payload,
    /// A simulation run manifest (`SimulateStartResult` / `AttestationSweepResult`).
    Simulation,
    /// A spoken-proof transcript (a `ProofReport`'s derivation).
    Proof,
    /// Free-form evidence: anything else committed to the store with a digest.
    Other,
}

/// An evidence envelope: immutable evidence addressed by SHA-256.
///
/// The envelope wraps a `serde_json::Value` payload (opaque to the store)
/// with the standard lorestore identity fields: `schema_version`, `authority`,
/// and a `payload_digest` that is the SHA-256 of the payload's JCS bytes.
/// Readers verify the envelope's content hash (computed over the envelope
/// itself) and the payload digest (computed over the payload) before exposing
/// any record — the same discipline as `dli_serve::evidence`.
///
/// The envelope's own content hash is what the store addresses; the
/// `payload_digest` is a *second* independent check that the payload hasn't
/// drifted inside its envelope. Both use RFC 8785 JCS canonicalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceEnvelope {
    /// Schema version; bumped when the stored shape changes.
    pub schema_version: u32,
    /// The authority behind this evidence record (unset in sovereign
    /// deployments). Participates in the content hash.
    pub authority: Authority,
    /// What this evidence is about — a stable identifier the verifier keys on
    /// (e.g. a document name, a question key, a question slug).
    pub subject: String,
    /// The evidence type, for routing without re-parsing the payload.
    pub kind: EvidenceKind,
    /// The evidence content, carried as opaque JSON. The envelope does not
    /// interpret this — it is the integrity layer, not the schema.
    pub payload: serde_json::Value,
    /// The `sha256:<hex>` digest of the payload's JCS canonical bytes.
    /// Computed at envelope construction time; verified on read.
    pub payload_digest: ContentHash,
}

impl EvidenceEnvelope {
    /// Construct an evidence envelope from a payload, computing its digest.
    ///
    /// The `payload_digest` is the SHA-256 of the payload's JCS serialization
    /// — the same canonicalization the envelope itself uses, so the two addresses
    /// are computed by the same code path.
    ///
    /// The payload is the one door where arbitrary JSON enters the store, so
    /// the no-floats storage rule (roadmap §LS-1) is **enforced** here: any
    /// number that is not an integer is refused. Future confidence values
    /// enter as decimal strings.
    pub fn new(
        authority: Authority,
        subject: impl Into<String>,
        kind: EvidenceKind,
        payload: serde_json::Value,
    ) -> Result<Self, crate::canonical::CanonicalError> {
        reject_floats(&payload, "$")?;
        let payload_digest = crate::canonical::content_hash(&payload)?;
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            authority,
            subject: subject.into(),
            kind,
            payload,
            payload_digest,
        })
    }

    /// The content hash of this envelope — over all fields including
    /// `payload_digest`. Used as the store key.
    pub fn content_hash(&self) -> Result<ContentHash, crate::canonical::CanonicalError> {
        crate::canonical::content_hash(self)
    }

    /// Verify that the stored `payload_digest` matches the payload's actual
    /// JCS hash. A mismatch means the payload was tampered with after the
    /// envelope was constructed.
    pub fn verify_payload(&self) -> Result<(), crate::canonical::CanonicalError> {
        let actual = crate::canonical::content_hash(&self.payload)?;
        if actual != self.payload_digest {
            return Err(crate::canonical::CanonicalError::AddressMismatch {
                path: self.subject.clone(),
                expected: self.payload_digest.0.clone(),
                actual: actual.0,
            });
        }
        Ok(())
    }
}

/// Enforce the no-floats storage rule on an arbitrary JSON payload: JCS
/// happily serializes finite floats, so the guard must live here, at the one
/// place unconstrained JSON enters the store.
fn reject_floats(
    value: &serde_json::Value,
    path: &str,
) -> Result<(), crate::canonical::CanonicalError> {
    match value {
        serde_json::Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                Ok(())
            } else {
                Err(crate::canonical::CanonicalError::Encode(format!(
                    "payload contains a non-integer number at {path}: {n} — stored objects are integers and strings only; carry decimals as strings"
                )))
            }
        }
        serde_json::Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                reject_floats(item, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                reject_floats(item, &format!("{path}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
