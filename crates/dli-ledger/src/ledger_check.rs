//! The diegetic proof checker — the Concordance-class engine.
//!
//! A **proof** is one casting written out as data: its premises, its steps, its
//! spends, and its close, run `in <practice>` (`docs/spec/04-practices.md` §5).
//! [`check_proof`] replays it against the practice's declared law-set and the
//! [`SceneProjection`] of the world as of the scene, and reports a per-step
//! verdict, a weakest-link [`Grade`], and every structural finding — each a
//! *refinement* of one of the seven classes `L1`..`L7` (`docs/spec/04` §2).
//!
//! The engine knows nothing about any particular practice: proof-magic,
//! true-naming, and the Turning all check here because the practice it runs
//! against is [data all the way down](crate::practice) (charter koan 10). The
//! seven behaviours it realizes:
//!
//! - **L1 unknown-form** — a step names a form the practice does not declare.
//! - **L2 premise-not-available** — a premise is *absent* (fizzle) or
//!   *contradicted* (rebound) in the scene projection.
//! - **L3 side-condition** — a timing premise is seated at the wrong phase of a
//!   cycle (wrong-phase), the series-opening beat.
//! - **L4 bad-instantiation** — a form is bound against arguments its declared
//!   parameters forbid.
//! - **L5 invalid-chain** — a step joins premises by a law the practice does not
//!   license.
//! - **L6 resource-violation** — a spend breaks a resource's algebra law
//!   (overload is the affine bound on strain).
//! - **L7 close-mismatch** — the close does not follow from the accounted ledger.
//!
//! Over all of that rides the **expected-verdict discipline**: a proof declares
//! the verdict it expects, and a scene built on a character's flawed argument
//! that *accidentally validates* is a caught bug (`fails-at` is xfail-for-drama),
//! not a silent success.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::decl::ProofDecl;
use dli_ast::ident::Ident;
use dli_ast::proof::{ProofBody, ProofOp, ProofPremise, ProofStep, ResourceKeyword};
use dli_ast::term::Term;
use dli_core::grade::Grade;

use crate::practice::{PracticeDef, StructuralClass};
use crate::premise::{PremiseStatus, SceneProjection, normalize_atom};
use crate::resource::{
    Delta, ResourceKind, ResourceViolationCode, check_affine_bounded, check_conserved,
    check_graded_monotone, check_linear,
};

/// Which proof form a [`StepVerdict`] records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProofForm {
    /// `premise <l> <atom> …` — an atom the casting rests on.
    Premise,
    /// `assume <l> <atom>` — a fork-local supposition.
    Assume,
    /// `step <l> instantiate <form> with …` — bind a form's parameters.
    Instantiate,
    /// `step <l> chain from … by <law> conclusion <atom>` — join by a law.
    Chain,
    /// `spend <l>` — draw a resource unit against a premise.
    Spend,
    /// `release <l>` — return a conserved unit to the multiset.
    Release,
    /// `redistribute <l>` — move a conserved unit between accounts.
    Redistribute,
    /// `reveal <l>` — surface a conserved unit already implied.
    Reveal,
    /// `close <atom> expect <verdict>` — the casting's conclusion.
    Close,
    /// A form the checker did not recognize (kept, never dropped — koan 4).
    Other,
}

/// One step's verdict in a checked proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StepVerdict {
    /// The step's label (`p1`, `s2`), or the keyword for an unlabeled form.
    pub label: String,
    /// Which form this step is.
    pub form: ProofForm,
    /// Whether the step stands — no finding fired against it.
    pub ok: bool,
    /// The grade this step contributes to the procedure chain.
    pub grade: Grade,
    /// A domain-spoken note (what the step did, or why it failed).
    pub note: String,
}

/// What the checker made of the close, before the expectation is weighed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProofVerdict {
    /// The close follows from the accounted ledger; nothing fired.
    Holds,
    /// The casting is blocked at a structural class, with the practice's refining
    /// error name (if one matched) and the step where it fired.
    FailedAt {
        /// The structural class the casting failed at.
        class: StructuralClass,
        /// The diegetic error this refines to, if the practice named one.
        error: Option<String>,
        /// The label of the step where it failed.
        at: String,
    },
}

/// The verdict a proof declares it expects at its close (`docs/spec/04` §5).
///
/// Externally tagged (`rename_all` only, no internal `tag`): the `FailsAt` and
/// `Unknown` newtype variants wrap a bare `String`, which serde's internal
/// tagging cannot merge a tag into — so this enum keeps serde's default
/// (external) representation, unlike the struct/unit-only [`ProofFinding`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Expectation {
    /// `expect holds` — the casting should reach its close cleanly.
    Holds,
    /// `expect blocked` — the casting should be blocked (by some failure).
    Blocked,
    /// `expect fails-at <error>` — the casting should fail at exactly this
    /// declared error. A wrong argument that accidentally validates is a bug.
    FailsAt(String),
    /// An `expect` clause naming a token that is not one of the three verdicts —
    /// a typo silently reading as `holds` would gut the whole discipline
    /// (`docs/spec/04` §5), so an unknown token is kept and refused, not coerced.
    Unknown(String),
}

/// A structural finding against a proof — a refinement of one of `L1`..`L7`, or
/// the meta-finding that the declared expectation was not met.
///
/// Two-channel like every load finding (charter koan 9): a stable machine
/// [`code`](ProofFinding::code) and a domain-spoken
/// [`message`](ProofFinding::message).
///
/// Internally tagged on `finding` (the house style for the fabric's data-carrying
/// finding enums — cf. `PracticeFinding` and `LoadFinding`): every variant is a
/// struct or unit variant, so the tag merges cleanly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "finding", rename_all = "kebab-case")]
pub enum ProofFinding {
    /// L1 — a step names a form the practice does not declare.
    UnknownForm {
        /// The undeclared form.
        form: String,
        /// The step that named it.
        at: String,
    },
    /// L2 (*absent*) — a premise merely unestablished in the projection: a fizzle.
    PremiseAbsent {
        /// The premise label.
        label: String,
        /// The premise atom, as written.
        atom: String,
    },
    /// L2 (*contradicted*) — a premise the certain projection denies: a rebound.
    PremiseContradicted {
        /// The premise label.
        label: String,
        /// The premise atom, as written.
        atom: String,
    },
    /// L3 (*timing*) — a timing premise seated at the wrong phase: wrong-phase.
    WrongPhase {
        /// The premise label.
        label: String,
        /// The cycle whose phase was wrong.
        cycle: String,
        /// The phase the casting required.
        required: String,
        /// The phase in force in the scene, if the scene established one.
        actual: Option<String>,
    },
    /// L4 — a form bound against an argument its parameters forbid.
    BadInstantiation {
        /// The form being instantiated.
        form: String,
        /// The binding key the form does not declare.
        binding: String,
        /// The step.
        at: String,
    },
    /// The proof header binds a role the practice does not declare. This is
    /// malformed proof metadata, not an expected diegetic failure.
    UnknownRole {
        /// The undeclared role key.
        role: String,
        /// The entity written as its filler.
        actor: String,
    },
    /// L5 — a step joins premises by a law the practice does not license, or over
    /// a premise it never established.
    InvalidChain {
        /// The law named (or `<unknown>` when a joined label does not resolve).
        law: String,
        /// The step.
        at: String,
    },
    /// L6 — a spend broke a resource's algebra law (overload is the affine case).
    ResourceViolation {
        /// The resource whose law broke.
        resource: String,
        /// The stable class of the breach.
        code: ResourceViolationCode,
        /// The domain-spoken description from the resource algebra.
        message: String,
        /// The step where the offending delta was applied.
        at: String,
    },
    /// L7 — the close does not follow from the accounted ledger.
    CloseMismatch {
        /// The close atom, as written.
        close: String,
    },
    /// The declared expectation was not met — most sharply, a proof that expected
    /// a failure but *validated* (the silent-success bug the discipline catches).
    ExpectationUnmet {
        /// The expectation the proof declared.
        expected: Expectation,
        /// The verdict the checker reached.
        got: ProofVerdict,
    },
    /// L6 — a resource operation names a label that is not a declared resource
    /// premise (an undefined label, or a world/timing premise). The accounting
    /// cannot operate on a unit that was never declared.
    InvalidSpend {
        /// The keyword written (`spend` or `release`).
        keyword: String,
        /// The label it named.
        label: String,
    },
    /// L4 — a timing premise names a spoke the cycle does not carry
    /// (`phase(tide, purple)` on `tide = ebb | slack | flood | high`): a
    /// bad-instantiation, the phase equivalent of laying a figure on a leaf the
    /// cycle has no room for.
    UnknownSpoke {
        /// The premise label.
        label: String,
        /// The cycle named.
        cycle: String,
        /// The undeclared spoke.
        spoke: String,
    },
    /// The `expect` clause named a token that is not `holds`, `blocked`, or
    /// `fails-at <error>` — a typo the discipline must refuse, not coerce to
    /// `holds`.
    UnknownExpectation {
        /// The unrecognized token.
        token: String,
    },
    /// The proof has no `close` — a casting with no conclusion cannot be judged.
    MissingClose,
    /// The proof's header `in <practice>` names a practice other than the one it
    /// was checked against; the verdict would be about the wrong law-set.
    PracticeMismatch {
        /// The practice the header named.
        header: String,
        /// The practice actually checked against.
        checked: String,
    },
    /// The direct AST header is not `<role> <actor> in <practice>`. Parser-backed
    /// callers reject this earlier; the checker retains the invariant.
    InvalidHeaderLayout,
    /// Captured proof clauses do not replay through the canonical surface and
    /// parser. This protects direct AST callers from bypassing proof syntax.
    MalformedBody {
        /// Domain diagnostics returned by the real parser.
        diagnostics: Vec<String>,
    },
    /// The proof does not carry exactly one final close. Parser-backed callers
    /// reject this earlier; the checker keeps the invariant for direct AST users.
    InvalidCloseLayout {
        /// Number of close operations present.
        count: usize,
        /// Whether the sole close, when there is one, is the final operation.
        final_operation: bool,
    },
}

impl ProofFinding {
    /// The structural class this finding refines, or [`None`] for the meta
    /// expectation finding (which is about the verdict, not a step).
    #[must_use]
    pub const fn class(&self) -> Option<StructuralClass> {
        Some(match self {
            Self::UnknownForm { .. } => StructuralClass::L1,
            Self::PremiseAbsent { .. } | Self::PremiseContradicted { .. } => StructuralClass::L2,
            Self::WrongPhase { .. } => StructuralClass::L3,
            Self::BadInstantiation { .. } | Self::UnknownSpoke { .. } => StructuralClass::L4,
            Self::InvalidChain { .. } => StructuralClass::L5,
            Self::ResourceViolation { .. } | Self::InvalidSpend { .. } => StructuralClass::L6,
            Self::CloseMismatch { .. } => StructuralClass::L7,
            Self::ExpectationUnmet { .. }
            | Self::UnknownExpectation { .. }
            | Self::MissingClose
            | Self::PracticeMismatch { .. }
            | Self::UnknownRole { .. }
            | Self::InvalidHeaderLayout
            | Self::MalformedBody { .. }
            | Self::InvalidCloseLayout { .. } => return None,
        })
    }

    /// A stable, machine-readable class for this finding (charter koan 9).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownForm { .. } => "proof.unknown-form",
            Self::PremiseAbsent { .. } => "proof.premise-absent",
            Self::PremiseContradicted { .. } => "proof.premise-contradicted",
            Self::WrongPhase { .. } => "proof.wrong-phase",
            Self::BadInstantiation { .. } => "proof.bad-instantiation",
            Self::UnknownRole { .. } => "proof.unknown-role",
            Self::InvalidChain { .. } => "proof.invalid-chain",
            Self::ResourceViolation { .. } => "proof.resource-violation",
            Self::CloseMismatch { .. } => "proof.close-mismatch",
            Self::ExpectationUnmet { .. } => "proof.expectation-unmet",
            Self::InvalidSpend { .. } => "proof.invalid-spend",
            Self::UnknownSpoke { .. } => "proof.unknown-spoke",
            Self::UnknownExpectation { .. } => "proof.unknown-expectation",
            Self::MissingClose => "proof.missing-close",
            Self::PracticeMismatch { .. } => "proof.practice-mismatch",
            Self::InvalidHeaderLayout => "proof.invalid-header-layout",
            Self::MalformedBody { .. } => "proof.malformed-body",
            Self::InvalidCloseLayout { .. } => "proof.invalid-close-layout",
        }
    }

    /// A domain-spoken description of the finding (charter koan 9).
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnknownForm { form, at } => {
                format!("{at}: the form '{form}' is not declared by this practice (L1)")
            }
            Self::PremiseAbsent { label, atom } => format!(
                "{label}: '{atom}' is not established in the scene — the casting fizzles \
                 (L2, absent)"
            ),
            Self::PremiseContradicted { label, atom } => format!(
                "{label}: '{atom}' is denied by the certain projection — the casting rebounds \
                 (L2, contradicted)"
            ),
            Self::WrongPhase {
                label,
                cycle,
                required,
                actual,
            } => {
                let is = actual.as_deref().unwrap_or("not established");
                format!(
                    "{label}: {cycle} is {is} in the scene; the casting requires {required} \
                     — wrong-phase (L3, timing)"
                )
            }
            Self::BadInstantiation { form, binding, at } => {
                format!("{at}: '{form}' has an invalid binding: {binding} (L4)")
            }
            Self::UnknownRole { role, actor } => format!(
                "the proof binds '{actor}' as '{role}', but this practice declares no such role \
                 — the proof header is malformed"
            ),
            Self::InvalidChain { law, at } => format!(
                "{at}: the chain by '{law}' is not licensed — no such law, or it joins a premise \
                 never established (L5)"
            ),
            Self::ResourceViolation {
                resource,
                message,
                at,
                ..
            } => format!("{at}: {resource} — {message} (L6)"),
            Self::CloseMismatch { close } => {
                format!("the close '{close}' does not follow from the accounted ledger (L7)")
            }
            Self::ExpectationUnmet { expected, got } => format!(
                "the casting expected {} but the checker reached {} — the expected verdict was \
                 not met",
                describe_expectation(expected),
                describe_verdict(got),
            ),
            Self::InvalidSpend { keyword, label } => format!(
                "{keyword} '{label}' names no declared resource premise — resource bookkeeping \
                 cannot operate on something that is not a unit (L6)"
            ),
            Self::UnknownSpoke {
                label,
                cycle,
                spoke,
            } => format!(
                "{label}: '{spoke}' is not a phase the cycle '{cycle}' carries — bad \
                 instantiation (L4)"
            ),
            Self::UnknownExpectation { token } => format!(
                "the close expects '{token}', which is not a verdict (holds | blocked | \
                 fails-at <error>) — refused, not read as holds"
            ),
            Self::MissingClose => {
                "the proof has no close — a casting with no conclusion cannot be judged".to_owned()
            }
            Self::PracticeMismatch { header, checked } => format!(
                "the proof runs `in {header}` but was checked against '{checked}' — the verdict \
                 would be about the wrong practice"
            ),
            Self::InvalidHeaderLayout => {
                "the proof header is malformed — expected `<role> <actor> in <practice>`".to_owned()
            }
            Self::MalformedBody { diagnostics } => format!(
                "the proof body does not replay through the language parser — {}",
                diagnostics.join("; ")
            ),
            Self::InvalidCloseLayout {
                count,
                final_operation,
            } => format!(
                "the proof has {count} close operation(s), final={final_operation} — a proof must \
                 carry exactly one final conclusion"
            ),
        }
    }
}

/// The full report of checking one proof.
///
/// This is a frozen seam: `SERVE-1`'s `dli.check-ledger` and the payload battery
/// (PAY-2) consume it. The [`checks`](ProofReport::checks) flag is the one-bit
/// verdict — the proof is structurally valid and its declared expectation was
/// met — while
/// [`findings`](ProofReport::findings), [`verdict`](ProofReport::verdict), and
/// [`grade`](ProofReport::grade) carry the why, the where, and the strength.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofReport {
    /// The proof's name.
    pub proof: Ident,
    /// The speaker named in the header (`speaker <who>`), if any.
    pub speaker: Option<String>,
    /// The practice it ran in.
    pub practice: Ident,
    /// Per-step verdicts, in ledger order.
    pub steps: Vec<StepVerdict>,
    /// The structural findings, ordered by the step where each fired.
    pub findings: Vec<ProofFinding>,
    /// The verdict the checker reached at the close.
    pub verdict: ProofVerdict,
    /// The expectation the proof declared.
    pub expected: Expectation,
    /// Whether the reached verdict met the declared expectation.
    pub expectation_met: bool,
    /// The weakest-link grade over the procedure chain (`Grade::weakest`).
    pub grade: Grade,
    /// Whether the proof checks: its shape is valid and its declared expectation
    /// was met.
    pub checks: bool,
}

/// Check one proof against a practice and the scene it is spoken in.
///
/// The projection carries the premise pool (`certain ∩ known-by-speaker ∩
/// present`), the denied side that splits a rebound from a fizzle, the cycle
/// phases in force, and the resolved affine capacities — everything the checker
/// needs about the world, as plain data the caller supplies (see
/// [`crate::premise`]).
///
/// ```
/// use dli_ast::decl::Decl;
/// use dli_ledger::practice::compile_practices;
/// use dli_ledger::premise::SceneProjection;
/// use dli_ledger::ledger_check::{Expectation, ProofVerdict, check_proof};
/// use dli_syntax::parse_document;
///
/// let src = "\
/// scope magic
/// cycle tide(4) = ebb | slack | flood | high  proposed
/// practice spoken-proof proposed {
///   kind ground = door | field | ore
///   resource debt : linear conserved
///   rule ward-form(ground, fibre)
///   rule give-and-take
///   error fizzle refines L2 premise-not-available where absent
///   error wrong-hour refines L3 side-condition [timing] over cycle tide
///   cost material
/// }
/// proof e01-s04 speaker protagonist in spoken-proof {
///   premise p1 holds(enfys, consent(household, door)) cited D-0113 cue \"line 112\"
///   premise p2 phase(tide, slack) from-scene cue \"line 118\"
///   premise p3 debt(1) drawn-against p1
///   step s1 instantiate ward-form with ground = door, fibre = withy
///   step s2 chain from p1, s1 by give-and-take conclusion bound(ward, door)
///   spend p3
///   close bound(ward, door) expect holds
/// }
/// ";
/// let (doc, diags) = parse_document(src);
/// assert!(diags.is_empty(), "{diags:?}");
/// let practices = compile_practices(&doc);
/// let proof = doc
///     .decls
///     .iter()
///     .find_map(|d| match d {
///         Decl::Proof(p) => Some(p),
///         _ => None,
///     })
///     .expect("one proof");
/// let scene = SceneProjection::new()
///     .seat("holds(enfys, consent(household, door))")
///     .at_phase("tide", "slack");
/// let report = check_proof(&practices[0], proof, &scene);
/// assert!(
///     report.checks,
///     "{:?}",
///     report.findings.iter().map(|f| f.message()).collect::<Vec<_>>()
/// );
/// assert_eq!(report.verdict, ProofVerdict::Holds);
/// assert_eq!(report.expected, Expectation::Holds);
/// ```
#[must_use]
pub fn check_proof(
    practice: &PracticeDef,
    proof: &ProofDecl,
    scene: &SceneProjection,
) -> ProofReport {
    let speaker = proof.actor().map(ToOwned::to_owned);
    let header_valid =
        proof.role().is_some() && proof.actor().is_some() && proof.practice().is_some();
    let items = reassemble(&proof.body);
    let close_count = items.iter().filter(|item| item.keyword == "close").count();
    let close_is_final = items.last().is_some_and(|item| item.keyword == "close");
    let surface_errors = if header_valid && close_count == 1 && close_is_final {
        proof_structural_errors(&proof.body)
    } else {
        Vec::new()
    };
    let resources: BTreeSet<&str> = practice
        .resources
        .iter()
        .map(|r| r.name.text.as_str())
        .collect();
    let cycles: BTreeSet<&str> = practice
        .cycles_in_scope
        .iter()
        .map(String::as_str)
        .collect();

    let LedgerRun {
        steps,
        mut findings,
        grades,
        established,
        deltas,
        close,
    } = run_ledger(&items, practice, scene, &resources, &cycles);

    let unknown_role = if let (Some(role), Some(actor)) = (proof.role(), proof.actor())
        && role != "speaker"
        && !practice
            .roles
            .iter()
            .any(|declared| declared.name.text == role)
    {
        Some((role, actor))
    } else {
        None
    };
    findings.extend(replay_resources(practice, scene, &deltas, &items));
    findings.sort_by_key(|(pos, _)| *pos);

    // The verdict is the earliest structural failure; if none, the close must
    // still follow from the accounted ledger (L7).
    let mut verdict = match findings.first() {
        Some((_, finding)) => failed_at(practice, finding),
        None => ProofVerdict::Holds,
    };
    if verdict == ProofVerdict::Holds
        && let Some((atom, _)) = &close
        && !established.contains(&normalize_atom(atom))
    {
        let finding = ProofFinding::CloseMismatch {
            close: atom.clone(),
        };
        verdict = failed_at(practice, &finding);
        findings.push((items.len(), finding));
    }

    // A procedure with no steps at all grades `asserted` — there is nothing to
    // re-run (`docs/spec/07-verdicts-and-llm-boundary.md` §1); `weakest` over an
    // empty chain would otherwise read the lattice top, `checked`.
    let grade = if grades.is_empty() {
        Grade::Asserted
    } else {
        Grade::weakest(grades)
    };

    let header_practice = proof.practice().map(ToOwned::to_owned);
    let shape = ProofShape {
        header_practice: header_practice.as_deref(),
        checked_practice: &practice.name.text,
        close: close.as_ref(),
        close_layout: (close_count, close_is_final),
        unknown_role,
        header_valid,
        surface_errors,
    };
    let (expected, expectation_met, checks) =
        weigh_expectation(&mut findings, items.len(), &shape, &verdict);
    let practice_name = header_practice.map_or_else(
        || practice.name.clone(),
        |t| Ident {
            text: t,
            span: proof.name.span,
        },
    );

    ProofReport {
        proof: proof.name.clone(),
        speaker,
        practice: practice_name,
        steps,
        findings: findings.into_iter().map(|(_, f)| f).collect(),
        verdict,
        expected,
        expectation_met,
        grade,
        checks,
    }
}

/// Weigh the proof's well-formedness and its declared expectation against the
/// reached verdict, pushing any meta finding. Returns the (possibly-defaulted)
/// expectation, whether the reached verdict met it, and whether the complete
/// proof checks (well-formed AND expectation met).
///
/// Well-formedness is distinct from the casting's verdict — a proof can hold and
/// still not check because it named the wrong practice, carried no close, or wrote
/// an `expect` token that is not a verdict.
struct ProofShape<'a> {
    header_practice: Option<&'a str>,
    checked_practice: &'a str,
    close: Option<&'a (String, Expectation)>,
    close_layout: (usize, bool),
    unknown_role: Option<(&'a str, &'a str)>,
    header_valid: bool,
    surface_errors: Vec<String>,
}

fn weigh_expectation(
    findings: &mut Vec<(usize, ProofFinding)>,
    end: usize,
    shape: &ProofShape<'_>,
    verdict: &ProofVerdict,
) -> (Expectation, bool, bool) {
    let mut well_formed = true;
    let (close_count, close_is_final) = shape.close_layout;
    if !shape.header_valid {
        findings.push((end, ProofFinding::InvalidHeaderLayout));
        well_formed = false;
    }
    // L3: the header `in <practice>` must name the practice we checked against.
    if let Some(named) = shape.header_practice
        && named != shape.checked_practice
    {
        findings.push((
            end,
            ProofFinding::PracticeMismatch {
                header: named.to_owned(),
                checked: shape.checked_practice.to_owned(),
            },
        ));
        well_formed = false;
    }
    if let Some((role, actor)) = shape.unknown_role {
        findings.push((
            end,
            ProofFinding::UnknownRole {
                role: role.to_owned(),
                actor: actor.to_owned(),
            },
        ));
        well_formed = false;
    }
    if !shape.surface_errors.is_empty() {
        findings.push((
            end,
            ProofFinding::MalformedBody {
                diagnostics: shape.surface_errors.clone(),
            },
        ));
        well_formed = false;
    }
    if close_count != 1 || !close_is_final {
        findings.push((
            end,
            ProofFinding::InvalidCloseLayout {
                count: close_count,
                final_operation: close_is_final,
            },
        ));
        well_formed = false;
    }
    // M6: a casting with no close has no conclusion to judge.
    let expected = if let Some((_, e)) = shape.close {
        e.clone()
    } else {
        findings.push((end, ProofFinding::MissingClose));
        well_formed = false;
        Expectation::Holds
    };
    // M5: an unrecognized `expect` token is refused, never coerced to holds.
    if let Expectation::Unknown(token) = &expected {
        findings.push((
            end,
            ProofFinding::UnknownExpectation {
                token: token.clone(),
            },
        ));
        well_formed = false;
    }
    let met = meets(&expected, verdict);
    // L2: emit the unmet finding in EVERY unmet direction — a held `expect blocked`
    // or a blocked `expect holds`, not only a validated `fails-at`. An unrecognized
    // token is already reported by UnknownExpectation.
    if !met && !matches!(expected, Expectation::Unknown(_)) {
        findings.push((
            end,
            ProofFinding::ExpectationUnmet {
                expected: expected.clone(),
                got: verdict.clone(),
            },
        ));
    }
    (expected, met, well_formed && met)
}

/// The running state of a ledger replay, before the verdict is weighed.
struct LedgerRun {
    steps: Vec<StepVerdict>,
    /// Findings paired with the ledger position they fired at, so the verdict can
    /// pick the *earliest* failure regardless of when it was computed.
    findings: Vec<(usize, ProofFinding)>,
    grades: Vec<Grade>,
    established: BTreeSet<String>,
    /// Per-resource delta sequence, each tagged with the step label that applied it.
    deltas: BTreeMap<String, Vec<(String, i64)>>,
    close: Option<(String, Expectation)>,
}

/// Walk the reassembled proof forms in order, building the per-step verdicts, the
/// procedure-chain grades, the established atoms, and the per-resource deltas.
#[expect(clippy::too_many_lines)]
fn run_ledger(
    items: &[Item<'_>],
    practice: &PracticeDef,
    scene: &SceneProjection,
    resources: &BTreeSet<&str>,
    cycles: &BTreeSet<&str>,
) -> LedgerRun {
    let mut steps: Vec<StepVerdict> = Vec::new();
    let mut findings: Vec<(usize, ProofFinding)> = Vec::new();
    let mut grades: Vec<Grade> = Vec::new();
    let mut premises: BTreeMap<String, Premise> = BTreeMap::new();
    let mut seen_labels: BTreeSet<String> = BTreeSet::new();
    let mut established: BTreeSet<String> = BTreeSet::new();
    let mut deltas: BTreeMap<String, Vec<(String, i64)>> = BTreeMap::new();
    let mut close: Option<(String, Expectation)> = None;

    for (pos, item) in items.iter().enumerate() {
        match item.op {
            ProofOp::Premise(premise) => {
                let premise = Premise {
                    label: premise.label.clone(),
                    atom: premise.atom.clone(),
                    grade: premise_grade(&premise.mods),
                    kind: classify_premise(premise, resources, cycles),
                };
                run_premise(
                    premise,
                    false,
                    scene,
                    pos,
                    &mut grades,
                    &mut seen_labels,
                    &mut established,
                    &mut deltas,
                    &mut findings,
                    &mut steps,
                    &mut premises,
                );
            }
            ProofOp::Assume(assume) => {
                let premise = Premise {
                    label: assume.label.clone(),
                    atom: assume.atom.clone(),
                    grade: Grade::Asserted,
                    kind: PremiseKind::Assumed,
                };
                run_premise(
                    premise,
                    true,
                    scene,
                    pos,
                    &mut grades,
                    &mut seen_labels,
                    &mut established,
                    &mut deltas,
                    &mut findings,
                    &mut steps,
                    &mut premises,
                );
            }
            ProofOp::Step(step) => {
                let (label, verdict, finding) =
                    check_step(step, item.text, practice, &seen_labels, &mut established);
                seen_labels.insert(label);
                if let Some(finding) = finding {
                    findings.push((pos, finding));
                }
                grades.push(Grade::Checked);
                steps.push(verdict);
            }
            ProofOp::Resource(resource) => {
                let keyword = resource.keyword.keyword();
                let label = resource.label.as_str();
                grades.push(Grade::Checked);
                match resource.keyword {
                    ResourceKeyword::Spend | ResourceKeyword::Release => {
                        let (verdict, delta, finding) = spend_step(keyword, label, &premises);
                        if let Some((name, amount)) = delta {
                            deltas
                                .entry(name)
                                .or_default()
                                .push((label.to_owned(), amount));
                        }
                        if let Some(finding) = finding {
                            findings.push((pos, finding));
                        }
                        steps.push(verdict);
                    }
                    // Reveal surfaces a conserved unit already implied (net zero);
                    // redistribute moves one between accounts (net zero). Neither
                    // mints nor burns, so neither perturbs the multiset.
                    ResourceKeyword::Reveal | ResourceKeyword::Redistribute => {
                        let (verdict, finding) = neutral_resource_step(keyword, label, &premises);
                        if let Some(finding) = finding {
                            findings.push((pos, finding));
                        }
                        steps.push(verdict);
                    }
                }
            }
            ProofOp::Close(c) => {
                let expect = c
                    .verdict
                    .as_deref()
                    .map_or(Expectation::Holds, parse_expectation);
                close = Some((c.atom.clone(), expect));
                steps.push(StepVerdict {
                    label: "close".to_owned(),
                    form: ProofForm::Close,
                    ok: true,
                    grade: Grade::Checked,
                    note: c.atom.clone(),
                });
            }
            ProofOp::Malformed(m) => steps.push(StepVerdict {
                label: m.keyword.clone(),
                form: ProofForm::Other,
                ok: true,
                grade: Grade::Checked,
                note: item.text.to_owned(),
            }),
        }
    }

    LedgerRun {
        steps,
        findings,
        grades,
        established,
        deltas,
        close,
    }
}

/// Fold one premise/assumption into the running ledger state: its grade, its
/// established atom, any resource delta, and its step verdict/finding.
#[expect(clippy::too_many_arguments)]
fn run_premise(
    premise: Premise,
    is_assume: bool,
    scene: &SceneProjection,
    pos: usize,
    grades: &mut Vec<Grade>,
    seen_labels: &mut BTreeSet<String>,
    established: &mut BTreeSet<String>,
    deltas: &mut BTreeMap<String, Vec<(String, i64)>>,
    findings: &mut Vec<(usize, ProofFinding)>,
    steps: &mut Vec<StepVerdict>,
    premises: &mut BTreeMap<String, Premise>,
) {
    grades.push(premise.grade);
    seen_labels.insert(premise.label.clone());
    let outcome = handle_premise(&premise, is_assume, scene);
    if let Some(atom) = outcome.establish {
        established.insert(atom);
    }
    if let Some((name, amount)) = outcome.delta {
        deltas
            .entry(name)
            .or_default()
            .push((premise.label.clone(), amount));
    }
    if let Some(finding) = outcome.finding {
        findings.push((pos, finding));
    }
    steps.push(outcome.verdict);
    premises.insert(premise.label.clone(), premise);
}

/// The effect of processing one premise: its step verdict, any finding it raised,
/// a resource delta to fold into the accounts, and an atom to add to the
/// established set (available for the close to follow from).
struct PremiseOutcome {
    verdict: StepVerdict,
    finding: Option<ProofFinding>,
    delta: Option<(String, i64)>,
    establish: Option<String>,
}

/// One premise's effect on the run: `(ok, note, finding, resource-delta,
/// atom-to-establish)`. The tuple the per-kind checkers return and
/// [`handle_premise`] folds into a [`PremiseOutcome`].
type PremiseEffect = (
    bool,
    String,
    Option<ProofFinding>,
    Option<(String, i64)>,
    Option<String>,
);

/// Check a timing premise `phase(<cycle>, <spoke>)`: an undeclared spoke is a
/// bad instantiation (L4), caught *before* the phase comparison — a phase the
/// cycle cannot reach would otherwise read as merely the wrong phase. A spoke in
/// force holds; any other established (or unestablished) phase is wrong-phase (L3).
fn timing_effect(
    premise: &Premise,
    cycle: &str,
    spoke: &str,
    scene: &SceneProjection,
) -> PremiseEffect {
    if scene.is_declared_spoke(cycle, spoke) == Some(false) {
        return (
            false,
            format!("{spoke} is not a phase of {cycle}"),
            Some(ProofFinding::UnknownSpoke {
                label: premise.label.clone(),
                cycle: cycle.to_owned(),
                spoke: spoke.to_owned(),
            }),
            None,
            None,
        );
    }
    let actual = scene.phase_of(cycle).map(str::to_owned);
    if actual.as_deref() == Some(spoke) {
        (
            true,
            format!("{cycle} is {spoke}"),
            None,
            None,
            Some(normalize_atom(&premise.atom)),
        )
    } else {
        (
            false,
            format!("{cycle} is not {spoke}"),
            Some(ProofFinding::WrongPhase {
                label: premise.label.clone(),
                cycle: cycle.to_owned(),
                required: spoke.to_owned(),
                actual,
            }),
            None,
            None,
        )
    }
}

/// Check a world-fact premise against the pool: available establishes it; absent
/// fizzles (L2); denied rebounds (L2).
fn world_effect(premise: &Premise, scene: &SceneProjection) -> PremiseEffect {
    match scene.availability(&premise.atom) {
        PremiseStatus::Available => (
            true,
            "available in the pool".to_owned(),
            None,
            None,
            Some(normalize_atom(&premise.atom)),
        ),
        PremiseStatus::Absent => (
            false,
            "absent from the pool".to_owned(),
            Some(ProofFinding::PremiseAbsent {
                label: premise.label.clone(),
                atom: premise.atom.clone(),
            }),
            None,
            None,
        ),
        PremiseStatus::Contradicted => (
            false,
            "denied by the projection".to_owned(),
            Some(ProofFinding::PremiseContradicted {
                label: premise.label.clone(),
                atom: premise.atom.clone(),
            }),
            None,
            None,
        ),
    }
}

/// Fold a `spend`/`release` step: it must name a declared resource premise. A
/// spend applies a negative delta, a release a positive one; anything else is an
/// invalid-spend finding.
fn spend_step(
    keyword: &str,
    label: &str,
    premises: &BTreeMap<String, Premise>,
) -> (StepVerdict, Option<(String, i64)>, Option<ProofFinding>) {
    if let Some(PremiseKind::Resource { name, amount }) = premises.get(label).map(|p| &p.kind) {
        let signed = if keyword == "spend" {
            -*amount
        } else {
            *amount
        };
        (
            bookkeeping_step(label, keyword, true),
            Some((name.clone(), signed)),
            None,
        )
    } else {
        (
            bookkeeping_step(label, keyword, false),
            None,
            Some(ProofFinding::InvalidSpend {
                keyword: keyword.to_owned(),
                label: label.to_owned(),
            }),
        )
    }
}

/// Validate a net-zero resource operation. `reveal` and `redistribute` do not
/// alter the multiset, but they still have to name a resource unit already
/// present in the ledger.
fn neutral_resource_step(
    keyword: &str,
    label: &str,
    premises: &BTreeMap<String, Premise>,
) -> (StepVerdict, Option<ProofFinding>) {
    if matches!(
        premises.get(label).map(|premise| &premise.kind),
        Some(PremiseKind::Resource { .. })
    ) {
        (bookkeeping_step(label, keyword, true), None)
    } else {
        (
            bookkeeping_step(label, keyword, false),
            Some(ProofFinding::InvalidSpend {
                keyword: keyword.to_owned(),
                label: label.to_owned(),
            }),
        )
    }
}

/// Validate one premise against the scene: a resource draw folds into the
/// accounts; a timing claim checks the phase in force (L3 on mismatch); a world
/// fact checks the pool (L2 absent/contradicted); a supposition is taken as given.
fn handle_premise(premise: &Premise, is_assume: bool, scene: &SceneProjection) -> PremiseOutcome {
    let form = if is_assume {
        ProofForm::Assume
    } else {
        ProofForm::Premise
    };
    let (ok, note, finding, delta, establish) = match &premise.kind {
        PremiseKind::Assumed => (
            true,
            "assumed (fork-local)".to_owned(),
            None,
            None,
            Some(normalize_atom(&premise.atom)),
        ),
        PremiseKind::Resource { name, amount } => (
            true,
            format!("draws {amount} {name}"),
            None,
            Some((name.clone(), *amount)),
            None,
        ),
        PremiseKind::Timing { cycle, spoke } => timing_effect(premise, cycle, spoke, scene),
        PremiseKind::World => world_effect(premise, scene),
    };
    PremiseOutcome {
        verdict: StepVerdict {
            label: premise.label.clone(),
            form,
            ok,
            grade: premise.grade,
            note,
        },
        finding,
        delta,
        establish,
    }
}

/// A bookkeeping step (`spend`/`release`/`reveal`/`redistribute`). A well-formed
/// one stands (the resource algebra judges the accounts separately); `ok: false`
/// marks a spend/release that named no resource premise.
fn bookkeeping_step(label: &str, keyword: &str, ok: bool) -> StepVerdict {
    let form = match keyword {
        "spend" => ProofForm::Spend,
        "release" => ProofForm::Release,
        "reveal" => ProofForm::Reveal,
        _ => ProofForm::Redistribute,
    };
    let note = if ok {
        format!("{keyword} {label}")
    } else {
        format!("{keyword} {label}: not a resource premise")
    };
    StepVerdict {
        label: label.to_owned(),
        form,
        ok,
        grade: Grade::Checked,
        note,
    }
}

/// Replay each resource's full delta sequence through its algebra law, lifting
/// every breach into an L6 finding tagged with the step that applied the offending
/// delta. An affine resource with a cap the scene did not resolve is deferred.
fn replay_resources(
    practice: &PracticeDef,
    scene: &SceneProjection,
    deltas: &BTreeMap<String, Vec<(String, i64)>>,
    items: &[Item<'_>],
) -> Vec<(usize, ProofFinding)> {
    let mut findings = Vec::new();
    for (name, seq) in deltas {
        let Some(decl) = practice.resources.iter().find(|r| &r.name.text == name) else {
            continue;
        };
        let Some(kind) = &decl.kind else { continue };
        let flat: Vec<Delta> = seq
            .iter()
            .map(|(_, amount)| Delta {
                resource: Ident {
                    text: name.clone(),
                    span: decl.span,
                },
                amount: *amount,
            })
            .collect();
        let violation = match kind {
            ResourceKind::Linear { conserved: false } => check_linear(&flat),
            ResourceKind::Linear { conserved: true } => check_conserved(&flat),
            ResourceKind::GradedMonotone => check_graded_monotone(&flat, decl.direction),
            ResourceKind::AffineBounded => match resolve_cap(scene, name, decl.cap.as_deref()) {
                Some(cap) => check_affine_bounded(&flat, cap),
                None => None,
            },
        };
        if let Some(v) = violation {
            let (at_label, at_pos) = seq.get(v.at).map_or_else(
                || (name.clone(), items.len()),
                |(label, _)| (label.clone(), position_of(items, label)),
            );
            findings.push((
                at_pos,
                ProofFinding::ResourceViolation {
                    resource: name.clone(),
                    code: v.code,
                    message: v.message,
                    at: at_label,
                },
            ));
        }
    }
    findings
}

// --- proof-form reading -----------------------------------------------------

/// One recognized proof-block operation from the typed interior: its keyword
/// discriminant, the typed op, and the retained clause text (used only to render
/// a malformed operation's note).
struct Item<'a> {
    keyword: &'a str,
    op: &'a ProofOp,
    text: &'a str,
}

/// Collect the recognized proof operations from the typed interior.
///
/// Every interior line that opened with a proof keyword carries a typed
/// [`ProofOp`] (`op.is_some()`); a line opening with no keyword is skipped here
/// (it is refused by [`proof_structural_errors`] instead). A malformed operation
/// is kept — its keyword still counts (e.g. toward the close layout).
fn reassemble(body: &ProofBody) -> Vec<Item<'_>> {
    body.items
        .iter()
        .filter_map(|clause| {
            clause.op.as_ref().map(|op| Item {
                keyword: op.keyword(),
                op,
                text: clause.clause.text.as_str(),
            })
        })
        .collect()
}

/// The structural well-formedness check over the typed proof interior — the
/// replacement for the retired `validate_proof_decl` format-and-reparse round
/// trip. A line the front end could not read as a valid proof operation — one
/// opening with no proof keyword, or a recognized keyword whose arguments failed
/// the strict per-line check ([`ProofOp::Malformed`]) — is refused here. A
/// well-formed body returns no errors (charter koan 4: nothing is dropped, the
/// malformed line is named).
fn proof_structural_errors(body: &ProofBody) -> Vec<String> {
    body.items
        .iter()
        .filter_map(|clause| match &clause.op {
            None if !clause.clause.text.trim().is_empty() => Some(format!(
                "a proof line cannot be read as an operation: `{}` — write premise, assume, \
                 step, spend, release, redistribute, reveal, or close",
                clause.clause.text
            )),
            Some(ProofOp::Malformed(op)) => Some(format!(
                "a `{}` operation is malformed: `{}`",
                op.keyword, clause.clause.text
            )),
            _ => None,
        })
        .collect()
}

/// How a premise stands relative to the practice's declared law-set.
enum PremiseKind {
    /// A world fact, validated against the scene pool.
    World,
    /// A cycle-phase timing claim `phase(<cycle>, <spoke>)`, validated against the
    /// phase in force.
    Timing {
        /// The cycle named.
        cycle: String,
        /// The phase the casting requires.
        spoke: String,
    },
    /// A resource draw `<resource>(<n>)`, folded into the account.
    Resource {
        /// The resource named.
        name: String,
        /// The signed magnitude drawn.
        amount: i64,
    },
    /// A fork-local supposition (`assume`).
    Assumed,
}

/// A premise as the ledger run reads it: its label, atom, contributed grade, and
/// how it stands against the practice's law-set.
struct Premise {
    label: String,
    atom: String,
    grade: Grade,
    kind: PremiseKind,
}

/// Classify a premise as a resource draw, a timing claim, or a world fact, from
/// the front end's pre-parsed atom and phase-comparison. The practice's declared
/// resources and in-scope cycles are the runtime data the front end could not
/// know, so the classification stays here — but it reads the typed
/// [`ProofPremise`] rather than re-parsing the atom text.
fn classify_premise(
    premise: &ProofPremise,
    resources: &BTreeSet<&str>,
    cycles: &BTreeSet<&str>,
) -> PremiseKind {
    if let Some(phase) = &premise.phase_eq
        && cycles.contains(phase.cycle.as_str())
    {
        return PremiseKind::Timing {
            cycle: phase.cycle.clone(),
            spoke: phase.spoke.clone(),
        };
    }
    let Some(expr) = &premise.atom_expr else {
        return PremiseKind::World;
    };
    let pred = expr.pred.text.as_str();
    if resources.contains(pred) {
        let amount = expr.args.first().and_then(term_int).unwrap_or(1);
        return PremiseKind::Resource {
            name: pred.to_owned(),
            amount,
        };
    }
    if pred == "phase"
        && expr.args.len() == 2
        && let Some(cycle) = term_sym(&expr.args[0])
        && cycles.contains(cycle.as_str())
        && let Some(spoke) = term_sym(&expr.args[1])
    {
        return PremiseKind::Timing { cycle, spoke };
    }
    PremiseKind::World
}

/// The grade a premise contributes, read from its provenance tokens
/// (`docs/spec/07-verdicts-and-llm-boundary.md`): a D-cited or scene-read premise
/// re-derives (`checked`); a stored model judgment is `attested`; a bare
/// supposition is `asserted` — nothing to re-run.
fn premise_grade(mods: &[String]) -> Grade {
    if mods.iter().any(|m| m == "attested") {
        Grade::Attested
    } else if mods.iter().any(|m| m == "tested") {
        Grade::Tested
    } else if mods.iter().any(|m| m == "asserted") {
        Grade::Asserted
    } else if mods.iter().any(|m| {
        matches!(
            m.as_str(),
            "cited" | "from-scene" | "observed" | "checked" | "drawn-against"
        )
    }) {
        Grade::Checked
    } else {
        Grade::Asserted
    }
}

/// Check a `step …` operation from the typed interior. Returns the step's label,
/// its verdict, and any finding. A step reaches here only when the front end read
/// it as a valid operation, so the bindings/argument lists are already parsed.
/// `clause_text` is the retained normalized line, used to render a custom step's
/// note exactly as the surface wrote it.
fn check_step(
    step: &ProofStep,
    clause_text: &str,
    practice: &PracticeDef,
    seen_labels: &BTreeSet<String>,
    established: &mut BTreeSet<String>,
) -> (String, StepVerdict, Option<ProofFinding>) {
    match step {
        ProofStep::Instantiate {
            label,
            form,
            bindings,
        } => {
            let mut verdict = StepVerdict {
                label: label.clone(),
                form: ProofForm::Instantiate,
                ok: true,
                grade: Grade::Checked,
                note: format!("instantiate {form}"),
            };
            let Some(rule) = practice.rules.iter().find(|r| r.form.text == *form) else {
                verdict.ok = false;
                verdict.note = format!("no such form '{form}'");
                return (
                    label.clone(),
                    verdict,
                    Some(ProofFinding::UnknownForm {
                        form: form.clone(),
                        at: label.clone(),
                    }),
                );
            };
            let bindings: Vec<(String, String)> = bindings
                .iter()
                .map(|binding| (binding.name.clone(), binding.value.clone()))
                .collect();
            if let Some(binding) = invalid_bindings(practice, rule, &bindings) {
                verdict.ok = false;
                verdict.note = format!("'{form}' has an invalid binding: {binding}");
                return (
                    label.clone(),
                    verdict,
                    Some(ProofFinding::BadInstantiation {
                        form: form.clone(),
                        binding,
                        at: label.clone(),
                    }),
                );
            }
            (label.clone(), verdict, None)
        }
        ProofStep::Chain {
            label,
            joins,
            law,
            conclusion,
        } => {
            let law_ok = law
                .as_deref()
                .is_some_and(|l| practice.rules.iter().any(|r| r.form.text == l));
            let joins_ok = joins
                .iter()
                .all(|j| seen_labels.contains(j) || established.contains(&normalize_atom(j)));
            let ok = law_ok && joins_ok;
            if let Some(atom) = conclusion {
                established.insert(normalize_atom(atom));
            }
            let note = if ok {
                law.as_ref()
                    .map_or_else(|| "chain".to_owned(), |l| format!("chain by {l}"))
            } else {
                "chain not licensed".to_owned()
            };
            let verdict = StepVerdict {
                label: label.clone(),
                form: ProofForm::Chain,
                ok,
                grade: Grade::Checked,
                note,
            };
            let finding = (!ok).then(|| ProofFinding::InvalidChain {
                law: law.clone().unwrap_or_else(|| "<unknown>".to_owned()),
                at: label.clone(),
            });
            (label.clone(), verdict, finding)
        }
        ProofStep::Custom {
            label,
            operation,
            args,
        } => custom_step(label, operation, args, clause_text, practice),
    }
}

fn custom_step(
    label: &str,
    operation: &str,
    args: &[String],
    clause_text: &str,
    practice: &PracticeDef,
) -> (String, StepVerdict, Option<ProofFinding>) {
    let rule = practice
        .rules
        .iter()
        .find(|rule| rule.form.text == operation);
    let invalid = rule.and_then(|rule| {
        let bindings = rule
            .args
            .iter()
            .zip(args)
            .map(|(arg, value)| (arg.text.clone(), value.clone()))
            .collect::<Vec<_>>();
        if args.len() == rule.args.len() {
            invalid_bindings(practice, rule, &bindings)
        } else {
            Some(format!(
                "expected {} argument(s), got {}",
                rule.args.len(),
                args.len()
            ))
        }
    });
    let declared = rule.is_some();
    let ok = declared && invalid.is_none();
    let verdict = StepVerdict {
        label: label.to_owned(),
        form: ProofForm::Other,
        ok,
        grade: Grade::Checked,
        note: if let Some(problem) = &invalid {
            format!("{operation}: {problem}")
        } else {
            custom_step_note(clause_text, label).to_owned()
        },
    };
    let finding = if declared {
        invalid.map(|binding| ProofFinding::BadInstantiation {
            form: operation.to_owned(),
            binding,
            at: label.to_owned(),
        })
    } else {
        Some(ProofFinding::UnknownForm {
            form: operation.to_owned(),
            at: label.to_owned(),
        })
    };
    (label.to_owned(), verdict, finding)
}

/// The `<operation> [<args…>]` body of a custom step, read straight off the
/// retained line (`step <label> <operation> …`) so the step note round-trips the
/// surface argument text exactly, quotes and comma spacing included.
fn custom_step_note<'a>(clause_text: &'a str, label: &str) -> &'a str {
    clause_text
        .strip_prefix("step ")
        .and_then(|rest| rest.strip_prefix(label))
        .map_or(clause_text, str::trim_start)
}

fn invalid_bindings(
    practice: &PracticeDef,
    rule: &crate::practice::RuleDef,
    bindings: &[(String, String)],
) -> Option<String> {
    let declared = rule
        .args
        .iter()
        .map(|arg| arg.text.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for (key, value) in bindings {
        if !declared.contains(key.as_str()) {
            return Some(format!("unknown parameter '{key}'"));
        }
        if !seen.insert(key.as_str()) {
            return Some(format!("duplicate parameter '{key}'"));
        }
        if let Some(sort) = practice
            .sorts
            .iter()
            .find(|sort| sort.name.text == *key && !sort.variants.is_empty())
            && !sort.variants.iter().any(|variant| variant.text == *value)
        {
            return Some(format!(
                "{key} = {value} is outside the declared kind '{}'",
                sort.name.text
            ));
        }
    }
    rule.args
        .iter()
        .find(|arg| !seen.contains(arg.text.as_str()))
        .map(|arg| format!("missing parameter '{}'", arg.text))
}

fn parse_expectation(verdict: &str) -> Expectation {
    let (head, tail) = split_first_word(verdict);
    match head {
        "holds" | "" => Expectation::Holds,
        "blocked" => Expectation::Blocked,
        "fails-at" => Expectation::FailsAt(first_word(tail).to_owned()),
        other => Expectation::Unknown(other.to_owned()),
    }
}

// --- verdict helpers --------------------------------------------------------

/// Build a [`ProofVerdict::FailedAt`] from a finding, resolving the practice's
/// refining error name for the finding's class.
fn failed_at(practice: &PracticeDef, finding: &ProofFinding) -> ProofVerdict {
    let class = finding.class().unwrap_or(StructuralClass::L7);
    let (error, at) = match finding {
        ProofFinding::WrongPhase { label, cycle, .. } => {
            (refining_error(practice, class, Some(cycle)), label.clone())
        }
        ProofFinding::ResourceViolation { resource, at, .. } => {
            (refining_error(practice, class, Some(resource)), at.clone())
        }
        ProofFinding::PremiseAbsent { label, .. } => (
            refining_error(practice, class, Some("absent")),
            label.clone(),
        ),
        ProofFinding::PremiseContradicted { label, .. } => (
            refining_error(practice, class, Some("contradicted")),
            label.clone(),
        ),
        ProofFinding::UnknownForm { at, .. }
        | ProofFinding::BadInstantiation { at, .. }
        | ProofFinding::InvalidChain { at, .. } => {
            (refining_error(practice, class, None), at.clone())
        }
        // A bad spoke is the phase equivalent of a bad instantiation (L4); it binds
        // by class, like the-turning's `mislaid-leaf`.
        ProofFinding::UnknownSpoke { label, .. } => {
            (refining_error(practice, class, None), label.clone())
        }
        // A malformed spend has no diegetic error to refine to — the practice does
        // not declare "you spent a non-resource".
        ProofFinding::InvalidSpend { label, .. } => (None, label.clone()),
        ProofFinding::CloseMismatch { .. } => {
            (refining_error(practice, class, None), "close".to_owned())
        }
        // Meta findings never drive the structural verdict (they are added after it
        // is fixed); this arm keeps the match total.
        ProofFinding::ExpectationUnmet { .. }
        | ProofFinding::UnknownExpectation { .. }
        | ProofFinding::MissingClose
        | ProofFinding::PracticeMismatch { .. }
        | ProofFinding::UnknownRole { .. }
        | ProofFinding::InvalidHeaderLayout
        | ProofFinding::MalformedBody { .. }
        | ProofFinding::InvalidCloseLayout { .. } => (None, "close".to_owned()),
    };
    ProofVerdict::FailedAt { class, error, at }
}

/// Find the practice's declared error that refines `class` and binds `hint`.
///
/// When a breach carries a specific binding — the resource it overspent, the cycle
/// it mistimed, the L2 discriminant *absent*/*contradicted* — the declared error
/// MUST name that binding, or **none binds** (`None`): a `debt` breach never
/// borrows `strain`'s `overload` just because they share class L6. Only a
/// binding-free class (`hint: None`, e.g. L1/L4/L5/L7 with one error per class)
/// falls back to the sole class member.
fn refining_error(
    practice: &PracticeDef,
    class: StructuralClass,
    hint: Option<&str>,
) -> Option<String> {
    let of_class = practice.errors.iter().filter(|e| e.class == Some(class));
    match hint {
        Some(hint) => of_class
            .filter(|e| {
                e.over_cycles.iter().any(|c| c.text == hint)
                    || e.on_resource.as_ref().is_some_and(|r| r.text == hint)
                    || e.discriminant.contains(hint)
            })
            .map(|e| e.name.text.clone())
            .next(),
        None => of_class.map(|e| e.name.text.clone()).next(),
    }
}

/// Whether a reached verdict meets a declared expectation.
fn meets(expected: &Expectation, verdict: &ProofVerdict) -> bool {
    match (expected, verdict) {
        (Expectation::Holds, ProofVerdict::Holds)
        | (Expectation::Blocked, ProofVerdict::FailedAt { .. }) => true,
        (Expectation::FailsAt(name), ProofVerdict::FailedAt { error, .. }) => {
            error.as_deref() == Some(name.as_str())
        }
        _ => false,
    }
}

fn resolve_cap(scene: &SceneProjection, resource: &str, decl_cap: Option<&str>) -> Option<i64> {
    scene
        .cap_of(resource)
        .or_else(|| decl_cap.and_then(parse_cap_int))
}

/// Parse an integer affine cap from a declared cap phrase — `9` or `9 leaves`
/// resolve; a symbolic `body-limit(caster)` does not (returns [`None`]).
fn parse_cap_int(cap: &str) -> Option<i64> {
    cap.split_whitespace().find_map(|w| w.parse::<i64>().ok())
}

// --- small parsing helpers --------------------------------------------------

fn split_first_word(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    match text.split_once(char::is_whitespace) {
        Some((first, rest)) => (first, rest.trim_start()),
        None => (text, ""),
    }
}

fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or_default()
}

/// The index of the reassembled item labeled `label`, so a resource breach binds
/// to the step where the offending delta was applied.
fn position_of(items: &[Item<'_>], label: &str) -> usize {
    items
        .iter()
        .position(|it| item_label(it.op) == Some(label))
        .unwrap_or(items.len())
}

/// The label a proof operation is keyed by in the ledger run (its premise/step
/// label). A close and a malformed operation carry no such label.
fn item_label(op: &ProofOp) -> Option<&str> {
    match op {
        ProofOp::Premise(p) => Some(p.label.as_str()),
        ProofOp::Assume(a) => Some(a.label.as_str()),
        ProofOp::Step(
            ProofStep::Instantiate { label, .. }
            | ProofStep::Chain { label, .. }
            | ProofStep::Custom { label, .. },
        ) => Some(label.as_str()),
        ProofOp::Resource(r) => Some(r.label.as_str()),
        ProofOp::Close(_) | ProofOp::Malformed(_) => None,
    }
}

fn term_int(term: &Term) -> Option<i64> {
    match term {
        Term::Int(n) => Some(*n),
        _ => None,
    }
}

fn term_sym(term: &Term) -> Option<String> {
    match term {
        Term::Sym(id) | Term::Var(id) => Some(id.text.clone()),
        _ => None,
    }
}

fn describe_expectation(expected: &Expectation) -> String {
    match expected {
        Expectation::Holds => "to hold".to_owned(),
        Expectation::Blocked => "to be blocked".to_owned(),
        Expectation::FailsAt(name) => format!("to fail at {name}"),
        Expectation::Unknown(token) => format!("'{token}' (not a verdict)"),
    }
}

fn describe_verdict(verdict: &ProofVerdict) -> String {
    match verdict {
        ProofVerdict::Holds => "a clean hold".to_owned(),
        ProofVerdict::FailedAt { class, error, at } => {
            let named = error.as_deref().unwrap_or("no declared error");
            format!("a failure at {at} ({} / {named})", class.meaning())
        }
    }
}
