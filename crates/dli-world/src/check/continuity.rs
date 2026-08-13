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
//! Three later world-internal classes join the battery in [`run`]:
//! `continuity.fluent-contradiction` (DL-0071) for interval-overlap clashes on a
//! fluent timeline, `continuity.fd-violation` (DL-0073) for canon's own
//! certain layer holding two distinct values for a subject on a predicate the
//! author declared `functional`, and `continuity.derived-fd-violation` (WS-2,
//! amending DL-0075) for the same break appearing only once a wire-authored
//! rule's own derivations are saturated in.
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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::term::{AtomExpr, Term, TimeRef};
use dli_core::ann::{Ann, Status};
use dli_core::atom::{Atom, Interner};
use dli_core::fd::{fd_violations as fd_violations_over, inferred_functional};
use dli_time::chronology::{Chronology, TemporalVerdict, TimePoint};
use dli_time::fluent::{FluentTimeline, Projection, TemporalCover};

use super::{CompiledWorld, WorldFinding, fd_value_key};
use crate::model::{LoadFinding, LoadedAtom};

/// The world-internal continuity battery: the classes that check the compiled
/// world against itself, needing no external draft.
///
/// Surfaces the loader's fail-closed [`LoadFinding`]s (provenance, uniqueness) and
/// the evaluator's lowering findings (a `citation.unknown-decision` forged-authority
/// claim, `B1`) as [`WorldFinding`]s, and emits `continuity.rejected-lore` for
/// every certain fact that realizes a killed token, `continuity.chronology-cycle`
/// for a precedence loop, `continuity.identity-temporal-unsupported` for a
/// `same-as` naming a declared era/event (WS-1, v1 boundary),
/// `continuity.fluent-contradiction` for an interval overlap (DL-0071),
/// `continuity.fd-violation` for a declared-`functional` slot canon holds two
/// ways (DL-0073), and `continuity.derived-fd-violation` for the same break
/// visible only once rules are saturated (WS-2). Findings are sorted by
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
    // The identity-closure canon map is built ONCE per battery run, from the
    // world's certain `same-as` links (WS-1) — every reader below
    // canonicalizes through it rather than re-deriving the closure.
    let canon = identity_canon(world);
    out.extend(certain_rejected_lore(world));
    out.extend(chronology_cycle(world));
    out.extend(identity_temporal_unsupported(world));
    out.extend(fluent_contradictions(world, &canon));
    out.extend(inertia_unanchored(world, &canon));
    out.extend(fd_violations(world, &canon));
    out.extend(derived_fd_violations(world, &canon));
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

// --- identity closure (WS-1) --------------------------------------------------

/// `continuity.identity-temporal-unsupported` for every `same-as` link naming a
/// declared era or event on either side — the v1 temporal boundary. Point
/// unification is structural (interned chronology indices): a defeasible merge
/// over time points is impossible without half-merging (derived `before`
/// copies would exist while chronology, `as-of`, and timelines would not see
/// them), and a non-defeasible merge would contradict the identity feature's
/// own defeasibility. Fires regardless of the link's own standing — even a
/// merely *proposed* `same-as` over events would half-merge silently if later
/// promoted, so the refusal is loud at declaration time (koan 4), not deferred
/// until promotion. The stage-time counterpart (`identity.temporal-unsupported`)
/// belongs to the wire op, PR-1b; this catches every path, including a
/// text-authored corpus.
fn identity_temporal_unsupported(world: &CompiledWorld) -> Vec<WorldFinding> {
    let sorts: BTreeMap<&str, &str> = world
        .model
        .entities()
        .iter()
        .filter_map(|e| e.sort.as_deref().map(|sort| (e.name.as_str(), sort)))
        .collect();
    let mut out = Vec::new();
    for link in declared_links(world) {
        for name in [link.left, link.right] {
            let Some(sort @ ("era" | "event")) = sorts.get(name).copied() else {
                continue;
            };
            out.push(
                WorldFinding::error(
                    "continuity.identity-temporal-unsupported",
                    format!(
                        "same-as {} = {} names {sort} '{name}'; identity over eras/events is not supported (v1 boundary) — point unification would half-merge the chronology",
                        link.left, link.right
                    ),
                )
                .with_citations(link.citations.clone()),
            );
            break; // one finding per link, even when both endpoints are temporal
        }
    }
    out
}

/// One declared identity link (WS-1): the pair and its compiled annotation
/// from the eval-layer declaration channel (`World::same_as_links` —
/// [`World::assert_same_as`]'s intake, what drives closure and status),
/// paired with its rendered citations, a load-time-only concern the eval
/// layer does not carry.
struct DeclaredLink<'a> {
    left: &'a str,
    right: &'a str,
    ann: &'a Ann,
    citations: Vec<String>,
}

/// Every declared identity link, in channel order. This — never a scan of
/// `world.model.atoms()` by predicate name — is the one place a check reads
/// "which `same-as` pairs are actual declarations": a plain `fact same-as(x,
/// y)` never appears in `world.same_as_links` (only `Decl::SameAs` populates
/// it, via [`World::assert_same_as`]), so it is invisible here even though
/// its atom shares the predicate name (DL-0078, koan 3). Citations are
/// recovered by an exact-pair cross-reference against the loaded-atom stream
/// (both are populated by the same `Decl::SameAs` lowering call, so the
/// lookup always finds its match); a miss degrades to no citations rather
/// than panicking.
fn declared_links(world: &CompiledWorld) -> Vec<DeclaredLink<'_>> {
    world
        .model
        .world
        .same_as_links
        .iter()
        .map(|(left, right, ann)| {
            let citations = world
                .model
                .atoms()
                .iter()
                .find(|atom| {
                    atom.atom.pred.text == "same-as"
                        && atom.atom.args.len() == 2
                        && term_symbol(&atom.atom.args[0]) == Some(left.as_str())
                        && term_symbol(&atom.atom.args[1]) == Some(right.as_str())
                })
                .map(|atom| atom.citations.clone())
                .unwrap_or_default();
            DeclaredLink {
                left,
                right,
                ann,
                citations,
            }
        })
        .collect()
}

/// Build the identity-closure canon map from the compiled world's certain
/// DECLARED `same-as` links (WS-1): union-find over pairs whose link is
/// [`Status::Certain`] under the world's valuation and theory — consistent
/// with the derived layer (a synthesized-rule copy is certain iff its
/// underlying link chain is). Built ONCE per battery run by
/// [`run`]/[`check_claims`]/`factcheck::factcheck` and threaded to every
/// canon-aware reader below, so a world with zero `same-as` declarations gets
/// an empty map and every [`dli_core::alias::canon`] call is the identity —
/// byte-identical to a world with none of this machinery (the
/// frozen-contract guard).
pub fn identity_canon(world: &CompiledWorld) -> BTreeMap<String, String> {
    let pairs = declared_links(world)
        .into_iter()
        .filter(|link| {
            link.ann
                .status(&world.model.world.valuation, &world.model.world.theory)
                == Status::Certain
        })
        .map(|link| (link.left.to_owned(), link.right.to_owned()));
    dli_core::alias::canon_map(pairs)
}

/// Canonicalize a `(pred, subject)` slot's subject through the identity canon
/// map. The predicate side never canonicalizes — `same-as` merges bearers, not
/// relations.
fn canon_slot(canon: &BTreeMap<String, String>, slot: &(String, String)) -> (String, String) {
    (
        slot.0.clone(),
        dli_core::alias::canon(canon, &slot.1).to_owned(),
    )
}

/// Whether ANY raw `(pred, subject)` key of the world's fluent-timeline table
/// canonicalizes into `slot` (already canonical) — the "fluent-skip consults
/// any class member" rule: an identity-merged class is interval-sensitive as a
/// whole the moment any one of its members is, not only when the literal
/// canonical spelling happens to carry a timeline.
pub(super) fn fluent_class_member(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    slot: &(String, String),
) -> bool {
    world
        .model
        .fluents
        .keys()
        .any(|raw| &canon_slot(canon, raw) == slot)
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
    let canon = identity_canon(world);
    out.extend(unknown_claim_entities(world, claims));
    out.extend(proposed_as_fact(world, &canon, claims));
    out.extend(intra_contradictions(world, &canon, claims));
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
fn proposed_as_fact(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    claims: &[AtomExpr],
) -> Vec<WorldFinding> {
    let mut out = Vec::new();
    for claim in claims {
        let Some((slot, _value)) = slot_of(claim) else {
            continue;
        };
        let certain = certain_values(world, canon, &slot);
        if !certain.is_empty() {
            continue; // a certain basis exists; this is a supported/contradicted case
        }
        if slot_has_possible(world, canon, &slot) {
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
///
/// Grouping is by **canon slot** (WS-1): two claims about the same predicate on
/// two names a certain `same-as` merges are one slot, not two — `sibling(x, a)`
/// and `sibling(y, b)` clash under `x = y` exactly as `sibling(x, a)` and
/// `sibling(x, b)` would (two DIFFERENT values on one merged slot — `sibling(x,
/// a)` and `sibling(y, a)` would instead be the SAME value restated, and would
/// not clash). The claimed *value* is canonicalized too, so `spouse(x, a)` and
/// `spouse(x, b)` under a certain `a = b` are one value, not an intra-episode
/// break.
fn intra_contradictions(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    claims: &[AtomExpr],
) -> Vec<WorldFinding> {
    // Group each slot's claimed values by their normalized key, keeping the first
    // raw spelling for the message. Distinctness is judged on the key, so `gold`
    // and `Gold` are one value, not a phantom two (issue 49, sub-claim 1).
    let mut by_slot: BTreeMap<(String, String), BTreeMap<String, String>> = BTreeMap::new();
    for claim in claims {
        if let Some((slot, value)) = slot_of(claim) {
            let slot = canon_slot(canon, &slot);
            let value = dli_core::alias::canon(canon, &value).to_owned();
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
        // the flat single-valued reader would false-fire, so skip them here —
        // consulting every class member, not only the literal canonical slot.
        .filter(|(slot, values)| {
            values.len() > 1
                && !fluent_class_member(world, canon, slot)
                && slot_is_single_valued(world, canon, slot)
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
fn slot_is_single_valued(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    slot: &(String, String),
) -> bool {
    if world.model.functional_predicates.contains(&slot.0) {
        return true;
    }
    match certain_value_keys(world, canon, slot).len() {
        1 => true,
        0 => predicate_is_functional(world, canon, &slot.0),
        _ => false,
    }
}

/// Whether the predicate is functional: it is declared `functional`, or it
/// appears in at least one certain fact and no subject carries more than one
/// certain value for it. The shape inference is the shared
/// [`inferred_functional`] primitive, so it cannot drift from the fork- and
/// merge-side checks that also union declared functionality with it.
fn predicate_is_functional(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    pred: &str,
) -> bool {
    world.model.functional_predicates.contains(pred)
        || inferred_functional(certain_binary_slots(world, canon)).contains(pred)
}

// --- fluent (interval) reasoning ---------------------------------------------

/// `continuity.fluent-contradiction` for every slot whose fluent timeline holds
/// two distinct values *certainly at once over overlapping intervals*. This is
/// the live path for `dli-time`'s interval-overlap reasoning (issue 48): a value
/// in force over disjoint spans (`commons until e`, `enclosed since e`) is one
/// truthful fluent and raises nothing; two certain values whose intervals
/// overlap is a genuine contradiction and raises exactly one finding. Slots with
/// no timeline are not fluents and are untouched here.
///
/// **Identity-merged fluents (WS-1).** The world's raw `(pred, subject)`
/// timeline keys are grouped by their canon slot first. A singleton group (the
/// overwhelming common case, and the only case in a world with no `same-as`
/// declarations) is checked exactly as before — same timeline reference, same
/// result. A multi-member group — two or more raw slots a certain `same-as`
/// merges — is checked as ONE fluent: a scratch timeline concatenating every
/// member's spans (each span keeps its own value/anchors/`Ann`, so intervals
/// are preserved, not flattened) is swept once. This is what lets a merged
/// alias see a genuine overlap between spans authored under different names,
/// while two spans that were always about one bearer under one name behave
/// identically to today.
fn fluent_contradictions(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
) -> Vec<WorldFinding> {
    let mut by_canon: BTreeMap<(String, String), Vec<&(String, String)>> = BTreeMap::new();
    for raw_slot in world.model.fluents.keys() {
        by_canon
            .entry(canon_slot(canon, raw_slot))
            .or_default()
            .push(raw_slot);
    }
    let mut out = Vec::new();
    for (slot, members) in by_canon {
        let mut scratch: FluentTimeline<String> = FluentTimeline::new();
        for member in &members {
            for span in &world.model.fluents[*member].spans {
                scratch.push(span.clone());
            }
        }
        // A canon slot is inertial iff its (canon) predicate carries an
        // `inertial` declaration — the predicate is never aliased by canon
        // (only subjects are), so `slot.0` keys the map directly. The
        // declaration's own `Ann` decides whether capping is active (DL-0081);
        // `None` for every non-inertial slot keeps `state_at` verbatim.
        let inertial = world.model.inertial_predicates.get(&slot.0);
        if let Some((witness, values)) = fluent_clash(world, &scratch, inertial) {
            out.push(
                WorldFinding::error(
                    "continuity.fluent-contradiction",
                    format!(
                        "the fluent {}.{} holds distinct values [{}] at once over overlapping spans; one interval must give",
                        slot.1,
                        slot.0,
                        values.join(", ")
                    ),
                )
                .with_citations(slot_citations(world, canon, &slot))
                .with_data(fluent_finding_data(
                    world,
                    &slot,
                    &scratch,
                    &values,
                    &[witness],
                    inertial,
                )),
            );
        } else if let Some(finding) = fluent_order_gap(world, canon, &slot, &scratch, inertial) {
            out.push(finding);
        }
    }
    out
}

/// The distinct values that hold certainly at a single point of a fluent's
/// timeline, when two or more do — an interval-overlap contradiction — else
/// `None`. Returns the witnessing point alongside the values (WS-2's
/// `witness_points` payload field).
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
fn fluent_clash(
    world: &CompiledWorld,
    timeline: &FluentTimeline<String>,
    inertial: Option<&Ann>,
) -> Option<(TimePoint, Vec<String>)> {
    let chronology = &world.model.world.chronology;
    let valuation = &world.model.world.valuation;
    let theory = &world.model.world.theory;
    for at in fluent_sweep_points(world) {
        // An inertial slot derives its effective boundaries from the live
        // competitors at each point (DL-0081); every other slot projects
        // through the plain open-interval reader, byte-identically.
        let state = match inertial {
            Some(decl_ann) => {
                timeline.state_at_inertial(chronology, valuation, theory, at, decl_ann)
            }
            None => timeline.state_at(chronology, valuation, theory, at),
        };
        if state.has_contradiction() {
            let mut values: Vec<String> = state.certain_values().into_iter().cloned().collect();
            values.sort();
            values.dedup();
            return Some((at, values));
        }
    }
    None
}

/// `continuity.fluent-order-undeclared` (WS-4, warn, non-blocking — user pin
/// §2.9): fires when two distinct-valued, certain-support spans on a fluent
/// slot cannot be PROVEN disjoint — some consistent extension of the
/// partial order would make them genuinely co-hold at a real point — but
/// the order needed to decide it is undeclared.
///
/// **Pairwise, not point-swept.** An earlier design probed every declared
/// time point and flagged a span whenever its cover read
/// `OrderIndeterminate` there — but that tests a span against an
/// UNRELATED third point, not against the other span it might clash with,
/// and false-positives on exactly the common transition shape: `commons
/// until x` / `enclosed since x` reads order-indeterminate against some
/// OTHER declared event `y` (unordered vs `x`) even though `x`-until and
/// `x`-since are the SAME anchor and the two spans can never actually
/// overlap under any order at all (rt-features SEV-B; reproduced by both
/// `examples/camelot.dli`'s `excalibur.keeper` and
/// `examples/norse/nine-realms.dli`'s `sol.course`, which share this exact
/// shape). The fix compares each pair of spans to EACH OTHER: two spans
/// `[a.since, a.until)` and `[b.since, b.until)` are provably disjoint when
/// one's `until` anchor equals or precedes the other's `since` anchor
/// ([`provably_disjoint`]) — a shared anchor (`a.until == b.since`) is the
/// clean-transition case and is always disjoint, regardless of what else
/// the corpus declares. When neither ordering can be proven, the pair is a
/// genuine ambiguity, and the candidate repair pairs are exactly the
/// UNDECLARED cross-boundary anchor pairs between the two spans
/// ([`undeclared_boundary_pairs`]) — declaring any of them settles this
/// pair, either to disjoint or to a certain overlap
/// `continuity.fluent-contradiction` would then catch.
///
/// **Soundness is unchanged:** this never asserts a contradiction itself,
/// only names an undecided pair; [`fluent_clash`]'s sweep is what actually
/// fires on a certain overlap, and this function runs only when it found
/// none. Fires at most once per slot per read — the first ambiguous pair,
/// spans compared in declaration order.
fn fluent_order_gap(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    slot: &(String, String),
    timeline: &FluentTimeline<String>,
    inertial: Option<&Ann>,
) -> Option<WorldFinding> {
    let chronology = &world.model.world.chronology;
    let valuation = &world.model.world.valuation;
    let theory = &world.model.world.theory;
    let certain: Vec<&dli_time::fluent::Span<String>> = timeline
        .spans
        .iter()
        .filter(|span| span.ann.status(valuation, theory) == Status::Certain)
        .collect();
    for (i, a) in certain.iter().enumerate() {
        for b in certain.iter().skip(i + 1) {
            if a.value == b.value || provably_disjoint(a, b, chronology) {
                continue;
            }
            let candidates = undeclared_boundary_pairs(a, b, chronology);
            if candidates.is_empty() {
                // Not provably disjoint yet nothing left undeclared between
                // them: every relevant crossing is already resolved (in a
                // direction that does not disjoin them), which means a
                // certain overlap — `fluent_clash` already owns that report,
                // reached before this function ever runs. Kept as an
                // explicit guard rather than an assumption.
                continue;
            }
            let interner = &world.model.world.time_points;
            let candidate_names: Vec<[String; 2]> = candidates
                .iter()
                .map(|&(p, q)| [point_name(interner, p), point_name(interner, q)])
                .collect();
            let named_pairs: Vec<String> = candidate_names
                .iter()
                .map(|pair| format!("{} / {}", pair[0], pair[1]))
                .collect();
            let mut values = [a.value.clone(), b.value.clone()];
            values.sort();
            let witnesses: Vec<TimePoint> = [a.since, a.until, b.since, b.until]
                .into_iter()
                .flatten()
                .collect();
            let mut data =
                fluent_finding_data(world, slot, timeline, &values, &witnesses, inertial);
            if let serde_json::Value::Object(map) = &mut data {
                map.insert(
                    "repair".to_owned(),
                    serde_json::json!({ "kind": "declare-order", "candidates": candidate_names }),
                );
            }
            return Some(
                WorldFinding::warn(
                    "continuity.fluent-order-undeclared",
                    format!(
                        "the fluent {}.{} holds distinct values [{}] whose order is undeclared; a before-fact between {} would settle it",
                        slot.1,
                        slot.0,
                        values.join(", "),
                        named_pairs.join(", ")
                    ),
                )
                .with_citations(slot_citations(world, canon, slot))
                .with_data(data),
            );
        }
    }
    None
}

/// Whether two spans are PROVEN to never overlap under any consistent
/// extension of the partial order: one span's `until` anchor equals or
/// precedes the other's `since` anchor. An open end or open start (`None`)
/// can never be proven to bound the other side, so disjointness needs both
/// anchors present on at least one of the two crossings.
fn provably_disjoint(
    a: &dli_time::fluent::Span<String>,
    b: &dli_time::fluent::Span<String>,
    chronology: &Chronology,
) -> bool {
    boundary_at_or_before(a.until, b.since, chronology)
        || boundary_at_or_before(b.until, a.since, chronology)
}

/// Whether `end` is known to be at-or-before `start` — the same point (the
/// half-open-interval boundary case: `until x` excludes `x`, `since x`
/// includes it, so a SHARED anchor is exactly the disjoint clean
/// transition, not an ambiguity) or a proven `Before`. `None` on either
/// side answers `false`: an open boundary proves nothing.
fn boundary_at_or_before(
    end: Option<TimePoint>,
    start: Option<TimePoint>,
    chronology: &Chronology,
) -> bool {
    match (end, start) {
        (Some(end), Some(start)) => {
            end == start || matches!(chronology.precedes(end, start), TemporalVerdict::Before)
        }
        _ => false,
    }
}

/// The cross-boundary anchor pairs between two spans whose ORDER is
/// currently undeclared — every `(Some, Some)` combination of `{a.since,
/// a.until} × {b.since, b.until}` that is a real `Unordered` verdict. These
/// are exactly the pairs [`provably_disjoint`] would need resolved (one
/// way or the other) to settle whether the spans overlap.
fn undeclared_boundary_pairs(
    a: &dli_time::fluent::Span<String>,
    b: &dli_time::fluent::Span<String>,
    chronology: &Chronology,
) -> Vec<(TimePoint, TimePoint)> {
    [
        (a.since, b.since),
        (a.since, b.until),
        (a.until, b.since),
        (a.until, b.until),
    ]
    .into_iter()
    .filter_map(|(p, q)| {
        let (p, q) = (p?, q?);
        (p != q && matches!(chronology.precedes(p, q), TemporalVerdict::Unordered))
            .then_some((p, q))
    })
    .collect()
}

/// Every declared time point, or the degenerate single sentinel for a world
/// with none — the sweep both [`fluent_clash`] and [`fluent_order_gap`] run.
fn fluent_sweep_points(world: &CompiledWorld) -> Vec<TimePoint> {
    let point_count = u32::try_from(world.model.world.time_points.len()).unwrap_or(u32::MAX);
    if point_count == 0 {
        vec![TimePoint(u32::MAX)]
    } else {
        (0..point_count).map(TimePoint).collect()
    }
}

/// `{pred, subject, values, spans: [{value, since, until}], witness_points}`
/// — enough to reconstruct a fluent verdict without parsing the message
/// (WS-2's `Finding.data` rider, §2.9), shared by
/// `continuity.fluent-contradiction` and `continuity.fluent-order-undeclared`.
///
/// For an **inertial** slot whose declaration is certain (capping active,
/// DL-0081) the payload gains `"inertial": true` and each open value's `until`
/// is the **derived** boundary — the `since` of its certain defeater, or `null`
/// when the value persists unbounded — so a consumer reads the transition
/// structure the message describes without re-deriving it.
fn fluent_finding_data(
    world: &CompiledWorld,
    slot: &(String, String),
    timeline: &FluentTimeline<String>,
    values: &[String],
    witness_points: &[TimePoint],
    inertial: Option<&Ann>,
) -> serde_json::Value {
    let interner = &world.model.world.time_points;
    let chronology = &world.model.world.chronology;
    let valuation = &world.model.world.valuation;
    let theory = &world.model.world.theory;
    let capping = inertial.is_some_and(|ann| ann.status(valuation, theory) == Status::Certain);
    let spans: Vec<serde_json::Value> = timeline
        .spans
        .iter()
        .enumerate()
        .filter(|(_, span)| values.contains(&span.value))
        .map(|(index, span)| {
            // With capping active, an open (no authored `until`) value reports
            // its derived boundary instead of `null`.
            let until = if capping && span.until.is_none() && span.since.is_some() {
                inertial_derived_until(
                    timeline,
                    index,
                    span,
                    chronology,
                    valuation,
                    theory,
                    &world.model.world.interner,
                )
                .map(|defeater| defeater.until)
            } else {
                span.until
            };
            serde_json::json!({
                "value": span.value,
                "since": span.since.map(|p| point_name(interner, p)),
                "until": until.map(|p| point_name(interner, p)),
            })
        })
        .collect();
    let mut points: Vec<String> = witness_points
        .iter()
        .map(|p| point_name(interner, *p))
        .collect();
    points.sort();
    points.dedup();
    let mut data = serde_json::json!({
        "pred": slot.0,
        "subject": slot.1,
        "values": values,
        "spans": spans,
        "witness_points": points,
    });
    if inertial.is_some()
        && let serde_json::Value::Object(map) = &mut data
    {
        map.insert("inertial".to_owned(), serde_json::Value::Bool(true));
    }
    data
}

/// The certain, distinct competitor that caps one open inertial span `vᵢ` —
/// [`inertial_derived_until`]'s return, extended (`author.as_of`, contract
/// 1.9.0, DL-0082) to carry the defeater's own value and citations alongside
/// its `since`, so a consumer of the derived boundary can name what defeated
/// the span without a second lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InertialDefeater {
    /// The defeater's `since` — also the capped span's derived `until`.
    until: TimePoint,
    /// The defeater's own value.
    value: String,
    /// The defeater span's originating decision citations.
    citations: Vec<String>,
}

/// The **derived** upper boundary of one open inertial span `vᵢ` (`since = tᵢ`,
/// no authored `until`): the earliest certain, distinct competitor that `tᵢ`
/// certainly precedes, or `None` when the value persists unbounded. This
/// mirrors the certain-defeat arm of [`FluentTimeline::state_at_inertial`]'s
/// `inertial_cover`, so the reported boundary and the projected cover cannot
/// disagree. `slot_state_at` (`author.as_of`) is the other reader: it uses the
/// same search to populate `AsOfHolding.defeater`, so the read and the
/// `continuity.fluent-contradiction`/`continuity.fluent-order-undeclared`
/// finding data can never report two different boundaries for one span.
fn inertial_derived_until(
    timeline: &FluentTimeline<String>,
    index: usize,
    span: &dli_time::fluent::Span<String>,
    chronology: &Chronology,
    valuation: &dli_core::valuation::Valuation,
    theory: &dli_core::ann::Theory,
    interner: &Interner,
) -> Option<InertialDefeater> {
    let since = span.since?;
    let mut best: Option<(usize, TimePoint)> = None;
    for (other_index, other) in timeline.spans.iter().enumerate() {
        if other_index == index || other.value == span.value {
            continue;
        }
        if other.ann.status(valuation, theory) != Status::Certain {
            continue;
        }
        let Some(other_since) = other.since else {
            continue;
        };
        if !matches!(
            chronology.precedes(since, other_since),
            TemporalVerdict::Before
        ) {
            continue;
        }
        // Keep the earliest defeater — the transition happens at the first
        // successor `tᵢ` precedes.
        best = Some(match best {
            Some((current_index, current_since))
                if matches!(
                    chronology.precedes(current_since, other_since),
                    TemporalVerdict::Before
                ) =>
            {
                (current_index, current_since)
            }
            _ => (other_index, other_since),
        });
    }
    let (best_index, until) = best?;
    let defeater = &timeline.spans[best_index];
    Some(InertialDefeater {
        until,
        value: defeater.value.clone(),
        citations: ann_citations(&defeater.ann, interner),
    })
}

// --- author.as_of (contract 1.9.0, DL-0082) ----------------------------------

/// The kernel projection of one `(pred, subject)` fluent slot's state at a
/// query point — what `author.as_of` maps to its wire shape
/// (`AuthorAsOfResult`). Lives here, next to `fluent_contradictions`,
/// because it reuses the exact same canon-grouping (`identity_canon`,
/// `canon_slot`), the same `inertial_predicates` lookup, and the same
/// `state_at`/`state_at_inertial` dispatch the continuity check already
/// performs — a silent inertial transition becomes an observable read without
/// a second, independently-derived notion of "what holds here". This never
/// touches `dli_eval::world::World::query_with` or the saturated derived
/// layer: a `(pred, subject)` with no entry in `world.model.fluents` yields
/// empty `holdings`, with no fallback to the raw extensional facts
/// `author.query` still answers unmodified (the DL-0081/DL-0082 hard
/// invariant that this is the ONLY inertia-aware read).
#[must_use]
pub fn slot_state_at(
    world: &CompiledWorld,
    pred: &str,
    subject: &str,
    at: TimePoint,
) -> SlotStateAt {
    let canon = identity_canon(world);
    let slot = canon_slot(&canon, &(pred.to_owned(), subject.to_owned()));
    let chronology = &world.model.world.chronology;
    let valuation = &world.model.world.valuation;
    let theory = &world.model.world.theory;
    let citation_interner = &world.model.world.interner;

    // The same canon-grouped scratch timeline `fluent_contradictions` builds
    // (:620-635), restricted to this one slot: every raw `(pred, subject)`
    // fluent key whose canon slot matches, spans concatenated in declaration
    // order.
    let mut scratch: FluentTimeline<String> = FluentTimeline::new();
    for raw_slot in world.model.fluents.keys() {
        if canon_slot(&canon, raw_slot) != slot {
            continue;
        }
        for span in &world.model.fluents[raw_slot].spans {
            scratch.push(span.clone());
        }
    }

    let inertial = world.model.inertial_predicates.get(&slot.0);
    let capping = inertial.is_some_and(|ann| ann.status(valuation, theory) == Status::Certain);
    let inertial_citations = inertial
        .map(|ann| ann_citations(ann, citation_interner))
        .unwrap_or_default();

    // The exact dispatch `fluent_clash` uses: `state_at_inertial` when the
    // predicate carries a declaration (it reverts to plain `state_at`
    // internally when that declaration is not certain), `state_at` when it
    // never does. Never `query_with`, never the derived layer.
    let state = match inertial {
        Some(decl_ann) => scratch.state_at_inertial(chronology, valuation, theory, at, decl_ann),
        None => scratch.state_at(chronology, valuation, theory, at),
    };
    let contested = state.has_contradiction();

    // Pair each surviving holding back to the span that produced it — greedy,
    // by value and support standing, consuming each span at most once. The
    // inertia declaration exists precisely to keep at most one live certain
    // value per point, so this is unambiguous on every corpus the eight
    // `as_of.rs` cases exercise; a contrived corpus with two identically
    // valued/supported spans both covering the same point consumes the first
    // in declaration order, deterministically, rather than panicking.
    let mut used = vec![false; scratch.spans.len()];
    let holdings = state
        .holdings
        .iter()
        .map(|holding| {
            let origin = scratch.spans.iter().enumerate().find(|(index, span)| {
                !used[*index]
                    && span.value == holding.value
                    && span.ann.status(valuation, theory) == holding.support
            });
            let Some((index, span)) = origin else {
                return SlotHolding {
                    value: holding.value.clone(),
                    projection: holding.projection(),
                    temporal: holding.temporal,
                    support: holding.support,
                    since: None,
                    until: None,
                    citations: Vec::new(),
                    defeater: None,
                };
            };
            used[index] = true;
            let citations = ann_citations(&span.ann, citation_interner);
            // The reported `until` and defeater come STRICTLY from
            // `inertial_derived_until` — never recomputed independently — so
            // the boundary this read reports and the one
            // `continuity.fluent-contradiction`'s `Finding.data` reports can
            // never disagree.
            let (until, defeater) = if capping && span.until.is_none() && span.since.is_some() {
                match inertial_derived_until(
                    &scratch,
                    index,
                    span,
                    chronology,
                    valuation,
                    theory,
                    citation_interner,
                ) {
                    Some(d) => (
                        Some(d.until),
                        Some(SlotDefeater {
                            value: d.value,
                            since: Some(d.until),
                            citations: d.citations,
                        }),
                    ),
                    None => (None, None),
                }
            } else {
                (span.until, None)
            };
            SlotHolding {
                value: holding.value.clone(),
                projection: holding.projection(),
                temporal: holding.temporal,
                support: holding.support,
                since: span.since,
                until,
                citations,
                defeater,
            }
        })
        .collect();

    SlotStateAt {
        pred: slot.0,
        subject: slot.1,
        inertial: inertial.is_some(),
        capping,
        contested,
        inertial_citations,
        holdings,
    }
}

/// [`slot_state_at`]'s result: the point-in-time projection of one `(pred,
/// subject)` fluent slot, `subject` canonicalized through the identity map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotStateAt {
    /// The predicate (never canonicalized — identity merges bearers, not
    /// relations).
    pub pred: String,
    /// The subject, canonicalized through the identity map.
    pub subject: String,
    /// Whether `pred` carries an `inertial` declaration (any standing).
    pub inertial: bool,
    /// Whether that declaration is certain under the world's valuation, so
    /// the projection is actually inertia-shaped (`state_at_inertial`'s
    /// capping arm is live) rather than byte-identical to plain `state_at`.
    pub capping: bool,
    /// Whether two or more distinct values hold certainly at `at` at once —
    /// [`dli_time::fluent::FluentState::has_contradiction`] over the same
    /// projection this struct reports.
    pub contested: bool,
    /// The inertia declaration's own decision citations, empty when `pred`
    /// was never declared inertial.
    pub inertial_citations: Vec<String>,
    /// The values whose span could be in force at the query point, each with
    /// its factorial verdict and, when capped, the defeater that bounds it.
    pub holdings: Vec<SlotHolding>,
}

/// One [`SlotStateAt`] value's standing at the query point, with its temporal
/// anchors and, for a capped inertial value, the competitor that defeats it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotHolding {
    /// The value in force.
    pub value: String,
    /// The collapsed three-valued projection
    /// ([`dli_time::fluent::Holding::projection`]).
    pub projection: Projection,
    /// The temporal half: whether the span's interval covers the query point.
    pub temporal: TemporalCover,
    /// The support half: the anchoring ruling's standing.
    pub support: Status,
    /// The originating span's authored `since` anchor.
    pub since: Option<TimePoint>,
    /// The originating span's upper bound: its authored `until` when one
    /// exists, else — for a capped inertial value — the boundary
    /// `inertial_derived_until` derives; `None` means unbounded.
    pub until: Option<TimePoint>,
    /// The originating span's own decision citations.
    pub citations: Vec<String>,
    /// The certain, distinct competitor that caps this value, when the
    /// `until` above is a derived (not authored) boundary.
    pub defeater: Option<SlotDefeater>,
}

/// The certain, distinct competitor that defeats a capped [`SlotHolding`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotDefeater {
    /// The defeater's value.
    pub value: String,
    /// The defeater's `since` — the same point as the capped holding's
    /// derived `until`.
    pub since: Option<TimePoint>,
    /// The defeater span's own decision citations.
    pub citations: Vec<String>,
}

/// `continuity.inertia-unanchored` (warn, DL-0081, koan 4): an inertial
/// predicate's value with **no `since` anchor** cannot produce a persistence
/// interval, so the declaration would silently do nothing. Rather than drop it,
/// name the slot. Declaration-only: a predicate never declared `inertial` is
/// untouched (its open-start value is ordinary open-interval lore).
///
/// Grouped by canon slot exactly as [`fluent_contradictions`] is, so an
/// identity-merged class is judged as one fluent. Non-blocking (a warn), and
/// independent of the clash/contested sweep — an unanchored value can be the
/// *only* thing wrong on a slot, or ride alongside a genuine overlap.
fn inertia_unanchored(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
) -> Vec<WorldFinding> {
    let mut by_canon: BTreeMap<(String, String), Vec<&(String, String)>> = BTreeMap::new();
    for raw_slot in world.model.fluents.keys() {
        by_canon
            .entry(canon_slot(canon, raw_slot))
            .or_default()
            .push(raw_slot);
    }
    let mut out = Vec::new();
    for (slot, members) in by_canon {
        if !world.model.inertial_predicates.contains_key(&slot.0) {
            continue;
        }
        let mut values: Vec<String> = members
            .iter()
            .flat_map(|member| &world.model.fluents[*member].spans)
            .filter(|span| span.since.is_none())
            .map(|span| span.value.clone())
            .collect();
        if values.is_empty() {
            continue;
        }
        values.sort();
        values.dedup();
        out.push(
            WorldFinding::warn(
                "continuity.inertia-unanchored",
                format!(
                    "the inertial fluent {}.{} carries value(s) [{}] with no `since` anchor, so no persistence interval can be derived; give each a `since` point",
                    slot.1,
                    slot.0,
                    values.join(", ")
                ),
            )
            .with_citations(slot_citations(world, canon, &slot)),
        );
    }
    out
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
fn fd_violations(world: &CompiledWorld, canon: &BTreeMap<String, String>) -> Vec<WorldFinding> {
    // The certain source slots this check reads: authored `attr`/`fact` values,
    // excluding fluents (interval-sensitive, judged by `fluent_contradictions` —
    // a value held over disjoint spans is not a clash). Only a predicate the
    // author *declared* `functional` is checked: canon's shape cannot settle
    // whether a relation is single- or multi-valued — a subject with several
    // edges among others with one each is a hub in a many-to-many relation (a
    // place with several neighbours, R2/B) just as readily as a functional-
    // dependency break — so the author rules on it (koan 3, DL-0074) and the
    // engine never guesses, passing only the *declared* set to the primitive.
    // Slots are canon-merged (WS-1): `rank(a, gold)`/`rank(b, silver)` under a
    // certain `a = b` is one subject carrying two values, a real break.
    let certain = certain_binary_slots(world, canon).filter(|(pred, subject, _)| {
        !fluent_class_member(world, canon, &(pred.clone(), subject.clone()))
    });
    fd_violations_over(certain, &world.model.functional_predicates)
        .into_iter()
        .map(|violation| {
            let slot = (violation.pred.clone(), violation.subject.clone());
            WorldFinding::error(
                "continuity.fd-violation",
                format!(
                    "canon breaks the functional dependency on '{}': {} carries {} distinct certain values [{}]",
                    violation.pred,
                    violation.subject,
                    violation.values.len(),
                    violation.values.join(", ")
                ),
            )
            // The class's slot citations, PLUS the same-as link atom(s) that
            // merged it — so the finding names the identity assertion, not
            // only the two facts it brought together.
            .with_citations(slot_citations(world, canon, &slot))
            .with_data(fd_violation_data(&violation))
        })
        .collect()
}

/// `{pred, subject, values, slot_key}` — enough to reconstruct an FD-violation
/// verdict without parsing the message (WS-2's `Finding.data` rider, §2.9).
/// `slot_key` is `pred/subject`, a stable compound id for the slot a consumer
/// can correlate multiple findings (or a repair) against without re-deriving
/// it from `pred` and `subject` separately.
fn fd_violation_data(violation: &dli_core::fd::FdViolation) -> serde_json::Value {
    serde_json::json!({
        "pred": violation.pred,
        "subject": violation.subject,
        "values": violation.values,
        "slot_key": format!("{}/{}", violation.pred, violation.subject),
    })
}

/// `continuity.derived-fd-violation` (WS-2, amending DL-0075): a functional-
/// dependency break the author's own check/materialize loop would otherwise
/// never see. [`fd_violations`] reads authored atoms pre-saturation — a
/// deliberate DL-0075 split — but a wire-authored `author.add_rule` clause can
/// derive a second value for a functional slot that exists **only** in the
/// saturated layer; nothing pre-saturation ever holds it. This is the load-
/// battery's own derived lens, the read-side sibling of the fork/merge FD
/// check (`certain_derived_slots` in `dli-eval`'s `fork.rs`), so an author
/// sees the break at `check`/`materialize` time rather than only on merge.
///
/// Reads the valuation-certain rows of the world's saturated `derived` layer
/// (facts and rule productions alike — a fact is trivially its own
/// derivation), the same shared [`fd_violations_over`] primitive, and
/// **declared-only** functional predicates — never [`inferred_functional`]:
/// a slot's shape *after* saturation is even less informative about whether a
/// relation is meant to be single-valued than the authored shape already is
/// (koan 3, DL-0074). Fluent slots are excluded (rules cannot create spans —
/// `wm.fluents` is populated only by `process_state` — so a rule-derived
/// timeless value on a fluent slot is the pre-existing, already-documented
/// residual, not this check's concern). Any `(pred, subject)` the source-
/// certain [`fd_violations`] already reports is excluded here too: the two
/// checks partition source-only breaks from breaks the saturated layer alone
/// reveals, so nothing double-reports.
fn derived_fd_violations(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
) -> Vec<WorldFinding> {
    let source_breaks: BTreeSet<(String, String)> = fd_violations_over(
        certain_binary_slots(world, canon).filter(|(pred, subject, _)| {
            !fluent_class_member(world, canon, &(pred.clone(), subject.clone()))
        }),
        &world.model.functional_predicates,
    )
    .into_iter()
    .map(|violation| (violation.pred, violation.subject))
    .collect();

    let derived = certain_derived_binary_slots(world, canon).filter(|(pred, subject, _)| {
        !fluent_class_member(world, canon, &(pred.clone(), subject.clone()))
    });

    fd_violations_over(derived, &world.model.functional_predicates)
        .into_iter()
        .filter(|violation| {
            !source_breaks.contains(&(violation.pred.clone(), violation.subject.clone()))
        })
        .map(|violation| {
            let slot = (violation.pred.clone(), violation.subject.clone());
            WorldFinding::error(
                "continuity.derived-fd-violation",
                format!(
                    "the evaluated world derives a functional-dependency break on '{}': {} carries {} distinct certain values [{}] once rules fire",
                    violation.pred,
                    violation.subject,
                    violation.values.len(),
                    violation.values.join(", ")
                ),
            )
            .with_citations(derived_slot_citations(world, canon, &slot))
            .with_data(fd_violation_data(&violation))
        })
        .collect()
}

/// Every certain binary source slot `(pred, subject, value)` in the compiled
/// world — the annotated authored facts, before rule saturation — with the
/// subject AND value canonicalized through the identity map (canon before
/// [`fd_value_key`], WS-1). This is the atom layer the *load* check reads; the
/// fork- and merge-side FD checks read the saturated *derived* layer instead,
/// an intentional split documented in DL-0075: the load check judges what
/// canon **asserts**, a merge judges what it **derives** once re-evaluated.
fn certain_binary_slots<'a>(
    world: &'a CompiledWorld,
    canon: &'a BTreeMap<String, String>,
) -> impl Iterator<Item = (String, String, String)> + 'a {
    world
        .model
        .atoms()
        .iter()
        .filter(move |atom| atom_status(world, atom) == Status::Certain)
        .filter_map(move |atom| {
            slot_of(&atom.atom).map(|(slot, value)| {
                let (pred, subject) = canon_slot(canon, &slot);
                let value = dli_core::alias::canon(canon, &value).to_owned();
                (pred, subject, value)
            })
        })
}

/// Every certain binary slot `(pred, subject, value)` in the world's
/// **saturated** derivation layer (`world.model.world.derived`) — facts and
/// rule productions alike, since a fact is trivially its own derivation with
/// support equal to its own annotation. Mirrors `certain_binary_slots`'
/// canon-merging exactly, over the derived layer instead of the authored one
/// (the split [`derived_fd_violations`] documents, mirroring `dli-eval`
/// `fork.rs`'s `certain_derived_slots`).
fn certain_derived_binary_slots<'a>(
    world: &'a CompiledWorld,
    canon: &'a BTreeMap<String, String>,
) -> impl Iterator<Item = (String, String, String)> + 'a {
    world
        .model
        .world
        .derived
        .iter()
        .filter(move |derived| {
            derived
                .ann()
                .status(&world.model.world.valuation, &world.model.world.theory)
                == Status::Certain
        })
        .filter_map(move |derived| {
            ground_binary_slot(&derived.atom).map(|(slot, value)| {
                let (pred, subject) = canon_slot(canon, &slot);
                let value = dli_core::alias::canon(canon, &value).to_owned();
                (pred, subject, value)
            })
        })
}

/// A ground binary atom's slot `(pred, subject)` and its value (arg1). `None`
/// for a non-binary atom — the [`slot_of`] sibling for the already-rendered
/// [`GroundAtom`](dli_eval::world::GroundAtom) the derived layer carries, so
/// no `Term`/symbol unwrapping is needed.
fn ground_binary_slot(atom: &dli_eval::world::GroundAtom) -> Option<((String, String), String)> {
    if atom.args.len() != 2 {
        return None;
    }
    Some((
        (atom.pred.clone(), atom.args[0].clone()),
        atom.args[1].clone(),
    ))
}

/// [`slot_citations`] plus the citations of every certain **derived-layer**
/// atom in the slot's class — the piece a purely rule-derived value needs,
/// since it carries no entry in `world.model.atoms()` for `slot_citations`'
/// authored-atom scan to find. A derived-fd-violation finding this way names
/// the rule (or fact) that produced *each* conflicting value, not only the
/// authored half.
fn derived_slot_citations(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    slot: &(String, String),
) -> Vec<String> {
    let mut out = slot_citations(world, canon, slot);
    let slot = canon_slot(canon, slot);
    let interner = &world.model.world.interner;
    for derived in &world.model.world.derived {
        if derived
            .ann()
            .status(&world.model.world.valuation, &world.model.world.theory)
            != Status::Certain
        {
            continue;
        }
        let Some((atom_slot, _)) = ground_binary_slot(&derived.atom) else {
            continue;
        };
        if canon_slot(canon, &atom_slot) != slot {
            continue;
        }
        for citation in ann_citations(&derived.ann(), interner) {
            if !out.contains(&citation) {
                out.push(citation);
            }
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
/// slot, canonicalizing both the query slot and every candidate atom's slot
/// through the identity map (WS-1) — so a caller may pass either an alias
/// spelling or the canonical one and read the same class.
pub(super) fn certain_values(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    slot: &(String, String),
) -> BTreeSet<String> {
    let slot = canon_slot(canon, slot);
    let mut out = BTreeSet::new();
    for atom in world.model.atoms() {
        if atom_status(world, atom) != Status::Certain {
            continue;
        }
        if let Some((atom_slot, value)) = slot_of(&atom.atom)
            && canon_slot(canon, &atom_slot) == slot
        {
            out.insert(dli_core::alias::canon(canon, &value).to_owned());
        }
    }
    out
}

/// The certain values a slot holds, keyed by their normalized form — so `gold`
/// and `Gold` count once. Cardinality here is what the FD check reads: two keys
/// is two genuinely distinct values (issue 49, sub-claim 1).
fn certain_value_keys(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    slot: &(String, String),
) -> BTreeSet<String> {
    certain_values(world, canon, slot)
        .iter()
        .map(|value| fd_value_key(value))
        .collect()
}

/// Whether the compiled world holds any *possible-but-not-certain* fact for a
/// slot (the shadow layer), canon-aware like [`certain_values`].
fn slot_has_possible(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    slot: &(String, String),
) -> bool {
    let slot = canon_slot(canon, slot);
    world.model.atoms().iter().any(|atom| {
        atom_status(world, atom) == Status::Possible
            && slot_of(&atom.atom)
                .is_some_and(|(atom_slot, _)| canon_slot(canon, &atom_slot) == slot)
    })
}

/// The citations backing the certain fact(s) for a slot, deduplicated in order
/// — PLUS, when the slot's class was merged by one or more certain `same-as`
/// links, those links' own citations (WS-1). This is what lets a post-merge
/// `continuity.fd-violation` (or any other slot-citing finding) name the
/// identity assertion alongside the facts it brought into conflict.
pub(super) fn slot_citations(
    world: &CompiledWorld,
    canon: &BTreeMap<String, String>,
    slot: &(String, String),
) -> Vec<String> {
    let slot = canon_slot(canon, slot);
    let mut out = Vec::new();
    for atom in world.model.atoms() {
        if atom_status(world, atom) != Status::Certain {
            continue;
        }
        if slot_of(&atom.atom).is_some_and(|(atom_slot, _)| canon_slot(canon, &atom_slot) == slot) {
            for cite in &atom.citations {
                if !out.contains(cite) {
                    out.push(cite.clone());
                }
            }
        }
    }
    // The certain DECLARED identity link(s) that merged this class — reading
    // the declaration channel, never a predicate-name scan, so a plain
    // `fact same-as(x, y)` never gets credited as the reason for a merge it
    // took no part in.
    for link in declared_links(world) {
        if link
            .ann
            .status(&world.model.world.valuation, &world.model.world.theory)
            != Status::Certain
        {
            continue;
        }
        let in_class = [link.left, link.right]
            .into_iter()
            .map(|name| dli_core::alias::canon(canon, name))
            .any(|c| c == slot.1);
        if in_class {
            for cite in &link.citations {
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
