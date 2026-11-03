//! `deloreis check <paths…>` — parse, compile, and run the world-check battery.
//!
//! The battery is three seams over one corpus: the syntax diagnostics from
//! parsing (rendered through the three-rung ladder), the loader's fail-closed
//! findings plus the continuity classes (`dli_world::check::run`), and the
//! language layer's world-true naming findings (`dli_world::language::check`).
//! A clean corpus exits `0`; any finding exits `1` with the finding lines and,
//! for a syntax fault, the ladder block that teaches the repair.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use dli_ast::decl::Decl;
use dli_ast::ident::Ident;
use dli_ast::term::Term;

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
                lines.push(render_semantic_finding(&finding, &files));
            }
            for finding in language::check(&world.model.language) {
                lines.push(render_semantic_finding(&finding, &files));
            }
        }
        Err(message) => {
            let finding = check::WorldFinding::error("compile.unstratifiable", message);
            let source = negative_cycle_source(&files);
            let finding = if let Some(span) = source {
                finding.with_span(span)
            } else {
                finding
            };
            let mut line = render_semantic_finding(&finding, &files);
            if source.is_some() {
                line.push_str(
                    "\n  help: remove or revise a negative dependency in the reported cycle",
                );
            }
            lines.push(line);
        }
    }
    lines.extend(likely_entity_typos(&files));
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

fn render_semantic_finding(finding: &check::WorldFinding, files: &[cmd::CorpusFile]) -> String {
    let mut line = cmd::render_world_finding(finding);
    if let Some(span) = finding.span
        && span.end > span.start
        && let Some(file) = files.get(span.file.0 as usize)
    {
        let offset = span.start as usize + usize::from(file.source.starts_with('\u{feff}')) * 3;
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
        "continuity.fd-violation" => {
            Some("revise or defeat a conflicting value for the functional slot")
        }
        "continuity.derived-fd-violation" => {
            Some("revise a conflicting source claim or derivation rule")
        }
        _ => None,
    };
    if let Some(help) = help {
        let _ = write!(line, "\n  help: {help}");
    }
    line
}

/// A near miss is useful evidence of a typo; unrelated undeclared symbols may
/// intentionally name abstract concepts, so they are not diagnosed here.
fn likely_entity_typos(files: &[cmd::CorpusFile]) -> Vec<String> {
    let mut declared: BTreeMap<Option<&str>, BTreeSet<&str>> = BTreeMap::new();
    for file in files {
        let scope = file
            .document
            .scope
            .as_ref()
            .map(|scope| scope.name.text.as_str());
        for decl in &file.document.decls {
            if let Decl::Entity(entity) = decl {
                declared.entry(scope).or_default().insert(&entity.name.text);
            }
        }
    }
    let entity_positions = known_entity_positions(files, &declared);
    let mut warnings = Vec::new();
    for file in files {
        let scope = file
            .document
            .scope
            .as_ref()
            .map(|scope| scope.name.text.as_str());
        let Some(names) = declared.get(&scope) else {
            continue;
        };
        for decl in &file.document.decls {
            let symbols: Vec<&Ident> = match decl {
                Decl::Attr(attr) => attr_symbols(
                    attr,
                    entity_positions.contains(&(scope, attr.attr.text.as_str(), 1)),
                ),
                Decl::Entity(entity) => entity
                    .slots
                    .iter()
                    .flat_map(|attr| {
                        attr_symbols(
                            attr,
                            entity_positions.contains(&(scope, attr.attr.text.as_str(), 1)),
                        )
                    })
                    .collect(),
                Decl::Fact(fact) => fact
                    .atom
                    .args
                    .iter()
                    .enumerate()
                    .filter_map(|(position, term)| {
                        if position != 0
                            && !entity_positions.contains(&(
                                scope,
                                fact.atom.pred.text.as_str(),
                                position,
                            ))
                        {
                            return None;
                        }
                        match term {
                            Term::Sym(id) => Some(id),
                            _ => None,
                        }
                    })
                    .collect(),
                _ => continue,
            };
            for symbol in symbols {
                let symbol_len = symbol.text.chars().count();
                if names.contains(symbol.text.as_str()) {
                    continue;
                }
                let candidate = names
                    .iter()
                    .filter(|name| {
                        let len = name.chars().count();
                        len.abs_diff(symbol_len) <= 2
                    })
                    .map(|name| (edit_distance(&symbol.text, name), *name))
                    .filter(|(distance, _)| *distance > 0 && *distance <= 2)
                    .min();
                if let Some((_, candidate)) = candidate {
                    let prefix = file.source.get(..symbol.span.start as usize).unwrap_or("");
                    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
                    let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
                    warnings.push(format!("warn  entity.undeclared — {}:{line}:{column}: `{}` is not declared; did you mean `{candidate}`?", file.path.display(), symbol.text));
                }
            }
        }
    }
    warnings
}

type EntityPositions<'a> = BTreeSet<(Option<&'a str>, &'a str, usize)>;

fn known_entity_positions<'a>(
    files: &'a [cmd::CorpusFile],
    declared: &BTreeMap<Option<&'a str>, BTreeSet<&'a str>>,
) -> EntityPositions<'a> {
    // Values may be concepts rather than entities. Treat a non-subject
    // position as an entity reference only when another fact in this scope
    // already puts a declared entity in that same position.
    let mut entity_positions = BTreeSet::new();
    for file in files {
        let scope = file
            .document
            .scope
            .as_ref()
            .map(|scope| scope.name.text.as_str());
        let Some(names) = declared.get(&scope) else {
            continue;
        };
        for decl in &file.document.decls {
            let attrs: Vec<_> = match decl {
                Decl::Attr(attr) => vec![attr],
                Decl::Entity(entity) => entity.slots.iter().collect(),
                _ => Vec::new(),
            };
            for attr in attrs {
                if let Term::Sym(id) = &attr.value
                    && names.contains(id.text.as_str())
                {
                    entity_positions.insert((scope, attr.attr.text.as_str(), 1usize));
                }
            }
            if let Decl::Fact(fact) = decl {
                for (position, term) in fact.atom.args.iter().enumerate() {
                    if let Term::Sym(id) = term
                        && names.contains(id.text.as_str())
                    {
                        entity_positions.insert((scope, fact.atom.pred.text.as_str(), position));
                    }
                }
            }
        }
    }
    entity_positions
}

fn attr_symbols(attr: &dli_ast::decl::AttrDecl, include_value: bool) -> Vec<&Ident> {
    let mut symbols = vec![&attr.subject];
    if include_value && let Term::Sym(id) = &attr.value {
        symbols.push(id);
    }
    symbols
}

/// Levenshtein distance capped at three. Only the diagonal band within two
/// edits is relevant to a suggestion, so long identifiers take linear work.
fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).map(|n| n.min(3)).collect();
    let mut current = vec![3; right.len() + 1];
    for (offset, a) in left.chars().enumerate() {
        let i = offset + 1;
        let first = i.saturating_sub(2).max(1);
        let last = (i + 2).min(right.len());
        current[0] = i.min(3);
        if first > 1 && first - 1 < current.len() {
            current[first - 1] = 3;
        }
        if last + 1 < current.len() {
            current[last + 1] = 3;
        }
        let mut best = current[0];
        if first <= last {
            for j in first..=last {
                current[j] = (previous[j] + 1)
                    .min(current[j - 1] + 1)
                    .min(previous[j - 1] + usize::from(a != right[j - 1]))
                    .min(3);
                best = best.min(current[j]);
            }
        }
        if best > 2 {
            return 3;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::edit_distance;

    #[test]
    fn bounded_distance_matches_full_distance_on_short_strings() {
        fn words(prefix: &str, remaining: usize, out: &mut Vec<String>) {
            out.push(prefix.to_owned());
            if remaining > 0 {
                for ch in ['a', 'b'] {
                    let mut next = prefix.to_owned();
                    next.push(ch);
                    words(&next, remaining - 1, out);
                }
            }
        }
        fn full(left: &str, right: &str) -> usize {
            let mut previous: Vec<usize> = (0..=right.len()).collect();
            for (i, a) in left.bytes().enumerate() {
                let mut current = vec![i + 1];
                for (j, b) in right.bytes().enumerate() {
                    current.push(
                        (previous[j + 1] + 1)
                            .min(current[j] + 1)
                            .min(previous[j] + usize::from(a != b)),
                    );
                }
                previous = current;
            }
            previous[right.len()]
        }
        let mut corpus = Vec::new();
        words("", 5, &mut corpus);
        for left in &corpus {
            for right in &corpus {
                assert_eq!(
                    edit_distance(left, right),
                    full(left, right).min(3),
                    "{left:?}, {right:?}"
                );
            }
        }
    }
}
