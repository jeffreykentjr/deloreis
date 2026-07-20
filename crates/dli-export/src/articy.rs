//! Export backend: the articy-shaped video-game profile (`EXP-4`).
//!
//! articy:draft X is the commercial proof of the seat this fills (spec 09 §1): a
//! narrative-production database of typed entities, dialogues, quests, and
//! variables, exported as JSON for Unity/Unreal importers. De Lore Is is the
//! missing **authoritative** layer that feeds such a tool, upgraded with the
//! provenance machinery articy lacks.
//!
//! This is a **scaffold** (real body next sub-wave): it establishes the shape and
//! the two mappings that make it more than a data dump (spec 09 §3), and it wires
//! the **shared keep-stamp visibility filter**, which has already run upstream in
//! [`export`](crate::export) before these atoms arrive — so a who-may-not-know
//! atom is simply absent, and the honest-vs-keep-stamped diff of two articy
//! exports is the who-may-know-what gate a dialogue system needs, as data.
//!
//! The two mappings (documented, minimally realised here):
//! - **keep-stamp visibility → who-may-know-what.** Which claims a faction, region,
//!   or character is licensed to know exports as [`ArticyFact::who_may_know`].
//! - **promise-ledger/debt → quest logs.** A promise is a hook, an open debt an
//!   objective, a discharge a completion ([`ArticyQuest`]).
//!
//! Provenance citations travel as dev-metadata and are **stripped for ship**
//! (spec 09 §3), governed by the profile's
//! [`carries_provenance`](crate::profile::ProfileSpec::carries_provenance).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_eval::world::GroundAtom;
use dli_world::model::LoadedAtom;

use crate::profile::{ProfileSpec, ProvenanceMode};

/// A typed articy entity (a person, place, or thing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArticyEntity {
    /// The entity id.
    pub id: String,
    /// The entity's sort (`person` / `place` / `thing`).
    pub sort: String,
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
/// discharge a completion (spec 09 §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArticyQuest {
    /// The quest id (the promise).
    pub id: String,
    /// The quest state (`hook` / `objective` / `complete`).
    pub state: String,
}

/// An articy-shaped export: typed entities, relations, dialogue facts with
/// knowledge scopes, and quest structure.
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
    /// Whether the export is still a scaffold (the rich body lands next sub-wave).
    pub pending: bool,
    /// The profile identity that produced it.
    pub profile: String,
    /// The layer exported.
    pub layer: String,
}

/// Compile a scaffold articy export from the certain visible atoms.
///
/// A minimal structural map: `person`/`place`/`thing` atoms become entities,
/// `knows(scope, claim)` atoms attach who-may-know scopes to facts, and
/// `promise`/`debt`/`discharge` atoms become quests. The keep-stamp filter already
/// ran upstream, so any who-may-not-know atom is absent from `atoms`. The rich
/// entity/dialogue/variable modelling is deferred; `pending` is `true`.
#[must_use]
pub fn compile_scaffold(spec: &ProfileSpec, layer: &str, atoms: &[&LoadedAtom]) -> ArticyExport {
    let with_provenance = matches!(spec.carries_provenance, ProvenanceMode::DevMetadata);

    let mut entities = Vec::new();
    let mut relations = Vec::new();
    let mut quests = Vec::new();
    // who-may-know scopes keyed by the claim they license.
    let mut scopes: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    let mut facts: Vec<(String, Vec<String>)> = Vec::new();

    for atom in atoms {
        let pred = atom.atom.pred.text.as_str();
        let rendered = GroundAtom::from_pattern(&atom.atom).map(|ground| ground.render());
        match pred {
            "person" | "place" | "thing" => {
                if let Some(id) = arg(atom, 0) {
                    entities.push(ArticyEntity {
                        id,
                        sort: pred.to_owned(),
                    });
                }
            }
            "knows" => {
                if let (Some(scope), Some(claim)) = (arg(atom, 0), arg(atom, 1)) {
                    scopes.entry(claim).or_default().push(scope);
                }
            }
            "promise" => {
                if let Some(id) = arg(atom, 0) {
                    quests.push(ArticyQuest {
                        id,
                        state: "hook".to_owned(),
                    });
                }
            }
            "debt" => {
                if let Some(id) = arg(atom, 0) {
                    quests.push(ArticyQuest {
                        id,
                        state: "objective".to_owned(),
                    });
                }
            }
            "discharge" => {
                if let Some(id) = arg(atom, 0) {
                    quests.push(ArticyQuest {
                        id,
                        state: "complete".to_owned(),
                    });
                }
            }
            _ => {
                if let Some(claim) = rendered {
                    let cited = if with_provenance {
                        atom.citations.clone()
                    } else {
                        Vec::new()
                    };
                    facts.push((claim, cited));
                    if atom.atom.args.len() >= 2 {
                        relations.push(ArticyRelation {
                            kind: pred.to_owned(),
                            args: atom.atom.args.iter().filter_map(term_arg).collect(),
                        });
                    }
                }
            }
        }
    }

    let facts = facts
        .into_iter()
        .map(|(claim, cited)| ArticyFact {
            who_may_know: scopes.get(&claim).cloned().unwrap_or_default(),
            claim,
            cited,
        })
        .collect();

    ArticyExport {
        entities,
        relations,
        facts,
        quests,
        pending: true,
        profile: spec.qualified(),
        layer: layer.to_owned(),
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
