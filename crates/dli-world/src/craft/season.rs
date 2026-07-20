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
    /// habit where a perspective shift belongs (`ten-reads-as-battle`; D-0052).
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
/// use dli_world::craft::season::{check_episode, CheckOptions};
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
/// let codes: Vec<_> = check_episode(&ep, CheckOptions::default())
///     .into_iter().map(|f| f.code).collect();
/// assert!(codes.contains(&"outline.cast-over-four".to_owned()));
/// ```
#[must_use]
pub fn check_episode(ep: &EpisodeOutline, opts: CheckOptions) -> Vec<WorldFinding> {
    let scene_ids: BTreeSet<&str> = ep.scene_ids().into_iter().collect();
    let mut out = Vec::new();
    check_promises(ep, &scene_ids, &mut out);
    check_character_sliders(ep, &scene_ids, opts, &mut out);
    check_mice_nesting(ep, &mut out);
    check_scene_work(ep, &scene_ids, opts, &mut out);
    check_tryfail(ep, &mut out);
    check_scenes(ep, &mut out);
    check_audio_plan(ep, opts, &mut out);
    check_material_assumptions(ep, opts, &mut out);
    check_loops(ep, &mut out);
    check_ten(ep, &mut out);
    if ep.hook_by_seconds > policy::HOOK_SECONDS_CEILING {
        out.push(warn(
            "hook-late",
            &ep.id,
            format!(
                "hook lands at {}s; earn attention inside {}s (the audio hook)",
                ep.hook_by_seconds,
                policy::HOOK_SECONDS_CEILING
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
pub fn check_season(season: &SeasonOutline, opts: CheckOptions) -> Vec<WorldFinding> {
    let mut out = Vec::new();
    check_season_promises(season, opts, &mut out);

    let mut opened_so_far: BTreeSet<&str> = BTreeSet::new();
    let mut ever_closed: BTreeSet<&str> = BTreeSet::new();
    for ep in &season.episodes {
        out.extend(check_episode(ep, opts));
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
        if high < policy::SLIDER_FLAT_CEILING {
            out.push(warn(
                "character-slider-flat",
                &slider.character,
                "all three sliders are below 5; the character may read flat",
            ));
        }
        if low < policy::SLIDER_LOW_FLOOR && slider.compensation.trim().is_empty() {
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

fn check_scenes(ep: &EpisodeOutline, out: &mut Vec<WorldFinding>) {
    for s in &ep.scenes {
        if s.speaking_cast.len() > policy::SCENE_CAST_CEILING {
            out.push(warn(
                "cast-over-four",
                &s.id,
                format!(
                    "{} speaking voices; keep a scene to ≤{} (Toscan) so the ear \
                     tracks who is who",
                    s.speaking_cast.len(),
                    policy::SCENE_CAST_CEILING
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

fn check_audio_plan(ep: &EpisodeOutline, opts: CheckOptions, out: &mut Vec<WorldFinding>) {
    let Some(plan) = &ep.audio_plan else {
        if opts.require_methodology {
            out.push(
                warn(
                    "no-audio-plan",
                    &ep.id,
                    "packet review needs narrator/SFX load accounting for D-0179 \
                     audio mode",
                )
                .with_citations(vec!["D-0179".to_owned()]),
            );
        }
        return;
    };

    if plan.direct_address {
        out.push(
            error(
                "narrator-direct-address",
                &ep.id,
                "narrator plan allows direct audience address; D-0176 forbids it",
            )
            .with_citations(vec!["D-0176".to_owned()]),
        );
    }
    if plan.narrator_mode == NarratorMode::Host {
        out.push(
            error(
                "narrator-host-frame",
                &ep.id,
                "narrator plan uses host framing; D-0176 forbids tale-host mode",
            )
            .with_citations(vec!["D-0176".to_owned()]),
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
        out.push(
            warn(
                "sfx-not-sparse",
                &ep.id,
                format!(
                    "SFX load is {}; D-0179 wants sparse support, not a major \
                     semantic channel",
                    sfx_load_label(plan.sfx_load)
                ),
            )
            .with_citations(vec!["D-0179".to_owned()]),
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

    check_material_provenance(ep, ledger, out);

    let unknowns: BTreeSet<MaterialUnknown> = ledger.unknowns.iter().copied().collect();
    if opts.material_worldbuilding {
        for category in MaterialUnknown::REQUIRED {
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
    out: &mut Vec<WorldFinding>,
) {
    // `_assumption_is_nonblank`: a supplied assumption must not be blank.
    for category in MaterialUnknown::ALL {
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
        if !is_canon_ref(r) {
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
        if !is_research_ref(r) {
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

fn check_ten(ep: &EpisodeOutline, out: &mut Vec<WorldFinding>) {
    if !ep.ten.is_empty() && reads_as_battle(&ep.ten) {
        out.push(
            warn(
                "ten-reads-as-battle",
                &ep.id,
                "the ten reads as a battle/confrontation; the turn should be a \
                 perspective shift dissolved through empathy and logic, not a clash",
            )
            .with_citations(vec!["D-0052".to_owned()]),
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

fn is_canon_ref(reference: &str) -> bool {
    // `^D-\d{4}$`
    let Some(digits) = reference.strip_prefix("D-") else {
        return false;
    };
    digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit())
}

// The `.md` match is intentionally case-sensitive: the legacy regex `\.md$` matches
// only lowercase, and parity requires the same.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn is_research_ref(reference: &str) -> bool {
    // `^(?:https?://\S+|RR-\d{3}|docs/research/RR-\d{3}[^/]*\.md)$`
    if let Some(rest) = reference
        .strip_prefix("https://")
        .or_else(|| reference.strip_prefix("http://"))
    {
        return !rest.is_empty() && rest.chars().all(|c| !c.is_whitespace());
    }
    if let Some(digits) = reference.strip_prefix("RR-")
        && digits.len() == 3
        && digits.bytes().all(|b| b.is_ascii_digit())
    {
        return true;
    }
    if let Some(rest) = reference.strip_prefix("docs/research/RR-") {
        // `\d{3}[^/]*\.md`
        let digits: String = rest.chars().take(3).collect();
        if digits.len() == 3 && digits.bytes().all(|b| b.is_ascii_digit()) {
            let tail = &rest[3..];
            return tail.ends_with(".md") && !tail.contains('/');
        }
    }
    false
}

fn reads_as_battle(ten: &str) -> bool {
    let tokens: Vec<String> = ten
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect();
    for token in &tokens {
        if policy::BATTLE_SINGLE.contains(&token.as_str()) {
            return true;
        }
    }
    for pair in tokens.windows(2) {
        for (leads, tail) in policy::BATTLE_PAIRS {
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
