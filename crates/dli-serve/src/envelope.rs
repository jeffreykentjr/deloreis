//! The request/response envelope.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ops::Op;

/// A wire request frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Request {
    /// Protocol tag (the contract version marker).
    pub dli: String,
    /// Correlation id echoed back on the response.
    pub id: String,
    /// The operation.
    pub op: Op,
    /// The op-specific parameters (validated against the op's request schema).
    #[serde(default)]
    pub params: serde_json::Value,
    /// Optimistic-concurrency fence for mutating `author.*` calls: the caller's
    /// last-seen workspace digest. A mismatch is a `stale-fence` error.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub expected_digest: Option<String>,
    /// Retry-safe key so a re-sent mutating call is applied at most once.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub idempotency_key: Option<String>,
}

/// A wire response frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Response {
    /// Protocol tag, echoed from the request.
    pub dli: String,
    /// Correlation id, echoed from the request.
    pub id: String,
    /// Whether the op succeeded.
    pub ok: bool,
    /// The op-specific result, present when `ok`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub result: Option<serde_json::Value>,
    /// The error body, present when not `ok`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<ErrorBody>,
}

/// A structured error body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorBody {
    /// The machine-readable error code.
    pub code: ErrorCode,
    /// A human-readable message.
    pub message: String,
    /// Optional structured detail.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data: Option<serde_json::Value>,
}

/// The closed set of wire error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    /// The request frame was malformed (missing/invalid envelope fields).
    BadFrame,
    /// The op name is not recognized.
    UnknownOp,
    /// Params (or an expected result) failed schema validation.
    SchemaViolation,
    /// A referenced document or workspace does not exist.
    UnknownDoc,
    /// The program could not be stratified.
    Unstratifiable,
    /// Evaluation derived a contradiction.
    Contradiction,
    /// A submitted fence (e.g. workspace digest) no longer matches.
    StaleFence,
    /// `author.materialize` was called with error-class findings open; the
    /// blocking finding codes ride in the error data.
    MaterializeBlocked,
    /// A killed or renamed op/tool: refused, with the message naming the
    /// successor. Distinct from `unknown-op` (a form the contract never defined);
    /// a killed form the contract once defined and deliberately retired.
    KilledForm,
    /// A recognized op whose engine is not wired in this build — capability-gated
    /// (the wired set is advertised in `dli.version`'s `capabilities`). A
    /// permanent-until-wired refusal, never a server defect.
    NotWired,
    /// A well-formed frame that declared a contract version this build does not
    /// speak. Refused, not a bad frame — the envelope parsed cleanly.
    VersionMismatch,
    /// An `idempotency_key` was reused with a *different* payload. Distinct from
    /// `stale-fence` (the digest moved): the key is bound to its first payload,
    /// and a conflicting reuse is refused rather than silently replayed.
    Conflict,
    /// An unexpected internal error.
    Internal,
}

impl ErrorCode {
    /// Every error code, in declaration order. The single source of truth for
    /// the closed code set (consumers derive their lists from this).
    pub const ALL: [ErrorCode; 13] = [
        ErrorCode::BadFrame,
        ErrorCode::UnknownOp,
        ErrorCode::SchemaViolation,
        ErrorCode::UnknownDoc,
        ErrorCode::Unstratifiable,
        ErrorCode::Contradiction,
        ErrorCode::StaleFence,
        ErrorCode::MaterializeBlocked,
        ErrorCode::KilledForm,
        ErrorCode::NotWired,
        ErrorCode::VersionMismatch,
        ErrorCode::Conflict,
        ErrorCode::Internal,
    ];

    /// The kebab-case wire name for this code (matches the serde encoding).
    #[must_use]
    pub fn wire_name(self) -> &'static str {
        match self {
            ErrorCode::BadFrame => "bad-frame",
            ErrorCode::UnknownOp => "unknown-op",
            ErrorCode::SchemaViolation => "schema-violation",
            ErrorCode::UnknownDoc => "unknown-doc",
            ErrorCode::Unstratifiable => "unstratifiable",
            ErrorCode::Contradiction => "contradiction",
            ErrorCode::StaleFence => "stale-fence",
            ErrorCode::MaterializeBlocked => "materialize-blocked",
            ErrorCode::KilledForm => "killed-form",
            ErrorCode::NotWired => "not-wired",
            ErrorCode::VersionMismatch => "version-mismatch",
            ErrorCode::Conflict => "conflict",
            ErrorCode::Internal => "internal",
        }
    }
}

impl From<dli_eval::error::EvalError> for ErrorCode {
    fn from(error: dli_eval::error::EvalError) -> Self {
        use dli_eval::error::EvalError as E;
        match error {
            E::Unstratifiable { .. } => ErrorCode::Unstratifiable,
            E::Contradiction { .. } => ErrorCode::Contradiction,
            E::UnknownPredicate { .. } => ErrorCode::SchemaViolation,
        }
    }
}
