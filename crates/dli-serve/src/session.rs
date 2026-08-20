//! Serve sessions (`SERVE-1`): doc-id to loaded-scope state, and the fork registry.
//!
//! A [`Sessions`] resolves a wire `document` id to a compiled [`Scope`] exactly
//! once and caches it — the in-memory shape of the derived-world cache
//! (`docs/spec/10-serving.md` §4): a second request for the same corpus reuses
//! the compiled world rather than recompiling. Forks are session state:
//! `dli.fork.new` mints a stable wire handle bound to `(document, ForkId)`, and
//! the later fork ops resolve that handle back to the owning scope's
//! [`ForkStore`].
//!
//! Production sessions resolve only logical ids declared by a versioned
//! [`Project`]. The embedded neutral corpora remain available solely through the
//! explicit demo constructor used by fixtures and examples. Neither source can
//! fall back to the other.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use dli_eval::fork::{ForkId, ForkStore};
use dli_export::profile::Registry;
use dli_export::source_fingerprint;
use dli_game::model::Ruleset;
use dli_time::cycle::CyclePos;
use dli_world::episode::{Episode, EpisodeAnchor, FactDelta, KnowledgeDelta, QuestionRef, Scene};
use dli_world::language::LanguageModel;
use dli_world::{CompiledWorld, Pack, compile};

use crate::demo;
use crate::envelope::ErrorCode;
use crate::project::Project;

/// Maximum simulation workers one serve process runs concurrently.
const MAX_SIMULATION_WORKERS: usize = 4;

/// The built-in `french-deck` game ruleset — the shipped standard 52-card
/// exemplar (`examples/games/french-deck/`). It gives the card / Ludii export
/// profiles a hermetic, resolvable game to render (a lore world carries no card
/// ruleset). Embedded as source (never a file read at runtime) so the worker
/// stays offline and self-contained.
const FRENCH_DECK_RULES: &str = include_str!("../../../examples/games/french-deck/rules.json");
/// The `french-deck` export profile catalog (a `cards` and a `ludii-gdl` profile).
const FRENCH_DECK_PROFILES: &str =
    include_str!("../../../examples/games/french-deck/profiles.json");

/// A loaded corpus scope: the compiled world every read op queries, and the fork
/// store every fork op layers over its saturated base.
#[derive(Debug)]
pub struct Scope {
    /// The compiled, evaluable world.
    pub world: CompiledWorld,
    /// The forks opened over this scope. Its base is a clone of the compiled
    /// world's saturated program, so a fork never disturbs the read surface.
    pub forks: ForkStore,
}

/// The session store shared across one serve process: the doc-id→scope cache and
/// the global fork-handle registry. One `Sessions` lives for the life of the
/// `serve` loop. Session scopes and forks are process-local; simulation jobs are
/// durable project evidence, with this store retaining only post-response worker
/// scheduling state.
#[derive(Debug)]
pub struct Sessions {
    source: CorpusSource,
    scopes: BTreeMap<String, Scope>,
    /// Wire fork handle → the scope and fork it names.
    fork_handles: BTreeMap<String, (String, ForkId)>,
    /// Monotonic handle counter, so handles (`fork-1`, `fork-2`, …) are stable
    /// and never reused within a process.
    next_handle: u64,
    /// Jobs to start only after the response that exposed their durable state has
    /// been flushed.
    queued_jobs: BTreeSet<String>,
    /// Jobs currently owned by a background worker in this process.
    active_jobs: Arc<Mutex<BTreeSet<String>>>,
}

#[derive(Debug)]
enum CorpusSource {
    Demo,
    Project(Arc<Project>),
}

impl Default for Sessions {
    fn default() -> Self {
        Self::new()
    }
}

impl Sessions {
    /// An empty session store over the explicit bundled demo corpus.
    ///
    /// Production entry points use [`from_project`](Self::from_project);
    /// `new` remains the fixture/test constructor for the contract's demo ids.
    #[must_use]
    pub fn new() -> Self {
        Self {
            source: CorpusSource::Demo,
            scopes: BTreeMap::new(),
            fork_handles: BTreeMap::new(),
            next_handle: 0,
            queued_jobs: BTreeSet::new(),
            active_jobs: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    /// A session store resolving only assets declared by `project`.
    #[must_use]
    pub fn from_project(project: Arc<Project>) -> Self {
        Self {
            source: CorpusSource::Project(project),
            scopes: BTreeMap::new(),
            fork_handles: BTreeMap::new(),
            next_handle: 0,
            queued_jobs: BTreeSet::new(),
            active_jobs: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    /// Resolve a document id to its compiled scope, compiling and caching on first
    /// use. Project sessions accept only manifest-declared logical ids; demo
    /// sessions accept only their embedded fixture ids.
    ///
    /// A compile failure maps through the frozen [`ErrorCode`] conversion — an
    /// unstratifiable or contradictory corpus surfaces as its typed error, never a
    /// panic.
    pub fn scope(&mut self, document: &str) -> Result<&mut Scope, ErrorCode> {
        if !self.scopes.contains_key(document) {
            let pack = self.pack(document).ok_or(ErrorCode::UnknownDoc)?;
            let mut world = compile(&pack).map_err(ErrorCode::from)?;
            if let CorpusSource::Project(project) = &self.source
                && let Some(loaded) = project.document(document)
            {
                world.killed = loaded.killed.clone();
                let target_interner = world.model.interner().clone();
                loaded
                    .store_valuation
                    .apply_to(&target_interner, &mut world.model.world.valuation)
                    .map_err(|_| ErrorCode::EvidenceCorrupt)?;
            }
            let forks = ForkStore::new(world.model.world.clone());
            self.scopes
                .insert(document.to_owned(), Scope { world, forks });
        }
        Ok(self
            .scopes
            .get_mut(document)
            .expect("scope was just resolved into the cache"))
    }

    /// Mint a fresh, stable wire handle bound to a fork within a document.
    pub fn register_fork(&mut self, document: &str, fork: ForkId) -> String {
        self.next_handle += 1;
        let handle = format!("fork-{}", self.next_handle);
        self.fork_handles
            .insert(handle.clone(), (document.to_owned(), fork));
        handle
    }

    /// Resolve a wire fork handle to the document and fork it names.
    #[must_use]
    pub fn fork_handle(&self, handle: &str) -> Option<(String, ForkId)> {
        self.fork_handles.get(handle).cloned()
    }

    /// The fork handles registered for a document, in handle order.
    #[must_use]
    pub fn forks_of(&self, document: &str) -> Vec<String> {
        self.fork_handles
            .iter()
            .filter(|(_, (doc, _))| doc == document)
            .map(|(handle, _)| handle.clone())
            .collect()
    }

    /// The wire handles and engine ids registered for one document.
    #[must_use]
    pub fn fork_records(&self, document: &str) -> Vec<(String, ForkId)> {
        self.fork_handles
            .iter()
            .filter(|(_, (doc, _))| doc == document)
            .map(|(handle, (_, id))| (handle.clone(), *id))
            .collect()
    }

    /// Queue a durable job for background advancement after the current response
    /// is visible to the caller.
    pub fn queue_simulation_worker(&mut self, job_id: String) {
        if matches!(&self.source, CorpusSource::Project(_)) {
            self.queued_jobs.insert(job_id);
        }
    }

    /// Start every queued simulation worker.
    ///
    /// The serve and MCP loops call this only after flushing their response. A
    /// direct dispatch test therefore observes deterministic `pending` state,
    /// while a real transport never performs the long-running work before
    /// acknowledging `simulate.start`.
    pub fn release_simulation_workers(&mut self) {
        let CorpusSource::Project(project) = &self.source else {
            self.queued_jobs.clear();
            return;
        };
        let queued = std::mem::take(&mut self.queued_jobs);
        for job_id in queued {
            let mut active = self
                .active_jobs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if active.len() >= MAX_SIMULATION_WORKERS {
                drop(active);
                self.queued_jobs.insert(job_id);
                continue;
            }
            if !active.insert(job_id.clone()) {
                continue;
            }
            drop(active);

            let project = Arc::clone(project);
            let active_jobs = Arc::clone(&self.active_jobs);
            let worker_id = job_id.clone();
            let spawn = std::thread::Builder::new()
                .name(format!("dli-sim-{}", &job_id[7..15]))
                .spawn(move || {
                    let _ = crate::simulation::run_to_terminal(&project, &worker_id);
                    active_jobs
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .remove(&worker_id);
                });
            if spawn.is_err() {
                self.active_jobs
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&job_id);
                self.queued_jobs.insert(job_id);
            }
        }
    }

    /// Run every queued simulation worker to completion synchronously, inline on
    /// the caller's thread, returning only once each queued job is terminal.
    ///
    /// This is the deterministic, clock-free drain (DL-0013): a durable job's
    /// terminal state is a pure function of its committed inputs, so a test or
    /// an offline tool can assert on completion with nothing to poll. Production
    /// transports use the threaded [`Sessions::release_simulation_workers`];
    /// this is the offline path.
    pub fn drain_simulation_workers(&mut self) {
        let CorpusSource::Project(project) = &self.source else {
            self.queued_jobs.clear();
            return;
        };
        let project = Arc::clone(project);
        let queued = std::mem::take(&mut self.queued_jobs);
        for job_id in queued {
            let _ = crate::simulation::run_to_terminal(&project, &job_id);
        }
    }

    /// Resolve and clone a logical document's source pack.
    #[must_use]
    pub fn pack(&self, document: &str) -> Option<Pack> {
        match &self.source {
            CorpusSource::Demo => builtin_pack(document),
            CorpusSource::Project(project) => {
                project.document(document).map(|loaded| loaded.pack.clone())
            }
        }
    }

    /// Resolve and clone a logical document's episode season.
    #[must_use]
    pub fn episodes(&self, document: &str) -> Option<Vec<Episode>> {
        match &self.source {
            CorpusSource::Demo => builtin_episodes(document),
            CorpusSource::Project(project) => project
                .document(document)
                .and_then(|loaded| loaded.episodes.clone()),
        }
    }

    /// Resolve and clone a game plus its profile catalog.
    #[must_use]
    pub fn ruleset(&self, document: &str) -> Option<(Ruleset, Registry)> {
        match &self.source {
            CorpusSource::Demo => builtin_ruleset(document),
            CorpusSource::Project(project) => project
                .game(document)
                .map(|game| (game.ruleset.clone(), game.profiles.clone())),
        }
    }

    /// Resolve a lore document's export profile catalog.
    #[must_use]
    pub fn export_registry(&self, document: &str) -> Option<Registry> {
        match &self.source {
            CorpusSource::Demo => demo::is_demo_document(document).then(Registry::builtin),
            CorpusSource::Project(project) => project
                .document(document)
                .map(|loaded| loaded.profiles.clone()),
        }
    }

    /// Deterministic source anchor for an export manifest.
    #[must_use]
    pub fn export_source(&self, document: &str) -> Option<String> {
        match &self.source {
            CorpusSource::Demo => demo_source_fingerprint(document),
            CorpusSource::Project(project) => project
                .document(document)
                .map(|loaded| loaded.source_digest.clone())
                .or_else(|| {
                    project
                        .game(document)
                        .map(|game| game.source_digest.clone())
                }),
        }
    }

    /// The language model visible to the current name-check wire operation.
    #[must_use]
    pub fn language_model(&self) -> LanguageModel {
        match &self.source {
            CorpusSource::Demo => demo_language_model(),
            CorpusSource::Project(project) => project.language().clone(),
        }
    }

    /// The caller project backing this session, or `None` in explicit demo mode.
    #[must_use]
    pub fn project(&self) -> Option<&Project> {
        match &self.source {
            CorpusSource::Demo => None,
            CorpusSource::Project(project) => Some(project),
        }
    }

    /// Clone the caller-project handle for an operation that also mutably
    /// resolves a session scope.
    #[must_use]
    pub fn project_handle(&self) -> Option<Arc<Project>> {
        match &self.source {
            CorpusSource::Demo => None,
            CorpusSource::Project(project) => Some(Arc::clone(project)),
        }
    }
}

/// The deterministic source anchor an export manifest records as its `commit`.
///
/// It is taken over a demo's **committed source** — the bytes in the tree, not
/// the stored facts it lowers to — so the anchor stays the frozen, wire-visible
/// value the contract fixtures pin (see [`crate::demo::source_bytes`]).
fn demo_source_fingerprint(document: &str) -> Option<String> {
    if let Some(source) = demo::source_bytes(document) {
        return Some(source_fingerprint([source.as_ref()]));
    }
    match document {
        "french-deck" => Some(source_fingerprint([FRENCH_DECK_RULES.as_bytes()])),
        _ => None,
    }
}

/// The built-in corpus pack for a doc-id, or `None` if the id is unknown.
///
/// The pack is reconstructed from the demo's **committed fact objects**
/// ([`crate::demo`]) — the same read path any lorestore-backed corpus takes — so
/// no `.dli` text is parsed to serve a demo (roadmap §LS-7). It is the single
/// source both the read surface ([`Sessions::scope`]) and the authoring seam's
/// store (`serve`'s `with_pack` construction) resolve a demo corpus through.
#[must_use]
pub fn builtin_pack(document: &str) -> Option<Pack> {
    demo::pack(document)
}

/// The built-in episode season for a doc-id, or `None` if the id names no season.
///
/// Episodes are serde payloads until their `.dli` surface lands (TIME-3), and the
/// compiled world carries none — so the worker resolves a season the same way it
/// resolves a corpus: by id, from an engine-owned neutral fixture. The
/// `round-table` season is a three-episode fold over the `round-table`
/// world (production order, story anchors, cyclic phases, a knowledge ledger, and
/// one open loop opened in the première and closed in the finale) — enough for the
/// `timeline.knows`/`timeline.episodes`/`timeline.phases` reads to project real
/// output. Wholly public-domain vocabulary; the worker embeds no production canon.
#[must_use]
pub fn builtin_episodes(document: &str) -> Option<Vec<Episode>> {
    match document {
        "round-table" => Some(round_table_season()),
        _ => None,
    }
}

/// The worker's demo language model — the compiled language layer of the
/// `round-table` world, reconstructed from its committed fact objects. See
/// [`crate::demo::language_model`].
#[must_use]
pub fn demo_language_model() -> LanguageModel {
    demo::language_model()
}

/// A three-episode season over the `round-table` world, built in Rust (the
/// episode payload's typed form). Production order is the listener's dimension;
/// each story anchor pins an era and a cyclic phase (`season`); the open loop
/// `who-holds-excalibur` opens in the première and closes in the finale, so the
/// season folds with zero loops left open.
fn round_table_season() -> Vec<Episode> {
    let phase = |index: u32| CyclePos {
        cycle: "season".to_owned(),
        index,
    };
    vec![
        Episode {
            id: "e01".to_owned(),
            production_order: 1,
            story_anchor: EpisodeAnchor::era("uthers-time").with_phase(phase(0)),
            scenes: vec![Scene {
                id: "s1".to_owned(),
                knowledge_deltas: vec![KnowledgeDelta::listener_learns("the-sword-drawn")],
                fact_deltas: vec![FactDelta::of("the-sword-drawn")],
                opens: vec![QuestionRef("who-holds-excalibur".to_owned())],
                ..Scene::default()
            }],
        },
        Episode {
            id: "e02".to_owned(),
            production_order: 2,
            story_anchor: EpisodeAnchor::era("arthurs-reign").with_phase(phase(1)),
            scenes: vec![Scene {
                id: "s1".to_owned(),
                knowledge_deltas: vec![
                    KnowledgeDelta::listener_learns("the-round-table-founded"),
                    KnowledgeDelta::cast_learns("lancelot", "the-sword-drawn"),
                ],
                fact_deltas: vec![FactDelta::of("the-round-table-founded")],
                ..Scene::default()
            }],
        },
        Episode {
            id: "e03".to_owned(),
            production_order: 3,
            story_anchor: EpisodeAnchor::era("after-camlann").with_phase(phase(3)),
            scenes: vec![Scene {
                id: "s1".to_owned(),
                knowledge_deltas: vec![KnowledgeDelta::listener_learns("the-battle-of-camlann")],
                closes: vec![QuestionRef("who-holds-excalibur".to_owned())],
                ..Scene::default()
            }],
        },
    ]
}

/// The built-in card ruleset for a game-document id, paired with its export
/// profile registry (the built-in catalog merged with the game's own profiles) —
/// the source the `Cards` / `LudiiGdl` export profiles render. `None` for a doc-id
/// that names no built-in game (a lore corpus carries no ruleset). The v0 resolver
/// knows only `french-deck`; pointing it at a games directory generalizes exactly
/// this one function, mirroring [`builtin_pack`] for lore.
#[must_use]
pub fn builtin_ruleset(document: &str) -> Option<(Ruleset, Registry)> {
    match document {
        "french-deck" => {
            let ruleset = Ruleset::from_rules(FRENCH_DECK_RULES).ok()?;
            let world: Registry = serde_json::from_str(FRENCH_DECK_PROFILES).ok()?;
            Some((ruleset, Registry::builtin().merge(world)))
        }
        _ => None,
    }
}
