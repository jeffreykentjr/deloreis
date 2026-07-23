//! Loading craft principles from data (design §2.2, §2.4, §4).
//!
//! Step 2 of the craft-separation DAG makes the [`Principle`] record
//! **data-loadable**: with `Cow<'static, str>` fields and `Deserialize` in place,
//! the builtin corpus round-trips through a serialized record format
//! ([`PrinciplePack`]). This module carries the record-level (de)serialization, the
//! minimal file loader, and the shipped generic doctrine packs
//! ([`doctrines`]). The world-config assembly (`load_config`/`apply`/`validate`)
//! lives in [`super::worldcfg`].
//!
//! The Step-2 green criterion is the round-trip in this module's tests: serialize
//! the builtin corpus, deserialize, and assert the result equals the builtin — the
//! proof that the data format faithfully carries the doctrine.
//!
//! ## Related (other modules / steps)
//!
//! - **Step 4 — shipped generic packs.** [`doctrines`] returns the world-agnostic
//!   [`DoctrinePrinciple`](super::kb::DoctrinePrinciple) records embedded from
//!   `craft-doctrines/*.toml` (design §1.1/§4).
//! - **Step 5 — world config.** [`load_config`](super::worldcfg::load_config),
//!   [`apply`](super::worldcfg::apply), and
//!   [`validate`](super::worldcfg::validate) (design §2.4/§4) compose a world's
//!   craft config with the shipped packs into the runtime corpus.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::kb::{DoctrinePack, Kb, Principle};

/// A loadable pack of runtime [`Principle`] records — the data form of a craft
/// corpus (design §2.2).
///
/// This is the Step-2 record format: it carries the *runtime* `Principle`, so the
/// builtin corpus can be emitted to data and loaded back identically. The shipped
/// world-agnostic [`DoctrinePrinciple`](super::kb::DoctrinePrinciple) packs
/// ([`doctrines`]) plus a world's overlays compose *into* runtime principles via
/// [`super::worldcfg::apply`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PrinciplePack {
    /// The principles this pack carries, in corpus order.
    pub principles: Vec<Principle>,
}

impl PrinciplePack {
    /// A pack snapshotting the engine's default generic builtin corpus
    /// ([`Kb::builtin`]).
    ///
    /// The clone turns the composed corpus's `Cow` fields into owned records;
    /// value-equality with the builtin is preserved (a `Cow` compares by content).
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            principles: Kb::builtin().principles().to_vec(),
        }
    }

    /// A borrow-generic [`Kb`] query handle over this pack's principles.
    #[must_use]
    pub fn kb(&self) -> Kb<'_> {
        Kb::new(&self.principles)
    }
}

/// Parse a [`PrinciplePack`] from its serialized record form (the minimal loader).
///
/// The record format is JSON here; the shipped generic packs move to embedded TOML
/// in Step 4 (design §1.1). The format is an implementation detail of the loader —
/// what Step 2 pins is that the [`Principle`] record deserializes faithfully.
///
/// # Errors
/// Returns [`LoadError::Parse`] when the input is not a valid pack record.
pub fn parse_pack(json: &str) -> Result<PrinciplePack, LoadError> {
    serde_json::from_str(json).map_err(LoadError::from)
}

/// Render a [`PrinciplePack`] to its serialized record form.
///
/// # Errors
/// Returns [`LoadError::Parse`] when the pack cannot be serialized.
pub fn render_pack(pack: &PrinciplePack) -> Result<String, LoadError> {
    serde_json::to_string_pretty(pack).map_err(LoadError::from)
}

/// Load a [`PrinciplePack`] from a record file on disk.
///
/// # Errors
/// Returns [`LoadError::Io`] when the file cannot be read and [`LoadError::Parse`]
/// when its contents are not a valid pack record.
pub fn load_pack(path: &Path) -> Result<PrinciplePack, LoadError> {
    let text = std::fs::read_to_string(path)?;
    parse_pack(&text)
}

/// The shipped generic doctrine packs, embedded from `craft-doctrines/*.toml`
/// (design §1.1/§4).
///
/// Each pack file is embedded with `include_str!`, so the shipped library needs no
/// runtime files. Every principle's [`pack`](super::kb::DoctrinePrinciple::pack)
/// field is stamped from its file's declared pack name on load (the pack name is
/// written once, at the top of each file, not repeated per record). The
/// `save-the-cat` pack ships as a declared, empty stub.
///
/// # Panics
/// Panics if a shipped pack fails to parse — the packs are compiled-in constants, so
/// a parse failure is a build-time authoring bug, not a runtime condition.
#[must_use]
pub fn doctrines() -> Vec<DoctrinePack> {
    const PACKS: &[&str] = &[
        include_str!("../../craft-doctrines/sanderson-laws-of-magic.toml"),
        include_str!("../../craft-doctrines/kishotenketsu.toml"),
        include_str!("../../craft-doctrines/mice-quotient.toml"),
        include_str!("../../craft-doctrines/promise-and-plot.toml"),
        include_str!("../../craft-doctrines/try-fail-cycles.toml"),
        include_str!("../../craft-doctrines/scene-and-sequel.toml"),
        include_str!("../../craft-doctrines/character-craft.toml"),
        include_str!("../../craft-doctrines/prose-craft.toml"),
        include_str!("../../craft-doctrines/worldbuilding.toml"),
        include_str!("../../craft-doctrines/serial-fiction.toml"),
        include_str!("../../craft-doctrines/audio-drama.toml"),
        include_str!("../../craft-doctrines/save-the-cat.toml"),
    ];
    PACKS
        .iter()
        .map(|text| {
            let mut pack: DoctrinePack =
                toml::from_str(text).expect("a shipped craft-doctrine pack parses");
            let name = pack.pack.clone();
            for principle in &mut pack.principles {
                principle.pack.clone_from(&name);
            }
            pack
        })
        .collect()
}

/// A failure loading a craft pack from data.
#[derive(Debug)]
pub enum LoadError {
    /// Reading the pack file from disk failed.
    Io(std::io::Error),
    /// Deserializing (or serializing) the pack record format failed.
    Parse(serde_json::Error),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "reading craft pack: {err}"),
            Self::Parse(err) => write!(f, "parsing craft pack: {err}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Parse(err) => Some(err),
        }
    }
}

impl From<std::io::Error> for LoadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(err: serde_json::Error) -> Self {
        Self::Parse(err)
    }
}

#[cfg(test)]
mod tests {
    use super::{PrinciplePack, parse_pack, render_pack};
    use crate::craft::kb::Kb;

    /// Step-2 green criterion (design §5), post-flip: serialize the builtin generic
    /// corpus → deserialize → assert the result equals the builtin, proving the data
    /// record format carries the doctrine losslessly. The builtin is now the generic
    /// doctrine corpus (Step 5: the compiled `TPtH` table is gone), so the snapshot is
    /// the 83 shipped generic records composed with an empty world config.
    #[test]
    fn builtin_pack_round_trips_through_the_record_format() {
        let builtin = PrinciplePack::builtin();
        assert_eq!(
            builtin.principles.len(),
            83,
            "the pack snapshots the 83-record generic doctrine corpus"
        );

        let json = render_pack(&builtin).expect("the builtin corpus serializes");
        let loaded = parse_pack(&json).expect("the record format deserializes");

        // Whole-pack value equality: the record round-trips.
        assert_eq!(loaded, builtin, "the pack round-trips by value");

        // The loaded corpus equals the composed builtin field-for-field.
        assert_eq!(
            loaded.principles.as_slice(),
            Kb::builtin().principles(),
            "deserialized principles equal the builtin generic corpus"
        );
    }
}

#[cfg(test)]
mod doctrine_tests {
    use super::doctrines;
    use crate::craft::kb::{CheckableKind, Kb};

    /// The shipped doctrine ids: the 26 pure-generic (G) principles verbatim plus
    /// the 57 stripped generic+overlay (G+) principles = **83** records (design
    /// §1.1; KB classification §4). No `TPtH`-config-only (T) principle ships, and the
    /// two `Family::Pillar` misfilings are reclassified: `bar-raiser-taste-gap`
    /// ships in `prose-craft`; `wonder-amid-ruin` is a T row and does not ship.
    const EXPECTED_IDS: &[&str] = &[
        // promise-and-plot (7)
        "promise-progress-payoff",
        "promise-is-a-contract",
        "arc-promise-ledger",
        "core-vs-umbrella-plot",
        "big-p-little-p",
        "foreshadow-and-payoff",
        "subplot-is-the-relationship-question",
        // try-fail-cycles (4)
        "causality-therefore-but",
        "tryfail-yes-but-no-and",
        "yes-and-is-terminal",
        "tryfail-count-three",
        // mice-quotient (4)
        "mice-quotient",
        "mice-lifo-nesting",
        "mice-milieu-as-hub",
        "mice-event-new-equilibrium",
        // scene-and-sequel (2)
        "scene-goal-conflict-outcome",
        "sequel-reaction-dilemma-decision",
        // kishotenketsu (3)
        "kishotenketsu-ten-is-the-turn",
        "climax-is-perspective-shift",
        "four-beat-labels-earn-their-turns",
        // character-craft (9)
        "three-sliders",
        "slider-ledger-per-major-character",
        "slider-compensation",
        "proactivity-as-investigation",
        "competence-reads-cold",
        "sympathy-through-jeopardy-and-care",
        "want-versus-need",
        "contrasting-temperament-cast",
        "agency-never-victimhood",
        // sanderson-laws-of-magic (6)
        "first-law-magic",
        "second-law-limitations",
        "magic-costs-the-body",
        "third-law-expand-before-add",
        "zeroth-law-err-awesome",
        "consistency-enables-solutions",
        // worldbuilding (2)
        "iceberg-worldbuilding",
        "world-as-plot-engine",
        // prose-craft (13)
        "windowpane-vs-stained-glass",
        "pyramid-of-abstraction",
        "concrete-detail-doctrine",
        "opening-sensory-seasonal-grounding",
        "said-asked-eighty-five",
        "no-adverb-tags",
        "show-versus-tell-is-pacing",
        "sentence-rhythm-and-sound",
        "read-everything-aloud",
        "no-filter-words",
        "subtext-over-announcement",
        "distilled-imagery-not-purple",
        "bar-raiser-taste-gap",
        // serial-fiction (10)
        "episode-is-complete-kishotenketsu",
        "open-loop-discipline",
        "micro-pacing-every-installment",
        "promise-payoff-new-promise",
        "escalate-depth-not-pace",
        "reader-breathing-room",
        "first-three-episodes-compression",
        "multiple-pov-loop-management",
        "modular-world-generates-arcs",
        "cold-open-is-a-tone-promise",
        // audio-drama (23)
        "sfx-as-character",
        "audio-show-dont-tell",
        "narrated-sound-to-sfx",
        "sfx-cannot-carry-mechanics",
        "sixty-second-hook",
        "three-four-voices-per-scene",
        "vocal-signature-per-character",
        "strategic-name-pepper",
        "scripted-silence",
        "performance-direction-markers",
        "first-meeting-audibility",
        "dialogue-scenes-have-turns",
        "audio-opening-pressure",
        "specific-family-context",
        "lone-opening-dialogue-context",
        "performable-voice-strangeness",
        "no-as-you-know-bob",
        "hybrid-script-formatting",
        "clarity-is-life-or-death",
        "plot-points-per-episode",
        "aphorism-as-acoustic-logo",
        "music-as-learned-lexicon",
        "final-musical-sting",
    ];

    /// The world (`TPtH`) tokens no shipped statement or source may name — the
    /// boundary's lint enforcement (design §1.1). Whole-word, case-insensitive.
    ///
    /// This list must be a subset of the canonical `WORLD_TOKENS` in
    /// `xtask/src/boundary.rs`. The `forbidden_world_tokens_are_subset_of_boundary`
    /// test in `xtask/tests/world_tokens.rs` enforces this invariant.
    const FORBIDDEN_WORLD_TOKENS: &[&str] = &[
        "enfys",
        "aalto",
        "turning",
        "proof-speech",
        "wyrd",
        "multitude",
        "the-multitude",
        "saltmarket",
        "cymric",
        "the-debt-square",
    ];

    /// Whole-word (ASCII-boundary), case-insensitive containment: `haystack_lower`
    /// is already lowercased and `needle` is lowercase. A match must be flanked by
    /// non-alphanumeric bytes (or string ends), so the world token `turning` never
    /// fires on the generic word `returning`.
    fn contains_word(haystack_lower: &str, needle: &str) -> bool {
        let bytes = haystack_lower.as_bytes();
        let mut from = 0;
        while let Some(rel) = haystack_lower[from..].find(needle) {
            let i = from + rel;
            let end = i + needle.len();
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
            from = i + 1;
        }
        false
    }

    /// The packs parse into `DoctrinePrinciple` records: eleven populated packs plus
    /// the declared-empty `save-the-cat` stub, every principle stamped with its
    /// pack, and `kernel_rule` present exactly for the deterministic-kernel kind.
    #[test]
    fn packs_parse_into_doctrine_principles() {
        let packs = doctrines();
        assert_eq!(packs.len(), 12, "eleven packs plus the save-the-cat stub");

        let stub = packs
            .iter()
            .find(|p| p.pack == "save-the-cat")
            .expect("the save-the-cat stub ships");
        assert!(
            stub.principles.is_empty(),
            "save-the-cat is a declared empty stub (no fabricated beat-sheet principle)"
        );

        for pack in &packs {
            for principle in &pack.principles {
                assert_eq!(
                    principle.pack, pack.pack,
                    "each principle is stamped with its declaring pack"
                );
                match principle.checkable_kind {
                    CheckableKind::DeterministicKernel => assert!(
                        principle.kernel_rule.is_some(),
                        "{}: a deterministic-kernel principle names its kernel rule",
                        principle.id
                    ),
                    _ => assert!(
                        principle.kernel_rule.is_none(),
                        "{}: only a deterministic-kernel principle carries a kernel rule",
                        principle.id
                    ),
                }
            }
        }
    }

    /// Coverage: the shipped ids are **exactly** the expected 83 (26 G + 57 stripped
    /// G+ per the classification) — no more, no fewer.
    #[test]
    fn shipped_ids_are_exactly_the_expected_83() {
        assert_eq!(
            EXPECTED_IDS.len(),
            83,
            "the expectation is 26 pure-generic + 57 stripped generic+overlay"
        );

        let shipped = Kb::doctrines();
        assert_eq!(
            shipped.len(),
            83,
            "the shipped library is 83 doctrine principles"
        );

        let mut got: Vec<&str> = shipped.iter().map(|p| p.id.as_ref()).collect();
        got.sort_unstable();
        let deduped = {
            let mut d = got.clone();
            d.dedup();
            d
        };
        assert_eq!(deduped.len(), got.len(), "no duplicate shipped ids");

        let mut want: Vec<&str> = EXPECTED_IDS.to_vec();
        want.sort_unstable();
        assert_eq!(
            got, want,
            "shipped ids are exactly the expected 83 — no more, no fewer"
        );
    }

    /// The world-token lint — the boundary's enforcement: no shipped statement or
    /// source may name a world (`TPtH`) token, whole-word and case-insensitive.
    #[test]
    fn shipped_statements_and_sources_carry_no_world_tokens() {
        for principle in Kb::doctrines() {
            for (field, text) in [
                ("statement", principle.statement.as_ref()),
                ("source", principle.source.as_ref()),
            ] {
                let lower = text.to_ascii_lowercase();
                for token in FORBIDDEN_WORLD_TOKENS {
                    assert!(
                        !contains_word(&lower, token),
                        "shipped {field} of `{}` names forbidden world token `{token}`: {text}",
                        principle.id
                    );
                }
            }
        }
    }
}
