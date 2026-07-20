//! `deloreis timeline <read>` — the five timeline reads.
//!
//! Two times meet here and stay apart (`03-time.md`): the world chronology (an
//! era spine and events as partially-ordered points) and the episode fold
//! (production order — the listener's dimension). `show` and `order` read the
//! compiled world's chronology; `knows`, `episodes`, and `phases` read the
//! episode fold over a YAML payload (episodes are serde payloads until their
//! `.dli` surface lands with TIME-3). Story-time is honestly partial: `order`
//! reports unordered pairs as first-class, never a silent tie-break.

use std::fs;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use dli_eval::world::World;
use dli_time::chronology::{TemporalVerdict, TimePoint};
use dli_world::episode::{Episode, Knower, fold};

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// Arguments for `deloreis timeline`.
#[derive(Debug, Args)]
pub(crate) struct TimelineArgs {
    /// The timeline read.
    #[command(subcommand)]
    pub(crate) read: TimelineRead,
}

/// The five timeline reads.
#[derive(Debug, Subcommand)]
pub(crate) enum TimelineRead {
    /// Render the era/event partial order (optionally around a cursor).
    Show(ShowArgs),
    /// The partial-order report: cycles and load-bearing unordered pairs.
    Order(CorpusArgs),
    /// What a knower knows once the listener reaches a production slot.
    Knows(KnowsArgs),
    /// Fold the season and check its production-order layout and open loops.
    Episodes(EpisodesArgs),
    /// The cyclic phases an episode pins.
    Phases(PhasesArgs),
}

/// A corpus for a chronology read.
#[derive(Debug, Args)]
pub(crate) struct CorpusArgs {
    /// The `.dli` corpus (files or directories) whose chronology to read.
    #[arg(short = 'c', long = "corpus", value_name = "PATH", required = true)]
    pub(crate) corpus: Vec<PathBuf>,
}

/// Arguments for `timeline show`.
#[derive(Debug, Args)]
pub(crate) struct ShowArgs {
    /// The `.dli` corpus (files or directories) whose chronology to read.
    #[arg(short = 'c', long = "corpus", value_name = "PATH", required = true)]
    pub(crate) corpus: Vec<PathBuf>,
    /// An era or event to place the cursor at.
    #[arg(long, value_name = "POINT")]
    pub(crate) at: Option<String>,
}

/// Arguments for `timeline knows`.
#[derive(Debug, Args)]
pub(crate) struct KnowsArgs {
    /// The episode payload YAML to fold.
    #[arg(short = 'e', long = "episodes", value_name = "FILE")]
    pub(crate) episodes: PathBuf,
    /// The production slot to read knowledge at.
    #[arg(long, value_name = "SLOT")]
    pub(crate) at: u32,
    /// A cast knower; omitted reads the listener's ledger.
    #[arg(long, value_name = "ID")]
    pub(crate) who: Option<String>,
}

/// Arguments for `timeline episodes`.
#[derive(Debug, Args)]
pub(crate) struct EpisodesArgs {
    /// The episode payload YAML to fold.
    #[arg(short = 'e', long = "episodes", value_name = "FILE")]
    pub(crate) episodes: PathBuf,
    /// Run the checks (reserved parity with the op man-page; always on at v0).
    #[arg(long)]
    pub(crate) check: bool,
}

/// Arguments for `timeline phases`.
#[derive(Debug, Args)]
pub(crate) struct PhasesArgs {
    /// The episode payload YAML to fold.
    #[arg(short = 'e', long = "episodes", value_name = "FILE")]
    pub(crate) episodes: PathBuf,
    /// The episode id whose phases to show.
    #[arg(value_name = "EPISODE")]
    pub(crate) episode: String,
}

/// Run a `deloreis timeline` read.
pub(crate) fn run(args: &TimelineArgs) -> CmdResult {
    match &args.read {
        TimelineRead::Show(show) => run_show(show),
        TimelineRead::Order(corpus) => run_order(corpus),
        TimelineRead::Knows(knows) => run_knows(knows),
        TimelineRead::Episodes(episodes) => run_episodes(episodes),
        TimelineRead::Phases(phases) => run_phases(phases),
    }
}

/// The name of a time point, or a stable fallback.
fn point_name(world: &World, point: TimePoint) -> String {
    world
        .time_points
        .names()
        .get(point.0 as usize)
        .cloned()
        .unwrap_or_else(|| format!("point#{}", point.0))
}

/// The full universe of time points in a compiled world.
fn universe(world: &World) -> Vec<TimePoint> {
    (0..world.time_points.names().len())
        .filter_map(|index| u32::try_from(index).ok())
        .map(TimePoint)
        .collect()
}

fn run_show(args: &ShowArgs) -> CmdResult {
    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "timeline")?.model.world;
    let points = universe(&world);
    if points.is_empty() {
        outln!("no chronology in this corpus");
        return Ok(EXIT_CLEAN);
    }

    match world.chronology.topological_layers(&points) {
        Ok(layers) => {
            for (rank, layer) in layers.iter().enumerate() {
                let names: Vec<String> = layer.iter().map(|p| point_name(&world, *p)).collect();
                outln!("layer {rank}: {}", names.join(", "));
            }
        }
        Err(blame) => {
            outln!(
                "the chronology has a cycle over {} edge(s):",
                blame.cycle.len()
            );
            for edge in &blame.cycle {
                outln!(
                    "  {} -> {}",
                    point_name(&world, edge.before),
                    point_name(&world, edge.after)
                );
            }
            return Ok(EXIT_FINDINGS);
        }
    }

    if let Some(at) = &args.at {
        let cursor = points
            .iter()
            .copied()
            .find(|p| &point_name(&world, *p) == at)
            .ok_or_else(|| format!("unknown timeline point '{at}'"))?;
        let mut before = Vec::new();
        let mut after = Vec::new();
        for &other in &points {
            if other == cursor {
                continue;
            }
            match world.chronology.precedes(other, cursor) {
                TemporalVerdict::Before => before.push(point_name(&world, other)),
                TemporalVerdict::After => after.push(point_name(&world, other)),
                TemporalVerdict::Unordered => {}
            }
        }
        outln!(
            "at {at}: after [{}], before [{}]",
            before.join(", "),
            after.join(", ")
        );
    }
    Ok(EXIT_CLEAN)
}

fn run_order(args: &CorpusArgs) -> CmdResult {
    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "timeline")?.model.world;

    if let Some(blame) = world.chronology.find_cycle(&[]) {
        outln!(
            "cycle in the chronology over {} edge(s):",
            blame.cycle.len()
        );
        for edge in &blame.cycle {
            outln!(
                "  {} -> {}",
                point_name(&world, edge.before),
                point_name(&world, edge.after)
            );
        }
        return Ok(EXIT_FINDINGS);
    }

    let points = universe(&world);
    let mut unordered = 0usize;
    for (i, &a) in points.iter().enumerate() {
        for &b in &points[i + 1..] {
            if world.chronology.precedes(a, b) == TemporalVerdict::Unordered {
                outln!(
                    "unordered: {} ? {}",
                    point_name(&world, a),
                    point_name(&world, b)
                );
                unordered += 1;
            }
        }
    }
    outln!("chronology acyclic; {unordered} unordered pair(s)");
    Ok(EXIT_CLEAN)
}

/// Read and deserialize an episode payload YAML into a season.
fn load_episodes(path: &PathBuf) -> Result<Vec<Episode>, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    serde_yaml_ng::from_str(&text)
        .map_err(|err| format!("the episode payload {} is malformed: {err}", path.display()))
}

fn run_knows(args: &KnowsArgs) -> CmdResult {
    let episodes = load_episodes(&args.episodes)?;
    let folded = fold(&episodes);
    let knower = match &args.who {
        Some(id) => Knower::cast(id.clone()),
        None => Knower::Listener,
    };
    let facts = folded.knowledge_at(args.at, &knower);
    let who = args
        .who
        .clone()
        .unwrap_or_else(|| "the listener".to_owned());
    if facts.is_empty() {
        outln!("at slot {}, {who} knows nothing", args.at);
        return Ok(EXIT_CLEAN);
    }
    outln!("at slot {}, {who} knows:", args.at);
    for fact in &facts {
        outln!("  {}", fact.0);
    }
    Ok(EXIT_CLEAN)
}

fn run_episodes(args: &EpisodesArgs) -> CmdResult {
    let episodes = load_episodes(&args.episodes)?;
    let folded = fold(&episodes);

    let mut ordered: Vec<&Episode> = episodes.iter().collect();
    ordered.sort_by_key(|ep| ep.production_order);
    for ep in &ordered {
        let anchor = ep
            .story_anchor
            .era
            .as_ref()
            .map_or_else(|| "unanchored".to_owned(), |era| era.0.clone());
        outln!("slot {}: {} @ {anchor}", ep.production_order, ep.id);
    }

    let loops = folded.open_loops();
    for question in &loops.open {
        outln!("open loop never resolved: {}", question.0);
    }
    outln!(
        "{} episode(s); {} loop(s) resolved, {} still open",
        episodes.len(),
        loops.closed.len(),
        loops.open.len()
    );
    if loops.open.is_empty() {
        Ok(EXIT_CLEAN)
    } else {
        Ok(EXIT_FINDINGS)
    }
}

fn run_phases(args: &PhasesArgs) -> CmdResult {
    let episodes = load_episodes(&args.episodes)?;
    let episode = episodes
        .iter()
        .find(|ep| ep.id == args.episode)
        .ok_or_else(|| format!("no episode '{}' in the payload", args.episode))?;

    let mut any = false;
    for phase in &episode.story_anchor.phases {
        outln!("episode anchor: {} = index {}", phase.cycle, phase.index);
        any = true;
    }
    for scene in &episode.scenes {
        if let Some(anchor) = &scene.scene_anchor {
            for phase in &anchor.phases {
                outln!(
                    "scene {} anchor: {} = index {}",
                    scene.id,
                    phase.cycle,
                    phase.index
                );
                any = true;
            }
        }
    }
    if !any {
        outln!("episode {} pins no cyclic phase", args.episode);
    }
    Ok(EXIT_CLEAN)
}
