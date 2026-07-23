//! The fork manager (`ENG-2`): fork stores, suppositions, derived-consequence
//! diffs, the ratification pre-flight, and generative promotion.
//!
//! A **fork** is a named condition context over an unchanged base [`World`]: a
//! valuation overlay (`suppose`/`suppress`), a set of fork-local proposed facts
//! (`add`), and an optional keep-stamp visibility restriction (`restrict`).
//! Creation is **`O(1)`** — no store is copied and no fixpoint re-runs; a fork is
//! an overlay `ν` plus local additions, layered only when a consequence is
//! actually read (charter koan 2, *nothing changes but a ruling*). The base
//! world is never mutated; a fork is a lens on consequence, not a copy of the
//! world.
//!
//! This lane defines its OWN result types ([`ForkDelta`], [`Preflight`],
//! [`RqDraft`]); it never imports `dli-serve` wire shapes. Dependencies flow
//! engine → wire, and `dli-serve` maps these onto the contract in wave 3.
//!
//! ```
//! use dli_core::ann::Ann;
//! use dli_core::posbool::PosBool;
//! use dli_eval::fork::ForkStore;
//! use dli_eval::world::{GroundAtom, World};
//!
//! // A base world with one shadowed fact: `keeps(fen)` holds only if the open
//! // question variable `q-bell` is ruled true — merely possible until then.
//! let mut world = World::new();
//! let q = world.interner.intern("q-bell");
//! world.assert_fact(
//!     GroundAtom { pred: "keeps".to_owned(), args: vec!["fen".to_owned()] },
//!     Ann::supported(PosBool::pos(q)),
//! );
//! let world = world.fixpoint().expect("stratifiable");
//!
//! // Forking is O(1): no store copy, no re-fixpoint.
//! let mut store = ForkStore::new(world);
//! let fork = store.fork();
//!
//! // Suppose the question ruled: the shadow becomes a certain consequence.
//! store.suppose(fork, "q-bell", true).expect("known variable");
//! let delta = store.diff(fork).expect("evaluable");
//! assert_eq!(delta.newly_certain.len(), 1);
//! assert_eq!(delta.newly_certain[0].atom.render(), "keeps(fen)");
//!
//! // Retracting the supposition restores the base behavior.
//! store.suppress(fork, "q-bell").expect("known variable");
//! assert!(store.diff(fork).expect("evaluable").newly_certain.is_empty());
//! ```

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use miette::Diagnostic;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use dli_core::ann::{Ann, Status};
use dli_core::atom::Atom;
use dli_core::grade::Grade;
use dli_core::valuation::Valuation;

use crate::error::EvalError;
use crate::world::{Fact, GroundAtom, World};

/// A handle to a fork within a [`ForkStore`]. Stable for the store's lifetime.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct ForkId(pub u64);

/// A keep-stamp's visibility dimension — *visibility, not valuation* (spec 02
/// §5). A kept-absent or removed atom is still present and true in the store; it
/// is only filtered from the fork's presented consequences, which is what keeps
/// licensed ignorance mechanically distinct from falsity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Force the scoped atoms to appear in the fork's view.
    KeptPresent,
    /// Hide the scoped atoms from the view though they remain true.
    KeptAbsent,
    /// Remove the scoped atoms from the view entirely.
    Removed,
}

/// A keep-stamp: a visibility restriction over a scope of atoms, applied *after*
/// evaluation as a filter over a fork's [`ForkDelta`]. It never changes any
/// atom's standing — only whether the fork surfaces it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KeepStamp {
    /// The scope: predicate names or rendered `pred(a, b)` atoms the stamp
    /// covers.
    pub scope: Vec<String>,
    /// The visibility applied to the scoped atoms.
    pub visibility: Visibility,
}

/// A fork-local proposed fact: a ground atom, its annotation, and the grade of
/// the procedure that produced it. Additions are layered onto the base program
/// and run the incremental fixpoint only when a consequence is read; provenance
/// is expected to rest on a fork-scoped hypothesis or a supposed option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Addition {
    /// The ground atom introduced within the fork.
    pub atom: GroundAtom,
    /// Its support and defeater polynomials.
    pub ann: Ann,
    /// The grade at which the fork-local claim is known.
    pub grade: Grade,
}

impl Addition {
    /// A fork-local proposed fact with an explicit grade.
    #[must_use]
    pub fn new(atom: GroundAtom, ann: Ann, grade: Grade) -> Self {
        Self { atom, ann, grade }
    }

    /// An unconditional fork-local fact, asserted by fiat (`Grade::Asserted`).
    #[must_use]
    pub fn asserted(atom: GroundAtom, ann: Ann) -> Self {
        Self {
            atom,
            ann,
            grade: Grade::Asserted,
        }
    }
}

/// One fork's private state: the valuation overlay, the fork-local additions,
/// and any keep-stamps. The base [`World`] is shared and never mutated, so this
/// is the entire cost of a fork — creation allocates only this.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fork {
    /// A display name, if the fork was named (used for the promotion slug).
    pub name: Option<String>,
    /// The fork this was branched from, if any (lineage).
    pub parent: Option<ForkId>,
    /// The valuation overlay: event variables the fork supposes true or false,
    /// applied over the base ledger `ν` at evaluation time.
    pub overlay: BTreeMap<Atom, bool>,
    /// The fork-local proposed facts.
    pub additions: Vec<Addition>,
    /// The keep-stamps applied to the fork's presented consequences.
    pub keep_stamps: Vec<KeepStamp>,
}

/// A claim's standing in one world under one valuation: its three-valued status
/// and its grade. `before`/`after` pairs of these are the atoms of a diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Standing {
    /// The claim's three-valued status (a dead or absent atom reads `dead`).
    pub status: Status,
    /// The grade at which the claim is known.
    pub grade: Grade,
}

/// One changed atom in a derived-consequence diff: the ground atom and its
/// standing before (base) and after (fork), with a rendered one-line summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AtomChange {
    /// The ground atom whose standing changed.
    pub atom: GroundAtom,
    /// Its standing under the base ledger.
    pub before: Standing,
    /// Its standing under the fork's candidate valuation.
    pub after: Standing,
    /// A rendered one-line summary of the change.
    pub rendered: String,
}

/// The kind of a conflict, naming the three ways forks (and a fork against the
/// ledger) can disagree (spec 02 §5, the merge constraint-join):
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictKind {
    /// Two sides valuate the same event variable differently.
    Valuation,
    /// The joined valuation violates `Θ` — two options of one exclusivity
    /// family are ruled true together.
    Exclusivity,
    /// The joined certain layers break a functional dependency or a
    /// `C`-constraint; the blame pair is the two monomials that cannot co-hold.
    Certain,
}

impl ConflictKind {
    /// A short human label for the conflict kind.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ConflictKind::Valuation => "valuation disagreement",
            ConflictKind::Exclusivity => "exclusivity violation",
            ConflictKind::Certain => "certain-layer contradiction",
        }
    }
}

/// A surfaced conflict: its kind, a blame pair (the two things that cannot both
/// hold), and a rendered one-line explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Conflict {
    /// Which of the three conflict kinds this is.
    pub kind: ConflictKind,
    /// The blame pair — the two claims or assignments that cannot co-hold.
    pub blame: (String, String),
    /// A rendered one-line explanation of the conflict.
    pub rendered: String,
}

/// The derived-consequence diff of a fork: the atoms whose `(status, grade)`
/// change between the base ledger and the fork, partitioned by how they moved,
/// plus any surfaced conflicts. This is the interesting output of a fork — a
/// lens on consequence, not a copy of the world.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkDelta {
    /// Atoms that became certain (assertable) under the fork.
    pub newly_certain: Vec<AtomChange>,
    /// Atoms that became merely possible (shadow) under the fork.
    pub newly_possible: Vec<AtomChange>,
    /// Atoms that went dead under the fork.
    pub newly_dead: Vec<AtomChange>,
    /// Atoms whose status held but whose grade changed.
    pub grade_changed: Vec<AtomChange>,
    /// Conflicts the fork's candidate valuation surfaces.
    pub conflicts: Vec<Conflict>,
}

/// The ratification pre-flight of a fork: the candidate valuation its
/// suppositions imply, the conflicts ratifying it would introduce, and the atoms
/// the ratification would affect. What would otherwise be discovered *after* a
/// decision propagates is asked *before* it (spec 02 §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Preflight {
    /// The would-be valuation if the fork's suppositions were ruled.
    pub candidate: Valuation,
    /// The conflicts the candidate valuation surfaces.
    pub conflicts: Vec<Conflict>,
    /// The atoms whose standing the candidate valuation changes.
    pub affected: Vec<GroundAtom>,
}

/// A ruling-request draft emitted by promoting a fork — pure data. The machine
/// never writes canon (charter koan 3); it emits a proposal whose `affects` list
/// is *generated* from the fork's touched atoms, so forged promotion of fork
/// content is closed by construction (spec 02 §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RqDraft {
    /// The ruling-request slug, from the fork name or its handle.
    pub slug: String,
    /// The question posed to the author.
    pub ask: String,
    /// The atoms the ruling would affect — generated from the diff, deduplicated
    /// and sorted; the affects-list of the would-be decision clause.
    pub affects: Vec<String>,
    /// The rendered derived-consequence diff.
    pub diff: Vec<String>,
    /// The evidence gathered for the ask — the fork's suppositions and additions.
    pub evidence: Vec<String>,
}

/// A failure in a fork operation.
#[derive(Debug, Clone, PartialEq, Eq, Error, Diagnostic)]
pub enum ForkError {
    /// The fork handle does not name a fork in this store.
    #[error("unknown fork {0:?}")]
    #[diagnostic(code(dli::fork::unknown_fork))]
    UnknownFork(ForkId),
    /// The supposition or suppression target is not an event variable of the
    /// base world.
    #[error("unknown supposition target '{0}'")]
    #[diagnostic(code(dli::fork::unknown_target))]
    UnknownTarget(String),
    /// Evaluating the fork's layered program failed.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Eval(#[from] EvalError),
}

/// The fork manager: a shared base [`World`] and the forks over it.
///
/// The base world is saturated once; every fork is a cheap overlay evaluated
/// against that single symbolic fixpoint. Creating a fork and supposing over it
/// never touch the base derived relation, so a thousand forks cost a thousand
/// small allocations, not a thousand fixpoints.
#[derive(Debug, Clone)]
pub struct ForkStore {
    base: World,
    forks: BTreeMap<ForkId, Fork>,
    next: u64,
}

impl ForkStore {
    /// Build a fork store over a saturated base world.
    #[must_use]
    pub fn new(base: World) -> Self {
        Self {
            base,
            forks: BTreeMap::new(),
            next: 0,
        }
    }

    /// The shared base world every fork is a lens over.
    #[must_use]
    pub fn base(&self) -> &World {
        &self.base
    }

    /// Create an anonymous fork over the base world. `O(1)`: no store copy, no
    /// fixpoint.
    pub fn fork(&mut self) -> ForkId {
        self.insert(Fork::default())
    }

    /// Create a named fork. The name seeds the promotion slug.
    pub fn fork_named(&mut self, name: impl Into<String>) -> ForkId {
        self.insert(Fork {
            name: Some(name.into()),
            ..Fork::default()
        })
    }

    /// Branch a new fork from an existing one, copying its overlay, additions,
    /// and keep-stamps and recording the lineage. `O(fork state)`, never
    /// `O(store)`.
    pub fn fork_from(&mut self, parent: ForkId) -> Result<ForkId, ForkError> {
        let mut child = self.get(parent)?.clone();
        child.parent = Some(parent);
        child.name = None;
        Ok(self.insert(child))
    }

    /// The handles of every open fork, in creation order.
    pub fn forks(&self) -> impl Iterator<Item = ForkId> + '_ {
        self.forks.keys().copied()
    }

    /// How many forks are open.
    #[must_use]
    pub fn len(&self) -> usize {
        self.forks.len()
    }

    /// Whether no forks are open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.forks.is_empty()
    }

    /// Borrow a fork's private state.
    pub fn get(&self, id: ForkId) -> Result<&Fork, ForkError> {
        self.forks.get(&id).ok_or(ForkError::UnknownFork(id))
    }

    /// The fork's effective valuation: the base ledger `ν` with the fork's
    /// overlay applied and exclusivity closed — a supposed option's siblings are
    /// already ruled false *inside* the returned valuation. This is the seam a
    /// query's `--fork` path consumes: because the theory pass is baked in here,
    /// a consumer may `eval_under` it directly or lay it over the ambient ledger
    /// as a plain overlay (overlay wins) and get the right dead siblings either
    /// way, with no re-derivation. It is exactly [`Preflight::candidate`].
    ///
    /// ```
    /// use dli_core::ann::Ann;
    /// use dli_core::atom::Atom;
    /// use dli_core::posbool::PosBool;
    /// use dli_eval::fork::ForkStore;
    /// use dli_eval::world::{GroundAtom, World};
    ///
    /// // Two exclusive options; each holds only when its own variable is ruled.
    /// let mut world = World::new();
    /// let fen = world.interner.intern("old-fen");
    /// let mara = world.interner.intern("mara");
    /// world.theory.add_exclusive([fen, mara]);
    /// world.assert_fact(
    ///     GroundAtom { pred: "keeps".to_owned(), args: vec!["old-fen".to_owned()] },
    ///     Ann::supported(PosBool::pos(fen)),
    /// );
    /// let world = world.fixpoint().expect("stratifiable");
    ///
    /// let mut store = ForkStore::new(world);
    /// let fork = store.fork();
    /// store.suppose(fork, "old-fen", true).expect("known variable");
    ///
    /// // Supposing `old-fen` true rules the sibling `mara` false in the handed-out
    /// // valuation — the theory pass is already baked in.
    /// let nu = store.valuation(fork).expect("known fork");
    /// assert_eq!(nu.get(fen), Some(true));
    /// assert_eq!(nu.get(mara), Some(false));
    /// ```
    pub fn valuation(&self, id: ForkId) -> Result<Valuation, ForkError> {
        Ok(self.effective_valuation(self.get(id)?))
    }

    /// Suppose an event variable (an option, a hypothesis, or a decision clause)
    /// to a value within the fork. Supposing an option true rules its
    /// exclusivity siblings out at evaluation time; supposing `false` knocks a
    /// bundle out (spec 02 §5). The base world is untouched.
    pub fn suppose(&mut self, id: ForkId, target: &str, value: bool) -> Result<(), ForkError> {
        let atom = self.resolve(target)?;
        self.get_mut(id)?.overlay.insert(atom, value);
        Ok(())
    }

    /// Retract a supposition, reverting the variable to its base valuation
    /// (`dli.fork.suppress`, *retract an assumption*). Retracting a variable that
    /// was never supposed is a no-op — it is already at base.
    pub fn suppress(&mut self, id: ForkId, target: &str) -> Result<(), ForkError> {
        let atom = self.resolve(target)?;
        self.get_mut(id)?.overlay.remove(&atom);
        Ok(())
    }

    /// Add fork-local proposed facts. These run the incremental fixpoint only
    /// when a consequence is read (`diff`/`preflight`/`promote`).
    pub fn add(
        &mut self,
        id: ForkId,
        additions: impl IntoIterator<Item = Addition>,
    ) -> Result<(), ForkError> {
        self.get_mut(id)?.additions.extend(additions);
        Ok(())
    }

    /// Apply a keep-stamp visibility restriction to the fork's presented
    /// consequences. Visibility, not valuation: standings are unchanged.
    pub fn restrict(&mut self, id: ForkId, stamp: KeepStamp) -> Result<(), ForkError> {
        self.get_mut(id)?.keep_stamps.push(stamp);
        Ok(())
    }

    /// The derived-consequence diff of a fork against the base ledger: the atoms
    /// whose `(status, grade)` change, partitioned, plus surfaced conflicts and
    /// with the fork's keep-stamps applied.
    pub fn diff(&self, id: ForkId) -> Result<ForkDelta, ForkError> {
        let fork = self.get(id)?;
        let candidate = self.effective_valuation(fork);
        let layered = self.saturated(fork)?;
        let before = standings(&self.base, &self.base.valuation);
        let after = standings(&layered, &candidate);
        let mut delta = classify(&before, &after);
        delta.conflicts = self.scan_conflicts(fork, &candidate);
        apply_keep_stamps(fork, &mut delta);
        Ok(delta)
    }

    /// The ratification pre-flight of a fork: the candidate valuation, the
    /// conflicts ratifying it would introduce, and the affected atoms. Unlike
    /// [`diff`](Self::diff), the affected set ignores keep-stamps — a hidden atom
    /// is still true and can still conflict.
    ///
    /// ```
    /// use dli_core::ann::Ann;
    /// use dli_core::posbool::PosBool;
    /// use dli_eval::fork::ForkStore;
    /// use dli_eval::world::{GroundAtom, World};
    ///
    /// // The ledger has already ruled `mara` out: a certain fact closed the
    /// // option (its variable is fixed false in the base ν).
    /// let mut world = World::new();
    /// let mara = world.interner.intern("mara");
    /// world.assert_fact(
    ///     GroundAtom { pred: "keeps".to_owned(), args: vec!["mara".to_owned()] },
    ///     Ann::supported(PosBool::pos(mara)),
    /// );
    /// world.valuation.set(mara, false);
    /// let world = world.fixpoint().expect("stratifiable");
    ///
    /// let mut store = ForkStore::new(world);
    /// let fork = store.fork();
    /// // Supposing the ruled-out option true contradicts the certain fact.
    /// store.suppose(fork, "mara", true).expect("known variable");
    /// let pre = store.preflight(fork).expect("evaluable");
    /// assert!(!pre.conflicts.is_empty());
    /// ```
    pub fn preflight(&self, id: ForkId) -> Result<Preflight, ForkError> {
        let fork = self.get(id)?;
        let candidate = self.effective_valuation(fork);
        let conflicts = self.scan_conflicts(fork, &candidate);
        let layered = self.saturated(fork)?;
        let before = standings(&self.base, &self.base.valuation);
        let after = standings(&layered, &candidate);
        let delta = classify(&before, &after);
        let mut affected: BTreeSet<GroundAtom> = BTreeSet::new();
        for change in touched(&delta) {
            affected.insert(change.atom.clone());
        }
        Ok(Preflight {
            candidate,
            conflicts,
            affected: affected.into_iter().collect(),
        })
    }

    /// Emit a ruling-request draft from a fork — pure data, generatively. The
    /// `affects` list is exactly the diff's touched atoms; the machine proposes,
    /// never rules.
    ///
    /// ```
    /// use dli_core::ann::Ann;
    /// use dli_core::posbool::PosBool;
    /// use dli_eval::fork::ForkStore;
    /// use dli_eval::world::{GroundAtom, World};
    ///
    /// let mut world = World::new();
    /// let q = world.interner.intern("q-bell");
    /// world.assert_fact(
    ///     GroundAtom { pred: "keeps".to_owned(), args: vec!["fen".to_owned()] },
    ///     Ann::supported(PosBool::pos(q)),
    /// );
    /// let world = world.fixpoint().expect("stratifiable");
    ///
    /// let mut store = ForkStore::new(world);
    /// let fork = store.fork_named("the-fen-keeps-the-ledger");
    /// store.suppose(fork, "q-bell", true).expect("known variable");
    /// let rq = store.promote(fork).expect("evaluable");
    /// assert_eq!(rq.slug, "the-fen-keeps-the-ledger");
    /// assert_eq!(rq.affects, vec!["keeps(fen)".to_owned()]);
    /// ```
    pub fn promote(&self, id: ForkId) -> Result<RqDraft, ForkError> {
        let fork = self.get(id)?;
        let delta = self.diff(id)?;

        let mut affects: BTreeSet<String> = BTreeSet::new();
        let mut diff: Vec<String> = Vec::new();
        for change in touched(&delta) {
            affects.insert(change.atom.render());
            diff.push(change.rendered.clone());
        }

        let mut evidence: Vec<String> = Vec::new();
        for (&atom, &value) in &fork.overlay {
            evidence.push(format!("suppose {} = {value}", self.name_of(atom)));
        }
        for addition in &fork.additions {
            evidence.push(format!(
                "add {} [{}]",
                addition.atom.render(),
                addition.grade
            ));
        }

        let slug = slug_of(fork, id);
        let ask = format!(
            "Ratify fork `{slug}`? {} newly certain, {} newly dead, {} grade change(s), \
             {} conflict(s).",
            delta.newly_certain.len(),
            delta.newly_dead.len(),
            delta.grade_changed.len(),
            delta.conflicts.len(),
        );

        Ok(RqDraft {
            slug,
            ask,
            affects: affects.into_iter().collect(),
            diff,
            evidence,
        })
    }

    /// Merge two forks, reporting the conflicts that block a join (spec 02 §5).
    /// An empty result means the forks are joinable. This is the constraint-join
    /// over the two overlays; the `Certain`-kind (functional-dependency) check
    /// lands with the continuity/slot layer.
    pub fn merge(&self, left: ForkId, right: ForkId) -> Result<Vec<Conflict>, ForkError> {
        let a = self.get(left)?;
        let b = self.get(right)?;
        let mut conflicts = Vec::new();

        // (1) Valuation disagreement: a variable both forks supposed, differently.
        for (&atom, &va) in &a.overlay {
            if let Some(&vb) = b.overlay.get(&atom)
                && va != vb
            {
                let name = self.name_of(atom);
                conflicts.push(Conflict {
                    kind: ConflictKind::Valuation,
                    blame: (
                        format!("left: {name} = {va}"),
                        format!("right: {name} = {vb}"),
                    ),
                    rendered: format!("the forks valuate {name} differently ({va} vs {vb})"),
                });
            }
        }

        // (2) Exclusivity: the joined true-set violates a Θ family.
        let mut joined = BTreeMap::new();
        for (&atom, &val) in a.overlay.iter().chain(&b.overlay) {
            joined.entry(atom).or_insert(val);
        }
        conflicts
            .extend(self.exclusivity_conflicts(|atom| joined.get(&atom).copied() == Some(true)));

        Ok(conflicts)
    }

    // -- internals -----------------------------------------------------------

    fn insert(&mut self, fork: Fork) -> ForkId {
        let id = ForkId(self.next);
        self.next += 1;
        self.forks.insert(id, fork);
        id
    }

    fn get_mut(&mut self, id: ForkId) -> Result<&mut Fork, ForkError> {
        self.forks.get_mut(&id).ok_or(ForkError::UnknownFork(id))
    }

    fn resolve(&self, name: &str) -> Result<Atom, ForkError> {
        self.base
            .interner
            .names()
            .iter()
            .position(|n| n == name)
            .and_then(|i| u32::try_from(i).ok())
            .map(Atom)
            .ok_or_else(|| ForkError::UnknownTarget(name.to_owned()))
    }

    fn name_of(&self, atom: Atom) -> String {
        self.base
            .interner
            .resolve(atom)
            .map_or_else(|| format!("var#{}", atom.0), ToOwned::to_owned)
    }

    /// The candidate valuation a fork implies: the base ledger, then the overlay,
    /// then exclusivity closure — supposing an option true rules its *open*
    /// siblings false, but never overrides a ruling or another supposition (so a
    /// genuine `Θ` clash stays visible to the conflict scan).
    fn effective_valuation(&self, fork: &Fork) -> Valuation {
        let mut nu = self.base.valuation.clone();
        for (&atom, &val) in &fork.overlay {
            nu.set(atom, val);
        }
        for (&atom, &val) in &fork.overlay {
            if !val {
                continue;
            }
            for family in &self.base.theory.exclusive {
                if !family.contains(&atom) {
                    continue;
                }
                for &sibling in family {
                    if sibling == atom
                        || fork.overlay.contains_key(&sibling)
                        || self.base.valuation.get(sibling).is_some()
                    {
                        continue;
                    }
                    nu.set(sibling, false);
                }
            }
        }
        nu
    }

    /// The saturated world a fork evaluates against. With no additions this *is*
    /// the base world (borrowed, zero-copy); additions layer onto the base
    /// program and run the fixpoint once.
    fn saturated(&self, fork: &Fork) -> Result<Cow<'_, World>, EvalError> {
        if fork.additions.is_empty() {
            return Ok(Cow::Borrowed(&self.base));
        }
        let mut program = self.base.clone();
        program.derived.clear();
        program.support_index.clear();
        for addition in &fork.additions {
            program.facts.push(Fact {
                atom: addition.atom.clone(),
                ann: addition.ann.clone(),
                grade: addition.grade,
            });
        }
        Ok(Cow::Owned(program.fixpoint()?))
    }

    /// Scan a fork's candidate valuation for conflicts: a supposition overriding
    /// a ledger ruling (valuation kind) and a joined exclusivity-family violation
    /// (exclusivity kind).
    fn scan_conflicts(&self, fork: &Fork, candidate: &Valuation) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        for (&atom, &val) in &fork.overlay {
            if let Some(base_val) = self.base.valuation.get(atom)
                && base_val != val
            {
                let name = self.name_of(atom);
                conflicts.push(Conflict {
                    kind: ConflictKind::Valuation,
                    blame: (
                        format!("suppose {name} = {val}"),
                        format!("ledger rules {name} = {base_val}"),
                    ),
                    rendered: format!(
                        "{name}: supposed {val}, but a certain fact rules it {base_val}"
                    ),
                });
            }
        }
        conflicts.extend(self.exclusivity_conflicts(|atom| candidate.get(atom) == Some(true)));
        conflicts
    }

    /// Exclusivity conflicts under a truth predicate: every family with two or
    /// more members held true at once.
    fn exclusivity_conflicts(&self, is_true: impl Fn(Atom) -> bool) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        for family in &self.base.theory.exclusive {
            let trues: Vec<Atom> = family.iter().copied().filter(|&a| is_true(a)).collect();
            if trues.len() >= 2 {
                let a = self.name_of(trues[0]);
                let b = self.name_of(trues[1]);
                conflicts.push(Conflict {
                    kind: ConflictKind::Exclusivity,
                    blame: (a.clone(), b.clone()),
                    rendered: format!(
                        "{a} and {b} cannot both be ruled — they are options of one question"
                    ),
                });
            }
        }
        conflicts
    }
}

/// Every atom's standing in a world under a valuation, dead atoms included (the
/// diff needs them, unlike a query answer). Support is untouched; only status is
/// re-derived, so this is one evaluation pass over the precomputed fixpoint.
fn standings(world: &World, nu: &Valuation) -> BTreeMap<GroundAtom, (Status, Grade)> {
    let mut out = BTreeMap::new();
    for derived in &world.derived {
        let ann = derived.ann();
        let status = ann.status(nu, &world.theory);
        let grade = world.grade_under(derived, nu);
        out.insert(derived.atom.clone(), (status, grade));
    }
    out
}

/// Partition the atoms whose standing changed between two evaluations. Status
/// transitions dominate; a pure grade move (same live status) is a grade change.
fn classify(
    before: &BTreeMap<GroundAtom, (Status, Grade)>,
    after: &BTreeMap<GroundAtom, (Status, Grade)>,
) -> ForkDelta {
    let mut delta = ForkDelta::default();
    let mut atoms: BTreeSet<&GroundAtom> = before.keys().collect();
    atoms.extend(after.keys());
    for atom in atoms {
        let b = before.get(atom).copied();
        let a = after.get(atom).copied();
        let before_status = b.map_or(Status::Dead, |x| x.0);
        let after_status = a.map_or(Status::Dead, |x| x.0);
        let before_standing = Standing {
            status: before_status,
            grade: b.map_or(Grade::Asserted, |x| x.1),
        };
        let after_standing = Standing {
            status: after_status,
            grade: a.map_or(Grade::Asserted, |x| x.1),
        };
        if before_status == after_status {
            if before_status != Status::Dead && before_standing.grade != after_standing.grade {
                delta.grade_changed.push(atom_change(
                    atom.clone(),
                    before_standing,
                    after_standing,
                ));
            }
            continue;
        }
        let change = atom_change(atom.clone(), before_standing, after_standing);
        match after_status {
            Status::Certain => delta.newly_certain.push(change),
            Status::Possible => delta.newly_possible.push(change),
            Status::Dead => delta.newly_dead.push(change),
        }
    }
    delta
}

/// Render a single change, noting the grade when only the grade moved.
fn atom_change(atom: GroundAtom, before: Standing, after: Standing) -> AtomChange {
    let rendered = if before.status == after.status {
        format!(
            "{}: {} ({} → {})",
            atom.render(),
            status_word(after.status),
            before.grade,
            after.grade,
        )
    } else {
        format!(
            "{}: {} → {}",
            atom.render(),
            status_word(before.status),
            status_word(after.status),
        )
    };
    AtomChange {
        atom,
        before,
        after,
        rendered,
    }
}

/// The four status/grade partitions of a delta, chained — every touched atom.
fn touched(delta: &ForkDelta) -> impl Iterator<Item = &AtomChange> {
    delta
        .newly_certain
        .iter()
        .chain(&delta.newly_possible)
        .chain(&delta.newly_dead)
        .chain(&delta.grade_changed)
}

/// Drop the keep-stamped-hidden atoms from a delta's presented lists. Kept-absent
/// and removed atoms remain true in the store; they are only filtered from view.
fn apply_keep_stamps(fork: &Fork, delta: &mut ForkDelta) {
    if fork.keep_stamps.is_empty() {
        return;
    }
    let hidden = |atom: &GroundAtom| {
        fork.keep_stamps.iter().any(|stamp| {
            matches!(
                stamp.visibility,
                Visibility::Removed | Visibility::KeptAbsent
            ) && stamp
                .scope
                .iter()
                .any(|entry| entry == &atom.pred || entry == &atom.render())
        })
    };
    delta.newly_certain.retain(|c| !hidden(&c.atom));
    delta.newly_possible.retain(|c| !hidden(&c.atom));
    delta.newly_dead.retain(|c| !hidden(&c.atom));
    delta.grade_changed.retain(|c| !hidden(&c.atom));
}

/// The promotion slug: the fork's name slugified, or its handle.
fn slug_of(fork: &Fork, id: ForkId) -> String {
    match &fork.name {
        Some(name) => {
            let slug = slugify(name);
            if slug.is_empty() {
                format!("fork-{}", id.0)
            } else {
                slug
            }
        }
        None => format!("fork-{}", id.0),
    }
}

/// Lowercase, hyphenate, and collapse a display name into a kebab slug.
fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }
    out
}

/// The lowercase status word for a rendered change line.
fn status_word(status: Status) -> &'static str {
    match status {
        Status::Certain => "certain",
        Status::Possible => "possible",
        Status::Dead => "dead",
    }
}
