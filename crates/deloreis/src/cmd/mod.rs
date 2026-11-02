//! The subcommand handlers and the seams they share.
//!
//! Every handler returns a [`CmdResult`] — the verdict exit code on success, or
//! a human message on an operational failure the caller could not have known was
//! coming (a missing file, a corpus that will not compile). `main` maps an `Err`
//! to [`EXIT_USAGE`]. The three exit codes are uniform across the surface:
//! [`EXIT_CLEAN`], [`EXIT_FINDINGS`], [`EXIT_USAGE`].
//!
//! The world-reading handlers (`check`, `query`, `fork`, `timeline`, `name`,
//! `report`) share one front door: [`load_corpus`] turns a list of paths (files
//! or directories) into parsed [`CorpusFile`]s, [`compile_corpus`] lowers them
//! into a [`CompiledWorld`], and [`parse_goal`] reads a goal atom off the command
//! line through the same `parse_clause_atom` seam the ledger layer uses.
//!
//! All CLI output goes through [`emit`]/[`emit_raw`] (the `outln!`/`out!`
//! macros). The workspace forbids `unsafe`, so the process cannot reset the
//! `SIGPIPE` disposition the usual way; instead a downstream consumer closing the
//! pipe early (`head`, `less` then `q`) is treated as a clean stop rather than the
//! panic `println!` would raise on a broken pipe.

/// Write a formatted line to stdout, EPIPE-safe (see [`emit`]). Reads like
/// `println!`.
#[cfg(feature = "syntax")]
macro_rules! outln {
    () => { $crate::cmd::emit("") };
    ($($arg:tt)*) => { $crate::cmd::emit(&::std::format!($($arg)*)) };
}

/// Write formatted text to stdout without a trailing newline, EPIPE-safe (see
/// [`emit_raw`]). Reads like `print!`. Only the `.dli` handlers emit
/// pre-rendered blocks (diagnostic ladders), so it is theirs.
#[cfg(feature = "syntax")]
macro_rules! out {
    ($($arg:tt)*) => { $crate::cmd::emit_raw(&::std::format!($($arg)*)) };
}

// Text handlers are included by default and can be excluded with
// `--no-default-features` for parser-free deployments.
#[cfg(feature = "syntax")]
pub(crate) mod check;
#[cfg(feature = "syntax")]
pub(crate) mod drafts;
#[cfg(feature = "syntax")]
pub(crate) mod fmt;
#[cfg(feature = "syntax")]
pub(crate) mod fork;
#[cfg(feature = "syntax")]
pub(crate) mod name;
#[cfg(feature = "syntax")]
pub(crate) mod project;
#[cfg(feature = "syntax")]
pub(crate) mod query;
#[cfg(feature = "syntax")]
pub(crate) mod report;
pub(crate) mod serve;
#[cfg(feature = "syntax")]
pub(crate) mod timeline;

use std::io::Write;
#[cfg(feature = "syntax")]
use std::path::Path;

#[cfg(feature = "syntax")]
use std::fs;
#[cfg(feature = "syntax")]
use std::path::PathBuf;

#[cfg(feature = "syntax")]
use dli_ast::clause::Clause;
#[cfg(feature = "syntax")]
use dli_ast::decl::Document;
#[cfg(feature = "syntax")]
use dli_ast::span::Span;
#[cfg(feature = "syntax")]
use dli_ast::term::AtomExpr;
#[cfg(feature = "syntax")]
use dli_syntax::{Diagnostic, parse_clause_atom, parse_document};
#[cfg(feature = "syntax")]
use dli_world::{CompiledWorld, Pack, compile};
#[cfg(feature = "syntax")]
use dli_world::{Severity, WorldFinding};

/// A handler's result: the verdict exit code, or a message describing an
/// operational failure (mapped to [`EXIT_USAGE`] by `main`).
pub(crate) type CmdResult = Result<i32, String>;

/// The corpus is internally consistent and the operation found nothing to flag.
pub(crate) const EXIT_CLEAN: i32 = 0;
/// The operation produced findings — a diagnosed problem, a contradiction, or a
/// query that matched nothing. Not a failure of the call.
#[cfg(feature = "syntax")]
pub(crate) const EXIT_FINDINGS: i32 = 1;
/// A usage or operational error: bad arguments, a missing file, an engine that
/// has not landed.
pub(crate) const EXIT_USAGE: i32 = 2;

/// Write one line to stdout, exiting cleanly on a broken pipe.
///
/// A downstream consumer closing the pipe early (`head`, `less` then `q`) is
/// normal Unix behaviour, not a crash — but because the workspace forbids
/// `unsafe`, the process cannot reset the ignored `SIGPIPE` disposition, and a
/// bare `println!` would panic ("failed printing to stdout: Broken pipe"). So a
/// failed write here is a clean `exit(0)` instead. Prefer the `outln!` macro.
#[cfg(feature = "syntax")]
pub(crate) fn emit(line: &str) {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    if writeln!(lock, "{line}").is_err() {
        std::process::exit(EXIT_CLEAN);
    }
}

/// Write text to stdout with no trailing newline, EPIPE-safe like [`emit`]. Used
/// for pre-rendered blocks (the diagnostic ladder already carries its newlines).
/// Prefer the `out!` macro.
#[cfg(feature = "syntax")]
pub(crate) fn emit_raw(text: &str) {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    if write!(lock, "{text}").is_err() {
        std::process::exit(EXIT_CLEAN);
    }
}

/// Write one line to stderr, best-effort — the error-report path. A broken
/// stderr pipe leaves nothing useful to do, so the failure is ignored and the
/// caller proceeds to its exit code.
pub(crate) fn emit_err(line: &str) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    let _ = writeln!(lock, "{line}");
}

/// One source document read off disk: its path, its raw text (kept for the
/// ladder renderer), the parsed AST, and the parse diagnostics.
#[cfg(feature = "syntax")]
#[derive(Debug)]
pub(crate) struct CorpusFile {
    /// Where the document was read from.
    pub(crate) path: PathBuf,
    /// The raw source text, kept so diagnostics can be rendered with carets.
    pub(crate) source: String,
    /// The parsed document (partial when diagnostics are present).
    pub(crate) document: Document,
    /// The parse-time diagnostics for this file.
    pub(crate) diagnostics: Vec<Diagnostic>,
}

/// Read and parse every `.dli` document under the given paths.
///
/// A path may be a file (used directly) or a directory (walked for `*.dli`
/// files). Files are returned sorted by path so a multi-file corpus compiles and
/// reports deterministically. An unreadable path or an empty result is an
/// operational error.
#[cfg(feature = "syntax")]
pub(crate) fn load_corpus(paths: &[PathBuf]) -> Result<Vec<CorpusFile>, String> {
    if paths.is_empty() {
        return Err("no corpus given — pass one or more .dli files or a directory".to_owned());
    }
    let mut discovered: Vec<PathBuf> = Vec::new();
    for path in paths {
        let meta =
            fs::metadata(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        if meta.is_dir() {
            discover_dli(path, &mut discovered)?;
        } else {
            discovered.push(path.clone());
        }
    }
    discovered.sort();
    discovered.dedup();
    if discovered.is_empty() {
        return Err("no .dli documents found under the given paths".to_owned());
    }

    let mut files = Vec::with_capacity(discovered.len());
    for path in discovered {
        let source = fs::read_to_string(&path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        let (document, diagnostics) = parse_document(&source);
        files.push(CorpusFile {
            path,
            source,
            document,
            diagnostics,
        });
    }
    Ok(files)
}

/// Collect every `.dli` file under `dir`, recursively, in sorted order.
#[cfg(feature = "syntax")]
fn discover_dli(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|err| format!("cannot read directory {}: {err}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for entry in entries {
        let meta = fs::metadata(&entry)
            .map_err(|err| format!("cannot read {}: {err}", entry.display()))?;
        if meta.is_dir() {
            discover_dli(&entry, out)?;
        } else if entry.extension().is_some_and(|ext| ext == "dli") {
            out.push(entry);
        }
    }
    Ok(())
}

/// The parsed documents of a corpus, cloned for the loader.
#[cfg(feature = "syntax")]
pub(crate) fn documents(files: &[CorpusFile]) -> Vec<Document> {
    files.iter().map(|file| file.document.clone()).collect()
}

/// Compile a loaded corpus into a [`CompiledWorld`].
///
/// The pack name is cosmetic; the loader keys residency on each document's own
/// `scope` header. An unstratifiable rule set is the only compile error — a
/// malformed input is a *finding* held on the model, not a failure here.
#[cfg(feature = "syntax")]
pub(crate) fn compile_corpus(files: &[CorpusFile], name: &str) -> Result<CompiledWorld, String> {
    let pack = Pack {
        name: name.to_owned(),
        documents: documents(files),
    };
    compile(&pack).map_err(|err| format!("the corpus will not compile: {err}"))
}

/// Parse a goal atom off the command line — `pred(a, b)`, the `fact s r o`
/// triple, or a `subject.slot = value` attribute — through the shared
/// `parse_clause_atom` seam.
#[cfg(feature = "syntax")]
pub(crate) fn parse_goal(goal: &str) -> Result<AtomExpr, String> {
    let clause = Clause {
        text: goal.to_owned(),
        span: Span::synthetic(),
    };
    parse_clause_atom(&clause).ok_or_else(|| {
        format!(
            "could not read goal '{goal}' — write it as a relation `pred(a, b)`, \
             a triple `fact subject relation object`, or an attribute `subject.slot = value`"
        )
    })
}

/// The lowercase word for a finding's severity, for a stable report line.
#[cfg(feature = "syntax")]
pub(crate) fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warn => "warn",
    }
}

/// The lowercase word for a three-valued standing.
#[cfg(feature = "syntax")]
pub(crate) fn status_word(status: dli_core::ann::Status) -> &'static str {
    use dli_core::ann::Status;
    match status {
        Status::Certain => "certain",
        Status::Possible => "possible",
        Status::Dead => "dead",
    }
}

/// Render one world-check finding as a stable single line.
///
/// The shape is `<severity>  <code> — <message>` with any ledger citations
/// appended in brackets, so a report is greppable by code and diffable between
/// runs.
#[cfg(feature = "syntax")]
pub(crate) fn render_world_finding(finding: &WorldFinding) -> String {
    let citations = if finding.citations.is_empty() {
        String::new()
    } else {
        format!(" [{}]", finding.citations.join(", "))
    };
    format!(
        "{sev}  {code} — {msg}{citations}",
        sev = severity_word(finding.severity),
        code = finding.code,
        msg = finding.message,
    )
}
