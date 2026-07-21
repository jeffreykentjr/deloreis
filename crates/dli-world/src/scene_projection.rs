//! Project a [`CompiledWorld`] into a [`SceneProjection`] — the premise pool a
//! diegetic proof is checked against (`docs/spec/04-practices.md` §2, PRAC-2).
//!
//! [`crate::check::factcheck`] resolves *claims* against the certain projection;
//! this module builds the *casting seam* the ledger checker
//! ([`dli_ledger::ledger_check::check_proof`]) reads: a [`SceneProjection`] of
//! plain data carrying the premise pool, the timing phases in force, the resolved
//! affine caps, and each cycle's declared spokes.
//!
//! # The seam and the dependency inversion
//!
//! `dli-ledger` deliberately knows nothing about `dli-world`: it consumes a
//! [`SceneProjection`] the caller supplies (see [`dli_ledger::premise`]). This
//! module is that caller (the PAY-2 side), lifting a compiled world into the seam.
//!
//! # What is read precisely, and the documented default
//!
//! The spec's pool is **certain ∩ known-by-speaker ∩ present-in-scene**
//! (`docs/spec/04-practices.md` §2). Three of those are read from the world
//! exactly:
//!
//! - **the certain layer** — every atom whose [`Status`] under the compiled
//!   ledger valuation is [`Certain`](Status::Certain) is the base pool;
//! - **timing phases** — a certain `phase(<cycle>, <spoke>)` fact on a declared
//!   cycle seats that cycle's phase in force;
//! - **affine caps** — a certain `cap(<resource>, <n>)` fact resolves that
//!   resource's affine bound for the scene;
//! - **cycle spokes** — every declared cycle contributes its spoke set, so a
//!   timing premise naming a spoke the cycle cannot reach is caught as an L4
//!   bad-instantiation rather than merely a wrong phase.
//!
//! **The known-by-speaker / present-in-scene default.** Computing those two
//! *precisely* needs a scene-scoped, story-time query — `knows(who, fact)`
//! resolved *as-of* the scene's position (`docs/spec/03-time.md` §4), and cast
//! presence on that beat — neither of which a `(world, speaker)` pair carries. So
//! a bare corpus is projected under the **full-information default**: every
//! certain fact is taken as known to the speaker and present on stage, hence the
//! pool equals the certain layer. This is the conservative choice, not a liberal
//! one — it fabricates no atom (every atom placed on stage is one the world
//! already holds certain), whereas an empty knowledge/presence set would fabricate
//! *absence*, turning valid castings into spurious L2 fizzles.
//!
//! **Speaker-relative refinement (opt-in).** When the corpus *does* model speaker
//! knowledge with ground `knows(<speaker>, <subject>)` certain facts, a named
//! speaker narrows the pool: a premise is known only if its subject (its first
//! argument) is one the speaker is recorded as knowing. Absent any such fact for
//! the speaker, the full-information default stands. Denials are never
//! synthesized: contradiction in this algebra is an FD-versus-certain clash
//! (`docs/spec/02-semantics.md` §4), a per-atom relation rather than a finite
//! enumerable set, so [`SceneProjection::denied`] is left empty and a caller
//! testing a specific rebound supplies it with [`SceneProjection::deny`].

use std::collections::BTreeSet;

use dli_ast::term::{AtomExpr, Term, TimeRef};
use dli_core::ann::Status;
use dli_ledger::SceneProjection;

use crate::check::CompiledWorld;

/// Project a compiled world into the [`SceneProjection`] a casting is checked
/// against (`docs/spec/04-practices.md` §2).
///
/// The certain layer is the base premise pool; `phase(<cycle>, <spoke>)` and
/// `cap(<resource>, <n>)` certain facts seat the timing phases and affine caps;
/// and every declared cycle contributes its spokes. Knowledge and presence follow
/// the **full-information default** — the pool equals the certain layer — unless
/// the corpus records ground `knows(<speaker>, <subject>)` facts for the given
/// speaker, in which case the pool is narrowed to premises whose subject the
/// speaker knows (see the module docs for why this is the conservative default).
///
/// `speaker` names the caster the pool is projected *for*; pass [`None`] to
/// project the speaker-independent full-information pool.
///
/// ```
/// use dli_syntax::parse_document;
/// use dli_world::pack::Pack;
/// use dli_world::{compile, scene_projection::project_scene};
/// use dli_ledger::premise::PremiseStatus;
///
/// // A tiny world: a certain consent fact, the tide's four-spoke cycle, and the
/// // tide seated at slack this scene.
/// let src = "\
/// scope demo
/// ratify d-1
/// cycle tide(4) = ebb | slack | flood | high  proposed
/// fact enfys holds consent  decided d-1
/// fact tide phase slack  decided d-1
/// ";
/// let (doc, diags) = parse_document(src);
/// assert!(diags.is_empty(), "{diags:?}");
/// let world = compile(&Pack { name: "demo".to_owned(), documents: vec![doc] })
///     .expect("the demo world compiles");
///
/// let scene = project_scene(&world, Some("enfys"));
/// // The certain consent fact is in the pool (full-information default).
/// assert_eq!(
///     scene.availability("holds(enfys, consent)"),
///     PremiseStatus::Available,
/// );
/// // The tide's phase in force and its declared spokes are read from the world.
/// assert_eq!(scene.phase_of("tide"), Some("slack"));
/// assert_eq!(scene.is_declared_spoke("tide", "flood"), Some(true));
/// assert_eq!(scene.is_declared_spoke("tide", "purple"), Some(false));
/// // A fact canon never asserts is merely absent, not denied.
/// assert_eq!(scene.availability("holds(elsewhere)"), PremiseStatus::Absent);
/// ```
#[must_use]
pub fn project_scene(world: &CompiledWorld, speaker: Option<&str>) -> SceneProjection {
    let evaluable = &world.model.world;
    let cycles: BTreeSet<&str> = evaluable.cycles.iter().map(|c| c.name.as_str()).collect();

    // Pass 1: harvest the certain layer (as pool entries paired with their
    // subject), the timing/cap bindings, and the speaker's recorded knowledge.
    let mut pool: Vec<(String, Option<String>)> = Vec::new();
    let mut phases: Vec<(String, String)> = Vec::new();
    let mut caps: Vec<(String, i64)> = Vec::new();
    let mut known_subjects: BTreeSet<String> = BTreeSet::new();
    let mut speaker_has_knowledge = false;

    for loaded in world.model.atoms() {
        if loaded.ann.status(&evaluable.valuation, &evaluable.theory) != Status::Certain {
            continue;
        }
        let atom = &loaded.atom;
        let pred = atom.pred.text.as_str();

        // Timing: `phase(<cycle>, <spoke>)` seats a declared cycle's phase.
        if pred == "phase"
            && let [cycle_term, spoke_term] = atom.args.as_slice()
            && let (Some(cycle), Some(spoke)) = (term_sym(cycle_term), term_sym(spoke_term))
            && cycles.contains(cycle)
        {
            phases.push((cycle.to_owned(), spoke.to_owned()));
        }

        // Affine cap: `cap(<resource>, <n>)` resolves a symbolic bound to an int.
        if pred == "cap"
            && let [resource_term, bound_term] = atom.args.as_slice()
            && let (Some(resource), Some(bound)) = (term_sym(resource_term), term_int(bound_term))
        {
            caps.push((resource.to_owned(), bound));
        }

        // Knowledge fluent (opt-in): `knows(<speaker>, <subject>)`.
        if pred == "knows"
            && let [knower_term, subject_term] = atom.args.as_slice()
            && let Some(knower) = term_sym(knower_term)
            && speaker == Some(knower)
        {
            speaker_has_knowledge = true;
            known_subjects.insert(render_term(subject_term));
        }

        let subject = atom.args.first().and_then(term_sym).map(str::to_owned);
        pool.push((render_atom(atom), subject));
    }

    // The pool is narrowed by knowledge only when the speaker is named AND the
    // corpus records at least one `knows(<speaker>, _)` fact; otherwise the
    // full-information default (the whole certain layer is known and present).
    let gate_knowledge = speaker.is_some() && speaker_has_knowledge;

    // Pass 2: assemble the projection. Spokes, phases, and caps are read from the
    // world; the pool atoms are seated (present always; known per the gate).
    let mut proj = SceneProjection::new();
    for cycle in &evaluable.cycles {
        let spokes: Vec<&str> = cycle.spokes.iter().map(String::as_str).collect();
        proj = proj.spokes(&cycle.name, &spokes);
    }
    for (cycle, spoke) in &phases {
        proj = proj.at_phase(cycle, spoke);
    }
    for (resource, bound) in &caps {
        proj = proj.cap(resource, *bound);
    }
    for (rendered, subject) in &pool {
        proj = proj.certain(rendered).present(rendered);
        let known = if gate_knowledge {
            subject
                .as_deref()
                .is_some_and(|s| known_subjects.contains(s))
        } else {
            true
        };
        if known {
            proj = proj.known(rendered);
        }
    }
    proj
}

/// Render an [`AtomExpr`] to its `pred(a, b)` surface form (bare `pred` when
/// nullary); [`SceneProjection`] normalizes whitespace on the way in, so this only
/// has to be canonical up to spacing.
fn render_atom(atom: &AtomExpr) -> String {
    if atom.args.is_empty() {
        return atom.pred.text.clone();
    }
    let args: Vec<String> = atom.args.iter().map(render_term).collect();
    format!("{}({})", atom.pred.text, args.join(", "))
}

/// Render one term to its ground string form (mirrors the loader's rendering).
fn render_term(term: &Term) -> String {
    match term {
        Term::Var(id) | Term::Sym(id) => id.text.clone(),
        Term::Str(text) => text.clone(),
        Term::Int(value) => value.to_string(),
        Term::Time(TimeRef::Era(id)) => format!("era:{}", id.text),
        Term::Time(TimeRef::Event(id)) => format!("event:{}", id.text),
    }
}

/// The symbol text of a term, or [`None`] if it is not a bare symbol/variable.
fn term_sym(term: &Term) -> Option<&str> {
    match term {
        Term::Sym(id) | Term::Var(id) => Some(id.text.as_str()),
        _ => None,
    }
}

/// The integer value of a term, or [`None`] if it is not an integer literal.
fn term_int(term: &Term) -> Option<i64> {
    match term {
        Term::Int(value) => Some(*value),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
    use dli_ast::decl::{CycleDecl, Decl, Document, FactDecl, RatifyDecl, ScopeHeader};
    use dli_ast::ident::Ident;
    use dli_ast::span::Span;
    use dli_ledger::premise::PremiseStatus;

    use crate::compile;
    use crate::pack::Pack;

    fn id(text: &str) -> Ident {
        Ident::synthetic(text)
    }

    fn decided(cite: &str) -> Annotation {
        Annotation {
            marker: ProvenanceMarker::Decided,
            citations: vec![CitationRef { id: id(cite) }],
            condition: None,
            tags: Vec::new(),
            span: Span::synthetic(),
        }
    }

    fn ratify(decision: &str) -> Decl {
        Decl::Ratify(RatifyDecl {
            decision: id(decision),
            body: Vec::new(),
        })
    }

    /// A certain fact `pred(args…)`, its decision declared so the citation carries
    /// authority (B1).
    fn fact(pred: &str, args: Vec<Term>, cite: &str) -> Decl {
        Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred: id(pred),
                args,
            },
            triple: false,
            provenance: Some(decided(cite)),
        })
    }

    fn sym(text: &str) -> Term {
        Term::Sym(id(text))
    }

    fn tide_cycle() -> Decl {
        Decl::Cycle(CycleDecl {
            name: id("tide"),
            modulus: Some(4),
            spokes: ["ebb", "slack", "flood", "high"]
                .iter()
                .map(|s| id(s))
                .collect(),
            provenance: None,
        })
    }

    fn world(decls: Vec<Decl>) -> CompiledWorld {
        compile(&Pack {
            name: "scene".to_owned(),
            documents: vec![Document {
                scope: Some(ScopeHeader {
                    name: id("scene"),
                    span: Span::synthetic(),
                }),
                decls,
            }],
        })
        .expect("the scene world compiles")
    }

    #[test]
    fn certain_layer_is_the_full_information_pool() {
        let world = world(vec![
            ratify("d-1"),
            fact("holds", vec![sym("enfys"), sym("consent")], "d-1"),
        ]);
        let scene = project_scene(&world, None);
        // The certain fact is seated (certain ∩ known ∩ present) under the default.
        assert_eq!(
            scene.availability("holds(enfys, consent)"),
            PremiseStatus::Available,
        );
        // A fact canon never asserts is absent (a fizzle), never a denial.
        assert_eq!(
            scene.availability("holds(nobody, nothing)"),
            PremiseStatus::Absent,
        );
        assert!(scene.denied.is_empty(), "denials are never synthesized");
    }

    #[test]
    fn proposed_shadow_lore_is_not_in_the_pool() {
        // A proposed fact carries no ruling, so it is not certain and stays out.
        let world = world(vec![Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred: id("holds"),
                args: vec![sym("enfys"), sym("consent")],
            },
            triple: false,
            provenance: Some(Annotation {
                marker: ProvenanceMarker::Proposed,
                citations: Vec::new(),
                condition: None,
                tags: Vec::new(),
                span: Span::synthetic(),
            }),
        })]);
        let scene = project_scene(&world, None);
        assert_eq!(
            scene.availability("holds(enfys, consent)"),
            PremiseStatus::Absent,
            "shadow lore is not projected as an available premise",
        );
    }

    #[test]
    fn phase_cap_and_spokes_are_read_from_the_world() {
        let world = world(vec![
            ratify("d-1"),
            tide_cycle(),
            fact("phase", vec![sym("tide"), sym("slack")], "d-1"),
            fact("cap", vec![sym("strain"), Term::Int(2)], "d-1"),
        ]);
        let scene = project_scene(&world, None);
        assert_eq!(scene.phase_of("tide"), Some("slack"));
        assert_eq!(scene.cap_of("strain"), Some(2));
        // Declared spokes let an undeclared spoke be caught as bad-instantiation.
        assert_eq!(scene.is_declared_spoke("tide", "flood"), Some(true));
        assert_eq!(scene.is_declared_spoke("tide", "purple"), Some(false));
        // A cycle the world never declares has an unknown (deferred) spoke set.
        assert_eq!(scene.is_declared_spoke("moon", "wax"), None);
    }

    #[test]
    fn a_non_declared_cycle_phase_fact_is_not_read_as_a_phase() {
        // `phase(mood, sour)` names no declared cycle, so it seats no phase — it is
        // just an ordinary certain atom in the pool.
        let world = world(vec![
            ratify("d-1"),
            fact("phase", vec![sym("mood"), sym("sour")], "d-1"),
        ]);
        let scene = project_scene(&world, None);
        assert_eq!(scene.phase_of("mood"), None);
        assert_eq!(
            scene.availability("phase(mood, sour)"),
            PremiseStatus::Available,
        );
    }

    #[test]
    fn a_named_speaker_narrows_the_pool_by_recorded_knowledge() {
        // The corpus records what enfys knows: the door, but not the well.
        let world = world(vec![
            ratify("d-1"),
            fact("knows", vec![sym("enfys"), sym("door")], "d-1"),
            fact("warded", vec![sym("door")], "d-1"),
            fact("warded", vec![sym("well")], "d-1"),
        ]);
        // For the knowing speaker, only the door-subject premise is in the pool.
        let scene = project_scene(&world, Some("enfys"));
        assert_eq!(
            scene.availability("warded(door)"),
            PremiseStatus::Available,
            "the speaker knows the door",
        );
        assert_eq!(
            scene.availability("warded(well)"),
            PremiseStatus::Absent,
            "the speaker does not know the well, so it is outside the pool",
        );
        // A different (unmodelled) speaker falls back to the full-information pool.
        let other = project_scene(&world, Some("nimue"));
        assert_eq!(
            other.availability("warded(well)"),
            PremiseStatus::Available,
            "no knows-facts for this speaker → full-information default",
        );
    }
}
