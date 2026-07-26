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

use crate::profile::{ExportError, ProfileSpec, ProvenanceMode};
use crate::projection;

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
    /// Cited provenance for the entity declaration (dev-metadata only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cited: Vec<String>,
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
    /// Cited provenance collected from the quest's stages (dev-metadata only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cited: Vec<String>,
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

/// Raw id -> (rendered id, sort, carries facts, citations).
type EntityMap = BTreeMap<String, (String, String, bool, BTreeSet<String>)>;
type QuestMap = BTreeMap<String, (BTreeSet<QuestStage>, BTreeSet<String>)>;

fn collect_entities(
    spec: &ProfileSpec,
    atoms: &[&LoadedAtom],
    wildcard: bool,
    with_provenance: bool,
) -> Result<EntityMap, ExportError> {
    if let Some(predicate) = spec
        .renders
        .keys()
        .find(|predicate| !spec.selects.sorts.contains(*predicate))
    {
        return Err(ExportError::InvalidProfile {
            profile: spec.qualified(),
            detail: format!(
                "articy render rule `{predicate}` does not name a selected entity sort"
            ),
        });
    }

    let mut entities = EntityMap::new();
    for atom in atoms {
        let pred = atom.atom.pred.text.as_str();
        let selected = if wildcard {
            matches!(pred, "person" | "place" | "thing")
        } else {
            spec.selects.sorts.iter().any(|name| name == pred)
        };
        if !selected {
            continue;
        }
        require_arity(atom, 1)?;
        let raw_id = required_arg(atom, 0, pred)?;
        let rule = spec.renders.get(pred);
        let carries_facts = articy_keep_facts(spec, pred, rule)?;
        let rendered_id = if let Some(rule) = rule.filter(|rule| !rule.id.is_empty()) {
            let mut fields = atom
                .atom
                .args
                .iter()
                .enumerate()
                .filter_map(|(index, term)| {
                    term_arg(term).map(|value| (format!("arg{index}"), value))
                })
                .collect::<BTreeMap<_, _>>();
            fields.insert("id".to_owned(), raw_id.clone());
            fields.insert("sort".to_owned(), pred.to_owned());
            projection::render_template(spec, pred, &rule.id, &fields)?
        } else {
            raw_id.clone()
        };
        if let Some((_, existing_sort, _, _)) = entities.get(&raw_id)
            && existing_sort != pred
        {
            return Err(ExportError::Render {
                target: "articy".to_owned(),
                detail: format!(
                    "entity `{raw_id}` is declared as both `{existing_sort}` and `{pred}`"
                ),
            });
        }
        if entities
            .iter()
            .any(|(existing_raw, (existing_id, _, _, _))| {
                existing_raw != &raw_id && existing_id == &rendered_id
            })
        {
            return Err(ExportError::Render {
                target: "articy".to_owned(),
                detail: format!(
                    "entity id template maps more than one source entity to `{rendered_id}`"
                ),
            });
        }
        let cited = if with_provenance {
            atom.citations.iter().cloned().collect()
        } else {
            BTreeSet::new()
        };
        if let Some((_, _, _, existing_cited)) = entities.get_mut(&raw_id) {
            existing_cited.extend(cited);
        } else {
            entities.insert(raw_id, (rendered_id, pred.to_owned(), carries_facts, cited));
        }
    }
    Ok(entities)
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
pub fn compile(
    spec: &ProfileSpec,
    layer: &str,
    atoms: &[&LoadedAtom],
) -> Result<ArticyExport, ExportError> {
    let with_provenance = matches!(spec.carries_provenance, ProvenanceMode::DevMetadata);
    let wildcard = spec.selects.sorts.is_empty() && spec.selects.relations.is_empty();
    let entities = collect_entities(spec, atoms, wildcard, with_provenance)?;
    let declared_entities = declared_entity_ids(atoms)?;

    // claim -> the scopes licensed to know it.
    let mut scopes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut scope_citations: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // quest id -> the stages it has reached.
    let mut quest_stages = QuestMap::new();
    // (claim, arg ids, citations) for every fact atom.
    let mut fact_rows: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    let mut relations: Vec<ArticyRelation> = Vec::new();

    for atom in atoms {
        let pred = atom.atom.pred.text.as_str();
        let selected_relation = wildcard || spec.selects.relations.iter().any(|name| name == pred);
        if !selected_relation {
            continue;
        }
        let selected_sort = if wildcard {
            matches!(pred, "person" | "place" | "thing")
        } else {
            spec.selects.sorts.iter().any(|name| name == pred)
        };
        if selected_sort {
            continue;
        }
        match pred {
            "knows" => {
                require_arity(atom, 2)?;
                let scope =
                    render_reference(required_arg(atom, 0, pred)?, &entities, &declared_entities)?;
                let claim = required_arg(atom, 1, pred)?;
                scopes.entry(claim.clone()).or_default().insert(scope);
                let citations = scope_citations.entry(claim).or_default();
                if with_provenance {
                    citations.extend(atom.citations.iter().cloned());
                }
            }
            "promise" => note_quest(&mut quest_stages, atom, QuestStage::Hook, with_provenance)?,
            "debt" => note_quest(
                &mut quest_stages,
                atom,
                QuestStage::Objective,
                with_provenance,
            )?,
            "discharge" => note_quest(
                &mut quest_stages,
                atom,
                QuestStage::Complete,
                with_provenance,
            )?,
            _ => {
                let claim = GroundAtom::from_pattern(&atom.atom)
                    .map(|ground| ground.render())
                    .ok_or_else(|| ExportError::Render {
                        target: "articy".to_owned(),
                        detail: format!("selected atom `{pred}` is not ground"),
                    })?;
                let arg_ids = render_args(atom, &entities, &declared_entities)?;
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

    for (claim, cited) in scope_citations {
        if let Some((_, _, fact_citations)) = fact_rows.iter_mut().find(|(row, _, _)| row == &claim)
        {
            fact_citations.extend(cited);
            fact_citations.sort();
            fact_citations.dedup();
        } else {
            fact_rows.push((claim, Vec::new(), cited.into_iter().collect()));
        }
    }

    Ok(ArticyExport {
        entities: build_entities(entities, &fact_rows),
        relations,
        facts: build_facts(fact_rows, &scopes),
        quests: fold_quests(quest_stages),
        pending: false,
        profile: spec.qualified(),
        layer: layer.to_owned(),
    })
}

/// A fact atom lowered to `(rendered claim, argument ids, citations)`.
type FactRow = (String, Vec<String>, Vec<String>);

/// Build the typed entities, each carrying (sorted) the facts it participates in —
/// its local knowledge slice.
fn build_entities(entities: EntityMap, fact_rows: &[FactRow]) -> Vec<ArticyEntity> {
    let mut entity_facts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (claim, arg_ids, _) in fact_rows {
        for id in arg_ids {
            if entities
                .values()
                .any(|(rendered, _, carries_facts, _)| *carries_facts && rendered == id)
            {
                entity_facts
                    .entry(id.clone())
                    .or_default()
                    .insert(claim.clone());
            }
        }
    }
    entities
        .into_iter()
        .map(|(_, (id, sort, carries_facts, cited))| {
            let facts = entity_facts
                .get(&id)
                .filter(|_| carries_facts)
                .map(|claims| claims.iter().cloned().collect())
                .unwrap_or_default();
            ArticyEntity {
                id,
                sort,
                facts,
                cited: cited.into_iter().collect(),
            }
        })
        .collect()
}

fn articy_keep_facts(
    spec: &ProfileSpec,
    predicate: &str,
    rule: Option<&crate::profile::RenderRule>,
) -> Result<bool, ExportError> {
    let Some(rule) = rule else {
        return Ok(true);
    };
    for field in &rule.keep {
        if !matches!(field.as_str(), "id" | "sort" | "facts") {
            return Err(ExportError::InvalidProfile {
                profile: spec.qualified(),
                detail: format!(
                    "articy render rule for `{predicate}` keeps unknown field `{field}`"
                ),
            });
        }
    }
    Ok(rule.keep.is_empty() || rule.keep.iter().any(|field| field == "facts"))
}

/// Build the who-may-know facts, deterministically ordered and deduped.
fn build_facts(
    fact_rows: Vec<FactRow>,
    scopes: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<ArticyFact> {
    let mut citations: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (claim, _, cited) in fact_rows {
        citations.entry(claim).or_default().extend(cited);
    }
    citations
        .into_iter()
        .map(|(claim, cited)| ArticyFact {
            who_may_know: scopes
                .get(&claim)
                .map(|values| values.iter().cloned().collect())
                .unwrap_or_default(),
            claim,
            cited: cited.into_iter().collect(),
        })
        .collect()
}

/// Fold the per-id quest stages into quests: `state` is the furthest stage
/// reached, `flow` the stages present in order.
fn fold_quests(quest_stages: QuestMap) -> Vec<ArticyQuest> {
    quest_stages
        .into_iter()
        .map(|(id, (stages, cited))| {
            let flow: Vec<String> = stages
                .iter()
                .map(|stage| stage.label().to_owned())
                .collect();
            let state = stages.iter().next_back().map_or_else(
                || QuestStage::Hook.label().to_owned(),
                |s| s.label().to_owned(),
            );
            ArticyQuest {
                id,
                state,
                flow,
                cited: cited.into_iter().collect(),
            }
        })
        .collect()
}

/// Record a stage against the quest an atom's first argument names.
fn note_quest(
    quests: &mut QuestMap,
    atom: &LoadedAtom,
    stage: QuestStage,
    with_provenance: bool,
) -> Result<(), ExportError> {
    require_arity(atom, 1)?;
    let id = required_arg(atom, 0, atom.atom.pred.text.as_str())?;
    let (stages, cited) = quests.entry(id).or_default();
    stages.insert(stage);
    if with_provenance {
        cited.extend(atom.citations.iter().cloned());
    }
    Ok(())
}

fn declared_entity_ids(atoms: &[&LoadedAtom]) -> Result<BTreeSet<String>, ExportError> {
    atoms
        .iter()
        .filter(|atom| matches!(atom.atom.pred.text.as_str(), "person" | "place" | "thing"))
        .map(|atom| {
            require_arity(atom, 1)?;
            required_arg(atom, 0, atom.atom.pred.text.as_str())
        })
        .collect()
}

fn render_args(
    atom: &LoadedAtom,
    entities: &EntityMap,
    declared_entities: &BTreeSet<String>,
) -> Result<Vec<String>, ExportError> {
    atom.atom
        .args
        .iter()
        .map(|term| {
            term_arg(term).ok_or_else(|| ExportError::Render {
                target: "articy".to_owned(),
                detail: format!(
                    "selected atom `{}` contains a non-ground argument",
                    atom.atom.pred.text
                ),
            })
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|id| render_reference(id, entities, declared_entities))
        .collect()
}

fn render_reference(
    id: String,
    entities: &EntityMap,
    declared_entities: &BTreeSet<String>,
) -> Result<String, ExportError> {
    if let Some((rendered, _, _, _)) = entities.get(&id) {
        return Ok(rendered.clone());
    }
    if declared_entities.contains(&id) {
        return Err(ExportError::Render {
            target: "articy".to_owned(),
            detail: format!(
                "selected relation references entity `{id}`, but its sort is not selected"
            ),
        });
    }
    Ok(id)
}

/// The i-th constant argument of an atom, or `None`.
fn arg(atom: &LoadedAtom, index: usize) -> Option<String> {
    atom.atom.args.get(index).and_then(term_arg)
}

fn required_arg(atom: &LoadedAtom, index: usize, predicate: &str) -> Result<String, ExportError> {
    arg(atom, index).ok_or_else(|| ExportError::Render {
        target: "articy".to_owned(),
        detail: format!(
            "selected atom `{predicate}` requires a ground argument at position {index}"
        ),
    })
}

fn require_arity(atom: &LoadedAtom, expected: usize) -> Result<(), ExportError> {
    if atom.atom.args.len() == expected {
        return Ok(());
    }
    Err(ExportError::Render {
        target: "articy".to_owned(),
        detail: format!(
            "selected atom `{}` requires {expected} argument(s), found {}",
            atom.atom.pred.text,
            atom.atom.args.len()
        ),
    })
}

/// Render a ground term as a constant string.
fn term_arg(term: &dli_ast::term::Term) -> Option<String> {
    use dli_ast::term::{Term, TimeRef};
    match term {
        Term::Sym(id) => Some(id.text.clone()),
        Term::Str(text) => Some(text.clone()),
        Term::Int(value) => Some(value.to_string()),
        Term::Time(TimeRef::Era(id)) => Some(format!("era:{}", id.text)),
        Term::Time(TimeRef::Event(id)) => Some(format!("event:{}", id.text)),
        Term::Var(_) => None,
    }
}
