//! Names as typed derivations, and their compiler (`LANG-1`/`LANG-2`).
//!
//! A name declaration cites its register and its composition, so "register-true"
//! stops being a reviewer's impression and becomes a compile verdict
//! (05-language-layer §2). This module compiles a `name` declaration into a
//! [`NameDef`] — a typed derivation — and a `lexeme` into a [`WordDef`]. The
//! three obligations a name must discharge (slot sequence, role membership,
//! phonotactics) are checked in [`super::derivation`]; here we only lift the
//! surface into structure.
//!
//! # The name-interior grammar (owned here)
//!
//! A `name` header is `<name> ["label"] <kind> in <register>`; its body is
//! captured as [`Clause`](dli_ast::clause::Clause) lines:
//!
//! ```text
//! = <role> <morpheme> [ + <role> <morpheme> … ]   # the composition
//! bearer <entity> [<provenance>]                  # who/what bears the name
//! <provenance>                                    # a standalone provenance line
//! ```
//!
//! Provenance may ride the header line or any body line; the compiler takes the
//! first it finds (header first), so `name … ratified D-1` and a trailing
//! `bearer x ratified D-1` are equivalent.

use dli_ast::annot::Annotation;
use dli_ast::decl::{NameDecl, WordDecl};
use dli_ast::language::NameItem;
use dli_ast::span::Span;
use dli_core::ann::Ann;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::Provenance;

use super::dialect::Compiler;

/// One composition step: a slot role filled by a named morpheme (05 §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Composition {
    /// The slot role (e.g. `given-proto`).
    pub role: String,
    /// The morpheme surface filling it (e.g. `en`).
    pub morpheme: String,
}

/// A name as a typed derivation citing its register and composition (05 §2).
///
/// The three obligations that compile together on every name — the slot sequence
/// matching a `name-kind` pattern, each morpheme existing under its claimed role,
/// and the composed surface passing the register's phonotactics — are verdicts
/// [`super::check`] produces, not notes a reviewer writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NameDef {
    /// The name identifier (e.g. `linet`).
    pub name: String,
    /// The rendered label (e.g. `Linet`), if written.
    pub label: Option<String>,
    /// The name kind (e.g. `person.given`, `place`).
    pub kind: String,
    /// The register the name derives in.
    pub register: String,
    /// The composition — the slot sequence, in order.
    pub composition: Vec<Composition>,
    /// The entity that bears the name, if written.
    pub bearer: Option<String>,
    /// The `(P⁺, P⁻)` annotation gating the name.
    pub ann: Ann,
    /// How the name entered.
    pub provenance: Provenance,
    /// Rendered ledger citations, when decided.
    pub citations: Vec<String>,
    /// Source location.
    pub span: Span,
}

impl NameDef {
    /// The composed surface form: the morpheme surfaces concatenated in order.
    ///
    /// When no composition was written the label (letters only, lowercased) is
    /// used, so a bare declared name still has a surface to check.
    #[must_use]
    pub fn surface(&self) -> String {
        if self.composition.is_empty() {
            let source = self.label.as_deref().unwrap_or(&self.name);
            source
                .chars()
                .filter(|c| c.is_alphabetic())
                .flat_map(char::to_lowercase)
                .collect()
        } else {
            self.composition
                .iter()
                .map(|c| c.morpheme.as_str())
                .collect()
        }
    }

    /// The slot-role sequence the composition claims.
    #[must_use]
    pub fn roles(&self) -> Vec<String> {
        self.composition.iter().map(|c| c.role.clone()).collect()
    }

    /// A trial [`NameDef`] over a bare candidate string in a named register — an
    /// unprovenanced, uncomposed name whose surface *is* the candidate.
    ///
    /// The vet of a bare candidate ([`super::vet_candidate_in_dialect`]) can only
    /// discharge the surface-only obligation — the register's phonotactic frame —
    /// because slot-sequence and role-membership are claims about a *composition*
    /// the candidate does not carry. This constructor exists for that vet; it is
    /// deliberately not a compiled canon name (its provenance is
    /// [`Provenance::Unprovenanced`], so it is unassertable by construction).
    #[must_use]
    pub fn trial(candidate: &str, register: &str) -> Self {
        Self {
            name: candidate.to_owned(),
            label: Some(candidate.to_owned()),
            kind: String::new(),
            register: register.to_owned(),
            composition: Vec::new(),
            bearer: None,
            ann: Ann::default(),
            provenance: Provenance::Unprovenanced,
            citations: Vec::new(),
            span: Span::synthetic(),
        }
    }
}

/// A coined lexeme with its composition and status (05 §4).
///
/// The lexeme is the loan-word layer the spoken-oath practice's `binder` sort
/// draws from; one fabric means the binder layer *is* the loan-word layer.
///
/// Note: the v0 surface parser captures only the lexeme's name and provenance;
/// its `= <role> <morpheme>` composition and label are not yet surfaced by the
/// front-end, so [`composition`](WordDef::composition) is populated only when
/// the parser hands them over. This is a front-end seam, not a model limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WordDef {
    /// The lexeme identifier.
    pub name: String,
    /// The rendered label, if surfaced.
    pub label: Option<String>,
    /// The register it belongs to, if surfaced.
    pub register: Option<String>,
    /// The composition, if surfaced.
    pub composition: Vec<Composition>,
    /// The `(P⁺, P⁻)` annotation gating the lexeme.
    pub ann: Ann,
    /// How the lexeme entered.
    pub provenance: Provenance,
    /// Rendered ledger citations, when decided.
    pub citations: Vec<String>,
    /// Source location.
    pub span: Span,
}

/// Compile a `name` declaration into a [`NameDef`] (and any compile findings).
///
/// Lenient by design: a malformed head or missing composition is not raised
/// here; [`super::check`] is the single place naming verdicts surface.
pub(crate) fn compile_name(
    compiler: &mut Compiler,
    decl: &NameDecl,
    pack: &str,
) -> (NameDef, Vec<super::LangFinding>) {
    let kind = decl.head.first().cloned().unwrap_or_default();
    let register = value_after(&decl.head, "in").unwrap_or_default();

    // Composition and bearer may ride the header line (captured in `head`) or a
    // continuation line (captured in `body`); the header form wins.
    let mut composition = head_composition(&decl.head).unwrap_or_default();
    let mut bearer = value_after(&decl.head, "bearer");
    let mut body_provenance: Option<Annotation> = None;

    // The parser (`dli_syntax::structure_name_body`) already classified each
    // body clause; apply the same header-first precedence (a header composition
    // or bearer wins; the first body provenance wins).
    for clause in &decl.body.clauses {
        match &clause.item {
            NameItem::Composition(steps) => {
                if composition.is_empty() && !steps.is_empty() {
                    composition = steps
                        .iter()
                        .map(|step| Composition {
                            role: step.role.clone(),
                            morpheme: step.morpheme.clone(),
                        })
                        .collect();
                }
            }
            NameItem::Bearer { entity, provenance } => {
                if bearer.is_none() {
                    bearer.clone_from(entity);
                }
                if body_provenance.is_none() {
                    body_provenance.clone_from(provenance);
                }
            }
            NameItem::Provenance(ann) => {
                if body_provenance.is_none() {
                    body_provenance = Some(ann.clone());
                }
            }
            NameItem::Other => {}
        }
    }

    let provenance = decl.provenance.clone().or(body_provenance);
    let (ann, kind_prov, citations) = compiler.compile_ann(provenance.as_ref(), pack);

    let def = NameDef {
        name: decl.name.text.clone(),
        label: decl.label.clone(),
        kind,
        register,
        composition,
        bearer,
        ann,
        provenance: kind_prov,
        citations,
        span: decl.name.span,
    };
    (def, Vec::new())
}

/// Compile a `lexeme` declaration into a [`WordDef`].
pub(crate) fn compile_lexeme(compiler: &mut Compiler, decl: &WordDecl, pack: &str) -> WordDef {
    let (ann, provenance, citations) = compiler.compile_ann(decl.provenance.as_ref(), pack);
    let composition = decl
        .name_parts
        .iter()
        .map(|m| Composition {
            role: String::new(),
            morpheme: m.text.clone(),
        })
        .collect();
    WordDef {
        name: decl.name.text.clone(),
        label: None,
        register: None,
        composition,
        ann,
        provenance,
        citations,
        span: decl.name.span,
    }
}

/// The header token immediately after `key` (`in elder-tongue-fringe` → `elder-tongue-fringe`).
fn value_after(head: &[String], key: &str) -> Option<String> {
    head.iter()
        .position(|t| t == key)
        .and_then(|i| head.get(i + 1))
        .cloned()
}

/// Parse an inline composition from the header tokens: the words after `=`, up to
/// a trailing `bearer`, split into `role morpheme` steps by `+`.
fn head_composition(head: &[String]) -> Option<Vec<Composition>> {
    let eq = head.iter().position(|t| t == "=")?;
    let end = head[eq + 1..]
        .iter()
        .position(|t| t == "bearer")
        .map_or(head.len(), |b| eq + 1 + b);
    let words: Vec<&str> = head[eq + 1..end].iter().map(String::as_str).collect();
    compose_words(&words)
}

/// Split a flat word list `[role morpheme (+ role morpheme)*]` into steps.
///
/// Reads a name declaration's typed `head` tokens (via [`head_composition`]) —
/// not clause text — so it needs no lexer (LS-9). The body composition is
/// pre-parsed by the front end into [`NameItem::Composition`].
fn compose_words(words: &[&str]) -> Option<Vec<Composition>> {
    let mut steps = Vec::new();
    for group in words.split(|w| *w == "+") {
        if let [role, morpheme] = group {
            steps.push(Composition {
                role: (*role).to_owned(),
                morpheme: (*morpheme).to_owned(),
            });
        }
    }
    (!steps.is_empty()).then_some(steps)
}
