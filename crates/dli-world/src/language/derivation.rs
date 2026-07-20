//! The derivation checker and its findings (`LANG-2`).
//!
//! "Register-true" is a compile verdict, not a reviewer's impression (05 §2). A
//! name that fails any of the three obligations — slot sequence, role membership,
//! phonotactics — does not compile, and the failure speaks the world's terms, not
//! a stack trace (charter koan 9). This module runs those obligations plus the
//! naming checks that re-home into the kernel because their ground truth is a
//! fact about the *story's* world (05 §3): intra-canon uniqueness, fixed-token
//! reuse, aural / set distinctness, gender morphology, sayability caps, and
//! banned-vocabulary-in-names.
//!
//! # The vet split
//!
//! Only the **world-true** half lives here. The world-safe screens — AI-tics,
//! pop-culture collisions, English homophones, anachronism, portent clichés,
//! tarot-figure knowledge — stay factory-side: they are facts about *our* world,
//! change on research's schedule, and must never gate assertability (05 §3). A
//! [`LangFinding`] is therefore always a claim about the story's world.
//!
//! # The aural key
//!
//! The phonetic-key distance primitive ([`phonetic_key`]) maps a name to a folded
//! phonetic skeleton and measures edit distance between keys, so "these two names
//! sound too alike" becomes a query over the name-space rather than an ear's
//! impression (05 §3). It is a faithful reconstruction of the consuming oracle's
//! aural-distinctness key: fold the digraphs and consonant equivalence classes an
//! English ear collapses, drop doubled letters, and compare.

use dli_ast::span::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::check::{Severity, WorldFinding};

use super::dialect::{DialectDef, Gender, LanguageModel};

/// The base vowels, always treated as vowels for cluster and syllable counting,
/// independent of a register's characteristic vowel inventory.
const BASE_VOWELS: [char; 5] = ['a', 'e', 'i', 'o', 'u'];

/// The aural-distinctness edit-distance threshold: two names whose phonetic keys
/// are this close (or equal) sound too alike.
const AURAL_THRESHOLD: usize = 1;

/// The default sayability-tier syllable ceiling.
///
/// The consuming repo's sayability caps are ruled world policy (D-0095); until
/// those tiers are ported as data this is a single generous ceiling that still
/// catches an unpronounceable pile-up.
const SAYABILITY_CAP: usize = 6;

/// A naming verdict about the story's world (05 §3, in-kernel half).
///
/// Every variant is world-true: it is a fact about the tongue, the name-space, or
/// a ruling — never a factory-side screen. Each carries a stable machine-readable
/// [`code`](LangFinding::code) and a domain-spoken [`message`](LangFinding::message).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "finding", rename_all = "kebab-case")]
pub enum LangFinding {
    /// The composed slot sequence matches no declared `name-kind` pattern.
    SlotSequence {
        /// The offending name.
        name: String,
        /// The register whose grammar was consulted.
        register: String,
        /// The name kind whose patterns were consulted.
        kind: String,
        /// The slot-role sequence the composition claimed.
        roles: Vec<String>,
        /// The register's declared patterns for the kind.
        patterns: Vec<Vec<String>>,
        /// Source location.
        span: Span,
    },
    /// A cited morpheme does not exist in the register under the claimed role.
    RoleMembership {
        /// The offending name.
        name: String,
        /// The register consulted.
        register: String,
        /// The morpheme surface cited.
        morpheme: String,
        /// The role it was claimed to fill.
        role: String,
        /// Source location.
        span: Span,
    },
    /// The name's register could not be resolved, or the kind is ungoverned.
    RegisterFit {
        /// The offending name.
        name: String,
        /// The register named (which may not exist).
        register: String,
        /// Why register-fit failed.
        reason: String,
        /// Source location.
        span: Span,
    },
    /// The composed surface form violates the register's phonotactic frame.
    Phonotactics {
        /// The offending name.
        name: String,
        /// The register whose frame was consulted.
        register: String,
        /// The composed surface form.
        surface: String,
        /// What the frame rejected.
        reason: String,
        /// Source location.
        span: Span,
    },
    /// Two declarations claim the same name — an intra-canon uniqueness clash.
    Uniqueness {
        /// The colliding name.
        name: String,
        /// The name already in the name-space it collides with.
        collides_with: String,
        /// Source location.
        span: Span,
    },
    /// A candidate reuses a reserved fixed token of the name-space.
    FixedTokenReuse {
        /// The candidate name.
        name: String,
        /// The reserved token it reused.
        token: String,
        /// Source location.
        span: Span,
    },
    /// A name sounds too like another name in the whole name-space.
    AuralClash {
        /// The offending name.
        name: String,
        /// The name it sounds too like.
        other: String,
        /// The shared phonetic key.
        key: String,
        /// Source location.
        span: Span,
    },
    /// A name sounds too like another within a declared set (a cast, a family).
    SetClash {
        /// The offending name.
        name: String,
        /// The name it sounds too like.
        other: String,
        /// The set they share.
        set: String,
        /// Source location.
        span: Span,
    },
    /// The name's morphemes carry conflicting gender, or clash with the bearer.
    GenderMorphology {
        /// The offending name.
        name: String,
        /// Why the gender join failed.
        reason: String,
        /// Source location.
        span: Span,
    },
    /// The name exceeds the ruled sayability-tier syllable cap (D-0095).
    SayabilityCap {
        /// The offending name.
        name: String,
        /// The estimated syllable count.
        syllables: usize,
        /// The cap it exceeded.
        cap: usize,
        /// Source location.
        span: Span,
    },
    /// The name contains a banned lexicon word (05 §4).
    BannedInName {
        /// The offending name.
        name: String,
        /// The banned word it contains.
        word: String,
        /// Source location.
        span: Span,
    },
    /// The identifier mixes a Latin-looking form with a look-alike character from
    /// another script — a homoglyph that could disguise one id as another.
    Confusable {
        /// The offending identifier.
        name: String,
        /// The confusable character it carries.
        character: char,
        /// The Latin letter that character mimics.
        looks_like: char,
        /// The script the confusable character actually belongs to.
        script: String,
        /// Source location.
        span: Span,
    },
}

impl LangFinding {
    /// A stable, machine-readable class for this finding (charter koan 9). Tools
    /// and benchmarks match on this; it never changes for a variant.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::SlotSequence { .. } => "name.slot-sequence",
            Self::RoleMembership { .. } => "name.role-membership",
            Self::RegisterFit { .. } => "name.register-fit",
            Self::Phonotactics { .. } => "name.phonotactics",
            Self::Uniqueness { .. } => "name.uniqueness",
            Self::FixedTokenReuse { .. } => "name.fixed-token-reuse",
            Self::AuralClash { .. } => "name.aural-clash",
            Self::SetClash { .. } => "name.set-clash",
            Self::GenderMorphology { .. } => "name.gender-morphology",
            Self::SayabilityCap { .. } => "name.sayability-cap",
            Self::BannedInName { .. } => "name.banned-in-name",
            Self::Confusable { .. } => "name.confusable",
        }
    }

    /// A domain-spoken description of the finding (charter koan 9, errors are
    /// lore).
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::SlotSequence {
                name,
                register,
                kind,
                roles,
                patterns,
                ..
            } => format!(
                "register-fit at name {name}: the slots [{}] match no {kind} pattern of {register}. declared name-kinds: {}",
                roles.join(" "),
                render_patterns(patterns)
            ),
            Self::RoleMembership {
                name,
                register,
                morpheme,
                role,
                ..
            } => format!(
                "register-fit at name {name}: morpheme '{morpheme}' is not declared in {register} under the role '{role}'"
            ),
            Self::RegisterFit {
                name,
                register,
                reason,
                ..
            } => format!("register-fit at name {name}: {reason} (register {register})"),
            Self::Phonotactics {
                name,
                register,
                surface,
                reason,
                ..
            } => format!(
                "register-fit at name {name} (surface '{surface}'): {reason}, against {register}'s frame"
            ),
            Self::Uniqueness {
                name,
                collides_with,
                ..
            } => format!("'{name}' is already a settled name in the world ({collides_with})"),
            Self::FixedTokenReuse { name, token, .. } => {
                format!("'{name}' reuses the reserved token '{token}' the world holds fixed")
            }
            Self::AuralClash {
                name, other, key, ..
            } => format!("'{name}' sounds too like '{other}' (both key to '{key}')"),
            Self::SetClash {
                name, other, set, ..
            } => format!("'{name}' sounds too like '{other}' within the set '{set}'"),
            Self::GenderMorphology { name, reason, .. } => {
                format!("gender morphology at name {name}: {reason}")
            }
            Self::SayabilityCap {
                name,
                syllables,
                cap,
                ..
            } => format!(
                "'{name}' has {syllables} syllables, over the sayability-tier cap of {cap} (D-0095)"
            ),
            Self::BannedInName { name, word, .. } => {
                format!(
                    "'{name}' carries the banned word '{word}' — the allegory is structural, never lexical (05 §4)"
                )
            }
            Self::Confusable {
                name,
                character,
                looks_like,
                script,
                ..
            } => format!(
                "'{name}' carries the {script} character '{character}' (U+{:04X}) that mimics Latin '{looks_like}' — a mixed-script look-alike that could disguise this id as another",
                *character as u32
            ),
        }
    }

    /// The source location of the finding.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::SlotSequence { span, .. }
            | Self::RoleMembership { span, .. }
            | Self::RegisterFit { span, .. }
            | Self::Phonotactics { span, .. }
            | Self::Uniqueness { span, .. }
            | Self::FixedTokenReuse { span, .. }
            | Self::AuralClash { span, .. }
            | Self::SetClash { span, .. }
            | Self::GenderMorphology { span, .. }
            | Self::SayabilityCap { span, .. }
            | Self::BannedInName { span, .. }
            | Self::Confusable { span, .. } => *span,
        }
    }

    /// How much the finding must be attended to.
    ///
    /// Distinctness and sayability are defects worth a look — a near-homophone or
    /// an over-long name is not, on its own, an unassertable derivation; the
    /// register-fit and name-space-integrity failures are hard violations.
    #[must_use]
    pub fn severity(&self) -> Severity {
        match self {
            Self::AuralClash { .. }
            | Self::SetClash { .. }
            | Self::SayabilityCap { .. }
            | Self::Confusable { .. } => Severity::Warn,
            _ => Severity::Error,
        }
    }

    /// Lift this naming finding into the world-check aggregator's unified
    /// [`WorldFinding`], preserving its code, severity, message, and span.
    #[must_use]
    pub fn to_world_finding(&self) -> WorldFinding {
        let base = match self.severity() {
            Severity::Error => WorldFinding::error(self.code(), self.message()),
            Severity::Warn => WorldFinding::warn(self.code(), self.message()),
        };
        base.with_span(self.span())
    }
}

/// Run the derivation battery over a compiled [`LanguageModel`], returning every
/// world-true naming finding.
///
/// Structural checks (slot sequence, role membership, phonotactics, sayability,
/// gender, banned-in-name) run on every name regardless of status — a
/// merely-possible name is still checked. Uniqueness and aural distinctness
/// compare each name against the *certain* name-space declared before it: a
/// proposed name is vetted against settled names, but never pollutes the space.
#[must_use]
pub fn check_language(model: &LanguageModel) -> Vec<LangFinding> {
    let mut findings = Vec::new();
    for (i, name) in model.names.iter().enumerate() {
        structural_checks(model, name, &mut findings);
        // Compare against certain names declared earlier.
        for earlier in &model.names[..i] {
            if !model.assertable(&earlier.ann) {
                continue;
            }
            collision_checks(name, earlier, &mut findings);
        }
    }
    findings
}

/// Vet a bare candidate string against a named register's **phonotactic frame** —
/// the surface-only half of register-fit (05 §2).
///
/// The oracle's register-fit obligation has three parts: slot sequence, role
/// membership, and phonotactics. The first two are claims about a name's
/// *composition* — which morphemes fill which slots — and a bare candidate string
/// carries no composition, so only the third, the register's phonotactic frame
/// (foreign clusters, the consonant-cluster ceiling), is dischargeable over a
/// bare candidate. This composes the candidate into a trial [`NameDef`](crate::language::NameDef)
/// ([`NameDef::trial`](super::name::NameDef::trial)) and runs exactly that check;
/// an unknown register is itself a register-fit failure.
///
/// Callers must disclose that the composition-dependent obligations were not run
/// — the CLI's `name check --dialect` does. This is the deliberate, honest subset
/// (05 §3): a check runs iff its ground truth is dischargeable over the input it
/// is given.
///
/// ```
/// use dli_world::language::{LanguageModel, vet_candidate_in_dialect};
/// use dli_syntax::parse_document;
///
/// let src = "scope language\n\
///            dialect cymric-fringe \"the fringe tongue\" ratified D-1 {\n\
///            vowels a e i o u\n\
///            foreign z\n\
///            cluster-max 2\n\
///            }\n";
/// let (doc, _d) = parse_document(src);
/// let model = LanguageModel::from_documents(&[doc]).model;
/// // A pile-up of register-foreign consonants fails the frame.
/// assert!(!vet_candidate_in_dialect(&model, "Zzgrbl", "cymric-fringe").is_empty());
/// // A register-shaped candidate clears it.
/// assert!(vet_candidate_in_dialect(&model, "Aelin", "cymric-fringe").is_empty());
/// // An unknown register is a register-fit failure.
/// assert!(vet_candidate_in_dialect(&model, "Aelin", "no-such").is_empty() == false);
/// ```
#[must_use]
pub fn vet_candidate_in_dialect(
    model: &LanguageModel,
    candidate: &str,
    dialect: &str,
) -> Vec<LangFinding> {
    let mut out = Vec::new();
    let Some(register) = model.register(dialect) else {
        out.push(LangFinding::RegisterFit {
            name: candidate.to_owned(),
            register: dialect.to_owned(),
            reason: "the named register is not declared".to_owned(),
            span: Span::synthetic(),
        });
        return out;
    };
    let trial = super::name::NameDef::trial(candidate, dialect);
    phonotactics(&trial, register, &mut out);
    out
}

/// The structural obligations on a single name (05 §2) plus the per-name
/// world-true checks (banned, sayability, gender).
///
/// Register-fit is a claim about a *derivation*: a bare declared name (no
/// composition) asserts none, so only the status-independent checks (banned,
/// sayability) run on it. A composed name whose register cannot be resolved is a
/// genuine register-fit failure.
fn structural_checks(
    model: &LanguageModel,
    name: &super::name::NameDef,
    out: &mut Vec<LangFinding>,
) {
    if !name.composition.is_empty() {
        match model.register(&name.register) {
            Some(register) => {
                slot_sequence(name, register, out);
                role_membership(model, name, register, out);
                phonotactics(name, register, out);
                gender_morphology(model, name, register, out);
            }
            None => out.push(LangFinding::RegisterFit {
                name: name.name.clone(),
                register: name.register.clone(),
                reason: "the named register is not declared".to_owned(),
                span: name.span,
            }),
        }
    }
    banned_and_sayability(model, name, out);
    // The identifier is the authority-bearing token; screen it for a homoglyph
    // that could disguise it as another id.
    out.extend(confusable_findings(&name.name, name.span));
}

/// The slot sequence must match one of the register's `name-kind` patterns.
fn slot_sequence(name: &super::name::NameDef, register: &DialectDef, out: &mut Vec<LangFinding>) {
    let roles = name.roles();
    let Some(name_kind) = register.name_kind(&name.kind) else {
        out.push(LangFinding::RegisterFit {
            name: name.name.clone(),
            register: register.name.clone(),
            reason: format!("no name-kind grammar is declared for '{}'", name.kind),
            span: name.span,
        });
        return;
    };
    if !name_kind.patterns.contains(&roles) {
        out.push(LangFinding::SlotSequence {
            name: name.name.clone(),
            register: register.name.clone(),
            kind: name.kind.clone(),
            roles,
            patterns: name_kind.patterns.clone(),
            span: name.span,
        });
    }
}

/// Each cited morpheme must exist in the register under the claimed role.
fn role_membership(
    model: &LanguageModel,
    name: &super::name::NameDef,
    register: &DialectDef,
    out: &mut Vec<LangFinding>,
) {
    for step in &name.composition {
        if model
            .resolve_morpheme(&register.name, &step.morpheme, &step.role)
            .is_none()
        {
            out.push(LangFinding::RoleMembership {
                name: name.name.clone(),
                register: register.name.clone(),
                morpheme: step.morpheme.clone(),
                role: step.role.clone(),
                span: name.span,
            });
        }
    }
}

/// The composed surface form must pass the register's phonotactic frame.
fn phonotactics(name: &super::name::NameDef, register: &DialectDef, out: &mut Vec<LangFinding>) {
    let surface = name.surface();
    if let Some(cluster) = contains_foreign(&surface, &register.phonotactics.foreign) {
        out.push(LangFinding::Phonotactics {
            name: name.name.clone(),
            register: register.name.clone(),
            surface: surface.clone(),
            reason: format!("the cluster '{cluster}' marks the form as not of this tongue"),
            span: name.span,
        });
    }
    let cap = register.phonotactics.cluster_max;
    if cap > 0 {
        let run = max_consonant_run(&surface, register);
        if run > cap {
            out.push(LangFinding::Phonotactics {
                name: name.name.clone(),
                register: register.name.clone(),
                surface,
                reason: format!("a run of {run} consonants exceeds cluster-max {cap}"),
                span: name.span,
            });
        }
    }
}

/// The name's morphemes must not carry conflicting gender.
///
/// The full join against a bearer's canon facts (05 §3) waits on the character
/// layer; the intra-name conflict — a name built from an explicitly-male and an
/// explicitly-female morpheme at once — is a world-true inconsistency checkable
/// now over the register's gender metadata.
fn gender_morphology(
    model: &LanguageModel,
    name: &super::name::NameDef,
    register: &DialectDef,
    out: &mut Vec<LangFinding>,
) {
    let mut male = false;
    let mut female = false;
    for step in &name.composition {
        match model
            .resolve_morpheme(&register.name, &step.morpheme, &step.role)
            .and_then(|m| m.gender)
        {
            Some(Gender::Male) => male = true,
            Some(Gender::Female) => female = true,
            _ => {}
        }
    }
    if male && female {
        out.push(LangFinding::GenderMorphology {
            name: name.name.clone(),
            reason: "the name joins an explicitly-male and an explicitly-female morpheme"
                .to_owned(),
            span: name.span,
        });
    }
}

/// Banned-in-name and sayability run regardless of register resolution.
fn banned_and_sayability(
    model: &LanguageModel,
    name: &super::name::NameDef,
    out: &mut Vec<LangFinding>,
) {
    let surface = name.surface();
    let label = name.label.clone().unwrap_or_default().to_lowercase();
    if let Some(word) = contains_banned(&surface, &label, model) {
        out.push(LangFinding::BannedInName {
            name: name.name.clone(),
            word,
            span: name.span,
        });
    }
    // Sayability is a per-word ceiling: a multi-word name (a title, a full
    // person-name) is sayable when each of its words is, so the longest word
    // governs — never the concatenation.
    let syllables = max_word_syllables(name);
    if syllables > SAYABILITY_CAP {
        out.push(LangFinding::SayabilityCap {
            name: name.name.clone(),
            syllables,
            cap: SAYABILITY_CAP,
            span: name.span,
        });
    }
}

/// The syllable count of a name's longest word (label if written, else surface).
fn max_word_syllables(name: &super::name::NameDef) -> usize {
    let source = name
        .label
        .as_deref()
        .map_or_else(|| name.surface(), str::to_owned);
    source
        .split_whitespace()
        .map(syllable_count)
        .max()
        .unwrap_or(1)
}

/// Uniqueness and aural distinctness between a name and an earlier settled name.
fn collision_checks(
    name: &super::name::NameDef,
    earlier: &super::name::NameDef,
    out: &mut Vec<LangFinding>,
) {
    let a = display_key(name);
    let b = display_key(earlier);
    if a == b {
        out.push(LangFinding::Uniqueness {
            name: name.name.clone(),
            collides_with: earlier.name.clone(),
            span: name.span,
        });
        return;
    }
    if let Some(key) = aural_clash(&name.surface(), &earlier.surface()) {
        out.push(LangFinding::AuralClash {
            name: name.name.clone(),
            other: earlier.name.clone(),
            key,
            span: name.span,
        });
    }
}

/// The comparison key for uniqueness: the label if present, else the surface.
fn display_key(name: &super::name::NameDef) -> String {
    name.label
        .as_deref()
        .map_or_else(|| name.surface(), str::to_lowercase)
}

// ---------------------------------------------------------------------------
// Primitives (shared with the name-space view).
// ---------------------------------------------------------------------------

/// The phonetic key of a name: a folded skeleton for aural comparison.
///
/// ```
/// use dli_world::language::phonetic_key;
/// assert_eq!(phonetic_key("Trefpenn"), phonetic_key("Trefpen"));
/// assert_ne!(phonetic_key("Gwern"), phonetic_key("Swartby"));
/// ```
#[must_use]
pub fn phonetic_key(name: &str) -> String {
    let lowered: String = name
        .chars()
        .filter(char::is_ascii_alphabetic)
        .flat_map(char::to_lowercase)
        .collect();
    let digraphs = lowered
        .replace("ph", "f")
        .replace("wh", "w")
        .replace("ck", "k")
        .replace("qu", "kw");
    let mut key = String::with_capacity(digraphs.len());
    let mut last = '\0';
    for c in digraphs.chars() {
        let folded = match c {
            'c' | 'q' | 'k' => 'k',
            'z' => 's',
            'y' | 'j' => 'i',
            'v' => 'f',
            other => other,
        };
        if folded != last {
            key.push(folded);
            last = folded;
        }
    }
    key
}

/// Whether two names sound too alike; returns the shared/near key on a clash.
#[must_use]
pub fn aural_clash(a: &str, b: &str) -> Option<String> {
    let ka = phonetic_key(a);
    let kb = phonetic_key(b);
    if ka.is_empty() || kb.is_empty() {
        return None;
    }
    if levenshtein(&ka, &kb) <= AURAL_THRESHOLD {
        Some(ka)
    } else {
        None
    }
}

/// The banned lexicon word a name carries, if any (checks surface and label).
#[must_use]
pub(crate) fn contains_banned(surface: &str, label: &str, model: &LanguageModel) -> Option<String> {
    model.bans.iter().find_map(|ban| {
        let word = ban.word.to_lowercase();
        (surface.contains(&word) || label.contains(&word)).then_some(word)
    })
}

/// Screen an identifier for mixed-script homoglyphs: a Latin-looking id carrying
/// a look-alike character from another script — a Cyrillic `а` amid Latin letters
/// — earns a WARN, because it could disguise one id as another and so quietly
/// mint the wrong authority.
///
/// A wholly non-Latin name is legitimate worldbuilding and never flagged; the
/// risk is the *mix*. This is a screen distinct from the free-text control-char
/// refusal (which guards bidi/format forgery): homoglyphs are visible, legal
/// characters, so they warn rather than refuse — escaping every non-ASCII letter
/// would break non-Latin worldbuilding. The confusables table is a small,
/// hand-picked set of the common Latin/Cyrillic/Greek look-alikes (Unicode TR39
/// in spirit), deliberately not a full database or a heavyweight dependency.
pub(crate) fn confusable_findings(id: &str, span: Span) -> Vec<LangFinding> {
    let mut has_latin = false;
    let mut suspects: Vec<(char, char, &'static str)> = Vec::new();
    for c in id.chars() {
        if is_latin_letter(c) {
            has_latin = true;
        } else if let Some((looks_like, script)) = confusable_prototype(c) {
            suspects.push((c, looks_like, script));
        }
    }
    if !has_latin {
        return Vec::new();
    }
    suspects.sort_unstable();
    suspects.dedup();
    suspects
        .into_iter()
        .map(|(character, looks_like, script)| LangFinding::Confusable {
            name: id.to_owned(),
            character,
            looks_like,
            script: script.to_owned(),
            span,
        })
        .collect()
}

/// Whether a character is a genuine Latin-script letter (ASCII or the accented
/// Latin ranges) — so an accented worldbuilding name is Latin, not a confusable.
fn is_latin_letter(c: char) -> bool {
    if c.is_ascii_alphabetic() {
        return true;
    }
    // Latin-1 Supplement and Latin Extended-A/B letters (é, ñ, ā, …).
    matches!(c, '\u{00C0}'..='\u{024F}') && c.is_alphabetic()
}

/// The Latin letter a look-alike from another script mimics, with that script's
/// name — the targeted homoglyphs an id-spoof leans on.
fn confusable_prototype(c: char) -> Option<(char, &'static str)> {
    Some(match c {
        // Cyrillic → Latin (lowercase).
        'а' => ('a', "Cyrillic"),
        'е' => ('e', "Cyrillic"),
        'о' => ('o', "Cyrillic"),
        'р' => ('p', "Cyrillic"),
        'с' => ('c', "Cyrillic"),
        'у' => ('y', "Cyrillic"),
        'х' => ('x', "Cyrillic"),
        'і' => ('i', "Cyrillic"),
        'ј' => ('j', "Cyrillic"),
        'ѕ' => ('s', "Cyrillic"),
        'ԁ' => ('d', "Cyrillic"),
        'һ' => ('h', "Cyrillic"),
        // Cyrillic → Latin (uppercase).
        'А' => ('A', "Cyrillic"),
        'В' => ('B', "Cyrillic"),
        'Е' => ('E', "Cyrillic"),
        'К' => ('K', "Cyrillic"),
        'М' => ('M', "Cyrillic"),
        'Н' => ('H', "Cyrillic"),
        'О' => ('O', "Cyrillic"),
        'Р' => ('P', "Cyrillic"),
        'С' => ('C', "Cyrillic"),
        'Т' => ('T', "Cyrillic"),
        'Х' => ('X', "Cyrillic"),
        'У' => ('Y', "Cyrillic"),
        'І' => ('I', "Cyrillic"),
        'Ј' => ('J', "Cyrillic"),
        // Greek → Latin.
        'ο' => ('o', "Greek"),
        'α' => ('a', "Greek"),
        'ρ' => ('p', "Greek"),
        'ν' => ('v', "Greek"),
        'τ' => ('t', "Greek"),
        'υ' => ('u', "Greek"),
        'κ' => ('k', "Greek"),
        'Α' => ('A', "Greek"),
        'Β' => ('B', "Greek"),
        'Ε' => ('E', "Greek"),
        'Η' => ('H', "Greek"),
        'Ι' => ('I', "Greek"),
        'Κ' => ('K', "Greek"),
        'Μ' => ('M', "Greek"),
        'Ν' => ('N', "Greek"),
        'Ο' => ('O', "Greek"),
        'Ρ' => ('P', "Greek"),
        'Τ' => ('T', "Greek"),
        'Υ' => ('Y', "Greek"),
        'Χ' => ('X', "Greek"),
        _ => return None,
    })
}

/// The Levenshtein edit distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// The maximum run of consecutive consonants in a surface form, where a letter
/// is a vowel iff it is a base vowel or a single-character vowel of the register.
fn max_consonant_run(surface: &str, register: &DialectDef) -> usize {
    let vowels = register_vowels(register);
    let mut max = 0;
    let mut run = 0;
    for c in surface.chars().filter(|c| c.is_alphabetic()) {
        if vowels.contains(&c) {
            run = 0;
        } else {
            run += 1;
            max = max.max(run);
        }
    }
    max
}

/// The set of characters that count as vowels for `register`: the base vowels
/// plus any single-character vowel the register declares (e.g. Welsh `w`, `y`).
fn register_vowels(register: &DialectDef) -> std::collections::BTreeSet<char> {
    let mut set: std::collections::BTreeSet<char> = BASE_VOWELS.into_iter().collect();
    for v in &register.phonotactics.vowels {
        if let [c] = v.chars().collect::<Vec<_>>().as_slice() {
            set.insert(*c);
        }
    }
    set
}

/// An estimate of a surface form's syllable count: the number of maximal vowel
/// groups (base vowels plus `y`).
fn syllable_count(surface: &str) -> usize {
    let mut count = 0;
    let mut prev_was_vowel = false;
    for c in surface.chars().filter(|c| c.is_alphabetic()) {
        let vowel = BASE_VOWELS.contains(&c) || c == 'y';
        if vowel && !prev_was_vowel {
            count += 1;
        }
        prev_was_vowel = vowel;
    }
    count.max(1)
}

/// The first register-foreign cluster present in a surface form, if any.
fn contains_foreign<'a>(surface: &str, foreign: &'a [String]) -> Option<&'a str> {
    foreign
        .iter()
        .find(|cluster| !cluster.is_empty() && surface.contains(cluster.as_str()))
        .map(String::as_str)
}

/// Render a register's declared patterns for a diagnostic:
/// `[given-proto given-deutero] | [given-simple]`.
fn render_patterns(patterns: &[Vec<String>]) -> String {
    patterns
        .iter()
        .map(|p| format!("[{}]", p.join(" ")))
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phonetic_key_folds_doubles_and_classes() {
        assert_eq!(phonetic_key("Trefpenn"), phonetic_key("Trefpen"));
        assert_eq!(phonetic_key("Whitecombe"), "witekombe");
        assert!(aural_clash("Gwern", "Gwerne").is_some());
        assert!(aural_clash("Gwern", "Swartby").is_none());
    }

    #[test]
    fn levenshtein_is_symmetric_and_zero_on_equal() {
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("abc", "abd"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn syllables_count_vowel_groups() {
        assert_eq!(syllable_count("halspetra"), 3);
        assert_eq!(syllable_count("gwern"), 1);
        assert_eq!(syllable_count("enfys"), 2);
    }

    #[test]
    fn confusable_flags_mixed_script_only() {
        let span = Span::synthetic();
        // "p\u{0430}ypal" — the second letter is a Cyrillic 'а' amid Latin
        // letters: a homoglyph that disguises this id as the Latin "paypal".
        let spoof = confusable_findings("p\u{0430}ypal", span);
        assert_eq!(spoof.len(), 1, "the mixed-script look-alike warns");
        assert_eq!(spoof[0].code(), "name.confusable");
        assert_eq!(spoof[0].severity(), Severity::Warn);
        assert!(spoof[0].message().contains("Cyrillic"));
        // A clean ASCII id, an accented-Latin id, and a wholly non-Latin
        // (single-script) name are all legitimate — the risk is only the mix.
        assert!(confusable_findings("paypal", span).is_empty());
        assert!(confusable_findings("bj\u{00f6}rn", span).is_empty());
        assert!(confusable_findings("\u{043e}\u{0440}\u{0435}", span).is_empty());
    }
}
