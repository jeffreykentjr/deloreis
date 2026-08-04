//! Identifiers.

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::span::Span;

/// A named identifier with its source span.
///
/// # Two accepted spellings, one serialization
///
/// An identifier that came from `.dli` text carries a real [`Span`] and always
/// serializes it, so every stored fact object and every response is the bytes
/// it has always been. But an identifier a *caller* writes on the wire has no
/// source to point into — a typed `goal_atom` is not text and never was — and
/// making such a caller spell `{"file":0,"start":0,"end":0}` beside every name
/// asks them to invent a location for something that has none.
///
/// So deserialization accepts either spelling and serialization has only one:
///
/// - `"serves"` — the terse form. The span is synthetic.
/// - `{"text": "serves"}` — the span defaults to synthetic.
/// - `{"text": "serves", "span": {…}}` — the full form, what the parser emits.
///
/// This is additive in the direction that matters: nothing that round-tripped
/// before changes, because [`Serialize`] is still derived and still writes the
/// span unconditionally. Only the set of *accepted* inputs grows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Ident {
    /// The identifier text as written.
    pub text: String,
    /// Where the identifier appears in source.
    pub span: Span,
}

impl Ident {
    /// Construct an identifier at a synthetic span.
    #[must_use]
    pub fn synthetic(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            span: Span::synthetic(),
        }
    }
}

impl<'de> Deserialize<'de> for Ident {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(IdentVisitor)
    }
}

/// Accepts the bare text or the `{text, span?}` object.
///
/// Hand-written rather than `#[serde(untagged)]` on purpose: `untagged`
/// collapses every failure into "data did not match any variant", which would
/// replace serde's own `missing field \u{60}text\u{60}` on a malformed
/// identifier — a message the `0.x` wire has been answering with all along.
/// This visitor keeps serde's wording exactly, including for a duplicate key,
/// and ignores unknown keys precisely as the derive did (the server's
/// normalized round-trip is what reports those, with a path).
struct IdentVisitor;

impl<'de> Visitor<'de> for IdentVisitor {
    type Value = Ident;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an identifier: its text, or an object with `text` and optional `span`")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Ident, E> {
        Ok(Ident::synthetic(text))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ident, A::Error> {
        let mut text: Option<String> = None;
        let mut span: Option<Span> = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "text" => {
                    if text.is_some() {
                        return Err(de::Error::duplicate_field("text"));
                    }
                    text = Some(map.next_value()?);
                }
                "span" => {
                    if span.is_some() {
                        return Err(de::Error::duplicate_field("span"));
                    }
                    span = Some(map.next_value()?);
                }
                _ => {
                    map.next_value::<de::IgnoredAny>()?;
                }
            }
        }
        Ok(Ident {
            text: text.ok_or_else(|| de::Error::missing_field("text"))?,
            span: span.unwrap_or_else(Span::synthetic),
        })
    }
}

impl JsonSchema for Ident {
    fn schema_name() -> Cow<'static, str> {
        "Ident".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A named identifier. Either the bare text, whose span is \
                            synthetic, or an object carrying the text and an optional span.",
            "oneOf": [
                { "type": "string" },
                {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        "span": <Span as JsonSchema>::json_schema(generator),
                    },
                    "required": ["text"],
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Serialization is untouched: an identifier still writes its span, always
    /// and unconditionally, so every stored fact object and every recorded
    /// response is the bytes it was.
    #[test]
    fn serialization_still_writes_the_span() {
        let ident = Ident {
            text: "serves".to_owned(),
            span: Span::synthetic(),
        };
        assert_eq!(
            serde_json::to_string(&ident).unwrap(),
            r#"{"text":"serves","span":{"file":0,"start":0,"end":0}}"#
        );
    }

    /// All three accepted spellings land on the same identifier, and the full
    /// form still round-trips byte-for-byte.
    #[test]
    fn every_spelling_decodes_to_the_same_identifier() {
        let full = r#"{"text":"serves","span":{"file":0,"start":0,"end":0}}"#;
        let expected = Ident::synthetic("serves");
        for encoded in [r#""serves""#, r#"{"text":"serves"}"#, full] {
            assert_eq!(
                serde_json::from_str::<Ident>(encoded).unwrap(),
                expected,
                "{encoded}"
            );
        }
        let decoded: Ident = serde_json::from_str(full).unwrap();
        assert_eq!(serde_json::to_string(&decoded).unwrap(), full);
    }

    /// A real span survives the terse-form addition — the parser's output is
    /// not quietly flattened to synthetic.
    #[test]
    fn a_real_span_round_trips() {
        let encoded = r#"{"text":"serves","span":{"file":2,"start":17,"end":23}}"#;
        let decoded: Ident = serde_json::from_str(encoded).unwrap();
        assert_eq!(decoded.span.start, 17);
        assert_eq!(serde_json::to_string(&decoded).unwrap(), encoded);
    }

    /// serde's own wording survives: a malformed identifier reports the missing
    /// field by name, not `untagged`'s anonymous "did not match any variant".
    #[test]
    fn a_malformed_identifier_keeps_serdes_message() {
        // Through `from_value`, the path the server decodes params on — where
        // serde adds no line/column and the message is the bare wording the
        // wire has always carried.
        let error = serde_json::from_value::<Ident>(
            serde_json::json!({ "span": { "file": 0, "start": 0, "end": 0 } }),
        )
        .expect_err("`text` is required in the object form");
        assert_eq!(error.to_string(), "missing field `text`");

        let error = serde_json::from_str::<Ident>("7").expect_err("a number is not an identifier");
        assert!(
            error.to_string().contains("an identifier"),
            "the expectation names what an identifier is: {error}"
        );
    }
}
