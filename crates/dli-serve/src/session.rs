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
//! The corpus resolver is deliberately small at v0: the built-in `camelot`
//! corpus (the crate's neutral demo world, `dli_world::loader::example_docs`)
//! resolves hermetically, and any other id is [`ErrorCode::UnknownDoc`]. Pointing
//! the resolver at a corpus directory — parse each `<id>.dli` through
//! `dli_syntax::parse_document` and compile it — is the single seam that opens the
//! surface to arbitrary corpora; the cache, handle registry, and every handler
//! above are already corpus-agnostic.

use std::collections::BTreeMap;

use dli_eval::fork::{ForkId, ForkStore};
use dli_export::profile::Registry;
use dli_game::model::Ruleset;
use dli_syntax::parse_document;
use dli_time::cycle::CyclePos;
use dli_world::episode::{Episode, EpisodeAnchor, FactDelta, KnowledgeDelta, QuestionRef, Scene};
use dli_world::language::LanguageModel;
use dli_world::loader::example_docs;
use dli_world::{CompiledWorld, Pack, compile};

use crate::envelope::ErrorCode;

/// The built-in `french-deck` game ruleset — the shipped standard 52-card
/// exemplar (`examples/games/french-deck/`). It gives the card / Ludii export
/// profiles a hermetic, resolvable game to render (a lore world carries no card
/// ruleset). Embedded as source (never a file read at runtime) so the worker
/// stays offline and self-contained.
const FRENCH_DECK_RULES: &str = include_str!("../../../examples/games/french-deck/rules.json");
/// The `french-deck` export profile catalog (a `cards` and a `ludii-gdl` profile).
const FRENCH_DECK_PROFILES: &str =
    include_str!("../../../examples/games/french-deck/profiles.json");

/// The `round-table` teaching corpus: the neutral, wholly public-domain Matter of
/// Britain world that ships with dli (`examples/camelot.dli`). It carries the one
/// shape the `camelot` in-Rust builtin lacks — a full chronology (an era spine,
/// events with a `before`-fact and a deliberately UNORDERED pair) plus a language
/// dialect — so the `timeline.show`/`timeline.order` and dialect-scoped
/// `name.check` reads have a resolvable, engine-owned world to run against. It is
/// the neutral counterpart to any one production world; the worker embeds no
/// production canon (the dli/TPtH separation invariant). Embedded as source (never
/// a file read at runtime) so the worker stays offline and self-contained, exactly
/// like the `french-deck` game rules above.
const ROUND_TABLE_SRC: &str = include_str!("../../../examples/camelot.dli");

/// The built-in `warding-vows` teaching corpus: a linear-conserved proof
/// discipline with one worked casting that checks. It gives the `check-proof` and
/// `simulate.*` ops a hermetic, resolvable document that carries both a practice
/// and a proof — the neutral `camelot` demo world carries neither. Embedded as
/// source (never a file read) so the worker stays offline and self-contained.
const WARDING_VOWS_SRC: &str = r#"scope warding-vows

use chronology

ratify d-1

cycle tide(4) = ebb | slack | flood | high  decided d-1

warded(arden)          decided d-1
phase(tide, slack)     decided d-1

practice warding-vows  proposed {
  kind ground = door | field | ore
  resource debt : linear conserved
  rule ward-form(ground, fibre)
  rule give-and-take
  error fizzle  refines L2 premise-not-available where absent
  error rebound refines L2 premise-not-available where contradicted
  cost material
  }

proof e01-s04  speaker arden  in warding-vows {
  premise p1  warded(arden)        cited d-1  cue "line 112"
  premise p2  phase(tide, slack)   from-scene cue "line 118"
  premise p3  debt(1)              drawn-against p1
  step    s1  instantiate ward-form  with ground = door, fibre = withy
  step    s2  chain from p1, s1 by give-and-take conclusion bound(ward, door)
  spend   p3
  close   bound(ward, door)  expect holds
  }
"#;

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

/// A completed simulation job held for the life of the serve loop. The worker
/// runs a battery synchronously on `simulate.start` (bounded, replayable), so a
/// job is `complete` the instant it is registered; `simulate.status` reads it back
/// and `simulate.cancel` drops it. The manifest is kept pre-serialized so this
/// module stays free of the simulator types (`10-serving.md` §5: jobs are
/// in-process, never persisted).
#[derive(Debug, Clone)]
pub struct SimJob {
    /// The job status (`complete` for a finished synchronous run).
    pub status: String,
    /// One rendered line per simulated turn.
    pub trace: Vec<String>,
    /// The committed run manifest, already serialized to JSON.
    pub manifest: serde_json::Value,
}

/// The session store shared across one serve process: the doc-id→scope cache and
/// the global fork-handle registry. One `Sessions` lives for the life of the
/// `serve` loop; nothing here is persisted (the worker holds no cross-process job
/// state — `10-serving.md` §5).
#[derive(Debug, Default)]
pub struct Sessions {
    scopes: BTreeMap<String, Scope>,
    /// Wire fork handle → the scope and fork it names.
    fork_handles: BTreeMap<String, (String, ForkId)>,
    /// Monotonic handle counter, so handles (`fork-1`, `fork-2`, …) are stable
    /// and never reused within a process.
    next_handle: u64,
    /// Wire job handle → the completed simulation job it names.
    jobs: BTreeMap<String, SimJob>,
    /// Monotonic job counter, so handles (`job-1`, `job-2`, …) are stable and
    /// never reused within a process.
    next_job: u64,
}

impl Sessions {
    /// An empty session store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve a document id to its compiled scope, compiling and caching on first
    /// use. Only the built-in `camelot` corpus is wired at v0; every other id is
    /// [`ErrorCode::UnknownDoc`] until the resolver is pointed at a corpus root.
    ///
    /// A compile failure maps through the frozen [`ErrorCode`] conversion — an
    /// unstratifiable or contradictory corpus surfaces as its typed error, never a
    /// panic.
    pub fn scope(&mut self, document: &str) -> Result<&mut Scope, ErrorCode> {
        if !self.scopes.contains_key(document) {
            let pack = builtin_pack(document).ok_or(ErrorCode::UnknownDoc)?;
            let world = compile(&pack).map_err(ErrorCode::from)?;
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

    /// Register a completed simulation job, minting a fresh stable wire handle.
    pub fn register_job(&mut self, job: SimJob) -> String {
        self.next_job += 1;
        let handle = format!("job-{}", self.next_job);
        self.jobs.insert(handle.clone(), job);
        handle
    }

    /// Resolve a wire job handle to the completed job it names.
    #[must_use]
    pub fn job(&self, handle: &str) -> Option<&SimJob> {
        self.jobs.get(handle)
    }

    /// Drop a job by handle, returning whether it was present — the
    /// `simulate.cancel` seam over a synchronous, already-complete run.
    pub fn cancel_job(&mut self, handle: &str) -> bool {
        self.jobs.remove(handle).is_some()
    }
}

/// The built-in corpus pack for a doc-id, or `None` if the id is unknown. The v0
/// resolver knows only `camelot` (the crate's neutral demo world); pointing it at
/// a corpus directory generalizes exactly this one function. It is the single
/// source both the read surface ([`Sessions::scope`]) and the Door T store
/// (`serve`'s `with_pack` construction) resolve a corpus through.
#[must_use]
pub fn builtin_pack(document: &str) -> Option<Pack> {
    match document {
        "camelot" => Some(Pack {
            name: document.to_owned(),
            documents: example_docs(),
        }),
        "warding-vows" => {
            let (doc, _diags) = parse_document(WARDING_VOWS_SRC);
            Some(Pack {
                name: document.to_owned(),
                documents: vec![doc],
            })
        }
        "round-table" => {
            let (doc, _diags) = parse_document(ROUND_TABLE_SRC);
            Some(Pack {
                name: document.to_owned(),
                documents: vec![doc],
            })
        }
        _ => None,
    }
}

/// The built-in episode season for a doc-id, or `None` if the id names no season.
///
/// Episodes are serde payloads until their `.dli` surface lands (TIME-3), and the
/// compiled world carries none — so the worker resolves a season the same way it
/// resolves a corpus: by id, from an engine-owned neutral fixture. The
/// `round-table` season is a three-episode fold over the `ROUND_TABLE_SRC`
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
/// `ROUND_TABLE_SRC` world (its `old-speech` dialect and name-parts). The
/// `name.check` op carries no corpus reference, so a dialect-scoped vet resolves
/// its register here, against the one engine-owned neutral corpus that declares a
/// dialect. An empty dialect keeps the register-free default behavior.
#[must_use]
pub fn demo_language_model() -> LanguageModel {
    let (doc, _diags) = parse_document(ROUND_TABLE_SRC);
    LanguageModel::from_documents(&[doc]).model
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
