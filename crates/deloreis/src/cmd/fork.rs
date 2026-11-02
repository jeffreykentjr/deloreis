//! `deloreis fork <op>` — the fork store over a workspace directory.
//!
//! A fork is a named condition context over an unchanged base world: a valuation
//! overlay (`suppose`/`suppress`), fork-local proposed facts (`add`), and an
//! optional keep-stamp (`restrict`). The engine's [`ForkStore`] holds all of
//! that, but it is not itself serializable — its overlay keys on interned atom
//! indices that only mean anything against one compiled world. So the CLI
//! persists the *intent* instead: a [`StoredStore`] under `<workspace>/.dli-cache/`
//! records each fork's suppositions, additions, and keep-stamps by **name**, plus
//! the corpus the base is compiled from. Every op rebuilds a live [`ForkStore`]
//! from that record (replaying the intent through the real engine, which is where
//! validation and typed errors come from), then reads or amends it.
//!
//! Because the base recompiles from the recorded corpus and the overlays replay
//! by name, the store survives a corpus edit: it is a description of what was
//! supposed, not a snapshot of a valuation that would rot the moment an atom is
//! re-interned. `.dli-cache/` is the worker's derived write surface
//! (`10-serving.md` §4) — content-derived, gitignored, never a source of truth.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use dli_core::ann::Ann;
use dli_core::valuation::Valuation;
use dli_eval::fork::{Addition, ForkId, ForkStore, KeepStamp, Visibility};
use dli_eval::world::{GroundAtom, World};
use serde::{Deserialize, Serialize};

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// Arguments for `deloreis fork`.
#[derive(Debug, Args)]
pub(crate) struct ForkArgs {
    /// The workspace directory whose `.dli-cache/` holds the fork store.
    #[arg(
        short = 'w',
        long,
        value_name = "DIR",
        default_value = ".",
        global = true
    )]
    pub(crate) workspace: PathBuf,
    /// The fork operation.
    #[command(subcommand)]
    pub(crate) op: ForkOp,
}

/// The fork operations, matching the `dli.fork.*` op family.
#[derive(Debug, Subcommand)]
pub(crate) enum ForkOp {
    /// Create a new fork over a corpus.
    New(NewArgs),
    /// List the open forks and their base corpus.
    List,
    /// Show a fork's derived-consequence diff against the base.
    Diff(RefArgs),
    /// Suppose an option or event value in a fork.
    Suppose(SupposeArgs),
    /// Add a proposed ground fact to a fork.
    Add(AddArgs),
    /// Retract a supposition from a fork.
    Suppress(TargetArgs),
    /// Restrict a fork's presented consequences with a keep-stamp.
    Restrict(RestrictArgs),
    /// Ratification pre-flight of a fork.
    Preflight(RefArgs),
    /// Emit a ruling-request draft from a fork.
    Promote(RefArgs),
}

/// Arguments for `fork new`.
#[derive(Debug, Args)]
pub(crate) struct NewArgs {
    /// The `.dli` corpus the base world compiles from (required for the first
    /// fork in a workspace; reused for later forks).
    #[arg(short = 'c', long = "corpus", value_name = "PATH")]
    pub(crate) corpus: Vec<PathBuf>,
    /// A name for the fork (seeds the promotion slug).
    #[arg(long, value_name = "NAME")]
    pub(crate) name: Option<String>,
}

/// A fork reference — its name or its numeric id.
#[derive(Debug, Args)]
pub(crate) struct RefArgs {
    /// The fork's name or numeric id.
    #[arg(value_name = "FORK")]
    pub(crate) fork: String,
}

/// Arguments for `fork suppose`.
#[derive(Debug, Args)]
pub(crate) struct SupposeArgs {
    /// The fork's name or numeric id.
    #[arg(value_name = "FORK")]
    pub(crate) fork: String,
    /// The option, hypothesis, or decision variable to suppose.
    #[arg(value_name = "TARGET")]
    pub(crate) target: String,
    /// The value to suppose (`true` rules the option in, `false` rules it out).
    #[arg(value_name = "VALUE", default_value_t = true, action = clap::ArgAction::Set)]
    pub(crate) value: bool,
}

/// Arguments for `fork suppress`.
#[derive(Debug, Args)]
pub(crate) struct TargetArgs {
    /// The fork's name or numeric id.
    #[arg(value_name = "FORK")]
    pub(crate) fork: String,
    /// The supposition target to retract.
    #[arg(value_name = "TARGET")]
    pub(crate) target: String,
}

/// Arguments for `fork add`.
#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    /// The fork's name or numeric id.
    #[arg(value_name = "FORK")]
    pub(crate) fork: String,
    /// The ground atom to add, e.g. `keeps(old-fen, the-salt-ledger)`.
    #[arg(value_name = "ATOM")]
    pub(crate) atom: String,
}

/// A keep-stamp visibility, as a CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum VisibilityArg {
    /// Force the scoped atoms to appear in the fork's view.
    KeptPresent,
    /// Hide the scoped atoms from the view though they remain true.
    KeptAbsent,
    /// Remove the scoped atoms from the view entirely.
    Removed,
}

/// Arguments for `fork restrict`.
#[derive(Debug, Args)]
pub(crate) struct RestrictArgs {
    /// The fork's name or numeric id.
    #[arg(value_name = "FORK")]
    pub(crate) fork: String,
    /// Predicate names or rendered `pred(a, b)` atoms the stamp covers.
    #[arg(long = "scope", value_name = "SCOPE", required = true)]
    pub(crate) scope: Vec<String>,
    /// The visibility applied to the scoped atoms.
    #[arg(long, value_enum, default_value_t = VisibilityArg::KeptAbsent)]
    pub(crate) visibility: VisibilityArg,
}

/// Arguments for the top-level `deloreis preflight <fork>`.
#[derive(Debug, Args)]
pub(crate) struct PreflightArgs {
    /// The workspace directory whose `.dli-cache/` holds the fork store.
    #[arg(short = 'w', long, value_name = "DIR", default_value = ".")]
    pub(crate) workspace: PathBuf,
    /// The fork's name or numeric id.
    #[arg(value_name = "FORK")]
    pub(crate) fork: String,
}

// -- persisted store ---------------------------------------------------------

/// One supposition, recorded by target name so it survives a recompile.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSupposition {
    /// The event-variable name supposed.
    target: String,
    /// The value it is supposed to.
    value: bool,
}

/// One keep-stamp, recorded by scope name and visibility word.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredKeepStamp {
    /// The predicate names or rendered atoms the stamp covers.
    scope: Vec<String>,
    /// The visibility word (`kept-present`, `kept-absent`, `removed`).
    visibility: String,
}

/// One fork's recorded intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredFork {
    /// The fork's stable numeric id (its [`ForkId`] index at creation).
    id: u64,
    /// The fork's name, if it was named.
    name: Option<String>,
    /// The suppositions, in application order.
    suppositions: Vec<StoredSupposition>,
    /// The fork-local proposed facts, rendered as ground atoms.
    additions: Vec<String>,
    /// The keep-stamps applied to the fork's view.
    keep_stamps: Vec<StoredKeepStamp>,
}

/// The persisted fork store for one workspace: the base corpus and every fork's
/// recorded intent.
#[derive(Debug, Default, Serialize, Deserialize)]
struct StoredStore {
    /// The corpus paths the base world compiles from.
    corpus: Vec<String>,
    /// The next fork id to assign.
    next: u64,
    /// The recorded forks, in creation order.
    forks: Vec<StoredFork>,
}

/// The `.dli-cache/forks.yaml` path for a workspace.
fn cache_path(workspace: &Path) -> PathBuf {
    workspace.join(".dli-cache").join("forks.yaml")
}

/// Read the persisted store, or an empty one when none exists yet.
fn load_store(workspace: &Path) -> Result<StoredStore, String> {
    let path = cache_path(workspace);
    if !path.exists() {
        return Ok(StoredStore::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("cannot read fork store {}: {err}", path.display()))?;
    serde_yaml_ng::from_str(&text)
        .map_err(|err| format!("the fork store {} is malformed: {err}", path.display()))
}

/// Write the store back under `.dli-cache/`.
fn save_store(workspace: &Path, store: &StoredStore) -> Result<(), String> {
    let path = cache_path(workspace);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    }
    let text = serde_yaml_ng::to_string(store)
        .map_err(|err| format!("cannot serialize the fork store: {err}"))?;
    fs::write(&path, text).map_err(|err| format!("cannot write {}: {err}", path.display()))
}

/// Compile the store's base world from its recorded corpus.
fn compile_base(store: &StoredStore) -> Result<dli_world::CompiledWorld, String> {
    if store.corpus.is_empty() {
        return Err(
            "this workspace has no fork store — run `deloreis fork new --corpus …` first"
                .to_owned(),
        );
    }
    let paths: Vec<PathBuf> = store.corpus.iter().map(PathBuf::from).collect();
    let files = cmd::load_corpus(&paths)?;
    cmd::compile_corpus(&files, "fork")
}

/// Rebuild a live [`ForkStore`] by replaying every recorded fork's intent, and
/// return the map from stored id to the live [`ForkId`].
fn build(store: &StoredStore) -> Result<(ForkStore, BTreeMap<u64, ForkId>), String> {
    let base = compile_base(store)?;
    build_from_base(store, base.model.world)
}

fn build_from_base(
    store: &StoredStore,
    base: World,
) -> Result<(ForkStore, BTreeMap<u64, ForkId>), String> {
    let mut live = ForkStore::new(base);
    let mut ids: BTreeMap<u64, ForkId> = BTreeMap::new();
    for stored in &store.forks {
        let id = match &stored.name {
            Some(name) => live.fork_named(name.clone()),
            None => live.fork(),
        };
        ids.insert(stored.id, id);
        for supposition in &stored.suppositions {
            live.suppose(id, &supposition.target, supposition.value)
                .map_err(|err| format!("replaying fork {}: {err}", stored.id))?;
        }
        for atom in &stored.additions {
            let ground = ground(atom)?;
            live.add(id, [Addition::asserted(ground, Ann::unconditional())])
                .map_err(|err| format!("replaying fork {}: {err}", stored.id))?;
        }
        for stamp in &stored.keep_stamps {
            let visibility = parse_visibility(&stamp.visibility)?;
            live.restrict(
                id,
                KeepStamp {
                    scope: stamp.scope.clone(),
                    visibility,
                },
            )
            .map_err(|err| format!("replaying fork {}: {err}", stored.id))?;
        }
    }
    Ok((live, ids))
}

/// Ground a `pred(a, b)` atom string, erroring on a non-ground pattern.
fn ground(atom: &str) -> Result<GroundAtom, String> {
    let expr = cmd::parse_goal(atom)?;
    GroundAtom::from_pattern(&expr)
        .ok_or_else(|| format!("'{atom}' has a variable — an added fact must be fully ground"))
}

/// Parse a stored visibility word into the engine [`Visibility`].
fn parse_visibility(word: &str) -> Result<Visibility, String> {
    match word {
        "kept-present" => Ok(Visibility::KeptPresent),
        "kept-absent" => Ok(Visibility::KeptAbsent),
        "removed" => Ok(Visibility::Removed),
        other => Err(format!("unknown keep-stamp visibility '{other}'")),
    }
}

impl VisibilityArg {
    /// The stored word for this visibility.
    fn word(self) -> &'static str {
        match self {
            VisibilityArg::KeptPresent => "kept-present",
            VisibilityArg::KeptAbsent => "kept-absent",
            VisibilityArg::Removed => "removed",
        }
    }
}

/// Resolve a fork reference (name or numeric id) to a stored fork index.
fn stored_index(store: &StoredStore, fork_ref: &str) -> Result<usize, String> {
    if let Ok(id) = fork_ref.parse::<u64>()
        && let Some(index) = store.forks.iter().position(|fork| fork.id == id)
    {
        return Ok(index);
    }
    store
        .forks
        .iter()
        .position(|fork| fork.name.as_deref() == Some(fork_ref))
        .ok_or_else(|| format!("no fork named or numbered '{fork_ref}' in this workspace"))
}

/// Resolve a fork reference to its live [`ForkId`] within a rebuilt store.
fn live_id(
    store: &StoredStore,
    ids: &BTreeMap<u64, ForkId>,
    fork_ref: &str,
) -> Result<ForkId, String> {
    let index = stored_index(store, fork_ref)?;
    let stored_id = store.forks[index].id;
    ids.get(&stored_id)
        .copied()
        .ok_or_else(|| format!("fork '{fork_ref}' was not rebuilt"))
}

/// A short label for a stored fork — its name, else `#id`.
fn label(fork: &StoredFork) -> String {
    fork.name.clone().unwrap_or_else(|| format!("#{}", fork.id))
}

// -- entry points ------------------------------------------------------------

/// Resolve a fork reference against a workspace store into its base world and
/// effective valuation — the seam `query --fork` consumes.
pub(crate) fn resolve(
    workspace: &Path,
    fork_ref: &str,
) -> Result<(dli_world::CompiledWorld, Valuation), String> {
    let store = load_store(workspace)?;
    let base = compile_base(&store)?;
    let (live, ids) = build_from_base(&store, base.model.world.clone())?;
    let id = live_id(&store, &ids, fork_ref)?;
    let valuation = live.valuation(id).map_err(|err| format!("{err}"))?;
    Ok((base, valuation))
}

/// Run a `deloreis fork` subcommand.
pub(crate) fn run(args: &ForkArgs) -> CmdResult {
    match &args.op {
        ForkOp::New(new) => run_new(&args.workspace, new),
        ForkOp::List => run_list(&args.workspace),
        ForkOp::Diff(reference) => run_diff(&args.workspace, &reference.fork),
        ForkOp::Suppose(suppose) => run_suppose(&args.workspace, suppose),
        ForkOp::Add(add) => run_add(&args.workspace, add),
        ForkOp::Suppress(target) => run_suppress(&args.workspace, target),
        ForkOp::Restrict(restrict) => run_restrict(&args.workspace, restrict),
        ForkOp::Preflight(reference) => run_preflight_ref(&args.workspace, &reference.fork),
        ForkOp::Promote(reference) => run_promote(&args.workspace, &reference.fork),
    }
}

/// Run the top-level `deloreis preflight <fork>` alias.
pub(crate) fn run_preflight(args: &PreflightArgs) -> CmdResult {
    run_preflight_ref(&args.workspace, &args.fork)
}

fn run_new(workspace: &Path, args: &NewArgs) -> CmdResult {
    let mut store = load_store(workspace)?;
    if store.corpus.is_empty() {
        if args.corpus.is_empty() {
            return Err("fork new needs --corpus to establish the base world".to_owned());
        }
        store.corpus = args
            .corpus
            .iter()
            .map(|path| path.display().to_string())
            .collect();
    } else if !args.corpus.is_empty() {
        let given: Vec<String> = args
            .corpus
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        if given != store.corpus {
            outln!(
                "note: this workspace's base corpus is already set to [{}]; keeping it",
                store.corpus.join(", ")
            );
        }
    }

    // Compile the base once so a broken corpus fails here, not on the next read.
    let _ = compile_base(&store)?;

    let id = store.next;
    store.next += 1;
    let fork = StoredFork {
        id,
        name: args.name.clone(),
        suppositions: Vec::new(),
        additions: Vec::new(),
        keep_stamps: Vec::new(),
    };
    let name = label(&fork);
    store.forks.push(fork);
    save_store(workspace, &store)?;
    outln!("created fork {name} over [{}]", store.corpus.join(", "));
    Ok(EXIT_CLEAN)
}

fn run_list(workspace: &Path) -> CmdResult {
    let store = load_store(workspace)?;
    if store.forks.is_empty() {
        outln!("no forks in this workspace");
        return Ok(EXIT_CLEAN);
    }
    // Rebuild so a corrupt store surfaces even on a plain list.
    let _ = build(&store)?;
    outln!("base corpus: [{}]", store.corpus.join(", "));
    for fork in &store.forks {
        outln!(
            "{name}: {} supposition(s), {} addition(s), {} keep-stamp(s)",
            fork.suppositions.len(),
            fork.additions.len(),
            fork.keep_stamps.len(),
            name = label(fork),
        );
    }
    Ok(EXIT_CLEAN)
}

fn run_diff(workspace: &Path, fork_ref: &str) -> CmdResult {
    let store = load_store(workspace)?;
    let (live, ids) = build(&store)?;
    let id = live_id(&store, &ids, fork_ref)?;
    let delta = live.diff(id).map_err(|err| format!("{err}"))?;

    render_changes("newly certain", &delta.newly_certain);
    render_changes("newly possible", &delta.newly_possible);
    render_changes("newly dead", &delta.newly_dead);
    render_changes("grade changed", &delta.grade_changed);
    for conflict in &delta.conflicts {
        outln!(
            "conflict [{}]  {}",
            conflict.kind.label(),
            conflict.rendered
        );
    }

    let changed = delta.newly_certain.len()
        + delta.newly_possible.len()
        + delta.newly_dead.len()
        + delta.grade_changed.len();
    outln!(
        "fork {fork_ref}: {changed} change(s), {} conflict(s)",
        delta.conflicts.len()
    );
    if delta.conflicts.is_empty() {
        Ok(EXIT_CLEAN)
    } else {
        Ok(EXIT_FINDINGS)
    }
}

fn run_suppose(workspace: &Path, args: &SupposeArgs) -> CmdResult {
    let mut store = load_store(workspace)?;
    let (mut live, ids) = build(&store)?;
    let id = live_id(&store, &ids, &args.fork)?;
    live.suppose(id, &args.target, args.value)
        .map_err(|err| format!("{err}"))?;

    let index = stored_index(&store, &args.fork)?;
    let suppositions = &mut store.forks[index].suppositions;
    if let Some(existing) = suppositions.iter_mut().find(|s| s.target == args.target) {
        existing.value = args.value;
    } else {
        suppositions.push(StoredSupposition {
            target: args.target.clone(),
            value: args.value,
        });
    }
    save_store(workspace, &store)?;
    outln!(
        "supposed {} = {} in fork {}",
        args.target,
        args.value,
        args.fork
    );
    Ok(EXIT_CLEAN)
}

fn run_add(workspace: &Path, args: &AddArgs) -> CmdResult {
    let mut store = load_store(workspace)?;
    let (mut live, ids) = build(&store)?;
    let id = live_id(&store, &ids, &args.fork)?;
    let atom = ground(&args.atom)?;
    live.add(id, [Addition::asserted(atom.clone(), Ann::unconditional())])
        .map_err(|err| format!("{err}"))?;

    let index = stored_index(&store, &args.fork)?;
    store.forks[index].additions.push(atom.render());
    save_store(workspace, &store)?;
    outln!("added {} to fork {}", atom.render(), args.fork);
    Ok(EXIT_CLEAN)
}

fn run_suppress(workspace: &Path, args: &TargetArgs) -> CmdResult {
    let mut store = load_store(workspace)?;
    let (mut live, ids) = build(&store)?;
    let id = live_id(&store, &ids, &args.fork)?;
    live.suppress(id, &args.target)
        .map_err(|err| format!("{err}"))?;

    let index = stored_index(&store, &args.fork)?;
    store.forks[index]
        .suppositions
        .retain(|s| s.target != args.target);
    save_store(workspace, &store)?;
    outln!("suppressed {} in fork {}", args.target, args.fork);
    Ok(EXIT_CLEAN)
}

fn run_restrict(workspace: &Path, args: &RestrictArgs) -> CmdResult {
    let mut store = load_store(workspace)?;
    let (mut live, ids) = build(&store)?;
    let id = live_id(&store, &ids, &args.fork)?;
    live.restrict(
        id,
        KeepStamp {
            scope: args.scope.clone(),
            visibility: match args.visibility {
                VisibilityArg::KeptPresent => Visibility::KeptPresent,
                VisibilityArg::KeptAbsent => Visibility::KeptAbsent,
                VisibilityArg::Removed => Visibility::Removed,
            },
        },
    )
    .map_err(|err| format!("{err}"))?;

    let index = stored_index(&store, &args.fork)?;
    store.forks[index].keep_stamps.push(StoredKeepStamp {
        scope: args.scope.clone(),
        visibility: args.visibility.word().to_owned(),
    });
    save_store(workspace, &store)?;
    outln!(
        "restricted [{}] to {} in fork {}",
        args.scope.join(", "),
        args.visibility.word(),
        args.fork
    );
    Ok(EXIT_CLEAN)
}

fn run_preflight_ref(workspace: &Path, fork_ref: &str) -> CmdResult {
    let store = load_store(workspace)?;
    let (live, ids) = build(&store)?;
    let id = live_id(&store, &ids, fork_ref)?;
    let preflight = live.preflight(id).map_err(|err| format!("{err}"))?;

    outln!("affected atoms: {}", preflight.affected.len());
    for atom in &preflight.affected {
        outln!("  {}", atom.render());
    }
    for conflict in &preflight.conflicts {
        outln!(
            "conflict [{}]  {}",
            conflict.kind.label(),
            conflict.rendered
        );
    }
    if preflight.conflicts.is_empty() {
        outln!("pre-flight clean — ratifying fork {fork_ref} introduces no conflict");
        Ok(EXIT_CLEAN)
    } else {
        outln!(
            "pre-flight found {} conflict(s) in fork {fork_ref}",
            preflight.conflicts.len()
        );
        Ok(EXIT_FINDINGS)
    }
}

fn run_promote(workspace: &Path, fork_ref: &str) -> CmdResult {
    let store = load_store(workspace)?;
    let (live, ids) = build(&store)?;
    let id = live_id(&store, &ids, fork_ref)?;
    let draft = live.promote(id).map_err(|err| format!("{err}"))?;

    outln!("rq-draft slug: {}", draft.slug);
    outln!("ask: {}", draft.ask);
    outln!("affects:");
    for affected in &draft.affects {
        outln!("  {affected}");
    }
    outln!("diff:");
    for line in &draft.diff {
        outln!("  {line}");
    }
    outln!("evidence:");
    for line in &draft.evidence {
        outln!("  {line}");
    }
    Ok(EXIT_CLEAN)
}

/// Print a labelled group of atom changes, skipping an empty group.
fn render_changes(label: &str, changes: &[dli_eval::fork::AtomChange]) {
    if changes.is_empty() {
        return;
    }
    outln!("{label}:");
    for change in changes {
        outln!("  {}", change.rendered);
    }
}
