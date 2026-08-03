//! Dialects, name-parts, and the compiled language registry (`LANG-1`).
//!
//! A **dialect** is a claim about a linguistic zone of the world: its
//! phonotactics, its name grammars, and its stock of cited name-parts
//! (05-language-layer §1). It is world knowledge, so it lives in the fabric like
//! any other lore — statused, cited, and forkable. This module compiles the
//! parsed surface (`dialect`, `name-part`, `name`, `word`, `term`, `banned`,
//! `hypothesis`, `ratify` declarations) into a typed [`LanguageModel`]: the
//! name-space the kernel now owns.
//!
//! # The dialect-interior grammar (owned here)
//!
//! A dialect body is captured by the parser as normalized
//! [`Clause`](dli_ast::clause::Clause) lines; this module structures each one.
//! The recognized interior forms are:
//!
//! ```text
//! basis [attested] "<real-world basis>"      # attestation seam (§7); side-band
//! vowels <v> <v> …                           # the vowel inventory
//! endings <e> <e> …                          # characteristic name-endings (a `-` prefix is stripped)
//! foreign <c> <c> …                          # clusters that mark a form as not-ours
//! cluster-max <n>                            # the consonant-cluster ceiling
//! sayability-max <n>                         # the per-word syllable ceiling
//! stress <initial|penultimate|final>         # the stress rule
//! name-kind <kind> = <role> <role> … [ | <role> … ]   # a slot grammar (alternatives split by `|`)
//! name-part <surface> ["gloss"] [role <role>] [gender <g>] <provenance>
//! ```
//!
//! Everything a dialect holds is a residency change from the consuming repo's
//! `registers.yaml`, not a redesign — one linguistic-zone profile becomes one
//! scope (05 §1). The composition *weights* the legacy generator sampled do
//! **not** migrate: they are proposal policy, not world fact, and stay
//! factory-side.

use dli_ast::annot::{Annotation, CondExpr, ProvenanceMarker};
use dli_ast::decl::{Decl, DialectDecl, Document, NamePartDecl, RatifyDecl};
use dli_ast::language::{DialectClause, DialectItem};
use dli_ast::span::Span;
use dli_core::ann::{Ann, Status, Theory};
use dli_core::atom::{Atom, Interner};
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::Provenance;

use super::name::{NameDef, WordDef, compile_lexeme, compile_name};

/// The stress rule a register places on its surface forms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Stress {
    /// Stress on the first syllable.
    Initial,
    /// Stress on the second-to-last syllable.
    Penultimate,
    /// Stress on the last syllable.
    Final,
    /// No stress rule was declared.
    #[default]
    Unspecified,
}

impl Stress {
    /// The [`Stress`] a surface keyword names, or [`Stress::Unspecified`].
    #[must_use]
    pub fn from_keyword(word: &str) -> Self {
        match word {
            "initial" => Self::Initial,
            "penultimate" => Self::Penultimate,
            "final" => Self::Final,
            _ => Self::Unspecified,
        }
    }
}

/// The gender a morpheme (and the names built from it) carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Gender {
    /// Grammatically/socially male.
    Male,
    /// Grammatically/socially female.
    Female,
    /// Explicitly unmarked / unisex.
    Neutral,
}

impl Gender {
    /// The [`Gender`] a surface keyword names, if any.
    #[must_use]
    pub fn from_keyword(word: &str) -> Option<Self> {
        match word {
            "male" => Some(Self::Male),
            "female" => Some(Self::Female),
            "neutral" | "unisex" => Some(Self::Neutral),
            _ => None,
        }
    }
}

/// The phonotactic frame of a register — the world-facts a composed surface form
/// is checked against (05 §1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Phonotactics {
    /// The characteristic vowel inventory (lowercased, as written).
    pub vowels: Vec<String>,
    /// The characteristic name-endings (a leading `-` is stripped).
    pub endings: Vec<String>,
    /// Clusters that mark a form as *not* of this tongue (05 §2 register-fit).
    pub foreign: Vec<String>,
    /// The maximum admissible consonant-cluster length. `0` means unset.
    pub cluster_max: usize,
    /// The maximum admissible syllable count for a single word — the sayability
    /// ceiling. `0` means unset, so the generic default applies.
    pub sayability_max: usize,
    /// The stress rule.
    pub stress: Stress,
}

/// A morpheme: a cited fact about a tongue, with its role and provenance.
///
/// Each morpheme is world lore with provenance — statused and forkable like any
/// fact (05 §1). Its [`ann`](NamePartDef::ann) gates it: a `proposed` morpheme is
/// merely-possible until a ruling retires its hypothesis; a rejected one valuates
/// false and every name derived through it dies into lineage (05 §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NamePartDef {
    /// The surface form of the morpheme (e.g. `en`, `wern`).
    pub surface: String,
    /// The gloss, if written.
    pub gloss: Option<String>,
    /// The slot role it fills (e.g. `given-proto`, `place-generic`).
    pub role: String,
    /// The gender it carries, if declared.
    pub gender: Option<Gender>,
    /// The register it belongs to (empty for a top-level, register-agnostic
    /// morpheme in the shared pool).
    pub register: String,
    /// The `(P⁺, P⁻)` annotation gating the morpheme.
    pub ann: Ann,
    /// How the morpheme entered.
    pub provenance: Provenance,
    /// Rendered ledger citations, when decided.
    pub citations: Vec<String>,
    /// Source location.
    pub span: Span,
}

/// A name-kind slot grammar: the alternative slot-role sequences a name kind
/// admits (05 §2).
///
/// `person.given = given-proto given-deutero | given-simple` compiles to two
/// [`patterns`](NameKind::patterns): `[given-proto, given-deutero]` and
/// `[given-simple]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NameKind {
    /// The name kind this grammar governs (e.g. `person.given`, `place`).
    pub kind: String,
    /// The admissible slot-role sequences (alternatives).
    pub patterns: Vec<Vec<String>>,
}

/// A compiled register — a claim about a linguistic zone of the world (05 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DialectDef {
    /// The register name (e.g. `cymric-fringe`).
    pub name: String,
    /// The display label, if written.
    pub label: Option<String>,
    /// The real-world basis, an **attestation** (05 §7): a claim about *our*
    /// world, graded and side-band, never a variable in a support polynomial.
    pub basis: Option<String>,
    /// Whether the basis was written with the `attested` grade word.
    pub basis_attested: bool,
    /// The phonotactic frame.
    pub phonotactics: Phonotactics,
    /// The name-kind slot grammars.
    pub name_kinds: Vec<NameKind>,
    /// The morphemes declared inside this register.
    pub morphemes: Vec<NamePartDef>,
    /// The `(P⁺, P⁻)` annotation gating the register.
    pub ann: Ann,
    /// How the register entered.
    pub provenance: Provenance,
    /// Rendered ledger citations, when decided.
    pub citations: Vec<String>,
    /// Source location.
    pub span: Span,
}

impl DialectDef {
    /// The name-kind grammar for `kind`, if the register declares one.
    #[must_use]
    pub fn name_kind(&self, kind: &str) -> Option<&NameKind> {
        self.name_kinds.iter().find(|nk| nk.kind == kind)
    }

    /// The morpheme with `surface` filling `role`, searched within this register.
    #[must_use]
    pub fn morpheme(&self, surface: &str, role: &str) -> Option<&NamePartDef> {
        self.morphemes
            .iter()
            .find(|m| m.surface == surface && m.role == role)
    }
}

/// A coined lexicon term with status and citation (05 §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TermDef {
    /// The term identifier.
    pub name: String,
    /// The rendered label, if written.
    pub label: Option<String>,
    /// The `(P⁺, P⁻)` annotation gating the term.
    pub ann: Ann,
    /// How the term entered.
    pub provenance: Provenance,
    /// Rendered ledger citations, when decided.
    pub citations: Vec<String>,
    /// Source location.
    pub span: Span,
}

/// A cited lexicon ban: the allegory's lexical bans are lore with provenance,
/// not config entries (05 §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BanDef {
    /// The banned word.
    pub word: String,
    /// The ruling that banned it, if cited.
    pub decision: Option<String>,
    /// The reason, if written.
    pub reason: Option<String>,
    /// Source location.
    pub span: Span,
}

/// A named speculative premise: one hypothesis variable per proposal bundle
/// (05 §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Hypothesis {
    /// The hypothesis name (e.g. `h-cymric-loans`).
    pub name: String,
    /// The statement text, if written.
    pub statement: Option<String>,
}

/// The compiled language registry: the kernel's name-space (05 §6).
///
/// This is the mirror's successor. The consuming repo's `ratified_names.yaml` was
/// a hand-synced copy of world knowledge; the compiled [`LanguageModel`] *is* the
/// name-space, and anything a legacy consumer needs comes from a query over it
/// (see [`super::view`]), never a second writable copy.
///
/// Standing is derived, never stored (charter koan 1): each entry carries an
/// [`Ann`], and [`assertable`](LanguageModel::assertable) reads it under the
/// registry's [`valuation`](LanguageModel::valuation) — the valuation that rules
/// every cited decision true and leaves open hypotheses open, so a name gated by
/// an unratified bundle is merely-possible *by construction* (05 §5).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LanguageModel {
    /// The shared interner the support polynomials index into.
    pub interner: Interner,
    /// The valuation: every cited decision ruled true, plus `ratify` assignments.
    pub valuation: Valuation,
    /// The compiled registers.
    pub registers: Vec<DialectDef>,
    /// Top-level, register-agnostic morphemes (the shared pool a name in any
    /// register may draw a loan-morpheme from).
    pub morphemes: Vec<NamePartDef>,
    /// The compiled names (typed derivations).
    pub names: Vec<NameDef>,
    /// The compiled lexemes.
    pub lexemes: Vec<WordDef>,
    /// The compiled lexicon terms.
    pub terms: Vec<TermDef>,
    /// The compiled lexical bans.
    pub bans: Vec<BanDef>,
    /// The proposal-bundle hypotheses.
    pub hypotheses: Vec<Hypothesis>,
}

impl LanguageModel {
    /// The register named `name`, if compiled.
    #[must_use]
    pub fn register(&self, name: &str) -> Option<&DialectDef> {
        self.registers.iter().find(|r| r.name == name)
    }

    /// Resolve the morpheme with `surface` filling `role` for a name in
    /// `register`: the register's own morphemes first, then the shared pool.
    #[must_use]
    pub fn resolve_morpheme(
        &self,
        register: &str,
        surface: &str,
        role: &str,
    ) -> Option<&NamePartDef> {
        self.register(register)
            .and_then(|r| r.morpheme(surface, role))
            .or_else(|| {
                self.morphemes
                    .iter()
                    .find(|m| m.surface == surface && m.role == role)
            })
    }

    /// Whether a claim with annotation `ann` is assertable in the certain layer.
    ///
    /// A claim is assertable exactly when it is `Certain` under the registry's
    /// valuation: a ledger-grounded claim (its decision ruled true) is; a proposal
    /// gated by a still-open hypothesis is not.
    #[must_use]
    pub fn assertable(&self, ann: &Ann) -> bool {
        ann.status(&self.valuation, &Theory::new()) == Status::Certain
    }

    /// Compile a set of parsed documents into a [`LanguageOutcome`].
    ///
    /// Pure and total: every language declaration is compiled leniently into the
    /// registry; structural violations are not raised here but by
    /// [`super::check`], the single place naming verdicts surface.
    ///
    /// ```
    /// use dli_world::language::LanguageModel;
    /// use dli_syntax::parse_document;
    ///
    /// let src = "scope language\n\
    ///            dialect fen \"the fen tongue\" ratified D-1 {\n\
    ///            cluster-max 2\n\
    ///            name-kind place = place-qualifier place-generic\n\
    ///            name-part cors \"bog\" role place-qualifier ratified D-1\n\
    ///            name-part wern \"marsh\" role place-generic ratified D-1\n\
    ///            }\n";
    /// let (doc, _diags) = parse_document(src);
    /// let out = LanguageModel::from_documents(&[doc]);
    /// let reg = out.model.register("fen").expect("dialect compiled");
    /// assert_eq!(reg.morphemes.len(), 2);
    /// assert_eq!(reg.phonotactics.cluster_max, 2);
    /// ```
    #[must_use]
    pub fn from_documents(docs: &[Document]) -> LanguageOutcome {
        let mut compiler = Compiler::default();
        for doc in docs {
            let pack = doc
                .scope
                .as_ref()
                .map_or_else(String::new, |header| header.name.text.clone());
            for decl in &doc.decls {
                compiler.process(decl, &pack);
            }
        }
        compiler.finish()
    }
}

/// The outcome of compiling documents into a [`LanguageModel`]: the registry plus
/// any compile-time findings (malformed input the compiler could not lift).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LanguageOutcome {
    /// The compiled registry.
    pub model: LanguageModel,
    /// Compile-time findings.
    pub findings: Vec<super::LangFinding>,
}

/// The mutable state threaded through one compile.
///
/// It owns the interner and valuation the finished [`LanguageModel`] carries, so
/// support polynomials and their satisfying valuation are built together.
#[derive(Debug, Default)]
pub(crate) struct Compiler {
    model: LanguageModel,
    findings: Vec<super::LangFinding>,
    bundle_counter: u32,
}

impl Compiler {
    fn process(&mut self, decl: &Decl, pack: &str) {
        match decl {
            Decl::Dialect(reg) => self.register(reg, pack),
            Decl::NamePart(morph) => {
                let m = self.morpheme(morph, "", pack);
                self.model.morphemes.push(m);
            }
            Decl::Name(name) => {
                let (def, findings) = compile_name(self, name, pack);
                self.findings.extend(findings);
                self.model.names.push(def);
            }
            Decl::Word(lexeme) => {
                let ldef = compile_lexeme(self, lexeme, pack);
                self.model.lexemes.push(ldef);
            }
            Decl::Term(term) => {
                let (ann, provenance, citations) = self.compile_ann(term.provenance.as_ref(), pack);
                self.model.terms.push(TermDef {
                    name: term.name.text.clone(),
                    label: term.label.clone(),
                    ann,
                    provenance,
                    citations,
                    span: term.name.span,
                });
            }
            Decl::Banned(ban) => {
                let decision = ban
                    .provenance
                    .as_ref()
                    .and_then(|a| a.citations.first())
                    .map(|c| c.id.text.clone());
                self.model.bans.push(BanDef {
                    word: ban.word.text.clone(),
                    decision,
                    reason: ban.reason.clone(),
                    span: ban.word.span,
                });
            }
            Decl::Hypothesis(hyp) => {
                self.model.hypotheses.push(Hypothesis {
                    name: hyp.name.text.clone(),
                    statement: hyp.statement.clone(),
                });
            }
            Decl::Ratify(ratify) => self.ratify(ratify),
            _ => {}
        }
    }

    fn register(&mut self, decl: &DialectDecl, pack: &str) {
        let (ann, provenance, citations) = self.compile_ann(decl.provenance.as_ref(), pack);
        let mut reg = DialectDef {
            name: decl.name.text.clone(),
            label: decl.label.clone(),
            basis: None,
            basis_attested: false,
            phonotactics: Phonotactics::default(),
            name_kinds: Vec::new(),
            morphemes: Vec::new(),
            ann,
            provenance,
            citations,
            span: decl.name.span,
        };
        for clause in &decl.body.clauses {
            self.register_clause(&mut reg, clause, pack);
        }
        self.model.registers.push(reg);
    }

    /// Read one typed interior clause of a register body into the [`DialectDef`].
    ///
    /// The parser (`dli_syntax::structure_dialect_body`) already classified the
    /// clause and pre-derived its values; this applies them, preserving the exact
    /// precedence of the old string-body compile (a later `cluster-max` line
    /// overrides an earlier one only when it named a count, and so on).
    fn register_clause(&mut self, reg: &mut DialectDef, clause: &DialectClause, pack: &str) {
        match &clause.item {
            DialectItem::Basis { attested, text } => {
                reg.basis_attested = *attested;
                reg.basis.clone_from(text);
            }
            DialectItem::Vowels(vowels) => reg.phonotactics.vowels.clone_from(vowels),
            DialectItem::Endings(endings) => reg.phonotactics.endings.clone_from(endings),
            DialectItem::Foreign(foreign) => reg.phonotactics.foreign.clone_from(foreign),
            DialectItem::ClusterMax(n) => {
                if let Some(n) = n {
                    reg.phonotactics.cluster_max = *n;
                }
            }
            DialectItem::SayabilityMax(n) => {
                if let Some(n) = n {
                    reg.phonotactics.sayability_max = *n;
                }
            }
            DialectItem::Stress(word) => {
                if let Some(word) = word {
                    reg.phonotactics.stress = Stress::from_keyword(word);
                }
            }
            DialectItem::NameKind(nk) => {
                if let Some(nk) = nk {
                    reg.name_kinds.push(NameKind {
                        kind: nk.kind.clone(),
                        patterns: nk.patterns.clone(),
                    });
                }
            }
            DialectItem::NamePart(part) => {
                let m = self.morpheme(part, &reg.name, pack);
                reg.morphemes.push(m);
            }
            DialectItem::Other => {}
        }
    }

    /// Compile a [`NamePartDecl`] into a [`NamePartDef`] under `register`.
    fn morpheme(&mut self, decl: &NamePartDecl, register: &str, pack: &str) -> NamePartDef {
        let role = head_value(&decl.head, "role").unwrap_or_default();
        let gender = head_value(&decl.head, "gender").and_then(|g| Gender::from_keyword(&g));
        let (ann, provenance, citations) = self.compile_ann(decl.provenance.as_ref(), pack);
        NamePartDef {
            surface: decl.name.text.clone(),
            gloss: decl.gloss.clone(),
            role,
            gender,
            register: register.to_owned(),
            ann,
            provenance,
            citations,
            span: decl.name.span,
        }
    }

    /// Record a `ratify <d> { h := true }` valuation: rule the decision true and
    /// apply each hypothesis assignment (05 §5, one valuation ratifies a bundle).
    ///
    /// The parser (`dli_syntax::structure_ratify_body`) already derived each
    /// `hyp := true|false` assignment; this applies them.
    fn ratify(&mut self, decl: &RatifyDecl) {
        self.intern_decision(&decl.decision.text);
        for clause in &decl.body.clauses {
            if let Some(assignment) = &clause.assignment {
                let atom = self.model.interner.intern(assignment.hyp.trim());
                self.model.valuation.set(atom, assignment.value);
            }
        }
    }

    /// Intern a decision id and rule it true (a cited decision is a resolved
    /// event). Returns the interned atom.
    fn intern_decision(&mut self, id: &str) -> Atom {
        let atom = self.model.interner.intern(id.trim());
        self.model.valuation.set(atom, true);
        atom
    }

    /// Compile a declaration's provenance into `(Ann, kind, citations)`, mirroring
    /// the loader's gate (spec 02 §§2–3): a `proposed` claim gates on an open
    /// hypothesis; a ledger claim on its cited decision; absent or malformed
    /// provenance fails closed at `P⁺ = 0`.
    pub(crate) fn compile_ann(
        &mut self,
        provenance: Option<&Annotation>,
        pack: &str,
    ) -> (Ann, Provenance, Vec<String>) {
        let Some(annotation) = provenance else {
            return (
                Ann::supported(PosBool::FALSE),
                Provenance::Unprovenanced,
                Vec::new(),
            );
        };
        if annotation.marker.requires_citation() && annotation.citations.is_empty() {
            return (
                Ann::supported(PosBool::FALSE),
                Provenance::Unprovenanced,
                Vec::new(),
            );
        }
        let citations: Vec<String> = annotation
            .citations
            .iter()
            .map(|c| c.id.text.clone())
            .collect();

        let mut support = if annotation.marker == ProvenanceMarker::Proposed {
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
                PosBool::pos(self.model.interner.intern(&format!("h.{key}")))
            }
        } else {
            let mut acc = PosBool::FALSE;
            for citation in &annotation.citations {
                let atom = self.intern_decision(&citation.id.text);
                acc = acc.or_(&PosBool::pos(atom));
            }
            acc
        };

        if let Some(condition) = &annotation.condition {
            support = support.and_(&self.cond_to_posbool(condition, false));
        }

        (
            Ann::supported(support),
            Provenance::from_marker(annotation.marker),
            citations,
        )
    }

    /// Compile a hypothesis condition into a support polynomial (De Morgan pushes
    /// negation to the literals).
    fn cond_to_posbool(&mut self, cond: &CondExpr, negated: bool) -> PosBool {
        match cond {
            CondExpr::Hyp(id) => {
                let atom = self.model.interner.intern(id.text.trim());
                if negated {
                    PosBool::neg(atom)
                } else {
                    PosBool::pos(atom)
                }
            }
            CondExpr::Not(inner) => self.cond_to_posbool(inner, !negated),
            CondExpr::And(list) => self.cond_group(list, negated, negated),
            CondExpr::Or(list) => self.cond_group(list, negated, !negated),
        }
    }

    fn cond_group(&mut self, list: &[CondExpr], negated: bool, effective_or: bool) -> PosBool {
        let mut acc = if effective_or {
            PosBool::FALSE
        } else {
            PosBool::truth()
        };
        for member in list {
            let child = self.cond_to_posbool(member, negated);
            acc = if effective_or {
                acc.or_(&child)
            } else {
                acc.and_(&child)
            };
        }
        acc
    }

    fn finish(self) -> LanguageOutcome {
        LanguageOutcome {
            model: self.model,
            findings: self.findings,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared low-level helpers (used across the language submodules).
// ---------------------------------------------------------------------------

/// The value token after a header keyword (`role given-proto` → `given-proto`).
///
/// Reads a `name-part` declaration's typed `head` tokens — not clause text — so
/// it needs no lexer (LS-9).
fn head_value(head: &[String], keyword: &str) -> Option<String> {
    head.iter()
        .position(|t| t == keyword)
        .and_then(|i| head.get(i + 1))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dli_syntax::parse_document;

    fn model(src: &str) -> LanguageModel {
        let (doc, _diags) = parse_document(src);
        LanguageModel::from_documents(&[doc]).model
    }

    #[test]
    fn register_phonotactics_and_name_kinds_compile() {
        let m = model(
            "scope language\n\
             dialect cymric \"fringe\" ratified D-1 {\n\
             vowels a e i o u w y\n\
             endings -ys -wen\n\
             foreign qu x z\n\
             cluster-max 2\n\
             stress penultimate\n\
             name-kind person.given = given-proto given-deutero | given-simple\n\
             name-part en \"spirit\" role given-proto ratified D-1\n\
             }\n",
        );
        let reg = m.register("cymric").expect("compiled");
        assert_eq!(reg.phonotactics.vowels, ["a", "e", "i", "o", "u", "w", "y"]);
        assert_eq!(reg.phonotactics.endings, ["ys", "wen"]);
        assert_eq!(reg.phonotactics.foreign, ["qu", "x", "z"]);
        assert_eq!(reg.phonotactics.cluster_max, 2);
        assert_eq!(reg.phonotactics.stress, Stress::Penultimate);
        let nk = reg.name_kind("person.given").expect("name-kind");
        assert_eq!(nk.patterns.len(), 2);
        assert_eq!(nk.patterns[0], ["given-proto", "given-deutero"]);
        assert_eq!(reg.morphemes.len(), 1);
        assert_eq!(reg.morphemes[0].role, "given-proto");
    }

    #[test]
    fn ledger_morpheme_is_assertable_proposed_is_not() {
        let m = model(
            "scope language\n\
             dialect r \"r\" ratified D-1 {\n\
             name-part a \"a\" role given-simple ratified D-1\n\
             name-part b \"b\" role given-simple proposed agent:run assumes h-x\n\
             }\n",
        );
        let reg = m.register("r").expect("compiled");
        let a = reg.morpheme("a", "given-simple").expect("a");
        let b = reg.morpheme("b", "given-simple").expect("b");
        assert!(m.assertable(&a.ann), "a ledger morpheme is certain");
        assert!(
            !m.assertable(&b.ann),
            "a proposed morpheme is merely-possible"
        );
    }

    #[test]
    fn ratify_promotes_a_proposed_bundle() {
        let m = model(
            "scope language\n\
             hypothesis h-x \"loan\"\n\
             dialect r \"r\" ratified D-1 {\n\
             name-part b \"b\" role given-simple proposed agent:run assumes h-x\n\
             }\n\
             ratify D-9 { h-x := true }\n",
        );
        let reg = m.register("r").expect("compiled");
        let b = reg.morpheme("b", "given-simple").expect("b");
        assert!(
            m.assertable(&b.ann),
            "one valuation ratifies the bundle (05 §5)"
        );
    }
}
