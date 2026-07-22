//! Export backend: the articy-shaped video-game profile (`EXP-4`).
//!
//! articy:draft X is the commercial proof of the seat this fills (spec 09 §1): a
//! narrative-production database of typed entities, dialogues, quests, and
//! variables, exported as JSON for Unity/Unreal importers. De Lore Is is the
//! missing **authoritative** layer that feeds such a tool, upgraded with the
//! provenance machinery articy lacks.
//!
//! # What this v1 models
//!
//! A real (v1) articy-shaped export of the **certain layer only** — proposed lore
//! never reaches here, the fail-closed filter having already run in
//! [`export`](crate::export). Four mappings carry knowledge, never behavior (spec
//! 09 §2 clause 3):
//!
//! - **`person`/`place`/`thing` → typed entities with their fact scopes.** Each
//!   sort atom becomes an [`ArticyEntity`] carrying the rendered facts it
//!   participates in — the local knowledge slice a dialogue node reads.
//! - **`knows(scope, claim)` → who-may-know-what.** Which factions, regions, or
//!   characters are licensed to know a claim rides on [`ArticyFact::who_may_know`].
//!   This is the *visibility* dimension, distinct from the *presence* one: the
//!   keep-stamp filter already dropped who-may-**not**-know atoms upstream, so the
//!   honest-vs-keep-stamped diff of two articy exports is the who-knows-what gate a
//!   dialogue system needs, as data.
//! - **promise/debt/discharge → quest flow.** A promise is a hook, an open debt an
//!   objective, a discharge a completion; the three stages of one id collapse into
//!   a single [`ArticyQuest`] whose `state` is the furthest reached and whose
//!   `flow` lists the stages present in order (spec 09 §3).
//! - **provenance → dev-metadata.** Cited decisions ride as [`ArticyFact::cited`]
//!   and are **stripped for ship** when the profile's
//!   [`carries_provenance`](crate::profile::ProfileSpec::carries_provenance) is
//!   [`Stripped`](crate::profile::ProvenanceMode::Stripped).
//!
//! # What a v1 does not model yet
//!
//! Rich **dialogue graphs** (branching lines, conditions, pins) and **global
//! variables** are articy features with no lore-side source in the certain atoms
//! today; they are left for a later wave rather than faked. Their absence is
//! honest: a consumer reads real entities, scopes, relations, and quests, and the
//! artifact is no longer `pending`.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_eval::world::GroundAtom;
use dli_world::model::LoadedAtom;

use crate::profile::{ProfileSpec, ProvenanceMode};

/// A typed articy entity (a person, place, or thing) with its fact scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArticyEntity {
    /// The entity id.
    pub id: String,
    /// The entity's sort (`person` / `place` / `thing`).
    pub sort: String,
    /// The rendered facts this entity participates in — its local knowledge slice,
    /// the claims a dialogue node for this entity may reference. Sorted, deduped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<String>,
}

/// A typed relation between two entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArticyRelation {
    /// The relation predicate.
    pub kind: String,
    /// The related argument ids, in order.
    pub args: Vec<String>,
}

/// A dialogue-relevant fact carrying its who-may-know-what scope — the keep-stamp
/// visibility dimension a dialogue system reads to decide what a speaker can
/// reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArticyFact {
    /// The rendered fact (`pred(a, b)`).
    pub claim: String,
    /// The scopes (factions/regions/characters) licensed to know this claim. Empty
    /// means unrestricted. A keep-stamped export never emits a claim a scope may
    /// not know — it is absent, not merely flagged.
    pub who_may_know: Vec<String>,
    /// Cited provenance (dev-metadata; stripped for ship builds).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cited: Vec<String>,
}

/// A quest-shaped structure: a promise is a hook, an open debt an objective, a
/// discharge a completion (spec 09 §3). The stages of one quest id collapse into a
/// single quest whose `state` is the furthest reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArticyQuest {
    /// The quest id (the promise).
    pub id: String,
    /// The furthest quest state reached (`hook` / `objective` / `complete`).
    pub state: String,
    /// The stages present for this quest, in flow order (`hook` before `objective`
    /// before `complete`) — the quest's progression, not just its head.
    pub flow: Vec<String>,
}

/// An articy-shaped export: typed entities, relations, dialogue facts with
/// knowledge scopes, and quest flow.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArticyExport {
    /// The typed entities.
    pub entities: Vec<ArticyEntity>,
    /// The typed relations.
    pub relations: Vec<ArticyRelation>,
    /// The dialogue-relevant facts with knowledge scopes.
    pub facts: Vec<ArticyFact>,
    /// The quest-shaped structures.
    pub quests: Vec<ArticyQuest>,
    /// Whether the export is still a scaffold. `false` for this v1: entities,
    /// scopes, relations, and quest flow are modelled; only rich dialogue graphs
    /// and global variables are deferred (see the module docs).
    pub pending: bool,
    /// The profile identity that produced it.
    pub profile: String,
    /// The layer exported.
    pub layer: String,
}

/// The progression stage a `promise`/`debt`/`discharge` atom contributes to a
/// quest. Ordered: a hook precedes an objective precedes a completion, so the
/// furthest reached is the maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum QuestStage {
    Hook,
    Objective,
    Complete,
}

impl QuestStage {
    /// The articy state label.
    fn label(self) -> &'static str {
        match self {
            Self::Hook => "hook",
            Self::Objective => "objective",
            Self::Complete => "complete",
        }
    }
}

/// Compile a v1 articy export from the certain visible atoms.
///
/// `person`/`place`/`thing` atoms become typed entities carrying the facts they
/// appear in; `knows(scope, claim)` atoms attach who-may-know scopes to facts;
/// `promise`/`debt`/`discharge` atoms fold into per-id quests with an ordered flow;
/// every other ground atom is a fact (and, if binary+, a relation). Output is
/// deterministic (every collection sorted) so two runs over the same atoms are
/// byte-identical. The keep-stamp filter already ran upstream, so any
/// who-may-not-know atom is absent from `atoms`; proposed lore never reaches here.
#[must_use]
pub fn compile(spec: &ProfileSpec, layer: &str, atoms: &[&LoadedAtom]) -> ArticyExport {
    let with_provenance = matches!(spec.carries_provenance, ProvenanceMode::DevMetadata);

    // id -> sort for the typed entities.
    let mut sorts: BTreeMap<String, String> = BTreeMap::new();
    // claim -> the scopes licensed to know it.
    let mut scopes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // quest id -> the stages it has reached.
    let mut quest_stages: BTreeMap<String, BTreeSet<QuestStage>> = BTreeMap::new();
    // (claim, arg ids, citations) for every fact atom.
    let mut fact_rows: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    let mut relations: Vec<ArticyRelation> = Vec::new();

    for atom in atoms {
        let pred = atom.atom.pred.text.as_str();
        match pred {
            "person" | "place" | "thing" => {
                if let Some(id) = arg(atom, 0) {
                    sorts.insert(id, pred.to_owned());
                }
            }
            "knows" => {
                if let (Some(scope), Some(claim)) = (arg(atom, 0), arg(atom, 1)) {
                    scopes.entry(claim).or_default().insert(scope);
                }
            }
            "promise" => note_quest(&mut quest_stages, atom, QuestStage::Hook),
            "debt" => note_quest(&mut quest_stages, atom, QuestStage::Objective),
            "discharge" => note_quest(&mut quest_stages, atom, QuestStage::Complete),
            _ => {
                let Some(claim) = GroundAtom::from_pattern(&atom.atom).map(|g| g.render()) else {
                    continue;
                };
                let arg_ids: Vec<String> = atom.atom.args.iter().filter_map(term_arg).collect();
                let cited = if with_provenance {
                    atom.citations.clone()
                } else {
                    Vec::new()
                };
                if atom.atom.args.len() >= 2 {
                    relations.push(ArticyRelation {
                        kind: pred.to_owned(),
                        args: arg_ids.clone(),
                    });
                }
                fact_rows.push((claim, arg_ids, cited));
            }
        }
    }

    relations.sort_by(|a, b| (&a.kind, &a.args).cmp(&(&b.kind, &b.args)));
    relations.dedup();

    ArticyExport {
        entities: build_entities(sorts, &fact_rows),
        relations,
        facts: build_facts(fact_rows, &scopes),
        quests: fold_quests(quest_stages),
        pending: false,
        profile: spec.qualified(),
        layer: layer.to_owned(),
    }
}

/// A fact atom lowered to `(rendered claim, argument ids, citations)`.
type FactRow = (String, Vec<String>, Vec<String>);

/// Build the typed entities, each carrying (sorted) the facts it participates in —
/// its local knowledge slice.
fn build_entities(sorts: BTreeMap<String, String>, fact_rows: &[FactRow]) -> Vec<ArticyEntity> {
    let mut entity_facts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (claim, arg_ids, _) in fact_rows {
        for id in arg_ids {
            if sorts.contains_key(id) {
                entity_facts
                    .entry(id.clone())
                    .or_default()
                    .insert(claim.clone());
            }
        }
    }
    sorts
        .into_iter()
        .map(|(id, sort)| {
            let facts = entity_facts
                .get(&id)
                .map(|claims| claims.iter().cloned().collect())
                .unwrap_or_default();
            ArticyEntity { id, sort, facts }
        })
        .collect()
}

/// Build the who-may-know facts, deterministically ordered and deduped.
fn build_facts(
    fact_rows: Vec<FactRow>,
    scopes: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<ArticyFact> {
    let mut facts: Vec<ArticyFact> = fact_rows
        .into_iter()
        .map(|(claim, _, cited)| ArticyFact {
            who_may_know: scopes
                .get(&claim)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default(),
            claim,
            cited,
        })
        .collect();
    facts.sort_by(|a, b| a.claim.cmp(&b.claim));
    facts.dedup();
    facts
}

/// Fold the per-id quest stages into quests: `state` is the furthest stage
/// reached, `flow` the stages present in order.
fn fold_quests(quest_stages: BTreeMap<String, BTreeSet<QuestStage>>) -> Vec<ArticyQuest> {
    quest_stages
        .into_iter()
        .map(|(id, stages)| {
            let flow: Vec<String> = stages
                .iter()
                .map(|stage| stage.label().to_owned())
                .collect();
            let state = stages.iter().next_back().map_or_else(
                || QuestStage::Hook.label().to_owned(),
                |s| s.label().to_owned(),
            );
            ArticyQuest { id, state, flow }
        })
        .collect()
}

/// Record a stage against the quest an atom's first argument names.
fn note_quest(
    quests: &mut BTreeMap<String, BTreeSet<QuestStage>>,
    atom: &LoadedAtom,
    stage: QuestStage,
) {
    if let Some(id) = arg(atom, 0) {
        quests.entry(id).or_default().insert(stage);
    }
}

/// The i-th constant argument of an atom, or `None`.
fn arg(atom: &LoadedAtom, index: usize) -> Option<String> {
    atom.atom.args.get(index).and_then(term_arg)
}

/// Render a ground term as a constant string.
fn term_arg(term: &dli_ast::term::Term) -> Option<String> {
    use dli_ast::term::Term;
    match term {
        Term::Sym(id) => Some(id.text.clone()),
        Term::Str(text) => Some(text.clone()),
        Term::Int(value) => Some(value.to_string()),
        _ => None,
    }
}
