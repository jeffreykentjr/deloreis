//! The evaluable world program: lowering, the annotated fixpoint, and query.
//!
//! A [`World`] is produced by [`World::lower`]ing a document, [`World::fixpoint`]
//! computes the symbolic support of every derivable atom *once*, and queries
//! read that saturation under a valuation. Because provenance is precomputed and
//! valuations are applied afterward, a fork re-evaluates through
//! [`World::eval_under`] with **no** re-lowering and no re-fixpoint.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use dli_ast::annot::{Annotation, CondExpr, ProvenanceMarker};
use dli_ast::decl::{AttrDecl, Decl, Document, EntityKind, OptionClause, StateDecl, TimeClause};
use dli_ast::ident::Ident;
use dli_ast::literal::BodyLiteral;
use dli_ast::term::{AtomExpr, Term, TimeRef};
use dli_core::ann::{Ann, DefeaterConditions, Status, Theory};
use dli_core::atom::{Atom, Interner};
use dli_core::grade::Grade;
use dli_core::posbool::{Monomial, PosBool};
use dli_core::valuation::Valuation;
use dli_core::verdict::Verdict;
use dli_time::chronology::{Chronology, TimePoint};
use dli_time::cycle::Cycle;

use crate::error::EvalError;
use crate::query::{Answer, Query, QueryRow};
use crate::strata::Strata;

/// A ground atom: a predicate applied to fully-ground argument renderings. This
/// is the identity of a fact or a derived tuple in the relation.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct GroundAtom {
    /// The predicate name.
    pub pred: String,
    /// The ground argument renderings, in order.
    pub args: Vec<String>,
}

impl GroundAtom {
    /// The canonical `pred(a, b)` rendering.
    #[must_use]
    pub fn render(&self) -> String {
        format!("{}({})", self.pred, self.args.join(", "))
    }

    /// Ground an atom pattern whose arguments are all constants, or `None` if
    /// any argument is a variable.
    #[must_use]
    pub fn from_pattern(pattern: &AtomExpr) -> Option<GroundAtom> {
        let mut args = Vec::with_capacity(pattern.args.len());
        for term in &pattern.args {
            args.push(constant_render(term)?);
        }
        Some(GroundAtom {
            pred: pattern.pred.text.clone(),
            args,
        })
    }
}

/// A lowered extensional fact: a ground atom with its annotation and grade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Fact {
    /// The ground atom asserted.
    pub atom: GroundAtom,
    /// Its support and defeater polynomials.
    pub ann: Ann,
    /// The grade at which it is known.
    pub grade: Grade,
}

/// A lowered derivation rule: an (unground) head and a conjunctive body of
/// polarity-aware literals, carrying the rule's own annotation, which multiplies
/// into every derivation passing through it.
///
/// A **negated** body literal (`not <atom>`) tests only against the certain
/// layer and poisons the conclusion's defeater polynomial (`docs/spec/02-semantics.md
/// §4`); it binds no variables, so every variable it names must also appear in a
/// positive body atom (range restriction, `08-reasoning.md §1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Rule {
    /// The rule name.
    pub name: String,
    /// The head atom pattern the rule derives.
    pub head: AtomExpr,
    /// The conjunctive body literals (positive atoms and negated `not` literals).
    pub body: Vec<BodyLiteral>,
    /// The rule's own annotation.
    pub ann: Ann,
    /// The rule's grade.
    pub grade: Grade,
}

/// Per-monomial grade storage: the grade of the derivation that introduced each
/// support monomial = min(rule.grade, meet of body atom grades). Keyed by the
/// same monomials present in `Derived::support`. The grade of an atom under a
/// valuation is the join over ν-true monomials' stored grades — no laundering
/// possible.
///
/// **Empty for old serialised data.** Missing grades fall back to
/// `Grade::Asserted` (conservative, not over-claiming).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonomialGrades {
    by_monomial: BTreeMap<Monomial, Grade>,
}

/// Wire form: `[{path: <single-monomial PosBool>, grade: "checked"}, ...]`
#[derive(Serialize, Deserialize, JsonSchema)]
struct GradePairWire {
    path: PosBool,
    grade: Grade,
}

impl MonomialGrades {
    /// No per-monomial grades.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether there are no stored grades.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_monomial.is_empty()
    }

    /// Get the grade stored for a monomial, or `None`.
    #[must_use]
    pub fn get(&self, monomial: &Monomial) -> Option<Grade> {
        self.by_monomial.get(monomial).copied()
    }

    /// Set (or upgrade via `join_alt`) the grade for a monomial. Multiple
    /// derivations of the same monomial keep the strongest alternative.
    pub fn set(&mut self, monomial: Monomial, grade: Grade) {
        self.by_monomial
            .entry(monomial)
            .and_modify(|existing| *existing = Grade::join_alt([*existing, grade]))
            .or_insert(grade);
    }

    /// Remove any grade entry for a monomial (used when absorption drops it).
    pub fn remove(&mut self, monomial: &Monomial) {
        self.by_monomial.remove(monomial);
    }

    /// The entries, in canonical monomial order.
    pub fn iter(&self) -> impl Iterator<Item = (&Monomial, Grade)> + '_ {
        self.by_monomial.iter().map(|(m, g)| (m, *g))
    }

    /// The weakest (minimum) grade across all stored entries, or
    /// `Grade::Asserted` if empty. Used by `join_body` for the conservative
    /// body-atom grade contribution.
    #[must_use]
    pub fn weakest_entry(&self) -> Grade {
        self.by_monomial
            .values()
            .copied()
            .min()
            .unwrap_or(Grade::Asserted)
    }
}

impl Serialize for MonomialGrades {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let pairs: Vec<GradePairWire> = self
            .by_monomial
            .iter()
            .map(|(monomial, grade)| GradePairWire {
                path: PosBool(std::iter::once(monomial.clone()).collect()),
                grade: *grade,
            })
            .collect();
        pairs.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MonomialGrades {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let pairs = Vec::<GradePairWire>::deserialize(deserializer)?;
        let mut by_monomial = BTreeMap::new();
        for pair in pairs {
            let Some(monomial) = pair.path.monomials().next() else {
                return Err(D::Error::custom("grade pair with empty path"));
            };
            by_monomial.insert(monomial.clone(), pair.grade);
        }
        Ok(MonomialGrades { by_monomial })
    }
}

impl JsonSchema for MonomialGrades {
    fn schema_name() -> Cow<'static, str> {
        "MonomialGrades".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "array",
            "items": <GradePairWire as JsonSchema>::json_schema(generator),
        })
    }
}

/// A saturated relation row: a ground atom with its computed support, defeaters,
/// and grade. Status is *not* stored — it is evaluated at query time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Derived {
    /// The ground atom.
    pub atom: GroundAtom,
    /// Its support polynomial `P⁺`.
    pub support: PosBool,
    /// Its atom-global defeater polynomial `P⁻`: the OR of every support
    /// monomial's defeater, kept for blame and citation. Which support monomial
    /// each defeater poisons lives in [`Derived::conditions`].
    pub defeaters: PosBool,
    /// The defeater conditioned per support monomial (`docs/spec/02-semantics.md
    /// §4`): the sound view that `status` reads. Empty (and omitted from the wire
    /// form) for every positive-only atom, so positive datalog is byte-identical.
    #[serde(default, skip_serializing_if = "DefeaterConditions::is_empty")]
    pub conditions: DefeaterConditions,
    /// Per-monomial grades: the grade of the derivation that introduced each
    /// support monomial = min(rule.grade, meet of body atom grades). Used by
    /// [`World::grade_under`] for valuation-dependent grading.
    #[serde(default, skip_serializing_if = "MonomialGrades::is_empty")]
    pub monomial_grades: MonomialGrades,
    /// Its grade.
    pub grade: Grade,
}

impl Derived {
    /// This row's annotation, carrying the per-monomial defeater conditions so
    /// `status`/`at_risk` evaluate the sound per-path defeat, not the atom-global
    /// pool. This is the seam every query and fork status read crosses.
    #[must_use]
    pub fn ann(&self) -> Ann {
        Ann::conditioned(self.support.clone(), self.conditions.clone())
    }
}

/// A non-fatal finding raised during lowering. Lowering never fails on one —
/// it fails the affected claim *closed* and records the finding here, so it can
/// surface through `check` without blocking the rest of the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LowerFinding {
    /// The stable finding code, e.g. `citation.unknown-decision`.
    pub code: String,
    /// A human-readable message in the world's terms.
    pub message: String,
}

/// A lowered, evaluable program together with its symbolic saturation.
///
/// Before [`World::fixpoint`], `derived` and `support_index` are empty and the
/// program is just its facts and rules. After it, `derived` holds every atom's
/// precomputed support and `support_index` inverts it for blame and revaluation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct World {
    /// The event-variable name table shared by every support polynomial.
    pub interner: Interner,
    /// The constraint theory (exclusivity families).
    pub theory: Theory,
    /// The ambient valuation — the ledger `ν` under which queries evaluate.
    pub valuation: Valuation,
    /// Extensional facts.
    pub facts: Vec<Fact>,
    /// Derivation rules.
    pub rules: Vec<Rule>,
    /// The saturated relation: every derivable atom's precomputed support.
    pub derived: Vec<Derived>,
    /// Inverted index: event variable to the atoms whose support mentions it.
    pub support_index: BTreeMap<Atom, BTreeSet<GroundAtom>>,
    /// Non-fatal findings raised during lowering (e.g. a citation naming a
    /// decision the corpus never declares).
    pub findings: Vec<LowerFinding>,
    /// The time-point name table: `TimePoint(i)` is the era/event named at
    /// index `i`. Distinct from `interner` (event variables) so the timeline
    /// namespace never collides with the support algebra's.
    pub time_points: Interner,
    /// The precedence chronology, populated during lowering from `before`
    /// facts and era-order chains; the time subsystem reasons over it.
    pub chronology: Chronology,
    /// The cyclic calendars.
    pub cycles: Vec<Cycle>,
    /// Per-variable grade: the grade of the evidence that introduced each
    /// support variable. Populated during fact lowering; retained for
    /// diagnostics and seeding. The evaluator now uses per-monomial grades
    /// stored on [`Derived::monomial_grades`] instead.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub var_grades: BTreeMap<Atom, Grade>,
}

impl World {
    /// An empty world.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Lower a document into an evaluable program: ground facts, option-gated
    /// facts, exclusivity families for questions and forks, and cycles. Event
    /// variables are interned as they are encountered.
    ///
    /// Derivation rules (`rule …`) carry a text body this crate cannot re-parse
    /// (the structured rule-body parser lives in the syntax lane, which this
    /// crate does not depend on), so they are not lowered here; the engine's
    /// typed rule intake is [`World::define_rule`], and the compiled loader
    /// (`dli_world`) parses bare world rules and routes them through it. Other
    /// non-deductive declarations (sorts, registers, drafts, …) are consumed by
    /// other subsystems and carry no fact.
    pub fn lower(document: &Document) -> Result<World, EvalError> {
        let mut world = World::new();
        let mut proposal_seq: u32 = 0;
        // First pass: the authoritative decision set — the ids a `ratify` decl
        // declares. Citing a decision asserts it exists; the kernel must not
        // take that on faith, so a citation naming anything outside this set
        // fails closed (its variable is never ruled true; see below).
        let known_decisions: BTreeSet<&str> = document
            .decls
            .iter()
            .filter_map(|decl| match decl {
                Decl::Ratify(ratify) => Some(ratify.decision.text.as_str()),
                _ => None,
            })
            .collect();
        for decl in &document.decls {
            match decl {
                Decl::Fact(fact) => {
                    let atom = ground_atom(&fact.atom)?;
                    let ann = world.lower_fact_annotation(
                        fact.provenance.as_ref(),
                        &mut proposal_seq,
                        &known_decisions,
                    );
                    world.facts.push(Fact {
                        atom,
                        ann,
                        grade: Grade::Asserted,
                    });
                }
                Decl::Attr(attr) => {
                    world.lower_attr(attr, None, &mut proposal_seq, &known_decisions)?;
                }
                Decl::Question(question) => world.lower_options(&question.options)?,
                Decl::Fork(fork) => world.lower_options(&fork.options)?,
                Decl::Cycle(cycle) => world.cycles.push(Cycle {
                    name: cycle.name.text.clone(),
                    spokes: cycle.spokes.iter().map(|s| s.text.clone()).collect(),
                }),
                Decl::Before(before) => {
                    let earlier = world.time_point(&before.earlier.text);
                    let later = world.time_point(&before.later.text);
                    world.chronology.edges.push((earlier, later));
                }
                Decl::Entity(entity) => {
                    for slot in &entity.slots {
                        world.lower_attr(
                            slot,
                            entity.provenance.as_ref(),
                            &mut proposal_seq,
                            &known_decisions,
                        )?;
                    }
                    if let Some(TimeClause::Order(chain)) = &entity.time {
                        // The parser hands us the chain AFTER the declared head
                        // (`era a < b < c` → name `a`, chain `[b, c]`); the full
                        // order is `[name] ++ chain`. Prepend the head so the
                        // `a → b` edge is not dropped (a dropped head silently
                        // loses ordering and hides an `a < b < a` cycle: H8).
                        let ordered: Vec<Ident> = std::iter::once(entity.name.clone())
                            .chain(chain.iter().cloned())
                            .collect();
                        world.add_order_chain(&ordered);
                    }
                    // Intern the entity's own name after its chain, so a lone era
                    // or event (in no `before` fact and no chain) still enters the
                    // time-point table and chain members keep their edge index.
                    if matches!(entity.kind, EntityKind::Era | EntityKind::Event) {
                        world.time_point(&entity.name.text);
                    }
                }
                Decl::Hypothesis(hypothesis) => {
                    // Register a declared hypothesis as an event variable so a
                    // fork can suppose it even when nothing references it yet.
                    world.interner.intern(&hypothesis.name.text);
                }
                Decl::State(state) => {
                    world.lower_state(state, &mut proposal_seq, &known_decisions)?;
                }
                _ => {}
            }
        }
        Ok(world)
    }

    fn lower_attr(
        &mut self,
        attr: &AttrDecl,
        inherit: Option<&Annotation>,
        proposal_seq: &mut u32,
        known_decisions: &BTreeSet<&str>,
    ) -> Result<(), EvalError> {
        let atom = ground_atom(&AtomExpr {
            pred: attr.attr.clone(),
            args: vec![Term::Sym(attr.subject.clone()), attr.value.clone()],
        })?;
        let ann = self.lower_fact_annotation(
            attr.provenance.as_ref().or(inherit),
            proposal_seq,
            known_decisions,
        );
        self.facts.push(Fact {
            atom,
            ann,
            grade: Grade::Asserted,
        });
        Ok(())
    }

    /// Lower a declared state to a ground fact, mirroring [`World::lower_attr`]:
    /// a state is an attribute that holds over an interval, so the atom it
    /// realizes is the same one the attribute form would produce and it answers
    /// the same queries. [`StateDecl::atom`] is the shared lowering seam, so the
    /// evaluator and the compiled loader cannot disagree about the atom.
    ///
    /// An undotted valueless state names nothing evaluable; it carries no fact
    /// (the loader records a finding for it).
    fn lower_state(
        &mut self,
        state: &StateDecl,
        proposal_seq: &mut u32,
        known_decisions: &BTreeSet<&str>,
    ) -> Result<(), EvalError> {
        let Some(pattern) = state.atom() else {
            return Ok(());
        };
        let atom = ground_atom(&pattern)?;
        let ann =
            self.lower_fact_annotation(state.provenance.as_ref(), proposal_seq, known_decisions);
        self.facts.push(Fact {
            atom,
            ann,
            grade: Grade::Asserted,
        });
        Ok(())
    }

    /// Intern an era/event name to a stable [`TimePoint`].
    fn time_point(&mut self, name: &str) -> TimePoint {
        TimePoint(self.time_points.intern(name).0)
    }

    /// Add the consecutive precedence edges of an era-order chain
    /// (`< a < b < c` becomes `a → b`, `b → c`).
    fn add_order_chain(&mut self, chain: &[Ident]) {
        for pair in chain.windows(2) {
            let earlier = self.time_point(&pair[0].text);
            let later = self.time_point(&pair[1].text);
            self.chronology.edges.push((earlier, later));
        }
    }

    /// Lower a run of question/fork options: each option name is interned as an
    /// event variable, the options form an exclusivity family, and any `implies`
    /// atom becomes a fact gated on that option (holding when the option rules).
    fn lower_options(&mut self, options: &[OptionClause]) -> Result<(), EvalError> {
        let mut family = Vec::with_capacity(options.len());
        for option in options {
            let var = self.interner.intern(&option.name.text);
            family.push(var);
            if let Some(implied) = &option.implies {
                let atom = ground_atom(implied)?;
                self.facts.push(Fact {
                    atom,
                    ann: Ann::supported(PosBool::pos(var)),
                    grade: Grade::Asserted,
                });
            }
        }
        self.theory.add_exclusive(family);
        Ok(())
    }

    /// Define a typed, unconditional derivation rule — the engine's recursive
    /// datalog intake. The rule is range-restriction checked before it is added.
    ///
    /// The body accepts anything convertible into a [`BodyLiteral`]; a bare
    /// [`AtomExpr`] converts to a **positive** literal, so a positive-only caller
    /// passes `Vec<AtomExpr>` unchanged, while a negation-aware caller passes a
    /// `Vec<BodyLiteral>` mixing [`BodyLiteral::pos`] and [`BodyLiteral::not`].
    /// Range restriction counts only positive body atoms as binding: every
    /// variable in the head *or in a negated literal* must appear in some
    /// positive body atom (`docs/spec/08-reasoning.md §1`).
    pub fn define_rule(
        &mut self,
        name: impl Into<String>,
        head: AtomExpr,
        body: impl IntoIterator<Item = impl Into<BodyLiteral>>,
    ) -> Result<(), EvalError> {
        let name = name.into();
        let body: Vec<BodyLiteral> = body.into_iter().map(Into::into).collect();
        check_range_restricted(&name, &head, &body)?;
        self.rules.push(Rule {
            name,
            head,
            body,
            ann: Ann::unconditional(),
            grade: Grade::Asserted,
        });
        Ok(())
    }

    /// Assert a typed ground fact with an explicit annotation — the engine's
    /// extensional intake, for callers that build facts without a document.
    pub fn assert_fact(&mut self, atom: GroundAtom, ann: Ann) {
        self.facts.push(Fact {
            atom,
            ann,
            grade: Grade::Asserted,
        });
    }

    /// Assert a typed ground fact with an explicit annotation and grade,
    /// populating `var_grades` for each variable in the fact's support.
    pub fn assert_fact_graded(&mut self, atom: GroundAtom, ann: Ann, grade: Grade) {
        for monomial in ann.support.monomials() {
            for lit in monomial.lits() {
                self.var_grades
                    .entry(lit.atom)
                    .and_modify(|existing| *existing = Grade::join_alt([*existing, grade]))
                    .or_insert(grade);
            }
        }
        self.facts.push(Fact { atom, ann, grade });
    }

    /// Compute the dynamic grade of an atom under a specific valuation
    /// (`docs/spec/02-semantics.md` §2): per-monomial stored grades; the grade
    /// of the atom is the join (strongest alternative) over all ν-true
    /// monomials' stored grades. No laundering through absorption or pooled-grade
    /// path. Defeat asymmetry documented: a defeated Checked monomial still lends
    /// its grade (pre-existing; future design ruling).
    ///
    /// Old data with an empty `monomial_grades` map falls back to
    /// `Grade::Asserted` (conservative default, not over-claiming).
    #[must_use]
    pub fn grade_under(&self, derived: &Derived, valuation: &Valuation) -> Grade {
        let mut best = None;
        for (monomial, mono_grade) in derived.monomial_grades.iter() {
            if !monomial.eval_under(valuation) {
                continue;
            }
            best = Some(match best {
                Some(prev) => Grade::join_alt([prev, mono_grade]),
                None => mono_grade,
            });
        }
        best.unwrap_or(Grade::Asserted)
    }

    /// Compile a fact's provenance into its support, gating on the provenance
    /// marker — not merely on the optional `assumes` condition (koan 1: nothing
    /// is true by being written down).
    ///
    /// - No provenance at all fails closed: `P⁺ = FALSE`, so the claim is dead
    ///   and excluded from every layer but the error report.
    /// - `proposed` rests on a fresh open proposal variable, left unruled, so it
    ///   evaluates *possible* until a ruling.
    /// - `decided`/`ratified`/`provisional` rest on the conjunction of their
    ///   cited decision variables, each set **true** in the default valuation —
    ///   citing a ruling asserts the ruling exists (the ledger ranks authority),
    ///   so it evaluates *certain*, revocable later by revaluing the variable
    ///   false (supersession). A ledger marker with no citation is malformed and
    ///   fails closed.
    ///
    /// Any `assumes` condition is conjoined onto the marker support, so a ruled
    /// claim gated on an open hypothesis stays possible.
    fn lower_fact_annotation(
        &mut self,
        annotation: Option<&Annotation>,
        proposal_seq: &mut u32,
        known_decisions: &BTreeSet<&str>,
    ) -> Ann {
        let Some(annotation) = annotation else {
            return Ann::supported(PosBool::FALSE);
        };
        let condition = annotation
            .condition
            .as_ref()
            .map(|condition| lower_condition(condition, &mut self.interner));
        let support = match annotation.marker {
            // An explicit `assumes` condition IS the proposal's bundle gate — the
            // one hypothesis variable a ruling ratifies. Only mint a fresh
            // proposal variable when the claim carries no condition, which would
            // otherwise leave it unratifiable.
            ProvenanceMarker::Proposed => condition.unwrap_or_else(|| {
                let name = format!("proposal#{proposal_seq}");
                *proposal_seq += 1;
                PosBool::pos(self.interner.intern(&name))
            }),
            ProvenanceMarker::Decided
            | ProvenanceMarker::Ratified
            | ProvenanceMarker::Provisional => {
                let ledger = if annotation.citations.is_empty() {
                    PosBool::FALSE
                } else {
                    let mut support = PosBool::truth();
                    for citation in &annotation.citations {
                        let var = self.interner.intern(&citation.id.text);
                        // Rule the decision true ONLY IF the corpus declares it.
                        // An unknown citation is a forged (or typo'd) authority
                        // claim: leave its variable open — so the fact can never
                        // be certain — and record a finding for `check`.
                        if known_decisions.contains(citation.id.text.as_str()) {
                            self.valuation.set(var, true);
                        } else {
                            self.findings.push(LowerFinding {
                                code: "citation.unknown-decision".to_owned(),
                                message: format!(
                                    "cited decision '{}' is not a declared decision",
                                    citation.id.text
                                ),
                            });
                        }
                        support = support.and_(&PosBool::pos(var));
                    }
                    support
                };
                // A ledger claim may still be gated on an open hypothesis.
                match condition {
                    Some(condition) => ledger.and_(&condition),
                    None => ledger,
                }
            }
        };
        Ann::supported(support)
    }

    /// Partition the rule set's intensional predicates into evaluation strata,
    /// least-dependent first. Mutually recursive predicates share a stratum; a
    /// negative dependency inside such a recursive clique is unstratifiable and
    /// returns [`EvalError::Unstratifiable`] naming the atoms.
    pub fn stratify(&self) -> Result<Strata, EvalError> {
        crate::strata::stratify_rules(&self.rules)
    }

    /// Compute the symbolic fixpoint: saturate every derivable atom's support
    /// once, then build the inverted support index. Stratification is computed
    /// internally.
    ///
    /// ```
    /// use dli_core::ann::Ann;
    /// use dli_eval::world::{GroundAtom, World};
    /// let mut world = World::new();
    /// let edge = |from: &str, to: &str| GroundAtom {
    ///     pred: "adjacent".to_owned(),
    ///     args: vec![from.to_owned(), to.to_owned()],
    /// };
    /// world.assert_fact(edge("a", "b"), Ann::unconditional());
    /// let saturated = world.fixpoint().expect("stratifiable");
    /// assert_eq!(saturated.derived.len(), 1);
    /// ```
    pub fn fixpoint(&self) -> Result<World, EvalError> {
        let strata = self.stratify()?;
        self.saturate_with(&strata)
    }

    /// The fixpoint over a precomputed stratification (the free-function seam).
    ///
    /// The ambient valuation and theory are threaded into the stratum saturation
    /// so a negated body literal can be evaluated against the **certain layer**
    /// (`docs/spec/02-semantics.md §4`): a certain `b` blocks `not b`, a merely-
    /// possible `b` poisons the conclusion's defeaters. A stratified negation is
    /// stable within a stratum because the negated predicate is always fully
    /// saturated in a strictly earlier one.
    pub fn saturate_with(&self, strata: &Strata) -> Result<World, EvalError> {
        let mut relation = self.seed_relation();
        for level in &strata.levels {
            let level_set: BTreeSet<&str> = level.iter().map(String::as_str).collect();
            let level_rules: Vec<&Rule> = self
                .rules
                .iter()
                .filter(|rule| level_set.contains(rule.head.pred.text.as_str()))
                .collect();
            saturate_stratum(&mut relation, &level_rules, &self.theory)?;
        }
        let mut world = self.clone();
        world.derived = relation.into_values().collect();
        world.derived.sort_by(|a, b| a.atom.cmp(&b.atom));
        world.support_index = build_support_index(&world.derived);
        // Populate var_grades from facts: each variable in a fact's support
        // inherits the fact's grade, keeping the strongest if seen twice.
        for fact in &world.facts {
            for monomial in fact.ann.support.monomials() {
                for lit in monomial.lits() {
                    world
                        .var_grades
                        .entry(lit.atom)
                        .and_modify(|existing| {
                            *existing = Grade::join_alt([*existing, fact.grade]);
                        })
                        .or_insert(fact.grade);
                }
            }
        }
        Ok(world)
    }

    /// Query the saturation under the ambient valuation.
    pub fn query(&self, query: &Query) -> Result<Answer, EvalError> {
        self.eval_under(&self.valuation, query)
    }

    /// Query the saturation under an arbitrary valuation — the fork seam.
    ///
    /// Supports are untouched; only each atom's status is re-derived, so a
    /// what-if costs one evaluation pass, never a re-lowering or re-fixpoint.
    pub fn eval_under(&self, valuation: &Valuation, query: &Query) -> Result<Answer, EvalError> {
        if !self.knows_predicate(&query.goal.pred.text) {
            return Err(EvalError::UnknownPredicate {
                name: query.goal.pred.text.clone(),
            });
        }
        let mut rows = Vec::new();
        for derived in &self.derived {
            if derived.atom.pred != query.goal.pred.text
                || derived.atom.args.len() != query.goal.args.len()
            {
                continue;
            }
            let mut bindings = BTreeMap::new();
            if !unify(&query.goal, &derived.atom, &mut bindings) {
                continue;
            }
            let ann = derived.ann();
            let status = ann.status(valuation, &self.theory);
            if status == Status::Dead {
                continue;
            }
            rows.push(QueryRow {
                atom: derived.atom.render(),
                bindings,
                verdict: Verdict {
                    support: derived.support.clone(),
                    grade: self.grade_under(derived, valuation),
                },
                status,
                // Explanations are attached by `query_with` (the query lane),
                // never by the base evaluator: a plain `query` carries none.
                why: None,
            });
        }
        rows.sort_by(|a, b| a.atom.cmp(&b.atom));
        Ok(Answer { rows })
    }

    /// A copy of this world with a different ambient valuation attached. The
    /// symbolic saturation is shared unchanged; only `ν` differs.
    #[must_use]
    pub fn with_valuation(&self, valuation: Valuation) -> World {
        let mut world = self.clone();
        world.valuation = valuation;
        world
    }

    /// Query while supposing a set of options ruled true (a fork's `suppose`).
    /// Supposing an option also rules its exclusivity siblings false, so the
    /// options a ruling excludes go dead. Unknown option names bind nothing.
    pub fn fork_eval(&self, query: &Query, assume: &[Ident]) -> Result<Answer, EvalError> {
        let mut valuation = self.valuation.clone();
        for ident in assume {
            let Some(position) = self.interner.names().iter().position(|n| n == &ident.text) else {
                continue;
            };
            let atom = Atom(u32::try_from(position).unwrap_or(u32::MAX));
            valuation.set(atom, true);
            for family in &self.theory.exclusive {
                if family.contains(&atom) {
                    for &sibling in family {
                        if sibling != atom {
                            valuation.set(sibling, false);
                        }
                    }
                }
            }
        }
        self.eval_under(&valuation, query)
    }

    /// The inverted support index: event variable to the atoms depending on it.
    #[must_use]
    pub fn support_index(&self) -> &BTreeMap<Atom, BTreeSet<GroundAtom>> {
        &self.support_index
    }

    fn seed_relation(&self) -> BTreeMap<GroundAtom, Derived> {
        let mut relation: BTreeMap<GroundAtom, Derived> = BTreeMap::new();
        for fact in &self.facts {
            let cond = CondSupport::of_ann(&fact.ann);
            merge_into(&mut relation, fact.atom.clone(), fact.grade, &cond);
        }
        relation
    }

    fn knows_predicate(&self, pred: &str) -> bool {
        self.facts.iter().any(|f| f.atom.pred == pred)
            || self.rules.iter().any(|r| r.head.pred.text == pred)
    }
}

/// Run the annotation-aware seminaive fixpoint for one stratum: seed a worklist
/// with the current relation and re-fire a rule whenever a body atom's support
/// strictly grows, until the finite support lattice reaches its fixpoint.
///
/// A rule with **no positive body literal** (a pure-negative body, e.g.
/// `p :- not q`) has no atom to trigger it via a delta, so it is fired once at
/// stratum start against the fully-saturated earlier strata. Everything else is
/// delta-driven off its positive atoms.
fn saturate_stratum(
    relation: &mut BTreeMap<GroundAtom, Derived>,
    rules: &[&Rule],
    theory: &Theory,
) -> Result<(), EvalError> {
    let mut worklist: VecDeque<GroundAtom> = relation.keys().cloned().collect();
    // Seed pure-negative-body rules: they cannot be delta-triggered because they
    // name no positive atom. Their negated predicates live in earlier strata
    // (stratification guarantees it), so the relation already holds them.
    for rule in rules {
        if rule.body.iter().all(|lit| lit.negated) {
            let negatives: Vec<&AtomExpr> = rule.body.iter().map(|lit| &lit.atom).collect();
            let bindings = BTreeMap::new();
            let poison = negative_poison(relation, &negatives, &bindings, theory)?;
            let Ok(head) = ground_head(&rule.head, &bindings) else {
                continue; // range restriction guarantees a ground head here
            };
            let cond = CondSupport::of_ann(&rule.ann).poison(&poison);
            if let Some(head) = merge_into(relation, head, rule.grade, &cond) {
                worklist.push_back(head);
            }
        }
    }
    while let Some(delta) = worklist.pop_front() {
        let Some(delta_row) = relation.get(&delta).cloned() else {
            continue;
        };
        let productions = fire_rules(relation, rules, &delta, &delta_row, theory)?;
        for (head, grade, cond) in productions {
            if let Some(head) = merge_into(relation, head, grade, &cond) {
                worklist.push_back(head);
            }
        }
    }
    Ok(())
}

/// A support polynomial together with its **per-monomial** defeater conditions —
/// the working form the fixpoint threads so a negated premise poisons only the
/// derivation path it rode in on (`docs/spec/02-semantics.md §4`), never the atom
/// as a whole. Every monomial of `support` maps to a defeater (`FALSE` for a
/// clean path); [`CondSupport::defeater_of`] reads it. For a positive-only
/// derivation every defeater is `FALSE`, so `and_`/`or_` reduce to the plain
/// [`PosBool`] lattice operations and positive datalog is byte-identical.
#[derive(Debug, Clone)]
struct CondSupport {
    support: PosBool,
    conditions: DefeaterConditions,
}

impl CondSupport {
    /// The conditioned support carried by an annotation.
    fn of_ann(ann: &Ann) -> CondSupport {
        CondSupport {
            support: ann.support.clone(),
            conditions: ann.conditions.clone(),
        }
    }

    /// The conditioned support of a saturated row.
    fn of_row(row: &Derived) -> CondSupport {
        CondSupport {
            support: row.support.clone(),
            conditions: row.conditions.clone(),
        }
    }

    /// The defeater pinned to a support monomial (`FALSE` for a clean path).
    fn defeater_of(&self, monomial: &Monomial) -> PosBool {
        self.conditions.defeater_of(monomial).clone()
    }

    /// Rebuild from a monomial→defeater map, dropping the dominated monomials.
    fn from_map(map: BTreeMap<Monomial, PosBool>) -> CondSupport {
        let kept = absorb_conditioned(map);
        let mut support = BTreeSet::new();
        let mut conditions = DefeaterConditions::new();
        for (monomial, defeater) in kept {
            support.insert(monomial.clone());
            conditions.set(monomial, defeater);
        }
        CondSupport {
            support: PosBool(support),
            conditions,
        }
    }

    /// The monomial→defeater map for this support (every monomial present, clean
    /// ones mapping to `FALSE`).
    fn to_map(&self) -> BTreeMap<Monomial, PosBool> {
        self.support
            .monomials()
            .map(|monomial| (monomial.clone(), self.defeater_of(monomial)))
            .collect()
    }

    /// Conjunction (rule-body join): cross-product the monomials, and a product
    /// path is defeated when *either* parent path is — so its defeater is the OR
    /// of the two parents'. When one product arises two ways, the paths disjoin,
    /// so their defeaters conjoin (`AND`): the clean way protects it.
    fn and_(&self, other: &CondSupport) -> CondSupport {
        let mut map: BTreeMap<Monomial, PosBool> = BTreeMap::new();
        for left in self.support.monomials() {
            let left_def = self.defeater_of(left);
            for right in other.support.monomials() {
                let mut lits = left.0.clone();
                lits.extend(right.0.iter().copied());
                let product = Monomial(lits);
                if product.is_contradictory() {
                    continue;
                }
                let defeater = left_def.or_(&other.defeater_of(right));
                map.entry(product)
                    .and_modify(|existing| *existing = existing.and_(&defeater))
                    .or_insert(defeater);
            }
        }
        CondSupport::from_map(map)
    }

    /// Disjunction (merge of two derivations of one head): union the monomials; a
    /// monomial reached both ways is defeated only when *both* defeaters are, so
    /// they conjoin (`AND`) — a clean way makes the monomial clean. A monomial on
    /// one side only keeps that side's defeater.
    fn or_(&self, other: &CondSupport) -> CondSupport {
        let mut map = self.to_map();
        for (monomial, defeater) in other.to_map() {
            map.entry(monomial)
                .and_modify(|existing| *existing = existing.and_(&defeater))
                .or_insert(defeater);
        }
        CondSupport::from_map(map)
    }

    /// Fold a negation poison into **every** monomial's defeater: the negated
    /// premise defeats the whole derivation regardless of which support monomial
    /// carried it. A `FALSE` poison (an absent or dead negated premise) is a
    /// no-op, so an untouched path stays clean.
    fn poison(&self, poison: &PosBool) -> CondSupport {
        if poison.is_false() {
            return self.clone();
        }
        let mut map = self.to_map();
        for defeater in map.values_mut() {
            *defeater = defeater.or_(poison);
        }
        // Poisoning preserves the monomial set and every subsumption relation, so
        // no re-absorption is needed.
        let mut conditions = DefeaterConditions::new();
        for (monomial, defeater) in &map {
            conditions.set(monomial.clone(), defeater.clone());
        }
        CondSupport {
            support: self.support.clone(),
            conditions,
        }
    }
}

/// Reduce a monomial→defeater map to its sound antichain: drop a monomial `m2`
/// when a more-general monomial `m1` covers it *and* `m1`'s defeater entails
/// `m2`'s (`d1 ⟹ d2`), so `m1`'s contribution dominates `m2`'s. With every
/// defeater `FALSE` this is exactly plain [`PosBool`] absorption, so a
/// positive-only support is byte-identical. Contradictory monomials drop out.
fn absorb_conditioned(map: BTreeMap<Monomial, PosBool>) -> BTreeMap<Monomial, PosBool> {
    let live: Vec<(Monomial, PosBool)> = map
        .into_iter()
        .filter(|(monomial, _)| !monomial.is_contradictory())
        .collect();
    let mut kept = BTreeMap::new();
    for (i, (monomial, defeater)) in live.iter().enumerate() {
        let dominated = live.iter().enumerate().any(|(j, (other, other_def))| {
            j != i && other.subsumes(monomial) && other_def.implies(defeater)
        });
        if !dominated {
            kept.insert(monomial.clone(), defeater.clone());
        }
    }
    kept
}

/// Fire every rule against a changed atom `delta`, joining the other *positive*
/// body atoms over the full relation and then applying the negated literals as a
/// certain-layer poison, and collect the head atoms they produce.
type Production = (GroundAtom, Grade, CondSupport);

fn fire_rules(
    relation: &BTreeMap<GroundAtom, Derived>,
    rules: &[&Rule],
    delta: &GroundAtom,
    delta_row: &Derived,
    theory: &Theory,
) -> Result<Vec<Production>, EvalError> {
    let mut out = Vec::new();
    for rule in rules {
        // Only positive literals trigger and join; negated literals poison.
        let positives: Vec<&AtomExpr> = rule
            .body
            .iter()
            .filter(|lit| !lit.negated)
            .map(|lit| &lit.atom)
            .collect();
        let negatives: Vec<&AtomExpr> = rule
            .body
            .iter()
            .filter(|lit| lit.negated)
            .map(|lit| &lit.atom)
            .collect();
        for (i, body_atom) in positives.iter().enumerate() {
            if body_atom.pred.text != delta.pred || body_atom.args.len() != delta.args.len() {
                continue;
            }
            let mut bindings = BTreeMap::new();
            if !unify(body_atom, delta, &mut bindings) {
                continue;
            }
            let rest: Vec<&AtomExpr> = positives
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, atom)| *atom)
                .collect();
            let solutions = join_body(
                relation,
                &rest,
                bindings,
                CondSupport::of_row(delta_row),
                delta_row.monomial_grades.weakest_entry(),
            );
            for (bindings, grade, cond) in solutions {
                // Negation last: every positive atom has bound the negated
                // literals' variables (range restriction), so each grounds. A
                // certain negated premise now poisons rather than blocks — the
                // block is a defeater condition the valuation can move both ways
                // (`docs/spec/02-semantics.md §5`), so it un-blocks without a
                // re-fixpoint.
                let poison = negative_poison(relation, &negatives, &bindings, theory)?;
                let head = ground_head(&rule.head, &bindings)?;
                let cond = cond.and_(&CondSupport::of_ann(&rule.ann)).poison(&poison);
                out.push((head, Grade::weakest([grade, rule.grade]), cond));
            }
        }
    }
    Ok(out)
}

/// The negation poison for a rule body's negated literals against the current
/// relation (`docs/spec/02-semantics.md §4`): the OR of every **structurally
/// admissible** monomial of each `¬b`'s support `P⁺`, to be folded into the
/// conclusion's defeater condition.
///
/// The poison is **valuation-neutral**: a monomial is included whenever it can be
/// true under *some* valuation — `!is_contradictory()` and theory-respecting —
/// never filtered by the base `ν`'s *liveness*. Baking the base valuation in would
/// permanently drop a poison monomial whose support var is ruled false at fixpoint
/// time, so a later fork re-supposing it true could never re-block; the defeater
/// must move both ways under `eval_under` for evaluation to commute with
/// specialization (§1). A **certain** `b` is likewise included — its monomial
/// rides in as a defeater rather than dropping the derivation, un-blocking without
/// a re-fixpoint (§5). Only a monomial that can NEVER be true under ANY valuation
/// (contradictory or theory-forbidden) contributes nothing.
///
/// A `b` the relation never derives has support `FALSE`: nothing to poison — the
/// standard "negation of an underived atom succeeds" cleanly, leaving the path
/// clean.
fn negative_poison(
    relation: &BTreeMap<GroundAtom, Derived>,
    negatives: &[&AtomExpr],
    bindings: &BTreeMap<String, String>,
    theory: &Theory,
) -> Result<PosBool, EvalError> {
    let mut poison = PosBool::FALSE;
    for neg in negatives {
        let ground = ground_head(neg, bindings)?;
        let Some(row) = relation.get(&ground) else {
            continue;
        };
        for monomial in row.support.monomials() {
            if !monomial.is_contradictory() && theory.respects(monomial) {
                poison = poison.or_(&PosBool::conjunction(monomial.lits()));
            }
        }
    }
    Ok(poison)
}

/// Recursively join the remaining body atoms over the relation, accumulating the
/// conjunction of their conditioned supports and the weakest-link grade.
fn join_body(
    relation: &BTreeMap<GroundAtom, Derived>,
    remaining: &[&AtomExpr],
    bindings: BTreeMap<String, String>,
    cond: CondSupport,
    grade: Grade,
) -> Vec<(BTreeMap<String, String>, Grade, CondSupport)> {
    let Some((atom, rest)) = remaining.split_first() else {
        return vec![(bindings, grade, cond)];
    };
    let mut out = Vec::new();
    for row in relation.values() {
        if row.atom.pred != atom.pred.text || row.atom.args.len() != atom.args.len() {
            continue;
        }
        let mut next = bindings.clone();
        if unify(atom, &row.atom, &mut next) {
            out.extend(join_body(
                relation,
                rest,
                next,
                cond.and_(&CondSupport::of_row(row)),
                Grade::weakest([grade, row.monomial_grades.weakest_entry()]),
            ));
        }
    }
    out
}

/// Merge a conditioned derivation into the relation. Grade and defeater
/// conditions fold on *every* merge — a verdict must not depend on the order
/// derivations arrive — while the atom is re-enqueued only when its support
/// strictly changed (support is the termination measure). The atom-global
/// `defeaters` is kept as the OR of the conditions for blame and citation.
fn merge_into(
    relation: &mut BTreeMap<GroundAtom, Derived>,
    atom: GroundAtom,
    grade: Grade,
    cond: &CondSupport,
) -> Option<GroundAtom> {
    if let Some(row) = relation.get_mut(&atom) {
        row.grade = Grade::join_alt([row.grade, grade]);
        let merged = CondSupport::of_row(row).or_(cond);
        let changed = merged.support != row.support;
        row.support = merged.support;
        row.defeaters = merged.conditions.atom_global();
        row.conditions = merged.conditions;
        // Per-monomial grades: the new derivation's monomials get the
        // derivation grade; existing monomials keep best via join_alt.
        for monomial in cond.support.monomials() {
            row.monomial_grades.set(monomial.clone(), grade);
        }
        // Absorbed monomials' entries stay in monomial_grades intentionally:
        // grade_under iterates all entries (including absorbed ones no longer
        // in minimal support), so a monomial's grade is still available when
        // its variables are true under the valuation.
        if changed {
            return Some(atom);
        }
        return None;
    }
    let mut monomial_grades = MonomialGrades::new();
    for monomial in cond.support.monomials() {
        monomial_grades.set(monomial.clone(), grade);
    }
    relation.insert(
        atom.clone(),
        Derived {
            atom: atom.clone(),
            support: cond.support.clone(),
            defeaters: cond.conditions.atom_global(),
            conditions: cond.conditions.clone(),
            monomial_grades,
            grade,
        },
    );
    Some(atom)
}

/// Invert the saturation: each event variable maps to the atoms whose support
/// mentions it, for revaluation and blame.
fn build_support_index(derived: &[Derived]) -> BTreeMap<Atom, BTreeSet<GroundAtom>> {
    let mut index: BTreeMap<Atom, BTreeSet<GroundAtom>> = BTreeMap::new();
    for row in derived {
        for monomial in row.support.monomials() {
            for lit in monomial.lits() {
                index.entry(lit.atom).or_default().insert(row.atom.clone());
            }
        }
    }
    index
}

/// Compile a boolean hypothesis condition into a support polynomial.
fn lower_condition(condition: &CondExpr, interner: &mut Interner) -> PosBool {
    match condition {
        CondExpr::Hyp(id) => PosBool::pos(interner.intern(&id.text)),
        CondExpr::Not(inner) => lower_condition(inner, interner).negate(),
        CondExpr::And(parts) => parts.iter().fold(PosBool::truth(), |acc, part| {
            acc.and_(&lower_condition(part, interner))
        }),
        CondExpr::Or(parts) => parts.iter().fold(PosBool::FALSE, |acc, part| {
            acc.or_(&lower_condition(part, interner))
        }),
    }
}

/// Ground a fact's atom, erroring if any argument is a variable.
fn ground_atom(atom: &AtomExpr) -> Result<GroundAtom, EvalError> {
    let mut args = Vec::with_capacity(atom.args.len());
    for term in &atom.args {
        args.push(
            constant_render(term).ok_or_else(|| EvalError::Unstratifiable {
                reason: format!("fact '{}' has an unbound variable argument", atom.pred.text),
            })?,
        );
    }
    Ok(GroundAtom {
        pred: atom.pred.text.clone(),
        args,
    })
}

/// Ground a rule head under a set of bindings; range restriction guarantees
/// every head variable is bound, but this reports the error defensively.
fn ground_head(
    head: &AtomExpr,
    bindings: &BTreeMap<String, String>,
) -> Result<GroundAtom, EvalError> {
    let mut args = Vec::with_capacity(head.args.len());
    for term in &head.args {
        args.push(
            bind_render(term, bindings).ok_or_else(|| EvalError::Unstratifiable {
                reason: format!("rule head '{}' left a variable unbound", head.pred.text),
            })?,
        );
    }
    Ok(GroundAtom {
        pred: head.pred.text.clone(),
        args,
    })
}

/// Unify a pattern atom against a ground atom, extending the bindings. Variables
/// bind (and must stay consistent); constants must match their rendering.
fn unify(pattern: &AtomExpr, ground: &GroundAtom, bindings: &mut BTreeMap<String, String>) -> bool {
    if pattern.pred.text != ground.pred || pattern.args.len() != ground.args.len() {
        return false;
    }
    for (term, value) in pattern.args.iter().zip(&ground.args) {
        match term {
            Term::Var(id) => match bindings.get(&id.text) {
                Some(bound) if bound != value => return false,
                Some(_) => {}
                None => {
                    bindings.insert(id.text.clone(), value.clone());
                }
            },
            _ => match constant_render(term) {
                Some(rendered) if &rendered == value => {}
                _ => return false,
            },
        }
    }
    true
}

/// Render a constant term, or `None` for a variable.
fn constant_render(term: &Term) -> Option<String> {
    match term {
        Term::Var(_) => None,
        Term::Sym(id) => Some(id.text.clone()),
        Term::Str(s) => Some(s.clone()),
        Term::Int(i) => Some(i.to_string()),
        Term::Time(TimeRef::Era(id)) => Some(format!("era:{}", id.text)),
        Term::Time(TimeRef::Event(id)) => Some(format!("event:{}", id.text)),
    }
}

/// Render a term under bindings: a variable resolves to its bound value, a
/// constant to its rendering.
fn bind_render(term: &Term, bindings: &BTreeMap<String, String>) -> Option<String> {
    match term {
        Term::Var(id) => bindings.get(&id.text).cloned(),
        _ => constant_render(term),
    }
}

/// Reject a rule that violates range restriction — the datalog safety discipline
/// (`docs/spec/08-reasoning.md §1`), checked at load time.
fn check_range_restricted(
    name: &str,
    head: &AtomExpr,
    body: &[BodyLiteral],
) -> Result<(), EvalError> {
    if let Some(variable) = unbound_variable(head, body) {
        return Err(EvalError::Unstratifiable {
            reason: format!(
                "rule '{name}' is not range-restricted: variable '{variable}' is not bound by any positive body atom"
            ),
        });
    }
    Ok(())
}

/// The first variable that violates range restriction, or `None` when the rule
/// is safe. Only **positive** body atoms bind variables; every variable in the
/// head *or in a negated literal* must appear in some positive body atom
/// (`docs/spec/08-reasoning.md §1`). Negated literals bind nothing, so a variable
/// that appears only under `not` is unsafe.
///
/// ```
/// use dli_ast::ident::Ident;
/// use dli_ast::literal::BodyLiteral;
/// use dli_ast::term::{AtomExpr, Term};
/// use dli_eval::world::unbound_variable;
///
/// let atom = |pred: &str, var: &str| AtomExpr {
///     pred: Ident::synthetic(pred),
///     args: vec![Term::Var(Ident::synthetic(var))],
/// };
/// // safe: x is bound by the positive `base(x)`.
/// let safe = [BodyLiteral::pos(atom("base", "x")), BodyLiteral::not(atom("b", "x"))];
/// assert_eq!(unbound_variable(&atom("risky", "x"), &safe), None);
/// // unsafe: x appears only in the negated literal.
/// let unsafe_body = [BodyLiteral::not(atom("b", "x"))];
/// assert_eq!(
///     unbound_variable(&atom("risky", "x"), &unsafe_body).as_deref(),
///     Some("x")
/// );
/// ```
#[must_use]
pub fn unbound_variable(head: &AtomExpr, body: &[BodyLiteral]) -> Option<String> {
    let mut bound: BTreeSet<&str> = BTreeSet::new();
    for lit in body {
        if !lit.negated {
            for term in &lit.atom.args {
                if let Term::Var(id) = term {
                    bound.insert(id.text.as_str());
                }
            }
        }
    }
    // Head variables must be positively bound.
    for term in &head.args {
        if let Term::Var(id) = term
            && !bound.contains(id.text.as_str())
        {
            return Some(id.text.clone());
        }
    }
    // A variable appearing only in a negated literal is unsafe (datalog safety).
    for lit in body {
        if lit.negated {
            for term in &lit.atom.args {
                if let Term::Var(id) = term
                    && !bound.contains(id.text.as_str())
                {
                    return Some(id.text.clone());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn nullary(pred: &str) -> GroundAtom {
        GroundAtom {
            pred: pred.to_owned(),
            args: Vec::new(),
        }
    }

    #[test]
    fn merge_folds_grade_independent_of_order() {
        let atom = nullary("a");
        // A dominated {+0}/Checked derivation and a dominating TRUE/Asserted one
        // must converge to the same verdict whichever arrives first (H3).
        let mut asserted_first = BTreeMap::new();
        merge_into(
            &mut asserted_first,
            atom.clone(),
            Grade::Asserted,
            &CondSupport {
                support: PosBool::truth(),
                conditions: DefeaterConditions::new(),
            },
        );
        merge_into(
            &mut asserted_first,
            atom.clone(),
            Grade::Checked,
            &CondSupport {
                support: PosBool::pos(Atom(0)),
                conditions: DefeaterConditions::new(),
            },
        );

        let mut checked_first = BTreeMap::new();
        merge_into(
            &mut checked_first,
            atom.clone(),
            Grade::Checked,
            &CondSupport {
                support: PosBool::pos(Atom(0)),
                conditions: DefeaterConditions::new(),
            },
        );
        merge_into(
            &mut checked_first,
            atom.clone(),
            Grade::Asserted,
            &CondSupport {
                support: PosBool::truth(),
                conditions: DefeaterConditions::new(),
            },
        );

        // Support is TRUE (absorbs {+0}); grade is the join (Checked) both ways.
        assert_eq!(asserted_first[&atom], checked_first[&atom]);
        assert!(asserted_first[&atom].support.is_true());
        assert_eq!(asserted_first[&atom].grade, Grade::Checked);
    }

    #[test]
    fn grade_is_per_valuation() {
        // Two facts derive the same atom via different monomials with different
        // grades. Under a valuation where only the asserted monomial is true,
        // grade should be Asserted. Under a valuation where only the checked
        // monomial is true, grade should be Checked. The pooled grade (both
        // monomials) should be Checked (join_alt).
        let mut world = World::new();
        let var_a = world.interner.intern("h.a");
        let var_b = world.interner.intern("h.b");

        // Fact 1: supported on var_a, grade Asserted.
        world.assert_fact_graded(
            nullary("p"),
            Ann::supported(PosBool::pos(var_a)),
            Grade::Asserted,
        );
        // Fact 2: supported on var_b, grade Checked.
        world.assert_fact_graded(
            nullary("p"),
            Ann::supported(PosBool::pos(var_b)),
            Grade::Checked,
        );

        let world = world.fixpoint().expect("stratifiable");

        // The pooled grade on the Derived row is the join (Checked).
        let derived = world
            .derived
            .iter()
            .find(|d| d.atom == nullary("p"))
            .expect("p is derived");
        assert_eq!(derived.grade, Grade::Checked);

        // Under a valuation where only var_a is true: grade is Asserted.
        let mut nu_a = Valuation::new();
        nu_a.set(var_a, true);
        nu_a.set(var_b, false);
        assert_eq!(world.grade_under(derived, &nu_a), Grade::Asserted,);

        // Under a valuation where only var_b is true: grade is Checked.
        let mut nu_b = Valuation::new();
        nu_b.set(var_a, false);
        nu_b.set(var_b, true);
        assert_eq!(world.grade_under(derived, &nu_b), Grade::Checked,);

        // Under a valuation where both are true: grade is Checked (join_alt).
        let mut nu_both = Valuation::new();
        nu_both.set(var_a, true);
        nu_both.set(var_b, true);
        assert_eq!(world.grade_under(derived, &nu_both), Grade::Checked,);
    }

    #[test]
    fn asserted_rule_checked_fact_grades_asserted() {
        // An Asserted-grade rule deriving from a Checked-grade fact: the
        // derivation's grade is min(rule.grade, body.grade) = Asserted (weakest
        // link). Under the derived monomial's valuation, grade must be Asserted.
        let mut world = World::new();
        let var_d = world.interner.intern("d.1");

        // Checked-grade extensional fact.
        world.assert_fact_graded(
            GroundAtom {
                pred: "base".to_owned(),
                args: vec!["x".to_owned()],
            },
            Ann::supported(PosBool::pos(var_d)),
            Grade::Checked,
        );

        // Asserted-grade rule: derived(X) :- base(X).
        world
            .define_rule(
                "r1",
                AtomExpr {
                    pred: Ident::synthetic("derived"),
                    args: vec![Term::Var(Ident::synthetic("X"))],
                },
                vec![BodyLiteral {
                    negated: false,
                    atom: AtomExpr {
                        pred: Ident::synthetic("base"),
                        args: vec![Term::Var(Ident::synthetic("X"))],
                    },
                }],
            )
            .expect("define_rule");

        let world = world.fixpoint().expect("stratifiable");

        let derived = world
            .derived
            .iter()
            .find(|d| d.atom.pred == "derived")
            .expect("derived(x) exists");

        // The derivation grade is min(Asserted, Checked) = Asserted.
        let mut nu = Valuation::new();
        nu.set(var_d, true);
        assert_eq!(world.grade_under(derived, &nu), Grade::Asserted);
    }

    #[test]
    fn true_absorption_preserves_derivation_grade() {
        // A fact with TRUE support (unconditional) at grade Checked: after
        // fixpoint, grade_under must return Checked, not some fallback.
        let mut world = World::new();

        world.assert_fact_graded(
            nullary("unconditional"),
            Ann::unconditional(),
            Grade::Checked,
        );

        let world = world.fixpoint().expect("stratifiable");

        let derived = world
            .derived
            .iter()
            .find(|d| d.atom == nullary("unconditional"))
            .expect("unconditional is derived");

        // TRUE support: the empty monomial.
        assert!(derived.support.is_true());

        // Under any valuation, grade must be Checked (the derivation's grade).
        let nu = Valuation::new();
        assert_eq!(world.grade_under(derived, &nu), Grade::Checked);
    }

    #[test]
    fn true_absorption_folds_absorbed_grade() {
        // A Checked fact on monomial {+x} and an Asserted fact on TRUE (absorbs
        // {+x}). After absorption the support is TRUE. The monomial_grades map
        // retains BOTH entries: TRUE@Asserted and {+x}@Checked.
        let mut world = World::new();
        let var_x = world.interner.intern("h.x");

        // Checked fact on {+x}.
        world.assert_fact_graded(
            nullary("abs"),
            Ann::supported(PosBool::pos(var_x)),
            Grade::Checked,
        );
        // Asserted fact with TRUE support.
        world.assert_fact_graded(nullary("abs"), Ann::unconditional(), Grade::Asserted);

        let world = world.fixpoint().expect("stratifiable");

        let derived = world
            .derived
            .iter()
            .find(|d| d.atom == nullary("abs"))
            .expect("abs is derived");

        // Support is TRUE (absorbed {+x}).
        assert!(derived.support.is_true());

        // Under empty valuation (x defaults false): only TRUE monomial fires →
        // Asserted (the fiat's grade — the only live evidence when x is false).
        let nu = Valuation::new();
        assert_eq!(world.grade_under(derived, &nu), Grade::Asserted);

        // Under {x: true}: both TRUE and {+x} fire; join_alt(Asserted, Checked) = Checked.
        let mut nu_x = Valuation::new();
        nu_x.set(var_x, true);
        assert_eq!(world.grade_under(derived, &nu_x), Grade::Checked);
    }

    #[test]
    fn multi_path_body_no_over_claim() {
        // A Checked-grade rule over a body atom that has two monomial entries:
        // {x}@Asserted and {y}@Checked. Under {x:true, y:false} the body atom's
        // weakest_entry is Asserted, so the derived grade must be Asserted.
        let mut world = World::new();
        let var_x = world.interner.intern("h.x");
        let var_y = world.interner.intern("h.y");

        // Body atom "base" has two derivation paths with different grades.
        world.assert_fact_graded(
            nullary("base"),
            Ann::supported(PosBool::pos(var_x)),
            Grade::Asserted,
        );
        world.assert_fact_graded(
            nullary("base"),
            Ann::supported(PosBool::pos(var_y)),
            Grade::Checked,
        );

        // Checked-grade rule: derived :- base.
        world
            .define_rule(
                "r_checked",
                AtomExpr {
                    pred: Ident::synthetic("derived"),
                    args: vec![],
                },
                vec![BodyLiteral {
                    negated: false,
                    atom: AtomExpr {
                        pred: Ident::synthetic("base"),
                        args: vec![],
                    },
                }],
            )
            .expect("define_rule");
        // Upgrade the rule's grade to Checked.
        world.rules.last_mut().unwrap().grade = Grade::Checked;

        let world = world.fixpoint().expect("stratifiable");

        let derived = world
            .derived
            .iter()
            .find(|d| d.atom.pred == "derived")
            .expect("derived exists");

        // Under {x:true, y:false}: the body atom contributes its weakest_entry
        // (Asserted) to the chain, so derived grade = min(Checked, Asserted) = Asserted.
        let mut nu = Valuation::new();
        nu.set(var_x, true);
        nu.set(var_y, false);
        assert_eq!(world.grade_under(derived, &nu), Grade::Asserted);
    }

    #[test]
    fn asserted_ward_fallback_empty_monomial_grades() {
        // A Derived with an empty monomial_grades map reports Asserted under any
        // valuation — the conservative fallback.
        let derived = Derived {
            atom: nullary("empty"),
            support: PosBool::truth(),
            defeaters: PosBool::FALSE,
            conditions: DefeaterConditions::new(),
            monomial_grades: MonomialGrades::new(),
            grade: Grade::Checked, // pooled grade is irrelevant
        };

        let world = World::new();
        let nu = Valuation::new();
        assert_eq!(world.grade_under(&derived, &nu), Grade::Asserted);

        // Under any arbitrary valuation, still Asserted.
        let mut nu2 = Valuation::new();
        nu2.set(Atom(0), true);
        nu2.set(Atom(1), false);
        assert_eq!(world.grade_under(&derived, &nu2), Grade::Asserted);
    }
}
