//! The fat-payload pipeline (`PAY-1`/`PAY-2`/`PAY-3`).
//!
//! A [`Payload`] (`payload/v1`) is the structured shadow of a drafted episode:
//! `meta`, `claims`, `deltas`, `scenes`, `beats`, `promises`, `proof_ledgers`,
//! `names`, and `lexicon`
//! ([`06-craft-and-payloads`](../../../../docs/spec/06-craft-and-payloads.md)
//! §7). A draft is prose and prose never enters the kernel; the shadow is the
//! replayable evidence the battery runs over. `PAY-1` lands the schema plus the
//! two verifications that make the shadow trustworthy (§7.4):
//!
//! - **Quote anchoring is a hard error.** Every claim, beat, and promise carries
//!   a verbatim `quote` and a `line`; [`verify_anchors`] checks the quote appears
//!   verbatim at the cited line in the *real draft*, so an extractor can never
//!   hallucinate an anchor and every downstream finding renders real text.
//! - **Staleness is by source hash.** [`Payload::meta`] pins the draft's
//!   `sha256`; [`check_staleness`] re-hashes the draft and flags the payload for
//!   re-extraction the moment the source moves underneath it.
//!
//! # Legacy thin projection
//!
//! [`DraftPayload`](crate::draft::DraftPayload) is the superseded thin library
//! projection. The contract, worker, CLI, durable evidence store, and merged
//! battery all consume [`Payload`]; the old type remains only for source
//! compatibility with library callers that have not migrated.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use dli_ast::clause::Clause;
use dli_ast::decl::{Decl, Document, ProofDecl};
use dli_ast::span::Span;
use dli_ast::term::AtomExpr;
use dli_core::grade::Grade;
use dli_ledger::check_proof;
use dli_ledger::practice::compile_practices;
use dli_syntax::{parse_clause_atom, parse_proof_close_parts};

use crate::check::factcheck::Verdict;
use crate::check::{self, CompiledWorld, Severity, WorldFinding};
use crate::craft::config::{DEFAULT_FINDING_POLICY, FindingPolicy};
use crate::craft::kb::Kb;
use crate::craft::outline::EpisodeOutline;
use crate::craft::season::{
    CraftLedger, DEFAULT_EPISODE_POLICY, DEFAULT_THRESHOLDS, EpisodeMeta, EpisodePolicy,
    LedgerBeat, LedgerPromise, LedgerScene, Thresholds, check_season_rules_with_policy,
};
use crate::craft::{PromiseKind, TryFailOutcome};
use crate::language::{aural_clash, vet_candidate};
use crate::scene_projection::project_scene;

// --- the merged report finding (the seam `PAY-2` fills) ----------------------

/// One finding in the merged payload report.
///
/// The battery over a [`Payload`] emits these; `PAY-1` emits the anchoring and
/// staleness families, and the remaining families (continuity, factcheck,
/// outline-conformance, craft, proof, naming, phase, lexicon) merge in here in
/// `PAY-2`. Every finding carries the [`code`](Self::code) of the rule or check
/// that raised it and the [`grade`](Self::grade) of the evidence. Findings that
/// turn on an extracted source item also carry its draft [`line`](Self::line)
/// and verbatim [`quote`](Self::quote); craft and lore findings additionally
/// carry their [`principle`](Self::principle) and
/// [`citations`](Self::citations) when available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PayloadFinding {
    /// The stable dotted class code (`family.item`), e.g. `anchor.quote-missing`.
    pub code: String,
    /// How much the finding must be attended to.
    pub severity: Severity,
    /// The verification grade of the check that produced it. Every deterministic
    /// kernel check re-derives exactly, so `PAY-1`'s findings carry
    /// [`Grade::Checked`].
    pub grade: Grade,
    /// A domain-spoken description of the finding.
    pub message: String,
    /// The draft line the finding turns on, when it anchors to one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub line: Option<u32>,
    /// The verbatim quote from the draft the finding renders, when it has one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub quote: Option<String>,
    /// The craft principle this finding cites, when the check is a craft rule.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub principle: Option<String>,
    /// Ledger citations backing the finding (`d-nnnn`), when it turns on a ruling.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub citations: Vec<String>,
}

impl PayloadFinding {
    /// An error-severity finding. Deterministic kernel checks re-derive exactly,
    /// so it carries [`Grade::Checked`].
    #[must_use]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            grade: Grade::Checked,
            message: message.into(),
            line: None,
            quote: None,
            principle: None,
            citations: Vec::new(),
        }
    }

    /// Anchor this finding to a draft line, builder-style.
    #[must_use]
    pub fn at_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Attach the verbatim quote the finding renders, builder-style.
    #[must_use]
    pub fn with_quote(mut self, quote: impl Into<String>) -> Self {
        self.quote = Some(quote.into());
        self
    }

    /// The finding's family — the segment of [`code`](Self::code) before the dot.
    #[must_use]
    pub fn family(&self) -> &str {
        self.code.split('.').next().unwrap_or(&self.code)
    }
}

// --- the payload/v1 schema ---------------------------------------------------

/// The structured shadow of a drafted episode — `payload/v1` (§7.1).
///
/// The top-level keys mirror the spec's worked YAML exactly: [`meta`](Self::meta)
/// (source hash and extractor manifest), [`claims`](Self::claims),
/// [`deltas`](Self::deltas), [`scenes`](Self::scenes), [`beats`](Self::beats),
/// [`promises`](Self::promises), [`proof_ledgers`](Self::proof_ledgers),
/// [`names`](Self::names), and [`lexicon`](Self::lexicon).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    /// Source provenance and the extractor manifest.
    #[serde(default)]
    pub meta: PayloadMeta,
    /// Factcheck atoms, each anchored to a draft quote and line.
    #[serde(default)]
    pub claims: Vec<Claim>,
    /// Knowledge / possession / location / injury deltas the draft records.
    #[serde(default)]
    pub deltas: Vec<DeltaRow>,
    /// Scenes: cast, presence, location, phase observations.
    #[serde(default)]
    pub scenes: Vec<SceneRow>,
    /// Beats: tries, fails, and accomplishments with threads and magnitudes.
    #[serde(default)]
    pub beats: Vec<BeatRow>,
    /// Promises opened and closed.
    #[serde(default)]
    pub promises: PromiseLedger,
    /// Per-scene proof ledgers, replayed through the practice checker in `PAY-2`.
    #[serde(default)]
    pub proof_ledgers: Vec<ProofLedgerRow>,
    /// Proper names introduced.
    #[serde(default)]
    pub names: NameIntro,
    /// The deterministic lexicon and banned-vocabulary scan (no model involved).
    #[serde(default)]
    pub lexicon: Lexicon,
}

/// The payload header: source provenance and the extractor manifest (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PayloadMeta {
    /// The episode this payload shadows.
    #[serde(default)]
    pub episode: String,
    /// The source draft this payload pins.
    #[serde(default)]
    pub source: SourceRef,
    /// The date the payload was extracted (`YYYY-MM-DD`).
    #[serde(default)]
    pub extracted: String,
    /// The extractor manifest: schema version and per-field extractor identity.
    #[serde(default)]
    pub extractors: ExtractorManifest,
}

/// The draft a payload shadows, pinned by content hash (§7.4 staleness).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    /// The draft file name.
    #[serde(default)]
    pub file: String,
    /// The `sha256` hex of the draft content this payload shadows. A `sha256:`
    /// prefix is tolerated on read ([`check_staleness`]).
    #[serde(default)]
    pub sha256: String,
    /// The draft word count at extraction time.
    #[serde(default)]
    pub words: u32,
}

/// The extractor manifest: the schema version, and the model + prompt that
/// produced each extracted field (§7.1).
///
/// The lexicon and banned-vocabulary rows are deterministic scans with no model
/// involved, so they carry no [`ExtractorRef`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtractorManifest {
    /// The payload schema version, e.g. `payload/v1`.
    #[serde(default)]
    pub schema: String,
    /// The extractor identity for each extracted field (`beats`, `claims`, …).
    #[serde(default)]
    pub fields: BTreeMap<String, ExtractorRef>,
}

/// The identity of the model and prompt that produced one extracted field (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtractorRef {
    /// The model that produced the field.
    pub model: String,
    /// The `sha256:` hash of the rendered extraction prompt.
    pub prompt: String,
}

/// A factcheck atom, anchored to the draft (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    /// The atom in surface form, e.g. `holds(enfys, consent(household, door))`.
    /// `PAY-2` parses this into a typed atom for the fact-checker.
    pub atom: String,
    /// The verbatim draft text this claim is drawn from.
    pub quote: String,
    /// The draft line the quote sits on (1-based).
    pub line: u32,
}

/// One normalized fact in caller-owned runtime continuity context.
///
/// This is deliberately not part of [`Payload`]: prior episode state is mutable
/// runtime context, while `payload/v1` is durable evidence about one immutable
/// draft. `dli.check-draft` may supply these rows through [`BatteryContext`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFact {
    /// The fact predicate / slot key.
    pub predicate: String,
    /// The entity whose slot is set.
    pub subject: String,
    /// The normalized value stored in the slot.
    pub value: String,
}

/// One explicit state transition in caller-owned runtime continuity context.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeChange {
    /// The fact predicate / slot key.
    pub predicate: String,
    /// The entity whose slot changes.
    pub subject: String,
    /// The value the episode says it changes from.
    pub from_value: String,
    /// The value the episode changes to.
    pub to_value: String,
}

/// Caller-owned state needed to check one episode against its predecessors.
///
/// The context contains the current episode's structured state projection plus
/// the already-folded prior ledger. It never enters the durable payload store.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeContinuity {
    /// Facts the current episode presents as established.
    #[serde(default)]
    pub established: Vec<RuntimeFact>,
    /// Facts the current episode changes.
    #[serde(default)]
    pub changes: Vec<RuntimeChange>,
    /// Entity ids referenced by the current episode state.
    #[serde(default)]
    pub references: Vec<String>,
    /// The last value established before this episode for each known slot.
    #[serde(default)]
    pub prior_facts: Vec<RuntimeFact>,
    /// Promise/debt ids open before this episode.
    #[serde(default)]
    pub prior_open_promises: Vec<String>,
    /// Promise/debt ids the current episode closes without also opening.
    #[serde(default)]
    pub closed_promises: Vec<String>,
}

/// The dimension a [`DeltaRow`] moves (§7.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DeltaKind {
    /// A character (or the listener) learns a fact.
    #[default]
    Knowledge,
    /// Possession of an object changes hands.
    Possession,
    /// A character's location changes.
    Location,
    /// A character takes or heals an injury.
    Injury,
}

/// A knowledge / possession / location / injury delta the draft records (§7.1).
///
/// Deltas carry no quote and are not quote-anchored — they are the state moves
/// the continuity fold consumes, keyed by scene and line for provenance.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeltaRow {
    /// The dimension moved.
    pub kind: DeltaKind,
    /// Whose state changes.
    pub who: String,
    /// The fact / object / place / injury moved.
    pub fact: String,
    /// The scene the delta happens in.
    pub scene: String,
    /// The draft line the delta is drawn from (1-based).
    pub line: u32,
}

/// A scene's cast, presence, location, and phase observations (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneRow {
    /// The scene identifier.
    pub id: String,
    /// The on-mic cast (active speakers).
    #[serde(default)]
    pub cast: Vec<String>,
    /// Where the scene takes place.
    #[serde(default)]
    pub location: Option<String>,
    /// The phase in force for each cycle in the scene (`tide` → `slack`).
    #[serde(default)]
    pub phases: BTreeMap<String, String>,
    /// Cast present but off-mic.
    #[serde(default)]
    pub off_mic: Vec<String>,
}

/// A beat's kind — a try, a fail, or an accomplishment (§7.1, §3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BeatKind {
    /// An attempt.
    #[default]
    Try,
    /// A failed attempt.
    Fail,
    /// An accomplishment — the beat the season's `accomplishment-earned` rule
    /// joins over.
    Accomplishment,
}

/// A beat's dramatic magnitude — the scale the `accomplishment-earned` rule
/// reads (§3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Magnitude {
    /// A minor beat.
    #[default]
    Minor,
    /// A major beat — the `accomplishment-earned` threshold.
    Major,
    /// A climax-class beat — requires two prior failures on its thread.
    Climax,
}

/// A beat: a try, fail, or accomplishment with a thread and a magnitude (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BeatRow {
    /// The beat identifier.
    pub id: String,
    /// The scene the beat sits in.
    pub scene: String,
    /// The beat's kind.
    pub kind: BeatKind,
    /// The thread the beat belongs to — the identity the season join runs on.
    pub thread: String,
    /// The beat's magnitude.
    pub magnitude: Magnitude,
    /// The try/fail typing, when the extractor carried it — sharpens the season
    /// `accomplishment-earned` join (spec §3 reads the failure set
    /// {yes-but, no-but, no-and} from here; `None` degrades to reading
    /// [`kind`](Self::kind), so a payload that records only the coarse
    /// try/fail/accomplishment signal still joins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<TryFailOutcome>,
    /// Who performs the beat.
    #[serde(default)]
    pub by: String,
    /// Whether the beat was assisted.
    #[serde(default)]
    pub assisted: bool,
    /// The verbatim draft text this beat is drawn from.
    pub quote: String,
    /// The draft line the quote sits on (1-based).
    pub line: u32,
}

/// The promises a payload opens and closes (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromiseLedger {
    /// Promises opened in this episode.
    #[serde(default)]
    pub opened: Vec<PromiseRow>,
    /// Promises closed in this episode.
    #[serde(default)]
    pub closed: Vec<PromiseRow>,
}

/// One promise the draft opens or closes, anchored to its opening/closing quote
/// (§7.1). [`kind`](Self::kind) reuses the outline schema's [`PromiseKind`] so a
/// payload promise and an outline promise speak the same vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromiseRow {
    /// The promise identifier.
    pub id: String,
    /// The kind of promise.
    pub kind: PromiseKind,
    /// The verbatim draft text the promise is drawn from.
    pub quote: String,
    /// The draft line the quote sits on (1-based).
    pub line: u32,
}

/// A scene's proof ledger — the shadow of a diegetic proof block (§7.1).
///
/// `PAY-2` replays it through the practice checker with the scene projection
/// ([`04-practices`](../../../../docs/spec/04-practices.md)).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofLedgerRow {
    /// The scene the ledger belongs to.
    pub scene: String,
    /// The ledger itself.
    pub ledger: ProofLedger,
}

/// The premises, steps, and close of a shadowed proof (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofLedger {
    /// The premises the casting rests on, in surface form.
    #[serde(default)]
    pub premises: Vec<String>,
    /// The proof steps, in surface form.
    #[serde(default)]
    pub steps: Vec<String>,
    /// The casting's conclusion and expected verdict.
    #[serde(default)]
    pub close: ProofClose,
}

/// A proof's conclusion: the atom it closes on and the verdict it expects (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProofClose {
    /// The atom the casting closes on, in surface form.
    pub atom: String,
    /// The expected verdict token (`holds`, `blocked`, `fails-at <error>`);
    /// `PAY-2` maps it to the ledger checker's
    /// [`Expectation`](dli_ledger::Expectation).
    #[serde(default)]
    pub expect: String,
}

/// The proper names a payload introduces (§7.1). Each is oracle-vetted or
/// hypothesis-carried by the naming family in `PAY-2`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NameIntro {
    /// Names first used in this episode.
    #[serde(default)]
    pub introduced: Vec<String>,
}

/// The deterministic lexicon scan carried in the payload (§7.1).
///
/// No model is involved: these ride along so the report is one artifact, but they
/// are not encodings and carry no extractor.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Lexicon {
    /// Banned-vocabulary hits the deterministic scan found (empty in a clean
    /// draft).
    #[serde(default)]
    pub banned_hits: Vec<String>,
    /// The count of register-true tokens the scan saw.
    #[serde(default)]
    pub register_tokens: u32,
}

// --- anchoring and staleness (§7.4) ------------------------------------------

/// The `sha256` hex of a draft's content — the hash a payload pins in
/// [`SourceRef::sha256`] to detect staleness.
#[must_use]
pub fn source_hash(source: &str) -> String {
    let digest = Sha256::digest(source.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        // Writing to a String is infallible.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Verify a payload against its source draft: the anchoring and staleness
/// families (§7.4).
///
/// Runs [`check_staleness`] then [`verify_anchors`] and returns one merged list.
/// Both are hard errors: a payload whose source has moved, or that anchors a
/// quote the draft does not bear at the cited line, is not a trustworthy shadow.
///
/// ```
/// use dli_world::payload::{Claim, Payload, SourceRef, source_hash, verify};
///
/// let source = "the house had said yes to her, in its way\nthe withy held\n";
/// let mut payload = Payload::default();
/// payload.meta.source = SourceRef { sha256: source_hash(source), ..SourceRef::default() };
/// payload.claims.push(Claim {
///     atom: "holds(enfys, consent(household, door))".to_owned(),
///     quote: "the house had said yes to her".to_owned(),
///     line: 1,
/// });
/// assert!(verify(&payload, source).is_empty());
///
/// // Move the quote off its cited line and the anchor fails hard.
/// payload.claims[0].line = 2;
/// let findings = verify(&payload, source);
/// assert_eq!(findings.len(), 1);
/// assert_eq!(findings[0].code, "anchor.quote-misplaced");
/// ```
#[must_use]
pub fn verify(payload: &Payload, source: &str) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    out.extend(check_staleness(payload, source));
    out.extend(verify_anchors(payload, source));
    out
}

/// Flag a payload whose source draft has moved underneath it (§7.4).
///
/// Re-hashes `source` and compares to the [`sha256`](SourceRef::sha256) the
/// payload pins. A mismatch is a hard `payload.stale` error queuing the payload
/// for re-extraction — the same family as the propagation staleness linter. A
/// `sha256:` prefix on the pinned hash is tolerated; an unpinned (empty) hash can
/// never match, so it too flags stale (fail closed — an unpinned shadow cannot be
/// trusted).
#[must_use]
pub fn check_staleness(payload: &Payload, source: &str) -> Option<PayloadFinding> {
    let actual = source_hash(source);
    let pinned = payload
        .meta
        .source
        .sha256
        .trim()
        .trim_start_matches("sha256:")
        .to_ascii_lowercase();
    if pinned == actual {
        return None;
    }
    Some(PayloadFinding::error(
        "payload.stale",
        format!(
            "payload pins source hash '{pinned}' but the draft now hashes '{actual}'; re-extract"
        ),
    ))
}

/// Verify every quote anchor in the payload against the source draft (§7.4).
///
/// Quote anchoring is a **hard error**: every claim, beat, and opened/closed
/// promise carries a verbatim `quote` and a `line`, and the quote must appear
/// verbatim at that line in `source`. A quote absent from the draft
/// (`anchor.quote-missing`), sitting on a different line
/// (`anchor.quote-misplaced`), or empty (`anchor.empty-quote`) each fails — so an
/// extractor cannot hallucinate an anchor and every downstream finding renders
/// real draft text. Deltas carry no quote and are not anchored.
#[must_use]
pub fn verify_anchors(payload: &Payload, source: &str) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    for claim in &payload.claims {
        out.extend(check_anchor(
            source,
            "claim",
            &claim.atom,
            &claim.quote,
            claim.line,
        ));
    }
    for beat in &payload.beats {
        out.extend(check_anchor(
            source,
            "beat",
            &beat.id,
            &beat.quote,
            beat.line,
        ));
    }
    for promise in &payload.promises.opened {
        out.extend(check_anchor(
            source,
            "opened promise",
            &promise.id,
            &promise.quote,
            promise.line,
        ));
    }
    for promise in &payload.promises.closed {
        out.extend(check_anchor(
            source,
            "closed promise",
            &promise.id,
            &promise.quote,
            promise.line,
        ));
    }
    out
}

/// Check one quote anchor, naming the offender by `kind` and `id`. Returns the
/// hard-error finding if the quote is empty, absent, or not on its cited line.
fn check_anchor(
    source: &str,
    kind: &str,
    id: &str,
    quote: &str,
    line: u32,
) -> Option<PayloadFinding> {
    if quote.is_empty() {
        return Some(
            PayloadFinding::error(
                "anchor.empty-quote",
                format!("{kind} '{id}' carries no quote to anchor at line {line}"),
            )
            .at_line(line),
        );
    }
    // A verbatim quote may occur more than once; the anchor holds if any
    // occurrence starts on the cited line.
    let match_lines: Vec<u32> = source
        .match_indices(quote)
        .map(|(byte, _)| line_of_byte(source, byte))
        .collect();
    if match_lines.contains(&line) {
        return None;
    }
    if match_lines.is_empty() {
        Some(
            PayloadFinding::error(
                "anchor.quote-missing",
                format!(
                    "{kind} '{id}': quote does not appear anywhere in the draft \
                     (claimed at line {line})"
                ),
            )
            .at_line(line)
            .with_quote(quote),
        )
    } else {
        let seen = match_lines
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        Some(
            PayloadFinding::error(
                "anchor.quote-misplaced",
                format!(
                    "{kind} '{id}': quote does not appear at line {line} \
                     (found at line(s) {seen})"
                ),
            )
            .at_line(line)
            .with_quote(quote),
        )
    }
}

/// The 1-based line number containing byte offset `byte` in `source`. `byte` is
/// a char boundary (a [`str::match_indices`] result), so the slice is valid.
fn line_of_byte(source: &str, byte: usize) -> u32 {
    let newlines = source[..byte].matches('\n').count();
    u32::try_from(newlines)
        .unwrap_or(u32::MAX)
        .saturating_add(1)
}

// --- the merged eight-family battery (`PAY-2`) -------------------------------

/// The context a full [`check_payload`] run needs beyond the payload itself.
///
/// The [`source`](Self::source) draft is required (anchoring + staleness). The
/// [`outline`](Self::outline) or [`outlines`](Self::outlines) enables the
/// two-sided outline-conformance diff (`PAY-3`); the [`season`](Self::season) is
/// the committed season ledger the
/// payload's beats merge into so the craft-season joins reach across episodes
/// (empty = the payload is checked against itself alone); and
/// [`thresholds`](Self::thresholds) is the craft policy data
/// ([`DEFAULT_THRESHOLDS`] when uncalibrated).
#[derive(Debug, Clone)]
pub struct BatteryContext<'a> {
    /// The draft prose the payload shadows (for anchoring + staleness).
    pub source: &'a str,
    /// The episode outline to diff against (two-sided outline-conformance).
    pub outline: Option<&'a EpisodeOutline>,
    /// The caller project's reviewed episode outlines. When supplied, the
    /// payload episode must resolve to exactly one of them.
    pub outlines: Option<&'a [EpisodeOutline]>,
    /// The committed season ledger the payload's beats merge into.
    pub season: Option<&'a CraftLedger>,
    /// The craft-rule thresholds (policy data).
    pub thresholds: Thresholds,
    /// The caller's outline and finding policy.
    pub episode_policy: EpisodePolicy<'static>,
    /// The knowledge base the craft family queries for governing principles
    /// ([`Kb::builtin`] by default).
    pub kb: Kb<'a>,
    /// Parsed source documents containing practices and declared proof blocks.
    pub documents: &'a [Document],
    /// Caller-owned prior/current episode state used only for runtime continuity.
    pub runtime_continuity: Option<&'a RuntimeContinuity>,
}

impl<'a> BatteryContext<'a> {
    /// A context over just the draft source, with default thresholds and no
    /// outline or season — the payload checked against the world and itself.
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            outline: None,
            outlines: None,
            season: None,
            thresholds: DEFAULT_THRESHOLDS,
            episode_policy: DEFAULT_EPISODE_POLICY,
            kb: Kb::builtin(),
            documents: &[],
            runtime_continuity: None,
        }
    }

    /// Add the episode outline to diff against, builder-style.
    #[must_use]
    pub fn with_outline(mut self, outline: &'a EpisodeOutline) -> Self {
        self.outline = Some(outline);
        self
    }

    /// Add the caller project's reviewed outline catalog, builder-style.
    #[must_use]
    pub fn with_outlines(mut self, outlines: &'a [EpisodeOutline]) -> Self {
        self.outlines = Some(outlines);
        self
    }

    /// Add the committed season ledger to merge into, builder-style.
    #[must_use]
    pub fn with_season(mut self, season: &'a CraftLedger) -> Self {
        self.season = Some(season);
        self
    }

    /// Override the craft-rule thresholds (policy data), builder-style.
    #[must_use]
    pub fn with_thresholds(mut self, thresholds: Thresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Override the caller's outline and finding policy, builder-style.
    #[must_use]
    pub fn with_episode_policy(mut self, policy: EpisodePolicy<'static>) -> Self {
        self.episode_policy = policy;
        self
    }

    /// Override the knowledge base the craft family queries, builder-style.
    #[must_use]
    pub fn with_kb(mut self, kb: Kb<'a>) -> Self {
        self.kb = kb;
        self
    }

    /// Add the source documents whose declared practices and proofs the payload
    /// shadows.
    #[must_use]
    pub fn with_documents(mut self, documents: &'a [Document]) -> Self {
        self.documents = documents;
        self
    }

    /// Add caller-owned runtime episode state without changing `payload/v1`.
    #[must_use]
    pub fn with_runtime_continuity(mut self, continuity: &'a RuntimeContinuity) -> Self {
        self.runtime_continuity = Some(continuity);
        self
    }
}

/// Run the full battery over one payload — the merged eight-family report (§7.2).
///
/// The kernel runs everything it has against the one artifact and returns **one**
/// merged, deterministically-ordered list of [`PayloadFinding`]s, each carrying
/// its draft line + verbatim quote (where it anchors to one), rule/check id,
/// principle citation (craft), support, and grade. The families:
///
/// 1. **Continuity** — the structural continuity classes over the payload's claims.
/// 2. **Factcheck** — claims resolve to supported / contradicted / unsupported.
/// 3. **Outline-conformance** — the two-sided diff, when an outline is supplied (`PAY-3`).
/// 4. **Craft rules** — §3's season battery, with the payload's beats merged in.
/// 5. **Proof checks** — proof shadows match declared proofs and replay through
///    their declared practice.
/// 6. **Naming** — introduced names vetted by the oracle.
/// 7. **Phase and lexicon checks** — observations follow declared cycle walks;
///    deterministic banned-word scans are independently reproduced.
/// 8. **Quote anchoring and staleness** — every quote is verbatim and the source
///    hash still pins the draft.
///
/// This is the battery behind `dli.check-draft`, `dli.payload-submit`, and the
/// standalone `deloreis check-draft` command.
#[must_use]
pub fn check_payload(
    world: &CompiledWorld,
    payload: &Payload,
    ctx: &BatteryContext,
) -> Vec<PayloadFinding> {
    let mut out = Vec::new();

    // (8) staleness and (7) quote anchoring — the PAY-1 families.
    out.extend(check_staleness(payload, ctx.source));
    out.extend(verify_anchors(payload, ctx.source));

    // Parse the claim atoms once; unparseable atoms are their own defect.
    let (claims, unparseable) = parse_claims(payload);
    out.extend(unparseable);

    // (2) factcheck and (1) continuity over the parsed claims.
    out.extend(factcheck_family(world, &claims));
    out.extend(continuity_family(world, &claims));
    if let Some(continuity) = ctx.runtime_continuity {
        out.extend(runtime_continuity_family(world, continuity));
    }

    // (4) craft season rules over the merged ledger.
    out.extend(craft_family(payload, ctx));

    // Structured rows that used to be accepted without semantics.
    out.extend(payload_structure_family(world, payload, ctx.source));
    out.extend(phase_family(world, payload));
    out.extend(lexicon_family(world, payload, ctx.source));

    // (6) naming and (5) proof-ledger replay.
    out.extend(naming_family(world, payload));
    out.extend(proof_family(world, payload, ctx));

    // (3) two-sided outline-conformance, when an outline is supplied (PAY-3).
    let catalog_outline = ctx.outlines.and_then(|outlines| {
        outlines
            .iter()
            .find(|outline| outline.id == payload.meta.episode)
    });
    if ctx.outlines.is_some() && catalog_outline.is_none() {
        out.push(PayloadFinding::error(
            "payload.outline-episode-unknown",
            format!(
                "payload episode '{}' has no matching reviewed outline",
                payload.meta.episode
            ),
        ));
    } else if let Some(outline) = catalog_outline.or(ctx.outline) {
        out.extend(outline_conformance_with_policy(
            payload,
            outline,
            &ctx.kb,
            &ctx.episode_policy.finding_policy,
        ));
    }

    out.sort_by(|a, b| (&a.code, a.line, &a.message).cmp(&(&b.code, b.line, &b.message)));
    out
}

/// A payload claim whose atom parsed, kept beside its source claim for attribution.
struct ParsedClaim<'a> {
    atom: AtomExpr,
    claim: &'a Claim,
}

/// Parse each claim's surface atom into a typed [`AtomExpr`]; an atom the parser
/// cannot read is a `payload.unparseable-claim` defect (the fact-checker would
/// otherwise silently skip it).
fn parse_claims(payload: &Payload) -> (Vec<ParsedClaim<'_>>, Vec<PayloadFinding>) {
    let mut parsed = Vec::new();
    let mut findings = Vec::new();
    for claim in &payload.claims {
        let clause = Clause {
            text: claim.atom.clone(),
            span: Span::synthetic(),
        };
        if let Some(atom) = parse_clause_atom(&clause) {
            parsed.push(ParsedClaim { atom, claim });
        } else {
            findings.push(
                PayloadFinding::error(
                    "payload.unparseable-claim",
                    format!(
                        "claim atom '{}' is not a well-formed atom; the fact-checker cannot read it",
                        claim.atom
                    ),
                )
                .at_line(claim.line)
                .with_quote(&claim.quote),
            );
        }
    }
    (parsed, findings)
}

/// The factcheck family: each claim resolves to supported / contradicted /
/// unsupported; defects carry the claim's draft line, quote, and D-citations.
fn factcheck_family(world: &CompiledWorld, claims: &[ParsedClaim<'_>]) -> Vec<PayloadFinding> {
    let atoms: Vec<AtomExpr> = claims.iter().map(|c| c.atom.clone()).collect();
    check::factcheck::factcheck(world, &atoms)
        .into_iter()
        .zip(claims)
        .filter_map(|(report, parsed)| {
            let code = match report.verdict {
                Verdict::Contradicted => Some("factcheck.contradicted"),
                Verdict::Unsupported => Some("factcheck.unsupported"),
                Verdict::Supported => None,
            };
            let mut finding = PayloadFinding::error(code?, report.reason)
                .at_line(parsed.claim.line)
                .with_quote(&parsed.claim.quote);
            finding.citations = report.citations;
            Some(finding)
        })
        .collect()
}

/// The continuity family: the structural continuity classes over the claims. A
/// per-claim pass attributes each finding to its claim's line/quote; a whole-set
/// pass then adds the cross-claim findings (e.g. a slot set two ways) unattributed.
fn continuity_family(world: &CompiledWorld, claims: &[ParsedClaim<'_>]) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for parsed in claims {
        let single = [parsed.atom.clone()];
        for wf in check::continuity::check_claims(world, &single) {
            if seen.insert((wf.code.clone(), wf.message.clone())) {
                out.push(
                    world_finding_to_payload(&wf)
                        .at_line(parsed.claim.line)
                        .with_quote(&parsed.claim.quote),
                );
            }
        }
    }
    let all: Vec<AtomExpr> = claims.iter().map(|c| c.atom.clone()).collect();
    for wf in check::continuity::check_claims(world, &all) {
        if seen.insert((wf.code.clone(), wf.message.clone())) {
            out.push(world_finding_to_payload(&wf));
        }
    }
    out
}

/// Runtime continuity over the caller-owned episode-state projection.
///
/// The durable payload remains a statement about one draft. This companion
/// context supplies the folded prior ledger needed for cross-episode checks and
/// the current state rows that may not have quote-anchored prose claims.
fn runtime_continuity_family(
    world: &CompiledWorld,
    continuity: &RuntimeContinuity,
) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    let mut current = Vec::new();

    for fact in &continuity.established {
        match parse_runtime_fact(fact) {
            Some(atom) => current.push(atom),
            None => out.push(invalid_runtime_fact("established", fact)),
        }
    }
    for change in &continuity.changes {
        let target = RuntimeFact {
            predicate: change.predicate.clone(),
            subject: change.subject.clone(),
            value: change.to_value.clone(),
        };
        match parse_runtime_fact(&target) {
            Some(atom) => current.push(atom),
            None => out.push(invalid_runtime_fact("changed target", &target)),
        }
    }

    // The factchecker's contradicted verdict is the world-contradiction class at
    // the episode-state boundary. Unsupported facts are legitimate new state.
    for report in check::factcheck::factcheck(world, &current) {
        if report.verdict == Verdict::Contradicted {
            let mut finding =
                PayloadFinding::error(runtime_contradiction_code(&report.reason), report.reason);
            finding.citations = report.citations;
            out.push(finding);
        }
    }
    out.extend(
        check::continuity::check_claims(world, &current)
            .iter()
            .map(world_finding_to_payload),
    );

    let known: BTreeSet<&str> = world
        .model()
        .entities()
        .iter()
        .map(|entity| entity.name.as_str())
        .collect();
    for reference in &continuity.references {
        if !known.contains(reference.as_str()) {
            out.push(PayloadFinding::error(
                "continuity.unknown-entity",
                format!(
                    "episode state references entity '{reference}', which canon does not define"
                ),
            ));
        }
    }

    let prior: BTreeMap<(&str, &str), &str> = continuity
        .prior_facts
        .iter()
        .map(|fact| {
            (
                (fact.predicate.as_str(), fact.subject.as_str()),
                fact.value.as_str(),
            )
        })
        .collect();
    for fact in &continuity.established {
        if let Some(prior_value) = prior.get(&(fact.predicate.as_str(), fact.subject.as_str()))
            && *prior_value != fact.value
        {
            out.push(PayloadFinding::error(
                "continuity.prior-contradiction",
                format!(
                    "re-establishes {}.{} = {}, reversing the prior value {}",
                    fact.subject, fact.predicate, fact.value, prior_value
                ),
            ));
        }
    }
    for change in &continuity.changes {
        if let Some(prior_value) = prior.get(&(change.predicate.as_str(), change.subject.as_str()))
            && *prior_value != change.from_value
        {
            out.push(PayloadFinding::error(
                "continuity.prior-contradiction",
                format!(
                    "changes {}.{} from {}, but the last established value was {}",
                    change.subject, change.predicate, change.from_value, prior_value
                ),
            ));
        }
    }

    let open_promises: BTreeSet<&str> = continuity
        .prior_open_promises
        .iter()
        .map(String::as_str)
        .collect();
    for promise in &continuity.closed_promises {
        if !open_promises.contains(promise.as_str()) {
            out.push(PayloadFinding::error(
                "continuity.prior-contradiction",
                format!("closes promise or debt '{promise}', which was never opened"),
            ));
        }
    }

    out.sort_by(|a, b| (&a.code, &a.message).cmp(&(&b.code, &b.message)));
    out.dedup_by(|a, b| a.code == b.code && a.message == b.message);
    out
}

fn runtime_contradiction_code(reason: &str) -> &'static str {
    if reason.starts_with("matches contextual rejected-claim")
        || reason.starts_with("realizes killed token")
    {
        "continuity.rejected-lore"
    } else {
        "continuity.world-contradiction"
    }
}

fn parse_runtime_fact(fact: &RuntimeFact) -> Option<AtomExpr> {
    let clause = Clause {
        text: format!("{}({}, {})", fact.predicate, fact.subject, fact.value),
        span: Span::synthetic(),
    };
    parse_clause_atom(&clause)
}

fn invalid_runtime_fact(kind: &str, fact: &RuntimeFact) -> PayloadFinding {
    PayloadFinding::error(
        "payload.runtime-continuity-invalid",
        format!(
            "{kind} runtime fact '{}({}, {})' is not a well-formed atom",
            fact.predicate, fact.subject, fact.value
        ),
    )
}

/// The craft family: merge the payload's beats into the season ledger and run
/// §3's rules; each finding cites its governing principle (via [`Kb::why`]) and,
/// where its message names a payload beat or promise, carries that item's line
/// and quote.
fn craft_family(payload: &Payload, ctx: &BatteryContext) -> Vec<PayloadFinding> {
    let ledger = build_ledger(payload, ctx);
    let anchors = beat_promise_anchors(payload);
    check_season_rules_with_policy(&ledger, &ctx.thresholds, &ctx.episode_policy.finding_policy)
        .into_iter()
        .filter(|finding| ctx.kb.governs(&finding.code))
        .map(|wf| {
            let mut finding = world_finding_to_payload(&wf);
            if let Some(principle) = ctx.kb.why(&wf.code).first() {
                finding.principle = Some(principle.id.to_string());
            }
            if let Some((line, quote)) = anchors
                .iter()
                .find(|(id, _)| wf.message.contains(id.as_str()))
                .map(|(_, lq)| lq.clone())
            {
                finding.line = Some(line);
                finding.quote = Some(quote);
            }
            finding
        })
        .collect()
}

/// Build the season ledger the craft rules run over: the committed season (or an
/// empty ledger) extended with this payload's episode, beats, scenes, and
/// promises, plus the naming-oracle confusable pairs its casts induce.
fn build_ledger(payload: &Payload, ctx: &BatteryContext) -> CraftLedger {
    let mut ledger = ctx.season.cloned().unwrap_or_default();
    let episode = payload.meta.episode.clone();

    if !ledger.episodes.iter().any(|e| e.id == episode) {
        ledger.episodes.push(EpisodeMeta {
            id: episode.clone(),
            slot: next_slot(&ledger),
            arc_shape: None,
            ten_position: None,
            audio_load: None,
            introduced_names: payload.names.introduced.clone(),
        });
    }
    for beat in &payload.beats {
        ledger.beats.push(LedgerBeat {
            id: beat.id.clone(),
            episode: episode.clone(),
            scene: beat.scene.clone(),
            kind: beat.kind,
            thread: beat.thread.clone(),
            magnitude: beat.magnitude,
            outcome: beat.outcome,
            by: beat.by.clone(),
            assisted: beat.assisted,
        });
    }
    for scene in &payload.scenes {
        ledger.scenes.push(LedgerScene {
            id: scene.id.clone(),
            episode: episode.clone(),
            cast: scene.cast.clone(),
            cold_open: false,
            opens_loop: false,
            act: None,
            deep_time: false,
        });
    }
    for promise in &payload.promises.opened {
        ledger.promises.push(LedgerPromise {
            id: promise.id.clone(),
            kind: promise.kind,
            opened_in: episode.clone(),
            touched_in: Vec::new(),
            closed_in: None,
            declared_carry: false,
        });
    }
    for promise in &payload.promises.closed {
        if let Some(existing) = ledger.promises.iter_mut().find(|lp| lp.id == promise.id) {
            existing.closed_in = Some(episode.clone());
        } else {
            ledger.promises.push(LedgerPromise {
                id: promise.id.clone(),
                kind: promise.kind,
                opened_in: episode.clone(),
                touched_in: Vec::new(),
                closed_in: Some(episode.clone()),
                declared_carry: false,
            });
        }
    }
    ledger.confusable_pairs.extend(confusable_pairs(payload));
    ledger
}

/// The next free production slot: one past the highest already in the ledger.
fn next_slot(ledger: &CraftLedger) -> u32 {
    ledger
        .episodes
        .iter()
        .map(|e| e.slot)
        .max()
        .map_or(1, |m| m.saturating_add(1))
}

/// The name pairs the payload's casts and introduced names put below the aural
/// distinctness floor — the scene-name-clash rule's join input.
fn confusable_pairs(payload: &Payload) -> BTreeSet<(String, String)> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for scene in &payload.scenes {
        names.extend(scene.cast.iter().cloned());
        names.extend(scene.off_mic.iter().cloned());
    }
    names.extend(payload.names.introduced.iter().cloned());
    let names: Vec<String> = names.into_iter().collect();
    let mut pairs = BTreeSet::new();
    for (i, a) in names.iter().enumerate() {
        for b in &names[i + 1..] {
            if aural_clash(a, b).is_some() {
                pairs.insert(ordered_pair(a, b));
            }
        }
    }
    pairs
}

/// A normalized `(lo, hi)` name pair — the form the confusability set stores.
fn ordered_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_owned(), b.to_owned())
    } else {
        (b.to_owned(), a.to_owned())
    }
}

/// The naming family: every introduced name is vetted by the oracle (uniqueness,
/// register fit, aural clash, banned-in-name, confusables). The offending name
/// rides as the finding's quote; introduced names carry no draft line.
fn naming_family(world: &CompiledWorld, payload: &Payload) -> Vec<PayloadFinding> {
    let language = &world.model().language;
    let mut out = Vec::new();
    for name in &payload.names.introduced {
        for lang_finding in vet_candidate(language, name) {
            let mut finding = world_finding_to_payload(&lang_finding.to_world_finding());
            finding.quote = Some(name.clone());
            out.push(finding);
        }
    }
    out
}

/// The proof-ledger family: basic shape checks run first, then the shadow must
/// match the canonical body of a declared `proof <episode>-<scene>` (or
/// `proof <scene>`) block. The declaration supplies the practice and speaker;
/// the existing practice checker replays it against the world projection with
/// the payload scene's observed phases. An invented or stale shadow therefore
/// fails closed rather than becoming an unearned verdict.
fn proof_family(
    world: &CompiledWorld,
    payload: &Payload,
    ctx: &BatteryContext<'_>,
) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    for row in &payload.proof_ledgers {
        let scene = &row.scene;
        if row.ledger.close.atom.trim().is_empty() {
            out.push(PayloadFinding::error(
                "proof.no-close",
                format!("proof ledger for scene '{scene}' has no close atom"),
            ));
        }
        if !is_known_verdict(&row.ledger.close.expect) {
            out.push(PayloadFinding::error(
                "proof.unknown-verdict",
                format!(
                    "proof ledger for scene '{scene}': expect '{}' is not one of \
                     holds / blocked / fails-at <error>",
                    row.ledger.close.expect
                ),
            ));
        }
        if row.ledger.premises.is_empty() && row.ledger.steps.is_empty() {
            out.push(PayloadFinding::error(
                "proof.empty-ledger",
                format!("proof ledger for scene '{scene}' carries neither premises nor steps"),
            ));
        }
        out.extend(replay_proof_shadow(world, payload, ctx, row));
    }
    out
}

fn replay_proof_shadow(
    world: &CompiledWorld,
    payload: &Payload,
    ctx: &BatteryContext<'_>,
    row: &ProofLedgerRow,
) -> Vec<PayloadFinding> {
    let scene = &row.scene;
    let declared = match select_declared_proof(
        ctx.documents,
        payload.meta.episode.as_str(),
        scene.as_str(),
    ) {
        ProofSelection::Selected(declared) => declared,
        ProofSelection::Missing => {
            return vec![PayloadFinding::error(
                "proof.undeclared-shadow",
                format!(
                    "proof ledger for scene '{scene}' has no matching declared `proof {scene}` block"
                ),
            )];
        }
        ProofSelection::Ambiguous { name, count } => {
            return vec![PayloadFinding::error(
                "proof.ambiguous-declaration",
                format!(
                    "proof ledger for scene '{scene}' matches {count} declarations named \
                     '{name}' at the selected precedence"
                ),
            )];
        }
    };

    if !proof_shadow_matches(&row.ledger, declared) {
        return vec![PayloadFinding::error(
            "proof.shadow-mismatch",
            format!(
                "proof ledger for scene '{scene}' does not match the declared proof's canonical premises, steps, and close"
            ),
        )];
    }

    let Some(practice_name) = declared.practice() else {
        return vec![PayloadFinding::error(
            "proof.practice-missing",
            format!("declared proof '{scene}' names no practice"),
        )];
    };
    let practice = ctx
        .documents
        .iter()
        .flat_map(compile_practices)
        .find(|practice| practice.name.text == practice_name);
    let Some(practice) = practice else {
        return vec![PayloadFinding::error(
            "proof.practice-unknown",
            format!("declared proof '{scene}' names unknown practice '{practice_name}'"),
        )];
    };

    let mut out = Vec::new();
    let speaker = declared.actor().map(ToOwned::to_owned);
    let mut projection = project_scene(world, speaker.as_deref());
    if let Some(payload_scene) = payload
        .scenes
        .iter()
        .find(|candidate| candidate.id == *scene)
    {
        projection.phases.clone_from(&payload_scene.phases);
        if let Some(speaker) = &speaker
            && !payload_scene.cast.contains(speaker)
            && !payload_scene.off_mic.contains(speaker)
        {
            out.push(PayloadFinding::error(
                    "proof.speaker-absent",
                    format!(
                        "proof '{scene}' is spoken by '{speaker}', who is not present in that payload scene"
                    ),
                ));
        }
    }
    let report = check_proof(&practice, declared, &projection);
    out.extend(report.findings.into_iter().map(|proof_finding| {
        let mut finding = PayloadFinding::error(proof_finding.code(), proof_finding.message());
        finding.grade = report.grade;
        finding
    }));
    out
}

enum ProofSelection<'a> {
    Missing,
    Selected(&'a ProofDecl),
    Ambiguous { name: String, count: usize },
}

fn select_declared_proof<'a>(
    documents: &'a [Document],
    episode: &str,
    scene: &str,
) -> ProofSelection<'a> {
    let matches = |name: &str| {
        documents
            .iter()
            .flat_map(|document| &document.decls)
            .filter_map(|decl| match decl {
                Decl::Proof(proof) if proof.name.text == name => Some(proof),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    let qualified_name = (!episode.is_empty()).then(|| format!("{episode}-{scene}"));
    let (selected_name, selected) = if let Some(name) = qualified_name {
        let qualified = matches(&name);
        if qualified.is_empty() {
            (scene.to_owned(), matches(scene))
        } else {
            (name, qualified)
        }
    } else {
        (scene.to_owned(), matches(scene))
    };

    match selected.as_slice() {
        [] => ProofSelection::Missing,
        [declared] => ProofSelection::Selected(declared),
        duplicates => ProofSelection::Ambiguous {
            name: selected_name,
            count: duplicates.len(),
        },
    }
}

fn proof_shadow_matches(shadow: &ProofLedger, declared: &ProofDecl) -> bool {
    let premises = declared
        .body
        .iter()
        .filter(|clause| clause.text.starts_with("premise "))
        .map(|clause| clause.text.clone())
        .collect::<Vec<_>>();
    let steps = declared
        .body
        .iter()
        .filter(|clause| {
            [
                "step ",
                "spend ",
                "release ",
                "reveal ",
                "redistribute ",
                "assume ",
            ]
            .iter()
            .any(|prefix| clause.text.starts_with(prefix))
        })
        .map(|clause| clause.text.clone())
        .collect::<Vec<_>>();
    let closes = declared
        .body
        .iter()
        .filter_map(|clause| parse_declared_close(&clause.text))
        .collect::<Vec<_>>();
    let close_is_final = declared
        .body
        .last()
        .and_then(|clause| parse_declared_close(&clause.text))
        .is_some();
    premises == shadow.premises
        && steps == shadow.steps
        && close_is_final
        && closes.first().is_some_and(|(atom, expect)| {
            closes.len() == 1 && atom == &shadow.close.atom && expect == &shadow.close.expect
        })
}

fn parse_declared_close(text: &str) -> Option<(String, String)> {
    let body = text.strip_prefix("close ")?;
    let (atom, expect) = parse_proof_close_parts(body)?;
    Some((atom, expect?))
}

fn payload_structure_family(
    world: &CompiledWorld,
    payload: &Payload,
    source: &str,
) -> Vec<PayloadFinding> {
    let mut out = payload_metadata_findings(payload, source);
    let (scene_findings, scene_ids) = scene_structure_findings(world, payload);
    out.extend(scene_findings);
    out.extend(delta_structure_findings(world, payload, source, &scene_ids));
    out.extend(beat_structure_findings(world, payload, &scene_ids));
    out.extend(promise_structure_findings(payload));
    out.extend(name_structure_findings(payload));
    for proof in &payload.proof_ledgers {
        if !scene_ids.contains(proof.scene.as_str()) {
            out.push(PayloadFinding::error(
                "payload.proof-unknown-scene",
                format!("proof shadow references undeclared scene '{}'", proof.scene),
            ));
        }
    }
    out.extend(cast_findings(world, payload));
    out
}

fn scene_structure_findings<'a>(
    world: &CompiledWorld,
    payload: &'a Payload,
) -> (Vec<PayloadFinding>, BTreeSet<&'a str>) {
    let mut out = Vec::new();
    let mut scene_ids = BTreeSet::new();
    for scene in &payload.scenes {
        if scene.id.trim().is_empty() {
            out.push(PayloadFinding::error(
                "payload.scene-id-empty",
                "a payload scene must carry a non-empty id",
            ));
        } else if !scene_ids.insert(scene.id.as_str()) {
            out.push(PayloadFinding::error(
                "payload.scene-duplicate",
                format!("payload scene '{}' is declared more than once", scene.id),
            ));
        }
        let mut present = BTreeSet::new();
        for cast in &scene.cast {
            if !present.insert(cast.as_str()) {
                out.push(PayloadFinding::error(
                    "payload.cast-duplicate",
                    format!("scene '{}' repeats an on-mic cast member", scene.id),
                ));
            }
        }
        for cast in &scene.off_mic {
            if !present.insert(cast.as_str()) {
                out.push(PayloadFinding::error(
                    "payload.presence-conflict",
                    format!(
                        "scene '{}' repeats a cast member or marks them both on-mic and off-mic",
                        scene.id
                    ),
                ));
            }
        }
        if let Some(location) = &scene.location
            && (location.trim().is_empty()
                || (!world
                    .model()
                    .entities()
                    .iter()
                    .any(|entity| entity.name == *location)
                    && !payload.names.introduced.contains(location)))
        {
            out.push(PayloadFinding::error(
                "payload.location-unknown",
                format!(
                    "scene '{}' location is empty or not a declared or introduced entity",
                    scene.id
                ),
            ));
        }
    }
    (out, scene_ids)
}

fn delta_structure_findings(
    world: &CompiledWorld,
    payload: &Payload,
    source: &str,
    scene_ids: &BTreeSet<&str>,
) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    let source_lines = u32::try_from(source.lines().count()).unwrap_or(u32::MAX);
    for delta in &payload.deltas {
        if delta.who.trim().is_empty()
            || delta.fact.trim().is_empty()
            || delta.scene.trim().is_empty()
            || delta.line == 0
        {
            out.push(
                PayloadFinding::error(
                    "payload.delta-incomplete",
                    "every delta requires non-empty `who`, `fact`, and `scene` plus a 1-based line",
                )
                .at_line(delta.line),
            );
            continue;
        }
        if delta.who != "listener"
            && !world
                .model()
                .entities()
                .iter()
                .any(|entity| entity.name == delta.who)
            && !payload.names.introduced.contains(&delta.who)
        {
            out.push(
                PayloadFinding::error(
                    "payload.delta-unknown-subject",
                    format!(
                        "delta subject '{}' is neither a declared entity, the listener, nor a name introduced by this payload",
                        delta.who
                    ),
                )
                .at_line(delta.line),
            );
        }
        if !scene_ids.contains(delta.scene.as_str()) {
            out.push(
                PayloadFinding::error(
                    "payload.delta-unknown-scene",
                    format!("delta references undeclared scene '{}'", delta.scene),
                )
                .at_line(delta.line),
            );
        }
        if delta.line > source_lines {
            out.push(
                PayloadFinding::error(
                    "payload.delta-line-out-of-range",
                    format!(
                        "delta line {} lies beyond the draft's {source_lines} line(s)",
                        delta.line
                    ),
                )
                .at_line(delta.line),
            );
        }
    }
    out
}

fn beat_structure_findings(
    world: &CompiledWorld,
    payload: &Payload,
    scene_ids: &BTreeSet<&str>,
) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    let mut beat_ids = BTreeSet::new();
    for beat in &payload.beats {
        if beat.id.trim().is_empty()
            || beat.scene.trim().is_empty()
            || beat.thread.trim().is_empty()
            || beat.line == 0
        {
            out.push(
                PayloadFinding::error(
                    "payload.beat-incomplete",
                    "every beat requires non-empty `id`, `scene`, and `thread` plus a 1-based line",
                )
                .at_line(beat.line)
                .with_quote(&beat.quote),
            );
        } else if !beat_ids.insert(beat.id.as_str()) {
            out.push(
                PayloadFinding::error(
                    "payload.beat-duplicate",
                    format!("payload beat '{}' is declared more than once", beat.id),
                )
                .at_line(beat.line)
                .with_quote(&beat.quote),
            );
        }
        if !scene_ids.contains(beat.scene.as_str()) {
            out.push(
                PayloadFinding::error(
                    "payload.beat-unknown-scene",
                    format!(
                        "beat '{}' references undeclared scene '{}'",
                        beat.id, beat.scene
                    ),
                )
                .at_line(beat.line)
                .with_quote(&beat.quote),
            );
        }
        if !beat.by.is_empty()
            && !world
                .model()
                .entities()
                .iter()
                .any(|entity| entity.name == beat.by)
            && !payload.names.introduced.contains(&beat.by)
        {
            out.push(
                PayloadFinding::error(
                    "payload.beat-unknown-performer",
                    format!("beat '{}' names an unknown performer", beat.id),
                )
                .at_line(beat.line)
                .with_quote(&beat.quote),
            );
        }
        if beat.assisted && beat.by.is_empty() {
            out.push(
                PayloadFinding::error(
                    "payload.beat-assistance-without-performer",
                    format!("assisted beat '{}' names no performer", beat.id),
                )
                .at_line(beat.line)
                .with_quote(&beat.quote),
            );
        }
    }
    out
}

fn promise_structure_findings(payload: &Payload) -> Vec<PayloadFinding> {
    let mut out = promise_list_findings(&payload.promises.opened, "opened");
    out.extend(promise_list_findings(&payload.promises.closed, "closed"));
    out
}

fn promise_list_findings(promises: &[PromiseRow], action: &str) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for promise in promises {
        if promise.id.trim().is_empty() {
            out.push(
                PayloadFinding::error(
                    "payload.promise-id-empty",
                    format!("a {action} promise must carry a non-empty id"),
                )
                .at_line(promise.line)
                .with_quote(&promise.quote),
            );
        } else if !seen.insert(promise.id.as_str()) {
            out.push(
                PayloadFinding::error(
                    "payload.promise-duplicate",
                    format!("promise '{}' is {action} more than once", promise.id),
                )
                .at_line(promise.line)
                .with_quote(&promise.quote),
            );
        }
    }
    out
}

fn name_structure_findings(payload: &Payload) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    let mut introduced_names = BTreeSet::new();
    for name in &payload.names.introduced {
        if name.trim().is_empty() {
            out.push(PayloadFinding::error(
                "payload.name-empty",
                "an introduced name must not be empty",
            ));
        } else if !introduced_names.insert(name.as_str()) {
            out.push(PayloadFinding::error(
                "payload.name-duplicate",
                format!("introduced name '{name}' is repeated"),
            ));
        }
    }
    out
}

fn payload_metadata_findings(payload: &Payload, source: &str) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    if payload.meta.episode.trim().is_empty() {
        out.push(PayloadFinding::error(
            "payload.episode-empty",
            "payload metadata requires a non-empty episode id",
        ));
    }
    if payload.meta.source.file.trim().is_empty() {
        out.push(PayloadFinding::error(
            "payload.source-file-empty",
            "payload metadata requires a non-empty source file",
        ));
    }
    if payload.meta.extractors.schema != "payload/v1" {
        out.push(PayloadFinding::error(
            "payload.schema-unsupported",
            "payload extractor schema must be exactly 'payload/v1'",
        ));
    }
    if !valid_calendar_date(&payload.meta.extracted) {
        out.push(PayloadFinding::error(
            "payload.extracted-invalid",
            "payload extraction date must be a valid YYYY-MM-DD calendar date",
        ));
    }

    let source_words = lexical_tokens(source).count();
    if u32::try_from(source_words).unwrap_or(u32::MAX) != payload.meta.source.words {
        out.push(PayloadFinding::error(
            "payload.source-word-count",
            format!(
                "payload source word count {} does not match the draft's {source_words} lexical token(s)",
                payload.meta.source.words
            ),
        ));
    }
    if u64::from(payload.lexicon.register_tokens) > source_words as u64 {
        out.push(PayloadFinding::error(
            "payload.register-token-count",
            format!(
                "payload register token count {} exceeds the draft's {source_words} lexical token(s)",
                payload.lexicon.register_tokens
            ),
        ));
    }

    let extracted_fields = [
        ("claims", !payload.claims.is_empty()),
        ("deltas", !payload.deltas.is_empty()),
        ("scenes", !payload.scenes.is_empty()),
        ("beats", !payload.beats.is_empty()),
        (
            "promises",
            !payload.promises.opened.is_empty() || !payload.promises.closed.is_empty(),
        ),
        ("proof_ledgers", !payload.proof_ledgers.is_empty()),
        ("names", !payload.names.introduced.is_empty()),
    ];
    for &(field, non_empty) in &extracted_fields {
        if non_empty && !payload.meta.extractors.fields.contains_key(field) {
            out.push(PayloadFinding::error(
                "payload.extractor-missing",
                format!("non-empty extracted field '{field}' has no extractor provenance"),
            ));
        }
    }
    let supported_fields = extracted_fields
        .iter()
        .map(|(field, _)| *field)
        .collect::<BTreeSet<_>>();
    for (field, extractor) in &payload.meta.extractors.fields {
        if !supported_fields.contains(field.as_str()) {
            out.push(PayloadFinding::error(
                "payload.extractor-field-unknown",
                "extractor manifest contains a field outside payload/v1",
            ));
        }
        if extractor.model.trim().is_empty() {
            out.push(PayloadFinding::error(
                "payload.extractor-model-invalid",
                "extractor provenance requires a non-empty model",
            ));
        }
        if !valid_sha256(&extractor.prompt) {
            out.push(PayloadFinding::error(
                "payload.extractor-prompt-invalid",
                "extractor provenance requires a lowercase sha256: prompt digest",
            ));
        }
    }
    out
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn valid_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return false;
    }
    let year = value[0..4].parse::<u32>().unwrap_or(0);
    let month = value[5..7].parse::<u32>().unwrap_or(0);
    let day = value[8..10].parse::<u32>().unwrap_or(0);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    year != 0 && day != 0 && day <= max_day
}

fn cast_findings(world: &CompiledWorld, payload: &Payload) -> Vec<PayloadFinding> {
    let known_entities = world
        .model()
        .entities()
        .iter()
        .map(|entity| entity.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    for scene in &payload.scenes {
        for cast in scene.cast.iter().chain(&scene.off_mic) {
            if cast.trim().is_empty() {
                out.push(PayloadFinding::error(
                    "payload.cast-empty",
                    format!("scene '{}' carries an empty cast identifier", scene.id),
                ));
            } else if !known_entities.contains(cast.as_str())
                && !payload.names.introduced.contains(cast)
            {
                out.push(PayloadFinding::error(
                    "payload.cast-unknown",
                    format!(
                        "scene '{}' names unknown cast member '{cast}'; declare or introduce the name",
                        scene.id
                    ),
                ));
            }
        }
    }
    out
}

fn phase_family(world: &CompiledWorld, payload: &Payload) -> Vec<PayloadFinding> {
    let cycles = world
        .model()
        .world
        .cycles
        .iter()
        .map(|cycle| (cycle.name.as_str(), cycle))
        .collect::<BTreeMap<_, _>>();
    let mut previous = BTreeMap::<&str, (&str, &str)>::new();
    let mut out = Vec::new();
    for scene in &payload.scenes {
        for (cycle_name, spoke) in &scene.phases {
            let Some(cycle) = cycles.get(cycle_name.as_str()) else {
                out.push(PayloadFinding::error(
                    "phase.unknown-cycle",
                    format!(
                        "scene '{}' observes undeclared cycle '{cycle_name}'",
                        scene.id
                    ),
                ));
                continue;
            };
            if cycle.spokes.is_empty() {
                out.push(PayloadFinding::error(
                    "phase.empty-cycle",
                    format!("declared cycle '{cycle_name}' carries no spokes"),
                ));
                continue;
            }
            let Some(index) = cycle.spokes.iter().position(|candidate| candidate == spoke) else {
                out.push(PayloadFinding::error(
                    "phase.unknown-spoke",
                    format!(
                        "scene '{}' observes '{spoke}', which cycle '{cycle_name}' does not carry",
                        scene.id
                    ),
                ));
                continue;
            };
            if let Some((previous_scene, previous_spoke)) = previous.get(cycle_name.as_str()) {
                let previous_index = cycle
                    .spokes
                    .iter()
                    .position(|candidate| candidate == previous_spoke)
                    .expect("a recorded previous spoke was validated");
                let legal_next = (previous_index + 1) % cycle.spokes.len();
                if index != previous_index && index != legal_next {
                    out.push(PayloadFinding::error(
                        "phase.illegal-walk",
                        format!(
                            "cycle '{cycle_name}' jumps from '{previous_spoke}' in scene '{previous_scene}' to '{spoke}' in scene '{}'; only the same or successor spoke is contiguous",
                            scene.id
                        ),
                    ));
                }
            }
            previous.insert(cycle_name, (&scene.id, spoke));
        }
    }
    out
}

fn lexicon_family(world: &CompiledWorld, payload: &Payload, source: &str) -> Vec<PayloadFinding> {
    let normalized = lexical_tokens(source)
        .map(str::to_lowercase)
        .collect::<BTreeSet<_>>();
    let mut expected = world
        .model()
        .language
        .bans
        .iter()
        .filter(|ban| normalized.contains(&ban.word.to_lowercase()))
        .map(|ban| ban.word.to_lowercase())
        .collect::<Vec<_>>();
    expected.sort();
    expected.dedup();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut supplied = Vec::new();
    for hit in &payload.lexicon.banned_hits {
        let normalized = hit.to_lowercase();
        if normalized.trim().is_empty() {
            out.push(PayloadFinding::error(
                "lexicon.empty-hit",
                "payload banned_hits contains an empty entry",
            ));
        } else if !seen.insert(normalized.clone()) {
            out.push(PayloadFinding::error(
                "lexicon.duplicate-hit",
                "payload banned_hits repeats an entry",
            ));
        }
        supplied.push(normalized);
    }
    supplied.sort();
    supplied.dedup();

    out.extend(expected.iter().map(|word| {
        PayloadFinding::error(
            "lexicon.banned-hit",
            format!("the draft contains lexically banned word '{word}'"),
        )
        .with_quote(word)
    }));
    if expected != supplied {
        out.push(PayloadFinding::error(
            "lexicon.scan-mismatch",
            format!(
                "payload banned_hits {supplied:?} do not match the kernel's deterministic scan {expected:?}"
            ),
        ));
    }
    out
}

fn lexical_tokens(source: &str) -> impl Iterator<Item = &str> {
    source
        .split(|character: char| !character.is_alphanumeric() && character != '-')
        .filter(|word| !word.is_empty())
}

/// Whether a proof close verdict token is one the ledger checker recognizes.
fn is_known_verdict(expect: &str) -> bool {
    let token = expect.trim();
    token == "holds" || token == "blocked" || token.starts_with("fails-at")
}

/// The two-sided outline-conformance diff (`PAY-3`, §7.3).
///
/// Extractors are quote-anchored and cannot invent, but they can *miss*, so
/// conformance is a two-sided diff whose findings state **both** readings.
/// Over beats, promises, and scene casts:
///
/// - an item the outline declares but the payload lacks →
///   `outline.<kind>-unaccounted` (*the draft dropped it, or extraction missed it*);
/// - an item the payload carries that the outline never declared →
///   `outline.<kind>-undeclared` (*draft drift, or a stale outline*), carrying the
///   payload item's line and quote.
///
/// Proof-cue conformance is not diffed here: `CRAFT-1`'s [`EpisodeOutline`]
/// declares no proof cues to diff the payload's `proof_ledgers` against.
#[must_use]
pub fn outline_conformance(
    payload: &Payload,
    outline: &EpisodeOutline,
    kb: &Kb<'_>,
) -> Vec<PayloadFinding> {
    outline_conformance_with_policy(payload, outline, kb, &DEFAULT_FINDING_POLICY)
}

/// Run the two-sided outline diff with caller-owned citation and severity policy.
#[must_use]
pub fn outline_conformance_with_policy(
    payload: &Payload,
    outline: &EpisodeOutline,
    kb: &Kb<'_>,
    finding_policy: &FindingPolicy<'_>,
) -> Vec<PayloadFinding> {
    let mut out = Vec::new();
    let beat_anchors = beat_anchors(payload);
    let promise_anchors = promise_anchors(payload);

    two_sided(
        &mut out,
        "beat",
        outline.tryfail_beats.iter().map(|b| b.id.as_str()),
        payload.beats.iter().map(|b| b.id.as_str()),
        &beat_anchors,
    );

    let payload_promise_ids: Vec<&str> = payload
        .promises
        .opened
        .iter()
        .chain(&payload.promises.closed)
        .map(|p| p.id.as_str())
        .collect();
    two_sided(
        &mut out,
        "promise",
        outline.promises.iter().map(|p| p.id.as_str()),
        payload_promise_ids.iter().copied(),
        &promise_anchors,
    );

    for outline_scene in &outline.scenes {
        let payload_scene = payload.scenes.iter().find(|s| s.id == outline_scene.id);
        for member in &outline_scene.speaking_cast {
            if payload_scene.is_none_or(|scene| !scene.cast.contains(member)) {
                out.push(PayloadFinding::error(
                    "outline.cast-unaccounted",
                    format!(
                        "the outline casts '{member}' in scene '{}' but the payload's scene lacks \
                         them — either the draft dropped the line or extraction missed it",
                        outline_scene.id
                    ),
                ));
            }
        }
        if let Some(payload_scene) = payload_scene {
            for member in &payload_scene.cast {
                if !outline_scene.speaking_cast.contains(member) {
                    out.push(PayloadFinding::error(
                        "outline.cast-undeclared",
                        format!(
                            "the payload casts '{member}' in scene '{}' the outline never declared — \
                             draft drift, or a stale outline",
                            outline_scene.id
                        ),
                    ));
                }
            }
        }
    }
    for payload_scene in &payload.scenes {
        if !outline
            .scenes
            .iter()
            .any(|scene| scene.id == payload_scene.id)
        {
            for member in &payload_scene.cast {
                out.push(PayloadFinding::error(
                    "outline.cast-undeclared",
                    format!(
                        "the payload casts '{member}' in scene '{}' the outline never declared — \
                         draft drift, or a stale outline",
                        payload_scene.id
                    ),
                ));
            }
        }
    }
    // Attribute each conformance finding to its governing principle. An
    // `outline.*` code resolves to the CRAFT-1 battery principles via the KB.
    out.retain(|finding| kb.governs(&finding.code));
    for finding in &mut out {
        if let Some(principle) = kb.why(&finding.code).first() {
            finding.principle = Some(principle.id.to_string());
        }
        apply_finding_policy(finding_policy, finding);
    }
    out
}

fn apply_finding_policy(policy: &FindingPolicy<'_>, finding: &mut PayloadFinding) {
    let citations = policy.citations(&finding.code);
    if !citations.is_empty() {
        finding.citations = citations;
    }
    if let Some(severity) = policy.severity_override(&finding.code) {
        finding.severity = severity;
    }
}

/// The two-sided id diff shared by beats and promises: outline-only ids are
/// `unaccounted`, payload-only ids are `undeclared` (carrying the payload item's
/// anchor). Both messages state both readings.
fn two_sided<'a>(
    out: &mut Vec<PayloadFinding>,
    kind: &str,
    outline_ids: impl Iterator<Item = &'a str>,
    payload_ids: impl Iterator<Item = &'a str>,
    payload_anchor: &BTreeMap<String, (u32, String)>,
) {
    let outline: BTreeSet<&str> = outline_ids.collect();
    let payload: BTreeSet<&str> = payload_ids.collect();
    for id in outline.difference(&payload) {
        out.push(PayloadFinding::error(
            format!("outline.{kind}-unaccounted"),
            format!(
                "the outline declares {kind} '{id}' but the payload lacks it — either the draft \
                 dropped the {kind} or extraction missed it"
            ),
        ));
    }
    for id in payload.difference(&outline) {
        let mut finding = PayloadFinding::error(
            format!("outline.{kind}-undeclared"),
            format!(
                "the payload carries {kind} '{id}' the outline never declared — draft drift, or a \
                 stale outline"
            ),
        );
        if let Some((line, quote)) = payload_anchor.get(*id) {
            finding = finding.at_line(*line).with_quote(quote);
        }
        out.push(finding);
    }
}

/// A `WorldFinding` re-homed as a [`PayloadFinding`] (no draft anchor yet).
fn world_finding_to_payload(finding: &WorldFinding) -> PayloadFinding {
    PayloadFinding {
        code: finding.code.clone(),
        severity: finding.severity,
        grade: finding.grade,
        message: finding.message.clone(),
        line: None,
        quote: None,
        principle: None,
        citations: finding.citations.clone(),
    }
}

/// The `id -> (line, quote)` anchor map for the payload's beats.
fn beat_anchors(payload: &Payload) -> BTreeMap<String, (u32, String)> {
    payload
        .beats
        .iter()
        .map(|b| (b.id.clone(), (b.line, b.quote.clone())))
        .collect()
}

/// The `id -> (line, quote)` anchor map for the payload's opened and closed
/// promises.
fn promise_anchors(payload: &Payload) -> BTreeMap<String, (u32, String)> {
    payload
        .promises
        .opened
        .iter()
        .chain(&payload.promises.closed)
        .map(|p| (p.id.clone(), (p.line, p.quote.clone())))
        .collect()
}

/// The combined beat + promise anchor map, for enriching craft findings.
fn beat_promise_anchors(payload: &Payload) -> Vec<(String, (u32, String))> {
    let mut anchors: Vec<(String, (u32, String))> = Vec::new();
    anchors.extend(beat_anchors(payload));
    anchors.extend(promise_anchors(payload));
    anchors
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::borrow::Cow;

    use super::*;

    fn sample_source() -> String {
        [
            "the house had said yes to her, in its way", // line 1
            "she set her hand to the frame",             // line 2
            "the withy held",                            // line 3
            "something four-legged kept to the dark",    // line 4
        ]
        .join("\n")
    }

    fn sample_payload(source: &str) -> Payload {
        Payload {
            meta: PayloadMeta {
                episode: "e01".to_owned(),
                source: SourceRef {
                    file: "episode-01.md".to_owned(),
                    sha256: source_hash(source),
                    words: 8,
                },
                extracted: "2026-07-19".to_owned(),
                extractors: ExtractorManifest {
                    schema: "payload/v1".to_owned(),
                    fields: BTreeMap::from([(
                        "beats".to_owned(),
                        ExtractorRef {
                            model: "claude-fable-5".to_owned(),
                            prompt: "sha256:b41c".to_owned(),
                        },
                    )]),
                },
            },
            claims: vec![Claim {
                atom: "holds(enfys, consent(household, door))".to_owned(),
                quote: "the house had said yes to her, in its way".to_owned(),
                line: 1,
            }],
            deltas: vec![DeltaRow {
                kind: DeltaKind::Knowledge,
                who: "apprentice".to_owned(),
                fact: "ward-needs-consent".to_owned(),
                scene: "s04".to_owned(),
                line: 2,
            }],
            scenes: vec![SceneRow {
                id: "s04".to_owned(),
                cast: vec!["enfys".to_owned(), "apprentice".to_owned()],
                location: Some("gwern-threshold".to_owned()),
                phases: BTreeMap::from([("tide".to_owned(), "slack".to_owned())]),
                off_mic: vec!["anchor".to_owned()],
            }],
            beats: vec![BeatRow {
                id: "b7".to_owned(),
                scene: "s04".to_owned(),
                kind: BeatKind::Accomplishment,
                thread: "ward-the-door".to_owned(),
                magnitude: Magnitude::Minor,
                outcome: None,
                by: "enfys".to_owned(),
                assisted: false,
                quote: "the withy held".to_owned(),
                line: 3,
            }],
            promises: PromiseLedger {
                opened: vec![PromiseRow {
                    id: "p-hound".to_owned(),
                    kind: PromiseKind::Mystery,
                    quote: "something four-legged kept to the dark".to_owned(),
                    line: 4,
                }],
                closed: Vec::new(),
            },
            proof_ledgers: vec![ProofLedgerRow {
                scene: "s04".to_owned(),
                ledger: ProofLedger {
                    premises: vec!["seated(withy)".to_owned()],
                    steps: vec!["chain from seated(withy) by ward-law".to_owned()],
                    close: ProofClose {
                        atom: "holds(ward(door))".to_owned(),
                        expect: "holds".to_owned(),
                    },
                },
            }],
            names: NameIntro {
                introduced: Vec::new(),
            },
            lexicon: Lexicon {
                banned_hits: Vec::new(),
                register_tokens: 61,
            },
        }
    }

    #[test]
    fn valid_payload_round_trips_and_verifies_clean() {
        let source = sample_source();
        let payload = sample_payload(&source);

        // serde round-trips through both JSON and the on-disk YAML form.
        let json = serde_json::to_string(&payload).unwrap();
        assert_eq!(payload, serde_json::from_str::<Payload>(&json).unwrap());
        let yaml = serde_yaml_ng::to_string(&payload).unwrap();
        assert_eq!(payload, serde_yaml_ng::from_str::<Payload>(&yaml).unwrap());

        // The extractor manifest keeps its schema and declared dynamic field map.
        assert!(yaml.contains("schema: payload/v1"), "{yaml}");
        assert!(yaml.contains("fields:"), "{yaml}");

        // Every anchor lands and the hash matches: a clean shadow.
        let findings = verify(&payload, &source);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn a_quote_not_verbatim_at_its_line_is_a_hard_error() {
        let source = sample_source();
        let mut payload = sample_payload(&source);

        // The beat's quote really lives on line 3; claim it on line 2.
        payload.beats[0].line = 2;
        let findings = verify_anchors(&payload, &source);
        assert_eq!(findings.len(), 1, "{findings:?}");
        let f = &findings[0];
        assert_eq!(f.code, "anchor.quote-misplaced");
        assert_eq!(f.severity, Severity::Error);
        assert_eq!(f.grade, Grade::Checked);
        assert_eq!(f.line, Some(2));
        assert_eq!(f.quote.as_deref(), Some("the withy held"));

        // A quote the draft does not bear at all is also a hard error.
        payload.beats[0].quote = "the withy snapped".to_owned();
        let findings = verify_anchors(&payload, &source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "anchor.quote-missing");

        // An empty quote cannot anchor anything.
        payload.beats[0].quote = String::new();
        let findings = verify_anchors(&payload, &source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "anchor.empty-quote");
    }

    #[test]
    fn a_stale_source_hash_flags_the_payload() {
        let source = sample_source();
        let payload = sample_payload(&source);
        assert!(check_staleness(&payload, &source).is_none());

        // Edit the draft underneath the payload: same payload, moved source. The
        // edit appends a line, so the existing anchors still land — staleness is
        // the sole finding, proving the hash catches a move the anchors miss.
        let edited = format!("{source}\nand the tide came in");
        let finding = check_staleness(&payload, &edited).expect("stale must flag");
        assert_eq!(finding.code, "payload.stale");
        assert_eq!(finding.severity, Severity::Error);

        let findings = verify(&payload, &edited);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].code, "payload.stale");
    }

    // --- the merged battery (PAY-2) ------------------------------------------

    #[test]
    fn battery_factchecks_claims_against_the_world_and_attributes_them() {
        use crate::Pack;
        use crate::compile;
        use crate::loader::example_docs;

        // The camelot golden world: `adjacent(camelot, the-lake)` is
        // ratified-certain; `kept-at(excalibur, camelot)` is proposed shadow lore.
        let world = compile(&Pack {
            name: "camelot".to_owned(),
            documents: example_docs(),
        })
        .unwrap();

        let source = "the king's hall looks out over the water\nExcalibur is kept at Camelot\n";
        let mut payload = Payload::default();
        payload.meta.source.sha256 = source_hash(source);
        payload.claims = vec![
            Claim {
                atom: "adjacent(camelot, the-lake)".to_owned(),
                quote: "the king's hall looks out over the water".to_owned(),
                line: 1,
            },
            Claim {
                atom: "kept-at(excalibur, camelot)".to_owned(),
                quote: "Excalibur is kept at Camelot".to_owned(),
                line: 2,
            },
        ];

        let findings = check_payload(&world, &payload, &BatteryContext::new(source));

        // Source hash matches and every quote anchors: no PAY-1 findings.
        assert!(
            !findings
                .iter()
                .any(|f| f.family() == "anchor" || f.code == "payload.stale"),
            "{findings:?}"
        );
        // The proposed claim is unsupported, attributed to its draft line + quote.
        let unsupported = findings
            .iter()
            .find(|f| f.code == "factcheck.unsupported")
            .expect("kept-at must be unsupported");
        assert_eq!(unsupported.line, Some(2));
        assert_eq!(
            unsupported.quote.as_deref(),
            Some("Excalibur is kept at Camelot")
        );
        // Continuity flags the same claim as stated-shadow-as-settled.
        assert!(
            findings.iter().any(|f| f.family() == "continuity"),
            "{findings:?}"
        );
        // The ratified claim is supported — no factcheck finding on line 1.
        assert!(
            !findings
                .iter()
                .any(|f| f.family() == "factcheck" && f.line == Some(1)),
            "{findings:?}"
        );
    }

    #[test]
    fn runtime_context_checks_world_prior_reference_and_promise_continuity() {
        use crate::Pack;
        use crate::check::factcheck::{KilledTokens, MatchMode, RejectedClaim};
        use crate::compile;
        use crate::loader::example_docs;

        let world = compile(&Pack {
            name: "camelot".to_owned(),
            documents: example_docs(),
        })
        .unwrap()
        .with_killed_tokens(
            KilledTokens::default().with_rejected_claims(vec![RejectedClaim {
                predicate: "kept-at".to_owned(),
                subject: "excalibur".to_owned(),
                value: "camelot".to_owned(),
                mode: MatchMode::Word,
                decision: "D-0009".to_owned(),
                scope: "the sword is not kept at Camelot".to_owned(),
            }]),
        );
        let source = sample_source();
        let mut payload = Payload::default();
        payload.meta.source.sha256 = source_hash(&source);
        let continuity = RuntimeContinuity {
            established: vec![
                RuntimeFact {
                    predicate: "adjacent".to_owned(),
                    subject: "camelot".to_owned(),
                    value: "excalibur".to_owned(),
                },
                RuntimeFact {
                    predicate: "kept-at".to_owned(),
                    subject: "excalibur".to_owned(),
                    value: "camelot".to_owned(),
                },
                RuntimeFact {
                    predicate: "mood".to_owned(),
                    subject: "camelot".to_owned(),
                    value: "grim".to_owned(),
                },
            ],
            changes: vec![RuntimeChange {
                predicate: "weather".to_owned(),
                subject: "camelot".to_owned(),
                from_value: "sunny".to_owned(),
                to_value: "rain".to_owned(),
            }],
            references: vec!["ghost".to_owned()],
            prior_facts: vec![
                RuntimeFact {
                    predicate: "mood".to_owned(),
                    subject: "camelot".to_owned(),
                    value: "weary".to_owned(),
                },
                RuntimeFact {
                    predicate: "weather".to_owned(),
                    subject: "camelot".to_owned(),
                    value: "cloudy".to_owned(),
                },
            ],
            prior_open_promises: Vec::new(),
            closed_promises: vec!["unowed-debt".to_owned()],
        };

        let findings = check_payload(
            &world,
            &payload,
            &BatteryContext::new(&source).with_runtime_continuity(&continuity),
        );
        let codes: Vec<&str> = findings
            .iter()
            .map(|finding| finding.code.as_str())
            .collect();

        assert!(
            codes.contains(&"continuity.world-contradiction"),
            "{findings:?}"
        );
        assert!(
            codes.contains(&"continuity.proposed-as-fact"),
            "{findings:?}"
        );
        assert!(codes.contains(&"continuity.rejected-lore"), "{findings:?}");
        assert!(codes.contains(&"continuity.unknown-entity"), "{findings:?}");
        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == "continuity.prior-contradiction")
                .count(),
            3,
            "{findings:?}"
        );
    }

    #[test]
    fn craft_family_flags_an_unearned_major_accomplishment() {
        const RULES: &[crate::craft::config::FindingRule<'static>] =
            &[crate::craft::config::FindingRule {
                code: Cow::Borrowed("craft.accomplishment-earned"),
                citations: Cow::Borrowed(&[Cow::Borrowed("D-9999")]),
                severity: None,
            }];

        // A major accomplishment with no prior same-thread failure fails the
        // season `accomplishment-earned` join.
        let mut payload = Payload::default();
        payload.meta.episode = "e01".to_owned();
        payload.beats = vec![BeatRow {
            id: "b-triumph".to_owned(),
            scene: "s01".to_owned(),
            kind: BeatKind::Accomplishment,
            thread: "the-long-road".to_owned(),
            magnitude: Magnitude::Major,
            outcome: None,
            by: "enfys".to_owned(),
            assisted: false,
            quote: "she carried it home unaided".to_owned(),
            line: 12,
        }];

        let ctx = BatteryContext::new("");
        let findings = craft_family(&payload, &ctx);

        let craft = findings
            .iter()
            .find(|f| f.code == "craft.accomplishment-earned")
            .expect("an unearned major accomplishment must draw a craft finding");
        // Generic engine calls carry no project decision ids.
        assert!(craft.citations.is_empty(), "{craft:?}");
        assert_eq!(craft.principle.as_deref(), Some("tryfail-yes-but-no-and"));
        // And it anchors to the beat that triggered it.
        assert_eq!(craft.line, Some(12));
        assert_eq!(craft.quote.as_deref(), Some("she carried it home unaided"));

        let policy = EpisodePolicy {
            finding_policy: crate::craft::config::FindingPolicy {
                rules: Cow::Borrowed(RULES),
            },
            ..DEFAULT_EPISODE_POLICY
        };
        let configured = craft_family(
            &payload,
            &BatteryContext::new("").with_episode_policy(policy),
        );
        let craft = configured
            .iter()
            .find(|f| f.code == "craft.accomplishment-earned")
            .unwrap();
        assert_eq!(craft.citations, vec!["D-9999"]);

        let empty = Vec::new();
        let disabled = craft_family(&payload, &BatteryContext::new("").with_kb(Kb::new(&empty)));
        assert!(
            disabled.is_empty(),
            "an empty selected doctrine corpus must disable craft verdicts"
        );
    }

    #[test]
    fn outline_conformance_states_both_readings() {
        use crate::craft::outline::TryFailBeat;

        let payload = Payload {
            beats: vec![BeatRow {
                id: "b-drafted".to_owned(),
                scene: "s01".to_owned(),
                kind: BeatKind::Try,
                thread: "t".to_owned(),
                magnitude: Magnitude::Minor,
                outcome: Some(TryFailOutcome::NoBut),
                by: String::new(),
                assisted: false,
                quote: "she tried the latch".to_owned(),
                line: 7,
            }],
            ..Payload::default()
        };

        let outline = EpisodeOutline {
            id: "e01".to_owned(),
            tryfail_beats: vec![TryFailBeat {
                id: "b-outlined".to_owned(),
                attempt: "force the door".to_owned(),
                outcome: TryFailOutcome::NoAnd,
            }],
            ..Default::default()
        };

        let findings = outline_conformance(&payload, &outline, &Kb::builtin());

        // The outline promised a beat the payload lacks — both readings stated.
        let unaccounted = findings
            .iter()
            .find(|f| f.code == "outline.beat-unaccounted")
            .expect("the outlined beat is unaccounted");
        assert!(unaccounted.message.contains("b-outlined"));
        assert!(
            unaccounted.message.contains("dropped") && unaccounted.message.contains("missed"),
            "both readings must be stated: {}",
            unaccounted.message
        );

        // The payload carries a beat the outline never declared — carries its anchor.
        let undeclared = findings
            .iter()
            .find(|f| f.code == "outline.beat-undeclared")
            .expect("the drafted beat is undeclared");
        assert_eq!(undeclared.line, Some(7));
        assert_eq!(undeclared.quote.as_deref(), Some("she tried the latch"));
        // Outline-conformance findings resolve to the CRAFT-1 battery principle.
        assert!(undeclared.principle.is_some(), "{undeclared:?}");
    }

    #[test]
    fn proof_family_rejects_an_unknown_verdict_before_declared_replay() {
        use crate::Pack;
        use crate::compile;
        use crate::loader::example_docs;

        let documents = example_docs();
        let world = compile(&Pack {
            name: "camelot".to_owned(),
            documents: documents.clone(),
        })
        .unwrap();
        let context = BatteryContext::new("").with_documents(&documents);
        let mut payload = Payload {
            proof_ledgers: vec![ProofLedgerRow {
                scene: "s04".to_owned(),
                ledger: ProofLedger {
                    premises: vec!["seated(withy)".to_owned()],
                    steps: Vec::new(),
                    close: ProofClose {
                        atom: "holds(ward(door))".to_owned(),
                        expect: "hodls".to_owned(), // typo: not a real verdict
                    },
                },
            }],
            ..Payload::default()
        };
        let findings = proof_family(&world, &payload, &context);
        assert!(
            findings
                .iter()
                .any(|finding| finding.code == "proof.unknown-verdict"),
            "{findings:?}"
        );

        // Correcting the token removes that defect, while the undeclared shadow
        // still fails closed before any invented proof can be accepted.
        payload.proof_ledgers[0].ledger.close.expect = "holds".to_owned();
        let findings = proof_family(&world, &payload, &context);
        assert!(
            findings
                .iter()
                .all(|finding| finding.code != "proof.unknown-verdict")
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.code == "proof.undeclared-shadow")
        );
    }

    #[test]
    fn proof_shadow_refuses_duplicate_or_nonfinal_closes() {
        use dli_ast::decl::Decl;

        let shadow = ProofLedger {
            premises: Vec::new(),
            steps: Vec::new(),
            close: ProofClose {
                atom: "first".to_owned(),
                expect: "blocked".to_owned(),
            },
        };
        for body in [
            "",
            "close first expect blocked\n  close second expect blocked",
            "close first expect blocked\n  assume p1 later",
        ] {
            let source = format!("proof p speaker a in practice {{\n  {body}\n}}\n");
            let (document, diagnostics) = dli_syntax::parse_document(&source);
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.class.is_error()),
                "parser should reject {body}"
            );
            let declared = document
                .decls
                .iter()
                .find_map(|decl| match decl {
                    Decl::Proof(proof) => Some(proof),
                    _ => None,
                })
                .expect("proof AST remains inspectable");
            assert!(
                !proof_shadow_matches(&shadow, declared),
                "ambiguous proof shadow matched {body}"
            );
        }
    }

    #[test]
    fn proof_shadow_includes_neutral_resource_operations() {
        use dli_ast::decl::Decl;

        let source = "\
proof p speaker a in practice {
  premise p1 debt(0) from-scene
  reveal p1
  redistribute p1
  close first expect blocked
}";
        let (document, diagnostics) = dli_syntax::parse_document(source);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let declared = document
            .decls
            .iter()
            .find_map(|decl| match decl {
                Decl::Proof(proof) => Some(proof),
                _ => None,
            })
            .expect("proof");
        let mut shadow = ProofLedger {
            premises: vec!["premise p1 debt(0) from-scene".to_owned()],
            steps: vec!["reveal p1".to_owned(), "redistribute p1".to_owned()],
            close: ProofClose {
                atom: "first".to_owned(),
                expect: "blocked".to_owned(),
            },
        };
        assert!(proof_shadow_matches(&shadow, declared));
        shadow.steps.pop();
        assert!(!proof_shadow_matches(&shadow, declared));
    }

    fn proof_document(name: &str, close: &str) -> Document {
        let source = format!(
            "proof {name} speaker witness in practice {{\n  close {close} expect holds\n}}\n"
        );
        let (document, diagnostics) = dli_syntax::parse_document(&source);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.class.is_error()),
            "{diagnostics:?}"
        );
        document
    }

    #[test]
    fn qualified_proof_selection_precedes_unqualified_regardless_of_source_order() {
        let unqualified = proof_document("s01", "unqualified");
        let qualified = proof_document("e01-s01", "qualified");

        for documents in [
            vec![unqualified.clone(), qualified.clone()],
            vec![qualified.clone(), unqualified.clone()],
        ] {
            let ProofSelection::Selected(selected) =
                select_declared_proof(&documents, "e01", "s01")
            else {
                panic!("qualified declaration should be selected");
            };
            assert_eq!(selected.name.text, "e01-s01");
        }
    }

    #[test]
    fn duplicate_proofs_at_selected_precedence_fail_closed() {
        use crate::{Pack, compile};

        let documents = vec![
            proof_document("s01", "unqualified"),
            proof_document("e01-s01", "qualified-one"),
            proof_document("e01-s01", "qualified-two"),
        ];
        let world = compile(&Pack {
            name: "proof-selection".to_owned(),
            documents: Vec::new(),
        })
        .expect("empty world compiles");
        let payload = Payload {
            meta: PayloadMeta {
                episode: "e01".to_owned(),
                ..PayloadMeta::default()
            },
            ..Payload::default()
        };
        let row = ProofLedgerRow {
            scene: "s01".to_owned(),
            ledger: ProofLedger::default(),
        };
        let context = BatteryContext::new("").with_documents(&documents);

        let findings = replay_proof_shadow(&world, &payload, &context, &row);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].code, "proof.ambiguous-declaration");
        assert!(findings[0].message.contains("e01-s01"));
        assert!(findings[0].message.contains('2'));
    }

    #[test]
    fn proof_close_parsing_is_quote_safe_and_preserves_atom_identity() {
        use dli_ast::decl::Decl;

        let source = r#"proof p speaker witness in practice {
  close says(witness, "we expect victory") expect holds
}"#;
        let (document, diagnostics) = dli_syntax::parse_document(source);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let declared = document
            .decls
            .iter()
            .find_map(|decl| match decl {
                Decl::Proof(proof) => Some(proof),
                _ => None,
            })
            .expect("proof");
        let mut shadow = ProofLedger {
            premises: Vec::new(),
            steps: Vec::new(),
            close: ProofClose {
                atom: r#"says(witness, "we expect victory")"#.to_owned(),
                expect: "holds".to_owned(),
            },
        };
        assert!(proof_shadow_matches(&shadow, declared));

        shadow.close.atom = "says(witness, we expect victory)".to_owned();
        assert!(
            !proof_shadow_matches(&shadow, declared),
            "removing string quotes must change atom identity"
        );
        assert_eq!(
            parse_declared_close(
                r#"close says(witness, "we said \"expect\" victory") expect blocked"#
            ),
            Some((
                r#"says(witness, "we said \"expect\" victory")"#.to_owned(),
                "blocked".to_owned(),
            ))
        );
    }
}
