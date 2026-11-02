//! The ratified-names view and name-space queries (`LANG-2`).
//!
//! The compiled world is the name-space; applications can query it rather
//! than maintaining a separate writable copy. [`ratified_names`] is that query: a generated view of the
//! certain layer, never a second writable copy.
//!
//! [`vet_candidate`] is the shadow-vet's kernel half: it runs the world-true
//! screens (uniqueness, fixed-token reuse, aural distinctness, banned-in-name)
//! over a bare candidate against the settled name-space. Applications may compose this with their own external screens (05 §3);
//! neither layer can overrule the other's ground truth.

use dli_ast::span::Span;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::derivation::{LangFinding, aural_clash, confusable_findings, contains_banned};
use super::dialect::LanguageModel;
use super::name::NameDef;

/// One rendered entry of the ratified-names view — the mirror's successor row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RatifiedName {
    /// The rendered name (label if written, else the identifier).
    pub name: String,
    /// The name kind (e.g. `person.given`, `place`), when the name declares one.
    pub kind: String,
    /// The register the name derives in, when written.
    pub register: String,
    /// The ruling that settled the name, if cited.
    pub decision: Option<String>,
    /// The entity that bears the name, if written.
    pub bearer: Option<String>,
}

/// The certain layer of the name-space, rendered as a generated view.
///
/// Only assertable names appear — a merely-possible (proposed) name is visible to
/// shadow queries but structurally excluded here, exactly as it is excluded from
/// every context pack and assertion (05 §5).
///
/// ```
/// use dli_world::language::{LanguageModel, ratified_names};
/// use dli_syntax::parse_document;
///
/// let src = "scope language\n\
///            name joren \"Joren\" place in moorlands ratified D-111\n\
///            name mist \"Mist\" place in moorlands proposed agent:run assumes h-x\n";
/// let (doc, _d) = parse_document(src);
/// let model = LanguageModel::from_documents(&[doc]).model;
/// let view = ratified_names(&model);
/// assert_eq!(view.len(), 1, "only the ratified name is in the certain layer");
/// assert_eq!(view[0].name, "Joren");
/// ```
#[must_use]
pub fn ratified_names(model: &LanguageModel) -> Vec<RatifiedName> {
    model
        .names
        .iter()
        .filter(|n| model.assertable(&n.ann))
        .map(|n| RatifiedName {
            name: display(n),
            kind: n.kind.clone(),
            register: n.register.clone(),
            decision: n.citations.first().cloned(),
            bearer: n.bearer.clone(),
        })
        .collect()
}

/// The surface forms of the certain name-space, for aural and uniqueness queries.
#[must_use]
pub fn certain_surfaces(model: &LanguageModel) -> Vec<String> {
    model
        .names
        .iter()
        .filter(|n| model.assertable(&n.ann))
        .map(NameDef::surface)
        .collect()
}

/// Vet a bare candidate string against the settled name-space — the kernel half
/// of the oracle's two-layer verdict (05 §3).
///
/// Runs only world-true screens: intra-canon uniqueness, fixed-token reuse, aural
/// distinctness, and banned-vocabulary-in-names. The application-side screens
/// (AI-tics, pop-culture collisions, English homophones, anachronism) are *not*
/// run here — a candidate that fails only those is kernel-clean, because their
/// ground truth is a fact about our world, not the story's.
///
/// ```
/// use dli_world::language::{LanguageModel, vet_candidate};
/// use dli_syntax::parse_document;
///
/// let src = "scope language\n\
///            name joren \"Joren\" place in moorlands ratified D-111\n";
/// let (doc, _d) = parse_document(src);
/// let model = LanguageModel::from_documents(&[doc]).model;
/// // A settled name collides; an unrelated one does not.
/// assert!(!vet_candidate(&model, "Joren").is_empty());
/// assert!(vet_candidate(&model, "Irondale").is_empty());
/// ```
#[must_use]
pub fn vet_candidate(model: &LanguageModel, candidate: &str) -> Vec<LangFinding> {
    let span = Span::synthetic();
    let mut out = Vec::new();
    let cand_norm = candidate.to_lowercase();
    let cand_surface = letters_lower(candidate);

    let certain: Vec<&NameDef> = model
        .names
        .iter()
        .filter(|n| model.assertable(&n.ann))
        .collect();

    let mut collided = false;
    for name in &certain {
        let disp = display(name).to_lowercase();
        if disp == cand_norm || name.surface() == cand_surface {
            out.push(LangFinding::Uniqueness {
                name: candidate.to_owned(),
                collides_with: display(name),
                span,
            });
            collided = true;
        } else if let Some(key) = aural_clash(candidate, &display(name)) {
            out.push(LangFinding::AuralClash {
                name: candidate.to_owned(),
                other: display(name),
                key,
                span,
            });
        }
    }

    // Fixed-token reuse: a multi-token candidate that embeds a reserved name.
    if !collided {
        let tokens: Vec<String> = cand_norm.split_whitespace().map(str::to_owned).collect();
        if tokens.len() > 1 {
            for name in &certain {
                let reserved = display(name).to_lowercase();
                if tokens.contains(&reserved) {
                    out.push(LangFinding::FixedTokenReuse {
                        name: candidate.to_owned(),
                        token: display(name),
                        span,
                    });
                }
            }
        }
    }

    if let Some((word, reason)) = contains_banned(&cand_surface, &cand_norm, model) {
        out.push(LangFinding::BannedInName {
            name: candidate.to_owned(),
            word,
            reason,
            span,
        });
    }

    out.extend(confusable_findings(candidate, span));
    out
}

/// Vet a candidate for aural distinctness within a declared **set** — a scene
/// cast, a family, a region (05 §3, set distinctness).
///
/// This is the scene-scoped sibling of [`vet_candidate`]'s whole-name-space aural
/// check, and the primitive the craft layer's `scene-name-clash` rule joins
/// against (06-craft-and-payloads §3) — a join possible only where naming data
/// and cast lists share a kernel. Pure over the given set; it needs no model,
/// only the phonetic key.
///
/// ```
/// use dli_world::language::vet_in_set;
/// // Two cast members who sound too alike clash within their scene.
/// let clashes = vet_in_set("Isobel", "scene-3-cast", &["Isobell", "Idal"]);
/// assert!(clashes.iter().any(|f| f.code() == "name.set-clash"));
/// // A distinct name is clean in the same set.
/// assert!(vet_in_set("Idal", "scene-3-cast", &["Isobel", "Linet"]).is_empty());
/// ```
#[must_use]
pub fn vet_in_set(candidate: &str, set: &str, members: &[&str]) -> Vec<LangFinding> {
    let span = Span::synthetic();
    members
        .iter()
        .filter(|member| letters_lower(member) != letters_lower(candidate))
        .filter_map(|member| {
            aural_clash(candidate, member).map(|_| LangFinding::SetClash {
                name: candidate.to_owned(),
                other: (*member).to_owned(),
                set: set.to_owned(),
                span,
            })
        })
        .collect()
}

/// The rendered form of a name: its label if written, else its identifier.
fn display(name: &NameDef) -> String {
    name.label.clone().unwrap_or_else(|| name.name.clone())
}

/// A string reduced to its lowercase alphabetic letters.
fn letters_lower(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphabetic)
        .flat_map(char::to_lowercase)
        .collect()
}
