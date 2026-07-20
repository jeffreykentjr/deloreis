//! The six continuity classes over the compiled world (`CHECK-1`).
//!
//! Ported faithfully from the legacy continuity checker (the consuming repo's
//! `fantasy.world.continuity`), whose six violation classes are:
//!
//! | legacy code | dli-world code | where it fires |
//! |---|---|---|
//! | `WORLD_CONTRADICTION` | `continuity.world-contradiction` | a claim clashes with a certain fact |
//! | `PRIOR_CONTRADICTION` | `continuity.prior-contradiction` | a claim reverses a prior episode's ledger |
//! | `UNKNOWN_ENTITY` | `continuity.unknown-entity` | a claim leans on an entity canon never declares |
//! | `PROPOSED_AS_FACT` | `continuity.proposed-as-fact` | a claim states shadow (possible-only) lore as settled |
//! | `INTRA_EPISODE_CONTRADICTION` | `continuity.intra-episode-contradiction` | a claim set sets one slot two ways |
//! | `REJECTED_LORE` | `continuity.rejected-lore` | a claim realizes a killed-token guardrail |
//!
//! One of the six is world-internal (it needs no external delta) and runs in
//! [`run`] alongside the loader's fail-closed findings: `rejected-lore` over the
//! certain projection. The other classes are claim-relative and run in the claim
//! battery ([`check_claims`]) that [`check_draft`](crate::check_draft) drives over
//! a draft's asserted claims; `world-contradiction` is reported there by the
//! fact-checker's `contradicted` verdict (`factcheck.contradicted`), so this
//! module's claim battery contributes the structural classes the verdict does
//! not: `unknown-entity`, `proposed-as-fact`, and `intra-episode-contradiction`.
//!
//! **Why the reference class is claim-relative, not world-internal.** Canon
//! legitimately attaches facts to *abstract subjects* — `magic-system`,
//! `structure`, `religion` — that carry no first-class entity declaration. The
//! loader registers every observed symbol, so "declared ∪ observed" is the honest
//! known-entity set (spec 05 §6: the compiled world *is* the name-space). A
//! world-internal "subject with no declaration" check would false-flag those
//! abstract subjects; the honest signal is a *claim* (an episode delta) that
//! references a symbol canon never even observed — which [`check_claims`] checks
//! against the full entity registry.
//!
//! [`run`] also surfaces the loader's fail-closed findings (missing/malformed
//! provenance, duplicate/cross-pack declarations) as [`WorldFinding`]s, so the
//! world-check battery is one report: fail-closed provenance *is* the
//! assertability gate (spec 02 §3), not a separate concern.

use std::collections::{BTreeMap, BTreeSet};

use dli_ast::term::{AtomExpr, Term, TimeRef};
use dli_core::ann::{Ann, Status};
use dli_core::atom::{Atom, Interner};

use super::{CompiledWorld, WorldFinding};
use crate::model::{LoadFinding, LoadedAtom};

/// The world-internal continuity battery: the classes that check the compiled
/// world against itself, needing no external draft.
///
/// Surfaces the loader's fail-closed [`LoadFinding`]s (provenance, uniqueness) and
/// the evaluator's lowering findings (a `citation.unknown-decision` forged-authority
/// claim, `B1`) as [`WorldFinding`]s, and emits `continuity.rejected-lore` for
/// every certain fact that realizes a killed token. Findings are sorted by
/// `(code, message)`.
#[must_use]
pub fn run(world: &CompiledWorld) -> Vec<WorldFinding> {
    let mut out = Vec::new();
    for finding in world.model.findings() {
        out.push(load_finding_to_world(finding));
    }
    for finding in &world.model.world.findings {
        out.push(WorldFinding::error(
            finding.code.clone(),
            finding.message.clone(),
        ));
    }
    out.extend(certain_rejected_lore(world));
    out.extend(chronology_cycle(world));
    out.sort_by(|a, b| (&a.code, &a.message).cmp(&(&b.code, &b.message)));
    out
}

/// Lift a loader fail-closed finding into a world-check finding, keeping its
/// stable code and domain-spoken message (the two-channel error model).
fn load_finding_to_world(finding: &LoadFinding) -> WorldFinding {
    WorldFinding::error(finding.code(), finding.message())
}

/// `continuity.chronology-cycle` when the era spine or the `before`-facts close
/// a precedence loop: a run of ordering rulings that says an event precedes
/// itself. The **same** cycle detection the event `before`-graph uses runs over
/// the whole chronology (era-order edges included), so a contradictory era chain
/// (`era a < b < a`) is a first-class finding — not silently accepted — and the
/// finding is blamed on the exact edges and their declaring rulings.
fn chronology_cycle(world: &CompiledWorld) -> Vec<WorldFinding> {
    let chronology = &world.model.world.chronology;
    let Some(blame) = chronology.find_cycle(&world.model.chronology_anns) else {
        return Vec::new();
    };
    let time_points = &world.model.world.time_points;
    let interner = &world.model.world.interner;

    let loop_render: Vec<String> = blame
        .cycle
        .iter()
        .map(|edge| {
            format!(
                "{} -> {}",
                point_name(time_points, edge.before),
                point_name(time_points, edge.after)
            )
        })
        .collect();

    let mut citations: Vec<String> = Vec::new();
    for edge in &blame.cycle {
        if let Some(ann) = &edge.ann {
            for citation in ann_citations(ann, interner) {
                if !citations.contains(&citation) {
                    citations.push(citation);
                }
            }
        }
    }

    vec![
        WorldFinding::error(
            "continuity.chronology-cycle",
            format!(
                "the chronology contradicts itself: an ordering cycle over {} edge(s) — {}",
                blame.cycle.len(),
                loop_render.join(", ")
            ),
        )
        .with_citations(citations),
    ]
}

/// The name of a time point, or a stable `#index` fallback.
fn point_name(time_points: &Interner, point: dli_time::chronology::TimePoint) -> String {
    time_points
        .resolve(Atom(point.0))
        .map_or_else(|| format!("#{}", point.0), ToOwned::to_owned)
}

/// The decision variables an ordering ruling's support rests on, resolved to
/// their citation names (`d-nnnn`) through the event-variable interner.
fn ann_citations(ann: &Ann, interner: &Interner) -> Vec<String> {
    let mut out = Vec::new();
    for monomial in ann.support.monomials() {
        for lit in monomial.lits() {
            if let Some(name) = interner.resolve(lit.atom)
                && !out.contains(&name.to_owned())
            {
                out.push(name.to_owned());
            }
        }
    }
    out
}

/// The claim-relative continuity battery over a set of asserted claims — the
/// structural classes the fact-checker's verdict does not itself report.
///
/// Emits `continuity.unknown-entity` for a claim whose subject the world does not
/// know, `continuity.proposed-as-fact` for a claim that states shadow (possible-
/// only) lore as settled, and `continuity.intra-episode-contradiction` for a claim
/// set that sets one single-valued slot two ways. Findings are sorted by
/// `(code, message)`.
#[must_use]
pub fn check_claims(world: &CompiledWorld, claims: &[AtomExpr]) -> Vec<WorldFinding> {
    let mut out = Vec::new();
    out.extend(unknown_claim_entities(world, claims));
    out.extend(proposed_as_fact(world, claims));
    out.extend(intra_contradictions(world, claims));
    out.sort_by(|a, b| (&a.code, &a.message).cmp(&(&b.code, &b.message)));
    out
}

/// The prior-episode contradiction class (`continuity.prior-contradiction`): a
/// claim whose value reverses the last value a prior episode established for the
/// same slot.
///
/// The prior ledger is `slot -> last certain value`, accumulated by the episode
/// fold. [`check_draft`](crate::check_draft) has no cross-episode ledger in a
/// single payload, so it drives this with an empty ledger today; the season
/// battery (`PAY-2`) supplies the real fold. Kept as a first-class function so the
/// class exists and is exercised now.
#[must_use]
pub fn prior_contradictions(
    claims: &[AtomExpr],
    prior: &BTreeMap<(String, String), String>,
) -> Vec<WorldFinding> {
    let mut out = Vec::new();
    for claim in claims {
        let Some((slot, value)) = slot_of(claim) else {
            continue;
        };
        if let Some(prior_value) = prior.get(&slot)
            && prior_value != &value
        {
            out.push(WorldFinding::error(
                "continuity.prior-contradiction",
                format!(
                    "re-establishes {}.{} = {value}, reversing the prior value {prior_value}",
                    slot.0, slot.1
                ),
            ));
        }
    }
    out.sort_by(|a, b| (&a.code, &a.message).cmp(&(&b.code, &b.message)));
    out
}

// --- world-internal classes --------------------------------------------------

/// `continuity.rejected-lore` for every certain fact whose rendered value or
/// subject realizes a killed token.
fn certain_rejected_lore(world: &CompiledWorld) -> Vec<WorldFinding> {
    let mut out = Vec::new();
    for atom in world.model.atoms() {
        if atom_status(world, atom) != Status::Certain {
            continue;
        }
        for token in atom
            .atom
            .args
            .iter()
            .filter_map(term_symbol)
            .chain(std::iter::once(atom.atom.pred.text.as_str()))
        {
            if let Some(hit) = world.killed.hit(token) {
                out.push(
                    WorldFinding::error(
                        "continuity.rejected-lore",
                        format!(
                            "certain fact '{}' realizes killed token '{}': {}",
                            atom.atom.pred.text, hit.token, hit.scope
                        ),
                    )
                    .with_citations(vec![hit.decision.clone()]),
                );
                break;
            }
        }
    }
    out
}

// --- claim-relative classes --------------------------------------------------

/// `continuity.unknown-entity` for a claim whose subject the world does not know
/// at all (not declared, not even observed).
fn unknown_claim_entities(world: &CompiledWorld, claims: &[AtomExpr]) -> Vec<WorldFinding> {
    let known: BTreeSet<&str> = world
        .model
        .entities()
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for claim in claims {
        if let Some(subject) = subject_of(claim)
            && !known.contains(subject.as_str())
            && seen.insert(subject.clone())
        {
            out.push(WorldFinding::error(
                "continuity.unknown-entity",
                format!("claim references entity '{subject}', which canon does not define"),
            ));
        }
    }
    out
}

/// `continuity.proposed-as-fact` for a claim that states as settled a slot whose
/// only canon basis is a possible-but-not-certain (shadow) fact.
fn proposed_as_fact(world: &CompiledWorld, claims: &[AtomExpr]) -> Vec<WorldFinding> {
    let mut out = Vec::new();
    for claim in claims {
        let Some((slot, _value)) = slot_of(claim) else {
            continue;
        };
        let certain = certain_values(world, &slot);
        if !certain.is_empty() {
            continue; // a certain basis exists; this is a supported/contradicted case
        }
        if slot_has_possible(world, &slot) {
            out.push(WorldFinding::error(
                "continuity.proposed-as-fact",
                format!(
                    "asserts {}.{} as settled, but it rests on proposed (shadow) lore",
                    slot.0, slot.1
                ),
            ));
        }
    }
    out
}

/// `continuity.intra-episode-contradiction` where the claim set assigns one
/// *single-valued* slot more than one value.
///
/// Single-valuedness is read from canon exactly as the fact-checker reads it
/// (spec 02 §4): a slot canon pins to one certain value is a functional
/// dependency, so two draft values clash; a slot canon holds several certain
/// values for is a multi-valued relation (a place with two neighbours), and two
/// draft values are just two edges — not a contradiction (R2/B). When canon is
/// silent on the slot, the predicate counts as single-valued only if canon proves
/// it functional everywhere it appears; a predicate canon never records leaves the
/// slot unprovable and is not flagged, so novel multi-valued lore never
/// false-positives.
fn intra_contradictions(world: &CompiledWorld, claims: &[AtomExpr]) -> Vec<WorldFinding> {
    let mut by_slot: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for claim in claims {
        if let Some((slot, value)) = slot_of(claim) {
            by_slot.entry(slot).or_default().insert(value);
        }
    }
    by_slot
        .into_iter()
        .filter(|(slot, values)| values.len() > 1 && slot_is_single_valued(world, slot))
        .map(|(slot, values)| {
            let rendered: Vec<&str> = values.iter().map(String::as_str).collect();
            WorldFinding::error(
                "continuity.intra-episode-contradiction",
                format!(
                    "sets single-valued slot {}.{} to multiple values [{}] in one draft",
                    slot.0,
                    slot.1,
                    rendered.join(", ")
                ),
            )
        })
        .collect()
}

/// Whether canon holds a `(pred, subject)` slot to be single-valued: exactly one
/// certain value here, or — when canon is silent here — the predicate is
/// functional everywhere it appears. Mirrors the fact-checker's data-driven
/// single-valuedness.
fn slot_is_single_valued(world: &CompiledWorld, slot: &(String, String)) -> bool {
    match certain_values(world, slot).len() {
        1 => true,
        0 => predicate_is_functional(world, &slot.0),
        _ => false,
    }
}

/// Whether the predicate is functional across canon: it appears in at least one
/// certain fact, and no subject carries more than one certain value for it.
fn predicate_is_functional(world: &CompiledWorld, pred: &str) -> bool {
    let mut by_subject: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for atom in world.model.atoms() {
        if atom_status(world, atom) != Status::Certain {
            continue;
        }
        if let Some(((atom_pred, subject), value)) = slot_of(&atom.atom)
            && atom_pred == *pred
        {
            by_subject.entry(subject).or_default().insert(value);
        }
    }
    !by_subject.is_empty() && by_subject.values().all(|values| values.len() <= 1)
}

// --- shared helpers ----------------------------------------------------------

/// The status of a loaded atom under the compiled world's ledger valuation.
pub(super) fn atom_status(world: &CompiledWorld, atom: &LoadedAtom) -> Status {
    atom.ann
        .status(&world.model.world.valuation, &world.model.world.theory)
}

/// The distinct certain values a compiled world holds for a `(pred, subject)`
/// slot.
pub(super) fn certain_values(world: &CompiledWorld, slot: &(String, String)) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for atom in world.model.atoms() {
        if atom_status(world, atom) != Status::Certain {
            continue;
        }
        if let Some((atom_slot, value)) = slot_of(&atom.atom)
            && &atom_slot == slot
        {
            out.insert(value);
        }
    }
    out
}

/// Whether the compiled world holds any *possible-but-not-certain* fact for a
/// slot (the shadow layer).
fn slot_has_possible(world: &CompiledWorld, slot: &(String, String)) -> bool {
    world.model.atoms().iter().any(|atom| {
        atom_status(world, atom) == Status::Possible
            && slot_of(&atom.atom).is_some_and(|(atom_slot, _)| &atom_slot == slot)
    })
}

/// The citations backing the certain fact(s) for a slot, deduplicated in order.
pub(super) fn slot_citations(world: &CompiledWorld, slot: &(String, String)) -> Vec<String> {
    let mut out = Vec::new();
    for atom in world.model.atoms() {
        if atom_status(world, atom) != Status::Certain {
            continue;
        }
        if slot_of(&atom.atom).is_some_and(|(atom_slot, _)| &atom_slot == slot) {
            for cite in &atom.citations {
                if !out.contains(cite) {
                    out.push(cite.clone());
                }
            }
        }
    }
    out
}

/// A binary atom's slot `(pred, subject)` and its value (arg1). `None` for a
/// non-binary atom or one whose subject is not a symbol.
pub(super) fn slot_of(atom: &AtomExpr) -> Option<((String, String), String)> {
    if atom.args.len() != 2 {
        return None;
    }
    let subject = term_symbol(&atom.args[0])?.to_owned();
    let value = render_term(&atom.args[1]);
    Some(((atom.pred.text.clone(), subject), value))
}

/// The subject (arg0) of an atom, if it is a symbol.
fn subject_of(atom: &AtomExpr) -> Option<String> {
    atom.args.first().and_then(term_symbol).map(str::to_owned)
}

/// The symbol text of a term, or `None` if it is not a bare symbol.
fn term_symbol(term: &Term) -> Option<&str> {
    match term {
        Term::Sym(id) => Some(id.text.as_str()),
        _ => None,
    }
}

/// Render a term to its ground string form (mirrors the loader's rendering).
pub(super) fn render_term(term: &Term) -> String {
    match term {
        Term::Var(id) | Term::Sym(id) => id.text.clone(),
        Term::Str(text) => text.clone(),
        Term::Int(value) => value.to_string(),
        Term::Time(TimeRef::Era(id)) => format!("era:{}", id.text),
        Term::Time(TimeRef::Event(id)) => format!("event:{}", id.text),
    }
}
