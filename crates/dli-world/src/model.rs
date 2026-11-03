//! The compiled world model and its surrounding tables.
//!
//! A [`WorldModel`] is what the loader produces from a set of source
//! [`Document`](dli_ast::decl::Document)s: the evaluable [`World`] plus the
//! authoring tables (such as registers and lexicon) *and* the compiled
//! provenance container — every source atom lifted with its
//! [`Ann`], the shared world interner its support polynomials index into, the
//! sort/entity registry, the pack-provenance map, and the load-time
//! [`LoadFinding`]s. Nothing that entered is silently dropped (charter koan 4);
//! everything a later evaluation needs to gate assertion is here.
//!
//! The annotation itself — the `(P⁺, P⁻)` pair and the standing it *evaluates*
//! to under a valuation — is the canonical [`dli_core::ann::Ann`]. This module
//! only adds the loader's descriptor of how a claim entered ([`Provenance`]) and
//! its rendered citations.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::Ann;
use dli_core::atom::{Atom, Interner, Rail};
use dli_eval::world::World;
use dli_time::fluent::FluentTimeline;

/// Stable code for the non-blocking union-policy promotion finding.  Keeping
/// the literal beside the load finding catalogue lets the finding-family
/// census assign this predicate-identity code to its own family while the
/// continuity check remains the emitter.
pub const PREDICATE_IDENTITY_POLICY_PROMOTED: &str = "predicate-identity.policy-promoted";

/// How a claim entered the world — the origin recorded on its annotation.
///
/// This mirrors [`dli_ast::annot::ProvenanceMarker`] (plus a fail-closed case);
/// it is *not* a stored status (charter koan 1: standing is derived from the
/// support polynomial via [`Ann::status`], never stored). It records which
/// register of origin gates the atom so a reader — and the grade channel
/// (CORE-3) — need not re-derive it. `Proposed` gates the atom by an *open*
/// hypothesis and is therefore structurally unassertable; the three ledger
/// markers gate it by a decision variable and become assertable once the ledger
/// valuation rules that decision true (spec 02 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    /// `proposed` — an agent's claim, gated by an open hypothesis variable;
    /// never certain until a ruling retires the hypothesis.
    Proposed,
    /// `decided <d-id>` — grounded in a recorded decision.
    Decided,
    /// `ratified <d-id>` — a ratified ruling (the strongest grade).
    Ratified,
    /// `provisional <d-id>` — a provisional ruling, open to revision.
    Provisional,
    /// No parseable provenance attached. Fail-closed: `P⁺ = 0`, excluded from
    /// every layer but the error report (spec 02 §3).
    Unprovenanced,
}

impl Provenance {
    /// The [`Provenance`] a well-formed
    /// [`ProvenanceMarker`](dli_ast::annot::ProvenanceMarker) compiles to.
    #[must_use]
    pub fn from_marker(marker: dli_ast::annot::ProvenanceMarker) -> Self {
        use dli_ast::annot::ProvenanceMarker as M;
        match marker {
            M::Proposed => Self::Proposed,
            M::Decided => Self::Decided,
            M::Ratified => Self::Ratified,
            M::Provisional => Self::Provisional,
        }
    }
}

/// A source atom lifted into the world with its compiled annotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LoadedAtom {
    /// The ground claim (predicate applied to argument terms).
    pub atom: AtomExpr,
    /// The `(P⁺, P⁻)` annotation. Its support is `PosBool::FALSE` when the claim
    /// is fail-closed (see [`is_fail_closed`](LoadedAtom::is_fail_closed)).
    pub ann: Ann,
    /// How the claim entered.
    pub kind: Provenance,
    /// Rendered ledger citations (`d-nnnn`), when the claim is decided.
    pub citations: Vec<String>,
    /// The pack the atom was declared in.
    pub pack: String,
    /// Source location of the declaration.
    pub span: Span,
    /// Source of its provenance, which may be inherited from an enclosing declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_span: Option<Span>,
}

impl LoadedAtom {
    /// Whether any argument of this atom is the ground symbol `entity`.
    #[must_use]
    pub fn mentions(&self, entity: &str) -> bool {
        self.atom.args.iter().any(|term| match term {
            Term::Sym(id) => id.text == entity,
            _ => false,
        })
    }

    /// Whether the atom is fail-closed: its support is identically false, so no
    /// valuation can ever make it hold (spec 02 §3).
    #[must_use]
    pub fn is_fail_closed(&self) -> bool {
        self.ann.support.is_false()
    }
}

/// A declared entity sort with its provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SortEntry {
    /// The sort name.
    pub name: String,
    /// The `(P⁺, P⁻)` annotation of the sort's existence.
    pub ann: Ann,
    /// How the sort entered.
    pub kind: Provenance,
    /// Rendered ledger citations, when decided.
    pub citations: Vec<String>,
    /// The pack the sort was declared in.
    pub pack: String,
    /// Source location of the declaration, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// An entity — a ground symbol observed in the world — with its sort when known.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EntityEntry {
    /// The entity symbol.
    pub name: String,
    /// The sort it belongs to. `Some(kind)` for a declared entity; `None` for a
    /// symbol only *referenced* in a fact, recorded so reachability sees it.
    pub sort: Option<String>,
}

/// What a pack contributed to the merged world (spec 00 §4 residency: source).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackProvenance {
    /// The pack name.
    pub pack: String,
    /// How many documents carried this pack header.
    pub documents: usize,
    /// How many declarations accrued into the pack.
    pub decls: usize,
}

/// A load-time well-formedness finding.
///
/// Nothing that entered is silently dropped (charter koan 4); every rejected or
/// degraded input surfaces here. Each variant carries a stable machine-readable
/// [`code`](LoadFinding::code) *and* a domain-spoken [`message`](LoadFinding::message)
/// — the two-channel error model (charter koan 9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "finding", rename_all = "kebab-case")]
pub enum LoadFinding {
    /// A lore-bearing claim carried no provenance — a claim from nowhere
    /// (charter koan 1). Fail-closed to `P⁺ = 0`.
    MissingProvenance {
        /// The declaration that lacked provenance (kind and name/predicate).
        decl: String,
        /// The pack it was declared in.
        pack: String,
        /// Source location.
        span: Span,
    },
    /// A provenance annotation was present but could not be compiled to a
    /// support polynomial. Fail-closed to `P⁺ = 0` (spec 02 §3, the fail-closed
    /// theorem).
    MalformedProvenance {
        /// The declaration whose annotation was malformed.
        decl: String,
        /// The pack it was declared in.
        pack: String,
        /// Why the annotation could not be compiled.
        reason: String,
        /// Source location.
        span: Span,
    },
    /// A bare top-level world rule could not be read as Datalog, so lowering it
    /// would otherwise discard the declaration without producing a rule.
    MalformedWorldRule {
        /// The rule name.
        rule: String,
        /// The body text that could not be read, if one was present.
        body: String,
        /// Source location.
        span: Span,
    },
    /// A rule head variable was not bound in the body (range restriction; spec
    /// 08 §1).
    UnboundHeadVariable {
        /// The rule name.
        rule: String,
        /// The unbound head variable.
        variable: String,
        /// Source location.
        span: Span,
    },
    /// A typed temporal clause named an era or event that the merged world does
    /// not declare. Unlike an ordinary fact symbol, a temporal target is a
    /// closed reference: `during`/`after` require an era and `since`/`until`
    /// require an event.
    UnknownTemporalReference {
        /// The declaration carrying the temporal clause.
        decl: String,
        /// The required target kind (`era` or `event`).
        expected_kind: String,
        /// The unresolved target name.
        target: String,
        /// The pack containing the reference.
        pack: String,
        /// Source location of the target.
        span: Span,
    },
    /// Two declarations of the same kind and name collided within one pack.
    DuplicateDeclaration {
        /// The pack the collision occurred in.
        pack: String,
        /// The declaration kind (`fact`, `rule`, `sort`, …).
        kind: String,
        /// The colliding name.
        name: String,
        /// Source location of the second declaration.
        span: Span,
    },
    /// The same name was declared in more than one pack.
    CrossPackCollision {
        /// The declaration kind.
        kind: String,
        /// The colliding name.
        name: String,
        /// The packs that each declared it.
        packs: Vec<String>,
        /// A declaration in one of the colliding packs, when available.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        span: Option<Span>,
    },
    /// The rule set has no valid stratification: a negative-dependency (or
    /// aggregation) cycle. Surfaced from the `dli_eval::stratify` seam (spec 08
    /// §1).
    Unstratifiable {
        /// What made stratification impossible.
        reason: String,
    },
    /// A declaration was accepted by the grammar but names nothing evaluable, so
    /// it would lower to no fact at all — an inert declaration (DL-0029). Rather
    /// than silently drop it (charter koan 4), the loader surfaces it. Today this
    /// is the residual `state <name>` with neither a dotted subject nor a value.
    InertDeclaration {
        /// The inert declaration (kind and name).
        decl: String,
        /// The pack it was declared in.
        pack: String,
        /// Source location.
        span: Span,
    },
    /// Two predicates in one declared identity component have incompatible
    /// observed arities.  The component is refused rather than half-merged.
    PredicateIdentityArityMismatch {
        /// The predicate mutation that caused the component-local check.
        touched_predicate: String,
        /// The predicates in the affected identity component.
        predicates: Vec<String>,
        /// The incompatible observed arities.
        arities: Vec<usize>,
        /// The pack containing the mutation.
        pack: String,
        /// The source location of the mutation.
        span: Span,
    },
}

impl LoadFinding {
    /// The source location, when the finding identifies a particular declaration.
    #[must_use]
    pub fn span(&self) -> Option<Span> {
        let span = match self {
            Self::MissingProvenance { span, .. }
            | Self::MalformedProvenance { span, .. }
            | Self::MalformedWorldRule { span, .. }
            | Self::UnboundHeadVariable { span, .. }
            | Self::UnknownTemporalReference { span, .. }
            | Self::DuplicateDeclaration { span, .. }
            | Self::InertDeclaration { span, .. }
            | Self::PredicateIdentityArityMismatch { span, .. } => Some(*span),
            Self::CrossPackCollision { span, .. } => *span,
            Self::Unstratifiable { .. } => None,
        };
        span.filter(|span| span.end > span.start)
    }

    /// A stable, machine-readable class for this finding (charter koan 9,
    /// two-channel error model). Clients and tools match on this; it never
    /// changes for a given variant.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingProvenance { .. } => "load.missing-provenance",
            Self::MalformedProvenance { .. } => "load.malformed-provenance",
            Self::MalformedWorldRule { .. } => "load.malformed-world-rule",
            Self::UnboundHeadVariable { .. } => "load.unbound-head-variable",
            Self::UnknownTemporalReference { .. } => "load.unknown-temporal-reference",
            Self::DuplicateDeclaration { .. } => "load.duplicate-declaration",
            Self::CrossPackCollision { .. } => "load.cross-pack-collision",
            Self::Unstratifiable { .. } => "load.unstratifiable",
            Self::InertDeclaration { .. } => "load.inert-declaration",
            Self::PredicateIdentityArityMismatch { .. } => "predicate-identity.arity-mismatch",
        }
    }

    /// A domain-spoken description of the finding (charter koan 9, errors are
    /// lore).
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::MissingProvenance { decl, pack, .. } => {
                format!(
                    "{decl} in pack '{pack}' claims something from nowhere: it carries no ruling and no hypothesis, so it cannot be asserted"
                )
            }
            Self::MalformedProvenance {
                decl, pack, reason, ..
            } => {
                format!(
                    "{decl} in pack '{pack}' has provenance that cannot be read ({reason}); it is held fail-closed until the annotation is fixed"
                )
            }
            Self::MalformedWorldRule { rule, body, .. } => {
                let detail = if body.is_empty() {
                    "it has no derivation body".to_owned()
                } else {
                    format!("`{body}` is not a Datalog derivation")
                };
                format!(
                    "rule '{rule}' is a bare world rule, but {detail}; write `<head> :- <premise>, ...`, use `over order` or `over season` for a checker rule, or put `rule ... requires` inside a practice"
                )
            }
            Self::UnboundHeadVariable { rule, variable, .. } => {
                format!(
                    "rule '{rule}' concludes about '{variable}', which nothing in its premises names"
                )
            }
            Self::UnknownTemporalReference {
                decl,
                expected_kind,
                target,
                pack,
                ..
            } => {
                format!(
                    "{decl} in pack '{pack}' references {expected_kind} '{target}', which the world does not declare"
                )
            }
            Self::DuplicateDeclaration {
                pack, kind, name, ..
            } => {
                format!("'{name}' is declared twice as a {kind} in pack '{pack}'")
            }
            Self::CrossPackCollision {
                kind, name, packs, ..
            } => {
                format!(
                    "the {kind} '{name}' is declared in more than one pack ({})",
                    packs.join(", ")
                )
            }
            Self::Unstratifiable { reason } => {
                format!("the rules cannot be ordered for evaluation: {reason}")
            }
            Self::InertDeclaration { decl, pack, .. } => {
                format!(
                    "{decl} in pack '{pack}' names nothing evaluable, so it would lower to no fact; give it a value (and a dotted subject) so it can be asserted"
                )
            }
            Self::PredicateIdentityArityMismatch {
                touched_predicate,
                predicates,
                arities,
                pack,
                ..
            } => format!(
                "predicate identity touched by '{touched_predicate}' in pack '{pack}' joins predicates [{}] with incompatible observed arities [{}]; the merge is refused",
                predicates.join(", "),
                arities
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

/// A provenance-bearing assertion that an event occurs during an era.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EraMembership {
    /// Event in the era.
    pub event: dli_time::chronology::TimePoint,
    /// Era containing the event.
    pub era: dli_time::chronology::TimePoint,
    /// Support for this membership.
    pub ann: Ann,
}

/// One symbolically inferred precedence between eras and its complete support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EraEdge {
    /// Earlier era.
    pub earlier: dli_time::chronology::TimePoint,
    /// Later era.
    pub later: dli_time::chronology::TimePoint,
    /// Symbolic support, including every alternative derivation.
    pub ann: Ann,
}

/// A compiled world model: the evaluable world, its authoring tables, and the
/// compiled provenance container.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorldModel {
    /// The evaluable world program (ground facts, rules, time structures).
    pub world: World,
    /// The compiled language layer: dialects, name-parts, names, terms, bans.
    pub language: crate::language::LanguageModel,
    /// The annotated source atoms, in load order.
    pub atoms: Vec<LoadedAtom>,
    /// The declared entity sorts.
    pub sorts: Vec<SortEntry>,
    /// The entities (ground symbols) observed in the world, deduplicated.
    pub entities: Vec<EntityEntry>,
    /// What each pack contributed, keyed by pack name.
    pub packs: BTreeMap<String, PackProvenance>,
    /// The load-time findings (charter koan 4: nothing silently dropped).
    pub findings: Vec<LoadFinding>,
    /// The rulings behind the chronology edges, parallel to
    /// `world.chronology.edges`: `chronology_anns[i]` is the annotation of the
    /// declaration (an era-order chain or a `before`-fact) that pushed edge `i`.
    ///
    /// Kept beside the chronology rather than inside it, so the plain-graph
    /// [`Chronology`](dli_time::chronology::Chronology) seam stays annotation-free;
    /// [`find_cycle`](dli_time::chronology::Chronology::find_cycle) takes this
    /// parallel slice to blame a precedence cycle on the rulings that closed it
    /// (declaring an order is a ruling). `#[serde(default)]` so a world
    /// serialized before this field round-trips.
    #[serde(default)]
    pub chronology_anns: Vec<Ann>,
    /// Source locations for direct chronology edges, parallel to the edge list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chronology_spans: Vec<dli_ast::span::Span>,
    /// Source locations keyed by index in `world.same_as_links`. Missing entries
    /// represent synthetic or older worlds without source information.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub same_as_spans: BTreeMap<usize, dli_ast::span::Span>,
    /// Authored source clauses keyed by index in `world.rules`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rule_spans: BTreeMap<usize, dli_ast::span::Span>,
    /// Event-in-era assertions, kept symbolic until read under a valuation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub era_memberships: Vec<EraMembership>,
    /// Source locations keyed by index in `era_memberships`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub era_membership_spans: BTreeMap<usize, dli_ast::span::Span>,
    /// Symbolic order among eras with event memberships.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inferred_era_edges: Vec<EraEdge>,
    /// Per-slot fluent timelines, keyed by the `(pred, subject)` slot a `state`
    /// declaration lowers to. A `state s.p = v [since/until …]` pushes a
    /// [`Span`](dli_time::fluent::Span) here carrying `v`, its interval anchors,
    /// and the declaration's ruling — so the same slot held over disjoint spans
    /// (`commons until e` / `enclosed since e`) is one fluent, not a
    /// contradiction. The check path reads these to reason about interval
    /// overlap; slots with no timeline fall back to the flat certain-value reader
    /// (issues 47/48). `#[serde(default)]` so a world serialized before this
    /// field round-trips.
    #[serde(default)]
    pub fluents: BTreeMap<(String, String), FluentTimeline<String>>,
    /// Evaluable fact indices corresponding to each slot's fluent spans, in
    /// the same order. This identifies temporal facts without conflating a
    /// separate timeless assertion of the same atom.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fluent_fact_indices: BTreeMap<(String, String), Vec<usize>>,
    /// Temporal state facts that do not have a binary subject/value slot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub standalone_temporal_facts: Vec<(usize, dli_time::fluent::Span<String>)>,
    /// Source atom index to corresponding evaluable fact index for state declarations.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub temporal_atom_facts: BTreeMap<usize, usize>,
    /// Whether the loader recorded every state-to-source mapping in this world.
    #[serde(default)]
    pub temporal_sources_complete: bool,
    /// Predicates an author has declared single-valued via `functional <pred>`.
    /// The FD/contradiction check treats a slot on one of these as a functional
    /// dependency even when canon bears no prior fact for it — so a *first-seen*
    /// predicate set to two distinct certain values is a contradiction, which
    /// canon-precedent inference alone cannot catch (issue 49, sub-claim 4).
    /// `#[serde(default)]` so a world serialized before this field round-trips.
    #[serde(default)]
    pub functional_predicates: BTreeSet<String>,
    /// Predicates an author has declared fluent-persistent via `inertial
    /// <pred>`, each mapped to the **OR of its declaring provenance's [`Ann`]**
    /// (a predicate may be declared inertial more than once). The continuity
    /// check consults this to derive an open value's effective boundary from the
    /// live competitors at projection time (`docs/decisions.md` DL-0081,
    /// `docs/design-notes/inertia-fluent-persistence.md`): capping applies only
    /// while the declaration's own `Ann` is certain, so defeating the
    /// declaration — a valuation flip, no recompile — reverts the slot to plain
    /// open-interval projection. `#[serde(default, skip_serializing_if)]` so a
    /// world that declares none serializes byte-identically to one from before
    /// this field existed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inertial_predicates: BTreeMap<String, Ann>,
    /// Predicate identity declarations and the incremental observed-arity
    /// cache used to validate only affected identity components.
    #[serde(
        default,
        skip_serializing_if = "crate::predicate_identity::PredicateIdentityState::is_empty"
    )]
    pub predicate_identity: crate::predicate_identity::PredicateIdentityState,
}

impl WorldModel {
    /// Visit inferred event edges, combining both memberships with the era path.
    fn for_each_inferred_edge(
        &self,
        mut visit: impl FnMut(dli_time::chronology::TimePoint, dli_time::chronology::TimePoint, Ann),
    ) {
        use dli_eval::world::conjoin_annotations;
        let mut by_era: BTreeMap<u32, Vec<&EraMembership>> = BTreeMap::new();
        for member in &self.era_memberships {
            by_era.entry(member.era.0).or_default().push(member);
        }
        for path in &self.inferred_era_edges {
            for first in by_era.get(&path.earlier.0).into_iter().flatten() {
                let prefix = conjoin_annotations(&first.ann, &path.ann);
                for second in by_era.get(&path.later.0).into_iter().flatten() {
                    visit(
                        first.event,
                        second.event,
                        conjoin_annotations(&prefix, &second.ann),
                    );
                }
            }
        }
    }

    /// Visit all distinct edges, joining alternatives when the same pair can
    /// be obtained more than once. The common case has one membership per
    /// event, so inferred pairs can be streamed without storing them.
    fn for_each_order_edge(
        &self,
        mut visit: impl FnMut(dli_time::chronology::TimePoint, dli_time::chronology::TimePoint, Ann),
    ) {
        use dli_eval::world::disjoin_annotations;
        use dli_time::chronology::TimePoint;
        let mut buffered: BTreeMap<(u32, u32), Ann> = BTreeMap::new();
        for (index, &(a, b)) in self.world.chronology.edges.iter().enumerate() {
            let ann = self
                .chronology_anns
                .get(index)
                .cloned()
                .unwrap_or_else(Ann::unconditional);
            buffered
                .entry((a.0, b.0))
                .and_modify(|old| *old = disjoin_annotations(old, &ann))
                .or_insert(ann);
        }
        let mut events = BTreeSet::new();
        let mut repeated_events = BTreeSet::new();
        for member in &self.era_memberships {
            if !events.insert(member.event.0) {
                repeated_events.insert(member.event.0);
            }
        }
        self.for_each_inferred_edge(|a, b, ann| {
            let key = (a.0, b.0);
            if repeated_events.contains(&a.0)
                || repeated_events.contains(&b.0)
                || buffered.contains_key(&key)
            {
                buffered
                    .entry(key)
                    .and_modify(|old| *old = disjoin_annotations(old, &ann))
                    .or_insert(ann);
            } else {
                visit(a, b, ann);
            }
        });
        for ((a, b), ann) in buffered {
            visit(TimePoint(a), TimePoint(b), ann);
        }
    }

    /// All authored and inferred edges with their matching annotations.
    /// This expands event pairs on demand; the stored model remains compact.
    #[must_use]
    pub fn annotated_chronology(&self) -> (dli_time::chronology::Chronology, Vec<Ann>) {
        let mut graph = dli_time::chronology::Chronology::default();
        let mut anns = Vec::new();
        self.for_each_order_edge(|earlier, later, ann| {
            graph.edges.push((earlier, later));
            anns.push(ann);
        });
        (graph, anns)
    }

    /// Only precedence that is certain under the current valuation.
    #[must_use]
    pub fn chronology_with_era_membership(&self) -> dli_time::chronology::Chronology {
        use dli_core::ann::Status;
        let mut graph = dli_time::chronology::Chronology::default();
        self.for_each_order_edge(|earlier, later, ann| {
            if ann.status(&self.world.valuation, &self.world.theory) == Status::Certain {
                graph.edges.push((earlier, later));
            }
        });
        graph
    }

    /// Certain support for one edge, including all independent authored and inferred paths.
    #[must_use]
    pub fn certain_edge_annotation(
        &self,
        earlier: dli_time::chronology::TimePoint,
        later: dli_time::chronology::TimePoint,
    ) -> Option<Ann> {
        use dli_core::ann::Status;
        use dli_eval::world::{conjoin_annotations, disjoin_annotations};
        let mut result: Option<Ann> = None;
        let mut add = |ann: Ann| {
            result = Some(
                result
                    .as_ref()
                    .map_or_else(|| ann.clone(), |old| disjoin_annotations(old, &ann)),
            );
        };
        for (index, &(a, b)) in self.world.chronology.edges.iter().enumerate() {
            if a == earlier && b == later {
                add(self
                    .chronology_anns
                    .get(index)
                    .cloned()
                    .unwrap_or_else(Ann::unconditional));
            }
        }
        for first in self
            .era_memberships
            .iter()
            .filter(|member| member.event == earlier)
        {
            for second in self
                .era_memberships
                .iter()
                .filter(|member| member.event == later)
            {
                for path in self
                    .inferred_era_edges
                    .iter()
                    .filter(|edge| edge.earlier == first.era && edge.later == second.era)
                {
                    let prefix = conjoin_annotations(&first.ann, &path.ann);
                    add(conjoin_annotations(&prefix, &second.ann));
                }
            }
        }
        result
            .filter(|ann| ann.status(&self.world.valuation, &self.world.theory) == Status::Certain)
    }

    /// The entities observed in the world.
    #[must_use]
    pub fn entities(&self) -> &[EntityEntry] {
        &self.entities
    }

    /// Every loaded atom that names `entity` in an argument position.
    ///
    /// ```
    /// use dli_world::loader::{Loader, example_docs};
    /// let out = Loader::load(&example_docs());
    /// assert!(!out.world.facts_about("excalibur").is_empty());
    /// ```
    #[must_use]
    pub fn facts_about(&self, entity: &str) -> Vec<&LoadedAtom> {
        self.atoms.iter().filter(|a| a.mentions(entity)).collect()
    }

    /// Fingerprint all semantic facts about one subject.
    ///
    /// The row includes the atom, its derived standing, and support/defeater
    /// variables resolved back to stable names. A ratification or support
    /// change therefore stales an attestation even when the atom text survives.
    #[must_use]
    pub fn subject_fingerprint(&self, entity: &str) -> crate::attest::ContentHash {
        crate::attest::fingerprint_strings(
            self.world
                .derived
                .iter()
                .filter(|derived| derived.atom.args.iter().any(|argument| argument == entity))
                .map(|derived| self.fingerprint_derived_row(derived)),
        )
    }

    /// The number of derivation rules in the compiled world.
    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.world.rules.len()
    }

    /// The load-time findings.
    #[must_use]
    pub fn findings(&self) -> &[LoadFinding] {
        &self.findings
    }

    /// The annotated source atoms.
    #[must_use]
    pub fn atoms(&self) -> &[LoadedAtom] {
        &self.atoms
    }

    /// The declared entity sorts.
    #[must_use]
    pub fn sorts(&self) -> &[SortEntry] {
        &self.sorts
    }

    /// What each pack contributed to the merged world.
    #[must_use]
    pub fn packs(&self) -> &BTreeMap<String, PackProvenance> {
        &self.packs
    }

    /// The name of an event variable, or `None` if the id is out of range.
    #[must_use]
    pub fn var_name(&self, atom: Atom) -> Option<&str> {
        self.world.interner.resolve(atom)
    }

    /// The [`Atom`] a support literal references an event variable by, looked up
    /// by name. `None` if the world never interned it.
    #[must_use]
    pub fn var_atom(&self, name: &str) -> Option<Atom> {
        self.world
            .interner
            .names()
            .iter()
            .position(|candidate| candidate == name)
            .map(|index| Atom(u32::try_from(index).unwrap_or(u32::MAX)))
    }

    /// The shared event-variable interner, so a caller can map
    /// support-polynomial literals back to variable names.
    #[must_use]
    pub fn interner(&self) -> &Interner {
        &self.world.interner
    }

    fn fingerprint_derived_row(&self, derived: &dli_eval::world::Derived) -> String {
        let ann = derived.ann();
        let status = ann.status(&self.world.valuation, &self.world.theory);
        let support = self.render_polynomial(&ann.support);
        let defeaters = self.render_polynomial(&ann.defeaters);
        let conditions = ann
            .conditions
            .iter()
            .map(|(path, poison)| {
                format!(
                    "{}=>{}",
                    self.render_monomial(path),
                    self.render_polynomial(poison)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let atom = serde_json::to_string(&derived.atom)
            .expect("a ground atom always serializes for a fingerprint");
        format!(
            "{atom}|{status:?}|{support}|{defeaters}|{conditions}|{:?}",
            derived.grade
        )
    }

    fn render_polynomial(&self, value: &dli_core::posbool::PosBool) -> String {
        value
            .monomials()
            .map(|monomial| self.render_monomial(monomial))
            .collect::<Vec<_>>()
            .join("|")
    }

    fn render_monomial(&self, value: &dli_core::posbool::Monomial) -> String {
        value
            .lits()
            .map(|literal| {
                let rail = match literal.rail {
                    Rail::Pos => '+',
                    Rail::Neg => '-',
                };
                let name = self.var_name(literal.atom).unwrap_or("<unknown>");
                format!("{rail}{name}")
            })
            .collect::<Vec<_>>()
            .join("&")
    }
}
