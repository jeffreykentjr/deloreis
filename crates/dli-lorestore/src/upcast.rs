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
//! No stored fact is rewritten: the deepening happens in memory on the value
//! read from disk. The old bytes keep their v1 content address; the lifted
//! in-memory object is stamped with the current [`SCHEMA_VERSION`].

use serde_json::Value;

use dli_ast::annot::Annotation;
use dli_ast::clause::Clause;
use dli_ast::decl::{Decl, ProofDecl, RuleDecl, RuleDomain};
use dli_ast::ident::Ident;

use crate::SCHEMA_VERSION;

/// A stored object shape that this build cannot lift to the current schema.
#[derive(Debug, thiserror::Error)]
pub enum UpcastError {
    /// A v1 carriage's embedded declaration JSON did not decode for deepening.
    #[error("v1 carriage could not be deepened to the current schema: {0}")]
    Carriage(String),
}

/// The v1 shape of a [`RuleDecl`]: the body is the old `Vec<Clause>` of
/// whitespace-normalized text, before LS-9 structured it into a `RuleBody`.
/// Field order and names mirror the v1 serialization exactly so serde reads an
/// old rule carriage verbatim.
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

/// Lift a stored object's JSON `value` from `from_version` to the current
/// [`SCHEMA_VERSION`].
///
/// v1 → v2 deepens an expert-tier **rule** carriage's embedded declaration from
/// a `Vec<Clause>` string body to the typed `RuleBody`, and a **proof** carriage
/// from its `Vec<Clause>` body to the typed `ProofBody`, via the front-end
/// lowering. Every other v1 object (gentle facts, other carriages, commits,
/// refs, edges, evidence) is byte-identical to its v2 form, so the lift is just
/// the version stamp. A `from_version` already at (or above) the current schema
/// is returned unchanged.
pub fn upcast_value(from_version: u64, mut value: Value) -> Result<Value, UpcastError> {
    if from_version >= u64::from(SCHEMA_VERSION) {
        return Ok(value);
    }

    // v1 → v2: deepen every expert carriage whose interior LS-9 structured. Each
    // helper no-ops on a carriage of another kind, so the order is irrelevant.
    deepen_rule_carriage(&mut value)?;
    deepen_proof_carriage(&mut value)?;

    // Stamp the lifted object with the schema it now conforms to.
    if let Some(obj) = value.as_object_mut() {
        obj.insert("schema_version".to_owned(), Value::from(SCHEMA_VERSION));
    }
    Ok(value)
}

/// If `value` is a fact object carrying a v1 `rule` expert body, rewrite the
/// embedded declaration JSON in place so its body is the typed `RuleBody`. A
/// value that is not a rule carriage is left untouched.
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

/// If `value` is a fact object carrying a v1 `proof` expert body, rewrite the
/// embedded declaration JSON in place so its body is the typed `ProofBody`. A
/// value that is not a proof carriage is left untouched.
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
