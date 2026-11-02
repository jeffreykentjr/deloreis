//! Context-pack export over the certain layer (`EXP-2`).
//!
//! Source packs become deterministically ordered, headed blocks. The builder
//! drops whole trailing blocks to meet the token budget and estimates tokens as
//! characters divided by four. Upstream filtering excludes proposed facts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_eval::world::GroundAtom;
use dli_world::model::LoadedAtom;

use crate::profile::{ProfileSpec, ProvenanceMode};

/// The default token budget, for the context-pack export.
pub const DEFAULT_BUDGET_TOKENS: usize = 30_000;

/// The cheap deterministic token estimate used by the export: characters // 4.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// One unit's placement in (or exclusion from) the pack — the analogue of
/// `pack.py`'s `PackedFile`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackedUnit {
    /// The unit's identity — a source pack name (the `relpath` analogue).
    pub unit: String,
    /// The layer it was drawn from (`certain` / `fork`) — the `status` analogue.
    pub layer: String,
    /// The estimated token cost of the unit's rendered block.
    pub est_tokens: usize,
}

/// The assembled context pack plus its inclusion/exclusion manifest — the shape
/// `pack.py`'s `ContextPack` emits.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextPack {
    /// The concatenated pack text: `<!-- canon: … -->`-headed blocks joined by a
    /// blank line.
    pub text: String,
    /// The units that fit the budget, in emission order.
    pub included: Vec<PackedUnit>,
    /// The units dropped from the tail because the budget was exceeded.
    pub skipped: Vec<PackedUnit>,
    /// The estimated token cost of the whole pack (`len(text) // 4`).
    pub est_tokens: usize,
}

/// Build a context pack from a run of certain atoms under the default budget.
#[must_use]
pub fn build(spec: &ProfileSpec, layer: &str, atoms: &[&LoadedAtom]) -> ContextPack {
    build_with_budget(spec, layer, atoms, DEFAULT_BUDGET_TOKENS)
}

/// Build a context pack under an explicit token budget.
///
/// Atoms are grouped into blocks by their source pack, each block headed
/// `<!-- canon: {pack} ({layer}) -->`; blocks sort by pack name (all share the one
/// export layer); over budget, whole blocks drop from the tail, exactly as
/// `pack.py` does.
///
/// ```
/// use dli_export::context_pack::{build_with_budget, estimate_tokens};
/// use dli_export::profile::{certain_atoms, Registry, ExportTarget};
/// use dli_world::{compile, pack::Pack, loader::example_docs};
///
/// let world = compile(&Pack { name: "camelot".to_owned(), documents: example_docs() })
///     .expect("compiles");
/// let atoms = certain_atoms(&world);
/// let registry = Registry::builtin();
/// let spec = registry.default_for(ExportTarget::ContextPack).unwrap();
/// let pack = build_with_budget(spec, "certain", &atoms, 30_000);
/// // The est_tokens invariant holds by construction.
/// assert_eq!(pack.est_tokens, estimate_tokens(&pack.text));
/// // A zero budget includes nothing and skips everything present.
/// let empty = build_with_budget(spec, "certain", &atoms, 0);
/// assert!(empty.included.is_empty());
/// assert!(empty.text.is_empty());
/// ```
#[must_use]
pub fn build_with_budget(
    spec: &ProfileSpec,
    layer: &str,
    atoms: &[&LoadedAtom],
    budget_tokens: usize,
) -> ContextPack {
    // Group by source pack, deterministically (BTreeMap keys sort). Within a pack,
    // atoms sort by their rendered form.
    let mut by_pack: std::collections::BTreeMap<&str, Vec<&LoadedAtom>> =
        std::collections::BTreeMap::new();
    for atom in atoms {
        by_pack.entry(atom.pack.as_str()).or_default().push(atom);
    }

    let mut candidates: Vec<(PackedUnit, String)> = Vec::new();
    for (pack, mut members) in by_pack {
        members.sort_by_key(|atom| render_atom(atom));
        let block = render_block(pack, layer, &members, spec.carries_provenance);
        let cost = estimate_tokens(&block);
        candidates.push((
            PackedUnit {
                unit: pack.to_owned(),
                layer: layer.to_owned(),
                est_tokens: cost,
            },
            block,
        ));
    }

    let mut included = Vec::new();
    let mut skipped = Vec::new();
    let mut kept_blocks = Vec::new();
    let mut running = 0usize;
    let mut over = false;
    for (unit, block) in candidates {
        if over || running + unit.est_tokens > budget_tokens {
            over = true;
            skipped.push(unit);
        } else {
            running += unit.est_tokens;
            included.push(unit);
            kept_blocks.push(block);
        }
    }

    let text = kept_blocks.join("\n\n");
    let est_tokens = estimate_tokens(&text);
    ContextPack {
        text,
        included,
        skipped,
        est_tokens,
    }
}

/// Render one pack's block: the canon header plus one line per atom.
fn render_block(
    pack: &str,
    layer: &str,
    members: &[&LoadedAtom],
    provenance: ProvenanceMode,
) -> String {
    let mut block = format!("<!-- canon: {pack} ({layer}) -->");
    for atom in members {
        block.push('\n');
        block.push_str(&render_atom(atom));
        if matches!(provenance, ProvenanceMode::DevMetadata) && !atom.citations.is_empty() {
            block.push_str("  ; cited ");
            block.push_str(&atom.citations.join(", "));
        }
    }
    block
}

/// Render an atom's `pred(args)` form; a non-ground atom (never a fact) falls
/// back to its predicate.
fn render_atom(atom: &LoadedAtom) -> String {
    GroundAtom::from_pattern(&atom.atom)
        .map_or_else(|| atom.atom.pred.text.clone(), |ground| ground.render())
}
