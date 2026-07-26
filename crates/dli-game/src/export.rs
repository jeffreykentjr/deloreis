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

use std::collections::{BTreeMap, BTreeSet};
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
/// renders an already materialized, assertable ruleset structurally
/// (`compile_cards` / `compile_ludii`), and pins a manifest whose valuation digests
/// the ruleset itself.
///
/// # Errors
///
/// `ExportError::UnknownProfile` / `ExportError::NoDefaultProfile` when no profile
/// resolves, `ExportError::ProfileTargetMismatch` when a named profile is for a
/// different target, `ExportError::UnsupportedOption` for lore-world options this
/// renderer cannot materialize, `ExportError::UnassertableSource` for a proposed
/// ruleset, and `ExportError::NoEngineRendererForTarget` for a non-game target.
pub fn run(
    registry: &Registry,
    target: ExportTarget,
    ruleset: &Ruleset,
    opts: &ExportOpts,
) -> Result<GameExport, ExportError> {
    if opts.source_commit.trim().is_empty() {
        return Err(ExportError::MissingSourceAnchor);
    }
    if !ruleset.is_assertable() {
        return Err(ExportError::UnassertableSource { target });
    }
    if opts.fork.is_some() {
        return Err(ExportError::UnsupportedOption {
            target,
            option: "fork",
        });
    }
    if !opts.keep_stamps.is_empty() {
        return Err(ExportError::UnsupportedOption {
            target,
            option: "keep_stamps",
        });
    }
    if opts.region.is_some() {
        return Err(ExportError::UnsupportedOption {
            target,
            option: "region",
        });
    }
    if opts.variant.is_some() {
        return Err(ExportError::UnsupportedOption {
            target,
            option: "variant",
        });
    }
    let spec = registry.resolve_for(target, opts.profile_name.as_deref())?;
    let layer = "certain";
    let artifact = match target {
        ExportTarget::Cards => GameArtifact::Cards(compile_cards(ruleset, spec, layer)?),
        ExportTarget::LudiiGdl => GameArtifact::LudiiGdl(compile_ludii(ruleset, spec, layer)?),
        ExportTarget::ContextPack
        | ExportTarget::Articy
        | ExportTarget::Csv
        | ExportTarget::StaticWiki => {
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
pub fn compile_cards(
    ruleset: &Ruleset,
    spec: &ProfileSpec,
    layer: &str,
) -> Result<CardDeck, ExportError> {
    validate_game_profile(ruleset, spec)?;
    let selected_layers = selected_layers(ruleset, spec);
    let mut cards: Vec<RenderedCard> = Vec::new();
    let mut source_ids = BTreeSet::new();
    for card in ruleset
        .cards()
        .into_iter()
        .filter(|card| selected_layers.contains(&card.layer))
    {
        source_ids.insert(card.id.clone());
        let fields = ruleset.render_fields(&card);
        let (id, kept) = match spec.renders.get(card.layer.as_str()) {
            Some(rule) => {
                let id = if rule.id.is_empty() {
                    card.id.clone()
                } else {
                    render_template(&rule.id, &card.bindings)
                };
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
    let unique_ids = cards
        .iter()
        .map(|card| card.id.as_str())
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != cards.len() {
        return Err(render_error(
            spec,
            "more than one selected card renders to the same id",
        ));
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
        .layouts
        .iter()
        .filter(|layout| {
            layout
                .requires_cards
                .iter()
                .all(|card| source_ids.contains(card))
        })
        .map(|layout| layout.name.clone())
        .collect();

    let cited = matches!(spec.carries_provenance, ProvenanceMode::DevMetadata)
        .then(|| ruleset.provenance.cites.clone());

    Ok(CardDeck {
        deck: ruleset.game_id.clone(),
        layer: layer.to_owned(),
        profile: spec.qualified(),
        cited,
        card_count: cards.len(),
        cards,
        layouts,
    })
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
pub fn compile_ludii(
    ruleset: &Ruleset,
    spec: &ProfileSpec,
    layer: &str,
) -> Result<LudiiExport, ExportError> {
    validate_game_profile(ruleset, spec)?;
    let selected_layers = selected_layers(ruleset, spec);
    let ids = ruleset
        .cards()
        .into_iter()
        .filter(|card| selected_layers.contains(&card.layer))
        .map(|card| card.id)
        .collect::<Vec<_>>();
    let wildcard = spec.selects.sorts.is_empty() && spec.selects.relations.is_empty();
    let emit_moves = wildcard
        || spec
            .selects
            .relations
            .iter()
            .any(|relation| relation == "legal-move");
    let emit_terminals = wildcard
        || spec
            .selects
            .relations
            .iter()
            .any(|relation| relation == "terminal");

    let mut program = String::new();
    let _ = writeln!(
        program,
        "(game \"{}\"",
        escape_ludii_string(&ruleset.game_id)
    );
    program.push_str("  (players 1)\n");
    program.push_str("  (equipment {\n");
    for id in &ids {
        let _ = writeln!(program, "    (component \"{}\")", escape_ludii_string(id));
    }
    program.push_str("  })\n");
    program.push_str("  (rules\n    (play (moves {\n");
    if emit_moves {
        for from in &ids {
            for to in &ids {
                if ruleset.follows(from, to) {
                    let _ = writeln!(
                        program,
                        "      (move (from \"{}\") (to \"{}\"))",
                        escape_ludii_string(from),
                        escape_ludii_string(to)
                    );
                }
            }
        }
    }
    program.push_str("    }))\n    (end {\n");
    if emit_terminals {
        let selected_ids = ids.iter().collect::<BTreeSet<_>>();
        for layout in &ruleset.layouts {
            if layout
                .requires_cards
                .iter()
                .all(|card| selected_ids.contains(card))
            {
                let _ = writeln!(
                    program,
                    "      (if (dealt \"{}\") (result terminal))",
                    escape_ludii_string(&layout.name)
                );
            }
        }
    }
    program.push_str("    })\n  ))\n");

    Ok(LudiiExport {
        program,
        differential: DifferentialStatus::Pending {
            blocker: "the Ludii runtime is a rented baseline in a non-gating lane and is not \
                 provisioned in the offline kernel environment; the trace-agreement battery \
                 runs out of band"
                .to_owned(),
        },
        profile: spec.qualified(),
        layer: layer.to_owned(),
    })
}

/// Escape one caller-controlled value for a quoted Ludii string literal.
fn escape_ludii_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control.is_control() => {
                let _ = write!(escaped, "\\u{:04X}", u32::from(control));
            }
            printable => escaped.push(printable),
        }
    }
    escaped
}

/// Validate one card/Ludii profile against the concrete ruleset it will render.
///
/// Structural registry validation cannot know a caller ruleset's axes, layers,
/// relations, or available card fields. This target-specific pass closes that
/// gap for project loading and direct library callers.
pub fn validate_game_profile(ruleset: &Ruleset, spec: &ProfileSpec) -> Result<(), ExportError> {
    if !matches!(spec.target, ExportTarget::Cards | ExportTarget::LudiiGdl) {
        return Err(render_error(
            spec,
            "game profile validation requires a cards or ludii-gdl target",
        ));
    }

    let available_sorts = ruleset
        .axes
        .iter()
        .map(|axis| axis.id.as_str())
        .chain(ruleset.layers.iter().map(|layer| layer.id.as_str()))
        .collect::<BTreeSet<_>>();
    if let Some(sort) = spec
        .selects
        .sorts
        .iter()
        .find(|sort| !available_sorts.contains(sort.as_str()))
    {
        return Err(render_error(
            spec,
            format!("selected sort `{sort}` is not declared by the ruleset"),
        ));
    }
    let available_relations = ruleset
        .relations
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if let Some(relation) = spec
        .selects
        .relations
        .iter()
        .find(|relation| !available_relations.contains(relation.as_str()))
    {
        return Err(render_error(
            spec,
            format!("selected relation `{relation}` is not declared by the ruleset"),
        ));
    }

    let selected_layers = selected_layers(ruleset, spec);
    if selected_layers.is_empty() {
        return Err(render_error(spec, "the profile selects no card layer"));
    }
    validate_selected_layer_sources(ruleset, spec, &selected_layers)?;
    if matches!(spec.target, ExportTarget::LudiiGdl) && !spec.renders.is_empty() {
        return Err(render_error(
            spec,
            "ludii-gdl profiles do not accept card render rules",
        ));
    }
    for (layer_id, rule) in &spec.renders {
        if !selected_layers.contains(layer_id) {
            return Err(render_error(
                spec,
                format!("render rule `{layer_id}` does not name a selected card layer"),
            ));
        }
        let layer = ruleset
            .layers
            .iter()
            .find(|layer| &layer.id == layer_id)
            .ok_or_else(|| {
                render_error(
                    spec,
                    format!("render rule `{layer_id}` does not name a card layer"),
                )
            })?;
        for placeholder in
            game_template_placeholders(&rule.id).map_err(|detail| render_error(spec, detail))?
        {
            if !layer.axes.contains(&placeholder) {
                return Err(render_error(
                    spec,
                    format!("render rule `{layer_id}` id names unavailable field `{placeholder}`"),
                ));
            }
        }
        let cards = ruleset
            .cards()
            .into_iter()
            .filter(|card| card.layer == *layer_id)
            .collect::<Vec<_>>();
        let available_fields = cards
            .iter()
            .flat_map(|card| ruleset.render_fields(card).into_keys())
            .collect::<BTreeSet<_>>();
        if let Some(field) = rule
            .keep
            .iter()
            .find(|field| !available_fields.contains(field.as_str()))
        {
            return Err(render_error(
                spec,
                format!("render rule `{layer_id}` keeps unavailable field `{field}`"),
            ));
        }
    }
    Ok(())
}

fn validate_selected_layer_sources(
    ruleset: &Ruleset,
    spec: &ProfileSpec,
    selected_layers: &BTreeSet<String>,
) -> Result<(), ExportError> {
    for layer in ruleset
        .layers
        .iter()
        .filter(|layer| selected_layers.contains(&layer.id))
    {
        let mut seen_axes = BTreeSet::new();
        if let Some(axis) = layer.axes.iter().find(|axis| {
            !seen_axes.insert(axis.as_str())
                || !ruleset.axes.iter().any(|candidate| candidate.id == **axis)
        }) {
            return Err(render_error(
                spec,
                format!(
                    "selected card layer `{}` references missing or duplicate axis `{axis}`",
                    layer.id
                ),
            ));
        }
        for placeholder in game_template_placeholders(&layer.id_template)
            .map_err(|detail| render_error(spec, detail))?
        {
            if !layer.axes.contains(&placeholder) {
                return Err(render_error(
                    spec,
                    format!(
                        "selected card layer `{}` id names unavailable field `{placeholder}`",
                        layer.id
                    ),
                ));
            }
        }
    }
    let source_cards = ruleset
        .cards()
        .into_iter()
        .filter(|card| selected_layers.contains(&card.layer))
        .collect::<Vec<_>>();
    let source_ids = source_cards
        .iter()
        .map(|card| card.id.as_str())
        .collect::<BTreeSet<_>>();
    if source_ids.len() != source_cards.len() {
        return Err(render_error(
            spec,
            "more than one selected source card has the same id",
        ));
    }
    Ok(())
}

fn selected_layers(ruleset: &Ruleset, spec: &ProfileSpec) -> BTreeSet<String> {
    let wildcard = spec.selects.sorts.is_empty() && spec.selects.relations.is_empty();
    ruleset
        .layers
        .iter()
        .filter(|layer| wildcard || spec.selects.sorts.contains(&layer.id))
        .map(|layer| layer.id.clone())
        .collect()
}

fn game_template_placeholders(template: &str) -> Result<Vec<String>, String> {
    let mut placeholders = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        if rest[..open].contains('}') {
            return Err(format!(
                "render template `{template}` has an unmatched `}}`"
            ));
        }
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('}') else {
            return Err(format!(
                "render template `{template}` has an unmatched `{{`"
            ));
        };
        let field = &after_open[..close];
        if field.is_empty() || field.contains('{') {
            return Err(format!(
                "render template `{template}` has an invalid placeholder"
            ));
        }
        placeholders.push(field.to_owned());
        rest = &after_open[close + 1..];
    }
    if rest.contains('}') {
        return Err(format!(
            "render template `{template}` has an unmatched `}}`"
        ));
    }
    Ok(placeholders)
}

fn render_error(spec: &ProfileSpec, detail: impl Into<String>) -> ExportError {
    ExportError::InvalidProfile {
        profile: spec.qualified(),
        detail: detail.into(),
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
