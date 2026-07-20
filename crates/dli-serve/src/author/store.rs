//! Door T store (`AUTH-1`): every open authoring workspace over a shared base.
//!
//! The store owns the compiled **base** the workspaces stage over and the map of
//! open workspaces. In `AUTH-1` the base is a single shared
//! [`CompiledWorld`] injected at construction; wiring a per-session/per-pack
//! corpus lookup is the server builder's seam (`SERVE-1`), which will construct
//! the store's base from the loaded session corpus before routing an
//! `author.open` through [`dispatch`](crate::author::dispatch).

use std::collections::BTreeMap;

use dli_world::CompiledWorld;

use super::workspace::{Mode, Workspace};

/// The workspace store: the shared compiled base plus every open workspace,
/// keyed by id.
#[derive(Debug, Default)]
pub struct Store {
    base: CompiledWorld,
    workspaces: BTreeMap<String, Workspace>,
    next_id: u32,
}

impl Store {
    /// A store over a given compiled base — the seam the server fills from the
    /// session corpus.
    #[must_use]
    pub fn with_base(base: CompiledWorld) -> Self {
        Self {
            base,
            workspaces: BTreeMap::new(),
            next_id: 0,
        }
    }

    /// The compiled base every workspace stages over.
    #[must_use]
    pub fn base(&self) -> &CompiledWorld {
        &self.base
    }

    /// Open a new empty workspace and return it. Ids are minted `ws-1`, `ws-2`, …
    /// in open order.
    pub fn open(&mut self, mode: Mode, base_name: String) -> &Workspace {
        self.next_id += 1;
        let id = format!("ws-{}", self.next_id);
        let workspace = Workspace::new(id.clone(), mode, base_name);
        self.workspaces.entry(id.clone()).or_insert(workspace)
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
        self.workspaces.get_mut(id).map(|ws| (&self.base, ws))
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
        self.workspaces.remove(id).is_some()
    }
}
