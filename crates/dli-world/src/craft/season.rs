//! The deterministic outline checker — the `outline.py` port (`CRAFT-1`).
//!
//! [`check_episode`] and [`check_season`] reproduce the consuming repo's
//! `fantasy.craft.outline.check_outline` / `check_season` exactly, with the legacy
//! violation codes carried across intact under the `outline.` family (see the
//! mapping table in [`super`]). Every finding is [`WorldFinding`]-shaped: a dotted
//! [`code`](WorldFinding::code), a [`Severity`],
//! [`Grade::Checked`](dli_core::grade::Grade::Checked) (these are
//! deterministic structural checks), a domain-spoken message, and — where the
//! legacy check cites an authorial decision — its `D-nnnn` citations.
//!
//! The legacy `Violation.where` (the id a finding attaches to) is preserved as the
//! leading token of the message (`"{where}: {message}"`), matching the legacy
//! `render_violations` layout, since [`WorldFinding`] has no separate `where`
//! slot.
//!
//! **Fail-closed, not fail-hard.** Two legacy pydantic *constructor* validators —
//! promise/arc-promise payoff-XOR-carry, and the material-ledger provenance
//! guards — raised `ValueError` at construction in Python. Serde does not run
//! them, so this port lowers them to check findings, consistent with the loader's
//! rule that malformed input is a fail-closed *finding*, never a panic
//! ([`compile`](crate::compile)). On well-formed outlines they are silent, so the
//! parity oracle's verdict is unchanged; on malformed outlines the kernel reports
//! rather than crashes.
//!
//! **Thresholds are policy data** ([06-craft-and-payloads](../../../../docs/spec/06-craft-and-payloads.md)
//! §4): the cast ceiling, the hook deadline, the slider floors, and the battle
//! lexicon live in [`policy`] as D-citable leaf tables, not magic numbers inline.
//! Each threshold-family finding rides at [`Severity::Warn`] until calibrated —
//! which is exactly the legacy severity, so parity holds.

use std::collections::{BTreeSet, HashMap};

use super::config::{
    ArcShapeCatalog, DEFAULT_ARC_SHAPES, DEFAULT_FINDING_POLICY, DEFAULT_MATERIAL_CATALOG,
    DEFAULT_NARRATOR_POLICY, DEFAULT_PROVENANCE, Disposition, FindingPolicy, MaterialCatalog,
    NarratorPolicy, ProvenanceFormats,
};
use super::outline::{
    AudioLoad, EpisodeOutline, MaterialAssumptionLedger, MaterialUnknown, NarratorMode, SceneWork,
    SeasonOutline, SfxLoad, SfxSemanticLoad, TryFailOutcome,
};
use crate::check::{Severity, WorldFinding};

/// Policy leaf tables — the calibratable thresholds and lexicons the checker
/// reads (spec §4). Substitutable data, never magic numbers inline; each rides at
/// warn severity until calibrated against the season-1 corpus.
pub mod policy {
    /// The most speaking voices a scene may carry before `cast-over-four` warns.
    ///
    /// Toscan's ear-tracking ceiling; ported from the legacy inline `> 4`.
    pub const SCENE_CAST_CEILING: usize = 4;

    /// The hook must land by this many seconds (`hook-late`; the audio hook).
    pub const HOOK_SECONDS_CEILING: u32 = 60;

    /// At or above this, no single character slider reads as flat.
    ///
    /// The legacy `character-slider-flat` fires when the *highest* of the three
    /// dials is below this value.
    pub const SLIDER_FLAT_CEILING: u8 = 5;

    /// Below this, a character slider needs an explicit compensation note
    /// (`slider-compensation-missing`, keyed on the *lowest* dial).
    pub const SLIDER_LOW_FLOOR: u8 = 4;

    /// Single-word combat vocabulary that, in the *ten*, signals a battle-climax
    /// habit where a perspective shift belongs (`ten-reads-as-battle`).
    /// The legacy `_BATTLE_IN_TEN` regex's wordlist, ported to data.
    pub const BATTLE_SINGLE: &[&str] = &[
        "battle", "sword", "blade", "army", "armies", "slay", "slain", "behead", "duel", "charge",
        "warhost",
    ];

    /// Multi-token combat phrases (each an adjacent pair after tokenizing), with
    /// the alternative leading tokens the legacy regex allowed.
    ///
    /// `(&["strike", "strikes"], "down")` reproduces `strike(?:s)? down`; the
    /// `war[- ]?host` and `final blow` and `kill(?:s|ed)? the` cases follow.
    pub const BATTLE_PAIRS: &[(&[&str], &str)] = &[
        (&["war"], "host"),
        (&["strike", "strikes"], "down"),
        (&["kill", "kills", "killed"], "the"),
        (&["final"], "blow"),
    ];
}

/// The per-episode outline policy — the calibratable thresholds, battle lexicon,
/// and world-supplied season policy the `CRAFT-1` battery reads, threaded in as an
/// argument rather than read from a global (design §2.3). The borrowed slices keep
/// this a `const`-constructible, zero-copy value; [`DEFAULT_EPISODE_POLICY`] wires
/// each field to the current [`policy`] leaf tables and the
/// [`config`](super::config) defaults, so passing the default is byte-for-byte the
/// legacy behavior.
#[derive(Debug, Clone, Copy)]
pub struct EpisodePolicy<'a> {
    /// The most speaking voices a scene may carry before `cast-over-four` warns.
    pub scene_cast_ceiling: usize,
    /// The hook must land by this many seconds (`hook-late`).
    pub hook_seconds_ceiling: u32,
    /// At or above this, no single character slider reads as flat.
    pub slider_flat_ceiling: u8,
    /// Below this, a character slider needs an explicit compensation note.
    pub slider_low_floor: u8,
    /// Single-word combat vocabulary that signals a battle-climax habit in the
    /// *ten* (`ten-reads-as-battle`).
    pub battle_single: &'a [&'a str],
    /// Multi-token combat phrases (each an adjacent pair after tokenizing), with the
    /// alternative leading tokens the legacy regex allowed.
    pub battle_pairs: &'a [(&'a [&'a str], &'a str)],
    /// The per-finding-code citation + severity policy (design §3a).
    pub finding_policy: FindingPolicy<'a>,
    /// The narrator-framing dispositions (design §3b).
    pub narrator_policy: NarratorPolicy<'a>,
    /// The material-ledger provenance reference formats (design §3c).
    pub provenance: ProvenanceFormats<'a>,
    /// The material-planning catalog — its categories and required set (design §3d).
    pub material_catalog: MaterialCatalog<'a>,
    /// The arc-shape assumptions — which slot the turn scan reads (design §3e).
    pub arc_shapes: ArcShapeCatalog,
}

/// The default outline policy — every field wired to the current [`policy`] leaf
/// tables, so `check_episode(ep, opts, &DEFAULT_EPISODE_POLICY)` reproduces the
/// legacy `check_outline(ep)` exactly (design §2.3, Step 1: zero behavior change).
pub const DEFAULT_EPISODE_POLICY: EpisodePolicy<'static> = EpisodePolicy {
    scene_cast_ceiling: policy::SCENE_CAST_CEILING,
    hook_seconds_ceiling: policy::HOOK_SECONDS_CEILING,
    slider_flat_ceiling: policy::SLIDER_FLAT_CEILING,
    slider_low_floor: policy::SLIDER_LOW_FLOOR,
    battle_single: policy::BATTLE_SINGLE,
    battle_pairs: policy::BATTLE_PAIRS,
    finding_policy: DEFAULT_FINDING_POLICY,
    narrator_policy: DEFAULT_NARRATOR_POLICY,
    provenance: DEFAULT_PROVENANCE,
    material_catalog: DEFAULT_MATERIAL_CATALOG,
    arc_shapes: DEFAULT_ARC_SHAPES,
};

/// The optional review modes that turn on the packet-methodology and
/// material-worldbuilding checks. The defaults ([`CheckOptions::default`]) run the
/// always-on structural battery only, matching a bare `check_outline(ep)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckOptions {
    /// Require the packet-review methodology ledgers (sliders, scene-work, audio
    /// plan, season promises) — the legacy `require_methodology`.
    pub require_methodology: bool,
    /// Run the opt-in material-worldbuilding review — the legacy
    /// `material_worldbuilding`.
    pub material_worldbuilding: bool,
}

/// Run every deterministic structural check over one episode outline — the port
/// of `check_outline`.
///
/// Findings are returned in the legacy check order (promises, sliders, MICE,
/// scene-work, try/fail, scenes, audio, material, loops, ten, hook), so the
/// sequence — not merely the multiset of codes — matches the oracle.
///
/// ```
/// use dli_world::craft::outline::{EpisodeOutline, Scene, Advance};
/// use dli_world::craft::season::{check_episode, CheckOptions, DEFAULT_EPISODE_POLICY};
///
/// let ep = EpisodeOutline {
///     id: "e1".to_owned(),
///     title: "T".to_owned(),
///     scenes: vec![Scene {
///         id: "s1".to_owned(),
///         summary: "s".to_owned(),
///         speaking_cast: vec!["a".to_owned(), "b".to_owned(), "c".to_owned(),
///                             "d".to_owned(), "e".to_owned()],
///         advances: vec![Advance::Plot],
///         silence_beat: false,
///     }],
///     ..EpisodeOutline::default()
/// };
/// let codes: Vec<_> = check_episode(&ep, CheckOptions::default(), &DEFAULT_EPISODE_POLICY)
///     .into_iter().map(|f| f.code).collect();
/// assert!(codes.contains(&"outline.cast-over-four".to_owned()));
/// ```
#[must_use]
pub fn check_episode(
    ep: &EpisodeOutline,
    opts: CheckOptions,
    policy: &EpisodePolicy,
) -> Vec<WorldFinding> {
    let scene_ids: BTreeSet<&str> = ep.scene_ids().into_iter().collect();
    let mut out = Vec::new();
    check_promises(ep, &scene_ids, &mut out);
    check_character_sliders(ep, &scene_ids, opts, policy, &mut out);
    check_mice_nesting(ep, &mut out);
    check_scene_work(ep, &scene_ids, opts, &mut out);
    check_tryfail(ep, &mut out);
    check_scenes(ep, policy, &mut out);
    check_audio_plan(ep, opts, policy, &mut out);
    check_material_assumptions(ep, opts, policy, &mut out);
    check_loops(ep, &mut out);
    check_ten(ep, policy, &mut out);
    if ep.hook_by_seconds > policy.hook_seconds_ceiling {
        out.push(warn(
            "hook-late",
            &ep.id,
            format!(
                "hook lands at {}s; earn attention inside {}s (the audio hook)",
                ep.hook_by_seconds, policy.hook_seconds_ceiling
            ),
        ));
    }
    out
}

/// Check every episode, plus season-scope promise and loop accounting — the port
/// of `check_season`.
///
/// A loop closed in an episode must have been opened in that episode or an
/// earlier one (`loop-closed-unopened`, error); a loop opened and never closed by
/// season's end is reported (`loop-never-closed`, warn — it may be a deliberate
/// multi-season carry).
#[must_use]
pub fn check_season(
    season: &SeasonOutline,
    opts: CheckOptions,
    policy: &EpisodePolicy,
) -> Vec<WorldFinding> {
    let mut out = Vec::new();
    check_season_promises(season, opts, &mut out);

    let mut opened_so_far: BTreeSet<&str> = BTreeSet::new();
    let mut ever_closed: BTreeSet<&str> = BTreeSet::new();
    for ep in &season.episodes {
        out.extend(check_episode(ep, opts, policy));
        for loop_id in &ep.loops_closed {
            if !opened_so_far.contains(loop_id.as_str()) {
                out.push(error(
                    "loop-closed-unopened",
                    &ep.id,
                    format!(
                        "loop {loop_id:?} closes but was never opened in a prior \
                         or current episode"
                    ),
                ));
            }
        }
        for loop_id in &ep.loops_opened {
            opened_so_far.insert(loop_id.as_str());
        }
        for loop_id in &ep.loops_closed {
            ever_closed.insert(loop_id.as_str());
        }
    }
    let mut dangling: Vec<&str> = opened_so_far.difference(&ever_closed).copied().collect();
    dangling.sort_unstable();
    for loop_id in dangling {
        out.push(warn(
            "loop-never-closed",
            &season.id,
            format!(
                "loop {loop_id:?} opens but never closes this season (intended \
                 multi-season carry?)"
            ),
        ));
    }
    out
}

/// The most severe severity present, or `None` when the outline passes clean —
/// the port of `worst_severity` (`pass` becomes `None`).
///
/// `Error` outranks `Warn`, matching the legacy `SEVERITY_ORDER`. This is
/// computed explicitly rather than via [`Severity`]'s derived `Ord`, whose
/// declaration order ranks `Error` *below* `Warn`.
#[must_use]
pub fn worst_severity(findings: &[WorldFinding]) -> Option<Severity> {
    if findings.iter().any(|f| f.severity == Severity::Error) {
        Some(Severity::Error)
    } else if findings.is_empty() {
        None
    } else {
        Some(Severity::Warn)
    }
}

// -- the individual checks (private; each mirrors a legacy `_check_*`) ---------

fn check_promises(ep: &EpisodeOutline, scene_ids: &BTreeSet<&str>, out: &mut Vec<WorldFinding>) {
    for p in &ep.promises {
        // Ported from the pydantic `_payoff_or_carry` validator: a promise pays
        // off here or is carried, never both and never neither.
        if p.payoff_ref.is_some() == p.open_loop_carry {
            out.push(error(
                "promise-payoff-xor-carry",
                &p.id,
                "set exactly one of payoff_ref or open_loop_carry (a promise is \
                 paid off here or carried, not both)",
            ));
        }
        if !valid_beat(&p.made_in, scene_ids) {
            out.push(error(
                "promise-made-in-unknown",
                &p.id,
                format!(
                    "promise made_in {:?} is not a scene id or 'cold-open'",
                    p.made_in
                ),
            ));
        }
        if let Some(payoff) = &p.payoff_ref
            && !scene_ids.contains(payoff.as_str())
        {
            out.push(error(
                "dangling-payoff",
                &p.id,
                format!("promise pays off in {payoff:?}, which is not a scene"),
            ));
        }
    }
}

fn check_character_sliders(
    ep: &EpisodeOutline,
    scene_ids: &BTreeSet<&str>,
    opts: CheckOptions,
    policy: &EpisodePolicy,
    out: &mut Vec<WorldFinding>,
) {
    if opts.require_methodology && ep.character_sliders.is_empty() {
        out.push(warn(
            "no-character-slider-ledger",
            &ep.id,
            "packet review needs per-major-character proactivity, competence, \
             and sympathy scores",
        ));
    }

    let mut seen: BTreeSet<String> = BTreeSet::new();
    for slider in &ep.character_sliders {
        let key = slider.character.to_lowercase();
        if seen.contains(&key) {
            out.push(error(
                "character-slider-duplicate",
                &slider.character,
                "character slider ledger repeats this character",
            ));
        }
        seen.insert(key);
        for scene_ref in &slider.scene_refs {
            if !scene_ids.contains(scene_ref.as_str()) {
                out.push(error(
                    "character-slider-scene-unknown",
                    &slider.character,
                    format!("slider scene_ref {scene_ref:?} is not a scene in the episode"),
                ));
            }
        }
        let high = slider
            .proactivity
            .max(slider.competence)
            .max(slider.sympathy);
        let low = slider
            .proactivity
            .min(slider.competence)
            .min(slider.sympathy);
        if high < policy.slider_flat_ceiling {
            out.push(warn(
                "character-slider-flat",
                &slider.character,
                "all three sliders are below 5; the character may read flat",
            ));
        }
        if low < policy.slider_low_floor && slider.compensation.trim().is_empty() {
            out.push(warn(
                "slider-compensation-missing",
                &slider.character,
                "a low slider needs an explicit compensation note naming which \
                 other dial carries the audience's investment",
            ));
        }
    }

    let slider_chars: BTreeSet<String> = ep
        .character_sliders
        .iter()
        .map(|s| s.character.to_lowercase())
        .collect();
    for character in &ep.major_characters {
        if !slider_chars.contains(&character.to_lowercase()) {
            out.push(warn(
                "major-character-slider-missing",
                character,
                "major character is listed without a proactivity/competence/\
                 sympathy ledger row",
            ));
        }
    }
}

fn check_mice_nesting(ep: &EpisodeOutline, out: &mut Vec<WorldFinding>) {
    let order: HashMap<&str, usize> = ep
        .scene_ids()
        .into_iter()
        .enumerate()
        .map(|(i, sid)| (sid, i))
        .collect();
    let mut spans: Vec<(&str, usize, usize)> = Vec::new();
    for t in &ep.mice_threads {
        let (Some(&lo), Some(&hi)) = (
            order.get(t.open_beat.as_str()),
            order.get(t.close_beat.as_str()),
        ) else {
            out.push(error(
                "mice-beat-unknown",
                &t.id,
                format!("thread {:?} references a scene not in the episode", t.id),
            ));
            continue;
        };
        if hi < lo {
            out.push(error(
                "mice-closes-before-open",
                &t.id,
                format!("thread {:?} closes before it opens", t.id),
            ));
            continue;
        }
        spans.push((&t.id, lo, hi));
    }
    for i in 0..spans.len() {
        let (a_id, a_lo, a_hi) = spans[i];
        for &(b_id, b_lo, b_hi) in &spans[i + 1..] {
            let disjoint = a_hi < b_lo || b_hi < a_lo;
            let a_in_b = b_lo <= a_lo && a_hi <= b_hi;
            let b_in_a = a_lo <= b_lo && b_hi <= a_hi;
            if !(disjoint || a_in_b || b_in_a) {
                out.push(error(
                    "mice-interleaved",
                    &format!("{a_id}+{b_id}"),
                    format!(
                        "threads {a_id:?} and {b_id:?} interleave; MICE nests LIFO \
                         (a thread opened later must close earlier)"
                    ),
                ));
            }
        }
    }
}

fn check_scene_work(
    ep: &EpisodeOutline,
    scene_ids: &BTreeSet<&str>,
    opts: CheckOptions,
    out: &mut Vec<WorldFinding>,
) {
    if opts.require_methodology && ep.scene_work.is_empty() {
        out.push(warn(
            "no-scene-work-ledger",
            &ep.id,
            "packet review needs a scene goal/conflict/disaster ledger",
        ));
    }

    let order: HashMap<&str, usize> = ep
        .scene_ids()
        .into_iter()
        .enumerate()
        .map(|(i, sid)| (sid, i))
        .collect();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut ordered_work: Vec<&SceneWork> = Vec::new();
    for work in &ep.scene_work {
        if seen.contains(work.scene_id.as_str()) {
            out.push(error(
                "scene-work-duplicate",
                &work.scene_id,
                "scene_work repeats this scene",
            ));
        }
        seen.insert(work.scene_id.as_str());
        if !scene_ids.contains(work.scene_id.as_str()) {
            out.push(error(
                "scene-work-unknown-scene",
                &work.scene_id,
                "scene_work references a scene not in the episode",
            ));
            continue;
        }
        ordered_work.push(work);
    }

    if opts.require_methodology {
        for scene_id in ep.scene_ids() {
            if !seen.contains(scene_id) {
                out.push(warn(
                    "scene-work-missing",
                    scene_id,
                    "scene is missing explicit goal/conflict/disaster work",
                ));
            }
        }
    }

    ordered_work.sort_by_key(|work| order.get(work.scene_id.as_str()).copied().unwrap_or(10_000));
    let last = ordered_work.len().saturating_sub(1);
    for (i, work) in ordered_work.iter().enumerate() {
        if work.outcome == TryFailOutcome::YesAnd && i != last {
            out.push(warn(
                "scene-work-premature-resolution",
                &work.scene_id,
                "a scene-level 'yes-and' before the final worked scene resolves \
                 the chain early",
            ));
        }
    }
    if let Some(tail) = ordered_work.last()
        && !ep.season_boundary
        && matches!(tail.outcome, TryFailOutcome::YesBut | TryFailOutcome::NoAnd)
    {
        out.push(warn(
            "scene-work-chain-unsettled",
            &ep.id,
            "the scene-work chain ends on an unsettled yes-but/no-and; make the \
             episode's payment, carry, or deliberate cliff clear",
        ));
    }
}

fn check_tryfail(ep: &EpisodeOutline, out: &mut Vec<WorldFinding>) {
    if !ep.scenes.is_empty() && ep.tryfail_beats.is_empty() && !ep.season_boundary {
        out.push(warn(
            "no-tryfail-cycle",
            &ep.id,
            "no try/fail beats; the middle has no yes-but/no-and engine",
        ));
    }
    let last = ep.tryfail_beats.len().saturating_sub(1);
    for (i, beat) in ep.tryfail_beats.iter().enumerate() {
        if beat.outcome == TryFailOutcome::YesAnd && i != last {
            out.push(warn(
                "premature-resolution",
                &beat.id,
                "a 'yes-and' before the final beat resolves the thread early",
            ));
        }
    }
}

fn check_scenes(ep: &EpisodeOutline, policy: &EpisodePolicy, out: &mut Vec<WorldFinding>) {
    for s in &ep.scenes {
        if s.speaking_cast.len() > policy.scene_cast_ceiling {
            out.push(warn(
                "cast-over-four",
                &s.id,
                format!(
                    "{} speaking voices; keep a scene to ≤{} (Toscan) so the ear \
                     tracks who is who",
                    s.speaking_cast.len(),
                    policy.scene_cast_ceiling
                ),
            ));
        }
        if s.advances.is_empty() {
            out.push(warn(
                "scene-idles",
                &s.id,
                "scene advances nothing; every scene must move plot, a \
                 relationship, or a revelation (micro-pacing)",
            ));
        }
    }
}

fn check_audio_plan(
    ep: &EpisodeOutline,
    opts: CheckOptions,
    policy: &EpisodePolicy,
    out: &mut Vec<WorldFinding>,
) {
    let fp = &policy.finding_policy;
    let Some(plan) = &ep.audio_plan else {
        if opts.require_methodology {
            let cite = fp.cite_token("outline.no-audio-plan");
            push_cited(
                out,
                fp,
                warn(
                    "no-audio-plan",
                    &ep.id,
                    format!(
                        "packet review needs narrator/SFX load accounting for {cite} \
                         audio mode"
                    ),
                ),
            );
        }
        return;
    };

    if plan.direct_address {
        let cite = fp.cite_token("outline.narrator-direct-address");
        push_disposition(
            out,
            fp,
            policy.narrator_policy.direct_address,
            "narrator-direct-address",
            &ep.id,
            format!("narrator plan allows direct audience address; {cite} forbids it"),
        );
    }
    if plan.narrator_mode == NarratorMode::Host {
        let cite = fp.cite_token("outline.narrator-host-frame");
        push_disposition(
            out,
            fp,
            policy.narrator_policy.mode(NarratorMode::Host),
            "narrator-host-frame",
            &ep.id,
            format!("narrator plan uses host framing; {cite} forbids tale-host mode"),
        );
    }
    if plan.narrator_mode == NarratorMode::None && plan.orientation_load == AudioLoad::High {
        out.push(warn(
            "narrator-needed-for-orientation",
            &ep.id,
            "high orientation load with no narrator repeats the failed \
             no-narrator packet path",
        ));
    }
    if plan.narrator_mode == NarratorMode::None
        && (matches!(plan.sfx_load, SfxLoad::Moderate | SfxLoad::Heavy)
            || plan.sfx_semantic_load == SfxSemanticLoad::LoadBearing)
    {
        out.push(warn(
            "no-narrator-sfx-burden",
            &ep.id,
            "without narrator support, SFX is carrying semantic load instead \
             of sparse support",
        ));
    }
    if matches!(plan.sfx_load, SfxLoad::Moderate | SfxLoad::Heavy) {
        let cite = fp.cite_token("outline.sfx-not-sparse");
        push_cited(
            out,
            fp,
            warn(
                "sfx-not-sparse",
                &ep.id,
                format!(
                    "SFX load is {}; {cite} wants sparse support, not a major \
                     semantic channel",
                    sfx_load_label(plan.sfx_load)
                ),
            ),
        );
    }
    if plan.sfx_semantic_load == SfxSemanticLoad::LoadBearing {
        out.push(warn(
            "sfx-load-bearing",
            &ep.id,
            "SFX is marked load-bearing; use windowpane narration for causal \
             orientation and let foley support it",
        ));
    }
    if plan.narrator_mode == NarratorMode::Windowpane
        && matches!(plan.orientation_load, AudioLoad::Medium | AudioLoad::High)
        && plan.narrator_tasks.is_empty()
    {
        out.push(warn(
            "narrator-tasks-missing",
            &ep.id,
            "windowpane narrator is present but no load-bearing tasks are declared",
        ));
    }
}

fn check_material_assumptions(
    ep: &EpisodeOutline,
    opts: CheckOptions,
    policy: &EpisodePolicy,
    out: &mut Vec<WorldFinding>,
) {
    let Some(ledger) = &ep.material_assumptions else {
        if opts.material_worldbuilding {
            out.push(warn(
                "no-material-assumption-ledger",
                &ep.id,
                "material-worldbuilding review needs an explicit assumption ledger; \
                 character-only outlines may omit it outside this opt-in mode",
            ));
        }
        return;
    };

    check_material_provenance(ep, ledger, policy, out);

    let unknowns: BTreeSet<MaterialUnknown> = ledger.unknowns.iter().copied().collect();
    if opts.material_worldbuilding {
        for category in policy.material_catalog.required() {
            if ledger.field(category).is_none() && !unknowns.contains(&category) {
                out.push(warn(
                    "material-assumption-incomplete",
                    &format!("{}.{}", ep.id, category.field_name()),
                    "supply a planning answer or list this category in unknowns",
                ));
            }
        }
    }

    let asserted = ledger.asserted_fields();
    if !asserted.is_empty() && ledger.canon_refs.is_empty() && ledger.research_refs.is_empty() {
        out.push(warn(
            "unsupported-material-assumption",
            &ep.id,
            "working material assertions have no canon_refs or research_refs; \
             references support planning provenance, not in-world truth",
        ));
    }
}

/// The material-ledger provenance guards ported from the pydantic field/model
/// validators (no legacy `Violation` code): silent on well-formed ledgers, a
/// fail-closed finding on malformed ones.
fn check_material_provenance(
    ep: &EpisodeOutline,
    ledger: &MaterialAssumptionLedger,
    policy: &EpisodePolicy,
    out: &mut Vec<WorldFinding>,
) {
    // `_assumption_is_nonblank`: a supplied assumption must not be blank.
    for category in policy.material_catalog.all() {
        if ledger.field(category).is_some_and(|v| v.trim().is_empty()) {
            out.push(error(
                "material-assumption-blank",
                &format!("{}.{}", ep.id, category.field_name()),
                "material assumptions must be non-empty when supplied",
            ));
        }
    }
    // `_answers_are_not_also_unknown`: a category cannot have both an answer and
    // unknown status.
    for category in &ledger.unknowns {
        if ledger.field(*category).is_some() {
            out.push(error(
                "material-answer-and-unknown",
                &format!("{}.{}", ep.id, category.field_name()),
                "material category cannot have both an answer and unknown status",
            ));
        }
    }
    // `_canon_refs_are_decisions`: unique `D-nnnn` decision ids.
    report_duplicates(
        &ledger.canon_refs,
        "material-canon-ref-duplicate",
        &ep.id,
        out,
    );
    for r in &ledger.canon_refs {
        if !policy.provenance.is_canon_ref(r) {
            out.push(error(
                "material-canon-ref-malformed",
                &ep.id,
                format!("canon_ref {r:?} must use an authorial decision id (D-nnnn)"),
            ));
        }
    }
    // `_research_refs_are_labelled`: unique `RR-nnn` / research-doc path / URL.
    report_duplicates(
        &ledger.research_refs,
        "material-research-ref-duplicate",
        &ep.id,
        out,
    );
    for r in &ledger.research_refs {
        if !policy.provenance.is_research_ref(r) {
            out.push(error(
                "material-research-ref-malformed",
                &ep.id,
                format!(
                    "research_ref {r:?} must use RR-nnn, a docs/research/RR-nnn*.md \
                     path, or an http(s) URL"
                ),
            ));
        }
    }
}

fn check_loops(ep: &EpisodeOutline, out: &mut Vec<WorldFinding>) {
    if ep.loops_opened.is_empty() {
        out.push(warn(
            "no-loop-opened",
            &ep.id,
            "episode opens no psychological loop; give the listener a reason \
             to lean into the next installment (open-loop discipline)",
        ));
    }
    if ep.loops_closed.is_empty() && !ep.season_boundary {
        out.push(warn(
            "no-loop-closed",
            &ep.id,
            "episode closes no loop; each installment should pay something off \
             (unless it sits on a season boundary)",
        ));
    }
}

fn check_ten(ep: &EpisodeOutline, policy: &EpisodePolicy, out: &mut Vec<WorldFinding>) {
    let turn = policy.arc_shapes.turn_slot.text(ep);
    if !turn.is_empty() && reads_as_battle(turn, policy) {
        push_cited(
            out,
            &policy.finding_policy,
            warn(
                "ten-reads-as-battle",
                &ep.id,
                "the ten reads as a battle/confrontation; the turn should be a \
                 perspective shift dissolved through empathy and logic, not a clash",
            ),
        );
    }
}

fn check_season_promises(season: &SeasonOutline, opts: CheckOptions, out: &mut Vec<WorldFinding>) {
    if opts.require_methodology && season.promises.is_empty() {
        out.push(warn(
            "no-season-promise-ledger",
            &season.id,
            "packet review needs an arc/season promise ledger across episodes",
        ));
    }

    let refs = season_refs(season);
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for promise in &season.promises {
        // Ported from the arc-promise `_payoff_or_carry` validator.
        if promise.payoff_ref.is_some() == promise.carry_scope.is_some() {
            out.push(error(
                "arc-promise-payoff-xor-carry",
                &promise.id,
                "set exactly one of payoff_ref or carry_scope",
            ));
        }
        if seen.contains(promise.id.as_str()) {
            out.push(error(
                "season-promise-duplicate",
                &promise.id,
                "season promise ledger repeats this id",
            ));
        }
        seen.insert(promise.id.as_str());
        if !valid_season_ref(&promise.opened_in, &refs) {
            out.push(error(
                "season-promise-open-unknown",
                &promise.id,
                format!(
                    "opened_in {:?} is not an episode, scene, 'cold-open', or \
                     'arc-open'",
                    promise.opened_in
                ),
            ));
        }
        if let Some(payoff) = &promise.payoff_ref
            && !valid_season_ref(payoff, &refs)
        {
            out.push(error(
                "season-promise-payoff-unknown",
                &promise.id,
                format!(
                    "payoff_ref {payoff:?} is not an episode, scene, \
                     'cold-open', or 'arc-open'"
                ),
            ));
        }
    }
}

// -- helpers ------------------------------------------------------------------

fn sfx_load_label(load: SfxLoad) -> &'static str {
    match load {
        SfxLoad::None => "none",
        SfxLoad::Sparse => "sparse",
        SfxLoad::Moderate => "moderate",
        SfxLoad::Heavy => "heavy",
    }
}

fn valid_beat(reference: &str, scene_ids: &BTreeSet<&str>) -> bool {
    reference == "cold-open" || scene_ids.contains(reference)
}

fn valid_season_ref(reference: &str, refs: &BTreeSet<String>) -> bool {
    refs.contains(reference) || reference == "cold-open" || reference == "arc-open"
}

fn season_refs(season: &SeasonOutline) -> BTreeSet<String> {
    let mut refs: BTreeSet<String> = season.episodes.iter().map(|ep| ep.id.clone()).collect();
    for ep in &season.episodes {
        for scene in &ep.scenes {
            refs.insert(scene.id.clone());
        }
    }
    refs
}

fn report_duplicates(items: &[String], code: &str, where_: &str, out: &mut Vec<WorldFinding>) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for item in items {
        if !seen.insert(item.as_str()) {
            out.push(error(code, where_, format!("duplicate reference {item:?}")));
        }
    }
}

fn reads_as_battle(ten: &str, policy: &EpisodePolicy) -> bool {
    let tokens: Vec<String> = ten
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect();
    for token in &tokens {
        if policy.battle_single.contains(&token.as_str()) {
            return true;
        }
    }
    for pair in tokens.windows(2) {
        for (leads, tail) in policy.battle_pairs {
            if leads.contains(&pair[0].as_str()) && pair[1] == *tail {
                return true;
            }
        }
    }
    false
}

fn warn(code: &str, where_: &str, message: impl Into<String>) -> WorldFinding {
    WorldFinding::warn(format!("outline.{code}"), with_where(where_, message))
}

fn error(code: &str, where_: &str, message: impl Into<String>) -> WorldFinding {
    WorldFinding::error(format!("outline.{code}"), with_where(where_, message))
}

fn with_where(where_: &str, message: impl Into<String>) -> String {
    format!("{where_}: {}", message.into())
}

/// Attach a finding's policy citations (and any severity override) and push it —
/// the drop-in for the inline `.with_citations(vec![…])` calls, keyed by the
/// finding's own code (design §3a). Called only where the legacy checker attached
/// a citation, so codes that never carried one stay uncited.
fn push_cited(out: &mut Vec<WorldFinding>, fp: &FindingPolicy, mut finding: WorldFinding) {
    let cites = fp.citations(&finding.code);
    if !cites.is_empty() {
        finding = finding.with_citations(cites);
    }
    if let Some(severity) = fp.severity_override(&finding.code) {
        finding.severity = severity;
    }
    out.push(finding);
}

/// Emit a narrator-framing finding per its [`Disposition`], then attach its policy
/// citation (design §3b): an [`Error`](Disposition::Error) forbids, a
/// [`Warn`](Disposition::Warn) discourages, an [`Allowed`](Disposition::Allowed)
/// emits nothing.
fn push_disposition(
    out: &mut Vec<WorldFinding>,
    fp: &FindingPolicy,
    disposition: Disposition,
    code: &str,
    where_: &str,
    message: String,
) {
    let finding = match disposition {
        Disposition::Error => error(code, where_, message),
        Disposition::Warn => warn(code, where_, message),
        Disposition::Allowed => return,
    };
    push_cited(out, fp, finding);
}

// =============================================================================
// CRAFT-2 — the season-ledger craft-rule engine.
//
// The `CRAFT-1` battery above sees one outline at a time. The merge
// ([06-craft-and-payloads](../../../../docs/spec/06-craft-and-payloads.md) §1)
// makes beats, sliders, promises, and reveals **season-wide ledger rows**, so a
// rule may *join across episodes* — the move that makes these checks possible at
// all. [`check_season_rules`] runs the join battery over a [`CraftLedger`],
// producing `craft.<rule>`-coded [`WorldFinding`]s that share the [`KernelRule`]
// namespace with the knowledge base ([`super::kb`]), so every finding's governing
// principle is recovered by `kb::why(finding.code)`.
//
// The ledger's beats reuse the payload schema's [`BeatKind`] and [`Magnitude`]
// ([`crate::payload`]) — a season-wide [`LedgerBeat`] is a payload
// [`BeatRow`](crate::payload::BeatRow) tagged with its production episode, which is
// exactly what `PAY-2`'s merge produces. The beat's try/fail typing rides in
// [`LedgerBeat::outcome`] (`Option<TryFailOutcome>`, filled by `PAY-2` from the
// payload's outcome field); the earned-outcomes join reads spec §3's failure set
// {yes-but, no-but, no-and} from it, and falls back to the coarse `fail` `kind`
// when a payload carried no finer outcome, so the rule works at either fidelity.
//
// The evaluator is the same linear-scan shape as the story-time order-rules
// (`dli_time::order_rules::check`): each rule is one or two passes over the ledger
// rows, findings out in rule order. **Thresholds are policy data** (spec §4): every
// numeric band lives in [`Thresholds`] as a D-citable leaf table and rides at
// [`Severity::Warn`] until calibrated. The two structural verdicts the spec fixes —
// the accomplishment-earned join (error) and the scene-name clash (error) — keep
// their declared severity; they turn on the *shape* of the ledger, not a tunable
// number.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::kb::KernelRule;
use super::outline::PromiseKind;

// The season ledger's beats speak the payload schema's vocabulary — a
// `LedgerBeat` is a `BeatRow` tagged with its episode. Re-exported so the craft
// API is self-contained.
pub use crate::payload::{BeatKind, Magnitude};

/// One beat on the season ledger — a payload [`BeatRow`](crate::payload::BeatRow)
/// tagged with the production episode it was merged from. The merge appends these
/// in **production order**; [`CraftLedger::episodes`] carries the slot each
/// `episode` sits at, so the join orders correctly across episodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LedgerBeat {
    /// The beat identifier.
    pub id: String,
    /// The production-order episode this beat sits in (an [`EpisodeMeta::id`]).
    pub episode: String,
    /// The scene the beat plays in.
    pub scene: String,
    /// Whether it is a try, a fail, or an accomplishment.
    pub kind: BeatKind,
    /// The thread the beat advances (season-wide identity).
    pub thread: String,
    /// The beat's magnitude.
    pub magnitude: Magnitude,
    /// The try/fail typing, when the extractor carried it. `PAY-2` fills it from
    /// the payload's `outcome: Option<TryFailOutcome>` (spec §3's outcome set);
    /// `None` degrades to reading [`kind`](Self::kind), so the join still works on
    /// a payload that only records the coarse `try`/`fail`/`accomplishment` signal.
    #[serde(default)]
    pub outcome: Option<TryFailOutcome>,
    /// Who acts (empty when unattributed) — for competence-consistency.
    #[serde(default)]
    pub by: String,
    /// Whether the accomplishment was assisted (aid, for competence-consistency).
    #[serde(default)]
    pub assisted: bool,
}

/// A character's competence/proactivity/sympathy as a **state on the production
/// axis** — one reading per episode (spec §3). The season checker reads these as
/// fluents, unlike the per-outline slider ledger of `CRAFT-1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SliderState {
    /// The character.
    pub character: String,
    /// The episode this reading is as-of.
    pub episode: String,
    /// How much the character drives events.
    pub proactivity: u8,
    /// How capable the character is.
    pub competence: u8,
    /// How much the audience is drawn to the character.
    pub sympathy: u8,
}

/// A promise as a season-ledger row: where it opens, the episodes that touch
/// (advance) it, and where it closes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LedgerPromise {
    /// The promise identifier.
    pub id: String,
    /// The kind of promise (mystery promises are the ones that can starve).
    pub kind: PromiseKind,
    /// The episode where the promise opens.
    pub opened_in: String,
    /// The episodes that advance the promise after it opens.
    #[serde(default)]
    pub touched_in: Vec<String>,
    /// The episode where it pays off, if it does.
    #[serde(default)]
    pub closed_in: Option<String>,
    /// Whether a finale-episode opening of this promise is a **declared carry**
    /// (a finale may open only declared carries — promise-density).
    #[serde(default)]
    pub declared_carry: bool,
}

/// A scene as a season-ledger row: its speaking cast plus the marketability flags
/// the clarity family reads (cold-open discipline, aural distinctness).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LedgerScene {
    /// The scene identifier.
    pub id: String,
    /// The episode the scene sits in.
    pub episode: String,
    /// The speaking cast (joined with the confusability table for name clashes).
    #[serde(default)]
    pub cast: Vec<String>,
    /// Whether the scene is the episode's cold open.
    #[serde(default)]
    pub cold_open: bool,
    /// Whether a cold open opens a psychological loop (required of a cold open).
    #[serde(default)]
    pub opens_loop: bool,
    /// The narrative act the scene sits in, when known.
    #[serde(default)]
    pub act: Option<u32>,
    /// Whether the scene reaches into deep time (forbidden in a cold open's act 1).
    #[serde(default)]
    pub deep_time: bool,
}

/// A point at which the listener learns a fact — a row on the **listener ledger**
/// (the distinguished knower on the production axis, spec §3), what makes reveal
/// fairness checkable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reveal {
    /// The fact the listener learns.
    pub fact: String,
    /// The episode at which the listener learns it.
    pub episode: String,
}

/// A kishōtenketsu turn (the *ten*): the episode it lands in, the fact it reframes
/// the whole around, and whether it resolves a conflict (which a *ten* must not —
/// the turn is a perspective shift, not a clash).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Turn {
    /// The episode the turn lands in.
    pub episode: String,
    /// The fact the turn reframes the arc around (must be on the listener ledger
    /// early — reveal fair-play).
    pub reframing_fact: String,
    /// Whether the turn is a conflict-resolution beat (a *ten* must not be).
    #[serde(default)]
    pub resolves_conflict: bool,
}

/// The declared arc shape of an episode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ArcShape {
    /// Kishōtenketsu — the four-beat turn (the mission's default).
    Kishotenketsu,
    /// A three-act shape.
    ThreeAct,
    /// Any other declared shape.
    Other,
}

/// Per-episode metadata on the production axis: the slot the listener meets it at
/// and the marketability/clarity fields the season rules read.
///
/// Carries an `f64` (`ten_position`), so it is `PartialEq` but not `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EpisodeMeta {
    /// The episode identifier (referenced by beats, sliders, scenes).
    pub id: String,
    /// The production slot — where the listener meets the episode (order key).
    pub slot: u32,
    /// The declared arc shape, when the episode declares one.
    #[serde(default)]
    pub arc_shape: Option<ArcShape>,
    /// Where the *ten* lands, as a fraction of the episode `0.0..=1.0`
    /// (arc-shape band conformance).
    #[serde(default)]
    pub ten_position: Option<f64>,
    /// The orientation load the episode places on the listener (audio-load run).
    #[serde(default)]
    pub audio_load: Option<AudioLoad>,
    /// The new named speakers introduced this episode (cast-introduction rate).
    #[serde(default)]
    pub introduced_names: Vec<String>,
}

/// The season-wide craft ledger the join rules run over — the merged first-class
/// objects (spec §1). The payload pipeline (`PAY-2`) merges each draft's
/// beats/reveals/etc. into one of these and calls [`check_season_rules`]; every
/// field is public so it is built directly, like a `dli_time::SlotView`.
///
/// Named `CraftLedger` to stand apart from the world model's
/// [`SeasonLedger`](crate::model::SeasonLedger), which is the loader's seasonal
/// cycle + practices — a different object entirely.
///
/// Carries `EpisodeMeta` (which holds an `f64`), so it is `PartialEq` but not `Eq`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct CraftLedger {
    /// The episodes, carrying their production slots and marketability fields.
    pub episodes: Vec<EpisodeMeta>,
    /// The beats, in production order.
    pub beats: Vec<LedgerBeat>,
    /// The per-episode slider readings.
    pub sliders: Vec<SliderState>,
    /// The promise ledger.
    pub promises: Vec<LedgerPromise>,
    /// The scenes.
    pub scenes: Vec<LedgerScene>,
    /// The listener-ledger reveals.
    pub reveals: Vec<Reveal>,
    /// The kishōtenketsu turns.
    pub turns: Vec<Turn>,
    /// Name pairs the naming oracle scored **below the aural-distinctness floor**
    /// ([05-language-layer](../../../../docs/spec/05-language-layer.md)). The global
    /// distinctness check stays the oracle's; the scene-scoped join is the kernel's
    /// (it holds both the confusability data and the scene casts). Stored
    /// normalized (lexicographically ordered), matched order-insensitively.
    pub confusable_pairs: BTreeSet<(String, String)>,
}

/// The calibratable policy thresholds the season rules read (spec §4) — every
/// numeric band, one place, D-citable. Substituting a table is a data diff citing
/// a ruling, never a code change. [`DEFAULT_THRESHOLDS`] ships uncalibrated, so the
/// band rules ride at [`Severity::Warn`].
///
/// Carries `f64` bands, so it is `PartialEq` but not `Eq`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    /// A `>= major` accomplishment needs at least this many prior same-thread
    /// failures (an error below it — the worked join).
    pub major_prior_failures: usize,
    /// A climax-class accomplishment needs at least this many (a warn below it).
    pub climax_prior_failures: usize,
    /// Below this competence, a major unassisted win needs an intervening growth
    /// or price beat.
    pub competence_floor: u8,
    /// A turn's reframing fact must sit on the listener ledger at least this many
    /// episodes before the turn.
    pub reveal_lead_episodes: u32,
    /// An open mystery untouched for this many consecutive episodes is starved.
    pub mystery_starved_gap: usize,
    /// A cold open may carry at most this many speaking voices.
    pub cold_open_cast_ceiling: usize,
    /// New named speakers per episode should sit within this inclusive band.
    pub cast_introduction_band: (usize, usize),
    /// The season named-cast budget, when set (total introductions ceiling).
    pub season_cast_budget: Option<usize>,
    /// A kishōtenketsu *ten* should land within this inclusive fraction band.
    pub ten_position_band: (f64, f64),
    /// No run of this many consecutive high-orientation-load episodes.
    pub audio_high_run: usize,
    /// Whether these thresholds are calibrated against the season-1 corpus. Ships
    /// `false`; band findings ride at warn until it is `true` (spec §4).
    pub calibrated: bool,
}

/// The default policy tables — uncalibrated, warn-severity bands (spec §4).
///
/// The numbers are the consuming repo's working values; each is owned by a ruling
/// when calibrated. Until then this is the honest starting point, not canon law.
pub const DEFAULT_THRESHOLDS: Thresholds = Thresholds {
    major_prior_failures: 1,
    climax_prior_failures: 2,
    competence_floor: 5,
    reveal_lead_episodes: 1,
    mystery_starved_gap: 3,
    cold_open_cast_ceiling: 2,
    cast_introduction_band: (1, 3),
    season_cast_budget: None,
    ten_position_band: (0.55, 0.80),
    audio_high_run: 3,
    calibrated: false,
};

impl Default for Thresholds {
    fn default() -> Self {
        DEFAULT_THRESHOLDS
    }
}

/// Run the season-ledger join battery over `ledger`, reading its numeric bands
/// from `thresholds`. Findings come out in rule order: accomplishment-earned,
/// yes-and-terminal, competence, reveal fair-play, mystery-starved, then the
/// marketability/clarity family. Each finding is `craft.<rule>`-coded so its
/// governing principle is recovered by [`super::kb::why`].
///
/// ```
/// use dli_world::craft::season::{
///     check_season_rules, BeatKind, CraftLedger, EpisodeMeta, LedgerBeat, Magnitude,
///     DEFAULT_THRESHOLDS,
/// };
///
/// // A major win on a thread that never failed — unearned.
/// let ledger = CraftLedger {
///     episodes: vec![EpisodeMeta { id: "e1".into(), slot: 1, arc_shape: None,
///         ten_position: None, audio_load: None, introduced_names: vec![] }],
///     beats: vec![LedgerBeat {
///         id: "b1".into(), episode: "e1".into(), scene: "s1".into(),
///         kind: BeatKind::Accomplishment, thread: "ward".into(),
///         magnitude: Magnitude::Major, outcome: None, by: String::new(), assisted: false,
///     }],
///     ..CraftLedger::default()
/// };
/// let codes: Vec<_> = check_season_rules(&ledger, &DEFAULT_THRESHOLDS)
///     .into_iter().map(|f| f.code).collect();
/// assert!(codes.contains(&"craft.accomplishment-earned".to_owned()));
/// ```
#[must_use]
pub fn check_season_rules(ledger: &CraftLedger, thresholds: &Thresholds) -> Vec<WorldFinding> {
    let mut out = Vec::new();
    let order = ProductionOrder::new(ledger);
    check_accomplishment_earned(ledger, thresholds, &order, &mut out);
    check_yes_and_terminal(ledger, &order, &mut out);
    check_competence(ledger, thresholds, &order, &mut out);
    check_reveal_fair_play(ledger, thresholds, &episode_slots(ledger), &mut out);
    check_mystery_starved(ledger, thresholds, &mut out);
    check_cold_open_discipline(ledger, thresholds, &mut out);
    check_cast_introduction(ledger, thresholds, &mut out);
    check_scene_name_clash(ledger, &mut out);
    check_promise_density(ledger, &mut out);
    check_arc_shape(ledger, thresholds, &mut out);
    check_audio_load(ledger, thresholds, &mut out);
    out
}

// -- production ordering ------------------------------------------------------

/// The production-order key of each beat: `(episode slot, position in the beat
/// vector)`. An episode with no declared slot sorts last (a large sentinel), so a
/// malformed ledger degrades to ledger order rather than panicking.
struct ProductionOrder {
    /// Per-beat ordinal, parallel to `ledger.beats`.
    ordinals: Vec<(u32, usize)>,
}

impl ProductionOrder {
    fn new(ledger: &CraftLedger) -> Self {
        let slots = episode_slots(ledger);
        let ordinals = ledger
            .beats
            .iter()
            .enumerate()
            .map(|(i, b)| {
                (
                    slots.get(b.episode.as_str()).copied().unwrap_or(u32::MAX),
                    i,
                )
            })
            .collect();
        Self { ordinals }
    }

    /// Whether beat `f` is produced strictly before beat `b` (both indices into
    /// `ledger.beats`).
    fn before(&self, f: usize, b: usize) -> bool {
        self.ordinals[f] < self.ordinals[b]
    }
}

/// The production slot of each episode id.
fn episode_slots(ledger: &CraftLedger) -> HashMap<&str, u32> {
    ledger
        .episodes
        .iter()
        .map(|e| (e.id.as_str(), e.slot))
        .collect()
}

/// The magnitude ladder as a rank (`payload::Magnitude` derives no `Ord`).
fn mag_rank(m: Magnitude) -> u8 {
    match m {
        Magnitude::Minor => 0,
        Magnitude::Major => 1,
        Magnitude::Climax => 2,
    }
}

// -- the join rules -----------------------------------------------------------

/// Accomplishment-earned: a `>= major` accomplishment requires a prior same-thread
/// failure (error if absent); a climax needs two (warn if only one).
fn check_accomplishment_earned(
    ledger: &CraftLedger,
    thresholds: &Thresholds,
    order: &ProductionOrder,
    out: &mut Vec<WorldFinding>,
) {
    for (bi, beat) in ledger.beats.iter().enumerate() {
        if beat.kind != BeatKind::Accomplishment
            || mag_rank(beat.magnitude) < mag_rank(Magnitude::Major)
        {
            continue;
        }
        let priors = ledger
            .beats
            .iter()
            .enumerate()
            .filter(|(fi, f)| f.thread == beat.thread && is_cost(f) && order.before(*fi, bi))
            .count();
        if priors < thresholds.major_prior_failures {
            push_cited(
                out,
                &DEFAULT_FINDING_POLICY,
                craft_error(
                    KernelRule::AccomplishmentEarned,
                    &beat.id,
                    format!(
                        "a {} win on thread {:?} lands with no prior failure on that thread; a \
                         triumph is earned by a cost paid first",
                        magnitude_label(beat.magnitude),
                        beat.thread
                    ),
                ),
            );
        } else if beat.magnitude == Magnitude::Climax && priors < thresholds.climax_prior_failures {
            out.push(craft_warn(
                KernelRule::AccomplishmentEarnedClimax,
                &beat.id,
                format!(
                    "a climax-class win on thread {:?} rests on only {priors} prior failure; a \
                     climax should be earned by at least {}",
                    beat.thread, thresholds.climax_prior_failures
                ),
            ));
        }
    }
}

/// Yes-and is terminal per thread across the season (spec §3): once a thread lands
/// its unqualified win — a `yes-and` outcome, or a climax accomplishment when the
/// payload carried no finer outcome — a later beat re-opening it is an error.
fn check_yes_and_terminal(
    ledger: &CraftLedger,
    order: &ProductionOrder,
    out: &mut Vec<WorldFinding>,
) {
    // Earliest ordinal of an unqualified win, per thread.
    let mut terminal: HashMap<&str, (u32, usize)> = HashMap::new();
    for (i, beat) in ledger.beats.iter().enumerate() {
        if is_terminal_win(beat) {
            let ord = order.ordinals[i];
            terminal
                .entry(beat.thread.as_str())
                .and_modify(|cur| {
                    if ord < *cur {
                        *cur = ord;
                    }
                })
                .or_insert(ord);
        }
    }
    for (i, beat) in ledger.beats.iter().enumerate() {
        if let Some(win) = terminal.get(beat.thread.as_str())
            && order.ordinals[i] > *win
        {
            out.push(craft_error(
                KernelRule::YesAndTerminal,
                &beat.id,
                format!(
                    "beat re-opens thread {:?} after its unqualified win; a yes-and is terminal \
                     per thread across the season",
                    beat.thread
                ),
            ));
        }
    }
}

/// Competence-consistency: a major unassisted win by an under-competent hand needs
/// an intervening growth or price beat (warn); and a competence trajectory that
/// never moves across the season reads arc-flat (warn).
fn check_competence(
    ledger: &CraftLedger,
    thresholds: &Thresholds,
    order: &ProductionOrder,
    out: &mut Vec<WorldFinding>,
) {
    let slots = episode_slots(ledger);
    // Per-character competence readings in production-slot order.
    let mut readings: HashMap<&str, Vec<(u32, u8)>> = HashMap::new();
    for s in &ledger.sliders {
        let slot = slots.get(s.episode.as_str()).copied().unwrap_or(u32::MAX);
        readings
            .entry(s.character.as_str())
            .or_default()
            .push((slot, s.competence));
    }
    for v in readings.values_mut() {
        v.sort_unstable();
    }

    for (bi, beat) in ledger.beats.iter().enumerate() {
        if beat.kind != BeatKind::Accomplishment
            || mag_rank(beat.magnitude) < mag_rank(Magnitude::Major)
            || beat.assisted
            || beat.by.is_empty()
        {
            continue;
        }
        let character = beat.by.as_str();
        let slot = slots
            .get(beat.episode.as_str())
            .copied()
            .unwrap_or(u32::MAX);
        let Some(competence) = competence_as_of(readings.get(character), slot) else {
            continue;
        };
        if competence >= thresholds.competence_floor {
            continue;
        }
        let grew = readings.get(character).is_some_and(|r| rises_by(r, slot));
        let paid_price = ledger
            .beats
            .iter()
            .enumerate()
            .any(|(fi, f)| order.before(fi, bi) && f.by == beat.by && is_cost(f));
        if !grew && !paid_price {
            out.push(craft_warn(
                KernelRule::CompetenceConsistency,
                &beat.id,
                format!(
                    "{character} lands a major unassisted win at competence {competence} (below \
                     {}) with no intervening growth, aid, or price beat",
                    thresholds.competence_floor
                ),
            ));
        }
    }

    // Arc-flat: a character whose competence never changes across >= 2 readings.
    let mut flat: Vec<&str> = readings
        .iter()
        .filter(|(_, r)| r.len() >= 2 && r.iter().all(|(_, c)| *c == r[0].1))
        .map(|(name, _)| *name)
        .collect();
    flat.sort_unstable();
    for character in flat {
        out.push(craft_warn(
            KernelRule::CompetenceArcFlat,
            character,
            "competence never moves across the season; a flat trajectory reads as a character \
             who does not grow",
        ));
    }
}

/// Reveal fair-play: a kishōtenketsu turn's reframing fact must sit on the listener
/// ledger at least `reveal_lead_episodes` episodes before the turn.
fn check_reveal_fair_play(
    ledger: &CraftLedger,
    thresholds: &Thresholds,
    slots: &HashMap<&str, u32>,
    out: &mut Vec<WorldFinding>,
) {
    for turn in &ledger.turns {
        let Some(&turn_slot) = slots.get(turn.episode.as_str()) else {
            continue;
        };
        let lead = thresholds.reveal_lead_episodes;
        let fair = ledger.reveals.iter().any(|r| {
            r.fact == turn.reframing_fact
                && slots
                    .get(r.episode.as_str())
                    .is_some_and(|&s| s + lead <= turn_slot)
        });
        if !fair {
            out.push(craft_warn(
                KernelRule::RevealFairPlay,
                &turn.episode,
                format!(
                    "the turn reframes on {:?}, which the listener has not held for {lead} \
                     episode(s) before the turn; kishōtenketsu is fair-play — the reframing fact \
                     must be on the ledger early",
                    turn.reframing_fact
                ),
            ));
        }
    }
}

/// Mystery-starved: an open mystery promise untouched for `mystery_starved_gap`
/// consecutive season episodes.
fn check_mystery_starved(
    ledger: &CraftLedger,
    thresholds: &Thresholds,
    out: &mut Vec<WorldFinding>,
) {
    let mut season_slots: Vec<u32> = ledger.episodes.iter().map(|e| e.slot).collect();
    season_slots.sort_unstable();
    let slots = episode_slots(ledger);

    for promise in &ledger.promises {
        if promise.kind != PromiseKind::Mystery {
            continue;
        }
        let Some(&open_slot) = slots.get(promise.opened_in.as_str()) else {
            continue;
        };
        let close_slot = promise
            .closed_in
            .as_deref()
            .and_then(|e| slots.get(e).copied());
        let mut touched: BTreeSet<u32> = BTreeSet::new();
        touched.insert(open_slot);
        for e in &promise.touched_in {
            if let Some(&s) = slots.get(e.as_str()) {
                touched.insert(s);
            }
        }
        let window_end =
            close_slot.unwrap_or_else(|| season_slots.last().copied().unwrap_or(open_slot));
        let mut run = 0usize;
        let mut worst = 0usize;
        for &slot in &season_slots {
            if slot <= open_slot || slot > window_end {
                continue;
            }
            if touched.contains(&slot) {
                run = 0;
            } else {
                run += 1;
                worst = worst.max(run);
            }
        }
        if worst >= thresholds.mystery_starved_gap {
            out.push(craft_warn(
                KernelRule::MysteryStarved,
                &promise.id,
                format!(
                    "mystery promise goes untouched for {worst} consecutive episodes; an open \
                     mystery starves the listener past {} episodes",
                    thresholds.mystery_starved_gap
                ),
            ));
        }
    }
}

/// Cold-open discipline: a cold open must open a loop, carry cast within the
/// ceiling, and hold no deep-time in act 1.
fn check_cold_open_discipline(
    ledger: &CraftLedger,
    thresholds: &Thresholds,
    out: &mut Vec<WorldFinding>,
) {
    for scene in &ledger.scenes {
        if !scene.cold_open {
            continue;
        }
        if !scene.opens_loop {
            out.push(craft_warn(
                KernelRule::ColdOpenDiscipline,
                &scene.id,
                "cold open does not open a loop; a cold open must give the listener a reason to \
                 lean in",
            ));
        }
        if scene.cast.len() > thresholds.cold_open_cast_ceiling {
            out.push(craft_warn(
                KernelRule::ColdOpenDiscipline,
                &scene.id,
                format!(
                    "cold open carries {} voices; keep it to <= {} so the ear can catch",
                    scene.cast.len(),
                    thresholds.cold_open_cast_ceiling
                ),
            ));
        }
        if scene.deep_time && scene.act.is_none_or(|a| a == 1) {
            out.push(craft_warn(
                KernelRule::ColdOpenDiscipline,
                &scene.id,
                "cold open reaches into deep time in act 1; orient the listener in the present \
                 before opening history",
            ));
        }
    }
}

/// Cast-introduction rate: new named speakers per episode within the band, and the
/// season total within the named-cast budget when one is set.
fn check_cast_introduction(
    ledger: &CraftLedger,
    thresholds: &Thresholds,
    out: &mut Vec<WorldFinding>,
) {
    let (lo, hi) = thresholds.cast_introduction_band;
    let mut total = 0usize;
    for ep in &ledger.episodes {
        let n = ep.introduced_names.len();
        total += n;
        if n > hi {
            out.push(craft_warn(
                KernelRule::CastIntroductionRate,
                &ep.id,
                format!("{n} new named speakers this episode; the band tops out at {hi}"),
            ));
        }
    }
    if let Some(budget) = thresholds.season_cast_budget
        && total > budget
    {
        out.push(craft_warn(
            KernelRule::CastIntroductionRate,
            "season",
            format!("{total} named speakers introduced across the season; the budget is {budget}"),
        ));
    }
    let _ = lo; // the lower band is advisory; a sparse episode is not a finding.
}

/// Scene-name clash: two names below the aural-distinctness floor may not share a
/// scene (error — the one join only a kernel holding both naming data and casts can
/// make; the global distinctness check stays the oracle's).
fn check_scene_name_clash(ledger: &CraftLedger, out: &mut Vec<WorldFinding>) {
    for scene in &ledger.scenes {
        for i in 0..scene.cast.len() {
            for j in (i + 1)..scene.cast.len() {
                let pair = normalized_pair(&scene.cast[i], &scene.cast[j]);
                if ledger.confusable_pairs.contains(&pair) {
                    out.push(craft_error(
                        KernelRule::SceneNameClash,
                        &scene.id,
                        format!(
                            "names {:?} and {:?} sit below the aural-distinctness floor and share \
                             this scene; they may coexist in the series but not in one scene",
                            pair.0, pair.1
                        ),
                    ));
                }
            }
        }
    }
}

/// Promise-density bands: opens should exceed closes in the season's early half and
/// the reverse late; a finale opens only declared carries.
fn check_promise_density(ledger: &CraftLedger, out: &mut Vec<WorldFinding>) {
    if ledger.episodes.is_empty() {
        return;
    }
    let slots = episode_slots(ledger);
    let mut sorted: Vec<u32> = ledger.episodes.iter().map(|e| e.slot).collect();
    sorted.sort_unstable();
    let midpoint = sorted[sorted.len() / 2];
    let finale = *sorted.last().expect("nonempty");

    let (mut early_open, mut early_close, mut late_open, mut late_close) = (0i32, 0i32, 0i32, 0i32);
    for p in &ledger.promises {
        if let Some(&s) = slots.get(p.opened_in.as_str()) {
            if s < midpoint {
                early_open += 1;
            } else {
                late_open += 1;
            }
            if s == finale && !p.declared_carry {
                out.push(craft_warn(
                    KernelRule::PromiseDensity,
                    &p.id,
                    "the finale opens a promise that is not a declared carry; a finale should pay \
                     off, opening only what it deliberately carries on",
                ));
            }
        }
        if let Some(close) = &p.closed_in
            && let Some(&s) = slots.get(close.as_str())
        {
            if s < midpoint {
                early_close += 1;
            } else {
                late_close += 1;
            }
        }
    }
    if early_open <= early_close && (early_open + early_close) > 0 {
        out.push(craft_warn(
            KernelRule::PromiseDensity,
            "season",
            format!(
                "the season's early half closes as many promises as it opens ({early_open} open / \
                 {early_close} close); early episodes should open ahead of closing"
            ),
        ));
    }
    if late_open > late_close {
        out.push(craft_warn(
            KernelRule::PromiseDensity,
            "season",
            format!(
                "the season's late half opens more than it closes ({late_open} open / \
                 {late_close} close); late episodes should close ahead of opening"
            ),
        ));
    }
}

/// Arc-shape conformance: a kishōtenketsu episode places its *ten* in the declared
/// band, and the turn is not a conflict-resolution beat.
fn check_arc_shape(ledger: &CraftLedger, thresholds: &Thresholds, out: &mut Vec<WorldFinding>) {
    let (lo, hi) = thresholds.ten_position_band;
    for ep in &ledger.episodes {
        if ep.arc_shape != Some(ArcShape::Kishotenketsu) {
            continue;
        }
        if let Some(pos) = ep.ten_position
            && (pos < lo || pos > hi)
        {
            out.push(craft_warn(
                KernelRule::ArcShape,
                &ep.id,
                format!(
                    "the ten lands at {pos:.2} of the episode; a kishōtenketsu turn belongs in \
                     the {lo:.2}..{hi:.2} band"
                ),
            ));
        }
        if ledger
            .turns
            .iter()
            .any(|t| t.episode == ep.id && t.resolves_conflict)
        {
            push_cited(
                out,
                &DEFAULT_FINDING_POLICY,
                craft_warn(
                    KernelRule::ArcShape,
                    &ep.id,
                    "the ten resolves a conflict; the turn should be a perspective shift, not a \
                     clash resolved",
                ),
            );
        }
    }
}

/// Audio-load trajectory: no run of `audio_high_run` consecutive high-load episodes.
fn check_audio_load(ledger: &CraftLedger, thresholds: &Thresholds, out: &mut Vec<WorldFinding>) {
    let mut sorted: Vec<&EpisodeMeta> = ledger.episodes.iter().collect();
    sorted.sort_by_key(|e| e.slot);
    let mut run = 0usize;
    let mut run_start: Option<&str> = None;
    for ep in sorted {
        if ep.audio_load == Some(AudioLoad::High) {
            if run == 0 {
                run_start = Some(ep.id.as_str());
            }
            run += 1;
            if run == thresholds.audio_high_run {
                out.push(craft_warn(
                    KernelRule::AudioLoadTrajectory,
                    run_start.unwrap_or(ep.id.as_str()),
                    format!(
                        "{run} consecutive high-orientation-load episodes; the listener needs a \
                         low-load episode to recover before the run reaches {}",
                        thresholds.audio_high_run
                    ),
                ));
            }
        } else {
            run = 0;
            run_start = None;
        }
    }
}

// -- helpers ------------------------------------------------------------------

/// The competence reading as of a production slot: the latest reading at or before
/// it (the fluent's value at that point on the production axis).
fn competence_as_of(readings: Option<&Vec<(u32, u8)>>, slot: u32) -> Option<u8> {
    readings?.iter().rfind(|(s, _)| *s <= slot).map(|(_, c)| *c)
}

/// Whether a sorted reading trajectory rises at any point up to and including
/// `slot` — an intervening growth beat, expressed as a competence increase.
fn rises_by(readings: &[(u32, u8)], slot: u32) -> bool {
    readings
        .iter()
        .take_while(|(s, _)| *s <= slot)
        .zip(readings.iter().skip(1))
        .any(|((_, prev), (_, next))| next > prev)
}

/// A name pair, normalized to lexicographic order for order-insensitive matching.
fn normalized_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_owned(), b.to_owned())
    } else {
        (b.to_owned(), a.to_owned())
    }
}

/// A beat that paid a cost the next win is earned by — a typed non-terminal
/// outcome {yes-but, no-but, no-and}, or a `fail` beat (the payload's coarse
/// signal when the finer outcome was not extracted). Spec §3's failure set, read
/// through whichever fidelity the payload carried.
fn is_cost(beat: &LedgerBeat) -> bool {
    beat.kind == BeatKind::Fail
        || matches!(
            beat.outcome,
            Some(TryFailOutcome::YesBut | TryFailOutcome::NoBut | TryFailOutcome::NoAnd)
        )
}

/// A thread's terminal, unqualified win — a `yes-and` outcome, or a climax
/// accomplishment when the payload carried no finer outcome (spec §3).
fn is_terminal_win(beat: &LedgerBeat) -> bool {
    matches!(beat.outcome, Some(TryFailOutcome::YesAnd))
        || (beat.kind == BeatKind::Accomplishment && beat.magnitude == Magnitude::Climax)
}

fn magnitude_label(m: Magnitude) -> &'static str {
    match m {
        Magnitude::Minor => "minor",
        Magnitude::Major => "major",
        Magnitude::Climax => "climax",
    }
}

fn craft_warn(rule: KernelRule, where_: &str, message: impl Into<String>) -> WorldFinding {
    WorldFinding::warn(format!("craft.{}", rule.id()), with_where(where_, message))
}

fn craft_error(rule: KernelRule, where_: &str, message: impl Into<String>) -> WorldFinding {
    WorldFinding::error(format!("craft.{}", rule.id()), with_where(where_, message))
}
