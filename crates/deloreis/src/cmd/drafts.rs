//! The payload, proof, simulation, and export ops — all wired to their real engine
//! crates (`DL-0021`).
//!
//! `check-draft` runs the merged fact-check battery (`dli_world::check_draft`) over
//! a `payload/v1` document. `check-proof` compiles the corpus, projects the
//! certain-layer scene, and replays a diegetic proof block against its practice
//! (PRAC-2), returning the rich per-step `ProofReport`. `simulate` runs a bounded
//! seeded battery over a declared practice — or a card ruleset with `--game` — and
//! renders its honest run manifest (SIM-1). `export` renders a compiled world
//! (context pack, articy) or a card ruleset (cards, Ludii GDL) under a profile,
//! pinning a provenance manifest (EXP-1). Each returns a rendered result and a
//! verdict-reflecting exit code — never a `todo!` panic or a guessed answer.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use dli_ast::decl::{Decl, ProofDecl};
use dli_export::profile::{ExportTarget, ProfileSpec, ProvenanceMode, Registry, Selection};
use dli_export::{
    ExportArtifact, ExportManifest, ExportOpts, ExportProfile, export_wire, export_with,
};
use dli_game::export::GameArtifact;
use dli_game::model::{Ruleset, Verdict};
use dli_game::{DealBattery, GameBindings, ReadingInvariant, ReadingMetric, ReadingObservable};
use dli_ledger::ledger_check::{ProofReport, ProofVerdict, check_proof};
use dli_ledger::practice::{PracticeDef, compile_practices};
use dli_sim::{
    Battery, GenBounds, Invariant, InvariantOutcome, InvariantSpec, Metric, Observable, RunManifest,
};
use dli_syntax::parse_document;
use dli_world::scene_projection::project_scene;
use dli_world::{DraftPayload, check_draft};

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// The default trial count when `--games` is not supplied.
const SIM_DEFAULT_TRIALS: u64 = 256;
/// The default generator bound on deltas drawn per resource per trial.
const SIM_DEFAULT_MAX_STEPS: u32 = 6;
/// The default generator bound on a single delta's magnitude.
const SIM_DEFAULT_MAX_MAGNITUDE: u32 = 3;

/// Arguments for `deloreis check-draft`.
#[derive(Debug, Args)]
pub(crate) struct CheckDraftArgs {
    /// The `payload/v1` draft document (YAML) to fact-check.
    #[arg(value_name = "PAYLOAD")]
    pub(crate) payload: PathBuf,
    /// The `.dli` corpus (files or directories) to check the draft against.
    #[arg(short = 'c', long = "corpus", value_name = "PATH", required = true)]
    pub(crate) corpus: Vec<PathBuf>,
}

/// Fact-check a draft payload against the compiled world.
pub(crate) fn run_check_draft(args: &CheckDraftArgs) -> CmdResult {
    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "check-draft")?;

    let text = fs::read_to_string(&args.payload)
        .map_err(|err| format!("cannot read {}: {err}", args.payload.display()))?;
    let draft: DraftPayload = serde_yaml_ng::from_str(&text).map_err(|err| {
        format!(
            "the draft payload {} is malformed: {err}",
            args.payload.display()
        )
    })?;

    let mut findings = check_draft(&world, &draft);
    findings.sort_by(|a, b| (&a.claim, &a.message).cmp(&(&b.claim, &b.message)));
    for finding in &findings {
        outln!(
            "error  {claim} — {message} ({grade})",
            claim = finding.claim,
            message = finding.message,
            grade = finding.grade,
        );
    }

    if findings.is_empty() {
        outln!("draft clean: {} claim(s) supported", draft.claims.len());
        Ok(EXIT_CLEAN)
    } else {
        outln!("draft: {} defect(s)", findings.len());
        Ok(EXIT_FINDINGS)
    }
}

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
    /// The practice the casting runs in. Optional — the proof's `in <practice>`
    /// header names it; `--in` overrides that when supplied.
    #[arg(long = "in", value_name = "PRACTICE")]
    pub(crate) practice: Option<String>,
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
    let (proof_doc, _diags) = parse_document(&text);
    let proof: &ProofDecl = proof_doc
        .decls
        .iter()
        .find_map(|decl| match decl {
            Decl::Proof(proof) => Some(proof),
            _ => None,
        })
        .ok_or_else(|| {
            format!(
                "no proof block found in {} — a proof file declares `proof <name> … {{ … }}`",
                args.proof.display()
            )
        })?;

    // The practice to check against: `--in` overrides the proof header's `in`.
    let practice_name = args
        .practice
        .clone()
        .or_else(|| header_value(&proof.head, "in"))
        .ok_or_else(|| {
            "the proof names no practice — write `in <practice>` in its header or pass --in"
                .to_owned()
        })?;
    let practice = practices
        .iter()
        .find(|practice| practice.name.text == practice_name)
        .ok_or_else(|| {
            format!(
                "the corpus declares no practice named '{practice_name}' — check the proof's \
                 `in <practice>` or --in against the corpus"
            )
        })?;

    // Project the certain-layer scene for the proof's speaker, then check.
    let speaker = header_value(&proof.head, "speaker");
    let scene = project_scene(&world, speaker.as_deref());
    let report = check_proof(practice, proof, &scene);

    render_proof_report(&report);

    if report.checks {
        Ok(EXIT_CLEAN)
    } else {
        Ok(EXIT_FINDINGS)
    }
}

/// Read the value following a header keyword (`in <practice>`, `speaker <who>`)
/// from a proof declaration's flat header token list.
fn header_value(head: &[String], key: &str) -> Option<String> {
    head.iter()
        .position(|token| token == key)
        .and_then(|i| head.get(i + 1))
        .cloned()
}

/// Hand-render a checked proof: its header, per-step verdicts, structural
/// findings, the reached verdict, and its weakest-link grade.
fn render_proof_report(report: &ProofReport) {
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
        outln!(
            "finding  {code} — {message}",
            code = finding.code(),
            message = finding.message(),
        );
    }
    outln!("verdict: {}", describe_verdict(&report.verdict));
    outln!("grade: {}", report.grade);
    if report.checks {
        outln!("proof checks: the declared expectation was met");
    } else {
        outln!(
            "proof: {} finding(s); the declared expectation was not met",
            report.findings.len()
        );
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

/// Arguments for `deloreis simulate`.
#[derive(Debug, Args)]
pub(crate) struct SimulateArgs {
    /// The target to simulate: a corpus practice name (the default path), or a
    /// dealable layout name when `--game` names a card ruleset.
    #[arg(value_name = "TARGET")]
    pub(crate) target: String,
    /// The `.dli` corpus the practice is declared in (required for a practice
    /// simulation; unused for a `--game` deal battery).
    #[arg(short = 'c', long = "corpus", value_name = "PATH")]
    pub(crate) corpus: Vec<PathBuf>,
    /// Simulate a card ruleset instead of a practice: a directory holding
    /// `rules.json` (and optional `profiles.json`), or a direct `rules.json` path.
    /// `TARGET` then names the layout to deal.
    #[arg(long = "game", value_name = "PATH")]
    pub(crate) game: Option<PathBuf>,
    /// The number of trials to run (`--games` → `trials`). Default 256.
    #[arg(long, value_name = "N")]
    pub(crate) games: Option<u64>,
    /// The master seed. Default 0.
    #[arg(long, value_name = "SEED")]
    pub(crate) seed: Option<u64>,
    /// Bind a symbolic affine cap so its law is ENFORCED (not deferred), e.g.
    /// `--cap megin=5`. Repeatable; keyed by resource name. An unbound symbolic
    /// cap stays honestly `deferred`.
    #[arg(long = "cap", value_name = "RESOURCE=N")]
    pub(crate) caps: Vec<String>,
}

/// Parse repeated `--cap <resource>=<n>` flags into the battery's `affine_caps`
/// (resource name → integer bound). An `affine bounded <symbolic-cap>` law is
/// enforced only when its resource is bound here.
fn parse_caps(specs: &[String]) -> Result<std::collections::BTreeMap<String, i64>, String> {
    let mut caps = std::collections::BTreeMap::new();
    for spec in specs {
        let (res, n) = spec
            .split_once('=')
            .ok_or_else(|| format!("--cap expects <resource>=<n>, got '{spec}'"))?;
        let res = res.trim();
        if res.is_empty() {
            return Err(format!("--cap '{spec}': empty resource name"));
        }
        let bound: i64 = n
            .trim()
            .parse()
            .map_err(|_| format!("--cap '{spec}': '{}' is not an integer bound", n.trim()))?;
        if bound < 0 {
            return Err(format!(
                "--cap '{spec}': a cap must be >= 0 (a negative bound would silently enforce 0)"
            ));
        }
        caps.insert(res.to_owned(), bound);
    }
    Ok(caps)
}

/// Run a seeded battery over a declared practice (or a card ruleset with
/// `--game`) and render its honest run manifest (SIM-1).
pub(crate) fn run_simulate(args: &SimulateArgs) -> CmdResult {
    match &args.game {
        Some(game) => run_simulate_game(args, game),
        None => run_simulate_practice(args),
    }
}

/// Simulate a declared practice: a bounded seeded generator over its resources,
/// graded by a synthesized default battery.
///
/// **Synthesized defaults** (documented, overridable by args): the generator draws
/// up to [`SIM_DEFAULT_MAX_STEPS`] deltas of magnitude ≤ [`SIM_DEFAULT_MAX_MAGNITUDE`]
/// per resource per trial; `trials` = `--games` or [`SIM_DEFAULT_TRIALS`]; `seed` =
/// `--seed` or 0. The metrics are an `any-violation` rate plus a per-resource
/// `<res>-violated` rate; the invariants are `all-laws-hold` plus a per-resource
/// `<res>-never-violated` sweep. Affine caps bind from `--cap <resource>=<n>`; an
/// unbound symbolic cap defers its check honestly, never guesses.
fn run_simulate_practice(args: &SimulateArgs) -> CmdResult {
    if args.corpus.is_empty() {
        return Err(
            "a practice simulation needs a corpus: pass -c/--corpus (or --game for a card ruleset)"
                .to_owned(),
        );
    }
    let files = cmd::load_corpus(&args.corpus)?;
    let practice = files
        .iter()
        .flat_map(|file| compile_practices(&file.document))
        .find(|practice| practice.name.text == args.target)
        .ok_or_else(|| {
            format!(
                "the corpus declares no practice named '{}' — name a practice or use --game",
                args.target
            )
        })?;

    let bounds = GenBounds {
        max_steps: SIM_DEFAULT_MAX_STEPS,
        max_magnitude: SIM_DEFAULT_MAX_MAGNITUDE,
    };
    let (metrics, invariants) = default_practice_grading(&practice);
    let affine_caps = parse_caps(&args.caps)?;
    let battery = Battery {
        practice,
        bounds,
        seed: args.seed.unwrap_or(0),
        trials: args.games.unwrap_or(SIM_DEFAULT_TRIALS),
        metrics,
        invariants,
        affine_caps,
    };
    for finding in battery.validate() {
        outln!("config  {finding}");
    }
    let manifest = battery.run();
    outln!(
        "simulate practice '{}' (default battery: max_steps {}, max_magnitude {})",
        args.target,
        bounds.max_steps,
        bounds.max_magnitude,
    );
    Ok(render_manifest(&manifest))
}

/// The synthesized default metrics and invariants for a practice battery: an
/// `any-violation` rate and `all-laws-hold` sweep, plus one per declared resource.
fn default_practice_grading(practice: &PracticeDef) -> (Vec<Metric>, Vec<InvariantSpec>) {
    let mut metrics = vec![Metric {
        name: "any-violation".to_owned(),
        observable: Observable::AnyViolation,
    }];
    let mut invariants = vec![InvariantSpec {
        name: "all-laws-hold".to_owned(),
        predicate: Invariant::AllLawsHold,
    }];
    for resource in &practice.resources {
        let name = resource.name.text.clone();
        metrics.push(Metric {
            name: format!("{name}-violated"),
            observable: Observable::Violated {
                resource: name.clone(),
            },
        });
        invariants.push(InvariantSpec {
            name: format!("{name}-never-violated"),
            predicate: Invariant::NeverViolated {
                resource: name.clone(),
            },
        });
    }
    (metrics, invariants)
}

/// Simulate a card ruleset: deal `TARGET` layout from a seeded stream and grade a
/// completed-rate metric plus the deck-conservation and legal-play sweeps.
fn run_simulate_game(args: &SimulateArgs, game: &Path) -> CmdResult {
    let ruleset = load_ruleset(game)?;
    let layout = args.target.clone();
    if ruleset
        .available_layouts()
        .iter()
        .all(|dealable| dealable.name != layout)
    {
        let names: Vec<&str> = ruleset
            .available_layouts()
            .iter()
            .map(|dealable| dealable.name.as_str())
            .collect();
        return Err(format!(
            "the deck has no dealable layout named '{layout}' — available: {}",
            names.join(", ")
        ));
    }
    let battery = DealBattery {
        ruleset,
        layout: layout.clone(),
        seed: args.seed.unwrap_or(0),
        trials: args.games.unwrap_or(SIM_DEFAULT_TRIALS),
        metrics: vec![ReadingMetric {
            name: "completed-rate".to_owned(),
            observable: ReadingObservable::VerdictIs {
                verdict_id: Verdict::COMPLETED.to_owned(),
            },
        }],
        invariants: vec![
            ReadingInvariant::DeckConserved,
            ReadingInvariant::OnlyLegalPlays,
        ],
    };
    let manifest = battery.run(&GameBindings::new());
    outln!(
        "simulate deck '{}' layout '{layout}'",
        battery.ruleset.game_id
    );
    Ok(render_manifest(&manifest))
}

/// Load a card [`Ruleset`] from a `--game` path: a directory's `rules.json`, or a
/// direct rules file.
fn load_ruleset(path: &Path) -> Result<Ruleset, String> {
    let rules_path = if path.is_dir() {
        path.join("rules.json")
    } else {
        path.to_path_buf()
    };
    let text = fs::read_to_string(&rules_path)
        .map_err(|err| format!("cannot read {}: {err}", rules_path.display()))?;
    Ruleset::from_rules(&text).map_err(|err| {
        format!(
            "the game rules {} are malformed: {err}",
            rules_path.display()
        )
    })
}

/// Render an honest run manifest — metrics (`tested`), invariant verdicts
/// (`checked`), and the honesty line — returning the verdict exit code
/// ([`EXIT_FINDINGS`] if any invariant was breached, else [`EXIT_CLEAN`]).
fn render_manifest(manifest: &RunManifest) -> i32 {
    outln!("run: {} trial(s), seed {}", manifest.trials, manifest.seed);
    for metric in &manifest.metrics {
        outln!(
            "metric  {}: mean {:.4} ±{:.4} (n={}, sum={}) [{}]",
            metric.metric,
            metric.mean,
            metric.half_width,
            metric.trials,
            metric.sum,
            metric.grade(),
        );
    }
    let mut breached = false;
    for verdict in &manifest.invariants {
        match &verdict.outcome {
            InvariantOutcome::Held => {
                outln!(
                    "invariant  {}: held [{}]",
                    verdict.invariant,
                    verdict.grade()
                );
            }
            InvariantOutcome::Violated { trial, detail } => {
                breached = true;
                outln!(
                    "invariant  {}: violated at trial {trial} — {detail} [{}]",
                    verdict.invariant,
                    verdict.grade(),
                );
            }
            // An unenforced law (an unresolved affine cap) never swept anything, so
            // it is honestly deferred — never "held [checked]", which would read as
            // a verified pass. Not a breach: it does not fail the run.
            InvariantOutcome::Deferred { reason } => {
                outln!(
                    "invariant  {}: deferred — {reason} (law not enforced, not checked)",
                    verdict.invariant,
                );
            }
        }
    }
    let honesty = if manifest.grades_are_honest() {
        "grades are honest (metrics tested, invariants checked)"
    } else {
        "DISHONEST — a grade was forged"
    };
    outln!("honesty: {honesty}");
    outln!("battery {}", manifest.digest());
    if breached { EXIT_FINDINGS } else { EXIT_CLEAN }
}

/// An export target, as a CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ProfileArg {
    /// A context pack for downstream generation.
    ContextPack,
    /// Printable reference cards.
    Cards,
    /// Ludii Game Description Language for the simulator.
    LudiiGdl,
    /// Articy-shaped video-game JSON (typed entities, who-may-know facts, quests).
    Articy,
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
    /// The card ruleset to export (required for the `cards` / `ludii-gdl`
    /// profiles): a directory holding `rules.json` (and optional `profiles.json`),
    /// or a direct `rules.json` path.
    #[arg(long = "game", value_name = "PATH")]
    pub(crate) game: Option<PathBuf>,
    /// The source commit to pin in the manifest, tracing the artifact to an exact
    /// revision. Optional — when omitted, a content fingerprint of the loaded source
    /// is derived instead, so two different corpora never share a manifest even when
    /// their decision set (valuation) and profile match.
    #[arg(long = "commit", value_name = "SHA")]
    pub(crate) commit: Option<String>,
}

/// Render a compiled world (context pack) or a card ruleset (cards / Ludii GDL)
/// under the requested profile, printing its provenance manifest (EXP-1/EXP-3).
pub(crate) fn run_export(args: &ExportArgs) -> CmdResult {
    match args.profile {
        ProfileArg::ContextPack => run_export_context_pack(args),
        ProfileArg::Articy => run_export_articy(args),
        ProfileArg::Cards => run_export_game(args, ExportTarget::Cards, "cards"),
        ProfileArg::LudiiGdl => run_export_game(args, ExportTarget::LudiiGdl, "ludii-gdl"),
    }
}

/// Export the certain layer of a compiled world as a context pack (EXP-2).
fn run_export_context_pack(args: &ExportArgs) -> CmdResult {
    if args.corpus.is_empty() {
        return Err(
            "the context-pack export needs a corpus: pass -c/--corpus one or more .dli files"
                .to_owned(),
        );
    }
    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "export")?;
    let opts = ExportOpts {
        source_commit: export_commit(args.commit.as_ref(), &corpus_material(&files)),
        ..ExportOpts::default()
    };
    let outcome = export_wire(ExportProfile::ContextPack, &world, &opts)
        .map_err(|err| format!("the context-pack export failed: {err}"))?;
    let pack = outcome
        .artifact
        .as_context_pack()
        .ok_or("a context-pack export must yield a context pack")?;
    outln!(
        "export context-pack: {} unit(s) included, {} skipped, ~{} tokens",
        pack.included.len(),
        pack.skipped.len(),
        pack.est_tokens,
    );
    render_export_manifest(&outcome.manifest);
    Ok(EXIT_CLEAN)
}

/// Export the certain layer of a compiled world as articy-shaped video-game JSON
/// (EXP-4): typed entities carrying their fact scopes, who-may-know facts, typed
/// relations, and quest flow. Fail-closed on proposed lore (certain layer only),
/// exactly like the context pack.
fn run_export_articy(args: &ExportArgs) -> CmdResult {
    if args.corpus.is_empty() {
        return Err(
            "the articy export needs a corpus: pass -c/--corpus one or more .dli files".to_owned(),
        );
    }
    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "export")?;
    let opts = ExportOpts {
        source_commit: export_commit(args.commit.as_ref(), &corpus_material(&files)),
        ..ExportOpts::default()
    };
    let outcome = export_with(
        &neutral_articy_registry(),
        ExportTarget::Articy,
        &world,
        &opts,
    )
    .map_err(|err| format!("the articy export failed: {err}"))?;
    let articy = match &outcome.artifact {
        ExportArtifact::Articy(articy) => articy,
        ExportArtifact::ContextPack(_) => {
            return Err("the articy export produced a non-articy artifact".to_owned());
        }
    };
    outln!(
        "export articy: {} entit{}, {} relation(s), {} fact(s), {} quest(s)",
        articy.entities.len(),
        if articy.entities.len() == 1 {
            "y"
        } else {
            "ies"
        },
        articy.relations.len(),
        articy.facts.len(),
        articy.quests.len(),
    );
    render_export_manifest(&outcome.manifest);
    Ok(EXIT_CLEAN)
}

/// The neutral engine-default articy profile registry: a generic `video/articy`
/// format selector naming no world sorts or relations — the articy analogue of the
/// builtin `context-pack/default`. A world that ships a richer `video/articy@N`
/// catalog under `worlds/<world>/export/profiles.json` would load and merge it (as
/// the `--game` exports merge card profiles); a lore corpus uses this neutral base.
fn neutral_articy_registry() -> Registry {
    Registry {
        profiles: vec![ProfileSpec {
            target: ExportTarget::Articy,
            name: "video/articy".to_owned(),
            version: 1,
            selects: Selection::default(),
            renders: BTreeMap::new(),
            carries_provenance: ProvenanceMode::DevMetadata,
        }],
    }
}

/// Export a card ruleset as printable cards or Ludii GDL (EXP-3), sourcing its
/// profile registry from the built-in catalog merged with the game's own
/// `profiles.json` when present.
fn run_export_game(args: &ExportArgs, target: ExportTarget, label: &str) -> CmdResult {
    let game = args.game.as_deref().ok_or_else(|| {
        format!(
            "the {label} export needs a card ruleset: pass --game <dir-or-rules.json> \
             (a lore corpus carries no ruleset)"
        )
    })?;
    let ruleset = load_ruleset(game)?;
    let registry = game_registry(game)?;
    let opts = ExportOpts {
        source_commit: export_commit(args.commit.as_ref(), &ruleset_material(game)),
        ..ExportOpts::default()
    };
    let export = dli_game::export::run(&registry, target, &ruleset, &opts)
        .map_err(|err| format!("the {label} export failed: {err}"))?;
    match &export.artifact {
        GameArtifact::Cards(deck) => outln!(
            "export cards: deck '{}', {} card(s), layout(s): {}",
            deck.deck,
            deck.card_count,
            deck.layouts.join(", "),
        ),
        GameArtifact::LudiiGdl(ludii) => outln!(
            "export ludii-gdl: {} byte(s) of L-GDL, differential {}",
            ludii.program.len(),
            differential_word(&ludii.differential),
        ),
    }
    render_export_manifest(&export.manifest);
    Ok(EXIT_CLEAN)
}

/// The profile registry for a `--game` export: the built-in catalog, merged with
/// the game's own `profiles.json` when it ships one.
fn game_registry(game: &Path) -> Result<Registry, String> {
    let profiles_path = if game.is_dir() {
        game.join("profiles.json")
    } else {
        game.with_file_name("profiles.json")
    };
    if profiles_path.exists() {
        let world = Registry::from_path(&profiles_path).map_err(|err| {
            format!(
                "cannot load export profiles {}: {err}",
                profiles_path.display()
            )
        })?;
        Ok(Registry::builtin().merge(world))
    } else {
        Ok(Registry::builtin())
    }
}

/// The one-word status of a Ludii trace-agreement differential.
fn differential_word(status: &dli_game::export::DifferentialStatus) -> String {
    use dli_game::export::DifferentialStatus;
    match status {
        DifferentialStatus::Pending { .. } => "pending".to_owned(),
        DifferentialStatus::Agreed { positions } => format!("agreed over {positions} position(s)"),
    }
}

/// The manifest `commit` for an export: the `--commit` value when supplied
/// (non-empty), else a best-effort FNV-1a content fingerprint of the source
/// material. The offline kernel reads no git, so a caller who pins no commit still
/// gets a manifest that traces to the exact bytes the artifact was built from —
/// two different corpora never collide on a byte-identical manifest even when their
/// valuation and profile hash match.
fn export_commit(explicit: Option<&String>, material: &[u8]) -> String {
    if let Some(commit) = explicit {
        let trimmed = commit.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    // FNV-1a 64: dependency-free, deterministic, and the same 64-bit fingerprint
    // family the worker manifests already use (`fnv1a64:`).
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in material {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("content-fnv1a64:{hash:016x}")
}

/// The source material a corpus export's content fingerprint is taken over: each
/// file's raw text ONLY — never its path. The fingerprint must trace the exact
/// source *bytes*, so identical content at a different path (or in a different
/// checkout location) yields the same fingerprint. Sorted so file/load order does
/// not change it either.
fn corpus_material(files: &[cmd::CorpusFile]) -> Vec<u8> {
    let mut contents: Vec<&[u8]> = files.iter().map(|f| f.source.as_bytes()).collect();
    contents.sort_unstable();
    let mut material = Vec::new();
    for content in contents {
        material.extend_from_slice(content);
        material.push(0);
    }
    material
}

/// The source material a `--game` export's fingerprint is taken over: the resolved
/// rules file bytes. Best-effort — an unreadable file yields empty material rather
/// than failing the export (the fingerprint is a convenience, never a gate).
fn ruleset_material(game: &Path) -> Vec<u8> {
    let rules_path = if game.is_dir() {
        game.join("rules.json")
    } else {
        game.to_path_buf()
    };
    fs::read(&rules_path).unwrap_or_default()
}

/// Render an export's provenance manifest — the layer, valuation fingerprint,
/// profile hash, and pinned commit that trace an artifact to its exact source.
fn render_export_manifest(manifest: &ExportManifest) {
    outln!("  layer: {}", manifest.layer);
    outln!("  valuation: {}", manifest.valuation);
    outln!("  profile: {}", manifest.profile_hash);
    let commit = if manifest.commit.is_empty() {
        "(none supplied)"
    } else {
        manifest.commit.as_str()
    };
    outln!("  commit: {commit}");
}
