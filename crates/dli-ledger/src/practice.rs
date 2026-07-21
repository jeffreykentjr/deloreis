//! The practice declaration layer.
//!
//! A **practice** — a magic system, a divination game, a proof discipline — is
//! declared entirely as data (`docs/spec/04-practices.md`). This module compiles
//! a parsed [`PracticeDecl`] into a typed [`PracticeDef`] and checks it for the
//! three compile-time well-formedness obligations of spec §6, before any casting
//! is run:
//!
//! 1. **Every declared error refines a structural class** — one of `L1`..`L7`,
//!    the seven the checker knows ([`StructuralClass`]). An error that names no
//!    class, or one outside the range, does not compile.
//! 2. **Resources use only the closed algebra** — a [`ResourceDecl`] whose kind
//!    is not one of [`ResourceKind`]'s members is a compile error; there is no
//!    open resource vocabulary.
//! 3. **Declarations resolve against the practice's sorts** — a role annotated
//!    with a sort the practice does not declare (nor a primitive kind nor a
//!    cycle), and a failure or law that cites a cycle not in scope, do not
//!    compile.
//!
//! # The interior grammar of a practice block
//!
//! Wave-1's parser captures a practice block's interior as a flat list of
//! whitespace-normalized [`Clause`]s (one per physical line), leaving the
//! interior grammar to this lane. That grammar is nine keyword-fronted forms:
//!
//! ```text
//! use <target>                                     # a consumed layer
//! kind <name> = <v1> | <v2> | …                    # a declared finite kind
//! kind <name> from <source>                        #   …or one drawn from a layer
//! role <name> : <kind> [requires <conditions>]     # a cast role and its kind
//! resource <name>[(<arg>)] : <kind …>              # a resource and its algebra law
//! rule <form>(<args>) [requires <premises>]        # a law of the practice
//! condition <name> requires <predicate …>          # a side condition
//! error <name> refines L<n> <discriminant …> [signals <signal>]
//! effect <form> <detail …>                         # what a casting does
//! cost <c1> | <c2> | …                             # the currency vocabulary
//! ```
//!
//! A line whose first word is not one of those keywords is a **continuation** of
//! the form above it — a `rule … requires` whose premises wrap across lines
//! re-joins into one form. Atom-shaped heads (`lay(figure, leaf)`,
//! `acquaintance(name)`) are read through [`parse_clause_atom`], the one shared
//! way captured clause text becomes a typed atom.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::annot::ProvenanceMarker;
use dli_ast::clause::Clause;
use dli_ast::decl::{Decl, Document, PracticeDecl};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::Term;
use dli_syntax::parse_clause_atom;

use crate::resource::{GradeDirection, ResourceKind};

/// The primitive entity kinds a role's sort may name without a `sort`
/// declaration — the world-entity sorts of `docs/spec/01-syntax.md`.
pub const PRIMITIVE_SORTS: &[&str] = &[
    "entity", "artifact", "place", "person", "thing", "event", "era",
];

/// The interior keywords that open a practice-block form (§ module docs). A
/// clause opening with any other word continues the form above it.
const ITEM_KEYWORDS: &[&str] = &[
    "use",
    "kind",
    "role",
    "resource",
    "rule",
    "condition",
    "error",
    "effect",
    "cost",
];

/// The seven structural error classes the checker knows.
///
/// Every diegetic failure a practice declares is a *refinement* of one of these
/// (`docs/spec/04-practices.md` §2). The classes are the closed target set that
/// well-formedness obligation 1 checks against: an `error` refining anything
/// else does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum StructuralClass {
    /// L1 — unknown-form: the casting names a form the practice does not declare.
    L1,
    /// L2 — premise-not-available: a required premise is absent or contradicted.
    L2,
    /// L3 — side-condition: a declared condition fails (timing, consent, scope).
    L3,
    /// L4 — bad-instantiation: a form is instantiated against forbidden sorts.
    L4,
    /// L5 — invalid-chain: a step joins premises the law does not license.
    L5,
    /// L6 — resource-violation: a spend breaks the resource's algebra.
    L6,
    /// L7 — close-mismatch: the close does not follow from the accounted ledger.
    L7,
}

impl StructuralClass {
    /// Parse a refinement token (`L1`..`L7`, case-insensitive) into a class.
    ///
    /// Anything else — a bare word, an out-of-range `L8`, a lone `L` — is
    /// [`None`], which well-formedness surfaces as an error refining nothing.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        match token.to_ascii_uppercase().as_str() {
            "L1" => Some(Self::L1),
            "L2" => Some(Self::L2),
            "L3" => Some(Self::L3),
            "L4" => Some(Self::L4),
            "L5" => Some(Self::L5),
            "L6" => Some(Self::L6),
            "L7" => Some(Self::L7),
            _ => None,
        }
    }

    /// The structural meaning of this class.
    #[must_use]
    pub const fn meaning(self) -> &'static str {
        match self {
            Self::L1 => "unknown-form",
            Self::L2 => "premise-not-available",
            Self::L3 => "side-condition",
            Self::L4 => "bad-instantiation",
            Self::L5 => "invalid-chain",
            Self::L6 => "resource-violation",
            Self::L7 => "close-mismatch",
        }
    }
}

/// A declared finite sort: `sort figure = the-tide | the-weir | …`, or a sort
/// drawn wholesale from a consumed layer: `sort binder from lexicon.loan-words`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SortDef {
    /// The sort name.
    pub name: Ident,
    /// The enumerated members, when written with `= a | b | …`.
    pub variants: Vec<Ident>,
    /// The layer the sort is drawn from, when written with `from <source>`.
    pub source: Option<String>,
    /// Source span of the declaration.
    pub span: Span,
}

/// A cast role: `role reader : entity requires standing(reader)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoleDef {
    /// The role name.
    pub name: Ident,
    /// The sort a filler of this role must belong to.
    pub sort: Ident,
    /// The standing conditions the role requires, verbatim.
    pub requires: Option<String>,
    /// Source span of the declaration.
    pub span: Span,
}

/// A declared resource and the closed-algebra law it obeys:
/// `resource acquaintance(name) : graded monotone whole > worn > husk`.
///
/// The [`kind`](ResourceDecl::kind) is the closed-algebra tag; it is [`None`]
/// exactly when the declared law is not a member of the algebra, which
/// well-formedness surfaces as a compile error. The law's parameters — an affine
/// [`cap`](ResourceDecl::cap), a graded [`order`](ResourceDecl::order) and
/// [`direction`](ResourceDecl::direction) — ride here alongside the tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceDecl {
    /// The resource name.
    pub name: Ident,
    /// The parameter sort, when written `name(arg)` (e.g. `acquaintance(name)`).
    pub arg: Option<Ident>,
    /// The closed-algebra law, or [`None`] if the declared law is outside it.
    pub kind: Option<ResourceKind>,
    /// The affine capacity expression, verbatim, for `affine bounded <cap>`.
    pub cap: Option<String>,
    /// The graded order, most-whole first, for `graded monotone <order>`.
    pub order: Vec<Ident>,
    /// The direction a graded resource wears (`>` reads as decay).
    pub direction: GradeDirection,
    /// The law phrase as written, kept for the error message when `kind` is
    /// [`None`].
    pub law_text: String,
    /// Source span of the declaration.
    pub span: Span,
}

/// A law of the practice: `rule lay(figure, leaf) requires phase(turning-leaf) = leaf`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleDef {
    /// The form the rule concludes (the head predicate).
    pub form: Ident,
    /// The head argument sorts/variables, in order.
    pub args: Vec<Ident>,
    /// The premise body after `requires`, verbatim.
    pub requires: Option<String>,
    /// Source span of the declaration.
    pub span: Span,
}

/// A side condition: `condition seated-reading requires phase(turning-leaf) within declared-span`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConditionDef {
    /// The condition name.
    pub name: Ident,
    /// The predicate body after `requires`, verbatim.
    pub requires: String,
    /// Source span of the declaration.
    pub span: Span,
}

/// A declared diegetic failure, refining a structural class:
/// `error wrong-turning refines L3 side-condition [timing] over cycle turning-leaf`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorClassDef {
    /// The failure name (the drama the scene voices).
    pub name: Ident,
    /// The token written after `refines` (`L3`, or whatever malformed thing).
    pub refines_token: String,
    /// The structural class, or [`None`] if `refines_token` names none.
    pub class: Option<StructuralClass>,
    /// The discriminant — the sub-case detail after the class token, verbatim.
    pub discriminant: String,
    /// The on-mic signal a performer voices when it fires (`signals <x>`).
    pub signal: Option<String>,
    /// The resource an `L6` failure spends against (`… on <resource>`).
    pub on_resource: Option<Ident>,
    /// The cycles a timing failure is seated on (`… over cycle <w1>, <w2>`).
    pub over_cycles: Vec<Ident>,
    /// Source span of the declaration.
    pub span: Span,
}

/// What a casting does: `effect invoke degrades acquaintance by 1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EffectDef {
    /// The form whose casting carries the effect.
    pub form: Ident,
    /// The effect body, verbatim.
    pub detail: String,
    /// Source span of the declaration.
    pub span: Span,
}

/// A compiled practice: the workbench's typed view of one declared law-set.
///
/// Everything that distinguishes one practice from another lives here as data
/// (charter koan 10); the generic checker and evaluator read this and nothing
/// else. It is self-contained for [`check_practice`]: the cycles that were in
/// scope at compile time are recorded on [`cycles_in_scope`](PracticeDef::cycles_in_scope)
/// so reference resolution needs no further environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PracticeDef {
    /// The practice name.
    pub name: Ident,
    /// The provenance marker the declaration carried, if any.
    pub marker: Option<ProvenanceMarker>,
    /// The layers this practice consumes (`use language.registers`).
    pub uses: Vec<String>,
    /// The declared finite sorts.
    pub sorts: Vec<SortDef>,
    /// The cast roles.
    pub roles: Vec<RoleDef>,
    /// The resources and their algebra laws.
    pub resources: Vec<ResourceDecl>,
    /// The laws of the practice.
    pub rules: Vec<RuleDef>,
    /// The side conditions.
    pub conditions: Vec<ConditionDef>,
    /// The declared failures.
    pub errors: Vec<ErrorClassDef>,
    /// The casting effects.
    pub effects: Vec<EffectDef>,
    /// The currency vocabulary a casting may spend in.
    pub costs: Vec<Ident>,
    /// The cycles in scope at compile time (those the document declares), against
    /// which cycle references are resolved. The engine seats no ambient cycles —
    /// a world's cyclic vocabulary is declared, never baked in (DL-0017).
    pub cycles_in_scope: BTreeSet<String>,
    /// Source span of the declaration.
    pub span: Span,
}

/// The compile-time environment a practice resolves its references against.
///
/// For PRAC-1 this is the set of cycles in scope — every cycle the document
/// declares, and nothing ambient. Build it with [`PracticeEnv::from_document`]
/// so a practice that seats a timing failure on a document-declared cycle
/// resolves; the engine seats no world's cycles of its own (DL-0017).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PracticeEnv {
    /// The cycle names in scope.
    pub cycles: BTreeSet<String>,
}

impl PracticeEnv {
    /// An empty environment — no cycles in scope, equal to [`Default`] so the two
    /// never disagree. A cycle resolves only where a document declares it.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The environment for a document: every cycle the document declares, and
    /// nothing ambient.
    #[must_use]
    pub fn from_document(doc: &Document) -> Self {
        let mut env = Self::default();
        for decl in &doc.decls {
            if let Decl::Cycle(w) = decl {
                env.cycles.insert(w.name.text.clone());
            }
        }
        env
    }
}

/// A practice-level well-formedness finding (spec §6).
///
/// Each carries a stable machine [`code`](PracticeFinding::code) and a
/// domain-spoken [`message`](PracticeFinding::message) — the two-channel error
/// model (charter koan 9). An empty finding list is a well-formed practice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "finding", rename_all = "kebab-case")]
pub enum PracticeFinding {
    /// A declared error refines no structural class (obligation 1).
    ErrorRefinesNothing {
        /// The offending failure name.
        error: String,
        /// The token it wrote after `refines`.
        refines: String,
        /// Source span.
        span: Span,
    },
    /// An error carries a structural class but no discriminant to distinguish its
    /// sub-case (spec §2: a declared error *adds* a discriminant).
    MissingDiscriminant {
        /// The offending failure name.
        error: String,
        /// Source span.
        span: Span,
    },
    /// A resource's declared law is outside the closed algebra (obligation 2).
    ResourceOutsideAlgebra {
        /// The offending resource name.
        resource: String,
        /// The law phrase as written.
        law: String,
        /// Source span.
        span: Span,
    },
    /// A role is annotated with a sort the practice does not declare, that is not
    /// a primitive kind, and is not a cycle in scope (obligation 3).
    UnboundSort {
        /// The role whose sort does not resolve.
        role: String,
        /// The unresolved sort name.
        sort: String,
        /// Source span.
        span: Span,
    },
    /// A failure or law cites a cycle that is not in scope (obligation 3).
    UnresolvedCycle {
        /// The unresolved cycle name.
        cycle: String,
        /// Where it was cited (`error wrong-turning`, `rule lay`, …).
        cited_by: String,
        /// Source span.
        span: Span,
    },
    /// An `L6 resource-violation` failure spends against a resource the practice
    /// does not declare (the discriminant checked against the class shape).
    ErrorResourceUnknown {
        /// The offending failure name.
        error: String,
        /// The undeclared resource it names.
        resource: String,
        /// Source span.
        span: Span,
    },
    /// A practice interior wrote a retired spelling; refuse it with its successor
    /// (DL-0016; the killed-form discipline, charter koan 7). The interior grammar
    /// mirrors the parser's top-level rule — an old word is never silently accepted
    /// nor read as unknown, it is named against the word that replaced it. The
    /// reference is still parsed leniently so nothing is dropped (koan 4).
    RenamedInteriorForm {
        /// The retired spelling as written (e.g. `over wheel`).
        old: String,
        /// The current spelling to write instead (e.g. `over cycle`).
        new: String,
        /// Where it was written (`error wrong-hour`, …).
        used_by: String,
        /// Source span.
        span: Span,
    },
}

impl PracticeFinding {
    /// A stable, machine-readable class for this finding (charter koan 9).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ErrorRefinesNothing { .. } => "practice.error-refines-nothing",
            Self::MissingDiscriminant { .. } => "practice.missing-discriminant",
            Self::ResourceOutsideAlgebra { .. } => "practice.resource-outside-algebra",
            Self::UnboundSort { .. } => "practice.unbound-sort",
            Self::UnresolvedCycle { .. } => "practice.unresolved-cycle",
            Self::ErrorResourceUnknown { .. } => "practice.error-resource-unknown",
            Self::RenamedInteriorForm { .. } => "practice.renamed-interior-form",
        }
    }

    /// The source span the finding points at.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::ErrorRefinesNothing { span, .. }
            | Self::MissingDiscriminant { span, .. }
            | Self::ResourceOutsideAlgebra { span, .. }
            | Self::UnboundSort { span, .. }
            | Self::UnresolvedCycle { span, .. }
            | Self::ErrorResourceUnknown { span, .. }
            | Self::RenamedInteriorForm { span, .. } => *span,
        }
    }

    /// A domain-spoken description of the finding (charter koan 9).
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::ErrorRefinesNothing { error, refines, .. } => format!(
                "the failure '{error}' refines '{refines}', which is not one of the seven \
                 structural classes L1–L7 — a declared error must refine one of them"
            ),
            Self::MissingDiscriminant { error, .. } => format!(
                "the failure '{error}' names a structural class but no discriminant — \
                 two failures may share a class only if a discriminant tells them apart"
            ),
            Self::ResourceOutsideAlgebra { resource, law, .. } => format!(
                "the resource '{resource}' is declared '{law}', which is not in the closed \
                 algebra (linear | linear conserved | affine bounded <cap> | graded monotone <order>)"
            ),
            Self::UnboundSort { role, sort, .. } => format!(
                "the role '{role}' is typed by sort '{sort}', which the practice does not \
                 declare and is neither a primitive kind nor a cycle in scope"
            ),
            Self::UnresolvedCycle {
                cycle, cited_by, ..
            } => format!(
                "{cited_by} is seated on the cycle '{cycle}', which is not in scope — \
                 declare it, or import the layer that carries it"
            ),
            Self::ErrorResourceUnknown {
                error, resource, ..
            } => format!(
                "the resource-violation failure '{error}' spends against '{resource}', \
                 which the practice does not declare as a resource"
            ),
            Self::RenamedInteriorForm {
                old, new, used_by, ..
            } => format!("{used_by} writes `{old}`, which was renamed — write `{new}`."),
        }
    }
}

impl PracticeDef {
    /// Compile a parsed practice declaration against an environment.
    ///
    /// The compile is total — it never fails and never panics; anything it
    /// cannot make sense of is recorded faithfully (a resource whose law is
    /// outside the algebra keeps [`kind: None`](ResourceDecl::kind); an error
    /// refining nothing keeps [`class: None`](ErrorClassDef::class)) and surfaces
    /// from [`check_practice`], never here. That two-phase split — total compile,
    /// then check — is what lets the acceptance fixtures compile and *then* be
    /// asserted well-formed.
    #[must_use]
    pub fn compile(decl: &PracticeDecl, env: &PracticeEnv) -> Self {
        let mut def = Self {
            name: decl.name.clone(),
            marker: decl.provenance.as_ref().map(|a| a.marker),
            uses: Vec::new(),
            sorts: Vec::new(),
            roles: Vec::new(),
            resources: Vec::new(),
            rules: Vec::new(),
            conditions: Vec::new(),
            errors: Vec::new(),
            effects: Vec::new(),
            costs: Vec::new(),
            cycles_in_scope: env.cycles.clone(),
            span: decl.name.span,
        };
        for item in reassemble(&decl.body) {
            def.absorb(&item);
        }
        def
    }

    fn absorb(&mut self, item: &RawItem) {
        let span = item.span;
        // `body` is the item text with its leading keyword removed.
        let body = item.text[item.keyword.len()..].trim();
        match item.keyword.as_str() {
            "use" => self.uses.push(body.to_owned()),
            "kind" => self.sorts.push(parse_sort(body, span)),
            "role" => {
                if let Some(role) = parse_role(body, span) {
                    self.roles.push(role);
                }
            }
            "resource" => self.resources.push(parse_resource(body, span)),
            "rule" => self.rules.push(parse_rule(body, span)),
            "condition" => {
                if let Some(cond) = parse_condition(body, span) {
                    self.conditions.push(cond);
                }
            }
            "error" => {
                if let Some(err) = parse_error(body, span) {
                    self.errors.push(err);
                }
            }
            "effect" => self.effects.push(parse_effect(body, span)),
            "cost" => self.costs.extend(split_pipe(body, span)),
            _ => {}
        }
    }

    /// The sorts a role annotation may resolve to: primitive kinds, this
    /// practice's declared sorts, and the cycles in scope.
    fn sort_scope(&self) -> BTreeSet<String> {
        let mut scope: BTreeSet<String> = PRIMITIVE_SORTS.iter().map(|s| (*s).to_owned()).collect();
        scope.extend(self.sorts.iter().map(|s| s.name.text.clone()));
        scope.extend(self.cycles_in_scope.iter().cloned());
        scope
    }
}

/// Compile every practice declared in a document against the document's cycles.
///
/// This is the workbench entry the acceptance gate drives: parse a `.dli` pack,
/// compile each practice, and the result hosts generically — no per-practice
/// kernel code (charter koan 10).
///
/// ```
/// use dli_syntax::parse_document;
/// use dli_ledger::practice::{compile_practices, check_practice};
///
/// let src = "\
/// scope magic
/// cycle turning-leaf(29) proposed
/// practice the-turning proposed {
///   kind figure = the-weir | the-drowned
///   role reader : entity requires standing(reader)
///   resource spread : affine bounded nine-leaves
///   error overspread refines L6 resource-violation on spread signals scatter
///   error wrong-turning refines L3 side-condition [timing] over cycle turning-leaf
///   cost epistemic | social
/// }
/// ";
/// let (doc, diags) = parse_document(src);
/// assert!(diags.is_empty());
/// let practices = compile_practices(&doc);
/// assert_eq!(practices.len(), 1);
/// // A disjoint, well-formed practice hosts with zero findings.
/// assert!(check_practice(&practices[0]).is_empty());
/// ```
#[must_use]
pub fn compile_practices(doc: &Document) -> Vec<PracticeDef> {
    let env = PracticeEnv::from_document(doc);
    doc.decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Practice(p) => Some(PracticeDef::compile(p, &env)),
            _ => None,
        })
        .collect()
}

/// Check a compiled practice for the well-formedness obligations of spec §6.
///
/// Returns every finding, in a stable order (errors, then resources, then roles,
/// then cycle references); an empty list is a well-formed practice. This is the
/// compile-time gate that runs before any casting: a magic system that would
/// need kernel support to host is caught here as a finding, not discovered in a
/// casting later.
///
/// ```
/// use dli_syntax::parse_document;
/// use dli_ledger::practice::{compile_practices, check_practice};
///
/// // A resource outside the closed algebra is caught.
/// let src = "\
/// scope magic
/// practice broken proposed {
///   resource mana : quadratic wobble
///   cost epistemic
/// }
/// ";
/// let (doc, _) = parse_document(src);
/// let findings = check_practice(&compile_practices(&doc)[0]);
/// assert_eq!(findings.len(), 1);
/// assert_eq!(findings[0].code(), "practice.resource-outside-algebra");
/// ```
#[must_use]
pub fn check_practice(def: &PracticeDef) -> Vec<PracticeFinding> {
    let mut findings = Vec::new();
    let declared_resources: BTreeSet<&str> =
        def.resources.iter().map(|r| r.name.text.as_str()).collect();

    // Obligation 1: every error refines a structural class, with a discriminant.
    for err in &def.errors {
        // The interior killed-form guard (DL-0016): a timing failure that seats
        // itself with the retired `over wheel` is refused with its successor, the
        // same discipline the parser applies to top-level keywords (koan 7).
        if uses_retired_cycle_form(&err.discriminant) {
            findings.push(PracticeFinding::RenamedInteriorForm {
                old: "over wheel".to_owned(),
                new: "over cycle".to_owned(),
                used_by: format!("error {}", err.name.text),
                span: err.span,
            });
        }
        match err.class {
            None => findings.push(PracticeFinding::ErrorRefinesNothing {
                error: err.name.text.clone(),
                refines: err.refines_token.clone(),
                span: err.span,
            }),
            Some(class) => {
                if err.discriminant.trim().is_empty() {
                    findings.push(PracticeFinding::MissingDiscriminant {
                        error: err.name.text.clone(),
                        span: err.span,
                    });
                }
                // The discriminant checked against the class shape: an L6 failure
                // spends against a resource, which must be one the practice declares.
                if class == StructuralClass::L6
                    && let Some(res) = &err.on_resource
                    && !declared_resources.contains(res.text.as_str())
                {
                    findings.push(PracticeFinding::ErrorResourceUnknown {
                        error: err.name.text.clone(),
                        resource: res.text.clone(),
                        span: err.span,
                    });
                }
            }
        }
    }

    // Obligation 2: resources use only the closed algebra.
    for res in &def.resources {
        if res.kind.is_none() {
            findings.push(PracticeFinding::ResourceOutsideAlgebra {
                resource: res.name.text.clone(),
                law: res.law_text.clone(),
                span: res.span,
            });
        }
    }

    // Obligation 3a: role sorts resolve.
    let scope = def.sort_scope();
    for role in &def.roles {
        if !scope.contains(&role.sort.text) {
            findings.push(PracticeFinding::UnboundSort {
                role: role.name.text.clone(),
                sort: role.sort.text.clone(),
                span: role.span,
            });
        }
    }

    // Obligation 3b: every cited cycle is in scope.
    for (cycle, cited_by, span) in cycle_references(def) {
        if !def.cycles_in_scope.contains(&cycle) {
            findings.push(PracticeFinding::UnresolvedCycle {
                cycle,
                cited_by,
                span,
            });
        }
    }

    findings
}

/// Collect every cycle reference in a practice with where it was cited: an
/// error's `over cycle …` clause, and any `phase(<cycle>)` in a rule or
/// condition body.
fn cycle_references(def: &PracticeDef) -> Vec<(String, String, Span)> {
    let mut refs = Vec::new();
    for err in &def.errors {
        for cycle in &err.over_cycles {
            refs.push((
                cycle.text.clone(),
                format!("error {}", err.name.text),
                err.span,
            ));
        }
    }
    for rule in &def.rules {
        if let Some(body) = &rule.requires {
            for cycle in phase_cycles(body) {
                refs.push((cycle, format!("rule {}", rule.form.text), rule.span));
            }
        }
    }
    for cond in &def.conditions {
        for cycle in phase_cycles(&cond.requires) {
            refs.push((cycle, format!("condition {}", cond.name.text), cond.span));
        }
    }
    refs
}

// --- interior parsing -------------------------------------------------------

/// One reassembled practice-block form: a leading keyword and its full text
/// (continuation lines re-joined).
struct RawItem {
    keyword: String,
    text: String,
    span: Span,
}

/// Fold captured clauses into forms: a clause opening with an interior keyword
/// begins a form; any other clause continues the form above it.
fn reassemble(body: &[Clause]) -> Vec<RawItem> {
    let mut items: Vec<RawItem> = Vec::new();
    for clause in body {
        let first = clause.text.split_whitespace().next().unwrap_or_default();
        if ITEM_KEYWORDS.contains(&first) {
            items.push(RawItem {
                keyword: first.to_owned(),
                text: clause.text.clone(),
                span: clause.span,
            });
        } else if let Some(last) = items.last_mut() {
            last.text.push(' ');
            last.text.push_str(&clause.text);
        }
    }
    items
}

fn parse_sort(body: &str, span: Span) -> SortDef {
    if let Some((name, variants)) = body.split_once(" = ") {
        return SortDef {
            name: ident(name.trim(), span),
            variants: split_pipe(variants, span),
            source: None,
            span,
        };
    }
    if let Some((name, source)) = body.split_once(" from ") {
        return SortDef {
            name: ident(name.trim(), span),
            variants: Vec::new(),
            source: Some(source.trim().to_owned()),
            span,
        };
    }
    SortDef {
        name: ident(first_word(body), span),
        variants: Vec::new(),
        source: None,
        span,
    }
}

fn parse_role(body: &str, span: Span) -> Option<RoleDef> {
    let (name, rest) = body.split_once(" : ")?;
    let (sort, requires) = match rest.split_once(" requires ") {
        Some((sort, req)) => (sort.trim(), Some(req.trim().to_owned())),
        None => (rest.trim(), None),
    };
    Some(RoleDef {
        name: ident(name.trim(), span),
        sort: ident(first_word(sort), span),
        requires,
        span,
    })
}

fn parse_resource(body: &str, span: Span) -> ResourceDecl {
    let (head, law) = match body.split_once(" : ") {
        Some((head, law)) => (head.trim(), law.trim()),
        None => (body.trim(), ""),
    };
    let (name, arg) = named_head(head, span);
    let (kind, cap, order, direction) = parse_resource_kind(law, span);
    ResourceDecl {
        name,
        arg: arg.into_iter().next(),
        kind,
        cap,
        order,
        direction,
        law_text: law.to_owned(),
        span,
    }
}

/// Read the law phrase after the `:` into a closed-algebra tag and its params.
fn parse_resource_kind(
    law: &str,
    span: Span,
) -> (
    Option<ResourceKind>,
    Option<String>,
    Vec<Ident>,
    GradeDirection,
) {
    let words: Vec<&str> = law.split_whitespace().collect();
    match words.as_slice() {
        ["linear"] => (
            Some(ResourceKind::Linear { conserved: false }),
            None,
            Vec::new(),
            GradeDirection::Decays,
        ),
        ["linear", "conserved"] => (
            Some(ResourceKind::Linear { conserved: true }),
            None,
            Vec::new(),
            GradeDirection::Decays,
        ),
        ["affine", "bounded", cap @ ..] if !cap.is_empty() => (
            Some(ResourceKind::AffineBounded),
            Some(cap.join(" ")),
            Vec::new(),
            GradeDirection::Decays,
        ),
        ["graded", "monotone", order @ ..] if !order.is_empty() => {
            // `whole > worn > husk` decays; the reverse operator would ripen.
            let direction = if order.contains(&"<") {
                GradeDirection::Grows
            } else {
                GradeDirection::Decays
            };
            let grades = order
                .iter()
                .filter(|t| **t != ">" && **t != "<")
                .map(|t| ident(t, span))
                .collect();
            (Some(ResourceKind::GradedMonotone), None, grades, direction)
        }
        _ => (None, None, Vec::new(), GradeDirection::Decays),
    }
}

fn parse_rule(body: &str, span: Span) -> RuleDef {
    let (head, requires) = match body.split_once(" requires ") {
        Some((head, req)) => (head.trim(), Some(req.trim().to_owned())),
        None => (body.trim(), None),
    };
    let (form, args) = named_head(head, span);
    RuleDef {
        form,
        args,
        requires,
        span,
    }
}

fn parse_condition(body: &str, span: Span) -> Option<ConditionDef> {
    let (name, requires) = body.split_once(" requires ")?;
    Some(ConditionDef {
        name: ident(name.trim(), span),
        requires: requires.trim().to_owned(),
        span,
    })
}

fn parse_error(body: &str, span: Span) -> Option<ErrorClassDef> {
    let (name, rest) = body.split_once(" refines ")?;
    let rest = rest.trim();
    let (class_token, detail) = match rest.split_once(char::is_whitespace) {
        Some((token, detail)) => (token, detail.trim()),
        None => (rest, ""),
    };
    // Split off a trailing `signals <signal>` from the discriminant detail.
    let (discriminant, signal) = match detail.split_once(" signals ") {
        Some((disc, sig)) => (disc.trim().to_owned(), Some(sig.trim().to_owned())),
        None => (detail.to_owned(), None),
    };
    let on_resource = discriminant
        .split_once(" on ")
        .map(|(_, rest)| ident(first_word(rest), span));
    let over_cycles = parse_over_cycles(&discriminant, span);
    Some(ErrorClassDef {
        name: ident(name.trim(), span),
        refines_token: class_token.to_owned(),
        class: StructuralClass::from_token(class_token),
        discriminant,
        signal,
        on_resource,
        over_cycles,
        span,
    })
}

/// The `over cycle <c1>, <c2>` clause naming the cycles a timing failure is
/// seated on. Both the current `over cycle` and the retired `over wheel` spelling
/// are read so nothing is dropped (koan 4); the retired form is separately
/// diagnosed by [`check_practice`] (DL-0016), so the reference resolves either way
/// and the writer is still told to update it.
fn parse_over_cycles(discriminant: &str, span: Span) -> Vec<Ident> {
    for marker in ["over cycle ", "over wheel "] {
        if let Some(at) = discriminant.find(marker) {
            return discriminant[at + marker.len()..]
                .split(',')
                .map(|w| ident(first_word(w.trim()), span))
                .filter(|i| !i.text.is_empty())
                .collect();
        }
    }
    Vec::new()
}

/// Whether a discriminant used the retired `over wheel` spelling (DL-0016).
fn uses_retired_cycle_form(discriminant: &str) -> bool {
    discriminant.contains("over wheel ") || discriminant.trim_end().ends_with("over wheel")
}

fn parse_effect(body: &str, span: Span) -> EffectDef {
    let (form, detail) = match body.split_once(char::is_whitespace) {
        Some((form, detail)) => (form, detail.trim().to_owned()),
        None => (body, String::new()),
    };
    EffectDef {
        form: ident(form, span),
        detail,
        span,
    }
}

// --- token helpers ----------------------------------------------------------

/// Split a `name(arg, …)` head into its name and argument idents through the
/// shared [`parse_clause_atom`]; a bare `name` yields no arguments.
fn named_head(head: &str, span: Span) -> (Ident, Vec<Ident>) {
    let clause = Clause {
        text: head.to_owned(),
        span,
    };
    if let Some(atom) = parse_clause_atom(&clause) {
        let args = atom
            .args
            .iter()
            .map(|term| ident(&term_text(term), span))
            .collect();
        return (ident(&atom.pred.text, span), args);
    }
    (ident(first_word(head), span), Vec::new())
}

/// The forward reference of `phase(<cycle>)` calls in a body, in order.
fn phase_cycles(body: &str) -> Vec<String> {
    let mut cycles = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find("phase(") {
        let after = &rest[at + "phase(".len()..];
        if let Some(close) = after.find(')') {
            let inner = after[..close].trim();
            let cycle = inner.split([',', ' ']).next().unwrap_or_default().trim();
            if !cycle.is_empty() {
                cycles.push(cycle.to_owned());
            }
            rest = &after[close + 1..];
        } else {
            break;
        }
    }
    cycles
}

/// Split a `a | b | c` list into its idents.
fn split_pipe(text: &str, span: Span) -> Vec<Ident> {
    text.split('|')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| ident(first_word(s), span))
        .collect()
}

fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or_default()
}

fn ident(text: &str, span: Span) -> Ident {
    Ident {
        text: text.to_owned(),
        span,
    }
}

fn term_text(term: &Term) -> String {
    match term {
        Term::Var(id) | Term::Sym(id) => id.text.clone(),
        Term::Str(s) => s.clone(),
        Term::Int(n) => n.to_string(),
        Term::Time(_) => String::new(),
    }
}
