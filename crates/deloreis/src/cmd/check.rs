//! `deloreis check <paths…>` — parse, compile, and run the world-check battery.
//!
//! The battery is three seams over one corpus: the syntax diagnostics from
//! parsing (rendered through the three-rung ladder), the loader's fail-closed
//! findings plus the continuity classes (`dli_world::check::run`), and the
//! language layer's world-true naming findings (`dli_world::language::check`).
//! A clean corpus exits `0`; any finding exits `1` with the finding lines and,
//! for a syntax fault, the ladder block that teaches the repair.

use std::fmt::Write as _;
use std::path::PathBuf;

use dli_ast::decl::Decl;

use clap::Args;
use dli_syntax::ladder;
use dli_world::{check, language};

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// Arguments for `deloreis check`.
#[derive(Debug, Args)]
pub(crate) struct CheckArgs {
    /// The `.dli` documents (or directories to walk) to validate.
    #[arg(value_name = "PATH", required = true)]
    pub(crate) paths: Vec<PathBuf>,
}

/// Validate the given corpus and report every finding.
pub(crate) fn run(args: &CheckArgs) -> CmdResult {
    let files = cmd::load_corpus(&args.paths)?;

    let mut findings = 0usize;

    // 1. Syntax diagnostics, rendered with their graded ladders under a caret
    //    block, per file.
    for file in &files {
        if !file.diagnostics.is_empty() {
            findings += file.diagnostics.len();
            out!(
                "{}:\n{}",
                file.path.display(),
                ladder::render(&file.source, &file.diagnostics)
            );
        }
    }

    // 2. Semantic findings: load-time, continuity, and language, collected into
    //    one list sorted for a deterministic, diffable report.
    let mut lines: Vec<String> = Vec::new();
    match cmd::compile_corpus(&files, "check") {
        Ok(world) => {
            for finding in check::run(&world) {
                if redundant_provenance_finding(&finding, &files) {
                    continue;
                }
                lines.push(render_semantic_finding(&finding, &files));
            }
            for finding in language::check(&world.model.language) {
                lines.push(render_semantic_finding(&finding, &files));
            }
        }
        Err(message) => lines.push(message),
    }
    lines.sort();
    findings += lines.len();
    for line in &lines {
        outln!("{line}");
    }

    let count = files.len();
    if findings == 0 {
        outln!("checked {count} document(s): clean");
        Ok(EXIT_CLEAN)
    } else {
        outln!("checked {count} document(s): {findings} finding(s)");
        Ok(EXIT_FINDINGS)
    }
}

/// A ruling without a decision produces both an origin-required syntax error
/// and a malformed-provenance loader finding. They describe the same repair.
fn redundant_provenance_finding(finding: &check::WorldFinding, files: &[cmd::CorpusFile]) -> bool {
    if finding.code != "load.malformed-provenance" {
        return false;
    }
    let Some(span) = finding.span else {
        return false;
    };
    let Some(file) = files.get(span.file.0 as usize) else {
        return false;
    };
    let source = file.source.as_bytes();
    file.diagnostics.iter().any(|diagnostic| {
        let first = (diagnostic.span.start as usize).min(source.len());
        let second = (span.start as usize).min(source.len());
        diagnostic.class == dli_syntax::diag::DiagClass::OriginRequired
            && diagnostic.span.file == span.file
            && !source[first.min(second)..first.max(second)].contains(&b'\n')
    })
}

/// Render a corpus compile failure consistently for every CLI command.
pub(super) fn render_compile_failure(message: String, files: &[cmd::CorpusFile]) -> String {
    let finding = check::WorldFinding::error("compile.unstratifiable", message);
    let source = negative_cycle_source(files);
    let finding = if let Some(span) = source {
        finding.with_span(span)
    } else {
        finding
    };
    let mut line = render_semantic_finding(&finding, files);
    if source.is_some() {
        line.push_str("\n  help: remove or revise a negative dependency in the reported cycle");
    }
    line
}

/// Locate an authored rule containing the exact negative edge selected by the
/// evaluator. Typed rule bodies are compared to loaded rules so an invalid or
/// synthetic rule cannot claim a source location.
fn negative_cycle_source(files: &[cmd::CorpusFile]) -> Option<dli_ast::span::Span> {
    let loaded = dli_world::loader::Loader::load(&cmd::documents(files));
    let rules = &loaded.world.world.rules;
    let (head, dep) = dli_eval::strata::negative_cycle_witness(rules)?;
    for file in files {
        for decl in &file.document.decls {
            let Decl::Rule(rule) = decl else { continue };
            if rule.domain.is_some() {
                continue;
            }
            let clauses = rule
                .body
                .clauses
                .iter()
                .map(|clause| (clause, clause.clause.span))
                .chain(
                    rule.body
                        .inline
                        .iter()
                        .map(|clause| (clause, rule.name.span)),
                );
            for (clause, span) in clauses {
                let Some(datalog) = &clause.datalog else {
                    continue;
                };
                if span.end <= span.start
                    || datalog.head.pred.text != head
                    || !datalog
                        .body
                        .iter()
                        .any(|lit| lit.negated && lit.atom.pred.text == dep)
                {
                    continue;
                }
                if rules.iter().any(|loaded| {
                    loaded.name == rule.name.text
                        && loaded.head == datalog.head
                        && loaded.body == datalog.body
                }) {
                    return Some(span);
                }
            }
        }
    }
    None
}

pub(super) fn render_semantic_finding(
    finding: &check::WorldFinding,
    files: &[cmd::CorpusFile],
) -> String {
    let mut line = cmd::render_world_finding(finding);
    if let Some(span) = finding.span
        && span.end > span.start
        && let Some(file) = files.get(span.file.0 as usize)
    {
        let offset = span.start as usize;
        if let Some(prefix) = file.source.get(..offset) {
            let prefix = prefix.strip_prefix('\u{feff}').unwrap_or(prefix);
            let row = prefix.bytes().filter(|&byte| byte == b'\n').count() + 1;
            let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
            let _ = write!(line, " ({}:{row}:{column})", file.path.display());
        }
    }
    let help = match finding.code.as_str() {
        "load.missing-provenance" => Some("add provenance to the declaration"),
        "load.malformed-provenance" => Some("correct the declaration's provenance"),
        "load.malformed-world-rule" => Some("write a valid rule head and body"),
        "load.unbound-head-variable" => Some("bind each head variable in a positive body atom"),
        "load.unknown-temporal-reference" => {
            Some("declare the referenced era or event, or correct its name")
        }
        "load.duplicate-declaration" => Some("rename or remove one of the duplicate declarations"),
        "load.cross-pack-collision" => {
            Some("rename or remove a declaration shared by multiple scopes")
        }
        "load.inert-declaration" => Some("give the state a subject or a value"),
        "predicate-identity.arity-mismatch" => {
            Some("use matching argument counts for the identified predicates")
        }
        "name.slot-sequence" => Some("use a slot sequence declared for the name kind"),
        "name.role-membership" => {
            Some("declare the name part in that role or choose a declared part")
        }
        "name.register-fit" => Some("declare the dialect and name kind, or correct their names"),
        "name.phonotactics" => Some("change the name to match the dialect's sound rules"),
        "name.uniqueness" => Some("choose a distinct name or remove the duplicate declaration"),
        "name.fixed-token-reuse" => Some("choose a name that does not reuse the reserved token"),
        "name.gender-morphology" => Some("use name parts without conflicting gender markers"),
        "name.banned-in-name" => Some("choose a name without the banned word or revise the rule"),
        "citation.unknown-decision" => {
            Some("declare the cited decision with ratify or correct the citation")
        }
        "continuity.chronology-cycle" => Some("correct an ordering in the reported cycle"),
        "continuity.identity-temporal-unsupported" => {
            Some("refer to one era or event instead of merging time points")
        }
        "continuity.rejected-lore" => Some("remove or replace the rejected term"),
        "continuity.fluent-contradiction" => {
            Some("adjust the time bounds or revise a conflicting state")
        }
        "continuity.fluent-order-undeclared" => {
            Some("declare an ordering between the named events or revise the states")
        }
        "continuity.inertia-unanchored" => {
            Some("give the state a since event, or leave the value unanchored deliberately")
        }
        "continuity.pending-conflict" => {
            Some("review the proposed values and decide which account should hold")
        }
        "continuity.fd-violation" => Some(
            "revise or defeat a conflicting value for the functional slot, or review its functional declarations",
        ),
        "continuity.derived-fd-violation" => Some(
            "revise a conflicting source claim or derivation rule, or review the functional declarations",
        ),
        _ => None,
    };
    if let Some(help) = help {
        let _ = write!(line, "\n  help: {help}");
    }
    line
}
