//! Export backend: the card/board and Ludii L-GDL game profiles (`EXP-3`).
//!
//! Both backends compile the **Turning ruleset** — `dli_sim::turning::Ruleset`,
//! rules-as-data with zero simulator state — so each is a *straight structural
//! render*. That triviality is the DL-0007 proof point (spec 09 §5): if a compile
//! ever needed to *compute* a relation rather than enumerate the ruleset's fields,
//! the rules were entangled with the kernel — the finding, not the fix.
//!
//! # The card export (spec 09 §6)
//!
//! [`compile_cards`] renders a ruleset's leaves, grounds, figures, and
//! **available** layouts to a card set. The crucial property: a layout is dealable
//! iff its required Weathers are still present ([`Ruleset::available_layouts`]), so
//! the keep-stamped deck — which cut the hard Weathers as a *lore* variant delta
//! ([`Variant::keep_stamped`](dli_sim::turning::Variant::keep_stamped)) — loses
//! `the-debt-square` **by the compile**, not by a profile author's hand. The
//! honest-vs-keep-stamped [`diff_decks`] is that loss as a shippable artifact.
//!
//! The three recompiles are the ruleset's own constructors — one profile,
//! `cards/lowlands@3`, over three variant states:
//! [`lowlands_honest`](Ruleset::lowlands_honest) (29),
//! [`salt_fringe`](Ruleset::salt_fringe) (29, re-suited),
//! [`keep_stamped`](Ruleset::keep_stamped) (26).
//!
//! # The Ludii L-GDL export (DL-0007)
//!
//! [`compile_ludii`] renders the ruleset's leaves, its static successor relation
//! ([`Ruleset::follows`]), and its terminal layouts to an L-GDL program. Soundness
//! is verified *differentially* against the Ludii runtime over seeded positions —
//! a **rented baseline in a non-gating lane** (DL-0002/DL-0013). The runtime is not
//! provisioned in the offline kernel, so [`compile_ludii`] emits the program and
//! records the differential as [`DifferentialStatus::Pending`] with the blocker.

use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_sim::turning::{HandRole, Layer as RuleLayer, Leaf as RuleLeaf, Ruleset};

use crate::profile::{ExportOpts, ProfileSpec, ProvenanceMode};

/// Select the ruleset a set of options compiles: the salt-fringe regional deck,
/// the keep-stamped variant, or the honest lowlands deck (the default). The
/// variant is **lore** (`region`/`variant` selectors), never a profile edit.
#[must_use]
pub fn ruleset_for(opts: &ExportOpts) -> Ruleset {
    match (opts.region.as_deref(), opts.variant.as_deref()) {
        (Some("salt-fringe"), _) => Ruleset::salt_fringe(),
        (_, Some("keep-stamped")) => Ruleset::keep_stamped(),
        _ => Ruleset::lowlands_honest(),
    }
}

/// The provenance stanza an export traces to (spec 09 §2/§6): the exact canon
/// state that produced the artifact.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CompiledFrom {
    /// The source commit.
    pub commit: String,
    /// The layer exported (`certain` / `fork`).
    pub layer: String,
    /// The profile identity (`cards/lowlands@3`).
    pub profile: String,
    /// The variant delta compiled, if any (`turning/keep-stamped`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Dev-metadata that ships in dev builds and is stripped for ship (spec 09 §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    /// The cited decisions backing this leaf.
    pub cited: Vec<String>,
}

/// One card leaf. Per-layer fields are optional so the JSON matches spec 09 §6
/// exactly — a ground leaf carries `name`, a hand leaf carries
/// `role`/`ground`/`figure`, a weather leaf carries `name`/`ring_next`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Leaf {
    /// The leaf id (`ground-hearth`, `hand-mender-field`, `weather-debt`).
    pub id: String,
    /// The card layer (`ground` / `hand` / `weather` / `reckoner`).
    pub layer: String,
    /// The display name, for ground/weather/reckoner leaves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The role, for a hand leaf.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The ground, for a hand leaf.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground: Option<String>,
    /// The riding folk-figure, for a hand leaf.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub figure: Option<String>,
    /// The next weather in the closed ring (`weather-fallow`), for a weather leaf.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ring_next: Option<String>,
    /// Cited provenance (dev-metadata; stripped for ship).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// A compiled card deck — the spec 09 §6 shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CardDeck {
    /// The deck id (`turning-lowlands-honest`).
    pub deck: String,
    /// What the deck was compiled from.
    pub compiled_from: CompiledFrom,
    /// The moon-count: Hands + Weathers + Reckoner ([`Ruleset::leaf_count`]).
    /// Falls out of the compile — 29 honest, 26 keep-stamped — never
    /// hand-maintained. The Ground suit emblems in [`leaves`](CardDeck::leaves) are
    /// surfaced for the consumer but not part of this count (spec 09 §6, turning's
    /// guidance).
    pub leaf_count: usize,
    /// The leaves: the Ground suit emblems, then the counted leaves, ordered by
    /// (layer, id).
    pub leaves: Vec<Leaf>,
    /// The layouts that compile — a layout survives iff its required Weathers are
    /// present.
    pub layouts: Vec<String>,
}

/// The compiled diff between two decks — the honest-vs-keep-stamped diff as a
/// first-class artifact (spec 09 §6): what the licensed decks cannot deal,
/// computed rather than asserted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeckDiff {
    /// The leaf count before (the honest deck).
    pub leaf_count_before: usize,
    /// The leaf count after (the keep-stamped deck).
    pub leaf_count_after: usize,
    /// The leaf ids the variant cut, sorted.
    pub leaves_removed: Vec<String>,
    /// The layouts that stopped compiling, sorted.
    pub layouts_removed: Vec<String>,
}

/// The compiled diff of `before` against `after` — the leaves and layouts the
/// second deck lost.
///
/// ```
/// use dli_sim::turning::Ruleset;
/// use dli_export::games::{compile_cards, diff_decks};
/// use dli_export::profile::{ExportOpts, Registry, ExportTarget};
///
/// let registry = Registry::builtin();
/// let spec = registry.default_for(ExportTarget::Cards).unwrap();
/// let honest = compile_cards(&Ruleset::lowlands_honest(), spec, "certain", &ExportOpts::default());
/// let keep = compile_cards(&Ruleset::keep_stamped(), spec, "certain", &ExportOpts::default());
///
/// let diff = diff_decks(&honest, &keep);
/// assert_eq!((diff.leaf_count_before, diff.leaf_count_after), (29, 26));
/// assert_eq!(diff.layouts_removed, vec!["the-debt-square".to_owned()]);
/// ```
#[must_use]
pub fn diff_decks(before: &CardDeck, after: &CardDeck) -> DeckDiff {
    let after_ids: std::collections::BTreeSet<&str> =
        after.leaves.iter().map(|leaf| leaf.id.as_str()).collect();
    let mut leaves_removed: Vec<String> = before
        .leaves
        .iter()
        .filter(|leaf| !after_ids.contains(leaf.id.as_str()))
        .map(|leaf| leaf.id.clone())
        .collect();
    leaves_removed.sort();

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
        leaf_count_before: before.leaf_count,
        leaf_count_after: after.leaf_count,
        leaves_removed,
        layouts_removed,
    }
}

/// Compile a card deck from a Turning ruleset under a profile — a straight
/// structural render (spec 09 §6, DL-0007 triviality).
#[must_use]
pub fn compile_cards(
    ruleset: &Ruleset,
    spec: &ProfileSpec,
    layer: &str,
    opts: &ExportOpts,
) -> CardDeck {
    let with_provenance = matches!(spec.carries_provenance, ProvenanceMode::DevMetadata);
    let cites = &ruleset.provenance.cites;

    let mut leaves: Vec<Leaf> = Vec::new();

    // Ground suit emblems (spec 09 §6; not part of the counted moon-count).
    for ground in &ruleset.grounds {
        leaves.push(Leaf {
            id: format!("ground-{}", ground.id),
            layer: "ground".to_owned(),
            name: Some(ground.name.clone()),
            ..Leaf::default()
        });
    }

    // The counted leaves, rendered per layer.
    for leaf in ruleset.leaves() {
        let id = leaf.id();
        match &leaf {
            RuleLeaf::Hand { role, ground } => leaves.push(Leaf {
                id,
                layer: "hand".to_owned(),
                role: Some(role.id().to_owned()),
                ground: Some(ground.clone()),
                figure: ruleset.figure_for(&leaf).map(|fig| fig.id.clone()),
                meta: with_provenance.then(|| Meta {
                    cited: cites.clone(),
                }),
                ..Leaf::default()
            }),
            RuleLeaf::Weather { weather } => {
                let found = ruleset.weathers.iter().find(|w| &w.id == weather);
                leaves.push(Leaf {
                    id,
                    layer: "weather".to_owned(),
                    name: found.map(|w| w.name.clone()),
                    ring_next: found.map(|w| format!("weather-{}", w.ring_next)),
                    ..Leaf::default()
                });
            }
            RuleLeaf::Reckoner => leaves.push(Leaf {
                id,
                layer: "reckoner".to_owned(),
                name: Some("The Reckoner".to_owned()),
                ..Leaf::default()
            }),
        }
    }

    leaves.sort_by(|a, b| (layer_rank(&a.layer), &a.id).cmp(&(layer_rank(&b.layer), &b.id)));

    let layouts = ruleset
        .available_layouts()
        .into_iter()
        .map(|layout| layout.name.clone())
        .collect();

    CardDeck {
        deck: ruleset.deck_id.clone(),
        compiled_from: CompiledFrom {
            commit: opts.source_commit.clone(),
            layer: layer.to_owned(),
            profile: spec.qualified(),
            variant: variant_of(&ruleset.deck_id),
        },
        leaf_count: ruleset.leaf_count(),
        leaves,
        layouts,
    }
}

/// The variant path a deck id names, or `None` for the honest lowlands deck.
fn variant_of(deck_id: &str) -> Option<String> {
    match deck_id {
        "turning-lowlands-keep-stamped" => Some("turning/keep-stamped".to_owned()),
        "turning-salt-fringe-honest" => Some("turning/salt-fringe".to_owned()),
        _ => None,
    }
}

/// The sort order of a card layer, so leaves emit ground → hand → weather →
/// reckoner (spec 09 §6).
fn layer_rank(layer: &str) -> u8 {
    match layer {
        "ground" => 0,
        "hand" => 1,
        "weather" => 2,
        "reckoner" => 3,
        _ => 4,
    }
}

// -- Ludii L-GDL (DL-0007) --------------------------------------------------

/// Whether the Ludii trace-agreement battery has been run (spec 09 §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum DifferentialStatus {
    /// The battery has not run: the Ludii runtime is a rented baseline in a
    /// non-gating lab lane (DL-0002/DL-0013), not provisioned in this environment.
    Pending {
        /// Why the differential could not run.
        blocker: String,
    },
    /// The simulator and the Ludii runtime agreed over the battery.
    Agreed {
        /// How many seeded positions agreed.
        positions: u32,
    },
}

/// A compiled Ludii L-GDL export (`EXP-3`, the DL-0007 proof point).
///
/// The `program` is a straight structural render of the ruleset; the
/// `differential` records whether the trace-agreement battery ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LudiiExport {
    /// The rendered L-GDL program.
    pub program: String,
    /// The trace-agreement status.
    pub differential: DifferentialStatus,
    /// The profile identity that produced it.
    pub profile: String,
    /// The layer exported.
    pub layer: String,
}

/// Compile a ruleset to Ludii L-GDL — a straight structural render of the leaves,
/// the static successor relation ([`Ruleset::follows`]), and the terminal layouts.
/// It enumerates declared fields; it never computes a relation, which is the
/// DL-0007 triviality property (spec 09 §5).
///
/// The trace-agreement differential against the Ludii runtime is a non-gating lab
/// lane; the runtime is not provisioned here, so it is recorded
/// [`Pending`](DifferentialStatus::Pending).
#[must_use]
pub fn compile_ludii(ruleset: &Ruleset, spec: &ProfileSpec, layer: &str) -> LudiiExport {
    let ids: Vec<String> = ruleset.leaves().iter().map(RuleLeaf::id).collect();

    let mut program = String::new();
    let _ = writeln!(program, "(game \"{}\"", ruleset.deck_id);
    program.push_str("  (players 1)\n");
    program.push_str("  (equipment {\n");
    for id in &ids {
        let _ = writeln!(program, "    (component \"{id}\")");
    }
    program.push_str("  })\n");
    program.push_str("  (rules\n    (play (moves {\n");
    // The static successor relation, enumerated (no computation).
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
            blocker: "the Ludii runtime is a rented baseline in a non-gating lab lane \
                 (DL-0002/DL-0013) and is not provisioned in the offline kernel \
                 environment; the trace-agreement battery runs lab-side"
                .to_owned(),
        },
        profile: spec.qualified(),
        layer: layer.to_owned(),
    }
}

/// The count of static successor edges in a ruleset's legal-move relation — a
/// structural measure of the compiled L-GDL, for the triviality check.
#[must_use]
pub fn follow_edge_count(ruleset: &Ruleset) -> usize {
    let ids: Vec<String> = ruleset.leaves().iter().map(RuleLeaf::id).collect();
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

/// The four Hand-roles, exposed so a downstream renderer need not import the
/// simulator crate to name them.
#[must_use]
pub fn hand_roles() -> [HandRole; 4] {
    HandRole::ALL
}

/// The card layer a ruleset leaf renders into (`hand` / `weather` / `reckoner`);
/// Ground emblems render as `ground` outside the counted leaves.
#[must_use]
pub fn layer_name(layer: RuleLayer) -> &'static str {
    match layer {
        RuleLayer::Hand => "hand",
        RuleLayer::Weather => "weather",
        RuleLayer::Reckoner => "reckoner",
    }
}
