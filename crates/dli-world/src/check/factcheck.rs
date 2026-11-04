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
//!   single-valued slot, the claim realizes a killed token, or it matches a
//!   contextual rejected claim.
//! - **unsupported** — no certain fact bears on the claim: candidate new lore,
//!   legal only where a hypothesis carries it, a defect when stated as settled.
//!
//! Single-valuedness is read from the data: a slot with exactly one certain value
//! is a functional dependency (spec 02 §4), so a different value contradicts it; a
//! slot with several certain values is a multi-valued relation, so a new value is
//! merely unsupported, never a contradiction. This keeps the checker from
//! false-flagging a legitimate second edge (a second adjacency, say) as a clash.

use dli_ast::term::AtomExpr;
use dli_core::fd::fd_value_key;
use dli_core::grade::Grade;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::CompiledWorld;
use super::continuity::{
    certain_values, fluent_class_member, render_term, slot_citations, slot_of,
};
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
///
/// The identity-closure canon map (WS-1) is built ONCE for the whole call —
/// `dli_world::check::continuity::identity_canon` — and threaded to every
/// claim's slot resolution, so `spouse(x, a)` reads the same certain layer
/// whether canon holds `x` or its certain alias.
#[must_use]
pub fn factcheck(world: &CompiledWorld, claims: &[AtomExpr]) -> Vec<ClaimReport> {
    let canon = super::continuity::identity_canon(world);
    claims
        .iter()
        .map(|claim| classify(world, &canon, claim))
        .collect()
}

/// Classify one claim.
#[allow(clippy::too_many_lines)]
fn classify(
    world: &CompiledWorld,
    alias: &BTreeMap<String, String>,
    claim: &AtomExpr,
) -> ClaimReport {
    let rendered = render_atom(claim);

    let slot = slot_of(claim);

    // A contextual guardrail rejects one ruled predicate/subject/value tuple.
    // Unlike a killed token it does not make the value illegal everywhere else.
    if let Some((slot, value)) = &slot {
        let rejected_hits = world.killed.rejected_hits(slot, value).collect::<Vec<_>>();
        if !rejected_hits.is_empty() {
            let citations = rejected_hits
                .iter()
                .map(|hit| hit.decision.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let reasons = rejected_hits
                .iter()
                .map(|hit| format!("{}: {}", hit.decision, hit.scope))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join("; ");
            return ClaimReport {
                claim: rendered,
                verdict: Verdict::Contradicted,
                citations,
                reason: format!("matches contextual rejected-claim ruling(s): {reasons}"),
            };
        }
    }

    // The `killed` registry predicate records a banned token as its payload; the
    // token is the fact's subject matter, not a realized usage, so it is exempt
    // from the killed-token substring scan (DL-0064). This
    // covers both the lore `killed(E, V)` form (`pred == "killed"`) and the
    // `declaration(E, killed, V, source)` form staged by `author.add_fact`
    // (`pred == "declaration"` with the `killed` attr as `args[1]`, since the
    // registry value embeds the very token being banned). The rejected_hits
    // (contextual rejected-claim) check above still applies to it.
    let registry_key_is_killed = claim.args.get(1).is_some_and(|term| match term {
        dli_ast::term::Term::Sym(id) => id.text.as_str() == "killed",
        dli_ast::term::Term::Str(text) => text.as_str() == "killed",
        _ => false,
    });
    let is_killed_registry_fact = claim.pred.text.as_str() == "killed"
        || (claim.pred.text.as_str() == "declaration" && registry_key_is_killed);
    if !is_killed_registry_fact {
        // Rejected lore first: a claim naming a killed token is contradicted outright.
        // Keep every matching ruling; one rejected form may have several authorities.
        let killed_hits = claim
            .args
            .iter()
            .filter_map(|term| match term {
                dli_ast::term::Term::Sym(id) => Some(id.text.as_str()),
                dli_ast::term::Term::Str(text) => Some(text.as_str()),
                _ => None,
            })
            .chain(std::iter::once(claim.pred.text.as_str()))
            .flat_map(|token| world.killed.hits(token))
            .collect::<Vec<_>>();
        if !killed_hits.is_empty() {
            let citations = killed_hits
                .iter()
                .map(|hit| hit.decision.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let reasons = killed_hits
                .iter()
                .map(|hit| format!("'{}' ({}): {}", hit.token, hit.decision, hit.scope))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join("; ");
            return ClaimReport {
                claim: rendered,
                verdict: Verdict::Contradicted,
                citations,
                reason: format!("realizes killed token ruling(s): {reasons}"),
            };
        }
    }

    let Some((slot, value)) = slot else {
        return ClaimReport {
            claim: rendered,
            verdict: Verdict::Unsupported,
            citations: Vec::new(),
            reason: "no binary slot to resolve; canon bears no matching fact".to_owned(),
        };
    };

    let certain = certain_values(world, alias, &slot);
    // Compare on the normalized value key, so cosmetic drift (`gold` vs `Gold`,
    // `bell-song` vs `bell song`) is not read as a distinct value — neither a
    // false contradiction nor a phantom second value that would silently switch
    // the detector off (issue 49, sub-claim 1). Distinctness is counted on the
    // keys; the raw spelling is kept for the message.
    let value_key = fd_value_key(&value);
    let certain_keys: BTreeSet<String> = certain.iter().map(|v| fd_value_key(v)).collect();
    // A fluent slot is interval-sensitive: a value canon holds over a disjoint
    // span is not a contradiction of a different value claimed here (issue 48).
    // Genuine interval overlaps are surfaced by the world-internal
    // `fluent-contradiction` check, so the fact-checker never returns
    // `contradicted` on a fluent slot — only supported (canon holds the value) or
    // unsupported. Identity-aware (WS-1): a slot merged with a fluent-holding
    // slot is judged as a fluent too, consulting every class member.
    if fluent_class_member(world, alias, &slot) {
        return if certain_keys.contains(&value_key) {
            ClaimReport {
                claim: rendered,
                verdict: Verdict::Supported,
                citations: slot_citations(world, alias, &slot),
                reason: format!(
                    "{}.{} = {value} holds over a fluent span in canon",
                    slot.0, slot.1
                ),
            }
        } else {
            ClaimReport {
                claim: rendered,
                verdict: Verdict::Unsupported,
                citations: Vec::new(),
                reason: format!(
                    "{}.{} is a fluent; {value} is not among canon's certain holdings",
                    slot.0, slot.1
                ),
            }
        };
    }
    if certain_keys.contains(&value_key) {
        return ClaimReport {
            claim: rendered,
            verdict: Verdict::Supported,
            citations: slot_citations(world, alias, &slot),
            reason: format!("{}.{} = {value} is asserted by canon", slot.0, slot.1),
        };
    }
    if certain_keys.len() == 1 {
        let canon = certain.iter().next().cloned().unwrap_or_default();
        return ClaimReport {
            claim: rendered,
            verdict: Verdict::Contradicted,
            citations: slot_citations(world, alias, &slot),
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
        reason: if certain_keys.is_empty() {
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

/// One rejected claim tuple: a value barred only for a specific binary slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RejectedClaim {
    /// Relation/predicate ID.
    pub predicate: String,
    /// Subject/entity ID.
    pub subject: String,
    /// Rejected object value.
    pub value: String,
    /// How the object value is matched.
    #[serde(default)]
    pub mode: MatchMode,
    /// The decision that rejected this tuple.
    pub decision: String,
    /// Human-readable description of the rejected claim.
    #[serde(default)]
    pub scope: String,
}

impl RejectedClaim {
    fn matches(&self, slot: &(String, String), value: &str) -> bool {
        normalize(&self.predicate) == normalize(&slot.0)
            && normalize(&self.subject) == normalize(&slot.1)
            && KilledToken {
                token: self.value.clone(),
                mode: self.mode,
                decision: self.decision.clone(),
                scope: self.scope.clone(),
            }
            .matches(value)
    }
}

impl KilledToken {
    /// Whether this token appears in `text` under its match mode.
    ///
    /// A killed `bell-song` is caught whether a draft writes `bell-song`,
    /// `bell song`, or `bellsong`: the separator-stripping evasion does not
    /// slip that token past the guard. A single-word needle does not match a
    /// split spelling in word or phrase mode. The modes differ in strictness
    /// at the boundary: [`Word`](MatchMode::Word) and [`Phrase`](MatchMode::Phrase)
    /// match a whole token run and reject a longer word that merely contains it
    /// (`bellsongs` is not `bellsong`), while [`Substring`](MatchMode::Substring)
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
            MatchMode::Word if !needle.contains(' ') => {
                haystack.split(' ').any(|word| word == needle)
            }
            MatchMode::Word | MatchMode::Phrase => phrase_present(&haystack, &needle),
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
    /// Claim tuples rejected only in their named predicate/subject slot.
    #[serde(default)]
    pub rejected_claims: Vec<RejectedClaim>,
}

impl KilledTokens {
    /// A store from a list of killed tokens.
    #[must_use]
    pub fn new(tokens: Vec<KilledToken>) -> Self {
        Self {
            tokens,
            rejected_claims: Vec::new(),
        }
    }

    /// Add contextual rejected claims to this store.
    #[must_use]
    pub fn with_rejected_claims(mut self, rejected_claims: Vec<RejectedClaim>) -> Self {
        self.rejected_claims = rejected_claims;
        self
    }

    /// The first killed token that appears in `text`, if any.
    #[must_use]
    pub fn hit(&self, text: &str) -> Option<&KilledToken> {
        self.tokens.iter().find(|token| token.matches(text))
    }

    /// Every killed-token ruling that matches `text`, in registry order.
    pub fn hits<'a>(&'a self, text: &'a str) -> impl Iterator<Item = &'a KilledToken> + 'a {
        self.tokens.iter().filter(move |token| token.matches(text))
    }

    /// Every contextual rejection matching this exact slot and object value.
    pub fn rejected_hits<'a>(
        &'a self,
        slot: &'a (String, String),
        value: &'a str,
    ) -> impl Iterator<Item = &'a RejectedClaim> + 'a {
        self.rejected_claims
            .iter()
            .filter(move |claim| claim.matches(slot, value))
    }

    /// Whether the store carries no tokens.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty() && self.rejected_claims.is_empty()
    }
}

/// Lowercase, and fold every non-alphanumeric run to a single space, collapsing
/// whitespace — so `shade-walker` and `Shade Walker` match alike. This is the liberal
/// fold the killed-token guardrail matches against (ban-evasion resistance); the
/// FD/contradiction path uses the stricter `fd_value_key`
/// ([`dli_core::fd::fd_value_key`], re-exported as [`crate::check::fd_value_key`])
/// instead, which must not merge distinct values.
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
/// canonical form (`bell song` → `bellsong`), so hyphenation, spacing, and
/// run-together spelling all compare equal.
fn strip_spaces(normalized: &str) -> String {
    normalized.split(' ').collect()
}

/// Whether `haystack` contains the normalized phrase or its run-together form.
/// Both inputs are already [`normalize`]d.
///
/// This treats hyphen, space, and concatenation as equivalent at token
/// boundaries: `bell-song` (normalized to `bell song`) matches `bellsong` and
/// `bell song` alike. It does not join arbitrary fragments such as `be llsong`,
/// and it never matches a longer word that merely contains the needle
/// (`bellsongs`). Those boundaries separate
/// [`Word`](MatchMode::Word)/[`Phrase`](MatchMode::Phrase) from the liberal
/// [`Substring`](MatchMode::Substring).
fn phrase_present(haystack: &str, needle: &str) -> bool {
    let needle_words: Vec<&str> = needle.split(' ').filter(|word| !word.is_empty()).collect();
    if needle_words.is_empty() {
        return false;
    }
    let haystack_words: Vec<&str> = haystack
        .split(' ')
        .filter(|word| !word.is_empty())
        .collect();
    haystack_words
        .windows(needle_words.len())
        .any(|window| window == needle_words)
        || haystack_words
            .iter()
            .any(|word| *word == strip_spaces(needle))
}
