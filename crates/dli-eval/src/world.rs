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
    /// Predicates the author declared `functional` (single-valued). This carries
    /// no atom and derives nothing — it is a constraint the fork engine consults
    /// so a declared functional dependency is honored on a merge exactly as the
    /// load check honors it (DL-0075). Populated from `functional` declarations by
    /// [`World::lower`] and, on the compiled path, seeded by the `dli-world`
    /// loader; empty for worlds that declare none, so serialization is unchanged.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub functional_predicates: BTreeSet<String>,
    /// Predicates the author declared `inertial` (fluent values persist
    /// forward until a later distinct value defeats them), each mapped to the
    /// **OR of its declaring provenance's [`Ann`]** so the declaration is itself
    /// a defeasible claim (defeating it reverts the slot to plain projection).
    /// The load-side continuity check reads the twin on
    /// [`WorldModel`](crate) — this evaluable-world copy is carried through
    /// forks and the durable store the same way
    /// [`functional_predicates`](World::functional_predicates) is. Populated from
    /// `inertial` declarations by [`World::lower`] and, on the compiled path,
    /// seeded by the `dli-world` loader; empty for worlds that declare none, so
    /// serialization is unchanged.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inertial_predicates: BTreeMap<String, Ann>,
    /// The identity-declaration channel (WS-1): every `Decl::SameAs` pair with
    /// its compiled annotation, populated ONLY by that declaration form — never
    /// by a plain `fact same-as(x, y)`, which pushes an ordinary [`Fact`] (via
    /// [`World::assert_same_as`]'s twin, [`World::lower`]'s `Decl::Fact` arm)
    /// and nothing else. This is the one signal [`synthesize_alias_rules`]
    /// reads to decide both *whether* to close identity at all and *which*
    /// pairs to close over: a `same-as` atom that merely happens to share the
    /// predicate name, with no entry here, never drives a merge — mirroring
    /// how a plain `fact before(a, b)` never adds a `chronology` edge. Empty
    /// for a world that declares no identity, so serialization is unchanged
    /// (the behavior-neutrality pin).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub same_as_links: Vec<(String, String, Ann)>,
    /// The separate predicate-identity declaration channel. Plain facts never
    /// populate this field; only `Decl::PredicateSameAs` and
    /// [`World::assert_predicate_same_as`] do. Empty for identity-free worlds,
    /// preserving their serialized form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub predicate_same_as_links: Vec<(String, String, Ann)>,
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
    #[expect(clippy::too_many_lines)]
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
                // A `functional <pred>` declaration carries no ground atom and
                // interns no provenance (its provenance rides the durable carriage
                // body, DL-0071), but it is not a no-op: it is a constraint the
                // fork engine consults, so the predicate name is recorded in the
                // world's `functional_predicates` set (DL-0075). This mints no
                // variable and derives nothing, so the evaluable core is otherwise
                // untouched — a fork sees the declaration exactly as the load check
                // does, closing the gap where a declared dependency was honored on
                // load but invisible on a merge (issue 59).
                Decl::Functional(functional) => {
                    world
                        .functional_predicates
                        .insert(functional.pred.text.clone());
                }
                // `inertial <pred>` is carriage-only (it mints no ground atom),
                // but — unlike `functional` — its standing is load-bearing: the
                // continuity check caps only while the declaration is certain.
                // So its provenance is compiled into an `Ann` and OR-ed into the
                // predicate's entry, exactly as `Decl::SameAs` above compiles its
                // link's provenance. This evaluable-world copy is carried for
                // forks/merges; the load check reads the `WorldModel` twin.
                Decl::Inertial(inertial) => {
                    let ann = world.lower_fact_annotation(
                        inertial.provenance.as_ref(),
                        &mut proposal_seq,
                        &known_decisions,
                    );
                    or_inertial(&mut world.inertial_predicates, &inertial.pred.text, ann);
                }
                // `same-as <left> = <right>` (WS-1) lowers to the queryable
                // ground fact `same-as(left, right)` — exactly like a `Fact`,
                // never like `Before`'s chronology-edge-only treatment — AND,
                // ONLY for this declaration form, registers the pair in the
                // identity-declaration channel (`same_as_links`) that drives
                // closure. A plain `fact same-as(x, y)` pushes the fact above
                // through the ordinary `Decl::Fact` arm and nothing else, so
                // it never merges anything (koan 3) — the channel, not the
                // predicate name, is what `synthesize_alias_rules` reads.
                Decl::SameAs(same_as) => {
                    let ann = world.lower_fact_annotation(
                        same_as.provenance.as_ref(),
                        &mut proposal_seq,
                        &known_decisions,
                    );
                    world.assert_same_as(&same_as.left.text, &same_as.right.text, ann);
                }
                // Predicate identity is structured carriage only. It has its
                // own declaration channel and never becomes an ordinary fact
                // or an entity-identity link.
                Decl::PredicateSameAs(predicate_same_as) => {
                    let ann = world.lower_fact_annotation(
                        predicate_same_as.provenance.as_ref(),
                        &mut proposal_seq,
                        &known_decisions,
                    );
                    world.assert_predicate_same_as(
                        &predicate_same_as.left.text,
                        &predicate_same_as.right.text,
                        ann,
                    );
                }
                // No atom here, each for a different reason:
                // - `kind`/`rule` need the loader's provenance-gated
                //   compilation (`dli_world::Loader::process_kind` /
                //   `process_rule`, the latter via `World::define_rule`);
                //   this direct document lowering has no equivalent pass.
                // - Dialect/Word/NamePart/Name/Term/Banned/Practice/Proof/
                //   Principle are language- and ledger-layer forms the full
                //   pipeline compiles (`dli-world::Loader` for language
                //   forms, `dli-ledger` for practice/proof/principle), not
                //   this lowering.
                // - `ratify` mints no atom either; it is consumed by the
                //   `known_decisions` pre-pass above, not this per-decl
                //   match.
                // - `attest` carries no atom here either, but it is NOT
                //   inert: `author.add_attest` (contract 1.3.0) is a live,
                //   materializable wire capability, and the ledger's
                //   attestation-sweep machinery reads it downstream of the
                //   store. `load.inert-declaration` is error-severity and
                //   blocks `author.materialize` (verified: flagging Attest
                //   breaks the shipped `add_attest` -> materialize flow), so
                //   it stays a plain no-op here.
                // - Use/Draft/Export/Variant/Narration are carried to the
                //   lorestore but carry no atom here either. Whether these
                //   deprecated-only forms should raise
                //   `load.inert-declaration` (koan 4 / DL-0029) is a per-form
                //   disposition DEFERRED to the deprecation audit (PR-FINAL):
                //   flagging them error-severity regresses the shipped
                //   round-table/camelot reference corpus (`use language`,
                //   `use chronology`, `narration`), so this stays
                //   behavior-neutral and the audit rules per form + fixes the
                //   corpus if a form turns out to be a genuine defect.
                Decl::Kind(_)
                | Decl::Rule(_)
                | Decl::Dialect(_)
                | Decl::Word(_)
                | Decl::NamePart(_)
                | Decl::Name(_)
                | Decl::Term(_)
                | Decl::Banned(_)
                | Decl::Practice(_)
                | Decl::Proof(_)
                | Decl::Principle(_)
                | Decl::Ratify(_)
                | Decl::Attest(_)
                | Decl::Use(_)
                | Decl::Draft(_)
                | Decl::Export(_)
                | Decl::Variant(_)
                | Decl::Narration(_) => {}
            }
        }
        // Identity closure (WS-1): synthesize the symmetric/transitive/rewrite
        // rules over every `same-as` fact just lowered, so this path and the
        // compiled loader's `finish()` call agree (three-path parity). A world
        // with zero `same-as` facts synthesizes zero rules.
        synthesize_alias_rules(&mut world);
        synthesize_predicate_alias_rules(&mut world);
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
    /// and is recorded as a finding rather than silently dropped (charter koan 4),
    /// mirroring the loader's `load.inert-declaration` so neither lowering path is
    /// a silent-drop offender (issue 47).
    fn lower_state(
        &mut self,
        state: &StateDecl,
        proposal_seq: &mut u32,
        known_decisions: &BTreeSet<&str>,
    ) -> Result<(), EvalError> {
        let Some(pattern) = state.atom() else {
            self.findings.push(LowerFinding {
                code: "load.inert-declaration".to_owned(),
                message: format!(
                    "state '{}' names nothing evaluable, so it lowers to no fact",
                    state.name.text
                ),
            });
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
    ///
    /// Delegates to [`define_rule_annotated`](World::define_rule_annotated) with
    /// [`Ann::unconditional()`] and [`Grade::Asserted`] — the engine-internal and
    /// test intake this crate has always had, untouched by the annotated form's
    /// arrival.
    pub fn define_rule(
        &mut self,
        name: impl Into<String>,
        head: AtomExpr,
        body: impl IntoIterator<Item = impl Into<BodyLiteral>>,
    ) -> Result<(), EvalError> {
        self.define_rule_annotated(name, head, body, Ann::unconditional(), Grade::Asserted)
    }

    /// Define a derivation rule carrying its own annotation and grade — the
    /// rule-as-claim intake (WS-2). `fire_rules` already multiplies a rule's
    /// `ann` into every derivation passing through it and weakest-grades with
    /// `rule.grade`; this is the constructor a provenanced caller (the loader,
    /// the `author.add_rule` wire op) uses so that multiplication has something
    /// real to work with, instead of every rule reading as an unconditional
    /// axiom. Same range-restriction check as [`define_rule`](World::define_rule).
    pub fn define_rule_annotated(
        &mut self,
        name: impl Into<String>,
        head: AtomExpr,
        body: impl IntoIterator<Item = impl Into<BodyLiteral>>,
        ann: Ann,
        grade: Grade,
    ) -> Result<(), EvalError> {
        let name = name.into();
        let body: Vec<BodyLiteral> = body.into_iter().map(Into::into).collect();
        check_range_restricted(&name, &head, &body)?;
        self.rules.push(Rule {
            name,
            head,
            body,
            ann,
            grade,
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

    /// Declare an identity link (WS-1) — the ONLY intake that registers a pair
    /// in the identity-declaration channel `same_as_links`, which
    /// [`synthesize_alias_rules`] reads to decide whether, and which pairs, to
    /// close. Also pushes the ordinary queryable `same-as(left, right)` fact
    /// with the same annotation, exactly as `Decl::Before` keeps its own
    /// `before(a, b)` fact alongside its (separate) chronology edge. A caller
    /// that wants a `same-as`-predicate fact with NO merge consequence should
    /// call [`assert_fact`](World::assert_fact) directly instead — that is
    /// precisely the plain-`Decl::Fact` path, and it is deliberately inert
    /// here.
    pub fn assert_same_as(&mut self, left: impl Into<String>, right: impl Into<String>, ann: Ann) {
        let left = left.into();
        let right = right.into();
        self.facts.push(Fact {
            atom: GroundAtom {
                pred: "same-as".to_owned(),
                args: vec![left.clone(), right.clone()],
            },
            ann: ann.clone(),
            grade: Grade::Asserted,
        });
        self.same_as_links.push((left, right, ann));
    }

    /// Declare a predicate identity link, the predicate-identity kernel's only
    /// evaluator intake. The pair retains its support annotation;
    /// [`synthesize_predicate_alias_rules`] turns it into symmetric rewrite
    /// rules after facts and typed rules are present. Unlike entity identity,
    /// this does not create a public ground fact: the declaration is structured
    /// carriage, not an authorable predicate relation.
    pub fn assert_predicate_same_as(
        &mut self,
        left: impl Into<String>,
        right: impl Into<String>,
        ann: Ann,
    ) {
        self.predicate_same_as_links
            .push((left.into(), right.into(), ann));
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
            saturate_stratum(&mut relation, &self.level_rules(level), &self.theory)?;
        }
        Ok(self.finish_saturation(relation))
    }

    /// The **incremental** fixpoint (DL-0087): saturate `self` — which must be
    /// `prior`'s program plus *appended* facts and/or rules — by reusing
    /// `prior`'s saturation instead of recomputing it, and return a world byte-
    /// identical to [`saturate_with`](World::saturate_with) on the same program.
    ///
    /// `prior` must already be saturated (its `derived` is the fixpoint of its
    /// own facts and rules) under a stratification of *its* program. When the
    /// preconditions do not hold — `self` revises rather than extends `prior`,
    /// the interner was renumbered, the theory changed, or `prior` is not
    /// saturated — this silently falls back to the full
    /// [`saturate_with`](World::saturate_with), so it is always safe to call.
    ///
    /// # The reuse unit is a **dependency-closed stratum**, not an atom
    ///
    /// The delta's *affected* predicate set is the closure of the added facts'
    /// predicates and the added rules' head predicates under "a rule whose body
    /// mentions an affected predicate has an affected head", widened so that a
    /// stratum with any affected predicate is affected whole (a recursive clique
    /// is saturated as a unit or not at all). Every predicate outside that set
    /// has, provably, the *same* derivations it had in `prior`: no new fact
    /// carries it, and every rule deriving it reads only unaffected predicates.
    /// So its prior rows are reused verbatim and its strata are skipped entirely;
    /// affected strata are re-saturated from the reused rows, in the ordinary
    /// stratum order, by the ordinary `saturate_stratum` worklist.
    ///
    /// # Why a retained row is *not* safe at atom granularity
    ///
    /// It is tempting to keep every prior row and merely seed the worklist with
    /// the delta. That is unsound here, and not subtly: `CondSupport::or_`
    /// **conjoins** the defeaters of a monomial reached two ways, because two
    /// genuine derivations of one path defeat it only when both are defeated. A
    /// re-derivation of the *same* path in a *changed* program is not a second
    /// derivation, so folding it into the stale row computes `d ∧ (d ∨ poison) =
    /// d` and the new poison vanishes — the atom keeps reading certain where a
    /// full recompute defeats it. Retracting the stale, defeater-free version
    /// would be over-deletion (`DRed`), which DL-0087 rules out. Dropping the
    /// affected predicates' rows and re-deriving them is the correction, and it
    /// is what makes byte-identity a construction rather than a hope.
    ///
    /// The permanent oracle, `crates/dli-eval/tests/incremental_parity.rs`,
    /// checks `saturate_incremental == saturate_with` byte-for-byte over
    /// generated programs and every ordering of the delta.
    // TODO(phase3): finer than stratum granularity — an atom-level delta needs
    // per-derivation provenance (so a re-derivation can replace, rather than
    // join with, its own prior contribution) before a retained row is sound
    // under negation. Until then the reuse unit is the dependency-closed
    // stratum, which is what the byte-identity argument above rests on.
    pub fn saturate_incremental(&self, prior: &World, strata: &Strata) -> Result<World, EvalError> {
        Ok(self.saturate_incremental_reported(prior, strata)?.0)
    }

    /// [`saturate_incremental`](World::saturate_incremental), returning a
    /// [`ReuseReport`] alongside the world: what was reused, what was
    /// recomputed, and whether reuse ran at all.
    ///
    /// # This exists so a parity test cannot go vacuous
    ///
    /// Reuse falls back to a full [`saturate_with`](World::saturate_with)
    /// whenever the prior is not a sound base, which is the right behaviour and a
    /// trap for anything that *checks* it: an oracle comparing
    /// `saturate_incremental` against `saturate_with` passes trivially if every
    /// case fell back, because then it is comparing a full recompute against
    /// itself. That failure is invisible — the suite is green and tests nothing.
    /// The report makes reuse observable, so the oracle can put a floor on how
    /// many trials genuinely reused something and go red when the answer is
    /// none.
    pub fn saturate_incremental_reported(
        &self,
        prior: &World,
        strata: &Strata,
    ) -> Result<(World, ReuseReport), EvalError> {
        let Some(affected) = reusable_delta(self, prior, strata) else {
            return Ok((self.saturate_with(strata)?, ReuseReport::fallback()));
        };
        let mut report = ReuseReport {
            reused: true,
            affected: affected.clone(),
            ..ReuseReport::default()
        };
        let mut relation = self.seed_relation();
        // Reuse: an unaffected predicate's prior row already folds its facts and
        // every derivation that reaches it, so it *replaces* the seed rather than
        // merging into it — the seed is one of the contributions it already
        // carries.
        for row in &prior.derived {
            if !affected.contains(&row.atom.pred) {
                relation.insert(row.atom.clone(), row.clone());
                report.reused_rows += 1;
            }
        }
        // Every level runs in stratification order, and the rows reused above
        // include predicates from levels *later* than the one being saturated —
        // they are preloaded before level 0 rather than derived in turn. That is
        // sound only because `stratify_rules` guarantees no rule at level k names
        // a predicate from a later level in its body: the relation being a
        // superset is invisible to level k, because `fire_rules` and `join_body`
        // match body atoms by predicate name and no such name is present. If that
        // guarantee ever weakens, this preload becomes a silent divergence.
        for level in &strata.levels {
            if level.iter().any(|pred| affected.contains(pred)) {
                saturate_stratum(&mut relation, &self.level_rules(level), &self.theory)?;
                report.recomputed_strata += 1;
            } else {
                report.skipped_strata += 1;
            }
        }
        Ok((self.finish_saturation(relation), report))
    }

    /// [`saturate_incremental`](World::saturate_incremental), cross-checked
    /// against a full [`saturate_with`](World::saturate_with) of the same
    /// program — the **verify-against-full mode**.
    ///
    /// This computes both fixpoints and, when they disagree, **panics** naming
    /// the first atom that differs. It is strictly slower than a full recompute
    /// (it *is* a full recompute, plus the incremental one), so it buys no speed
    /// and is not a production path. What it buys is that a divergence surfaces
    /// at the seam that caused it, instead of as a wrong verdict somewhere
    /// downstream — the failure mode that makes an incremental evaluator worse
    /// than none at all.
    ///
    /// # Why a panic and not an error
    ///
    /// A divergence is a broken kernel invariant, not a condition any caller can
    /// handle: there is no "recover and carry on" from a saturation that does not
    /// reproduce its own full recompute, because every verdict read from it is
    /// unreplayable. Rust spells that a panic. It is also the *honest* channel —
    /// routing it through [`EvalError`] would give it a wire error code, and every
    /// code in that taxonomy names something wrong with the **corpus**, so a
    /// caller would be told their lore is broken when the truth is that the
    /// evaluator is. Callers that want to decide for themselves have
    /// [`incremental_divergence`], which returns the same report as a value and
    /// never panics.
    ///
    /// [`ForkStore`](crate::fork::ForkStore) calls this instead of the plain
    /// primitive whenever `debug_assertions` is on, so every debug run and every
    /// test in the workspace re-checks the reuse against the shipped corpora, not
    /// only against the generated programs in the oracle — and compiles back to
    /// the plain primitive in release, exactly like a debug assertion.
    pub fn saturate_incremental_verified(
        &self,
        prior: &World,
        strata: &Strata,
    ) -> Result<World, EvalError> {
        let incremental = self.saturate_incremental(prior, strata)?;
        let full = self.saturate_with(strata)?;
        assert!(
            incremental == full,
            "incremental saturation diverged from a full recompute of the same program \
             (this is an evaluator bug, not a corpus error): {}",
            incremental_divergence(&incremental, &full)
                .unwrap_or_else(|| "no atom-level difference found".to_owned())
        );
        Ok(incremental)
    }

    /// The rules whose head predicate sits in one stratum level.
    fn level_rules(&self, level: &[String]) -> Vec<&Rule> {
        let level_set: BTreeSet<&str> = level.iter().map(String::as_str).collect();
        self.rules
            .iter()
            .filter(|rule| level_set.contains(rule.head.pred.text.as_str()))
            .collect()
    }

    /// Turn a saturated relation into the saturated world: the sorted `derived`,
    /// the rebuilt inverted index, and the fact-seeded `var_grades`. Shared by
    /// the full and incremental fixpoints so neither can drift from the other.
    fn finish_saturation(&self, relation: BTreeMap<GroundAtom, Derived>) -> World {
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
        world
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

/// Synthesize the identity-closure rules over a world's DECLARED identity
/// links (WS-1, the identity kernel), iff `world.same_as_links` — the channel
/// [`World::assert_same_as`] populates, and ONLY that method — is non-empty.
/// A world with none synthesizes nothing, which is what makes a world with
/// zero `same-as` declarations byte-identical to one with none of this
/// machinery at all (the behavior-neutrality pin every existing golden and
/// oracle relies on). Called from both lowering paths — [`World::lower`] and
/// the compiled `dli-world` loader's `finish()` — so a corpus authored on the
/// wire, through `.dli` text, or reconstructed from the durable store all
/// synthesize the identical rule set (three-path parity).
///
/// **Declaration-only, by construction, not by a predicate-name filter.** The
/// closure must never pick up a plain `fact same-as(x, y)` — DL-0078 rules
/// identity a claim the declaration form alone makes (koan 3), the same
/// asymmetry `Decl::Before`'s chronology edge already has against a plain
/// `fact before(a, b)`. A predicate-name guard (“does ANY `same-as` atom
/// exist”) is not enough: once ANY rule referencing the public `same-as`
/// relation exists, it would join against every ground tuple under that
/// predicate regardless of origin — including an unrelated plain fact sharing
/// the name. So every rule below is defined over a **reserved, unauthorable**
/// internal predicate, `same-as:link` (`:` cannot open an author-facing
/// predicate name), extensionally seeded HERE, once per call, directly from
/// the channel — never by reading `world.facts`. A plain `same-as`-predicate
/// fact never enters `same-as:link` and therefore never reaches any of these
/// rules, no matter what else in the world triggers synthesis.
///
/// The synthesized rules, in deterministic order:
///
/// - one **extensional `same-as:link` fact per declared pair**, seeded with
///   THAT pair's own compiled annotation — this is where defeasibility rides:
///   defeating the pair's citing decision (one valuation flip) makes this row
///   (and everything derived through it) dead on the next read, with no
///   re-fixpoint, exactly as the original queryable `same-as(left, right)`
///   fact [`World::assert_same_as`] pushed alongside it already would.
/// - **symmetry** — `same-as:link(Y, X) :- same-as:link(X, Y)`.
/// - **transitivity** — `same-as:link(X, Z) :- same-as:link(X, Y),
///   same-as:link(Y, Z)`.
/// - **public exposure** — `same-as(X, Y) :- same-as:link(X, Y)`, so the
///   full symmetric-transitive class (not only the originally-declared pairs)
///   answers under the ordinary `same-as` predicate too — closure runs both
///   directions with no goal rewriting needed anywhere, including here.
/// - **per-predicate rewrite** — for every `(pred, arity)` observed among
///   extensional facts and rule heads (EXCLUDING `same-as` AND `same-as:link`
///   themselves, so this census never rewrites the identity relation through
///   itself), and for every argument position `i` of that predicate:
///   `pred(.., Y, ..) :- pred(.., X, ..), same-as:link(X, Y)` with `Y` at
///   position `i`. This is the mechanism that makes a merged copy answer
///   under every alias name — no query-side goal rewriting is needed, because
///   the copy is a first-class derived row like any other.
///
/// Every rule is positive and range-restricted (every head variable is bound
/// by a positive body atom) and touches no negated literal, so stratification
/// is unaffected — the synthesized rules can join any existing stratum without
/// the loader's `dli_eval::stratify` seam ever seeing a new cycle from them.
/// Rule names are reserved: `same-as:sym`, `same-as:trans`, `same-as:public`,
/// and `same-as:<pred>/<arity>@<pos>`.
///
/// A rule's own `Ann` multiplies into every derivation through it
/// (`fire_rules`); `define_rule` mints `Ann::unconditional()` (the neutral
/// element), so a synthesized rule never narrows a merged copy's support below
/// the conjunction of the seeded `same-as:link` row's annotation — the
/// declared pair's own — and the base fact(s) it rode in on. The merge is
/// exactly as defeasible as the identity claim itself (spec 02: rules-as-claims).
pub fn synthesize_alias_rules(world: &mut World) {
    const SAME_AS: &str = "same-as";
    const SAME_AS_LINK: &str = "same-as:link";
    if world.same_as_links.is_empty() {
        return;
    }

    // Seed the reserved link relation, ONE extensional fact per declared
    // pair, each carrying that pair's own compiled annotation — never the
    // public `same-as` relation, which may also hold unrelated plain facts.
    for (left, right, ann) in world.same_as_links.clone() {
        world.facts.push(Fact {
            atom: GroundAtom {
                pred: SAME_AS_LINK.to_owned(),
                args: vec![left, right],
            },
            ann,
            grade: Grade::Asserted,
        });
    }

    let var = |name: &str| Term::Var(Ident::synthetic(name));
    let link_atom = |left: &str, right: &str| AtomExpr {
        pred: Ident::synthetic(SAME_AS_LINK),
        args: vec![var(left), var(right)],
    };

    // Symmetry over the link relation: same-as:link(Y, X) :- same-as:link(X, Y).
    let _ = world.define_rule(
        "same-as:sym",
        link_atom("Y", "X"),
        vec![link_atom("X", "Y")],
    );
    // Transitivity over the link relation.
    let _ = world.define_rule(
        "same-as:trans",
        link_atom("X", "Z"),
        vec![link_atom("X", "Y"), link_atom("Y", "Z")],
    );
    // Public exposure: the full closure answers under the ordinary predicate.
    let _ = world.define_rule(
        "same-as:public",
        AtomExpr {
            pred: Ident::synthetic(SAME_AS),
            args: vec![var("X"), var("Y")],
        },
        vec![link_atom("X", "Y")],
    );

    // Census: every (pred, arity) over facts ∪ rule heads, excluding same-as
    // AND same-as:link themselves, in deterministic (BTreeSet) order.
    let mut census: BTreeSet<(String, usize)> = BTreeSet::new();
    for fact in &world.facts {
        if fact.atom.pred != SAME_AS && fact.atom.pred != SAME_AS_LINK {
            census.insert((fact.atom.pred.clone(), fact.atom.args.len()));
        }
    }
    for rule in &world.rules {
        if rule.head.pred.text != SAME_AS && rule.head.pred.text != SAME_AS_LINK {
            census.insert((rule.head.pred.text.clone(), rule.head.args.len()));
        }
    }

    for (pred, arity) in census {
        for pos in 0..arity {
            let arg_vars: Vec<String> = (0..arity).map(|i| format!("A{i}")).collect();
            let body_atom = AtomExpr {
                pred: Ident::synthetic(&pred),
                args: arg_vars.iter().map(|v| var(v)).collect(),
            };
            let mut head_args: Vec<Term> = arg_vars.iter().map(|v| var(v)).collect();
            head_args[pos] = var("Y");
            let head = AtomExpr {
                pred: Ident::synthetic(&pred),
                args: head_args,
            };
            let body = vec![body_atom, link_atom(&arg_vars[pos], "Y")];
            let name = format!("same-as:{pred}/{arity}@{pos}");
            let _ = world.define_rule(name, head, body);
        }
    }
}

/// Synthesize predicate-identity rewrite rules over the separate declared
/// predicate links.
///
/// Predicate names cannot be variables in ordinary Datalog, so predicate
/// identity is compiled into one pair of ordinary positive rules for each
/// direct declaration edge and each observed arity in that edge's connected
/// component:
///
/// ```text
/// current_job(A0, ...) :- current_work(A0, ...),
///     predicate-same-as:link("current-work", "current-job")
/// current_work(A0, ...) :- current_job(A0, ...),
///     predicate-same-as:link("current-work", "current-job")
/// ```
///
/// Chaining those rules supplies symmetric-transitive closure. The internal
/// link fact carries the declaration's [`Ann`], so every copied or derived row
/// carries the identity support polynomial. A proposed link therefore yields a
/// possible copy; defeating its citation kills the copy on the next valuation
/// read while leaving the source fact untouched.
///
/// The rule plan is deterministic: direct edges, predicate names, and arities
/// are all ordered by `BTree*` collections. Calling this function repeatedly is
/// safe; reserved link facts and generated rule names are not duplicated.
pub fn synthesize_predicate_alias_rules(world: &mut World) {
    const LINK: &str = "predicate-same-as:link";

    let direct: BTreeSet<(String, String)> = world
        .predicate_same_as_links
        .iter()
        .map(|(left, right, _)| (left.clone(), right.clone()))
        .filter(|(left, right)| left != right)
        .collect();
    if direct.is_empty() {
        return;
    }

    // Seed one internal link fact per declaration, preserving every
    // provenance alternative for duplicate declarations of the same pair.
    let seeded: BTreeSet<(String, String)> = world
        .facts
        .iter()
        .filter(|fact| fact.atom.pred == LINK && fact.atom.args.len() == 2)
        .map(|fact| (fact.atom.args[0].clone(), fact.atom.args[1].clone()))
        .collect();
    for (left, right, ann) in &world.predicate_same_as_links {
        if left == right || seeded.contains(&(left.clone(), right.clone())) {
            continue;
        }
        world.facts.push(Fact {
            atom: GroundAtom {
                pred: LINK.to_owned(),
                args: vec![left.clone(), right.clone()],
            },
            ann: ann.clone(),
            grade: Grade::Asserted,
        });
    }

    // Census every predicate occurrence, including rule bodies. A body-only
    // predicate is still a real input to an identity rewrite and must not be
    // missed just because it has no head of its own.
    let mut arities: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for fact in &world.facts {
        if fact.atom.pred != LINK {
            arities
                .entry(fact.atom.pred.clone())
                .or_default()
                .insert(fact.atom.args.len());
        }
    }
    for rule in &world.rules {
        if rule.head.pred.text != LINK {
            arities
                .entry(rule.head.pred.text.clone())
                .or_default()
                .insert(rule.head.args.len());
        }
        for literal in &rule.body {
            if literal.atom.pred.text != LINK {
                arities
                    .entry(literal.atom.pred.text.clone())
                    .or_default()
                    .insert(literal.atom.args.len());
            }
        }
    }

    // A connected component's observed shapes are the available rewrite
    // shapes for every edge in that component. A richer declared shape system
    // can tighten this boundary later; the kernel itself stays deterministic
    // and does not infer identity from the observed data.
    let identity_map = dli_core::predicate_alias::predicate_canon_map(direct.iter().cloned());
    let mut component_arities: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for (pred, pred_arities) in &arities {
        let root = dli_core::alias::canon(&identity_map, pred).to_owned();
        component_arities
            .entry(root)
            .or_default()
            .extend(pred_arities);
    }

    let var = |name: &str| Term::Var(Ident::synthetic(name));
    let link_atom = |left: &str, right: &str| AtomExpr {
        pred: Ident::synthetic(LINK),
        args: vec![
            Term::Sym(Ident::synthetic(left)),
            Term::Sym(Ident::synthetic(right)),
        ],
    };
    let existing_names: BTreeSet<String> =
        world.rules.iter().map(|rule| rule.name.clone()).collect();
    let mut generated_names = existing_names;

    for (left, right) in direct {
        let root = dli_core::alias::canon(&identity_map, &left).to_owned();
        let Some(component_shapes) = component_arities.get(&root) else {
            continue;
        };
        // A declaration that joins incompatible observed shapes is a loud,
        // graded refusal, not a partial merge. The loader reports the exact
        // component mismatch; the evaluator also fails closed here so no
        // arity-specific rewrite can leak through before materialization.
        if component_shapes.len() > 1 {
            continue;
        }
        for arity in component_shapes {
            let args: Vec<String> = (0..*arity).map(|index| format!("P{index}")).collect();
            let source = |pred: &str| AtomExpr {
                pred: Ident::synthetic(pred),
                args: args.iter().map(|name| var(name)).collect(),
            };

            let forward_name = format!("predicate-same-as:{left}->{right}/{arity}");
            if generated_names.insert(forward_name.clone()) {
                let _ = world.define_rule(
                    forward_name,
                    source(&right),
                    vec![
                        BodyLiteral::pos(source(&left)),
                        BodyLiteral::pos(link_atom(&left, &right)),
                    ],
                );
            }

            let reverse_name = format!("predicate-same-as:{right}->{left}/{arity}");
            if generated_names.insert(reverse_name.clone()) {
                let _ = world.define_rule(
                    reverse_name,
                    source(&left),
                    vec![
                        BodyLiteral::pos(source(&right)),
                        BodyLiteral::pos(link_atom(&left, &right)),
                    ],
                );
            }
        }
    }
}

/// OR one declaring provenance's [`Ann`] into a predicate's inertia entry
/// (`docs/decisions.md` DL-0081). A predicate declared `inertial` more than once
/// is inertial under the **disjunction** of its declarations' supports, so
/// defeating one declaration leaves the slot inertial while any other still
/// stands. Both minting seams (`dli_world`'s `compile_ann`, this crate's
/// `World::lower_fact_annotation`) produce positive-only annotations — no
/// defeater conditions — so the join is exactly the disjunction of supports.
pub fn or_inertial(map: &mut BTreeMap<String, Ann>, pred: &str, ann: Ann) {
    match map.get_mut(pred) {
        Some(existing) => *existing = Ann::supported(existing.support.or_(&ann.support)),
        None => {
            map.insert(pred.to_owned(), ann);
        }
    }
}

/// Whether a world's `derived` is consistent with having been saturated from its
/// own facts and rules — the cheap necessary conditions a reuse base must meet.
///
/// This is a **screen, not a proof**. The type system does not distinguish a
/// saturated `World` from a program, so a prior that merely *looks* saturated —
/// saturated once and then appended to, most of all — passes every structural
/// precondition (`derived` is non-empty, the facts extend cleanly) while
/// carrying rows that no longer reflect its own program. Reuse would then copy
/// stale rows forward and produce a world a full recompute disagrees with, in
/// release, with nothing to notice. So this checks the two invariants a genuine
/// saturation cannot violate:
///
/// - **every fact's atom has a row.** [`World::seed_relation`] inserts one per
///   fact and saturation never removes rows, so a missing row means the fact was
///   appended after the fixpoint ran.
/// - **every fact's support is entailed by its atom's row.** Support only grows
///   through saturation, so a fact whose own support its row does not carry is a
///   fact the fixpoint never saw — the case where the appended fact's atom
///   already had a row from an earlier fact and the presence check alone passes.
///
/// It also refuses a world with rules and no rows at all: a pure-negative-body
/// rule with a ground head derives without any fact, so an empty `derived` there
/// is indistinguishable from a fixpoint that never ran.
///
/// Every failure mode is a **refusal**, never a wrong answer: the caller falls
/// back to the full fixpoint, costing time and nothing else. What remains
/// uncovered is a prior stale in a way these invariants cannot see. The durable
/// fix is a saturated-world newtype that cannot be forged — an unforgeable
/// contract rather than a screen — scheduled for PR3, where the lorestore cache
/// makes a stale prior load-bearing rather than hypothetical.
fn looks_saturated(prior: &World) -> bool {
    if prior.derived.is_empty() {
        return prior.facts.is_empty() && prior.rules.is_empty();
    }
    let rows: BTreeMap<&GroundAtom, &Derived> =
        prior.derived.iter().map(|row| (&row.atom, row)).collect();
    prior.facts.iter().all(|fact| {
        rows.get(&fact.atom)
            .is_some_and(|row| fact.ann.support.implies(&row.support))
    })
}

/// What one call to [`World::saturate_incremental_reported`] actually did.
///
/// Reuse is best-effort by design — it falls back to a full fixpoint whenever
/// the prior is not a sound base — so "the answer was right" says nothing about
/// whether any reuse happened. This is the channel that does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReuseReport {
    /// Whether reuse ran at all. `false` means the preconditions were refused
    /// and the result came from a plain full fixpoint.
    pub reused: bool,
    /// The predicates the delta can disturb, whose prior rows were dropped and
    /// re-derived. Empty on a fallback.
    pub affected: BTreeSet<String>,
    /// Prior rows copied forward verbatim.
    pub reused_rows: usize,
    /// Stratum levels skipped whole because nothing in them was affected.
    pub skipped_strata: usize,
    /// Stratum levels re-saturated.
    pub recomputed_strata: usize,
}

impl ReuseReport {
    /// The report for a call that refused reuse and ran the full fixpoint.
    #[must_use]
    pub fn fallback() -> Self {
        Self::default()
    }

    /// Whether this call genuinely reused work: it took the reuse path *and*
    /// skipped at least one stratum. A call that reused the path but recomputed
    /// every level did the full job with extra bookkeeping, and a test counting
    /// it as coverage would be fooling itself.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.reused && self.skipped_strata > 0
    }
}

/// Describe the first disagreement between an incremental saturation and the
/// full recompute it is supposed to reproduce, or `None` when they are
/// identical — the value-level half of the verify-against-full mode.
///
/// Keyed by atom, so the report names the row to go look at rather than dumping
/// two whole worlds into a diagnostic. It reports the first divergence in atom
/// order, which may be a *consequence* of the fault rather than its cause: this
/// says what diverged, not what broke.
///
/// [`World::saturate_incremental_verified`] is the assertive form (it panics on
/// a non-`None` result); this is here for callers that want to check reuse
/// without surrendering control of what happens next — a harness, a
/// non-gating audit lane, or a wire site that would rather degrade to a full
/// recompute than abort.
#[must_use]
pub fn incremental_divergence(incremental: &World, full: &World) -> Option<String> {
    if incremental == full {
        return None;
    }
    Some(divergence_detail(incremental, full))
}

/// The first disagreement, as prose. Only reached when the two worlds are known
/// to differ, so the fallthroughs describe a difference outside `derived`.
fn divergence_detail(incremental: &World, full: &World) -> String {
    let mine: BTreeMap<&GroundAtom, &Derived> = incremental
        .derived
        .iter()
        .map(|row| (&row.atom, row))
        .collect();
    let theirs: BTreeMap<&GroundAtom, &Derived> =
        full.derived.iter().map(|row| (&row.atom, row)).collect();
    for (atom, row) in &theirs {
        match mine.get(atom) {
            None => {
                return format!(
                    "{} is derived by the full fixpoint but missing from the reused one",
                    atom.render()
                );
            }
            Some(reused) if reused != row => {
                return format!(
                    "{} disagrees — reused support {:?} conditions {:?} grade {:?}, full support {:?} conditions {:?} grade {:?}",
                    atom.render(),
                    reused.support,
                    reused.conditions,
                    reused.grade,
                    row.support,
                    row.conditions,
                    row.grade
                );
            }
            Some(_) => {}
        }
    }
    for atom in mine.keys() {
        if !theirs.contains_key(atom) {
            return format!(
                "{} is derived only by the reused fixpoint, never by the full one",
                atom.render()
            );
        }
    }
    if incremental.support_index != full.support_index {
        return "the derived relation agrees but the inverted support index does not".to_owned();
    }
    if incremental.var_grades != full.var_grades {
        return "the derived relation agrees but the per-variable grades do not".to_owned();
    }
    "the two saturations differ outside the derived relation".to_owned()
}

/// The predicates whose saturated rows a delta can disturb — or `None` when
/// `prior` is not a reusable base for `new` at all and the caller must fall back
/// to a full fixpoint.
///
/// Reuse is **append-only**: `new`'s facts, rules and interned names must extend
/// `prior`'s, and the constraint theory must be unchanged (it feeds
/// [`negative_poison`], so a new exclusivity family can move any defeater in the
/// world). `prior` must additionally pass [`looks_saturated`], which screens out
/// the priors whose rows do not reflect their own program — read that function's
/// note for what the screen does and does not cover, and for why the durable fix
/// is a saturated-world newtype in PR3 rather than more checks here.
///
/// The affected set is the least set containing
///
/// - every added fact's predicate and every added rule's head predicate, and
/// - the head of any rule with an affected predicate in its body (positive *or*
///   negated — a new poison travels the negative edge too), and
/// - every predicate sharing a stratum level with an affected predicate.
///
/// The last clause is what makes reuse a *construction*: a level this set
/// touches is re-saturated with its **whole** rule set, from the relation state
/// a full fixpoint would have at that point, so the two runs perform the same
/// merges in the same order and byte-identity is an argument rather than a hope.
/// Everything outside the set is provably unchanged — no added fact names it,
/// and every rule deriving it reads only unaffected predicates — so its prior
/// rows are reused and the levels that build them are skipped.
///
/// The clause has **no known reachable case that justifies it**, and is kept as
/// a cheap guard rather than as a live mechanism — stated plainly so nobody
/// mistakes it for load-bearing. It is not merely shadowed by
/// [`stratify_rules`] putting one strongly-connected component per level: it is
/// *subsumed* by the head-from-body closure above, which walks the dependency
/// edges in reverse and therefore already carries affectedness around any cycle
/// to every member of its own component. Removing it leaves the oracle green,
/// and a red-team pass could not construct a program where it changes the
/// answer. It costs one pass over the levels, so it stays — but if it ever needs
/// deleting, delete it, and do not reason as though something was protecting it.
///
/// [`stratify_rules`]: crate::strata::stratify_rules
fn reusable_delta(new: &World, prior: &World, strata: &Strata) -> Option<BTreeSet<String>> {
    if new.theory != prior.theory
        || !new.facts.starts_with(&prior.facts)
        || !new.rules.starts_with(&prior.rules)
        || !new.interner.names().starts_with(prior.interner.names())
    {
        return None;
    }
    if !looks_saturated(prior) {
        return None;
    }
    let mut affected: BTreeSet<String> = BTreeSet::new();
    for fact in &new.facts[prior.facts.len()..] {
        affected.insert(fact.atom.pred.clone());
    }
    for rule in &new.rules[prior.rules.len()..] {
        affected.insert(rule.head.pred.text.clone());
    }
    loop {
        let mut grew = false;
        for rule in &new.rules {
            if affected.contains(&rule.head.pred.text) {
                continue;
            }
            if rule
                .body
                .iter()
                .any(|lit| affected.contains(&lit.atom.pred.text))
            {
                affected.insert(rule.head.pred.text.clone());
                grew = true;
            }
        }
        for level in &strata.levels {
            if !level.iter().any(|pred| affected.contains(pred)) {
                continue;
            }
            for pred in level {
                grew |= affected.insert(pred.clone());
            }
        }
        if !grew {
            break;
        }
    }
    Some(affected)
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
/// clean path); `CondSupport::defeater_of` reads it. For a positive-only
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
/// derivations arrive — and the atom is re-enqueued whenever **either half** of
/// its conditioned support moved. The atom-global `defeaters` is kept as the OR
/// of the conditions for blame and citation.
///
/// **Support alone is not the change measure.** A merge can leave the support
/// monomials identical and still move the row: a second derivation of the same
/// path with a different poison re-conditions that monomial
/// (`CondSupport::or_` conjoins the two defeaters). Gating the re-enqueue on
/// support alone stranded that move inside the row — a consumer sharing the
/// stratum (a recursive clique, or a delta-driven re-saturation) never saw it,
/// so an atom kept a defeater a full derivation clears, or missed one it should
/// carry. Both halves are monotone under merging — support grows, each surviving
/// monomial's defeater weakens — over a finite lattice, so the wider gate still
/// terminates.
fn merge_into(
    relation: &mut BTreeMap<GroundAtom, Derived>,
    atom: GroundAtom,
    grade: Grade,
    cond: &CondSupport,
) -> Option<GroundAtom> {
    if let Some(row) = relation.get_mut(&atom) {
        row.grade = Grade::join_alt([row.grade, grade]);
        let merged = CondSupport::of_row(row).or_(cond);
        let changed = merged.support != row.support || merged.conditions != row.conditions;
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
