//! The premise-pool projection — the one seam between a practice and the world.
//!
//! A casting's available premises are exactly **certain ∩ known-by-speaker ∩
//! present-in-scene** (`docs/spec/04-practices.md` §2). This module is that
//! projection, as a [`SceneProjection`] of plain data: the checker
//! ([`crate::ledger_check`]) reads it and nothing else about the world.
//!
//! # The dependency inversion
//!
//! The projection *inputs* live upstream — the certain atoms come from the
//! evaluator's valuation, the knowledge map from the timeline, cast presence from
//! the scene. But `dli-world` already depends on `dli-ledger` (it stores compiled
//! [`PracticeDef`](crate::PracticeDef)s), so this crate cannot depend back on
//! `dli-world` or `dli-eval` without a cycle. The inversion: **the caller
//! constructs the [`SceneProjection`]** from those sources and hands it in as
//! plain data. `dli-world` (PAY-2) and the server (SERVE-1) build it; this crate
//! only consumes it. Atoms cross the seam as their canonical text — normalize both
//! sides with [`normalize_atom`] (the builder helpers do this for you).

use std::collections::{BTreeMap, BTreeSet};

/// Where a premise stands relative to the scene projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PremiseStatus {
    /// The atom is in the pool — certain, known to the speaker, and on stage.
    Available,
    /// The atom is merely unestablished: not denied, but not in the pool. A
    /// casting resting here *fizzles* (the L2 discriminant *absent*).
    Absent,
    /// The atom is actively denied by the certain projection. A casting resting
    /// here *rebounds* (the L2 discriminant *contradicted*).
    Contradicted,
}

/// The world as projected into one scene, for one speaker — the premise pool.
///
/// Every set holds **canonical atom text** ([`normalize_atom`]); the builder
/// helpers normalize on the way in, so mix builder and direct construction freely
/// as long as directly-set strings are already canonical.
///
/// The pool proper is [`certain`](Self::certain) ∩
/// [`known_by_speaker`](Self::known_by_speaker) ∩ [`present`](Self::present);
/// [`denied`](Self::denied) is the certain projection's *negative* side (what it
/// actively refutes), which splits a rebound from a fizzle.
/// [`phases`](Self::phases) carries the cycle phases in force (for the timing side
/// condition), and [`caps`](Self::caps) resolves a practice's symbolic affine
/// capacities (`body-limit(caster)`) to the integer bound in force this scene.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneProjection {
    /// Atoms the certain projection of the world affirms as of this scene.
    pub certain: BTreeSet<String>,
    /// Atoms the certain projection actively denies as of this scene.
    pub denied: BTreeSet<String>,
    /// Atoms the speaker knows as of this scene.
    pub known_by_speaker: BTreeSet<String>,
    /// Atoms the scene puts on stage.
    pub present: BTreeSet<String>,
    /// The phase in force for each cycle in the scene (`tide` → `flood`).
    pub phases: BTreeMap<String, String>,
    /// The integer capacity in force for each affine resource this scene.
    pub caps: BTreeMap<String, i64>,
    /// The declared spokes (phases) of each cycle in scope (`tide` →
    /// {ebb, slack, flood, high}). A cycle absent from this map has an unknown
    /// spoke set, so spoke validation is deferred for it (never guessed) — like a
    /// symbolic affine cap. The caller populates it from the compiled cycle
    /// declarations.
    pub cycle_spokes: BTreeMap<String, BTreeSet<String>>,
}

impl SceneProjection {
    /// An empty projection — nothing certain, known, or present.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a **seated** premise: one that is certain, known to the speaker, and
    /// present — i.e. squarely in the pool. This is the common case a worked
    /// scene builds by hand (`docs/spec/04-practices.md` §5).
    #[must_use]
    pub fn seat(mut self, atom: &str) -> Self {
        let key = normalize_atom(atom);
        self.certain.insert(key.clone());
        self.known_by_speaker.insert(key.clone());
        self.present.insert(key);
        self
    }

    /// Add an atom the certain projection affirms (without asserting the speaker
    /// knows it or that it is on stage).
    #[must_use]
    pub fn certain(mut self, atom: &str) -> Self {
        self.certain.insert(normalize_atom(atom));
        self
    }

    /// Add an atom the certain projection **denies** — the seam that turns a
    /// fizzle into a rebound.
    #[must_use]
    pub fn deny(mut self, atom: &str) -> Self {
        self.denied.insert(normalize_atom(atom));
        self
    }

    /// Add an atom the speaker knows as of this scene.
    #[must_use]
    pub fn known(mut self, atom: &str) -> Self {
        self.known_by_speaker.insert(normalize_atom(atom));
        self
    }

    /// Add an atom the scene puts on stage.
    #[must_use]
    pub fn present(mut self, atom: &str) -> Self {
        self.present.insert(normalize_atom(atom));
        self
    }

    /// Seat a cycle at a phase (`at_phase("tide", "slack")`).
    #[must_use]
    pub fn at_phase(mut self, cycle: &str, spoke: &str) -> Self {
        self.phases.insert(cycle.to_owned(), spoke.to_owned());
        self
    }

    /// Resolve a practice's affine capacity for this scene (`cap("strain", 2)`).
    #[must_use]
    pub fn cap(mut self, resource: &str, bound: i64) -> Self {
        self.caps.insert(resource.to_owned(), bound);
        self
    }

    /// Declare a cycle's spokes so a timing premise's phase can be validated
    /// (`spokes("tide", &["ebb", "slack", "flood", "high"])`). Without this, a
    /// cycle's spoke set is unknown and spoke validation is deferred.
    #[must_use]
    pub fn spokes(mut self, cycle: &str, spokes: &[&str]) -> Self {
        self.cycle_spokes.insert(
            cycle.to_owned(),
            spokes.iter().map(|s| (*s).to_owned()).collect(),
        );
        self
    }

    /// Where `atom` stands in the pool: available, merely absent, or contradicted.
    ///
    /// A denied atom is [`Contradicted`](PremiseStatus::Contradicted) regardless
    /// of the other sets — the world refutes it. Otherwise it is
    /// [`Available`](PremiseStatus::Available) only if it clears all three pool
    /// gates, and [`Absent`](PremiseStatus::Absent) if it clears none or only
    /// some.
    ///
    /// ```
    /// use dli_ledger::premise::{PremiseStatus, SceneProjection};
    ///
    /// let scene = SceneProjection::new()
    ///     .seat("holds(linet, consent(household, door))")
    ///     .deny("hour(spring-tide)");
    /// assert_eq!(
    ///     scene.availability("holds(linet, consent(household, door))"),
    ///     PremiseStatus::Available
    /// );
    /// assert_eq!(scene.availability("hour(spring-tide)"), PremiseStatus::Contradicted);
    /// assert_eq!(scene.availability("holds(unwritten)"), PremiseStatus::Absent);
    /// ```
    #[must_use]
    pub fn availability(&self, atom: &str) -> PremiseStatus {
        let key = normalize_atom(atom);
        if self.denied.contains(&key) {
            return PremiseStatus::Contradicted;
        }
        if self.certain.contains(&key)
            && self.known_by_speaker.contains(&key)
            && self.present.contains(&key)
        {
            return PremiseStatus::Available;
        }
        PremiseStatus::Absent
    }

    /// The phase in force for `cycle` this scene, if the scene establishes one.
    #[must_use]
    pub fn phase_of(&self, cycle: &str) -> Option<&str> {
        self.phases.get(cycle).map(String::as_str)
    }

    /// The integer affine capacity in force for `resource` this scene, if resolved.
    #[must_use]
    pub fn cap_of(&self, resource: &str) -> Option<i64> {
        self.caps.get(resource).copied()
    }

    /// Whether `spoke` is a declared phase of `cycle`: `Some(true)` / `Some(false)`
    /// when the cycle's spokes are known, [`None`] when they are not (so the caller
    /// has not pinned them and validation is deferred).
    #[must_use]
    pub fn is_declared_spoke(&self, cycle: &str, spoke: &str) -> Option<bool> {
        self.cycle_spokes
            .get(cycle)
            .map(|spokes| spokes.contains(spoke))
    }
}

/// The canonical text of an atom: insignificant surface whitespace is removed,
/// while whitespace inside quoted values is preserved. Thus `holds(a, b)` and
/// `holds(a,b)` are one atom, but `says(a, "a b")` and `says(a, "ab")` remain
/// distinct claims.
#[must_use]
pub fn normalize_atom(atom: &str) -> String {
    let mut out = String::with_capacity(atom.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in atom.chars() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            in_string = true;
            out.push(ch);
        } else if !ch.is_whitespace() {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_surface_spacing() {
        assert_eq!(normalize_atom("holds(a, b)"), "holds(a,b)");
        assert_eq!(
            normalize_atom("  phase( tide , slack ) "),
            "phase(tide,slack)"
        );
        assert_ne!(
            normalize_atom("says(the-seeress, \"a b\")"),
            normalize_atom("says(the-seeress, \"ab\")")
        );
        assert_eq!(
            normalize_atom("says( the-seeress , \"a  b\" )"),
            "says(the-seeress,\"a  b\")"
        );
    }

    #[test]
    fn seat_puts_an_atom_in_all_three_gates() {
        let scene = SceneProjection::new().seat("lit(hearth)");
        assert_eq!(scene.availability("lit(hearth)"), PremiseStatus::Available);
    }

    #[test]
    fn partial_membership_is_absent_not_available() {
        // Certain and present, but the speaker does not know it: outside the pool.
        let scene = SceneProjection::new()
            .certain("secret(x)")
            .present("secret(x)");
        assert_eq!(scene.availability("secret(x)"), PremiseStatus::Absent);
    }

    #[test]
    fn denial_wins_over_the_pool() {
        // Even fully seated, an atom the world denies rebounds.
        let scene = SceneProjection::new()
            .seat("hour(slack)")
            .deny("hour(slack)");
        assert_eq!(
            scene.availability("hour(slack)"),
            PremiseStatus::Contradicted
        );
    }

    #[test]
    fn phase_and_cap_resolve() {
        let scene = SceneProjection::new()
            .at_phase("tide", "flood")
            .cap("strain", 2);
        assert_eq!(scene.phase_of("tide"), Some("flood"));
        assert_eq!(scene.phase_of("moon"), None);
        assert_eq!(scene.cap_of("strain"), Some(2));
    }
}
