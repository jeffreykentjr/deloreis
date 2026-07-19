//! JSON Schema validation helper shared by the gate and the fixture check.

use anyhow::{Result, anyhow, bail};

/// Validate `instance` against `schema`, collecting every error into one report.
pub fn validate_json(schema: &serde_json::Value, instance: &serde_json::Value) -> Result<()> {
    let validator =
        jsonschema::validator_for(schema).map_err(|error| anyhow!("invalid schema: {error}"))?;
    if !validator.is_valid(instance) {
        let messages: Vec<String> = validator
            .iter_errors(instance)
            .map(|error| error.to_string())
            .collect();
        bail!("does not conform to schema:\n{}", messages.join("\n"));
    }
    Ok(())
}
