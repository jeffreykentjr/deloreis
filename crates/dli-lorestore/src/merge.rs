//! The merge engine: 3-way merge over fact sets with structural conflict
//! detection (DL-0041, roadmap §LS-6).
//!
//! ## Why merge here is simpler than git's
//!
//! Facts are immutable and the fact set is append-only, so the store's monotone
//! core is a **grow-only set** with a per-fact polynomial join (`⊕` =
//! [`PosBool::or_`](dli_core::posbool::PosBool::or_), alternative support). Union
//! of monotone accumulations is a join-semilattice merge — confluent,
//! order-independent, coordination-free (CALM; roadmap §4). **Modify/modify
//! conflicts, the entire hard case of text merging, cannot exist:** the same
//! proposition on two branches is the same content-addressed object, so a merge
//! is set union and the polynomial join happens for free on recompile.
//!
//! Non-monotonicity is quarantined into exactly three named places, and that
//! quarantine *is* the merge report:
//!
//! 1. **Valuation disagreement** — two branches rule the same event variable
//!    differently. Never auto-resolved; the sovereign decides (koan 1).
//! 2. **Constraint violations in the merged world** — exclusivity-family and
//!    functional-dependency / certain-layer contradiction, detected by
//!    recompiling the union and surfaced as graded findings with repair data.
//! 3. **Stratified-negation effects** — a fact added on one branch can kill a
//!    higher-stratum derivation certain on the other, or make the merged rule
//!    set unstratifiable; the report names the atoms whose standing changed and
//!    flags an unstratifiable merge.
//!
//! The fact-set union itself is always sound, so the merge commit is always
//! written; the conflict report is advisory data the sovereign rules on. This
//! module is pure detection over compiled worlds; the commit-writing orchestration
//! is [`Lorestore::merge`](crate::Lorestore::merge).

use std::collections::{BTreeMap, BTreeSet};

use dli_core::ann::Status;
use dli_core::atom::Atom;
use dli_core::valuation::Valuation;
use dli_eval::error::EvalError;
use dli_eval::fork::{Conflict, ConflictKind};
use dli_world::CompiledWorld;
use dli_world::check::{Severity, WorldFinding};

use crate::identity::ContentHash;
use crate::lorestore::Lorestore;
use crate::storage::StoreError;

/// One atom whose standing changed **downward** in the merged world relative to a
/// pre-merge branch — the non-monotone (stratified-negation) effect surface. A
/// downgrade means a new fact from the other branch defeated or un-derived a
/// claim the branch held. Upgrades (expected monotone growth) are not reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandingChange {
    /// The affected atom, rendered `pred(a, b)`.
    pub atom: String,
    /// Which pre-merge branch the atom was stronger on (`target` or `source`).
    pub branch: String,
    /// The atom's standing on that branch (`certain` | `possible` | `dead`).
    pub before: String,
    /// The atom's standing in the merged world.
    pub after: String,
}

/// The full report of merging one branch into another (LS-6).
#[derive(Debug, Clone)]
pub struct MergeReport {
    /// The merge commit written (`parent` = target head, `merge_parent` = source
    /// head).
    pub merge_commit: ContentHash,
    /// The merge base (lowest common ancestor), if the branches share history.
    pub merge_base: Option<ContentHash>,
    /// Structural conflicts across the three classes. Empty means the merge is
    /// clean; a non-empty report means the sovereign must rule. The fact-set
    /// union is confluent regardless, so the commit is written either way.
    pub conflicts: Vec<Conflict>,
    /// Graded findings over the recompiled merged world — the constraint
    /// violations (exclusivity / functional-dependency), the recompile's own
    /// `check::run` battery (rejected-lore, chronology-cycle, loader
    /// fail-closed), and any stratification failure — each with repair data.
    pub findings: Vec<WorldFinding>,
    /// Whether the merged rule set stratifies. `false` means the merge
    /// introduced a negative-dependency cycle (a stratified-negation conflict).
    pub stratifies: bool,
    /// Atoms whose standing changed downward vs. either pre-merge branch.
    pub standing_changes: Vec<StandingChange>,
}

/// The detection result (everything except the commit hash and merge base, which
/// [`Lorestore::merge`](crate::Lorestore::merge) supplies).
pub(crate) struct ConflictScan {
    pub conflicts: Vec<Conflict>,
    pub findings: Vec<WorldFinding>,
    pub stratifies: bool,
    pub standing_changes: Vec<StandingChange>,
}

/// Reconcile two parents' interners into the merged commit's interner.
///
/// The merge commit's interner must extend **both** parents append-only — every
/// stored fact's support polynomial uses interner indices, so a merged interner
/// that dropped or reordered either parent's names would silently misresolve that
/// branch's provenance (the interner-regression hazard, now checked against both
/// parents). The only order-preserving union that keeps both parents as prefixes
/// is the longer of the two, and it exists **iff** the shorter interner is a
/// prefix of the longer. When neither is a prefix of the other the branches
/// minted colliding event variables independently, and the merge is refused —
/// loud totality over a silent misresolution.
///
/// Scope caveat (interner review, low): even when this succeeds, bare-`@proposed`
/// facts mint order-dependent positional gates (`h.<pack>#N`), so recompiled
/// *bytes* — not observable worlds — can depend on merge fold-order. The LS-6
/// oracles cite a ruled decision (`D-1`) to stay in the order-independent regime;
/// the roadmap + the DL entry document the caveat.
pub fn reconcile_interners(a: &[String], b: &[String]) -> Result<Vec<String>, StoreError> {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let diverges_at = short
        .iter()
        .zip(long.iter())
        .take_while(|(x, y)| x == y)
        .count();
    if diverges_at < short.len() {
        return Err(StoreError::InternerRegression {
            parent_len: short.len(),
            diverges_at,
        });
    }
    Ok(long.to_vec())
}

/// Union two branch valuation overlays. Where both rule the same variable the
/// **target** value is kept — deterministic, and any disagreement is reported as
/// a [`ConflictKind::Valuation`] conflict, never silently reconciled.
pub fn union_valuations(target: &Valuation, source: &Valuation) -> Valuation {
    let mut merged = target.clone();
    for (&atom, &value) in &source.0 {
        merged.0.entry(atom).or_insert(value);
    }
    merged
}

/// Valuation-disagreement conflicts (class 1): a variable both branches ruled,
/// differently. Never auto-resolved.
pub fn valuation_conflicts(
    target: &Valuation,
    source: &Valuation,
    interner_names: &[String],
) -> Vec<Conflict> {
    let mut out = Vec::new();
    for (&atom, &tv) in &target.0 {
        if let Some(&sv) = source.0.get(&atom)
            && tv != sv
        {
            let name = name_at(interner_names, atom);
            out.push(Conflict {
                kind: ConflictKind::Valuation,
                blame: (
                    format!("target rules {name} = {tv}"),
                    format!("source rules {name} = {sv}"),
                ),
                rendered: format!(
                    "the branches rule {name} differently ({tv} vs {sv}); \
                     the sovereign decides — merge never auto-resolves a valuation"
                ),
            });
        }
    }
    out
}

/// Run the three-class conflict scan over a written merge commit. Called by
/// [`Lorestore::merge`](crate::Lorestore::merge) with the merged interner and the
/// two branch overlays (indexed by the merged interner — merge reconciled them).
#[allow(clippy::too_many_arguments)]
pub(crate) fn scan(
    store: &Lorestore,
    merge_hash: &ContentHash,
    target_head: &ContentHash,
    source_head: &ContentHash,
    base: Option<&ContentHash>,
    target_val: &Valuation,
    source_val: &Valuation,
    merged_val: &Valuation,
    merged_interner: &[String],
) -> Result<ConflictScan, StoreError> {
    // Class 1 — valuation disagreement, straight off the two overlays.
    let mut conflicts = valuation_conflicts(target_val, source_val, merged_interner);

    let merged_named = overlay_to_named(merged_val, merged_interner);
    let target_named = overlay_to_named(target_val, merged_interner);
    let source_named = overlay_to_named(source_val, merged_interner);

    // Recompile the union. The stratify seam surfaces `Unstratifiable` here — a
    // negative-dependency cycle the merge introduced (class 3, extreme form).
    let merged_pack = store.reconstruct_at(merge_hash)?;
    let merged_world = match dli_world::compile(&merged_pack) {
        Ok(world) => world,
        Err(EvalError::Unstratifiable { reason }) => {
            let conflict = Conflict {
                kind: ConflictKind::Certain,
                blame: (
                    "merged rule set".to_owned(),
                    format!("negative-dependency cycle: {reason}"),
                ),
                rendered: format!(
                    "merging introduced a stratified-negation failure — the merged \
                     world is not stratifiable: {reason}"
                ),
            };
            let finding = WorldFinding::error("merge.unstratifiable", conflict.rendered.clone());
            conflicts.push(conflict);
            return Ok(ConflictScan {
                conflicts,
                findings: vec![finding],
                stratifies: false,
                standing_changes: Vec::new(),
            });
        }
        Err(other) => {
            return Err(StoreError::Io {
                path: "merge-compile".to_owned(),
                source: std::io::Error::other(other.to_string()),
            });
        }
    };

    let merged_eff = effective(&merged_world, &merged_named);

    // Class 2 — constraint violations in the merged world. The recompile's own
    // check battery is graded findings with repair data already; exclusivity and
    // functional-dependency are detected structurally and surfaced as both a
    // typed `Conflict` and a graded finding.
    let mut findings = dli_world::check::run(&merged_world);

    let exclusivity = exclusivity_conflicts(&merged_world, &merged_eff);
    let base_world = match base {
        Some(b) => Some(store.compile_at(b)?),
        None => None,
    };
    let certain_layer = certain_layer_conflicts(base_world.as_ref(), &merged_world, &merged_eff);
    for conflict in exclusivity.iter().chain(&certain_layer) {
        findings.push(constraint_finding(conflict));
    }
    conflicts.extend(exclusivity);
    conflicts.extend(certain_layer);

    // Class 3 — stratified-negation / non-monotone standing flips. Compile each
    // pre-merge branch under its own overlay and name atoms the merge downgraded.
    let target_world = store.compile_at(target_head)?;
    let source_world = store.compile_at(source_head)?;
    let target_eff = effective(&target_world, &target_named);
    let source_eff = effective(&source_world, &source_named);
    let standing_changes = standing_downgrades(
        &[
            ("target", &target_world, &target_eff),
            ("source", &source_world, &source_eff),
        ],
        &merged_world,
        &merged_eff,
    );
    for change in &standing_changes {
        findings.push(WorldFinding::warn(
            "merge.standing-change",
            format!(
                "{}: {} → {} — a fact merged from the other branch changed its standing",
                change.atom, change.before, change.after
            ),
        ));
    }

    Ok(ConflictScan {
        conflicts,
        findings,
        stratifies: true,
        standing_changes,
    })
}

// -- constraint detectors ----------------------------------------------------

/// Exclusivity conflicts: the merged valuation rules two members of one
/// exclusivity family true at once (`Theory::respects` would reject any monomial
/// resting on both). Mirrors the fork engine's `exclusivity_conflicts`, run over
/// the merged effective valuation.
fn exclusivity_conflicts(world: &CompiledWorld, eff: &Valuation) -> Vec<Conflict> {
    let interner = &world.model.world.interner;
    let mut out = Vec::new();
    for family in &world.model.world.theory.exclusive {
        let trues: Vec<Atom> = family
            .iter()
            .copied()
            .filter(|&a| eff.get(a) == Some(true))
            .collect();
        if trues.len() >= 2 {
            let a = interner
                .resolve(trues[0])
                .map_or_else(|| format!("var#{}", trues[0].0), ToOwned::to_owned);
            let b = interner
                .resolve(trues[1])
                .map_or_else(|| format!("var#{}", trues[1].0), ToOwned::to_owned);
            out.push(Conflict {
                kind: ConflictKind::Exclusivity,
                blame: (a.clone(), b.clone()),
                rendered: format!(
                    "{a} and {b} are options of one question and cannot both be \
                     ruled true in the merged world"
                ),
            });
        }
    }
    out
}

/// Certain-layer / functional-dependency conflicts: a predicate the base world
/// holds functional now carries two distinct certain values for one subject in
/// the merged world. Base functionality is data-driven (mirrors
/// `continuity::predicate_is_functional`); the merge that breaks it is the
/// conflict (mirrors `continuity::intra_contradictions`), so a genuinely
/// multi-valued relation never false-positives.
fn certain_layer_conflicts(
    base: Option<&CompiledWorld>,
    merged: &CompiledWorld,
    merged_eff: &Valuation,
) -> Vec<Conflict> {
    let Some(base) = base else {
        // Without a shared base we cannot say a predicate was *intended*
        // functional, so we do not guess — an honest limitation (roadmap §LS-6).
        return Vec::new();
    };
    let base_eff = base.model.world.valuation.clone();
    let functional = functional_predicates(base, &base_eff);

    let mut slots: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for (pred, subject, value) in certain_slots(merged, merged_eff) {
        slots.entry((pred, subject)).or_default().insert(value);
    }

    let mut out = Vec::new();
    for ((pred, subject), values) in slots {
        if values.len() >= 2 && functional.contains(&pred) {
            let rendered: Vec<&str> = values.iter().map(String::as_str).collect();
            out.push(Conflict {
                kind: ConflictKind::Certain,
                blame: (
                    format!("{pred}({subject}) = {}", rendered[0]),
                    format!("{pred}({subject}) = {}", rendered[1]),
                ),
                rendered: format!(
                    "the merged certain layer breaks the functional dependency on \
                     '{pred}': {subject} carries {} distinct certain values [{}]",
                    values.len(),
                    rendered.join(", ")
                ),
            });
        }
    }
    out
}

/// The set of predicates the world holds functional under `eff`: every predicate
/// that appears in at least one certain binary fact and whose every subject
/// carries at most one certain value.
fn functional_predicates(world: &CompiledWorld, eff: &Valuation) -> BTreeSet<String> {
    let mut by_pred: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    for (pred, subject, value) in certain_slots(world, eff) {
        by_pred
            .entry(pred)
            .or_default()
            .entry(subject)
            .or_default()
            .insert(value);
    }
    by_pred
        .into_iter()
        .filter(|(_, subjects)| {
            !subjects.is_empty() && subjects.values().all(|values| values.len() <= 1)
        })
        .map(|(pred, _)| pred)
        .collect()
}

/// The `(pred, subject, value)` triples for every certain binary fact under `eff`.
fn certain_slots(world: &CompiledWorld, eff: &Valuation) -> Vec<(String, String, String)> {
    let theory = &world.model.world.theory;
    let mut out = Vec::new();
    for derived in &world.model.world.derived {
        if derived.ann().status(eff, theory) != Status::Certain {
            continue;
        }
        let atom = &derived.atom;
        if atom.args.len() == 2 {
            out.push((
                atom.pred.clone(),
                atom.args[0].clone(),
                atom.args[1].clone(),
            ));
        }
    }
    out
}

// -- standing (class 3) ------------------------------------------------------

/// Every atom's best standing under `eff`, keyed by rendered `pred(a, b)`.
fn standings_map(world: &CompiledWorld, eff: &Valuation) -> BTreeMap<String, Status> {
    let theory = &world.model.world.theory;
    let mut out: BTreeMap<String, Status> = BTreeMap::new();
    for derived in &world.model.world.derived {
        let status = derived.ann().status(eff, theory);
        let key = derived.atom.render();
        out.entry(key)
            .and_modify(|s| {
                if rank(status) > rank(*s) {
                    *s = status;
                }
            })
            .or_insert(status);
    }
    out
}

/// Atoms the merge downgraded relative to a pre-merge branch — the non-monotone
/// effect the stratified-negation channel produces.
fn standing_downgrades(
    branches: &[(&str, &CompiledWorld, &Valuation)],
    merged: &CompiledWorld,
    merged_eff: &Valuation,
) -> Vec<StandingChange> {
    let merged_standings = standings_map(merged, merged_eff);
    let mut out = Vec::new();
    for (label, world, eff) in branches {
        for (atom, &before) in &standings_map(world, eff) {
            let after = merged_standings.get(atom).copied().unwrap_or(Status::Dead);
            if rank(after) < rank(before) {
                out.push(StandingChange {
                    atom: atom.clone(),
                    branch: (*label).to_owned(),
                    before: status_word(before).to_owned(),
                    after: status_word(after).to_owned(),
                });
            }
        }
    }
    out
}

// -- helpers -----------------------------------------------------------------

/// A constraint conflict rendered as a graded, deterministic finding with its
/// blame pair as citations (the repair data). The code comes from
/// [`ConflictKind::merge_code`] — the single source of truth.
fn constraint_finding(conflict: &Conflict) -> WorldFinding {
    WorldFinding {
        code: conflict.kind.merge_code().to_owned(),
        severity: Severity::Error,
        grade: dli_core::grade::Grade::Checked,
        message: conflict.rendered.clone(),
        citations: vec![conflict.blame.0.clone(), conflict.blame.1.clone()],
        span: None,
    }
}

/// Resolve overlay atom indices to names through the (merged) interner.
fn overlay_to_named(overlay: &Valuation, interner_names: &[String]) -> BTreeMap<String, bool> {
    overlay
        .0
        .iter()
        .filter_map(|(atom, &value)| {
            interner_names
                .get(atom.0 as usize)
                .map(|name| (name.clone(), value))
        })
        .collect()
}

/// The compiled world's base valuation with a name-keyed overlay applied — robust
/// to any interner-index remapping between the stored commit and the recompiled
/// world (names are the stable key).
fn effective(world: &CompiledWorld, named: &BTreeMap<String, bool>) -> Valuation {
    let mut eff = world.model.world.valuation.clone();
    let table = world.model.world.interner.names();
    let by_name: BTreeMap<&str, u32> = table
        .iter()
        .enumerate()
        .map(|(pos, n)| (n.as_str(), u32::try_from(pos).unwrap_or(u32::MAX)))
        .collect();
    for (name, &value) in named {
        if let Some(&pos) = by_name.get(name.as_str()) {
            eff.set(Atom(pos), value);
        }
    }
    eff
}

fn name_at(interner_names: &[String], atom: Atom) -> String {
    interner_names
        .get(atom.0 as usize)
        .cloned()
        .unwrap_or_else(|| format!("var#{}", atom.0))
}

const fn rank(status: Status) -> u8 {
    match status {
        Status::Dead => 0,
        Status::Possible => 1,
        Status::Certain => 2,
    }
}

const fn status_word(status: Status) -> &'static str {
    match status {
        Status::Certain => "certain",
        Status::Possible => "possible",
        Status::Dead => "dead",
    }
}
