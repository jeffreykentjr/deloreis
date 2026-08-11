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
//! Two later world-internal classes join the battery in [`run`]:
//! `continuity.fluent-contradiction` (DL-0071) for interval-overlap clashes on a
//! fluent timeline, and `continuity.fd-violation` (DL-0073) for canon's own
//! certain layer holding two distinct values for a subject on a predicate the
//! author declared `functional`.
//!
//! One of the original six is world-internal (it needs no external delta) and
//! runs in [`run`] alongside the loader's fail-closed findings: `rejected-lore`
//! over the certain projection. The other classes are claim-relative and run in the claim
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
use dli_time::chronology::TimePoint;
use dli_time::fluent::FluentTimeline;

use super::{CompiledWorld, WorldFinding, fd_value_key};
use crate::model::{LoadFinding, LoadedAtom};

/// The world-internal continuity battery: the classes that check the compiled
/// world against itself, needing no external draft.
///
/// Surfaces the loader's fail-closed [`LoadFinding`]s (provenance, uniqueness) and
/// the evaluator's lowering findings (a `citation.unknown-decision` forged-authority
/// claim, `B1`) as [`WorldFinding`]s, and emits `continuity.rejected-lore` for
/// every certain fact that realizes a killed token, `continuity.chronology-cycle`
/// for a precedence loop, `continuity.fluent-contradiction` for an interval
/// overlap (DL-0071), and `continuity.fd-violation` for a declared-`functional`
/// slot canon holds two ways (DL-0073). Findings are sorted by `(code, message)`.
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
    out.extend(fluent_contradictions(world));
    out.extend(fd_violations(world));
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
        // Compare on the FD value key, so cosmetic drift (`gold` vs `Gold`) is
        // not read as reversing the prior value — the same key every other
        // comparison site uses (issue 49, sub-claim 1).
        if let Some(prior_value) = prior.get(&slot)
            && fd_value_key(prior_value) != fd_value_key(&value)
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
        // The `killed` registry predicate embeds banned tokens as its value
        // (DL-0063, refined by DL-0064); the token is the fact's subject matter,
        // not a realized usage, so it is exempt from the killed-token scan here.
        if atom.atom.pred.text == "killed" {
            continue;
        }
        let hits = atom
            .atom
            .args
            .iter()
            .filter_map(|term| match term {
                Term::Sym(id) => Some(id.text.as_str()),
                Term::Str(text) => Some(text.as_str()),
                Term::Var(_) | Term::Int(_) | Term::Time(_) => None,
            })
            .chain(std::iter::once(atom.atom.pred.text.as_str()))
            .flat_map(|token| world.killed.hits(token))
            .collect::<Vec<_>>();
        if hits.is_empty() {
            continue;
        }
        let citations = hits
            .iter()
            .map(|hit| hit.decision.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let rejected = hits
            .iter()
            .map(|hit| format!("'{}' ({}): {}", hit.token, hit.decision, hit.scope))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("; ");
        out.push(
            WorldFinding::error(
                "continuity.rejected-lore",
                format!(
                    "certain fact '{}' realizes killed token ruling(s): {rejected}",
                    atom.atom.pred.text
                ),
            )
            .with_citations(citations),
        );
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
    // Group each slot's claimed values by their normalized key, keeping the first
    // raw spelling for the message. Distinctness is judged on the key, so `gold`
    // and `Gold` are one value, not a phantom two (issue 49, sub-claim 1).
    let mut by_slot: BTreeMap<(String, String), BTreeMap<String, String>> = BTreeMap::new();
    for claim in claims {
        if let Some((slot, value)) = slot_of(claim) {
            by_slot
                .entry(slot)
                .or_default()
                .entry(fd_value_key(&value))
                .or_insert(value);
        }
    }
    by_slot
        .into_iter()
        // A fluent slot's contradictions are interval-sensitive: two values over
        // disjoint spans (`commons until e` / `enclosed since e`) are not a clash.
        // Those slots are judged by `fluent_contradictions` over their timeline;
        // the flat single-valued reader would false-fire, so skip them here.
        .filter(|(slot, values)| {
            values.len() > 1
                && !world.model.fluents.contains_key(slot)
                && slot_is_single_valued(world, slot)
        })
        .map(|(slot, values)| {
            let rendered: Vec<&str> = values.values().map(String::as_str).collect();
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

/// Whether a `(pred, subject)` slot is single-valued: its predicate is declared
/// `functional`, or exactly one certain value holds here, or — when canon is
/// silent here — the predicate is functional everywhere it appears. Mirrors the
/// fact-checker's data-driven single-valuedness, plus the author-declared
/// override that lets a *first-seen* predicate be judged (issue 49, sub-claim 4).
fn slot_is_single_valued(world: &CompiledWorld, slot: &(String, String)) -> bool {
    if world.model.functional_predicates.contains(&slot.0) {
        return true;
    }
    match certain_value_keys(world, slot).len() {
        1 => true,
        0 => predicate_is_functional(world, &slot.0),
        _ => false,
    }
}

/// Whether the predicate is functional: it is declared `functional`, or it
/// appears in at least one certain fact and no subject carries more than one
/// certain value for it. Distinctness is judged on the normalized value key.
fn predicate_is_functional(world: &CompiledWorld, pred: &str) -> bool {
    if world.model.functional_predicates.contains(pred) {
        return true;
    }
    let mut by_subject: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for atom in world.model.atoms() {
        if atom_status(world, atom) != Status::Certain {
            continue;
        }
        if let Some(((atom_pred, subject), value)) = slot_of(&atom.atom)
            && atom_pred == *pred
        {
            by_subject
                .entry(subject)
                .or_default()
                .insert(fd_value_key(&value));
        }
    }
    !by_subject.is_empty() && by_subject.values().all(|values| values.len() <= 1)
}

// --- fluent (interval) reasoning ---------------------------------------------

/// `continuity.fluent-contradiction` for every slot whose fluent timeline holds
/// two distinct values *certainly at once over overlapping intervals*. This is
/// the live path for `dli-time`'s interval-overlap reasoning (issue 48): a value
/// in force over disjoint spans (`commons until e`, `enclosed since e`) is one
/// truthful fluent and raises nothing; two certain values whose intervals
/// overlap is a genuine contradiction and raises exactly one finding. Slots with
/// no timeline are not fluents and are untouched here.
fn fluent_contradictions(world: &CompiledWorld) -> Vec<WorldFinding> {
    let mut out = Vec::new();
    for ((pred, subject), timeline) in &world.model.fluents {
        if let Some(values) = fluent_clash(world, timeline) {
            let slot = (pred.clone(), subject.clone());
            out.push(
                WorldFinding::error(
                    "continuity.fluent-contradiction",
                    format!(
                        "the fluent {subject}.{pred} holds distinct values [{}] at once over overlapping spans; one interval must give",
                        values.join(", ")
                    ),
                )
                .with_citations(slot_citations(world, &slot)),
            );
        }
    }
    out
}

/// The distinct values that hold certainly at a single point of a fluent's
/// timeline, when two or more do — an interval-overlap contradiction — else
/// `None`.
///
/// The timeline is projected (`FluentTimeline::state_at`) at **every declared
/// time point**, because a certain overlap need not be witnessed at either
/// span's own anchor: two `until e` values collide over the common *past* before
/// `e` (an open lower bound, so no anchor there), and two `since` values whose
/// starts are unordered collide only at a shared *downstream* event that is
/// neither span's `since`. Every anchor is itself an interned time point, so the
/// full table subsumes the anchors; a sentinel point covers the degenerate case
/// of a world with no declared points but two beginningless, endless spans.
/// A point where the two anchors are merely unordered projects to
/// `OrderIndeterminate`, not certain, so sweeping widely raises no false
/// positive.
fn fluent_clash(world: &CompiledWorld, timeline: &FluentTimeline<String>) -> Option<Vec<String>> {
    let chronology = &world.model.world.chronology;
    let valuation = &world.model.world.valuation;
    let theory = &world.model.world.theory;
    let point_count = u32::try_from(world.model.world.time_points.len()).unwrap_or(u32::MAX);
    let candidates: Vec<TimePoint> = if point_count == 0 {
        vec![TimePoint(u32::MAX)]
    } else {
        (0..point_count).map(TimePoint).collect()
    };
    for at in candidates {
        let state = timeline.state_at(chronology, valuation, theory, at);
        if state.has_contradiction() {
            let mut values: Vec<String> = state.certain_values().into_iter().cloned().collect();
            values.sort();
            values.dedup();
            return Some(values);
        }
    }
    None
}

// --- functional-dependency reasoning -----------------------------------------

/// `continuity.fd-violation` for a slot whose own certain projection holds two or
/// more distinct values on a *functional* predicate — a functional-dependency
/// break already sitting in canon (issue 49, sub-claim 3). The read-side twin of
/// the merge engine's `certain_conflicts` (`dli-eval` `fork.rs`), so canon that
/// would be rejected on a merge is not silently accepted on a load.
///
/// A predicate counts as functional when the author declared it `functional`, or
/// — undeclared — when it is functional among its *other* subjects: every subject
/// but this one carries at most one value, and at least one other subject exists
/// to witness it. A first-seen predicate with a single two-valued subject is
/// *not* flagged: canon alone cannot tell a functional slip from a genuine
/// multi-valued relation, which is exactly what the `functional` declaration is
/// for (sub-claim 4).
///
/// Fluent slots are interval-sensitive and judged by `fluent_contradictions`; a
/// value held over disjoint spans is not a clash, so they are skipped here.
/// Distinctness is judged on the normalized value key (sub-claim 1).
fn fd_violations(world: &CompiledWorld) -> Vec<WorldFinding> {
    // pred -> subject -> (normalized key -> first raw spelling).
    let mut by_pred: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>> = BTreeMap::new();
    for atom in world.model.atoms() {
        if atom_status(world, atom) != Status::Certain {
            continue;
        }
        let Some(((pred, subject), value)) = slot_of(&atom.atom) else {
            continue;
        };
        if world
            .model
            .fluents
            .contains_key(&(pred.clone(), subject.clone()))
        {
            continue;
        }
        by_pred
            .entry(pred)
            .or_default()
            .entry(subject)
            .or_default()
            .entry(fd_value_key(&value))
            .or_insert(value);
    }
    let mut out = Vec::new();
    for (pred, subjects) in &by_pred {
        // Only a predicate the author *declared* `functional` is checked. Canon's
        // shape cannot settle whether a relation is single- or multi-valued: a
        // subject with several edges among others with one each is a hub in a
        // many-to-many relation (a place with several neighbours, R2/B) just as
        // readily as a functional-dependency break. So the author rules on it
        // (koan 3, DL-0074) and the engine never guesses from the data — an
        // undeclared predicate raises nothing here, no matter its degree.
        if !world.model.functional_predicates.contains(pred) {
            continue;
        }
        for (subject, values) in subjects {
            if values.len() < 2 {
                continue;
            }
            let slot = (pred.clone(), subject.clone());
            let rendered: Vec<&str> = values.values().map(String::as_str).collect();
            out.push(
                WorldFinding::error(
                    "continuity.fd-violation",
                    format!(
                        "canon breaks the functional dependency on '{pred}': {subject} carries {} distinct certain values [{}]",
                        rendered.len(),
                        rendered.join(", ")
                    ),
                )
                .with_citations(slot_citations(world, &slot)),
            );
        }
    }
    out
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

/// The certain values a slot holds, keyed by their normalized form — so `gold`
/// and `Gold` count once. Cardinality here is what the FD check reads: two keys
/// is two genuinely distinct values (issue 49, sub-claim 1).
fn certain_value_keys(world: &CompiledWorld, slot: &(String, String)) -> BTreeSet<String> {
    certain_values(world, slot)
        .iter()
        .map(|value| fd_value_key(value))
        .collect()
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
