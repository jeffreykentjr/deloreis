//! Schema emission and the `--check` drift guard.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

/// Emit (or, with `check`, verify) the AST and contract schemas.
pub fn run_schema(root: &Path, check: bool) -> Result<()> {
    let mut targets: Vec<(PathBuf, String)> = Vec::new();

    let ast = dli_ast::schema::ast_schema();
    targets.push((
        root.join("schemas/ast/document.schema.json"),
        serde_json::to_string_pretty(&ast)? + "\n",
    ));

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
