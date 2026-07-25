//! Door T store (`AUTH-1`): each workspace bound to one loaded document base.
//!
//! A project store owns one compiled base per logical document. `author.open`
//! resolves a scope or declared fork to exactly one of those bases and records
//! that ownership for every subsequent read and mutation. The fallback base
//! exists only for focused unit tests and explicit demo fixtures.

use std::collections::BTreeMap;

use dli_ast::decl::{Decl, Document};
use dli_eval::error::EvalError;
use dli_world::pack::Pack;
use dli_world::{CompiledWorld, compile};
use thiserror::Error;

use super::workspace::{Mode, Workspace};

/// The workspace store: loaded document bases plus every open workspace.
///
/// Each base keeps both its **compiled** world (what mutation-time findings check
/// against) and its **source documents**, so a read tool can
/// recompile the staged world — the base documents plus the workspace's staged
/// declarations — through the one real [`compile`] loader (spec 11: the reads
/// evaluate `base + staged`). A store built from only a compiled base
/// ([`with_base`](Store::with_base)) carries no source, so its reads see the
/// staged declarations alone; [`with_pack`](Store::with_pack) is the explicit
/// demo constructor and [`with_packs`](Store::with_packs) is the strict project
/// constructor.
#[derive(Debug)]
pub struct Store {
    fallback: Option<Base>,
    bases: BTreeMap<String, Base>,
    fork_bases: BTreeMap<String, String>,
    strict_forks: bool,
    workspaces: BTreeMap<String, Workspace>,
    workspace_bases: BTreeMap<String, String>,
    next_id: u32,
}

#[derive(Debug)]
struct Base {
    world: CompiledWorld,
    documents: Vec<Document>,
}

/// A caller-project store cannot be constructed unambiguously.
#[derive(Debug, Error)]
pub enum StoreError {
    /// One project document failed to compile.
    #[error(transparent)]
    Compile(#[from] EvalError),
    /// Two supplied packs used the same logical document id.
    #[error("project document `{0}` was supplied more than once")]
    DuplicateDocument(String),
    /// A fork question was declared by more than one project document.
    #[error("fork `{fork}` is declared by both project documents `{first}` and `{second}`")]
    DuplicateFork {
        /// The duplicated fork question id.
        fork: String,
        /// The first owning document.
        first: String,
        /// The second owning document.
        second: String,
    },
}

impl Default for Store {
    fn default() -> Self {
        Self {
            fallback: Some(Base {
                world: CompiledWorld::default(),
                documents: Vec::new(),
            }),
            bases: BTreeMap::new(),
            fork_bases: BTreeMap::new(),
            strict_forks: false,
            workspaces: BTreeMap::new(),
            workspace_bases: BTreeMap::new(),
            next_id: 0,
        }
    }
}

impl Store {
    /// A test store over a given compiled base. The base carries no
    /// re-compilable source, so read tools
    /// over this store evaluate the staged declarations alone; prefer
    /// [`with_pack`](Store::with_pack) when the source pack is available.
    #[must_use]
    pub fn with_base(base: CompiledWorld) -> Self {
        Self {
            fallback: Some(Base {
                world: base,
                documents: Vec::new(),
            }),
            bases: BTreeMap::new(),
            fork_bases: BTreeMap::new(),
            strict_forks: false,
            workspaces: BTreeMap::new(),
            workspace_bases: BTreeMap::new(),
            next_id: 0,
        }
    }

    /// A store over a source pack: compile it for the mutation-time gate **and**
    /// keep its documents, so the read tools recompile `base + staged` through the
    /// real loader. This is the read-complete constructor.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] if the pack does not compile (an unstratifiable
    /// program); malformed claims are never errors — they compile to fail-closed
    /// findings.
    pub fn with_pack(pack: Pack) -> Result<Self, EvalError> {
        let base = compile(&pack)?;
        let fork_bases = fork_owners(&pack.name, &pack.documents)
            .into_iter()
            .collect();
        let mut bases = BTreeMap::new();
        bases.insert(
            pack.name.clone(),
            Base {
                world: base,
                documents: pack.documents,
            },
        );
        Ok(Self {
            fallback: None,
            bases,
            fork_bases,
            strict_forks: false,
            workspaces: BTreeMap::new(),
            workspace_bases: BTreeMap::new(),
            next_id: 0,
        })
    }

    /// A strict multi-document store for a caller project.
    ///
    /// Duplicate logical ids and fork questions are rejected because either
    /// would make `author.open` ownership depend on insertion order.
    pub fn with_packs(
        packs: impl IntoIterator<Item = (String, Pack, dli_world::KilledTokens)>,
    ) -> Result<Self, StoreError> {
        let mut bases = BTreeMap::new();
        let mut fork_bases = BTreeMap::new();
        for (id, pack, killed) in packs {
            if bases.contains_key(&id) {
                return Err(StoreError::DuplicateDocument(id));
            }
            let world = compile(&pack)?.with_killed_tokens(killed);
            for (fork, owner) in fork_owners(&id, &pack.documents) {
                if let Some(first) = fork_bases.insert(fork.clone(), owner.clone()) {
                    return Err(StoreError::DuplicateFork {
                        fork,
                        first,
                        second: owner,
                    });
                }
            }
            bases.insert(
                id,
                Base {
                    world,
                    documents: pack.documents,
                },
            );
        }
        Ok(Self {
            fallback: None,
            bases,
            fork_bases,
            strict_forks: true,
            workspaces: BTreeMap::new(),
            workspace_bases: BTreeMap::new(),
            next_id: 0,
        })
    }

    /// Every open workspace, in id order.
    pub fn workspaces(&self) -> impl Iterator<Item = &Workspace> {
        self.workspaces.values()
    }

    /// Open a new empty workspace and return it. Ids are minted `ws-1`, `ws-2`, …
    /// in open order.
    pub fn open(&mut self, mode: Mode, base_name: String) -> Option<&Workspace> {
        let base_key = match mode {
            Mode::Scope => base_name.clone(),
            Mode::Fork => self
                .fork_bases
                .get(&base_name)
                .cloned()
                .or_else(|| {
                    (!self.strict_forks && self.bases.len() == 1)
                        .then(|| self.bases.keys().next().cloned())
                        .flatten()
                })
                .or_else(|| (!self.strict_forks).then(|| base_name.clone()))?,
        };
        self.base_named(&base_key)?;
        self.next_id += 1;
        let id = format!("ws-{}", self.next_id);
        let workspace = Workspace::new(id.clone(), mode, base_name);
        self.workspace_bases.insert(id.clone(), base_key);
        Some(self.workspaces.entry(id.clone()).or_insert(workspace))
    }

    /// A shared reference to a workspace by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Workspace> {
        self.workspaces.get(id)
    }

    /// The base and a mutable workspace together, borrowing disjoint fields so a
    /// tool can compute findings against the base while staging into the
    /// workspace.
    pub fn parts_mut(&mut self, id: &str) -> Option<(&CompiledWorld, &mut Workspace)> {
        let base_name = self.workspace_bases.get(id)?.clone();
        let base = if let Some(base) = self.bases.get(&base_name) {
            &base.world
        } else {
            &self.fallback.as_ref()?.world
        };
        self.workspaces.get_mut(id).map(|ws| (base, ws))
    }

    /// The compiled base selected by a workspace.
    #[must_use]
    pub fn workspace_base(&self, id: &str) -> Option<&CompiledWorld> {
        let base_name = self.workspace_bases.get(id)?;
        self.base_named(base_name).map(|base| &base.world)
    }

    /// The source documents selected by a workspace.
    #[must_use]
    pub fn workspace_base_docs(&self, id: &str) -> Option<&[Document]> {
        let base_name = self.workspace_bases.get(id)?;
        self.base_named(base_name)
            .map(|base| base.documents.as_slice())
    }

    /// Cache a mutating call's result under its idempotency key together with the
    /// payload fingerprint that produced it, so a retry with the *same* payload
    /// replays and stages nothing, while the same key with a *different* payload
    /// is caught as a conflict (spec 11 §2; R3/M9). No-op if the workspace is gone.
    pub fn remember(
        &mut self,
        id: &str,
        key: String,
        fingerprint: String,
        result: serde_json::Value,
    ) {
        if let Some(ws) = self.workspaces.get_mut(id) {
            ws.idempotency.insert(key, (fingerprint, result));
        }
    }

    /// Discard a whole workspace, returning whether one was present.
    pub fn discard(&mut self, id: &str) -> bool {
        self.workspace_bases.remove(id);
        self.workspaces.remove(id).is_some()
    }

    fn base_named(&self, name: &str) -> Option<&Base> {
        self.bases.get(name).or(self.fallback.as_ref())
    }
}

fn fork_owners(document: &str, sources: &[Document]) -> Vec<(String, String)> {
    sources
        .iter()
        .flat_map(|source| &source.decls)
        .filter_map(|decl| match decl {
            Decl::Fork(fork) => Some((fork.question.text.clone(), document.to_owned())),
            _ => None,
        })
        .collect()
}
