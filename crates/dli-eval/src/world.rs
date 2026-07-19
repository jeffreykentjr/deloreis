//! The evaluable world program: lowering, the annotated fixpoint, and query.
//!
//! A [`World`] is produced by [`World::lower`]ing a document, [`World::fixpoint`]
//! computes the symbolic support of every derivable atom *once*, and queries
//! read that saturation under a valuation. Because provenance is precomputed and
//! valuations are applied afterward, a fork re-evaluates through
//! [`World::eval_under`] with **no** re-lowering and no re-fixpoint.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::annot::{Annotation, CondExpr, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, OptionClause, TimeClause};
use dli_ast::ident::Ident;
use dli_ast::term::{AtomExpr, Term, TimeRef};
use dli_core::ann::{Ann, Status, Theory};
use dli_core::atom::{Atom, Interner};
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;
use dli_core::verdict::Verdict;
use dli_time::chronology::{Chronology, TimePoint};
use dli_time::wheel::Wheel;

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

/// A lowered derivation rule: an (unground) head and body carrying the rule's
/// own annotation, which multiplies into every derivation passing through it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Rule {
    /// The rule name.
    pub name: String,
    /// The head atom pattern the rule derives.
    pub head: AtomExpr,
    /// The conjunctive body atom patterns.
    pub body: Vec<AtomExpr>,
    /// The rule's own annotation.
    pub ann: Ann,
    /// The rule's grade.
    pub grade: Grade,
}

/// A saturated relation row: a ground atom with its computed support, defeaters,
/// and grade. Status is *not* stored — it is evaluated at query time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Derived {
    /// The ground atom.
    pub atom: GroundAtom,
    /// Its support polynomial `P⁺`.
    pub support: PosBool,
    /// Its defeater polynomial `P⁻`.
    pub defeaters: PosBool,
    /// Its grade.
    pub grade: Grade,
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
    /// The time-point name table: `TimePoint(i)` is the era/event named at
    /// index `i`. Distinct from `interner` (event variables) so the timeline
    /// namespace never collides with the support algebra's.
    pub time_points: Interner,
    /// The precedence chronology, populated during lowering from `before`
    /// facts and era-order chains; the time subsystem reasons over it.
    pub chronology: Chronology,
    /// The cyclic calendars.
    pub wheels: Vec<Wheel>,
}

impl World {
    /// An empty world.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Lower a document into an evaluable program: ground facts, option-gated
    /// facts, exclusivity families for questions and forks, and wheels. Event
    /// variables are interned as they are encountered.
    ///
    /// Derivation rules (`rule …`) are captured as expert-tier text clauses by
    /// the current surface, not as a typed head/body, so they are not lowered
    /// here; the engine's typed rule intake is [`World::define_rule`], and
    /// lowering structured rule bodies waits on the syntax lane exposing them.
    /// Other non-deductive declarations (sorts, registers, drafts, …) are
    /// consumed by other subsystems and carry no fact.
    pub fn lower(document: &Document) -> Result<World, EvalError> {
        let mut world = World::new();
        let mut proposal_seq: u32 = 0;
        for decl in &document.decls {
            match decl {
                Decl::Fact(fact) => {
                    let atom = ground_atom(&fact.atom)?;
                    let ann =
                        world.lower_fact_annotation(fact.provenance.as_ref(), &mut proposal_seq);
                    world.facts.push(Fact {
                        atom,
                        ann,
                        grade: Grade::Asserted,
                    });
                }
                Decl::Question(question) => world.lower_options(&question.options)?,
                Decl::Fork(fork) => world.lower_options(&fork.options)?,
                Decl::Wheel(wheel) => world.wheels.push(Wheel {
                    name: wheel.name.text.clone(),
                    spokes: wheel.spokes.iter().map(|s| s.text.clone()).collect(),
                }),
                Decl::Before(before) => {
                    let earlier = world.time_point(&before.earlier.text);
                    let later = world.time_point(&before.later.text);
                    world.chronology.edges.push((earlier, later));
                }
                Decl::Entity(entity) => {
                    if let Some(TimeClause::Order(chain)) = &entity.time {
                        world.add_order_chain(chain);
                    }
                }
                _ => {}
            }
        }
        Ok(world)
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
    pub fn define_rule(
        &mut self,
        name: impl Into<String>,
        head: AtomExpr,
        body: Vec<AtomExpr>,
    ) -> Result<(), EvalError> {
        let name = name.into();
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
    ) -> Ann {
        let Some(annotation) = annotation else {
            return Ann::supported(PosBool::FALSE);
        };
        let base = match annotation.marker {
            ProvenanceMarker::Proposed => {
                let name = format!("proposal#{proposal_seq}");
                *proposal_seq += 1;
                PosBool::pos(self.interner.intern(&name))
            }
            ProvenanceMarker::Decided
            | ProvenanceMarker::Ratified
            | ProvenanceMarker::Provisional => {
                if annotation.citations.is_empty() {
                    PosBool::FALSE
                } else {
                    let mut support = PosBool::truth();
                    for citation in &annotation.citations {
                        let var = self.interner.intern(&citation.id.text);
                        self.valuation.set(var, true);
                        support = support.and_(&PosBool::pos(var));
                    }
                    support
                }
            }
        };
        let support = match &annotation.condition {
            Some(condition) => base.and_(&lower_condition(condition, &mut self.interner)),
            None => base,
        };
        Ann::supported(support)
    }

    /// Partition the rule set's intensional predicates into evaluation strata,
    /// least-dependent first. Mutually recursive predicates share a stratum;
    /// with no negation in scope every program is stratifiable.
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
    pub fn saturate_with(&self, strata: &Strata) -> Result<World, EvalError> {
        let mut relation = self.seed_relation();
        for level in &strata.levels {
            let level_set: BTreeSet<&str> = level.iter().map(String::as_str).collect();
            let level_rules: Vec<&Rule> = self
                .rules
                .iter()
                .filter(|rule| level_set.contains(rule.head.pred.text.as_str()))
                .collect();
            saturate_stratum(&mut relation, &level_rules)?;
        }
        let mut world = self.clone();
        world.derived = relation.into_values().collect();
        world.derived.sort_by(|a, b| a.atom.cmp(&b.atom));
        world.support_index = build_support_index(&world.derived);
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
            let ann = Ann::new(derived.support.clone(), derived.defeaters.clone());
            let status = ann.status(valuation, &self.theory);
            if status == Status::Dead {
                continue;
            }
            rows.push(QueryRow {
                atom: derived.atom.render(),
                bindings,
                verdict: Verdict {
                    support: derived.support.clone(),
                    grade: derived.grade,
                },
                status,
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
            merge_into(
                &mut relation,
                fact.atom.clone(),
                &fact.ann.support,
                fact.grade,
                &fact.ann.defeaters,
            );
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
fn saturate_stratum(
    relation: &mut BTreeMap<GroundAtom, Derived>,
    rules: &[&Rule],
) -> Result<(), EvalError> {
    let mut worklist: VecDeque<GroundAtom> = relation.keys().cloned().collect();
    while let Some(delta) = worklist.pop_front() {
        let Some(delta_row) = relation.get(&delta).cloned() else {
            continue;
        };
        let productions = fire_rules(relation, rules, &delta, &delta_row)?;
        for (head, support, grade, defeaters) in productions {
            if let Some(head) = merge_into(relation, head, &support, grade, &defeaters) {
                worklist.push_back(head);
            }
        }
    }
    Ok(())
}

/// Fire every rule against a changed atom `delta`, joining the other body atoms
/// over the full relation, and collect the head atoms they produce.
type Production = (GroundAtom, PosBool, Grade, PosBool);

fn fire_rules(
    relation: &BTreeMap<GroundAtom, Derived>,
    rules: &[&Rule],
    delta: &GroundAtom,
    delta_row: &Derived,
) -> Result<Vec<Production>, EvalError> {
    let mut out = Vec::new();
    for rule in rules {
        for (i, body_atom) in rule.body.iter().enumerate() {
            if body_atom.pred.text != delta.pred || body_atom.args.len() != delta.args.len() {
                continue;
            }
            let mut bindings = BTreeMap::new();
            if !unify(body_atom, delta, &mut bindings) {
                continue;
            }
            let rest: Vec<&AtomExpr> = rule
                .body
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, atom)| atom)
                .collect();
            let solutions = join_body(
                relation,
                &rest,
                bindings,
                delta_row.support.clone(),
                delta_row.grade,
                delta_row.defeaters.clone(),
            );
            for (bindings, support, grade, defeaters) in solutions {
                let head = ground_head(&rule.head, &bindings)?;
                out.push((
                    head,
                    support.and_(&rule.ann.support),
                    Grade::weakest([grade, rule.grade]),
                    defeaters.or_(&rule.ann.defeaters),
                ));
            }
        }
    }
    Ok(out)
}

/// Recursively join the remaining body atoms over the relation, accumulating the
/// conjunction of their supports, the weakest-link grade, and their defeaters.
fn join_body(
    relation: &BTreeMap<GroundAtom, Derived>,
    remaining: &[&AtomExpr],
    bindings: BTreeMap<String, String>,
    support: PosBool,
    grade: Grade,
    defeaters: PosBool,
) -> Vec<(BTreeMap<String, String>, PosBool, Grade, PosBool)> {
    let Some((atom, rest)) = remaining.split_first() else {
        return vec![(bindings, support, grade, defeaters)];
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
                support.and_(&row.support),
                Grade::weakest([grade, row.grade]),
                defeaters.or_(&row.defeaters),
            ));
        }
    }
    out
}

/// Merge a derived support into the relation. Grade and defeaters fold on
/// *every* merge — a verdict must not depend on the order derivations arrive —
/// while the atom is re-enqueued only when its support strictly grew (support
/// is the termination measure).
fn merge_into(
    relation: &mut BTreeMap<GroundAtom, Derived>,
    atom: GroundAtom,
    support: &PosBool,
    grade: Grade,
    defeaters: &PosBool,
) -> Option<GroundAtom> {
    if let Some(row) = relation.get_mut(&atom) {
        // Fold grade and defeaters regardless of whether support grows, so a
        // stronger-grade derivation with dominated support still lifts the grade.
        row.grade = Grade::join_alt([row.grade, grade]);
        row.defeaters = row.defeaters.or_(defeaters);
        let merged = row.support.or_(support);
        if merged == row.support {
            return None;
        }
        row.support = merged;
        return Some(atom);
    }
    relation.insert(
        atom.clone(),
        Derived {
            atom: atom.clone(),
            support: support.clone(),
            grade,
            defeaters: defeaters.clone(),
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

/// Reject a rule whose head carries a variable absent from every body atom —
/// the range-restriction discipline, checked at load time.
fn check_range_restricted(name: &str, head: &AtomExpr, body: &[AtomExpr]) -> Result<(), EvalError> {
    let mut bound: BTreeSet<&str> = BTreeSet::new();
    for atom in body {
        for term in &atom.args {
            if let Term::Var(id) = term {
                bound.insert(id.text.as_str());
            }
        }
    }
    for term in &head.args {
        if let Term::Var(id) = term
            && !bound.contains(id.text.as_str())
        {
            return Err(EvalError::Unstratifiable {
                reason: format!(
                    "rule '{name}' is not range-restricted: head variable '{}' is not bound in the body",
                    id.text
                ),
            });
        }
    }
    Ok(())
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
            &PosBool::truth(),
            Grade::Asserted,
            &PosBool::FALSE,
        );
        merge_into(
            &mut asserted_first,
            atom.clone(),
            &PosBool::pos(Atom(0)),
            Grade::Checked,
            &PosBool::FALSE,
        );

        let mut checked_first = BTreeMap::new();
        merge_into(
            &mut checked_first,
            atom.clone(),
            &PosBool::pos(Atom(0)),
            Grade::Checked,
            &PosBool::FALSE,
        );
        merge_into(
            &mut checked_first,
            atom.clone(),
            &PosBool::truth(),
            Grade::Asserted,
            &PosBool::FALSE,
        );

        // Support is TRUE (absorbs {+0}); grade is the join (Checked) both ways.
        assert_eq!(asserted_first[&atom], checked_first[&atom]);
        assert!(asserted_first[&atom].support.is_true());
        assert_eq!(asserted_first[&atom].grade, Grade::Checked);
    }
}
