//! The practice declaration layer.
//!
//! A **practice** — a magic system, a divination game, a proof discipline — is
//! declared entirely as data (`docs/spec/04-practices.md`). This module reads the
//! typed [`PracticeDef`] the front end structured into the practice body (LS-9)
//! and checks it for the three compile-time well-formedness obligations of
//! spec §6, before any casting is run:
//!
//! 1. **Every declared error refines a structural class** — one of `L1`..`L7`,
//!    the seven the checker knows ([`StructuralClass`]). An error that names no
//!    class, or one outside the range, does not compile.
//! 2. **Resources use only the closed algebra** — a [`ResourceDecl`] whose kind
//!    is not one of [`ResourceKind`](crate::resource::ResourceKind)'s members is
//!    a compile error; there is no open resource vocabulary.
//! 3. **Declarations resolve against the practice's sorts** — a role annotated
//!    with a sort the practice does not declare (nor a primitive kind nor a
//!    cycle), and a failure or law that cites a cycle not in scope, do not
//!    compile.
//!
//! # Reader, not parser (LS-9)
//!
//! The typed [`PracticeDef`] and its sub-structs now live in `dli-ast`; the
//! ~1k-line interior parser that folded the body's `Clause`s into forms moved
//! behind the front end as [`dli_syntax::structure_practice_body`]. This module
//! is a *reader*: [`compile_practice`] takes the front end's typed structure and
//! overlays the document's cycle scope, and [`check_practice`] runs the
//! well-formedness obligations over it. There is no `dli_syntax` dependency on
//! this path — the parser is the single producer of the typed interior.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::decl::{Decl, Document, PracticeDecl};
use dli_ast::span::Span;

// The typed practice interior lives in `dli-ast` (LS-9) so the front-end producer
// (`dli_syntax`) and this reader share it without a dependency cycle. Re-exported
// here so `dli_ledger::practice::…` paths keep resolving for downstream readers.
pub use dli_ast::practice::{
    ConditionDef, EffectDef, ErrorClassDef, PracticeBody, PracticeDef, ResourceDecl, RoleDef,
    RuleDef, SortDef, StructuralClass,
};

/// The primitive entity kinds a role's sort may name without a `sort`
/// declaration — the world-entity sorts of `docs/spec/01-syntax.md`.
pub const PRIMITIVE_SORTS: &[&str] = &[
    "entity", "artifact", "place", "person", "thing", "event", "era",
];

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

/// Compile a parsed practice declaration against an environment.
///
/// The typed [`PracticeDef`] was already structured by the front end
/// ([`dli_syntax::structure_practice_body`]); the compile is the thin overlay
/// that seats the practice in its document's cycle scope. It is total — it never
/// fails and never panics; anything the front end could not make sense of is
/// recorded faithfully (a resource whose law is outside the algebra keeps
/// [`kind: None`](ResourceDecl::kind); an error refining nothing keeps
/// [`class: None`](ErrorClassDef::class)) and surfaces from [`check_practice`],
/// never here. That two-phase split — total compile, then check — is what lets
/// the acceptance fixtures compile and *then* be asserted well-formed.
#[must_use]
pub fn compile_practice(decl: &PracticeDecl, env: &PracticeEnv) -> PracticeDef {
    // The front end produced the typed interior with an empty `cycles_in_scope`;
    // seat it in the document's cyclic vocabulary here (DL-0017). Everything else
    // is read straight from the structured body.
    let mut def = decl.body.def.as_ref().clone();
    def.cycles_in_scope.clone_from(&env.cycles);
    def
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
            Decl::Practice(p) => Some(compile_practice(p, &env)),
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
    let scope = sort_scope(def);
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

/// The sorts a role annotation may resolve to: primitive kinds, this practice's
/// declared sorts, and the cycles in scope.
fn sort_scope(def: &PracticeDef) -> BTreeSet<String> {
    let mut scope: BTreeSet<String> = PRIMITIVE_SORTS.iter().map(|s| (*s).to_owned()).collect();
    scope.extend(def.sorts.iter().map(|s| s.name.text.clone()));
    scope.extend(def.cycles_in_scope.iter().cloned());
    scope
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

/// Whether a discriminant used the retired `over wheel` spelling (DL-0016).
fn uses_retired_cycle_form(discriminant: &str) -> bool {
    discriminant.contains("over wheel ") || discriminant.trim_end().ends_with("over wheel")
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
