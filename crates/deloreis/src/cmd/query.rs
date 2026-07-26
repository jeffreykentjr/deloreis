//! `deloreis query <goal>` — answer a goal over the compiled world.
//!
//! The query rides `World::query_with`: a standing-layer floor (`--layer`), an
//! as-of time filter (`--as-of`), a stored fork's valuation overlay (`--fork`),
//! and optional support rendering (`--why`) all layer onto one evaluation pass.
//! The three verdicts are disjoint (`10-serving.md` §2.1): a matched goal exits
//! `0`, an evaluated-but-empty goal exits `1` (asked and answered, nothing
//! matched — not a failure), and an evaluation error exits `2`.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use dli_eval::query::{Layer, Query, QueryOpts};
use dli_eval::world::World;
use dli_world::language::ratified_names;

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// The standing floor a query answer is filtered to, as a CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum LayerArg {
    /// Only assertable (certain) rows.
    Certain,
    /// The shadow layer and the certain layer together.
    Possible,
    /// No standing floor.
    All,
}

impl LayerArg {
    /// The engine [`Layer`] this maps to.
    fn layer(self) -> Layer {
        match self {
            LayerArg::Certain => Layer::Certain,
            LayerArg::Possible => Layer::Possible,
            LayerArg::All => Layer::All,
        }
    }

    /// The lowercase word for a message.
    fn word(self) -> &'static str {
        match self {
            LayerArg::Certain => "certain",
            LayerArg::Possible => "possible",
            LayerArg::All => "all",
        }
    }
}

/// Arguments for `deloreis query`.
#[derive(Debug, Args)]
pub(crate) struct QueryArgs {
    /// The goal atom: `pred(a, b)`, a `fact s r o` triple, or `subject.slot = value`.
    #[arg(value_name = "GOAL")]
    pub(crate) goal: String,
    /// The `.dli` corpus to query (files or directories). Required unless
    /// `--fork` names a fork whose store already records its corpus.
    #[arg(short = 'c', long = "corpus", value_name = "PATH")]
    pub(crate) corpus: Vec<PathBuf>,
    /// The standing floor for the returned rows.
    #[arg(long, value_enum, default_value_t = LayerArg::All)]
    pub(crate) layer: LayerArg,
    /// Drop rows anchored strictly after this era or event.
    #[arg(long = "as-of", value_name = "POINT")]
    pub(crate) as_of: Option<String>,
    /// Evaluate under a stored fork's valuation overlay (read from `--workspace`).
    #[arg(long, value_name = "FORK")]
    pub(crate) fork: Option<String>,
    /// The workspace directory holding the fork store (used with `--fork`).
    #[arg(short = 'w', long, value_name = "DIR", default_value = ".")]
    pub(crate) workspace: PathBuf,
    /// Attach the support antichain (`--why`) to every row.
    #[arg(long)]
    pub(crate) why: bool,
    /// Emit a machine-readable generated view. Currently defined for the
    /// certain-layer `name` view only.
    #[arg(long)]
    pub(crate) json: bool,
}

/// Answer the query and render its rows.
pub(crate) fn run(args: &QueryArgs) -> CmdResult {
    if args.goal == "name" {
        return run_name_view(args);
    }
    if args.json {
        return Err("--json is defined only for `query name --layer certain --json`".to_owned());
    }

    let goal = cmd::parse_goal(&args.goal)?;
    let query = Query { goal };

    let (world, fork_valuation): (World, Option<_>) = if let Some(fork_ref) = &args.fork {
        let (world, valuation) = cmd::fork::resolve(&args.workspace, fork_ref)?;
        (world, Some(valuation))
    } else {
        if args.corpus.is_empty() {
            return Err(
                "no corpus — pass --corpus <path>, or --fork with a workspace store".to_owned(),
            );
        }
        let files = cmd::load_corpus(&args.corpus)?;
        let compiled = cmd::compile_corpus(&files, "query")?;
        (compiled.model.world, None)
    };

    // A dangling `--as-of` would silently filter nothing; reject it like the
    // sibling unknown-predicate / unknown-dialect paths rather than lie.
    if let Some(as_of) = &args.as_of
        && !world.time_points.names().iter().any(|name| name == as_of)
    {
        return Err(format!(
            "unknown era or event '{as_of}' — --as-of must name a point on the chronology"
        ));
    }

    let opts = QueryOpts {
        layer: args.layer.layer(),
        as_of: args.as_of.clone(),
        fork: fork_valuation,
        why: args.why,
    };

    let answer = world
        .query_with(&query, &opts)
        .map_err(|err| format!("the query could not run: {err}"))?;

    if answer.rows.is_empty() {
        outln!(
            "no rows match {} in the {} layer",
            args.goal,
            args.layer.word()
        );
        return Ok(EXIT_FINDINGS);
    }

    let mut rows = answer.rows;
    rows.sort_by(|a, b| a.atom.cmp(&b.atom));
    for row in &rows {
        outln!(
            "{status}  {atom}  ({grade})",
            status = cmd::status_word(row.status),
            atom = row.atom,
            grade = row.verdict.grade,
        );
        if let Some(why) = &row.why {
            for witness in &why.rendered {
                outln!("    why: {witness}");
            }
        }
    }
    outln!("{} row(s)", rows.len());
    Ok(EXIT_CLEAN)
}

fn run_name_view(args: &QueryArgs) -> CmdResult {
    if !args.json {
        return Err("the generated name view requires --json".to_owned());
    }
    if args.layer != LayerArg::Certain {
        return Err("the generated name view is defined only for --layer certain".to_owned());
    }
    if args.as_of.is_some() || args.fork.is_some() || args.why {
        return Err(
            "`query name --json` does not define --as-of, --fork, or --why semantics".to_owned(),
        );
    }
    if args.corpus.is_empty() {
        return Err("the generated name view requires --corpus <path>".to_owned());
    }

    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "query-name")?;
    let mut names = ratified_names(&world.model.language);
    names.sort_by(|a, b| {
        (&a.name, &a.kind, &a.register, &a.decision, &a.bearer).cmp(&(
            &b.name,
            &b.kind,
            &b.register,
            &b.decision,
            &b.bearer,
        ))
    });
    let rendered = serde_json::to_string_pretty(&names)
        .map_err(|err| format!("cannot render the generated name view: {err}"))?;
    outln!("{rendered}");
    Ok(EXIT_CLEAN)
}
