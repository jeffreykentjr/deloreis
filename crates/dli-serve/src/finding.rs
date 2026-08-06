//! Findings and provenance shared across op results.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_core::grade::Grade;

/// A finding's severity. Orthogonal to grade (evidence strength): severity gates
/// `author.materialize` — an `error` blocks it, a `warn` passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Blocks `author.materialize`.
    Error,
    /// Advisory; does not block materialization.
    Warn,
}

/// A single finding attached to a result: a coded, graded observation with an
/// optional machine-applicable repair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    /// The severity, which gates materialization.
    pub severity: Severity,
    /// A stable finding code.
    pub code: String,
    /// A human-readable message. Serialized with every control, bidi, and
    /// zero-width character escaped (DL-0061): this message is the LS-5 review
    /// surface a reviewer reads, and caller-supplied text (ids, values) is
    /// `format!`-ed into it — the escape is what stops a newline in a caller id
    /// from forging a second line, exactly as the render channel escapes the
    /// `.dli` diff (DM-4). See the `serialize_message` function.
    #[serde(serialize_with = "serialize_message")]
    pub message: String,
    /// The grade at which the finding is asserted.
    pub grade: Grade,
    /// An optional structured repair suggestion.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub repair: Option<serde_json::Value>,
    /// Supporting citations.
    #[serde(default)]
    pub citations: Vec<String>,
}

/// Escape every control, bidi, and zero-width character in a finding message at
/// the serialization boundary, so caller-supplied text embedded in the message
/// can never forge or reorder a line in the review surface a reviewer reads.
///
/// This is the finding-channel sibling of the render channel's display escape
/// (DM-4): it reuses the very same [`escape_controls`] sanitizer, applied once at
/// the single point every `Finding` — from any builder, in any result struct,
/// now or future — crosses to the wire. The finding **shape** is untouched: only
/// the `message` field's bytes change, and only for the forbidden characters, so
/// a well-formed message serializes byte-identically (DL-0061).
///
/// [`escape_controls`]: crate::author::render::escape_controls
fn serialize_message<S>(message: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&crate::author::render::escape_controls(message))
}

/// Provenance supplied when adding authored material. Internally tagged on
/// `kind`; origin tracking is load-bearing for Door T (nothing an agent supplies
/// mints authority).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Provenance {
    /// Cited to one or more decisions: `{"kind":"cited","decisions":["D-0104"]}`.
    Cited {
        /// The cited decision identifiers in the consuming repo's ledger.
        decisions: Vec<String>,
    },
    /// Proposed (not yet ratified): `{"kind":"proposed","origin":"session-7c9"}`.
    Proposed {
        /// The origin token of the proposal (e.g. an agent session id).
        origin: String,
    },
    /// Conditional on a hypothesis: `{"kind":"hypothesis","hyp":"coast-runs-north"}`.
    Hypothesis {
        /// The hypothesis the material is conditioned on.
        hyp: String,
    },
}

#[cfg(test)]
mod tests {
    use dli_core::grade::Grade;

    use super::{Finding, Provenance, Severity};

    /// The message-escape boundary lives on the `Finding` type, so it covers
    /// every finding on the wire regardless of which builder or tool produced it
    /// (DL-0061). This pins the boundary itself: a control character and a bidi
    /// override embedded in a message serialize as their escapes, never raw, so
    /// caller text `format!`-ed into a message can neither forge a second review
    /// line nor silently reorder one. Remove the `serialize_with` and this reds.
    #[test]
    fn message_is_control_char_escaped_on_the_wire() {
        let finding = Finding {
            severity: Severity::Error,
            code: "test.code".to_owned(),
            message: "ghost\nforged \u{202e}reversed".to_owned(),
            grade: Grade::Checked,
            repair: None,
            citations: Vec::new(),
        };
        let wire = serde_json::to_value(&finding).expect("a finding serializes");
        let message = wire["message"].as_str().expect("message is a string");
        assert!(
            !message.contains('\n'),
            "a raw newline survives to the wire and can forge a line: {message:?}"
        );
        assert!(
            !message
                .chars()
                .any(crate::author::render::is_forbidden_char),
            "a forbidden character reaches the reviewer raw: {message:?}"
        );
        assert_eq!(
            message, "ghost\\nforged \\u{202e}reversed",
            "the message must carry the exact control-char escapes"
        );
        // The finding SHAPE is untouched — only the message bytes changed.
        assert_eq!(wire["severity"], serde_json::json!("error"));
        assert_eq!(wire["code"], serde_json::json!("test.code"));
        assert_eq!(wire["grade"], serde_json::json!("checked"));
    }

    /// A well-formed message with no forbidden character serializes
    /// byte-identically — the escape is a strict no-op on clean text, so every
    /// released fixture's finding message stays exactly as pinned (DL-0061).
    #[test]
    fn a_clean_message_serializes_unchanged() {
        let finding = Finding {
            severity: Severity::Warn,
            code: "name.unvetted".to_owned(),
            message: "The name \"mara\" has not passed the naming oracle.".to_owned(),
            grade: Grade::Asserted,
            repair: None,
            citations: Vec::new(),
        };
        let wire = serde_json::to_value(&finding).expect("a finding serializes");
        assert_eq!(
            wire["message"],
            serde_json::json!("The name \"mara\" has not passed the naming oracle.")
        );
    }

    /// H2: an unknown field in a nested `provenance` object is rejected at
    /// deserialize time, not silently dropped — the `deny_unknown_fields` on the
    /// internally-tagged enum must enforce at runtime, not only in the schema.
    #[test]
    fn provenance_rejects_unknown_nested_fields() {
        assert!(
            serde_json::from_str::<Provenance>(r#"{"kind":"proposed","origin":"o"}"#).is_ok(),
            "a well-formed provenance must deserialize"
        );
        assert!(
            serde_json::from_str::<Provenance>(r#"{"kind":"proposed","origin":"o","evil":1}"#)
                .is_err(),
            "an unknown field inside provenance must be refused (H2 nested rejection)"
        );
    }
}
