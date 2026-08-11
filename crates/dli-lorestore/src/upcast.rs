//! Typed upcast-on-read: old schema versions are lifted to the current shape at
//! the load boundary (roadmap §LS-1 "upcast on read", §LS-9 expert-tier
//! structuring).
//!
//! This is the **one place** the store re-enters the front end. Deepening a v1
//! string-bodied expert carriage to its v2 typed interior is a load-boundary op
//! — exactly like ingesting `.dli` text — so it is allowed to reuse
//! [`dli_syntax::structure_rule_body`]. Default reads of an already-current
//! object skip this module entirely and stay pure deserialization; the parser is
//! never on the v2 read path.
//!
//! Because the deepening is the store's *only* parser edge, LS-7 puts it behind
//! the off-by-default `syntax` feature. A default build reads v2 natively and
//! lifts every v1 object whose v2 form is byte-identical (gentle facts, commits,
//! refs, edges, evidence, and the carriages LS-9 did not restructure); a v1
//! object that genuinely needs the front end refuses with
//! [`UpcastError::SyntaxRequired`] rather than being silently mis-lifted.
//!
//! No stored fact is rewritten: the deepening happens in memory on the value
//! read from disk. The old bytes keep their v1 content address; the lifted
//! in-memory object is stamped with the current [`SCHEMA_VERSION`].

use serde_json::Value;

#[cfg(feature = "syntax")]
use dli_ast::annot::Annotation;
#[cfg(feature = "syntax")]
use dli_ast::clause::Clause;
#[cfg(feature = "syntax")]
use dli_ast::decl::{
    Decl, DialectDecl, NameDecl, PracticeDecl, ProofDecl, RatifyDecl, RuleDecl, RuleDomain,
};
#[cfg(feature = "syntax")]
use dli_ast::ident::Ident;

use crate::SCHEMA_VERSION;

/// A stored object shape that this build cannot lift to the current schema.
#[derive(Debug, thiserror::Error)]
pub enum UpcastError {
    /// A v1 carriage's embedded declaration JSON did not decode for deepening.
    #[error("v1 carriage could not be deepened to the current schema: {0}")]
    Carriage(String),
    /// A v1 expert carriage whose interior only the front end can deepen was
    /// read by a build without the parser (LS-7). The store is not corrupt —
    /// this build simply cannot lift it.
    #[error(
        "v1 `{kind}` carriage needs the front end to deepen its interior, and this build links \
         no parser; read this store with a `syntax`-enabled build (or write it forward to v2)"
    )]
    SyntaxRequired {
        /// The expert carriage kind that could not be deepened.
        kind: String,
    },
}

/// The expert carriage kinds whose v1 interior is a `Vec<Clause>` of text that
/// only the front end can structure. Every other v1 object's v2 form is
/// byte-identical, so lifting it is just the version stamp. This is the mirror
/// of the six `deepen_*_carriage` functions, consulted only by the build that
/// has none of them.
#[cfg(not(feature = "syntax"))]
const DEEPENED_CARRIAGE_KINDS: &[&str] =
    &["rule", "dialect", "name", "ratify", "proof", "practice"];

/// The `kind` of a fact object's expert carriage, if it has one.
#[cfg(not(feature = "syntax"))]
fn expert_carriage_kind(value: &Value) -> Option<&str> {
    value
        .get("expert_body")?
        .get("kind")
        .and_then(Value::as_str)
}

/// The v1 shape of a [`RuleDecl`]: the body is the old `Vec<Clause>` of
/// whitespace-normalized text, before LS-9 structured it into a `RuleBody`.
/// Field order and names mirror the v1 serialization exactly so serde reads an
/// old rule carriage verbatim.
#[cfg(feature = "syntax")]
#[derive(serde::Deserialize)]
struct RuleDeclV1 {
    name: Ident,
    domain: Option<RuleDomain>,
    #[serde(default)]
    head: Vec<String>,
    #[serde(default)]
    body: Vec<Clause>,
    provenance: Option<Annotation>,
}

/// The v1 shape of a [`ProofDecl`]: the body is the old `Vec<Clause>` of
/// whitespace-normalized text, before LS-9 structured it into a `ProofBody`.
#[cfg(feature = "syntax")]
#[derive(serde::Deserialize)]
struct ProofDeclV1 {
    name: Ident,
    #[serde(default)]
    head: Vec<String>,
    #[serde(default)]
    body: Vec<Clause>,
    #[serde(default)]
    provenance: Option<Annotation>,
}

/// The v1 shape of a [`PracticeDecl`]: the body is the old `Vec<Clause>` of
/// whitespace-normalized interior text, before LS-9 structured it into a
/// `PracticeBody`. Field order and names mirror the v1 serialization exactly so
/// serde reads an old practice carriage verbatim.
#[cfg(feature = "syntax")]
#[derive(serde::Deserialize)]
struct PracticeDeclV1 {
    name: Ident,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    body: Vec<Clause>,
    provenance: Option<Annotation>,
}

/// Lift a stored object's JSON `value` from `from_version` to the current
/// [`SCHEMA_VERSION`].
///
/// v1 → v2 deepens each expert-tier carriage whose interior LS-9 structured —
/// **rule**, **dialect**, **name**, **ratify**, **proof**, and **practice** — from a
/// `Vec<Clause>` string body to its typed interior, via the front-end lowering.
/// Every other v1 object (gentle facts, other carriages, commits, refs, edges,
/// evidence) is byte-identical to its v2 form, so the lift is just the version
/// stamp. Each deepen is a no-op on a carriage of the wrong kind, so applying
/// every one to every object is safe. A `from_version` already at (or above)
/// the current schema is returned unchanged.
///
/// Without the `syntax` feature there is no front end to deepen with, so a v1
/// object that *is* one of those carriages refuses with
/// [`UpcastError::SyntaxRequired`]; every other v1 object still lifts, because
/// its lift was only ever the version stamp.
pub fn upcast_value(from_version: u64, mut value: Value) -> Result<Value, UpcastError> {
    if from_version >= u64::from(SCHEMA_VERSION) {
        return Ok(value);
    }

    // v1 → v2: deepen every expert carriage whose interior LS-9 structured.
    // A v2/v3 carriage is already typed; running a v1 deepener against it would
    // reinterpret its typed body as the old Vec<Clause> shape and reject a
    // valid existing store. Each deepener is a no-op unless the carriage is
    // that kind, so at most one fires per object.
    if from_version < 2 {
        #[cfg(feature = "syntax")]
        {
            deepen_rule_carriage(&mut value)?;
            deepen_dialect_carriage(&mut value)?;
            deepen_name_carriage(&mut value)?;
            deepen_ratify_carriage(&mut value)?;
            deepen_proof_carriage(&mut value)?;
            deepen_practice_carriage(&mut value)?;
        }
        #[cfg(not(feature = "syntax"))]
        if let Some(kind) = expert_carriage_kind(&value)
            && DEEPENED_CARRIAGE_KINDS.contains(&kind)
        {
            return Err(UpcastError::SyntaxRequired {
                kind: kind.to_owned(),
            });
        }
    }

    // v2 → v3: take source position out of the identity bytes (DL-0055).
    // Pure serde on both build lanes — no parser edge.
    if from_version < 3 {
        normalize_carriage_spans(&mut value);
    }

    // v3 → v4: retire `decided` as a stored vocabulary. Keep the AST variant
    // readable for old frames, but normalize it before any current projection
    // or authoring path can observe it. Provenance is outside the identity
    // projection, so this changes no fact address.
    if from_version < 4 {
        normalize_decided_marker(&mut value);
    }

    // Stamp the lifted object with the schema it now conforms to.
    if let Some(obj) = value.as_object_mut() {
        obj.insert("schema_version".to_owned(), Value::from(SCHEMA_VERSION));
    }
    Ok(value)
}

fn normalize_decided_marker(value: &mut Value) {
    if let Some(marker) = value
        .get_mut("provenance")
        .and_then(Value::as_object_mut)
        .and_then(|provenance| provenance.get_mut("marker"))
        && marker.as_str() == Some("Decided")
    {
        *marker = Value::String("Provisional".to_owned());
    }
}

/// Lift a pre-v3 carriage into the v3 span discipline: the hashed body becomes
/// span-normalized and the located original moves into `provenance`.
///
/// This is what makes an existing store the **same lineage** rather than a
/// foreign one. A v2 object hashed its located body, so its identity carried
/// source offsets; normalizing here lands it on exactly the identity a freshly
/// lowered v3 object has, which is what `ensure_pack_imported` compares
/// (`lorestore.rs`). No marker is invented — v2 never recorded one, and an
/// absent marker is honest where a guessed one would not be.
fn normalize_carriage_spans(value: &mut Value) {
    let Some(first) = value
        .get_mut("expert_body")
        .and_then(|expert| expert.get_mut("body"))
        .and_then(Value::as_array_mut)
        .and_then(|body| body.first_mut())
    else {
        return;
    };
    let Some(decl_json) = first.as_str() else {
        return;
    };
    let Ok(mut decl_value) = serde_json::from_str::<Value>(decl_json) else {
        // Not a JSON body (a v1 clause-text carriage this build left alone):
        // there is nothing span-shaped to normalize.
        return;
    };
    // Re-serialize the parsed value for the located side too, so the comparison
    // below turns on spans alone and not on how the two strings were produced.
    let Ok(located) = serde_json::to_string(&decl_value) else {
        return;
    };
    crate::lower::normalize_spans(&mut decl_value);
    let Ok(normalized) = serde_json::to_string(&decl_value) else {
        return;
    };
    // Always rewrite the hashed body from the re-serialized value, even when no
    // span moved. The deepening above emits its result straight from the `Decl`
    // struct, and a freshly lowered v3 carriage emits from a `Value`; the two
    // routes can order keys differently, and the body is inside the content
    // hash, so an upcast object would otherwise fail to match the fresh object
    // it is supposed to be identical to. One route for both.
    *first = Value::from(normalized.clone());
    if normalized == located {
        return; // already position-free; no located copy worth carrying
    }
    if let Some(obj) = value.as_object_mut() {
        let provenance = obj
            .entry("provenance")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(map) = provenance.as_object_mut() {
            // Fill in only. An object that already carries a located body was
            // recorded with one at authoring time, and that original outranks
            // anything reconstructed here — deepening a v1 interior re-derives
            // the body, so overwriting would replace real source positions with
            // the re-parse's own.
            map.entry("located_body".to_owned())
                .or_insert_with(|| Value::from(located));
        }
    }
}

/// If `value` is a fact object carrying a v1 `rule` expert body, rewrite the
/// embedded declaration JSON in place so its body is the typed `RuleBody`. A
/// value that is not a rule carriage is left untouched.
#[cfg(feature = "syntax")]
fn deepen_rule_carriage(value: &mut Value) -> Result<(), UpcastError> {
    let Some(expert) = value.get_mut("expert_body") else {
        return Ok(());
    };
    if expert.get("kind").and_then(Value::as_str) != Some("rule") {
        return Ok(());
    }
    let Some(first) = expert
        .get_mut("body")
        .and_then(Value::as_array_mut)
        .and_then(|arr| arr.first_mut())
    else {
        return Ok(());
    };
    let Some(decl_json) = first.as_str() else {
        return Ok(());
    };

    let decl_value: Value =
        serde_json::from_str(decl_json).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    // The embedded declaration is an externally-tagged `Decl`; only the `Rule`
    // variant needs deepening.
    let Some(rule_v) = decl_value.get("Rule").cloned() else {
        return Ok(());
    };
    let v1: RuleDeclV1 =
        serde_json::from_value(rule_v).map_err(|e| UpcastError::Carriage(e.to_string()))?;

    let body = dli_syntax::structure_rule_body(&v1.body, &v1.name, &v1.head);
    let v2 = Decl::Rule(RuleDecl {
        name: v1.name,
        domain: v1.domain,
        head: v1.head,
        body,
        provenance: v1.provenance,
    });
    let deepened = serde_json::to_string(&v2).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    *first = Value::String(deepened);
    Ok(())
}

/// The v1 shape of a [`DialectDecl`]: the body is the old `Vec<Clause>` string
/// body, before LS-9 structured it into a `DialectBody`.
#[cfg(feature = "syntax")]
#[derive(serde::Deserialize)]
struct DialectDeclV1 {
    name: Ident,
    label: Option<String>,
    #[serde(default)]
    body: Vec<Clause>,
    provenance: Option<Annotation>,
}

/// If `value` is a v1 `dialect` carriage, deepen its embedded declaration's
/// string body to the typed `DialectBody` via the front-end lowering.
#[cfg(feature = "syntax")]
fn deepen_dialect_carriage(value: &mut Value) -> Result<(), UpcastError> {
    let Some(first) = carriage_decl_json(value, "dialect") else {
        return Ok(());
    };
    let Some(decl_json) = first.as_str() else {
        return Ok(());
    };
    let decl_value: Value =
        serde_json::from_str(decl_json).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    let Some(dialect_v) = decl_value.get("Dialect").cloned() else {
        return Ok(());
    };
    let v1: DialectDeclV1 =
        serde_json::from_value(dialect_v).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    let v2 = Decl::Dialect(DialectDecl {
        name: v1.name,
        label: v1.label,
        body: dli_syntax::structure_dialect_body(&v1.body),
        provenance: v1.provenance,
    });
    let deepened = serde_json::to_string(&v2).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    *first = Value::String(deepened);
    Ok(())
}

/// The v1 shape of a [`NameDecl`]: the body is the old `Vec<Clause>` string body,
/// before LS-9 structured it into a `NameBody`.
#[cfg(feature = "syntax")]
#[derive(serde::Deserialize)]
struct NameDeclV1 {
    name: Ident,
    label: Option<String>,
    #[serde(default)]
    head: Vec<String>,
    #[serde(default)]
    body: Vec<Clause>,
    provenance: Option<Annotation>,
}

/// If `value` is a v1 `name` carriage, deepen its embedded declaration's string
/// body to the typed `NameBody` via the front-end lowering.
#[cfg(feature = "syntax")]
fn deepen_name_carriage(value: &mut Value) -> Result<(), UpcastError> {
    let Some(first) = carriage_decl_json(value, "name") else {
        return Ok(());
    };
    let Some(decl_json) = first.as_str() else {
        return Ok(());
    };
    let decl_value: Value =
        serde_json::from_str(decl_json).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    let Some(name_v) = decl_value.get("Name").cloned() else {
        return Ok(());
    };
    let v1: NameDeclV1 =
        serde_json::from_value(name_v).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    let v2 = Decl::Name(NameDecl {
        name: v1.name,
        label: v1.label,
        head: v1.head,
        body: dli_syntax::structure_name_body(&v1.body),
        provenance: v1.provenance,
    });
    let deepened = serde_json::to_string(&v2).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    *first = Value::String(deepened);
    Ok(())
}

/// The v1 shape of a [`RatifyDecl`]: the body is the old `Vec<Clause>` string
/// body, before LS-9 structured it into a `RatifyBody`.
#[cfg(feature = "syntax")]
#[derive(serde::Deserialize)]
struct RatifyDeclV1 {
    decision: Ident,
    #[serde(default)]
    body: Vec<Clause>,
}

/// If `value` is a v1 `ratify` carriage, deepen its embedded declaration's string
/// body to the typed `RatifyBody` via the front-end lowering.
#[cfg(feature = "syntax")]
fn deepen_ratify_carriage(value: &mut Value) -> Result<(), UpcastError> {
    let Some(first) = carriage_decl_json(value, "ratify") else {
        return Ok(());
    };
    let Some(decl_json) = first.as_str() else {
        return Ok(());
    };
    let decl_value: Value =
        serde_json::from_str(decl_json).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    let Some(ratify_v) = decl_value.get("Ratify").cloned() else {
        return Ok(());
    };
    let v1: RatifyDeclV1 =
        serde_json::from_value(ratify_v).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    let v2 = Decl::Ratify(RatifyDecl {
        decision: v1.decision,
        body: dli_syntax::structure_ratify_body(&v1.body),
    });
    let deepened = serde_json::to_string(&v2).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    *first = Value::String(deepened);
    Ok(())
}

/// Return a mutable handle to the first embedded-declaration JSON string of a
/// fact object's expert carriage, iff the carriage is of `kind`. Shared by the
/// LS-9 deepen functions so each is just its own V1 → V2 mapping.
#[cfg(feature = "syntax")]
fn carriage_decl_json<'a>(value: &'a mut Value, kind: &str) -> Option<&'a mut Value> {
    let expert = value.get_mut("expert_body")?;
    if expert.get("kind").and_then(Value::as_str) != Some(kind) {
        return None;
    }
    expert
        .get_mut("body")
        .and_then(Value::as_array_mut)
        .and_then(|arr| arr.first_mut())
}

/// If `value` is a fact object carrying a v1 `proof` expert body, rewrite the
/// embedded declaration JSON in place so its body is the typed `ProofBody`. A
/// value that is not a proof carriage is left untouched.
#[cfg(feature = "syntax")]
fn deepen_proof_carriage(value: &mut Value) -> Result<(), UpcastError> {
    let Some(expert) = value.get_mut("expert_body") else {
        return Ok(());
    };
    if expert.get("kind").and_then(Value::as_str) != Some("proof") {
        return Ok(());
    }
    let Some(first) = expert
        .get_mut("body")
        .and_then(Value::as_array_mut)
        .and_then(|arr| arr.first_mut())
    else {
        return Ok(());
    };
    let Some(decl_json) = first.as_str() else {
        return Ok(());
    };

    let decl_value: Value =
        serde_json::from_str(decl_json).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    // The embedded declaration is an externally-tagged `Decl`; only the `Proof`
    // variant needs deepening.
    let Some(proof_v) = decl_value.get("Proof").cloned() else {
        return Ok(());
    };
    let v1: ProofDeclV1 =
        serde_json::from_value(proof_v).map_err(|e| UpcastError::Carriage(e.to_string()))?;

    let body = dli_syntax::structure_proof_body(&v1.body);
    let v2 = Decl::Proof(ProofDecl {
        name: v1.name,
        head: v1.head,
        body,
        provenance: v1.provenance,
    });
    let deepened = serde_json::to_string(&v2).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    *first = Value::String(deepened);
    Ok(())
}

/// If `value` is a fact object carrying a v1 `practice` expert body, rewrite the
/// embedded declaration JSON in place so its body is the typed `PracticeBody`. A
/// value that is not a practice carriage is left untouched.
#[cfg(feature = "syntax")]
fn deepen_practice_carriage(value: &mut Value) -> Result<(), UpcastError> {
    let Some(expert) = value.get_mut("expert_body") else {
        return Ok(());
    };
    if expert.get("kind").and_then(Value::as_str) != Some("practice") {
        return Ok(());
    }
    let Some(first) = expert
        .get_mut("body")
        .and_then(Value::as_array_mut)
        .and_then(|arr| arr.first_mut())
    else {
        return Ok(());
    };
    let Some(decl_json) = first.as_str() else {
        return Ok(());
    };

    let decl_value: Value =
        serde_json::from_str(decl_json).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    // The embedded declaration is an externally-tagged `Decl`; only the `Practice`
    // variant needs deepening.
    let Some(practice_v) = decl_value.get("Practice").cloned() else {
        return Ok(());
    };
    let v1: PracticeDeclV1 =
        serde_json::from_value(practice_v).map_err(|e| UpcastError::Carriage(e.to_string()))?;

    let body = dli_syntax::structure_practice_body(&v1.body, &v1.name, v1.provenance.as_ref());
    let v2 = Decl::Practice(PracticeDecl {
        name: v1.name,
        label: v1.label,
        body,
        provenance: v1.provenance,
    });
    let deepened = serde_json::to_string(&v2).map_err(|e| UpcastError::Carriage(e.to_string()))?;
    *first = Value::String(deepened);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SCHEMA_VERSION, UpcastError, upcast_value};

    /// A v1 rule carriage, its embedded declaration in the v1 string-bodied
    /// shape. The body is empty, which is enough: what is under test is which
    /// build can *reach* the deepening, not what the deepening produces.
    fn v1_rule_carriage() -> serde_json::Value {
        let decl = serde_json::json!({
            "Rule": {
                "name": { "text": "r", "span": { "file": 0, "start": 0, "end": 0 } },
                "domain": null,
                "head": [],
                "body": [],
                "provenance": null,
            }
        });
        serde_json::json!({
            "schema_version": 1,
            "expert_body": { "kind": "rule", "body": [decl.to_string()] },
        })
    }

    #[test]
    fn a_v1_object_that_needs_no_front_end_lifts_on_every_build() {
        // A gentle fact's v2 form is byte-identical to its v1 form, so the lift
        // is only the version stamp — no parser involved, either way.
        let lifted = upcast_value(
            1,
            serde_json::json!({ "schema_version": 1, "atom": "p(a, b)" }),
        )
        .expect("a gentle fact lifts without a front end");
        assert_eq!(lifted["schema_version"], serde_json::json!(SCHEMA_VERSION));
        assert_eq!(lifted["atom"], serde_json::json!("p(a, b)"));
    }

    #[test]
    fn a_v1_expert_carriage_lifts_only_where_there_is_a_front_end() {
        let lifted = upcast_value(1, v1_rule_carriage());
        if cfg!(feature = "syntax") {
            let lifted = lifted.expect("a syntax build deepens the v1 interior");
            assert_eq!(lifted["schema_version"], serde_json::json!(SCHEMA_VERSION));
        } else {
            // Refused by name, not mis-lifted and not silently stamped v2.
            match lifted {
                Err(UpcastError::SyntaxRequired { kind }) => assert_eq!(kind, "rule"),
                other => panic!("a parser-free build must refuse the v1 carriage, got {other:?}"),
            }
        }
    }

    #[test]
    fn an_already_current_object_is_returned_untouched() {
        let current = serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "expert_body": { "kind": "rule", "body": ["{}"] },
        });
        assert_eq!(
            upcast_value(u64::from(SCHEMA_VERSION), current.clone()).expect("v2 needs no lift"),
            current
        );
    }

    #[test]
    fn a_v3_decided_marker_upcasts_to_provisional() {
        let old = serde_json::json!({
            "schema_version": 3,
            "provenance": { "marker": "Decided" }
        });
        let lifted = upcast_value(3, old).expect("v3 marker lifts");
        assert_eq!(lifted["schema_version"], serde_json::json!(SCHEMA_VERSION));
        assert_eq!(lifted["provenance"]["marker"], "Provisional");
    }

    #[test]
    fn a_v3_typed_carriage_is_not_reparsed_as_v1() {
        let declaration = serde_json::json!({
            "Dialect": {
                "name": {"span": {"start": 0, "end": 0, "file": 0}, "text": "imperial"},
                "label": "The Imperial Tongue",
                "body": {"clauses": []},
                "provenance": null
            }
        });
        let old = serde_json::json!({
            "schema_version": 3,
            "expert_body": {
                "kind": "dialect",
                "name": "imperial",
                "body": [serde_json::to_string(&declaration).expect("declaration JSON")]
            }
        });

        let lifted = upcast_value(3, old).expect("v3 typed carriage lifts");
        let body = lifted["expert_body"]["body"][0]
            .as_str()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .expect("typed declaration remains JSON");
        assert!(body["Dialect"]["body"].is_object());
        assert_eq!(lifted["schema_version"], serde_json::json!(SCHEMA_VERSION));
    }
}
