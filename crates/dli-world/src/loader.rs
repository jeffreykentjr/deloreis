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
//! # Rules: the load-time checks of spec 08 §1
//!
//! A **bare** world `rule` is a derivation rule. Its `head :- body` clause is
//! pre-parsed by the front end into typed literals (LS-9:
//! [`RuleClause::datalog`](dli_ast::rule::RuleClause::datalog); a body literal
//! prefixed by `not` is negated). The loader is a *reader* of that typed AST —
//! it no longer re-enters `dli_syntax` — and routes each clause through the
//! evaluator's negation-aware intake
//! ([`World::define_rule_annotated`](dli_eval::world::World::define_rule_annotated),
//! carrying the rule's own compiled provenance so it is a defeasible claim like
//! any other — WS-2), populating `world.rules`. Three load-time checks fire
//! from there:
//!
//! - **Rule shape** — every bare top-level rule must parse as Datalog; a
//!   malformed clause or body-less declaration is a
//!   [`MalformedWorldRule`](crate::model::LoadFinding::MalformedWorldRule)
//!   finding rather than an empty lowering.
//! - **Range restriction** — every variable in the head or in a negated literal
//!   must appear in a positive body atom — is checked as each rule is defined; a
//!   violation is an [`UnboundHeadVariable`](crate::model::LoadFinding::UnboundHeadVariable)
//!   finding and the rule is dropped.
//! - **Negative-dependency stratification** is checked once over the whole rule
//!   set via the `dli_eval::stratify` seam in `finish`; a
//!   negative cycle (`p :- not q`, `q :- not p`) is an
//!   [`Unstratifiable`](crate::model::LoadFinding::Unstratifiable) finding.
//!
//! `rule … over order` and `rule … over season` are checker rules for other
//! engines and are not lowered here. Nothing is silently dropped (charter
//! koan 4).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
use dli_ast::decl::{
    AttrDecl, BeforeDecl, CycleDecl, Decl, Document, EntityDecl, EntityKind, FactDecl,
    FunctionalDecl, InertialDecl, KindDecl, OptionClause, PredicateSameAsDecl, RuleDecl,
    SameAsDecl, ScopeHeader, StateDecl, TimeClause,
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
use dli_time::cycle::Cycle;
use dli_time::fluent::Span as FluentSpan;

use crate::model::{
    EntityEntry, LoadFinding, LoadedAtom, PackProvenance, Provenance, SortEntry, WorldModel,
};
use crate::predicate_identity::PredicateIdentityLink;

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
        let mut load = IncrementalLoad::new();
        load.extend(docs);
        load.finish()
    }
}

/// A load **in progress**: every document lowered declaration-by-declaration,
/// with the whole-pack aggregation ([`finish`](IncrementalLoad::finish)) not yet
/// applied.
///
/// [`Loader::load`] is this type used once — `new`, `extend`, `finish` — so the
/// two paths cannot drift: there is only one per-declaration lowering and only
/// one aggregation, and successive extensions run the identical operations in
/// the identical order over a longer document run. What the
/// split buys is that the *lowering* half is append-only and therefore reusable:
/// `extend` may be called again with more documents and only those documents are
/// lowered, while everything the earlier documents produced — the atoms, sorts,
/// facts, rules, chronology edges, fluent spans, the shared interner and
/// time-point table, the pack provenance, the duplicate/cross-pack bookkeeping,
/// the temporal references still to resolve, the anonymous-proposal bundle
/// counter, and the language layer's own compiler state — is carried forward
/// verbatim.
///
/// [`finish`](IncrementalLoad::finish) does **not** consume the load: the
/// aggregation runs against a snapshot, so the pre-aggregation state survives for
/// the next `extend`. That is what makes the aggregation's whole-pack steps
/// (cross-pack collision, `synthesize_alias_rules`, stratification) recompute
/// from the same starting point a full load would reach, which is what byte-
/// identity rests on.
#[derive(Debug, Clone, Default)]
pub struct IncrementalLoad {
    ctx: LoadCtx,
    language: crate::language::dialect::Compiler,
}

impl IncrementalLoad {
    /// A load with nothing lowered yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Lower a further run of documents onto this load, append-only.
    ///
    /// Every table this touches grows at the end and nothing is renumbered — most
    /// of all the shared interner, whose indices every already-compiled support
    /// polynomial points into.
    pub fn extend(&mut self, docs: &[Document]) {
        for doc in docs {
            let pack = doc
                .scope
                .as_ref()
                .map_or_else(String::new, |header| header.name.text.clone());
            {
                let entry =
                    self.ctx
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
                    if !self.ctx.declared.insert(key) {
                        self.ctx.findings.push(LoadFinding::DuplicateDeclaration {
                            pack: pack.clone(),
                            kind: kind.to_owned(),
                            name: name.clone(),
                            span,
                        });
                    }
                    self.ctx
                        .cross
                        .entry((kind.to_owned(), name))
                        .or_default()
                        .insert(pack.clone());
                }
                self.ctx.process_decl(decl, &pack);
                // The language layer compiles the same declaration stream into
                // its own registry. Interleaving it here rather than running a
                // second pass over the documents (as `Loader::load` once did)
                // is order-identical — the two compilers share no state — and it
                // is what lets the language half be resumed too.
                self.language.process(decl, &pack);
            }
        }
    }

    /// Run the whole-pack aggregation over everything lowered so far and return
    /// the [`LoadOutcome`], leaving this load extendable.
    ///
    /// The aggregation is a snapshot: it clones the in-progress state, so the
    /// steps that are whole-pack by nature — temporal-reference resolution,
    /// cross-pack collision, identity-closure synthesis, stratification, entity
    /// materialization — always run over the *complete* document set from an
    /// unsynthesized starting point, exactly as a single full load would.
    #[must_use]
    pub fn finish(&self) -> LoadOutcome {
        let mut outcome = self.ctx.clone().finish();
        outcome.world.language = self.language.clone().finish().model;
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
#[derive(Debug, Clone, Default)]
struct LoadCtx {
    wm: WorldModel,
    findings: Vec<LoadFinding>,
    /// Observed entities: name to sort (declared kind), preferring `Some`.
    entities: BTreeMap<String, Option<String>>,
    /// `(pack, kind, name)` triples already declared — for duplicate detection.
    declared: BTreeSet<(String, String, String)>,
    /// `(kind, name)` to the packs that declared it — for collision detection.
    cross: BTreeMap<(String, String), BTreeSet<String>>,
    /// Typed era/event declarations, including every member of an era-order
    /// chain. Kept separate from observed symbols because temporal clauses are
    /// closed, typed references while ordinary facts may name abstract subjects.
    temporal_decls: BTreeMap<String, String>,
    /// Temporal references are checked after all documents load so forward and
    /// cross-pack references remain legal.
    temporal_refs: Vec<TemporalReference>,
    /// Monotonic counter for anonymous proposal bundles.
    bundle_counter: u32,
}

#[derive(Debug, Clone)]
struct TemporalReference {
    decl: String,
    expected_kind: String,
    target: String,
    pack: String,
    span: Span,
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
    /// (`a < b < c` becomes `a → b`, `b → c`), each blamed on the era
    /// declaration's ruling `ann`. The chain passed here is the *full* order,
    /// head included (see [`process_entity`](LoadCtx::process_entity)).
    fn add_order_chain(&mut self, chain: &[Ident], ann: &Ann) {
        for pair in chain.windows(2) {
            let earlier = self.time_point(&pair[0].text);
            let later = self.time_point(&pair[1].text);
            self.wm.world.chronology.edges.push((earlier, later));
            self.wm.chronology_anns.push(ann.clone());
        }
    }

    fn process_decl(&mut self, decl: &Decl, pack: &str) {
        match decl {
            Decl::Entity(entity) => self.process_entity(entity, pack),
            Decl::Attr(attr) => self.process_attr(attr, pack, None),
            Decl::State(state) => self.process_state(state, pack),
            Decl::Functional(functional) => self.process_functional(functional),
            Decl::Inertial(inertial) => self.process_inertial(inertial, pack),
            Decl::Fact(fact) => self.process_fact(fact, pack),
            Decl::Before(before) => self.process_before(before, pack),
            Decl::SameAs(same_as) => self.process_same_as(same_as, pack),
            Decl::PredicateSameAs(predicate_same_as) => {
                self.process_predicate_same_as(predicate_same_as, pack);
            }
            Decl::Kind(kind) => self.process_kind(kind, pack),
            Decl::Rule(rule) => self.process_rule(rule, pack),
            Decl::Cycle(cycle) => self.process_cycle(cycle, pack),
            // Questions and forks lower their options; a hypothesis interns its
            // variable so a fork can `suppose` it. Matches `World::lower`.
            Decl::Question(question) => self.process_options(&question.options, pack),
            Decl::Fork(fork) => self.process_options(&fork.options, pack),
            Decl::Hypothesis(hypothesis) => {
                self.intern(&hypothesis.name.text);
            }
            // No world-model atom here, each for a different reason:
            // - Dialect/Word/NamePart/Name/Term/Banned are compiled by the
            //   LanguageModel pass over the raw documents
            //   (`LanguageModel::from_documents`, `Loader::load` above,
            //   :159), not by this per-decl dispatch.
            // - `ratify` is consumed by `ledger_valuation`/
            //   `declared_decisions` (dli-world::lib.rs), which turns a
            //   corpus's `ratify` decls into the authoritative decision set
            //   a citation must land in.
            // - Practice/Proof/Principle are read by dli-ledger
            //   (practice.rs, ledger_check.rs), not the world model:
            //   resource-tracked practices, casting proofs, and craft
            //   principles lower to no ground atom here.
            // - `attest` carries no atom here either, but it is NOT inert:
            //   `author.add_attest` (contract 1.3.0) is a live, materializable
            //   wire capability, and the ledger's attestation-sweep machinery
            //   (`dli.attest.missing` / `dli.attestation-sweep`) reads it
            //   downstream of the store, not through this per-decl dispatch.
            //   `load.inert-declaration` is error-severity and blocks
            //   `author.materialize` (verified: flagging Attest here breaks
            //   the shipped `add_attest` -> materialize flow), so it stays a
            //   plain no-op here.
            // - Use/Draft/Export/Variant/Narration are carried to the
            //   lorestore but carry no evaluable atom here either. Whether
            //   these deprecated-only forms should raise
            //   `load.inert-declaration` (koan 4 / DL-0029) is a per-form
            //   disposition DEFERRED to the deprecation audit (PR-FINAL):
            //   flagging them error-severity regresses the shipped
            //   round-table/camelot reference corpus (`use language`,
            //   `use chronology`, `narration`), so this stays behavior-neutral
            //   and the audit rules per form + fixes the corpus if a form
            //   turns out to be a genuine defect.
            Decl::Dialect(_)
            | Decl::Word(_)
            | Decl::NamePart(_)
            | Decl::Name(_)
            | Decl::Term(_)
            | Decl::Banned(_)
            | Decl::Ratify(_)
            | Decl::Practice(_)
            | Decl::Proof(_)
            | Decl::Principle(_)
            | Decl::Attest(_)
            | Decl::Use(_)
            | Decl::Draft(_)
            | Decl::Export(_)
            | Decl::Variant(_)
            | Decl::Narration(_) => {}
        }
    }

    /// Lower a run of question/fork options: intern each option name as an event
    /// variable, register the exclusivity family, and lower each `implies` atom
    /// as an option-gated fact — exactly as `World::lower` does, so a query sees
    /// the implied predicate and a fork can `suppose` the option variable.
    fn process_options(&mut self, options: &[OptionClause], pack: &str) {
        let mut family = Vec::with_capacity(options.len());
        for option in options {
            let var = self.intern(&option.name.text);
            family.push(var);
            if let Some(implied) = &option.implies {
                self.observe_predicate_arity(
                    &implied.pred.text,
                    implied.args.len(),
                    pack,
                    implied.pred.span,
                );
                let ann = Ann::supported(PosBool::pos(var));
                self.wm.world.facts.push(Fact {
                    atom: ground_atom(implied),
                    ann: ann.clone(),
                    grade: Grade::Asserted,
                });
                self.collect_entities(implied);
                let compiled = Compiled {
                    ann,
                    kind: Provenance::Proposed,
                    citations: Vec::new(),
                };
                self.push_atom(implied.clone(), compiled, pack, implied.pred.span);
            }
        }
        self.wm.world.theory.add_exclusive(family);
    }

    fn process_entity(&mut self, entity: &EntityDecl, pack: &str) {
        let sort = entity.kind.keyword();
        if matches!(entity.kind, EntityKind::Era | EntityKind::Event) {
            self.temporal_decls
                .insert(entity.name.text.clone(), sort.to_owned());
        }
        match &entity.time {
            Some(TimeClause::After(target) | TimeClause::During(target)) => {
                self.temporal_refs.push(TemporalReference {
                    decl: format!("{sort} '{}'", entity.name.text),
                    expected_kind: "era".to_owned(),
                    target: target.text.clone(),
                    pack: pack.to_owned(),
                    span: target.span,
                });
            }
            Some(TimeClause::Since(target) | TimeClause::Until(target)) => {
                self.temporal_refs.push(TemporalReference {
                    decl: format!("{sort} '{}'", entity.name.text),
                    expected_kind: "event".to_owned(),
                    target: target.text.clone(),
                    pack: pack.to_owned(),
                    span: target.span,
                });
            }
            Some(TimeClause::Order(chain)) => {
                for member in chain {
                    self.temporal_decls
                        .insert(member.text.clone(), "era".to_owned());
                }
            }
            None => {}
        }
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
        if entity.kind == EntityKind::Event
            && let Some(TimeClause::During(era)) = &entity.time
        {
            let event = self.time_point(&entity.name.text);
            let era = self.time_point(&era.text);
            self.wm.era_memberships.push(crate::model::EraMembership {
                event,
                era,
                ann: compiled.ann.clone(),
            });
        }
        // Lift the era-order chain onto the chronology exactly as the evaluator
        // does (defect F), INCLUDING its declared head. The parser hands us the
        // run *after* the entity name (`era a < b < c` → name `a`, chain
        // `[b, c]`), so the full order is `[name] ++ chain`; dropping the head
        // silently loses the `a → b` edge — and hides an `a < b < a` cycle (H8).
        // Each edge is blamed on this declaration's ruling. Time points are
        // interned in document order, matching `World::lower`'s assignment.
        if let Some(TimeClause::Order(chain)) = &entity.time {
            let ordered: Vec<Ident> = std::iter::once(entity.name.clone())
                .chain(chain.iter().cloned())
                .collect();
            self.add_order_chain(&ordered, &compiled.ann);
        }
        // Intern every era/event point at its declaration (not only at edge
        // endpoints), so a point in no `before`-fact or order chain still reaches
        // the timeline — e.g. `timeline show` must list a lone era.
        if matches!(entity.kind, EntityKind::Era | EntityKind::Event) {
            self.time_point(&entity.name.text);
        }
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
        self.observe_predicate_arity(&atom.pred.text, atom.args.len(), pack, atom.pred.span);
        let provenance = attr.provenance.clone().or_else(|| inherit.cloned());
        let label = format!("slot '{}.{}'", attr.subject.text, attr.attr.text);
        let compiled = self.compile_ann(provenance.as_ref(), &label, pack, true, attr.subject.span);
        self.entities
            .entry(attr.subject.text.clone())
            .or_insert(None);
        if let Term::Sym(value) = &attr.value {
            self.entities.entry(value.text.clone()).or_insert(None);
        }
        self.wm.world.facts.push(Fact {
            atom: ground_atom(&atom),
            ann: compiled.ann.clone(),
            grade: Grade::Asserted,
        });
        self.push_atom(atom, compiled, pack, attr.subject.span);
    }

    /// Lower a `state <name> [= <value>] [time]` fluent (issues 47/48).
    ///
    /// A state is realized exactly as an attribute is — a gated, queryable ground
    /// fact via [`StateDecl::atom`] and the same provenance seam — *and*, when it
    /// has a value on a dotted subject, a [`Span`] on the slot's
    /// [`FluentTimeline`](dli_time::fluent::FluentTimeline). The span carries the
    /// interval (`since`/`until`) and the ruling, so two values of the same slot
    /// over disjoint spans read as one fluent rather than a contradiction; the
    /// timeless fact stays queryable. Nothing is silently dropped (koan 4): a
    /// declaration naming nothing evaluable surfaces an [`InertDeclaration`]
    /// finding, and an undeclared time anchor a [`TemporalReference`] one.
    ///
    /// [`InertDeclaration`]: crate::model::LoadFinding::InertDeclaration
    fn process_state(&mut self, state: &StateDecl, pack: &str) {
        let label = format!("state '{}'", state.name.text);
        let Some(atom) = state.atom() else {
            self.findings.push(LoadFinding::InertDeclaration {
                decl: label,
                pack: pack.to_owned(),
                span: state.name.span,
            });
            return;
        };
        let compiled = self.compile_ann(
            state.provenance.as_ref(),
            &label,
            pack,
            true,
            state.name.span,
        );
        self.observe_predicate_arity(&atom.pred.text, atom.args.len(), pack, atom.pred.span);
        self.collect_entities(&atom);
        // Thread the temporal clause so an undeclared anchor surfaces, exactly as
        // an entity's does: `since`/`until` name events, `after`/`during` eras.
        self.record_state_time(state, &label, pack);
        // The evaluable core carries the gated fact — a state is queryable like
        // any slot (issue 47).
        let fact_index = self.wm.world.facts.len();
        self.wm.world.facts.push(Fact {
            atom: ground_atom(&atom),
            ann: compiled.ann.clone(),
            grade: Grade::Asserted,
        });
        // A valued binary slot also bears a fluent span, so the check path reads
        // it over its interval rather than as a timeless clash (issue 48).
        if let [Term::Sym(subject), value] = atom.args.as_slice() {
            let slot = (atom.pred.text.clone(), subject.text.clone());
            let (since, until) = self.state_bounds(state.time.as_ref());
            let span = FluentSpan::new(render_term(value), since, until, compiled.ann.clone());
            self.wm
                .fluent_fact_indices
                .entry(slot.clone())
                .or_default()
                .push(fact_index);
            self.wm.fluents.entry(slot).or_default().push(span);
        } else {
            let (since, until) = self.state_bounds(state.time.as_ref());
            self.wm.standalone_temporal_facts.push((
                fact_index,
                FluentSpan::new(atom.pred.text.clone(), since, until, compiled.ann.clone()),
            ));
        }
        self.wm
            .temporal_atom_facts
            .insert(self.wm.atoms.len(), fact_index);
        self.push_atom(atom, compiled, pack, state.name.span);
    }

    /// Record a `functional <pred>` declaration: the predicate is single-valued,
    /// so the FD/contradiction check treats a slot on it as a functional
    /// dependency even when canon bears no prior fact for it (issue 49). This is a
    /// carriage-only declaration — it lowers to no atom and interns no provenance
    /// (its provenance rides the durable store's serialized body, keeping the
    /// three lowerings byte-identical, DL-0071).
    ///
    /// The predicate name is recorded in **two** places, both read-only sets that
    /// carry no atom and shift no interner: the world-model set the load-side
    /// checker reads, and the evaluable `World`'s own set the fork engine reads.
    /// The fork base is `WorldModel::world`, so seeding it here is what lets a
    /// declared functional dependency reach the in-memory merge (DL-0075) — the
    /// loader builds the eval world directly rather than through `World::lower`.
    fn process_functional(&mut self, functional: &FunctionalDecl) {
        self.wm
            .functional_predicates
            .insert(functional.pred.text.clone());
        self.wm
            .world
            .functional_predicates
            .insert(functional.pred.text.clone());
    }

    /// Lower the structured predicate-identity carriage.  It is deliberately
    /// not an ordinary atom: the evaluator's separate declaration channel is
    /// the only input that can merge predicate names.
    fn process_predicate_same_as(&mut self, identity: &PredicateSameAsDecl, pack: &str) {
        let label = format!(
            "predicate identity '{} = {}'",
            identity.left.text, identity.right.text
        );
        let compiled = self.compile_ann(
            identity.provenance.as_ref(),
            &label,
            pack,
            true,
            identity.left.span,
        );
        let link = PredicateIdentityLink::new(
            identity.left.text.clone(),
            identity.right.text.clone(),
            compiled.ann.clone(),
        )
        .with_citations(compiled.citations.clone());
        if let Some(mismatch) = self.wm.predicate_identity.add_link(link) {
            self.record_predicate_arity_mismatch(mismatch, pack, identity.left.span);
        }
        self.wm.world.assert_predicate_same_as(
            identity.left.text.clone(),
            identity.right.text.clone(),
            compiled.ann,
        );
    }

    /// Record one observed predicate arity and retain only the first finding
    /// for an identical affected component.  The state cache itself is
    /// component-local: unrelated predicate mutations do not rescan this
    /// component or any other component.
    fn observe_predicate_arity(&mut self, predicate: &str, arity: usize, pack: &str, span: Span) {
        if let Some(mismatch) = self
            .wm
            .predicate_identity
            .observe_arity(predicate.to_owned(), arity)
        {
            self.record_predicate_arity_mismatch(mismatch, pack, span);
        }
    }

    fn record_predicate_arity_mismatch(
        &mut self,
        mismatch: crate::predicate_identity::PredicateArityMismatch,
        pack: &str,
        span: Span,
    ) {
        let finding = LoadFinding::PredicateIdentityArityMismatch {
            touched_predicate: mismatch.touched_predicate,
            predicates: mismatch.predicates,
            arities: mismatch.arities,
            pack: pack.to_owned(),
            span,
        };
        if !self.findings.contains(&finding) {
            self.findings.push(finding);
        }
    }

    /// Record an `inertial <pred>` declaration (`docs/decisions.md` DL-0081).
    /// Carriage-only — it mints no ground atom — but, unlike `functional`, its
    /// provenance is **compiled into an `Ann`** and OR-ed into the predicate's
    /// entry, because the continuity check caps only while the declaration is
    /// certain (defeating it reverts the slot to plain projection). Both the
    /// world-model map (the load check reads it) and the evaluable `World`'s own
    /// map (carried through forks/merges) are seeded, mirroring
    /// [`process_functional`](LoadCtx::process_functional)'s two-place seeding.
    ///
    /// Interning the provenance here is what keeps the load path in lock-step
    /// with the lorestore walk (`dli_lorestore::lower::decl_provenance` routes
    /// `Decl::Inertial` through its mirror `compile_ann`): both intern the same
    /// variables in the same order, so `import_pack` never sees a
    /// `LoweringDiverged`.
    fn process_inertial(&mut self, inertial: &InertialDecl, pack: &str) {
        let label = format!("inertial '{}'", inertial.pred.text);
        let compiled = self.compile_ann(
            inertial.provenance.as_ref(),
            &label,
            pack,
            true,
            inertial.pred.span,
        );
        dli_eval::world::or_inertial(
            &mut self.wm.inertial_predicates,
            &inertial.pred.text,
            compiled.ann.clone(),
        );
        dli_eval::world::or_inertial(
            &mut self.wm.world.inertial_predicates,
            &inertial.pred.text,
            compiled.ann,
        );
    }

    /// The interval anchors a state's [`TimeClause`] bounds it to. `since`/`after`
    /// open the span at a point; `until` closes it at one; `during` opens it at
    /// the era (there is no era-end point to close on); an absent clause is the
    /// migration default — open on both ends (`docs/spec/03-time.md` §2). An
    /// `order` chain orders events rather than bounding one state, so it does not
    /// apply here (and `author.add_state` cannot produce it).
    fn state_bounds(
        &mut self,
        time: Option<&TimeClause>,
    ) -> (Option<TimePoint>, Option<TimePoint>) {
        match time {
            Some(TimeClause::Since(t) | TimeClause::After(t) | TimeClause::During(t)) => {
                (Some(self.time_point(&t.text)), None)
            }
            Some(TimeClause::Until(t)) => (None, Some(self.time_point(&t.text))),
            Some(TimeClause::Order(_)) | None => (None, None),
        }
    }

    /// Record a state's temporal clause as a [`TemporalReference`] to be resolved
    /// after all documents load, mirroring [`process_entity`](LoadCtx::process_entity):
    /// `since`/`until` require an event, `after`/`during` an era.
    fn record_state_time(&mut self, state: &StateDecl, label: &str, pack: &str) {
        let (target, expected_kind) = match &state.time {
            Some(TimeClause::Since(t) | TimeClause::Until(t)) => (t, "event"),
            Some(TimeClause::After(t) | TimeClause::During(t)) => (t, "era"),
            Some(TimeClause::Order(_)) | None => return,
        };
        self.temporal_refs.push(TemporalReference {
            decl: label.to_owned(),
            expected_kind: expected_kind.to_owned(),
            target: target.text.clone(),
            pack: pack.to_owned(),
            span: target.span,
        });
    }

    fn process_fact(&mut self, fact: &FactDecl, pack: &str) {
        let label = format!("fact '{}'", fact.atom.pred.text);
        self.observe_predicate_arity(
            &fact.atom.pred.text,
            fact.atom.args.len(),
            pack,
            fact.atom.pred.span,
        );
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
        // world must answer `precedes`/`as_of`, not carry inert time. The edge's
        // ruling rides parallel, so a `before`-cycle is blamed on it.
        let earlier = self.time_point(&before.earlier.text);
        let later = self.time_point(&before.later.text);
        self.wm.world.chronology.edges.push((earlier, later));
        self.wm.chronology_anns.push(compiled.ann.clone());
        self.push_atom(atom, compiled, pack, before.earlier.span);
    }

    /// Lower `same-as <left> = <right>` (WS-1, the identity kernel): the ground
    /// atom `same-as(left, right)` with mandatory provenance (fail-closed
    /// without one — koan 1, exactly [`process_before`](LoadCtx::process_before)'s
    /// discipline). A plain `fact same-as(x, y)` does NOT merge anything —
    /// identity comes from *this* declaration only, the same asymmetry
    /// `before`-the-fact has against `before`-the-ordering (koan 3). No
    /// chronology edge: unlike `before`, a same-as link orders nothing. The v1
    /// temporal boundary (an endpoint that is a declared era/event is refused)
    /// is checked in `crate::check::continuity`, where entity sorts are
    /// available — not here.
    fn process_same_as(&mut self, same_as: &SameAsDecl, pack: &str) {
        let atom = AtomExpr {
            pred: Ident {
                text: "same-as".to_owned(),
                span: same_as.left.span,
            },
            args: vec![
                Term::Sym(same_as.left.clone()),
                Term::Sym(same_as.right.clone()),
            ],
        };
        let label = format!("identity '{} = {}'", same_as.left.text, same_as.right.text);
        let compiled = self.compile_ann(
            same_as.provenance.as_ref(),
            &label,
            pack,
            true,
            same_as.left.span,
        );
        self.collect_entities(&atom);
        // `World::assert_same_as` is the ONLY intake that registers the pair
        // in the identity-declaration channel (`same_as_links`) — it also
        // pushes the queryable `same-as(left, right)` fact, so this replaces
        // the direct `wm.world.facts.push` a plain fact-producing decl would
        // use. A plain `Decl::Fact(same-as(x, y))` still routes through
        // `process_fact`, which never touches the channel — that asymmetry
        // (mirroring `Decl::Before`'s chronology edge) is what keeps identity
        // a declaration-only claim (koan 3, DL-0078).
        self.wm.world.assert_same_as(
            same_as.left.text.clone(),
            same_as.right.text.clone(),
            compiled.ann.clone(),
        );
        self.push_atom(atom, compiled, pack, same_as.left.span);
    }

    fn process_kind(&mut self, kind: &KindDecl, pack: &str) {
        let label = format!("kind '{}'", kind.name.text);
        let compiled =
            self.compile_ann(kind.provenance.as_ref(), &label, pack, true, kind.name.span);
        self.wm.sorts.push(SortEntry {
            name: kind.name.text.clone(),
            ann: compiled.ann,
            kind: compiled.kind,
            citations: compiled.citations,
            pack: pack.to_owned(),
        });
    }

    fn process_rule(&mut self, rule: &RuleDecl, pack: &str) {
        // A rule's provenance is checkable now, and — since WS-2 — it is also the
        // annotation every derivation through this rule carries: `fire_rules`
        // multiplies `rule.ann` into each production it fires, so a rule is a
        // claim like any other, defeasible on its own citation.
        let label = format!("rule '{}'", rule.name.text);
        let compiled =
            self.compile_ann(rule.provenance.as_ref(), &label, pack, true, rule.name.span);

        // Only a **bare** `rule` is a world (derivation) rule. `rule … over order`
        // and `rule … over season` route to the episode-ordering and season-ledger
        // engines respectively; they are not lowered into `world.rules`.
        if rule.domain.is_some() {
            return;
        }

        // The Datalog `head :- body` was structured by the front end (LS-9): it
        // lives either in the typed block clauses or in the typed inline clause.
        // This loader is a *reader* of that typed AST — it no longer re-enters
        // `dli_syntax`. Every clause that did not structure into datalog surfaces
        // a finding; a body-less rule likewise fails closed (koan 4).
        if !rule.body.clauses.is_empty() {
            for rc in &rule.body.clauses {
                self.lower_or_flag_world_rule(&rule.name, rc, rc.clause.span, &compiled.ann, pack);
            }
            return;
        }

        match &rule.body.inline {
            Some(rc) => {
                self.lower_or_flag_world_rule(&rule.name, rc, rule.name.span, &compiled.ann, pack);
            }
            None => self.findings.push(LoadFinding::MalformedWorldRule {
                rule: rule.name.text.clone(),
                body: String::new(),
                span: rule.name.span,
            }),
        }
    }

    /// Route one typed rule clause: lower its pre-parsed [`Datalog`] if present,
    /// or record a malformed-world-rule finding carrying the clause's normalized
    /// text — the exact behaviour the loader had when it re-parsed the text.
    fn lower_or_flag_world_rule(
        &mut self,
        name: &Ident,
        rc: &dli_ast::rule::RuleClause,
        span: Span,
        ann: &Ann,
        pack: &str,
    ) {
        match &rc.datalog {
            Some(datalog) => {
                self.lower_world_rule(
                    name,
                    datalog.head.clone(),
                    datalog.body.clone(),
                    span,
                    ann,
                    pack,
                );
            }
            None => self.findings.push(LoadFinding::MalformedWorldRule {
                rule: name.text.clone(),
                body: rc.clause.text.clone(),
                span,
            }),
        }
    }

    /// Route one structured world rule through the evaluator's negation-aware,
    /// range-restriction-checked intake, surfacing a range-restriction violation
    /// as a [`LoadFinding::UnboundHeadVariable`] and a stratification failure — a
    /// negative-dependency cycle across the whole rule set — from the
    /// `dli_eval::stratify` seam in [`finish`](LoadCtx::finish). Carries the
    /// rule's own compiled annotation into
    /// [`define_rule_annotated`](dli_eval::world::World::define_rule_annotated)
    /// (WS-2): every derivation through this rule multiplies in `ann`, so a
    /// rule is a defeasible claim rather than a standing axiom.
    fn lower_world_rule(
        &mut self,
        name: &Ident,
        head: AtomExpr,
        body: Vec<dli_ast::literal::BodyLiteral>,
        span: Span,
        ann: &Ann,
        pack: &str,
    ) {
        self.observe_predicate_arity(&head.pred.text, head.args.len(), pack, span);
        for literal in &body {
            self.observe_predicate_arity(
                &literal.atom.pred.text,
                literal.atom.args.len(),
                pack,
                span,
            );
        }
        if let Some(variable) = dli_eval::world::unbound_variable(&head, &body) {
            self.findings.push(LoadFinding::UnboundHeadVariable {
                rule: name.text.clone(),
                variable,
                span,
            });
            return;
        }
        // define_rule_annotated only errs on range restriction (handled above);
        // ignore the redundant error rather than double-report.
        let _ = self.wm.world.define_rule_annotated(
            name.text.clone(),
            head,
            body,
            ann.clone(),
            Grade::Asserted,
        );
    }

    fn process_cycle(&mut self, cycle: &CycleDecl, pack: &str) {
        self.wm.world.cycles.push(Cycle {
            name: cycle.name.text.clone(),
            spokes: cycle
                .spokes
                .iter()
                .map(|spoke| spoke.text.clone())
                .collect(),
        });
        if cycle.provenance.is_some() {
            let label = format!("cycle '{}'", cycle.name.text);
            let _ = self.compile_ann(
                cycle.provenance.as_ref(),
                &label,
                pack,
                false,
                cycle.name.span,
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
        self.wm.temporal_sources_complete = true;
        for reference in &self.temporal_refs {
            if self.temporal_decls.get(&reference.target) != Some(&reference.expected_kind) {
                self.findings.push(LoadFinding::UnknownTemporalReference {
                    decl: reference.decl.clone(),
                    expected_kind: reference.expected_kind.clone(),
                    target: reference.target.clone(),
                    pack: reference.pack.clone(),
                    span: reference.span,
                });
            }
        }
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

        // Identity closure (WS-1): synthesize the symmetric/transitive/rewrite
        // rules over every `same-as` fact BEFORE stratification, so the
        // synthesized rules are checked (and, on a negative-dependency cycle,
        // blamed) exactly like any other rule. A world with zero `same-as`
        // facts synthesizes zero rules — this call is then a no-op and the
        // compiled world is byte-identical to one that never called it (the
        // frozen-contract guard).
        dli_eval::world::synthesize_alias_rules(&mut self.wm.world);
        // Predicate identity has the same compiled-world reachability contract
        // as entity identity.  Keep this after all facts and typed rules have
        // loaded so the arity census sees rule heads and bodies as well as
        // extensional rows, and before stratification/saturation so every
        // three-path compile observes the same generated rewrite program.
        dli_eval::world::synthesize_predicate_alias_rules(&mut self.wm.world);

        // The load-time stratification handoff: consume the evaluator seam and
        // surface a negative-dependency cycle as a finding (spec 08 §1).
        if let Err(EvalError::Unstratifiable { reason }) = dli_eval::stratify(&self.wm.world) {
            self.findings.push(LoadFinding::Unstratifiable { reason });
        }

        // One malformation, one finding. A single declaration compiles several
        // claims — an entity alongside its slots, an option alongside its
        // implied fact — and an origin-less one can route more than one
        // identically-keyed finding through `compile_ann`. Collapse findings
        // that share a variant, declaration, pack, and span: an exact duplicate
        // carries no new information. Distinct findings — even a second
        // declaration of the same name at a *different* span — are all kept
        // (charter koan 4: nothing silently dropped). First-seen order is
        // preserved so the report stays deterministic.
        dedup_findings(&mut self.findings);

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

/// Collapse byte-identical load findings in place, preserving first-seen order.
///
/// Two findings with the same variant, declaration, pack, and span are the same
/// finding reported twice; the second is redundant. Anything that differs — a
/// second declaration of the same name at a *different* span, or a genuinely
/// distinct malformation — is kept, so no finding is silently dropped. The
/// finding set per load is small, so the linear membership scan is cheap.
fn dedup_findings(findings: &mut Vec<LoadFinding>) {
    let mut unique: Vec<LoadFinding> = Vec::with_capacity(findings.len());
    for finding in std::mem::take(findings) {
        if !unique.contains(&finding) {
            unique.push(finding);
        }
    }
    *findings = unique;
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
        Decl::Kind(s) => Some(("kind", s.name.text.clone(), s.name.span)),
        Decl::Question(q) => Some(("question", q.name.text.clone(), q.name.span)),
        Decl::Rule(r) => Some(("rule", r.name.text.clone(), r.name.span)),
        // `state` (a fluent) is deliberately multi-span: `state x.y = a until e`
        // and `state x.y = b since e` are one fluent over disjoint spans, not a
        // duplicate. Keyed out of the flat duplicate check.
        Decl::Cycle(w) => Some(("cycle", w.name.text.clone(), w.name.span)),
        Decl::Dialect(r) => Some(("dialect", r.name.text.clone(), r.name.span)),
        Decl::Word(l) => Some(("word", l.name.text.clone(), l.name.span)),
        Decl::NamePart(m) => Some(("name-part", m.name.text.clone(), m.name.span)),
        Decl::Practice(p) => Some(("practice", p.name.text.clone(), p.name.span)),
        Decl::Fork(f) => Some(("fork", f.question.text.clone(), f.question.span)),
        Decl::Name(n) => Some(("name", n.name.text.clone(), n.name.span)),
        Decl::Term(t) => Some(("term", t.name.text.clone(), t.name.span)),
        Decl::Principle(p) => Some(("principle", p.name.text.clone(), p.name.span)),
        Decl::Hypothesis(h) => Some(("hypothesis", h.name.text.clone(), h.name.span)),
        Decl::Proof(l) => Some(("proof", l.name.text.clone(), l.name.span)),
        _ => None,
    }
}

/// A hand-built document set used by the crate's doctests and tests: a neutral
/// public-domain **Camelot** subset — a sort, three entities (two places and a
/// thing), and two facts, all with well-formed provenance, so it loads with zero
/// findings. It is the small in-Rust demo world; the full teaching corpus ships
/// as `examples/camelot.dli`.
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
        scope: Some(ScopeHeader {
            name: Ident::synthetic("camelot"),
            span: Span::synthetic(),
        }),
        decls: vec![
            // The example world declares the decisions it cites, so a cited
            // fact reads certain (a forged citation still fails closed — B1).
            Decl::Ratify(dli_ast::decl::RatifyDecl {
                decision: Ident::synthetic("D-0001"),
                body: dli_ast::language::RatifyBody::default(),
            }),
            Decl::Ratify(dli_ast::decl::RatifyDecl {
                decision: Ident::synthetic("D-0002"),
                body: dli_ast::language::RatifyBody::default(),
            }),
            Decl::Kind(KindDecl {
                name: Ident::synthetic("place"),
                provenance: Some(ratified("D-0001")),
            }),
            place("camelot"),
            place("the-lake"),
            Decl::Entity(EntityDecl {
                kind: EntityKind::Thing,
                name: Ident::synthetic("excalibur"),
                label: None,
                time: None,
                slots: Vec::new(),
                provenance: Some(marked(ProvenanceMarker::Decided, Some("D-0002"))),
            }),
            fact(
                "adjacent",
                &["camelot", "the-lake"],
                Some(ratified("D-0001")),
                false,
            ),
            fact(
                "kept-at",
                &["excalibur", "camelot"],
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

    use dli_ast::decl::{Decl, Document, EntityDecl, EntityKind, ScopeHeader};
    use dli_ast::span::Span;

    fn originless_place() -> Decl {
        Decl::Entity(EntityDecl {
            kind: EntityKind::Place,
            name: Ident::synthetic("storybook"),
            label: None,
            time: None,
            slots: Vec::new(),
            provenance: None,
        })
    }

    fn one_pack(decls: Vec<Decl>) -> Document {
        Document {
            scope: Some(ScopeHeader {
                name: Ident::synthetic("storybook"),
                span: Span::synthetic(),
            }),
            decls,
        }
    }

    fn count_code(out: &super::LoadOutcome, code: &str) -> usize {
        out.findings.iter().filter(|f| f.code() == code).count()
    }

    #[test]
    fn single_originless_place_yields_one_missing_provenance() {
        // A place with no provenance is one malformation; it must surface once,
        // not once per compiled claim — the finding-dedup contract.
        let out = super::Loader::load(&[one_pack(vec![originless_place()])]);
        assert_eq!(
            count_code(&out, "load.missing-provenance"),
            1,
            "one malformation, one finding: {:?}",
            out.findings
        );
    }

    #[test]
    fn identical_findings_collapse_but_distinct_ones_survive() {
        // Two origin-less declarations of the same name compile to byte-identical
        // missing-provenance findings (same code, decl, pack, and span, since the
        // synthetic spans coincide): they collapse to one. The distinct
        // duplicate-declaration finding they also raise is untouched.
        let out = super::Loader::load(&[one_pack(vec![originless_place(), originless_place()])]);
        assert_eq!(
            count_code(&out, "load.missing-provenance"),
            1,
            "byte-identical findings collapse to one: {:?}",
            out.findings
        );
        assert_eq!(
            count_code(&out, "load.duplicate-declaration"),
            1,
            "the distinct duplicate-declaration finding is kept: {:?}",
            out.findings
        );
    }
}
