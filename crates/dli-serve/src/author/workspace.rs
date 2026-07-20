//! Door T workspace (`AUTH-1`): the in-memory staged layer over a compiled base.
//!
//! Nothing an agent does touches disk and nothing changes canon (spec 11 §2). A
//! [`Workspace`] is a staged layer of typed AST declarations over an unchanged
//! compiled base, its fork-local suppositions, an idempotency replay cache, and
//! the current digest fence. The fence is a content hash over the staged state
//! ([`digest`](crate::author::digest)), so it moves exactly when the staged
//! state moves.
//!
//! The fence is resealed **incrementally** (R3/H4): each declaration's canonical
//! JSON is cached the moment it is staged, so a mutation over an `n`-declaration
//! workspace re-serializes only the one declaration it touched, not all `n`. The
//! bytes hashed are byte-identical to serializing the whole snapshot, so the
//! digest value is unchanged — the optimization is invisible on the wire.

use std::collections::BTreeMap;

use serde::Serialize;

use dli_ast::decl::Decl;

use crate::author::digest::digest_bytes;

/// Which authoring door a workspace was opened in (spec 11 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// `{"scope": …}` — stage over an existing scope (a pack, in DL-0016's
    /// renamed surface); additions accrue to that scope's surface.
    Scope,
    /// `{"fork": …}` — a fork-scoped workspace; hypotheses and suppositions are
    /// fork-local and never leak into canon (koan 2).
    Fork,
}

/// One authoring workspace.
#[derive(Debug, Clone)]
pub struct Workspace {
    /// The workspace id (`ws-<n>`).
    pub id: String,
    /// The door the workspace was opened in.
    pub mode: Mode,
    /// The pack or fork name given at open — the base the staging is over, and
    /// the `scope` header `author.materialize` emits.
    pub base_name: String,
    /// The staged declarations, in the order they were added (typed, AST-level).
    /// Mutate only through [`push_decl`](Workspace::push_decl),
    /// [`replace_decl`](Workspace::replace_decl), and
    /// [`remove_decl`](Workspace::remove_decl), which keep the digest cache in
    /// step.
    pub decls: Vec<Decl>,
    /// The hypothesis ids supposed true within the fork (spec 11 §3.9).
    pub suppositions: Vec<String>,
    /// The current digest fence.
    pub digest: String,
    /// Idempotency replay cache: key → (payload fingerprint, the exact result
    /// value first produced). The fingerprint guards key reuse: the same key with
    /// a different payload is a conflict, never a silent cached replay (R3/M9).
    pub idempotency: BTreeMap<String, (String, serde_json::Value)>,
    /// Cached canonical JSON of each staged declaration, parallel to `decls`, so
    /// a reseal re-serializes only the declaration that changed.
    decl_json: Vec<String>,
}

impl Workspace {
    /// Open an empty workspace with its initial digest sealed.
    #[must_use]
    pub fn new(id: String, mode: Mode, base_name: String) -> Self {
        let mut workspace = Self {
            id,
            mode,
            base_name,
            decls: Vec::new(),
            suppositions: Vec::new(),
            digest: String::new(),
            idempotency: BTreeMap::new(),
            decl_json: Vec::new(),
        };
        workspace.reseal();
        workspace
    }

    /// Stage a declaration, caching its canonical JSON and resealing the digest.
    pub fn push_decl(&mut self, decl: Decl) {
        self.decl_json.push(encode(&decl));
        self.decls.push(decl);
        self.reseal();
    }

    /// Replace the declaration at `index` (e.g. a question that gained an option),
    /// re-encoding just that one, and reseal.
    pub fn replace_decl(&mut self, index: usize, decl: Decl) {
        if let (Some(slot), Some(cached)) =
            (self.decls.get_mut(index), self.decl_json.get_mut(index))
        {
            *cached = encode(&decl);
            *slot = decl;
            self.reseal();
        }
    }

    /// Discard the single staged declaration at `index`, returning it, and reseal.
    pub fn remove_decl(&mut self, index: usize) -> Option<Decl> {
        if index >= self.decls.len() {
            return None;
        }
        self.decl_json.remove(index);
        let removed = self.decls.remove(index);
        self.reseal();
        Some(removed)
    }

    /// Recompute the digest over the current staged state. Assembles the snapshot
    /// bytes from the cached per-declaration JSON (so unchanged declarations are
    /// not re-serialized) — byte-identical to serializing the whole snapshot.
    pub fn reseal(&mut self) {
        let material = format!(
            "{{\"base_name\":{},\"mode\":{},\"decls\":[{}],\"suppositions\":{}}}",
            encode(&self.base_name),
            encode(&self.mode),
            self.decl_json.join(","),
            encode(&self.suppositions),
        );
        self.digest = digest_bytes(material.as_bytes());
    }
}

/// The canonical JSON encoding of a value (empty on the impossible serialize
/// failure — every type here is infallibly serializable).
fn encode<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{Mode, Workspace, encode};

    use dli_ast::decl::{Decl, EntityDecl, EntityKind};
    use dli_ast::ident::Ident;
    use serde::Serialize;

    /// The whole-snapshot shape the incremental reseal must reproduce byte for
    /// byte.
    #[derive(Serialize)]
    struct Snapshot<'a> {
        base_name: &'a str,
        mode: Mode,
        decls: &'a [Decl],
        suppositions: &'a [String],
    }

    fn person(name: &str) -> Decl {
        Decl::Entity(EntityDecl {
            kind: EntityKind::Person,
            name: Ident::synthetic(name),
            label: Some(name.to_owned()),
            time: None,
            slots: Vec::new(),
            provenance: None,
        })
    }

    #[test]
    fn incremental_reseal_matches_the_full_snapshot() {
        let mut ws = Workspace::new("ws-1".to_owned(), Mode::Scope, "saltmarket".to_owned());
        ws.push_decl(person("mara"));
        ws.push_decl(person("the-warden"));
        ws.suppositions.push("h".to_owned());
        ws.reseal();
        ws.remove_decl(0);

        let snapshot = crate::author::digest::digest_of(&Snapshot {
            base_name: &ws.base_name,
            mode: ws.mode,
            decls: &ws.decls,
            suppositions: &ws.suppositions,
        });
        assert_eq!(
            ws.digest, snapshot,
            "the incremental reseal equals the full-snapshot digest"
        );
        // `encode` is exercised above; keep the import meaningful.
        assert!(!encode(&ws.mode).is_empty());
    }
}
