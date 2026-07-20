//! Attestations: the corpus-adjacent store of recorded model judgments and the
//! attested-grade discipline that keeps them below tested and out of support.
//!
//! An attestation is a model judgment on a bounded question that will **never
//! receive human review — by design, not neglect** (spec
//! `07-verdicts-and-llm-boundary.md` §3). It is stored, graded, and replayable:
//! a committed record, not a transient answer. This module types that record,
//! the append-only [`AttestationStore`] it lives in, and the four discipline
//! seams the kernel enforces around it:
//!
//! - **The kernel never calls a model.** A check that needs an attestation the
//!   store lacks reports [`AttestFinding::Missing`] via [`require_attestation`] —
//!   a finding, never a guess or a default.
//! - **Staleness is mechanical.** Each record carries the content-fingerprint of
//!   what it judged ([`Attestation::fingerprint`]); [`Attestation::is_stale`]
//!   flips when the current content no longer matches, and the world-level
//!   [`crate::sweep`] returns exactly the re-attestation queue.
//! - **No consensus lifts the grade.** Samples and agreement live in the record,
//!   but consuming any number of agreeing attestations through
//!   [`dli_core::Grade::weakest`]/[`dli_core::Grade::join_alt`] or
//!   [`dli_core::Verdict::chain`] never yields a grade above
//!   [`Grade::Attested`] (proved in `tests/attest.rs`).
//! - **Ratification-exit** ([`ratification_exit`]) converts a judgment to a fact
//!   graded [`Grade::Checked`] — a lookup is exact — while the attestation chain
//!   is retained as history.
//!
//! Attestations are check-evidence, **never support-polynomial variables**
//! (DL-0006): an observation is not a creative fork, and certainty can never be
//! minted by model opinion. Nothing here ever writes canon.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::term::{AtomExpr, Term, TimeRef};
use dli_core::Grade;

/// A content fingerprint of what an attestation judged.
///
/// Rendered as `fnv1a64:<hex>`. This is the kernel's *staleness* fingerprint —
/// a deterministic, offline digest recomputed by [`crate::sweep`] and compared
/// for change detection; it needs no cryptographic strength because the factory
/// that writes the store is trusted and the threat is accidental drift, not
/// adversarial collision. It is deliberately distinct from the factory-supplied
/// `sha256:` hashes carried in [`Attestation::rationale_hash`] and
/// [`PromptRef::hash`], which the kernel only stores and compares, never
/// computes. The type is format-agnostic: swapping the one-line body of
/// [`fingerprint_facts`] for a `sha256` digest is the upgrade seam.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ContentHash(pub String);

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The stable identity of a bounded question: the key an attestation chain is
/// appended under. Distinct answers to the *same* question supersede along one
/// chain; different questions never collide.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct QuestionKey(pub String);

impl std::fmt::Display for QuestionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The bounded question an attestation answers — a claim with a fact-of-the-
/// matter *outside* the repo (a dialect's real basis, homophone plausibility,
/// tone-fit), exactly the kind of question to spend model time and zero author
/// time on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Question {
    /// The question class, e.g. `dialect-real-basis`.
    pub kind: String,
    /// The rendered atom under judgment, e.g. `name-basis(enfys, welsh-common-given)`.
    pub atom: String,
    /// The world entity whose current content decides staleness, e.g. `enfys`.
    /// The staleness anchor: when the world's facts about this subject change,
    /// the attestation goes stale and joins the re-attestation queue.
    pub subject: String,
}

impl Question {
    /// The stable identity of this question — `kind::atom`. The subject is the
    /// staleness anchor, not part of the identity: re-posing the same bounded
    /// question against fresh content supersedes along this key's chain.
    #[must_use]
    pub fn key(&self) -> QuestionKey {
        QuestionKey(format!("{}::{}", self.kind, self.atom))
    }
}

/// The model that produced an attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelRef {
    /// The provider, e.g. `bedrock`.
    pub provider: String,
    /// The model id, e.g. `claude-fable-5`. API aliases drift and models
    /// deprecate — this is why the record is replayable but the judgment only
    /// weakly so, and why attested sits below tested.
    pub id: String,
    /// The date the judgment was taken, e.g. `2026-07-19`.
    pub as_of: String,
}

/// The prompt an attestation was posed under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PromptRef {
    /// The template reference, e.g. `naming/dialect-basis@v3`.
    pub template: String,
    /// A `sha256:` hash of the rendered prompt, supplied by the factory.
    pub hash: String,
}

/// The sampling parameters, including consensus — recorded, but never able to
/// lift the grade (spec §3: "no amount of model consensus lifts the grade").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Sampling {
    /// The sampling temperature, rendered (e.g. `0.0`). Held as a string so the
    /// record stays `Eq`/`Hash`-clean — it is committed evidence, not a number
    /// the kernel does arithmetic on.
    pub temperature: String,
    /// The token ceiling.
    pub max_tokens: u32,
    /// How many samples were drawn.
    pub samples: u32,
    /// The agreement across samples, e.g. `3/3`. Three models agreeing is still
    /// `attested`; the grade changes only when the *procedure class* changes.
    pub agreement: String,
}

/// The re-attestation policy carried on a record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Policy {
    /// When to re-pose the question, e.g. `on-model-change`.
    pub reattest: String,
}

/// A recorded model answer to a bounded question, carrying everything needed to
/// replay the question and decide whether the record is still current.
///
/// The record is replayable — serialize it, ship it, commit it — but the
/// judgment is only weakly so. It is an evidence class that lives with the
/// corpus, machine-written and agent-committed, append-only per question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Attestation {
    /// The bounded question this record answers.
    pub question: Question,
    /// The recorded answer, e.g. `yes`.
    pub answer: String,
    /// A `sha256:` hash of the model's rationale; the full text lives in the
    /// sweep manifest. Factory-supplied — stored and compared, never recomputed.
    pub rationale_hash: String,
    /// The model that produced the judgment.
    pub model: ModelRef,
    /// The prompt the question was posed under.
    pub prompt: PromptRef,
    /// The sampling parameters and consensus.
    pub params: Sampling,
    /// The re-attestation policy.
    pub policy: Policy,
    /// The `rationale_hash` of the record this one supersedes, or `None` for the
    /// first answer to a question. Provenance metadata: the chain is retained
    /// and readable positionally in the [`AttestationStore`] regardless.
    pub supersedes: Option<String>,
    /// The sweep manifest that committed this record, e.g.
    /// `lab/campaign-082/runs/attest-0007.manifest.json`.
    pub sweep: String,
    /// The content-fingerprint of what the judgment rested on, captured when the
    /// attestation was taken. Staleness is `fingerprint != current`
    /// ([`Attestation::is_stale`]).
    pub fingerprint: ContentHash,
}

impl Attestation {
    /// The staleness anchor — the world entity whose current content decides
    /// whether this record is still current.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.question.subject
    }

    /// This record's identity within its question's chain — its `rationale_hash`,
    /// which a superseding record names in [`Attestation::supersedes`].
    #[must_use]
    pub fn record_id(&self) -> &str {
        &self.rationale_hash
    }

    /// Whether the content this judgment rested on has changed: the stored
    /// fingerprint no longer matches the `current` one.
    ///
    /// This is the whole staleness test — a pure comparison, no model call, no
    /// I/O. [`crate::sweep`] computes `current` for each head from the world and
    /// collects every record that flips.
    ///
    /// ```
    /// use dli_world::attest::{fingerprint_facts, ContentHash};
    /// use dli_ast::ident::Ident;
    /// use dli_ast::term::{AtomExpr, Term};
    ///
    /// let atom = AtomExpr {
    ///     pred: Ident::synthetic("rings"),
    ///     args: vec![Term::Sym(Ident::synthetic("tide-bell"))],
    /// };
    /// let taken = fingerprint_facts([&atom]);
    /// // Same content re-fingerprints identically — fresh.
    /// assert_eq!(taken, fingerprint_facts([&atom]));
    /// // A changed content set fingerprints differently — the staleness signal.
    /// let changed = fingerprint_facts(std::iter::empty::<&AtomExpr>());
    /// assert_ne!(taken, changed);
    /// ```
    #[must_use]
    pub fn is_stale(&self, current: &ContentHash) -> bool {
        self.fingerprint != *current
    }
}

/// The append-only store of recorded attestations — an evidence class that lives
/// with the corpus, like simulation manifests.
///
/// Append-only **per question**: a new answer to a question [`append`]s a
/// superseding record; the chain is retained and readable. There is **no
/// overwrite, replace, or remove API** — the only mutator is [`append`]. The
/// head record answers a question; the tail is history (the guardrail-corpus
/// rule, applied to model opinions).
///
/// [`append`]: AttestationStore::append
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestationStore {
    /// Every recorded attestation, in insertion order. Private so the only
    /// mutation is append — superseding never overwrites.
    attestations: Vec<Attestation>,
}

impl AttestationStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a record. A record for a question that already has a head
    /// **supersedes** it — both stay in the chain, readable via [`chain`]. This
    /// is the only mutator; there is deliberately no way to overwrite or remove.
    ///
    /// [`chain`]: AttestationStore::chain
    pub fn append(&mut self, record: Attestation) {
        self.attestations.push(record);
    }

    /// Every record, in insertion order.
    #[must_use]
    pub fn records(&self) -> &[Attestation] {
        &self.attestations
    }

    /// The head record answering `key` — the most recently appended for that
    /// question — or `None` if the question was never attested.
    #[must_use]
    pub fn head(&self, key: &QuestionKey) -> Option<&Attestation> {
        self.attestations
            .iter()
            .rev()
            .find(|record| record.question.key() == *key)
    }

    /// The full retained chain for `key`, oldest to newest — the head last. The
    /// tail before it is superseded history, never deleted.
    #[must_use]
    pub fn chain(&self, key: &QuestionKey) -> Vec<&Attestation> {
        self.attestations
            .iter()
            .filter(|record| record.question.key() == *key)
            .collect()
    }

    /// The current head of every distinct question, ordered by question key.
    /// This is what a sweep re-fingerprints — one record per question, never the
    /// superseded tail.
    #[must_use]
    pub fn heads(&self) -> Vec<&Attestation> {
        let mut latest: BTreeMap<QuestionKey, usize> = BTreeMap::new();
        for (index, record) in self.attestations.iter().enumerate() {
            latest.insert(record.question.key(), index);
        }
        latest
            .into_values()
            .map(|index| &self.attestations[index])
            .collect()
    }

    /// The distinct question keys the store holds, in key order.
    #[must_use]
    pub fn question_keys(&self) -> Vec<QuestionKey> {
        self.heads()
            .into_iter()
            .map(|record| record.question.key())
            .collect()
    }
}

/// A finding raised by the attestation discipline — a coded, domain-spoken
/// observation, never a silent fallback (charter koan 9, the two-channel error
/// model; koan 4, nothing silently dropped).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "finding", rename_all = "kebab-case")]
pub enum AttestFinding {
    /// A check demanded an attestation the store lacks. The kernel reports this
    /// rather than guessing or defaulting an answer: attestation-missing is a
    /// finding, because the kernel never calls a model (spec §3). The factory
    /// answers it by running a sweep and committing a record.
    Missing {
        /// The question whose attestation is absent.
        question: QuestionKey,
    },
}

impl AttestFinding {
    /// A stable, machine-readable class for this finding. Benchmarks and tools
    /// match on this; it never changes for a given variant. The serving layer
    /// surfaces the missing case through its `dli.attest.missing` op.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Missing { .. } => "attest.missing",
        }
    }

    /// A domain-spoken description of the finding (errors are lore).
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Missing { question } => format!(
                "the question '{question}' has no attestation in the store; \
                 a sweep must pose it before a check can consume it — the kernel \
                 will not guess an answer"
            ),
        }
    }
}

/// Look up the head attestation a check requires, or report it missing.
///
/// This is the attestation-missing seam: the kernel reads what a sweep
/// committed and **never** fabricates a judgment. An absent record is an
/// [`AttestFinding::Missing`], not a default.
///
/// ```
/// use dli_world::attest::{require_attestation, AttestationStore, QuestionKey};
///
/// let store = AttestationStore::new();
/// let key = QuestionKey("dialect-real-basis::name-basis(enfys, welsh)".to_owned());
/// let finding = require_attestation(&store, &key).unwrap_err();
/// assert_eq!(finding.code(), "attest.missing");
/// ```
pub fn require_attestation<'store>(
    store: &'store AttestationStore,
    key: &QuestionKey,
) -> Result<&'store Attestation, AttestFinding> {
    store.head(key).ok_or_else(|| AttestFinding::Missing {
        question: key.clone(),
    })
}

/// A question converted from model judgment to fact by an authorial ruling —
/// the ratification-exit (spec §3).
///
/// The ruling rides in the support channel (`ruling`), the procedure is now a
/// lookup and so the grade is [`Grade::Checked`] — a lookup is exact. The
/// attestation chain is **retained** in the store as history; this record only
/// references it by [`RatifiedFact::attested_history`], the two-channel
/// semantics demonstrating itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RatifiedFact {
    /// The question, now settled as fact.
    pub question: Question,
    /// The settled answer, carried over from the attestation head.
    pub answer: String,
    /// The decision citation the fact now rests on, e.g. `D-0146`. Support cites
    /// the ledger; the machine never wrote it.
    pub ruling: String,
    /// Always [`Grade::Checked`]: a lookup re-derives exactly.
    pub grade: Grade,
    /// The retained attestation chain this fact exited from, by question key.
    /// The chain stays in the store — killed judgments are never deleted.
    pub attested_history: QuestionKey,
}

/// Convert an attestation head plus an authorial ruling into a fact-shaped
/// record graded [`Grade::Checked`], retaining the chain reference.
///
/// The author may review an attested question whenever they please; the ruling
/// converts the question from judgment to fact. The consuming check becomes a
/// lookup, the grade becomes `checked`, and the attestation chain is retained as
/// history. Canon moves by valuation, never by this edit.
///
/// ```
/// use dli_core::Grade;
/// use dli_world::attest::{ratification_exit, Attestation, ContentHash, ModelRef,
///     Policy, PromptRef, Question, Sampling};
///
/// let head = Attestation {
///     question: Question {
///         kind: "dialect-real-basis".to_owned(),
///         atom: "name-basis(enfys, welsh)".to_owned(),
///         subject: "enfys".to_owned(),
///     },
///     answer: "yes".to_owned(),
///     rationale_hash: "sha256:c41a".to_owned(),
///     model: ModelRef { provider: "bedrock".to_owned(), id: "claude-fable-5".to_owned(),
///         as_of: "2026-07-19".to_owned() },
///     prompt: PromptRef { template: "naming/dialect-basis@v3".to_owned(),
///         hash: "sha256:88d1".to_owned() },
///     params: Sampling { temperature: "0.0".to_owned(), max_tokens: 400, samples: 3,
///         agreement: "3/3".to_owned() },
///     policy: Policy { reattest: "on-model-change".to_owned() },
///     supersedes: None,
///     sweep: "lab/campaign-082/runs/attest-0007.manifest.json".to_owned(),
///     fingerprint: ContentHash("fnv1a64:0000000000000000".to_owned()),
/// };
///
/// let fact = ratification_exit(&head, "D-0146").expect("a real ruling");
/// assert_eq!(fact.grade, Grade::Checked);  // a lookup is exact
/// assert_eq!(fact.ruling, "D-0146");        // fiat rides in support
/// assert_eq!(fact.attested_history, head.question.key()); // chain retained
///
/// // Exit without a ruling is impossible — the ruling IS the exit.
/// assert!(ratification_exit(&head, "").is_none());
/// assert!(ratification_exit(&head, "   ").is_none());
/// ```
#[must_use]
pub fn ratification_exit(head: &Attestation, ruling: impl Into<String>) -> Option<RatifiedFact> {
    let ruling = ruling.into();
    if ruling.trim().is_empty() {
        return None;
    }
    Some(RatifiedFact {
        question: head.question.clone(),
        answer: head.answer.clone(),
        ruling,
        grade: Grade::Checked,
        attested_history: head.question.key(),
    })
}

/// Fingerprint a set of world facts into a [`ContentHash`], order-independent.
///
/// The renderings are sorted and de-duplicated before hashing, so the
/// fingerprint depends on the *content* of the fact set and not on iteration
/// order or repetition. This is the projection [`crate::sweep`] recomputes for a
/// subject's current facts and compares against each head's stored fingerprint;
/// the factory computes the stored fingerprint the same way when it takes the
/// attestation. Source spans are excluded — only the rendered atom (predicate
/// and arguments) is hashed — so an unrelated edit that moves a fact does not
/// spuriously stale it.
///
/// ```
/// use dli_world::attest::fingerprint_facts;
/// use dli_ast::ident::Ident;
/// use dli_ast::term::{AtomExpr, Term};
///
/// let a = AtomExpr { pred: Ident::synthetic("rings"),
///     args: vec![Term::Sym(Ident::synthetic("tide-bell"))] };
/// let b = AtomExpr { pred: Ident::synthetic("adjacent"),
///     args: vec![Term::Sym(Ident::synthetic("saltmarket")),
///                Term::Sym(Ident::synthetic("the-shallows"))] };
///
/// // Order-independent: {a, b} fingerprints the same either way.
/// assert_eq!(fingerprint_facts([&a, &b]), fingerprint_facts([&b, &a]));
/// // Content-sensitive: {a} differs from {a, b}.
/// assert_ne!(fingerprint_facts([&a]), fingerprint_facts([&a, &b]));
/// ```
#[must_use]
pub fn fingerprint_facts<'a>(facts: impl IntoIterator<Item = &'a AtomExpr>) -> ContentHash {
    let mut rendered: Vec<String> = facts.into_iter().map(render_atom).collect();
    rendered.sort_unstable();
    rendered.dedup();
    let preimage = rendered.join("\n");
    ContentHash(format!("fnv1a64:{:016x}", fnv1a64(preimage.as_bytes())))
}

/// A canonical, span-free rendering of an atom, e.g. `rings(tide-bell)`.
fn render_atom(atom: &AtomExpr) -> String {
    let args: Vec<String> = atom.args.iter().map(render_term).collect();
    format!("{}({})", atom.pred.text, args.join(","))
}

/// A canonical, span-free rendering of a term.
fn render_term(term: &Term) -> String {
    match term {
        Term::Var(id) => format!("?{}", id.text),
        Term::Sym(id) => id.text.clone(),
        Term::Str(text) => format!("{text:?}"),
        Term::Int(value) => value.to_string(),
        Term::Time(TimeRef::Era(id)) => format!("era:{}", id.text),
        Term::Time(TimeRef::Event(id)) => format!("event:{}", id.text),
    }
}

/// A 64-bit FNV-1a hash — a small, dependency-free, version-stable content
/// digest sufficient for accidental-drift detection (see [`ContentHash`]).
fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}
