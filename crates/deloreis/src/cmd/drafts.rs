//! Proof checking and exports over a compiled lore corpus.

use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use dli_ast::decl::{Decl, ProofDecl};
use dli_ast::proof::{ProofOp, ProofStep};
use dli_ast::span::Span;
use dli_export::profile::{ExportTarget, Registry};
use dli_export::{ExportOpts, export_with, source_fingerprint};
use dli_ledger::ledger_check::{ProofFinding, ProofReport, ProofVerdict, check_proof};
use dli_ledger::practice::{PracticeDef, compile_practices};
use dli_syntax::{ladder, parse_document};
use dli_world::scene_projection::project_scene;

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// Arguments for `deloreis check-proof`.
#[derive(Debug, Args)]
pub(crate) struct CheckProofArgs {
    /// The diegetic proof-block file to replay.
    #[arg(value_name = "PROOF")]
    pub(crate) proof: PathBuf,
    /// The `.dli` corpus (files or directories) the practice is declared in and
    /// the scene is projected from.
    #[arg(short = 'c', long = "corpus", value_name = "PATH", required = true)]
    pub(crate) corpus: Vec<PathBuf>,
    /// The proof declaration to replay. May be omitted when the file declares
    /// exactly one proof.
    #[arg(long = "proof", value_name = "NAME")]
    pub(crate) proof_name: Option<String>,
}

/// Replay a diegetic proof block against its practice and the scene it is spoken
/// in (PRAC-2). The corpus supplies the practice's law-set and the certain-layer
/// scene projection; the proof file supplies the casting.
pub(crate) fn run_check_proof(args: &CheckProofArgs) -> CmdResult {
    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "check-proof")?;

    // The practices declared across the whole corpus.
    let practices: Vec<PracticeDef> = files
        .iter()
        .flat_map(|file| compile_practices(&file.document))
        .collect();

    // Parse the proof file and extract its casting.
    let text = fs::read_to_string(&args.proof)
        .map_err(|err| format!("cannot read {}: {err}", args.proof.display()))?;
    let (proof_doc, diags) = parse_document(&text);
    let errors = diags
        .into_iter()
        .filter(|diag| diag.class.is_error())
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        out!(
            "{}:\n{}",
            args.proof.display(),
            ladder::render(text.strip_prefix('\u{feff}').unwrap_or(&text), &errors)
        );
        outln!(
            "proof: {} syntax finding(s); replay refused because malformed proof text \
             cannot be discarded",
            errors.len()
        );
        return Ok(EXIT_FINDINGS);
    }
    let proofs = proof_doc
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Proof(proof) => Some(proof),
            _ => None,
        })
        .collect::<Vec<_>>();
    let proof: &ProofDecl = match args.proof_name.as_deref() {
        Some(name) => {
            let matches = proofs
                .iter()
                .copied()
                .filter(|proof| proof.name.text == name)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [proof] => proof,
                [] => {
                    return Err(format!(
                        "no proof named '{name}' found in {}",
                        args.proof.display()
                    ));
                }
                _ => {
                    return Err(format!(
                        "proof '{name}' is declared more than once in {}",
                        args.proof.display()
                    ));
                }
            }
        }
        None => match proofs.as_slice() {
            [proof] => proof,
            [] => {
                return Err(format!(
                    "no proof block found in {} — a proof file declares `proof <name> … {{ … }}`",
                    args.proof.display()
                ));
            }
            _ => {
                let names = proofs
                    .iter()
                    .map(|proof| proof.name.text.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!(
                    "{} declares multiple proofs ({names}); pass --proof <name> to select one",
                    args.proof.display()
                ));
            }
        },
    };

    let practice_name = proof.practice().ok_or_else(|| {
        "the proof names no practice — write `in <practice>` in its header".to_owned()
    })?;
    let practice = practices
        .iter()
        .find(|practice| practice.name.text == practice_name)
        .ok_or_else(|| {
            format!(
                "the corpus declares no practice named '{practice_name}' — check the proof's \
                 `in <practice>` against the corpus"
            )
        })?;

    // Project the certain-layer scene for the proof's speaker, then check.
    let speaker = proof.actor().map(ToOwned::to_owned);
    let scene = project_scene(&world, speaker.as_deref());
    let report = check_proof(practice, proof, &scene);

    render_proof_report(&report, proof, &args.proof, &text);

    if report.checks {
        Ok(EXIT_CLEAN)
    } else {
        Ok(EXIT_FINDINGS)
    }
}

/// Hand-render a checked proof: its header, per-step verdicts, structural
/// findings, the reached verdict, and its weakest-link grade.
fn render_proof_report(report: &ProofReport, proof: &ProofDecl, path: &Path, source: &str) {
    let speaker = report
        .speaker
        .as_deref()
        .map_or_else(String::new, |who| format!(" (speaker {who})"));
    outln!(
        "proof {} in {}{speaker}",
        report.proof.text,
        report.practice.text,
    );
    for step in &report.steps {
        let mark = if step.ok { "ok  " } else { "fail" };
        outln!(
            "  {mark} {label}: {note}",
            label = step.label,
            note = step.note
        );
    }
    for finding in &report.findings {
        let location = proof_finding_span(proof, finding)
            .and_then(|span| proof_source_position(source, span))
            .map_or_else(String::new, |(line, column)| {
                format!(" ({}:{line}:{column})", path.display())
            });
        outln!(
            "finding  {code} — {message}{location}\n  help: {help}",
            code = finding.code(),
            message = finding.message(),
            help = proof_finding_help(finding),
        );
    }
    outln!("verdict: {}", describe_verdict(&report.verdict));
    outln!("grade: {}", report.grade);
    if report.checks {
        outln!("proof checks: the declared expectation was met");
    } else if report.expectation_met {
        outln!(
            "proof: {} finding(s); the declared expectation was met, but the proof did not check",
            report.findings.len()
        );
    } else {
        outln!(
            "proof: {} finding(s); the declared expectation was not met",
            report.findings.len()
        );
    }
}

fn proof_source_position(source: &str, span: Span) -> Option<(usize, usize)> {
    if span.end <= span.start {
        return None;
    }
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let prefix = source.get(..span.start as usize)?;
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    Some((line, column))
}

fn proof_finding_span(proof: &ProofDecl, finding: &ProofFinding) -> Option<Span> {
    use ProofFinding as F;
    let matches = |op: &ProofOp| match (finding, op) {
        (
            F::UnknownForm { at, .. } | F::BadInstantiation { at, .. } | F::InvalidChain { at, .. },
            ProofOp::Step(
                ProofStep::Instantiate { label, .. }
                | ProofStep::Chain { label, .. }
                | ProofStep::Custom { label, .. },
            ),
        ) => at == label,
        (
            F::PremiseAbsent { label, .. }
            | F::PremiseContradicted { label, .. }
            | F::WrongPhase { label, .. }
            | F::UnknownSpoke { label, .. },
            ProofOp::Premise(premise),
        ) => label == &premise.label,
        (F::InvalidSpend { keyword, label }, ProofOp::Resource(resource)) => {
            keyword == resource.keyword.keyword() && label == &resource.label
        }
        (F::ResourceViolation { at, .. }, ProofOp::Resource(resource)) => at == &resource.label,
        (
            F::CloseMismatch { .. } | F::ExpectationUnmet { .. } | F::UnknownExpectation { .. },
            ProofOp::Close(_),
        ) => true,
        _ => false,
    };
    let mut spans = proof.body.items.iter().filter_map(|item| {
        item.op
            .as_ref()
            .filter(|op| matches(op))
            .map(|_| item.clause.span)
    });
    match (spans.next(), spans.next()) {
        (Some(span), None) if span.end > span.start => Some(span),
        _ if proof.name.span.end > proof.name.span.start => Some(proof.name.span),
        _ => None,
    }
}

fn proof_finding_help(finding: &ProofFinding) -> &'static str {
    use ProofFinding as F;
    match finding {
        F::UnknownForm { .. } => "use a form declared by the practice or declare it",
        F::PremiseAbsent { .. } => "establish the premise in the scene or revise the proof",
        F::PremiseContradicted { .. } => {
            "resolve the conflicting scene claim or revise the premise"
        }
        F::WrongPhase { .. } => "use the required phase or revise the timing premise",
        F::BadInstantiation { .. } => "use bindings accepted by the declared form",
        F::UnknownRole { .. } => "use a role declared by the practice",
        F::InvalidChain { .. } => "use established premises and a law declared by the practice",
        F::ResourceViolation { .. } => "revise the resource operations to satisfy the declared law",
        F::CloseMismatch { .. } => "derive the close from established proof steps",
        F::ExpectationUnmet { .. } => "revise the proof or its declared expectation",
        F::InvalidSpend { .. } => "use a declared resource premise for the operation",
        F::UnknownSpoke { .. } => "use a phase declared by the cycle",
        F::UnknownExpectation { .. } => "use holds, blocked, or fails-at followed by an error",
        F::MissingClose => "add a final close to the proof",
        F::PracticeMismatch { .. } => "select the practice named by the proof",
        F::InvalidHeaderLayout => "write the proof header as role, actor, and practice",
        F::MalformedBody { .. } => "correct the malformed proof body",
        F::InvalidCloseLayout { .. } => "write exactly one final close",
    }
}

/// A one-line description of a reached proof verdict.
fn describe_verdict(verdict: &ProofVerdict) -> String {
    match verdict {
        ProofVerdict::Holds => "holds — the close follows from the accounted ledger".to_owned(),
        ProofVerdict::FailedAt { class, error, at } => {
            let named = error.as_deref().unwrap_or("no declared error");
            format!("failed at {at} ({} / {named})", class.meaning())
        }
    }
}

/// An export target, as a CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ProfileArg {
    /// A context pack for downstream generation.
    ContextPack,
    /// Articy-shaped video-game JSON (typed entities, who-may-know facts, quests).
    Articy,
    /// Producer-facing flat CSV.
    Csv,
    /// A static Markdown wiki bundle.
    StaticWiki,
}

/// Arguments for `deloreis export`.
#[derive(Debug, Args)]
pub(crate) struct ExportArgs {
    /// The export profile to render.
    #[arg(long, value_enum, value_name = "PROFILE")]
    pub(crate) profile: ProfileArg,
    /// The `.dli` corpus to export (required for the `context-pack` profile).
    #[arg(short = 'c', long = "corpus", value_name = "PATH")]
    pub(crate) corpus: Vec<PathBuf>,
    /// The source commit to pin in the manifest, tracing the artifact to an exact
    /// revision. Optional — when omitted, a content fingerprint of the loaded source
    /// is derived instead, so two different corpora never share a manifest even when
    /// their decision set (valuation) and profile match.
    #[arg(long = "commit", value_name = "SHA")]
    pub(crate) commit: Option<String>,
    /// Optional JSON profile catalog for a lore corpus.
    #[arg(long = "profiles", value_name = "PATH")]
    pub(crate) profiles: Option<PathBuf>,
    /// Optional profile name or qualified `name@version`; otherwise the target's
    /// last registered profile is used.
    #[arg(long = "profile-name", value_name = "NAME")]
    pub(crate) profile_name: Option<String>,
    /// Project temporal states at this era or event.
    #[arg(long = "as-of", value_name = "POINT")]
    pub(crate) as_of: Option<String>,
}

/// Render a compiled world under the requested profile and print its manifest.
pub(crate) fn run_export(args: &ExportArgs) -> CmdResult {
    match args.profile {
        ProfileArg::ContextPack => {
            run_export_world(args, ExportTarget::ContextPack, "context-pack")
        }
        ProfileArg::Articy => run_export_world(args, ExportTarget::Articy, "articy"),
        ProfileArg::Csv => run_export_world(args, ExportTarget::Csv, "csv"),
        ProfileArg::StaticWiki => run_export_world(args, ExportTarget::StaticWiki, "static-wiki"),
    }
}

/// Export the certain layer of a compiled lore world.
fn run_export_world(args: &ExportArgs, target: ExportTarget, label: &str) -> CmdResult {
    if args.corpus.is_empty() {
        return Err(format!(
            "the {label} export needs a corpus: pass -c/--corpus one or more .dli files"
        ));
    }
    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "export")?;
    let registry = load_export_registry(args.profiles.as_deref())?;
    let opts = ExportOpts {
        source_commit: export_commit(
            args.commit.as_ref(),
            files.iter().map(|file| file.source.as_bytes()),
        ),
        profile_name: args.profile_name.clone(),
        as_of: args.as_of.clone(),
        ..ExportOpts::default()
    };
    let outcome = export_with(&registry, target, &world, &opts)
        .map_err(|err| format!("the {label} export failed: {err}"))?;
    render_export_json(&outcome)?;
    Ok(EXIT_CLEAN)
}

fn load_export_registry(path: Option<&Path>) -> Result<Registry, String> {
    let Some(path) = path else {
        return Ok(Registry::builtin());
    };
    let loaded = Registry::from_path(path)
        .map_err(|err| format!("cannot load export profiles {}: {err}", path.display()))?;
    Ok(Registry::builtin().merge(loaded))
}

/// The manifest `commit` for an export: the `--commit` value when supplied
/// (non-empty), else a SHA-256 fingerprint of the source byte set. The offline
/// kernel reads no git, so a caller who pins no commit still gets a manifest that
/// traces to the exact bytes the artifact was built from.
fn export_commit<'a>(
    explicit: Option<&String>,
    contents: impl IntoIterator<Item = &'a [u8]>,
) -> String {
    if let Some(commit) = explicit {
        let trimmed = commit.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    source_fingerprint(contents)
}

/// Emit the complete artifact and manifest as one machine-readable JSON value.
fn render_export_json<T: serde::Serialize>(export: &T) -> Result<(), String> {
    let rendered = serde_json::to_string_pretty(export)
        .map_err(|err| format!("cannot serialize export artifact: {err}"))?;
    outln!("{rendered}");
    Ok(())
}
