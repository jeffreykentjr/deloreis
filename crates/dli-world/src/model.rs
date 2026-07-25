//! The compiled world model and its surrounding tables.
//!
//! A [`WorldModel`] is what the loader produces from a set of source
//! [`Document`](dli_ast::decl::Document)s: the evaluable [`World`] plus the
//! authoring tables (registers, lexicon, craft rules, the season ledger) *and*
//! the compiled provenance container — every source atom lifted with its
//! [`Ann`], the shared world interner its support polynomials index into, the
//! sort/entity registry, the pack-provenance map, and the load-time
//! [`LoadFinding`]s. Nothing that entered is silently dropped (charter koan 4);
//! everything a later evaluation needs to gate assertion is here.
//!
//! The annotation itself — the `(P⁺, P⁻)` pair and the standing it *evaluates*
//! to under a valuation — is the canonical [`dli_core::ann::Ann`]. This module
//! only adds the loader's descriptor of how a claim entered ([`Provenance`]) and
//! its rendered citations.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::Ann;
use dli_core::atom::{Atom, Interner};
use dli_eval::world::World;
use dli_ledger::{LedgerEntry, PracticeDef};
use dli_time::cycle::Cycle;

/// A craft rule binding a principle to a register.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CraftRule {
    /// The craft principle.
    pub principle: String,
    /// The register it governs.
    pub register: String,
}

/// The season ledger: a seasonal cycle plus the practices tracked against it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SeasonLedger {
    /// The seasonal cycle, if one is defined.
    pub season_cycle: Option<Cycle>,
    /// Practice definitions checked this season.
    pub practices: Vec<PracticeDef>,
    /// Recorded ledger entries.
    pub entries: Vec<LedgerEntry>,
}

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
    },
    /// The rule set has no valid stratification: a negative-dependency (or
    /// aggregation) cycle. Surfaced from the `dli_eval::stratify` seam (spec 08
    /// §1).
    Unstratifiable {
        /// What made stratification impossible.
        reason: String,
    },
}

impl LoadFinding {
    /// A stable, machine-readable class for this finding (charter koan 9,
    /// two-channel error model). Benchmarks and tools match on this; it never
    /// changes for a given variant.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingProvenance { .. } => "load.missing-provenance",
            Self::MalformedProvenance { .. } => "load.malformed-provenance",
            Self::MalformedWorldRule { .. } => "load.malformed-world-rule",
            Self::UnboundHeadVariable { .. } => "load.unbound-head-variable",
            Self::DuplicateDeclaration { .. } => "load.duplicate-declaration",
            Self::CrossPackCollision { .. } => "load.cross-pack-collision",
            Self::Unstratifiable { .. } => "load.unstratifiable",
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
            Self::DuplicateDeclaration {
                pack, kind, name, ..
            } => {
                format!("'{name}' is declared twice as a {kind} in pack '{pack}'")
            }
            Self::CrossPackCollision { kind, name, packs } => {
                format!(
                    "the {kind} '{name}' is declared in more than one pack ({})",
                    packs.join(", ")
                )
            }
            Self::Unstratifiable { reason } => {
                format!("the rules cannot be ordered for evaluation: {reason}")
            }
        }
    }
}

/// A compiled world model: the evaluable world, its authoring tables, and the
/// compiled provenance container.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorldModel {
    /// The evaluable world program (ground facts, rules, time structures).
    pub world: World,
    /// The compiled language layer: dialects, name-parts, names, terms, bans.
    pub language: crate::language::LanguageModel,
    /// Craft rules.
    pub craft_rules: Vec<CraftRule>,
    /// The season ledger.
    pub season: SeasonLedger,
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
}

impl WorldModel {
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
}
