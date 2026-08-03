#![allow(clippy::print_stdout, clippy::print_stderr)]
//! The `xtask` entry point.

use std::path::PathBuf;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::env::current_dir()?;

    let Some((command, rest)) = args.split_first() else {
        eprintln!(
            "usage: xtask <gate|authority|schema|fixtures|docgen|docs-oracle|docs-tokens|book|boundary|syntax-free|hooks-check>"
        );
        std::process::exit(2);
    };

    match command.as_str() {
        "gate" => {
            let ci = has_flag(rest, "--ci");
            if has_flag(rest, "--list") {
                xtask::gate::list_gate(ci, has_flag(rest, "--json"))
            } else {
                let receipt = flag_value(rest, "--receipt").map(PathBuf::from);
                xtask::gate::run_gate(&root, ci, receipt.as_deref())
            }
        }
        "schema" => xtask::schema::run_schema(&root, has_flag(rest, "--check")),
        "authority" => xtask::authority::run_authority(&root),
        "docgen" => xtask::docgen::run_docgen(&root, has_flag(rest, "--check")),
        "book" => xtask::book::run_book(&root),
        "fixtures" => {
            let contract = root.join("contracts/dli-worker");
            let schemas = contract.join("schemas");
            xtask::fixtures::run_fixtures(&contract, &schemas, has_flag(rest, "--update-digests"))
        }
        "docs-oracle" => xtask::docs_oracle::run_docs_oracle(&root),
        "docs-tokens" => xtask::docs_tokens::run_docs_tokens(&root),
        "boundary" => xtask::boundary::run_boundary(&root),
        "syntax-free" => xtask::syntax_free::run_syntax_free(&root),
        "hooks-check" => xtask::hooks::run_hooks_check(),
        other => bail!("unknown xtask command: {other}"),
    }
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let prefix = format!("{flag}=");
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().cloned();
        }
        if let Some(value) = arg.strip_prefix(&prefix) {
            return Some(value.to_owned());
        }
    }
    None
}
