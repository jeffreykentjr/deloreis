//! Schema emission and the `--check` drift guard.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use schemars::schema_for;

/// Emit (or, with `check`, verify) the AST and contract schemas.
pub fn run_schema(root: &Path, check: bool) -> Result<()> {
    let mut targets: Vec<(PathBuf, String)> = Vec::new();

    let ast = dli_ast::schema::ast_schema();
    targets.push((
        root.join("schemas/ast/document.schema.json"),
        serde_json::to_string_pretty(&ast)? + "\n",
    ));

    for (name, schema) in [
        (
            "doctrine-pack-v1",
            schema_for!(dli_world::craft::DoctrinePack),
        ),
        (
            "doctrine-principle-v1",
            schema_for!(dli_world::craft::DoctrinePrinciple),
        ),
        (
            "principle-overlay-v1",
            schema_for!(dli_world::craft::worldcfg::PrincipleOverlay),
        ),
        (
            "world-principle-v1",
            schema_for!(dli_world::craft::worldcfg::WorldPrinciple),
        ),
        (
            "craft-config-v1",
            schema_for!(dli_world::craft::worldcfg::CraftConfig),
        ),
    ] {
        targets.push((
            root.join(format!("schemas/craft/{name}.schema.json")),
            serde_json::to_string_pretty(&schema)? + "\n",
        ));
    }

    let config_schemas = dli_world::craft::worldcfg::config_file_schemas();
    validate_craft_config_files(root, &config_schemas)?;
    for file_schema in config_schemas {
        targets.push((
            root.join(format!(
                "schemas/craft/{}.schema.json",
                file_schema.schema_name
            )),
            serde_json::to_string_pretty(&file_schema.schema)? + "\n",
        ));
    }

    for (name, schema) in dli_serve::schema::contract_schemas() {
        targets.push((
            root.join(format!("contracts/dli-worker/schemas/{name}.json")),
            serde_json::to_string_pretty(&schema)? + "\n",
        ));
    }

    // Lorestore fact-object schemas (DL-0041).
    for (name, schema) in dli_lorestore::schemas() {
        targets.push((
            root.join(format!("schemas/lorestore/{name}.json")),
            serde_json::to_string_pretty(&schema)? + "\n",
        ));
    }

    if check {
        let mut problems: Vec<String> = Vec::new();
        for (path, content) in &targets {
            match std::fs::read_to_string(path) {
                Ok(existing) if existing == *content => {}
                Ok(_) => problems.push(format!("drift:   {}", path.display())),
                Err(_) => problems.push(format!("missing: {}", path.display())),
            }
        }
        if !problems.is_empty() {
            bail!(
                "schema --check found {} problem(s); run `cargo run -p xtask -- schema`:\n{}",
                problems.len(),
                problems.join("\n")
            );
        }
        println!(
            "schema: {} schemas match the committed copies",
            targets.len()
        );
    } else {
        for (path, content) in &targets {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, content)?;
        }
        println!("schema: wrote {} schemas", targets.len());
    }
    Ok(())
}

fn validate_craft_config_files(
    root: &Path,
    schemas: &[dli_world::craft::worldcfg::ConfigFileSchema],
) -> Result<()> {
    for relative_dir in ["crates/dli-world/worlds/example/craft", "worlds/tpth/craft"] {
        let dir = root.join(relative_dir);
        for entry in std::fs::read_dir(&dir)
            .with_context(|| format!("reading craft config directory {}", dir.display()))?
        {
            let entry = entry.with_context(|| format!("reading entry in {}", dir.display()))?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .with_context(|| format!("non-UTF-8 craft config filename in {}", dir.display()))?;
            let file_schema = schemas
                .iter()
                .find(|schema| schema.config_file == name)
                .with_context(|| format!("no published schema for {}", entry.path().display()))?;
            let path = entry.path();
            if !path.is_file() {
                bail!("craft config entry is not a file: {}", path.display());
            }
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let value: toml::Value =
                toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
            let instance = serde_json::to_value(value)?;
            let schema = serde_json::to_value(&file_schema.schema)?;
            crate::jsonval::validate_json(&schema, &instance)
                .with_context(|| format!("validating {}", path.display()))?;
        }
    }
    Ok(())
}
