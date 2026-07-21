//! World-supplied craft policy — the season-battery **policy** Step 3 lifts out of
//! [`season`](super::season) and [`outline`](super::outline) so the engine ships
//! the *mechanism* and a world supplies the *content*
//! (`docs/design-notes/craft-separation-design.md` §3). Each default here
//! reproduces the consuming repo's behavior **byte-for-byte** (zero behavior
//! change): [`DEFAULT_FINDING_POLICY`], [`DEFAULT_NARRATOR_POLICY`],
//! [`DEFAULT_PROVENANCE`], [`DEFAULT_MATERIAL_CATALOG`], and [`DEFAULT_ARC_SHAPES`]
//! carry the legacy citations, dispositions, patterns, catalog, and turn slot, so
//! `check_episode(ep, opts, &DEFAULT_EPISODE_POLICY)` is identical to the
//! pre-Step-3 checker.
//!
//! The `D-nnnn` citation literals and the material ontology live **here** (config
//! data), not in the checker: after Step 3 neither `season.rs` nor `outline.rs`
//! carries a decision literal. In Step 5 these defaults relocate to the factory
//! (`worlds/tpth/craft/*`) and the engine ships generic (empty / URL) defaults;
//! this module is their interim engine-side home while the checker learns to read
//! them as config. All types are borrowed-slice / `Copy` values (matching
//! [`EpisodePolicy`](super::season::EpisodePolicy)'s zero-copy style) so each
//! default is a `const`; the serde/`JsonSchema` loadable form lands with the
//! `CraftConfig` fold, not here.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::outline::{EpisodeOutline, MaterialUnknown, NarratorMode};
use crate::check::Severity;

// ---- 3a  FindingPolicy ------------------------------------------------------

/// The citation (and optional severity override) a finding code carries — the
/// per-code policy that replaces the inline `.with_citations(vec!["D-nnnn"])`
/// calls that used to sit in the checker.
#[derive(Debug, Clone, Copy)]
pub struct FindingRule<'a> {
    /// The dotted finding code this rule governs (e.g. `outline.sfx-not-sparse`).
    pub code: &'a str,
    /// The citations attached to the finding, in order.
    pub citations: &'a [&'a str],
    /// An optional severity override; `None` keeps the code's intrinsic severity.
    pub severity: Option<Severity>,
}

/// The finding-policy table: citations + optional severity keyed by finding code.
/// A world supplies its own; [`DEFAULT_FINDING_POLICY`] reproduces today's exact
/// citations.
#[derive(Debug, Clone, Copy)]
pub struct FindingPolicy<'a> {
    /// The per-code rules; codes absent from the table carry no citation.
    pub rules: &'a [FindingRule<'a>],
}

impl<'a> FindingPolicy<'a> {
    fn rule(&self, code: &str) -> Option<&FindingRule<'a>> {
        self.rules.iter().find(|r| r.code == code)
    }

    /// The citations to attach to `code`, owned (empty when the policy has no rule
    /// for it) — the drop-in for the legacy `.with_citations(vec![…])`.
    #[must_use]
    pub fn citations(&self, code: &str) -> Vec<String> {
        self.rule(code)
            .map(|r| r.citations.iter().map(|c| (*c).to_owned()).collect())
            .unwrap_or_default()
    }

    /// The lead citation token a finding's prose weaves in (empty when none) — the
    /// `D-0179` in `"…accounting for D-0179 audio mode"`. For the default policy
    /// this is the legacy literal, so the rendered message is byte-identical.
    #[must_use]
    pub fn cite_token(&self, code: &str) -> &'a str {
        self.rule(code)
            .and_then(|r| r.citations.first().copied())
            .unwrap_or("")
    }

    /// The optional severity override for `code` (none in the default policy).
    #[must_use]
    pub fn severity_override(&self, code: &str) -> Option<Severity> {
        self.rule(code).and_then(|r| r.severity)
    }
}

/// Today's finding policy: the exact citations the checker used to inline. The
/// codes are the full dotted forms the finding carries.
pub const DEFAULT_FINDING_POLICY: FindingPolicy<'static> = FindingPolicy {
    rules: &[
        FindingRule {
            code: "outline.no-audio-plan",
            citations: &["D-0179"],
            severity: None,
        },
        FindingRule {
            code: "outline.narrator-direct-address",
            citations: &["D-0176"],
            severity: None,
        },
        FindingRule {
            code: "outline.narrator-host-frame",
            citations: &["D-0176"],
            severity: None,
        },
        FindingRule {
            code: "outline.sfx-not-sparse",
            citations: &["D-0179"],
            severity: None,
        },
        FindingRule {
            code: "outline.ten-reads-as-battle",
            citations: &["D-0052"],
            severity: None,
        },
        FindingRule {
            code: "craft.accomplishment-earned",
            citations: &["D-0128"],
            severity: None,
        },
        FindingRule {
            code: "craft.arc-shape",
            citations: &["D-0052"],
            severity: None,
        },
    ],
};

// ---- 3b  NarratorPolicy -----------------------------------------------------

/// What a narrator-framing choice earns: an error, a warning, or nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    /// The framing is forbidden — an error finding.
    Error,
    /// The framing is discouraged — a warn finding.
    Warn,
    /// The framing is permitted — no finding.
    #[default]
    Allowed,
}

/// The disposition a single narrator mode earns.
#[derive(Debug, Clone, Copy)]
pub struct NarratorModeRule {
    /// The narrator mode this rule governs.
    pub mode: NarratorMode,
    /// Its disposition.
    pub disposition: Disposition,
}

/// The narrator-framing policy: a disposition per mode plus the direct-address
/// disposition. A mode absent from [`mode_rules`](Self::mode_rules) is
/// [`Allowed`](Disposition::Allowed). [`DEFAULT_NARRATOR_POLICY`] reproduces the
/// host-narrator / direct-address bans.
#[derive(Debug, Clone, Copy)]
pub struct NarratorPolicy<'a> {
    /// Per-mode dispositions; only non-permitted modes need appear.
    pub mode_rules: &'a [NarratorModeRule],
    /// The disposition a direct-audience-address plan earns.
    pub direct_address: Disposition,
}

impl NarratorPolicy<'_> {
    /// The disposition for `mode` — [`Allowed`](Disposition::Allowed) when the
    /// policy names no rule for it.
    #[must_use]
    pub fn mode(&self, mode: NarratorMode) -> Disposition {
        self.mode_rules
            .iter()
            .find(|r| r.mode == mode)
            .map_or(Disposition::Allowed, |r| r.disposition)
    }
}

/// Today's narrator policy: host framing and direct address are errors, every
/// other mode is permitted.
pub const DEFAULT_NARRATOR_POLICY: NarratorPolicy<'static> = NarratorPolicy {
    mode_rules: &[NarratorModeRule {
        mode: NarratorMode::Host,
        disposition: Disposition::Error,
    }],
    direct_address: Disposition::Error,
};

// ---- 3c  ProvenanceFormats --------------------------------------------------

/// The reference-format patterns the material ledger's provenance guards accept —
/// the shapes `is_canon_ref` / `is_research_ref` used to hard-code. Externalizing
/// them here keeps the ledger's `D-`/`RR-`/`docs/research/` conventions out of the
/// checker; [`DEFAULT_PROVENANCE`] reproduces the legacy matchers exactly.
#[derive(Debug, Clone, Copy)]
pub struct ProvenanceFormats<'a> {
    /// The canon-decision prefix (`D-`).
    pub canon_prefix: &'a str,
    /// The exact digit count following the canon prefix (`4`).
    pub canon_digits: usize,
    /// Accepted research-URL scheme prefixes, tried in order.
    pub research_url_schemes: &'a [&'a str],
    /// The research-id prefix (`RR-`).
    pub research_id_prefix: &'a str,
    /// The exact digit count following the research-id prefix (`3`).
    pub research_id_digits: usize,
    /// The research-doc path prefix (`docs/research/RR-`).
    pub research_path_prefix: &'a str,
    /// The digit count following the research-doc path prefix (`3`).
    pub research_path_digits: usize,
    /// The research-doc path suffix (`.md`).
    pub research_path_suffix: &'a str,
}

impl ProvenanceFormats<'_> {
    /// Whether `reference` is a canon decision id — `prefix` + exactly
    /// `canon_digits` ASCII digits, nothing else (the legacy `^D-\d{4}$`).
    #[must_use]
    pub fn is_canon_ref(&self, reference: &str) -> bool {
        let Some(digits) = reference.strip_prefix(self.canon_prefix) else {
            return false;
        };
        digits.len() == self.canon_digits && digits.bytes().all(|b| b.is_ascii_digit())
    }

    /// Whether `reference` is a labelled research reference: a scheme URL, an
    /// `RR-nnn` id, or a `docs/research/RR-nnn*.md` path (the legacy
    /// `^(?:https?://\S+|RR-\d{3}|docs/research/RR-\d{3}[^/]*\.md)$`).
    // The suffix match is intentionally case-sensitive: the legacy regex `\.md$`
    // matches only lowercase, and parity requires the same.
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    #[must_use]
    pub fn is_research_ref(&self, reference: &str) -> bool {
        for scheme in self.research_url_schemes {
            if let Some(rest) = reference.strip_prefix(scheme) {
                return !rest.is_empty() && rest.chars().all(|c| !c.is_whitespace());
            }
        }
        if let Some(digits) = reference.strip_prefix(self.research_id_prefix)
            && digits.len() == self.research_id_digits
            && digits.bytes().all(|b| b.is_ascii_digit())
        {
            return true;
        }
        if let Some(rest) = reference.strip_prefix(self.research_path_prefix) {
            let digits: String = rest.chars().take(self.research_path_digits).collect();
            if digits.len() == self.research_path_digits
                && digits.bytes().all(|b| b.is_ascii_digit())
            {
                let tail = &rest[self.research_path_digits..];
                return tail.ends_with(self.research_path_suffix) && !tail.contains('/');
            }
        }
        false
    }
}

/// Today's provenance formats: the consuming repo's `D-nnnn` / `RR-nnn` /
/// `docs/research/RR-nnn*.md` / `http(s)://` conventions.
pub const DEFAULT_PROVENANCE: ProvenanceFormats<'static> = ProvenanceFormats {
    canon_prefix: "D-",
    canon_digits: 4,
    research_url_schemes: &["https://", "http://"],
    research_id_prefix: "RR-",
    research_id_digits: 3,
    research_path_prefix: "docs/research/RR-",
    research_path_digits: 3,
    research_path_suffix: ".md",
};

// ---- 3d  MaterialCatalog ----------------------------------------------------

/// One material-planning category and whether a worldbuilding review requires an
/// answer or an explicit unknown for it. The category *key* stays the wire-stable
/// [`MaterialUnknown`] (its `field_name` and finding codes are unchanged); the
/// catalog externalizes only which categories exist and which are required.
#[derive(Debug, Clone, Copy)]
pub struct MaterialCategory {
    /// The category key (its `snake_case` ledger field name is unchanged).
    pub category: MaterialUnknown,
    /// Whether a material-worldbuilding review requires it.
    pub required: bool,
}

/// The world's material catalog: the ordered categories the completeness and
/// blank-provenance checks loop over. [`DEFAULT_MATERIAL_CATALOG`] is the legacy
/// 12 categories in ledger order with the legacy required set.
#[derive(Debug, Clone, Copy)]
pub struct MaterialCatalog<'a> {
    /// The catalog entries, in ledger order.
    pub entries: &'a [MaterialCategory],
}

impl MaterialCatalog<'_> {
    /// Every catalog category, in ledger order (the legacy `MaterialUnknown::ALL`).
    pub fn all(&self) -> impl Iterator<Item = MaterialUnknown> + '_ {
        self.entries.iter().map(|c| c.category)
    }

    /// The categories a material-worldbuilding review requires, in ledger order
    /// (the legacy `MaterialUnknown::REQUIRED`).
    pub fn required(&self) -> impl Iterator<Item = MaterialUnknown> + '_ {
        self.entries
            .iter()
            .filter(|c| c.required)
            .map(|c| c.category)
    }
}

/// Today's material catalog: the 12 planning categories in ledger order, every one
/// required except `ritual_obligation`.
pub const DEFAULT_MATERIAL_CATALOG: MaterialCatalog<'static> = MaterialCatalog {
    entries: &[
        MaterialCategory {
            category: MaterialUnknown::SeasonTimePressure,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::StapleDependency,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::WaterDependency,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::FuelDependency,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::ShelterDependency,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::LaborGroup,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::Source,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::Transport,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::Storage,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::FailurePoint,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::Institution,
            required: true,
        },
        MaterialCategory {
            category: MaterialUnknown::RitualObligation,
            required: false,
        },
    ],
};

// ---- 3e  ArcShapeCatalog ----------------------------------------------------

/// Which kishōtenketsu slot names the *turn* the outline battle-scan reads. The
/// `ki`/`shō`/`ten`/`ketsu` fields stay on the outline (kishōtenketsu is a named
/// framework, not world content); the catalog drives only *which* slot is scanned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TurnSlot {
    /// The opening image.
    Ki,
    /// The development.
    Sho,
    /// The turn.
    Ten,
    /// The resolution.
    Ketsu,
}

impl TurnSlot {
    /// The outline text this slot names.
    #[must_use]
    pub fn text(self, ep: &EpisodeOutline) -> &str {
        match self {
            TurnSlot::Ki => &ep.ki,
            TurnSlot::Sho => &ep.sho,
            TurnSlot::Ten => &ep.ten,
            TurnSlot::Ketsu => &ep.ketsu,
        }
    }
}

/// The world's arc-shape assumptions the outline battery reads. Today only the
/// turn slot is externalized here; the season-ledger arc bands (`ten_position`,
/// reveal-lead) remain in the calibrated [`Thresholds`](super::season::Thresholds)
/// seam. [`DEFAULT_ARC_SHAPES`] scans the *ten*, matching the legacy checker.
#[derive(Debug, Clone, Copy)]
pub struct ArcShapeCatalog {
    /// The slot the `ten-reads-as-battle` scan targets.
    pub turn_slot: TurnSlot,
}

/// Today's arc shape: the turn is the *ten*.
pub const DEFAULT_ARC_SHAPES: ArcShapeCatalog = ArcShapeCatalog {
    turn_slot: TurnSlot::Ten,
};
