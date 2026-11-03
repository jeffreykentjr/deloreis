//! `deloreis report [--min-grade G]` — the findings report.
//!
//! A CLI rendering over the check battery (`10-serving.md` §3: `report` is not a
//! worker op at v0, it is a rendering over `dli.check`/`dli.query` results). It
//! runs the world-check battery over the corpus and prints the findings whose
//! verification grade meets a floor — `--min-grade checked` keeps only the
//! exactly-re-derivable findings, `--min-grade asserted` keeps everything. Grades
//! are honest (charter koan 6): the report never launders a weaker grade upward.

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use dli_core::grade::Grade;
use dli_world::{check, language};

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// A verification grade floor, as a CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum GradeArg {
    /// Authorial fiat — the weakest evidence.
    Asserted,
    /// Rests on stored model judgment.
    Attested,
    /// Re-derives statistically.
    Tested,
    /// Re-derives exactly.
    Checked,
}

impl GradeArg {
    /// The engine [`Grade`] this floor maps to.
    fn grade(self) -> Grade {
        match self {
            GradeArg::Asserted => Grade::Asserted,
            GradeArg::Attested => Grade::Attested,
            GradeArg::Tested => Grade::Tested,
            GradeArg::Checked => Grade::Checked,
        }
    }
}

/// Arguments for `deloreis report`.
#[derive(Debug, Args)]
pub(crate) struct ReportArgs {
    /// The `.dli` corpus (files or directories) to report over.
    #[arg(short = 'c', long = "corpus", value_name = "PATH", required = true)]
    pub(crate) corpus: Vec<PathBuf>,
    /// Keep only findings whose grade meets this floor.
    #[arg(long = "min-grade", value_enum)]
    pub(crate) min_grade: Option<GradeArg>,
}

/// Render the findings report.
pub(crate) fn run(args: &ReportArgs) -> CmdResult {
    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "report")?;
    let floor = args.min_grade.map(GradeArg::grade);

    // Every finding paired with its verification grade. `check::run` includes
    // loader findings, so they must not be collected independently here.
    let mut graded = Vec::new();
    for file in &files {
        for diagnostic in &file.diagnostics {
            let severity = if diagnostic.class.is_error() {
                "error"
            } else {
                "warn"
            };
            let ladder = dli_syntax::ladder::render(
                file.source.strip_prefix('\u{feff}').unwrap_or(&file.source),
                std::slice::from_ref(diagnostic),
            );
            let details = ladder.split_once('\n').map_or("", |(_, details)| details);
            graded.push((
                Grade::Checked,
                format!(
                    "{severity}  {} — {}\n{}:\n{details}",
                    diagnostic.class.code(),
                    diagnostic.message,
                    file.path.display(),
                ),
            ));
        }
    }
    for finding in check::run(&world) {
        graded.push((
            finding.grade,
            cmd::check::render_semantic_finding(&finding, &files),
        ));
    }
    for finding in language::check(&world.model.language) {
        graded.push((
            finding.grade,
            cmd::check::render_semantic_finding(&finding, &files),
        ));
    }

    if let Some(floor) = floor {
        graded.retain(|(grade, _)| *grade >= floor);
    }
    graded.sort_by(|a, b| a.1.cmp(&b.1));

    for (_, line) in &graded {
        outln!("{line}");
    }

    let count = graded.len();
    match floor {
        Some(floor) => {
            outln!(
                "report: {count} finding(s) at grade {} or stronger",
                floor.as_str()
            );
        }
        None => outln!("report: {count} finding(s)"),
    }
    if count == 0 {
        Ok(EXIT_CLEAN)
    } else {
        Ok(EXIT_FINDINGS)
    }
}
