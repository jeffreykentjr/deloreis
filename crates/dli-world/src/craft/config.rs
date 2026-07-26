//! World-supplied craft policy — the season-battery **policy** Step 3 lifts out of
//! [`season`](super::season) and [`outline`](super::outline) so the engine ships
//! the *mechanism* and a world supplies the *content*
//! (`docs/design-notes/craft-separation-design.md` §3). The generic finding
//! policy is deliberately empty: project decision ids belong in caller config,
//! never in an engine default. The remaining structural defaults keep the
//! checker usable when no project policy is supplied.
//!
//! Project citation literals live in caller-owned `finding-policy.toml`, not in
//! this module or the checker. Policies use `Cow`: generic defaults borrow static
//! tables, while loaded caller policies own their data without process leaks.

use std::borrow::Cow;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::outline::{EpisodeOutline, MaterialCategoryId, NarratorMode};
use crate::check::Severity;

// ---- 3a  FindingPolicy ------------------------------------------------------

/// The citation (and optional severity override) a finding code carries — the
/// per-code policy that replaces the inline `.with_citations(vec!["D-nnnn"])`
/// calls that used to sit in the checker.
#[derive(Debug, Clone)]
pub struct FindingRule<'a> {
    /// The dotted finding code this rule governs (e.g. `outline.sfx-not-sparse`).
    pub code: Cow<'a, str>,
    /// The citations attached to the finding, in order.
    pub citations: Cow<'a, [Cow<'a, str>]>,
    /// An optional severity override; `None` keeps the code's intrinsic severity.
    pub severity: Option<Severity>,
}

/// The finding-policy table: citations + optional severity keyed by finding code.
/// A world supplies its own; [`DEFAULT_FINDING_POLICY`] is world-neutral.
#[derive(Debug, Clone)]
pub struct FindingPolicy<'a> {
    /// The per-code rules; codes absent from the table carry no citation.
    pub rules: Cow<'a, [FindingRule<'a>]>,
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
            .map(|r| r.citations.iter().map(ToString::to_string).collect())
            .unwrap_or_default()
    }

    /// The lead citation token a finding's prose weaves in (empty when none).
    #[must_use]
    pub fn cite_token(&self, code: &str) -> &str {
        self.rule(code)
            .and_then(|r| r.citations.first())
            .map_or("", Cow::as_ref)
    }

    /// The optional severity override for `code` (none in the default policy).
    #[must_use]
    pub fn severity_override(&self, code: &str) -> Option<Severity> {
        self.rule(code).and_then(|r| r.severity)
    }
}

/// The world-neutral default. Projects attach citations and severity overrides
/// through `finding-policy.toml`.
pub const DEFAULT_FINDING_POLICY: FindingPolicy<'static> = FindingPolicy {
    rules: Cow::Borrowed(&[]),
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
/// [`Allowed`](Disposition::Allowed).
#[derive(Debug, Clone)]
pub struct NarratorPolicy<'a> {
    /// Per-mode dispositions; only non-permitted modes need appear.
    pub mode_rules: Cow<'a, [NarratorModeRule]>,
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

/// The doctrine-neutral narrator policy: every framing mode is permitted.
///
/// Projects that forbid host framing or direct address declare that policy in
/// `narrator-policy.toml`.
pub const DEFAULT_NARRATOR_POLICY: NarratorPolicy<'static> = NarratorPolicy {
    mode_rules: Cow::Borrowed(&[]),
    direct_address: Disposition::Allowed,
};

// ---- 3c  ProvenanceFormats --------------------------------------------------

/// The reference-format patterns the material ledger's provenance guards accept.
/// Externalizing them keeps project decision and research-path conventions out
/// of the checker.
#[derive(Debug, Clone)]
pub struct ProvenanceFormats<'a> {
    /// The canon-decision prefix (`D-`).
    pub canon_prefix: Cow<'a, str>,
    /// The exact digit count following the canon prefix (`4`).
    pub canon_digits: usize,
    /// Accepted research-URL scheme prefixes, tried in order.
    pub research_url_schemes: Cow<'a, [Cow<'a, str>]>,
    /// The research-id prefix (`RR-`).
    pub research_id_prefix: Cow<'a, str>,
    /// The exact digit count following the research-id prefix (`3`).
    pub research_id_digits: usize,
    /// The research-doc path prefix (`docs/research/RR-`).
    pub research_path_prefix: Cow<'a, str>,
    /// The digit count following the research-doc path prefix (`3`).
    pub research_path_digits: usize,
    /// The research-doc path suffix (`.md`).
    pub research_path_suffix: Cow<'a, str>,
}

impl ProvenanceFormats<'_> {
    /// Whether `reference` is a canon decision id — `prefix` + exactly
    /// `canon_digits` ASCII digits, nothing else (the legacy `^D-\d{4}$`).
    #[must_use]
    pub fn is_canon_ref(&self, reference: &str) -> bool {
        if self.canon_prefix.is_empty() {
            return !reference.trim().is_empty();
        }
        let Some(digits) = reference.strip_prefix(self.canon_prefix.as_ref()) else {
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
        let has_configured_format = !self.research_url_schemes.is_empty()
            || !self.research_id_prefix.is_empty()
            || !self.research_path_prefix.is_empty();
        if !has_configured_format {
            return !reference.trim().is_empty();
        }
        for scheme in self.research_url_schemes.iter() {
            if let Some(rest) = reference.strip_prefix(scheme.as_ref()) {
                return !rest.is_empty() && rest.chars().all(|c| !c.is_whitespace());
            }
        }
        if !self.research_id_prefix.is_empty()
            && let Some(digits) = reference.strip_prefix(self.research_id_prefix.as_ref())
            && digits.len() == self.research_id_digits
            && digits.bytes().all(|b| b.is_ascii_digit())
        {
            return true;
        }
        if !self.research_path_prefix.is_empty()
            && let Some(rest) = reference.strip_prefix(self.research_path_prefix.as_ref())
        {
            let digits: String = rest.chars().take(self.research_path_digits).collect();
            if digits.len() == self.research_path_digits
                && digits.bytes().all(|b| b.is_ascii_digit())
            {
                let tail = &rest[self.research_path_digits..];
                return tail.ends_with(self.research_path_suffix.as_ref()) && !tail.contains('/');
            }
        }
        false
    }
}

/// The doctrine-neutral provenance formats. With no caller-defined format,
/// nonblank references are accepted without imposing one project's identifier
/// or repository conventions.
pub const DEFAULT_PROVENANCE: ProvenanceFormats<'static> = ProvenanceFormats {
    canon_prefix: Cow::Borrowed(""),
    canon_digits: 0,
    research_url_schemes: Cow::Borrowed(&[]),
    research_id_prefix: Cow::Borrowed(""),
    research_id_digits: 0,
    research_path_prefix: Cow::Borrowed(""),
    research_path_digits: 0,
    research_path_suffix: Cow::Borrowed(""),
};

// ---- 3d  MaterialCatalog ----------------------------------------------------

/// One material-planning category and whether a worldbuilding review requires an
/// answer or an explicit unknown for it.
#[derive(Debug, Clone)]
pub struct MaterialCategory {
    /// The caller-owned category key.
    pub category: MaterialCategoryId,
    /// Whether a material-worldbuilding review requires it.
    pub required: bool,
}

/// The world's material catalog: the ordered categories the completeness and
/// blank-provenance checks loop over.
#[derive(Debug, Clone)]
pub struct MaterialCatalog<'a> {
    /// The catalog entries, in ledger order.
    pub entries: Cow<'a, [MaterialCategory]>,
}

impl MaterialCatalog<'_> {
    /// Every caller-owned catalog category, in ledger order.
    pub fn all(&self) -> impl Iterator<Item = &MaterialCategoryId> {
        self.entries.iter().map(|c| &c.category)
    }

    /// The categories a material-worldbuilding review requires, in ledger order
    /// The caller-owned required categories, in ledger order.
    pub fn required(&self) -> impl Iterator<Item = &MaterialCategoryId> {
        self.entries
            .iter()
            .filter(|c| c.required)
            .map(|c| &c.category)
    }
}

/// The doctrine-neutral material catalog. Projects opt into a taxonomy through
/// `material-catalog.toml`.
pub const DEFAULT_MATERIAL_CATALOG: MaterialCatalog<'static> = MaterialCatalog {
    entries: Cow::Borrowed(&[]),
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
