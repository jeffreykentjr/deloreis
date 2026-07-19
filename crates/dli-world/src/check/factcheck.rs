//! Draft claim-checking: supported / contradicted / unsupported with
//! citations; rejected-lore constraints enforced (`CHECK-1`).
//!
//! [`factcheck`] resolves each asserted claim against the compiled world's
//! *certain* projection and returns a [`ClaimReport`]: a [`Verdict`] with the
//! D-citations of the fact it was judged against.
//!
//! - **supported** — a certain fact holds the claim's value for its slot; the
//!   fact's citations ride along.
//! - **contradicted** — a certain fact holds a *different* value for a
//!   single-valued slot, or the claim realizes a killed token (rejected lore).
//! - **unsupported** — no certain fact bears on the claim: candidate new lore,
//!   legal only where a hypothesis carries it, a defect when stated as settled.
//!
//! Single-valuedness is read from the data: a slot with exactly one certain value
//! is a functional dependency (spec 02 §4), so a different value contradicts it; a
//! slot with several certain values is a multi-valued relation, so a new value is
//! merely unsupported, never a contradiction. This keeps the checker from
//! false-flagging a legitimate second edge (a second adjacency, say) as a clash.

use dli_ast::term::AtomExpr;
use dli_core::grade::Grade;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::CompiledWorld;
use super::continuity::{certain_values, render_term, slot_citations, slot_of};
use crate::draft::DraftFinding;

/// A claim's standing against the certain projection of the world.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// A certain fact bears the claim's value.
    Supported,
    /// A certain fact contradicts the claim, or the claim realizes rejected lore.
    Contradicted,
    /// No certain fact bears on the claim — candidate new lore.
    Unsupported,
}

/// One claim's fact-check outcome: the rendered claim, its verdict, the D-citations
/// of the canon it was judged against, and a domain-spoken reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClaimReport {
    /// The claim, rendered `pred(a, b)`.
    pub claim: String,
    /// Its standing against the certain projection.
    pub verdict: Verdict,
    /// The ledger citations of the fact the claim was judged against.
    pub citations: Vec<String>,
    /// A domain-spoken explanation of the verdict.
    pub reason: String,
}

impl ClaimReport {
    /// Whether this outcome is a defect worth surfacing (anything but
    /// [`Verdict::Supported`]).
    #[must_use]
    pub fn is_defect(&self) -> bool {
        self.verdict != Verdict::Supported
    }

    /// Render this outcome as a [`DraftFinding`]. Every deterministic check
    /// re-derives exactly, so the finding carries [`Grade::Checked`].
    #[must_use]
    pub fn to_draft_finding(&self) -> DraftFinding {
        DraftFinding {
            claim: self.claim.clone(),
            grade: Grade::Checked,
            message: self.reason.clone(),
        }
    }
}

/// Fact-check a set of asserted claims against the compiled world's certain
/// projection, one [`ClaimReport`] per claim in input order.
#[must_use]
pub fn factcheck(world: &CompiledWorld, claims: &[AtomExpr]) -> Vec<ClaimReport> {
    claims.iter().map(|claim| classify(world, claim)).collect()
}

/// Classify one claim.
fn classify(world: &CompiledWorld, claim: &AtomExpr) -> ClaimReport {
    let rendered = render_atom(claim);

    // Rejected lore first: a claim naming a killed token is contradicted outright.
    for token in claim
        .args
        .iter()
        .filter_map(|term| match term {
            dli_ast::term::Term::Sym(id) => Some(id.text.as_str()),
            _ => None,
        })
        .chain(std::iter::once(claim.pred.text.as_str()))
    {
        if let Some(hit) = world.killed.hit(token) {
            return ClaimReport {
                claim: rendered,
                verdict: Verdict::Contradicted,
                citations: vec![hit.decision.clone()],
                reason: format!(
                    "realizes killed token '{}' ({}): {}",
                    hit.token, hit.decision, hit.scope
                ),
            };
        }
    }

    let Some((slot, value)) = slot_of(claim) else {
        return ClaimReport {
            claim: rendered,
            verdict: Verdict::Unsupported,
            citations: Vec::new(),
            reason: "no binary slot to resolve; canon bears no matching fact".to_owned(),
        };
    };

    let certain = certain_values(world, &slot);
    if certain.contains(&value) {
        return ClaimReport {
            claim: rendered,
            verdict: Verdict::Supported,
            citations: slot_citations(world, &slot),
            reason: format!("{}.{} = {value} is asserted by canon", slot.0, slot.1),
        };
    }
    if certain.len() == 1 {
        let canon = certain.iter().next().cloned().unwrap_or_default();
        return ClaimReport {
            claim: rendered,
            verdict: Verdict::Contradicted,
            citations: slot_citations(world, &slot),
            reason: format!(
                "claims {}.{} = {value}, but canon asserts {canon}",
                slot.0, slot.1
            ),
        };
    }
    ClaimReport {
        claim: rendered,
        verdict: Verdict::Unsupported,
        citations: Vec::new(),
        reason: if certain.is_empty() {
            format!("canon bears no certain fact for {}.{}", slot.0, slot.1)
        } else {
            format!(
                "{}.{} is multi-valued; {value} is a new value canon does not assert",
                slot.0, slot.1
            )
        },
    }
}

/// Render an atom to its `pred(a, b)` surface form.
fn render_atom(atom: &AtomExpr) -> String {
    let args: Vec<String> = atom.args.iter().map(render_term).collect();
    format!("{}({})", atom.pred.text, args.join(", "))
}

// --- the killed-token guardrail store (the C-store) --------------------------

/// How a killed token is matched against claim text.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    /// A whole word, bounded by non-alphanumerics (the default).
    #[default]
    Word,
    /// A multi-word phrase; internal whitespace is flexible.
    Phrase,
    /// Anywhere in the text.
    Substring,
}

/// One killed token: a candidate the ledger struck down, refused re-entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KilledToken {
    /// The struck token (a name, a title, a compound element).
    pub token: String,
    /// How it is matched.
    #[serde(default)]
    pub mode: MatchMode,
    /// The decision that killed it.
    pub decision: String,
    /// The human note: what it was killed *as*.
    #[serde(default)]
    pub scope: String,
}

impl KilledToken {
    /// Whether this token appears in `text` under its match mode.
    ///
    /// Hyphen, space, and run-together spelling are treated as equivalent, so a
    /// killed `tide-bell` is caught whether a draft writes `tide-bell`,
    /// `tide bell`, or `tidebell` — the separator-stripping evasion (R2/D) does
    /// not slip a killed token past the guard. The modes differ only in strictness
    /// at the boundary: [`Word`](MatchMode::Word) and [`Phrase`](MatchMode::Phrase)
    /// match a whole token run and reject a longer word that merely contains it
    /// (`tidebells` is not `tide-bell`), while [`Substring`](MatchMode::Substring)
    /// is liberal and matches anywhere in the separator-stripped text.
    #[must_use]
    pub fn matches(&self, text: &str) -> bool {
        let needle = normalize(&self.token);
        if needle.is_empty() {
            return false;
        }
        let haystack = normalize(text);
        match self.mode {
            MatchMode::Substring => strip_spaces(&haystack).contains(&strip_spaces(&needle)),
            MatchMode::Word | MatchMode::Phrase => token_run_present(&haystack, &needle),
        }
    }
}

/// The negative-constraint store: the killed tokens whose reappearance realizes
/// rejected lore. The kernel reads no files, so a consuming repo compiles its
/// rejected ledger into this and attaches it with
/// [`CompiledWorld::with_killed_tokens`](super::CompiledWorld::with_killed_tokens).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KilledTokens {
    /// The killed tokens, in registry order.
    pub tokens: Vec<KilledToken>,
}

impl KilledTokens {
    /// A store from a list of killed tokens.
    #[must_use]
    pub fn new(tokens: Vec<KilledToken>) -> Self {
        Self { tokens }
    }

    /// The first killed token that appears in `text`, if any.
    #[must_use]
    pub fn hit(&self, text: &str) -> Option<&KilledToken> {
        self.tokens.iter().find(|token| token.matches(text))
    }

    /// Whether the store carries no tokens.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

/// Lowercase, and fold hyphens/underscores to spaces, collapsing runs of
/// whitespace — so `doom-wight` and `Doom Wight` normalize alike.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = true;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// A normalized string with its word separators removed — the concatenated
/// canonical form (`tide bell` → `tidebell`), so hyphenation, spacing, and
/// run-together spelling all compare equal.
fn strip_spaces(normalized: &str) -> String {
    normalized.split(' ').collect()
}

/// Whether some contiguous run of `haystack` words, concatenated, equals the
/// concatenated `needle`. Both are already [`normalize`]d.
///
/// This treats hyphen, space, and concatenation as equivalent at token
/// boundaries: `tide-bell` (normalized to `tide bell`) matches `tidebell` and
/// `tide bell` alike, because both reduce to the run `tidebell`. It never matches
/// a longer word that merely contains the needle (`tidebells`), because a run
/// must equal the needle exactly — that boundary is what separates
/// [`Word`](MatchMode::Word)/[`Phrase`](MatchMode::Phrase) from the liberal
/// [`Substring`](MatchMode::Substring).
fn token_run_present(haystack: &str, needle: &str) -> bool {
    let needle_concat = strip_spaces(needle);
    if needle_concat.is_empty() {
        return false;
    }
    let words: Vec<&str> = haystack
        .split(' ')
        .filter(|word| !word.is_empty())
        .collect();
    for start in 0..words.len() {
        let mut acc = String::new();
        for word in &words[start..] {
            acc.push_str(word);
            if acc == needle_concat {
                return true;
            }
            if acc.len() >= needle_concat.len() {
                break;
            }
        }
    }
    false
}
