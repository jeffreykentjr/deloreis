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

use std::collections::BTreeMap;
use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use dli_core::grade::Grade;

use crate::check::Severity;
use crate::craft::PromiseKind;

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
}
