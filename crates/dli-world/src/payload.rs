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
//! # Reconciliation with the wire [`DraftPayload`](crate::draft::DraftPayload)
//!
//! [`DraftPayload`](crate::draft::DraftPayload) is the *thin*, wire-frozen
//! projection consumed by `dli.check-draft` and `dli.payload-submit` (its JSON
//! schema is committed under `contracts/`). [`Payload`] is the *fat* structured
//! shadow the merged battery reads. The two are kept apart on purpose: growing
//! `DraftPayload` in place would drift a frozen wire contract. The merge —
//! `check_draft` growing into the eight-family battery over a [`Payload`] — lands
//! in `PAY-2` as an announced additive change to the wire.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use dli_ast::clause::Clause;
use dli_ast::span::Span;
use dli_ast::term::AtomExpr;
use dli_core::grade::Grade;
use dli_syntax::parse_clause_atom;

use crate::check::factcheck::Verdict;
use crate::check::{self, CompiledWorld, Severity, WorldFinding};
use crate::craft::outline::EpisodeOutline;
use crate::craft::season::{
    CraftLedger, DEFAULT_THRESHOLDS, EpisodeMeta, LedgerBeat, LedgerPromise, LedgerScene,
    Thresholds, check_season_rules,
};
use crate::craft::{PromiseKind, TryFailOutcome, kb};
use crate::language::{aural_clash, vet_candidate};

// --- the merged report finding (the seam `PAY-2` fills) ----------------------

/// One finding in the merged payload report.
///
/// The battery over a [`Payload`] emits these; `PAY-1` emits the anchoring and
/// staleness families, and the remaining families (continuity, factcheck,
/// outline-conformance, craft, proof, naming, phase, lexicon) merge in here in
/// `PAY-2`. Every finding is fully attributed: the draft [`line`](Self::line) and
/// verbatim [`quote`](Self::quote) it turns on, the [`code`](Self::code) of the
/// rule or check that raised it, the [`principle`](Self::principle) it cites when
/// the check is craft, its supporting [`citations`](Self::citations), and the
/// [`grade`](Self::grade) of the evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
pub struct ExtractorManifest {
    /// The payload schema version, e.g. `payload/v1`.
    #[serde(default)]
    pub schema: String,
    /// The extractor identity for each extracted field (`beats`, `claims`, …),
    /// flattened alongside `schema` as the spec's YAML has them.
    #[serde(flatten)]
    pub fields: BTreeMap<String, ExtractorRef>,
}

/// The identity of the model and prompt that produced one extracted field (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExtractorRef {
    /// The model that produced the field.
    pub model: String,
    /// The `sha256:` hash of the rendered extraction prompt.
    pub prompt: String,
}

/// A factcheck atom, anchored to the draft (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Claim {
    /// The atom in surface form, e.g. `holds(enfys, consent(household, door))`.
    /// `PAY-2` parses this into a typed atom for the fact-checker.
    pub atom: String,
    /// The verbatim draft text this claim is drawn from.
    pub quote: String,
    /// The draft line the quote sits on (1-based).
    pub line: u32,
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
pub struct ProofLedgerRow {
    /// The scene the ledger belongs to.
    pub scene: String,
    /// The ledger itself.
    pub ledger: ProofLedger,
}

/// The premises, steps, and close of a shadowed proof (§7.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
/// [`outline`](Self::outline) enables the two-sided outline-conformance diff
/// (`PAY-3`); the [`season`](Self::season) is the committed season ledger the
/// payload's beats merge into so the craft-season joins reach across episodes
/// (empty = the payload is checked against itself alone); and
/// [`thresholds`](Self::thresholds) is the craft policy data
/// ([`DEFAULT_THRESHOLDS`] when uncalibrated).
#[derive(Debug, Clone, Copy)]
pub struct BatteryContext<'a> {
    /// The draft prose the payload shadows (for anchoring + staleness).
    pub source: &'a str,
    /// The episode outline to diff against (two-sided outline-conformance).
    pub outline: Option<&'a EpisodeOutline>,
    /// The committed season ledger the payload's beats merge into.
    pub season: Option<&'a CraftLedger>,
    /// The craft-rule thresholds (policy data).
    pub thresholds: Thresholds,
}

impl<'a> BatteryContext<'a> {
    /// A context over just the draft source, with default thresholds and no
    /// outline or season — the payload checked against the world and itself.
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            outline: None,
            season: None,
            thresholds: DEFAULT_THRESHOLDS,
        }
    }

    /// Add the episode outline to diff against, builder-style.
    #[must_use]
    pub fn with_outline(mut self, outline: &'a EpisodeOutline) -> Self {
        self.outline = Some(outline);
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
/// 5. **Proof checks** — the proof ledgers' close verdicts and completeness.
/// 6. **Naming** — introduced names vetted by the oracle.
/// 7. **Quote anchoring** — every quote verbatim at its line (a hard error).
/// 8. **Staleness** — the source hash still pins the draft.
///
/// This is the fat-payload sibling of the wire-frozen [`check_draft`](crate::check_draft);
/// wiring `dli.check-draft` / `dli.payload-submit` to it is an announced additive
/// wire change.
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

    // (4) craft season rules over the merged ledger.
    out.extend(craft_family(payload, ctx));

    // (6) naming and (5) proof-ledger.
    out.extend(naming_family(world, payload));
    out.extend(proof_family(payload));

    // (3) two-sided outline-conformance, when an outline is supplied (PAY-3).
    if let Some(outline) = ctx.outline {
        out.extend(outline_conformance(payload, outline));
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
        .filter(|(report, _)| report.is_defect())
        .map(|(report, parsed)| {
            let code = match report.verdict {
                Verdict::Contradicted => "factcheck.contradicted",
                Verdict::Unsupported => "factcheck.unsupported",
                Verdict::Supported => "factcheck.supported",
            };
            let mut finding = PayloadFinding::error(code, report.reason)
                .at_line(parsed.claim.line)
                .with_quote(&parsed.claim.quote);
            finding.citations = report.citations;
            finding
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

/// The craft family: merge the payload's beats into the season ledger and run
/// §3's rules; each finding cites its governing principle (via [`kb::why`]) and,
/// where its message names a payload beat or promise, carries that item's line
/// and quote.
fn craft_family(payload: &Payload, ctx: &BatteryContext) -> Vec<PayloadFinding> {
    let ledger = build_ledger(payload, ctx);
    let anchors = beat_promise_anchors(payload);
    check_season_rules(&ledger, &ctx.thresholds)
        .into_iter()
        .map(|wf| {
            let mut finding = world_finding_to_payload(&wf);
            if let Some(principle) = kb::why(&wf.code).first() {
                finding.principle = Some(principle.id.to_owned());
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

/// The proof-ledger family: the payload carries a proof *shadow* ({premises,
/// steps, close}), not a full castable proof block with a named practice, so this
/// validates what the shadow can prove on its own — the close atom is present,
/// the expected verdict is a real verdict (an unknown token silently reading as
/// `holds` would gut the discipline), and the ledger rests on something. The
/// close atom's grammar is left unchecked: it is proof-language, not kernel
/// Datalog, so a `holds(ward(door))`-style nested term is legitimate. Full replay
/// through the practice checker awaits an extractor that carries the practice
/// reference and the castable block.
fn proof_family(payload: &Payload) -> Vec<PayloadFinding> {
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
    }
    out
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
pub fn outline_conformance(payload: &Payload, outline: &EpisodeOutline) -> Vec<PayloadFinding> {
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
        let Some(payload_scene) = payload.scenes.iter().find(|s| s.id == outline_scene.id) else {
            continue;
        };
        for member in &outline_scene.speaking_cast {
            if !payload_scene.cast.contains(member) {
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
    // Attribute each conformance finding to its governing principle. An
    // `outline.*` code resolves to the CRAFT-1 battery principles via the KB.
    for finding in &mut out {
        if let Some(principle) = kb::why(&finding.code).first() {
            finding.principle = Some(principle.id.to_owned());
        }
    }
    out
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

        // The extractor manifest keeps `schema` flattened beside the field keys.
        assert!(yaml.contains("schema: payload/v1"), "{yaml}");

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

        // The saltmarket golden world: `adjacent(saltmarket, the-shallows)` is
        // ratified-certain; `rings(tide-bell, saltmarket)` is proposed shadow lore.
        let world = compile(&Pack {
            name: "saltmarket".to_owned(),
            documents: example_docs(),
        })
        .unwrap();

        let source = "the bell and the market share a wall\nthe tide-bell rings for the market\n";
        let mut payload = Payload::default();
        payload.meta.source.sha256 = source_hash(source);
        payload.claims = vec![
            Claim {
                atom: "adjacent(saltmarket, the-shallows)".to_owned(),
                quote: "the bell and the market share a wall".to_owned(),
                line: 1,
            },
            Claim {
                atom: "rings(tide-bell, saltmarket)".to_owned(),
                quote: "the tide-bell rings for the market".to_owned(),
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
            .expect("rings must be unsupported");
        assert_eq!(unsupported.line, Some(2));
        assert_eq!(
            unsupported.quote.as_deref(),
            Some("the tide-bell rings for the market")
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
    fn craft_family_flags_an_unearned_major_accomplishment() {
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
        // The finding carries its rule id (the code) and D-citation as support.
        // (The KB binds this fine season rule to no principle yet — honest
        // checkable-debt — so `principle` is legitimately None here.)
        assert!(craft.citations.contains(&"D-0128".to_owned()), "{craft:?}");
        // And it anchors to the beat that triggered it.
        assert_eq!(craft.line, Some(12));
        assert_eq!(craft.quote.as_deref(), Some("she carried it home unaided"));
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

        let findings = outline_conformance(&payload, &outline);

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
    fn proof_family_rejects_an_unknown_verdict_and_passes_a_clean_ledger() {
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
        let findings = proof_family(&payload);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].code, "proof.unknown-verdict");

        // A real verdict on a complete ledger is clean.
        payload.proof_ledgers[0].ledger.close.expect = "holds".to_owned();
        assert!(proof_family(&payload).is_empty());
    }
}
