//! The generic card and Ludii `L-GDL` export, driven by a profile's render DSL.
//!
//! Both renders read a `Ruleset` — rules-as-data with zero simulator state — so
//! each is a straight structural render. The card render honors the profile's
//! declared `RenderRule` (an id template plus a keep-list) over the ruleset's cards
//! and their fields, rather than hardcoding any layer; the Ludii render enumerates
//! the ruleset's cards as components, its `follows` relation as successor edges, and
//! its available layouts as terminals.
//!
//! It depends on `dli_export::profile` for the profile vocabulary
//! (`ProfileSpec`/`Registry`/`ExportTarget`/`ExportOpts`/`ExportManifest`/`ExportError`)
//! and satisfies the `Cards`/`LudiiGdl` targets the world-atom export fails closed
//! on.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_sim::digest_json;

use dli_export::ExportManifest;
use dli_export::profile::{
    ExportError, ExportOpts, ExportTarget, ProfileSpec, ProvenanceMode, Registry,
};

use crate::model::{AttrValue, Ruleset, render_template};

/// One rendered card: the export id (from the render rule), its layer, and the
/// kept fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RenderedCard {
    /// The rendered card id.
    pub id: String,
    /// The layer this card belongs to.
    pub layer: String,
    /// The fields kept per the render rule's keep-list.
    pub kept: BTreeMap<String, AttrValue>,
}

/// A compiled card deck — a straight structural render of a ruleset under a
/// profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CardDeck {
    /// The game id.
    pub deck: String,
    /// The layer exported (`certain` / `fork`).
    pub layer: String,
    /// The profile identity that produced it.
    pub profile: String,
    /// The ruleset's cited provenance, when the profile carries it (dev-metadata);
    /// `None` when stripped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cited: Option<Vec<String>>,
    /// The declared card count.
    pub card_count: usize,
    /// The rendered cards, ordered by (layer declaration, id).
    pub cards: Vec<RenderedCard>,
    /// The layouts that compile — a layout survives iff its required cards are
    /// present.
    pub layouts: Vec<String>,
}

/// The compiled diff between two decks — what the second deck lost.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeckDiff {
    /// The card count before.
    pub card_count_before: usize,
    /// The card count after.
    pub card_count_after: usize,
    /// The card ids the second deck lost, sorted.
    pub cards_removed: Vec<String>,
    /// The layouts that stopped compiling, sorted.
    pub layouts_removed: Vec<String>,
}

/// Whether the Ludii trace-agreement battery has been run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum DifferentialStatus {
    /// The battery has not run: the Ludii runtime is a rented baseline in a
    /// non-gating lane, not provisioned in the offline kernel.
    Pending {
        /// Why the differential could not run.
        blocker: String,
    },
    /// The engine render and the Ludii runtime agreed over the battery.
    Agreed {
        /// How many seeded positions agreed.
        positions: u32,
    },
}

/// A compiled Ludii `L-GDL` export: a straight structural render of the ruleset,
/// plus whether the trace-agreement differential ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LudiiExport {
    /// The rendered `L-GDL` program.
    pub program: String,
    /// The trace-agreement status.
    pub differential: DifferentialStatus,
    /// The profile identity that produced it.
    pub profile: String,
    /// The layer exported.
    pub layer: String,
}

/// The rendered artifact an export emits. Serialised untagged, so a consumer gets
/// the target's own shape with no envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GameArtifact {
    /// A compiled card deck.
    Cards(CardDeck),
    /// A Ludii `L-GDL` program plus its differential status.
    LudiiGdl(LudiiExport),
}

impl GameArtifact {
    /// The card deck, if this artifact is one.
    #[must_use]
    pub fn as_cards(&self) -> Option<&CardDeck> {
        match self {
            Self::Cards(deck) => Some(deck),
            Self::LudiiGdl(_) => None,
        }
    }

    /// The Ludii export, if this artifact is one.
    #[must_use]
    pub fn as_ludii(&self) -> Option<&LudiiExport> {
        match self {
            Self::LudiiGdl(gdl) => Some(gdl),
            Self::Cards(_) => None,
        }
    }
}

/// The outcome of an export: the rendered artifact and the manifest that pins it to
/// the exact ruleset state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameExport {
    /// The rendered, target-shaped artifact.
    pub artifact: GameArtifact,
    /// The provenance manifest pinning the artifact to the ruleset it compiled.
    pub manifest: ExportManifest,
}

/// Render a ruleset under a target, returning its artifact and manifest.
///
/// The pipeline resolves the `ProfileSpec` (default or `ExportOpts::profile_name`),
/// picks the layer (`fork` under an `ExportOpts::fork` overlay, else `certain`),
/// renders the ruleset structurally (`compile_cards` / `compile_ludii`), and pins a
/// manifest whose valuation digests the ruleset itself.
///
/// # Errors
///
/// `ExportError::UnknownProfile` / `ExportError::NoDefaultProfile` when no profile
/// resolves, `ExportError::ProfileTargetMismatch` when a named profile is for a
/// different target, and `ExportError::NoEngineRendererForTarget` for a non-card
/// target (the context pack and articy scaffold are rendered by the world-atom
/// export, not here).
pub fn run(
    registry: &Registry,
    target: ExportTarget,
    ruleset: &Ruleset,
    opts: &ExportOpts,
) -> Result<GameExport, ExportError> {
    let spec = match &opts.profile_name {
        Some(name) => registry
            .resolve(name)
            .ok_or_else(|| ExportError::UnknownProfile(name.clone()))?,
        None => registry
            .default_for(target)
            .ok_or(ExportError::NoDefaultProfile(target))?,
    };
    if spec.target != target {
        return Err(ExportError::ProfileTargetMismatch {
            name: spec.name.clone(),
            found: spec.target,
            requested: target,
        });
    }

    let layer = if opts.fork.is_some() {
        "fork"
    } else {
        "certain"
    };
    let artifact = match target {
        ExportTarget::Cards => GameArtifact::Cards(compile_cards(ruleset, spec, layer)),
        ExportTarget::LudiiGdl => GameArtifact::LudiiGdl(compile_ludii(ruleset, spec, layer)),
        ExportTarget::ContextPack | ExportTarget::Articy => {
            return Err(ExportError::NoEngineRendererForTarget(target));
        }
    };

    let manifest = ExportManifest {
        commit: opts.source_commit.clone(),
        layer: layer.to_owned(),
        valuation: digest_json(ruleset),
        profile_hash: digest_json(spec),
    };
    Ok(GameExport { artifact, manifest })
}

/// Compile a card deck from a ruleset under a profile, honoring the profile's
/// declared `RenderRule`s: each card's id is rendered from its layer's rule id
/// template, and only the fields the rule keeps are carried onto it. A card whose
/// layer has no render rule keeps its ruleset id and no fields.
#[must_use]
pub fn compile_cards(ruleset: &Ruleset, spec: &ProfileSpec, layer: &str) -> CardDeck {
    let mut cards: Vec<RenderedCard> = Vec::new();
    for card in ruleset.cards() {
        let fields = ruleset.render_fields(&card);
        let (id, kept) = match spec.renders.get(card.layer.as_str()) {
            Some(rule) => {
                let id = render_template(&rule.id, &card.bindings);
                let kept = rule
                    .keep
                    .iter()
                    .filter_map(|key| fields.get(key).map(|value| (key.clone(), value.clone())))
                    .collect();
                (id, kept)
            }
            None => (card.id.clone(), BTreeMap::new()),
        };
        cards.push(RenderedCard {
            id,
            layer: card.layer.clone(),
            kept,
        });
    }

    let order: Vec<&str> = ruleset
        .layers
        .iter()
        .map(|layer| layer.id.as_str())
        .collect();
    let rank = |layer: &str| {
        order
            .iter()
            .position(|id| *id == layer)
            .unwrap_or(usize::MAX)
    };
    cards.sort_by(|left, right| {
        (rank(left.layer.as_str()), left.id.as_str())
            .cmp(&(rank(right.layer.as_str()), right.id.as_str()))
    });

    let layouts = ruleset
        .available_layouts()
        .into_iter()
        .map(|layout| layout.name.clone())
        .collect();

    let cited = matches!(spec.carries_provenance, ProvenanceMode::DevMetadata)
        .then(|| ruleset.provenance.cites.clone());

    CardDeck {
        deck: ruleset.game_id.clone(),
        layer: layer.to_owned(),
        profile: spec.qualified(),
        cited,
        card_count: ruleset.card_count(),
        cards,
        layouts,
    }
}

/// The compiled diff of `before` against `after` — the cards and layouts the second
/// deck lost.
#[must_use]
pub fn diff_decks(before: &CardDeck, after: &CardDeck) -> DeckDiff {
    let after_ids: std::collections::BTreeSet<&str> =
        after.cards.iter().map(|card| card.id.as_str()).collect();
    let mut cards_removed: Vec<String> = before
        .cards
        .iter()
        .filter(|card| !after_ids.contains(card.id.as_str()))
        .map(|card| card.id.clone())
        .collect();
    cards_removed.sort();

    let after_layouts: std::collections::BTreeSet<&str> =
        after.layouts.iter().map(String::as_str).collect();
    let mut layouts_removed: Vec<String> = before
        .layouts
        .iter()
        .filter(|layout| !after_layouts.contains(layout.as_str()))
        .cloned()
        .collect();
    layouts_removed.sort();

    DeckDiff {
        card_count_before: before.card_count,
        card_count_after: after.card_count,
        cards_removed,
        layouts_removed,
    }
}

/// Compile a ruleset to Ludii `L-GDL` — a straight structural render of the cards
/// (components), the static successor relation (`Ruleset::follows`), and the
/// terminal layouts. It enumerates declared fields; it never computes a relation.
///
/// The trace-agreement differential against the Ludii runtime is a non-gating lane;
/// the runtime is not provisioned here, so it is recorded `Pending`.
#[must_use]
pub fn compile_ludii(ruleset: &Ruleset, spec: &ProfileSpec, layer: &str) -> LudiiExport {
    let ids = ruleset.card_ids();

    let mut program = String::new();
    let _ = writeln!(program, "(game \"{}\"", ruleset.game_id);
    program.push_str("  (players 1)\n");
    program.push_str("  (equipment {\n");
    for id in &ids {
        let _ = writeln!(program, "    (component \"{id}\")");
    }
    program.push_str("  })\n");
    program.push_str("  (rules\n    (play (moves {\n");
    for from in &ids {
        for to in &ids {
            if ruleset.follows(from, to) {
                let _ = writeln!(program, "      (move (from \"{from}\") (to \"{to}\"))");
            }
        }
    }
    program.push_str("    }))\n    (end {\n");
    for layout in ruleset.available_layouts() {
        let _ = writeln!(
            program,
            "      (if (dealt \"{}\") (result terminal))",
            layout.name
        );
    }
    program.push_str("    })\n  ))\n");

    LudiiExport {
        program,
        differential: DifferentialStatus::Pending {
            blocker: "the Ludii runtime is a rented baseline in a non-gating lane and is not \
                 provisioned in the offline kernel environment; the trace-agreement battery \
                 runs out of band"
                .to_owned(),
        },
        profile: spec.qualified(),
        layer: layer.to_owned(),
    }
}

/// The count of static successor edges in a ruleset's move relation — a structural
/// measure of the compiled `L-GDL`, for a triviality check.
#[must_use]
pub fn follow_edge_count(ruleset: &Ruleset) -> usize {
    let ids = ruleset.card_ids();
    let mut count = 0;
    for from in &ids {
        for to in &ids {
            if ruleset.follows(from, to) {
                count += 1;
            }
        }
    }
    count
}
