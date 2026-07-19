//! The loader front-end: source documents fronted as a compiled [`WorldModel`].
//!
//! [`Loader::load`] is a pure function from a set of parsed
//! [`Document`]s to a [`LoadOutcome`] — the compiled
//! world plus its load-time [`LoadFinding`]s. It never
//! touches the filesystem and never rules anything true (charter koan 3); it
//! records where every claim came from and gates each on the right event
//! variable, so a later evaluation can decide standing without re-deriving
//! provenance.
//!
//! # What the loader does at load time
//!
//! - **Merges packs.** Documents sharing a pack name accrue into one pack; a
//!   name declared twice in one pack, or across two packs, is a finding.
//! - **Attaches provenance and gates on status.** Every world-claim declaration
//!   (`entity`, slot, `fact`, `before`, `sort`) is compiled to an
//!   [`Ann`]: a `proposed` claim enters gated by an *open*
//!   hypothesis variable and is therefore structurally unassertable
//!   ([`Status::Possible`](dli_core::ann::Status), never `Certain`, until a
//!   ruling); a `decided`/`ratified`/`provisional` claim enters gated by its
//!   cited decision variable; a claim with no provenance, or one whose
//!   annotation cannot be compiled, is held **fail-closed** at `P⁺ = 0` and
//!   reported (spec 02 §§2–3). This is the compiled gate the context-pack
//!   builder relies on.
//! - **Builds the evaluable core.** Ground `fact`s become status-gated
//!   [`Fact`]s over the shared world interner; `before`-orderings and era-order
//!   chains populate the chronology and time-point table exactly as the
//!   evaluator's lowering does; wheels are carried through — so the
//!   [`WorldModel`]'s `world` is fixpoint-ready with its supports already gated
//!   and its timeline live.
//! - **Builds the entity and sort registry** and records what each pack
//!   contributed.
//!
//! # What the loader defers
//!
//! Two load-time checks named by spec 08 §1 — **range restriction** and
//! **negative-dependency stratification** — are properties of *structured*
//! rules. The AST deliberately captures a rule's body as normalized text
//! ([`Clause`](dli_ast::clause::Clause)); the evaluator owns the typed rule
//! intake ([`World::define_rule`](dli_eval::world::World::define_rule), which
//! range-restriction checks the structured head/body). The loader consumes the
//! `dli_eval::stratify` seam at load and surfaces a negative-dependency cycle as
//! an [`Unstratifiable`](crate::model::LoadFinding::Unstratifiable) finding —
//! `Ok` today over the loader's rule-less world, and it lights up the moment
//! structured rules reach the world. Range-restriction of authored text rules
//! stays with the evaluator's typed intake. Nothing is silently dropped (charter
//! koan 4).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
use dli_ast::decl::{
    AttrDecl, BeforeDecl, Decl, Document, EntityDecl, EntityKind, FactDecl, PackHeader, RuleDecl,
    SortDecl, TimeClause, WheelDecl,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term, TimeRef};
use dli_core::ann::Ann;
use dli_core::atom::Atom;
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_eval::EvalError;
use dli_eval::world::{Fact, GroundAtom};
use dli_time::chronology::TimePoint;
use dli_time::wheel::Wheel;

use crate::model::{
    EntityEntry, LoadFinding, LoadedAtom, PackProvenance, Provenance, SortEntry, WorldModel,
};

/// The outcome of loading a set of documents: the compiled world and the
/// findings raised while compiling it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LoadOutcome {
    /// The compiled world model. It carries its own findings too
    /// ([`WorldModel::findings`]); this field
    /// mirrors them for callers that want the report without the world.
    pub world: WorldModel,
    /// The load-time findings, in a deterministic order.
    pub findings: Vec<LoadFinding>,
}

/// The loader front-end — a namespace for [`load`](Loader::load).
#[derive(Debug, Clone, Copy)]
pub struct Loader;

impl Loader {
    /// Compile a set of parsed documents into a [`LoadOutcome`].
    ///
    /// Pure and total: every input is either compiled into the world or surfaced
    /// as a [`LoadFinding`]; the function never panics
    /// and never reads the filesystem.
    ///
    /// ```
    /// use dli_world::loader::{Loader, example_docs};
    /// let out = Loader::load(&example_docs());
    /// assert!(out.findings.is_empty());
    /// assert_eq!(out.world.entities().len(), 3);
    /// ```
    #[must_use]
    pub fn load(docs: &[Document]) -> LoadOutcome {
        let mut ctx = LoadCtx::default();
        for doc in docs {
            let pack = doc
                .pack
                .as_ref()
                .map_or_else(String::new, |header| header.name.text.clone());
            {
                let entry = ctx
                    .wm
                    .packs
                    .entry(pack.clone())
                    .or_insert_with(|| PackProvenance {
                        pack: pack.clone(),
                        documents: 0,
                        decls: 0,
                    });
                entry.documents += 1;
                entry.decls += doc.decls.len();
            }
            for decl in &doc.decls {
                if let Some((kind, name, span)) = decl_identity(decl) {
                    let key = (pack.clone(), kind.to_owned(), name.clone());
                    if !ctx.declared.insert(key) {
                        ctx.findings.push(LoadFinding::DuplicateDeclaration {
                            pack: pack.clone(),
                            kind: kind.to_owned(),
                            name: name.clone(),
                            span,
                        });
                    }
                    ctx.cross
                        .entry((kind.to_owned(), name))
                        .or_default()
                        .insert(pack.clone());
                }
                ctx.process_decl(decl, &pack);
            }
        }
        let mut outcome = ctx.finish();
        outcome.world.language = crate::language::LanguageModel::from_documents(docs).model;
        outcome
    }
}

/// The compiled provenance of one declaration: its annotation, how it entered,
/// and its rendered citations.
struct Compiled {
    ann: Ann,
    kind: Provenance,
    citations: Vec<String>,
}

impl Compiled {
    /// The fail-closed compilation: `P⁺ = 0`, no origin (spec 02 §3).
    fn fail_closed() -> Self {
        Self {
            ann: Ann::supported(PosBool::FALSE),
            kind: Provenance::Unprovenanced,
            citations: Vec::new(),
        }
    }
}

/// The mutable state threaded through one load.
#[derive(Debug, Default)]
struct LoadCtx {
    wm: WorldModel,
    findings: Vec<LoadFinding>,
    /// Observed entities: name to sort (declared kind), preferring `Some`.
    entities: BTreeMap<String, Option<String>>,
    /// `(pack, kind, name)` triples already declared — for duplicate detection.
    declared: BTreeSet<(String, String, String)>,
    /// `(kind, name)` to the packs that declared it — for collision detection.
    cross: BTreeMap<(String, String), BTreeSet<String>>,
    /// Monotonic counter for anonymous proposal bundles.
    bundle_counter: u32,
}

impl LoadCtx {
    /// Intern an event-variable name to its stable [`Atom`] in the shared world
    /// interner. Idempotent.
    fn intern(&mut self, name: &str) -> Atom {
        self.wm.world.interner.intern(name)
    }

    /// Intern an era/event name to a stable [`TimePoint`] in the time-point
    /// table (kept distinct from the event-variable interner). Idempotent.
    fn time_point(&mut self, name: &str) -> TimePoint {
        TimePoint(self.wm.world.time_points.intern(name).0)
    }

    /// Add the consecutive precedence edges of an era-order chain
    /// (`< a < b < c` becomes `a → b`, `b → c`).
    fn add_order_chain(&mut self, chain: &[Ident]) {
        for pair in chain.windows(2) {
            let earlier = self.time_point(&pair[0].text);
            let later = self.time_point(&pair[1].text);
            self.wm.world.chronology.edges.push((earlier, later));
        }
    }

    fn process_decl(&mut self, decl: &Decl, pack: &str) {
        match decl {
            Decl::Entity(entity) => self.process_entity(entity, pack),
            Decl::Attr(attr) => self.process_attr(attr, pack, None),
            Decl::Fact(fact) => self.process_fact(fact, pack),
            Decl::Before(before) => self.process_before(before, pack),
            Decl::Sort(sort) => self.process_sort(sort, pack),
            Decl::Rule(rule) => self.process_rule(rule, pack),
            Decl::Wheel(wheel) => self.process_wheel(wheel, pack),
            _ => {}
        }
    }

    fn process_entity(&mut self, entity: &EntityDecl, pack: &str) {
        // Lift an era-order chain (`< a < b < c`) onto the chronology exactly as
        // the evaluator does, so the compiled world's timeline is not inert
        // (defect F). Time points are interned in document order, matching
        // `World::lower`'s `TimePoint` assignment.
        if let Some(TimeClause::Order(chain)) = &entity.time {
            self.add_order_chain(chain);
        }
        let sort = entity.kind.keyword();
        let atom = AtomExpr {
            pred: Ident {
                text: sort.to_owned(),
                span: entity.name.span,
            },
            args: vec![Term::Sym(entity.name.clone())],
        };
        let label = format!("entity '{}'", entity.name.text);
        let compiled = self.compile_ann(
            entity.provenance.as_ref(),
            &label,
            pack,
            true,
            entity.name.span,
        );
        self.entities
            .entry(entity.name.text.clone())
            .and_modify(|slot| {
                if slot.is_none() {
                    *slot = Some(sort.to_owned());
                }
            })
            .or_insert_with(|| Some(sort.to_owned()));
        self.push_atom(atom, compiled, pack, entity.name.span);
        for slot in &entity.slots {
            self.process_attr(slot, pack, entity.provenance.as_ref());
        }
    }

    fn process_attr(&mut self, attr: &AttrDecl, pack: &str, inherit: Option<&Annotation>) {
        let atom = AtomExpr {
            pred: attr.attr.clone(),
            args: vec![Term::Sym(attr.subject.clone()), attr.value.clone()],
        };
        let provenance = attr.provenance.clone().or_else(|| inherit.cloned());
        let label = format!("slot '{}.{}'", attr.subject.text, attr.attr.text);
        let compiled = self.compile_ann(provenance.as_ref(), &label, pack, true, attr.subject.span);
        self.entities
            .entry(attr.subject.text.clone())
            .or_insert(None);
        if let Term::Sym(value) = &attr.value {
            self.entities.entry(value.text.clone()).or_insert(None);
        }
        self.push_atom(atom, compiled, pack, attr.subject.span);
    }

    fn process_fact(&mut self, fact: &FactDecl, pack: &str) {
        let label = format!("fact '{}'", fact.atom.pred.text);
        let compiled = self.compile_ann(
            fact.provenance.as_ref(),
            &label,
            pack,
            true,
            fact.atom.pred.span,
        );
        self.collect_entities(&fact.atom);
        // The evaluable core carries the *gated* support, so a fixpoint over the
        // world honours status gating (spec 02 §2).
        self.wm.world.facts.push(Fact {
            atom: ground_atom(&fact.atom),
            ann: compiled.ann.clone(),
            grade: Grade::Asserted,
        });
        self.push_atom(fact.atom.clone(), compiled, pack, fact.atom.pred.span);
    }

    fn process_before(&mut self, before: &BeforeDecl, pack: &str) {
        let atom = AtomExpr {
            pred: Ident {
                text: "before".to_owned(),
                span: before.earlier.span,
            },
            args: vec![
                Term::Sym(before.earlier.clone()),
                Term::Sym(before.later.clone()),
            ],
        };
        let label = format!(
            "ordering '{} -> {}'",
            before.earlier.text, before.later.text
        );
        let compiled = self.compile_ann(
            before.provenance.as_ref(),
            &label,
            pack,
            true,
            before.earlier.span,
        );
        self.collect_entities(&atom);
        // Record the precedence edge on the chronology (defect F): the compiled
        // world must answer `precedes`/`as_of`, not carry inert time.
        let earlier = self.time_point(&before.earlier.text);
        let later = self.time_point(&before.later.text);
        self.wm.world.chronology.edges.push((earlier, later));
        self.push_atom(atom, compiled, pack, before.earlier.span);
    }

    fn process_sort(&mut self, sort: &SortDecl, pack: &str) {
        let label = format!("sort '{}'", sort.name.text);
        let compiled =
            self.compile_ann(sort.provenance.as_ref(), &label, pack, true, sort.name.span);
        self.wm.sorts.push(SortEntry {
            name: sort.name.text.clone(),
            ann: compiled.ann,
            kind: compiled.kind,
            citations: compiled.citations,
            pack: pack.to_owned(),
        });
    }

    fn process_rule(&mut self, rule: &RuleDecl, pack: &str) {
        // A rule's provenance is checkable now; its typed head/body intake and
        // range restriction go through the evaluator once the syntax lane
        // structures the captured clause body (see the module docs).
        let label = format!("rule '{}'", rule.name.text);
        let _ = self.compile_ann(rule.provenance.as_ref(), &label, pack, true, rule.name.span);
    }

    fn process_wheel(&mut self, wheel: &WheelDecl, pack: &str) {
        self.wm.world.wheels.push(Wheel {
            name: wheel.name.text.clone(),
            spokes: wheel
                .spokes
                .iter()
                .map(|spoke| spoke.text.clone())
                .collect(),
        });
        if wheel.provenance.is_some() {
            let label = format!("wheel '{}'", wheel.name.text);
            let _ = self.compile_ann(
                wheel.provenance.as_ref(),
                &label,
                pack,
                false,
                wheel.name.span,
            );
        }
    }

    fn push_atom(&mut self, atom: AtomExpr, compiled: Compiled, pack: &str, span: Span) {
        self.wm.atoms.push(LoadedAtom {
            atom,
            ann: compiled.ann,
            kind: compiled.kind,
            citations: compiled.citations,
            pack: pack.to_owned(),
            span,
        });
    }

    fn collect_entities(&mut self, atom: &AtomExpr) {
        for arg in &atom.args {
            if let Term::Sym(id) = arg {
                self.entities.entry(id.text.clone()).or_insert(None);
            }
        }
    }

    /// Compile a declaration's provenance into a [`Compiled`] annotation, pushing
    /// any finding.
    ///
    /// `required` marks a world-claim declaration for which absent provenance is
    /// a claim-from-nowhere (charter koan 1). Every fail path returns
    /// [`Compiled::fail_closed`] (`P⁺ = 0`), the fail-closed theorem of spec
    /// 02 §3.
    fn compile_ann(
        &mut self,
        provenance: Option<&Annotation>,
        label: &str,
        pack: &str,
        required: bool,
        decl_span: Span,
    ) -> Compiled {
        let Some(annotation) = provenance else {
            if required {
                self.findings.push(LoadFinding::MissingProvenance {
                    decl: label.to_owned(),
                    pack: pack.to_owned(),
                    span: decl_span,
                });
            }
            return Compiled::fail_closed();
        };

        if annotation.marker.requires_citation() && annotation.citations.is_empty() {
            self.findings.push(LoadFinding::MalformedProvenance {
                decl: label.to_owned(),
                pack: pack.to_owned(),
                reason: format!(
                    "a `{}` claim needs a ruling to cite",
                    annotation.marker.keyword()
                ),
                span: annotation.span,
            });
            return Compiled::fail_closed();
        }

        let citations: Vec<String> = annotation
            .citations
            .iter()
            .map(|citation| citation.id.text.clone())
            .collect();

        let mut support = if annotation.marker == ProvenanceMarker::Proposed {
            // A proposal is gated by an open hypothesis. With an explicit
            // `assumes` condition the truth base lets the condition stand alone;
            // otherwise it enters under a fresh bundle variable.
            if annotation.condition.is_some() {
                PosBool::truth()
            } else {
                let key = if annotation.tags.is_empty() {
                    let key = format!("{pack}#{}", self.bundle_counter);
                    self.bundle_counter += 1;
                    key
                } else {
                    annotation.tags.join("+")
                };
                let var = self.intern(&format!("h.{key}"));
                PosBool::pos(var)
            }
        } else {
            // A ledger claim rests on ALL its cited decisions: superseding any
            // one of them degrades the claim (ruling A), so the citations conjoin
            // into a single monomial `{+D-1, …, +D-n}` — matching the evaluator's
            // lowering. This is why independent bases are separate declarations.
            let mut acc = PosBool::truth();
            for citation in &annotation.citations {
                let var = self.intern(citation.id.text.trim());
                acc = acc.and_(&PosBool::pos(var));
            }
            acc
        };

        if let Some(condition) = &annotation.condition {
            match self.cond_to_posbool(condition, false) {
                Ok(cond) => support = support.and_(&cond),
                Err(reason) => {
                    self.findings.push(LoadFinding::MalformedProvenance {
                        decl: label.to_owned(),
                        pack: pack.to_owned(),
                        reason,
                        span: annotation.span,
                    });
                    return Compiled::fail_closed();
                }
            }
        }

        if support.is_false() {
            self.findings.push(LoadFinding::MalformedProvenance {
                decl: label.to_owned(),
                pack: pack.to_owned(),
                reason: "the condition can never hold".to_owned(),
                span: annotation.span,
            });
            return Compiled::fail_closed();
        }

        Compiled {
            ann: Ann::supported(support),
            kind: Provenance::from_marker(annotation.marker),
            citations,
        }
    }

    /// Compile a hypothesis condition into a support polynomial, interning each
    /// named hypothesis. Negation is pushed to the literals via De Morgan
    /// (`negated` tracks the running polarity), so the result stays a positive
    /// formula over signed literals; the real [`PosBool`] algebra absorbs it.
    fn cond_to_posbool(&mut self, cond: &CondExpr, negated: bool) -> Result<PosBool, String> {
        match cond {
            CondExpr::Hyp(id) => {
                let text = id.text.trim();
                if text.is_empty() {
                    return Err("a hypothesis in the condition has no name".to_owned());
                }
                let atom = self.intern(text);
                Ok(if negated {
                    PosBool::neg(atom)
                } else {
                    PosBool::pos(atom)
                })
            }
            CondExpr::Not(inner) => self.cond_to_posbool(inner, !negated),
            CondExpr::And(list) => self.cond_group(list, negated, negated),
            CondExpr::Or(list) => self.cond_group(list, negated, !negated),
        }
    }

    /// Compile an `and`/`or` group. `effective_or` is the connective *after* De
    /// Morgan: an `and` under negation behaves as an `or` and vice versa.
    fn cond_group(
        &mut self,
        list: &[CondExpr],
        negated: bool,
        effective_or: bool,
    ) -> Result<PosBool, String> {
        if list.is_empty() {
            return Err("a condition group has no members".to_owned());
        }
        let mut children = Vec::with_capacity(list.len());
        for member in list {
            children.push(self.cond_to_posbool(member, negated)?);
        }
        let mut acc = if effective_or {
            PosBool::FALSE
        } else {
            PosBool::truth()
        };
        for child in &children {
            acc = if effective_or {
                acc.or_(child)
            } else {
                acc.and_(child)
            };
        }
        Ok(acc)
    }

    fn finish(mut self) -> LoadOutcome {
        let mut collisions = Vec::new();
        for ((kind, name), packs) in &self.cross {
            if packs.len() > 1 {
                collisions.push(LoadFinding::CrossPackCollision {
                    kind: kind.clone(),
                    name: name.clone(),
                    packs: packs.iter().cloned().collect(),
                });
            }
        }
        self.findings.extend(collisions);

        // The load-time stratification handoff: consume the evaluator seam and
        // surface a negative-dependency cycle as a finding (spec 08 §1). The
        // loader lowers no typed rules today (rule bodies are captured text), so
        // this is `Ok` over an empty rule set; it lights up the moment structured
        // rules reach the world through `World::define_rule`.
        if let Err(EvalError::Unstratifiable { reason }) = dli_eval::stratify(&self.wm.world) {
            self.findings.push(LoadFinding::Unstratifiable { reason });
        }

        self.wm.entities = self
            .entities
            .into_iter()
            .map(|(name, sort)| EntityEntry { name, sort })
            .collect();
        self.wm.findings = self.findings.clone();
        LoadOutcome {
            findings: self.findings,
            world: self.wm,
        }
    }
}

/// Render a term to its ground string form (a variable renders as its name).
fn render_term(term: &Term) -> String {
    match term {
        Term::Var(id) | Term::Sym(id) => id.text.clone(),
        Term::Str(text) => text.clone(),
        Term::Int(value) => value.to_string(),
        Term::Time(TimeRef::Era(id)) => format!("era:{}", id.text),
        Term::Time(TimeRef::Event(id)) => format!("event:{}", id.text),
    }
}

/// Lift a fact's atom into the evaluator's ground-atom identity.
fn ground_atom(atom: &AtomExpr) -> GroundAtom {
    GroundAtom {
        pred: atom.pred.text.clone(),
        args: atom.args.iter().map(render_term).collect(),
    }
}

/// The `(kind, name, span)` identity of a *named* declaration, or `None` for the
/// declarations whose identity is not a single name (facts, slots, orderings).
fn decl_identity(decl: &Decl) -> Option<(&'static str, String, Span)> {
    match decl {
        Decl::Entity(e) => Some(("entity", e.name.text.clone(), e.name.span)),
        Decl::Sort(s) => Some(("sort", s.name.text.clone(), s.name.span)),
        Decl::Question(q) => Some(("question", q.name.text.clone(), q.name.span)),
        Decl::Rule(r) => Some(("rule", r.name.text.clone(), r.name.span)),
        Decl::Fluent(f) => Some(("fluent", f.name.text.clone(), f.name.span)),
        Decl::Wheel(w) => Some(("wheel", w.name.text.clone(), w.name.span)),
        Decl::Register(r) => Some(("register", r.name.text.clone(), r.name.span)),
        Decl::Lexeme(l) => Some(("lexeme", l.name.text.clone(), l.name.span)),
        Decl::Morpheme(m) => Some(("morpheme", m.name.text.clone(), m.name.span)),
        Decl::Craft(c) => Some(("craft", c.name.text.clone(), c.name.span)),
        Decl::Practice(p) => Some(("practice", p.name.text.clone(), p.name.span)),
        Decl::Fork(f) => Some(("fork", f.question.text.clone(), f.question.span)),
        Decl::Name(n) => Some(("name", n.name.text.clone(), n.name.span)),
        Decl::Term(t) => Some(("term", t.name.text.clone(), t.name.span)),
        Decl::Principle(p) => Some(("principle", p.name.text.clone(), p.name.span)),
        Decl::Hypothesis(h) => Some(("hypothesis", h.name.text.clone(), h.name.span)),
        Decl::OrderRule(o) => Some(("order-rule", o.name.text.clone(), o.name.span)),
        Decl::Ledger(l) => Some(("ledger", l.name.text.clone(), l.name.span)),
        _ => None,
    }
}

/// A hand-built document set used by the crate's doctests and tests: the
/// saltmarket pack — a sort, three entities, and two facts, all with well-formed
/// provenance, so it loads with zero findings.
#[doc(hidden)]
#[must_use]
pub fn example_docs() -> Vec<Document> {
    fn marked(marker: ProvenanceMarker, id: Option<&str>) -> Annotation {
        Annotation {
            marker,
            citations: id
                .map(|id| {
                    vec![CitationRef {
                        id: Ident::synthetic(id),
                    }]
                })
                .unwrap_or_default(),
            condition: None,
            tags: Vec::new(),
            span: Span::synthetic(),
        }
    }
    fn ratified(id: &str) -> Annotation {
        marked(ProvenanceMarker::Ratified, Some(id))
    }
    fn place(name: &str) -> Decl {
        Decl::Entity(EntityDecl {
            kind: EntityKind::Place,
            name: Ident::synthetic(name),
            label: None,
            time: None,
            slots: Vec::new(),
            provenance: Some(ratified("D-0001")),
        })
    }
    fn fact(pred: &str, args: &[&str], provenance: Option<Annotation>, triple: bool) -> Decl {
        Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred: Ident::synthetic(pred),
                args: args
                    .iter()
                    .map(|a| Term::Sym(Ident::synthetic(*a)))
                    .collect(),
            },
            triple,
            provenance,
        })
    }

    vec![Document {
        pack: Some(PackHeader {
            name: Ident::synthetic("saltmarket"),
            span: Span::synthetic(),
        }),
        decls: vec![
            Decl::Sort(SortDecl {
                name: Ident::synthetic("place"),
                provenance: Some(ratified("D-0001")),
            }),
            place("saltmarket"),
            place("the-shallows"),
            Decl::Entity(EntityDecl {
                kind: EntityKind::Thing,
                name: Ident::synthetic("tide-bell"),
                label: None,
                time: None,
                slots: Vec::new(),
                provenance: Some(marked(ProvenanceMarker::Decided, Some("D-0002"))),
            }),
            fact(
                "adjacent",
                &["saltmarket", "the-shallows"],
                Some(ratified("D-0001")),
                false,
            ),
            fact(
                "rings",
                &["tide-bell", "saltmarket"],
                Some(marked(ProvenanceMarker::Proposed, None)),
                true,
            ),
        ],
    }]
}

#[cfg(test)]
mod tests {
    use super::LoadCtx;

    use dli_ast::annot::CondExpr;
    use dli_ast::ident::Ident;

    fn hyp(name: &str) -> CondExpr {
        CondExpr::Hyp(Ident::synthetic(name))
    }

    #[test]
    fn condition_and_distributes_into_one_monomial() {
        let mut ctx = LoadCtx::default();
        let cond = CondExpr::And(vec![hyp("q-0001.guild"), hyp("q-0002.tide")]);
        let pos = ctx.cond_to_posbool(&cond, false).expect("well-formed");
        assert_eq!(pos.monomials().count(), 1, "an AND is a single conjunction");
    }

    #[test]
    fn condition_or_yields_two_monomials() {
        let mut ctx = LoadCtx::default();
        let cond = CondExpr::Or(vec![hyp("a"), hyp("b")]);
        let pos = ctx.cond_to_posbool(&cond, false).expect("well-formed");
        assert_eq!(pos.monomials().count(), 2);
    }

    #[test]
    fn de_morgan_pushes_negation_to_literals() {
        // not(a and b) == (not a) or (not b): two single-literal monomials.
        let mut ctx = LoadCtx::default();
        let cond = CondExpr::Not(Box::new(CondExpr::And(vec![hyp("a"), hyp("b")])));
        let pos = ctx.cond_to_posbool(&cond, false).expect("well-formed");
        assert_eq!(pos.monomials().count(), 2);
    }

    #[test]
    fn empty_hypothesis_name_is_malformed() {
        let mut ctx = LoadCtx::default();
        assert!(ctx.cond_to_posbool(&hyp("   "), false).is_err());
    }

    #[test]
    fn contradictory_condition_absorbs_to_false() {
        // a and not a  ->  no satisfiable monomial
        let mut ctx = LoadCtx::default();
        let cond = CondExpr::And(vec![hyp("a"), CondExpr::Not(Box::new(hyp("a")))]);
        let pos = ctx.cond_to_posbool(&cond, false).expect("compiles");
        assert!(pos.is_false());
    }
}
